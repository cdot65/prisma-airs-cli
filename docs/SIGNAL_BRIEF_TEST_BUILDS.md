# Signal Brief development test builds

These are unpublished **8.1.0-rc.4** validation builds of the Signal Brief
implementation and application-bound scanner evidence, targeting stable **8.1.0** after real-tenant validation. Both the
crate and executable report 8.1.0-rc.4. Verify the checksums and BUILD-INFO.json.
The earlier RC3, RC2, RC1 and 8.0.0-labeled development archives are superseded. Saved
rules-v1 bundles remain supported offline; old binaries should not be used for the
new collection validation.

Build output directory on enterprise:

```text
target/dev-builds/signal-brief-8.1.0-rc.4/packages/
```

The same packages and testing fixtures are on worf at:

```text
/Users/cdot/airs-builds/signal-brief-8.1.0-rc.4/packages/
```

| System | Archive | Build host |
| --- | --- | --- |
| Linux amd64, portable static musl | `airs-cli-8.1.0-rc.4-x86_64-unknown-linux-musl.tar.gz` | enterprise |
| Linux arm64, portable static musl | `airs-cli-8.1.0-rc.4-aarch64-unknown-linux-musl.tar.gz` | enterprise (execution under QEMU) |
| Apple Silicon macOS arm64 | `airs-cli-8.1.0-rc.4-aarch64-apple-darwin.tar.gz` | worf |

RC4 adds named application scanner credentials, guarded routing and resumable bulk
references, positive management detector evidence, fair positive session priority,
per-app diagnostics and prominent session aggregates. The profile-by-service
enablement matrix remains. SQLite history belongs to the separate8.2 track.

Core implementation acceptance passed 3735 workspace tests with required parity
and no parity skips, formatting and Clippy; phases A0/A1/A2 each received independent
9/10. BUILD-INFO.json records the final RC4 platform, extraction, Rust1.89 and
documentation-build qualification receipts. Inspect that manifest for each artifact.


The prepared package directory contains SHA256SUMS, BUILD-INFO.json, SOURCE-SHA256.json,
this testing guide and synthetic smoke fixtures. The Linux executables use static
musl; macOS validation uses an ad hoc signature, without notarization. These are
unpublished test builds. The optimized host-native GNU binary is built at
`target/release/airs`. Existing RC3 artifacts remain at their original paths.

## Try a downloaded build

Copy the packages directory to the target machine. Verify its archives with
`sha256sum -c SHA256SUMS` on Linux or `shasum -a 256 -c SHA256SUMS` on macOS. Extract
only the archive for your platform into a new directory, for example on macOS:

```sh
mkdir test-airs
tar -xzf airs-cli-8.1.0-rc.4-aarch64-apple-darwin.tar.gz -C test-airs
./test-airs/airs --version
./test-airs/airs runtime report --help
```

The report help should include `--bundle-dir`, `--from-evidence`, `--review-file`
and `--max-reports`. Each archive contains the `airs` executable plus compatibility
symlinks `airs-cli` and `airs-cli-rs`, README.md, LICENSE and NOTICE.

Run the synthetic workflow without tenant or API access:

```sh
./test-airs/airs runtime report --from-evidence smoke-fixtures/evidence.json \
  --bundle-dir ./synthetic-brief
./test-airs/airs runtime report --from-evidence ./synthetic-brief/evidence.json \
  --review-file smoke-fixtures/agent-assessment.md --output-file ./reviewed.html
```

The default smoke-fixtures above are legacy rules-v1 snapshots that retain their
original 8.0.0 producer metadata. Also test current rules-v2 metadata findings:

```sh
./test-airs/airs runtime report --from-evidence smoke-fixtures/v2/evidence.json \
  --bundle-dir ./synthetic-v2-brief
./test-airs/airs runtime report --from-evidence ./synthetic-v2-brief/evidence.json \
  --review-file smoke-fixtures/v2/agent-assessment.md --output-file ./reviewed-v2.html
```

The v2 fixture includes one metadata-only finding and one illustrative report
example. Both fixture families are synthetic; replay preserves their provenance.

Open `synthetic-brief/report.html` and `reviewed.html`. The second contains an
explicitly synthetic agent assessment. Existing output files/directories are never
overwritten; choose a new path when repeating these commands.

For your own tenant, use the isolated store and explicit application bindings
described below. Start with metadata-only collection. Management credentials and
usable application-bound scanner credentials are needed for full result coverage:

