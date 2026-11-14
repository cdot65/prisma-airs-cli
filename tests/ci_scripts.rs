//! The CI helper scripts fail safe (final M8 review 3).
//!
//! `scripts/ci/classify-change.sh` decides whether CI may skip the Rust steps; it must print
//! `rust=false` only for a change that touches documentation alone, and `rust=true` for
//! renames out of Rust paths, long path lists, a missing or unrelated base and empty diffs.
//! `scripts/ci/ci-status-ok.py` decides whether `release.sh` trusts CI; it must accept only a
//! combined status in which linux, musl and docs-site all reported success.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn script(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts/ci")
        .join(name)
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Exercise the actual install step with synthetic downloads. Corrupt or failed
/// candidates must never reach extraction or advertise a tool on GITHUB_PATH.
#[cfg(unix)]
#[test]
fn zig_downloads_verify_before_extracting_and_fail_over_safely() {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::PermissionsExt;

    let workflow = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".forgejo/workflows/ci.yml"),
    )
    .unwrap();
    let step = workflow
        .split_once("- name: Install zig\n")
        .unwrap()
        .1
        .split_once("run: |\n")
        .unwrap()
        .1
        .lines()
        .take_while(|line| line.is_empty() || line.starts_with("          "))
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    let pinned = step
        .split_once("x86_64) arch=x86_64; sum=")
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap();
    let bytes = b"synthetic Zig archive";
    let digest = format!("{:x}", Sha256::digest(bytes));
    let arm_pin = step
        .split_once("aarch64 | arm64) arch=aarch64; sum=")
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap();
    let step = step.replace(pinned, &digest).replace(arm_pin, &digest);
    // Resolve before the mock shadows PATH, avoiding recursive self-invocation.
    let verifier = Command::new("sh")
        .args(["-c", "command -v sha256sum || command -v shasum"])
        .output()
        .unwrap();
    assert!(verifier.status.success(), "a real SHA-256 tool is required");
    let verifier = String::from_utf8(verifier.stdout).unwrap();
    let executable = |path: &Path, body: &str| {
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    };
    for arch in ["x86_64", "aarch64"] {
        for mode in [
            "good",
            "corrupt-first",
            "failed-first",
            "all-corrupt",
            "all-failed",
        ] {
            let dir = repo(&format!("zig-{arch}-{mode}"));
            let name = format!("zig-{arch}-linux-0.17.0");
            let archive = format!("{name}.tar.xz");
            let bin = dir.join("bin");
            let temp = dir.join("runner");
            fs::create_dir_all(&bin).unwrap();
            fs::create_dir_all(temp.join(&name)).unwrap();
            fs::write(dir.join("good"), bytes).unwrap();
            executable(
                &bin.join("uname"),
                "#!/bin/sh\nprintf '%s\\n' \"$TEST_ARCH\"\n",
            );
            executable(
                &bin.join("sha256sum"),
                r#"#!/bin/sh
case "$REAL_SHA256" in
  */sha256sum) exec "$REAL_SHA256" "$@" ;;
  *) exec "$REAL_SHA256" -a 256 "$@" ;;
esac
"#,
            );
            executable(
                &bin.join("tar"),
                "#!/bin/sh\nprintf 'extract\\n' >> \"$TEST_DIR/extractions\"\n",
            );
            executable(
                &temp.join(format!("{name}/zig")),
                "#!/bin/sh\nprintf '0.17.0\\n'\n",
            );
            executable(
                &bin.join("curl"),
                r#"#!/bin/sh
set -eu
output=
connect=
limit=
proto=
redirect=
tls=false
quiet_config=false
fail_http=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) output=$2; shift 2 ;;
    --proto) proto=$2; shift 2 ;;
    --proto-redir) redirect=$2; shift 2 ;;
    --tlsv1.2) tls=true; shift ;;
    -q) quiet_config=true; shift ;;
    -sSfL) fail_http=true; shift ;;
    --connect-timeout) connect=$2; shift 2 ;;
    --max-time) limit=$2; shift 2 ;;
    *) url=$1; shift ;;
  esac
done
if [ "$connect" != 15 ] || [ "$limit" != 180 ] \
  || [ "$proto" != =https ] || [ "$redirect" != =https ] \
  || [ "$tls" != true ] || [ "$quiet_config" != true ] || [ "$fail_http" != true ]; then
  exit 99
