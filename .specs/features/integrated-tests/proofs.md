# integrated-tests proofs

**HEAD**: 3b8e4b0f
**Tree**: clean
**Shell**: /bin/sh
**Result**: 73 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run hygiene / test the_on_demand_targets_are_the_buck_set ... ok` | pass |
| C2 | `out=$(cargo test --no-run 2>&1) && echo "$out" / grep -q 'tests/integrated/main.rs' && ! echo "$out" / grep -E` | pass |
| C3 | `run whole / test the_buck_compiles_with_no_warning_and_valid_shapes ... ok` | pass |
| C4 | `git diff --quiet 856d2668 HEAD -- tests/buck_scenarios.rs crates/piperine-parity/tests/buck_whole.rs crates/pi` | pass |
| C5 | `run hygiene / test on_demand_targets_are_registered ... ok` | pass |
| C6 | `run hygiene / test the_on_demand_registry_names_only_on_demand_targets ... ok` | pass |
| C7 | `run hygiene / test the_buck_readme_names_every_on_demand_target ... ok` | pass |
| C8 | `run hygiene / test the_on_demand_reader_finds_test_false ... ok` | pass |
| C9 | `run integrated / test package::the_package_declares_the_five_runs_and_checks_clean ... ok` | pass |
| C10 | `run integrated / test package::every_run_compiles_with_no_warning_and_valid_shapes ... ok` | pass |
| C11 | `run integrated / test package::opening_every_run_leaves_only_the_package_sources ... ok` | pass |
| C12 | `run integrated / test package::the_runs_execute_at_once_in_their_own_build_directories ... ok` | pass |
| C13 | `run harness / test scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run ... ok` | pass |
| C14 | `run integrated / test package::every_run_reports_its_wall_time_and_rows ... ok` | pass |
| C15 | `run harness / test scenario::run::tests::a_completed_run_keeps_every_record_in_delivery_order ... ok` | pass |
| C16 | `run integrated / test threshold::the_counters_read_four_rises_and_four_falls ... ok` | pass |
| C17 | `run integrated / test threshold::the_edges_stream_holds_eight_alternating_records ... ok` | pass |
| C18 | `run integrated / test threshold::each_edge_record_is_where_the_capacitor_crosses_its_threshold ... ok` | pass |
| C19 | `run integrated / test threshold::each_edge_falls_in_its_half_period ... ok` | pass |
| C20 | `run integrated / test threshold::the_capacitor_follows_its_closed_form ... ok` | pass |
| C21 | `run integrated / test sample_hold::each_sample_commits_the_code_of_its_held_voltage ... ok` | pass |
| C22 | `run integrated / test sample_hold::the_code_appears_after_tconv_with_a_50_ns_done_strobe ... ok` | pass |
| C23 | `run integrated / test sample_hold::the_hold_node_stays_put_while_holding ... ok` | pass |
| C24 | `run integrated / test sample_hold::the_hold_node_tracks_the_source ... ok` | pass |
| C25 | `run integrated / test pwm_filter::each_period_holds_its_duty_on_the_high_and_low_sides ... ok` | pass |
| C26 | `run integrated / test pwm_filter::the_dead_time_is_two_clock_periods_on_both_edges ... ok` | pass |
| C27 | `run integrated / test pwm_filter::the_sides_never_overlap_and_every_never_check_passes ... ok` | pass |
| C28 | `run integrated / test pwm_filter::the_driven_duty_is_latched_at_the_next_wrap ... ok` | pass |
| C29 | `run integrated / test pwm_filter::the_filter_follows_its_closed_form ... ok` | pass |
| C30 | `run integrated / test state_machine::the_phase_follows_the_reference_at_every_edge ... ok` | pass |
| C31 | `run integrated / test state_machine::the_enums_print_as_their_variant_indices ... ok` | pass |
| C32 | `run integrated / test state_machine::the_capacitor_follows_its_closed_form_through_each_phase ... ok` | pass |
| C33 | `run integrated / test state_machine::the_trip_record_is_where_the_capacitor_crosses_0_91_v ... ok` | pass |
| C34 | `run integrated / test state_machine::two_edges_with_ov_enter_fault_with_the_dump_switch_on ... ok` | pass |
| C35 | `run integrated / test state_machine::the_hiccup_lasts_30_clock_periods_then_retries ... ok` | pass |
| C36 | `run integrated / test state_machine::a_clean_retry_clears_the_fault_and_resumes_the_cycle ... ok` | pass |
| C37 | `run integrated / test state_machine::one_phase_record_per_change_at_its_edge ... ok` | pass |
| C38 | `run integrated / test pi_loop::every_duty_follows_the_pi_reference ... ok` | pass |
| C39 | `run integrated / test pi_loop::the_code_stays_within_three_of_128_after_50_us ... ok` | pass |
| C40 | `run integrated / test pi_loop::every_regulated_check_passes_after_50_us ... ok` | pass |
| C41 | `run integrated / test pi_loop::one_loop_record_per_sample ... ok` | pass |
| C42 | `run integrated / test pi_loop::the_table_has_10001_rows_on_the_10_ns_grid ... ok` | pass |
| C43 | `run integrated / test pi_loop::the_duty_saturates_and_the_integrator_holds ... ok` | pass |
| C44 | `run sourcehost / test a_host_square_into_the_rc_matches_tran_square_rc ... ok` | pass |
| C45 | `grep -q "tran 0.1u 42u 0 10n uic" tests/e2e/twins/tran_square_rc.cir` | pass |
| C46 | `git diff --diff-filter=MD --quiet 856d2668 HEAD -- crates/piperine-parity/src/contract.rs crates/piperine-tran` | pass |
| C47 | `run cliintegrated / test pi_loop_prints_its_run_line_and_10001_rows_ending_near_128 ... ok` | pass |
| C48 | `run cliintegrated / test pi_loop_prints_no_failed_check_and_records_every_passing_one ... ok` | pass |
| C49 | `run cliintegrated / test piperine_test_passes_pi_loop_and_threshold ... ok` | pass |
| C50 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C51 | `run clibookbuck / test the_buck_block_is_what_the_binary_prints ... ok` | pass |
| C52 | `run clibook / test the_buck_block_shows_the_example_files ... ok` | pass |
| C53 | `run bookbuck / test the_buck_bench_starts_with_the_supervisor_off ... ok` | pass |
| C54 | `run bookcli / test the_buck_bench_of_the_chapter_is_the_example ... ok` | pass |
| C55 | `run bookguards / test every_block_of_the_book_has_one_row ... ok` | pass |
| C56 | `run bookguards / test binary_claims_name_command_line_tests ... ok` | pass |
| C57 | `run sources / test two_commits_exchange_two_carriers ... ok` | pass |
| C58 | `run sources / test never_reports_each_rising_edge ... ok` | pass |
| C59 | `run sources / test every_from_the_stdlib_fires_once_per_period ... ok` | pass |
| C60 | `! test -e crates/piperine-parity/tests/buck_commits.rs && ! test -e crates/piperine-parity/tests/buck_records.` | pass |
| C61 | `test -f crates/piperine-parity/tests/digital/time.msl && ! grep -rq "buck::" crates/piperine-parity/tests/digi` | pass |
| C62 | `run docs / test the_buck_readme_says_when_to_run_the_buck_on_demand ... ok` | pass |
| C63 | `run docs / test the_buck_readme_points_at_the_scenarios ... ok` | pass |
| C64 | `run docs / test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok` | pass |
| C66 | `run shape / test root_tests_hold_no_end_to_end_target ... ok` | pass |
| C65 | `grep -q "^### MD-77" .specs/STATE.md && grep -q "unknown-semantics-per-level" .specs/STATE.md` | pass |
| Build | `cargo build -p piperine -p piperine-parity -p piperine-api -p piperine-cli -p piperine-lang -p piperine-solver` | pass |
| Build | `cargo test --no-run -p piperine --test buck_scenarios 2>&1 / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo test --no-run -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_boun` | pass |
| Build | `cargo test --no-run -p piperine-api --test buck_long 2>&1 / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo test --no-run -p piperine-cli --test buck 2>&1 / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape` | pass |

## Output

`Run hygiene` - exit 0

```
test mod_rs_declares_only ... ok
test every_integration_target_declares_its_scope ... ok
test the_walk_covers_the_three_solver_crates ... ok
test every_ignored_doc_example_is_registered ... ok
test root_targets_match_the_naming_rule ... ok
test no_dead_architecture_identifiers ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_file_scope_lint_suppression ... ok
test no_ignored_tests ... ok
test no_disabled_test_code ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s

    Finished `test` profile [unoptimized] target(s) in 0.26s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-b/debug/deps/suite_hygiene-354b8333c6c0effe)
