//! TS-vs-Rust parity for `runtime report` (DESIGN §18.14).
//!
//! Each scenario runs once against airs-cli-rs and once against the TypeScript reference,
//! each on a fresh seeded [`FixtureServer`] and a fresh [`Sandbox`]. The HTML is compared
//! as the intentional Signal Brief replacement (D340); the
//! requests are compared as a sorted set (TypeScript reads the seven sources concurrently,
//! airs-cli-rs one after the other, choice 17). The `divergence` tests pin both sides of
//! the §12 rows of the report (D155, D156, D214).

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use airs_test_harness::{
    Cli, FixtureServer, NormalizedRequest, Planned, Route, Run, RunOptions, Rx, Sandbox, fixtures,
    normalize_all, require_ts, run,
};
use serde_json::json;

const TSG: &str = "100";
const FAILED_LINE: &str = "  ✗ Report generation failed. Verify Management API configuration and a writable, new output path. Existing files are never overwritten.\n";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

struct Side {
    run: Run,
    requests: Vec<NormalizedRequest>,
    sandbox: Sandbox,
}

type Edit<'a> = dyn Fn(&FixtureServer, &Sandbox) + 'a;

fn exec_with(cli: &Cli, args: &[&str], edit: &Edit<'_>) -> Side {
    let server = FixtureServer::start();
    server.seed_runtime();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", TSG);
    edit(&server, &sandbox);
    let options = RunOptions::new().allow_writes_under("cwd");
    let options = if cli.is_ts() {
        options.allow_secret_output()
    } else {
        options
    };
    let out = run(cli, &sandbox, args, &options);
    let mut requests = normalize_all(&server.requests());
    requests.sort_by_key(|request| format!("{request:?}"));
    Side {
        run: out,
        requests,
        sandbox,
    }
}

fn sides_with(name: &str, args: &[&str], edit: &Edit<'_>) -> Option<(Side, Side)> {
    let ts = require_ts(name)?;
    Some((exec_with(&rust(), args, edit), exec_with(&ts, args, edit)))
}

