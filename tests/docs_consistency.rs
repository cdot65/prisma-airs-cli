//! Keeps `docs/COMPATIBILITY.md` in step with the test suites and DESIGN §12.
//!
//! The checklist cites tests as backticked `path/to/file.rs::module::function` spans. These
//! tests check that every cited function exists as a `#[test]` inside the named modules of
//! the named file, that every DESIGN §12 row has a row in the checklist's divergence index
//! (and the other way round), and that every `dNN`-named parity test is cited. They only
//! read files in this repository: no process, no network.

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const CHECKLIST: &str = "docs/COMPATIBILITY.md";
const DESIGN: &str = "docs/DESIGN.md";

fn read(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

/// Inline code spans, line by line (the checklist has no fenced blocks or multi-line spans).
fn code_spans(text: &str) -> Vec<&str> {
    text.lines()
        .flat_map(|line| line.split('`').skip(1).step_by(2))
        .collect()
}

/// A cited test: the file (relative to the repository root), its module path and its name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TestRef {
    file: String,
    modules: Vec<String>,
    name: String,
}

impl TestRef {
    fn parse(span: &str) -> Option<TestRef> {
        if !["tests/", "src/", "crates/"]
            .iter()
            .any(|prefix| span.starts_with(prefix))
        {
            return None;
        }
        let split = span.find(".rs::")?;
        let file = span[..split + 3].to_owned();
        let mut segments: Vec<String> = span[split + 5..].split("::").map(str::to_owned).collect();
        let name = segments.pop()?;
        let identifier =
            |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        assert!(
            identifier(&name) && segments.iter().all(|s| identifier(s)),
            "{CHECKLIST}: malformed test reference `{span}`"
        );
        Some(TestRef {
            file,
            modules: segments,
            name,
        })
    }

    fn display(&self) -> String {
        let mut parts = vec![self.file.clone()];
        parts.extend(self.modules.iter().cloned());
        parts.push(self.name.clone());
        parts.join("::")
    }
}

fn cited_tests(checklist: &str) -> BTreeSet<TestRef> {
    code_spans(checklist)
        .into_iter()
        .filter_map(TestRef::parse)
        .collect()
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn declares_module(line: &str, module: &str) -> bool {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
    rest.strip_prefix("mod ")
        .and_then(|after| after.strip_prefix(module))
        .is_some_and(|after| after.trim_start().starts_with('{'))
}

fn starts_item_at(line: &str, indent: usize) -> bool {
    if line.trim().is_empty() || indent_of(line) > indent {
        return false;
    }
    let trimmed = line.trim_start();
    trimmed.starts_with("mod ") || trimmed.starts_with("pub mod ")
}

fn declares_fn(line: &str, name: &str) -> bool {
    let mut rest = line.trim_start();
    for qualifier in ["pub ", "async "] {
        rest = rest.strip_prefix(qualifier).unwrap_or(rest);
    }
    rest.strip_prefix("fn ")
        .and_then(|after| after.strip_prefix(name))
        .is_some_and(|after| after.starts_with('('))
}

/// Whether the attributes and comments directly above line `index` include `#[test]`.
fn is_test_attribute_above(lines: &[&str], index: usize) -> bool {
    lines[..index]
        .iter()
        .rev()
        .map(|line| line.trim())
        .take_while(|line| line.starts_with("#[") || line.starts_with("//"))
        .any(|line| line == "#[test]" || line.starts_with("#[tokio::test"))
}

/// Finds `reference` in its file: each module nested in the previous one (a module ends at
/// the next `mod` declaration at the same or a shallower indentation), then a `#[test]`
/// function at the indentation of the innermost module's body.
fn locate(reference: &TestRef) -> Result<(), String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&reference.file);
    let source = fs::read_to_string(&path).map_err(|error| format!("cannot read file: {error}"))?;
    let lines: Vec<&str> = source.lines().collect();
    let (mut start, mut end, mut indent) = (0, lines.len(), 0);
    for module in &reference.modules {
        let opening = (start..end)
            .find(|&i| indent_of(lines[i]) == indent && declares_module(lines[i], module))
            .ok_or_else(|| format!("no `mod {module}` at indentation {indent}"))?;
        let closing = (opening + 1..end)
            .find(|&i| starts_item_at(lines[i], indent))
            .unwrap_or(end);
        (start, end, indent) = (opening + 1, closing, indent + 4);
    }
    let found = (start..end)
        .find(|&i| indent_of(lines[i]) == indent && declares_fn(lines[i], &reference.name))
        .ok_or_else(|| format!("no `fn {}` in that module", reference.name))?;
    if is_test_attribute_above(&lines, found) {
        Ok(())
    } else {
        Err(format!("`fn {}` is not a #[test]", reference.name))
    }
}

