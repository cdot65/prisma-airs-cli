//! Executable acceptance tests for the M6c step-1 model-security reads (DESIGN
//! §17.1–§17.7): `model-security groups|rules|rule-instances|scans|models list|ls|get`,
//! the scan sub-reads (`evaluations`, `evaluation`, `violations`, `violation`, `files`),
//! `models versions|version|files` and `labels keys|values`.
//!
//! Every test spawns the real `airs-cli-rs` against the loopback fixture server with the
//! seeded model-security dataset (`FixtureServer::seed_modelsec`). The runner's leak check
//! forbids every `FAKE-MS-SECRET-<nn>` marker on stderr and in the debug log of each run;
//! the tests assert where they may appear on stdout (§17.15). The read cases of the
//! TypeScript `tests/unit/airs/modelsecurity.spec.ts`,
//! `tests/unit/cli/modelsecurity-groupdetail-output.spec.ts`, `aliases.spec.ts:66-86`,
//! `help-examples.spec.ts:42` and `tests/unit/config/client-options.spec.ts` are ported
//! here (§17.17).

use std::time::Duration;

use airs_test_harness::fixtures::modelsec as fx;
use airs_test_harness::{
    Cli, ClosedPort, FixtureConfig, FixtureServer, Ms, Planned, RecordedRequest, Route, Run,
    RunOptions, Sandbox, debug_log, run, spawn,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Local helpers
// ---------------------------------------------------------------------------

const TSG: &str = "100";
const BANNER: &str = "\n  Prisma AIRS — Model Security\n  ML model supply chain security\n\n";
const GROUPS: &str = "/aims/mgmt/v1/security-groups";
const RULES: &str = "/aims/mgmt/v1/security-rules";
const SCANS: &str = "/aims/data/v1/scans";
const MODELS: &str = "/aims/data/v1/models";
const VERSIONS: &str = "/aims/data/v1/model-versions";

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

/// The seeded model-security dataset and one standard tenant `prod` (TSG 100).
fn fx() -> Fx {
    fx_with(|_| {})
}

fn fx_with(edit: impl FnOnce(&mut FixtureConfig)) -> Fx {
    let server = FixtureServer::start();
    server.seed_modelsec();
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

    /// `model-security <args>`, with the read-only and loopback checks.
    fn ms(&self, args: &[&str]) -> Run {
        self.ms_with(args, &RunOptions::new())
    }

    fn ms_with(&self, args: &[&str], options: &RunOptions) -> Run {
        let mut full = vec!["model-security"];
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

fn s(text: &str) -> String {
    text.to_owned()
}

/// The `uuid`s of a JSON list.
fn uuids(out: &Run) -> Vec<String> {
    out.json()
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["uuid"].as_str().unwrap_or_default().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// Request shapes (TS `unit/airs/modelsecurity.spec.ts`, §17.3–§17.7)
// ---------------------------------------------------------------------------

mod requests {
    use super::*;

    /// Every single page: `skip` and `limit` as TypeScript sends them (always both on
    /// groups, rules and scans; `limit` alone on the sub-lists; neither on the model lists
    /// unless given).
    #[test]
    fn single_pages_send_skip_and_limit_as_typescript() {
        let f = fx();
        let (g, s, m, v) = (u(1), u(401), u(801), u(901));
        for (args, line) in [
            (
                vec!["groups", "list"],
                format!("GET {GROUPS}?skip=0&limit=20"),
            ),
            (
                vec!["groups", "ls", "--limit", "5", "--offset", "3"],
                format!("GET {GROUPS}?skip=3&limit=5"),
            ),
            (vec!["rules", "ls"], format!("GET {RULES}?skip=0&limit=20")),
            (
                vec!["rule-instances", "ls", &g],
                format!("GET {GROUPS}/{g}/rule-instances?limit=20"),
            ),
            (
                vec!["rule-instances", "ls", &g, "--offset", "1"],
                format!("GET {GROUPS}/{g}/rule-instances?skip=1&limit=20"),
            ),
            (vec!["scans", "ls"], format!("GET {SCANS}?skip=0&limit=20")),
            (
                vec!["scans", "evaluations", &s],
                format!("GET {SCANS}/{s}/evaluations?limit=20"),
            ),
            (
                vec!["scans", "violations", &s, "--offset", "0"],
                format!("GET {SCANS}/{s}/rule-violations?skip=0&limit=20"),
            ),
            (
                vec!["scans", "files", &s],
                format!("GET {SCANS}/{s}/files?limit=20"),
            ),
            (vec!["models", "ls"], format!("GET {MODELS}")),
            (
                vec!["models", "ls", "--offset", "0"],
                format!("GET {MODELS}?skip=0"),
            ),
            (
                vec!["models", "versions", &m],
                format!("GET {MODELS}/{m}/model-versions"),
            ),
            (
                vec!["models", "files", &v, "--limit", "2"],
                format!("GET {VERSIONS}/{v}/files?limit=2"),
            ),
            (
                vec!["labels", "keys"],
                format!("GET {SCANS}/label-keys?limit=20"),
            ),
            (
                vec!["labels", "values", "env"],
                format!("GET {SCANS}/label-keys/env/values?limit=20"),
            ),
        ] {
            f.clear();
            let mut full = args.clone();
            full.extend(["--output", "json"]);
            f.ms(&full).assert_success();
            assert_eq!(f.api(), [line], "{args:?}");
        }
    }

    /// The filters of each list (`modelsecurity.spec.ts:150-174, :338-354, :445-455,
    /// :525-548, :729-739, :814-880, :897-945, :964-1000`): comma lists split and trimmed
    /// and repeated, `--search` as `search_query` on groups and rules, the single-value
    /// scan filters as one-element arrays, unset filters and empty values not sent.
    #[test]
    fn filters_are_sent_in_the_sdk_order() {
        let f = fx();
        let (g, s, m, v, r) = (u(1), u(401), u(801), u(901), u(201));
        for (args, line) in [
            (
                vec![
                    "groups",
                    "list",
                    "--source-types",
                    "LOCAL, S3",
                    "--search",
                    "Default",
                    "--sort-field",
                    "created_at",
                    "--sort-dir",
                    "desc",
                    "--enabled-rules",
                    &r,
                ],
                format!(
                    "GET {GROUPS}?skip=0&limit=20&sort_field=created_at&sort_dir=desc&source_types=LOCAL&source_types=S3&search_query=Default&enabled_rules={r}"
                ),
            ),
            (
                vec![
                    "rules",
                    "list",
                    "--source-type",
                    "LOCAL",
                    "--search",
                    "Load",
                ],
                format!("GET {RULES}?skip=0&limit=20&source_type=LOCAL&search_query=Load"),
            ),
            (
                vec![
                    "rule-instances",
                    "list",
                    &g,
                    "--security-rule-uuid",
                    &r,
                    "--state",
                    "BLOCKING",
                ],
                format!(
                    "GET {GROUPS}/{g}/rule-instances?limit=20&security_rule_uuid={r}&state=BLOCKING"
                ),
            ),
            (
                vec![
                    "scans",
                    "list",
                    "--model-version",
                    &v,
                    "--eval-outcome",
                    "BLOCKED",
                    "--source-type",
                    "LOCAL",
                    "--search",
                    "s1",
                    "--start",
                    "2026-01-01T00:00:00Z",
                    "--end",
                    "2026-02-01T00:00:00+01:00",
                    "--limit",
                    "2",
                    "--offset",
                    "1",
                ],
                format!(
                    "GET {SCANS}?skip=1&limit=2&search=s1&model_version_uuid={v}&eval_outcomes=BLOCKED&source_types=LOCAL&start_time=2026-01-01T00%3A00%3A00Z&end_time=2026-02-01T00%3A00%3A00%2B01%3A00"
                ),
            ),
            (
                vec!["scans", "files", &s, "--type", "FILE", "--result", "FAILED"],
                format!("GET {SCANS}/{s}/files?limit=20&type=FILE&result=FAILED"),
            ),
            (
                vec![
                    "models",
                    "list",
                    "--search",
                    "a",
                    "--search-query",
                    "b",
                    "--sort-field",
                    "created_at",
                    "--sort-order",
                    "desc",
                    "--limit",
                    "1",
                    "--offset",
                    "1",
                ],
                format!(
                    "GET {MODELS}?skip=1&limit=1&search=a&search_query=b&sort_field=created_at&sort_order=desc"
                ),
            ),
            (
                vec![
                    "models",
                    "versions",
                    &m,
                    "--sort-order",
                    "asc",
                    "--limit",
                    "1",
                    "--offset",
                    "1",
                ],
                format!("GET {MODELS}/{m}/model-versions?skip=1&limit=1&sort_order=asc"),
            ),
            (
                vec!["groups", "list", "--search", "", "--sort-field", ""],
                format!("GET {GROUPS}?skip=0&limit=20"),
            ),
            (
                vec!["scans", "list", "--eval-outcome", "", "--scan-origin", ""],
                format!("GET {SCANS}?skip=0&limit=20"),
            ),
            (
                // TS's help example: an outcome the service does not know is sent.
                vec![
                    "scans",
                    "list",
                    "--eval-outcome",
                    "MALICIOUS",
                    "--limit",
                    "10",
                ],
                format!("GET {SCANS}?skip=0&limit=10&eval_outcomes=MALICIOUS"),
            ),
        ] {
            f.clear();
            let mut full = args.clone();
            full.extend(["--output", "json"]);
            f.ms(&full).assert_success();
            assert_eq!(f.api(), [line], "{args:?}");
        }
    }

    /// Gets are one request each on their plane; a label key is one encoded segment.
    #[test]
    fn gets_and_label_keys_address_one_record() {
        let f = fx();
        let (g, i, r, s, e, v, m, mv) =
            (u(1), u(301), u(201), u(401), u(501), u(601), u(801), u(901));
        for (args, line) in [
            (vec!["groups", "get", &g], format!("GET {GROUPS}/{g}")),
            (vec!["rules", "get", &r], format!("GET {RULES}/{r}")),
            (
                vec!["rule-instances", "get", &g, &i],
                format!("GET {GROUPS}/{g}/rule-instances/{i}"),
            ),
            (vec!["scans", "get", &s], format!("GET {SCANS}/{s}")),
            (
                vec!["scans", "evaluation", &e],
                format!("GET /aims/data/v1/evaluations/{e}"),
            ),
            (
                vec!["scans", "violation", &v],
                format!("GET /aims/data/v1/violations/{v}"),
            ),
            (vec!["models", "get", &m], format!("GET {MODELS}/{m}")),
            (
                vec!["models", "version", &mv],
                format!("GET {VERSIONS}/{mv}"),
            ),
            (
                vec!["labels", "values", "a/b c?d"],
                format!("GET {SCANS}/label-keys/a%2Fb%20c%3Fd/values?limit=20"),
            ),
        ] {
            f.clear();
            f.ms(&args).assert_success();
            assert_eq!(f.api(), [line], "{args:?}");
        }
        // Upper-case UUIDs are valid (the SDK and zod rule).
        f.clear();
        let upper = "ABCDEF01-1111-4111-8111-00000000000A";
        f.ms(&["groups", "get", upper]).assert_code(1);
        assert_eq!(f.api(), [format!("GET {GROUPS}/{upper}")]);
    }

    /// `--all` walks from `skip=0` with `--limit` (else 20, or 50 on the model lists) as
    /// the page size and stops at `--max` with the D14 line (D183).
    #[test]
    fn all_walks_every_page_and_stops_at_max() {
        let f = fx();
        f.server.modelsec(|data| {
            data.groups = (1..=5)
                .map(|n| fx::group(n, &format!("group {n}"), "", "LOCAL", "ACTIVE"))
                .collect();
        });
        let out = f.ms(&[
            "groups", "list", "--all", "--limit", "2", "--output", "json",
        ]);
        out.assert_success();
        assert_eq!(uuids(&out).len(), 5);
        assert_eq!(
            f.api(),
            [
                format!("GET {GROUPS}?skip=0&limit=2"),
                format!("GET {GROUPS}?skip=2&limit=2"),
                format!("GET {GROUPS}?skip=4&limit=2"),
            ]
        );
        assert_eq!(out.stderr, "");
        f.clear();
        let out = f.ms(&[
            "groups", "list", "--all", "--limit", "2", "--max", "3", "--output", "json",
        ]);
        assert_eq!(uuids(&out).len(), 3);
        assert_eq!(
            out.stderr,
            "  Showing first 3 (stopped at --max 3; more available)\n"
        );
        assert_eq!(f.api().len(), 2);
        f.clear();
        f.ms(&["models", "list", "--all", "--output", "json"])
            .assert_success();
        assert_eq!(f.api(), [format!("GET {MODELS}?skip=0&limit=50")]);
        // `--max` without `--all` is validated, then ignored (TS).
        f.clear();
        f.ms(&["groups", "list", "--max", "1", "--output", "json"])
            .assert_success();
        assert_eq!(f.api(), [format!("GET {GROUPS}?skip=0&limit=20")]);
    }

    /// D183: the Rust-only paging flags of the sub-lists, the label lists and the model
    /// version lists, and `rule-instances list --all`.
    #[test]
    fn sub_lists_gain_the_read_contract() {
        let f = fx();
        f.server.modelsec(|data| {
            data.label_keys = (0..5).map(|n| format!("key-{n}")).collect();
        });
        let out = f.ms(&[
            "labels", "keys", "--all", "--limit", "2", "--output", "json",
        ]);
        out.assert_success();
        assert_eq!(
            out.json(),
            json!(["key-0", "key-1", "key-2", "key-3", "key-4"])
        );
        assert_eq!(f.api().len(), 3);
        f.clear();
        let out = f.ms(&[
            "labels", "keys", "--limit", "2", "--offset", "2", "--output", "json",
        ]);
        assert_eq!(out.json(), json!(["key-2", "key-3"]));
        assert_eq!(out.stderr, "  Showing 2 of 5 (next --offset 4)\n");
        assert_eq!(f.api(), [format!("GET {SCANS}/label-keys?skip=2&limit=2")]);
        let g = u(1);
        f.clear();
        let out = f.ms(&[
            "rule-instances",
            "list",
            &g,
            "--all",
            "--limit",
            "1",
            "--output",
            "json",
        ]);
        assert_eq!(uuids(&out), [u(301), u(302)]);
        assert_eq!(
            f.api(),
            [
                format!("GET {GROUPS}/{g}/rule-instances?skip=0&limit=1"),
                format!("GET {GROUPS}/{g}/rule-instances?skip=1&limit=1"),
            ]
        );
        let m = u(801);
        f.clear();
        let out = f.ms(&[
            "models", "versions", &m, "--all", "--limit", "1", "--output", "json",
        ]);
        assert_eq!(uuids(&out), [u(901), u(902)]);
        let s = u(401);
        f.clear();
        let out = f.ms(&[
            "scans", "files", &s, "--all", "--max", "2", "--limit", "1", "--output", "json",
        ]);
        assert_eq!(uuids(&out), [u(701), u(702)]);
        assert_eq!(
            out.stderr,
            "  Showing first 2 (stopped at --max 2; more available)\n"
        );
    }

    /// Choice 6: a status line only when a page does not show everything.
    #[test]
    fn status_lines_say_when_a_page_is_incomplete() {
        let f = fx();
        let out = f.ms(&["groups", "list", "--limit", "1"]);
        out.assert_success();
        assert_eq!(out.stderr, "  Showing 1 of 2 (next --offset 1)\n");
        let out = f.ms(&["groups", "list"]);
        assert_eq!(out.stderr, "");
        f.server.modelsec(|data| data.omit_totals = true);
        let out = f.ms(&["groups", "list", "--limit", "2", "--output", "table"]);
        assert_eq!(
            out.stderr,
            "  Showing 2 (more may be available; use --all)\n"
        );
        let out = f.ms(&["--quiet", "groups", "list", "--limit", "2"]);
        assert_eq!(out.stderr, "", "status lines are quiet-suppressed");
        let out = f.ms(&["groups", "list", "--limit", "3"]);
        assert_eq!(out.stderr, "");
    }

    /// Choice 4: `scans list --search` is `search` on a single page and `search_query` on
    /// a walk (TS per mode).
    #[test]
    fn scan_search_is_sent_per_mode() {
        let f = fx();
        f.ms(&["scans", "list", "--search", "x", "--output", "json"])
            .assert_success();
        f.ms(&[
            "scans", "list", "--search", "x", "--all", "--output", "json",
        ])
        .assert_success();
        f.ms(&[
            "scans",
            "list",
            "--search",
            "x",
            "--scan-origin",
            "MODEL_SECURITY_SDK",
            "--output",
            "json",
        ])
        .assert_success();
        assert_eq!(
            f.api(),
            [
                format!("GET {SCANS}?skip=0&limit=20&search=x"),
                format!("GET {SCANS}?skip=0&limit=20&search_query=x"),
                format!("GET {SCANS}?skip=0&limit=100&search_query=x"),
            ]
        );
    }

    /// D181: `--scan-origin` walks every page of 100 with the other filters, keeps the
    /// rows of that origin exactly (a row without one dropped), then slices.
    #[test]
    fn scan_origin_filters_locally_across_pages() {
        let f = fx();
        f.server.modelsec(|data| {
            data.scans = (1..=250u64)
                .map(|n| {
                    let origin = if n % 2 == 0 {
                        "MODEL_SECURITY_SDK"
                    } else {
                        "MODEL_SECURITY_API"
                    };
                    fx::scan(1000 + n, "ALLOWED", "", origin, None)
                })
                .collect();
        });
        // 125 rows match (n even); rows [110, 120) are n = 222, 224, …, 240.
        let out = f.ms(&[
            "scans",
            "list",
            "--scan-origin",
            "MODEL_SECURITY_SDK",
            "--eval-outcome",
            "ALLOWED",
            "--offset",
            "110",
            "--limit",
            "10",
            "--output",
            "json",
        ]);
        out.assert_success();
        let expected: Vec<String> = (111..=120u64).map(|k| u(1000 + 2 * k)).collect();
        assert_eq!(uuids(&out), expected);
        assert_eq!(out.stderr, "  Showing 10 of 125 (next --offset 120)\n");
        assert_eq!(
            f.api(),
            [
                format!("GET {SCANS}?skip=0&limit=100&eval_outcomes=ALLOWED"),
                format!("GET {SCANS}?skip=100&limit=100&eval_outcomes=ALLOWED"),
                format!("GET {SCANS}?skip=200&limit=100&eval_outcomes=ALLOWED"),
            ]
        );
        f.clear();
        let out = f.ms(&[
            "scans",
            "list",
            "--scan-origin",
            "MODEL_SECURITY_SDK",
            "--all",
            "--max",
            "7",
            "--output",
            "json",
        ]);
        assert_eq!(uuids(&out).len(), 7);
        assert_eq!(
            out.stderr,
            "  Showing first 7 (stopped at --max 7; more available)\n"
        );
        let out = f.ms(&["scans", "list", "--scan-origin", "NONE", "--output", "json"]);
        assert_eq!(out.json(), json!([]));
        assert_eq!(out.stderr, "  Showing 0 of 0\n");
        // The last slice has no next offset.
        let out = f.ms(&[
            "scans",
            "list",
            "--scan-origin",
            "MODEL_SECURITY_SDK",
            "--offset",
            "120",
            "--output",
            "json",
        ]);
        assert_eq!(uuids(&out).len(), 5);
        assert_eq!(out.stderr, "  Showing 5 of 125\n");
    }

    /// D182: once a scan page says `count_capped`, its total is ignored and the walk ends
    /// at a short page.
    #[test]
    fn a_capped_count_is_walked_to_the_short_page() {
        let f = fx();
        f.server.modelsec(|data| {
            data.scans = (401..=405)
                .map(|n| fx::scan(n, "ALLOWED", "", "MODEL_SECURITY_SDK", None))
                .collect();
            data.count_capped = Some(2);
        });
        let out = f.ms(&["scans", "list", "--all", "--limit", "2", "--output", "json"]);
        out.assert_success();
        assert_eq!(uuids(&out).len(), 5);
        assert_eq!(f.api().len(), 3);
    }

    /// D182 on a single page: a capped total is not a count, so a full capped page says
    /// more may be available whether the cap equals the rows shown or exceeds them.
    #[test]
    fn a_capped_single_page_never_looks_complete() {
        for cap in [2, 100] {
            let f = fx();
            f.server.modelsec(|data| {
                data.scans = (401..=406)
                    .map(|n| fx::scan(n, "ALLOWED", "", "MODEL_SECURITY_SDK", None))
                    .collect();
                data.count_capped = Some(cap);
            });
            let out = f.ms(&["scans", "list", "--limit", "2", "--output", "json"]);
            out.assert_success();
            assert_eq!(uuids(&out).len(), 2);
            assert_eq!(
                out.stderr, "  Showing 2 (more may be available; use --all)\n",
                "cap {cap}"
            );
        }
    }

    /// A service that caps its page below `--limit` is walked on while the total says
    /// more exist (the shared page loop, §17.3).
    #[test]
    fn a_capped_page_size_is_walked_to_the_end() {
        let f = fx();
        f.server.modelsec(|data| {
            data.groups = (1..=5)
                .map(|n| fx::group(n, &format!("group {n}"), "", "LOCAL", "ACTIVE"))
                .collect();
            data.page_cap = Some(2);
        });
        let out = f.ms(&[
            "groups", "list", "--all", "--limit", "10", "--output", "json",
        ]);
        out.assert_success();
        assert_eq!(uuids(&out).len(), 5);
        assert_eq!(
            f.api(),
            [
                format!("GET {GROUPS}?skip=0&limit=10"),
                format!("GET {GROUPS}?skip=2&limit=10"),
                format!("GET {GROUPS}?skip=4&limit=10"),
            ]
        );
    }
}

// ---------------------------------------------------------------------------
// Formats and documents (TS `modelsecurity-groupdetail-output.spec.ts`, §17.3–§17.7)
// ---------------------------------------------------------------------------

mod formats {
    use super::*;

    /// `groups get --output json` is the full projection; pretty keeps `Security Group
    /// Detail`; the list json is a bare array (`[]` when empty); the table has the TS
    /// columns.
    #[test]
    fn group_documents_in_every_format() {
        let f = fx();
        let g = u(1);
        let out = f.ms(&["groups", "get", &g, "--output", "json"]);
        out.assert_success();
        assert_eq!(
            out.stdout,
            format!(
                "{{\n  \"uuid\": \"{g}\",\n  \"name\": \"Default LOCAL\",\n  \"description\": \"Auto group\",\n  \"sourceType\": \"LOCAL\",\n  \"state\": \"ACTIVE\",\n  \"createdAt\": \"2026-01-01T00:00:00Z\",\n  \"updatedAt\": \"2026-01-01T00:00:00Z\"\n}}\n"
            )
        );
        let out = f.ms(&["groups", "get", &g]);
        assert!(
            out.stdout
                .starts_with(&format!("{BANNER}\n  Security Group Detail:\n\n"))
        );
        assert!(out.stdout.contains("  Description  Auto group\n"));
        let out = f.ms(&["groups", "get", &u(2)]);
        assert!(
            out.stdout.contains("  Description  (none)\n"),
            "{}",
            out.stdout
        );
        let out = f.ms(&["groups", "list", "--output", "json"]);
        assert_eq!(uuids(&out), [u(1), u(2)]);
        assert_eq!(out.json()[1]["sourceType"], "S3");
        let out = f.ms(&["groups", "list", "--output", "table"]);
        assert!(
            out.stdout.starts_with(
                " ID                                   │ Name          │ State   │ Source Type \n"
            ),
            "{}",
            out.stdout
        );
        let out = f.ms(&["groups", "get", &g, "--output", "table"]);
        assert!(
            out.stdout
                .starts_with(" Key         │ Value                                \n"),
            "{}",
            out.stdout
        );
        f.server.modelsec(|data| data.groups.clear());
        assert_eq!(f.ms(&["groups", "list", "--output", "json"]).stdout, "[]\n");
        assert_eq!(f.ms(&["groups", "list", "--output", "table"]).stdout, "");
        assert_eq!(
            f.ms(&["groups", "list"]).stdout,
            format!("{BANNER}  No security groups found\n")
        );
    }

    /// D189: the class B reads honour every format (TS prints pretty only).
    #[test]
    fn class_b_reads_honour_output() {
        let f = fx();
        let (s, e, v) = (u(401), u(501), u(601));
        let out = f.ms(&["scans", "evaluations", &s, "--output", "json"]);
        assert_eq!(out.json()[0]["violationCount"], 2);
        assert_eq!(out.json()[0]["ruleInstanceUuid"], json!(u(301)));
        let out = f.ms(&["scans", "evaluations", &s, "--output", "table"]);
        assert!(out.stdout.starts_with(" ID "), "{}", out.stdout);
        assert!(out.stdout.lines().next().unwrap().contains("│ Rule "));
        let out = f.ms(&["scans", "evaluation", &e, "--output", "json"]);
        assert_eq!(out.json()["ruleName"], "Load Time Code Execution Check");
        let out = f.ms(&["scans", "violation", &v, "--output", "table"]);
        assert!(
            out.stdout
                .contains(" threatDescription │ Arbitrary code at load time"),
            "{}",
            out.stdout
        );
        let out = f.ms(&["scans", "violations", &s, "--output", "json"]);
        assert_eq!(out.json()[0]["threat"], "PAIT-PKL-100");
        let out = f.ms(&["scans", "files", &s, "--output", "table"]);
        assert!(out.stdout.starts_with(" ID "));
        assert!(out.stdout.contains("model.pkl"));
        assert_eq!(
            f.ms(&["labels", "keys", "--output", "json"]).json(),
            json!(["env", "team"])
        );
        let out = f.ms(&["labels", "values", "env", "--output", "table"]);
        assert_eq!(out.stdout, " Value   \n─────────\n prod    \n staging \n");
        assert_eq!(
            f.ms(&["labels", "values", "none", "--output", "json"])
                .stdout,
            "[]\n"
        );
    }

    /// Choice 19: the tenant's `defaultOutput` applies to the class A reads only.
    #[test]
    fn default_output_applies_to_class_a_reads_only() {
        let f = fx_with(|config| {
            config.set("defaultOutput", "json");
        });
        assert!(f.ms(&["groups", "list"]).json().is_array());
        assert!(f.ms(&["models", "versions", &u(801)]).json().is_array());
        assert!(f.ms(&["scans", "get", &u(401)]).json().is_object());
        assert!(f.ms(&["labels", "keys"]).stdout.starts_with(BANNER));
        assert!(
            f.ms(&["scans", "evaluations", &u(401)])
                .stdout
                .starts_with(BANNER)
        );
        assert!(
            f.ms(&["groups", "list", "--output", "pretty"])
                .stdout
                .starts_with(BANNER)
        );
    }

    /// D190: `--quiet` keeps every data line (the ids of the pretty lists, the
    /// remediation URL) and drops the banner and the headings.
    #[test]
    fn quiet_keeps_the_data_lines() {
        let f = fx();
        let out = f.ms(&["--quiet", "scans", "list"]);
        assert_eq!(
            out.stdout,
            format!(
                "  {}\n    BLOCKED  MODEL_SECURITY_SDK  2026-01-01T00:00:00Z\n    hf://org/model\n    Rules: 3 passed  1 failed  / 4 total\n  {}\n    ALLOWED  MODEL_SECURITY_API  2026-01-01T00:00:00Z\n\n",
                u(401),
                u(402)
            )
        );
        let out = f.ms(&["--quiet", "rules", "get", &u(201)]);
        assert!(
            out.stdout.contains("  https://docs.example.invalid/r1\n"),
            "{}",
            out.stdout
        );
        assert!(!out.stdout.contains("Remediation:"));
    }

    /// D180: a summary without counts prints no `Rules` line and `null` counts; a file
    /// without `formats` and a scan without labels omit them in json.
    #[test]
    fn values_the_server_did_not_send_are_not_printed() {
        let f = fx();
        f.server.modelsec(|data| {
            data.scans.push(fx::pending_scan(403));
            data.files
                .push(fx::file(704, "x.bin", "FILE", "SUCCESS", None, 401, 901));
            data.versions.push(fx::uncounted_version(903));
        });
        let out = f.ms(&["scans", "get", &u(403)]);
        assert!(!out.stdout.contains("Rules"), "{}", out.stdout);
        let doc = f.ms(&["scans", "get", &u(403), "--output", "json"]).json();
        assert_eq!(
            doc["evalSummary"],
            json!({"rulesFailed": null, "rulesPassed": null, "totalRules": null})
        );
        assert!(doc.get("labels").is_none());
        let doc = f.ms(&["scans", "get", &u(402), "--output", "json"]).json();
        assert_eq!(
            doc["evalSummary"],
            Value::Null,
            "an absent summary is null (TS)"
        );
        let files = f
            .ms(&["scans", "files", &u(401), "--output", "json"])
            .json();
        assert!(files[3].get("formats").is_none());
        assert_eq!(files[1]["formats"], json!([]), "a sent empty list is kept");
        let table = f.ms(&["scans", "get", &u(403), "--output", "table"]).stdout;
        assert!(table.contains(" labels            │ "), "{table}");
        let doc = f
            .ms(&["models", "version", &u(903), "--output", "json"])
            .json();
        assert_eq!(
            doc["lastEvalSummary"],
            json!({"rulesFailed": null, "rulesPassed": null, "totalRules": null})
        );
        assert!(doc.get("fileCount").is_none());
        let doc = f
            .ms(&["models", "version", &u(902), "--output", "json"])
            .json();
        assert!(
            doc.get("lastEvalSummary").is_none(),
            "absent stays absent (TS)"
        );
        let pretty = f.ms(&["models", "versions", &u(801)]).stdout;
        assert!(pretty.contains("    pending  PENDING\n"), "{pretty}");
    }

    /// D191: a URI with userinfo, a signed query and a fragment is withheld in every
    /// format; its parts never reach stdout, stderr or the debug log.
    #[test]
    fn model_uris_are_withheld_everywhere() {
        let f = fx();
        f.server
            .modelsec(|data| data.scans.push(fx::credential_uri_scan(403)));
        let shown = "https://[withheld]@models.example.invalid/org/model.bin?[withheld]#[withheld]";
        let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
        for args in [
            vec!["--debug", "scans", "list"],
            vec!["--debug", "scans", "list", "--output", "json"],
            vec!["--debug", "scans", "list", "--output", "table"],
            vec!["--debug", "scans", "get", &u(403)],
            vec!["--debug", "scans", "get", &u(403), "--output", "json"],
            vec!["--debug", "scans", "get", &u(403), "--output", "table"],
        ] {
            let mut full = vec!["--debug", "model-security"];
            full.extend(args.iter().skip(1));
            let out = f.run_with(&full, &RunOptions::new().env("XDG_STATE_HOME", &state));
            out.assert_success();
            assert!(out.stdout.contains(shown), "{args:?}: {}", out.stdout);
            for n in 1..=4 {
                assert!(!out.stdout.contains(&fx::secret(n)), "{args:?}");
            }
        }
        let markers = fx::markers();
        let markers: Vec<&str> = markers.iter().map(String::as_str).collect();
        debug_log::assert_logs_clean(
            &f.sandbox,
            &f.server,
            &[debug_log::log_dir(&f.sandbox)],
            &markers,
        );
    }

    /// Choice 44: a presigned URI's ordinary query words (`host`, `3600`) never join the
    /// secret set, so names and label values that contain them stay visible; its
    /// signature is still withheld everywhere.
    #[test]
    fn presigned_uri_words_leave_names_and_labels_visible() {
        let f = fx();
        let signature = fx::secret(19);
        f.server.modelsec(|data| {
            let mut scan = fx::scan(
                451,
                "ALLOWED",
                &format!(
                    "https://bucket.s3.amazonaws.com/llm.bin?X-Amz-Expires=3600&X-Amz-SignedHeaders=host&X-Amz-Signature={signature}"
                ),
                "MODEL_SECURITY_SDK",
                None,
            );
            scan["security_group_name"] = json!("localhost models");
            scan["labels"] = json!([{"key": "ttl", "value": "3600"}, {"key": "owner", "value": "ghost-team"}]);
            data.scans.push(scan);
        });
        let list = f.ms(&["scans", "list", "--output", "json"]);
        list.assert_success();
        let row = list
            .json()
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["uuid"] == json!(u(451)))
            .cloned()
            .unwrap();
        assert_eq!(row["securityGroupName"], json!("localhost models"));
        assert_eq!(
            row["modelUri"],
            json!("https://bucket.s3.amazonaws.com/llm.bin?[withheld]")
        );
        let get = f.ms(&["scans", "get", &u(451)]);
        get.assert_success();
        for shown in ["localhost models", "3600", "ghost-team"] {
            assert!(get.stdout.contains(shown), "{shown}: {}", get.stdout);
        }
        let doc = f.ms(&["scans", "get", &u(451), "--output", "json"]).json();
        assert_eq!(
            doc["labels"],
            json!([{"key": "ttl", "value": "3600"}, {"key": "owner", "value": "ghost-team"}])
        );
        for out in [&list, &get] {
            assert!(!out.stdout.contains(&signature) && !out.stderr.contains(&signature));
        }
    }

    /// D192: a custom-rule instance is named and keeps its custom rule in json; an object
    /// field value is compact JSON.
    #[test]
    fn custom_rule_instances_are_named() {
        let f = fx();
        f.server.modelsec(|data| {
            data.rule_instances.push(fx::custom_rule_instance(303));
            data.rule_instances[1]["field_values"]["limits"] = json!({"max": 2});
        });
        let g = u(1);
        let out = f.ms(&["rule-instances", "list", &g]);
        assert!(
            out.stdout.contains("    Tenant Custom Rule  BLOCKING\n"),
            "{}",
            out.stdout
        );
        let out = f.ms(&["rule-instances", "list", &g, "--output", "table"]);
        assert!(
            out.stdout.contains(&format!("│ {} │ BLOCKING", u(311))),
            "{}",
            out.stdout
        );
        let out = f.ms(&["rule-instances", "get", &g, &u(303)]);
        assert!(
            out.stdout.contains(&format!("  Rule UUID   {}\n", u(311))),
            "{}",
            out.stdout
        );
        assert!(out.stdout.contains("  Rule Name   Tenant Custom Rule\n"));
        let doc = f
            .ms(&["rule-instances", "get", &g, &u(303), "--output", "json"])
            .json();
        assert_eq!(doc["customRuleUuid"], json!(u(311)));
        assert_eq!(
            doc["customRule"],
            json!({"description": "A tenant rule", "name": "Tenant Custom Rule", "uuid": u(311)})
        );
        assert!(doc.get("securityRuleUuid").is_none());
        let out = f.ms(&["rule-instances", "get", &g, &u(302)]);
        assert!(
            out.stdout.contains("  limits            {\"max\":2}\n"),
            "{}",
            out.stdout
        );
        let table = f
            .ms(&["rule-instances", "get", &g, &u(302), "--output", "table"])
            .stdout;
        assert!(
            !table.contains("customRule"),
            "the Rust keys only when sent: {table}"
        );
    }

    /// D52: control characters are replaced in every layout; descriptions keep their line
    /// breaks. Names carrying a tenant secret are withheld.
    #[test]
    fn server_text_is_terminal_safe_and_secrets_are_withheld() {
        let f = fx();
        f.server.modelsec(|data| {
            data.groups[0]["name"] = json!("Evil\u{1b}[31mRED");
            data.groups[0]["description"] = json!("line1\nline2\u{7}");
            data.groups[1]["name"] = json!("leaks FAKE-SECRET-100");
            data.label_keys.push("FAKE-SECRET-100".to_owned());
        });
        for args in [
            vec!["groups", "list"],
            vec!["groups", "list", "--output", "table"],
            vec!["groups", "get", &u(1)],
            vec!["groups", "get", &u(1), "--output", "table"],
        ] {
            let out = f.ms(&args);
            out.assert_success();
            assert!(!out.stdout.contains('\u{1b}'), "{args:?}");
            assert!(
                out.stdout.contains("Evil\u{fffd}[31mRED"),
                "{args:?}: {}",
                out.stdout
            );
        }
        assert!(
            f.ms(&["groups", "get", &u(1)])
                .stdout
                .contains("  Description  line1\nline2\u{fffd}\n")
        );
        let out = f.ms(&["groups", "list", "--output", "json"]);
        assert_eq!(out.json()[1]["name"], "[withheld]");
        assert_eq!(
            f.ms(&["labels", "keys", "--output", "json"]).json()[2],
            "[withheld]"
        );
    }

    /// Colour per stream (§2.8): the TS spans on a forced-colour stdout.
    #[test]
    fn colours_are_the_typescript_spans() {
        let f = fx();
        let out = f.ms_with(
            &["groups", "list"],
            &RunOptions::new()
                .without_env("NO_COLOR")
                .env("CLICOLOR_FORCE", "1"),
        );
        assert!(
            out.stdout.contains(&format!("  \u{1b}[2m{}\u{1b}[22m\n    Default LOCAL  \u{1b}[32mACTIVE\u{1b}[39m  source: \u{1b}[2mLOCAL\u{1b}[22m\n", u(1))),
            "{:?}",
            out.stdout
        );
        assert!(out.stdout.contains("\u{1b}[33mPENDING\u{1b}[39m"));
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

mod errors {
    use super::*;

    /// A 404 and a 500 are the §6 messages with their HTTP lines (exit 1, nothing on
    /// stdout); an error body is never echoed (D11).
    #[test]
    fn http_errors_are_the_section_6_messages() {
        let f = fx();
        let out = f.ms(&["groups", "get", &u(99)]);
        out.assert_code(1);
        assert_eq!(out.http_status(), Some(404));
        assert_eq!(out.stdout, "");
        f.server.set_default(
            Route::ModelSecurity(Ms::ScanList),
            Planned::json(
                500,
                &json!({"message": format!("echo {} \u{1b}[31m", fx::secret(5))}),
            )
            .with_retry_after(0),
        );
        let out = f.ms(&["scans", "list"]);
        out.assert_code(1);
        assert_eq!(out.http_status(), Some(500));
        assert_eq!(out.stdout, "");
        assert!(
            out.error_line()
                .unwrap()
                .starts_with("AISEC_SERVER_SIDE_ERROR:API error 500"),
            "{}",
            out.stderr
        );
        assert_eq!(
            f.server.ms_requests(Ms::ScanList).len(),
            1,
            "a 500 is not retried (D10)"
        );
        // A 503 is: 5 retries (the management read budget).
        f.server.set_default(
            Route::ModelSecurity(Ms::ScanList),
            Planned::json(503, &json!({})).with_retry_after(0),
        );
        f.clear();
        let out = f.ms(&["scans", "list"]);
        out.assert_code(1);
        assert_eq!(f.server.ms_requests(Ms::ScanList).len(), 6);
        f.server.push_once(
            Route::ModelSecurity(Ms::GroupGet),
            Planned::raw(400, "text/plain", format!("plain {}", fx::secret(6))),
        );
        let out = f.ms(&["groups", "get", &u(1)]);
        out.assert_code(1);
        assert_eq!(
            out.error_line().as_deref(),
            Some("AISEC_CLIENT_SIDE_ERROR:API error 400")
        );
    }

    /// D37: a service that ignores `skip` (repeated uuids) and an empty page before the
    /// total fail the walk; no partial list is printed.
    #[test]
    fn broken_walks_fail_without_printing_a_partial_list() {
        let f = fx();
        f.server.modelsec(|data| data.ignore_skip = true);
        let out = f.ms(&["groups", "list", "--all", "--limit", "1"]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert_eq!(
            out.error_line().as_deref(),
            Some(
                "AISEC_SDK_ERROR:Pagination stopped: the service returned a repeated cursor or an empty intermediate page"
            )
        );
        assert_eq!(f.api().len(), 2);
        f.clear();
        f.server.modelsec(|data| {
            data.ignore_skip = false;
            data.total_override = Some(9);
        });
        let out = f.ms(&["rules", "list", "--all"]);
        out.assert_code(1);
        assert!(
            out.error_line()
                .unwrap()
                .starts_with("AISEC_SDK_ERROR:Pagination stopped")
        );
        assert_eq!(
            f.api().len(),
            2,
            "the page of two rules, then the empty page"
        );
        // The --scan-origin walk is checked too.
        f.clear();
        let out = f.ms(&["scans", "list", "--scan-origin", "X"]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
    }

    /// A response that does not match the schema is a decode error (a missing required
    /// `model_uri`: both SDKs require it).
    #[test]
    fn a_broken_record_is_a_decode_error() {
        let f = fx();
        f.server.modelsec(|data| {
            data.scans[0].as_object_mut().unwrap().remove("model_uri");
        });
        let out = f.ms(&["scans", "get", &u(401)]);
        out.assert_code(1);
        assert_eq!(out.stdout, "");
        assert!(
            out.error_line()
                .unwrap()
                .starts_with("AISEC_RESPONSE_VALIDATION:"),
            "{}",
            out.stderr
        );
    }

    #[test]
    fn a_refused_connection_is_a_network_error() {
        let closed = ClosedPort::new();
        let f = fx_with(|config| {
            config.set(
                "modelSecDataEndpoint",
                format!("http://127.0.0.1:{}/aims/data", closed.port()),
            );
        });
        let out = f.ms(&["models", "list"]);
        out.assert_code(1);
        assert_eq!(
            out.error_line().as_deref(),
            Some(
                "AISEC_CLIENT_SIDE_ERROR:Network request failed (connection refused, reset or unreachable)"
            )
        );
    }

    /// §3.5: missing management credentials cost zero requests (TS `client-options.spec`:
    /// the model-security plane is the management OAuth plane).
    #[test]
    fn missing_management_credentials_send_nothing() {
        let f = fx_with(|config| {
            config.remove("mgmtClientSecret");
        });
        let out = f.ms(&["groups", "list"]);
        out.assert_code(1);
        assert_eq!(
            out.error_line().as_deref(),
            Some(
                "Management credentials are not configured (missing mgmtClientSecret). Run 'airs tenant set prod <key>' for mgmtClientSecret (secrets prompt hidden, or use --stdin)"
            )
        );
        assert_eq!(f.server.request_count(), 0);
        assert_eq!(out.stdout, "");
    }

    /// D48: an endpoint that is not https (or loopback http) is refused locally, the key
    /// named and the value never echoed.
    #[test]
    fn a_bad_endpoint_is_named_without_its_value() {
        for key in ["modelSecDataEndpoint", "modelSecMgmtEndpoint"] {
            let server = FixtureServer::start();
            let sandbox = Sandbox::new();
            let tenant = sandbox.add_fixture_tenant_allowing_external_endpoints(
                &server,
                "prod",
                TSG,
                |config| {
                    config.set(key, "http://aims.example.test/x");
                },
            );
            sandbox.register(&[&tenant], Some("prod"));
            let out = run(
                &rust(),
                &sandbox,
                &["model-security", "rules", "list"],
                &RunOptions::new(),
            );
            out.assert_code(1);
            assert_eq!(
                out.error_line(),
                Some(format!(
                    "Invalid {key}: endpoints must use https (http is allowed only for loopback hosts) and must not contain credentials, a query or a fragment"
                ))
            );
            assert!(!out.stderr.contains("example.test"));
            assert_eq!(server.request_count(), 0);
        }
    }

    /// A token failure is the OAuth message; no model-security request is sent.
    #[test]
    fn a_token_failure_sends_no_request() {
        let f = fx();
        f.server.push_once(Route::Token, Planned::empty_json(401));
        let out = f.ms(&["scans", "get", &u(401)]);
        out.assert_code(1);
        assert!(
            out.error_line().unwrap().starts_with("AISEC_OAUTH_ERROR:"),
            "{}",
            out.stderr
        );
        assert!(f.api().is_empty());
    }
}

// ---------------------------------------------------------------------------
// Planning (D184, D50)
// ---------------------------------------------------------------------------

mod checks {
    use super::*;

    /// Every argument error is a usage error before the tenant: no tenant is needed, no
    /// request is sent, no debug log is created, nothing is printed on stdout.
    #[test]
    fn argument_errors_are_usage_errors_before_the_tenant() {
        let server = FixtureServer::start();
        let sandbox = Sandbox::new();
        let state = debug_log::xdg_state_home(&sandbox).display().to_string();
        let g = u(1);
        let many = vec!["x"; 257].join(",");
        let long = "x".repeat(4097);
        for (args, message) in [
            (
                vec![s("groups"), s("get"), s("not-a-uuid")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid security group uuid: not-a-uuid"),
            ),
            (
                vec![s("rules"), s("get"), s("x")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid security rule uuid: x"),
            ),
            (
                vec![s("rule-instances"), s("get"), g.clone(), s("x")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid rule instance uuid: x"),
            ),
            (
                vec![s("scans"), s("get"), s("x\u{1b}y")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid scan uuid: x\u{fffd}y"),
            ),
            (
                vec![s("scans"), s("evaluation"), s("x")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid evaluation uuid: x"),
            ),
            (
                vec![s("scans"), s("violation"), s("x")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid violation uuid: x"),
            ),
            (
                vec![s("models"), s("versions"), s("x")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid model uuid: x"),
            ),
            (
                vec![s("models"), s("files"), s("x")],
                s("AISEC_USER_REQUEST_PAYLOAD_ERROR:Invalid model version uuid: x"),
            ),
            (
                vec![s("labels"), s("values"), s("..")],
                s("Invalid label key: .."),
            ),
            (
                vec![s("labels"), s("values"), s("")],
                s("Invalid label key: "),
            ),
            (
                vec![s("labels"), s("values"), s("%2e%2E")],
                s("Invalid label key: %2e%2E"),
            ),
            (
                vec![s("scans"), s("list"), s("--start"), s("2026-01-01")],
                s("Use valid ISO timestamps with start <= end and a valid model-version UUID."),
            ),
            (
                vec![
                    s("scans"),
                    s("list"),
                    s("--start"),
                    s("2026-02-01T00:00Z"),
                    s("--end"),
                    s("2026-01-01T00:00Z"),
                ],
                s("Use valid ISO timestamps with start <= end and a valid model-version UUID."),
            ),
            (
                vec![s("scans"), s("list"), s("--model-version"), s("not-a-uuid")],
                s("Use valid ISO timestamps with start <= end and a valid model-version UUID."),
            ),
            (
                vec![s("models"), s("list"), s("--end"), s("2026-02-30T00:00Z")],
                s("Use valid ISO timestamps with start <= end and a valid model-version UUID."),
            ),
            (
                vec![s("groups"), s("list"), s("--source-types"), s("LOCAL,,S3")],
                s("--source-types values must not be empty"),
            ),
            (
                vec![s("groups"), s("list"), s("--enabled-rules"), s("")],
                s("--enabled-rules values must not be empty"),
            ),
            (
                vec![s("groups"), s("list"), s("--source-types"), many],
                s("--source-types accepts at most 256 values"),
            ),
            (
                vec![s("rules"), s("list"), s("--search"), long],
                s("--search must be at most 4096 bytes and contain no control characters"),
            ),
            (
                vec![s("scans"), s("list"), s("--scan-origin"), s("a\tb")],
                s("--scan-origin must be at most 4096 bytes and contain no control characters"),
            ),
            (
                vec![s("groups"), s("list"), s("--limit"), s("0")],
                s("--limit must be a positive integer"),
            ),
            (
                vec![s("groups"), s("list"), s("--limit"), s("abc")],
                s("--limit must be a positive integer"),
            ),
            (
                vec![s("groups"), s("list"), s("--limit"), s("1.5")],
                s("--limit must be a positive integer"),
            ),
            (
                vec![s("models"), s("list"), s("--limit"), s("-1")],
                s("--limit must be a positive integer"),
            ),
            (
                vec![s("rules"), s("list"), s("--offset"), s("-2")],
                s("--offset must be a non-negative integer"),
            ),
            (
                vec![s("models"), s("list"), s("--offset"), s("abc")],
                s("--offset must be a non-negative integer"),
            ),
            (
                vec![s("labels"), s("keys"), s("--max"), s("abc")],
                s("--max must be a non-negative integer"),
            ),
            (
                vec![s("groups"), s("list"), s("--all"), s("--offset"), s("0")],
                s("--all cannot be combined with --offset"),
            ),
            (
                vec![
                    s("rule-instances"),
                    s("list"),
                    g.clone(),
                    s("--all"),
                    s("--offset"),
                    s("1"),
                ],
                s("--all cannot be combined with --offset"),
            ),
            (
                vec![s("groups"), s("list"), s("--output"), s("yaml")],
                s("Invalid output format 'yaml'. Expected: pretty, table, json"),
            ),
            (
                vec![
                    s("scans"),
                    s("evaluations"),
                    u(401),
                    s("--output"),
                    s("csv"),
                ],
                s("Invalid output format 'csv'. Expected: pretty, table, json"),
            ),
            (
                vec![s("labels"), s("keys"), s("--output"), s("markdown")],
                s("Invalid output format 'markdown'. Expected: pretty, table, json"),
            ),
        ] {
            let mut full = vec![s("--debug"), s("model-security")];
            full.extend(args.iter().cloned());
            let full: Vec<&str> = full.iter().map(String::as_str).collect();
            let out = run(
                &rust(),
                &sandbox,
                &full,
                &RunOptions::new().env("XDG_STATE_HOME", &state),
            );
            out.assert_code(2);
            assert_eq!(out.stderr, format!("\n  ✗ {message}\n\n"), "{args:?}");
            assert_eq!(out.stdout, "");
            assert_eq!(server.request_count(), 0);
            assert!(
                debug_log::log_files(&debug_log::log_dir(&sandbox)).is_empty(),
                "{args:?} created a debug log"
            );
        }
    }

    /// The command tree (`aliases.spec.ts:66-86`, `help-examples.spec.ts:42`): `ls` on
    /// every list, the `scans list` and `models list` examples with the Rust program name,
    /// bare groups print their help on stderr (exit 2, D1), the commands of the later
    /// steps are not registered yet.
    #[test]
    fn the_command_tree_and_its_help() {
        let f = fx();
        for args in [
            vec!["groups", "ls"],
            vec!["rules", "ls"],
            vec!["rule-instances", "ls", &u(1)],
            vec!["scans", "ls"],
            vec!["models", "ls"],
        ] {
            let mut full = args.clone();
            full.extend(["--output", "json"]);
            f.ms(&full).assert_success();
        }
        let out = f.ms(&["scans", "list", "--help"]);
        out.assert_success();
        assert!(out.stdout.contains("Examples:\n  $ airs model-security scans list\n  $ airs model-security scans list --eval-outcome MALICIOUS --limit 10\n  $ airs model-security scans list --output json"), "{}", out.stdout);
        assert!(
            f.ms(&["models", "list", "--help"])
                .stdout
                .contains("  $ airs model-security models list")
        );
        for group in [
            &[][..],
            &["groups"],
            &["rules"],
            &["rule-instances"],
            &["scans"],
            &["models"],
            &["labels"],
        ] {
            let out = f.ms(group);
            out.assert_code(2);
            assert_eq!(out.stdout, "");
            assert!(
                out.stderr.contains("Usage: airs model-security"),
                "{group:?}: {}",
                out.stderr
            );
        }
        f.clear();
        let root = f.run_with(&["--help"], &RunOptions::new());
        assert!(
            root.stdout.contains(
                "  model-security  AI Model Security operations — groups, rules, scans\n"
            ),
            "{}",
            root.stdout
        );
        // Every command of the three steps, in TypeScript's sorted order (§17.2).
        let help = f.ms(&["--help"]).stdout;
        let names: Vec<&str> = help
            .lines()
            .skip_while(|line| *line != "Commands:")
            .skip(1)
            .take_while(|line| !line.trim().is_empty())
            .filter_map(|line| line.split_whitespace().next())
            .filter(|name| *name != "help")
            .collect();
        assert_eq!(
            names,
            [
                "groups",
                "install",
                "labels",
                "models",
                "pypi-auth",
                "rule-instances",
                "rules",
                "scans"
            ],
            "{help}"
        );
        for args in [
            vec!["groups", "create"],
            vec!["pypi-auth", "extra"],
            vec!["install", "--extras", "nope"],
            vec!["labels", "add", "x"],
        ] {
            f.ms(&args).assert_code(2);
        }
        assert_eq!(f.server.request_count(), 0);
    }
}

// ---------------------------------------------------------------------------
// Tenants and endpoints (TS `client-options.spec.ts:38,55,71,100-107,146-152`)
// ---------------------------------------------------------------------------

mod tenants {
    use super::*;
    use airs_test_harness::AuthIdentity;

    /// The selected tenant's management credentials and token endpoint; the management
    /// base for groups, rules and rule instances, the data base for scans, models and
    /// labels (each overridable); `mgmtEndpoint` unused; `PANW_*` variables ignored.
    #[test]
    fn the_selected_tenant_and_its_two_bases_are_used() {
        let server = FixtureServer::start();
        server.seed_modelsec();
        let sandbox = Sandbox::new();
        let prod = sandbox.add_fixture_tenant_with(&server, "prod", "100", |config| {
            config.set("mgmtEndpoint", server.url("/not-model-security"));
        });
        let dev = sandbox.add_fixture_tenant_with(&server, "dev", "200", |config| {
            config.set("mgmtEndpoint", server.url("/not-model-security"));
        });
        sandbox.register(&[&prod, &dev], Some("prod"));
        let trap = server.url("/env-trap");
        for (args, path) in [
            (vec!["groups", "list"], GROUPS),
            (vec!["rules", "list"], RULES),
            (vec!["scans", "list"], SCANS),
            (vec!["models", "list"], MODELS),
            (vec!["labels", "keys"], "/aims/data/v1/scans/label-keys"),
        ] {
            for (tenant, tsg) in [(None, "100"), (Some("dev"), "200")] {
                server.clear_requests();
                let mut full = Vec::new();
                if let Some(tenant) = tenant {
                    full.extend(["--tenant", tenant]);
                }
                full.push("model-security");
                full.extend(args.iter().copied());
                full.extend(["--output", "json"]);
                let options = RunOptions::new()
                    .env("PANW_MODEL_SEC_DATA_ENDPOINT", &trap)
                    .env("PANW_MODEL_SEC_MGMT_ENDPOINT", &trap)
                    .env("PANW_MGMT_CLIENT_ID", "env-client")
                    .env("PANW_MGMT_TSG_ID", "999");
                let out = run(&rust(), &sandbox, &full, &options);
                out.assert_success();
                let requests = server.requests();
                assert_eq!(requests.len(), 2, "{args:?}");
                assert_eq!(requests[0].route, Route::Token);
                assert!(
                    requests[1].path.starts_with(path),
                    "{args:?}: {}",
                    requests[1].path
                );
                match &requests[1].auth {
                    AuthIdentity::Bearer { tsg: Some(found) } => assert_eq!(found, tsg),
                    other => panic!("{other:?}"),
                }
                assert!(
                    requests[1].header("x-tsg-id").is_none(),
                    "no TSG header (TS)"
                );
            }
        }
        // The overrides move each plane: the management base on the other server.
        server.clear_requests();
        let f = fx_with(|config| {
            config.set("modelSecMgmtEndpoint", server.url("/aims/mgmt"));
        });
        f.ms(&["groups", "list", "--output", "json"])
            .assert_success();
        f.ms(&["scans", "list", "--output", "json"])
            .assert_success();
        assert_eq!(
            f.api(),
            [format!("GET {SCANS}?skip=0&limit=20")],
            "the data base stays"
        );
        assert_eq!(server.ms_requests(Ms::GroupList).len(), 1);
    }
}

// ---------------------------------------------------------------------------
// Debug log (D64, G23)
// ---------------------------------------------------------------------------

mod debug {
    use super::*;

    /// `--debug` writes status and headers only: no body, no query, no marker; the SDK
    /// operation name of every read (G23).
    #[test]
    fn every_read_records_its_operation_name_and_no_body() {
        let (g, i, r, s, e, v, m, mv) =
            (u(1), u(301), u(201), u(401), u(501), u(601), u(801), u(901));
        for (args, operation) in [
            (
                vec!["groups", "list", "--search", "secret-search"],
                "airs.model_security.groups.list",
            ),
            (vec!["groups", "get", &g], "airs.model_security.groups.get"),
            (
                vec!["rule-instances", "list", &g],
                "airs.model_security.groups.list_rule_instances",
            ),
            (
                vec!["rule-instances", "get", &g, &i],
                "airs.model_security.groups.get_rule_instance",
            ),
            (vec!["rules", "list"], "airs.model_security.rules.list"),
            (vec!["rules", "get", &r], "airs.model_security.rules.get"),
            (vec!["scans", "list"], "airs.model_security.scans.list"),
            (vec!["scans", "get", &s], "airs.model_security.scans.get"),
            (
                vec!["scans", "evaluations", &s],
                "airs.model_security.scans.get_evaluations",
            ),
            (
                vec!["scans", "evaluation", &e],
                "airs.model_security.scans.get_evaluation",
            ),
            (
                vec!["scans", "violations", &s],
                "airs.model_security.scans.get_violations",
            ),
            (
                vec!["scans", "violation", &v],
                "airs.model_security.scans.get_violation",
            ),
            (
                vec!["scans", "files", &s],
                "airs.model_security.scans.get_files",
            ),
            (
                vec!["models", "list"],
                "airs.model_security.models.list_models",
            ),
            (
                vec!["models", "get", &m],
                "airs.model_security.models.get_model",
            ),
            (
                vec!["models", "versions", &m],
                "airs.model_security.models.list_model_versions",
            ),
            (
                vec!["models", "version", &mv],
                "airs.model_security.models.get_model_version",
            ),
            (
                vec!["models", "files", &mv],
                "airs.model_security.models.list_model_version_files",
            ),
            (
                vec!["labels", "keys"],
                "airs.model_security.scans.get_label_keys",
            ),
            (
                vec!["labels", "values", "env"],
                "airs.model_security.scans.get_label_values",
            ),
        ] {
            let f = fx();
            f.server.modelsec(|data| {
                data.label_values
                    .insert("env".to_owned(), vec![fx::secret(9)]);
            });
            let state = debug_log::xdg_state_home(&f.sandbox).display().to_string();
            let mut full = vec!["--debug", "model-security"];
            full.extend(args.iter().copied());
            let out = f.run_with(&full, &RunOptions::new().env("XDG_STATE_HOME", &state));
            out.assert_success();
            let (path, records) = debug_log::only_log(&f.sandbox);
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                !text.contains("secret-search") && !text.contains("limit="),
                "no URL query: {args:?}"
            );
            assert!(!text.contains("Default LOCAL"), "no body: {args:?}");
            let operations: Vec<&str> = records
                .iter()
                .filter_map(|record| record["operation"].as_str())
                .collect();
            assert_eq!(operations, ["oauth.token", operation], "{args:?}");
            let markers = fx::markers();
            let markers: Vec<&str> = markers.iter().map(String::as_str).collect();
            debug_log::assert_logs_clean(
                &f.sandbox,
                &f.server,
                &[debug_log::log_dir(&f.sandbox)],
                &markers,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Interruption (D193)
// ---------------------------------------------------------------------------

mod interrupt {
    use super::*;

    /// A signal during a read writes no data (exit 130 / 143), a walk included.
    #[test]
    fn a_signal_during_a_read_writes_nothing() {
        for (signal, code, args) in [
            ("INT", 130, vec!["model-security", "groups", "list"]),
            (
                "TERM",
                143,
                vec!["model-security", "groups", "list", "--all"],
            ),
        ] {
            let f = fx();
            f.server
                .push_once(Route::ModelSecurity(Ms::GroupList), Planned::hang());
            let running = spawn(
                &rust(),
                &f.sandbox,
                &args,
                &RunOptions::new().expect_signal(),
            );
            assert!(f.server.wait_for_route(
                Route::ModelSecurity(Ms::GroupList),
                1,
                Duration::from_secs(10)
            ));
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
