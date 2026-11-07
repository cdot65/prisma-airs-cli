//! The withholding traversal (airs-runtime-core, DESIGN §19.3) seen through the CLI's own
//! renderers: a withheld document never prints a credential in json, pretty or table, and
//! the values the rules keep stay printed. Core's tests hold the traversal itself; this is
//! the end-to-end half that needs the CLI's `pretty` and `table` layouts.

use crate::error::SecretSet;
use crate::gateway::views::render_detail;
use crate::gateway::withhold::{PLAIN, apply};
use crate::json::{OValue, parse};
use crate::output::Format;
use crate::output::testing::capture;
use airs_runtime_core::withhold_rules::corpus;

fn printed_formats(doc: &OValue) -> [(Format, String); 3] {
    [Format::Json, Format::Pretty, Format::Table].map(|format| {
        let (mut out, stdout, _) = capture(false);
        render_detail(&mut out, doc, format).unwrap();
        out.commit().unwrap();
        (format, stdout.text())
    })
}

fn withheld(text: &str) -> OValue {
    let mut doc = parse(text).unwrap();
    apply(&mut doc, &PLAIN, &SecretSet::default());
    doc
}

/// D238: every planted credential, in every container shape, is absent from all three
/// formats, and its plain control value is present.
#[test]
fn d238_a_planted_credential_never_prints_in_any_format() {
    for case in &corpus::planted() {
        for (format, text) in printed_formats(&withheld(&case.document)) {
            assert!(
                !text.contains(&case.secret),
                "{} printed in {format:?}: {text}",
                case.case
            );
            assert!(
                text.contains(&case.control),
                "{} in {format:?}: {text}",
                case.case
            );
        }
    }
}

/// The real-world texts: no registered credential prints in any format.
#[test]
fn d238_real_world_texts_never_print_a_registered_credential() {
    for case in corpus::REAL_WORLD_TEXTS {
        let (received, _) = case.documents();
        for (format, shown) in printed_formats(&withheld(&received)) {
            for secret in case.registered {
                assert!(!shown.contains(secret), "{} in {format:?}", case.case);
            }
        }
    }
}

/// Choices 181, 183 and 186: words the rules withhold only in their own text (a model id
/// after a prose scheme word, placeholder values under credential keys, short words) stay
/// printed elsewhere in every format, while connection-string passwords never print.
#[test]
fn d238_locally_withheld_words_stay_printed_elsewhere_in_every_format() {
    let cases: [(&str, &[&str], &[&str]); 4] = [
        (
            r#"[{"error":"token gpt-4o-mini-2024-07-18 quota exceeded","ai_model":"o1"},{"error":"","ai_model":"gpt-4o-mini-2024-07-18"},{"usage":{"gpt-4o-mini-2024-07-18":3}}]"#,
            &["gpt-4o-mini-2024-07-18"],
            &[],
        ),
        (
            r#"[{"description":"basic claude-3-5-sonnet-20241022 routing","model":"m"},{"description":"","model":"claude-3-5-sonnet-20241022"}]"#,
            &["claude-3-5-sonnet-20241022"],
            &[],
        ),
        (
            r#"[{"description":"Set auth=none for local"},{"mode":"none","guardrail":"nonexistent-check"},{"note":"password=true"},{"enabled":"true"},{"note":"Pwd=****;"},{"mask":"****"},{"label":"token gpt-4o-mini-2024-07-18"},{"ai_model":"gpt-4o-mini-2024-07-18"},{"password":"none"},{"state":"none"}]"#,
            &["nonexistent-check", "gpt-4o-mini-2024-07-18"],
            &[],
        ),
        (
            r#"[{"note":"Use auth=oauth2 for remote"},{"auth_type":"oauth2"},{"note":"secret=sha256 and pwd=v2.0"},{"algorithm":"sha256","version":"v2.0"},{"conn":"Server=db;User Id=sa;Password=p@ss w0rd;"},{"echo":"p@ss w0rd"},{"odbc":"Server=db;PWD={p@ss;w0rd};"},{"echo":"p@ss;w0rd"},{"ini":"set password = hunter2x! here"},{"echo":"hunter2x!"}]"#,
            &["oauth2", "sha256", "v2.0"],
            &["w0rd", "hunter2x!"],
        ),
    ];
    for (document, kept, never) in cases {
        for (format, shown) in printed_formats(&withheld(document)) {
            for word in kept {
                assert!(shown.contains(word), "{format:?}: {shown}");
            }
            for secret in never {
                assert!(!shown.contains(secret), "{format:?}: {shown}");
            }
        }
    }
}
