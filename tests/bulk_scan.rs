//! Executable acceptance tests for `airs-cli-rs runtime bulk-scan` and
//! `runtime resume-poll` (DESIGN §14).
//!
//! Every test runs the real binary against the loopback fixture server with its async
//! scan routes (receipts from a per-server counter, scripted results per prompt marker,
//! held requests released by the test). Each run sets `XDG_STATE_HOME=<sandbox>/tmp/xdg`
//! and an `--output-file` under `tmp/`, so the runner's check that nothing outside
//! `tmp/` changes stays on (the default-name and default-location tests grant their
//! one directory). The TypeScript specs are ported first (`int-spec` #9–#22, DESIGN
//! §14.18; their `resume-poll` halves are in `resume`), then the Rust-only rows: crash
//! points, interruption, the §12 rows D118–D131 and D30.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use airs_test_harness::{
    AsyncRule, Cli, ClosedPort, FixtureConfig, FixtureServer, Planned, RecordedRequest, Route, Run,
    RunOptions, Sandbox, fixtures, payload_hash,
};
use serde_json::{Value, json};

const PROFILE: &str = "bulk-profile";
const API_KEY: &str = "FAKE-APIKEY-100";
const SESSION: &str = "s1";
const HEADER: &str = "prompt,action,category,triggered,topic_violation,injection,toxic_content,dlp,url_cats,malicious_code,source_code,agent,scan_id,report_id,error";
const OUTCOME_UNKNOWN: &str =
    "Outcome unknown: the scan may have been received and processed by AIRS.";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

fn scan_id(n: u64) -> String {
    fixtures::async_scan_id(n)
}

fn report_id(n: u64) -> String {
    fixtures::async_report_id(n)
}

struct Fx {
    server: FixtureServer,
    sandbox: Sandbox,
}

fn fx() -> Fx {
    fx_with(|_| {})
}

fn fx_with(edit: impl FnOnce(&mut FixtureConfig)) -> Fx {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_with(&server, "prod", "100", edit);
    sandbox.register(&[&tenant], Some("prod"));
    Fx { server, sandbox }
}

fn without_tenant() -> Fx {
    Fx {
        server: FixtureServer::start(),
        sandbox: Sandbox::new(),
    }
}

impl Fx {
    fn xdg(&self) -> PathBuf {
        self.sandbox.tmp().join("xdg")
    }

    fn state_dir(&self) -> PathBuf {
        self.xdg().join("airs-cli/bulk-scans")
    }

    fn out_path(&self) -> String {
        self.sandbox.tmp().join("out.csv").display().to_string()
    }

    /// Write an input file under `tmp/` and return its absolute path.
    fn input(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.sandbox.tmp().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.display().to_string()
    }

    fn prompts(&self, prompts: &[String]) -> String {
        self.input("prompts.txt", prompts.join("\n").as_bytes())
    }

    fn options(&self) -> RunOptions {
        RunOptions::new().env("XDG_STATE_HOME", self.xdg().display().to_string())
    }

    fn run(&self, args: &[&str], options: &RunOptions) -> Run {
        let out = airs_test_harness::run(&rust(), &self.sandbox, args, options);
        self.server.assert_no_unknown_routes();
        out
    }

    /// `runtime bulk-scan --profile bulk-profile --file <file> --output-file <tmp/out.csv>
    /// --session-id s1 <extra>`.
    fn bulk(&self, file: &str, extra: &[&str]) -> Run {
        self.bulk_with(file, extra, &self.options())
    }

    fn bulk_with(&self, file: &str, extra: &[&str], options: &RunOptions) -> Run {
        let out = self.out_path();
        let mut args = vec![
            "runtime",
            "bulk-scan",
            "--profile",
            PROFILE,
            "--file",
            file,
            "--output-file",
            &out,
            "--session-id",
            SESSION,
        ];
        args.extend_from_slice(extra);
        self.run(&args, options)
    }

    fn spawn_bulk(
        &self,
        file: &str,
        extra: &[&str],
        options: &RunOptions,
    ) -> airs_test_harness::Running {
        let out = self.out_path();
        let mut args = vec![
            "runtime",
            "bulk-scan",
            "--profile",
            PROFILE,
            "--file",
            file,
            "--output-file",
            &out,
            "--session-id",
            SESSION,
        ];
        args.extend_from_slice(extra);
        airs_test_harness::spawn(&rust(), &self.sandbox, &args, options)
    }

    /// `runtime resume-poll <state> <extra>`.
    fn resume_at(&self, state: &str, extra: &[&str], options: &RunOptions) -> Run {
        let mut args = vec!["runtime", "resume-poll", state];
        args.extend_from_slice(extra);
        self.run(&args, options)
    }

    /// `runtime resume-poll <the one state> <extra>`.
    fn resume(&self, extra: &[&str]) -> Run {
        self.resume_at(&self.state_shown(), extra, &self.options())
    }

    fn spawn_resume(&self, extra: &[&str], options: &RunOptions) -> airs_test_harness::Running {
        let state = self.state_shown();
        let mut args = vec!["runtime", "resume-poll", state.as_str()];
        args.extend_from_slice(extra);
        airs_test_harness::spawn(&rust(), &self.sandbox, &args, options)
    }

    /// The request sequence after the first `mark` requests.
    fn sequence_since(&self, mark: usize) -> Vec<String> {
        self.sequence().split_off(mark)
    }

    /// The job lock next to the state.
    fn lock_path(&self) -> PathBuf {
        let state = self.state_path();
        let name = state.file_name().unwrap().to_string_lossy().into_owned();
        let job = name.strip_suffix(".bulk-scan.json").unwrap().to_owned();
        self.state_dir().join(format!("{job}.lock"))
    }

    /// The names in the state directory (empty when it does not exist).
    fn state_names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.state_dir())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// The one state file.
    fn state_path(&self) -> PathBuf {
        let states: Vec<String> = self
            .state_names()
            .into_iter()
            .filter(|name| name.ends_with(".bulk-scan.json"))
            .collect();
        assert_eq!(
            states.len(),
            1,
            "exactly one state: {:?}",
            self.state_names()
        );
        self.state_dir().join(&states[0])
    }

    fn state(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.state_path()).unwrap()).unwrap()
    }

    fn statuses(&self) -> Vec<String> {
        self.state()["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["status"].as_str().unwrap().to_owned())
            .collect()
    }

    fn csv(&self) -> String {
        std::fs::read_to_string(self.out_path()).unwrap()
    }

    fn posts(&self) -> Vec<RecordedRequest> {
        self.server.requests_for(Route::ScanAsync)
    }

    /// Every POST's `req_id`s.
    fn posted(&self) -> Vec<Vec<u64>> {
        self.posts()
            .iter()
            .map(|post| {
                post.body_json()
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|object| object["req_id"].as_u64().unwrap())
                    .collect()
            })
            .collect()
    }

    /// The request sequence: `P[a-b]` per POST (its req_id ranges), `G` per poll, `R`
    /// per report query.
    fn sequence(&self) -> Vec<String> {
        self.server
            .requests()
            .iter()
            .map(|request| match request.route {
                Route::ScanAsync => {
                    let ids: Vec<u64> = request
                        .body_json()
                        .unwrap()
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|object| object["req_id"].as_u64().unwrap())
                        .collect();
                    format!("P[{}-{}]", ids[0], ids[ids.len() - 1])
                }
                Route::ScanResults => "G".to_owned(),
                Route::ScanReports => "R".to_owned(),
                other => other.label().to_owned(),
            })
            .collect()
    }

    fn state_shown(&self) -> String {
        self.state_path().display().to_string()
    }

    fn resume_detail(&self) -> String {
        format!(
            "The job was saved; resume it with 'airs runtime resume-poll {}'",
            self.state_shown()
        )
    }

    /// Wait (on events, never a timer) until the state file satisfies `ready`.
    fn wait_for_state(&self, ready: impl Fn(&Value) -> bool) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(state) = self
                .state_names()
                .iter()
                .find(|name| name.ends_with(".bulk-scan.json"))
                .and_then(|name| std::fs::read(self.state_dir().join(name)).ok())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                && ready(&state)
            {
                return state;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the state never became ready"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn numbered(count: usize) -> Vec<String> {
    (0..count).map(|n| format!("distinct prompt {n}")).collect()
}

