//! Assertion helpers for golden-rules enforcement.
//!
//! Each function enforces one or more rules from GOLDEN_RULES.md.
//! The macro in `macros.rs` wires these into `#[test]` functions.

use crate::contract::OperatorContract;
use crate::types::OperatorManifest;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

// ── Manifest view: operator.v1 (strict) or legacy operator.v0 ───────────────

/// The few manifest facts the R-002 / R-007 helpers need, read from whichever
/// schema the manifest declares. `operator.v1` goes through the strict
/// [`OperatorContract`] (so legacy spellings such as `version` / `subcommands`
/// fail even when `schema_version` says v1); `operator.v0` keeps the legacy
/// types so unmigrated tools still run.
struct ManifestView {
    exit_codes: Vec<(String, String)>,
    option_flags: BTreeSet<String>,
    has_doctor: bool,
    refusal_codes: BTreeSet<String>,
}

fn manifest_view(operator_json: &str) -> ManifestView {
    let raw: serde_json::Value =
        serde_json::from_str(operator_json).expect("operator.json must be valid JSON");
    let flags = |options: &[serde_json::Value]| -> BTreeSet<String> {
        options
            .iter()
            .filter_map(|o| o.get("flag").and_then(|f| f.as_str()).map(str::to_owned))
            .collect()
    };
    match raw.get("schema_version").and_then(|v| v.as_str()) {
        Some(crate::contract::SCHEMA_VERSION) => {
            let c = OperatorContract::parse(operator_json)
                .unwrap_or_else(|e| panic!("operator.v1 manifest is invalid: {e}"));
            ManifestView {
                exit_codes: c
                    .exit_codes
                    .iter()
                    .map(|(k, e)| (k.clone(), e.domain.clone()))
                    .collect(),
                option_flags: flags(&c.options),
                has_doctor: c.commands.iter().any(|x| x.name == "doctor"),
                refusal_codes: c
                    .refusals
                    .iter()
                    .chain(c.commands.iter().flat_map(|x| x.refusals.iter()))
                    .map(|r| r.code.clone())
                    .collect(),
            }
        }
        Some("operator.v0") => {
            let m: OperatorManifest = serde_json::from_str(operator_json)
                .expect("operator.v0 manifest must match the legacy schema");
            ManifestView {
                exit_codes: m
                    .exit_codes
                    .iter()
                    .map(|(k, e)| (k.clone(), e.domain.clone()))
                    .collect(),
                option_flags: flags(&m.options),
                has_doctor: m.subcommands.iter().any(|c| c.name == "doctor"),
                refusal_codes: m
                    .refusals
                    .iter()
                    .chain(m.subcommands.iter().flat_map(|c| c.refusals.iter()))
                    .map(|r| r.code.clone())
                    .collect(),
            }
        }
        other => panic!(
            "operator.json schema_version {other:?} is unsupported; expected \"operator.v1\" (or legacy \"operator.v0\")"
        ),
    }
}

// ── R-002 / R-007: Exit-code trinity ────────────────────────────────────────

/// Assert operator.json declares the family exit-code dictionary
/// (0/1/2, and 3 only as a `policy` verdict).
///
/// If `require_1` is false (e.g. vacuum), exit code 1 is optional unless the
/// tool declares `doctor` (then the dictionary check requires it anyway).
pub fn assert_exit_codes_trinity(operator_json: &str, require_1: bool) {
    let view = manifest_view(operator_json);
    let has = |code: &str| view.exit_codes.iter().any(|(k, _)| k == code);

    assert!(
        has("0"),
        "R-002/R-007: operator.json must declare exit code 0"
    );
    if require_1 {
        assert!(
            has("1"),
            "R-002/R-007: operator.json must declare exit code 1"
        );
    }
    assert!(
        has("2"),
        "R-002/R-007: operator.json must declare exit code 2"
    );

    let pairs: Vec<(&str, &str)> = view
        .exit_codes
        .iter()
        .map(|(k, d)| (k.as_str(), d.as_str()))
        .collect();
    let violations =
        crate::exit_codes::check_exit_codes(&pairs, &view.option_flags, view.has_doctor);
    assert!(
        violations.is_empty(),
        "R-002 VIOLATION: operator.json exit codes break the family dictionary:\n  - {}",
        violations.join("\n  - ")
    );
}

// ── R-007: Refusal codes match source ───────────────────────────────────────

