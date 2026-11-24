# resolved-nets proofs

**HEAD**: b77ca5bbb
**Tree**: clean
**Shell**: /bin/sh
**Result**: 98 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sourcenatures / test a_counter_over_a_project_byte_nature_commits_and_records ... ok` | pass |
| C2 | `run sourcenatures / test a_project_nature_carrier_connects_to_a_port_of_the_same_nature ... ok` | pass |
| C3 | `run typesnatures / test a_project_discrete_port_without_a_direction_is_e0310 ... ok` | pass |
| C4 | `run typesnatures / test connecting_byte_to_bit_u8_is_e0251_naming_both_natures ... ok` | pass |
| C5 | `run typesnatures / test a_nature_value_without_repr_is_e0112 ... ok` | pass |
| C6 | `run typesnatures / test a_second_driver_of_a_project_discrete_carrier_is_e0220 ... ok` | pass |
| C7 | `run typesnatures / test a_generic_project_nature_takes_its_argument_or_its_default ... ok` | pass |
| C8 | `run typesnatures / test an_impl_resolve_makes_a_discrete_nature_resolved ... ok` | pass |
| C9 | `run typesnatures / test resolve_may_call_a_library_or_project_function ... ok` | pass |
| C10 | `run typesnatures / test an_impl_resolve_of_another_shape_is_e0203 ... ok` | pass |
| C11 | `run typesnatures / test a_resolve_field_is_not_a_nature_field ... ok` | pass |
| C12 | `run typesnatures / test a_direction_on_a_resolved_port_is_e0310 ... ok` | pass |
| C13 | `run sourceresolved / test an_initial_value_is_the_declaring_modules_first_drive ... ok` | pass |
| C14 | `run elabnatures / test resolve_is_instantiated_once_per_drive_count ... ok` | pass |
| C15 | `run sourceresolved / test three_summing_drivers_give_their_sum_in_the_microstep_any_changes ... ok` | pass |
| C16 | `run sourceresolved / test an_undriven_resolved_carrier_holds_resolve_of_no_drive ... ok` | pass |
| C17 | `run sourceresolved / test resolve_receives_the_drives_in_instance_path_order ... ok` | pass |
| C18 | `run sourceresolved / test a_drive_without_initial_value_is_its_types_default_until_its_first_commit ... ok` | pass |
| C19 | `run sourceresolved / test a_drive_change_that_leaves_the_value_fires_no_changed ... ok` | pass |
| C20 | `run typesnatures / test a_definition_and_a_commit_of_one_resolved_carrier_is_e0221 ... ok` | pass |
| C21 | `run sourceresolved / test others_is_resolve_of_every_drive_but_the_readers ... ok` | pass |
| C22 | `run sourceresolved / test others_of_a_module_without_a_drive_is_the_value ... ok` | pass |
| C23 | `run typesnatures / test others_on_a_one_driver_carrier_is_e0224 ... ok` | pass |
| C24 | `run elabnatures / test others_depends_on_the_other_drives_only ... ok` | pass |
| C25 | `run sourceresolved / test a_loop_of_resolved_drives_is_accepted_and_settles ... ok` | pass |
| C26 | `run relax / test a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes ... ok` | pass |
| C27 | `run sourcenets / test two_parallel_trans_carry_a_strong_level_and_stay_two_carriers ... ok` | pass |
| C28 | `run sourceresolved / test a_loop_that_does_not_settle_stops_the_run_with_e0511_naming_its_drives ... ok` | pass |
| C29 | `run elabnatures / test a_loop_through_a_one_driver_carrier_across_instances_is_e0339 ... ok` | pass |
| C30 | `run relax / test a_released_loop_keeps_no_earlier_level ... ok` | pass |
| C31 | `run relax / test an_absent_drive_is_left_out_and_a_present_one_is_passed ... ok` | pass |
| C32 | `run sourcenets / test strongest_gives_the_table_of_22_3_2 ... ok` | pass |
| C33 | `run sourcenets / test strongest_of_equal_drives_is_that_drive_in_any_order ... ok` | pass |
| C34 | `run sourcenets / test a_typed_bus_resolves_to_free_byte_or_contended ... ok` | pass |
| C35 | `run sourcenets / test tribuf_drives_strong_or_high_impedance ... ok` | pass |
| C36 | `run sourcenets / test a_pull_wins_only_while_every_stronger_drive_is_released ... ok` | pass |
| C37 | `run sourcenets / test a_keeper_drives_weak_x_until_it_sees_a_level_then_holds_it ... ok` | pass |
| C38 | `run sourcenets / test tran_drives_each_side_with_the_others_of_the_other ... ok` | pass |
| C39 | `run sourcenets / test tgate_joins_while_s_high_or_sn_low ... ok` | pass |
| C40 | `run sourceresolved / test resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac ... ok` | pass |
| C41 | `run sourcenets / test bidir_drives_the_node_while_the_wire_is_driven ... ok` | pass |
| C42 | `run sourcenets / test bidir_drives_the_wire_from_the_node_when_released ... ok` | pass |
| C43 | `run sourcenets / test bidir_acts_as_driven_on_a_contended_net ... ok` | pass |
| C44 | `run sourcenets / test a_change_of_driving_is_a_time_point_and_a_jump ... ok` | pass |
| C45 | `run sourcenets / test two_bidirs_alone_stop_the_run_with_e0511 ... ok` | pass |
| C46 | `run sourcenatures / test a_signal_flow_node_with_one_driver_solves_and_loads_nothing ... ok` | pass |
| C47 | `run sourcenatures / test a_gain_chain_over_vsignal_gives_the_product_of_the_gains ... ok` | pass |
| C48 | `run elabnatures / test a_signal_flow_node_with_no_or_two_drivers_is_e0242 ... ok` | pass |
| C49 | `run typesnatures / test a_flow_contribution_to_a_signal_flow_node_is_e0413 ... ok` | pass |
| C50 | `run sourcenatures / test vsignal_connected_to_an_electrical_port_is_conservative ... ok` | pass |
| C51 | `run typesnatures / test a_flow_probe_of_a_signal_flow_node_is_e0206 ... ok` | pass |
| C52 | `run booknatures / test the_alarm_prints_three_records_of_the_line ... ok` | pass |
| C53 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C54 | `run booknatures / test the_pulldown_alarm_prints_the_exercise_answer ... ok` | pass |
| C55 | `run docs / test gaps_lists_no_resolved_net_gap ... ok` | pass |
| C56 | `run spectext / test c_2_3_and_g_38_say_tran_on_a_resolved_nature_runs ... ok` | pass |
| C57 | `run speccodes / test no_owned_code_is_raised ... ok` | pass |
| C58 | `run irresolved / test a_resolved_net_is_built_checked_and_printed ... ok` | pass |
| C59 | `run interp / test resolved_forms_are_refused_by_name ... ok` | pass |
| C60 | `run planshape / test a_resolved_carrier_has_value_drive_and_others_pins ... ok` | pass |
| C61 | `run transresolve / test resolve_calls_the_instance_of_the_present_count ... ok` | pass |
| C62 | `run prototran / test an_unsettled_loop_is_e0511_naming_its_drives ... ok` | pass |
| C63 | `run elabnatures / test a_resolved_drive_without_default_or_initial_value_is_e0187 ... ok` | pass |
| C64 | `run bookmodules / test the_pi_controller_runs_with_its_generic_idt ... ok` | pass |
| C65 | `run bookdigital / test the_irq_line_runs_on_its_resolved_nature ... ok` | pass |
| C66 | `run bookcorestd / test the_resolved_line_runs_on_its_nature ... ok` | pass |
| C67 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C68 | `run sourcenatures / test an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run ... ok` | pass |
| C69 | `run typesnatures / test a_second_drive_of_a_one_driver_carrier_names_the_nature_and_resolve ... ok` | pass |
| C70 | `run sourcenets / test a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type ... ok` | pass |
| C71 | `run typesnatures / test a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181 ... ok` | pass |
| C72 | `run elabnatures / test two_driven_types_on_one_net_are_e0225 ... ok` | pass |
| C73 | `run sourcenets / test one_drive_still_goes_through_resolve ... ok` | pass |
| C74 | `run typesnatures / test a_library_module_on_a_nature_without_resolve_driven_is_e0187 ... ok` | pass |
| C75 | `run booknatures / test the_book_no_longer_says_the_alarm_is_refused ... ok` | pass |
| C76 | `run bookdigital / test the_four_worked_nets_give_the_tables_of_f_4 ... ok` | pass |
| C77 | `run booknatures / test signals_of_every_family_run ... ok` | pass |
| C78 | `run docs / test gaps_section_11_holds_no_net_row ... ok` | pass |
| C79 | `run typesgenmod / test a_nature_parameter_bound_by_resolve_is_a_resolved_carrier_in_the_body ... ok` | pass |
| C80 | `run typesgenmod / test a_placement_on_a_nature_without_the_resolve_impl_is_e0187_naming_nature_and_bound ... o` | pass |
| C81 | `run sourcenets / test a_tribuf_over_a_nature_parameter_runs_on_a_declared_nature ... ok` | pass |
| C82 | `run typesnatures / test an_impure_resolve_is_e0203_naming_its_class ... ok` | pass |
| C83 | `run typesaggregates / test a_param_of_the_librarys_logic_type_checks ... ok` | pass |
| C84 | `run sourcelogic / test a_logic_param_binds_a_pattern_and_a_reaction_reads_it ... ok` | pass |
| C85 | `run hostpatterns / test a_logic_param_is_set_by_its_pattern ... ok` | pass |
| C86 | `run hostpatterns / test the_host_drives_a_four_state_in_port ... ok` | pass |
| C87 | `run hostpatterns / test a_bad_character_and_a_wrong_length_are_e0702 ... ok` | pass |
| C88 | `run sourcenets / test two_carriers_of_one_instance_on_one_net_hold_two_drives ... ok` | pass |
| C89 | `run sourcenets / test a_tran_with_both_sides_on_one_net_settles_on_the_level ... ok` | pass |
| C90 | `run sourcenets / test a_match_over_options_and_a_resolve_walking_option_drives_run ... ok` | pass |
| C91 | `run irresolved / test a_resolved_signal_is_no_port_of_a_child_binding ... ok` | pass |
| C92 | `run hostpatterns / test a_logic_param_of_any_width_is_set_exactly_by_its_pattern ... ok` | pass |
| C93 | `run hostpatterns / test a_logic_param_of_any_width_is_set_exactly_by_its_leaves ... ok` | pass |
| C94 | `run irresolved / test a_resolution_whose_holders_do_not_fit_its_drives_is_refused ... ok` | pass |
| C95 | `run spectext / test rule_2_orders_the_drives_of_one_instance_by_carrier_declaration ... ok` | pass |
| C96 | `run elabgenmod / test a_resolved_nature_parameter_lowers_to_a_resolved_carrier_of_the_instance_nature ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run sourcenatures` - exit 0

```
running 6 tests
test vsignal_connected_to_an_electrical_port_is_conservative ... ok
test an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run ... ok
test a_signal_flow_node_with_one_driver_solves_and_loads_nothing ... ok
test a_counter_over_a_project_byte_nature_commits_and_records ... ok
test a_gain_chain_over_vsignal_gives_the_product_of_the_gains ... ok
test a_project_nature_carrier_connects_to_a_port_of_the_same_nature ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.79s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_natures.rs (/var/tmp/piperine-target-b/debug/deps/source_natures-b8984ec36a3d3d9a)
```

`Run typesnatures` - exit 0

```
test a_module_local_loop_of_resolved_drives_is_accepted ... ok
test a_flow_contribution_to_a_signal_flow_node_is_e0413 ... ok
test an_impl_resolve_makes_a_discrete_nature_resolved ... ok
test others_on_a_one_driver_carrier_is_e0224 ... ok
test a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181 ... ok
test connecting_byte_to_bit_u8_is_e0251_naming_both_natures ... ok
test resolve_may_call_a_library_or_project_function ... ok
test others_of_a_resolved_carrier_has_the_value_type ... ok
test an_impure_resolve_is_e0203_naming_its_class ... ok
test an_impl_resolve_of_another_shape_is_e0203 ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.63s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_natures.rs (/var/tmp/piperine-target-b/debug/deps/types_natures-c0472f988f483116)
```

`Run sourceresolved` - exit 0

```
test a_loop_of_resolved_drives_is_accepted_and_settles ... ok
test a_loop_settling_on_its_first_pass_raises_nothing ... ok
test an_initial_value_is_the_declaring_modules_first_drive ... ok
test a_released_ring_latches_nothing ... ok
test a_drive_without_initial_value_is_its_types_default_until_its_first_commit ... ok
test an_undriven_resolved_carrier_holds_resolve_of_no_drive ... ok
test a_drive_change_that_leaves_the_value_fires_no_changed ... ok
test three_summing_drivers_give_their_sum_in_the_microstep_any_changes ... ok
test resolve_receives_the_drives_in_instance_path_order ... ok
test resolved_nets_run_at_the_microsteps_of_each_sweep_point_and_hold_in_ac ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.77s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/source_resolved.rs (/var/tmp/piperine-target-b/debug/deps/source_resolved-868a082cd90e56f1)
```

`Run elabnatures` - exit 0

```
test resolve_is_instantiated_once_per_drive_count ... ok
test drives_are_ordered_by_instance_path_segment_by_segment ... ok
test others_lowers_to_the_bodys_others_read ... ok
test others_depends_on_the_other_drives_only ... ok
test a_node_meeting_a_nature_with_flow_is_not_counted ... ok
test two_driven_types_on_one_net_are_e0225 ... ok
test a_loop_through_a_one_driver_carrier_across_instances_is_e0339 ... ok
test a_signal_flow_node_with_no_or_two_drivers_is_e0242 ... ok
test an_initial_value_is_the_declaring_modules_first_drive_commit ... ok
test a_resolved_drive_without_default_or_initial_value_is_e0187 ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_natures.rs (/var/tmp/piperine-target-b/debug/deps/elab_natures-2ac1986ae98cf2da)
```

`Run relax` - exit 0

```
running 5 tests
test a_released_loop_keeps_no_earlier_level ... ok
test a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes ... ok
test an_unsettled_loop_is_e0511_naming_its_drives ... ok
test an_absent_drive_is_left_out_and_a_present_one_is_passed_whatever_its_value ... ok
test a_drive_pin_ends_with_a_plain_driving_byte ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/relax.rs (/var/tmp/piperine-target-b/debug/deps/relax-b90cf2f70ae93855)
```

`Run sourcenets` - exit 0

```
test a_nature_with_two_impls_resolves_each_carrier_by_its_driven_type ... ok
test a_match_over_options_and_a_resolve_walking_option_drives_run ... ok
test tgate_joins_while_s_high_or_sn_low ... ok
test strongest_of_equal_drives_is_that_drive_in_any_order ... ok
test tran_drives_each_side_with_the_others_of_the_other ... ok
test two_carriers_of_one_instance_on_one_net_hold_two_drives ... ok
test two_parallel_trans_carry_a_strong_level_and_stay_two_carriers ... ok
test strongest_gives_the_table_of_22_3_2 ... ok
test tribuf_drives_strong_or_high_impedance ... ok
test a_pull_wins_only_while_every_stronger_drive_is_released ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 49.36s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_nets.rs (/var/tmp/piperine-target-b/debug/deps/source_nets-2711638356bfef13)
```

`Run booknatures` - exit 0

```
test e0310_a_one_driver_port_needs_a_direction ... ok
test a_bundle_is_refused_where_the_solver_reaches_it ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok
test an_out_port_gives_the_initial_value ... ok
test behavior_crosses_freely_inside_one_module ... ok
test the_alarm_prints_three_records_of_the_line ... ok
test the_pulldown_alarm_prints_the_exercise_answer ... ok
test signals_of_every_family_run ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.25s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/natures.rs (/var/tmp/piperine-target-b/debug/deps/book_natures-573ee03fdb953ba2)
```

`Run clibook` - exit 0

```
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test the_chapter_shows_the_session_options ... ok
test a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one ... ok
test the_cycle_counter_prints_three_pulses ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_chapter_runs_each_new_analysis ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 54.70s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target-b/debug/deps/book-b94a48232bcc738f)
```

`Run docs` - exit 0

```
test the_scenario_package_states_its_measured_step_count ... ok
test state_records_the_unknown_semantics_decision ... ok
test transient_performance_closes_v12 ... ok
test gaps_no_longer_lists_x5_l20_l39_h12 ... ok
test the_roadmap_is_forward_looking ... ok
test state_records_the_observation_decision ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-b/debug/deps/docs-8e80bcc00dcc90ee)
```

`Run spectext` - exit 0

```
test tran_in_fences_is_the_module ... ok
test the_step_that_reads_a_jump_is_first_order ... ok
test zero_bytes_are_the_default_value ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test the_library_declares_no_net_nature ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test the_removed_words_are_absent_from_the_specification ... ok
test every_error_code_is_defined_exactly_once ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok

test result: ok. 140 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_text.rs (/var/tmp/piperine-target-b/debug/deps/spec_text-fc077905224603d9)
```

`Run speccodes` - exit 0

```
running 5 tests
test every_owned_code_names_its_owner_and_construct ... ok
test no_source_spells_p0002 ... ok
test every_spec_code_is_raised_or_owned ... ok
test this_features_codes_are_all_raised ... ok
test no_owned_code_is_raised ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_codes.rs (/var/tmp/piperine-target-b/debug/deps/spec_codes-a942a887390f8334)
```

`Run irresolved` - exit 0

```
running 4 tests
test a_resolved_signal_is_no_port_of_a_child_binding ... ok
test a_resolve_with_a_missing_count_or_another_type_is_refused ... ok
test a_resolution_whose_holders_do_not_fit_its_drives_is_refused ... ok
test a_resolved_net_is_built_checked_and_printed ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/resolved.rs (/var/tmp/piperine-target-b/debug/deps/resolved-fb517c6430ade59b)
```

`Run interp` - exit 0

```
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test each_generics_form_is_refused_by_name ... ok
test each_number_width_is_refused_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_level_nodes_are_refused_by_name ... ok
test the_interpreter_refuses_each_analog_operator_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/frozen.rs (/var/tmp/piperine-target-b/debug/deps/frozen-1d51dac525b72719)
```

`Run planshape` - exit 0

```
test an_if_lists_its_then_statement_before_its_else ... ok
test an_event_is_an_event_pin_and_its_function ... ok
test only_a_literal_positive_delay_is_later ... ok
test an_analog_read_of_a_carrier_is_a_read_of_terms ... ok
test the_drive_layout_appends_a_plain_driving_byte ... ok
test a_reaction_lists_its_events_reads_and_statements ... ok
test a_drive_is_a_discrete_function ... ok
test carriers_are_discrete_pins_that_hold_the_value_only ... ok
test every_record_layout_holds_its_declared_fields_only ... ok
test every_buck_slice_shape_validates ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/digital_shape.rs (/var/tmp/piperine-target-b/debug/deps/digital_shape-e3c10ee17d1090e5)
```

`Run transresolve` - exit 0

```
running 2 tests
test a_drive_write_marks_the_drive_present ... ok
test resolve_calls_the_instance_of_the_present_count ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.89s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/resolve.rs (/var/tmp/piperine-target-b/debug/deps/resolve-00b68532899feae0)
```

`Run prototran` - exit 0

```
test crossing_retries_count_toward_the_rejection_cap ... ok
test its_devices_pass_the_partials_verifier ... ok
test no_landing_means_one_attempt_to_the_proposed_time ... ok
test no_unknowns_means_no_attempt ... ok
test settle_runs_at_time_points_and_at_the_stop_time_only ... ok
test the_proposal_sees_the_next_due_time ... ok
test an_off_grid_step_lands_on_the_tick_grid ... ok
test an_unsettled_loop_is_e0511_naming_its_drives ... ok
test the_step_after_a_settle_commit_is_a_landing_and_restarts ... ok
test consecutive_rejections_beyond_the_cap_fail_loud ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/protocol_transient.rs (/var/tmp/piperine-target-b/debug/deps/protocol_transient-7b23e51a4de3e365)
```

`Run bookmodules` - exit 0

```
test the_book_shows_the_integrator_the_standard_library_gives ... ok
test a_module_body_takes_a_param_with_a_where_and_an_instance_with_a_param_block ... ok
test e0340_a_generation_range_read_from_a_signal ... ok
test the_lowpass_module_runs_over_the_generic_integrator ... ok
test e0320_a_block_names_a_param_the_module_does_not_have ... ok
test a_ladder_written_out_runs_an_operating_point_and_a_dc_sweep ... ok
test the_half_reactions_report_each_stage_in_time_order ... ok
test the_ladder_runs_for_an_operating_point_and_a_sweep ... ok
test the_switched_capacitor_integrator_is_a_netlist_that_runs ... ok
test the_pi_controller_runs_with_its_generic_idt ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.72s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/modules.rs (/var/tmp/piperine-target-b/debug/deps/book_modules-f54c744dbbe5981c)
```

`Run bookdigital` - exit 0

```
test e0501_a_clock_with_no_delay_never_goes_quiet ... ok
test the_library_declares_the_monitors_and_never_runs ... ok
test the_phase_detector_runs_assignment_and_events_read_as_bools ... ok
test the_pwm_latches_its_duty_word_at_the_wrap ... ok
test the_set_reset_latch_needs_no_reaction ... ok
test the_debouncer_pwm_and_pwm_check_print_their_records ... ok
test a_pipeline_in_one_reaction_reads_every_register_before_the_edge ... ok
test the_irq_line_runs_on_its_resolved_nature ... ok
test the_emit_prints_the_same_first_in_the_middle_or_last ... ok
test the_four_worked_nets_give_the_tables_of_f_4 ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.78s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/digital.rs (/var/tmp/piperine-target-b/debug/deps/book_digital-39a5776c7074e035)
```

`Run bookcorestd` - exit 0

```
test the_library_declares_the_noise_sources_and_the_monitors_only_report ... ok
test the_dac_ramp_scales_its_code_by_the_reference ... ok
test the_probes_have_the_short_names_the_nature_gives ... ok
test the_pad_drives_its_node_and_reads_it_back ... ok
test the_tour_module_runs_its_analog_and_digital_blocks ... ok
test the_watchdog_builds_with_its_u16_param ... ok
test a_crates_own_vt_replaces_the_library_one ... ok
test the_watchdog_ends_the_run_at_the_fourth_edge_with_the_button_held ... ok
test the_register_with_a_setup_check_runs ... ok
test the_resolved_line_runs_on_its_nature ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.29s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book/core_std.rs (/var/tmp/piperine-target-b/debug/deps/book_core_std-1339ea21078baf85)
```

`Run bookinventory` - exit 0

```
test the_owner_and_disputed_rules_name_what_they_break ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test a_model_across_blocks_names_one_test ... ok
test disputed_rows_are_listed_for_the_user ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test every_block_of_the_book_has_one_row ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok
test the_book_uses_no_removed_name ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-b/debug/deps/book_inventory-4e223d7a754dd3b2)
```

`Run typesgenmod` - exit 0

```
test a_carrier_handed_to_a_generic_out_port_needs_no_initial_value ... ok
test a_placement_on_a_nature_without_the_resolve_impl_is_e0187_naming_nature_and_bound ... ok
test two_natures_for_one_nature_parameter_are_e0180_naming_both ... ok
test a_generic_module_body_is_checked_once_at_its_definition ... ok
test behavior_names_that_differ_are_e0403_naming_both_lists ... ok
test a_behavior_block_binds_its_modules_generic_names ... ok
test a_param_of_a_type_parameter_is_typed_under_its_bounds ... ok
test a_resolve_bound_takes_one_or_two_arguments_and_a_nature ... ok
test const_expressions_in_written_types_are_accepted_everywhere ... ok
test generic_module_arguments_are_explicit_or_inferred ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.44s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_generic_modules.rs (/var/tmp/piperine-target-b/debug/deps/types_generic_modules-63c124af6223dbbf)
```

`Run typesaggregates` - exit 0

```
test struct_expressions_and_field_reads_type_check ... ok
test tuples_and_tuple_patterns_type_check ... ok
test an_enum_without_repr_is_a_value_but_not_a_carried_type ... ok
test the_rows_of_h5_to_h7_type_check ... ok
test a_param_of_the_librarys_logic_type_checks ... ok
test two_sites_giving_a_field_two_composite_types_is_e0605 ... ok
test generic_arguments_on_a_plain_type_are_refused ... ok
test composite_params_and_record_fields_type_check ... ok
test a_carried_type_without_repr_is_e0112 ... ok
test composite_carriers_type_check ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.80s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_aggregates.rs (/var/tmp/piperine-target-b/debug/deps/types_aggregates-9ae44a4f4df4957b)
```

`Run sourcelogic` - exit 0

```
test logic_and_matches_bits_give_the_table_values ... ok
test logic_lays_out_two_planes ... ok
test logic_reads_a_four_state_pattern ... ok
test the_tables_hold_at_sixty_four_levels ... ok
test not_follows_the_four_state_table ... ok
test same_compares_the_four_states ... ok
test or_follows_the_four_state_table ... ok
test shifts_move_states_and_fill_with_zero ... ok
test the_four_tests_of_a_logic ... ok
test xor_follows_the_four_state_table ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.33s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_logic.rs (/var/tmp/piperine-target-b/debug/deps/source_logic-7a1c183f548c2d32)
```

`Run hostpatterns` - exit 0

```
running 9 tests
test a_bad_character_and_a_wrong_length_are_e0702 ... ok
test a_pattern_lays_out_its_two_planes_highest_level_first ... ok
test a_timed_pattern_lands_at_its_time ... ok
test the_host_drives_a_four_state_in_port ... ok
test a_refused_pattern_changes_nothing ... ok
test the_host_drive_of_a_resolved_port_is_the_net_when_it_is_the_only_drive ... ok
test a_logic_param_of_any_width_is_set_exactly_by_its_pattern ... ok
test a_logic_param_is_set_by_its_pattern ... ok
test a_logic_param_of_any_width_is_set_exactly_by_its_leaves ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.12s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/host_patterns.rs (/var/tmp/piperine-target-b/debug/deps/host_patterns-f68bd67ffb1f0328)
```

`Run elabgenmod` - exit 0

```
running 6 tests
test a_param_read_by_a_generic_argument_or_a_length_is_structural ... ok
test a_generic_module_passing_its_resolved_nature_on_lowers_its_inner_instances_at_the_nature ... ok
test a_resolved_nature_parameter_lowers_to_a_resolved_carrier_of_the_instance_nature ... ok
test a_false_module_where_is_e0304_naming_the_instance ... ok
test one_body_per_distinct_module_instance_and_the_pom_as_written ... ok
test a_chain_of_growing_module_arguments_is_e0341 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/elab_generic_modules.rs (/var/tmp/piperine-target-b/debug/deps/elab_generic_modules-7bbedd7d1db91076)
```
