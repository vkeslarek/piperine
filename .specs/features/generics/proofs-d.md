# generics phase D proofs (tasks-d.md checks, run from a scratch feature dir), round 10

**HEAD**: c2b8a3207
**Tree**: clean
**Shell**: /bin/sh
**Result**: 128 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C87 | `run typestempl / test a_template_is_a_value_of_its_modules_type ... ok` | pass |
| C87 | `run elabtempl / test templates_fold_from_consts_functions_and_two_levels_of_params ... ok` | pass |
| C87 | `run elabtempl / test a_template_of_a_generic_module_names_its_arguments ... ok` | pass |
| C87 | `run elabtempl / test two_templates_of_one_generic_module_with_different_arguments_are_two_values ... ok` | pass |
| C87 | `run elabtempl / test a_template_of_a_generic_module_obeys_its_where_and_names_its_missing_arguments ... ok` | pass |
| C88 | `run typestempl / test a_template_leaving_a_required_param_is_e0319 ... ok` | pass |
| C88 | `run typestempl / test a_template_leaves_an_option_param_out_as_a_placement_does ... ok` | pass |
| C88 | `run elabtempl / test a_template_that_leaves_an_option_param_out_places_none ... ok` | pass |
| C88 | `run srctempl / test a_const_template_leaves_an_option_param_out ... ok` | pass |
| C89 | `run typestempl / test a_template_that_is_not_const_is_e0362 ... ok` | pass |
| C90 | `run typestempl / test a_template_is_never_carried_e0112 ... ok` | pass |
| C91 | `run typestempl / test a_placement_through_a_template_or_a_function_types_as_its_module ... ok` | pass |
| C91 | `run elabtempl / test a_template_placement_is_identical_to_the_written_placement ... ok` | pass |
| C91 | `run bookmodules / test a_function_that_returns_a_template_places_its_result ... ok` | pass |
| C92 | `run typestempl / test a_param_block_on_a_template_placement_is_e0361 ... ok` | pass |
| C93 | `run elabtempl / test a_binning_function_places_the_module_it_chooses ... ok` | pass |
| C93 | `run srctempl / test binning_places_the_chosen_model ... ok` | pass |
| C94 | `run typestempl / test a_module_whose_ports_match_fits_fn_p_names_not_counting ... ok` | pass |
| C94 | `run srctempl / test a_memory_bank_places_its_cell_template ... ok` | pass |
| C94 | `run typestempl / test a_fn_bounded_parameter_is_inferred_from_its_template_param ... ok` | pass |
| C95 | `run typestempl / test a_module_whose_ports_differ_is_e0360_naming_the_first ... ok` | pass |
| C96 | `run elabtempl / test inst_name_through_a_template_param_reads_the_concrete_module ... ok` | pass |
| C96 | `run elabtempl / test a_missing_item_through_a_template_param_is_e0343 ... ok` | pass |
| C96 | `run elabtempl / test inst_name_in_the_module_that_places_the_template_reads_the_concrete_module ... ok` | pass |
| C96 | `run elabtempl / test a_non_node_item_through_a_template_reads_the_concrete_module ... ok` | pass |
| C96 | `run elabtempl / test a_non_node_item_through_a_template_of_another_type_is_e0180_at_elaboration ... ok` | pass |
| C96 | `run elabtempl / test a_generated_cells_param_reads_the_iteration_its_indexes_name ... ok` | pass |
| C96 | `run typestempl / test a_non_node_item_through_a_template_takes_the_type_its_context_wants ... ok` | pass |
| C96 | `run typestempl / test a_cast_of_a_template_item_is_e0181 ... ok` | pass |
| C96 | `run elabtempl / test a_cast_of_a_template_item_is_e0181_not_a_node_conversion ... ok` | pass |
| C96 | `run typestempl / test an_unread_let_of_a_template_item_is_e0181 ... ok` | pass |
| C96 | `run elabtempl / test an_unread_let_of_a_template_item_is_e0181_not_an_elaboration_refusal ... ok` | pass |
| C96 | `run srctempl / test the_items_of_a_template_placed_child_read_as_values ... ok` | pass |
| C96 | `run spectext / test a_template_item_takes_the_type_its_context_wants_in_the_spec ... ok` | pass |
| C96 | `run docs / test gaps_drops_what_generics_phase_d_fix_round_2_closed ... ok` | pass |
| C96 | `run elabtempl / test an_out_of_range_generated_cell_names_its_element_and_the_length_of_its_own_loop ... ok` | pass |
| C97 | `run spectext / test a_module_type_takes_trait_and_inherent_impls_in_the_spec ... ok` | pass |
| C97 | `run spectext / test a_bound_combines_a_trait_and_fn_p_in_the_spec ... ok` | pass |
| C97 | `run typestraits / test an_impl_on_a_module_type_is_accepted ... ok` | pass |
| C97 | `run typestraits / test a_module_impl_function_that_reads_the_solve_is_e0122 ... ok` | pass |
| C97 | `run elabtempl / test a_module_impl_function_runs_on_a_template_at_elaboration ... ok` | pass |
| C97 | `run elabtempl / test the_trait_of_a_combined_bound_dispatches_to_the_template_modules_impl ... ok` | pass |
| C97 | `run srctempl / test the_swappable_mosfet_runs_each_model ... ok` | pass |
| C98 | `run elabimplicit / test the_temperature_call_lowers_to_the_run_read_and_nothing_is_scaled ... ok` | pass |
| C98 | `run srcimplicit / test nothing_set_reads_none ... ok` | pass |
| C99 | `run typesimplicit / test multiplicity_is_a_stdlib_function_over_simparam ... ok` | pass |
| C99 | `run elabimplicit / test multiplicity_lowers_to_the_session_parameter ... ok` | pass |
| C99 | `run elabimplicit / test a_function_that_reaches_the_session_parameter_at_any_depth_reads_it_in_the_body ... ok` | pass |
| C100 | `run srcimplicit / test an_instance_param_combined_with_the_run_multiplicity_is_four_in_parallel ... ok` | pass |
| C100 | `run srcimplicit / test multiplicity_in_a_param_default_is_e0321 ... ok` | pass |
| C101 | `run typesimplicit / test temperature_is_an_option_of_the_run ... ok` | pass |
| C101 | `run srcimplicit / test a_model_chooses_how_to_use_its_temperature ... ok` | pass |
| C101 | `run spectext / test the_recommended_fallback_is_the_ambient_temperature_in_the_spec ... ok` | pass |
| C101 | `run bookanalog / test the_temperature_listings_run_with_the_ambient_fallback ... ok` | pass |
| C102 | `run typesimplicit / test m_and_dtemp_are_ordinary_names ... ok` | pass |
| C102 | `run spectext / test instances_have_no_implicit_params_in_the_spec ... ok` | pass |
| C102 | `run spectext / test temperature_and_multiplicity_are_run_settings_in_the_spec ... ok` | pass |
| C103 | `run srcimplicit / test the_run_settings_hold_for_the_whole_run ... ok` | pass |
| C104 | `run bookmodules / test the_multiplicity_listing_runs_as_four_in_parallel ... ok` | pass |
| C105 | `run interp / test each_generics_form_is_refused_by_name ... ok` | pass |
| C105 | `run irgen / test run_setting_reads_are_built_checked_and_printed ... ok` | pass |
| C105 | `run tgen / test a_device_reads_whether_the_run_set_its_temperature ... ok` | pass |
| C106 | `run docs / test gaps_drops_what_generics_phase_d_closed ... ok` | pass |
| C107 | `run bookmodules / test the_swappable_mosfet_listing_runs ... ok` | pass |
| C107 | `run bookmodules / test the_memory_bank_is_refused_only_for_its_bundle ... ok` | pass |
| C107 | `run bookcheatsheet / test the_template_card_is_refused_only_for_its_bundle ... ok` | pass |
| C107 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C108 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C110 | `run typestempl / test a_param_default_reads_earlier_params_and_a_template_params_trait ... ok` | pass |
| C110 | `run elabparams / test a_default_that_reads_earlier_params_is_folded_per_instance ... ok` | pass |
| C110 | `run elabparams / test a_default_that_reads_itself_or_a_later_param_is_e0321 ... ok` | pass |
| C110 | `run elabtempl / test a_param_default_dispatches_the_trait_of_a_template_param ... ok` | pass |
| C110 | `run srctempl / test a_param_default_reads_the_params_before_it ... ok` | pass |
| C110 | `run spectext / test a_param_default_reads_earlier_params_in_the_spec ... ok` | pass |
| C110 | `run docs / test gaps_drops_what_generics_phase_d_fix_round_2_closed ... ok` | pass |
| C111 | `run typestempl / test the_type_of_a_generic_template_carries_its_arguments ... ok` | pass |
| C111 | `run typestempl / test a_placement_through_a_template_of_a_nature_generic_module_checks_its_ports ... ok` | pass |
| C111 | `run elabtempl / test a_placement_through_a_template_of_a_nature_generic_module_places_its_instance ... ok` | pass |
| C111 | `run srctempl / test a_template_of_a_nature_generic_module_runs_through_its_type ... ok` | pass |
| C111 | `run docs / test gaps_drops_what_generics_phase_d_fix_round_2_closed ... ok` | pass |
| C112 | `run typestempl / test gnd_connects_through_a_callable_bound_as_at_a_placement ... ok` | pass |
| C112 | `run elabtempl / test gnd_at_a_placement_through_a_callable_bound_is_ground ... ok` | pass |
| C112 | `run srctempl / test gnd_connects_through_a_callable_bound ... ok` | pass |
| C113 | `run typeshandles / test a_let_bound_to_a_node_keeps_its_nature_for_a_probe ... ok` | pass |
| C113 | `run elabhandles / test a_let_bound_to_a_node_probes_the_node ... ok` | pass |
| C113 | `run srctempl / test a_let_bound_to_a_node_probes_the_node ... ok` | pass |
| C114 | `run typeshandles / test a_let_bound_to_a_node_is_a_contribution_target ... ok` | pass |
| C114 | `run elabhandles / test a_contribution_through_a_let_bound_to_a_node_is_the_nodes ... ok` | pass |
| C114 | `run srctempl / test a_let_bound_to_a_node_is_a_contribution_target ... ok` | pass |
| C115 | `run typeshandles / test an_access_a_let_bound_nodes_nature_lacks_is_refused_as_for_the_node ... ok` | pass |
| C115 | `run elabhandles / test an_access_a_let_bound_nodes_nature_lacks_is_e0206 ... ok` | pass |
| C116 | `run typeshandles / test parentheses_around_a_node_argument_name_the_node ... ok` | pass |
| C116 | `run elabhandles / test parentheses_around_a_node_argument_lower_to_the_node ... ok` | pass |
| C117 | `run typeshandles / test a_node_chosen_by_the_run_is_e0133 ... ok` | pass |
| C117 | `run typeshandles / test a_node_chosen_at_elaboration_is_typed_as_the_node ... ok` | pass |
| C117 | `run typeshandles / test a_node_chosen_at_a_node_argument_is_e0133_alone ... ok` | pass |
| C117 | `run elabhandles / test a_node_chosen_at_run_time_is_e0133_not_an_elaboration_refusal ... ok` | pass |
| C117 | `run elabhandles / test a_node_chosen_at_a_probe_or_a_target_is_e0133_alone ... ok` | pass |
| C117 | `run elabhandles / test a_node_chosen_at_elaboration_folds_to_the_node ... ok` | pass |
| C117 | `run elabhandles / test a_node_chosen_at_elaboration_connects_through_a_template ... ok` | pass |
| C117 | `run elabhandles / test a_node_chosen_through_a_function_that_reads_the_run_is_e0133 ... ok` | pass |
| C117 | `run elabhandles / test a_node_chosen_through_lets_and_arm_statements_folds_to_the_node ... ok` | pass |
| C117 | `run srctempl / test a_param_picks_the_node_a_module_probes_and_drives ... ok` | pass |
| C117 | `run spectext / test a_node_chosen_by_an_expression_is_e0133_in_the_spec ... ok` | pass |
| C118 | `run typeshandles / test a_node_of_a_child_is_a_connection ... ok` | pass |
| C118 | `run elabhandles / test a_node_of_a_child_connects_to_another_placement ... ok` | pass |
| C118 | `run srctempl / test a_childs_node_connects_to_another_placement ... ok` | pass |
| C119 | `run typeshandles / test a_module_body_is_typed_in_any_order ... ok` | pass |
| C119 | `run elabhandles / test a_body_written_in_reverse_order_elaborates_as_written_forward ... ok` | pass |
| C119 | `run srctempl / test a_body_written_in_reverse_order_runs_as_forward ... ok` | pass |
| C120 | `run typeshandles / test a_cycle_of_node_connections_is_hardware ... ok` | pass |
| C120 | `run elabhandles / test a_placement_binds_the_node_of_a_child_written_after_it ... ok` | pass |
| C120 | `run elabhandles / test a_cycle_of_node_connections_binds_each_port_to_the_node_it_names ... ok` | pass |
| C120 | `run srctempl / test a_placement_connects_to_the_node_of_a_child_written_after_it ... ok` | pass |
| C120 | `run srctempl / test a_cycle_of_node_connections_runs_as_hardware ... ok` | pass |
| C120 | `run srctempl / test a_cycle_through_values_is_refused ... ok` | pass |
| C121 | `run typeshandles / test a_let_assigned_only_values_of_elaboration_chooses_a_node ... ok` | pass |
| C121 | `run elabhandles / test a_let_assigned_only_values_of_elaboration_folds_to_the_node_it_holds_there ... ok` | pass |
| C121 | `run srctempl / test a_let_assigned_from_params_picks_the_node ... ok` | pass |
| C122 | `run typeshandles / test a_cycle_of_node_connections_through_any_depth_of_reach_is_hardware ... ok` | pass |
| C122 | `run elabhandles / test a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port ... ok` | pass |
| C122 | `run srctempl / test a_cycle_of_node_connections_two_levels_deep_runs_as_hardware ... ok` | pass |
| C122 | `run docs / test gaps_records_a_reach_three_levels_deep ... ok` | pass |
| C123 | `run elabhandles / test a_choice_whose_condition_assigns_folds_and_ends ... ok` | pass |
| C123 | `run srctempl / test a_condition_that_assigns_its_let_picks_the_node_and_ends ... ok` | pass |
| C109 | `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run typestempl` - exit 0

