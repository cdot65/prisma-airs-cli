//! M8 owner-run helpers (scripts/m8/), run as child processes (M8 review 1 findings 3, 4
//! and 8):
//! - `user-checklist.sh --check KIND` refuses credential-like text by field kind, accepts
//!   image digests where a hash is expected, and restricts status answers;
//! - `check-r2-delete-rights.sh` is an offline validator of a sanitized inventory: clean,
//!   incomplete, violation and secret-bearing inventories each get their own exit code,
//!   and a refused value is never echoed;
//! - `build-hashes.sh` fails when a build fails or produces no binary, records a missing
//!   required target as missing (exit 3), and records the whole platform matrix, using
//!   mock `cargo`, `rustc` and `rustup` on `PATH`.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn script(name: &str) -> PathBuf {
    root().join("scripts/m8").join(name)
}

fn temp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("airs-m8-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&d).unwrap();
    d
}

fn check(kind: &str, value: &str) -> i32 {
    let mut child = Command::new("bash")
        .arg(script("user-checklist.sh"))
        .args(["--check", kind])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("{value}\n").as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains(value) || value.is_empty(),
        "the value was echoed"
    );
    out.status.code().unwrap()
}

/// Finding 4: credential-like answers are refused (3), digests and ordinary answers are
/// accepted (0), and status answers are restricted (4).
#[test]
fn the_checklist_validates_each_field_kind() {
    let digest = format!("sha256:{}", "a1".repeat(32));
    for refused in [
        "password=hunter2",
        "the admin PASSWORD: s3cretvalue",
        "client_secret = abcdef123456",
        "api_key=AbCdEf123456",
        "token: abc.def.ghi",
        "Authorization: Bearer abcdefgh12345678",
        "authorization: basic dXNlcjpwYXNz",
        "curl -H 'Bearer abcdefghijklmnop' https://x",
        "-----BEGIN PRIVATE KEY-----",
        "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.sig",
        "AKIAIOSFODNN7EXAMPLE",
        "x7Kq9vR2mN4pL8sT1wY6zB3cF5hJ0dG2aQ9e",
    ] {
        assert_eq!(check("text", refused), 3, "{refused:?} must be refused");
    }
    let pinned = format!("pinned {digest} in kustomization.yaml");
    for accepted in [
        "the pod is Running and /readyz returned 200",
        pinned.as_str(),
        "tenant 0f0e0d0c-0b0a-4908-8706-050403020100 signed in",
        "/home/owner/m8-evidence/step-7-status.txt",
        "",
    ] {
        assert_eq!(check("text", accepted), 0, "{accepted:?} must be accepted");
    }
    assert_eq!(check("hash", &digest), 0, "a sha256 digest is accepted");
    assert_eq!(
        check("hash", &"b2".repeat(32)),
        0,
        "a bare sha256 is accepted"
    );
    for bad in ["sha256:abc", "latest", "SHA256:ABCD", "password=x"] {
        assert_eq!(check("hash", bad), 4, "{bad:?} is not a digest");
    }
    for ok in ["pass", "fail", "skip"] {
        assert_eq!(check("status", ok), 0);
    }
    for bad in ["yes", "done", "PASS", "pass ", ""] {
        assert_eq!(check("status", bad), 4, "{bad:?} is not a status");
    }
}

fn inventory(credentials: Value) -> Value {
    json!({"bucket": "truffles-prod-backups", "environment": "coder agent-workspace v1",
        "inventory_complete": true,
        "sources_checked": ["env", "aws_profiles", "rclone", "wrangler", "cf_cli", "mounted_secrets", "other"],
        "credentials": credentials})
}

fn r2(dir: &Path, name: &str, body: &str) -> (i32, String, String) {
    let inv = dir.join(format!("{name}.json"));
    std::fs::write(&inv, body).unwrap();
    let evidence = dir.join(format!("{name}.md"));
    let out: Output = Command::new("bash")
        .arg(script("check-r2-delete-rights.sh"))
        .arg(&inv)
        .arg(&evidence)
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let recorded = std::fs::read_to_string(&evidence).unwrap_or_default();
    (out.status.code().unwrap(), text, recorded)
}

