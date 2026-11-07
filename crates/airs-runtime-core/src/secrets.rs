//! The `SecretSet`: every secret the CLI knows in the forms it may appear in, and the checks
//! of a printed string against them (DESIGN §6, §8; choice 187). `error.rs` re-exports it,
//! so callers name `crate::error::SecretSet` as before.
//!
//! The set is read once per printed string (the gateway documents check every string and
//! member name against it), so the forms are one ordered set (a form joins in logarithmic
//! time and is stored once), and [`SecretSet::carried_by`] and
//! [`SecretSet::appears_in_ignoring_case`] check a text against every form in one pass over
//! its bytes ([`Matcher`], built on the first read after a form joins). The fragment half of
//! [`SecretSet::taints`] (a text of at least four bytes found inside a form) still reads each
//! form, once per checked text: the §6 metadata checks, `scrub` and the red team, model
//! security and scan views use it; the gateway's document pass reads `carried_by` only.

use crate::error::WITHHELD;
use crate::json::OValue;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Configured secrets, used to drop server-derived metadata that would reveal one.
///
/// Clones share one set: the OAuth provider wrapper ([`crate::clients`]) keeps a clone
/// and registers every token lease the SDK obtains, including the leases of a silent
/// 401/403 refresh-and-replay, so every later error mapping sees them.
#[derive(Clone, Default)]
pub struct SecretSet {
    forms: Arc<Mutex<Forms>>,
}

/// The registered forms, one ordered set (deduplicated, in a stable order), and the matchers
/// of the current forms (exact and ASCII case-folded), dropped whenever a form joins.
#[derive(Default)]
struct Forms {
    set: BTreeSet<String>,
    local_output_guard: bool,
    exact: Option<Matcher>,
    folded: Option<Matcher>,
    /// The matcher steps taken so far (the scale tests' structural bound; counted in test
    /// builds only).
    steps: Steps,
}

/// A count of matcher steps that only test builds keep (a scale test's structural bound): in
/// any other build it holds nothing and [`Steps::tick`] does nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Steps(#[cfg(test)] u64);

// The binary carries no step counter.
#[cfg(not(test))]
const _: () = assert!(std::mem::size_of::<Steps>() == 0);

impl Steps {
    /// One more step.
    #[inline]
    fn tick(&mut self) {
        #[cfg(test)]
        {
            self.0 += 1;
        }
    }

    /// Add the steps of one read.
    #[inline]
    fn add(&mut self, other: Self) {
        #[cfg(test)]
        {
            self.0 += other.0;
        }
        #[cfg(not(test))]
        let _ = other;
    }
}

impl Forms {
    /// Whether `text` contains a form, by the exact or the folded matcher.
    fn found_in(&mut self, text: &str, folded: bool) -> bool {
        let set = &self.set;
        let matcher = if folded {
            self.folded.get_or_insert_with(|| {
                Matcher::new(
                    set.iter()
                        .map(|form| form.to_ascii_lowercase().into_bytes()),
                )
            })
        } else {
            self.exact.get_or_insert_with(|| {
                Matcher::new(set.iter().map(|form| form.as_bytes().to_vec()))
            })
        };
        let (found, steps) = matcher.find(text.as_bytes());
        self.steps.add(steps);
        found
    }
}

/// A multi-pattern byte matcher (Aho–Corasick): a trie of the patterns whose every node has a
/// failure link to the longest proper suffix of its path that is also a trie path, so one pass
/// over a text finds whether any pattern occurs in it. The pass takes at most two steps per
/// byte (each byte moves down once, and the failure links followed never climb higher than
/// the moves down made), whatever the number of patterns.
struct Matcher {
    nodes: Vec<Node>,
}

/// One trie node: its children by byte (sorted), its failure link, and whether a pattern ends
/// here or at a node its failure links reach.
#[derive(Default)]
struct Node {
    next: Vec<(u8, usize)>,
    fail: usize,
    end: bool,
}

