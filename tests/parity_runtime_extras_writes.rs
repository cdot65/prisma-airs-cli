//! TS-vs-Rust parity for the M6d step-2 writes (DESIGN §18.5, §18.6, §18.8): `runtime
//! api-keys create|regenerate|delete` and `runtime customer-apps update|delete`.
//!
//! Each scenario runs once against airs-cli-rs and once against the TypeScript reference,
//! each on a fresh seeded [`FixtureServer`] and a fresh [`Sandbox`]. The Rust side adds its
//! own required options (`--key-file`, `--force`); TypeScript has neither. `parity` tests
//! compare the requests (bodies as JSON) and the outcome lines after
//! `split_outcome_lines` (TypeScript stdout ↔ Rust stderr, D222); `divergence` tests pin
//! both sides of each §12 row this step implements. The TypeScript side prints the key
//! value (D204), so its runs disable the marker check and its key is never compared with a
//! Rust output; the airs-cli-rs runs keep the check.

use std::time::Duration;

use airs_test_harness::fixtures::runtime as fx;
use airs_test_harness::{
    Cli, FixtureServer, NormalizedRequest, Planned, Route, Run, RunOptions, RuntimeData, Rx,
    Sandbox, normalize_all, require_ts, run, spawn, split_outcome_lines,
};
use serde_json::{Value, json};

const TSG: &str = "100";
const BANNER: &str =
    "\n  Prisma AIRS — Runtime Configuration\n  Security profile and topic management\n\n";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

struct Side {
    run: Run,
    requests: Vec<NormalizedRequest>,
    lines: Vec<String>,
    data: RuntimeData,
}

/// What a scenario needs: the files it writes into the sandbox's `tmp/` and the server
/// changes.
type Setup<'a> = dyn Fn(&FixtureServer) + 'a;

fn key_config() -> Value {
    json!({
        "auth_code": "FAKE-AUTHCODE-1",
        "cust_app": "app-1",
        "created_by": "admin@example.com",
        "api_key_name": "new-key",
        "revoked": false,
        "rotation_time_interval": 30,
        "rotation_time_unit": "days",
        "extra": "kept"
    })
}

fn app_config() -> Value {
    json!({"app_name": "app-1", "cloud_provider": "azure", "environment": "dev"})
}

/// Runs `runtime <args>` with `{config}` replaced by a file holding `config` and
/// `{key}` by a key-file path (both under `tmp/`); `rust_extra` is appended on the Rust
/// side only.
fn exec(
    cli: &Cli,
    args: &[&str],
    rust_extra: &[&str],
    config: Option<&str>,
    setup: &Setup<'_>,
) -> Side {
    let server = FixtureServer::start();
    server.seed_runtime();
    setup(&server);
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", TSG);
    let config_path = sandbox.tmp().join("config.json").display().to_string();
    if let Some(config) = config {
        std::fs::write(&config_path, config).unwrap();
    }
    let key_path = sandbox.tmp().join("new.key").display().to_string();
    let mut full: Vec<String> = vec!["runtime".to_owned()];
    let extra: &[&str] = if cli.is_ts() { &[] } else { rust_extra };
    for arg in args.iter().chain(extra) {
        full.push(
            arg.replace("{config}", &config_path)
                .replace("{key}", &key_path),
        );
    }
    let full: Vec<&str> = full.iter().map(String::as_str).collect();
    let options = if cli.is_ts() {
        RunOptions::new().allow_secret_output()
    } else {
        RunOptions::new()
    };
    let out = run(cli, &sandbox, &full, &options);
    let raw = server.requests();
    Side {
        run: out,
        requests: normalize_all(&raw),
        lines: raw.iter().map(|request| request.line()).collect(),
        data: server.runtime_data(),
    }
}

fn sides(
    name: &str,
    args: &[&str],
    rust_extra: &[&str],
    config: Option<&str>,
    setup: &Setup<'_>,
) -> Option<(Side, Side)> {
    let ts = require_ts(name)?;
    Some((
        exec(&rust(), args, rust_extra, config, setup),
        exec(&ts, args, rust_extra, config, setup),
    ))
}

fn context(r: &Side, t: &Side) -> String {
    format!("\n--- rust ---\n{:?}\n--- ts ---\n{:?}", r.run, t.run)
}

fn api(side: &Side) -> Vec<String> {
    side.lines
        .iter()
        .filter(|line| !line.starts_with("POST /oauth/token"))
        .cloned()
        .collect()
}

