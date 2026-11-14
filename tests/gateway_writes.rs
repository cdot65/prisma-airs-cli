//! Executable acceptance tests for the M6e chunk-3 gateway writes (DESIGN §19.3, §19.6,
//! §19.7, §19.10–§19.13): every `create`, `update`, `delete|rm`, `archive`, `rotate`,
//! `set`, `sync`, `upsert` and `bind`, the provisioning of `workspaces create`, the
//! one-time credential channel (`--secret-output`), the credential inputs, the
//! confirmations and the interruption contexts.
//!
//! Every test spawns the real `airs-cli-rs` against the loopback fixture server with the
//! seeded gateway dataset; the runner's leak check forbids every `FAKE-GW-SECRET-*` marker
//! (the one-time credentials of the write answers included) on every stream and in the
//! debug log of each run. The TypeScript `aigateway-mutation-commands.spec.ts`,
//! `aigateway-structured-input.spec.ts`, `aigateway-write-helpers.spec.ts`,
//! `aigateway-workspace-provision.spec.ts`, `aigateway-admin-guardrails.spec.ts` (writes)
//! and `unit/airs/aigateway.spec.ts` (provisioning, update, archive, bind) cases are ported
//! here (§19.19), adjusted by the §12 rows.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use airs_test_harness::fixtures::gateway as fx;
use airs_test_harness::{
    Cli, FixtureServer, GatewayData, Gw, Planned, PtyRun, RecordedRequest, Route, Run, RunOptions,
    Sandbox, debug_log, run, spawn, spawn_pty,
};
use serde_json::{Value, json};

#[path = "gateway_writes/reliability.rs"]
mod reliability;

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

const TSG: &str = "100";
const DATA: &str = "/ai_gw/v2";
const ADMIN: &str = "/ai_gw/admin/v2";
const IAM: &str = "/iam";
const DEBUG_HINT: &str = "    Re-run with --debug to capture redacted API diagnostics.\n";
const DATA_HINT: &str = "  ⚠ 403: the service account is missing a workspace-scope grant (data plane, /ai_gw/v2). SCM Access Management edits the existing role row by default — use \"Add Role\" so the account ends up with both role rows, not one row moved.\n";

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
    let server = FixtureServer::start();
    server.seed_gateway();
    let sandbox = Sandbox::new();
    sandbox.single_tenant(&server, "prod", TSG);
    Fx { server, sandbox }
}

/// A sandbox without any tenant (usage errors need none).
fn bare() -> Fx {
    Fx {
        server: FixtureServer::start(),
        sandbox: Sandbox::new(),
    }
}

fn gw<'a>(args: &[&'a str]) -> Vec<&'a str> {
    std::iter::once("gateway")
        .chain(args.iter().copied())
        .collect()
}

impl Fx {
    fn gw(&self, args: &[&str]) -> Run {
        self.gw_with(args, &RunOptions::new())
    }

    fn gw_with(&self, args: &[&str], options: &RunOptions) -> Run {
        let out = run(&rust(), &self.sandbox, &gw(args), options);
        self.server.assert_no_unknown_routes();
        assert!(self.sandbox.cwd_listing().is_empty(), "wrote into cwd");
        assert!(self.sandbox.home_listing().is_empty(), "wrote into HOME");
        out
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

    fn writes(&self) -> Vec<RecordedRequest> {
        self.server
            .requests()
            .into_iter()
            .filter(|request| request.route.is_write())
            .collect()
    }

    fn tokens(&self) -> usize {
        self.server
            .routes()
            .iter()
            .filter(|route| **route == Route::Token)
            .count()
    }

    /// The body of the one request on `gw`.
    fn body(&self, gw: Gw) -> Value {
        let requests = self.server.gw_requests(gw);
        assert_eq!(requests.len(), 1, "{gw:?}: {requests:?}");
        requests[0].body_json().unwrap_or(Value::Null)
    }

    /// A file under the sandbox's `tmp/`; its absolute path.
    fn file(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.sandbox.tmp().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.display().to_string()
    }

    /// A path under `tmp/` (nothing there).
    fn path(&self, name: &str) -> String {
        self.sandbox.tmp().join(name).display().to_string()
    }

    fn data(&self, edit: impl FnOnce(&mut GatewayData)) {
        self.server.gateway(edit);
    }

    fn once(&self, gw: Gw, planned: Planned) {
        self.server.push_once(Route::Gateway(gw), planned);
    }
}

fn error_block(message: &str, status: Option<u16>, details: &[&str]) -> String {
    let mut text = format!("\n  ✗ Error: {message}\n");
    if let Some(status) = status {
        text.push_str(&format!("    HTTP {status}\n{DEBUG_HINT}"));
    }
    for line in details {
        text.push_str(&format!("    {line}\n"));
    }
    text.push('\n');
    text
}

fn usage_block(message: &str) -> String {
    format!("\n  ✗ {message}\n\n")
}

/// A usage error in a sandbox without any tenant: exit 2, the exact message, no request.
fn assert_usage(args: &[&str], message: &str) {
    let f = bare();
    let out = f.gw(args);
    out.assert_code(2);
    assert_eq!(out.stdout, "", "{args:?}");
    assert_eq!(out.stderr, usage_block(message), "{args:?}");
    assert_eq!(f.server.request_count(), 0, "{args:?}");
}

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// The `.airs-*.tmp` temporaries in `dir`.
fn temporaries(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".airs-")
        })
        .collect()
}

fn transcript(out: &PtyRun) -> String {
    out.transcript.replace("\r\n", "\n")
}

/// The `Field │ Value` box table of `rows` (pretty and table of a write answer).
fn detail_table(rows: &[(&str, &str)]) -> String {
    let width = |items: &mut dyn Iterator<Item = usize>| items.max().unwrap_or(0);
    let field = width(&mut rows.iter().map(|(k, _)| k.chars().count()).chain([5]));
    let value = width(&mut rows.iter().map(|(_, v)| v.chars().count()).chain([5]));
    let mut text = format!(
        " {:field$} │ {:value$} \n{}┼{}\n",
        "Field",
        "Value",
        "─".repeat(field + 2),
        "─".repeat(value + 2)
    );
    for (k, v) in rows {
        text.push_str(&format!(" {k:field$} │ {v:value$} \n"));
    }
    text
}

// ---------------------------------------------------------------------------
// The command tree (TS `aigateway-command-tree.spec.ts`, chunk 3)
// ---------------------------------------------------------------------------

mod tree {
    use super::*;

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

    /// Every group lists its write leaves in TS's sorted order; `delete` has `rm` (the
    /// hidden `workspaces delete` is listed nowhere and has none, TS).
    #[test]
    fn write_leaves_are_registered_in_typescript_order() {
        let f = bare();
        for (path, expected) in [
            (
                &["workspaces"][..],
                &["archive", "create", "get", "list", "update"][..],
            ),
            (&["scopes"], &["bind", "create", "delete", "get", "list"]),
            (
                &["configs"],
                &["create", "delete", "get", "list", "update", "versions"],
            ),
            (
                &["guardrails"],
                &["catalog", "create", "delete", "get", "list", "update"],
            ),
            (
                &["providers"],
                &["create", "delete", "get", "list", "update"],
            ),
            (
                &["api-keys", "service"],
                &["create", "delete", "get", "list", "rotate", "update"],
            ),
            (
                &["api-keys", "user"],
                &["create", "delete", "get", "list", "rotate", "update"],
            ),
            (
                &["deployments"],
                &["archive", "create", "get", "list", "ping", "update"],
            ),
            (
                &["integrations"],
                &[
                    "create",
                    "delete",
                    "get",
                    "list",
                    "models",
                    "providers",
                    "update",
                    "workspaces",
                ],
            ),
            (&["integrations", "models"], &["list", "set"]),
            (&["integrations", "workspaces"], &["list", "set"]),
            (
                &["mcp", "integrations"],
                &[
                    "capabilities",
                    "create",
                    "delete",
                    "get",
                    "list",
                    "metadata",
                    "update",
                    "workspaces",
                ],
            ),
            (&["mcp", "integrations", "capabilities"], &["list", "set"]),
            (&["mcp", "integrations", "workspaces"], &["list", "set"]),
            (&["organisations", "self"], &["get", "update"]),
            (&["organisations", "auth-settings"], &["get", "update"]),
            (&["plugins"], &["create", "list"]),
            (
                &["admin-guardrails"],
                &["create", "delete", "get", "list", "mcp-servers", "update"],
            ),
            (
                &["admin-guardrails", "mcp-servers"],
                &["list", "sync", "upsert"],
            ),
        ] {
            let mut args = path.to_vec();
            args.push("--help");
            let out = f.gw(&args);
            out.assert_success();
            assert_eq!(commands(&out.stdout), expected, "{path:?}");
            if expected.contains(&"delete") {
                assert!(out.stdout.contains("[aliases: rm]"), "{path:?}");
            }
        }
        // The hidden `workspaces delete` exists (with --force) but has no `rm`.
        let out = f.gw(&["workspaces", "delete", "--help"]);
        out.assert_success();
        assert!(out.stdout.contains("--force"));
        let out = f.gw(&["workspaces", "rm", "x", "--force"]);
        out.assert_code(2);
    }

    /// TS "structured flags primary, `--file` optional" and the repeatable relationship
    /// flags; `--show-secret` is hidden, `providers create` has no `--reset-usage` (D245).
    #[test]
    fn structured_flags_and_hidden_refusals() {
        let f = bare();
        for path in [
            &["configs", "create"][..],
            &["guardrails", "update"],
            &["integrations", "create"],
            &["mcp", "integrations", "workspaces", "set"],
            &["admin-guardrails", "mcp-servers", "sync"],
            &["organisations", "auth-settings", "update"],
        ] {
            let mut args = path.to_vec();
            args.push("--help");
            let out = f.gw(&args);
            out.assert_success();
            for flag in [
                "--file <path>",
                "--set <path=value>",
                "--set-string <path=value>",
            ] {
                assert!(out.stdout.contains(flag), "{path:?}: {flag}");
            }
            assert!(
                out.stdout
                    .contains("Advanced JSON request base; named flags override it"),
                "{path:?}"
            );
        }
        let out = f.gw(&["api-keys", "service", "create", "--help"]);
        assert!(!out.stdout.contains("--show-secret"));
        assert!(out.stdout.contains(
            "New private file (mode 0600) that receives the one-time credential; never overwritten (the credential is never printed)"
        ));
        let out = f.gw(&["providers", "create", "--help"]);
        assert!(!out.stdout.contains("--reset-usage"));
        let out = f.gw(&["providers", "update", "--help"]);
        assert!(out.stdout.contains("--reset-usage <boolean>"));
        let out = f.gw(&["integrations", "workspaces", "set", "--help"]);
        assert!(out.stdout.contains("--workspace-binding <id=enabled>"));
        assert!(out.stdout.contains("--preserve-existing"));
    }
}

// ---------------------------------------------------------------------------
// Request bodies (§19.3; TS `aigateway-mutation-commands.spec.ts`,
// `aigateway-structured-input.spec.ts`)
// ---------------------------------------------------------------------------

mod bodies {
    use super::*;

    /// TS "passes a validated JSON config escape-hatch body to the SDK without reshaping
    /// it" and "builds config bodies from named and dotted flags without a file": one POST;
    /// the answer on stdout (generic detail), the Rust acknowledgement on stderr (D244).
    #[test]
    fn configs_from_a_file_and_from_flags() {
        let f = fx();
        let body = json!({"name": "test-config", "workspace_id": u(1), "config": {"retry": {"attempts": 3}}});
        let file = f.file("config.json", body.to_string().as_bytes());
        let out = f.gw(&["configs", "create", "--file", &file, "--output", "json"]);
        out.assert_success();
        assert_eq!(f.body(Gw::ConfigCreate), body);
        assert_eq!(f.api(), [format!("POST {DATA}/configs")]);
        assert_eq!(
            out.json(),
            json!({"id": u(fx::NEW_CONFIG), "version_id": u(fx::NEW_CONFIG + 50), "slug": "config-new", "object": "config"})
        );
        assert_eq!(
            out.stderr,
            format!("  ✓ Config created: {}\n", u(fx::NEW_CONFIG))
        );
        let f = fx();
        let out = f.gw(&[
            "configs",
            "create",
            "--name",
            "from-flags",
            "--workspace",
            &u(1),
            "--set",
            "config.retry.attempts=3",
            "--set",
            "config.strategy.mode=fallback",
        ]);
        out.assert_success();
        assert_eq!(
            f.body(Gw::ConfigCreate),
            json!({"name": "from-flags", "workspace_id": u(1), "config": {"retry": {"attempts": 3}, "strategy": {"mode": "fallback"}}})
        );
        assert_eq!(
            out.stdout,
            detail_table(&[
                ("id", &u(fx::NEW_CONFIG)),
                ("version_id", &u(fx::NEW_CONFIG + 50)),
                ("slug", "config-new"),
                ("object", "config"),
            ])
        );
    }

    /// TS "merges an optional file with command flags taking precedence" and "supports
    /// forced strings": file < flags < `--set`/`--set-string`.
    #[test]
    fn file_flags_and_assignments_take_precedence_in_order() {
        let f = fx();
        let file = f.file(
            "base.json",
            json!({"name": "file-name", "workspace_id": u(1), "config": {"strategy": {"mode": "single"}, "note": "x"}})
                .to_string()
                .as_bytes(),
        );
        let out = f.gw(&[
            "configs",
            "create",
            "--file",
            &file,
            "--name",
            "override",
            "--set",
            "config.strategy.mode=\"loadbalance\"",
            "--set-string",
            "config.note=123",
        ]);
        out.assert_success();
        assert_eq!(
            f.body(Gw::ConfigCreate),
            json!({"name": "override", "workspace_id": u(1), "config": {"strategy": {"mode": "loadbalance"}, "note": "123"}})
        );
    }

