# msl-polish proofs

**HEAD**: 5230a28a
**Tree**: clean
**Shell**: /bin/sh
**Result**: 137 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run spec / test b36_lists_every_removed_form ... ok` | pass |
| C2 | `run spec / test b36_names_the_forms_taken_in ... ok` | pass |
| C3 | `run spec / test b36_rows_carry_a_code_and_a_replacement ... ok` | pass |
| C4 | `run spec / test appendix_h_has_every_group ... ok` | pass |
| C5 | `run spec / test every_method_called_in_a_spec_fence_is_catalogued ... ok` | pass |
| C6 | `run spec / test catalogue_is_closed_by_e0161 ... ok` | pass |
| C7 | `run spec / test appendix_h_is_in_the_file_map_and_nav ... ok` | pass |
| C8 | `run resolve / test catalogue_traits_and_types_resolve ... ok` | pass |
| C9 | `run resolve / test builtin_table_matches_appendix_h ... ok` | pass |
| C10 | `run spec / test gnd_and_the_implicit_params_are_core_names ... ok` | pass |
| C11 | `run resolve / test str_is_a_builtin_type ... ok` | pass |
| C12 | `run classify / test nature_bound_is_builtin_and_classifies_the_port ... ok` | pass |
| C13 | `run resolve / test gnd_is_a_builtin_node ... ok` | pass |
| C14 | `run resolve / test m_and_dtemp_resolve_inside_a_module ... ok` | pass |
| C15 | `run resolve / test m_and_dtemp_are_unknown_outside_a_module ... ok` | pass |
| C16 | `run resolve / test a_local_shadows_an_implicit_param ... ok` | pass |
| C17 | `run resolve / test core_names_cannot_be_redeclared ... ok` | pass |
| C18 | `run resolve / test fixture_prelude_declares_no_core_name ... ok` | pass |
| C19 | `run spec / test e0162_is_defined_in_3_4 ... ok` | pass |
| C20 | `run spec / test glob_shadowing_and_e0143_are_stated ... ok` | pass |
| C21 | `run resolve / test duplicate_item_is_e0162_and_both_are_kept ... ok` | pass |
| C22 | `run resolve / test import_colliding_with_an_item_is_e0162 ... ok` | pass |
| C23 | `run resolve / test ambiguous_glob_is_e0162_only_when_used ... ok` | pass |
| C24 | `run resolve / test two_globs_of_one_declaration_are_clean ... ok` | pass |
| C25 | `run resolve / test two_functions_of_one_name_stay_e0143 ... ok` | pass |
| C26 | `run resolve / test one_name_in_two_tables_is_clean ... ok` | pass |
| C27 | `run spec / test behavior_generics_are_repeated_not_positional ... ok` | pass |
| C28 | `run spec / test items_sit_at_namespace_scope ... ok` | pass |
| C29 | `run classify / test mismatched_behavior_names_bind_by_name ... ok` | pass |
| C30 | `run classify / test behavior_binds_across_namespaces_of_one_file ... ok` | pass |
| C31 | `run spec / test shared_borrow_of_any_argument_is_allowed ... ok` | pass |
| C32 | `run spec / test identifier_pattern_rule_is_stated ... ok` | pass |
| C33 | `run resolve / test identifier_pattern_naming_none_is_a_path ... ok` | pass |
| C34 | `run resolve / test identifier_pattern_naming_a_const_variant_or_unit_struct_is_a_path ... ok` | pass |
| C35 | `run resolve / test other_identifier_patterns_stay_bindings ... ok` | pass |
| C36 | `run spec / test e0404_and_e0121_state_their_precedence ... ok` | pass |
| C37 | `run spec / test value_blocks_may_hold_contributions_and_commits ... ok` | pass |
| C38 | `run spec / test place_has_no_parentheses ... ok` | pass |
| C39 | `run spec / test attrs_are_allowed_before_statements_and_hardware_items ... ok` | pass |
| C40 | `run spec / test quantity_and_reaction_docs_are_metadata ... ok` | pass |
| C41 | `run spec / test post_parse_constraints_name_their_phase ... ok` | pass |
| C42 | `run spec / test e0125_covers_bodies ... ok` | pass |
| C43 | `run spec / test raw_identifiers_are_specified_and_g15_is_closed ... ok` | pass |
| C44 | `run spec / test field_shorthand_is_specified_and_g14_is_closed ... ok` | pass |
| C45 | `run spec / test every_error_code_is_defined_exactly_once ... ok` | pass |
| C46 | `run spec / test b1_still_counts_65_constructs ... ok` | pass |
| C47 | `run spec / test appendix_g_records_the_decisions ... ok` | pass |
| C48 | `run parser / test negative_literal_is_a_const_generic_argument ... ok` | pass |
| C49 | `run parser / test commit_token_in_generic_arguments_is_still_an_error ... ok` | pass |
| C50 | `run parser / test function_pointer_types_parse ... ok` | pass |
| C51 | `run body / test function_pointer_type_round_trips ... ok` | pass |
| C52 | `run errors / test removed_rust_forms_report_targeted_messages ... ok` | pass |
| C53 | `run errors / test every_b36_row_has_a_fixture ... ok` | pass |
| C54 | `run errors / test body_let_annotation_has_its_own_message ... ok` | pass |
| C55 | `run lexer / test raw_identifier_of_an_added_keyword_is_an_identifier ... ok` | pass |
| C56 | `run errors / test raw_identifier_of_another_word_is_e0900 ... ok` | pass |
| C57 | `run print / test raw_identifiers_print_with_their_prefix ... ok` | pass |
| C58 | `run resolve / test raw_identifier_names_drop_the_prefix ... ok` | pass |
| C59 | `run parser / test param_block_and_template_shorthand_keep_their_form ... ok` | pass |
| C60 | `run print / test field_shorthand_prints_as_written ... ok` | pass |
| C61 | `run classify / test shorthand_field_binds_param_and_reads_the_local ... ok` | pass |
| C62 | `run parser / test docs_and_attrs_attach_to_statements ... ok` | pass |
| C63 | `run errors / test dangling_doc_comment_has_its_own_message ... ok` | pass |
| C64 | `run parser / test docs_and_attrs_attach_to_hardware_items ... ok` | pass |
| C65 | `run print / test statement_and_hardware_docs_round_trip ... ok` | pass |
| C66 | `run pomcorpus / test docs_at_every_position_round_trip ... ok` | pass |
| C67 | `run tree / test docs_reach_reactions_quantities_definitions_and_statements ... ok` | pass |
| C68 | `run spec / test e0909_is_defined_for_out_of_range_literals ... ok` | pass |
| C69 | `run lexer / test float_literal_out_of_range_is_e0909 ... ok` | pass |
| C70 | `run lexer / test integer_literal_out_of_range_is_e0909 ... ok` | pass |
| C71 | `run errors / test every_parse_phase_row_has_a_fixture ... ok` | pass |
| C72 | `run lexer / test invalid_string_escape_is_e0900 ... ok` | pass |
| C73 | `run print / test raw_strings_print_as_written ... ok` | pass |
| C74 | `run lexer / test literal_accessors_return_cooked_values ... ok` | pass |
| C75 | `run reflect / test float_literals_round_trip_through_json ... ok` | pass |
| C76 | `run reflect / test float_literal_is_serialized_from_its_text ... ok` | pass |
| C77 | `run errors / test commit_target_must_be_a_place ... ok` | pass |
| C78 | `run errors / test parenthesized_place_is_rejected_in_both_forms ... ok` | pass |
| C79 | `run errors / test signal_outside_a_module_body_is_e0404 ... ok` | pass |
| C80 | `run errors / test behavior_statements_in_a_module_body_are_e0404 ... ok` | pass |
| C81 | `run errors / test function_body_reports_e0121_only ... ok` | pass |
| C82 | `run errors / test pub_on_a_behavior_block_is_e0900 ... ok` | pass |
| C83 | `run errors / test forms_outside_the_grammar_are_parse_errors ... ok` | pass |
| C84 | `run resolve / test misplaced_signal_declares_nothing ... ok` | pass |
| C85 | `run spec / test e0910_counts_nesting_and_bounds_chains ... ok` | pass |
| C86 | `run errors / test flat_chains_up_to_4096_links_are_not_nesting ... ok` | pass |
| C87 | `run errors / test flat_chain_past_4096_links_is_e0910 ... ok` | pass |
| C88 | `run errors / test nesting_without_delimiters_is_also_bounded ... ok` | pass |
| C89 | `run errors / test bad_where_list_is_one_diagnostic ... ok` | pass |
| C90 | `run errors / test statement_errors_recover_inside_the_block ... ok` | pass |
| C91 | `run errors / test a_bad_port_keeps_the_module ... ok` | pass |
| C92 | `run resolve / test module_with_a_bad_port_still_resolves ... ok` | pass |
| C93 | `run resolve / test use_self_imports_the_namespace ... ok` | pass |
| C94 | `run resolve / test use_self_with_alias_and_nesting ... ok` | pass |
| C95 | `run resolve / test use_leaf_text_is_its_own_segments ... ok` | pass |
| C96 | `run resolve / test repr_members_resolve_through_a_bound ... ok` | pass |
| C97 | `run resolve / test unknown_repr_member_is_e0161 ... ok` | pass |
| C98 | `run resolve / test member_through_a_builtin_bound_is_deferred ... ok` | pass |
| C99 | `run classify / test param_block_errors_are_e0320 ... ok` | pass |
| C100 | `run classify / test named_argument_errors_are_e0140 ... ok` | pass |
| C101 | `run classify / test bundle_literal_field_errors_are_e0263 ... ok` | pass |
| C102 | `run classify / test unknown_struct_field_is_reported_in_literals_and_patterns ... ok` | pass |
| C103 | `run classify / test extern_module_shape_is_e0370 ... ok` | pass |
| C104 | `run classify / test function_body_rule_is_e0125 ... ok` | pass |
| C105 | `run classify / test nature_field_errors_are_e0201 ... ok` | pass |
| C106 | `run classify / test access_of_another_nature_is_e0206 ... ok` | pass |
| C107 | `run classify / test carrier_method_on_an_array_is_e0211 ... ok` | pass |
| C108 | `run reflect / test parent_drive_is_a_writer_of_the_child_carrier ... ok` | pass |
| C109 | `run reflect / test by_name_lookups_prefer_project_files ... ok` | pass |
| C110 | `run reflect / test collections_list_project_files_only ... ok` | pass |
| C111 | `run reflect / test design_has_a_collection_and_lookup_per_item_kind ... ok` | pass |
| C112 | `run body / test expr_children_and_parent_are_inverse ... ok` | pass |
| C113 | `run body / test decl_exprs_is_preorder_over_the_body ... ok` | pass |
| C114 | `run body / test block_and_stmt_handles_walk_a_body ... ok` | pass |
| C115 | `run classify / test arg_and_field_handles_expose_name_value_target ... ok` | pass |
| C116 | `run classify / test builtin_instance_names_its_primitive ... ok` | pass |
| C117 | `run reflect / test types_list_their_impls_and_methods ... ok` | pass |
| C118 | `run tree / test generation_if_items_cover_every_arm ... ok` | pass |
| C119 | `run reflect / test from_json_reads_back_a_valid_design ... ok` | pass |
| C120 | `run reflect / test from_json_rejects_a_corrupted_design ... ok` | pass |
| C121 | `run design / test member_tables_scale_subquadratically ... ok` | pass |
| C122 | `run design / test name_lookups_do_not_scan ... ok` | pass |
| C123 | `run docs / test every_b1_construct_is_mapped ... ok` | pass |
| C124 | `run docs / test place_has_no_parenthesized_form_in_the_docs ... ok` | pass |
| C125 | `run docs / test every_code_the_pom_reports_is_documented ... ok` | pass |
| C126 | `run docs / test every_builtin_row_is_documented ... ok` | pass |
| C127 | `run docs / test every_reflection_method_is_documented ... ok` | pass |
| C128 | `run mslcorpus / test every_valid_spec_block_parses_clean ... ok` | pass |
| C129 | `run pomcorpus / test every_spec_block_round_trips_to_its_ast ... ok` | pass |
| C130 | `run errors / test comparison_lists_are_not_nesting ... ok` | pass |
| C131 | `run errors / test flat_chains_survive_a_default_2_mib_thread ... ok` | pass |
| C132 | `run reflect / test from_json_never_panics_on_a_corrupted_field ... ok` | pass |
| C133 | `run spec / test appendix_h_renders_every_code_span ... ok` | pass |
| C134 | `run errors / test reserved_keyword_help_is_true_and_a_doc_before_else_documents_nothing ... ok` | pass |
| C135 | `run resolve / test duplicate_items_in_a_block_are_e0162 ... ok` | pass |
| C136 | `run spec / test block_items_follow_the_namespace_rules ... ok` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^warning')` | pass |

