# msl-ir proofs

**HEAD**: b631738a
**Tree**: clean
**Shell**: /bin/sh
**Result**: 41 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run model / test ids_are_u32_newtypes_one_per_arena ... ok` | pass |
| C2 | `run model / test a_finished_ir_has_no_mutating_operation ... ok` | pass |
| C3 | `run model / test equal_builder_sequences_give_equal_irs ... ok` | pass |
| C4 | `run model / test ir_is_send_and_sync ... ok` | pass |
| C5 | `run model / test handles_are_copy ... ok` | pass |
| C6 | `run model / test contained_items_come_in_declaration_order ... ok` | pass |
| C7 | `run model / test an_instance_reaches_its_parent_and_body ... ok` | pass |
| C8 | `run model / test path_lookup_finds_instances_and_nodes ... ok` | pass |
| C9 | `run model / test value_code_of_the_subset_is_representable ... ok` | pass |
| C10 | `run model / test a_body_holds_params_nodes_branches_and_sites ... ok` | pass |
| C11 | `run model / test a_probe_spans_two_terminals ... ok` | pass |
| C12 | `run model / test environment_reads_are_nodes ... ok` | pass |
| C13 | `run model / test limit_and_stim_sites_are_indexed ... ok` | pass |
| C14 | `run model / test a_contribution_is_addends_direct_or_ddt ... ok` | pass |
| C15 | `run model / test a_nature_carries_abstol_and_units ... ok` | pass |
| C16 | `run model / test every_expression_records_its_type ... ok` | pass |
| C17 | `run model / test one_body_serves_every_instance_of_it ... ok` | pass |
| C18 | `run model / test instances_are_stored_in_pre_order ... ok` | pass |
| C19 | `run model / test the_node_table_names_nature_declarer_and_init ... ok` | pass |
| C20 | `run model / test the_authored_tree_is_walkable ... ok` | pass |
| C21 | `run serde / test storage_types_implement_serde ... ok` | pass |
| C22 | `run serde / test json_round_trip_gives_an_equal_ir ... ok` | pass |
| C23 | `run serde / test text_that_is_no_ir_fails_to_deserialize ... ok` | pass |
| C24 | `run print / test printed_text_shows_every_item ... ok` | pass |
| C25 | `run print / test printing_twice_is_byte_identical ... ok` | pass |
| C26 | `run print / test no_parser_of_the_text_exists ... ok` | pass |
| C27 | `run print / test unequal_irs_print_different_texts ... ok` | pass |
| C28 | `run check / test a_built_ir_checks_clean ... ok` | pass |
| C29 | `run check / test an_out_of_range_id_is_named ... ok` | pass |
| C30 | `run check / test an_instance_with_wrong_arity_is_named ... ok` | pass |
| C31 | `run check / test a_misplaced_or_duplicated_site_is_named ... ok` | pass |
| C32 | `run check / test a_wrong_type_in_an_analog_position_is_named ... ok` | pass |
| C33 | `run deps / test dependencies_follow_locals_and_calls ... ok` | pass |
| C34 | `run deps / test dependency_sets_have_a_fixed_order ... ok` | pass |
| C35 | `run deps / test affine_means_probes_meet_only_linear_operators ... ok` | pass |
| C36 | `run check / test a_top_alone_prints_and_checks ... ok` | pass |
| C37 | `run model / test a_body_without_analog_has_no_branches ... ok` | pass |
| C38 | `run check / test a_duplicated_path_is_named ... ok` | pass |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `test ! -d crates/piperine-ir // cargo build -p piperine-ir --all-targets 2>&1 / tee /dev/stderr / (! grep -qE ` | pass |

## Output

`Run model` - exit 0

```
test instances_are_stored_in_pre_order ... ok
test the_authored_tree_is_walkable ... ok
test one_body_serves_every_instance_of_it ... ok
test path_lookup_finds_instances_and_nodes ... ok
test limit_and_stim_sites_are_indexed ... ok
test the_node_table_names_nature_declarer_and_init ... ok
test a_finished_ir_has_no_mutating_operation ... ok
test value_code_of_the_subset_is_representable ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Blocking waiting for file lock on build directory
   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-ir/crates/piperine-ir)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 6.17s
     Running tests/model.rs (/var/tmp/piperine-target/debug/deps/model-37b4b500b758490e)
```

`Run serde` - exit 0

```
running 4 tests
test storage_types_implement_serde ... ok
test text_that_is_no_ir_fails_to_deserialize ... ok
test reals_survive_bit_for_bit ... ok
test json_round_trip_gives_an_equal_ir ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Blocking waiting for file lock on build directory
   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-ir/crates/piperine-ir)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 26.75s
     Running tests/serde.rs (/var/tmp/piperine-target/debug/deps/serde-ac60f5b7bbf7b790)
```

`Run print` - exit 0

```
running 4 tests
test no_parser_of_the_text_exists ... ok
test printed_text_shows_every_item ... ok
test printing_twice_is_byte_identical ... ok
test unequal_irs_print_different_texts ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-ir/crates/piperine-ir)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
     Running tests/print.rs (/var/tmp/piperine-target/debug/deps/print-15cc330508aed8f3)
```

`Run check` - exit 0

```
running 8 tests
test a_top_alone_prints_and_checks ... ok
test a_built_ir_checks_clean ... ok
test a_duplicated_path_is_named ... ok
test an_instance_with_wrong_arity_is_named ... ok
test a_cyclic_expression_is_named_without_hanging ... ok
test a_wrong_type_in_an_analog_position_is_named ... ok
test an_out_of_range_id_is_named ... ok
test a_misplaced_or_duplicated_site_is_named ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-ir/crates/piperine-ir)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.24s
     Running tests/check.rs (/var/tmp/piperine-target/debug/deps/check-74c101b8702f60de)
```

`Run deps` - exit 0

```
running 3 tests
test dependency_sets_have_a_fixed_order ... ok
test affine_means_probes_meet_only_linear_operators ... ok
test dependencies_follow_locals_and_calls ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-msl-ir/crates/piperine-ir)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.18s
     Running tests/deps.rs (/var/tmp/piperine-target/debug/deps/deps-0fbf51e077f2aa9a)
```
