# msl-stdlib proofs

**HEAD**: e12c8946
**Tree**: clean
**Shell**: /bin/sh
**Result**: 114 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run spec / test s22_says_std_is_the_normative_source ... ok` | pass |
| C2 | `run spec / test s22_maps_sections_to_namespaces ... ok` | pass |
| C3 | `run spec / test s22_drops_the_implied_pub_resolution ... ok` | pass |
| C4 | `run spec / test g38_is_closed ... ok` | pass |
| C5 | `run spec / test appendix_c_says_parsed_and_resolved ... ok` | pass |
| C6 | `run spec / test s1_constants_are_std_items ... ok` | pass |
| C7 | `run spec / test analysis_kinds_list_transient ... ok` | pass |
| C8 | `run spec / test no_analysis_tran_remains ... ok` | pass |
| C9 | `run spec / test tran_in_fences_is_the_module ... ok` | pass |
| C10 | `run spec / test appendix_g_records_the_stdlib_decisions ... ok` | pass |
| C11 | `run spec / test s3_lists_the_intrinsic_abi ... ok` | pass |
| C12 | `run spec / test s3_states_the_intrinsic_contract ... ok` | pass |
| C13 | `run spec / test e0125_covers_intrinsic_outside_std ... ok` | pass |
| C14 | `run spec / test s22_lists_the_engine_items ... ok` | pass |
| C15 | `run spec / test s22_rule_4_names_extern_intrinsic ... ok` | pass |
| C16 | `run spec / test b1_still_counts_65_constructs ... ok` | pass |
| C17 | `run spec / test e0707_is_replaced_by_e0317 ... ok` | pass |
| C18 | `run spec / test laplace_states_the_proper_rule ... ok` | pass |
| C19 | `run spec / test g64_names_the_normal_generator ... ok` | pass |
| C20 | `run resolve / test intrinsic_fn_is_accepted_in_std ... ok` | pass |
| C21 | `run resolve / test intrinsic_fn_outside_std_is_e0125 ... ok` | pass |
| C22 | `run resolve / test intrinsic_fn_with_a_body_is_e0125 ... ok` | pass |
| C23 | `run resolve / test is_intrinsic_reads_the_abi ... ok` | pass |
| C24 | `run resolve / test extern_c_fn_is_not_intrinsic ... ok` | pass |
| C25 | `run resolve / test a_call_to_an_intrinsic_resolves ... ok` | pass |
| C26 | `run resolve / test self_assoc_resolves_through_repr_supertrait ... ok` | pass |
| C27 | `run resolve / test self_assoc_resolves_through_a_user_supertrait ... ok` | pass |
| C28 | `run resolve / test self_assoc_missing_everywhere_is_e0161 ... ok` | pass |
| C29 | `run resolve / test blanket_method_resolves_on_a_carrier ... ok` | pass |
| C30 | `run resolve / test blanket_method_resolves_on_self_in_a_nature_impl ... ok` | pass |
| C31 | `run resolve / test a_nature_impl_wins_over_a_blanket ... ok` | pass |
| C32 | `run resolve / test an_unlinked_member_keeps_its_slot ... ok` | pass |
| C33 | `run resolve / test a_blanket_call_is_a_reference_of_the_trait_method ... ok` | pass |
| C34 | `run std / test std_root_declares_nine_namespaces ... ok` | pass |
| C35 | `run std / test each_namespace_holds_its_section ... ok` | pass |
| C36 | `run std / test prelude_is_eight_reexports ... ok` | pass |
| C37 | `run std / test embedded_list_names_every_file_once ... ok` | pass |
| C38 | `run std / test every_std_file_is_embedded ... ok` | pass |
| C39 | `run std / test every_index_item_is_declared ... ok` | pass |
| C40 | `run std / test every_item_but_the_intrinsics_has_a_body ... ok` | pass |
| C41 | `run std / test intrinsics_are_exactly_the_engine_table ... ok` | pass |
| C42 | `run std / test every_intrinsic_reads_a_str ... ok` | pass |
| C43 | `run std / test every_index_item_is_pub ... ok` | pass |
| C44 | `run std / test std_declares_no_core_name ... ok` | pass |
| C45 | `run std / test constants_have_the_values_of_s1 ... ok` | pass |
| C46 | `run std / test root_forms_place_their_coefficient_form ... ok` | pass |
| C47 | `run std / test root_params_carry_a_where ... ok` | pass |
| C48 | `run std / test laplace_nd_has_state_signals_and_an_analog_block ... ok` | pass |
| C49 | `run std / test noise_table_is_noise_of_table_model ... ok` | pass |
| C50 | `run std / test pure_bodies_name_no_primitive ... ok` | pass |
| C51 | `run std / test tgate_places_two_tran ... ok` | pass |
| C52 | `run std / test a_std_item_resolves_by_its_single_name ... ok` | pass |
| C53 | `run std / test prelude_exports_equal_the_index ... ok` | pass |
| C54 | `run std / test a_std_path_names_the_same_declaration ... ok` | pass |
| C55 | `run std / test a_private_std_item_is_e0161 ... ok` | pass |
| C56 | `run std / test no_export_has_a_core_name ... ok` | pass |
| C57 | `run std / test bit_rise_resolves_into_std_values ... ok` | pass |
| C58 | `run std / test known_resolves_into_std_values ... ok` | pass |
| C59 | `run std / test converters_resolve_into_std_convert ... ok` | pass |
| C60 | `run std / test monitors_resolve_into_std_monitor ... ok` | pass |
| C61 | `run std / test embedded_std_has_no_diagnostics ... ok` | pass |
| C62 | `run std / test std_has_no_unresolved_member ... ok` | pass |
| C63 | `run std / test embedded_std_passes_check ... ok` | pass |
| C64 | `run std / test a_replaced_std_without_an_item_is_e0161 ... ok` | pass |
| C65 | `run std / test transient_is_the_kind_and_tran_the_module ... ok` | pass |
| C66 | `run excerpts / test every_part_vii_fence_is_in_its_file ... ok` | pass |
| C67 | `run excerpts / test part_vii_examples_are_fragments ... ok` | pass |
| C68 | `run excerpts / test the_forward_guard_covers_part_vii ... ok` | pass |
| C69 | `run excerpts / test a_missing_fence_is_named ... ok` | pass |
| C70 | `run excerpts / test a_fence_one_character_off_is_not_found ... ok` | pass |
| C71 | `run excerpts / test every_std_item_is_in_a_fence ... ok` | pass |
| C72 | `run excerpts / test every_std_behavior_block_is_in_a_fence ... ok` | pass |
| C73 | `run excerpts / test prelude_items_outside_part_vii_are_shown_pub ... ok` | pass |
| C74 | `run excerpts / test a_missing_item_is_named ... ok` | pass |
| C75 | `run excerpts / test the_reverse_guard_covers_std ... ok` | pass |
| C76 | `run galleries / test gallery_fixtures_exist ... ok` | pass |
| C77 | `run galleries / test every_fixture_line_is_spec_text ... ok` | pass |
| C78 | `run galleries / test the_fixture_table_covers_every_gallery ... ok` | pass |
| C79 | `run galleries / test the_fixture_table_names_only_used_models ... ok` | pass |
| C80 | `run galleries / test every_gallery_builds_with_no_diagnostics ... ok` | pass |
| C81 | `run galleries / test every_gallery_passes_check ... ok` | pass |
| C82 | `run galleries / test no_gallery_has_an_unresolved_member ... ok` | pass |
| C83 | `run galleries / test every_gallery_references_its_std_items ... ok` | pass |
| C84 | `run galleries / test the_gate_builds_18_fences ... ok` | pass |
| C85 | `run galleries / test a_dirty_gallery_is_reported_by_fence ... ok` | pass |
| C86 | `run galleries / test galleries_fail_against_an_empty_prelude ... ok` | pass |
| C87 | `run galleries / test every_gallery_lists_a_std_item ... ok` | pass |
| C88 | `run resolve / test the_constants_are_not_builtin ... ok` | pass |
| C89 | `run resolve / test k_b_resolves_to_std_constants ... ok` | pass |
| C90 | `run resolve / test transient_is_a_builtin_variant ... ok` | pass |
| C91 | `run resolve / test the_core_rows_are_unchanged ... ok` | pass |
| C92 | `run resolve / test a_model_constant_hides_the_prelude_constant ... ok` | pass |
| C93 | `run std / test the_fixture_prelude_is_gone ... ok` | pass |
| C94 | `run std / test no_pom_test_was_removed ... ok` | pass |
| C95 | `run docs / test the_builtin_table_doc_has_no_const_row ... ok` | pass |
| C96 | `run docs / test resolution_doc_describes_intrinsics ... ok` | pass |
| C97 | `run docs / test resolution_doc_describes_the_supertrait_rule ... ok` | pass |
| C98 | `run docs / test members_doc_lists_the_blanket_step ... ok` | pass |
| C99 | `cargo test -p piperine-lang --doc` | pass |
| C100 | `run std / test std_root_comment_names_the_namespaces ... ok` | pass |
| C101 | `run book / test the_book_source_is_in_docs_book ... ok` | pass |
| C102 | `run book / test build_py_writes_an_ignored_index ... ok` | pass |
| C103 | `run book / test the_core_std_chapter_follows_packages ... ok` | pass |
| C104 | `run book / test the_chapter_names_every_core_name ... ok` | pass |
| C105 | `run book / test the_chapter_has_a_part_per_namespace ... ok` | pass |
| C106 | `run book / test the_chapter_names_every_prelude_export ... ok` | pass |
| C107 | `run book / test the_chapter_explains_the_prelude_and_the_engine_items ... ok` | pass |
| C108 | `run book / test chapter_blocks_build_against_std ... ok` | pass |
| C109 | `run book / test every_msl_block_parses ... ok` | pass |
| C110 | `run book / test packages_and_cheatsheet_name_std ... ok` | pass |
| C111 | `run book / test the_book_has_at_least_40_msl_blocks ... ok` | pass |
| C112 | `run book / test a_missing_name_is_listed ... ok` | pass |
| C113 | `run book / test the_book_says_transient ... ok` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run spec` - exit 0

