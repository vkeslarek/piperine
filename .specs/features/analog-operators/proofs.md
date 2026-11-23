# analog-operators proofs

**HEAD**: 4edfff35
**Tree**: clean
**Shell**: /bin/sh
**Result**: 99 pass, 5 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run spartial / test ddx_of_the_diode_current_is_its_conductance ... ok` | pass |
| C2 | `run spartial / test ddx_against_the_reversed_branch_is_minus_one ... ok` | pass |
| C3 | `run irops / test probes_of_other_branches_are_held_constant ... ok` | pass |
| C3 | `run spartial / test ddx_of_single_node_probes_against_a_branch_is_zero ... ok` | pass |
| C4 | `run spartial / test ddx_against_a_flow_probe_gives_its_factor ... ok` | pass |
| C5 | `run irops / test limit_is_transparent_to_ddx ... ok` | pass |
| C5 | `run spartial / test ddx_differentiates_through_limit ... ok` | pass |
| C6 | `run irops / test results_of_other_instances_are_held_constant ... ok` | pass |
| C7 | `run typeops / test ddx_against_a_non_probe_is_e0451 ... ok` | pass |
| C8 | `run typeops / test ddx_outside_analog_or_read_by_a_definition_is_e0450 ... ok` | pass |
| C9 | `run spartial / test an_ac_sweep_takes_ddx_at_the_operating_point ... ok` | pass |
| C10 | `run partran / test delay_tline ... ok` | pass |
| C11 | `run sdelay / test before_td_the_delay_reads_the_operating_point ... ok` | pass |
| C12 | `run sdelay / test an_operating_point_and_a_zero_delay_give_the_input ... ok` | pass |
| C13 | `run sdelay / test a_varying_delay_reads_back_td_and_max_while_td_exceeds_it ... ok` | pass |
| C13 | `run tops / test the_delay_terms_clamp_td_to_max ... ok` | pass |
| C14 | `run elabops / test a_delay_without_max_whose_td_varies_is_e0452 ... ok` | pass |
| C15 | `run sdelay / test a_negative_td_or_a_non_positive_max_fails_with_e0453 ... ok` | pass |
| C15 | `run tops / test the_delay_function_faults_453 ... ok` | pass |
| C16 | `run parac / test delay_tline_ac ... ok` | pass |
| C17 | `run snoise / test a_noise_source_is_zero_outside_a_noise_sweep_and_never_reads_its_density ... ok` | pass |
| C18 | `run snoise / test one_source_read_twice_is_fully_correlated ... ok` | pass |
| C19 | `run snoise / test two_sources_add_their_powers ... ok` | pass |
| C20 | `run snoise / test a_density_reading_freq_is_evaluated_at_each_sweep_frequency ... ok` | pass |
| C21 | `run snoise / test noise_through_ddt_and_delay_takes_their_response ... ok` | pass |
| C22 | `run snoise / test a_negative_density_fails_with_e0454_naming_the_frequency ... ok` | pass |
| C22 | `run tops / test the_noise_function_writes_its_density_and_faults_454 ... ok` | pass |
| C23 | `run snoise / test a_reaction_or_an_event_reads_noise_as_zero ... ok` | pass |
| C23 | `run tops / test a_reaction_reads_noise_as_zero ... ok` | pass |
| C24 | `run snoise / test two_sources_sharing_a_name_stay_independent ... ok` | pass |
| C24 | `run report / test two_inputs_sharing_a_name_sum_into_one_series ... ok` | pass |
| C25 | `run snoise / test white_noise_is_a_source_of_its_density ... ok` | pass |
| C26 | `run snoise / test flicker_noise_falls_as_one_over_f_to_the_exponent ... ok` | pass |
| C27 | `run elabops / test noise_table_waits_for_its_dependencies ... ok` | pass |
| C27 | `run subset / test p0001_names_the_construct ... ok` | pass |
| C28 | `run apinoise / test a_noise_run_prints_freq_onoise_and_the_total ... ok` | pass |
| C28 | `run clianalyses / test a_noise_run_prints_one_row_per_frequency_then_the_total ... ok` | pass |
| C29 | `run apinoise / test an_input_adds_inoise_and_its_total ... ok` | pass |
| C29 | `run report / test the_input_referred_density_divides_by_the_transfer_from_the_input ... ok` | pass |
| C30 | `run apinoise / test the_output_and_input_flags_win_over_the_keys ... ok` | pass |
| C31 | `run apinoise / test a_noise_run_without_output_or_sweep_is_p0010 ... ok` | pass |
| C32 | `run apinoise / test an_output_or_input_that_names_nothing_is_e0801 ... ok` | pass |
| C33 | `run apinoise / test a_key_of_another_analysis_is_p0010_in_a_noise_run ... ok` | pass |
| C34 | `run snoise / test analysis_noise_is_true_and_ac_stim_zero_in_a_noise_run ... ok` | pass |
| C35 | `run apinoise / test the_library_returns_rows_totals_and_group_densities ... ok` | pass |
| C35 | `run apinoise / test report::the_report_takes_roots_groups_by_name_else_instance_and_carries_the_totals ... ok` | pass |
| C36 | `run parnoise / test noise_rc ... ok` | pass |
| C37 | `run parnoise / test noise_switch ... ok` | pass |
| C38 | `run parnoise / test noise_diode ... ok` | pass |
| C39 | `run parnoise / test noise_twins_run_and_print_the_skip_line_without_ngspice ... ok` | pass |
| C40 | `run ssession / test simparam_reads_the_run_entry_else_its_default ... ok` | pass |
| C41 | `run ssession / test simparam_reads_the_options_by_their_key_names ... ok` | pass |
| C42 | `run apisession / test a_name_the_run_lacks_reads_none ... ok` | pass |
| C43 | `run ssession / test simparam_runs_in_a_definition_and_a_selector_with_one_value ... ok` | pass |
| C44 | `run clioptions / test simparam_flag_is_repeatable_and_a_non_number_is_p0010 ... ok` | pass |
| C44 | `run apioptions / test a_simparam_entry_naming_an_option_is_p0010 ... ok` | pass |
| C45 | `run apioptions / test tnom_key_and_flag_default_to_27 ... ok` | **no test ran** |
| C45 | `run apioptions / test the_tnom_key_of_a_run_table_reaches_the_session_context ... ok` | **no test ran** |
| C46 | `run apioptions / test tnom_at_or_below_absolute_zero_is_p0010 ... ok` | **no test ran** |
| C46 | `run clioptions / test a_tnom_that_is_no_number_is_p0010_naming_the_flag ... ok` | **no test ran** |
| C47 | `run apisession / test the_session_hands_its_options_and_entries_to_the_circuit ... ok` | pass |
| C47 | `run parop / test tnom_res ... ok` | pass |
| C48 | `run parop / test tnom_res ... ok` | pass |
| C49 | `run straces / test a_trace_writes_one_record_per_evaluation_with_path_analysis_instant_and_iteration ... ok` | pass |
| C50 | `run straces / test traces_of_a_rejected_step_stay ... ok` | pass |
| C51 | `run straces / test an_ac_or_noise_sweep_writes_no_trace ... ok` | pass |
| C52 | `run straces / test a_trace_under_an_if_writes_only_when_executed ... ok` | pass |
| C52 | `run tops / test a_trace_writes_its_watch_only_where_it_executes ... ok` | pass |
| C53 | `run typeops / test a_site_reached_from_analog_writing_sim_or_checks_is_e0603 ... ok` | pass |
| C54 | `run apitraces / test traces_are_recorded_only_for_a_named_stream ... ok` | pass |
| C55 | `run straces / test a_trace_changes_no_solution ... ok` | pass |
| C56 | `run apinatures / test a_potential_abstol_of_one_millivolt_reaches_its_node ... ok` | pass |
| C57 | `run apinatures / test a_flow_abstol_of_one_nanoampere_reaches_its_branch ... ok` | pass |
| C71 | `run apinatures / test a_nature_only_tightens_the_session_abstol_of_a_branch ... ok` | pass |
| C58 | `run apinatures / test compatible_natures_take_the_smaller_abstol_and_the_divider_holds ... ok` | pass |
| C59 | `run apinatures / test natures_with_different_flow_names_at_a_port_are_e0251 ... ok` | pass |
| C60 | `run frozen / test the_interpreter_refuses_each_analog_operator_by_name ... ok` | pass |
| C61 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C61 | `run tgolden / test the_five_m1_bodies_print_as_the_golden_files ... ok` | pass |
| C61 | `run planops / test a_body_without_a_new_form_keeps_its_shape ... ok` | pass |
| C62 | `run contract / test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok` | pass |
| C62 | `run corpus / test every_twin_has_a_cir_and_a_test ... ok` | pass |
| C63 | `run docs / test gaps_and_roadmap_follow_analog_operators ... ok` | pass |
| C63 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C63 | `run inventory / test every_block_of_the_book_has_one_row ... ok` | pass |
| C64 | `run sslew / test slew_limits_an_ideal_step_to_its_rate ... ok` | pass |
| C64 | `run retake / test a_crossing_made_by_a_jump_retakes_the_step ... ok` | pass |
| C64 | `run spectext / test part_v_retakes_the_step_a_jump_crossing_changes ... ok` | pass |
| C65 | `run report / test a_flow_output_reads_amperes_per_root_hertz ... ok` | pass |
| C66 | `run apinoise / test a_design_with_no_noise_source_prints_zero_and_succeeds ... ok` | pass |
| C67 | `run spartial / test ddx_of_an_expression_that_reads_no_probe_is_zero ... ok` | pass |
| C68 | `run elabops / test a_field_type_conflict_across_trace_sites_is_e0605 ... ok` | pass |
| C69 | `run typeops / test a_value_of_another_type_given_to_delay_or_noise_is_a_type_mismatch ... ok` | pass |
| C72 | `run retake / test a_crossing_made_by_a_step_that_reads_a_settle_commit_retakes_it ... ok` | pass |
| C72 | `run retake / test a_second_round_retake_runs_settle_once_and_delivers_its_settle_commits ... ok` | pass |
| C72 | `run retake / test the_first_step_after_an_operating_point_that_holds_a_settle_commit_is_first_order ... ok` | pass |
| C72 | `run digital / test a_reopened_instant_settles_at_its_last_microstep ... ok` | pass |
| C72 | `run digital / test a_firing_after_a_reopened_instant_is_due_at_the_next_microstep ... ok` | pass |
| C70 | `run numerics / test the_power_law_integral_is_exact_on_power_laws ... ok` | pass |
| C70 | `run report / test the_band_total_integrates_the_density_as_ngspice_does ... ok` | pass |
| C70 | `run abinoise / test test_per_source_noise_reporting_and_conservation ... ok` | pass |
| Build | `cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piper` | **fail** - Finished dev profile [unoptimized] target(s) in 0.48s |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run spartial` - exit 0

