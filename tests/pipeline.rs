//! Pipeline integration tests — multi-tool chains exercising all 9 spine binaries.
//!
//! Each test chains two or more tools the way a real operator would. Tests skip
//! gracefully (via `require_binaries!`) when any required binary is missing.
//!
//! All fixture data is inline `const &str` and written to tempdirs at runtime.

use spine_rules::pipeline::*;
use spine_rules::require_binaries;
use std::fs;
use tempfile::tempdir;

// ── Fixture data ─────────────────────────────────────────────────────────────

const TAPE_OLD: &str = "\
loan_id,outstanding_balance,rate,term,status
L001,100000,4.5,360,current
L002,200000,3.75,240,current
L003,250000,5.0,180,delinquent
L004,175000,4.25,300,current
L005,325000,3.5,360,prepaid
";

const TAPE_NEW: &str = "\
loan_id,outstanding_balance,rate,term,status
L001,100000,4.5,360,current
L002,200000,3.75,240,current
L003,255000,5.0,180,delinquent
L004,175000,4.25,300,current
L005,325000,3.5,360,prepaid
";

const TAPE_A: &str = "\
loan_id,outstanding_balance,rate
L001,100000,4.5
L002,200000,3.75
";

const TAPE_B: &str = "\
loan_id,outstanding_balance,rate
L003,300000,5.0
";

const ENTITIES_CSV: &str = "\
id,ticker,amount
1,AAPL,50000
2,MSFT,75000
3,GOOGL,120000
4,AMZN,90000
5,TSLA,60000
";

const REGISTRY_JSON: &str = r#"{"id":"test-tickers","version":"1.0.0","description":"Test ticker to CUSIP registry","updated":"2026-01-01","entry_count":5}"#;

const TICKER_MAPPINGS: &str = r#"[
  {"input":"AAPL","canonical_id":"037833100","canonical_type":"cusip","rule_id":"ticker-match"},
  {"input":"MSFT","canonical_id":"594918104","canonical_type":"cusip","rule_id":"ticker-match"},
  {"input":"GOOGL","canonical_id":"02079K305","canonical_type":"cusip","rule_id":"ticker-match"},
  {"input":"AMZN","canonical_id":"023135106","canonical_type":"cusip","rule_id":"ticker-match"},
  {"input":"TSLA","canonical_id":"88160R101","canonical_type":"cusip","rule_id":"ticker-match"}
]"#;

const FP_CSV_HEADER: &str = "\
fingerprint_id: csv-header.v1
format: csv
assertions:
  - name: is_csv
    filename_regex:
      pattern: \"(?i)\\\\.csv$\"
";

const DRAFT_PROFILE: &str = "\
schema_version: 1
status: draft
format: csv
key:
  - loan_id
include_columns:
  - loan_id
  - outstanding_balance
  - rate
  - term
  - status
";

// ── Test 1: vacuum → hashbytes → lock → lock verify ─────────────────────────

