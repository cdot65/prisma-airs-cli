# `airs runtime profiles cleanup` — Design Spec

## Problem

AIRS creates a new profile revision (with a new UUID) on every update. Over time this accumulates dozens or hundreds of old revisions visible in `profiles list`. There is no built-in way to prune them — users must manually identify and delete each old revision by UUID.

## Solution

Add a `cleanup` subcommand under `airs runtime profiles` that automatically identifies duplicate profile revisions (same name, different UUIDs), keeps only the highest revision of each, and deletes the rest.

## Command Signature

```
airs runtime profiles cleanup [--force] [--updated-by <email>] [--output json|pretty]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--force` | false | Skip confirmation prompt |
| `--updated-by <email>` | `git config user.email` | Email passed to `forceDeleteProfile()` |
| `--output <format>` | `pretty` | Output format: `pretty` or `json` |

## Behavior

### 1. Fetch and Group

Call `listProfiles()` to get all profiles. Group by `profileName`. A group is a "duplicate set" if it contains more than one entry. Within each group, sort by `revision` descending — the highest revision is kept, all others are marked for deletion.

### 2. No Duplicates Path

If no duplicate sets exist, print "No duplicate profiles found." and exit 0.

### 3. Preview (pretty)

Display a table showing each duplicate group:

```
Profile                                Revisions  Keeping   Deleting
InfoSec - Topic Guardrails - Moderate  58         rev 58    57
InfoSec - Code Assistant - Strict      12         rev 12    11
Truffles - Agent Security - Moderate   2          rev 2     1
InfoSec - AI Firewall - Strict        2          rev 2     1

Total: 70 old revisions to delete across 4 profiles
```

### 4. Confirmation

Prompt: `Delete 70 old revisions? (y/n)`

Skipped when `--force` is passed.

### 5. Resolve `--updated-by`

Resolution order:
1. `--updated-by` flag value if provided
2. Output of `git config user.email` (via `child_process.execSync`)
3. If neither available, print error and exit 1

### 6. Delete

Iterate over all old revisions, calling `forceDeleteProfile(id, updatedBy)` for each. Print progress per deletion in pretty mode.

### 7. Summary

```
Cleanup complete: 70 deleted, 0 failed
```

If any deletions fail, print the failures with profile name, revision, and error message. Exit 1 if any failures occurred.

### 8. JSON Output

When `--output json`, print structured result instead of human-readable output:

```json
{
  "duplicates": [
    {
      "name": "InfoSec - Topic Guardrails - Moderate",
      "kept": { "id": "...", "revision": 58 },
      "deleted": [
        { "id": "...", "revision": 57, "status": "ok" },
        { "id": "...", "revision": 56, "status": "ok" }
      ]
    }
  ],
  "summary": { "deleted": 70, "failed": 0 }
}
```

## File Changes

### `src/cli/commands/runtime.ts`

Register new `cleanup` subcommand under `profiles` (after the existing `delete` command). Implementation lives inline in the action handler, following the same pattern as `list` and `delete`.

Core logic (grouping/dedup) extracted into a pure helper function for testability:

```typescript
interface DuplicateGroup {
  name: string;
  keep: { id: string; revision: number };
  remove: Array<{ id: string; revision: number }>;
}

function findDuplicateProfiles(
  profiles: Array<{ profileId: string; profileName: string; revision?: number }>
): DuplicateGroup[]
```

### `src/cli/renderer/runtime.ts`

Two new functions:

- `renderCleanupPreview(groups: DuplicateGroup[], format: OutputFormat)` — renders the preview table or JSON
- `renderCleanupResult(results: CleanupResult[], format: OutputFormat)` — renders the summary or JSON

### No changes to `src/airs/management.ts` or `src/airs/types.ts`

Existing `listProfiles()` and `forceDeleteProfile()` are sufficient.

## Testing

### Unit Tests

Test the `findDuplicateProfiles()` pure function:

- Empty input → empty output
- All unique names → empty output
- Multiple revisions of same name → keeps highest, removes rest
- Mixed unique and duplicate profiles → only duplicates in output
- Profiles with undefined revision → treated as revision 0

Test file: `tests/unit/cli/profiles-cleanup.spec.ts`

### No E2E Tests

This command mutates real profiles — not suitable for automated E2E.

## Unresolved Questions

None.