```
running 7 tests
test an_ac_sweep_takes_ddx_at_the_operating_point ... ok
test ddx_differentiates_through_limit ... ok
test ddx_of_single_node_probes_against_a_branch_is_zero ... ok
test ddx_of_the_diode_current_is_its_conductance ... ok
test ddx_against_a_flow_probe_gives_its_factor ... ok
test ddx_of_an_expression_that_reads_no_probe_is_zero ... ok
test ddx_against_the_reversed_branch_is_minus_one ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_partial.rs (/var/tmp/piperine-target-b/debug/deps/source_partial-57b3052cf6691a1a)
```

`Run irops` - exit 0

```
test a_call_a_block_with_lets_and_a_second_derivative_differentiate ... ok
test a_session_parameter_is_built_checked_printed_and_round_tripped ... ok
test a_noise_source_is_built_checked_printed_and_round_tripped ... ok
test a_delay_site_is_built_checked_printed_and_round_tripped ... ok
test a_trace_site_is_built_checked_printed_and_round_tripped ... ok
test limit_is_transparent_to_ddx ... ok
test probes_of_other_branches_are_held_constant ... ok
test results_of_other_instances_are_held_constant ... ok
test the_reversed_probe_and_a_flow_have_their_own_rule ... ok
test each_form_differentiates_by_the_rule_of_its_row ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/operators.rs (/var/tmp/piperine-target-b/debug/deps/operators-fbe860871a0c0021)
```