## Output

`Run spec` - exit 0

```
test value_blocks_may_hold_contributions_and_commits ... ok
test quantity_and_reaction_docs_are_metadata ... ok
test raw_identifiers_are_specified_and_g15_is_closed ... ok
test post_parse_constraints_name_their_phase ... ok
test e0404_and_e0121_state_their_precedence ... ok
test attrs_are_allowed_before_statements_and_hardware_items ... ok
test every_method_called_in_a_spec_fence_is_catalogued ... ok
test gnd_and_the_implicit_params_are_core_names ... ok
test e0162_is_defined_in_3_4 ... ok
test every_error_code_is_defined_exactly_once ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/spec_text.rs (/var/tmp/piperine-target/debug/deps/spec_text-8e0716f5857daff9)
```

`Run resolve` - exit 0

```
test use_and_qualified_paths_resolve ... ok
test unresolved_path_is_one_e0161 ... ok
test use_leaf_text_is_its_own_segments ... ok
test type_paths_resolve_to_declarations ... ok
test use_self_imports_the_namespace ... ok
test use_self_with_alias_and_nesting ... ok
test use_in_a_block_resolves_with_block_scope ... ok
test fixture_prelude_declares_no_core_name ... ok
test prelude_source_items_are_visible_everywhere ... ok
test fixture_prelude_builds_clean ... ok

test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_resolve.rs (/var/tmp/piperine-target/debug/deps/pom_resolve-f44ddda9709d3f18)
```

