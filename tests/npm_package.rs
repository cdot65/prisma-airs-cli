//! The npm channel (8.0, DESIGN §21.16): the launcher's own tests (`node --test`), the
//! argument and input refusals of `scripts/npm-package.sh`, and, on macOS (the release
//! tarballs are packaged with bsdtar), the staged packages and an install from them
//! through a loopback registry into a temporary prefix that runs `airs --version` and
//! `airs-cli --version`. Nothing is published and no request leaves loopback.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh private directory with `home/` and `tmp/`.
fn sandbox(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "npm-package-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(dir.join("tmp")).unwrap();
    dir
}

/// Node: `AIRS_TEST_NODE`, else the first `node` on `PATH` (as other test helpers find
/// it). The npm channel's tests need it and fail without it.
fn node() -> PathBuf {
    if let Some(node) = std::env::var_os("AIRS_TEST_NODE") {
        return PathBuf::from(node);
    }
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join("node"))
                .find(|candidate| candidate.is_file())
        })
        .expect("node is required for the npm launcher tests (AIRS_TEST_NODE or PATH)")
}

/// `PATH` for the scripts: node's directory (npm lives beside it), then the system.
fn script_path() -> String {
    format!(
        "{}:/usr/bin:/bin:/usr/sbin:/sbin",
        node().parent().unwrap().display()
    )
}