fn writes(side: &Side, route: Rx) -> usize {
    side.lines
        .iter()
        .zip(&side.requests)
        .filter(|(_, request)| request.route == Route::Runtime(route))
        .count()
}

fn unchanged(_: &FixtureServer) {}

// ---------------------------------------------------------------------------
// Parity
// ---------------------------------------------------------------------------

mod parity {
    use super::*;

    /// The same requests, bodies and service state for every write; the acknowledgements
    /// TypeScript prints on stdout are the Rust stderr lines where D208 does not apply.
    #[test]
    fn writes_send_the_typescript_requests() {
        let key = key_config().to_string();
        let app = app_config().to_string();
        let id = fx::uuid(1);
        let app_id = fx::uuid(101);
        type Case<'a> = (Vec<&'a str>, Vec<&'a str>, Option<&'a str>, Option<&'a str>);
        let cases: Vec<Case<'_>> = vec![
            (
                vec!["api-keys", "create", "--config", "{config}"],
                vec!["--key-file", "{key}"],
                Some(&key),
                Some("  ✓ API key created: 22222222-2222-4222-8222-000000000901\n"),
            ),
            (
                vec![
                    "api-keys",
                    "regenerate",
                    &id,
                    "--interval",
                    "30",
                    "--unit",
                    "days",
                    "--updated-by",
                    "u@example.com",
                ],
                vec!["--key-file", "{key}", "--force"],
                None,
                Some("  ✓ API key regenerated: 22222222-2222-4222-8222-000000000901\n"),
            ),
            (
                vec!["api-keys", "rm", "key-1", "--updated-by", "u@example.com"],
                vec!["--force"],
                None,
                None,
            ),
            (
                vec!["customer-apps", "update", &app_id, "--config", "{config}"],
                vec!["--force"],
                Some(&app),
                Some("  ✓ Customer app updated: app-1\n"),
            ),
            (
                vec![
                    "customer-apps",
                    "rm",
                    "app-1",
                    "--updated-by",
                    "u@example.com",
                ],
                vec!["--force"],
                None,
                None,
            ),
        ];
        for (args, extra, config, ack) in cases {
            let Some((r, t)) = sides(
                "writes_send_the_typescript_requests",
                &args,
                &extra,
                config,
                &unchanged,
            ) else {
                return;
            };
            assert_eq!(r.run.code, 0, "{args:?}{}", context(&r, &t));
            assert_eq!(t.run.code, 0, "{args:?}{}", context(&r, &t));
            assert_eq!(r.requests, t.requests, "{args:?}{}", context(&r, &t));
            assert_eq!(r.data.api_keys, t.data.api_keys, "{args:?}");
            assert_eq!(r.data.customer_apps, t.data.customer_apps, "{args:?}");
            let (ts_outcome, _) = split_outcome_lines(&t.run.stdout);
            if let Some(ack) = ack {
                assert_eq!(ts_outcome, ack, "{args:?}");
                assert!(r.run.stderr.starts_with(ack), "{args:?}{}", context(&r, &t));
            }
        }
    }

    /// The help of every step-2 leaf carries TypeScript's description and option texts,
    /// plus the Rust-only `--key-file` and `--force`.
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
        let squash = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        for (path, ts_options, rust_only) in [
            (
                &["runtime", "api-keys", "create"][..],
                &["--config <path>  JSON file with API key configuration"][..],
                &["--key-file <path>"][..],
            ),
            (
                &["runtime", "api-keys", "regenerate"],
                &[
                    "--interval <n>  Rotation time interval",
                    "--unit <unit>  Rotation time unit (hours, days, months)",
                    "--updated-by <email>  Email of user performing regeneration",
                ],
                &["--key-file <path>", "--force"],
            ),
            (
                &["runtime", "api-keys", "delete"],
                &["--updated-by <email>  Email of user performing deletion"],
                &["--force"],
            ),
            (
                &["runtime", "customer-apps", "update"],
                &["--config <path>  JSON file with app updates"],
                &["--force"],
            ),
            (
                &["runtime", "customer-apps", "delete"],
                &["--updated-by <email>  Email of user performing deletion"],
                &["--force"],
            ),
        ] {
            let (t, r) = (squash(&help(&ts, path)), squash(&help(&rust(), path)));
            let about = squash(help(&ts, path).lines().nth(2).unwrap_or_default());
            assert!(r.contains(&about), "{path:?}: {about:?} in {r}");
            for option in ts_options {
                let option = squash(option);
                assert!(t.contains(&option), "{path:?} TS: {option}");
                assert!(r.contains(&option), "{path:?} Rust: {option} in {r}");
            }
            for option in rust_only {
                assert!(r.contains(option), "{path:?}: {option}");
                assert!(!t.contains(option), "{path:?}: TS has {option}");
            }
        }
        // The groups list the step-2 commands with TS's descriptions.
        for (group, commands) in [
            (
                "api-keys",
                &[
                    "Create a new API key",
                    "Delete an API key",
                    "Regenerate an API key",
                ][..],
            ),
            (
                "customer-apps",
                &["Delete a customer app", "Update a customer app"],
            ),
        ] {
            let r = help(&rust(), &["runtime", group]);
            let t = help(&ts, &["runtime", group]);
            for command in commands {
                assert!(
                    r.contains(command) && t.contains(command),
                    "{group}: {command}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Divergences (DESIGN §12)
// ---------------------------------------------------------------------------

mod divergence {
    use super::*;

    /// D204, D222: TypeScript prints the full key in the pretty `Key` row on stdout (also
    /// under `--quiet`) with its acknowledgement; airs-cli-rs writes the key only into the
    /// new 0600 key file and prints `…<last8>` and the path. A 2xx that fails the
    /// response schema loses the key in TypeScript and keeps it in airs-cli-rs (G26).
    #[test]
    fn d204_the_key_reaches_only_the_key_file() {
        let key = key_config().to_string();
        let Some((r, t)) = sides(
            "d204_the_key_reaches_only_the_key_file",
            &["--quiet", "api-keys", "create", "--config", "{config}"],
            &["--key-file", "{key}"],
            Some(&key),
            &unchanged,
        ) else {
            return;
        };
        let secret = fx::key_secret(TSG, "1");
        assert!(
            t.run.stdout.contains(&format!("  Key   {secret}\n")),
            "{}",
            t.run.stdout
        );
        assert!(t.run.stdout.contains("✓ API key created"));
        r.run.assert_success();
        assert!(!r.run.stdout.contains(fx::KEY_MARKER));
        assert!(
            r.run.stdout.contains("  Key       …NEW8-001\n"),
            "{}",
            r.run.stdout
        );
        let omit: &Setup<'_> = &|server| server.runtime(|data| data.omit_auth_code = true);
        let Some((r, t)) = sides(
            "d204_the_key_reaches_only_the_key_file",
            &["api-keys", "create", "--config", "{config}"],
            &["--key-file", "{key}"],
            Some(&key),
            omit,
        ) else {
            return;
        };
        assert_eq!(t.run.code, 1);
        assert!(!t.run.stdout.contains(fx::KEY_MARKER), "TS lost the key");
        assert!(t.run.stderr.contains("AISEC_RESPONSE_VALIDATION"));
        assert_eq!(r.run.code, 1);
        assert!(
            r.run.stderr.contains("its value was saved in"),
            "{}",
            r.run.stderr
        );
    }

    /// D206: TypeScript never asks and sends the delete; airs-cli-rs refuses a destructive
    /// write without `--force` when it cannot ask (exit 2, no request).
    #[test]
    fn d206_destructive_writes_confirm() {
        for args in [
            vec!["api-keys", "delete", "key-1", "--updated-by", "u"],
            vec!["customer-apps", "delete", "app-1", "--updated-by", "u"],
        ] {
            let Some((r, t)) = sides(
                "d206_destructive_writes_confirm",
                &args,
                &[],
                None,
                &unchanged,
            ) else {
                return;
            };
            t.run.assert_success();
            assert_eq!(api(&t).len(), 1, "TS deletes without asking");
            assert_eq!(r.run.code, 2);
            assert!(
                r.run
                    .stderr
                    .contains("without --force in non-interactive mode"),
                "{}",
                r.run.stderr
            );
            assert!(r.requests.is_empty());
        }
    }

    /// D208: TypeScript prints the server's text (`✓ undefined` for `{}`, `Customer app
    /// "" deleted.` for a message answer); airs-cli-rs prints fixed texts from the
    /// argument.
    #[test]
    fn d208_acknowledgements_are_fixed() {
        let empty: &Setup<'_> =
            &|server| server.runtime(|data| data.key_delete_answer = Some(json!({})));
        let Some((r, t)) = sides(
            "d208_acknowledgements_are_fixed",
            &["api-keys", "delete", "key-1", "--updated-by", "u"],
            &["--force"],
            None,
            empty,
        ) else {
            return;
        };
        assert!(t.run.stdout.contains("  ✓ undefined\n"), "{}", t.run.stdout);
        assert_eq!(r.run.stderr, "  ✓ API key key-1 deleted.\n");
        let Some((r, t)) = sides(
            "d208_acknowledgements_are_fixed",
            &["customer-apps", "delete", "app-1", "--updated-by", "u"],
            &["--force"],
            None,
            &unchanged,
        ) else {
            return;
        };
        assert!(
            t.run.stdout.contains("  ✓ Customer app \"\" deleted.\n"),
            "{}",
            t.run.stdout
        );
        assert_eq!(r.run.stderr, "  ✓ Customer app \"app-1\" deleted.\n");
    }

    /// D213: `regenerate ..` and `delete ..`: TypeScript's URL parser resolves the segment
    /// to another route; airs-cli-rs refuses it before the tenant.
    #[test]
    fn d213_path_segments_cannot_reach_another_route() {
        for (args, extra, line) in [
            (
                vec![
                    "api-keys",
                    "regenerate",
                    "..",
                    "--interval",
                    "1",
                    "--unit",
                    "days",
                ],
                &["--key-file", "{key}", "--force"][..],
                "POST /aisec/v1/mgmt/apikey/",
            ),
            (
                vec!["api-keys", "delete", "..", "--updated-by", "u"],
                &["--force"],
                "DELETE /aisec/v1/mgmt/apikey/",
            ),
        ] {
            let Some((r, t)) = sides(
                "d213_path_segments_cannot_reach_another_route",
                &args,
                extra,
                None,
                &unchanged,
            ) else {
                return;
            };
            assert!(
                api(&t).iter().any(|sent| sent.starts_with(line)),
                "{args:?}: {:?}",
                api(&t)
            );
            assert_eq!(r.run.code, 2);
            assert!(r.run.stderr.contains("Invalid API key"), "{}", r.run.stderr);
            assert!(r.requests.is_empty());
        }
    }

    /// D95: a write answered 502 (`Retry-After: 0`): TypeScript re-sends it; airs-cli-rs
    /// sends it once and says the outcome is unknown.
    #[test]
    fn d95_writes_are_sent_once() {
        let key = key_config().to_string();
        let busy: &Setup<'_> = &|server| {
            server.set_default(
                Route::Runtime(Rx::ApiKeyCreate),
                Planned::json(502, &json!({"message": "busy"})).with_retry_after(0),
            );
        };
        let Some((r, t)) = sides(
            "d95_writes_are_sent_once",
            &["api-keys", "create", "--config", "{config}"],
            &["--key-file", "{key}"],
            Some(&key),
            busy,
        ) else {
            return;
        };
        assert!(writes(&t, Rx::ApiKeyCreate) > 1, "TS re-sends");
        assert_eq!(writes(&r, Rx::ApiKeyCreate), 1);
        assert!(
            r.run
                .stderr
                .contains("Outcome unknown: the API key may have been created.")
        );
    }

    /// D220 (writes): `--interval 1.9` and an empty `--updated-by` are sent by TypeScript;
    /// airs-cli-rs refuses them before the tenant.
    #[test]
    fn d220_write_arguments_are_strict() {
        let id = fx::uuid(1);
        let Some((r, t)) = sides(
            "d220_write_arguments_are_strict",
            &[
                "api-keys",
                "regenerate",
                &id,
                "--interval",
                "1.9",
                "--unit",
                "days",
            ],
            &["--key-file", "{key}", "--force"],
            None,
            &unchanged,
        ) else {
            return;
        };
        assert_eq!(
            t.requests
                .iter()
                .find(|request| request.route == Route::Runtime(Rx::ApiKeyRegenerate))
                .map(|request| format!("{:?}", request.body)),
            Some(format!(
                "{:?}",
                airs_test_harness::NormalizedBody::Json(
                    json!({"rotation_time_interval": 1, "rotation_time_unit": "days"})
                )
            ))
        );
        assert_eq!(r.run.code, 2);
        assert_eq!(
            r.run.stderr,
            "\n  ✗ --interval must be a positive integer\n\n"
        );
        let Some((r, t)) = sides(
            "d220_write_arguments_are_strict",
            &["customer-apps", "delete", "app-1", "--updated-by", ""],
            &["--force"],
            None,
            &unchanged,
        ) else {
            return;
        };
        assert!(
            api(&t).iter().any(|sent| sent.contains("updated_by=")),
            "{:?}",
            api(&t)
        );
        assert_eq!(r.run.code, 2);
        assert!(r.requests.is_empty());
    }

    /// D221: TypeScript reads `--config` after the tenant and echoes its content in a JSON
    /// error; airs-cli-rs reads it first and never echoes it. A file naming another app
    /// is sent by TypeScript and refused by airs-cli-rs.
    #[test]
    fn d221_config_files_are_checked_and_never_echoed() {
        let bad = "{\"api_key_name\": FAKE-AUTHCODE-9}";
        let Some((r, t)) = sides(
            "d221_config_files_are_checked_and_never_echoed",
            &["api-keys", "create", "--config", "{config}"],
            &["--key-file", "{key}"],
            Some(bad),
            &unchanged,
        ) else {
            return;
        };
        assert_eq!(t.run.code, 1);
        assert!(
            t.run.stderr.contains("FAKE-AUTHC"),
            "TS quotes the file: {}",
            t.run.stderr
        );
        assert_eq!(r.run.code, 1);
        assert!(!r.run.stderr.contains("FAKE-AUTHC"));
        assert!(r.run.stderr.contains("is not valid JSON"));
        assert!(r.requests.is_empty());
        let mut other = app_config();
        other["customer_appId"] = json!(fx::uuid(102));
        let other = other.to_string();
        let Some((r, t)) = sides(
            "d221_config_files_are_checked_and_never_echoed",
            &[
                "customer-apps",
                "update",
                &fx::uuid(101),
                "--config",
                "{config}",
            ],
            &["--force"],
            Some(&other),
            &unchanged,
        ) else {
            return;
        };
        assert_eq!(api(&t).len(), 1, "TS sends it");
        assert_eq!(r.run.code, 1);
        assert!(r.run.stderr.contains("names another customer app than"));
        assert!(r.requests.is_empty());
    }

    /// D222: a failing TypeScript write leaves the banner on stdout; airs-cli-rs leaves
    /// stdout empty.
    #[test]
    fn d222_failing_writes_print_no_banner() {
        let Some((r, t)) = sides(
            "d222_failing_writes_print_no_banner",
            &["api-keys", "delete", "missing", "--updated-by", "u"],
            &["--force"],
            None,
            &unchanged,
        ) else {
            return;
        };
        assert_eq!(t.run.code, 1);
        assert!(t.run.stdout.starts_with(BANNER));
        assert_eq!(r.run.code, 1);
        assert_eq!(r.run.stdout, "");
    }

    /// D223: TypeScript ignores `--output` on the writes; airs-cli-rs prints the record
    /// in the chosen format.
    #[test]
    fn d223_writes_honour_output() {
        let app = app_config().to_string();
        let Some((r, t)) = sides(
            "d223_writes_honour_output",
            &[
                "--output",
                "json",
                "customer-apps",
                "update",
                &fx::uuid(101),
                "--config",
                "{config}",
            ],
            &["--force"],
            Some(&app),
            &unchanged,
        ) else {
            return;
        };
        assert!(t.run.stdout.starts_with(BANNER), "TS prints pretty");
        r.run.assert_success();
        assert_eq!(r.run.json()["name"], "app-1");
    }

    /// D224: interrupting a held write: TypeScript ends with nothing printed; airs-cli-rs
    /// says the change may have been applied.
    #[test]
    fn d224_an_interrupted_write_prints_its_context() {
        let Some(ts) = require_ts("d224_an_interrupted_write_prints_its_context") else {
            return;
        };
        for cli in [rust(), ts] {
            let server = FixtureServer::start();
            server.seed_runtime();
            server.push_once(Route::Runtime(Rx::ApiKeyDelete), Planned::held());
            let sandbox = Sandbox::new();
            sandbox.single_tenant(&server, "prod", TSG);
            let mut args = vec![
                "runtime",
                "api-keys",
                "delete",
                "key-1",
                "--updated-by",
                "u",
            ];
            if !cli.is_ts() {
                args.push("--force");
            }
            let options = if cli.is_ts() {
                RunOptions::new().expect_signal().allow_secret_output()
            } else {
                RunOptions::new().expect_signal()
            };
            let running = spawn(&cli, &sandbox, &args, &options);
            assert!(server.wait_for_route(
                Route::Runtime(Rx::ApiKeyDelete),
                1,
                Duration::from_secs(30)
            ));
            running.send_signal("INT");
            let out = running.wait();
            server.release_held();
            if cli.is_ts() {
                assert!(!out.stderr.contains("Interrupted"), "{}", out.stderr);
            } else {
                assert_eq!(out.code, 130);
                assert_eq!(
                    out.stderr,
                    "\n  ✗ Error: Interrupted; the change may already have been applied (outcome unknown)\n\n"
                );
            }
        }
    }
}
