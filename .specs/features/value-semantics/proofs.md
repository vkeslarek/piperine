# value-semantics proofs

**HEAD**: 3dcd2f9a
**Tree**: clean
**Shell**: /bin/sh
**Result**: 41 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run errors / test every_reference_form_is_e0101_once ... ok` | pass |
| C2 | `run errors / test a_lifetime_is_e0905_static_included ... ok` | pass |
| C3 | `run errors / test a_slice_type_does_not_parse ... ok` | pass |
| C4 | `run errors / test binary_ampersand_and_star_still_parse ... ok` | pass |
| C5 | `run errors / test the_markers_are_reserved_words_with_a_help_each ... ok` | pass |
| C6 | `run errors / test let_mut_in_a_module_body_is_e0903 ... ok` | pass |
| C7 | `run errors / test e0102_and_e0331_are_raised_nowhere ... ok` | pass |
| C8 | `run srcvs / test a_let_without_mut_runs_in_a_function_a_reaction_and_analog ... ok` | pass |
| C9 | `run typesvs / test a_parameter_a_capture_and_a_body_let_are_not_places ... ok` | pass |
| C10 | `run srcvs / test an_assigned_loop_variable_does_not_change_the_loop ... ok` | pass |
| C11 | `run typesvs / test a_deferred_let_is_assigned_twice_and_never_read_first ... ok` | pass |
| C12 | `run srcvs / test a_closure_keeps_the_value_it_captured ... ok` | pass |
| C13 | `run srcvs / test a_callee_cannot_change_the_callers_local ... ok` | pass |
| C14 | `run srcvs / test a_value_is_read_twice ... ok` | pass |
| C15 | `run typesvs / test clone_and_copy_are_e0161_with_a_help ... ok` | pass |
| C16 | `run srcvs / test an_enum_without_copy_is_carried_and_compared ... ok` | pass |
| C17 | `run typesvs / test clone_as_a_method_is_e0161 ... ok` | pass |
| C18 | `run typesvs / test a_range_index_is_an_array_of_the_folded_length ... ok` | pass |
| C19 | `run typesvs / test a_range_bound_that_is_not_const_is_e0188 ... ok` | pass |
| C20 | `run typesvs / test a_range_index_out_of_the_array_is_refused ... ok` | pass |
| C21 | `run typesvs / test an_array_rest_pattern_binds_an_array ... ok` | pass |
| C22 | `run typesvs / test an_array_is_passed_as_itself ... ok` | pass |
| C23 | `run typesvs / test str_is_a_value_type_in_every_position ... ok` | pass |
| C24 | `run srcvs / test the_reporting_functions_emit_the_same_records ... ok` | pass |
| C25 | `run stdlib / test std_holds_no_removed_form ... ok` | pass |
| C26 | `run excerpts / test every_part_vii_fence_is_in_its_file ... ok` | pass |
| C27 | `run spectext / test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok` | pass |
| C28 | `run srcvs / test the_library_items_that_took_borrows_give_the_same_results ... ok` | pass |
| C29 | `run hygiene / test no_msl_file_uses_a_removed_form ... ok` | pass |
| C30 | `run buckcli / test startup_prints_the_whole_start_on_its_grid ... ok` | pass |
| C31 | `run print / test the_printer_and_the_object_model_hold_no_removed_form ... ok` | pass |
| C32 | `run resolve / test builtin_table_matches_appendix_h ... ok` | pass |
| C33 | `run docs / test gaps_drops_the_value_semantics_gap ... ok` | pass |
| C34 | `run booklang / test e0903_mut_is_not_a_word_of_the_language ... ok` | pass |
| C35 | `run booktypes / test e0101_nothing_is_lent ... ok` | pass |
| C36 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C37 | `run goldens / test the_five_m1_bodies_print_as_the_golden_files ... ok` | pass |
| C38 | `run iragg / test the_ir_holds_no_slice ... ok` | pass |
| C39 | `run iragg / test a_local_is_assignable_exactly_when_its_body_assigns_it ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run errors` - exit 0

```
test unexpected_token_names_found_and_expected ... ok
test removed_rust_forms_report_targeted_messages ... ok
test parse_phase_rows_report_their_codes ... ok
test nesting_without_delimiters_is_also_bounded ... ok
test depth_over_256_is_diagnostic_not_overflow ... ok
test depth_limit_fits_8mib_stack_in_debug ... ok
test flat_chain_past_4096_links_is_e0910 ... ok
test comparison_lists_are_not_nesting ... ok
test flat_chains_up_to_4096_links_are_not_nesting ... ok
test flat_chains_survive_a_default_2_mib_thread ... ok

