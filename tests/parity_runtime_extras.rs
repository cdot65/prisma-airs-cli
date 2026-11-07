//! TS-vs-Rust parity for the M6d step-1 runtime extras reads (DESIGN §18.1, §18.4–§18.11):
//! `runtime api-keys list`, `customer-apps list|get|consumption`, `deployment-profiles list`
//! and the `scan-logs query` stub.
//!
//! Each scenario runs once against airs-cli-rs and once against the TypeScript reference,
//! each on a fresh [`FixtureServer`] seeded with the runtime extras dataset and a fresh
//! [`Sandbox`] whose tenant points `mgmtEndpoint` and `mgmtDashboardEndpoint` at the server
//! (the TypeScript client passes both explicitly, TX§0). `parity` tests compare exit codes,
//! stdout bytes, stderr and the normalised requests; the parity datasets carry the keys
//! TypeScript reads (`created_at`, `expires_at`, `customer_app_id`), so both CLIs project
//! the same values. `divergence` tests pin both sides of each §12 row this step implements.
//! The TypeScript side prints the fixture markers (auth codes, a listed key), so its runs
//! disable the marker check; the airs-cli-rs runs keep it.

use std::time::Duration;

use airs_test_harness::fixtures::runtime as fx;
use airs_test_harness::{
    Cli, FixtureServer, NormalizedRequest, Planned, Route, Run, RunOptions, RuntimeData, Rx,
    Sandbox, normalize_all, require_ts, run, spawn,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

const TSG: &str = "100";
const BANNER: &str =
    "\n  Prisma AIRS — Runtime Configuration\n  Security profile and topic management\n\n";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

/// One CLI's run of a scenario.
struct Side {
    run: Run,
    requests: Vec<NormalizedRequest>,
    lines: Vec<String>,
}

/// The parity dataset: the seeded one with the keys TypeScript reads.
fn ts_readable(data: &mut RuntimeData) {
    data.api_keys = (1..=3).map(fx::api_key_ts).collect();
    data.customer_apps = (101..=103).map(fx::customer_app_ts).collect();
}

fn unchanged(_: &mut RuntimeData) {}

fn exec_with(
    cli: &Cli,
    args: &[&str],
    seed: &dyn Fn(&mut RuntimeData),
    plan: &dyn Fn(&FixtureServer),
    options: &RunOptions,
) -> Side {
    let server = FixtureServer::start();
    server.seed_runtime();
    server.runtime(|data| seed(data));
    plan(&server);
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", TSG);
    let mut full = vec!["runtime"];
    full.extend_from_slice(args);
    let options = if cli.is_ts() {
        options.clone().allow_secret_output()
    } else {
        options.clone()
    };
    let out = run(cli, &sandbox, &full, &options);
    let raw = server.requests();
    Side {
        run: out,
        requests: normalize_all(&raw),
        lines: raw.iter().map(|request| request.line()).collect(),
    }
}

fn exec(cli: &Cli, args: &[&str], seed: &dyn Fn(&mut RuntimeData)) -> Side {
    exec_with(cli, args, seed, &|_| {}, &RunOptions::new())
}

/// Both sides (`None` after a `PARITY SKIPPED` notice).
fn sides_seeded(
    name: &str,
    args: &[&str],
    seed: &dyn Fn(&mut RuntimeData),
) -> Option<(Side, Side)> {
    let ts = require_ts(name)?;
    Some((exec(&rust(), args, seed), exec(&ts, args, seed)))
}

fn sides(name: &str, args: &[&str]) -> Option<(Side, Side)> {
    sides_seeded(name, args, &ts_readable)
}

fn context(r: &Side, t: &Side) -> String {
    format!("\n--- rust ---\n{:?}\n--- ts ---\n{:?}", r.run, t.run)
}

/// Byte parity: exit code, stdout, stderr and the requests.
fn assert_bytes(r: &Side, t: &Side) {
    assert_eq!(r.run.code, t.run.code, "exit codes differ{}", context(r, t));
    assert_eq!(
        r.run.stdout,
        t.run.stdout,
        "stdout differs{}",
        context(r, t)
    );
    assert_eq!(
        r.run.stderr,
        t.run.stderr,
        "stderr differs{}",
        context(r, t)
    );
    assert_eq!(r.requests, t.requests, "requests differ{}", context(r, t));
}

/// The API requests of a side (the token request left out).
fn api(side: &Side) -> Vec<String> {
    side.lines
        .iter()
        .filter(|line| !line.starts_with("POST /oauth/token"))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Parity
// ---------------------------------------------------------------------------

mod parity {
    use super::*;

    /// Every read in every format, byte for byte, with the requests.
    #[test]
    fn reads_match_typescript() {
        for args in [
            &["api-keys", "list"][..],
            &["api-keys", "ls", "--output", "table"],
            &["api-keys", "list", "--output", "json"],
            &["api-keys", "list", "--limit", "2", "--offset", "1"],
            &[
                "api-keys", "list", "--all", "--limit", "1", "--output", "json",
            ],
            &["customer-apps", "list"],
            &["customer-apps", "ls", "--output", "table"],
            &["customer-apps", "list", "--output", "json"],
            &["customer-apps", "get", "app-1"],
            &["customer-apps", "get", "app-2", "--output", "json"],
            &["customer-apps", "get", "app-3", "--output", "table"],
            &["deployment-profiles", "list"],
            &[
                "deployment-profiles",
                "ls",
                "--unactivated",
                "--output",
                "table",
            ],
            &["deployment-profiles", "list", "--output", "json"],
            &["customer-apps", "consumption", "chatbot"],
            &[
                "customer-apps",
                "consumption",
                "litellm",
                "--time-interval",
                "30",
                "--output",
                "json",
            ],
            &[
                "customer-apps",
                "consumption",
                "LiteLLM",
                "--output",
                "table",
            ],
        ] {
            let Some((r, t)) = sides("reads_match_typescript", args) else {
                return;
            };
            assert_bytes(&r, &t);
        }
    }

    /// TS `rr.ts:562-566`: an absent `status` is `''` in json and table, and an empty
    /// `dp_name` prints empty in pretty (only an absent name would be `unknown`).
    #[test]
    fn deployment_profiles_without_status_or_name_match_typescript() {
        let seed = |data: &mut RuntimeData| {
            ts_readable(data);
            data.deployment_profiles[0]
                .as_object_mut()
                .unwrap()
                .remove("status");
            data.deployment_profiles[1]["dp_name"] = json!("");
        };
        for args in [
            &["deployment-profiles", "list"][..],
            &["deployment-profiles", "list", "--output", "json"],
            &["deployment-profiles", "list", "--output", "table"],
        ] {
            let Some((r, t)) = sides_seeded(
                "deployment_profiles_without_status_or_name_match_typescript",
                args,
                &seed,
            ) else {
                return;
            };
            assert_bytes(&r, &t);
        }
    }

    /// An HTTP error is the same exit code and `HTTP` line (the message text is D11).
    #[test]
    fn errors_match_in_shape() {
        let Some(ts) = require_ts("errors_match_in_shape") else {
            return;
        };
        for cli in [rust(), ts] {
            let side = exec(&cli, &["customer-apps", "get", "nope"], &ts_readable);
            side.run.assert_code(1);
            assert!(
                side.run.stderr.contains("    HTTP 404\n"),
                "{}",
                cli.label()
            );
        }
    }

    /// D338: Rust discovers through dashboard sessions; the frozen reference keeps its stub.
    #[test]
    fn d338_scan_log_discovery_replaces_stub() {
        let args = ["scan-logs", "query", "--interval", "24", "--unit", "hours"];
        let Some((r, t)) = sides("d338_scan_log_discovery_replaces_stub", &args) else {
            return;
        };
        assert_eq!(t.run.code, 1);
        assert!(t.run.stdout.is_empty());
        assert!(t.lines.is_empty());
        assert!(t.run.stderr.contains("broken and under refactor"));
        assert_eq!(r.run.code, 1);
        assert!(!r.lines.is_empty());
        assert!(r.run.stdout.contains("coverage"));
        assert!(!r.run.stderr.contains("broken and under refactor"));
        let args = [
            "scan-logs",
            "query",
            "--interval",
            "1",
            "--unit",
            "x",
            "--page",
            "4",
        ];
        let Some((r, t)) = sides("d338_scan_log_discovery_validation", &args) else {
            return;
        };
        assert_eq!(r.run.code, 2);
        assert_eq!(t.run.code, 1);
        assert!(r.lines.is_empty());
        assert!(t.lines.is_empty());
    }

    /// The tenant `defaultOutput` applies to the reads on both sides (choice 18).
    #[test]
    fn default_output_applies_to_the_reads() {
        let Some(ts) = require_ts("default_output_applies_to_the_reads") else {
            return;
        };
        for cli in [rust(), ts] {
            let server = FixtureServer::start();
            server.seed_runtime();
            let sandbox = Sandbox::new();
            let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, |config| {
                config.set("defaultOutput", "json");
            });
            sandbox.register(&[&tenant], Some("prod"));
            let options = if cli.is_ts() {
                RunOptions::new().allow_secret_output()
            } else {
                RunOptions::new()
            };
            let out = run(
                &cli,
                &sandbox,
                &["runtime", "deployment-profiles", "list"],
                &options,
            );
            out.assert_success();
            assert_eq!(out.json()[0]["name"], "dp-1", "{}", cli.label());
        }
    }

    /// Every runtime-extras group and step-1 leaf carries TypeScript's descriptions and
    /// option texts (commander's doubled `(default: …)` printed once, §18.2).
    #[test]
    fn help_texts_match_typescript() {
        let Some(ts) = require_ts("help_texts_match_typescript") else {
            return;
        };
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        sandbox.single_tenant(&server, "prod", TSG);
        let help = |cli: &Cli, path: &[&str]| {
            let mut args = path.to_vec();
            args.push("--help");
            let out = run(cli, &sandbox, &args, &RunOptions::new());
            out.assert_success();
            out.stdout
        };
        let squash = |text: &str| {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            // commander prints `(default: 100) (default: 100)` for the list flags.
            text.replace("(default: 100) (default: 100)", "(default: 100)")
                .replace("(default: 0) (default: 0)", "(default: 0)")
        };
        let section = |text: &str, title: &str| -> Vec<String> {
            let mut lines = text.lines();
            if !lines.any(|line| line == title) {
                return Vec::new();
            }
            lines
                .take_while(|line| !line.trim().is_empty())
                .map(str::to_owned)
                .collect()
        };
        let entries = |text: &str, title: &str| -> Vec<(String, String)> {
            let mut out: Vec<(String, String)> = Vec::new();
            for line in section(text, title) {
                let trimmed = line.trim_start();
                let indent = line.len() - trimmed.len();
                if indent <= 6 {
                    let (spec, description) = trimmed.split_once("  ").unwrap_or((trimmed, ""));
                    let name = spec
                        .split([',', ' '])
                        .find(|part| {
                            if title == "Options:" {
                                part.starts_with("--")
                            } else {
                                !part.is_empty()
                            }
                        })
                        .unwrap_or(spec);
                    let name = name.split('|').next().unwrap_or(name).to_owned();
                    out.push((name, squash(description)));
                } else if let Some((_, description)) = out.last_mut() {
                    *description = squash(&format!("{description} {trimmed}"));
                }
            }
            out
        };
        let mut problems = Vec::new();
        // The `runtime` group lists the M6d groups of this step in TS order.
        let built = [
            "api-keys",
            "customer-apps",
            "dashboard",
            "deployment-profiles",
            "sessions",
        ];
        let (t, r) = (help(&ts, &["runtime"]), help(&rust(), &["runtime"]));
        let ts_subs: Vec<(String, String)> = entries(&t, "Commands:")
            .into_iter()
            .filter(|(name, _)| built.contains(&name.as_str()))
            .collect();
        let rust_subs: Vec<(String, String)> = entries(&r, "Commands:")
            .into_iter()
            .filter(|(name, _)| built.contains(&name.as_str()))
            .collect();
        if ts_subs != rust_subs {
            problems.push(format!("runtime: {rust_subs:?} vs {ts_subs:?}"));
        }
        // Each group lists its commands with TS's descriptions (clap appends
        // ` [aliases: ls]`; commander shows `list|ls` in the name column); the step-2
        // writes are listed too (their leaves are checked in
        // `parity_runtime_extras_writes.rs`).
        let step_one = [
            "create",
            "delete",
            "regenerate",
            "update",
            "list",
            "get",
            "consumption",
            "query",
            "applications",
            "application",
            "application-violations",
            "top-applications",
            "violations-trend",
            "apps-list",
            "chart",
            "transaction",
            "scan-content",
        ];
        for group in built {
            let path = ["runtime", group];
            let (t, r) = (help(&ts, &path), help(&rust(), &path));
            let ts_subs: Vec<(String, String)> = entries(&t, "Commands:")
                .into_iter()
                .filter(|(name, _)| step_one.contains(&name.as_str()))
                .collect();
            let rust_subs: Vec<(String, String)> = entries(&r, "Commands:")
                .into_iter()
                .filter(|(name, _)| name != "help")
                .map(|(name, text)| {
                    (
                        name,
                        text.split(" [aliases:")
                            .next()
                            .unwrap_or_default()
                            .to_owned(),
                    )
                })
                .collect();
            if ts_subs != rust_subs {
                problems.push(format!("{group}: {rust_subs:?} vs {ts_subs:?}"));
            }
        }
        let leaves: [&[&str]; 20] = [
            &["runtime", "dashboard"],
            &["runtime", "dashboard", "applications"],
            &["runtime", "dashboard", "application"],
            &["runtime", "dashboard", "application-violations"],
            &["runtime", "dashboard", "top-applications"],
            &["runtime", "dashboard", "violations-trend"],
            &["runtime", "dashboard", "apps-list"],
            &["runtime", "sessions"],
            &["runtime", "sessions", "chart"],
            &["runtime", "sessions", "list"],
            &["runtime", "sessions", "get"],
            &["runtime", "sessions", "transaction"],
            &["runtime", "sessions", "scan-content"],
            &["runtime", "api-keys", "list"],
            &["runtime", "customer-apps", "list"],
            &["runtime", "customer-apps", "get"],
            &["runtime", "customer-apps", "consumption"],
            &["runtime", "deployment-profiles", "list"],
            &["runtime", "api-keys"],
            &["runtime", "customer-apps"],
        ];
        for path in leaves {
            let (t, r) = (help(&ts, path), help(&rust(), path));
            let command = path.join(" ");
            let rust_options = entries(&r, "Options:");
            for (flag, description) in entries(&t, "Options:") {
                if matches!(flag.as_str(), "--help" | "--output") {
                    continue;
                }
                match rust_options.iter().find(|(f, _)| *f == flag) {
                    Some((_, actual)) if *actual == description => {}
                    other => {
                        problems.push(format!("{command} {flag}: {other:?} vs {description:?}"))
                    }
                }
            }
            for (argument, description) in entries(&t, "Arguments:") {
                let found = entries(&r, "Arguments:").into_iter().any(|(name, text)| {
                    name.trim_matches(['<', '>', '[', ']']) == argument && text == description
                });
                if !found {
                    problems.push(format!("{command} argument {argument}: {description:?}"));
                }
            }
            let ts_about = squash(t.lines().nth(2).unwrap_or_default());
            if !squash(&r).contains(&ts_about) {
                problems.push(format!("{command} description: {ts_about:?}"));
            }
        }
        assert!(
            problems.is_empty(),
            "help differs:\n{}",
            problems.join("\n")
        );
    }
}

// ---------------------------------------------------------------------------
// Divergences (DESIGN §12)
// ---------------------------------------------------------------------------

mod divergence {
    use super::*;

    /// D205: an `api_key` member of a list row: TypeScript json prints `apiKey`; airs-cli-rs
    /// omits it in every format (and the runner forbids the marker on every stream).
    #[test]
    fn d205_listed_key_values_are_omitted() {
        let seed = |data: &mut RuntimeData| {
            ts_readable(data);
            data.api_keys[0]["api_key"] = json!(fx::key_secret(TSG, "list"));
            data.customer_apps[0]["api_keys_dp_info"][0]["api_key"] =
                json!(fx::key_secret(TSG, "nested"));
        };
        let Some((r, t)) = sides_seeded(
            "d205_listed_key_values_are_omitted",
            &["api-keys", "list", "--output", "json"],
            &seed,
        ) else {
            return;
        };
        r.run.assert_success();
        assert_eq!(
            t.run.json()[0]["apiKey"],
            json!(fx::key_secret(TSG, "list"))
        );
        assert_eq!(r.run.json()[0].get("apiKey"), None);
        let Some((r, t)) = sides_seeded(
            "d205_listed_key_values_are_omitted",
            &["customer-apps", "list", "--output", "json"],
            &seed,
        ) else {
            return;
        };
        assert!(t.run.stdout.contains(&fx::key_secret(TSG, "nested")));
        r.run.assert_success();
        assert!(!r.run.stdout.contains("api_key\""));
    }

    /// D207: with the schema-valid wire records (`creation_ts`, `expiration`,
    /// `customer_appId`), TypeScript shows no dates and no app id; airs-cli-rs reads them, and
    /// a cut `Data` is marked.
    #[test]
    fn d207_projections_read_the_wire_keys() {
        let Some((r, t)) = sides_seeded(
            "d207_projections_read_the_wire_keys",
            &["api-keys", "list", "--output", "json"],
            &unchanged,
        ) else {
            return;
        };
        assert_eq!(t.run.json()[0].get("expiresAt"), None);
        assert_eq!(r.run.json()[0]["expiresAt"], "2027-01-01T00:00:00Z");
        assert_eq!(r.run.json()[0]["createdAt"], fx::AT);
        let Some((r, t)) = sides_seeded(
            "d207_projections_read_the_wire_keys",
            &["customer-apps", "list", "--output", "json"],
            &unchanged,
        ) else {
            return;
        };
        assert_eq!(t.run.json()[0].get("id"), None);
        assert_eq!(r.run.json()[0]["id"], json!(fx::uuid(101)));
        let Some((r, t)) = sides_seeded(
            "d207_projections_read_the_wire_keys",
            &["customer-apps", "get", "app-1"],
            &unchanged,
        ) else {
            return;
        };
        assert!(!t.run.stdout.contains("  ID  "));
        assert!(
            r.run
                .stdout
                .contains(&format!("  ID    {}\n", fx::uuid(101)))
        );
        let long = |data: &mut RuntimeData| {
            ts_readable(data);
            data.customer_apps[0]["notes"] = json!("n".repeat(600));
        };
        let Some((r, t)) = sides_seeded(
            "d207_projections_read_the_wire_keys",
            &["customer-apps", "get", "app-1"],
            &long,
        ) else {
            return;
        };
        assert!(!t.run.stdout.contains("cut at 500"));
        assert_eq!(
            r.run.stdout.replace(
                "\n  … (cut at 500 characters; use --output json for the whole record)",
                ""
            ),
            t.run.stdout
        );
    }

    /// D209: `null` stats and an empty breakdown: TypeScript invents zeros; airs-cli-rs prints
    /// only what the server sent.
    #[test]
    fn d209_consumption_invents_nothing() {
        let seed = |data: &mut RuntimeData| {
            data.applications.insert(
                "uuid-shared|chatbot".to_owned(),
                json!({"name": null, "token_stats": null, "session_stats": null, "profiles": null}),
            );
            data.breakdowns.insert(
                "uuid-shared|chatbot".to_owned(),
                json!({"detection_type_violation_breakdown": [{"detection_type": "pi", "violation_breakdown": {"total": 2}}]}),
            );
        };
        let Some((r, t)) = sides_seeded(
            "d209_consumption_invents_nothing",
            &["customer-apps", "consumption", "chatbot"],
            &seed,
        ) else {
            return;
        };
        assert!(t.run.stdout.contains("  Total      0\n  Violating  0\n"));
        assert!(t.run.stdout.contains("c=0 h=0 m=0 l=0"));
        assert!(t.run.stdout.contains("Detectors (0 violating"));
        assert!(r.run.stdout.contains("  Total      -\n  Violating  -\n"));
        assert!(r.run.stdout.contains("c=? h=? m=? l=?"));
        assert!(
            r.run
                .stdout
                .contains("Detectors (? violating, 1/1 firing):")
        );
        let Some((r, t)) = sides_seeded(
            "d209_consumption_invents_nothing",
            &[
                "customer-apps",
                "consumption",
                "chatbot",
                "--output",
                "json",
            ],
            &seed,
        ) else {
            return;
        };
        assert_eq!(t.run.json()[0]["sessions_total"], 0);
        assert_eq!(t.run.json()[0]["daily_avg"], "-");
        assert_eq!(t.run.json()[0]["monitoring_since"], "");
        assert_eq!(r.run.json()[0]["sessions_total"], Value::Null);
        assert_eq!(r.run.json()[0]["daily_avg"], Value::Null);
        assert_eq!(r.run.json()[0]["critical"], Value::Null);
    }

    /// D210: all apps: TypeScript prints one JSON array per app (several documents), loses an
    /// app without detectors, exits 0 after a failed app and walks the overview again per
    /// app; airs-cli-rs prints one document, keeps the app, exits 1 and walks once.
    #[test]
    fn d210_all_apps_is_one_document() {
        let seed = |data: &mut RuntimeData| {
            data.breakdowns.insert(
                "uuid-litellm|litellm".to_owned(),
                json!({"detection_type_violation_breakdown": []}),
            );
        };
        let Some((r, t)) = sides_seeded(
            "d210_all_apps_is_one_document",
            &["customer-apps", "consumption", "--output", "json"],
            &seed,
        ) else {
            return;
        };
        assert!(
            serde_json::from_str::<Value>(&t.run.stdout).is_err(),
            "several documents"
        );
        assert!(t.run.stdout.ends_with("]\n[]\n"));
        let rows = r.run.json();
        assert_eq!(rows.as_array().unwrap().len(), 7);
        assert_eq!(rows[6]["app_name"], "litellm");
        assert_eq!(rows[6]["detector"], Value::Null);
        let overview = |side: &Side| {
            api(side)
                .iter()
                .filter(|line| line.contains("applicationsoverview"))
                .count()
        };
        assert_eq!(overview(&r), 1);
        assert_eq!(overview(&t), 4);
        let Some(ts) = require_ts("d210_all_apps_is_one_document") else {
            return;
        };
        let plan = |server: &FixtureServer| {
            server.push_once(Route::Runtime(Rx::Application), Planned::builtin());
            server.push_once(
                Route::Runtime(Rx::Application),
                Planned::json(400, &json!({"message": "bad"})),
            );
            // TS walks the overview per app and replays nothing on a 400.
        };
        let r = exec_with(
            &rust(),
            &["customer-apps", "consumption", "--output", "json"],
            &unchanged,
            &plan,
            &RunOptions::new(),
        );
        let t = exec_with(
            &ts,
            &["customer-apps", "consumption", "--output", "json"],
            &unchanged,
            &plan,
            &RunOptions::new(),
        );
        r.run.assert_code(1);
        assert!(r.run.stderr.contains("  ✗ [LiteLLM] "));
        t.run.assert_code(0);
        assert!(t.run.stderr.contains("  ✗ [LiteLLM] "));
    }

    /// D211: the lookup window, a capped overview page, a name shared by two ids and `''`.
    #[test]
    fn d211_the_lookup_is_complete_and_unambiguous() {
        let Some((r, t)) = sides_seeded(
            "d211_the_lookup_is_complete_and_unambiguous",
            &[
                "customer-apps",
                "consumption",
                "litellm",
                "--time-interval",
                "60",
                "--output",
                "json",
            ],
            &unchanged,
        ) else {
            return;
        };
        assert!(
            api(&t)[0].contains("time_interval=30&"),
            "TS looks up over 30 days"
        );
        assert!(api(&r)[0].contains("time_interval=60&"));
        // TypeScript sends the two per-app reads concurrently (`Promise.all`, choice 17), so
        // their order is compared as a set.
        let sorted = |side: &Side| {
            let mut reads = api(side)[1..].to_vec();
            reads.sort();
            reads
        };
        assert_eq!(sorted(&r), sorted(&t), "the reads use the window on both");
        let capped = |data: &mut RuntimeData| data.page_cap = Some(2);
        let Some((r, t)) = sides_seeded(
            "d211_the_lookup_is_complete_and_unambiguous",
            &[
                "customer-apps",
                "consumption",
                "litellm",
                "--output",
                "json",
            ],
            &capped,
        ) else {
            return;
        };
        t.run.assert_code(1);
        assert!(
            t.run.stderr.contains("Dashboard application not found"),
            "a false not-found"
        );
        r.run.assert_success();
        let shared =
            |data: &mut RuntimeData| data.apps.push(fx::overview_item("uuid-other", "chatbot"));
        let Some((r, t)) = sides_seeded(
            "d211_the_lookup_is_complete_and_unambiguous",
            &[
                "customer-apps",
                "consumption",
                "chatbot",
                "--output",
                "json",
            ],
            &shared,
        ) else {
            return;
        };
        t.run.assert_success();
        assert!(
            api(&t)[1].contains("appid=uuid-shared"),
            "TS takes the first"
        );
        r.run.assert_code(1);
        assert!(
            r.run
                .stderr
                .contains("matches 2 applications with different ids")
        );
        assert_eq!(api(&r).len(), 1);
        let Some((r, t)) = sides_seeded(
            "d211_the_lookup_is_complete_and_unambiguous",
            &["customer-apps", "consumption", "", "--output", "json"],
            &unchanged,
        ) else {
            return;
        };
        t.run.assert_success();
        assert!(api(&t).len() > 1, "TS reads every app");
        r.run.assert_code(2);
        assert!(r.lines.is_empty());
    }

    /// D219: repeated rows in an `--all` walk: TypeScript prints them twice; airs-cli-rs fails
    /// the walk.
    #[test]
    fn d219_repeated_rows_are_an_error() {
        let seed = |data: &mut RuntimeData| {
            ts_readable(data);
            data.repeat_rows = true;
        };
        for args in [
            &[
                "api-keys", "list", "--all", "--limit", "2", "--output", "json",
            ][..],
            &[
                "customer-apps",
                "list",
                "--all",
                "--limit",
                "2",
                "--output",
                "json",
            ],
        ] {
            let Some((r, t)) = sides_seeded("d219_repeated_rows_are_an_error", args, &seed) else {
                return;
            };
            t.run.assert_success();
            assert_eq!(t.run.json().as_array().unwrap().len(), 4, "{args:?}");
            r.run.assert_code(1);
            assert_eq!(r.run.stdout, "");
            assert!(
                r.run.stderr.contains("Pagination stopped"),
                "{}",
                r.run.stderr
            );
        }
    }

    /// D12 and D14 (extended): `next_offset: 0` is not "more available", and a `--max` cut
    /// says so.
    #[test]
    fn d12_d14_status_lines_tell_the_truth() {
        let zero = |data: &mut RuntimeData| {
            ts_readable(data);
            data.zero_next_offset = true;
        };
        let Some((r, t)) = sides_seeded(
            "d12_d14_status_lines_tell_the_truth",
            &["customer-apps", "list", "--output", "json"],
            &zero,
        ) else {
            return;
        };
        assert_eq!(t.run.stderr, "  Showing 3 (more available)\n");
        assert_eq!(r.run.stderr, "  Showing 3\n");
        let Some((r, t)) = sides(
            "d12_d14_status_lines_tell_the_truth",
            &[
                "api-keys", "list", "--all", "--limit", "1", "--max", "2", "--output", "json",
            ],
        ) else {
            return;
        };
        assert_eq!(t.run.stderr, "  Showing all 2\n");
        assert_eq!(
            r.run.stderr,
            "  Showing first 2 (stopped at --max 2; more available)\n"
        );
    }

    /// D220: argument checks before the tenant: without a tenant TypeScript reports the
    /// tenant for `--limit 0`; it takes `7.5` and `60abc` as a window.
    #[test]
    fn d220_argument_checks_come_before_the_tenant() {
        let Some(ts) = require_ts("d220_argument_checks_come_before_the_tenant") else {
            return;
        };
        for args in [
            &["runtime", "api-keys", "list", "--limit", "0"][..],
            &[
                "runtime",
                "customer-apps",
                "consumption",
                "--time-interval",
                "14",
            ],
        ] {
            let sandbox = Sandbox::new();
            let r = run(&rust(), &sandbox, args, &RunOptions::new());
            let t = run(&ts, &sandbox, args, &RunOptions::new());
            r.assert_code(2);
            t.assert_code(1);
            assert!(t.stderr.contains("No tenant"), "{}", t.stderr);
        }
        let Some((r, t)) = sides(
            "d220_argument_checks_come_before_the_tenant",
            &[
                "customer-apps",
                "consumption",
                "chatbot",
                "--time-interval",
                "7.5",
                "--output",
                "json",
            ],
        ) else {
            return;
        };
        t.run.assert_success();
        assert!(api(&t)[1].contains("time_interval=7&"));
        r.run.assert_code(2);
        assert!(r.lines.is_empty());
    }

    /// D222: the banner on a failing run, `--quiet` and the empty deployment-profile json.
    #[test]
    fn d222_quiet_keeps_data_and_the_banner_needs_success() {
        let plan = |server: &FixtureServer| {
            server.set_default(Route::Runtime(Rx::ApiKeys), Planned::json(404, &json!({})));
        };
        let Some(ts) = require_ts("d222_quiet_keeps_data_and_the_banner_needs_success") else {
            return;
        };
        let r = exec_with(
            &rust(),
            &["api-keys", "list"],
            &ts_readable,
            &plan,
            &RunOptions::new(),
        );
        let t = exec_with(
            &ts,
            &["api-keys", "list"],
            &ts_readable,
            &plan,
            &RunOptions::new(),
        );
        r.run.assert_code(1);
        t.run.assert_code(1);
        assert_eq!(t.run.stdout, BANNER);
        assert_eq!(r.run.stdout, "");
        let Some((r, t)) = sides(
            "d222_quiet_keeps_data_and_the_banner_needs_success",
            &["--quiet", "api-keys", "list"],
        ) else {
            return;
        };
        assert!(!t.run.stdout.contains(&fx::uuid(1)));
        assert!(r.run.stdout.contains(&format!("  {}\n", fx::uuid(1))));
        let Some((r, t)) = sides(
            "d222_quiet_keeps_data_and_the_banner_needs_success",
            &["--quiet", "customer-apps", "consumption", "chatbot"],
        ) else {
            return;
        };
        assert!(!t.run.stdout.contains("(uuid-shared)"));
        assert!(r.run.stdout.contains("\n  chatbot\n  (uuid-shared)\n\n"));
        let empty = |data: &mut RuntimeData| data.deployment_profiles.clear();
        let Some((r, t)) = sides_seeded(
            "d222_quiet_keeps_data_and_the_banner_needs_success",
            &["deployment-profiles", "list", "--output", "json"],
            &empty,
        ) else {
            return;
        };
        assert_eq!(t.run.stdout, "  No deployment profiles found\n");
        assert_eq!(r.run.stdout, "[]\n");
    }

    /// D50 (extended): `markdown`, `csv` and `yaml` are usage errors before the tenant;
    /// TypeScript renders them.
    #[test]
    fn d50_dropped_formats_are_usage_errors_on_the_runtime_extras() {
        for format in ["markdown", "csv", "yaml"] {
            let Some((r, t)) = sides(
                "d50_dropped_formats_are_usage_errors_on_the_runtime_extras",
                &["deployment-profiles", "list", "--output", format],
            ) else {
                return;
            };
            assert_eq!(r.run.code, 2);
            assert!(r.requests.is_empty());
            t.run.assert_success();
        }
    }

    /// D11 and D52 (extended): a 409 echoing text is not echoed, and control characters in
    /// a name are replaced; TypeScript prints both raw.
    #[test]
    fn d11_d52_server_text_is_neither_echoed_nor_raw() {
        let Some(ts) = require_ts("d11_d52_server_text_is_neither_echoed_nor_raw") else {
            return;
        };
        let plan = |server: &FixtureServer| {
            server.push_once(
                Route::Runtime(Rx::CustomerApp),
                Planned::json(409, &json!({"message": "key \u{1b}[31mECHOED exists"})),
            );
        };
        let r = exec_with(
            &rust(),
            &["customer-apps", "get", "app-1"],
            &ts_readable,
            &plan,
            &RunOptions::new(),
        );
        let t = exec_with(
            &ts,
            &["customer-apps", "get", "app-1"],
            &ts_readable,
            &plan,
            &RunOptions::new(),
        );
        assert!(t.run.stderr.contains("\u{1b}[31mECHOED"));
        assert!(!r.run.stderr.contains("ECHOED"));
        let seed = |data: &mut RuntimeData| {
            ts_readable(data);
            data.api_keys[0]["api_key_name"] = json!("evil\u{1b}[31mRED");
        };
        let Some((r, t)) = sides_seeded(
            "d11_d52_server_text_is_neither_echoed_nor_raw",
            &["api-keys", "list"],
            &seed,
        ) else {
            return;
        };
        assert!(t.run.stdout.contains("evil\u{1b}[31mRED"));
        assert!(r.run.stdout.contains("evil\u{fffd}[31mRED"));
    }

    /// D224: an interrupted read prints its context; TypeScript exits by the signal with
    /// nothing printed.
    #[test]
    fn d224_an_interrupted_read_prints_its_context() {
        let Some(ts) = require_ts("d224_an_interrupted_read_prints_its_context") else {
            return;
        };
        let mut outs = Vec::new();
        for cli in [rust(), ts] {
            let server = FixtureServer::start();
            server.seed_runtime();
            server.push_once(Route::Runtime(Rx::ApiKeys), Planned::hang());
            let sandbox = Sandbox::new();
            sandbox.single_tenant(&server, "prod", TSG);
            let running = spawn(
                &cli,
                &sandbox,
                &["runtime", "api-keys", "list", "--output", "json"],
                &RunOptions::new().expect_signal(),
            );
            assert!(server.wait_for_route(Route::Runtime(Rx::ApiKeys), 1, Duration::from_secs(20)));
            running.send_signal("INT");
            outs.push(running.wait());
        }
        assert_eq!(outs[0].code, 130);
        assert_eq!(
            outs[0].stderr,
            "\n  ✗ Error: Interrupted; no data was written\n\n"
        );
        assert_eq!(outs[1].code, 130);
        assert_eq!(outs[1].stderr, "");
    }
}