```
test a_fn_bounded_parameter_is_inferred_from_its_template_param ... ok
test a_template_leaving_a_required_param_is_e0319 ... ok
test a_cast_of_a_template_item_is_e0181 ... ok
test a_module_whose_ports_differ_is_e0360_naming_the_first ... ok
test a_template_that_is_not_const_is_e0362 ... ok
test the_type_of_a_generic_template_carries_its_arguments ... ok
test an_unread_let_of_a_template_item_is_e0181 ... ok
test a_template_leaves_an_option_param_out_as_a_placement_does ... ok
test gnd_connects_through_a_callable_bound_as_at_a_placement ... ok
test a_placement_through_a_template_of_a_nature_generic_module_checks_its_ports ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.26s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_templates.rs (/var/tmp/piperine-target-a/debug/deps/types_templates-71532b19f8b42f57)
```

`Run elabtempl` - exit 0

```
test inst_name_in_the_module_that_places_the_template_reads_the_concrete_module ... ok
test an_out_of_range_generated_cell_names_its_element_and_the_length_of_its_own_loop ... ok
test gnd_at_a_placement_through_a_callable_bound_is_ground ... ok
test a_placement_through_a_template_of_a_nature_generic_module_places_its_instance ... ok
test a_template_of_a_generic_module_names_its_arguments ... ok
test a_template_placement_is_identical_to_the_written_placement ... ok
test two_templates_of_one_generic_module_with_different_arguments_are_two_values ... ok
test a_non_node_item_through_a_template_reads_the_concrete_module ... ok
test the_trait_of_a_combined_bound_dispatches_to_the_template_modules_impl ... ok
test templates_fold_from_consts_functions_and_two_levels_of_params ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.64s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_templates.rs (/var/tmp/piperine-target-a/debug/deps/elab_templates-48f7175a2550e82b)
```

