# msl-fixes proofs

**HEAD**: de923aae
**Tree**: clean
**Shell**: /bin/sh
**Result**: 90 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sfix / test finish_ends_the_transient_at_its_instant ... ok` | pass |
| C2 | `run sfix / test fatal_ends_the_transient_at_its_instant ... ok` | pass |
| C3 | `run sfix / test a_finish_field_false_or_unknown_runs_to_stop ... ok` | pass |
| C4 | `run sfix / test finish_ends_an_operating_point ... ok` | pass |
| C5 | `run records / test a_run_ended_by_finish_gives_its_rows_to_the_instant ... ok` | pass |
| C6 | `run clirun / test run_prints_the_fatal_line_and_exits_1 ... ok` | pass |
| C7 | `run clicmd / test test_prints_fail_with_the_fatal_message ... ok` | pass |
| C8 | `run clicmd / test test_passes_a_run_ended_by_finish ... ok` | pass |
| C9 | `run records / test a_short_run_without_finish_still_fails ... ok` | pass |
| C10 | `run tmod / test a_where_that_reads_a_later_param_is_e0318 ... ok` | pass |
| C11 | `run tmod / test a_where_over_its_own_and_earlier_params_is_accepted ... ok` | pass |
| C12 | `run sfix / test a_literal_flow_source_builds_and_runs ... ok` | pass |
| C13 | `run sfix / test a_literal_potential_source_builds_and_runs ... ok` | pass |
| C14 | `run sfix / test a_carrier_driven_current_builds_and_runs ... ok` | pass |
| C15 | `cargo test -p piperine-transpile --test golden && cargo test -p piperine-cli --test twins && git diff --exit-c` | pass |
| C16 | `run sfix / test a_cross_over_a_maybe_unknown_carrier_builds ... ok` | pass |
| C17 | `run sfix / test absdelta_fires_every_two_tenths_of_a_volt ... ok` | pass |
| C18 | `run watch / test a_nan_cross_watch_is_unknown_and_never_fires ... ok` | pass |
| C19 | `run watch / test an_infinite_cross_watch_fails_the_solve ... ok` | pass |
| C20 | `run sfix / test left_out_optional_arguments_are_none ... ok` | pass |
| C21 | `run sfix / test a_named_optional_argument_binds_by_name ... ok` | pass |
| C22 | `run sfix / test a_plain_value_for_an_option_is_some ... ok` | pass |
| C23 | `run sfix / test none_and_some_pass_positionally ... ok` | pass |
| C24 | `run tvalues / test a_named_argument_that_is_not_optional_or_bound_is_e0140 ... ok` | pass |
| C25 | `run tvalues / test a_wrong_argument_count_is_e0141 ... ok` | pass |
| C26 | `run tvalues / test an_option_before_a_required_parameter_is_positional_only ... ok` | pass |
| C27 | `run sfix / test an_enum_deriving_partial_ord_orders_its_variants_in_a_reaction ... ok` | pass |
| C28 | `run tvalues / test comparing_an_enum_without_partial_ord_is_p0002 ... ok` | pass |
| C29 | `run sfix / test two_analog_blocks_act_as_one ... ok` | pass |
| C30 | `run eanalog / test two_analog_blocks_keep_their_locals_apart ... ok` | pass |
| C31 | `run senv / test bound_step_caps_every_step ... ok` | pass |
| C32 | `run senv / test the_smaller_of_two_caps_wins ... ok` | pass |
| C33 | `run senv / test a_non_positive_bound_step_is_e0509 ... ok` | pass |
| C34 | `run eanalog / test bound_step_under_an_if_is_p0001 ... ok` | pass |
| C35 | `run senv / test freq_follows_the_ac_sweep ... ok` | pass |
| C36 | `run senv / test freq_is_zero_outside_a_sweep ... ok` | pass |
| C37 | `run senv / test freq_in_a_reaction_is_zero ... ok` | pass |
| C38 | `run tanalog / test simparam_stays_p0001 ... ok` | pass |
| C39 | `run senv / test analysis_dc_holds_in_an_op ... ok` | pass |
| C40 | `run senv / test analysis_transient_holds_in_a_transient ... ok` | pass |
| C41 | `run senv / test analysis_ac_holds_in_an_ac_run ... ok` | pass |
| C42 | `run env / test a_noise_operating_point_carries_the_noise_flag ... ok` | pass |
| C43 | `run senv / test a_reaction_reads_the_analysis_kind ... ok` | pass |
| C44 | `run sfix / test cross_with_tolerances_runs ... ok` | pass |
| C45 | `run plan / test bound_step_is_a_cap_watch_and_cross_tolerances_are_pins ... ok` | pass |
| C46 | `run sfix / test a_cross_with_ttol_fires_within_it ... ok` | pass |
| C47 | `run edig / test a_cross_tolerance_that_is_not_constant_is_p0001 ... ok` | pass |
| C48 | `run sfix / test above_with_an_emit_writes_one_record_at_its_crossing ... ok` | pass |
| C49 | `run sfix / test above_true_from_the_start_writes_one_record_at_zero ... ok` | pass |
| C50 | `run sfix / test two_disjuncts_of_one_microstep_run_the_body_once ... ok` | pass |
| C51 | `run frozen / test the_interpreter_refuses_each_new_form_by_name ... ok` | pass |
| C52 | `run tvalues / test every_h2_row_types_with_its_signature ... ok` | pass |
| C53 | `run scat / test f64_rows_equal_rusts_bit_for_bit_in_analog_and_in_a_reaction ... ok` | pass |
| C54 | `run tlib / test rt::real::tests::the_differentiable_rows_agree_with_finite_differences ... ok` | pass |
| C55 | `run tlib / test rt::real::tests::the_piecewise_rows_have_derivative_zero_or_one ... ok` | pass |
| C56 | `run scat / test a_row_over_an_unknown_value_is_unknown ... ok` | pass |
| C57 | `run tvalues / test a_method_outside_h2_on_f64_is_e0161 ... ok` | pass |
| C58 | `run tint / test every_h1_row_types_with_its_signature ... ok` | pass |
| C59 | `run scat / test pow_wraps ... ok` | pass |
| C60 | `run scat / test saturating_rows_clamp ... ok` | pass |
| C61 | `run scat / test checked_rows_give_none_on_overflow ... ok` | pass |
| C62 | `run scat / test rem_euclid_by_zero_faults_naming_it ... ok` | pass |
| C63 | `run scat / test signed_abs_signum_rem_euclid_follow_rust ... ok` | pass |
| C64 | `run tint / test abs_and_signum_on_an_unsigned_integer_are_e0161 ... ok` | pass |
| C65 | `run scat / test unwrap_and_expect_of_some_give_the_payload ... ok` | pass |
| C66 | `run scat / test unwrap_of_none_faults_naming_unwrap ... ok` | pass |
| C67 | `run scat / test expect_of_none_faults_with_its_message ... ok` | pass |
| C68 | `run scat / test unwrap_or_else_evaluates_its_default_for_none_only ... ok` | pass |
| C69 | `run scat / test map_and_and_then_bind_the_payload ... ok` | pass |
| C70 | `run scat / test or_takes_the_first_some ... ok` | pass |
| C71 | `run scat / test then_and_then_some_follow_the_bool ... ok` | pass |
| C72 | `run tcomp / test expect_with_a_non_literal_message_is_p0001 ... ok` | pass |
| C73 | `run tcomp / test a_method_outside_h3_h4_is_e0161 ... ok` | pass |
| C74 | `run scat / test db10_and_db20_run ... ok` | pass |
| C75 | `run scat / test db10_and_db20_type_check_from_the_stdlib ... ok` | pass |
| C76 | `run clibook / test the_cycle_counter_prints_three_pulses ... ok` | pass |
| C77 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C78 | `run clibook / test the_clipper_refusal_no_longer_names_sin ... ok` | pass |
| C79 | `run docs / test gaps_drops_what_msl_fixes_closed ... ok` | pass |
| C80 | `run docs / test the_roadmap_and_state_record_msl_fixes ... ok` | pass |
| C81 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C82 | `cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran --test par` | pass |
| C83 | `run sfix / test a_cross_over_a_nan_of_known_values_fails_the_solve ... ok` | pass |
| C84 | `run scat / test a_crossed_or_nan_clamp_in_analog_faults_naming_it ... ok` | pass |
| C85 | `run sfix / test the_ttol_given_to_a_cross_reaches_the_solver ... ok` | pass |
| C86 | `run sfix / test a_cross_ttol_that_is_not_positive_is_e0510 ... ok` | pass |
| C87 | `run sfix / test a_zero_argument_function_builds_and_runs ... ok` | pass |
| Build | `cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piper` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run sfix` - exit 0

