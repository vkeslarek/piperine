# composite-values proofs

**HEAD**: 3c2e2522
**Tree**: clean
**Shell**: /bin/sh
**Result**: 109 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run srcctl / test let_mut_and_every_compound_operator_give_rusts_values ... ok` | pass |
| C2 | `run typesctl / test assigning_a_non_place_is_e0183_naming_the_target ... ok` | pass |
| C3 | `run errors / test let_mut_in_a_module_body_is_e0903 ... ok` | pass |
| C4 | `run srcctl / test every_loop_form_gives_rusts_values ... ok` | pass |
| C5 | `run elabctl / test an_analog_loop_that_contributes_or_places_is_unrolled ... ok` | pass |
| C6 | `run elabctl / test a_contributing_analog_loop_without_a_constant_trip_count_is_e0416 ... ok` | pass |
| C7 | `run srcctl / test an_analog_loop_without_contribution_is_differentiated_through ... ok` | pass |
| C8 | `run srcctl / test a_commit_in_a_reaction_loop_is_one_statement_last_write_winning ... ok` | pass |
| C9 | `run srcctl / test an_early_return_gives_the_caller_its_value ... ok` | pass |
| C10 | `run srcctl / test the_question_mark_on_none_returns_none ... ok` | pass |
| C11 | `run typesctl / test return_outside_a_function_is_e0185 ... ok` | pass |
| C12 | `run srcctl / test a_match_guard_takes_the_arm_only_when_true ... ok` | pass |
| C13 | `run srcctl / test range_patterns_match_as_rust ... ok` | pass |
| C14 | `run typesctl / test an_integer_match_covered_by_ranges_is_exhaustive ... ok` | pass |
| C15 | `run srcctl / test or_patterns_and_at_bindings_bind_as_rust ... ok` | pass |
| C16 | `run typesctl / test alternatives_binding_differently_are_e0184 ... ok` | pass |
| C17 | `run srcctl / test refutable_patterns_run_in_if_let_while_let_and_let_else ... ok` | pass |
| C18 | `run typesctl / test a_non_exhaustive_match_or_refutable_let_is_e0184 ... ok` | pass |
| C19 | `run srcctl / test closures_run_and_capture_by_value ... ok` | pass |
| C20 | `run typesctl / test a_closure_called_outside_its_class_is_e0122 ... ok` | pass |
| C21 | `run srcctl / test a_closure_in_analog_is_differentiated_through ... ok` | pass |
| C22 | `run typesctl / test a_closure_with_an_excluded_form_in_a_fn_is_e0121 ... ok` | pass |
| C23 | `run srcctl / test a_function_value_calls_the_named_function ... ok` | pass |
| C24 | `run typesctl / test a_non_capturing_closure_is_a_function_pointer ... ok` | pass |
| C25 | `run typesctl / test a_capturing_closure_is_not_a_function_pointer ... ok` | pass |
| C26 | `run typesctl / test a_function_value_is_not_carried_e0112 ... ok` | pass |
| C27 | `run srcctl / test recursion_from_a_reaction_runs ... ok` | pass |
| C28 | `run elabctl / test a_recursion_reached_from_analog_is_unrolled ... ok` | pass |
| C29 | `run elabctl / test an_unfoldable_recursion_from_analog_is_e0126 ... ok` | pass |
| C30 | `run srcctl / test runaway_loops_and_recursions_fail_the_analysis_by_name ... ok` | pass |
| C31 | `run rt / test rt::control::tests::trips_panic_past_two_to_the_twenty_four ... ok` | pass |
| C32 | `run rt / test rt::control::tests::depth_panics_past_one_thousand_twenty_four ... ok` | pass |
| C33 | `run spectext / test section_1_rule_11_states_the_loop_and_depth_bounds ... ok` | pass |
| C34 | `run tctl / test unknown_decisions_merge_written_locals_and_loop_exits ... ok` | pass |
| C35 | `run rt / test rt::control::tests::unrolled_cuts_at_sixty_four ... ok` | pass |
| C36 | `run tctl / test a_reaction_loop_committing_after_an_unknown_decision_commits_maybe ... ok` | pass |
| C37 | `run typesagg / test every_struct_form_is_a_value_type ... ok` | pass |
| C38 | `run srcagg / test struct_expressions_give_rusts_values ... ok` | pass |
| C39 | `run typesagg / test struct_update_syntax_is_a_parse_error ... ok` | pass |
| C40 | `run srcagg / test field_reads_give_the_fields ... ok` | pass |
| C41 | `run typesagg / test a_bad_struct_expression_is_e0186_naming_the_field ... ok` | pass |
| C42 | `run srcagg / test assigning_a_field_changes_that_field_only ... ok` | pass |
| C43 | `run srcagg / test tuples_give_rusts_values ... ok` | pass |
| C44 | `run srcagg / test tuple_patterns_bind_each_component ... ok` | pass |
| C45 | `run srcagg / test payload_variants_build_and_match ... ok` | pass |
| C46 | `run srcagg / test discriminants_are_the_tag_and_the_cast ... ok` | pass |
| C47 | `run typesagg / test a_discriminant_that_does_not_fit_or_repeats_is_e0111 ... ok` | pass |
| C48 | `run plancomp / test a_one_variant_enum_lays_out_its_payload_only ... ok` | pass |
| C49 | `run srcagg / test options_of_aggregates_behave_as_enums ... ok` | pass |
| C50 | `run srcagg / test derived_comparisons_and_defaults_give_rusts_results ... ok` | pass |
| C51 | `run typesagg / test an_enum_deriving_default_is_refused ... ok` | pass |
| C52 | `run typesagg / test comparing_without_the_trait_is_refused_naming_it ... ok` | pass |
| C53 | `run typesagg / test debug_and_hash_derives_are_accepted ... ok` | pass |
| C54 | `run srcvs / test a_value_is_read_twice ... ok` | pass |
| C55 | `run srcagg / test arrays_give_rusts_values ... ok` | pass |
| C56 | `run srcagg / test a_range_index_gives_the_array_of_its_elements ... ok` | pass |
| C57 | `run srcagg / test an_index_out_of_range_panics_at_run_time ... ok` | pass |
| C58 | `run elabagg / test an_index_out_of_range_in_a_const_is_e0104 ... ok` | pass |
| C59 | `run typesvs / test a_range_index_out_of_the_array_is_refused ... ok` | pass |
| C60 | `run typesagg / test a_non_constant_array_length_is_refused_naming_it ... ok` | pass |
| C61 | `run srcagg / test an_unknown_index_reads_and_writes_by_merge ... ok` | pass |
| C65 | `run srcagg / test every_row_of_h5_to_h7_gives_rusts_result ... ok` | pass |
| C66 | `run srcagg / test for_visits_elements_in_rusts_order ... ok` | pass |
| C67 | `run typesagg / test a_method_outside_appendix_h_is_e0161 ... ok` | pass |
| C68 | `run srcagg / test an_empty_array_has_length_zero_and_no_first ... ok` | pass |
| C69 | `run srcagg / test the_zi_nd_reaction_value_code_runs_as_a_function ... ok` | pass |
| C70 | `run srcagg / test normal_matches_the_spec_text_bit_for_bit ... ok` | pass |
| C71 | `run srcagg / test normal_with_zero_sigma_returns_the_mean ... ok` | pass |
| C72 | `run srcagg / test the_jitter_clock_repeats_with_its_seed ... ok` | pass |
| C73 | `run srcagg / test derived_repr_layout_and_width_follow_section_2 ... ok` | pass |
| C74 | `run typesagg / test deriving_repr_with_a_field_without_repr_is_e0110 ... ok` | pass |
| C75 | `run srcagg / test composite_carriers_initialize_define_commit_and_change ... ok` | pass |
| C76 | `run srcagg / test committing_an_equal_composite_fires_no_change ... ok` | pass |
| C77 | `run srcagg / test a_tag_naming_no_variant_reads_unknown ... ok` | pass |
| C78 | `run tagg / test narrow_payloads_pad_with_false_levels ... ok` | pass |
| C79 | `run srcagg / test composite_params_bind_and_read_everywhere ... ok` | pass |
| C80 | `run apicomp / test a_host_sets_a_composite_param_by_leaf_and_checks_its_where ... ok` | pass |
| C81 | `run srcagg / test a_record_carries_a_composite_field_and_its_type ... ok` | pass |
| C82 | `run typesagg / test two_sites_giving_a_field_two_composite_types_is_e0605 ... ok` | pass |
| C83 | `run iragg / test ir_check_types_every_aggregate_expression ... ok` | pass |
| C84 | `run tagg / test every_composite_value_crosses_the_abi_unchanged ... ok` | pass |
| C85 | `run plancomp / test two_pins_of_a_composite_net_have_equal_layouts ... ok` | pass |
| C86 | `run tagg / test each_source_type_prints_as_one_rust_item_naming_its_path ... ok` | pass |
| C87 | `run clirun / test composites_print_in_each_output_format ... ok` | pass |
| C88 | `run clirun / test a_leaf_path_prints_that_leaf_alone ... ok` | pass |
| C89 | `run apicomp / test a_carried_enum_reads_as_its_variant_name ... ok` | pass |
| C90 | `run apicomp / test enum_names_live_in_the_ir_not_in_the_net_bytes ... ok` | pass |
| C91 | `run interp / test each_composite_values_form_is_refused_by_name ... ok` | pass |
| C92 | `run typesagg / test phase_c_and_d_forms_stay_p0001_by_name ... ok` | pass |
| C93 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C94 | `cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` | pass |
| C95 | `run elabctl / test phase_a_forms_fold_in_const_contexts ... ok` | pass |
| C96 | `run elabagg / test aggregates_fold_in_const_contexts ... ok` | pass |
| C97 | `run elabctl / test a_panic_in_a_const_evaluation_names_the_expression ... ok` | pass |
| C98 | `run docs / test gaps_drops_what_composite_values_phase_a_closed ... ok` | pass |
| C99 | `run docs / test gaps_drops_what_composite_values_phase_b_closed ... ok` | pass |
| C100 | `run galleries / test the_fences_composite_values_lifted_are_checked ... ok` | pass |
| C101 | `run srcctl / test an_empty_range_runs_zero_times_in_analog_too ... ok` | pass |
| C102 | `run plancomp / test an_empty_struct_is_carried_with_zero_levels ... ok` | pass |
| C103 | `run irctl / test recursion_reached_from_analog_is_refused ... ok` | pass |
| C104 | `run irctl / test a_loop_reaching_a_probe_on_a_later_iteration_reads_it ... ok` | pass |
| C105 | `run typesagg / test an_enum_without_repr_is_a_value_but_not_a_carried_type ... ok` | pass |
| C106 | `run booktypes / test an_enum_without_repr_runs_as_a_value ... ok` | pass |
| C107 | `run srcctl / test option_arms_take_rusts_patterns ... ok` | pass |
| C108 | `run srcagg / test an_unknown_composite_is_refused_by_name ... ok` | pass |
| C109 | `run typesagg / test generic_arguments_on_a_plain_type_are_refused ... ok` | pass |
| C110 | `run srcctl / test a_match_or_if_let_yielding_an_option_runs_as_rust ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run srcctl` - exit 0

```
test every_loop_form_gives_rusts_values ... ok
test option_arms_take_rusts_patterns ... ok
test a_commit_in_a_reaction_loop_is_one_statement_last_write_winning ... ok
test or_patterns_and_at_bindings_bind_as_rust ... ok
test range_patterns_match_as_rust ... ok
test recursion_from_a_reaction_runs ... ok
test the_question_mark_on_none_returns_none ... ok
test refutable_patterns_run_in_if_let_while_let_and_let_else ... ok
test a_match_guard_takes_the_arm_only_when_true ... ok
test runaway_loops_and_recursions_fail_the_analysis_by_name ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.59s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/source_control.rs (/var/tmp/piperine-target-c/debug/deps/source_control-99b28fe19d2a67b0)
```

`Run typesctl` - exit 0

```
test return_outside_a_function_is_e0185 ... ok
test refutable_patterns_type_check_in_if_let_while_let_and_let_else ... ok
test closures_type_check_where_rust_allows_them ... ok
test direct_and_mutual_recursion_type_check ... ok
test a_non_exhaustive_match_or_refutable_let_is_e0184 ... ok
test let_mut_and_every_compound_operator_type_check ... ok
test return_and_question_mark_type_check ... ok
test every_loop_form_type_checks ... ok
test assigning_a_non_place_is_e0183_naming_the_target ... ok
test a_function_value_is_not_carried_e0112 ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_control.rs (/var/tmp/piperine-target-c/debug/deps/types_control-02f816d4cc5bd9e2)
```

`Run errors` - exit 0

```
test unexpected_token_names_found_and_expected ... ok
test parse_phase_rows_report_their_codes ... ok
test removed_rust_forms_report_targeted_messages ... ok
test nesting_without_delimiters_is_also_bounded ... ok
test depth_over_256_is_diagnostic_not_overflow ... ok
test depth_limit_fits_8mib_stack_in_debug ... ok
test flat_chain_past_4096_links_is_e0910 ... ok
test comparison_lists_are_not_nesting ... ok
test flat_chains_up_to_4096_links_are_not_nesting ... ok
test flat_chains_survive_a_default_2_mib_thread ... ok

