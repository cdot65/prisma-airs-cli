---
'@cdot65/prisma-airs-cli': patch
---

Rewire `redteam properties` commands for SDK 0.10.0 response shapes.

- `properties list` now renders the SDK's plain `string[]` (was incorrectly typed/rendered as `{name}[]`); adds `--output json|yaml` support.
- `properties values <name>` now renders the SDK's `{name, values: string[]}` object (was incorrectly typed/rendered as `{name, value}[]`); adds `--output json|yaml`.
- `properties create` and `properties add-value` now print the SDK's mutation message instead of fabricating a fake `name=value` line.
- Adds curated input/output examples for `redteam properties list` (refreshed), `redteam properties values`, and `redteam properties add-value`; updates `.missing-allowlist` accordingly.
