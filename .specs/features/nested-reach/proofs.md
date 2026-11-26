# nested-reach proofs

**HEAD**: 1bdb0c5ff
**Tree**: clean
**Shell**: /bin/sh
**Result**: 34 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run elabnested / test a_node_is_reached_three_and_four_levels_below_the_reader ... ok` | pass |
| C2 | `run sourcenested / test a_cell_three_and_four_levels_down_through_templates_reads_its_node_lets_carrier_and_pa` | pass |
| C3 | `run sourcenested / test a_cell_of_a_generation_three_and_four_levels_down_reads_the_iteration_its_index_names ` | pass |
| C4 | `run elabnested / test a_reach_is_the_same_wherever_the_models_and_the_top_are_written ... ok` | pass |
| C5 | `run elabnested / test a_param_and_a_let_over_the_params_read_the_value_the_placement_gives_three_and_four_leve` | pass |
| C6 | `run elabnested / test a_let_over_the_solve_reaches_the_node_it_reads_three_and_four_levels_below_the_reader ..` | pass |
| C7 | `run elabnested / test a_carrier_is_reached_three_and_four_levels_below_the_reader ... ok` | pass |
| C8 | `run sourcenested / test a_cell_three_and_four_levels_down_by_name_reads_its_node_lets_carrier_and_params ... o` | pass |
| C9 | `run elabnested / test a_cycle_of_node_connections_three_and_four_levels_deep_binds_each_port ... ok` | pass |
| C10 | `run sourcenested / test a_cycle_of_node_connections_three_and_four_levels_deep_runs_as_hardware ... ok` | pass |
| C11 | `run sourcenested / test a_cycle_of_node_connections_through_a_generation_four_levels_deep_runs_as_hardware ...` | pass |
| C12 | `run typesnested / test every_kind_of_item_types_three_and_four_levels_below_the_reader ... ok` | pass |
| C13 | `run typesnested / test a_cycle_of_node_connections_three_and_four_levels_deep_types_as_hardware ... ok` | pass |
| C14 | `run typesnested / test an_item_that_is_not_pub_is_refused_three_and_four_levels_below_the_reader ... ok` | pass |
| C15 | `run elabnested / test a_missing_or_private_item_is_e0343_three_and_four_levels_below_the_reader ... ok` | pass |
| C16 | `run docs / test gaps_drops_the_reach_rows_nested_reach_closed ... ok` | pass |
| C17 | `run spectext / test section_8_2_8_reads_through_children_of_children_at_any_depth ... ok` | pass |
| C18 | `run elabhandles / test a_cycle_of_node_connections_through_any_depth_of_reach_binds_each_port ... ok` | pass |
| C19 | `run elabtemplates / test a_non_node_item_through_a_template_reads_the_concrete_module ... ok` | pass |
| C20 | `run elabnested / test a_drive_of_a_resolved_carrier_of_a_cell_three_levels_down_is_held_beside_the_cells_own .` | pass |
| C21 | `run sourcenested / test a_parent_drives_a_resolved_carrier_of_a_cell_three_levels_down ... ok` | pass |
| C22 | `run sourcenested / test a_cell_through_generic_templates_three_and_four_levels_down_reads_its_node_lets_carrie` | pass |
| C23 | `run elabnested / test a_reach_refused_for_its_cause_reports_that_cause_once ... ok` | pass |
| C24 | `run elabnested / test a_let_over_the_solve_three_and_four_levels_below_the_reader_is_elaborated_in_one_pass ..` | pass |
| C25 | `run elabnested / test every_read_whose_hops_the_text_names_is_elaborated_in_one_pass ... ok` | pass |
| C26 | `run elabnested / test a_refusal_of_another_cause_hides_no_refusal_of_a_read ... ok` | pass |
| C27 | `run elabnested / test a_let_over_the_solve_and_a_param_reads_the_value_the_placement_gives_one_three_and_four_` | pass |
| C28 | `run typesnested / test a_let_over_a_param_and_the_solve_types_one_three_and_four_levels_below_the_reader ... o` | pass |
| C29 | `run sourcenested / test a_let_over_a_param_and_the_solve_one_three_and_four_levels_down_reads_the_placements_p` | pass |
| C30 | `run elabnested / test a_given_value_over_a_param_of_the_reader_is_read_in_place_one_and_three_levels_below ...` | pass |
| C31 | `run sourcenested / test a_given_value_over_a_param_of_the_reader_one_and_three_levels_down_reads_the_cell_it_s` | pass |
| C32 | `run elabnested / test a_placement_with_a_false_where_or_a_missing_param_is_reported_once_through_a_reach ... o` | pass |
| Build | `cargo build -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |

## Output

`Run elabnested` - exit 0