/// A CSV row (every field quoted).
fn row(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|field| format!("\"{}\"", field.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(",")
}

fn allow_row(prompt: &str, n: u64) -> String {
    let id = scan_id(n);
    let report = report_id(n);
    let mut fields = vec![prompt, "allow", "benign", "false"];
    fields.extend(["false"; 8]);
    fields.extend([id.as_str(), report.as_str(), ""]);
    row(&fields)
}

fn status_block(prompts: usize, batches: &str, state: &str, per_batch: usize) -> String {
    let mut text = format!(
        "  Prisma AIRS Bulk Scan\n  Profile:  {PROFILE}\n  Session:  {SESSION}\n  Prompts:  {prompts}\n  Batches:  {batches}\n  State:    {state}\n"
    );
    for batch in 1..=per_batch {
        text.push_str(&format!(
            "  Submitting batch {batch}...\n  Scan IDs saved: {state}\n"
        ));
    }
    text
}

fn summary(total: usize, blocked: usize, allowed: usize, failed: usize, output: &str) -> String {
    format!(
        "\n  Bulk Scan Complete\n\n  Total    {total}\n  Blocked  {blocked}\n  Allowed  {allowed}\n  Failed   {failed}\n  Output   {output}\n"
    )
}

fn resume_status(scan_ids: usize, prompts: usize) -> String {
    format!(
        "  Prisma AIRS Resume Poll\n  Profile:  {PROFILE}\n  Scan IDs: {scan_ids}\n  Prompts:  {prompts}\n"
    )
}

fn resume_summary(
    total: usize,
    blocked: usize,
    allowed: usize,
    failed: usize,
    output: &str,
) -> String {
    format!(
        "\n  Resume Poll Complete\n\n  Total    {total}\n  Blocked  {blocked}\n  Allowed  {allowed}\n  Failed   {failed}\n  Output   {output}\n"
    )
}

fn error_block(message: &str, status: Option<u16>, details: &[&str]) -> String {
    let mut text = format!("\n  ✗ Error: {message}\n");
    if let Some(status) = status {
        text.push_str(&format!(
            "    HTTP {status}\n    Re-run with --debug to capture redacted API diagnostics.\n"
        ));
    }
    for detail in details {
        text.push_str(&format!("    {detail}\n"));
    }
    text.push('\n');
    text
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn block_verdict(flag: &str) -> Value {
    let mut verdict = fixtures::scan_block(PROFILE);
    verdict["prompt_detected"] = json!({"injection": false, flag: true});
    verdict["category"] = json!("malicious");
    verdict
}

// ---------------------------------------------------------------------------------------
// The TypeScript integration spec (`tests/integration/cli/runtime-bulk-scan.spec.ts`)
// ---------------------------------------------------------------------------------------

mod typescript_spec {
    use super::*;

    /// int-spec #9: seven prompts, rows returned out of order: seven rows in input
    /// order with the right actions and detector columns; the TS status lines and
    /// summary bytes.
    #[test]
    fn seven_prompts_with_rows_out_of_order_are_aligned() {
        let fx = fx();
        fx.server.set_async_reversed(true);
        fx.server.add_async_rule(
            "distinct prompt 0",
            AsyncRule::verdict(block_verdict("source_code")),
        );
        fx.server.add_async_rule(
            "distinct prompt 1",
            AsyncRule::verdict(block_verdict("agent")),
        );
        let file = fx.prompts(&numbered(7));
        let out = fx.bulk(&file, &[]);
        out.assert_success();
        let state = fx.state_shown();
        assert_eq!(out.stderr, status_block(7, "1 (size 25)", &state, 1));
        assert_eq!(out.stdout, summary(7, 2, 5, 0, &fx.out_path()));
        let csv = fx.csv();
        let lines: Vec<&str> = csv.split('\n').collect();
        assert_eq!(lines[0], HEADER);
        assert_eq!(lines.len(), 8);
        let block = |prompt: &str, flag: usize| {
            let mut fields = vec![prompt, "block", "malicious", "true"];
            let mut flags = ["false"; 8];
            flags[flag] = "true";
            fields.extend(flags);
            let (id, report) = (scan_id(1), report_id(1));
            fields.extend([id.as_str(), report.as_str(), ""]);
            row(&fields)
        };
        assert_eq!(lines[1], block("distinct prompt 0", 6));
        assert_eq!(lines[2], block("distinct prompt 1", 7));
        for n in 2..7 {
            assert_eq!(lines[n + 1], allow_row(&format!("distinct prompt {n}"), 1));
        }
        assert!(!csv.ends_with('\n'), "no trailing newline");
        assert_eq!(fx.sequence(), ["P[0-6]", "G"]);
    }

    /// int-spec #10 (the bulk-scan half): a poll failure exits 1, names the state and
    /// keeps the accepted receipt.
    #[test]
    fn a_poll_failure_exits_1_and_keeps_the_receipt() {
        let fx = fx();
        fx.server.push_once(
            Route::ScanResults,
            Planned::json(500, &json!({"message": "temporary"})),
        );
        let file = fx.prompts(&numbered(2));
        let out = fx.bulk(&file, &["--quiet"]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                "AISEC_SERVER_SIDE_ERROR:API error 500",
                Some(500),
                &[&fx.resume_detail()]
            )
        );
        assert_eq!(out.stdout, "");
        assert_eq!(fx.statuses(), ["submitted", "submitted"]);
        assert_eq!(fx.csv(), HEADER);
        assert_eq!(fx.sequence(), ["P[0-1]", "G"]);
    }

    /// int-spec #12 (the bulk-scan half): 21 prompts, the second chunk exhausts its
    /// rate-limit retries: six POSTs of `req_id` 20, which stays pending; the accepted
    /// chunk is not polled (TS parity).
    #[test]
    fn rate_limit_exhaustion_leaves_the_chunk_pending() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::builtin());
        for _ in 0..6 {
            fx.server.push_once(
                Route::ScanAsync,
                Planned::json(429, &json!({"message": "slow down"})).with_retry_after(0),
            );
        }
        let file = fx.prompts(&numbered(21));
        let out = fx.bulk(&file, &[]);
        out.assert_code(1);
        assert_eq!(fx.posted()[0], (0..20).collect::<Vec<u64>>());
        assert_eq!(&fx.posted()[1..], vec![vec![20u64]; 6]);
        assert_eq!(fx.server.route_count(Route::ScanResults), 0);
        let mut expected = vec!["submitted"; 20];
        expected.push("pending");
        assert_eq!(fx.statuses(), expected);
        for retry in 1..=5 {
            assert!(
                out.stderr.contains(&format!(
                    "  ⚠ Rate limited while submitting — retry {retry} in 0s...\n"
                )),
                "{}",
                out.stderr
            );
        }
        assert!(out.stderr.ends_with(&error_block(
            "AISEC_CLIENT_SIDE_ERROR:API error 429",
            Some(429),
            &[&fx.resume_detail()]
        )));
        let attempts = fx.state()["attempts"].as_array().unwrap().clone();
        assert_eq!(attempts.len(), 7);
        assert_eq!(attempts[0]["outcome"], "accepted");
        assert!(
            attempts[1..]
                .iter()
                .all(|a| a["outcome"] == "rejected" && a["httpStatus"] == 429)
        );
    }

    /// int-spec #13 (the bulk-scan half): a 4xx-rejected chunk stays pending.
    #[test]
    fn a_rejected_chunk_stays_pending() {
        let fx = fx();
        fx.server.push_once(
            Route::ScanAsync,
            Planned::json(400, &json!({"message": "profile not found"})),
        );
        let file = fx.prompts(&numbered(2));
        let out = fx.bulk(&file, &[]);
        out.assert_code(1);
        assert!(out.stderr.ends_with(&error_block(
            "AISEC_CLIENT_SIDE_ERROR:API error 400",
            Some(400),
            &[&fx.resume_detail()]
        )));
        assert!(
            !out.stderr.contains("profile not found"),
            "no server text (D131)"
        );
        assert_eq!(fx.statuses(), ["pending", "pending"]);
        assert_eq!(fx.posted(), [vec![0, 1]]);
        let attempt = &fx.state()["attempts"][0];
        assert_eq!(attempt["outcome"], "rejected");
        assert_eq!(attempt["error"], "AISEC_CLIENT_SIDE_ERROR:API error 400");
    }

    /// int-spec #14 (the bulk-scan half): a failed row is written with the others, the
    /// summary is printed, then the failure line and exit 1.
    #[test]
    fn a_failed_row_is_kept_and_exits_1() {
        let fx = fx();
        fx.server.add_async_rule("FAILROW", AsyncRule::failed());
        let file = fx.prompts(&["a ok".to_owned(), "b FAILROW".to_owned()]);
        let out = fx.bulk(&file, &[]);
        out.assert_code(1);
        assert_eq!(out.stdout, summary(2, 0, 1, 1, &fx.out_path()));
        assert!(
            out.stderr
                .ends_with("  ✗ 1 prompt(s) failed; successful results were preserved.\n")
        );
        let mut failed = vec!["b FAILROW", "failed", "error", "false"];
        failed.extend(["false"; 8]);
        let id = scan_id(1);
        failed.extend([id.as_str(), "", "AIRS async scan failed"]);
        assert_eq!(
            fx.csv(),
            format!("{HEADER}\n{}\n{}", allow_row("a ok", 1), row(&failed))
        );
        assert_eq!(fx.statuses(), ["complete", "failed"]);
    }

    /// int-spec #15 (the bulk-scan half): a dropped connection after the request was
    /// read is ambiguous: one POST, never resubmitted, the state named.
    #[test]
    fn an_ambiguous_submission_is_saved_as_ambiguous() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::disconnect());
        let file = fx.prompts(&numbered(2));
        let out = fx.bulk(&file, &[]);
        out.assert_code(1);
        assert!(out.stderr.ends_with(&error_block(
            "AISEC_CLIENT_SIDE_ERROR:Network request failed (connection refused, reset or unreachable)",
            None,
            &[
                OUTCOME_UNKNOWN,
                &format!(
                    "Prompts 0-1 will not be resubmitted; the job was saved in {}",
                    fx.state_shown()
                )
            ]
        )), "{}", out.stderr);
        assert_eq!(fx.posted(), [vec![0, 1]]);
        assert_eq!(fx.statuses(), ["ambiguous", "ambiguous"]);
        assert_eq!(fx.state()["attempts"][0]["outcome"], "unknown");
        assert_eq!(fx.server.route_count(Route::ScanResults), 0);
    }

    /// int-spec #15 with a refused connection (G1): the SDK cannot tell it from a reset
    /// after sending, so the chunk is ambiguous too.
    #[test]
    fn a_refused_connection_is_ambiguous() {
        let closed = ClosedPort::new();
        let fx = fx_with(|config| {
            config.set(
                "airsApiEndpoint",
                format!("http://127.0.0.1:{}/scan", closed.port()),
            );
        });
        let file = fx.prompts(&numbered(1));
        let out = fx.bulk(&file, &["--quiet"]);
        out.assert_code(1);
        assert_eq!(fx.statuses(), ["ambiguous"]);
        assert!(out.stderr.contains(OUTCOME_UNKNOWN));
        assert_eq!(fx.server.request_count(), 0);
    }

    /// int-spec #16, **inverted** (D121): an existing output (a file, a symlink, a
    /// directory) is refused before any file is created or anything is sent.
    #[test]
    fn an_existing_output_is_refused_before_anything_d121() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        let existing = fx.sandbox.tmp().join("out.csv");
        std::fs::write(&existing, "precious").unwrap();
        let out = fx.bulk(&file, &[]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "{} already exists; choose another --output-file",
                    fx.out_path()
                ),
                None,
                &[]
            )
        );
        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "precious");
        std::fs::remove_file(&existing).unwrap();
        std::os::unix::fs::symlink(fx.sandbox.tmp().join("target.csv"), &existing).unwrap();
        fx.bulk(&file, &[]).assert_code(1);
        assert!(!fx.sandbox.tmp().join("target.csv").exists());
        std::fs::remove_file(&existing).unwrap();
        std::fs::create_dir(&existing).unwrap();
        fx.bulk(&file, &[]).assert_code(1);
        assert_eq!(fx.server.request_count(), 0);
        assert!(!fx.xdg().exists(), "no state directory was created");
    }

    /// int-spec #17 (the bulk-scan half): an accepted chunk then an ambiguous one.
    #[test]
    fn an_accepted_then_an_ambiguous_chunk() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::builtin());
        fx.server
            .push_once(Route::ScanAsync, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(21));
        let out = fx.bulk(&file, &["--quiet"]);
        out.assert_code(1);
        assert!(
            out.stderr
                .contains("Prompts 20 will not be resubmitted; the job was saved in ")
        );
        let mut expected = vec!["submitted"; 20];
        expected.push("ambiguous");
        assert_eq!(fx.statuses(), expected);
        assert_eq!(fx.sequence(), ["P[0-19]", "P[20-20]"]);
    }

    /// int-spec #18 (the bulk-scan half): an accepted chunk then a rejected one.
    #[test]
    fn an_accepted_then_a_rejected_chunk() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::builtin());
        fx.server
            .push_once(Route::ScanAsync, Planned::json(400, &json!({})));
        let file = fx.prompts(&numbered(21));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let mut expected = vec!["submitted"; 20];
        expected.push("pending");
        assert_eq!(fx.statuses(), expected);
        assert_eq!(fx.sequence(), ["P[0-19]", "P[20-20]"]);
    }

    /// int-spec #19 (the bulk-scan half): a later batch's poll fails; the earlier
    /// batch's rows are kept.
    #[test]
    fn a_later_batch_poll_failure_keeps_earlier_rows() {
        let fx = fx();
        fx.server.push_once(Route::ScanResults, Planned::builtin());
        fx.server
            .push_once(Route::ScanResults, Planned::json(503, &json!({})));
        let file = fx.prompts(&numbered(21));
        let out = fx.bulk(&file, &["--batch-size", "20", "--quiet"]);
        out.assert_code(1);
        assert_eq!(
            fx.sequence(),
            ["P[0-19]", "G", "P[20-20]", "G"],
            "one attempt per poll"
        );
        assert_eq!(fx.csv().split('\n').count(), 21);
        let mut expected = vec!["complete"; 20];
        expected.push("submitted");
        assert_eq!(fx.statuses(), expected);
    }

    /// int-spec #20: rows resolved by a poll are in the state (and the CSV) before the
    /// next request.
    #[test]
    fn results_are_checkpointed_before_the_next_request() {
        let fx = fx();
        fx.server.add_async_rule(
            "slow",
            AsyncRule::verdict(fixtures::scan_allow(PROFILE)).after_pending(1),
        );
        fx.server.push_once(Route::ScanResults, Planned::builtin());
        fx.server.push_once(Route::ScanResults, Planned::held());
        let file = fx.prompts(&["fast".to_owned(), "slow".to_owned()]);
        let running = fx.spawn_bulk(&file, &[], &fx.options());
        assert!(
            fx.server
                .wait_for_route(Route::ScanResults, 2, Duration::from_secs(20))
        );
        assert_eq!(fx.statuses(), ["complete", "submitted"]);
        assert_eq!(fx.csv(), format!("{HEADER}\n{}", allow_row("fast", 1)));
        fx.server.release_held();
        running.wait().assert_success();
        assert_eq!(fx.statuses(), ["complete", "complete"]);
    }

    /// int-spec #21: invalid batch sizes exit 2 with no request and no file.
    #[test]
    fn invalid_batch_sizes_exit_2_with_nothing_created() {
        let fx = fx();
        let file = fx.prompts(&numbered(2));
        for size in ["0", "-1", "1.5", "abc", "007", "9007199254740992", ""] {
            let out = fx.bulk(&file, &["--batch-size", size]);
            out.assert_code(2);
            assert_eq!(
                out.stderr, "\n  ✗ --batch-size must be a positive integer\n\n",
                "{size:?}"
            );
        }
        assert_eq!(fx.server.request_count(), 0);
        assert!(!fx.xdg().exists());
        assert!(!Path::new(&fx.out_path()).exists());
    }

    /// int-spec #22 (the bulk-scan half): logical batches run one after another; every
    /// chunk of a batch is submitted before its receipts are polled (B§5 sequences).
    #[test]
    fn batches_are_sequential() {
        for (count, size, expected) in [
            (
                45,
                "25",
                &["P[0-19]", "P[20-24]", "G", "G", "P[25-44]", "G"][..],
            ),
            (3, "2", &["P[0-1]", "G", "P[2-2]", "G"]),
            (7, "5", &["P[0-4]", "G", "P[5-6]", "G"]),
        ] {
            let fx = fx();
            let file = fx.prompts(&numbered(count));
            let out = fx.bulk(&file, &["--batch-size", size, "--quiet"]);
            out.assert_success();
            assert_eq!(fx.sequence(), expected, "{count}/{size}");
            assert_eq!(fx.csv().split('\n').count(), count + 1);
        }
    }
}

