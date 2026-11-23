# composite-values phase C proofs (tasks-c.md)

**HEAD**: 8c44b34b
**Tree**: clean
**Shell**: /bin/sh
**Result**: 43 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C111 | `run srcarr / test each_element_of_an_array_of_carriers_has_its_own_events ... ok` | pass |
| C112 | `run elabarr / test an_array_of_nodes_is_one_node_per_element ... ok` | pass |
| C113 | `run srcarr / test arrays_and_elements_connect_element_to_element ... ok` | pass |
| C114 | `run typesarr / test an_array_argument_of_another_length_is_e0313_naming_both_lengths ... ok` | pass |
| C115 | `run srcarr / test an_array_of_carriers_reads_whole_as_an_array_value ... ok` | pass |
| C116 | `run srcarr / test element_history_and_events_apply_to_that_element_only ... ok` | pass |
| C117 | `run typesarr / test a_carrier_method_on_a_whole_array_is_e0211 ... ok` | pass |
| C118 | `run srcarr / test an_element_never_committed_reads_the_arrays_initial_value ... ok` | pass |
| C119 | `run srcarr / test an_element_commit_changes_that_element_only ... ok` | pass |
| C120 | `run srcarr / test a_field_or_value_element_commit_changes_those_levels_only ... ok` | pass |
| C121 | `run typesarr / test a_definition_and_commits_of_one_element_are_e0221 ... ok` | pass |
| C122 | `run srcarr / test overlapping_commits_in_one_microstep_keep_the_last_write_per_level ... ok` | pass |
| C123 | `run srcarr / test a_run_time_element_index_out_of_range_names_carrier_index_instance_and_instant ... ok` | pass |
| C124 | `run srcarr / test a_ladder_over_an_array_of_nodes_solves_as_written_out ... ok` | pass |
| C125 | `run elabarr / test an_analog_node_index_that_does_not_fold_is_refused_naming_it ... ok` | pass |
| C126 | `run storage / test peak_heap_does_not_depend_on_the_declared_length ... ok` | pass |
| C127 | `run clirun / test an_array_of_carriers_prints_by_element_and_whole_in_every_format ... ok` | pass |
| C128 | `run apiarr / test array_probes_read_an_element_and_the_whole_array ... ok` | pass |
| C129 | `run interp / test each_composite_values_form_is_refused_by_name ... ok` | pass |
| C130 | `run typesarr / test an_array_of_carriers_as_a_function_parameter_is_refused_by_name ... ok` | pass |
| C131 | `run srcarr / test an_element_given_to_an_out_port_is_driven_by_the_child ... ok` | pass |
| C132 | `run src / test an_element_never_committed_reads_its_initial_value ... ok` | pass |
| C133 | `run abi / test array_pins_validate_their_count_and_init ... ok` | pass |
| C134 | `run dsarr / test a_sparse_array_stores_only_written_elements_and_rewinds ... ok` | pass |
| C135 | `run bind / test a_window_binds_a_pin_to_elements_of_an_array_net ... ok` | pass |
| C136 | `run rt / test carrier_array_index_past_the_end_panics_naming_it ... ok` | pass |
| C137 | `run irarr / test element_forms_are_checked_against_the_array ... ok` | pass |
| C138 | `run planarr / test an_array_of_carriers_is_one_array_pin ... ok` | pass |
| C139 | `run tarr / test element_reads_and_commits_compile_and_run ... ok` | pass |
| C140 | `run elabarr / test an_array_length_read_from_a_param_makes_it_structural ... ok` | pass |
| C141 | `run docs / test gaps_drops_what_composite_values_phase_c_closed ... ok` | pass |
| C142 | `run galleries / test the_fences_composite_values_lifted_are_checked ... ok` | pass |
| C143 | `run book / test signals_of_every_family_are_refused_for_the_logic_and_the_net ... ok` | pass |
| C143 | `run book / test the_memory_bank_is_refused_for_its_generic_module ... ok` | pass |
| C144 | `cargo test -p piperine-cli --test twins` | pass |
| C145 | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| C146 | `run elabarr / test an_array_of_length_zero_is_an_empty_array ... ok` | pass |
| C147 | `run dsarr / test a_reaction_committing_an_element_a_function_writes_is_a_fault ... ok` | pass |
| C148 | `run elabarr / test a_logic_literal_is_a_carriers_initial_value ... ok` | pass |
| C149 | `run apiarr / test the_metadata_lists_the_length_of_an_array_port ... ok` | pass |
| C150 | `run apiarr / test a_set_of_a_param_an_array_length_reads_is_e0803 ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run srcarr` - exit 0