`Run classify` - exit 0

```
test generic_carrier_follows_the_resolved_bound ... ok
test shorthand_field_binds_param_and_reads_the_local ... ok
test port_kinds ... ok
test param_block_errors_are_e0320 ... ok
test self_referential_struct_bundle_check_ends ... ok
test unresolved_callee_is_plain_call ... ok
test struct_expression_classes ... ok
test unknown_receiver_is_deferred_without_diagnostic ... ok
test struct_with_carrier_field_is_bundle ... ok
test unknown_struct_field_is_reported_in_literals_and_patterns ... ok

test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_classify.rs (/var/tmp/piperine-target/debug/deps/pom_classify-a28ce0a42d0b3e76)
```

`Run parser` - exit 0

```
test docs_and_attrs_attach_to_hardware_items ... ok
test doc_comments_and_attrs_attach ... ok
test function_pointer_types_parse ... ok
test contribution_commit_at_any_depth ... ok
test contextual_keywords_are_identifiers_outside_nature ... ok
test unknown_as_value_ok_as_name_e0908 ... ok
test inherited_rust_forms_parse_items ... ok
test inherited_rust_forms_parse ... ok
test inherited_rust_forms_parse_expressions ... ok
test every_b3_production_parses_with_span ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/msl_parser.rs (/var/tmp/piperine-target/debug/deps/msl_parser-5aac91ca3ae812b0)
```

