# resolved-nets proofs

**HEAD**: 73cef16b
**Tree**: clean
**Shell**: /bin/sh
**Result**: 25 pass, 49 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sourcenatures / test a_counter_over_a_project_byte_nature_commits_and_records ... ok` | pass |
| C2 | `run sourcenatures / test a_project_nature_carrier_connects_to_a_port_of_the_same_nature ... ok` | pass |
| C3 | `run typesnatures / test a_project_discrete_port_without_a_direction_is_e0310 ... ok` | pass |
| C4 | `run typesnatures / test connecting_byte_to_bit_u8_is_e0251_naming_both_natures ... ok` | pass |
| C5 | `run typesnatures / test a_nature_value_without_repr_is_e0112 ... ok` | pass |
| C6 | `run typesnatures / test a_second_driver_of_a_project_discrete_carrier_is_e0220 ... ok` | pass |
| C7 | `run typesnatures / test a_generic_project_nature_takes_its_argument_or_its_default ... ok` | pass |
| C8 | `run typesnatures / test a_const_generic_resolve_over_an_array_is_accepted ... ok` | **no test ran** |
| C9 | `run typesnatures / test resolve_may_name_an_associated_function_or_a_const ... ok` | **no test ran** |
| C10 | `run typesnatures / test a_resolve_of_another_shape_is_e0203 ... ok` | **no test ran** |
| C11 | `run typesnatures / test resolve_without_value_or_with_potential_is_e0201 ... ok` | **no test ran** |
| C12 | `run typesnatures / test a_direction_on_a_resolved_port_is_e0310 ... ok` | **no test ran** |
| C13 | `run sourceresolved / test an_initial_value_is_the_declaring_modules_first_drive ... ok` | **fail** |
| C14 | `run elabnatures / test resolve_is_instantiated_once_per_drive_count ... ok` | **no test ran** |
| C15 | `run sourceresolved / test three_sumnet_drivers_give_their_sum_in_the_microstep_any_changes ... ok` | **fail** |
| C16 | `run sourceresolved / test an_undriven_resolved_carrier_holds_resolve_of_no_drive ... ok` | **fail** |
| C17 | `run sourceresolved / test resolve_receives_the_drives_in_instance_path_order ... ok` | **fail** |
| C18 | `run sourceresolved / test a_drive_without_initial_value_is_unknown_until_its_first_commit ... ok` | **fail** |
| C19 | `run sourceresolved / test a_drive_change_that_leaves_the_value_fires_no_changed ... ok` | **fail** |
| C20 | `run typesnatures / test a_definition_and_a_commit_of_one_resolved_carrier_is_e0221 ... ok` | **no test ran** |
| C21 | `run sourceresolved / test others_is_resolve_of_every_drive_but_the_readers ... ok` | **fail** |
| C22 | `run sourceresolved / test others_of_a_module_without_a_drive_is_the_value ... ok` | **fail** |
| C23 | `run typesnatures / test others_on_a_one_driver_carrier_is_e0224 ... ok` | pass |
| C24 | `run elabnatures / test others_depends_on_the_other_drives_only ... ok` | **no test ran** |
| C25 | `run sourceresolved / test a_loop_of_resolved_drives_is_accepted_and_settles ... ok` | **fail** |
| C26 | `run relax / test a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes ... ok` | pass |
| C27 | `run sourcenets / test two_parallel_trans_carry_a_strong_level_and_stay_two_carriers ... ok` | **fail** |
| C28 | `run sourceresolved / test a_loop_that_does_not_settle_is_unknown_and_logged_once ... ok` | **fail** |
| C29 | `run elabnatures / test a_loop_through_a_one_driver_carrier_across_instances_is_e0339 ... ok` | **no test ran** |
| C30 | `run relax / test a_released_loop_keeps_no_earlier_level ... ok` | pass |
| C31 | `run relax / test an_absent_drive_is_left_out_and_an_unknown_one_is_passed ... ok` | pass |
| C32 | `run sourcenets / test wired_gives_the_table_of_22_3_2 ... ok` | **fail** |
| C33 | `run sourcenets / test wired_of_equal_drives_is_that_drive_in_any_order ... ok` | **fail** |
| C34 | `run sourcenets / test tri_gives_the_table_of_22_3_3 ... ok` | **fail** |
| C35 | `run sourcenets / test tribuf_drives_strong_z_or_their_merge ... ok` | **fail** |
| C36 | `run sourcenets / test a_pullup_wins_only_while_every_stronger_drive_is_released ... ok` | **fail** |
| C37 | `run sourcenets / test a_keeper_holds_the_last_driven_level_weakly ... ok` | **fail** |
| C38 | `run sourcenets / test tran_drives_each_side_with_the_others_of_the_other ... ok` | **fail** |
| C39 | `run sourcenets / test tgate_joins_while_s_high_or_sn_low ... ok` | **fail** |
| C40 | `run sourceresolved / test resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac ... ok` | **fail** |
| C41 | `run sourcenets / test bidir_drives_the_node_while_the_wire_is_driven ... ok` | **fail** |
| C42 | `run sourcenets / test bidir_drives_the_wire_from_the_node_when_released ... ok` | **fail** |
| C43 | `run sourcenets / test bidir_acts_as_driven_under_an_unknown_strength ... ok` | **fail** |
| C44 | `run sourcenets / test a_change_of_driving_is_a_time_point_and_a_jump ... ok` | **fail** |
| C45 | `run sourcenets / test two_bidirs_alone_make_the_wire_unknown_and_log_the_loop ... ok` | **fail** |
| C46 | `run sourcenatures / test a_signal_flow_node_with_one_driver_solves_and_loads_nothing ... ok` | pass |
| C47 | `run sourcenatures / test a_gain_chain_over_vsignal_gives_the_product_of_the_gains ... ok` | pass |
| C48 | `run elabnatures / test a_signal_flow_node_with_no_or_two_drivers_is_e0242 ... ok` | pass |
| C49 | `run typesnatures / test a_flow_contribution_to_a_signal_flow_node_is_e0413 ... ok` | pass |
| C50 | `run sourcenatures / test vsignal_connected_to_an_electrical_port_is_conservative ... ok` | pass |
| C51 | `run typesnatures / test a_flow_probe_of_a_signal_flow_node_is_e0206 ... ok` | pass |
| C52 | `run booknatures / test the_alarm_prints_three_records_of_the_line ... ok` | **no test ran** |
| C53 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C54 | `run booknatures / test the_pulldown_alarm_prints_the_exercise_answer ... ok` | **no test ran** |
| C55 | `run docs / test gaps_lists_no_resolved_net_gap ... ok` | **no test ran** |
| C56 | `run spectext / test c_2_3_and_g_38_say_tran_on_wires_runs ... ok` | **no test ran** |
| C57 | `run speccodes / test no_owned_code_is_raised ... ok` | pass |
| C58 | `run irresolved / test a_resolved_net_is_built_checked_and_printed ... ok` | **fail** |
| C59 | `run interp / test resolved_forms_are_refused_by_name ... ok` | **no test ran** |
| C60 | `run planshape / test a_resolved_carrier_has_value_drive_and_others_pins ... ok` | **no test ran** |
| C61 | `run transresolve / test resolve_calls_the_instance_of_the_present_count ... ok` | **fail** |
| C62 | `run prototran / test an_unsettled_loop_reaches_the_owner_once_per_step ... ok` | pass |
| C63 | `run apirecords / test an_unsettled_loop_is_one_log_record_per_analysis ... ok` | **no test ran** |
| C64 | `run bookmodules / test the_pi_controller_is_refused_only_for_its_generic_idt ... ok` | pass |
| C65 | `run bookdigital / test the_irq_line_runs_on_its_resolved_wire ... ok` | **no test ran** |
| C66 | `run bookcorestd / test the_resolved_line_runs_on_its_wire ... ok` | **no test ran** |
| C67 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C68 | `run sourcenatures / test an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run ... ok` | pass |
| Run sourceresolved | `cargo test -p piperine-parity --test source_resolved` | **fail** - exit 101 |
| Run sourcenets | `cargo test -p piperine-parity --test source_nets` | **fail** - exit 101 |
| Run irresolved | `cargo test -p piperine-ir --test resolved` | **fail** - exit 101 |
| Run transresolve | `cargo test -p piperine-transpile --test resolve` | **fail** - exit 101 |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run sourcenatures` - exit 0

