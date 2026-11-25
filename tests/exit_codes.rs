//! Exit-code dictionary: policy band vs refusal, expected_change opt-in,
//! empty query results.

use spine_rules::exit_codes::{assert_empty_query_exits_zero, check_exit_codes};
use spine_rules::helpers::assert_exit_codes_trinity;
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;

fn flags(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn script(dir: &tempfile::TempDir, body: &str) -> String {
    let path = dir.path().join("tool");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_owned()
}

#[test]
fn legacy_trinity_domains_still_pass() {
    let v = check_exit_codes(
        &[("0", "positive"), ("1", "negative"), ("2", "error")],
        &flags(&[]),
        false,
    );
    assert!(v.is_empty(), "{v:?}");
    assert!(check_exit_codes(&[("0", "positive"), ("2", "error")], &flags(&[]), false).is_empty());
}

#[test]
fn assess_style_policy_band_is_separate_from_refusal() {
    // assess: BLOCK is a valid verdict (3), REFUSAL/CLI error stays 2.
    let codes = [
        ("0", "positive"),
        ("1", "negative"),
        ("2", "error"),
        ("3", "policy"),
    ];
    assert!(check_exit_codes(&codes, &flags(&[]), false).is_empty());
}

#[test]
fn exit_three_must_be_policy_and_policy_must_be_three() {
    let v = check_exit_codes(
        &[("0", "positive"), ("2", "error"), ("3", "error")],
        &flags(&[]),
        false,
    )
    .join("\n");
    assert!(v.contains("exit code 3 must have domain"), "{v}");
    let v = check_exit_codes(&[("0", "positive"), ("2", "policy")], &flags(&[]), false).join("\n");
    assert!(v.contains("exit code 2 must have domain"), "{v}");
}

#[test]
fn other_codes_and_unknown_domains_rejected() {
    let v = check_exit_codes(
        &[
            ("0", "positive"),
            ("2", "error"),
            ("127", "error"),
            ("1", "wat"),
        ],
        &flags(&[]),
        false,
    )
    .join("\n");
    assert!(
        v.contains("\"127\" is outside the family dictionary"),
        "{v}"
    );
    assert!(v.contains("unknown domain \"wat\""), "{v}");
}

#[test]
fn expected_change_requires_exit_zero_on_change_flag() {
    let codes = [("0", "positive"), ("1", "expected_change"), ("2", "error")];
    let v = check_exit_codes(&codes, &flags(&[]), false).join("\n");
    assert!(v.contains("--exit-zero-on-change"), "{v}");
    assert!(check_exit_codes(&codes, &flags(&["--exit-zero-on-change"]), false).is_empty());
}

#[test]
fn zero_and_two_are_mandatory() {
    let v = check_exit_codes(&[("1", "negative")], &flags(&[]), false).join("\n");
    assert!(
        v.contains("missing \"0\"") && v.contains("missing \"2\""),
        "{v}"
    );
}

// The legacy helper now delegates to the same dictionary.

#[test]
fn trinity_helper_accepts_policy_band_manifest() {
    let manifest = r#"{
      "schema_version": "operator.v0", "name": "assess", "version": "0.1.0", "description": "d",
      "exit_codes": {
        "0": {"meaning": "PROCEED", "domain": "positive"},
        "1": {"meaning": "ESCALATE", "domain": "negative"},
        "2": {"meaning": "REFUSAL", "domain": "error"},
        "3": {"meaning": "BLOCK", "domain": "policy"}
      }
    }"#;
    assert_exit_codes_trinity(manifest, true);
}

#[test]
#[should_panic(expected = "outside the family dictionary")]
fn trinity_helper_still_rejects_exit_127() {
    let manifest = r#"{
      "schema_version": "operator.v0", "name": "x", "version": "0.1.0", "description": "d",
      "exit_codes": {
        "0": {"meaning": "OK", "domain": "positive"},
        "1": {"meaning": "NO", "domain": "negative"},
        "2": {"meaning": "ERR", "domain": "error"},
        "127": {"meaning": "?", "domain": "error"}
      }
    }"#;
    assert_exit_codes_trinity(manifest, true);
}

#[test]
#[should_panic(expected = "exit code 3 must have domain")]
fn trinity_helper_rejects_exit_three_not_marked_policy() {
    let manifest = r#"{
      "schema_version": "operator.v0", "name": "x", "version": "0.1.0", "description": "d",
      "exit_codes": {
        "0": {"meaning": "OK", "domain": "positive"},
        "1": {"meaning": "NO", "domain": "negative"},
        "2": {"meaning": "ERR", "domain": "error"},
        "3": {"meaning": "BLOCK", "domain": "negative"}
      }
    }"#;
    assert_exit_codes_trinity(manifest, true);
}