/// Finding 3: the R2 helper is an offline validator. Only a complete, clean inventory
/// passes; incomplete or uninspected entries block; a delete-capable credential on the
/// bucket is a violation; malformed input or a secret-looking value is refused without
/// being echoed. The script contains no network or credential-reading command.
#[test]
fn the_r2_helper_validates_a_sanitized_inventory_offline() {
    let dir = temp("r2");
    let read = json!({"name": "CF_API_TOKEN", "kind": "cloudflare_api_token", "inspected": true,
        "permissions": ["Workers R2 Storage Read", "Zone Read"], "resources": ["account:*"]});
    let s3_read = json!({"name": "aws profile r2-read", "kind": "r2_access_key", "inspected": true,
        "permissions": ["s3:GetObject", "s3:ListBucket"], "resources": ["bucket:truffles-prod-backups"]});
    let cases: Vec<(&str, Value, i32)> = vec![
        ("clean", inventory(json!([read.clone(), s3_read])), 0),
        (
            "other-bucket",
            inventory(
                json!([{"name": "site uploads", "kind": "r2_access_key", "inspected": true,
                "permissions": ["s3:PutObject", "s3:DeleteObject"], "resources": ["bucket:site-uploads"]}]),
            ),
            0,
        ),
        (
            "write",
            inventory(
                json!([{"name": "CF_API_TOKEN", "kind": "cloudflare_api_token", "inspected": true,
                "permissions": ["Workers R2 Storage Write"], "resources": ["account:*"]}]),
            ),
            4,
        ),
        (
            "s3-delete",
            inventory(
                json!([{"name": "rclone remote backups", "kind": "r2_access_key", "inspected": true,
                "permissions": ["s3:GetObject", "s3:DeleteObject"], "resources": ["bucket:truffles-prod-backups"]}]),
            ),
            4,
        ),
        (
            "admin",
            inventory(
                json!([{"name": "CF_GLOBAL", "kind": "cloudflare_api_token", "inspected": true,
                "permissions": ["Super Administrator"], "resources": ["*"]}]),
            ),
            4,
        ),
        (
            "uninspected",
            inventory(
                json!([{"name": "CF_API_TOKEN", "kind": "cloudflare_api_token", "inspected": false}]),
            ),
            3,
        ),
        (
            "sources",
            {
                let mut v = inventory(json!([read.clone()]));
                v["sources_checked"] = json!(["env", "aws_profiles"]);
                v
            },
            3,
        ),
        (
            "not-complete",
            {
                let mut v = inventory(json!([]));
                v["inventory_complete"] = json!(false);
                v
            },
            3,
        ),
        // M8 review 2 finding 4: the schema is strict. Unknown permissions, scopes, kinds
        // and keys block (3); a modify permission on any scope that covers the bucket is a
        // violation (4), including object ARNs, wildcards and account-wide grants.
        (
            "unknown-permission",
            inventory(
                json!([{"name": "CF_API_TOKEN", "kind": "cloudflare_api_token", "inspected": true,
                "permissions": ["Some Future Permission"], "resources": ["account:*"]}]),
            ),
            3,
        ),
        (
            "arn-object-delete",
            inventory(
                json!([{"name": "r2 key", "kind": "r2_access_key", "inspected": true,
                "permissions": ["s3:DeleteObject"], "resources": ["arn:aws:s3:::truffles-prod-backups/*"]}]),
            ),
            4,
        ),
        (
            "arn-wildcard-put",
            inventory(
                json!([{"name": "r2 key", "kind": "r2_access_key", "inspected": true,
                "permissions": ["s3:PutObject"], "resources": ["arn:aws:s3:::truffles-*"]}]),
            ),
            4,
        ),
        (
            "arn-star",
            inventory(
                json!([{"name": "r2 key", "kind": "r2_access_key", "inspected": true,
                "permissions": ["s3:*"], "resources": ["arn:aws:s3:::*"]}]),
            ),
            4,
        ),
        (
            "unknown-s3-action",
            inventory(
                json!([{"name": "r2 key", "kind": "r2_access_key", "inspected": true,
                "permissions": ["s3:FrobnicateObject"], "resources": ["bucket:other"]}]),
            ),
            3,
        ),
        (
            "unknown-scope",
            inventory(
                json!([{"name": "r2 key", "kind": "r2_access_key", "inspected": true,
                "permissions": ["s3:GetObject"], "resources": ["gcs://bucket"]}]),
            ),
            3,
        ),
        (
            "unknown-kind",
            inventory(
                json!([{"name": "x", "kind": "mystery_token", "inspected": true,
                "permissions": ["none"], "resources": ["none"]}]),
            ),
            3,
        ),
        (
            "extra-key",
            inventory(
                json!([{"name": "x", "kind": "environment_secret", "inspected": true,
                "permissions": ["none"], "resources": ["none"], "scope_notes": "n/a"}]),
            ),
            3,
        ),
        (
            "none-holder",
            inventory(
                json!([{"name": "GITHUB_TOKEN", "kind": "environment_secret", "inspected": true,
                "permissions": ["none"], "resources": ["none"]}]),
            ),
            0,
        ),
        (
            "missing-field",
            {
                let mut v = inventory(json!([]));
                v.as_object_mut().unwrap().remove("sources_checked");
                v
            },
            2,
        ),
    ];
    for (name, body, want) in cases {
        let (code, text, recorded) = r2(&dir, name, &body.to_string());
        assert_eq!(code, want, "{name}: {text}");
        if want != 2 {
            assert!(recorded.contains("Verdict:"), "{name}: evidence written");
            let mode = std::fs::metadata(dir.join(format!("{name}.md")))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{name}: evidence is 0600");
        }
    }
    // A secret-looking value is refused and never echoed or recorded.
    let secret = "x7Kq9vR2mN4pL8sT1wY6zB3cF5hJ0dG2aQ9e";
    let mut leaky = inventory(json!([read]));
    leaky["credentials"][0]["note"] = json!(format!("value {secret}"));
    let (code, text, recorded) = r2(&dir, "secret", &leaky.to_string());
    assert_eq!(code, 2, "{text}");
    assert!(!text.contains(secret) && !recorded.contains(secret));
    let (code, _, _) = r2(&dir, "malformed", "{\"bucket\": [");
    assert_eq!(code, 2);
    // Offline: no network client, no environment or credential-file reads.
    let source = std::fs::read_to_string(script("check-r2-delete-rights.sh")).unwrap();
    let code_lines: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "curl",
        "wget",
        "env |",
        "printenv",
        ".aws/credentials",
        "rclone.conf",
        "read -r -s",
    ] {
        assert!(!code_lines.contains(forbidden), "{forbidden} in the helper");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Mock `cargo`, `rustc` and `rustup`: `mode` is ok (build both binaries), fail (exit 101)
/// or nobin (succeed without binaries); `installed` lists the installed Rust targets.
fn shims(dir: &Path, target_dir: &Path, mode: &str, installed: &[&str]) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let write = |name: &str, body: String| {
        let p = bin.join(name);
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    };
    write(
        "rustc",
        "#!/bin/sh\ncase \"$1\" in -vV) echo 'rustc 1.99.0 (mock)'; echo 'host: x86_64-unknown-linux-gnu';; *) echo 'rustc 1.99.0 (mock)';; esac\n".into(),
    );
    write(
        "rustup",
        format!("#!/bin/sh\nprintf '%s\\n' {}\n", installed.join(" ")),
    );
    write(
        "cargo",
        format!(
            r#"#!/bin/sh
case "$1" in
  metadata) echo '{{"target_directory": "{td}"}}' ;;
  --version) echo 'cargo 1.99.0 (mock)' ;;
  build)
    t=""; prev=""
    for a in "$@"; do [ "$prev" = "--target" ] && t="$a"; prev="$a"; done
    case "{mode}" in
      fail) echo "error[E0001]: mock compile failure" >&2; exit 101 ;;
      nobin) exit 0 ;;
      ok) mkdir -p "{td}/$t/release"; echo "airs $t" > "{td}/$t/release/airs"; echo "mcp $t" > "{td}/$t/release/airs-mcp" ;;
    esac ;;
