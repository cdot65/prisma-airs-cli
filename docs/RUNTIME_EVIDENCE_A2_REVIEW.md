# Runtime evidence A2 — independent implementation review

## Review 1 — 2026-11-04 — changes required

Reviewer: independent Codex instance `/root/phase6_review`.

Scope: implementation plan A2; DESIGN D344/§18.14; typed management observations,
session priority/fairness, lookup diagnostics, client transport counters and
executive-summary session aggregates. A0/A1 acceptance is a prerequisite, not
replaced by this review. No A3 acceptance is implied.

The initial probes exercised the pre-correction CLI while the uncommitted A2
working tree was changing. No immutable source manifest was supplied for that
first binary. Corrections and the final reviewed snapshot must be distinguished
from the initial failures below; a later file hash is not proof of the initial
binary's source.

**Initial score: 8/10 — do not advance to A3.** The ordinary routing/evidence
workflow passes, but the new aggregate summary needs population/semantic safety
corrections. Preserve this unsuccessful assessment when appending a rescore.

### Material findings

1. **Summary population and interpretation were underspecified in rendering.**
   Initial `session_activity` grouped only by population and omitted window and
   scale. Independent synthetic, valid-digest offline probes rendered numeric
   total/violated pairs when their windows differed, when total carried a scale,
   and when violated sessions exceeded total sessions. All probes exited 0.
   Required: exact window separation and labeling; no silently scaled counts;
   explicit Unknown/data-quality treatment of contradictory pairs. The author
   has since added those source changes and a focused summary test. Independent
   execution of that test passed. Final acceptance still requires the complete
   corrected snapshot and checks.

2. **Privacy projection can create a new aggregate association or semantic label.**
   The new renderer groups metrics by population/window and recognizes metric
   name/unit. At initial inspection `privacy::correlation_keys` protected only
   metric.source. Independent synthetic valid-digest offline probes confirmed:
   two different email-bearing populations with complementary total/violated
   metrics collapse to the same withholding marker and render as one numeric
   pair; `sessions_\ntotal` becomes a recognized total; `ses\nsions` becomes the
   recognized unit. All probes exited 0. Shared projection must not manufacture
   these joins or labels. Guard newly semantic metric fields and preserve
   injectivity of newly grouped fields, or otherwise fail closed before the
   summary can gain associations. Add executable regressions for all three cases.
   This finding remains blocking at the time of this entry.

### Other review findings and verified behavior

- Independently ran all 14 `runtime_scanner_routing` tests: all passed. The
  unbound-app case retains positive action detector observations with unknown
  scan action/verdict/direction, issues no scanner/report calls, and shows
  reported violated sessions without turning them into detector/event counts.
- Session overview detector breakdowns remain session-level metrics rather than
  hints copied onto every action. Positive sessions are stably prioritized inside
  each application queue; the application ring remains round-robin. Existing
  aggregate page/event ceilings remain in force.
- An additional independently compiled loopback harness probe with 1,001 positive
  overview detector rows retained 1,000 detector metrics and reported a partial
  sessions source with an explicit breakdown-cap note. The inspected updated
  source now discloses overview truncation, as well as action truncation.
- New overview metadata and action hints have separate 4 MiB retention bounds;
  metrics have a 10,000-row cap. Source status/notes record truncation. Whole
  evidence publication retains its existing bounded projection/storage contract.
- `LastStatus` counts fixed SDK operation names at the client transport inside
  shared budget admission. Separate clients avoid concurrent application count
  inflation; local validation or budget refusal is not a recorded transport send.
  The operation name is static SDK metadata, not server-supplied input.
- Lookup diagnostics distinguish ID, row, event and transport-attempt units.
  Missing bindings have zero sends. Pending wrappers, empty successful arrays,
  failed SDK calls and absent IDs remain different evidence. No wrong-key,
  rotation, sync-only or retention explanation is inferred from missing rows.
- Results remain exactly correlated by scan/subrequest and supported application/
  session context. Optional reports remain under the existing invocation-wide
  five/default, ten/maximum and two/actual-send constraints; no additional API
  endpoint or payload-content read was introduced by A2.
- Typed metadata ignores extension-supplied detector actions/directions. Existing
  verdict/action separation, malicious-plus-allow monitoring interpretation,
  source attribution, secret projection and immutable offline evidence remain
  required. The projection finding above must be fixed within that same boundary.

### Validation and qualification limits

Independent logs: `/tmp/airs-history-a2-independent-routing.log` and
`/tmp/airs-history-a2-independent-summary.log`. The extra overview-cap harness was
compiled and executed from `/tmp/airs-a2-cap-review.rs`; the fixture was synthetic
and every endpoint loopback. The valid-digest summary probes used temporary
synthetic evidence and an empty isolated tenant-store path.

Final scoring requires the corrected source, executable semantic/collision
regressions, focused reruns, the applicable paired D344 test and required
fmt/Clippy/full workspace parity results. A3 platform packaging/live qualification
is a later phase. This review does not establish an upstream missing-result cause.

Only this review file was edited in the repository by the reviewer. No private
bundle, tenant configuration, credential, live API, SDK edit or feature change
was used.

## Review 2 — 2026-11-04 — final A2 acceptance

Reviewer: independent Codex instance `/root/phase6_review`.

**Final score: 9/10 — A2 accepted.** No unresolved material A2 blocker remains.
The initial 8/10 assessment above is preserved. A3 release qualification is a
separate phase; this score does not qualify packages or establish live service
behavior.

### Corrections and independent rechecks

