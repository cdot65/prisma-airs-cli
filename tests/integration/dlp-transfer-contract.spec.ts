import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';
import type { DataPatternResponse, DataProfileResponse } from '@cdot65/prisma-airs-sdk';
import type { DlpResourcesBackup } from '../../src/backup/dlp-resources.js';
import { writeTestRegistry } from '../helpers/tenant.js';

const exec = promisify(execFile);
const secretKeyword = 'PRIVATE-DICTIONARY-KEYWORD';
const privateRegex = 'PRIVATE-REGEX-[0-9]{6}';
const collections = ['dictionaries', 'data-patterns', 'data-profiles'] as const;
type Collection = (typeof collections)[number];
type RecordValue = Record<string, unknown>;
let directory: string;
let server: Server;
let env: NodeJS.ProcessEnv;
let requests: Array<{ method: string; path: string; body: RecordValue | undefined }>;
let records: Record<Collection, RecordValue[]>;
let rejectedDictionary: string | undefined;

function predefined(id: string, name: string): DataPatternResponse {
  return { id, name, type: 'predefined', version: 1, detection_config: { technique: 'regex' } };
}
function profile(name: string, patterns: DataPatternResponse[]): DataProfileResponse {
  return {
    id: `source-${name}`,
    name,
    type: 'custom',
    profile_type: 'advanced',
    profile_status: 'active',
    tenant_id: 'source-tenant',
    detection_rules: [
      {
        rule_type: 'expression_tree',
        expression_tree: {
          operator_type: 'or',
          sub_expressions: patterns.map((pattern) => ({
            rule_item: {
              detection_technique: 'regex',
              id: pattern.id,
              name: pattern.name,
              version: pattern.version,
              confidence_level: 'high',
              match_type: 'include',
              occurrence_operator_type: 'any',
              occurrence_count: 1,
            },
          })),
        },
      },
    ],
  };
}
function envelope(
  patterns: DataPatternResponse[],
  profiles: DataProfileResponse[],
): DlpResourcesBackup {
  return {
    version: 1,
    resourceType: 'dlp-resources',
    exportedAt: '2026-09-11T00:00:00.000Z',
    source: { tsgId: '100' },
    dictionaries: [],
    patterns,
    profiles,
  };
}
async function restore(backup: DlpResourcesBackup, flags: string[] = [], expectedCode = 0) {
  const file = join(directory, 'backup.json');
  await writeFile(file, JSON.stringify(backup), { mode: 0o600 });
  const entry = process.env.AIRS_CLI_ENTRY
    ? [resolve(process.env.AIRS_CLI_ENTRY)]
    : ['--import', resolve('node_modules/tsx/dist/loader.mjs'), resolve('src/cli/index.ts')];
  let result: { stdout: string; stderr: string; code?: number };
  try {
    result = await exec(
      process.execPath,
      [
        ...entry,
        'runtime',
        'dlp',
        'restore',
        file,
        '--expect-tsg',
        '200',
        '--force',
        '--output',
        'json',
        ...flags,
      ],
      {
        cwd: directory,
        env,
        timeout: 20000,
      },
    );
  } catch (error) {
    result = error as typeof result;
  }
  expect(result.code ?? 0, result.stderr).toBe(expectedCode);
  expect(result.stdout + result.stderr).not.toMatch(
    new RegExp(`FAKE-SECRET|mock-token|${secretKeyword}|PRIVATE-REGEX|PRIVATE-ERROR-PAYLOAD`),
  );
  expect(requests.some(({ method }) => ['PUT', 'PATCH', 'DELETE'].includes(method))).toBe(false);
  return result;
}
function writes(collection?: Collection) {
  return requests.filter(
    ({ method, path }) =>
      method === 'POST' &&
      path !== '/oauth/token' &&
      (collection === undefined || path.endsWith(`/${collection}`)),
  );
}
function report(stdout: string): RecordValue {
  const result = JSON.parse(stdout);
  return Array.isArray(result) ? result[0] : result;
}
function createdLeaves(index = 0): RecordValue[] {
  const body = writes('data-profiles')[index].body as DataProfileResponse;
  const rule = body.detection_rules?.[0] as {
    expression_tree: { sub_expressions: Array<{ rule_item: RecordValue }> };
  };
  return rule.expression_tree.sub_expressions.map(({ rule_item }) => rule_item);
}