```
test a_node_is_reached_three_and_four_levels_below_the_reader ... ok
test a_let_over_the_solve_reaches_the_node_it_reads_three_and_four_levels_below_the_reader ... ok
test a_param_and_a_let_over_the_params_read_the_value_the_placement_gives_three_and_four_levels_below ... ok
test a_let_over_the_solve_three_and_four_levels_below_the_reader_is_elaborated_in_one_pass ... ok
test a_reach_is_the_same_wherever_the_models_and_the_top_are_written ... ok
test a_cycle_of_node_connections_three_and_four_levels_deep_binds_each_port ... ok
test a_let_over_the_solve_and_a_param_reads_the_value_the_placement_gives_one_three_and_four_levels_below ... ok
test a_missing_or_private_item_is_e0343_three_and_four_levels_below_the_reader ... ok
test every_read_whose_hops_the_text_names_is_elaborated_in_one_pass ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.75s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.63s
     Running tests/elab_nested_reach.rs (/var/tmp/piperine-target-d/debug/deps/elab_nested_reach-118b68d694d56af1)
```

`Run sourcenested` - exit 0

```
test a_given_value_over_a_param_of_the_reader_one_and_three_levels_down_reads_the_cell_it_sets ... ok
test a_cell_through_generic_templates_three_and_four_levels_down_reads_its_node_lets_carrier_and_params ... ok
test a_cycle_of_node_connections_three_and_four_levels_deep_runs_as_hardware ... ok
test a_cell_of_a_generation_three_and_four_levels_down_reads_the_iteration_its_index_names ... ok
test a_let_over_a_param_and_the_solve_one_three_and_four_levels_down_reads_the_placements_params ... ok
test a_parent_drives_a_resolved_carrier_of_a_cell_three_levels_down ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.95s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-lang)
   Compiling piperine-project v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-project)
   Compiling piperine-api v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-api)
   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-parity)
    Finished `test` profile [unoptimized] target(s) in 25.09s
     Running tests/source_nested_reach.rs (/var/tmp/piperine-target-d/debug/deps/source_nested_reach-ed64fb613eb3f775)
```

`Run typesnested` - exit 0

```
running 4 tests
test a_cycle_of_node_connections_three_and_four_levels_deep_types_as_hardware ... ok
test an_item_that_is_not_pub_is_refused_three_and_four_levels_below_the_reader ... ok
test every_kind_of_item_types_three_and_four_levels_below_the_reader ... ok
test a_let_over_a_param_and_the_solve_types_one_three_and_four_levels_below_the_reader ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.46s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.44s
     Running tests/types_nested_reach.rs (/var/tmp/piperine-target-d/debug/deps/types_nested_reach-35fb82764faa2049)
```

`Run docs` - exit 0

```
test the_scenario_package_states_its_measured_step_count ... ok
test gaps_drops_what_generics_phase_d_closed ... ok
test transient_performance_closes_v12 ... ok
test the_roadmap_is_forward_looking ... ok
test the_instructions_name_the_integrated_gate_and_the_buck_on_demand ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_generics_check_ids_are_unique_across_its_phases ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok
test the_generics_checks_file_holds_every_phase ... ok

test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-cli v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-cli)
    Finished `test` profile [unoptimized] target(s) in 1.14s
     Running tests/docs.rs (/var/tmp/piperine-target-d/debug/deps/docs-8e80bcc00dcc90ee)
```

`Run spectext` - exit 0

```
test the_step_that_reads_a_jump_is_first_order ... ok
test tran_in_fences_is_the_module ... ok
test the_library_declares_no_net_nature ... ok
test zero_bytes_are_the_default_value ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok

test result: ok. 149 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 1.09s
     Running tests/spec_text.rs (/var/tmp/piperine-target-d/debug/deps/spec_text-fc077905224603d9)
```

`Run elabhandles` - exit 0

```
test a_placement_binds_the_node_of_a_child_written_after_it ... ok
test an_access_a_let_bound_nodes_nature_lacks_is_e0206 ... ok
test a_recursion_through_a_handle_function_is_e0126 ... ok
test a_node_of_a_child_connects_to_another_placement ... ok
test parentheses_around_a_node_argument_lower_to_the_node ... ok
test a_cycle_of_node_connections_binds_each_port_to_the_node_it_names ... ok
test a_node_chosen_through_lets_and_arm_statements_folds_to_the_node ... ok
test a_choice_whose_condition_assigns_folds_and_ends ... ok
test a_let_assigned_only_values_of_elaboration_folds_to_the_node_it_holds_there ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.16s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.59s
     Running tests/elab_handles.rs (/var/tmp/piperine-target-d/debug/deps/elab_handles-62e11896d25669a5)
```

`Run elabtemplates` - exit 0

```
test a_bare_module_name_used_as_a_value_places_its_template_with_every_default ... ok
test a_template_carrying_composite_params_places_with_and_without_a_result ... ok
test two_templates_with_equal_composite_params_are_one_value ... ok
test the_trait_of_a_combined_bound_dispatches_to_the_template_modules_impl ... ok
test a_param_declared_as_an_array_of_templates_runs_by_default_and_by_override ... ok
test a_template_placement_is_identical_to_the_written_placement ... ok
test templates_fold_from_consts_functions_and_two_levels_of_params ... ok
test an_array_of_templates_written_as_a_repeat_is_that_many_equal_templates ... ok
test every_callee_form_elaborates_to_what_it_names ... ok

test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.49s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-reach/crates/piperine-lang)
    Finished `test` profile [unoptimized] target(s) in 0.63s
     Running tests/elab_templates.rs (/var/tmp/piperine-target-d/debug/deps/elab_templates-48f7175a2550e82b)
```
