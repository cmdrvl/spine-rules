//! operator.v1 contract: validation, schema/struct lockstep, parity assertions.

use spine_rules::contract::{
    OPERATOR_V1_SCHEMA, OperatorContract, assert_required_subcommands, assert_version_parity,
    cargo_package_version,
};

const GOOD: &str = r#"{
  "schema_version": "operator.v1",
  "name": "demo",
  "tool_version": "0.3.0",
  "contract_version": "1.0.0",
  "description": "Demo tool",
  "commands": [
    {"name": "capabilities"}, {"name": "robot-docs"}, {"name": "doctor"},
    {"name": "verify", "refusals": [{"code": "E_IO"}]}
  ],
  "exit_codes": {
    "0": {"meaning": "OK", "domain": "positive"},
    "1": {"meaning": "N", "domain": "negative"},
    "2": {"meaning": "REFUSAL", "domain": "error"}
  },
  "refusals": [{"code": "E_BAD_INPUT", "message": "bad"}],
  "env_vars": [{"name": "NO_COLOR", "description": "disable color"}],
  "formats": ["csv"],
  "side_effects": {"reads_files": true, "writes_files": false, "network": false, "writes_witness": true},
  "pipeline": {"upstream": [], "downstream": ["rvl"]}
}"#;

type Mutation = Box<dyn Fn(&mut OperatorContract)>;

fn contract() -> OperatorContract {
    OperatorContract::parse(GOOD).expect("parses")
}

#[test]
fn good_contract_is_valid_and_round_trips() {
    let c = contract();
    c.assert_valid();
    let again = OperatorContract::parse(&c.to_canonical_json()).unwrap();
    assert_eq!(c, again);
    assert_eq!(c.to_canonical_json(), again.to_canonical_json());
}

#[test]
fn legacy_spellings_do_not_parse_as_operator_v1() {
    // v0 spellings (`version`, `subcommands`) are not accepted by the strict contract,
    // whatever schema_version claims.
    for sv in ["operator.v0", "operator.v1"] {
        let legacy = format!(
            r#"{{
          "schema_version": "{sv}", "name": "old", "version": "0.8.0",
          "description": "d", "subcommands": [{{"name": "doctor"}}],
          "exit_codes": {{"0": {{"meaning": "OK", "domain": "positive"}}, "2": {{"meaning": "R", "domain": "error"}}}},
          "refusals": [], "env_vars": [], "formats": ["csv"], "contract_version": "1.0.0",
          "side_effects": {{"reads_files": true, "writes_files": false, "network": false, "writes_witness": false}},
          "pipeline": {{"upstream": [], "downstream": []}}
        }}"#
        );
        let err = OperatorContract::parse(&legacy).unwrap_err();
        assert!(
            err.contains("tool_version") || err.contains("commands"),
            "{sv}: {err}"
        );
    }
}

#[test]
fn missing_required_sections_refuse_to_parse() {
    for key in [
        "env_vars",
        "formats",
        "side_effects",
        "pipeline",
        "contract_version",
        "tool_version",
    ] {
        let mut doc: serde_json::Value = serde_json::from_str(GOOD).unwrap();
        doc.as_object_mut().unwrap().remove(key);
        assert!(
            OperatorContract::parse(&doc.to_string()).is_err(),
            "{key} must be required"
        );
    }
}

#[test]
fn validation_catches_each_violation() {
    let cases: Vec<(&str, Mutation)> = vec![
        ("tool_version", Box::new(|c| c.tool_version = "v1".into())),
        (
            "exit_codes is missing \"2\"",
            Box::new(|c| {
                c.exit_codes.remove("2");
            }),
        ),
        (
            "outside the family dictionary",
            Box::new(|c| {
                let z = c.exit_codes["0"].clone();
                c.exit_codes.insert("4".into(), z);
            }),
        ),
        (
            "exit code 3 must have domain",
            Box::new(|c| {
                let z = c.exit_codes["0"].clone();
                c.exit_codes.insert("3".into(), z);
            }),
        ),
        (
            "does not start with E_",
            Box::new(|c| c.refusals[0].code = "BAD".into()),
        ),
        (
            "duplicate top-level refusal",
            Box::new(|c| {
                let r = c.refusals[0].clone();
                c.refusals.push(r);
            }),
        ),
        (
            "UPPER_SNAKE_CASE",
            Box::new(|c| c.env_vars[0].name = "no_color".into()),
        ),
        ("formats is empty", Box::new(|c| c.formats.clear())),
        (
            "duplicate command",
            Box::new(|c| {
                let d = c.commands[0].clone();
                c.commands.push(d);
            }),
        ),
        (
            "missing required subcommand \"doctor\"",
            Box::new(|c| c.commands.retain(|x| x.name != "doctor")),
        ),
    ];
    for (needle, mutate) in cases {
        let mut c = contract();
        mutate(&mut c);
        let v = c.validate().join("\n");
        assert!(v.contains(needle), "expected {needle:?} in:\n{v}");
    }
}

