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

## Unified `operator.v1` contract

`spine_rules::contract::OperatorContract` is the one struct behind both `<tool> --describe` (`operator.json`) and `<tool> capabilities --json`. Render both from the same value (`to_canonical_json()`); `golden_rules_suite!` fails if the two outputs differ. The JSON Schema is `schemas/operator.v1.schema.json` (also `contract::OPERATOR_V1_SCHEMA`); a test keeps it in lockstep with the struct.

Required: `schema_version` (`operator.v1`), `name`, `tool_version`, `contract_version`, `description`, `commands`, `exit_codes` (0/1/2 only), `refusals` (`E_*`), `env_vars`, `formats`, `side_effects`, `pipeline`. v0 manifests (`version`, `subcommands`) still parse through serde aliases; only `operator.v1` passes `validate()`.

Parsing is strict: only the canonical spellings (`tool_version`, `commands`, `refusals`) are accepted, so a file that keeps the legacy spellings (`version`, `subcommands`) but declares `operator.v1` fails. The R-002/R-007 helpers read `operator.v1` manifests through `OperatorContract` and keep the legacy path only for `operator.v0` manifests. A tool with no refusal envelope declares `refusals: []`.

New generated tests: `r006_operator_version_matches_cargo_toml` (operator version == `Cargo.toml` package version; workspace-inherited versions are rejected), `r006_operator_declares_required_subcommands` (`capabilities`, `robot-docs`, `doctor`; override with `required_subcommands: &[...]`), `r006_capabilities_json_matches_describe`.

## Doctor contract (`cmdrvl.doctor.v1`)

One schema for all three doctor reports (`spine_rules::doctor::DoctorReport`):

| Command | `mode` | Extra required field |
|---|---|---|
| `<tool> doctor --json` | `health` | none |
| `<tool> doctor capabilities --json` | `capabilities` | `contract`: the `operator.v1` document, identical to `capabilities --json` |
| `<tool> doctor triage --json` | `triage` | `next_actions` (non-empty when unhealthy) |

Fields: `schema_version` (`cmdrvl.doctor.v1`), `mode`, `tool`, `tool_version`, `healthy`, `findings[] {id, severity: info|warning|error, message, fix?}`. `healthy` must equal "no `error` finding". Exit codes: `0` healthy, `1` unhealthy, `2` CLI/usage error only; a parsed unhealthy report is never exit `2`. Every tool with `doctor` declares exit `1` in `exit_codes` (see `docs/exit-codes.md`).

### Migrating from today's spellings

The canonical invocations above replace `--robot-triage` and the tool-specific `doctor capabilities` output. They are **additive**: a tool adopts the new spellings and keeps every old spelling working as an alias for at least one full release (old and new emit the same `cmdrvl.doctor.v1` document where the old form was already JSON), so scripts and agents that call `<tool> --robot-triage` or the old `doctor capabilities --json` do not break during fan-out. The new spelling is the one documented in `operator.json`, `robot-docs` and this README; an alias is listed in the command's `description` as deprecated. Aliases are removed no earlier than the release after the one that introduced the canonical form, announced in `CHANGELOG.md`. Old per-tool schema ids (`<tool>.doctor.v1`, `*.doctor.{health,capabilities,triage}.v1`) are replaced by `cmdrvl.doctor.v1` in the same release; consumers should key on `schema_version`. The generated tests only exercise the canonical spellings, so aliases are not enforced here.

JSON Schema for both contracts ships next to the structs: `schemas/operator.v1.schema.json` and `schemas/cmdrvl.doctor.v1.schema.json` (`contract::OPERATOR_V1_SCHEMA`, `doctor::DOCTOR_V1_SCHEMA`). In both cases the Rust `validate()` is authoritative; the schemas express the subset JSON Schema can, and tests run planted documents through schema and `validate()` to keep them in step. In capabilities mode the embedded `contract` must equal `capabilities --json` (`family_doctor_capabilities_conforms` runs both and compares).

## Family conventions

- **Flag dictionary** (`conventions::lint_flags`): canonical `--out`, `--capsule-dir`, `--json`, `--explicit`, `--no-witness`. Legacy `--output` and `--capsule-out` may appear only as entries in an option's `aliases`, never as its primary `flag`.
- **ANSI-free output**: stdout and stderr contain no `ESC [` when piped, with `NO_COLOR=1 TERM=dumb` and with neither variable set.
- **`SOURCE_DATE_EPOCH`**: opt in with `fixture_timestamped_args`; two runs 1.1 s apart under the same value must produce identical stdout.
- **Bare invocation**: no args, stdin `/dev/null`, output captured, must exit within 10 s.

