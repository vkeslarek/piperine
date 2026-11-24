# unknown-semantics-events proofs

**HEAD**: 9deffe77
**Tree**: clean
**Shell**: /bin/sh
**Result**: 70 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sourceevents / test an_event_read_as_a_bool_says_whether_it_fired ... ok` | pass |
| C2 | `run sourceevents / test two_events_in_one_microstep_run_the_reaction_once_both_true ... ok` | pass |
| C3 | `run sourceevents / test pfd3_rising_together_leaves_state_unchanged ... ok` | pass |
| C4 | `run sourceevents / test a_settle_reaction_reads_an_event_as_true_only_when_settle_fires_it ... ok` | pass |
| C5 | `run sourceevents / test a_guard_reading_events_fires_only_when_d_changed_with_en_high_and_still ... ok` | pass |
| C6 | `run sourceevents / test a_guard_reads_what_the_reaction_would_read ... ok` | pass |
| C7 | `run typesevents / test negating_an_event_outside_a_guard_or_body_is_e0440 ... ok` | pass |
| C8 | `run typesevents / test a_bool_and_an_event_in_an_event_position_is_e0440 ... ok` | pass |
| C9 | `run typesevents / test an_event_where_another_type_is_expected_is_e0441 ... ok` | pass |
| C10 | `run sourceevents / test a_committed_event_read_commits_whether_it_fired ... ok` | pass |
| C11 | `run spectext / test the_commit_of_an_event_read_as_a_bool_is_stated ... ok` | pass |
| C12 | `run sourceevents / test settle_runs_once_per_accepted_time_point ... ok` | pass |
| C13 | `run sourceevents / test settle_does_not_run_at_an_event_time_that_is_not_a_time_point ... ok` | pass |
| C14 | `run sourceevents / test a_settle_commit_is_read_at_the_next_time_point ... ok` | pass |
| C14 | `run prototran / test a_settle_commit_lands_at_the_next_time_point ... ok` | pass |
| C15 | `run prototran / test the_step_after_a_settle_commit_is_a_landing_and_restarts ... ok` | pass |
| C16 | `run prototran / test an_event_time_before_the_next_time_point_reads_the_old_value ... ok` | pass |
| C17 | `run instant / test a_settle_commit_with_an_advancing_delay_is_an_ordinary_commit ... ok` | pass |
| C17 | `run sourceevents / test a_delayed_settle_commit_is_ordinary ... ok` | pass |
| C18 | `run prototran / test a_rejected_step_moves_the_settle_commit ... ok` | pass |
| C18 | `run checkpoint / test a_restore_brings_back_the_held_settle_commits ... ok` | pass |
| C19 | `run prototran / test a_settle_commit_at_the_stop_time_is_dropped ... ok` | pass |
| C19 | `run sourceevents / test a_settle_commit_at_the_stop_time_is_dropped_from_source ... ok` | pass |
| C20 | `run instant / test settle_records_carry_the_last_microstep_index ... ok` | pass |
| C20 | `run sourceevents / test settle_records_follow_every_microstep_of_their_time_point ... ok` | pass |
| C21 | `run sourceevents / test settle_runs_at_the_operating_point_each_dc_point_and_the_last_instant ... ok` | pass |
| C21 | `run protoop / test a_settle_commit_of_a_dc_point_lands_at_the_next_point ... ok` | pass |
| C22 | `run sourceevents / test settle_combined_with_an_event_fires_by_rule_1 ... ok` | pass |
| C22 | `run instant / test a_reaction_in_the_settle_phase_is_on_settle_whatever_its_head ... ok` | pass |
| C23 | `run typesevents / test a_function_returning_event_is_accepted_in_every_event_position ... ok` | pass |
| C23 | `run sourceevents / test an_event_function_in_every_event_position ... ok` | pass |
| C24 | `run sourceevents / test handshake_fires_when_req_rises_with_ack_high ... ok` | pass |
| C25 | `run sourceevents / test an_event_function_read_as_a_bool_says_whether_it_fired ... ok` | pass |
| C26 | `run typesevents / test a_function_returning_event_outside_an_event_position_is_e0441 ... ok` | pass |
| C27 | `run typesanalog / test limexp_checks_with_its_closure_bindings ... ok` | pass |
| C27 | `run sourcelimexp / test limexp_elaborates_from_the_stdlib_with_its_closure ... ok` | pass |
| C28 | `run sourcelimexp / test limexp_and_pnjlim_diodes_agree_at_the_operating_point ... ok` | pass |
| C29 | `run sourcelimexp / test limexp_moves_its_argument_at_most_one_per_iteration ... ok` | pass |
| C30 | `run sourcelimexp / test the_derivative_of_limexp_is_limexp ... ok` | pass |
| C31 | `run sourcelimexp / test the_limexp_diode_series_twin_agrees_with_ngspice ... ok` | pass |
| C32 | `run bookanalog / test the_clipper_prints_one_clip_record_per_period_near_0_72_volts ... ok` | pass |
| C33 | `run bookmixed / test the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28 ... ok` | pass |
| C34 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C34 | `run clibook / test the_clipper_runs_as_the_book_prints ... ok` | pass |
| C35 | `run paritytran / test tran_clipper ... ok` | pass |
| C36 | `run docs / test gaps_no_longer_lists_settle_or_limexp ... ok` | pass |
| C36 | `run docs / test gaps_no_longer_lists_l51_or_l53 ... ok` | pass |
| C37 | `run docs / test the_book_no_longer_says_settle_is_not_compiled ... ok` | pass |
| C38 | `run sourceevents / test a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record ... ok` | pass |
| C39 | `run sourceevents / test settle_runs_once_per_time_point_across_a_jump ... ok` | pass |
| C40 | `run sourceevents / test an_event_function_calling_another_fires_under_the_outer_guard ... ok` | pass |
| C41 | `run sourceevents / test a_probing_settle_reaction_reads_the_time_point_solution ... ok` | pass |
| C42 | `run sourceevents / test settle_in_a_design_with_nothing_analog_runs_at_the_operating_point_and_the_stop ... ok` | pass |
| C43 | `run sourceevents / test an_event_not_in_the_head_reads_whether_it_fired ... ok` | pass |
| C44 | `run irdigital / test a_settle_event_is_built_checked_and_printed ... ok` | pass |
| C44 | `run interp / test a_settle_event_is_refused_by_name ... ok` | pass |
| C44 | `run planshape / test a_settle_event_reads_the_global_settle_pin ... ok` | pass |
| C44 | `run reaction / test a_settle_event_function_fires_on_the_settle_pin_and_its_guard ... ok` | pass |
| C44 | `run typesevents / test settle_is_an_event_in_every_event_position ... ok` | pass |
| C44 | `run elabdigital / test settle_lowers_to_the_settle_event_source ... ok` | pass |
| C45 | `run elabdigital / test an_event_read_as_a_bool_in_a_body_lowers_to_fired ... ok` | pass |
| C45 | `run planshape / test an_event_function_reading_another_event_runs_after_it ... ok` | pass |
| C45 | `run elabdigital / test an_event_function_call_lowers_to_its_body_event ... ok` | pass |
| C46 | `run bookdigital / test the_phase_detector_runs_assignment_and_events_read_as_bools ... ok` | pass |
| C46 | `run bookvalues / test a_function_has_the_class_its_body_gives ... ok` | pass |
| C46 | `run bookmixed / test the_comparator_without_hysteresis_reads_its_event_as_a_bool ... ok` | pass |
| C46 | `run bookinventory / test every_block_of_the_book_has_one_row ... ok` | pass |
| C46 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run sourceevents` - exit 0