/// The published schema and the Rust struct must describe the same top-level keys.
#[test]
fn schema_matches_struct() {
    let schema: serde_json::Value = serde_json::from_str(OPERATOR_V1_SCHEMA).unwrap();
    let props: std::collections::BTreeSet<String> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let required: std::collections::BTreeSet<String> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();

    // Fully populated document: every key the struct can emit.
    let mut doc: serde_json::Value = serde_json::from_str(GOOD).unwrap();
    let o = doc.as_object_mut().unwrap();
    o.insert("repository".into(), "r".into());
    o.insert("license".into(), "MIT".into());
    o.insert("agent_guide".into(), "g".into());
    o.insert("invocation".into(), serde_json::json!({"binary": "demo"}));
    o.insert("arguments".into(), serde_json::json!([]));
    o.insert("options".into(), serde_json::json!([]));
    let full = OperatorContract::parse(&doc.to_string()).unwrap();
    let emitted: std::collections::BTreeSet<String> = serde_json::to_value(&full)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(
        props, emitted,
        "schema properties drifted from struct fields"
    );

    // Required keys == keys that the struct refuses to parse without.
    let mut actually_required = std::collections::BTreeSet::new();
    let base: serde_json::Value = serde_json::from_str(GOOD).unwrap();
    for key in base.as_object().unwrap().keys() {
        let mut d = base.clone();
        d.as_object_mut().unwrap().remove(key);
        if OperatorContract::parse(&d.to_string()).is_err() {
            actually_required.insert(key.clone());
        }
    }
    assert_eq!(
        required, actually_required,
        "schema `required` drifted from struct"
    );
}

#[test]
fn version_parity_detects_drift() {
    let cargo =
        "[package]\nname = \"demo\"\nversion = \"0.3.0\"\n\n[dependencies]\nversion = \"9\"\n";
    assert_eq!(cargo_package_version(cargo).as_deref(), Some("0.3.0"));
    assert_version_parity(GOOD, cargo);
    assert_version_parity(r#"{"version": "0.3.0"}"#, cargo); // v0 manifests
}

#[test]
#[should_panic(expected = "!= Cargo.toml package version")]
fn version_parity_panics_on_mismatch() {
    assert_version_parity(GOOD, "[package]\nversion = \"0.4.0\"\n");
}

#[test]
fn workspace_inherited_version_is_not_silently_accepted() {
    assert_eq!(
        cargo_package_version("[package]\nversion.workspace = true\n"),
        None
    );
}

#[test]
fn subcommand_parity_accepts_v0_and_v1() {
    assert_required_subcommands(GOOD, &["capabilities", "robot-docs", "doctor"]);
    assert_required_subcommands(
        r#"{"subcommands":[{"name":"doctor"},{"name":"capabilities"},{"name":"robot-docs"}]}"#,
        &["capabilities", "robot-docs", "doctor"],
    );
}

#[test]
#[should_panic(expected = "missing required subcommands")]
fn subcommand_parity_catches_omission() {
    assert_required_subcommands(
        r#"{"subcommands":[{"name":"scan"},{"name":"doctor"}]}"#,
        &["capabilities", "robot-docs", "doctor"],
    );
}

// ── generated-suite entry point: full validate() ────────────────────────

use spine_rules::contract::{assert_capabilities_match_describe, assert_operator_valid};
use std::os::unix::fs::PermissionsExt;

#[test]
fn assert_operator_valid_accepts_good_manifest() {
    assert_operator_valid(GOOD);
}

fn mutate(f: impl Fn(&mut serde_json::Value)) -> String {
    let mut doc: serde_json::Value = serde_json::from_str(GOOD).unwrap();
    f(&mut doc);
    doc.to_string()
}

/// `r006_operator_json_is_valid_operator_v1` calls exactly this function.
#[test]
fn bad_manifests_fail_the_generated_suite_check() {
    let bad: Vec<(&str, String)> = vec![
        (
            "schema_version",
            mutate(|d| d["schema_version"] = "operator.v0".into()),
        ),
        (
            "does not start with E_",
            mutate(|d| d["refusals"][0]["code"] = "BAD".into()),
        ),
        (
            "UPPER_SNAKE_CASE",
            mutate(|d| d["env_vars"][0]["name"] = "lower".into()),
        ),
        (
            "formats is empty",
            mutate(|d| d["formats"] = serde_json::json!([])),
        ),
        (
            "does not parse",
            mutate(|d| {
                d.as_object_mut().unwrap().remove("side_effects");
            }),
        ),
        (
            "does not parse",
            mutate(|d| {
                d.as_object_mut().unwrap().remove("pipeline");
            }),
        ),
        (
            "does not parse",
            mutate(|d| {
                d.as_object_mut().unwrap().remove("env_vars");
            }),
        ),
    ];
    for (needle, doc) in bad {
        let err = std::panic::catch_unwind(|| assert_operator_valid(&doc))
            .expect_err("bad manifest must fail");
        let msg = err
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(msg.contains(needle), "expected {needle:?} in panic: {msg}");
    }
}

// ── capabilities --json vs --describe ───────────────────────────────────

fn script(dir: &tempfile::TempDir, body: &str) -> String {
    let path = dir.path().join("tool");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_owned()
}

#[test]
fn capabilities_matching_describe_passes() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, &format!("cat <<'E'\n{}\nE", GOOD.replace('\n', " ")));
    assert_capabilities_match_describe(&bin);
}

