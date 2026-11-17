# long-run proofs

**HEAD**: 864a4111
**Tree**: clean
**Shell**: /bin/sh
**Result**: 71 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run step / test the_observer_is_called_once_per_accepted_step_in_time_order ... ok` | pass |
| C2 | `run step / test each_observed_step_holds_the_values_of_the_kept_step ... ok` | pass |
| C3 | `run step / test a_transient_observes_its_start_and_every_accepted_step ... ok` | pass |
| C4 | `run step / test a_resumed_transient_observes_its_start ... ok` | pass |
| C5 | `run step / test the_sink_has_every_observation_up_to_the_step_before_the_observer ... ok` | pass |
| C6 | `run step / test a_rejected_or_retaken_attempt_is_never_observed ... ok` | pass |
| C7 | `run step / test without_an_observer_the_result_and_the_sink_stream_are_unchanged ... ok` | pass |
| C8 | `run step / test keep_last_holds_only_the_last_step_with_its_reentry_state ... ok` | pass |
| C9 | `run step / test reentry_from_the_kept_step_equals_reentry_from_a_full_run ... ok` | pass |
| C10 | `run step / test the_default_keeps_every_accepted_step ... ok` | pass |
| C11 | `run grid / test a_gridless_transient_equals_the_table_of_a_run_that_kept_every_step ... ok` | pass |
| C12 | `run apilib / test recorder::tests::a_from_past_the_start_drops_the_start_row ... ok` | pass |
| C13 | `run grid / test a_printed_signal_with_no_value_fails_before_the_first_step ... ok` | pass |
| C14 | `run apilib / test session::tests::a_transient_keeps_only_the_last_step ... ok` | pass |
| C15 | `run memory / test a_run_ten_times_longer_at_equal_rows_holds_no_more_memory ... ok` | pass |
| C16 | `run apilib / test recorder::tests::grid_rows_run_from_the_first_multiple_not_before_from_to_stop ... ok` | pass |
| C17 | `run grid / test a_gridded_run_with_from_starts_at_the_first_multiple_not_before_it ... ok` | pass |
| C18 | `run apilib / test recorder::tests::a_real_column_is_bit_equal_to_on_grid ... ok` | pass |
| C19 | `run apilib / test recorder::tests::a_row_on_an_accepted_step_takes_its_value ... ok` | pass |
| C20 | `run apilib / test recorder::tests::a_discrete_column_holds_the_value_after_the_last_instant ... ok` | pass |
| C21 | `run apilib / test recorder::tests::each_grid_row_is_made_when_its_step_arrives ... ok` | pass |
| C22 | `run apilib / test recorder::tests::a_row_past_the_last_step_fails ... ok` | pass |
| C23 | `run run / test a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot ... ok` | pass |
| C24 | `run grid / test the_gridded_tran_rc_is_bit_equal_to_on_grid_on_both_backends ... ok` | pass |
| C25 | `run apilib / test recorder::tests::a_discrete_column_that_changes_only_at_steps_equals_on_grid ... ok` | pass |
| C26 | `run apilib / test recorder::tests::a_change_between_two_steps_reaches_the_rows_after_it ... ok` | pass |
| C27 | `run grid / test a_counter_that_changes_between_analog_steps_is_exact_on_the_grid ... ok` | pass |
| C28 | `run clilib / test every_run_key_has_its_flag ... ok` | pass |
| C29 | `run grid / test a_run_table_and_flags_give_equal_tables ... ok` | pass |
| C30 | `run bucklong / test the_whole_start_is_60001_rows_on_a_10_ns_grid ... ok` | pass |
| C31 | `run bucklong / test every_check_of_the_whole_start_passes ... ok` | pass |
| C32 | `run bucklong / test the_overshoot_is_within_15_mv ... ok` | pass |
| C33 | `run bucklong / test the_output_settles_into_10_mv_within_the_ramp_window ... ok` | pass |
| C34 | `run bucklong / test the_start_ends_in_regulation_with_power_good ... ok` | pass |
| C35 | `run bucklong / test the_ripple_is_within_10_mv_peak_to_peak ... ok` | pass |
| C36 | `run bucklong / test the_whole_start_stays_under_one_gibibyte ... ok` | pass |
| C37 | `run bucklong / test the_wall_time_and_the_peak_are_reported ... ok` | pass |
| C38 | `run buck / test startup_prints_the_whole_start_on_its_grid ... ok` | pass |
| C39 | `run buck / test piperine_test_passes_on_the_buck ... ok` | pass |
| C40 | `run twins / test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C41 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C42 | `run parity / test tran_rc ... ok` | pass |
| C43 | `run baseline / test parity_rc_transient ... ok` | pass |
| C44 | `git diff --quiet a5f23ea4 -- crates/piperine-parity/src crates/piperine-parity/tests/parity_op.rs crates/piper` | pass |
| C45 | `run paritygrid / tran_rc grid on rust: 100 rows matched` | pass |
| C46 | `run paritygrid / tran_rc on rust: 100 pairs` | pass |
| C47 | `run paritygrid / test a_skipped_reference_still_runs_the_twin ... ok` | pass |
| C48 | `run apilib / test verdict::tests::a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive ... ok` | pass |
| C49 | `run commands / test test_prints_pass_with_the_check_count ... ok` | pass |
| C50 | `run commands / test test_prints_fail_and_the_failing_records ... ok` | pass |
| C51 | `run apilib / test verdict::tests::a_record_of_another_stream_never_decides ... ok` | pass |
| C52 | `run options / test a_plot_of_a_gridded_run_draws_its_rows ... ok` | pass |
| C53 | `run options / test a_plot_of_a_gridless_run_draws_every_accepted_step ... ok` | pass |
| C54 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C55 | `run clibook / test the_buck_block_is_what_the_binary_prints ... ok` | pass |
| C56 | `run docs / test gaps_says_a_host_run_is_bounded ... ok` | pass |
| C57 | `run docs / test the_roadmap_lists_what_the_long_run_leaves ... ok` | pass |
| C58 | `run docs / test the_buck_readme_says_how_to_run_it ... ok` | pass |
| C59 | `run docs / test state_records_the_observation_decision ... ok` | pass |
| C60 | `run apilib / test recorder::tests::a_print_step_past_stop_gives_the_row_at_zero ... ok` | pass |
| C61 | `run apilib / test recorder::tests::a_row_within_rounding_of_the_last_step_takes_its_values ... ok` | pass |
| C62 | `run apilib / test recorder::tests::a_change_within_rounding_of_a_row_counts_at_it ... ok` | pass |
| C63 | `run apilib / test recorder::tests::an_unknown_net_value_is_an_unknown_cell ... ok` | pass |
| C64 | `run apilib / test recorder::tests::a_real_typed_net_is_held_not_interpolated ... ok` | pass |
| C65 | `run apilib / test recorder::tests::a_net_that_changed_unobserved_fails_naming_it ... ok` | pass |
| C66 | `run apilib / test recorder::tests::a_signal_named_twice_gives_two_equal_columns ... ok` | pass |
| C67 | `run analyses / test a_transient_has_one_row_per_accepted_step ... ok` | pass |
| C68 | `run probes / test a_probe_with_no_value_in_the_circuit_is_a_failure_naming_it ... ok` | pass |
| Build | `cargo build -p piperine-solver -p piperine-api -p piperine -p piperine-cli -p piperine-parity --all-targets 2>` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run step` - exit 0

```
test keep_last_refuses_device_state_recording ... ok
test the_default_keeps_every_accepted_step ... ok
test a_resumed_transient_observes_its_start ... ok
test reentry_from_the_kept_step_equals_reentry_from_a_full_run ... ok
test a_transient_observes_its_start_and_every_accepted_step ... ok
test without_an_observer_the_result_and_the_sink_stream_are_unchanged ... ok
test each_observed_step_holds_the_values_of_the_kept_step ... ok
test keep_last_holds_only_the_last_step_with_its_reentry_state ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-long-run/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-long-run/crates/piperine-solver)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.48s
     Running tests/step_observer.rs (/var/tmp/piperine-target/debug/deps/step_observer-ddb5cabba88084d6)
