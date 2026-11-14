# msl-packages proofs

**HEAD**: d08dec18
**Tree**: clean
**Shell**: /bin/sh
**Result**: 118 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run spec / test s35_defines_crate_root_and_dependency_name ... ok` | pass |
| C2 | `run spec / test s35_orders_the_first_segment_lookup ... ok` | pass |
| C3 | `run spec / test s35_local_names_win_in_paths_and_clash_in_use ... ok` | pass |
| C4 | `run spec / test s35_crate_and_super_are_per_crate ... ok` | pass |
| C5 | `run spec / test s35_only_pub_crosses_a_crate ... ok` | pass |
| C6 | `run spec / test std_is_implicit_and_the_prelude_is_std_prelude ... ok` | pass |
| C7 | `run spec / test e0161_row_covers_an_unknown_crate ... ok` | pass |
| C8 | `run spec / test b36_lists_extern_crate ... ok` | pass |
| C9 | `run errors / test extern_crate_is_a_removed_form ... ok` | pass |
| C10 | `run spec / test e0163_is_defined_in_3_5 ... ok` | pass |
| C11 | `run spec / test phase_package_is_listed ... ok` | pass |
| C12 | `run spec / test s36_fixes_the_package_layout ... ok` | pass |
| C13 | `run spec / test s36_lists_every_manifest_key ... ok` | pass |
| C14 | `run spec / test s36_names_are_identifiers ... ok` | pass |
| C15 | `run spec / test s36_states_the_dependency_sources ... ok` | pass |
| C16 | `run spec / test s36_gives_the_version_requirement_table ... ok` | pass |
| C17 | `run spec / test s36_has_no_search_path_and_no_cycle ... ok` | pass |
| C18 | `run spec / test s36_states_the_lockfile_rule ... ok` | pass |
| C19 | `run spec / test s36_defines_the_package_codes ... ok` | pass |
| C20 | `run spec / test appendix_g_records_the_package_decisions ... ok` | pass |
| C21 | `run spec / test glossary_and_file_map_name_crates_and_packages ... ok` | pass |
| C22 | `run crates / test graph_keeps_crates_and_edges_as_given ... ok` | pass |
| C23 | `run crates / test graph_rejects_invalid_and_reserved_names ... ok` | pass |
| C24 | `run crates / test graph_rejects_a_cycle_and_names_it ... ok` | pass |
| C25 | `run crates / test graph_rejects_a_repeated_dependency_name ... ok` | pass |
| C26 | `run crates / test graph_rejects_a_foreign_id ... ok` | pass |
| C27 | `run crates / test piperine_lang_takes_the_graph_as_data ... ok` | pass |
| C28 | `run crates / test single_source_design_has_a_root_crate_named_by_its_file ... ok` | pass |
| C29 | `run crates / test every_design_holds_std_last ... ok` | pass |
| C30 | `run crates / test graph_crates_become_design_crates_in_order ... ok` | pass |
| C31 | `run crates / test mod_files_load_beside_their_own_crate_root ... ok` | pass |
| C32 | `run crates / test an_unreadable_crate_root_is_one_e0161 ... ok` | pass |
| C33 | `run crates / test every_crate_keeps_its_authored_tree ... ok` | pass |
| C34 | `run crates / test disk_and_memory_graphs_build_the_same_design ... ok` | pass |
| C35 | `run crates / test a_dependency_name_starts_a_path ... ok` | pass |
| C36 | `run crates / test an_instance_links_to_a_module_of_a_dependency ... ok` | pass |
| C37 | `run crates / test crate_names_the_root_of_the_containing_crate ... ok` | pass |
| C38 | `run crates / test a_renamed_dependency_resolves_by_its_dependency_name ... ok` | pass |
| C39 | `run crates / test a_local_name_wins_over_a_dependency_in_a_path ... ok` | pass |
| C40 | `run crates / test a_use_path_that_names_a_local_and_a_dependency_is_e0163 ... ok` | pass |
| C41 | `run crates / test crate_prefix_names_the_local_one_in_a_clash ... ok` | pass |
| C42 | `run crates / test std_is_a_dependency_of_every_crate ... ok` | pass |
| C43 | `run crates / test the_prelude_reaches_every_crate ... ok` | pass |
| C44 | `run crates / test a_dependency_name_hides_a_prelude_item ... ok` | pass |
| C45 | `run crates / test a_crate_that_is_no_dependency_is_e0161_with_help ... ok` | pass |
| C46 | `run crates / test a_private_item_of_another_crate_is_e0161 ... ok` | pass |
| C47 | `run crates / test a_pub_item_in_a_private_namespace_is_e0161 ... ok` | pass |
| C48 | `run crates / test super_at_a_crate_root_is_e0161 ... ok` | pass |
| C49 | `run crates / test a_glob_of_a_dependency_brings_only_pub_items ... ok` | pass |
| C50 | `run crates / test a_pub_use_re_exports_across_crates ... ok` | pass |
| C51 | `run crates / test embedded_std_builds_clean_and_has_a_prelude ... ok` | pass |
| C52 | `run crates / test prelude_items_resolve_without_an_import ... ok` | pass |
| C53 | `run crates / test only_pub_items_of_std_prelude_are_implicit ... ok` | pass |
| C54 | `run crates / test std_source_replaces_the_embedded_library ... ok` | pass |
| C55 | `run crates / test a_namespace_item_hides_a_prelude_item ... ok` | pass |
| C56 | `run crates / test std_files_are_numbered_last ... ok` | pass |
| C57 | `run crates / test std_files_see_the_prelude ... ok` | pass |
| C58 | `run crates / test a_crate_reports_its_name_root_and_files ... ok` | pass |
| C59 | `run crates / test declarations_and_files_know_their_crate ... ok` | pass |
| C60 | `run crates / test a_crate_lists_its_declared_dependencies ... ok` | pass |
| C61 | `run crates / test collections_list_the_root_crate_and_a_crate_lists_its_own ... ok` | pass |
| C62 | `run crates / test by_name_lookups_prefer_the_root_crate_then_crate_order ... ok` | pass |
| C63 | `run crates / test crates_are_found_by_name ... ok` | pass |
| C64 | `run crates / test crate_find_inverts_qualified_name ... ok` | pass |
| C65 | `run crates / test a_multi_crate_design_round_trips_through_json ... ok` | pass |
| C66 | `run crates / test from_json_rejects_out_of_range_crate_links ... ok` | pass |
| C67 | `run crates / test from_json_rejects_a_dependency_cycle ... ok` | pass |
| C68 | `run crates / test from_json_rejects_a_crate_whose_root_is_not_its_own ... ok` | pass |
| C69 | `run docs / test docs_describe_crates_and_no_prelude_sources ... ok` | pass |
| C70 | `run docs / test mapping_has_a_crate_row ... ok` | pass |
| C71 | `cargo test -p piperine-lang --doc` | pass |
| C72 | `run manifest / test a_missing_manifest_is_e0170 ... ok` | pass |
| C73 | `run manifest / test invalid_toml_is_e0170_at_its_span ... ok` | pass |
| C74 | `run manifest / test a_package_error_renders_as_a_miette_report ... ok` | pass |
| C75 | `run manifest / test a_valid_manifest_reads_every_package_field ... ok` | pass |
| C76 | `run manifest / test an_unknown_key_is_e0171_at_the_key ... ok` | pass |
| C77 | `run manifest / test a_missing_or_mistyped_package_key_is_e0171 ... ok` | pass |
| C78 | `run manifest / test a_bad_package_name_is_e0171 ... ok` | pass |
| C79 | `run manifest / test a_bad_version_is_e0171 ... ok` | pass |
| C80 | `run manifest / test every_fault_of_a_manifest_is_reported ... ok` | pass |
| C81 | `run manifest / test dependencies_read_in_source_order ... ok` | pass |
| C82 | `run manifest / test a_dependency_needs_exactly_one_source ... ok` | pass |
| C83 | `run manifest / test git_refs_are_exclusive_and_git_only ... ok` | pass |
| C84 | `run manifest / test a_bad_dependency_name_or_requirement_is_e0171 ... ok` | pass |
| C85 | `run paths / test path_dependencies_are_relative_to_their_manifest ... ok` | pass |
| C86 | `run paths / test the_graph_mirrors_the_manifests ... ok` | pass |
| C87 | `run paths / test a_shared_dependency_is_one_crate ... ok` | pass |
| C88 | `run paths / test a_path_without_a_package_is_e0172 ... ok` | pass |
| C89 | `run paths / test a_package_without_a_crate_root_is_e0175 ... ok` | pass |
| C90 | `run paths / test a_package_of_another_name_is_e0173 ... ok` | pass |
| C91 | `run paths / test package_key_renames_a_dependency ... ok` | pass |
| C92 | `run paths / test version_requirements_follow_the_caret_table ... ok` | pass |
| C93 | `run paths / test an_unsatisfied_requirement_is_e0173 ... ok` | pass |
| C94 | `run paths / test a_dependency_cycle_is_e0174_with_its_chain ... ok` | pass |
| C95 | `run paths / test a_package_that_depends_on_itself_is_e0174 ... ok` | pass |
| C96 | `run git / test a_git_dependency_checks_out_the_default_branch ... ok` | pass |
| C97 | `run git / test branch_tag_and_rev_select_their_commit ... ok` | pass |
| C98 | `run git / test an_unreachable_repository_or_ref_is_e0172 ... ok` | pass |
| C99 | `run git / test two_commits_of_one_repository_are_two_crates ... ok` | pass |
| C100 | `run git / test one_commit_of_one_repository_is_one_crate ... ok` | pass |
| C101 | `run git / test a_git_package_resolves_its_own_dependencies ... ok` | pass |
| C102 | `run lock / test the_lockfile_records_each_git_package ... ok` | pass |
| C103 | `run lock / test a_locked_commit_is_kept_when_the_branch_moves ... ok` | pass |
| C104 | `run lock / test no_git_package_no_lockfile ... ok` | pass |
| C105 | `run lock / test the_lockfile_is_deterministic ... ok` | pass |
| C106 | `run lock / test update_moves_the_lock_to_the_new_commit ... ok` | pass |
| C107 | `run lock / test stale_entries_leave_the_lockfile ... ok` | pass |
| C108 | `run lock / test an_invalid_lockfile_is_e0176 ... ok` | pass |
| C109 | `run lock / test a_locked_checkout_needs_no_repository ... ok` | pass |
| C110 | `run lock / test only_the_root_lockfile_counts ... ok` | pass |
| C111 | `run project / test the_project_root_is_the_nearest_manifest ... ok` | pass |
| C112 | `run project / test load_project_builds_the_design_of_a_package_graph ... ok` | pass |
| C113 | `run project / test a_package_error_stops_load_project ... ok` | pass |
| C114 | `run project / test a_model_error_is_a_diagnostic_not_a_package_error ... ok` | pass |
| C115 | `run project / test the_crate_holds_no_retired_surface ... ok` | pass |
| C116 | `run project / test the_crate_reports_exactly_the_package_phase_codes ... ok` | pass |
| C117 | `cargo build -p piperine-project --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^warning')` | pass |

