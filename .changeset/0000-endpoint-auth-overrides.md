---
"@cdot65/prisma-airs-cli": minor
---

Add full SDK parity for authentication and API endpoint overrides. New config keys (settable via `~/.prisma-airs/config.json` or env): `airsApiToken` (bearer-token alternative to the scan API key), `airsApiEndpoint`, `airsNumRetries`, `redTeamDataEndpoint`, `redTeamMgmtEndpoint`, `redTeamTokenEndpoint`, `modelSecDataEndpoint`, `modelSecMgmtEndpoint`, `modelSecTokenEndpoint`. Scan commands now accept `PANW_AI_SEC_API_TOKEN` in place of `PANW_AI_SEC_API_KEY`.
