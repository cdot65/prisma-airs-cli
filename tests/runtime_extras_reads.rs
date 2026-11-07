//! Executable acceptance tests for the M6d step-1 runtime extras reads (DESIGN §18.1,
//! §18.3–§18.11): `runtime api-keys list|ls`, `runtime customer-apps list|ls|get|consumption`,
//! `runtime deployment-profiles list|ls` and the `runtime scan-logs query` stub.
//!
//! Every test spawns the real `airs-cli-rs` against the loopback fixture server with the
//! seeded runtime extras dataset (`FixtureServer::seed_runtime`). The runner's leak check
//! forbids every `FAKE-APIKEY-SECRET-*` marker on every stream and in the debug log, and
//! the auth codes on stderr and in the debug log, of each run (DESIGN §10 M6d). The read
//! cases of the TypeScript `tests/unit/airs/management.spec.ts:1071-1440`,
//! `tests/unit/cli/aliases.spec.ts:55-79`, `flag-standardization.spec.ts:66,253-280` and
//! `tests/unit/config/client-options.spec.ts:113-133` are ported here (§18.19).

use std::time::Duration;

use airs_test_harness::fixtures::runtime as fx;
use airs_test_harness::{
    Cli, FixtureConfig, FixtureServer, Planned, RecordedRequest, Route, Run, RunOptions, Rx,
    Sandbox, debug_log, run, spawn,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

const TSG: &str = "100";
const BANNER: &str =
    "\n  Prisma AIRS — Runtime Configuration\n  Security profile and topic management\n\n";
const KEYS: &str = "/aisec/v1/mgmt/apikeys/tsg/100";
const APPS: &str = "/aisec/v1/mgmt/customerapp/tsg/100";
const APP: &str = "/aisec/v1/mgmt/customerapp";
const OVERVIEW: &str = "/dashboard/v1/mgmt/dashboard/v2/apps/applicationsoverview";
const APPLICATION: &str = "/dashboard/v1/mgmt/dashboard/v2/apps/application";
const BREAKDOWN: &str = "/dashboard/v1/mgmt/dashboard/v2/apps/applicationviolationbreakdown";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

struct Fx {
    server: FixtureServer,
    sandbox: Sandbox,
}

/// The seeded runtime dataset and one standard tenant `prod` (TSG 100).
fn fx() -> Fx {
    fx_with(|_| {})
}

fn fx_with(edit: impl FnOnce(&mut FixtureConfig)) -> Fx {
    let server = FixtureServer::start();
    server.seed_runtime();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, edit);
    sandbox.register(&[&tenant], Some("prod"));
    Fx { server, sandbox }
}

impl Fx {
    fn run_with(&self, args: &[&str], options: &RunOptions) -> Run {
        let before = self.sandbox.snapshot();
        let out = run(&rust(), &self.sandbox, args, options);
        self.sandbox.assert_unchanged(&before);
        self.server.assert_no_unknown_routes();
        assert!(self.sandbox.cwd_listing().is_empty(), "wrote into cwd");
        assert!(self.sandbox.home_listing().is_empty(), "wrote into HOME");
        assert!(
            self.server.rx_requests(Rx::ScanLogs).is_empty(),
            "the scan-logs trap was reached"
        );
        out
    }

    /// `runtime <args>`, with the read-only and loopback checks.
    fn rt(&self, args: &[&str]) -> Run {
        self.rt_with(args, &RunOptions::new())
    }

    fn rt_with(&self, args: &[&str], options: &RunOptions) -> Run {
        let mut full = vec!["runtime"];
        full.extend_from_slice(args);
        self.run_with(&full, options)
    }

    /// The `METHOD /path?query` of every API request (the token request left out).
    fn api(&self) -> Vec<String> {
        self.server
            .requests()
            .iter()
            .filter(|request| request.route != Route::Token)
            .map(RecordedRequest::line)
            .collect()
    }

    fn clear(&self) {
        self.server.clear_requests();
    }
}