test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.07s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/msl_errors.rs (/var/tmp/piperine-target-c/debug/deps/msl_errors-7ad5845525a83174)
```

`Run srcvs` - exit 0

```
running 9 tests
test a_closure_keeps_the_value_it_captured ... ok
test a_callee_cannot_change_the_callers_local ... ok
test an_enum_without_copy_is_carried_and_compared ... ok
test the_library_items_that_took_borrows_give_the_same_results ... ok
test a_value_is_read_twice ... ok
test the_reporting_functions_emit_the_same_records ... ok
test an_assigned_loop_variable_does_not_change_the_loop ... ok
test a_string_is_passed_and_compared_by_value ... ok
test a_let_without_mut_runs_in_a_function_a_reaction_and_analog ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.10s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_value_semantics.rs (/var/tmp/piperine-target-c/debug/deps/source_value_semantics-a54b1ba10ea014c3)
```

`Run typesvs` - exit 0

```
test a_range_index_out_of_the_array_is_refused ... ok
test an_array_rest_pattern_binds_an_array ... ok
test a_range_index_is_an_array_of_the_folded_length ... ok
test a_pattern_bound_name_is_a_place ... ok
test clone_and_copy_are_e0161_with_a_help ... ok
test a_deferred_let_is_assigned_twice_and_never_read_first ... ok
test an_array_is_passed_as_itself ... ok
test a_let_without_mut_is_assigned_with_every_operator ... ok
test str_is_a_value_type_in_every_position ... ok
test a_parameter_a_capture_and_a_body_let_are_not_places ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_value_semantics.rs (/var/tmp/piperine-target-c/debug/deps/types_value_semantics-711cf21a8dcd52a9)
```

`Run stdlib` - exit 0

```
test root_forms_place_their_coefficient_form ... ok
test std_declares_no_core_name ... ok
test pure_bodies_name_no_primitive ... ok
test the_index_lists_stay_honest ... ok
test tgate_places_two_tran ... ok
test std_has_no_unresolved_member ... ok
test std_root_comment_names_the_namespaces ... ok
test std_root_declares_nine_namespaces ... ok
test transient_is_the_kind_and_tran_the_module ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.85s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-composite-values/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.73s
     Running tests/std_library.rs (/var/tmp/piperine-target-c/debug/deps/std_library-91744a3bb3abad26)
```

`Run excerpts` - exit 0

```
test the_forward_guard_covers_part_vii ... ok
test a_missing_item_is_named ... ok
test the_reverse_guard_counts_only_valid_fences_by_whole_line ... ok
test every_part_vii_fence_is_in_its_file ... ok
test prelude_items_outside_part_vii_are_shown_pub ... ok
test the_reverse_guard_covers_std ... ok
test every_std_behavior_block_is_in_a_fence ... ok
test every_std_item_is_in_a_fence ... ok
test the_ahead_lists_hold_only_what_still_differs ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-composite-values/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.58s
     Running tests/std_excerpts.rs (/var/tmp/piperine-target-c/debug/deps/std_excerpts-cbdd03ac9b332509)
```

`Run spectext` - exit 0

```
test the_commit_of_an_event_read_as_a_bool_is_stated ... ok
test tran_in_fences_is_the_module ... ok
test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok
test zero_bytes_are_the_default_value ... ok
test the_library_declares_no_net_nature ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok

test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_text.rs (/var/tmp/piperine-target-c/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run hygiene` - exit 0

```
test the_walk_covers_the_three_solver_crates ... ok
test every_integration_target_declares_its_scope ... ok
test root_targets_match_the_naming_rule ... ok
test mod_rs_declares_only ... ok
test every_ignored_doc_example_is_registered ... ok
test no_file_scope_lint_suppression ... ok
test no_ignored_tests ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_dead_architecture_identifiers ... ok
test no_disabled_test_code ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-c/debug/deps/suite_hygiene-354b8333c6c0effe)
```

`Run buckcli` - exit 0

```
test startup_prints_no_failed_check has been running for over 60 seconds
test startup_prints_the_whole_start_on_its_grid has been running for over 60 seconds
test the_buck_runs_print_at_most_16_lines has been running for over 60 seconds
test the_buck_runs_print_at_most_16_lines ... ok
test startup_ends_within_3_mv_of_0_8_volts ... ok
test startup_ends_with_the_reference_at_128 ... ok
test startup_prints_the_whole_start_on_its_grid ... ok
test piperine_test_passes_on_the_buck ... ok
test startup_prints_no_failed_check ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 182.77s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-composite-values/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 0.64s
     Running tests/buck.rs (/var/tmp/piperine-target-c/debug/deps/buck-e0a5ced46b7d0d71)