```

`Run whole` - exit 0

```
running 3 tests
test the_buck_compiles_with_no_warning_and_valid_shapes ... ok
test a_buck_that_is_not_enabled_stays_off_with_vin_ok_high ... ok
test no_never_record_fails_while_not_enabled ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.78s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/buck_whole.rs (/var/tmp/piperine-target-b/debug/deps/buck_whole-e6e8a004d8a3d997)
```

`Run integrated` - exit 0

```
test threshold::the_counters_read_four_rises_and_four_falls ... ok
test threshold::the_capacitor_follows_its_closed_form ... ok
test threshold::the_edges_stream_holds_eight_alternating_records ... ok
test pi_loop::every_duty_follows_the_pi_reference ... ok
test sample_hold::the_hold_node_stays_put_while_holding ... ok
test pi_loop::every_regulated_check_passes_after_50_us ... ok
test pwm_filter::the_filter_follows_its_closed_form ... ok
test sample_hold::the_hold_node_tracks_the_source ... ok
test package::opening_every_run_leaves_only_the_package_sources ... ok
test package::every_run_compiles_with_no_warning_and_valid_shapes ... ok

test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.12s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/integrated/main.rs (/var/tmp/piperine-target-b/debug/deps/integrated-e114db84690e682b)
```

`Run harness` - exit 0

```
test scenario::trace::tests::peak_to_peak_is_the_largest_minus_the_smallest ... ok
test scenario::trace::tests::settled_into_is_the_first_row_from_which_every_row_stays_inside ... ok
test scenario::trace::tests::deviation_is_the_largest_absolute_difference_with_its_time ... ok
test scenario::trace::tests::first_crossing_is_the_first_row_at_or_past_the_level_after_a_time ... ok
test scenario::trace::tests::scaled_multiplies_every_value ... ok
test scenario::stimulus::tests::a_stimulus_is_applied_to_the_session_and_its_drives_and_sets_land ... ok
test scenario::run::tests::a_completed_run_keeps_every_record_in_delivery_order ... ok
test scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run ... ok
test scenario::stimulus::tests::a_refused_action_is_returned_naming_it ... ok
test scenario::run::tests::each_run_builds_in_its_own_directory ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out; finished in 3.87s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running unittests src/lib.rs (/var/tmp/piperine-target-b/debug/deps/piperine_parity-6187a033365891a7)
```

`Run sourcehost` - exit 0

```
running 4 tests
test a_host_pwl_into_the_rc_matches_tran_pwl_rc ... ok
test a_host_driven_rc_matches_ac_rc_in_ac ... ok
test a_host_driven_rc_step_matches_tran_rc ... ok
test a_host_square_into_the_rc_matches_tran_square_rc ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.11s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/source_host.rs (/var/tmp/piperine-target-b/debug/deps/source_host-480988613ba19c2b)
```

`Run cliintegrated` - exit 0

```
running 3 tests
test pi_loop_prints_its_run_line_and_10001_rows_ending_near_128 ... ok
test piperine_test_passes_pi_loop_and_threshold ... ok
test pi_loop_prints_no_failed_check_and_records_every_passing_one ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.04s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/integrated.rs (/var/tmp/piperine-target-b/debug/deps/integrated-51bf5678f957a87c)
```

`Run clibook` - exit 0

```
test the_chapter_shows_fmt_and_says_comments_stay ... ok
test a_plot_draws_a_curve_and_a_legend_per_real_signal_and_a_lane_per_digital_one ... ok
test the_chapter_shows_the_session_options ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test the_cycle_counter_prints_three_pulses ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_clipper_runs_as_the_book_prints ... ok
test the_chapter_runs_each_new_analysis ... ok
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 44.98s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book.rs (/var/tmp/piperine-target-b/debug/deps/book-a1f18a8423c7ed6b)
```

`Run clibookbuck` - exit 0

```
running 1 test
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 100.62s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book_buck.rs (/var/tmp/piperine-target-b/debug/deps/book_buck-e2f5b58f7ea60209)
```

`Run bookbuck` - exit 0

```
running 1 test
test the_buck_bench_starts_with_the_supervisor_off ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.68s

    Finished `test` profile [unoptimized] target(s) in 0.31s
     Running tests/book/buck.rs (/var/tmp/piperine-target-b/debug/deps/book_buck-a47c6c199602e904)
