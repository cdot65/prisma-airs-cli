import { describe, expect, it } from 'vitest';
import {
  buildFilteringProfileBody,
  buildPatternBody,
  buildProfileBody,
  repeatable,
} from '../../../src/cli/commands/dlp/build-body.js';

describe('buildPatternBody', () => {
  it('requires --name', () => {
    expect(() => buildPatternBody({})).toThrow(/--name/);
  });

  it('defaults type=custom, technique=regex', () => {
    const body = buildPatternBody({ name: 'X' });
    expect(body).toMatchObject({
      name: 'X',
      type: 'custom',
      detection_config: { technique: 'regex' },
    });
  });

  it('builds matching_rules with regexes (weight 1)', () => {
    const body = buildPatternBody({
      name: 'X',
      regex: ['ABC', 'DEF'],
    });
    expect(body.matching_rules).toEqual({
      regexes: [
        { regex: 'ABC', weight: 1 },
        { regex: 'DEF', weight: 1 },
      ],
    });
  });

  it('parses weighted-regex PATTERN|N', () => {
    const body = buildPatternBody({
      name: 'X',
      weightedRegex: ['ABC|2', 'DEF|5'],
    });
    expect(body.matching_rules).toEqual({
      regexes: [
        { regex: 'ABC', weight: 2 },
        { regex: 'DEF', weight: 5 },
      ],
    });
  });

  it('weighted-regex splits on LAST pipe', () => {
    const body = buildPatternBody({ name: 'X', weightedRegex: ['A|B|3'] });
    expect(
      (body.matching_rules as { regexes: { regex: string; weight: number }[] }).regexes[0],
    ).toEqual({
      regex: 'A|B',
      weight: 3,
    });
  });

  it('weighted-regex rejects missing pipe', () => {
    expect(() => buildPatternBody({ name: 'X', weightedRegex: ['NOPIPE'] })).toThrow(
      /PATTERN\|weight/,
    );
  });

  it('weighted-regex rejects non-numeric weight', () => {
    expect(() => buildPatternBody({ name: 'X', weightedRegex: ['ABC|nope'] })).toThrow(
      /weight invalid/,
    );
  });

  it('parses confidence-levels CSV', () => {
    const body = buildPatternBody({
      name: 'X',
      confidenceLevels: 'high, low',
    });
    expect(body.detection_config).toEqual({
      technique: 'regex',
      supported_confidence_levels: ['high', 'low'],
    });
  });

  it('parses tags with comma-separated values', () => {
    const body = buildPatternBody({
      name: 'X',
      tag: ['classification=pab,endpoint', 'region=us'],
    });
    expect(body.tags).toEqual({
      classification: ['pab', 'endpoint'],
      region: ['us'],
    });
  });

  it('rejects tag without "="', () => {
    expect(() => buildPatternBody({ name: 'X', tag: ['nope'] })).toThrow(/key=value/);
  });

  it('includes delimiter, proximity_distance, proximity_keywords', () => {
    const body = buildPatternBody({
      name: 'X',
      delimiter: ';',
      proximityDistance: '100',
      proximityKeyword: ['acct', 'account'],
    });
    expect(body.matching_rules).toEqual({
      delimiter: ';',
      proximity_distance: 100,
      proximity_keywords: ['acct', 'account'],
    });
  });
});

describe('buildFilteringProfileBody', () => {
  it('requires both --file-based and --non-file-based', () => {
    expect(() => buildFilteringProfileBody({})).toThrow(/file-based/);
    expect(() => buildFilteringProfileBody({ fileBased: true })).toThrow(/file-based/);
  });

  it('builds minimal body', () => {
    expect(buildFilteringProfileBody({ fileBased: true, nonFileBased: false })).toEqual({
      file_based: true,
      non_file_based: false,
    });
  });

  it('includes optional flat fields', () => {
    const body = buildFilteringProfileBody({
      fileBased: true,
      nonFileBased: true,
      description: 'd',
      direction: 'BOTH',
      logSeverity: 'HIGH',
      scanType: 'include',
      dataProfileId: '12345',
      eucTemplateId: 'tmpl-1',
      endUserCoaching: true,
      granular: false,
      fileType: ['pdf', 'docx'],
    });
    expect(body).toEqual({
      file_based: true,
      non_file_based: true,
      description: 'd',
      direction: 'BOTH',
      log_severity: 'HIGH',
      scan_type: 'include',
      data_profile_id: 12345,
      euc_template_id: 'tmpl-1',
      is_end_user_coaching_enabled: true,
      is_granular_profile: false,
      file_type: ['pdf', 'docx'],
    });
  });
});