```
test two_analog_blocks_act_as_one ... ok
test an_enum_deriving_partial_ord_orders_its_variants_in_a_reaction ... ok
test cross_with_tolerances_runs ... ok
test finish_ends_an_operating_point ... ok
test fatal_ends_the_transient_at_its_instant ... ok
test finish_ends_the_transient_at_its_instant ... ok
test none_and_some_pass_positionally ... ok
test two_disjuncts_of_one_microstep_run_the_body_once ... ok
test left_out_optional_arguments_are_none ... ok
test the_ttol_given_to_a_cross_reaches_the_solver ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.16s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_fixes.rs (/var/tmp/piperine-target-a/debug/deps/source_fixes-88f117572db6c497)
```

`Run records` - exit 0

```
test records_arrive_in_time_order ... ok
test a_reserved_stream_keeps_its_name_and_its_site ... ok
test a_short_run_without_finish_still_fails ... ok
test a_design_without_emit_gives_no_record ... ok
test a_text_field_is_the_sources_literal ... ok
test a_failed_require_is_one_checks_record_with_pass_false ... ok
test an_unknown_field_is_unknown ... ok
test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok
test a_run_ended_by_finish_gives_its_rows_to_the_instant ... ok
test keep_matches_a_stream_name_or_its_full_path ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/records.rs (/var/tmp/piperine-target-a/debug/deps/records-3347b0766b7addd0)
```

