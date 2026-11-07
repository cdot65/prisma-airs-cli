use airs_test_harness::{Cli, FixtureServer, RunOptions, Sandbox, run};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs-history-driver"))
}
fn evidence() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/signal/evidence.json")).unwrap()
}
fn save(s: &Sandbox, mut value: Value) -> PathBuf {
    value.as_object_mut().unwrap().remove("digest");
    value["digest"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&value).unwrap())
    ));
    let file = s.tmp().join("input.json");
    std::fs::write(&file, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    file
}
fn scope(s: &Sandbox, root: &Path) -> Value {
    let out = run(
        &rust(),
        s,
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
    out.assert_success();
    serde_json::from_str(&out.stdout).unwrap()
}
fn import(s: &Sandbox, root: &Path, file: &Path, environment: &str) -> airs_test_harness::Run {
    run(
        &rust(),
        s,
        &[
            "runtime",
            "history",
            "import-bundle",
            "--file",
            file.to_str().unwrap(),
            "--history-dir",
            root.to_str().unwrap(),
            "--expect-tsg",
            "100",
            "--expect-environment",
            environment,
            "--output",
            "json",
        ],
        &RunOptions::new(),
    )
}
fn connection(root: &Path, scope: &Value) -> Connection {
    Connection::open_with_flags(
        root.join(scope["cloud_id"].as_str().unwrap())
            .join("archive.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
}

#[test]
fn imported_facts_are_idempotent_and_conflicts_do_not_use_last_write_truth() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("imported");
    let binding = scope(&s, &root);
    let env = binding["environment_id"].as_str().unwrap();
    let file = save(&s, evidence());
    let input = std::fs::read(&file).unwrap();
    let out = import(&s, &root, &file, env);
    out.assert_success();
    assert_eq!(std::fs::read(&file).unwrap(), input);
    let after_first = s.snapshot_with(&[&root]);
    let out = import(&s, &root, &file, env);
    out.assert_success();
    assert_eq!(
        serde_json::from_str::<Value>(&out.stdout).unwrap()["processed_records"],
        0
    );
    s.assert_unchanged(&after_first);
    let conn = connection(&root, &binding);
    let count: i64 = conn
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    drop(conn);
    let mut changed = evidence();
    changed["events"][0]["action"] = json!("block");
    let file = save(&s, changed.clone());
    let out = import(&s, &root, &file, env);
    out.assert_code(1);
    s.assert_unchanged(&after_first);
    changed["collection_id"] = json!("22222222-2222-4222-8222-222222222222");
    let file = save(&s, changed);
    import(&s, &root, &file, env).assert_success();
    let conn = connection(&root, &binding);
    let result: (String, String, i64) = conn
        .query_row("SELECT verdict,action,conflicting FROM events", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(result, ("malicious".into(), "unknown".into(), 1));
    for (table, expected) in [
        ("event_facts", 2),
        ("fact_runs", 2),
        ("collection_runs", 2),
        ("profile_observations", 1),
        ("profile_runs", 2),
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, expected, "{table}");
    }
    assert_eq!(server.request_count(), 0);
}

#[test]
fn partial_receipts_promote_in_both_orders_and_later_ambiguity_invalidates() {
    for receipt_first in [true, false] {
        let server = FixtureServer::start();
        let s = Sandbox::new();
        s.single_tenant(&server, "prod", "100");
        let root = s.tmp().join("promotions");
        let binding = scope(&s, &root);
        let env = binding["environment_id"].as_str().unwrap();
        let mut partial = evidence();
        partial["events"][0]["identity"]["sub_request"] = Value::Null;
        partial["collection_id"] = json!("33333333-3333-4333-8333-333333333333");
        let pair = if receipt_first {
            vec![partial, evidence()]
        } else {
            vec![evidence(), partial]
        };
        for item in pair {
            let path = save(&s, item);
            import(&s, &root, &path, env).assert_success();
        }
        let conn = connection(&root, &binding);
        let state: String = conn
            .query_row("SELECT state FROM receipts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(state, "promoted");
        let events: i64 = conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(events, 1);
        drop(conn);
        let mut competing = evidence();
        competing["collection_id"] = json!("44444444-4444-4444-8444-444444444444");
        competing["events"][0]["identity"]["sub_request"] = json!(1);
        let path = save(&s, competing);
        import(&s, &root, &path, env).assert_success();
        let conn = connection(&root, &binding);
        let result: (String, Option<String>) = conn
            .query_row("SELECT state,promoted_event_id FROM receipts", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(result, ("ambiguous".into(), None));
        let units: (i64, i64) = conn
            .query_row(
                "SELECT count(*),count(DISTINCT scan_id) FROM events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(units, (2, 1));
        assert_eq!(server.request_count(), 0);
    }
}

#[test]
fn invalid_input_binding_and_versions_do_not_create_archives_or_change_input() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("invalid");
    let binding = scope(&s, &root);
    let env = binding["environment_id"].as_str().unwrap();
    let file = save(&s, evidence());
    let original = std::fs::read(&file).unwrap();
    let wrong = format!("env1:{}", "a".repeat(64));
    import(&s, &root, &file, &wrong).assert_code(1);
    assert!(!root.exists());
    assert_eq!(std::fs::read(&file).unwrap(), original);
    for mode in ["digest", "duplicate", "version", "outcome"] {
        let mut item = evidence();
        match mode {
            "digest" => item["digest"] = json!("f".repeat(64)),
            "version" => item["rules_version"] = json!(999),
            "outcome" => item["events"][0]["action"] = json!("invented-allow"),
            _ => {}
        };
        let path = if mode == "digest" {
            std::fs::write(&file, serde_json::to_vec(&item).unwrap()).unwrap();
            file.clone()
        } else if mode == "duplicate" {
            std::fs::write(&file, "{\"schema_version\":1,\"schema_version\":1}").unwrap();
            file.clone()
        } else {
            save(&s, item)
        };
        let before = std::fs::read(&path).unwrap();
        import(&s, &root, &path, env).assert_code(1);
        assert!(!root.exists(), "{mode}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    assert_eq!(server.request_count(), 0);
}

#[test]
fn privacy_changed_and_prewithheld_labels_cannot_forge_durable_associations() {
    for already_withheld in [false, true] {
        let server = FixtureServer::start();
        let s = Sandbox::new();
        s.single_tenant(&server, "prod", "100");
        let root = s.tmp().join("privacy");
        let binding = scope(&s, &root);
        let env = binding["environment_id"].as_str().unwrap();
        for (index, label) in ["alpha@example.invalid", "beta@example.invalid"]
            .iter()
            .enumerate()
        {
            let mut item = evidence();
            item["collection_id"] =
                json!(format!("{}1111111-1111-4111-8111-111111111111", index + 5));
            let name = if already_withheld {
                "[sensitive value withheld]"
            } else {
                label
            };
            item["profiles"][0]["name"] = json!(name);
            item["events"][0]["profile"] = json!(name);
            if !item["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["name"] == "applications")
            {
                let mut source = item["sources"][0].clone();
                source["name"] = json!("applications");
                item["sources"].as_array_mut().unwrap().push(source);
            }
            item["metrics"] = json!([
                {"name":"sessions_total","value":100.0,"unit":"sessions","scale":null,"population":name,"window":"Rolling 24 hours","source":"applications","status":"complete"},
                {"name":"sessions_violated","value":20.0,"unit":"sessions","scale":null,"population":name,"window":"Rolling 24 hours","source":"applications","status":"complete"}
            ]);
            for metric in item["metrics"].as_array_mut().unwrap() {
                metric["population"] = json!(name);
            }
            let input = save(&s, item);
            import(&s, &root, &input, env).assert_success();
        }
        let conn = connection(&root, &binding);
        let profiles: (i64, i64) = conn
            .query_row(
                "SELECT count(*),count(DISTINCT profile_id) FROM profile_observations",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(profiles, (2, 2));
        let linked: i64 = conn
            .query_row(
                "SELECT count(*) FROM events WHERE profile IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked, 0);
        let count: i64 = conn
            .query_row(
                "SELECT count(DISTINCT population) FROM aggregate_observations",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, if already_withheld { 4 } else { 2 });
        let private:i64=conn.query_row("SELECT count(*) FROM aggregate_observations WHERE population NOT LIKE 'unlinked:%'",[],|r|r.get(0)).unwrap();
        assert_eq!(private, 0);
        drop(conn);
        let bytes = std::fs::read(
            root.join(binding["cloud_id"].as_str().unwrap())
                .join("archive.sqlite3"),
        )
        .unwrap();
        for label in [
            b"alpha@example.invalid".as_slice(),
            b"beta@example.invalid".as_slice(),
        ] {
            assert!(!bytes.windows(label.len()).any(|part| part == label));
        }
        assert_eq!(server.request_count(), 0);
    }
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("withheld-identity");
    let binding = scope(&s, &root);
    let env = binding["environment_id"].as_str().unwrap();
    let mut item = evidence();
    item["events"][0]["identity"]["app_name"] = json!("[sensitive value withheld]");
    let input = save(&s, item);
    import(&s, &root, &input, env).assert_success();
    let conn = connection(&root, &binding);
    let count: i64 = conn
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    let name: Option<String> = conn
        .query_row("SELECT app_name FROM receipts", [], |r| r.get(0))
        .unwrap();
    assert!(name.is_none());
}

#[test]
fn legacy_selected_coverage_is_observed_and_preserves_its_exact_scope() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("scoped");
    let binding = scope(&s, &root);
    let env = binding["environment_id"].as_str().unwrap();
    for (index, apps) in [
        vec!["app-a"],
        vec!["app-a", "app-b"],
        vec!["[sensitive value withheld]"],
        vec!["changed-app\n"],
    ]
    .iter()
    .enumerate()
    {
        let mut item = evidence();
        item["collection_id"] = json!(format!("{}2222222-2222-4222-8222-222222222222", index + 5));
        item["app_ids"] = json!(apps);
        for source in item["sources"].as_array_mut().unwrap() {
            source["status"] = json!("complete");
        }
        let input = save(&s, item);
        import(&s, &root, &input, env).assert_success();
    }
    let conn = connection(&root, &binding);
    let wrong:i64=conn.query_row("SELECT count(*) FROM coverage_segments WHERE state='traversed_as_of' OR app_id='[sensitive value withheld]'",[],|r|r.get(0)).unwrap();
    assert_eq!(wrong, 0);
    let scoped:i64=conn.query_row("SELECT count(*) FROM coverage_segments WHERE app_id='app-a' AND json_extract(fields_json,'$.scope_kind')='application'",[],|r|r.get(0)).unwrap();
    assert!(scoped > 0);
    let selected:i64=conn.query_row("SELECT count(*) FROM coverage_segments WHERE app_id IS NULL AND json_extract(fields_json,'$.scope_kind')='selection' AND json_array_length(fields_json,'$.selected_app_ids')=2",[],|r|r.get(0)).unwrap();
    assert!(selected > 0);
    let unknown:i64=conn.query_row("SELECT count(*) FROM coverage_segments WHERE app_id IS NULL AND json_extract(fields_json,'$.scope_kind')='unknown_selection'",[],|r|r.get(0)).unwrap();
    assert!(unknown > 0);
    let changed:i64=conn.query_row("SELECT count(*) FROM coverage_segments WHERE app_id IS NULL AND json_extract(fields_json,'$.scope_kind')='unknown_selection' AND json_extract(fields_json,'$.selected_app_ids[0]')='changed-app' AND json_extract(fields_json,'$.scope_identity_supported')=0",[],|r|r.get(0)).unwrap();
    assert!(changed > 0);
    assert_eq!(server.request_count(), 0);
}

#[test]
fn detector_projection_distinguishes_positive_unknown_hints_from_report_context() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("detectors");
    let binding = scope(&s, &root);
    let env = binding["environment_id"].as_str().unwrap();
    let mut item = evidence();
    item["events"][0]["detector_hints"] = json!(["unknown/pi"]);
    item["events"][0]["detections"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    item["events"][0]["detections"][0]["detector"] = json!("pi");
    item["events"][0]["detections"][0]["direction"] = json!("prompt");
    item["events"][0]["detections"][0]["verdict"] = json!("unknown");
    item["events"][0]["detections"][0]["action"] = json!("unknown");
    let input = save(&s, item.clone());
    import(&s, &root, &input, env).assert_success();
    let conn = connection(&root, &binding);
    let rows:Vec<(String,i64,i64)>=conn.prepare("SELECT direction,json_extract(provenance_json,'$.positive_metadata'),json_extract(provenance_json,'$.report_observed') FROM detector_observations WHERE detector='prompt-injection' ORDER BY direction").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    assert_eq!(
        rows,
        vec![("prompt".into(), 0, 1), ("unknown".into(), 1, 0)]
    );
    drop(conn);
    item["collection_id"] = json!("77777777-7777-4777-8777-777777777777");
    item["events"][0]["detections"][0]["verdict"] = json!("malicious");
    let input = save(&s, item);
    import(&s, &root, &input, env).assert_success();
    let conn = connection(&root, &binding);
    let count:i64=conn.query_row("SELECT count(*) FROM detector_observations WHERE detector='prompt-injection' AND direction='unknown'",[],|r|r.get(0)).unwrap();
    assert_eq!(count, 0);
    let raw: i64 = conn
        .query_row(
            "SELECT count(*) FROM event_facts WHERE fields_json LIKE '%unknown/pi%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(raw, 2);
    drop(conn);
    // A pure unknown-direction context row must survive a known positive hint.
    let other_root = s.tmp().join("report-context");
    let mut item = evidence();
    item["events"][0]["detector_hints"] = json!(["prompt/pi"]);
    item["events"][0]["detections"]
        .as_array_mut()
        .unwrap()
        .truncate(1);
    let detection = &mut item["events"][0]["detections"][0];
    detection["detector"] = json!("pi");
    detection["direction"] = json!("unknown");
    detection["verdict"] = json!("benign");
    let input = save(&s, item);
    import(&s, &other_root, &input, env).assert_success();
    let conn = connection(&other_root, &binding);
    let context:(String,i64,i64)=conn.query_row("SELECT verdict,json_extract(provenance_json,'$.report_observed'),json_extract(provenance_json,'$.positive_metadata') FROM detector_observations WHERE detector='prompt-injection' AND direction='unknown'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(context, ("benign".into(), 1, 0));
}

#[test]
fn failed_outcome_never_materializes_as_allow() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("failed-outcome");
    let binding = scope(&s, &root);
    let mut item = evidence();
    item["events"][0]["verdict"] = json!("error");
    item["events"][0]["action"] = json!("allow");
    let input = save(&s, item);
    import(
        &s,
        &root,
        &input,
        binding["environment_id"].as_str().unwrap(),
    )
    .assert_success();
    let conn = connection(&root, &binding);
    let outcome: (String, String, i64) = conn
        .query_row("SELECT verdict,action,conflicting FROM events", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(outcome, ("error".into(), "unknown".into(), 1));
    assert_eq!(server.request_count(), 0);
}

#[test]
fn zero_event_inventory_retains_public_identity_and_unlinked_private_observations() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("inventory");
    let binding = scope(&s, &root);
    for (i, label) in [
        "Synthetic assistant",
        "Synthetic assistant",
        "private@example.invalid",
    ]
    .iter()
    .enumerate()
    {
        let mut item = evidence();
        item["events"] = json!([]);
        item["sampling"]["selected_report_ids"] = json!([]);
        item["sampling"]["candidates"] = json!(0);
        item["sources"][0]["records"] = json!(0);
        item["collection_id"] = json!(format!("{}7777777-7777-4777-8777-777777777777", i + 1));
        item["applications"][0]["name"] = json!(label);
        let input = save(&s, item);
        import(
            &s,
            &root,
            &input,
            binding["environment_id"].as_str().unwrap(),
        )
        .assert_success();
    }
    let conn = connection(&root, &binding);
    let counts: (i64, i64, i64, i64) = conn.query_row(
        "SELECT (SELECT count(*) FROM events),(SELECT count(*) FROM application_observations),(SELECT count(*) FROM application_runs),(SELECT count(*) FROM application_observations WHERE app_id IS NULL AND app_name IS NULL)", [],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    ).unwrap();
    assert_eq!(counts, (0, 2, 3, 1));
    drop(conn);
    let bytes = std::fs::read(
        root.join(binding["cloud_id"].as_str().unwrap())
            .join("archive.sqlite3"),
    )
    .unwrap();
    assert!(
        !bytes
            .windows(b"private@example.invalid".len())
            .any(|s| s == b"private@example.invalid")
    );
    assert_eq!(server.request_count(), 0);
}

#[test]
fn repeated_profile_content_keeps_out_of_order_observation_times() {
    let server = FixtureServer::start();
    let s = Sandbox::new();
    s.single_tenant(&server, "prod", "100");
    let root = s.tmp().join("profile-times");
    let binding = scope(&s, &root);
    let env = binding["environment_id"].as_str().unwrap();
    let baseline = evidence();
    let end = baseline["window"]["end_ms"].as_i64().unwrap();
    for (position, offset, revision) in [(0, 3000, 1), (1, 1000, 1), (2, 2000, 2)] {
        let mut item = baseline.clone();
        item["collection_id"] = json!(format!(
            "{}3333333-3333-4333-8333-333333333333",
            position + 5
        ));
        item["collected_ms"] = json!(end + offset);
        item["profiles"][0]["revision"] = json!(revision);
        let input = save(&s, item);
        import(&s, &root, &input, env).assert_success();
    }
    let conn = connection(&root, &binding);
    let counts:(i64,i64)=conn.query_row("SELECT (SELECT count(*) FROM profile_observations),(SELECT count(*) FROM profile_runs)",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(counts, (2, 3));
    for (offset, expected) in [(1000, 1), (2000, 2), (3000, 1)] {
        let revision:i64=conn.query_row("SELECT p.revision FROM profile_observations p JOIN profile_runs r ON r.observation_id=p.observation_id WHERE r.observed_ms<=?1 ORDER BY r.observed_ms DESC LIMIT 1",[end+offset],|r|r.get(0)).unwrap();
        assert_eq!(revision, expected);
    }
}

#[cfg(debug_assertions)]
#[test]
fn import_signals_after_a_committed_batch_report_partial_progress_and_resume() {
    use std::time::{Duration, Instant};
    for (signal, code) in [("INT", 130), ("TERM", 143)] {
        let server = FixtureServer::start();
        let s = Sandbox::new();
        s.single_tenant(&server, "prod", "100");
        let root = s.tmp().join("interrupted-import");
        let binding = scope(&s, &root);
        let env = binding["environment_id"].as_str().unwrap();
        let file = save(&s, evidence());
        let input = std::fs::read(&file).unwrap();
        let marker = s.tmp().join("batch-committed");
        let signal_marker = s.tmp().join("signal-recorded");
        let options = RunOptions::new()
            .env(
                "AIRS_CLI_TEST_HISTORY_BATCH_MARKER",
                marker.to_str().unwrap(),
            )
            .signal_marker(&signal_marker);
        let args = [
            "runtime",
            "history",
            "import-bundle",
            "--file",
            file.to_str().unwrap(),
            "--history-dir",
            root.to_str().unwrap(),
            "--expect-tsg",
            "100",
            "--expect-environment",
            env,
            "--output",
            "json",
        ];
        let mut running = airs_test_harness::spawn(&rust(), &s, &args, &options);
        let until = Instant::now() + Duration::from_secs(10);
        while !marker.exists() {
            assert!(
                Instant::now() < until,
                "import never committed its first batch"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        running.send_signal_recorded(signal);
        let out = running.wait();
        out.assert_code(code);
        assert!(out.stdout.is_empty());
        assert!(out.stderr.contains("may contain committed import batches"));
        assert!(
            !out.stderr
                .to_ascii_lowercase()
                .contains("no data was written")
        );
        let conn = connection(&root, &binding);
        let count: i64 = conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let state: String = conn
            .query_row("SELECT state FROM collection_runs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(state, "running");
        drop(conn);
        assert_eq!(std::fs::read(&file).unwrap(), input);
        import(&s, &root, &file, env).assert_success();
        let conn = connection(&root, &binding);
        let count: i64 = conn
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(server.request_count(), 0);
    }
}