impl Matcher {
    fn new(patterns: impl Iterator<Item = Vec<u8>>) -> Self {
        let mut nodes = vec![Node::default()];
        for pattern in patterns {
            let mut at = 0;
            for byte in pattern {
                at = match nodes[at].next.binary_search_by_key(&byte, |(b, _)| *b) {
                    Ok(found) => nodes[at].next[found].1,
                    Err(slot) => {
                        nodes.push(Node::default());
                        let child = nodes.len() - 1;
                        nodes[at].next.insert(slot, (byte, child));
                        child
                    }
                };
            }
            nodes[at].end = true;
        }
        // Breadth first, so a node's failure link is set before its children's.
        let mut queue = std::collections::VecDeque::from([0]);
        while let Some(at) = queue.pop_front() {
            for (byte, child) in nodes[at].next.clone() {
                let fail = if at == 0 {
                    0
                } else {
                    let mut link = nodes[at].fail;
                    loop {
                        if let Some(target) = Self::child(&nodes, link, byte) {
                            break target;
                        }
                        if link == 0 {
                            break 0;
                        }
                        link = nodes[link].fail;
                    }
                };
                nodes[child].fail = fail;
                nodes[child].end |= nodes[fail].end;
                queue.push_back(child);
            }
        }
        Self { nodes }
    }

    fn child(nodes: &[Node], at: usize, byte: u8) -> Option<usize> {
        let next = &nodes[at].next;
        next.binary_search_by_key(&byte, |(b, _)| *b)
            .ok()
            .map(|found| next[found].1)
    }

    /// Whether a pattern occurs in `text`, and the steps taken (moves down and failure links
    /// followed; at most twice the bytes read; counted in test builds only).
    fn find(&self, text: &[u8]) -> (bool, Steps) {
        let mut steps = Steps::default();
        let mut at = 0;
        if self.nodes[0].end {
            return (true, steps);
        }
        for &byte in text {
            loop {
                steps.tick();
                if let Some(next) = Self::child(&self.nodes, at, byte) {
                    at = next;
                    break;
                }
                if at == 0 {
                    break;
                }
                at = self.nodes[at].fail;
            }
            if self.nodes[at].end {
                return (true, steps);
            }
        }
        (false, steps)
    }
}

impl SecretSet {
    /// Named scanner configuration/input opts local paths into final filtering.
    pub fn guard_local_output(&self) {
        self.forms().local_output_guard = true;
    }