The executive summary now groups by exact population and reported window, shows
that window, accepts only unique complete unscaled safe-integer counts, and marks
both values Unknown when violated sessions exceed total sessions. A data-quality
cell and explicit gap explain contradictions. The focused renderer test covers
window separation, scale, contradiction, duplicate values, escaping and the
20-bucket display cap; it passed independently.

Metric source/name/unit/window/scale are now exact semantic keys during privacy
projection. Population participates in the existing injectivity guard. I reran
six independent valid-float, valid-digest offline cases: the pristine control
rendered successfully; distinct-population collapse and normalized name, unit,
window and scale cases all failed with exit 1 and empty stdout. The guard prevents
new aggregate associations and recognized labels from being created by masking or
control removal.

The first regression draft could fail before reaching projection because raw
integer metric values were hashed before conversion to typed floating values;
its source references also needed a valid applications source. The repaired test
uses floating values, includes that source and proves a pristine baseline renders
before testing each mutation. Its five refusal cases preserve the input and
publish no artifact. The full offline suite passed independently after that
repair. This test-validity finding was corrected rather than counted as proof.

The direction-enrichment follow-up preserves original management and scanner
hints in evidence. `detectors::observation_hints` performs comparison-only
canonicalization and subsumes an unknown-direction observation only when that
same canonical detector has an explicit positive direction for the event.
Analysis and report selection share it. The unit regression keeps genuine prompt
and response observations separate, retains an unrelated unknown-direction
detector, and checks an unknown-only observation. The routing regression retains
both raw hints and source references while rendering one corresponding finding.

### Final evidence and scope assessment

- Independently executed 43 Signal Brief unit tests, 15 scanner-routing tests and
  12 offline tests: all passed. This includes management-only positive evidence,
  missing credentials, separate aggregate units, application fairness, scoped
  transport counts, rate-limit isolation, privacy and raw-hint preservation.
- Independently ran the paired D344 test with Node 24 and
  `AIRS_REQUIRE_PARITY=1`: passed against the frozen TypeScript reference. The
  final full required suite also includes that test.
- Inspected final focus/fmt/Clippy/workspace receipts: all exit 0. Independently
  parsed the complete workspace log: **93 groups, 3,735 passed, 0 failed,
  5 ignored, 0 PARITY SKIPPED notices**. These match the recorded summary.
- Metadata counting is source/unit-aware. Overview observations remain session
  metrics; action breakdowns can establish an event/detector observation without
  inventing scan enforcement, detector direction, report ID or a payload. Known
  positive sessions receive stable priority while page scheduling remains fair
  across application buckets.
- Overview breakdown truncation, retained metadata/action byte caps and metric
  row caps are explicit. Per-app/client operation counters count admitted
  transport sends without conflating local refusal, returned rows or usable
  event observations. Diagnostics state missing-data uncertainty without
  assigning an unproven authentication or endpoint cause.
- Report examples remain supplemental and share the existing global ID/attempt
  limits across named clients. No additional report requests or content reads
  were added for metadata recovery or diagnostics. Existing direct lookup shape,
  credential-plane isolation, private publication, offline replay and other
  report families remain covered by the passing required suite.

Validation receipts:
`.local/dev/runtime-history/a2-acceptance-results.json`,
`a2-focus-acceptance.log`, `a2-workspace-acceptance.log` and
`a2-test-summary.json`. Independent focused logs are
`/tmp/airs-history-a2-independent-signal-final.log`,
`/tmp/airs-history-a2-independent-acceptance-final.log`,
`/tmp/airs-history-a2-independent-summary.log` and
`/tmp/airs-history-a2-independent-parity.log`.

### Final reviewed source fingerprints

The uncommitted source was stable through the final required checks. SHA-256:

| File | Hash |
| --- | --- |
| src/clients.rs | a724b59d17b0ae2c229989ce43e80f6b567a1895a207ee18ee45049a5b6247ca |
| src/report/signal/collect.rs | 98f4d4ade44c70892e60a81fad9d636cac2cc60c43822628a822dc437a3168ab |
| src/report/signal/analyze.rs | 67a6c06c811a79cf1e3b3d7eaae6639e5a9ceac357d636e969417682956babc1 |
| src/report/signal/detectors.rs | 4e400583dbe96be099b5eb91d34413b99aafbcf7d74234126d0f3da44e899bf7 |
| src/runtime/scan_logs.rs | a7397437ef2e22c5eb8cf0ee7482a373b2dde03633581bc7988ecf092079b89d |
| src/report/signal/html.rs | 0e342399303ccebe4da2047dcc48ecc22bee882a499537ea22fca6bae35793fe |
| src/report/signal/privacy.rs | 9b70b6d1a2c1499dd1a47637fbc334a7eaf27eeade54117ccdc6403af334e6ec |
| src/commands/runtime_scan_logs.rs | ed65be5bbca7a24d9cada612e983a34154c35f2fc11a8e6160e97f3a24c06f44 |
| tests/runtime_scanner_routing.rs | f004fafcef37b4813c0dbbf538597aa83eb2f40cf1b54e6f9e4cc156cf46b043 |
| tests/signal_offline.rs | ebdf61f187fd4ce2135e3b4f9c9394e1315a3beeef7bc87d1613f2cd45826471 |
| tests/parity_runtime_credentials.rs | 487a75cda74e8058241c8a4adbc12ce28c45316e45a394e491533b76eb758358 |

Only this review file was edited in the repository by the reviewer. Test probes
used synthetic evidence and loopback endpoints. No private bundle, real tenant
configuration, credential, live API, SDK edit, publication or feature edit was
used. Separate user-run service qualification remains necessary in A3.
