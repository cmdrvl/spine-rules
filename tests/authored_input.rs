//! Tests for the authored-input self-discovery helper, including planted failures.

use spine_rules::authored_input::check_self_discovery;
use std::os::unix::fs::PermissionsExt;

const OPERATOR: &str = r#"{
  "schema_version": "operator.v1",
  "name": "demo",
  "tool_version": "0.3.0",
  "contract_version": "1.0.0",
  "description": "Demo tool",
  "commands": [
    {"name": "capabilities"},
    {"name": "robot-docs"},
    {"name": "doctor"},
    {"name": "ground", "authored_inputs": [{
      "name": "rules",
      "schema_flag": "--input-schema",
      "example": "demo ground rules.jsonl",
      "allowed_values": ["exact", "numeric"]
    }]}
  ],
  "exit_codes": {
    "0": {"meaning": "OK", "domain": "positive"},
    "1": {"meaning": "N", "domain": "negative"},
    "2": {"meaning": "REFUSAL", "domain": "error"}
  },
  "refusals": [],
  "env_vars": [],
  "formats": ["jsonl"],
  "side_effects": {"reads_files": true, "writes_files": false, "network": false, "writes_witness": true},
  "pipeline": {"upstream": [], "downstream": []}
}"#;

fn script(dir: &tempfile::TempDir, body: &str) -> String {
    let path = dir.path().join("demo");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_owned()
}

fn conforming_script(dir: &tempfile::TempDir) -> String {
    script(
        dir,
        r#"case "$1 $2" in
  "ground --input-schema") printf '%s\n' '{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"kind":{"enum":["exact","numeric"]}}}' ;;
  "ground --describe") printf '%s\n' '{"name":"ground","authored_inputs":[{"name":"rules","schema_flag":"--input-schema"}]}' ;;
  "ground --help"|"robot-docs guide") printf '%s\n' 'Example: demo ground rules.jsonl' 'Allowed values: exact, numeric' ;;
  *) exit 2 ;;
esac"#,
    )
}

#[test]
fn conforming_tool_passes_metadata_help_and_guide_checks() {
    let dir = tempfile::tempdir().unwrap();
    let binary = conforming_script(&dir);
    let violations = check_self_discovery(&binary, OPERATOR, &["robot-docs", "guide"]);
    assert!(
        violations.is_empty(),
        "unexpected violations: {violations:#?}"
    );
}

#[test]
fn planted_tool_that_requires_authored_files_fails() {
    let dir = tempfile::tempdir().unwrap();
    let binary = script(
        &dir,
        r#"case "$1 $2" in
  "ground --input-schema"|"ground --describe") exit 2 ;;
  "ground --help"|"robot-docs guide") printf '%s\n' 'Allowed values: exact, numeric' ;;
  *) exit 2 ;;
esac"#,
    );
    let violations = check_self_discovery(&binary, OPERATOR, &["robot-docs", "guide"]);
    assert!(
        violations
            .iter()
            .any(|v| v.contains("must require no file") || v.contains("must be standalone")),
        "the required-file regression was not detected: {violations:#?}"
    );
    assert!(
        violations.iter().any(|v| v.contains("copy-paste example")),
        "the missing example was not detected: {violations:#?}"
    );
}

#[test]
fn empty_example_cannot_satisfy_the_contract() {
    let dir = tempfile::tempdir().unwrap();
    let binary = conforming_script(&dir);
    let operator = OPERATOR.replace("demo ground rules.jsonl", "");
    let violations = check_self_discovery(&binary, &operator, &["robot-docs", "guide"]);
    assert!(
        violations.iter().any(|v| v.contains("empty example")),
        "empty authored example was accepted: {violations:#?}"
    );
}