#[test]
#[should_panic(expected = "differs from")]
fn capabilities_disagreeing_with_describe_fails() {
    let dir = tempfile::tempdir().unwrap();
    let describe = GOOD.replace('\n', " ");
    let caps = describe.replace("\"0.3.0\"", "\"0.4.0\"");
    let body = format!(
        "if [ \"$1\" = \"--describe\" ]; then cat <<'E'\n{describe}\nE\nelse cat <<'E'\n{caps}\nE\nfi"
    );
    let bin = script(&dir, &body);
    assert_capabilities_match_describe(&bin);
}

// ── schema is never looser than validate() on what it can express ───────

fn schema_accepts(doc: &str) -> bool {
    let schema: serde_json::Value = serde_json::from_str(OPERATOR_V1_SCHEMA).unwrap();
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    validator.is_valid(&serde_json::from_str::<serde_json::Value>(doc).unwrap())
}

#[test]
fn schema_and_validate_agree_on_good_manifest() {
    assert!(schema_accepts(GOOD));
    assert!(contract().validate().is_empty());
}

#[test]
fn schema_and_validate_both_reject_planted_documents() {
    let planted: Vec<(&str, String)> = vec![
        (
            "commands without capabilities",
            mutate(|d| {
                d["commands"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|c| c["name"] != "capabilities")
            }),
        ),
        (
            "exit 3 with domain negative",
            mutate(|d| {
                d["exit_codes"]["3"] = serde_json::json!({"meaning": "BLOCK", "domain": "negative"})
            }),
        ),
        (
            "expected_change without --exit-zero-on-change",
            mutate(|d| {
                d["exit_codes"]["1"] =
                    serde_json::json!({"meaning": "CHANGE", "domain": "expected_change"})
            }),
        ),
        (
            "doctor without exit 1",
            mutate(|d| {
                d["exit_codes"].as_object_mut().unwrap().remove("1");
            }),
        ),
    ];
    for (label, doc) in &planted {
        assert!(!schema_accepts(doc), "schema accepted: {label}");
        let c = OperatorContract::parse(doc).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert!(!c.validate().is_empty(), "validate() accepted: {label}");
    }
    // ...and the same expected_change document becomes valid in both once the flag is declared.
    let fixed = mutate(|d| {
        d["exit_codes"]["1"] =
            serde_json::json!({"meaning": "CHANGE", "domain": "expected_change"});
        d["options"] = serde_json::json!([{"flag": "--exit-zero-on-change"}]);
    });
    assert!(schema_accepts(&fixed));
    assert!(
        OperatorContract::parse(&fixed)
            .unwrap()
            .validate()
            .is_empty()
    );
}
