---
"@cdot65/prisma-airs-cli": minor
---

Removed the legacy `airs runtime dlp-profiles list` command. Use `airs runtime dlp profiles list` instead — it is now the canonical DLP profile listing and returns populated IDs plus `type`, `profile_type`, `status`, and `version` fields. The two endpoints overlap heavily but are not identical on the same tenant (see release notes Migration note for legacy-only and new-only profile name differences observed in #226).
