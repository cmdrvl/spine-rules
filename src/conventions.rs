//! Family CLI conventions: canonical flag dictionary, ANSI-free piped output,
//! `SOURCE_DATE_EPOCH` determinism, and non-hanging bare invocation.

use std::collections::BTreeSet;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// `(legacy spelling, canonical spelling)`. A legacy flag may exist only as a
/// declared alias of the canonical flag, never as an option's primary `flag`.
pub const LEGACY_FLAGS: &[(&str, &str)] =
    &[("--output", "--out"), ("--capsule-out", "--capsule-dir")];

/// Canonical flag dictionary: flags whose meaning is fixed across the family.
pub const CANONICAL_FLAGS: &[&str] = &[
    "--out",
    "--capsule-dir",
    "--json",
    "--explicit",
    "--no-witness",
];

/// Violations of the flag dictionary in an `operator.json` document
/// (`options[].flag`, optional `options[].aliases`). Empty means conformant.
pub fn lint_flags(operator_json: &str) -> Vec<String> {
    let doc: serde_json::Value = match serde_json::from_str(operator_json) {
        Ok(d) => d,
        Err(e) => return vec![format!("operator.json is not valid JSON: {e}")],
    };
    let options = doc
        .get("options")
        .and_then(|o| o.as_array())
        .cloned()
        .unwrap_or_default();

    let mut primaries = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    let mut v = Vec::new();
    for opt in &options {
        let Some(flag) = opt.get("flag").and_then(|f| f.as_str()) else {
            continue;
        };
        if !primaries.insert(flag.to_owned()) {
            v.push(format!("duplicate option flag {flag}"));
        }
        for a in opt
            .get("aliases")
            .and_then(|a| a.as_array())
            .into_iter()
            .flatten()
            .filter_map(|a| a.as_str())
        {
            aliases.insert((flag.to_owned(), a.to_owned()));
        }
    }
    for (legacy, canonical) in LEGACY_FLAGS {
        if primaries.contains(*legacy) {
            v.push(format!(
                "option uses legacy flag {legacy} as primary; use {canonical} and declare {legacy} in `aliases`"
            ));
        }
    }
    for (primary, alias) in &aliases {
        if let Some((_, canonical)) = LEGACY_FLAGS.iter().find(|(l, _)| l == alias)
            && primary != canonical
        {
            v.push(format!(
                "alias {alias} must belong to {canonical}, not {primary}"
            ));
        }
    }
    v
}

pub fn assert_canonical_flags(operator_json: &str) {
    let v = lint_flags(operator_json);
    assert!(
        v.is_empty(),
        "operator.json violates the family flag dictionary:\n  - {}",
        v.join("\n  - ")
    );
}

/// `true` when `bytes` contain an ANSI CSI escape (`ESC [`).
pub fn contains_ansi(bytes: &[u8]) -> bool {
    bytes.windows(2).any(|w| w == b"\x1b[")
}

/// Assert stdout and stderr carry no ANSI escapes when piped, both with
/// `NO_COLOR=1 TERM=dumb` and with neither variable set (non-TTY).
pub fn assert_no_ansi_when_piped(binary: &str, args: &[&str]) {
    for (label, no_color) in [
        ("NO_COLOR=1 TERM=dumb", true),
        ("non-TTY, no NO_COLOR", false),
    ] {
        let state = tempfile::tempdir().expect("create scratch HOME");
        let witness = state.path().join("witness.jsonl");
        let mut cmd = Command::new(binary);
        cmd.args(args)
            .stdin(Stdio::null())
            .env("HOME", state.path())
            .env("EPISTEMIC_WITNESS", &witness);
        if no_color {
            cmd.env("NO_COLOR", "1").env("TERM", "dumb");
        } else {
            cmd.env_remove("NO_COLOR").env_remove("TERM");
        }
        let out = cmd
            .output()
            .unwrap_or_else(|e| panic!("failed to run {binary} {args:?}: {e}"));
        assert!(
            !contains_ansi(&out.stdout) && !contains_ansi(&out.stderr),
            "`{binary} {}` emitted ANSI escapes with piped output ({label})",
            args.join(" ")
        );
    }
}

/// Assert two runs under the same `SOURCE_DATE_EPOCH` produce identical stdout
/// and exit code even when wall-clock time has moved on (so any embedded
/// timestamp must derive from the variable, not the clock).
pub fn assert_source_date_epoch_deterministic(binary: &str, args: &[&str]) {
    let state = tempfile::tempdir().expect("create scratch HOME");
    let witness = state.path().join("witness.jsonl");
    let run = || {
        Command::new(binary)
            .args(args)
            .env("SOURCE_DATE_EPOCH", "1700000000")
            .env("HOME", state.path())
            .env("EPISTEMIC_WITNESS", &witness)
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|e| panic!("failed to run {binary} {args:?}: {e}"))
    };
    let first = run();
    std::thread::sleep(Duration::from_millis(1100));
    let second = run();
    assert_eq!(
        first.status.code(),
        second.status.code(),
        "exit code changed between SOURCE_DATE_EPOCH runs"
    );
    assert_eq!(
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&second.stdout),
        "`{binary} {}` stdout differs between runs with the same SOURCE_DATE_EPOCH; timestamps must honor it",
        args.join(" ")
    );
}

/// Run `binary` with stdin=/dev/null and captured stdout/stderr; fail if it
/// is still running after `timeout`. Returns the exit code.
pub fn run_with_timeout(binary: &str, args: &[&str], timeout: Duration) -> Option<i32> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(tempfile::tempfile().expect("tempfile"))
        .stderr(tempfile::tempfile().expect("tempfile"))
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {binary}: {e}"));
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status.code();
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "`{binary} {}` still running after {timeout:?} with stdin=/dev/null; bare invocation must not hang",
                args.join(" ")
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What a child printed and how it ended.
#[derive(Debug, Clone)]
pub struct Captured {
    /// `None` when the child was ended by a signal.
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Like [`run_with_timeout`], but sets environment variables, returns what the child printed,
/// and reports a timeout as `None` (after killing the child) instead of panicking, so a caller
/// can record the hang and keep going.
pub fn run_captured_with_timeout(
    binary: &str,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
    timeout: Duration,
) -> Option<Captured> {
    use std::io::{Read, Seek, SeekFrom};

    let mut out = tempfile::tempfile().expect("tempfile");
    let mut err = tempfile::tempfile().expect("tempfile");
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(out.try_clone().expect("clone stdout file"))
        .stderr(err.try_clone().expect("clone stderr file"));
    for (name, value) in env {
        command.env(name, value);
    }
    // A script written moments ago can still be open for writing in another test thread's forked
    // child, which makes exec fail with ETXTBSY ("Text file busy"). That clears in milliseconds.
    let mut attempts = 0;
    let mut child = loop {
        match command.spawn() {
            Ok(child) => break child,
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 50 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("failed to spawn {binary}: {e}"),
        }
    };
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let read_back = |file: &mut std::fs::File| {
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(0)).expect("seek");
        file.read_to_end(&mut bytes).expect("read captured output");
        bytes
    };
    Some(Captured {
        code: status.code(),
        stdout: read_back(&mut out),
        stderr: read_back(&mut err),
    })
}

/// Bare invocation (no args, stdin=/dev/null, stdout piped) must terminate
/// on its own, by exit code rather than by signal.
pub fn assert_bare_invocation_does_not_hang(binary: &str) {
    let code = run_with_timeout(binary, &[], Duration::from_secs(10));
    assert!(
        code.is_some(),
        "bare `{binary}` was terminated by a signal instead of exiting"
    );
}
