# explicit-unknowns proofs

**HEAD**: 25719eb8
**Tree**: clean
**Shell**: /bin/sh
**Result**: 64 pass, 32 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run src / test a_binary_carrier_is_read_as_its_value ... ok` | pass |
| C2 | `run src / test control_on_a_carrier_runs_one_path ... ok` | pass |
| C3 | `run ir / test no_value_type_is_flagged ... ok` | pass |
| C4 | `run types / test unknown_is_a_name_that_does_not_resolve ... ok` | **no test ran** |
| C5 | `run types / test the_flag_methods_do_not_resolve ... ok` | pass |
| C6 | `run types / test maybe_is_an_ordinary_field_name ... ok` | pass |
| C7 | `run hygiene / test retired_unknown_codes_are_raised_nowhere ... ok` | pass |
| C8 | `run ops / test control_refuses_a_logic_and_an_option ... ok` | **fail** |
| C9 | `run logic / test analog_reads_a_logic_through_a_function ... ok` | **fail** |
| C10 | `run ops / test analog_refuses_a_bare_logic ... ok` | **fail** |
| C11 | `run src / test a_written_initial_value_is_the_value_at_the_start ... ok` | pass |
| C12 | `run src / test a_carrier_without_a_value_starts_at_its_default ... ok` | pass |
| C13 | `run src / test an_element_never_committed_reads_its_initial_value ... ok` | pass |
| C14 | `run types / test a_missing_initial_value_is_e0187_naming_the_carrier ... ok` | pass |
| C15 | `run types / test an_undriven_carrier_without_default_is_e0187 ... ok` | pass |
| C16 | `run session / test an_undriven_top_port_without_default_refuses_the_analysis ... ok` | pass |
| C17 | `run types / test the_types_that_have_a_default ... ok` | pass |
| C18 | `run types / test deriving_default_over_a_number_is_e0187 ... ok` | pass |
| C19 | `run types / test no_primitive_numeric_type_has_a_default ... ok` | pass |
| C20 | `run src / test an_unbound_option_param_is_none ... ok` | **no test ran** |
| C21 | `run src / test an_option_carrier_is_read_by_matching ... ok` | pass |
| C22 | `run logic / test an_option_of_logic_is_none_or_a_whole_vector ... ok` | **fail** |
| C23 | `run ops / test logic_is_compiled_from_the_library_source ... ok` | **fail** |
| C24 | `run logic / test logic_lays_out_two_planes ... ok` | **fail** |
| C25 | `run logic / test logic_reads_a_four_state_pattern ... ok` | **fail** |
| C26 | `run logic / test at_gives_the_state_of_one_level ... ok` | **fail** |
| C27 | `run logic / test and_follows_the_four_state_table ... ok` | **fail** |
| C28 | `run logic / test or_follows_the_four_state_table ... ok` | **fail** |
| C29 | `run logic / test xor_follows_the_four_state_table ... ok` | **fail** |
| C30 | `run logic / test not_follows_the_four_state_table ... ok` | **fail** |
| C31 | `run logic / test shifts_move_states_and_fill_with_zero ... ok` | **fail** |
| C32 | `run logic / test a_logic_and_a_binary_value_combine_level_by_level ... ok` | **fail** |
| C33 | `run ops / test arithmetic_and_ordering_refuse_a_logic ... ok` | **fail** |
| C34 | `run logic / test equality_of_logic_is_a_logic ... ok` | **fail** |
| C35 | `run logic / test same_compares_the_four_states ... ok` | **fail** |
| C36 | `run logic / test the_four_tests_of_a_logic ... ok` | **fail** |
| C37 | `run ops / test deriving_partial_eq_over_a_logic_is_e0187 ... ok` | **fail** |
| C38 | `run logic / test a_binary_value_converts_to_logic ... ok` | **fail** |
| C39 | `run logic / test a_logic_converts_to_an_option_of_bits ... ok` | **fail** |
| C40 | `run logic / test to_volts_gives_the_level_of_each_state ... ok` | **fail** |
| C41 | `run spectext / test a_nature_takes_trait_impls ... ok` | pass |
| C42 | `run spectext / test resolve_is_a_trait_of_two_parameters ... ok` | pass |
| C43 | `run spectext / test many_drives_need_the_resolve_capability ... ok` | pass |
| C44 | `run spectext / test a_drive_picks_its_impl_by_its_type ... ok` | pass |
| C45 | `run spectext / test resolve_receives_plain_values_for_every_count ... ok` | pass |
| C46 | `run spectext / test a_drive_without_a_value_starts_at_its_default ... ok` | pass |
| C47 | `run spectext / test a_relaxing_drive_starts_absent ... ok` | pass |
| C48 | `run spectext / test a_loop_that_does_not_settle_is_e0511 ... ok` | pass |
| C49 | `run spectext / test strongest_resolves_by_strength ... ok` | pass |
| C50 | `run spectext / test the_library_declares_no_net_nature ... ok` | pass |
| C51 | `run spectext / test the_gallery_works_four_nets ... ok` | pass |
| C52 | `run src / test a_change_is_a_different_value ... ok` | pass |
| C53 | `run src / test the_first_commit_is_a_change ... ok` | pass |
| C54 | `run src / test prev_is_the_initial_value_before_a_change ... ok` | pass |
| C55 | `run src / test an_edge_is_a_transition ... ok` | pass |
| C56 | `run src / test a_carrier_that_starts_high_has_not_risen ... ok` | pass |
| C57 | `run src / test settle_runs_as_before ... ok` | pass |
| C58 | `run src / test an_event_read_is_a_bool ... ok` | pass |
| C59 | `run src / test a_false_guard_runs_nothing ... ok` | pass |
| C60 | `run records / test a_record_holds_its_declared_fields_only ... ok` | pass |
| C61 | `run records / test a_require_is_a_pass_or_a_fail ... ok` | pass |
| C62 | `run records / test a_fatal_record_ends_the_run ... ok` | pass |
| C63 | `run clirun / test a_logic_cell_prints_its_pattern ... ok` | **no test ran** |
| C64 | `run clirun / test an_option_cell_prints_none_or_the_value ... ok` | pass |
| C65 | `run cache / test an_entry_of_the_earlier_contract_is_rebuilt ... ok` | pass |
| C66 | `run abi / test the_shipped_layouts_hold_the_value_only ... ok` | pass |
| C67 | `run abicontract / test the_host_has_one_commit ... ok` | pass |
| C68 | `run abi / test an_option_pin_is_presence_then_payload ... ok` | pass |
| C69 | `run abi / test a_logic_pin_is_two_planes ... ok` | **no test ran** |
| C70 | `run relax / test a_drive_pin_ends_with_a_plain_driving_byte ... ok` | pass |
| C71 | `run spectext / test section_3_3_states_the_explicit_model ... ok` | pass |
| C72 | `run spectext / test section_22_1_holds_the_four_state_tables ... ok` | pass |
| C73 | `run spectext / test the_removed_words_are_absent_from_the_specification ... ok` | pass |
| C74 | `run spectext / test appendix_g_records_the_explicit_model ... ok` | pass |
| C75 | `run docs / test abi_md_describes_the_unreleased_contract ... ok` | pass |
| C76 | `run spectext / test appendix_h_holds_default_from_and_the_operator_traits ... ok` | pass |
| C77 | `run bookvalues / test the_values_chapter_teaches_the_explicit_model ... ok` | pass |
| C78 | `run booklang / test the_book_says_numbers_have_no_default ... ok` | pass |
| C79 | `run bookinv / test the_book_uses_no_removed_name ... ok` | pass |
| C80 | `run booknatures / test the_book_drives_nets_with_strength ... ok` | pass |
| C81 | `run bookinv / test every_listing_that_does_not_run_names_its_gap ... ok` | pass |
| C82 | `run bookabi / test the_abi_chapter_describes_the_unreleased_contract ... ok` | pass |
| C83 | `run docs / test state_holds_the_explicit_unknowns_decision ... ok` | pass |
| C84 | `run docs / test gaps_closes_s6_and_tracks_the_flag ... ok` | pass |
| C85 | `run docs / test the_per_level_feature_is_recorded_as_dropped ... ok` | pass |
| C86 | `run hygiene / test no_source_uses_a_removed_unknown_form ... ok` | pass |
| C87 | `run hygiene / test every_unknown_test_has_a_successor ... ok` | pass |
| C88 | `run excerpts / test every_fence_is_an_excerpt_of_std ... ok` | **no test ran** |
| C89 | `run twins / test every_twin_matches_its_golden ... ok` | **no test ran** |
| C90 | `run buckcli / test the_buck_runs_from_the_cli ... ok` | **no test ran** |
| C91 | `run integrated / test the_integrated_suite_passes ... ok` | **no test ran** |
| C92 | `run types / test a_definition_or_a_child_out_port_specifies_the_value ... ok` | pass |
| Run ops | `cargo test -p piperine-lang --test types_operators` | **fail** - exit 101 |
| Run logic | `cargo test -p piperine-parity --test source_logic` | **fail** - exit 101 |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run src` - exit 0

