# unknown-semantics proofs

**HEAD**: 2013bd1f
**Tree**: clean
**Shell**: /bin/sh
**Result**: 62 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run sourceunknowns / test an_unknown_initial_value_starts_every_level_unknown ... ok` | pass |
| C2 | `run sourceunknowns / test an_unknown_initial_value_fires_no_change_at_zero ... ok` | pass |
| C3 | `run sourceunknowns / test a_const_holding_unknown_reads_unknown_everywhere ... ok` | pass |
| C4 | `run elabunknowns / test unknown_deciding_structure_is_e0151 ... ok` | pass |
| C5 | `run sourceunknowns / test committing_unknown_makes_every_level_unknown ... ok` | pass |
| C6 | `run sourceunknowns / test level_reads_each_level_of_a_known_value ... ok` | pass |
| C7 | `run sourceunknowns / test level_with_a_known_index_is_known ... ok` | pass |
| C8 | `run elabunknowns / test width_is_the_level_count_of_each_repr_type ... ok` | pass |
| C9 | `run sourceunknowns / test level_out_of_range_is_a_panic_naming_k ... ok` | pass |
| C10 | `run sourceunknowns / test level_with_an_unknown_index_merges_every_level ... ok` | pass |
| C11 | `run sourceunknowns / test merge_of_equal_known_values_is_that_value ... ok` | pass |
| C12 | `run sourceunknowns / test merge_of_different_values_is_unknown_and_commutative ... ok` | pass |
| C13 | `run sourceunknowns / test same_compares_flags_and_values ... ok` | pass |
| C14 | `run sourceunknowns / test cast_keeps_each_level_and_its_flag ... ok` | pass |
| C15 | `run sourceunknowns / test cast_to_a_tag_with_no_variant_is_unknown ... ok` | pass |
| C16 | `run sourceunknowns / test logic_builds_known_and_unknown_values ... ok` | pass |
| C17 | `run sourceunknowns / test matches_bits_follows_its_table ... ok` | pass |
| C18 | `run typesunknowns / test cast_between_widths_is_e0701 ... ok` | pass |
| C19 | `run typesunknowns / test a_bad_pattern_is_e0702 ... ok` | pass |
| C20 | `run sourceunknowns / test an_unknown_if_runs_both_arms_without_a_fault ... ok` | pass |
| C21 | `run sourceunknowns / test both_arms_committing_commit_their_merge ... ok` | pass |
| C22 | `run sourceunknowns / test one_arm_committing_merges_with_the_value_then ... ok` | pass |
| C23 | `run sourceunknowns / test equal_known_commits_stay_known ... ok` | pass |
| C24 | `run sourceunknowns / test an_if_expression_and_an_if_statement_commit_the_same ... ok` | pass |
| C25 | `run sourceunknowns / test an_unknown_match_merges_every_arm ... ok` | pass |
| C26 | `run sourceunknowns / test a_panic_under_unknown_control_makes_the_merge_unknown ... ok` | pass |
| C27 | `run sourceunknowns / test short_circuit_operands_follow_the_merge ... ok` | pass |
| C28 | `run sourceunknowns / test an_unknown_guard_runs_the_reaction_as_a_maybe ... ok` | pass |
| C29 | `run sourceunknowns / test definitions_lets_and_functions_merge_alike ... ok` | pass |
| C30 | `run spectext / test appendix_c_states_the_loop_bound_under_unknown_control ... ok` | pass |
| C31 | `run docs / test gaps_carries_the_unknown_index_merge_to_composite_values ... ok` | pass |
| C32 | `run sourceunknowns / test an_unknown_delay_under_unknown_control_commits_unknown_next ... ok` | pass |
| C33 | `run sourceunknowns / test a_record_under_unknown_control_is_maybe ... ok` | pass |
| C34 | `run clirun / test a_maybe_record_prints_maybe_and_a_known_one_does_not ... ok` | pass |
| C35 | `run sourceunknowns / test a_maybe_finish_does_not_end_the_run ... ok` | pass |
| C36 | `run sourceunknowns / test a_maybe_check_is_judged_as_unknown ... ok` | pass |
| C37 | `run sourceunknowns / test a_record_field_keeps_its_flag ... ok` | pass |
| C38 | `run sourceunknowns / test a_math_function_of_an_unknown_value_is_unknown ... ok` | pass |
| C39 | `run sourceunknowns / test known_or_with_an_unknown_default_gives_that_default ... ok` | pass |
| C40 | `run elabunknowns / test analog_reading_a_maybe_unknown_value_is_e0150 ... ok` | pass |
| C41 | `run typesunknowns / test a_flag_total_function_may_take_a_carrier_in_analog ... ok` | pass |
| C42 | `run docs / test gaps_drops_l18_and_l19 ... ok` | pass |
| C43 | `run spectext / test appendix_c_lists_the_loop_bound_among_the_defaults ... ok` | pass |
| C44 | `run book / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C45 | `run sourceunknowns / test neither_arm_committing_commits_nothing ... ok` | pass |
| C46 | `run instant / test a_merge_commit_equal_to_the_value_fires_no_change ... ok` | pass |
| C47 | `run sourceunknowns / test merge_of_a_value_with_itself_is_that_value ... ok` | pass |
| C48 | `run sourceunknowns / test the_pfd3_reaction_keeps_state_unknown_until_a_known_commit ... ok` | pass |
| C49 | `run queue / test a_maybe_commit_converts_the_pending_it_would_remove ... ok` | pass |
| C50 | `run sourceunknowns / test delays_one_tick_apart_are_one_key ... ok` | pass |
| C51 | `run rt / test rt::control::tests::choose_runs_one_arm_or_merges_both ... ok` | pass |
| C52 | `run rt / test rt::commits::tests::a_join_merges_keys_by_carrier_and_due ... ok` | pass |
| C53 | `run reaction / test an_unknown_condition_merges_both_arms ... ok` | pass |
| C54 | `run abilayouts / test merge_keeps_equal_known_fields_and_clears_the_rest ... ok` | pass |
| C55 | `run abicaller / test a_host_without_commit_with_is_a_named_fault ... ok` | pass |
| C56 | `run irdigital / test the_unknown_nodes_type_print_and_check ... ok` | pass |
| C57 | `run interp / test the_unknown_nodes_are_refused_by_name ... ok` | pass |
| C58 | `run planshape / test every_record_layout_ends_with_maybe ... ok` | pass |
| C59 | `run records / test maybe_is_decoded_beside_the_fields ... ok` | pass |
| C60 | `run verdict / test verdict::tests::a_maybe_fatal_is_not_fatal ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-solver -p piperine-ir -p piperine-interp -p` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run sourceunknowns` - exit 0

```
test merge_of_a_value_with_itself_is_that_value ... ok
test matches_bits_follows_its_table ... ok
test merge_of_different_values_is_unknown_and_commutative ... ok
test merge_of_equal_known_values_is_that_value ... ok
test same_compares_flags_and_values ... ok
test short_circuit_operands_follow_the_merge ... ok
test two_commits_under_an_unknown_guard_merge_exactly ... ok
test neither_arm_committing_commits_nothing ... ok
test one_arm_committing_merges_with_the_value_then ... ok
test the_pfd3_reaction_keeps_state_unknown_until_a_known_commit ... ok

