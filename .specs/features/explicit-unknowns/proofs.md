# explicit-unknowns proofs

**HEAD**: ae5b876a
**Tree**: clean
**Shell**: /bin/sh
**Result**: 95 pass, 1 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run src / test a_binary_carrier_of_any_shape_is_read_as_its_value_everywhere ... ok` | pass |
| C2 | `run src / test control_on_a_carrier_runs_one_path ... ok` | pass |
| C3 | `run src / test the_buck_elaborated_from_source_holds_no_flag_and_no_merge ... ok` | pass |
| C4 | `run types / test unknown_is_a_name_that_does_not_resolve ... ok` | pass |
| C5 | `run types / test the_flag_methods_do_not_resolve ... ok` | pass |
| C6 | `run src / test maybe_is_an_ordinary_field_of_a_record ... ok` | pass |
| C7 | `run hygiene / test retired_unknown_codes_are_raised_nowhere ... ok` | pass |
| C8 | `run ops / test control_refuses_a_logic_and_an_option ... ok` | pass |
| C9 | `run logic / test analog_reads_a_logic_through_a_function ... ok` | pass |
| C10 | `run ops / test analog_refuses_a_bare_logic ... ok` | pass |
| C11 | `run src / test a_written_initial_value_is_the_value_at_the_start ... ok` | pass |
| C12 | `run src / test a_carrier_without_a_value_starts_at_its_default ... ok` | pass |
| C13 | `run src / test an_element_never_committed_reads_its_initial_value ... ok` | pass |
| C14 | `run types / test a_missing_initial_value_is_e0187_naming_the_carrier ... ok` | pass |
| C15 | `run types / test an_undriven_carrier_without_default_is_e0187 ... ok` | pass |
| C16 | `run session / test an_undriven_top_port_without_default_refuses_the_analysis ... ok` | pass |
| C17 | `run types / test the_types_that_have_a_default ... ok` | pass |
| C18 | `run types / test deriving_default_over_a_number_is_e0187 ... ok` | pass |
| C19 | `run types / test no_primitive_numeric_type_has_a_default ... ok` | pass |
| C20 | `run src / test an_unbound_option_param_is_none ... ok` | pass |
| C21 | `run src / test an_option_carrier_is_read_by_matching ... ok` | pass |
| C22 | `run logic / test an_option_of_logic_is_none_or_a_whole_vector ... ok` | pass |
| C23 | `run ops / test logic_is_compiled_from_the_library_source ... ok` | pass |
| C24 | `run logic / test logic_lays_out_two_planes ... ok` | pass |
| C25 | `run logic / test logic_reads_a_four_state_pattern ... ok` | pass |
| C26 | `run logic / test at_gives_the_state_of_one_level ... ok` | pass |
| C27 | `run logic / test and_follows_the_four_state_table ... ok` | pass |
| C28 | `run logic / test or_follows_the_four_state_table ... ok` | pass |
| C29 | `run logic / test xor_follows_the_four_state_table ... ok` | pass |
| C30 | `run logic / test not_follows_the_four_state_table ... ok` | pass |
| C31 | `run logic / test shifts_move_states_and_fill_with_zero ... ok` | pass |
| C32 | `run logic / test a_logic_and_a_binary_value_combine_level_by_level ... ok` | pass |
| C33 | `run ops / test arithmetic_and_ordering_refuse_a_logic ... ok` | pass |
| C34 | `run logic / test equality_of_logic_is_a_logic ... ok` | pass |
| C35 | `run logic / test same_compares_the_four_states ... ok` | pass |
| C36 | `run logic / test the_four_tests_of_a_logic ... ok` | pass |
| C37 | `run ops / test deriving_partial_eq_over_a_logic_is_e0187 ... ok` | pass |
| C38 | `run logic / test a_binary_value_converts_to_logic ... ok` | pass |
| C39 | `run logic / test a_logic_converts_to_an_option_of_bits ... ok` | pass |
| C40 | `run logic / test to_volts_gives_the_level_of_each_state ... ok` | pass |
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
| C63 | `run clirun / test a_logic_cell_prints_its_pattern ... ok` | pass |
| C64 | `run clirun / test an_option_cell_prints_none_or_the_value ... ok` | pass |
| C65 | `run cache / test an_entry_of_the_earlier_contract_is_rebuilt ... ok` | pass |
| C66 | `run abi / test the_shipped_layouts_hold_the_value_only ... ok` | pass |
| C67 | `run abicontract / test the_host_has_one_commit ... ok` | pass |
| C68 | `run plan / test an_option_carrier_pin_is_presence_then_payload ... ok` | pass |
| C69 | `run abi / test a_logic_pin_is_two_planes ... ok` | pass |
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
| C88 | `run excerpts / test every_part_vii_fence_is_in_its_file ... ok` | pass |
| C89 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C90 | `run buckcli / test piperine_test_passes_on_the_buck ... ok` | pass |
| C91 | `run integrated / test the_runs_execute_at_once_in_their_own_build_directories ... ok` | pass |
| C91 | `run cliintegrated / test piperine_test_passes_pi_loop_and_threshold ... ok` | pass |
| C92 | `run types / test a_definition_or_a_child_out_port_specifies_the_value ... ok` | pass |
| Run buckcli | `cargo test -p piperine-cli --test buck` | **fail** - exit 101 |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run src` - exit 0

