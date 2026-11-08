//! Executable-level acceptance tests for the M2 read-only management commands
//! (`runtime profiles list|ls|get`, `runtime topics list|ls|get`), DESIGN §5–§7.
//!
//! Every test spawns the real `airs-cli-rs` binary against the loopback
//! [`FixtureServer`] with a synthetic tenant whose every endpoint key points at the
//! server ([`Sandbox::single_tenant`] → `FixtureConfig::standard`). After every run the
//! tenant files must be unchanged and no request may reach an unknown route (except
//! where a test proves the opposite). The runner's secret-leak check stays on.

use std::time::Duration;

use airs_test_harness::{
    AuthIdentity, Cli, ClosedPort, FixtureServer, FixtureTenant, Planned, RecordedRequest, Route,
    Run, RunOptions, Sandbox, fixtures, run, spawn,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

const TSG: &str = "100";
const PROFILES_PATH: &str = "/aisec/v1/mgmt/profiles/tsg/100";
const TOPICS_PATH: &str = "/aisec/v1/mgmt/topics/tsg/100";
const TOKEN_LINE: &str = "POST /oauth/token";

fn rust() -> Cli {
    Cli::rust(env!("CARGO_BIN_EXE_airs"))
}

struct Fx {
    server: FixtureServer,
    sandbox: Sandbox,
    #[allow(dead_code)]
    tenant: FixtureTenant,
}

/// One standard tenant `prod` (TSG 100), registered and active.
fn fx() -> Fx {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.single_tenant(&server, "prod", TSG);
    Fx {
        server,
        sandbox,
        tenant,
    }
}

/// Like [`fx`], with the config edited before it is written.
fn fx_with(edit: impl FnOnce(&mut airs_test_harness::FixtureConfig)) -> Fx {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, edit);
    sandbox.register(&[&tenant], Some("prod"));
    Fx {
        server,
        sandbox,
        tenant,
    }
}

/// Rust-only: like [`fx_with`], but the edit may point an endpoint away from loopback
/// (harness opt-out; the TS CLI can no longer run in this sandbox).
fn fx_with_external_endpoints(edit: impl FnOnce(&mut airs_test_harness::FixtureConfig)) -> Fx {
    let server = FixtureServer::start();
    let sandbox = Sandbox::new();
    let tenant = sandbox.add_fixture_tenant_allowing_external_endpoints(&server, "prod", TSG, edit);
    sandbox.register(&[&tenant], Some("prod"));
    Fx {
        server,
        sandbox,
        tenant,
    }
}

impl Fx {
    /// Runs airs-cli-rs, then checks the read-only and loopback invariants.
    fn run(&self, args: &[&str]) -> Run {
        self.run_with(args, &RunOptions::new())
    }

    fn run_with(&self, args: &[&str], options: &RunOptions) -> Run {
        let before = self.sandbox.snapshot();
        let out = run(&rust(), &self.sandbox, args, options);
        self.sandbox.assert_unchanged(&before);
        self.server.assert_no_unknown_routes();
        assert!(
            self.sandbox.home_listing().is_empty(),
            "airs wrote into HOME"
        );
        assert!(self.sandbox.cwd_listing().is_empty(), "airs wrote into cwd");
        out
    }

    /// `METHOD /path?query` of every request, in order.
    fn lines(&self) -> Vec<String> {
        self.server
            .requests()
            .iter()
            .map(RecordedRequest::line)
            .collect()
    }

    fn profiles(&self, rows: Vec<Value>) {
        self.server.set_profiles(TSG, rows);
    }

    fn topics(&self, rows: Vec<Value>) {
        self.server.set_topics(TSG, rows);
    }

    /// The management wire contract (§5): token requests use valid Basic client
    /// credentials and the client-credentials grant scoped to the TSG; every API GET
    /// carries the issued bearer, no body, no `x-tsg-id`, no `x-pan-token`.
    fn assert_management_wire(&self) {
        for request in self.server.requests() {
            match request.route {
                Route::Token => {
                    assert_eq!(request.method, "POST");
                    assert_eq!(
                        request.auth,
                        AuthIdentity::Basic {
                            client_id: "client-100".into(),
                            tsg: Some(TSG.into()),
                            valid: true
                        },
                        "{request:?}"
                    );
                    let mut form = request.form();
                    form.sort();
                    assert_eq!(
                        form,
                        vec![
                            ("grant_type".to_owned(), "client_credentials".to_owned()),
                            ("scope".to_owned(), "tsg_id:100".to_owned()),
                        ]
                    );
                    assert!(
                        request
                            .header("content-type")
                            .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded")),
                        "{request:?}"
                    );
                }
                Route::Profiles | Route::Topics => {
                    assert_eq!(request.method, "GET");
                    assert_eq!(
                        request.auth,
                        AuthIdentity::Bearer {
                            tsg: Some(TSG.into())
                        },
                        "{request:?}"
                    );
                    assert!(
                        request.header("authorization") == Some("Bearer mock-token-100"),
                        "bearer missing: {request:?}"
                    );
                    assert!(request.header("x-tsg-id").is_none(), "{request:?}");
                    assert!(request.header("x-pan-token").is_none(), "{request:?}");
                    assert!(request.body.is_empty(), "{request:?}");
                }
                other => panic!("unexpected route {other}: {request:?}"),
            }
        }
    }
}

fn profiles_q(query: &str) -> String {
    format!("GET {PROFILES_PATH}?{query}")
}

fn topics_q(query: &str) -> String {
    format!("GET {TOPICS_PATH}?{query}")
}

/// Token, then the given GET lines.
fn expect(gets: &[String]) -> Vec<String> {
    let mut lines = vec![TOKEN_LINE.to_owned()];
    lines.extend(gets.iter().cloned());
    lines
}

/// Object keys found at exactly `indent` spaces in pretty JSON, in printed order.
fn keys_at_indent(text: &str, indent: usize) -> Vec<String> {
    let prefix = " ".repeat(indent);
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix(&prefix)?;
            let rest = rest.strip_prefix('"')?;
            let (key, _) = rest.split_once("\":")?;
            Some(key.to_owned())
        })
        .collect()
}

/// `(name, revision, id)` of every row of a JSON array.
fn rows_of(value: &Value, name_key: &str, id_key: &str) -> Vec<(String, f64, String)> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("expected array, got {value}"))
        .iter()
        .map(|row| {
            (
                row[name_key].as_str().unwrap_or_default().to_owned(),
                row["revision"].as_f64().unwrap_or(-1.0),
                row[id_key].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

fn names(value: &Value, name_key: &str) -> Vec<String> {
    rows_of(value, name_key, "_")
        .into_iter()
        .map(|(name, _, _)| name)
        .collect()
}

/// The operational-error rendering of DESIGN §2.2.
fn op_error(message: &str, status: Option<u16>, details: &[&str]) -> String {
    let mut text = format!("\n  ✗ Error: {message}\n");
    if let Some(status) = status {
        text.push_str(&format!("    HTTP {status}\n"));
        // The TS hint follows the HTTP line when --debug is off (DESIGN §2.2).
        text.push_str("    Re-run with --debug to capture redacted API diagnostics.\n");
    }
    for line in details {
        text.push_str(&format!("    {line}\n"));
    }
    text.push('\n');
    text
}

fn usage_error(message: &str) -> String {
    format!("\n  ✗ {message}\n\n")
}

fn assert_failed(out: &Run, stderr: &str) {
    out.assert_code(1);
    assert_eq!(out.stdout, "", "failing runs leave stdout empty");
    assert_eq!(out.stderr, stderr);
}

fn minimal_profile(name: &str, revision: u64) -> Value {
    json!({"profile_name": name, "revision": revision})
}

fn minimal_topic(name: &str, revision: u64) -> Value {
    json!({"topic_name": name, "revision": revision, "description": "d"})
}

const LIST: &[&str] = &["runtime", "profiles", "list", "--output", "json"];
const TLIST: &[&str] = &["runtime", "topics", "list", "--output", "json"];

fn with<'a>(base: &[&'a str], extra: &[&'a str]) -> Vec<&'a str> {
    base.iter().chain(extra.iter()).copied().collect()
}

// ---------------------------------------------------------------------------
// profiles list: single page (§7.2, §7.3, §7.5)
// ---------------------------------------------------------------------------

mod profiles_list {
    use super::*;

    const FIXTURE_ALPHA_2: &str = r#"  {
    "profileId": "00000000-0000-4000-8000-000000000002",
    "profileName": "alpha",
    "revision": 2,
    "active": true,
    "createdBy": "creator@example.com",
    "updatedBy": "updater@example.com",
    "lastModifiedTs": "2026-02-01T00:00:00Z",
    "policy": {
      "ai-security-profiles": [
        {
          "model-type": "default",
          "model-configuration": {
            "mask-data-in-storage": false
          }
        }
      ],
      "dlp-data-profiles": []
    }
  }"#;

    #[test]
    fn default_page_requests_latest_and_prints_exact_json() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "alpha", 2),
            fixtures::profile(3, "beta", 1),
        ]);
        let out = f.run(LIST);
        out.assert_success();
        let beta = FIXTURE_ALPHA_2
            .replace("000000000002", "000000000003")
            .replace("\"alpha\"", "\"beta\"")
            .replace("\"revision\": 2", "\"revision\": 1");
        assert_eq!(out.stdout, format!("[\n{FIXTURE_ALPHA_2},\n{beta}\n]\n"));
        assert_eq!(out.stderr, "  Showing 2\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
        f.assert_management_wire();
    }

    #[test]
    fn ls_alias_behaves_like_list() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(&["runtime", "profiles", "ls", "--output", "json"]);
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["alpha"]);
        assert_eq!(out.stderr, "  Showing 1\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
    }

    #[test]
    fn all_versions_sends_latest_false_and_returns_every_revision() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "alpha", 2),
        ]);
        let out = f.run(&with(LIST, &["--all-versions"]));
        out.assert_success();
        assert_eq!(
            rows_of(&out.json(), "profileName", "profileId"),
            [
                ("alpha".into(), 1.0, fixtures::uuid(1)),
                ("alpha".into(), 2.0, fixtures::uuid(2))
            ]
        );
        assert_eq!(out.stderr, "  Showing 2\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=false")])
        );
        f.assert_management_wire();
    }

    #[test]
    fn limit_and_offset_select_one_page_in_query_order() {
        let f = fx();
        f.profiles(
            (1..=6)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(LIST, &["--limit", "2", "--offset", "4"]));
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["p5", "p6"]);
        assert_eq!(out.stderr, "  Showing 2\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=4&limit=2&latest=true")])
        );
        let get = &f.server.requests_for(Route::Profiles)[0];
        assert_eq!(
            get.query,
            vec![
                ("offset".to_owned(), "4".to_owned()),
                ("limit".to_owned(), "2".to_owned()),
                ("latest".to_owned(), "true".to_owned())
            ]
        );
    }

    #[test]
    fn positive_next_offset_reports_more_available() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "beta", 1),
        ]);
        let out = f.run(&with(LIST, &["--limit", "1"]));
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["alpha"]);
        assert_eq!(out.stderr, "  Showing 1 (more available)\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=1&latest=true")])
        );
    }

    /// D12: `next_offset: 0` ends pagination, so it is not "more available".
    #[test]
    fn next_offset_zero_is_not_more_available_d12() {
        let f = fx();
        f.server.set_zero_next_offset_on_last_page(true);
        f.profiles(vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "beta", 1),
        ]);
        let out = f.run(LIST);
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(2));
        assert_eq!(out.stderr, "  Showing 2\n");
    }

    #[test]
    fn empty_list_prints_empty_array() {
        let f = fx();
        let out = f.run(LIST);
        out.assert_success();
        assert_eq!(out.stdout, "[]\n");
        assert_eq!(out.stderr, "  Showing 0\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
    }

    #[test]
    fn quiet_removes_status_lines_but_keeps_data() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(&with(LIST, &["--quiet", "--limit", "1"]));
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["alpha"]);
        assert_eq!(out.stderr, "");
    }

    #[test]
    fn output_accepted_before_subcommand_and_with_equals() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let a = f.run(&["--output", "json", "runtime", "profiles", "list"]);
        let b = f.run(&["runtime", "profiles", "list", "--output=json"]);
        let c = f.run(&[
            "runtime", "profiles", "list", "--output", "xml", "--output", "json",
        ]);
        for out in [&a, &b, &c] {
            out.assert_success();
            assert_eq!(out.stdout, a.stdout);
            assert_eq!(out.stderr, "  Showing 1\n");
        }
    }

    #[test]
    fn without_output_defaults_to_pretty() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(&["runtime", "profiles", "list"]);
        out.assert_success();
        assert!(
            out.stdout
                .starts_with("\n  Prisma AIRS — Runtime Configuration\n"),
            "{}",
            out.stdout
        );
        assert_eq!(out.stderr, "  Showing 1\n");
    }

    #[test]
    fn tenant_default_output_json_is_honoured_silently() {
        let f = fx_with(|config| {
            config.set("defaultOutput", "json");
        });
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(&["runtime", "profiles", "list"]);
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["alpha"]);
        assert_eq!(out.stderr, "  Showing 1\n");
    }

    /// D51: a dropped format in `defaultOutput` makes the config invalid (no fallback).
    #[test]
    fn tenant_default_output_naming_a_dropped_format_is_an_invalid_config() {
        for value in ["yaml", "markdown", "csv"] {
            let f = fx_with(|config| {
                config.set("defaultOutput", value);
            });
            // Import drops the dead format (§3.12); the TOML file is given it directly.
            f.sandbox.set_tenant_line(
                "prod",
                "defaultOutput",
                &format!("defaultOutput = \"{value}\""),
            );
            f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
            for args in [
                &["runtime", "profiles", "list"][..],
                LIST,
                &["runtime", "topics", "get", "x"],
            ] {
                let out = f.run(args);
                assert_failed(
                    &out,
                    &op_error(
                        &format!(
                            "Cannot read a valid tenant config at {}",
                            f.tenant.store_path.display()
                        ),
                        None,
                        &[],
                    ),
                );
            }
            assert_eq!(f.server.request_count(), 0);
        }
    }

    #[test]
    fn tenant_default_output_table_applies_to_gets_and_an_explicit_output_wins() {
        let f = fx_with(|config| {
            config.set("defaultOutput", "table");
        });
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(&["runtime", "profiles", "get", "alpha"]);
        out.assert_success();
        assert!(
            out.stdout.starts_with(" Key            │ Value"),
            "{}",
            out.stdout
        );
        assert_eq!(out.stderr, "");
        let out = f.run(&["runtime", "profiles", "get", "alpha", "--output", "json"]);
        assert_eq!(out.json()["profileName"], "alpha");
        let out = f.run(&["--output", "pretty", "runtime", "profiles", "get", "alpha"]);
        assert!(out.stdout.contains("  Profile Detail:\n"), "{}", out.stdout);
    }

    #[test]
    fn management_reads_need_no_scanner_credentials() {
        let f = fx_with(|config| {
            config.remove("airsApiKey");
        });
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(LIST);
        out.assert_success();
        assert_eq!(out.stderr, "  Showing 1\n");
    }
}