```
test a_run_time_element_index_out_of_range_names_carrier_index_instance_and_instant ... ok
test a_field_or_value_element_commit_changes_those_levels_only ... ok
test each_element_of_an_array_of_carriers_has_its_own_events ... ok
test overlapping_commits_in_one_microstep_keep_the_last_write_per_level ... ok
test an_element_given_to_an_out_port_is_driven_by_the_child ... ok
test arrays_and_elements_connect_element_to_element ... ok
test element_history_and_events_apply_to_that_element_only ... ok
test the_shift_register_and_the_ram_give_their_expected_sequences ... ok
test an_array_of_carriers_reads_whole_as_an_array_value ... ok
test an_element_commit_changes_that_element_only ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.83s

    Finished `test` profile [unoptimized] target(s) in 0.26s
     Running tests/source_carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/source_carrier_arrays-58a9ddbd6ecee6c5)
```

`Run elabarr` - exit 0

```
test node_array_contributions_unroll_to_their_nodes ... ok
test a_logic_literal_is_a_carriers_initial_value ... ok
test arrays_and_elements_bind_windows_of_child_ports ... ok
test an_array_of_nodes_is_one_node_per_element ... ok
test an_element_definition_is_one_drive ... ok
test an_analog_node_index_that_does_not_fold_is_refused_naming_it ... ok
test an_array_length_read_from_a_param_makes_it_structural ... ok
test element_and_field_commits_lower_to_their_targets ... ok
test element_reads_and_events_lower_per_element ... ok
test an_array_of_length_zero_is_an_empty_array ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.89s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/elab_carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/elab_carrier_arrays-87a09a55b4e61bea)
```

`Run typesarr` - exit 0

```
test an_initialized_array_with_an_element_on_an_out_port_is_e0223 ... ok
test an_array_of_carriers_as_a_function_parameter_is_refused_by_name ... ok
test element_reads_history_and_probes_type_per_element ... ok
test arrays_of_carriers_type_as_signals_and_ports ... ok
test a_carrier_method_on_a_whole_array_is_e0211 ... ok
test an_array_initial_value_of_another_type_is_e0210 ... ok
test an_array_argument_of_another_length_is_e0313_naming_both_lengths ... ok
test arrays_and_elements_connect_to_ports_of_their_shape ... ok
test a_definition_and_commits_of_one_element_are_e0221 ... ok
test element_and_field_targets_type_their_levels ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.25s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/types_carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/types_carrier_arrays-6c4233f4c3db21b6)
```

`Run storage` - exit 0

```
running 1 test
test peak_heap_does_not_depend_on_the_declared_length ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.55s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/carrier_array_storage.rs (/var/tmp/piperine-target-d/debug/deps/carrier_array_storage-c0fb95a0211ae76a)
```

`Run clirun` - exit 0

```
test an_option_cell_prints_none_or_the_value ... ok
test run_of_a_top_with_a_node_port_is_e0810 ... ok
test the_project_flag_uses_that_directory_and_never_searches ... ok
test composites_print_in_each_output_format ... ok
test an_array_of_carriers_prints_by_element_and_whole_in_every_format ... ok
test a_fatal_run_prints_its_msg_and_exits_1 ... ok
test records_stay_when_the_analysis_fails ... ok
test run_prints_the_fatal_line_and_exits_1 ... ok
test param_of_an_integer_from_the_command_line_reads_from_the_start ... ok
test stdout_is_the_same_from_two_caches_and_two_directories ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.97s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/run.rs (/var/tmp/piperine-target-d/debug/deps/run-3deb22af66054c5e)
```