`Run typeops` - exit 0

```
running 6 tests
test ddx_outside_analog_or_read_by_a_definition_is_e0450 ... ok
test a_value_of_another_type_given_to_delay_or_noise_is_a_type_mismatch ... ok
test ddx_against_a_non_probe_is_e0451 ... ok
test ddx_types_against_a_potential_or_a_flow_probe ... ok
test delay_noise_and_simparam_type_and_wait_for_lowering ... ok
test a_site_reached_from_analog_writing_sim_or_checks_is_e0603 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_analog_operators.rs (/var/tmp/piperine-target-b/debug/deps/types_analog_operators-ccf229161c3c9c24)
```

`Run partran` - exit 0

```
test tran_corner ... ok
test tran_floor ... ok
test tran_jump ... ok
test delay_tline ... ok
test uic_rc ... ok
test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok
test tran_rc_with_a_scaled_derivative ... ok
test tran_clipper ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.29s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_tran.rs (/var/tmp/piperine-target-b/debug/deps/parity_tran-1f68c93d92106e3e)
```

`Run sdelay` - exit 0

```
running 4 tests
test a_negative_td_or_a_non_positive_max_fails_with_e0453 ... ok
test before_td_the_delay_reads_the_operating_point ... ok
test a_varying_delay_reads_back_td_and_max_while_td_exceeds_it ... ok
test an_operating_point_and_a_zero_delay_give_the_input ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.86s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_delay.rs (/var/tmp/piperine-target-b/debug/deps/source_delay-f52eac1d92ae4bee)
```

