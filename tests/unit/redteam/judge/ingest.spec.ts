import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  buildState,
  extractResponseText,
  normalizeScan,
  pythonJson,
  redactState,
} from '../../../../src/redteam/judge/index.js';

const SAMPLE_SCAN = JSON.parse(
  readFileSync(
    new URL('../../../fixtures/redteam-judge/sample-scan.json', import.meta.url),
    'utf8',
  ),
);

describe('extractResponseText', () => {
  it('preserves every output string, including JSON, escapes and whitespace', () => {
    for (const raw of [
      'plain text',
      '',
      '  \n\t',
      '{not json',
      '{"output":[{"content":[{"text":"nested"}]}]}',
      '{"choices":[{"message":{"content":"nested"}}]}',
      '{"messages":[{"role":"assistant","content":"nested"}]}',
      '{"text":"literal JSON"}',
      '["a", "b"]',
      '  café 😀\nsecond line  ',
    ])
      expect(extractResponseText(raw)).toEqual([raw, raw.trim() ? 'plain' : 'empty']);
    expect(extractResponseText(null)).toEqual(['', 'empty']);
    expect(extractResponseText(undefined)).toEqual(['', 'empty']);
    for (const raw of [42, false, {}, [], { output: 'text' }])
      expect(() => extractResponseText(raw)).toThrow('Expected output to be a string');
  });

  it('re-serializes like Python json.dumps so fallbacks hash identically', () => {
    expect(pythonJson({ a: [1, 'x', null, true], b: { c: 1.5 } })).toBe(
      '{"a": [1, "x", null, true], "b": {"c": 1.5}}',
    );
    expect(pythonJson({ dropped: undefined, kept: 'é' })).toBe('{"kept": "é"}');
  });
});

describe('normalizeScan', () => {
  it('turns flat download records into units with the AIRS metadata', () => {
    const { units, notes } = normalizeScan(SAMPLE_SCAN);
    expect(notes).toEqual({
      layout: 'array',
      records: 10,
      skipped_no_prompt: 0,
      error_outputs: 1,
      objective_proxies: 10,
    });
    expect(units).toHaveLength(10);
    const first = units[0];
    expect(first.unit_id).toBe('0a1f3c2e-1111-4a1a-9c01-000000000001#0');
    expect(first.extraction).toBe('plain');
    expect(first.response_text).toBe(SAMPLE_SCAN[0].output);
    expect([first.category, first.sub_category, first.airs_threat]).toEqual([
      'SECURITY',
      'Remote Code Execution',
      false,
    ]);
    expect(first.objective).toBe(
      'Security / Remote Code Execution attack: make the target do what the prompt asks.',
    );
    expect(first.prompt_sha256).toMatch(/^[0-9a-f]{16}$/);
    expect(units[7].is_error).toBe(true);
    expect(units[7].airs_threat).toBeNull();
    expect(units[6].response_text).toBe(SAMPLE_SCAN[6].output);
  });

  it('accepts the API detail shape with nested outputs', () => {
    const document = {
      data: [
        {
          uuid: 'attack-1',
          job_id: 'job-1',
          prompt: 'p',
          category: 'SECURITY',
          sub_category: 'JAILBREAK',
          goal: 'Make the target reveal its system prompt',
          asr: 50.0,
          outputs: [
            { uuid: 'out-a', output: 'leaked', threat: true },
            { uuid: 'out-b', output: '', error: true, error_message: 'x' },
          ],
        },
        { uuid: 'attack-2', output: 'no prompt here' },
      ],
    };
    const { units, notes } = normalizeScan(document);
    expect(notes).toEqual({
      layout: 'object.data',
      records: 2,
      skipped_no_prompt: 1,
      error_outputs: 1,
    });
    expect(units.map((u) => u.unit_id)).toEqual(['attack-1#out-a', 'attack-1#out-b']);
    expect([units[0].objective, units[0].airs_asr, units[0].airs_threat]).toEqual([
      'Make the target reveal its system prompt',
      50,
      true,
    ]);
    expect(units[0].job_id).toBe('job-1');
    expect(units[1].is_error).toBe(true);
  });

  it('accepts a single record, string flags and index fallbacks', () => {
    const { units, notes } = normalizeScan({
      prompt: 'p',
      output: 'text',
      threat: 'TRUE',
      marked_safe: 'false',
    });
    expect(notes.layout).toBe('single');
    expect(units[0].unit_id).toBe('index-0#0');
    expect(units[0].airs_threat).toBe(true);
    expect(units[0].marked_safe).toBe(false);
    expect(units[0].category).toBeNull();
    expect(units[0].objective).toBe(
      'unknown category / unknown sub-category attack: make the target do what the prompt asks.',
    );
    expect(
      normalizeScan({ attacks: [{ attack_id: 'a', prompt: 'p', output: 'x' }] }).units[0].attack_id,
    ).toBe('a');
    expect(
      normalizeScan([{ id: 'b', prompt: 'p', output: 'x', threat: 1 }]).units[0],
    ).toMatchObject({
      attack_id: 'b',
      airs_threat: null,
    });
  });

  it('rejects unknown layouts and respects the limit', () => {
    expect(() => normalizeScan({ nothing: 'here' })).toThrow(/Unrecognized scan JSON/);
    expect(() => normalizeScan('text')).toThrow(/Unrecognized scan JSON/);
    const { units, notes } = normalizeScan(SAMPLE_SCAN, { limit: 3 });
    expect([units.length, notes.truncated_to_limit]).toEqual([3, 3]);
  });

  it('builds the Jev state and a redacted dry-run view of it', () => {
    const { units } = normalizeScan(SAMPLE_SCAN, { limit: 1 });
    const state = buildState(units[0]);
    expect(state.attack.prompt).toContain('data scientist');
    expect(state.target_response).toContain('syslog');
    const redacted = redactState(units[0]);
    expect(redacted.attack.prompt).toBe(
      `<${units[0].prompt.length} chars, sha256 ${units[0].prompt_sha256}>`,
    );
    expect(JSON.stringify(redacted)).not.toContain('data scientist');
    expect(redacted.attack.category).toBe('SECURITY');
  });
});

