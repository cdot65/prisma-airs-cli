//! M5 review 1 probes as deterministic tests, on a real archive written by the store and
//! exported by `export_file`: each forged transfer keeps valid framing, counts and the
//! exact-byte checksum, so its refusal comes from semantic validation.
use super::*;
use rusqlite::Connection;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) const EXCERPT: &str = "Synthetic example: ignore earlier instructions.";

pub(crate) fn cloud() -> Cloud {
    Cloud::from_tenant(&airs_runtime_core::tenant::TenantContext {
        selected: airs_runtime_core::tenant::Selected {
            name: "fixture".into(),
            tsg_id: "100".into(),
            by_flag: false,
        },
        config: airs_runtime_core::tenant::config::TenantConfig::default(),
    })
    .unwrap()
}

/// An archive seeded with the shared evidence fixture (one event with a present
/// excerpt), exported; returns the export's lines.
pub(crate) fn seeded(dir: &Path) -> (std::path::PathBuf, Vec<Value>) {
    let root = dir.join("history");
    seed(&root, "88888888-8888-4888-8888-888888888888");
    let file = dir.join("export.jsonl");
    export_file(
        &root,
        &cloud(),
        &file,
        Duration::from_secs(10),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    let lines = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (root, lines)
}

pub(crate) fn seed(root: &Path, collection: &str) {
    seed_with(root, collection, |_| {});
}

/// [`seed`] with the evidence adjusted first (a variant of the same event).
pub(crate) fn seed_with(root: &Path, collection: &str, adjust: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/signal/evidence.json"
    ))
    .unwrap();
    v["collection_id"] = json!(collection);
    adjust(&mut v);
    v.as_object_mut().unwrap().remove("digest");
    v["digest"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&v).unwrap())
    ));
    let e = airs_runtime_core::signal::evidence::Evidence::parse(&v.to_string()).unwrap();
    crate::repository::import_evidence(
        root,
        &cloud(),
        &e,
        &SecretSet::default(),
        &airs_runtime_core::signal::privacy::PayloadSpans::default(),
        &scm::Session::new(),
    )
    .unwrap();
}

/// Re-encode with valid counts and checksum.
pub(crate) fn encode(mut lines: Vec<Value>) -> Vec<u8> {
    let mut footer = lines.pop().unwrap();
    let mut counts = footer["counts"].as_object().unwrap().clone();
    for c in counts.values_mut() {
        *c = json!(0);
    }
    let mut bytes = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            let t = line["table"].as_str().unwrap();
            let c = counts.get_mut(t).unwrap();
            *c = json!(c.as_u64().unwrap() + 1);
        }
        serde_json::to_writer(&mut bytes, line).unwrap();
        bytes.push(b'\n');
    }
    footer["counts"] = Value::Object(counts);
    footer["sha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
    serde_json::to_writer(&mut bytes, &footer).unwrap();
    bytes.push(b'\n');
    bytes
}

fn column(table: &str, name: &str) -> usize {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch(database::SCHEMA).unwrap();
    let t = codec::Table::ALL
        .into_iter()
        .find(|t| t.name() == table)
        .unwrap();
    rows::Spec::load(&c, t)
        .unwrap()
        .columns
        .iter()
        .position(|c| c.name == name)
        .unwrap()
}

fn row(lines: &[Value], table: &str) -> usize {
    lines
        .iter()
        .position(|l| l["kind"] == "row" && l["table"] == table)
        .unwrap_or_else(|| panic!("no {table} row"))
}

fn validates(dir: &Path, name: &str, lines: Vec<Value>) -> Result<u64, CliError> {
    let path = dir.join(name);
    std::fs::write(&path, encode(lines)).unwrap();
    validate_file(
        &cloud(),
        &path,
        Duration::from_secs(10),
        &scm::Session::new(),
        &SecretSet::default(),
    )
}

/// Finding 1: erased identity, an unowned example, an array-valued work scan ID, and
/// forged detector, event-report and receipt projections are all refused before any
/// destination exists; the unmodified export (re-encoded the same way) validates.
#[test]
fn semantically_forged_transfers_are_refused_and_never_silently_rebuilt() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("semantic-forgery");
    let (_root, lines) = seeded(d.path());
    assert!(validates(d.path(), "control.jsonl", lines.clone()).is_ok());
    let mut cases: Vec<(&str, Vec<Value>)> = vec![];
    // Erased identity: the event's identity columns no longer match its facts.
    let mut erased = lines.clone();
    let e = row(&erased, "events");
    for name in ["app_id", "app_name", "session_id"] {
        erased[e]["values"][column("events", name)] = json!("");
    }
    cases.push(("erased-identity", erased));
    // An example referencing a report its event's facts do not own.
    let mut unowned = lines.clone();
    let x = row(&unowned, "examples");
    unowned[x]["values"][column("examples", "report_id")] = json!("unowned-report");
    cases.push(("unowned-example-report", unowned));
    // An example its facts do not derive at all (forged excerpt).
    let mut forged_example = lines.clone();
    let x = row(&forged_example, "examples");
    forged_example[x]["values"][column("examples", "excerpts_json")] =
        json!("[\"forged excerpt\"]");
    cases.push(("forged-example-excerpt", forged_example));
    // A work descriptor whose scan ID is an array.
    let mut work = lines.clone();
    let footer = work.pop().unwrap();
    work.push(json!({"kind": "row", "table": "sync_work", "values": [
        "work-forged", "reports", json!({"identity": {"scan_id": []}}).to_string(),
        "pending", null, 0, 0, null, null, null, "{}"]}));
    work.push(footer);
    cases.push(("array-work-scan-id", work));
    // A forged detector projection (its facts say malicious).
    let mut detector = lines.clone();
    let o = row(&detector, "detector_observations");
    detector[o]["values"][column("detector_observations", "verdict")] = json!("benign");
    cases.push(("forged-detector-projection", detector));
    // A forged event-report projection.
    let mut reports = lines.clone();
    let r = row(&reports, "event_reports");
    reports[r]["values"][column("event_reports", "report_id")] = json!("report-9");
    cases.push(("forged-event-report", reports));
    for (name, rows) in cases {
        let refused = validates(d.path(), &format!("{name}.jsonl"), rows);
        assert!(refused.is_err(), "{name} must be refused");
    }
}

