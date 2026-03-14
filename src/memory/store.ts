import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import type { TopicMemory } from './types.js';

const STOP_WORDS = new Set([
  'a',
  'an',
  'the',
  'and',
  'or',
  'of',
  'about',
  'for',
  'in',
  'on',
  'to',
  'with',
  'that',
  'this',
  'is',
  'are',
  'was',
  'were',
]);

/**
 * Normalize a topic description into a stable category key.
 * Removes stop words, lowercases, deduplicates, and sorts alphabetically.
 * @param description - Free-text topic description.
 * @returns Hyphen-joined keyword string used as the memory file name.
 */
export function normalizeCategory(description: string): string {
  const words = description
    .toLowerCase()
    .replace(/[^a-z0-9\s]/g, '')
    .split(/\s+/)
    .filter((w) => w.length > 0 && !STOP_WORDS.has(w));

  const unique = [...new Set(words)].sort();
  return unique.length > 0 ? unique.join('-') : 'uncategorized';
}

/**
 * File-based persistence for cross-run learnings.
 * Stores one JSON file per category at `{dir}/{category}.json`.
 * Supports keyword-overlap-based retrieval for cross-topic transfer.
 */
export class MemoryStore {
  constructor(private dir: string) {}

  private filePath(category: string): string {
    return join(this.dir, `${category}.json`);
  }

  /** Persist a topic memory to disk, creating the directory if needed. */
  async save(memory: TopicMemory): Promise<void> {
    await mkdir(this.dir, { recursive: true });
    await writeFile(this.filePath(memory.category), JSON.stringify(memory, null, 2), 'utf-8');
  }

  /** Load a topic memory by category, or null if not found. */
  async load(category: string): Promise<TopicMemory | null> {
    try {
      const data = await readFile(this.filePath(category), 'utf-8');
      return JSON.parse(data) as TopicMemory;
    } catch {
      return null;
    }
  }

  /** Find all topic memories with >= 50% keyword overlap to the given description. */
  async findRelevant(topicDescription: string): Promise<TopicMemory[]> {
    const targetCategory = normalizeCategory(topicDescription);
    const categories = await this.listCategories();
    const results: TopicMemory[] = [];

    for (const cat of categories) {
      if (this.categoriesOverlap(targetCategory, cat)) {
        const memory = await this.load(cat);
        if (memory) results.push(memory);
      }
    }

    return results;
  }

  /** List all stored category names. */
  async listCategories(): Promise<string[]> {
    try {
      const files = await readdir(this.dir);
      return files.filter((f) => f.endsWith('.json')).map((f) => f.replace(/\.json$/, ''));
    } catch {
      return [];
    }
  }

  private categoriesOverlap(a: string, b: string): boolean {
    const wordsA = new Set(a.split('-'));
    const wordsB = new Set(b.split('-'));
    let overlap = 0;
    for (const w of wordsA) {
      if (wordsB.has(w)) overlap++;
    }
    // Require at least 50% keyword overlap relative to smaller set
    const minSize = Math.min(wordsA.size, wordsB.size);
    return minSize > 0 && overlap / minSize >= 0.5;
  }
}
