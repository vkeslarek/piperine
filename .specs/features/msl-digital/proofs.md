# msl-digital proofs

**HEAD**: 060613c9
**Tree**: clean
**Shell**: /bin/sh
**Result**: 189 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run tint / test integer_types_and_constants_have_types ... ok` | pass |
| C2 | `run tint / test a_literal_takes_the_expected_integer_type_else_i32 ... ok` | pass |
| C3 | `run tint / test a_literal_out_of_range_is_e0909 ... ok` | pass |
| C4 | `run tint / test integer_operators_take_one_type ... ok` | pass |
| C5 | `run tint / test mixed_integer_operands_are_p0002 ... ok` | pass |
| C6 | `run tint / test a_shift_takes_any_integer_amount ... ok` | pass |
| C7 | `run tint / test as_goes_between_integers_and_f64 ... ok` | pass |
| C8 | `run tint / test min_max_clamp_and_floor_are_typed ... ok` | pass |
| C9 | `run tint / test an_unsupported_width_or_float_is_named ... ok` | pass |
| C10 | `run tint / test a_width_below_one_is_e0103 ... ok` | pass |
| C11 | `run tcar / test a_catalogue_method_on_a_carrier_is_typed_on_its_value ... ok` | pass |
| C12 | `run tint / test an_uncatalogued_method_is_p0001_by_name ... ok` | pass |
| C13 | `run tint / test every_accepted_call_has_a_callee ... ok` | pass |
| C14 | `run tcomp / test unit_enums_and_their_variants_are_typed ... ok` | pass |
| C15 | `run tcomp / test an_enum_outside_the_subset_is_p0001 ... ok` | pass |
| C16 | `run tcomp / test option_values_and_their_methods_are_typed ... ok` | pass |
| C17 | `run tcomp / test match_is_typed_over_enums_bools_integers_and_options ... ok` | pass |
| C18 | `run tcomp / test a_match_that_is_not_exhaustive_is_p0002 ... ok` | pass |
| C19 | `run tcomp / test if_let_some_binds_the_payload ... ok` | pass |
| C20 | `run tcomp / test a_tuple_let_names_each_component ... ok` | pass |
| C21 | `run tcar / test carriers_of_bit_are_typed_by_their_value ... ok` | pass |
| C22 | `run tcar / test a_carried_type_without_repr_is_e0112 ... ok` | pass |
| C23 | `run tcar / test a_carrier_read_has_its_value_type ... ok` | pass |
| C24 | `run tcar / test the_flag_reads_are_typed ... ok` | pass |
| C25 | `run tcar / test an_initial_value_of_the_wrong_kind_is_e0210 ... ok` | pass |
| C26 | `run tcar / test a_definition_of_another_type_is_e0337 ... ok` | pass |
| C27 | `run tcar / test a_continuous_definition_is_e0336 ... ok` | pass |
| C28 | `run tcar / test a_continuous_expression_at_an_in_port_is_e0316 ... ok` | pass |
| C29 | `run tcar / test a_target_the_module_may_not_drive_is_e0222 ... ok` | pass |
| C30 | `run tcar / test two_definitions_of_one_carrier_are_e0335 ... ok` | pass |
| C31 | `run tev / test a_definition_and_a_commit_are_e0221 ... ok` | pass |
| C32 | `run tcar / test a_second_drive_is_e0220 ... ok` | pass |
| C33 | `run tcar / test an_initial_value_without_the_drive_is_e0223 ... ok` | pass |
| C34 | `run tcar / test an_initial_value_under_an_undelayed_definition_is_e0338 ... ok` | pass |
| C35 | `run tcar / test an_out_port_takes_only_a_carrier_the_parent_hands_over ... ok` | pass |
| C36 | `run tcar / test a_carrier_of_another_value_type_is_e0251 ... ok` | pass |
| C37 | `run tcar / test a_loop_of_definitions_is_e0339 ... ok` | pass |
| C38 | `run tev / test event_expressions_are_typed ... ok` | pass |
| C39 | `run tev / test a_bool_head_is_e0430 ... ok` | pass |
| C40 | `run tev / test a_misplaced_not_or_and_is_e0440 ... ok` | pass |
| C41 | `run tev / test an_event_outside_an_event_position_is_e0441 ... ok` | pass |
| C42 | `run tev / test a_continuous_at_is_e0442 ... ok` | pass |
| C43 | `run tev / test cross_or_at_outside_an_event_position_is_e0443 ... ok` | pass |
| C44 | `run tev / test a_reaction_body_is_typed ... ok` | pass |
| C45 | `run tev / test a_commit_of_another_type_is_e0432 ... ok` | pass |
| C46 | `run tev / test a_delay_that_is_not_f64_is_e0433 ... ok` | pass |
| C47 | `run tev / test a_placement_in_a_reaction_is_e0350 ... ok` | pass |
| C48 | `run tev / test emit_is_typed_in_a_reaction ... ok` | pass |
| C49 | `run tev / test a_repeated_or_reserved_field_is_e0604 ... ok` | pass |
| C50 | `run tev / test a_field_type_conflict_is_e0605 ... ok` | pass |
| C51 | `run tev / test emit_where_it_is_not_allowed_is_e0601 ... ok` | pass |
| C52 | `run tev / test a_check_reached_from_analog_is_e0603 ... ok` | pass |
| C53 | `run tres / test a_module_with_a_result_is_typed ... ok` | pass |
| C54 | `run tres / test a_placement_has_its_result_type ... ok` | pass |
| C55 | `run tres / test trailing_option_inputs_may_be_left_out ... ok` | pass |
| C56 | `run tres / test a_placement_in_a_const_context_is_e0352 ... ok` | pass |
| C57 | `run tres / test now_and_analysis_static_are_typed ... ok` | pass |
| C58 | `run tres / test an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150 ... ok` | pass |
| C59 | `run tres / test a_let_that_reads_itself_is_e0332_or_e0353 ... ok` | pass |
| C60 | `run tcomp / test a_value_for_an_option_parameter_is_some_of_it ... ok` | pass |
| C61 | `run tbuck / test what_stays_outside_is_p0001_by_name ... ok` | pass |
| C62 | `run edig / test the_m1_goldens_are_unchanged ... ok` | pass |
| C63 | `run tbuck / test the_buck_type_checks_with_no_diagnostic ... ok` | pass |
| C64 | `run evalues / test integers_lower_to_typed_ir ... ok` | pass |
| C65 | `run evalues / test integer_constants_fold ... ok` | pass |
| C66 | `run evalues / test folded_casts_shifts_and_sums_wrap_as_compiled_code_does ... ok` | pass |
| C67 | `run evalues / test a_constant_division_by_zero_is_e0104 ... ok` | pass |
| C68 | `run evalues / test a_carrier_read_is_flagged_and_the_flag_reads_are_not ... ok` | pass |
| C69 | `run evalues / test a_short_circuit_over_a_flagged_operand_becomes_bitwise ... ok` | pass |
| C70 | `run evalues / test a_partial_right_operand_under_a_flagged_short_circuit_is_p0001 ... ok` | pass |
| C71 | `run evalues / test a_function_gets_one_variant_per_flag_pattern ... ok` | pass |
| C72 | `run evalues / test an_enum_is_its_tag_and_a_match_is_an_if_chain ... ok` | pass |
| C73 | `run evalues / test an_option_is_a_flag_and_a_payload ... ok` | pass |
| C74 | `run evalues / test a_tuple_let_is_one_local_per_component ... ok` | pass |
| C75 | `run edig / test carriers_follow_ports_then_signals ... ok` | pass |
| C76 | `run edig / test a_definition_is_a_drive ... ok` | pass |
| C77 | `run edig / test an_expression_at_an_in_port_is_a_carrier_and_a_drive ... ok` | pass |
| C78 | `run edig / test every_signal_carrier_is_a_design_net ... ok` | pass |
| C79 | `run edig / test a_top_with_discrete_ports_gets_one_net_per_port ... ok` | pass |
| C80 | `run edig / test changed_rise_fall_and_cross_lower_to_events ... ok` | pass |
| C81 | `run edig / test a_guard_splits_into_its_discrete_and_continuous_parts ... ok` | pass |
| C82 | `run edig / test an_or_head_is_one_reaction_or_one_per_disjunct ... ok` | pass |
| C83 | `run edig / test a_head_the_ir_cannot_hold_is_p0001 ... ok` | pass |
| C84 | `run edig / test a_reaction_body_lowers_in_source_order ... ok` | pass |
| C85 | `run edig / test a_child_param_reads_the_parents_value ... ok` | pass |
| C86 | `run iramend / test a_child_param_may_read_the_parents_params ... ok` | pass |
| C87 | `run edig / test elaborating_twice_gives_the_same_print ... ok` | pass |
| C88 | `run edig / test the_design_is_not_written ... ok` | pass |
| C89 | `run edig / test every_ir_passes_check ... ok` | pass |
| C90 | `run eres / test a_placement_is_hoisted_under_its_name ... ok` | pass |
| C91 | `run eres / test value_inputs_are_substituted_at_each_read ... ok` | pass |
| C92 | `run eres / test an_omitted_option_input_reads_as_none ... ok` | pass |
| C93 | `run eres / test two_placements_are_two_sets_of_carriers ... ok` | pass |
| C94 | `run eres / test a_result_module_gets_no_body_and_no_instance ... ok` | pass |
| C95 | `run eres / test an_expression_at_a_discrete_input_drives_the_hoisted_carrier ... ok` | pass |
| C96 | `run eres / test at_now_and_static_lower_to_their_nodes ... ok` | pass |
| C97 | `run iramend / test time_and_static_reads_print_and_check ... ok` | pass |
| C98 | `run iramend / test an_at_event_prints_and_checks ... ok` | pass |
| C99 | `run eres / test emit_lowers_to_a_site_with_its_texts ... ok` | pass |
| C100 | `run iramend / test an_emit_site_prints_and_checks ... ok` | pass |
| C101 | `run iramend / test a_record_site_gives_its_stream_and_texts ... ok` | pass |
| C102 | `run eres / test a_function_that_emits_is_inlined ... ok` | pass |
| C103 | `run eres / test a_text_field_that_is_not_a_literal_is_p0001 ... ok` | pass |
| C104 | `run brecords / test never_reports_each_rising_edge ... ok` | pass |
| C105 | `run amended / test an_emit_commits_one_record_and_adds_no_microstep ... ok` | pass |
| C106 | `run planamend / test a_stream_is_a_record_pin_with_one_field_per_value ... ok` | pass |
| C107 | `run brecords / test log_carries_its_severity_and_its_text ... ok` | pass |
| C108 | `run amended / test at_fires_once_at_its_time ... ok` | pass |
| C109 | `run planamend / test an_at_event_is_a_pin_an_arming_reaction_and_a_replace_statement ... ok` | pass |
| C110 | `run btime / test every_from_the_stdlib_fires_once_per_period ... ok` | pass |
| C111 | `run amended / test a_rearmed_at_drops_the_earlier_firing ... ok` | pass |
| C112 | `run amended / test a_past_or_unknown_time_does_not_fire ... ok` | pass |
| C113 | `run amended / test now_is_the_time_of_the_instant_and_of_the_solve ... ok` | pass |
| C114 | `run planamend / test time_and_static_are_ambient_reads ... ok` | pass |
| C115 | `run btime / test a_reaction_reads_now_and_static_from_source ... ok` | pass |
| C116 | `run amended / test static_is_true_only_in_the_equilibrium ... ok` | pass |
| C117 | `run emix / test a_flow_probe_of_a_potential_branch_lowers ... ok` | pass |
| C118 | `run iramend / test a_flow_read_and_a_potential_ddt_print_and_check ... ok` | pass |
| C119 | `run emix / test a_potential_contribution_keeps_its_ddt ... ok` | pass |
| C120 | `run planamend / test a_flow_read_is_the_branch_pin_and_a_potential_ddt_is_a_ddt_term ... ok` | pass |
| C121 | `run emix / test a_flow_probe_of_a_branch_without_a_potential_contribution_is_p0001 ... ok` | pass |
| C122 | `run bpower / test the_inductor_follows_the_rl_response ... ok` | pass |
| C123 | `run amended / test the_inductor_functions_pass_verify_partials ... ok` | pass |
| C124 | `run bpower / test the_current_sense_scales_the_shunt_voltage ... ok` | pass |
| C125 | `run bpower / test the_power_switch_ramps_its_conductance_in_tsw ... ok` | pass |
| C126 | `run emix / test analog_reads_a_hoisted_result ... ok` | pass |
| C127 | `run bpower / test a_gate_change_makes_two_time_points ... ok` | pass |
| C128 | `run ebuck / test six_slice_bodies_equal_their_hand_built_fixtures ... ok` | pass |
| C129 | `run ebuck / test dpwm_and_a2d_differ_only_as_listed ... ok` | pass |
| C130 | `run bdpwm / test the_oscillator_toggles_every_half_period ... ok` | pass |
| C131 | `run bdpwm / test the_dpwm_dithers_keeps_dead_time_and_never_overlaps ... ok` | pass |
| C132 | `run bcontrol / test the_ramp_and_the_pid_follow_their_difference_equations ... ok` | pass |
| C133 | `run bboundary / test the_adc_commits_the_rounded_code_after_tconv ... ok` | pass |
| C134 | `run bdpwm / test two_runs_give_the_same_changes ... ok` | pass |
| C135 | `run bboundary / test the_comparator_trips_at_its_thresholds ... ok` | pass |
| C136 | `run bboundary / test the_comparator_is_right_at_time_zero ... ok` | pass |
| C137 | `run bcontrol / test the_setpoint_register_takes_known_data_and_saturates ... ok` | pass |
| C138 | `run bcommits / test two_commits_exchange_two_carriers ... ok` | pass |
| C139 | `run bcommits / test the_later_statement_wins ... ok` | pass |
| C140 | `run bcommits / test two_statements_make_a_pulse ... ok` | pass |
| C141 | `run bcommits / test an_unchanged_commit_fires_nothing ... ok` | pass |
| C142 | `run bpower / test the_gate_driver_delays_and_interlocks ... ok` | pass |
| C143 | `run ebuck / test the_buck_elaborates_with_no_diagnostic_and_a_sound_ir ... ok` | pass |
| C144 | `run ebuck / test the_buck_ir_is_pinned ... ok` | pass |
| C145 | `run bwhole / test the_buck_compiles_with_no_warning_and_valid_shapes ... ok` | pass |
| C146 | `run project / test a_project_with_a_dependency_crate_loads_and_runs ... ok` | pass |
| C147 | `run bsupervisor / test the_supervisor_follows_its_state_table ... ok` | pass |
| C148 | `run bsupervisor / test power_good_asserts_after_64_and_drops_after_4 ... ok` | pass |
| C149 | `run bwhole / test a_buck_that_is_not_enabled_stays_off_with_vin_ok_high ... ok` | pass |
| C150 | `run bwhole / test no_never_record_fails_while_not_enabled ... ok` | pass |
| C151 | `run bloop / test the_buck_starts_and_reaches_regulate ... ok` | pass |
| C152 | `run bloop / test power_good_is_high_and_no_fault_is_latched ... ok` | pass |
| C153 | `run bloop / test the_output_settles_within_10_mv_of_0v8 ... ok` | pass |
| C154 | `run bloop / test no_shoot_through_during_the_start ... ok` | pass |
| C155 | `grep -q '^- \*\*An unknown control is a fault' GAPS.md && grep -q '^- \*\*A derived child param is not re-deri` | pass |
| C156 | `! grep -q -e 'G3' -e 'saturating_add' -e 'Unresolved' GAPS.md && grep -q '^## G3\. .*(closed: now reported)$' ` | pass |
| C157 | `! grep -q 'Nothing executes MSL yet' examples/buck/README.md` | pass |
| C158 | `run tcar / test prev_is_typed_on_a_carrier ... ok` | pass |
| C159 | `run iramend / test a_previous_value_read_prints_and_checks ... ok` | pass |
| C160 | `run planamend / test a_previous_value_is_the_previous_part_of_the_pin ... ok` | pass |
| C161 | `run amended / test a_previous_value_is_read_in_a_guard_and_in_a_reaction ... ok` | pass |
| C162 | `run btime / test rise_and_fall_fire_on_edges_to_and_from_unknown ... ok` | pass |
| C163 | `run edig / test a_module_with_only_a_digital_block_has_no_analog_statements ... ok` | pass |
| C164 | `run edig / test a_carrier_with_no_value_and_no_drive_is_kept ... ok` | pass |
| C165 | `run tcar / test a_single_variant_enum_cannot_be_carried ... ok` | pass |
| C166 | `run eres / test a_result_module_is_hoisted_once_per_body ... ok` | pass |
| C167 | `run eres / test a_param_block_binds_the_hoisted_params ... ok` | pass |
| C168 | `run frozen / test a_body_with_an_amended_form_is_rejected_by_name ... ok` | pass |
| C169 | `run iramend / test the_earlier_prints_are_unchanged ... ok` | pass |
| C170 | `run project / test a_diagnostic_is_returned_with_its_stage ... ok` | pass |
| C171 | `run edig / test a_placement_records_its_carrier_bindings ... ok` | pass |
| C172 | `run edig / test a_net_takes_the_init_of_its_declaring_carrier ... ok` | pass |
| C173 | `run amended / test hoisted_names_print_as_rust_identifiers ... ok` | pass |
| C174 | `run iramend / test a_child_records_its_carrier_bindings ... ok` | pass |
| C175 | `run nets / test a_parent_signal_reaches_its_childrens_ports ... ok` | pass |
| C176 | `run nets / test a_net_starts_at_its_own_init ... ok` | pass |
| C177 | `run tcomp / test a_nested_pattern_inside_some_covers_its_payload ... ok` | pass |
| C178 | `run eres / test the_stdlib_d2a_reads_its_match_scrutinee_at_each_test ... ok` | pass |
| C179 | `run bboundary / test the_comparator_trips_at_its_thresholds ... ok` | pass |
| C180 | `run brecords / test records_of_two_reactions_are_told_apart ... ok` | pass |
| C181 | `run brecords / test records_of_two_reactions_at_different_statements_keep_their_texts ... ok` | pass |
| C182 | `run tev / test an_emit_in_analog_is_p0001_by_name ... ok` | pass |
| C183 | `cargo test -p piperine-lang --test pom_resolve a_body_let_gives_its_module_only_the_names_of_its_own_pattern -` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-ir --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-interp --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-plan --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-transpile --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-parity --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run tint` - exit 0

```
test min_max_clamp_and_floor_are_typed ... ok
test a_literal_takes_the_expected_integer_type_else_i32 ... ok
test a_shift_takes_any_integer_amount ... ok
test integer_types_and_constants_have_types ... ok
test a_literal_out_of_range_is_e0909 ... ok
test mixed_integer_operands_are_p0002 ... ok
test an_unsupported_width_or_float_is_named ... ok
test integer_operators_take_one_type ... ok
test as_goes_between_integers_and_f64 ... ok
test every_accepted_call_has_a_callee ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.23s
     Running tests/types_integers.rs (/var/tmp/piperine-target/debug/deps/types_integers-140f5fa8f7c5828b)
