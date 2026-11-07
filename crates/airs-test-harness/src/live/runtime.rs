//! The M6d live runtime-extras reads (DESIGN §10, "M6d Runtime extras", Reads).
//!
//! [`run_live_runtime_reads`] runs only when `tests/live.rs` passes the read gate
//! (`AIRS_RS_LIVE=1` and `AIRS_E2E_TENANT`), or against the loopback fixture server
//! through [`super::LiveTarget::loopback`]. It is its own run (run id, private store,
//! fingerprints, leak checks, 90 s per CLI run) and read-only on both CLIs; the TypeScript
//! CLI is sandboxed as in every live run.
//!
//! Two probes come first: Rust `runtime api-keys list --limit 1 --output table` (an HTTP
//! 401, 403 or 404 notes `api-keys unavailable` and skips the key checks) and Rust
//! `runtime dashboard applications --limit 1 --output json` (likewise `dashboard
//! unavailable` for the dashboard, sessions and consumption checks). Every check is
//! bounded to one page and the first listed item. Tenant content (names, ids, prompts,
//! responses, user ids, auth codes, policies) is compared in memory only: rows and notices
//! hold fixed texts. The TypeScript `api-keys list` and `customer-apps list` run only with
//! `--output table` (their json could print a key, D205; the app table has only `ID`,
//! `Name` and `Description`); the app json checks are Rust only, with the keys withheld
//! (choice 70). The TypeScript `customer-apps get` (every format prints the app's `raw`,
//! keys included) runs only after the Rust get failed, with `--output table` and its stdout
//! sent to `/dev/null`: an HTTP 403 on both CLIs notes the get unavailable, anything else
//! stays the failure; `scan-content` runs only with `--output-file` (choice 37);
//! the report and backup files are written into the run's sandbox, checked and removed at
//! once.

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Exec, LiveAbort, LivePlan, LiveReport, LiveRow, Session};
use crate::compare::json_semantic_eq;

/// The keys of a Rust `api-keys list` row (D207).
pub const KEY_ROW_KEYS: [&str; 5] = ["id", "name", "last8", "createdAt", "expiresAt"];

/// The table part of a stdout (from the first `│` line on; the runtime banner dropped).
pub fn table_part(stdout: &str) -> String {
    let lines: Vec<&str> = stdout.split_inclusive('\n').collect();
    match lines.iter().position(|line| line.contains('│')) {
        Some(start) => lines[start..].concat(),
        None => String::new(),
    }
}

/// The Rust and TypeScript `api-keys list --output table` agree cell by cell, except the
/// `Expires` column, which TypeScript leaves empty (D207).
pub fn key_tables_agree(rust: &str, ts: &str) -> bool {
    tables_agree(rust, ts, "Expires")
}

/// The Rust and TypeScript `customer-apps list --output table` agree cell by cell, except
/// the `ID` column, which TypeScript leaves empty for the API's `customer_appId` (D207).
pub fn app_tables_agree(rust: &str, ts: &str) -> bool {
    tables_agree(rust, ts, "ID")
}

/// Two tables agree cell by cell; in the `tolerated` column the TypeScript cell may be
/// empty (a D207 key TypeScript never reads).
fn tables_agree(rust: &str, ts: &str, tolerated: &str) -> bool {
    let (Some((rust_header, rust_rows)), Some((ts_header, ts_rows))) = (
        super::table_cells(&table_part(rust)),
        super::table_cells(&table_part(ts)),
    ) else {
        return false;
    };
    if rust_header != ts_header || rust_rows.len() != ts_rows.len() {
        return false;
    }
    let tolerated = rust_header.iter().position(|cell| cell == tolerated);
    rust_rows.iter().zip(&ts_rows).all(|(r, t)| {
        r.len() == t.len()
            && r.iter().zip(t).enumerate().all(|(index, (r, t))| {
                if Some(index) == tolerated {
                    t.is_empty() || r == t
                } else {
                    r == t
                }
            })
    })
}

/// A Rust customer-app json document (the list array, or the `get` object) holds no
/// key material: no `apiKey`/`api_key` member at any depth (D205).
pub fn apps_withheld(rust: &Value) -> bool {
    (rust.is_array() || rust.is_object()) && !mentions_key(rust)
}

/// A Rust `api-keys list --output json` document holds no key material: an array of
/// objects with only the projection keys (D205, D207), no `apiKey`/`api_key` anywhere.
pub fn keys_withheld(rust: &Value) -> bool {
    rust.as_array().is_some_and(|rows| {
        rows.iter().all(|row| {
            row.as_object()
                .is_some_and(|map| map.keys().all(|key| KEY_ROW_KEYS.contains(&key.as_str())))
        })
    }) && !mentions_key(rust)
}

fn mentions_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, child)| key == "apiKey" || key == "api_key" || mentions_key(child)),
        Value::Array(items) => items.iter().any(mentions_key),
        _ => false,
    }
}

/// The note of a `customer-apps get` that both CLIs were refused (choice 70).
pub const GET_FORBIDDEN: &str = "unavailable: HTTP 403 on both CLIs (no customer-app read access)";

