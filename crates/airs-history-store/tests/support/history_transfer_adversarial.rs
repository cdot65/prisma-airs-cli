//! Independent B4 acceptance probes. All data and paths are synthetic and local.
use airs_test_harness::{Cli, FixtureServer, Run, RunOptions, Sandbox, run};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

const EXAMPLE: &str = "Synthetic archive excerpt deliberately retained for the prune check.";

fn cli() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs-history-driver"))
}

struct Context {
    sandbox: Sandbox,
    root: PathBuf,
    tsg: String,
    environment: String,
    cloud: String,
}

impl Context {
    fn new(server: &FixtureServer, tsg: &str) -> Self {
        let sandbox = Sandbox::new();
        sandbox.single_tenant(server, "prod", tsg);
        let root = sandbox.tmp().join("archive");
        let status = run(
            &cli(),
            &sandbox,
            &[
                "runtime",
                "history",
                "status",
                "--history-dir",
                root.to_str().unwrap(),
                "--output",
                "json",
            ],
            &RunOptions::new(),
        );
        status.assert_success();
        let binding: Value = serde_json::from_str(&status.stdout).unwrap();
        assert_eq!(binding["archive_present"], false);
        assert!(!root.exists());
        Self {
            sandbox,
            root,
            tsg: tsg.to_owned(),
            environment: binding["environment_id"].as_str().unwrap().to_owned(),
            cloud: binding["cloud_id"].as_str().unwrap().to_owned(),
        }
    }

    fn command(&self, leaf: &str, flags: &[&str]) -> Run {
        let mut args = vec![
            "runtime",
            "history",
            leaf,
            "--history-dir",
            self.root.to_str().unwrap(),
            "--output",
            "json",
        ];
        args.extend_from_slice(flags);
        run(&cli(), &self.sandbox, &args, &RunOptions::new())
    }

    fn input(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.sandbox.tmp().join(name);
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    }

    fn seed(&self, name: &str, mut evidence: Value) {
        evidence.as_object_mut().unwrap().remove("digest");
        evidence["digest"] = json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&evidence).unwrap())
        ));
        let input = self.input(name, &serde_json::to_vec_pretty(&evidence).unwrap());
        self.command(
            "import-bundle",
            &[
                "--file",
                input.to_str().unwrap(),
                "--expect-tsg",
                &self.tsg,
                "--expect-environment",
                &self.environment,
            ],
        )
        .assert_success();
    }

    fn export(&self, file: &Path) -> Run {
        self.command("export", &["--file", file.to_str().unwrap()])
    }

    fn import(&self, file: &Path) -> Run {
        self.command(
            "import",
            &[
                "--file",
                file.to_str().unwrap(),
                "--expect-tsg",
                &self.tsg,
                "--expect-environment",
                &self.environment,
            ],
        )
    }

    fn database(&self) -> PathBuf {
        self.root.join(&self.cloud).join("archive.sqlite3")
    }

    fn connection(&self) -> Connection {
        Connection::open_with_flags(self.database(), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
    }

    fn fact_counts(&self) -> Vec<i64> {
        let connection = self.connection();
        [
            "events",
            "event_facts",
            "receipts",
            "receipt_facts",
            "examples",
        ]
        .iter()
        .map(|table| {
            connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap()
        })
        .collect()
    }
}

fn evidence() -> Value {
    let mut evidence: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/signal/evidence.json"
    ))
    .unwrap();
    evidence["events"][0]["detections"][0]["snippets"][0]["excerpts"] = json!([EXAMPLE]);
    evidence
}

fn contains(bytes: &[u8], needle: &str) -> bool {
    bytes
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn logical_transfer_lines(bytes: &[u8]) -> Vec<Value> {
    assert!(bytes.ends_with(b"\n"), "export must end with a newline");
    let lines: Vec<Value> = std::str::from_utf8(bytes)
        .unwrap()
        .split_terminator('\n')
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.first().unwrap()["kind"], "header");
    assert_eq!(lines.last().unwrap()["kind"], "footer");
    assert!(
        lines[1..lines.len() - 1]
            .iter()
            .all(|line| line["kind"] == "row")
    );
    lines
}