```
test prev_is_the_initial_value_before_a_change ... ok
test an_if_in_a_cross_argument_picks_its_arm ... ok
test an_option_carrier_is_read_by_matching ... ok
test settle_runs_as_before ... ok
test a_finish_ends_the_run_where_its_guard_holds ... ok
test short_circuit_operands_decide_as_bools ... ok
test an_edge_is_a_transition ... ok
test control_on_a_carrier_runs_one_path ... ok
test the_first_commit_is_a_change ... ok
test a_carrier_that_starts_high_has_not_risen ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.37s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/source_explicit.rs (/var/tmp/piperine-target-a/debug/deps/source_explicit-f3ba4bb5b2b38629)
```

`Run ir` - exit 0

```
test a_settle_event_is_built_checked_and_printed ... ok
test same_and_cast_type_print_and_check ... ok
test integer_and_bit_functions_print ... ok
test carriers_and_drives_print ... ok
test events_and_reactions_print ... ok
test no_value_type_is_flagged ... ok
test the_m1_prints_are_unchanged ... ok
test integers_and_their_operators_print ... ok
test the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-ir)
    Finished `test` profile [unoptimized] target(s) in 3.57s
     Running tests/digital.rs (/var/tmp/piperine-target-a/debug/deps/digital-b1250f05f42d82d3)
```

