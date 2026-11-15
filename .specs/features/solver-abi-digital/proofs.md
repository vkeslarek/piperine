# solver-abi-digital proofs

**HEAD**: faf2712e
**Tree**: clean
**Shell**: /bin/sh
**Result**: 47 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run digital / test a_delay_is_due_at_the_nearest_tick ... ok` | pass |
| C2 | `run digital / test a_time_outside_the_tick_range_is_e0502 ... ok` | pass |
| C3 | `run digital / test a_nan_delay_is_e0435 ... ok` | pass |
| C4 | `run digital / test an_infinite_delay_never_comes_due ... ok` | pass |
| C5 | `run digital / test a_net_starts_as_zero_bytes ... ok` | pass |
| C6 | `run digital / test a_change_keeps_the_previous_bytes ... ok` | pass |
| C7 | `run digital / test changed_reads_one_for_exactly_one_microstep ... ok` | pass |
| C8 | `run digital / test two_writers_of_one_net_are_refused_naming_both ... ok` | pass |
| C9 | `run digital / test unequal_layouts_on_one_net_are_refused ... ok` | pass |
| C10 | `run digital / test a_cycle_through_no_relax_net_is_refused_naming_its_functions ... ok` | pass |
| C11 | `run digital / test a_function_runs_when_a_read_net_changed ... ok` | pass |
| C12 | `run digital / test a_function_with_unchanged_inputs_is_not_called ... ok` | pass |
| C13 | `run digital / test commits_apply_in_made_statement_call_order ... ok` | pass |
| C14 | `run digital / test transport_removes_later_commits_of_the_same_statement_only ... ok` | pass |
| C15 | `run digital / test replace_removes_every_pending_commit_of_the_statement ... ok` | pass |
| C16 | `run digital / test a_relax_group_settles_within_its_bound ... ok` | pass |
| C17 | `run digital / test a_moving_known_field_is_zeroed_and_observed ... ok` | pass |
| C18 | `run digital / test later_is_dropped_when_its_bound_is_under_one_tick ... ok` | pass |
| C19 | `run digital / test a_reaction_runs_once_when_its_event_fired ... ok` | pass |
| C20 | `run digital / test reactions_of_one_microstep_read_one_snapshot ... ok` | pass |
| C21 | `run digital / test a_zero_delay_commit_applies_at_the_next_microstep ... ok` | pass |
| C22 | `run digital / test a_commit_outside_its_statement_range_is_a_fault ... ok` | pass |
| C23 | `run digital / test the_traced_instant_of_section_15_1 ... ok` | pass |
| C24 | `run digital / test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok` | pass |
| C25 | `run digital / test a_function_fault_carries_code_tick_and_instance ... ok` | pass |
| C26 | `run digital / test finish_marks_the_instant_final ... ok` | pass |
| C27 | `run digital / test a_quiet_commit_does_not_start_a_microstep ... ok` | pass |
| C28 | `run digital / test start_commits_inits_and_fires_start ... ok` | pass |
| C29 | `run digital / test a_settle_commit_with_no_delay_is_refused ... ok` | pass |
| C30 | `run digital / test restore_returns_store_queue_and_observations_byte_for_byte ... ok` | pass |
| C31 | `run digital / test two_runs_give_identical_observations ... ok` | pass |
| C32 | `run digital / test advance_stops_before_the_first_time_point_and_leaves_it_untouched ... ok` | pass |
| C33 | `run digital / test an_instant_reports_boundary_fired_and_final ... ok` | pass |
| C34 | `run digital / test each_clause_of_the_time_point_test_stops_advance ... ok` | pass |
| C35 | `run digital / test horizon_is_before_the_earliest_pending_commit_and_the_limit ... ok` | pass |
| C36 | `run digital / test a_commit_due_after_the_target_stays_pending ... ok` | pass |
| C37 | `run digital / test every_change_commit_and_record_is_observed_in_order ... ok` | pass |
| C38 | `run digital / test observations_after_a_checkpoint_are_gone_after_restore ... ok` | pass |
| C39 | `run digital / test a_host_write_is_a_commit_at_microstep_zero ... ok` | pass |
| C40 | `run digital / test a_structural_param_write_during_a_run_is_refused ... ok` | pass |
| C41 | `run digital / test a_false_check_refuses_the_write_and_changes_nothing ... ok` | pass |
| C42 | `run digital / test reshape_on_the_shadow_names_the_instance_and_changes_nothing ... ok` | pass |
| C44 | `run digital / test abi_modules_name_no_analog_or_owner_item ... ok` | pass |
| C45 | `cargo test -p piperine-digital-solver --lib 2>&1 / grep -E '^test result: ok\. 27 passed'` | pass |
| C46 | `run digital / test horizon_counts_the_commits_other_reactions_make_in_the_same_microstep ... ok` | pass |
| Build | `cargo build -p piperine-digital-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run digital` - exit 0

```
     Running tests/advance.rs (/var/tmp/piperine-target/debug/deps/advance-f79babe5f6c91f9c)
     Running tests/bind.rs (/var/tmp/piperine-target/debug/deps/bind-3086bacece866c3b)
     Running tests/boundary.rs (/var/tmp/piperine-target/debug/deps/boundary-77a3f9fd47207f0c)
     Running tests/checkpoint.rs (/var/tmp/piperine-target/debug/deps/checkpoint-0762ffcfd86abd21)
     Running tests/dag.rs (/var/tmp/piperine-target/debug/deps/dag-1ba11004d5bf21ae)
     Running tests/devices.rs (/var/tmp/piperine-target/debug/deps/devices-5eaca71a77bd5b81)
     Running tests/functions.rs (/var/tmp/piperine-target/debug/deps/functions-d37ec655bf42d964)
     Running tests/host.rs (/var/tmp/piperine-target/debug/deps/host-1dbf2a7c81d5a14e)
     Running tests/instant.rs (/var/tmp/piperine-target/debug/deps/instant-1872408436915898)
     Running tests/queue.rs (/var/tmp/piperine-target/debug/deps/queue-8ce281259e0add3c)
     Running tests/reactions.rs (/var/tmp/piperine-target/debug/deps/reactions-6545fbfa642e1baa)
     Running tests/round1.rs (/var/tmp/piperine-target/debug/deps/round1-9cbbeec96634d37c)
     Running tests/store.rs (/var/tmp/piperine-target/debug/deps/store-5dd2056c91c78ed8)
     Running tests/ticks.rs (/var/tmp/piperine-target/debug/deps/ticks-ad1d170213cba4c3)
   Doc-tests piperine_digital_solver
```

`cargo test -p piperine-digital-solver --lib 2>&1 / grep -E '^test result: ok\. 27 passed'` - exit 0

```
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
