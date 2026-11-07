//! The credential-withholding rules of the gateway commands (DESIGN §19.3 rules 3 and 4, D238,
//! D251): every decision on a name, a string or a printed document, made by one set of files
//! that the CLI and its live tier both compile (choices 172, 185, 187), so the CLI's traversal and
//! the live tier's check read the same rules by construction.
//!
//! - [`names`]: the name rule as data. A member name is normalised, classified by the first
//!   row of [`names::NAME_RULES`] that matches it (a [`names::Class`]), and the class and the
//!   value's [`names::Shape`] give the [`names::Action`] ([`names::action`]): read the value
//!   by the document's rules, read it as a header map, print it, or withhold it and register
//!   it as a [`names::Leaf`]; [`names::registrable`] is the one filter on what a withheld
//!   string may register (choice 183).
//! - [`values`]: the tests of a string (looks like a credential or a password, a date, a JSON
//!   document, a private key, the words after an authorization scheme, a credential header's
//!   value, a credential in a `key=value` segment), the URL parser of rule 4, the four text
//!   rules in order ([`values::text_withheld`]) and the model-output subset
//!   ([`values::model_output_withheld`]).
//! - [`view`]: the read-only view of a JSON value both traversals read shapes through (the
//!   [`view::Json`] trait, implemented by the CLI for its own value type; a value's
//!   [`names::Shape`]; whether an object is a JSON Schema).
//! - `check` (test builds only; the harness compiles it in every build): the read-only checker
//!   of a printed document (`check::withheld`), generic over `check::Document` (a
//!   [`view::Json`] that parses a JSON text; implemented there for `serde_json::Value`, and by
//!   the CLI's tests for its own value type), which the live tier runs on every Rust-only read
//!   and the CLI's tests run on everything the CLI prints. The binary never checks what it
//!   printed, so it does not carry the checker (choice 187).
//! - `corpus` (test builds only): the pinned documents, the generated documents (a planted
//!   credential under every name shape and in every container shape) and the rule-independent
//!   names and texts that the CLI's and the harness's tests iterate (choice 182). It is test
//!   data, never compiled into the CLI.
//!
//! The shared traversal (`airs-runtime-core/src/withhold.rs`) rewrites its values and registers what
//! it withholds; it reads every name, pair, header, schema and string through this module.
//!
//! These files are also compiled by the unpublished test harness
//! (`crates/airs-test-harness/src/withhold.rs`, by `#[path]`), whose crate root is not the
//! CLI's: inside them a sibling is reached as `super::`, never as `crate::`, and they use
//! `std` and `serde_json` only (the CLI's test `the_rules_compile_outside_the_cli` holds it).

/// The checker (choice 187): compiled into test builds only (this crate's, and the CLI's
/// through the dev-only `test-support` feature). The harness compiles the same file in
/// every build, for the live tier (`airs_test_harness::withhold::check`).
#[cfg(any(test, feature = "test-support"))]
pub mod check;
/// The test corpus (choice 182): compiled into test builds only. The harness compiles the
/// same file for the workspace's other tests (`airs_test_harness::withhold::corpus`).
#[cfg(any(test, feature = "test-support"))]
pub mod corpus;
pub mod names;
pub mod values;
pub mod view;

/// The marker of a withheld value (D238, the D47 word).
pub const WITHHELD: &str = "[withheld]";