```
test a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record ... ok
test settle_runs_once_per_accepted_time_point ... ok
test two_events_in_one_microstep_run_the_reaction_once_both_true ... ok
test settle_records_follow_every_microstep_of_their_time_point ... ok
test settle_runs_once_per_time_point_across_a_jump ... ok
test settle_runs_at_the_operating_point_each_dc_point_and_the_last_instant ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 23.30s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-lang)
   Compiling piperine-project v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-project)
   Compiling piperine-api v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-api)
   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-parity)
    Finished `test` profile [unoptimized] target(s) in 28.69s
     Running tests/source_events.rs (/var/tmp/piperine-target-b/debug/deps/source_events-1acdcd051b419339)
```

`Run typesevents` - exit 0

```
test negating_an_event_outside_a_guard_or_body_is_e0440 ... ok
test an_event_outside_an_event_position_is_e0441 ... ok
test settle_outside_an_event_position_is_e0441 ... ok
test an_event_reads_as_a_bool_in_a_reaction_body ... ok
test cross_or_at_outside_an_event_position_is_e0443 ... ok
test event_expressions_are_typed ... ok
test an_operand_of_another_type_in_a_commit_is_e0432 ... ok
test emit_where_it_is_not_allowed_is_e0601 ... ok
test cross_takes_ttol_and_etol_by_position_and_by_name ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.79s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.81s
     Running tests/types_events.rs (/var/tmp/piperine-target-b/debug/deps/types_events-8fe76880d327cc8d)
```

