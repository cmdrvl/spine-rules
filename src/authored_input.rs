//! Authored-input self-discovery (bd-dbe).
//!
//! A command that accepts an operator-authored file declares that file in its
//! `operator.v1` command entry. The generated check then runs the command's
//! `--describe`, input-schema flag, and help with no input file, and checks the
//! robot guide for a real copy-paste example and its allowed values.

use crate::contract::{AuthoredInput, OperatorContract};
use crate::conventions::{Captured, run_captured_with_timeout};
use std::ffi::OsStr;
use std::time::Duration;

/// Hard limit for each metadata/help child process.
pub const SELF_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Check authored-input self-discovery. Empty means conformant.
pub fn check_self_discovery(
    binary: &str,
    operator_json: &str,
    robot_guide_args: &[&str],
) -> Vec<String> {
    let contract = match OperatorContract::parse(operator_json) {
        Ok(contract) => contract,
        Err(error) => return vec![format!("operator.json is not valid operator.v1: {error}")],
    };
    let mut violations = Vec::new();
    violations.extend(
        contract
            .validate()
            .into_iter()
            .map(|error| format!("operator contract: {error}")),
    );
    let mut authored_commands = 0;

    for command in &contract.commands {
        if command.authored_inputs.is_empty() {
            continue;
        }
        authored_commands += 1;
        for input in &command.authored_inputs {
            check_input(binary, command.name.as_str(), input, &mut violations);
        }
    }

    if authored_commands == 0 {
        violations.push(
            "operator.json declares no authored input; opt-in self-discovery must name at least one"
                .into(),
        );
        return violations;
    }

    let guide = run_metadata(binary, robot_guide_args);
    match guide {
        None => violations.push(format!(
            "`{binary} {}` hung after {:?}",
            robot_guide_args.join(" "),
            SELF_DISCOVERY_TIMEOUT
        )),
        Some(output) => {
            if output.code != Some(0) {
                violations.push(format!(
                    "`{binary} {}` exited {:?}; robot-docs guide must be standalone (stderr: {})",
                    robot_guide_args.join(" "),
                    output.code,
                    display(&output.stderr)
                ));
            }
            let guide_text = combined_text(&output);
            for command in &contract.commands {
                for input in &command.authored_inputs {
                    check_documented_text(
                        &mut violations,
                        &format!(
                            "robot-docs guide for command {:?} input {:?}",
                            command.name, input.name
                        ),
                        input,
                        &guide_text,
                    );
                }
            }
        }
    }

    violations
}

/// Assert authored-input self-discovery, for use from `golden_rules_suite!`.
pub fn assert_self_discovery(binary: &str, operator_json: &str, robot_guide_args: &[&str]) {
    let violations = check_self_discovery(binary, operator_json, robot_guide_args);
    assert!(
        violations.is_empty(),
        "authored-input self-discovery violations for `{binary}`:\n  {}",
        violations.join("\n  ")
    );
}

fn check_input(
    binary: &str,
    command_name: &str,
    input: &AuthoredInput,
    violations: &mut Vec<String>,
) {
    if input.name.trim().is_empty() {
        violations.push(format!(
            "command {command_name:?} has an authored input with an empty name"
        ));
    }
    if !matches!(input.schema_flag.as_str(), "--input-schema" | "--schema") {
        violations.push(format!(
            "command {command_name:?} input {:?} has unsupported schema flag {:?}",
            input.name, input.schema_flag
        ));
    }
    if input.example.trim().is_empty() {
        violations.push(format!(
            "command {command_name:?} input {:?} has an empty example; an empty example must not satisfy the contract",
            input.name
        ));
    }

    let route = route_args(command_name);
    let mut schema_args = route.clone();
    schema_args.push(input.schema_flag.as_str());
    check_schema(binary, &schema_args, command_name, &input.name, violations);

    let mut describe_args = route.clone();
    describe_args.push("--describe");
    check_describe(binary, &describe_args, command_name, violations);

    let mut help_args = route;
    help_args.push("--help");
    let label = format!("command {command_name:?} input {:?} help", input.name);
    match run_metadata(binary, &help_args) {
        None => violations.push(format!(
            "`{binary} {}` hung after {:?}",
            help_args.join(" "),
            SELF_DISCOVERY_TIMEOUT
        )),
        Some(output) if output.code != Some(0) => violations.push(format!(
            "`{binary} {}` exited {:?}; authored-input help must be standalone",
            help_args.join(" "),
            output.code
        )),
        Some(output) => check_documented_text(violations, &label, input, &combined_text(&output)),
    }
}