## Output

`Run spec` - exit 0

```
test std_is_implicit_and_the_prelude_is_std_prelude ... ok
test s36_states_the_lockfile_rule ... ok
test s36_has_no_search_path_and_no_cycle ... ok
test value_blocks_may_hold_contributions_and_commits ... ok
test every_method_called_in_a_spec_fence_is_catalogued ... ok
test gnd_and_the_implicit_params_are_core_names ... ok
test s36_defines_the_package_codes ... ok
test e0163_is_defined_in_3_5 ... ok
test e0162_is_defined_in_3_4 ... ok
test every_error_code_is_defined_exactly_once ... ok

test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/spec_text.rs (/var/tmp/piperine-target/debug/deps/spec_text-8e0716f5857daff9)
```

`Run errors` - exit 0

```
test unexpected_token_names_found_and_expected ... ok
test removed_rust_forms_report_targeted_messages ... ok
test parse_phase_rows_report_their_codes ... ok
test nesting_without_delimiters_is_also_bounded ... ok
test depth_limit_fits_8mib_stack_in_debug ... ok
test depth_over_256_is_diagnostic_not_overflow ... ok
test flat_chain_past_4096_links_is_e0910 ... ok
test comparison_lists_are_not_nesting ... ok
test flat_chains_up_to_4096_links_are_not_nesting ... ok
test flat_chains_survive_a_default_2_mib_thread ... ok

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.49s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/msl_errors.rs (/var/tmp/piperine-target/debug/deps/msl_errors-f5f3f043c2e1f80b)
```

