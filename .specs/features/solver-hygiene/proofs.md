# solver-hygiene proofs

**HEAD**: b623dd0e
**Tree**: clean
**Shell**: /bin/sh
**Result**: 73 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run deviationlib / test deviation::tests::the_worst_ratio_is_the_largest_difference_over_its_bound ... ok` | pass |
| C2 | `run deviationlib / test deviation::tests::a_pair_at_the_bound_has_ratio_one ... ok` | pass |
| C3 | `run deviation / test the_table_covers_every_twin ... ok` | pass |
| C4 | `run analogddt / test a_nonlinear_charge_through_a_limit_keeps_the_operating_point ... ok` | pass |
| C5 | `run analogddt / test the_ddt_history_holds_the_charge_at_the_solution ... ok` | pass |
| C6 | `run parity / test tran_dq ... ok` | pass |
| C7 | `run parity / test every_twin_has_a_cir_and_a_test ... ok` | pass |
| C8 | `run analoglib / test convergence::tests::the_pi_controller_resumes_its_memory ... ok` | pass |
| C9 | `run reentry / test a_reentry_from_every_recorded_step_continues_bit_for_bit ... ok` | pass |
| C10 | `run reentry / test a_reentry_after_a_jump_takes_the_same_restart ... ok` | pass |
| C11 | `run reentry / test the_reentry_state_holds_the_newton_history_and_the_controller ... ok` | pass |
| C12 | `run reentry / test reentry_last_step_attaches_state_to_the_last_step_only ... ok` | pass |
| C13 | `run reentry / test the_default_reentry_keeps_every_step_state ... ok` | pass |
| C14 | `run analoglib / test error::tests::a_run_error_displays_its_code_time_and_names ... ok` | pass |
| C15 | `run analoglib / test error::tests::a_run_error_without_names_shows_its_unknowns ... ok` | pass |
| C16 | `run analoglib / test singular::tests::a_loop_of_two_potential_sources_is_singular_in_both_flows ... ok` | pass |
| C17 | `run analoglib / test singular::tests::a_node_reached_only_through_capacitors_is_singular_at_dc ... ok` | pass |
| C18 | `run analoglib / test singular::tests::a_singular_system_above_the_dense_bound_says_it_skipped ... ok` | pass |
| C19 | `run analoglib / test newton_raphson::tests::a_non_finite_solution_of_finite_stamps_is_e0503 ... ok` | pass |
| C20 | `run analoglib / test newton_raphson::tests::exhaustion_names_the_unknowns_that_failed ... ok` | pass |
| C21 | `run failure / test a_singular_solve_names_the_pins_of_its_node ... ok` | pass |
| C22 | `run failure / test a_loop_of_sources_names_both_instances ... ok` | pass |
| C23 | `run failure / test a_non_finite_contribution_is_e0507_and_names_the_instance ... ok` | pass |
| C24 | `run analoglib / test stamp_validation::tests::a_non_finite_stamp_is_e0507_with_its_row ... ok` | pass |
| C25 | `run failure / test an_equilibrium_that_never_converges_is_e0505_with_its_unknowns ... ok` | pass |
| C26 | `run failure / test a_step_that_fails_newton_at_the_floor_is_e0508_with_its_time ... ok` | pass |
| C27 | `run failure / test a_jump_step_that_fails_at_the_floor_is_e0504_naming_the_carrier ... ok` | pass |
| C28 | `run runerrors / test a_loop_of_two_vsources_from_source_is_e0503 ... ok` | pass |
| C29 | `run runerrors / test a_floating_node_from_source_is_e0503 ... ok` | pass |
| C30 | `run runerrors / test a_nan_contribution_from_source_is_e0507 ... ok` | pass |
| C31 | `run spectext / test part_v_states_the_solver_hygiene_rules ... ok` | pass |
| C32 | `run spectext / test appendix_c_states_the_solver_hygiene_numerics ... ok` | pass |
| C33 | `run analoglib / test integration::tests::the_state_estimate_reads_chgtol ... ok` | pass |
| C34 | `run analoglib / test integration::tests::the_state_estimate_is_zero_for_a_linear_charge ... ok` | pass |
| C35 | `run analoglib / test integration::tests::a_rise_from_flat_is_measured_and_rejected ... ok` | pass |
| C36 | `run analogddt / test the_assembly_reports_each_ddt_state_after_a_step ... ok` | pass |
| C37 | `run truncation / test a_source_driven_nonlinear_capacitor_meets_the_current_tolerance ... ok` | pass |
| C38 | `run truncation / test a_larger_chgtol_takes_fewer_steps ... ok` | pass |
| C39 | `run parity / test tran_rc ... ok` | pass |
| C40 | `run truncation / test an_unannounced_corner_fails_e0508_at_the_floor ... ok` | pass |
| C41 | `run truncation / test the_floor_is_retried_once_before_failing ... ok` | pass |
| C42 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok` | pass |
| C43 | `run analoglib / test convergence::tests::node_damping_follows_ngspice ... ok` | pass |
| C44 | `run analoglib / test convergence::tests::no_damping_leaves_the_update ... ok` | pass |
| C45 | `run abidc / test the_dc_driver_iterates_with_the_plans_strategy ... ok` | pass |
| C46 | `run abitran / test the_transient_driver_iterates_with_the_plans_strategy ... ok` | pass |
| C47 | `run parity / test diode_iv ... ok` | pass |
| C48 | `run protocol / test the_proposal_sees_the_next_due_time ... ok` | pass |
| C49 | `! grep -nE '^\/ V(1/2/3/4/5/6/7/8/9/10/11/14/15) \/' GAPS.md && grep -cE '^\/ V1[236] \/' GAPS.md / grep -qx 3` | pass |
| C50 | `grep -q '^### MD-68' .specs/STATE.md` | pass |
| C51 | `run abitran / test the_step_after_a_jump_is_capped_at_a_tenth_of_the_last ... ok` | pass |
| C52 | `run boundary / test a_carrier_change_is_a_time_point_and_starts_the_response ... ok` | pass |
| C53 | `run hostset / test a_scheduled_host_write_lands_and_restarts_small ... ok` | pass |
| C54 | `run instant / test start_at_commits_init_at_its_tick ... ok` | pass |
| C55 | `run reentry / test a_late_start_solves_the_operating_point_at_its_start ... ok` | pass |
| C56 | `run reentry / test a_late_start_runs_no_instant_before_it ... ok` | pass |
| C57 | `run reentry / test a_start_that_is_not_finite_fails_before_the_operating_point ... ok` | pass |
| C58 | `run removed / test the_dead_newton_hooks_are_gone ... ok` | pass |
| C59 | `run removed / test the_phdl_port_text_is_gone ... ok` | pass |
| C60 | `run abidevices / test every_hand_written_device_is_in_a_partials_check ... ok` | pass |
| C61 | `run abidevices / test a_check_of_another_module_does_not_count ... ok` | pass |
| C62 | `run digital / test a_rewind_equals_the_checkpoint_taken_at_the_mark ... ok` | pass |
| C63 | `run digital / test nested_marks_rewind_in_order ... ok` | pass |
| C64 | `run digital / test a_mark_journals_only_the_nets_that_changed ... ok` | pass |
| C65 | `git log --format=%B --grep='the truncation test measures the ddt states' / grep -c 'tran_rc' / grep -qE '^[2-9` | pass |
| C66 | `run reentry / test a_reentry_with_another_stop_lands_on_the_old_stop ... ok` | pass |
| C67 | `run reentry / test a_middle_step_under_last_step_cannot_be_reentered ... ok` | pass |
| C68 | `run failure / test an_instance_is_named_once_in_a_singular_report ... ok` | pass |
| C69 | `run truncation / test the_floor_at_the_stop_time_names_that_time ... ok` | pass |
| C70 | `run abitran / test two_jumps_in_one_restart_step_restart_from_the_later ... ok` | pass |
| Build | `cargo build -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver -p piperin` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` | pass |

## Output

`Run deviationlib` - exit 0

```
running 3 tests
test deviation::tests::a_pair_at_the_bound_has_ratio_one ... ok
test deviation::tests::the_table_has_one_line_per_deviation ... ok
test deviation::tests::the_worst_ratio_is_the_largest_difference_over_its_bound ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.35s
     Running unittests src/lib.rs (/var/tmp/piperine-target-c/debug/deps/piperine_parity-6187a033365891a7)
