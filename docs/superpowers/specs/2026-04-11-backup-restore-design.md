# Backup & Restore — Design Spec

## Overview

Add `airs backup` and `airs restore` top-level command groups for exporting and importing AIRS configuration to/from local JSON/YAML files. Starting with redteam targets; extensible to profiles, topics, prompt-sets.

## CLI Commands

### `airs backup targets`

```
airs backup targets [--output-dir <path>] [--format json|yaml] [--name <targetName>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--output-dir` | `./airs-backup/targets/` | Directory to write backup files |
| `--format` | `json` | Output format: `json` or `yaml` |
| `--name` | _(none)_ | Single target by name; omit for bulk (all targets) |

- **Bulk mode** (no `--name`): fetches all targets, writes one file per target.
- **Single mode** (`--name`): fetches that target, writes one file.
- Filenames derived from target name, sanitized to filesystem-safe chars (e.g. `my-openai-target.json`).

### `airs restore targets`

```
airs restore targets [--input-dir <path>] [--file <path>] [--format json|yaml] [--overwrite] [--validate]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--input-dir` | _(none)_ | Directory to read backup files from (bulk mode) |
| `--file` | _(none)_ | Single backup file path |
| `--format` | _(auto-detect)_ | Override format detection from file extension |
| `--overwrite` | `false` | Update existing targets with same name; default skips |
| `--validate` | `false` | Test connection before saving (pass-through to SDK) |

- Must specify one of `--input-dir` or `--file`.
- Format auto-detected from file extension; `--format` overrides.
- Name collision without `--overwrite` → skip with warning.
- Individual target failures don't abort batch — errors collected and reported at end.

## Backup File Schema

```json
{
  "version": "1",
  "resourceType": "redteam-target",
  "exportedAt": "2026-04-11T12:00:00.000Z",
  "data": {
    "name": "my-openai-target",
    "target_type": "OPENAI",
    "connection_params": { "..." : "..." },
    "background": { "..." : "..." },
    "additional_context": { "..." : "..." },
    "metadata": { "..." : "..." }
  }
}
```

- **`version`**: envelope version for forward compatibility; restore validates before proceeding.
- **`resourceType`**: discriminator for multi-resource backup directories.
- **`data`**: fields needed for `createTarget` / `updateTarget` only — server-assigned fields (`uuid`, `status`, `active`) stripped.
- **`connection_params`**: included as-is (full fidelity; user responsible for securing files).

Same envelope shape reused for future resource types.

## Architecture

### Approach: Shared utilities + thin CLI commands

Small `src/backup/` module with shared I/O helpers. Orchestration logic stays in CLI command handlers. When a second resource type is added, common patterns can be extracted into an interface.

### New Files

```
src/
├── backup/
│   ├── types.ts       # BackupEnvelope<T>, BackupFormat, ResourceType
│   ├── io.ts          # writeBackupFile, readBackupFile, readBackupDir, sanitizeFilename, resolveOutputDir
│   └── index.ts       # Barrel exports
├── cli/
│   ├── commands/
│   │   ├── backup.ts  # airs backup targets command handler
│   │   └── restore.ts # airs restore targets command handler
│   └── renderer/
│       └── backup.ts  # renderBackupSummary, renderRestoreSummary
tests/
├── unit/
│   ├── backup/
│   │   └── io.spec.ts     # I/O utility tests
│   └── cli/
│       └── backup.spec.ts # Command handler tests
```

### `src/backup/types.ts`

- `BackupEnvelope<T>` — `{ version: string; resourceType: ResourceType; exportedAt: string; data: T }`
- `BackupFormat` — `'json' | 'yaml'`
- `ResourceType` — `'redteam-target'` (union, extended later)

### `src/backup/io.ts`

- `writeBackupFile(dir, filename, envelope, format)` — serialize to JSON/YAML, write to `dir/filename.ext`, create dir if needed.
- `readBackupFile<T>(filePath)` — read file, detect format from extension, parse, validate envelope, return typed `BackupEnvelope<T>`.
- `readBackupDir<T>(dirPath, resourceType)` — glob `*.json` + `*.yaml`/`*.yml`, call `readBackupFile`, filter by `resourceType`.
- `sanitizeFilename(name)` — replace non-filesystem-safe chars with hyphens, lowercase.
- `resolveOutputDir(userDir, defaultSubdir)` — resolve path, default to `./airs-backup/<defaultSubdir>/`.

### Command Handlers

**Backup flow (`src/cli/commands/backup.ts`):**

1. Create service via existing pattern (`loadConfig` → `SdkRedTeamService`).
2. `--name` → find target by name from `listTargets()`, then `getTarget(uuid)`. Not found → error.
3. No `--name` → `listTargets()`, then `getTarget(uuid)` for each.
4. Per target: strip `uuid`/`status`/`active`, normalize `targetType` → `target_type`, wrap in envelope, `writeBackupFile()`.
5. Render summary.

**Restore flow (`src/cli/commands/restore.ts`):**

1. Create service.
2. Read file(s) via `readBackupFile` or `readBackupDir`.
3. Validate each envelope: `version === "1"`, `resourceType === "redteam-target"`.
4. `listTargets()` to check name collisions.
5. Per backup:
   - Exists + no `--overwrite` → skip, warn.
   - Exists + `--overwrite` → `updateTarget(existingUuid, data, { validate })`.
   - New → `createTarget(data, { validate })`.
6. Render summary: created/updated/skipped/failed counts.

### Renderer (`src/cli/renderer/backup.ts`)

- `renderBackupSummary(results, outputDir)` — target name + filename per entry, total count, output path.
- `renderRestoreSummary(results)` — target name + action (created/updated/skipped/failed) per entry, totals per category.

### Registration (`src/cli/index.ts`)

```typescript
program.addCommand(createBackupCommand());
program.addCommand(createRestoreCommand());
```

## Testing

### `tests/unit/backup/io.spec.ts`

- `writeBackupFile`: writes JSON/YAML, creates directories, sanitizes filenames.
- `readBackupFile`: parses JSON/YAML, validates envelope, throws on invalid version/missing fields.
- `readBackupDir`: finds matching files, filters by resourceType, skips non-matching.
- `sanitizeFilename`: special chars, spaces, unicode, empty string.

### `tests/unit/cli/backup.spec.ts`

- Backup single target: mock service, assert envelope shape.
- Backup all targets: mock service, assert one file per target.
- Restore single (new): assert `createTarget` called with correct data.
- Restore name collision (skip): assert skip + warning.
- Restore `--overwrite`: assert `updateTarget` called.
- Restore invalid envelope: assert error reported, batch continues.
- Restore service error: assert failure collected, other targets processed.

## Unresolved Questions

_(none — all decisions captured above)_
