//! Conformance rules over Evidence Protocol objects.
//!
//! The wire types are defined once, in the public `evidence-protocol` crate; this module
//! only CONSUMES them and never declares an `*.v0` type. A rule is added here only when a
//! named consumer needs it (see each rule's doc comment).
//!
//! Rules so far:
//!
//! - [`check_answer_coverage`]: `answer.v0` coverage reconciles. First consumer:
//!   `evidence-machine` `em-36x` (sealer / offline verifier), which must refuse to seal an
//!   answer whose coverage does not add up.

use evidence_protocol::v0::answer::Answer;
use serde_json::Value;

/// The terminal buckets an `answer.v0` coverage count adds up from.
const ATTEMPTED_BUCKETS: &[&str] = &[
    "complete_empty",
    "complete_positive",
    "incomplete_authority",
    "incomplete_budget",
    "incomplete_evidence",
    "incomplete_execution",
    "incomplete_source",
];

fn number(counts: &Value, name: &str) -> Option<u64> {
    counts.get(name)?.as_u64()
}

/// Violations in one `counts` object, empty when it reconciles. Re-derived from the raw JSON,
/// independently of the typed parser, so the rule still bites if a type's own check changes.
fn counts_violations(label: &str, counts: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    let mut attempted = 0_u64;
    for bucket in ATTEMPTED_BUCKETS {
        match number(counts, bucket) {
            Some(value) => attempted = attempted.saturating_add(value),
            None => violations.push(format!("{label}: `{bucket}` is missing or not a count")),
        }
    }
    let (Some(declared_attempted), Some(not_investigated), Some(out_of_scope), Some(total)) = (
        number(counts, "attempted"),
        number(counts, "not_investigated"),
        number(counts, "out_of_scope"),
        number(counts, "total"),
    ) else {
        violations.push(format!(
            "{label}: `attempted`, `not_investigated`, `out_of_scope` and `total` must all be counts"
        ));
        return violations;
    };
    if declared_attempted != attempted {
        violations.push(format!(
            "{label}: attempted is {declared_attempted} but its terminal buckets sum to {attempted}"
        ));
    }
    let expected_total = attempted
        .saturating_add(not_investigated)
        .saturating_add(out_of_scope);
    if total != expected_total {
        violations.push(format!(
            "{label}: total is {total} but all terminal buckets sum to {expected_total}"
        ));
    }
    violations
}

/// Check that `answer_json` is an `answer.v0` whose coverage reconciles: the document parses as
/// the `evidence-protocol` `Answer` type, and `total` equals the sum of the terminal states for
/// the certificate and for every breakdown. Returns every violation (empty means conformant).
pub fn check_answer_coverage(answer_json: &str) -> Vec<String> {
    let value: Value = match serde_json::from_str(answer_json) {
        Ok(value) => value,
        Err(error) => return vec![format!("not JSON: {error}")],
    };
    let mut violations = Vec::new();
    if value.get("schema").and_then(Value::as_str) != Some("answer.v0") {
        violations.push("schema must be `answer.v0`".to_owned());
    }
    if let Err(error) = serde_json::from_value::<Answer>(value.clone()) {
        violations.push(format!("not a valid answer.v0 object: {error}"));
    }
    if let Some(counts) = value.pointer("/coverage/counts") {
        violations.extend(counts_violations("coverage.counts", counts));
    } else {
        violations.push("coverage.counts is missing".to_owned());
    }
    if let Some(breakdowns) = value
        .pointer("/coverage/breakdowns")
        .and_then(Value::as_array)
    {
        for (index, breakdown) in breakdowns.iter().enumerate() {
            let label = format!("coverage.breakdowns[{index}].counts");
            match breakdown.get("counts") {
                Some(counts) => violations.extend(counts_violations(&label, counts)),
                None => violations.push(format!("{label} is missing")),
            }
        }
    }
    violations
}