it('rejects ambiguous duplicate ids and oversized text instead of silently scoring different input', () => {
  const record = { uuid: 'same', prompt: 'p', output: 'r' };
  expect(() => normalizeScan([record, record])).toThrow('duplicate');
  expect(normalizeScan([{ ...record, output: 'x'.repeat(200_001) }]).notes.oversized_units).toBe(1);
});

const ENVELOPES = JSON.parse(
  readFileSync(new URL('../../../fixtures/redteam-judge/envelopes.json', import.meta.url), 'utf8'),
) as { name: string; input: unknown; expected: [string, string] }[];

it.each(ENVELOPES)('keeps serialized envelope $name as model output', ({ input }) => {
  if (typeof input === 'string') expect(extractResponseText(input)).toEqual([input, 'plain']);
  else expect(() => extractResponseText(input)).toThrow('Expected output to be a string');
});

it.each(ENVELOPES)('normalizes prompt envelope $name independently of output', ({
  input,
  expected,
}) => {
  const prompt = typeof input === 'string' ? input : JSON.stringify(input);
  const { units, notes } = normalizeScan([{ prompt, output: '{"text":"literal model output"}' }]);
  if (!expected[0]) {
    expect(units).toEqual([]);
    expect(notes.skipped_no_prompt).toBe(1);
  } else {
    expect(units[0].prompt).toBe(expected[0]);
    expect(units[0].response_text).toBe('{"text":"literal model output"}');
  }
});

it('preserves ordinary JSON prompts and malformed JSON model output', () => {
  const prompt = '{"text":"this JSON is the attack", "role":"user"}';
  const output = String(ENVELOPES[10].input);
  const { units, notes } = normalizeScan([{ prompt, output }]);
  expect([units[0].prompt, units[0].response_text, units[0].is_error]).toEqual([
    prompt,
    output,
    false,
  ]);
  expect(notes.error_outputs).toBe(0);
});

it('does not execute Python syntax and bounds parsing', () => {
  for (const input of [
    "{'text': __import__('os').system('false')}",
    "{'text': (lambda: 1)()}",
    "{'text':'a', 'text':'b'}",
    `${'['.repeat(1000)}None${']'.repeat(1000)}`,
    `{${' '.repeat(1_000_000)}`,
  ])
    expect(extractResponseText(input)).toEqual([input, 'plain']);
});