```
test an_edge_is_a_transition ... ok
test an_option_carrier_is_read_by_matching ... ok
test the_buck_elaborated_from_source_holds_no_flag_and_no_merge ... ok
test a_finish_ends_the_run_where_its_guard_holds ... ok
test prev_is_the_initial_value_before_a_change ... ok
test control_on_a_carrier_runs_one_path ... ok
test settle_runs_as_before ... ok
test short_circuit_operands_decide_as_bools ... ok
test the_first_commit_is_a_change ... ok
test a_carrier_that_starts_high_has_not_risen ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.85s

    Finished `test` profile [unoptimized] target(s) in 0.20s
     Running tests/source_explicit.rs (/var/tmp/piperine-target-a/debug/deps/source_explicit-f3ba4bb5b2b38629)
```

`Run types` - exit 0

```
test maybe_is_an_ordinary_field_name ... ok
test width_is_typed_u32_on_a_concrete_type ... ok
test a_function_that_reads_its_parameter_takes_a_carrier_in_analog ... ok
test unknown_is_a_name_that_does_not_resolve ... ok
test a_cast_into_a_sequence_that_holds_an_enum_is_refused ... ok
test logic_and_matches_bits_are_typed_over_logic ... ok
test a_definition_or_a_child_out_port_specifies_the_value ... ok
test a_missing_initial_value_is_e0187_naming_the_carrier ... ok
test the_flag_methods_do_not_resolve ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.15s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 1.08s
     Running tests/types_explicit.rs (/var/tmp/piperine-target-a/debug/deps/types_explicit-be7a1e4108a8020d)
```

`Run hygiene` - exit 0

```
test every_ignored_doc_example_is_registered ... ok
test the_walk_covers_the_three_solver_crates ... ok
test no_file_scope_lint_suppression ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_dead_architecture_identifiers ... ok
test no_ignored_tests ... ok
test retired_unknown_codes_are_raised_nowhere ... ok
test no_disabled_test_code ... ok
test every_unknown_test_has_a_successor ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.86s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.58s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-a/debug/deps/suite_hygiene-354b8333c6c0effe)
```

`Run ops` - exit 0

```
test two_impls_for_one_type_and_one_rhs_are_e0187 ... ok
test the_right_operand_picks_the_impl ... ok
test an_operator_without_an_impl_is_e0180 ... ok
test deriving_partial_eq_over_a_logic_is_e0187 ... ok
test analog_refuses_a_bare_logic ... ok
test control_refuses_a_logic_and_an_option ... ok
test an_operator_on_a_struct_is_its_impl_function_and_takes_its_out ... ok
test arithmetic_and_ordering_refuse_a_logic ... ok
test logic_is_compiled_from_the_library_source ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.67s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.90s
     Running tests/types_operators.rs (/var/tmp/piperine-target-a/debug/deps/types_operators-13213b31eee407e3)
```