```

`Run deviation` - exit 0

```
diode_series  v(a)            0.0005  OperatingPoint
diode_series  v(b)            0.0003  OperatingPoint
divider       v(vin)          0.0000  OperatingPoint
divider       v(mid)          0.0000  OperatingPoint
isrc_r        v(a)            0.0000  OperatingPoint
tran_dq       v(a)            0.5123  Time(2e-7)
tran_rc       v(out)          0.5123  Time(2e-7)
tran_dq on rust: steps accepted 1046, rejected 0, dt_min floor hits 0
tran_rc on rust: steps accepted 1000, rejected 0, dt_min floor hits 0
test the_table_covers_every_twin ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 20.97s

    Finished `test` profile [unoptimized] target(s) in 0.22s
     Running tests/deviation.rs (/var/tmp/piperine-target-c/debug/deps/deviation-b80e1723150a094e)
```

`Run analogddt` - exit 0

```
running 7 tests
test a_ddt_term_is_silent_in_an_equilibrium_solve ... ok
test a_restore_returns_histories_and_limit_state ... ok
test a_ddt_term_stamps_the_capacitor_companion ... ok
test the_ddt_history_holds_the_charge_at_the_solution ... ok
test the_assembly_reports_each_ddt_state_after_a_step ... ok
test a_nonlinear_charge_through_a_limit_keeps_the_operating_point ... ok
test an_rc_step_reads_one_minus_exp ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-analog-solver)
    Finished `test` profile [unoptimized] target(s) in 8.52s
     Running tests/assembly_ddt.rs (/var/tmp/piperine-target-c/debug/deps/assembly_ddt-6b389f68791f6e03)