beforeEach(async () => {
  directory = await mkdtemp(join(tmpdir(), 'airs-dlp-transfer-'));
  requests = [];
  records = { dictionaries: [], 'data-patterns': [], 'data-profiles': [] };
  rejectedDictionary = undefined;
  server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(Buffer.from(chunk));
    const raw = Buffer.concat(chunks);
    const url = new URL(req.url ?? '/', 'http://localhost');
    const path = url.pathname;
    const method = req.method ?? '';
    res.setHeader('Content-Type', 'application/json');
    if (path === '/oauth/token') {
      requests.push({ method, path, body: undefined });
      if (new URLSearchParams(raw.toString()).get('scope') !== 'tsg_id:200') {
        res.writeHead(401).end('{}');
        return;
      }
      res.end(
        JSON.stringify({ access_token: 'mock-token', expires_in: 3600, token_type: 'Bearer' }),
      );
      return;
    }
    if (req.headers.authorization !== 'Bearer mock-token') {
      res.writeHead(401).end('{}');
      return;
    }
    const match = /^\/dlp\/v2\/api\/(dictionaries|data-patterns|data-profiles)(?:\/([^/]+))?$/.exec(
      path,
    );
    if (!match) {
      res.writeHead(404).end('{}');
      return;
    }
    const collection = match[1] as Collection;
    let body: RecordValue | undefined;
    let keywords: string[] = [];
    if (method === 'POST') {
      if (collection === 'dictionaries') {
        const form = await new Response(raw, {
          headers: {
            'Content-Type': req.headers['content-type'] ?? '',
          },
        }).formData();
        body = JSON.parse(await (form.get('json') as Blob).text());
        keywords = (await (form.get('file') as Blob).text()).split('\n').filter(Boolean);
      } else body = JSON.parse(raw.toString());
    }
    requests.push({ method, path, body });
    if (method === 'GET' && !match[2]) {
      if (url.searchParams.get('sort') !== 'name,asc' || url.searchParams.get('size') !== '100') {
        res.writeHead(400).end('{}');
        return;
      }
      const content = records[collection];
      res.end(
        JSON.stringify({
          content,
          totalElements: content.length,
          totalPages: content.length ? 1 : 0,
          number: 0,
          size: 100,
          last: true,
        }),
      );
    } else if (method === 'GET') {
      const record = records[collection].find(({ id }) => id === match[2]);
      if (!record) res.writeHead(404).end('{}');
      else res.end(JSON.stringify(record));
    } else if (method === 'POST' && body) {
      if (collection === 'dictionaries' && body.name === rejectedDictionary) {
        res.setHeader('Content-Type', 'application/problem+json');
        res.writeHead(400).end(
          JSON.stringify({
            title: 'Bad Request',
            detail: `PRIVATE-ERROR-PAYLOAD ${secretKeyword}`,
            errors: [{ property: 'file', reason: secretKeyword }],
          }),
        );
        return;
      }
      const record = {
        ...body,
        id: `dest-${collection}-${records[collection].length}`,
        version: 7,
        type: 'custom',
        tenant_id: 'destination-tenant',
      };
      if (collection === 'dictionaries') {
        const { original_file_name, ...metadata } = record;
        records[collection].push({
          ...metadata,
          keywords,
          dictionary_metadata: { original_file_name },
        });
      } else records[collection].push(record);
      res.end(JSON.stringify(records[collection].at(-1)));
    } else res.writeHead(405).end('{}');
  });
  await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  env = Object.fromEntries(
    Object.entries(process.env).filter(
      ([key]) => !key.startsWith('PANW_') && !key.startsWith('PRISMA_AIRS_'),
    ),
  );
  Object.assign(env, { PRISMA_AIRS_TENANTS_PATH: join(directory, 'tenants.json'), NO_COLOR: '1' });
  await writeTestRegistry(
    String(env.PRISMA_AIRS_TENANTS_PATH),
    [
      {
        name: 'dev',
        config: {
          mgmtClientId: 'client-dev',
          mgmtClientSecret: 'FAKE-SECRET',
          mgmtTsgId: '200',
          mgmtTokenEndpoint: `${base}/oauth/token`,
          dlpEndpoint: `${base}/dlp`,
        },
      },
    ],
    'dev',
  );
});
afterEach(async () => {
  await new Promise<void>((done) => server.close(() => done()));
  await rm(directory, { recursive: true, force: true });
});

