# generics phase C proofs (tasks-c.md checks, run from a scratch feature dir), after round 6 and the main merge

**HEAD**: 85de31fc3
**Tree**: clean
**Shell**: /bin/sh
**Result**: 120 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C51 | `run elabgen / test encode_and_decode_of_every_layout_lower_through_the_level_cast ... ok` | pass |
| C51 | `run srcgen / test encode_and_decode_run_at_every_layout ... ok` | pass |
| C52 | `run typestraits / test a_hand_written_repr_on_a_generic_type_is_typed_per_argument ... ok` | pass |
| C52 | `run srcgen / test a_generic_hand_written_repr_is_carried_with_its_layout ... ok` | pass |
| C53 | `run srcgen / test a_carrier_of_a_hand_written_type_starts_at_its_default_levels ... ok` | pass |
| C54 | `run typesmods / test generic_module_arguments_are_explicit_or_inferred ... ok` | pass |
| C54 | `run srcmods / test generic_modules_run_with_explicit_and_inferred_arguments ... ok` | pass |
| C55 | `run typesmods / test a_behavior_block_binds_its_modules_generic_names ... ok` | pass |
| C56 | `run typesmods / test behavior_names_that_differ_are_e0403_naming_both_lists ... ok` | pass |
| C57 | `run elabmods / test a_false_module_where_is_e0304_naming_the_instance ... ok` | pass |
| C57 | `run bookmodules / test e0304_a_generic_ladder_of_zero_sections ... ok` | pass |
| C58 | `run typesmods / test const_expressions_in_written_types_are_accepted_everywhere ... ok` | pass |
| C59 | `run elabgen / test a_const_fn_and_a_generic_repeat_fold_in_a_const_context ... ok` | pass |
| C60 | `run elabmods / test one_body_per_distinct_module_instance_and_the_pom_as_written ... ok` | pass |
| C61 | `run irgen / test a_body_instance_is_built_checked_and_printed ... ok` | pass |
| C61 | `run tgen / test module_instance_names_compile_and_list_in_the_library ... ok` | pass |
| C62 | `run srcmods / test short_merges_two_nodes_of_each_nature ... ok` | pass |
| C63 | `run typesmods / test two_natures_for_one_nature_parameter_are_e0180_naming_both ... ok` | pass |
| C63 | `run srcmods / test short_over_two_natures_is_refused_naming_both ... ok` | pass |
| C64 | `run typesgenr / test generated_items_are_typed_as_arrays_by_ordinal ... ok` | pass |
| C64 | `run srcgenr / test generated_instances_run_and_read_by_ordinal ... ok` | pass |
| C65 | `run elabgenr / test generation_for_makes_arrays_and_if_keeps_the_taken_arm ... ok` | pass |
| C65 | `run srcgenr / test a_generation_if_places_the_taken_arm ... ok` | pass |
| C66 | `run typesgenr / test a_generation_range_that_is_not_const_is_e0340 ... ok` | pass |
| C66 | `run bookmodules / test e0340_a_generation_range_read_from_a_signal ... ok` | pass |
| C67 | `run elabgenr / test an_untaken_arm_or_an_index_outside_is_e0342 ... ok` | pass |
| C68 | `run typesgenr / test a_param_inside_generation_is_e0322 ... ok` | pass |
| C69 | `run elabgenr / test a_recursion_by_generation_unrolls ... ok` | pass |
| C70 | `run srcgenr / test the_generic_ladder_equals_the_ladder_written_out ... ok` | pass |
| C70 | `run bookmodules / test the_ladder_runs_for_an_operating_point_and_a_sweep ... ok` | pass |
| C71 | `run elabgenr / test a_generation_recursion_that_does_not_end_is_e0341 ... ok` | pass |
| C72 | `run typesgenr / test generation_in_a_digital_block_is_typed_per_iteration ... ok` | pass |
| C72 | `run srcgenr / test a_generated_bank_reacts_per_element ... ok` | pass |
| C72 | `run srcgenr / test generation_in_a_digital_block_runs_from_every_entry ... ok` | pass |
| C73 | `run elabgenr / test generated_reactions_are_one_per_iteration_in_source_order ... ok` | pass |
| C73 | `run bookmodules / test the_half_reactions_report_each_stage_in_time_order ... ok` | pass |
| C74 | `run elabmods / test a_param_read_by_a_generic_argument_or_a_length_is_structural ... ok` | pass |
| C75 | `run apisession / test a_structural_param_overrides_before_the_run_and_is_refused_during_it ... ok` | pass |
| C76 | `run typesmods / test a_generic_module_body_is_checked_once_at_its_definition ... ok` | pass |
| C76 | `run typesmodel / test an_unreached_generic_module_with_an_error_fails_the_model_check ... ok` | pass |
| C77 | `run interp / test each_generics_form_is_refused_by_name ... ok` | pass |
| C78 | `run typesgen / test phase_d_and_e_forms_stay_p0001_by_name ... ok` | pass |
| C79 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C80 | `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` | pass |
| C81 | `run docs / test gaps_drops_l70 ... ok` | pass |
| C81 | `run docs / test gaps_drops_l69 ... ok` | pass |
| C81 | `run docs / test gaps_drops_s3 ... ok` | pass |
| C81 | `run docs / test gaps_drops_what_generics_phase_c_closed ... ok` | pass |
| C82 | `run bookdigital / test the_flip_flop_runs_for_a_type_with_a_default ... ok` | pass |
| C82 | `run bookcheatsheet / test the_module_card_runs_for_its_const_generic_without_its_value_input ... ok` | pass |
| C82 | `run bookcheatsheet / test the_module_card_runs_with_its_value_input_and_its_out_carrier ... ok` | pass |
| C82 | `run bookcheatsheet / test the_body_items_card_runs_with_its_generation ... ok` | pass |
| C82 | `run bookcorestd / test the_register_with_a_setup_check_runs ... ok` | pass |
| C82 | `run bookmodules / test the_lowpass_module_runs_over_the_generic_integrator ... ok` | pass |
| C82 | `run bookmodules / test the_pi_controller_runs_with_its_generic_idt ... ok` | pass |
| C82 | `run galleries / test the_fences_generics_phase_c_lifted_are_checked ... ok` | pass |
| C82 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C83 | `run elabmods / test a_chain_of_growing_module_arguments_is_e0341 ... ok` | pass |
| C84 | `run srcmods / test a_nature_parameter_takes_its_default ... ok` | pass |
| C84 | `run srcmods / test a_const_generic_of_zero_makes_an_empty_module ... ok` | pass |
| C85 | `run elabgenr / test an_empty_generation_range_makes_an_empty_array ... ok` | pass |
| C86 | `run srcgen / test cast_and_same_from_source_agree_with_the_removed_lowering ... ok` | pass |
| C86 | `run srcgen / test a_cast_to_a_tuple_holding_an_enum_runs ... ok` | pass |
| C86 | `run typesgen / test no_intrinsic_stands_for_cast_or_same ... ok` | pass |
| C86 | `run docs / test gaps_drops_the_cast_and_same_intrinsics ... ok` | pass |
| C120 | `run bookcorestd / test a_carrier_port_left_out_is_e0312_as_the_old_register_listing_was ... ok` | pass |
| C120 | `run bookcheatsheet / test an_array_of_bits_with_no_initial_value_is_e0187 ... ok` | pass |
| C120 | `run booknatures / test the_slice_listing_bounds_t_by_default_as_gallery_e6_does ... ok` | pass |
| C120 | `run typescarrarr / test an_array_of_carriers_without_a_default_is_judged_per_element ... ok` | pass |
| C120 | `run typescarrarr / test an_initial_value_is_the_value_of_the_elements_no_undelayed_definition_names ... ok` | pass |
| C120 | `run typescarrarr / test a_refused_whole_array_definition_is_the_driver_and_raises_no_e0187 ... ok` | pass |
| C120 | `run paritycarrarr / test an_element_given_to_an_out_port_is_driven_by_the_child ... ok` | pass |
| C121 | `run typesmodules / test a_trailing_option_value_input_passed_by_name_binds_it ... ok` | pass |
| C121 | `run typesmodules / test a_named_connection_that_is_no_trailing_option_input_is_e0140 ... ok` | pass |
| C121 | `run elabresults / test a_value_input_passed_by_name_reads_its_argument ... ok` | pass |
| C122 | `run srcgenr / test generation_in_a_digital_block_runs_from_every_entry ... ok` | pass |
| C123 | `run typesmods / test a_generic_placement_feeds_back_through_its_unknown ... ok` | pass |
| C124 | `run typesmods / test a_param_of_a_type_parameter_is_typed_under_its_bounds ... ok` | pass |
| C125 | `run bookdigital / test e0187_the_flip_flop_at_u8_has_no_default ... ok` | pass |
| C125 | `run typesmods / test a_carrier_handed_to_a_generic_out_port_needs_no_initial_value ... ok` | pass |
| C126 | `run typesmods / test an_underscore_generic_argument_is_inferred ... ok` | pass |
| C127 | `run bookmodules / test the_memory_bank_is_refused_for_its_template_and_bundle ... ok` | pass |
| C127 | `run bookcheatsheet / test the_template_card_is_refused_for_its_template_and_bundle ... ok` | pass |
| C127 | `run booknatures / test a_bundle_is_refused_where_the_solver_reaches_it ... ok` | pass |
| C127 | `run bookgenerics / test a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it ... ok` | pass |
| C127 | `run bookwelcome / test the_status_paragraph_lists_what_is_still_refused_and_one_item_runs ... ok` | pass |
| C127 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C128 | `run elabcarrarr / test a_generic_length_array_is_judged_per_element_at_each_instance ... ok` | pass |
| C128 | `run elabcarrarr / test a_param_length_array_is_judged_per_element_at_each_variant ... ok` | pass |
| C128 | `run elabcarrarr / test a_generation_index_names_the_elements_it_reaches ... ok` | pass |
| C129 | `run typescarrarr / test an_initial_value_is_the_value_of_the_elements_no_out_port_holds_and_e0223_once_all_are` | pass |
| C129 | `run elabcarrarr / test an_initial_value_over_elements_no_out_port_holds_is_clean_and_over_every_element_is_e02` | pass |
| C129 | `run paritycarrarr / test an_element_given_to_an_out_port_is_driven_by_the_child ... ok` | pass |
| C130 | `run elabcarrarr / test an_initial_value_beside_definitions_of_every_element_is_e0338_at_the_instance_that_fold` | pass |
| C131 | `run elabgenr / test a_carrier_declared_in_a_generation_for_is_an_array_of_one_element_per_iteration ... ok` | pass |
| C131 | `run elabgenr / test a_definition_in_generation_is_one_drive_per_iteration_its_arm_takes ... ok` | pass |
| C131 | `run elabgenr / test a_hoisted_placement_in_generation_is_one_frame_per_iteration_and_its_let_an_array ... ok` | pass |
| C131 | `run elabgenr / test a_let_in_generation_read_outside_its_loops_is_the_array_of_its_values ... ok` | pass |
| C131 | `run elabgenr / test an_item_only_some_iterations_elaborate_and_generation_in_a_hoisted_module_are_refused_by_n` | pass |
| C131 | `run elabcarrarr / test a_generation_index_names_the_elements_it_reaches ... ok` | pass |
| C131 | `run srcgenr / test carriers_definitions_and_hoisted_placements_in_generation_run ... ok` | pass |
| C131 | `run srcgenr / test a_hoisted_placement_with_contributions_runs_once_per_iteration ... ok` | pass |
| C132 | `run elabcarrarr / test a_nested_array_with_a_generic_inner_length_judges_shared_elements_per_instance ... ok` | pass |
| C132 | `run elabcarrarr / test drives_written_in_generation_are_judged_where_generation_is_unrolled ... ok` | pass |
| C133 | `run paritycarrarr / test a_definition_reading_an_element_a_definition_drives_follows_it ... ok` | pass |
| C133 | `run paritycarrarr / test whole_array_run_time_element_and_reaction_readers_follow_the_elements ... ok` | pass |
| C133 | `run paritycarrarr / test an_analog_block_reads_an_element_of_an_array_of_carriers ... ok` | pass |
| C133 | `run dag / test definitions_of_different_elements_of_one_array_net_chain_without_a_cycle ... ok` | pass |
| C134 | `run elabgenr / test a_member_of_a_generated_instance_is_read_by_its_iteration_or_its_ordinal ... ok` | pass |
| C134 | `run srcgenr / test carriers_definitions_and_hoisted_placements_in_generation_run ... ok` | pass |
| C136 | `run apicarrarr / test an_analog_read_of_an_element_lists_no_internal_net ... ok` | pass |
| C137 | `run elabcarrarr / test a_run_time_element_read_in_a_definition_is_one_read_of_the_array ... ok` | pass |
| C137 | `run paritycarrarr / test a_ram_read_port_of_65536_elements_builds_and_follows_its_address_and_its_element ... ` | pass |
| C137 | `run paritycarrarr / test whole_array_run_time_element_and_reaction_readers_follow_the_elements ... ok` | pass |
| C138 | `run docs / test gaps_l77_is_owned_by_generics_phase_e ... ok` | pass |
| C139 | `run bookcompiler / test a_compiler_code_says_what_the_chapter_says ... ok` | pass |
| C140 | `run apicarrarr / test an_internal_net_is_no_name_a_probe_can_print ... ok` | pass |
| C141 | `run elabcarrarr / test a_loop_of_definitions_through_elements_is_e0339_naming_the_elements ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run elabgen` - exit 0