fi
case "$url" in https://*) ;; *) exit 99 ;; esac
printf '%s\n' "$url" >> "$TEST_DIR/requests"
case "$TEST_MODE:$url" in
  all-failed:*|failed-first:*zig.linus.dev*)
    printf 'a truncated archive longer than the good fixture' > "$output"
    exit 28 ;;
  all-corrupt:*|corrupt-first:*zig.linus.dev*) printf 'corrupt' > "$output" ;;
  *) cp "$TEST_DIR/good" "$output" ;;
esac
"#,
            );
            let out = Command::new("bash")
                .arg("-c")
                .arg(&step)
                .env_clear()
                .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
                .env("RUNNER_TEMP", &temp)
                .env("GITHUB_PATH", dir.join("paths"))
                .env("ZIG_VERSION", "0.17.0")
                .env("TEST_DIR", &dir)
                .env("TEST_MODE", mode)
                .env("TEST_ARCH", arch)
                .env("REAL_SHA256", verifier.trim())
                .output()
                .unwrap();
            let succeeds = !mode.starts_with("all-");
            assert_eq!(
                out.status.success(),
                succeeds,
                "{mode}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let requests = fs::read_to_string(dir.join("requests")).unwrap();
            let requests = requests.lines().collect::<Vec<_>>();
            assert_eq!(requests.len(), if mode == "good" { 1 } else { 2 });
            assert_eq!(
                requests[0],
                format!("https://zig.linus.dev/zig/{archive}?source=ci")
            );
            if requests.len() == 2 {
                assert_eq!(
                    requests[1],
                    format!("https://ziglang.org/download/0.17.0/{archive}")
                );
            }
            assert!(!temp.join(format!("{archive}.part")).exists());
            if succeeds {
                assert_eq!(fs::read(temp.join(&archive)).unwrap(), bytes);
                assert_eq!(
                    fs::read_to_string(dir.join("extractions")).unwrap(),
                    "extract\n"
                );
                assert!(dir.join("paths").exists());
            } else {
                assert!(!temp.join(&archive).exists());
                assert!(!dir.join("extractions").exists());
                assert!(!dir.join("paths").exists());
            }
        }
    }
}

