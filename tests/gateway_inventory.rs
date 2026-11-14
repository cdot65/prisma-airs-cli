//! Executable acceptance tests for the M6e chunk-2 gateway reads (DESIGN §19.9, §19.13):
//! every inventory read (`api-keys service|user`, `audit-logs`, `configs`, `deployments`,
//! `guardrails`, `integrations`, `mcp integrations`, `organisations`, `plugins`,
//! `providers`) and the `admin-guardrails` reads.
//!
//! Every test spawns the real `airs-cli-rs` against the loopback fixture server with the
//! seeded gateway dataset, whose answers carry the `FAKE-GW-SECRET-*` markers in every
//! credential the service can return; the runner's leak check forbids every marker on
//! every stream and in the debug log of each run (DESIGN §10 M6e). The chunk-2 cases of the
//! TypeScript `tests/unit/cli/aigateway-read-commands.spec.ts`,
//! `aigateway-admin-guardrails.spec.ts`, `tests/integration/aigateway-admin-guardrails.spec.ts`
//! and `aigateway-command-tree.spec.ts` are ported here (§19.19), adjusted by the §12 rows.

use std::time::Duration;

use airs_test_harness::fixtures::gateway as fx;
use airs_test_harness::{
    Cli, FixtureConfig, FixtureServer, GatewayData, Gw, Planned, RecordedRequest, Route, Run,
    RunOptions, Sandbox, debug_log, run, spawn,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

const TSG: &str = "100";
const DATA: &str = "/ai_gw/v2";
const ADMIN: &str = "/ai_gw/admin/v2";
const WORKSPACE_HINT: &str = "  ⚠ 403: the service account is missing a workspace-scope grant (data plane, /ai_gw/v2). SCM Access Management edits the existing role row by default — use \"Add Role\" so the account ends up with both role rows, not one row moved.\n";
const ADMIN_HINT: &str = "  ⚠ 403: the service account is missing a tenant-root admin grant (admin plane, /ai_gw/admin/v2). SCM Access Management edits the existing role row by default — use \"Add Role\" so the account ends up with both role rows, not one row moved.\n";
const REVEAL_REFUSED: &str =
    "--reveal-sensitive is not supported here: airs never prints credentials";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

fn u(n: u64) -> String {
    fx::uuid(n)
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
    server.seed_gateway();
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
        out
    }

    /// `gateway <args>`, with the read-only and loopback checks.
    fn gw(&self, args: &[&str]) -> Run {
        let mut full = vec!["gateway"];
        full.extend_from_slice(args);
        self.run_with(&full, &RunOptions::new())
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

    fn data(&self, edit: impl FnOnce(&mut GatewayData)) {
        self.server.gateway(edit);
    }
}

fn commands(help: &str) -> Vec<String> {
    help.lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .map(str::to_owned)
        .collect()
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn refs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

/// Every chunk-2 read with its one request (TS `aigateway-read-commands.spec.ts`: the SDK
/// mappings; ids and workspace UUIDs passed through unchanged).
fn reads() -> Vec<(Vec<String>, String)> {
    let ws = u(1);
    vec![
        (
            strings(&["api-keys", "service", "list", "--workspace", &ws]),
            format!("GET {DATA}/api-keys/service?workspace_id={ws}"),
        ),
        (
            strings(&["api-keys", "service", "get", &u(fx::SERVICE_KEY)]),
            format!("GET {DATA}/api-keys/service/{}", u(fx::SERVICE_KEY)),
        ),
        (
            strings(&["api-keys", "user", "ls", "--workspace", &ws]),
            format!("GET {DATA}/api-keys/user?workspace_id={ws}"),
        ),
        (
            strings(&["api-keys", "user", "get", &u(fx::USER_KEY)]),
            format!("GET {DATA}/api-keys/user/{}", u(fx::USER_KEY)),
        ),
        (
            strings(&[
                "audit-logs",
                "list",
                "--start",
                "2026-10-01T00:00:00Z",
                "--end",
                "2026-10-02T00:00:00Z",
            ]),
            format!(
                "GET {ADMIN}/audit-logs?start_time=2026-10-01T00%3A00%3A00.000Z&end_time=2026-10-02T00%3A00%3A00.000Z"
            ),
        ),
        (
            strings(&["configs", "list", "--workspace", &ws]),
            format!("GET {DATA}/configs?workspace_id={ws}"),
        ),
        (
            strings(&["configs", "get", &u(fx::CONFIG)]),
            format!("GET {DATA}/configs/{}", u(fx::CONFIG)),
        ),
        (
            strings(&["configs", "versions", &u(fx::CONFIG)]),
            format!("GET {DATA}/configs/{}/versions", u(fx::CONFIG)),
        ),
        (
            strings(&["deployments", "ls"]),
            format!("GET {ADMIN}/deployments"),
        ),
        (
            strings(&["deployments", "get", &u(fx::DEPLOYMENT)]),
            format!("GET {ADMIN}/deployments/{}", u(fx::DEPLOYMENT)),
        ),
        (
            strings(&["deployments", "ping", &u(fx::DEPLOYMENT)]),
            format!("GET {ADMIN}/deployments/{}/ping", u(fx::DEPLOYMENT)),
        ),
        (
            strings(&["guardrails", "list", "--workspace", &ws]),
            format!("GET {DATA}/guardrails?workspace_id={ws}"),
        ),
        (
            strings(&["guardrails", "get", &u(fx::GUARDRAIL)]),
            format!("GET {DATA}/guardrails/{}", u(fx::GUARDRAIL)),
        ),
        (
            strings(&["guardrails", "catalog"]),
            format!("GET {ADMIN}/utils/static-resources/schema?resource=guardrails"),
        ),
        (
            strings(&["integrations", "list"]),
            format!("GET {ADMIN}/integrations"),
        ),
        (
            strings(&["integrations", "get", &u(fx::INTEGRATION)]),
            format!("GET {ADMIN}/integrations/{}", u(fx::INTEGRATION)),
        ),
        (
            strings(&["integrations", "providers"]),
            format!("GET {ADMIN}/utils/static-resources/ai-providers"),
        ),
        (
            strings(&["integrations", "models", "list", &u(fx::INTEGRATION)]),
            format!("GET {ADMIN}/integrations/{}/models", u(fx::INTEGRATION)),
        ),
        (
            strings(&["integrations", "workspaces", "ls", &u(fx::INTEGRATION)]),
            format!("GET {ADMIN}/integrations/{}/workspaces", u(fx::INTEGRATION)),
        ),
        (
            strings(&["mcp", "integrations", "list"]),
            format!("GET {ADMIN}/mcp-integrations"),
        ),
        (
            strings(&["mcp", "integrations", "get", &u(fx::MCP_INTEGRATION)]),
            format!("GET {ADMIN}/mcp-integrations/{}", u(fx::MCP_INTEGRATION)),
        ),
        (
            strings(&[
                "mcp",
                "integrations",
                "capabilities",
                "list",
                &u(fx::MCP_INTEGRATION),
            ]),
            format!(
                "GET {ADMIN}/mcp-integrations/{}/capabilities",
                u(fx::MCP_INTEGRATION)
            ),
        ),
        (
            strings(&["mcp", "integrations", "metadata", &u(fx::MCP_INTEGRATION)]),
            format!(
                "GET {ADMIN}/mcp-integrations/{}/metadata",
                u(fx::MCP_INTEGRATION)
            ),
        ),
        // TS reads the integration detail (choice 37).
        (
            strings(&[
                "mcp",
                "integrations",
                "workspaces",
                "list",
                &u(fx::MCP_INTEGRATION),
            ]),
            format!("GET {ADMIN}/mcp-integrations/{}", u(fx::MCP_INTEGRATION)),
        ),
        (
            strings(&["organisations", "info"]),
            format!("GET {ADMIN}/organisations/{TSG}/info"),
        ),
        (
            strings(&["organisations", "self", "get"]),
            format!("GET {ADMIN}/organisations/self"),
        ),
        (
            strings(&["organisations", "auth-settings", "get", "--tsg-id", TSG]),
            format!("GET {ADMIN}/organisations/{TSG}/auth-settings"),
        ),
        (
            strings(&["plugins", "list"]),
            format!("GET {ADMIN}/plugins"),
        ),
        (
            strings(&["providers", "list", "--workspace", &ws]),
            format!("GET {DATA}/providers?workspace_id={ws}"),
        ),
        (
            strings(&["providers", "get", &u(fx::PROVIDER)]),
            format!("GET {DATA}/providers/{}", u(fx::PROVIDER)),
        ),
        (
            strings(&["admin-guardrails", "list"]),
            format!("GET {ADMIN}/guardrails"),
        ),
        (
            strings(&["admin-guardrails", "get", &u(fx::ADMIN_GUARDRAIL)]),
            format!("GET {ADMIN}/guardrails/{}", u(fx::ADMIN_GUARDRAIL)),
        ),
        (
            strings(&[
                "admin-guardrails",
                "mcp-servers",
                "ls",
                &u(fx::ADMIN_GUARDRAIL),
            ]),
            format!(
                "GET {ADMIN}/guardrails/{}/mcp-servers",
                u(fx::ADMIN_GUARDRAIL)
            ),
        ),
    ]
}

// ---------------------------------------------------------------------------
// The command tree (TS aigateway-command-tree.spec.ts, chunk 2)
// ---------------------------------------------------------------------------

mod tree {
    use super::*;

    /// The chunk-2 groups and their read leaves in TypeScript's sorted order (the chunk-3
    /// write leaves between them are pinned by `tests/gateway_writes.rs::tree`); `mcp
    /// integrations` nests under `mcp`.
    #[test]
    fn groups_list_their_read_leaves() {
        let f = fx();
        assert_eq!(
            commands(&f.gw(&["--help"]).stdout),
            [
                "admin-guardrails",
                "api-keys",
                "audit-logs",
                "configs",
                "deployments",
                "guardrails",
                "inference",
                "integrations",
                "mcp",
                "organisations",
                "plugins",
                "providers",
                "report",
                "scopes",
                "telemetry",
                "workspaces",
            ]
        );
        for (path, expected) in [
            (
                &["admin-guardrails"][..],
                &["get", "list", "mcp-servers"][..],
            ),
            (&["admin-guardrails", "mcp-servers"], &["list"]),
            (&["api-keys"], &["service", "user"]),
            (&["api-keys", "service"], &["get", "list"]),
            (&["api-keys", "user"], &["get", "list"]),
            (&["audit-logs"], &["list"]),
            (&["configs"], &["get", "list", "versions"]),
            (&["deployments"], &["get", "list", "ping"]),
            (&["guardrails"], &["catalog", "get", "list"]),
            (
                &["integrations"],
                &["get", "list", "models", "providers", "workspaces"],
            ),
            (&["integrations", "models"], &["list"]),
            (&["integrations", "workspaces"], &["list"]),
            (&["mcp"], &["integrations"]),
            (
                &["mcp", "integrations"],
                &["capabilities", "get", "list", "metadata", "workspaces"],
            ),
            (&["mcp", "integrations", "capabilities"], &["list"]),
            (&["mcp", "integrations", "workspaces"], &["list"]),
            (&["organisations"], &["auth-settings", "info", "self"]),
            (&["organisations", "self"], &["get"]),
            (&["organisations", "auth-settings"], &["get"]),
            (&["plugins"], &["list"]),
            (&["providers"], &["get", "list"]),
        ] {
            let mut args = path.to_vec();
            args.push("--help");
            let writes = [
                "archive", "bind", "create", "delete", "rotate", "set", "sync", "update", "upsert",
            ];
            let reads: Vec<String> = commands(&f.gw(&args).stdout)
                .into_iter()
                .filter(|name| !writes.contains(&name.as_str()))
                .collect();
            assert_eq!(reads, expected, "{path:?}");
        }
        assert!(f.server.requests().is_empty());
    }

    /// A group without a subcommand prints its help on stderr, exit 2 (D1); every list has
    /// `ls`; the refused `--reveal-sensitive` is hidden (D238) and the audit-log one shown
    /// with its Rust help (D239); `--tsg-id` is optional (D245).
    #[test]
    fn help_aliases_and_hidden_options() {
        let f = fx();
        for group in [
            &["configs"][..],
            &["api-keys", "user"],
            &["mcp", "integrations"],
            &["organisations", "auth-settings"],
        ] {
            let out = f.gw(group);
            out.assert_code(2);
            assert_eq!(out.stdout, "");
            assert!(out.stderr.contains("Commands:"), "{group:?}");
        }
        for group in [
            &["configs"][..],
            &["deployments"],
            &["api-keys", "service"],
            &["integrations", "models"],
            &["mcp", "integrations", "workspaces"],
            &["admin-guardrails", "mcp-servers"],
            &["plugins"],
        ] {
            let mut args = group.to_vec();
            args.push("--help");
            assert!(f.gw(&args).stdout.contains("[aliases: ls]"), "{group:?}");
        }
        for leaf in [
            &["api-keys", "service", "list"][..],
            &["api-keys", "user", "get"],
            &["providers", "get"],
            &["organisations", "auth-settings", "get"],
        ] {
            let mut args = leaf.to_vec();
            args.push("--help");
            let help = f.gw(&args).stdout;
            assert!(!help.contains("reveal"), "{leaf:?}: {help}");
            assert!(
                help.contains("Output format: pretty, table, json"),
                "{leaf:?}"
            );
        }
        let help = f.gw(&["audit-logs", "list", "--help"]).stdout;
        assert!(help.contains(
            "--reveal-sensitive  Show request fields; credentials inside them stay withheld"
        ));
        let help = f.gw(&["organisations", "info", "--help"]).stdout;
        assert!(help.contains(
            "--tsg-id <tsg>     Numeric TSG id (default: the selected tenant's mgmtTsgId; another value is refused)"
        ), "{help}");
        assert!(f.server.requests().is_empty());
    }
}

// ---------------------------------------------------------------------------
// The reads (TS aigateway-read-commands.spec.ts)
// ---------------------------------------------------------------------------

mod reads {
    use super::*;

    /// Every read sends its one request on its plane with the tenant's `x-tsg-id` and a
    /// bearer, and succeeds in every format (the runner forbids every marker).
    #[test]
    fn every_read_sends_its_one_request() {
        let f = fx();
        for (args, line) in reads() {
            for format in ["pretty", "table", "json"] {
                f.clear();
                let mut full = refs(&args);
                full.extend(["--output", format]);
                let out = f.gw(&full);
                out.assert_success();
                assert_eq!(f.api(), std::slice::from_ref(&line), "{full:?}");
                for request in f.server.requests() {
                    if request.route != Route::Token {
                        assert_eq!(request.header("x-tsg-id"), Some(TSG));
                        assert!(
                            request
                                .header("authorization")
                                .is_some_and(|value| value.starts_with("Bearer ")),
                            "{full:?}"
                        );
                    }
                }
                if format == "json" {
                    let _ = out.json();
                }
            }
        }
    }

    /// TS "emits structured data only on stdout": json stdout parses, stderr is empty, and
    /// lists print the rows (`responseItems`) while details print the record.
    #[test]
    fn json_is_the_only_stdout() {
        let f = fx();
        let out = f.gw(&["plugins", "list", "--output", "json"]);
        out.assert_success();
        assert_eq!(out.stderr, "");
        let rows = out.json();
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["id"], u(fx::PLUGIN));
        let out = f.gw(&["configs", "get", &u(fx::CONFIG), "--output", "json"]);
        assert_eq!(out.json()["id"], u(fx::CONFIG));
        assert_eq!(out.json()["format"], "json");
        let out = f.gw(&[
            "admin-guardrails",
            "mcp-servers",
            "list",
            &u(fx::ADMIN_GUARDRAIL),
            "--output",
            "json",
        ]);
        assert_eq!(out.json()[0]["run_on"], json!(["input"]));
        // The `{data: […]}` form of the mapping list (TS union) prints the same rows.
        f.data(|data| {
            data.inventory
                .insert(Gw::AdminMcpMappings, json!({"data": [fx::mcp_mapping()]}));
        });
        let again = f.gw(&[
            "admin-guardrails",
            "mcp-servers",
            "list",
            &u(fx::ADMIN_GUARDRAIL),
            "--output",
            "json",
        ]);
        assert_eq!(again.json(), out.json());
    }

    /// The generic list: TS's preferred columns then the others sorted, at most 8, in pretty
    /// and table alike; a withheld object stays out of the columns (choice 79).
    #[test]
    fn lists_print_typescript_columns() {
        let f = fx();
        let pretty = f.gw(&["deployments", "list"]).stdout;
        let table = f.gw(&["deployments", "list", "--output", "table"]).stdout;
        assert_eq!(pretty, table);
        assert_eq!(
            pretty.lines().next().unwrap(),
            " Id                                   │ Slug │ Name │ Type       │ Status │ Created At               │ Last Updated At          │ Created By "
        );
        let plugins = f.gw(&["plugins", "list"]).stdout;
        assert!(!plugins.contains("Credentials"), "{plugins}");
        assert!(plugins.contains(" Owner Id "), "{plugins}");
    }

    /// The generic detail: `Field │ Value` of the top-level members in record order,
    /// nested values as compact JSON.
    #[test]
    fn details_print_field_value_tables() {
        let f = fx();
        let out = f.gw(&["deployments", "ping", &u(fx::DEPLOYMENT)]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            " Field            │ Value                                               \n──────────────────┼─────────────────────────────────────────────────────\n status           │ healthy                                             \n gateway_base_url │ https://gw.example.test                             \n outbound         │ {\"status\":\"ok\",\"status_code\":200,\"version\":\"1.2.3\"} \n inbound          │ {\"status\":\"ok\"}                                     \n object           │ deployment.ping                                     \n"
        );
        assert_eq!(
            out.stdout,
            f.gw(&[
                "deployments",
                "ping",
                &u(fx::DEPLOYMENT),
                "--output",
                "table"
            ])
            .stdout
        );
    }

    /// Empty lists: pretty the TS phrase with the TS label, table nothing, json `[]`
    /// (D231).
    #[test]
    fn empty_lists() {
        let f = fx();
        let other = u(9);
        for (args, label) in [
            (
                strings(&["api-keys", "service", "list", "--workspace", &other]),
                "service API keys",
            ),
            (
                strings(&["api-keys", "user", "list", "--workspace", &other]),
                "user API keys",
            ),
            (
                strings(&["configs", "list", "--workspace", &other]),
                "configs",
            ),
            (
                strings(&["guardrails", "list", "--workspace", &other]),
                "guardrails",
            ),
            (
                strings(&["providers", "list", "--workspace", &other]),
                "providers",
            ),
        ] {
            let out = f.gw(&refs(&args));
            out.assert_success();
            assert_eq!(out.stdout, format!("  No {label} found\n"), "{args:?}");
            let mut table = refs(&args);
            table.extend(["--output", "table"]);
            assert_eq!(f.gw(&table).stdout, "");
            let mut json = refs(&args);
            json.extend(["--output", "json"]);
            assert_eq!(f.gw(&json).stdout, "[]\n");
        }
        f.data(|data| {
            data.inventory
                .insert(Gw::AdminGuardrails, json!({"data": [], "total": 0}));
        });
        assert_eq!(
            f.gw(&["admin-guardrails", "list"]).stdout,
            "  No Admin guardrails found\n"
        );
    }

    /// D253: `integrations models list` lists the answer's `models`.
    #[test]
    fn integration_models_are_listed() {
        let f = fx();
        let out = f.gw(&[
            "integrations",
            "models",
            "list",
            &u(fx::INTEGRATION),
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(out.json()[0]["slug"], "gpt-4o");
        let pretty = f.gw(&["integrations", "models", "ls", &u(fx::INTEGRATION)]);
        assert!(pretty.stdout.starts_with(" Slug "), "{}", pretty.stdout);
        f.data(|data| {
            data.inventory.insert(
                Gw::IntegrationModels,
                json!({"allow_all_models": true, "models": [], "object": "list"}),
            );
        });
        assert_eq!(
            f.gw(&["integrations", "models", "list", &u(fx::INTEGRATION)])
                .stdout,
            "  No integration models found\n"
        );
    }

    /// `audit-logs list`: the rolling window (default 7 days), sent with milliseconds and
    /// `Z` (TS `toISOString`); an offset `--start` is sent in UTC.
    #[test]
    fn audit_windows() {
        let f = fx();
        f.gw(&["audit-logs", "list", "--output", "json"])
            .assert_success();
        let requests = f.server.gw_requests(Gw::AuditLogs);
        let instant = |key: &str| -> i64 {
            let text = requests[0].query_value(key).unwrap().to_owned();
            assert!(text.ends_with('Z') && text.len() == 24, "{text}");
            chrono_free_ms(&text)
        };
        assert_eq!(instant("end_time") - instant("start_time"), 7 * 86_400_000);
        f.clear();
        f.gw(&[
            "audit-logs",
            "ls",
            "--days",
            "2",
            "--end",
            "2026-10-03T00:00:00.000+02:00",
            "--output",
            "json",
        ])
        .assert_success();
        assert_eq!(
            f.api(),
            [format!(
                "GET {ADMIN}/audit-logs?start_time=2026-09-30T22%3A00%3A00.000Z&end_time=2026-10-02T22%3A00%3A00.000Z"
            )]
        );
    }

    /// `YYYY-MM-DDTHH:MM:SS.mmmZ` in epoch milliseconds (days from the civil calendar).
    fn chrono_free_ms(text: &str) -> i64 {
        let number = |range: std::ops::Range<usize>| text[range].parse::<i64>().unwrap();
        let (year, month, day) = (number(0..4), number(5..7), number(8..10));
        let (hour, minute, second, millis) = (
            number(11..13),
            number(14..16),
            number(17..19),
            number(20..23),
        );
        let y = if month <= 2 { year - 1 } else { year };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (month + 9) % 12;
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        ((days * 24 + hour) * 60 + minute) * 60_000 + second * 1000 + millis
    }

    /// `organisations info|auth-settings get` default to the tenant's TSG (D245); the same
    /// value given explicitly is accepted.
    #[test]
    fn organisation_reads_default_to_the_tenant_tsg() {
        let f = fx();
        for args in [
            &["organisations", "info"][..],
            &["organisations", "info", "--tsg-id", TSG],
            &["organisations", "auth-settings", "get"],
        ] {
            f.clear();
            f.gw(args).assert_success();
            assert!(
                f.api()[0].contains(&format!("/organisations/{TSG}/")),
                "{args:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Withholding (§19.3; D238, D239, D251)
// ---------------------------------------------------------------------------

mod withholding {
    use super::*;

    /// Every credential the reads return is `[withheld]` in every format (the runner
    /// forbids each marker), and `masked_api_key` (the service's fingerprint) is printed.
    #[test]
    fn credentials_are_withheld_in_every_format() {
        let f = fx();
        for (args, pointer) in [
            (
                strings(&["api-keys", "service", "get", &u(fx::SERVICE_KEY)]),
                "/key",
            ),
            (
                strings(&["api-keys", "user", "list", "--workspace", &u(1)]),
                "/0/key",
            ),
            (
                strings(&["deployments", "get", &u(fx::DEPLOYMENT)]),
                "/client_auth",
            ),
            (
                strings(&["deployments", "get", &u(fx::DEPLOYMENT)]),
                "/credentials",
            ),
            (
                strings(&["integrations", "get", &u(fx::INTEGRATION)]),
                "/configurations/aws_secret_access_key",
            ),
            (
                strings(&["mcp", "integrations", "get", &u(fx::MCP_INTEGRATION)]),
                "/configurations/custom_headers/x-api-key",
            ),
            (
                strings(&[
                    "mcp",
                    "integrations",
                    "workspaces",
                    "list",
                    &u(fx::MCP_INTEGRATION),
                ]),
                "/configurations/client_secret",
            ),
            (strings(&["organisations", "info"]), "/settings"),
            (
                strings(&["configs", "versions", &u(fx::CONFIG)]),
                "/0/config",
            ),
            (
                strings(&["organisations", "auth-settings", "get"]),
                "/data/scim_token",
            ),
            (strings(&["plugins", "list"]), "/0/credentials"),
            (
                strings(&["providers", "list", "--workspace", &u(1)]),
                "/0/key",
            ),
            (
                strings(&["providers", "get", &u(fx::PROVIDER)]),
                "/model_config",
            ),
        ] {
            let mut json = refs(&args);
            json.extend(["--output", "json"]);
            let out = f.gw(&json);
            out.assert_success();
            let found = out.json().pointer(pointer).cloned().unwrap_or_default();
            if pointer == "/0/config" {
                // Choice 81: the config text withheld inside.
                assert_eq!(
                    found,
                    json!(
                        r#"{"retry":{"attempts":3},"targets":[{"api_key":"[withheld]","provider":"openai"}]}"#
                    )
                );
            } else {
                assert_eq!(found, json!("[withheld]"), "{args:?}");
            }
            for format in ["pretty", "table"] {
                let mut full = refs(&args);
                full.extend(["--output", format]);
                f.gw(&full).assert_success();
            }
        }
        let out = f.gw(&["providers", "get", &u(fx::PROVIDER), "--output", "json"]);
        assert_eq!(out.json()["masked_api_key"], "sk-...abcd");
        assert_eq!(out.json()["key"], "[withheld]");
        let out = f.gw(&[
            "integrations",
            "get",
            &u(fx::INTEGRATION),
            "--output",
            "json",
        ]);
        assert_eq!(
            out.json()["configurations"]["custom_host"],
            "https://llm.example.test/v1?[withheld]",
            "D251"
        );
        assert_eq!(out.json()["masked_key"], "sk-...abcd");
    }

    /// D238 rule 3f (choice 167) and D251 (choices 168, 169): a `config` text that
    /// serde_json cannot read (a lone surrogate escape `JSON.stringify` writes for a string
    /// cut mid-emoji) is `[withheld]` whole and its key registered, so its echo is withheld
    /// too; a URL inside prose and a password holding `/` are withheld, the host printed, in
    /// every format. The runner forbids each marker on every stream.
    #[test]
    fn d238_d251_unreadable_config_texts_and_urls_in_prose_are_withheld() {
        let f = fx();
        let m = fx::marker;
        let config = format!(
            r#"{{"targets":[{{"api_key":"{}","note":"\ud83d"}}]}}"#,
            m(64)
        );
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::ConfigGet).unwrap();
            detail["config"] = json!(config);
            detail["notes"] = json!(format!("rotated from {}", m(64)));
            detail["error"] = json!(format!(
                "connect ECONNREFUSED https://admin:{}@llm.internal/v1",
                m(65)
            ));
            detail["source"] = json!(format!(
                " https://AKIAIOSFODNN7EXAMPLE:wJal/{}@s3.example.test/bucket",
                m(66)
            ));
        });
        let get = ["configs", "get", &u(fx::CONFIG)];
        let out = f.gw(&[&get[..], &["--output", "json"]].concat());
        out.assert_success();
        let doc = out.json();
        assert_eq!(doc["config"], "[withheld]");
        assert_eq!(doc["notes"], "[withheld]", "the key's echo");
        assert_eq!(
            doc["error"],
            "connect ECONNREFUSED https://[withheld]@llm.internal/v1"
        );
        assert_eq!(doc["source"], " https://[withheld]@s3.example.test/bucket");
        assert_eq!(doc["name"], "Default routing");
        for format in ["pretty", "table"] {
            let out = f.gw(&[&get[..], &["--output", format]].concat());
            out.assert_success();
            assert!(out.stdout.contains("llm.internal"), "{format}");
        }
    }

    /// Review B1 (§19.3 rule 3): a config text whose Portkey target holds a stringified
    /// service account (and a credential member inside a JSON text nested in it) prints the
    /// members withheld in every format, `configs get` and `configs versions`; the private
    /// key joins the `SecretSet`, so its echo in another member is withheld too. The runner
    /// forbids each marker on every stream.
    #[test]
    fn config_texts_withhold_service_accounts_at_any_depth() {
        let f = fx();
        let account = json!({
            "client_email": "sa@project.example",
            "private_key": fx::marker(38),
            "type": "service_account",
        })
        .to_string();
        let config = json!({
            "strategy": {"mode": "fallback"},
            "targets": [{
                "override_params": json!({"auth_config": {"token": fx::marker(39)}}).to_string(),
                "provider": "vertex-ai",
                "vertex_region": "us-central1",
                "vertex_service_account_json": account,
            }],
        })
        .to_string();
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::ConfigGet).unwrap();
            detail["config"] = json!(config.clone());
            detail["notes"] = json!(format!("rotated from {}", fx::marker(38)));
            let versions = data.inventory.get_mut(&Gw::ConfigVersions).unwrap();
            versions["data"][0]["config"] = json!(config);
        });
        let expected = r#"{"strategy":{"mode":"fallback"},"targets":[{"override_params":"{\"auth_config\":\"[withheld]\"}","provider":"vertex-ai","vertex_region":"us-central1","vertex_service_account_json":"[withheld]"}]}"#;
        let get = ["configs", "get", &u(fx::CONFIG)];
        let out = f.gw(&[&get[..], &["--output", "json"]].concat());
        out.assert_success();
        assert_eq!(out.json()["config"], json!(expected));
        assert_eq!(out.json()["notes"], "[withheld]", "the private key's echo");
        assert_eq!(out.json()["name"], "Default routing");
        let versions = ["configs", "versions", &u(fx::CONFIG)];
        let out = f.gw(&[&versions[..], &["--output", "json"]].concat());
        out.assert_success();
        assert_eq!(out.json()[0]["config"], json!(expected));
        for args in [&get[..], &versions[..]] {
            for format in ["pretty", "table"] {
                let out = f.gw(&[args, &["--output", format]].concat());
                out.assert_success();
                assert!(out.stdout.contains("[withheld]"), "{args:?} {format}");
                assert!(out.stdout.contains("vertex-ai"), "{args:?} {format}");
            }
        }
    }

    /// D238 (review): a header credential outside the SDK's secret paths (an integration
    /// model's `configurations.custom_headers` as a `{key, value}` pair or a map, a webhook
    /// guardrail's `parameters.headers` on both planes) is `[withheld]` in every format,
    /// with its header name and the safe headers printed; its echo elsewhere is withheld
    /// too. The runner forbids each marker on every stream.
    #[test]
    fn d238_header_credentials_outside_the_sdk_paths_are_withheld() {
        let f = fx();
        let m = fx::marker;
        let webhook = json!([{
            "id": "default.webhook",
            "is_enabled": true,
            "parameters": {
                "webhookURL": "https://hook.example.test/check",
                "headers": {"X-Functions-Key": m(43), "Content-Type": "application/json"},
            },
        }]);
        f.data(|data| {
            data.inventory.insert(
                Gw::IntegrationModels,
                json!({
                    "allow_all_models": false,
                    "models": [
                        {"slug": "m1", "enabled": true, "configurations": {"custom_headers": {"key": "X-Tenant-Auth", "value": m(41)}}},
                        {"slug": "m2", "enabled": true, "configurations": {"custom_headers": {"X-Tenant": m(42), "Accept": "*/*"}}, "note": format!("set {}", m(42))},
                    ],
                    "object": "list",
                    "total": 2,
                }),
            );
            let guardrail = data.inventory.get_mut(&Gw::GuardrailGet).unwrap();
            guardrail["checks"] = webhook.clone();
            let admin = data.inventory.get_mut(&Gw::AdminGuardrailGet).unwrap();
            admin["checks"] = webhook.clone();
            admin["checks"][0]["parameters"]["headers"]["X-Functions-Key"] = json!(m(44));
        });
        let models = ["integrations", "models", "list", &u(fx::INTEGRATION)];
        let out = f.gw(&[&models[..], &["--output", "json"]].concat());
        out.assert_success();
        let doc = out.json();
        let rows = doc.get("models").unwrap_or(&doc);
        assert_eq!(
            rows[0]["configurations"]["custom_headers"],
            json!({"key": "X-Tenant-Auth", "value": "[withheld]"})
        );
        assert_eq!(
            rows[1]["configurations"]["custom_headers"],
            json!({"X-Tenant": "[withheld]", "Accept": "*/*"})
        );
        assert_eq!(rows[1]["note"], "[withheld]", "the header value's echo");
        let guardrail = ["guardrails", "get", &u(fx::GUARDRAIL)];
        let admin = ["admin-guardrails", "get", &u(fx::ADMIN_GUARDRAIL)];
        for args in [&guardrail[..], &admin[..]] {
            let out = f.gw(&[args, &["--output", "json"]].concat());
            out.assert_success();
            assert_eq!(
                out.json()["checks"][0]["parameters"]["headers"],
                json!({"X-Functions-Key": "[withheld]", "Content-Type": "application/json"}),
                "{args:?}"
            );
        }
        for args in [&models[..], &guardrail[..], &admin[..]] {
            for format in ["pretty", "table"] {
                f.gw(&[args, &["--output", format]].concat())
                    .assert_success();
            }
        }
    }

    /// D238 (review, choice 139): a short configuration header (`x-env: prod`,
    /// `anthropic-version` set to the record's date) is withheld in place, by a path rule
    /// (a provider's `configurations.custom_headers.*`) and by a header map (an integration
    /// model's), but not registered: the record's `created_at`, a name containing it and a
    /// member named after it stay printed. A bearer token under a non-credential header is
    /// registered, so its echo is withheld. The runner forbids each marker on every stream.
    #[test]
    fn d238_a_short_configuration_header_does_not_withhold_other_data() {
        let f = fx();
        let headers = json!({
            "anthropic-version": "2026-10-01",
            "x-env": "prod",
            "x-upstream": format!("Bearer {}", fx::marker(47)),
        });
        let shown = json!({
            "anthropic-version": "[withheld]",
            "x-env": "[withheld]",
            "x-upstream": "[withheld]",
        });
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::ProviderGet).unwrap();
            detail["configurations"]["custom_headers"] = headers.clone();
            detail["name"] = json!("prod-gateway");
            detail["prod_region"] = json!("us");
            detail["note"] = json!(format!("via {}", fx::marker(47)));
            data.inventory.insert(
                Gw::IntegrationModels,
                json!({
                    "allow_all_models": false,
                    "models": [
                        {"slug": "prod-model", "enabled": true, "configurations": {"custom_headers": headers.clone()}, "created_at": fx::AT},
                    ],
                    "object": "list",
                    "total": 1,
                }),
            );
        });
        let out = f.gw(&["providers", "get", &u(fx::PROVIDER), "--output", "json"]);
        out.assert_success();
        let doc = out.json();
        assert_eq!(doc["configurations"]["custom_headers"], shown);
        assert_eq!(doc["created_at"], fx::AT);
        assert_eq!(doc["name"], "prod-gateway");
        assert_eq!(doc["prod_region"], "us");
        assert_eq!(doc["note"], "[withheld]", "the bearer token's echo");
        let models = ["integrations", "models", "list", &u(fx::INTEGRATION)];
        let out = f.gw(&[&models[..], &["--output", "json"]].concat());
        out.assert_success();
        let doc = out.json();
        let rows = doc.get("models").unwrap_or(&doc);
        assert_eq!(rows[0]["configurations"]["custom_headers"], shown);
        assert_eq!(rows[0]["slug"], "prod-model");
        assert_eq!(rows[0]["created_at"], fx::AT);
        for format in ["pretty", "table"] {
            f.gw(&[&models[..], &["--output", format]].concat())
                .assert_success();
        }
    }

    /// D239 (review): an audit record's transport headers are withheld but not registered,
    /// so a record's `client_ip` equal to its `x-forwarded-for`, its `workspace_id` equal
    /// to `x-portkey-workspace` and the same `client_ip` in a record without headers stay
    /// printed, with and without `--reveal-sensitive`.
    #[test]
    fn d239_audit_header_values_do_not_withhold_the_record() {
        let f = fx();
        f.data(|data| {
            let logs = data.inventory.get_mut(&Gw::AuditLogs).unwrap();
            logs["records"][0]["request_headers"] = json!(
                json!({
                    "x-forwarded-for": "203.0.113.9",
                    "x-portkey-workspace": "ws-main-a",
                    "x-forwarded-proto": "https",
                    "authorization": format!("Bearer {}", fx::marker(13)),
                })
                .to_string()
            );
            logs["records"][0]["client_ip"] = json!("203.0.113.9");
            logs["records"][0]["workspace_id"] = json!("ws-main-a");
            logs["records"][0]["uri"] = json!("/v2/workspaces/ws-main-a");
            logs["records"][1]["client_ip"] = json!("203.0.113.9");
        });
        let mut args = vec![
            "audit-logs",
            "list",
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
            "--output",
            "json",
        ];
        for reveal in [false, true] {
            if reveal {
                args.push("--reveal-sensitive");
            }
            let out = f.gw(&args);
            out.assert_success();
            let rows = out.json();
            assert_eq!(rows[0]["client_ip"], "203.0.113.9", "reveal {reveal}");
            assert_eq!(rows[0]["workspace_id"], "ws-main-a", "reveal {reveal}");
            assert_eq!(
                rows[0]["uri"], "/v2/workspaces/ws-main-a",
                "reveal {reveal}"
            );
            assert_eq!(rows[1]["client_ip"], "203.0.113.9", "reveal {reveal}");
            if reveal {
                let headers: serde_json::Value =
                    serde_json::from_str(rows[0]["request_headers"].as_str().unwrap()).unwrap();
                assert_eq!(
                    headers,
                    json!({"x-forwarded-for": "[withheld]", "x-portkey-workspace": "[withheld]", "x-forwarded-proto": "[withheld]", "authorization": "[withheld]"})
                );
            }
        }
    }

    /// Choice 141 (review): a generic name's short value in a logged request body
    /// (`{"key": "prod"}`) is withheld but not registered, so another record's `prod-…` uri
    /// stays printed, revealed or not; a credential in the same body is still withheld where
    /// another record echoes it.
    #[test]
    fn d239_a_generic_name_in_an_audit_body_does_not_withhold_the_log() {
        let f = fx();
        f.data(|data| {
            let logs = data.inventory.get_mut(&Gw::AuditLogs).unwrap();
            logs["records"][0]["request_body"] =
                json!(json!({"key": "prod", "provider": {"api_key": fx::marker(12)}}).to_string());
            logs["records"][1]["uri"] = json!("/v2/workspaces/prod-gateway");
            logs["records"][1]["request_id"] = json!(format!("echo {}", fx::marker(12)));
        });
        let mut args = vec![
            "audit-logs",
            "list",
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
            "--output",
            "json",
        ];
        for reveal in [false, true] {
            if reveal {
                args.push("--reveal-sensitive");
            }
            let out = f.gw(&args);
            out.assert_success();
            let rows = out.json();
            assert_eq!(
                rows[1]["uri"], "/v2/workspaces/prod-gateway",
                "reveal {reveal}"
            );
            assert_eq!(rows[1]["request_id"], "[withheld]", "reveal {reveal}");
            assert!(!out.stdout.contains(&fx::marker(12)), "{}", out.stdout);
            if reveal {
                assert_eq!(
                    rows[0]["request_body"],
                    r#"{"key":"[withheld]","provider":{"api_key":"[withheld]"}}"#
                );
            }
        }
    }

    /// Choices 150 and 152 (review, HIGH): a credential named inside a schema object, under any
    /// property of a `properties` map, is withheld in every format (the runner forbids each
    /// marker), in the guardrail catalog and in a config's JSON text, and its echo too; a
    /// describing text that shows a credential under a credential-named property is withheld
    /// in place, the rest of the schema printed.
    #[test]
    fn d238_credentials_named_inside_schemas_are_withheld() {
        let f = fx();
        f.data(|data| {
            let catalog = data.inventory.get_mut(&Gw::GuardrailCatalog).unwrap();
            let properties = &mut catalog["evals"][1]["parameters"]["properties"];
            properties["db"] = json!({"type": "object", "password": fx::marker(50)});
            properties["openai"] = json!({"type": "string", "api_key": fx::marker(51)});
            properties["client_secret"]["title"] = json!(format!("Bearer {}", fx::marker(53)));
            catalog["evals"][1]["name"] = json!(format!("Webhook {}", fx::marker(50)));
            let detail = data.inventory.get_mut(&Gw::ConfigGet).unwrap();
            detail["config"] = json!(format!(
                r#"{{"properties":{{"t":{{"type":"string","secret":"{}"}}}}}}"#,
                fx::marker(52)
            ));
        });
        for format in ["json", "table", "pretty"] {
            f.gw(&["guardrails", "catalog", "--output", format])
                .assert_success();
            f.gw(&["configs", "get", &u(fx::CONFIG), "--output", format])
                .assert_success();
        }
        let doc = f.gw(&["guardrails", "catalog", "--output", "json"]).json();
        let properties = &doc["evals"][1]["parameters"]["properties"];
        assert_eq!(
            properties["db"],
            json!({"type": "object", "password": "[withheld]"})
        );
        assert_eq!(
            properties["openai"],
            json!({"type": "string", "api_key": "[withheld]"})
        );
        // The word after `Bearer` is withheld in place by the scheme rule (choice 177).
        assert_eq!(properties["client_secret"]["title"], "Bearer [withheld]");
        assert_eq!(properties["client_secret"]["label"], "Client secret");
        assert_eq!(doc["evals"][1]["name"], "[withheld]");
        assert_eq!(doc["object"], "list");
        let doc = f
            .gw(&["configs", "get", &u(fx::CONFIG), "--output", "json"])
            .json();
        assert_eq!(
            doc["config"],
            r#"{"properties":{"t":{"type":"string","secret":"[withheld]"}}}"#
        );
    }

    /// Choices 151 and 153 (review): inside a withheld credential object a short password beside a
    /// user name is registered, so its echo in another member is withheld in every format (a
    /// deployment's `client_auth` JSON text, a plugin's `credentials` map); a word-shaped value
    /// there (the plugin's `region`, `us-east-1`) is withheld in place but not registered, so
    /// the `integration_slug` carrying it stays printed.
    #[test]
    fn d238_credential_objects_register_passwords_not_words() {
        let f = fx();
        let rows = f.gw(&["plugins", "list", "--output", "json"]).json();
        assert_eq!(rows[0]["integration_slug"], "openai-us-east-1");
        assert_eq!(rows[0]["credentials"], "[withheld]");
        let table = f.gw(&["plugins", "list", "--output", "table"]);
        table.assert_success();
        assert!(
            table.stdout.contains(" openai-us-east-1 "),
            "{}",
            table.stdout
        );
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::DeploymentGet).unwrap();
            // `client_auth` is a string: a JSON text holding the login.
            detail["client_auth"] = json!(r#"{"user":"u","pwd":"hunter2x"}"#);
            detail["note"] = json!("login hunter2x");
            let plugins = data.inventory.get_mut(&Gw::Plugins).unwrap();
            // A map of strings (the schema's): each member by its own name (`credentials.*`).
            plugins["data"][0]["credentials"] =
                json!({"user": "n", "pass": "p4ssw0rd", "region": "us-east-1"});
            plugins["data"][0]["note"] = json!("p4ssw0rd");
        });
        for format in ["json", "table", "pretty"] {
            let out = f.gw(&["deployments", "get", &u(fx::DEPLOYMENT), "--output", format]);
            out.assert_success();
            assert!(!out.stdout.contains("hunter2x"), "{format}: {}", out.stdout);
            let out = f.gw(&["plugins", "list", "--output", format]);
            out.assert_success();
            assert!(!out.stdout.contains("p4ssw0rd"), "{format}: {}", out.stdout);
        }
        let doc = f
            .gw(&["deployments", "get", &u(fx::DEPLOYMENT), "--output", "json"])
            .json();
        assert_eq!(doc["client_auth"], "[withheld]");
        assert_eq!(doc["note"], "[withheld]");
        let rows = f.gw(&["plugins", "list", "--output", "json"]).json();
        assert_eq!(rows[0]["note"], "[withheld]");
        assert_eq!(rows[0]["integration_slug"], "openai-us-east-1");
    }

    /// Choices 146 and 147 (live): the guardrail catalog, a vendor schema catalog whose
    /// parameter schemas sit under credential names (`credentials.apiKey`, `token`,
    /// `client_secret`, `headers`), prints whole in every format: no schema word
    /// (`string`, `object`, `subHeading`) is withheld or registered, and the top-level
    /// `object` member stays. A `default` value under a credential-named property is the one
    /// credential there: withheld, and its echo elsewhere too.
    #[test]
    fn d238_the_guardrail_catalog_withholds_nothing_ordinary() {
        let f = fx();
        let catalog = fx::guardrail_catalog();
        let out = f.gw(&["guardrails", "catalog", "--output", "json"]);
        out.assert_success();
        assert_eq!(out.json(), catalog);
        assert!(!out.stdout.contains("[withheld]"), "{}", out.stdout);
        for format in ["table", "pretty"] {
            let out = f.gw(&["guardrails", "catalog", "--output", format]);
            out.assert_success();
            assert!(
                !out.stdout.contains("[withheld]"),
                "{format}: {}",
                out.stdout
            );
            // The SDK model's member order (TS zod's too): `object`, then `evals`.
            let rows: Vec<Vec<&str>> = out
                .stdout
                .lines()
                .filter(|line| line.contains('│'))
                .map(|line| line.split('│').map(str::trim).collect())
                .collect();
            let fields: Vec<&str> = rows.iter().map(|row| row[0]).collect();
            assert_eq!(fields, ["Field", "object", "evals"], "{format}");
            assert_eq!(rows[1][1], "list", "{format}");
            let evals: serde_json::Value = serde_json::from_str(rows[2][1]).unwrap();
            assert_eq!(evals, catalog["evals"], "{format}");
        }
        // One real credential: a `default` under the credential-named `apiKey`.
        f.data(|data| {
            let catalog = data.inventory.get_mut(&Gw::GuardrailCatalog).unwrap();
            catalog["evals"][0]["parameters"]["properties"]["credentials"]["properties"]["apiKey"]
                ["default"] = json!(fx::marker(48));
            catalog["evals"][1]["name"] = json!(format!("Webhook {}", fx::marker(48)));
        });
        let out = f.gw(&["guardrails", "catalog", "--output", "json"]);
        out.assert_success();
        let doc = out.json();
        let mut expected = catalog.clone();
        expected["evals"][0]["parameters"]["properties"]["credentials"]["properties"]["apiKey"]["default"] =
            json!("[withheld]");
        expected["evals"][1]["name"] = json!("[withheld]");
        assert_eq!(doc, expected);
        assert_eq!(doc["object"], "list");
    }

    /// Choice 188 (review): the required-metadata guardrail's `metadataKeys` (a list of
    /// metadata key names the `_keys` suffix read as credentials) prints on `guardrails get`
    /// and `admin-guardrails get` in every format, and in the catalog with its empty `default`
    /// and its `examples`; the credential-named `client_secret`'s empty `default` prints there
    /// too, as TypeScript prints both.
    #[test]
    fn d238_guardrail_metadata_keys_and_empty_schema_defaults_print() {
        let f = fx();
        for args in [
            ["guardrails", "get", &u(fx::GUARDRAIL)],
            ["admin-guardrails", "get", &u(fx::ADMIN_GUARDRAIL)],
        ] {
            let out = f.gw(&[args[0], args[1], args[2], "--output", "json"]);
            out.assert_success();
            assert_eq!(out.json()["checks"], fx::guardrail_checks(), "{args:?}");
            assert_eq!(
                out.json()["checks"][1]["parameters"]["metadataKeys"],
                json!(["user_id", "team"])
            );
            for format in ["json", "pretty", "table"] {
                let out = f.gw(&[args[0], args[1], args[2], "--output", format]);
                out.assert_success();
                assert!(
                    !out.stdout.contains("[withheld]"),
                    "{args:?} {format}: {}",
                    out.stdout
                );
                assert!(out.stdout.contains("user_id"), "{args:?} {format}");
            }
        }
        let doc = f.gw(&["guardrails", "catalog", "--output", "json"]).json();
        let metadata = &doc["evals"][3]["parameters"]["properties"]["metadataKeys"];
        assert_eq!(metadata["default"], json!([]));
        assert_eq!(metadata["examples"], json!([["user_id", "team"]]));
        assert_eq!(
            doc["evals"][1]["parameters"]["properties"]["client_secret"]["default"],
            ""
        );
        assert!(!doc.to_string().contains("[withheld]"), "{doc}");
    }

    /// A withheld value echoed in another member, a member named after one, and a URL with
    /// userinfo are withheld too (the `SecretSet` rule).
    #[test]
    fn echoed_credentials_are_withheld() {
        let f = fx();
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::ProviderGet).unwrap();
            detail["note"] = json!(format!("rotated from {}", fx::marker(24)));
            detail["custom_link"] = json!(format!("https://user:{}@h.example/p", fx::marker(30)));
            detail[fx::marker(24)] = json!("x");
        });
        let out = f.gw(&["providers", "get", &u(fx::PROVIDER), "--output", "json"]);
        out.assert_success();
        let doc = out.json();
        assert_eq!(doc["note"], "[withheld]");
        assert_eq!(doc["custom_link"], "https://[withheld]@h.example/p");
        assert!(doc.get(fx::marker(24)).is_none());
    }

    /// D239: the audit request fields are withheld; `--reveal-sensitive` shows their JSON
    /// with credentials withheld inside, keeps plain text withheld, and a credential from
    /// a withheld body echoed elsewhere is withheld too (choice 80).
    #[test]
    fn audit_request_fields() {
        let f = fx();
        let window = [
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
        ];
        let mut args = vec!["audit-logs", "list", "--output", "json"];
        args.extend(window);
        let rows = f.gw(&args).json();
        for field in ["request_body", "request_headers", "query_params"] {
            assert_eq!(rows[0][field], "[withheld]");
            assert_eq!(rows[1][field], "[withheld]");
        }
        args.push("--reveal-sensitive");
        let rows = f.gw(&args).json();
        assert_eq!(
            rows[0]["request_body"],
            r#"{"key":"[withheld]","name":"ci-bot"}"#
        );
        assert_eq!(rows[1]["request_body"], "[withheld]");
        assert_eq!(rows[1]["query_params"], "[withheld]");
        assert_eq!(rows[1]["request_headers"], "{}");
        f.data(|data| {
            let logs = data.inventory.get_mut(&Gw::AuditLogs).unwrap();
            logs["records"][1]["uri"] = json!(format!("/v2/keys/{}", fx::marker(12)));
        });
        args.pop();
        let rows = f.gw(&args).json();
        assert_eq!(rows[1]["uri"], "[withheld]");
        let table = f.gw(&["audit-logs", "ls", "--output", "table", "--days", "1"]);
        table.assert_success();
        assert!(table.stdout.contains(" Request Body "), "{}", table.stdout);
    }

    /// D239: `--reveal-sensitive` never shows a credential: the SDK's request secret paths
    /// of every gateway resource, credentials held as JSON text, the request credential
    /// members and every header value but the safe headers stay `[withheld]` in a revealed
    /// field, in every format (the harness forbids each marker on every stream).
    #[test]
    fn d239_revealed_fields_never_show_credentials() {
        let f = fx();
        let m = fx::marker;
        let body = json!({
            "configurations": {
                "vertex_service_account_json": json!({"private_key": m(31)}).to_string(),
                "custom_headers": {"x-auth-review": m(32)},
                "aws_secret_key": m(33),
                "region": "us-east-1",
            },
            "model_config": {"secret_key": m(34)},
            "name": "review",
        });
        let headers = json!({
            "ocp-apim-subscription-key": m(35),
            "x-custom-auth": m(36),
            "accept": "*/*",
        });
        f.data(|data| {
            let logs = data.inventory.get_mut(&Gw::AuditLogs).unwrap();
            logs["records"][0]["request_body"] = json!(body.to_string());
            logs["records"][0]["request_headers"] = json!(headers.to_string());
        });
        let mut args = vec![
            "audit-logs",
            "list",
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
            "--reveal-sensitive",
            "--output",
            "json",
        ];
        let out = f.gw(&args);
        out.assert_success();
        let rows = out.json();
        let revealed: serde_json::Value =
            serde_json::from_str(rows[0]["request_body"].as_str().unwrap()).unwrap();
        assert_eq!(
            revealed,
            json!({
                "configurations": {
                    "vertex_service_account_json": "[withheld]",
                    "custom_headers": {"x-auth-review": "[withheld]"},
                    "aws_secret_key": "[withheld]",
                    "region": "us-east-1",
                },
                "model_config": "[withheld]",
                "name": "review",
            })
        );
        let revealed: serde_json::Value =
            serde_json::from_str(rows[0]["request_headers"].as_str().unwrap()).unwrap();
        assert_eq!(
            revealed,
            json!({"ocp-apim-subscription-key": "[withheld]", "x-custom-auth": "[withheld]", "accept": "*/*"})
        );
        for format in ["pretty", "table"] {
            *args.last_mut().unwrap() = format;
            f.gw(&args).assert_success();
        }
    }

    /// Choices 170 and 171 (review): through `deployments get` in every format, a
    /// `{name, value}` environment list and header list withhold the credential's value and
    /// print an ordinary variable, a private key under a neutral name is withheld, a number
    /// inside a credential object is withheld with the object, a number under a credential
    /// name is withheld too and a count prints (choice 187); the runner's leak check fails any
    /// run that shows a marker.
    #[test]
    fn d238_name_value_pairs_private_keys_and_numbers_are_withheld() {
        let f = fx();
        let m = fx::marker;
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
            m(69)
        );
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::DeploymentGet).unwrap();
            detail["deployment_config"] = json!({
                "env": [
                    {"name": "OPENAI_API_KEY", "value": m(67)},
                    {"name": "LOG_LEVEL", "value": "info"}
                ],
                "forward_headers": [{"name": "Authorization", "value": format!("Bearer {}", m(68))}],
                "cert": pem,
                "credentials": {"pin": 12_345_678},
                "password": 4242,
                "usage": {"prompt_tokens": 3}
            });
        });
        let doc = f
            .gw(&["deployments", "get", &u(fx::DEPLOYMENT), "--output", "json"])
            .json();
        let config = &doc["deployment_config"];
        assert_eq!(
            config["env"][0],
            json!({"name": "OPENAI_API_KEY", "value": "[withheld]"})
        );
        assert_eq!(
            config["env"][1],
            json!({"name": "LOG_LEVEL", "value": "info"})
        );
        assert_eq!(config["forward_headers"][0]["value"], "[withheld]");
        assert_eq!(config["cert"], "[withheld]");
        assert_eq!(config["credentials"], "[withheld]");
        assert_eq!(config["password"], "[withheld]", "choice 187");
        assert_eq!(config["usage"]["prompt_tokens"], 3);
        for format in ["table", "pretty"] {
            let out = f.gw(&["deployments", "get", &u(fx::DEPLOYMENT), "--output", format]);
            out.assert_success();
            assert!(out.stdout.contains("LOG_LEVEL"), "{format}: {}", out.stdout);
            assert!(!out.stdout.contains("12345678"), "{format}: {}", out.stdout);
        }
    }

    /// Choices 176 and 177 (review): `pass` and `pat` as words, `_keys`, a run-together
    /// `token` name, a credential after `Bearer` or `Basic` under a neutral name or in a
    /// command line, a `_headers` map and a `{header, value}` pair are withheld in a
    /// `deployments get` detail in every format (the harness forbids each marker on every
    /// stream), and the names around them stay printed. Choice 181: `auth_header` by its
    /// name, `Authorization:Bearer` with no space, a connection string's password; a model
    /// id after the prose word `token` is withheld in that text only, so `ai_model` prints.
    #[test]
    fn d238_pass_keys_scheme_and_header_shapes_are_withheld() {
        let f = fx();
        let m = fx::marker;
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::DeploymentGet).unwrap();
            detail["deployment_config"] = json!({
                "env": [
                    {"name": "DB_PASS", "value": m(70)},
                    {"name": "GITHUB_PAT", "value": m(71)},
                    {"name": "LOG_LEVEL", "value": "info"}
                ],
                "smtp_pass": m(72),
                "access_keys": [m(73)],
                "tokenvalue": m(74),
                "authentication": format!("Bearer {}", m(75)),
                "auth_header": m(79),
                "probe": format!("curl -H 'Authorization: Bearer {}' https://h.example.test/v1", m(76)),
                "probe_joined": format!("curl -H Authorization:Bearer {} https://h.example.test/v1", m(80)),
                "connection": format!("Server=db;User Id=sa;Password={};Encrypt=True", m(81)),
                "error": "token gpt-4o-mini-2024-07-18 quota exceeded",
                "ai_model": "gpt-4o-mini-2024-07-18",
                "extra_headers": {"X-Signature": m(77), "accept": "*/*"},
                "rules": [{"header": "x-api-key", "value": m(78)}],
                "allowed_headers": ["x-trace", "Authorization"],
                "passthrough": "on",
                "tokenizer": "cl100k"
            });
        });
        let doc = f
            .gw(&["deployments", "get", &u(fx::DEPLOYMENT), "--output", "json"])
            .json();
        let config = &doc["deployment_config"];
        assert_eq!(
            config["env"],
            json!([
                {"name": "DB_PASS", "value": "[withheld]"},
                {"name": "GITHUB_PAT", "value": "[withheld]"},
                {"name": "LOG_LEVEL", "value": "info"}
            ])
        );
        for name in ["smtp_pass", "access_keys", "tokenvalue"] {
            assert_eq!(config[name], "[withheld]", "{name}");
        }
        assert_eq!(config["authentication"], "Bearer [withheld]");
        assert_eq!(config["auth_header"], "[withheld]");
        assert_eq!(
            config["probe"],
            "curl -H 'Authorization: Bearer [withheld]' https://h.example.test/v1"
        );
        // Choice 181: a scheme joined to its header name, a connection string's password, and
        // a model id after the prose word `token` withheld in that text only.
        assert_eq!(
            config["probe_joined"],
            "curl -H Authorization:Bearer [withheld] https://h.example.test/v1"
        );
        assert_eq!(
            config["connection"],
            "Server=db;User Id=sa;Password=[withheld];Encrypt=True"
        );
        assert_eq!(config["error"], "token [withheld] quota exceeded");
        assert_eq!(config["ai_model"], "gpt-4o-mini-2024-07-18");
        assert_eq!(
            config["extra_headers"],
            json!({"X-Signature": "[withheld]", "accept": "*/*"})
        );
        assert_eq!(config["rules"][0]["value"], "[withheld]");
        assert_eq!(
            config["allowed_headers"],
            json!(["x-trace", "Authorization"])
        );
        assert_eq!(config["passthrough"], "on");
        assert_eq!(config["tokenizer"], "cl100k");
        for format in ["table", "pretty"] {
            let out = f.gw(&["deployments", "get", &u(fx::DEPLOYMENT), "--output", format]);
            out.assert_success();
            assert!(out.stdout.contains("LOG_LEVEL"), "{format}: {}", out.stdout);
            assert!(out.stdout.contains("cl100k"), "{format}: {}", out.stdout);
            assert!(
                out.stdout.contains("gpt-4o-mini-2024-07-18"),
                "{format}: {}",
                out.stdout
            );
        }
    }

    /// Choice 157 (review): a credential under a camelCase name or under a name whose
    /// credential word is not a suffix (`clientSecret`, `awsSessionToken`, `bearerToken`,
    /// `adminpassword`, `secret_value`, the names TypeScript's `redactDeep` masks) is
    /// withheld in `providers get` and in a revealed audit body, in every format, and its
    /// echo elsewhere too (the harness forbids each marker on every stream).
    #[test]
    fn d238_camel_case_and_worded_credential_names_are_withheld() {
        let f = fx();
        let m = fx::marker;
        let names = [
            "clientSecret",
            "awsSessionToken",
            "bearerToken",
            "adminpassword",
            "secret_value",
        ];
        let configurations: serde_json::Map<String, serde_json::Value> = names
            .iter()
            .zip(54..)
            .map(|(name, n)| ((*name).to_owned(), json!(m(n))))
            .chain([("region".to_owned(), json!("us-east-1"))])
            .collect();
        let echo = (54..59).map(m).collect::<Vec<_>>().join(" ");
        let body = json!({"configurations": configurations.clone(), "name": "review"});
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::ProviderGet).unwrap();
            detail["configurations"] = json!(configurations);
            detail["note"] = json!(echo);
            let logs = data.inventory.get_mut(&Gw::AuditLogs).unwrap();
            logs["records"][0]["request_body"] = json!(body.to_string());
        });
        let doc = f
            .gw(&["providers", "get", &u(fx::PROVIDER), "--output", "json"])
            .json();
        for name in names {
            assert_eq!(doc["configurations"][name], "[withheld]", "{name}");
        }
        assert_eq!(doc["configurations"]["region"], "us-east-1");
        assert_eq!(doc["note"], "[withheld]");
        for format in ["table", "pretty"] {
            let out = f.gw(&["providers", "get", &u(fx::PROVIDER), "--output", format]);
            out.assert_success();
            assert!(
                out.stdout.contains("[withheld]"),
                "{format}: {}",
                out.stdout
            );
        }
        let mut args = vec![
            "audit-logs",
            "list",
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
            "--reveal-sensitive",
            "--output",
            "json",
        ];
        let rows = f.gw(&args).json();
        let revealed: serde_json::Value =
            serde_json::from_str(rows[0]["request_body"].as_str().unwrap()).unwrap();
        for name in names {
            assert_eq!(revealed["configurations"][name], "[withheld]", "{name}");
        }
        assert_eq!(revealed["configurations"]["region"], "us-east-1");
        assert_eq!(revealed["name"], "review");
        for format in ["pretty", "table"] {
            *args.last_mut().unwrap() = format;
            f.gw(&args).assert_success();
        }
    }

    /// D238 (choice 163, review): a credential under a plural or camelCase-plural token name
    /// (`access_tokens`, `refreshTokens`, `apiTokens`, `tokens`) is withheld in a
    /// `providers get` detail and in a revealed audit body, and its echo too; a token count
    /// stays a number (the harness forbids each marker on every stream).
    #[test]
    fn d238_plural_token_names_are_withheld() {
        let f = fx();
        let m = fx::marker;
        let configurations = json!({
            "access_tokens": [m(60)],
            "apiTokens": {"ci": m(61)},
            "refreshTokens": m(62),
            "tokens": m(63),
            "region": "us-east-1",
        });
        let body = json!({"configurations": configurations.clone(), "usage": {"prompt_tokens": 3}});
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::ProviderGet).unwrap();
            detail["configurations"] = configurations.clone();
            detail["note"] = json!(format!("from {} and {}", m(60), m(62)));
            let logs = data.inventory.get_mut(&Gw::AuditLogs).unwrap();
            logs["records"][0]["request_body"] = json!(body.to_string());
        });
        let doc = f
            .gw(&["providers", "get", &u(fx::PROVIDER), "--output", "json"])
            .json();
        assert_eq!(
            doc["configurations"],
            json!({"access_tokens": "[withheld]", "apiTokens": "[withheld]", "refreshTokens": "[withheld]", "tokens": "[withheld]", "region": "us-east-1"})
        );
        assert_eq!(doc["note"], "[withheld]");
        for format in ["table", "pretty"] {
            f.gw(&["providers", "get", &u(fx::PROVIDER), "--output", format])
                .assert_success();
        }
        let rows = f
            .gw(&[
                "audit-logs",
                "list",
                "--start",
                "2026-10-01T00:00:00Z",
                "--end",
                "2026-10-02T00:00:00Z",
                "--reveal-sensitive",
                "--output",
                "json",
            ])
            .json();
        let revealed: serde_json::Value =
            serde_json::from_str(rows[0]["request_body"].as_str().unwrap()).unwrap();
        assert_eq!(
            revealed,
            json!({"configurations": {"access_tokens": "[withheld]", "apiTokens": "[withheld]", "refreshTokens": "[withheld]", "tokens": "[withheld]", "region": "us-east-1"}, "usage": {"prompt_tokens": 3}})
        );
    }

    /// D238 (choice 164, review): a provider's secret mapping prints whole; its `secret_key`
    /// names the key inside the referenced secret and is never registered, so the mapping's
    /// `target_field`, a `note` naming the same key and the `masked_api_key` fingerprint
    /// survive in every format.
    #[test]
    fn d238_a_secret_mapping_prints_and_withholds_nothing_ordinary() {
        let f = fx();
        f.data(|data| {
            let detail = data.inventory.get_mut(&Gw::ProviderGet).unwrap();
            detail["note"] = json!("rotate the api_key monthly");
        });
        let doc = f
            .gw(&["providers", "get", &u(fx::PROVIDER), "--output", "json"])
            .json();
        assert_eq!(
            doc["secret_mappings"],
            fx::provider_detail()["secret_mappings"]
        );
        assert_eq!(doc["secret_mappings"][0]["secret_key"], "api_key");
        assert_eq!(doc["secret_mappings"][0]["target_field"], "api_key");
        assert_eq!(doc["note"], "rotate the api_key monthly");
        assert_eq!(doc["masked_api_key"], "sk-...abcd");
        assert_eq!(doc["key"], "[withheld]");
        for format in ["table", "pretty"] {
            let out = f.gw(&["providers", "get", &u(fx::PROVIDER), "--output", format]);
            out.assert_success();
            for shown in [
                "rotate the api_key monthly",
                "sk-...abcd",
                r#""secret_key":"api_key""#,
            ] {
                assert!(
                    out.stdout.contains(shown),
                    "{format}: {shown}: {}",
                    out.stdout
                );
            }
        }
    }

    /// D238 (choice 165, review): a model's prices sit under `token`-named members
    /// (`pay_as_you_go.request_token`) that hold only numbers, so `integrations models list`
    /// prints them as TypeScript does, in every format.
    #[test]
    fn d238_model_prices_are_printed() {
        let f = fx();
        let args = ["integrations", "models", "list", &u(fx::INTEGRATION)];
        let mut json_args = args.to_vec();
        json_args.extend(["--output", "json"]);
        let doc = f.gw(&json_args).json();
        assert_eq!(doc, fx::integration_models()["models"]);
        assert_eq!(
            doc[0]["pricing_config"]["pay_as_you_go"]["request_token"]["price"],
            0.5
        );
        for format in ["table", "pretty"] {
            let mut format_args = args.to_vec();
            format_args.extend(["--output", format]);
            let out = f.gw(&format_args);
            out.assert_success();
            assert!(
                !out.stdout.contains("[withheld]"),
                "{format}: {}",
                out.stdout
            );
        }
    }

    /// D239 and the SDK's `secretReferences.*` rules: an audited secret-reference create
    /// (HashiCorp AppRole, AWS access key) never shows its `auth_config`, with or without
    /// `--reveal-sensitive`, in every format; a `vault_secret_id` outside it is withheld by
    /// the name rule (the harness forbids each marker on every stream).
    #[test]
    fn audited_secret_reference_bodies_never_show_the_auth_config() {
        let f = fx();
        let m = fx::marker;
        let approle = json!({
            "name": "vault-ref",
            "manager_type": "hashicorp_vault",
            "auth_config": {
                "vault_auth_type": "approle",
                "vault_addr": "https://vault.example",
                "vault_role_id": "role-1",
                "vault_secret_id": m(37),
            },
            "secret_path": "kv/app",
            "rotation": {"vault_secret_id": m(39)},
        });
        let aws = json!({
            "name": "aws-ref",
            "manager_type": "aws_sm",
            "auth_config": {
                "aws_auth_type": "accessKey",
                "aws_access_key_id": "AKIDEXAMPLE",
                "aws_secret_access_key": m(38),
                "aws_region": "us-east-1",
            },
            "secret_path": "prod/app",
        });
        f.data(|data| {
            let logs = data.inventory.get_mut(&Gw::AuditLogs).unwrap();
            for (n, body) in [(0, &approle), (1, &aws)] {
                logs["records"][n]["request_body"] = json!(body.to_string());
                logs["records"][n]["resource_type"] = json!("secret-reference");
                logs["records"][n]["uri"] = json!("/v2/secret-references");
            }
        });
        let mut args = vec![
            "audit-logs",
            "list",
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
            "--output",
            "json",
        ];
        let rows = f.gw(&args).json();
        assert_eq!(rows[0]["request_body"], "[withheld]");
        assert_eq!(rows[1]["request_body"], "[withheld]");
        args.push("--reveal-sensitive");
        let out = f.gw(&args);
        out.assert_success();
        let rows = out.json();
        let revealed = |n: usize| -> serde_json::Value {
            serde_json::from_str(rows[n]["request_body"].as_str().unwrap()).unwrap()
        };
        assert_eq!(
            revealed(0),
            json!({
                "name": "vault-ref",
                "manager_type": "hashicorp_vault",
                "auth_config": "[withheld]",
                "secret_path": "kv/app",
                "rotation": {"vault_secret_id": "[withheld]"},
            })
        );
        assert_eq!(
            revealed(1),
            json!({
                "name": "aws-ref",
                "manager_type": "aws_sm",
                "auth_config": "[withheld]",
                "secret_path": "prod/app",
            })
        );
        for format in ["pretty", "table"] {
            let mut shown = args.clone();
            let at = shown.len() - 2;
            shown[at] = format;
            let out = f.gw(&shown);
            out.assert_success();
            assert!(out.stdout.contains("vault-ref"), "{format}: {}", out.stdout);
        }
    }
}