`Run spectext` - exit 0

```
test s36_defines_the_package_codes ... ok
test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok
test the_library_declares_no_net_nature ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test zero_bytes_are_the_default_value ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok
test the_removed_words_are_absent_from_the_specification ... ok

test result: ok. 137 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 1.33s
     Running tests/spec_text.rs (/var/tmp/piperine-target-b/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run prototran` - exit 0

```
test no_landing_means_one_attempt_to_the_proposed_time ... ok
test a_resume_off_the_grid_steps_on_from_its_tick ... ok
test no_unknowns_means_no_attempt ... ok
test crossing_retries_count_toward_the_rejection_cap ... ok
test settle_runs_at_time_points_and_at_the_stop_time_only ... ok
test the_proposal_sees_the_next_due_time ... ok
test the_step_after_a_settle_commit_is_a_landing_and_restarts ... ok
test consecutive_rejections_beyond_the_cap_fail_loud ... ok
test its_devices_pass_the_partials_verifier ... ok
test an_unsettled_loop_is_e0511_naming_its_drives ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/protocol_transient.rs (/var/tmp/piperine-target-b/debug/deps/protocol_transient-d72a4e415285a9b3)
```

`Run instant` - exit 0

```
test a_reopened_instant_settles_at_its_last_microstep ... ok
test an_instant_reports_boundary_fired_and_final ... ok
test previous_is_the_value_before_the_microstep ... ok
test settle_records_carry_the_last_microstep_index ... ok
test start_commits_inits_and_fires_start ... ok
test start_at_commits_init_at_its_tick ... ok
test the_last_plain_commit_of_a_microstep_is_the_value ... ok
test two_commits_to_one_net_settle_to_one_write ... ok
test the_traced_instant_of_section_15_1 ... ok
test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/instant.rs (/var/tmp/piperine-target-b/debug/deps/instant-cf6184e72773bc7c)
```

`Run checkpoint` - exit 0

```
running 9 tests
test a_restore_takes_back_a_landing ... ok
test a_restore_brings_back_the_held_settle_commits ... ok
test restore_returns_store_queue_and_observations_byte_for_byte ... ok
test every_change_commit_and_record_is_observed_in_order ... ok
test nested_marks_rewind_in_order ... ok
test two_runs_give_identical_observations ... ok
test observations_after_a_checkpoint_are_gone_after_restore ... ok
test a_mark_journals_only_the_nets_that_changed ... ok
test a_rewind_equals_the_checkpoint_taken_at_the_mark ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/checkpoint.rs (/var/tmp/piperine-target-b/debug/deps/checkpoint-bffa3fb7b4b6a608)
```

`Run protoop` - exit 0

```
running 6 tests
test a_settle_commit_of_a_dc_point_lands_at_the_next_point ... ok
test a_settle_commit_of_the_last_point_is_dropped ... ok
test an_operating_point_without_unknowns_runs_the_instant_only ... ok
test rounds_beyond_the_bound_are_e0506_naming_the_nets ... ok
test the_operating_point_follows_the_call_sequence ... ok
test finish_at_the_operating_point_ends_the_run ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/protocol_operating_point.rs (/var/tmp/piperine-target-b/debug/deps/protocol_operating_point-79ca27345f262a89)
```

`Run typesanalog` - exit 0

```
test a_contribution_target_that_is_no_probe_is_e0410 ... ok
test a_contribution_that_is_not_f64_is_e0417 ... ok
test potential_probes_are_f64 ... ok
test a_core_primitive_in_a_fn_is_e0450 ... ok
test limit_checks_its_input_function_and_seed ... ok
test ddt_ac_stim_and_environment_are_f64 ... ok
test the_std_vt_checks_clean ... ok
test limexp_checks_with_its_closure_bindings ... ok
test freq_analysis_kinds_and_bound_step_type ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.57s
     Running tests/types_analog.rs (/var/tmp/piperine-target-b/debug/deps/types_analog-a4abdab24d7cd951)
```

`Run sourcelimexp` - exit 0

