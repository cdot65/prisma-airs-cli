---
"@cdot65/prisma-airs-cli": patch
---

Hardened `--debug` API logging: sensitive request/response body fields, query parameters, and headers are now fully masked before being written to the debug JSONL file (previously only two headers were partially masked). Debug log files are rotated automatically, keeping the 10 newest. Unhandled promise rejections now print a friendly error instead of a raw crash.