/// Finding 3 at import: a destination that pruned the fixture's excerpt refuses an
/// import of the unpruned export (the inline fact copy, not only the example ID), and
/// is unchanged.
#[test]
fn an_import_never_restores_a_pruned_excerpt_in_any_fact_copy() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("import-erasure");
    // The source holds a new variant of the event (another note: another fact ID) carrying the same excerpt the destination pruned, so only the erasure records
    // (not held-record immutability) can refuse it.
    let source = d.path().join("variant-source");
    seed_with(&source, "88888888-8888-4888-8888-888888888888", |v| {
        v["events"][0]["notes"] = json!(["synthetic variant note"]);
    });
    let file = d.path().join("variant.jsonl");
    export_file(
        &source,
        &cloud(),
        &file,
        Duration::from_secs(10),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    let lines: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let destination = d.path().join("destination");
    seed(&destination, "77777777-7777-4777-8777-777777777777");
    crate::repository::prune_examples(
        &destination,
        &cloud(),
        i64::MAX,
        &[],
        true,
        Duration::from_secs(10),
        &scm::Session::new(),
    )
    .unwrap();
    let before = std::fs::read(destination.join(cloud().cloud_id).join("archive.sqlite3")).unwrap();
    // Without its examples rows, only the fact copies carry the excerpt.
    let facts_only: Vec<Value> = lines
        .into_iter()
        .filter(|l| l["table"] != "examples" && l["table"] != "example_runs")
        .collect();
    let path = d.path().join("unpruned.jsonl");
    std::fs::write(&path, encode(facts_only)).unwrap();
    let refused = import_file(
        &destination,
        &cloud(),
        &path,
        Duration::from_secs(10),
        &scm::Session::new(),
        &SecretSet::default(),
    );
    assert!(refused.is_err());
    assert_eq!(
        std::fs::read(destination.join(cloud().cloud_id).join("archive.sqlite3")).unwrap(),
        before,
        "a refused import changes nothing"
    );
}