`Run srctempl` - exit 0

```
test a_body_written_in_reverse_order_runs_as_forward ... ok
test a_cycle_of_node_connections_two_levels_deep_runs_as_hardware ... ok
test binning_places_the_chosen_model ... ok
test gnd_connects_through_a_callable_bound ... ok
test a_param_default_reads_the_params_before_it ... ok
test a_template_of_a_nature_generic_module_runs_through_its_type ... ok
test the_items_of_a_template_placed_child_read_as_values ... ok
test a_placement_connects_to_the_node_of_a_child_written_after_it ... ok
test a_cycle_of_node_connections_runs_as_hardware ... ok
test the_swappable_mosfet_runs_each_model ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.57s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_templates.rs (/var/tmp/piperine-target-a/debug/deps/source_templates-f3f45abd6916a32d)
```

`Run bookmodules` - exit 0

```
test a_function_that_returns_a_template_places_its_result ... ok
test e0340_a_generation_range_read_from_a_signal ... ok
test e0320_a_block_names_a_param_the_module_does_not_have ... ok
test a_ladder_written_out_runs_an_operating_point_and_a_dc_sweep ... ok
test the_ladder_runs_for_an_operating_point_and_a_sweep ... ok
test the_half_reactions_report_each_stage_in_time_order ... ok
test the_switched_capacitor_integrator_is_a_netlist_that_runs ... ok
test the_multiplicity_listing_runs_as_four_in_parallel ... ok
test the_pi_controller_runs_with_its_generic_idt ... ok
test the_swappable_mosfet_listing_runs ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.56s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/modules.rs (/var/tmp/piperine-target-a/debug/deps/book_modules-f54c744dbbe5981c)
```

