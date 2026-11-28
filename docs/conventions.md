# Shared conventions

## Authored-input self-discovery

An authored input is a file whose contents are written by an operator or agent and then passed
to a spine command: rules, policies, assertions, profiles, fingerprints, or similar. A command
that accepts one must make the format discoverable without reading source code.

Its `operator.v1` command entry lists each authored input with:

- `name`, a stable label;
- `schema_flag`, either `--input-schema` or a documented subcommand `--schema`;
- a non-empty `example` that can be copied and adapted; and
- `allowed_values`, or an explicit empty list when that input has no enum.

For every entry, all of these metadata routes are zero-argument with respect to the authored
file: `<tool> [subcmd] --input-schema` (or the declared `--schema`) and
`<tool> [subcmd] --describe` exit `0` without a positional path, `--bind`, or `--lock`.
The subcommand's `--help` and standalone `robot-docs guide` repeat the example and the allowed
values. When there is no enum, the text must still say `allowed values` and `none`; an empty
example is never evidence of conformance.

Consumers opt into the generated `family_authored_input_self_discovery` test by passing
`authored_input_guide_args` to `golden_rules_suite!`. The helper runs each child under a five
second timeout and sets scratch `HOME` and `EPISTEMIC_WITNESS`, so metadata checks cannot append
to the operator's real witness ledger.