/// Finding 5: an import in bounded batches reports its progress after each committed
/// batch; a write between batches (here a concurrent prune) stops it with the batches
/// already committed reported as such; an interrupted import resumes from its
/// checkpoint to exactly the result of an uninterrupted one.
#[test]
fn bounded_imports_checkpoint_resume_and_stop_on_a_concurrent_write() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("bounded-import");
    let (_root, lines) = seeded(d.path());
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines)).unwrap();
    let session = scm::Session::new();
    let budget = Duration::from_secs(30);
    let run = |root: &Path,
               resume: Option<ImportCheckpoint>,
               stop_after: Option<u64>,
               expected: Option<i64>| {
        let mut seen: Vec<ImportCheckpoint> = vec![];
        let mut on_batch = |c: &ImportCheckpoint| {
            seen.push(c.clone());
            if stop_after == Some(c.batches) && !c.rebuilt {
                return Err(failure("interrupted after a durable batch"));
            }
            Ok(())
        };
        let r = import_file_controlled(
            root,
            &cloud(),
            &path,
            budget,
            &session,
            &SecretSet::default(),
            ImportControl {
                expected_generation: expected,
                batch_rows: 3,
                resume,
                operation_id: Some("test-import"),
                on_batch: &mut on_batch,
            },
        );
        (r, seen)
    };
    // Uninterrupted reference.
    let reference = d.path().join("reference");
    let (done, seen) = run(&reference, None, None, None);
    let done = done.unwrap();
    assert!(done["batches"].as_u64().unwrap() > 2, "{done}");
    assert!(seen.last().unwrap().rebuilt);
    let rows_of = |root: &Path| -> Vec<String> {
        let file = d.path().join(format!("{}.jsonl", uuid::Uuid::new_v4()));
        export_file(
            root,
            &cloud(),
            &file,
            budget,
            &session,
            &SecretSet::default(),
        )
        .unwrap();
        std::fs::read_to_string(&file)
            .unwrap()
            .lines()
            .filter(|l| l.contains("\"kind\":\"row\""))
            .map(str::to_owned)
            .collect()
    };
    // Interrupted after batch 2, then resumed from the durable checkpoint.
    let interrupted = d.path().join("interrupted");
    let (r, seen) = run(&interrupted, None, Some(2), None);
    let stopped = r.unwrap_err();
    let ImportOutcome::PartiallyCommitted(checkpoint) = *stopped.outcome else {
        panic!("{:?}", stopped.outcome)
    };
    assert_eq!(checkpoint.batches, 2);
    assert_eq!(seen.len(), 2);
    let (resumed, _) = run(&interrupted, Some(checkpoint.clone()), None, None);
    let resumed = resumed.unwrap();
    assert_eq!(resumed["inserted"], done["inserted"]);
    assert_eq!(
        rows_of(&interrupted),
        rows_of(&reference),
        "resume equals one import"
    );
    // Bound to a generation, a write between batches (a prune) stops the import.
    let bound = d.path().join("bound");
    seed(&bound, "66666666-6666-4666-8666-666666666666");
    let generation = crate::inspect::data_generation(&bound, &cloud(), &session).unwrap();
    let mut first = true;
    let mut on_batch = |_: &ImportCheckpoint| {
        if first {
            first = false;
            crate::repository::prune_examples(
                &bound,
                &cloud(),
                i64::MAX,
                &[],
                true,
                budget,
                &scm::Session::new(),
            )
            .unwrap();
        }
        Ok(())
    };
    let r = import_file_controlled(
        &bound,
        &cloud(),
        &path,
        budget,
        &session,
        &SecretSet::default(),
        ImportControl {
            expected_generation: Some(generation),
            batch_rows: 3,
            resume: None,
            operation_id: Some("bound-import"),
            on_batch: &mut on_batch,
        },
    );
    let stopped = r.unwrap_err();
    assert!(
        matches!(*stopped.outcome, ImportOutcome::PartiallyCommitted(ref c) if c.batches == 1),
        "{:?}",
        stopped.outcome
    );
    // A stale binding refuses before any change.
    let before = std::fs::read(bound.join(cloud().cloud_id).join("archive.sqlite3")).unwrap();
    let (r, _) = run(&bound, None, None, Some(generation));
    let r = r.unwrap_err();
    assert_eq!(*r.outcome, ImportOutcome::RefusedBeforeChange);
    assert_eq!(
        std::fs::read(bound.join(cloud().cloud_id).join("archive.sqlite3")).unwrap(),
        before
    );
}

/// Finding 2: the service's run provenance is accepted in its strict shapes only.
#[test]
fn service_run_provenance_is_accepted_only_in_its_strict_shapes() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("provenance-shapes");
    let (_root, lines) = seeded(d.path());
    let run = row(&lines, "collection_runs");
    let column = column("collection_runs", "provenance_json");
    let with = |extra: Value| {
        let mut l = lines.clone();
        let mut p: Value =
            serde_json::from_str(l[run]["values"][column].as_str().unwrap()).unwrap();
        for (k, v) in extra.as_object().unwrap() {
            p[k] = v.clone();
        }
        l[run]["values"][column] = json!(p.to_string());
        l
    };
    let digest = format!("{:x}", Sha256::digest(b"receipt"));
    assert!(
        validates(
            d.path(),
            "ok.jsonl",
            with(json!({
                "credential_revision": "11111111-1111-4111-8111-111111111111",
                "fact_counts": {"new": 1, "duplicate": 0},
                "first_accepted": [digest],
            }))
        )
        .is_ok()
    );
    for (name, bad) in [
        ("revision", json!({"credential_revision": "not-a-uuid"})),
        ("counts-shape", json!({"fact_counts": {"new": 1}})),
        (
            "counts-type",
            json!({"fact_counts": {"new": -1, "duplicate": 0}}),
        ),
        ("accepted", json!({"first_accepted": ["not-a-digest"]})),
        ("accepted-dup", json!({"first_accepted": [digest, digest]})),
    ] {
        assert!(
            validates(d.path(), &format!("{name}.jsonl"), with(bad)).is_err(),
            "{name}"
        );
    }
}

fn controlled(
    root: &Path,
    path: &Path,
    batch_rows: usize,
    resume: Option<ImportCheckpoint>,
    operation: Option<&str>,
    on_batch: &mut dyn FnMut(&ImportCheckpoint) -> Result<(), CliError>,
) -> Result<Value, ImportError> {
    import_file_controlled(
        root,
        &cloud(),
        path,
        Duration::from_secs(30),
        &scm::Session::new(),
        &SecretSet::default(),
        ImportControl {
            expected_generation: None,
            batch_rows,
            resume,
            operation_id: operation,
            on_batch,
        },
    )
}

