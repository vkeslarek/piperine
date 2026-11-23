# generics proofs

**HEAD**: d90cbaff
**Tree**: clean
**Shell**: /bin/sh
**Result**: 68 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run srcgen / test generic_functions_run_with_explicit_and_inferred_arguments ... ok` | pass |
| C1 | `run srcgen / test a_function_bounded_by_an_h8_trait_or_repr_runs_per_instance ... ok` | pass |
| C2 | `run srcgen / test a_capturing_closure_a_plain_closure_and_a_function_name_pass_for_an_fn_bound ... ok` | pass |
| C3 | `run typesgen / test an_uninferred_generic_argument_is_e0181_naming_the_parameter ... ok` | pass |
| C4 | `run elabgen / test a_false_where_is_e0127_naming_the_instantiation ... ok` | pass |
| C5 | `run elabgen / test one_ir_function_per_distinct_instantiation_named_by_its_arguments ... ok` | pass |
| C5 | `run irgen / test a_function_instance_is_built_checked_and_printed ... ok` | pass |
| C6 | `run elabgen / test the_pom_keeps_the_generic_function_as_written ... ok` | pass |
| C7 | `run srcgen / test methods_associated_functions_and_constants_run_with_rusts_meaning ... ok` | pass |
| C8 | `run typestraits / test an_impl_on_a_module_is_e0130 ... ok` | pass |
| C9 | `run typestraits / test a_method_named_like_a_carrier_method_is_e0131 ... ok` | pass |
| C10 | `run srcgen / test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok` | pass |
| C11 | `run srcgen / test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok` | pass |
| C12 | `run typestraits / test a_blanket_impl_applies_to_every_type_its_bound_admits ... ok` | pass |
| C12 | `run srcgen / test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok` | pass |
| C13 | `run typestraits / test a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type ... ok` | pass |
| C13 | `run typestraits / test overlapping_impls_are_e0187 ... ok` | pass |
| C14 | `run typestraits / test impl_trait_for_a_bundle_is_e0134 ... ok` | pass |
| C14 | `run typestraits / test impl_trait_for_a_nature_is_accepted ... ok` | pass |
| C15 | `run srcgen / test a_hand_written_repr_carries_its_own_layout ... ok` | pass |
| C15 | `run plancomp / test a_hand_written_repr_is_one_leaf_of_its_width ... ok` | pass |
| C15 | `run irgen / test a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed ... ok` | pass |
| C16 | `run srcgen / test a_decode_returning_none_is_returned_to_the_caller ... ok` | pass |
| C17 | `run srcgen / test generic_structs_and_enums_instantiate_per_argument_list ... ok` | pass |
| C18 | `run srcgen / test a_generic_repr_instance_is_laid_out_per_section_2 ... ok` | pass |
| C18 | `run typesgen / test a_generic_repr_type_over_an_argument_without_repr_is_e0110 ... ok` | pass |
| C19 | `run typesgen / test a_generic_body_is_checked_once_at_its_definition ... ok` | pass |
| C20 | `run typesgen / test a_member_no_bound_gives_is_refused_at_the_definition ... ok` | pass |
| C21 | `run elabgen / test instantiation_errors_name_the_item_and_its_arguments ... ok` | pass |
| C22 | `run spectext / test rule_9_splits_definition_and_instantiation_errors ... ok` | pass |
| C23 | `run typesmodel / test every_item_kind_of_every_crate_is_checked_unreached ... ok` | pass |
| C23 | `run apiproject / test a_dependency_crate_is_type_checked ... ok` | pass |
| C24 | `run apiproject / test a_library_package_with_no_run_is_type_checked ... ok` | pass |
| C24 | `run typesmodel / test an_unreached_item_has_the_code_it_has_when_reached ... ok` | pass |
| C24 | `run clicommands / test check_reports_one_error_per_unreached_item_kind ... ok` | pass |
| C25 | `run typesmodel / test an_unreached_function_with_an_error_fails_the_model_check ... ok` | pass |
| C26 | `run typesmodel / test an_unplaced_module_calling_ddt_from_a_function_is_e0350 ... ok` | pass |
| C27 | `run parityproject / test the_source_runner_checks_the_whole_model ... ok` | pass |
| C27 | `run apisession / test a_run_fails_on_an_unreached_error_with_the_check_diagnostic ... ok` | pass |
| C27 | `run clicommands / test build_and_run_fail_with_the_check_diagnostic ... ok` | pass |
| C28 | `run srchandles / test impl_thermal_celsius_reads_temp_minus_273_15 ... ok` | pass |
| C29 | `run typeshandles / test a_carrier_method_call_follows_the_lookup_order ... ok` | pass |
| C30 | `run typestraits / test a_non_function_item_in_an_impl_on_a_nature_is_e0132 ... ok` | pass |
| C31 | `run srchandles / test a_handle_is_read_probed_passed_on_and_has_carrier_methods ... ok` | pass |
| C32 | `run typeshandles / test a_handle_returned_stored_or_written_is_e0133 ... ok` | pass |
| C33 | `run srchandles / test a_discrete_handle_reads_the_value_with_its_flags ... ok` | pass |
| C34 | `run srchandles / test a_nature_generic_handle_function_instantiates_per_nature ... ok` | pass |
| C35 | `run srchandles / test a_probing_function_runs_from_analog_and_a_reaction ... ok` | pass |
| C35 | `run typeshandles / test solve_reading_functions_are_accepted_where_the_table_allows ... ok` | pass |
| C36 | `run typeshandles / test a_solve_reading_call_in_a_const_context_is_e0122 ... ok` | pass |
| C36 | `run spectext / test a_solve_reading_call_in_a_definition_is_e0336_in_the_function_classes ... ok` | pass |
| C37 | `run typeshandles / test a_probe_of_a_handle_in_a_function_is_not_e0450 ... ok` | pass |
| C38 | `run interp / test each_generics_form_is_refused_by_name ... ok` | pass |
| C38 | `run elabhandles / test a_handle_call_lowers_to_its_written_out_body ... ok` | pass |
| C39 | `run typesgen / test phase_c_d_and_e_forms_stay_p0001_by_name ... ok` | pass |
| C40 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C41 | `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` | pass |
| C42 | `run docs / test gaps_drops_what_generics_phase_a_closed ... ok` | pass |
| C42 | `run docs / test gaps_drops_what_generics_phase_b_closed ... ok` | pass |
| C43 | `run galleries / test the_fences_generics_lifted_are_checked ... ok` | pass |
| C44 | `run elabgen / test equal_arguments_make_one_instance ... ok` | pass |
| C45 | `run srcgen / test a_const_generic_of_zero_runs_its_loops_zero_times ... ok` | pass |
| C46 | `run elabgen / test an_endless_instantiation_chain_is_e0341 ... ok` | pass |
| C47 | `run elabgen / test the_elaborator_reads_types_only_through_the_substitution ... ok` | pass |
| C48 | `run tgen / test instance_names_compile_and_nested_arguments_spell_apart ... ok` | pass |
| C49 | `run typesgen / test a_call_of_a_generic_extern_intrinsic_types_through_the_generic_path ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run srcgen` - exit 0

```
test methods_associated_functions_and_constants_run_with_rusts_meaning ... ok
test a_const_context_reads_the_const_generic_arguments ... ok
test a_decode_returning_none_is_returned_to_the_caller ... ok
test a_hand_written_repr_carries_its_own_layout ... ok
test a_generic_repr_instance_is_laid_out_per_section_2 ... ok
test generic_structs_and_enums_instantiate_per_argument_list ... ok
test trait_methods_dispatch_statically_to_their_impl_default_or_blanket ... ok
test a_function_bounded_by_an_h8_trait_or_repr_runs_per_instance ... ok
test generic_functions_run_with_explicit_and_inferred_arguments ... ok
test a_const_generic_of_zero_runs_its_loops_zero_times ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.99s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_generics.rs (/var/tmp/piperine-target-a/debug/deps/source_generics-9185a2b6afbff3ff)
```

`Run typesgen` - exit 0

```
test array_lengths_and_where_are_deferred_to_the_instantiation ... ok
test generic_arguments_are_explicit_or_inferred ... ok
test an_argument_that_misses_a_bound_is_e0187 ... ok
test fn_bounds_take_closures_and_function_names ... ok
test generic_structs_and_enums_type_check_per_instance ... ok
test written_generic_types_lower_with_their_arguments ... ok
test m_and_dtemp_bound_in_a_param_block_stay_refused_until_phase_d ... ok
test an_unreached_method_of_a_generic_impl_is_checked_at_its_definition ... ok
test a_call_of_a_generic_extern_intrinsic_types_through_the_generic_path ... ok
test phase_c_d_and_e_forms_stay_p0001_by_name ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_generics.rs (/var/tmp/piperine-target-a/debug/deps/types_generics-8f09431183b8a8ef)
```

`Run elabgen` - exit 0

```
test the_elaborator_reads_types_only_through_the_substitution ... ok
test a_bound_that_writes_the_trait_arguments_picks_the_impl_of_that_list ... ok
test a_hand_written_repr_on_a_generic_type_is_refused_by_name ... ok
test one_ir_function_per_distinct_instantiation_named_by_its_arguments ... ok
test the_pom_keeps_the_generic_function_as_written ... ok
test a_carrier_of_a_hand_written_type_with_no_initial_value_is_refused_by_name ... ok
test equal_arguments_make_one_instance ... ok
test instantiation_errors_name_the_item_and_its_arguments ... ok
test a_false_where_is_e0127_naming_the_instantiation ... ok
test an_endless_instantiation_chain_is_e0341 ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.46s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_generics.rs (/var/tmp/piperine-target-a/debug/deps/elab_generics-50a8f44f4db0067a)
```

`Run irgen` - exit 0

```
running 4 tests
test an_instance_whose_name_disagrees_is_refused ... ok
test a_function_instance_is_built_checked_and_printed ... ok
test a_hand_written_repr_with_wrong_signatures_is_refused ... ok
test a_type_instance_and_a_hand_written_repr_are_built_checked_and_printed ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/generics.rs (/var/tmp/piperine-target-a/debug/deps/generics-0b08bb68a4775b21)
```

`Run typestraits` - exit 0

```
test impl_trait_for_a_nature_is_accepted ... ok
test an_impl_on_a_module_is_e0130 ... ok
test a_method_named_like_a_carrier_method_is_e0131 ... ok
test impl_trait_for_a_bundle_is_e0134 ... ok
test a_trait_impl_missing_an_item_is_e0187_naming_trait_and_type ... ok
test a_hand_written_repr_type_checks_and_is_carried ... ok
test a_non_function_item_in_an_impl_on_a_nature_is_e0132 ... ok
test a_hand_written_repr_missing_an_item_is_e0187 ... ok
test overlapping_impls_are_e0187 ... ok
test extra_items_signatures_supertraits_crate_and_pub_are_e0187 ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.98s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/types_traits.rs (/var/tmp/piperine-target-a/debug/deps/types_traits-3dfdc1b1302d9028)
```

`Run plancomp` - exit 0

```
running 6 tests
test a_constant_is_the_bytes_the_layout_names ... ok
test composite_layouts_list_their_leaves ... ok
test an_empty_struct_is_carried_with_zero_levels ... ok
test two_pins_of_a_composite_net_have_equal_layouts ... ok
test a_one_variant_enum_lays_out_its_payload_only ... ok
test a_hand_written_repr_is_one_leaf_of_its_width ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/composite_shape.rs (/var/tmp/piperine-target-a/debug/deps/composite_shape-e5279a28cf15b2eb)
```

`Run spectext` - exit 0

```
test value_blocks_may_hold_contributions_and_commits ... ok
test the_step_that_reads_a_jump_is_first_order ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test the_library_declares_no_net_nature ... ok
test zero_bytes_are_the_default_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok
test the_removed_words_are_absent_from_the_specification ... ok

