# analog-operators proofs

**HEAD**: 630b6b6d
**Tree**: clean
**Shell**: /bin/sh
**Result**: 44 pass, 72 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run spartial / test ddx_of_the_diode_current_is_its_conductance ... ok` | **fail** |
| C2 | `run spartial / test ddx_against_the_reversed_branch_is_minus_one ... ok` | **fail** |
| C3 | `run irops / test probes_of_other_branches_are_held_constant ... ok` | **fail** |
| C3 | `run spartial / test ddx_of_single_node_probes_against_a_branch_is_zero ... ok` | **fail** |
| C4 | `run spartial / test ddx_against_a_flow_probe_gives_its_factor ... ok` | **fail** |
| C5 | `run irops / test limit_is_transparent_to_ddx ... ok` | **fail** |
| C5 | `run spartial / test ddx_differentiates_through_limit ... ok` | **fail** |
| C6 | `run irops / test results_of_other_instances_are_held_constant ... ok` | **fail** |
| C7 | `run typeops / test ddx_against_a_non_probe_is_e0451 ... ok` | **fail** |
| C8 | `run typeops / test ddx_outside_analog_or_read_by_a_definition_is_e0450 ... ok` | **fail** |
| C9 | `run spartial / test an_ac_sweep_takes_ddx_at_the_operating_point ... ok` | **fail** |
| C10 | `run partran / test delay_tline ... ok` | **no test ran** |
| C11 | `run sdelay / test before_td_the_delay_reads_the_operating_point ... ok` | **fail** |
| C12 | `run sdelay / test an_operating_point_and_a_zero_delay_give_the_input ... ok` | **fail** |
| C13 | `run sdelay / test a_varying_delay_reads_back_td_and_max_while_td_exceeds_it ... ok` | **fail** |
| C13 | `run tops / test the_delay_terms_clamp_td_to_max ... ok` | **fail** |
| C14 | `run elabops / test a_delay_without_max_whose_td_varies_is_e0452 ... ok` | **fail** |
| C15 | `run sdelay / test a_negative_td_or_a_non_positive_max_fails_with_e0453 ... ok` | **fail** |
| C15 | `run tops / test the_delay_function_faults_453 ... ok` | **fail** |
| C16 | `run parac / test delay_tline_ac ... ok` | **no test ran** |
| C17 | `run snoise / test a_noise_source_is_zero_outside_a_noise_sweep_and_never_reads_its_density ... ok` | **fail** |
| C18 | `run snoise / test one_source_read_twice_is_fully_correlated ... ok` | **fail** |
| C19 | `run snoise / test two_sources_add_their_powers ... ok` | **fail** |
| C20 | `run snoise / test a_density_reading_freq_is_evaluated_at_each_sweep_frequency ... ok` | **fail** |
| C21 | `run snoise / test noise_through_ddt_and_delay_takes_their_response ... ok` | **fail** |
| C22 | `run snoise / test a_negative_density_fails_with_e0454_naming_the_frequency ... ok` | **fail** |
| C22 | `run tops / test the_noise_function_writes_its_density_and_faults_454 ... ok` | **fail** |
| C23 | `run snoise / test a_reaction_or_an_event_reads_noise_as_zero ... ok` | **fail** |
| C23 | `run tops / test a_reaction_reads_noise_as_zero ... ok` | **fail** |
| C24 | `run snoise / test two_sources_sharing_a_name_stay_independent ... ok` | **fail** |
| C24 | `run report / test two_inputs_sharing_a_name_sum_into_one_series ... ok` | pass |
| C25 | `run snoise / test white_noise_is_a_source_of_its_density ... ok` | **fail** |
| C26 | `run snoise / test flicker_noise_falls_as_one_over_f_to_the_exponent ... ok` | **fail** |
| C27 | `run elabops / test noise_table_waits_for_its_dependencies ... ok` | **fail** |
| C27 | `run subset / test p0001_names_the_construct ... ok` | pass |
| C28 | `run apinoise / test a_noise_run_prints_freq_onoise_and_the_total ... ok` | pass |
| C28 | `run clianalyses / test a_noise_run_prints_one_row_per_frequency_then_the_total ... ok` | pass |
| C29 | `run apinoise / test an_input_adds_inoise_and_its_total ... ok` | pass |
| C29 | `run report / test the_input_referred_density_divides_by_the_transfer_from_the_input ... ok` | pass |
| C30 | `run apinoise / test the_output_and_input_flags_win_over_the_keys ... ok` | pass |
| C31 | `run apinoise / test a_noise_run_without_output_or_sweep_is_p0010 ... ok` | pass |
| C32 | `run apinoise / test an_output_or_input_that_names_nothing_is_e0801 ... ok` | pass |
| C33 | `run apinoise / test a_key_of_another_analysis_is_p0010_in_a_noise_run ... ok` | pass |
| C34 | `run snoise / test analysis_noise_is_true_and_ac_stim_zero_in_a_noise_run ... ok` | **fail** |
| C35 | `run apinoise / test the_library_returns_rows_totals_and_group_densities ... ok` | pass |
| C35 | `run apinoise / test report::the_report_takes_roots_groups_by_name_else_instance_and_carries_the_totals ... ok` | pass |
| C36 | `run parnoise / test noise_rc ... ok` | **fail** |
| C37 | `run parnoise / test noise_switch ... ok` | **fail** |
| C38 | `run parnoise / test noise_diode ... ok` | **fail** |
| C39 | `run parnoise / test noise_twins_run_and_print_the_skip_line_without_ngspice ... ok` | **fail** |
| C40 | `run ssession / test simparam_reads_the_run_entry_else_its_default ... ok` | **fail** |
| C41 | `run ssession / test simparam_reads_the_session_options_and_their_defaults ... ok` | **fail** |
| C42 | `run apisession / test simparam_without_default_and_without_entry_is_e0456 ... ok` | **fail** |
| C43 | `run ssession / test simparam_runs_in_a_definition_and_a_selector_with_one_value ... ok` | **fail** |
| C44 | `run clioptions / test simparam_flag_is_repeatable_and_a_non_number_is_p0010 ... ok` | pass |
| C44 | `run apioptions / test a_simparam_entry_naming_an_option_is_p0010 ... ok` | pass |
| C45 | `run apioptions / test tnom_key_and_flag_default_to_27 ... ok` | pass |
| C45 | `run apioptions / test the_tnom_key_of_a_run_table_reaches_the_session_context ... ok` | pass |
| C46 | `run apioptions / test tnom_at_or_below_absolute_zero_is_p0010 ... ok` | pass |
| C47 | `run ssession / test simparam_tnom_reads_the_run_tnom_in_celsius ... ok` | **fail** |
| C48 | `run parop / test tnom_res ... ok` | **no test ran** |
| C49 | `run straces / test a_trace_writes_one_record_per_evaluation_with_path_analysis_instant_and_iteration ... ok` | **fail** |
| C50 | `run straces / test traces_of_a_rejected_step_stay ... ok` | **fail** |
| C51 | `run straces / test an_ac_or_noise_sweep_writes_no_trace ... ok` | **fail** |
| C52 | `run straces / test a_trace_under_an_if_writes_only_when_executed ... ok` | **fail** |
| C52 | `run tops / test a_trace_writes_its_watch_only_where_it_executes ... ok` | **fail** |
| C53 | `run typeops / test a_site_reached_from_analog_writing_sim_or_checks_is_e0603 ... ok` | **fail** |
| C54 | `run apitraces / test traces_are_recorded_only_for_a_named_stream ... ok` | **fail** |
| C55 | `run straces / test a_trace_changes_no_solution ... ok` | **fail** |
| C56 | `run apinatures / test a_potential_abstol_of_one_millivolt_reaches_its_node ... ok` | pass |
| C57 | `run apinatures / test a_flow_abstol_of_one_nanoampere_reaches_its_branch ... ok` | pass |
| C71 | `run apinatures / test a_nature_only_tightens_the_session_abstol_of_a_branch ... ok` | pass |
| C58 | `run apinatures / test compatible_natures_take_the_smaller_abstol_and_the_divider_holds ... ok` | pass |
| C59 | `run apinatures / test natures_with_different_flow_names_at_a_port_are_e0251 ... ok` | pass |
| C60 | `run frozen / test the_interpreter_refuses_each_analog_operator_by_name ... ok` | **no test ran** |
| C61 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C61 | `run tgolden / test the_five_m1_bodies_print_as_the_golden_files ... ok` | pass |
| C61 | `run planops / test a_body_without_a_new_form_keeps_its_shape ... ok` | **fail** |
| C62 | `run contract / test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok` | pass |
| C62 | `run corpus / test every_twin_has_a_cir_and_a_test ... ok` | pass |
| C63 | `run docs / test gaps_and_roadmap_follow_analog_operators ... ok` | **no test ran** |
| C63 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C63 | `run inventory / test every_block_of_the_book_has_one_row ... ok` | pass |
| C64 | `run sslew / test slew_limits_an_ideal_step_to_its_rate ... ok` | pass |
| C64 | `run retake / test a_crossing_made_by_a_jump_retakes_the_step ... ok` | pass |
| C64 | `run spectext / test part_v_retakes_the_step_a_jump_crossing_changes ... ok` | pass |
| C65 | `run report / test a_flow_output_reads_amperes_per_root_hertz ... ok` | pass |
| C66 | `run apinoise / test a_design_with_no_noise_source_prints_zero_and_succeeds ... ok` | pass |
| C67 | `run spartial / test ddx_of_an_expression_that_reads_no_probe_is_zero ... ok` | **fail** |
| C68 | `run elabops / test a_field_type_conflict_across_trace_sites_is_e0605 ... ok` | **fail** |
| C69 | `run elabops / test a_delay_reading_an_unknown_value_is_e0150 ... ok` | **fail** |
| C69 | `run elabops / test a_noise_density_reading_an_unknown_value_is_e0150 ... ok` | **fail** |
| C72 | `run retake / test a_crossing_made_by_a_step_that_reads_a_settle_commit_retakes_it ... ok` | pass |
| C72 | `run retake / test a_second_round_retake_runs_settle_once_and_delivers_its_settle_commits ... ok` | pass |
| C72 | `run retake / test the_first_step_after_an_operating_point_that_holds_a_settle_commit_is_first_order ... ok` | pass |
| C72 | `run digital / test a_reopened_instant_settles_at_its_last_microstep ... ok` | pass |
| C72 | `run digital / test a_firing_after_a_reopened_instant_is_due_at_the_next_microstep ... ok` | pass |
| C70 | `run numerics / test the_power_law_integral_is_exact_on_power_laws ... ok` | pass |
| C70 | `run report / test the_band_total_integrates_the_density_as_ngspice_does ... ok` | pass |
| C70 | `run abinoise / test test_per_source_noise_reporting_and_conservation ... ok` | pass |
| Run spartial | `cargo test -p piperine-parity --test source_partial` | **fail** - exit 101 |
| Run irops | `cargo test -p piperine-ir --test operators` | **fail** - exit 101 |
| Run typeops | `cargo test -p piperine-lang --test types_operators` | **fail** - exit 101 |
| Run sdelay | `cargo test -p piperine-parity --test source_delay` | **fail** - exit 101 |
| Run tops | `cargo test -p piperine-transpile --test operators` | **fail** - exit 101 |
| Run elabops | `cargo test -p piperine-lang --test elab_operators` | **fail** - exit 101 |
| Run snoise | `cargo test -p piperine-parity --test source_noise` | **fail** - exit 101 |
| Run parnoise | `cargo test -p piperine-parity --test parity_noise` | **fail** - exit 101 |
| Run ssession | `cargo test -p piperine-parity --test source_session` | **fail** - exit 101 |
| Run apisession | `cargo test -p piperine-api --test session_parameters` | **fail** - exit 101 |
| Run straces | `cargo test -p piperine-parity --test source_traces` | **fail** - exit 101 |
| Run apitraces | `cargo test -p piperine-api --test traces` | **fail** - exit 101 |
| Run planops | `cargo test -p piperine-plan --test operators_shape` | **fail** - exit 101 |
| Build | `cargo build -p piperine-lang -p piperine-ir -p piperine-plan -p piperine-transpile -p piperine-interp -p piper` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run spartial` - exit 101

