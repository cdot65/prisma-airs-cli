//! The CLI's gateway withholding rules (DESIGN §19.3; choices 172, 185), compiled here from the
//! CLI's own source files by `#[path]`, so the live tier's check and the CLI's rewriting
//! traversal read one set of rules by construction, with no second crate to publish and no
//! copy to keep in step.
//!
//! [`check`] is the read-only checker the live tier runs on every Rust-only gateway read (on
//! `serde_json::Value`, whose [`check::Document`] implementation lives in the same file; the
//! CLI compiles it in its tests only); [`names`], [`values`] and [`view`] are the
//! classification, the string rules and the value view; [`corpus`] is the CLI's test corpus
//! (the pinned, generated and rule-independent documents), test data for this crate's tests
//! and the workspace's `tests/*.rs`.

#[path = "../../airs-runtime-core/src/withhold_rules/mod.rs"]
mod rules;

pub use rules::{WITHHELD, names, values, view};

/// The checker (choice 187). The rules module compiles it in a test build only (this crate's
/// own unit tests read it from there); any other build of this crate compiles the same file
/// here, for the live tier and the workspace's tests. The CLI's binary never carries it.
#[cfg(not(test))]
#[path = "../../airs-runtime-core/src/withhold_rules/check.rs"]
pub mod check;
#[cfg(test)]
pub use rules::check;

/// The test corpus (choice 182). The rules module compiles it in a test build (this crate's
/// own unit tests read it from there); any other build of this crate compiles the same file
/// here, for the CLI's and the workspace's tests.
#[cfg(not(test))]
#[path = "../../airs-runtime-core/src/withhold_rules/corpus.rs"]
pub mod corpus;
#[cfg(test)]
pub use rules::corpus;