test result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_text.rs (/var/tmp/piperine-target-a/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run typesmodel` - exit 0

```
running 5 tests
test an_unreached_function_with_an_error_fails_the_model_check ... ok
test an_unplaced_module_calling_ddt_from_a_function_is_e0350 ... ok
test every_item_kind_of_every_crate_is_checked_unreached ... ok
test the_standard_library_is_left_to_its_own_check ... ok
test an_unreached_item_has_the_code_it_has_when_reached ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_model.rs (/var/tmp/piperine-target-a/debug/deps/types_model-dc6987995b3e205b)
```

`Run apiproject` - exit 0

```
test a_file_under_src_is_refused_with_p0010 ... ok
test a_missing_bench_file_is_p0010_at_the_value ... ok
test the_only_run_is_run_without_a_name ... ok
test a_dependencys_run_table_is_ignored ... ok
test a_dependency_crate_is_type_checked ... ok
test a_bench_is_the_root_crate_over_the_package_and_its_dependencies ... ok
test a_run_without_a_bench_roots_at_the_package ... ok
test a_file_under_a_package_is_its_bench ... ok
test a_lone_file_with_no_manifest_is_a_crate_alone ... ok
test a_library_package_with_no_run_is_type_checked ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/project.rs (/var/tmp/piperine-target-a/debug/deps/project-23b1650b4d14a88c)
```

`Run clicommands` - exit 0

```
test a_second_build_is_cached_and_compiles_nothing ... ok
test test_prints_target_margins_after_the_run_line ... ok
test test_prints_no_table_and_no_other_record ... ok
test test_prints_pass_with_the_check_count ... ok
test a_new_project_checks_runs_and_tests ... ok
test test_prints_fail_and_the_failing_records ... ok
test the_last_line_counts_and_the_exit_code_follows ... ok
test clean_cache_removes_the_compiled_cache ... ok
test verbose_prints_a_source_path_that_exists ... ok
test test_runs_every_run_or_the_named_ones ... ok

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.08s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/commands.rs (/var/tmp/piperine-target-a/debug/deps/commands-e62622f9813395ac)
```

`Run parityproject` - exit 0

```
running 3 tests
test the_source_runner_checks_the_whole_model ... ok
test a_project_with_a_dependency_crate_loads_and_runs ... ok
test a_diagnostic_is_returned_with_its_stage ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/project.rs (/var/tmp/piperine-target-a/debug/deps/project-c0a010504379005e)
```

`Run apisession` - exit 0

```
test a_leaf_text_is_read_by_the_leafs_type ... ok
test a_refused_text_is_named_as_the_user_wrote_it ... ok
test an_integer_param_set_by_text_in_range ... ok
test an_option_param_set_to_a_value_or_none ... ok
test a_session_returns_the_statistics_of_its_last_transient ... ok
test an_undriven_top_port_without_default_refuses_the_analysis ... ok
test an_unknown_variant_is_e0802_listing_the_variants ... ok
test a_structural_params_bad_text_is_e0802 ... ok
test params_passed_through_a_wrappers_placement_run ... ok
test an_integer_above_2_pow_53_is_set_exactly ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 59.10s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/session.rs (/var/tmp/piperine-target-a/debug/deps/session-b8872f77132e3cb3)
```

`Run srchandles` - exit 0

```
running 6 tests
test a_nature_generic_handle_function_instantiates_per_nature ... ok
test impl_thermal_celsius_reads_temp_minus_273_15 ... ok
test a_handle_generic_over_the_value_of_its_carrier_calls_prev ... ok
test a_handle_is_read_probed_passed_on_and_has_carrier_methods ... ok
test a_probing_function_runs_from_analog_and_a_reaction ... ok
test a_discrete_handle_reads_the_value_with_its_flags ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.72s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/source_handles.rs (/var/tmp/piperine-target-a/debug/deps/source_handles-63252e3acca26e9e)
```

`Run typeshandles` - exit 0

```
running 8 tests
test a_carrier_generic_over_its_value_type_is_a_handle ... ok
test a_probe_of_a_handle_in_a_function_is_not_e0450 ... ok
test a_solve_reading_call_in_a_const_context_is_e0122 ... ok
test a_handle_is_read_probed_passed_on_and_has_carrier_methods_in_the_checker ... ok
test solve_reading_functions_are_accepted_where_the_table_allows ... ok
test a_nature_generic_handle_function_type_checks_and_records_its_nature ... ok
test a_carrier_method_call_follows_the_lookup_order ... ok
test a_handle_returned_stored_or_written_is_e0133 ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.06s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_handles.rs (/var/tmp/piperine-target-a/debug/deps/types_handles-9bc1d3f2e7e797d5)
```

`Run interp` - exit 0

```
test a_table_expression_is_refused_by_name ... ok
test a_host_source_body_is_rejected_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test the_level_nodes_are_refused_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test each_generics_form_is_refused_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/frozen.rs (/var/tmp/piperine-target-a/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run elabhandles` - exit 0

```
running 2 tests
test a_handle_call_lowers_to_its_written_out_body ... ok
test a_recursion_through_a_handle_function_is_e0126 ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_handles.rs (/var/tmp/piperine-target-a/debug/deps/elab_handles-4ba0c4ea0eccf2b1)
```

`Run twins` - exit 0

```
running 6 tests
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.12s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-9b01bd7200d69219)
```

`Run docs` - exit 0

```
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test transient_performance_closes_v12 ... ok
test state_records_the_generics_decision ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_roadmap_is_forward_looking ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```

`Run galleries` - exit 0

```
test every_fixture_line_is_spec_text ... ok
test galleries_fail_against_an_empty_prelude ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test every_gallery_fence_is_in_the_formatters_form ... ok
test the_fences_composite_values_lifted_are_checked ... ok
test every_gallery_builds_with_no_diagnostics ... ok
test every_gallery_references_its_std_items ... ok
test no_gallery_has_an_unresolved_member ... ok
test every_gallery_passes_check ... ok
test the_gate_builds_16_fences ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.59s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/std_galleries.rs (/var/tmp/piperine-target-a/debug/deps/std_galleries-20eb711d5dacf84a)
```

`Run tgen` - exit 0

```
running 2 tests
test instance_names_compile_and_nested_arguments_spell_apart ... ok
test a_hand_written_repr_round_trips_and_none_is_returned_to_the_caller ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/generics.rs (/var/tmp/piperine-target-a/debug/deps/generics-afab0e3ff8e1a861)
```

`cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` - exit 0

```
test every_ignored_doc_example_is_registered ... ok
test root_targets_match_the_naming_rule ... ok
test the_walk_covers_the_three_solver_crates ... ok
test no_file_scope_lint_suppression ... ok
test no_ignored_tests ... ok
test no_dead_architecture_identifiers ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_disabled_test_code ... ok
test retired_unknown_codes_are_raised_nowhere ... ok
test every_unknown_test_has_a_successor ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.37s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-a/debug/deps/suite_hygiene-354b8333c6c0effe)
```