    /// TS "builds each configuration resource from named SDK-backed flags", with the
    /// organisation defaulting to the tenant's TSG (D245).
    #[test]
    fn every_resource_builds_its_named_flags() {
        let catalog = u(fx::CATALOG_PROVIDER);
        let integration = u(fx::INTEGRATION);
        let cases: Vec<(Vec<String>, Gw, Value)> = vec![
            (
                vec![
                    "guardrails".into(),
                    "create".into(),
                    "--workspace".into(),
                    u(1),
                    "--name".into(),
                    "deny-risk".into(),
                    "--set".into(),
                    "checks[0].id=prompt-injection".into(),
                    "--set".into(),
                    "actions.deny=true".into(),
                ],
                Gw::GuardrailCreate,
                json!({"workspace_id": u(1), "name": "deny-risk", "checks": [{"id": "prompt-injection"}], "actions": {"deny": true}}),
            ),
            (
                vec![
                    "mcp".into(),
                    "integrations".into(),
                    "create".into(),
                    "--name".into(),
                    "readonly-mcp".into(),
                    "--slug".into(),
                    "readonly-mcp".into(),
                    "--url".into(),
                    "https://mcp.example.test/mcp".into(),
                    "--auth-type".into(),
                    "none".into(),
                    "--transport".into(),
                    "http".into(),
                ],
                Gw::McpCreate,
                json!({"name": "readonly-mcp", "organisation_id": TSG, "slug": "readonly-mcp", "url": "https://mcp.example.test/mcp", "auth_type": "none", "transport": "http"}),
            ),
            (
                vec![
                    "providers".into(),
                    "create".into(),
                    "--workspace".into(),
                    u(1),
                    "--ai-provider-id".into(),
                    catalog.clone(),
                    "--integration-id".into(),
                    integration.clone(),
                    "--name".into(),
                    "vertex-binding".into(),
                    "--slug".into(),
                    "vertex-binding".into(),
                    "--expires-at".into(),
                    "2027-01-01T00:00:00Z".into(),
                ],
                Gw::ProviderCreate,
                json!({"workspace_id": u(1), "ai_provider_id": catalog, "integration_id": integration, "name": "vertex-binding", "slug": "vertex-binding", "expires_at": "2027-01-01T00:00:00Z"}),
            ),
            (
                vec![
                    "admin-guardrails".into(),
                    "create".into(),
                    "--name".into(),
                    "Org".into(),
                    "--target".into(),
                    "mcp_tools".into(),
                    "--organisation".into(),
                    u(fx::ORGANISATION),
                ],
                Gw::AdminGuardrailCreate,
                json!({"name": "Org", "target": "mcp_tools", "organisation_id": u(fx::ORGANISATION)}),
            ),
            (
                vec![
                    "admin-guardrails".into(),
                    "update".into(),
                    u(fx::ADMIN_GUARDRAIL),
                    "--name".into(),
                    "Renamed".into(),
                ],
                Gw::AdminGuardrailUpdate,
                json!({"name": "Renamed"}),
            ),
            (
                vec![
                    "admin-guardrails".into(),
                    "mcp-servers".into(),
                    "upsert".into(),
                    u(fx::ADMIN_GUARDRAIL),
                    u(fx::MCP_SERVER),
                    "--set".into(),
                    "run_on=[\"input\"]".into(),
                ],
                Gw::McpUpsert,
                json!({"run_on": ["input"]}),
            ),
            (
                vec![
                    "api-keys".into(),
                    "user".into(),
                    "update".into(),
                    u(fx::USER_KEY),
                    "--scopes".into(),
                    "mcp.invoke".into(),
                    "--alert-emails".into(),
                    " a@example.test, ,b@example.test ".into(),
                ],
                Gw::KeyUpdate,
                json!({"scopes": ["mcp.invoke"], "alert_emails": ["a@example.test", "b@example.test"]}),
            ),
            (
                vec![
                    "deployments".into(),
                    "update".into(),
                    u(fx::DEPLOYMENT),
                    "--name".into(),
                    "edge".into(),
                    "--is-default".into(),
                    "false".into(),
                    "--status".into(),
                    "active".into(),
                ],
                Gw::DeploymentUpdate,
                json!({"name": "edge", "is_default": false, "status": "active"}),
            ),
        ];
        for (args, route, expected) in cases {
            let f = fx();
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = f.gw(&args);
            out.assert_success();
            assert_eq!(f.body(route), expected, "{args:?}");
            assert_eq!(f.writes().len(), 1, "{args:?}");
        }
    }

    /// TS "reports structured-input failures as usage errors": exit 2, no request, no
    /// stack; the file is read before the tenant (a sandbox without one).
    #[test]
    fn structured_input_failures_are_usage_errors_before_any_request() {
        assert_usage(
            &[
                "configs",
                "create",
                "--name",
                "invalid",
                "--workspace",
                &u(1),
                "--set",
                "config.a=1",
                "--set",
                "config.a=2",
            ],
            "Invalid AI Gateway request: AISEC_USER_REQUEST_PAYLOAD_ERROR:Duplicate dotted path",
        );
        assert_usage(
            &["configs", "create", "--name", "missing-workspace"],
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /configs did not match schema",
        );
        assert_usage(
            &[
                "deployments",
                "update",
                &u(fx::DEPLOYMENT),
                "--is-default",
                "yes",
            ],
            "Invalid --is-default: Expected true or false",
        );
        assert_usage(
            &["configs", "update", &u(fx::CONFIG), "--set", "a..b=1"],
            "Invalid AI Gateway request: AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid empty segment in dotted path: a..b",
        );
        assert_usage(
            &[
                "configs",
                "update",
                &u(fx::CONFIG),
                "--set",
                "a.__proto__=1",
            ],
            "Invalid AI Gateway request: AISEC_USER_REQUEST_PAYLOAD_ERROR:Unsafe dotted path segment: __proto__",
        );
    }

    /// G49: the TS refinements are the CLI's, with TS's texts, before any request.
    #[test]
    fn refinements_use_the_typescript_texts() {
        for (args, message) in [
            (
                vec!["configs".to_owned(), "update".into(), u(fx::CONFIG)],
                "Invalid AI Gateway request: <root>: Provide at least one field",
            ),
            (
                vec![
                    "guardrails".to_owned(),
                    "create".into(),
                    "--workspace".into(),
                    u(1),
                    "--name".into(),
                    "g".into(),
                ],
                "Invalid AI Gateway request: checks: LLM guardrails require checks; actions: LLM guardrails require actions",
            ),
            (
                vec![
                    "api-keys".to_owned(),
                    "service".into(),
                    "rotate".into(),
                    u(fx::SERVICE_KEY),
                    "--transition-ms".into(),
                    "1000".into(),
                    "--secret-output".into(),
                    "k.json".into(),
                    "--force".into(),
                ],
                "Invalid AI Gateway request: key_transition_period_ms: Number must be greater than or equal to 1800000",
            ),
            (
                vec![
                    "integrations".to_owned(),
                    "models".into(),
                    "set".into(),
                    u(fx::INTEGRATION),
                    "--allow-all-models".into(),
                    "true".into(),
                    "--force".into(),
                ],
                "Invalid AI Gateway request: models: Required",
            ),
            (
                vec![
                    "integrations".to_owned(),
                    "models".into(),
                    "set".into(),
                    u(fx::INTEGRATION),
                    "--set".into(),
                    "models=[]".into(),
                    "--force".into(),
                ],
                "Invalid AI Gateway request: models: Array must contain at least 1 element(s)",
            ),
            (
                vec![
                    "plugins".to_owned(),
                    "create".into(),
                    "--integration-id".into(),
                    u(fx::INTEGRATION),
                ],
                "Invalid AI Gateway request: credentials: Provide at least one field",
            ),
            (
                vec![
                    "mcp".to_owned(),
                    "integrations".into(),
                    "capabilities".into(),
                    "set".into(),
                    u(fx::MCP_INTEGRATION),
                    "--force".into(),
                ],
                "Invalid AI Gateway request: capabilities: Required",
            ),
            (
                vec![
                    "mcp".to_owned(),
                    "integrations".into(),
                    "capabilities".into(),
                    "set".into(),
                    u(fx::MCP_INTEGRATION),
                    "--set".into(),
                    "capabilities=[]".into(),
                    "--force".into(),
                ],
                "Invalid AI Gateway request: capabilities: Array must contain at least 1 element(s)",
            ),
        ] {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            assert_usage(&refs, message);
        }
    }

    /// D232: an argument check is a usage error before the debug log is opened: no log
    /// file, no request; `--quiet` keeps the answer and drops the acknowledgement.
    #[test]
    fn argument_checks_open_no_debug_log() {
        let f = fx();
        let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
        let out = run(
            &rust(),
            &f.sandbox,
            &[
                "--debug", "gateway", "configs", "update", "c-1", "--name", "n",
            ],
            &RunOptions::new().env("XDG_STATE_HOME", &state),
        );
        out.assert_code(2);
        assert!(debug_log::log_files(&debug_log::log_dir(&f.sandbox)).is_empty());
        assert_eq!(f.server.request_count(), 0);
        let out = run(
            &rust(),
            &f.sandbox,
            &[
                "--quiet",
                "gateway",
                "guardrails",
                "update",
                &u(fx::GUARDRAIL),
                "--name",
                "g",
                "--output",
                "json",
            ],
            &RunOptions::new(),
        );
        out.assert_success();
        assert_eq!(out.json(), json!({"success": true}));
        assert_eq!(out.stderr, "");
    }

    /// D246: `--file` is one bounded JSON object; YAML and non-objects are refused without
    /// echoing the file; a missing file is the §13.2 message (exit 1).
    #[test]
    fn request_files_are_json_objects_only() {
        let f = bare();
        let yaml = f.file(
            "request.yaml",
            b"name: from-yaml\nsecret: FAKE-GW-SECRET-37\n",
        );
        let out = f.gw(&["configs", "create", "--file", &yaml]);
        out.assert_code(2);
        assert_eq!(
            out.stderr,
            usage_block(
                "--file must contain one JSON object (YAML request files are not supported)"
            )
        );
        let array = f.file("array.json", b"[1]");
        f.gw(&["configs", "create", "--file", &array])
            .assert_code(2);
        let missing = f.path("missing.json");
        let out = f.gw(&["configs", "create", "--file", &missing]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                &format!(
                    "Cannot read {missing} (missing, not a regular file, unreadable or larger than 1 MiB)"
                ),
                None,
                &[]
            )
        );
        assert_eq!(f.server.request_count(), 0);
    }

    /// D233: no value reaches an error: `--set` without `=`, a JSON flag, a map value, a
    /// schema mismatch quoting an enum value.
    #[test]
    fn values_are_never_echoed() {
        let marker = fx::marker(38);
        for (args, message) in [
            (vec!["configs".to_owned(), "create".into(), "--set".into(), marker.clone()], "Invalid --set: Expected path=value".to_owned()),
            (vec!["guardrails".to_owned(), "create".into(), "--checks".into(), format!("{{{marker}")], "Invalid --checks: Expected valid JSON".to_owned()),
            (vec!["plugins".to_owned(), "create".into(), "--credential".into(), format!("a={marker}"), "--credential".into(), "a=x".into()], "Invalid --credential: Duplicate key: a".to_owned()),
            (vec!["admin-guardrails".to_owned(), "create".into(), "--name".into(), "g".into(), "--target".into(), marker.clone()], "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for POST /guardrails did not match schema".to_owned()),
        ] {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let f = bare();
            let out = f.gw(&refs);
            out.assert_code(2);
            assert!(!out.stderr.contains(&marker), "{refs:?}: {}", out.stderr);
            assert!(out.stderr.contains(&message), "{refs:?}: {}", out.stderr);
        }
    }
}

// ---------------------------------------------------------------------------
// One attempt per write and the outcome lines (D228, D234, D244)
// ---------------------------------------------------------------------------

mod outcomes {
    use super::*;

    fn create(f: &Fx) -> Run {
        f.gw(&[
            "configs",
            "create",
            "--name",
            "c",
            "--workspace",
            &u(1),
            "--set",
            "config.retry.attempts=1",
        ])
    }

    /// D228: a 5xx, a dropped connection and an applied-then-dropped write are each sent
    /// once and end with the outcome-unknown line and the hint (TS re-sends up to 5 times).
    #[test]
    fn writes_are_sent_once_with_an_outcome_unknown_line() {
        let hint = format!(
            "Check 'airs gateway configs list --workspace {}' before creating it again.",
            u(1)
        );
        for planned in [
            Planned::json(503, &json!({"message": "busy FAKE-GW-SECRET-37"})).with_retry_after(0),
            Planned::disconnect(),
            Planned::applied_then_dropped(),
        ] {
            let f = fx();
            let status = (planned.status == 503).then_some(503);
            f.server
                .set_default(Route::Gateway(Gw::ConfigCreate), planned);
            let out = create(&f);
            out.assert_code(1);
            assert_eq!(out.stdout, "");
            assert_eq!(
                f.server.gw_requests(Gw::ConfigCreate).len(),
                1,
                "one attempt"
            );
            assert!(
                out.stderr.ends_with(&format!(
                    "    Outcome unknown: the config may have been created.\n    {hint}\n\n"
                )),
                "{}",
                out.stderr
            );
            if let Some(status) = status {
                assert!(out.stderr.contains(&format!("    HTTP {status}\n")));
            }
            assert!(!out.stderr.contains("FAKE-GW-SECRET-37"));
        }
    }

    /// D228: a 401 is replayed once after a fresh token (`token, write, token, write`); a
    /// 403 is sent once and gets the data-plane grant hint on stderr (D234); a 400 is a
    /// plain rejection without an outcome line.
    #[test]
    fn a_401_is_replayed_once_and_a_403_never() {
        let f = fx();
        f.once(Gw::ConfigCreate, Planned::empty_json(401));
        create(&f).assert_success();
        assert_eq!(f.tokens(), 2);
        assert_eq!(f.server.gw_requests(Gw::ConfigCreate).len(), 2);
        let f = fx();
        f.server.set_default(
            Route::Gateway(Gw::ConfigCreate),
            Planned::json(403, &json!({"message": "Forbidden"})),
        );
        let out = create(&f);
        out.assert_code(1);
        assert_eq!(f.server.gw_requests(Gw::ConfigCreate).len(), 1);
        assert!(out.stderr.starts_with(DATA_HINT), "{}", out.stderr);
        assert!(!out.stderr.contains("Outcome"));
        let f = fx();
        f.once(
            Gw::ConfigCreate,
            Planned::json(400, &json!({"message": "bad"})),
        );
        let out = create(&f);
        out.assert_code(1);
        assert!(!out.stderr.contains("Outcome"), "{}", out.stderr);
    }

    /// A 2xx whose answer cannot be read was applied: `Outcome: … could not be read.`
    #[test]
    fn an_unreadable_2xx_says_it_was_applied() {
        let f = fx();
        f.once(
            Gw::ConfigUpdate,
            Planned::raw(200, "application/json", "{not json"),
        );
        let out = f.gw(&["configs", "update", &u(fx::CONFIG), "--name", "n"]);
        out.assert_code(1);
        assert!(
            out.stderr.contains(&format!(
                "    Outcome: the config {} was updated, but the response could not be read.\n    Check it with 'airs gateway configs get {}'.\n",
                u(fx::CONFIG),
                u(fx::CONFIG)
            )),
            "{}",
            out.stderr
        );
    }

