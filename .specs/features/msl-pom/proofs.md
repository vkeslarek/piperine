# msl-pom proofs

**HEAD**: 5965ea85
**Tree**: clean
**Shell**: /bin/sh
**Result**: 104 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run design / test build_retains_name_text_and_ast ... ok` | pass |
| C2 | `run design / test parse_diagnostics_pass_through_with_file ... ok` | pass |
| C3 | `run design / test two_builds_dump_identical ... ok` | pass |
| C4 | `run design / test design_is_send_and_sync ... ok` | pass |
| C5 | `run design / test mod_loads_sibling_file_or_mod_file ... ok` | pass |
| C6 | `run design / test mod_without_file_is_empty_namespace_e0161 ... ok` | pass |
| C7 | `run tree / test root_children_mirror_items_in_source_order ... ok` | pass |
| C8 | `run tree / test signal_is_declared_in_its_module_only ... ok` | pass |
| C9 | `run tree / test generation_for_holds_its_signal_once ... ok` | pass |
| C10 | `run tree / test generation_if_chain_exposes_three_arms ... ok` | pass |
| C11 | `run tree / test inline_mod_is_a_namespace ... ok` | pass |
| C12 | `run tree / test item_in_fn_body_is_declared_in_the_fn ... ok` | pass |
| C13 | `run tree / test ports_in_order_with_direction_type_default ... ok` | pass |
| C14 | `run tree / test params_type_default_where_required ... ok` | pass |
| C15 | `run tree / test signal_pub_type_init ... ok` | pass |
| C16 | `run tree / test body_let_pattern_names_each_component ... ok` | pass |
| C17 | `run tree / test definition_place_value_after ... ok` | pass |
| C18 | `run tree / test extern_module_abi ... ok` | pass |
| C19 | `run tree / test module_result_and_result_type ... ok` | pass |
| C20 | `run tree / test module_generics_and_where ... ok` | pass |
| C21 | `run tree / test nature_quantities_and_kind ... ok` | pass |
| C22 | `run tree / test struct_fields_and_enum_variants ... ok` | pass |
| C23 | `run tree / test fn_abi_params_ret_body ... ok` | pass |
| C24 | `run tree / test impl_and_trait_parts ... ok` | pass |
| C25 | `run tree / test use_tree_flattens_to_imports ... ok` | pass |
| C26 | `run tree / test const_type_and_value ... ok` | pass |
| C27 | `run tree / test analog_block_is_file_item_linked_to_module ... ok` | pass |
| C28 | `run tree / test nested_contribution_is_listed ... ok` | pass |
| C29 | `run tree / test reactions_and_deep_commits ... ok` | pass |
| C30 | `run tree / test reaction_in_digital_generation ... ok` | pass |
| C31 | `run tree / test behavior_generic_names ... ok` | pass |
| C32 | `run corpus / test every_spec_block_round_trips_to_its_ast ... ok` | pass |
| C33 | `run body / test node_text_is_authored_text_and_enclosing_is_nearest_decl ... ok` | pass |
| C34 | `run body / test every_expression_form_has_its_kind ... ok` | pass |
| C35 | `run tree / test doc_comments_reach_docs ... ok` | pass |
| C36 | `run tree / test attribute_arguments_are_exposed ... ok` | pass |
| C37 | `run resolve / test block_reads_resolve_to_module_members ... ok` | pass |
| C38 | `run resolve / test body_names_are_order_independent ... ok` | pass |
| C39 | `run resolve / test shadowed_local_resolves_to_previous_binding ... ok` | pass |
| C40 | `run resolve / test binders_scope_their_names ... ok` | pass |
| C41 | `run resolve / test type_paths_resolve_to_declarations ... ok` | pass |
| C42 | `run resolve / test builtin_names_resolve_to_table_entries ... ok` | pass |
| C43 | `run resolve / test use_and_qualified_paths_resolve ... ok` | pass |
| C44 | `run resolve / test prelude_source_items_are_visible_everywhere ... ok` | pass |
| C45 | `run resolve / test unresolved_path_is_one_e0161 ... ok` | pass |
| C46 | `run resolve / test private_item_links_and_reports_e0161 ... ok` | pass |
| C47 | `run resolve / test associated_paths_resolve_to_variant_const_fn ... ok` | pass |
| C48 | `run resolve / test self_type_resolves ... ok` | pass |
| C49 | `run resolve / test core_name_redefined_is_e0457 ... ok` | pass |
| C50 | `run resolve / test duplicate_member_name_is_e0330 ... ok` | pass |
| C51 | `run resolve / test diagnostics_are_ordered ... ok` | pass |
| C52 | `run classify / test port_kinds ... ok` | pass |
| C53 | `run classify / test struct_with_carrier_field_is_bundle ... ok` | pass |
| C54 | `run classify / test named_instance_with_connections_and_params ... ok` | pass |
| C55 | `run classify / test anonymous_instance_in_analog_and_plain_call ... ok` | pass |
| C56 | `run classify / test struct_expression_classes ... ok` | pass |
| C57 | `run classify / test instance_through_template_value ... ok` | pass |
| C58 | `run classify / test behavior_without_module_is_e0401 ... ok` | pass |
| C59 | `run classify / test behavior_for_extern_module_is_e0402 ... ok` | pass |
| C60 | `run classify / test behavior_names_mismatch_is_e0403 ... ok` | pass |
| C61 | `run classify / test access_alias_resolves_by_first_argument ... ok` | pass |
| C62 | `run classify / test impl_on_module_is_e0130 ... ok` | pass |
| C63 | `run classify / test instance_pub_member_resolves ... ok` | pass |
| C64 | `run classify / test instance_private_member_is_e0343 ... ok` | pass |
| C65 | `run classify / test bundle_field_resolves ... ok` | pass |
| C66 | `run classify / test methods_resolve_to_impl_fn_or_builtin ... ok` | pass |
| C67 | `run classify / test unknown_receiver_is_deferred_without_diagnostic ... ok` | pass |
| C68 | `run reflect / test lookups_by_name_and_path ... ok` | pass |
| C69 | `run reflect / test qualified_names ... ok` | pass |
| C70 | `run reflect / test casts_match_kind_only ... ok` | pass |
| C71 | `run reflect / test carrier_nature_through_array_and_flip ... ok` | pass |
| C72 | `run reflect / test instance_connection_by_port ... ok` | pass |
| C73 | `run reflect / test handle_equality ... ok` | pass |
| C74 | `run reflect / test references_in_source_order ... ok` | pass |
| C75 | `run reflect / test writers_are_definitions_and_commits ... ok` | pass |
| C76 | `run reflect / test instantiated_by_lists_instances ... ok` | pass |
| C77 | `run classify / test generation_body_is_not_unrolled ... ok` | pass |
| C78 | `run classify / test self_placement_under_generation_if ... ok` | pass |
| C79 | `run reflect / test resolution_creates_no_node ... ok` | pass |
| C80 | `run docs / test every_public_pom_type_has_a_heading ... ok` | pass |
| C81 | `run docs / test every_reflection_method_is_documented ... ok` | pass |
| C82 | `run docs / test every_b3_production_is_mapped ... ok` | pass |
| C83 | `run docs / test msl_fences_parse_clean ... ok` | pass |
| C84 | `cargo test -p piperine-lang --doc pom::guide 2>&1 / tee /dev/stderr / grep -qE 'test result: ok\. [1-9][0-9]+ ` | pass |
| C85 | `run docs / test mkdocs_nav_lists_every_pom_page ... ok` | pass |
| C86 | `run corpus / test every_spec_block_builds_with_consistent_invariants ... ok` | pass |
| C87 | `run body / test error_nodes_are_kept ... ok` | pass |
| C88 | `run body / test depth_256_builds_on_default_stack ... ok` | pass |
| C89 | `run design / test mod_with_two_candidates_is_e0161 ... ok` | pass |
| C90 | `run design / test nested_mod_files_follow_rust_layout ... ok` | pass |
| C91 | `run design / test mod_cycle_is_e0161_and_not_reloaded ... ok` | pass |
| C92 | `cargo test -p piperine-lang --test pom_design 2>&1 / tee /dev/stderr / grep -cE 'test (load_from_disk_equals_i` | pass |
| C93 | `run reflect / test design_and_storage_types_implement_serde ... ok` | pass |
| C94 | `run reflect / test gallery_design_round_trips_through_json ... ok` | pass |
| C95 | `run reflect / test json_is_deterministic ... ok` | pass |
| C96 | `run design / test docs_keep_kind_and_file_across_mod_files ... ok` | pass |
| C97 | `run resolve / test use_in_a_block_resolves_with_block_scope ... ok` | pass |
| C98 | `run reflect / test references_navigate_to_declarations ... ok` | pass |
| C99 | `run classify / test generic_carrier_follows_the_resolved_bound ... ok` | pass |
| C100 | `run reflect / test qualified_names_are_unambiguous_and_findable ... ok` | pass |
| C101 | `run classify / test named_binding_without_a_target_is_e0161 ... ok` | pass |
| C102 | `run classify / test missing_instance_member_is_e0343 ... ok` | pass |
| C103 | `run resolve / test access_name_resolves_to_its_quantity ... ok` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^warning')` | pass |