`Run tops` - exit 0

```
running 6 tests
test the_delay_terms_clamp_td_to_max ... ok
test a_session_parameter_reads_its_pin_or_none ... ok
test a_trace_writes_its_watch_only_where_it_executes ... ok
test a_reaction_reads_noise_as_zero ... ok
test the_noise_function_writes_its_density_and_faults_454 ... ok
test the_delay_function_faults_453 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.74s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/operators.rs (/var/tmp/piperine-target-b/debug/deps/operators-fb1f119147d5319f)
```

`Run elabops` - exit 0

```
running 8 tests
test ddx_through_a_fn_call_is_p0001_naming_the_form ... ok
test an_emit_in_analog_lowers_to_a_trace_site ... ok
test one_delay_call_is_one_site_however_often_it_is_read ... ok
test noise_table_waits_for_its_dependencies ... ok
test a_field_type_conflict_across_trace_sites_is_e0605 ... ok
test ddx_lowers_to_its_derivative_and_is_recorded ... ok
test simparam_lowers_to_one_session_parameter_per_name ... ok
test a_delay_without_max_whose_td_varies_is_e0452 ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.85s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_operators.rs (/var/tmp/piperine-target-b/debug/deps/elab_operators-135d1d7e6eca17f8)
```

`Run parac` - exit 0

```
running 3 tests
test delay_tline_ac ... ok
test ac_rc_with_a_scaled_derivative ... ok
test ac_rc ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/parity_ac.rs (/var/tmp/piperine-target-b/debug/deps/parity_ac-ddd91a46c3793786)
```

`Run snoise` - exit 0

