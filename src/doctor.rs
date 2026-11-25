//! Family doctor contract (`cmdrvl.doctor.v1`).
//!
//! Every spine tool exposes three doctor reports under one schema:
//!
//! | invocation                          | `mode`         | extra required field |
//! |-------------------------------------|----------------|----------------------|
//! | `<tool> doctor --json`              | `health`       | —                    |
//! | `<tool> doctor capabilities --json` | `capabilities` | `contract` (the `operator.v1` document, identical to `capabilities --json`) |
//! | `<tool> doctor triage --json`       | `triage`       | `next_actions`       |
//!
//! Exit codes for `doctor`: `0` healthy, `1` unhealthy (at least one `error`
//! finding), `2` CLI/usage error only. A tool never answers an unhealthy
//! report with `2`.

use crate::contract::OperatorContract;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::process::Command;

/// `schema_version` of every doctor report.
pub const DOCTOR_SCHEMA_VERSION: &str = "cmdrvl.doctor.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DoctorMode {
    Health,
    Capabilities,
    Triage,
}

impl DoctorMode {
    pub const ALL: [DoctorMode; 3] = [Self::Health, Self::Capabilities, Self::Triage];

    /// Arguments after the binary name that produce this report as JSON.
    pub fn args(self) -> &'static [&'static str] {
        match self {
            Self::Health => &["doctor", "--json"],
            Self::Capabilities => &["doctor", "capabilities", "--json"],
            Self::Triage => &["doctor", "triage", "--json"],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: Severity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub schema_version: String,
    pub mode: DoctorMode,
    pub tool: String,
    pub tool_version: String,
    pub healthy: bool,
    pub findings: Vec<Finding>,
    /// Required (and only allowed) when `mode` is `capabilities`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<OperatorContract>,
    /// Required (and only allowed) when `mode` is `triage`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_actions: Option<Vec<String>>,
}

impl DoctorReport {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid doctor report JSON: {e}"))
    }

    /// `true` when no finding has severity `error`.
    pub fn derived_healthy(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// The process exit code this report must be returned with.
    pub fn exit_code(&self) -> i32 {
        if self.healthy { 0 } else { 1 }
    }

    /// Every violation of `cmdrvl.doctor.v1`; empty means conformant.
    pub fn validate(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.schema_version != DOCTOR_SCHEMA_VERSION {
            v.push(format!(
                "schema_version is {:?}, expected {DOCTOR_SCHEMA_VERSION:?}",
                self.schema_version
            ));
        }
        if self.tool.trim().is_empty() {
            v.push("tool is empty".into());
        }
        if self.healthy != self.derived_healthy() {
            v.push(format!(
                "healthy={} contradicts findings (error findings present: {})",
                self.healthy,
                !self.derived_healthy()
            ));
        }
        let mut ids = BTreeSet::new();
        for f in &self.findings {
            if f.id.trim().is_empty() {
                v.push("finding with empty id".into());
            } else if !ids.insert(f.id.as_str()) {
                v.push(format!("duplicate finding id {:?}", f.id));
            }
        }
        match (self.mode, &self.contract) {
            (DoctorMode::Capabilities, None) => {
                v.push("mode capabilities requires `contract`".into())
            }
            (DoctorMode::Capabilities, Some(c)) => {
                v.extend(c.validate().into_iter().map(|e| format!("contract: {e}")));
                if c.name != self.tool || c.tool_version != self.tool_version {
                    v.push(
                        "contract name/tool_version differ from the report's tool/tool_version"
                            .into(),
                    );
                }
            }
            (_, Some(_)) => v.push("`contract` is only allowed in capabilities mode".into()),
            (_, None) => {}
        }
        match (self.mode, &self.next_actions) {
            (DoctorMode::Triage, None) => v.push("mode triage requires `next_actions`".into()),
            (DoctorMode::Triage, Some(a)) if !self.healthy && a.is_empty() => {
                v.push("unhealthy triage report has empty next_actions".into())
            }
            (m, Some(_)) if m != DoctorMode::Triage => {
                v.push("`next_actions` is only allowed in triage mode".into())
            }
            _ => {}
        }
        v
    }
}

/// Check a doctor run's stdout and exit code against the contract.
/// Returns the violations (empty = conformant). Pure, so it is unit-testable.
pub fn check_doctor_run(mode: DoctorMode, stdout: &[u8], exit_code: Option<i32>) -> Vec<String> {
    let text = String::from_utf8_lossy(stdout);
    let report = match DoctorReport::parse(&text) {
        Ok(r) => r,
        Err(e) => return vec![e],
    };
    let mut v = report.validate();
    if report.mode != mode {
        v.push(format!(
            "report mode is {:?}, expected {mode:?}",
            report.mode
        ));
    }
    if exit_code != Some(report.exit_code()) {
        v.push(format!(
            "exit code {exit_code:?} but healthy={} requires {} (0 healthy, 1 unhealthy, 2 is usage errors only)",
            report.healthy,
            report.exit_code()
        ));
    }
    v
}

/// Run one doctor mode against `binary` and assert it conforms.
pub fn assert_doctor_contract(binary: &str, mode: DoctorMode) {
    let out = Command::new(binary)
        .args(mode.args())
        .output()
        .unwrap_or_else(|e| panic!("failed to run {binary} {:?}: {e}", mode.args()));
    let v = check_doctor_run(mode, &out.stdout, out.status.code());
    assert!(
        v.is_empty(),
        "`{binary} {}` violates cmdrvl.doctor.v1:\n  - {}\nstderr: {}",
        mode.args().join(" "),
        v.join("\n  - "),
        String::from_utf8_lossy(&out.stderr)
    );
}