/// Collect all E_ codes declared in operator.json (including subcommand/command refusals).
/// A tool with no refusal envelope declares `refusals: []` and yields an empty set.
pub fn collect_refusal_codes_from_manifest(operator_json: &str) -> BTreeSet<String> {
    manifest_view(operator_json).refusal_codes
}

/// Truncate source at the `#[cfg(test)]` module boundary.
///
/// Avoids false positives from test-only E_ code fixtures (e.g. `"E_NOPE"`
/// in error-path tests).
fn strip_test_module(source: &str) -> String {
    let mut result = String::new();
    for line in source.lines() {
        if line.trim() == "#[cfg(test)]" {
            break;
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

/// Extract E_ codes from source text (string literals matching `"E_UPPER_CASE"`).
pub fn extract_e_codes_from_source(source: &str) -> BTreeSet<String> {
    let mut codes = BTreeSet::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("\"E_") {
            continue;
        }
        let mut remaining = trimmed;
        while let Some(start) = remaining.find("\"E_") {
            let after = &remaining[start + 1..];
            if let Some(end) = after.find('"') {
                let candidate = &after[..end];
                if candidate.starts_with("E_")
                    && candidate
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_')
                {
                    codes.insert(candidate.to_string());
                }
            }
            remaining = &remaining[start + 1..];
        }
    }

    codes
}

/// Assert bidirectional match between operator.json refusal codes and source code.
pub fn assert_refusal_codes_match_source(
    operator_json: &str,
    source_files: &[&str],
    manifest_dir: &str,
) {
    let declared = collect_refusal_codes_from_manifest(operator_json);
    for code in &declared {
        assert!(
            code.starts_with("E_"),
            "R-007 VIOLATION: refusal code '{code}' must start with E_"
        );
    }

    let mut combined_source = String::new();
    for file in source_files {
        let path = Path::new(manifest_dir).join(file);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Cannot read {}: {}", path.display(), e));
        // Strip #[cfg(test)] modules to avoid false positives from test fixtures.
        let production_only = strip_test_module(&content);
        combined_source.push_str(&production_only);
        combined_source.push('\n');
    }

    let source_codes = extract_e_codes_from_source(&combined_source);

    // Every declared code must appear in source.
    for code in &declared {
        assert!(
            combined_source.contains(code),
            "R-007 VIOLATION: operator.json declares '{}' but it never appears in source",
            code
        );
    }

    // Every E_ code in source should be declared.
    for code in &source_codes {
        assert!(
            declared.contains(code),
            "R-007 VIOLATION: Source emits '{}' but operator.json does not declare it",
            code
        );
    }
}

// ── R-006: Introspection flags ──────────────────────────────────────────────

/// Assert `--describe` exits 0 and emits valid JSON with a "name" field.
pub fn assert_describe_valid(binary: &str) {
    let output = Command::new(binary)
        .arg("--describe")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} --describe: {}", binary, e));

    assert_eq!(
        output.status.code(),
        Some(0),
        "R-006: --describe must exit 0"
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("R-006: --describe must emit valid JSON");

    assert!(
        json.get("name").is_some(),
        "R-006: --describe output must include tool name"
    );
}

/// Assert `--schema` exits 0 and emits valid JSON with "properties".
pub fn assert_schema_valid(binary: &str) {
    let output = Command::new(binary)
        .arg("--schema")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} --schema: {}", binary, e));

    assert_eq!(output.status.code(), Some(0), "R-006: --schema must exit 0");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("R-006: --schema must emit valid JSON");

    assert!(
        json.get("$schema").is_some() || json.get("type").is_some(),
        "R-006: --schema output must be a JSON Schema document"
    );
    assert!(
        json.get("properties").is_some() || json.get("definitions").is_some(),
        "R-006: --schema must describe output structure (properties or definitions)"
    );
}

/// Assert `--version` exits 0.
pub fn assert_version_exits_zero(binary: &str) {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} --version: {}", binary, e));

    assert_eq!(
        output.status.code(),
        Some(0),
        "R-006: --version must exit 0"
    );
}

// ── R-001: Version field in output ──────────────────────────────────────────

/// Assert that a successful run includes "version" in JSON stdout.
///
/// Handles both single-object JSON and streaming JSONL (checks first line).
pub fn assert_output_has_version(binary: &str, args: &[&str]) {
    let output = Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {}", binary, e));

    // Try single JSON object first, fall back to first JSONL line.
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout
            .lines()
            .next()
            .expect("R-001: stdout must not be empty");
        serde_json::from_str(first_line).expect("R-001: first line must be valid JSON")
    });

    assert!(
        json.get("version").is_some(),
        "R-001 VIOLATION: Missing `version` field in RESOLVED output"
    );
}