`Run spectext` - exit 0

```
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test the_recommended_fallback_is_the_ambient_temperature_in_the_spec ... ok
test tran_in_fences_is_the_module ... ok
test the_step_that_reads_a_jump_is_first_order ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test zero_bytes_are_the_default_value ... ok
test every_error_code_is_defined_exactly_once ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok
test the_removed_words_are_absent_from_the_specification ... ok

test result: ok. 145 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_text.rs (/var/tmp/piperine-target-a/debug/deps/spec_text-fc077905224603d9)
```

`Run docs` - exit 0

```
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_buck_readme_points_at_the_scenarios ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test transient_performance_closes_v12 ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test the_roadmap_is_forward_looking ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok
test abi_md_describes_the_unreleased_contract ... ok

test result: ok. 58 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-8e80bcc00dcc90ee)
```

`Run typestraits` - exit 0

```
test trait_methods_resolve_on_values_through_bounds_and_by_path ... ok
test a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type ... ok
test a_trait_plus_fn_bound_admits_only_a_module_that_implements_the_trait ... ok
test impl_trait_for_a_nature_is_accepted ... ok
test a_hand_written_repr_type_checks_and_is_carried ... ok
test a_non_function_item_in_an_impl_on_a_nature_is_e0132 ... ok
test a_hand_written_repr_missing_an_item_is_e0187 ... ok
test a_hand_written_repr_on_a_generic_type_is_typed_per_argument ... ok
test overlapping_impls_are_e0187 ... ok
test extra_items_signatures_supertraits_crate_and_pub_are_e0187 ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.30s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_traits.rs (/var/tmp/piperine-target-a/debug/deps/types_traits-64e37fe997026720)
```

