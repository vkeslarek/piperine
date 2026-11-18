# host-ii proofs

**HEAD**: 72bb5e9d
**Tree**: clean
**Shell**: /bin/sh
**Result**: 105 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run apilib / test request::tests::option_keys_are_read_for_every_analysis ... ok` | pass |
| C2 | `run clilib / test cli::tests::every_run_key_has_its_flag ... ok` | pass |
| C3 | `run apilib / test request::tests::tolerance_keys_must_be_positive ... ok` | pass |
| C4 | `run apilib / test request::tests::temp_must_be_above_absolute_zero ... ok` | pass |
| C5 | `run apilib / test request::tests::bounds_must_be_whole_numbers_of_at_least_one ... ok` | pass |
| C6 | `run apioptions / test a_nodeset_naming_nothing_is_e0801 ... ok` | pass |
| C7 | `run apioptions / test no_option_gives_the_default_context ... ok` | pass |
| C8 | `run apioptions / test tolerance_keys_reach_the_context ... ok` | pass |
| C9 | `run apioptions / test temp_is_celsius_read_by_temperature ... ok` | pass |
| C10 | `run apioptions / test a_nodeset_seeds_the_operating_point_and_keeps_its_solution ... ok` | pass |
| C11 | `run apioptions / test resolution_rounds_an_at_to_its_tick ... ok` | pass |
| C12 | `run apioptions / test maxrounds_bounds_the_operating_point ... ok` | pass |
| C13 | `run apioptions / test maxmicro_bounds_an_instant ... ok` | pass |
| C14 | `run apioptions / test a_smaller_trtol_takes_more_steps ... ok` | pass |
| C14 | `run apioptions / test a_run_past_max_steps_fails_and_succeeds_when_raised ... ok` | pass |
| C15 | `run newparity / test temp_res ... ok` | pass |
| C16 | `run newparity / test temp_res_at_the_default_temperature ... ok` | pass |
| C17 | `run apilib / test table::tests::a_transfer_table_has_three_rows_in_order ... ok` | pass |
| C18 | `run newparity / test tf_divider ... ok` | pass |
| C19 | `run apilib / test request::tests::tf_and_pz_need_input_and_output ... ok` | pass |
| C20 | `run apianalyses / test tf_input_must_be_a_voltage_source ... ok` | pass |
| C21 | `run apianalyses / test tf_output_naming_nothing_is_e0801 ... ok` | pass |
| C22 | `run apilib / test table::tests::roots_list_poles_then_zeros ... ok` | pass |
| C23 | `run newparity / test pz_lowpass ... ok` | pass |
| C24 | `run apianalyses / test pz_of_a_circuit_without_poles_fails ... ok` | pass |
| C25 | `run apilib / test request::tests::tf_and_pz_need_input_and_output ... ok` | pass |
| C26 | `run apilib / test table::tests::a_sensitivity_table_has_a_row_per_wrt ... ok` | pass |
| C27 | `run newparity / test sens_divider ... ok` | pass |
| C28 | `run apilib / test request::tests::sens_needs_a_non_empty_wrt ... ok` | pass |
| C29 | `run apianalyses / test sens_wrt_naming_nothing_is_e0801 ... ok` | pass |
| C30 | `run apianalyses / test sens_of_a_discrete_net_is_refused ... ok` | pass |
| C31 | `run apianalyses / test pss_gives_one_period_of_rows ... ok` | pass |
| C32 | `run apianalyses / test pss_on_a_grid_gives_the_grid_rows ... ok` | pass |
| C33 | `run apianalyses / test pss_starts_its_orbit_at_tstab ... ok` | pass |
| C34 | `run newparity / test pss_rc ... ok` | pass |
| C35 | `run apilib / test request::tests::pss_needs_a_positive_period_and_a_non_negative_tstab ... ok` | pass |
| C36 | `run apianalyses / test pss_that_does_not_converge_fails ... ok` | pass |
| C37 | `run apianalyses / test a_pss_plot_draws_its_rows ... ok` | pass |
| C38 | `run clianalyses / test each_new_analysis_prints_header_records_and_table ... ok` | pass |
| C39 | `run clianalyses / test each_new_analysis_writes_csv ... ok` | pass |
| C40 | `run apilib / test request::tests::plot_is_refused_for_tf_pz_and_sens ... ok` | pass |
| C41 | `run apilib / test request::tests::the_eight_analysis_names ... ok` | pass |
| C42 | `run apimeta / test a_param_prints_its_value_in_every_row ... ok` | pass |
| C43 | `run apimeta / test the_flow_of_a_single_flow_contribution_prints ... ok` | pass |
| C44 | `run apimeta / test a_flow_of_several_contributions_is_e0801 ... ok` | pass |
| C45 | `run apimeta / test metadata_lists_every_param_with_type_value_where_and_structural ... ok` | pass |
| C46 | `run apimeta / test metadata_lists_ports_and_streams ... ok` | pass |
| C47 | `run apimeta / test metadata_runs_no_analysis ... ok` | pass |
| C48 | `run cliinfo / test info_prints_params_ports_and_streams ... ok` | pass |
| C49 | `run cliinfo / test info_of_a_broken_design_prints_diagnostics_and_exits_1 ... ok` | pass |
| C50 | `run apihost / test a_drive_at_zero_is_the_initial_value ... ok` | pass |
| C51 | `run apihost / test a_drive_later_fires_a_reaction_at_its_time ... ok` | pass |
| C52 | `run apihost / test driving_a_carrier_that_is_not_a_top_input_is_e0804 ... ok` | pass |
| C53 | `run apihost / test a_drive_of_the_wrong_type_is_e0802 ... ok` | pass |
| C54 | `run apihost / test an_undriven_top_input_is_unknown ... ok` | pass |
| C55 | `run apihost / test a_set_at_zero_applies_before_the_analysis ... ok` | pass |
| C56 | `run apihost / test a_timed_set_applies_after_the_step_to_its_time ... ok` | pass |
| C57 | `run apihost / test a_timed_set_of_a_structural_param_is_e0803 ... ok` | pass |
| C58 | `run apihost / test a_negative_at_is_e0806 ... ok` | pass |
| C59 | `run apihost / test a_timed_set_failing_where_is_e0802 ... ok` | pass |
| C60 | `run apihost / test subscribe_before_the_run_narrows_the_streams ... ok` | pass |
| C61 | `run apihost / test subscribe_after_the_run_is_e0805 ... ok` | pass |
| C62 | `run clirun / test a_fatal_run_prints_its_msg_and_exits_1 ... ok` | pass |
| C63 | `run clicommands / test test_prints_fail_fatal_with_the_msg ... ok` | pass |
| C64 | `run apilib / test verdict::tests::a_finish_record_does_not_fail_the_verdict ... ok` | pass |
| C65 | `run apilib / test verdict::tests::the_last_record_of_each_target_is_kept_in_first_seen_order ... ok` | pass |
| C66 | `run clicommands / test test_prints_target_margins_after_the_run_line ... ok` | pass |
| C67 | `run apilib / test verdict::tests::a_negative_margin_does_not_fail ... ok` | pass |
| C68 | `run apidigital / test a_digital_only_transient_reaches_its_stop ... ok` | pass |
| C69 | `run apidigital / test a_digital_only_grid_holds_net_values ... ok` | pass |
| C70 | `run apidigital / test a_digital_only_run_ended_by_finish_is_ok ... ok` | pass |
| C71 | `run clibook / test blinky_runs_to_its_stop_in_the_book ... ok` | pass |
| C72 | `run apilib / test params::tests::a_set_derives_its_children ... ok` | pass |
| C73 | `run apiderived / test a_sweep_recomputes_the_child_at_each_point ... ok` | pass |
| C74 | `run apiderived / test a_timed_set_recomputes_at_the_same_instant ... ok` | pass |
| C75 | `run apiderived / test a_child_failing_where_refuses_the_set ... ok` | pass |
| C76 | `run apiderived / test a_set_neither_elaborates_nor_compiles ... ok` | pass |
| C77 | `run apiderived / test the_s2_probe_prints_three_quarters ... ok` | pass |
| C78 | `run apiderived / test a_body_let_reads_the_new_param ... ok` | pass |
| C79 | `run apiderived / test a_definition_is_recomputed_at_the_set ... ok` | pass |
| C80 | `run apilib / test params::tests::a_derived_param_cannot_be_set_directly ... ok` | pass |
| C81 | `run apilib / test params::tests::the_interpreter_refuses_a_set_others_derive_from ... ok` | pass |
| C82 | `run apilib / test params::tests::no_set_gives_the_folded_constants ... ok` | pass |
| C83 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C84 | `run parity / test every_twin_has_a_cir_and_a_test ... ok` | pass |
| C85 | `run newparity / test temp_res ... ok` | pass |
| C86 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C87 | `run clibook / test the_chapter_runs_each_new_analysis ... ok` | pass |
| C88 | `run clibook / test blinky_runs_to_its_stop_in_the_book ... ok` | pass |
| C89 | `run docs / test gaps_drops_what_host_ii_closed ... ok` | pass |
| C90 | `run docs / test the_roadmap_lists_the_host_follow_ups ... ok` | pass |
| C91 | `run root / test the_root_runs_tf_a_drive_and_a_timed_set ... ok` | pass |
| C92 | `run solverbounds / test an_operating_point_past_max_rounds_fails_naming_the_bound ... ok` | pass |
| C93 | `run solverbounds / test an_instant_past_the_microstep_bound_fails_naming_it ... ok` | pass |
| C94 | `run planlib / test netlist::tests::with_microsteps_reaches_the_builder ... ok` | pass |
| C95 | `run solverhost / test write_net_before_a_run_is_an_initial_value ... ok` | pass |
| C96 | `run solverhost / test a_scheduled_write_commits_at_its_time ... ok` | pass |
| C97 | `run baseline / test parity_divider_op_dc ... ok` | pass |
| C98 | `run planlib / test derive::tests::a_chain_derives_top_down ... ok` | pass |
| C99 | `run transpile / test compiled_derive_evaluates_the_placement_expression ... ok` | pass |
| C100 | `run paritylib / test ngspice::tests::print_lines_parse_to_named_values ... ok` | pass |
| C101 | `run paritylib / test contract::tests::new_domains_carry_a_rationale ... ok` | pass |
| Build | `cargo build -p piperine-solver -p piperine-plan -p piperine-transpile -p piperine-api -p piperine -p piperine-` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run apilib` - exit 0