```
test a_density_reading_freq_is_evaluated_at_each_sweep_frequency ... ok
test a_negative_density_fails_with_e0454_naming_the_frequency ... ok
test flicker_noise_falls_as_one_over_f_to_the_exponent ... ok
test white_noise_is_a_source_of_its_density ... ok
test two_sources_add_their_powers ... ok
test two_sources_sharing_a_name_stay_independent ... ok
test a_noise_source_is_zero_outside_a_noise_sweep_and_never_reads_its_density ... ok
test a_reaction_or_an_event_reads_noise_as_zero ... ok
test one_source_read_twice_is_fully_correlated ... ok
test noise_through_ddt_and_delay_takes_their_response ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.66s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_noise.rs (/var/tmp/piperine-target-b/debug/deps/source_noise-7f546399e886f4bd)
```

`Run report` - exit 0

```
running 8 tests
test a_term_output_adds_its_direct_noise_to_the_circuit_path ... ok
test a_differential_output_reads_the_pair ... ok
test a_flow_output_reads_amperes_per_root_hertz ... ok
test its_device_passes_the_partials_verifier ... ok
test an_input_with_no_stimulus_is_refused_by_name ... ok
test the_band_total_integrates_the_density_as_ngspice_does ... ok
test the_input_referred_density_divides_by_the_transfer_from_the_input ... ok
test two_inputs_sharing_a_name_sum_into_one_series ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/noise_report.rs (/var/tmp/piperine-target-b/debug/deps/noise_report-ef38662df71a701e)
```

`Run subset` - exit 0

```
running 4 tests
test unreached_items_are_not_reported ... ok
test a_construct_outside_the_subset_is_p0001 ... ok
test p0001_names_the_construct ... ok
test std_types_outside_the_subset_are_p0001 ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.98s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/types_subset.rs (/var/tmp/piperine-target-b/debug/deps/types_subset-57a42cb007e51583)
```

`Run apinoise` - exit 0

```
running 9 tests
test a_noise_run_without_output_or_sweep_is_p0010 ... ok
test report::the_report_takes_roots_groups_by_name_else_instance_and_carries_the_totals ... ok
test the_output_and_input_flags_win_over_the_keys ... ok
test a_key_of_another_analysis_is_p0010_in_a_noise_run ... ok
test resolution::an_output_or_input_that_names_nothing_is_e0801 ... ok
test run::a_noise_run_prints_freq_onoise_and_the_total ... ok
test run::an_input_adds_inoise_and_its_total ... ok
test run::a_design_with_no_noise_source_prints_zero_and_succeeds ... ok
test run::the_library_returns_rows_totals_and_group_densities ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.98s

    Finished `test` profile [unoptimized] target(s) in 0.24s
     Running tests/noise.rs (/var/tmp/piperine-target-b/debug/deps/noise-de9847af3e7fa6ea)
```

`Run clianalyses` - exit 0

```
running 4 tests
test a_noise_run_prints_one_row_per_frequency_then_the_total ... ok
test the_new_keys_have_flags ... ok
test each_new_analysis_writes_csv ... ok
test each_new_analysis_prints_header_records_and_table ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.92s

    Finished `test` profile [unoptimized] target(s) in 0.28s
     Running tests/analyses.rs (/var/tmp/piperine-target-b/debug/deps/analyses-3e9aa3dc5dbc2976)
```

`Run parnoise` - exit 0

```
running 6 tests
test noise_diode_at_one_kilohertz_is_the_hand_derived_density ... ok
test noise_rc ... ok
test noise_switch_open_agrees_with_ngspice ... ok
test noise_switch ... ok
test noise_diode ... ok
test noise_twins_run_and_print_the_skip_line_without_ngspice ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.43s

    Finished `test` profile [unoptimized] target(s) in 0.41s
     Running tests/parity_noise.rs (/var/tmp/piperine-target-b/debug/deps/parity_noise-24a19d854b6103b5)
```

`Run ssession` - exit 0

