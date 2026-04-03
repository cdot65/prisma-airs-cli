import { describe, expect, it } from 'vitest';
import { findDuplicateProfiles } from '../../../src/cli/commands/profiles-cleanup.js';

describe('findDuplicateProfiles', () => {
  it('returns empty array when no profiles', () => {
    expect(findDuplicateProfiles([])).toEqual([]);
  });

  it('returns empty array when all names are unique', () => {
    const profiles = [
      { profileId: 'a', profileName: 'Alpha', revision: 1 },
      { profileId: 'b', profileName: 'Beta', revision: 1 },
    ];
    expect(findDuplicateProfiles(profiles)).toEqual([]);
  });

  it('keeps highest revision and removes the rest', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 3 },
      { profileId: 'a2', profileName: 'Alpha', revision: 1 },
      { profileId: 'a3', profileName: 'Alpha', revision: 2 },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('Alpha');
    expect(result[0].keep).toEqual({ id: 'a1', revision: 3 });
    expect(result[0].remove).toEqual([
      { id: 'a3', revision: 2 },
      { id: 'a2', revision: 1 },
    ]);
  });

  it('handles mix of unique and duplicate profiles', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 2 },
      { profileId: 'a2', profileName: 'Alpha', revision: 1 },
      { profileId: 'b1', profileName: 'Beta', revision: 1 },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('Alpha');
  });

  it('treats undefined revision as 0', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 1 },
      { profileId: 'a2', profileName: 'Alpha' },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result[0].keep).toEqual({ id: 'a1', revision: 1 });
    expect(result[0].remove).toEqual([{ id: 'a2', revision: 0 }]);
  });

  it('sorts remove list by revision descending', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'A', revision: 5 },
      { profileId: 'a2', profileName: 'A', revision: 1 },
      { profileId: 'a3', profileName: 'A', revision: 3 },
      { profileId: 'a4', profileName: 'A', revision: 4 },
      { profileId: 'a5', profileName: 'A', revision: 2 },
    ];
    const result = findDuplicateProfiles(profiles);
    const revisions = result[0].remove.map((r) => r.revision);
    expect(revisions).toEqual([4, 3, 2, 1]);
  });

  it('handles multiple duplicate groups', () => {
    const profiles = [
      { profileId: 'a1', profileName: 'Alpha', revision: 2 },
      { profileId: 'a2', profileName: 'Alpha', revision: 1 },
      { profileId: 'b1', profileName: 'Beta', revision: 3 },
      { profileId: 'b2', profileName: 'Beta', revision: 1 },
      { profileId: 'b3', profileName: 'Beta', revision: 2 },
    ];
    const result = findDuplicateProfiles(profiles);
    expect(result).toHaveLength(2);
    const names = result.map((g) => g.name).sort();
    expect(names).toEqual(['Alpha', 'Beta']);
  });
});