```

`Run parity` - exit 0

```


running 3 tests
test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.04s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/parity_ac.rs (/var/tmp/piperine-target-c/debug/deps/parity_ac-55a5dfa8bfad5186)
     Running tests/parity_corpus.rs (/var/tmp/piperine-target-c/debug/deps/parity_corpus-3c6b0216946f1435)
     Running tests/parity_dc.rs (/var/tmp/piperine-target-c/debug/deps/parity_dc-e39252c43babc518)
     Running tests/parity_op.rs (/var/tmp/piperine-target-c/debug/deps/parity_op-89aa68e5e8a64ddb)
     Running tests/parity_tran.rs (/var/tmp/piperine-target-c/debug/deps/parity_tran-92e4bdddedb6562d)
```

`Run analoglib` - exit 0

```
test math::singular::tests::a_singular_system_above_the_dense_bound_says_it_skipped ... ok
test math::integration::tests::trbdf2_stage_coeffs_degrade_to_backward_euler_on_restart ... ok
test math::faer::tests::singular_system_fails_loud_through_the_existing_error_family ... ok
test math::faer::tests::real_transpose_solve_matches_a_hand_computed_lambda ... ok
test math::faer::tests::one_symbolic_factorization_serves_both_forward_and_transpose_solves ... ok
test math::singular::tests::a_node_reached_only_through_capacitors_is_singular_at_dc ... ok
test math::singular::tests::a_loop_of_two_potential_sources_is_singular_in_both_flows ... ok
test math::newton_raphson::tests::a_non_finite_solution_of_finite_stamps_is_e0503 ... ok
test math::newton_raphson::tests::exhaustion_names_the_unknowns_that_failed ... ok

test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-analog-solver)
    Finished `test` profile [unoptimized] target(s) in 3.95s
     Running unittests src/lib.rs (/var/tmp/piperine-target-c/debug/deps/piperine_analog_solver-03fa06dcb9685482)
```

`Run reentry` - exit 0

```
test reentry_from_captured_state_matches_continuous_run ... ok
test a_start_that_is_not_finite_fails_before_the_operating_point ... ok
test a_reentry_with_another_stop_lands_on_the_old_stop ... ok
test reentry_last_step_attaches_state_to_the_last_step_only ... ok
test a_reentry_after_a_jump_takes_the_same_restart ... ok
test the_reentry_state_holds_the_newton_history_and_the_controller ... ok
test reentry_from_any_captured_step_matches_the_continuous_run ... ok
test a_reentry_from_every_recorded_step_continues_bit_for_bit ... ok
test the_default_reentry_keeps_every_step_state ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.83s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 2.05s
     Running tests/transient_reentry.rs (/var/tmp/piperine-target-c/debug/deps/transient_reentry-b3380ea61d4187fd)
