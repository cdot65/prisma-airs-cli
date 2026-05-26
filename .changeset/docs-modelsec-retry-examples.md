---
"@cdot65/prisma-airs-cli": patch
---

Re-probed all 14 model-security commands previously blocked under epic #127 (#130 — license-deferred + fixture-blocked) against a fresh model-security-licensed tenant. Landed curated input/output examples for `groups create/update/delete` (#132/#133/#135), `rule-instances update` (#139), `labels add/set/delete` (#136/#137/#138), `scans create/get/evaluations/evaluation/violations/violation/files` (#141/#142/#143/#144/#145/#147/#148). All 14 entries removed from `.missing-allowlist` (60 → 44). Sub-issues closed.
