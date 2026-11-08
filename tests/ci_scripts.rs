//! The CI helper scripts fail safe (final M8 review 3).
//!
//! `scripts/ci/classify-change.sh` decides whether CI may skip the Rust steps; it must print
//! `rust=false` only for a change that touches documentation alone, and `rust=true` for
//! renames out of Rust paths, long path lists, a missing or unrelated base and empty diffs.
//! `scripts/ci/ci-status-ok.sh` decides whether `release.sh` trusts CI; it must accept only a
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

fn status_ok(json: &str) -> bool {
    let mut child = Command::new(script("ci-status-ok.sh"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
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
    assert!(!status_ok("not json"), "malformed");
    assert!(!status_ok(r#"{"state":"success"}"#), "no statuses array");
    assert!(!status_ok(r#"[]"#), "wrong shape");
}

#[test]
fn release_sh_uses_the_strict_status_check() {
    let release =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/release.sh"))
            .unwrap();
    assert!(release.contains("scripts/ci/ci-status-ok.sh"));
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