// ---------------------------------------------------------------------------
// profiles JSON projection (§7.5)
// ---------------------------------------------------------------------------

mod profile_projection {
    use super::*;

    #[test]
    fn key_order_drops_csp_tsg_and_unknown_fields() {
        let f = fx();
        f.profiles(vec![json!({
            "zzz_unknown": {"nested": 1},
            "policy": {"dlp-data-profiles": [], "ai-security-profiles": []},
            "last_modified_ts": "2026-02-01T00:00:00Z",
            "updated_by": "u@example.com",
            "created_by": "c@example.com",
            "tsg_id": "100",
            "csp_id": "csp-1",
            "active": false,
            "revision": 7,
            "profile_name": "alpha",
            "profile_id": fixtures::uuid(9)
        })]);
        let out = f.run(LIST);
        out.assert_success();
        assert_eq!(
            keys_at_indent(&out.stdout, 4),
            [
                "profileId",
                "profileName",
                "revision",
                "active",
                "createdBy",
                "updatedBy",
                "lastModifiedTs",
                "policy"
            ]
        );
        for dropped in ["csp", "tsg", "zzz_unknown", "nested"] {
            assert!(
                !out.stdout.contains(dropped),
                "{dropped} leaked into output"
            );
        }
        assert!(out.stdout.contains("\"active\": false"));
    }

    #[test]
    fn unset_fields_are_omitted() {
        let f = fx();
        f.profiles(vec![json!({"profile_name": "bare"})]);
        let out = f.run(LIST);
        out.assert_success();
        assert_eq!(
            out.stdout,
            "[\n  {\n    \"profileName\": \"bare\"\n  }\n]\n"
        );
    }

    #[test]
    fn integral_float_revision_prints_as_integer_and_fraction_is_kept() {
        let f = fx();
        f.profiles(vec![
            json!({"profile_name": "a", "revision": 2.0}),
            json!({"profile_name": "b", "revision": 2.5}),
        ]);
        let out = f.run(LIST);
        out.assert_success();
        assert!(out.stdout.contains("\"revision\": 2\n"), "{}", out.stdout);
        assert!(!out.stdout.contains("2.0"), "{}", out.stdout);
        assert!(out.stdout.contains("\"revision\": 2.5\n"), "{}", out.stdout);
    }

    #[test]
    fn policy_unknown_keys_are_preserved() {
        let f = fx();
        let policy = json!({
            "zeta-extension": {"b": 1, "a": [true, null]},
            "dlp-data-profiles": [],
            "ai-security-profiles": [{
                "model-type": "default",
                "model-configuration": {"mask-data-in-storage": true, "x-custom": "kept"}
            }],
            "alpha-extension": "v"
        });
        f.profiles(vec![json!({"profile_name": "p", "policy": policy.clone()})]);
        let out = f.run(LIST);
        out.assert_success();
        // Semantic comparison (D35: unknown keys are sorted by the SDK model).
        airs_test_harness::assert_json_semantic_eq(&out.json()[0]["policy"], &policy);
        let policy_keys = keys_at_indent(&out.stdout, 6);
        let known = policy_keys
            .iter()
            .position(|k| k == "ai-security-profiles")
            .unwrap_or(usize::MAX);
        let unknown = policy_keys
            .iter()
            .position(|k| k == "alpha-extension")
            .unwrap_or(0);
        assert!(known < unknown, "known policy keys first: {policy_keys:?}");
    }
}

// ---------------------------------------------------------------------------
// profiles list --all (§7.2, §7.3, D14)
// ---------------------------------------------------------------------------

mod profiles_all {
    use super::*;

