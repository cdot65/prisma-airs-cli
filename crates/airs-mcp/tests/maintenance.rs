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