```

`Run bookcli` - exit 0

```
test the_records_of_one_instant_come_in_the_order_the_solver_delivers_them ... ok
test a_value_the_design_refuses_is_e0802_and_no_analysis_runs ... ok
test the_bode_exercise_reads_0_707_and_minus_45_degrees_at_159_kilohertz ... ok
test from_trims_the_table_and_leaves_the_solver_and_the_records_alone ... ok
test the_rc_project_of_the_chapter_runs_its_operating_point_and_its_step_response ... ok
test the_options_carry_the_names_ngspice_gives_its_options_in_lowercase ... ok
test a_node_gives_its_voltage_a_digital_signal_its_value_and_an_instance_its_current ... ok
test a_nodeset_seeds_the_operating_point_of_ac_and_transient ... ok
test the_bounds_of_the_solver_have_the_defaults_and_the_keys_the_chapter_gives ... ok
test the_square_wave_rc_closes_its_orbit_over_one_period ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.99s

    Finished `test` profile [unoptimized] target(s) in 0.27s
     Running tests/book/cli.rs (/var/tmp/piperine-target-b/debug/deps/book_cli-3b04e829375302c3)
```

`Run bookguards` - exit 0

```
test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test no_block_row_is_pending ... ok
test run_rows_name_tests_that_run_their_model ... ok
test every_block_of_the_book_has_one_row ... ok
test every_gaps_row_names_its_owner ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/claims_guard.rs (/var/tmp/piperine-target-b/debug/deps/book_claims-ad679f2d8052019c)
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-b/debug/deps/book_inventory-1a21c47b4e1a6a60)
```

`Run sources` - exit 0

```

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.89s


