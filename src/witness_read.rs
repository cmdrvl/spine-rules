//! Witness-read conformance (bd-kf5): what `witness query`, `witness count` and `witness last` do
//! with ledgers that hold nothing readable, and with a ledger that cannot be read at all.
//!
//! Family ADR (`docs/exit-codes.md`):
//!
//! - an absent, empty or only-invalid-UTF-8 ledger is a successful empty read: exit `0` and
//!   `[]` (query), `{"count":0}` (count), `null` (last);
//! - an unreadable ledger (a directory, a mode-000 file) is a refusal: exit `2`, empty stdout,
//!   and a stderr line that names the path;
//! - nothing may hang: every run is a child with a hard timeout, and a hang is a violation.
//!
//! The checks run the binary with `EPISTEMIC_WITNESS=<path>`. Opt in through the
//! `fixture_witness_*_args` keys of [`golden_rules_suite!`](crate::golden_rules_suite); a tool that
//! omits them gets no new tests. This does not exercise a ledger that holds valid records (the
//! record format is tool specific), human-mode output, or Windows (the mode-000 case is Unix only).

use crate::conventions::{Captured, run_captured_with_timeout};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long any one child may run before it is killed and reported as a hang.
pub const WITNESS_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// The argument lists of a tool's three witness-read commands (they differ per tool).
#[derive(Debug, Clone, Copy)]
pub struct WitnessReadArgs<'a> {
    pub query: &'a [&'a str],
    pub count: &'a [&'a str],
    pub last: &'a [&'a str],
}

#[derive(Clone, Copy)]
enum Command {
    Query,
    Count,
    Last,
}

impl Command {
    const ALL: [Command; 3] = [Command::Query, Command::Count, Command::Last];

    fn name(self) -> &'static str {
        match self {
            Command::Query => "query",
            Command::Count => "count",
            Command::Last => "last",
        }
    }

    fn empty_value(self) -> serde_json::Value {
        match self {
            Command::Query => serde_json::json!([]),
            Command::Count => serde_json::json!({"count": 0}),
            Command::Last => serde_json::Value::Null,
        }
    }
}

/// Violations of the witness-read contract. Empty means conformant.
pub fn check_witness_read(binary: &str, args: WitnessReadArgs<'_>) -> Vec<String> {
    let mut violations = Vec::new();
    let dir = tempfile::tempdir().expect("tempdir");

    let absent = dir.path().join("absent.jsonl");
    let empty = dir.path().join("empty.jsonl");
    std::fs::write(&empty, b"").expect("write empty ledger");
    let non_utf8 = dir.path().join("non-utf8.jsonl");
    std::fs::write(&non_utf8, [0xff, 0xfe, b'\n']).expect("write non-utf8 ledger");
    let directory = dir.path().join("a_directory");
    std::fs::create_dir(&directory).expect("create directory ledger");

    for (label, path) in [
        ("absent", &absent),
        ("empty", &empty),
        ("invalid-utf8 line", &non_utf8),
    ] {
        for command in Command::ALL {
            if check_empty_read(binary, args, command, label, path, &mut violations) {
                break; // a hang is reported once per ledger; do not wait out the rest
            }
        }
    }

    let mode_000 = mode_000_file(dir.path());
    let mut unreadable: Vec<(&str, &Path)> = vec![("directory", directory.as_path())];
    if let Some(path) = &mode_000 {
        unreadable.push(("mode-000 file", path.as_path()));
    }
    for (label, path) in &unreadable {
        for command in Command::ALL {
            if check_unreadable(binary, args, command, label, path, &mut violations) {
                break;
            }
        }
    }

    #[cfg(unix)]
    if let Some(path) = &mode_000 {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    violations
}

/// Panics with every violation, for use inside a generated test.
pub fn assert_witness_read_conformance(binary: &str, args: WitnessReadArgs<'_>) {
    let violations = check_witness_read(binary, args);
    assert!(
        violations.is_empty(),
        "witness-read conformance violations for `{binary}`:\n  {}",
        violations.join("\n  ")
    );
}

fn args_for<'a>(args: WitnessReadArgs<'a>, command: Command) -> &'a [&'a str] {
    match command {
        Command::Query => args.query,
        Command::Count => args.count,
        Command::Last => args.last,
    }
}

fn run(
    binary: &str,
    args: WitnessReadArgs<'_>,
    command: Command,
    ledger: &Path,
) -> Option<Captured> {
    run_captured_with_timeout(
        binary,
        args_for(args, command),
        &[("EPISTEMIC_WITNESS", ledger.as_os_str())],
        WITNESS_READ_TIMEOUT,
    )
}

fn check_empty_read(
    binary: &str,
    args: WitnessReadArgs<'_>,
    command: Command,
    label: &str,
    ledger: &Path,
    violations: &mut Vec<String>,
) -> bool {
    let what = format!("witness {} on {label} ledger", command.name());
    let Some(out) = run(binary, args, command, ledger) else {
        violations.push(format!(
            "{what}: hung (no exit within {WITNESS_READ_TIMEOUT:?})"
        ));
        return true;
    };
    if out.code != Some(0) {
        violations.push(format!(
            "{what}: exit {:?}, want 0 (an empty read is success); stderr: {}",
            out.code,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
        return false;
    }
    match serde_json::from_slice::<serde_json::Value>(&out.stdout) {
        Ok(value) if value == command.empty_value() => {}
        Ok(value) => violations.push(format!(
            "{what}: stdout is {value}, want {}",
            command.empty_value()
        )),
        Err(_) => violations.push(format!(
            "{what}: exit 0 but stdout is not JSON ({:?}); want {}",
            String::from_utf8_lossy(&out.stdout).trim(),
            command.empty_value()
        )),
    }
    false
}

fn check_unreadable(
    binary: &str,
    args: WitnessReadArgs<'_>,
    command: Command,
    label: &str,
    ledger: &Path,
    violations: &mut Vec<String>,
) -> bool {
    let what = format!("witness {} on {label} ledger", command.name());
    let Some(out) = run(binary, args, command, ledger) else {
        violations.push(format!(
            "{what}: hung (no exit within {WITNESS_READ_TIMEOUT:?})"
        ));
        return true;
    };
    if out.code != Some(2) {
        violations.push(format!(
            "{what}: exit {:?}, want 2 (an unreadable ledger is a refusal, not an empty read)",
            out.code
        ));
        return false;
    }
    if !out.stdout.is_empty() {
        violations.push(format!("{what}: stdout must be empty on a refusal"));
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !stderr.contains(&ledger.display().to_string()) {
        violations.push(format!(
            "{what}: stderr must name the path {}; got {:?}",
            ledger.display(),
            stderr.trim()
        ));
    }
    false
}

/// A file nobody can read, or `None` where mode 000 is not enforced (non-Unix, or a process
/// that can read it anyway, such as root).
fn mode_000_file(dir: &Path) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("mode-000.jsonl");
        std::fs::write(&path, b"{}\n").expect("write mode-000 ledger");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::File::open(&path).is_ok() {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            return None;
        }
        Some(path)
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        None
    }
}