```
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_slew
    source_switch
    source_time
    source_unknowns
    source_value_semantics
    twins
```

`Run irops` - exit 101

```
error: no test target named `operators` in `piperine-ir` package
help: available test targets:
    aggregates
    amendments
    check
    control
    deps
    digital
    model
    print
    serde
    structure
```

`Run typeops` - exit 101

```
    types_buck
    types_carriers
    types_compound
    types_control
    types_entry
    types_events
    types_integers
    types_modules
    types_natures
    types_results
    types_structure
    types_subset
    types_unknowns
    types_value_semantics
    types_values
```

`Run partran` - exit 0

```
test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok
test tran_rc_with_a_scaled_derivative ... ok
test tran_jump ... ok
test tran_corner ... ok
test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok
test tran_floor ... ok
test tran_clipper ... ok
test uic_rc ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.52s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_tran.rs (/var/tmp/piperine-target-d/debug/deps/parity_tran-1f68c93d92106e3e)
```

`Run sdelay` - exit 101

```
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_slew
    source_switch
    source_time
    source_unknowns
    source_value_semantics
    twins
```

`Run tops` - exit 101

```
    control
    derive
    differential
    digital_circuit
    dpwm
    environment
    golden
    load
    pid
    reaction
    reject
    residue
    rustc
    session
    structure
```

