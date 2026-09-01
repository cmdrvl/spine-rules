# AGENTS.md — spine-rules

## Project Overview

**spine-rules** is the shared golden-rules enforcement crate for CMD+RVL spine tools. It provides:

1. A `golden_rules_suite!` macro that generates integration tests from minimal config
2. Helper assertion functions enforcing GOLDEN_RULES.md invariants (R-001 through R-022)
3. Serde types for the `operator.v0` manifest schema
4. ast-grep rules for static analysis (exit codes, HashMap, witness append)

## Repository Role

**Shared test infrastructure.** This crate is consumed as a `[dev-dependency]` by every active spine tool. One rule update here propagates to all tools on their next `cargo test`.

## Architecture

```
src/
  lib.rs       — re-exports
  composite_key.rs — ordered-tuple fixtures and consumer conformance assertions
  macros.rs    — golden_rules_suite! (macro_rules!, not proc_macro)
  helpers.rs   — assertion functions (the real logic)
  types.rs     — OperatorManifest serde types
rules/         — ast-grep YAML rules (copied to consuming tools)
tests/
  smoke.rs     — self-tests for types and helpers
```

## Critical Rules

1. **NEVER delete files or directories without explicit permission** (RULE 1)
2. **NEVER run**: `git push --force`, `git reset --hard`, `git checkout .`, `git clean -f`
3. **macro_rules! only** — no proc_macro dependency (no syn/quote)
4. **serde + serde_json only** — keep the dependency footprint minimal
5. **All maps use BTreeMap** — deterministic serialization (R-005)
6. **Logic lives in helpers.rs** — the macro is a thin expansion layer

## Quality Gate

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Adding a New Rule

1. Add the assertion function to `src/helpers.rs`
2. Add the macro expansion to `src/macros.rs`
3. Add a smoke test to `tests/smoke.rs`
4. Bump the version in `Cargo.toml`
5. Tag and push — all consuming tools pick it up on next build

## Consumer Integration

Each spine tool adds ~20 lines:

```toml
# Cargo.toml
[dev-dependencies]
spine-rules = { git = "https://github.com/cmdrvl/spine-rules" }
```

```rust
// tests/golden_rules.rs
use spine_rules::golden_rules_suite;

golden_rules_suite! {
    binary: env!("CARGO_BIN_EXE_<tool>"),
    operator_json: include_str!("../operator.json"),
    source_files: &["src/lib.rs"],
}
```