`Run types` - exit 0

```
test logic_and_matches_bits_wait_for_logic ... ok
test an_undriven_carrier_without_default_is_e0187 ... ok
test maybe_is_an_ordinary_field_name ... ok
test a_function_that_reads_its_parameter_takes_a_carrier_in_analog ... ok
test a_definition_or_a_child_out_port_specifies_the_value ... ok
test a_missing_initial_value_is_e0187_naming_the_carrier ... ok
test the_flag_methods_do_not_resolve ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.40s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-plan)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-transpile)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 2.99s
     Running tests/types_explicit.rs (/var/tmp/piperine-target-a/debug/deps/types_explicit-be7a1e4108a8020d)
```

`Run hygiene` - exit 0

```
test every_ignored_doc_example_is_registered ... ok
test root_targets_match_the_naming_rule ... ok
test no_file_scope_lint_suppression ... ok
test no_ignored_tests ... ok
test no_dead_architecture_identifiers ... ok
test lrm_covers_declarations_are_well_formed ... ok
test retired_unknown_codes_are_raised_nowhere ... ok
test no_disabled_test_code ... ok
test every_unknown_test_has_a_successor ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.07s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.55s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-a/debug/deps/suite_hygiene-354b8333c6c0effe)
```

`Run ops` - exit 101

```
    types_buck
    types_carriers
    types_compound
    types_control
    types_entry
    types_events
    types_explicit
    types_integers
    types_modules
    types_natures
    types_results
    types_structure
    types_subset
    types_value_semantics
    types_values
```

`Run logic` - exit 101

```
    source_environment
    source_events
    source_explicit
    source_fixes
    source_host
    source_limexp
    source_modules
    source_natures
    source_params
    source_reach
    source_records
    source_switch
    source_time
    source_value_semantics
    twins
```

`Run session` - exit 0

```
test the_interpreter_refusal_names_the_body_and_the_rust_backend ... ok
test the_top_key_names_the_module ... ok
test a_missing_rustc_names_rustc_and_the_interpreter ... ok
test the_one_portless_module_is_the_top ... ok
test a_top_with_a_node_port_is_refused_by_the_interpreter_naming_the_host_source ... ok
test a_module_typed_param_is_not_set_by_text ... ok
test no_top_or_several_is_p0011_with_candidates ... ok
test a_run_compiles_once_and_never_during_an_analysis ... ok
test a_top_with_discrete_in_ports_starts_them_at_their_initial_values ... ok
test an_undriven_top_port_without_default_refuses_the_analysis ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.23s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/session.rs (/var/tmp/piperine-target-a/debug/deps/session-79e63c113c6b08bd)
```

`Run spectext` - exit 0

```
test the_host_drives_a_tops_node_ports_e0810_e0811 ... ok
test tran_in_fences_is_the_module ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test zero_bytes_are_the_default_value ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test the_library_declares_no_net_nature ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok

test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 1.20s
     Running tests/spec_text.rs (/var/tmp/piperine-target-a/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run records` - exit 0

