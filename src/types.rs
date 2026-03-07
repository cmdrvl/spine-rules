//! Serde types for the `operator.v0` manifest schema.
//!
//! All map types use `BTreeMap` for deterministic serialization (R-005).

use serde::Deserialize;
use std::collections::BTreeMap;

/// Top-level operator manifest (`operator.v0`).
#[derive(Debug, Deserialize)]
pub struct OperatorManifest {
    pub schema_version: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub agent_guide: Option<String>,
    #[serde(default)]
    pub invocation: Option<Invocation>,
    #[serde(default)]
    pub arguments: Vec<serde_json::Value>,
    #[serde(default)]
    pub options: Vec<serde_json::Value>,
    #[serde(default)]
    pub subcommands: Vec<Subcommand>,
    pub exit_codes: BTreeMap<String, ExitCode>,
    #[serde(default)]
    pub refusals: Vec<RefusalEntry>,
    #[serde(default)]
    pub capabilities: Option<Capabilities>,
    #[serde(default)]
    pub pipeline: Option<Pipeline>,
}

/// Invocation metadata.
#[derive(Debug, Deserialize)]
pub struct Invocation {
    pub binary: String,
    #[serde(default)]
    pub usage: Vec<String>,
    #[serde(default)]
    pub output_mode: Option<String>,
    #[serde(default)]
    pub output_schema: Option<String>,
    #[serde(default)]
    pub json_flag: Option<String>,
}

/// Exit code entry.
#[derive(Debug, Deserialize)]
pub struct ExitCode {
    pub meaning: String,
    pub domain: String,
}

/// A refusal entry — top-level or nested under a subcommand.
#[derive(Debug, Deserialize)]
pub struct RefusalEntry {
    pub code: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub flag: Option<String>,
}

/// Subcommand entry — may contain its own refusals.
#[derive(Debug, Deserialize)]
pub struct Subcommand {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub refusals: Vec<RefusalEntry>,
    // Capture remaining fields without strict typing.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Tool capabilities.
#[derive(Debug, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub profile_aware: bool,
    #[serde(default)]
    pub streaming: bool,
    #[serde(default)]
    pub witness: Option<WitnessCapabilities>,
}

/// Witness sub-capabilities.
#[derive(Debug, Deserialize)]
pub struct WitnessCapabilities {
    #[serde(default)]
    pub ambient_recording: Option<String>,
    #[serde(default)]
    pub query_subcommands: serde_json::Value,
    #[serde(default)]
    pub no_witness_flag: Option<String>,
}

/// Pipeline adjacency.
#[derive(Debug, Deserialize)]
pub struct Pipeline {
    #[serde(default)]
    pub upstream: Vec<String>,
    #[serde(default)]
    pub downstream: Vec<String>,
}