```

`Run failure` - exit 0

```
test section_18_a_negative_pre_roll_is_an_analysis_error ... ok
test section_15_a_non_finite_stamp_is_a_convergence_failure ... ok
test section_12_unresolvable_noise_output_node_is_an_analysis_error ... ok
test section_18_a_non_positive_period_is_an_analysis_error ... ok
test section_12_unresolvable_noise_reference_node_is_an_analysis_error ... ok
test a_step_that_fails_newton_at_the_floor_is_e0508_with_its_time ... ok
test a_jump_step_that_fails_at_the_floor_is_e0504_naming_the_carrier ... ok
test an_equilibrium_that_never_converges_is_e0505_with_its_unknowns ... ok
test section_9_exhausting_every_dc_strategy_is_a_convergence_failure ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.79s
     Running tests/failure_rules.rs (/var/tmp/piperine-target-c/debug/deps/failure_rules-109848642e3c883b)
```

`Run runerrors` - exit 0

```
running 3 tests
test a_nan_contribution_from_source_is_e0507 ... ok
test a_floating_node_from_source_is_e0503 ... ok
test a_loop_of_two_vsources_from_source_is_e0503 ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.36s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/run_errors.rs (/var/tmp/piperine-target-c/debug/deps/run_errors-788896b7fa74cc14)
```

`Run spectext` - exit 0

```
test the_testbench_table_splits_stimulus_and_host_work ... ok
test tran_in_fences_is_the_module ... ok
test s36_defines_the_package_codes ... ok
test every_error_code_is_defined_exactly_once ... ok
test zero_bytes_are_the_unknown_value ... ok
test no_spec_text_says_stimulus_is_host_only ... ok

test result: ok. 86 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-ir)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-plan)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-lang)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-transpile)
    Finished `test` profile [unoptimized] target(s) in 35.03s
     Running tests/spec_text.rs (/var/tmp/piperine-target-c/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run truncation` - exit 0

```
running 7 tests
test its_device_passes_the_partials_verifier ... ok
test its_corner_source_passes_the_partials_verifier ... ok
test the_floor_at_the_stop_time_names_that_time ... ok
test the_floor_is_retried_once_before_failing ... ok
test an_unannounced_corner_fails_e0508_at_the_floor ... ok
test a_larger_chgtol_takes_fewer_steps ... ok
test a_source_driven_nonlinear_capacitor_meets_the_current_tolerance ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.30s
     Running tests/truncation.rs (/var/tmp/piperine-target-c/debug/deps/truncation-5ab22096ccc0419d)
```

`Run twins` - exit 0

```
running 5 tests
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_three_op_twins_run_on_both_backends ... ok
test the_two_backends_print_the_same_bytes ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.44s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-c/debug/deps/twins-9c63cef8d18c33f2)
```

`Run abidc` - exit 0

```
running 6 tests
test diode_satisfies_its_own_equation_on_devices ... ok
test gmin_stepping_reaches_a_device_through_the_ambient_input ... ok
test source_stepping_scales_sources_through_the_ambient_input ... ok
test its_devices_pass_the_partials_verifier ... ok
test the_dc_driver_iterates_with_the_plans_strategy ... ok
test divider_reads_the_analytic_ratio_on_devices ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 4.93s
     Running tests/abi_dc.rs (/var/tmp/piperine-target-c/debug/deps/abi_dc-61ee0ff6405ae32a)
```

`Run abitran` - exit 0

```
test a_continuous_init_starts_newton_and_is_held_through_the_operating_point ... ok
test the_transient_driver_iterates_with_the_plans_strategy ... ok
test a_rejected_stage_restores_the_term_history ... ok
test two_jumps_in_one_restart_step_restart_from_the_later ... ok
test rc_step_reads_one_minus_exp_on_devices ... ok
test the_step_after_a_jump_is_capped_at_a_tenth_of_the_last ... ok
test a_stop_on_the_tick_grid_ends_on_its_own_step ... ok
test a_stop_off_the_tick_grid_is_reached_once_by_an_integrating_step ... ok
test the_stop_is_reached_bit_exactly_and_recorded_once ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 3.45s
     Running tests/abi_tran.rs (/var/tmp/piperine-target-c/debug/deps/abi_tran-4aeba793e31ffa8a)
