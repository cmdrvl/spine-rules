/// Generate a golden-rules test suite for a spine tool.
///
/// # Required fields
///
/// - `binary`: path to the compiled binary (use `env!("CARGO_BIN_EXE_<name>")`)
/// - `operator_json`: the operator.json content (use `include_str!("../operator.json")`)
/// - `source_files`: slice of source paths to scan for E_ codes
///
/// # Optional fields
///
/// - `fixture_success_args`: args that produce a successful (exit 0) run
/// - `fixture_refusal_args`: args that produce a refusal (exit 2) run
/// - `fixture_csv`: path to a CSV input file for row-count test
/// - `fixture_csv_args`: args for the CSV row-count run (must accompany `fixture_csv`)
/// - `exit_code_1_required`: set to `false` for tools without a negative domain (default: true)
/// - `fixture_empty_query_args`: a valid read/query that matches nothing (must exit 0)
/// - `fixture_canary_args` + `fixture_canaries` (together, last): OPT IN to the zero-retention
///   check. `fixture_canary_args` runs the tool, WITHOUT `--explicit`, on a fixture whose
///   cells hold the strings in `fixture_canaries`; none may appear in stdout or stderr.
///   A control run with `--explicit` must show at least one, and `operator.json` must
///   declare `--explicit`. Tools without this pair are unaffected. See [`crate::redaction`].
/// - `fixture_witness_query_args` + `fixture_witness_count_args` + `fixture_witness_last_args`
///   (together, last): OPT IN to witness-read conformance. Each is the tool's own args list for
///   that command (e.g. `&["witness", "query", "--json"]`). The suite runs them with
///   `EPISTEMIC_WITNESS` pointing at an absent, an empty and an invalid-UTF-8 ledger (exit 0 with
///   `[]` / `{"count":0}` / `null`) and at a directory and a mode-000 file (exit 2, empty stdout,
///   stderr names the path), each under a 5 s kill. Tools without these keys are unaffected.
///   See [`crate::witness_read`].
///
/// # Example
///
/// ```ignore
/// use spine_rules::golden_rules_suite;
///
/// golden_rules_suite! {
///     binary: env!("CARGO_BIN_EXE_canon"),
///     operator_json: include_str!("../operator.json"),
///     source_files: &["src/lib.rs", "src/refusal.rs"],
///     fixture_success_args: &["tests/fixtures/inputs/all_resolved.csv", "--no-witness"],
///     fixture_refusal_args: &["nonexistent.csv", "--no-witness"],
///     fixture_csv: "tests/fixtures/inputs/partial.csv",
///     fixture_csv_args: &["tests/fixtures/inputs/partial.csv", "--emit", "csv", "--no-witness"],
/// }
/// ```
#[macro_export]
macro_rules! golden_rules_suite {
    (
        binary: $binary:expr,
        operator_json: $operator_json:expr,
        source_files: $source_files:expr,
        $(fixture_success_args: $success_args:expr,)?
        $(fixture_refusal_args: $refusal_args:expr,)?
        $(fixture_csv: $csv_path:expr, fixture_csv_args: $csv_args:expr,)?
        $(exit_code_1_required: $require_1:expr,)?
        $(required_subcommands: $required_subs:expr,)?
        $(fixture_timestamped_args: $timestamped_args:expr,)?
        $(fixture_empty_query_args: $empty_query_args:expr,)?
        $(fixture_canary_args: $canary_args:expr, fixture_canaries: $canaries:expr,)?
        $(fixture_witness_query_args: $wq_args:expr, fixture_witness_count_args: $wc_args:expr, fixture_witness_last_args: $wl_args:expr,)?
    ) => {
        // ── Zero-retention: no raw cell values without --explicit (opt-in) ──
        $(
            #[test]
            fn family_no_raw_cell_leak_by_default() {
                $crate::redaction::assert_explicit_declared($operator_json);
                $crate::redaction::assert_no_raw_cell_leak($binary, $canary_args, $canaries);
            }

            #[test]
            fn family_explicit_reveals_canaries() {
                $crate::redaction::assert_canaries_visible_with_explicit(
                    $binary,
                    $canary_args,
                    $canaries,
                );
            }
        )?

        // ── Witness-read conformance (opt-in): absent/empty/invalid-UTF-8 ledgers read as empty
        // (exit 0), a directory or mode-000 ledger is a refusal (exit 2), nothing hangs ──────
        $(
            #[test]
            fn family_witness_read_conformance() {
                $crate::witness_read::assert_witness_read_conformance(
                    $binary,
                    $crate::witness_read::WitnessReadArgs {
                        query: $wq_args,
                        count: $wc_args,
                        last: $wl_args,
                    },
                );
            }
        )?

        // ── Empty read/query result is exit 0, never 1 ──────────────────
        $(
            #[test]
            fn family_empty_query_exits_zero() {
                $crate::exit_codes::assert_empty_query_exits_zero($binary, $empty_query_args);
            }
        )?

        // ── Family doctor contract (cmdrvl.doctor.v1) ───────────────────
        #[test]
        fn family_doctor_health_conforms() {
            $crate::doctor::assert_doctor_contract($binary, $crate::doctor::DoctorMode::Health);
        }

        #[test]
        fn family_doctor_capabilities_conforms() {
            $crate::doctor::assert_doctor_contract(
                $binary,
                $crate::doctor::DoctorMode::Capabilities,
            );
        }

        #[test]
        fn family_doctor_triage_conforms() {
            $crate::doctor::assert_doctor_contract($binary, $crate::doctor::DoctorMode::Triage);
        }

        // ── Family conventions ──────────────────────────────────────────
        #[test]
        fn family_flags_follow_dictionary() {
            $crate::conventions::assert_canonical_flags($operator_json);
        }

        #[test]
        fn family_no_ansi_when_piped() {
            $crate::conventions::assert_no_ansi_when_piped($binary, &["--describe"]);
            $( $crate::conventions::assert_no_ansi_when_piped($binary, $success_args); )?
            $( $crate::conventions::assert_no_ansi_when_piped($binary, $refusal_args); )?
        }

        #[test]
        fn family_bare_invocation_does_not_hang() {
            $crate::conventions::assert_bare_invocation_does_not_hang($binary);
        }

        $(
            #[test]
            fn family_source_date_epoch_deterministic() {
                $crate::conventions::assert_source_date_epoch_deterministic(
                    $binary,
                    $timestamped_args,
                );
            }
        )?

        // ── Family contract: version + subcommand parity ────────────────
        #[test]
        fn r006_operator_json_is_valid_operator_v1() {
            $crate::contract::assert_operator_valid($operator_json);
        }

        #[test]
        fn r006_operator_version_matches_cargo_toml() {
            $crate::contract::assert_version_parity(
                $operator_json,
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")),
            );
        }

        #[test]
        fn r006_operator_declares_required_subcommands() {
            let required: &[&str] = $crate::contract::REQUIRED_SUBCOMMANDS
                $( ; let _ = required; let required: &[&str] = $required_subs )?;
            $crate::contract::assert_required_subcommands($operator_json, required);
        }

        #[test]
        fn r006_capabilities_json_matches_describe() {
            $crate::contract::assert_capabilities_match_describe($binary);
        }

        // ── R-002 / R-007: Exit-code trinity ────────────────────────────
        #[test]
        fn r002_r007_exit_codes_trinity() {
            let require_1: bool = true $( ; let _ = require_1; let require_1: bool = $require_1)?;
            $crate::helpers::assert_exit_codes_trinity($operator_json, require_1);
        }

        // ── R-007: Refusal codes match source ───────────────────────────
        #[test]
        fn r007_refusal_codes_match_source() {
            $crate::helpers::assert_refusal_codes_match_source(
                $operator_json,
                $source_files,
                env!("CARGO_MANIFEST_DIR"),
            );
        }

        // ── R-006: Introspection flags ──────────────────────────────────
        #[test]
        fn r006_describe_valid() {
            $crate::helpers::assert_describe_valid($binary);
        }

        #[test]
        fn r006_schema_valid() {
            $crate::helpers::assert_schema_valid($binary);
        }

        #[test]
        fn r006_version_exits_zero() {
            $crate::helpers::assert_version_exits_zero($binary);
        }

        // ── R-001: Version in output (optional) ─────────────────────────
        $(
            #[test]
            fn r001_resolved_has_version() {
                $crate::helpers::assert_output_has_version($binary, $success_args);
            }

            #[test]
            fn r008_no_witness_suppresses() {
                $crate::helpers::assert_no_witness_suppresses_ledger($binary, $success_args);
            }

            #[test]
            fn r017_witness_failure_nonfatal() {
                $crate::helpers::assert_witness_failure_nonfatal($binary, $success_args);
            }
        )?

        $(
            #[test]
            fn r001_refusal_has_version() {
                $crate::helpers::assert_refusal_has_version($binary, $refusal_args);
            }
        )?

        // ── R-021: CSV row count (optional) ─────────────────────────────
        $(
            #[test]
            fn r021_csv_row_count_preserved() {
                let csv_path = concat!(env!("CARGO_MANIFEST_DIR"), "/", $csv_path);
                $crate::helpers::assert_csv_row_count_preserved($binary, csv_path, $csv_args);
            }
        )?
    };
}
