//! The release scripts' offline parts (DESIGN §21.7, §21.13): every script parses,
//! `release.sh` refuses bad arguments before its first step, and (macOS, where the
//! release is cut and `tar` is bsdtar) `scripts/package-release.sh` builds the tarball
//! layout from stub binaries under `CARGO_TARGET_TMPDIR`. Nothing here builds, signs,
//! notarises, tags, uploads or contacts a host; the scripts run with a cleared
//! environment and `HOME` and `TMPDIR` inside the test's directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const SCRIPTS: [&str; 5] = [
    "scripts/release.sh",
    "scripts/package-release.sh",
    "scripts/homebrew-formula.sh",
    "scripts/npm-package.sh",
    "scripts/cli-reference.sh",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh private directory with `home/` and `tmp/`.
fn sandbox(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "release-packaging-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(dir.join("tmp")).unwrap();
    dir
}

/// Runs `bash <script> <args>` in `dir` with a cleared environment; `path` is `PATH`.
fn run_script(dir: &Path, path: &str, script: &str, args: &[&str]) -> (i32, String, String) {
    let out = Command::new("/bin/bash")
        .arg(root().join(script))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", path)
        .env("HOME", dir.join("home"))
        .env("TMPDIR", dir.join("tmp"))
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

/// `bash -n` accepts every release script, and each is executable in the repository.
#[test]
fn the_release_scripts_parse() {
    use std::os::unix::fs::PermissionsExt;
    for script in SCRIPTS {
        let out = Command::new("/bin/bash")
            .arg("-n")
            .arg(root().join(script))
            .env_clear()
            .output()
            .unwrap();
        assert!(out.status.success(), "{script}: {out:?}");
        let mode = fs::metadata(root().join(script))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "{script} is executable: {mode:o}");
        assert!(
            read(script).starts_with("#!/usr/bin/env bash\n"),
            "{script}"
        );
    }
}