    #[test]
    fn walk_follows_next_offset_with_one_token() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "alpha", 2),
            fixtures::profile(3, "beta", 1),
        ]);
        let out = f.run(&with(LIST, &["--all", "--limit", "1", "--all-versions"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(3));
        assert_eq!(out.stderr, "  Showing all 3\n");
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=1&latest=false"),
                profiles_q("offset=1&limit=1&latest=false"),
                profiles_q("offset=2&limit=1&latest=false"),
            ])
        );
        f.assert_management_wire();
    }

    #[test]
    fn walk_with_default_limit_spans_pages_of_100() {
        let f = fx();
        f.profiles(
            (0..250)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(LIST, &["--all"]));
        out.assert_success();
        let rows = names(&out.json(), "profileName");
        assert_eq!(rows.len(), 250);
        assert_eq!(rows[249], "p249");
        assert_eq!(out.stderr, "  Showing all 250\n");
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=100&latest=true"),
                profiles_q("offset=100&limit=100&latest=true"),
                profiles_q("offset=200&limit=100&latest=true"),
            ])
        );
        assert_eq!(f.server.requests_for(Route::Token).len(), 1);
    }

    #[test]
    fn next_offset_zero_ends_the_walk() {
        let f = fx();
        f.server.set_zero_next_offset_on_last_page(true);
        f.profiles(
            (0..3)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(LIST, &["--all", "--limit", "2"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(3));
        assert_eq!(out.stderr, "  Showing all 3\n");
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=2&latest=true"),
                profiles_q("offset=2&limit=2&latest=true"),
            ])
        );
    }

    /// D14: a walk stopped by `--max` says so and fetches no further page.
    #[test]
    fn max_stops_without_extra_fetch_and_says_first_d14() {
        let f = fx();
        f.profiles(
            (0..5)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(LIST, &["--all", "--limit", "1", "--max", "2"]));
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["p0", "p1"]);
        assert_eq!(
            out.stderr,
            "  Showing first 2 (stopped at --max 2; more available)\n"
        );
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=1&latest=true"),
                profiles_q("offset=1&limit=1&latest=true"),
            ])
        );
    }

    #[test]
    fn max_equal_to_total_is_complete() {
        let f = fx();
        f.profiles(
            (0..2)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(LIST, &["--all", "--limit", "5", "--max", "2"]));
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["p0", "p1"]);
        assert_eq!(out.stderr, "  Showing all 2\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=5&latest=true")])
        );
    }

    #[test]
    fn max_zero_is_uncapped() {
        let f = fx();
        f.profiles(
            (0..12)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(LIST, &["--all", "--limit", "5", "--max", "0"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(12));
        assert_eq!(out.stderr, "  Showing all 12\n");
        assert_eq!(f.server.requests_for(Route::Profiles).len(), 3);
    }

    #[test]
    fn max_without_all_is_validated_but_does_not_change_a_page() {
        let f = fx();
        f.profiles(
            (0..3)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(LIST, &["--max", "1"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(3));
        assert_eq!(out.stderr, "  Showing 3\n");
    }

    #[test]
    fn empty_walk_shows_all_zero() {
        let f = fx();
        let out = f.run(&with(LIST, &["--all"]));
        out.assert_success();
        assert_eq!(out.stdout, "[]\n");
        assert_eq!(out.stderr, "  Showing all 0\n");
    }

    #[test]
    fn quiet_walk_has_empty_stderr() {
        let f = fx();
        f.profiles(
            (0..3)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        let out = f.run(&with(
            LIST,
            &["--all", "--limit", "1", "--max", "2", "--quiet"],
        ));
        out.assert_success();
        assert_eq!(out.stderr, "");
        assert_eq!(out.json().as_array().map(Vec::len), Some(2));
    }
}

// ---------------------------------------------------------------------------
// profiles get (§7.2, §7.4)
// ---------------------------------------------------------------------------

mod profiles_get {
    use super::*;

    /// `runtime profiles get <extra> --output json` (these tests assert the JSON data).
    fn get<'a>(extra: &[&'a str]) -> Vec<&'a str> {
        with(
            &with(&["runtime", "profiles", "get"], extra),
            &["--output", "json"],
        )
    }

    #[test]
    fn by_name_returns_highest_revision_without_latest_query() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(3, "alpha", 3),
            fixtures::profile(2, "alpha", 2),
            fixtures::profile(4, "beta", 9),
        ]);
        let out = f.run(&get(&["alpha"]));
        out.assert_success();
        let value = out.json();
        assert_eq!(value["profileId"], fixtures::uuid(3).as_str());
        assert_eq!(value["revision"], 3);
        assert!(out.stdout.starts_with("{\n  \"profileId\""), "bare object");
        assert!(out.stdout.ends_with("}\n"));
        assert_eq!(
            keys_at_indent(&out.stdout, 2),
            [
                "profileId",
                "profileName",
                "revision",
                "active",
                "createdBy",
                "updatedBy",
                "lastModifiedTs",
                "policy"
            ]
        );
        assert_eq!(out.stderr, "", "gets print no status line");
        assert_eq!(f.lines(), expect(&[profiles_q("offset=0&limit=100")]));
        f.assert_management_wire();
    }

    #[test]
    fn by_name_tie_keeps_first_seen() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(1, "alpha", 2),
            fixtures::profile(2, "alpha", 2),
        ]);
        let out = f.run(&get(&["alpha"]));
        out.assert_success();
        assert_eq!(out.json()["profileId"], fixtures::uuid(1).as_str());
    }

    #[test]
    fn by_name_unset_revision_counts_as_zero() {
        let f = fx();
        f.profiles(vec![
            json!({"profile_name": "alpha", "profile_id": fixtures::uuid(1)}),
            json!({"profile_name": "alpha", "profile_id": fixtures::uuid(2), "revision": 0}),
        ]);
        let out = f.run(&get(&["alpha"]));
        out.assert_success();
        assert_eq!(
            out.json()["profileId"],
            fixtures::uuid(1).as_str(),
            "tie at 0"
        );
        f.server.clear_requests();
        f.profiles(vec![
            json!({"profile_name": "alpha", "profile_id": fixtures::uuid(1)}),
            json!({"profile_name": "alpha", "profile_id": fixtures::uuid(2), "revision": 1}),
        ]);
        let out = f.run(&get(&["alpha"]));
        out.assert_success();
        assert_eq!(out.json()["profileId"], fixtures::uuid(2).as_str());
    }

    #[test]
    fn by_uuid_returns_first_exact_match() {
        let f = fx();
        let id = fixtures::uuid(7);
        f.profiles(vec![
            fixtures::profile(1, "other", 5),
            json!({"profile_name": "alpha", "profile_id": id, "revision": 1}),
            json!({"profile_name": "alpha", "profile_id": id, "revision": 2}),
        ]);
        let out = f.run(&get(&[&id]));
        out.assert_success();
        assert_eq!(out.json()["revision"], 1, "first match, not highest");
        assert_eq!(f.lines(), expect(&[profiles_q("offset=0&limit=100")]));
    }

    #[test]
    fn uppercase_uuid_is_an_id_compared_exactly() {
        let f = fx();
        let id = "0000abcd-0000-4000-8000-00000000000a";
        f.profiles(vec![
            json!({"profile_name": "alpha", "profile_id": id, "revision": 1}),
        ]);
        let upper = id.to_uppercase();
        let out = f.run(&get(&[&upper]));
        assert_failed(
            &out,
            &op_error(
                &format!("AISEC_USER_REQUEST_PAYLOAD_ERROR:Profile not found: {upper}"),
                None,
                &[],
            ),
        );
        assert_eq!(f.lines(), expect(&[profiles_q("offset=0&limit=100")]));
    }

    #[test]
    fn lookup_walks_every_page() {
        let f = fx();
        let mut rows: Vec<Value> = (0..150)
            .map(|n| minimal_profile(&format!("p{n}"), 1))
            .collect();
        rows.push(minimal_profile("p3", 4));
        f.profiles(rows);
        let out = f.run(&get(&["p3"]));
        out.assert_success();
        assert_eq!(out.json()["revision"], 4);
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=100"),
                profiles_q("offset=100&limit=100")
            ])
        );
        assert_eq!(f.server.requests_for(Route::Token).len(), 1);
    }

    #[test]
    fn missing_name_is_sdk_style_not_found() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(&get(&["missing"]));
        assert_failed(
            &out,
            "\n  ✗ Error: AISEC_USER_REQUEST_PAYLOAD_ERROR:Profile not found: missing\n\n",
        );
    }

    #[test]
    fn missing_uuid_is_sdk_style_not_found() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let id = fixtures::uuid(99);
        let out = f.run(&get(&[&id]));
        assert_failed(
            &out,
            &op_error(
                &format!("AISEC_USER_REQUEST_PAYLOAD_ERROR:Profile not found: {id}"),
                None,
                &[],
            ),
        );
    }

    #[test]
    fn revision_selects_exact_revision_with_latest_false() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(1, "alpha", 1),
            fixtures::profile(2, "alpha", 2),
            fixtures::profile(3, "alpha", 3),
        ]);
        let out = f.run(&get(&["alpha", "--revision", "2"]));
        out.assert_success();
        assert_eq!(out.json()["profileId"], fixtures::uuid(2).as_str());
        assert!(out.json().is_object());
        assert_eq!(out.stderr, "");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=false")])
        );
        f.assert_management_wire();
    }

    #[test]
    fn revision_zero_never_matches_unset_revision() {
        let f = fx();
        f.profiles(vec![json!({"profile_name": "alpha"})]);
        let out = f.run(&get(&["alpha", "--revision", "0"]));
        assert_failed(&out, &op_error("Profile alpha not found", None, &[]));
    }

    #[test]
    fn revision_not_found_uses_cli_wording() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(&get(&["alpha", "--revision", "9"]));
        assert_failed(&out, "\n  ✗ Error: Profile alpha not found\n\n");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=false")])
        );
    }

    #[test]
    fn all_versions_prints_matching_rows_in_api_order() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(3, "alpha", 3),
            fixtures::profile(9, "beta", 1),
            fixtures::profile(1, "alpha", 1),
        ]);
        let out = f.run(&get(&["alpha", "--all-versions"]));
        out.assert_success();
        assert_eq!(
            rows_of(&out.json(), "profileName", "profileId"),
            [
                ("alpha".into(), 3.0, fixtures::uuid(3)),
                ("alpha".into(), 1.0, fixtures::uuid(1))
            ]
        );
        assert_eq!(out.stderr, "", "no paging status for gets");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=false")])
        );
    }

    #[test]
    fn all_versions_with_revision_filters_the_list() {
        let f = fx();
        f.profiles(vec![
            fixtures::profile(3, "alpha", 3),
            fixtures::profile(1, "alpha", 1),
        ]);
        let out = f.run(&get(&["alpha", "--all-versions", "--revision", "1"]));
        out.assert_success();
        assert_eq!(
            rows_of(&out.json(), "profileName", "profileId"),
            [("alpha".into(), 1.0, fixtures::uuid(1))]
        );
    }

    #[test]
    fn all_versions_by_uuid_and_not_found() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let id = fixtures::uuid(1);
        let out = f.run(&get(&[&id, "--all-versions"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(1));
        let out = f.run(&get(&["nope", "--all-versions"]));
        assert_failed(&out, &op_error("Profile nope not found", None, &[]));
    }
}

// ---------------------------------------------------------------------------
// topics list (§7.2, §7.3, D13, D16, D39)
// ---------------------------------------------------------------------------

mod topics_list {
    use super::*;

    const FIN_2: &str = r#"[
  {
    "topicId": "00000000-0000-4000-8000-000000000002",
    "topicName": "fin",
    "revision": 2,
    "active": true,
    "description": "new",
    "examples": [
      "fin example"
    ],
    "createdBy": "creator@example.com",
    "lastModifiedTs": "2026-02-01T00:00:00Z",
    "createdTs": "2026-01-01T00:00:00Z"
  },"#;