`Run crates` - exit 0

```
test by_name_lookups_prefer_the_root_crate_then_crate_order ... ok
test only_pub_items_of_std_prelude_are_implicit ... ok
test the_prelude_reaches_every_crate ... ok
test use_forms_of_a_dependency_resolve ... ok
test super_at_a_crate_root_is_e0161 ... ok
test from_json_rejects_a_crate_whose_root_is_not_its_own ... ok
test single_source_design_has_a_root_crate_named_by_its_file ... ok
test from_json_rejects_a_dependency_cycle ... ok
test from_json_rejects_out_of_range_crate_links ... ok
test from_json_never_panics_on_a_corrupted_crate_field ... ok

test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/pom_crates.rs (/var/tmp/piperine-target/debug/deps/pom_crates-a9f38328ce432af0)
```

`Run docs` - exit 0

```
test mkdocs_nav_lists_every_pom_page ... ok
test every_builtin_row_is_documented ... ok
test place_has_no_parenthesized_form_in_the_docs ... ok
test docs_describe_crates_and_no_prelude_sources ... ok
test every_code_the_pom_reports_is_documented ... ok
test every_b3_production_is_mapped ... ok
test every_public_pom_type_has_a_heading ... ok
test every_b1_construct_is_mapped ... ok
test msl_fences_parse_clean ... ok
test every_reflection_method_is_documented ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/pom_docs.rs (/var/tmp/piperine-target/debug/deps/pom_docs-031147b33902f8bc)
```

