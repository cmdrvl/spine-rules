//! Doctor contract and family conventions. Runtime helpers run against real
//! shell-script binaries written to a temp dir.

use spine_rules::conventions::{
    assert_bare_invocation_does_not_hang, assert_no_ansi_when_piped,
    assert_source_date_epoch_deterministic, contains_ansi, lint_flags, run_with_timeout,
};
use spine_rules::doctor::{DoctorMode, DoctorReport, assert_doctor_contract, check_doctor_run};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

fn script(dir: &tempfile::TempDir, name: &str, body: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_owned()
}

const CONTRACT: &str = r#"{
  "schema_version": "operator.v1", "name": "demo", "tool_version": "0.3.0",
  "contract_version": "1.0.0", "description": "d",
  "commands": [{"name":"capabilities"},{"name":"robot-docs"},{"name":"doctor"}],
  "exit_codes": {"0":{"meaning":"OK","domain":"positive"},"1":{"meaning":"N","domain":"negative"},"2":{"meaning":"R","domain":"error"}},
  "refusals": [], "env_vars": [], "formats": ["csv"],
  "side_effects": {"reads_files":true,"writes_files":false,"network":false,"writes_witness":false},
  "pipeline": {"upstream":[],"downstream":[]}
}"#;

fn report(mode: &str, healthy: bool, extra: &str) -> String {
    let findings = if healthy {
        "[]"
    } else {
        r#"[{"id":"witness_unwritable","severity":"error","message":"cannot write ledger"}]"#
    };
    format!(
        r#"{{"schema_version":"cmdrvl.doctor.v1","mode":"{mode}","tool":"demo","tool_version":"0.3.0","healthy":{healthy},"findings":{findings}{extra}}}"#
    )
}

// ── doctor ──────────────────────────────────────────────────────────────

#[test]
fn healthy_exits_zero_unhealthy_exits_one() {
    let ok = report("health", true, "");
    assert!(check_doctor_run(DoctorMode::Health, ok.as_bytes(), Some(0)).is_empty());
    let bad = report("health", false, "");
    assert!(check_doctor_run(DoctorMode::Health, bad.as_bytes(), Some(1)).is_empty());
}

#[test]
fn unhealthy_with_exit_two_is_rejected() {
    // verify's current behavior (doctor.rs:106) — must be flagged.
    let bad = report("health", false, "");
    let v = check_doctor_run(DoctorMode::Health, bad.as_bytes(), Some(2)).join("\n");
    assert!(v.contains("requires 1"), "{v}");
}

#[test]
fn healthy_with_nonzero_exit_is_rejected() {
    let ok = report("health", true, "");
    assert!(!check_doctor_run(DoctorMode::Health, ok.as_bytes(), Some(1)).is_empty());
}

