# Family exit-code contract

Status: accepted. Enforced by `spine_rules::exit_codes`, `helpers::assert_exit_codes_trinity`
(`r002_r007_exit_codes_trinity`), `family_doctor_*`, `family_empty_query_exits_zero`, and the
ast-grep rules `exit-code-range.yml` / `exit-code-policy-band.yml`.

## Why this changed

The 0/1/2 trinity forced three different things into the same code, so an agent running
`case $?` could not tell them apart:

1. `assess` has four decision bands plus `REFUSAL`. A valid `BLOCK` verdict shared exit `2`
   with refusals and CLI typos.
2. `shape` exit `1` means "halt" (`INCOMPATIBLE`); `rvl` exit `1` means the normal, expected
   `REAL_CHANGE`. Under `set -e` the second silently aborts scripts.
3. `verify witness query` exited `1` on zero matches, conflating "ran fine, no rows" with a
   domain `FAIL`.

## The dictionary

| Code | `domain` in `operator.json` | Meaning |
|------|-----------------------------|---------|
| 0 | `positive` | Success / clean verdict. |
| 1 | `negative` | Domain verdict that should halt a pipeline (`shape` INCOMPATIBLE, `lock` verify FAIL). |
| 1 | `expected_change` | Normal, expected outcome of a diff-style tool (`rvl` REAL_CHANGE). Requires the `--exit-zero-on-change` option. |
| 2 | `error` | Refusal, CLI/usage error, or a tool that could not run. Never a verdict. |
| 3 | `policy` | A valid policy verdict distinct from refusal (`assess` BLOCK). Optional; only for multi-band tools. |

Anything else (4, 127, 255, ...) is a violation. `0` and `2` are mandatory; `1` and `3` are
optional (`vacuum` has no `1`). The family stays backward compatible: every tool that declares
only `positive`/`negative`/`error` on 0/1/2 today already conforms.

## Rules

- **Policy band.** Multi-band tools (`assess`) move `BLOCK` to exit `3` (domain `policy`) and
  keep `2` for `REFUSAL` and CLI errors. Intermediate bands (`PROCEED_WITH_RISK`, `ESCALATE`)
  keep their existing codes; the tool documents the mapping in `exit_codes[].meaning`. Migration:
  ship `3` in a minor version, list the old behavior under the code's `meaning`, and update
  `capabilities --json` (the same document as `--describe`) in the same release.
- **Expected change.** A tool whose exit `1` is an expected outcome declares it with domain
  `expected_change` and provides `--exit-zero-on-change`, which returns `0` for that outcome and
  leaves `2` for errors. In `set -e` scripts either pass that flag or guard the call
  (`rvl a.csv b.csv || [ $? -eq 1 ]`).
- **Empty results are success.** Read/query subcommands (`witness query`, `list`, `inspect`)
  never exit `1` for an empty result set: exit `0` and print `[]` (or the empty equivalent).
  Opt-in test: `fixture_empty_query_args`.
- **Doctor.** `doctor` exits `0` healthy, `1` unhealthy (an `error` finding), `2` usage errors
  only. See the doctor contract in the README.
- **Where the code comes from.** Use the tool's `ExitCode` enum or return from `run()`; do not
  hardcode `process::exit(n)`. Exit `3` via `process::exit` raises a lint warning and must match
  the manifest.

## Not covered

The ast-grep rules match `process::exit(n)` written exactly that way; a fully-qualified
`std::process::exit(n)` is not matched (pre-existing limitation). The manifest check is the
authoritative gate.