```
test each_instance_of_a_generic_hand_written_repr_has_its_own_codec ... ok
test a_const_fn_and_a_generic_repeat_fold_in_a_const_context ... ok
test equal_arguments_make_one_instance ... ok
test the_pom_keeps_the_generic_function_as_written ... ok
test instantiation_errors_name_the_item_and_its_arguments ... ok
test a_false_where_is_e0127_naming_the_instantiation ... ok
test a_logic_call_in_a_function_tail_lowers ... ok
test encode_and_decode_of_every_layout_lower_through_the_level_cast ... ok
test a_pattern_call_lowers_in_any_position ... ok
test an_endless_instantiation_chain_is_e0341 ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.19s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_generics.rs (/var/tmp/piperine-target-d/debug/deps/elab_generics-9181231ca608f26c)
```

`Run srcgen` - exit 0

```
test a_hand_written_repr_carries_its_own_layout ... ok
test a_cast_to_a_tuple_holding_an_enum_runs ... ok
test a_function_bounded_by_an_h8_trait_or_repr_runs_per_instance ... ok
test generic_structs_and_enums_instantiate_per_argument_list ... ok
test methods_associated_functions_and_constants_run_with_rusts_meaning ... ok
test generic_functions_run_with_explicit_and_inferred_arguments ... ok
test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok
test a_generic_hand_written_repr_is_carried_with_its_layout ... ok
test cast_and_same_from_source_agree_with_the_removed_lowering ... ok
test encode_and_decode_run_at_every_layout ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.83s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_generics.rs (/var/tmp/piperine-target-d/debug/deps/source_generics-ba610f4d04956ec0)
```

