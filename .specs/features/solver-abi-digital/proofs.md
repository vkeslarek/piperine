# solver-abi-digital proofs

**HEAD**: fa2a6e1a
**Tree**: clean
**Shell**: /bin/sh
**Result**: 2 pass, 46 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run digital / test a_delay_is_due_at_the_nearest_tick ... ok` | **fail** |
| C2 | `run digital / test a_time_outside_the_tick_range_is_e0502 ... ok` | **fail** |
| C3 | `run digital / test a_nan_delay_is_e0435 ... ok` | **fail** |
| C4 | `run digital / test an_infinite_delay_never_comes_due ... ok` | **fail** |
| C5 | `run digital / test a_net_starts_as_zero_bytes ... ok` | **fail** |
| C6 | `run digital / test a_change_keeps_the_previous_bytes ... ok` | **fail** |
| C7 | `run digital / test changed_reads_one_for_exactly_one_microstep ... ok` | **fail** |
| C8 | `run digital / test two_writers_of_one_net_are_refused_naming_both ... ok` | **fail** |
| C9 | `run digital / test unequal_layouts_on_one_net_are_refused ... ok` | **fail** |
| C10 | `run digital / test a_cycle_through_no_relax_net_is_refused_naming_its_functions ... ok` | **fail** |
| C11 | `run digital / test a_function_runs_when_a_read_net_changed ... ok` | **fail** |
| C12 | `run digital / test a_function_with_unchanged_inputs_is_not_called ... ok` | **fail** |
| C13 | `run digital / test commits_apply_in_made_statement_call_order ... ok` | **fail** |
| C14 | `run digital / test transport_removes_later_commits_of_the_same_statement_only ... ok` | **fail** |
| C15 | `run digital / test replace_removes_every_pending_commit_of_the_statement ... ok` | **fail** |
| C16 | `run digital / test a_relax_group_settles_within_its_bound ... ok` | **fail** |
| C17 | `run digital / test a_moving_known_field_is_zeroed_and_observed ... ok` | **fail** |
| C18 | `run digital / test later_is_dropped_when_its_bound_is_under_one_tick ... ok` | **fail** |
| C19 | `run digital / test a_reaction_runs_once_when_its_event_fired ... ok` | **fail** |
| C20 | `run digital / test reactions_of_one_microstep_read_one_snapshot ... ok` | **fail** |
| C21 | `run digital / test a_zero_delay_commit_applies_at_the_next_microstep ... ok` | **fail** |
| C22 | `run digital / test a_commit_outside_its_statement_range_is_a_fault ... ok` | **fail** |
| C23 | `run digital / test the_traced_instant_of_section_15_1 ... ok` | **fail** |
| C24 | `run digital / test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok` | **fail** |
| C25 | `run digital / test a_function_fault_carries_code_tick_and_instance ... ok` | **fail** |
| C26 | `run digital / test finish_marks_the_instant_final ... ok` | **fail** |
| C27 | `run digital / test a_quiet_commit_does_not_start_a_microstep ... ok` | **fail** |
| C28 | `run digital / test start_commits_inits_and_fires_start ... ok` | **fail** |
| C29 | `run digital / test a_settle_commit_with_no_delay_is_refused ... ok` | **fail** |
| C30 | `run digital / test restore_returns_store_queue_and_observations_byte_for_byte ... ok` | **fail** |
| C31 | `run digital / test two_runs_give_identical_observations ... ok` | **fail** |
| C32 | `run digital / test advance_stops_before_the_first_time_point_and_leaves_it_untouched ... ok` | **fail** |
| C33 | `run digital / test an_instant_reports_boundary_fired_and_final ... ok` | **fail** |
| C34 | `run digital / test each_clause_of_the_time_point_test_stops_advance ... ok` | **fail** |
| C35 | `run digital / test horizon_is_before_the_earliest_pending_commit_and_the_limit ... ok` | **fail** |
| C36 | `run digital / test a_commit_due_after_the_target_stays_pending ... ok` | **fail** |
| C37 | `run digital / test every_change_commit_and_record_is_observed_in_order ... ok` | **fail** |
| C38 | `run digital / test observations_after_a_checkpoint_are_gone_after_restore ... ok` | **fail** |
| C39 | `run digital / test a_host_write_is_a_commit_at_microstep_zero ... ok` | **fail** |
| C40 | `run digital / test a_structural_param_write_during_a_run_is_refused ... ok` | **fail** |
| C41 | `run digital / test a_false_check_refuses_the_write_and_changes_nothing ... ok` | **fail** |
| C42 | `run digital / test reshape_on_the_shadow_names_the_instance_and_changes_nothing ... ok` | **fail** |
| C43 | `run digital / test preview_boundary_applies_microstep_zero_on_a_copy ... ok` | **fail** |
| C44 | `run digital / test abi_modules_name_no_analog_or_owner_item ... ok` | **fail** |
| C45 | `cargo test -p piperine-digital-solver --lib 2>&1 / grep -E '^test result: ok\. 27 passed'` | pass |
| Run digital | `cargo test -p piperine-digital-solver` | **fail** - exit 101 |
| Build | `cargo build -p piperine-digital-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | **fail** - error[E0451]: fields name, prototype, data, create, destroy, eval and react of struct Module are pri |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run digital` - exit 101

```
    |         ^^^^^^^ private field
754 |         eval: rogue_eval,
    |         ^^^^ private field
755 |         react: rogue_react,
    |         ^^^^^ private field

For more information about this error, try `rustc --explain E0451`.
error: could not compile `piperine-digital-solver` (test "devices") due to 1 previous error
warning: build failed, waiting for other jobs to finish...
error: could not compile `piperine-digital-solver` (test "functions") due to 1 previous error
error: could not compile `piperine-digital-solver` (test "reactions") due to 1 previous error
error: could not compile `piperine-digital-solver` (test "advance") due to 1 previous error
error: could not compile `piperine-digital-solver` (test "host") due to 1 previous error
error: could not compile `piperine-digital-solver` (test "instant") due to 1 previous error
error: could not compile `piperine-digital-solver` (test "checkpoint") due to 1 previous error
```

`cargo test -p piperine-digital-solver --lib 2>&1 / grep -E '^test result: ok\. 27 passed'` - exit 0

```
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