```
test verdict::tests::a_fatal_record_fails_the_verdict_with_its_message ... ok
test table::tests::continuous_is_interpolated_and_discrete_is_held ... ok
test verdict::tests::a_finish_record_does_not_fail_the_verdict ... ok
test verdict::tests::a_record_of_another_stream_never_decides ... ok
test verdict::tests::a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive ... ok
test request::tests::option_keys_are_read_for_every_analysis ... ok
test verdict::tests::an_unknown_pass_fails ... ok
test verdict::tests::the_last_record_of_each_target_is_kept_in_first_seen_order ... ok
test verdict::tests::no_false_and_no_unknown_pass_is_a_pass ... ok
test verdict::tests::a_negative_margin_does_not_fail ... ok

test result: ok. 69 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running unittests src/lib.rs (/var/tmp/piperine-target-b/debug/deps/piperine_api-267fdcea6096b5f4)
```

`Run clilib` - exit 0

```
test render::record::tests::a_record_is_one_line ... ok
test render::json::tests::each_value_kind_has_its_json ... ok
test render::svg::tests::one_path_per_signal_with_its_name_and_a_legend ... ok
test render::table::tests::a_table_is_left_aligned_two_spaces_no_trailing_space ... ok
test render::svg::tests::the_same_plot_is_the_same_bytes ... ok
test render::svg::tests::each_axis_has_ticks_labels_and_a_title ... ok
test cli::tests::a_nodeset_flag_without_equals_exits_2 ... ok
test cli::tests::every_run_key_has_its_flag ... ok
test cli::tests::every_command_is_built ... ok
test render::diagnostic::tests::a_diagnostic_has_code_file_line_and_excerpt ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running unittests src/lib.rs (/var/tmp/piperine-target-b/debug/deps/piperine_cli-5f6cb87a0dbf3b47)
```