    pub fn guards_local_output(&self) -> bool {
        self.forms().local_output_guard
    }
    fn forms(&self) -> MutexGuard<'_, Forms> {
        self.forms.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, candidate: String) {
        let mut forms = self.forms();
        if forms.set.insert(candidate) {
            forms.exact = None;
            forms.folded = None;
        }
    }

    /// The number of registered forms.
    #[cfg(test)]
    pub fn form_count(&self) -> usize {
        self.forms().set.len()
    }

    /// The matcher steps taken by every read so far (a scale test's structural bound: at most
    /// twice the bytes read, whatever the number of forms).
    #[cfg(test)]
    pub fn steps(&self) -> u64 {
        self.forms().steps.0
    }

    /// Register a secret in its raw, trimmed, form-encoded and base64 forms.
    pub fn add(&self, value: &str) {
        for candidate in [value, crate::tenant::config::js_trim(value)] {
            if candidate.is_empty() {
                continue;
            }
            let encoded: String =
                url::form_urlencoded::byte_serialize(candidate.as_bytes()).collect();
            for form in [
                candidate.to_owned(),
                encoded,
                base64(candidate.as_bytes(), STANDARD_ALPHABET),
                base64(candidate.as_bytes(), URL_SAFE_ALPHABET),
            ] {
                self.push(form);
            }
        }
    }

    /// Register the OAuth client pair as the HTTP Basic credential carries it:
    /// `base64(id:secret)` with raw and RFC 6749 form-encoded halves (DESIGN §6, D29),
    /// in the standard and URL-safe alphabets, plus their raw `id:secret` text.
    pub fn add_basic_credentials(&self, client_id: &str, client_secret: &str) {
        let form = |text: &str| -> String {
            url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
        };
        for (id, secret) in [
            (client_id, client_secret),
            (
                crate::tenant::config::js_trim(client_id),
                crate::tenant::config::js_trim(client_secret),
            ),
        ] {
            if secret.is_empty() {
                continue;
            }
            for pair in [
                format!("{id}:{secret}"),
                format!("{}:{}", form(id), form(secret)),
            ] {
                for candidate in [
                    base64(pair.as_bytes(), STANDARD_ALPHABET),
                    base64(pair.as_bytes(), URL_SAFE_ALPHABET),
                    pair,
                ] {
                    self.push(candidate);
                }
            }
        }
    }

    /// Register every secret a tenant config carries: each secret-named key's value
    /// and the HTTP Basic client pair. Commands and doctor share this one rule.
    pub fn register_config(&self, config: &crate::tenant::config::TenantConfig) {
        if config.scanners().is_some() {
            self.guard_local_output();
        }
        for secret in config.secret_values() {
            self.add(secret);
        }
        if let (Some(id), Some(secret)) = (
            config.effective_str("mgmtClientId"),
            config.effective_str("mgmtClientSecret"),
        ) {
            self.add_basic_credentials(id, secret);
        }
    }

    /// Whether `text` contains a secret (or is a sizeable fragment of one): the matcher's one
    /// pass, then the fragment rule, which reads each form.
    pub fn taints(&self, text: &str) -> bool {
        let mut forms = self.forms();
        forms.found_in(text, false)
            || (text.len() >= 4 && forms.set.iter().any(|form| form.contains(text)))
    }

    /// Whether `text` contains a registered form (no fragment rule: a short ordinary
    /// value such as `SECRET` or `true` that happens to occur inside a secret is not
    /// tainted). For structured values printed under any key (§15.4).
    pub fn carried_by(&self, text: &str) -> bool {
        self.forms().found_in(text, false)
    }

    /// Whether `text` contains any registered form, ignoring ASCII case (the debug
    /// log's line check). No fragment rule, so ordinary words inside a long line never
    /// trigger it. Case is ignored because header names arrive lowercased: a secret a
    /// server echoes as a header name is written case-folded (DESIGN §2.9).
    pub fn appears_in_ignoring_case(&self, text: &str) -> bool {
        self.forms().found_in(&text.to_ascii_lowercase(), true)
    }

    /// Exact membership without cloning the registry (payload identity checks).
    pub fn contains_exact_form(&self, text: &str) -> bool {
        self.forms().set.contains(text)
    }

    /// The registered forms as they are now, in the set's order (the install output splitter,
    /// which keeps a form that straddles a split whole, DESIGN §17.13, and the inference
    /// stream's carry-over).
    pub fn snapshot(&self) -> Vec<String> {
        self.forms().set.iter().cloned().collect()
    }

    /// `text` with every occurrence of a registered form replaced by [`WITHHELD`]
    /// (longest forms first): the output of an external tool the CLI forwards
    /// (`model-security install`, DESIGN §17.13).
    pub fn redact(&self, text: &str) -> String {
        let mut forms = self.snapshot();
        forms.sort_by_key(|form| std::cmp::Reverse(form.len()));
        let mut out = text.to_owned();
        for form in forms {
            if !form.is_empty() && out.contains(form.as_str()) {
                out = out.replace(form.as_str(), WITHHELD);
            }
        }
        out
    }

    /// `text`, or [`WITHHELD`] when it carries a secret (server-provided strings the
    /// CLI prints, DESIGN §6 and §8).
    pub fn scrub<'a>(&self, text: &'a str) -> &'a str {
        if self.taints(text) { WITHHELD } else { text }
    }
}

const STANDARD_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_SAFE_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Unpadded standard base64 of `bytes` (the form an HTTP Basic credential takes).
pub fn base64_standard(bytes: &[u8]) -> String {
    base64(bytes, STANDARD_ALPHABET)
}

/// Unpadded base64 (a padded encoding always contains the unpadded one).
fn base64(bytes: &[u8], alphabet: &[u8; 64]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        let symbols = chunk.len() + 1;
        for index in 0..symbols {
            let shift = 18 - 6 * index;
            out.push(char::from(alphabet[((n >> shift) & 0x3f) as usize]));
        }
    }
    out
}