```

`Run tcar` - exit 0

```
test prev_is_typed_on_a_carrier ... ok
test a_loop_of_definitions_is_e0339 ... ok
test two_definitions_of_one_carrier_are_e0335 ... ok
test an_out_port_takes_only_a_carrier_the_parent_hands_over ... ok
test a_target_the_module_may_not_drive_is_e0222 ... ok
test a_carrier_of_another_value_type_is_e0251 ... ok
test a_continuous_expression_at_an_in_port_is_e0316 ... ok
test an_initial_value_of_the_wrong_kind_is_e0210 ... ok
test the_flag_reads_are_typed ... ok
test a_continuous_definition_is_e0336 ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.51s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/types_carriers.rs (/var/tmp/piperine-target/debug/deps/types_carriers-26c9f2f7986ac3b1)
```

`Run tcomp` - exit 0

```
running 9 tests
test a_value_for_an_option_parameter_is_some_of_it ... ok
test if_let_some_binds_the_payload ... ok
test option_values_and_their_methods_are_typed ... ok
test a_tuple_let_names_each_component ... ok
test an_enum_outside_the_subset_is_p0001 ... ok
test match_is_typed_over_enums_bools_integers_and_options ... ok
test a_match_that_is_not_exhaustive_is_p0002 ... ok
test a_nested_pattern_inside_some_covers_its_payload ... ok
test unit_enums_and_their_variants_are_typed ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/types_compound.rs (/var/tmp/piperine-target/debug/deps/types_compound-db1390b9ba770d19)
```

`Run tev` - exit 0

```
test a_placement_in_a_reaction_is_e0350 ... ok
test a_bool_head_is_e0430 ... ok
test a_reaction_body_is_typed ... ok
test a_misplaced_not_or_and_is_e0440 ... ok
test a_field_type_conflict_is_e0605 ... ok
test emit_is_typed_in_a_reaction ... ok
test an_event_outside_an_event_position_is_e0441 ... ok
test cross_or_at_outside_an_event_position_is_e0443 ... ok
test emit_where_it_is_not_allowed_is_e0601 ... ok
test event_expressions_are_typed ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.61s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/types_events.rs (/var/tmp/piperine-target/debug/deps/types_events-b1cab65d30f1c5ba)
```

`Run tres` - exit 0

```
running 7 tests
test a_placement_has_its_result_type ... ok
test a_let_that_reads_itself_is_e0332_or_e0353 ... ok
test trailing_option_inputs_may_be_left_out ... ok
test a_placement_in_a_const_context_is_e0352 ... ok
test now_and_analysis_static_are_typed ... ok
test a_module_with_a_result_is_typed ... ok
test an_analog_read_of_a_carrier_outside_the_flag_reads_is_e0150 ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.69s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/types_results.rs (/var/tmp/piperine-target/debug/deps/types_results-2376c9ef742f3144)
```

`Run tbuck` - exit 0

```
running 2 tests
test the_buck_type_checks_with_no_diagnostic ... ok
test what_stays_outside_is_p0001_by_name ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.34s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/types_buck.rs (/var/tmp/piperine-target/debug/deps/types_buck-d98f9823db6e28e0)
```

`Run edig` - exit 0

```
test a_head_the_ir_cannot_hold_is_p0001 ... ok
test a_definition_is_a_drive ... ok
test a_child_param_reads_the_parents_value ... ok
test a_reaction_body_lowers_in_source_order ... ok
test every_signal_carrier_is_a_design_net ... ok
test carriers_follow_ports_then_signals ... ok
test the_m1_goldens_are_unchanged ... ok
test the_design_is_not_written ... ok
test every_ir_passes_check ... ok
test elaborating_twice_gives_the_same_print ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.62s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/elab_digital.rs (/var/tmp/piperine-target/debug/deps/elab_digital-59aa911b97c9094f)
```

`Run evalues` - exit 0

```
test integer_constants_fold ... ok
test folded_casts_shifts_and_sums_wrap_as_compiled_code_does ... ok
test integers_lower_to_typed_ir ... ok
test a_short_circuit_over_a_flagged_operand_becomes_bitwise ... ok
test an_enum_is_its_tag_and_a_match_is_an_if_chain ... ok
test a_carrier_read_is_flagged_and_the_flag_reads_are_not ... ok
test an_option_is_a_flag_and_a_payload ... ok
test a_function_gets_one_variant_per_flag_pattern ... ok
test a_constant_division_by_zero_is_e0104 ... ok
test a_partial_right_operand_under_a_flagged_short_circuit_is_p0001 ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.06s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/elab_values.rs (/var/tmp/piperine-target/debug/deps/elab_values-e4682f41678f6475)
```

`Run iramend` - exit 0

```
test a_record_site_gives_its_stream_and_texts ... ok
test an_at_event_prints_and_checks ... ok
test a_previous_value_read_prints_and_checks ... ok
test time_and_static_reads_print_and_check ... ok
test an_emit_site_prints_and_checks ... ok
test a_child_param_may_read_the_parents_params ... ok
test a_flow_read_and_a_potential_ddt_print_and_check ... ok
test a_child_records_its_carrier_bindings ... ok
test the_earlier_prints_are_unchanged ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-ir)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.20s
     Running tests/amendments.rs (/var/tmp/piperine-target/debug/deps/amendments-71576dfa08336c88)