`Run apioptions` - exit 0

```
test no_option_gives_the_default_context ... ok
test a_nodeset_on_a_run_that_takes_its_own_operating_point_is_p0010 ... ok
test tolerance_keys_reach_the_context ... ok
test temp_is_celsius_read_by_temperature ... ok
test a_nodeset_seeds_the_operating_point_and_keeps_its_solution ... ok
test a_smaller_trtol_takes_more_steps ... ok
test a_run_past_max_steps_fails_and_succeeds_when_raised ... ok
test maxrounds_bounds_the_operating_point ... ok
test resolution_rounds_an_at_to_its_tick ... ok
test maxmicro_bounds_an_instant ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/options.rs (/var/tmp/piperine-target-b/debug/deps/options-f903258bfef75340)
```

`Run newparity` - exit 0

```
     Running tests/parity_sens.rs (/var/tmp/piperine-target-b/debug/deps/parity_sens-b1015a1290493991)
sens_divider d v(mid) / d r1.r on interp: ngspice -1.874998125e-3, piperine -1.874999999262883e-3, deviation 1.874262882915756e-9
sens_divider d v(mid) / d r2.r on interp: ngspice 6.2499937497e-4, piperine 6.24999999902324e-4, deviation 6.249323239882615e-10
sens_divider d v(mid) / d v1.dc on interp: ngspice 7.4999999997e-1, piperine 7.499999999716068e-1, deviation 1.606825783539989e-12
sens_divider d v(mid) / d r1.r on rust: ngspice -1.874998125e-3, piperine -1.874999999262883e-3, deviation 1.874262882915756e-9
sens_divider d v(mid) / d r2.r on rust: ngspice 6.2499937497e-4, piperine 6.24999999902324e-4, deviation 6.249323239882615e-10
sens_divider d v(mid) / d v1.dc on rust: ngspice 7.4999999997e-1, piperine 7.499999999716068e-1, deviation 1.606825783539989e-12
     Running tests/parity_temp.rs (/var/tmp/piperine-target-b/debug/deps/parity_temp-e786937b3b76f9db)
     Running tests/parity_tf.rs (/var/tmp/piperine-target-b/debug/deps/parity_tf-c887934fe12acf99)
tf_divider gain on interp: ngspice 7.5e-1, piperine 7.5e-1, deviation 0e0
tf_divider output_resistance on interp: ngspice 7.5e2, piperine 7.5e2, deviation 0e0
tf_divider input_resistance on interp: ngspice 4e3, piperine 4e3, deviation 0e0
tf_divider gain on rust: ngspice 7.5e-1, piperine 7.5e-1, deviation 0e0
tf_divider output_resistance on rust: ngspice 7.5e2, piperine 7.5e2, deviation 0e0
tf_divider input_resistance on rust: ngspice 4e3, piperine 4e3, deviation 0e0
```