```

`Run grid` - exit 0

```
running 6 tests
test a_printed_signal_with_no_value_fails_before_the_first_step ... ok
test a_counter_that_changes_between_analog_steps_is_exact_on_the_grid ... ok
test a_run_table_and_flags_give_equal_tables ... ok
test a_gridded_run_with_from_starts_at_the_first_multiple_not_before_it ... ok
test the_gridded_tran_rc_is_bit_equal_to_on_grid_on_both_backends ... ok
test a_gridless_transient_equals_the_table_of_a_run_that_kept_every_step ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.36s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/grid.rs (/var/tmp/piperine-target/debug/deps/grid-106bf09cc70f69e6)
```

`Run apilib` - exit 0

```
test recorder::tests::a_real_column_is_bit_equal_to_on_grid ... ok
test request::tests::an_unknown_key_or_a_wrong_type_is_p0010_at_its_span ... ok
test verdict::tests::a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive ... ok
test session::tests::a_transient_keeps_only_the_last_step ... ok
test request::tests::a_run_table_reads_its_nineteen_keys ... ok
test verdict::tests::no_false_and_no_unknown_pass_is_a_pass ... ok
test verdict::tests::a_record_of_another_stream_never_decides ... ok
test request::tests::a_dc_sweep_needs_its_four_keys_and_a_step_toward_to ... ok
test request::tests::a_transient_needs_a_positive_stop ... ok
test verdict::tests::an_unknown_pass_fails ... ok

