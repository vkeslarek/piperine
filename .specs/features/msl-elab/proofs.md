# msl-elab proofs

**HEAD**: d7e027ea
**Tree**: clean
**Shell**: /bin/sh
**Result**: 42 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run entry / test a_twin_elaborates_to_a_checked_ir ... ok` | pass |
| C2 | `run entry / test the_design_is_unchanged_by_elaboration ... ok` | pass |
| C3 | `run entry / test front_end_diagnostics_are_returned_without_an_ir ... ok` | pass |
| C4 | `run entry / test an_unknown_or_ported_top_is_one_diagnostic ... ok` | pass |
| C5 | `run entry / test elaborating_twice_prints_the_same_text ... ok` | pass |
| C6 | `run params / test an_unset_param_takes_its_default ... ok` | pass |
| C7 | `run params / test a_required_param_left_out_is_e0319 ... ok` | pass |
| C8 | `run params / test a_false_where_is_e0317_with_the_path ... ok` | pass |
| C9 | `run params / test a_bound_value_that_is_not_constant_is_refused ... ok` | pass |
| C10 | `run params / test a_const_read_is_folded_once ... ok` | pass |
| C11 | `run params / test a_param_read_is_never_folded ... ok` | pass |
| C12 | `run params / test a_where_is_recorded_over_the_body_params ... ok` | pass |
| C13 | `run instances / test one_body_per_module_one_instance_per_placement ... ok` | pass |
| C14 | `run instances / test instances_come_in_pre_order ... ok` | pass |
| C15 | `run instances / test paths_start_at_the_top_module ... ok` | pass |
| C16 | `run instances / test signals_are_nodes_and_ports_bind_the_parents ... ok` | pass |
| C17 | `run instances / test a_node_init_is_recorded_on_body_and_design ... ok` | pass |
| C18 | `run instances / test a_node_init_that_reads_a_param_is_p0001 ... ok` | pass |
| C19 | `run instances / test an_unnamed_placement_is_module_hash_k ... ok` | pass |
| C20 | `run instances / test a_module_that_places_itself_is_e0341 ... ok` | pass |
| C20 | `run instances / test a_module_that_places_itself_through_another_is_e0341 ... ok` | pass |
| C21 | `run analog / test value_code_lowers_to_the_same_forms_and_functions_once ... ok` | pass |
| C22 | `run analog / test probes_and_environment_lower_to_their_nodes ... ok` | pass |
| C23 | `run analog / test a_branch_is_one_unordered_pair_with_its_kind ... ok` | pass |
| C24 | `run analog / test a_sum_splits_into_direct_and_ddt_addends ... ok` | pass |
| C25 | `run analog / test a_reversed_pair_negates_its_addends ... ok` | pass |
| C26 | `run analog / test sites_are_numbered_in_source_order_and_named ... ok` | pass |
| C27 | `run analog / test a_branch_from_a_node_to_itself_is_e0241 ... ok` | pass |
| C28 | `run analog / test potential_and_flow_on_one_branch_is_e0415 ... ok` | pass |
| C29 | `run unlowered / test a_site_or_contribution_under_if_is_p0001 ... ok` | pass |
| C30 | `run unlowered / test ddt_off_the_top_of_a_flow_contribution_is_p0001 ... ok` | pass |
| C31 | `run unlowered / test m_dtemp_or_extern_at_a_placement_is_p0001 ... ok` | pass |
| C32 | `run unlowered / test a_digital_block_is_p0001 ... ok` | pass |
| C33 | `run unlowered / test a_diagnostic_means_no_ir ... ok` | pass |
| C34 | `run entry / test every_diagnostic_has_code_span_and_path ... ok` | pass |
| C35 | `run entry / test the_twins_elaborate_without_diagnostics ... ok` | pass |
| C36 | `run instances / test an_empty_module_is_an_empty_body ... ok` | pass |
| C37 | `run instances / test a_module_under_two_parents_is_one_body ... ok` | pass |
| C38 | `run instances / test an_unconnected_node_stays_in_the_table ... ok` | pass |
| C39 | `run analog / test a_single_addend_is_one_direct_addend ... ok` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `test ! -d crates/piperine-ir // cargo build -p piperine-ir --all-targets 2>&1 / tee /dev/stderr / (! grep -qE ` | pass |