`Run manifest` - exit 0

```
test an_unknown_key_is_e0171_at_the_key ... ok
test a_valid_manifest_reads_every_package_field ... ok
test a_missing_or_mistyped_package_key_is_e0171 ... ok
test a_package_error_renders_as_a_miette_report ... ok
test a_dependency_needs_exactly_one_source ... ok
test every_fault_of_a_manifest_is_reported ... ok
test invalid_toml_is_e0170_at_its_span ... ok
test a_bad_dependency_name_or_requirement_is_e0171 ... ok
test dependencies_read_in_source_order ... ok
test git_refs_are_exclusive_and_git_only ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
     Running tests/manifest.rs (/var/tmp/piperine-target/debug/deps/manifest-3a7458efc0eca0e0)
```

`Run paths` - exit 0

```
test a_package_of_another_name_is_e0173 ... ok
test a_package_without_a_crate_root_is_e0175 ... ok
test a_path_without_a_package_is_e0172 ... ok
test a_shared_dependency_is_one_crate ... ok
test path_dependencies_are_relative_to_their_manifest ... ok
test an_unsatisfied_requirement_is_e0173 ... ok
test package_key_renames_a_dependency ... ok
test the_graph_mirrors_the_manifests ... ok
test a_dependency_cycle_is_e0174_with_its_chain ... ok
test version_requirements_follow_the_caret_table ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/paths.rs (/var/tmp/piperine-target/debug/deps/paths-9a9c63782e8dba66)
```

`Run git` - exit 0

```
running 6 tests
test a_git_package_resolves_its_own_dependencies ... ok
test one_commit_of_one_repository_is_one_crate ... ok
test a_git_dependency_checks_out_the_default_branch ... ok
test an_unreachable_repository_or_ref_is_e0172 ... ok
test two_commits_of_one_repository_are_two_crates ... ok
test branch_tag_and_rev_select_their_commit ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.14s
     Running tests/git.rs (/var/tmp/piperine-target/debug/deps/git-d8585614044d79ba)
```

`Run lock` - exit 0

```
running 9 tests
test no_git_package_no_lockfile ... ok
test an_invalid_lockfile_is_e0176 ... ok
test a_locked_commit_is_kept_when_the_branch_moves ... ok
test a_locked_checkout_needs_no_repository ... ok
test only_the_root_lockfile_counts ... ok
test update_moves_the_lock_to_the_new_commit ... ok
test stale_entries_leave_the_lockfile ... ok
test the_lockfile_is_deterministic ... ok
test the_lockfile_records_each_git_package ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
     Running tests/lock.rs (/var/tmp/piperine-target/debug/deps/lock-c995643327bfc307)
```

`Run project` - exit 0

```
running 6 tests
test a_package_error_stops_load_project ... ok
test the_crate_holds_no_retired_surface ... ok
test the_project_root_is_the_nearest_manifest ... ok
test the_crate_reports_exactly_the_package_phase_codes ... ok
test a_model_error_is_a_diagnostic_not_a_package_error ... ok
test load_project_builds_the_design_of_a_package_graph ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
     Running tests/project.rs (/var/tmp/piperine-target/debug/deps/project-fa4e98260f8d6243)
```

`cargo test -p piperine-lang --doc` - exit 0

```
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 111) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 138) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 954) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::reflection (line 93) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 188) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 239) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 283) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 383) ... ok
test crates/piperine-lang/src/pom/guide.rs - pom::guide::resolution (line 358) ... ok

test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

all doctests ran in 1.05s; merged doctests compilation took 0.98s
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
   Doc-tests piperine_lang
```

`cargo build -p piperine-project --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` - exit 0

```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
```