`Run clirun` - exit 0

```
test a_solver_failure_prints_no_table_and_exits_1 ... ok
test a_completed_run_exits_0 ... ok
test records_come_before_the_table ... ok
test run_prints_header_records_blank_line_and_table ... ok
test a_closed_stdout_ends_with_exit_1_and_no_panic ... ok
test a_first_compile_is_announced_on_stderr ... ok
test records_stay_when_the_analysis_fails ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target-a/debug/deps/run-ef7b05ca62e6bd9b)
```

`Run clicmd` - exit 0

```
test build_prints_one_line_per_run ... ok
test a_second_build_is_cached_and_compiles_nothing ... ok
test test_prints_no_table_and_no_other_record ... ok
test a_new_project_checks_runs_and_tests ... ok
test test_prints_fail_and_the_failing_records ... ok
test test_prints_pass_with_the_check_count ... ok
test clean_cache_removes_the_compiled_cache ... ok
test the_last_line_counts_and_the_exit_code_follows ... ok
test verbose_prints_a_source_path_that_exists ... ok
test test_runs_every_run_or_the_named_ones ... ok

test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.63s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/commands.rs (/var/tmp/piperine-target-a/debug/deps/commands-2cccc266e135b8c2)
```

`Run tmod` - exit 0

```
test a_default_of_the_wrong_type_is_rejected ... ok
test a_where_that_is_not_bool_is_e0318 ... ok
test a_node_init_that_is_not_f64_is_e0210 ... ok
test a_where_that_reads_a_later_param_is_e0318 ... ok
test ports_and_signals_are_nodes_and_gnd_fits_any ... ok
test an_environment_function_in_a_const_context_is_e0323 ... ok
test a_where_on_a_module_or_fn_is_bool ... ok
test a_core_primitive_is_checked_where_it_is_written ... ok
test params_are_f64_or_bool ... ok
test a_placement_checks_nodes_count_and_params ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_modules.rs (/var/tmp/piperine-target-a/debug/deps/types_modules-e42f34873c4ac40f)
```

