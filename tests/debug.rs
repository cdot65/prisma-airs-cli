//! Executable tests for the `--debug` API log (DESIGN §2.9).
//!
//! Every run points `XDG_STATE_HOME` at `<sandbox>/tmp/xdg` (the runner allows writes
//! under `tmp/`); `PRISMA_AIRS_TENANTS_PATH` still selects the registry. The runner
//! leak-checks every log file like stdout and stderr, and each test also runs
//! [`debug_log::assert_logs_clean`]: no fixture secret form, `mock-token-` lease,
//! `x-payload-hash`, credential header value, URL query, body or prompt text in a log.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use airs_test_harness::debug_log::{self, log_dir, log_files, records, xdg_state_home};
use airs_test_harness::{
    Cli, FixtureConfig, FixtureServer, Planned, Route, Run, RunOptions, Sandbox, fixtures, run,
    spawn,
};
use serde_json::{Value, json};

const TSG: &str = "100";
const LIST: &[&str] = &["--debug", "runtime", "profiles", "list", "--output", "json"];
const NOTICE: &str = "  Debug: API log → ";
const HINT: &str = "    Re-run with --debug to capture redacted API diagnostics.\n";
const WARNING: &str = "  ⚠ Debug: the API log could not be written; it is incomplete\n";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

struct Fx {
    server: FixtureServer,
    sandbox: Sandbox,
}

/// One standard tenant `prod` (TSG 100), registered and active.
fn fx() -> Fx {
    fx_with(|_| {})
}

fn fx_with(edit: impl FnOnce(&mut FixtureConfig)) -> Fx {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, edit);
    sandbox.register(&[&tenant], Some("prod"));
    Fx { server, sandbox }
}

impl Fx {
    /// `XDG_STATE_HOME=<sandbox>/tmp/xdg`.
    fn options(&self) -> RunOptions {
        RunOptions::new().env(
            "XDG_STATE_HOME",
            xdg_state_home(&self.sandbox).display().to_string(),
        )
    }

    fn run(&self, args: &[&str]) -> Run {
        self.run_with(&rust(), args, self.options())
    }

    /// Runs, then checks the read-only and loopback invariants and the log leak check.
    fn run_with(&self, cli: &Cli, args: &[&str], options: RunOptions) -> Run {
        let before = self.sandbox.snapshot();
        let out = run(cli, &self.sandbox, args, &options);
        self.sandbox.assert_unchanged(&before);
        self.server.assert_no_unknown_routes();
        assert!(self.sandbox.cwd_listing().is_empty(), "wrote into cwd");
        self.clean(&[]);
        out
    }

    fn clean(&self, extra: &[&str]) {
        debug_log::assert_logs_clean(
            &self.sandbox,
            &self.server,
            &[
                log_dir(&self.sandbox),
                debug_log::default_log_dir(&self.sandbox),
            ],
            extra,
        );
    }

    fn only_log(&self) -> (PathBuf, Vec<Value>) {
        debug_log::only_log(&self.sandbox)
    }

    fn profiles(&self, count: u64) {
        let rows = (1..=count)
            .map(|n| fixtures::profile(n, &format!("p{n}"), 1))
            .collect();
        self.server.set_profiles(TSG, rows);
    }
}

/// The path the notice names (the first stderr line).
fn notice_path(stderr: &str) -> PathBuf {
    let line = stderr.lines().next().unwrap_or_default();
    let path = line
        .strip_prefix(NOTICE.trim_end())
        .map(str::trim_start)
        .unwrap_or_else(|| panic!("no notice in {stderr:?}"));
    PathBuf::from(path)
}

