# unknown-semantics-events proofs

**HEAD**: 3ed72693
**Tree**: clean
**Shell**: /bin/sh
**Result**: 60 pass, 10 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sourceevents / test an_event_read_as_a_bool_says_whether_it_fired ... ok` | pass |
| C2 | `run sourceevents / test two_events_in_one_microstep_run_the_reaction_once_both_true ... ok` | pass |
| C3 | `run sourceevents / test pfd3_rising_together_leaves_state_unchanged ... ok` | **no test ran** |
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
| C15 | `run prototran / test the_step_after_a_settle_commit_is_exempt_and_restarts ... ok` | pass |
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
| C23 | `run typesevents / test a_function_returning_event_is_accepted_in_every_event_position ... ok` | **no test ran** |
| C23 | `run sourceevents / test an_event_function_in_every_event_position ... ok` | **no test ran** |
| C24 | `run sourceevents / test handshake_fires_when_req_rises_with_ack_high ... ok` | **no test ran** |
| C25 | `run sourceevents / test an_event_function_read_as_a_bool_says_whether_it_fired ... ok` | **no test ran** |
| C26 | `run typesevents / test a_function_returning_event_outside_an_event_position_is_e0441 ... ok` | **no test ran** |
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
| C36 | `run docs / test gaps_no_longer_lists_l51_or_l53 ... ok` | **no test ran** |
| C37 | `run docs / test the_book_no_longer_says_settle_is_not_compiled ... ok` | pass |
| C38 | `run sourceevents / test a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record ... ok` | pass |
| C39 | `run sourceevents / test settle_runs_once_per_time_point_across_a_jump ... ok` | pass |
| C40 | `run sourceevents / test an_event_function_calling_another_fires_under_the_outer_guard ... ok` | **no test ran** |
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
| C45 | `run elabdigital / test an_event_function_call_lowers_to_its_body_event ... ok` | **no test ran** |
| C46 | `run bookdigital / test the_phase_detector_tells_its_two_edges_apart ... ok` | **no test ran** |
| C46 | `run bookvalues / test a_function_has_the_class_its_body_gives ... ok` | pass |
| C46 | `run bookmixed / test the_comparator_without_hysteresis_reads_its_event_as_a_bool ... ok` | pass |
| C46 | `run bookinventory / test every_block_of_the_book_has_one_row ... ok` | pass |
| C46 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run sourceevents` - exit 0

```
test a_committed_event_read_commits_whether_it_fired ... ok
test settle_in_a_design_with_nothing_analog_runs_at_the_operating_point_and_the_stop ... ok
test two_events_in_one_microstep_run_the_reaction_once_both_true ... ok
test settle_does_not_run_at_an_event_time_that_is_not_a_time_point ... ok
test settle_combined_with_an_event_fires_by_rule_1 ... ok
test settle_runs_once_per_time_point_across_a_jump ... ok
test settle_runs_once_per_accepted_time_point ... ok
test settle_records_follow_every_microstep_of_their_time_point ... ok
test a_time_point_with_no_settle_reaction_adds_no_microstep_and_no_record ... ok
test settle_runs_at_the_operating_point_each_dc_point_and_the_last_instant ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.07s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_events.rs (/var/tmp/piperine-target-a/debug/deps/source_events-1acdcd051b419339)
```

`Run typesevents` - exit 0

```
test emit_is_typed_in_a_reaction ... ok
test an_event_reads_as_a_bool_in_a_reaction_body ... ok
test an_operand_of_another_type_in_a_commit_is_e0432 ... ok
test event_expressions_are_typed ... ok
test emit_where_it_is_not_allowed_is_e0601 ... ok
test cross_takes_ttol_and_etol_by_position_and_by_name ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.33s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-ir)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-plan)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-lang)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 27.62s
     Running tests/types_events.rs (/var/tmp/piperine-target-a/debug/deps/types_events-8fe76880d327cc8d)
```

`Run spectext` - exit 0

```
test gnd_and_the_implicit_params_are_core_names ... ok
test the_commit_of_an_event_read_as_a_bool_is_stated ... ok
test tran_in_fences_is_the_module ... ok
test s36_defines_the_package_codes ... ok
test s3_states_the_intrinsic_contract ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test zero_bytes_are_the_unknown_value ... ok
test every_error_code_is_defined_exactly_once ... ok
test no_spec_text_says_stimulus_is_host_only ... ok

test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.89s
     Running tests/spec_text.rs (/var/tmp/piperine-target-a/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run prototran` - exit 0

