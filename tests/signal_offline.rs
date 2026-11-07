use airs_test_harness::{Cli, RunOptions, Sandbox, run};

#[test]
fn saved_evidence_renders_without_a_tenant_or_network() {
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("evidence.json");
    std::fs::write(&input, include_str!("fixtures/signal/evidence.json")).unwrap();
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "--tenant",
            "does-not-exist",
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(out.stdout.contains("Synthetic assistant") || out.stdout.contains("prompt_injection"));
    assert!(!out.stderr.contains("Collecting"));
}

#[test]
fn metric_projection_cannot_create_summary_joins_or_recognized_labels() {
    use serde_json::json;
    use sha2::{Digest, Sha256};
    for mode in ["population", "name", "unit", "window", "scale"] {
        let s = Sandbox::new();
        let input = s.tmp().join("metrics.json");
        let destination = s.tmp().join("report.html");
        let mut value: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/signal/evidence.json")).unwrap();
        let first = json!({"name":"sessions_total","value":100.0,"unit":"sessions","scale":null,"population":"Application A","window":"Rolling 24 hours","source":"applications","status":"complete"});
        let mut second = first.clone();
        second["name"] = json!("sessions_violated");
        second["value"] = json!(33.0);
        value["metrics"] = json!([first, second]);
        let mut source = value["sources"][0].clone();
        source["name"] = json!("applications");
        source["unit"] = json!("application buckets");
        value["sources"].as_array_mut().unwrap().push(source);
        let mut pristine = value.clone();
        pristine.as_object_mut().unwrap().remove("digest");
        pristine["digest"] = json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&pristine).unwrap())
        ));
        std::fs::write(&input, serde_json::to_vec(&pristine).unwrap()).unwrap();
        let baseline = run(
            &Cli::rust(env!("CARGO_BIN_EXE_airs")),
            &s,
            &[
                "runtime",
                "report",
                "--from-evidence",
                input.to_str().unwrap(),
                "--output-file",
                "-",
            ],
            &RunOptions::new(),
        );
        baseline.assert_success();
        assert!(baseline.stdout.contains("Management session activity"));
        assert!(baseline.stdout.contains(">100</td><td>33</td>"));
        match mode {
            "population" => {
                value["metrics"][0]["population"] = json!("alpha@example.invalid");
                value["metrics"][1]["population"] = json!("beta@example.invalid");
            }
            "name" => value["metrics"][0]["name"] = json!("sessions_\ntotal"),
            "unit" => value["metrics"][0]["unit"] = json!("ses\nsions"),
            "window" => value["metrics"][0]["window"] = json!("Rolling\n 24 hours"),
            "scale" => value["metrics"][0]["scale"] = json!("thou\nsands"),
            _ => unreachable!(),
        }
        value.as_object_mut().unwrap().remove("digest");
        let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap()));
        value["digest"] = json!(digest);
        std::fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
        let before = std::fs::read(&input).unwrap();
        let out = run(
            &Cli::rust(env!("CARGO_BIN_EXE_airs")),
            &s,
            &[
                "runtime",
                "report",
                "--from-evidence",
                input.to_str().unwrap(),
                "--output-file",
                destination.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        out.assert_code(1);
        assert!(out.stdout.is_empty());
        assert!(!destination.exists());
        assert_eq!(std::fs::read(input).unwrap(), before);
    }
}
#[test]
fn malformed_evidence_and_usage_do_not_resolve_a_tenant() {
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("bad.json");
    std::fs::write(&input, "{\"SECRET-FILE-TEXT\":true}").unwrap();
    let cli = Cli::rust(env!("CARGO_BIN_EXE_airs"));
    let args = [
        "runtime",
        "report",
        "--from-evidence",
        input.to_str().unwrap(),
        "--output-file",
        "-",
    ];
    let out = run(&cli, &sandbox, &args, &RunOptions::new());
    assert_eq!(out.code, 1);
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.contains("SECRET-FILE-TEXT"));
    let mut usage = args.to_vec();
    usage.extend(["--max-pages", "2"]);
    let out = run(&cli, &sandbox, &usage, &RunOptions::new());
    assert_eq!(out.code, 2);
}

#[test]
fn credentials_discovered_late_cannot_leak_from_earlier_fields() {
    use sha2::{Digest, Sha256};
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("forged.json");
    let canary = "UNIQUE-CANARY-secret-123456789";
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/signal/evidence.json")).unwrap();
    value["events"][0]["identity"]["scan_id"] =
        serde_json::json!(format!("{}{canary}", "a ".repeat(2042)));
    value["events"][0]["notes"] = serde_json::json!([format!("Authorization: Bearer {canary}")]);
    value.as_object_mut().unwrap().remove("digest");
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap()));
    value["digest"] = serde_json::json!(digest);
    std::fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(!out.stdout.contains("UNIQUE-CANAR"));
    assert!(
        out.stdout
            .contains("Additional privacy withholding was applied")
    );
    assert!(!out.stderr.contains(canary));
}