`Run logic` - exit 0

```
test a_binary_value_converts_to_logic ... ok
test to_volts_gives_the_level_of_each_state ... ok
test equality_of_logic_is_a_logic ... ok
test not_follows_the_four_state_table ... ok
test shifts_move_states_and_fill_with_zero ... ok
test same_compares_the_four_states ... ok
test or_follows_the_four_state_table ... ok
test the_tables_hold_at_sixty_four_levels ... ok
test the_four_tests_of_a_logic ... ok
test xor_follows_the_four_state_table ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.87s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/source_logic.rs (/var/tmp/piperine-target-a/debug/deps/source_logic-13255a9caa9977d1)
```

`Run session` - exit 0

```
test an_integer_out_of_range_is_e0802 ... ok
test an_integer_param_set_by_text_in_range ... ok
test a_run_compiles_once_and_never_during_an_analysis ... ok
test an_option_param_set_to_a_value_or_none ... ok
test an_undriven_top_port_without_default_refuses_the_analysis ... ok
test an_unknown_variant_is_e0802_listing_the_variants ... ok
test a_param_flag_is_read_by_the_params_type_before_the_run ... ok
test params_passed_through_a_wrappers_placement_run ... ok
test an_integer_above_2_pow_53_is_set_exactly ... ok
test a_structural_params_bad_text_is_e0802 ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 128.60s

    Finished `test` profile [unoptimized] target(s) in 0.52s
     Running tests/session.rs (/var/tmp/piperine-target-a/debug/deps/session-b8872f77132e3cb3)
```

`Run spectext` - exit 0

```
test tran_in_fences_is_the_module ... ok
test the_step_that_reads_a_jump_is_first_order ... ok
test the_library_declares_no_net_nature ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test zero_bytes_are_the_default_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok

test result: ok. 137 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/spec_text.rs (/var/tmp/piperine-target-a/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run records` - exit 0

```
test a_design_without_emit_gives_no_record ... ok
test a_digital_only_run_without_finish_reaches_its_stop ... ok
test a_run_ended_by_finish_gives_its_rows_to_the_instant ... ok
test a_reserved_stream_keeps_its_name_and_its_site ... ok
test a_record_holds_its_declared_fields_only ... ok
test a_fatal_record_ends_the_run ... ok
test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok
test records_arrive_in_time_order ... ok
test a_require_is_a_pass_or_a_fail ... ok
test keep_matches_a_stream_name_or_its_full_path ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.20s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/records.rs (/var/tmp/piperine-target-a/debug/deps/records-a996361eedbb915b)
```

`Run clirun` - exit 0

```
test a_first_compile_is_announced_on_stderr ... ok
test a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot ... ok
test composites_print_in_each_output_format ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test run_of_a_top_with_a_node_port_is_e0810 ... ok
test records_stay_when_the_analysis_fails ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test param_of_an_integer_from_the_command_line_reads_from_the_start ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.45s

    Finished `test` profile [unoptimized] target(s) in 0.24s
     Running tests/run.rs (/var/tmp/piperine-target-a/debug/deps/run-3deb22af66054c5e)
```

`Run cache` - exit 0

```
running 6 tests
test the_key_changes_with_any_part ... ok
test an_unwritable_directory_is_named ... ok
test a_directory_is_created_private ... ok
test concurrent_opens_of_one_directory_both_succeed ... ok
test an_entry_of_the_earlier_contract_is_rebuilt ... ok
test two_racing_builds_leave_one_entry_and_no_staging ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.37s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 1.21s
     Running tests/cache.rs (/var/tmp/piperine-target-a/debug/deps/cache-3c43db06aa2a7a83)
```

`Run abi` - exit 0

```
running 4 tests
test a_logic_pin_is_two_planes ... ok
test an_option_pin_is_presence_then_payload ... ok
test layout_equality_ignores_names ... ok
test the_shipped_layouts_hold_the_value_only ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.06s
     Running tests/layouts.rs (/var/tmp/piperine-target-a/debug/deps/layouts-76eb62529b0e07fa)
```