`Run elabops` - exit 101

```
    types_buck
    types_carriers
    types_compound
    types_control
    types_entry
    types_events
    types_integers
    types_modules
    types_natures
    types_results
    types_structure
    types_subset
    types_unknowns
    types_value_semantics
    types_values
```

`Run parac` - exit 0

```
running 2 tests
test ac_rc_with_a_scaled_derivative ... ok
test ac_rc ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_ac.rs (/var/tmp/piperine-target-d/debug/deps/parity_ac-ddd91a46c3793786)
```

`Run snoise` - exit 101

```
error: no test target named `source_noise` in `piperine-parity` package

help: a target with a similar name exists: `source_host`
```

`Run report` - exit 0

```
running 8 tests
test its_device_passes_the_partials_verifier ... ok
test two_inputs_sharing_a_name_sum_into_one_series ... ok
test a_term_output_adds_its_direct_noise_to_the_circuit_path ... ok
test an_input_with_no_stimulus_is_refused_by_name ... ok
test the_input_referred_density_divides_by_the_transfer_from_the_input ... ok
test the_band_total_integrates_the_density_as_ngspice_does ... ok
test a_differential_output_reads_the_pair ... ok
test a_flow_output_reads_amperes_per_root_hertz ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/noise_report.rs (/var/tmp/piperine-target-d/debug/deps/noise_report-ef38662df71a701e)
```

