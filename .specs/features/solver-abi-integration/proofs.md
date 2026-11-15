# solver-abi-integration proofs

**HEAD**: 65716919
**Tree**: clean
**Shell**: /bin/sh
**Result**: 34 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run solver / test pins_bind_by_role ... ok` | pass |
| C2 | `run solver / test ports_on_one_net_share_one_unknown ... ok` | pass |
| C3 | `run solver / test a_continuous_init_starts_newton_and_is_held_through_the_operating_point ... ok` | pass |
| C4 | `run solver / test a_pin_abstol_tightens_its_node ... ok` | pass |
| C5 | `run solver / test a_clock_nothing_analog_reads_adds_no_step ... ok` | pass |
| C6 | `run solver / test a_boundary_change_is_delivered_after_the_landing_and_restarts_the_next ... ok` | pass |
| C7 | `run solver / test a_rejected_step_leaves_no_trace ... ok` | pass |
| C8 | `run solver / test an_irreversible_reaction_runs_once_at_an_accepted_time ... ok` | pass |
| C9 | `run solver / test finish_ends_the_analysis_at_an_accepted_time ... ok` | pass |
| C10 | `run solver / test a_design_without_analog_runs_every_instant_with_no_solve ... ok` | pass |
| C11 | `grep -q 'test transient_takes_the_same_steps_on_both_contracts ... ok' .specs/features/solver-abi-integration/` | pass |
| C12 | `run solver / test a_mixed_loop_settles_at_its_operating_point ... ok` | pass |
| C13 | `run solver / test operating_point_rounds_beyond_the_bound_are_e0506 ... ok` | pass |
| C14 | `run solver / test the_sink_receives_everything_in_instant_order ... ok` | pass |
| C15 | `run solver / test records_of_an_undone_instant_do_not_reach_the_sink ... ok` | pass |
| C16 | `run solver / test a_false_check_refuses_a_set ... ok` | pass |
| C17 | `run solver / test reshape_is_refused_in_a_transient_and_rebinds_between_analyses ... ok` | pass |
| C18 | `run solver / test a_scheduled_host_write_lands_and_restarts_small ... ok` | pass |
| C19 | `run solver / test globals_hold_the_session_values ... ok` | pass |
| C20 | `run solver / test sensitivity_uses_declared_partials_and_falls_back_to_a_finite_difference ... ok` | pass |
| C21 | `o=$(cargo test -p piperine-solver --test abi_dc --test abi_tran --test abi_ac --test abi_noise --test limiting` | pass |
| C22 | `o=$(cargo test -p piperine-solver --test protocol_crossings 2>&1); for t in a_cap_watch_bounds_the_next_step a` | pass |
| C23 | `a=$(mktemp); b=$(mktemp); git show 98b30ad6:crates/piperine-solver/tests/parity_baseline.rs / grep -E 'abs\(\)` | pass |
| C24 | `run solver / test removed_tests_are_listed_with_their_item ... ok` | pass |
| C25 | `run solver / test the_element_contract_is_gone ... ok` | pass |
| C26 | `run solver / test the_old_digital_engine_is_gone ... ok` | pass |
| C27 | `o=$(cargo test -p piperine-solver 2>&1); for t in every_device_function_passes_the_partials_verifier every_hel` | pass |
| C28 | `run guards / test analog_sources_name_no_digital_type ... ok` | pass |
| C29 | `run guards / test digital_sources_name_no_analog_type ... ok` | pass |
| C30 | `cargo test -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver` | pass |
| Build | `cargo build -p piperine-abi --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-analog-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-digital-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run solver` - exit 0

```
     Running tests/protocol_real.rs (/var/tmp/piperine-target/debug/deps/protocol_real-f55fe40255e56d47)
     Running tests/protocol_seam.rs (/var/tmp/piperine-target/debug/deps/protocol_seam-c1ce6dacc52dcbdd)
     Running tests/protocol_sink.rs (/var/tmp/piperine-target/debug/deps/protocol_sink-b1209bce1bede8f0)
     Running tests/protocol_transient.rs (/var/tmp/piperine-target/debug/deps/protocol_transient-397b184b55ccee2d)
     Running tests/pss.rs (/var/tmp/piperine-target/debug/deps/pss-40159cebe3d4a24b)
     Running tests/pz_device_fault.rs (/var/tmp/piperine-target/debug/deps/pz_device_fault-fd1a27b38fc1dd77)
     Running tests/removed_surface.rs (/var/tmp/piperine-target/debug/deps/removed_surface-aa314b7897f7034c)
     Running tests/sens.rs (/var/tmp/piperine-target/debug/deps/sens-00d1e1c41d1271b8)
     Running tests/sensitivity_fallback.rs (/var/tmp/piperine-target/debug/deps/sensitivity_fallback-06a01a9281965c08)
     Running tests/solver_entry.rs (/var/tmp/piperine-target/debug/deps/solver_entry-70c2d931927c464a)
     Running tests/stepper_fold.rs (/var/tmp/piperine-target/debug/deps/stepper_fold-d56eeb19d5384314)
     Running tests/temperature.rs (/var/tmp/piperine-target/debug/deps/temperature-b56cf1990597c205)
     Running tests/transient_reentry.rs (/var/tmp/piperine-target/debug/deps/transient_reentry-458cfbcdb3a61bde)
     Running tests/workspace_shape.rs (/var/tmp/piperine-target/debug/deps/workspace_shape-f7f528358e48ad39)
   Doc-tests piperine_solver
```

`Run guards` - exit 0

```
running 8 tests
test abi_crate_is_a_leaf ... ok
test owner_depends_on_the_abi_and_both_sub_solvers_only ... ok
test each_sub_solver_holds_its_modules ... ok
test sub_solvers_depend_on_the_abi_crate_only ... ok
test the_three_solver_crates_exist ... ok
test the_manifest_scan_reads_table_form_dependencies ... ok
test digital_sources_name_no_analog_type ... ok
test analog_sources_name_no_digital_type ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/crate_boundaries.rs (/var/tmp/piperine-target/debug/deps/crate_boundaries-59af74054bbd5ab6)
```

`grep -q 'test transient_takes_the_same_steps_on_both_contracts ... ok' .specs/features/solver-abi-integration/` - exit 0

```
a9a23f55b4cc4c32ba5038ab776722dcc9ef5958
```

`o=$(cargo test -p piperine-solver --test abi_dc --test abi_tran --test abi_ac --test abi_noise --test limiting` - exit 0

```
(no output)
```

`o=$(cargo test -p piperine-solver --test protocol_crossings 2>&1); for t in a_cap_watch_bounds_the_next_step a` - exit 0

```
(no output)
```

`a=$(mktemp); b=$(mktemp); git show 98b30ad6:crates/piperine-solver/tests/parity_baseline.rs / grep -E 'abs\(\)` - exit 0

```
     Running tests/protocol_real.rs (/var/tmp/piperine-target/debug/deps/protocol_real-f55fe40255e56d47)
     Running tests/protocol_seam.rs (/var/tmp/piperine-target/debug/deps/protocol_seam-c1ce6dacc52dcbdd)
     Running tests/protocol_sink.rs (/var/tmp/piperine-target/debug/deps/protocol_sink-b1209bce1bede8f0)
     Running tests/protocol_transient.rs (/var/tmp/piperine-target/debug/deps/protocol_transient-397b184b55ccee2d)
     Running tests/pss.rs (/var/tmp/piperine-target/debug/deps/pss-40159cebe3d4a24b)
     Running tests/pz_device_fault.rs (/var/tmp/piperine-target/debug/deps/pz_device_fault-fd1a27b38fc1dd77)
     Running tests/removed_surface.rs (/var/tmp/piperine-target/debug/deps/removed_surface-aa314b7897f7034c)
     Running tests/sens.rs (/var/tmp/piperine-target/debug/deps/sens-00d1e1c41d1271b8)
     Running tests/sensitivity_fallback.rs (/var/tmp/piperine-target/debug/deps/sensitivity_fallback-06a01a9281965c08)
     Running tests/solver_entry.rs (/var/tmp/piperine-target/debug/deps/solver_entry-70c2d931927c464a)
     Running tests/stepper_fold.rs (/var/tmp/piperine-target/debug/deps/stepper_fold-d56eeb19d5384314)
     Running tests/temperature.rs (/var/tmp/piperine-target/debug/deps/temperature-b56cf1990597c205)
     Running tests/transient_reentry.rs (/var/tmp/piperine-target/debug/deps/transient_reentry-458cfbcdb3a61bde)
     Running tests/workspace_shape.rs (/var/tmp/piperine-target/debug/deps/workspace_shape-f7f528358e48ad39)
   Doc-tests piperine_solver
```

`o=$(cargo test -p piperine-solver 2>&1); for t in every_device_function_passes_the_partials_verifier every_hel` - exit 0

```
(no output)
```

`cargo test -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver` - exit 0

```
     Running tests/protocol_transient.rs (/var/tmp/piperine-target/debug/deps/protocol_transient-397b184b55ccee2d)
     Running tests/pss.rs (/var/tmp/piperine-target/debug/deps/pss-40159cebe3d4a24b)
     Running tests/pz_device_fault.rs (/var/tmp/piperine-target/debug/deps/pz_device_fault-fd1a27b38fc1dd77)
     Running tests/removed_surface.rs (/var/tmp/piperine-target/debug/deps/removed_surface-aa314b7897f7034c)
     Running tests/sens.rs (/var/tmp/piperine-target/debug/deps/sens-00d1e1c41d1271b8)
     Running tests/sensitivity_fallback.rs (/var/tmp/piperine-target/debug/deps/sensitivity_fallback-06a01a9281965c08)
     Running tests/solver_entry.rs (/var/tmp/piperine-target/debug/deps/solver_entry-70c2d931927c464a)
     Running tests/stepper_fold.rs (/var/tmp/piperine-target/debug/deps/stepper_fold-d56eeb19d5384314)
     Running tests/temperature.rs (/var/tmp/piperine-target/debug/deps/temperature-b56cf1990597c205)
     Running tests/transient_reentry.rs (/var/tmp/piperine-target/debug/deps/transient_reentry-458cfbcdb3a61bde)
     Running tests/workspace_shape.rs (/var/tmp/piperine-target/debug/deps/workspace_shape-f7f528358e48ad39)
   Doc-tests piperine_abi
   Doc-tests piperine_analog_solver
   Doc-tests piperine_digital_solver
   Doc-tests piperine_solver
```