/// `D<digits>` tokens bounded by non-alphanumeric characters.
fn divergence_tokens(text: &str) -> BTreeSet<u32> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    for (i, &byte) in bytes.iter().enumerate() {
        if byte != b'D' || (i > 0 && bytes[i - 1].is_ascii_alphanumeric()) {
            continue;
        }
        let digits: String = text[i + 1..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let after = text[i + 1 + digits.len()..].chars().next();
        if !digits.is_empty() && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            found.insert(digits.parse().expect("digits"));
        }
    }
    found
}

/// Table rows whose first cell is `D<n>`: the number and the whole row.
fn divergence_rows(text: &str) -> Vec<(u32, &str)> {
    text.lines()
        .filter_map(|line| {
            let cell = line.strip_prefix("| D")?.split(" |").next()?;
            let number = cell.parse().ok()?;
            Some((number, line))
        })
        .collect()
}

fn design_section_12(design: &str) -> &str {
    let start = design
        .find("\n## 12.")
        .expect("DESIGN.md has a §12 heading");
    let rest = &design[start + 1..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |i| i + 3);
    &rest[..end]
}

#[test]
fn every_cited_test_exists_in_the_named_module() {
    let checklist = read(CHECKLIST);
    let cited = cited_tests(&checklist);
    assert!(
        cited.len() > 100,
        "expected the checklist to cite its tests, found {}",
        cited.len()
    );
    let missing: Vec<String> = cited
        .iter()
        .filter_map(|reference| {
            locate(reference)
                .err()
                .map(|why| format!("`{}`: {why}", reference.display()))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "{CHECKLIST} cites tests that do not exist:\n{}",
        missing.join("\n")
    );
}

#[test]
fn every_design_divergence_has_an_index_row_and_no_other_rows_exist() {
    let design = read(DESIGN);
    let checklist = read(CHECKLIST);
    let in_design: BTreeSet<u32> = divergence_rows(design_section_12(&design))
        .into_iter()
        .map(|(number, _)| number)
        .collect();
    assert!(in_design.len() >= 47, "DESIGN §12 rows: {in_design:?}");
    let in_index: BTreeSet<u32> = divergence_rows(&checklist)
        .into_iter()
        .map(|(number, _)| number)
        .collect();
    let missing: Vec<_> = in_design.difference(&in_index).collect();
    let stale: Vec<_> = in_index.difference(&in_design).collect();
    assert!(
        missing.is_empty(),
        "DESIGN §12 rows without a row in the {CHECKLIST} divergence index: {missing:?}"
    );
    assert!(
        stale.is_empty(),
        "{CHECKLIST} index rows with no DESIGN §12 row: {stale:?}"
    );
    let unknown: Vec<_> = divergence_tokens(&checklist)
        .into_iter()
        .filter(|number| !in_design.contains(number))
        .collect();
    assert!(
        unknown.is_empty(),
        "{CHECKLIST} mentions divergences DESIGN §12 does not define: {unknown:?}"
    );
}

#[test]
fn every_index_row_cites_a_test_or_says_why_not() {
    let checklist = read(CHECKLIST);
    let bare: Vec<u32> = divergence_rows(&checklist)
        .into_iter()
        .filter(|(_, row)| {
            !row.contains("no executable test")
                && code_spans(row)
                    .into_iter()
                    .filter_map(TestRef::parse)
                    .next()
                    .is_none()
        })
        .map(|(number, _)| number)
        .collect();
    assert!(
        bare.is_empty(),
        "{CHECKLIST} index rows with neither a test nor 'no executable test': {bare:?}"
    );
}

#[test]
fn reference_parser_and_locator_behave() {
    let reference =
        TestRef::parse("tests/docs_consistency.rs::reference_parser_and_locator_behave")
            .expect("a test reference");
    assert!(reference.modules.is_empty());
    assert_eq!(locate(&reference), Ok(()));
    let helper = TestRef::parse("tests/docs_consistency.rs::locate").expect("a reference");
    assert!(locate(&helper).unwrap_err().contains("not a #[test]"));
    let nested = TestRef::parse("src/pagination.rs::tests::collect_limits").expect("a reference");
    assert_eq!(nested.modules, ["tests"]);
    assert_eq!(locate(&nested), Ok(()));
    let wrong_module =
        TestRef::parse("src/pagination.rs::nowhere::collect_limits").expect("a reference");
    assert!(locate(&wrong_module).is_err());
    assert_eq!(TestRef::parse("src/clients.rs"), None);
    assert_eq!(TestRef::parse("scm::Endpoint::loopback"), None);
    assert_eq!(
        divergence_tokens("D1, D47; DLP, UTF-16, D3x, (D12)"),
        BTreeSet::from([1, 12, 47])
    );
}

/// The version the README's status line (`Version X covers …`) names.
fn readme_status_version(readme: &str) -> Option<&str> {
    readme
        .lines()
        .find_map(|line| line.strip_prefix("Version ")?.split_once(" covers"))
        .map(|(version, _)| version)
}

/// The example version of the README's install example, the version-neutral
/// `airs --version     # prints the version, e.g. X`.
fn readme_version_example(readme: &str) -> Option<&str> {
    readme
        .lines()
        .find_map(|line| {
            line.strip_prefix("airs --version")?
                .trim_start()
                .strip_prefix("# prints the version, e.g. ")
        })
        .map(str::trim)
}

/// The README ships in every release tarball, so its `airs --version` example says it
/// prints the version and gives the version its status line names (DESIGN §21.13,
/// choice 56).
#[test]
fn the_readme_version_example_matches_its_status_line() {
    let readme = read("README.md");
    let status = readme_status_version(&readme).expect("README.md has a `Version X covers` line");
    let example = readme_version_example(&readme)
        .expect("README.md has an `airs --version     # prints the version, e.g. X` example");
    // Cargo validates SemVer, including release candidates. Match the actual
    // package version so a source build's install example cannot lag the manifest.
    assert_eq!(
        status,
        env!("CARGO_PKG_VERSION"),
        "README status must match Cargo.toml"
    );
    assert_eq!(
        example, status,
        "README.md: the `airs --version` example gives {example}, the status line says {status}"
    );
    // The parsers see what they are meant to.
    let sample =
        "Version 2.3.4 covers x\n```sh\nairs --version     # prints the version, e.g. 2.3.5\n```\n";
    assert_eq!(readme_status_version(sample), Some("2.3.4"));
    assert_eq!(readme_version_example(sample), Some("2.3.5"));
    // A bare `# X` claims the binary prints X whatever its version; it is not accepted.
    assert_eq!(readme_version_example("airs --version     # 2.3.5\n"), None);
}

/// Every `` `a` → `b` `` in `text` (fenced blocks skipped, whitespace inside a span
/// collapsed) whose two sides are the same.
fn identity_renames(text: &str) -> Vec<String> {
    let mut prose = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            prose.push_str(line);
            prose.push('\n');
        }
    }
    let collapse = |span: &str| span.split_whitespace().collect::<Vec<_>>().join(" ");
    let segments: Vec<&str> = prose.split('`').collect();
    let mut found = Vec::new();
    for i in (1..segments.len().saturating_sub(2)).step_by(2) {
        let (from, between, to) = (segments[i], segments[i + 1], segments[i + 2]);
        if between.trim() == "→" && !from.trim().is_empty() && collapse(from) == collapse(to) {
            found.push(collapse(from));
        }
    }
    found
}

