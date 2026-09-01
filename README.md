# spine-rules

Shared golden-rules enforcement for [CMD+RVL](https://cmdrvl.com) spine tools.

One crate, one update, all 13 tools enforced.

## What it does

- **`golden_rules_suite!`** macro — generates integration tests from ~20 lines of config
- **Assertion helpers** — debuggable, standalone functions for each rule (R-001 through R-022)
- **Composite-key conformance** — reusable ordered-tuple fixtures and JSON output assertions for `shape` / `rvl`
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

## Composite-key conformance

Composite row identity is an ordered tuple of raw byte strings. Consumers must:

- ASCII-trim spaces and tabs from each component independently
- treat a tuple as incomplete when any trimmed component is empty
- keep strings such as `NA`, `N/A`, `NULL`, and `null` as literal key bytes
- compare equality and ordering lexicographically over component byte strings
- keep component boundaries structural; never flatten identity through a sentinel-delimited string
- expose encoded component arrays as the authoritative machine value; a joined label is display-only

Use the shared assertion with an adapter around the consumer's internal key constructor:

```rust
use spine_rules::composite_key::assert_ordered_tuple_semantics;

#[test]
fn composite_keys_follow_the_spine_contract() {
    assert_ordered_tuple_semantics(|components| build_consumer_key(components));
}
```

For JSON output, `assert_authoritative_key_output` checks both the authoritative component array and its derived display label. `shape` should apply it to `key_columns` / `key_column`; `rvl` should apply it to `key_columns` / `key_column`, `row_key` / `row_id`, and refusal `key_values` / `key` pairs where present. The live pipeline test accepts `SPINE_SHAPE_BIN` and `SPINE_RVL_BIN`, otherwise preferring sibling debug builds and then `PATH`.

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