`Run watch` - exit 0

```
test a_tolerance_the_discrete_side_has_not_written_is_the_default ... ok
test accept_fails_with_e0509_on_an_invalid_cap ... ok
test a_trace_record_survives_a_rejected_step ... ok
test a_var_watch_is_readable_at_an_accepted_solution ... ok
test accept_fails_with_e0510_on_an_invalid_tolerance ... ok
test an_infinite_cross_watch_fails_the_solve ... ok
test accept_reports_the_smallest_cap ... ok
test rising_reports_a_sign_change_without_accepting ... ok
test samples_are_the_continuous_pins_reactions_read ... ok
test an_instance_lists_its_terms_and_var_watches_and_reads_a_term ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/assembly_watch.rs (/var/tmp/piperine-target-a/debug/deps/assembly_watch-eea1d8e19770cb42)
```

`Run tvalues` - exit 0

```
test an_option_before_a_required_parameter_is_positional_only ... ok
test literals_are_f64_or_bool ... ok
test if_and_block_take_their_value_type ... ok
test an_unsupported_form_does_not_hide_an_independent_error ... ok
test diagnostics_come_in_source_order_one_per_expression ... ok
test every_h2_row_types_with_its_signature ... ok
test comparing_an_enum_without_partial_ord_is_p0002 ... ok
test a_call_takes_its_declared_result ... ok
test operators_type_as_rust_types_them ... ok
test the_std_pnjlim_checks_clean ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_values.rs (/var/tmp/piperine-target-a/debug/deps/types_values-65c645751b306c9f)
```

`Run eanalog` - exit 0

```
test a_branch_is_one_unordered_pair_with_its_kind ... ok
test freq_analysis_and_bound_step_lower ... ok
test a_sum_splits_into_direct_and_ddt_addends ... ok
test two_analog_blocks_keep_their_locals_apart ... ok
test a_branch_from_a_node_to_itself_is_e0241 ... ok
test the_device_bodies_equal_the_structural_goldens ... ok
test sites_are_numbered_in_source_order_and_named ... ok
test value_code_lowers_to_the_same_forms_and_functions_once ... ok
test bound_step_under_an_if_is_p0001 ... ok
test a_ddt_off_the_top_of_a_flow_contribution_is_refused ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_analog.rs (/var/tmp/piperine-target-a/debug/deps/elab_analog-52fe00f5ce65d9cd)
```

`Run senv` - exit 0

```
test analysis_ac_holds_in_an_ac_run ... ok
test analysis_dc_holds_in_an_op ... ok
test freq_follows_the_ac_sweep ... ok
test the_smaller_of_two_caps_wins ... ok
test analysis_transient_holds_in_a_transient ... ok
test a_reaction_reads_the_analysis_kind ... ok
test freq_is_zero_outside_a_sweep ... ok
test freq_in_a_reaction_is_zero ... ok
test bound_step_caps_every_step ... ok
test a_non_positive_bound_step_is_e0509 ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.70s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_environment.rs (/var/tmp/piperine-target-a/debug/deps/source_environment-cb130105ec6bd598)
```

`Run tanalog` - exit 0

```
test simparam_stays_p0001 ... ok
test probe_nodes_of_two_natures_are_e0240 ... ok
test a_contribution_that_is_not_f64_is_e0417 ... ok
test a_contribution_target_that_is_no_probe_is_e0410 ... ok
test potential_probes_are_f64 ... ok
test limit_checks_its_input_function_and_seed ... ok
test a_core_primitive_in_a_fn_is_e0450 ... ok
test ddt_ac_stim_and_environment_are_f64 ... ok
test the_std_vt_checks_clean ... ok
test freq_analysis_kinds_and_bound_step_type ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_analog.rs (/var/tmp/piperine-target-a/debug/deps/types_analog-a4abdab24d7cd951)
```

`Run env` - exit 0