```

`Run eres` - exit 0

```
test two_placements_are_two_sets_of_carriers ... ok
test an_expression_at_a_discrete_input_drives_the_hoisted_carrier ... ok
test a_function_that_emits_is_inlined ... ok
test the_stdlib_d2a_reads_its_match_scrutinee_at_each_test ... ok
test a_param_block_binds_the_hoisted_params ... ok
test an_omitted_option_input_reads_as_none ... ok
test a_result_module_gets_no_body_and_no_instance ... ok
test emit_lowers_to_a_site_with_its_texts ... ok
test a_placement_is_hoisted_under_its_name ... ok
test value_inputs_are_substituted_at_each_read ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/elab_results.rs (/var/tmp/piperine-target/debug/deps/elab_results-c0b0d81a6bca4e7f)
```

`Run brecords` - exit 0

```
running 4 tests
test log_carries_its_severity_and_its_text ... ok
test records_of_two_reactions_are_told_apart ... ok
test never_reports_each_rising_edge ... ok
test records_of_two_reactions_at_different_statements_keep_their_texts ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.20s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_records.rs (/var/tmp/piperine-target/debug/deps/buck_records-a71a40eeb24963ad)
```

`Run amended` - exit 0

```
test static_is_true_only_in_the_equilibrium ... ok
test the_inductor_functions_pass_verify_partials ... ok
test a_past_or_unknown_time_does_not_fire ... ok
test an_emit_commits_one_record_and_adds_no_microstep ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.23s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-analog-solver)
   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-ir)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-interp)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.76s
     Running tests/amended.rs (/var/tmp/piperine-target/debug/deps/amended-5e6b8480ba5437b5)
