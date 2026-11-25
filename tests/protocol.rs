//! Protocol conformance rule (answer.v0 coverage), the "no *.v0 type defined here" boundary
//! test, and the cargo-deny dependency boundary with its negative test.

use std::{fs, path::Path, process::Command};

use serde_json::Value;
use spine_rules::protocol::check_answer_coverage;

const PASSING: &str = include_str!("../fixtures/answers/nport_forced_established.v0.json");
const FAILING_TOTAL: &str = include_str!("../fixtures/answers/invalid-coverage-total.v0.json");

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

// ---------------------------------------------------------------------------
// The conformance rule
// ---------------------------------------------------------------------------

#[test]
fn a_reconciling_answer_is_conformant() {
    assert_eq!(check_answer_coverage(PASSING), Vec::<String>::new());
}

#[test]
fn a_certificate_total_that_does_not_add_up_is_reported() {
    let violations = check_answer_coverage(FAILING_TOTAL);
    assert!(
        violations
            .iter()
            .any(|v| v.contains("coverage.counts: total is 4 but all terminal buckets sum to 3")),
        "{violations:?}"
    );
}

#[test]
fn a_breakdown_that_does_not_add_up_is_reported_with_its_index() {
    let mut answer: Value = serde_json::from_str(PASSING).expect("fixture parses");
    answer["coverage"]["breakdowns"][1]["counts"]["complete_positive"] = Value::from(5);
    let violations = check_answer_coverage(&answer.to_string());
    assert!(
        violations
            .iter()
            .any(|v| v.starts_with("coverage.breakdowns[1].counts: attempted is 3 but")),
        "{violations:?}"
    );
    assert!(
        !violations.iter().any(|v| v.contains("breakdowns[0]")),
        "only the tampered breakdown should be named: {violations:?}"
    );
}

#[test]
fn wrong_schema_and_non_json_are_refused() {
    assert!(!check_answer_coverage("{").is_empty());
    let mut answer: Value = serde_json::from_str(PASSING).expect("fixture parses");
    answer["schema"] = Value::from("answer.v1");
    assert!(
        check_answer_coverage(&answer.to_string())
            .iter()
            .any(|v| v.contains("schema must be `answer.v0`"))
    );
}

// ---------------------------------------------------------------------------
// Boundary: this crate consumes the protocol, it never defines a *.v0 wire type
// ---------------------------------------------------------------------------

#[test]
fn no_protocol_wire_type_is_defined_in_this_crate() {
    const PROTOCOL_TYPES: &[&str] = &[
        "Inquiry",
        "Grant",
        "Artifact",
        "Plan",
        "PlanPatch",
        "Receipt",
        "Run",
        "Answer",
        "Capability",
    ];
    const PROTOCOL_IDS: &[&str] = &[
        "inquiry",
        "grant",
        "artifact",
        "plan",
        "plan_patch",
        "receipt",
        "run",
        "answer",
        "capability",
    ];
    for entry in fs::read_dir(root().join("src")).expect("src dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("source file");
        for name in PROTOCOL_TYPES {
            for keyword in ["struct", "enum"] {
                let declaration = format!("pub {keyword} {name} ");
                let declaration_brace = format!("pub {keyword} {name}{{");
                assert!(
                    !source.contains(&declaration) && !source.contains(&declaration_brace),
                    "{} declares protocol type `{name}`; it belongs in evidence-protocol",
                    path.display()
                );
            }
        }
        for id in PROTOCOL_IDS {
            let rename = format!("rename = \"{id}.v0\"");
            assert!(
                !source.contains(&rename),
                "{} declares the wire id `{id}.v0`; it belongs in evidence-protocol",
                path.display()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Dependency boundary: cargo-deny bans evidence-machine and SaltIO git sources
// ---------------------------------------------------------------------------

fn cargo_deny(args: &[&str], manifest: &Path) -> (bool, String) {
    let output = Command::new("cargo")
        .arg("deny")
        .arg("--manifest-path")
        .arg(manifest)
        .args(args)
        .output()
        .expect("cargo should run");
    (
        output.status.success(),
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

fn require_cargo_deny() {
    let ok = Command::new("cargo")
        .args(["deny", "--version"])
        .output()
        .is_ok_and(|output| output.status.success());
    assert!(
        ok,
        "cargo-deny is required for the boundary test: cargo install cargo-deny"
    );
}

#[test]
fn this_crate_passes_its_own_dependency_boundary() {
    require_cargo_deny();
    let (ok, out) = cargo_deny(
        &[
            "--config",
            root().join("deny.toml").to_str().expect("utf-8 path"),
            "check",
            "bans",
            "sources",
        ],
        &root().join("Cargo.toml"),
    );
    // `check` options must follow the global ones; cargo-deny prints its verdict on stderr.
    assert!(ok, "cargo deny check bans sources failed:\n{out}");
}

fn negative_manifest(dir: &Path, dependency: &str) -> std::path::PathBuf {
    fs::create_dir_all(dir.join("member/src")).expect("member dir");
    fs::write(
        dir.join("member/Cargo.toml"),
        format!("[package]\nname = \"{dependency}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n"),
    )
    .expect("member manifest");
    fs::write(dir.join("member/src/lib.rs"), "").expect("member lib");
    fs::create_dir_all(dir.join("consumer/src")).expect("consumer dir");
    let manifest = dir.join("consumer/Cargo.toml");
    fs::write(
        &manifest,
        format!(
            "[package]\nname = \"consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependency} = {{ version = \"0.0.0\", path = \"../member\" }}\n"
        ),
    )
    .expect("consumer manifest");
    fs::write(dir.join("consumer/src/lib.rs"), "").expect("consumer lib");
    manifest
}

#[test]
fn a_manifest_that_adds_evidence_machine_fails_the_ban() {
    require_cargo_deny();
    let dir = tempfile::tempdir().expect("temp dir");
    let config = root().join("deny.toml");
    let config = config.to_str().expect("utf-8 path");

    // Control: the same shape with an innocent crate name passes, so the failure below is the ban's.
    let control = negative_manifest(&dir.path().join("control"), "innocent-crate");
    let (ok, out) = cargo_deny(&["--config", config, "check", "bans"], &control);
    assert!(
        ok,
        "control with an innocent dependency should pass:\n{out}"
    );

    let banned = negative_manifest(&dir.path().join("banned"), "evidence-machine");
    let (ok, out) = cargo_deny(&["--config", config, "check", "bans"], &banned);
    assert!(
        !ok,
        "a dependency on evidence-machine must fail the ban:\n{out}"
    );
    assert!(out.contains("evidence-machine"), "{out}");
}

#[test]
fn the_reusable_template_bans_the_private_runtime_and_saltio_git_sources() {
    let template = fs::read_to_string(root().join("docs/deny-template.toml")).expect("template");
    assert!(template.contains("evidence-machine"));
    assert!(template.contains("unknown-git = \"deny\""));
    // Operator repos depend on spine-rules by git, so it is allowed; SaltIO never is.
    assert!(template.contains("\"https://github.com/cmdrvl/spine-rules\""));
    let allow_git = template
        .split("allow-git")
        .nth(1)
        .expect("template has an allow-git list");
    let allowed: Vec<&str> = allow_git
        .lines()
        .filter(|line| line.trim_start().starts_with('"'))
        .collect();
    assert!(
        allowed
            .iter()
            .all(|line| line.contains("github.com/cmdrvl/")),
        "allow-git must list cmdrvl repos only: {allowed:?}"
    );
    assert!(
        !template.contains("name = \"spine-rules\""),
        "the template is for operator repos, which depend on spine-rules"
    );
}