`Run typestraits` - exit 0

```
test impl_trait_for_a_nature_is_accepted ... ok
test an_impl_on_a_module_is_e0130 ... ok
test a_method_named_like_a_carrier_method_is_e0131 ... ok
test a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type ... ok
test a_hand_written_repr_missing_an_item_is_e0187 ... ok
test a_non_function_item_in_an_impl_on_a_nature_is_e0132 ... ok
test a_hand_written_repr_on_a_generic_type_is_typed_per_argument ... ok
test a_hand_written_repr_type_checks_and_is_carried ... ok
test overlapping_impls_are_e0187 ... ok
test extra_items_signatures_supertraits_crate_and_pub_are_e0187 ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.34s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_traits.rs (/var/tmp/piperine-target-d/debug/deps/types_traits-64e37fe997026720)
```

`Run typesmods` - exit 0

```
test two_natures_for_one_nature_parameter_are_e0180_naming_both ... ok
test an_underscore_generic_argument_is_inferred ... ok
test a_generic_placement_feeds_back_through_its_unknown ... ok
test a_carrier_handed_to_a_generic_out_port_needs_no_initial_value ... ok
test a_param_of_a_type_parameter_is_typed_under_its_bounds ... ok
test const_expressions_in_written_types_are_accepted_everywhere ... ok
test a_behavior_block_binds_its_modules_generic_names ... ok
test a_generic_module_body_is_checked_once_at_its_definition ... ok
test behavior_names_that_differ_are_e0403_naming_both_lists ... ok
test generic_module_arguments_are_explicit_or_inferred ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.97s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_generic_modules.rs (/var/tmp/piperine-target-d/debug/deps/types_generic_modules-63c124af6223dbbf)
```

