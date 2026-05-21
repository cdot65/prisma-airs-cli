import { digits, pick } from './rng.js';
import type { PayloadValue } from './types.js';

const BASE32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

/** Luhn checksum validation for a digit string. */
export function luhnValid(pan: string): boolean {
  if (pan.length === 0) {
    return false;
  }
  let sum = 0;
  let alt = false;
  for (let i = pan.length - 1; i >= 0; i--) {
    let d = pan.charCodeAt(i) - 48;
    if (alt) {
      d *= 2;
      if (d > 9) {
        d -= 9;
      }
    }
    sum += d;
    alt = !alt;
  }
  return sum % 10 === 0;
}

/** A 16-digit, Luhn-valid PAN from the Visa-family test BIN (starts with 4). */
function testPan(rng: () => number): string {
  const body = `4${digits(rng, 14)}`;
  for (let c = 0; c < 10; c++) {
    const candidate = `${body}${c}`;
    if (luhnValid(candidate)) {
      return candidate;
    }
  }
  return `${body}0`;
}

// SSNs in area 900-999 are never issued by the SSA, so these are synthetic-safe.
const ssn = (rng: () => number): string => `9${digits(rng, 2)}-${digits(rng, 2)}-${digits(rng, 4)}`;
const phone = (rng: () => number): string => `555-01${digits(rng, 2)}`;
const email = (rng: () => number): string =>
  `${pick(rng, ['john.public', 'jane.doe', 'sam.roe'])}.${digits(rng, 3)}@${pick(rng, [
    'example.com',
    'example.org',
    'example.net',
  ])}`;
const awsKey = (rng: () => number): string =>
  `AKIA${Array.from({ length: 9 }, () => pick(rng, [...BASE32])).join('')}EXAMPLE`;
const dob = (rng: () => number): string => {
  const year = 1960 + Math.floor(rng() * 40);
  const month = String(1 + Math.floor(rng() * 12)).padStart(2, '0');
  const day = String(1 + Math.floor(rng() * 28)).padStart(2, '0');
  return `${year}-${month}-${day}`;
};

/** Generate a set of clearly-synthetic, format-valid sensitive markers. */
export function makePayload(rng: () => number): PayloadValue[] {
  const pan = testPan(rng);
  return [
    { category: 'ssn', value: ssn(rng) },
    { category: 'credit_card', value: pan.replace(/(\d{4})(?=\d)/g, '$1 ').trim() },
    { category: 'email', value: email(rng) },
    { category: 'phone', value: phone(rng) },
    { category: 'aws_key', value: awsKey(rng) },
    { category: 'passport', value: `X${digits(rng, 8)}` },
    { category: 'dob', value: dob(rng) },
  ];
}