```sh
./test-airs/airs --tenant YOUR_TENANT runtime report --strict --max-reports 0 \
  --bundle-dir ./live-metadata-brief
```

RC4 selects up to five distinct example report IDs by default. The hard maximum
is ten IDs and two actual report sends across the entire invocation, including
all application credential groups. Different apps require separate batches; the
global budget can defer examples even when fewer than five IDs are selected.
Calls are serial without retries; 429 stops enrichment without waiting.
`--max-reports 0` makes no Reports API calls and retains metadata-backed findings.
The direct `scan-logs reports` command also caps distinct IDs at ten.

Required configuration runs before session traversal. Missing results may receive
bounded exact-identity management metadata fallback, but transaction status never
supplies missing allow/block decisions. Large inventories may still exceed page/time
limits. Strict mode exits 1 after saving if required metadata remains incomplete;
missing optional examples alone do not fail strict. Use a fresh path such as
`./signal-brief-rc4-validation` for comparison with RC3.

Real-service qualification remains user-run; these builds have synthetic loopback
and offline qualification coverage. Report generation does not submit scans or
change enforcement.

## Forgejo runners

No runner is needed for these one-off builds: enterprise builds Linux locally and
worf builds macOS over SSH. For repeatable CI, the existing manual workflow
`.forgejo/workflows/ci-macos.yml` expects a `macos-arm64` runner label. Register or map
that label to a repository-scoped macOS runner under a dedicated build account,
following docs/TESTING.md section 10. Signing/notarization is separate from this
development test-build task.

## Configure application-bound scanner access

Use an isolated copy of your tenant store when evaluating the new format. Keep
that directory and its copied TOML files private (0700/0600). Set only the store
path through `AIRS_CLI_CONFIG_DIR`; credentials/endpoints still come from TOML.
Do not point an older CLI at the upgraded test store.

```sh
# TENANT and APP_ID must be your exact registered tenant and dashboard app ID.
airs tenant scanners bind-default TENANT monitor --app-id APP_ID --use key
airs tenant scanners list TENANT
airs --tenant TENANT runtime scan-logs results --scanner monitor --scan-id KNOWN_SCAN_UUID --output json
```

`--use key` selects the flat key; choose `--use token` only for the intended token.
The command never guesses precedence when both exist. The binding is a copy:
rotating flat settings does not rotate it. Separate app credentials can be supplied
with `tenant scanners set TENANT NAME --file PRIVATE_TOML` (0600), with
applicationId, endpoint and exactly apiKey or apiToken. Never pass secrets in argv.

The first named write saves a private rollback snapshot. Inspect metadata with
`airs tenant scanners snapshots TENANT`; restore an exact snapshot using
`airs tenant scanners restore TENANT --file SNAPSHOT_PATH --expect-tsg TSG_ID --force`.
Restore first preserves the current config in another private snapshot. Snapshot
files contain credentials and should be retained/deleted deliberately.

## Budgeted real-service qualification — user-run only

Use a known existing scan and its administrator-confirmed app credential. The
agent does not perform these calls. Do not search across keys or regenerate keys
for this check. Keep the same scan/app/profile identifiers for comparison.

1. Run the result lookup above with that one named binding. Check the returned
   identity and separate malicious/benign classification from allow/block action.
   An empty, failed or denied read does not establish why access is unavailable.
2. Generate a metadata-only report: `airs --tenant TENANT runtime report --strict
   --max-reports 0 --bundle-dir NEW_METADATA_BUNDLE`. This spends zero report API
   requests. Unbound applications remain visible and can make strict collection
   incomplete; bind only credentials you actually possess for those apps.
3. If the result supplied a report ID, optionally query that one example once:
   `airs --tenant TENANT runtime scan-logs reports --scanner monitor --report-id
   KNOWN_REPORT_ID --output json`. This is at most one report send. Do not repeat
   it after HTTP429 or authorization denial. After either response, stop all
   remaining Reports API qualification and skip step4.
4. A final example-enriched report may use `--max-reports 5` and a new bundle path.
   It selects up to five IDs and makes at most two report sends total. HTTP429
   stops remaining sends without waiting or retrying.

These optional steps together spend at most three report sends. No new scans are
required. Wrong-key and post-rotation probes are separate opt-in investigations;
this recipe does not qualify either behavior. Saved evidence can be rendered
offline with `runtime report --from-evidence BUNDLE/evidence.json --output-file
NEW_REPORT.html`, spending no API quota.