`Run subset` - exit 0

```
running 4 tests
test unreached_items_are_not_reported ... ok
test p0001_names_the_construct ... ok
test a_construct_outside_the_subset_is_p0001 ... ok
test std_types_outside_the_subset_are_p0001 ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_subset.rs (/var/tmp/piperine-target-d/debug/deps/types_subset-57a42cb007e51583)
```

`Run apinoise` - exit 0

```
running 9 tests
test report::the_report_takes_roots_groups_by_name_else_instance_and_carries_the_totals ... ok
test the_output_and_input_flags_win_over_the_keys ... ok
test a_noise_run_without_output_or_sweep_is_p0010 ... ok
test a_key_of_another_analysis_is_p0010_in_a_noise_run ... ok
test resolution::an_output_or_input_that_names_nothing_is_e0801 ... ok
test run::an_input_adds_inoise_and_its_total ... ok
test run::a_noise_run_prints_freq_onoise_and_the_total ... ok
test run::a_design_with_no_noise_source_prints_zero_and_succeeds ... ok
test run::the_library_returns_rows_totals_and_group_densities ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.60s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/noise.rs (/var/tmp/piperine-target-d/debug/deps/noise-de9847af3e7fa6ea)
```

`Run clianalyses` - exit 0

```
running 4 tests
test a_noise_run_prints_one_row_per_frequency_then_the_total ... ok
test the_new_keys_have_flags ... ok
test each_new_analysis_writes_csv ... ok
test each_new_analysis_prints_header_records_and_table ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.98s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/analyses.rs (/var/tmp/piperine-target-d/debug/deps/analyses-3e9aa3dc5dbc2976)
```

