//! A read-only view of a JSON value (DESIGN §19.3; choices 185, 187), the one the CLI's
//! rewriting traversal (`src/gateway/withhold.rs`, on its own value type) and the checker
//! (`check`, in test builds and the harness) read shapes through: the [`Json`] trait,
//! a value's [`Shape`] for the action table and whether an object is a JSON Schema (rule 3e).

use super::names::{Class, SCHEMA_TYPES, Shape, classify};
use super::values::{looks_like_credential, printable_member_name};

/// The kind of a JSON value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Null,
    Bool,
    Number,
    String,
    Array,
    Object,
}

/// A read-only view of a JSON value.
pub trait Json: Sized {
    /// The value's kind.
    fn kind(&self) -> Kind;
    /// The text of a string.
    fn text(&self) -> Option<&str>;
    /// The items of an array (empty for any other value).
    fn items(&self) -> Vec<&Self>;
    /// The members of an object, in order (empty for any other value).
    fn members(&self) -> Vec<(&str, &Self)>;
}

/// The member `name` of an object.
pub fn get<'a, J: Json>(value: &'a J, name: &str) -> Option<&'a J> {
    value
        .members()
        .into_iter()
        .find(|(member, _)| *member == name)
        .map(|(_, child)| child)
}

/// Whether `value` is an object or array that holds no text (choices 165, 171): no string at
/// any depth, and every member name inside it printed, a fingerprint or a count
/// ([`Class::Plain`], [`Class::Fingerprint`], [`Class::Count`]) that shows no credential
/// ([`printable_member_name`]).
pub fn text_free<J: Json>(value: &J) -> bool {
    match value.kind() {
        Kind::String => false,
        Kind::Array => value.items().into_iter().all(text_free),
        Kind::Object => value.members().into_iter().all(|(name, child)| {
            matches!(
                classify(name),
                Class::Plain | Class::Fingerprint | Class::Count
            ) && printable_member_name(name)
                && text_free(child)
        }),
        _ => true,
    }
}

/// Whether `value` is the empty string, an empty array or an empty object (choice 188: a
/// schema's empty instance value, [`super::names::SCHEMA_VALUE_KEYWORDS`]).
pub fn is_empty<J: Json>(value: &J) -> bool {
    match value.kind() {
        Kind::String => value.text().is_some_and(str::is_empty),
        Kind::Array => value.items().is_empty(),
        Kind::Object => value.members().is_empty(),
        _ => false,
    }
}

/// The [`Shape`] of a value.
pub fn shape<J: Json>(value: &J) -> Shape {
    match value.kind() {
        Kind::Null => Shape::Null,
        Kind::Bool => Shape::Bool,
        Kind::Number => Shape::Number,
        Kind::String if value.text().is_some_and(looks_like_credential) => Shape::CredentialLike,
        Kind::String => Shape::String,
        Kind::Array | Kind::Object if text_free(value) => Shape::TextFree,
        Kind::Array | Kind::Object => Shape::Container,
    }
}

/// Whether `value` is a JSON Schema object (rule 3e): an object whose `type` is one of
/// [`SCHEMA_TYPES`] or a non-empty list of them.
pub fn is_schema<J: Json>(value: &J) -> bool {
    let is_type = |name: &J| name.text().is_some_and(|name| SCHEMA_TYPES.contains(&name));
    match get(value, "type") {
        Some(names) if names.kind() == Kind::Array => {
            let names = names.items();
            !names.is_empty() && names.into_iter().all(is_type)
        }
        Some(name) => is_type(name),
        None => false,
    }
}