    /// D244: deletes, archives and revocations leave stdout empty in every format and
    /// acknowledge on stderr; `--quiet` drops the acknowledgement.
    #[test]
    fn removals_leave_stdout_empty_and_acknowledge() {
        for (args, ack) in [
            (
                vec!["configs".to_owned(), "delete".into(), u(fx::CONFIG)],
                format!("Config deleted: {}", u(fx::CONFIG)),
            ),
            (
                vec!["guardrails".to_owned(), "rm".into(), u(fx::GUARDRAIL)],
                format!("Guardrail deleted: {}", u(fx::GUARDRAIL)),
            ),
            (
                vec!["providers".to_owned(), "delete".into(), u(fx::PROVIDER)],
                format!("Provider deleted: {}", u(fx::PROVIDER)),
            ),
            (
                vec![
                    "api-keys".to_owned(),
                    "user".into(),
                    "delete".into(),
                    u(fx::USER_KEY),
                ],
                format!("User API key revoked: {}", u(fx::USER_KEY)),
            ),
            (
                vec![
                    "deployments".to_owned(),
                    "archive".into(),
                    u(fx::DEPLOYMENT),
                ],
                format!("Deployment archived: {}", u(fx::DEPLOYMENT)),
            ),
            (
                vec![
                    "integrations".to_owned(),
                    "delete".into(),
                    u(fx::INTEGRATION),
                ],
                format!("Integration deleted: {}", u(fx::INTEGRATION)),
            ),
            (
                vec![
                    "mcp".to_owned(),
                    "integrations".into(),
                    "rm".into(),
                    u(fx::MCP_INTEGRATION),
                ],
                format!("MCP integration deleted: {}", u(fx::MCP_INTEGRATION)),
            ),
            (
                vec![
                    "admin-guardrails".to_owned(),
                    "delete".into(),
                    u(fx::ADMIN_GUARDRAIL),
                ],
                format!("Admin guardrail deleted: {}", u(fx::ADMIN_GUARDRAIL)),
            ),
        ] {
            for format in ["pretty", "table", "json"] {
                let f = fx();
                let mut full: Vec<&str> = args.iter().map(String::as_str).collect();
                full.extend(["--force", "--output", format]);
                let out = f.gw(&full);
                out.assert_success();
                assert_eq!(out.stdout, "", "{full:?}");
                assert_eq!(out.stderr, format!("  ✓ {ack}\n"), "{full:?}");
                assert_eq!(f.writes().len(), 1);
            }
        }
        let f = fx();
        let out = f.gw_with(
            &["configs", "delete", &u(fx::CONFIG), "--force"],
            &RunOptions::new(),
        );
        out.assert_success();
        let quiet = run(
            &rust(),
            &f.sandbox,
            &[
                "--quiet",
                "gateway",
                "configs",
                "delete",
                &u(fx::CONFIG),
                "--force",
            ],
            &RunOptions::new(),
        );
        quiet.assert_success();
        assert_eq!(quiet.stderr, "");
        // The integration and deployment deletes send the tenant's TSG (D245).
        let f = fx();
        f.gw(&["integrations", "delete", &u(fx::INTEGRATION), "--force"])
            .assert_success();
        assert_eq!(
            f.api(),
            [format!(
                "DELETE {ADMIN}/integrations/{}?organisation_id={TSG}",
                u(fx::INTEGRATION)
            )]
        );
    }

    /// Every other write prints its answer through the generic detail renderer in every
    /// format; an empty `{}` answer prints `No fields returned`, nothing, `{}` (D231).
    #[test]
    fn write_answers_in_every_format() {
        let args = [
            "mcp",
            "integrations",
            "workspaces",
            "set",
            &u(fx::MCP_INTEGRATION),
            "--workspace-binding",
            "ws-main-a-349e0e=true",
            "--force",
        ];
        for (format, expected) in [
            ("pretty", "  No fields returned\n"),
            ("table", ""),
            ("json", "{}\n"),
        ] {
            let f = fx();
            let mut full = args.to_vec();
            full.extend(["--output", format]);
            let out = f.gw(&full);
            out.assert_success();
            assert_eq!(out.stdout, expected, "{format}");
            assert_eq!(
                out.stderr,
                format!(
                    "  ✓ MCP workspace access of {} replaced\n",
                    u(fx::MCP_INTEGRATION)
                )
            );
        }
        let f = fx();
        let out = f.gw(&[
            "organisations",
            "self",
            "update",
            "--name",
            "Dev",
            "--force",
            "--output",
            "table",
        ]);
        out.assert_success();
        assert_eq!(out.stdout, detail_table(&[("success", "true")]));
        assert_eq!(out.stderr, "  ✓ Current organisation updated\n");
    }
}

// ---------------------------------------------------------------------------
// One-time credentials (§19.11, D237, G36)
// ---------------------------------------------------------------------------

mod secrets {
    use super::*;

    fn key_args(f: &Fx, kind: &str, path: &str) -> Vec<String> {
        let _ = f;
        let mut args: Vec<String> = vec![
            "api-keys".into(),
            kind.into(),
            "create".into(),
            "--name".into(),
            "ci-bot".into(),
            "--scopes".into(),
            "completions.write,logs.write".into(),
            "--type".into(),
            "service".into(),
            "--workspace".into(),
            u(1),
            "--secret-output".into(),
            path.into(),
        ];
        if kind == "user" {
            args.extend(["--user-id".into(), u(1501)]);
        }
        args
    }