`Run srcmods` - exit 0

```
test short_over_two_natures_is_refused_naming_both ... ok
test equal_arguments_share_one_body ... ok
test a_monitor_over_a_type_reads_its_let_of_the_carrier ... ok
test a_low_pass_over_the_generic_integrator_settles_at_its_input ... ok
test a_const_generic_of_zero_makes_an_empty_module ... ok
test a_nature_parameter_takes_its_default ... ok
test short_merges_two_nodes_of_each_nature ... ok
test a_hoisted_generic_module_commits_to_the_carriers_its_out_ports_are_given ... ok
test generic_modules_run_with_explicit_and_inferred_arguments ... ok
test a_generic_module_takes_a_whole_array_and_a_window_of_one_at_a_port_of_length_rows ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_generic_modules.rs (/var/tmp/piperine-target-d/debug/deps/source_generic_modules-fbdb3e014c53de2a)
```

`Run elabmods` - exit 0

```
running 4 tests
test a_param_read_by_a_generic_argument_or_a_length_is_structural ... ok
test a_false_module_where_is_e0304_naming_the_instance ... ok
test one_body_per_distinct_module_instance_and_the_pom_as_written ... ok
test a_chain_of_growing_module_arguments_is_e0341 ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_generic_modules.rs (/var/tmp/piperine-target-d/debug/deps/elab_generic_modules-7bbedd7d1db91076)
```

`Run bookmodules` - exit 0

```
test the_book_shows_the_integrator_the_standard_library_gives ... ok
test the_lowpass_module_runs_over_the_generic_integrator ... ok
test a_module_body_takes_a_param_with_a_where_and_an_instance_with_a_param_block ... ok
test e0340_a_generation_range_read_from_a_signal ... ok
test e0320_a_block_names_a_param_the_module_does_not_have ... ok
test a_ladder_written_out_runs_an_operating_point_and_a_dc_sweep ... ok
test the_half_reactions_report_each_stage_in_time_order ... ok
test the_ladder_runs_for_an_operating_point_and_a_sweep ... ok
test the_switched_capacitor_integrator_is_a_netlist_that_runs ... ok
test the_pi_controller_runs_with_its_generic_idt ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.65s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book/modules.rs (/var/tmp/piperine-target-d/debug/deps/book_modules-f54c744dbbe5981c)
```

`Run irgen` - exit 0