`Run body` - exit 0

```
test function_pointer_type_round_trips ... ok
test statement_pattern_and_type_forms_lower ... ok
test error_nodes_are_kept ... ok
test decl_exprs_is_preorder_over_the_body ... ok
test node_text_is_authored_text_and_enclosing_is_nearest_decl ... ok
test expr_children_and_parent_are_inverse ... ok
test every_expression_form_has_its_kind ... ok
test fns_consts_metas_and_generics_round_trip ... ok
test depth_256_builds_on_default_stack ... ok
test value_bodies_round_trip ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_body.rs (/var/tmp/piperine-target/debug/deps/pom_body-0eda4aa62bb7a864)
```

`Run errors` - exit 0

```
test unexpected_token_names_found_and_expected ... ok
test parse_phase_rows_report_their_codes ... ok
test removed_rust_forms_report_targeted_messages ... ok
test nesting_without_delimiters_is_also_bounded ... ok
test depth_limit_fits_8mib_stack_in_debug ... ok
test depth_over_256_is_diagnostic_not_overflow ... ok
test flat_chain_past_4096_links_is_e0910 ... ok
test comparison_lists_are_not_nesting ... ok
test flat_chains_up_to_4096_links_are_not_nesting ... ok
test flat_chains_survive_a_default_2_mib_thread ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.15s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/msl_errors.rs (/var/tmp/piperine-target/debug/deps/msl_errors-f5f3f043c2e1f80b)
```

`Run lexer` - exit 0