/// `release.sh` checks its arguments before the first step: a bad version, an unknown
/// option or a second version exits 2 with nothing run, and `--help` prints the usage.
#[test]
fn release_sh_refuses_bad_arguments_before_any_step() {
    let dir = sandbox("release-args");
    let path = "/usr/bin:/bin";
    for (args, message) in [
        (
            &["v1.0.0"][..],
            "release: 'v1.0.0' is not a version like 0.2.0 (no leading v)\n",
        ),
        (
            &["1.0"][..],
            "release: '1.0' is not a version like 0.2.0 (no leading v)\n",
        ),
        (
            &["1.0.0", "--bogus"][..],
            "release: unknown option: --bogus\n",
        ),
        (
            &["1.0.0", "1.0.1"][..],
            "release: more than one version given\n",
        ),
    ] {
        let (code, stdout, stderr) = run_script(&dir, path, "scripts/release.sh", args);
        assert_eq!(
            (code, stdout.as_str(), stderr.as_str()),
            (2, "", message),
            "{args:?}"
        );
    }
    let (code, stdout, stderr) = run_script(&dir, path, "scripts/release.sh", &[]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert!(stderr.starts_with("Cuts an airs-cli release"), "{stderr}");
    let (code, stdout, stderr) = run_script(&dir, path, "scripts/release.sh", &["--help"]);
    assert_eq!((code, stderr.as_str()), (0, ""));
    assert!(
        stdout.contains("  scripts/release.sh <version> [--dry-run] [--skip-notarize]\n"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// Each script's usage is the heredoc in its `usage()` (never lines cut from its own
/// header by number, which a new header line would shift or truncate): `--help` prints
/// that whole text, an exit-2 refusal prints it whole on stderr after the message, and
/// it names every option the script's argument parser accepts.
#[test]
fn every_script_prints_its_whole_usage() {
    let dir = sandbox("usage");
    let path = "/usr/bin:/bin";
    for (script, refusal, message) in [
        ("scripts/release.sh", &[][..], ""),
        (
            "scripts/package-release.sh",
            &["--run-check"][..],
            "package-release: --run-check needs a target\n",
        ),
        (
            "scripts/homebrew-formula.sh",
            &["--checksum"][..],
            "homebrew-formula: --checksum needs a value\n",
        ),
        (
            "scripts/npm-package.sh",
            &["--dist"][..],
            "npm-package: --dist needs a directory\n",
        ),
        (
            "scripts/cli-reference.sh",
            &["--bogus"][..],
            "cli-reference: unknown argument: --bogus\n",
        ),
    ] {
        let source = read(script);
        assert!(
            !source.contains("\"$0\" | sed"),
            "{script} cuts its usage from its own lines"
        );
        let start = source
            .find("usage() {\n  cat <<'USAGE'\n")
            .unwrap_or_else(|| panic!("{script} has no usage heredoc"));
        let body = &source[start..];
        let body = &body[body.find('\n').unwrap() + 1..];
        let body = &body[body.find('\n').unwrap() + 1..];
        let usage = &body[..body
            .find("\nUSAGE\n}\n")
            .unwrap_or_else(|| panic!("{script}: unterminated usage"))
            + 1];
        assert!(usage.lines().count() >= 8, "{script}: {usage}");
        let (code, stdout, stderr) = run_script(&dir, path, script, &["--help"]);
        assert_eq!(
            (code, stdout.as_str(), stderr.as_str()),
            (0, usage, ""),
            "{script} --help"
        );
        // Every option the parser handles (case arms such as `--a | --b)`) is described.
        let options: Vec<&str> = source
            .lines()
            .map(str::trim_start)
            .filter(|line| line.starts_with("--"))
            .filter_map(|line| line.split_once(')').map(|(arm, _)| arm))
            .filter(|arm| {
                arm.split(" | ").all(|option| {
                    option.len() > 2
                        && option.starts_with("--")
                        && option[2..]
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b == b'-')
                })
            })
            .flat_map(|arm| arm.split(" | "))
            .filter(|option| *option != "--help")
            .collect();
        assert!(!options.is_empty(), "{script}: no options found");
        for option in options {
            assert!(usage.contains(option), "{script}: usage lacks {option}");
        }
        let (code, stdout, stderr) = run_script(&dir, path, script, refusal);
        assert_eq!(
            (code, stdout.as_str(), stderr),
            (2, "", format!("{message}{usage}")),
            "{script} {refusal:?}"
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

/// `release.sh` builds only `airs` and packages through `scripts/package-release.sh`
/// (DESIGN §21.7): its build lines name `--bin "$BIN"` with `BIN="airs"`, the signature
/// check names the identifier, the real run verifies the packaged signature, the asset
/// names are read from the `SHA256SUMS` package-release.sh wrote (the tarball naming
/// rule lives only there), and the release names are `airs-cli <version>`. Its earlier
/// checks are all still there.
#[test]
fn release_sh_builds_airs_and_packages_the_link() {
    let script = read("scripts/release.sh");
    for line in [
        "BIN=\"airs\"",
        "COMPAT_BIN=\"airs-cli-rs\"",
        "run cargo build --release --locked --target \"$MAC_TARGET\" --bin \"$BIN\"",
        "  run cargo zigbuild --release --locked --target \"$target\" --bin \"$BIN\"",
        "act codesign --force --options runtime --timestamp --sign \"$SIGN_IDENTITY\" \"$mac_bin\"",
        "  grep -qx \"Identifier=$BIN\" <<<\"$details\" || die \"signature identifier is not $BIN\"",
        "  package_args+=(--verify-signature \"$MAC_TARGET\")",
        "run scripts/package-release.sh \"${package_args[@]}\" \"$version\" \"$DIST\" \"${package_specs[@]}\"",
        "  name=\"${line#*  }\"",
        "  tarballs+=(\"$name\")",
        "done <\"$DIST/SHA256SUMS\"",
        "[ \"${#tarballs[@]}\" -eq \"${#package_specs[@]}\" ] \\",
        "act git tag -a \"$TAG\" -m \"airs-cli $version\"",
    ] {
        assert!(
            script.lines().any(|got| got == line),
            "release.sh lacks {line:?}"
        );
    }
    assert!(script.contains("--arg name \"airs-cli $version\""));
    // The tarball naming rule is package-release.sh's alone.
    assert!(
        !script.contains("PKG=") && !script.contains("$version-$target"),
        "release.sh repeats the tarball naming rule"
    );
    assert!(read("scripts/package-release.sh").contains("  name=\"$PKG-$version-$target\"\n"));
    assert!(script.contains("printf 'airs-cli %s.\\n\\n"));
    // The 0.11 checks and safety steps (RELEASING §3) are unchanged.
    for check in [
        "check \"on branch main\" on_branch_main",
        "check \"working tree is clean (no changes, no untracked files)\" tree_is_clean",
        "check \"HEAD is origin/main\" head_is_origin_main",
        "check \"tag $TAG does not exist locally\" tag_absent_locally",
        "check \"tag $TAG does not exist on origin\" tag_absent_on_origin",
        "check \"no Forgejo release for $TAG\" release_absent_on_forgejo",
        "check \"no Forgejo draft release for $TAG\" no_draft_for_tag",
        "check \"the Forgejo token's user can push to the repository\" token_can_push",
        "check \"CI passed on a commit with HEAD's tree (HEAD or the merged pull request head)\" ci_passed_for_head",
        "  grep -q 'flags=.*runtime' <<<\"$details\" || die \"signature lacks the hardened runtime\"",
        "    run ditto -c -k --keepParent \"$mac_bin\" \"$notary_zip\"",
        "trap on_error ERR",
    ] {
        assert!(
            script
                .lines()
                .any(|got| got.trim_start() == check.trim_start()),
            "release.sh lacks {check:?}"
        );
    }
    // The script never renders, commits or pushes the formula, and never echoes a token.
    assert!(
        !script.contains("homebrew-formula.sh 2>")
            && !script.contains("run scripts/homebrew-formula.sh")
    );
    assert!(!script.contains("set -x"));
    // The release trusts CI on the exact tree instead of re-running the suite on the Mac.
    assert!(
        !script.contains("cargo test"),
        "release.sh runs no test suite"
    );
}

/// An empty positional argument is refused with the usage before anything is created
/// or removed: an empty `<dist>` would otherwise point the cleanup at `/.stage`. The
/// argument checks run before the bsdtar check, so this runs on every platform.
#[test]
fn package_release_refuses_empty_arguments() {
    let dir = sandbox("empty-arguments");
    let usage = "  scripts/package-release.sh [--run-check <target>] [--verify-signature <target>]";
    let spec = "aarch64-apple-darwin=/bin/sh";
    for (args, message) in [
        (
            &["1.0.0", "", spec][..],
            "package-release: the <dist> argument is empty",
        ),
        (
            &["--run-check", "aarch64-apple-darwin", "1.0.0", "", spec][..],
            "package-release: the <dist> argument is empty",
        ),
        (
            &["", "dist", spec][..],
            "package-release: the <version> argument is empty",
        ),
        (
            &["1.0.0", "dist", spec, ""][..],
            "package-release: a <target>=<binary> argument is empty",
        ),
        (
            &["1.0.0", "dist", "", spec][..],
            "package-release: a <target>=<binary> argument is empty",
        ),
    ] {
        let (code, stdout, stderr) =
            run_script(&dir, "/usr/bin:/bin", "scripts/package-release.sh", args);
        assert_eq!((code, stdout.as_str()), (2, ""), "{args:?}: {stderr}");
        assert_eq!(stderr.lines().next(), Some(message), "{args:?}: {stderr}");
        assert!(
            stderr.lines().any(|line| line == usage),
            "{args:?}: {stderr}"
        );
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["home", "tmp"], "{args:?} wrote something");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(target_os = "macos")]
mod packaging {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    const TARGETS: [&str; 3] = [
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
    ];
    const PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

    /// Stub binaries: the macOS one prints `version` for `--version`; the Linux ones
    /// only have to be executable files.
    fn stubs(dir: &Path, version: &str) -> Vec<String> {
        let bins = dir.join("bins");
        fs::create_dir_all(&bins).unwrap();
        TARGETS
            .iter()
            .map(|target| {
                let path = bins.join(target);
                let body = if *target == "aarch64-apple-darwin" {
                    format!("#!/bin/sh\n[ \"$1\" = --version ] && echo {version}\n")
                } else {
                    format!("#!/bin/sh\necho 'a stub {target} airs'\n")
                };
                fs::write(&path, body).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
                format!("{target}={}", path.display())
            })
            .collect()
    }

    fn package(dir: &Path, args: &[&str]) -> (i32, String, String) {
        run_script(dir, PATH, "scripts/package-release.sh", args)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn tar(args: &[&str]) -> String {
        let out = Command::new("/usr/bin/tar")
            .args(args)
            .env_clear()
            .output()
            .unwrap();
        assert!(out.status.success(), "tar {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap()
    }

    /// Three tarballs `airs-cli-<version>-<target>.tar.gz`, each listing exactly `airs`,
    /// `airs-cli`, `airs-cli-rs`, `README.md`, `LICENSE`, `NOTICE` in that order,
    /// root-owned, with `airs` a 0755 copy of the binary, `airs-cli` and `airs-cli-rs`
    /// symlinks to exactly `airs` and the text files 0644 copies of the repository's;
    /// `SHA256SUMS` lists exactly the tarballs.
    #[test]
    fn the_tarballs_hold_airs_and_the_compatibility_link() {
        let dir = sandbox("layout");
        let specs = stubs(&dir, "1.0.0");
        let dist = dir.join("dist");
        let mut args = vec![
            "--run-check",
            "aarch64-apple-darwin",
            "1.0.0",
            dist.to_str().unwrap(),
        ];
        args.extend(specs.iter().map(String::as_str));
        let (code, stdout, stderr) = package(&dir, &args);
        assert_eq!(code, 0, "{stdout}{stderr}");
        assert_eq!(stderr, "");
        assert!(
            stdout.contains(
                "    ok: airs-cli-1.0.0-aarch64-apple-darwin.tar.gz: ./airs, ./airs-cli and ./airs-cli-rs --version print 1.0.0\n"
            ),
            "{stdout}"
        );
        let tarballs: Vec<String> = TARGETS
            .iter()
            .map(|target| format!("airs-cli-1.0.0-{target}.tar.gz"))
            .collect();
        let mut expected = tarballs.clone();
        expected.push("SHA256SUMS".into());
        expected.sort();
        assert_eq!(names(&dist), expected, "no .stage or .verify left");

        let sums = fs::read_to_string(dist.join("SHA256SUMS")).unwrap();
        let listed: Vec<&str> = sums
            .lines()
            .map(|line| line.split_once("  ").unwrap().1)
            .collect();
        assert_eq!(
            listed,
            tarballs.iter().map(String::as_str).collect::<Vec<_>>()
        );
        for line in sums.lines() {
            let (hash, _) = line.split_once("  ").unwrap();
            assert!(
                hash.len() == 64
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "{line}"
            );
        }
        let check = Command::new("/usr/bin/shasum")
            .args(["-a", "256", "-c", "SHA256SUMS"])
            .current_dir(&dist)
            .env_clear()
            .output()
            .unwrap();
        assert!(check.status.success(), "{check:?}");

        for (tarball, (target, spec)) in tarballs.iter().zip(TARGETS.iter().zip(&specs)) {
            let file = dist.join(tarball);
            let file = file.to_str().unwrap();
            assert_eq!(
                tar(&["-tzf", file]),
                "airs\nairs-cli\nairs-cli-rs\nREADME.md\nLICENSE\nNOTICE\n",
                "{tarball}"
            );
            let listing = tar(&["-tvzf", file]);
            let rows: Vec<Vec<&str>> = listing
                .lines()
                .map(|line| line.split_whitespace().collect())
                .collect();
            assert_eq!(rows.len(), 6, "{listing}");
            for row in &rows {
                assert_eq!((row[2], row[3]), ("root", "root"), "{listing}");
            }
            assert_eq!(rows[0][0], "-rwxr-xr-x", "{listing}");
            for (row, link) in [(1, "airs-cli"), (2, "airs-cli-rs")] {
                assert!(rows[row][0].starts_with('l'), "{listing}");
                assert_eq!(
                    rows[row][rows[row].len() - 3..],
                    [link, "->", "airs"],
                    "{listing}"
                );
            }
            assert_eq!(
                (rows[3][0], rows[4][0], rows[5][0]),
                ("-rw-r--r--", "-rw-r--r--", "-rw-r--r--"),
                "{listing}"
            );
            assert!(!listing.contains("._"), "no AppleDouble entries: {listing}");

            let out = dir.join("extract").join(target);
            fs::create_dir_all(&out).unwrap();
            tar(&["-xzf", file, "-C", out.to_str().unwrap()]);
            let airs = fs::symlink_metadata(out.join("airs")).unwrap();
            assert!(
                airs.file_type().is_file(),
                "{tarball}: airs is a regular file"
            );
            assert_eq!(airs.mode() & 0o777, 0o755, "{tarball}");
            let binary = spec.split_once('=').unwrap().1;
            assert_eq!(
                fs::read(out.join("airs")).unwrap(),
                fs::read(binary).unwrap(),
                "{tarball}"
            );
            for name in ["airs-cli", "airs-cli-rs"] {
                let link = fs::symlink_metadata(out.join(name)).unwrap();
                assert!(
                    link.file_type().is_symlink(),
                    "{tarball}: {name} is a symlink"
                );
                assert_eq!(
                    fs::read_link(out.join(name)).unwrap(),
                    Path::new("airs"),
                    "{tarball}: {name}"
                );
            }
            for text in ["README.md", "LICENSE", "NOTICE"] {
                let meta = fs::symlink_metadata(out.join(text)).unwrap();
                assert!(meta.file_type().is_file());
                assert_eq!(meta.mode() & 0o777, 0o644, "{tarball}: {text}");
                assert_eq!(
                    fs::read(out.join(text)).unwrap(),
                    fs::read(root().join(text)).unwrap()
                );
            }
            assert_eq!(
                names(&out),
                [
                    "LICENSE",
                    "NOTICE",
                    "README.md",
                    "airs",
                    "airs-cli",
                    "airs-cli-rs"
                ]
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    /// A non-empty dist, a missing or non-executable binary, a bad version or target
    /// spec, a check target that is not packaged and a tar that is not bsdtar are refused
    /// before anything is written.
    #[test]
    fn refusals_write_nothing() {
        let dir = sandbox("refusals");
        let specs = stubs(&dir, "1.0.0");
        let dist = dir.join("dist");
        let dist_s = dist.to_str().unwrap().to_owned();
        let missing = format!("aarch64-apple-darwin={}", dir.join("bins/absent").display());
        let plain = dir.join("bins/plain");
        fs::write(&plain, "not executable").unwrap();
        fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
        let plain = format!("aarch64-apple-darwin={}", plain.display());
        let usage =
            "  scripts/package-release.sh [--run-check <target>] [--verify-signature <target>]";
        let cases: Vec<(Vec<&str>, i32, String)> = vec![
            (vec!["v1.0.0", &dist_s, &specs[0]], 2, "package-release: 'v1.0.0' is not a version like 1.0.0 (no leading v)".into()),
            (vec!["1.0.0", &dist_s], 2, "package-release: needs a version, a dist directory and at least one <target>=<binary>".into()),
            (vec!["1.0.0", &dist_s, "aarch64-apple-darwin"], 2, "package-release: 'aarch64-apple-darwin' is not <target>=<binary>".into()),
            (vec!["1.0.0", &dist_s, "../x=/bin/sh"], 2, "package-release: '../x' is not a target name".into()),
            (vec!["1.0.0", &dist_s, &specs[0], &specs[0]], 2, "package-release: target aarch64-apple-darwin given more than once".into()),
            (vec!["--run-check", "x86_64-apple-darwin", "1.0.0", &dist_s, &specs[0]], 2, "package-release: x86_64-apple-darwin is not one of the packaged targets".into()),
            (vec!["--bogus", "1.0.0", &dist_s, &specs[0]], 2, "package-release: unknown option: --bogus".into()),
            // A target is never another option: the flag that follows is not swallowed.
            (vec!["--run-check", "--verify-signature", "1.0.0", &dist_s, &specs[0]], 2, "package-release: --run-check needs a target".into()),
            (vec!["--verify-signature", "--run-check", "aarch64-apple-darwin", "1.0.0", &dist_s, &specs[0]], 2, "package-release: --verify-signature needs a target".into()),
            (vec!["--run-check", "-", "1.0.0", &dist_s, &specs[0]], 2, "package-release: --run-check needs a target".into()),
            (vec!["1.0.0", &dist_s, &missing], 1, format!("package-release: aarch64-apple-darwin: {} is missing or not a file", dir.join("bins/absent").display())),
            (vec!["1.0.0", &dist_s, &plain], 1, format!("package-release: aarch64-apple-darwin: {} is not executable", dir.join("bins/plain").display())),
        ];
        for (args, code, message) in cases {
            let (got, stdout, stderr) = package(&dir, &args);
            assert_eq!(got, code, "{args:?}: {stderr}");
            assert_eq!(stdout, "", "{args:?}");
            assert_eq!(
                stderr.lines().next(),
                Some(message.as_str()),
                "{args:?}: {stderr}"
            );
            assert_eq!(code == 2, stderr.contains(usage), "{args:?}: {stderr}");
            assert!(!dist.exists(), "{args:?} created the dist directory");
        }

        // A non-empty dist is refused and left as it was.
        fs::create_dir_all(&dist).unwrap();
        fs::write(dist.join("old.tar.gz"), "an earlier package").unwrap();
        let mut args = vec!["1.0.0", dist_s.as_str()];
        args.extend(specs.iter().map(String::as_str));
        let (code, stdout, stderr) = package(&dir, &args);
        assert_eq!((code, stdout.as_str()), (1, ""), "{stderr}");
        assert_eq!(
            stderr,
            format!("package-release: {dist_s} exists and is not empty\n")
        );
        assert_eq!(names(&dist), ["old.tar.gz"]);
        fs::remove_dir_all(&dist).unwrap();

        // A tar that is not bsdtar (GNU tar would store no link or drop no metadata).
        let gnu = dir.join("gnu");
        fs::create_dir_all(&gnu).unwrap();
        fs::write(gnu.join("tar"), "#!/bin/sh\necho 'tar (GNU tar) 1.35'\n").unwrap();
        fs::set_permissions(gnu.join("tar"), fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!("{}:{PATH}", gnu.display());
        let (code, _, stderr) = run_script(&dir, &path, "scripts/package-release.sh", &args);
        assert_eq!(code, 1, "{stderr}");
        assert_eq!(
            stderr,
            "package-release: tar is not bsdtar (it must store the link as a link and drop macOS metadata)\n"
        );
        assert!(!dist.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    /// `--verify-signature` runs `codesign --verify --strict` on the extracted `airs`
    /// (DESIGN §21.7, choice 42). The signed binary is a copy of `/usr/bin/true` (Apple
    /// signs it; nothing here signs anything or reads a keychain); an unsigned stub fails
    /// the check, and the run removes what it wrote. Skipped, with a note, where
    /// `/usr/bin/codesign` is missing or does not accept the copy.
    #[test]
    fn verify_signature_checks_the_packaged_binary() {
        let codesign = Path::new("/usr/bin/codesign");
        let dir = sandbox("verify-signature");
        let signed = dir.join("signed-airs");
        let usable = codesign.exists()
            && fs::copy("/usr/bin/true", &signed).is_ok()
            && Command::new(codesign)
                .args(["--verify", "--strict"])
                .arg(&signed)
                .env_clear()
                .output()
                .is_ok_and(|out| out.status.success());
        if !usable {
            eprintln!("skipped: /usr/bin/codesign cannot verify a copy of /usr/bin/true here");
            let _ = fs::remove_dir_all(&dir);
            return;
        }
        let specs = stubs(&dir, "1.0.0");
        let dist = dir.join("dist");
        let dist_s = dist.to_str().unwrap().to_owned();
        let mac = format!("aarch64-apple-darwin={}", signed.display());
        let (code, stdout, stderr) = package(
            &dir,
            &[
                "--verify-signature",
                "aarch64-apple-darwin",
                "1.0.0",
                &dist_s,
                &mac,
                &specs[1],
            ],
        );
        assert_eq!((code, stderr.as_str()), (0, ""), "{stdout}");
        assert!(
            stdout.contains(
                "    ok: airs-cli-1.0.0-aarch64-apple-darwin.tar.gz: the packaged airs passes codesign --verify --strict\n"
            ),
            "{stdout}"
        );
        assert!(
            !stdout.contains("x86_64-unknown-linux-musl.tar.gz: the packaged airs passes"),
            "only the named target is verified: {stdout}"
        );
        fs::remove_dir_all(&dist).unwrap();

        // The unsigned stub fails the check; nothing the run wrote is left.
        let (code, stdout, stderr) = package(
            &dir,
            &[
                "--verify-signature",
                "aarch64-apple-darwin",
                "1.0.0",
                &dist_s,
                &specs[0],
            ],
        );
        assert_eq!(code, 1, "{stdout}{stderr}");
        assert_eq!(
            stderr.lines().last(),
            Some(
                "package-release: airs-cli-1.0.0-aarch64-apple-darwin.tar.gz: the packaged airs fails codesign --verify --strict"
            ),
            "{stderr}"
        );
        assert!(!dist.exists(), "the dist directory the run created is gone");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A packaged `airs` that does not print the release version fails the run check,
    /// and the run removes everything it wrote (the tarballs, `SHA256SUMS`, the dist it
    /// created).
    #[test]
    fn a_failed_check_removes_what_the_run_wrote() {
        let dir = sandbox("failed-check");
        let specs = stubs(&dir, "0.11.0");
        let dist = dir.join("dist");
        let mut args = vec![
            "--run-check",
            "aarch64-apple-darwin",
            "1.0.0",
            dist.to_str().unwrap(),
        ];
        args.extend(specs.iter().map(String::as_str));
        let (code, _, stderr) = package(&dir, &args);
        assert_eq!(code, 1, "{stderr}");
        assert_eq!(
            stderr,
            "package-release: airs-cli-1.0.0-aarch64-apple-darwin.tar.gz: ./airs --version prints '0.11.0', expected '1.0.0'\n"
        );
        assert!(!dist.exists(), "the dist directory the run created is gone");

        // Into an existing empty dist, the failure leaves it empty.
        fs::create_dir_all(&dist).unwrap();
        let (code, _, _) = package(&dir, &args);
        assert_eq!(code, 1);
        assert!(names(&dist).is_empty(), "{:?}", names(&dist));
        let _ = fs::remove_dir_all(&dir);
    }
}
