---
"@cdot65/prisma-airs-cli": patch
---

`airs runtime dlp filtering-profiles get`, `profiles get`, `patterns get`, and `dictionaries get` no longer mangle JSON/YAML keys for multi-word fields. The label→key transformer used by the detail renderers tried to produce camelCase but stripped uppercase letters in its final pass, yielding `datarofile`/`scanype`/`fileased`/`nonileased`/`fileypes`/`profileype`. It now emits snake_case directly: `data_profile`, `scan_type`, `file_based`, `non_file_based`, `file_types`, `profile_type`. `pretty` output was unaffected.