```
running 6 tests
test vsignal_connected_to_an_electrical_port_is_conservative ... ok
test a_signal_flow_node_with_one_driver_solves_and_loads_nothing ... ok
test an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run ... ok
test a_counter_over_a_project_byte_nature_commits_and_records ... ok
test a_gain_chain_over_vsignal_gives_the_product_of_the_gains ... ok
test a_project_nature_carrier_connects_to_a_port_of_the_same_nature ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.12s

    Finished `test` profile [unoptimized] target(s) in 0.24s
     Running tests/source_natures.rs (/var/tmp/piperine-target-a/debug/deps/source_natures-0e3a717a399fc10c)
```

`Run typesnatures` - exit 0

```
test a_second_driver_of_a_project_discrete_carrier_is_e0220 ... ok
test a_nature_value_without_repr_is_e0112 ... ok
test connecting_byte_to_bit_u8_is_e0251_naming_both_natures ... ok
test a_project_discrete_port_without_a_direction_is_e0310 ... ok
test a_flow_probe_of_a_signal_flow_node_is_e0206 ... ok
test others_on_a_one_driver_carrier_is_e0224 ... ok
test a_flow_contribution_to_a_signal_flow_node_is_e0413 ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.54s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-plan)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-lang)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 32.55s
     Running tests/types_natures.rs (/var/tmp/piperine-target-a/debug/deps/types_natures-445ea08d0e7f636e)
```