```
test a_digital_only_run_without_finish_reaches_its_stop ... ok
test a_text_field_is_the_sources_literal ... ok
test a_require_is_a_pass_or_a_fail ... ok
test a_reserved_stream_keeps_its_name_and_its_site ... ok
test a_record_holds_its_declared_fields_only ... ok
test a_run_ended_by_finish_gives_its_rows_to_the_instant ... ok
test a_fatal_record_ends_the_run ... ok
test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok
test records_arrive_in_time_order ... ok
test keep_matches_a_stream_name_or_its_full_path ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.27s

    Finished `test` profile [unoptimized] target(s) in 0.20s
     Running tests/records.rs (/var/tmp/piperine-target-a/debug/deps/records-636b5e3d604fb760)
```

`Run clirun` - exit 0

```
test run_prints_header_records_blank_line_and_table ... ok
test a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test records_come_before_the_table ... ok
test a_first_compile_is_announced_on_stderr ... ok
test run_of_a_top_with_a_node_port_is_e0810 ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test records_stay_when_the_analysis_fails ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.06s

    Finished `test` profile [unoptimized] target(s) in 0.15s
     Running tests/run.rs (/var/tmp/piperine-target-a/debug/deps/run-3deb22af66054c5e)
```

`Run cache` - exit 0

```
running 6 tests
test the_key_changes_with_any_part ... ok
test an_unwritable_directory_is_named ... ok
test a_directory_is_created_private ... ok
test an_entry_of_the_earlier_contract_is_rebuilt ... ok
test concurrent_opens_of_one_directory_both_succeed ... ok
test two_racing_builds_leave_one_entry_and_no_staging ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.87s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-interp)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 1.03s
     Running tests/cache.rs (/var/tmp/piperine-target-a/debug/deps/cache-3c43db06aa2a7a83)
```

`Run abi` - exit 0

```
running 3 tests
test an_option_pin_is_presence_then_payload ... ok
test the_shipped_layouts_hold_the_value_only ... ok
test layout_equality_ignores_names ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.06s
     Running tests/layouts.rs (/var/tmp/piperine-target-a/debug/deps/layouts-76eb62529b0e07fa)
```

`Run abicontract` - exit 0

```
running 8 tests
test a_list_whose_stride_is_below_its_element_is_refused ... ok
test empty_list_yields_nothing ... ok
test crate_declares_no_dependency ... ok
test list_reads_by_the_producers_stride ... ok
test every_constant_has_its_pinned_value ... ok
test the_host_has_one_commit ... ok
test no_unsafe_outside_raw_thunk_and_caller ... ok
test raw_views_have_no_public_pointer_field ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/contract.rs (/var/tmp/piperine-target-a/debug/deps/contract-f409397b0e770cc4)
```

`Run relax` - exit 0

```
running 5 tests
test a_released_loop_keeps_no_earlier_level ... ok
test an_unsettled_loop_is_e0511_naming_its_drives ... ok
test a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes ... ok
test an_absent_drive_is_left_out_and_a_present_one_is_passed_whatever_its_value ... ok
test a_drive_pin_ends_with_a_plain_driving_byte ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/relax.rs (/var/tmp/piperine-target-a/debug/deps/relax-b90cf2f70ae93855)
```

`Run docs` - exit 0

```
test the_per_level_feature_is_recorded_as_dropped ... ok
test state_records_the_derived_param_decision ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test state_records_the_unknown_semantics_decision ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test the_roadmap_is_forward_looking ... ok
test state_holds_the_explicit_unknowns_decision ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```

`Run bookvalues` - exit 0

```
test e0901_there_is_no_println ... ok
test after_schedules_the_new_value_a_delay_later_and_each_toggle_fires_the_reaction_again ... ok
test a_message_with_values_is_a_record_with_a_text_field ... ok
test analog_reads_a_digital_carrier_directly ... ok
test e0187_a_signal_with_no_first_value_and_no_default_is_refused ... ok
test blinky_counts_three_rising_edges_in_three_microseconds ... ok
test a_function_has_the_class_its_body_gives ... ok
test an_undriven_top_level_in_port_with_no_default_is_refused ... ok
test a_param_has_a_default_a_check_and_a_binding_but_no_assignment ... ok
test the_monitor_file_runs_its_window_monitor_and_its_adder_and_refuses_logic ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.79s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/values.rs (/var/tmp/piperine-target-a/debug/deps/book_values-04a3e7259033323a)
```

`Run booklang` - exit 0

