//! The R-002 / R-007 helpers must accept a REAL operator.v1 manifest and reject
//! a legacy-spelling file that merely declares schema_version operator.v1.

use spine_rules::contract::OperatorContract;
use spine_rules::helpers::{
    assert_exit_codes_trinity, assert_refusal_codes_match_source,
    collect_refusal_codes_from_manifest,
};
use std::panic::catch_unwind;

const V1: &str = r#"{
  "schema_version": "operator.v1", "name": "demo", "tool_version": "0.3.0",
  "contract_version": "1.0.0", "description": "d",
  "commands": [
    {"name": "capabilities"}, {"name": "robot-docs"}, {"name": "doctor"},
    {"name": "verify", "refusals": [{"code": "E_BAD_LOCKFILE"}]}
  ],
  "exit_codes": {
    "0": {"meaning": "OK", "domain": "positive"},
    "1": {"meaning": "NO (doctor: unhealthy)", "domain": "negative"},
    "2": {"meaning": "REFUSAL", "domain": "error"}
  },
  "refusals": [{"code": "E_IO"}],
  "env_vars": [], "formats": ["csv"],
  "side_effects": {"reads_files": true, "writes_files": false, "network": false, "writes_witness": false},
  "pipeline": {"upstream": [], "downstream": []}
}"#;

/// Same facts as V1 but with the legacy spellings, still claiming operator.v1.
const LEGACY_SPELLING_V1: &str = r#"{
  "schema_version": "operator.v1", "name": "demo", "version": "0.3.0",
  "contract_version": "1.0.0", "description": "d",
  "subcommands": [
    {"name": "capabilities"}, {"name": "robot-docs"}, {"name": "doctor"},
    {"name": "verify", "refusals": [{"code": "E_BAD_LOCKFILE"}]}
  ],
  "exit_codes": {
    "0": {"meaning": "OK", "domain": "positive"},
    "1": {"meaning": "NO", "domain": "negative"},
    "2": {"meaning": "REFUSAL", "domain": "error"}
  },
  "refusals": [{"code": "E_IO"}],
  "env_vars": [], "formats": ["csv"],
  "side_effects": {"reads_files": true, "writes_files": false, "network": false, "writes_witness": false},
  "pipeline": {"upstream": [], "downstream": []}
}"#;

fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
    let err = catch_unwind(f).expect_err("expected a panic");
    err.downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

#[test]
fn real_v1_manifest_passes_trinity_and_refusal_collection() {
    OperatorContract::parse(V1).unwrap().assert_valid();
    assert_exit_codes_trinity(V1, true);
    let codes = collect_refusal_codes_from_manifest(V1);
    assert_eq!(
        codes.iter().map(String::as_str).collect::<Vec<_>>(),
        ["E_BAD_LOCKFILE", "E_IO"]
    );
}

#[test]
fn real_v1_manifest_passes_refusal_source_match() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.rs"),
        "const A: &str = \"E_IO\";\nconst B: &str = \"E_BAD_LOCKFILE\";\n",
    )
    .unwrap();
    assert_refusal_codes_match_source(V1, &["lib.rs"], dir.path().to_str().unwrap());
}

#[test]
fn legacy_spelling_manifest_claiming_v1_fails_the_helpers() {
    let m = panic_message(|| assert_exit_codes_trinity(LEGACY_SPELLING_V1, true));
    assert!(
        m.contains("operator.v1 manifest is invalid") && m.contains("tool_version"),
        "{m}"
    );
    let m = panic_message(|| {
        collect_refusal_codes_from_manifest(LEGACY_SPELLING_V1);
    });
    assert!(m.contains("operator.v1 manifest is invalid"), "{m}");
}

#[test]
fn legacy_v0_manifest_still_works_and_unknown_schema_is_rejected() {
    let v0 = r#"{
      "schema_version": "operator.v0", "name": "old", "version": "0.8.0", "description": "d",
      "subcommands": [{"name": "scan", "refusals": [{"code": "E_X"}]}],
      "exit_codes": {"0": {"meaning": "OK", "domain": "positive"}, "1": {"meaning": "N", "domain": "negative"}, "2": {"meaning": "R", "domain": "error"}},
      "refusals": [{"code": "E_Y"}]
    }"#;
    assert_exit_codes_trinity(v0, true);
    assert_eq!(collect_refusal_codes_from_manifest(v0).len(), 2);

    let other = v0.replace("operator.v0", "operator.v9");
    let m = panic_message(|| assert_exit_codes_trinity(&other, true));
    assert!(m.contains("unsupported"), "{m}");
}

// ── a tool with no verdict and no refusal envelope (e.g. veil) ──────────

fn no_refusal_tool() -> String {
    V1.replace(
        r#"{"name": "verify", "refusals": [{"code": "E_BAD_LOCKFILE"}]}"#,
        r#"{"name": "scan"}"#,
    )
    .replace(r#""refusals": [{"code": "E_IO"}],"#, r#""refusals": [],"#)
    .replace("NO (doctor: unhealthy)", "doctor: unhealthy")
    .replace(r#""domain": "negative""#, r#""domain": "unhealthy""#)
}

#[test]
fn empty_refusals_is_legal_and_requires_clean_source() {
    let m = no_refusal_tool();
    OperatorContract::parse(&m).unwrap().assert_valid();
    assert_exit_codes_trinity(&m, true);
    assert!(collect_refusal_codes_from_manifest(&m).is_empty());

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "fn main() {}\n").unwrap();
    assert_refusal_codes_match_source(&m, &["lib.rs"], dir.path().to_str().unwrap());

    // ...but source that emits an undeclared E_ code is still caught.
    std::fs::write(dir.path().join("lib.rs"), "const A: &str = \"E_SECRET\";\n").unwrap();
    let msg = panic_message(|| {
        assert_refusal_codes_match_source(&m, &["lib.rs"], dir.path().to_str().unwrap())
    });
    assert!(msg.contains("Source emits 'E_SECRET'"), "{msg}");
}

#[test]
fn non_e_refusal_is_rejected_by_validate_and_by_the_source_check() {
    let bad = V1.replace(
        r#""refusals": [{"code": "E_IO"}]"#,
        r#""refusals": [{"code": "IO_ERROR"}]"#,
    );
    let v = OperatorContract::parse(&bad).unwrap().validate().join("\n");
    assert!(v.contains("does not start with E_"), "{v}");

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("lib.rs"),
        "const A: &str = \"IO_ERROR\";\nconst B: &str = \"E_BAD_LOCKFILE\";\n",
    )
    .unwrap();
    let msg = panic_message(|| {
        assert_refusal_codes_match_source(&bad, &["lib.rs"], dir.path().to_str().unwrap())
    });
    assert!(msg.contains("must start with E_"), "{msg}");
}