/// M5 review 2 finding 3: a destination conflict a late batch would meet (here an
/// erasure record for the transfer's example, with one row per batch) refuses the whole
/// import before any destination change.
#[test]
fn a_late_destination_conflict_refuses_before_any_change() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("late-conflict");
    let (_root, lines) = seeded(d.path());
    let example = row(&lines, "examples");
    assert!(example > 1, "the conflict is not in the first batch");
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines.clone())).unwrap();
    let destination = d.path().join("destination");
    seed(&destination, "77777777-7777-4777-8777-777777777777");
    let file = destination.join(cloud().cloud_id).join("archive.sqlite3");
    let example_id = lines[example]["values"][0].as_str().unwrap().to_owned();
    Connection::open(&file)
        .unwrap()
        .execute("INSERT INTO pruned_examples VALUES (?1, 1)", [&example_id])
        .unwrap();
    let before = std::fs::read(&file).unwrap();
    let mut ignore = |_: &ImportCheckpoint| Ok(());
    let refused = controlled(&destination, &path, 1, None, None, &mut ignore).unwrap_err();
    assert_eq!(*refused.outcome, ImportOutcome::RefusedBeforeChange);
    assert_eq!(std::fs::read(&file).unwrap(), before, "nothing changed");
}

/// M5 review 2 finding 3: an archive another writer creates (with data) after the
/// import saw it missing is never treated as the empty archive the import validated
/// against: the first batch refuses.
#[test]
fn a_concurrently_created_archive_refuses_the_first_batch() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("creation-race");
    let (_root, lines) = seeded(d.path());
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines)).unwrap();
    let destination = d.path().join("destination");
    let staged = stage_and_validate(
        &cloud(),
        &path,
        Duration::from_secs(30),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    assert!(
        prevalidate(&destination, &cloud(), &staged, &scm::Session::new(), None)
            .unwrap()
            .is_none()
    );
    // Another writer creates the archive, with data, in between.
    seed(&destination, "55555555-5555-4555-8555-555555555555");
    let counts: Vec<u64> = staged
        .specs
        .iter()
        .map(|s| {
            staged
                .stage
                .query_row(
                    &format!("SELECT count(*) FROM {}", s.table.name()),
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap() as u64
        })
        .collect();
    let progress = ImportCheckpoint {
        transfer_sha256: staged.sha256.clone(),
        batches: 0,
        rows: 0,
        inserted: 0,
        generation: 0,
        rebuilt: false,
    };
    let refused = apply_batch(
        &destination,
        &cloud(),
        &staged,
        &counts,
        0,
        1000,
        Duration::from_secs(30),
        &scm::Session::new(),
        (true, 0),
        ("race", &progress),
    )
    .unwrap_err();
    assert!(refused.0.message().contains("changed since the preview"));
}

/// M5 review 2 finding 4: a batch that committed in the archive but whose control
/// checkpoint was never written (the process stopped in between) is reconciled from
/// the archive's own journal: the import completes exactly once, and a refusal after
/// committed batches reports them, never `refused_before_change`.
#[test]
fn committed_but_unchecked_batches_are_reconciled_from_the_archive_journal() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("journal-reconcile");
    let (_root, lines) = seeded(d.path());
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines.clone())).unwrap();
    let mut ignore = |_: &ImportCheckpoint| Ok(());
    let reference = d.path().join("reference");
    let done = controlled(&reference, &path, 3, None, None, &mut ignore).unwrap();
    let destination = d.path().join("destination");
    // The control store records batch 1; batch 2 commits in the archive, then the
    // process stops before its checkpoint is written.
    let mut recorded: Option<ImportCheckpoint> = None;
    let mut crash = |c: &ImportCheckpoint| {
        if c.batches == 2 {
            return Err(failure("process stopped before the checkpoint"));
        }
        recorded = Some(c.clone());
        Ok(())
    };
    controlled(&destination, &path, 3, None, Some("job-1"), &mut crash).unwrap_err();
    let stale = recorded.clone().unwrap();
    assert_eq!(stale.batches, 1, "the control store only saw batch 1");
    // A refusal of the restarted job (here: its upload now fails validation) reports
    // the committed batches the archive journal holds.
    let corrupt = d.path().join("corrupt.jsonl");
    std::fs::write(&corrupt, b"not a transfer\n").unwrap();
    let refused = import_file_controlled(
        &destination,
        &cloud(),
        &corrupt,
        Duration::from_secs(30),
        &scm::Session::new(),
        &SecretSet::default(),
        ImportControl {
            expected_generation: None,
            batch_rows: 3,
            resume: Some(stale.clone()),
            operation_id: Some("job-1"),
            on_batch: &mut ignore,
        },
    )
    .unwrap_err();
    assert!(
        matches!(*refused.outcome, ImportOutcome::PartiallyCommitted(ref c) if c.batches == 2),
        "{:?}",
        refused.outcome
    );
    // The restarted job resumes from its stale control checkpoint and completes exactly
    // once (the archive's journal decides where).
    let completed = controlled(
        &destination,
        &path,
        3,
        Some(stale),
        Some("job-1"),
        &mut ignore,
    )
    .unwrap();
    assert_eq!(completed["inserted"], done["inserted"], "{completed}");
    assert_eq!(completed["batches"], done["batches"]);
    // A restart that lost even the control checkpoint reconciles too: nothing more.
    let again = controlled(&destination, &path, 3, None, Some("job-1"), &mut ignore).unwrap();
    assert_eq!(again["inserted"], done["inserted"]);
}