`Run parnoise` - exit 101

```
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_slew
    source_switch
    source_time
    source_unknowns
    source_value_semantics
    twins
```

`Run ssession` - exit 101

```
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_slew
    source_switch
    source_time
    source_unknowns
    source_value_semantics
    twins
```

`Run apisession` - exit 101

```
    derived
    digital_only
    grid
    host_actions
    host_sources
    memory
    metadata
    natures
    noise
    option_reach
    options
    probes
    project
    records
    session
```

`Run clioptions` - exit 0

```
test a_passing_check_is_hidden_a_failing_one_shown_and_all_checks_shows_both ... ok
test records_go_to_the_file_as_json_lines_and_not_to_stdout ... ok
test csv_writes_the_table_and_prints_wrote ... ok
test a_relative_output_path_is_under_the_project ... ok
test simparam_flag_is_repeatable_and_a_non_number_is_p0010 ... ok
test the_divider_with_r2_at_1k_gives_5 ... ok
test the_stream_flag_keeps_only_that_stream ... ok
test tnom_key_and_flag_default_to_27 ... ok
test a_param_is_checked_at_equality_and_against_the_others ... ok
test a_long_transient_at_the_default_resolution_lands_its_crossings ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.28s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/options.rs (/var/tmp/piperine-target-d/debug/deps/options-d07736c36fa4a077)
```

`Run apioptions` - exit 0

```
test tolerance_keys_reach_the_context ... ok
test temp_is_celsius_read_by_temperature ... ok
test tnom_key_and_flag_default_to_27 ... ok
test a_smaller_trtol_takes_more_steps ... ok
test a_run_past_max_steps_fails_and_succeeds_when_raised ... ok
test resolution_rounds_an_at_to_its_tick ... ok
test maxmicro_bounds_an_instant ... ok
test maxrounds_bounds_the_operating_point ... ok
test a_twenty_second_transient_runs_at_the_default_resolution ... ok
test a_hundred_second_transient_runs_at_the_default_resolution ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.72s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/options.rs (/var/tmp/piperine-target-d/debug/deps/options-5b5980cc81321f7a)
```

`Run parop` - exit 0

```
running 6 tests
test divider ... ok
test diode_rs_runs_and_prints_the_skip_line_without_ngspice ... ok
test diode_rs ... ok
test isrc_r ... ok
test diode_series ... ok
test diode_rs_with_rs_zero_merges_and_matches_rdiode ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.07s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_op.rs (/var/tmp/piperine-target-d/debug/deps/parity_op-eac8afb1dbfbcbd7)
```