test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.96s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/msl_errors.rs (/var/tmp/piperine-target-c/debug/deps/msl_errors-7ad5845525a83174)
```

`Run elabctl` - exit 0

```
test a_function_returning_an_option_is_two_ir_functions ... ok
test a_recursion_reached_from_analog_is_unrolled ... ok
test loops_lower_to_the_loops_of_the_ir ... ok
test a_function_value_called_with_an_unknown_is_the_flagged_variant_or_p0001 ... ok
test let_mut_lowers_to_mutable_locals_and_assignments ... ok
test an_unfoldable_recursion_from_analog_is_e0126 ... ok
test an_analog_loop_that_contributes_or_places_is_unrolled ... ok
test phase_a_forms_fold_in_const_contexts ... ok
test a_contributing_analog_loop_without_a_constant_trip_count_is_e0416 ... ok
test a_panic_in_a_const_evaluation_names_the_expression ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.57s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_control.rs (/var/tmp/piperine-target-c/debug/deps/elab_control-55cc686e4c56fa9c)
```

`Run rt` - exit 0

```
test rt::real::tests::min_max_clamp_and_floor_carry_the_tangent_of_the_value_chosen ... ok
test rt::real::tests::the_differentiable_rows_agree_with_finite_differences ... ok
test rt::real::tests::the_piecewise_rows_have_derivative_zero_or_one ... ok
test rt::real::tests::f64_is_a_real ... ok
test rt::real::tests::dual_gives_the_interpreters_values_and_tangents ... ok
test rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result ... ok
test rt::real::tests::each_row_has_the_value_of_rusts_method ... ok
test rt::int::tests::integer_rows_wrap_saturate_and_test_overflow ... ok
test rt::int::tests::arithmetic_wraps_at_every_width ... ok
test rt::control::tests::trips_panic_past_two_to_the_twenty_four ... ok

