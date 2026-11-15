# solver-abi-core proofs

**HEAD**: 7605cf25
**Tree**: clean
**Shell**: /bin/sh
**Result**: 54 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run abi / test struct_layouts_match_the_abi ... ok` | pass |
| C2 | `run abi / test list_reads_by_the_producers_stride ... ok` | pass |
| C3 | `run abi / test crate_declares_no_dependency ... ok` | pass |
| C4 | `run abi / test shipped_layouts_have_their_stated_fields ... ok` | pass |
| C5 | `run abi / test layout_equality_ignores_names ... ok` | pass |
| C6 | `run abi / test a_valueless_field_with_value_bytes_is_not_canonical ... ok` | pass |
| C7 | `run abi / test resistor_shape_reads_back_through_raw_pointers ... ok` | pass |
| C8 | `run abi / test every_shape_local_rule_is_enforced ... ok` | pass |
| C9 | `run abi / test module_pointers_call_the_device ... ok` | pass |
| C10 | `run abi / test destroy_drops_the_device_and_releases_the_shape ... ok` | pass |
| C11 | `run abi / test a_panic_is_a_fault_with_its_message ... ok` | pass |
| C12 | `run abi / test a_fault_carries_its_code_and_a_cut_message ... ok` | pass |
| C13 | `run abi / test typed_access_that_does_not_fit_is_none ... ok` | pass |
| C14 | `run abi / test commit_reaches_the_host_once ... ok` | pass |
| C15 | `run abi / test finish_is_a_status ... ok` | pass |
| C16 | `run abi / test handle_builds_the_frames_and_returns_results ... ok` | pass |
| C17 | `run abi / test exact_partials_verify ... ok` | pass |
| C18 | `run abi / test a_wrong_partial_is_reported_with_its_index ... ok` | pass |
| C19 | `run abi / test a_point_where_the_limit_is_active_is_skipped ... ok` | pass |
| C20 | `run abi / test worked_examples_produce_their_stated_values ... ok` | pass |
| C21 | `run abi / test worked_examples_pass_validation ... ok` | pass |
| C22 | `run guards / test sub_solvers_depend_on_the_abi_crate_only ... ok` | pass |
| C23 | `run guards / test abi_crate_is_a_leaf ... ok` | pass |
| C24 | `run guards / test instruction_files_and_state_name_the_abi_crate ... ok` | pass |
| C25 | `run owner / test the_protocol_module_names_no_sub_solver_item ... ok` | pass |
| C26 | `run owner / test no_landing_means_one_attempt_to_the_proposed_time ... ok` | pass |
| C27 | `run owner / test a_landing_is_solved_on_the_left_value_then_its_instant_runs ... ok` | pass |
| C28 | `run owner / test a_rejected_attempt_restores_both_sides_and_replays ... ok` | pass |
| C29 | `run owner / test acceptance_fires_crossings_runs_the_instant_and_asks_for_restart ... ok` | pass |
| C30 | `run owner / test a_host_param_write_asks_for_the_small_restart ... ok` | pass |
| C31 | `run owner / test an_irreversible_landing_runs_once_after_acceptance_and_does_not_settle ... ok` | pass |
| C32 | `run owner / test finish_ends_after_its_instant_and_is_dropped_on_an_earlier_landing ... ok` | pass |
| C33 | `run owner / test no_unknowns_means_no_attempt ... ok` | pass |
| C34 | `run owner / test the_operating_point_follows_the_call_sequence ... ok` | pass |
| C35 | `run owner / test rounds_beyond_the_bound_are_e0506_naming_the_nets ... ok` | pass |
| C36 | `run owner / test observations_reach_the_sink_after_acceptance_in_instant_order ... ok` | pass |
| C37 | `run owner / test an_undone_instant_delivers_nothing ... ok` | pass |
| C38 | `run owner / test a_host_waiter_runs_once_and_its_writes_are_due_next_microstep ... ok` | pass |
| C39 | `run owner / test a_write_is_committed_only_after_the_shadow_accepts ... ok` | pass |
| C40 | `run abi / test a_non_finite_partial_is_a_mismatch ... ok` | pass |
| C41 | `run abi / test a_list_whose_stride_is_below_its_element_is_refused ... ok` | pass |
| C42 | `run abi / test raw_views_have_no_public_pointer_field ... ok` | pass |
| C43 | `run abi / test no_unsafe_outside_raw_thunk_and_caller ... ok` | pass |
| C44 | `run owner / test a_landing_is_solved_on_the_left_value_then_its_instant_runs ... ok` | pass |
| C45 | `run owner / test an_off_grid_step_lands_on_the_tick_grid ... ok` | pass |
| C46 | `run owner / test finish_at_the_operating_point_ends_the_run ... ok` | pass |
| C47 | `run owner / test consecutive_rejections_beyond_the_cap_fail_loud ... ok` | pass |
| C48 | `run owner / test irreversible_reactions_are_bounded_by_the_current_instant ... ok` | pass |
| C49 | `run owner / test traces_reach_the_sink_in_time_order ... ok` | pass |
| C50 | `run abi / test an_unreadable_value_list_is_a_fault ... ok` | pass |
| C51 | `run owner / test a_crossing_within_half_a_tick_of_the_target_does_not_loop ... ok` | pass |
| C52 | `run abi / from_slice (line - compile fail ... ok` | pass |
| Build | `cargo build -p piperine-abi --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run abi` - exit 0

```
test crates/piperine-abi/src/raw.rs - raw::List<'a,T>::from_slice (line 216) - compile fail ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_abi-e7348cb5af0292bb)
     Running tests/caller.rs (/var/tmp/piperine-target/debug/deps/caller-e0f20ffd0203eae0)
     Running tests/contract.rs (/var/tmp/piperine-target/debug/deps/contract-e6058296f0975f1a)
     Running tests/examples.rs (/var/tmp/piperine-target/debug/deps/examples-f3f830d95031e1ae)
     Running tests/layouts.rs (/var/tmp/piperine-target/debug/deps/layouts-9457e404aa498867)
     Running tests/shape.rs (/var/tmp/piperine-target/debug/deps/shape-d7138db77bd0c5c2)
     Running tests/thunks.rs (/var/tmp/piperine-target/debug/deps/thunks-f0f5137229b0439b)
     Running tests/validate.rs (/var/tmp/piperine-target/debug/deps/validate-1d6a89bfc874e467)
     Running tests/verify.rs (/var/tmp/piperine-target/debug/deps/verify-f34510de2ff47b59)
   Doc-tests piperine_abi
```

`Run guards` - exit 0

```
test instructions_have_no_entry_for_a_retired_crate ... ok
test retired_crates_are_gone ... ok
test solver_manifest_has_no_in_tree_dev_dependency ... ok
test instructions_list_the_kept_crates ... ok
test no_manifest_names_a_retired_crate ... ok
test instruction_files_and_state_name_the_abi_crate ... ok
test root_tests_hold_no_end_to_end_target ... ok
test workflows_name_no_retired_crate ... ok
test state_records_the_solver_structure_decision ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/crate_boundaries.rs (/var/tmp/piperine-target/debug/deps/crate_boundaries-0c5174e643cd6c6a)
     Running tests/workspace_shape.rs (/var/tmp/piperine-target/debug/deps/workspace_shape-4e1a2c802e20ae74)
```

`Run owner` - exit 0

```
     Running tests/protocol_operating_point.rs (/var/tmp/piperine-target/debug/deps/protocol_operating_point-b1d9434868e15534)
     Running tests/protocol_seam.rs (/var/tmp/piperine-target/debug/deps/protocol_seam-4b1dc620b76ef182)
     Running tests/protocol_sink.rs (/var/tmp/piperine-target/debug/deps/protocol_sink-71f8e6e13006f53b)
     Running tests/protocol_transient.rs (/var/tmp/piperine-target/debug/deps/protocol_transient-a1335977476e19b8)
     Running tests/pss.rs (/var/tmp/piperine-target/debug/deps/pss-aa9367824e73e638)
     Running tests/removed_surface.rs (/var/tmp/piperine-target/debug/deps/removed_surface-a2055f83393b01b4)
     Running tests/sens.rs (/var/tmp/piperine-target/debug/deps/sens-707ad0a28d345ded)
     Running tests/sensitivity_fallback.rs (/var/tmp/piperine-target/debug/deps/sensitivity_fallback-0744a42e9e49ecf3)
     Running tests/solver_entry.rs (/var/tmp/piperine-target/debug/deps/solver_entry-505a33bed81e5230)
     Running tests/stamp_bypass.rs (/var/tmp/piperine-target/debug/deps/stamp_bypass-df1b09cb03e0984f)
     Running tests/stepper_fold.rs (/var/tmp/piperine-target/debug/deps/stepper_fold-ae21a4136fd93b2a)
     Running tests/temperature.rs (/var/tmp/piperine-target/debug/deps/temperature-31c3d61e276d9a8f)
     Running tests/transient_reentry.rs (/var/tmp/piperine-target/debug/deps/transient_reentry-83e3b0d63c97404e)
     Running tests/workspace_shape.rs (/var/tmp/piperine-target/debug/deps/workspace_shape-4e1a2c802e20ae74)
   Doc-tests piperine_solver
```