```
running 3 tests
test simparam_reads_the_options_by_their_key_names ... ok
test simparam_reads_the_run_entry_else_its_default ... ok
test simparam_runs_in_a_definition_and_a_selector_with_one_value ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.43s

    Finished `test` profile [unoptimized] target(s) in 0.18s
     Running tests/source_session.rs (/var/tmp/piperine-target-b/debug/deps/source_session-06c1bb572709e053)
```

`Run apisession` - exit 0

```
running 2 tests
test a_name_the_run_lacks_reads_none ... ok
test the_session_hands_its_options_and_entries_to_the_circuit ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.80s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/session_parameters.rs (/var/tmp/piperine-target-b/debug/deps/session_parameters-4e08259f015f363e)
```

`Run clioptions` - exit 0

```
test an_existing_records_file_is_replaced ... ok
test plot_writes_an_svg_and_prints_wrote ... ok
test records_go_to_the_file_as_json_lines_and_not_to_stdout ... ok
test a_passing_check_is_hidden_a_failing_one_shown_and_all_checks_shows_both ... ok
test simparam_flag_is_repeatable_and_a_non_number_is_p0010 ... ok
test the_divider_with_r2_at_1k_gives_5 ... ok
test a_relative_output_path_is_under_the_project ... ok
test the_stream_flag_keeps_only_that_stream ... ok
test a_long_transient_at_the_default_resolution_lands_its_crossings ... ok
test a_param_is_checked_at_equality_and_against_the_others ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.13s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/options.rs (/var/tmp/piperine-target-b/debug/deps/options-d07736c36fa4a077)
```

`Run apioptions` - exit 0

```
test the_time_point_rule_is_a_host_option_with_no_run_key ... ok
test a_smaller_trtol_takes_more_steps ... ok
test a_run_past_max_steps_fails_and_succeeds_when_raised ... ok
test temp_is_celsius_read_by_temperature ... ok
test a_twenty_second_transient_runs_at_the_default_resolution ... ok
test resolution_rounds_an_at_to_its_tick ... ok
test maxmicro_bounds_an_instant ... ok
test maxrounds_bounds_the_operating_point ... ok
test a_hundred_second_transient_runs_at_the_default_resolution ... ok
test an_hour_long_transient_runs_at_the_default_resolution ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 35.78s

    Finished `test` profile [unoptimized] target(s) in 0.22s
     Running tests/options.rs (/var/tmp/piperine-target-b/debug/deps/options-5b5980cc81321f7a)
```

`Run parop` - exit 0

```
running 7 tests
test diode_rs ... ok
test tnom_res ... ok
test isrc_r ... ok
test diode_series ... ok
test diode_rs_runs_and_prints_the_skip_line_without_ngspice ... ok
test divider ... ok
test diode_rs_with_rs_zero_merges_and_matches_rdiode ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.62s

    Finished `test` profile [unoptimized] target(s) in 0.74s
     Running tests/parity_op.rs (/var/tmp/piperine-target-b/debug/deps/parity_op-eac8afb1dbfbcbd7)
```

`Run straces` - exit 0

```
running 6 tests
test traces_are_recorded_only_for_a_named_stream ... ok
test traces_of_a_rejected_step_stay ... ok
test a_trace_writes_one_record_per_evaluation_with_path_analysis_instant_and_iteration ... ok
test a_trace_under_an_if_writes_only_when_executed ... ok
test an_ac_or_noise_sweep_writes_no_trace ... ok
test a_trace_changes_no_solution ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.37s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/source_traces.rs (/var/tmp/piperine-target-b/debug/deps/source_traces-c170dbfac9353c02)
```

`Run apitraces` - exit 0

```
running 1 test
test traces_are_recorded_only_for_a_named_stream ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.27s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/traces.rs (/var/tmp/piperine-target-b/debug/deps/traces-e245f3922c69985a)
```

`Run apinatures` - exit 0