#[test]
fn title_discovery_also_masks_prior_evidence_fields() {
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("evidence.json");
    let canary = "TITLE-CANARY-secret-123456789";
    let mut value: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/signal/evidence.json")).unwrap();
    value["events"][0]["identity"]["scan_id"] =
        serde_json::json!(format!("{}{canary}", "a ".repeat(2042)));
    value.as_object_mut().unwrap().remove("digest");
    use sha2::{Digest, Sha256};
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap()));
    value["digest"] = serde_json::json!(digest);
    std::fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    let title = format!("Authorization: Bearer {canary}");
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--title",
            &title,
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(!out.stdout.contains("TITLE-CANAR"));
    assert!(!out.stderr.contains(canary));
}

#[test]
fn private_bundle_preserves_digest_and_refuses_unsafe_republication() {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("evidence.json");
    let original = include_str!("fixtures/signal/evidence.json");
    std::fs::write(&input, original).unwrap();
    let dest = sandbox.tmp().join("bundle");
    let invoke = |path: &std::path::Path, extra: &[&str]| {
        let mut args = vec![
            "--quiet",
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--bundle-dir",
            path.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        run(
            &Cli::rust(env!("CARGO_BIN_EXE_airs")),
            &sandbox,
            &args,
            &RunOptions::new(),
        )
    };
    let out = invoke(&dest, &[]);
    out.assert_success();
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dest.join("evidence.json")).unwrap())
            .unwrap();
    let expected: serde_json::Value = serde_json::from_str(original).unwrap();
    assert_eq!(saved, expected);
    assert_eq!(
        std::fs::metadata(&dest).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for file in ["report.html", "evidence.json", "review.md"] {
        assert_eq!(
            std::fs::metadata(dest.join(file))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let packet = std::fs::read_to_string(dest.join("review.md")).unwrap();
    assert!(packet.contains(saved["digest"].as_str().unwrap()));
    assert!(packet.contains(saved["collection_id"].as_str().unwrap()));
    assert!(packet.contains("prompt") && packet.contains("finding_id"));
    invoke(&dest, &[]).assert_code(1);
    assert_eq!(std::fs::read_to_string(&input).unwrap(), original);
    let mut forged = expected;
    forged["events"][0]["notes"] =
        serde_json::json!(["Authorization: Bearer FORGED-SECRET-0123456789"]);
    forged.as_object_mut().unwrap().remove("digest");
    forged["digest"] = serde_json::json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&forged).unwrap())
    ));
    std::fs::write(&input, serde_json::to_vec(&forged).unwrap()).unwrap();
    let blocked = sandbox.tmp().join("unsafe-bundle");
    invoke(&blocked, &[]).assert_code(1);
    assert!(!blocked.exists());
    assert!(std::fs::read_dir(sandbox.tmp()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".airs-")
    }));
}

#[test]
fn bundle_preflight_and_mode_conflicts_precede_tenant_access() {
    let sandbox = Sandbox::new();
    let existing = sandbox.tmp().join("existing");
    std::fs::create_dir(&existing).unwrap();
    let cli = Cli::rust(env!("CARGO_BIN_EXE_airs"));
    let out = run(
        &cli,
        &sandbox,
        &[
            "runtime",
            "report",
            "--bundle-dir",
            existing.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    assert!(!out.stderr.contains("tenant"));
    let out = run(
        &cli,
        &sandbox,
        &[
            "runtime",
            "report",
            "--bundle-dir",
            existing.to_str().unwrap(),
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    );
    out.assert_code(2);
}

#[test]
fn restrictive_umask_failure_cleans_owned_staging_before_open() {
    // Run in a child shell so process-global umask cannot affect concurrent tests.
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("evidence.json");
    std::fs::write(&input, include_str!("fixtures/signal/evidence.json")).unwrap();
    let path = sandbox.tmp().join("brief");
    let output = std::process::Command::new("bash")
        .args([
            "-c",
            "umask 0777; exec \"$1\" runtime report --from-evidence \"$2\" --bundle-dir \"$3\"",
            "airs-bundle-test",
        ])
        .arg(env!("CARGO_BIN_EXE_airs"))
        .arg(&input)
        .arg(&path)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("AIRS_CLI_CONFIG_DIR", sandbox.tmp().join("private-store"))
        .output()
        .unwrap();
    // A privileged runner may open mode000; both a complete private result and safe
    // failure are permitted. No partially staged directory may survive.
    if output.status.success() {
        assert!(path.join("report.html").is_file());
    } else {
        assert!(!path.exists());
    }
    assert!(std::fs::read_dir(sandbox.tmp()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".airs-")
    }));
}

#[test]
fn control_normalized_dlp_matches_are_withheld_across_families_offline() {
    use sha2::{Digest, Sha256};
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("forged-evidence.json");
    let mut e: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/signal/evidence.json")).unwrap();
    let sensitive = "SYNTHETIC-CONTROL-CANARY-A\nSYNTHETIC-CONTROL-CANARY-B\u{1b}";
    let normalized: String = sensitive.chars().filter(|c| !c.is_control()).collect();
    e["events"][0]["detections"][0]["snippets"][0]["excerpts"] =
        serde_json::json!([sensitive, normalized]);
    e["events"][0]["detections"][0]["snippets"][1] = serde_json::json!({"family":"dlp","state":"present","excerpts":[sensitive],"variants":1,"truncated":false});
    e.as_object_mut().unwrap().remove("digest");
    e["digest"] = serde_json::json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&e).unwrap())
    ));
    std::fs::write(&input, serde_json::to_vec(&e).unwrap()).unwrap();
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(!out.stdout.contains("SYNTHETIC-CONTROL-CANARY"));
}