```

`Run protocol` - exit 0

```
test acceptance_fires_crossings_runs_the_instant_and_asks_for_restart ... ok
test an_earlier_landing_restores_and_replays ... ok
test an_off_grid_step_lands_on_the_tick_grid ... ok
test crossing_retries_count_toward_the_rejection_cap ... ok
test no_landing_means_one_attempt_to_the_proposed_time ... ok
test no_unknowns_means_no_attempt ... ok
test settle_runs_at_time_points_and_at_the_stop_time_only ... ok
test the_proposal_sees_the_next_due_time ... ok
test consecutive_rejections_beyond_the_cap_fail_loud ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.42s
     Running tests/protocol_transient.rs (/var/tmp/piperine-target-c/debug/deps/protocol_transient-d72a4e415285a9b3)
```

`Run boundary` - exit 0

```
running 3 tests
test the_adc_commits_the_rounded_code_after_tconv ... ok
test a_carrier_change_is_a_time_point_and_starts_the_response ... ok
test the_comparator_trips_at_its_thresholds ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.51s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/boundary.rs (/var/tmp/piperine-target-c/debug/deps/boundary-8d6961f951687c76)
```

`Run hostset` - exit 0

```
running 5 tests
test its_devices_pass_the_partials_verifier ... ok
test a_structural_set_during_a_run_is_refused ... ok
test a_false_check_refuses_a_set ... ok
test reshape_is_refused_in_a_transient_and_rebinds_between_analyses ... ok
test a_scheduled_host_write_lands_and_restarts_small ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 1.57s
     Running tests/host_set.rs (/var/tmp/piperine-target-c/debug/deps/host_set-5701f03124c1a69a)
```

`Run instant` - exit 0

```
test a_plan_with_no_code_applies_host_commits_only ... ok
test a_settle_commit_with_no_delay_is_refused ... ok
test a_quiet_commit_does_not_start_a_microstep ... ok
test start_commits_inits_and_fires_start ... ok
test start_at_commits_init_at_its_tick ... ok
test two_commits_to_one_net_settle_to_one_write ... ok
test the_traced_instant_of_section_15_1 ... ok
test previous_is_the_value_before_the_microstep ... ok
test a_settled_change_is_one_observation ... ok
test an_instant_that_never_goes_quiet_is_e0501_naming_the_nets ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/instant.rs (/var/tmp/piperine-target-c/debug/deps/instant-cf6184e72773bc7c)
```

`Run removed` - exit 0

```
test disto_is_gone ... ok
test simprobe_is_gone ... ok
test verification_channels_are_gone ... ok
test the_phdl_port_text_is_gone ... ok
test regions_are_gone ... ok
test text_io_is_gone ... ok
test the_element_contract_is_gone ... ok
test the_old_digital_engine_is_gone ... ok
test removed_tests_are_listed_with_their_item ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 0.67s
     Running tests/removed_surface.rs (/var/tmp/piperine-target-c/debug/deps/removed_surface-b915945964cdf246)
```

`Run abidevices` - exit 0

```
running 5 tests
test a_check_of_another_module_does_not_count ... ok
test every_device_shape_is_valid ... ok
test every_device_function_passes_the_partials_verifier ... ok
test every_helper_device_passes_at_generic_points ... ok
test every_hand_written_device_is_in_a_partials_check ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-solver-hygiene/crates/piperine-solver)
    Finished `test` profile [unoptimized] target(s) in 3.65s
     Running tests/abi_devices.rs (/var/tmp/piperine-target-c/debug/deps/abi_devices-c8f186128d2c66bd)
```

`Run digital` - exit 0

```
running 7 tests
test every_change_commit_and_record_is_observed_in_order ... ok
test observations_after_a_checkpoint_are_gone_after_restore ... ok
test nested_marks_rewind_in_order ... ok
test restore_returns_store_queue_and_observations_byte_for_byte ... ok
test two_runs_give_identical_observations ... ok
test a_mark_journals_only_the_nets_that_changed ... ok
test a_rewind_equals_the_checkpoint_taken_at_the_mark ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/checkpoint.rs (/var/tmp/piperine-target-c/debug/deps/checkpoint-bffa3fb7b4b6a608)
```

`! grep -nE '^\/ V(1/2/3/4/5/6/7/8/9/10/11/14/15) \/' GAPS.md && grep -cE '^\/ V1[236] \/' GAPS.md / grep -qx 3` - exit 0

```
(no output)
```

`grep -q '^### MD-68' .specs/STATE.md` - exit 0

```
(no output)
```

`git log --format=%B --grep='the truncation test measures the ddt states' / grep -c 'tran_rc' / grep -qE '^[2-9` - exit 0

```
(no output)
```