esac
"#,
            td = target_dir.display(),
            mode = mode
        ),
    );
    bin
}

fn build_hashes(mode: &str, installed: &[&str]) -> (i32, PathBuf, String) {
    let dir = temp(&format!("bh-{mode}"));
    let target_dir = dir.join("target");
    let bin = shims(&dir, &target_dir, mode, installed);
    let out_dir = dir.join("out");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new("bash")
        .arg(script("build-hashes.sh"))
        .arg(&out_dir)
        .env("PATH", path)
        .output()
        .unwrap();
    let build = std::fs::read_to_string(out_dir.join("BUILD.txt")).unwrap_or_default();
    (out.status.code().unwrap(), out_dir, build)
}

const ALL: [&str; 3] = [
    "x86_64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
];

/// Finding 8: build failures are fatal, a missing required target blocks with its own
/// exit code, and the whole matrix (with the user-run platforms) is always recorded.
#[test]
fn build_hashes_fails_closed_and_records_the_matrix() {
    // Every build fails: exit 1 and no BIN-SHA256SUMS at all.
    let (code, out, build) = build_hashes("fail", &ALL);
    assert_eq!(code, 1, "{build}");
    assert!(
        !out.join("BIN-SHA256SUMS").exists(),
        "no (empty) success file"
    );
    assert!(build.contains("FAILED: x86_64-unknown-linux-gnu"));
    assert!(build.contains("release-qualification: blocked (a build failed)"));
    // A build that produces no binary is a failure too.
    let (code, out, build) = build_hashes("nobin", &ALL);
    assert_eq!(code, 1, "{build}");
    assert!(!out.join("BIN-SHA256SUMS").exists());
    // Every required target builds: exit 0, six hashes, user-run platforms pending.
    let (code, out, build) = build_hashes("ok", &ALL);
    assert_eq!(code, 0, "{build}");
    let sums = std::fs::read_to_string(out.join("BIN-SHA256SUMS")).unwrap();
    assert_eq!(sums.lines().count(), 6, "{sums}");
    for t in ALL {
        assert!(build.contains(&format!("built: {t}")), "{build}");
    }
    assert!(build.contains("local-evidence: complete"));
    assert!(build.contains("user-run: aarch64-apple-darwin"));
    assert!(build.contains(
        "release-qualification: blocked (pending: aarch64-apple-darwin, airs-mcp container image)"
    ));
    // A required target not installed: recorded as missing, exit 3, never as built.
    let (code, out, build) = build_hashes("ok", &["x86_64-unknown-linux-gnu"]);
    assert_eq!(code, 3, "{build}");
    assert!(build.contains("missing: x86_64-unknown-linux-musl (target not installed here)"));
    assert!(build.contains("missing: aarch64-unknown-linux-musl"));
    assert!(!build.contains("built: x86_64-unknown-linux-musl"));
    assert!(build.contains("local-evidence: incomplete"));
    let sums = std::fs::read_to_string(out.join("BIN-SHA256SUMS")).unwrap();
    assert_eq!(sums.lines().count(), 2, "host hashes recorded");
}