/// Whether a stderr names HTTP 403: the Rust `    HTTP 403` line or a TypeScript
/// `(HTTP 403)`.
pub fn names_http_403(stderr: &str) -> bool {
    stderr
        .lines()
        .any(|line| line.trim() == "HTTP 403" || line.contains("(HTTP 403)"))
}

/// Both CLIs exited 1 with HTTP 403 on stderr (only exit codes and stderr are read).
fn both_forbidden(rust: &Exec, ts: &Exec) -> bool {
    [rust, ts]
        .iter()
        .all(|side| side.code == Some(1) && names_http_403(&side.stderr))
}

/// Whether `value` is one TypeScript prints for a value the server did not send (D209).
fn invented(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_f64() == Some(0.0),
        Value::String(text) => text.is_empty() || text == "-",
        Value::Null => true,
        _ => false,
    }
}

/// The `consumption <app>` documents agree after D209 (a TypeScript `0`, `''` or `"-"`
/// against a Rust `null` or absent value) and D210 (both one array; an app without
/// detectors is `[]` on the TS side and Rust rows of one app, each `"detector": null`).
pub fn consumption_agree(rust: &Value, ts: &Value) -> bool {
    /// D210: Rust rows of one app (`app_name`, `app_id`), each with `"detector": null`.
    fn no_detectors(rows: &[Value]) -> bool {
        let app = |row: &Value| (row.get("app_name").cloned(), row.get("app_id").cloned());
        rows.first().is_some_and(|first| {
            rows.iter().all(|row| {
                row.get("detector") == Some(&Value::Null)
                    && row.get("app_name").is_some_and(Value::is_string)
                    && app(row) == app(first)
            })
        })
    }
    fn agree(rust: &Value, ts: &Value) -> bool {
        match (rust, ts) {
            (Value::Object(r), Value::Object(t)) => {
                let keys: BTreeSet<&String> = r.keys().chain(t.keys()).collect();
                keys.into_iter().all(|key| match (r.get(key), t.get(key)) {
                    (Some(rv), Some(tv)) => agree(rv, tv) || (rv.is_null() && invented(tv)),
                    (None, Some(tv)) => invented(tv),
                    (Some(rv), None) => rv.is_null(),
                    (None, None) => true,
                })
            }
            (Value::Array(r), Value::Array(t)) => {
                r.len() == t.len() && r.iter().zip(t).all(|(r, t)| agree(r, t))
            }
            (r, t) => json_semantic_eq(r, t),
        }
    }
    match (rust, ts) {
        (Value::Array(rows), Value::Array(t)) if t.is_empty() && !rows.is_empty() => {
            no_detectors(rows)
        }
        _ => rust.is_array() && ts.is_array() && agree(rust, ts),
    }
}

/// The dashboard documents agree in shape (D35 key order ignored): the same keys where
/// both sides have a value, the same JSON types, the first array items alike. The rolling
/// windows move between the two runs, so counts and lengths are not compared.
pub fn same_shape(rust: &Value, ts: &Value) -> bool {
    match (rust, ts) {
        (Value::Object(r), Value::Object(t)) => {
            let keys: BTreeSet<&String> = r.keys().chain(t.keys()).collect();
            keys.into_iter().all(|key| match (r.get(key), t.get(key)) {
                (Some(rv), Some(tv)) => rv.is_null() || tv.is_null() || same_shape(rv, tv),
                (Some(Value::Null), None) | (None, Some(Value::Null)) => true,
                _ => false,
            })
        }
        (Value::Array(r), Value::Array(t)) => match (r.first(), t.first()) {
            (Some(r), Some(t)) => same_shape(r, t),
            _ => true,
        },
        (Value::Number(_), Value::Number(_))
        | (Value::String(_), Value::String(_))
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Null, Value::Null) => true,
        _ => false,
    }
}

/// The comparable parts of a runtime report file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReportShape {
    pub sections: Vec<String>,
    /// `(source — method, status, notes)` of each evidence row.
    pub evidence: Vec<(String, String, String)>,
    pub csp: bool,
    pub external: bool,
}