test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.70s

    Finished `test` profile [unoptimized] target(s) in 0.21s
     Running tests/source_unknowns.rs (/var/tmp/piperine-target-c/debug/deps/source_unknowns-4723441ecc38021e)
```

`Run elabunknowns` - exit 0

```
running 6 tests
test a_real_that_is_nan_merges_with_itself ... ok
test unknown_folds_by_the_level_rules ... ok
test width_is_the_level_count_of_each_repr_type ... ok
test unknown_deciding_structure_is_e0151 ... ok
test analog_reading_a_maybe_unknown_value_is_e0150 ... ok
test no_analog_refusal_of_an_unknown_is_p0001 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.61s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.49s
     Running tests/elab_unknowns.rs (/var/tmp/piperine-target-c/debug/deps/elab_unknowns-483f53e85698518c)
```

`Run typesunknowns` - exit 0

```
test level_is_typed_on_every_repr_type ... ok
test the_intrinsics_are_typed_at_the_concrete_type ... ok
test a_project_function_named_like_an_intrinsic_is_an_ordinary_function ... ok
test level_on_a_value_without_repr_is_refused ... ok
test cast_between_widths_is_e0701 ... ok
test width_is_typed_u32_on_a_concrete_type ... ok
test a_function_that_reads_its_parameter_bare_is_not_flag_total ... ok
test a_bad_pattern_is_e0702 ... ok
test a_flag_total_function_may_take_a_carrier_in_analog ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.80s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.49s
     Running tests/types_unknowns.rs (/var/tmp/piperine-target-c/debug/deps/types_unknowns-ab8a4e7b58f8b925)