describe('buildProfileBody', () => {
  const patterns = [
    { id: 'p1', name: 'First', version: 3, technique: 'regex', confidenceLevels: ['high', 'low'] },
    {
      id: 'p2',
      name: 'Second',
      version: 2,
      technique: 'weighted_regex',
      confidenceLevels: ['high'],
    },
  ];
  it('requires --name', () => {
    expect(() => buildProfileBody({})).toThrow(/--name/);
  });

  it('enforces the live 32-character profile-name boundary', () => {
    expect(buildProfileBody({ name: 'A'.repeat(32) }).name).toBe('A'.repeat(32));
    expect(() => buildProfileBody({ name: 'A'.repeat(33) })).toThrow(/at most 32 characters/);
    expect(() => buildProfileBody({ name: '   ' })).toThrow(/nonblank string/);
  });

  it('defaults profile_type=advanced', () => {
    expect(buildProfileBody({ name: 'X' })).toEqual({
      name: 'X',
      profile_type: 'advanced',
    });
  });

  it('builds expression_tree from --pattern-id with default OR combinator', () => {
    const body = buildProfileBody({ name: 'X', patternId: ['p1', 'p2'] }, patterns);
    const rules = (
      body.detection_rules as { rule_type: string; expression_tree: { operator_type: string } }[]
    )[0];
    expect(rules.rule_type).toBe('expression_tree');
    expect(rules.expression_tree).toEqual({
      operator_type: 'or',
      sub_expressions: patterns.map((pattern) => ({
        rule_item: {
          id: pattern.id,
          name: pattern.name,
          version: pattern.version,
          detection_technique: pattern.technique,
          confidence_level: 'high',
          supported_confidence_levels: pattern.confidenceLevels,
          match_type: 'include',
          occurrence_operator_type: 'more_than_equal_to',
          occurrence_count: 1,
        },
      })),
    });
  });

  it('uses custom combinator', () => {
    const body = buildProfileBody({ name: 'X', patternId: ['p1'], combinator: 'AND' }, patterns);
    const rules = (body.detection_rules as { expression_tree: { operator_type: string } }[])[0];
    expect(rules.expression_tree.operator_type).toBe('and');
  });

  it('requires every pattern to resolve and rejects unsupported confidence', () => {
    expect(() => buildProfileBody({ name: 'X', patternId: ['missing'] }, patterns)).toThrow(
      /must resolve/,
    );
    expect(() =>
      buildProfileBody({ name: 'X', patternId: ['p2'], confidence: 'low' }, patterns),
    ).toThrow(/not supported/);
  });

  it('rejects basic writes and invalid confidence before building rules', () => {
    expect(() => buildProfileBody({ name: 'X', profileType: 'basic' })).toThrow(
      /Basic profile writes are unsupported/,
    );
    expect(() => buildProfileBody({ name: 'X', profileType: 'unknown' })).toThrow(
      /must be advanced/,
    );
    expect(() => buildProfileBody({ name: 'X', confidence: 'unknown' })).toThrow(/--confidence/);
  });

  it('rejects invalid combinator', () => {
    expect(() => buildProfileBody({ name: 'X', patternId: ['p1'], combinator: 'xor' })).toThrow(
      /combinator/,
    );
  });
});

describe('repeatable', () => {
  it('accumulates values', () => {
    expect(repeatable('b', ['a'])).toEqual(['a', 'b']);
    expect(repeatable('a')).toEqual(['a']);
  });
});
