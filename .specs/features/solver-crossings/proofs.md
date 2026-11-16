# solver-crossings proofs

**HEAD**: 601264c4
**Tree**: clean
**Shell**: /bin/sh
**Result**: 25 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run digital / test two_commits_to_one_net_settle_to_one_write ... ok` | pass |
| C2 | `run digital / test a_commit_undone_in_its_microstep_is_no_change ... ok` | pass |
| C3 | `run digital / test a_settled_change_is_one_observation ... ok` | pass |
| C4 | `run solver / test x_changed_does_not_fire_when_the_settled_value_is_unchanged ... ok` | pass |
| C5 | `run solver / test a_ramp_lands_within_ttol_plus_one_tick ... ok` | pass |
| C6 | `run solver / test a_sine_crossing_is_located ... ok` | pass |
| C7 | `run solver / test an_rc_charge_trips_at_its_threshold ... ok` | pass |
| C8 | `run solver / test a_fired_crossing_is_a_commit_at_the_landing ... ok` | pass |
| C9 | `run solver / test the_earliest_of_several_crossings_lands_first ... ok` | pass |
| C10 | `run solver / test crossings_on_one_tick_fire_together ... ok` | pass |
| C11 | `run solver / test the_session_ttol_applies_to_a_watch_without_a_ttol_pin ... ok` | pass |
| C12 | `run solver / test a_watch_without_an_etol_pin_takes_the_potential_abstol ... ok` | pass |
| C13 | `run solver / test a_crossing_in_the_step_after_a_jump_is_not_located ... ok` | pass |
| C14 | `run solver / test a_crossing_between_operating_point_rounds_fires_in_the_later_round ... ok` | pass |
| C15 | `run solver / test a_crossing_between_two_operating_points_fires_at_the_later ... ok` | pass |
| C16 | `run solver / test an_invalid_ttol_or_etol_fails_the_analysis_with_e0510 ... ok` | pass |
| C17 | `run solver / test an_invalid_step_cap_fails_the_analysis_with_e0509 ... ok` | pass |
| C18 | `run parity / test parity_rc_transient ... ok` | pass |
| C19 | `run analog / test rising_reports_a_sign_change_without_accepting ... ok` | pass |
| C20 | `run abi / test a_reaction_may_read_gmin_and_source_scale ... ok` | pass |
| C21 | `run env / test reactions_and_functions_read_the_continuation_values ... ok` | pass |
| C22 | `run solverenv / test a_reaction_reads_temperature_gmin_and_source_scale ... ok` | pass |
| C23 | `run solverenv / test a_discrete_function_reads_gmin_and_source_scale ... ok` | pass |
| C24 | `run parity / test parity_rc_transient ... ok` | pass |
| Build | `cargo build --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run digital` - exit 0

```
test a_commit_undone_in_its_microstep_is_no_change ... ok
test a_settled_change_is_one_observation ... ok
test a_plan_with_no_code_applies_host_commits_only ... ok
test start_commits_inits_and_fires_start ... ok
test a_settle_commit_with_no_delay_is_refused ... ok
test an_instant_reports_boundary_fired_and_final ... ok
test two_commits_to_one_net_settle_to_one_write ... ok
test a_quiet_commit_does_not_start_a_microstep ... ok
test the_traced_instant_of_section_15_1 ... ok
test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running tests/instant.rs (/var/tmp/piperine-target/debug/deps/instant-1872408436915898)
```

`Run solver` - exit 0

```
test a_ramp_lands_within_ttol_plus_one_tick ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s


running 2 tests
test its_devices_pass_the_partials_verifier ... ok
test x_changed_does_not_fire_when_the_settled_value_is_unchanged ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/crossing_rules.rs (/var/tmp/piperine-target/debug/deps/crossing_rules-b948d3f471201c5e)
     Running tests/protocol_crossings.rs (/var/tmp/piperine-target/debug/deps/protocol_crossings-a49070822fa2ef49)
     Running tests/settled_value.rs (/var/tmp/piperine-target/debug/deps/settled_value-d9094b11e06d8b5f)
```

`Run parity` - exit 0

```
running 7 tests
test its_devices_pass_the_partials_verifier ... ok
test parity_digital_scheduler_snapshot ... ok
test parity_divider_op_dc ... ok
test parity_diode_dc_point ... ok
test parity_mixed_signal_dc_settle ... ok
test parity_rlc_ac_resonant_peak ... ok
test parity_rc_transient ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/parity_baseline.rs (/var/tmp/piperine-target/debug/deps/parity_baseline-0b1b4c73656ab636)
```

`Run analog` - exit 0

```
test a_cross_watch_fires_on_a_rising_sign_change ... ok
test an_instance_lists_its_terms_and_var_watches_and_reads_a_term ... ok
test a_trace_record_survives_a_rejected_step ... ok
test samples_are_the_continuous_pins_reactions_read ... ok
test accept_reports_the_smallest_cap ... ok
test rising_reports_a_sign_change_without_accepting ... ok
test a_var_watch_is_readable_at_an_accepted_solution ... ok
test accept_fails_with_e0509_on_an_invalid_cap ... ok
test accept_fails_with_e0510_on_an_invalid_tolerance ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-solver-crossings/crates/piperine-analog-solver)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.91s
     Running tests/assembly_watch.rs (/var/tmp/piperine-target/debug/deps/assembly_watch-78f836e6cfd00fff)
```

`Run abi` - exit 0

```
running 6 tests
test a_zeroed_shape_entry_is_an_unknown_constant ... ok
test a_shape_with_an_unreadable_list_is_refused ... ok
test a_reaction_may_read_gmin_and_source_scale ... ok
test the_valid_fixtures_pass ... ok
test an_index_is_checked_before_it_is_followed ... ok
test every_shape_local_rule_is_enforced ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running tests/validate.rs (/var/tmp/piperine-target/debug/deps/validate-1d6a89bfc874e467)
```

`Run env` - exit 0

```
test a_moving_field_without_a_known_byte_fails ... ok
test a_function_with_unchanged_inputs_is_not_called ... ok
test a_relax_group_settles_within_its_bound ... ok
test a_function_fault_carries_code_tick_and_instance ... ok
test changed_is_zero_one_microstep_later ... ok
test every_function_runs_once_at_the_first_instant ... ok
test a_moving_known_field_is_zeroed_and_observed ... ok
test functions_run_after_every_writer_of_their_reads ... ok
test an_absent_optional_pin_reads_zero_bytes ... ok
test reactions_and_functions_read_the_continuation_values ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running tests/functions.rs (/var/tmp/piperine-target/debug/deps/functions-d37ec655bf42d964)
```

`Run solverenv` - exit 0

```
running 3 tests
test its_devices_pass_the_partials_verifier ... ok
test a_discrete_function_reads_gmin_and_source_scale ... ok
test a_reaction_reads_temperature_gmin_and_source_scale ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/environment.rs (/var/tmp/piperine-target/debug/deps/environment-b554f2f596d4baed)
```
