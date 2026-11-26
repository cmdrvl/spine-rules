//! Witness-read conformance (bd-kf5), proven against shell-script fake tools: a conforming fake
//! passes and each known defect class is caught with a message that names it.

use spine_rules::witness_read::{InvalidLine, WitnessReadArgs, check_witness_read};
use std::os::unix::fs::PermissionsExt;

fn args(invalid_line: InvalidLine) -> WitnessReadArgs<'static> {
    WitnessReadArgs {
        query: &["witness", "query", "--json"],
        count: &["witness", "count", "--json"],
        last: &["witness", "last", "--json"],
        invalid_line,
    }
}

fn fake(dir: &tempfile::TempDir, body: &str) -> String {
    let path = dir.path().join("tool");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_owned()
}

/// Shell prelude shared by the fakes: `$2` is the action (`query|count|last`), `$EPISTEMIC_WITNESS`
/// the ledger. `empty_out` prints the empty value for the action; `read_check` decides what an
/// unreadable ledger does.
fn tool(read_check: &str, empty_out: &str) -> String {
    format!(
        r#"
action="$2"
ledger="$EPISTEMIC_WITNESS"
emit_empty() {{
{empty_out}
}}
if [ -d "$ledger" ] || {{ [ -e "$ledger" ] && [ ! -r "$ledger" ]; }}; then
{read_check}
fi
emit_empty
exit 0
"#
    )
}

const GOOD_EMPTY: &str = r#"case "$action" in
  query) echo '[]' ;;
  count) echo '{"count":0}' ;;
  last) echo 'null' ;;
esac"#;

const GOOD_REFUSE: &str = r#"  echo "tool: witness: failed to read ledger $ledger: unreadable" >&2
  exit 2"#;

/// A tool that declares `Skip` (the fakes above skip an invalid-UTF-8 ledger as empty).
fn violations(body: &str) -> String {
    violations_declaring(body, InvalidLine::Skip)
}

fn violations_declaring(body: &str, invalid_line: InvalidLine) -> String {
    let dir = tempfile::tempdir().unwrap();
    let binary = fake(&dir, body);
    check_witness_read(&binary, args(invalid_line)).join("\n")
}

/// Like [`tool`], but an invalid-UTF-8 ledger is REFUSED (exit 2, naming the ledger) the way
/// shape/vacuum/profile do; `named` is what the refusal prints for the ledger.
fn tool_refusing_invalid(named: &str) -> String {
    let base = tool(GOOD_REFUSE, GOOD_EMPTY);
    base.replacen(
        "emit_empty\nexit 0\n",
        &format!(
            "case \"$ledger\" in *non-utf8*) echo \"tool: witness: not valid UTF-8: {named}\" >&2; exit 2 ;; esac\nemit_empty\nexit 0\n"
        ),
        1,
    )
}

#[test]
fn conforming_fake_passes() {
    let v = violations(&tool(GOOD_REFUSE, GOOD_EMPTY));
    assert!(v.is_empty(), "a conforming tool must pass:\n{v}");
}

#[test]
fn exit_1_on_empty_last_is_caught() {
    // rvl 0.8.0: `witness last` on an empty ledger exited 1 with no output.
    let empty = r#"case "$action" in
  query) echo '[]' ;;
  count) echo '{"count":0}' ;;
  last) exit 1 ;;
esac"#;
    let v = violations(&tool(GOOD_REFUSE, empty));
    assert!(
        v.contains("witness last on absent ledger: exit Some(1), want 0"),
        "{v}"
    );
    assert!(v.contains("witness last on empty ledger"), "{v}");
    assert!(!v.contains("witness query"), "only last is broken:\n{v}");
}

#[test]
fn exit_0_with_no_output_on_empty_last_is_caught() {
    let empty = r#"case "$action" in
  query) echo '[]' ;;
  count) echo '{"count":0}' ;;
  last) : ;;
esac"#;
    let v = violations(&tool(GOOD_REFUSE, empty));
    assert!(
        v.contains("witness last on absent ledger: exit 0 but stdout is not JSON"),
        "{v}"
    );
}

#[test]
fn wrong_empty_value_is_caught() {
    let empty = r#"case "$action" in
  query) echo '[]' ;;
  count) echo '0' ;;
  last) echo '{}' ;;
esac"#;
    let v = violations(&tool(GOOD_REFUSE, empty));
    assert!(
        v.contains("witness count on empty ledger: stdout is 0, want {\"count\":0}"),
        "{v}"
    );
    assert!(
        v.contains("witness last on empty ledger: stdout is {}, want null"),
        "{v}"
    );
}

#[test]
fn directory_read_as_empty_is_caught() {
    // pack 0.7.0 / rvl 0.8.0: the read error was swallowed, so a directory looked empty.
    let v = violations(&tool("  emit_empty\n  exit 0", GOOD_EMPTY));
    assert!(
        v.contains("witness query on directory ledger: exit Some(0), want 2"),
        "{v}"
    );
    assert!(v.contains("witness count on directory ledger"), "{v}");
    assert!(v.contains("witness last on directory ledger"), "{v}");
}