```
running 6 tests
test a_body_whose_name_disagrees_with_its_instance_is_refused ... ok
test an_instance_whose_name_disagrees_is_refused ... ok
test a_body_instance_is_built_checked_and_printed ... ok
test a_function_instance_is_built_checked_and_printed ... ok
test a_hand_written_repr_with_wrong_signatures_is_refused ... ok
test a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-generics/crates/piperine-ir)
    Finished `test` profile [unoptimized] target(s) in 4.31s
     Running tests/generics.rs (/var/tmp/piperine-target-d/debug/deps/generics-0b08bb68a4775b21)
```

`Run tgen` - exit 0

```
running 3 tests
test instance_names_compile_and_nested_arguments_spell_apart ... ok
test a_hand_written_repr_round_trips_and_none_is_returned_to_the_caller ... ok
test module_instance_names_compile_and_list_in_the_library ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.67s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-generics/crates/piperine-interp)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-generics/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 1.00s
     Running tests/generics.rs (/var/tmp/piperine-target-d/debug/deps/generics-ac3082b36ae9d38d)
```

`Run typesgenr` - exit 0

```
running 4 tests
test a_param_inside_generation_is_e0322 ... ok
test generation_in_a_digital_block_is_typed_per_iteration ... ok
test a_generation_range_that_is_not_const_is_e0340 ... ok
test generated_items_are_typed_as_arrays_by_ordinal ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_generation.rs (/var/tmp/piperine-target-d/debug/deps/types_generation-31ba472fff25117e)
```

`Run srcgenr` - exit 0

```
running 8 tests
test a_hoisted_placement_with_contributions_runs_once_per_iteration ... ok
test a_generation_if_places_the_taken_arm ... ok
test nested_loops_make_a_nested_array ... ok
test generation_in_a_digital_block_runs_from_every_entry ... ok
test generated_instances_run_and_read_by_ordinal ... ok
test a_generated_bank_reacts_per_element ... ok
test the_generic_ladder_equals_the_ladder_written_out ... ok
test carriers_definitions_and_hoisted_placements_in_generation_run ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.68s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_generation.rs (/var/tmp/piperine-target-d/debug/deps/source_generation-eadf17ef959929ea)
```

`Run elabgenr` - exit 0

```
test a_let_in_generation_read_outside_its_loops_is_the_array_of_its_values ... ok
test an_item_only_some_iterations_elaborate_and_generation_in_a_hoisted_module_are_refused_by_name ... ok
test a_carrier_declared_in_a_generation_for_is_an_array_of_one_element_per_iteration ... ok
test a_recursion_by_generation_unrolls ... ok
test generated_reactions_are_one_per_iteration_in_source_order ... ok
test an_untaken_arm_or_an_index_outside_is_e0342 ... ok
test a_member_of_a_generated_instance_is_read_by_its_iteration_or_its_ordinal ... ok
test a_definition_in_generation_is_one_drive_per_iteration_its_arm_takes ... ok
test generation_for_makes_arrays_and_if_keeps_the_taken_arm ... ok
test a_generation_recursion_that_does_not_end_is_e0341 ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/elab_generation.rs (/var/tmp/piperine-target-d/debug/deps/elab_generation-e77cb8dfe3d92284)
```

`Run apisession` - exit 0

```
test an_integer_param_set_by_text_in_range ... ok
test an_option_param_set_to_a_value_or_none ... ok
test a_structural_param_overrides_before_the_run_and_is_refused_during_it ... ok
test an_undriven_top_port_without_default_refuses_the_analysis ... ok
test a_structural_params_bad_text_is_e0802 has been running for over 60 seconds
test an_unknown_variant_is_e0802_listing_the_variants ... ok
test a_structural_params_bad_text_is_e0802 ... ok
test params_passed_through_a_wrappers_placement_run ... ok
test an_integer_above_2_pow_53_is_set_exactly has been running for over 60 seconds
test an_integer_above_2_pow_53_is_set_exactly ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 65.27s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/session.rs (/var/tmp/piperine-target-d/debug/deps/session-f2eb7230d9ebe161)
```

`Run typesmodel` - exit 0

```
running 6 tests
test an_unplaced_module_calling_ddt_from_a_function_is_e0350 ... ok
test the_standard_library_is_left_to_its_own_check ... ok
test an_unreached_function_with_an_error_fails_the_model_check ... ok
test an_unreached_item_has_the_code_it_has_when_reached ... ok
test every_item_kind_of_every_crate_is_checked_unreached ... ok
test an_unreached_generic_module_with_an_error_fails_the_model_check ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_model.rs (/var/tmp/piperine-target-d/debug/deps/types_model-0ee2a7ac1b3e9eba)
```

`Run interp` - exit 0

```
test each_number_width_is_refused_by_name ... ok
test each_generics_form_is_refused_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_interpreter_refuses_each_analog_operator_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-generics/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-generics/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 1.82s
     Running tests/frozen.rs (/var/tmp/piperine-target-d/debug/deps/frozen-1d51dac525b72719)
```