fn ids(out: &Run, key: &str) -> Vec<String> {
    out.json()
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row[key].as_str().unwrap_or_default().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// api-keys list (§18.4)
// ---------------------------------------------------------------------------

mod api_keys {
    use super::*;

    /// TS `management.spec.ts:1071-1087`: the keys are normalised (D207: the schema-valid
    /// wire keys `creation_ts` and `expiration` are read) and `nextOffset` drives the
    /// status line (D12).
    #[test]
    fn lists_one_page_with_the_typescript_projection() {
        let f = fx();
        let out = f.rt(&["api-keys", "list", "--output", "json"]);
        out.assert_success();
        assert_eq!(f.api(), [format!("GET {KEYS}?offset=0&limit=100")]);
        assert_eq!(
            out.json()[0],
            json!({
                "id": fx::uuid(1),
                "name": "key-1",
                "last8": "LAST8-01",
                "createdAt": fx::AT,
                "expiresAt": "2027-01-01T00:00:00Z"
            })
        );
        assert_eq!(out.stderr, "  Showing 3\n");
        f.clear();
        let out = f.rt(&[
            "api-keys", "ls", "--limit", "2", "--offset", "1", "--output", "json",
        ]);
        out.assert_success();
        assert_eq!(f.api(), [format!("GET {KEYS}?offset=1&limit=2")]);
        assert_eq!(ids(&out, "id"), [fx::uuid(2), fx::uuid(3)]);
        assert_eq!(out.stderr, "  Showing 2\n");
        f.clear();
        let out = f.rt(&["api-keys", "list", "--limit", "2", "--output", "json"]);
        assert_eq!(out.stderr, "  Showing 2 (more available)\n");
    }

    #[test]
    fn layouts() {
        let f = fx();
        let out = f.rt(&["api-keys", "list"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            format!(
                "{BANNER}\n  API Keys:\n\n  {}\n    key-1 key: …LAST8-01 expires: 2027-01-01T00:00:00Z\n  {}\n    key-2 key: …LAST8-02 expires: 2027-02-01T00:00:00Z\n  {}\n    key-3 key: …LAST8-03 expires: 2027-03-01T00:00:00Z\n\n",
                fx::uuid(1),
                fx::uuid(2),
                fx::uuid(3)
            )
        );
        let out = f.rt(&["api-keys", "list", "--output", "table"]);
        assert_eq!(
            out.stdout.lines().next().unwrap(),
            " ID                                   │ Name  │ Last 8   │ Expires              "
        );
        f.server.runtime(|data| data.api_keys.clear());
        let out = f.rt(&["api-keys", "list"]);
        assert_eq!(out.stdout, format!("{BANNER}  No API keys found\n"));
        assert_eq!(out.stderr, "  Showing 0\n");
        let out = f.rt(&["api-keys", "list", "--output", "table"]);
        assert_eq!(out.stdout, "");
        let out = f.rt(&["api-keys", "list", "--output", "json"]);
        assert_eq!(out.stdout, "[]\n");
    }

    /// `--all` follows `next_offset` from 0 with `--limit` (the SDK walk), and `--max`
    /// stops it with the D14 line.
    #[test]
    fn all_walks_every_page_and_stops_at_max() {
        let f = fx();
        let out = f.rt(&[
            "api-keys", "list", "--all", "--limit", "1", "--output", "json",
        ]);
        out.assert_success();
        assert_eq!(
            f.api(),
            [
                format!("GET {KEYS}?offset=0&limit=1"),
                format!("GET {KEYS}?offset=1&limit=1"),
                format!("GET {KEYS}?offset=2&limit=1"),
            ]
        );
        assert_eq!(ids(&out, "id").len(), 3);
        assert_eq!(out.stderr, "  Showing all 3\n");
        let out = f.rt(&[
            "api-keys", "list", "--all", "--limit", "1", "--max", "2", "--output", "json",
        ]);
        out.assert_success();
        assert_eq!(ids(&out, "id").len(), 2);
        assert_eq!(
            out.stderr,
            "  Showing first 2 (stopped at --max 2; more available)\n"
        );
    }

    /// D205: an `api_key` member of a list row is omitted in every format, and nothing
    /// else that echoes it is printed (the runner forbids the marker on every stream).
    #[test]
    fn a_listed_key_value_is_never_printed() {
        let f = fx();
        f.server.runtime(|data| {
            data.api_keys[0]["api_key"] = json!(fx::key_secret(TSG, "list"));
            data.api_keys[1]["api_key_name"] = json!(fx::key_secret(TSG, "list"));
        });
        for format in ["pretty", "table", "json"] {
            let out = f.rt(&["api-keys", "list", "--output", format]);
            out.assert_success();
            assert!(!out.stdout.contains("apiKey"), "{format}");
        }
        let out = f.rt(&["api-keys", "list", "--output", "json"]);
        assert_eq!(out.json()[1]["name"], "[withheld]");
    }

    /// D219 (G28): a service that repeats rows is the D37 error, no partial list; D46: a
    /// repeated cursor and an empty page with a fresh cursor fail too.
    #[test]
    fn broken_walks_fail_without_a_partial_list() {
        for knob in ["repeat_rows", "repeat_cursor", "empty_first_page"] {
            let f = fx();
            f.server.runtime(|data| match knob {
                "repeat_rows" => data.repeat_rows = true,
                "repeat_cursor" => data.repeat_cursor = true,
                _ => data.empty_first_page = true,
            });
            let out = f.rt(&["api-keys", "list", "--all", "--limit", "1"]);
            out.assert_code(1);
            assert_eq!(out.stdout, "", "{knob}");
            assert!(out.stderr.contains("✗ Error: "), "{knob}: {}", out.stderr);
        }
    }

    /// An HTTP failure is the §6 message (no server text, D11).
    #[test]
    fn failures_print_no_server_text() {
        let f = fx();
        f.server.push_once(
            Route::Runtime(Rx::ApiKeys),
            Planned::json(409, &json!({"message": "key FAKE-ECHO exists \u{1b}[31m"})),
        );
        let out = f.rt(&["api-keys", "list"]);
        out.assert_code(1);
        assert_eq!(out.stdout, "", "no banner on a failing run (D222)");
        assert!(!out.stderr.contains("FAKE-ECHO"));
        assert!(out.stderr.contains("HTTP 409"));
    }
}

// ---------------------------------------------------------------------------
// customer-apps list|get (§18.7)
// ---------------------------------------------------------------------------

mod customer_apps {
    use super::*;

    /// TS `management.spec.ts:1123-1150`: names and ids (D207: `customer_appId`), `get`
    /// by name.
    #[test]
    fn list_and_get() {
        let f = fx();
        let out = f.rt(&["customer-apps", "list", "--output", "json"]);
        out.assert_success();
        assert_eq!(f.api(), [format!("GET {APPS}?offset=0&limit=100")]);
        assert_eq!(
            ids(&out, "id"),
            [fx::uuid(101), fx::uuid(102), fx::uuid(103)]
        );
        assert_eq!(ids(&out, "name"), ["app-1", "app-2", "app-3"]);
        assert_eq!(
            out.json()[0]["raw"]["api_keys_dp_info"][0]["auth_code"],
            fx::auth_code(1),
            "auth codes stay on stdout (choice 6)"
        );
        f.clear();
        let out = f.rt(&["customer-apps", "get", "app 1&x", "--output", "json"]);
        out.assert_code(1);
        assert_eq!(f.api(), [format!("GET {APP}?app_name=app+1%26x")]);
        f.clear();
        let out = f.rt(&["customer-apps", "get", "app-2", "--output", "json"]);
        out.assert_success();
        assert_eq!(f.api(), [format!("GET {APP}?app_name=app-2")]);
        assert_eq!(out.json()["id"], json!(fx::uuid(102)));
        assert_eq!(out.json()["name"], "app-2");
        assert_eq!(out.json()["raw"]["environment"], "prod");
    }

    #[test]
    fn layouts() {
        let f = fx();
        let out = f.rt(&["customer-apps", "ls"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            format!(
                "{BANNER}\n  Customer Apps:\n\n  {}\n    app-1\n  {}\n    app-2\n  {}\n    app-3\n\n",
                fx::uuid(101),
                fx::uuid(102),
                fx::uuid(103)
            )
        );
        let out = f.rt(&["customer-apps", "list", "--output", "table"]);
        assert!(
            out.stdout
                .starts_with(" ID                                   │ Name  │ Description \n")
        );
        let out = f.rt(&["customer-apps", "get", "app-1"]);
        assert!(out.stdout.starts_with(&format!(
            "{BANNER}\n  Customer App Detail:\n\n  ID    {}\n  Name  app-1\n  Data  {{\n  \"customer_appId\"",
            fx::uuid(101)
        )));
        assert!(!out.stdout.contains("cut at 500"));
        let out = f.rt(&["customer-apps", "get", "app-1", "--output", "table"]);
        let keys: Vec<&str> = out
            .stdout
            .lines()
            .skip(2)
            .map(|line| line.split('│').next().unwrap().trim())
            .collect();
        assert_eq!(keys, ["id", "name", "description", "raw"]);
    }

    /// D207: a record longer than 500 UTF-16 units is cut and marked.
    #[test]
    fn a_cut_record_is_marked() {
        let f = fx();
        f.server.runtime(|data| {
            data.customer_apps[0]["notes"] = json!("n".repeat(600));
        });
        let out = f.rt(&["customer-apps", "get", "app-1"]);
        out.assert_success();
        assert!(out.stdout.ends_with(
            "\n  … (cut at 500 characters; use --output json for the whole record)\n\n"
        ));
        let out = f.rt(&["--quiet", "customer-apps", "get", "app-1"]);
        assert!(
            out.stdout.contains("cut at 500 characters"),
            "data under --quiet"
        );
    }

    /// D205: an `api_key` nested in a customer-app record is omitted; D219: repeated rows
    /// fail the walk.
    #[test]
    fn nested_keys_and_repeated_rows() {
        let f = fx();
        f.server.runtime(|data| {
            data.customer_apps[0]["api_keys_dp_info"][0]["api_key"] =
                json!(fx::key_secret(TSG, "nested"));
        });
        for args in [
            &["customer-apps", "list", "--output", "json"][..],
            &["customer-apps", "get", "app-1"],
            &["customer-apps", "get", "app-1", "--output", "table"],
        ] {
            f.rt(args).assert_success();
        }
        let f = fx();
        f.server.runtime(|data| data.repeat_rows = true);
        let out = f.rt(&["customer-apps", "list", "--all", "--limit", "2"]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
    }

    /// D222: the app ids are data, kept under `--quiet`; the banner is decoration.
    #[test]
    fn quiet_keeps_every_data_line() {
        let f = fx();
        let out = f.rt(&["--quiet", "customer-apps", "list"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            format!(
                "  {}\n    app-1\n  {}\n    app-2\n  {}\n    app-3\n\n",
                fx::uuid(101),
                fx::uuid(102),
                fx::uuid(103)
            )
        );
        assert_eq!(out.stderr, "");
        let out = f.rt(&["--quiet", "api-keys", "list"]);
        assert!(out.stdout.starts_with(&format!("  {}\n", fx::uuid(1))));
    }

    /// The tenant `defaultOutput` applies to the reads (choice 18).
    #[test]
    fn default_output_is_honoured() {
        let f = fx_with(|config| {
            config.set("defaultOutput", "json");
        });
        for args in [
            &["api-keys", "list"][..],
            &["customer-apps", "list"],
            &["customer-apps", "get", "app-1"],
            &["deployment-profiles", "list"],
            &["customer-apps", "consumption", "chatbot"],
        ] {
            let out = f.rt(args);
            out.assert_success();
            assert!(
                out.stdout.starts_with(['[', '{']),
                "{args:?}: {}",
                out.stdout
            );
        }
    }
}

// ---------------------------------------------------------------------------
// deployment-profiles list (§18.10)
// ---------------------------------------------------------------------------

mod deployment_profiles {
    use super::*;

    /// TS `management.spec.ts:1176-1194`: rows, and `--unactivated` sends
    /// `?unactivated=true` (only with the flag).
    #[test]
    fn lists_and_unactivated() {
        let f = fx();
        let out = f.rt(&["deployment-profiles", "list", "--output", "json"]);
        out.assert_success();
        assert_eq!(f.api(), ["GET /aisec/v1/mgmt/deploymentprofiles"]);
        assert_eq!(
            out.json(),
            json!([
                {"name": "dp-1", "status": "active", "authCode": fx::auth_code(1)},
                {"name": "dp-2", "status": "activated", "authCode": fx::auth_code(2)}
            ])
        );
        assert_eq!(out.stderr, "", "no status line (TS)");
        f.clear();
        let out = f.rt(&[
            "deployment-profiles",
            "ls",
            "--unactivated",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            f.api(),
            ["GET /aisec/v1/mgmt/deploymentprofiles?unactivated=true"]
        );
        assert_eq!(ids(&out, "name"), ["dp-1", "dp-2", "dp-3"]);
    }

    /// D222: an empty list is `[]` in json, nothing in table, the phrase in pretty.
    #[test]
    fn empty_lists() {
        let f = fx();
        f.server.runtime(|data| data.deployment_profiles.clear());
        assert_eq!(
            f.rt(&["deployment-profiles", "list", "--output", "json"])
                .stdout,
            "[]\n"
        );
        assert_eq!(
            f.rt(&["deployment-profiles", "list", "--output", "table"])
                .stdout,
            ""
        );
        assert_eq!(
            f.rt(&["deployment-profiles", "list"]).stdout,
            format!("{BANNER}  No deployment profiles found\n")
        );
    }
}

// ---------------------------------------------------------------------------
// customer-apps consumption (§18.9)
// ---------------------------------------------------------------------------

mod consumption {
    use super::*;

    fn overview(interval: u32, offset: u32) -> String {
        format!("GET {OVERVIEW}?time_interval={interval}&time_unit=days&limit=100&offset={offset}")
    }

    fn reads(id: &str, name: &str, interval: u32) -> [String; 2] {
        let query = format!("appid={id}&appname={name}&time_interval={interval}&time_unit=days");
        [
            format!("GET {APPLICATION}?{query}"),
            format!("GET {BREAKDOWN}?{query}"),
        ]
    }

    /// TS `management.spec.ts:1227-1288`: the app id comes from the overview, both reads
    /// carry `{appId, appName, timeInterval}` (default 30), on the dashboard plane with
    /// `x-tsg-id`; the management base is not used.
    #[test]
    fn a_named_app_reads_the_overview_then_both_breakdowns() {
        let f = fx();
        let out = f.rt(&[
            "customer-apps",
            "consumption",
            "chatbot",
            "--output",
            "json",
        ]);
        out.assert_success();
        let mut expected = vec![overview(30, 0)];
        expected.extend(reads("uuid-shared", "chatbot", 30));
        assert_eq!(f.api(), expected);
        for request in f
            .server
            .requests()
            .iter()
            .filter(|r| r.route != Route::Token)
        {
            assert_eq!(request.header("x-tsg-id"), Some(TSG), "{}", request.line());
        }
        let rows = out.json();
        assert_eq!(rows.as_array().unwrap().len(), 3, "one row per detector");
        assert_eq!(
            rows[0],
            json!({
                "app_name": "chatbot", "app_id": "uuid-shared",
                "monitoring_since": "2026-04-29T17:04:52Z", "daily_avg": "744.233K",
                "monthly_total": "17.71M", "sessions_total": 56935, "sessions_violating": 31136,
                "detector": "topic_guardrails", "critical": 0, "high": 0, "medium": 3, "low": 0,
                "total": 3
            })
        );
    }

    /// `--time-interval 60` is passed to both reads (TS) and, D211, to the lookup too.
    #[test]
    fn the_window_applies_to_the_lookup_and_the_reads() {
        let f = fx();
        f.rt(&[
            "customer-apps",
            "consumption",
            "litellm",
            "--time-interval",
            "60",
            "--output",
            "json",
        ])
        .assert_success();
        let mut expected = vec![overview(60, 0)];
        expected.extend(reads("uuid-litellm", "litellm", 60));
        assert_eq!(f.api(), expected);
    }

    /// TS: a shared app id with two names resolves the named bucket; a missing name lists
    /// names and sends neither read; D211: one name with two ids is refused.
    #[test]
    fn lookups() {
        let f = fx();
        f.rt(&[
            "customer-apps",
            "consumption",
            "LiteLLM",
            "--output",
            "json",
        ])
        .assert_success();
        assert_eq!(f.api()[1], reads("uuid-shared", "LiteLLM", 30)[0]);
        f.clear();
        let out = f.rt(&["customer-apps", "consumption", "nonexistent"]);
        out.assert_code(1);
        assert_eq!(f.api(), [overview(30, 0)]);
        assert!(out.stderr.contains(
            "Dashboard application not found: \"nonexistent\". Available (as shown in SCM AI Applications view): \"chatbot\", \"LiteLLM\", \"litellm\"."
        ));
        let f = fx();
        f.server.runtime(|data| {
            data.apps.push(fx::overview_item("uuid-other", "chatbot"));
        });
        let out = f.rt(&["customer-apps", "consumption", "chatbot"]);
        out.assert_code(1);
        assert_eq!(f.api(), [overview(30, 0)]);
        assert!(out.stderr.contains(
            "Dashboard application name \"chatbot\" matches 2 applications with different ids"
        ));
    }

    /// D209: `null` stats and an empty breakdown print no invented zeros.
    #[test]
    fn absent_values_are_not_invented() {
        let f = fx();
        f.server.runtime(|data| {
            data.applications.insert(
                "uuid-shared|chatbot".to_owned(),
                json!({"name": null, "token_stats": null, "session_stats": null, "profiles": null}),
            );
            data.breakdowns.insert(
                "uuid-shared|chatbot".to_owned(),
                json!({"detection_type_violation_breakdown": []}),
            );
        });
        let out = f.rt(&["customer-apps", "consumption", "chatbot"]);
        out.assert_success();
        assert!(out.stdout.ends_with(
            "\n  chatbot\n  (uuid-shared)\n\n\n  Token consumption:\n\n  Daily avg      -\n  Monthly total  -\n\n  Sessions:\n\n  Total      -\n  Violating  -\n\n  Detectors (? violating, 0/0 firing):\n\n  no detector violations in window\n\n"
        ));
        let out = f.rt(&[
            "customer-apps",
            "consumption",
            "chatbot",
            "--output",
            "json",
        ]);
        let row = &out.json()[0];
        assert_eq!(row["sessions_total"], Value::Null);
        assert_eq!(row["detector"], Value::Null);
        assert_eq!(row["daily_avg"], Value::Null);
    }

    /// TS `management.spec.ts:1397-1414`, D211: the walk follows `total_items` past a
    /// capped page (`offset 2` next, total 3); items without a string id or name are
    /// skipped with a warning.
    #[test]
    fn the_overview_walk_is_complete() {
        let f = fx();
        f.server.runtime(|data| data.page_cap = Some(2));
        let out = f.rt(&[
            "customer-apps",
            "consumption",
            "litellm",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(&f.api()[..2], [overview(30, 0), overview(30, 2)]);
        let f = fx();
        f.server.runtime(|data| {
            data.apps.push(json!({"id": null, "name": "no-id"}));
            data.apps.push(json!({"id": "uuid-3", "name": null}));
        });
        let out = f.rt(&[
            "customer-apps",
            "consumption",
            "litellm",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            out.stderr,
            "  ⚠ 2 dashboard application(s) without an id or name were skipped\n"
        );
        let out = f.rt(&["customer-apps", "consumption", "no-id"]);
        out.assert_code(1);
        // An empty page before the total is the D37 error, not a not-found.
        let f = fx();
        f.server.runtime(|data| data.total_override = Some(9));
        let out = f.rt(&["customer-apps", "consumption", "nope"]);
        out.assert_code(1);
        assert!(!out.stderr.contains("not found"), "{}", out.stderr);
    }

    /// D210: all apps in overview order, one overview walk, one JSON document; an empty
    /// overview prints the phrase (pretty), `[]` (json) or nothing (table).
    #[test]
    fn all_apps_are_one_document() {
        let f = fx();
        let out = f.rt(&["customer-apps", "consumption", "--output", "json"]);
        out.assert_success();
        let mut expected = vec![overview(30, 0)];
        expected.extend(reads("uuid-shared", "chatbot", 30));
        expected.extend(reads("uuid-shared", "LiteLLM", 30));
        expected.extend(reads("uuid-litellm", "litellm", 30));
        assert_eq!(f.api(), expected);
        assert_eq!(out.json().as_array().unwrap().len(), 9);
        f.server.runtime(|data| data.apps.clear());
        assert_eq!(
            f.rt(&["customer-apps", "consumption"]).stdout,
            format!("{BANNER}  No dashboard applications found\n")
        );
        assert_eq!(
            f.rt(&["customer-apps", "consumption", "--output", "json"])
                .stdout,
            "[]\n"
        );
        assert_eq!(
            f.rt(&["customer-apps", "consumption", "--output", "table"])
                .stdout,
            ""
        );
    }

    /// D210: one failing app is named on stderr, the others are printed, exit 1; an app
    /// without detectors is one row with `detector: null`.
    #[test]
    fn a_failing_app_is_reported_and_the_run_exits_1() {
        let f = fx();
        f.server.runtime(|data| {
            data.breakdowns.insert(
                "uuid-litellm|litellm".to_owned(),
                json!({"detection_type_violation_breakdown": []}),
            );
        });
        f.server.set_default(
            Route::Runtime(Rx::Application),
            Planned::json(500, &json!({"message": "FAKE-ECHO boom"})),
        );
        f.server.clear_overrides(Route::Runtime(Rx::Application));
        f.server
            .push_once(Route::Runtime(Rx::Application), Planned::builtin());
        f.server.push_once(
            Route::Runtime(Rx::Application),
            Planned::json(400, &json!({"message": "FAKE-ECHO bad"})),
        );
        let out = f.rt(&["customer-apps", "consumption", "--output", "json"]);
        out.assert_code(1);
        let rows = out.json();
        let names: Vec<&str> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["app_name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["chatbot", "chatbot", "chatbot", "litellm"]);
        assert_eq!(rows[3]["detector"], Value::Null);
        assert!(
            out.stderr
                .starts_with("  ✗ [LiteLLM] AISEC_CLIENT_SIDE_ERROR:API error 400 (HTTP 400)\n"),
            "{}",
            out.stderr
        );
        assert!(!out.stderr.contains("FAKE-ECHO"));
    }

    /// §18.9: when every app's reads fail the overview was not empty, so pretty prints the
    /// banner and no "No dashboard applications found"; each app is named on stderr, json
    /// is `[]`, table nothing, and the run exits 1.
    #[test]
    fn every_app_failing_is_not_an_empty_overview() {
        let f = fx();
        f.server.set_default(
            Route::Runtime(Rx::Application),
            Planned::json(400, &json!({"message": "FAKE-ECHO bad"})),
        );
        let out = f.rt(&["customer-apps", "consumption"]);
        out.assert_code(1);
        assert_eq!(out.stdout, BANNER);
        assert_eq!(
            out.stderr,
            "  ✗ [chatbot] AISEC_CLIENT_SIDE_ERROR:API error 400 (HTTP 400)\n  ✗ [LiteLLM] AISEC_CLIENT_SIDE_ERROR:API error 400 (HTTP 400)\n  ✗ [litellm] AISEC_CLIENT_SIDE_ERROR:API error 400 (HTTP 400)\n"
        );
        let mut expected = vec![overview(30, 0)];
        for (id, name) in [
            ("uuid-shared", "chatbot"),
            ("uuid-shared", "LiteLLM"),
            ("uuid-litellm", "litellm"),
        ] {
            expected.push(reads(id, name, 30)[0].clone());
        }
        assert_eq!(
            f.api(),
            expected,
            "a failed application read skips its breakdown"
        );
        let out = f.rt(&["customer-apps", "consumption", "--output", "json"]);
        out.assert_code(1);
        assert_eq!(out.stdout, "[]\n");
        let out = f.rt(&["customer-apps", "consumption", "--output", "table"]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
    }
}

// ---------------------------------------------------------------------------
// scan-logs query (§18.11)
// ---------------------------------------------------------------------------

mod scan_logs {
    use super::*;
    #[test]
    fn discovery_is_bounded_and_reports_unresolved_identifiers() {
        let f = fx();
        let out = f.rt(&["scan-logs", "query", "--interval", "24", "--unit", "hours"]);
        out.assert_code(1);
        assert!(out.stdout.contains("coverage"));
        assert!(out.stdout.contains("false"));
        assert!(!out.stderr.contains("broken and under refactor"));
        assert!(f.server.request_count() > 0);
        let out = f.rt(&[
            "scan-logs",
            "query",
            "--interval",
            "1",
            "--unit",
            "x",
            "--page",
            "4",
        ]);
        out.assert_code(2);
    }
    #[test]
    fn the_tenant_is_read_and_output_validated() {
        let f = fx_with(|c| {
            c.remove("mgmtClientSecret");
        });
        let out = f.rt(&["scan-logs", "query", "--interval", "1", "--unit", "hours"]);
        out.assert_code(1);
        assert!(out.stdout.contains("unavailable"));
        f.rt(&[
            "scan-logs",
            "query",
            "--interval",
            "1",
            "--unit",
            "hours",
            "--output",
            "yaml",
        ])
        .assert_code(2);
        let sandbox = Sandbox::new();
        let out = run(
            &rust(),
            &sandbox,
            &[
                "runtime",
                "scan-logs",
                "query",
                "--interval",
                "1",
                "--unit",
                "hours",
            ],
            &RunOptions::new(),
        );
        out.assert_code(1);
        assert!(out.stderr.contains("No tenant"));
        f.rt(&["scan-logs", "query", "--unit", "hours"])
            .assert_code(2);
    }
}

// ---------------------------------------------------------------------------
// Argument checks (D220) and shared rules
// ---------------------------------------------------------------------------

mod checks {
    use super::*;

    /// Every §18.3 check of these commands is a usage error before the tenant: no request
    /// and no debug log, also with an unusable tenant store.
    #[test]
    fn argument_errors_are_usage_errors_before_the_tenant() {
        let sandbox = Sandbox::new();
        let state = debug_log::xdg_state_home(&sandbox).display().to_string();
        for (args, message) in [
            (
                &["api-keys", "list", "--limit", "0"][..],
                "--limit must be a positive integer",
            ),
            (
                &["api-keys", "ls", "--limit", "1.5"],
                "--limit must be a positive integer",
            ),
            (
                &["api-keys", "ls", "--offset", "-1"],
                "--offset must be a non-negative integer",
            ),
            (
                &["customer-apps", "ls", "--max", "-1"],
                "--max must be a non-negative integer",
            ),
            (
                &["customer-apps", "list", "--all", "--offset", "0"],
                "--all cannot be combined with --offset",
            ),
            (
                &["customer-apps", "get", ""],
                "Customer app name must not be empty",
            ),
            (
                &["customer-apps", "consumption", " "],
                "appName must not be empty; omit it to report every dashboard application",
            ),
            (
                &["customer-apps", "consumption", "--time-interval", "14"],
                "--time-interval must be 7, 30, or 60 (the API rejects other values)",
            ),
            (
                &["customer-apps", "consumption", "--time-interval", "60abc"],
                "--time-interval must be 7, 30, or 60 (the API rejects other values)",
            ),
            (
                &["api-keys", "list", "--output", "yaml"],
                "Invalid output format 'yaml'. Expected: pretty, table, json",
            ),
            (
                &["deployment-profiles", "list", "--output", "csv"],
                "Invalid output format 'csv'. Expected: pretty, table, json",
            ),
            (
                &["customer-apps", "get", "x", "--output", "markdown"],
                "Invalid output format 'markdown'. Expected: pretty, table, json",
            ),
        ] {
            let mut full = vec!["--debug", "runtime"];
            full.extend_from_slice(args);
            let out = run(
                &rust(),
                &sandbox,
                &full,
                &RunOptions::new().env("XDG_STATE_HOME", &state),
            );
            out.assert_code(2);
            assert_eq!(out.stderr, format!("\n  ✗ {message}\n\n"), "{args:?}");
            assert!(
                debug_log::log_files(&debug_log::log_dir(&sandbox)).is_empty(),
                "{args:?}: a debug log"
            );
        }
    }

    /// TS `aliases.spec.ts:55-79`: `ls` on the lists; group help without a subcommand.
    #[test]
    fn aliases_and_group_help() {
        let f = fx();
        for args in [
            &["api-keys", "ls"][..],
            &["customer-apps", "ls"],
            &["deployment-profiles", "ls"],
        ] {
            f.rt(args).assert_success();
        }
        for group in [
            "api-keys",
            "customer-apps",
            "deployment-profiles",
            "scan-logs",
        ] {
            let out = f.rt(&[group]);
            out.assert_code(2);
            assert!(out.stderr.contains("Usage: airs runtime"), "{group}");
        }
        // Step 1 registers the reads only (§18.1).
        let out = f.rt(&["api-keys", "create", "--config", "c.json"]);
        out.assert_code(2);
    }

    /// TS `client-options.spec.ts:113-133`: `mgmtEndpoint` serves keys and apps,
    /// `mgmtDashboardEndpoint` the dashboard (its own key, not `mgmtEndpoint`); `PANW_*`
    /// variables are ignored; an inadmissible dashboard endpoint is refused by key.
    #[test]
    fn endpoints_come_from_the_tenant_only() {
        let f = fx();
        let trap = f.server.ts_env_trap_url();
        let options = RunOptions::new()
            .env("PANW_MGMT_ENDPOINT", &trap)
            .env("PANW_MGMT_DASHBOARD_ENDPOINT", &trap);
        f.rt_with(&["api-keys", "list"], &options).assert_success();
        f.rt_with(&["customer-apps", "consumption", "chatbot"], &options)
            .assert_success();
        assert!(f.server.requests_for(Route::TsEnvTrap).is_empty());
        // An inadmissible dashboard endpoint is refused by its key, with no request; the
        // other commands do not admit it (their policy does not widen).
        let server = FixtureServer::start();
        server.seed_runtime();
        let sandbox = Sandbox::new();
        let tenant = sandbox.add_fixture_tenant_allowing_external_endpoints(
            &server,
            "prod",
            TSG,
            |config| {
                config.set(
                    "mgmtDashboardEndpoint",
                    "http://dashboard.example.invalid/aisec",
                );
            },
        );
        sandbox.register(&[&tenant], Some("prod"));
        let out = run(
            &rust(),
            &sandbox,
            &["runtime", "customer-apps", "consumption", "chatbot"],
            &RunOptions::new(),
        );
        out.assert_code(1);
        assert!(
            out.stderr
                .contains("Invalid mgmtDashboardEndpoint: endpoints must use https"),
            "{}",
            out.stderr
        );
        assert_eq!(server.request_count(), 0);
        let out = run(
            &rust(),
            &sandbox,
            &["runtime", "api-keys", "list", "--output", "json"],
            &RunOptions::new(),
        );
        out.assert_success();
    }

    /// D64: `--debug` records status and headers only: no query (`app_name`, appids),
    /// no body, no auth code; the SDK operation names (G34).
    #[test]
    fn the_debug_log_records_no_bodies_or_queries() {
        for (args, operations) in [
            (
                vec!["api-keys", "list"],
                vec!["airs.management.api_keys.list"],
            ),
            (
                vec!["customer-apps", "get", "app-1"],
                vec!["airs.management.customer_apps.get"],
            ),
            (
                vec!["customer-apps", "list"],
                vec!["airs.management.customer_apps.list"],
            ),
            (
                vec!["deployment-profiles", "list", "--unactivated"],
                vec!["airs.management.deployment_profiles.list"],
            ),
            (
                vec!["customer-apps", "consumption", "chatbot"],
                vec![
                    "airs.management.dashboard.applications_overview",
                    "airs.management.dashboard.application",
                    "airs.management.dashboard.application_violation_breakdown",
                ],
            ),
        ] {
            let f = fx();
            let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
            let mut full = vec!["--debug", "runtime"];
            full.extend(args.iter().copied());
            let out = f.run_with(&full, &RunOptions::new().env("XDG_STATE_HOME", &state));
            out.assert_success();
            let (path, records) = debug_log::only_log(&f.sandbox);
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                !text.contains("app-1") && !text.contains("chatbot"),
                "{args:?}"
            );
            let found: Vec<&str> = records
                .iter()
                .filter_map(|record| record["operation"].as_str())
                .filter(|operation| *operation != "oauth.token")
                .collect();
            assert_eq!(found, operations, "{args:?}");
            debug_log::assert_logs_clean(
                &f.sandbox,
                &f.server,
                &[debug_log::log_dir(&f.sandbox)],
                &[fx::AUTHCODE_MARKER, "LAST8-01"],
            );
        }
        // Evidence reads refuse debug before opening a log.
        let f = fx();
        let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
        let out = f.run_with(
            &[
                "--debug",
                "runtime",
                "scan-logs",
                "query",
                "--interval",
                "1",
                "--unit",
                "h",
            ],
            &RunOptions::new().env("XDG_STATE_HOME", &state),
        );
        out.assert_code(2);
        assert!(debug_log::log_files(&debug_log::log_dir(&f.sandbox)).is_empty());
    }

    /// D52: server text in pretty layouts, table cells and messages is terminal-safe.
    #[test]
    fn server_text_is_terminal_safe() {
        let f = fx();
        f.server.runtime(|data| {
            data.api_keys[0]["api_key_name"] = json!("evil\u{1b}[31mRED\nline2");
            data.customer_apps[0]["description"] = json!("desc\u{1b}[2J");
            data.apps[0]["name"] = json!("bad\u{9b}31m");
        });
        for args in [
            &["api-keys", "list"][..],
            &["api-keys", "list", "--output", "table"],
            &["customer-apps", "list"],
            &["customer-apps", "consumption", "nope"],
        ] {
            let out = f.rt(args);
            for text in [&out.stdout, &out.stderr] {
                assert!(
                    !text.chars().any(|c| c.is_control() && c != '\n'),
                    "{args:?}: {text:?}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Interruption (D224)
// ---------------------------------------------------------------------------

mod interrupt {
    use super::*;

    /// A signal during a read or a walk writes no data (exit 130 / 143).
    #[test]
    fn a_signal_during_a_read_writes_nothing() {
        for (signal, code, route, args) in [
            ("INT", 130, Rx::ApiKeys, vec!["runtime", "api-keys", "list"]),
            (
                "TERM",
                143,
                Rx::AppsOverview,
                vec!["runtime", "customer-apps", "consumption"],
            ),
        ] {
            let f = fx();
            f.server.push_once(Route::Runtime(route), Planned::hang());
            let running = spawn(
                &rust(),
                &f.sandbox,
                &args,
                &RunOptions::new().expect_signal(),
            );
            assert!(
                f.server
                    .wait_for_route(Route::Runtime(route), 1, Duration::from_secs(10))
            );
            running.send_signal(signal);
            let out = running.wait();
            out.assert_code(code);
            assert_eq!(out.stdout, "");
            assert_eq!(
                out.stderr,
                "\n  ✗ Error: Interrupted; no data was written\n\n"
            );
        }
    }
}