`Run sourceresolved` - exit 101

```
    source_derivative
    source_environment
    source_events
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_switch
    source_time
    source_unknowns
    twins
```

`Run elabnatures` - exit 0

```
running 2 tests
test a_node_meeting_a_nature_with_flow_is_not_counted ... ok
test a_signal_flow_node_with_no_or_two_drivers_is_e0242 ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.42s
     Running tests/elab_natures.rs (/var/tmp/piperine-target-a/debug/deps/elab_natures-420739a32c6eb0c5)
```

`Run relax` - exit 0

```
running 4 tests
test an_absent_drive_is_left_out_and_an_unknown_one_is_passed ... ok
test a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes ... ok
test a_released_loop_keeps_no_earlier_level ... ok
test an_unsettled_step_is_one_observation_naming_every_relax_net ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/relax.rs (/var/tmp/piperine-target-a/debug/deps/relax-b90cf2f70ae93855)
```

`Run sourcenets` - exit 101

```
error: no test target named `source_nets` in `piperine-parity` package

help: a target with a similar name exists: `source_events`
```

`Run booknatures` - exit 0

```
test e0222_a_module_cannot_drive_its_own_input ... ok
test e0310_a_one_driver_port_needs_a_direction ... ok
test a_bundle_is_refused_where_the_solver_reaches_it ... ok
test signals_of_every_family_are_refused_for_the_arrays_and_the_wire ... ok
test the_open_drain_line_is_refused_while_resolved_natures_are_not_compiled ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok
test an_out_port_gives_the_initial_value ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test behavior_crosses_freely_inside_one_module ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.39s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/natures.rs (/var/tmp/piperine-target-a/debug/deps/book_natures-84da6f1f33ea55da)
```

`Run clibook` - exit 0

```
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test the_chapter_shows_the_session_options ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_chapter_runs_each_new_analysis ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.31s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book.rs (/var/tmp/piperine-target-a/debug/deps/book-a1f18a8423c7ed6b)
```

`Run docs` - exit 0

```
test the_buck_spec_holds_the_bench_bounds ... ok
test gaps_drops_what_analog_structure_closed ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test gaps_drops_what_msl_fixes_closed ... ok
test the_roadmap_and_state_record_analog_structure ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test gaps_no_longer_lists_x5_l20_l39_h12 ... ok
test the_roadmap_and_state_record_root_e2e ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```

`Run spectext` - exit 0

```
test value_blocks_may_hold_contributions_and_commits ... ok
test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok
test the_commit_of_an_event_read_as_a_bool_is_stated ... ok
test s36_defines_the_package_codes ... ok
test s3_states_the_intrinsic_contract ... ok
test zero_bytes_are_the_unknown_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok

test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.91s
     Running tests/spec_text.rs (/var/tmp/piperine-target-a/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run speccodes` - exit 0

```
running 4 tests
test every_owned_code_names_its_owner_and_construct ... ok
test no_source_spells_p0002 ... ok
test no_owned_code_is_raised ... ok
test every_spec_code_is_raised_or_owned ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.30s
     Running tests/spec_codes.rs (/var/tmp/piperine-target-a/debug/deps/spec_codes-86724e06e4c5ed72)
```

`Run irresolved` - exit 101

```
error: no test target named `resolved` in `piperine-ir` package
help: available test targets:
    amendments
    check
    deps
    digital
    model
    print
    serde
    structure
```

`Run interp` - exit 0

```
test a_settle_event_is_refused_by_name ... ok
test the_unknown_nodes_are_refused_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 9.58s
     Running tests/frozen.rs (/var/tmp/piperine-target-a/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run planshape` - exit 0

```
test an_analog_read_of_a_carrier_is_a_read_of_terms ... ok
test a_cross_event_is_a_watch_with_partials ... ok
test every_record_layout_ends_with_maybe ... ok
test a_reaction_with_an_unknown_decision_reads_the_resolution ... ok
test only_a_literal_positive_delay_is_later ... ok
test a_reaction_lists_its_events_reads_and_statements ... ok
test a_drive_is_a_discrete_function ... ok
test carriers_are_discrete_pins_with_a_known_byte ... ok
test every_buck_slice_shape_validates ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-plan)
    Finished `test` profile [unoptimized] target(s) in 0.83s
     Running tests/digital_shape.rs (/var/tmp/piperine-target-a/debug/deps/digital_shape-3b66a9e5414938d1)
```

`Run transresolve` - exit 101

