//! # spine-rules
//!
//! Shared golden-rules enforcement for CMD+RVL spine tools.
//!
//! Add as a dev-dependency and invoke the [`golden_rules_suite!`] macro
//! to generate a standard test suite that enforces spine protocol invariants
//! (R-001 through R-022).
//!
//! ## Quick start
//!
//! ```ignore
//! // tests/golden_rules.rs
//! use spine_rules::golden_rules_suite;
//!
//! golden_rules_suite! {
//!     binary: env!("CARGO_BIN_EXE_mytool"),
//!     operator_json: include_str!("../operator.json"),
//!     source_files: &["src/lib.rs"],
//! }
//! ```

pub mod helpers;
#[doc(hidden)]
pub mod macros;
pub mod types;