`Run elabimplicit` - exit 0

```
running 3 tests
test multiplicity_lowers_to_the_session_parameter ... ok
test a_function_that_reaches_the_session_parameter_at_any_depth_reads_it_in_the_body ... ok
test the_temperature_call_lowers_to_the_run_read_and_nothing_is_scaled ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_implicit.rs (/var/tmp/piperine-target-a/debug/deps/elab_implicit-2d4196a8cc7ce969)
```

`Run srcimplicit` - exit 0

```
running 5 tests
test multiplicity_in_a_param_default_is_e0321 ... ok
test the_run_settings_hold_for_the_whole_run ... ok
test nothing_set_reads_none ... ok
test a_model_chooses_how_to_use_its_temperature ... ok
test an_instance_param_combined_with_the_run_multiplicity_is_four_in_parallel ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.63s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_implicit.rs (/var/tmp/piperine-target-a/debug/deps/source_implicit-9025362e3484c5ee)
```

`Run typesimplicit` - exit 0

```
running 3 tests
test m_and_dtemp_are_ordinary_names ... ok
test temperature_is_an_option_of_the_run ... ok
test multiplicity_is_a_stdlib_function_over_simparam ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_implicit.rs (/var/tmp/piperine-target-a/debug/deps/types_implicit-ee92cccdd4891539)
```

