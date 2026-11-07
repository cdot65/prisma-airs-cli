//! The read-only checker of a printed gateway document (DESIGN §19.3; choices 166, 172, 179,
//! 185, 187): [`violations`] names every value of a document that shows what the rules
//! withhold, and [`withheld`] says whether there is none, reading it with the same
//! classification and action tables as the CLI's traversal ([`super::names`]) and the same
//! value view ([`super::view`]).
//! The live tier runs it on every Rust-only gateway read; the CLI's tests run it on everything
//! the CLI prints for the pinned and the generated documents (the `corpus` module).
//!
//! The CLI compiles it in test builds only (the binary never checks a document it printed);
//! the harness compiles it in every build. It is generic over [`Document`], a [`Json`] view
//! that also parses a JSON text: the CLI implements it for its own order-keeping value type in
//! its tests, and this file for `serde_json::Value` (the live tier's reading; the CLI's tests
//! read both).

use std::collections::BTreeSet;

use super::WITHHELD;
use super::names::{
    AUDIT_FIELDS, Action, Class, Pair, SCHEMA_KEYWORDS, SCHEMA_VALUE_KEYWORDS, SUBSCHEMA_KEYWORDS,
    action, classify, header_action, header_class, header_item_action, is_secret_location,
    member_class, normalised,
};
use super::values::{
    has_private_key, header_credential_spans, key_value_credential_spans, looks_like_json_text,
    scheme_credential_spans, shows_credential, urls_in,
};
pub use super::view::is_schema;
use super::view::{Json, Kind, is_empty, shape};

/// A [`Json`] view that also reads the JSON document a text holds.
pub trait Document: Json {
    /// The JSON object or array a text holds (a text that starts, after whitespace, with `{`
    /// or `[` and parses as one), read as the CLI reads it (`serde_json`); `None` otherwise.
    fn document(text: &str) -> Option<Self>;
}

/// Whether a value is an object or an array.
pub fn is_container<J: Json>(value: &J) -> bool {
    matches!(value.kind(), Kind::Object | Kind::Array)
}

/// The [`Pair`] an object is, if it is one.
pub fn pair<J: Json>(value: &J) -> Option<Pair> {
    Pair::of(
        value
            .members()
            .into_iter()
            .map(|(name, child)| (name, child.text())),
    )
}

/// Whether an object is a secret mapping or a secret reference ([`is_secret_location`]).
pub fn located<J: Json>(value: &J) -> bool {
    is_secret_location(value.members().into_iter().map(|(name, _)| name))
}

/// One step of a [`Path`]: a member, an array item, or into the JSON document a string holds
/// (rule 3f).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// The member of that name.
    Member(String),
    /// The array item at that index.
    Item(usize),
    /// The JSON object or array the string holds as text.
    Text,
}

/// Where in a document a value sits: the steps from the root.
pub type Path = Vec<Step>;

/// Whether a printed document withholds every credential the rules name (DESIGN §19.3): it is
/// an object or an array and [`violations`] finds nothing in it.
pub fn withheld<J: Document>(doc: &J) -> bool {
    is_container(doc) && violations(doc).is_empty()
}

/// Every place where `doc` shows what the rules withhold (DESIGN §19.3; choice 179): every
/// member's value must be what [`action`] says of its class ([`member_class`]) and its shape
/// (a withheld one `[withheld]`); a pair's value is read as a header with the pair's name
/// ([`Pair`]); a header map shows only the safe headers' values and the names of a header
/// list; every audit request field is `[withheld]`; a JSON Schema property map is read as rule
/// 3e reads it; a string holding a JSON document passes the same check inside, one shaped as
/// a JSON document that does not parse is `[withheld]`, and no other string holds a private
/// key, a URL userinfo, query or fragment, a credential after an authorization scheme, or a
/// credential in a `key=value` segment ([`super::values::schemes_withheld`]).
///
/// On a printed document the set is empty. On a document as received it is exactly the set of
/// values the CLI's rewriting traversal changes (the CLI's
/// `withhold::tests::corpus::the_checker_names_every_value_the_cli_changes`), so the two traversals
/// cannot drift apart without a failing test.
pub fn violations<J: Document>(doc: &J) -> BTreeSet<Path> {
    let mut checker = Checker::default();
    checker.walk(doc);
    checker.found
}

/// The one read-only traversal: the path it stands at and the paths that fail.
#[derive(Default)]
struct Checker {
    at: Path,
    found: BTreeSet<Path>,
}