test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.12s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running unittests src/lib.rs (/var/tmp/piperine-target-c/debug/deps/piperine_transpile-6d48b28f4fb0a670)
```

`Run spectext` - exit 0

```
test s36_defines_the_package_codes ... ok
test tran_in_fences_is_the_module ... ok
test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test the_library_declares_no_net_nature ... ok
test zero_bytes_are_the_default_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok

test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_text.rs (/var/tmp/piperine-target-c/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run tctl` - exit 0

```
running 6 tests
test a_recursion_past_the_depth_bound_faults_by_name ... ok
test function_values_and_recursion_compile_and_run ... ok
test known_control_prints_plain_rust_and_runs ... ok
test a_known_loop_past_the_bound_panics_naming_it ... ok
test unknown_decisions_merge_written_locals_and_loop_exits ... ok
test a_reaction_loop_committing_after_an_unknown_decision_commits_maybe ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.85s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/control.rs (/var/tmp/piperine-target-c/debug/deps/control-bd6900f5ad3bc0d6)
```

`Run typesagg` - exit 0

```
test every_struct_form_is_a_value_type ... ok
test tuples_and_tuple_patterns_type_check ... ok
test an_enum_without_repr_is_a_value_but_not_a_carried_type ... ok
test two_sites_giving_a_field_two_composite_types_is_e0605 ... ok
test the_rows_of_h5_to_h7_type_check ... ok
test generic_arguments_on_a_plain_type_are_refused ... ok
test phase_c_and_d_forms_stay_p0001_by_name ... ok
test composite_params_and_record_fields_type_check ... ok
test a_carried_type_without_repr_is_e0112 ... ok
test composite_carriers_type_check ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.31s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_aggregates.rs (/var/tmp/piperine-target-c/debug/deps/types_aggregates-9f78ce2b51ba7208)
```

`Run srcagg` - exit 0

```
test normal_with_zero_sigma_returns_the_mean ... ok
test two_sites_giving_a_field_different_composite_types_are_refused ... ok
test derived_repr_layout_and_width_follow_section_2 ... ok
test payload_variants_build_and_match ... ok
test struct_expressions_give_rusts_values ... ok
test tuple_patterns_bind_each_component ... ok
test tuples_give_rusts_values ... ok
test the_zi_nd_reaction_value_code_runs_as_a_function ... ok
test options_of_aggregates_behave_as_enums ... ok
test the_jitter_clock_repeats_with_its_seed ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.04s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_aggregates.rs (/var/tmp/piperine-target-c/debug/deps/source_aggregates-c98694b9c9ded7d7)
```

`Run plancomp` - exit 0

```
running 5 tests
test a_constant_is_the_bytes_the_layout_names ... ok
test a_one_variant_enum_lays_out_its_payload_only ... ok
test composite_layouts_list_their_leaves ... ok
test two_pins_of_a_composite_net_have_equal_layouts ... ok
test an_empty_struct_is_carried_with_zero_levels ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/composite_shape.rs (/var/tmp/piperine-target-c/debug/deps/composite_shape-e5279a28cf15b2eb)
```

`Run srcvs` - exit 0

```
running 9 tests
test a_string_is_passed_and_compared_by_value ... ok
test an_enum_without_copy_is_carried_and_compared ... ok
test the_reporting_functions_emit_the_same_records ... ok
test a_value_is_read_twice ... ok
test a_callee_cannot_change_the_callers_local ... ok
test an_assigned_loop_variable_does_not_change_the_loop ... ok
test the_library_items_that_took_borrows_give_the_same_results ... ok
test a_closure_keeps_the_value_it_captured ... ok
test a_let_without_mut_runs_in_a_function_a_reaction_and_analog ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.93s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_value_semantics.rs (/var/tmp/piperine-target-c/debug/deps/source_value_semantics-a54b1ba10ea014c3)
```

`Run elabagg` - exit 0

```
running 7 tests
test struct_fields_are_evaluated_in_written_order ... ok
test every_reached_struct_and_enum_is_one_arena_type ... ok
test aggregates_fold_in_const_contexts ... ok
test a_folded_composite_becomes_an_aggregate_constant ... ok
test a_composite_const_read_in_value_code_is_written_out ... ok
test the_stdlib_polynomial_helpers_fold ... ok
test an_index_out_of_range_in_a_const_is_e0104 ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_aggregates.rs (/var/tmp/piperine-target-c/debug/deps/elab_aggregates-0857b82158cd1204)
```

`Run typesvs` - exit 0

```
test a_range_index_is_an_array_of_the_folded_length ... ok
test clone_as_a_method_is_e0161 ... ok
test a_pattern_bound_name_is_a_place ... ok
test an_array_rest_pattern_binds_an_array ... ok
test a_let_without_mut_is_assigned_with_every_operator ... ok
test a_range_index_out_of_the_array_is_refused ... ok
test clone_and_copy_are_e0161_with_a_help ... ok
test a_deferred_let_is_assigned_twice_and_never_read_first ... ok
test str_is_a_value_type_in_every_position ... ok
test a_parameter_a_capture_and_a_body_let_are_not_places ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_value_semantics.rs (/var/tmp/piperine-target-c/debug/deps/types_value_semantics-711cf21a8dcd52a9)
```

`Run tagg` - exit 0

```
running 4 tests
test each_source_type_prints_as_one_rust_item_naming_its_path ... ok
test aggregate_expressions_compile_and_run ... ok
test narrow_payloads_pad_with_false_levels ... ok
test every_composite_value_crosses_the_abi_unchanged ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/aggregates.rs (/var/tmp/piperine-target-c/debug/deps/aggregates-9d043d0f237bb6a6)
```

`Run apicomp` - exit 0

```
running 4 tests
test a_carried_enum_reads_as_its_variant_name ... ok
test a_timed_write_of_a_leaf_is_refused_by_name ... ok
test enum_names_live_in_the_ir_not_in_the_net_bytes ... ok
test a_host_sets_a_composite_param_by_leaf_and_checks_its_where ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.45s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/composites.rs (/var/tmp/piperine-target-c/debug/deps/composites-4815f3c46262b153)
```

`Run iragg` - exit 0

```
test aggregate_forms_the_derives_do_not_allow_are_refused ... ok
test a_type_that_contains_itself_is_refused ... ok
test ill_typed_aggregate_expressions_are_refused ... ok
test the_type_arena_builds_checks_and_prints ... ok
test a_malformed_aggregate_type_is_refused ... ok
test variant_tests_option_packs_and_comparisons_check ... ok
test an_enum_tag_holds_its_discriminants ... ok
test ir_check_types_every_aggregate_expression ... ok
test the_ir_holds_no_slice ... ok
test composite_constants_check_print_and_round_trip ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/aggregates.rs (/var/tmp/piperine-target-c/debug/deps/aggregates-787016fdf83073e7)
```

`Run clirun` - exit 0

```
test the_project_flag_uses_that_directory_and_never_searches ... ok
test a_first_compile_is_announced_on_stderr ... ok
test a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot ... ok
test records_come_before_the_table ... ok
test run_of_a_top_with_a_node_port_is_e0810 ... ok
test records_stay_when_the_analysis_fails ... ok
test run_prints_header_records_blank_line_and_table ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.80s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target-c/debug/deps/run-3deb22af66054c5e)
```

`Run interp` - exit 0

```
running 9 tests
test a_host_source_body_is_rejected_by_name ... ok
test a_settle_event_is_refused_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test the_unknown_nodes_are_refused_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/frozen.rs (/var/tmp/piperine-target-c/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run twins` - exit 0

```
running 6 tests
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.20s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-c/debug/deps/twins-9b01bd7200d69219)
```

`Run docs` - exit 0

```
test state_records_the_unknown_semantics_decision ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test state_records_the_derived_param_decision ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test gaps_no_longer_lists_x5_l20_l39_h12 ... ok
test the_roadmap_is_forward_looking ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok
test abi_md_describes_the_unreleased_contract ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-c/debug/deps/docs-93bbd657b5852db6)
```

`Run galleries` - exit 0

```
test the_fixture_table_names_only_used_models ... ok
test galleries_fail_against_an_empty_prelude ... ok
test every_gallery_fence_is_in_the_formatters_form ... ok
test the_fences_composite_values_lifted_are_checked ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test every_gallery_references_its_std_items ... ok
test no_gallery_has_an_unresolved_member ... ok
test every_gallery_builds_with_no_diagnostics ... ok
test the_gate_builds_16_fences ... ok
test every_gallery_passes_check ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.70s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/std_galleries.rs (/var/tmp/piperine-target-c/debug/deps/std_galleries-20eb711d5dacf84a)
```

`Run irctl` - exit 0

```
test an_assignment_to_a_mutable_local_checks_and_prints ... ok
test a_loop_reaching_a_probe_on_a_later_iteration_reads_it ... ok
test an_assignment_to_an_immutable_local_is_refused ... ok
test a_function_value_is_called_through_its_reference ... ok
test an_assignment_joins_what_it_reads_into_the_local ... ok
test break_outside_a_loop_is_refused ... ok
test a_return_checks_against_the_result_type ... ok
test loops_check_type_and_print ... ok
test recursion_reached_from_analog_is_refused ... ok
test recursion_is_accepted_outside_analog ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/control.rs (/var/tmp/piperine-target-c/debug/deps/control-08c5e397081baf79)
```

`Run booktypes` - exit 0

```
test a_table_const_runs_where_a_top_reaches_it ... ok
test an_enum_without_repr_runs_as_a_value ... ok
test optional_arguments_may_be_left_out_or_named ... ok
test a_match_that_misses_a_variant_stops_the_build ... ok
test the_forms_still_on_the_way_are_refused_where_the_solver_reaches_them ... ok
test e0161_copying_is_not_a_trait ... ok
test an_enum_is_matched_compared_and_printed_by_the_name_of_its_variant ... ok
test tuples_and_structs_run_where_a_top_reaches_them ... ok
test an_option_is_read_with_its_methods_a_match_and_an_if_let ... ok
test an_impl_gives_a_type_its_constants_and_functions ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.92s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/types.rs (/var/tmp/piperine-target-c/debug/deps/book_types-48876d7b796ff112)
```

`cargo build -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piper` - exit 0

```
test no_msl_file_uses_a_removed_form ... ok
test root_targets_match_the_naming_rule ... ok
test every_integration_target_declares_its_scope ... ok
test the_walk_covers_the_three_solver_crates ... ok
test every_ignored_doc_example_is_registered ... ok
test no_file_scope_lint_suppression ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_dead_architecture_identifiers ... ok
test no_ignored_tests ... ok
test no_disabled_test_code ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-c/debug/deps/suite_hygiene-354b8333c6c0effe)
```
