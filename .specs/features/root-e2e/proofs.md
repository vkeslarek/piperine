# root-e2e proofs

**HEAD**: 474eeccf
**Tree**: clean
**Shell**: /bin/sh
**Result**: 38 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run inventory / test every_block_of_the_book_has_one_row ... ok` | pass |
| C2 | `run inventory / test every_row_names_an_existing_test ... ok` | pass |
| C3 | `run inventory / test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok` | pass |
| C4 | `run inventory / test run_rows_name_tests_that_run_their_model ... ok` | pass |
| C5 | `run welcome / test the_rc_step_charges_as_the_book_says ... ok` | pass |
| C6 | `run inventory / test refused_rows_name_tests_that_expect_their_refusal ... ok` | pass |
| C7 | `run inventory / test refused_rows_and_gaps_section_11_agree ... ok` | pass |
| C8 | `run errors / test a_refusal_that_runs_fails_naming_its_row ... ok` | pass |
| C9 | `run claims / test every_value_sentence_has_a_claim_row ... ok` | pass |
| C10 | `run claims / test holding_claims_name_root_tests_that_cite_them ... ok` | pass |
| C11 | `run claims / test every_claim_row_is_a_sentence_of_its_chapter ... ok` | pass |
| C12 | `run claims / test the_detector_flags_a_value_a_code_and_an_outcome ... ok` | pass |
| C13 | `run corpus / test every_twin_has_its_files_in_one_scenario_directory ... ok` | pass |
| C14 | `run corpus / test tests_ngspice_holds_no_twin ... ok` | pass |
| C15 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| C16 | `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity` | pass |
| C17 | `git diff --quiet $(git merge-base HEAD origin/main) -- crates/piperine-parity/src/contract.rs crates/piperine-` | pass |
| C18 | `run scenarios / test every_twin_has_a_scenario_test ... ok` | pass |
| C19 | `run scenarios / test divider ... ok` | pass |
| C20 | `run scenarios / test tran_rc ... ok` | pass |
| C21 | `run scenarios / SKIP divider: ngspice comparison disabled by PIPERINE_NGSPICE=skip` | pass |
| C22 | `run scenarios / test each_scenario_runs_in_a_directory_of_its_own ... ok` | pass |
| C23 | `run inventory / test a_model_across_blocks_names_one_test ... ok` | pass |
| C24 | `run clibook / test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok` | pass |
| C25 | `run corpus / test a_scenario_with_benches_holds_a_manifest ... ok` | pass |
| C26 | `run inventory / test no_block_row_is_pending ... ok` | pass |
| C27 | `run claims / test no_claim_row_is_pending ... ok` | pass |
| C28 | `run hygiene / test the_root_manifest_names_its_test_targets ... ok` | pass |
| C29 | `run hygiene / test root_targets_match_the_naming_rule ... ok` | pass |
| C30 | `run docs / test gaps_drops_what_root_e2e_closed ... ok` | pass |
| C31 | `run docs / test the_roadmap_and_state_record_root_e2e ... ok` | pass |
| C33 | `run inventory / test every_gaps_row_names_its_owner ... ok` | pass |
| C34 | `run inventory / test disputed_rows_are_listed_for_the_user ... ok` | pass |
| C35 | `run inventory / test no_row_diverges ... ok` | pass |
| C32 | `run paritylib / test corpus::tests::a_name_in_two_scenarios_is_refused ... ok` | pass |
| Build | `cargo build -p piperine -p piperine-parity -p piperine-cli -p piperine-api --all-targets 2>&1 / tee /dev/stder` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape` | pass |

## Output

`Run inventory` - exit 0

```
test the_gaps_guard_names_a_missing_and_an_extra_row ... ok
test the_owner_and_disputed_rules_name_what_they_break ... ok
test the_guard_names_a_block_without_a_row_and_a_row_naming_no_test ... ok
test refused_rows_name_tests_that_expect_their_refusal ... ok
test every_gaps_row_names_its_owner ... ok
test every_block_of_the_book_has_one_row ... ok
test run_rows_name_tests_that_run_their_model ... ok
test refused_rows_and_gaps_section_11_agree ... ok
test every_row_names_an_existing_test ... ok
test the_book_reads_every_block ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_inventory-fa5abaa513c18484)
```