// Live sorted prod/dev catalogs on 2026-09-11 have matching predefined names.
// Missing-catalog cases here deliberately use HTTP fixtures, not fabricated live evidence.
describe('DLP transfer CLI/SDK HTTP contract', () => {
  it('aggregates missing references and ranks suggestions without creating any resources', async () => {
    const passport = predefined('source-passport', 'Passport - UK');
    const tax = predefined('source-tax', 'Tax - ID');
    records['data-patterns'] = [
      predefined('passport-enhanced', 'Passport - UK Enhanced'),
      predefined('passport-normalized', 'Passport UK'),
      predefined('tax-normalized', 'Tax ID'),
    ];
    const result = await restore(
      envelope([passport, tax], [profile('Missing both', [passport, tax])]),
      [],
      1,
    );
    expect(result.stderr).toContain('Passport - UK');
    expect(result.stderr).toContain('Tax - ID');
    expect(result.stderr).toContain('Passport UK');
    expect(result.stderr.indexOf('"Passport UK"')).toBeLessThan(
      result.stderr.indexOf('"Passport - UK Enhanced"'),
    );
    expect(result.stderr).toContain('Tax ID');
    expect(writes()).toEqual([]);
  });

  it('honors explicit pattern maps and verifies the created profile through re-GET', async () => {
    const source = predefined('source-passport', 'Passport - UK');
    records['data-patterns'] = [predefined('chosen-destination', 'Passport UK')];
    const result = await restore(envelope([source], [profile('Mapped profile', [source])]), [
      '--pattern-map',
      'Passport - UK=Passport UK',
    ]);
    expect(report(result.stdout).complete).toBe(true);
    expect(writes()).toHaveLength(1);
    expect(createdLeaves()).toEqual([
      expect.objectContaining({ id: 'chosen-destination', name: 'Passport UK' }),
    ]);
    expect(requests.at(-1)).toMatchObject({
      method: 'GET',
      path: '/dlp/v2/api/data-profiles/dest-data-profiles-0',
    });
  });

  it('skips the whole mixed profile while restoring an independent resolvable profile', async () => {
    const present = predefined('source-present', 'Present Pattern');
    const missing = predefined('source-missing', 'Missing Pattern');
    records['data-patterns'] = [predefined('destination-present', 'Present Pattern')];
    const result = await restore(
      envelope(
        [present, missing],
        [profile('Mixed profile', [present, missing]), profile('Independent profile', [present])],
      ),
      ['--skip-unresolved'],
    );
    const output = report(result.stdout);
    expect(output.complete).toBe(true);
    expect(output.profiles).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ name: 'Mixed profile', action: 'skipped' }),
        expect.objectContaining({ name: 'Independent profile', action: 'created' }),
      ]),
    );
    expect(writes('data-profiles').map(({ body }) => body?.name)).toEqual(['Independent profile']);
    expect(createdLeaves()).toEqual([expect.objectContaining({ id: 'destination-present' })]);
    expect(result.stderr).toContain('Skipping data profile: Mixed profile');
  });

  it('reports a verified dictionary create then stops on HTTP 400 before patterns/profiles without leaking keywords', async () => {
    const custom = {
      ...predefined('source-custom', 'Custom Pattern'),
      type: 'custom',
      matching_rules: { regexes: [{ regex: privateRegex, weight: 1 }] },
    };
    const backup = envelope([custom], [profile('Pending profile', [custom])]);
    backup.dictionaries = ['A created dictionary', 'B rejected dictionary'].map((name, i) => ({
      id: `source-dictionary-${i}`,
      name,
      type: 'custom',
      category: 'Confidential',
      region_name: 'United States',
      is_case_sensitive: false,
      keywords: [secretKeyword],
      dictionary_metadata: { original_file_name: 'keywords.txt' },
    }));
    rejectedDictionary = 'B rejected dictionary';
    const result = await restore(backup, [], 1);
    const output = report(result.stdout);
    expect(output.complete).toBe(false);
    expect(output.dictionaries).toEqual([
      expect.objectContaining({ name: 'A created dictionary', action: 'created' }),
    ]);
    expect(output.patterns).toEqual([]);
    expect(output.profiles).toEqual([]);
    expect(writes().map(({ path }) => path)).toEqual([
      '/dlp/v2/api/dictionaries',
      '/dlp/v2/api/dictionaries',
    ]);
    expect(
      requests.some(
        ({ method, path }) => method === 'GET' && path.endsWith('/dest-dictionaries-0'),
      ),
    ).toBe(true);
    expect(result.stderr).toContain('400');
  });

  it('restores an existing v1 envelope with custom IDs remapped and predefined identity/name bindings intact', async () => {
    const custom = {
      ...predefined('source-custom', 'Custom Pattern'),
      type: 'custom',
      matching_rules: { regexes: [{ regex: privateRegex, weight: 1 }] },
    };
    const identity = predefined('shared-id', 'Shared Identity');
    const renamedId = predefined('source-name-id', 'Same Name');
    records['data-patterns'] = [identity, predefined('destination-name-id', 'Same Name')];
    const result = await restore(
      envelope(
        [custom, identity, renamedId],
        [profile('Mixed bindings', [custom, identity, renamedId])],
      ),
    );
    expect(report(result.stdout).complete).toBe(true);
    expect(writes().map(({ path }) => path)).toEqual([
      '/dlp/v2/api/data-patterns',
      '/dlp/v2/api/data-profiles',
    ]);
    expect(createdLeaves().map(({ id }) => id)).toEqual([
      'dest-data-patterns-2',
      'shared-id',
      'destination-name-id',
    ]);
    const patternWrite = requests.findIndex(
      ({ method, path }) => method === 'POST' && path.endsWith('/data-patterns'),
    );
    expect(requests[patternWrite + 1]).toMatchObject({
      method: 'GET',
      path: '/dlp/v2/api/data-patterns/dest-data-patterns-2',
    });
    expect(requests.at(-1)).toMatchObject({
      method: 'GET',
      path: '/dlp/v2/api/data-profiles/dest-data-profiles-0',
    });
  });
});