// ---------------------------------------------------------------------------------------
// §14.2, §14.3: argument and input checks (before the tenant; nothing created)
// ---------------------------------------------------------------------------------------

mod inputs {
    use super::*;

    fn usage(message: &str) -> String {
        format!("\n  ✗ {message}\n\n")
    }

    #[test]
    fn argument_checks_need_no_tenant() {
        let fx = without_tenant();
        let file = fx.prompts(&numbered(1));
        let long = "x".repeat(101);
        let opts = fx.options();
        for (args, message) in [
            (vec!["runtime", "bulk-scan", "--profile", "p"], "--file <file> is required".to_owned()),
            (
                vec!["runtime", "bulk-scan", "--profile", "", "--file", &file],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /v1/scan/async/request did not match schema: scan_req.ai_profile: custom".to_owned(),
            ),
            (
                vec!["runtime", "bulk-scan", "--profile", &long, "--file", &file],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /v1/scan/async/request did not match schema: scan_req.ai_profile.profile_name: too_big".to_owned(),
            ),
            (
                vec!["runtime", "bulk-scan", "--profile", "p", "--file", &file, "--session-id", &long],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /v1/scan/async/request did not match schema: scan_req.session_id: too_big".to_owned(),
            ),
            (
                vec!["runtime", "bulk-scan", "--profile", "p", "--file", &file, "--output-file", ""],
                "--output-file needs a path".to_owned(),
            ),
            (
                vec!["runtime", "bulk-scan", "--profile", "p", "--file", &file, "--output", "csv"],
                "Invalid output format 'csv'. Expected: pretty, table, json".to_owned(),
            ),
        ] {
            let out = fx.run(&args, &opts);
            out.assert_code(2);
            assert_eq!(out.stderr, usage(&message), "{args:?}");
            assert_eq!(out.stdout, "");
        }
        // clap: a missing --profile is a usage error too (D1).
        fx.run(&["runtime", "bulk-scan", "--file", &file], &opts)
            .assert_code(2);
        assert!(!fx.xdg().exists());
        assert_eq!(fx.server.request_count(), 0);
    }

    /// D128: the input file is read before the tenant, with fixed messages.
    #[test]
    fn input_file_rules_d128() {
        let fx = without_tenant();
        let opts = fx.options();
        let run = |file: &str| {
            fx.run(
                &[
                    "runtime",
                    "bulk-scan",
                    "--profile",
                    "p",
                    "--file",
                    file,
                    "--debug",
                ],
                &opts,
            )
        };
        let missing = fx.sandbox.tmp().join("missing.txt").display().to_string();
        let cases: Vec<(String, i32, String)> = vec![
            (
                missing.clone(),
                1,
                error_block(
                    &format!(
                        "Cannot read {missing} (missing, not a regular file, unreadable or larger than 16 MiB)"
                    ),
                    None,
                    &[],
                ),
            ),
            (
                fx.input("latin1.txt", b"caf\xe9\n"),
                1,
                error_block(
                    &format!(
                        "{} is not valid UTF-8",
                        fx.sandbox.tmp().join("latin1.txt").display()
                    ),
                    None,
                    &[],
                ),
            ),
            (
                fx.input("unterminated.csv", b"prompt\n\"abc\ndef\n"),
                1,
                error_block(
                    &format!(
                        "{} has an unterminated quoted field",
                        fx.sandbox.tmp().join("unterminated.csv").display()
                    ),
                    None,
                    &[],
                ),
            ),
            (
                fx.input("nocol.csv", b"id,text\n1,hi\n"),
                1,
                error_block("No \"prompt\" column found in CSV header", None, &[]),
            ),
            (
                fx.input("hdr.csv", b"prompt\n"),
                2,
                usage("No prompts found in input file"),
            ),
            (
                fx.input("blank.txt", b" \n\t\n"),
                2,
                usage("No prompts found in input file"),
            ),
            (
                fx.input("many.txt", "p\n".repeat(10_001).as_bytes()),
                1,
                error_block(
                    &format!(
                        "{} has more than 10000 prompts; split it into several jobs",
                        fx.sandbox.tmp().join("many.txt").display()
                    ),
                    None,
                    &[],
                ),
            ),
            (
                fx.input(
                    "big.txt",
                    format!("ok\n{}\n", "x".repeat(2_097_153)).as_bytes(),
                ),
                1,
                error_block(
                    "AISEC_USER_REQUEST_PAYLOAD_ERROR:prompt exceeds max length of 2097152 bytes",
                    None,
                    &["Prompt index 1"],
                ),
            ),
        ];
        for (file, code, expected) in cases {
            let out = run(&file);
            out.assert_code(code);
            assert_eq!(out.stderr, expected, "{file}");
        }
        assert!(
            !fx.xdg().exists(),
            "no debug log and no state for a failing input"
        );
        assert_eq!(fx.server.request_count(), 0);
    }

    /// B§3: the CSV and line parsers' quirks reach the request bodies.
    /// D132: a prompt within the 2 MiB content limit whose control characters JSON-encode
    /// to more than the request budget (serde writes each as `\u00XX`) is refused at
    /// planning, before the tenant is read, with its index; nothing is created or sent.
    #[test]
    fn a_prompt_too_large_once_encoded_is_refused_d132() {
        let fx = without_tenant();
        let mut bytes = b"short one\n".to_vec();
        bytes.push(b'a');
        bytes.extend(std::iter::repeat_n(0x01u8, 2_000_000));
        bytes.extend(b"b\n");
        let file = fx.input("controls.txt", &bytes);
        let out = fx.run(
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                PROFILE,
                "--file",
                &file,
                "--output-file",
                &fx.out_path(),
            ],
            &fx.options(),
        );
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "AISEC_USER_REQUEST_PAYLOAD_ERROR:prompt exceeds the request size limit of {} bytes once JSON-encoded",
                    8 * 1024 * 1024 - 64 * 1024
                ),
                None,
                &["Prompt index 1"]
            )
        );
        assert_eq!(fx.server.request_count(), 0);
        assert!(!fx.xdg().exists(), "no state directory");
        assert!(!Path::new(&fx.out_path()).exists());
    }

    #[test]
    fn parsed_prompts_reach_the_body() {
        let fx = fx();
        let file = fx.input(
            "quirks.CSV",
            b"\xef\xbb\xbfid, PROMPT \r\n1,\"He said \"\"hi\"\"\"\r\n2,\"line1\nline2\"\r\n\r\n3,\"ab\"cd\r\n4, \"a,b\"\r\n",
        );
        fx.bulk(&file, &["--quiet"]).assert_success();
        let body = fx.posts()[0].body_json().unwrap();
        let prompts: Vec<&str> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|object| {
                object["scan_req"]["contents"][0]["prompt"]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(prompts, ["He said \"hi\"", "line1\nline2", "ab", "\"a"]);
    }

    /// The hidden `--input` alias works and warns (TS text, never quiet-suppressed).
    #[test]
    fn the_input_alias_warns() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        let out_path = fx.out_path();
        let out = fx.run(
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                PROFILE,
                "--input",
                &file,
                "--output-file",
                &out_path,
                "--quiet",
            ],
            &fx.options(),
        );
        out.assert_success();
        assert_eq!(
            out.stderr,
            "  ⚠ --input is deprecated and will be removed in v4 — use --file\n"
        );
        assert_eq!(fx.posted(), [vec![0]]);
    }
}