fn context(r: &Side, t: &Side) -> String {
    format!("\n--- rust ---\n{:?}\n--- ts ---\n{:?}", r.run, t.run)
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

mod parity {
    use super::*;

    /// D340: the default report uses Signal Brief evidence and SDK discovery;
    /// the frozen reference retains its seven-source aggregate report.
    #[test]
    fn d340_signal_brief_replaces_legacy_runtime_report() {
        let scenarios: [&Edit<'_>; 5] = [
            &|_, _| {},
            &|server, _| {
                server.set_default(
                    Route::Runtime(Rx::SessionsChart),
                    Planned::json(404, &json!({"message": "PRIVATE-ERROR"})),
                );
            },
            &|server, _| {
                server.runtime(|data| {
                    data.apps[0]["name"] = json!("A & <b>\u{1b}[31m");
                    data.apps[1]["sessions_violated"] = json!(50);
                });
            },
            &|server, _| {
                for route in [
                    Route::Runtime(Rx::AppsOverview),
                    Route::Runtime(Rx::SessionsOverview),
                    Route::Runtime(Rx::SessionsChart),
                    Route::Runtime(Rx::TopApplications),
                    Route::Runtime(Rx::ViolationsTrend),
                    Route::Runtime(Rx::CustomerApps),
                    Route::Profiles,
                ] {
                    server.set_default(
                        route,
                        Planned::json(500, &json!({"message": "PRIVATE-ERROR"}))
                            .with_retry_after(0),
                    );
                }
            },
            // 8.0 (§13.17): the "Protection configurations" column of a directional
            // profile (one row per direction, the shared settings left out), a
            // multi-entry legacy profile and an entry without protection sections.
            &|server, _| {
                let mut directional = fixtures::runtime::directional_profile();
                directional["policy"]["ai-security-profiles"][0]["content-type-configurations"]["tool-response"] =
                    json!({"x-future": true});
                let mut multi = fixtures::profile(2, "multi", 4);
                multi["policy"]["ai-security-profiles"] = json!([
                    {"model-type": "default", "model-configuration": {
                        "model-protection": [{"name": "prompt-injection", "action": "block"}],
                        "latency": {"inline-timeout-action": "allow"}}},
                    {"model-configuration": {"mask-data-in-storage": true}}
                ]);
                server.set_profiles(TSG, vec![directional, multi]);
            },
        ];
        for (index, edit) in scenarios.iter().enumerate() {
            let Some((r, t)) = sides_with(
                "d340_signal_brief_replaces_legacy_runtime_report",
                &[
                    "runtime",
                    "report",
                    "--output-file",
                    "-",
                    "--title",
                    "Daily <report> & co",
                ],
                edit,
            ) else {
                return;
            };
            assert!(
                r.run.stdout.contains("AIRS Signal Brief"),
                "scenario {index}{}",
                context(&r, &t)
            );
            assert!(t.run.stdout.contains("Report schema 2"));
            assert!(!t.run.stdout.contains("AIRS Signal Brief"));
            assert_ne!(r.run.stdout, t.run.stdout);
            assert!(
                r.run
                    .stderr
                    .starts_with("  Collecting runtime Signal Brief evidence")
            );
            assert!(
                t.run
                    .stderr
                    .starts_with("  Collecting daily runtime activity")
            );
            assert_ne!(r.requests, t.requests);
            assert!(!r.run.stdout.contains("PRIVATE"));
        }
    }

    /// Both write a new 0600 file, refuse an existing path before any request with the
    /// same failure line, and validate the options alike (exit 2).
    #[test]
    fn files_and_checks_match_typescript() {
        let Some(ts) = require_ts("files_and_checks_match_typescript") else {
            return;
        };
        for cli in [rust(), ts.clone()] {
            let side = exec_with(
                &cli,
                &["runtime", "report", "--output-file", "r.html"],
                &|_, _| {},
            );
            side.run.assert_success();
            assert_eq!(
                mode(&side.sandbox.cwd().join("r.html")),
                0o600,
                "{}",
                cli.label()
            );
            let existing = exec_with(
                &cli,
                &["runtime", "report", "--output-file", "e.html"],
                &|_, sandbox| {
                    std::fs::write(sandbox.cwd().join("e.html"), "KEEP").unwrap();
                },
            );
            assert_eq!(existing.run.code, 1, "{}", cli.label());
            assert_eq!(existing.run.stderr, FAILED_LINE, "{}", cli.label());
            assert!(existing.requests.is_empty(), "{}", cli.label());
            assert_eq!(
                std::fs::read_to_string(existing.sandbox.cwd().join("e.html")).unwrap(),
                "KEEP"
            );
        }
        for (args, message) in [
            (
                vec!["runtime", "report", "--max-pages", "0"],
                "--max-pages must be an integer from 1 to 100",
            ),
            (
                vec!["runtime", "report", "--max-pages", "101"],
                "--max-pages must be an integer from 1 to 100",
            ),
            (
                vec!["runtime", "report", "--title", ""],
                "--title must contain 1–240 characters",
            ),
            (
                vec!["runtime", "report", "--output-file", " "],
                "--output-file cannot be empty",
            ),
        ] {
            for cli in [rust(), ts.clone()] {
                let side = exec_with(&cli, &args, &|_, _| {});
                assert_eq!(side.run.code, 2, "{} {args:?}", cli.label());
                assert!(
                    side.run.stderr.contains(&format!("✗ {message}")),
                    "{} {args:?}: {}",
                    cli.label(),
                    side.run.stderr
                );
                assert!(side.requests.is_empty());
            }
        }
    }
}

mod divergence {
    use super::*;

    /// D214: an absent `ai_profiles`: TypeScript's SDK default makes it a complete empty
    /// inventory with the `No security profiles returned` finding; airs-cli-rs reports the
    /// source unavailable. A missing parent directory: TypeScript sends every read first;
    /// airs-cli-rs sends none.
    #[test]
    fn d214_absent_profiles_and_the_destination_parent() {
        let edit: &Edit<'_> = &|server, _| {
            server.set_default(Route::Profiles, Planned::json(200, &json!({})));
        };
        let Some((r, t)) = sides_with(
            "d214_absent_profiles_and_the_destination_parent",
            &["runtime", "report", "--output-file", "-"],
            edit,
        ) else {
            return;
        };
        assert!(t.run.stdout.contains("No security profiles returned"));
        assert!(!r.run.stdout.contains("No security profiles returned"));
        assert!(
            r.run
                .stdout
                .contains("profiles (required)</td><td>unavailable")
        );
        let missing = "nodir/r.html";
        let Some((r, t)) = sides_with(
            "d214_absent_profiles_and_the_destination_parent",
            &["runtime", "report", "--output-file", missing],
            &|_, _| {},
        ) else {
            return;
        };
        assert_eq!(r.run.code, 1);
        assert_eq!(t.run.code, 1);
        assert_eq!(r.run.stderr, FAILED_LINE);
        assert!(t.run.stderr.ends_with(FAILED_LINE));
        assert!(r.requests.is_empty());
        assert!(t.requests.len() >= 7, "TS reads every source first");
    }

    /// D155: TypeScript writes Markdown for `--output markdown` and refuses `json` with
    /// its own text; airs-cli-rs writes HTML only (markdown is an invalid format, any
    /// valid `--output` is refused).
    #[test]
    fn d155_the_runtime_report_is_html_only() {
        let Some((r, t)) = sides_with(
            "d155_the_runtime_report_is_html_only",
            &[
                "--output",
                "markdown",
                "runtime",
                "report",
                "--output-file",
                "-",
            ],
            &|_, _| {},
        ) else {
            return;
        };
        t.run.assert_success();
        assert!(t.run.stdout.starts_with("# Daily environment report"));
        assert_eq!(r.run.code, 2);
        assert_eq!(
            r.run.stderr,
            "\n  ✗ Invalid output format 'markdown'. Expected: pretty, table, json\n\n"
        );
        let Some((r, t)) = sides_with(
            "d155_the_runtime_report_is_html_only",
            &["--output", "json", "runtime", "report"],
            &|_, _| {},
        ) else {
            return;
        };
        assert_eq!(t.run.code, 2);
        assert!(
            t.run
                .stderr
                .contains("Report output must be html or markdown")
        );
        assert_eq!(r.run.code, 2);
        assert_eq!(
            r.run.stderr,
            "\n  ✗ The environment report is written as HTML; omit --output\n\n"
        );
    }

    /// D156: both refuse `--debug`; the Rust text names no environment variable.
    #[test]
    fn d156_the_report_refuses_debug() {
        let Some((r, t)) = sides_with(
            "d156_the_report_refuses_debug",
            &["--debug", "runtime", "report"],
            &|_, _| {},
        ) else {
            return;
        };
        assert_eq!(t.run.code, 2);
        assert!(
            t.run.stderr.contains("PANW_AI_SEC_DEBUG"),
            "{}",
            t.run.stderr
        );
        assert_eq!(r.run.code, 2);
        assert_eq!(
            r.run.stderr,
            "\n  ✗ Disable --debug for environment reports to avoid persisting sensitive traffic content\n\n"
        );
        assert!(r.requests.is_empty() && t.requests.is_empty());
    }
}