```
test an_event_time_before_the_next_time_point_reads_the_old_value ... ok
test an_off_grid_step_lands_on_the_tick_grid ... ok
test crossing_retries_count_toward_the_rejection_cap ... ok
test no_landing_means_one_attempt_to_the_proposed_time ... ok
test no_unknowns_means_no_attempt ... ok
test settle_runs_at_time_points_and_at_the_stop_time_only ... ok
test the_proposal_sees_the_next_due_time ... ok
test the_step_after_a_settle_commit_is_exempt_and_restarts ... ok
test consecutive_rejections_beyond_the_cap_fail_loud ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.52s
     Running tests/protocol_transient.rs (/var/tmp/piperine-target-a/debug/deps/protocol_transient-d72a4e415285a9b3)
```

`Run instant` - exit 0

```
test a_merge_commit_the_layout_cannot_merge_is_a_run_error_naming_the_net ... ok
test a_settled_change_is_one_observation ... ok
test previous_is_the_value_before_the_microstep ... ok
test settle_records_carry_the_last_microstep_index ... ok
test an_instant_reports_boundary_fired_and_final ... ok
test start_at_commits_init_at_its_tick ... ok
test two_commits_to_one_net_settle_to_one_write ... ok
test start_commits_inits_and_fires_start ... ok
test the_traced_instant_of_section_15_1 ... ok
test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/instant.rs (/var/tmp/piperine-target-a/debug/deps/instant-cf6184e72773bc7c)
```

`Run checkpoint` - exit 0

```
running 9 tests
test a_restore_brings_back_the_held_settle_commits ... ok
test a_restore_takes_back_a_landing ... ok
test every_change_commit_and_record_is_observed_in_order ... ok
test observations_after_a_checkpoint_are_gone_after_restore ... ok
test nested_marks_rewind_in_order ... ok
test restore_returns_store_queue_and_observations_byte_for_byte ... ok
test two_runs_give_identical_observations ... ok
test a_mark_journals_only_the_nets_that_changed ... ok
test a_rewind_equals_the_checkpoint_taken_at_the_mark ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/checkpoint.rs (/var/tmp/piperine-target-a/debug/deps/checkpoint-bffa3fb7b4b6a608)
```

`Run protoop` - exit 0

```
running 6 tests
test a_settle_commit_of_a_dc_point_lands_at_the_next_point ... ok
test an_operating_point_without_unknowns_runs_the_instant_only ... ok
test a_settle_commit_of_the_last_point_is_dropped ... ok
test rounds_beyond_the_bound_are_e0506_naming_the_nets ... ok
test finish_at_the_operating_point_ends_the_run ... ok
test the_operating_point_follows_the_call_sequence ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 0.85s
     Running tests/protocol_operating_point.rs (/var/tmp/piperine-target-a/debug/deps/protocol_operating_point-79ca27345f262a89)
```

`Run typesanalog` - exit 0

```
test a_contribution_that_is_not_f64_is_e0417 ... ok
test a_contribution_target_that_is_no_probe_is_e0410 ... ok
test potential_probes_are_f64 ... ok
test limit_checks_its_input_function_and_seed ... ok
test a_core_primitive_in_a_fn_is_e0450 ... ok
test ddt_ac_stim_and_environment_are_f64 ... ok
test limexp_checks_with_its_closure_bindings ... ok
test the_std_vt_checks_clean ... ok
test freq_analysis_kinds_and_bound_step_type ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.48s
     Running tests/types_analog.rs (/var/tmp/piperine-target-a/debug/deps/types_analog-a4abdab24d7cd951)
```

`Run sourcelimexp` - exit 0

```
running 5 tests
test limexp_elaborates_from_the_stdlib_with_its_closure ... ok
test the_derivative_of_limexp_is_limexp ... ok
test limexp_moves_its_argument_at_most_one_per_iteration ... ok
test the_limexp_diode_series_twin_agrees_with_ngspice ... ok
test limexp_and_pnjlim_diodes_agree_at_the_operating_point ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.08s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_limexp.rs (/var/tmp/piperine-target-a/debug/deps/source_limexp-e6385f079ae4c5dd)
```