// ---------------------------------------------------------------------------------------
// §14.6, §14.9–§14.12: the request, the state, the files
// ---------------------------------------------------------------------------------------

mod job {
    use super::*;

    /// The body (§14.6), headers and payload hash of a submission.
    #[test]
    fn the_submission_body_and_headers() {
        let fx = fx();
        let file = fx.prompts(&["a".to_owned(), "b BLOCK".to_owned()]);
        fx.bulk(&file, &["--quiet"]).assert_success();
        let post = &fx.posts()[0];
        assert_eq!(
            post.body_json().unwrap(),
            json!([
                {"req_id": 0, "scan_req": {"ai_profile": {"profile_name": PROFILE}, "contents": [{"prompt": "a"}], "session_id": SESSION}},
                {"req_id": 1, "scan_req": {"ai_profile": {"profile_name": PROFILE}, "contents": [{"prompt": "b BLOCK"}], "session_id": SESSION}}
            ])
        );
        assert_eq!(post.header("x-pan-token"), Some(API_KEY));
        assert_eq!(
            post.header("x-payload-hash"),
            Some(payload_hash(API_KEY, &post.body).as_str())
        );
        let poll = &fx.server.requests_for(Route::ScanResults)[0];
        assert_eq!(poll.query_value("scan_ids"), Some(scan_id(1).as_str()));
    }