```
running 7 tests
test its_devices_pass_the_partials_verifier ... ok
test an_ac_operating_point_carries_the_ac_flag ... ok
test a_noise_operating_point_carries_the_noise_flag ... ok
test a_reaction_reads_temperature_gmin_and_source_scale ... ok
test a_discrete_function_reads_gmin_and_source_scale ... ok
test analog_and_reactions_read_one_gmin_at_the_transient_start ... ok
test a_transient_operating_point_carries_the_tran_flag ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/environment.rs (/var/tmp/piperine-target-a/debug/deps/environment-c79fe8f8302871d8)
```

`Run plan` - exit 0

```
running 8 tests
test a_fired_read_names_the_event_pin ... ok
test a_stream_is_a_record_pin_with_one_field_per_value ... ok
test freq_reads_the_ambient_and_analysis_reads_its_global_pin ... ok
test a_previous_value_is_the_previous_part_of_the_pin ... ok
test an_at_event_is_a_pin_an_arming_reaction_and_a_replace_statement ... ok
test a_flow_read_is_the_branch_pin_and_a_potential_ddt_is_a_ddt_term ... ok
test bound_step_is_a_cap_watch_and_cross_tolerances_are_pins ... ok
test time_and_static_are_ambient_reads ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/amended_shape.rs (/var/tmp/piperine-target-a/debug/deps/amended_shape-530066127389ba5a)
```

`Run edig` - exit 0

```
test changed_rise_fall_and_cross_lower_to_events ... ok
test a_child_param_reads_the_parents_value ... ok
test a_cross_tolerance_that_is_not_constant_is_p0001 ... ok
test a_reaction_body_lowers_in_source_order ... ok
test every_signal_carrier_is_a_design_net ... ok
test carriers_follow_ports_then_signals ... ok
test the_m1_goldens_are_unchanged ... ok
test every_ir_passes_check ... ok
test the_design_is_not_written ... ok
test elaborating_twice_gives_the_same_print ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.90s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_digital.rs (/var/tmp/piperine-target-a/debug/deps/elab_digital-421f4441f369eca2)
```

`Run frozen` - exit 0

```
running 3 tests
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/frozen.rs (/var/tmp/piperine-target-a/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run scat` - exit 0