```
test inherited_lexical_forms_lex ... ok
test float_literal_out_of_range_is_e0909 ... ok
test invalid_string_escape_is_e0900 ... ok
test keywords_reserved_vs_contextual ... ok
test malformed_si_literal_reports_e0902 ... ok
test non_utf8_one_diagnostic ... ok
test raw_identifier_of_an_added_keyword_is_an_identifier ... ok
test unterminated_comment_and_string_one_diagnostic ... ok
test si_literal_scaled_value_and_raw_text ... ok
test literal_accessors_return_cooked_values ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/msl_lexer.rs (/var/tmp/piperine-target/debug/deps/msl_lexer-1414a897fdea453d)
```

`Run print` - exit 0

```
test raw_identifiers_print_with_their_prefix ... ok
test inherited_lexical_forms_roundtrip ... ok
test statement_and_hardware_docs_round_trip ... ok
test raw_strings_print_as_written ... ok
test field_shorthand_prints_as_written ... ok
test string_literals_are_printed_verbatim ... ok
test printer_preserves_docs_and_attrs ... ok
test roundtrip_ast_equal_over_corpus ... ok
test print_is_idempotent_over_corpus ... ok
test long_chains_print_without_overflow ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/msl_print.rs (/var/tmp/piperine-target/debug/deps/msl_print-0cd0e3f235c88803)
```

`Run pomcorpus` - exit 0

```
running 6 tests
test a_loaded_file_rebuilds_to_its_own_tree ... ok
test docs_at_every_position_round_trip ... ok
test every_item_kind_round_trips_when_written_by_hand ... ok
test check_reports_every_kind_of_broken_link ... ok
test every_spec_block_builds_with_consistent_invariants ... ok
test every_spec_block_round_trips_to_its_ast ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.11s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_corpus.rs (/var/tmp/piperine-target/debug/deps/pom_corpus-2a1a1bbd546c609c)
```

`Run tree` - exit 0

```
test signal_is_declared_in_its_module_only ... ok
test nested_contribution_is_listed ... ok
test signal_pub_type_init ... ok
test impl_and_trait_parts ... ok
test reaction_in_digital_generation ... ok
test struct_fields_and_enum_variants ... ok
test use_tree_flattens_to_imports ... ok
test several_analog_blocks_in_order ... ok
test reactions_and_deep_commits ... ok
test root_children_mirror_items_in_source_order ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_tree.rs (/var/tmp/piperine-target/debug/deps/pom_tree-f9daa3229358c79f)
```

`Run reflect` - exit 0

```
test references_navigate_to_declarations ... ok
test references_in_source_order ... ok
test writers_are_definitions_and_commits ... ok
test sar_adc_instances_name_their_modules ... ok
test gallery_design_round_trips_through_json ... ok
test from_json_reads_back_a_valid_design ... ok
test json_is_deterministic ... ok
test gallery_signals_are_declared_in_their_module ... ok
test resolution_creates_no_node ... ok
test from_json_never_panics_on_a_corrupted_field ... ok

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.91s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_reflect.rs (/var/tmp/piperine-target/debug/deps/pom_reflect-ef5de612485d375e)
```

`Run design` - exit 0

```
test mod_loads_sibling_file_or_mod_file ... ok
test parse_diagnostics_pass_through_with_file ... ok
test unreadable_root_is_an_empty_root_with_e0161 ... ok
test two_builds_dump_identical ... ok
test prelude_files_are_numbered_first ... ok
test load_from_disk_equals_in_memory_build ... ok
test nested_mod_files_follow_rust_layout ... ok
test docs_keep_kind_and_file_across_mod_files ... ok
test member_tables_scale_subquadratically ... ok
test name_lookups_do_not_scan ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.96s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_design.rs (/var/tmp/piperine-target/debug/deps/pom_design-6114fd27de7dd9d0)
```

`Run docs` - exit 0

```
running 9 tests
test every_builtin_row_is_documented ... ok
test mkdocs_nav_lists_every_pom_page ... ok
test place_has_no_parenthesized_form_in_the_docs ... ok
test every_code_the_pom_reports_is_documented ... ok
test every_public_pom_type_has_a_heading ... ok
test every_b1_construct_is_mapped ... ok
test every_b3_production_is_mapped ... ok
test msl_fences_parse_clean ... ok
test every_reflection_method_is_documented ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_docs.rs (/var/tmp/piperine-target/debug/deps/pom_docs-031147b33902f8bc)
```

`Run mslcorpus` - exit 0

```
running 2 tests
test every_compile_fail_block_diagnoses ... ok
test every_valid_spec_block_parses_clean ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/msl_corpus.rs (/var/tmp/piperine-target/debug/deps/msl_corpus-ef5895bf68ec9ad8)
```