    /// `--session-id ""` means no session: the banner shows it empty and no object
    /// carries `session_id` (TS parity); without the flag the TS default form is used.
    #[test]
    fn session_ids() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        let out_path = fx.out_path();
        let out = fx.run(
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                PROFILE,
                "--file",
                &file,
                "--output-file",
                &out_path,
                "--session-id",
                "",
            ],
            &fx.options(),
        );
        out.assert_success();
        assert!(out.stderr.contains("  Session:  \n"));
        assert!(
            fx.posts()[0].body_json().unwrap()[0]["scan_req"]
                .get("session_id")
                .is_none()
        );
        assert_eq!(fx.state()["sessionId"], "");
        std::fs::remove_file(&out_path).unwrap();
        let out = fx.run(
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                PROFILE,
                "--file",
                &file,
                "--output-file",
                &out_path,
            ],
            &fx.options(),
        );
        out.assert_success();
        let session = fx.posts()[1].body_json().unwrap()[0]["scan_req"]["session_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let digits = session.strip_prefix("prisma-airs-cli-bulk-").unwrap();
        assert!(
            digits
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase()),
            "{session}"
        );
        assert!(out.stderr.contains(&format!("  Session:  {session}\n")));
    }

    /// state-spec #2, #3, #5 and D118: one `<job>.bulk-scan.json` in
    /// `$XDG_STATE_HOME/airs-cli/bulk-scans/` (0700), the state, CSV and lock 0600, no
    /// temporary, the lock left empty, the Rust-only format with its journal.
    #[test]
    fn the_state_file_d118() {
        let fx = fx();
        let file = fx.prompts(&numbered(3));
        fx.bulk(&file, &["--quiet"]).assert_success();
        let names = fx.state_names();
        assert_eq!(names.len(), 2, "{names:?}");
        let state_name = names
            .iter()
            .find(|n| n.ends_with(".bulk-scan.json"))
            .unwrap();
        let job = state_name.strip_suffix(".bulk-scan.json").unwrap();
        assert_eq!(
            names.iter().find(|n| n.ends_with(".lock")).unwrap(),
            &format!("{job}.lock")
        );
        let bytes = job.as_bytes();
        assert_eq!(bytes.len(), 33, "{job}");
        assert!(
            bytes[..8].iter().all(u8::is_ascii_digit)
                && bytes[8] == b'T'
                && bytes[15] == b'Z'
                && bytes[16] == b'-'
        );
        assert!(
            bytes[17..]
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
        );
        assert_eq!(mode(&fx.state_dir()), 0o700);
        assert_eq!(mode(&fx.state_path()), 0o600);
        assert_eq!(mode(Path::new(&fx.out_path())), 0o600);
        let lock = fx.state_dir().join(format!("{job}.lock"));
        assert_eq!(mode(&lock), 0o600);
        assert_eq!(
            std::fs::read(&lock).unwrap(),
            b"",
            "released with ftruncate"
        );
        let state = fx.state();
        assert_eq!(state["format"], "airs-cli-rs.bulk-scan");
        assert_eq!(state["version"], 1);
        assert_eq!(state["jobId"], job);
        assert_eq!(state["tenant"], "prod");
        assert_eq!(
            state["scanEndpoint"],
            format!("{}/scan", fx.server.base_url())
        );
        assert_eq!(state["profile"], PROFILE);
        assert_eq!(state["sessionId"], SESSION);
        assert_eq!(state["batchSize"], 25);
        assert_eq!(state["output"]["path"], fx.out_path());
        let ino = std::fs::metadata(fx.out_path())
            .map(|m| std::os::unix::fs::MetadataExt::ino(&m))
            .unwrap();
        let next = &state["output"]["next"];
        assert_eq!(
            next["ino"],
            ino.to_string(),
            "the last rewrite is recorded as next"
        );
        assert_eq!(state["attempts"][0]["outcome"], "accepted");
        assert_eq!(state["attempts"][0]["indices"], json!([0, 1, 2]));
        assert_eq!(state["attempts"][0]["scanId"], scan_id(1));
        assert_eq!(state["items"][2]["result"]["reportId"], report_id(1));
        assert!(state["sequence"].as_u64().unwrap() >= 4);
        let text = std::fs::read_to_string(fx.state_path()).unwrap();
        assert!(
            text.starts_with("{\n  \"format\": \"airs-cli-rs.bulk-scan\",\n")
                && text.ends_with("}\n")
        );
    }

    /// The state directory defaults to `~/.local/state/airs-cli/bulk-scans/`.
    #[test]
    fn the_default_state_location() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        let options = RunOptions::new().allow_writes_under("home/.local/state/airs-cli");
        fx.bulk_with(&file, &["--quiet"], &options).assert_success();
        let dir = fx.sandbox.home().join(".local/state/airs-cli/bulk-scans");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        assert_eq!(mode(&dir), 0o700);
    }

    /// D122: the default output is `<profile>-bulk-scan.csv` in the working directory,
    /// with whitespace runs, `/` and controls mapped to `-`.
    #[test]
    fn the_default_name_stays_in_the_cwd_d122() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        let options = fx.options().allow_writes_under("cwd");
        let out = fx.run(
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                "../esc dir/x",
                "--file",
                &file,
                "--quiet",
            ],
            &options,
        );
        out.assert_success();
        let expected = fx.sandbox.cwd().join("..-esc-dir-x-bulk-scan.csv");
        assert_eq!(
            out.stdout,
            format!(
                "  Total    1\n  Blocked  0\n  Allowed  1\n  Failed   0\n  Output   {}\n",
                expected.display()
            )
        );
        assert_eq!(fx.sandbox.cwd_listing().len(), 1);
        assert!(expected.is_file());
    }

    /// D123: a setup failure (the output's directory is missing) leaves no job: no
    /// state, no lock, no output, nothing sent.
    #[test]
    fn a_failed_setup_leaves_no_job_d123() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        let missing = fx.sandbox.tmp().join("nodir/out.csv").display().to_string();
        let out = fx.run(
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                PROFILE,
                "--file",
                &file,
                "--output-file",
                &missing,
            ],
            &fx.options(),
        );
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!("Cannot write {missing}; check that its directory exists and is writable"),
                None,
                &[]
            )
        );
        assert!(fx.state_names().is_empty(), "{:?}", fx.state_names());
        assert!(
            !fx.sandbox.tmp().join("nodir").exists(),
            "no parent directory is created"
        );
        assert_eq!(fx.server.request_count(), 0);
    }

    /// An unsafe state directory is refused before anything is created or sent.
    #[test]
    fn an_unsafe_state_directory_is_refused() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        std::fs::create_dir_all(fx.state_dir()).unwrap();
        std::fs::set_permissions(fx.state_dir(), std::fs::Permissions::from_mode(0o777)).unwrap();
        let out = fx.bulk(&file, &[]);
        out.assert_code(1);
        assert!(out.stderr.contains("Refusing to use "), "{}", out.stderr);
        assert!(fx.state_names().is_empty());
        assert!(!Path::new(&fx.out_path()).exists());
        assert_eq!(fx.server.request_count(), 0);
    }

    /// D120: chunks are split to fit the SDK request limit; every body fits.
    #[test]
    fn large_prompts_split_chunks_d120() {
        let fx = fx();
        let prompts: Vec<String> = (0..5)
            .map(|n| format!("{n}{}", "z".repeat(2_000_000)))
            .collect();
        let file = fx.prompts(&prompts);
        // This functional case serializes/checkpoints ten MiB and makes multiple
        // bounded SDK calls. Allow emulated targets time without changing any
        // production request deadline or the body/chunk assertions below.
        fx.bulk_with(
            &file,
            &["--quiet"],
            &fx.options().timeout(Duration::from_secs(120)),
        )
        .assert_success();
        // 4 × 2,000,000 bytes fit the 8 MiB − 64 KiB budget; a fifth does not.
        assert_eq!(fx.posted(), [vec![0, 1, 2, 3], vec![4]]);
        assert!(
            fx.posts()
                .iter()
                .all(|post| post.body.len() <= 8 * 1024 * 1024)
        );
    }

    /// D121: an output name the file system accepts but whose rewrite temporary
    /// (about 28 bytes longer) it would refuse is the claim failure, before anything
    /// exists or is sent, so no billed prompt is submitted into a job that cannot write
    /// its CSV.
    #[test]
    fn an_output_name_too_long_for_its_rewrites_is_refused_d121() {
        let fx = fx();
        let long = fx
            .sandbox
            .tmp()
            .join(format!("{}.csv", "o".repeat(236)))
            .display()
            .to_string();
        let file = fx.prompts(&numbered(2));
        let out = fx.run(
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                PROFILE,
                "--file",
                &file,
                "--output-file",
                &long,
                "--session-id",
                SESSION,
            ],
            &fx.options(),
        );
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!("Cannot write {long}; check that its directory exists and is writable"),
                None,
                &[]
            )
        );
        assert_eq!(fx.server.request_count(), 0);
        assert!(!Path::new(&long).exists());
        let leftovers: Vec<String> = std::fs::read_dir(fx.sandbox.tmp())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        assert!(
            fx.state_names()
                .iter()
                .all(|name| !name.ends_with(".bulk-scan.json")),
            "no job: {:?}",
            fx.state_names()
        );
    }

    /// A correlation error in a poll response keeps the rows resolved before it (state
    /// and CSV, as TypeScript saves each row), then stops with the detail line.
    #[test]
    fn rows_before_a_correlation_error_are_kept() {
        let fx = fx();
        let mut good = fixtures::scan_allow(PROFILE);
        good["scan_id"] = json!(scan_id(1));
        good["report_id"] = json!(report_id(1));
        let mut bad = good.clone();
        bad["scan_id"] = json!("00000000-0000-4000-8000-00000000ffff");
        fx.server.push_once(
            Route::ScanResults,
            Planned::json(
                200,
                &json!([
                    {"req_id": 0, "status": "complete", "scan_id": scan_id(1), "result": good},
                    {"req_id": 1, "status": "complete", "scan_id": scan_id(1), "result": bad}
                ]),
            ),
        );
        let file = fx.prompts(&numbered(2));
        let out = fx.bulk(&file, &["--quiet"]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "AIRS result correlation mismatch: nested scan ID 00000000-0000-4000-8000-00000000ffff does not match {}",
                    scan_id(1)
                ),
                None,
                &[&fx.resume_detail()]
            )
        );
        assert_eq!(fx.statuses(), ["complete", "submitted"]);
        assert_eq!(
            fx.csv(),
            format!("{HEADER}\n{}", allow_row("distinct prompt 0", 1))
        );
        // A resume polls only the open entry.
        let mark = fx.server.request_count();
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(fx.sequence_since(mark), ["G"]);
        assert_eq!(fx.csv().split('\n').count(), 3);
    }

    /// §14.12: a CSV temporary that cannot be written (here the output's directory
    /// became read-only during a held poll) still checkpoints the rows first, so the
    /// failure's "the results are saved" is true; once the directory is writable again
    /// a resume writes the CSV from the state with no request.
    #[test]
    fn a_failed_csv_rewrite_keeps_the_results_in_the_state() {
        let fx = fx();
        let sub = fx.sandbox.tmp().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).unwrap();
        let output = sub.join("out.csv").display().to_string();
        fx.server.push_once(Route::ScanResults, Planned::held());
        let file = fx.prompts(&numbered(2));
        let running = airs_test_harness::spawn(
            &rust(),
            &fx.sandbox,
            &[
                "runtime",
                "bulk-scan",
                "--profile",
                PROFILE,
                "--file",
                &file,
                "--output-file",
                &output,
                "--session-id",
                SESSION,
                "--quiet",
            ],
            &fx.options(),
        );
        assert!(
            fx.server
                .wait_for_route(Route::ScanResults, 1, Duration::from_secs(20))
        );
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o500)).unwrap();
        fx.server.release_held();
        let out = running.wait();
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).unwrap();
        out.assert_code(1);
        let state = fx.state_shown();
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "Cannot write {output}; the results are saved in {state}: run 'airs runtime resume-poll {state}' to write it again"
                ),
                None,
                &[]
            )
        );
        assert_eq!(fx.statuses(), ["complete", "complete"]);
        assert!(fx.state()["output"]["next"].is_null());
        assert_eq!(std::fs::read_to_string(&output).unwrap(), HEADER);
        let mark = fx.server.request_count();
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(fx.server.request_count(), mark, "no request");
        assert_eq!(
            std::fs::read_to_string(&output).unwrap(),
            format!(
                "{HEADER}\n{}\n{}",
                allow_row("distinct prompt 0", 1),
                allow_row("distinct prompt 1", 1)
            )
        );
    }

    /// D126: a receipt whose scan id is not a UUID makes the chunk ambiguous at once;
    /// it is never polled.
    #[test]
    fn a_non_uuid_receipt_is_ambiguous_d126() {
        let fx = fx();
        fx.server.push_once(
            Route::ScanAsync,
            Planned::json(
                200,
                &json!({"received": "2026-10-17T00:00:00Z", "scan_id": "not-a-uuid"}),
            ),
        );
        let file = fx.prompts(&numbered(2));
        let out = fx.bulk(&file, &["--quiet"]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                "AIRS accepted prompts 0-1, but its receipt has no usable scan id; they will not be resubmitted",
                None,
                &[&format!(
                    "Prompts 0-1 will not be resubmitted; the job was saved in {}",
                    fx.state_shown()
                )]
            )
        );
        assert_eq!(fx.statuses(), ["ambiguous", "ambiguous"]);
        assert_eq!(fx.state()["attempts"][0]["outcome"], "unusableReceipt");
        assert_eq!(fx.server.route_count(Route::ScanResults), 0);
    }

    /// D30 (0.5): a redirected submission is a rejection; it is not followed.
    #[test]
    fn a_redirected_submission_stays_pending_d30() {
        let fx = fx();
        let location = fx.server.url("/scan/v1/scan/async/request");
        fx.server
            .push_once(Route::ScanAsync, Planned::redirect(307, &location));
        let file = fx.prompts(&numbered(1));
        let out = fx.bulk(&file, &["--quiet"]);
        out.assert_code(1);
        assert!(out.stderr.starts_with(&error_block(
            "AISEC_CLIENT_SIDE_ERROR:Redirect responses are not followed",
            Some(307),
            &[&fx.resume_detail()]
        )));
        assert_eq!(fx.statuses(), ["pending"]);
        assert_eq!(fx.posts().len(), 1);
    }

    /// D125: a poll 500 whose body says "rate limit" is not retried.
    #[test]
    fn only_429_is_a_rate_limit_d125() {
        let fx = fx();
        fx.server.push_once(
            Route::ScanResults,
            Planned::json(500, &json!({"message": "rate limit exceeded"})).with_retry_after(0),
        );
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        assert_eq!(fx.server.route_count(Route::ScanResults), 1);
        // A poll 429 is retried (with Retry-After: 0) and the run completes.
        let fx = super::fx();
        fx.server.push_once(
            Route::ScanResults,
            Planned::json(429, &json!({})).with_retry_after(0),
        );
        let out = fx.bulk(&file_for(&fx), &[]);
        out.assert_success();
        assert!(out.stderr.contains("  ⚠ Rate limited — retry 1 in 0s...\n"));
        assert_eq!(fx.server.route_count(Route::ScanResults), 2);
    }

    fn file_for(fx: &Fx) -> String {
        fx.prompts(&numbered(1))
    }

    /// The report fallback: a terminal batch row without `req_id` names the report.
    #[test]
    fn the_report_fallback() {
        let fx = fx();
        fx.server.push_once(
            Route::ScanResults,
            Planned::json(200, &json!([{"status": "complete", "scan_id": scan_id(1)}])),
        );
        fx.server.push_once(
            Route::ScanReports,
            Planned::json(200, &json!([
                {"report_id": report_id(1), "scan_id": scan_id(1), "req_id": 0, "detection_results": [{"detection_service": "pi", "verdict": "malicious", "action": "block"}]},
                {"report_id": report_id(1), "req_id": 1, "detection_results": []}
            ])),
        );
        let file = fx.prompts(&numbered(2));
        fx.bulk(&file, &["--quiet"]).assert_success();
        assert_eq!(fx.sequence(), ["P[0-1]", "G", "R"]);
        assert_eq!(
            fx.server.requests_for(Route::ScanReports)[0].query_value("report_ids"),
            Some(report_id(1).as_str())
        );
        let csv = fx.csv();
        let lines: Vec<&str> = csv.split('\n').collect();
        assert!(
            lines[1].starts_with(
                "\"distinct prompt 0\",\"block\",\"malicious\",\"true\",\"false\",\"true\","
            ),
            "{csv}"
        );
        assert!(
            lines[2].starts_with("\"distinct prompt 1\",\"allow\",\"benign\",\"false\","),
            "{csv}"
        );
    }

    /// D131: server values that carry a secret are withheld in the CSV and the state.
    #[test]
    fn secrets_are_withheld_from_files_d131() {
        let fx = fx();
        let mut verdict = fixtures::scan_allow(PROFILE);
        verdict["category"] = json!(format!("leak {API_KEY}"));
        fx.server.add_async_rule("x", AsyncRule::verdict(verdict));
        let file = fx.prompts(&["x".to_owned()]);
        fx.bulk(&file, &["--quiet"]).assert_success();
        assert!(fx.csv().contains("\"allow\",\"[withheld]\""));
        let text = std::fs::read_to_string(fx.state_path()).unwrap();
        assert!(!text.contains(API_KEY));
        assert!(!fx.csv().contains(API_KEY));
    }

    /// `--debug` records each attempt's status and headers, never a body or a prompt.
    #[test]
    fn the_debug_log_has_no_prompt() {
        let fx = fx();
        let file = fx.prompts(&["SECRET-PROMPT-TEXT".to_owned()]);
        let out = fx.bulk(&file, &["--debug"]);
        out.assert_success();
        assert!(out.stderr.starts_with("  Debug: API log → "));
        let dir = fx.xdg().join("airs-cli/debug");
        let logs = airs_test_harness::debug_log::log_files(&dir);
        assert_eq!(logs.len(), 1);
        let text = std::fs::read_to_string(&logs[0]).unwrap();
        assert!(!text.contains("SECRET-PROMPT-TEXT"));
        let operations: Vec<String> = airs_test_harness::debug_log::records(&logs[0])
            .iter()
            .map(|record| record["operation"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(operations, ["airs.scan.async", "airs.scan.results"]);
    }
}

// ---------------------------------------------------------------------------------------
// §14.14 formats and streams (D130)
// ---------------------------------------------------------------------------------------

mod formats {
    use super::*;

    #[test]
    fn json_table_and_quiet_d130() {
        let fx = fx();
        let file = fx.prompts(&["a".to_owned(), "b".to_owned()]);
        let out = fx.bulk(&file, &["--output", "json"]);
        out.assert_success();
        let state = fx.state_shown();
        assert_eq!(
            out.stdout,
            format!(
                "{{\n  \"profile\": \"{PROFILE}\",\n  \"sessionId\": \"{SESSION}\",\n  \"state\": \"{state}\",\n  \"output\": \"{}\",\n  \"prompts\": 2,\n  \"total\": 2,\n  \"blocked\": 0,\n  \"allowed\": 2,\n  \"failed\": 0\n}}\n",
                fx.out_path()
            )
        );
        assert_eq!(out.stderr, status_block(2, "1 (size 25)", &state, 1));
        let fx = super::fx();
        let file = fx.prompts(&["a".to_owned()]);
        let out = fx.bulk(&file, &["--output", "table", "--quiet"]);
        out.assert_success();
        assert!(
            out.stdout.starts_with(" Key       │ Value"),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains(" failed    │ 0 "));
        assert_eq!(out.stderr, "");
        let fx = super::fx();
        let file = fx.prompts(&["a".to_owned()]);
        let out = fx.bulk(&file, &["--quiet"]);
        out.assert_success();
        assert_eq!(out.stderr, "");
        assert_eq!(
            out.stdout,
            format!(
                "  Total    1\n  Blocked  0\n  Allowed  1\n  Failed   0\n  Output   {}\n",
                fx.out_path()
            )
        );
    }
}

// ---------------------------------------------------------------------------------------
// §14.8 crash points and §14.16 interruption (D129)
// ---------------------------------------------------------------------------------------

mod interruption {
    use super::*;

    /// C1 is durable before the POST arrives: killed while the fixture holds the POST,
    /// the state says `submitting` with the attempt in flight, and nothing else is sent.
    #[test]
    fn a_crash_after_c1_leaves_the_chunk_submitting() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::held());
        let file = fx.prompts(&numbered(2));
        let running = fx.spawn_bulk(&file, &[], &fx.options().expect_signal());
        assert!(
            fx.server
                .wait_for_route(Route::ScanAsync, 1, Duration::from_secs(20))
        );
        assert_eq!(fx.statuses(), ["submitting", "submitting"]);
        assert_eq!(fx.state()["attempts"][0]["outcome"], "inFlight");
        assert!(fx.state()["attempts"][0]["endedAt"].is_null());
        running.send_signal("KILL");
        let out = running.wait();
        assert_eq!(out.signal, Some(9));
        fx.server.release_held();
        assert_eq!(fx.statuses(), ["submitting", "submitting"]);
        assert_eq!(fx.posts().len(), 1);
        assert_eq!(fx.server.route_count(Route::ScanResults), 0);
        // The kernel released the job lock: it can be taken at once.
        let lock = fx.state_path().with_extension("").with_extension("lock");
        drop(airs_test_harness::lock_holder::hold_file(&lock).unwrap());
    }

    /// D129: SIGINT during a held POST cancels it: the chunk is ambiguous (it may have
    /// been received), the state is named, exit 130.
    #[test]
    fn sigint_during_a_submission_is_ambiguous_d129() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::held());
        let file = fx.prompts(&numbered(2));
        let running = fx.spawn_bulk(&file, &["--quiet"], &fx.options());
        assert!(
            fx.server
                .wait_for_route(Route::ScanAsync, 1, Duration::from_secs(20))
        );
        running.send_signal("INT");
        let out = running.wait();
        fx.server.release_held();
        out.assert_code(130);
        assert_eq!(
            out.stderr,
            format!(
                "\n  ✗ Error: Interrupted; prompts 0-1 may already have been submitted (outcome unknown); the job was saved in {}\n\n",
                fx.state_shown()
            )
        );
        assert_eq!(fx.statuses(), ["ambiguous", "ambiguous"]);
        assert_eq!(fx.posts().len(), 1);
    }

    /// SIGTERM during polling: the receipt stays `submitted`, exit 143, the job named.
    #[test]
    fn sigterm_during_polling_saves_the_job() {
        let fx = fx();
        fx.server.push_once(Route::ScanResults, Planned::held());
        let file = fx.prompts(&numbered(2));
        let running = fx.spawn_bulk(&file, &["--quiet"], &fx.options());
        assert!(
            fx.server
                .wait_for_route(Route::ScanResults, 1, Duration::from_secs(20))
        );
        running.send_signal("TERM");
        let out = running.wait();
        fx.server.release_held();
        out.assert_code(143);
        assert_eq!(
            out.stderr,
            format!(
                "\n  ✗ Error: Interrupted; 2 of 2 prompts were submitted; the job was saved: resume it with 'airs runtime resume-poll {}'\n\n",
                fx.state_shown()
            )
        );
        assert_eq!(fx.statuses(), ["submitted", "submitted"]);
    }

    /// D124: a far-future `Retry-After` is capped at 60 s, and the wait ends at once on
    /// a signal; the rate-limited chunk stays pending.
    #[test]
    fn retry_after_is_capped_and_waits_are_cancellable_d124() {
        let fx = fx();
        fx.server.push_once(
            Route::ScanAsync,
            Planned::json(429, &json!({}))
                .with_header("retry-after", "Thu, 31 Dec 2099 23:59:59 GMT"),
        );
        let file = fx.prompts(&numbered(1));
        let running = fx.spawn_bulk(&file, &[], &fx.options());
        fx.wait_for_state(|state| state["attempts"][0]["outcome"] == "rejected");
        let started = std::time::Instant::now();
        running.send_signal("INT");
        let out = running.wait();
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the wait was cut short"
        );
        out.assert_code(130);
        assert!(
            out.stderr
                .contains("  ⚠ Rate limited while submitting — retry 1 in 60s...\n"),
            "{}",
            out.stderr
        );
        assert!(out.stderr.ends_with(&format!(
            "\n  ✗ Error: Interrupted; 0 of 1 prompts were submitted; the job was saved: resume it with 'airs runtime resume-poll {}'\n\n",
            fx.state_shown()
        )), "{}", out.stderr);
        assert_eq!(fx.statuses(), ["pending"]);
        assert_eq!(fx.posts().len(), 1);
    }
}