## Output

`Run design` - exit 0

```
test build_retains_name_text_and_ast ... ok
test non_utf8_file_is_a_parse_diagnostic ... ok
test unreadable_root_is_an_empty_root_with_e0161 ... ok
test mod_loads_sibling_file_or_mod_file ... ok
test parse_diagnostics_pass_through_with_file ... ok
test prelude_files_are_numbered_first ... ok
test two_builds_dump_identical ... ok
test load_from_disk_equals_in_memory_build ... ok
test nested_mod_files_follow_rust_layout ... ok
test docs_keep_kind_and_file_across_mod_files ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_design.rs (/var/tmp/piperine-target/debug/deps/pom_design-22a732d12060c623)
```

`Run tree` - exit 0

```
test reactions_and_deep_commits ... ok
test use_tree_flattens_to_imports ... ok
test nested_contribution_is_listed ... ok
test signal_is_declared_in_its_module_only ... ok
test reaction_in_digital_generation ... ok
test several_analog_blocks_in_order ... ok
test nature_quantities_and_kind ... ok
test struct_fields_and_enum_variants ... ok
test root_children_mirror_items_in_source_order ... ok

test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-spec/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.37s
     Running tests/pom_tree.rs (/var/tmp/piperine-target/debug/deps/pom_tree-9b656571fbeeeb13)
```