    /// TS "writes one-time credentials to a new owner-only file": the whole answer (sorted
    /// keys) in a new 0600 file, the key in no stream; stdout `{saved_to}`; the body with
    /// the tenant's TSG as `organisation_id` (D245); one raw POST (G36).
    #[test]
    fn key_creates_save_the_whole_answer_to_a_new_private_file() {
        for kind in ["service", "user"] {
            let f = fx();
            let path = f.path("key.json");
            let args = key_args(&f, kind, &path);
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = f.gw(&refs);
            out.assert_success();
            let mut expected = json!({
                "name": "ci-bot",
                "scopes": ["completions.write", "logs.write"],
                "type": "service",
                "workspace_id": u(1),
                "organisation_id": TSG,
            });
            if kind == "user" {
                expected["user_id"] = json!(u(1501));
            }
            assert_eq!(f.body(Gw::KeyCreate), expected);
            assert_eq!(f.api(), [format!("POST {DATA}/api-keys/{kind}")]);
            let saved: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(saved, fx::key_created(kind));
            assert!(std::fs::read_to_string(&path).unwrap().ends_with("}\n"));
            assert_eq!(mode(Path::new(&path)), 0o600);
            assert!(temporaries(&f.sandbox.tmp()).is_empty());
            let id = if kind == "user" {
                u(fx::NEW_USER_KEY)
            } else {
                u(fx::NEW_SERVICE_KEY)
            };
            let noun = if kind == "user" { "User" } else { "Service" };
            assert_eq!(
                out.stderr,
                format!(
                    "  ✓ {noun} API key created: {id}\n  ✓ Wrote the one-time credential to {path} (mode 0600); it is shown only once.\n"
                )
            );
            assert_eq!(out.stdout, detail_table(&[("saved_to", &path)]));
        }
        // json prints TS's `{saved_to}` document.
        let f = fx();
        let path = f.path("key2.json");
        let mut args = key_args(&f, "service", &path);
        args.extend(["--output".into(), "json".into()]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = f.gw(&refs);
        out.assert_success();
        assert_eq!(out.json(), json!({"saved_to": path}));
    }

    /// TS "requires an explicit destination before requesting a one-time API-key secret"
    /// and the D237/D226 path rules: usage errors before the tenant; path errors exit 1
    /// before the tenant; nothing is sent, no file and no temporary is left.
    #[test]
    fn destination_rules() {
        let f = bare();
        let base = ["api-keys", "service", "create", "--name", "k"];
        for (extra, message) in [
            (
                &[][..],
                "A one-time credential needs --secret-output <new file>; airs never prints it",
            ),
            (&["--secret-output", ""], "--secret-output needs a path"),
            (
                &["--secret-output", "-"],
                "--secret-output - would print the credential; give a file path",
            ),
            (
                &["--show-secret"],
                "--show-secret is not supported: one-time credentials are written only to --secret-output <new file>",
            ),
        ] {
            let mut args = base.to_vec();
            args.extend_from_slice(extra);
            assert_usage(&args, message);
        }
        let existing = f.file("exists.json", b"old");
        let dangling = f.sandbox.tmp().join("dangling.json");
        std::os::unix::fs::symlink(f.sandbox.tmp().join("nowhere"), &dangling).unwrap();
        let dangling = dangling.display().to_string();
        let missing = f
            .sandbox
            .tmp()
            .join("nodir")
            .join("k.json")
            .display()
            .to_string();
        let newdir = format!("{}/newdir/", f.sandbox.tmp().display());
        for (path, message) in [
            (
                existing.clone(),
                format!("{existing} already exists; choose another --secret-output"),
            ),
            (
                dangling.clone(),
                format!("{dangling} already exists; choose another --secret-output"),
            ),
            (
                missing.clone(),
                format!("Cannot write {missing}; check that its directory exists and is writable"),
            ),
            (
                newdir.clone(),
                format!("Cannot write {newdir}; check that its directory exists and is writable"),
            ),
        ] {
            let mut args = base.to_vec();
            args.extend(["--secret-output", &path]);
            let out = f.gw(&args);
            out.assert_code(1);
            assert_eq!(out.stderr, error_block(&message, None, &[]), "{path}");
        }
        assert_eq!(std::fs::read(&existing).unwrap(), b"old");
        assert_eq!(f.server.request_count(), 0);
        assert!(temporaries(&f.sandbox.tmp()).is_empty());
        // Deployments: a rotating update needs a destination; a non-rotating one refuses it.
        assert_usage(
            &[
                "deployments",
                "update",
                &u(fx::DEPLOYMENT),
                "--rotate-auth",
                "true",
                "--force",
            ],
            "A one-time credential needs --secret-output <new file>; airs never prints it",
        );
        let f = bare();
        let out = f.gw(&[
            "deployments",
            "update",
            &u(fx::DEPLOYMENT),
            "--name",
            "x",
            "--secret-output",
            &f.path("d.json"),
        ]);
        out.assert_code(2);
        assert_eq!(
            out.stderr,
            usage_block("--secret-output is used only with --rotate-auth true")
        );
    }

    /// TS "removes a reserved secret file when the API mutation fails": a rejected,
    /// unknown or credential-less answer leaves no file and no temporary.
    #[test]
    fn a_failed_write_removes_the_reservation() {
        for (planned, line) in [
            (Planned::json(400, &json!({"message": "bad"})), None),
            (
                Planned::json(502, &json!({})),
                Some("Outcome unknown: the service API key may have been created."),
            ),
        ] {
            let f = fx();
            let path = f.path("k.json");
            f.once(Gw::KeyCreate, planned);
            let args = key_args(&f, "service", &path);
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = f.gw(&refs);
            out.assert_code(1);
            assert!(!Path::new(&path).exists());
            assert!(temporaries(&f.sandbox.tmp()).is_empty());
            if let Some(line) = line {
                assert!(out.stderr.contains(line), "{}", out.stderr);
                assert!(
                    out.stderr
                        .contains("revoke a key whose credential was not saved")
                );
            }
            assert_eq!(f.server.gw_requests(Gw::KeyCreate).len(), 1);
        }
        // A 2xx without a credential: nothing written, the missing-credential message.
        let f = fx();
        let path = f.path("k.json");
        f.once(
            Gw::KeyCreate,
            Planned::json(
                200,
                &json!({"id": u(fx::NEW_SERVICE_KEY), "object": "api-key"}),
            ),
        );
        let args = key_args(&f, "service", &path);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = f.gw(&refs);
        out.assert_code(1);
        assert!(
            out.stderr.contains(&format!(
                "The service created service API key {} but returned no one-time credential; nothing was written to {path}.",
                u(fx::NEW_SERVICE_KEY)
            )),
            "{}",
            out.stderr
        );
        assert!(!Path::new(&path).exists());
        assert!(temporaries(&f.sandbox.tmp()).is_empty());
    }

    /// D228 on the G36 raw credential writes (`api-keys … create|rotate`, a rotating
    /// `deployments update`): a 401 is replayed once after a fresh token (`token, write,
    /// token, write`) and the credential saved once; a 403 is sent once with the plane's
    /// grant hint and leaves no file; an applied-then-dropped write is sent once, leaves no
    /// file and ends with the outcome-unknown line and the write's hint.
    #[test]
    fn credential_writes_follow_the_attempt_rules() {
        const ADMIN_HINT: &str = "  ⚠ 403: the service account is missing a tenant-root admin grant (admin plane, /ai_gw/admin/v2). SCM Access Management edits the existing role row by default — use \"Add Role\" so the account ends up with both role rows, not one row moved.\n";
        let key = u(fx::SERVICE_KEY);
        let deployment = u(fx::DEPLOYMENT);
        /// One raw credential write: its route, arguments, saved member, 403 hint and
        /// outcome-unknown lines.
        struct Case {
            gw: Gw,
            args: fn(&Fx, &str) -> Vec<String>,
            member: &'static str,
            grant: &'static str,
            unknown: String,
            hint: String,
        }
        let cases = [
            Case {
                gw: Gw::KeyCreate,
                args: |f, path| key_args(f, "service", path),
                member: "key",
                grant: DATA_HINT,
                unknown: "Outcome unknown: the service API key may have been created.".to_owned(),
                hint: "Check 'airs gateway api-keys service list --workspace <uuid>' before creating another key; revoke a key whose credential was not saved.".to_owned(),
            },
            Case {
                gw: Gw::KeyRotate,
                args: |_, path| {
                    let key = u(fx::SERVICE_KEY);
                    ["api-keys", "service", "rotate", &key, "--secret-output", path, "--force"]
                        .map(str::to_owned)
                        .to_vec()
                },
                member: "key",
                grant: DATA_HINT,
                unknown: format!("Outcome unknown: the service API key {key} may have been rotated."),
                hint: format!(
                    "The old key stops working after its transition period; check 'airs gateway api-keys service get {key}' and rotate it again to obtain a usable credential."
                ),
            },
            Case {
                gw: Gw::DeploymentUpdate,
                args: |_, path| {
                    let deployment = u(fx::DEPLOYMENT);
                    [
                        "deployments",
                        "update",
                        &deployment,
                        "--rotate-auth",
                        "true",
                        "--secret-output",
                        path,
                        "--force",
                    ]
                    .map(str::to_owned)
                    .to_vec()
                },
                member: "client_auth",
                grant: ADMIN_HINT,
                unknown: format!(
                    "Outcome unknown: the deployment {deployment} authentication may have been rotated."
                ),
                hint: "Its old credentials may no longer work; rotate it again with --secret-output <new file> to obtain usable ones.".to_owned(),
            },
        ];
        for Case {
            gw,
            args,
            member,
            grant,
            unknown,
            hint,
        } in &cases
        {
            let run_case = |f: &Fx, path: &str| {
                let args = args(f, path);
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                f.gw(&refs)
            };
            // A 401: a fresh token and one replay; the credential saved once.
            let f = fx();
            let path = f.path("c.json");
            f.once(*gw, Planned::empty_json(401));
            let out = run_case(&f, &path);
            out.assert_success();
            let order: Vec<Route> = f
                .server
                .routes()
                .into_iter()
                .filter(|route| *route == Route::Token || *route == Route::Gateway(*gw))
                .collect();
            assert_eq!(
                order,
                [
                    Route::Token,
                    Route::Gateway(*gw),
                    Route::Token,
                    Route::Gateway(*gw)
                ],
                "{gw:?}"
            );
            assert_eq!(f.tokens(), 2, "{gw:?}");
            assert_eq!(f.server.gw_requests(*gw).len(), 2, "{gw:?}");
            let saved: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert!(saved.get(*member).is_some(), "{gw:?}: {saved}");
            assert_eq!(mode(Path::new(&path)), 0o600);
            assert!(temporaries(&f.sandbox.tmp()).is_empty(), "{gw:?}");
            assert_eq!(
                out.stderr
                    .matches("Wrote the one-time credential to ")
                    .count(),
                1,
                "{gw:?}"
            );
            assert!(out.stdout.contains("saved_to"), "{gw:?}");
            // A 403: sent once, the grant hint, no file and no outcome line.
            let f = fx();
            let path = f.path("c.json");
            f.server.set_default(
                Route::Gateway(*gw),
                Planned::json(403, &json!({"message": "Forbidden"})),
            );
            let out = run_case(&f, &path);
            out.assert_code(1);
            assert_eq!(f.server.gw_requests(*gw).len(), 1, "{gw:?}");
            assert_eq!(f.tokens(), 1, "{gw:?}");
            assert!(out.stderr.starts_with(grant), "{gw:?}: {}", out.stderr);
            assert!(!out.stderr.contains("Outcome"), "{gw:?}: {}", out.stderr);
            assert!(!Path::new(&path).exists(), "{gw:?}");
            assert!(temporaries(&f.sandbox.tmp()).is_empty(), "{gw:?}");
            assert_eq!(out.stdout, "");
            // Applied, then the connection dropped: one request, no file, the outcome
            // unknown with the hint.
            let f = fx();
            let path = f.path("c.json");
            f.server
                .set_default(Route::Gateway(*gw), Planned::applied_then_dropped());
            let out = run_case(&f, &path);
            out.assert_code(1);
            assert_eq!(f.server.gw_requests(*gw).len(), 1, "{gw:?}");
            assert!(
                out.stderr
                    .ends_with(&format!("    {unknown}\n    {hint}\n\n")),
                "{gw:?}: {}",
                out.stderr
            );
            assert!(!Path::new(&path).exists(), "{gw:?}");
            assert!(temporaries(&f.sandbox.tmp()).is_empty(), "{gw:?}");
            assert_eq!(out.stdout, "");
        }
    }

    /// G36: a rotate answer without `key_transition_expires_at` fails its schema, yet the
    /// key is saved first; exit 1 with the saved-credential outcome line, stdout empty.
    #[test]
    fn a_schema_mismatched_2xx_still_saves_the_credential() {
        let f = fx();
        let path = f.path("rot.json");
        f.once(
            Gw::KeyRotate,
            Planned::json(
                200,
                &json!({"id": u(fx::SERVICE_KEY), "key": fx::marker(32)}),
            ),
        );
        let out = f.gw(&[
            "api-keys",
            "service",
            "rotate",
            &u(fx::SERVICE_KEY),
            "--secret-output",
            &path,
            "--force",
            "--transition-ms",
            "1800000",
        ]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert_eq!(
            out.stderr,
            error_block(
                "AISEC_RESPONSE_VALIDATION:Response did not match the expected schema",
                None,
                &[&format!(
                    "Outcome: the service API key {} was rotated and its one-time credential was saved in {path}, but the rest of the response could not be read.",
                    u(fx::SERVICE_KEY)
                )]
            )
        );
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains(&fx::marker(32))
        );
        assert_eq!(
            f.body(Gw::KeyRotate),
            json!({"key_transition_period_ms": 1_800_000})
        );
    }

    /// The destination appears between the reservation and the link: the credential is
    /// kept in the temporary (0600) and named, never overwriting the file.
    #[test]
    fn a_lost_race_keeps_the_temporary() {
        let f = fx();
        let path = f.path("race.json");
        f.once(Gw::KeyCreate, Planned::held());
        let args = key_args(&f, "service", &path);
        let refs: Vec<String> = gw(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .iter()
            .map(|arg| (*arg).to_owned())
            .collect();
        let refs: Vec<&str> = refs.iter().map(String::as_str).collect();
        let running = spawn(&rust(), &f.sandbox, &refs, &RunOptions::new());
        assert!(
            f.server
                .wait_for_route(Route::Gateway(Gw::KeyCreate), 1, Duration::from_secs(20))
        );
        std::fs::write(&path, "OTHER").unwrap();
        f.server.release_held();
        let out = running.wait();
        out.assert_code(1);
        assert!(
            out.stderr.contains(&format!(
                "The service API key was created, but {path} now exists; the credential is in "
            )),
            "{}",
            out.stderr
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "OTHER");
        let kept = temporaries(&f.sandbox.tmp());
        assert_eq!(kept.len(), 1);
        assert_eq!(mode(&kept[0]), 0o600);
        assert!(
            std::fs::read_to_string(&kept[0])
                .unwrap()
                .contains(&fx::marker(30))
        );
    }

    /// TS "gates and safely renders deployment auth rotation responses": a create saves
    /// `client_auth` and the password; a rotating update confirms and saves its new
    /// credentials; a plain update prints its answer withheld.
    #[test]
    fn deployments_create_and_rotate() {
        let f = fx();
        let path = f.path("dep.json");
        let out = f.gw(&[
            "deployments",
            "create",
            "--name",
            "edge",
            "--type",
            "production",
            "--auth-settings",
            "{\"gateway_base_url\":\"https://gw.example.test\"}",
            "--is-default",
            "false",
            "--secret-output",
            &path,
        ]);
        out.assert_success();
        assert_eq!(
            f.body(Gw::DeploymentCreate),
            json!({"name": "edge", "type": "production", "organisation_id": TSG, "auth_settings": {"gateway_base_url": "https://gw.example.test"}, "is_default": false})
        );
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved, fx::deployment_created());
        assert!(out.stderr.starts_with(&format!(
            "  ✓ Deployment registered: {}\n",
            u(fx::NEW_DEPLOYMENT)
        )));
        let f = fx();
        let path = f.path("rot.json");
        let out = f.gw(&[
            "deployments",
            "update",
            &u(fx::DEPLOYMENT),
            "--rotate-auth",
            "true",
            "--secret-output",
            &path,
            "--force",
        ]);
        out.assert_success();
        assert_eq!(f.body(Gw::DeploymentUpdate), json!({"rotate_auth": true}));
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains(&fx::marker(36))
        );
        assert!(out.stderr.starts_with(&format!(
            "  ✓ Deployment authentication rotated: {}\n",
            u(fx::DEPLOYMENT)
        )));
        // A plain update whose answer echoes credentials prints them withheld (D238).
        let f = fx();
        f.once(
            Gw::DeploymentUpdate,
            Planned::json(200, &fx::deployment_rotated()),
        );
        let out = f.gw(&[
            "deployments",
            "update",
            &u(fx::DEPLOYMENT),
            "--name",
            "edge",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            out.json(),
            json!({"id": u(fx::DEPLOYMENT), "success": true, "client_auth": "[withheld]", "credentials": "[withheld]"})
        );
    }

    /// The credential never reaches the debug log; the raw writes log `custom.request`
    /// (G36, G45); the typed writes their SDK operation names.
    #[test]
    fn the_debug_log_records_operations_and_no_body() {
        for (args, operation) in [
            (
                vec![
                    "configs".to_owned(),
                    "delete".into(),
                    u(fx::CONFIG),
                    "--force".into(),
                ],
                "airs.gateway.configs.delete",
            ),
            (
                vec![
                    "providers".to_owned(),
                    "update".into(),
                    u(fx::PROVIDER),
                    "--name".into(),
                    "p".into(),
                ],
                "airs.gateway.providers.update",
            ),
            (
                vec![
                    "admin-guardrails".to_owned(),
                    "mcp-servers".into(),
                    "sync".into(),
                    u(fx::ADMIN_GUARDRAIL),
                    "--set".into(),
                    format!("mcp_servers.{}.run_on=[\"input\"]", u(fx::MCP_SERVER)),
                    "--force".into(),
                ],
                "airs.gateway.admin_guardrails.sync_mcp_servers",
            ),
            (
                vec![
                    "scopes".to_owned(),
                    "create".into(),
                    "--name".into(),
                    "ws_new_aaaaaa".into(),
                ],
                "shared.iam.scopes.create",
            ),
            (
                vec![
                    "api-keys".to_owned(),
                    "service".into(),
                    "rotate".into(),
                    u(fx::SERVICE_KEY),
                    "--secret-output".into(),
                    "tmp/x".into(),
                    "--force".into(),
                ],
                "custom.request",
            ),
        ] {
            let f = fx();
            let mut args = args;
            if let Some(slot) = args.iter_mut().find(|arg| *arg == "tmp/x") {
                *slot = f.path("rotated.json");
            }
            let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
            let mut full = vec!["--debug", "gateway"];
            full.extend(args.iter().map(String::as_str));
            let out = run(
                &rust(),
                &f.sandbox,
                &full,
                &RunOptions::new().env("XDG_STATE_HOME", &state),
            );
            out.assert_success();
            let (path, records) = debug_log::only_log(&f.sandbox);
            let text = std::fs::read_to_string(&path).unwrap();
            for absent in ["run_on", "ws_new_aaaaaa", "key_transition", "\"name\""] {
                assert!(!text.contains(absent), "{args:?}: {absent}");
            }
            assert!(
                records
                    .iter()
                    .any(|record| record.to_string().contains(operation)),
                "{args:?}: {operation} not in {text}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// workspaces create|update|archive (§19.6; TS `aigateway-workspace-provision.spec.ts`,
// `unit/airs/aigateway.spec.ts`; D229, D235)
// ---------------------------------------------------------------------------

mod workspaces {
    use super::*;

    const SCOPE: &str = "ws_truffles_bx7qw1";
    const BANNER: &str = "\n  Prisma AIRS — AI Gateway\n  Gateway workspace operations\n\n";

    fn created_id() -> String {
        u(4)
    }

    fn create(f: &Fx, extra: &[&str]) -> Run {
        let mut args = vec![
            "workspaces",
            "create",
            "--name",
            "Truffles",
            "--scope-name",
            SCOPE,
        ];
        args.extend_from_slice(extra);
        f.gw(&args)
    }

    /// The four steps in order with their bodies (G37): the scope, the workspace, the
    /// bind (read + PUT), the read-back; stdout the detail (banner in pretty); stderr the
    /// TS acknowledgement and status line (D230).
    #[test]
    fn create_provisions_scope_workspace_binding_and_read_back() {
        let f = fx();
        let out = create(
            &f,
            &[
                "--description",
                "Recipes",
                "--users",
                "u1, u2,,u3",
                "--metadata",
                "{\"env\":\"production\"}",
            ],
        );
        out.assert_success();
        let slug = "ws-truffl-000004";
        assert_eq!(
            f.api(),
            [
                format!("POST {IAM}/scopes"),
                format!("POST {ADMIN}/workspaces"),
                format!("GET {IAM}/scopes/{SCOPE}"),
                format!("PUT {IAM}/scopes/{SCOPE}"),
                format!("GET {ADMIN}/workspaces/{}", created_id()),
            ]
        );
        assert_eq!(
            f.body(Gw::ScopeCreate),
            json!({"description": "Recipes", "name": SCOPE, "resources": []})
        );
        assert_eq!(
            f.body(Gw::WorkspaceCreate),
            json!({"name": "Truffles", "scope_name": SCOPE, "description": "Recipes", "users": ["u1", "u2", "u3"], "defaults": {"metadata": {"env": "production"}}})
        );
        assert_eq!(
            f.body(Gw::ScopeUpdate),
            json!({"description": "Recipes", "name": SCOPE, "resources": [{"metadata": [], "resource_id": slug, "resource_type": "workspace"}]})
        );
        assert!(
            out.stdout
                .starts_with(&format!("{BANNER}\n  Workspace Detail:\n\n  ID    ")),
            "{}",
            out.stdout
        );
        assert!(
            out.stdout.contains(&format!("  Slug         {slug}\n")) || out.stdout.contains(slug)
        );
        assert_eq!(
            out.stderr,
            format!(
                "  ✓ Workspace created: {}\n  IAM scope {SCOPE} created and bound to workspace {slug}. Grant that scope to the service accounts that should reach the workspace on the data plane.\n",
                created_id()
            )
        );
        // The scope now binds the workspace (the fixture state).
        let scopes = f.server.gateway_data().scopes;
        assert!(scopes.iter().any(|scope| scope["name"] == SCOPE && scope["resources"][0]["resource_id"] == slug));
    }

    /// TS "provisions with a generated scope when --scope-name is omitted": `ws_<stem>_<6>`.
    #[test]
    fn a_generated_scope_name() {
        let f = fx();
        let out = f.gw(&[
            "workspaces",
            "create",
            "--name",
            "Truffles Two",
            "--output",
            "json",
        ]);
        out.assert_success();
        let scope = f.body(Gw::ScopeCreate)["name"].as_str().unwrap().to_owned();
        let suffix = scope.strip_prefix("ws_truffles_two_").unwrap();
        assert_eq!(suffix.len(), 6);
        assert!(
            suffix
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        );
        assert_eq!(f.body(Gw::WorkspaceCreate)["scope_name"], scope.as_str());
        assert_eq!(out.json()["scopeName"], scope.as_str());
    }

    /// TS "passes --scope-name and --existing-scope through and reports a reused scope";
    /// "rejects --existing-scope without --scope-name … before any call"; the unrelated
    /// scope warning on stderr (D230).
    #[test]
    fn an_existing_scope_and_the_checks() {
        let f = fx();
        let out = f.gw(&[
            "workspaces",
            "create",
            "--name",
            "Production Two",
            "--scope-name",
            "ws_orphan_zzzzzz",
            "--existing-scope",
        ]);
        out.assert_success();
        assert!(f.server.gw_requests(Gw::ScopeCreate).is_empty());
        assert!(
            out.stderr
                .contains("  IAM scope ws_orphan_zzzzzz bound to workspace "),
            "{}",
            out.stderr
        );
        assert!(out.stderr.starts_with(
            "  ⚠ --scope-name 'ws_orphan_zzzzzz' shares no token with --name 'Production Two'. Operators find workspaces by scope in SCM Access Management; an unrelated name is easy to lose.\n"
        ));
        assert_usage(
            &["workspaces", "create", "--name", "x", "--existing-scope"],
            "--existing-scope requires --scope-name",
        );
        assert_usage(
            &[
                "workspaces",
                "create",
                "--name",
                "x",
                "--scope-name",
                "bad name",
            ],
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid name: bad name (expected an IAM scope name such as ws_production_bx7qw0)",
        );
        assert_usage(
            &["workspaces", "create", "--name", "x", "--defaults", "[1]"],
            "--defaults must be a JSON object",
        );
        assert_usage(
            &["workspaces", "create", "--name", "x", "--defaults", "{"],
            "--defaults must be valid JSON",
        );
        assert_usage(
            &[
                "workspaces",
                "create",
                "--name",
                "x",
                "--usage-limits",
                "[{\"type\":\"FAKE-GW-SECRET-37\"}]",
            ],
            "Invalid --usage-limits: expected a JSON array of usage-limit policies",
        );
        assert_usage(
            &["workspaces", "update", "w"],
            "Specify at least one of --name --description --icon --metadata --defaults --usage-limits --rate-limits",
        );
        assert_usage(
            &["workspaces", "update", "w", "--name", ""],
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Request body for PUT /workspaces/w did not match schema",
        );
    }

    /// D229: the outcome of each step, the scope deleted again only after a rejected
    /// workspace create, every outcome naming the scope and an `airs` remedy.
    #[test]
    fn provisioning_outcomes() {
        // Step 1 rejected: nothing else is sent.
        let f = fx();
        f.once(
            Gw::ScopeCreate,
            Planned::json(409, &json!({"message": "Scope exists"})),
        );
        let out = create(&f, &[]);
        out.assert_code(1);
        assert_eq!(f.api(), [format!("POST {IAM}/scopes")]);
        assert!(!out.stderr.contains("Outcome"));
        // Step 1 unknown.
        let f = fx();
        f.once(Gw::ScopeCreate, Planned::json(502, &json!({})));
        let out = create(&f, &[]);
        out.assert_code(1);
        assert!(out.stderr.contains(&format!(
            "    Outcome unknown: the IAM scope {SCOPE} may have been created.\n    Check it with 'airs gateway scopes get {SCOPE}'; reuse it with 'airs gateway workspaces create --existing-scope --scope-name {SCOPE} …' or delete it with 'airs gateway scopes delete {SCOPE}'.\n"
        )), "{}", out.stderr);
        assert_eq!(f.writes().len(), 1);
        // Step 2 rejected: the scope is deleted again.
        let f = fx();
        f.once(
            Gw::WorkspaceCreate,
            Planned::json(400, &json!({"message": "No update fields provided"})),
        );
        let out = create(&f, &[]);
        out.assert_code(1);
        assert_eq!(
            f.api(),
            [
                format!("POST {IAM}/scopes"),
                format!("POST {ADMIN}/workspaces"),
                format!("DELETE {IAM}/scopes/{SCOPE}")
            ]
        );
        assert!(out.stderr.ends_with(&format!(
            "    Outcome: the workspace was not created; IAM scope {SCOPE} was deleted again.\n\n"
        )), "{}", out.stderr);
        // Step 2 rejected and the delete fails: the scope is named with its remedy.
        let f = fx();
        f.once(Gw::WorkspaceCreate, Planned::json(400, &json!({})));
        f.once(Gw::ScopeDelete, Planned::json(405, &json!({})));
        let out = create(&f, &[]);
        out.assert_code(1);
        assert!(out.stderr.ends_with(&format!(
            "    Outcome: the workspace was not created; IAM scope {SCOPE} was created and is still present.\n    Delete it with 'airs gateway scopes delete {SCOPE}' or reuse it with --existing-scope.\n\n"
        )), "{}", out.stderr);
        // Step 2 redirected (a 3xx is a refusal whatever the SDK calls it): the workspace
        // was not created, so the scope is deleted again.
        let f = fx();
        f.once(
            Gw::WorkspaceCreate,
            Planned::redirect(307, "http://127.0.0.1:9/elsewhere"),
        );
        let out = create(&f, &[]);
        out.assert_code(1);
        assert_eq!(
            f.api(),
            [
                format!("POST {IAM}/scopes"),
                format!("POST {ADMIN}/workspaces"),
                format!("DELETE {IAM}/scopes/{SCOPE}")
            ]
        );
        assert!(out.stderr.ends_with(&format!(
            "    Outcome: the workspace was not created; IAM scope {SCOPE} was deleted again.\n\n"
        )), "{}", out.stderr);
        // Step 2 never sent (a local failure: the replayed 401's token request refused):
        // the scope is deleted again, or named with its remedy when that fails too.
        let f = fx();
        f.once(Gw::WorkspaceCreate, Planned::json(401, &json!({})));
        f.server.push_once(Route::Token, Planned::builtin());
        f.server.push_once(
            Route::Token,
            Planned::json(400, &json!({"error": "invalid_client"})),
        );
        let out = create(&f, &[]);
        out.assert_code(1);
        assert_eq!(f.server.gw_requests(Gw::WorkspaceCreate).len(), 1);
        assert_eq!(f.api()[2], format!("DELETE {IAM}/scopes/{SCOPE}"));
        assert!(out.stderr.ends_with(&format!(
            "    Outcome: the workspace was not created; IAM scope {SCOPE} was deleted again.\n\n"
        )), "{}", out.stderr);
        // Step 2 with --existing-scope rejected: no delete.
        let f = fx();
        f.once(Gw::WorkspaceCreate, Planned::json(400, &json!({})));
        let out = f.gw(&[
            "workspaces",
            "create",
            "--name",
            "Truffles",
            "--scope-name",
            "ws_orphan_zzzzzz",
            "--existing-scope",
        ]);
        out.assert_code(1);
        assert!(f.server.gw_requests(Gw::ScopeDelete).is_empty());
        // Step 2 unknown: no delete; the scope is kept (TS deletes it, D229).
        let f = fx();
        f.once(Gw::WorkspaceCreate, Planned::applied_then_dropped());
        let out = create(&f, &[]);
        out.assert_code(1);
        assert!(f.server.gw_requests(Gw::ScopeDelete).is_empty());
        assert_eq!(f.server.gw_requests(Gw::WorkspaceCreate).len(), 1);
        assert!(out.stderr.contains(&format!(
            "    Outcome unknown: the workspace may have been created; IAM scope {SCOPE} is kept.\n    Check 'airs gateway workspaces list --plane admin'; if the workspace exists, finish with 'airs gateway scopes bind {SCOPE} --workspace <slug or name>'; otherwise delete the scope with 'airs gateway scopes delete {SCOPE}'.\n"
        )), "{}", out.stderr);
        // Step 3 rejected: the workspace and the remedy are named (TS names an SDK method).
        let f = fx();
        f.once(
            Gw::ScopeUpdate,
            Planned::json(400, &json!({"message": "bind refused"})),
        );
        let out = create(&f, &[]);
        out.assert_code(1);
        assert!(out.stderr.contains(&format!(
            "    The workspace ws-truffl-000004 ({}) was created, but IAM scope {SCOPE} could not be bound to it.\n    Finish with 'airs gateway scopes bind {SCOPE} --workspace ws-truffl-000004'.\n",
            created_id()
        )), "{}", out.stderr);
        // Step 3 unknown.
        let f = fx();
        f.once(Gw::ScopeUpdate, Planned::json(503, &json!({})));
        let out = create(&f, &[]);
        out.assert_code(1);
        assert!(out.stderr.contains(&format!(
            "    Outcome unknown: IAM scope {SCOPE} may now bind workspace ws-truffl-000004 ({}).\n",
            created_id()
        )), "{}", out.stderr);
        // Step 4 fails: the acknowledgement, then the read-back outcome; stdout empty
        // (TS falls back to the create answer silently, choice 26).
        let f = fx();
        f.server.set_default(
            Route::Gateway(Gw::AdminWorkspaceGet),
            Planned::json(404, &json!({"message": "Workspace not found"})),
        );
        let out = create(&f, &[]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert!(
            out.stderr.starts_with(&format!(
                "  ✓ Workspace created: {}\n  IAM scope ",
                created_id()
            )),
            "{}",
            out.stderr
        );
        assert!(out.stderr.contains(&format!(
            "    Outcome: the workspace ws-truffl-000004 ({}) was created and bound to IAM scope {SCOPE}, but reading it back failed.\n",
            created_id()
        )), "{}", out.stderr);
    }

    /// TS "update re-read" with the D235 resolution (admin plane, a name to its slug), the
    /// block confirmation (D243) and the TS acknowledgement.
    #[test]
    fn update_resolves_confirms_blocks_and_reads_back() {
        let f = fx();
        let out = f.gw(&[
            "workspaces",
            "update",
            "Production",
            "--description",
            "Production workloads, us-east",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            f.api(),
            [
                format!("GET {ADMIN}/workspaces"),
                format!("PUT {ADMIN}/workspaces/ws-produc-985697"),
                format!("GET {ADMIN}/workspaces/ws-produc-985697"),
            ]
        );
        assert_eq!(
            f.body(Gw::WorkspaceUpdate),
            json!({"description": "Production workloads, us-east"})
        );
        assert_eq!(out.json()["description"], "Production workloads, us-east");
        assert_eq!(out.stderr, format!("  ✓ Workspace updated: {}\n", u(2)));
        // `--metadata` replaces `defaults` whole: a confirmation (refused off a terminal).
        assert_usage(
            &[
                "workspaces",
                "update",
                "Production",
                "--metadata",
                "{\"env\":\"prod\"}",
            ],
            "refusing to replace the settings of workspace Production without --force in non-interactive mode",
        );
        let f = fx();
        f.gw(&[
            "workspaces",
            "update",
            "Production",
            "--metadata",
            "{\"env\":\"prod\"}",
            "--force",
        ])
        .assert_success();
        assert_eq!(
            f.body(Gw::WorkspaceUpdate),
            json!({"defaults": {"metadata": {"env": "prod"}}})
        );
        // An unknown reference is refused with nothing sent (TS PUTs it, D235).
        let f = fx();
        let out = f.gw(&["workspaces", "update", "NoSuch", "--name", "x"]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                "Workspace 'NoSuch' was not found on the admin plane; nothing was sent",
                None,
                &[]
            )
        );
        assert!(f.writes().is_empty());
        // A failing read-back after the PUT: the acknowledgement and the outcome line.
        let f = fx();
        f.server.set_default(
            Route::Gateway(Gw::AdminWorkspaceGet),
            Planned::json(500, &json!({})),
        );
        let out = f.gw(&["workspaces", "update", "ws-produc-985697", "--icon", "star"]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert!(
            out.stderr
                .starts_with(&format!("  ✓ Workspace updated: {}\n", u(2)))
        );
        assert!(out.stderr.contains(
            "    Outcome: the workspace ws-produc-985697 was updated, but reading it back failed.\n"
        ));
    }

    /// TS "archive without verify": the name resolved to its slug, one DELETE, no read;
    /// stdout empty; the acknowledgement names the ref as typed; the hidden `delete`
    /// warns on stderr (D230).
    #[test]
    fn archive_and_the_hidden_delete() {
        let f = fx();
        let out = f.gw(&[
            "workspaces",
            "archive",
            "Production",
            "--force",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            f.api(),
            [
                format!("GET {ADMIN}/workspaces"),
                format!("DELETE {ADMIN}/workspaces/ws-produc-985697")
            ]
        );
        assert_eq!(out.stdout, "");
        assert_eq!(
            out.stderr,
            "  ✓ Workspace archived: Production\n  This is a soft delete — the workspace remains visible via `workspaces list --plane admin --status archived`. A `get` on it now answers 404; that is expected.\n"
        );
        let f = fx();
        let out = f.gw(&["workspaces", "delete", "ws-produc-985697", "--force"]);
        out.assert_success();
        assert!(out.stderr.starts_with(
            "  ⚠ `gateway workspace delete` is deprecated because this operation archives; use `gateway workspaces archive`.\n  ✓ Workspace archived: ws-produc-985697\n"
        ));
        assert_usage(
            &["workspaces", "archive", "Production"],
            "refusing to archive workspace Production without --force in non-interactive mode",
        );
        let f = fx();
        f.once(Gw::WorkspaceArchive, Planned::json(500, &json!({})));
        let out = f.gw(&["workspaces", "archive", "ws-produc-985697", "--force"]);
        out.assert_code(1);
        assert_eq!(
            f.server.gw_requests(Gw::WorkspaceArchive).len(),
            1,
            "TS sends 6"
        );
        assert!(out.stderr.contains("    Outcome unknown: the workspace ws-produc-985697 may have been archived.\n    Check it with 'airs gateway workspaces list --plane admin --status archived'.\n"));
    }

    /// D254: an incomplete workspace list (`has_more: true`, or a `total` other than its
    /// rows) resolves only an exact id or slug; a name or a miss is refused with nothing
    /// sent (a second row with the name may sit beyond the answer), for every write and
    /// the telemetry leaves (TS resolves the visible row and archives it).
    #[test]
    fn d254_an_incomplete_list_resolves_only_an_id_or_slug() {
        let refusal = |reference: &str| {
            error_block(
                &format!(
                    "Workspace '{reference}' cannot be resolved: the workspace list is incomplete (the API offers no continuation); use the workspace slug or UUID; nothing was sent"
                ),
                None,
                &[],
            )
        };
        let incomplete: [&dyn Fn(&mut GatewayData); 2] = [
            &|data: &mut GatewayData| data.has_more = Some(true),
            &|data: &mut GatewayData| data.list_total = Some(json!(50)),
        ];
        let window = [
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
        ];
        for seed in incomplete {
            let by_name: Vec<(Vec<&str>, &str)> = vec![
                (
                    vec!["workspaces", "archive", "Production", "--force"],
                    "Production",
                ),
                (
                    vec![
                        "workspaces",
                        "update",
                        "Production",
                        "--description",
                        "d",
                        "--force",
                    ],
                    "Production",
                ),
                (
                    vec![
                        "scopes",
                        "bind",
                        "ws_orphan_zzzzzz",
                        "--workspace",
                        "Main A",
                    ],
                    "Main A",
                ),
                (vec!["workspaces", "archive", "NoSuch", "--force"], "NoSuch"),
                (vec!["telemetry", "cost", "--workspace", "Main A"], "Main A"),
                (
                    [
                        &["telemetry", "requests", "--workspace", "Main A"][..],
                        &window,
                    ]
                    .concat(),
                    "Main A",
                ),
            ];
            for (args, reference) in by_name {
                let f = fx();
                f.data(seed);
                let out = f.gw(&args);
                out.assert_code(1);
                assert_eq!(out.stdout, "", "{args:?}");
                assert_eq!(out.stderr, refusal(reference), "{args:?}");
                assert!(f.writes().is_empty(), "{args:?}");
                // The writes read the admin list; a telemetry leaf reads its three lists
                // for an exact id or slug before it gives up on the name (choice 127).
                let lists = if args[0] == "telemetry" { 3 } else { 1 };
                assert_eq!(f.api().len(), lists, "list reads only: {args:?}");
            }
            // An exact slug or id still resolves on the incomplete list.
            let f = fx();
            f.data(seed);
            f.gw(&["workspaces", "archive", "ws-produc-985697", "--force"])
                .assert_success();
            assert_eq!(
                f.api()[1],
                format!("DELETE {ADMIN}/workspaces/ws-produc-985697")
            );
            let f = fx();
            f.data(seed);
            f.gw(&["workspaces", "archive", &u(2), "--force"])
                .assert_success();
            assert_eq!(
                f.api()[1],
                format!("DELETE {ADMIN}/workspaces/ws-produc-985697")
            );
            let f = fx();
            f.data(seed);
            let args = [
                &["telemetry", "requests", "--workspace", "ws-main-a-349e0e"][..],
                &window,
            ]
            .concat();
            f.gw(&args).assert_success();
        }
    }

    /// A matched workspace whose list row carries an empty slug (the SDK's schema requires
    /// the member, so only `""` decodes) is refused with nothing sent, never sent as an
    /// empty path segment.
    #[test]
    fn a_matched_workspace_without_a_slug_is_refused() {
        let window = [
            "--start",
            "2026-10-01T00:00:00Z",
            "--end",
            "2026-10-02T00:00:00Z",
        ];
        let production = u(2);
        for args in [
            vec!["workspaces", "archive", "Production", "--force"],
            vec![
                "workspaces",
                "update",
                &production,
                "--description",
                "d",
                "--force",
            ],
            vec!["telemetry", "cost", "--workspace", "Production"],
            [
                &["telemetry", "errors", "--workspace", &production][..],
                &window,
            ]
            .concat(),
        ] {
            let f = fx();
            f.data(|data| {
                for row in &mut data.workspaces {
                    if row["slug"] == "ws-produc-985697" {
                        row["slug"] = json!("");
                    }
                }
            });
            let out = f.gw(&args);
            out.assert_code(1);
            assert_eq!(out.stdout, "", "{args:?}");
            let reference = args[if args[0] == "telemetry" { 3 } else { 2 }];
            assert_eq!(
                out.stderr,
                error_block(
                    &format!(
                        "Workspace '{reference}' cannot be resolved: the matching workspace carries no slug in the list answer; nothing was sent"
                    ),
                    None,
                    &[],
                ),
                "{args:?}"
            );
            assert!(f.writes().is_empty(), "{args:?}");
            assert!(
                f.api()
                    .iter()
                    .all(|line| line.starts_with("GET ") && line.contains("/workspaces")),
                "list reads only: {:?}",
                f.api()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// scopes create|bind|delete (§19.7, D236)
// ---------------------------------------------------------------------------

mod scopes {
    use super::*;

    /// TS "creates an unbound scope and says it still needs binding"; the name is checked
    /// (D236: TS sends any name).
    #[test]
    fn create_checks_the_name_and_prints_the_scope() {
        let f = fx();
        let out = f.gw(&[
            "scopes",
            "create",
            "--name",
            "ws_new_aaaaaa",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            f.body(Gw::ScopeCreate),
            json!({"description": "", "name": "ws_new_aaaaaa", "resources": []})
        );
        assert_eq!(
            out.json(),
            json!({"name": "ws_new_aaaaaa", "description": "", "resources": [], "tsgId": TSG, "id": format!("ws_new_aaaaaa:{TSG}")})
        );
        assert_eq!(
            out.stderr,
            "  ✓ IAM scope created: ws_new_aaaaaa\n  The scope is not bound to anything yet — `workspaces create --existing-scope` or `scopes bind` finishes the job.\n"
        );
        assert_usage(
            &["scopes", "create", "--name", "bad name"],
            "AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid name: bad name (expected an IAM scope name such as ws_production_bx7qw0)",
        );
    }

    /// TS "binds a workspace ref to a scope" with the D235 resolution and the D236
    /// verification read; an unknown workspace is refused with nothing sent (TS binds the
    /// non-existent slug); a binding a concurrent writer replaced is an error.
    #[test]
    fn bind_resolves_and_verifies() {
        let f = fx();
        let out = f.gw(&[
            "scopes",
            "bind",
            "ws_orphan_zzzzzz",
            "--workspace",
            "Main A",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(
            f.api(),
            [
                format!("GET {ADMIN}/workspaces"),
                format!("GET {IAM}/scopes/ws_orphan_zzzzzz"),
                format!("PUT {IAM}/scopes/ws_orphan_zzzzzz"),
                format!("GET {IAM}/scopes/ws_orphan_zzzzzz"),
            ]
        );
        assert_eq!(
            f.body(Gw::ScopeUpdate)["resources"],
            json!([{"metadata": [], "resource_id": "ws-main-a-349e0e", "resource_type": "workspace"}])
        );
        assert_eq!(
            out.stderr,
            "  ✓ IAM scope ws_orphan_zzzzzz now binds 1 resource(s)\n"
        );
        assert_eq!(
            out.json()["resources"],
            json!([{"resourceType": "workspace", "resourceId": "ws-main-a-349e0e"}])
        );
        let f = fx();
        let out = f.gw(&[
            "scopes",
            "bind",
            "ws_orphan_zzzzzz",
            "--workspace",
            "NoSuch",
        ]);
        out.assert_code(1);
        assert!(
            out.stderr
                .contains("Workspace 'NoSuch' was not found on the admin plane; nothing was sent")
        );
        assert!(f.writes().is_empty());
        let f = fx();
        f.data(|data| data.scope_put_lost = true);
        let out = f.gw(&[
            "scopes",
            "bind",
            "ws_orphan_zzzzzz",
            "--workspace",
            "ws-main-a-349e0e",
        ]);
        out.assert_code(1);
        assert_eq!(
            out.stderr,
            error_block(
                "IAM scope ws_orphan_zzzzzz was updated, but it does not bind workspace ws-main-a-349e0e; another change may have replaced it. Check it with 'airs gateway scopes get ws_orphan_zzzzzz'.",
                None,
                &[]
            )
        );
    }

    /// TS "deletes a scope only with --force in a non-TTY run" and "refuses to delete
    /// without confirmation"; `--force` reads nothing first (choice 27).
    #[test]
    fn delete_with_force_and_the_refusal() {
        let f = fx();
        let out = f.gw(&["scopes", "rm", "ws_orphan_zzzzzz", "--force"]);
        out.assert_success();
        assert_eq!(f.api(), [format!("DELETE {IAM}/scopes/ws_orphan_zzzzzz")]);
        assert_eq!(out.stdout, "");
        assert_eq!(out.stderr, "  ✓ IAM scope deleted: ws_orphan_zzzzzz\n");
        assert_usage(
            &["scopes", "delete", "ws_orphan_zzzzzz"],
            "refusing to delete IAM scope ws_orphan_zzzzzz without --force in non-interactive mode",
        );
    }
}

// ---------------------------------------------------------------------------
// Integrations: provider slugs, custom hosts and the credential inputs (§19.12; TS
// "integration credentials, provider slugs, and custom hosts"; D247)
// ---------------------------------------------------------------------------

mod integrations {
    use super::*;

    const KEY: &str = "FAKE-GW-SECRET-38";

    /// TS "resolves --ai-provider slugs through the catalog and maps --base-url/--header to
    /// the verified custom-host shape": one catalog read after the token, before the POST;
    /// the inline-key and header warnings on stderr (D247).
    #[test]
    fn slugs_resolve_over_the_catalog_and_custom_hosts_map() {
        let f = fx();
        let key_file = f.file("provider.key", format!("{KEY}\n").as_bytes());
        let out = f.gw(&[
            "integrations",
            "create",
            "--ai-provider",
            "OPENAI",
            "--name",
            "i1",
            "--slug",
            "i1",
            "--base-url",
            "http://127.0.0.1:9/v1",
            "--header",
            "x-tenant=FAKE-GW-SECRET-39",
            "--header",
            "x-b=2",
            "--key-file",
            &key_file,
        ]);
        out.assert_success();
        assert_eq!(
            f.api(),
            [
                format!("GET {ADMIN}/utils/static-resources/ai-providers"),
                format!("POST {ADMIN}/integrations")
            ]
        );
        let catalog = f.server.gateway_data().inventory[&Gw::CatalogProviders].clone();
        let id = catalog["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["slug"].as_str().unwrap().eq_ignore_ascii_case("openai"))
            .unwrap()["id"]
            .clone();
        assert_eq!(
            f.body(Gw::IntegrationCreate),
            json!({
                "organisation_id": TSG,
                "ai_provider_id": id,
                "name": "i1",
                "slug": "i1",
                "configurations": {"provider_auth_type": "apiKey", "custom_host": "http://127.0.0.1:9/v1", "custom_headers": {"x-tenant": "FAKE-GW-SECRET-39", "x-b": "2"}},
                "key": KEY,
            })
        );
        assert!(out.stderr.starts_with(
            "  ⚠ Inline --header is visible in shell history and process listings; prefer a --file request body (mode 0600)\n"
        ), "{}", out.stderr);
        assert!(out.stderr.ends_with(&format!(
            "  ✓ Integration created: {}\n",
            u(fx::NEW_INTEGRATION)
        )));
        // An unknown slug: the CLI's message with suggestions; nothing written.
        let f = fx();
        let out = f.gw(&[
            "integrations",
            "create",
            "--ai-provider",
            "opnai",
            "--key",
            "k",
            "--name",
            "i",
            "--slug",
            "i",
        ]);
        out.assert_code(1);
        assert!(
            out.stderr.contains("Unknown provider 'opnai'"),
            "{}",
            out.stderr
        );
        assert!(
            out.stderr
                .contains("(list slugs with 'airs gateway integrations providers')")
        );
        assert!(out.stderr.contains("  ⚠ Inline --key is visible in shell history and process listings; prefer --key-stdin, --key-file, or --secret-mappings\n"));
        assert!(f.writes().is_empty());
        // A UUID is lowercased and used without a catalog read.
        let f = fx();
        f.gw(&[
            "integrations",
            "create",
            "--ai-provider",
            &u(fx::CATALOG_PROVIDER).to_uppercase(),
            "--key",
            "k",
            "--name",
            "i",
            "--slug",
            "i",
        ])
        .assert_success();
        assert_eq!(f.api(), [format!("POST {ADMIN}/integrations")]);
        assert_eq!(
            f.body(Gw::IntegrationCreate)["ai_provider_id"],
            u(fx::CATALOG_PROVIDER).as_str()
        );
    }

    /// TS "reads a piped credential with --key-stdin outside a terminal", "explains the
    /// pipe form", "refuses a credential-less create before any request" (Rust: before the
    /// catalog too, D247) and "accepts secret mappings as the credential".
    #[test]
    fn credential_sources() {
        let f = fx();
        let out = f.gw_with(
            &[
                "integrations",
                "create",
                "--ai-provider-id",
                &u(fx::CATALOG_PROVIDER),
                "--name",
                "i",
                "--slug",
                "i",
                "--key-stdin",
            ],
            &RunOptions::new().stdin(format!("{KEY}\n")),
        );
        out.assert_success();
        assert_eq!(f.body(Gw::IntegrationCreate)["key"], KEY);
        let f = bare();
        let out = f.gw_with(
            &[
                "integrations",
                "create",
                "--ai-provider-id",
                &u(fx::CATALOG_PROVIDER),
                "--key-stdin",
            ],
            &RunOptions::new().stdin(b"a\nb\n".to_vec()),
        );
        out.assert_code(2);
        assert_eq!(
            out.stderr,
            usage_block(
                "--key-stdin reads one credential from piped stdin, e.g. 'airs gateway integrations create ... --key-stdin < provider.key'; in a terminal, omit every key flag to be prompted"
            )
        );
        assert_usage(
            &[
                "integrations",
                "create",
                "--ai-provider",
                "openai",
                "--name",
                "i",
            ],
            "A provider credential is required: run in a terminal to be prompted, or pass --key-file, --key-stdin (piped), --secret-mappings, or --key (the gateway rejects a credential-less integration with a generic AB01)",
        );
        let f = bare();
        for (bytes, message) in [
            (
                &b"\n"[..],
                "--key-file must contain exactly one nonempty credential",
            ),
            (
                b"a\nb",
                "--key-file must contain exactly one nonempty credential",
            ),
        ] {
            let path = f.file("bad.key", bytes);
            let out = f.gw(&[
                "integrations",
                "create",
                "--ai-provider-id",
                &u(fx::CATALOG_PROVIDER),
                "--key-file",
                &path,
            ]);
            out.assert_code(2);
            assert_eq!(out.stderr, usage_block(message));
        }
        let missing = f.path("missing.key");
        let out = f.gw(&["integrations", "create", "--key-file", &missing]);
        assert_eq!(
            out.stderr,
            usage_block(&format!("Cannot read --key-file {missing}"))
        );
        let f = fx();
        let out = f.gw(&[
            "integrations",
            "create",
            "--ai-provider-id",
            &u(fx::CATALOG_PROVIDER),
            "--name",
            "i",
            "--slug",
            "i",
            "--secret-mappings",
            "[{\"target_field\":\"key\",\"secret_reference_id\":\"ref-1\"}]",
        ]);
        out.assert_success();
        assert_eq!(
            f.body(Gw::IntegrationCreate)["secret_mappings"][0]["secret_reference_id"],
            "ref-1"
        );
    }

    /// TS "prompts for the key with hidden input in a terminal when no credential flag is
    /// given": the masked prompt before the tenant; the key reaches only the body.
    #[test]
    fn the_hidden_key_prompt() {
        let f = fx();
        let catalog = u(fx::CATALOG_PROVIDER);
        let args = gw(&[
            "integrations",
            "create",
            "--ai-provider-id",
            &catalog,
            "--name",
            "i",
            "--slug",
            "i",
        ]);
        let mut session = spawn_pty(&rust(), &f.sandbox, &args, &RunOptions::new());
        session.expect("? Provider API key: [input is masked] ");
        assert_eq!(
            f.server.request_count(),
            0,
            "the prompt comes before the tenant"
        );
        session.send(format!("{KEY}\n").as_bytes());
        let out = session.wait();
        out.run.assert_success();
        assert!(!transcript(&out).contains(KEY));
        assert_eq!(f.body(Gw::IntegrationCreate)["key"], KEY);
        // `--key-stdin` on a terminal is refused (TS falls back to the prompt, D247).
        let f = fx();
        let args = gw(&[
            "integrations",
            "create",
            "--ai-provider-id",
            &catalog,
            "--key-stdin",
        ]);
        let session = spawn_pty(&rust(), &f.sandbox, &args, &RunOptions::new());
        let out = session.wait();
        out.run.assert_code(2);
        assert!(transcript(&out).contains("--key-stdin reads one credential from piped stdin"));
    }

    /// TS "updates the custom host without touching the credential" (no prompt); a
    /// credential-replacing update confirms (D243).
    #[test]
    fn updates_confirm_only_credential_replacements() {
        let f = fx();
        let out = f.gw(&[
            "integrations",
            "update",
            &u(fx::INTEGRATION),
            "--base-url",
            "https://llm.example.test/v1",
        ]);
        out.assert_success();
        assert_eq!(
            f.body(Gw::IntegrationUpdate),
            json!({"configurations": {"provider_auth_type": "apiKey", "custom_host": "https://llm.example.test/v1"}})
        );
        let f = bare();
        let out = f.gw(&["integrations", "update", &u(fx::INTEGRATION), "--key", "k"]);
        out.assert_code(2);
        assert_eq!(
            out.stderr,
            format!(
                "  ⚠ Inline --key is visible in shell history and process listings; prefer --key-stdin, --key-file, or --secret-mappings\n{}",
                usage_block(&format!(
                    "refusing to replace the provider credential of integration {} without --force in non-interactive mode",
                    u(fx::INTEGRATION)
                ))
            )
        );
        assert_usage(
            &[
                "mcp",
                "integrations",
                "update",
                &u(fx::MCP_INTEGRATION),
                "--configurations",
                "{\"client_secret\":\"x\"}",
            ],
            &format!(
                "refusing to replace the authentication settings of MCP integration {} without --force in non-interactive mode",
                u(fx::MCP_INTEGRATION)
            ),
        );
        let f = fx();
        f.gw(&[
            "integrations",
            "update",
            &u(fx::INTEGRATION),
            "--key",
            "k",
            "--force",
        ])
        .assert_success();
        assert_eq!(f.body(Gw::IntegrationUpdate), json!({"key": "k"}));
    }

    /// D243, choices 174 and 178: a `deployments update` that replaces the deployment's
    /// `auth_settings` or archives it (`--status archived`, or `status` through `--set-string`)
    /// and an MCP update that sets `auth_type` confirm, refused before the tenant without
    /// `--force` in non-interactive mode, and sent once with it.
    #[test]
    fn d243_authentication_replacing_updates_confirm() {
        for (args, action) in [
            (
                vec![
                    "deployments",
                    "update",
                    &u(fx::DEPLOYMENT)[..],
                    "--auth-settings",
                    "{\"gateway_base_url\":\"https://gw.example.test\"}",
                ],
                format!(
                    "replace the authentication settings of deployment {}",
                    u(fx::DEPLOYMENT)
                ),
            ),
            (
                vec![
                    "deployments",
                    "update",
                    &u(fx::DEPLOYMENT)[..],
                    "--set",
                    "auth_settings.gateway_base_url=https://gw.example.test",
                ],
                format!(
                    "replace the authentication settings of deployment {}",
                    u(fx::DEPLOYMENT)
                ),
            ),
            (
                vec![
                    "mcp",
                    "integrations",
                    "update",
                    &u(fx::MCP_INTEGRATION)[..],
                    "--auth-type",
                    "none",
                ],
                format!(
                    "replace the authentication settings of MCP integration {}",
                    u(fx::MCP_INTEGRATION)
                ),
            ),
            (
                vec![
                    "mcp",
                    "integrations",
                    "update",
                    &u(fx::MCP_INTEGRATION)[..],
                    "--set",
                    "auth_type=none",
                ],
                format!(
                    "replace the authentication settings of MCP integration {}",
                    u(fx::MCP_INTEGRATION)
                ),
            ),
            // Choice 178: an update that archives the deployment confirms as the archive does.
            (
                vec![
                    "deployments",
                    "update",
                    &u(fx::DEPLOYMENT)[..],
                    "--status",
                    "archived",
                ],
                format!("archive deployment {}", u(fx::DEPLOYMENT)),
            ),
            (
                vec![
                    "deployments",
                    "update",
                    &u(fx::DEPLOYMENT)[..],
                    "--name",
                    "edge",
                    "--set-string",
                    "status=archived",
                ],
                format!("archive deployment {}", u(fx::DEPLOYMENT)),
            ),
        ] {
            assert_usage(
                &args,
                &format!("refusing to {action} without --force in non-interactive mode"),
            );
        }
        let f = fx();
        f.gw(&[
            "deployments",
            "update",
            &u(fx::DEPLOYMENT),
            "--auth-settings",
            "{\"gateway_base_url\":\"https://gw.example.test\"}",
            "--force",
        ])
        .assert_success();
        assert_eq!(
            f.body(Gw::DeploymentUpdate),
            json!({"auth_settings": {"gateway_base_url": "https://gw.example.test"}})
        );
        let f = fx();
        f.gw(&[
            "mcp",
            "integrations",
            "update",
            &u(fx::MCP_INTEGRATION),
            "--auth-type",
            "none",
            "--force",
        ])
        .assert_success();
        assert_eq!(f.body(Gw::McpUpdate), json!({"auth_type": "none"}));
        let f = fx();
        f.gw(&[
            "deployments",
            "update",
            &u(fx::DEPLOYMENT),
            "--status",
            "archived",
            "--force",
        ])
        .assert_success();
        assert_eq!(f.body(Gw::DeploymentUpdate), json!({"status": "archived"}));
        assert_eq!(f.writes().len(), 1);
        // A description change still runs unasked.
        let f = fx();
        f.gw(&[
            "mcp",
            "integrations",
            "update",
            &u(fx::MCP_INTEGRATION),
            "--description",
            "d",
        ])
        .assert_success();
    }

    /// TS "replaces integration workspace bindings by default and preserves only when
    /// requested", "passes MCP workspace replacement bodies exactly" and "builds MCP
    /// workspace replacement bodies from repeatable bindings"; the model and capability
    /// bindings.
    #[test]
    fn replacement_bodies() {
        for (args, route, expected) in [
            (
                vec![
                    "integrations".to_owned(),
                    "workspaces".into(),
                    "set".into(),
                    u(fx::INTEGRATION),
                    "--workspace-binding".into(),
                    "ws-main-a-349e0e=true".into(),
                    "--global-access".into(),
                    "false".into(),
                    "--force".into(),
                ],
                Gw::IntegrationWorkspacesSet,
                json!({"workspaces": [{"id": "ws-main-a-349e0e", "enabled": true}], "global_workspace_access": {"enabled": false}, "override_existing_workspace_access": true}),
            ),
            (
                vec![
                    "integrations".to_owned(),
                    "workspaces".into(),
                    "set".into(),
                    u(fx::INTEGRATION),
                    "--workspace-binding".into(),
                    "ws-main-a-349e0e=false".into(),
                    "--preserve-existing".into(),
                    "--force".into(),
                ],
                Gw::IntegrationWorkspacesSet,
                json!({"workspaces": [{"id": "ws-main-a-349e0e", "enabled": false}], "override_existing_workspace_access": false}),
            ),
            (
                vec![
                    "mcp".to_owned(),
                    "integrations".into(),
                    "workspaces".into(),
                    "set".into(),
                    u(fx::MCP_INTEGRATION),
                    "--workspace-binding".into(),
                    "ws-a=true".into(),
                    "--workspace-binding".into(),
                    "ws-b=false".into(),
                    "--force".into(),
                ],
                Gw::McpWorkspacesSet,
                json!({"workspaces": [{"id": "ws-a", "enabled": true}, {"id": "ws-b", "enabled": false}], "override_existing_workspace_access": true}),
            ),
            (
                vec![
                    "integrations".to_owned(),
                    "models".into(),
                    "set".into(),
                    u(fx::INTEGRATION),
                    "--model".into(),
                    "gpt-4o=true".into(),
                    "--allow-all-models".into(),
                    "false".into(),
                    "--force".into(),
                ],
                Gw::ModelsSet,
                json!({"models": [{"slug": "gpt-4o", "enabled": true}], "allow_all_models": false}),
            ),
            (
                vec![
                    "mcp".to_owned(),
                    "integrations".into(),
                    "capabilities".into(),
                    "set".into(),
                    u(fx::MCP_INTEGRATION),
                    "--capability".into(),
                    "tool:search=true".into(),
                    "--force".into(),
                ],
                Gw::CapabilitiesSet,
                json!({"capabilities": [{"type": "tool", "name": "search", "enabled": true}]}),
            ),
        ] {
            let f = fx();
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            f.gw(&refs).assert_success();
            assert_eq!(f.body(route), expected, "{refs:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Organisation ids (D245)
// ---------------------------------------------------------------------------

mod organisations {
    use super::*;

    /// `--organisation-id` and `--tsg-id` default to the tenant's TSG; another value, from
    /// a flag or the body, is refused after the tenant with no request, no token.
    #[test]
    fn organisation_ids_default_to_the_tenant_and_refuse_others() {
        let f = fx();
        f.gw(&[
            "plugins",
            "create",
            "--integration-id",
            &u(fx::INTEGRATION),
            "--credential",
            "api_key=FAKE-GW-SECRET-38",
            "--credential",
            "region=us",
        ])
        .assert_success();
        assert_eq!(
            f.body(Gw::PluginCreate),
            json!({"credentials": {"api_key": "FAKE-GW-SECRET-38", "region": "us"}, "integration_id": u(fx::INTEGRATION), "organisation_id": TSG})
        );
        let f = fx();
        f.gw(&[
            "organisations",
            "auth-settings",
            "update",
            "--domains",
            "a.test, b.test",
            "--force",
        ])
        .assert_success();
        assert_eq!(
            f.api(),
            [format!("PUT {ADMIN}/organisations/{TSG}/auth-settings")]
        );
        assert_eq!(
            f.body(Gw::AuthSettingsUpdate),
            json!({"domains": ["a.test", "b.test"]})
        );
        for args in [
            vec![
                "deployments".to_owned(),
                "archive".into(),
                u(fx::DEPLOYMENT),
                "--organisation-id".into(),
                "999".into(),
                "--force".into(),
            ],
            vec![
                "mcp".to_owned(),
                "integrations".into(),
                "create".into(),
                "--name".into(),
                "m".into(),
                "--slug".into(),
                "m".into(),
                "--url".into(),
                "https://m.test".into(),
                "--auth-type".into(),
                "none".into(),
                "--transport".into(),
                "http".into(),
                "--set".into(),
                "organisation_id=\"999\"".into(),
            ],
        ] {
            let f = fx();
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = f.gw(&refs);
            out.assert_code(1);
            assert_eq!(
                out.stderr,
                error_block(
                    "--organisation-id 999 is not the selected tenant's TSG; airs reads and changes only the selected tenant's organisation",
                    None,
                    &[]
                )
            );
            assert_eq!(f.server.request_count(), 0, "{refs:?}");
        }
        let f = fx();
        let out = f.gw(&[
            "organisations",
            "auth-settings",
            "update",
            "--tsg-id",
            "999",
            "--scim-token",
            "FAKE-GW-SECRET-38",
            "--force",
        ]);
        out.assert_code(1);
        assert!(
            out.stderr
                .contains("--tsg-id 999 is not the selected tenant's TSG")
        );
        assert!(out.stderr.starts_with("  ⚠ Inline --scim-token is visible in shell history and process listings; prefer a --file request body (mode 0600)\n"));
        assert_eq!(f.server.request_count(), 0);
    }
}

// ---------------------------------------------------------------------------
// Confirmations (§19.3 "Confirmations", D243; TS "requires force or confirmation …",
// "uses an imperative non-interactive refusal message", the admin-guardrail spec)
// ---------------------------------------------------------------------------

mod confirmations {
    use super::*;

    /// Every confirming write: its arguments (no `--force`), its prompt and its route.
    fn cases(f: &Fx) -> Vec<(Vec<String>, String, Gw)> {
        let secret = f.path("pty.json");
        let s = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        vec![
            (s(&["configs", "delete", &u(fx::CONFIG)]), format!("Permanently delete config {}?", u(fx::CONFIG)), Gw::ConfigDelete),
            (s(&["api-keys", "service", "delete", &u(fx::SERVICE_KEY)]), format!("Revoke service API key {}?", u(fx::SERVICE_KEY)), Gw::KeyDelete),
            (
                s(&["api-keys", "service", "rotate", &u(fx::SERVICE_KEY), "--secret-output", &secret]),
                format!("Rotate service API key {}? The new key is written to {secret}.", u(fx::SERVICE_KEY)),
                Gw::KeyRotate,
            ),
            (
                s(&["providers", "update", &u(fx::PROVIDER), "--reset-usage", "true"]),
                format!("Reset the accumulated usage of provider {}?", u(fx::PROVIDER)),
                Gw::ProviderUpdate,
            ),
            (
                s(&["deployments", "update", &u(fx::DEPLOYMENT), "--rotate-auth", "true", "--secret-output", &secret]),
                format!("Rotate the authentication of deployment {}? Its current credentials stop working; the new ones are written to {secret}.", u(fx::DEPLOYMENT)),
                Gw::DeploymentUpdate,
            ),
            (
                s(&["integrations", "workspaces", "set", &u(fx::INTEGRATION), "--workspace-binding", "ws-a=true"]),
                format!("Replace workspace bindings on {}? Bindings not named are removed.", u(fx::INTEGRATION)),
                Gw::IntegrationWorkspacesSet,
            ),
            (
                s(&["integrations", "workspaces", "set", &u(fx::INTEGRATION), "--workspace-binding", "ws-a=true", "--preserve-existing"]),
                format!("Replace workspace bindings on {}?", u(fx::INTEGRATION)),
                Gw::IntegrationWorkspacesSet,
            ),
            // The prompt follows the body sent: a later `--set` overrides --preserve-existing.
            (
                s(&["integrations", "workspaces", "set", &u(fx::INTEGRATION), "--workspace-binding", "ws-a=true", "--preserve-existing", "--set", "override_existing_workspace_access=true"]),
                format!("Replace workspace bindings on {}? Bindings not named are removed.", u(fx::INTEGRATION)),
                Gw::IntegrationWorkspacesSet,
            ),
            (
                s(&["mcp", "integrations", "workspaces", "set", &u(fx::MCP_INTEGRATION), "--workspace-binding", "ws-a=true", "--preserve-existing", "--set", "override_existing_workspace_access=true"]),
                format!("Replace MCP workspace access on {}? Bindings not named are removed.", u(fx::MCP_INTEGRATION)),
                Gw::McpWorkspacesSet,
            ),
            (
                s(&["organisations", "auth-settings", "update", "--domains", "a.test"]),
                format!("Replace the authentication settings of organisation {TSG}? Single sign-on and SCIM provisioning use them at once."),
                Gw::AuthSettingsUpdate,
            ),
            (
                s(&["admin-guardrails", "mcp-servers", "sync", &u(fx::ADMIN_GUARDRAIL), "--set", &format!("mcp_servers.{}.run_on=[\"input\"]", u(fx::MCP_SERVER))]),
                format!("Replace all MCP mappings on admin guardrail {}?", u(fx::ADMIN_GUARDRAIL)),
                Gw::McpSync,
            ),
            (
                s(&["workspaces", "archive", "Production"]),
                "Archive workspace Production? (soft delete — the row remains under --status archived)".to_owned(),
                Gw::WorkspaceArchive,
            ),
            (
                s(&["workspaces", "update", "Production", "--defaults", "{}", "--usage-limits", "[]"]),
                "Replace the defaults and usage limits of workspace Production? The service replaces each named block whole.".to_owned(),
                Gw::WorkspaceUpdate,
            ),
            (
                s(&["scopes", "delete", "ws_production_bx7qw0"]),
                "Delete IAM scope ws_production_bx7qw0? Workspaces bound to it lose their data-plane grant. It binds 1 resource(s): workspace:ws-produc-985697.".to_owned(),
                Gw::ScopeDelete,
            ),
        ]
    }

    /// `y`/`yes` send the write; `n`, Enter, EOF and anything else decline with `ℹ Aborted`
    /// (exit 0, nothing sent, no `--secret-output` file and no temporary). No request is
    /// open while the prompt waits.
    #[test]
    fn every_prompt_confirms_only_yes() {
        let count = cases(&fx()).len();
        for index in 0..count {
            for answer in [&b"n\n"[..], b"\n", b"\x04", b"yep\n", b"y\n", b" YES \n"] {
                let f = fx();
                let (args, prompt, route) = cases(&f).remove(index);
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                let mut session = spawn_pty(&rust(), &f.sandbox, &gw(&refs), &RunOptions::new());
                session.expect(&format!("? {prompt} (y/N) "));
                assert!(f.writes().is_empty(), "no write while the prompt waits");
                session.send(answer);
                let out = session.wait();
                out.run.assert_success();
                let confirmed = answer.trim_ascii().eq_ignore_ascii_case(b"yes")
                    || answer.trim_ascii().eq_ignore_ascii_case(b"y");
                if confirmed {
                    assert_eq!(f.server.gw_requests(route).len(), 1, "{refs:?}");
                } else {
                    assert!(
                        transcript(&out).contains("  ℹ Aborted\n"),
                        "{refs:?} {answer:?}"
                    );
                    assert!(f.writes().is_empty(), "{refs:?}");
                    assert!(!f.sandbox.tmp().join("pty.json").exists());
                    assert!(temporaries(&f.sandbox.tmp()).is_empty());
                }
            }
        }
    }

    /// The non-interactive refusals (exit 2, before the tenant, no request; TS's
    /// imperative text for TS's prompts).
    #[test]
    fn non_interactive_refusals() {
        let f = bare();
        for (args, prompt, _) in cases(&f) {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            let out = f.gw(&refs);
            out.assert_code(2);
            assert!(
                out.stderr.contains("refusing to "),
                "{refs:?}: {}",
                out.stderr
            );
            assert!(
                out.stderr
                    .ends_with(" without --force in non-interactive mode\n\n")
            );
            let _ = prompt;
        }
        assert_eq!(f.server.request_count(), 0);
        assert_usage(
            &["admin-guardrails", "delete", &u(fx::ADMIN_GUARDRAIL)],
            &format!(
                "refusing to permanently delete admin guardrail {} without --force in non-interactive mode",
                u(fx::ADMIN_GUARDRAIL)
            ),
        );
        assert_usage(
            &[
                "integrations",
                "models",
                "set",
                &u(fx::INTEGRATION),
                "--model",
                "m=true",
            ],
            &format!(
                "refusing to replace model bindings on {} without --force in non-interactive mode",
                u(fx::INTEGRATION)
            ),
        );
    }

    /// Ctrl-C at a prompt: `Cancelled; nothing was changed.`, exit 130, nothing sent.
    #[test]
    fn a_signal_at_a_prompt_cancels() {
        let f = fx();
        let mut session = spawn_pty(
            &rust(),
            &f.sandbox,
            &gw(&["configs", "delete", &u(fx::CONFIG)]),
            &RunOptions::new(),
        );
        session.expect("(y/N) ");
        session.signal("INT");
        let out = session.wait();
        out.run.assert_code(130);
        assert!(transcript(&out).contains("Cancelled; nothing was changed."));
        assert!(f.writes().is_empty());
    }

    /// TS "creates organisation-scoped input", "updates and upserts through strict
    /// structured requests": creates, `upsert` and other updates never confirm (choice 28,
    /// 29).
    #[test]
    fn creates_and_upserts_do_not_confirm() {
        let f = fx();
        f.gw(&[
            "admin-guardrails",
            "mcp-servers",
            "upsert",
            &u(fx::ADMIN_GUARDRAIL),
            &u(fx::MCP_SERVER),
            "--set",
            "run_on=[\"output\"]",
        ])
        .assert_success();
        f.gw(&[
            "providers",
            "update",
            &u(fx::PROVIDER),
            "--reset-usage",
            "false",
        ])
        .assert_success();
        f.gw(&[
            "api-keys",
            "user",
            "update",
            &u(fx::USER_KEY),
            "--name",
            "n",
        ])
        .assert_success();
        assert_eq!(f.writes().len(), 3);
    }
}

// ---------------------------------------------------------------------------
// Interruption (D252)
// ---------------------------------------------------------------------------

mod interrupt {
    use super::*;

    fn spawn_gw(f: &Fx, args: &[&str]) -> airs_test_harness::Running {
        spawn(
            &rust(),
            &f.sandbox,
            &gw(args),
            &RunOptions::new().expect_signal(),
        )
    }

    /// A signal while a write is held: the change may have been applied; nothing is
    /// re-sent (TS exits with nothing printed).
    #[test]
    fn a_signal_during_a_write_reports_an_unknown_outcome() {
        for (signal, code) in [("INT", 130), ("TERM", 143)] {
            let f = fx();
            f.once(Gw::ConfigUpdate, Planned::held());
            let running = spawn_gw(&f, &["configs", "update", &u(fx::CONFIG), "--name", "n"]);
            assert!(f.server.wait_for_route(
                Route::Gateway(Gw::ConfigUpdate),
                1,
                Duration::from_secs(20)
            ));
            running.send_signal(signal);
            let out = running.wait();
            f.server.release_held();
            out.assert_code(code);
            assert_eq!(out.stdout, "");
            assert_eq!(
                out.stderr,
                "\n  ✗ Error: Interrupted; the change may already have been applied (outcome unknown)\n\n"
            );
            assert_eq!(f.server.gw_requests(Gw::ConfigUpdate).len(), 1);
        }
    }

    /// A signal before the write (the token request hangs): nothing was changed; a
    /// reserved `--secret-output` is removed (TS leaves an empty 0600 file).
    #[test]
    fn a_signal_before_the_write_changes_nothing() {
        let f = fx();
        let path = f.path("held.json");
        f.server.push_once(Route::Token, Planned::hang());
        let running = spawn_gw(
            &f,
            &[
                "api-keys",
                "service",
                "rotate",
                &u(fx::SERVICE_KEY),
                "--secret-output",
                &path,
                "--force",
            ],
        );
        assert!(
            f.server
                .wait_for_route(Route::Token, 1, Duration::from_secs(20))
        );
        running.send_signal("TERM");
        let out = running.wait();
        out.assert_code(143);
        assert_eq!(
            out.stderr,
            "\n  ✗ Error: Interrupted; nothing was changed\n\n"
        );
        assert!(f.writes().is_empty());
        assert!(!Path::new(&path).exists());
        assert!(temporaries(&f.sandbox.tmp()).is_empty());
    }

    /// A signal during a held one-time-credential write: outcome unknown, the
    /// reservation (holding nothing) removed.
    #[test]
    fn a_signal_during_a_secret_write_removes_the_reservation() {
        let f = fx();
        let path = f.path("held.json");
        f.once(Gw::KeyRotate, Planned::held());
        let running = spawn_gw(
            &f,
            &[
                "api-keys",
                "user",
                "rotate",
                &u(fx::USER_KEY),
                "--secret-output",
                &path,
                "--force",
            ],
        );
        assert!(
            f.server
                .wait_for_route(Route::Gateway(Gw::KeyRotate), 1, Duration::from_secs(20))
        );
        running.send_signal("INT");
        let out = running.wait();
        f.server.release_held();
        out.assert_code(130);
        assert!(out.stderr.contains("outcome unknown"), "{}", out.stderr);
        assert!(!Path::new(&path).exists());
        assert!(temporaries(&f.sandbox.tmp()).is_empty());
    }

    /// A signal once the answer's credential is saved, while the run is still reporting
    /// (its acknowledgement, with an id longer than any pipe buffer, blocks on a stalled
    /// stderr): the file is complete and the interruption names it (§19.11).
    #[test]
    fn a_signal_after_the_credential_is_saved_names_the_file() {
        let f = fx();
        let path = f.path("saved.json");
        let answer = json!({"id": format!("k-{}", "x".repeat(300_000)), "key": fx::marker(30), "object": "api-key"});
        f.once(
            Gw::KeyCreate,
            Planned::raw(200, "application/json", answer.to_string()),
        );
        let mut running = spawn(
            &rust(),
            &f.sandbox,
            &gw(&[
                "api-keys",
                "service",
                "create",
                "--name",
                "k",
                "--scopes",
                "mcp.invoke",
                "--type",
                "service",
                "--workspace",
                &u(1),
                "--secret-output",
                &path,
            ]),
            &RunOptions::new()
                .stall_stderr()
                .expect_signal()
                .signal_marker(&f.sandbox.tmp().join("signal.marker")),
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !Path::new(&path).exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "the file was never saved"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        // The run is blocked on stderr (or has not reached its acknowledgement); stderr is
        // released only once the signal thread has recorded the signal.
        running.send_signal_recorded("INT");
        running.release_stderr();
        let out = running.wait();
        out.assert_code(130);
        assert_eq!(out.stdout, "");
        assert!(
            out.stderr.ends_with(&format!(
                "\n  ✗ Error: Interrupted; the one-time credential was saved in {path}\n\n"
            )),
            "{}",
            &out.stderr[out.stderr.len().saturating_sub(300)..]
        );
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains(&fx::marker(30))
        );
        assert!(temporaries(&f.sandbox.tmp()).is_empty());
    }

    /// The provisioning contexts (§19.6): during the scope create, during the workspace
    /// create, during the bind.
    #[test]
    fn provisioning_names_the_step() {
        let scope = "ws_truffles_bx7qw1";
        for (route, context) in [
            (
                Gw::ScopeCreate,
                format!("the IAM scope {scope} may have been created (outcome unknown)"),
            ),
            (
                Gw::WorkspaceCreate,
                format!(
                    "IAM scope {scope} was created; the workspace may have been created (outcome unknown)"
                ),
            ),
            (
                Gw::ScopeUpdate,
                format!(
                    "the workspace ws-truffl-000004 was created; IAM scope {scope} may not bind it yet"
                ),
            ),
        ] {
            let f = fx();
            f.once(route, Planned::held());
            let running = spawn_gw(
                &f,
                &[
                    "workspaces",
                    "create",
                    "--name",
                    "Truffles",
                    "--scope-name",
                    scope,
                ],
            );
            assert!(
                f.server
                    .wait_for_route(Route::Gateway(route), 1, Duration::from_secs(20))
            );
            running.send_signal("INT");
            let out = running.wait();
            f.server.release_held();
            out.assert_code(130);
            assert_eq!(
                out.stderr,
                format!("\n  ✗ Error: Interrupted; {context}\n\n"),
                "{route:?}"
            );
            assert!(
                f.server.gw_requests(Gw::ScopeDelete).is_empty(),
                "no rollback after a signal"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Withholding of write answers and request credentials (D238, G43)
// ---------------------------------------------------------------------------

mod withholding {
    use super::*;

    /// A write answer goes through the resource's policy in every format: a provider key,
    /// a SCIM token, plugin credentials are `[withheld]`.
    /// D52: control characters in a write answer are replaced in pretty and table (JSON
    /// escapes them); the `✓` acknowledgement names the id as typed.
    #[test]
    fn write_answers_are_terminal_safe() {
        let f = fx();
        f.server.gateway(|data| {
            data.write_answers.insert(
                Gw::GuardrailUpdate,
                json!({"success": true, "note": "a\u{1b}[31mb\u{7}"}),
            );
        });
        for format in ["pretty", "table"] {
            let out = f.gw(&[
                "guardrails",
                "update",
                &u(fx::GUARDRAIL),
                "--name",
                "g",
                "--output",
                format,
            ]);
            out.assert_success();
            assert!(
                out.stdout.contains("a\u{fffd}[31mb\u{fffd}"),
                "{format}: {}",
                out.stdout
            );
            assert!(!out.stdout.contains('\u{1b}'));
        }
        let out = f.gw(&[
            "guardrails",
            "update",
            &u(fx::GUARDRAIL),
            "--name",
            "g",
            "--output",
            "json",
        ]);
        assert_eq!(out.json()["note"], "a\u{1b}[31mb\u{7}");
    }

    /// D178 (G40): a non-empty MCP workspace-set answer (the frozen TypeScript schema
    /// expects `{}` and fails the answer) is printed, terminal-safe.
    #[test]
    fn an_unfamiliar_mcp_workspace_set_answer_is_printed() {
        let f = fx();
        f.server.gateway(|data| {
            data.write_answers.insert(
                Gw::McpWorkspacesSet,
                json!({"updated": 2, "workspaces": [{"id": "ws-a", "enabled": true}]}),
            );
        });
        let out = f.gw(&[
            "mcp",
            "integrations",
            "workspaces",
            "set",
            &u(fx::MCP_INTEGRATION),
            "--workspace-binding",
            "ws-a=true",
            "--force",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(out.json()["updated"], 2);
        assert_eq!(out.json()["workspaces"][0]["id"], "ws-a");
    }

    #[test]
    fn write_answers_are_withheld() {
        let marker = fx::marker(37);
        for (args, route, answer, pointer) in [
            (
                vec![
                    "providers".to_owned(),
                    "update".into(),
                    u(fx::PROVIDER),
                    "--name".into(),
                    "p".into(),
                ],
                Gw::ProviderUpdate,
                json!({"success": true, "key": marker}),
                "/key",
            ),
            (
                vec![
                    "organisations".to_owned(),
                    "auth-settings".into(),
                    "update".into(),
                    "--domains".into(),
                    "a.test".into(),
                    "--force".into(),
                ],
                Gw::AuthSettingsUpdate,
                json!({"success": true, "data": {"scim_token": marker}}),
                "/data/scim_token",
            ),
            (
                vec![
                    "plugins".to_owned(),
                    "create".into(),
                    "--integration-id".into(),
                    u(fx::INTEGRATION),
                    "--credential".into(),
                    "region=us".into(),
                ],
                Gw::PluginCreate,
                json!({"id": u(fx::NEW_PLUGIN), "credentials": {"region": marker}}),
                "/credentials",
            ),
        ] {
            for format in ["pretty", "table", "json"] {
                let f = fx();
                f.once(route, Planned::json(200, &answer));
                let mut refs: Vec<&str> = args.iter().map(String::as_str).collect();
                refs.extend(["--output", format]);
                let out = f.gw(&refs);
                out.assert_success();
                assert!(!out.stdout.contains(&marker));
                assert!(
                    out.stdout.contains("[withheld]"),
                    "{refs:?}: {}",
                    out.stdout
                );
                if format == "json" {
                    assert_eq!(out.json().pointer(pointer).unwrap(), "[withheld]");
                }
            }
        }
    }

    /// A credential typed into a request (`--key`, `--file`) joins the `SecretSet`: an
    /// answer that echoes it elsewhere prints `[withheld]`.
    #[test]
    fn request_credentials_join_the_secret_set() {
        let key = fx::marker(39);
        let f = fx();
        f.once(
            Gw::IntegrationUpdate,
            Planned::json(
                200,
                &json!({"success": true, "note": format!("stored {key}")}),
            ),
        );
        let out = f.gw(&[
            "integrations",
            "update",
            &u(fx::INTEGRATION),
            "--key",
            &key,
            "--force",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(out.json(), json!({"success": true, "note": "[withheld]"}));
        let f = fx();
        let file = f.file(
            "mcp.json",
            json!({"configurations": {"client_secret": key}})
                .to_string()
                .as_bytes(),
        );
        f.once(
            Gw::McpUpdate,
            Planned::json(200, &json!({"success": true, "echo": key})),
        );
        let out = f.gw(&[
            "mcp",
            "integrations",
            "update",
            &u(fx::MCP_INTEGRATION),
            "--file",
            &file,
            "--force",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(out.json(), json!({"success": true, "echo": "[withheld]"}));
        assert_eq!(
            f.body(Gw::McpUpdate),
            json!({"configurations": {"client_secret": key}})
        );
    }

    /// D238 (review): a header credential in a `models set` or `guardrails update` body
    /// (outside the SDK's secret paths) joins the `SecretSet` before the request, so an
    /// answer that echoes it, at its own place or elsewhere, prints `[withheld]`; the body
    /// carries it unchanged.
    #[test]
    fn d238_header_credentials_in_a_body_join_the_secret_set() {
        let header = fx::marker(45);
        let f = fx();
        let body = json!({"models": [{"slug": "m1", "enabled": true, "configurations": {"custom_headers": {"key": "X-Tenant-Auth", "value": header}}}], "allow_all_models": false});
        let file = f.file("m.json", body.to_string().as_bytes());
        f.once(
            Gw::ModelsSet,
            Planned::json(
                200,
                &json!({"success": true, "models": body["models"].clone(), "note": format!("stored {header}")}),
            ),
        );
        let out = f.gw(&[
            "integrations",
            "models",
            "set",
            &u(fx::INTEGRATION),
            "--file",
            &file,
            "--force",
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(out.json()["note"], "[withheld]");
        assert_eq!(
            out.json()["models"][0]["configurations"]["custom_headers"],
            json!({"key": "X-Tenant-Auth", "value": "[withheld]"})
        );
        assert_eq!(f.body(Gw::ModelsSet), body);
        let header = fx::marker(46);
        let f = fx();
        let checks = json!([{"id": "default.webhook", "is_enabled": true, "parameters": {"headers": {"X-Functions-Key": header}}}]);
        f.once(
            Gw::GuardrailUpdate,
            Planned::json(
                200,
                &json!({"success": true, "data": {"checks": checks.clone(), "note": format!("key {header}")}}),
            ),
        );
        let out = f.gw(&[
            "guardrails",
            "update",
            &u(fx::GUARDRAIL),
            "--checks",
            &checks.to_string(),
            "--output",
            "json",
        ]);
        out.assert_success();
        assert_eq!(out.json()["data"]["note"], "[withheld]");
        assert_eq!(
            out.json()["data"]["checks"][0]["parameters"]["headers"]["X-Functions-Key"],
            "[withheld]"
        );
        assert_eq!(f.body(Gw::GuardrailUpdate)["checks"], checks);
    }
}