// ---------------------------------------------------------------------------------------
// `runtime resume-poll` (§14.13): the TypeScript spec's resume halves, then the Rust rows
// ---------------------------------------------------------------------------------------

mod resume {
    use super::*;

    fn ambiguous_refusal(fx: &Fx, lowest: u64, ambiguous: usize, pending: usize) -> String {
        error_block(
            &format!(
                "Cannot safely resubmit prompt {lowest}: its submission outcome is ambiguous. Known accepted results were preserved; inspect {} before taking manual action.",
                fx.state_shown()
            ),
            None,
            &[&format!(
                "{ambiguous} prompt(s) have an unknown submission outcome and {pending} were not submitted; nothing will be resubmitted automatically."
            )],
        )
    }

    /// int-spec #10: a poll failure exits 1; resume polls the saved receipt (no POST)
    /// and completes, with the TS status lines and summary.
    #[test]
    fn a_poll_failure_then_resume_completes() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanResults, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(2));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_success();
        assert_eq!(out.stderr, resume_status(1, 2));
        assert_eq!(out.stdout, resume_summary(2, 0, 2, 0, &fx.out_path()));
        assert_eq!(fx.sequence_since(mark), ["G"]);
        assert_eq!(
            fx.csv(),
            format!(
                "{HEADER}\n{}\n{}",
                allow_row("distinct prompt 0", 1),
                allow_row("distinct prompt 1", 1)
            )
        );
        assert_eq!(fx.statuses(), ["complete", "complete"]);
    }

    /// int-spec #11 and D119: overlapping resumes: the second is refused at once,
    /// naming the first; one POST in total; the lock is released for a later run.
    #[test]
    fn overlapping_resumes_submit_once_d119() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanAsync, Planned::json(400, &json!({})));
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        fx.server.push_once(Route::ScanAsync, Planned::held());
        let first = fx.spawn_resume(&["--quiet"], &fx.options());
        assert!(
            fx.server
                .wait_for_route(Route::ScanAsync, 2, Duration::from_secs(20))
        );
        let second = fx.resume(&[]);
        second.assert_code(1);
        let expected = format!(
            "\n  ✗ Error: Bulk-scan job is already active in process {} on {} since ",
            first.pid(),
            airs_test_harness::lock_holder::host_name()
        );
        assert!(second.stderr.starts_with(&expected), "{}", second.stderr);
        assert!(second.stderr.ends_with(&format!(
            ". Wait for it to finish before resuming {}.\n\n",
            fx.state_shown()
        )));
        assert_eq!(second.stdout, "");
        fx.server.release_held();
        first.wait().assert_success();
        assert_eq!(
            fx.posted(),
            [vec![0], vec![0]],
            "one rejected, one accepted"
        );
        assert_eq!(fx.statuses(), ["complete"]);
        assert_eq!(std::fs::read(fx.lock_path()).unwrap(), b"");
    }

    /// int-spec #12: 21 prompts, the second chunk exhausted its rate limit; resume
    /// polls the accepted receipt, submits only `req_id` 20 and writes 21 rows.
    #[test]
    fn resume_submits_only_the_rate_limited_prompt() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::builtin());
        for _ in 0..6 {
            fx.server.push_once(
                Route::ScanAsync,
                Planned::json(429, &json!({})).with_retry_after(0),
            );
        }
        let file = fx.prompts(&numbered(21));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let mark = fx.server.request_count();
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(fx.sequence_since(mark), ["G", "P[20-20]", "G"]);
        assert_eq!(fx.posted().last().unwrap(), &vec![20]);
        assert_eq!(fx.csv().split('\n').count(), 22);
        assert_eq!(fx.statuses(), vec!["complete"; 21]);
    }

    /// int-spec #13: a 4xx-rejected chunk stays pending; resume submits it once.
    #[test]
    fn resume_submits_a_rejected_chunk_once() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanAsync, Planned::json(400, &json!({})));
        let file = fx.prompts(&numbered(2));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_success();
        assert_eq!(fx.sequence_since(mark), ["P[0-1]", "G"]);
        assert_eq!(out.stderr, resume_status(0, 2));
        let attempts = fx.state()["attempts"].as_array().unwrap().clone();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1]["outcome"], "accepted");
    }

    /// int-spec #14: a failed row is terminal: resume sends nothing, rewrites the same
    /// CSV, prints the summary and the failure line, and exits 1 again.
    #[test]
    fn a_failed_row_exits_1_again_on_resume() {
        let fx = fx();
        fx.server.add_async_rule("FAILROW", AsyncRule::failed());
        let file = fx.prompts(&["a ok".to_owned(), "b FAILROW".to_owned()]);
        fx.bulk(&file, &[]).assert_code(1);
        let csv = fx.csv();
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert_eq!(fx.server.request_count(), mark);
        assert_eq!(out.stdout, resume_summary(2, 0, 1, 1, &fx.out_path()));
        assert!(
            out.stderr
                .ends_with("  ✗ 1 prompt(s) failed; successful results were preserved.\n")
        );
        assert_eq!(fx.csv(), csv);
    }

    /// int-spec #15: an ambiguous (network) submission: resume sends nothing (no POST,
    /// no poll), prints no status line and refuses with the TS message and the counts.
    #[test]
    fn an_ambiguous_submission_is_never_resubmitted() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::disconnect());
        let file = fx.prompts(&numbered(2));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert_eq!(out.stderr, ambiguous_refusal(&fx, 0, 2, 0));
        assert_eq!(out.stdout, "");
        assert_eq!(fx.server.request_count(), mark);
        assert_eq!(fx.statuses(), ["ambiguous", "ambiguous"]);
        assert_eq!(fx.csv(), HEADER);
    }

    /// int-spec #17: an accepted chunk and an ambiguous one: resume polls the accepted
    /// receipt once, writes its 20 rows, makes no POST and exits 1.
    #[test]
    fn accepted_results_are_recovered_before_the_refusal() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::builtin());
        fx.server
            .push_once(Route::ScanAsync, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(21));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert_eq!(fx.sequence_since(mark), ["G"]);
        assert_eq!(out.stderr, ambiguous_refusal(&fx, 20, 1, 0));
        assert_eq!(fx.csv().split('\n').count(), 21);
        let mut expected = vec!["complete"; 20];
        expected.push("ambiguous");
        assert_eq!(fx.statuses(), expected);
    }

    /// int-spec #18: an accepted chunk and a rejected one: resume polls before it
    /// submits; the retried submission is ambiguous, so it ends `complete` ×20 and
    /// `ambiguous`.
    #[test]
    fn resume_polls_before_it_submits() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::builtin());
        fx.server
            .push_once(Route::ScanAsync, Planned::json(400, &json!({})));
        let file = fx.prompts(&numbered(21));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        fx.server
            .push_once(Route::ScanAsync, Planned::json(503, &json!({})));
        let mark = fx.server.request_count();
        let out = fx.resume(&["--quiet"]);
        out.assert_code(1);
        assert_eq!(fx.sequence_since(mark), ["G", "P[20-20]"]);
        assert!(
            out.stderr.ends_with(&error_block(
                "AISEC_SERVER_SIDE_ERROR:API error 503",
                Some(503),
                &[
                    OUTCOME_UNKNOWN,
                    &format!(
                        "Prompts 20 will not be resubmitted; the job was saved in {}",
                        fx.state_shown()
                    )
                ]
            )),
            "{}",
            out.stderr
        );
        let mut expected = vec!["complete"; 20];
        expected.push("ambiguous");
        assert_eq!(fx.statuses(), expected);
        // A further resume sends nothing and refuses.
        let mark = fx.server.request_count();
        fx.resume(&[]).assert_code(1);
        assert_eq!(fx.server.request_count(), mark);
    }

    /// int-spec #19: a later batch's poll fails; resume completes 21 rows; a second
    /// resume rewrites the same bytes with no request.
    #[test]
    fn a_completed_job_resumes_with_no_request() {
        let fx = fx();
        fx.server.push_once(Route::ScanResults, Planned::builtin());
        fx.server
            .push_once(Route::ScanResults, Planned::json(503, &json!({})));
        let file = fx.prompts(&numbered(21));
        fx.bulk(&file, &["--batch-size", "20", "--quiet"])
            .assert_code(1);
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_success();
        assert_eq!(out.stderr, resume_status(2, 21));
        assert_eq!(fx.sequence_since(mark), ["G"]);
        let csv = fx.csv();
        assert_eq!(csv.split('\n').count(), 22);
        let state_before = fx.state();
        let mark = fx.server.request_count();
        let again = fx.resume(&["--debug"]);
        again.assert_success();
        assert_eq!(fx.server.request_count(), mark, "no request");
        assert_eq!(fx.csv(), csv, "byte-identical");
        assert_eq!(again.stdout, out.stdout);
        assert_eq!(fx.state()["items"], state_before["items"]);
        let logs = airs_test_harness::debug_log::log_files(&fx.xdg().join("airs-cli/debug"));
        assert_eq!(logs.len(), 1);
        assert!(airs_test_harness::debug_log::records(&logs[0]).is_empty());
    }

    /// int-spec #22: resume keeps the stored batch size: per logical batch it polls,
    /// submits the pending prompts, then polls again (TS `poll:0, submit:2,3, poll:2,
    /// submit:4,5, poll:4`).
    #[test]
    fn resume_keeps_the_stored_batch_size() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanResults, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(6));
        fx.bulk(&file, &["--batch-size", "2", "--quiet"])
            .assert_code(1);
        assert_eq!(fx.sequence(), ["P[0-1]", "G"]);
        let mark = fx.server.request_count();
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(fx.sequence_since(mark), ["G", "P[2-3]", "G", "P[4-5]", "G"]);
        assert_eq!(fx.csv().split('\n').count(), 7);
    }

    /// §14.8: a crash after C1 leaves `submitting`; resume turns it into `ambiguous`
    /// (its attempt `unknown`), never sends it again, and refuses.
    #[test]
    fn a_crash_after_c1_is_never_resubmitted() {
        let fx = fx();
        fx.server.push_once(Route::ScanAsync, Planned::held());
        let file = fx.prompts(&numbered(2));
        let running = fx.spawn_bulk(&file, &[], &fx.options().expect_signal());
        assert!(
            fx.server
                .wait_for_route(Route::ScanAsync, 1, Duration::from_secs(20))
        );
        running.send_signal("KILL");
        running.wait();
        fx.server.release_held();
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert_eq!(out.stderr, ambiguous_refusal(&fx, 0, 2, 0));
        assert_eq!(fx.server.request_count(), mark);
        assert_eq!(fx.statuses(), ["ambiguous", "ambiguous"]);
        let attempt = &fx.state()["attempts"][0];
        assert_eq!(attempt["outcome"], "unknown");
        assert_eq!(
            attempt["error"],
            "the process stopped before the outcome was recorded"
        );
        assert!(attempt["endedAt"].is_string());
    }

    /// §14.16: SIGINT while polling leaves `submitted`; resume finishes with polls only.
    #[test]
    fn an_interrupted_poll_resumes_without_a_submission() {
        let fx = fx();
        fx.server.push_once(Route::ScanResults, Planned::held());
        let file = fx.prompts(&numbered(2));
        let running = fx.spawn_bulk(&file, &["--quiet"], &fx.options());
        assert!(
            fx.server
                .wait_for_route(Route::ScanResults, 1, Duration::from_secs(20))
        );
        running.send_signal("INT");
        running.wait().assert_code(130);
        fx.server.release_held();
        let mark = fx.server.request_count();
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(fx.sequence_since(mark), ["G"]);
        assert_eq!(fx.posts().len(), 1);
    }

    /// D119 (lock-spec #7, #8): a live holder of the job lock is named and nothing is
    /// sent or written; once it is killed the next resume proceeds with no manual
    /// step, and the lock file stays.
    #[test]
    fn the_job_lock_names_its_holder_and_never_goes_stale_d119() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanResults, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let state_bytes = std::fs::read(fx.state_path()).unwrap();
        let holder = airs_test_harness::lock_holder::spawn_file(
            "resume::lock_holder_helper",
            &fx.lock_path(),
            Duration::from_secs(60),
        );
        let mark = fx.server.request_count();
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert!(
            out.stderr.starts_with(&format!(
                "\n  ✗ Error: Bulk-scan job is already active in process {} on {} since ",
                holder.pid(),
                airs_test_harness::lock_holder::host_name()
            )),
            "{}",
            out.stderr
        );
        assert_eq!(fx.server.request_count(), mark);
        assert_eq!(std::fs::read(fx.state_path()).unwrap(), state_bytes);
        holder.kill();
        fx.resume(&["--quiet"]).assert_success();
        assert!(fx.lock_path().exists(), "the lock file is never deleted");
    }

    /// §14.11: a job lock file removed while `bulk-scan` holds it no longer keeps a
    /// `resume-poll` out, so the holder stops before its next submission (nothing of
    /// that chunk is sent); a resume then finishes the job, every prompt sent once.
    #[test]
    fn a_removed_job_lock_stops_the_holder_before_it_submits() {
        let fx = fx();
        fx.server.push_once(Route::ScanResults, Planned::held());
        let file = fx.prompts(&numbered(2));
        let running = fx.spawn_bulk(&file, &["--quiet", "--batch-size", "1"], &fx.options());
        assert!(
            fx.server
                .wait_for_route(Route::ScanResults, 1, Duration::from_secs(20))
        );
        let lock = fx.lock_path();
        std::fs::remove_file(&lock).unwrap();
        fx.server.release_held();
        let out = running.wait();
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "{} was removed or replaced by another program; the job was stopped",
                    lock.display()
                ),
                None,
                &[&fx.resume_detail()]
            )
        );
        assert_eq!(fx.posted(), [vec![0]]);
        assert_eq!(fx.statuses(), ["complete", "pending"]);
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(fx.posted(), [vec![0], vec![1]]);
        assert!(lock.exists(), "the resume created a new lock file");
    }

    /// §14.13 (TS order): the state path and the job lock come before the tenant, so a
    /// held job is refused naming its holder even when the scanner credentials are
    /// broken, and a missing state is reported without a tenant.
    #[test]
    fn the_state_and_its_lock_come_before_the_tenant() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanResults, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let holder = airs_test_harness::lock_holder::spawn_file(
            "resume::lock_holder_helper",
            &fx.lock_path(),
            Duration::from_secs(60),
        );
        fx.sandbox
            .set_tenant_line("prod", "airsApiKey", "# airsApiKey removed");
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert!(
            out.stderr.starts_with(&format!(
                "\n  ✗ Error: Bulk-scan job is already active in process {} on ",
                holder.pid()
            )),
            "{}",
            out.stderr
        );
        holder.kill();
        let bare = without_tenant();
        let missing = bare.sandbox.tmp().join("nope.json").display().to_string();
        let out = bare.resume_at(&missing, &[], &bare.options());
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "Cannot read {missing} (missing, not a regular file, unreadable or larger than 128 MiB)"
                ),
                None,
                &[]
            )
        );
    }

    /// §14.13 "Details": the classification read opens only a regular file, so a FIFO
    /// argument gets the location refusal at once instead of blocking in `open`.
    #[test]
    fn a_fifo_state_argument_is_refused_at_once_d127() {
        let fx = fx();
        let fifo = fx.sandbox.tmp().join("state.fifo");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap();
        assert!(made.success());
        let shown = fifo.display().to_string();
        let out = fx.resume_at(&shown, &[], &fx.options().timeout(Duration::from_secs(20)));
        out.assert_code(1);
        let dir = fx.state_dir();
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "{shown} is not an airs bulk-scan job in {}; resume-poll resumes only jobs in its state directory (a moved or copied state cannot be resumed)",
                    dir.display()
                ),
                None,
                &[]
            )
        );
        assert_eq!(fx.server.request_count(), 0);
    }

    /// The lock-holder helper process (`lock_holder::spawn_file`).
    #[test]
    #[ignore = "helper process for the lock tests"]
    fn lock_holder_helper() {
        airs_test_harness::lock_holder::run_from_env();
    }

    /// D118: a TypeScript state is refused with its own message, outside the state
    /// directory and inside it; nothing is sent.
    #[test]
    fn a_typescript_state_is_refused_d118() {
        let fx = fx();
        let ts_state = json!({
            "version": 2, "profile": PROFILE, "sessionId": SESSION, "batchSize": 25,
            "outputFile": fx.out_path(), "createdAt": "2026-10-17T00:00:00.000Z",
            "updatedAt": "2026-10-17T00:00:00.000Z", "items": []
        })
        .to_string();
        let outside = fx.input("ts.bulk-scan.json", ts_state.as_bytes());
        let message = |path: &str| {
            error_block(
                &format!(
                    "{path} is a TypeScript airs-cli bulk-scan state; airs cannot resume it. Resume it with the TypeScript airs-cli, or start a new job with 'airs runtime bulk-scan'."
                ),
                None,
                &[],
            )
        };
        let out = fx.resume_at(&outside, &[], &fx.options());
        out.assert_code(1);
        assert_eq!(out.stderr, message(&outside));
        std::fs::create_dir_all(fx.state_dir()).unwrap();
        std::fs::set_permissions(fx.state_dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let inside = fx
            .state_dir()
            .join("20261017T120000Z-0123456789abcdef.bulk-scan.json");
        std::fs::write(&inside, &ts_state).unwrap();
        std::fs::set_permissions(&inside, std::fs::Permissions::from_mode(0o600)).unwrap();
        let canonical = inside.canonicalize().unwrap().display().to_string();
        let out = fx.resume_at(&inside.display().to_string(), &[], &fx.options());
        out.assert_code(1);
        assert_eq!(out.stderr, message(&canonical));
        assert_eq!(std::fs::read_to_string(&inside).unwrap(), ts_state);
        assert_eq!(fx.server.request_count(), 0);
    }

    /// D127: a copied or moved state, a renamed one, another tenant and another scan
    /// endpoint are refused before anything is sent or written.
    #[test]
    fn only_the_jobs_own_state_under_its_tenant_resumes_d127() {
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        let prod = sandbox.add_fixture_tenant_with(&server, "prod", "100", |_| {});
        let other = sandbox.add_fixture_tenant_with(&server, "other", "100", |_| {});
        sandbox.register(&[&prod, &other], Some("prod"));
        let fx = Fx { server, sandbox };
        fx.server
            .push_once(Route::ScanAsync, Planned::json(400, &json!({})));
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let state = fx.state_shown();
        let bytes = std::fs::read(fx.state_path()).unwrap();
        let dir = fx.state_dir().canonicalize().unwrap().display().to_string();
        // A copy outside the state directory.
        let copy = fx.input("copy.bulk-scan.json", &bytes);
        let out = fx.resume_at(&copy, &[], &fx.options());
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "{copy} is not an airs bulk-scan job in {dir}; resume-poll resumes only jobs in its state directory (a moved or copied state cannot be resumed)"
                ),
                None,
                &[]
            )
        );
        // A copy in the state directory under another job id.
        let renamed = fx
            .state_dir()
            .join("20261017T120000Z-00000000000000ff.bulk-scan.json");
        std::fs::write(&renamed, &bytes).unwrap();
        std::fs::set_permissions(&renamed, std::fs::Permissions::from_mode(0o600)).unwrap();
        let out = fx.resume_at(&renamed.display().to_string(), &[], &fx.options());
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "Invalid bulk-scan state {}: jobId does not match the file name",
                    renamed.canonicalize().unwrap().display()
                ),
                None,
                &[]
            )
        );
        std::fs::remove_file(&renamed).unwrap();
        // A file name that is not a job.
        let odd = fx.state_dir().join("notes.json");
        std::fs::write(&odd, &bytes).unwrap();
        fx.resume_at(&odd.display().to_string(), &[], &fx.options())
            .assert_code(1);
        std::fs::remove_file(&odd).unwrap();
        // Another tenant.
        let out = fx.resume(&["--tenant", "other"]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "{state} was started with tenant prod; select that tenant (for example with --tenant prod) to resume it"
                ),
                None,
                &[]
            )
        );
        // Another scan endpoint.
        let moved = fx.server.url("/scan-moved");
        fx.sandbox.set_tenant_line(
            "prod",
            "airsApiEndpoint",
            &format!("airsApiEndpoint = \"{moved}\""),
        );
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "{state} was started against {}/scan, but tenant prod now uses {moved}; restore its airsApiEndpoint to resume it",
                    fx.server.base_url()
                ),
                None,
                &[]
            )
        );
        assert_eq!(fx.posts().len(), 1, "nothing was sent by any resume");
        assert_eq!(std::fs::read(fx.state_path()).unwrap(), bytes);
        // A missing argument file.
        let missing = fx.sandbox.tmp().join("missing.json").display().to_string();
        let out = fx.resume_at(&missing, &[], &fx.options());
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "Cannot read {missing} (missing, not a regular file, unreadable or larger than 128 MiB)"
                ),
                None,
                &[]
            )
        );
    }

    /// D121: `--output-file` to another existing file is refused before anything; to a
    /// new path it re-points the job (the old file is kept); a removed output is
    /// claimed again; a replaced one is refused with the identity message.
    #[test]
    fn resume_output_rules_d121() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanResults, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        let state = fx.state_shown();
        // Another, existing file.
        let precious = fx.input("precious.csv", b"precious");
        let mark = fx.server.request_count();
        let out = fx.resume(&["--output-file", &precious]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!("{precious} already exists; choose another --output-file"),
                None,
                &[]
            )
        );
        assert_eq!(std::fs::read_to_string(&precious).unwrap(), "precious");
        assert_eq!(fx.server.request_count(), mark);
        // The recorded output replaced by another file: refused before anything.
        let original = std::fs::read(fx.out_path()).unwrap();
        let edited = fx.input("edited.csv", b"edited");
        std::fs::rename(&edited, fx.out_path()).unwrap();
        let out = fx.resume(&[]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "{} is not the file this job wrote (it was replaced, moved or edited); the results are saved in {state}: resume with --output-file <new path>",
                    fx.out_path()
                ),
                None,
                &[]
            )
        );
        assert_eq!(fx.server.request_count(), mark);
        // A new path: claimed and recorded; the old file is left as it is.
        let fresh = fx.sandbox.tmp().join("fresh.csv").display().to_string();
        let out = fx.resume(&["--output-file", &fresh, "--quiet"]);
        out.assert_success();
        assert!(out.stdout.ends_with(&format!("  Output   {fresh}\n")));
        assert_eq!(fx.state()["output"]["path"], fresh);
        assert_eq!(std::fs::read_to_string(fx.out_path()).unwrap(), "edited");
        assert_eq!(
            std::fs::read_to_string(&fresh).unwrap().split('\n').count(),
            2
        );
        assert_eq!(mode(Path::new(&fresh)), 0o600);
        assert_ne!(original, std::fs::read(&fresh).unwrap());
        // The same path again is the job's own file: rewritten, not refused.
        fx.resume(&["--output-file", &fresh, "--quiet"])
            .assert_success();
        // Removed by the user: claimed again with the same bytes.
        let bytes = std::fs::read(&fresh).unwrap();
        std::fs::remove_file(&fresh).unwrap();
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(std::fs::read(&fresh).unwrap(), bytes);
    }

    /// §14.10, §14.12: a job's stale temporaries (its state's and its CSV's recorded
    /// one) are removed under the lock; others are left alone.
    #[test]
    fn stale_temporaries_are_swept() {
        let fx = fx();
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_success();
        let name = fx
            .state_path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let stale = fx
            .state_dir()
            .join(format!(".{name}.4242.0123456789abcdef.tmp"));
        let foreign = fx
            .state_dir()
            .join(".20261017T120000Z-00000000000000ff.bulk-scan.json.1.0123456789abcdef.tmp");
        for path in [&stale, &foreign] {
            std::fs::write(path, "x").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        fx.resume(&["--quiet"]).assert_success();
        assert!(!stale.exists());
        assert!(foreign.exists());
        // The CSV is rewritten through a temporary that never stays behind.
        let leftovers: Vec<_> = std::fs::read_dir(fx.sandbox.tmp())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    /// §14.2: `--output-file ""` is a usage error before the tenant; `--output csv` too;
    /// clap requires the state argument (D1).
    #[test]
    fn usage_errors_need_no_tenant() {
        let fx = without_tenant();
        let opts = fx.options();
        for (args, message) in [
            (
                vec!["runtime", "resume-poll", "x.json", "--output-file", ""],
                "--output-file needs a path",
            ),
            (
                vec!["runtime", "resume-poll", "x.json", "--output", "csv"],
                "Invalid output format 'csv'. Expected: pretty, table, json",
            ),
        ] {
            let out = fx.run(&args, &opts);
            out.assert_code(2);
            assert_eq!(out.stderr, format!("\n  ✗ {message}\n\n"), "{args:?}");
        }
        fx.run(&["runtime", "resume-poll"], &opts).assert_code(2);
        assert!(!fx.xdg().exists());
        assert_eq!(fx.server.request_count(), 0);
    }

    /// D130: the json summary of a resume; `--quiet` keeps only the data.
    #[test]
    fn resume_formats_d130() {
        let fx = fx();
        let file = fx.prompts(&numbered(2));
        fx.bulk(&file, &["--quiet"]).assert_success();
        let out = fx.resume(&["--output", "json", "--quiet"]);
        out.assert_success();
        assert_eq!(out.stderr, "");
        let summary = out.json();
        assert_eq!(summary["state"], fx.state_shown());
        assert_eq!(summary["output"], fx.out_path());
        assert_eq!(summary["prompts"], 2);
        assert_eq!(summary["allowed"], 2);
        let out = fx.resume(&["--output", "table", "--quiet"]);
        out.assert_success();
        assert!(out.stdout.contains(" total     │ 2 "), "{}", out.stdout);
    }

    /// D129: SIGTERM while a resume polls: exit 143 naming the job; a later resume
    /// finishes it.
    #[test]
    fn an_interrupted_resume_names_the_job_d129() {
        let fx = fx();
        fx.server
            .push_once(Route::ScanResults, Planned::json(500, &json!({})));
        let file = fx.prompts(&numbered(1));
        fx.bulk(&file, &["--quiet"]).assert_code(1);
        fx.server.push_once(Route::ScanResults, Planned::held());
        let running = fx.spawn_resume(&["--quiet"], &fx.options());
        assert!(
            fx.server
                .wait_for_route(Route::ScanResults, 2, Duration::from_secs(20))
        );
        running.send_signal("TERM");
        let out = running.wait();
        fx.server.release_held();
        out.assert_code(143);
        assert_eq!(
            out.stderr,
            format!(
                "\n  ✗ Error: Interrupted; 1 of 1 prompts were submitted; the job was saved: resume it with 'airs runtime resume-poll {}'\n\n",
                fx.state_shown()
            )
        );
        fx.resume(&["--quiet"]).assert_success();
        assert_eq!(fx.posts().len(), 1);
    }
}