`Run typesgen` - exit 0

```
test an_uninferred_generic_argument_is_e0181_naming_the_parameter ... ok
test generic_arguments_are_explicit_or_inferred ... ok
test fn_bounds_take_closures_and_function_names ... ok
test written_generic_types_lower_with_their_arguments ... ok
test generic_structs_and_enums_type_check_per_instance ... ok
test m_and_dtemp_bound_in_a_param_block_stay_refused_until_phase_d ... ok
test no_intrinsic_stands_for_cast_or_same ... ok
test an_unreached_method_of_a_generic_impl_is_checked_at_its_definition ... ok
test a_call_of_an_extern_intrinsic_types_over_the_logic_of_std ... ok
test phase_d_and_e_forms_stay_p0001_by_name ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.38s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_generics.rs (/var/tmp/piperine-target-d/debug/deps/types_generics-07dcb527b6f69e72)
```

`Run twins` - exit 0

```
running 6 tests
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.65s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-d/debug/deps/twins-89feed4c96f5a89f)
```

`Run docs` - exit 0

```
test the_per_level_feature_is_recorded_as_dropped ... ok
test state_records_the_cli_decision ... ok
test state_records_the_generics_decision ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test transient_performance_closes_v12 ... ok
test the_roadmap_is_forward_looking ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-d/debug/deps/docs-8e80bcc00dcc90ee)
```

`Run bookdigital` - exit 0

```
test the_debouncer_flips_once_at_the_fourth_clean_sample ... ok
test e0501_a_clock_with_no_delay_never_goes_quiet ... ok
test the_library_declares_the_monitors_and_never_runs ... ok
test the_phase_detector_runs_assignment_and_events_read_as_bools ... ok
test the_set_reset_latch_needs_no_reaction ... ok
test a_pipeline_in_one_reaction_reads_every_register_before_the_edge ... ok
test the_pwm_latches_its_duty_word_at_the_wrap ... ok
test the_irq_line_and_the_four_nets_are_refused_until_a_nature_can_implement_resolve ... ok
test the_debouncer_pwm_and_pwm_check_print_their_records ... ok
test the_emit_prints_the_same_first_in_the_middle_or_last ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.41s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/digital.rs (/var/tmp/piperine-target-d/debug/deps/book_digital-39a5776c7074e035)
```

`Run bookcheatsheet` - exit 0

```
test an_array_of_bits_with_no_initial_value_is_e0187 ... ok
test the_nature_card_is_the_standard_librarys ... ok
test the_analog_card_runs ... ok
test the_module_card_runs_with_its_value_input_and_its_out_carrier ... ok
test the_digital_card_runs ... ok
test the_body_items_card_runs_with_its_generation ... ok
test the_module_card_runs_for_its_const_generic_without_its_value_input ... ok
test the_debug_prints_card_runs_with_its_analog_trace ... ok
test the_files_card_runs ... ok
test a_name_of_yours_hides_the_library_but_not_the_core ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.24s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/cheatsheet.rs (/var/tmp/piperine-target-d/debug/deps/book_cheatsheet-1449ebc6201d4733)
```

`Run bookcorestd` - exit 0

```
test the_dac_ramp_scales_its_code_by_the_reference ... ok
test the_latch_takes_its_input_at_the_rising_edge ... ok
test the_probe_writes_a_check_and_a_log_record ... ok
test a_crates_own_vt_replaces_the_library_one ... ok
test the_pad_drives_its_node_and_reads_it_back ... ok
test the_probes_have_the_short_names_the_nature_gives ... ok
test the_watchdog_builds_with_its_u16_param ... ok
test the_tour_module_runs_its_analog_and_digital_blocks ... ok
test the_watchdog_ends_the_run_at_the_fourth_edge_with_the_button_held ... ok
test the_register_with_a_setup_check_runs ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.52s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/core_std.rs (/var/tmp/piperine-target-d/debug/deps/book_core_std-1339ea21078baf85)
```

`Run galleries` - exit 0

```
test galleries_fail_against_an_empty_prelude ... ok
test every_gallery_fence_is_in_the_formatters_form ... ok
test the_fences_composite_values_lifted_are_checked ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test the_fences_generics_phase_c_lifted_are_checked ... ok
test every_gallery_references_its_std_items ... ok
test every_gallery_builds_with_no_diagnostics ... ok
test every_gallery_passes_check ... ok
test no_gallery_has_an_unresolved_member ... ok
test the_gate_builds_16_fences ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.23s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/std_galleries.rs (/var/tmp/piperine-target-d/debug/deps/std_galleries-a171482597723e6e)
```

`Run bookinventory` - exit 0

```
test the_gaps_guard_names_a_missing_and_an_extra_row ... ok
test a_model_across_blocks_names_one_test ... ok
test a_verdict_fits_its_block ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test a_marked_file_joins_its_blocks ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_block_of_the_book_has_one_row ... ok
test every_row_names_an_existing_test ... ok
test the_book_uses_no_removed_name ... ok
test the_book_reads_every_block ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-d/debug/deps/book_inventory-4e223d7a754dd3b2)
```

