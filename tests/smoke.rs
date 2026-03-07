//! Smoke tests: types deserialize, helpers compile and work standalone.

use spine_rules::helpers;
use spine_rules::types::OperatorManifest;

const CANON_OPERATOR: &str = r#"{
  "schema_version": "operator.v0",
  "name": "canon",
  "version": "0.2.1",
  "description": "Canonical identifier resolution",
  "exit_codes": {
    "0": { "meaning": "RESOLVED", "domain": "positive" },
    "1": { "meaning": "PARTIAL", "domain": "negative" },
    "2": { "meaning": "REFUSAL", "domain": "error" }
  },
  "refusals": [
    { "code": "E_IO", "message": "Cannot read file" },
    { "code": "E_ENCODING", "message": "Bad encoding" }
  ],
  "subcommands": []
}"#;

const VACUUM_OPERATOR: &str = r#"{
  "schema_version": "operator.v0",
  "name": "vacuum",
  "version": "0.2.1",
  "description": "Artifact enumerator",
  "exit_codes": {
    "0": { "meaning": "SCAN_COMPLETE", "domain": "positive" },
    "2": { "meaning": "REFUSAL", "domain": "error" }
  },
  "refusals": [
    { "code": "E_ROOT_NOT_FOUND", "message": "Root path doesn't exist" }
  ],
  "subcommands": []
}"#;

const LOCK_OPERATOR: &str = r#"{
  "schema_version": "operator.v0",
  "name": "lock",
  "version": "0.3.1",
  "description": "Dataset lockfile tool",
  "exit_codes": {
    "0": { "meaning": "LOCK_CREATED", "domain": "positive" },
    "1": { "meaning": "LOCK_PARTIAL", "domain": "negative" },
    "2": { "meaning": "REFUSAL", "domain": "error" }
  },
  "refusals": [
    { "code": "E_EMPTY", "message": "No input records" },
    { "code": "E_BAD_INPUT", "message": "Invalid JSONL" },
    { "code": "E_MISSING_HASH", "message": "Records lack bytes_hash" }
  ],
  "subcommands": [
    {
      "name": "verify",
      "description": "Verify lockfile integrity",
      "refusals": [
        { "code": "E_IO", "message": "Cannot read lockfile" },
        { "code": "E_BAD_LOCKFILE", "message": "Malformed JSON" },
        { "code": "E_UNSUPPORTED_VERSION", "message": "Version not supported" },
        { "code": "E_ROOT_NOT_FOUND", "message": "Root dir missing" },
        { "code": "E_UNKNOWN_ALGORITHM", "message": "Unknown hash algo" }
      ]
    }
  ]
}"#;

#[test]
fn types_deserialize_canon() {
    let m: OperatorManifest = serde_json::from_str(CANON_OPERATOR).unwrap();
    assert_eq!(m.name, "canon");
    assert_eq!(m.exit_codes.len(), 3);
    assert_eq!(m.refusals.len(), 2);
}

#[test]
fn types_deserialize_vacuum_no_exit_1() {
    let m: OperatorManifest = serde_json::from_str(VACUUM_OPERATOR).unwrap();
    assert_eq!(m.name, "vacuum");
    assert_eq!(m.exit_codes.len(), 2);
    assert!(!m.exit_codes.contains_key("1"));
}

#[test]
fn types_deserialize_lock_with_subcommand_refusals() {
    let m: OperatorManifest = serde_json::from_str(LOCK_OPERATOR).unwrap();
    assert_eq!(m.name, "lock");
    assert_eq!(m.subcommands.len(), 1);
    assert_eq!(m.subcommands[0].refusals.len(), 5);
}

#[test]
fn exit_codes_trinity_standard() {
    helpers::assert_exit_codes_trinity(CANON_OPERATOR, true);
}

#[test]
fn exit_codes_trinity_no_exit_1() {
    helpers::assert_exit_codes_trinity(VACUUM_OPERATOR, false);
}

#[test]
#[should_panic(expected = "R-002/R-007")]
fn exit_codes_trinity_fails_when_1_required_but_missing() {
    helpers::assert_exit_codes_trinity(VACUUM_OPERATOR, true);
}

#[test]
fn collect_refusal_codes_includes_subcommands() {
    let codes = helpers::collect_refusal_codes_from_manifest(LOCK_OPERATOR);
    // Top-level: E_EMPTY, E_BAD_INPUT, E_MISSING_HASH
    // Subcommand: E_IO, E_BAD_LOCKFILE, E_UNSUPPORTED_VERSION, E_ROOT_NOT_FOUND, E_UNKNOWN_ALGORITHM
    assert_eq!(codes.len(), 8);
    assert!(codes.contains("E_EMPTY"));
    assert!(codes.contains("E_BAD_LOCKFILE"));
    assert!(codes.contains("E_UNKNOWN_ALGORITHM"));
}

#[test]
fn extract_e_codes_from_source_works() {
    let source = r#"
        fn handle_error(e: Error) -> RefusalCode {
            match e {
                Error::Io(_) => "E_IO",
                Error::Parse(_) => "E_CSV_PARSE",
            }
        }
        // Not a code: "EXAMPLE_THING"
        let msg = "E_ENCODING";
    "#;

    let codes = helpers::extract_e_codes_from_source(source);
    assert!(codes.contains("E_IO"));
    assert!(codes.contains("E_CSV_PARSE"));
    assert!(codes.contains("E_ENCODING"));
    assert!(!codes.contains("EXAMPLE_THING"));
    assert_eq!(codes.len(), 3);
}
