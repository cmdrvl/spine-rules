//! Planted negatives for `assert_no_raw_cell_leak`: it must catch a leak on either
//! stream, pass a clean tool, and refuse to pass vacuously. The "tool" is `/bin/sh`
//! printing a fixed canary, so each case needs no real spine tool.

use spine_rules::redaction::{
    assert_canaries_visible_with_explicit, assert_explicit_declared, assert_no_raw_cell_leak,
    explicit_flag_declared, find_leaks,
};

const SH: &str = "/bin/sh";
const CANARY: &str = "CANARY-LOAN-0f3a91";

#[test]
fn find_leaks_reports_the_stream_that_leaked() {
    assert_eq!(
        find_leaks(CANARY.as_bytes(), b"", &[CANARY]),
        vec![format!("stdout: {CANARY}")]
    );
    assert_eq!(
        find_leaks(b"", format!("error near {CANARY}").as_bytes(), &[CANARY]),
        vec![format!("stderr: {CANARY}")]
    );
    assert_eq!(
        find_leaks(CANARY.as_bytes(), CANARY.as_bytes(), &[CANARY]).len(),
        2
    );
    assert!(find_leaks(b"redacted:sha256:abcd", b"all clear", &[CANARY]).is_empty());
}

#[test]
fn a_clean_tool_passes() {
    assert_no_raw_cell_leak(SH, &["-c", "echo 'redacted:sha256:0123'"], &[CANARY]);
}

#[test]
#[should_panic(expected = "leaked raw cell values")]
fn a_stdout_leak_is_caught() {
    assert_no_raw_cell_leak(SH, &["-c", &format!("echo {CANARY}")], &[CANARY]);
}

#[test]
#[should_panic(expected = "leaked raw cell values")]
fn a_stderr_leak_is_caught() {
    assert_no_raw_cell_leak(
        SH,
        &[
            "-c",
            &format!("echo refused >&1; echo {CANARY} >&2; exit 2"),
        ],
        &[CANARY],
    );
}

#[test]
#[should_panic(expected = "printed nothing")]
fn a_silent_tool_cannot_pass_vacuously() {
    assert_no_raw_cell_leak(SH, &["-c", "exit 0"], &[CANARY]);
}

#[test]
#[should_panic(expected = "WITHOUT --explicit")]
fn the_leak_check_refuses_to_run_with_explicit() {
    assert_no_raw_cell_leak(SH, &["-c", "echo x", "--explicit"], &[CANARY]);
}

#[test]
#[should_panic(expected = "shorter than")]
fn a_short_canary_is_rejected() {
    assert_no_raw_cell_leak(SH, &["-c", "echo ab"], &["ab"]);
}

#[test]
fn the_explicit_control_needs_the_canary_to_show() {
    // The shell sees `--explicit` as $0 and ignores it; the canary prints regardless.
    assert_canaries_visible_with_explicit(SH, &["-c", &format!("echo {CANARY}")], &[CANARY]);
}

#[test]
#[should_panic(expected = "even with --explicit")]
fn the_explicit_control_fails_when_the_fixture_never_shows_the_canary() {
    assert_canaries_visible_with_explicit(SH, &["-c", "echo nothing here"], &[CANARY]);
}

#[test]
fn explicit_declaration_is_read_from_operator_options() {
    let declared = r#"{"options":[{"name":"explicit","flag":"--explicit"}]}"#;
    let missing = r#"{"options":[{"name":"json","flag":"--json"}]}"#;
    assert!(explicit_flag_declared(declared));
    assert!(!explicit_flag_declared(missing));
    assert!(!explicit_flag_declared("not json"));
    assert_explicit_declared(declared);
}

#[test]
#[should_panic(expected = "must declare")]
fn a_tool_that_opts_in_without_declaring_explicit_fails() {
    assert_explicit_declared(r#"{"options":[{"name":"json","flag":"--json"}]}"#);
}