/// D47 over a printed document (the runtime, gateway and supplychain documents, §18.3
/// "Server text", choice 81, §19.3, §20.3): every string (any key, any depth) that
/// carries a registered secret becomes `[withheld]`, and every object member whose name
/// carries one is dropped, value and all.
pub fn withhold(value: &mut OValue, secrets: &SecretSet) {
    match value {
        OValue::String(text) if secrets.carried_by(text) => *text = WITHHELD.to_owned(),
        OValue::Object(entries) => {
            entries.retain(|(key, _)| !secrets.carried_by(key));
            for (_, member) in entries {
                withhold(member, secrets);
            }
        }
        OValue::Array(items) => {
            for item in items {
                withhold(item, secrets);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    /// Choice 81: a member whose name carries a secret is dropped with its value, at any
    /// depth; other members (one whose name is only a fragment of the secret too) stay.
    #[test]
    fn a_member_named_by_a_secret_is_dropped() {
        let secrets = SecretSet::default();
        secrets.add("FAKE-K1-SECRET");
        let mut doc = crate::json::parse(
            r#"{"FAKE-K1-SECRET":"v","a":[{"x FAKE-K1-SECRET":{"deep":1},"b":"FAKE-K1-SECRET","K1":2}],"name":"n"}"#,
        )
        .unwrap();
        super::withhold(&mut doc, &secrets);
        assert_eq!(
            crate::json::to_compact(&doc).unwrap(),
            r#"{"a":[{"b":"[withheld]","K1":2}],"name":"n"}"#
        );
    }

    use super::*;

    /// The matcher finds exactly what a `contains` per form finds: overlapping forms, a form
    /// inside another, a failure link to a shorter form, multi-byte text, and none.
    #[test]
    fn the_matcher_agrees_with_contains_per_form() {
        let forms = ["abcd", "bcde", "cd", "xbc", "aab", "é€x", "zzzzzz"];
        let matcher = Matcher::new(forms.iter().map(|form| form.as_bytes().to_vec()));
        let texts = [
            "", "a", "ab", "abc", "abce", "xabcd", "xbcde", "aaab", "aaaa", "c d", "acd", "xbx",
            "xxbc", "é€", "aé€xb", "zzzzz", "zzzzzzz", "bcdf", "abxbcd",
        ];
        for text in texts {
            let expected = forms.iter().any(|form| text.contains(form));
            let (found, steps) = matcher.find(text.as_bytes());
            assert_eq!(found, expected, "{text:?}");
            assert!(
                steps.0 <= 2 * text.len() as u64,
                "{text:?}: {steps:?} steps"
            );
        }
        assert!(!Matcher::new(std::iter::empty()).find(b"anything").0);
    }

    /// Choice 187 (review: the audit read was quadratic): a form joins once, and a read after
    /// a join rebuilds the matchers so it sees the new form, in both the exact and the
    /// case-folded reading.
    #[test]
    fn forms_join_once_and_every_read_sees_the_latest() {
        let secrets = SecretSet::default();
        secrets.add("s3cr3t-value");
        let count = secrets.form_count();
        secrets.add("s3cr3t-value");
        assert_eq!(secrets.form_count(), count, "no duplicate forms");
        assert!(secrets.carried_by("x s3cr3t-value y"));
        assert!(!secrets.carried_by("another-value"));
        assert!(!secrets.appears_in_ignoring_case("ANOTHER-VALUE"));
        secrets.add("another-value");
        assert!(secrets.carried_by("another-value"));
        assert!(secrets.appears_in_ignoring_case("ANOTHER-VALUE"));
    }

    /// Choice 187 (review: every form was stored twice, in a list and in its hash index): the
    /// forms are one ordered set, so a snapshot is sorted and holds each form once whatever
    /// the order they joined in, and `redact` replaces the longest form first.
    #[test]
    fn the_forms_are_one_ordered_set() {
        let secrets = SecretSet::default();
        for form in ["zz-secret-2", "aa-secret-1", "zz-secret-2", "mm-secret-3"] {
            secrets.push(form.to_owned());
        }
        assert_eq!(
            secrets.snapshot(),
            ["aa-secret-1", "mm-secret-3", "zz-secret-2"]
        );
        assert_eq!(secrets.form_count(), 3);
        secrets.push("aa-secret-1-longer".to_owned());
        assert_eq!(
            secrets.redact("x aa-secret-1-longer y"),
            format!("x {WITHHELD} y")
        );
    }

    /// The fragment rule of `taints` (a text of at least four bytes inside a form) is not
    /// the matcher's: it reads each form, and `carried_by` has no fragment rule.
    #[test]
    fn only_taints_reads_fragments() {
        let secrets = SecretSet::default();
        secrets.add("FAKE-FRAGMENT-SECRET-1");
        assert!(secrets.taints("FRAGMENT"));
        assert!(!secrets.carried_by("FRAGMENT"));
        assert!(!secrets.taints("FRA"));
    }

    /// The step counter is a test build's only (the binary carries none; a `const` assertion
    /// holds it there), and in a test build it counts every matcher step of every read.
    #[test]
    fn the_step_counter_counts_reads() {
        let secrets = SecretSet::default();
        secrets.add("FAKE-STEP-SECRET-1");
        let before = secrets.steps();
        assert!(!secrets.carried_by("abcdefgh"));
        let steps = secrets.steps() - before;
        assert!((8..=16).contains(&steps), "{steps}");
    }

    /// Choice 187 (review: the `SecretSet` lived in `error.rs`): the set, its forms and its
    /// matcher are this module's, `error.rs` only re-exports them, and DESIGN §11 names the
    /// module (in airs-runtime-core since M1).
    #[test]
    fn the_secret_set_is_its_own_module() {
        let root = env!("CARGO_MANIFEST_DIR");
        let error = std::fs::read_to_string(format!("{root}/src/error.rs")).unwrap();
        for item in [
            "struct SecretSet",
            "struct Forms",
            "struct Matcher",
            "fn base64(",
        ] {
            assert!(!error.contains(item), "error.rs still defines {item}");
        }
        assert!(error.contains("pub use crate::secrets::"));
        let design = std::fs::read_to_string(format!("{root}/../../docs/DESIGN.md")).unwrap();
        assert!(
            design.contains("crates/airs-runtime-core/src/secrets.rs"),
            "DESIGN §11 names crates/airs-runtime-core/src/secrets.rs"
        );
    }

    #[test]
    fn secret_forms() {
        let secrets = SecretSet::default();
        secrets.add(" a b/c+ ");
        assert!(secrets.taints("x a b/c+ y"));
        assert!(secrets.taints("a+b%2Fc%2B"));
        assert!(secrets.taints(&base64(b"a b/c+", STANDARD_ALPHABET)));
        assert!(!secrets.taints("unrelated"));
        assert_eq!(
            base64(b"FAKE-SECRET-100", STANDARD_ALPHABET),
            "RkFLRS1TRUNSRVQtMTAw"
        );
        assert_eq!(base64(b"ab", STANDARD_ALPHABET), "YWI");
        assert_eq!(base64(b"\xfb\xff", URL_SAFE_ALPHABET), "-_8");
    }

    /// The debug line check ignores ASCII case: a secret echoed as a (lowercased)
    /// header name still matches.
    #[test]
    fn line_check_ignores_case() {
        let secrets = SecretSet::default();
        secrets.add("FAKE-Secret-100");
        assert!(secrets.appears_in_ignoring_case(r#"{"fake-secret-100":"***"}"#));
        assert!(secrets.appears_in_ignoring_case("x FAKE-SECRET-100 y"));
        assert!(secrets.appears_in_ignoring_case(
            &base64(b"FAKE-Secret-100", STANDARD_ALPHABET).to_lowercase()
        ));
        assert!(!secrets.appears_in_ignoring_case("fake-secret-10"));
    }

    #[test]
    fn basic_credential_forms() {
        let secrets = SecretSet::default();
        secrets.add_basic_credentials("client 100", "FAKE/SECRET 100");
        // base64(id:secret), raw and form-encoded halves (D29).
        assert!(secrets.taints(&format!(
            "x{}y",
            base64(b"client 100:FAKE/SECRET 100", STANDARD_ALPHABET)
        )));
        assert!(secrets.taints(&base64(b"client+100:FAKE%2FSECRET+100", STANDARD_ALPHABET)));
        assert!(secrets.taints(&base64(b"client+100:FAKE%2FSECRET+100", URL_SAFE_ALPHABET)));
        assert!(!secrets.taints("unrelated"));
        // Without a secret there is no credential pair to protect.
        let empty = SecretSet::default();
        empty.add_basic_credentials("client-100", "");
        assert!(!empty.taints(&base64(b"client-100:", STANDARD_ALPHABET)));
    }
}