```
error: no test target named `resolve` in `piperine-transpile` package

help: a target with a similar name exists: `residue`
```

`Run prototran` - exit 0

```
test its_devices_pass_the_partials_verifier ... ok
test an_off_grid_step_lands_on_the_tick_grid ... ok
test no_unknowns_means_no_attempt ... ok
test no_landing_means_one_attempt_to_the_proposed_time ... ok
test the_step_after_a_settle_commit_is_exempt_and_restarts ... ok
test the_proposal_sees_the_next_due_time ... ok
test an_unsettled_loop_reaches_the_owner_once_per_step ... ok
test consecutive_rejections_beyond_the_cap_fail_loud ... ok
test settle_runs_at_time_points_and_at_the_stop_time_only ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-resolved-nets/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 2.18s
     Running tests/protocol_transient.rs (/var/tmp/piperine-target-a/debug/deps/protocol_transient-d72a4e415285a9b3)
```

`Run apirecords` - exit 0

```
test a_design_without_emit_gives_no_record ... ok
test records_arrive_in_time_order ... ok
test an_unknown_field_is_unknown ... ok
test a_reserved_stream_keeps_its_name_and_its_site ... ok
test a_failed_require_is_one_checks_record_with_pass_false ... ok
test a_text_field_is_the_sources_literal ... ok
test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok
test maybe_is_decoded_beside_the_fields ... ok
test a_run_ended_by_finish_gives_its_rows_to_the_instant ... ok
test keep_matches_a_stream_name_or_its_full_path ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.45s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/records.rs (/var/tmp/piperine-target-a/debug/deps/records-636b5e3d604fb760)
```

`Run bookmodules` - exit 0

```
test the_lowpass_module_is_refused_for_the_generic_integrator ... ok
test the_ladder_is_refused_for_its_generic_module ... ok
test the_half_reactions_are_refused_with_the_ladder ... ok
test the_memory_bank_is_refused_for_its_struct_arrays_and_generics ... ok
test the_pi_controller_is_refused_only_for_its_generic_idt ... ok
test the_book_shows_the_integrator_the_standard_library_gives ... ok
test a_module_body_takes_a_param_with_a_where_and_an_instance_with_a_param_block ... ok
test e0320_a_block_names_a_param_the_module_does_not_have ... ok
test a_ladder_written_out_runs_an_operating_point_and_a_dc_sweep ... ok
test the_switched_capacitor_integrator_is_a_netlist_that_runs ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.17s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/modules.rs (/var/tmp/piperine-target-a/debug/deps/book_modules-a4ed62e653059f7f)
```

`Run bookdigital` - exit 0

```
test without_its_delay_the_book_latch_is_a_loop_e0339 ... ok
test the_check_passes_every_cycle_and_ends_the_run_at_the_twentieth ... ok
test e0501_a_clock_with_no_delay_never_goes_quiet ... ok
test the_debouncer_flips_once_at_the_fourth_clean_sample ... ok
test the_library_declares_the_monitors_and_never_runs ... ok
test the_debouncer_is_refused_for_its_integer_param_alone ... ok
test the_set_reset_latch_needs_no_reaction ... ok
test the_pwm_latches_its_duty_word_at_the_wrap ... ok
test a_pipeline_in_one_reaction_reads_every_register_before_the_edge ... ok
test the_emit_prints_the_same_first_in_the_middle_or_last ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.95s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/digital.rs (/var/tmp/piperine-target-a/debug/deps/book_digital-98cddfefc66a9ac1)
```

`Run bookcorestd` - exit 0

```
test the_watchdog_is_refused_for_its_integer_param ... ok
test the_probe_writes_a_check_and_a_log_record ... ok
test the_rc_module_of_the_carriers_part_runs ... ok
test the_dac_ramp_scales_its_code_by_the_reference ... ok
test the_pad_drives_its_node_and_reads_it_back ... ok
test the_tour_module_runs_its_analog_and_digital_blocks ... ok
test a_crates_own_vt_replaces_the_library_one ... ok
test the_watchdog_ends_the_run_at_the_fourth_edge_with_the_button_held ... ok
test the_latch_takes_its_input_at_the_rising_edge_or_zero_when_unknown ... ok
test the_probes_have_the_short_names_the_nature_gives ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.45s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/core_std.rs (/var/tmp/piperine-target-a/debug/deps/book_core_std-732527cde9b72258)
```

`Run bookinventory` - exit 0

```
test refused_rows_name_tests_that_expect_their_refusal ... ok
test every_gaps_row_names_its_owner ... ok
test no_row_diverges ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test run_rows_name_tests_that_run_their_model ... ok
test a_model_across_blocks_names_one_test ... ok
test every_block_of_the_book_has_one_row ... ok
test a_verdict_fits_its_block ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-1a21c47b4e1a6a60)
```
