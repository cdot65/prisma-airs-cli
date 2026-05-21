import { pick } from './rng.js';

const WORDS = [
  'lorem',
  'ipsum',
  'dolor',
  'sit',
  'amet',
  'consectetur',
  'adipiscing',
  'elit',
  'sed',
  'do',
  'eiusmod',
  'tempor',
  'incididunt',
  'ut',
  'labore',
  'et',
  'dolore',
  'magna',
  'aliqua',
  'enim',
  'ad',
  'minim',
  'veniam',
  'quis',
  'nostrud',
  'exercitation',
  'ullamco',
  'laboris',
  'nisi',
  'aliquip',
  'ex',
  'ea',
  'commodo',
  'consequat',
  'duis',
  'aute',
  'irure',
  'in',
  'reprehenderit',
  'voluptate',
  'velit',
  'esse',
  'cillum',
  'fugiat',
  'nulla',
  'pariatur',
  'excepteur',
  'sint',
];

function sentence(rng: () => number): string {
  const len = 8 + Math.floor(rng() * 8);
  const words = Array.from({ length: len }, () => pick(rng, WORDS));
  const text = words.join(' ');
  return `${text.charAt(0).toUpperCase()}${text.slice(1)}.`;
}

/** Generate `paragraphs` paragraphs of lorem ipsum, joined by blank lines. */
export function lorem(rng: () => number, paragraphs: number): string {
  return Array.from({ length: paragraphs }, () => {
    const sentences = 3 + Math.floor(rng() * 3);
    return Array.from({ length: sentences }, () => sentence(rng)).join(' ');
  }).join('\n\n');
}