    #[test]
    fn latest_default_walks_limit_200_and_collapses_by_name() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "old"),
            fixtures::topic(2, "fin", 2, "new"),
            fixtures::topic(3, "legal", 1, "law"),
        ]);
        let out = f.run(TLIST);
        out.assert_success();
        assert!(out.stdout.starts_with(FIN_2), "{}", out.stdout);
        assert_eq!(
            rows_of(&out.json(), "topicName", "topicId"),
            [
                ("fin".into(), 2.0, fixtures::uuid(2)),
                ("legal".into(), 1.0, fixtures::uuid(3))
            ]
        );
        assert_eq!(out.stderr, "  Showing 2\n");
        assert_eq!(f.lines(), expect(&[topics_q("offset=0&limit=200")]));
        f.assert_management_wire();
    }

    #[test]
    fn ls_alias_behaves_like_list() {
        let f = fx();
        f.topics(vec![fixtures::topic(1, "fin", 1, "d")]);
        let out = f.run(&["runtime", "topics", "ls", "--output", "json"]);
        out.assert_success();
        assert_eq!(names(&out.json(), "topicName"), ["fin"]);
        assert_eq!(out.stderr, "  Showing 1\n");
    }

    #[test]
    fn latest_walk_over_200_rows_and_local_slice() {
        let f = fx();
        f.topics(
            (0..250)
                .map(|n| minimal_topic(&format!("t{n:03}"), 1))
                .collect(),
        );
        let out = f.run(&with(TLIST, &["--limit", "1000"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(250));
        assert_eq!(out.stderr, "  Showing 250\n");
        assert_eq!(
            f.lines(),
            expect(&[
                topics_q("offset=0&limit=200"),
                topics_q("offset=200&limit=200")
            ])
        );
        assert_eq!(f.server.requests_for(Route::Token).len(), 1);
        f.assert_management_wire();
    }

    /// D13: a latest page truncated by `--limit` reports more available.
    #[test]
    fn latest_default_limit_truncation_reports_more_available_d13() {
        let f = fx();
        f.topics(
            (0..250)
                .map(|n| minimal_topic(&format!("t{n:03}"), 1))
                .collect(),
        );
        let out = f.run(TLIST);
        out.assert_success();
        let rows = names(&out.json(), "topicName");
        assert_eq!(rows.len(), 100);
        assert_eq!(rows[0], "t000");
        assert_eq!(out.stderr, "  Showing 100 (more available)\n");
    }

    #[test]
    fn latest_limit_and_offset_slice_locally_d13() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "d"),
            fixtures::topic(2, "legal", 1, "d"),
            fixtures::topic(3, "fin", 2, "d"),
        ]);
        let first = f.run(&with(TLIST, &["--limit", "1"]));
        first.assert_success();
        assert_eq!(names(&first.json(), "topicName"), ["fin"]);
        assert_eq!(first.json()[0]["revision"], 2);
        assert_eq!(first.stderr, "  Showing 1 (more available)\n");
        let second = f.run(&with(TLIST, &["--limit", "1", "--offset", "1"]));
        second.assert_success();
        assert_eq!(names(&second.json(), "topicName"), ["legal"]);
        assert_eq!(second.stderr, "  Showing 1\n");
        let beyond = f.run(&with(TLIST, &["--offset", "5"]));
        beyond.assert_success();
        assert_eq!(beyond.stdout, "[]\n");
        assert_eq!(beyond.stderr, "  Showing 0\n");
        // --limit/--offset never reach the wire.
        assert_eq!(
            f.lines(),
            [
                TOKEN_LINE.to_owned(),
                topics_q("offset=0&limit=200"),
                TOKEN_LINE.to_owned(),
                topics_q("offset=0&limit=200"),
                TOKEN_LINE.to_owned(),
                topics_q("offset=0&limit=200"),
            ]
        );
    }

    #[test]
    fn empty_topics_print_empty_array() {
        let f = fx();
        let out = f.run(TLIST);
        out.assert_success();
        assert_eq!(out.stdout, "[]\n");
        assert_eq!(out.stderr, "  Showing 0\n");
    }

    #[test]
    fn all_returns_every_latest_topic() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "d"),
            fixtures::topic(2, "fin", 2, "d"),
            fixtures::topic(3, "legal", 1, "d"),
        ]);
        let out = f.run(&with(TLIST, &["--all", "--limit", "1"]));
        out.assert_success();
        assert_eq!(names(&out.json(), "topicName"), ["fin", "legal"]);
        assert_eq!(out.stderr, "  Showing all 2\n");
        assert_eq!(f.lines(), expect(&[topics_q("offset=0&limit=200")]));
    }

    /// D14: `--all --max` truncation is reported, never "all".
    #[test]
    fn all_max_reports_first_d14() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "d"),
            fixtures::topic(2, "legal", 1, "d"),
            fixtures::topic(3, "ops", 1, "d"),
        ]);
        let out = f.run(&with(TLIST, &["--all", "--max", "2"]));
        out.assert_success();
        assert_eq!(names(&out.json(), "topicName"), ["fin", "legal"]);
        assert_eq!(
            out.stderr,
            "  Showing first 2 (stopped at --max 2; more available)\n"
        );
        let exact = f.run(&with(TLIST, &["--all", "--max", "3"]));
        exact.assert_success();
        assert_eq!(exact.stderr, "  Showing all 3\n");
    }

    /// D16: `--max 0` for topics is uncapped.
    #[test]
    fn all_max_zero_is_uncapped_d16() {
        let f = fx();
        f.topics(
            (0..450)
                .map(|n| minimal_topic(&format!("t{n:03}"), 1))
                .collect(),
        );
        let out = f.run(&with(TLIST, &["--all", "--max", "0"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(450));
        assert_eq!(out.stderr, "  Showing all 450\n");
        assert_eq!(
            f.lines(),
            expect(&[
                topics_q("offset=0&limit=200"),
                topics_q("offset=200&limit=200"),
                topics_q("offset=400&limit=200"),
            ])
        );
    }

    #[test]
    fn all_versions_page_slices_locally_from_limit_100_walk() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "d"),
            fixtures::topic(2, "fin", 2, "d"),
            fixtures::topic(3, "legal", 1, "d"),
        ]);
        let out = f.run(&with(
            TLIST,
            &["--all-versions", "--limit", "1", "--offset", "1"],
        ));
        out.assert_success();
        assert_eq!(
            rows_of(&out.json(), "topicName", "topicId"),
            [("fin".into(), 2.0, fixtures::uuid(2))]
        );
        assert_eq!(out.stderr, "  Showing 1 of 3 (next --offset 2)\n");
        assert_eq!(f.lines(), expect(&[topics_q("offset=0&limit=100")]));
        let last = f.run(&with(
            TLIST,
            &["--all-versions", "--limit", "1", "--offset", "2"],
        ));
        last.assert_success();
        assert_eq!(last.stderr, "  Showing 1 of 3\n");
        let beyond = f.run(&with(TLIST, &["--all-versions", "--offset", "9"]));
        beyond.assert_success();
        assert_eq!(beyond.stdout, "[]\n");
        assert_eq!(beyond.stderr, "  Showing 0 of 3\n");
    }

    #[test]
    fn all_versions_walks_more_than_100_rows() {
        let f = fx();
        f.topics(
            (0..150)
                .map(|n| minimal_topic(&format!("t{n:03}"), 1))
                .collect(),
        );
        let out = f.run(&with(TLIST, &["--all-versions"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(100));
        assert_eq!(out.stderr, "  Showing 100 of 150 (next --offset 100)\n");
        assert_eq!(
            f.lines(),
            expect(&[
                topics_q("offset=0&limit=100"),
                topics_q("offset=100&limit=100")
            ])
        );
    }

    #[test]
    fn all_versions_all_returns_everything() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "d"),
            fixtures::topic(2, "fin", 2, "d"),
            fixtures::topic(3, "legal", 1, "d"),
        ]);
        let out = f.run(&with(TLIST, &["--all-versions", "--all"]));
        out.assert_success();
        assert_eq!(out.json().as_array().map(Vec::len), Some(3));
        assert_eq!(out.stderr, "  Showing all 3\n");
        assert_eq!(f.lines(), expect(&[topics_q("offset=0&limit=100")]));
    }

    /// D39: `--all-versions --all --max N` honours `--max`.
    #[test]
    fn all_versions_all_honours_max_d39() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "d"),
            fixtures::topic(2, "fin", 2, "d"),
            fixtures::topic(3, "legal", 1, "d"),
        ]);
        let out = f.run(&with(TLIST, &["--all-versions", "--all", "--max", "2"]));
        out.assert_success();
        assert_eq!(
            rows_of(&out.json(), "topicName", "topicId"),
            [
                ("fin".into(), 1.0, fixtures::uuid(1)),
                ("fin".into(), 2.0, fixtures::uuid(2))
            ]
        );
        assert_eq!(
            out.stderr,
            "  Showing first 2 (stopped at --max 2; more available)\n"
        );
    }

    #[test]
    fn quiet_removes_topic_status_lines() {
        let f = fx();
        f.topics(vec![fixtures::topic(1, "fin", 1, "d")]);
        for extra in [
            &["--quiet"][..],
            &["--quiet", "--all-versions"],
            &["--quiet", "--all"],
        ] {
            let out = f.run(&with(TLIST, extra));
            out.assert_success();
            assert_eq!(out.stderr, "", "{extra:?}");
            assert_eq!(out.json().as_array().map(Vec::len), Some(1));
        }
    }
}

// ---------------------------------------------------------------------------
// topics JSON projection (§7.5)
// ---------------------------------------------------------------------------

mod topic_projection {
    use super::*;

    #[test]
    fn key_order_drops_csp_tsg_and_unknown_fields() {
        let f = fx();
        f.topics(vec![json!({
            "zzz_unknown": 1,
            "created_ts": "2025-12-01T00:00:00Z",
            "last_modified_ts": "2026-01-01T00:00:00Z",
            "updated_by": "u@example.com",
            "created_by": "c@example.com",
            "examples": ["e1", "e2"],
            "description": "desc",
            "active": true,
            "revision": 1,
            "tsg_id": "100",
            "csp_id": "csp-1",
            "topic_name": "fin",
            "topic_id": fixtures::uuid(11)
        })]);
        let out = f.run(TLIST);
        out.assert_success();
        assert_eq!(
            keys_at_indent(&out.stdout, 4),
            [
                "topicId",
                "topicName",
                "revision",
                "active",
                "description",
                "examples",
                "createdBy",
                "updatedBy",
                "lastModifiedTs",
                "createdTs"
            ]
        );
        for dropped in ["csp", "tsg", "zzz_unknown"] {
            assert!(
                !out.stdout.contains(dropped),
                "{dropped} leaked into output"
            );
        }
    }

    #[test]
    fn examples_default_to_empty_array_and_unset_fields_are_omitted() {
        let f = fx();
        f.topics(vec![
            json!({"topic_name": "t", "revision": 3.0, "description": ""}),
        ]);
        let out = f.run(TLIST);
        out.assert_success();
        assert_eq!(
            out.stdout,
            "[\n  {\n    \"topicName\": \"t\",\n    \"revision\": 3,\n    \"description\": \"\",\n    \"examples\": []\n  }\n]\n"
        );
    }

    #[test]
    fn get_detail_uses_the_same_projection() {
        let f = fx();
        f.topics(vec![
            json!({"topic_name": "t", "revision": 1, "description": "d", "csp_id": "x"}),
        ]);
        let out = f.run(&["runtime", "topics", "get", "t", "--output", "json"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            "{\n  \"topicName\": \"t\",\n  \"revision\": 1,\n  \"description\": \"d\",\n  \"examples\": []\n}\n"
        );
        assert_eq!(out.stderr, "");
    }
}

// ---------------------------------------------------------------------------
// topics get (§7.2, §7.4)
// ---------------------------------------------------------------------------

mod topics_get {
    use super::*;

