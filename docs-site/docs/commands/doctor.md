---
title: doctor
sidebar_position: 2
---

# `airs doctor`

`doctor` checks the executable on `PATH`, the selected tenant, its config file, the
environment, both credential sets and the connectivity of three APIs, in nine checks. It exits 1 when any check fails;
warnings and skips do not change the exit code.

```sh
airs doctor
airs doctor --output json | jq '.[] | select(.status != "pass")'
airs doctor --output table                         # Name │ Status │ Detail │ Hint
airs --tenant staging doctor
```

| Check | What it verifies |
|---|---|
| Executable | warns when another `airs`, `airs-cli` or `airs-cli-rs` earlier on `PATH` resolves to a different file than the running program; never fails; runs first, before the tenant is read |
| Tenant | a tenant is selected (or named by `--tenant`) in the store |
| Config file | the tenant file is private, valid TOML, holds only supported keys with valid values, and its `mgmtTsgId` matches the pinned TSG |
| Environment | lists ignored configuration variables (`PANW_*` and the retired names) and the proxy variables in effect; never fails |
| Scanner credentials | `airsApiKey` or `airsApiToken` is set (skip when not: scanning is unavailable) |
| Management credentials | `mgmtClientId`, `mgmtClientSecret` and `mgmtTsgId` are set |
| Scanner API | one probe of the scan API with the API key, within 5 s |
| Management OAuth | an OAuth token, then the custom topics list, within 5 s |
| Gateway API | the AI Gateway data-plane workspaces with the same token, within 5 s; a 403 or 429 warns |

Each status is `pass`, `warn`, `fail` or `skip`. When a check fails, the checks that depend
on it are skipped as `not evaluated — fix the failed checks above first`. Each probe makes
exactly one attempt (no retries), and a server error fails while a rate limit warns.

`--output json` prints an array of `{name, status, detail, hint}` objects (`hint` only when
there is one); `--output table` prints the four columns with no summary line. The details
carry key names, paths and the registered TSG, never a setting's value or server text that
could hold a secret.

The `Executable` check helps when two install channels put different copies on `PATH`
(see [Choosing a channel](../getting-started/install.md#choosing-a-channel)).