// ── empty query results ─────────────────────────────────────────────────

#[test]
fn empty_query_exit_zero_with_json_passes() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "echo '[]'");
    assert_empty_query_exits_zero(&bin, &["witness", "query", "--tool", "none"]);
}

#[test]
#[should_panic(expected = "must exit 0")]
fn empty_query_exit_one_is_caught() {
    // verify witness query today: zero matches -> exit 1.
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "echo '[]'\nexit 1");
    assert_empty_query_exits_zero(&bin, &["witness", "query"]);
}

#[test]
#[should_panic(expected = "stdout is not JSON")]
fn empty_query_exit_zero_without_json_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "echo 'no matches'");
    assert_empty_query_exits_zero(&bin, &["witness", "query"]);
}

// ── doctor interplay ────────────────────────────────────────────────────

#[test]
fn tool_with_doctor_must_declare_exit_one() {
    let codes = [("0", "positive"), ("2", "error")];
    assert!(check_exit_codes(&codes, &flags(&[]), false).is_empty());
    let v = check_exit_codes(&codes, &flags(&[]), true).join("\n");
    assert!(v.contains("must declare \"1\" (doctor unhealthy)"), "{v}");
}

#[test]
fn verdictless_tool_declares_one_as_unhealthy() {
    // vacuum: no verdict 1, but doctor can be unhealthy.
    let codes = [("0", "positive"), ("1", "unhealthy"), ("2", "error")];
    assert!(check_exit_codes(&codes, &flags(&[]), true).is_empty());
    // `unhealthy` is only valid on code 1.
    let v =
        check_exit_codes(&[("0", "positive"), ("2", "unhealthy")], &flags(&[]), true).join("\n");
    assert!(v.contains("exit code 2 must have domain"), "{v}");
}

#[test]
fn trinity_helper_requires_one_when_manifest_has_doctor() {
    let manifest = |codes: &str| {
        format!(
            r#"{{"schema_version":"operator.v0","name":"v","version":"0.1.0","description":"d",
            "subcommands":[{{"name":"doctor"}}],"exit_codes":{{{codes}}}}}"#
        )
    };
    let ok = manifest(
        r#""0":{"meaning":"OK","domain":"positive"},"1":{"meaning":"UNHEALTHY","domain":"unhealthy"},"2":{"meaning":"E","domain":"error"}"#,
    );
    assert_exit_codes_trinity(&ok, false);
    let bad = manifest(
        r#""0":{"meaning":"OK","domain":"positive"},"2":{"meaning":"E","domain":"error"}"#,
    );
    let r = std::panic::catch_unwind(|| assert_exit_codes_trinity(&bad, false));
    assert!(r.is_err(), "a tool with doctor and no exit 1 must fail");
}

// ── assess band table (docs/exit-codes.md) ──────────────────────────────

#[test]
fn assess_band_table_is_accepted() {
    // PROCEED 0, PROCEED_WITH_RISK 0, ESCALATE 1, BLOCK 3, REFUSAL/CLI 2.
    let manifest = r#"{
      "schema_version": "operator.v0", "name": "assess", "version": "0.1.0", "description": "d",
      "subcommands": [{"name": "doctor"}],
      "exit_codes": {
        "0": {"meaning": "PROCEED | PROCEED_WITH_RISK", "domain": "positive"},
        "1": {"meaning": "ESCALATE (doctor: unhealthy)", "domain": "negative"},
        "2": {"meaning": "REFUSAL | CLI error", "domain": "error"},
        "3": {"meaning": "BLOCK", "domain": "policy"}
      }
    }"#;
    assert_exit_codes_trinity(manifest, true);
}

#[test]
#[should_panic(expected = "must print an empty JSON array")]
fn empty_query_printing_empty_object_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "echo '{}'");
    assert_empty_query_exits_zero(&bin, &["witness", "query"]);
}

#[test]
#[should_panic(expected = "must print an empty JSON array")]
fn empty_query_printing_rows_is_caught() {
    let dir = tempfile::tempdir().unwrap();
    let bin = script(&dir, "echo '[{\"id\":1}]'");
    assert_empty_query_exits_zero(&bin, &["witness", "query"]);
}