`Run bookanalog` - exit 0

```
test e0507_a_non_finite_contribution_is_a_run_error_naming_it ... ok
test the_thermal_voltage_is_about_25_9_millivolts_at_the_default_temperature ... ok
test a_louder_second_diode_clips_the_negative_half_higher ... ok
test a_source_with_ac_stim_is_its_dc_value_and_a_one_volt_phasor_in_the_sweep ... ok
test the_clipper_prints_one_clip_record_per_period_near_0_72_volts ... ok
test e0417_a_contribution_is_an_f64_and_the_fix_runs ... ok
test the_sampling_time_constant_of_a_switch_is_a_tenth_of_a_nanosecond ... ok
test the_clipper_clips_near_the_forward_voltage_and_the_clip_record_confirms_the_hand_calculation ... ok
test the_nullor_drives_its_output_to_a_virtual_short_and_coupled_inductors_couple ... ok
test the_switch_shorts_its_nodes_while_its_control_is_high ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.56s

    Finished `test` profile [unoptimized] target(s) in 0.24s
     Running tests/book/analog.rs (/var/tmp/piperine-target-a/debug/deps/book_analog-ea4e99d19fab749c)
```

`Run bookmixed` - exit 0

```
test e0504_the_step_after_a_jump_that_does_not_converge_names_the_carrier ... ok
test the_digital_to_analog_converter_drives_half_the_supply_for_unknown ... ok
test the_comparator_without_hysteresis_reads_its_event_as_a_bool ... ok
test the_sar_adc_runs_with_its_comparator_and_its_field_access ... ok
test the_delta_sigma_modulator_averages_its_input_and_starts_held_at_zero ... ok
test the_cycle_counter_counts_the_located_crossings_of_the_sine ... ok
test a_faster_integral_gain_makes_the_buck_loop_swing ... ok
test the_buck_starts_from_the_operating_point_with_the_first_tick_applied ... ok
test the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28 ... ok
test the_math_methods_and_the_settle_event_run ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.89s

    Finished `test` profile [unoptimized] target(s) in 0.26s
     Running tests/book/mixed.rs (/var/tmp/piperine-target-a/debug/deps/book_mixed-518cb5d34563f444)
```

`Run clibook` - exit 0

```
test the_cycle_counter_prints_three_pulses ... ok
test the_chapter_shows_the_session_options ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 207.70s

    Finished `test` profile [unoptimized] target(s) in 0.45s
     Running tests/book.rs (/var/tmp/piperine-target-a/debug/deps/book-a1f18a8423c7ed6b)
```

`Run paritytran` - exit 0

```
test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok
test tran_jump ... ok
test uic_rc ... ok
test tran_floor ... ok
test tran_rc_with_a_scaled_derivative ... ok
test tran_corner ... ok
test tran_clipper ... ok
test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.46s

    Finished `test` profile [unoptimized] target(s) in 0.33s
     Running tests/parity_tran.rs (/var/tmp/piperine-target-a/debug/deps/parity_tran-1f68c93d92106e3e)
```

`Run docs` - exit 0

```
test state_records_the_observation_decision ... ok
test state_records_the_derived_param_decision ... ok
test the_roadmap_and_state_record_analog_structure ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test the_roadmap_and_state_record_root_e2e ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_roadmap_and_state_record_msl_fixes ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized] target(s) in 0.34s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```

`Run irdigital` - exit 0

```
test the_unknown_nodes_type_print_and_check ... ok
test carriers_and_drives_print ... ok
test a_carrier_initial_value_outside_its_type_is_an_entry ... ok
test analog_reads_a_carrier_through_level_and_same_only ... ok
test events_and_reactions_print ... ok
test integers_and_their_operators_print ... ok
test the_m1_prints_are_unchanged ... ok
test unknown_values_and_their_reads_print ... ok
test the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-ir)
    Finished `test` profile [unoptimized] target(s) in 7.36s
     Running tests/digital.rs (/var/tmp/piperine-target-a/debug/deps/digital-b1250f05f42d82d3)
```

`Run interp` - exit 0

```
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_unknown_nodes_are_refused_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 16.25s
     Running tests/frozen.rs (/var/tmp/piperine-target-a/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run planshape` - exit 0

