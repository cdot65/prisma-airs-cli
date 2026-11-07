# Isolated runtime history feasibility probe

This workspace is the B0 platform/performance gate. It is not linked into `airs`.
Its own lockfile pins the candidate bundled SQLite binding and dependencies;
the root Cargo manifest and lockfile do not adopt SQLite during this phase.
All generated data is synthetic. No tenant, credentials, network client, or SDK
is used. Project Rust code forbids unsafe.

```sh
cargo +1.89.0 test --locked --manifest-path prototypes/runtime-history/Cargo.toml
cargo +1.89.0 clippy --locked --all-targets --manifest-path prototypes/runtime-history/Cargo.toml -- -D warnings
cargo +1.89.0 build --release --locked --manifest-path prototypes/runtime-history/Cargo.toml
prototypes/runtime-history/target/release/airs-history-prototype
```

The default invocation checks private journals, unchanged read-only files,
missing-file refusal, two-query snapshot consistency, query/lock deadlines,
maintenance-gate exclusion, whole-database secure deletion and a private verified
`VACUUM INTO` copy. It prints JSON. Fixtures are private and cleaned on exit.

The benchmark accepts a row count and a **new** database path inside an existing
private directory. It deliberately leaves the synthetic database for inspection:

```sh
mkdir -m 700 /absolute/private/benchmark
prototypes/runtime-history/target/release/airs-history-prototype benchmark \
  1000000 /absolute/private/benchmark/archive.sqlite3
```

It writes 200-record transactions spanning three years and sixteen applications,
retains changed facts, deduplicates repeated facts and labels partial receipts
separately. Each 30/90-day summary uses one read transaction with a two-second
monotonic deadline. Output records counts, timings, size and integrity results.
Capture peak RSS externally (`/usr/bin/time -v` on Linux, `time -l` on macOS).
The process sets a 256 MiB SQLite allocator cap once; this does not bound total
process RSS. B6 must repeat full product report/compaction measurements.

This is not the B1 repository implementation: hostile-path admission, journal
recovery validation, crash-safe publication, migrations, normalized-fact merging
and receipt reconciliation remain separately tested integration work. Prototype
SQL uses only fixed statements and bound values. Ordinary connections permit zero
attached databases; only the fixed compaction statement temporarily permits the
one internal attachment required by SQLite's `VACUUM INTO` implementation.