```

`Run print` - exit 0

```
test statement_and_hardware_docs_round_trip ... ok
test field_shorthand_prints_as_written ... ok
test raw_strings_print_as_written ... ok
test string_literals_are_printed_verbatim ... ok
test the_printer_and_the_object_model_hold_no_removed_form ... ok
test printer_preserves_docs_and_attrs ... ok
test roundtrip_ast_equal_over_corpus ... ok
test print_is_idempotent_over_corpus ... ok
test long_chains_print_without_overflow ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.11s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-composite-values/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.47s
     Running tests/msl_print.rs (/var/tmp/piperine-target-c/debug/deps/msl_print-59890e37e76e2ff2)
```

`Run resolve` - exit 0

```
test use_in_a_block_resolves_with_block_scope ... ok
test unresolved_path_is_one_e0161 ... ok
test use_self_with_alias_and_nesting ... ok
test type_paths_resolve_to_declarations ... ok
test a_model_constant_hides_the_prelude_constant ... ok
test embedded_std_declares_no_core_name ... ok
test k_b_resolves_to_std_constants ... ok
test embedded_std_builds_clean ... ok
test prelude_source_items_are_visible_everywhere ... ok

test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-composite-values/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.90s
     Running tests/pom_resolve.rs (/var/tmp/piperine-target-c/debug/deps/pom_resolve-c485c5a83a39f901)
```

`Run docs` - exit 0

```
test the_buck_readme_says_how_to_run_it ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test gaps_no_longer_lists_x5_l20_l39_h12 ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_roadmap_is_forward_looking ... ok
test state_holds_the_explicit_unknowns_decision ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok
test abi_md_describes_the_unreleased_contract ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-c/debug/deps/docs-93bbd657b5852db6)
```

`Run booklang` - exit 0

```
test nothing_converts_by_itself_and_a_condition_is_a_bool ... ok
test the_prelude_gives_its_constants ... ok
test the_sum_below_listing_runs_with_the_literal_typed_by_its_use ... ok
test e0909_e0905_and_e0903_a_literal_too_large_a_character_and_a_keyword_as_a_name ... ok
test e0903_mut_is_not_a_word_of_the_language ... ok
test the_literal_forms_have_the_values_their_comments_give ... ok
test blocks_ifs_and_matches_are_expressions_with_one_value ... ok
test a_fault_ends_the_analysis_or_the_build_and_a_non_finite_value_ends_the_solve ... ok
test constants_and_functions_compute_before_and_during_the_run ... ok
test arithmetic_keeps_the_low_bits_and_as_converts_without_failing ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.45s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/language.rs (/var/tmp/piperine-target-c/debug/deps/book_language-f568cd1da330e959)
```

`Run booktypes` - exit 0

```
test an_enum_without_repr_runs_as_a_value ... ok
test a_table_const_runs_where_a_top_reaches_it ... ok
test e0161_copying_is_not_a_trait ... ok
test the_forms_still_on_the_way_are_refused_where_the_solver_reaches_them ... ok
test optional_arguments_may_be_left_out_or_named ... ok
test a_match_that_misses_a_variant_stops_the_build ... ok
test an_enum_is_matched_compared_and_printed_by_the_name_of_its_variant ... ok
test tuples_and_structs_run_where_a_top_reaches_them ... ok
test an_option_is_read_with_its_methods_a_match_and_an_if_let ... ok
test an_impl_gives_a_type_its_constants_and_functions ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.07s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/types.rs (/var/tmp/piperine-target-c/debug/deps/book_types-48876d7b796ff112)
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

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.16s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-c/debug/deps/twins-9b01bd7200d69219)
```

`Run goldens` - exit 0

```
running 7 tests
test a_keyword_identifier_is_raw_and_the_abi_name_is_not ... ok
test a_body_without_analog_prints_no_terms_function ... ok
test the_five_m1_bodies_print_as_the_golden_files ... ok
test every_item_sits_under_its_path_and_origin ... ok
test each_function_is_a_generic_fn_and_each_body_a_device_mod ... ok
test transpiling_twice_gives_the_same_bytes ... ok
test a_design_with_nothing_to_run_has_no_module ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-composite-values/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 0.61s
     Running tests/golden.rs (/var/tmp/piperine-target-c/debug/deps/golden-ab85f3ee5d658ee1)
```

`Run iragg` - exit 0

```
test derives_imply_what_h8_says ... ok
test aggregate_forms_the_derives_do_not_allow_are_refused ... ok
test a_malformed_aggregate_type_is_refused ... ok
test ill_typed_aggregate_expressions_are_refused ... ok
test an_enum_tag_holds_its_discriminants ... ok
test the_type_arena_builds_checks_and_prints ... ok
test composite_constants_check_print_and_round_trip ... ok
test the_ir_holds_no_slice ... ok
test ir_check_types_every_aggregate_expression ... ok
test variant_tests_option_packs_and_comparisons_check ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/aggregates.rs (/var/tmp/piperine-target-c/debug/deps/aggregates-787016fdf83073e7)
```