`Run apiarr` - exit 0

```
running 5 tests
test a_name_that_is_no_element_of_the_array_is_e0801 ... ok
test the_metadata_lists_the_length_of_an_array_port ... ok
test array_probes_read_an_element_and_the_whole_array ... ok
test a_set_of_a_param_an_array_length_reads_is_e0803 ... ok
test a_node_array_prints_one_column_per_node ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.71s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/carrier_arrays-ff06cf672ae7f7a0)
```

`Run interp` - exit 0

```
test a_host_source_body_is_rejected_by_name ... ok
test a_table_expression_is_refused_by_name ... ok
test a_body_with_an_amended_form_is_rejected_by_name ... ok
test each_generics_form_is_refused_by_name ... ok
test a_digital_body_is_rejected_by_name ... ok
test the_level_nodes_are_refused_by_name ... ok
test an_analog_body_without_new_forms_still_runs_on_the_interpreter ... ok
test the_interpreter_refuses_each_analog_structure_form_by_name ... ok
test each_composite_values_form_is_refused_by_name ... ok
test the_interpreter_refuses_each_new_form_by_name ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/frozen.rs (/var/tmp/piperine-target-d/debug/deps/frozen-1902f3691d3ff8e6)
```

`Run src` - exit 0

```
test an_edge_is_a_transition ... ok
test the_buck_elaborated_from_source_holds_no_flag_and_no_merge ... ok
test maybe_is_an_ordinary_field_of_a_record ... ok
test a_finish_ends_the_run_where_its_guard_holds ... ok
test prev_is_the_initial_value_before_a_change ... ok
test control_on_a_carrier_runs_one_path ... ok
test settle_runs_as_before ... ok
test short_circuit_operands_decide_as_bools ... ok
test the_first_commit_is_a_change ... ok
test a_carrier_that_starts_high_has_not_risen ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.94s

    Finished `test` profile [unoptimized] target(s) in 0.13s
     Running tests/source_explicit.rs (/var/tmp/piperine-target-d/debug/deps/source_explicit-f3ba4bb5b2b38629)
```

`Run abi` - exit 0

```
running 2 tests
test array_pins_validate_their_count_and_init ... ok
test a_whole_read_cell_holds_an_array_view ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/array_pins.rs (/var/tmp/piperine-target-d/debug/deps/array_pins-15dc83d4d4809697)
```

`Run dsarr` - exit 0

```
running 5 tests
test an_element_window_reads_and_writes_one_element ... ok
test a_reaction_committing_an_element_a_function_writes_is_a_fault ... ok
test an_unwritten_element_reads_its_initial_value_and_previous ... ok
test element_commits_keep_the_last_write_per_level_and_transport_per_element ... ok
test a_sparse_array_stores_only_written_elements_and_rewinds ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/array_nets.rs (/var/tmp/piperine-target-d/debug/deps/array_nets-59bb83da17a50769)
```

`Run bind` - exit 0

```
running 3 tests
test a_pin_of_another_count_is_a_bind_error_naming_both_counts ... ok
test the_fixture_devices_pass_the_partials_check ... ok
test a_window_binds_a_pin_to_elements_of_an_array_net ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/array_windows.rs (/var/tmp/piperine-target-d/debug/deps/array_windows-c5e90fa36bb328e8)
```

`Run rt` - exit 0

```
running 2 tests
test rt::array::tests::element_commits_reach_the_host_with_their_element ... ok
test rt::array::tests::carrier_array_index_past_the_end_panics_naming_it ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running unittests src/lib.rs (/var/tmp/piperine-target-d/debug/deps/piperine_transpile-6d48b28f4fb0a670)
```

