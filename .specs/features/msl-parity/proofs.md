# msl-parity proofs

**HEAD**: bd4c1ed8
**Tree**: clean
**Shell**: /bin/sh
**Result**: 41 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run twins / test the_corpus_holds_the_library_and_the_twins ... ok` | pass |
| C2 | `run twins / test every_twin_builds_without_diagnostics ... ok` | pass |
| C3 | `run twins / test top_nodes_carry_the_cir_names ... ok` | pass |
| C4 | `run twins / test the_cir_fixtures_are_byte_identical ... ok` | pass |
| C5 | `run bench / test a_twin_loads_into_a_bound_circuit ... ok` | pass |
| C6 | `run bench / test a_failing_stage_is_named_with_its_diagnostics ... ok` | pass |
| C7 | `run bench / test nodes_and_sources_are_reached_by_cir_names ... ok` | pass |
| C8 | `run harness / test ngspice_runs_in_batch_in_an_empty_scratch_directory ... ok` | pass |
| C9 | `run harness / test each_run_has_its_own_scratch_removed_on_drop ... ok` | pass |
| C10 | `run harness / test a_failed_run_or_missing_export_names_the_fixture ... ok` | pass |
| C11 | `run lib / test golden::tests::an_unparsable_row_names_file_and_row ... ok` | pass |
| C12 | `run lib / test golden::tests::an_export_is_found_with_or_without_data_suffix ... ok` | pass |
| C13 | `run lib / test contract::tests::a_rejection_names_the_ngspice_version ... ok` | pass |
| C14 | `run lib / test ngspice::tests::the_binary_comes_from_the_variable_or_the_path ... ok` | pass |
| C15 | `run lib / test ngspice::tests::a_missing_binary_fails_naming_the_opt_out ... ok` | pass |
| C16 | `run lib / test ngspice::tests::skip_passes_and_prints_one_line ... ok` | pass |
| C17 | `run bench / test a_skipped_reference_still_solves_the_msl_side ... ok` | pass |
| C18 | `run lib / test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok` | pass |
| C19 | `run lib / test contract::tests::holds_is_abstol_plus_reltol_of_the_larger ... ok` | pass |
| C20 | `run lib / test contract::tests::a_rejection_names_point_values_bound_and_rationale ... ok` | pass |
| C21 | `run lib / test contract::tests::phase_is_gated_on_the_magnitude_floor ... ok` | pass |
| C22 | `run corpus / test no_parity_target_compares_against_a_literal ... ok` | pass |
| C23 | `run op / test divider ... ok` | pass |
| C24 | `run op / test diode_series ... ok` | pass |
| C25 | `run dc / test diode_iv ... ok` | pass |
| C26 | `run ac / test ac_rc ... ok` | pass |
| C27 | `run lib / test golden::tests::a_grid_mismatch_names_the_point ... ok` | pass |
| C28 | `run tran / test tran_rc ... ok` | pass |
| C29 | `run tran / test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok` | pass |
| C30 | `run corpus / test every_twin_has_a_cir_and_a_test ... ok` | pass |
| C31 | `run corpus / test a_literal_tolerance_in_a_parity_target_is_reported ... ok` | pass |
| C32 | `run corpus / test the_guards_report_planted_text ... ok` | pass |
| C33 | `run twins / test the_current_source_twin_exists ... ok` | pass |
| C34 | `run op / test isrc_r ... ok` | pass |
| C35 | `run lib / test contract::tests::two_zeros_are_accepted ... ok` | pass |
| C36 | `run lib / test golden::tests::a_wrong_row_count_names_both_counts ... ok` | pass |
| C37 | `run lib / test waveform::tests::a_time_outside_the_result_is_none ... ok` | pass |
| C38 | `run harness / test parallel_runs_do_not_share_a_scratch_directory ... ok` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `test ! -d crates/piperine-parity // cargo build -p piperine-parity --all-targets 2>&1 / tee /dev/stderr / (! g` | pass |

## Output

`Run twins` - exit 0

```
running 5 tests
test the_current_source_twin_exists ... ok
test the_corpus_holds_the_library_and_the_twins ... ok
test the_cir_fixtures_are_byte_identical ... ok
test every_twin_builds_without_diagnostics ... ok
test top_nodes_carry_the_cir_names ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/twins.rs (/var/tmp/piperine-target/debug/deps/twins-a814ee5aa6a9c3f3)
```

`Run bench` - exit 0

```
running 4 tests
test a_failing_stage_is_named_with_its_diagnostics ... ok
test nodes_and_sources_are_reached_by_cir_names ... ok
test a_twin_loads_into_a_bound_circuit ... ok
test a_skipped_reference_still_solves_the_msl_side ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/bench.rs (/var/tmp/piperine-target/debug/deps/bench-f1be51f3a87b18ae)
```

`Run harness` - exit 0

```
running 5 tests
test ngspice_runs_in_batch_in_an_empty_scratch_directory ... ok
test a_failed_run_or_missing_export_names_the_fixture ... ok
test parallel_runs_do_not_share_a_scratch_directory ... ok
test each_run_has_its_own_scratch_removed_on_drop ... ok
test the_exports_of_the_corpus_parse_to_their_row_counts ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/harness.rs (/var/tmp/piperine-target/debug/deps/harness-afa899d895b7538d)
```

`Run lib` - exit 0

```
test contract::tests::two_zeros_are_accepted ... ok
test golden::tests::a_wrong_row_count_names_both_counts ... ok
test golden::tests::a_grid_mismatch_names_the_point ... ok
test golden::tests::the_print_precision_is_read_from_the_token ... ok
test golden::tests::an_export_is_found_with_or_without_data_suffix ... ok
test ngspice::tests::skip_passes_and_prints_one_line ... ok
test golden::tests::an_unparsable_row_names_file_and_row ... ok
test ngspice::tests::a_missing_binary_fails_naming_the_opt_out ... ok
test waveform::tests::a_time_outside_the_result_is_none ... ok
test ngspice::tests::the_binary_comes_from_the_variable_or_the_path ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_parity-df705e4d73d3d345)
```

`Run corpus` - exit 0

```
running 4 tests
test a_literal_tolerance_in_a_parity_target_is_reported ... ok
test the_guards_report_planted_text ... ok
test no_parity_target_compares_against_a_literal ... ok
test every_twin_has_a_cir_and_a_test ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/parity_corpus.rs (/var/tmp/piperine-target/debug/deps/parity_corpus-28c57b8cbe4fe1a3)
```

`Run op` - exit 0

```
running 3 tests
test isrc_r ... ok
test diode_series ... ok
test divider ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/parity_op.rs (/var/tmp/piperine-target/debug/deps/parity_op-9c7317c9cf58954e)
```

`Run dc` - exit 0

```
running 1 test
test diode_iv ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/parity_dc.rs (/var/tmp/piperine-target/debug/deps/parity_dc-9b5533275bec3ee8)
```

`Run ac` - exit 0

```
running 1 test
test ac_rc ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/parity_ac.rs (/var/tmp/piperine-target/debug/deps/parity_ac-c8453cadbc9c63c5)
```

`Run tran` - exit 0

```
running 2 tests
test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok
test tran_rc ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/parity_tran.rs (/var/tmp/piperine-target/debug/deps/parity_tran-f7621f59e550f6d9)
```