fn peak_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("VmHWM:"))
        .and_then(|v| v.trim().trim_end_matches(" kB").trim().parse().ok())
        .unwrap()
}

/// An archive of `n` distinct events (each with its fact, excerpt, example and own
/// report), exported to `dir/large.jsonl`.
fn large_export(dir: &Path, n: usize) -> std::path::PathBuf {
    let root = dir.join("history");
    let mut written = 0;
    while written < n {
        let chunk = (n - written).min(2_000);
        let start = written;
        seed_with(&root, &uuid::Uuid::new_v4().to_string(), |v| {
            let template = v["events"][0].clone();
            v["budgets"]["max_events"] = json!(chunk);
            v["sampling"]["selected_report_ids"] = json!([]);
            v["events"] = Value::Array(
                (start..start + chunk)
                    .map(|i| {
                        let mut e = template.clone();
                        e["identity"]["scan_id"] =
                            json!(format!("00000000-0000-4000-8000-{i:012x}"));
                        e["identity"]["session_id"] = json!(format!("session-{i}"));
                        // Each event owns its own report.
                        let report = format!("report-large-{i}");
                        e["report_ids"] = json!([report]);
                        if let Some(ds) = e["detections"].as_array_mut() {
                            for d in ds {
                                d["report_id"] = json!(report);
                            }
                        }
                        e
                    })
                    .collect(),
            );
        });
        written += chunk;
    }
    let file = dir.join("large.jsonl");
    export_file(
        &root,
        &cloud(),
        &file,
        Duration::from_secs(3600),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    file
}

/// M5 review 5 minor (opt-in like the validation test, `AIRS_LARGE_TRANSFER_EVENTS=<n>`,
/// run alone with `--ignored --test-threads=1`): a complete import of that transfer into
/// a fresh archive, staging, validation, every batch and the paged final rebuild, stays
/// under the same fixed memory ceiling.
#[test]
#[ignore]
fn large_transfer_import_stays_under_a_memory_ceiling() {
    let n: usize = std::env::var("AIRS_LARGE_TRANSFER_EVENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    let d = crate::test_support::TestDir::new("large-import");
    let file = large_export(d.path(), n);
    let bytes = std::fs::metadata(&file).unwrap().len();
    let destination = d.path().join("destination");
    std::fs::write("/proc/self/clear_refs", "5").unwrap();
    let base = peak_rss_kib();
    let result = import_file(
        &destination,
        &cloud(),
        &file,
        Duration::from_secs(3600),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    let grown = peak_rss_kib().saturating_sub(base);
    eprintln!("large import: {n} events, {bytes} bytes, {result}; peak RSS growth {grown} KiB");
    assert!(
        grown < 128 * 1024,
        "peak RSS grew {grown} KiB importing {bytes} bytes"
    );
}

/// M5 review 2 finding 7 (opt-in, `AIRS_LARGE_TRANSFER_EVENTS=<n>`, run alone with
/// `--ignored --test-threads=1`): validating a transfer of `n` distinct events (each with
/// its fact, excerpt and example) stays under a fixed memory ceiling, because staging and
/// every semantic comparison are disk-backed SQL, not whole-transfer Rust structures.
/// The peak RSS is reset (`/proc/self/clear_refs`) just before validation and read after.
#[test]
#[ignore]
fn large_transfer_validation_stays_under_a_memory_ceiling() {
    let n: usize = std::env::var("AIRS_LARGE_TRANSFER_EVENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    let d = crate::test_support::TestDir::new("large-transfer");
    let file = large_export(d.path(), n);
    let bytes = std::fs::metadata(&file).unwrap().len();
    std::fs::write("/proc/self/clear_refs", "5").unwrap();
    let base = peak_rss_kib();
    let rows = validate_file(
        &cloud(),
        &file,
        Duration::from_secs(3600),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    let grown = peak_rss_kib().saturating_sub(base);
    eprintln!(
        "large transfer: {n} events, {rows} rows, {bytes} bytes; peak RSS growth {grown} KiB"
    );
    assert!(
        grown < 128 * 1024,
        "peak RSS grew {grown} KiB validating {bytes} bytes"
    );
}

/// The generation chain on its own: an unrelated write between two batches (no
/// conflict, no erasure) still stops the import after the committed batch.
#[test]
fn an_unrelated_write_between_batches_stops_the_import() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("chain-write");
    let (_root, lines) = seeded(d.path());
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines)).unwrap();
    let destination = d.path().join("destination");
    seed(&destination, "44444444-4444-4444-8444-444444444444");
    let file = destination.join(cloud().cloud_id).join("archive.sqlite3");
    let mut first = true;
    let mut interleave = |_: &ImportCheckpoint| {
        if first {
            first = false;
            Connection::open(&file)
                .unwrap()
                .execute(
                    "UPDATE archive_meta SET bootstrap_ms=12345 WHERE singleton=1",
                    [],
                )
                .unwrap();
        }
        Ok(())
    };
    let stopped =
        controlled(&destination, &path, 3, None, Some("chain"), &mut interleave).unwrap_err();
    assert!(
        matches!(*stopped.outcome, ImportOutcome::PartiallyCommitted(ref c) if c.batches == 1),
        "{:?}",
        stopped.outcome
    );
}

/// Child half of the process-death probe (runs only when the parent sets
/// `AIRS_IMPORT_KILL_ROOT`): a real controlled import whose process exits for real right
/// after batch 2 commits in the archive and before its control checkpoint is persisted.
/// Batch 1's checkpoint is persisted to a file first, as the control store would.
#[test]
fn import_kill_child() {
    let _isolation = crate::test_isolation();
    let Some(root) = std::env::var_os("AIRS_IMPORT_KILL_ROOT") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    let path = root.join("transfer.jsonl");
    let destination = root.join("destination");
    let saved = root.join("control-checkpoint.json");
    let mut on_batch = |c: &ImportCheckpoint| {
        if c.batches == 2 {
            // The archive transaction for batch 2 has committed; the control
            // checkpoint has not been written. The process dies here.
            std::process::exit(86);
        }
        std::fs::write(&saved, serde_json::to_vec(c).unwrap()).unwrap();
        Ok(())
    };
    let _ = controlled(
        &destination,
        &path,
        3,
        None,
        Some("job-kill"),
        &mut on_batch,
    );
    unreachable!("the import must die inside the commit-to-checkpoint window");
}

/// M5 review 2 finding 4, with an actual process death: the child process exits between
/// an archive batch commit and its control checkpoint. A restart from the stale control
/// checkpoint, and one with no control checkpoint at all, both reconcile from the
/// archive journal and complete exactly once (same inserted rows as an uninterrupted
/// import).
#[test]
fn a_real_process_death_between_commit_and_checkpoint_is_reconciled() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("import-kill");
    let (_root, lines) = seeded(d.path());
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines.clone())).unwrap();
    let mut ignore = |_: &ImportCheckpoint| Ok(());
    let done = controlled(
        &d.path().join("reference"),
        &path,
        3,
        None,
        None,
        &mut ignore,
    )
    .unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "transfer::review_tests::import_kill_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("AIRS_IMPORT_KILL_ROOT", d.path().canonicalize().unwrap())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(86), "the child died in the window");
    let stale: ImportCheckpoint =
        serde_json::from_slice(&std::fs::read(d.path().join("control-checkpoint.json")).unwrap())
            .unwrap();
    assert_eq!(stale.batches, 1, "the control store only saw batch 1");
    let destination = d.path().join("destination");
    let resumed = controlled(
        &destination,
        &path,
        3,
        Some(stale),
        Some("job-kill"),
        &mut ignore,
    )
    .unwrap();
    assert_eq!(resumed["inserted"], done["inserted"], "{resumed}");
    assert_eq!(resumed["batches"], done["batches"]);
    let again = controlled(&destination, &path, 3, None, Some("job-kill"), &mut ignore).unwrap();
    assert_eq!(again["inserted"], done["inserted"], "{again}");
}

