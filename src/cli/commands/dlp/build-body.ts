/**
 * Body-builders for DLP CLI commands. Each function takes parsed commander opts
 * and returns a plain object suitable for the SDK request. Validation is light —
 * Zod in the SDK does the heavy lifting on the wire.
 */

// biome-ignore lint/suspicious/noExplicitAny: commander opts are loosely typed
type Opts = Record<string, any>;

function asCsv(v: string | undefined): string[] | undefined {
  if (!v) return undefined;
  return v
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
}

function asNum(v: string | undefined): number | undefined {
  if (v == null) return undefined;
  const n = Number(v);
  return Number.isFinite(n) ? n : undefined;
}

function parseTags(values: string[] | undefined): Record<string, string[]> | undefined {
  if (!values || values.length === 0) return undefined;
  const out: Record<string, string[]> = {};
  for (const v of values) {
    const eq = v.indexOf('=');
    if (eq < 0) throw new Error(`--tag must be 'key=value' (got '${v}')`);
    const key = v.slice(0, eq).trim();
    const val = v.slice(eq + 1).trim();
    if (!key) throw new Error(`--tag missing key (got '${v}')`);
    if (!out[key]) out[key] = [];
    for (const part of val
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)) {
      out[key].push(part);
    }
  }
  return out;
}

function parseRegexes(
  plain: string[] | undefined,
  weighted: string[] | undefined,
): { regex: string; weight: number }[] | undefined {
  const out: { regex: string; weight: number }[] = [];
  for (const r of plain ?? []) out.push({ regex: r, weight: 1 });
  for (const r of weighted ?? []) {
    const pipe = r.lastIndexOf('|');
    if (pipe < 0) throw new Error(`--weighted-regex must be 'PATTERN|weight' (got '${r}')`);
    const pattern = r.slice(0, pipe);
    const weight = Number(r.slice(pipe + 1));
    if (!Number.isFinite(weight)) throw new Error(`--weighted-regex weight invalid in '${r}'`);
    out.push({ regex: pattern, weight });
  }
  return out.length > 0 ? out : undefined;
}

/**
 * Build a Data Pattern request body from CLI flags.
 * `requireRequired` enforces name + type (use false for replace where caller already validated).
 */
export function buildPatternBody(opts: Opts, requireRequired = true): Record<string, unknown> {
  if (requireRequired) {
    if (!opts.name) throw new Error('--name is required');
  }
  const technique = opts.technique ?? 'regex';
  const confidence = asCsv(opts.confidenceLevels);
  const regexes = parseRegexes(opts.regex, opts.weightedRegex);
  const tags = parseTags(opts.tag);

  const detectionConfig: Record<string, unknown> = { technique };
  if (confidence) detectionConfig.supported_confidence_levels = confidence;

  const matchingRules: Record<string, unknown> = {};
  if (opts.delimiter != null) matchingRules.delimiter = opts.delimiter;
  if (opts.proximityDistance != null)
    matchingRules.proximity_distance = asNum(opts.proximityDistance);
  if (opts.proximityKeyword?.length) matchingRules.proximity_keywords = opts.proximityKeyword;
  if (regexes) matchingRules.regexes = regexes;

  const body: Record<string, unknown> = {
    name: opts.name,
    type: opts.type ?? 'custom',
    detection_config: detectionConfig,
  };
  if (opts.description != null) body.description = opts.description;
  if (Object.keys(matchingRules).length > 0) body.matching_rules = matchingRules;
  if (tags) body.tags = tags;
  return body;
}

/**
 * Build a Data Filtering Profile request body from CLI flags.
 * Required: --file-based, --non-file-based (booleans).
 */