fn check_schema(
    binary: &str,
    args: &[&str],
    command_name: &str,
    input_name: &str,
    violations: &mut Vec<String>,
) {
    let label = format!("command {command_name:?} input {input_name:?} schema");
    let Some(output) = run_metadata(binary, args) else {
        violations.push(format!(
            "`{binary} {}` hung after {:?}",
            args.join(" "),
            SELF_DISCOVERY_TIMEOUT
        ));
        return;
    };
    if output.code != Some(0) {
        violations.push(format!(
            "`{binary} {}` exited {:?}; authored-input schema must require no file or bind/lock arguments (stderr: {})",
            args.join(" "), output.code, display(&output.stderr)
        ));
        return;
    }
    let value: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(value) => value,
        Err(error) => {
            violations.push(format!("{label} did not emit JSON Schema: {error}"));
            return;
        }
    };
    if !value.is_object()
        || (value.get("$schema").is_none()
            && value.get("type").is_none()
            && value.get("properties").is_none()
            && value.get("$defs").is_none())
    {
        violations.push(format!("{label} did not emit a JSON Schema document"));
    }
}

fn check_describe(binary: &str, args: &[&str], command_name: &str, violations: &mut Vec<String>) {
    let label = format!("command {command_name:?} --describe");
    let Some(output) = run_metadata(binary, args) else {
        violations.push(format!(
            "`{binary} {}` hung after {:?}",
            args.join(" "),
            SELF_DISCOVERY_TIMEOUT
        ));
        return;
    };
    if output.code != Some(0) {
        violations.push(format!(
            "`{binary} {}` exited {:?}; --describe must be standalone (stderr: {})",
            args.join(" "),
            output.code,
            display(&output.stderr)
        ));
        return;
    }
    let value: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(value) => value,
        Err(error) => {
            violations.push(format!("{label} did not emit JSON: {error}"));
            return;
        }
    };
    if !value.is_object() {
        violations.push(format!("{label} did not emit a JSON object"));
    }
}

fn check_documented_text(
    violations: &mut Vec<String>,
    label: &str,
    input: &AuthoredInput,
    text: &str,
) {
    if input.example.trim().is_empty() || !text.contains(input.example.trim()) {
        violations.push(format!("{label} omits the non-empty copy-paste example"));
    }
    if input.allowed_values.is_empty() {
        let lower = text.to_ascii_lowercase();
        if !lower.contains("allowed values") || !lower.contains("none") {
            violations.push(format!(
                "{label} omits the allowed-values list (or an explicit `none`)"
            ));
        }
    } else {
        for value in &input.allowed_values {
            if value.trim().is_empty() || !text.contains(value) {
                violations.push(format!("{label} omits allowed value {:?}", value));
            }
        }
    }
}

fn route_args(command_name: &str) -> Vec<&str> {
    command_name.split_whitespace().collect()
}

fn run_metadata(binary: &str, args: &[&str]) -> Option<Captured> {
    let state = tempfile::tempdir().expect("create authored-input scratch state");
    let witness = state.path().join("witness.jsonl");
    let home: &OsStr = state.path().as_os_str();
    let witness: &OsStr = witness.as_os_str();
    run_captured_with_timeout(
        binary,
        args,
        &[("HOME", home), ("EPISTEMIC_WITNESS", witness)],
        SELF_DISCOVERY_TIMEOUT,
    )
}

fn combined_text(output: &Captured) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn display(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}
