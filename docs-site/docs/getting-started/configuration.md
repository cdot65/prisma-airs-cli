---
title: Configuration
---

# Configuration

Prisma AIRS CLI is configured entirely through **tenants**. A tenant is a private JSON file
holding one set of SCM OAuth credentials (client ID, secret, TSG ID) plus any optional
settings, registered under a name. Exactly one tenant is selected at a time, and every command
reads only that file. Environment variables are never consulted.

## Config Cascade

Settings are resolved in priority order (highest wins):

```
CLI flags  >  Selected tenant's config file  >  Zod defaults
```

## First-time setup

```bash
airs-cli tenant create development     # prompts for TSG ID, client ID, and a hidden client secret
airs-cli tenant switch development     # every command now uses this tenant
airs-cli doctor                        # verify the file, credentials, and connectivity
```

For scanning, add the runtime key with a hidden prompt:

```bash
airs-cli tenant set development airsApiKey
```

Already have a JSON file? Register it without copying or editing it:

```bash
airs-cli tenant create production --config /secure/production.json
```

## Managing settings

`airs-cli tenant` covers every read and write:

```bash
airs-cli tenant read                          # all settings of the selected tenant, secrets redacted
airs-cli tenant get development scanConcurrency
airs-cli tenant set development scanConcurrency 3
airs-cli tenant set development defaultOutput json
airs-cli tenant set development mgmtClientSecret --stdin < /secure/rotated-secret.txt
airs-cli tenant set development typesafeApiKey    # hidden prompt; enables redteam judge
airs-cli tenant unset development defaultOutput   # defaults take over
airs-cli tenant path                          # print the selected tenant's file path
```

- **`set`** validates the value through the schema before writing; invalid values are rejected
  and nothing is written. Credentials never go on the command line: omit the value for a hidden
  prompt or pipe it with `--stdin`. The registered `mgmtTsgId` is pinned.
- **`unset`** removes a key so the schema default applies; credential keys cannot be cleared.
- **`get`** and **`read`** redact every key whose name ends in `key`, `secret`, `token`, or
  `password`. There is intentionally no `--reveal`.
- **`path`** prints only the file path, which is pipe-friendly.

Keys and defaults are listed in [configuration options](../reference/configuration.md).
Multi-tenant workflows, automation, and the registry layout are in
[tenant selection](../cli/tenant.md).

## Tuning Parameters

| Config Key | Default | What it does |
|-----------|---------|-------------|
| `scanConcurrency` | `5` | Parallel scan requests per batch (1--20) |
| `dataDir` | `~/.prisma-airs/runs` | Bulk-scan state directory |
| `defaultOutput` | `pretty` | Default read format: `pretty`, `table`, `markdown`, `csv`, `json`, or `yaml` |

:::tip[Concurrency vs. rate limits]
Keep `scanConcurrency` at 5 or lower to avoid AIRS rate limiting. Increase only if your tenant has elevated quotas.
:::

## Data Locations

| Path | Purpose |
|------|---------|
| `~/.local/state/prisma-airs/tenants.json` | Tenant registry (names, file paths, TSG IDs; never credentials) |
| `~/.local/state/prisma-airs/configs/` | Config files created by `airs-cli tenant create` (mode `0600`) |
| `~/.prisma-airs/runs/` | Bulk-scan state (`dataDir`) |

## OAuth fetch failures inside an agent

`AISEC_OAUTH_ERROR: Token request failed: fetch failed` means the token request
failed at the transport step. It does not establish invalid credentials or a
Gateway permission denial. DNS, TLS trust, proxy configuration, endpoint
reachability, or the agent's shell sandbox may be responsible. A successful local
configuration check establishes only that settings are present and valid.

For an already-authorized read, request the agent host's supported **per-command
network approval**, then retry the same command once. If that mechanism is
unavailable or denied, report the execution restriction. Do not disable the
sandbox, replace credentials, or change API planes to make the error disappear.
A command-prefix approval alone may not grant network access.

If the approved request still fails, compare the same read from a normal terminal
on the same host, using the same managed CLI and selected product tenant. Inspect
DNS, proxy and certificate trust for the configured OAuth endpoint. Report only
sanitized error codes and outcomes; never print tokens, secrets or raw tenant
files. Do not disable TLS verification.

An HTTP rejection from the token endpoint requires authentication diagnosis;
a Gateway API 403 after token acquisition requires permission diagnosis. Neither
is equivalent to a token fetch failure. An empty successful list is a valid
result, while a failed request provides no inventory or pagination evidence.

## Product credentials and Harness credentials

`airs cli doctor` runs product CLI diagnostics for the selected **tenant**.
`airs doctor` and in-session `/doctor` inspect the selected **Harness environment**.
These are different credential scopes; environment names need not match tenants
or Gateway workspaces.

The standalone CLI reads scanner and TypeSafe keys from its tenant configuration.
Missing scanner credentials disable scanner-dependent commands, not Gateway
management reads. A CLI report that TypeSafe is not configured does not prove
that the Harness environment has no saved Jev key. The bundled ASR skill resolves
that key through its native environment credential helper and passes it only to
the judge child process. Use `/typesafe` and the installed
`$prisma-airs-asr-judge` skill for this workflow; do not copy the saved key into the
tenant file to satisfy a diagnostic. See [judge authentication](../cli/redteam/judge.md).