/// A local talos-cluster checkout with the airs-mcp DNS wiring in place.
fn talos_fixture(dir: &Path) -> PathBuf {
    let t = dir.join("talos");
    let host = "mcp-backend-airs.goshawks.io";
    for (rel, body) in [
        (
            "coredns/configmap.yaml",
            format!("    rewrite name exact {host} airs-mcp.airs-mcp.svc.cluster.local\n"),
        ),
        (
            "aigw-goshawks/values.yaml",
            format!("    TRUSTED_CUSTOM_HOSTS: \"a.example,{host}\"\n"),
        ),
        (
            "pihole/dns1/values.yaml",
            "    - host-record=mcp.goshawks.io,192.168.1.0\n    - host-record=gateway.goshawks.io,192.168.1.0\n".into(),
        ),
        (
            "pihole/dns2/values.yaml",
            "    - host-record=mcp.goshawks.io,192.168.1.0\n    - host-record=gateway.goshawks.io,192.168.1.0\n".into(),
        ),
        ("talos/cluster-issuer.yaml", "      - dns01:\n          cloudflare: {}\n".into()),
    ] {
        let p = t.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
    t
}

/// The service CLIs the derive helper must never run.
const SERVICE_CLIS: [&str; 7] = ["az", "kubectl", "cf", "airs", "mise", "curl", "wget"];

/// Service-CLI stubs: each writes a marker and exits 99. The derive helper is offline
/// only (M8 review 3), so no stub may ever run.
fn service_stubs(dir: &Path) -> PathBuf {
    let bin = dir.join("stubs");
    std::fs::create_dir_all(&bin).unwrap();
    for name in SERVICE_CLIS {
        let body = format!(
            "#!/bin/sh\necho \"{name} $*\" >> '{marker}'\nexit 99\n",
            marker = dir.join("INVOKED").display()
        );
        let p = bin.join(name);
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}

/// Run derive-config.sh with `answers` on stdin; returns (exit code, combined output).
fn derive(dir: &Path, args: &[&str], answers: &[&str], stubs: &Path) -> (i32, String) {
    derive_with(dir, args, answers, stubs, &[])
}

/// [`derive`] with environment overrides: `Some` sets a variable, `None` removes it.
fn derive_with(
    dir: &Path,
    args: &[&str],
    answers: &[&str],
    stubs: &Path,
    envs: &[(&str, Option<&str>)],
) -> (i32, String) {
    let mut cmd = Command::new("bash");
    cmd.arg(script("derive-config.sh"))
        .args(args)
        .arg("--talos-cluster")
        .arg(talos_fixture(dir))
        .arg("--spike")
        .arg(dir.join("no-spike.toml"))
        .arg("--out")
        .arg(dir.join("out/airs-mcp.local.toml"))
        .env(
            "PATH",
            format!("{}:/usr/local/bin:/usr/bin:/bin", stubs.display()),
        )
        .env("AIRS_MCP_BIN", env!("CARGO_BIN_EXE_airs-mcp"))
        .env("HOME", dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        match v {
            Some(v) => cmd.env(k, v),
            None => cmd.env_remove(k),
        };
    }
    let mut child = cmd.spawn().unwrap();
    let input: String = answers.iter().map(|a| format!("{a}\n")).collect();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// The 20 answers, in prompt order (an empty answer accepts the suggestion).
fn answers(revision: &str) -> Vec<String> {
    let cc = format!("pk_{}", "a1".repeat(32));
    [
        "",                                              // backend hostname (default)
        "11111111-2222-4333-8444-555555555551",          // Entra tenant
        "11111111-2222-4333-8444-555555555552",          // API app
        "11111111-2222-4333-8444-555555555553",          // gateway client
        "11111111-2222-4333-8444-555555555554",          // owners
        "portkey-mcp-gateway",                           // assertion issuer
        "https://mcp.goshawks.io/.well-known/jwks.json", // JWKS
        "org-1",                                         // assertion audiences
        "org-1",                                         // organisation IDs
        "ws-1",                                          // workspace IDs
        "1234567890",                                    // TSG
        "11111111-2222-4333-8444-555555555556",          // management revision
        "",                                              // scanner name (monitor)
        "app-test",                                      // scanner application ID
        "",                                              // scanner endpoint (default)
        "11111111-2222-4333-8444-555555555557",          // scanner revision
        "owner@corp.test",                               // caller subject
        "11111111-2222-4333-8444-555555555558",          // airs CLI client
        cc.as_str(),                                     // Claude Code client
        revision,                                        // revision label
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// M8 review 2 finding 3: the derive helper is offline by default. With service-CLI stubs
/// that fail if invoked, it runs none of them, writes a 0600 configuration that the real
/// service parser (`airs-mcp check-config`) accepts, and never creates DNS records. The
/// minor: a revision with quotes and a backslash is serialized safely and round-trips.
#[test]
fn derive_config_is_offline_by_default_and_writes_a_valid_configuration() {
    let dir = temp("derive");
    let stubs = service_stubs(&dir);
    let revision = r#"prod "quoted" \ 1"#;
    let a = answers(revision);
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, text) = derive(&dir, &[], &refs, &stubs);
    assert_eq!(code, 0, "{text}");
    assert!(
        !dir.join("INVOKED").exists(),
        "a service CLI ran in offline mode: {}",
        std::fs::read_to_string(dir.join("INVOKED")).unwrap_or_default()
    );
    let out = dir.join("out/airs-mcp.local.toml");
    let mode = std::fs::metadata(&out).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let check = Command::new(env!("CARGO_BIN_EXE_airs-mcp"))
        .args(["check-config", "--config"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let parsed: Value =
        serde_json::from_slice(&check.stdout).expect("check-config prints a JSON event");
    assert_eq!(
        parsed["revision"], revision,
        "the quoted revision round-trips"
    );
    assert!(text.contains("never creates DNS records"), "{text}");
    // No temporary file is left behind.
    let leftovers: Vec<_> = std::fs::read_dir(dir.join("out"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".airs-mcp.toml.")
        })
        .collect();
    assert!(leftovers.is_empty());
    // The script holds no record-creating call (only the printed hint).
    let source = std::fs::read_to_string(script("derive-config.sh")).unwrap();
    for line in source.lines().filter(|l| l.contains("records create")) {
        let l = line.trim_start();
        assert!(
            l.starts_with("dim ") || l.starts_with('#'),
            "a records-create call outside a printed hint: {line}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// M8 review 3 finding 1: the helper is offline under every option. With failing stubs for
/// every service CLI on PATH, the supported options (`--help`, and the `--talos-cluster`,
/// `--spike` and `--out` the runner always passes) run none of them, and the removed
/// `--run-lookups` is a usage error (exit 2) that runs nothing either.
#[test]
fn derive_config_runs_no_service_cli_under_any_option() {
    let dir = temp("derive-options");
    let stubs = service_stubs(&dir);
    let a = answers("prod-1");
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, text) = derive(&dir, &[], &refs, &stubs);
    assert_eq!(code, 0, "{text}");
    let (code, text) = derive(&dir, &["--help"], &[], &stubs);
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("OFFLINE ONLY"), "{text}");
    let (code, text) = derive(&dir, &["--run-lookups"], &refs, &stubs);
    assert_eq!(code, 2, "{text}");
    assert!(text.contains("offline only"), "{text}");
    assert!(
        !dir.join("INVOKED").exists(),
        "a service CLI ran: {}",
        std::fs::read_to_string(dir.join("INVOKED")).unwrap_or_default()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `line` with every single- and double-quoted span removed (backslash escapes kept out
/// of double quotes), so only shell code remains.
fn unquoted(line: &str) -> String {
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in line.chars() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if q == '"' && c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '#' {
                    break;
                }
                if c == '\'' || c == '"' {
                    quote = Some(c);
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// M8 review 3 finding 1, statically: outside comments and quoted (printed) text, the
/// script's code never names a service CLI in a command position (start of a command,
/// after `$(`, a pipe, `;`, `&&` or `||`, or as an argument of `command`/`exec`/`env`).
#[test]
fn the_derive_helper_source_invokes_no_service_cli() {
    let source = std::fs::read_to_string(script("derive-config.sh")).unwrap();
    let mut offenders = Vec::new();
    let mut heredoc: Option<String> = None;
    for (n, line) in source.lines().enumerate() {
        if let Some(end) = &heredoc {
            if line.trim() == end {
                heredoc = None;
            }
            continue;
        }
        let code = unquoted(line);
        if let Some(pos) = code.find("<<") {
            let tag: String = line[line.find("<<").unwrap_or(pos) + 2..]
                .trim_start_matches('-')
                .trim()
                .trim_matches(|c| c == '\'' || c == '"')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !tag.is_empty() {
                heredoc = Some(tag);
            }
        }
        let separated = code
            .replace("$(", " ; ")
            .replace("&&", " ; ")
            .replace("||", " ; ")
            .replace(['|', '(', '`'], " ; ");
        for segment in separated.split(';') {
            let mut words = segment.split_whitespace();
            let mut first = words.next();
            while matches!(
                first,
                Some("command" | "exec" | "env" | "then" | "do" | "else" | "!")
            ) {
                first = words.next();
            }
            if let Some(w) = first
                && SERVICE_CLIS.contains(&w)
            {
                offenders.push(format!("{}: {line}", n + 1));
            }
        }
    }
    assert!(offenders.is_empty(), "service CLIs invoked: {offenders:#?}");
}

/// The minor: output that does not validate is never written. A management revision that
/// is a well-formed GUID but not version 4 passes the helper's own check, and the service
/// parser refuses it: the helper exits 1 and leaves no file.
#[test]
fn derive_config_writes_nothing_when_the_service_parser_refuses_it() {
    let dir = temp("derive-invalid");
    let stubs = service_stubs(&dir);
    let mut a = answers("prod-1");
    a[11] = "11111111-2222-1333-8444-555555555556".into();
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    let (code, text) = derive(&dir, &[], &refs, &stubs);
    assert_eq!(code, 1, "{text}");
    assert!(!dir.join("out/airs-mcp.local.toml").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// M8 review 6 finding 2: when python3 has no TOML module, `airs-mcp check-config` is the
/// only validator, so it must actually run before anything is published. A validator
/// path that is unset, missing or not executable must refuse (exit 1, no file); a real
/// validator must reject invalid input and accept valid input. `AIRS_DERIVE_ASSUME_NO_TOML`
/// forces the no-parser path on machines whose python3 has `tomllib`.
#[test]
fn derive_config_requires_a_working_validator_without_a_toml_parser() {
    let dir = temp("derive-validator");
    let stubs = service_stubs(&dir);
    let not_exec = dir.join("not-executable");
    std::fs::write(&not_exec, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&not_exec, std::fs::Permissions::from_mode(0o600)).unwrap();
    let missing = dir.join("does-not-exist");
    let valid = answers("prod-1");
    let mut invalid = answers("prod-1");
    invalid[11] = "11111111-2222-1333-8444-555555555556".into();
    let out = dir.join("out/airs-mcp.local.toml");
    // `want` is Some((exit code, file written)) where the outcome is fixed. With
    // AIRS_MCP_BIN unset the helper may still find a built `target/*/airs-mcp` (it does
    // under `cargo test`), so that case only checks the invariant below.
    /// (name, AIRS_MCP_BIN, answers, fixed outcome).
    type ValidatorCase<'a> = (&'a str, Option<&'a str>, &'a [String], Option<(i32, bool)>);
    let cases: [ValidatorCase; 5] = [
        ("unset", None, &valid, None),
        (
            "missing",
            Some(missing.to_str().unwrap()),
            &valid,
            Some((1, false)),
        ),
        (
            "not executable",
            Some(not_exec.to_str().unwrap()),
            &valid,
            Some((1, false)),
        ),
        (
            "rejecting",
            Some(env!("CARGO_BIN_EXE_airs-mcp")),
            &invalid,
            Some((1, false)),
        ),
        (
            "accepting",
            Some(env!("CARGO_BIN_EXE_airs-mcp")),
            &valid,
            Some((0, true)),
        ),
    ];
    for (name, bin, answers, want) in cases {
        let _ = std::fs::remove_file(&out);
        let refs: Vec<&str> = answers.iter().map(String::as_str).collect();
        let (code, text) = derive_with(
            &dir,
            &[],
            &refs,
            &stubs,
            &[
                ("AIRS_DERIVE_ASSUME_NO_TOML", Some("1")),
                ("AIRS_MCP_BIN", bin),
            ],
        );
        assert!(
            text.contains("no TOML module"),
            "{name}: the no-parser path was taken: {text}"
        );
        // The invariant: without a TOML parser, a file exists only if the service parser
        // ran and accepted it; otherwise the helper refused with exit 1.
        if out.exists() {
            assert!(text.contains("check-config accepts it"), "{name}: {text}");
            assert_eq!(code, 0, "{name}: {text}");
        } else {
            assert_eq!(code, 1, "{name}: {text}");
        }
        if let Some((want_code, want_file)) = want {
            assert_eq!(code, want_code, "{name}: {text}");
            assert_eq!(out.exists(), want_file, "{name}: output presence");
        }
        if matches!(name, "missing" | "not executable") {
            // An explicit validator that cannot run is an error, never replaced by a
            // discovered binary.
            assert!(text.contains("not an executable file"), "{name}: {text}");
            assert!(!text.contains("check-config accepts it"), "{name}: {text}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Runbook §12 step 5 and checklist step 12 (M8 review 4 finding 4): the low-space
/// filler is a *sibling* of the data directory (`/data/qual-filler` beside `/data/airs`),
/// created 0700 for UID 65532 by the qualification overlay's init container, because the
/// volume root is root-owned 0755. Two code facts make the sibling the right place, and
/// this pins both:
/// - admission checks the data directory and its ancestors only (`storage::admit`), so a
///   sibling is invisible to it, and a 0700 sibling owned by the service user under a
///   0755 root is itself admitted without any ancestor change;
/// - the disk reserve sums the control directory and the history root
///   (`metrics::store_bytes_checked`), so a filler *inside* the data directory would be
///   counted as database bytes and inflate the reserve, while the sibling changes only
///   the volume's free space.
#[test]
fn the_qualification_filler_is_a_sibling_the_reserve_never_counts() {
    use std::os::unix::fs::DirBuilderExt;
    let root = temp("qual-filler");
    // The volume-root shape: 0755. The test cannot be root while creating children as
    // 65532; that the init container gives the sibling to the service user is proved
    // in the owner's step 6.
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    let data = root.join("airs");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&data)
        .unwrap();
    for sub in ["control", "history", "history/cloud-a"] {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(data.join(sub))
            .unwrap();
    }
    std::fs::write(data.join("control/control.sqlite3"), vec![1u8; 4096]).unwrap();
    std::fs::write(
        data.join("history/cloud-a/archive.sqlite3"),
        vec![2u8; 8192],
    )
    .unwrap();
    let filler_dir = root.join("qual-filler");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&filler_dir)
        .unwrap();
    std::fs::write(filler_dir.join("fill"), vec![0u8; 1 << 20]).unwrap();

    // The data directory is admitted with the sibling present, and the sibling itself
    // passes the same ancestor rules.
    let admitted = airs_mcp::storage::admit(&data).expect("the data directory is admitted");
    assert_eq!(admitted.path, data.canonicalize().unwrap());
    airs_mcp::storage::admit(&filler_dir).expect("the sibling is admitted unchanged");

    // The reserve's inputs ignore the sibling entirely.
    let control = airs_mcp::metrics::store_bytes_checked(&data.join("control")).unwrap();
    let history = airs_mcp::metrics::store_bytes_checked(&data.join("history")).unwrap();
    assert_eq!(
        (control, history),
        (4096, 8192),
        "the filler is not counted"
    );

    // A filler inside the stores would be counted as database bytes: the reason the
    // filler must not live there.
    std::fs::write(data.join("history/cloud-a/fill"), vec![0u8; 1 << 20]).unwrap();
    let inflated = airs_mcp::metrics::store_bytes_checked(&data.join("history")).unwrap();
    assert_eq!(
        inflated,
        8192 + (1 << 20),
        "a filler inside the stores inflates the reserve"
    );

    let _ = std::fs::remove_dir_all(&root);
}