    /// `runtime topics get <extra> --output json` (these tests assert the JSON data).
    fn get<'a>(extra: &[&'a str]) -> Vec<&'a str> {
        with(
            &with(&["runtime", "topics", "get"], extra),
            &["--output", "json"],
        )
    }

    fn data(f: &Fx) {
        f.topics(vec![
            fixtures::topic(1, "fin", 1, "old"),
            fixtures::topic(3, "fin", 3, "newest"),
            fixtures::topic(2, "fin", 2, "mid"),
            fixtures::topic(4, "legal", 1, "law"),
        ]);
    }

    #[test]
    fn by_name_returns_highest_revision_with_limit_100_walk() {
        let f = fx();
        data(&f);
        let out = f.run(&get(&["fin"]));
        out.assert_success();
        assert_eq!(out.json()["topicId"], fixtures::uuid(3).as_str());
        assert_eq!(out.json()["description"], "newest");
        assert!(out.json().is_object());
        assert_eq!(out.stderr, "");
        assert_eq!(f.lines(), expect(&[topics_q("offset=0&limit=100")]));
        f.assert_management_wire();
    }

    #[test]
    fn by_name_tie_keeps_first_seen() {
        let f = fx();
        f.topics(vec![
            fixtures::topic(1, "fin", 2, "first"),
            fixtures::topic(2, "fin", 2, "second"),
        ]);
        let out = f.run(&get(&["fin"]));
        out.assert_success();
        assert_eq!(out.json()["description"], "first");
    }

    #[test]
    fn by_uuid_returns_first_exact_match() {
        let f = fx();
        let id = fixtures::uuid(5);
        f.topics(vec![
            json!({"topic_id": id, "topic_name": "fin", "revision": 1, "description": "one"}),
            json!({"topic_id": id, "topic_name": "fin", "revision": 2, "description": "two"}),
        ]);
        let out = f.run(&get(&[&id]));
        out.assert_success();
        assert_eq!(out.json()["description"], "one");
    }

    #[test]
    fn lookup_walks_every_page() {
        let f = fx();
        let mut rows: Vec<Value> = (0..120)
            .map(|n| minimal_topic(&format!("t{n}"), 1))
            .collect();
        rows.push(minimal_topic("t5", 7));
        f.topics(rows);
        let out = f.run(&get(&["t5"]));
        out.assert_success();
        assert_eq!(out.json()["revision"], 7);
        assert_eq!(
            f.lines(),
            expect(&[
                topics_q("offset=0&limit=100"),
                topics_q("offset=100&limit=100")
            ])
        );
    }

    #[test]
    fn missing_name_is_quoted() {
        let f = fx();
        data(&f);
        let out = f.run(&get(&["nope"]));
        assert_failed(&out, "\n  ✗ Error: Topic \"nope\" not found\n\n");
    }

    #[test]
    fn missing_uuid_is_unquoted() {
        let f = fx();
        data(&f);
        let id = fixtures::uuid(77);
        let out = f.run(&get(&[&id]));
        assert_failed(&out, &op_error(&format!("Topic {id} not found"), None, &[]));
    }

    #[test]
    fn uppercase_uuid_is_an_id_compared_exactly() {
        let f = fx();
        let id = "0000abcd-0000-4000-8000-00000000000a";
        f.topics(vec![
            json!({"topic_id": id, "topic_name": "fin", "revision": 1, "description": "d"}),
        ]);
        let upper = id.to_uppercase();
        let out = f.run(&get(&[&upper]));
        assert_failed(
            &out,
            &op_error(&format!("Topic {upper} not found"), None, &[]),
        );
    }

    #[test]
    fn revision_selects_exact_revision() {
        let f = fx();
        data(&f);
        let out = f.run(&get(&["fin", "--revision", "2"]));
        out.assert_success();
        assert_eq!(out.json()["description"], "mid");
        assert_eq!(f.lines(), expect(&[topics_q("offset=0&limit=100")]));
    }

    #[test]
    fn revision_not_found_is_unquoted() {
        let f = fx();
        data(&f);
        let out = f.run(&get(&["fin", "--revision", "9"]));
        assert_failed(&out, "\n  ✗ Error: Topic fin not found\n\n");
    }

    #[test]
    fn all_versions_lists_matching_rows_in_api_order() {
        let f = fx();
        data(&f);
        let out = f.run(&get(&["fin", "--all-versions"]));
        out.assert_success();
        assert_eq!(
            rows_of(&out.json(), "topicName", "topicId"),
            [
                ("fin".into(), 1.0, fixtures::uuid(1)),
                ("fin".into(), 3.0, fixtures::uuid(3)),
                ("fin".into(), 2.0, fixtures::uuid(2)),
            ]
        );
        assert_eq!(out.stderr, "");
        let filtered = f.run(&get(&["fin", "--all-versions", "--revision", "3"]));
        filtered.assert_success();
        assert_eq!(filtered.json().as_array().map(Vec::len), Some(1));
        let none = f.run(&get(&["nope", "--all-versions"]));
        assert_failed(&none, &op_error("Topic nope not found", None, &[]));
    }

    #[test]
    fn uuid_with_flags_not_found_is_unquoted() {
        let f = fx();
        data(&f);
        let id = fixtures::uuid(1);
        let out = f.run(&get(&[&id, "--revision", "5"]));
        assert_failed(&out, &op_error(&format!("Topic {id} not found"), None, &[]));
    }
}

// ---------------------------------------------------------------------------
// Flag validation: exit 2, exact stderr, zero requests, no tenant needed (§2.1, §7.1)
// ---------------------------------------------------------------------------

mod validation {
    use super::*;

    fn assert_usage(f: &Fx, args: &[&str], message: &str) {
        let out = f.run(args);
        out.assert_code(2);
        assert_eq!(out.stdout, "", "{args:?}");
        assert_eq!(out.stderr, usage_error(message), "{args:?}");
        assert_eq!(f.server.request_count(), 0, "{args:?} made requests");
    }

    #[test]
    fn limit_must_be_a_positive_decimal_integer_d17() {
        let f = fx();
        for cmd in [LIST, TLIST] {
            for value in ["0", "-1", "abc", "1.5", "1e2", "0x10", " 5", "+5", ""] {
                assert_usage(
                    &f,
                    &with(cmd, &["--limit", value]),
                    "--limit must be a positive integer",
                );
            }
        }
    }

    #[test]
    fn offset_must_be_a_non_negative_integer() {
        let f = fx();
        for cmd in [LIST, TLIST] {
            for value in ["-1", "x", "1.5", "1e2"] {
                assert_usage(
                    &f,
                    &with(cmd, &["--offset", value]),
                    "--offset must be a non-negative integer",
                );
            }
        }
    }

    #[test]
    fn max_is_validated_even_without_all() {
        let f = fx();
        for cmd in [LIST, TLIST] {
            for extra in [
                &["--max", "-1"][..],
                &["--max", "x"],
                &["--all", "--max", "1e2"],
            ] {
                assert_usage(
                    &f,
                    &with(cmd, extra),
                    "--max must be a non-negative integer",
                );
            }
        }
    }

    #[test]
    fn all_cannot_be_combined_with_explicit_offset_even_zero() {
        let f = fx();
        for cmd in [LIST, TLIST] {
            for value in ["0", "3"] {
                assert_usage(
                    &f,
                    &with(cmd, &["--all", "--offset", value]),
                    "--all cannot be combined with --offset",
                );
            }
        }
    }

    #[test]
    fn validation_order_is_limit_offset_max_then_all() {
        let f = fx();
        assert_usage(
            &f,
            &with(
                LIST,
                &["--all", "--offset", "x", "--limit", "0", "--max", "y"],
            ),
            "--limit must be a positive integer",
        );
        assert_usage(
            &f,
            &with(LIST, &["--all", "--offset", "x", "--max", "y"]),
            "--offset must be a non-negative integer",
        );
        assert_usage(
            &f,
            &with(LIST, &["--all", "--offset", "1", "--max", "y"]),
            "--max must be a non-negative integer",
        );
    }

    #[test]
    fn revision_must_be_a_non_negative_integer_d17() {
        let f = fx();
        for kind in ["profiles", "topics"] {
            for value in ["abc", "-1", "1.0", "0x2", ""] {
                assert_usage(
                    &f,
                    &["runtime", kind, "get", "alpha", "--revision", value],
                    "--revision must be a non-negative integer",
                );
            }
        }
    }

    #[test]
    fn invalid_and_dropped_output_formats() {
        let f = fx();
        for args in [
            with(LIST, &[]),
            with(TLIST, &[]),
            vec!["runtime", "profiles", "get", "alpha"],
            vec!["runtime", "topics", "get", "alpha"],
        ] {
            let mut bad = args.clone();
            bad.retain(|a| *a != "--output" && *a != "json");
            for format in ["xml", "yaml", "csv", "markdown", ""] {
                assert_usage(
                    &f,
                    &with(&bad, &["--output", format]),
                    &format!("Invalid output format '{format}'. Expected: pretty, table, json"),
                );
            }
        }
    }

    /// D2: usage errors need no tenant (validated before the registry is read).
    #[test]
    fn usage_errors_win_without_any_tenant_d2() {
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        for args in [
            with(LIST, &["--limit", "0"]),
            with(TLIST, &["--all", "--offset", "0"]),
            vec!["runtime", "profiles", "get", "x", "--revision", "abc"],
            vec!["runtime", "profiles", "list", "--output", "xml"],
        ] {
            let before = sandbox.snapshot();
            let out = run(&rust(), &sandbox, &args, &RunOptions::new());
            sandbox.assert_unchanged(&before);
            out.assert_code(2);
            assert_eq!(out.stdout, "");
            assert!(out.stderr.starts_with("\n  ✗ "), "{}", out.stderr);
        }
        assert_eq!(server.request_count(), 0);
    }

    /// D1: argument-parse errors exit 2 with zero requests.
    #[test]
    fn parse_errors_exit_2_d1() {
        let f = fx();
        for args in [
            vec!["runtime", "profiles", "get"],
            vec!["runtime", "topics", "get"],
            vec!["runtime", "profiles", "list", "extra"],
            vec!["runtime", "topics", "list", "--bogus"],
            vec!["runtime", "profiles", "get", "a", "b"],
            vec!["runtime", "profiles", "list", "--limit"],
        ] {
            let out = f.run(&args);
            out.assert_code(2);
            assert_eq!(out.stdout, "", "{args:?}");
            assert!(!out.stderr.is_empty(), "{args:?}");
        }
        for group in [["runtime", "profiles"], ["runtime", "topics"]] {
            let out = f.run(&group);
            out.assert_code(2);
            assert_eq!(out.stdout, "", "group help goes to stderr");
            assert!(out.stderr.contains("Usage:"), "{}", out.stderr);
        }
        assert_eq!(f.server.request_count(), 0);
    }

    #[test]
    fn help_prints_to_stdout_with_zero_requests() {
        let f = fx();
        for args in [
            ["runtime", "profiles", "list", "--help"],
            ["runtime", "topics", "get", "--help"],
        ] {
            let out = f.run(&args);
            out.assert_success();
            assert!(out.stdout.contains("Usage:"), "{}", out.stdout);
        }
        assert_eq!(f.server.request_count(), 0);
    }
}

// ---------------------------------------------------------------------------
// Tenant / credential / endpoint failures before any request (§3.5, §5)
// ---------------------------------------------------------------------------

mod local_failures {
    use super::*;