```

`Run planamend` - exit 0

```
running 5 tests
test a_stream_is_a_record_pin_with_one_field_per_value ... ok
test a_previous_value_is_the_previous_part_of_the_pin ... ok
test an_at_event_is_a_pin_an_arming_reaction_and_a_replace_statement ... ok
test a_flow_read_is_the_branch_pin_and_a_potential_ddt_is_a_ddt_term ... ok
test time_and_static_are_ambient_reads ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.66s
     Running tests/amended_shape.rs (/var/tmp/piperine-target/debug/deps/amended_shape-684fd8708d621da9)
```

`Run btime` - exit 0

```
running 3 tests
test every_from_the_stdlib_fires_once_per_period ... ok
test a_reaction_reads_now_and_static_from_source ... ok
test rise_and_fall_fire_on_edges_to_and_from_unknown ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.39s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_time.rs (/var/tmp/piperine-target/debug/deps/buck_time-00ff16b88a5c0cea)
```

`Run emix` - exit 0

```
running 4 tests
test a_potential_contribution_keeps_its_ddt ... ok
test a_flow_probe_of_a_branch_without_a_potential_contribution_is_p0001 ... ok
test a_flow_probe_of_a_potential_branch_lowers ... ok
test analog_reads_a_hoisted_result ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/elab_mixed.rs (/var/tmp/piperine-target/debug/deps/elab_mixed-c3fb88d25b013989)
```

`Run bpower` - exit 0

```
running 5 tests
test the_current_sense_scales_the_shunt_voltage ... ok
test the_gate_driver_delays_and_interlocks ... ok
test the_power_switch_ramps_its_conductance_in_tsw ... ok
test a_gate_change_makes_two_time_points ... ok
test the_inductor_follows_the_rl_response ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.98s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_power.rs (/var/tmp/piperine-target/debug/deps/buck_power-3b3b34241afb4fbe)
```

`Run ebuck` - exit 0

```
running 4 tests
test dpwm_and_a2d_differ_only_as_listed ... ok
test the_buck_ir_is_pinned ... ok
test six_slice_bodies_equal_their_hand_built_fixtures ... ok
test the_buck_elaborates_with_no_diagnostic_and_a_sound_ir ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/elab_buck.rs (/var/tmp/piperine-target/debug/deps/elab_buck-33afea82805031b1)
```

`Run bdpwm` - exit 0

```
running 3 tests
test the_oscillator_toggles_every_half_period ... ok
test the_dpwm_dithers_keeps_dead_time_and_never_overlaps ... ok
test two_runs_give_the_same_changes ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.13s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_dpwm.rs (/var/tmp/piperine-target/debug/deps/buck_dpwm-84aad3b330f66e68)
```

`Run bcontrol` - exit 0

```
running 2 tests
test the_setpoint_register_takes_known_data_and_saturates ... ok
test the_ramp_and_the_pid_follow_their_difference_equations ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_control.rs (/var/tmp/piperine-target/debug/deps/buck_control-2b799f4d486b3456)
```

`Run bboundary` - exit 0

```
running 3 tests
test the_comparator_is_right_at_time_zero ... ok
test the_adc_commits_the_rounded_code_after_tconv ... ok
test the_comparator_trips_at_its_thresholds ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_boundary.rs (/var/tmp/piperine-target/debug/deps/buck_boundary-96508b3f636069e4)
```

`Run bcommits` - exit 0

```
running 4 tests
test two_statements_make_a_pulse ... ok
test the_later_statement_wins ... ok
test two_commits_exchange_two_carriers ... ok
test an_unchanged_commit_fires_nothing ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.53s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_commits.rs (/var/tmp/piperine-target/debug/deps/buck_commits-e34fcba5187cb280)
```

`Run bwhole` - exit 0

```
running 3 tests
test the_buck_compiles_with_no_warning_and_valid_shapes ... ok
test a_buck_that_is_not_enabled_stays_off_with_vin_ok_high ... ok
test no_never_record_fails_while_not_enabled ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.93s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_whole.rs (/var/tmp/piperine-target/debug/deps/buck_whole-9573fbcfca8827b4)
```

`Run project` - exit 0

```
running 2 tests
test a_project_with_a_dependency_crate_loads_and_runs ... ok
test a_diagnostic_is_returned_with_its_stage ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/project.rs (/var/tmp/piperine-target/debug/deps/project-b3d553a6bc54679f)
```

`Run bsupervisor` - exit 0

```
running 2 tests
test power_good_asserts_after_64_and_drops_after_4 ... ok
test the_supervisor_follows_its_state_table ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.71s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_supervisor.rs (/var/tmp/piperine-target/debug/deps/buck_supervisor-d807af2fae5dee26)
```

`Run bloop` - exit 0

```
running 4 tests
test no_shoot_through_during_the_start has been running for over 60 seconds
test power_good_is_high_and_no_fault_is_latched has been running for over 60 seconds
test the_buck_starts_and_reaches_regulate has been running for over 60 seconds
test the_output_settles_within_10_mv_of_0v8 has been running for over 60 seconds
test the_buck_starts_and_reaches_regulate ... ok
test power_good_is_high_and_no_fault_is_latched ... ok
test the_output_settles_within_10_mv_of_0v8 ... ok
test no_shoot_through_during_the_start ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 773.20s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/buck_closed_loop.rs (/var/tmp/piperine-target/debug/deps/buck_closed_loop-b7823460891a445e)
```

`Run frozen` - exit 0

```
running 2 tests
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.51s
     Running tests/frozen.rs (/var/tmp/piperine-target/debug/deps/frozen-7080003fc9c2119c)