`Run irarr` - exit 0

```
running 5 tests
test an_element_event_names_an_element_of_an_array_carrier ... ok
test an_element_commit_in_a_loop_is_one_statement ... ok
test an_array_of_nodes_names_consecutive_body_nodes_and_round_trips ... ok
test element_forms_are_checked_against_the_array ... ok
test commit_targets_and_windows_are_checked ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/carrier_arrays-d85acdc68e342699)
```

`Run planarr` - exit 0

```
running 2 tests
test an_array_of_carriers_is_one_array_pin ... ok
test a_field_commit_is_a_statement_over_its_levels ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/carrier_arrays-aa5946c0a1ffedca)
```

`Run tarr` - exit 0

```
running 1 test
test element_reads_and_commits_compile_and_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.76s

    Finished `test` profile [unoptimized] target(s) in 0.09s
     Running tests/carrier_arrays.rs (/var/tmp/piperine-target-d/debug/deps/carrier_arrays-a2f8a6f9d3e0d79f)
```

`Run docs` - exit 0

```
test abi_md_describes_the_unreleased_contract ... ok
test the_buck_spec_holds_the_bench_bounds ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test state_records_msl_fixes ... ok
test state_holds_the_explicit_unknowns_decision ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test transient_performance_closes_v12 ... ok
test the_roadmap_is_forward_looking ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/docs.rs (/var/tmp/piperine-target-d/debug/deps/docs-93bbd657b5852db6)
```

`Run galleries` - exit 0

```
test the_fixture_table_names_only_used_models ... ok
test galleries_fail_against_an_empty_prelude ... ok
test every_gallery_fence_is_in_the_formatters_form ... ok
test a_dirty_gallery_is_reported_by_fence ... ok
test the_fences_composite_values_lifted_are_checked ... ok
test no_gallery_has_an_unresolved_member ... ok
test every_gallery_builds_with_no_diagnostics ... ok
test the_gate_builds_16_fences ... ok
test every_gallery_references_its_std_items ... ok
test every_gallery_passes_check ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.96s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/std_galleries.rs (/var/tmp/piperine-target-d/debug/deps/std_galleries-20eb711d5dacf84a)
```

`Run book` - exit 0

```
test a_bundle_is_refused_where_the_solver_reaches_it ... ok
test the_open_drain_line_is_refused_until_a_nature_can_implement_resolve ... ok
test the_book_shows_the_natures_the_standard_library_gives ... ok
test an_out_port_gives_the_initial_value ... ok
test behavior_crosses_freely_inside_one_module ... ok
test a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out ... ok
test a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere ... ok
test signals_of_every_family_are_refused_for_the_logic_and_the_net ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.71s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/book/inventory_guard.rs (/var/tmp/piperine-target-d/debug/deps/book_inventory-1a21c47b4e1a6a60)
     Running tests/book/modules.rs (/var/tmp/piperine-target-d/debug/deps/book_modules-a4ed62e653059f7f)
     Running tests/book/natures.rs (/var/tmp/piperine-target-d/debug/deps/book_natures-84da6f1f33ea55da)
```

`cargo test -p piperine-cli --test twins` - exit 0

```
test the_sweep_and_transient_twins_run_on_both_backends has been running for over 60 seconds
test the_three_op_twins_run_on_both_backends has been running for over 60 seconds
test the_two_backends_print_the_same_bytes has been running for over 60 seconds
test tran_rc_prints_the_bytes_it_printed_before_the_long_run has been running for over 60 seconds
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 61.65s

    Finished `test` profile [unoptimized] target(s) in 0.33s
     Running tests/twins.rs (/var/tmp/piperine-target-d/debug/deps/twins-9b01bd7200d69219)
```

`cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` - exit 0

```
Finished `dev` profile [unoptimized] target(s) in 0.53s
```