`Run corpus` - exit 0

```
running 5 tests
test a_loaded_file_rebuilds_to_its_own_tree ... ok
test check_reports_every_kind_of_broken_link ... ok
test every_item_kind_round_trips_when_written_by_hand ... ok
test every_spec_block_builds_with_consistent_invariants ... ok
test every_spec_block_round_trips_to_its_ast ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.96s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-spec/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.47s
     Running tests/pom_corpus.rs (/var/tmp/piperine-target/debug/deps/pom_corpus-b0f4230ca68c7387)
```

`Run body` - exit 0

```
running 8 tests
test error_expressions_become_error_nodes ... ok
test node_text_is_authored_text_and_enclosing_is_nearest_decl ... ok
test error_nodes_are_kept ... ok
test statement_pattern_and_type_forms_lower ... ok
test fns_consts_metas_and_generics_round_trip ... ok
test every_expression_form_has_its_kind ... ok
test depth_256_builds_on_default_stack ... ok
test value_bodies_round_trip ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-spec/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.35s
     Running tests/pom_body.rs (/var/tmp/piperine-target/debug/deps/pom_body-de675aebd654bab4)
```

`Run resolve` - exit 0

```
test block_reads_resolve_to_module_members ... ok
test unresolved_path_is_one_e0161 ... ok
test type_paths_resolve_to_declarations ... ok
test self_type_resolves ... ok
test body_names_are_order_independent ... ok
test use_in_a_block_resolves_with_block_scope ... ok
test binders_scope_their_names ... ok
test prelude_source_items_are_visible_everywhere ... ok
test fixture_prelude_builds_clean ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-spec/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.35s
     Running tests/pom_resolve.rs (/var/tmp/piperine-target/debug/deps/pom_resolve-1d3a335ca31030c7)
```

`Run classify` - exit 0

```
test self_placement_under_generation_if ... ok
test self_referential_struct_bundle_check_ends ... ok
test anonymous_instance_in_analog_and_plain_call ... ok
test access_alias_resolves_by_first_argument ... ok
test struct_expression_classes ... ok
test named_instance_with_connections_and_params ... ok
test unknown_receiver_is_deferred_without_diagnostic ... ok
test generic_carrier_follows_the_resolved_bound ... ok
test unresolved_callee_is_plain_call ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-spec/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.34s
     Running tests/pom_classify.rs (/var/tmp/piperine-target/debug/deps/pom_classify-3c2ec81106f91996)
```

`Run reflect` - exit 0

```
test qualified_names ... ok
test references_in_source_order ... ok
test writers_are_definitions_and_commits ... ok
test references_navigate_to_declarations ... ok
test sar_adc_instances_name_their_modules ... ok
test gallery_design_round_trips_through_json ... ok
test json_is_deterministic ... ok
test gallery_signals_are_declared_in_their_module ... ok
test resolution_creates_no_node ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-spec/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.55s
     Running tests/pom_reflect.rs (/var/tmp/piperine-target/debug/deps/pom_reflect-6f4aabdd0d8a5976)
```

`Run docs` - exit 0

```
running 5 tests
test mkdocs_nav_lists_every_pom_page ... ok
test every_b3_production_is_mapped ... ok
test msl_fences_parse_clean ... ok
test every_public_pom_type_has_a_heading ... ok
test every_reflection_method_is_documented ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-spec/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.33s
     Running tests/pom_docs.rs (/var/tmp/piperine-target/debug/deps/pom_docs-adec16db49b9e191)
```

`cargo test -p piperine-lang --doc pom::guide 2>&1 / tee /dev/stderr / grep -qE 'test result: ok\. [1-9][0-9]+ ` - exit 0

```
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 836) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 609) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 681) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 563) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 117) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 902) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 957) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 156) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 93) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 341) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 251) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 207) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 316) ... ok

test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.03s
```

`cargo test -p piperine-lang --test pom_design 2>&1 / tee /dev/stderr / grep -cE 'test (load_from_disk_equals_i` - exit 0

```
test mod_with_two_candidates_is_e0161 ... ok
test mod_without_file_is_empty_namespace_e0161 ... ok
test non_utf8_file_is_a_parse_diagnostic ... ok
test unreadable_root_is_an_empty_root_with_e0161 ... ok
test build_retains_name_text_and_ast ... ok
test mod_cycle_is_e0161_and_not_reloaded ... ok
test prelude_files_are_numbered_first ... ok
test parse_diagnostics_pass_through_with_file ... ok
test mod_loads_sibling_file_or_mod_file ... ok
test two_builds_dump_identical ... ok
test load_from_disk_equals_in_memory_build ... ok
test nested_mod_files_follow_rust_layout ... ok
test docs_keep_kind_and_file_across_mod_files ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```
