# spine-rules

Shared golden-rules enforcement for [CMD+RVL](https://cmdrvl.com) spine tools.

One crate, one update, all 13 tools enforced.

## What it does

- **`golden_rules_suite!`** macro — generates integration tests from ~20 lines of config
- **Assertion helpers** — debuggable, standalone functions for each rule (R-001 through R-022)
- **`operator.v0` types** — serde structs for operator.json manifests
- **ast-grep rules** — static analysis YAML rules for exit codes, HashMap, and witness safety

## Quick start

Add to your spine tool:

```toml
[dev-dependencies]
spine-rules = { git = "https://github.com/cmdrvl/spine-rules" }
```

```rust
// tests/golden_rules.rs
use spine_rules::golden_rules_suite;

golden_rules_suite! {
    binary: env!("CARGO_BIN_EXE_canon"),
    operator_json: include_str!("../operator.json"),
    source_files: &["src/lib.rs", "src/refusal.rs"],
    fixture_success_args: &[
        "tests/fixtures/inputs/all_resolved.csv",
        "--registry", "tests/fixtures/registries/cusip-isin",
        "--column", "cusip", "--no-witness",
    ],
    fixture_refusal_args: &[
        "nonexistent.csv",
        "--registry", "tests/fixtures/registries/cusip-isin",
        "--column", "cusip", "--no-witness",
    ],
    fixture_csv: "tests/fixtures/inputs/partial.csv",
    fixture_csv_args: &[
        "tests/fixtures/inputs/partial.csv",
        "--registry", "tests/fixtures/registries/cusip-isin",
        "--column", "cusip", "--emit", "csv", "--no-witness",
    ],
}
```

## Generated tests

| Test | Rule | When |
|------|------|------|
| `r002_r007_exit_codes_trinity` | R-002, R-007 | Always |
| `r007_refusal_codes_match_source` | R-007 | Always |
| `r006_describe_valid` | R-006 | Always |
| `r006_schema_valid` | R-006 | Always |
| `r006_version_exits_zero` | R-006 | Always |
| `r001_resolved_has_version` | R-001 | If `fixture_success_args` set |
| `r001_refusal_has_version` | R-001 | If `fixture_refusal_args` set |
| `r008_no_witness_suppresses` | R-008 | If `fixture_success_args` set |
| `r017_witness_failure_nonfatal` | R-017 | If `fixture_success_args` set |
| `r021_csv_row_count_preserved` | R-021 | If `fixture_csv` + `fixture_csv_args` set |

## ast-grep rules

Copy `rules/` to your tool and add `sgconfig.yml`:

```yaml
ruleDirs:
  - rules
```

| Rule | Checks |
|------|--------|
| `exit-code-range.yml` | `process::exit()` outside 0/1/2 |
| `no-hashmap-in-output.yml` | `HashMap::new()` in output code |
| `witness-must-append.yml` | `File::create()` in witness code |

## License

MIT
