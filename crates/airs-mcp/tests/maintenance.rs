//! M3.3 offline control-store maintenance through the real binary (contract C9–C11):
//! backup, an explicit move-aside, restore into a pending epoch, and Administrator
//! activation. Loopback-free: these commands never open a listener or a network
//! connection.
use std::os::unix::fs::DirBuilderExt;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_airs-mcp"))
}

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> (bool, serde_json::Value, String) {
    let out = bin().args(args).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let value = serde_json::from_str(stdout.trim()).unwrap_or(serde_json::Value::Null);
    (
        out.status.success(),
        value,
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn backup_restore_activation_and_reconciliation_through_the_binary() {
    let base = std::env::temp_dir().join(format!("airs-mcp-maint-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&base)
        .unwrap();
    let base = Dir(base.canonicalize().unwrap());
    let data = base.0.join("data");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&data)
        .unwrap();
    let text = include_str!("../config/example.toml")
        .replace("listen = \"0.0.0.0:8443\"", "listen = \"127.0.0.1:0\"")
        .replace("health_listen = \"0.0.0.0:8081\"\n", "")
        .replace("[server.tls]\ncert_file = \"/etc/airs-mcp/tls/tls.crt\"\nkey_file = \"/etc/airs-mcp/tls/tls.key\"\n", "")
        .replace("data_dir = \"/data/airs\"", &format!("data_dir = \"{}\"", data.display()));
    let config = base.0.join("config.toml");
    std::fs::write(&config, text).unwrap();
    let config = config.to_str().unwrap();
    // A first open creates the store from the seed (as `serve` would).
    let set = base.0.join("backup-1");
    let (ok, v, err) = run(&[
        "control-backup",
        "--config",
        config,
        "--dir",
        set.to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    assert_eq!(v["event"], "control_backup");
    let first_epoch = v["epoch"].as_str().unwrap().to_owned();
    assert!(set.join("manifest.json").exists());
    // Restore refuses to overwrite the live store.
    let (ok, _, err) = run(&[
        "control-restore",
        "--config",
        config,
        "--set",
        set.to_str().unwrap(),
    ]);
    assert!(!ok && err.contains("move it aside"), "{err}");
    // The operator moves the store aside explicitly; restore then yields a pending epoch.
    let control = data.join("control");
    std::fs::rename(
        control.join("control.sqlite3"),
        base.0.join("control.sqlite3.aside"),
    )
    .unwrap();
    let (ok, v, err) = run(&[
        "control-restore",
        "--config",
        config,
        "--set",
        set.to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    let pending = v["pending_epoch"].as_str().unwrap().to_owned();
    assert_ne!(pending, first_epoch);
    // Activation is explicit and one-shot.
    let (ok, v, err) = run(&[
        "control-activate-epoch",
        "--config",
        config,
        "--epoch",
        &pending,
    ]);
    assert!(ok, "{err}");
    assert_eq!(v["epoch"], pending.as_str());
    let (ok, _, _) = run(&[
        "control-activate-epoch",
        "--config",
        config,
        "--epoch",
        &pending,
    ]);
    assert!(!ok, "a second activation is refused");
    // Activation does not end the recovery quarantine; reconciliation is a separate,
    // explicit step that reports what it ended.
    let (ok, v, err) = run(&["control-reconcile", "--config", config]);
    assert!(ok, "{err}");
    assert_eq!(v["event"], "control_reconciled");
    assert_eq!(v["quarantine"], "restored");
    let (ok, v, _) = run(&["control-reconcile", "--config", config]);
    assert!(ok);
    assert_eq!(
        v["quarantine"],
        serde_json::Value::Null,
        "nothing left to reconcile"
    );
    // Bad arguments are usage errors.
    let out = bin()
        .args([
            "control-activate-epoch",
            "--config",
            config,
            "--epoch",
            "xyz",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    // M8 review 3 minor: a missing, repeated, non-numeric or zero `--keep`, and any
    // repeated, unknown or valueless flag, is a usage error (exit 2) before any storage
    // is admitted. (The configuration here is valid, so a later failure would be 1.)
    for bad in [
        vec!["control-backup-set", "--config", config, "--keep"],
        vec![
            "control-backup-set",
            "--config",
            config,
            "--keep",
            "3",
            "--keep",
            "3",
        ],
        vec!["control-backup-set", "--config", config, "--keep", "three"],
        vec!["control-backup-set", "--config", config, "--keep", "0"],
        vec!["control-backup-set", "--config", config, "--keep", "--dir"],
        vec!["control-backup-set", "--config", config, "--config", config],
        vec!["control-backup-set", "--config", config, "--unknown", "x"],
        vec!["control-backup-verify", "--set", "a", "--set", "b"],
        vec!["control-backup-verify", "--set", "a", "--latest-in", "b"],
        vec!["control-backup-verify", "--latest-in"],
    ] {
        let out = bin().args(&bad).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{bad:?}");
    }
}

/// M2 review 5 minor: command output to a failed stream never panics; each command
/// keeps its intended exit status (a successful check stays 0, a refusal stays 1).
#[test]
fn commands_keep_their_exit_status_when_output_fails() {
    let full = || {
        std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap()
    };
    let base = std::env::temp_dir().join(format!("airs-mcp-full-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&base)
        .unwrap();
    let config = base.join("config.toml");
    std::fs::write(&config, include_str!("../config/example.toml")).unwrap();
    let ok = bin()
        .args(["check-config", "--config", config.to_str().unwrap()])
        .stdout(full())
        .status()
        .unwrap();
    assert_eq!(ok.code(), Some(0), "stdout on a full device");
    let refused = bin()
        .args([
            "check-config",
            "--config",
            base.join("missing.toml").to_str().unwrap(),
        ])
        .stderr(full())
        .status()
        .unwrap();
    assert_eq!(refused.code(), Some(1), "stderr on a full device");
    std::fs::remove_dir_all(&base).unwrap();
}

/// M8 review 1 (fork C gap): the coordinated backup set (contract §9.4 "Content") through
/// the real binary. Control and every archive are copied under the exclusive owner lock,
/// with the artifact list, into a new private set whose manifest is written last; a held
/// lock refuses it; verification re-hashes every store and refuses a corrupted store,
/// an incomplete manifest and an unlisted file.
#[test]
fn the_coordinated_backup_set_is_complete_private_and_verified() {
    use std::os::unix::fs::PermissionsExt;
    let base = std::env::temp_dir().join(format!("airs-mcp-set-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&base)
        .unwrap();
    let base = Dir(base.canonicalize().unwrap());
    let data = base.0.join("data");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&data)
        .unwrap();
    let text = include_str!("../config/example.toml")
        .replace("listen = \"0.0.0.0:8443\"", "listen = \"127.0.0.1:0\"")
        .replace("health_listen = \"0.0.0.0:8081\"\n", "")
        .replace(
            "[server.tls]\ncert_file = \"/etc/airs-mcp/tls/tls.crt\"\nkey_file = \"/etc/airs-mcp/tls/tls.key\"\n",
            "",
        )
        .replace(
            "data_dir = \"/data/airs\"",
            &format!("data_dir = \"{}\"", data.display()),
        );
    let config = base.0.join("config.toml");
    std::fs::write(&config, text).unwrap();
    let config = config.to_str().unwrap();
    // The control store exists (created from the seed) and one archive store holds rows.
    let (ok, _, err) = run(&[
        "control-backup",
        "--config",
        config,
        "--dir",
        base.0.join("seed-backup").to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    let cloud = data.join("history").join("c".repeat(64));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&cloud)
        .unwrap();
    {
        let db = rusqlite::Connection::open(cloud.join("archive.sqlite3")).unwrap();
        db.execute_batch(
            "PRAGMA user_version=7; CREATE TABLE facts(id INTEGER PRIMARY KEY, v TEXT);
             INSERT INTO facts(v) VALUES ('one'),('two');",
        )
        .unwrap();
    }
    // A published version's artifacts are listed from the control store.
    {
        let c = rusqlite::Connection::open(data.join("control").join("control.sqlite3")).unwrap();
        c.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        c.execute(
            "INSERT INTO reports(report_id,tenant,kind,since_ms,until_ms,app_ids_json,title,created_ms) VALUES ('r1_x','t','manual',0,1,'[]','marker',0)",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO report_versions VALUES ('v1_x','r1_x',0,'manual',?1,?2,'s','j',0)",
            rusqlite::params![
                "d".repeat(64),
                format!(
                    r#"{{"files":[{{"name":"report.html","sha256":"{}","bytes":3}}]}}"#,
                    "a".repeat(64)
                )
            ],
        )
        .unwrap();
    }
    let sets = base.0.join("sets");
    let (ok, v, err) = run(&[
        "control-backup-set",
        "--config",
        config,
        "--dir",
        sets.to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    assert_eq!(v["event"], "backup_set");
    let set = sets.join(v["set"].as_str().unwrap());
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&set), 0o700);
    let m: serde_json::Value =
        serde_json::from_slice(&std::fs::read(set.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(m["format"], "airs-mcp-backup/1");
    assert_eq!(m["complete"], true);
    assert!(m["created_at"].is_string());
    assert_eq!(m["epoch"].as_str().unwrap().len(), 8);
    let stores = m["stores"].as_array().unwrap();
    let names: Vec<String> = stores
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        vec!["control".to_owned(), format!("archive-{}", "c".repeat(64))]
    );
    for s in stores {
        for k in ["schema", "sqlite_version", "bytes", "sha256", "rows_digest"] {
            assert!(!s[k].is_null(), "{k}");
        }
        let file = set.join(format!("{}.sqlite3", s["name"].as_str().unwrap()));
        assert_eq!(mode(&file), 0o600);
    }
    // M8 review 2 finding 5: the application schema, not the unused user_version.
    assert_eq!(
        stores[0]["schema"],
        airs_mcp::control::SCHEMA_VERSION,
        "control is schema 3"
    );
    assert_eq!(stores[1]["schema"], 7);
    assert_eq!(mode(&set.join("manifest.json")), 0o600);
    assert_eq!(
        m["artifacts"],
        serde_json::json!([{"id": "v1_x/report.html", "sha256": "a".repeat(64), "bytes": 3}])
    );
    let (ok, v, err) = run(&["control-backup-verify", "--set", set.to_str().unwrap()]);
    assert!(ok, "{err}");
    assert_eq!(v["event"], "backup_set_verified");
    // A second set is a new directory; the first is unchanged.
    let before = std::fs::read(set.join("manifest.json")).unwrap();
    let (ok, v2, err) = run(&[
        "control-backup-set",
        "--config",
        config,
        "--dir",
        sets.to_str().unwrap(),
    ]);
    assert!(ok, "{err}");
    let second = sets.join(v2["set"].as_str().unwrap());
    assert_ne!(second, set);
    assert_eq!(std::fs::read(set.join("manifest.json")).unwrap(), before);
    // The default location is DATA/backups/<id>/.
    let (ok, v3, err) = run(&["control-backup-set", "--config", config]);
    assert!(ok, "{err}");
    assert!(
        data.join("backups")
            .join(v3["set"].as_str().unwrap())
            .join("manifest.json")
            .exists()
    );
    // Refused while the data subtree's owner lock is held (the service is running).
    let admitted = airs_mcp::storage::admit(&data).unwrap();
    let owner = airs_mcp::storage::acquire(&admitted).unwrap();
    let (ok, _, err) = run(&[
        "control-backup-set",
        "--config",
        config,
        "--dir",
        sets.to_str().unwrap(),
    ]);
    assert!(!ok && err.contains("stop it first"), "{err}");
    drop(owner);
    // Verification refuses a corrupted store, an incomplete manifest and an extra file.
    let f = second.join("control.sqlite3");
    let mut b = std::fs::read(&f).unwrap();
    let last = b.len() - 1;
    b[last] ^= 0xff;
    std::fs::write(&f, b).unwrap();
    let (ok, _, err) = run(&["control-backup-verify", "--set", second.to_str().unwrap()]);
    assert!(!ok && err.contains("does not match"), "{err}");
    let mut incomplete = m.clone();
    incomplete["complete"] = serde_json::json!(false);
    std::fs::write(
        set.join("manifest.json"),
        serde_json::to_vec(&incomplete).unwrap(),
    )
    .unwrap();
    let (ok, _, err) = run(&["control-backup-verify", "--set", set.to_str().unwrap()]);
    assert!(!ok && err.contains("not complete"), "{err}");
    std::fs::write(set.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    std::fs::write(set.join("extra.sqlite3"), b"x").unwrap();
    let (ok, _, err) = run(&["control-backup-verify", "--set", set.to_str().unwrap()]);
    assert!(!ok && err.contains("unlisted"), "{err}");
    std::fs::remove_file(set.join("extra.sqlite3")).unwrap();
    // M8 review 2 finding 5: verify checks the manifest's metadata against the copies.
    let tampered = |f: &dyn Fn(&mut serde_json::Value), want: &str| {
        let mut t = m.clone();
        f(&mut t);
        std::fs::write(set.join("manifest.json"), serde_json::to_vec(&t).unwrap()).unwrap();
        let (ok, _, err) = run(&["control-backup-verify", "--set", set.to_str().unwrap()]);
        assert!(!ok && err.contains(want), "{want}: {err}");
    };
    tampered(&|t| t["epoch"] = serde_json::json!("ffffffff"), "epoch");
    tampered(
        &|t| t["stores"][0]["schema"] = serde_json::json!(9),
        "schema",
    );
    tampered(
        &|t| t["stores"][1]["schema"] = serde_json::json!(0),
        "schema",
    );
    tampered(
        &|t| t["artifacts"] = serde_json::json!({"not": "a list"}),
        "artifact",
    );
    tampered(
        &|t| {
            t["artifacts"] =
                serde_json::json!([{"id": "v1_x/report.html", "sha256": "aaaa", "bytes": 3}])
        },
        "artifact",
    );
    tampered(
        &|t| t["stores"][0]["sqlite_version"] = serde_json::json!("x"),
        "SQLite",
    );
    // M8 review 3 finding 2: the artifact list must equal the control copy's inventory.
    let good = serde_json::json!({"id": "v1_x/report.html", "sha256": "a".repeat(64), "bytes": 3});
    tampered(&|t| t["artifacts"] = serde_json::json!([]), "inventory");
    tampered(
        &|t| {
            t["artifacts"] = serde_json::json!([{"id": "v1_x/report.html", "sha256": "b".repeat(64), "bytes": 3}])
        },
        "inventory",
    );
    tampered(
        &|t| {
            t["artifacts"] = serde_json::json!([{"id": "v1_x/report.html", "sha256": "a".repeat(64), "bytes": 4}])
        },
        "inventory",
    );
    tampered(
        &|t| t["artifacts"] = serde_json::json!([good.clone(), good.clone()]),
        "inventory",
    );
    tampered(
        &|t| {
            t["artifacts"] = serde_json::json!([good.clone(),
                {"id": "v1_y/report.html", "sha256": "c".repeat(64), "bytes": 1}])
        },
        "inventory",
    );
    std::fs::write(set.join("manifest.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    let (ok, _, err) = run(&["control-backup-verify", "--set", set.to_str().unwrap()]);
    assert!(ok, "the restored manifest verifies: {err}");
    // A malformed report-version manifest fails the set instead of being skipped.
    {
        let c = rusqlite::Connection::open(data.join("control").join("control.sqlite3")).unwrap();
        c.execute(
            "UPDATE report_versions SET manifest_json=?1 WHERE version_id='v1_x'",
            [r#"{"files":[{"name":"report.html","sha256":"aaaa","bytes":3}]}"#],
        )
        .unwrap();
    }
    let before_sets = std::fs::read_dir(&sets).unwrap().count();
    let (ok, _, err) = run(&[
        "control-backup-set",
        "--config",
        config,
        "--dir",
        sets.to_str().unwrap(),
    ]);
    assert!(!ok && err.contains("malformed"), "{err}");
    let completed = std::fs::read_dir(&sets)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("manifest.json").exists())
        .count();
    assert_eq!(completed, before_sets, "no new complete set");
    {
        let c = rusqlite::Connection::open(data.join("control").join("control.sqlite3")).unwrap();
        c.execute(
            "UPDATE report_versions SET manifest_json=?1 WHERE version_id='v1_x'",
            [format!(
                r#"{{"files":[{{"name":"report.html","sha256":"{}","bytes":3}}]}}"#,
                "a".repeat(64)
            )],
        )
        .unwrap();
    }
    // An archive that cannot be inspected fails the set instead of being left out.
    if rustix::process::geteuid().as_raw() != 0 {
        std::fs::set_permissions(&cloud, std::fs::Permissions::from_mode(0o000)).unwrap();
        let (ok, _, err) = run(&[
            "control-backup-set",
            "--config",
            config,
            "--dir",
            sets.to_str().unwrap(),
        ]);
        std::fs::set_permissions(&cloud, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(!ok && err.contains("archive"), "{err}");
    }
}