`Run abicontract` - exit 0

```
running 8 tests
test a_list_whose_stride_is_below_its_element_is_refused ... ok
test empty_list_yields_nothing ... ok
test crate_declares_no_dependency ... ok
test every_constant_has_its_pinned_value ... ok
test list_reads_by_the_producers_stride ... ok
test the_host_has_one_commit ... ok
test no_unsafe_outside_raw_thunk_and_caller ... ok
test raw_views_have_no_public_pointer_field ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/contract.rs (/var/tmp/piperine-target-a/debug/deps/contract-f409397b0e770cc4)
```

`Run plan` - exit 0

```
test every_record_layout_holds_its_declared_fields_only ... ok
test a_drive_is_a_discrete_function ... ok
test only_a_literal_positive_delay_is_later ... ok
test a_reaction_lists_its_events_reads_and_statements ... ok
test carriers_are_discrete_pins_that_hold_the_value_only ... ok
test every_buck_slice_shape_validates ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 16.22s
     Running tests/digital_shape.rs (/var/tmp/piperine-target-a/debug/deps/digital_shape-3b66a9e5414938d1)
```

`Run relax` - exit 0

```
running 5 tests
test a_relaxing_cycle_starts_absent_and_stops_within_drives_plus_one_passes ... ok
test a_released_loop_keeps_no_earlier_level ... ok
test a_drive_pin_ends_with_a_plain_driving_byte ... ok
test an_absent_drive_is_left_out_and_a_present_one_is_passed_whatever_its_value ... ok
test an_unsettled_loop_is_e0511_naming_its_drives ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/relax.rs (/var/tmp/piperine-target-a/debug/deps/relax-b90cf2f70ae93855)
```

`Run docs` - exit 0

```
test the_buck_spec_holds_the_bench_bounds ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test transient_performance_closes_v12 ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test state_holds_the_explicit_unknowns_decision ... ok
test the_roadmap_is_forward_looking ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```

`Run bookvalues` - exit 0

```
test e0601_an_emit_in_a_module_body_is_misplaced_output ... ok
test a_message_with_values_is_a_record_with_a_text_field ... ok
test analog_reads_a_digital_carrier_directly ... ok
test after_schedules_the_new_value_a_delay_later_and_each_toggle_fires_the_reaction_again ... ok
test e0187_a_signal_with_no_first_value_and_no_default_is_refused ... ok
test blinky_counts_three_rising_edges_in_three_microseconds ... ok
test a_function_has_the_class_its_body_gives ... ok
test an_undriven_top_level_in_port_with_no_default_is_refused ... ok
test a_param_has_a_default_a_check_and_a_binding_but_no_assignment ... ok
test the_monitor_file_runs_its_window_monitor_and_its_adder_and_refuses_logic ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.57s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/values.rs (/var/tmp/piperine-target-a/debug/deps/book_values-04a3e7259033323a)
```

`Run booklang` - exit 0

```
test nothing_converts_by_itself_and_a_condition_is_a_bool ... ok
test the_sum_below_listing_runs_with_the_literal_typed_by_its_use ... ok
test e0903_mut_is_not_a_word_of_the_language ... ok
test the_prelude_gives_its_constants ... ok
test e0909_e0905_and_e0903_a_literal_too_large_a_character_and_a_keyword_as_a_name ... ok
test blocks_ifs_and_matches_are_expressions_with_one_value ... ok
test the_literal_forms_have_the_values_their_comments_give ... ok
test a_fault_ends_the_analysis_or_the_build_and_a_non_finite_value_ends_the_solve ... ok
test constants_and_functions_compute_before_and_during_the_run ... ok
test arithmetic_keeps_the_low_bits_and_as_converts_without_failing ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.35s

    Finished `test` profile [unoptimized] target(s) in 0.15s
     Running tests/book/language.rs (/var/tmp/piperine-target-a/debug/deps/book_language-f568cd1da330e959)
```