/// `(operation, status or error)` of every record.
fn summary(records: &[Value]) -> Vec<(String, String)> {
    records
        .iter()
        .map(|record| {
            let outcome = match record.get("response") {
                Some(response) => response["status"].to_string(),
                None => record["error"].as_str().unwrap().to_owned(),
            };
            (record["operation"].as_str().unwrap().to_owned(), outcome)
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

/// The fixed shape of a record line: fields in DESIGN §2.9 order, no body, no query.
fn assert_record_shape(path: &Path) {
    let text = fs::read_to_string(path).unwrap();
    for line in text.lines() {
        let at = |key: &str| line.find(&format!("\"{key}\":"));
        let order: Vec<usize> = ["timestamp", "durationMs", "operation", "request"]
            .iter()
            .map(|key| at(key).unwrap_or_else(|| panic!("{key} missing: {line}")))
            .collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "field order: {line}");
        assert!(line.starts_with("{\"timestamp\":\""), "{line}");
        let tail = at("response").or(at("error")).expect("response or error");
        assert!(tail > order[3]);
        assert!(at("response").is_none() || at("error").is_none(), "{line}");
        assert!(!line.contains("\"body\""), "{line}");
        assert!(!line.contains('?'), "no URL query: {line}");
        let record: Value = serde_json::from_str(line).unwrap();
        let url = record["request"]["url"].as_str().unwrap();
        assert!(url.starts_with("http://127.0.0.1:"), "{url}");
        assert!(!url.contains('#'));
        let headers = record["request"]["headers"].as_object().unwrap();
        let names: Vec<&String> = headers.keys().collect();
        assert!(names.iter().all(|name| **name == name.to_lowercase()));
        for (name, value) in headers {
            if ![
                "accept",
                "content-length",
                "content-type",
                "service-name",
                "user-agent",
                "x-tsg-id",
            ]
            .contains(&name.as_str())
            {
                assert_eq!(value, "***", "{name} must be masked");
            }
        }
    }
}

mod records {
    use super::*;

    /// One token record and one page record, written to a private file under
    /// `XDG_STATE_HOME`; stdout and the exit code equal a run without `--debug` (D62,
    /// D64, D66).
    #[test]
    fn profiles_list_records_the_token_and_the_page() {
        let f = fx();
        f.profiles(2);
        let out = f.run(LIST);
        out.assert_success();
        let (path, records) = f.only_log();
        assert_eq!(notice_path(&out.stderr), path);
        assert_eq!(
            out.stderr,
            format!("{NOTICE}{}\n  Showing 2\n", path.display())
        );
        assert_eq!(
            summary(&records),
            pairs(&[
                ("oauth.token", "200"),
                ("airs.management.profiles.list", "200")
            ])
        );
        let token = &records[0];
        assert_eq!(token["request"]["method"], "POST");
        assert_eq!(
            token["request"]["url"],
            f.server.url("/oauth/token").as_str()
        );
        assert_eq!(token["request"]["headers"]["authorization"], "***");
        assert_eq!(
            token["request"]["headers"]["content-type"],
            "application/x-www-form-urlencoded"
        );
        assert_eq!(token["response"]["statusText"], "OK");
        let page = &records[1];
        assert_eq!(
            page["request"]["url"],
            f.server.url("/aisec/v1/mgmt/profiles/tsg/100").as_str(),
            "the query is dropped"
        );
        assert_eq!(page["request"]["headers"]["authorization"], "***");
        assert_eq!(page["request"]["headers"]["service-name"], "api");
        assert_eq!(
            page["request"]["headers"]["user-agent"],
            "PAN-AIRS/0.2.6-rust-sdk"
        );
        assert_eq!(
            page["response"]["headers"]["content-type"],
            "application/json"
        );
        assert!(page["durationMs"].is_u64());
        assert_record_shape(&path);

        // The same command without --debug: identical stdout and exit code.
        let plain = run(&rust(), &f.sandbox, &LIST[1..], &RunOptions::new());
        assert_eq!(plain.code, out.code);
        assert_eq!(plain.stdout, out.stdout);
        assert_eq!(plain.stderr, "  Showing 2\n");
        assert_eq!(
            log_files(&log_dir(&f.sandbox)).len(),
            1,
            "no log without --debug"
        );
    }

    /// A 401 refresh-and-replay is four attempts and four records; a lease the server
    /// echoes in an allow-listed header is masked (D62, D65).
    #[test]
    fn refresh_and_replay_records_every_attempt() {
        let f = fx();
        f.profiles(1);
        f.server.set_rotating_tokens(true);
        f.server.push_once(
            Route::Profiles,
            Planned::empty_json(401)
                .with_header("x-request-id", Planned::BEARER)
                .echoing_bearer(),
        );
        let out = f.run(LIST);
        out.assert_success();
        let (_, records) = f.only_log();
        assert_eq!(
            summary(&records),
            pairs(&[
                ("oauth.token", "200"),
                ("airs.management.profiles.list", "401"),
                ("oauth.token", "200"),
                ("airs.management.profiles.list", "200"),
            ])
        );
        assert_eq!(records[1]["response"]["headers"]["x-request-id"], "***");
        assert_eq!(f.server.requests().len(), 4);
    }

    /// The token response is recorded only after its lease is registered: a token the
    /// server echoes in allow-listed headers of that same response is masked, for the
    /// first token and for the refresh-and-replay token (D65, §2.9).
    #[test]
    fn token_echoed_in_its_own_response_is_masked() {
        let f = fx();
        f.profiles(1);
        let token = |name: &str| {
            Planned::json(
                200,
                &json!({"access_token": name, "token_type": "Bearer", "expires_in": 3600}),
            )
        };
        f.server.push_once(
            Route::Token,
            token("mock-token-100.r1")
                .with_header("x-request-id", "mock-token-100.r1")
                .with_header("request-id", "mock-token-100.r1")
                .with_header("server", "mock-token-100.r1"),
        );
        f.server.push_once(
            Route::Token,
            token("mock-token-100.r2")
                .with_header("x-correlation-id", "mock-token-100.r2")
                .with_header("vary", "mock-token-100.r2"),
        );
        f.server
            .push_once(Route::Profiles, Planned::empty_json(401));
        let out = f.run(LIST);
        out.assert_success();
        let (path, records) = f.only_log();
        assert_eq!(
            summary(&records),
            pairs(&[
                ("oauth.token", "200"),
                ("airs.management.profiles.list", "401"),
                ("oauth.token", "200"),
                ("airs.management.profiles.list", "200"),
            ])
        );
        let first = &records[0]["response"]["headers"];
        for name in ["x-request-id", "request-id", "server"] {
            assert_eq!(first[name], "***", "{name}");
        }
        assert_eq!(first["content-type"], "application/json", "not a fallback");
        let second = &records[2]["response"]["headers"];
        for name in ["x-correlation-id", "vary"] {
            assert_eq!(second[name], "***", "{name}");
        }
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("mock-token-"), "no lease in the log");
        assert!(!text.contains("redactionFallback"));
        assert_record_shape(&path);
    }

    /// A secret echoed as a response header name reaches the log lowercased; the
    /// line check ignores case, so the record falls back to no URL and no headers.
    #[test]
    fn secret_echoed_as_a_header_name_is_never_written() {
        let f = fx();
        f.profiles(1);
        let basic = "Y2xpZW50LTEwMDpGQUtFLVNFQ1JFVC0xMDA"; // base64("client-100:FAKE-SECRET-100")
        f.server.push_once(
            Route::Profiles,
            Planned::json(
                200,
                &json!({"ai_profiles": [fixtures::profile(1, "p1", 1)]}),
            )
            .with_header("FAKE-SECRET-100", "1")
            .with_header(basic, "1"),
        );
        let out = f.run(LIST);
        out.assert_success();
        let (path, records) = f.only_log();
        assert_eq!(records[1]["operation"], "airs.management.profiles.list");
        assert_eq!(records[1]["redactionFallback"], true);
        assert!(records[1]["response"].get("headers").is_none());
        let text = fs::read_to_string(&path).unwrap().to_ascii_lowercase();
        assert!(!text.contains("fake-secret-100"));
        assert!(!text.contains(&basic.to_ascii_lowercase()));
    }

    /// Every retry of a 503 is a record; the error block has no `Re-run` hint (D69).
    #[test]
    fn retries_record_every_attempt() {
        let f = fx();
        f.server.set_default(
            Route::Profiles,
            Planned::json(503, &json!({"message": "busy"})).with_retry_after(0),
        );
        let out = f.run(LIST);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        let (path, records) = f.only_log();
        let mut expected = vec![("oauth.token", "200")];
        expected.extend([("airs.management.profiles.list", "503"); 6]);
        assert_eq!(summary(&records), pairs(&expected));
        assert_eq!(records[1]["response"]["headers"]["retry-after"], "0");
        assert_eq!(records[1]["response"]["statusText"], "Service Unavailable");
        assert_eq!(
            out.stderr,
            format!(
                "{NOTICE}{}\n\n  ✗ Error: AISEC_SERVER_SIDE_ERROR:API error 503\n    HTTP 503\n\n",
                path.display()
            )
        );
        assert!(!out.stderr.contains(HINT));
    }

    /// A refused token request is one record with masked credentials, and no API record.
    #[test]
    fn token_failure_is_one_masked_record() {
        let f = fx();
        f.server.push_once(Route::Token, Planned::empty_json(401));
        let out = f.run(LIST);
        out.assert_code(1);
        let (_, records) = f.only_log();
        assert_eq!(summary(&records), pairs(&[("oauth.token", "401")]));
        assert_eq!(records[0]["request"]["headers"]["authorization"], "***");
    }

    /// A connection that fails records `Request failed` and never the error text.
    #[test]
    fn connection_failure_records_request_failed() {
        let f = fx_with(|config| {
            config.set("mgmtTokenEndpoint", "http://127.0.0.1:1/oauth/token");
        });
        let out = f.run(LIST);
        out.assert_code(1);
        let (path, records) = f.only_log();
        assert!(!records.is_empty());
        for record in &records {
            assert_eq!(record["operation"], "oauth.token");
            assert_eq!(record["error"], "Request failed");
            assert!(record.get("response").is_none());
        }
        assert_record_shape(&path);
    }

    /// The scan request's credentials and payload hash are masked, and neither the
    /// prompt nor the response text is persisted (D64, D65).
    #[test]
    fn scan_masks_credentials_and_omits_content() {
        let f = fx_with(|config| {
            config.set("airsApiToken", "FAKE-APITOKEN-100");
        });
        let prompt = "debug-prompt-marker-7f3a";
        let response = "debug-response-marker-9c1b";
        let out = f.run(&[
            "--debug",
            "runtime",
            "scan",
            "--profile",
            "fixture-profile",
            "--response",
            response,
            prompt,
        ]);
        out.assert_success();
        f.clean(&[prompt, response, "fixture-profile"]);
        let (path, records) = f.only_log();
        assert_eq!(summary(&records), pairs(&[("airs.scan.sync", "200")]));
        let headers = records[0]["request"]["headers"].as_object().unwrap();
        assert_eq!(headers["x-payload-hash"], "***");
        for name in ["x-pan-token", "authorization"] {
            if let Some(value) = headers.get(name) {
                assert_eq!(value, "***", "{name}");
            }
        }
        assert!(headers.contains_key("x-pan-token") || headers.contains_key("authorization"));
        assert_eq!(headers["content-type"], "application/json");
        assert_record_shape(&path);
        assert!(out.stderr.starts_with(NOTICE));
        assert!(
            out.stderr
                .lines()
                .nth(1)
                .unwrap()
                .contains("Prisma AIRS Runtime Scan"),
            "the notice comes before every other status line"
        );
    }

    /// Doctor records each probe attempt; its report and exit code are unchanged.
    #[test]
    fn doctor_records_every_probe() {
        let f = fx();
        let out = f.run(&["--debug", "doctor", "--output", "json"]);
        let sent = f.server.requests().len();
        f.server.clear_requests();
        let plain = run(
            &rust(),
            &f.sandbox,
            &["doctor", "--output", "json"],
            &RunOptions::new(),
        );
        assert_eq!(out.code, plain.code);
        assert_eq!(out.stdout, plain.stdout);
        assert_eq!(f.server.requests().len(), sent, "the same requests");
        let (path, records) = f.only_log();
        let operations: Vec<String> = summary(&records).into_iter().map(|(op, _)| op).collect();
        assert!(
            operations.contains(&"airs.scan.results".to_owned()),
            "{operations:?}"
        );
        assert!(
            operations.contains(&"oauth.token".to_owned()),
            "{operations:?}"
        );
        assert!(
            operations.contains(&"airs.management.topics.list".to_owned()),
            "{operations:?}"
        );
        assert_eq!(records.len(), sent, "one record per request");
        assert_record_shape(&path);
    }

    /// A request cut short by SIGINT is recorded as cancelled before the exit.
    #[test]
    fn interrupted_request_is_recorded_as_cancelled() {
        let f = fx();
        f.server.set_default(Route::Profiles, Planned::hang());
        let child = spawn(&rust(), &f.sandbox, LIST, &f.options());
        assert!(
            f.server.wait_for_requests(2, Duration::from_secs(20)),
            "the GET never arrived"
        );
        child.send_signal("INT");
        let out = child.wait();
        out.assert_code(130);
        f.clean(&[]);
        let (_, records) = f.only_log();
        assert_eq!(
            summary(&records),
            pairs(&[
                ("oauth.token", "200"),
                (
                    "airs.management.profiles.list",
                    "Request cancelled before a response"
                ),
            ])
        );
    }

    /// A walk records every page.
    #[test]
    fn walk_records_every_page() {
        let f = fx();
        f.profiles(5);
        let out = f.run(&[
            "--debug", "runtime", "profiles", "list", "--all", "--limit", "2", "--output", "json",
        ]);
        out.assert_success();
        let (_, records) = f.only_log();
        let pages = records
            .iter()
            .filter(|record| record["operation"] == "airs.management.profiles.list")
            .count();
        assert_eq!(pages, f.server.requests_for(Route::Profiles).len());
        assert!(pages >= 3);
    }
}

mod files {
    use super::*;

    /// `--quiet` suppresses the notice; the log is still written.
    #[test]
    fn quiet_hides_the_notice_only() {
        let f = fx();
        f.profiles(1);
        let out = f.run(&[
            "--quiet", "--debug", "runtime", "profiles", "list", "--output", "json",
        ]);
        out.assert_success();
        assert_eq!(out.stderr, "");
        assert_eq!(f.only_log().1.len(), 2);
    }

    /// The directory is 0700 when created and the file 0600 (D61).
    #[test]
    fn directory_and_file_are_private() {
        let f = fx();
        f.profiles(1);
        f.run(LIST).assert_success();
        let (path, _) = f.only_log();
        assert_eq!(mode(&log_dir(&f.sandbox)), 0o700);
        assert_eq!(mode(&xdg_state_home(&f.sandbox).join("airs-cli")), 0o700);
        assert_eq!(mode(&path), 0o600);
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(debug_log::is_log_name(name), "{name}");
        // A second run adds a second file; an existing 0755 directory is kept as it is.
        fs::set_permissions(log_dir(&f.sandbox), fs::Permissions::from_mode(0o755)).unwrap();
        f.run(LIST).assert_success();
        assert_eq!(log_files(&log_dir(&f.sandbox)).len(), 2);
        assert_eq!(mode(&log_dir(&f.sandbox)), 0o755);
    }

    /// Without `XDG_STATE_HOME` the log goes under `~/.local/state/airs-cli/debug/`.
    #[test]
    fn default_location_is_under_home() {
        let f = fx();
        f.profiles(1);
        let out = f.run_with(
            &rust(),
            LIST,
            RunOptions::new().allow_writes_under("home/.local/state/airs-cli"),
        );
        out.assert_success();
        let dir = debug_log::default_log_dir(&f.sandbox);
        let files = log_files(&dir);
        assert_eq!(files.len(), 1);
        assert_eq!(notice_path(&out.stderr), files[0]);
        assert_eq!(records(&files[0]).len(), 2);
        assert_eq!(
            f.sandbox
                .home_listing()
                .iter()
                .filter(|path| !path.starts_with(".local/state/airs-cli"))
                .filter(|path| !Path::new(".local/state/airs-cli").starts_with(path))
                .count(),
            0,
            "nothing else under HOME"
        );
    }

    /// The newest 20 logs are kept, this run's included; other names are untouched.
    #[test]
    fn retention_keeps_the_newest_twenty() {
        let f = fx();
        f.profiles(1);
        let dir = log_dir(&f.sandbox);
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        for index in 0..25u64 {
            let name = format!("debug-api-{}-0000000a.jsonl", 1_000_000_000_000 + index);
            fs::write(dir.join(name), b"").unwrap();
        }
        fs::write(dir.join("notes.txt"), b"keep me").unwrap();
        let out = f.run(LIST);
        out.assert_success();
        let files = log_files(&dir);
        assert_eq!(files.len(), 20);
        assert!(files.contains(&notice_path(&out.stderr)));
        assert!(files.contains(&dir.join("debug-api-1000000000024-0000000a.jsonl")));
        assert!(files.contains(&dir.join("debug-api-1000000000006-0000000a.jsonl")));
        assert!(!files.contains(&dir.join("debug-api-1000000000005-0000000a.jsonl")));
        assert_eq!(fs::read(dir.join("notes.txt")).unwrap(), b"keep me");
    }

    fn assert_setup_failure(f: &Fx, out: &Run, dir: &Path) {
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert_eq!(
            out.stderr,
            format!(
                "\n  ✗ Error: Cannot create the debug log in {}; fix that directory or omit --debug\n\n",
                dir.display()
            )
        );
        assert_eq!(f.server.request_count(), 0, "zero requests");
    }

    /// A directory the CLI cannot write to is a setup failure: exit 1, zero requests,
    /// the D68 message naming the directory.
    #[test]
    fn unwritable_directory_is_a_setup_failure() {
        let f = fx();
        let dir = log_dir(&f.sandbox);
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
        let out = f.run(LIST);
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        assert_setup_failure(&f, &out, &dir);
        assert!(log_files(&dir).is_empty());
    }

    /// A group- or world-writable directory, or a symlink, is refused.
    #[test]
    fn unsafe_directories_are_refused() {
        let f = fx();
        let dir = log_dir(&f.sandbox);
        fs::create_dir_all(&dir).unwrap();
        for unsafe_mode in [0o777, 0o720] {
            fs::set_permissions(&dir, fs::Permissions::from_mode(unsafe_mode)).unwrap();
            let out = f.run(LIST);
            assert_setup_failure(&f, &out, &dir);
            assert!(log_files(&dir).is_empty());
        }
        fs::remove_dir(&dir).unwrap();
        let target = f.sandbox.tmp().join("elsewhere");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, &dir).unwrap();
        let out = f.run(LIST);
        assert_setup_failure(&f, &out, &dir);
        assert!(log_files(&target).is_empty());
    }

    /// A relative `XDG_STATE_HOME` and an empty `HOME` are setup failures.
    #[test]
    fn unusable_state_locations_are_setup_failures() {
        let f = fx();
        let out = f.run_with(
            &rust(),
            LIST,
            RunOptions::new().env("XDG_STATE_HOME", "relative/state"),
        );
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            "\n  ✗ Error: XDG_STATE_HOME must be an absolute path\n\n"
        );
        let out = f.run_with(&rust(), LIST, RunOptions::new().env("HOME", ""));
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            "\n  ✗ Error: Cannot create the debug log: HOME is not an absolute path; set XDG_STATE_HOME or omit --debug\n\n"
        );
        assert_eq!(f.server.request_count(), 0);
    }

    /// A write that fails mid-run (the file-size limit makes every write to the open
    /// log fail with EFBIG) never changes the command: same requests, stdout and exit
    /// code; one warning; an incomplete log (D67).
    #[test]
    fn write_failure_keeps_the_outcome() {
        let f = fx();
        f.profiles(2);
        let wrapper = f.sandbox.tmp().join("limited.sh");
        fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\ntrap '' XFSZ\nulimit -f 0\nexec '{}' \"$@\"\n",
                env!("CARGO_BIN_EXE_airs")
            ),
        )
        .unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        let out = f.run_with(&Cli::rust(&wrapper), LIST, f.options());
        out.assert_success();
        let path = notice_path(&out.stderr);
        assert_eq!(
            out.stderr,
            format!("{NOTICE}{}\n{WARNING}  Showing 2\n", path.display())
        );
        assert_eq!(fs::read(&path).unwrap(), b"", "nothing could be written");
        assert_eq!(f.server.requests().len(), 2, "the requests are unchanged");
        let plain = run(&rust(), &f.sandbox, &LIST[1..], &RunOptions::new());
        assert_eq!(plain.stdout, out.stdout);
        // --quiet never hides the warning.
        let quiet = f.run_with(
            &Cli::rust(&wrapper),
            &[
                "--quiet", "--debug", "runtime", "profiles", "list", "--output", "json",
            ],
            f.options(),
        );
        quiet.assert_success();
        assert_eq!(quiet.stderr, WARNING);
    }
}

