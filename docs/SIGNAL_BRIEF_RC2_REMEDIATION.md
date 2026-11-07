# Signal Brief RC2 remediation

Status: implemented and qualified as unpublished 8.1.0-rc.2. All phases 1–5
independently accepted at 9/10. Required checks and native/package validation pass;
repeat user-run real-service qualification remains before stable publication.

User-approved scope, 2026-11-03. Stable publication remains on hold. The RC1
user-run validation identified required configuration starvation and detector alias
mismatches, plus unresolved scan results whose cause is not established offline.
Reports API calls are limited to concrete, strategically selected examples.

1. Collection scheduling: independent required configuration walks first, bounded
   fair operation time, actual concurrent session-detail pages, optional telemetry
   last. Preserve incomplete traversal status under page/time/attempt caps.
2. Metadata-backed findings: analysis rules v2, recognized detector equivalence
   with raw provenance retained, scan verdict/action distinct from detector action,
   findings/counts independent of optional report examples. Keep rules-v1 saved
   evidence and review-packet binding reproducible offline.
3. Reports API restraint: default five distinct IDs, hard maximum ten, batches of
   five, maximum two actual endpoint attempts per invocation, serial, no retries,
   stop on 429. Selected reports supplemental. Direct report lookup also capped at
   ten distinct IDs; results lookup retains its separate bounds. Show selection,
   coverage gaps, request counts and evidence limitations.
4. Missing-result investigation: bounded exact-identity session-transaction SDK
   fallback for explicit management metadata only. This model has no typed scan
   enforcement/category/report ID: never invent those from status or extensions.
   Keep unresolved scanner coverage visible. No Reports API fishing or live probes.
5. Qualification and RC2: loopback slow/large/rate-limited fixtures, offline saved
   bundle checks, required workspace/parity/lint/format checks plus MSRV and docs.
   Independent Codex review of every phase must score at least 9/10 before that
   phase is accepted. Build unpublished 8.1.0-rc.2 Linux amd64 and macOS arm64
   packages, verify their extracted binaries, document user-run live validation.

Phase acceptance and check results are recorded in SIGNAL_BRIEF_PHASE_REVIEWS.md
and SIGNAL_BRIEF_VALIDATION.md. No real tenant calls, new scans, full-content reads,
enforcement writes, commits, tags or publication are part of this work.