`Run welcome` - exit 0

```
test e0319_a_param_with_no_default_must_be_bound ... ok
test e0312_a_port_must_be_connected ... ok
test si_suffixes_are_f64_and_a_plain_number_is_an_integer ... ok
test the_inverter_drives_its_output_a_propagation_delay_after_its_input ... ok
test a_monitor_that_checks_every_edge_writes_one_record_per_edge ... ok
test the_delayed_bench_steps_at_one_microsecond ... ok
test the_rc_step_charges_as_the_book_says ... ok
test the_rc_project_holds_its_operating_point_and_its_netlist ... ok
test the_status_paragraph_lists_what_is_still_refused_and_one_item_runs ... ok
test adding_or_removing_an_emit_gives_the_same_waveforms ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.44s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/welcome.rs (/var/tmp/piperine-target-a/debug/deps/book_welcome-fa174e4ce53492c0)
```

`Run errors` - exit 0

```
test without_an_initial_value_the_output_sits_at_zero_volts ... ok
test the_three_fixes_make_the_worked_example_elaborate ... ok
test a_reaction_on_a_changed_unknown_emits_the_inputs ... ok
test a_refusal_that_runs_fails_naming_its_row ... ok
test e0501_is_microsteps_that_never_go_quiet ... ok
test e0507_is_a_non_finite_value_in_a_solve ... ok
test e0508_is_a_transient_step_that_cannot_be_completed_at_the_smallest_step ... ok
test e0504_is_a_step_after_a_digital_change_that_does_not_converge ... ok
test e0505_is_an_operating_point_that_does_not_converge ... ok
test e0503_is_a_singular_solve_that_names_the_node_or_the_branches ... ok

test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/errors.rs (/var/tmp/piperine-target-a/debug/deps/book_errors-904a2d02de7e4e4b)
```

`Run claims` - exit 0

```
running 7 tests
test the_detector_flags_a_value_a_code_and_an_outcome ... ok
test the_claim_guard_names_a_sentence_without_a_row ... ok
test no_claim_row_is_pending ... ok
test binary_claims_name_command_line_tests ... ok
test holding_claims_name_root_tests_that_cite_them ... ok
test every_value_sentence_has_a_claim_row ... ok
test every_claim_row_is_a_sentence_of_its_chapter ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.68s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/book/claims_guard.rs (/var/tmp/piperine-target-a/debug/deps/book_claims-4e3735230b24aa8e)
```

`Run corpus` - exit 0

```
running 8 tests
test a_literal_tolerance_in_a_parity_target_is_reported ... ok
test tests_ngspice_holds_no_twin ... ok
test the_guards_report_planted_text ... ok
test no_parity_target_compares_against_a_literal ... ok
test the_layout_guards_report_planted_trees ... ok
test a_scenario_with_benches_holds_a_manifest ... ok
test every_twin_has_its_files_in_one_scenario_directory ... ok
test every_twin_has_a_cir_and_a_test ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-a/debug/deps/parity_corpus-0b598b4d30604325)
```

`Run twins` - exit 0

```
running 6 tests
test the_two_backends_print_the_same_bytes ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.37s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-a/debug/deps/twins-9b01bd7200d69219)
```

`Run scenarios` - exit 0

```
test tran_corner ... ok
tran_floor on rust: 1 points through piperine::, largest deviation from ngspice 0e0
test tran_floor ... ok
tran_jump on rust: 20 points through piperine::, largest deviation from ngspice 0e0
test tran_jump ... ok
test a_disabled_ngspice_still_runs_the_scenario ... ok
tran_rc on rust: 100 points through piperine::, largest deviation from ngspice 8.60249613218178e-5
test tran_rc ... ok
tran_dq on rust: 100 points through piperine::, largest deviation from ngspice 6.100109293155631e-5
test tran_dq ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.38s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/scenarios.rs (/var/tmp/piperine-target-a/debug/deps/scenarios-76b3ed7de830fcb3)
```

`Run clibook` - exit 0