/// Keep framing, per-table counts and the exact-byte checksum valid so a refusal
/// must come from logical validation rather than a stale integrity trailer.
fn encode_logical_transfer(mut lines: Vec<Value>) -> Vec<u8> {
    let mut footer = lines.pop().unwrap();
    assert_eq!(footer["kind"], "footer");
    let mut counts = footer["counts"].as_object().unwrap().clone();
    for count in counts.values_mut() {
        *count = json!(0);
    }
    let mut bytes = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if index == 0 {
            assert_eq!(line["kind"], "header");
        } else {
            assert_eq!(line["kind"], "row");
            let table = line["table"].as_str().unwrap();
            let count = counts
                .get_mut(table)
                .expect("exported table must have a footer count");
            *count = json!(count.as_u64().unwrap().checked_add(1).unwrap());
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

fn logical_row(lines: &[Value], table: &str) -> usize {
    lines
        .iter()
        .position(|line| line["kind"] == "row" && line["table"] == table)
        .unwrap_or_else(|| panic!("exported fixture has no {table} row"))
}

fn column(context: &Context, table: &str, name: &str) -> usize {
    // Identifiers are fixed test-side SQL, never strings from the transfer.
    let sql = match table {
        "events" => "PRAGMA table_info(events)",
        "event_facts" => "PRAGMA table_info(event_facts)",
        "fact_runs" => "PRAGMA table_info(fact_runs)",
        _ => panic!("unsupported test table"),
    };
    let connection = context.connection();
    let mut query = connection.prepare(sql).unwrap();
    let columns = query
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    columns
        .iter()
        .position(|column| column == name)
        .unwrap_or_else(|| panic!("fixture schema has no {table}.{name}"))
}

#[test]
fn valid_digest_logical_corruption_refuses_before_any_destination_mutation() {
    let server = FixtureServer::start();
    let source = Context::new(&server, "100");
    source.seed("semantic-source-evidence.json", evidence());
    let source_before = source.sandbox.snapshot_with(&[&source.root]);
    let exported = source.sandbox.tmp().join("semantic-source.jsonl");
    source.export(&exported).assert_success();
    source.sandbox.assert_unchanged(&source_before);
    let lines = logical_transfer_lines(&fs::read(&exported).unwrap());

    // Prove our re-encoding/checksum helper produces an importable transfer
    // before using it to manufacture semantically invalid but intact files.
    let control = Context::new(&server, "100");
    let input = control.input(
        "valid-reencoded.jsonl",
        &encode_logical_transfer(lines.clone()),
    );
    control.import(&input).assert_success();
    assert_eq!(control.fact_counts(), source.fact_counts());

    let events = logical_row(&lines, "events");
    let event_id = column(&source, "events", "event_id");
    let verdict = column(&source, "events", "verdict");
    let fact_event_id = column(&source, "event_facts", "event_id");
    let fact_id = column(&source, "event_facts", "fact_id");
    let fields = column(&source, "event_facts", "fields_json");
    let linked_fact = column(&source, "fact_runs", "fact_id");
    let selected_event = lines[events]["values"][event_id].clone();
    assert_eq!(lines[events]["values"][verdict], "malicious");
    assert!(lines.iter().any(|line| {
        if line["table"] != "event_facts" || line["values"][fact_event_id] != selected_event {
            return false;
        }
        let fact: Value = serde_json::from_str(line["values"][fields].as_str().unwrap()).unwrap();
        fact["event"]["verdict"] == "malicious"
    }));

    let mut duplicate = lines.clone();
    let run = logical_row(&duplicate, "collection_runs");
    duplicate.insert(run + 1, duplicate[run].clone());

    let mut cardinality = lines.clone();
    cardinality[events]["values"]
        .as_array_mut()
        .unwrap()
        .pop()
        .unwrap();

    let mut orphan = lines.clone();
    let orphan_id = json!(format!(
        "{:x}",
        Sha256::digest(b"synthetic-fact-absent-from-transfer")
    ));
    assert!(
        lines
            .iter()
            .filter(|line| line["table"] == "event_facts")
            .all(|line| line["values"][fact_id] != orphan_id)
    );
    let link = logical_row(&orphan, "fact_runs");
    orphan[link]["values"][linked_fact] = orphan_id;

    let mut projection = lines.clone();
    projection[events]["values"][verdict] = json!("benign");
    let original_facts: Vec<_> = lines
        .iter()
        .filter(|line| line["table"] == "event_facts")
        .collect();
    let unchanged_facts: Vec<_> = projection
        .iter()
        .filter(|line| line["table"] == "event_facts")
        .collect();
    assert_eq!(unchanged_facts, original_facts);

    for (name, rows) in [
        ("duplicate-primary-key", duplicate),
        ("wrong-column-cardinality", cardinality),
        ("orphan-fact-run", orphan),
        ("forged-benign-projection", projection),
    ] {
        let bytes = encode_logical_transfer(rows);
        for existing in [false, true] {
            let destination = Context::new(&server, "100");
            if existing {
                let mut retained = evidence();
                retained["collection_id"] = json!("88888888-8888-4888-8888-888888888888");
                destination.seed("already-retained.json", retained);
            }
            let input = destination.input(&format!("{name}.jsonl"), &bytes);
            let before = destination
                .sandbox
                .snapshot_with(&[&destination.root, &input]);
            destination.import(&input).assert_code(1);
            if !existing {
                assert!(
                    !destination.root.exists(),
                    "{name} initialized a missing archive"
                );
            }
            destination.sandbox.assert_unchanged(&before);
        }
    }
    source.sandbox.assert_unchanged(&source_before);
    assert_eq!(server.request_count(), 0);
}

#[test]
fn logical_transfer_is_private_no_clobber_read_only_at_source_and_idempotent_at_destination() {
    let server = FixtureServer::start();
    let source = Context::new(&server, "100");
    source.seed("source-evidence.json", evidence());
    let counts = source.fact_counts();
    assert_eq!(counts[0], 1);
    let source_before = source.sandbox.snapshot_with(&[&source.root]);
    let exported = source.sandbox.tmp().join("transfer.jsonl");
    source.export(&exported).assert_success();
    source.sandbox.assert_unchanged(&source_before);
    let bytes = fs::read(&exported).unwrap();
    assert!(contains(&bytes, EXAMPLE));
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    assert_eq!(
        fs::metadata(&exported).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let before_collision = source.sandbox.snapshot_with(&[&source.root, &exported]);
    source.export(&exported).assert_code(1);
    source.sandbox.assert_unchanged(&before_collision);

    let destination = Context::new(&server, "100");
    // Transfer between machines is represented by a copy into the receiving sandbox.
    let input = destination.input("received.jsonl", &bytes);
    let input_before = destination.sandbox.snapshot_with(&[&input]);
    destination.import(&input).assert_success();
    destination.sandbox.assert_unchanged(&input_before);
    assert_eq!(destination.fact_counts(), counts);
    destination.import(&input).assert_success();
    destination.sandbox.assert_unchanged(&input_before);
    assert_eq!(destination.fact_counts(), counts);
    assert_eq!(server.request_count(), 0);
}

#[test]
fn wrong_cloud_and_invalid_transfers_refuse_before_destination_mutation() {
    let server = FixtureServer::start();
    let source = Context::new(&server, "100");
    source.seed("source-evidence.json", evidence());
    let exported = source.sandbox.tmp().join("transfer.jsonl");
    source.export(&exported).assert_success();
    let bytes = fs::read(&exported).unwrap();
    assert!(bytes.len() > 32);

    // Both expectations match this selected context, but not the source header's TSG.
    let wrong_cloud = Context::new(&server, "200");
    wrong_cloud.seed("existing-evidence.json", evidence());
    let input = wrong_cloud.input("wrong-cloud.jsonl", &bytes);
    let before = wrong_cloud
        .sandbox
        .snapshot_with(&[&wrong_cloud.root, &input]);
    wrong_cloud.import(&input).assert_code(1);
    wrong_cloud.sandbox.assert_unchanged(&before);

    let mut damaged = bytes.clone();
    let example = damaged
        .windows(EXAMPLE.len())
        .position(|window| window == EXAMPLE.as_bytes())
        .expect("exported fixture must retain its synthetic example");
    // Keep valid UTF-8/JSON while changing actual record content without its digest.
    damaged[example] = b'X';
    let truncated = bytes[..bytes.len() / 2].to_vec();
    let mut trailing = bytes;
    trailing.extend_from_slice(b"\n{}\n");
    let physical = fs::read(source.database()).unwrap();
    for (name, bytes) in [
        ("damaged.jsonl", damaged),
        ("truncated.jsonl", truncated),
        ("trailing.jsonl", trailing),
        ("physical.sqlite3", physical),
    ] {
        let destination = Context::new(&server, "100");
        let input = destination.input(name, &bytes);
        let before = destination
            .sandbox
            .snapshot_with(&[&destination.root, &input]);
        destination.import(&input).assert_code(1);
        assert!(!destination.root.exists(), "{name} initialized an archive");
        destination.sandbox.assert_unchanged(&before);
    }
    assert_eq!(server.request_count(), 0);
}

#[test]
fn examples_only_prune_previews_without_writes_and_erases_all_live_copies() {
    let server = FixtureServer::start();
    let source = Context::new(&server, "100");
    source.seed("full-evidence.json", evidence());
    let mut receipt = evidence();
    receipt["collection_id"] = json!("33333333-3333-4333-8333-333333333333");
    receipt["events"][0]["identity"]["sub_request"] = Value::Null;
    source.seed("receipt-evidence.json", receipt);

    let connection = source.connection();
    for table in ["event_facts", "receipt_facts"] {
        let copies: i64 = connection
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE instr(fields_json,?1)>0"),
                [EXAMPLE],
                |row| row.get(0),
            )
            .unwrap();
        assert!(copies > 0, "fixture did not retain an example in {table}");
    }
    let promoted: i64 = connection
        .query_row(
            "SELECT count(*) FROM receipts WHERE state='promoted'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        promoted, 1,
        "fixture must have a known dated owner for the receipt"
    );
    drop(connection);
    assert!(contains(&fs::read(source.database()).unwrap(), EXAMPLE));
    let old_export = source.sandbox.tmp().join("before-prune.jsonl");
    source.export(&old_export).assert_success();
    let original_export = fs::read(&old_export).unwrap();
    assert!(contains(&original_export, EXAMPLE));

    let flags = [
        "--before",
        "2026-11-02T00:00:00Z",
        "--app-id",
        "app-1",
        "--examples-only",
    ];
    let preview_before = source.sandbox.snapshot_with(&[&source.root]);
    source.command("prune", &flags).assert_success();
    source.sandbox.assert_unchanged(&preview_before);
    let mut force = flags.to_vec();
    force.push("--force");
    source.command("prune", &force).assert_success();

    let connection = source.connection();
    let metadata: (i64, String, String) = connection
        .query_row(
            "SELECT count(*),min(verdict),min(action) FROM events",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(metadata, (1, "malicious".into(), "allow".into()));
    for table in ["event_facts", "receipt_facts"] {
        let copies: i64 = connection
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE instr(fields_json,?1)>0"),
                [EXAMPLE],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(copies, 0, "example remains in {table}");
    }
    drop(connection);
    assert!(!contains(&fs::read(source.database()).unwrap(), EXAMPLE));
    let journal = source.database().with_extension("sqlite3-journal");
    assert!(!journal.exists(), "prune left a rollback journal");
    let new_export = source.sandbox.tmp().join("after-prune.jsonl");
    source.export(&new_export).assert_success();
    assert!(!contains(&fs::read(new_export).unwrap(), EXAMPLE));
    // Explicit pruning does not erase previously saved logical exports.
    assert_eq!(fs::read(&old_export).unwrap(), original_export);
    // Re-importing the pre-prune export never resurrects the pruned excerpts (M5): it
    // refuses before any change, and the archive still holds no copy.
    let before_reimport = source.sandbox.snapshot_with(&[&source.root]);
    source.import(&old_export).assert_code(1);
    source.sandbox.assert_unchanged(&before_reimport);
    assert!(!contains(&fs::read(source.database()).unwrap(), EXAMPLE));
    assert_eq!(server.request_count(), 0);
}

/// M5 review 3 finding 7: the shared exit-gate cases (`transfer_cases.rs`), exactly as
/// the service adapters run them, through the store driver: each refuses with no
/// destination change, into a missing archive (never initialised) and into a populated
/// one (byte for byte unchanged).
#[test]
fn shared_adversarial_cases_refuse_through_the_driver() {
    let server = FixtureServer::start();
    let source = Context::new(&server, "100");
    source.seed("shared-source-evidence.json", evidence());
    let exported = source.sandbox.tmp().join("shared-source.jsonl");
    source.export(&exported).assert_success();
    let export = fs::read(&exported).unwrap();
    let control = Context::new(&server, "100");
    let input = control.input(
        "shared-valid.jsonl",
        &crate::transfer_cases::encode(crate::transfer_cases::lines(&export)),
    );
    control.import(&input).assert_success();
    for (name, bytes) in crate::transfer_cases::adversarial(&export) {
        for existing in [false, true] {
            let destination = Context::new(&server, "100");
            if existing {
                let mut retained = evidence();
                retained["collection_id"] = json!("88888888-8888-4888-8888-888888888888");
                destination.seed("already-retained.json", retained);
            }
            let input = destination.input(&format!("{name}.jsonl"), &bytes);
            let before = destination
                .sandbox
                .snapshot_with(&[&destination.root, &input]);
            destination.import(&input).assert_code(1);
            if !existing {
                assert!(
                    !destination.root.exists(),
                    "{name} initialized a missing archive"
                );
            }
            destination.sandbox.assert_unchanged(&before);
        }
    }
    assert_eq!(server.request_count(), 0);
}