`Run apianalyses` - exit 0

```
test the_divider_mid_is_7_5_on_both_backends ... ok
test the_rc_corner_is_minus_3_db_and_minus_45_degrees ... ok
test pss_that_does_not_converge_fails ... ok
test pss_on_a_grid_gives_the_grid_rows ... ok
test pss_gives_one_period_of_rows ... ok
test the_diode_sweep_has_37_rows ... ok
test a_pss_plot_draws_its_rows ... ok
test the_rc_step_follows_one_minus_exp_on_both_backends ... ok
test params_are_set_in_order_table_then_flags ... ok
test pss_starts_its_orbit_at_tstab ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.28s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/analyses.rs (/var/tmp/piperine-target-b/debug/deps/analyses-cd3a194175d709f6)
```

`Run clianalyses` - exit 0

```
running 3 tests
test the_new_keys_have_flags ... ok
test each_new_analysis_writes_csv ... ok
test each_new_analysis_prints_header_records_and_table ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.55s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/analyses.rs (/var/tmp/piperine-target-b/debug/deps/analyses-1e3a1aa1ac242762)
```

`Run apimeta` - exit 0

```
running 6 tests
test a_flow_of_several_contributions_is_e0801 ... ok
test metadata_lists_every_param_with_type_value_where_and_structural ... ok
test metadata_runs_no_analysis ... ok
test the_flow_of_a_single_flow_contribution_prints ... ok
test a_param_prints_its_value_in_every_row ... ok
test metadata_lists_ports_and_streams ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.67s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/metadata.rs (/var/tmp/piperine-target-b/debug/deps/metadata-1de86750d8a04096)
```

`Run cliinfo` - exit 0