impl Checker {
    /// The value at the current path fails.
    fn fail(&mut self) {
        self.found.insert(self.at.clone());
    }

    /// Run `check` one step further down.
    fn under(&mut self, step: Step, check: impl FnOnce(&mut Self)) {
        self.at.push(step);
        check(self);
        self.at.pop();
    }

    /// The value must be `[withheld]`.
    fn hidden<J: Document>(&mut self, value: &J) {
        if value.text() != Some(WITHHELD) {
            self.fail();
        }
    }

    /// The value must be as `action` says.
    fn take<J: Document>(&mut self, action: Action, value: &J) {
        match action {
            Action::Read => self.walk(value),
            Action::Headers => self.header_map(value),
            Action::Keep => {}
            Action::Withhold(_) => self.hidden(value),
        }
    }

    /// One member under its name: an audit request field `[withheld]` (or `null`), any other
    /// as the action table says.
    fn member<J: Document>(&mut self, name: &str, located: bool, child: &J) {
        if AUDIT_FIELDS.contains(&normalised(name).as_str()) {
            if child.kind() != Kind::Null {
                self.hidden(child);
            }
            return;
        }
        self.take(action(member_class(name, located), shape(child)), child);
    }

    /// A header map (rule 3c): an object's values (a pair's value under its name) as
    /// [`header_action`] says, a list of maps, names or values ([`header_item_action`]), a
    /// JSON text holding one, any other string `[withheld]`.
    fn header_map<J: Document>(&mut self, value: &J) {
        match value.kind() {
            Kind::Object => {
                let pair = pair(value);
                for (name, child) in value.members() {
                    self.under(Step::Member(name.to_owned()), |checker| match &pair {
                        Some(pair) if pair.names_member(name, child.text()) => checker.walk(child),
                        Some(pair) if name == super::names::PAIR_VALUE => {
                            checker.take(header_action(pair.class(), shape(child)), child);
                        }
                        _ => checker.take(header_action(header_class(name), shape(child)), child),
                    });
                }
            }
            Kind::Array => {
                for (at, item) in value.items().into_iter().enumerate() {
                    self.under(Step::Item(at), |checker| {
                        if item.kind() == Kind::Object {
                            checker.header_map(item);
                        } else {
                            checker.take(header_item_action(item.text(), shape(item)), item);
                        }
                    });
                }
            }
            Kind::String => match value.text().and_then(J::document) {
                Some(inner) => self.under(Step::Text, |checker| checker.header_map(&inner)),
                None => self.hidden(value),
            },
            _ => {}
        }
    }

    /// A JSON Schema property map: a schema member by [`Checker::schema`], any other by its
    /// name.
    fn property_map<J: Document>(&mut self, map: &J) {
        for (name, child) in map.members() {
            self.under(Step::Member(name.to_owned()), |checker| {
                if is_schema(child) {
                    checker.schema(name, child);
                } else {
                    checker.member(name, false, child);
                }
            });
        }
    }

    /// A schema member `own` of the property `name` that holds a value: checked under
    /// whichever name is the stronger class, its own on a tie (choice 150).
    fn instance<J: Document>(&mut self, name: &str, own: &str, child: &J) {
        if classify(own) >= classify(name) {
            self.member(own, false, child);
        } else {
            self.member(name, false, child);
        }
    }

    /// No string inside `value` shows a credential (choice 152).
    fn describes_no_credential<J: Document>(&mut self, value: &J) {
        match value.kind() {
            Kind::String => {
                if value.text().is_some_and(shows_credential) {
                    self.fail();
                }
            }
            Kind::Array => {
                for (at, item) in value.items().into_iter().enumerate() {
                    self.under(Step::Item(at), |checker| {
                        checker.describes_no_credential(item)
                    });
                }
            }
            Kind::Object => {
                for (name, child) in value.members() {
                    self.under(Step::Member(name.to_owned()), |checker| {
                        checker.describes_no_credential(child);
                    });
                }
            }
            _ => {}
        }
    }

