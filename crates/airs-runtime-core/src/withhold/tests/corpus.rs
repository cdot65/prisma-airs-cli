//! The corpus-driven checks: the pinned, generated and real-world documents, the checker
//! held equal to the traversal, and the rules module compiled outside the CLI.

use super::*;

/// Choices 172, 185: the checker reads the CLI's value type and `serde_json::Value` (the live
/// tier's) alike, in the CLI's build of the rules and in the harness's: the same verdict on
/// every pinned document and on each schema shape.
#[test]
fn the_checker_reads_both_value_types_alike() {
    use airs_test_harness::live::gateway as live;
    let schemas = [
        r#"{"type":"string"}"#,
        r#"{"type":["string","null"]}"#,
        r#"{"type":[]}"#,
        r#"{"type":"basic"}"#,
        r#"{"items":{}}"#,
        r#""string""#,
    ];
    for text in schemas {
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        let own = check::is_schema(&parse(text).unwrap());
        assert_eq!(live::is_schema(&value), own, "{text}");
        assert_eq!(check::is_schema(&value), own, "{text}");
    }
    for (received, printed) in PINNED_DOCUMENTS {
        for text in [received, printed] {
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(live::withheld(&value), checked(text), "{text}");
            assert_eq!(check::withheld(&value), checked(text), "{text}");
        }
    }
}

/// Choices 158, 166 and 172: every pinned document ([`PINNED_DOCUMENTS`], one case per
/// rule and sub-rule) is printed by the CLI as the table says, and the shared checker (the
/// live tier's) accepts what the CLI prints and refuses what it receives unless the CLI
/// prints it as received (the harness's own tests iterate the same table).
#[test]
fn the_live_tier_accepts_what_the_cli_prints() {
    for (received, printed) in PINNED_DOCUMENTS {
        let (text, _) = applied(received, &PLAIN);
        assert_eq!(text, *printed, "received: {received}");
        assert!(checked(&text), "printed: {text}");
        assert_eq!(
            checked(received),
            received == printed,
            "received: {received}"
        );
    }
}

/// Choice 173 (review): a credential planted under every name shape the name rule claims
/// (every row of `NAME_RULES`, in snake, kebab, spaced, dotted, camel, Pascal and upper
/// case, with prefixes and suffixes) and in every container shape (a member, a nested
/// member, a list, an object, a `{name, value}` and a `{key, value}` pair, a header map, a
/// header list, a JSON text, a schema default, an echo, a bearer echo, a number), a private
/// key under neutral names and a credential in a URL never appears in the json, pretty or
/// table output, while the plain control value does; the shared checker accepts every
/// printed document; and revealed as an audit request body it never appears either.
#[test]
fn d238_a_planted_credential_never_prints_in_any_format() {
    let planted = withhold_rules::corpus::planted();
    for case in &planted {
        let secrets = SecretSet::default();
        let mut doc = parse(&case.document).unwrap();
        apply(&mut doc, &PLAIN, &secrets);
        assert!(
            check::withheld(&doc),
            "{}: {}",
            case.case,
            to_compact(&doc).unwrap()
        );
        for (format, text) in printed_formats(&doc) {
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
        let mut rows = OValue::Array(vec![OValue::Object(vec![(
            "request_body".into(),
            OValue::String(case.document.clone()),
        )])]);
        apply_audit(&mut rows, true, &SecretSet::default());
        let text = to_compact(&rows).unwrap();
        assert!(
            !text.contains(&case.secret),
            "{} revealed: {text}",
            case.case
        );
    }
}

/// The paths at which `printed` differs from `received` (a string holding a JSON document
/// on both sides is compared inside it), as `check::violations` names them.
fn changed_paths(
    received: &OValue,
    printed: &OValue,
    at: &mut check::Path,
    out: &mut std::collections::BTreeSet<check::Path>,
) {
    use check::Step;
    if received == printed {
        return;
    }
    let mut under = |step: Step, received: &OValue, printed: &OValue, at: &mut check::Path| {
        at.push(step);
        changed_paths(received, printed, at, out);
        at.pop();
    };
    match (received, printed) {
        (OValue::Object(a), OValue::Object(b))
            if a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.0 == y.0) =>
        {
            for ((name, x), (_, y)) in a.iter().zip(b) {
                under(Step::Member(name.clone()), x, y, at);
            }
        }
        (OValue::Array(a), OValue::Array(b)) if a.len() == b.len() => {
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                under(Step::Item(index), x, y, at);
            }
        }
        (OValue::String(a), OValue::String(b)) => match (json_document(a), json_document(b)) {
            (Some(x), Some(y)) => under(Step::Text, &x, &y, at),
            _ => {
                out.insert(at.clone());
            }
        },
        _ => {
            out.insert(at.clone());
        }
    }
}