test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_api-f46f164108a134e4)
```

`Run memory` - exit 0

```
running 1 test
test a_run_ten_times_longer_at_equal_rows_holds_no_more_memory ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.50s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/memory.rs (/var/tmp/piperine-target/debug/deps/memory-48399b974c97c1ee)
```

`Run run` - exit 0

```
test a_lone_file_run_is_named_by_its_stem ... ok
test a_completed_run_exits_0 ... ok
test run_prints_header_records_blank_line_and_table ... ok
test records_come_before_the_table ... ok
test a_closed_stdout_ends_with_exit_1_and_no_panic ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test a_first_compile_is_announced_on_stderr ... ok
test records_stay_when_the_analysis_fails ... ok
test a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.14s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target/debug/deps/run-8bc9482a472d21be)
```

`Run clilib` - exit 0

```
test render::number::tests::numbers_have_one_text_each ... ok
test render::json::tests::each_value_kind_has_its_json ... ok
test render::record::tests::a_record_is_one_line ... ok
test render::table::tests::a_table_is_left_aligned_two_spaces_no_trailing_space ... ok
test render::svg::tests::one_path_per_signal_with_its_name_and_a_legend ... ok
test render::svg::tests::each_axis_has_ticks_labels_and_a_title ... ok
test render::svg::tests::the_same_plot_is_the_same_bytes ... ok
test cli::tests::every_command_is_built ... ok
test cli::tests::every_run_key_has_its_flag ... ok
test render::diagnostic::tests::a_diagnostic_has_code_file_line_and_excerpt ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_cli-f58330f3a788d0cd)
```

`Run bucklong` - exit 0

```
test the_whole_start_is_60001_rows_on_a_10_ns_grid has been running for over 60 seconds
test the_whole_start_stays_under_one_gibibyte has been running for over 60 seconds
test every_check_of_the_whole_start_passes ... ok
test the_wall_time_and_the_peak_are_reported ... ok
test the_whole_start_stays_under_one_gibibyte ... ok
test the_start_ends_in_regulation_with_power_good ... ok
test the_whole_start_is_60001_rows_on_a_10_ns_grid ... ok
test the_output_settles_into_10_mv_within_the_ramp_window ... ok
test the_ripple_is_within_10_mv_peak_to_peak ... ok
test the_overshoot_is_within_15_mv ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 135.19s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/buck_long.rs (/var/tmp/piperine-target/debug/deps/buck_long-c09affaf76231d2d)
```

`Run buck` - exit 0

```
test piperine_test_passes_on_the_buck has been running for over 60 seconds
test startup_ends_with_the_reference_at_128 has been running for over 60 seconds
test startup_prints_no_failed_check has been running for over 60 seconds
test startup_prints_the_whole_start_on_its_grid has been running for over 60 seconds
test the_buck_runs_print_at_most_16_lines has been running for over 60 seconds
test the_buck_runs_print_at_most_16_lines ... ok
test startup_ends_with_the_reference_at_128 ... ok
test startup_prints_the_whole_start_on_its_grid ... ok
test piperine_test_passes_on_the_buck ... ok
test startup_prints_no_failed_check ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 212.83s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/buck.rs (/var/tmp/piperine-target/debug/deps/buck-a661d56f54666ee6)
```

`Run twins` - exit 0

```
running 5 tests
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.87s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/twins.rs (/var/tmp/piperine-target/debug/deps/twins-2aa9f00922f0fbd5)
```

`Run parity` - exit 0

```
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.13s


running 2 tests
test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok
test tran_rc ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/parity_ac.rs (/var/tmp/piperine-target/debug/deps/parity_ac-d6b9b1cec2f0e864)
     Running tests/parity_corpus.rs (/var/tmp/piperine-target/debug/deps/parity_corpus-1ebffe6723db0cf5)
     Running tests/parity_dc.rs (/var/tmp/piperine-target/debug/deps/parity_dc-35b4b79f0c30c5ec)
     Running tests/parity_op.rs (/var/tmp/piperine-target/debug/deps/parity_op-7b0f742ce655e3bf)
     Running tests/parity_tran.rs (/var/tmp/piperine-target/debug/deps/parity_tran-3b5361e3d29361a3)
```

`Run baseline` - exit 0

```
running 7 tests
test its_devices_pass_the_partials_verifier ... ok
test parity_digital_scheduler_snapshot ... ok
test parity_diode_dc_point ... ok
test parity_divider_op_dc ... ok
test parity_mixed_signal_dc_settle ... ok
test parity_rlc_ac_resonant_peak ... ok
test parity_rc_transient ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-long-run/crates/piperine-solver)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.48s
     Running tests/parity_baseline.rs (/var/tmp/piperine-target/debug/deps/parity_baseline-0f0d3a20498e83a0)