```
running 2 tests
test info_prints_params_ports_and_streams ... ok
test info_of_a_broken_design_prints_diagnostics_and_exits_1 ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s

    Finished `test` profile [unoptimized] target(s) in 0.24s
     Running tests/info.rs (/var/tmp/piperine-target-b/debug/deps/info-0a2dc3be4b6f6fb6)
```

`Run apihost` - exit 0

```
test a_timed_set_applies_after_the_step_to_its_time ... ok
test a_drive_of_the_wrong_type_is_e0802 ... ok
test a_negative_at_is_e0806 ... ok
test subscribe_after_the_run_is_e0805 ... ok
test a_set_at_zero_applies_before_the_analysis ... ok
test a_timed_set_of_a_structural_param_is_e0803 ... ok
test a_drive_later_fires_a_reaction_at_its_time ... ok
test driving_a_carrier_that_is_not_a_top_input_is_e0804 ... ok
test a_drive_at_zero_is_the_initial_value ... ok
test subscribe_before_the_run_narrows_the_streams ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.91s

    Finished `test` profile [unoptimized] target(s) in 0.29s
     Running tests/host_actions.rs (/var/tmp/piperine-target-b/debug/deps/host_actions-18b27e35a5135d49)
```

`Run clirun` - exit 0

```
test the_project_flag_uses_that_directory_and_never_searches ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test run_prints_header_records_blank_line_and_table ... ok
test a_completed_run_exits_0 ... ok
test a_closed_stdout_ends_with_exit_1_and_no_panic ... ok
test records_stay_when_the_analysis_fails ... ok
test a_first_compile_is_announced_on_stderr ... ok
test a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.79s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target-b/debug/deps/run-ef7b05ca62e6bd9b)
```

`Run clicommands` - exit 0

```
test a_second_build_is_cached_and_compiles_nothing ... ok
test test_prints_target_margins_after_the_run_line ... ok
test test_prints_no_table_and_no_other_record ... ok
test test_prints_pass_with_the_check_count ... ok
test a_new_project_checks_runs_and_tests ... ok
test test_prints_fail_and_the_failing_records ... ok
test the_last_line_counts_and_the_exit_code_follows ... ok
test clean_cache_removes_the_compiled_cache ... ok
test verbose_prints_a_source_path_that_exists ... ok
test test_runs_every_run_or_the_named_ones ... ok

test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.08s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/commands.rs (/var/tmp/piperine-target-b/debug/deps/commands-2cccc266e135b8c2)
```

`Run apidigital` - exit 0

```
running 3 tests
test a_digital_only_grid_holds_net_values ... ok
test a_digital_only_run_ended_by_finish_is_ok ... ok
test a_digital_only_transient_reaches_its_stop ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.24s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/digital_only.rs (/var/tmp/piperine-target-b/debug/deps/digital_only-527fb356188469a1)
```

`Run clibook` - exit 0

```
test the_cycle_counter_prints_three_pulses ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok
test the_buck_block_is_what_the_binary_prints ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 127.08s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target-b/debug/deps/book-f38e47a15c9a1081)
```

`Run apiderived` - exit 0

```
running 7 tests
test a_sweep_recomputes_the_child_at_each_point ... ok
test a_timed_set_recomputes_at_the_same_instant ... ok
test a_set_neither_elaborates_nor_compiles ... ok
test a_definition_is_recomputed_at_the_set ... ok
test a_body_let_reads_the_new_param ... ok
test the_s2_probe_prints_three_quarters ... ok
test a_child_failing_where_refuses_the_set ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.48s

    Finished `test` profile [unoptimized] target(s) in 0.15s
     Running tests/derived.rs (/var/tmp/piperine-target-b/debug/deps/derived-ddb6d30a6e116306)
```

`Run twins` - exit 0

```
running 5 tests
test the_two_backends_print_the_same_bytes ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.75s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/twins.rs (/var/tmp/piperine-target-b/debug/deps/twins-9c63cef8d18c33f2)
```

`Run parity` - exit 0