`Run bookanalog` - exit 0

```
test the_diode_with_a_trace_runs ... ok
test the_clipper_prints_one_clip_record_per_period_near_0_72_volts ... ok
test a_source_with_ac_stim_is_its_dc_value_and_a_one_volt_phasor_in_the_sweep ... ok
test the_clipper_clips_near_the_forward_voltage_and_the_clip_record_confirms_the_hand_calculation ... ok
test e0417_a_contribution_is_an_f64_and_the_fix_runs ... ok
test the_thermal_voltage_is_about_25_9_millivolts_at_the_default_temperature ... ok
test the_temperature_listings_run_with_the_ambient_fallback ... ok
test the_sampling_time_constant_of_a_switch_is_a_tenth_of_a_nanosecond ... ok
test the_nullor_drives_its_output_to_a_virtual_short_and_coupled_inductors_couple ... ok
test the_switch_shorts_its_nodes_while_its_control_is_high ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.61s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/analog.rs (/var/tmp/piperine-target-a/debug/deps/book_analog-221e4c586a515805)
```

`Run interp` - exit 0

```
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test the_level_nodes_are_refused_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_interpreter_refuses_each_analog_operator_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test each_generics_form_is_refused_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test each_number_width_is_refused_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/frozen.rs (/var/tmp/piperine-target-a/debug/deps/frozen-1d51dac525b72719)
```

`Run irgen` - exit 0

```
running 7 tests
test a_body_whose_name_disagrees_with_its_instance_is_refused ... ok
test a_body_instance_is_built_checked_and_printed ... ok
test an_instance_whose_name_disagrees_is_refused ... ok
test run_setting_reads_are_built_checked_and_printed ... ok
test a_hand_written_repr_with_wrong_signatures_is_refused ... ok
test a_function_instance_is_built_checked_and_printed ... ok
test a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/generics.rs (/var/tmp/piperine-target-a/debug/deps/generics-0b08bb68a4775b21)
```

`Run tgen` - exit 0

```
running 4 tests
test instance_names_compile_and_nested_arguments_spell_apart ... ok
test a_hand_written_repr_round_trips_and_none_is_returned_to_the_caller ... ok
test module_instance_names_compile_and_list_in_the_library ... ok
test a_device_reads_whether_the_run_set_its_temperature ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.44s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/generics.rs (/var/tmp/piperine-target-a/debug/deps/generics-ac3082b36ae9d38d)
```

`Run bookcheatsheet` - exit 0

