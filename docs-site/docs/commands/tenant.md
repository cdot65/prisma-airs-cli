---
title: tenant
sidebar_position: 1
---

# `airs tenant`

Tenants are the only configuration source. `tenant` commands read and change the `airs`
store; they build no API client and send no request.

| Command | What it does |
|---|---|
| `tenant list` (`ls`) | every registered tenant, the selected one marked; no credentials read |
| `tenant path [name]` | the tenant file's path (default: the selected tenant) |
| `tenant read [name]` | the effective settings as `Key │ Value`, secrets `[REDACTED]` |
| `tenant get <name> <key>` | one effective setting, secrets redacted |
| `tenant create <name>` | a new tenant from prompts, or `--tsg-id`, `--client-id` and `--client-secret-stdin`; does not select it |
| `tenant set <name> <key> [value]` | one setting; omit the value for a prompt (hidden for secrets) or pipe it with `--stdin` |
| `tenant unset <name> <key>` | remove one setting so the default applies; credentials cannot be cleared |
| `tenant switch <name>` | select the tenant for later commands |
| `tenant delete <name>` (`rm`) | delete the tenant and its file; asks first, `--force` skips the question |

```sh
airs tenant create development
airs tenant switch development
airs tenant set development defaultOutput json
printf '%s\n' "$KEY" | airs tenant set development airsApiKey --stdin
airs tenant read
```

- Outcome lines (`✓ …`, `ℹ …`) go to stderr and `--quiet` hides them; stdout stays empty
  for the write commands.
- Input errors (a bad name, key or value, a secret on the command line, no terminal for a
  prompt, `delete` without `--force` when not interactive) exit 2.
- Tenant names are 1–64 letters, digits, hyphens or underscores; names that differ only in
  case are refused.

See [Configuration and tenants](../getting-started/configuration.md) for the store, the
settings and the credential planes, and the [reference](../reference/commands/tenant.md) for every flag.

## Named application scanner credentials (8.1 development)

Bind scanner access to the exact dashboard application ID so report enrichment
uses the intended credential. Management discovery still lists applications with
missing bindings and marks their scanner detail incomplete. Credentials and
endpoints remain in the selected tenant TOML file.

```sh
# Explicitly copy an existing flat key. With both flat kinds, --use is required.
airs tenant scanners bind-default development monitor --app-id APP_ID --use key
airs tenant scanners list development

# Add or rotate from a private 0600 TOML file; never pass secrets in argv.
airs tenant scanners set development monitor --file ./scanner.toml --replace
airs runtime scan --profile monitor --scanner monitor "Synthetic example"
airs runtime scan-logs results --scanner monitor --scan-id SCAN_UUID
```

The input file contains `applicationId`, `endpoint`, exactly one of `apiKey` or
`apiToken`, and optional `numRetries` (default 0). One application ID can have only
one named set. Replacing a set generates a new public revision. Flat credential
changes do not rotate an existing bound copy. Direct scan/results/reports use flat
credentials unless `--scanner` is explicit; named-only tenants require selection.
Reports and discovery use exact application bindings and never try alternate keys.

**Configuration format upgrade:** the first named write saves a private, exact-byte
rollback snapshot before adding the namespace. Older binaries refuse the new
format. List and restore snapshots locally:

```sh
airs tenant scanners snapshots development
airs tenant scanners restore development --file SNAPSHOT_PATH --expect-tsg TSG_ID --force
airs tenant scanners delete-snapshot development --file SNAPSHOT_PATH --force
airs tenant scanners remove development monitor --force
```

Restore first saves the current configuration as another private snapshot. These
files contain credentials: retain them privately and delete them explicitly when
no longer needed. Removing the final set preserves the new format marker; restoring
a flat-format snapshot is the explicit downgrade path. Reads never rewrite files.

Named bulk jobs save the scanner name, application, endpoint and revision, without
secrets. Resume refuses a removed or retargeted binding. A rotation at the same
application and endpoint uses that one current credential with a warning that
historical access is unqualified; accepted scans are never resubmitted.