```
test nothing_converts_by_itself_and_a_condition_is_a_bool ... ok
test the_prelude_gives_its_constants ... ok
test the_sum_below_listing_runs_with_the_literal_typed_by_its_use ... ok
test e0903_mut_is_not_a_word_of_the_language ... ok
test e0909_e0905_and_e0903_a_literal_too_large_a_character_and_a_keyword_as_a_name ... ok
test the_literal_forms_have_the_values_their_comments_give ... ok
test blocks_ifs_and_matches_are_expressions_with_one_value ... ok
test constants_and_functions_compute_before_and_during_the_run ... ok
test a_fault_ends_the_analysis_or_the_build_and_a_non_finite_value_ends_the_solve ... ok
test arithmetic_keeps_the_low_bits_and_as_converts_without_failing ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.94s

    Finished `test` profile [unoptimized] target(s) in 0.23s
     Running tests/book/language.rs (/var/tmp/piperine-target-a/debug/deps/book_language-f568cd1da330e959)
```

`Run bookinv` - exit 0

```
test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok
test no_block_row_is_pending ... ok
test a_marked_file_joins_its_blocks ... ok
test no_row_diverges ... ok
test a_model_across_blocks_names_one_test ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok
test the_book_uses_no_removed_name ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s

    Finished `test` profile [unoptimized] target(s) in 0.22s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-1a21c47b4e1a6a60)
```

`Run booknatures` - exit 0

```
test e0222_a_module_cannot_drive_its_own_input ... ok
test e0310_a_one_driver_port_needs_a_direction ... ok
test a_bundle_is_refused_where_the_solver_reaches_it ... ok
test the_open_drain_line_is_refused_until_a_nature_can_implement_resolve ... ok
test signals_of_every_family_are_refused_for_the_arrays_the_option_the_logic_and_the_net ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test behavior_crosses_freely_inside_one_module ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test an_out_port_gives_the_initial_value ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.98s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book/natures.rs (/var/tmp/piperine-target-a/debug/deps/book_natures-84da6f1f33ea55da)
```

`Run bookabi` - exit 0

```
running 3 tests
test the_abi_chapter_describes_the_unreleased_contract ... ok
test two_resistors_of_one_shape_divide_a_source_by_their_values ... ok
test the_flip_flop_starts_low_and_takes_its_input_at_the_rising_edge ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.37s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/book/abi.rs (/var/tmp/piperine-target-a/debug/deps/book_abi-4995c3abf324bc35)
```

`Run excerpts` - exit 0

```
test part_vii_examples_are_fragments ... ok
test a_missing_item_is_named ... ok
test the_forward_guard_covers_part_vii ... ok
test every_part_vii_fence_is_in_its_file ... ok
test prelude_items_outside_part_vii_are_shown_pub ... ok
test the_reverse_guard_covers_std ... ok
test every_std_behavior_block_is_in_a_fence ... ok
test every_std_item_is_in_a_fence ... ok
test the_ahead_lists_hold_only_what_still_differs ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.67s
     Running tests/std_excerpts.rs (/var/tmp/piperine-target-a/debug/deps/std_excerpts-cbdd03ac9b332509)
```

`Run twins` - exit 0

```
running 6 tests
test the_three_op_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.28s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-9b01bd7200d69219)
```

`Run buckcli` - exit 0

```
test startup_prints_no_failed_check has been running for over 60 seconds
test startup_prints_the_whole_start_on_its_grid has been running for over 60 seconds
test the_buck_runs_print_at_most_16_lines has been running for over 60 seconds
test the_buck_runs_print_at_most_16_lines ... ok
test startup_prints_the_whole_start_on_its_grid ... ok
test startup_ends_within_3_mv_of_0_8_volts ... ok
test startup_ends_with_the_reference_at_128 ... ok
test piperine_test_passes_on_the_buck ... ok
test startup_prints_no_failed_check ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 241.06s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-explicit-unknowns/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 0.72s
     Running tests/buck.rs (/var/tmp/piperine-target-a/debug/deps/buck-e0a5ced46b7d0d71)
```

`Run integrated` - exit 0

```
test threshold::the_edges_stream_holds_eight_alternating_records ... ok
test threshold::the_counters_read_four_rises_and_four_falls ... ok
test suite::a_failed_run_panics_every_asker_with_its_message_and_an_unknown_run_is_named ... ok
test sample_hold::the_hold_node_stays_put_while_holding ... ok
test sample_hold::the_hold_node_tracks_the_source ... ok
test pwm_filter::the_filter_follows_its_closed_form ... ok
test pi_loop::every_regulated_check_passes_after_50_us ... ok
test sample_hold::the_done_strobe_is_exactly_50_ns_wide ... ok
test package::opening_every_run_leaves_only_the_package_sources ... ok
test package::every_run_compiles_with_no_warning_and_valid_shapes ... ok

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.45s

    Finished `test` profile [unoptimized] target(s) in 0.44s
     Running tests/integrated/main.rs (/var/tmp/piperine-target-a/debug/deps/integrated-e114db84690e682b)
```
