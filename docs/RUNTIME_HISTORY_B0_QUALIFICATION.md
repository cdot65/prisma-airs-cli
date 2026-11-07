# B0 bundled SQLite qualification

Status: complete, independently accepted **9/10**. The initial 8/10 review and corrections
are preserved in [RUNTIME_HISTORY_B0_REVIEW.md](RUNTIME_HISTORY_B0_REVIEW.md).
This qualifies an isolated feasibility probe, not integrated history commands.

## Candidate and reproducibility

- Rust 1.89.0 on enterprise and worf, rusqlite=0.41.1 with default features off and
  bundled/hooks/limits enabled; separate checked-in prototype Cargo.lock resolves
  libsqlite3-sys 0.39.1 and SQLite 3.54.1.
- Project code forbids unsafe. No system SQLite library/executable is required.
  The default bundled library has load-extension support compiled, but connection
  extension loading is disabled and an attempted SQL load is verified unauthorized.
- The root CLI has no SQLite dependency. All 337 qualified RC4 source/test/manifest/
  lock files remain byte-identical to the frozen RC4 source manifest.
- Reproduction commands: [prototype README](../prototypes/runtime-history/README.md).
  Machine-readable logs, hashes and telemetry are in the private local directory
  `.local/dev/runtime-history/b0/acceptance2/`; no customer data or credentials.

Reviewed executable-source hashes:

- `prototypes/runtime-history/Cargo.toml`: `94876efa5b38defad89c73a55b69edb4ee23c17ebe8dbcba683bd71662624931`
- `prototypes/runtime-history/Cargo.lock`: `fefcf60850bc2361bda53697694db90df71c5d897a5b7b5e864bf18d1463da6d`
- `prototypes/runtime-history/src/main.rs`: `ddef651ae04f085912d4798ebcbb3e9c919d0ef28ee35e2cf900a70373a5a574`
- `src/history/schema.sql`: `a8f92645e56f14b6b129ae86141bd0ea4c4a7a7d2c4f0c0b36885e31d130df01`

## Measured synthetic workload

Every benchmark has 1,000,000 distinct scan UUIDs/full-event observations, 16 apps,
three years, 1,000 meaningful contradictory facts, 1,000,000 duplicate fact attempts
that create no rows, and one separate unpromoted receipt. No API traffic. Windows
are UTC half-open. Counts are asserted against the generator, independently of
query output. Each 30/90-day read computes total/distinct scan counts, verdict/action
counts, application groups, detector counts and partial receipts in one transaction.

| Target | 30-day read | 90-day read | Ingest | Peak process RSS | Write p99 / max |
| --- | ---: | ---: | ---: | ---: | ---: |
| Linux amd64 musl | 27.1 ms | 155.3 ms | 73.5 s | 45.3 MiB | 16.3 / 54.4 ms |
| macOS arm64 | 18.7 ms | 139.4 ms | 25.8 s | 33.5 MiB | 5.9 / 13.7 ms |
| Linux arm64 musl under QEMU | 287.4 ms | 1276.8 ms | 159.8 s | 59.1 MiB | 37.7 / 68.4 ms |

Linux host: enterprise, AMD Ryzen 9 9950X3D, 32 logical CPUs, about 123 GiB RAM,
local Btrfs. macOS host: worf, Apple M5 Pro, native arm64. Linux arm64 uses
qemu-aarch64-static 11.1.2 via binfmt; its process RSS includes the emulator.
These are fresh SQLite connections after ingestion with warm OS filesystem cache;
no cold-cache or full product-rendering claim. Database size756,043,776 bytes on
the final schema; 200-record transactions, 8 MiB SQLite page-cache target, MEMORY
temporary store, mmap disabled and 256 MiB SQLite allocator cap. RSS comes from the
individual benchmark child's wait4 usage, in KiB on Linux and bytes on Darwin.
The cap is not a bound on total process RSS.

The initial GNU pilot had a 277.7 ms maximum write transaction. It is retained as
observed tail latency, not hidden by the successful final musl measurements. B1/B6
must measure contention and write-hold tails with the actual repository and adapt
batch sizes as necessary; no hard real-time filesystem completion is promised.

## Safety and deadline evidence

The same executable on all shipped targets must pass:

- DELETE/FULL, secure_delete ON, MEMORY temp, foreign keys, trusted-schema off,
  parameter/SQL limits, read-only connections and disabled extension loading.
- Initial private 0600 database, private rollback journal during a real write,
  unchanged database/journal bytes, modes and mtimes when a reader runs concurrently,
  no creation of a missing database, and stable two-query reader snapshot while a
  writer cannot commit until the read transaction ends.
- Captured monotonic deadline interrupts a deliberately expensive recursive query.
  A forced 100 ms gate wait hands the original 250 ms contention deadline to SQLite;
  there is no fresh 250 ms SQLite timeout. The elapsed test tolerance is 50 ms solely
  for OS scheduling, not additional policy wait. Mac measures 250 ms total, with
 105 ms gate plus144 ms SQLite. Linux also passes: amd64-musl: 250 ms total (100 ms gate + 149 ms SQLite); arm64-musl-qemu: 250 ms total (101 ms gate + 148 ms SQLite).
- A marker demonstrably written to disk is absent throughout the whole live file
  after secure deletion; no rollback journal remains after successful commit.
- VACUUM INTO a precreated private file preserves 0600 and produces an integrity-
  checked copy, followed by explicit file/directory fsync. Only that fixed statement
  temporarily permits one SQLite internal attachment.
- Invalid promoted receipts are rejected, event deletion requires explicit
  demotion, retry reset preserves prior 403 evidence, no-send attempts release an
  ordinal, repeated actual ordinals fail, and management traversal can record more
  than three pages without a scanner retry counter.

## Platform and root regression checks

All seven Linux checks pass: prototype format, Clippy with denied warnings, native
Rust 1.89 tests, both musl builds and both target test executables. Each target
runs two tests containing the safety/schema/benchmark assertions. Native Mac
Rust 1.89 tests and build pass; all three shipped targets execute their corrected
qualification binary and million-row workload successfully. Linux arm64 execution
is emulated, not native. The binary/source hashes and complete SQLite compile
options are retained in the machine-readable receipts. Root format and all seven
documentation-consistency tests pass. A3's full required checks remain valid for
the unchanged CLI sources; this phase adds no root dependency or runtime behavior.

## Scope still requiring repository qualification

B1 owns hostile-path/ownership/symlink admission, journal recovery refusal/explicit
maintenance, crash/disk-full injection, migrations/backups, transactional normalized
merging and receipt reconciliation. B2 owns automatic retention and one-warning
scan semantics. B3–B5 own API sync, lifecycle and historical HTML. B6 repeats full
report/compaction memory and performance tests. The small private-copy prototype
is not million-row compaction/RSS qualification. No real-service call, secret read,
SDK modification, publication or commit occurred during this phase.


B1 reproducibility note: the prototype now embeds its own `schema.sql`, copied
byte-for-byte from the accepted B0 source archive. This isolates the original
benchmark from production schema evolution. The accepted source/receipt hashes
above still identify the frozen original run; this include-path-only packaging
change does not claim a new benchmark run or replace those receipts.