```
running 5 tests
test a_flow_abstol_of_one_nanoampere_reaches_its_branch ... ok
test a_nature_only_tightens_the_session_abstol_of_a_branch ... ok
test natures_with_different_flow_names_at_a_port_are_e0251 ... ok
test a_potential_abstol_of_one_millivolt_reaches_its_node ... ok
test compatible_natures_take_the_smaller_abstol_and_the_divider_holds ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.23s

    Finished `test` profile [unoptimized] target(s) in 0.37s
     Running tests/natures.rs (/var/tmp/piperine-target-b/debug/deps/natures-edaace15ba5a99ea)
```

`Run frozen` - exit 0

```
test a_digital_body_is_rejected_by_name ... ok
test each_generics_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_analog_operator_by_name ... ok
test a_table_expression_is_refused_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test a_settle_event_is_refused_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_level_nodes_are_refused_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

    Finished `test` profile [unoptimized] target(s) in 0.45s
     Running tests/frozen.rs (/var/tmp/piperine-target-b/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run twins` - exit 0

```
running 6 tests
test the_three_op_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 55.68s

    Finished `test` profile [unoptimized] target(s) in 0.47s
     Running tests/twins.rs (/var/tmp/piperine-target-b/debug/deps/twins-9b01bd7200d69219)
```

`Run tgolden` - exit 0

```
running 7 tests
test a_keyword_identifier_is_raw_and_the_abi_name_is_not ... ok
test a_body_without_analog_prints_no_terms_function ... ok
test each_function_is_a_generic_fn_and_each_body_a_device_mod ... ok
test every_item_sits_under_its_path_and_origin ... ok
test the_five_m1_bodies_print_as_the_golden_files ... ok
test transpiling_twice_gives_the_same_bytes ... ok
test a_design_with_nothing_to_run_has_no_module ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.67s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/golden.rs (/var/tmp/piperine-target-b/debug/deps/golden-ab85f3ee5d658ee1)
```

`Run planops` - exit 0

```
running 7 tests
test a_session_parameter_is_a_named_global_optional_pin ... ok
test a_body_without_a_new_form_keeps_its_shape ... ok
test a_trace_site_is_a_trace_watch_written_by_terms ... ok
test the_netlist_hands_its_session_parameters_to_the_builder ... ok
test a_delay_site_has_its_pin_terms_and_maximum ... ok
test a_noise_source_is_an_input_with_its_density_function ... ok
test a_delay_without_a_maximum_bounds_the_history_by_td ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/operators_shape.rs (/var/tmp/piperine-target-b/debug/deps/operators_shape-4ff1bd8ca950844e)
```

`Run contract` - exit 0

```
running 8 tests
test contract::tests::phase_is_gated_on_the_magnitude_floor ... ok
test contract::tests::new_domains_carry_a_rationale ... ok
test contract::tests::a_rejection_names_the_ngspice_version ... ok
test contract::tests::holds_is_abstol_plus_reltol_of_the_larger ... ok
test contract::tests::a_rejection_names_point_values_bound_and_rationale ... ok
test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok
test contract::tests::the_noise_contract_carries_its_bound_and_rationale ... ok
test contract::tests::two_zeros_are_accepted ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running unittests src/lib.rs (/var/tmp/piperine-target-b/debug/deps/piperine_parity-6187a033365891a7)
```

`Run corpus` - exit 0

```
running 8 tests
test a_literal_tolerance_in_a_parity_target_is_reported ... ok
test tests_ngspice_holds_no_twin ... ok
test the_guards_report_planted_text ... ok
test no_parity_target_compares_against_a_literal ... ok
test a_scenario_with_benches_holds_a_manifest ... ok
test the_layout_guards_report_planted_trees ... ok
test every_twin_has_its_files_in_one_scenario_directory ... ok
test every_twin_has_a_cir_and_a_test ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-b/debug/deps/parity_corpus-0b598b4d30604325)
```

`Run docs` - exit 0

```
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test transient_performance_closes_v12 ... ok
test the_roadmap_is_forward_looking ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test state_holds_the_explicit_unknowns_decision ... ok
test state_records_the_cli_decision ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.19s
     Running tests/docs.rs (/var/tmp/piperine-target-b/debug/deps/docs-93bbd657b5852db6)
```

