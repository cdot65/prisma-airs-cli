---
title: Configuration and tenants
sidebar_position: 3
---

# Configuration and tenants

`airs` has one configuration source: **tenants** in its own store. Every command that talks
to Prisma AIRS uses the selected tenant (or the one named by `--tenant <name>` for a single
invocation). Credentials and endpoints never come from environment variables.

## Where tenants live

The store is the first of:

1. `$AIRS_CLI_CONFIG_DIR`
2. `$XDG_CONFIG_HOME/airs-cli`
3. `~/.config/airs-cli`

It holds `config.toml` (the selection and each tenant's pinned TSG) and
`tenants/<name>.toml` (that tenant's settings, secrets included). Files are mode 0600 and
directories 0700; a file that is a symlink, owned by someone else or readable by group or
others is refused, and so is a directory that others can write. Only the tenant write
commands change the store. Every change takes one lock
(`config.lock`; a second change waits up to 5 s and then names the process holding it) and
replaces files atomically, so a reader never sees a half-written file.

## Creating and selecting a tenant

```sh
airs tenant create development           # prompts for the TSG ID, client ID and (hidden) client secret
printf '%s\n' "$SECRET" | airs tenant create ci --tsg-id 1234567890 --client-id ci-client --client-secret-stdin
airs tenant switch development           # select it for later commands
airs tenant list                         # every tenant, no credentials read (alias: ls)
airs --tenant ci doctor                  # use another tenant for one invocation
```

Coming from the TypeScript CLI? See [Upgrading from 7.x](./upgrading-from-typescript.md).

## Editing settings

```sh
airs tenant set development defaultOutput json
airs tenant set development airsApiKey                    # secrets: a hidden prompt ...
printf '%s\n' "$KEY" | airs tenant set development airsApiKey --stdin   # ... or stdin
airs tenant set development mgmtTsgId 2345678901          # changes the TSG and its pin
airs tenant unset development defaultOutput               # credentials cannot be cleared
airs tenant read                                          # Key │ Value, secrets [REDACTED]
airs tenant get development mgmtTsgId --output json
airs tenant path                                          # the selected tenant's file
airs tenant delete development                            # asks first; --force skips the question
```

- **Values are checked when you enter them.** Numbers are decimal integers in range,
  endpoints must be https (http only for loopback hosts) without credentials, a query or a
  fragment, `mgmtTsgId` is 1–32 digits, and `defaultOutput` is `pretty`, `table` or
  `json`. An empty value is refused; use `tenant unset` to remove a setting.
- **Secrets never go on the command line.** `tenant set <name> <secret key> <value>` is
  refused; omit the value for a hidden prompt, or pipe it with `--stdin`. Prompts need
  stdin and stderr to be terminals.
- **Wrong-tenant protection.** A tenant file whose `mgmtTsgId` differs from the TSG pinned
  in `config.toml` is refused; `tenant set <name> mgmtTsgId <tsg>` changes both.
- **Deleting** removes the tenant's file and clears the selection if it was selected.
  Without a terminal it needs `--force`.

## Settings

| Key | Purpose |
|---|---|
| `airsApiKey`, `airsApiToken` | scanning credentials (secret) |
| `airsApiEndpoint`, `airsNumRetries` | scan API endpoint and retries (0–5) |
| `mgmtClientId`, `mgmtClientSecret`, `mgmtTsgId` | the management OAuth credentials |
| `mgmtEndpoint`, `mgmtDashboardEndpoint`, `mgmtTokenEndpoint` | management, dashboard and token endpoints |
| `dlpEndpoint` | DLP API |
| `redTeamDataEndpoint`, `redTeamMgmtEndpoint`, `redTeamNetworkBrokerEndpoint` | AI Red Teaming APIs |
| `modelSecDataEndpoint`, `modelSecMgmtEndpoint` | AI Model Security APIs |
| `supplychainDataEndpoint`, `supplychainMgmtEndpoint` | AI Supply Chain APIs |
| `gatewayDataEndpoint`, `gatewayAdminEndpoint`, `iamEndpoint` | AI Gateway data and admin planes, IAM |
| `gatewayInferenceEndpoint`, `gatewayInferenceApiKey`, `gatewayInferenceModel`, `gatewayEmbeddingModel` | `gateway inference` (the API key is secret) |
| `scanConcurrency` | carried over from the TypeScript CLI, 1–20 (default 5); `runtime topics eval` takes `--concurrency` |
| `defaultOutput` | `pretty`, `table` or `json` |

Every endpoint is optional; without it the default Prisma AIRS endpoint is used. An
unknown key makes the tenant file invalid.

## Two credential planes

- **Management** commands (profiles, topics, DLP, red team, model security, gateway,
  supply chain, API keys, reports, backups) authenticate with OAuth client credentials:
  `mgmtClientId`, `mgmtClientSecret` and `mgmtTsgId`.
- **Scanning** (`runtime scan`, `runtime bulk-scan`, `runtime resume-poll`,
  `runtime topics eval`) uses `airsApiKey` or `airsApiToken`.
- **Gateway inference** uses `gatewayInferenceApiKey`.

Neither plane sends the other's credentials, and each command requires only its own.

## Environment variables

Configuration comes only from tenant files. Credential and endpoint variables (`PANW_*`,
`PRISMA_AIRS_CONFIG_PATH` and the other retired TypeScript names) are ignored; `airs doctor`
lists the ones that are still set. The variables that do matter:

| Variable | Effect |
|---|---|
| `AIRS_CLI_CONFIG_DIR`, `XDG_CONFIG_HOME` | where the tenant store lives |
| `XDG_STATE_HOME` | where bulk-scan state and debug logs live (`$XDG_STATE_HOME/airs-cli/`) |
| `XDG_DATA_HOME` | where `model-security install --fetch-uv` keeps uv |
| `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY`, `NO_PROXY` (and lower case) | proxies for https endpoints; macOS system proxies are honoured too |
| `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE` | colour |

## Checking a tenant

```sh
airs doctor
airs doctor --output json | jq '.[] | select(.status != "pass")'
```

See [doctor](../commands/doctor.md).