Generated tests: `family_doctor_{health,capabilities,triage}_conforms`, `family_flags_follow_dictionary`, `family_no_ansi_when_piped`, `family_bare_invocation_does_not_hang`, `family_source_date_epoch_deterministic` (opt-in).

## Exit codes

`0` positive, `1` negative, `expected_change` or `unhealthy` (always declared: every tool has `doctor`), `2` error (refusal / CLI error), `3` `policy` (valid verdict distinct from refusal, e.g. `assess` BLOCK). Diff-style tools whose `1` is expected must offer `--exit-zero-on-change`; `--exit-zero-on-change` never affects `doctor`; read/query commands exit `0` and print `[]` on an empty result; the canonical `assess` band table is in the ADR; `doctor` exits `1` when unhealthy. Full contract and migration notes: [`docs/exit-codes.md`](docs/exit-codes.md). Opt-in test: `fixture_empty_query_args` -> `family_empty_query_exits_zero`. Opt-in witness-read test: `fixture_witness_{query,count,last}_args` + `fixture_witness_invalid_line` (`InvalidLine::Skip` or `Refuse`, required, no default) -> `family_witness_read_conformance` (absent/empty ledger = exit 0 with `[]` / `{"count":0}` / `null`; directory or mode-000 ledger = always exit 2, empty stdout, ledger named on stderr; an invalid-UTF-8 line follows the declaration; 5 s kill per child).

## Three layers, one-way dependencies

The Evidence Protocol is split across three layers. [`evidence-protocol`](https://github.com/cmdrvl/evidence-protocol) (public) owns the `*.v0` wire types, schemas and fixtures. `spine-rules` (this crate, public) owns the conformance rules and `operator.v0`, and it only consumes `evidence-protocol` (`src/protocol.rs`; a test fails if this crate declares a protocol wire type). `evidence-machine` (private) is the runtime and depends on both. Dependencies only point one way: `evidence-machine` -> `spine-rules` -> `evidence-protocol`, and nothing public may depend on `evidence-machine`. `deny.toml` enforces the ban and a negative test proves it (a manifest that adds `evidence-machine` fails `cargo deny check bans`); [`docs/deny-template.toml`](docs/deny-template.toml) is the same core for operator repos. Protocol rules so far: `check_answer_coverage` (`answer.v0` coverage reconciles, for the `em-36x` sealer); rules are added only as consumers appear.

## Zero-retention conformance (`assert_no_raw_cell_leak`)

Any tool that reads cell data and may run under `veil` must print no raw cell values unless `--explicit` is passed. This is opt-in per tool, so existing suites are unchanged. Add a fixture whose cells hold canary strings and pass both fields (last in the macro):

```rust
golden_rules_suite! {
    // ...existing fields...
    fixture_canary_args: &["run", "rules.json", "--bind", "input=tests/fixtures/canary.csv", "--json", "--no-witness"],
    fixture_canaries: &["CANARY-LOAN-0f3a91", "CANARY-BAL-77c2d4"],
}
```

This generates `family_no_raw_cell_leak_by_default` (no canary in stdout or stderr of that run, `--explicit` absent from the args, and `operator.json` declares `options[].flag = "--explicit"` so `veil` can ban it by name) and `family_explicit_reveals_canaries` (a control: with `--explicit` at least one canary does appear, so a fixture the tool never prints cannot pass for the wrong reason). A run that prints nothing, or a canary shorter than 8 bytes, fails.

What it proves: none of those strings appear on either stream of that one invocation. What it does not prove: that other commands or failure paths are clean, that a hashed or truncated form of a low-entropy value is unguessable, or that column names are safe (list them as canaries to check them). Use a failing or refusing invocation as `fixture_canary_args`, since failure output is where leaks hide.

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
| `family_witness_read_conformance` | exit-codes ADR | If `fixture_witness_query_args` + `fixture_witness_count_args` + `fixture_witness_last_args` + `fixture_witness_invalid_line` set |

## ast-grep rules

Copy `rules/` to your tool and add `sgconfig.yml`:

```yaml
ruleDirs:
  - rules
```

| Rule | Checks |
|------|--------|
| `exit-code-range.yml` | `process::exit()` outside 0/1/2/3 |
| `exit-code-policy-band.yml` | `process::exit(3)` (warning: policy verdicts only) |
| `no-hashmap-in-output.yml` | `HashMap::new()` in output code |
| `witness-must-append.yml` | `File::create()` in witness code |

## License

MIT
