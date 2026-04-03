export interface DuplicateGroup {
  name: string;
  keep: { id: string; revision: number };
  remove: Array<{ id: string; revision: number }>;
}

export interface CleanupDeleteResult {
  id: string;
  revision: number;
  name: string;
  status: 'ok' | 'failed';
  error?: string;
}

export function findDuplicateProfiles(
  profiles: Array<{ profileId: string; profileName: string; revision?: number }>,
): DuplicateGroup[] {
  const groups = new Map<string, Array<{ id: string; revision: number }>>();

  for (const p of profiles) {
    const rev = p.revision ?? 0;
    const entry = { id: p.profileId, revision: rev };
    const existing = groups.get(p.profileName);
    if (existing) {
      existing.push(entry);
    } else {
      groups.set(p.profileName, [entry]);
    }
  }

  const duplicates: DuplicateGroup[] = [];
  for (const [name, entries] of groups) {
    if (entries.length <= 1) continue;
    entries.sort((a, b) => b.revision - a.revision);
    duplicates.push({
      name,
      keep: entries[0],
      remove: entries.slice(1),
    });
  }

  return duplicates;
}
