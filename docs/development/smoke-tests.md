# Live Smoke Tests

## Why

The unit suite uses fully mocked services. Zero tests in this repo hit a real Prisma AIRS tenant. The only `tests/e2e/` file targets LangChain's Vertex AI provider for the `audit` command's LLM path — not the AIRS Scanner, Management, RedTeam, or ModelSecurity APIs.

That means every AIRS API endpoint group can drift in production without CI noticing. SDK 0.8.0 enables runtime Zod validation on every response, so this 16-command smoke check is the lightest way to catch wire-format drift before it reaches users.

!!! warning "Run after every release"
    These commands are read-only and side-effect free. Run them after publishing a new CLI release, after upgrading the `@cdot65/prisma-airs-sdk` dependency, or any time the AIRS backend rolls out a change.

!!! tip "Need deeper coverage?"
    For an end-to-end audit covering every command (including write paths and long-running workflows), see the [Full CLI Command Sweep](full-cli-sweep.md).

## Prerequisites

Set the following environment variables (or have them in `~/.prisma-airs/config.json`):

- `PANW_AI_SEC_API_KEY` — Scanner API key
- `PANW_MGMT_CLIENT_ID`, `PANW_MGMT_CLIENT_SECRET`, `PANW_MGMT_TSG_ID` — Management OAuth2

You'll also need the name of an existing security profile in your tenant. Pick one from the output of step 4 below; the docs use `<profile>` as a placeholder.

---

## Step 1 — Install and version verification

Run these in order. They confirm the binary on your `PATH` is the latest CLI and that the bundled SDK is `0.8.0` or newer.

```bash
# 1. Install the latest CLI globally (or use `pnpm add -g`, `npx`, or a Docker image)
npm install -g @cdot65/prisma-airs-cli@latest

# 2. Confirm the CLI is on PATH and report its version
which airs && airs --version

# 3. Confirm the SDK that the CLI bundled is 0.8.0 or newer
#    Works whether the CLI is installed globally or locally; reads the package.json directly
#    because the SDK's `exports` map blocks `require('@cdot65/prisma-airs-sdk/package.json')`.
npm ls -g @cdot65/prisma-airs-sdk 2>/dev/null || cat "$(npm root -g)/@cdot65/prisma-airs-cli/node_modules/@cdot65/prisma-airs-sdk/package.json" | grep '"version"'

# 4. Confirm credentials work end-to-end (lists profiles via OAuth2 — fails fast if creds are wrong)
airs runtime profiles list
```

!!! tip "Running from source"
    If you're testing an unpublished change, run `pnpm install && pnpm run build && npm link` from the repo root, then use `airs` as normal.

---

## Step 2 — Runtime Security

Each command exercises a different Zod schema in the SDK. Replace `<profile>` with one of the profile names returned by step 1, command 4.

```bash
# 1. List all profiles — exercises Policy + ProfileList schemas; biggest payload, most likely to surface drift
airs runtime profiles list

# 2. Get one profile in JSON — exercises full single-profile shape including topic-list with revision pinning
airs runtime profiles get <profile> --output json

# 3. List custom topics — exercises CustomTopic schema (revision/description/examples now required in 0.8.0)
airs runtime topics list

# 4. Sync scan a benign prompt — exercises ScanResponse schema (timeout/error/errors now required in 0.8.0)
airs runtime scan --profile <profile> "What is the capital of France?"
```

---

## Step 3 — AI Red Teaming

```bash
# 1. List attack categories — small read, fast Zod sanity check on the catalog endpoint
airs redteam categories

# 2. List all targets — exercises target list shape (target_background, target_metadata field names)
airs redteam targets list

# 3. List custom prompt sets — exercises customAttacks list (CUSTOM scan source-of-truth)
airs redteam prompt-sets list

# 4. List recent scan jobs — exercises scan job shape (ASR/score/threatRate fields)
airs redteam list
```

---

## Step 4 — Model Security

```bash
# 1. List security groups — exercises group shape across all source types (LOCAL, S3, GCS, AZURE, HUGGING_FACE)
airs model-security groups list

# 2. List rules — exercises rule shape (snake_case → camelCase normalization path)
airs model-security rules list

# 3. List rule instances for one of the groups from command 1 — exercises state enum
#    (BLOCKING | ALLOWING | DISABLED). Replace <groupUuid> with the UUID of any
#    group from the previous command's output.
airs model-security rule-instances list <groupUuid>

# 4. List recent scans — exercises scan summary shape (evaluations/violations/files counts)
airs model-security scans list
```

---

## Interpreting results

Quietly successful output across all 16 commands means the SDK and AIRS API are in sync — your release is good to ship.

If one of the commands errors out with something like:

```text
AISecSDKException: RESPONSE_VALIDATION — <field path> ...
```

…that is SDK 0.8.0's runtime Zod validation catching a wire-format mismatch. Note the failing endpoint and the field path it complains about, then file an issue (or open a fix on the SDK side if the API changed legitimately).

!!! danger "Don't silently swallow `RESPONSE_VALIDATION`"
    Per the SDK 0.8.0 migration guidance, `ErrorType.RESPONSE_VALIDATION` indicates a real schema drift worth surfacing. Do not catch and ignore it.

## When to run

- Immediately after publishing a new CLI release to npm
- Immediately after upgrading the `@cdot65/prisma-airs-sdk` dependency
- After a major AIRS backend rollout, especially if release notes mention API surface changes
- When debugging mysterious failures that don't reproduce locally — a `RESPONSE_VALIDATION` from one of these commands tells you instantly which endpoint drifted