`Run straces` - exit 101

```
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_slew
    source_switch
    source_time
    source_unknowns
    source_value_semantics
    twins
```

`Run apitraces` - exit 101

```
error: no test target named `traces` in `piperine-api` package

help: a target with a similar name exists: `probes`
```

`Run apinatures` - exit 0

```
running 5 tests
test a_flow_abstol_of_one_nanoampere_reaches_its_branch ... ok
test a_nature_only_tightens_the_session_abstol_of_a_branch ... ok
test natures_with_different_flow_names_at_a_port_are_e0251 ... ok
test compatible_natures_take_the_smaller_abstol_and_the_divider_holds ... ok
test a_potential_abstol_of_one_millivolt_reaches_its_node ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/natures.rs (/var/tmp/piperine-target-d/debug/deps/natures-edaace15ba5a99ea)
```

`Run frozen` - exit 0

```
running 9 tests
test a_host_source_body_is_rejected_by_name ... ok
test a_settle_event_is_refused_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test the_unknown_nodes_are_refused_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/frozen.rs (/var/tmp/piperine-target-d/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run twins` - exit 0

```
running 6 tests
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.12s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/twins.rs (/var/tmp/piperine-target-d/debug/deps/twins-9b01bd7200d69219)
```

`Run tgolden` - exit 0

```
running 7 tests
test a_keyword_identifier_is_raw_and_the_abi_name_is_not ... ok
test a_body_without_analog_prints_no_terms_function ... ok
test the_five_m1_bodies_print_as_the_golden_files ... ok
test every_item_sits_under_its_path_and_origin ... ok
test each_function_is_a_generic_fn_and_each_body_a_device_mod ... ok
test transpiling_twice_gives_the_same_bytes ... ok
test a_design_with_nothing_to_run_has_no_module ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.03s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/golden.rs (/var/tmp/piperine-target-d/debug/deps/golden-ab85f3ee5d658ee1)
```

`Run planops` - exit 101

```
error: no test target named `operators_shape` in `piperine-plan` package
help: available test targets:
    amended_shape
    composite_shape
    digital_shape
    shared
    structure_shape
```

`Run contract` - exit 0

```
running 7 tests
test contract::tests::holds_is_abstol_plus_reltol_of_the_larger ... ok
test contract::tests::a_rejection_names_the_ngspice_version ... ok
test contract::tests::new_domains_carry_a_rationale ... ok
test contract::tests::phase_is_gated_on_the_magnitude_floor ... ok
test contract::tests::two_zeros_are_accepted ... ok
test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok
test contract::tests::a_rejection_names_point_values_bound_and_rationale ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.32s
     Running unittests src/lib.rs (/var/tmp/piperine-target-d/debug/deps/piperine_parity-6187a033365891a7)
```

`Run corpus` - exit 0

```
running 8 tests
test a_scenario_with_benches_holds_a_manifest ... ok
test a_literal_tolerance_in_a_parity_target_is_reported ... ok
test every_twin_has_its_files_in_one_scenario_directory ... ok
test no_parity_target_compares_against_a_literal ... ok
test tests_ngspice_holds_no_twin ... ok
test the_guards_report_planted_text ... ok
test every_twin_has_a_cir_and_a_test ... ok
test the_layout_guards_report_planted_trees ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.31s
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-d/debug/deps/parity_corpus-0b598b4d30604325)
```

`Run docs` - exit 0

```
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test state_records_the_observation_decision ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_roadmap_is_forward_looking ... ok
test the_buck_readme_says_how_to_run_it ... ok
test the_buck_readme_says_when_to_run_the_buck_on_demand ... ok
test state_records_the_unknown_semantics_decision ... ok
test state_records_the_derived_param_decision ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized] target(s) in 0.27s
     Running tests/docs.rs (/var/tmp/piperine-target-d/debug/deps/docs-93bbd657b5852db6)
```