```
test an_array_of_bits_with_no_initial_value_is_e0187 ... ok
test the_nature_card_is_the_standard_librarys ... ok
test the_analog_card_runs ... ok
test the_module_card_runs_for_its_const_generic_without_its_value_input ... ok
test the_module_card_runs_with_its_value_input_and_its_out_carrier ... ok
test the_body_items_card_runs_with_its_generation ... ok
test the_debug_prints_card_runs_with_its_analog_trace ... ok
test the_digital_card_runs ... ok
test the_files_card_runs ... ok
test a_name_of_yours_hides_the_library_but_not_the_core ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.19s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/cheatsheet.rs (/var/tmp/piperine-target-a/debug/deps/book_cheatsheet-1449ebc6201d4733)
```

`Run bookinventory` - exit 0

```
test the_gaps_guard_names_a_missing_and_an_extra_row ... ok
test every_block_of_the_book_has_one_row ... ok
test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok
test no_block_row_is_pending ... ok
test no_row_diverges ... ok
test run_rows_name_tests_that_run_their_model ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok
test the_book_uses_no_removed_name ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-4e223d7a754dd3b2)
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

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.77s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-89feed4c96f5a89f)
```

`Run elabparams` - exit 0

```
test an_option_param_left_out_is_none ... ok
test an_unbound_option_param_is_none ... ok
test an_unset_param_takes_its_default ... ok
test division_by_zero_is_inf_not_an_error ... ok
test n_as_f64_reaches_the_contribution ... ok
test option_param_methods_read_the_bound_case ... ok
test functions_methods_and_logic_fold ... ok
test an_integer_param_is_bound_and_read ... ok
test an_option_param_bound_over_the_parents_param_reads_some_of_it ... ok
test an_option_param_bound_to_a_value_is_some ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_params.rs (/var/tmp/piperine-target-a/debug/deps/elab_params-994acc5dddc0ee7d)
```

`Run typeshandles` - exit 0

```
test a_solve_reading_call_in_a_const_context_is_e0122 ... ok
test a_carrier_method_call_follows_the_lookup_order ... ok
test a_node_chosen_by_the_run_is_e0133 ... ok
test a_node_chosen_at_a_node_argument_is_e0133_alone ... ok
test parentheses_around_a_node_argument_name_the_node ... ok
test an_access_a_let_bound_nodes_nature_lacks_is_refused_as_for_the_node ... ok
test solve_reading_functions_are_accepted_where_the_table_allows ... ok
test a_let_assigned_only_values_of_elaboration_chooses_a_node ... ok
test a_node_chosen_at_elaboration_is_typed_as_the_node ... ok
test a_handle_returned_stored_or_written_is_e0133 ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.84s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_handles.rs (/var/tmp/piperine-target-a/debug/deps/types_handles-263992ad949300d1)
```

`Run elabhandles` - exit 0

```
test a_let_bound_to_a_node_probes_the_node ... ok
test a_placement_binds_the_node_of_a_child_written_after_it ... ok
test an_access_a_let_bound_nodes_nature_lacks_is_e0206 ... ok
test a_node_of_a_child_connects_to_another_placement ... ok
test a_cycle_of_node_connections_binds_each_port_to_the_node_it_names ... ok
test a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port ... ok
test parentheses_around_a_node_argument_lower_to_the_node ... ok
test a_node_chosen_through_lets_and_arm_statements_folds_to_the_node ... ok
test a_choice_whose_condition_assigns_folds_and_ends ... ok
test a_let_assigned_only_values_of_elaboration_folds_to_the_node_it_holds_there ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_handles.rs (/var/tmp/piperine-target-a/debug/deps/elab_handles-62e11896d25669a5)
```

`cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` - exit 0

```
test the_walk_covers_the_three_solver_crates ... ok
test root_targets_match_the_naming_rule ... ok
test every_ignored_doc_example_is_registered ... ok
test no_file_scope_lint_suppression ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_ignored_tests ... ok
test no_dead_architecture_identifiers ... ok
test no_disabled_test_code ... ok
test retired_unknown_codes_are_raised_nowhere ... ok
test every_unknown_test_has_a_successor ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.70s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-a/debug/deps/suite_hygiene-04ba8e96779992cb)
```