```

`Run nets` - exit 0

```
running 4 tests
test every_buck_slice_body_compiles_with_no_warning ... ok
test carrier_ports_bind_to_design_nets ... ok
test a_parent_signal_reaches_its_childrens_ports ... ok
test a_net_starts_at_its_own_init ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.45s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-msl-digital/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.56s
     Running tests/digital_circuit.rs (/var/tmp/piperine-target/debug/deps/digital_circuit-9fdef7abfb866f91)
```

`grep -q '^- \*\*An unknown control is a fault' GAPS.md && grep -q '^- \*\*A derived child param is not re-deri` - exit 0

```
(no output)
```

`! grep -q -e 'G3' -e 'saturating_add' -e 'Unresolved' GAPS.md && grep -q '^## G3\. .*(closed: now reported)$' ` - exit 0

```
(no output)
```

`! grep -q 'Nothing executes MSL yet' examples/buck/README.md` - exit 0

```
(no output)
```

`cargo test -p piperine-lang --test pom_resolve a_body_let_gives_its_module_only_the_names_of_its_own_pattern -` - exit 0

```
running 1 test
test a_body_let_gives_its_module_only_the_names_of_its_own_pattern ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 73 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/pom_resolve.rs (/var/tmp/piperine-target/debug/deps/pom_resolve-850bfbe327f73a05)
```
