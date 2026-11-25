//! Zero-retention conformance: a tool run WITHOUT `--explicit` must not print raw cell values.
//!
//! The check is opt-in per tool (the `fixture_canary_args` / `fixture_canaries` fields of
//! `golden_rules_suite!`), because only tools that read tabular cell data have anything to
//! redact. A tool that opts in supplies a fixture whose cells hold unmistakable canary
//! strings and arguments that make the tool fail, refuse or report on that fixture.
//!
//! What it proves: none of the canary strings appear in stdout or stderr of that one
//! invocation. What it does not prove: that every other command, flag combination or
//! failure path is clean; that values are not leaked in a transformed form (a hash of a
//! low-entropy value is guessable); or that column names are safe (they are not canaries
//! unless the tool opts them in as canaries too).

use std::process::{Command, Stdio};

/// The `--explicit` flag a tool must declare in `operator.v1` `options[]` when it redacts.
pub const EXPLICIT_FLAG: &str = "--explicit";

/// Shortest canary accepted. Shorter strings collide with ordinary output.
pub const MIN_CANARY_LEN: usize = 8;

/// Which canaries appear in which stream. Each entry is `"<stream>: <canary>"`.
/// Pure function: no process is run, so it is what the unit tests exercise directly.
pub fn find_leaks(stdout: &[u8], stderr: &[u8], canaries: &[&str]) -> Vec<String> {
    let mut leaks = Vec::new();
    for (stream, bytes) in [("stdout", stdout), ("stderr", stderr)] {
        for canary in canaries {
            if contains(bytes, canary.as_bytes()) {
                leaks.push(format!("{stream}: {canary}"));
            }
        }
    }
    leaks
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn validate_canaries(canaries: &[&str]) {
    assert!(
        !canaries.is_empty(),
        "assert_no_raw_cell_leak needs at least one canary"
    );
    for canary in canaries {
        assert!(
            canary.len() >= MIN_CANARY_LEN,
            "canary {canary:?} is shorter than {MIN_CANARY_LEN} bytes and would collide with ordinary output"
        );
    }
}

fn run(binary: &str, args: &[&str]) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let output = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|error| panic!("cannot run {binary}: {error}"));
    (output.status.code(), output.stdout, output.stderr)
}

/// Run `binary args` (which must NOT contain `--explicit`) and assert that no canary
/// appears in stdout or stderr. Also asserts the run printed something, so a tool that
/// crashes silently or prints nothing cannot pass vacuously.
pub fn assert_no_raw_cell_leak(binary: &str, args: &[&str], canaries: &[&str]) {
    assert!(
        !args.contains(&EXPLICIT_FLAG),
        "assert_no_raw_cell_leak must run WITHOUT {EXPLICIT_FLAG}; got args {args:?}"
    );
    validate_canaries(canaries);
    let (code, stdout, stderr) = run(binary, args);
    assert!(
        !stdout.is_empty() || !stderr.is_empty(),
        "{binary} {args:?} printed nothing (exit {code:?}); the fixture proves nothing"
    );
    let leaks = find_leaks(&stdout, &stderr, canaries);
    assert!(
        leaks.is_empty(),
        "{binary} {args:?} leaked raw cell values without {EXPLICIT_FLAG} (exit {code:?}):\n  - {}",
        leaks.join("\n  - ")
    );
}

/// Control for [`assert_no_raw_cell_leak`]: with `--explicit` added, at least one canary
/// must appear. Without this, a fixture whose canaries the tool never prints would make
/// the leak check pass for the wrong reason.
pub fn assert_canaries_visible_with_explicit(binary: &str, args: &[&str], canaries: &[&str]) {
    validate_canaries(canaries);
    let mut explicit: Vec<&str> = args.to_vec();
    explicit.push(EXPLICIT_FLAG);
    let (code, stdout, stderr) = run(binary, &explicit);
    assert!(
        !find_leaks(&stdout, &stderr, canaries).is_empty(),
        "{binary} {explicit:?} (exit {code:?}) printed none of the canaries even with {EXPLICIT_FLAG}; the fixture cannot detect a leak"
    );
}

/// `true` when `operator_json` declares an option whose `flag` is `--explicit`.
pub fn explicit_flag_declared(operator_json: &str) -> bool {
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(operator_json) else {
        return false;
    };
    doc.get("options")
        .and_then(|options| options.as_array())
        .is_some_and(|options| {
            options.iter().any(|option| {
                option.get("flag").and_then(|flag| flag.as_str()) == Some(EXPLICIT_FLAG)
            })
        })
}

/// A tool that opts into the leak check must declare `--explicit` so `veil` can ban it
/// uniformly under protected profiles.
pub fn assert_explicit_declared(operator_json: &str) {
    assert!(
        explicit_flag_declared(operator_json),
        "operator.json must declare an `options[]` entry with flag {EXPLICIT_FLAG} (veil bans it by name)"
    );
}