```
test tran_jump ... ok
test tran_corner ... ok
test tran_floor ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.61s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_ac.rs (/var/tmp/piperine-target-b/debug/deps/parity_ac-55a5dfa8bfad5186)
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-b/debug/deps/parity_corpus-3c6b0216946f1435)
     Running tests/parity_dc.rs (/var/tmp/piperine-target-b/debug/deps/parity_dc-e39252c43babc518)
     Running tests/parity_grid.rs (/var/tmp/piperine-target-b/debug/deps/parity_grid-c132f3741e4fa212)
     Running tests/parity_op.rs (/var/tmp/piperine-target-b/debug/deps/parity_op-89aa68e5e8a64ddb)
     Running tests/parity_tran.rs (/var/tmp/piperine-target-b/debug/deps/parity_tran-92e4bdddedb6562d)
```

`Run docs` - exit 0

```
test the_roadmap_marks_the_cli_item_done ... ok
test state_records_the_derived_param_decision ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test state_records_the_observation_decision ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test state_records_the_cli_decision ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test gaps_drops_what_msl_fixes_closed ... ok
test gaps_drops_what_host_ii_closed ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-b/debug/deps/docs-a5e77d72b1cc2f1e)
```

`Run root` - exit 0

```
running 1 test
test the_root_runs_tf_a_drive_and_a_timed_set ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.80s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running crates/piperine-api/tests/root_surface/host_ii.rs (/var/tmp/piperine-target-b/debug/deps/host_ii-ec53b8c1c898cecb)
```

`Run solverbounds` - exit 0

```
running 4 tests
test an_instant_past_the_microstep_bound_fails_naming_it ... ok
test an_operating_point_past_max_rounds_fails_naming_the_bound ... ok
test max_rounds_defaults_to_twenty ... ok
test microsteps_default_to_a_thousand ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.46s
     Running tests/session_bounds.rs (/var/tmp/piperine-target-b/debug/deps/session_bounds-a9dace9f74047326)
```

`Run planlib` - exit 0

```
running 4 tests
test derive::tests::a_chain_derives_top_down ... ok
test netlist::tests::with_microsteps_reaches_the_builder ... ok
test derive::tests::a_constant_placement_is_no_binding ... ok
test derive::tests::a_derived_child_param_is_a_binding_of_its_parent ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-analog-solver)
   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-ir)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 10.51s
     Running unittests src/lib.rs (/var/tmp/piperine-target-b/debug/deps/piperine_plan-9cf08a4c4fa0b943)
```

`Run solverhost` - exit 0

```
running 2 tests
test write_net_before_a_run_is_an_initial_value ... ok
test a_scheduled_write_commits_at_its_time ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.16s
     Running tests/host_writes.rs (/var/tmp/piperine-target-b/debug/deps/host_writes-e71a7b2a4abefbeb)
```

`Run baseline` - exit 0

```
running 7 tests
test its_devices_pass_the_partials_verifier ... ok
test parity_digital_scheduler_snapshot ... ok
test parity_divider_op_dc ... ok
test parity_mixed_signal_dc_settle ... ok
test parity_diode_dc_point ... ok
test parity_rlc_ac_resonant_peak ... ok
test parity_rc_transient ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.98s
     Running tests/parity_baseline.rs (/var/tmp/piperine-target-b/debug/deps/parity_baseline-2aced5317f54915e)
```

`Run transpile` - exit 0

```
running 2 tests
test the_source_holds_one_derive_function_per_binding ... ok
test compiled_derive_evaluates_the_placement_expression ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.68s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-interp)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-host-ii/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 1.89s
     Running tests/derive.rs (/var/tmp/piperine-target-b/debug/deps/derive-cbe2bd60b2025a53)
```

`Run paritylib` - exit 0

```
test ngspice::tests::skip_passes_and_prints_one_line ... ok
test ngspice::tests::print_lines_parse_to_named_values ... ok
test waveform::tests::a_time_outside_the_result_is_none ... ok
test golden::tests::a_message_is_found_in_either_stream ... ok
test golden::tests::a_wrong_row_count_names_both_counts ... ok
test golden::tests::a_grid_mismatch_names_the_point ... ok
test ngspice::tests::a_missing_binary_fails_naming_the_opt_out ... ok
test golden::tests::an_unparsable_row_names_file_and_row ... ok
test ngspice::tests::the_binary_comes_from_the_variable_or_the_path ... ok
test golden::tests::an_export_is_found_with_or_without_data_suffix ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running unittests src/lib.rs (/var/tmp/piperine-target-b/debug/deps/piperine_parity-6187a033365891a7)
```
