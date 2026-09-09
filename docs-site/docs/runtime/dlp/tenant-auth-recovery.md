---
title: Resume after a DLP tenant authentication failure
---

# Resume the prod-to-dev migration

CLI **5.7.1** fixes DLP commands bypassing selected-tenant JSON credentials and
ignoring structured output on writes. SDK 0.30.0 remains unchanged; this was CLI wiring,
not an API schema change.

## Keep the existing evidence

If `create_test_dlp prod` failed with `AISEC_MISSING_VARIABLE:clientId is required`,
that invocation failed during local client construction, before OAuth or resource creation.
It does not undo earlier steps. The draft's `set -e` exits Bash on failure, returning
to the parent shell (often zsh). It does not remove tenant registrations or captured files.
Shell functions and exported variables from that Bash session do need to be reloaded.

1. Install the fix, then start Bash again:

   ```bash
   npm install --global @cdot65/prisma-airs-cli@5.7.1
   airs --version
   bash
   ```

2. Change to the **existing** evidence directory printed by the original run. Do not
   create a new workspace or repeat tenant onboarding. Inspect `tenants.json` and the
   `prod-pattern-create.json.stderr` / `.exit-code.txt` files. Preserve the failed
   attempt before retrying: the capture helper intentionally refuses overwrites.

   Run this from that evidence directory. The guard refuses recovery if later DLP files
   already exist; inspect those resources instead of repeating creates.

   ```bash
   bash <<'BASH'
   set -euo pipefail
   umask 077
   test -f tenants.json
   test -f prod-pattern-create.json.exit-code.txt
   test ! -s prod-pattern-create.json
   test ! -e prod-pattern.json
   test ! -e prod-dlp-test-request.json
   test ! -e prod-dlp-create.json
   test ! -e prod-dlp-test.json
   node -e '
     const fs = require("node:fs");
     const error = fs.readFileSync("prod-pattern-create.json.stderr", "utf8");
     const code = Number(fs.readFileSync("prod-pattern-create.json.exit-code.txt", "utf8"));
     if (!code || !error.includes("AISEC_MISSING_VARIABLE:clientId is required"))
       throw Error("Not the pre-request credential-loading failure; inspect before retrying");
   '
   failed_attempt=$(mktemp -d "$PWD/failed-dlp-auth-XXXXXX")
   mv -- prod-pattern-create.json.command.txt prod-pattern-create.json \
     prod-pattern-create.json.stderr prod-pattern-create.json.exit-code.txt "$failed_attempt/"
   printf 'Original failure retained in %s\n' "$failed_attempt"
   BASH
   ```

3. In the interactive Bash session, restore environment isolation and the TSG variables
   from the existing receipt:

   ```bash
   for key in ${!PANW_@}; do unset "$key"; done
   unset PRISMA_AIRS_CONFIG_PATH
   export DOTENV_CONFIG_PATH=/dev/null
   umask 077
   set -o noclobber
   export DEV_TSG=$(node -pe 'JSON.parse(require("node:fs").readFileSync("tenants.json","utf8")).find(t=>t.name==="dev").tsgId')
   export PROD_TSG=$(node -pe 'JSON.parse(require("node:fs").readFileSync("tenants.json","utf8")).find(t=>t.name==="prod").tsgId')
   airs tenant switch prod
   airs tenant list
   airs runtime dlp patterns list --all --output json
   airs runtime dlp profiles list --all --output json
   ```

   Confirm prod is selected with the expected TSG and inspect both complete inventories
   for `dlp-test-pattern` / `dlp-test`. If either already exists, stop and inspect its ID
   and content before deciding to reuse it. Do not blindly rerun the create helper.

4. Re-paste **only the definitions** of `capture`, `json_id`, `assert_empty`, and
   `create_test_dlp` from the draft. Restore `set -euo pipefail` before resuming its
   fail-fast sequence. Run `create_test_dlp prod`, then continue with the two topic
   guardrails. No tenant recreation or repeat backup is needed for this specific failure.

Do not wrap the whole create helper in `if` or `|| true` to suppress shell exit: Bash
can disable its internal fail-fast behavior in those contexts and continue after a failed write.
The preserved failed attempt plus successful retry belong in the final acceptance evidence.

## Verification evidence — 2026-09-09

- Reproduced the missing-client-ID failure in all four DLP groups using installed 5.7.0.
- Exercised the real CLI and SDK against a local OAuth/HTTP server: named prod/dev configs,
  tenant switching, pattern and advanced-profile creation/read-back, all four DLP list
  commands, explicit config paths, credential-file immutability and mixed-env rejection.
- Verified JSON create acknowledgements parse directly, followed by full GET rule checks.
- Live read-only checks with the available aisecurity config succeeded for pattern and
  profile inventories (25 records each on the default page; not complete inventory totals).
- The available cdot65 config reached OAuth but received HTTP 401. This is distinct from
  the fixed local missing-credentials error. No configuration or cloud resource was changed.
- The new dev/prod registrations were not present in the agent's available registry;
  their live create/migrate acceptance remains to be captured by the operator.

Local-server contract tests are not a claim that the full cloud migration passed.

Release gates: 1,687 tests across 117 files passed; 97.37% statement/line coverage,
90.33% branch coverage and 96.52% function coverage (configured coverage excludes the CLI
layer, which is exercised by the subprocess tests). Fourteen release-policy tests,
TypeScript, lint/format and the Docusaurus build passed; the production dependency audit
reported no known vulnerabilities. Seven preexisting lint warnings remain outside this fix.
