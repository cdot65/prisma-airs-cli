---
title: Backup & restore
sidebar_label: Backup & restore
---

# Back up and restore DLP config

Copy custom DLP dictionaries, data patterns, and data profiles from one tenant to
another. `airs runtime dlp backup` writes your custom config to a file, and
`airs runtime dlp restore` recreates it in whichever tenant you have selected.

Predefined (PANW-shipped) resources are never copied — every tenant already has them, so
restore just re-links your profiles to the destination's own copies.

## Back up

Select the source tenant and write its custom config to a file:

```bash
airs tenant switch prod
airs runtime dlp backup --output-file ./dlp-backup.json
```

That captures every custom dictionary (with its keywords), pattern, and profile, and
prints a summary of the counts. The file is written privately (mode `0600`) and contains
keyword lists, so store it somewhere safe.

Back up a single kind with `--resources` (comma-separated: `dictionaries`, `patterns`,
`profiles`):

```bash
airs runtime dlp backup --resources patterns --output-file ./patterns.json
```

## Restore

Switch to the destination tenant and preview first — a dry run reads the destination but
writes nothing:

```bash
airs tenant switch dev
airs runtime dlp restore ./dlp-backup.json --dry-run
```

The plan shows what will happen to each resource:

| Action    | Meaning                                                            |
| --------- | ----------------------------------------------------------------- |
| `create`  | New resource — will be created.                                   |
| `reuse`   | An identical resource already exists — left untouched.            |
| `resolve` | A predefined resource, matched in the destination catalog.        |
| `map`     | Bound to a destination pattern you named with `--pattern-map`.    |
| `verify`  | An existing profile, checked against the backup — no write.       |
| `skip`    | Excluded from the restore (see [When you need to decide](#when-you-need-to-decide)). |

When the plan looks right, run it for real:

```bash
airs runtime dlp restore ./dlp-backup.json
```

You confirm the source and destination TSGs, then the restore writes in order —
dictionaries, then patterns, then profiles — re-linking each profile's references to the
destination's own ids as it goes, and reading back every resource it creates to confirm
it landed. To run unattended, assert the destination and skip the prompt:

```bash
airs runtime dlp restore ./dlp-backup.json --expect-tsg <destination-tsg> --force
```

## Re-running is safe

Restore again whenever you like. Unchanged resources are reused, existing profiles are
verified against the backup, and only what is missing gets created. If a run stops
partway — an API error, or another operator changing the tenant mid-restore — nothing is
rolled back: it reports exactly what completed and exits non-zero. Fix the cause and
re-run; it picks up where it left off.

## End-to-end example: prod → dev

Migrate a small config — one keyword dictionary, one custom regex pattern, and a profile
that references both a custom pattern and the predefined `Social Security Numbers`
pattern.

```bash
# 1. Register both tenants once.
airs tenant create prod --config /secure/prod.json
airs tenant create dev  --config /secure/dev.json

# 2. Back up the source.
airs tenant switch prod
airs runtime dlp backup --output-file ./dlp-backup.json

# 3. Preview against the destination.
airs tenant switch dev
airs runtime dlp restore ./dlp-backup.json --dry-run --output json

# 4. Restore.
airs runtime dlp restore ./dlp-backup.json --expect-tsg 2020202020 --force
```

The dry run in step 3 prints the plan as JSON:

```json
[
  {
    "sourceTsgId": "1010101010",
    "destinationTsgId": "2020202020",
    "dryRun": true,
    "dictionaries": [{ "name": "Compliance Keywords", "action": "create" }],
    "patterns": [
      { "name": "Customer Account Regex", "action": "create" },
      { "name": "Social Security Numbers", "action": "resolve" }
    ],
    "profiles": [{ "name": "PII Guardrail", "action": "create" }]
  }
]
```

The custom dictionary and pattern are created, the predefined pattern resolves to dev's
own copy, and the profile is created with its references pointed at the new dev ids.

## Command reference

### `airs runtime dlp backup`

| Flag                     | Purpose                                                        |
| ------------------------ | ------------------------------------------------------------- |
| `--output-file <path>`   | Where to write the backup (required).                         |
| `--resources <list>`     | Limit to `dictionaries`, `patterns`, and/or `profiles`.       |
| `--file-format <fmt>`    | `json` (default) or `yaml`.                                    |
| `--skip-unsupported`     | Exclude profiles that can't be backed up, instead of failing. |

### `airs runtime dlp restore`

| Flag                          | Purpose                                                       |
| ----------------------------- | ------------------------------------------------------------ |
| `--dry-run`                   | Print the plan; write nothing.                               |
| `--expect-tsg <id>`           | Assert the destination TSG (required with `--force`).        |
| `--force`                     | Skip the confirmation prompt.                                |
| `--name-prefix <prefix>`      | Restore under prefixed names, alongside existing resources.  |
| `--pattern-map "src=dest"`    | Bind a source pattern to a named destination pattern (repeatable). |
| `--on-conflict <policy>`      | How to handle an existing profile name: `error` (default), `verify`, `skip`, `reconcile`. |
| `--skip-unresolved`           | Skip references with no destination match, and the profiles that use them. |

## When you need to decide

Most restores are clean. These are the cases that stop and ask you to make a call —
each one prints the exact command to run next.

### A profile can't be backed up

Some profiles can't be recreated faithfully, so the backup refuses them and lists every
one at once:

```
✗ Profiles cannot be exported: <name> (Unsupported detection rule type: multi_profile);
  exclude unsupported profiles explicitly with --skip-unsupported
```

This covers **multi-profile** rules (they reference other profiles by per-tenant numeric
id, which can't be transplanted), **direct EDM dataset** references, and profiles that
depend on a **retired** pattern. Re-run with `--skip-unsupported` to exclude them and
back up the rest; each exclusion is reported.

### A pattern has no match in the destination

Predefined catalogs differ between tenants. Restore first matches a predefined reference
by identity, then by name; if neither works it lists every miss with suggested
equivalents:

```
✗ Missing predefined destination data patterns: Internet - ipv4
  (candidates: "Internet - IPv4"); bind each with --pattern-map "<source>=<destination>"
```

List the destination catalog to find the right one (predefined records are hidden by
default):

```bash
airs runtime dlp patterns list --all --include-predefined --output json
```

Then bind it: `--pattern-map "Internet - ipv4=Internet - IPv4"`. The CLI never guesses a
near-match on your behalf, because binding the wrong detector would silently change what
a profile catches.

The same `--pattern-map` binds **tenant-bound patterns** (EDM, fingerprints, trained
classifiers) that a backup can't recreate — provision the equivalent in the destination
first, then map to it.

If you'd rather drop the unmatched references than bind them, `--skip-unresolved` excludes
each one **and every profile that depends on it** (profiles are skipped whole — a
detection rule is never partially removed), reporting each so nothing is lost silently.

### A profile name already exists

`--on-conflict` decides what happens when the destination already has a profile with the
same name:

| Policy       | Behavior                                                                  |
| ------------ | ------------------------------------------------------------------------- |
| `error`      | Stop (the default).                                                       |
| `verify`     | Check the existing profile matches the backup; write nothing.             |
| `skip`       | Leave the existing profile alone and move on.                             |
| `reconcile`  | Verify an active duplicate; for an archived-name collision, create a suffixed copy. |

`reconcile` exists because of how the API handles profile names (see below). Use it when
a destination has *archived* profiles blocking the names you're restoring.

## Known limits

A few hard edges of the DLP API worth knowing before a large migration:

- **Profiles can't be deleted, and their names never free up.** The API has no profile
  delete, and a profile deleted in the SCM UI still reserves its name — a later create
  under that name returns `409 Conflict`. `--on-conflict reconcile` works around this by
  creating the profile under a unique suffixed name (`<name>-<6 hex>`).
- **`reconcile` is not idempotent for archived-name collisions.** Because each suffixed
  copy has a new name, re-running `reconcile` creates *another* copy rather than reusing
  the first. Run it once, or clear the archived profiles in the SCM UI before restoring.
- **Profile names are limited to 32 characters.** `--name-prefix` and reconcile suffixes
  are truncated to fit.
- **Dictionary regions use the display name.** Pass the SCM region label — for example
  `--region "United States"` — not a code like `GLOBAL` or `us-west-2`, which the API
  rejects with a bare `400`.
- **Predefined resources are read-only.** They're referenced, never created or modified,
  and hidden from `list` output unless you pass `--include-predefined`.