/// M5 review 3 finding 1: a forged promoted receipt with no supporting facts is
/// refused whatever its ID, the empty string included (the keyset cursor used to start
/// at `""` and skip it), and a receipt whose ID is not its origin key's digest refuses.
#[test]
fn a_forged_receipt_is_refused_whatever_its_id() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("forged-receipt-id");
    let (_root, original) = seeded(d.path());
    validates(d.path(), "control.jsonl", original.clone()).unwrap();
    let event_id = original[row(&original, "events")]["values"][0].clone();
    let width = {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(database::SCHEMA).unwrap();
        let t = codec::Table::ALL
            .into_iter()
            .find(|t| t.name() == "receipts")
            .unwrap();
        rows::Spec::load(&c, t).unwrap().columns.len()
    };
    for id in ["", "nonempty"] {
        let mut lines = original.clone();
        let footer = lines.pop().unwrap();
        let mut values = vec![Value::Null; width];
        values[column("receipts", "receipt_id")] = json!(id);
        values[column("receipts", "origin_key")] = json!("forged-origin");
        values[column("receipts", "first_seen_ms")] = json!(1);
        values[column("receipts", "last_seen_ms")] = json!(1);
        values[column("receipts", "binding_json")] = json!("{\"identity_withheld\":false}");
        values[column("receipts", "state")] = json!("promoted");
        values[column("receipts", "promoted_event_id")] = event_id.clone();
        lines.push(json!({"kind":"row","table":"receipts","values":values}));
        lines.push(footer);
        assert!(
            validates(d.path(), &format!("forged-{}.jsonl", id.len()), lines).is_err(),
            "forged receipt with id {id:?} must refuse"
        );
    }
}

/// An initialized archive turned into the exact accepted-M4 (version 1) schema.
fn as_version_1(root: &Path) -> std::path::PathBuf {
    let file = root.join(cloud().cloud_id).join("archive.sqlite3");
    let conn = Connection::open(&file).unwrap();
    conn.execute_batch(
        "DROP TABLE pruned_examples; DROP TABLE archive_generation; DROP TABLE import_operations; PRAGMA user_version = 1;",
    )
    .unwrap();
    let expected = Connection::open_in_memory().unwrap();
    expected.execute_batch(database::SCHEMA_V1).unwrap();
    file
}

