//! Family exit-code dictionary. See `docs/exit-codes.md` for the rationale.
//!
//! | code | domain            | meaning                                                    |
//! |------|-------------------|------------------------------------------------------------|
//! | 0    | `positive`        | success / clean verdict                                    |
//! | 1    | `negative`        | domain verdict that should halt a pipeline                 |
//! | 1    | `expected_change` | normal, expected outcome (e.g. `rvl` `REAL_CHANGE`); the tool must offer `--exit-zero-on-change` |
//! | 1    | `unhealthy`       | the tool has no verdict `1`; `1` exists only for `doctor` unhealthy |
//! | 2    | `error`           | refusal or CLI/usage error; never a verdict, never doctor-unhealthy |
//! | 3    | `policy`          | valid policy verdict (e.g. `assess` `BLOCK`) distinct from refusal |
//!
//! Every tool with a `doctor` command declares `1`: doctor-unhealthy shares the
//! code with the verdict `1` (exempt from the verdict domain) or, when the tool
//! has no verdict `1`, declares it with domain `unhealthy`.
//! `--exit-zero-on-change` affects the verdict only, never `doctor`.
//! Read/query subcommands never exit `1` for an empty result set.

use std::collections::BTreeSet;
use std::process::Command;

pub const DOMAINS: &[&str] = &[
    "positive",
    "negative",
    "expected_change",
    "unhealthy",
    "policy",
    "error",
];

/// Opt-in flag that maps an `expected_change` outcome to exit 0.
pub const EXIT_ZERO_ON_CHANGE_FLAG: &str = "--exit-zero-on-change";

/// Violations of the exit-code dictionary. `codes` are `(code, domain)` pairs
/// from `exit_codes`; `option_flags` are the tool's declared `options[].flag`;
/// `has_doctor` is whether the tool declares a `doctor` command.
pub fn check_exit_codes(
    codes: &[(&str, &str)],
    option_flags: &BTreeSet<String>,
    has_doctor: bool,
) -> Vec<String> {
    let mut v = Vec::new();
    let has = |c: &str| codes.iter().any(|(k, _)| *k == c);
    for required in ["0", "2"] {
        if !has(required) {
            v.push(format!("exit_codes is missing {required:?}"));
        }
    }
    if has_doctor && !has("1") {
        v.push(
            "tool declares `doctor`, so exit_codes must declare \"1\" (doctor unhealthy); \
             use domain negative/expected_change for a verdict 1, or `unhealthy` when there is none"
                .into(),
        );
    }
    for (code, domain) in codes {
        if !DOMAINS.contains(domain) {
            v.push(format!(
                "exit code {code} has unknown domain {domain:?}; expected one of {DOMAINS:?}"
            ));
            continue;
        }
        let allowed: &[&str] = match *code {
            "0" => &["positive"],
            "1" => &["negative", "expected_change", "unhealthy"],
            "2" => &["error"],
            "3" => &["policy"],
            other => {
                v.push(format!(
                    "exit code {other:?} is outside the family dictionary (0, 1, 2, 3); \
                     3 is reserved for policy verdicts"
                ));
                continue;
            }
        };
        if !allowed.contains(domain) {
            v.push(format!(
                "exit code {code} must have domain {allowed:?}, found {domain:?}"
            ));
        }
        if *domain == "expected_change" && !option_flags.contains(EXIT_ZERO_ON_CHANGE_FLAG) {
            v.push(format!(
                "exit code {code} is expected_change, so the tool must declare option {EXIT_ZERO_ON_CHANGE_FLAG} for `set -e` pipelines"
            ));
        }
    }
    v
}

/// Run a read/query invocation whose fixture yields no rows and assert it
/// exits 0 and prints exactly the empty JSON array `[]` (an empty result is
/// not a failure).
pub fn assert_empty_query_exits_zero(binary: &str, args: &[&str]) {
    let out = Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {binary} {args:?}: {e}"));
    assert_eq!(
        out.status.code(),
        Some(0),
        "`{binary} {}` returned exit {:?} for an empty result set; read/query commands must exit 0 (stderr: {})",
        args.join(" "),
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let value = serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "`{binary} {}` exit 0 but stdout is not JSON: {e}",
            args.join(" ")
        )
    });
    assert_eq!(
        value,
        serde_json::json!([]),
        "`{binary} {}` must print an empty JSON array `[]` for an empty result set",
        args.join(" ")
    );
}