`Run clibook` - exit 0

```
test the_chapter_shows_the_session_options ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 105.82s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book.rs (/var/tmp/piperine-target-b/debug/deps/book-a1f18a8423c7ed6b)
```

`Run inventory` - exit 0

```
test every_block_of_the_book_has_one_row ... ok
test no_row_diverges ... ok
test a_model_across_blocks_names_one_test ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test a_verdict_fits_its_block ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_row_names_an_existing_test ... ok
test the_book_uses_no_removed_name ... ok
test the_book_reads_every_block ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-b/debug/deps/book_inventory-1a21c47b4e1a6a60)
```

`Run sslew` - exit 0

```
running 1 test
test slew_limits_an_ideal_step_to_its_rate ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.66s

    Finished `test` profile [unoptimized] target(s) in 0.17s
     Running tests/source_slew.rs (/var/tmp/piperine-target-b/debug/deps/source_slew-5ffa113f3c8c4d72)
```

`Run retake` - exit 0

```
running 8 tests
test the_first_step_after_an_operating_point_that_holds_a_settle_commit_is_first_order ... ok
test its_devices_pass_the_partials_verifier ... ok
test a_step_that_reads_no_jump_never_retakes ... ok
test a_crossing_made_by_a_step_that_reads_a_settle_commit_retakes_it ... ok
test a_crossing_made_by_a_jump_retakes_the_step ... ok
test endless_retakes_fail_with_e0504 ... ok
test a_second_round_retake_runs_settle_once_and_delivers_its_settle_commits ... ok
test a_retaken_step_is_never_rejected_by_the_jump_estimate_whatever_the_time_constant ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/jump_retake.rs (/var/tmp/piperine-target-b/debug/deps/jump_retake-313fbf3cb14da38c)
```

`Run spectext` - exit 0

```
test tran_in_fences_is_the_module ... ok
test the_step_that_reads_a_jump_is_first_order ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_library_declares_no_net_nature ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test zero_bytes_are_the_default_value ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test the_removed_words_are_absent_from_the_specification ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok

test result: ok. 137 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s

    Finished `test` profile [unoptimized] target(s) in 0.30s
     Running tests/spec_text.rs (/var/tmp/piperine-target-b/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run digital` - exit 0

```
test a_settled_change_is_one_observation ... ok
test an_instant_reports_boundary_fired_and_final ... ok
test previous_is_the_value_before_the_microstep ... ok
test settle_records_carry_the_last_microstep_index ... ok
test start_commits_inits_and_fires_start ... ok
test two_commits_to_one_net_settle_to_one_write ... ok
test the_last_plain_commit_of_a_microstep_is_the_value ... ok
test the_traced_instant_of_section_15_1 ... ok
test start_at_commits_init_at_its_tick ... ok
test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/instant.rs (/var/tmp/piperine-target-b/debug/deps/instant-cf6184e72773bc7c)
```

`Run numerics` - exit 0

```
running 5 tests
test a_stimulus_column_carries_its_partials_and_phase ... ok
test a_term_linearization_carries_its_unknown_and_noise_partials ... ok
test the_power_law_integral_is_exact_on_power_laws ... ok
test a_zero_density_takes_the_plain_rectangle ... ok
test the_power_law_integral_tracks_a_two_pole_sum_against_a_fine_trapezoid ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/noise_numerics.rs (/var/tmp/piperine-target-b/debug/deps/noise_numerics-8567472147f9a2de)
```

`Run abinoise` - exit 0

```
running 4 tests
test its_devices_pass_the_partials_verifier ... ok
test a_resistor_divider_has_its_thermal_noise_on_devices ... ok
test test_per_source_noise_reporting_and_conservation ... ok
test a_shared_noise_input_cancels_across_the_pair ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/abi_noise.rs (/var/tmp/piperine-target-b/debug/deps/abi_noise-e5410922ed420bd0)
```