`Run clibook` - exit 0

```
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test the_chapter_shows_the_session_options ... ok
test a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 56.55s

    Finished `test` profile [unoptimized] target(s) in 0.25s
     Running tests/book.rs (/var/tmp/piperine-target-d/debug/deps/book-a1f18a8423c7ed6b)
```

`Run inventory` - exit 0

```
test the_gaps_guard_names_a_missing_and_an_extra_row ... ok
test a_marked_file_joins_its_blocks ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test disputed_rows_are_listed_for_the_user ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_listing_that_does_not_run_names_its_gap ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test every_row_names_an_existing_test ... ok
test the_book_uses_no_removed_name ... ok
test the_book_reads_every_block ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s

    Finished `test` profile [unoptimized] target(s) in 0.28s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-d/debug/deps/book_inventory-1a21c47b4e1a6a60)
```

`Run sslew` - exit 0

```
running 1 test
test slew_limits_an_ideal_step_to_its_rate ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.16s

    Finished `test` profile [unoptimized] target(s) in 0.40s
     Running tests/source_slew.rs (/var/tmp/piperine-target-d/debug/deps/source_slew-5ffa113f3c8c4d72)
```

`Run retake` - exit 0

```
running 7 tests
test its_devices_pass_the_partials_verifier ... ok
test the_first_step_after_an_operating_point_that_holds_a_settle_commit_is_first_order ... ok
test a_step_that_reads_no_jump_never_retakes ... ok
test a_crossing_made_by_a_step_that_reads_a_settle_commit_retakes_it ... ok
test a_crossing_made_by_a_jump_retakes_the_step ... ok
test a_second_round_retake_runs_settle_once_and_delivers_its_settle_commits ... ok
test endless_retakes_fail_with_e0504 ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/jump_retake.rs (/var/tmp/piperine-target-d/debug/deps/jump_retake-313fbf3cb14da38c)
```

`Run spectext` - exit 0

```
test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok
test tran_in_fences_is_the_module ... ok
test the_library_declares_no_net_nature ... ok
test the_step_that_reads_a_jump_is_first_order ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test zero_bytes_are_the_default_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok

test result: ok. 119 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/spec_text.rs (/var/tmp/piperine-target-d/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run digital` - exit 0

```
test a_settle_commit_with_an_advancing_delay_is_an_ordinary_commit ... ok
test a_settled_change_is_one_observation ... ok
test an_instant_reports_boundary_fired_and_final ... ok
test previous_is_the_value_before_the_microstep ... ok
test settle_records_carry_the_last_microstep_index ... ok
test start_at_commits_init_at_its_tick ... ok
test start_commits_inits_and_fires_start ... ok
test the_traced_instant_of_section_15_1 ... ok
test two_commits_to_one_net_settle_to_one_write ... ok
test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/instant.rs (/var/tmp/piperine-target-d/debug/deps/instant-cf6184e72773bc7c)
```

`Run numerics` - exit 0

```
running 5 tests
test the_power_law_integral_is_exact_on_power_laws ... ok
test a_zero_density_takes_the_plain_rectangle ... ok
test a_term_linearization_carries_its_unknown_and_noise_partials ... ok
test a_stimulus_column_carries_its_partials_and_phase ... ok
test the_power_law_integral_tracks_a_two_pole_sum_against_a_fine_trapezoid ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/noise_numerics.rs (/var/tmp/piperine-target-d/debug/deps/noise_numerics-8567472147f9a2de)
```

`Run abinoise` - exit 0

```
running 4 tests
test a_resistor_divider_has_its_thermal_noise_on_devices ... ok
test test_per_source_noise_reporting_and_conservation ... ok
test its_devices_pass_the_partials_verifier ... ok
test a_shared_noise_input_cancels_across_the_pair ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/abi_noise.rs (/var/tmp/piperine-target-d/debug/deps/abi_noise-e5410922ed420bd0)
```