/// A bulk replace once turned the §21.6 test renames into `x` → `x`, losing the 0.11
/// names; no §21 rename may map a name to itself (DESIGN §21.13, choice 57).
#[test]
fn no_section_21_rename_maps_a_name_to_itself() {
    let design = read(DESIGN);
    let identities = identity_renames(&design[common::design_section_21(&design)]);
    assert!(
        identities.is_empty(),
        "DESIGN §21 renames that map a name to itself: {identities:?}"
    );
    // The scan sees identities across a line break and ignores real renames and fences.
    let sample = "`old_x` → `x`, `y` →\n  `y`, `` `a` → `b` ``\n```\n`z` → `z`\n```\n";
    assert_eq!(identity_renames(sample), ["y"]);
    // The shared §21 rule (also tests/program_name.rs's): its heading line up to the next.
    let design = "# D\n## 20. a\nx\n## 21. b\n### 21.1 c\ny\n## 22. d\nz\n";
    let section = common::design_section_21(design);
    assert_eq!(&design[section.clone()], "## 21. b\n### 21.1 c\ny\n");
    assert_eq!(
        format!("{}{}", &design[..section.start], &design[section.end..]),
        "# D\n## 20. a\nx\n## 22. d\nz\n"
    );
    let last = "# D\n## 21. b\ny\n";
    assert_eq!(&last[common::design_section_21(last)], "## 21. b\ny\n");
}