mod no_log {
    use super::*;

    fn assert_no_state(f: &Fx) {
        assert!(
            !xdg_state_home(&f.sandbox).exists(),
            "no directory, no file"
        );
    }

    /// `tenant *` accepts `--debug` and creates nothing (D63).
    #[test]
    fn tenant_commands_create_nothing() {
        let f = fx();
        for args in [
            &["--debug", "tenant", "list"][..],
            &["tenant", "read", "--debug"],
            &["tenant", "get", "prod", "mgmtTsgId", "--debug"],
            &["tenant", "path", "--debug"],
        ] {
            let out = f.run(args);
            out.assert_success();
            assert!(!out.stderr.contains("Debug:"), "{args:?}: {}", out.stderr);
        }
        assert_no_state(&f);
        assert_eq!(f.server.request_count(), 0);
    }

    /// Usage errors, parse errors, help, version and bare groups create nothing.
    #[test]
    fn usage_and_parse_errors_create_nothing() {
        let f = fx();
        for (args, code) in [
            (
                &["--debug", "runtime", "profiles", "list", "--output", "xml"][..],
                2,
            ),
            (&["--debug", "runtime", "topics", "list", "--limit", "0"], 2),
            (&["--debug", "runtime", "scan", "--profile", "p", ""], 2),
            (&["--debug", "runtime", "profiles", "list", "--bogus"], 2),
            (&["--debug", "doctor", "--help"], 0),
            (&["--debug", "doctor", "--version"], 0),
            (&["--debug", "runtime"], 2),
        ] {
            f.run(args).assert_code(code);
        }
        assert_no_state(&f);
        assert_eq!(f.server.request_count(), 0);
    }