/// The section ids, the evidence rows, the CSP meta and any external reference of a
/// report.
pub fn report_shape(html: &str) -> Option<ReportShape> {
    if !html.starts_with("<!doctype html>") {
        return None;
    }
    let sections = html
        .match_indices("<section id=\"")
        .filter_map(|(at, mark)| {
            let rest = &html[at + mark.len()..];
            rest.find('"').map(|end| rest[..end].to_owned())
        })
        .collect();
    let evidence = html
        .find("<section id=\"evidence\"")
        .map(|at| {
            let section = &html[at..];
            let section = &section[..section.find("</section>").unwrap_or(section.len())];
            section
                .split("<tr>")
                .skip(1)
                .filter_map(|row| {
                    let cells: Vec<&str> = row
                        .split("<td>")
                        .skip(1)
                        .map(|cell| cell.split("</td>").next().unwrap_or_default())
                        .collect();
                    (cells.len() >= 6).then(|| {
                        (
                            cells[0].to_owned(),
                            cells[2].to_owned(),
                            cells[5].to_owned(),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let lower = html.to_ascii_lowercase();
    Some(ReportShape {
        sections,
        evidence,
        csp: lower.contains("<meta http-equiv=\"content-security-policy\""),
        external: lower.contains("src=") || lower.contains("http://") || lower.contains("https://"),
    })
}

/// The two reports agree: the same sections, evidence sources and statuses (D214: a
/// TypeScript `complete` profile source against a Rust `unavailable` one whose note is
/// the no-record-array note), a CSP and no external reference on both.
pub fn reports_agree(rust: &ReportShape, ts: &ReportShape) -> bool {
    const NO_ARRAY: &str = "The API returned no record array.";
    rust.sections == ts.sections
        && rust.csp
        && ts.csp
        && !rust.external
        && !ts.external
        && rust.evidence.len() == ts.evidence.len()
        && rust.evidence.iter().zip(&ts.evidence).all(|(r, t)| {
            r.0 == t.0
                && (r.1 == t.1
                    || (r.0.starts_with("Security profiles")
                        && r.1 == "unavailable"
                        && t.1 == "complete"
                        && r.2.contains(NO_ARRAY)))
        })
}

/// The Rust and TypeScript backups of one profile agree: the Rust file is the §18.15
/// format; its first record equals the TypeScript `profiles[0]` (D35; a TypeScript
/// `examples: []` tolerated against an absent one, G27, in topics); the topic sets are
/// equal by `[topic_id, revision]`.
pub fn backups_agree(rust: &Value, ts: &Value) -> bool {
    let format = rust.get("format").and_then(Value::as_str) == Some("airs-cli-rs.runtime-profiles")
        && rust.get("version").and_then(Value::as_u64) == Some(1);
    let profile = rust
        .pointer("/profiles/0/record")
        .zip(ts.pointer("/profiles/0"))
        .is_some_and(|(r, t)| json_semantic_eq(r, t));
    let keys = |topics: Option<&Value>, record: bool| -> Option<BTreeSet<String>> {
        topics?
            .as_array()?
            .iter()
            .map(|topic| {
                let topic = if record { topic.get("record")? } else { topic };
                Some(format!(
                    "{}|{}",
                    topic.get("topic_id")?.as_str()?,
                    crate::compare::js_normalize(topic.get("revision")?)
                ))
            })
            .collect()
    };
    let topics = keys(rust.get("topics"), true)
        .zip(keys(ts.get("topics"), false))
        .is_some_and(|(r, t)| r == t);
    format && profile && topics
}

/// The mode of a file (`None` when it is missing).
fn mode_of(path: &Path) -> Option<u32> {
    std::fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions().mode() & 0o777)
}

struct Reads<'s, 'p> {
    session: &'s mut Session<'p>,
}

/// The first dashboard bucket (`id`, `name`) of an applications overview.
fn first_bucket(doc: &Value) -> Option<(String, String)> {
    let item = doc.get("items")?.as_array()?.first()?;
    Some((
        item.get("id")?.as_str()?.to_owned(),
        item.get("name")?.as_str()?.to_owned(),
    ))
}

impl Reads<'_, '_> {
    fn row(&mut self, check: &str, rust: &Exec, ts: Option<&Exec>, agree: bool, note: &str) {
        let both_ran = rust.code.is_some() && ts.is_none_or(|ts| ts.code.is_some());
        self.session.report.rows.push(LiveRow {
            check: check.to_owned(),
            rust_exit: rust.exit_cell(),
            ts_exit: ts.map_or_else(|| "-".to_owned(), Exec::exit_cell),
            agree: agree && both_ran,
            rust_verdict: None,
            ts_verdict: None,
            note: note.to_owned(),
        });
    }

    fn rust(&self, check: &str, args: &[&str]) -> Result<Exec, LiveAbort> {
        self.session.exec(&self.session.plan.rust, check, args)
    }

    /// Both CLIs `--output json`; `compare` on the parsed documents.
    fn json(
        &mut self,
        check: &str,
        args: &[&str],
        compare: fn(&Value, &Value) -> bool,
        note: &str,
    ) -> Result<(Exec, Exec), LiveAbort> {
        let mut full = vec!["runtime"];
        full.extend_from_slice(args);
        full.extend(["--output", "json"]);
        let (rust, ts) = self.session.both(check, &full)?;
        let agree = rust.code == Some(0) && ts.code == Some(0) && compare(&rust.json(), &ts.json());
        self.row(
            check,
            &rust,
            Some(&ts),
            agree,
            if agree { note } else { "json differs" },
        );
        Ok((rust, ts))
    }

    fn steps(&mut self) -> Result<(), LiveAbort> {
        self.keys()?;
        self.apps_and_inventory()?;
        let bucket = self.dashboard()?;
        if let Some(bucket) = &bucket {
            self.consumption(bucket)?;
        }
        self.report()?;
        self.backup()
    }

    /// The API key checks: the TS table only (D205), the Rust json withheld.
    fn keys(&mut self) -> Result<(), LiveAbort> {
        let probe = self.rust(
            "api-keys list (probe)",
            &[
                "runtime", "api-keys", "list", "--limit", "1", "--output", "table",
            ],
        )?;
        if super::redteam::no_access(probe.code, &probe.stderr) {
            self.session.report.notices.push(
                "api-keys unavailable: the tenant has no API key access (HTTP 401, 403 or 404); the key checks were not run".into(),
            );
            return Ok(());
        }
        let args = [
            "runtime", "api-keys", "list", "--limit", "20", "--output", "table",
        ];
        let (rust, ts) = self
            .session
            .both("api-keys list --limit 20 (table)", &args)?;
        let agree = rust.code == Some(0)
            && ts.code == Some(0)
            && key_tables_agree(&rust.stdout, &ts.stdout);
        self.row(
            "api-keys list --limit 20 (table)",
            &rust,
            Some(&ts),
            agree,
            if agree {
                "cells equal; Expires empty on the TS side (D207)"
            } else {
                "tables differ"
            },
        );
        let json = self.rust(
            "api-keys list --limit 20 (rs json)",
            &[
                "runtime", "api-keys", "list", "--limit", "20", "--output", "json",
            ],
        )?;
        let withheld = json.code == Some(0) && keys_withheld(&json.json());
        self.row(
            "api-keys list --limit 20 (rs json)",
            &json,
            None,
            withheld,
            if withheld {
                "no key value printed (D205)"
            } else {
                "a key member was printed"
            },
        );
        Ok(())
    }

    /// The customer-app checks: the TS table only (its json and every `get` print the
    /// app's `raw`, keys included, D205), the Rust json withheld (choice 70).
    fn apps_and_inventory(&mut self) -> Result<(), LiveAbort> {
        let args = [
            "runtime",
            "customer-apps",
            "list",
            "--limit",
            "20",
            "--output",
            "table",
        ];
        let (rust, ts) = self
            .session
            .both("customer-apps list --limit 20 (table)", &args)?;
        let agree = rust.code == Some(0)
            && ts.code == Some(0)
            && app_tables_agree(&rust.stdout, &ts.stdout);
        self.row(
            "customer-apps list --limit 20 (table)",
            &rust,
            Some(&ts),
            agree,
            if agree {
                "cells equal; ID empty on the TS side (D207)"
            } else {
                "tables differ"
            },
        );
        let list = self.rust(
            "customer-apps list --limit 20 (rs json)",
            &[
                "runtime",
                "customer-apps",
                "list",
                "--limit",
                "20",
                "--output",
                "json",
            ],
        )?;
        let doc = list.json();
        let withheld = list.code == Some(0) && doc.is_array() && apps_withheld(&doc);
        self.row(
            "customer-apps list --limit 20 (rs json)",
            &list,
            None,
            withheld,
            if withheld {
                "no key value printed (D205)"
            } else {
                "a key member was printed"
            },
        );
        let first = doc
            .as_array()
            .and_then(|rows| rows.first())
            .and_then(|row| row.get("name"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        match first {
            Some(name) => {
                let got = self.rust(
                    "customer-apps get <first> (rs json)",
                    &["runtime", "customer-apps", "get", &name, "--output", "json"],
                )?;
                let got_doc = got.json();
                let ok = got.code == Some(0)
                    && got_doc.get("name").and_then(Value::as_str) == Some(name.as_str())
                    && apps_withheld(&got_doc);
                if ok {
                    self.row(
                        "customer-apps get <first> (rs json)",
                        &got,
                        None,
                        true,
                        "the listed app; no key value printed (D205)",
                    );
                } else {
                    self.get_unavailable(&name, &got)?;
                }
            }
            None => self
                .session
                .report
                .notices
                .push("customer-apps get skipped: the listing returned no row".into()),
        }
        let args = ["runtime", "deployment-profiles", "list", "--output", "json"];
        let (rust, ts) = self.session.both("deployment-profiles list", &args)?;
        let rust_doc = rust.json();
        let agree = rust.code == Some(0)
            && ts.code == Some(0)
            && (json_semantic_eq(&rust_doc, &ts.json())
                || (rust_doc == Value::Array(Vec::new())
                    && ts.stdout.contains("No deployment profiles found")));
        self.row(
            "deployment-profiles list",
            &rust,
            Some(&ts),
            agree,
            if agree {
                "json equal (D222)"
            } else {
                "json differs"
            },
        );
        let args = [
            "runtime",
            "scan-logs",
            "query",
            "--interval",
            "1",
            "--unit",
            "hours",
            "--limit",
            "5",
        ];
        let mut rust_args = args.to_vec();
        rust_args.extend([
            "--output",
            "json",
            "--max-pages",
            "1",
            "--max-events",
            "5",
            "--max-requests",
            "50",
            "--timeout",
            "30",
        ]);
        let rust = self.rust("scan-logs query", &rust_args)?;
        let ts = self
            .session
            .exec(&self.session.plan.ts, "scan-logs query", &args)?;
        let data = rust.json();
        let agree = matches!(rust.code, Some(0 | 1))
            && data.get("items").is_some_and(Value::is_array)
            && data.get("complete").is_some_and(Value::is_boolean)
            && ts.code == Some(1)
            && ts.stdout.is_empty()
            && ts.stderr.contains("Legacy scan-logs retrieval is broken");
        self.row(
            "scan-logs query",
            &rust,
            Some(&ts),
            agree,
            if agree {
                "bounded discovery; frozen TS stub (D338)"
            } else {
                "discovery envelope or frozen stub failed"
            },
        );
        Ok(())
    }

    /// A failed Rust `customer-apps get`: the TypeScript get runs once with `--output
    /// table` and its stdout discarded unread (every format prints the app's `raw`, keys
    /// included, D205); a 403 on both CLIs is the tenant's missing customer-app read
    /// access (choice 70), anything else the failure.
    fn get_unavailable(&mut self, name: &str, got: &Exec) -> Result<(), LiveAbort> {
        let check = "customer-apps get <first> (rs json)";
        if got.code == Some(0) {
            self.row(
                check,
                got,
                None,
                false,
                "another app, or a key member was printed",
            );
            return Ok(());
        }
        let ts = self.session.ts_exit_only(
            check,
            &["runtime", "customer-apps", "get", name, "--output", "table"],
        )?;
        if both_forbidden(got, &ts) {
            self.row(check, got, Some(&ts), true, GET_FORBIDDEN);
            self.session.report.notices.push(format!(
                "customer-apps get {GET_FORBIDDEN}; the app json check was not run"
            ));
        } else {
            self.row(
                check,
                got,
                Some(&ts),
                false,
                "the Rust get failed (the TS get not a 403 too)",
            );
        }
        Ok(())
    }

    /// The dashboard and sessions reads; the first bucket for consumption.
    fn dashboard(&mut self) -> Result<Option<(String, String)>, LiveAbort> {
        let probe = self.rust(
            "dashboard applications (probe)",
            &[
                "runtime",
                "dashboard",
                "applications",
                "--limit",
                "1",
                "--output",
                "json",
            ],
        )?;
        if probe.code != Some(0)
            && (super::redteam::no_access(probe.code, &probe.stderr)
                || ["HTTP 401", "HTTP 403", "HTTP 404"]
                    .iter()
                    .any(|status| probe.stderr.contains(&format!("({status})"))))
        {
            self.session.report.notices.push(
                "dashboard unavailable: the tenant has no dashboard access (HTTP 401, 403 or 404); the dashboard, sessions and consumption checks were not run".into(),
            );
            return Ok(None);
        }
        let shape = "same shape (D35; rolling windows)";
        let (rust, _) = self.json(
            "dashboard applications --limit 5",
            &["dashboard", "applications", "--limit", "5"],
            same_shape,
            shape,
        )?;
        let bucket = first_bucket(&rust.json());
        for (check, args) in [
            (
                "dashboard top-applications",
                vec!["dashboard", "top-applications"],
            ),
            (
                "dashboard violations-trend",
                vec!["dashboard", "violations-trend"],
            ),
            ("dashboard apps-list", vec!["dashboard", "apps-list"]),
            ("sessions chart", vec!["sessions", "chart"]),
        ] {
            self.json(check, &args, same_shape, shape)?;
        }
        match &bucket {
            Some((id, name)) => {
                for read in ["application", "application-violations"] {
                    self.json(
                        &format!("dashboard {read} <first bucket> --interval 7"),
                        &[
                            "dashboard",
                            read,
                            "--app-id",
                            id,
                            "--app-name",
                            name,
                            "--interval",
                            "7",
                        ],
                        same_shape,
                        shape,
                    )?;
                }
            }
            None => self
                .session
                .report
                .notices
                .push("dashboard application reads skipped: no bucket listed".into()),
        }
        let (rust, _) = self.json(
            "sessions list --limit 5",
            &["sessions", "list", "--limit", "5"],
            same_shape,
            shape,
        )?;
        let session = rust
            .json()
            .as_array()
            .and_then(|rows| rows.first())
            .and_then(|row| {
                Some((
                    row.get("session_id")?.as_str()?.to_owned(),
                    row.get("application_id")?.as_str()?.to_owned(),
                    row.get("application_name")?.as_str()?.to_owned(),
                ))
            });
        let Some((session_id, app_id, app_name)) = session else {
            self.session
                .report
                .notices
                .push("sessions get skipped: the listing returned no session".into());
            return Ok(bucket);
        };
        let identity = [
            "--session-id",
            session_id.as_str(),
            "--app-id",
            app_id.as_str(),
            "--app-name",
            app_name.as_str(),
        ];
        let mut args = vec!["sessions", "get"];
        args.extend(identity);
        args.extend(["--limit", "5"]);
        let (rust, _) = self.json("sessions get <first session>", &args, same_shape, shape)?;
        let action = rust
            .json()
            .get("session_actions")
            .and_then(Value::as_array)
            .and_then(|actions| actions.first())
            .and_then(|action| {
                Some((
                    action.get("scan_id")?.as_str()?.to_owned(),
                    crate::compare::js_normalize(action.get("scan_sub_req_id")?).to_string(),
                ))
            });
        let Some((scan_id, sub_request)) = action else {
            self.session
                .report
                .notices
                .push("sessions transaction skipped: the session lists no action".into());
            return Ok(bucket);
        };
        let mut args = vec!["sessions", "transaction"];
        args.extend(identity);
        args.extend([
            "--scan-id",
            scan_id.as_str(),
            "--scan-sub-req-id",
            sub_request.as_str(),
        ]);
        self.json(
            "sessions transaction <first action>",
            &args,
            same_shape,
            shape,
        )?;
        self.scan_content(&scan_id, &sub_request)?;
        Ok(bucket)
    }

    /// `scan-content` into each CLI's own 0600 file (choice 37), compared in memory and
    /// removed at once; neither stream may carry the content.
    fn scan_content(&mut self, scan_id: &str, sub_request: &str) -> Result<(), LiveAbort> {
        let mut runs = Vec::new();
        for (label, ts) in [("rs", false), ("ts", true)] {
            let path = self.dir(label)?.join("scan.json");
            let path_text = path.display().to_string();
            let cli = if ts {
                self.session.plan.ts.clone()
            } else {
                self.session.plan.rust.clone()
            };
            let exec = self.session.exec(
                &cli,
                "sessions scan-content --output-file",
                &[
                    "runtime",
                    "sessions",
                    "scan-content",
                    "--scan-id",
                    scan_id,
                    "--scan-sub-req-id",
                    sub_request,
                    "--output-file",
                    &path_text,
                ],
            )?;
            let mode = mode_of(&path);
            let doc: Option<Value> = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok());
            let _ = std::fs::remove_dir_all(path.parent().unwrap_or(&path));
            runs.push((exec, mode, doc));
        }
        let (rust, rust_mode, rust_doc) = &runs[0];
        let (ts, ts_mode, ts_doc) = &runs[1];
        let content: Vec<String> = rust_doc
            .iter()
            .chain(ts_doc)
            .filter_map(|doc| doc.get("scan_contents"))
            .flat_map(|contents| {
                ["prompt", "response"]
                    .map(|key| contents.get(key).and_then(Value::as_str).map(str::to_owned))
            })
            .flatten()
            .filter(|text| text.len() >= 8)
            .collect();
        let clean = |exec: &Exec| {
            content
                .iter()
                .all(|text| !exec.stdout.contains(text) && !exec.stderr.contains(text))
        };
        let agree = rust.code == Some(0)
            && ts.code == Some(0)
            && rust.stdout.is_empty()
            && ts.stdout.is_empty()
            && *rust_mode == Some(0o600)
            && *ts_mode == Some(0o600)
            && rust_doc
                .as_ref()
                .zip(ts_doc.as_ref())
                .is_some_and(|(r, t)| json_semantic_eq(r, t))
            && clean(rust)
            && clean(ts);
        self.row(
            "sessions scan-content --output-file",
            rust,
            Some(ts),
            agree,
            if agree {
                "0600 files equal; content on neither stream (D225)"
            } else {
                "scan-content files differ or content was printed"
            },
        );
        Ok(())
    }

    fn consumption(&mut self, (_, name): &(String, String)) -> Result<(), LiveAbort> {
        self.json(
            "customer-apps consumption <first bucket> --time-interval 7",
            &["customer-apps", "consumption", name, "--time-interval", "7"],
            consumption_agree,
            "json equal (D209, D210)",
        )?;
        Ok(())
    }

    /// `<sandbox>/tmp/<label>/`, created.
    fn dir(&self, label: &str) -> Result<PathBuf, LiveAbort> {
        let dir = self.session.sandbox.tmp().join(label);
        std::fs::create_dir_all(&dir)
            .map_err(|err| LiveAbort::Setup(format!("cannot create a run directory: {err}")))?;
        Ok(dir)
    }

    /// `runtime report`, both CLIs, into each CLI's own file (TS with `--output html`).
    fn report(&mut self) -> Result<(), LiveAbort> {
        let mut runs = Vec::new();
        for (label, ts) in [("rs", false), ("ts", true)] {
            let path = self.dir(label)?.join("report.html");
            let path_text = path.display().to_string();
            let mut args = vec!["runtime", "report", "--output-file", path_text.as_str()];
            let cli = if ts {
                args.extend(["--output", "html"]);
                self.session.plan.ts.clone()
            } else {
                args.extend([
                    "--max-pages",
                    "1",
                    "--max-events",
                    "5",
                    "--max-reports",
                    "0",
                    "--timeout",
                    "30",
                    "--max-requests",
                    "50",
                ]);
                self.session.plan.rust.clone()
            };
            let exec = self.session.exec(&cli, "runtime report", &args)?;
            let mode = mode_of(&path);
            let html = std::fs::read_to_string(&path).unwrap_or_default();
            let _ = std::fs::remove_dir_all(path.parent().unwrap_or(&path));
            runs.push((exec, mode, html));
        }
        let (rust, rust_mode, rust_html) = &runs[0];
        let (ts, ts_mode, ts_html) = &runs[1];
        let leak = super::leak_in(&self.session.secrets, rust_html).is_some();
        let agree = matches!(rust.code, Some(0 | 1))
            && matches!(ts.code, Some(0 | 1))
            && *rust_mode == Some(0o600)
            && *ts_mode == Some(0o600)
            && !leak
            && rust_html.contains("AIRS Signal Brief")
            && rust_html.contains("id=\"coverage\"")
            && rust_html.contains("Content-Security-Policy")
            && !rust_html.contains("<script")
            && ts_html.contains("Report schema 2");
        self.row(
            "runtime report --output-file",
            rust,
            Some(ts),
            agree,
            if agree {
                "0600 files; Signal Brief and frozen aggregate report (D340)"
            } else {
                "the reports differ"
            },
        );
        Ok(())
    }

    /// `runtime profiles backup "<first profile>"`, both CLIs, read-only on the tenant.
    fn backup(&mut self) -> Result<(), LiveAbort> {
        let listing = self.rust(
            "profiles list --limit 1",
            &[
                "runtime", "profiles", "list", "--limit", "1", "--output", "json",
            ],
        )?;
        let Some(name) = listing
            .json()
            .as_array()
            .and_then(|rows| rows.first())
            .and_then(|row| row.get("profileName"))
            .and_then(Value::as_str)
            .map(str::to_owned)
        else {
            self.session
                .report
                .notices
                .push("profiles backup skipped: the tenant lists no profile".into());
            return Ok(());
        };
        let mut runs = Vec::new();
        for (label, ts) in [("rs", false), ("ts", true)] {
            let path = self.dir(label)?.join("backup.json");
            let path_text = path.display().to_string();
            let cli = if ts {
                self.session.plan.ts.clone()
            } else {
                self.session.plan.rust.clone()
            };
            let exec = self.session.exec(
                &cli,
                "profiles backup <first profile>",
                &[
                    "runtime",
                    "profiles",
                    "backup",
                    &name,
                    "--output-file",
                    &path_text,
                ],
            )?;
            let mode = mode_of(&path);
            let doc: Option<Value> = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok());
            let _ = std::fs::remove_dir_all(path.parent().unwrap_or(&path));
            runs.push((exec, mode, doc));
        }
        let (rust, rust_mode, rust_doc) = &runs[0];
        let (ts, ts_mode, ts_doc) = &runs[1];
        let agree = rust.code == Some(0)
            && ts.code == Some(0)
            && *rust_mode == Some(0o600)
            && *ts_mode == Some(0o600)
            && rust_doc
                .as_ref()
                .zip(ts_doc.as_ref())
                .is_some_and(|(r, t)| backups_agree(r, t));
        self.row(
            "profiles backup <first profile>",
            rust,
            Some(ts),
            agree,
            if agree {
                "0600 files; the record and the topic keys equal (D215, G27)"
            } else {
                "the backups differ"
            },
        );
        Ok(())
    }
}

/// Runs the runtime-extras reads and returns the sanitized summary.
pub fn run_live_runtime_reads(plan: &LivePlan) -> Result<LiveReport, LiveAbort> {
    let mut session = Session::open(plan)?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut reads = Reads {
            session: &mut session,
        };
        reads.steps()?;
        reads.session.verify()?;
        reads.session.summary_check()
    }));
    for label in ["rs", "ts"] {
        let _ = std::fs::remove_dir_all(session.sandbox.tmp().join(label));
    }
    session.report.store_removed = Some(session.store.remove());
    match outcome {
        Err(panic) => std::panic::resume_unwind(panic),
        Ok(result) => result?,
    }
    session.report.rust_runs = session.rust_runs.get();
    session.report.ts_runs = session.ts_runs.get();
    Ok(session.report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_tables_tolerate_only_the_empty_typescript_expires() {
        let rust = " ID │ Name │ Last 8 │ Expires \n────┼──────┼────────┼─────────\n a  │ k    │ L      │ 2027    \n";
        let ts = "\n  banner\n\n ID │ Name │ Last 8 │ Expires \n────┼──────┼────────┼─────────\n a  │ k    │ L      │         \n";
        assert!(key_tables_agree(rust, ts));
        assert!(!key_tables_agree(rust, &ts.replace(" k ", " x ")));
        assert!(!key_tables_agree(
            &rust.replace("2027", "    "),
            &rust.replace(" L ", " M ")
        ));
        assert!(keys_withheld(
            &json!([{"id": "a", "name": "k", "last8": "L"}])
        ));
        assert!(!keys_withheld(&json!([{"id": "a", "apiKey": "x"}])));
        assert!(!keys_withheld(&json!([{"id": "a", "secret": "x"}])));
    }

    #[test]
    fn app_tables_tolerate_d207_and_app_json_withholds_keys() {
        let rust = "\n ID │ Name  │ Description \n────┼───────┼─────────────\n u  │ app-1 │             \n";
        let ts = "\n ID │ Name  │ Description \n────┼───────┼─────────────\n    │ app-1 │             \n";
        assert!(app_tables_agree(rust, ts), "an empty TS ID cell (D207)");
        assert!(app_tables_agree(rust, rust));
        assert!(!app_tables_agree(rust, &ts.replace("app-1", "app-2")));
        assert!(apps_withheld(
            &json!([{"name": "app-1", "id": "u", "raw": {"a": 1}}])
        ));
        assert!(apps_withheld(
            &json!({"name": "app-1", "raw": {"api_keys_dp_info": [{"auth_code": "c"}]}})
        ));
        assert!(!apps_withheld(
            &json!({"name": "app-1", "raw": {"api_keys_dp_info": [{"api_key": "x"}]}})
        ));
        assert!(!apps_withheld(&json!([{"apiKey": "x"}])));
        assert!(!apps_withheld(&Value::Null));
    }

    #[test]
    fn consumption_tolerates_invented_values() {
        let ts = json!([{"app": "a", "sessions_total": 0, "daily_avg": "-", "since": ""}]);
        let rust = json!([{"app": "a", "sessions_total": null, "daily_avg": null}]);
        assert!(consumption_agree(&rust, &ts));
        assert!(!consumption_agree(&json!([{"app": "b"}]), &ts));
        assert!(!consumption_agree(&json!({"app": "a"}), &ts));
    }

    #[test]
    fn consumption_accepts_d210_apps_without_detectors() {
        let row = |app: &str, detector: Value| json!({"app_name": app, "app_id": "u", "sessions_total": null, "detector": detector, "total": null});
        let ts = json!([]);
        assert!(consumption_agree(&json!([row("a", Value::Null)]), &ts));
        assert!(consumption_agree(
            &json!([row("a", Value::Null), row("a", Value::Null)]),
            &ts
        ));
        // A detector on the Rust side, two apps, or a row without an app is not D210.
        assert!(!consumption_agree(&json!([row("a", json!("dlp"))]), &ts));
        assert!(!consumption_agree(
            &json!([row("a", Value::Null), row("b", Value::Null)]),
            &ts
        ));
        assert!(!consumption_agree(&json!([{"detector": null}]), &ts));
        assert!(!consumption_agree(&json!([{"app_name": "a"}]), &ts));
        // Both empty still agree; a TS row against an empty Rust array does not.
        assert!(consumption_agree(&json!([]), &ts));
        assert!(!consumption_agree(&ts, &json!([row("a", Value::Null)])));
    }

    #[test]
    fn a_403_is_named_in_either_cli_format() {
        assert!(names_http_403("\n  ✗ Error: x\n    HTTP 403\n"));
        assert!(names_http_403("  ✗ Failed (HTTP 403). Try again\n"));
        assert!(!names_http_403("    HTTP 404\n"));
        assert!(!names_http_403("    HTTP 4030\n"));
        let exec = |code: Option<i32>, stderr: &str| Exec {
            code,
            stdout: String::new(),
            stderr: stderr.to_owned(),
            elapsed: std::time::Duration::ZERO,
        };
        let forbidden = exec(Some(1), "    HTTP 403\n");
        assert!(both_forbidden(&forbidden, &forbidden));
        assert!(!both_forbidden(&forbidden, &exec(Some(0), "")));
        assert!(!both_forbidden(
            &forbidden,
            &exec(Some(1), "    HTTP 500\n")
        ));
        assert!(!both_forbidden(&exec(None, "    HTTP 403\n"), &forbidden));
    }

    #[test]
    fn shapes_compare_types_and_keys() {
        assert!(same_shape(
            &json!({"items": [{"a": 1}], "n": 2}),
            &json!({"n": 5, "items": []})
        ));
        assert!(!same_shape(
            &json!({"items": [{"a": 1}]}),
            &json!({"items": [{"a": "x"}]})
        ));
        assert!(!same_shape(&json!({"a": 1}), &json!({"b": 1})));
    }

    #[test]
    fn report_shapes_read_sections_and_evidence() {
        let html = "<!doctype html>\n<meta http-equiv=\"Content-Security-Policy\" content=\"x\"><section id=\"findings\"></section><section id=\"evidence\" aria><table><tr><th>h</th></tr><tr><td>Security profiles — list</td><td>w</td><td>unavailable</td><td>0</td><td>1</td><td>The API returned no record array. x</td></tr></table></section>";
        let shape = report_shape(html).unwrap();
        assert_eq!(shape.sections, ["findings", "evidence"]);
        assert_eq!(shape.evidence[0].1, "unavailable");
        assert!(shape.csp && !shape.external);
        let mut ts = shape.clone();
        ts.evidence[0].1 = "complete".into();
        assert!(reports_agree(&shape, &ts), "D214");
        assert!(!reports_agree(&ts, &shape));
        assert!(report_shape("<html>").is_none());
    }

    #[test]
    fn backups_compare_the_record_and_the_topic_keys() {
        let record = json!({"profile_name": "P", "policy": {}});
        let rust = json!({"format": "airs-cli-rs.runtime-profiles", "version": 1,
            "profiles": [{"record": record}], "topics": [{"record": {"topic_id": "t", "revision": 3}}]});
        let ts = json!({"profiles": [record], "topics": [{"topic_id": "t", "revision": 3, "examples": []}]});
        assert!(backups_agree(&rust, &ts));
        let other = json!({"profiles": [record], "topics": [{"topic_id": "t", "revision": 4}]});
        assert!(!backups_agree(&rust, &other));
    }
}