// ---------------------------------------------------------------------------
// Argument checks (§19.3, D232, D245)
// ---------------------------------------------------------------------------

mod checks {
    use super::*;

    /// Every check is a usage error before the tenant: exit 2, nothing sent, no debug log.
    #[test]
    fn argument_checks_send_nothing() {
        let f = fx();
        for (args, message) in [
            (
                &["configs", "get", "config-1"][..],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid configId: config-1",
            ),
            (
                &["configs", "versions", "x"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid configId: x",
            ),
            (
                &["configs", "list", "--workspace", "workspace-1"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid workspaceId: workspace-1",
            ),
            (
                &["api-keys", "user", "get", "key-1"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid keyId: key-1",
            ),
            (
                &["guardrails", "get", "g"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid guardrailId: g",
            ),
            (
                &["providers", "get", "p"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid providerId: p",
            ),
            (
                &["integrations", "workspaces", "list", "i"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid integrationId: i",
            ),
            (
                &["mcp", "integrations", "capabilities", "list", "m"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid mcpIntegrationId: m",
            ),
            (
                &["deployments", "get", "d"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid deploymentId: d",
            ),
            (
                &["admin-guardrails", "get", "a"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid guardrailId: a",
            ),
            (
                &["organisations", "auth-settings", "get", "--tsg-id", "1 2"],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid tsgId: 1 2",
            ),
            (
                &[
                    "api-keys",
                    "service",
                    "get",
                    &fx::uuid(1),
                    "--reveal-sensitive",
                ],
                REVEAL_REFUSED,
            ),
            (
                &[
                    "api-keys",
                    "user",
                    "list",
                    "--workspace",
                    "x",
                    "--reveal-sensitive",
                ],
                REVEAL_REFUSED,
            ),
            (
                &["providers", "get", "p", "--reveal-sensitive"],
                REVEAL_REFUSED,
            ),
            (
                &[
                    "organisations",
                    "auth-settings",
                    "get",
                    "--reveal-sensitive",
                ],
                REVEAL_REFUSED,
            ),
            (
                &["audit-logs", "list", "--days", "0"],
                "Invalid --days: Expected a positive integer",
            ),
            (
                &["audit-logs", "list", "--start", "2026-10-01"],
                "Invalid --start: Expected an ISO-8601 timestamp",
            ),
            (
                &[
                    "audit-logs",
                    "list",
                    "--start",
                    "2026-10-02T00:00:00Z",
                    "--end",
                    "2026-10-01T00:00:00Z",
                ],
                "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid telemetry query options or time window",
            ),
            (
                &["admin-guardrails", "list", "--page-size", "0"],
                "--page-size must be an integer from 1 to 1000",
            ),
            (
                &[
                    "admin-guardrails",
                    "list",
                    "--current-page",
                    "9007199254740992",
                ],
                "Expected a safe integer",
            ),
            (
                &["deployments", "list", "--output", "yaml"],
                "Invalid output format 'yaml'. Expected: pretty, table, json",
            ),
        ] {
            let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
            let mut full = vec!["--debug", "gateway"];
            full.extend_from_slice(args);
            let out = f.run_with(&full, &RunOptions::new().env("XDG_STATE_HOME", &state));
            out.assert_code(2);
            assert_eq!(out.stderr, format!("\n  ✗ {message}\n\n"), "{args:?}");
            assert!(f.server.requests().is_empty(), "{args:?}");
            assert!(!debug_log::log_dir(&f.sandbox).exists(), "{args:?}");
        }
        // A missing required `--workspace` is clap's (D1).
        f.gw(&["providers", "list"]).assert_code(2);
        // Without a tenant the usage error still comes first (D232).
        let sandbox = Sandbox::new();
        let out = run(
            &rust(),
            &sandbox,
            &["gateway", "configs", "get", "config-1"],
            &RunOptions::new(),
        );
        out.assert_code(2);
    }

    /// D245: an explicit `--tsg-id` other than the tenant's is refused after the tenant
    /// (exit 1) with no token request and nothing sent.
    #[test]
    fn a_foreign_tsg_is_refused() {
        let f = fx();
        for args in [
            &["organisations", "info", "--tsg-id", "999"][..],
            &["organisations", "auth-settings", "get", "--tsg-id", "0100"],
        ] {
            let out = f.gw(args);
            out.assert_code(1);
            let value = args[args.len() - 1];
            assert_eq!(
                out.stderr,
                format!(
                    "\n  ✗ Error: --tsg-id {value} is not the selected tenant's TSG; airs reads and changes only the selected tenant's organisation\n\n"
                )
            );
            assert!(f.server.requests().is_empty(), "{args:?}");
        }
    }

    /// The §3.5 credential check comes before any request.
    #[test]
    fn credentials_are_checked_first() {
        let f = fx_with(|config| {
            config.remove("mgmtClientSecret");
        });
        let out = f.gw(&["plugins", "list"]);
        out.assert_code(1);
        assert!(
            out.stderr.contains("missing mgmtClientSecret"),
            "{}",
            out.stderr
        );
        assert!(f.server.requests().is_empty());
    }
}

// ---------------------------------------------------------------------------
// admin-guardrails (TS aigateway-admin-guardrails.spec.ts, integration spec)
// ---------------------------------------------------------------------------

mod admin_guardrails {
    use super::*;

    /// TS: list without a workspace sends no filter; an explicit page 0 is kept; only the
    /// given keys are sent, in TS order; `get` by id; the admin endpoint with OAuth and
    /// `x-tsg-id`.
    #[test]
    fn list_queries_and_get() {
        let f = fx();
        f.gw(&["admin-guardrails", "list"]).assert_success();
        assert_eq!(f.api(), [format!("GET {ADMIN}/guardrails")]);
        f.clear();
        f.gw(&["admin-guardrails", "list", "--current-page", "0"])
            .assert_success();
        assert_eq!(f.api(), [format!("GET {ADMIN}/guardrails?current_page=0")]);
        f.clear();
        f.gw(&[
            "admin-guardrails",
            "ls",
            "--current-page",
            "2",
            "--page-size",
            "50",
            "--workspace",
            &u(1),
        ])
        .assert_success();
        assert_eq!(
            f.api(),
            [format!(
                "GET {ADMIN}/guardrails?workspace_id={}&page_size=50&current_page=2",
                u(1)
            )]
        );
        f.clear();
        let out = f.gw(&[
            "admin-guardrails",
            "get",
            &u(fx::ADMIN_GUARDRAIL),
            "--output",
            "json",
        ]);
        assert_eq!(out.json()["target"], "mcp_tools");
        let request = &f.server.gw_requests(Gw::AdminGuardrailGet)[0];
        assert_eq!(request.header("x-tsg-id"), Some(TSG));
    }

    /// Choice 64: a page whose `total` says more exist prints the paged status line on
    /// stderr (quiet-suppressed), never the G39 warning.
    #[test]
    fn a_partial_page_prints_its_status_line() {
        let f = fx();
        f.data(|data| {
            data.inventory.get_mut(&Gw::AdminGuardrails).unwrap()["total"] = json!(7);
        });
        let out = f.gw(&[
            "admin-guardrails",
            "list",
            "--page-size",
            "1",
            "--current-page",
            "2",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            out.stderr,
            "  Returned 1 of 7 admin guardrails (page 2); this is one page.\n"
        );
        let quiet = f.run_with(
            &["--quiet", "gateway", "admin-guardrails", "list"],
            &RunOptions::new(),
        );
        assert_eq!(quiet.stderr, "");
        f.data(|data| {
            data.inventory.get_mut(&Gw::AdminGuardrails).unwrap()["total"] = json!(1);
        });
        assert_eq!(f.gw(&["admin-guardrails", "list"]).stderr, "");
    }

    /// TS `integration/aigateway-admin-guardrails.spec.ts`: the real client on the admin
    /// endpoint with OAuth and `x-tsg-id`; an empty page is `[]`; invalid paging sends
    /// nothing; a denied read is re-read once after a token refresh and stays on the admin
    /// plane.
    #[test]
    fn the_admin_endpoint_with_oauth_and_the_tsg() {
        let f = fx();
        f.data(|data| {
            data.inventory
                .insert(Gw::AdminGuardrails, json!({"data": [], "total": 0}));
        });
        let out = f.gw(&[
            "admin-guardrails",
            "list",
            "--page-size",
            "100",
            "--current-page",
            "0",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(out.stdout, "[]\n");
        let routes: Vec<String> = f
            .server
            .requests()
            .iter()
            .map(RecordedRequest::line)
            .collect();
        assert_eq!(
            routes,
            [
                "POST /oauth/token".to_owned(),
                format!("GET {ADMIN}/guardrails?page_size=100&current_page=0")
            ]
        );
        f.clear();
        f.gw(&["admin-guardrails", "list", "--page-size", "1001"])
            .assert_code(2);
        assert!(f.server.requests().is_empty());
        f.server.set_default(
            Route::Gateway(Gw::AdminGuardrails),
            Planned::json(403, &json!({"message": "denied"})),
        );
        let out = f.gw(&["admin-guardrails", "list"]);
        out.assert_code(1);
        assert!(out.stderr.starts_with(ADMIN_HINT), "{}", out.stderr);
        let routes: Vec<String> = f
            .server
            .requests()
            .iter()
            .map(RecordedRequest::line)
            .collect();
        assert_eq!(
            routes,
            [
                "POST /oauth/token".to_owned(),
                format!("GET {ADMIN}/guardrails"),
                "POST /oauth/token".to_owned(),
                format!("GET {ADMIN}/guardrails"),
            ]
        );
    }

    /// TS: malformed paging integers are refused before the tenant and any credential.
    #[test]
    fn malformed_integers_need_no_tenant() {
        let sandbox = Sandbox::new();
        for (value, message) in [
            ("1.5", "Expected an integer"),
            ("1e3", "Expected an integer"),
            ("2000", "--page-size must be an integer from 1 to 1000"),
        ] {
            let out = run(
                &rust(),
                &sandbox,
                &["gateway", "admin-guardrails", "list", "--page-size", value],
                &RunOptions::new(),
            );
            out.assert_code(2);
            assert_eq!(out.stderr, format!("\n  ✗ {message}\n\n"));
        }
    }
}

// ---------------------------------------------------------------------------
// Incomplete lists (G39, D248)
// ---------------------------------------------------------------------------

mod incomplete {
    use super::*;

    /// A list whose `total` or `has_more` says it is incomplete warns on stderr (never
    /// quiet-suppressed), exit 0; so do the detail-rendered lists (choice 78).
    #[test]
    fn incomplete_lists_warn() {
        let f = fx();
        f.data(|data| {
            data.inventory.get_mut(&Gw::Deployments).unwrap()["total"] = json!(3);
            data.inventory.get_mut(&Gw::McpIntegrations).unwrap()["has_more"] = json!(true);
            data.inventory.get_mut(&Gw::McpCapabilities).unwrap()["has_more"] = json!(true);
            data.inventory.get_mut(&Gw::IntegrationWorkspaces).unwrap()["total"] = json!(4);
            data.inventory.get_mut(&Gw::AuditLogs).unwrap()["total"] = json!(10);
        });
        for (args, warning) in [
            (strings(&["deployments", "list"]), "1 of 3 deployments"),
            (
                strings(&["mcp", "integrations", "list"]),
                "1 of more than 1 MCP integrations",
            ),
            (
                strings(&[
                    "mcp",
                    "integrations",
                    "capabilities",
                    "list",
                    &u(fx::MCP_INTEGRATION),
                ]),
                "1 of more than 1 MCP capabilities",
            ),
            (
                strings(&["integrations", "workspaces", "list", &u(fx::INTEGRATION)]),
                "1 of 4 workspace bindings",
            ),
            (
                strings(&["audit-logs", "list", "--days", "1"]),
                "2 of 10 audit logs",
            ),
        ] {
            let mut full = vec!["--quiet", "gateway"];
            full.extend(refs(&args));
            let out = f.run_with(&full, &RunOptions::new());
            out.assert_success();
            assert_eq!(
                out.stderr,
                format!(
                    "  ⚠ The service returned {warning}; the list is incomplete (the API offers no continuation)\n"
                ),
                "{args:?}"
            );
        }
        assert_eq!(f.gw(&["plugins", "list"]).stderr, "", "a complete list");
    }
}

// ---------------------------------------------------------------------------
// Failures (§19.3; D10, D11, D234)
// ---------------------------------------------------------------------------

mod failures {
    use super::*;

    /// The grant hint by plane and service code on stderr before the error; a read 403 is
    /// re-read once after a token refresh (D10).
    #[test]
    fn grant_hints_by_plane() {
        for (route, args, body, hint) in [
            (
                Gw::ServiceKeys,
                strings(&["api-keys", "service", "list", "--workspace", &u(1)]),
                json!({"message": "x"}),
                Some(WORKSPACE_HINT),
            ),
            (
                Gw::ProviderGet,
                strings(&["providers", "get", &u(fx::PROVIDER)]),
                json!({"errorCode": "AB01"}),
                None,
            ),
            (
                Gw::Integrations,
                strings(&["integrations", "list"]),
                json!({"message": "x"}),
                Some(ADMIN_HINT),
            ),
            (
                Gw::AuditLogs,
                strings(&["audit-logs", "list"]),
                json!({"error": {"code": "AB03"}}),
                Some(WORKSPACE_HINT),
            ),
        ] {
            let f = fx();
            f.server
                .set_default(Route::Gateway(route), Planned::json(403, &body));
            let out = f.gw(&refs(&args));
            out.assert_code(1);
            assert_eq!(out.stdout, "");
            match hint {
                Some(hint) => assert!(out.stderr.starts_with(hint), "{args:?}: {}", out.stderr),
                None => assert!(out.stderr.starts_with("\n  ✗ Error:"), "{}", out.stderr),
            }
            assert!(out.stderr.contains("    HTTP 403\n"));
            assert_eq!(f.server.route_count(Route::Gateway(route)), 2, "{args:?}");
        }
    }

    /// A 404 is the §6 error (exit 1, no server text); a 503 and a dropped connection are
    /// read again (D10); a 2xx answer that fails the schema is a decode error.
    #[test]
    fn errors_retries_and_decode_failures() {
        let f = fx();
        f.data(|data| {
            data.inventory.get_mut(&Gw::ConfigGet).unwrap()["id"] = json!(u(fx::CONFIG));
        });
        let out = f.gw(&["configs", "get", &u(399)]);
        out.assert_code(1);
        assert!(out.stderr.contains("    HTTP 404\n"), "{}", out.stderr);
        assert!(!out.stderr.contains("Not found"), "{}", out.stderr);
        f.server.push_once(
            Route::Gateway(Gw::Plugins),
            Planned::json(503, &json!({"message": "busy SERVERTEXT"}))
                .with_header("retry-after", "0"),
        );
        let out = f.gw(&["plugins", "list", "--output", "json"]);
        out.assert_success();
        assert_eq!(f.server.route_count(Route::Gateway(Gw::Plugins)), 2);
        f.server
            .push_once(Route::Gateway(Gw::Deployments), Planned::disconnect());
        f.gw(&["deployments", "list"]).assert_success();
        assert_eq!(f.server.route_count(Route::Gateway(Gw::Deployments)), 2);
        f.data(|data| {
            data.inventory
                .insert(Gw::GuardrailCatalog, json!({"evals": "SERVERTEXT"}));
        });
        let out = f.gw(&["guardrails", "catalog"]);
        out.assert_code(1);
        assert!(out.stderr.contains("AISEC_"), "{}", out.stderr);
        assert!(!out.stderr.contains("SERVERTEXT"), "{}", out.stderr);
    }
}

// ---------------------------------------------------------------------------
// --debug (§19.3, D64, G45)
// ---------------------------------------------------------------------------

mod debug {
    use super::*;

    /// Every read records its SDK operation name (G45), the pinned SDK's user agent and no
    /// body or query.
    #[test]
    fn reads_record_their_operations_and_no_body_or_query() {
        for (args, operation) in [
            (
                strings(&["api-keys", "user", "list", "--workspace", &u(1)]),
                "airs.gateway.api_keys.list_user",
            ),
            (
                strings(&["audit-logs", "list"]),
                "airs.gateway.audit_logs.list",
            ),
            (
                strings(&["configs", "versions", &u(fx::CONFIG)]),
                "airs.gateway.configs.list_versions",
            ),
            (
                strings(&["deployments", "ping", &u(fx::DEPLOYMENT)]),
                "airs.gateway.deployments.ping",
            ),
            (
                strings(&["guardrails", "catalog"]),
                "airs.gateway.guardrails.get_catalog",
            ),
            (
                strings(&["integrations", "providers"]),
                "airs.gateway.integrations.catalog",
            ),
            (
                strings(&["mcp", "integrations", "metadata", &u(fx::MCP_INTEGRATION)]),
                "airs.gateway.mcp_integrations.get_metadata",
            ),
            (
                strings(&["organisations", "auth-settings", "get"]),
                "airs.gateway.organisations.get_auth_settings",
            ),
            (
                strings(&["providers", "get", &u(fx::PROVIDER)]),
                "airs.gateway.providers.get",
            ),
            (
                strings(&["admin-guardrails", "list", "--page-size", "5"]),
                "airs.gateway.admin_guardrails.list",
            ),
        ] {
            let f = fx();
            let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
            let mut full = vec!["--debug", "gateway"];
            full.extend(refs(&args));
            let out = f.run_with(&full, &RunOptions::new().env("XDG_STATE_HOME", &state));
            out.assert_success();
            let (path, records) = debug_log::only_log(&f.sandbox);
            let text = std::fs::read_to_string(&path).unwrap();
            for absent in ["workspace_id", "start_time", "page_size", "ci-bot", "Edge"] {
                assert!(!text.contains(absent), "{args:?}: {absent}");
            }
            let found: Vec<&str> = records
                .iter()
                .filter_map(|record| record["operation"].as_str())
                .filter(|operation| *operation != "oauth.token")
                .collect();
            assert_eq!(found, [operation], "{args:?}");
            // The published SDK's user agent follows the exact scm-sdk 0.2.6 pin.
            for record in records
                .iter()
                .filter(|record| record["operation"] != "oauth.token")
            {
                assert_eq!(
                    record["request"]["headers"]["user-agent"], "PAN-AIRS/0.2.6-rust-sdk",
                    "{args:?}"
                );
            }
            debug_log::assert_logs_clean(
                &f.sandbox,
                &f.server,
                &[debug_log::log_dir(&f.sandbox)],
                &[],
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Interruption (§19.3, D252)
// ---------------------------------------------------------------------------

mod interrupt {
    use super::*;

    /// A signal during an inventory read writes no data (exit 130 / 143).
    #[test]
    fn a_signal_during_a_read_writes_nothing() {
        for (signal, code, route, args) in [
            (
                "INT",
                130,
                Gw::AuthSettings,
                vec!["gateway", "organisations", "auth-settings", "get"],
            ),
            ("TERM", 143, Gw::Plugins, vec!["gateway", "plugins", "list"]),
        ] {
            let f = fx();
            f.server.push_once(Route::Gateway(route), Planned::hang());
            let running = spawn(
                &rust(),
                &f.sandbox,
                &args,
                &RunOptions::new().expect_signal(),
            );
            assert!(
                f.server
                    .wait_for_route(Route::Gateway(route), 1, Duration::from_secs(10))
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