    /// The log is opened before the tenant is read: a tenant error leaves an empty log.
    #[test]
    fn tenant_errors_leave_an_empty_log() {
        let f = fx();
        let out = f.run(&[
            "--debug", "--tenant", "missing", "runtime", "topics", "list",
        ]);
        out.assert_code(1);
        let (path, records) = f.only_log();
        assert!(records.is_empty());
        assert!(
            out.stderr
                .starts_with(&format!("{NOTICE}{}\n", path.display()))
        );
        assert_eq!(f.server.request_count(), 0);
    }

    /// `--debug` repeated, or after the subcommand, is the same flag.
    #[test]
    fn debug_is_global_and_may_repeat() {
        let f = fx();
        f.profiles(1);
        f.run(&[
            "runtime", "--debug", "profiles", "list", "--debug", "--output", "json",
        ])
        .assert_success();
        assert_eq!(f.only_log().1.len(), 2);
    }
}

mod hint {
    use super::*;

    /// The `Re-run with --debug` hint follows the HTTP line only without `--debug` (D69).
    #[test]
    fn hint_only_without_debug() {
        let f = fx();
        f.server
            .set_default(Route::Profiles, Planned::empty_json(404));
        let plain = run(&rust(), &f.sandbox, &LIST[1..], &RunOptions::new());
        plain.assert_code(1);
        assert_eq!(
            plain.stderr,
            format!("\n  ✗ Error: AISEC_CLIENT_SIDE_ERROR:API error 404\n    HTTP 404\n{HINT}\n")
        );
        let debug = f.run(LIST);
        debug.assert_code(1);
        let path = notice_path(&debug.stderr);
        assert_eq!(
            debug.stderr,
            format!(
                "{NOTICE}{}\n\n  ✗ Error: AISEC_CLIENT_SIDE_ERROR:API error 404\n    HTTP 404\n\n",
                path.display()
            )
        );
    }
}