`Run booknatures` - exit 0

```
test the_slice_listing_bounds_t_by_default_as_gallery_e6_does ... ok
test a_bundle_is_refused_where_the_solver_reaches_it ... ok
test e0310_a_one_driver_port_needs_a_direction ... ok
test the_open_drain_line_is_refused_until_a_nature_can_implement_resolve ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test an_out_port_gives_the_initial_value ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok
test behavior_crosses_freely_inside_one_module ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test the_signal_list_elaborates_and_only_the_net_nature_is_missing ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.94s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/natures.rs (/var/tmp/piperine-target-d/debug/deps/book_natures-573ee03fdb953ba2)
```

`Run typescarrarr` - exit 0

```
test arrays_of_carriers_type_as_signals_and_ports ... ok
test an_array_initial_value_of_another_type_is_e0210 ... ok
test an_initial_value_is_the_value_of_the_elements_no_undelayed_definition_names ... ok
test an_array_argument_of_another_length_is_e0313_naming_both_lengths ... ok
test element_reads_history_and_probes_type_per_element ... ok
test an_initial_value_is_the_value_of_the_elements_no_out_port_holds_and_e0223_once_all_are_handed ... ok
test arrays_and_elements_connect_to_ports_of_their_shape ... ok
test a_definition_and_commits_of_one_element_are_e0221 ... ok
test element_and_field_targets_type_their_levels ... ok
test an_array_of_carriers_without_a_default_is_judged_per_element ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.24s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/types_carrier_arrays-ed79c4da7fa1db0b)
```

`Run paritycarrarr` - exit 0

```
test arrays_and_elements_connect_element_to_element ... ok
test each_element_of_an_array_of_carriers_has_its_own_events ... ok
test an_element_given_to_an_out_port_is_driven_by_the_child ... ok
test overlapping_commits_in_one_microstep_keep_the_last_write_per_level ... ok
test element_history_and_events_apply_to_that_element_only ... ok
test an_array_of_carriers_reads_whole_as_an_array_value ... ok
test the_shift_register_and_the_ram_give_their_expected_sequences ... ok
test an_element_commit_changes_that_element_only ... ok
test whole_array_run_time_element_and_reaction_readers_follow_the_elements ... ok
test a_definition_reading_an_element_a_definition_drives_follows_it ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.79s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/source_carrier_arrays-4c03b8c3ba178d9d)
```

`Run typesmodules` - exit 0

```
test integer_enum_and_option_params_type_check ... ok
test a_named_connection_that_is_no_trailing_option_input_is_e0140 ... ok
test an_expression_on_a_carrier_port_that_is_not_in_is_e0315 ... ok
test a_placement_checks_nodes_count_and_params ... ok
test params_are_f64_or_bool ... ok
test an_environment_function_in_a_const_context_is_e0323 ... ok
test a_trailing_option_value_input_passed_by_name_binds_it ... ok
test a_where_on_a_module_or_fn_is_bool ... ok
test a_core_primitive_is_checked_where_it_is_written ... ok
test ports_and_signals_are_nodes_and_gnd_fits_any ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_modules.rs (/var/tmp/piperine-target-d/debug/deps/types_modules-d13c7d74e247ba8b)
```

`Run elabresults` - exit 0

```
test a_function_that_emits_is_inlined ... ok
test two_placements_are_two_sets_of_carriers ... ok
test an_omitted_option_input_reads_as_none ... ok
test emit_lowers_to_a_site_with_its_texts ... ok
test a_value_input_passed_by_name_reads_its_argument ... ok
test a_result_module_gets_no_body_and_no_instance ... ok
test a_param_block_binds_the_hoisted_params ... ok
test the_stdlib_d2a_gives_transition_the_level_of_its_input ... ok
test a_placement_is_hoisted_under_its_name ... ok
test value_inputs_are_substituted_at_each_read ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_results.rs (/var/tmp/piperine-target-d/debug/deps/elab_results-a19a765d34caece5)
```

`Run bookgenerics` - exit 0

```
running 7 tests
test an_impl_is_compared_with_its_trait ... ok
test e0101_e0905_and_e0903_no_dyn_no_lifetime_and_no_type_alias ... ok
test e0900_a_bound_is_not_a_where_an_impl_type_or_a_qualified_path ... ok
test a_closure_is_passed_to_the_methods_of_an_option ... ok
test a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it ... ok
test the_methods_of_a_trait_run_where_a_top_reaches_them ... ok
test the_generic_listings_run_where_a_top_reaches_them ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.64s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/generics.rs (/var/tmp/piperine-target-d/debug/deps/book_generics-348e6b2c001d9093)
```

`Run bookwelcome` - exit 0