```
test the_cycle_counter_prints_three_pulses ... ok
test blinky_runs_to_its_stop_in_the_book ... ok
test the_plot_in_the_chapter_is_the_file_the_binary_writes ... ok
test the_chapter_runs_each_new_analysis ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints has been running for over 60 seconds
test the_buck_block_is_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints has been running for over 60 seconds
test the_rc_output_blocks_are_what_the_binary_prints ... ok
test the_buck_block_is_what_the_binary_prints ... ok
test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 135.34s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/book.rs (/var/tmp/piperine-target-a/debug/deps/book-a1f18a8423c7ed6b)
```

`Run hygiene` - exit 0

```
test every_integration_target_declares_its_scope ... ok
test every_ignored_doc_example_is_registered ... ok
test the_walk_covers_the_three_solver_crates ... ok
test mod_rs_declares_only ... ok
test root_targets_match_the_naming_rule ... ok
test no_file_scope_lint_suppression ... ok
test no_dead_architecture_identifiers ... ok
test no_ignored_tests ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_disabled_test_code ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target-a/debug/deps/suite_hygiene-354b8333c6c0effe)
```

`Run docs` - exit 0

```
test state_records_the_observation_decision ... ok
test gaps_drops_what_root_e2e_closed ... ok
test state_records_the_derived_param_decision ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test the_roadmap_lists_what_the_long_run_leaves ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test the_roadmap_lists_the_host_follow_ups ... ok
test gaps_drops_what_closed_and_lists_what_is_left ... ok
test the_roadmap_and_state_record_root_e2e ... ok
test gaps_drops_what_msl_fixes_closed ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```

`Run paritylib` - exit 0

```
test golden::tests::an_unparsable_row_names_file_and_row ... ok
test corpus::tests::a_name_in_two_scenarios_is_refused - should panic ... ok
test golden::tests::the_print_precision_is_read_from_the_token ... ok
test golden::tests::an_export_is_found_with_or_without_data_suffix ... ok
test ngspice::tests::skip_passes_and_prints_one_line ... ok
test golden::tests::a_grid_mismatch_names_the_point ... ok
test waveform::tests::a_time_outside_the_result_is_none ... ok
test ngspice::tests::print_lines_parse_to_named_values ... ok
test ngspice::tests::a_missing_binary_fails_naming_the_opt_out ... ok
test ngspice::tests::the_binary_comes_from_the_variable_or_the_path ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running unittests src/lib.rs (/var/tmp/piperine-target-a/debug/deps/piperine_parity-6187a033365891a7)
```

`env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity` - exit 0

```

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.82s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_ac.rs (/var/tmp/piperine-target-a/debug/deps/parity_ac-ddd91a46c3793786)
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-a/debug/deps/parity_corpus-0b598b4d30604325)
     Running tests/parity_dc.rs (/var/tmp/piperine-target-a/debug/deps/parity_dc-f35f75664572bfd4)
     Running tests/parity_grid.rs (/var/tmp/piperine-target-a/debug/deps/parity_grid-034d95c031cae4b1)
     Running tests/parity_op.rs (/var/tmp/piperine-target-a/debug/deps/parity_op-eac8afb1dbfbcbd7)
     Running tests/parity_pss.rs (/var/tmp/piperine-target-a/debug/deps/parity_pss-3f2db937cd0e368c)
     Running tests/parity_pz.rs (/var/tmp/piperine-target-a/debug/deps/parity_pz-b671b174110ec6b1)
     Running tests/parity_sens.rs (/var/tmp/piperine-target-a/debug/deps/parity_sens-8d1dc96466c54e68)
     Running tests/parity_temp.rs (/var/tmp/piperine-target-a/debug/deps/parity_temp-cb6d58e99dd4bedc)
     Running tests/parity_tf.rs (/var/tmp/piperine-target-a/debug/deps/parity_tf-bf5979a0fb72f244)
     Running tests/parity_tran.rs (/var/tmp/piperine-target-a/debug/deps/parity_tran-1f68c93d92106e3e)
```

`git diff --quiet $(git merge-base HEAD origin/main) -- crates/piperine-parity/src/contract.rs crates/piperine-` - exit 0

```
(no output)
```
