//! Unified family contract (`operator.v1`).
//!
//! One struct, [`OperatorContract`], backs both `<tool> --describe` (the
//! `operator.json` file) and `<tool> capabilities --json`. A tool renders both
//! surfaces from the same value, so they cannot drift. [`OperatorContract::validate`]
//! is the executable definition of the contract; `schemas/operator.v1.schema.json`
//! is the published JSON Schema and is kept in lockstep with the struct by a test.
//!
//! Parsing is strict: the canonical spellings are `tool_version`, `commands` and
//! `refusals`. Legacy spellings (`version`, `subcommands`, `error_kinds`) are NOT
//! accepted, so a legacy-spelling manifest that merely declares
//! `schema_version: "operator.v1"` fails. Unmigrated `operator.v0` manifests are
//! handled only by the legacy helpers in `helpers.rs`.
//!
//! All maps are `BTreeMap` for deterministic serialization (R-005).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// `schema_version` value for this contract.
pub const SCHEMA_VERSION: &str = "operator.v1";

/// Published JSON Schema for [`OperatorContract`].
pub const OPERATOR_V1_SCHEMA: &str = include_str!("../schemas/operator.v1.schema.json");

/// Subcommands every spine tool must declare (and implement).
pub const REQUIRED_SUBCOMMANDS: &[&str] = &["capabilities", "robot-docs", "doctor"];

/// The canonical machine-readable contract for one spine tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperatorContract {
    pub schema_version: String,
    pub name: String,
    /// The tool's own release version; must equal the `Cargo.toml` package version.
    pub tool_version: String,
    /// Version of the family contract the tool conforms to (semver).
    pub contract_version: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_guide: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation: Option<Invocation>,
    #[serde(default)]
    pub arguments: Vec<serde_json::Value>,
    #[serde(default)]
    pub options: Vec<serde_json::Value>,
    pub commands: Vec<Command>,
    pub exit_codes: BTreeMap<String, ExitCode>,
    pub refusals: Vec<Refusal>,
    pub env_vars: Vec<EnvVar>,
    pub formats: Vec<String>,
    pub side_effects: SideEffects,
    pub pipeline: Pipeline,
    /// Tool-specific additions; never required by the family contract.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Invocation {
    pub binary: String,
    #[serde(default)]
    pub usage: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_flag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Authored input files accepted by this command and their self-discovery surfaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authored_inputs: Vec<AuthoredInput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refusals: Vec<Refusal>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// One file format that an operator authors and passes to a command.
///
/// The command must expose the declared `schema_flag` with no positional input,
/// and must repeat `example` and the allowed values in its help and robot guide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredInput {
    pub name: String,
    pub schema_flag: String,
    pub example: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExitCode {
    pub meaning: String,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<String>,
}

/// An environment variable the tool reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// What running the tool can touch. Every field is mandatory so a tool cannot
/// stay silent about a side effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SideEffects {
    pub reads_files: bool,
    pub writes_files: bool,
    pub network: bool,
    pub writes_witness: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pipeline {
    pub upstream: Vec<String>,
    pub downstream: Vec<String>,
}

impl OperatorContract {
    /// Parse a contract from `operator.json` / `--describe` / `capabilities --json` text.
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid operator contract JSON: {e}"))
    }

    /// Canonical rendering shared by `--describe` and `capabilities --json`.
    pub fn to_canonical_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("contract serializes")
    }

    /// Every violation of the `operator.v1` contract; empty means conformant.
    pub fn validate(&self) -> Vec<String> {
        let mut v = Vec::new();

        if self.schema_version != SCHEMA_VERSION {
            v.push(format!(
                "schema_version is {:?}, expected {SCHEMA_VERSION:?}",
                self.schema_version
            ));
        }
        if self.name.trim().is_empty() {
            v.push("name is empty".into());
        }
        if parse_semver(&self.tool_version).is_none() {
            v.push(format!(
                "tool_version {:?} is not MAJOR.MINOR.PATCH",
                self.tool_version
            ));
        }
        if parse_semver(&self.contract_version).is_none() {
            v.push(format!(
                "contract_version {:?} is not MAJOR.MINOR.PATCH",
                self.contract_version
            ));
        }
        if self.description.trim().is_empty() {
            v.push("description is empty".into());
        }

        let mut names = BTreeSet::new();
        for c in &self.commands {
            if !names.insert(c.name.as_str()) {
                v.push(format!("duplicate command {:?}", c.name));
            }
            for input in &c.authored_inputs {
                if input.name.trim().is_empty() {
                    v.push(format!(
                        "command {:?} has authored input with empty name",
                        c.name
                    ));
                }
                if !matches!(input.schema_flag.as_str(), "--input-schema" | "--schema") {
                    v.push(format!(
                        "command {:?} authored input {:?} has unsupported schema flag {:?}; expected --input-schema or --schema",
                        c.name, input.name, input.schema_flag
                    ));
                }
                if input.example.trim().is_empty() {
                    v.push(format!(
                        "command {:?} authored input {:?} has an empty example",
                        c.name, input.name
                    ));
                }
                let mut values = BTreeSet::new();
                for value in &input.allowed_values {
                    if value.trim().is_empty() {
                        v.push(format!(
                            "command {:?} authored input {:?} has an empty allowed value",
                            c.name, input.name
                        ));
                    }
                    if !values.insert(value.as_str()) {
                        v.push(format!(
                            "command {:?} authored input {:?} repeats allowed value {:?}",
                            c.name, input.name, value
                        ));
                    }
                }
            }
        }
        for required in REQUIRED_SUBCOMMANDS {
            if !names.contains(required) {
                v.push(format!(
                    "commands is missing required subcommand {required:?}"
                ));
            }
        }

        let pairs: Vec<(&str, &str)> = self
            .exit_codes
            .iter()
            .map(|(k, e)| (k.as_str(), e.domain.as_str()))
            .collect();
        let flags: BTreeSet<String> = self
            .options
            .iter()
            .filter_map(|o| o.get("flag").and_then(|f| f.as_str()).map(str::to_owned))
            .collect();
        v.extend(crate::exit_codes::check_exit_codes(
            &pairs,
            &flags,
            self.commands.iter().any(|c| c.name == "doctor"),
        ));

        let mut codes = BTreeSet::new();
        for r in self
            .refusals
            .iter()
            .chain(self.commands.iter().flat_map(|c| c.refusals.iter()))
        {
            if !r.code.starts_with("E_") {
                v.push(format!("refusal code {:?} does not start with E_", r.code));
            }
            codes.insert(r.code.as_str());
        }
        let mut top = BTreeSet::new();
        for r in &self.refusals {
            if !top.insert(r.code.as_str()) {
                v.push(format!("duplicate top-level refusal {:?}", r.code));
            }
        }

        let mut envs = BTreeSet::new();
        for e in &self.env_vars {
            let ok = !e.name.is_empty()
                && e.name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            if !ok {
                v.push(format!("env var name {:?} is not UPPER_SNAKE_CASE", e.name));
            }
            if !envs.insert(e.name.as_str()) {
                v.push(format!("duplicate env var {:?}", e.name));
            }
        }

        if self.formats.is_empty() {
            v.push("formats is empty".into());
        }
        let mut seen = BTreeSet::new();
        for f in &self.formats {
            if !seen.insert(f.as_str()) {
                v.push(format!("duplicate format {f:?}"));
            }
        }
        v
    }

    /// Panic with every violation, for use in tests.
    pub fn assert_valid(&self) {
        let v = self.validate();
        assert!(
            v.is_empty(),
            "operator contract for {:?} violates operator.v1:\n  - {}",
            self.name,
            v.join("\n  - ")
        );
    }
}

fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let core = s.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Read `[package] version` from `Cargo.toml` text. Returns `None` for
/// `version.workspace = true` or a missing key (callers must then fail loudly).
pub fn cargo_package_version(cargo_toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == "version"
        {
            let value = value.split('#').next()?.trim();
            return value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .map(str::to_owned);
        }
    }
    None
}

/// Assert the contract's `tool_version` (or v0 `version`) equals the
/// `Cargo.toml` package version. Catches drift such as `shape` 0.9.0 vs 0.8.0.
pub fn assert_version_parity(operator_json: &str, cargo_toml: &str) {
    let contract = OperatorContract::parse_lenient_version(operator_json);
    let cargo = cargo_package_version(cargo_toml).expect(
        "Cargo.toml has no literal [package] version (workspace-inherited versions are unsupported)",
    );
    assert_eq!(
        contract, cargo,
        "operator.json version {contract:?} != Cargo.toml package version {cargo:?}"
    );
}

/// Assert the contract's command names include every name in `required`.
/// Accepts v0 (`subcommands`) and v1 (`commands`) manifests.
pub fn assert_required_subcommands(operator_json: &str, required: &[&str]) {
    let doc: serde_json::Value =
        serde_json::from_str(operator_json).expect("operator.json is valid JSON");
    let list = doc
        .get("commands")
        .or_else(|| doc.get("subcommands"))
        .and_then(|v| v.as_array())
        .expect("operator.json has no commands/subcommands array");
    let have: BTreeSet<&str> = list
        .iter()
        .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
        .collect();
    let missing: Vec<&&str> = required.iter().filter(|r| !have.contains(**r)).collect();
    assert!(
        missing.is_empty(),
        "operator.json is missing required subcommands {missing:?}; declared: {have:?}"
    );
}

/// Parse `operator.json` and assert it satisfies the whole `operator.v1`
/// contract (`validate()`): schema_version, env_vars, side_effects, pipeline,
/// formats, `E_` refusals, exit-code dictionary, required subcommands.
pub fn assert_operator_valid(operator_json: &str) {
    OperatorContract::parse(operator_json)
        .unwrap_or_else(|e| panic!("operator.json does not parse as operator.v1: {e}"))
        .assert_valid();
}

/// Assert `capabilities --json` emits exactly the document `--describe` emits.
pub fn assert_capabilities_match_describe(binary: &str) {
    let describe = run_json(binary, &["--describe"]);
    let caps = run_json(binary, &["capabilities", "--json"]);
    assert_eq!(
        describe, caps,
        "`{binary} capabilities --json` differs from `{binary} --describe`; render both from one OperatorContract"
    );
}

fn run_json(binary: &str, args: &[&str]) -> serde_json::Value {
    let out = std::process::Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {binary} {args:?}: {e}"));
    assert!(
        out.status.success(),
        "{binary} {args:?} exited {:?}; stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{binary} {args:?} stdout is not JSON: {e}"))
}

impl OperatorContract {
    /// Version string from a v0 or v1 manifest without requiring the rest of it to be valid.
    fn parse_lenient_version(operator_json: &str) -> String {
        let doc: serde_json::Value =
            serde_json::from_str(operator_json).expect("operator.json is valid JSON");
        doc.get("tool_version")
            .or_else(|| doc.get("version"))
            .and_then(|v| v.as_str())
            .expect("operator.json has no tool_version/version string")
            .to_owned()
    }
}