#[test]
fn healthy_flag_must_match_findings() {
    let lie = report("health", true, "")
        .replace("[]", r#"[{"id":"x","severity":"error","message":"m"}]"#);
    let v = check_doctor_run(DoctorMode::Health, lie.as_bytes(), Some(0)).join("\n");
    assert!(v.contains("contradicts findings"), "{v}");
}

#[test]
fn mode_specific_fields_enforced() {
    let v = check_doctor_run(
        DoctorMode::Capabilities,
        report("capabilities", true, "").as_bytes(),
        Some(0),
    )
    .join("\n");
    assert!(v.contains("requires `contract`"), "{v}");
    let v = check_doctor_run(
        DoctorMode::Triage,
        report("triage", true, "").as_bytes(),
        Some(0),
    )
    .join("\n");
    assert!(v.contains("requires `next_actions`"), "{v}");
    let v = check_doctor_run(
        DoctorMode::Health,
        report("health", true, r#","next_actions":["x"]"#).as_bytes(),
        Some(0),
    )
    .join("\n");
    assert!(v.contains("only allowed in triage"), "{v}");
    let v = check_doctor_run(
        DoctorMode::Triage,
        report("health", true, "").as_bytes(),
        Some(0),
    )
    .join("\n");
    assert!(v.contains("expected Triage"), "{v}");
}

#[test]
fn capabilities_report_embeds_valid_contract() {
    let good = report("capabilities", true, &format!(r#","contract":{CONTRACT}"#));
    assert!(check_doctor_run(DoctorMode::Capabilities, good.as_bytes(), Some(0)).is_empty());
    let drift = good.replace(r#""tool_version": "0.3.0""#, r#""tool_version": "0.4.0""#);
    let v = check_doctor_run(DoctorMode::Capabilities, drift.as_bytes(), Some(0)).join("\n");
    assert!(v.contains("differ"), "{v}");
}

#[test]
fn unhealthy_triage_needs_next_actions() {
    let t = report("triage", false, r#","next_actions":[]"#);
    let v = check_doctor_run(DoctorMode::Triage, t.as_bytes(), Some(1)).join("\n");
    assert!(v.contains("empty next_actions"), "{v}");
    let t = report("triage", false, r#","next_actions":["demo doctor --fix"]"#);
    assert!(check_doctor_run(DoctorMode::Triage, t.as_bytes(), Some(1)).is_empty());
}

#[test]
fn report_round_trips() {
    let r = DoctorReport::parse(&report("triage", false, r#","next_actions":["a"]"#)).unwrap();
    assert_eq!(
        DoctorReport::parse(&serde_json::to_string(&r).unwrap()).unwrap(),
        r
    );
    assert!(DoctorReport::parse(r#"{"schema_version":"x"}"#).is_err());
}

#[test]
fn doctor_contract_runs_real_binary() {
    let dir = tempfile::tempdir().unwrap();
    let body = format!(
        "case \"$*\" in\n\
         'doctor --json') cat <<'E'\n{}\nE\n ;;\n\
         'doctor capabilities --json') cat <<'E'\n{}\nE\n ;;\n\
         'capabilities --json') cat <<'E'\n{}\nE\n ;;\n\
         'doctor triage --json') cat <<'E'\n{}\nE\n exit 1 ;;\n\
         esac",
        report("health", true, ""),
        report(
            "capabilities",
            true,
            &format!(r#","contract":{}"#, CONTRACT.replace('\n', " "))
        ),
        CONTRACT.replace('\n', " "),
        report("triage", false, r#","next_actions":["demo doctor --fix"]"#),
    );
    let bin = script(&dir, "demo", &body);
    for mode in DoctorMode::ALL {
        assert_doctor_contract(&bin, mode);
    }
}

#[test]
#[should_panic(expected = "violates cmdrvl.doctor.v1")]
fn doctor_contract_fails_real_binary_exiting_two() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(
        &dir,
        "demo",
        &format!("cat <<'E'\n{}\nE\nexit 2", report("health", false, "")),
    );
    assert_doctor_contract(&bin, DoctorMode::Health);
}

// ── flag dictionary ─────────────────────────────────────────────────────

#[test]
fn flag_lint_accepts_alias_and_rejects_legacy_primary() {
    let ok = r#"{"options":[{"flag":"--out","aliases":["--output"]},{"flag":"--capsule-dir","aliases":["--capsule-out"]},{"flag":"--json"}]}"#;
    assert!(lint_flags(ok).is_empty(), "{:?}", lint_flags(ok));

    let v = lint_flags(r#"{"options":[{"flag":"--output"},{"flag":"--capsule-out"}]}"#).join("\n");
    assert!(v.contains("--output as primary; use --out"), "{v}");
    assert!(
        v.contains("--capsule-out as primary; use --capsule-dir"),
        "{v}"
    );

    let v = lint_flags(r#"{"options":[{"flag":"--json","aliases":["--output"]}]}"#).join("\n");
    assert!(v.contains("must belong to --out"), "{v}");

    let v = lint_flags(r#"{"options":[{"flag":"--out"},{"flag":"--out"}]}"#).join("\n");
    assert!(v.contains("duplicate"), "{v}");
}

// ── conventions against real scripts ────────────────────────────────────

#[test]
fn ansi_detection() {
    assert!(contains_ansi(b"\x1b[31mred"));
    assert!(!contains_ansi(b"plain [31m text"));
}

#[test]
fn clean_output_passes_ansi_check() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "clean", "echo plain");
    assert_no_ansi_when_piped(&bin, &["--describe"]);
}

#[test]
#[should_panic(expected = "emitted ANSI escapes")]
fn color_forced_on_stderr_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "colorful", r#"printf '\033[31mbad\033[0m\n' >&2"#);
    assert_no_ansi_when_piped(&bin, &[]);
}

#[test]
#[should_panic(expected = "emitted ANSI escapes")]
fn color_unless_no_color_is_caught_in_non_tty_mode() {
    // Honors NO_COLOR but colors by default even though stdout is a pipe.
    let dir = tempfile::tempdir().unwrap();
    let bin = script(
        &dir,
        "half",
        r#"[ -z "$NO_COLOR" ] && printf '\033[1mx\033[0m\n'; exit 0"#,
    );
    assert_no_ansi_when_piped(&bin, &[]);
}

#[test]
fn source_date_epoch_honored_passes() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(
        &dir,
        "ts",
        r#"echo "generated_at=${SOURCE_DATE_EPOCH:-$(date +%s)}""#,
    );
    assert_source_date_epoch_deterministic(&bin, &[]);
}

#[test]
#[should_panic(expected = "stdout differs between runs")]
fn source_date_epoch_ignored_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "ts", "date +%s");
    assert_source_date_epoch_deterministic(&bin, &[]);
}

#[test]
fn bare_invocation_that_exits_passes() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "usage", "echo usage >&2; exit 2");
    assert_bare_invocation_does_not_hang(&bin);
}

#[test]
#[should_panic(expected = "must not hang")]
fn bare_invocation_that_never_exits_is_caught() {
    // A process that never exits within the timeout.
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "hang", "sleep 30");
    run_with_timeout(&bin, &[], Duration::from_millis(300));
}

// ── contract embedded in doctor == capabilities --json ──────────────────

use spine_rules::doctor::{DOCTOR_V1_SCHEMA, check_contract_equals_capabilities};

#[test]
fn embedded_contract_must_equal_capabilities_document() {
    let caps = CONTRACT.replace('\n', " ");
    let good = report("capabilities", true, &format!(r#","contract":{caps}"#));
    assert!(check_contract_equals_capabilities(good.as_bytes(), caps.as_bytes()).is_empty());

    // Embedded copy is valid on its own but is not what `capabilities --json` prints.
    let other = caps
        .replace("\"Demo\"", "\"x\"")
        .replace("\"d\"", "\"different\"");
    let v = check_contract_equals_capabilities(good.as_bytes(), other.as_bytes()).join("\n");
    assert!(v.contains("differs"), "{v}");

    let no_contract = report("capabilities", true, "");
    let v = check_contract_equals_capabilities(no_contract.as_bytes(), caps.as_bytes()).join("\n");
    assert!(v.contains("no `contract`"), "{v}");
}

#[test]
#[should_panic(expected = "differs from `capabilities --json`")]
fn real_binary_with_diverging_doctor_contract_fails() {
    let dir = tempfile::tempdir().unwrap();
    let caps = CONTRACT.replace('\n', " ");
    let embedded = caps.replace("\"d\"", "\"stale copy\"");
    let body = format!(
        "case \"$*\" in\n\
         'doctor capabilities --json') cat <<'E'\n{}\nE\n ;;\n\
         'capabilities --json') cat <<'E'\n{}\nE\n ;;\n\
         esac",
        report("capabilities", true, &format!(r#","contract":{embedded}"#)),
        caps,
    );
    let bin = script(&dir, "demo", &body);
    assert_doctor_contract(&bin, DoctorMode::Capabilities);
}

// ── cmdrvl.doctor.v1 schema ─────────────────────────────────────────────

fn doctor_schema_accepts(doc: &str) -> bool {
    let schema: serde_json::Value = serde_json::from_str(DOCTOR_V1_SCHEMA).unwrap();
    let validator = jsonschema::validator_for(&schema).expect("doctor schema compiles");
    validator.is_valid(&serde_json::from_str::<serde_json::Value>(doc).unwrap())
}

#[test]
fn doctor_schema_matches_struct() {
    let schema: serde_json::Value = serde_json::from_str(DOCTOR_V1_SCHEMA).unwrap();
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

    // Every key the struct can emit.
    let full = DoctorReport::parse(&report(
        "capabilities",
        true,
        &format!(r#","contract":{CONTRACT},"next_actions":["a"]"#),
    ))
    .unwrap();
    let mut value = serde_json::to_value(&full).unwrap();
    value["findings"] = serde_json::json!([{"id":"i","severity":"info","message":"m","fix":"f"}]);
    let emitted: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        props, emitted,
        "doctor schema properties drifted from struct"
    );

    let base: serde_json::Value = serde_json::from_str(&report("health", true, "")).unwrap();
    let mut actually_required = std::collections::BTreeSet::new();
    for key in base.as_object().unwrap().keys() {
        let mut d = base.clone();
        d.as_object_mut().unwrap().remove(key);
        if DoctorReport::parse(&d.to_string()).is_err() {
            actually_required.insert(key.clone());
        }
    }
    assert_eq!(
        required, actually_required,
        "doctor schema `required` drifted"
    );

    let finding_props: std::collections::BTreeSet<String> =
        schema["properties"]["findings"]["items"]["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
    let finding_emitted: std::collections::BTreeSet<String> = value["findings"][0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(finding_props, finding_emitted);
}

#[test]
fn doctor_schema_and_validate_agree() {
    let contract = CONTRACT.replace('\n', " ");
    let accepted = [
        report("health", true, ""),
        report("health", false, ""),
        report("capabilities", true, &format!(r#","contract":{contract}"#)),
        report("triage", false, r#","next_actions":["demo doctor --fix"]"#),
    ];
    for doc in &accepted {
        assert!(doctor_schema_accepts(doc), "schema rejected {doc}");
        assert!(
            DoctorReport::parse(doc).unwrap().validate().is_empty(),
            "{doc}"
        );
    }

    let rejected = [
        // healthy contradicts an error finding
        report("health", false, "").replace("\"healthy\":false", "\"healthy\":true"),
        // capabilities without contract
        report("capabilities", true, ""),
        // triage without next_actions
        report("triage", true, ""),
        // next_actions outside triage
        report("health", true, r#","next_actions":["x"]"#),
        // unhealthy triage with empty next_actions
        report("triage", false, r#","next_actions":[]"#),
        // contract outside capabilities
        report("health", true, &format!(r#","contract":{contract}"#)),
    ];
    for doc in &rejected {
        assert!(!doctor_schema_accepts(doc), "schema accepted {doc}");
        assert!(
            !DoctorReport::parse(doc).unwrap().validate().is_empty(),
            "{doc}"
        );
    }
}
