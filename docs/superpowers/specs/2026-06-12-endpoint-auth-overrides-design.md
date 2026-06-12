# Endpoint + Auth Overrides — Full SDK Parity

**Date:** 2026-06-12
**Status:** Approved

## Problem

The SDK supports overriding auth and API endpoints on every client surface, but the CLI
config (`~/.prisma-airs/config.json` + env + CLI overrides) only exposes a subset. Users
pointing at non-default environments (staging, EU regions, proxies) can only do so via
SDK env-var fallback — the config file and CLI overrides can't set them, and bearer-token
auth (`apiToken`) for the runtime scan API isn't plumbed at all.

### Gap matrix (before)

| SDK surface | SDK supports | CLI config today |
|---|---|---|
| Runtime scan (`init`) | `apiKey`, `apiToken`, `apiEndpoint`, `numRetries` | only `airsApiKey` |
| Management | clientId/secret/tsgId/endpoint/tokenEndpoint | all covered |
| DLP | endpoint | `dlpEndpoint` |
| Red team | dataEndpoint, mgmtEndpoint, tokenEndpoint | only tokenEndpoint (reused mgmt) |
| Model security | dataEndpoint, mgmtEndpoint, tokenEndpoint | only tokenEndpoint (reused mgmt) |

## Design

### 1. Schema additions (`src/config/schema.ts`)

All optional; no defaults (absent → SDK resolves its own env/defaults):

- `airsApiToken` — string; bearer-token alternative to `airsApiKey`
- `airsApiEndpoint` — string; runtime scan API URL
- `airsNumRetries` — coerced int, 0–5
- `redTeamDataEndpoint`, `redTeamMgmtEndpoint`, `redTeamTokenEndpoint` — strings
- `modelSecDataEndpoint`, `modelSecMgmtEndpoint`, `modelSecTokenEndpoint` — strings

### 2. Loader env mapping (`src/config/loader.ts`)

Reuse the SDK's exact env names so behavior is identical whether the SDK or the CLI
resolves the variable:

| Config key | Env var |
|---|---|
| `airsApiToken` | `PANW_AI_SEC_API_TOKEN` |
| `airsApiEndpoint` | `PANW_AI_SEC_API_ENDPOINT` |
| `airsNumRetries` | `PANW_AI_SEC_NUM_RETRIES` |
| `redTeamDataEndpoint` | `PANW_RED_TEAM_DATA_ENDPOINT` |
| `redTeamMgmtEndpoint` | `PANW_RED_TEAM_MGMT_ENDPOINT` |
| `redTeamTokenEndpoint` | `PANW_RED_TEAM_TOKEN_ENDPOINT` |
| `modelSecDataEndpoint` | `PANW_MODEL_SEC_DATA_ENDPOINT` |
| `modelSecMgmtEndpoint` | `PANW_MODEL_SEC_MGMT_ENDPOINT` |
| `modelSecTokenEndpoint` | `PANW_MODEL_SEC_TOKEN_ENDPOINT` |

Precedence unchanged: CLI > env > file > defaults.

### 3. Wiring

- **`SdkRuntimeService`** (`src/airs/runtime.ts`): constructor signature changes from
  `(apiKey: string)` to `(opts: { apiKey?, apiToken?, apiEndpoint?, numRetries? })`,
  passed through to SDK `init()`. Update all call sites (`src/cli/commands/runtime.ts` ×3,
  plus any in topics-eval).
- **Runtime command validation:** accept `airsApiToken` OR `airsApiKey` (currently
  requires `airsApiKey`). Error message names both.
- **Red team factories** (`src/cli/commands/redteam.ts`, `backup.ts`): add
  `dataEndpoint: config.redTeamDataEndpoint`,
  `mgmtEndpoint: config.redTeamMgmtEndpoint`,
  `tokenEndpoint: config.redTeamTokenEndpoint ?? config.mgmtTokenEndpoint`
  (preserves current fallback).
- **Model security factory** (`src/cli/commands/modelsecurity.ts`): same pattern with
  `modelSec*` keys.
- **Credentials stay shared with mgmt** for red team / model security. The SDK's own
  `PANW_RED_TEAM_CLIENT_ID` (etc.) env fallback still enables per-service credential
  splits — no new config keys (YAGNI).

### 4. Behavior notes

- All new keys `undefined` by default → SDK falls back to its own env/defaults. **No
  behavior change for existing users.**
- No URL-format validation beyond zod string — SDK strips trailing slashes and errors
  clearly on bad URLs.

### 5. Testing

- Loader unit tests: file/env precedence for each new key; `airsNumRetries` coercion +
  range.
- Runtime service: opts pass-through to `init()` (mock SDK).
- Factory wiring: red team / model-sec factories forward endpoint keys; tokenEndpoint
  fallback to mgmt preserved.

### 6. Docs & release

- README config table: new keys + env vars.
- Changeset: **minor** (new feature, backward compatible).