    #[test]
    fn missing_management_secret_fails_with_zero_requests() {
        let f = fx_with(|config| {
            config.remove("mgmtClientSecret");
        });
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "Management credentials are not configured (missing mgmtClientSecret). Run 'airs tenant set prod <key>' for mgmtClientSecret (secrets prompt hidden, or use --stdin)",
                None,
                &[],
            ),
        );
        assert_eq!(f.server.request_count(), 0);
    }

    #[test]
    fn whitespace_credentials_count_as_missing() {
        let f = fx_with(|config| {
            config.set("mgmtClientId", "   ");
            config.remove("mgmtClientSecret");
        });
        let out = f.run(TLIST);
        assert_failed(
            &out,
            &op_error(
                "Management credentials are not configured (missing mgmtClientId, mgmtClientSecret). Run 'airs tenant set prod <key>' for mgmtClientId, mgmtClientSecret (secrets prompt hidden, or use --stdin)",
                None,
                &[],
            ),
        );
        assert_eq!(f.server.request_count(), 0);
    }

    /// Rust-only: a non-loopback http endpoint is rejected locally (zero requests).
    #[test]
    fn non_loopback_http_management_endpoint_is_rejected_locally() {
        for key in ["mgmtEndpoint", "mgmtTokenEndpoint"] {
            let f = fx_with_external_endpoints(|config| {
                config.set(key, "http://example.test/aisec");
            });
            let out = f.run(LIST);
            assert_failed(
                &out,
                &op_error(
                    &format!(
                        "Invalid {key}: endpoints must use https (http is allowed only for loopback hosts) and must not contain credentials, a query or a fragment"
                    ),
                    None,
                    &[],
                ),
            );
            assert!(!out.stderr.contains("example.test"), "value never echoed");
            assert_eq!(f.server.request_count(), 0);
        }
    }

    /// D28: a non-numeric TSG fails locally.
    #[test]
    fn non_numeric_tsg_fails_locally_d28() {
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        let tenant = sandbox.add_fixture_tenant(&server, "prod", "tsg-abc");
        sandbox.register(&[&tenant], Some("prod"));
        let before = sandbox.snapshot();
        let out = run(&rust(), &sandbox, LIST, &RunOptions::new());
        sandbox.assert_unchanged(&before);
        assert_failed(
            &out,
            &op_error(
                "Invalid mgmtTsgId: the Rust SDK requires 1-32 ASCII digits",
                None,
                &[],
            ),
        );
        assert_eq!(server.request_count(), 0);
    }

    #[test]
    fn tsg_pin_mismatch_fails_with_zero_requests() {
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        let tenant = sandbox.add_fixture_tenant_with(&server, "prod", TSG, |config| {
            config.set("mgmtTsgId", "200");
        });
        sandbox.register(&[&tenant], Some("prod"));
        let out = run(&rust(), &sandbox, TLIST, &RunOptions::new());
        assert_failed(
            &out,
            &op_error(
                "Tenant config TSG differs from its registered identity; register it under a new name",
                None,
                &[],
            ),
        );
        assert_eq!(server.request_count(), 0);
    }

    #[test]
    fn credential_environment_variables_never_change_requests() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let options = RunOptions::new()
            .env("PANW_MGMT_CLIENT_ID", "env-client")
            .env("PANW_MGMT_CLIENT_SECRET", "ENV-SECRET-VALUE")
            .env("PANW_MGMT_TSG_ID", "999")
            .env("PANW_MGMT_ENDPOINT", "http://127.0.0.1:1/nowhere")
            .env("PANW_MGMT_TOKEN_ENDPOINT", "http://127.0.0.1:1/nowhere")
            .secret("ENV-SECRET-VALUE");
        let out = f.run_with(LIST, &options);
        out.assert_success();
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
        f.assert_management_wire();
    }
}

// ---------------------------------------------------------------------------
// HTTP / OAuth / decode failures (§6, D10, D11, D30, D37, D38)
// ---------------------------------------------------------------------------

mod errors {
    use super::*;