/// A fresh git repository with one commit holding `src/lib.rs` and `README.md`.
fn repo(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ci-scripts-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/lib.rs"), "pub fn f() {}\n").unwrap();
    fs::write(dir.join("README.md"), "# readme\n").unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    commit(&dir, "base");
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn commit(dir: &Path, message: &str) -> String {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

fn classify(dir: &Path, base: &str) -> String {
    let out = Command::new(script("classify-change.sh"))
        .arg(base)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "classifier failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[test]
fn a_docs_only_change_skips_the_rust_steps() {
    let dir = repo("docs");
    let base = git(&dir, &["rev-parse", "HEAD"]);
    fs::create_dir_all(dir.join("docs-site/docs")).unwrap();
    fs::write(dir.join("docs-site/docs/a.md"), "a\n").unwrap();
    fs::create_dir_all(dir.join("docs")).unwrap();
    fs::write(dir.join("docs/b.txt"), "b\n").unwrap();
    fs::write(dir.join("README.md"), "# changed\n").unwrap();
    commit(&dir, "docs");
    assert_eq!(classify(&dir, &base), "rust=false");
}

#[test]
fn any_other_path_runs_the_rust_steps() {
    let dir = repo("code");
    let base = git(&dir, &["rev-parse", "HEAD"]);
    fs::write(dir.join("src/lib.rs"), "pub fn g() {}\n").unwrap();
    commit(&dir, "code");
    assert_eq!(classify(&dir, &base), "rust=true");
}

#[test]
fn moving_a_rust_file_into_docs_runs_the_rust_steps() {
    let dir = repo("rename");
    let base = git(&dir, &["rev-parse", "HEAD"]);
    fs::create_dir_all(dir.join("docs")).unwrap();
    git(&dir, &["mv", "src/lib.rs", "docs/lib.rs"]);
    commit(&dir, "move");
    // With rename detection the diff would list only docs/lib.rs.
    let renamed = git(&dir, &["diff", "--name-only", "-M", &base, "HEAD"]);
    assert_eq!(
        renamed, "docs/lib.rs",
        "the fixture must be a detected rename"
    );
    assert_eq!(classify(&dir, &base), "rust=true");
}

#[test]
fn a_code_path_after_many_docs_paths_runs_the_rust_steps() {
    let dir = repo("many");
    let base = git(&dir, &["rev-parse", "HEAD"]);
    fs::create_dir_all(dir.join("docs/many")).unwrap();
    for i in 0..5000 {
        fs::write(
            dir.join(format!(
                "docs/many/{i:05}-a-rather-long-name-to-fill-pipe-buffers.md"
            )),
            "x\n",
        )
        .unwrap();
    }
    fs::write(dir.join("zz-build.rs"), "fn main() {}\n").unwrap();
    commit(&dir, "many docs and a build script");
    assert_eq!(classify(&dir, &base), "rust=true");
}

#[test]
fn an_odd_path_name_is_still_classified_by_its_whole_name() {
    let dir = repo("odd");
    let base = git(&dir, &["rev-parse", "HEAD"]);
    fs::write(dir.join("src/a\nb.md"), "x\n").unwrap();
    fs::write(dir.join("src/new.rs"), "x\n").unwrap();
    commit(&dir, "odd");
    assert_eq!(classify(&dir, &base), "rust=true");
}

#[test]
fn an_unusable_base_runs_the_rust_steps() {
    let dir = repo("base");
    fs::write(dir.join("README.md"), "# changed\n").unwrap();
    commit(&dir, "docs");
    assert_eq!(classify(&dir, ""), "rust=true", "empty base");
    assert_eq!(
        classify(&dir, &"0".repeat(40)),
        "rust=true",
        "all-zero base (a new branch push)"
    );
    assert_eq!(
        classify(&dir, &"1".repeat(40)),
        "rust=true",
        "unknown commit"
    );
    // An unrelated history: no merge base.
    git(&dir, &["checkout", "-q", "--orphan", "other"]);
    git(&dir, &["rm", "-rq", "--cached", "."]);
    fs::write(dir.join("README.md"), "# other\n").unwrap();
    let orphan = commit(&dir, "orphan");
    git(&dir, &["checkout", "-qf", "main"]);
    assert_eq!(classify(&dir, &orphan), "rust=true", "no common ancestor");
}

#[test]
fn an_empty_diff_runs_the_rust_steps() {
    let dir = repo("empty");
    let base = git(&dir, &["rev-parse", "HEAD"]);
    commit(&dir, "empty");
    assert_eq!(classify(&dir, &base), "rust=true");
}

/// Fields whose values `jq` tolerates but RFC 8259 forbids.
const NON_STANDARD_FIELDS: [&str; 5] = [
    "\"total_count\":NaN",
    "\"total_count\":Infinity",
    "\"total_count\":01",
    "\"total_count\":+1",
    "\"total_count\":1.",
];

/// `valid` with an extra string field holding a byte that is not UTF-8.
fn invalid_utf8(valid: &str) -> Vec<u8> {
    let mut bytes = b"{\"note\":\"\xff\",".to_vec();
    bytes.extend_from_slice(&valid.as_bytes()[1..]);
    bytes
}

fn status_ok(json: &str) -> bool {
    status_ok_bytes(json.as_bytes())
}

fn status_ok_bytes(json: &[u8]) -> bool {
    let mut child = Command::new("python3")
        .arg("-I")
        .arg(script("ci-status-ok.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(json).unwrap();
    child.wait().unwrap().success()
}

fn combined(state: &str, statuses: &[(&str, &str)]) -> String {
    let list: Vec<String> = statuses
        .iter()
        .map(|(context, status)| format!(r#"{{"context":"{context}","status":"{status}"}}"#))
        .collect();
    format!(r#"{{"state":"{state}","statuses":[{}]}}"#, list.join(","))
}

#[test]
fn the_release_trusts_ci_only_when_every_required_job_passed() {
    let all = |suffix: &str, status: &str| {
        combined(
            "success",
            &[
                (&format!("CI / linux ({suffix})"), status),
                (&format!("CI / musl ({suffix})"), status),
                (&format!("CI / docs-site ({suffix})"), status),
            ],
        )
    };
    assert!(status_ok(&all("push", "success")));
    assert!(status_ok(&all("pull_request", "success")));
    let mixed = combined(
        "success",
        &[
            ("CI / linux (push)", "success"),
            ("CI / musl (pull_request)", "success"),
            ("CI / docs-site (push)", "success"),
        ],
    );
    assert!(status_ok(&mixed));

    assert!(!status_ok(&combined("success", &[])), "no statuses at all");
    let missing = combined(
        "success",
        &[
            ("CI / linux (push)", "success"),
            ("CI / docs-site (push)", "success"),
        ],
    );
    assert!(!status_ok(&missing), "musl missing");
    assert!(!status_ok(&all("push", "pending")), "pending");
    assert!(!status_ok(&all("push", "failure")), "failed");
    let one_failed = combined(
        "success",
        &[
            ("CI / linux (push)", "success"),
            ("CI / linux (pull_request)", "failure"),
            ("CI / musl (push)", "success"),
            ("CI / docs-site (push)", "success"),
        ],
    );
    assert!(!status_ok(&one_failed), "one context of a job failed");
    let other_workflow = combined(
        "success",
        &[
            ("Other / linux (push)", "success"),
            ("CI / musl (push)", "success"),
            ("CI / docs-site (push)", "success"),
        ],
    );
    assert!(
        !status_ok(&other_workflow),
        "a look-alike context does not count"
    );
    let aggregate =
        all("push", "success").replacen(r#""state":"success""#, r#""state":"pending""#, 1);
    assert!(!status_ok(&aggregate), "aggregate not success");
    assert!(!status_ok(""), "empty input");
    assert!(!status_ok(" \n"), "blank input");
    assert!(!status_ok("null"), "null");
    assert!(
        !status_ok(r#"{"state":"success","statuses":[1]}"#),
        "malformed entries"
    );
    assert!(!status_ok("not json"), "malformed");
    let valid = all("push", "success");
    assert!(status_ok(&valid), "the valid object on its own passes");
    assert!(!status_ok(&format!("{valid}not-json")), "trailing garbage");
    assert!(
        !status_ok(&format!("{valid}\nnot-json")),
        "a trailing garbage line"
    );
    assert!(
        !status_ok(&format!("{valid}{{\"state\":")),
        "truncated second object"
    );
    assert!(!status_ok(&format!("{valid}\n{valid}")), "two documents");
    assert!(!status_ok(&format!("{valid}\0")), "a trailing NUL");
    for field in NON_STANDARD_FIELDS {
        let bad = valid.replacen('{', &format!("{{{field},"), 1);
        assert!(!status_ok(&bad), "non-standard field: {field}");
    }
    assert!(
        status_ok(&valid.replacen('{', "{\"total_count\":3,", 1)),
        "an ordinary extra field is fine"
    );
    assert!(!status_ok_bytes(&invalid_utf8(&valid)), "invalid UTF-8");
    assert!(
        !status_ok(&format!("\u{feff}{valid}")),
        "a byte order mark is not JSON"
    );
    assert!(
        !status_ok(&valid.replacen('{', "{\"state\":\"failure\",", 1)),
        "a duplicate key"
    );
    assert!(
        !status_ok(&valid.replacen('{', "{\"note\":\"\\ud800\",", 1)),
        "an unpaired surrogate"
    );
    assert!(
        !status_ok(&valid.replacen(r#""state":"success""#, "\"state\":\"suc\0cess\"", 1)),
        "a NUL inside the aggregate state"
    );
    assert!(
        !status_ok(&valid.replacen("CI / linux", "CI / lin\0ux", 1)),
        "a NUL inside a required context"
    );
    assert!(!status_ok(r#"{"state":"success"}"#), "no statuses array");
    assert!(!status_ok(r#"[]"#), "wrong shape");
}

#[test]
fn release_sh_uses_the_strict_status_check() {
    let release =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/release.sh"))
            .unwrap();
    assert!(release.contains("python3 -I scripts/ci/ci-status-ok.py"));
    assert!(
        !release.contains("jq -r '.state // empty'"),
        "the aggregate-only check must be gone"
    );
    let ci = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".forgejo/workflows/ci.yml"),
    )
    .unwrap();
    assert_eq!(
        ci.matches("scripts/ci/classify-change.sh \"$base\"")
            .count(),
        2,
        "both jobs classify with the script"
    );
}

/// Runs the unchanged `ci_passed_for_head` from `release.sh` in `dir` with Forgejo stubbed:
/// the status of HEAD^2 is `second`, of any other commit `head`.
fn release_function_accepts_each(
    dir: &Path,
    head: impl AsRef<[u8]>,
    second: impl AsRef<[u8]>,
) -> bool {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::create_dir_all(dir.join("scripts/ci")).unwrap();
    fs::copy(
        script("ci-status-ok.py"),
        dir.join("scripts/ci/ci-status-ok.py"),
    )
    .unwrap();
    let head_file = dir.join(".git/status-head");
    let second_file = dir.join(".git/status-second");
    fs::write(&head_file, head).unwrap();
    fs::write(&second_file, second).unwrap();
    let body = r#"eval "$(awk '/^ci_passed_for_head\(\) \{/,/^\}/' "$RELEASE")"
API=stub
second="$(git rev-parse --quiet --verify 'HEAD^2^{commit}' || echo none)"
forgejo() { case "$1" in *"/commits/$second/"*) cat "$SECOND" ;; *) cat "$HEAD_STATUS" ;; esac; }
ci_passed_for_head"#;
    Command::new("bash")
        .args(["-c", body])
        .current_dir(dir)
        .env("RELEASE", root.join("scripts/release.sh"))
        .env("HEAD_STATUS", &head_file)
        .env("SECOND", &second_file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

fn release_function_accepts(dir: &Path, response: impl AsRef<[u8]>) -> bool {
    release_function_accepts_each(dir, response.as_ref(), response.as_ref())
}

#[test]
fn the_release_function_refuses_malformed_responses_and_other_trees() {
    let valid = combined(
        "success",
        &[
            ("CI / linux (pull_request)", "success"),
            ("CI / musl (pull_request)", "success"),
            ("CI / docs-site (pull_request)", "success"),
        ],
    );
    let dir = repo("release-fn");
    assert!(
        release_function_accepts(&dir, valid.as_bytes()),
        "HEAD passed CI"
    );
    for (name, bad) in [
        ("empty", String::new()),
        ("trailing NUL", format!("{valid}\0")),
        (
            "NUL in a context",
            valid.replacen("CI / linux", "CI / lin\0ux", 1),
        ),
        ("trailing garbage", format!("{valid}not-json")),
    ] {
        assert!(!release_function_accepts(&dir, bad.as_bytes()), "{name}");
    }
    for field in NON_STANDARD_FIELDS {
        let bad = valid.replacen('{', &format!("{{{field},"), 1);
        assert!(!release_function_accepts(&dir, bad.as_bytes()), "{field}");
    }
    assert!(
        !release_function_accepts(&dir, invalid_utf8(&valid)),
        "invalid UTF-8"
    );

    // A merge is eligible through HEAD^2 only while HEAD^2 has HEAD's tree.
    let merged = repo("release-merge");
    git(&merged, &["checkout", "-q", "-b", "topic"]);
    fs::write(merged.join("src/lib.rs"), "pub fn topic() {}\n").unwrap();
    commit(&merged, "topic");
    git(&merged, &["checkout", "-q", "main"]);
    git(&merged, &["merge", "-q", "--no-ff", "-m", "merge", "topic"]);
    let failing = valid.replacen("success", "failure", 1);
    assert!(
        release_function_accepts_each(&merged, failing.as_bytes(), valid.as_bytes()),
        "HEAD has no passing CI, the identical-tree HEAD^2 has"
    );
    assert!(
        !release_function_accepts(&merged, failing.as_bytes()),
        "neither passed"
    );
    assert!(
        !release_function_accepts_each(&merged, failing.as_bytes(), format!("{valid}\0")),
        "the only candidate's response holds a NUL"
    );

    let nan = valid.replacen('{', "{\"total_count\":NaN,", 1);
    assert!(
        !release_function_accepts_each(&merged, failing.as_bytes(), nan.as_bytes()),
        "the identical-tree HEAD^2's response is not JSON"
    );
    assert!(
        !release_function_accepts_each(&merged, failing.as_bytes(), invalid_utf8(&valid)),
        "the identical-tree HEAD^2's response is not UTF-8"
    );

    fs::write(merged.join("README.md"), "# amended\n").unwrap();
    git(&merged, &["add", "README.md"]);
    git(
        &merged,
        &["commit", "-q", "--amend", "-m", "merge with another tree"],
    );
    assert_ne!(
        git(&merged, &["rev-parse", "HEAD^{tree}"]),
        git(&merged, &["rev-parse", "HEAD^2^{tree}"])
    );
    assert!(
        !release_function_accepts_each(&merged, failing.as_bytes(), valid.as_bytes()),
        "a passing HEAD^2 with another tree does not count"
    );
}