/// Runs `bash scripts/npm-package.sh <args>` in `dir` with a cleared environment.
fn npm_package(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new("/bin/bash")
        .arg(root().join("scripts/npm-package.sh"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", script_path())
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

fn usage() -> String {
    let (code, stdout, stderr) = npm_package(&sandbox("usage"), &["--help"]);
    assert_eq!((code, stderr.as_str()), (0, ""));
    stdout
}

/// The launcher's `node --test` suite (npm/test/launcher.test.js): platform mapping, the
/// refusals for unsupported platforms and a missing platform package, argument, stream
/// and exit-code passthrough, signal forwarding and re-raising.
#[test]
fn the_launcher_tests_pass() {
    let dir = sandbox("launcher");
    let out = Command::new(node())
        .args(["--test", "npm/test/launcher.test.js"])
        .current_dir(root())
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", dir.join("home"))
        .env("TMPDIR", dir.join("tmp"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("\n# fail 0\n") || stdout.contains("ℹ fail 0"),
        "{stdout}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// Argument errors exit 2 with the usage; a missing dist, a missing `SHA256SUMS`, one
/// that lists other files or does not match exit 1. None of them writes anything.
#[test]
fn npm_package_refuses_bad_input_and_writes_nothing() {
    let dir = sandbox("refusals");
    let usage = usage();
    let out = dir.join("out");
    let out_arg = out.to_str().unwrap();
    for (args, message) in [
        (&[][..], "npm-package: needs exactly one version\n"),
        (
            &["v8.0.0"],
            "npm-package: 'v8.0.0' is not a version like 8.0.0 (no leading v)\n",
        ),
        (
            &["8.0.0", "8.0.1"],
            "npm-package: needs exactly one version\n",
        ),
        (
            &["--bogus", "8.0.0"],
            "npm-package: unknown option: --bogus\n",
        ),
        (
            &["--output", "--dist", "x", "8.0.0"],
            "npm-package: --output needs a directory\n",
        ),
        (
            &["--dist", "a", "--dist", "b", "8.0.0"],
            "npm-package: --dist given more than once\n",
        ),
    ] {
        let (code, stdout, stderr) = npm_package(&dir, args);
        assert_eq!(
            (code, stdout.as_str(), stderr),
            (2, "", format!("{message}{usage}")),
            "{args:?}"
        );
    }
    let dist = dir.join("dist");
    let dist_arg = dist.to_str().unwrap();
    let refused = |expected: &str| {
        let (code, stdout, stderr) =
            npm_package(&dir, &["--dist", dist_arg, "--output", out_arg, "8.0.0"]);
        assert_eq!((code, stdout.as_str()), (1, ""), "{stderr}");
        assert!(stderr.contains(expected), "{stderr}");
        assert!(!out.exists(), "nothing written");
    };
    refused("is not a directory");
    fs::create_dir_all(&dist).unwrap();
    refused("SHA256SUMS is missing");
    let names = [
        "airs-cli-8.0.0-aarch64-apple-darwin.tar.gz",
        "airs-cli-8.0.0-x86_64-unknown-linux-musl.tar.gz",
        "airs-cli-8.0.0-aarch64-unknown-linux-musl.tar.gz",
    ];
    for name in names {
        fs::write(dist.join(name), name).unwrap();
    }
    fs::write(
        dist.join("SHA256SUMS"),
        format!("{}  {}\n", "0".repeat(64), names[0]),
    )
    .unwrap();
    refused("does not list exactly the three airs-cli-8.0.0-<target>.tar.gz");
    let sums: String = names
        .iter()
        .map(|name| format!("{}  {name}\n", "0".repeat(64)))
        .collect();
    fs::write(dist.join("SHA256SUMS"), sums).unwrap();
    refused("do not match SHA256SUMS");
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(target_os = "macos")]
mod staging {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const TARGETS: [(&str, &str); 3] = [
        ("darwin-arm64", "aarch64-apple-darwin"),
        ("linux-x64", "x86_64-unknown-linux-musl"),
        ("linux-arm64", "aarch64-unknown-linux-musl"),
    ];

    /// Release tarballs from `scripts/package-release.sh`: the real `airs` (this build)
    /// for macOS, stub executables for Linux.
    fn release(dir: &Path) -> PathBuf {
        let bins = dir.join("bins");
        fs::create_dir_all(&bins).unwrap();
        let stub = bins.join("linux-stub");
        fs::write(&stub, "#!/bin/sh\necho 'a stub airs'\n").unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        let dist = dir.join("dist");
        let out = Command::new("/bin/bash")
            .arg(root().join("scripts/package-release.sh"))
            .args([
                "--run-check",
                "aarch64-apple-darwin",
                VERSION,
                dist.to_str().unwrap(),
                &format!("aarch64-apple-darwin={}", env!("CARGO_BIN_EXE_airs")),
                &format!("x86_64-unknown-linux-musl={}", stub.display()),
                &format!("aarch64-unknown-linux-musl={}", stub.display()),
            ])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", dir.join("home"))
            .env("TMPDIR", dir.join("tmp"))
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        dist
    }

    fn tar_list(file: &Path) -> Vec<String> {
        let out = Command::new("/usr/bin/tar")
            .arg("-tzf")
            .arg(file)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        let mut names: Vec<String> = String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        names.sort();
        names
    }

    fn json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    /// The staged packages: one platform version per target holding exactly the
    /// released binary (byte-equal) with `os`/`cpu`, and the launcher package whose
    /// optional dependencies alias them; the packed tarballs hold exactly those files
    /// and `SHA256SUMS` lists them.
    #[test]
    fn the_staged_packages_hold_the_release_binaries() {
        let dir = sandbox("staged");
        let dist = release(&dir);
        let out = dir.join("npm");
        let (code, stdout, stderr) = npm_package(
            &dir,
            &[
                "--dist",
                dist.to_str().unwrap(),
                "--output",
                out.to_str().unwrap(),
                VERSION,
            ],
        );
        assert_eq!(code, 0, "{stdout}{stderr}");
        assert!(
            stdout.ends_with(
                "next: publish the platform versions, then the launcher (docs/RELEASING.md §7)\n"
            ),
            "{stdout}"
        );
        let mut packed = Vec::new();
        for (platform, target) in TARGETS {
            let dir = out.join("packages").join(platform);
            let manifest = json(&dir.join("package.json"));
            let (os, cpu) = platform.split_once('-').unwrap();
            assert_eq!(manifest["name"], "@cdot65/prisma-airs-cli");
            assert_eq!(manifest["version"], format!("{VERSION}-{platform}"));
            assert_eq!(manifest["os"], serde_json::json!([os]));
            assert_eq!(manifest["cpu"], serde_json::json!([cpu]));
            assert_eq!(manifest["license"], "Apache-2.0");
            assert!(manifest.get("bin").is_none() && manifest.get("scripts").is_none());
            let binary = dir.join("vendor").join(target).join("airs");
            assert_eq!(
                fs::metadata(&binary).unwrap().permissions().mode() & 0o777,
                0o755
            );
            if platform == "darwin-arm64" {
                assert_eq!(
                    fs::read(&binary).unwrap(),
                    fs::read(env!("CARGO_BIN_EXE_airs")).unwrap()
                );
            }
            let file = format!("cdot65-prisma-airs-cli-{VERSION}-{platform}.tgz");
            let mut expected: Vec<String> = [
                "package/LICENSE".to_owned(),
                "package/NOTICE".to_owned(),
                "package/README.md".to_owned(),
                "package/package.json".to_owned(),
                format!("package/vendor/{target}/airs"),
            ]
            .to_vec();
            expected.sort();
            assert_eq!(tar_list(&out.join(&file)), expected, "{file}");
            packed.push(file);
        }
        let main = json(&out.join("packages/main/package.json"));
        assert_eq!(main["version"], VERSION);
        assert_eq!(
            main["bin"],
            serde_json::json!({"airs": "bin/airs.js", "airs-cli": "bin/airs.js"})
        );
        assert_eq!(
            main["optionalDependencies"],
            serde_json::json!({
                "@cdot65/prisma-airs-cli-darwin-arm64": format!("npm:@cdot65/prisma-airs-cli@{VERSION}-darwin-arm64"),
                "@cdot65/prisma-airs-cli-linux-x64": format!("npm:@cdot65/prisma-airs-cli@{VERSION}-linux-x64"),
                "@cdot65/prisma-airs-cli-linux-arm64": format!("npm:@cdot65/prisma-airs-cli@{VERSION}-linux-arm64"),
            })
        );
        let file = format!("cdot65-prisma-airs-cli-{VERSION}.tgz");
        assert_eq!(
            tar_list(&out.join(&file)),
            [
                "package/LICENSE",
                "package/NOTICE",
                "package/README.md",
                "package/bin/airs.js",
                "package/lib/launcher.js",
                "package/package.json",
            ]
        );
        packed.push(file);
        // `npm pack --dry-run` of the staged launcher lists the same files.
        let dry = Command::new(node().parent().unwrap().join("npm"))
            .args(["pack", "--dry-run", "--json", "--ignore-scripts"])
            .current_dir(out.join("packages/main"))
            .env_clear()
            .env("PATH", script_path())
            .env("HOME", dir.join("home"))
            .env("npm_config_userconfig", dir.join("tmp/npmrc"))
            .env("npm_config_update_notifier", "false")
            .output()
            .unwrap();
        assert!(dry.status.success(), "{dry:?}");
        let listing: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
        let mut files: Vec<&str> = listing[0]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|file| file["path"].as_str().unwrap())
            .collect();
        files.sort_unstable();
        assert_eq!(
            files,
            [
                "LICENSE",
                "NOTICE",
                "README.md",
                "bin/airs.js",
                "lib/launcher.js",
                "package.json"
            ]
        );
        let sums = fs::read_to_string(out.join("SHA256SUMS")).unwrap();
        let listed: Vec<&str> = sums
            .lines()
            .map(|line| line.split_once("  ").unwrap().1)
            .collect();
        assert_eq!(
            listed,
            packed.iter().map(String::as_str).collect::<Vec<_>>()
        );

        // A second run into the same, now non-empty, output is refused.
        let (code, _, stderr) = npm_package(
            &dir,
            &[
                "--dist",
                dist.to_str().unwrap(),
                "--output",
                out.to_str().unwrap(),
                VERSION,
            ],
        );
        assert_eq!(code, 1);
        assert!(stderr.contains("exists and is not empty"), "{stderr}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// `npm install --global` of the staged launcher from a loopback registry
    /// (npm/test/install-check.mjs) installs only this Mac's platform package, and the
    /// installed `airs` and `airs-cli` print the version; the doctor's Executable check
    /// counts the npm links as this program (AIRS_NPM_LAUNCHER).
    #[test]
    fn an_install_from_the_staged_tarballs_runs_both_names() {
        let dir = sandbox("install");
        let dist = release(&dir);
        let out = dir.join("npm");
        let (code, stdout, stderr) = npm_package(
            &dir,
            &[
                "--dist",
                dist.to_str().unwrap(),
                "--output",
                out.to_str().unwrap(),
                VERSION,
            ],
        );
        assert_eq!(code, 0, "{stdout}{stderr}");
        let check = Command::new(node())
            .arg(root().join("npm/test/install-check.mjs"))
            .arg(&out)
            .arg(VERSION)
            .env_clear()
            .env("PATH", script_path())
            .env("HOME", dir.join("home"))
            .env("TMPDIR", dir.join("tmp"))
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&check.stdout),
            String::from_utf8_lossy(&check.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
        let version = format!("{VERSION}\n");
        assert_eq!(
            result,
            serde_json::json!({
                "installed": {"darwin-arm64": true, "linux-x64": false, "linux-arm64": false},
                "airs": {"code": 0, "stdout": version, "stderr": ""},
                "airsCli": {"code": 0, "stdout": version, "stderr": ""},
                "helpUsage": "Usage: airs [OPTIONS] [COMMAND]",
                "doctorCode": 1,
                "doctorFirst": {
                    "name": "Executable",
                    "status": "pass",
                    "detail": "no other airs, airs-cli or airs-cli-rs earlier on PATH"
                }
            })
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