    #[test]
    fn not_found_404_prints_api_error_and_http_line() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::json(404, &json!({"message": "nope"})),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            "\n  ✗ Error: AISEC_CLIENT_SIDE_ERROR:API error 404\n    HTTP 404\n    Re-run with --debug to capture redacted API diagnostics.\n\n",
        );
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
    }

    #[test]
    fn not_found_404_on_topics_and_gets() {
        let f = fx();
        for args in [
            TLIST.to_vec(),
            vec!["runtime", "profiles", "get", "alpha"],
            vec!["runtime", "topics", "get", "alpha", "--revision", "1"],
        ] {
            f.server.clear_requests();
            f.server
                .set_default(Route::Profiles, Planned::empty_json(404));
            f.server
                .set_default(Route::Topics, Planned::empty_json(404));
            let out = f.run(&args);
            assert_failed(
                &out,
                &op_error("AISEC_CLIENT_SIDE_ERROR:API error 404", Some(404), &[]),
            );
            assert_eq!(f.server.request_count(), 2, "{args:?}: token + one GET");
        }
    }

    #[test]
    fn request_id_and_error_code_are_printed_when_safe() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::json(404, &json!({"message": "nope", "code": "E404"}))
                .with_header("x-request-id", "req-123"),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_CLIENT_SIDE_ERROR:API error 404 (errorCode: E404)",
                Some(404),
                &["Request ID: req-123"],
            ),
        );
    }

    #[test]
    fn alternate_request_id_headers_and_code_locations() {
        let f = fx();
        for (header, body, code) in [
            ("x-correlation-id", json!({"error": {"code": "E1"}}), "E1"),
            ("request-id", json!({"data": {"errorCode": "E2"}}), "E2"),
            ("x-request-id", json!({"errorCode": "E3"}), "E3"),
        ] {
            f.server.clear_requests();
            f.server.push_once(
                Route::Topics,
                Planned::json(409, &body).with_header(header, "rid-1.a:b_c"),
            );
            let out = f.run(TLIST);
            assert_failed(
                &out,
                &op_error(
                    &format!("AISEC_CLIENT_SIDE_ERROR:API error 409 (errorCode: {code})"),
                    Some(409),
                    &["Request ID: rid-1.a:b_c"],
                ),
            );
        }
    }

    #[test]
    fn invalid_request_id_or_code_characters_are_dropped() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::json(404, &json!({"code": "has space"}))
                .with_header("x-request-id", "bad/value"),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_CLIENT_SIDE_ERROR:API error 404", Some(404), &[]),
        );
    }

    #[test]
    fn problem_duplicate_title_and_detail_printed_once() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::raw(
                422,
                "application/problem+json",
                json!({"title": "Validation failed", "detail": "Validation failed"}).to_string(),
            ),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_CLIENT_SIDE_ERROR:API error 422; Validation failed",
                Some(422),
                &[],
            ),
        );
    }

    /// §6 `Limit` with an HTTP error status is reported as that HTTP error.
    #[test]
    fn oversized_error_body_is_reported_as_its_http_status() {
        let f = fx();
        let body = format!("{{\"message\":\"{}\"}}", "x".repeat(100 * 1024));
        f.server
            .push_once(Route::Profiles, Planned::raw(404, "application/json", body));
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_CLIENT_SIDE_ERROR:API error 404", Some(404), &[]),
        );
    }

    /// §6 `Limit` without an error status: a success body over the client limit.
    #[test]
    fn oversized_success_body_is_a_safety_limit_error() {
        let f = fx();
        let rows: Vec<Value> = (0..20)
            .map(|n| json!({"profile_name": format!("p{n}"), "created_by": "y".repeat(512 * 1024)}))
            .collect();
        f.server.push_once(
            Route::Profiles,
            Planned::json(200, &json!({"ai_profiles": rows})),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_SDK_ERROR:Response exceeded a client safety limit",
                None,
                &[],
            ),
        );
    }

    /// §6 `Transport`: the management API is unreachable after a successful token.
    #[test]
    fn api_endpoint_unreachable_is_network_error() {
        let port = ClosedPort::new().port();
        let f = fx_with(|config| {
            config.set("mgmtEndpoint", format!("http://127.0.0.1:{port}/aisec"));
        });
        let out = f.run_with(LIST, &RunOptions::new().timeout(Duration::from_secs(60)));
        assert_failed(
            &out,
            &op_error(
                "AISEC_CLIENT_SIDE_ERROR:Network request failed (connection refused, reset or unreachable)",
                None,
                &[],
            ),
        );
        assert_eq!(f.lines(), [TOKEN_LINE.to_owned()]);
    }

    /// D11 + §6: server text, request IDs and codes that echo a secret are never printed.
    #[test]
    fn echoed_secret_or_token_is_never_printed() {
        let f = fx();
        for (body, request_id) in [
            (
                json!({"message": "FAKE-SECRET-100", "code": "FAKE-SECRET-100"}),
                "FAKE-SECRET-100",
            ),
            (
                json!({"message": "mock-token-100", "data": {"errorCode": "mock-token-100"}}),
                "mock-token-100",
            ),
            (
                json!({"error": {"code": "RkFLRS1TRUNSRVQtMTAw"}}),
                "bW9jay10b2tlbi0xMDA",
            ),
            (
                json!({"errorCode": "x-FAKE-SECRET-100-y", "msg": "FAKE%2DSECRET"}),
                "pre.mock-token-100.post",
            ),
        ] {
            f.server.clear_requests();
            f.server.push_once(
                Route::Profiles,
                Planned::json(403, &body)
                    .with_header("x-opa-decision", "false")
                    .with_header("x-request-id", request_id),
            );
            let out = f.run(LIST);
            assert_failed(
                &out,
                &op_error("AISEC_CLIENT_SIDE_ERROR:API error 403", Some(403), &[]),
            );
        }
    }

    #[test]
    fn problem_json_400_prints_allowlisted_fields_only() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::raw(
                400,
                "application/problem+json",
                json!({
                    "type": "https://example.test/probs/x",
                    "title": "Bad Request",
                    "detail": "profile FAKE-SECRET-100 is broken",
                    "errors": [
                        {"property": "name", "reason": "must not be blank"},
                        {"property": "secret_field", "reason": "leaky text"}
                    ]
                })
                .to_string(),
            ),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_CLIENT_SIDE_ERROR:API error 400; Bad Request; [withheld]; name: must not be blank; [withheld]: [withheld]",
                Some(400),
                &[],
            ),
        );
        assert_eq!(
            f.server.requests_for(Route::Profiles).len(),
            1,
            "400 is final"
        );
    }

    #[test]
    fn unauthorized_once_refreshes_token_and_replays_once() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        f.server
            .push_once(Route::Profiles, Planned::empty_json(401));
        let out = f.run(LIST);
        out.assert_success();
        assert_eq!(names(&out.json(), "profileName"), ["alpha"]);
        assert_eq!(out.stderr, "  Showing 1\n");
        let q = profiles_q("offset=0&limit=100&latest=true");
        assert_eq!(
            f.lines(),
            [TOKEN_LINE.to_owned(), q.clone(), TOKEN_LINE.to_owned(), q]
        );
        f.assert_management_wire();
    }

    #[test]
    fn forbidden_without_opa_header_refreshes_like_401() {
        let f = fx();
        f.topics(vec![fixtures::topic(1, "fin", 1, "d")]);
        f.server.push_once(Route::Topics, Planned::empty_json(403));
        let out = f.run(TLIST);
        out.assert_success();
        assert_eq!(f.server.requests_for(Route::Token).len(), 2);
        assert_eq!(f.server.requests_for(Route::Topics).len(), 2);
    }

    #[test]
    fn unauthorized_twice_fails_after_one_refresh() {
        let f = fx();
        f.server
            .set_default(Route::Profiles, Planned::empty_json(401));
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_CLIENT_SIDE_ERROR:API error 401", Some(401), &[]),
        );
        let q = profiles_q("offset=0&limit=100&latest=true");
        assert_eq!(
            f.lines(),
            [TOKEN_LINE.to_owned(), q.clone(), TOKEN_LINE.to_owned(), q]
        );
    }

    #[test]
    fn forbidden_with_opa_decision_false_does_not_refresh() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::empty_json(403).with_header("x-opa-decision", "false"),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_CLIENT_SIDE_ERROR:API error 403", Some(403), &[]),
        );
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
    }

    #[test]
    fn service_unavailable_is_attempted_six_times() {
        let f = fx();
        f.server.set_default(
            Route::Profiles,
            Planned::json(503, &json!({"message": "busy"})).with_retry_after(0),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_SERVER_SIDE_ERROR:API error 503", Some(503), &[]),
        );
        assert_eq!(f.server.requests_for(Route::Profiles).len(), 6);
        assert_eq!(f.server.requests_for(Route::Token).len(), 1);
    }

    #[test]
    fn bad_gateway_text_body_is_retried_and_never_echoed() {
        let f = fx();
        f.server.set_default(
            Route::Topics,
            Planned::raw(502, "text/plain", "oops upstream").with_retry_after(0),
        );
        let out = f.run(TLIST);
        assert_failed(
            &out,
            &op_error("AISEC_SERVER_SIDE_ERROR:API error 502", Some(502), &[]),
        );
        assert_eq!(f.server.requests_for(Route::Topics).len(), 6);
    }

    #[test]
    fn transient_503_then_success_recovers() {
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        f.server.push_once(
            Route::Profiles,
            Planned::empty_json(503).with_retry_after(0),
        );
        let out = f.run(LIST);
        out.assert_success();
        assert_eq!(f.server.requests_for(Route::Profiles).len(), 2);
        assert_eq!(f.server.requests_for(Route::Token).len(), 1);
    }

    /// D10: 500 is not retried by the SDK.
    #[test]
    fn internal_server_error_is_attempted_once_d10() {
        let f = fx();
        f.server.set_default(
            Route::Profiles,
            Planned::json(500, &json!({"error_message": "boom"})).with_retry_after(0),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_SERVER_SIDE_ERROR:API error 500", Some(500), &[]),
        );
        assert_eq!(f.server.requests_for(Route::Profiles).len(), 1);
    }

    /// D10: 429 is retried by the SDK (6 attempts with Retry-After 0).
    #[test]
    fn too_many_requests_is_retried_d10() {
        let f = fx();
        f.server.set_default(
            Route::Profiles,
            Planned::json(429, &json!({"message": "slow down"})).with_retry_after(0),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_CLIENT_SIDE_ERROR:API error 429", Some(429), &[]),
        );
        assert_eq!(f.server.requests_for(Route::Profiles).len(), 6);
    }

    /// D10: the 401 refresh counts against the retry budget (1 refresh + 4 retries).
    #[test]
    fn refresh_counts_against_retry_budget_d10() {
        let f = fx();
        f.server
            .push_once(Route::Profiles, Planned::empty_json(401));
        f.server.set_default(
            Route::Profiles,
            Planned::empty_json(503).with_retry_after(0),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_SERVER_SIDE_ERROR:API error 503", Some(503), &[]),
        );
        assert_eq!(f.server.requests_for(Route::Profiles).len(), 6);
        assert_eq!(f.server.requests_for(Route::Token).len(), 2);
    }

    #[test]
    fn token_401_is_oauth_error_with_zero_api_requests() {
        let f = fx();
        f.server.push_once(Route::Token, Planned::empty_json(401));
        let out = f.run(LIST);
        assert_failed(
            &out,
            "\n  ✗ Error: AISEC_OAUTH_ERROR:Token request failed with status 401\n    HTTP 401\n    Re-run with --debug to capture redacted API diagnostics.\n\n",
        );
        assert_eq!(f.lines(), [TOKEN_LINE.to_owned()]);
    }

    /// D11: the token endpoint's error_description is not echoed.
    #[test]
    fn token_401_error_description_is_not_echoed_d11() {
        let f = fx();
        f.server.push_once(
            Route::Token,
            Planned::json(
                401,
                &json!({"error": "invalid_client", "error_description": "Client authentication failed"}),
            ),
        );
        let out = f.run(TLIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_OAUTH_ERROR:Token request failed with status 401",
                Some(401),
                &[],
            ),
        );
        assert_eq!(f.lines(), [TOKEN_LINE.to_owned()]);
    }

    #[test]
    fn token_schema_mismatch_is_oauth_schema_error() {
        let f = fx();
        f.server.push_once(
            Route::Token,
            Planned::json(200, &json!({"token_type": "Bearer"})),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_OAUTH_ERROR:Token response did not match the OAuth schema",
                None,
                &[],
            ),
        );
        assert_eq!(f.lines(), [TOKEN_LINE.to_owned()]);
    }

    #[test]
    fn token_endpoint_unreachable_is_network_error() {
        // A loopback port with nothing listening that no fixture server can be given.
        let port = ClosedPort::new().port();
        let f = fx_with(|config| {
            config.set(
                "mgmtTokenEndpoint",
                format!("http://127.0.0.1:{port}/oauth/token"),
            );
        });
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_OAUTH_ERROR:Token request failed: network error",
                None,
                &[],
            ),
        );
        assert_eq!(f.server.request_count(), 0);
    }

    /// D37: a repeated cursor stops the walk with the SDK pagination error.
    #[test]
    fn repeated_next_offset_is_pagination_error_d37() {
        let f = fx();
        f.server.set_default(
            Route::Profiles,
            Planned::json(
                200,
                &json!({"ai_profiles": [fixtures::profile(1, "alpha", 1)], "next_offset": 5}),
            ),
        );
        let out = f.run(&with(LIST, &["--all"]));
        assert_failed(
            &out,
            &op_error(
                "AISEC_SDK_ERROR:Pagination stopped: the service returned a repeated cursor or an empty intermediate page",
                None,
                &[],
            ),
        );
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=100&latest=true"),
                profiles_q("offset=5&limit=100&latest=true"),
            ])
        );
    }

    #[test]
    fn repeated_next_offset_in_topic_walk_is_pagination_error_d37() {
        let f = fx();
        f.server.set_default(
            Route::Topics,
            Planned::json(
                200,
                &json!({"custom_topics": [fixtures::topic(1, "fin", 1, "d")], "next_offset": 1}),
            ),
        );
        let out = f.run(TLIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_SDK_ERROR:Pagination stopped: the service returned a repeated cursor or an empty intermediate page",
                None,
                &[],
            ),
        );
    }

    const DECODE: &str = "AISEC_RESPONSE_VALIDATION:Response did not match the expected schema";

    /// D38: schema-invalid rows are one generic decode error, exit 1, empty stdout.
    #[test]
    fn schema_invalid_profile_rows_are_decode_errors_d38() {
        for body in [
            json!({"ai_profiles": [{"revision": 1}]}),
            json!({"ai_profiles": [{"profile_name": "p", "created_by": null}]}),
            json!({"ai_profiles": [{"profile_name": "p"}], "next_offset": null}),
            json!({"ai_profiles": [{"profile_name": 5}]}),
            json!({"ai_profiles": [{"profile_name": "p", "revision": "1"}]}),
        ] {
            let f = fx();
            f.server
                .push_once(Route::Profiles, Planned::json(200, &body));
            let out = f.run(LIST);
            assert_failed(&out, &op_error(DECODE, None, &[]));
            assert_eq!(f.server.requests_for(Route::Profiles).len(), 1, "{body}");
        }
    }

    #[test]
    fn schema_invalid_topic_rows_are_decode_errors_d38() {
        for body in [
            json!({"custom_topics": [{"topic_name": "t", "revision": 1}]}),
            json!({"custom_topics": [{"topic_name": "t", "description": "d"}]}),
            json!({"custom_topics": [{"topic_name": "t", "revision": 1, "description": "d", "active": null}]}),
        ] {
            let f = fx();
            f.server.push_once(Route::Topics, Planned::json(200, &body));
            let out = f.run(TLIST);
            assert_failed(&out, &op_error(DECODE, None, &[]));
        }
    }

    #[test]
    fn non_json_success_body_is_decode_error_d38() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::raw(200, "application/json", "{not json"),
        );
        let out = f.run(LIST);
        assert_failed(&out, &op_error(DECODE, None, &[]));
    }

    #[test]
    fn decode_error_mid_walk_writes_no_partial_data() {
        let f = fx();
        f.profiles(
            (0..3)
                .map(|n| minimal_profile(&format!("p{n}"), 1))
                .collect(),
        );
        // The first page is served normally, the second is invalid.
        f.server.push_once(
            Route::Profiles,
            Planned::json(
                200,
                &json!({"ai_profiles": [minimal_profile("p0", 1)], "next_offset": 1}),
            ),
        );
        f.server.push_once(
            Route::Profiles,
            Planned::json(200, &json!({"ai_profiles": [{"revision": 1}]})),
        );
        let out = f.run(&with(LIST, &["--all", "--limit", "1"]));
        assert_failed(&out, &op_error(DECODE, None, &[]));
    }

    /// D30: redirects are never followed; the Location target is never contacted.
    #[test]
    fn redirect_is_an_error_and_target_never_contacted_d30() {
        let f = fx();
        f.server.push_once(
            Route::Profiles,
            Planned::redirect(302, &f.server.url("/elsewhere")),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error(
                "AISEC_CLIENT_SIDE_ERROR:Redirect responses are not followed",
                Some(302),
                &[],
            ),
        );
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
        assert!(f.server.requests_for(Route::Unknown).is_empty());
    }

    #[test]
    fn redirect_on_token_endpoint_is_not_followed_d30() {
        let f = fx();
        f.server.push_once(
            Route::Token,
            Planned::redirect(307, &f.server.url("/elsewhere")),
        );
        let out = f.run(LIST);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert_eq!(f.lines(), [TOKEN_LINE.to_owned()]);
    }

    /// §2.5: SIGINT during a hung read cancels it, prints the interrupt line, exits 130.
    #[test]
    fn sigint_during_hung_request_exits_130_without_data() {
        let f = fx();
        f.server.set_default(Route::Profiles, Planned::hang());
        let before = f.sandbox.snapshot();
        let child = spawn(&rust(), &f.sandbox, LIST, &RunOptions::new());
        assert!(
            f.server.wait_for_requests(2, Duration::from_secs(20)),
            "the GET never arrived"
        );
        child.send_signal("INT");
        let out = child.wait();
        f.sandbox.assert_unchanged(&before);
        out.assert_code(130);
        assert_eq!(out.stdout, "");
        assert_eq!(
            out.stderr,
            "\n  ✗ Error: Interrupted; no data was written\n\n"
        );
        assert!(
            f.server.wait_for_disconnects(1, Duration::from_secs(10)),
            "the in-flight request was not cancelled"
        );
    }

    /// D15: a lookup that cannot complete within the SDK's 10,000-row bound fails
    /// instead of reporting a false not-found.
    #[test]
    fn lookup_beyond_10000_rows_is_an_error_not_a_false_not_found_d15() {
        let f = fx();
        let mut rows: Vec<Value> = (0..10_000)
            .map(|n| json!({"profile_name": format!("p{n}")}))
            .collect();
        rows.push(json!({"profile_name": "needle", "revision": 1}));
        f.profiles(rows);
        let out = f.run_with(
            &["runtime", "profiles", "get", "needle"],
            &RunOptions::new().timeout(Duration::from_secs(120)),
        );
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert!(
            !out.stderr.contains("not found"),
            "false not-found: {}",
            out.stderr
        );
        assert_eq!(
            out.stderr,
            op_error(
                "AISEC_SDK_ERROR:Response exceeded a client safety limit",
                None,
                &[]
            )
        );
    }
}