/// Choice 179 (review: the structural sub-rules are walked twice, by this rewriting
/// traversal and by the rules module's read-only checker): for every pinned and generated
/// document as received, the values the name, URL and scheme rules change are exactly the
/// values the checker names as showing a credential (`check::violations`), so a sub-rule
/// changed in one traversal and not the other fails here, path by path.
#[test]
fn the_checker_names_every_value_the_cli_changes() {
    let pinned = PINNED_DOCUMENTS
        .iter()
        .map(|(received, _)| ((*received).to_owned(), (*received).to_owned()));
    let planted = withhold_rules::corpus::planted()
        .into_iter()
        .map(|case| (case.document, case.case));
    let mut documents = 0;
    for (document, case) in pinned.chain(planted) {
        let received = parse(&document).unwrap();
        let mut printed = received.clone();
        names_and_urls(&mut printed, &mut Vec::new());
        let mut cli = std::collections::BTreeSet::new();
        changed_paths(&received, &printed, &mut Vec::new(), &mut cli);
        let checker = check::violations(&received);
        assert_eq!(cli, checker, "{case}: {document}");
        assert!(check::violations(&printed).is_empty(), "{case}: {document}");
        documents += 1;
    }
    assert!(documents > 5000, "{documents}");
}

/// Choice 181: every rule-independent text (`corpus::REAL_WORLD_TEXTS`: `curl` lines in
/// both header spellings, connection strings, a form body, a flag, a JDBC URL, and prose
/// naming a model after `token` or `basic`) prints exactly as pinned under a neutral name;
/// each registered credential echoed under another name is withheld and each kept word
/// printed; the checker names exactly the text's path on the received document and
/// nothing on the printed one; and no registered credential appears in json, pretty or
/// table.
#[test]
fn d238_real_world_texts_print_as_pinned() {
    use check::Step;
    for case in withhold_rules::corpus::REAL_WORLD_TEXTS {
        let (received, printed) = case.documents();
        let (text, secrets) = applied(&received, &PLAIN);
        assert_eq!(text, printed, "{}", case.case);
        for secret in case.registered {
            assert!(secrets.carried_by(secret), "{}: {secret}", case.case);
        }
        for word in case.kept {
            assert!(!secrets.carried_by(word), "{}: {word}", case.case);
        }
        let as_received = parse(&received).unwrap();
        let expected: std::collections::BTreeSet<check::Path> = if case.text == case.printed {
            std::collections::BTreeSet::new()
        } else {
            [vec![Step::Item(0), Step::Member("note".into())]].into()
        };
        assert_eq!(check::violations(&as_received), expected, "{}", case.case);
        let as_printed = parse(&text).unwrap();
        assert!(check::violations(&as_printed).is_empty(), "{}", case.case);
        for (format, shown) in printed_formats(&as_printed) {
            for secret in case.registered {
                assert!(!shown.contains(secret), "{} in {format:?}", case.case);
            }
        }
    }
}

/// Choices 185 and 187: the rules module is compiled twice, by airs-runtime-core and by the test harness (by
/// `#[path]`, from these same files), so its files name their siblings as `super::` (a
/// `crate::` path means a different module, or nothing, in the harness) and use `std` and
/// `serde_json` only; the harness includes the module root and the corpus by those paths,
/// and no manifest names a second crate for them.
#[test]
fn the_rules_compile_outside_the_cli() {
    // The repository root: the rules live in this crate, the harness beside it.
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let read = |path: &str| std::fs::read_to_string(format!("{root}/{path}")).unwrap();
    let dir = format!("{root}/crates/airs-runtime-core/src/withhold_rules");
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    assert_eq!(
        files,
        [
            "check.rs",
            "corpus.rs",
            "mod.rs",
            "names.rs",
            "values.rs",
            "view.rs"
        ],
        "a new file needs a `mod` line in mod.rs (and the harness reads it through mod.rs)"
    );
    for file in &files {
        let text = read(&format!(
            "crates/airs-runtime-core/src/withhold_rules/{file}"
        ));
        for (at, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for path in ["crate::", "airs_cli_rs::", "scm::", "airs_test_harness::"] {
                assert!(
                    !code.contains(path),
                    "{file}:{}: `{path}` does not resolve in the harness: {line}",
                    at + 1
                );
            }
        }
    }
    let harness = read("crates/airs-test-harness/src/withhold.rs");
    for include in [
        r#"#[path = "../../airs-runtime-core/src/withhold_rules/mod.rs"]"#,
        r#"#[path = "../../airs-runtime-core/src/withhold_rules/corpus.rs"]"#,
        r#"#[path = "../../airs-runtime-core/src/withhold_rules/check.rs"]"#,
    ] {
        assert!(harness.contains(include), "{include}");
    }
    // Choice 187: the binary carries no checker and no test helper; the module root
    // declares the checker and the corpus for test builds only, and no blanket `dead_code`
    // allow hides an unused rule.
    let module = read("crates/airs-runtime-core/src/withhold_rules/mod.rs");
    for declaration in [
        "#[cfg(any(test, feature = \"test-support\"))]\npub mod check;",
        "#[cfg(any(test, feature = \"test-support\"))]\npub mod corpus;",
    ] {
        assert!(module.contains(declaration), "{declaration}");
    }
    for root_module in ["src/gateway/mod.rs", "crates/airs-runtime-core/src/lib.rs"] {
        assert!(
            !read(root_module).contains("dead_code"),
            "no blanket allow on the rules module: {root_module}"
        );
    }
    for manifest in [
        "Cargo.toml",
        "crates/airs-runtime-core/Cargo.toml",
        "crates/airs-test-harness/Cargo.toml",
    ] {
        assert!(!read(manifest).contains("withhold"), "{manifest}");
    }
}