```
thread '<unnamed>' (673937) panicked at /var/tmp/piperine-tmp-a/.tmpeAl4t4/rust/.staging-16f1f054439da24c6e87a3220d6407e3-673931-32/rt/int.rs:292:30:
the reading is gone
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

thread '<unnamed>' (673942) panicked at /var/tmp/piperine-tmp-a/.tmpeyiYRd/rust/.staging-09e9e007ce1f2e3c59a8e12bc4c24beb-673931-34/rt/int.rs:109:13:
`rem_euclid` by zero: UInt<8> 7 rem_euclid 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

thread '<unnamed>' (674805) panicked at /var/tmp/piperine-tmp-a/.tmpqdPqIF/rust/.staging-a883d92cf6aa5a6c7331fd0383f336dc-673931-49/rt/int.rs:293:21:
called `Option::unwrap()` on a `None` value
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

thread '<unnamed>' (674813) panicked at /var/tmp/piperine-tmp-a/.tmpcHzGh1/rust/.staging-d59c6cc2531b778de0555470a5fb4c8e-673931-55/rt/int.rs:323:13:
integer division by zero: UInt<8> 90 / 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

`Run tlib` - exit 0

```
test shape_check::tests::equal_shapes_have_no_difference_and_a_changed_entry_is_named ... ok
test print::site::tests::stim_and_check_print_plain_values ... ok
test print::value::tests::integer_and_flagged_expressions_print ... ok
test print::text::tests::reals_round_trip_in_the_shortest_form ... ok
test rt::real::tests::dual_gives_the_interpreters_values_and_tangents ... ok
test print::value::tests::each_new_row_prints_plain_and_flagged ... ok
test rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result ... ok
test rt::int::tests::integer_rows_wrap_saturate_and_test_overflow ... ok
test rt::real::tests::each_row_has_the_value_of_rusts_method ... ok
test rt::int::tests::arithmetic_wraps_at_every_width ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running unittests src/lib.rs (/var/tmp/piperine-target-a/debug/deps/piperine_transpile-6d48b28f4fb0a670)
```

`Run tint` - exit 0

```
test a_width_below_one_is_e0103 ... ok
test integer_types_and_constants_have_types ... ok
test every_h1_row_types_with_its_signature ... ok
test min_max_clamp_and_floor_are_typed ... ok
test a_literal_out_of_range_is_e0909 ... ok
test mixed_integer_operands_are_p0002 ... ok
test an_unsupported_width_or_float_is_named ... ok
test integer_operators_take_one_type ... ok
test as_goes_between_integers_and_f64 ... ok
test every_accepted_call_has_a_callee ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_integers.rs (/var/tmp/piperine-target-a/debug/deps/types_integers-473f76264640c674)
```

`Run tcomp` - exit 0

```
test a_value_for_an_option_parameter_is_some_of_it ... ok
test a_method_outside_h3_h4_is_e0161 ... ok
test an_enum_outside_the_subset_is_p0001 ... ok
test every_option_and_bool_row_types_with_a_closure_or_value ... ok
test a_tuple_let_names_each_component ... ok
test option_values_and_their_methods_are_typed ... ok
test match_is_typed_over_enums_bools_integers_and_options ... ok
test a_match_that_is_not_exhaustive_is_p0002 ... ok
test unit_enums_and_their_variants_are_typed ... ok
test a_nested_pattern_inside_some_covers_its_payload ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_compound.rs (/var/tmp/piperine-target-a/debug/deps/types_compound-1cbb3f8d1a2a062c)
```

`Run clibook` - exit 0

```
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test the_clipper_refusal_no_longer_names_sin ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_cycle_counter_prints_three_pulses ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 164.90s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target-a/debug/deps/book-f38e47a15c9a1081)
```

`Run docs` - exit 0

```
test gaps_says_a_host_run_is_bounded ... ok
test gaps_drops_what_msl_fixes_closed ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test gaps_drops_what_closed_and_lists_what_is_left ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test state_records_the_observation_decision ... ok
test the_buck_readme_says_how_to_run_it ... ok
test state_records_the_cli_decision ... ok
test the_roadmap_and_state_record_msl_fixes ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.24s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-a5e77d72b1cc2f1e)
```

`Run twins` - exit 0

```
running 5 tests
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test the_three_op_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.74s

    Finished `test` profile [unoptimized] target(s) in 0.30s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-9c63cef8d18c33f2)
```

`cargo test -p piperine-transpile --test golden && cargo test -p piperine-cli --test twins && git diff --exit-c` - exit 0

```


running 5 tests
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_three_op_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.41s

    Finished `test` profile [unoptimized] target(s) in 0.22s
     Running tests/golden.rs (/var/tmp/piperine-target-a/debug/deps/golden-ab85f3ee5d658ee1)
    Finished `test` profile [unoptimized] target(s) in 0.31s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-9c63cef8d18c33f2)
```

`cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran --test par` - exit 0

```
test parity_mixed_signal_dc_settle ... ok
test parity_rlc_ac_resonant_peak ... ok
test parity_rc_transient ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_ac.rs (/var/tmp/piperine-target-a/debug/deps/parity_ac-55a5dfa8bfad5186)
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-a/debug/deps/parity_corpus-3c6b0216946f1435)
     Running tests/parity_dc.rs (/var/tmp/piperine-target-a/debug/deps/parity_dc-e39252c43babc518)
     Running tests/parity_grid.rs (/var/tmp/piperine-target-a/debug/deps/parity_grid-c132f3741e4fa212)
     Running tests/parity_op.rs (/var/tmp/piperine-target-a/debug/deps/parity_op-89aa68e5e8a64ddb)
     Running tests/parity_tran.rs (/var/tmp/piperine-target-a/debug/deps/parity_tran-92e4bdddedb6562d)
    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/parity_baseline.rs (/var/tmp/piperine-target-a/debug/deps/parity_baseline-2aced5317f54915e)
```