```
test e0319_a_param_with_no_default_must_be_bound ... ok
test e0404_a_commit_belongs_in_a_reaction ... ok
test a_monitor_that_checks_every_edge_writes_one_record_per_edge ... ok
test si_suffixes_are_f64_and_a_plain_number_is_an_integer ... ok
test the_inverter_drives_its_output_a_propagation_delay_after_its_input ... ok
test the_rc_project_holds_its_operating_point_and_its_netlist ... ok
test the_rc_step_charges_as_the_book_says ... ok
test the_delayed_bench_steps_at_one_microsecond ... ok
test adding_or_removing_an_emit_gives_the_same_waveforms ... ok
test the_status_paragraph_lists_what_is_still_refused_and_one_item_runs ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.02s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/welcome.rs (/var/tmp/piperine-target-d/debug/deps/book_welcome-0dfe2f23f65d594b)
```

`Run clibook` - exit 0

```
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test the_chapter_shows_the_session_options ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_chapter_runs_each_new_analysis ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 47.50s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target-d/debug/deps/book-b94a48232bcc738f)
```

`Run elabcarrarr` - exit 0

```
test an_initial_value_beside_definitions_of_every_element_is_e0338_at_the_instance_that_folds_so ... ok
test an_initial_value_over_elements_no_out_port_holds_is_clean_and_over_every_element_is_e0223 ... ok
test a_nested_array_with_a_generic_inner_length_judges_shared_elements_per_instance ... ok
test a_loop_of_definitions_through_elements_is_e0339_naming_the_elements ... ok
test node_array_contributions_unroll_to_their_nodes ... ok
test element_and_field_commits_lower_to_their_targets ... ok
test a_generation_index_names_the_elements_it_reaches ... ok
test element_reads_and_events_lower_per_element ... ok
test an_array_of_length_zero_is_an_empty_array ... ok
test drives_written_in_generation_are_judged_where_generation_is_unrolled ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.35s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/elab_carrier_arrays-193aa187f90f2743)
```

`Run dag` - exit 0

```
running 7 tests
test an_inner_cycle_through_no_relax_net_inside_a_relax_component_is_refused ... ok
test readers_lists_the_functions_that_read_a_net ... ok
test a_cycle_through_relax_nets_is_one_relax_step ... ok
test a_cycle_through_no_relax_net_is_refused_naming_its_functions ... ok
test definitions_of_different_elements_of_one_array_net_chain_without_a_cycle ... ok
test the_order_is_stable_across_runs ... ok
test a_function_runs_after_every_writer_of_its_reads ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/dag.rs (/var/tmp/piperine-target-d/debug/deps/dag-6b6783c4fafd5c44)
```

`Run apicarrarr` - exit 0

```
running 9 tests
test a_name_that_is_no_element_of_the_array_is_e0801 ... ok
test an_internal_net_is_no_name_a_probe_can_print ... ok
test the_metadata_lists_the_length_of_an_array_port ... ok
test an_analog_read_of_an_element_lists_no_internal_net ... ok
test an_empty_array_of_a_hoisted_module_prints_under_the_placement ... ok
test array_probes_read_an_element_and_the_whole_array ... ok
test a_set_of_a_param_an_array_length_reads_is_e0803 ... ok
test a_declared_empty_array_prints_as_the_empty_array ... ok
test a_node_array_prints_one_column_per_node ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.09s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/carrier_arrays-fa234b7fb57296de)
```

`Run bookcompiler` - exit 0

```
running 9 tests
test every_circuit_of_the_comparison_table_has_a_twin_and_a_scenario_test ... ok
test the_intermediate_form_prints_in_a_stable_form ... ok
test a_compiler_code_says_what_the_chapter_says ... ok
test the_rust_backend_prints_one_source_file_and_loads_the_library_it_builds ... ok
test the_reverse_bias_sweep_goes_down_to_minus_20_volts ... ok
test the_divider_puts_mid_at_7_5_volts_and_ngspice_prints_the_same ... ok
test the_reverse_sweep_worst_difference_is_the_one_the_book_prints ... ok
test the_diode_with_a_limited_junction_settles_where_the_junction_equation_says ... ok
test a_digital_if_runs_the_arm_its_condition_selects_and_only_that_arm_commits ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.23s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/compiler.rs (/var/tmp/piperine-target-d/debug/deps/book_compiler-f4a6c68d8a2aabed)
```

`cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` - exit 0

```
test the_walk_covers_the_three_solver_crates ... ok
test root_targets_match_the_naming_rule ... ok
test every_ignored_doc_example_is_registered ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_ignored_tests ... ok
test no_dead_architecture_identifiers ... ok
test no_file_scope_lint_suppression ... ok
test retired_unknown_codes_are_raised_nowhere ... ok
test no_disabled_test_code ... ok
test every_unknown_test_has_a_successor ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.58s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-d/debug/deps/suite_hygiene-04ba8e96779992cb)
```