```
running 5 tests
test limexp_elaborates_from_the_stdlib_with_its_closure ... ok
test the_derivative_of_limexp_is_limexp ... ok
test limexp_moves_its_argument_at_most_one_per_iteration ... ok
test the_limexp_diode_series_twin_agrees_with_ngspice ... ok
test limexp_and_pnjlim_diodes_agree_at_the_operating_point ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.39s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-parity)
    Finished `test` profile [unoptimized] target(s) in 0.76s
     Running tests/source_limexp.rs (/var/tmp/piperine-target-b/debug/deps/source_limexp-e6385f079ae4c5dd)
```

`Run bookanalog` - exit 0

```
test the_clipper_prints_one_clip_record_per_period_near_0_72_volts ... ok
test a_source_with_ac_stim_is_its_dc_value_and_a_one_volt_phasor_in_the_sweep ... ok
test the_thermal_voltage_is_about_25_9_millivolts_at_the_default_temperature ... ok
test the_clipper_clips_near_the_forward_voltage_and_the_clip_record_confirms_the_hand_calculation ... ok
test e0417_a_contribution_is_an_f64_and_the_fix_runs ... ok
test the_nullor_drives_its_output_to_a_virtual_short_and_coupled_inductors_couple ... ok
test the_sampling_time_constant_of_a_switch_is_a_tenth_of_a_nanosecond ... ok
test the_switch_shorts_its_nodes_while_its_control_is_high ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.95s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-use)
   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 1.66s
     Running tests/book/analog.rs (/var/tmp/piperine-target-b/debug/deps/book_analog-2ef7faff28f93cd2)
```

`Run bookmixed` - exit 0

```
test the_digital_to_analog_converter_drives_the_supply_or_ground ... ok
test the_cycle_counter_counts_the_located_crossings_of_the_sine ... ok
test the_delta_sigma_modulator_averages_its_input_and_starts_held_at_zero ... ok
test a_faster_integral_gain_makes_the_buck_loop_swing ... ok
test the_math_methods_and_the_settle_event_run ... ok
test a_bit_control_in_a_division_is_read_as_its_value ... ok
test the_sar_adc_runs_with_its_comparator_and_its_field_access ... ok
test the_buck_starts_from_the_operating_point_with_the_first_tick_applied ... ok
test the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28 ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.29s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-use)
    Finished `test` profile [unoptimized] target(s) in 1.03s
     Running tests/book/mixed.rs (/var/tmp/piperine-target-b/debug/deps/book_mixed-05320ae04b238750)
```

`Run clibook` - exit 0

```
test the_chapter_shows_the_session_options ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_chapter_runs_each_new_analysis ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 56.22s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 1.12s
     Running tests/book.rs (/var/tmp/piperine-target-b/debug/deps/book-a1f18a8423c7ed6b)
```

`Run paritytran` - exit 0

```
test tran_floor ... ok
test tran_corner ... ok
test tran_clipper ... ok
test tran_jump ... ok
test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok
test uic_rc ... ok
test tran_rc_with_a_scaled_derivative ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.50s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-parity)
    Finished `test` profile [unoptimized] target(s) in 0.78s
     Running tests/parity_tran.rs (/var/tmp/piperine-target-b/debug/deps/parity_tran-1f68c93d92106e3e)
```

`Run docs` - exit 0

```
test the_buck_spec_holds_the_bench_bounds ... ok
test state_records_the_observation_decision ... ok
test transient_performance_closes_v12 ... ok
test the_buck_readme_points_at_the_scenarios ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test the_buck_readme_says_how_to_run_it ... ok
test the_roadmap_is_forward_looking ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 0.90s
     Running tests/docs.rs (/var/tmp/piperine-target-b/debug/deps/docs-93bbd657b5852db6)
```

`Run irdigital` - exit 0

```
test a_settle_event_is_built_checked_and_printed ... ok
test a_defective_digital_body_gives_one_entry_per_defect ... ok
test integer_and_bit_functions_print ... ok
test carriers_and_drives_print ... ok
test same_and_cast_type_print_and_check ... ok
test events_and_reactions_print ... ok
test no_value_type_is_flagged ... ok
test integers_and_their_operators_print ... ok
test the_m1_prints_are_unchanged ... ok
test the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.07s
     Running tests/digital.rs (/var/tmp/piperine-target-b/debug/deps/digital-b1250f05f42d82d3)
```

`Run interp` - exit 0

```
test each_generics_form_is_refused_by_name ... ok
test a_host_source_body_is_rejected_by_name ... ok
test a_table_expression_is_refused_by_name ... ok
test the_interpreter_refuses_each_analog_operator_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test the_level_nodes_are_refused_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/frozen.rs (/var/tmp/piperine-target-b/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run planshape` - exit 0