```
test std_is_implicit_and_the_prelude_is_std_prelude ... ok
test shared_borrow_of_any_argument_is_allowed ... ok
test e0162_is_defined_in_3_4 ... ok
test s3_states_the_intrinsic_contract ... ok
test tran_in_fences_is_the_module ... ok
test every_method_called_in_a_spec_fence_is_catalogued ... ok
test e0163_is_defined_in_3_5 ... ok
test s36_defines_the_package_codes ... ok
test gnd_and_the_implicit_params_are_core_names ... ok
test every_error_code_is_defined_exactly_once ... ok

test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/spec_text.rs (/var/tmp/piperine-target/debug/deps/spec_text-8e0716f5857daff9)
```

`Run resolve` - exit 0

```
test unresolved_path_is_one_e0161 ... ok
test use_in_a_block_resolves_with_block_scope ... ok
test use_leaf_text_is_its_own_segments ... ok
test use_self_imports_the_namespace ... ok
test type_paths_resolve_to_declarations ... ok
test use_self_with_alias_and_nesting ... ok
test embedded_std_builds_clean ... ok
test k_b_resolves_to_std_constants ... ok
test embedded_std_declares_no_core_name ... ok
test prelude_source_items_are_visible_everywhere ... ok

test result: ok. 67 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_resolve.rs (/var/tmp/piperine-target/debug/deps/pom_resolve-f44ddda9709d3f18)
```