#[test]
fn test_vacuum_hash_lock_verify() {
    require_binaries!(vacuum, hashbytes, lock);

    let dir = tempdir().expect("create tempdir");
    let root = dir.path();

    // Create 3 files in the scan root.
    fs::write(root.join("data.csv"), "col_a,col_b\n1,2\n3,4\n").unwrap();
    fs::write(root.join("config.json"), r#"{"key":"value"}"#).unwrap();
    fs::write(root.join("notes.txt"), "some notes\n").unwrap();

    // 1. vacuum → JSONL
    let root_str = root.to_str().unwrap();
    let vacuum_run = run_tool("vacuum", &[root_str, "--no-witness"]);
    assert!(
        vacuum_run.success(),
        "vacuum failed (exit {:?}): {}",
        vacuum_run.code,
        vacuum_run.stderr_text()
    );

    let vacuum_records = parse_jsonl(&vacuum_run.stdout);
    assert_eq!(
        vacuum_records.len(),
        3,
        "vacuum should emit 3 records, got {}",
        vacuum_records.len()
    );
    for rec in &vacuum_records {
        assert_eq!(rec["version"], "vacuum.v0");
        assert!(rec["tool_versions"]["vacuum"].is_string());
    }

    // 2. hashbytes (piped from vacuum JSONL bytes)
    let hash_run = run_tool_with_stdin("hashbytes", &["--no-witness"], &vacuum_run.stdout);
    assert!(
        hash_run.success(),
        "hashbytes failed (exit {:?}): {}",
        hash_run.code,
        hash_run.stderr_text()
    );

    let hash_records = parse_jsonl(&hash_run.stdout);
    assert_eq!(hash_records.len(), 3);
    for rec in &hash_records {
        assert_eq!(rec["version"], "hash.v0");
        assert!(
            rec["bytes_hash"].as_str().unwrap().starts_with("sha256:"),
            "bytes_hash should start with sha256:"
        );
        assert!(rec["tool_versions"]["hash"].is_string());
        assert!(rec["tool_versions"]["vacuum"].is_string());
    }

    // 3. lock (reads hash JSONL, writes lockfile to stdout)
    //    Write hash output to a file outside the scan root so lock doesn't
    //    pick it up as a member.
    let manifest_path = dir.path().join("_manifest.jsonl");
    fs::write(&manifest_path, &hash_run.stdout).unwrap();
    let manifest_str = manifest_path.to_str().unwrap();

    let lock_run = run_tool("lock", &[manifest_str, "--no-witness"]);
    assert!(
        lock_run.success(),
        "lock failed (exit {:?}): {}",
        lock_run.code,
        lock_run.stderr_text()
    );

    let lockfile = parse_json(&lock_run.stdout);
    assert_eq!(lockfile["version"], "lock.v0");
    assert_eq!(lockfile["member_count"], 3);
    assert!(lockfile["tool_versions"]["vacuum"].is_string());
    assert!(lockfile["tool_versions"]["hash"].is_string());
    assert!(lockfile["tool_versions"]["lock"].is_string());

    // 4. lock verify — write lockfile to disk, then verify.
    let lockfile_path = dir.path().join("test.lock.json");
    fs::write(&lockfile_path, &lock_run.stdout).unwrap();
    let lockfile_str = lockfile_path.to_str().unwrap();

    let verify_run = run_tool(
        "lock",
        &["verify", lockfile_str, "--root", root_str, "--json", "--no-witness"],
    );
    assert!(
        verify_run.success(),
        "lock verify failed (exit {:?}): {}",
        verify_run.code,
        verify_run.stderr_text()
    );

    let verify_result = parse_json(&verify_run.stdout);
    assert_eq!(verify_result["outcome"], "VERIFY_OK");
    assert!(verify_result["lock_hash"]["valid"].as_bool().unwrap());
}

// ── Test 2: vacuum → hashbytes → fingerprint → lock ─────────────────────────

#[test]
fn test_vacuum_hash_fingerprint_lock() {
    require_binaries!(vacuum, hashbytes, fingerprint, lock);

    let dir = tempdir().expect("create tempdir");
    let csv_dir = dir.path().join("csvs");
    fs::create_dir(&csv_dir).unwrap();

    // Create 2 CSV files (realistic loan-tape headers).
    fs::write(csv_dir.join("tape_a.csv"), TAPE_A).unwrap();
    fs::write(csv_dir.join("tape_b.csv"), TAPE_B).unwrap();

    // Create fingerprint definitions dir.
    let fp_dir = dir.path().join("fp_defs");
    fs::create_dir(&fp_dir).unwrap();
    fs::write(fp_dir.join("csv-header.fp.yaml"), FP_CSV_HEADER).unwrap();

    let csv_dir_str = csv_dir.to_str().unwrap();
    let fp_dir_str = fp_dir.to_str().unwrap();

    // 1. vacuum → hashbytes (piped)
    let hash_run = pipe_tools(
        "vacuum",
        &[csv_dir_str, "--no-witness"],
        "hashbytes",
        &["--no-witness"],
    );
    assert!(
        hash_run.success(),
        "vacuum|hashbytes failed (exit {:?}): {}",
        hash_run.code,
        hash_run.stderr_text()
    );

    let hash_records = parse_jsonl(&hash_run.stdout);
    assert_eq!(hash_records.len(), 2, "expected 2 hash records");

    // 2. Write hash JSONL → fingerprint
    let manifest_path = dir.path().join("_manifest.jsonl");
    fs::write(&manifest_path, &hash_run.stdout).unwrap();
    let manifest_str = manifest_path.to_str().unwrap();

    let fp_run = run_tool_with_env(
        "fingerprint",
        &[manifest_str, "--fp", "csv-header.v1", "--no-witness"],
        &[("FINGERPRINT_DEFINITIONS", fp_dir_str)],
    );
    assert!(
        fp_run.success(),
        "fingerprint failed (exit {:?}): {}",
        fp_run.code,
        fp_run.stderr_text()
    );

    let fp_records = parse_jsonl(&fp_run.stdout);
    assert_eq!(fp_records.len(), 2, "expected 2 fingerprint records");
    for rec in &fp_records {
        assert_eq!(rec["version"], "fingerprint.v0");
        assert!(rec["fingerprint"]["matched"].as_bool().unwrap());
        assert_eq!(rec["fingerprint"]["fingerprint_id"], "csv-header.v1");
        assert!(rec["tool_versions"]["fingerprint"].is_string());
        assert!(rec["tool_versions"]["hash"].is_string());
        assert!(rec["tool_versions"]["vacuum"].is_string());
    }

    // 3. fingerprint JSONL → lock
    let fp_manifest = dir.path().join("_fp_manifest.jsonl");
    fs::write(&fp_manifest, &fp_run.stdout).unwrap();
    let fp_manifest_str = fp_manifest.to_str().unwrap();

    let lock_run = run_tool("lock", &[fp_manifest_str, "--no-witness"]);
    assert!(
        lock_run.success(),
        "lock failed (exit {:?}): {}",
        lock_run.code,
        lock_run.stderr_text()
    );

    let lockfile = parse_json(&lock_run.stdout);
    assert_eq!(lockfile["version"], "lock.v0");
    assert_eq!(lockfile["member_count"], 2);

    // Lock members should carry fingerprint data.
    let members = lockfile["members"].as_array().unwrap();
    for member in members {
        assert!(
            member["fingerprint"].is_object(),
            "lock members should include fingerprint data"
        );
    }

    // tool_versions should include all 4 tools.
    let tv = &lockfile["tool_versions"];
    assert!(tv["vacuum"].is_string(), "missing vacuum in tool_versions");
    assert!(tv["hash"].is_string(), "missing hash in tool_versions");
    assert!(
        tv["fingerprint"].is_string(),
        "missing fingerprint in tool_versions"
    );
    assert!(tv["lock"].is_string(), "missing lock in tool_versions");
}

// ── Test 3: shape + rvl — structural check + change detection ────────────────

#[test]
fn test_shape_rvl_comparison() {
    require_binaries!(shape, rvl);

    let dir = tempdir().expect("create tempdir");
    let old_path = dir.path().join("old.csv");
    let new_path = dir.path().join("new.csv");

    fs::write(&old_path, TAPE_OLD).unwrap();
    fs::write(&new_path, TAPE_NEW).unwrap();

    let old_str = old_path.to_str().unwrap();
    let new_str = new_path.to_str().unwrap();

    // 1. shape: same columns, same row count → COMPATIBLE (exit 0)
    let shape_run = run_tool("shape", &[old_str, new_str, "--json", "--no-witness"]);
    assert!(
        shape_run.success(),
        "shape should exit 0 (COMPATIBLE), got {:?}: {}",
        shape_run.code,
        shape_run.stderr_text()
    );

    let shape_result = parse_json(&shape_run.stdout);
    assert_eq!(shape_result["version"], "shape.v0");
    assert_eq!(shape_result["outcome"], "COMPATIBLE");

    // 2. rvl: L003 balance changed 250000→255000 → REAL_CHANGE (exit 1)
    let rvl_run = run_tool("rvl", &[old_str, new_str, "--json", "--no-witness"]);
    assert_eq!(
        rvl_run.code,
        Some(1),
        "rvl should exit 1 (REAL_CHANGE), got {:?}: {}",
        rvl_run.code,
        rvl_run.stderr_text()
    );

    let rvl_result = parse_json(&rvl_run.stdout);
    assert_eq!(rvl_result["version"], "rvl.v0");
    assert_eq!(rvl_result["outcome"], "REAL_CHANGE");

    // Contributors should identify the changed cell.
    let contributors = rvl_result["contributors"].as_array().unwrap();
    assert!(
        !contributors.is_empty(),
        "rvl should report at least one contributor"
    );

    let first = &contributors[0];
    assert_eq!(first["old"], 250000.0);
    assert_eq!(first["new"], 255000.0);
    assert_eq!(first["delta"], 5000.0);
}

// ── Test 4: canon — identity resolution ──────────────────────────────────────

#[test]
fn test_canon_resolution() {
    require_binaries!(canon);

    let dir = tempdir().expect("create tempdir");

    // Create input CSV.
    let input_path = dir.path().join("entities.csv");
    fs::write(&input_path, ENTITIES_CSV).unwrap();

    // Create registry directory.
    let registry_dir = dir.path().join("registry");
    fs::create_dir(&registry_dir).unwrap();
    fs::write(registry_dir.join("registry.json"), REGISTRY_JSON).unwrap();
    fs::write(
        registry_dir.join("ticker-to-cusip.json"),
        TICKER_MAPPINGS,
    )
    .unwrap();

    let input_str = input_path.to_str().unwrap();
    let registry_str = registry_dir.to_str().unwrap();

    // 1. JSON mode: all 5 tickers resolved → RESOLVED (exit 0)
    let json_run = run_tool(
        "canon",
        &[
            input_str,
            "--registry",
            registry_str,
            "--column",
            "ticker",
            "--no-witness",
        ],
    );
    assert!(
        json_run.success(),
        "canon JSON mode failed (exit {:?}): {}",
        json_run.code,
        json_run.stderr_text()
    );

    let json_result = parse_json(&json_run.stdout);
    assert_eq!(json_result["version"], "canon.v0");
    assert_eq!(json_result["outcome"], "RESOLVED");
    assert_eq!(json_result["summary"]["total"], 5);
    assert_eq!(json_result["summary"]["resolved"], 5);
    assert_eq!(json_result["summary"]["unresolved"], 0);

    let mappings = json_result["mappings"].as_array().unwrap();
    assert_eq!(mappings.len(), 5);

    // 2. CSV mode: preserves row count (header + 5 data rows = 6 lines)
    let csv_run = run_tool(
        "canon",
        &[
            input_str,
            "--registry",
            registry_str,
            "--column",
            "ticker",
            "--emit",
            "csv",
            "--no-witness",
        ],
    );
    assert!(
        csv_run.success(),
        "canon CSV mode failed (exit {:?}): {}",
        csv_run.code,
        csv_run.stderr_text()
    );

    let csv_output = csv_run.stdout_text();
    let csv_lines: Vec<&str> = csv_output.lines().collect();
    // Header + 5 data rows = 6 lines
    assert_eq!(
        csv_lines.len(),
        6,
        "canon CSV should have 6 lines (header + 5 data), got {}",
        csv_lines.len()
    );
    // Header should have the appended canonical column.
    assert!(
        csv_lines[0].contains("ticker__canon"),
        "CSV header should include ticker__canon"
    );
}

// ── Test 5: full evidence pipeline ───────────────────────────────────────────

#[test]
fn test_full_evidence_pipeline() {
    require_binaries!(profile, shape, rvl, vacuum, hashbytes, lock, pack);

    let dir = tempdir().expect("create tempdir");

    // Create CSVs.
    let csv_dir = dir.path().join("csvs");
    fs::create_dir(&csv_dir).unwrap();
    let old_path = csv_dir.join("old.csv");
    let new_path = csv_dir.join("new.csv");
    fs::write(&old_path, TAPE_OLD).unwrap();
    fs::write(&new_path, TAPE_NEW).unwrap();

    let old_str = old_path.to_str().unwrap();
    let new_str = new_path.to_str().unwrap();

    // Create profile YAML.
    let profile_path = dir.path().join("profile.yaml");
    fs::write(&profile_path, DRAFT_PROFILE).unwrap();
    let profile_str = profile_path.to_str().unwrap();

    // 1. profile show — validate profile is parseable.
    let profile_run = run_tool(
        "profile",
        &["show", profile_str, "--json", "--no-witness"],
    );
    assert!(
        profile_run.success(),
        "profile show failed (exit {:?}): {}",
        profile_run.code,
        profile_run.stderr_text()
    );
    let profile_result = parse_json(&profile_run.stdout);
    assert_eq!(profile_result["outcome"], "SUCCESS");

    // 2. shape — structural compatibility gate.
    let shape_run = run_tool("shape", &[old_str, new_str, "--json", "--no-witness"]);
    assert!(
        shape_run.success(),
        "shape failed (exit {:?}): {}",
        shape_run.code,
        shape_run.stderr_text()
    );

    let reports_dir = dir.path().join("reports");
    fs::create_dir(&reports_dir).unwrap();
    let shape_report = reports_dir.join("shape_report.json");
    fs::write(&shape_report, &shape_run.stdout).unwrap();

    // 3. rvl — change detection.
    let rvl_run = run_tool("rvl", &[old_str, new_str, "--json", "--no-witness"]);
    assert_eq!(
        rvl_run.code,
        Some(1),
        "rvl should exit 1 (REAL_CHANGE), got {:?}",
        rvl_run.code
    );

    let rvl_report = reports_dir.join("rvl_report.json");
    fs::write(&rvl_report, &rvl_run.stdout).unwrap();

    // 4. vacuum → hashbytes → lock (piped chain).
    let csv_dir_str = csv_dir.to_str().unwrap();

    let hash_run = pipe_tools(
        "vacuum",
        &[csv_dir_str, "--no-witness"],
        "hashbytes",
        &["--no-witness"],
    );
    assert!(
        hash_run.success(),
        "vacuum|hashbytes failed (exit {:?}): {}",
        hash_run.code,
        hash_run.stderr_text()
    );

    let manifest_path = dir.path().join("_manifest.jsonl");
    fs::write(&manifest_path, &hash_run.stdout).unwrap();
    let manifest_str = manifest_path.to_str().unwrap();

    let lock_run = run_tool("lock", &[manifest_str, "--no-witness"]);
    assert!(
        lock_run.success(),
        "lock failed (exit {:?}): {}",
        lock_run.code,
        lock_run.stderr_text()
    );

    let lockfile_path = reports_dir.join("dataset.lock.json");
    fs::write(&lockfile_path, &lock_run.stdout).unwrap();

    // 5. pack seal — bundle lockfile + reports into an evidence pack.
    let pack_dir = dir.path().join("evidence_pack");
    let pack_dir_str = pack_dir.to_str().unwrap();
    let lockfile_str = lockfile_path.to_str().unwrap();
    let shape_report_str = shape_report.to_str().unwrap();
    let rvl_report_str = rvl_report.to_str().unwrap();

    let seal_run = run_tool(
        "pack",
        &[
            "seal",
            lockfile_str,
            shape_report_str,
            rvl_report_str,
            "--output",
            pack_dir_str,
            "--no-witness",
        ],
    );
    assert!(
        seal_run.success(),
        "pack seal failed (exit {:?}): {}",
        seal_run.code,
        seal_run.stderr_text()
    );

    // 6. pack verify — verify the evidence pack is intact.
    let verify_run = run_tool(
        "pack",
        &["verify", pack_dir_str, "--no-witness"],
    );
    assert!(
        verify_run.success(),
        "pack verify failed (exit {:?}): {}",
        verify_run.code,
        verify_run.stderr_text()
    );
}

// ── Helper: run_tool with extra environment variables ────────────────────────

fn run_tool_with_env(binary: &str, args: &[&str], env_vars: &[(&str, &str)]) -> ToolRun {
    use std::process::{Command, Stdio};

    let mut cmd = Command::new(binary);
    cmd.args(args)
        .env_remove("EPISTEMIC_WITNESS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, val) in env_vars {
        cmd.env(key, val);
    }

    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} {:?}: {}", binary, args, e));

    ToolRun {
        code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}