## Output

`Run entry` - exit 0

```
test an_empty_top_is_one_body_and_one_instance ... ok
test an_unknown_or_ported_top_is_one_diagnostic ... ok
test front_end_diagnostics_are_returned_without_an_ir ... ok
test every_diagnostic_has_code_span_and_path ... ok
test the_design_is_unchanged_by_elaboration ... ok
test the_twins_elaborate_without_diagnostics ... ok
test elaborating_twice_prints_the_same_text ... ok
test a_twin_elaborates_to_a_checked_ir ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-elab/crates/piperine-ir)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-elab/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.16s
     Running tests/elab_entry.rs (/var/tmp/piperine-target/debug/deps/elab_entry-4cb1a4f29eda6799)
```

`Run params` - exit 0

```
test a_param_read_is_never_folded ... ok
test a_const_read_is_folded_once ... ok
test a_bound_value_that_is_not_constant_is_refused ... ok
test functions_methods_and_logic_fold ... ok
test division_by_zero_is_inf_not_an_error ... ok
test a_where_is_recorded_over_the_body_params ... ok
test an_unset_param_takes_its_default ... ok
test a_required_param_left_out_is_e0319 ... ok
test a_false_where_is_e0317_with_the_path ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-elab/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.44s
     Running tests/elab_params.rs (/var/tmp/piperine-target/debug/deps/elab_params-03aa0cdb7ccbf522)
```

`Run instances` - exit 0

```
test a_node_init_is_recorded_on_body_and_design ... ok
test a_node_init_that_reads_a_param_is_p0001 ... ok
test an_empty_module_is_an_empty_body ... ok
test a_module_that_places_itself_is_e0341 ... ok
test an_unnamed_placement_is_module_hash_k ... ok
test a_module_under_two_parents_is_one_body ... ok
test paths_start_at_the_top_module ... ok
test one_body_per_module_one_instance_per_placement ... ok
test signals_are_nodes_and_ports_bind_the_parents ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-elab/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
     Running tests/elab_instances.rs (/var/tmp/piperine-target/debug/deps/elab_instances-a4b15ad94c9e8401)
```

`Run analog` - exit 0

```
test sites_are_numbered_in_source_order_and_named ... ok
test probes_and_environment_lower_to_their_nodes ... ok
test a_lowered_expression_carries_its_type_and_the_origin_of_its_construct ... ok
test the_device_bodies_equal_the_structural_goldens ... ok
test a_sum_splits_into_direct_and_ddt_addends ... ok
test a_site_under_an_if_is_refused_not_mis_lowered ... ok
test value_code_lowers_to_the_same_forms_and_functions_once ... ok
test a_branch_from_a_node_to_itself_is_e0241 ... ok
test a_ddt_off_the_top_of_a_flow_contribution_is_refused ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-elab/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running tests/elab_analog.rs (/var/tmp/piperine-target/debug/deps/elab_analog-4602fd91a315722d)
```

`Run unlowered` - exit 0

```
running 6 tests
test a_digital_block_is_p0001 ... ok
test a_diagnostic_means_no_ir ... ok
test a_site_on_the_left_of_a_short_circuit_still_lowers ... ok
test m_dtemp_or_extern_at_a_placement_is_p0001 ... ok
test ddt_off_the_top_of_a_flow_contribution_is_p0001 ... ok
test a_site_or_contribution_under_if_is_p0001 ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s

   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-msl-elab/crates/piperine-lang)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.36s
     Running tests/elab_unlowered.rs (/var/tmp/piperine-target/debug/deps/elab_unlowered-82eb98fae575c8e9)
```