`Run bookinv` - exit 0

```
test the_gaps_guard_names_a_missing_and_an_extra_row ... ok
test disputed_rows_are_listed_for_the_user ... ok
test every_listing_that_does_not_run_names_its_gap ... ok
test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_row_names_an_existing_test ... ok
test the_book_uses_no_removed_name ... ok
test the_book_reads_every_block ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-1a21c47b4e1a6a60)
```

`Run booknatures` - exit 0

```
test e0250_discrete_and_continuous_never_connect ... ok
test e0310_a_one_driver_port_needs_a_direction ... ok
test a_bundle_is_refused_where_the_solver_reaches_it ... ok
test the_open_drain_line_is_refused_until_a_nature_can_implement_resolve ... ok
test signals_of_every_family_are_refused_for_the_arrays_the_option_the_logic_and_the_net ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok
test an_out_port_gives_the_initial_value ... ok
test behavior_crosses_freely_inside_one_module ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.60s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/natures.rs (/var/tmp/piperine-target-a/debug/deps/book_natures-84da6f1f33ea55da)
```

`Run bookabi` - exit 0

```
running 3 tests
test the_abi_chapter_describes_the_unreleased_contract ... ok
test two_resistors_of_one_shape_divide_a_source_by_their_values ... ok
test the_flip_flop_starts_low_and_takes_its_input_at_the_rising_edge ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.36s

    Finished `test` profile [unoptimized] target(s) in 0.14s
     Running tests/book/abi.rs (/var/tmp/piperine-target-a/debug/deps/book_abi-4995c3abf324bc35)
```

`Run excerpts` - exit 0

```
test the_reverse_guard_counts_only_valid_fences_by_whole_line ... ok
test part_vii_examples_are_fragments ... ok
test the_forward_guard_covers_part_vii ... ok
test every_part_vii_fence_is_in_its_file ... ok
test prelude_items_outside_part_vii_are_shown_pub ... ok
test the_reverse_guard_covers_std ... ok
test every_std_behavior_block_is_in_a_fence ... ok
test every_std_item_is_in_a_fence ... ok
test the_ahead_lists_hold_only_what_still_differs ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.66s
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

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.03s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-9b01bd7200d69219)
```

`Run buckcli` - exit 101

```
assertion `left == right` failed: the Book prints the value it reads
  left: "0.8025"
 right: "0.8026"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    startup_ends_within_3_mv_of_0_8_volts

test result: FAILED. 7 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 87.53s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-eu-b/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 1.11s
     Running tests/buck.rs (/var/tmp/piperine-target-a/debug/deps/buck-e0a5ced46b7d0d71)
error: test failed, to rerun pass `-p piperine-cli --test buck`
```

`Run integrated` - exit 0

```
test threshold::the_capacitor_follows_its_closed_form ... ok
test threshold::the_counters_read_four_rises_and_four_falls ... ok
test sample_hold::the_hold_node_stays_put_while_holding ... ok
test suite::a_failed_run_panics_every_asker_with_its_message_and_an_unknown_run_is_named ... ok
test pwm_filter::the_filter_follows_its_closed_form ... ok
test sample_hold::the_hold_node_tracks_the_source ... ok
test pi_loop::every_regulated_check_passes_after_50_us ... ok
test sample_hold::the_done_strobe_is_exactly_50_ns_wide ... ok
test package::opening_every_run_leaves_only_the_package_sources ... ok
test package::every_run_compiles_with_no_warning_and_valid_shapes ... ok

test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.35s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/integrated/main.rs (/var/tmp/piperine-target-a/debug/deps/integrated-e114db84690e682b)
```

`Run cliintegrated` - exit 0

```
running 3 tests
test pi_loop_prints_its_run_line_and_10001_rows_ending_near_128 ... ok
test piperine_test_passes_pi_loop_and_threshold ... ok
test pi_loop_prints_no_failed_check_and_records_every_passing_one ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/integrated.rs (/var/tmp/piperine-target-a/debug/deps/integrated-51bf5678f957a87c)
```
