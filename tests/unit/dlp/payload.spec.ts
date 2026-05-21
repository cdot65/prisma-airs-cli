import { describe, expect, it } from 'vitest';
import { luhnValid, makePayload } from '../../../src/dlp/payload.js';
import { makeRng } from '../../../src/dlp/rng.js';

const get = (rng: () => number, category: string): string =>
  makePayload(rng).find((v) => v.category === category)?.value ?? '';

describe('luhnValid', () => {
  it('accepts a known-valid PAN and rejects a tampered one', () => {
    expect(luhnValid('4111111111111111')).toBe(true);
    expect(luhnValid('4111111111111112')).toBe(false);
    expect(luhnValid('')).toBe(false);
  });
});

describe('makePayload', () => {
  it('is deterministic by seed', () => {
    expect(makePayload(makeRng(5))).toEqual(makePayload(makeRng(5)));
  });

  it('emits a Luhn-valid Visa-family test PAN', () => {
    const pan = get(makeRng(11), 'credit_card').replace(/\s/g, '');
    expect(pan).toMatch(/^4\d{15}$/);
    expect(luhnValid(pan)).toBe(true);
  });

  it('uses only reserved / example ranges and no real-PII shapes', () => {
    const all = makePayload(makeRng(13))
      .map((v) => v.value)
      .join(' ');
    expect(all).toMatch(/@example\.(com|org|net)/);
    expect(all).toMatch(/\b555-01\d\d\b/);
    expect(all).toMatch(/\b9\d\d-\d\d-\d{4}\b/); // 900-range SSN, never issued
    expect(all).toMatch(/AKIA[A-Z2-7]{9}EXAMPLE/);
    expect(all).not.toMatch(/sk_live_/); // avoid GitHub push-protection trigger
  });

  it('covers all expected categories', () => {
    const cats = makePayload(makeRng(1)).map((v) => v.category);
    expect(cats).toEqual(
      expect.arrayContaining([
        'ssn',
        'credit_card',
        'email',
        'phone',
        'aws_key',
        'passport',
        'dob',
      ]),
    );
  });
});
