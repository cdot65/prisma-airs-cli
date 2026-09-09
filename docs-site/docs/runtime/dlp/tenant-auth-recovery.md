---
title: Resume after a DLP tenant authentication failure
---

# Resume the prod-to-dev migration

CLI **5.7.1** fixes DLP commands bypassing selected-tenant JSON credentials and
ignoring structured output on writes. SDK 0.30.0 remains unchanged; this was CLI wiring,
not an API schema change.

## Resume without starting over

The `AISEC_MISSING_VARIABLE:clientId is required` failure in CLI 5.7.0 occurred during
local client construction, before OAuth or resource creation. It does not undo earlier
successful steps. A terminal closing does not remove tenant registrations or backup files.

Use CLI 5.7.1 or newer, return to the existing backup directory, and inspect your tenant
registrations. The [direct CLI migration guide](../prod-dev-migration.md) requires no
shell functions or exported TSG variables.

```bash
airs --version
airs tenant list
airs tenant read prod
airs tenant read dev
airs tenant switch prod
airs tenant list
airs runtime dlp patterns list --all --max 0 --output json
airs runtime dlp profiles list --all --max 0 --output json
```

Confirm prod's selected TSG. Avoid conflicting `PANW_*`, explicit config-path, or
`.env` credential overrides; do not export secrets as a workaround. If credentials need
correction, update one setting at a time using hidden secret entry:

```bash
airs tenant set prod mgmtClientId
airs tenant set prod mgmtClientSecret
```

Only run those setters if the registered credentials are wrong. Do not change a correctly
configured registration to fix an outdated CLI.

If `dlp-test-pattern` or `dlp-test` already exists, inspect it by its returned ID:

```bash
airs runtime dlp patterns get "<PROD_PATTERN_ID>" --output json
airs runtime dlp profiles get "<PROD_DLP_PROFILE_ID>" --output json
```

Replace the placeholders with the IDs from the lists. Continue from the first missing
resource in the [migration guide](../prod-dev-migration.md), not from tenant onboarding.
An HTTP error or interrupted request is different from the pre-request missing-client-ID
failure: inspect state before retrying any create.

If the Runtime restore was interrupted later, retain the source backup, select dev,
and inspect a recovery plan before making more changes:

```bash
airs tenant switch dev
airs tenant list
airs runtime profiles restore ./prod-runtime-backup.json \
  --dlp-map 'dlp-test=dlp-test' --on-missing-dlp error --on-conflict verify \
  --expect-tsg "<DEV_TSG>" --dry-run --output json
```

Use the actual dev TSG ID from the tenant list. Keep failed output and successful backup
files private; do not overwrite them. Remove `--dry-run` only after reviewing the plan.

## Verification evidence — 2026-09-09

- Reproduced the missing-client-ID failure in all four DLP groups using installed 5.7.0.
- Exercised the real CLI and SDK against a local OAuth/HTTP server: named prod/dev configs,
  tenant switching, pattern and advanced-profile creation/read-back, all four DLP list
  commands, explicit config paths, credential-file immutability and mixed-env rejection.
- Verified JSON create acknowledgements parse directly, followed by full GET rule checks.
- Live read-only checks using the available aisecurity config, registered as a named tenant
  in an isolated test registry, succeeded for all four groups. The normal registry and its
  selected tenant were unchanged. Aggregate receipt (25 is the default page size, not the
  complete inventory total):

  ```json
  {"group":"patterns","exitCode":0,"pageLength":25,"auth":"named tenant JSON via isolated registry"}
  {"group":"profiles","exitCode":0,"pageLength":25,"auth":"named tenant JSON via isolated registry"}
  {"group":"dictionaries","exitCode":0,"pageLength":25,"auth":"named tenant JSON via isolated registry"}
  {"group":"filtering-profiles","exitCode":0,"pageLength":25,"auth":"named tenant JSON via isolated registry"}
  ```

  These are JSONL test-summary records, not the raw CLI inventory response.
- The available cdot65 config reached OAuth but received HTTP 401. This is distinct from
  the fixed local missing-credentials error. No configuration or cloud resource was changed.
- At the time of the authentication fix, dev/prod were absent from the agent's registry.
  The operator subsequently completed the migration; its separately reviewed results are
  in the [prod-to-dev guide](../prod-dev-migration.md#validated-results--2026-09-09).

Local-server contract tests are not a claim that the full cloud migration passed.

Release gates: 1,687 tests across 117 files passed; 97.37% statement/line coverage,
90.33% branch coverage and 96.52% function coverage (configured coverage excludes the CLI
layer, which is exercised by the subprocess tests). Fourteen release-policy tests,
TypeScript, lint/format and the Docusaurus build passed; the production dependency audit
reported no known vulnerabilities. Seven preexisting lint warnings remain outside this fix.