    /// The schema of the property `name` (rule 3e): its describing members walked (and, under
    /// a property the name rule withholds, showing no credential), its `properties` a property
    /// map, its subschemas schemas of `name`, an empty instance value printed (choice 188),
    /// every other member checked by [`Checker::instance`].
    fn schema<J: Document>(&mut self, name: &str, value: &J) {
        let credential = classify(name) > Class::Plain;
        for (keyword, child) in value.members() {
            self.under(Step::Member(keyword.to_owned()), |checker| {
                if keyword == "properties" && child.kind() == Kind::Object {
                    checker.property_map(child);
                } else if SUBSCHEMA_KEYWORDS.contains(&keyword) {
                    match child.kind() {
                        Kind::Object => checker.schema(name, child),
                        Kind::Array => {
                            for (at, item) in child.items().into_iter().enumerate() {
                                checker.under(Step::Item(at), |checker| {
                                    if item.kind() == Kind::Object {
                                        checker.schema(name, item);
                                    } else {
                                        checker.instance(name, keyword, item);
                                    }
                                });
                            }
                        }
                        _ => checker.instance(name, keyword, child),
                    }
                } else if SCHEMA_KEYWORDS.contains(&keyword) {
                    checker.walk(child);
                    if credential {
                        checker.describes_no_credential(child);
                    }
                } else if SCHEMA_VALUE_KEYWORDS.contains(&keyword) && is_empty(child) {
                    // Choice 188: an empty instance value prints.
                } else {
                    checker.instance(name, keyword, child);
                }
            });
        }
    }

    /// Any value by the document's rules.
    fn walk<J: Document>(&mut self, value: &J) {
        match value.kind() {
            Kind::Object => {
                let pair = pair(value);
                let located = located(value);
                for (name, child) in value.members() {
                    self.under(Step::Member(name.to_owned()), |checker| match &pair {
                        Some(pair) if pair.names_member(name, child.text()) => checker.walk(child),
                        Some(pair) if name == super::names::PAIR_VALUE => {
                            checker.take(action(pair.class_outside_headers(), shape(child)), child);
                        }
                        _ if name == "properties" && child.kind() == Kind::Object => {
                            checker.property_map(child);
                        }
                        _ => checker.member(name, located, child),
                    });
                }
            }
            Kind::Array => {
                for (at, item) in value.items().into_iter().enumerate() {
                    self.under(Step::Item(at), |checker| checker.walk(item));
                }
            }
            Kind::String => {
                let text = value.text().unwrap_or_default();
                match J::document(text) {
                    Some(inner) => self.under(Step::Text, |checker| checker.walk(&inner)),
                    None => {
                        if looks_like_json_text(text)
                            || has_private_key(text)
                            || !urls_withheld(text)
                            || !schemes_withheld(text)
                        {
                            self.fail();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Whether a string shows no credential after an authorization scheme (choice 177), none as a
/// credential header's value (choice 183) and none as a `key=value` segment (choice 181).
pub fn schemes_withheld(text: &str) -> bool {
    scheme_credential_spans(text).is_empty()
        && header_credential_spans(text).is_empty()
        && key_value_credential_spans(text).is_empty()
}

/// Whether a string shows no URL userinfo, query or fragment (D251): every absolute URL inside
/// it ([`urls_in`]) has each of them empty or `[withheld]`.
pub fn urls_withheld(text: &str) -> bool {
    urls_in(text).into_iter().all(|url| {
        [url.userinfo, url.query, url.fragment]
            .into_iter()
            .flatten()
            .all(|range| range.is_empty() || &text[range] == WITHHELD)
    })
}

/// `serde_json::Value`, as the live tier reads the CLI's json output.
impl Json for serde_json::Value {
    fn kind(&self) -> Kind {
        match self {
            Self::Null => Kind::Null,
            Self::Bool(_) => Kind::Bool,
            Self::Number(_) => Kind::Number,
            Self::String(_) => Kind::String,
            Self::Array(_) => Kind::Array,
            Self::Object(_) => Kind::Object,
        }
    }
    fn text(&self) -> Option<&str> {
        self.as_str()
    }
    fn items(&self) -> Vec<&Self> {
        self.as_array()
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    }
    fn members(&self) -> Vec<(&str, &Self)> {
        self.as_object()
            .map(|map| {
                map.iter()
                    .map(|(name, child)| (name.as_str(), child))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// `serde_json::Value` reads a JSON text as `serde_json` parses it.
impl Document for serde_json::Value {
    fn document(text: &str) -> Option<Self> {
        if !text.trim_start().starts_with(['{', '[']) {
            return None;
        }
        serde_json::from_str::<Self>(text)
            .ok()
            .filter(|inner| inner.is_object() || inner.is_array())
    }
}