#[test]
fn ambiguous_masked_profile_names_fail_before_artifacts_are_published() {
    use sha2::{Digest, Sha256};
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("ambiguous-evidence.json");
    let mut e: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/signal/evidence.json")).unwrap();
    e["events"][0]["profile"] = serde_json::json!("blocker@example.invalid");
    e["profiles"][0]["name"] = serde_json::json!("blocker@example.invalid");
    e["profiles"][0]["protections"][0]["action"] = serde_json::json!("block");
    let mut other = e["profiles"][0].clone();
    other["name"] = serde_json::json!("monitor@example.invalid");
    other["revision"] = serde_json::json!(2);
    other["protections"][0]["action"] = serde_json::json!("allow");
    e["profiles"].as_array_mut().unwrap().push(other);
    e.as_object_mut().unwrap().remove("digest");
    e["digest"] = serde_json::json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&e).unwrap())
    ));
    let original = serde_json::to_vec(&e).unwrap();
    std::fs::write(&input, &original).unwrap();
    let path = sandbox.tmp().join("refused.html");
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--output-file",
            path.to_str().unwrap(),
        ],
        &RunOptions::new(),
    );
    out.assert_code(1);
    assert!(out.stdout.is_empty());
    assert!(!path.exists());
    assert!(!out.stderr.contains("example.invalid"));
    assert_eq!(std::fs::read(&input).unwrap(), original);
}

#[test]
fn generated_header_dates_cannot_repeat_withheld_dlp_payloads() {
    use sha2::{Digest, Sha256};
    let sandbox = Sandbox::new();
    let input = sandbox.tmp().join("sensitive-date.json");
    let mut e: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/signal/evidence.json")).unwrap();
    let date = "2026-11-01T00:00:00.000Z";
    e["events"][0]["detections"][0]["snippets"][1] = serde_json::json!({"family":"dlp","state":"present","excerpts":[date],"variants":1,"truncated":false});
    e.as_object_mut().unwrap().remove("digest");
    e["digest"] = serde_json::json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&e).unwrap())
    ));
    std::fs::write(&input, serde_json::to_vec(&e).unwrap()).unwrap();
    let out = run(
        &Cli::rust(env!("CARGO_BIN_EXE_airs")),
        &sandbox,
        &[
            "runtime",
            "report",
            "--from-evidence",
            input.to_str().unwrap(),
            "--output-file",
            "-",
        ],
        &RunOptions::new(),
    );
    out.assert_success();
    assert!(!out.stdout.contains(date));
    assert!(
        out.stdout
            .contains("Reporting window: [sensitive value withheld]")
    );
}

#[test]
fn detector_alias_control_normalization_fails_before_offline_bundle_publication() {
    use sha2::{Digest, Sha256};
    let sandbox = Sandbox::new();
    for case in ["control", "hint"] {
        let mut e: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/signal/evidence-v2.json")).unwrap();
        if case == "control" {
            e["profiles"][0]["protections"][0]["detector"] =
                serde_json::json!("prompt\n-injection");
        } else {
            e["events"][0]["detector_hints"] = serde_json::json!(["prompt/p\ni"]);
        }
        e.as_object_mut().unwrap().remove("digest");
        e["digest"] = serde_json::json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&e).unwrap())
        ));
        let input = sandbox.tmp().join(format!("{case}.json"));
        std::fs::write(&input, serde_json::to_vec(&e).unwrap()).unwrap();
        let dest = sandbox.tmp().join(format!("{case}-bundle"));
        let out = run(
            &Cli::rust(env!("CARGO_BIN_EXE_airs")),
            &sandbox,
            &[
                "runtime",
                "report",
                "--from-evidence",
                input.to_str().unwrap(),
                "--bundle-dir",
                dest.to_str().unwrap(),
            ],
            &RunOptions::new(),
        );
        out.assert_code(1);
        assert!(out.stdout.is_empty());
        assert!(!dest.exists());
        assert!(!out.stderr.contains("prompt\n-injection"));
    }
}