running 3 tests
test a_reaction_reads_now_and_static_from_source ... ok
test every_from_the_stdlib_fires_once_per_period ... ok
test rise_and_fall_fire_on_edges_to_and_from_unknown ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.43s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/source_commits.rs (/var/tmp/piperine-target-b/debug/deps/source_commits-8983751bb30b7250)
     Running tests/source_records.rs (/var/tmp/piperine-target-b/debug/deps/source_records-9927e53ff9bcd459)
     Running tests/source_time.rs (/var/tmp/piperine-target-b/debug/deps/source_time-7d1a2d277816dcb9)
```

`Run docs` - exit 0

```
test the_roadmap_lists_what_the_long_run_leaves ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test the_buck_readme_points_at_the_scenarios ... ok
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test gaps_no_longer_lists_x5_l20_l39_h12 ... ok
test state_records_the_observation_decision ... ok
test state_records_the_unknown_semantics_decision ... ok
test state_records_the_derived_param_decision ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

    Finished `test` profile [unoptimized] target(s) in 0.30s
     Running tests/docs.rs (/var/tmp/piperine-target-b/debug/deps/docs-93bbd657b5852db6)
```

`Run shape` - exit 0

```
test examples_hold_no_vams_or_python ... ok
test state_records_the_solver_structure_decision ... ok
test no_manifest_names_a_retired_crate ... ok
test a_declared_main_target_accounts_for_its_own_modules_only ... ok
test instruction_files_and_state_name_the_abi_crate ... ok
test solver_manifest_has_no_in_tree_dev_dependency ... ok
test instructions_list_the_kept_crates ... ok
test default_members_are_the_green_crates ... ok
test instructions_have_no_entry_for_a_retired_crate ... ok
test host_crates_name_no_deleted_surface ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized] target(s) in 0.56s
     Running tests/workspace_shape.rs (/var/tmp/piperine-target-b/debug/deps/workspace_shape-15595651e2d2688b)
```

`out=$(cargo test --no-run 2>&1) && echo "$out" / grep -q 'tests/integrated/main.rs' && ! echo "$out" / grep -E` - exit 0

```
(no output)
```

`git diff --quiet 856d2668 HEAD -- tests/buck_scenarios.rs crates/piperine-parity/tests/buck_whole.rs crates/pi` - exit 0

```
(no output)
```

`grep -q "tran 0.1u 42u 0 10n uic" tests/e2e/twins/tran_square_rc.cir` - exit 0

```
(no output)
```

`git diff --diff-filter=MD --quiet 856d2668 HEAD -- crates/piperine-parity/src/contract.rs crates/piperine-tran` - exit 0

```
(no output)
```

`! test -e crates/piperine-parity/tests/buck_commits.rs && ! test -e crates/piperine-parity/tests/buck_records.` - exit 0

```
(no output)
```

`test -f crates/piperine-parity/tests/digital/time.msl && ! grep -rq "buck::" crates/piperine-parity/tests/digi` - exit 0

```
(no output)
```

`grep -q "^### MD-77" .specs/STATE.md && grep -q "unknown-semantics-per-level" .specs/STATE.md` - exit 0

```
(no output)
```