`Run std` - exit 0

```
test every_item_but_the_intrinsics_has_a_body ... ok
test known_resolves_into_std_values ... ok
test root_forms_place_their_coefficient_form ... ok
test std_declares_no_core_name ... ok
test root_params_carry_a_where ... ok
test std_has_no_unresolved_member ... ok
test tgate_places_two_tran ... ok
test transient_is_the_kind_and_tran_the_module ... ok
test std_root_comment_names_the_namespaces ... ok
test std_root_declares_nine_namespaces ... ok

test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/std_library.rs (/var/tmp/piperine-target/debug/deps/std_library-d659b89a2fdefd78)
```

`Run excerpts` - exit 0

```
test a_fence_one_character_off_is_not_found ... ok
test a_missing_fence_is_named ... ok
test part_vii_examples_are_fragments ... ok
test the_forward_guard_covers_part_vii ... ok
test a_missing_item_is_named ... ok
test every_part_vii_fence_is_in_its_file ... ok
test prelude_items_outside_part_vii_are_shown_pub ... ok
test the_reverse_guard_covers_std ... ok
test every_std_item_is_in_a_fence ... ok
test every_std_behavior_block_is_in_a_fence ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/std_excerpts.rs (/var/tmp/piperine-target/debug/deps/std_excerpts-420843ec3999f39a)
```

`Run galleries` - exit 0

```
test the_fixture_table_covers_every_gallery ... ok
test every_fixture_line_is_spec_text ... ok
test the_fixture_table_names_only_used_models ... ok
test galleries_fail_against_an_empty_prelude ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test the_gate_builds_18_fences ... ok
test every_gallery_builds_with_no_diagnostics ... ok
test every_gallery_references_its_std_items ... ok
test no_gallery_has_an_unresolved_member ... ok
test every_gallery_passes_check ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.79s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/std_galleries.rs (/var/tmp/piperine-target/debug/deps/std_galleries-f0c43a9b8d6704b6)
```

`Run docs` - exit 0

```
test resolution_doc_describes_intrinsics ... ok
test every_builtin_row_is_documented ... ok
test docs_describe_crates_and_no_prelude_sources ... ok
test the_builtin_table_doc_has_no_const_row ... ok
test every_b3_production_is_mapped ... ok
test every_public_pom_type_has_a_heading ... ok
test every_code_the_pom_reports_is_documented ... ok
test msl_fences_parse_clean ... ok
test every_b1_construct_is_mapped ... ok
test every_reflection_method_is_documented ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/pom_docs.rs (/var/tmp/piperine-target/debug/deps/pom_docs-031147b33902f8bc)
```

`Run book` - exit 0

```
test the_book_says_transient ... ok
test the_core_std_chapter_follows_packages ... ok
/home/keslarek/Git/piperine-msl-stdlib/docs/book/index.html 142779
test build_py_writes_an_ignored_index ... ok
test every_msl_block_parses ... ok
test the_chapter_has_a_part_per_namespace ... ok
test the_chapter_names_every_prelude_export ... ok
test the_chapter_explains_the_prelude_and_the_engine_items ... ok
test packages_and_cheatsheet_name_std ... ok
test chapter_blocks_build_against_std ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running tests/book.rs (/var/tmp/piperine-target/debug/deps/book-ca9323b66dec99ca)
```

`cargo test -p piperine-lang --doc` - exit 0

```
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 93) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 955) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 129) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 238) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 111) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 291) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 187) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 392) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 367) ... ok

test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.28s

all doctests ran in 3.32s; merged doctests compilation took 1.00s
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
   Doc-tests piperine_lang
```