```
test an_event_function_reading_another_event_runs_after_it ... ok
test a_reaction_lists_its_events_reads_and_statements ... ok
test an_event_is_an_event_pin_and_its_function ... ok
test every_record_layout_ends_with_maybe ... ok
test an_if_lists_its_then_statement_before_its_else ... ok
test a_drive_is_a_discrete_function ... ok
test only_a_literal_positive_delay_is_later ... ok
test carriers_are_discrete_pins_with_a_known_byte ... ok
test every_buck_slice_shape_validates ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-plan)
    Finished `test` profile [unoptimized] target(s) in 0.98s
     Running tests/digital_shape.rs (/var/tmp/piperine-target-a/debug/deps/digital_shape-3b66a9e5414938d1)
```

`Run reaction` - exit 0

```
test reactions_and_drives_read_the_environment_as_analog_code_does ... ok
test a_reaction_commits_in_source_order ... ok
test an_unknown_condition_merges_both_arms ... ok
test a_settle_event_function_fires_on_the_settle_pin_and_its_guard ... ok
test level_merge_same_and_recast_compute_their_tables ... ok
test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok
test a_known_or_over_an_unwritten_input_gives_the_default ... ok
test an_unknown_delay_commits_unknown_at_the_next_microstep ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.47s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-interp)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 1.05s
     Running tests/reaction.rs (/var/tmp/piperine-target-a/debug/deps/reaction-762b225443b4d4ef)
```

`Run elabdigital` - exit 0

```
test settle_lowers_to_the_settle_event_source ... ok
test a_reaction_body_lowers_in_source_order ... ok
test a_cross_tolerance_that_is_not_constant_is_p0001 ... ok
test every_signal_carrier_is_a_design_net ... ok
test carriers_follow_ports_then_signals ... ok
test the_m1_goldens_are_unchanged ... ok
test every_ir_passes_check ... ok
test the_design_is_not_written ... ok
test elaborating_twice_gives_the_same_print ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.90s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics-events/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.81s
     Running tests/elab_digital.rs (/var/tmp/piperine-target-a/debug/deps/elab_digital-421f4441f369eca2)
```

`Run bookdigital` - exit 0

```
test the_counter_raises_full_exactly_while_the_count_is_255 ... ok
test without_its_delay_the_book_latch_is_a_loop_e0339 ... ok
test the_check_passes_every_cycle_and_ends_the_run_at_the_twentieth ... ok
test the_library_declares_the_monitors_and_never_runs ... ok
test e0501_a_clock_with_no_delay_never_goes_quiet ... ok
test the_debouncer_flips_once_at_the_fourth_clean_sample ... ok
test the_pwm_latches_its_duty_word_at_the_wrap ... ok
test the_set_reset_latch_needs_no_reaction ... ok
test a_pipeline_in_one_reaction_reads_every_register_before_the_edge ... ok
test the_emit_prints_the_same_first_in_the_middle_or_last ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.30s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book/digital.rs (/var/tmp/piperine-target-a/debug/deps/book_digital-9dc58690984a778f)
```

`Run bookvalues` - exit 0

```
test an_extern_function_is_refused_with_or_without_its_derivative ... ok
test e0901_there_is_no_println ... ok
test the_flag_is_read_with_known_known_or_and_is_known ... ok
test a_message_with_values_is_a_record_with_a_text_field ... ok
test after_schedules_the_new_value_a_delay_later_and_each_toggle_fires_the_reaction_again ... ok
test a_function_has_the_class_its_body_gives ... ok
test analog_reads_a_digital_carrier_through_known_or ... ok
test blinky_counts_three_rising_edges_in_three_microseconds ... ok
test a_param_has_a_default_a_check_and_a_binding_but_no_assignment ... ok
test the_monitor_file_runs_its_window_monitor_and_its_adder ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.83s

    Finished `test` profile [unoptimized] target(s) in 0.30s
     Running tests/book/values.rs (/var/tmp/piperine-target-a/debug/deps/book_values-8d0f784b66e62348)
```

`Run bookinventory` - exit 0

```
test every_gaps_row_names_its_owner ... ok
test every_block_of_the_book_has_one_row ... ok
test the_owner_and_disputed_rules_name_what_they_break ... ok
test no_block_row_is_pending ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test disputed_rows_are_listed_for_the_user ... ok
test run_rows_name_tests_that_run_their_model ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-fa5abaa513c18484)
```