```

`Run paritygrid` - exit 0

```
running 3 tests
SKIP tran_rc: ngspice comparison disabled by PIPERINE_NGSPICE=skip
tran_rc on interp: 100 pairs, largest |v(out)| deviation from ngspice 8.60249613218178e-5 V
tran_rc grid on interp: 100 rows matched
test a_skipped_reference_still_runs_the_twin ... ok
tran_rc grid on rust: 100 rows matched
test the_grid_of_tran_rc_has_a_row_at_every_time_ngspice_exports ... ok
tran_rc on rust: 100 pairs, largest |v(out)| deviation from ngspice 8.60249613218178e-5 V
test tran_rc_on_the_grid_agrees_with_ngspice ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.47s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/parity_grid.rs (/var/tmp/piperine-target/debug/deps/parity_grid-2d6402ccc0b51c31)
```

`Run commands` - exit 0

```
test test_prints_no_table_and_no_other_record ... ok
test build_prints_one_line_per_run ... ok
test a_second_build_is_cached_and_compiles_nothing ... ok
test verbose_prints_a_source_path_that_exists ... ok
test a_new_project_checks_runs_and_tests ... ok
test test_prints_pass_with_the_check_count ... ok
test the_last_line_counts_and_the_exit_code_follows ... ok
test test_prints_fail_and_the_failing_records ... ok
test clean_cache_removes_the_compiled_cache ... ok
test test_runs_every_run_or_the_named_ones ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.48s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/commands.rs (/var/tmp/piperine-target/debug/deps/commands-e67a6e5d751dd843)
```

`Run options` - exit 0

```
test an_existing_records_file_is_replaced ... ok
test an_unwritable_plot_names_the_path_and_exits_1 ... ok
test an_existing_csv_is_replaced ... ok
test a_plot_of_a_gridless_run_draws_every_accepted_step ... ok
test a_plot_of_a_gridded_run_draws_its_rows ... ok
test csv_writes_the_table_and_prints_wrote ... ok
test a_passing_check_is_hidden_a_failing_one_shown_and_all_checks_shows_both ... ok
test the_divider_with_r2_at_1k_gives_5 ... ok
test a_relative_output_path_is_under_the_project ... ok
test the_stream_flag_keeps_only_that_stream ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.99s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/options.rs (/var/tmp/piperine-target/debug/deps/options-b939bfd130e3db2e)
```

`Run clibook` - exit 0

```
test the_chapter_shows_the_rc_project_file_for_file ... ok
test the_chapter_lists_commands_keys_flags_codes ... ok
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 119.99s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target/debug/deps/book-41913a260b16a2a6)
```

`Run docs` - exit 0

```
running 8 tests
test the_buck_readme_says_how_to_run_it ... ok
test gaps_drops_what_closed_and_lists_what_is_left ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test gaps_says_a_host_run_is_bounded ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test state_records_the_cli_decision ... ok
test state_records_the_observation_decision ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target/debug/deps/docs-7e405e0698d57fd4)
```

`Run analyses` - exit 0

```
test a_param_path_that_names_nothing_is_e0801 ... ok
test a_refused_value_is_e0802_and_nothing_runs ... ok
test the_rc_corner_is_minus_3_db_and_minus_45_degrees ... ok
test an_ac_sweep_has_mag_and_phase_in_degrees ... ok
test an_operating_point_is_signal_and_value_in_print_order ... ok
test the_diode_sweep_has_37_rows ... ok
test the_divider_mid_is_7_5_on_both_backends ... ok
test the_rc_step_follows_one_minus_exp_on_both_backends ... ok
test a_dc_sweep_has_one_row_per_point ... ok
test params_are_set_in_order_table_then_flags ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.71s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/analyses.rs (/var/tmp/piperine-target/debug/deps/analyses-b506f377e01c0549)
```

`Run probes` - exit 0

```
running 7 tests
test a_column_is_named_by_the_full_path ... ok
test a_signal_printed_twice_is_two_columns ... ok
test a_node_a_net_and_a_flow_resolve_relative_or_full ... ok
test an_entry_that_names_nothing_is_e0801_before_any_analysis ... ok
test the_default_is_the_tops_own_signals_in_order ... ok
test a_probe_with_no_value_in_the_circuit_is_a_failure_naming_it ... ok
test a_node_a_flow_and_a_net_resolve_to_their_columns ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.02s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/probes.rs (/var/tmp/piperine-target/debug/deps/probes-f0711161f95d64f9)
```

`git diff --quiet a5f23ea4 -- crates/piperine-parity/src crates/piperine-parity/tests/parity_op.rs crates/piper` - exit 0

```
(no output)
```