/// Assert that a refusal (exit 2) includes "version" in JSON stdout.
pub fn assert_refusal_has_version(binary: &str, args: &[&str]) {
    let output = Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {}", binary, e));

    assert_eq!(
        output.status.code(),
        Some(2),
        "R-001: refusal fixture must exit 2, got {}",
        output.status.code().unwrap_or(-1)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("R-001: refusal stdout must be valid JSON");

    assert!(
        json.get("version").is_some(),
        "R-001 VIOLATION: Missing `version` field in REFUSAL output"
    );
}

// ── R-008 / R-017: Witness behaviour ────────────────────────────────────────

/// Assert `--no-witness` suppresses ledger file creation.
///
/// Establishes a baseline exit code first (without witness env), then
/// reruns with `EPISTEMIC_WITNESS` + `--no-witness` and asserts the
/// exit code is unchanged and no ledger file was created.
pub fn assert_no_witness_suppresses_ledger(binary: &str, args: &[&str]) {
    // Baseline: run without EPISTEMIC_WITNESS to get domain exit code.
    let baseline = Command::new(binary)
        .env_remove("EPISTEMIC_WITNESS")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} (baseline): {}", binary, e));
    let baseline_code = baseline.status.code();

    // Now run with EPISTEMIC_WITNESS pointed at a temp dir + --no-witness.
    let temp = tempfile::tempdir().expect("create temp dir");
    let ledger_path = temp.path().join("witness.jsonl");

    let mut full_args: Vec<&str> = args.to_vec();
    if !full_args.contains(&"--no-witness") {
        full_args.push("--no-witness");
    }

    let output = Command::new(binary)
        .env("EPISTEMIC_WITNESS", &ledger_path)
        .args(&full_args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {}", binary, e));

    assert_eq!(
        output.status.code(),
        baseline_code,
        "R-008: --no-witness must not change exit code (baseline {}, got {})",
        baseline_code.unwrap_or(-1),
        output.status.code().unwrap_or(-1)
    );

    assert!(
        !ledger_path.exists(),
        "R-008 VIOLATION: --no-witness specified but witness file was created"
    );
}

/// Assert witness write failure does not change the exit code.
///
/// Establishes a baseline exit code (without witness), then reruns
/// with an impossible `EPISTEMIC_WITNESS` path (without `--no-witness`)
/// and asserts the exit code is unchanged.
pub fn assert_witness_failure_nonfatal(binary: &str, args: &[&str]) {
    // Strip --no-witness from args so witness code actually runs.
    let bare_args: Vec<&str> = args
        .iter()
        .copied()
        .filter(|a| *a != "--no-witness")
        .collect();

    // Baseline: run without witness env.
    let baseline = Command::new(binary)
        .env_remove("EPISTEMIC_WITNESS")
        .args(&bare_args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} (baseline): {}", binary, e));
    let baseline_code = baseline.status.code();

    // Run with an impossible witness path — write will fail.
    let output = Command::new(binary)
        .env("EPISTEMIC_WITNESS", "/dev/null/impossible/witness.jsonl")
        .args(&bare_args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {}", binary, e));

    assert_eq!(
        output.status.code(),
        baseline_code,
        "R-017 VIOLATION: Witness write failure changed exit code from {} to {}",
        baseline_code.unwrap_or(-1),
        output.status.code().unwrap_or(-1)
    );
}

// ── R-021: CSV row count preservation ───────────────────────────────────────

/// Assert CSV output has the same number of lines as input.
pub fn assert_csv_row_count_preserved(binary: &str, csv_path: &str, csv_args: &[&str]) {
    let input_content = std::fs::read_to_string(csv_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {}", csv_path, e));
    let input_lines: Vec<&str> = input_content.lines().collect();

    let output = Command::new(binary)
        .args(csv_args)
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {}: {}", binary, e));

    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let output_lines: Vec<&str> = stdout.lines().collect();

    assert_eq!(
        input_lines.len(),
        output_lines.len(),
        "R-021 VIOLATION: CSV input has {} lines but output has {} lines. \
         Row count in must equal row count out.",
        input_lines.len(),
        output_lines.len()
    );
}
