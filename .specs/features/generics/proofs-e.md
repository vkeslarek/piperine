# generics-e proofs

**HEAD**: 1c8b3b4e7
**Tree**: clean
**Shell**: /bin/sh
**Result**: 48 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C142 | `run stdlibrary / test each_generics_group_item_resolves_to_its_std_declaration ... ok` | pass |
| C142 | `run stdlibrary / test no_generics_group_item_is_refused_because_generic ... ok` | pass |
| C143 | `run stdlibrary / test a_blocked_item_is_refused_naming_its_form ... ok` | pass |
| C144 | `run typesmodel / test the_standard_library_is_checked_whole_with_no_instantiation ... ok` | pass |
| C145 | `run srcstd / test since_reads_the_time_from_the_last_change ... ok` | pass |
| C145 | `run srcstd / test min_pulse_fails_exactly_for_a_narrow_pulse ... ok` | pass |
| C145 | `run srcstd / test setup_hold_and_stable_fail_exactly_on_their_violations ... ok` | pass |
| C145 | `run srcstd / test inertial_delays_and_swallows_short_pulses ... ok` | pass |
| C145 | `run srcstd / test short_makes_one_node_against_section_eleven ... ok` | pass |
| C146 | `run srcstd / test idt_and_idtmod_integrate_against_section_22 ... ok` | pass |
| C146 | `run srcstd / test the_laplace_family_gives_the_closed_form_step_response ... ok` | pass |
| C146 | `run srcstd / test an_improper_or_unpaired_laplace_is_refused ... ok` | pass |
| C146 | `run srcstd / test the_zi_family_follows_its_difference_equation ... ok` | pass |
| C146 | `run srcstd / test noise_table_gives_the_interpolated_psd ... ok` | pass |
| C147 | `run srcstd / test the_water_model_boils_and_vaporizes ... ok` | pass |
| C148 | `run docs / test gaps_drops_the_generics_group_of_l58 ... ok` | pass |
| C148 | `run docs / test the_generics_checks_file_holds_every_phase ... ok` | pass |
| C148 | `run docs / test the_generics_check_ids_are_unique_across_its_phases ... ok` | pass |
| C135 | `run elabgenr / test generation_in_a_hoisted_module_unrolls_under_its_frame ... ok` | pass |
| C135 | `run srcgenr / test a_hoisted_module_with_generation_runs ... ok` | pass |
| C149 | `run galleries / test the_fences_of_the_generic_library_are_checked ... ok` | pass |
| C149 | `run bookinventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C152 | `run typesresults / test a_composite_param_of_a_module_with_a_result_checks_at_its_placement ... ok` | pass |
| C152 | `run elabresults / test a_composite_param_of_a_hoisted_placement_is_bound_to_its_constant ... ok` | pass |
| C152 | `run elabresults / test an_option_param_of_a_hoisted_placement_is_bound_to_its_constant ... ok` | pass |
| C152 | `run elabtemplates / test a_template_carrying_composite_params_places_with_and_without_a_result ... ok` | pass |
| C152 | `run elabtemplates / test two_templates_with_equal_composite_params_are_one_value ... ok` | pass |
| C152 | `run srcgenr / test a_template_carrying_composite_params_runs_through_a_placement ... ok` | pass |
| C153 | `run elabtemplates / test an_array_of_templates_places_the_element_at_an_index ... ok` | pass |
| C153 | `run elabtemplates / test an_array_of_templates_of_a_module_with_a_result_gives_each_read_its_elements_params .` | pass |
| C153 | `run typestempl / test an_array_of_templates_of_one_module_checks_and_places_its_element ... ok` | pass |
| C153 | `run srcgenr / test a_template_array_element_placed_in_generation_runs ... ok` | pass |
| C153 | `run elabtemplates / test equal_arrays_of_templates_share_one_body_keyed_by_value ... ok` | pass |
| C153 | `run elabresults / test a_module_with_a_result_placed_through_a_callable_bound_param_takes_the_templates_params` | pass |
| C153 | `run srcgenr / test a_module_with_a_result_through_a_callable_bound_param_runs ... ok` | pass |
| C153 | `run elabtemplates / test a_template_param_of_a_module_with_a_result_places_the_templates_params ... ok` | pass |
| C154 | `run elabresults / test an_option_result_of_a_hoisted_module_is_read_as_the_pair_of_its_halves ... ok` | pass |
| C154 | `run srcstd / test inertial_over_an_option_reads_its_result_in_a_reaction ... ok` | pass |
| C155 | `run typestempl / test a_param_declared_as_an_array_of_templates_checks_its_default_and_its_placement ... ok` | pass |
| C155 | `run elabtemplates / test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok` | pass |
| C155 | `run elabtemplates / test the_params_an_array_of_templates_reads_become_structural ... ok` | pass |
| C155 | `run srcgenr / test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok` | pass |
| C155 | `run apisession / test a_module_typed_param_is_not_set_by_text ... ok` | pass |
| C155 | `run docs / test gaps_drops_the_generics_group_of_l58 ... ok` | pass |
| C150 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C151 | `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run stdlibrary` - exit 0

```
test root_params_carry_a_where ... ok
test pure_bodies_name_no_primitive ... ok
test std_declares_no_core_name ... ok
test std_root_comment_names_the_namespaces ... ok
test tgate_places_two_tran ... ok
test std_root_declares_its_namespaces ... ok
test std_has_no_unresolved_member ... ok
test the_index_lists_stay_honest ... ok
test transient_is_the_kind_and_tran_the_module ... ok
test no_generics_group_item_is_refused_because_generic ... ok

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.92s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/std_library.rs (/var/tmp/piperine-target-a/debug/deps/std_library-559768631c9dc7e4)
```

`Run typesmodel` - exit 0

```
running 6 tests
test every_item_kind_of_every_crate_is_checked_unreached ... ok
test an_unreached_function_with_an_error_fails_the_model_check ... ok
test an_unplaced_module_calling_ddt_from_a_function_is_e0350 ... ok
test an_unreached_item_has_the_code_it_has_when_reached ... ok
test the_standard_library_is_checked_whole_with_no_instantiation ... ok
test an_unreached_generic_module_with_an_error_fails_the_model_check ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_model.rs (/var/tmp/piperine-target-a/debug/deps/types_model-0ee2a7ac1b3e9eba)
```

`Run srcstd` - exit 0

```
test inertial_over_an_option_reads_its_result_in_a_reaction ... ok
test since_reads_the_time_from_the_last_change ... ok
test noise_table_gives_the_interpolated_psd ... ok
test min_pulse_fails_exactly_for_a_narrow_pulse ... ok
test idt_and_idtmod_integrate_against_section_22 ... ok
test the_water_model_boils_and_vaporizes ... ok
test the_laplace_family_gives_the_closed_form_step_response ... ok
test the_zi_family_follows_its_difference_equation ... ok
test inertial_delays_and_swallows_short_pulses ... ok
test setup_hold_and_stable_fail_exactly_on_their_violations ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.67s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_stdlib_generic.rs (/var/tmp/piperine-target-a/debug/deps/source_stdlib_generic-8b02640865071e8b)
```

`Run docs` - exit 0

```
test the_buck_spec_holds_the_bench_bounds ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test state_records_the_observation_decision ... ok
test state_records_the_derived_param_decision ... ok
test the_roadmap_is_forward_looking ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test transient_performance_closes_v12 ... ok
test the_generics_check_ids_are_unique_across_its_phases ... ok
test the_generics_checks_file_holds_every_phase ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 64 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-8e80bcc00dcc90ee)
```

`Run elabgenr` - exit 0

```
test a_let_in_generation_read_outside_its_loops_is_the_array_of_its_values ... ok
test a_recursion_by_generation_unrolls ... ok
test generation_in_a_hoisted_module_unrolls_under_its_frame ... ok
test a_hoisted_placement_in_generation_is_one_frame_per_iteration_and_its_let_an_array ... ok
test generated_reactions_are_one_per_iteration_in_source_order ... ok
test a_member_of_a_generated_instance_is_read_by_its_iteration_or_its_ordinal ... ok
test an_untaken_arm_or_an_index_outside_is_e0342 ... ok
test a_definition_in_generation_is_one_drive_per_iteration_its_arm_takes ... ok
test generation_for_makes_arrays_and_if_keeps_the_taken_arm ... ok
test a_generation_recursion_that_does_not_end_is_e0341 ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.22s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_generation.rs (/var/tmp/piperine-target-a/debug/deps/elab_generation-e77cb8dfe3d92284)
```

`Run srcgenr` - exit 0

```
test generation_in_a_digital_block_runs_from_every_entry ... ok
test a_template_array_element_placed_in_generation_runs ... ok
test a_module_with_a_result_through_a_callable_bound_param_runs ... ok
test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok
test generated_instances_run_and_read_by_ordinal ... ok
test a_template_carrying_composite_params_runs_through_a_placement ... ok
test a_generated_bank_reacts_per_element ... ok
test a_hoisted_module_with_generation_runs ... ok
test the_generic_ladder_equals_the_ladder_written_out ... ok
test carriers_definitions_and_hoisted_placements_in_generation_run ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.23s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_generation.rs (/var/tmp/piperine-target-a/debug/deps/source_generation-eadf17ef959929ea)
```

`Run galleries` - exit 0

```
test every_gallery_fence_is_in_the_formatters_form ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test the_fences_composite_values_lifted_are_checked ... ok
test the_fences_generics_lifted_are_checked ... ok
test the_fences_generics_phase_c_lifted_are_checked ... ok
test the_gate_builds_20_fences ... ok
test every_gallery_references_its_std_items ... ok
test every_gallery_builds_with_no_diagnostics ... ok
test no_gallery_has_an_unresolved_member ... ok
test every_gallery_passes_check ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.32s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/std_galleries.rs (/var/tmp/piperine-target-a/debug/deps/std_galleries-a171482597723e6e)
```

`Run bookinventory` - exit 0

```
test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok
test the_owner_and_disputed_rules_name_what_they_break ... ok
test a_marked_file_joins_its_blocks ... ok
test no_row_diverges ... ok
test every_listing_that_does_not_run_names_its_gap ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok
test the_book_uses_no_removed_name ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-4e223d7a754dd3b2)
```

`Run typesresults` - exit 0

```
test a_continuous_result_read_where_probes_are_not_allowed_is_e0354 ... ok
test a_composite_param_of_a_module_with_a_result_checks_at_its_placement ... ok
test a_placement_has_its_result_type ... ok
test an_event_result_outside_an_event_position_is_e0354 ... ok
test a_let_that_reads_itself_is_e0332_or_e0353 ... ok
test trailing_option_inputs_may_be_left_out ... ok
test a_placement_in_a_const_context_is_e0352 ... ok
test a_module_with_a_result_is_typed ... ok
test an_analog_read_of_a_carrier_is_the_value ... ok
test now_and_analysis_static_are_typed ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.66s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_results.rs (/var/tmp/piperine-target-a/debug/deps/types_results-1375aece72e53a01)
```

`Run elabresults` - exit 0

```
test an_omitted_option_input_reads_as_none ... ok
test a_module_with_a_result_placed_through_a_callable_bound_param_takes_the_templates_params ... ok
test a_value_input_passed_by_name_reads_its_argument ... ok
test a_function_that_emits_is_inlined ... ok
test emit_lowers_to_a_site_with_its_texts ... ok
test two_placements_are_two_sets_of_carriers ... ok
test a_placement_is_hoisted_under_its_name ... ok
test the_stdlib_d2a_gives_transition_the_level_of_its_input ... ok
test value_inputs_are_substituted_at_each_read ... ok
test an_option_param_of_a_hoisted_placement_is_bound_to_its_constant ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.04s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_results.rs (/var/tmp/piperine-target-a/debug/deps/elab_results-a19a765d34caece5)
```

`Run elabtemplates` - exit 0

```
test the_params_an_array_of_templates_reads_become_structural ... ok
test two_templates_of_one_generic_module_with_different_arguments_are_two_values ... ok
test an_array_of_templates_places_the_element_at_an_index ... ok
test the_trait_of_a_combined_bound_dispatches_to_the_template_modules_impl ... ok
test a_template_carrying_composite_params_places_with_and_without_a_result ... ok
test two_templates_with_equal_composite_params_are_one_value ... ok
test a_bare_module_name_used_as_a_value_places_its_template_with_every_default ... ok
test templates_fold_from_consts_functions_and_two_levels_of_params ... ok
test a_template_placement_is_identical_to_the_written_placement ... ok
test every_callee_form_elaborates_to_what_it_names ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.02s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_templates.rs (/var/tmp/piperine-target-a/debug/deps/elab_templates-48f7175a2550e82b)
```

`Run typestempl` - exit 0

```
test an_unread_let_of_a_template_item_is_e0181 ... ok
test the_type_of_a_generic_template_carries_its_arguments ... ok
test a_placement_through_a_template_of_a_nature_generic_module_checks_its_ports ... ok
test an_array_of_templates_of_one_module_checks_and_places_its_element ... ok
test a_template_leaves_an_option_param_out_as_a_placement_does ... ok
test a_placement_through_a_template_or_a_function_types_as_its_module ... ok
test a_template_param_value_may_read_the_placing_modules_params ... ok
test gnd_connects_through_a_callable_bound_as_at_a_placement ... ok
test a_bare_module_name_used_as_a_value_is_its_template_with_every_default ... ok
test every_callee_form_the_grammar_admits_is_called_by_its_value ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.07s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/types_templates.rs (/var/tmp/piperine-target-a/debug/deps/types_templates-71532b19f8b42f57)
```

`Run apisession` - exit 0

```
test an_option_param_set_to_a_value_or_none ... ok
test a_structural_params_bad_text_is_e0802 has been running for over 60 seconds
test an_undriven_top_port_without_default_refuses_the_analysis ... ok
test an_unknown_variant_is_e0802_listing_the_variants ... ok
test an_integer_above_2_pow_53_is_set_exactly has been running for over 60 seconds
test an_integer_above_2_pow_53_is_set_exactly ... ok
test params_passed_through_a_wrappers_placement_run ... ok
test a_session_returns_the_statistics_of_its_last_transient ... ok
test a_structural_params_bad_text_is_e0802 ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 69.58s

   Compiling piperine-api v0.2.0 (/home/keslarek/Git/piperine-generics/crates/piperine-api)
    Finished `test` profile [unoptimized] target(s) in 1.22s
     Running tests/session.rs (/var/tmp/piperine-target-a/debug/deps/session-f2eb7230d9ebe161)
```

`Run twins` - exit 0

```
running 6 tests
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.64s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-generics/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 0.61s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-89feed4c96f5a89f)
```

`cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` - exit 0

```
test the_walk_covers_the_three_solver_crates ... ok
test root_targets_match_the_naming_rule ... ok
test every_ignored_doc_example_is_registered ... ok
test no_file_scope_lint_suppression ... ok
test no_dead_architecture_identifiers ... ok
test no_ignored_tests ... ok
test lrm_covers_declarations_are_well_formed ... ok
test retired_unknown_codes_are_raised_nowhere ... ok
test no_disabled_test_code ... ok
test every_unknown_test_has_a_successor ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.91s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-a/debug/deps/suite_hygiene-04ba8e96779992cb)
```