export function buildFilteringProfileBody(opts: Opts): Record<string, unknown> {
  if (opts.fileBased == null || opts.nonFileBased == null) {
    throw new Error('--file-based and --non-file-based are both required');
  }
  const body: Record<string, unknown> = {
    file_based: !!opts.fileBased,
    non_file_based: !!opts.nonFileBased,
  };
  if (opts.description != null) body.description = opts.description;
  if (opts.direction != null) body.direction = opts.direction;
  if (opts.logSeverity != null) body.log_severity = opts.logSeverity;
  if (opts.scanType != null) body.scan_type = opts.scanType;
  if (opts.dataProfileId != null) body.data_profile_id = asNum(opts.dataProfileId);
  if (opts.eucTemplateId != null) body.euc_template_id = opts.eucTemplateId;
  if (opts.endUserCoaching != null) body.is_end_user_coaching_enabled = !!opts.endUserCoaching;
  if (opts.granular != null) body.is_granular_profile = !!opts.granular;
  if (opts.fileType?.length) body.file_type = opts.fileType;
  return body;
}

export interface ResolvedProfilePattern {
  id: string;
  name: string;
  version: number;
  technique: string;
  confidenceLevels?: string[] | null;
}

export function validateProfileType(value: unknown): void {
  // Live 2026-09-11: POST with profile_type basic returned advanced (transfer acceptance log).
  if (value === 'basic') {
    throw new Error(
      'Basic profile writes are unsupported: the API returned advanced for basic requests. Use advanced or manage basic profiles in SCM.',
    );
  }
  if (value !== undefined && value !== 'advanced') {
    throw new Error('Profile type must be advanced');
  }
}

export function validateProfileName(name: unknown): void {
  if (typeof name !== 'string' || !name.trim()) {
    throw new Error('Profile name is required and must be a nonblank string');
  }
  // Live 2026-09-11: 32-character name created profile 11995036; 33 returned HTTP 400.
  // docs-site/docs/runtime/dlp/transfer.md records the identical-body boundary probe.
  // The published SDK schema allows 64; preserve the stricter observed API limit here.
  if (name.length > 32) throw new Error('Profile name must be at most 32 characters');
}

export function validateProfileFlags(opts: Opts): void {
  if (!opts.name) throw new Error('--name is required');
  validateProfileName(opts.name);
  validateProfileType(opts.profileType);
  if (
    !['and', 'or', 'not', 'and_not', 'or_not'].includes((opts.combinator ?? 'or').toLowerCase())
  ) {
    throw new Error('--combinator must be one of and|or|not|and_not|or_not');
  }
  if (!['low', 'medium', 'high'].includes(opts.confidence ?? 'high')) {
    throw new Error('--confidence must be low|medium|high');
  }
}

/** Pure builder: references must already have been resolved by the command's GETs. */
export function buildProfileBody(
  opts: Opts,
  patterns: ResolvedProfilePattern[] = [],
): Record<string, unknown> {
  validateProfileFlags(opts);
  const body: Record<string, unknown> = {
    name: opts.name,
    profile_type: 'advanced',
  };
  if (opts.description != null) body.description = opts.description;
  if (opts.granular != null) body.is_granular_data_profile = !!opts.granular;
  if (opts.patternId?.length) {
    const confidence = opts.confidence ?? 'high';
    const leaves = opts.patternId.map((id: string) => {
      const pattern = patterns.find((item) => item.id === id);
      if (!pattern)
        throw new Error('Every --pattern-id must resolve before building profile rules');
      if (pattern.confidenceLevels?.length && !pattern.confidenceLevels.includes(confidence)) {
        throw new Error('Requested confidence is not supported by every referenced pattern');
      }
      return {
        rule_item: {
          id: pattern.id,
          name: pattern.name,
          version: pattern.version,
          detection_technique: pattern.technique,
          confidence_level: confidence,
          supported_confidence_levels: pattern.confidenceLevels?.length
            ? [...pattern.confidenceLevels]
            : [confidence],
          match_type: 'include',
          occurrence_operator_type: 'more_than_equal_to',
          occurrence_count: 1,
        },
      };
    });
    // Live 2026-09-11 source profile 11995031: sub_expressions/rule_item round-trips;
    // the old condition_pattern/data_pattern_id shape is not a supported detection leaf.
    body.detection_rules = [
      {
        rule_type: 'expression_tree',
        expression_tree: {
          operator_type: (opts.combinator ?? 'or').toLowerCase(),
          sub_expressions: leaves,
        },
      },
    ];
  }
  return body;
}

/**
 * Collect repeatable options. Use as the commander parser function.
 */
export function repeatable(value: string, prev: string[] = []): string[] {
  return [...prev, value];
}