/// M5 review 3 finding 3: importing into an accepted-M4 (version 1) destination. A
/// transfer that conflicts with what the destination holds is refused before any
/// change: the archive is byte for byte unchanged and still version 1 (validation never
/// upgrades it). A transfer that does not conflict imports: the first batch's writer
/// upgrades the archive (after its verified pre-upgrade copy) and the import's binding
/// to the version-1 generation still holds across that upgrade.
#[test]
fn an_import_into_an_m4_archive_refuses_unchanged_or_upgrades_then_imports() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("import-into-m4");
    let (_root, lines) = seeded(d.path());
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines.clone())).unwrap();
    let mut ignore = |_: &ImportCheckpoint| Ok(());
    // A conflicting destination: it holds the transfer's collection run with other
    // content.
    let conflicting = d.path().join("conflicting");
    seed(&conflicting, "77777777-7777-4777-8777-777777777777");
    let file = as_version_1(&conflicting);
    let run = lines[row(&lines, "collection_runs")]["values"][0]
        .as_str()
        .unwrap()
        .to_owned();
    Connection::open(&file)
        .unwrap()
        .execute(
            "INSERT INTO collection_runs VALUES (?1,'import',9,9,NULL,NULL,'complete','{}')",
            [&run],
        )
        .unwrap();
    let before = std::fs::read(&file).unwrap();
    let refused = controlled(&conflicting, &path, 3, None, None, &mut ignore).unwrap_err();
    assert_eq!(*refused.outcome, ImportOutcome::RefusedBeforeChange);
    assert_eq!(std::fs::read(&file).unwrap(), before, "truly unchanged");
    let version: i64 = Connection::open(&file)
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 1, "never upgraded by a refused import");
    // A non-conflicting M4 destination, with a binding taken before the import (as a
    // preview does): the import upgrades and completes under that binding.
    let clean = d.path().join("clean");
    seed(&clean, "66666666-6666-4666-8666-666666666666");
    let file = as_version_1(&clean);
    let generation = {
        let r = database::Reader::open(&clean, &cloud(), &scm::Session::new())
            .unwrap()
            .unwrap();
        r.generation().unwrap()
    };
    let done = import_file_controlled(
        &clean,
        &cloud(),
        &path,
        Duration::from_secs(30),
        &scm::Session::new(),
        &SecretSet::default(),
        ImportControl {
            expected_generation: Some(generation),
            batch_rows: 3,
            resume: None,
            operation_id: Some("job-m4"),
            on_batch: &mut ignore,
        },
    )
    .unwrap();
    assert!(done["inserted"].as_u64().unwrap() > 0, "{done}");
    let version: i64 = Connection::open(&file)
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 2);
}