```

`Run spectext` - exit 0

```
test s36_has_no_search_path_and_no_cycle ... ok
test time_point_runs_the_procedure ... ok
test every_error_code_is_defined_exactly_once ... ok
test tran_in_fences_is_the_module ... ok
test s3_states_the_intrinsic_contract ... ok
test zero_bytes_are_the_unknown_value ... ok
test s36_defines_the_package_codes ... ok
test every_method_called_in_a_spec_fence_is_catalogued ... ok
test no_spec_text_says_stimulus_is_host_only ... ok

test result: ok. 88 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.92s
     Running tests/spec_text.rs (/var/tmp/piperine-target-c/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run docs` - exit 0

```
test gaps_drops_what_msl_fixes_closed ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test state_records_the_cli_decision ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test gaps_drops_what_host_ii_closed ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test state_records_the_unknown_semantics_decision ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test state_records_the_derived_param_decision ... ok
test state_records_the_observation_decision ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-c/debug/deps/docs-a5e77d72b1cc2f1e)
```

`Run clirun` - exit 0

```
test a_maybe_record_prints_maybe_and_a_known_one_does_not ... ok
test records_come_before_the_table ... ok
test a_first_compile_is_announced_on_stderr ... ok
test a_completed_run_exits_0 ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test run_prints_header_records_blank_line_and_table ... ok
test records_stay_when_the_analysis_fails ... ok
test a_digital_only_transient_runs_to_its_stop_for_table_csv_and_plot ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.85s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target-c/debug/deps/run-ef7b05ca62e6bd9b)
```

`Run book` - exit 0

```
test the_chapter_shows_the_session_options ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 115.40s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target-c/debug/deps/book-f38e47a15c9a1081)
```

`Run instant` - exit 0

```
test a_merge_commit_equal_to_the_value_fires_no_change ... ok
test a_settled_change_is_one_observation ... ok
test an_instant_reports_boundary_fired_and_final ... ok
test previous_is_the_value_before_the_microstep ... ok
test a_plan_with_no_code_applies_host_commits_only ... ok
test start_at_commits_init_at_its_tick ... ok
test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok
test the_traced_instant_of_section_15_1 ... ok
test start_commits_inits_and_fires_start ... ok
test two_commits_to_one_net_settle_to_one_write ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/instant.rs (/var/tmp/piperine-target-c/debug/deps/instant-cf6184e72773bc7c)
```

`Run queue` - exit 0

```
test a_maybe_commit_converts_the_pending_it_would_remove ... ok
test a_maybe_replace_converts_every_pending_commit_of_the_statement ... ok
test a_queue_rewind_restores_its_pending_commits ... ok
test commits_apply_in_made_statement_call_order ... ok
test a_rewind_undoes_a_conversion ... ok
test a_host_commit_is_never_transported ... ok
test disjoint_byte_ranges_of_one_statement_do_not_remove_each_other ... ok
test next_due_is_the_earliest_instant ... ok
test replace_removes_every_pending_commit_of_the_statement ... ok
test transport_removes_later_commits_of_the_same_statement_only ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/queue.rs (/var/tmp/piperine-target-c/debug/deps/queue-867b2535f21de770)
```

`Run rt` - exit 0

```
test rt::real::tests::abs_has_the_derivative_of_sign ... ok
test rt::real::tests::the_differentiable_rows_agree_with_finite_differences ... ok
test rt::real::tests::the_piecewise_rows_have_derivative_zero_or_one ... ok
test rt::real::tests::dual_gives_the_interpreters_values_and_tangents ... ok
test rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result ... ok
test rt::real::tests::each_row_has_the_value_of_rusts_method ... ok
test rt::int::tests::integer_rows_wrap_saturate_and_test_overflow ... ok
test rt::int::tests::arithmetic_wraps_at_every_width ... ok

test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.03s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-interp)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 2.82s
     Running unittests src/lib.rs (/var/tmp/piperine-target-c/debug/deps/piperine_transpile-6d48b28f4fb0a670)
```

`Run reaction` - exit 0

```
running 7 tests
test reactions_and_drives_read_the_environment_as_analog_code_does ... ok
test a_known_or_over_an_unwritten_input_gives_the_default ... ok
test an_unknown_delay_commits_unknown_at_the_next_microstep ... ok
test a_reaction_commits_in_source_order ... ok
test an_unknown_condition_merges_both_arms ... ok
test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok
test level_merge_same_and_recast_compute_their_tables ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.55s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 0.57s
     Running tests/reaction.rs (/var/tmp/piperine-target-c/debug/deps/reaction-762b225443b4d4ef)
