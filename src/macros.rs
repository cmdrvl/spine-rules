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
/// - `fixture_csv_args`: args for the CSV row-count run
/// - `exit_code_1_required`: set to `false` for tools without a negative domain (default: true)
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
        $(fixture_csv: $csv_path:expr,)?
        $(fixture_csv_args: $csv_args:expr,)?
        $(exit_code_1_required: $require_1:expr,)?
    ) => {
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
                // We need csv_args — this block only expands if fixture_csv is set.
                // The csv_args must also be set when fixture_csv is set.
                golden_rules_suite!(@csv_test $binary, csv_path, $($csv_args)?);
            }
        )?
    };

    // Internal helper: expand CSV test with args.
    (@csv_test $binary:expr, $csv_path:expr, $csv_args:expr) => {
        $crate::helpers::assert_csv_row_count_preserved($binary, $csv_path, $csv_args);
    };
}