```
test a_guard_or_a_decision_adds_no_read_and_no_resolution_pin ... ok
test a_settle_event_reads_the_global_settle_pin ... ok
test an_event_is_an_event_pin_and_its_function ... ok
test every_record_layout_holds_its_declared_fields_only ... ok
test a_body_without_settle_has_no_settle_pin ... ok
test carriers_are_discrete_pins_that_hold_the_value_only ... ok
test only_a_literal_positive_delay_is_later ... ok
test a_reaction_lists_its_events_reads_and_statements ... ok
test a_drive_is_a_discrete_function ... ok
test every_buck_slice_shape_validates ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/digital_shape.rs (/var/tmp/piperine-target-b/debug/deps/digital_shape-3b66a9e5414938d1)
```

`Run reaction` - exit 0

```
running 7 tests
test reactions_and_drives_read_the_environment_as_analog_code_does ... ok
test a_commit_is_made_after_the_delay_it_names ... ok
test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok
test same_cast_and_math_compute_their_tables ... ok
test an_if_commits_through_the_arm_it_takes ... ok
test a_settle_event_function_fires_on_the_settle_pin_and_its_guard ... ok
test a_reaction_commits_in_source_order ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.59s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/reaction.rs (/var/tmp/piperine-target-b/debug/deps/reaction-762b225443b4d4ef)
```

`Run elabdigital` - exit 0

```
test every_signal_carrier_is_a_design_net ... ok
test carriers_follow_ports_then_signals ... ok
test an_event_function_binds_its_arguments_as_a_call_does ... ok
test an_event_function_chooses_between_events_by_a_value_decided_at_the_instant ... ok
test an_event_function_call_lowers_to_its_body_event ... ok
test the_m1_goldens_are_unchanged ... ok
test every_ir_passes_check ... ok
test the_design_is_not_written ... ok
test elaborating_twice_gives_the_same_print ... ok

test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.35s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-use/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.91s
     Running tests/elab_digital.rs (/var/tmp/piperine-target-b/debug/deps/elab_digital-421f4441f369eca2)
```

`Run bookdigital` - exit 0

```
test a_press_released_before_the_fourth_sample_gives_no_flip ... ok
test the_library_declares_the_monitors_and_never_runs ... ok
test the_phase_detector_runs_assignment_and_events_read_as_bools ... ok
test e0501_a_clock_with_no_delay_never_goes_quiet ... ok
test the_pwm_latches_its_duty_word_at_the_wrap ... ok
test the_set_reset_latch_needs_no_reaction ... ok
test a_pipeline_in_one_reaction_reads_every_register_before_the_edge ... ok
test the_debouncer_pwm_and_pwm_check_print_their_records ... ok
test the_emit_prints_the_same_first_in_the_middle_or_last ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.04s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-use)
    Finished `test` profile [unoptimized] target(s) in 1.73s
     Running tests/book/digital.rs (/var/tmp/piperine-target-b/debug/deps/book_digital-98cddfefc66a9ac1)
```

`Run bookvalues` - exit 0

```
test after_schedules_the_new_value_a_delay_later_and_each_toggle_fires_the_reaction_again ... ok
test a_message_with_values_is_a_record_with_a_text_field ... ok
test analog_reads_a_digital_carrier_directly ... ok
test a_function_has_the_class_its_body_gives ... ok
test e0187_a_signal_with_no_first_value_and_no_default_is_refused ... ok
test blinky_counts_three_rising_edges_in_three_microseconds ... ok
test a_param_has_a_default_a_check_and_a_binding_but_no_assignment ... ok
test an_undriven_top_level_in_port_with_no_default_is_refused ... ok
test the_monitor_file_runs_its_window_monitor_its_adder_and_its_logic_peek ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.65s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-use)
    Finished `test` profile [unoptimized] target(s) in 1.31s
     Running tests/book/values.rs (/var/tmp/piperine-target-b/debug/deps/book_values-04a3e7259033323a)
```

`Run bookinventory` - exit 0

```
test run_rows_name_tests_that_run_their_model ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test disputed_rows_are_listed_for_the_user ... ok
test no_row_diverges ... ok
test no_block_row_is_pending ... ok
test every_listing_that_does_not_run_names_its_gap ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok
test the_book_uses_no_removed_name ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

   Compiling piperine v0.2.0 (/home/keslarek/Git/piperine-use)
    Finished `test` profile [unoptimized] target(s) in 1.97s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-b/debug/deps/book_inventory-1a21c47b4e1a6a60)
```
