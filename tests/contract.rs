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
fn v0_aliases_parse_but_fail_validation() {
    let v0 = r#"{
      "schema_version": "operator.v0", "name": "old", "version": "0.8.0",
      "description": "d", "subcommands": [{"name": "doctor"}],
      "exit_codes": {"0": {"meaning": "OK", "domain": "positive"}, "2": {"meaning": "R", "domain": "error"}},
      "refusals": [], "env_vars": [], "formats": ["csv"], "contract_version": "1.0.0",
      "side_effects": {"reads_files": true, "writes_files": false, "network": false, "writes_witness": false},
      "pipeline": {"upstream": [], "downstream": []}
    }"#;
    let c = OperatorContract::parse(v0).unwrap();
    assert_eq!(c.tool_version, "0.8.0");
    let v = c.validate().join("\n");
    assert!(v.contains("schema_version"), "{v}");
    assert!(v.contains("\"capabilities\""), "{v}");
    assert!(v.contains("\"robot-docs\""), "{v}");
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