#[test]
fn hang_on_a_directory_is_caught_and_killed() {
    // verify 0.6.0 / assess 0.4.0 spun forever on a directory ledger.
    let started = std::time::Instant::now();
    let v = violations(&tool("  sleep 30", GOOD_EMPTY));
    assert!(v.contains("witness query on directory ledger: hung"), "{v}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(25),
        "hung children must be killed at the timeout, not waited for"
    );
}

#[test]
fn refusal_that_does_not_name_the_path_is_caught() {
    let silent = "  echo 'tool: cannot read the ledger' >&2\n  exit 2";
    let v = violations(&tool(silent, GOOD_EMPTY));
    assert!(
        v.contains("witness last on directory ledger: stderr must name the ledger (a_directory)"),
        "{v}"
    );
}

#[test]
fn refusal_that_prints_stdout_is_caught() {
    let noisy = "  echo '[]'\n  echo \"failed to read ledger $ledger\" >&2\n  exit 2";
    let v = violations(&tool(noisy, GOOD_EMPTY));
    assert!(
        v.contains("witness query on directory ledger: stdout must be empty on a refusal"),
        "{v}"
    );
}

#[test]
fn mode_000_file_is_exercised_when_the_process_cannot_read_it() {
    // The fake treats a mode-000 file as readable-and-empty (exit 0): caught only where mode 000
    // is enforced, so assert on the case label only when this process is not allowed to read it.
    let probe = tempfile::tempdir().unwrap();
    let path = probe.path().join("probe");
    std::fs::write(&path, b"x").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let enforced = std::fs::File::open(&path).is_err();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

    let body = format!(
        r#"
action="$2"
ledger="$EPISTEMIC_WITNESS"
emit_empty() {{
{GOOD_EMPTY}
}}
if [ -d "$ledger" ]; then
  echo "failed to read ledger $ledger" >&2
  exit 2
fi
emit_empty
exit 0
"#
    );
    let v = violations(&body);
    if enforced {
        assert!(
            v.contains("witness last on mode-000 file ledger: exit Some(0), want 2"),
            "{v}"
        );
    } else {
        assert!(
            !v.contains("mode-000"),
            "case must be skipped when mode 000 is not enforced:\n{v}"
        );
    }
}

#[test]
fn declared_skip_but_refusing_is_caught() {
    // shape/vacuum/profile behaviour, declared as Skip: must be flagged on every command.
    let v = violations_declaring(&tool_refusing_invalid("$ledger"), InvalidLine::Skip);
    for command in ["query", "count", "last"] {
        assert!(
            v.contains(&format!(
                "witness {command} on invalid-utf8 line (declared Skip) ledger: exit Some(2), want 0"
            )),
            "{v}"
        );
    }
}

#[test]
fn declared_refuse_but_skipping_is_caught() {
    // verify/assess/pack/rvl behaviour, declared as Refuse: must be flagged on every command.
    let v = violations_declaring(&tool(GOOD_REFUSE, GOOD_EMPTY), InvalidLine::Refuse);
    for command in ["query", "count", "last"] {
        assert!(
            v.contains(&format!(
                "witness {command} on invalid-utf8 line (declared Refuse) ledger: exit Some(0), want 2 (a tool that declares Refuse must refuse)"
            )),
            "{v}"
        );
    }
}

#[test]
fn declared_refuse_and_refusing_passes_and_declared_skip_and_skipping_passes() {
    let refusing = tool_refusing_invalid("$ledger");
    let v = violations_declaring(&refusing, InvalidLine::Refuse);
    assert!(v.is_empty(), "Refuse declared and honoured must pass:\n{v}");
    let v = violations_declaring(&tool(GOOD_REFUSE, GOOD_EMPTY), InvalidLine::Skip);
    assert!(v.is_empty(), "Skip declared and honoured must pass:\n{v}");
}

#[test]
fn a_refusal_naming_only_the_file_name_passes_but_one_naming_nothing_fails() {
    // shape's message names the ledger without its directory. A file-name-only message passes...
    let by_name =
        "  echo \"tool: witness: cannot read ledger $(basename \"$ledger\")\" >&2\n  exit 2";
    let v = violations(&tool(by_name, GOOD_EMPTY));
    assert!(v.is_empty(), "file-name-only refusal must pass:\n{v}");
    // ...and a generic message that names no ledger fails.
    let generic = "  echo 'tool: witness: ledger unreadable' >&2\n  exit 2";
    let v = violations(&tool(generic, GOOD_EMPTY));
    assert!(
        v.contains("stderr must name the ledger (a_directory)"),
        "{v}"
    );
}

#[test]
fn a_refusing_invalid_line_that_names_nothing_is_caught() {
    let v = violations_declaring(&tool_refusing_invalid("a ledger"), InvalidLine::Refuse);
    assert!(
        v.contains("stderr must name the ledger (non-utf8.jsonl)"),
        "{v}"
    );
}