/// M5 review 3 finding 5: an interrupted merge never leaves a stale projection. The
/// destination holds the event with one fact; the transfer adds a conflicting fact for
/// the same event (another verdict and action). The import stops after every batch has
/// committed but before its final publication: the event's projection already reflects
/// both facts (each batch rebuilt what it touched), exactly as a completed import gives,
/// and the archive reports the import as unfinished.
#[test]
fn an_interrupted_merge_keeps_projections_consistent_and_reports_the_import_unfinished() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("interrupted-merge");
    let source = d.path().join("conflicting-source");
    seed_with(&source, "88888888-8888-4888-8888-888888888888", |v| {
        v["events"][0]["verdict"] = json!("benign");
        v["events"][0]["action"] = json!("block");
        v["events"][0]["notes"] = json!(["conflicting variant"]);
    });
    let file = d.path().join("conflicting.jsonl");
    export_file(
        &source,
        &cloud(),
        &file,
        Duration::from_secs(10),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    let rows = std::fs::read_to_string(&file).unwrap().lines().count() as u64 - 2;
    let projection = |root: &Path| -> Vec<Value> {
        let file = root.join(cloud().cloud_id).join("archive.sqlite3");
        let c = Connection::open(&file).unwrap();
        let mut st = c
            .prepare("SELECT event_id, verdict, action FROM events ORDER BY event_id")
            .unwrap();
        st.query_map([], |r| {
            Ok(json!([
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?
            ]))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    };
    let mut ignore = |_: &ImportCheckpoint| Ok(());
    let reference = d.path().join("reference");
    seed(&reference, "77777777-7777-4777-8777-777777777777");
    controlled(&reference, &file, 1, None, Some("job-ref"), &mut ignore).unwrap();
    let interrupted = d.path().join("interrupted");
    seed(&interrupted, "77777777-7777-4777-8777-777777777777");
    let mut stop = |c: &ImportCheckpoint| {
        if c.batches == rows && !c.rebuilt {
            return Err(failure("stopped before publication"));
        }
        Ok(())
    };
    controlled(&interrupted, &file, 1, None, Some("job-int"), &mut stop).unwrap_err();
    assert_eq!(
        projection(&interrupted),
        projection(&reference),
        "every committed batch left the projection consistent"
    );
    assert_eq!(
        crate::inspect::unfinished_imports(&interrupted, &cloud(), &scm::Session::new()).unwrap(),
        1
    );
    assert_eq!(
        crate::inspect::unfinished_imports(&reference, &cloud(), &scm::Session::new()).unwrap(),
        0
    );
}

/// M5 review 4 finding 1: a canceled (here: stopped) import followed by a fresh job that
/// imports the same transfer in full leaves no unfinished import, so coverage reads are
/// usable again; the stopped import's journal row is kept (for audit) and marked
/// finished. A different transfer's unfinished import is not superseded.
#[test]
fn a_replacement_import_of_the_same_transfer_retires_the_unfinished_one() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("import-replace");
    let (_root, lines) = seeded(d.path());
    let path = d.path().join("transfer.jsonl");
    std::fs::write(&path, encode(lines.clone())).unwrap();
    let destination = d.path().join("destination");
    let mut stop = |c: &ImportCheckpoint| {
        if c.batches == 1 {
            return Err(failure("the job was canceled"));
        }
        Ok(())
    };
    controlled(&destination, &path, 3, None, Some("job-stopped"), &mut stop).unwrap_err();
    let session = scm::Session::new();
    assert_eq!(
        crate::inspect::unfinished_imports(&destination, &cloud(), &session).unwrap(),
        1,
        "the stopped import is unfinished"
    );
    assert!(matches!(
        crate::inspect::coverage_if_published(&destination, &cloud(), 0, i64::MAX, &[], &session)
            .unwrap(),
        crate::inspect::PublishedCoverage::Unfinished
    ));
    let mut ignore = |_: &ImportCheckpoint| Ok(());
    controlled(
        &destination,
        &path,
        3,
        None,
        Some("job-replacement"),
        &mut ignore,
    )
    .unwrap();
    assert_eq!(
        crate::inspect::unfinished_imports(&destination, &cloud(), &session).unwrap(),
        0,
        "the replacement publication retires it"
    );
    assert!(matches!(
        crate::inspect::coverage_if_published(&destination, &cloud(), 0, i64::MAX, &[], &session)
            .unwrap(),
        crate::inspect::PublishedCoverage::Groups(_)
    ));
    let rows: i64 =
        rusqlite::Connection::open(destination.join(cloud().cloud_id).join("archive.sqlite3"))
            .unwrap()
            .query_row("SELECT count(*) FROM import_operations", [], |r| r.get(0))
            .unwrap();
    assert_eq!(rows, 2, "both journal rows are kept");
}

/// M5 review 4 minor (mutation A2): a supported receipt whose `origin_key` is changed
/// still passes projection validation (its facts and projection are untouched); only the
/// receipt identity check (ID = digest of the origin key) refuses it.
#[test]
fn a_receipt_with_a_changed_origin_key_is_refused_by_its_identity() {
    let _isolation = crate::test_isolation();
    let d = crate::test_support::TestDir::new("receipt-origin");
    let root = d.path().join("history");
    seed(&root, "88888888-8888-4888-8888-888888888888");
    let endpoint = "https://example.invalid/scan".to_owned();
    let received = crate::repository::Received {
        receipt_key: "receipt-origin-0001".into(),
        batch_report_id: None,
        event: airs_runtime_core::signal::evidence::Event {
            identity: airs_runtime_core::signal::evidence::Identity {
                app_id: "app-1".into(),
                app_name: String::new(),
                session_id: "session-1".into(),
                scan_id: "00000000-0000-4000-8000-0000000000c1".into(),
                sub_request: Some(0),
            },
            timestamp_ms: None,
            profile: None,
            verdict: "unknown".into(),
            action: "allow".into(),
            report_ids: vec![],
            detections: vec![],
            detector_hints: vec![],
            sources: vec!["client_asserted".into()],
            notes: vec![],
        },
        source: "client_asserted",
        endpoint: endpoint.clone(),
        reference: Some(airs_runtime_core::tenant::scanners::ScannerReference {
            name: "archive".into(),
            revision: "11111111-1111-4111-8111-111111111111".into(),
            application_id: "app-1".into(),
            endpoint,
        }),
    };
    crate::repository::ingest_client_receipts(
        &root,
        &cloud(),
        "33333333-3333-4333-8333-333333333333",
        &[received],
        &SecretSet::default(),
        &airs_runtime_core::signal::privacy::PayloadSpans::default(),
    )
    .unwrap();
    let file = d.path().join("export.jsonl");
    export_file(
        &root,
        &cloud(),
        &file,
        Duration::from_secs(10),
        &scm::Session::new(),
        &SecretSet::default(),
    )
    .unwrap();
    let original: Vec<Value> = std::fs::read_to_string(&file)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    validates(d.path(), "control.jsonl", original.clone()).unwrap();
    let index = original
        .iter()
        .position(|l| l["kind"] == "row" && l["table"] == "receipts")
        .expect("the seeded transfer holds a receipt");
    let mut lines = original.clone();
    lines[index]["values"][column("receipts", "origin_key")] = json!("a-different-origin-key");
    assert!(
        validates(d.path(), "changed-origin.jsonl", lines).is_err(),
        "a receipt whose ID is not its origin key's digest refuses"
    );
}