```

`Run abilayouts` - exit 0

```
running 6 tests
test a_valueless_field_with_value_bytes_is_not_canonical ... ok
test layout_equality_ignores_names ... ok
test merge_keeps_equal_known_fields_and_clears_the_rest ... ok
test merge_refuses_a_field_without_a_known_byte ... ok
test merge_works_element_by_element_over_an_array_field ... ok
test shipped_layouts_have_their_stated_fields ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/layouts.rs (/var/tmp/piperine-target-c/debug/deps/layouts-76eb62529b0e07fa)
```

`Run abicaller` - exit 0

```
running 7 tests
test a_commit_with_flags_reaches_the_owner ... ok
test a_host_without_commit_with_is_a_named_fault ... ok
test a_shared_handle_is_one_instance_destroyed_with_its_last_share ... ok
test dropping_a_handle_destroys_the_instance ... ok
test handle_builds_the_frames_and_returns_results ... ok
test handle_reports_fault_and_reshape_as_errors ... ok
test handle_is_send ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/caller.rs (/var/tmp/piperine-target-c/debug/deps/caller-7e19d0200214589c)
```

`Run irdigital` - exit 0

```
test a_defective_digital_body_gives_one_entry_per_defect ... ok
test analog_reads_a_carrier_through_level_and_same_only ... ok
test carriers_and_drives_print ... ok
test the_unknown_nodes_type_print_and_check ... ok
test unknown_values_and_their_reads_print ... ok
test events_and_reactions_print ... ok
test the_m1_prints_are_unchanged ... ok
test integers_and_their_operators_print ... ok
test the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-ir)
    Finished `test` profile [unoptimized] target(s) in 2.61s
     Running tests/digital.rs (/var/tmp/piperine-target-c/debug/deps/digital-b1250f05f42d82d3)
```

`Run interp` - exit 0

```
running 4 tests
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_unknown_nodes_are_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-interp)
    Finished `test` profile [unoptimized] target(s) in 15.59s
     Running tests/frozen.rs (/var/tmp/piperine-target-c/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run planshape` - exit 0

```
test a_delayed_drive_is_a_pin_an_event_and_a_reaction ... ok
test a_drive_is_a_discrete_function ... ok
test a_reaction_lists_its_events_reads_and_statements ... ok
test every_record_layout_ends_with_maybe ... ok
test an_if_lists_its_then_statement_before_its_else ... ok
test carriers_are_discrete_pins_with_a_known_byte ... ok
test a_reaction_on_a_flagged_guard_reads_the_guard_and_fired ... ok
test every_buck_slice_shape_validates ... ok
test a_probing_reaction_reads_the_continuous_pin ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-unknown-semantics/crates/piperine-plan)
    Finished `test` profile [unoptimized] target(s) in 1.69s
     Running tests/digital_shape.rs (/var/tmp/piperine-target-c/debug/deps/digital_shape-f16ca75f355e66eb)
```

`Run records` - exit 0

```
test a_digital_only_run_without_finish_reaches_its_stop ... ok
test maybe_is_decoded_beside_the_fields ... ok
test a_failed_require_is_one_checks_record_with_pass_false ... ok
test a_reserved_stream_keeps_its_name_and_its_site ... ok
test records_arrive_in_time_order ... ok
test a_run_ended_by_finish_gives_its_rows_to_the_instant ... ok
test an_unknown_field_is_unknown ... ok
test a_text_field_is_the_sources_literal ... ok
test an_emit_gives_one_record_with_time_instance_stream_and_fields ... ok
test keep_matches_a_stream_name_or_its_full_path ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.21s

    Finished `test` profile [unoptimized] target(s) in 0.29s
     Running tests/records.rs (/var/tmp/piperine-target-c/debug/deps/records-636b5e3d604fb760)
```

`Run verdict` - exit 0

```
test verdict::tests::a_finish_record_does_not_fail_the_verdict ... ok
test verdict::tests::a_maybe_check_fails_as_unknown ... ok
test verdict::tests::a_fatal_record_fails_the_verdict_with_its_message ... ok
test verdict::tests::a_negative_margin_does_not_fail ... ok
test verdict::tests::a_maybe_fatal_is_not_fatal ... ok
test verdict::tests::a_record_of_another_stream_never_decides ... ok
test verdict::tests::a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive ... ok
test verdict::tests::an_unknown_pass_fails ... ok
test verdict::tests::no_false_and_no_unknown_pass_is_a_pass ... ok
test verdict::tests::the_last_record_of_each_target_is_kept_in_first_seen_order ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 64 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running unittests src/lib.rs (/var/tmp/piperine-target-c/debug/deps/piperine_api-267fdcea6096b5f4)
```