// ---------------------------------------------------------------------------
// Review fixes: refreshed-token redaction (§6), >10,000-row topic pages (§7.2),
// empty intermediate pages (D46), proxies (§2.6, D36), Basic encoding (D29)
// ---------------------------------------------------------------------------

mod review_fixes {
    use super::*;

    /// §6: the SDK answers a 401 with one refresh and one replay; the replay's new token
    /// is registered too, so a server echoing it in `x-request-id` and the error code
    /// gets neither printed. Every issued token is distinct; the runner's leak check
    /// (raw `mock-token-` prefix and its base64) guards both streams.
    #[test]
    fn a_refreshed_token_echoed_after_a_401_replay_is_never_printed() {
        let f = fx();
        f.server.set_rotating_tokens(true);
        f.server.set_default(
            Route::Profiles,
            Planned::json(401, &json!({"code": Planned::BEARER}))
                .with_header("x-request-id", Planned::BEARER)
                .echoing_bearer(),
        );
        let out = f.run(LIST);
        assert_failed(
            &out,
            &op_error("AISEC_CLIENT_SIDE_ERROR:API error 401", Some(401), &[]),
        );
        let bearers: Vec<String> = f
            .server
            .requests_for(Route::Profiles)
            .iter()
            .filter_map(|r| r.header("authorization").map(str::to_owned))
            .collect();
        assert_eq!(
            bearers,
            ["Bearer mock-token-100.r1", "Bearer mock-token-100.r2"],
            "token, request, token, request"
        );
    }

    fn many_topics(count: u64) -> Vec<Value> {
        (0..count)
            .map(|n| json!({"topic_name": format!("t{n:05}"), "revision": 1, "description": "d"}))
            .collect()
    }

    /// §7.2: page listings walk under the `--all` bounds, so a tenant with more than
    /// 10,000 topic rows lists like `--all` does (the SDK default would fail it).
    #[test]
    fn topic_pages_work_beyond_10000_rows() {
        let f = fx();
        f.topics(many_topics(10_050));
        let long = RunOptions::new().timeout(Duration::from_secs(120));
        let out = f.run_with(&with(TLIST, &["--limit", "2"]), &long);
        out.assert_success();
        assert_eq!(names(&out.json(), "topicName"), ["t00000", "t00001"]);
        assert_eq!(out.stderr, "  Showing 2 (more available)\n");
        let out = f.run_with(
            &with(
                TLIST,
                &["--all-versions", "--limit", "2", "--offset", "10049"],
            ),
            &long,
        );
        out.assert_success();
        assert_eq!(names(&out.json(), "topicName"), ["t10049"]);
        assert_eq!(out.stderr, "  Showing 1 of 10050\n");
    }

    /// D46: an empty page whose `next_offset` points further fails the walk (the SDK's
    /// fail-closed guard) with no partial data; TS keeps following the cursor.
    #[test]
    fn an_empty_intermediate_page_fails_the_walk_d46() {
        let pagination = op_error(
            "AISEC_SDK_ERROR:Pagination stopped: the service returned a repeated cursor or an empty intermediate page",
            None,
            &[],
        );
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        f.server.push_once(
            Route::Profiles,
            Planned::json(200, &json!({"ai_profiles": [], "next_offset": 100})),
        );
        let out = f.run(&with(LIST, &["--all"]));
        assert_failed(&out, &pagination);
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );

        let f = fx();
        f.topics(vec![minimal_topic("alpha", 1)]);
        f.server.push_once(
            Route::Topics,
            Planned::json(200, &json!({"custom_topics": [], "next_offset": 200})),
        );
        let out = f.run(TLIST);
        assert_failed(&out, &pagination);
        assert_eq!(f.lines(), expect(&[topics_q("offset=0&limit=200")]));
    }

    /// §2.6, D36: plain-http (loopback) endpoints never go through a proxy, so the
    /// cleartext Basic pair and bearer cannot leave the host via `HTTP_PROXY`.
    #[test]
    fn plain_http_loopback_endpoints_bypass_proxy_variables() {
        let proxy = FixtureServer::start();
        let f = fx();
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let mut options = RunOptions::new().without_env("NO_PROXY");
        for name in ["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"] {
            options = options.env(name, proxy.base_url());
        }
        let out = f.run_with(LIST, &options);
        out.assert_success();
        assert_eq!(proxy.connections(), 0, "the proxy was contacted");
        assert_eq!(
            f.lines(),
            expect(&[profiles_q("offset=0&limit=100&latest=true")])
        );
    }

    /// D29: reserved characters in the client pair are RFC 6749 form-encoded before
    /// base64 (the fixture accepts either encoding, so the recorded encoding is checked).
    #[test]
    fn basic_credentials_with_reserved_characters_are_form_encoded_d29() {
        let f = fx_with(|config| {
            config.set("mgmtClientId", "client+/100");
            config.set("mgmtClientSecret", "FAKE+SECRET/=x y:z%é");
        });
        f.profiles(vec![fixtures::profile(1, "alpha", 1)]);
        let out = f.run(LIST);
        out.assert_success();
        let token = &f.server.requests_for(Route::Token)[0];
        assert_eq!(
            token.basic_encoding,
            Some(airs_test_harness::BasicEncoding::FormUrlEncoded)
        );
        assert_eq!(
            token.auth,
            AuthIdentity::Basic {
                client_id: "client+/100".into(),
                tsg: Some(TSG.into()),
                valid: true
            }
        );
    }
}

// ---------------------------------------------------------------------------
// scm-sdk 0.2.4: directional records read losslessly on every page (§15.25)
// ---------------------------------------------------------------------------

mod sdk_0_2_4_pages {
    use super::*;

    /// The TypeScript 7.3.0 directional fixture as `name` (id `uuid(n)`, `revision`),
    /// with the values scm-sdk 0.2.4 keeps only through its tolerant response binding: an
    /// unknown direction with an unknown member, an unknown mode, a `null` inspection
    /// setting, a `null` `dlp_tenant_id` and an unknown top-level key.
    fn unusual(n: u64, name: &str, revision: u64) -> Value {
        let mut profile = fixtures::runtime::directional_profile();
        profile["profile_id"] = json!(fixtures::uuid(n));
        profile["profile_name"] = json!(name);
        profile["revision"] = json!(revision);
        profile["dlp_tenant_id"] = Value::Null;
        profile["extra_top_level"] = json!({"kept": [1, null]});
        let ai = &mut profile["policy"]["ai-security-profiles"][0];
        ai["content-type-mode"] = json!("per_modality");
        ai["content-type-configurations"]["audio"] = json!({"model-protection": [{"name": "future-detector", "action": "alert"}], "future": null});
        ai["content-type-configurations"]
            .as_object_mut()
            .unwrap()
            .remove("tool-call");
        ai["model-configuration"]["enable-full-conversation-inspection"] = Value::Null;
        profile
    }

    /// 150 minimal records with the directional ones placed so that each walk needs its
    /// second page: `Near` (the plain fixture: a string `dlp_tenant_id`, an explicit
    /// `false`) at index 5, `Twice` revision 1 at index 10 and revision 2 at index 140,
    /// `Far` at index 130.
    fn rows() -> Vec<Value> {
        let mut rows: Vec<Value> = (0..150)
            .map(|n| minimal_profile(&format!("p{n}"), 1))
            .collect();
        let mut near = fixtures::runtime::directional_profile();
        near["profile_id"] = json!(fixtures::uuid(901));
        near["profile_name"] = json!("Near");
        rows[5] = near;
        rows[10] = unusual(902, "Twice", 1);
        rows[130] = unusual(903, "Far", 1);
        rows[140] = unusual(904, "Twice", 2);
        rows
    }

    /// The printed JSON keeps the record's policy and `dlp_tenant_id` exactly (`null`
    /// included); it is the §7.5 projection, so the unknown top-level key, `csp_id` and
    /// `tsg_id` are not printed (`profiles backup` keeps them, D215).
    fn assert_printed(printed: &Value, record: &Value) {
        assert_eq!(printed["profileName"], record["profile_name"]);
        assert_eq!(printed["policy"], record["policy"]);
        assert_eq!(printed.get("dlpTenantId"), record.get("dlp_tenant_id"));
        for absent in ["extra_top_level", "extraTopLevel", "tsg_id", "csp_id"] {
            assert!(printed.get(absent).is_none(), "{absent}: {printed}");
        }
    }

    /// `list --all` prints the page-2 record as sent, and every page is read.
    #[test]
    fn list_all_prints_every_page_losslessly() {
        let f = fx();
        let rows = rows();
        f.profiles(rows.clone());
        let out = f.run(&with(LIST, &["--all"]));
        out.assert_success();
        let printed = out.json();
        let named = |name: &str| {
            printed
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["profileName"] == name)
                .cloned()
                .unwrap()
        };
        assert_printed(&named("Near"), &rows[5]);
        assert_printed(&named("Far"), &rows[130]);
        assert_eq!(
            named("Near")["policy"]["ai-security-profiles"][0]["model-configuration"]["enable-full-conversation-inspection"],
            json!(false)
        );
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=100&latest=true"),
                profiles_q("offset=100&limit=100&latest=true"),
            ])
        );
    }

    /// `get <name>` finds the record on page 2 of its walk; `get --all-versions` prints
    /// both revisions, one per page, as sent.
    #[test]
    fn get_walks_print_page_two_losslessly() {
        let f = fx();
        let rows = rows();
        f.profiles(rows.clone());
        let out = f.run(&["runtime", "profiles", "get", "Far", "--output", "json"]);
        out.assert_success();
        assert_printed(&out.json(), &rows[130]);
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=100"),
                profiles_q("offset=100&limit=100"),
            ])
        );
        f.server.clear_requests();
        let out = f.run(&[
            "runtime",
            "profiles",
            "get",
            "Twice",
            "--all-versions",
            "--output",
            "json",
        ]);
        out.assert_success();
        let printed = out.json();
        let printed = printed.as_array().unwrap();
        assert_eq!(printed.len(), 2);
        assert_printed(&printed[0], &rows[10]);
        assert_printed(&printed[1], &rows[140]);
        assert_eq!(
            f.lines(),
            expect(&[
                profiles_q("offset=0&limit=100&latest=false"),
                profiles_q("offset=100&limit=100&latest=false"),
            ])
        );
    }
}
