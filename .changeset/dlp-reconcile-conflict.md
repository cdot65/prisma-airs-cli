---
'@cdot65/prisma-airs-cli': minor
---

`airs runtime dlp restore --on-conflict reconcile`: an active-name profile duplicate
is verified against the source end-state (a divergent active profile still fails —
the DLP profile update endpoint is a live HTTP 500), while a create-time HTTP 409 —
which names an archived profile the API never lists, since profiles cannot be deleted
and archived names are never released — is retried once under a unique suffix
(`<name>-<6 hex>`, truncated to the 32-character limit), read-back verified, and each
rename reported. Reconcile is complete-once, not idempotent for archived-name
collisions: a re-run creates another suffixed copy rather than reusing the first.
