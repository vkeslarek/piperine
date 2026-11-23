# transient-performance proofs

**HEAD**: 5f0494fb
**Tree**: clean
**Shell**: /bin/sh
**Result**: 60 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run solver / test the_protocol_counts_one_landing_of_each_cause ... ok` | pass |
| C2 | `run solver / test the_analog_side_reports_rejections_by_cause_and_jump_steps ... ok` | pass |
| C3 | `run protocol / test the_ends_sum_to_the_accepted_steps ... ok` | pass |
| C4 | `run apistats / test a_session_returns_the_statistics_of_its_last_transient ... ok` | pass |
| C5 | `run bucklong / test the_statistics_of_the_whole_start_are_reported ... ok` | pass |
| C6 | `run digital / test a_falling_clock_heard_only_by_a_rising_edge_reaction_makes_no_landing ... ok` | pass |
| C7 | `run digital / test a_commit_that_changes_a_carrier_the_analog_side_reads_lands_in_both_rules ... ok` | pass |
| C8 | `run digital / test a_reaction_that_reads_a_continuous_pin_lands_in_both_rules ... ok` | pass |
| C9 | `run digital / test a_net_no_reaction_that_reaches_the_analog_side_reads_makes_no_landing_in_both_rules ... ok` | pass |
| C10 | `run digital / test a_write_to_a_reaching_net_that_changes_nothing_lands_only_under_the_declared_rule ... ok` | pass |
| C11 | `run digital / test a_reaction_on_settle_keeps_every_declared_landing ... ok` | pass |
| C12 | `run digital / test a_host_waiter_keeps_every_declared_landing ... ok` | pass |
| C13 | `run digital / test a_change_found_at_a_later_microstep_rewinds_the_instant_and_leaves_no_record ... ok` | pass |
| C14 | `run digital / test a_commit_delayed_by_an_expression_makes_no_landing_at_its_making ... ok` | pass |
| C15 | `run buckacc / test the_first_100_us_land_at_most_1000_times ... ok` | pass |
| C16 | `run solver / test a_landing_step_over_the_tolerance_is_rejected_and_retaken_smaller ... ok` | pass |
| C17 | `run solver / test the_instants_run_ahead_of_a_rejected_step_leave_no_record ... ok` | pass |
| C18 | `run spectext / test appendix_c_2_tests_the_landing_step_and_the_jump_step ... ok` | pass |
| C18 | `run spectext / test appendix_d_and_f_restart_rows_say_the_step_to_the_landing_is_ordinary ... ok` | pass |
| C19 | `run integration / test the_jump_estimate_of_a_quadratic_state_is_backward_eulers_error ... ok` | pass |
| C20 | `run solver / test a_jump_step_over_the_tolerance_is_retaken_smaller ... ok` | pass |
| C21 | `run solver / test a_jump_step_at_the_floor_is_accepted_and_counted ... ok` | pass |
| C22 | `run solver / test an_ideal_step_is_read_by_an_rc_to_one_minus_e_to_the_minus_one ... ok` | pass |
| C23 | `run buckacc / test the_first_100_us_match_the_fine_reference_within_0_104_mv ... ok` | pass |
| C24 | `run spectext / test part_v_rule_6_tests_the_step_that_reads_a_jump_with_the_first_order_estimate ... ok` | pass |
| C25 | `run bucklong / test the_whole_start_takes_at_most_300000_steps ... ok` | pass |
| C26 | `run bucklong / test the_overshoot_is_within_15_mv ... ok` | pass |
| C27 | `run bucklong / test the_start_ends_in_regulation_with_power_good ... ok` | pass |
| C28 | `run solver / test the_declared_rule_lands_on_every_clock_edge_and_the_observable_rule_on_the_input_changes ...` | pass |
| C29 | `run bucklong / test the_wall_time_and_the_peak_are_reported ... ok` | pass |
| C30 | `run scenarios / test the_package_declares_the_five_scenario_runs ... ok` | pass |
| C31 | `run abialloc / test the_second_eval_of_a_function_allocates_nothing ... ok` | pass |
| C32 | `run alloc / test the_second_stamping_of_each_stage_into_a_kept_buffer_allocates_nothing ... ok` | pass |
| C33 | `run newton / test without_timing_a_solve_reads_no_clock ... ok` | pass |
| C34 | `run newton / test with_timing_a_solve_reports_both_totals ... ok` | pass |
| C35 | `run solver / test a_pure_analog_circuit_runs_no_instant ... ok` | pass |
| C36 | `run buckacc / test the_first_100_us_run_an_instant_only_where_something_is_due ... ok` | pass |
| C37 | `run solver / test the_open_loop_fixture_gives_the_same_observations_in_both_rules ... ok` | pass |
| C38 | `run solver / test the_instants_run_ahead_of_a_rejected_step_leave_no_record ... ok` | pass |
| C39 | `run solver / test the_observations_of_both_rules_come_in_time_order ... ok` | pass |
| C40 | `run parity / test tran_rc ... ok` | pass |
| C41 | `env PATH=/usr/bin/nonexistent:$HOME/.cargo/bin:/bin cargo test -p piperine-parity --test parity_tran -- --noca` | pass |
| C42 | `run docs / test the_scenario_package_states_its_measured_step_count ... ok` | pass |
| C43 | `run docs / test transient_performance_closes_v12 ... ok` | pass |
| C44 | `run spectext / test appendix_c_2_time_points_row_names_the_dynamic_test_its_exceptions_and_the_reference_mode ` | pass |
| C45 | `run docs / test transient_performance_closes_v12 ... ok` | pass |
| C45 | `run docs / test the_scenario_package_states_its_measured_step_count ... ok` | pass |
| C46 | `run twins / test the_two_backends_print_the_same_bytes ... ok` | pass |
| C47 | `run grid / test parity_rc_transient ... ok` | pass |
| C48 | `git diff --quiet $(git merge-base origin/main HEAD) -- crates/piperine-parity/tests crates/piperine-parity/src` | pass |
| C49 | `run bucklong / test the_statistics_of_the_whole_start_are_reported ... ok` | pass |
| C50 | `run epoch / test past_2_52_ticks_the_analog_side_is_told_small_times_since_the_epoch ... ok` | pass |
| C51 | `run epoch / test a_step_of_one_tick_is_one_tick_long_past_2_52_ticks ... ok` | pass |
| C52 | `run epoch / test a_run_under_2_52_ticks_never_moves_its_epoch ... ok` | pass |
| C53 | `run options / test an_hour_long_transient_runs_at_the_default_resolution ... ok` | pass |
| C54 | `run hostset / test a_scheduled_host_write_lands_and_restarts_small ... ok` | pass |
| C55 | `run twins / test every_twin_prints_the_bytes_it_printed_before_the_move ... ok` | pass |
| Build | `cargo build -p piperine -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solve` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape` | pass |

## Output

`Run solver` - exit 0

```
test a_rejected_attempt_adds_no_end ... ok
test the_protocol_counts_one_landing_of_each_cause ... ok
test a_step_at_max_step_is_counted ... ok
test the_result_returns_the_protocols_statistics ... ok
test one_commit_due_runs_one_instant ... ok
test the_analog_side_reports_rejections_by_cause_and_jump_steps ... ok
test a_pure_analog_circuit_runs_no_instant ... ok
test a_reaction_on_settle_runs_an_instant_at_every_accepted_step ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/jump_step.rs (/var/tmp/piperine-target-d/debug/deps/jump_step-bc161e92257a6de8)
     Running tests/landing_rule.rs (/var/tmp/piperine-target-d/debug/deps/landing_rule-7fb0b5e84b1110bc)
     Running tests/step_statistics.rs (/var/tmp/piperine-target-d/debug/deps/step_statistics-99d548359e8b3b79)
```

`Run protocol` - exit 0

```
running 4 tests
test protocol::statistics::tests::the_ends_sum_to_the_accepted_steps ... ok
test protocol::statistics::tests::instants_and_retakes_are_counted_apart_from_ends ... ok
test protocol::statistics::tests::the_sampling_share_is_continuous_read_landings_over_accepted_steps ... ok
test protocol::statistics::tests::landings_read_the_landing_ends ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 34 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running unittests src/lib.rs (/var/tmp/piperine-target-d/debug/deps/piperine_solver-5bd475c9257037c3)
```

`Run apistats` - exit 0

```
test the_one_portless_module_is_the_top ... ok
test a_session_returns_the_statistics_of_its_last_transient ... ok
test a_top_with_a_node_port_is_refused_by_the_interpreter_naming_the_host_source ... ok
test a_missing_rustc_names_rustc_and_the_interpreter ... ok
test the_top_key_names_the_module ... ok
test the_interpreter_refusal_names_the_body_and_the_rust_backend ... ok
test no_top_or_several_is_p0011_with_candidates ... ok
test a_module_typed_param_is_not_set_by_text ... ok
test a_top_with_discrete_in_ports_runs_with_them_unknown ... ok
test a_run_compiles_once_and_never_during_an_analysis ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.78s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/session.rs (/var/tmp/piperine-target-d/debug/deps/session-b8872f77132e3cb3)
```

`Run bucklong` - exit 0

```
buck statistics: steps ended at crossing: 0
buck statistics: steps ended at max_step: 56809
buck statistics: steps ended at cap: 0
buck statistics: steps ended at stop: 1
buck statistics: steps ended at proposal: 150711
buck statistics: rejected by truncation 2468
buck statistics: rejected by Newton 0
buck statistics: steps retaken toward a crossing 0
buck statistics: jump steps 2367, jump retakes 891
buck statistics: steps at max_step 56809
buck statistics: instants run 2918
buck statistics: sampling share 0.001427
settling: buck.vout inside 0.8 V +- 10 mV from 2.6287e-4 s (window 207.5 to 307.5 us)
ripple: 1.494120 mV peak to peak over 4001 rows of the last 40 us (limit 10 mV)
overshoot: largest buck.vout 0.807500 V at 2.8444e-4 s (limit 0.815 V)
```

`Run digital` - exit 0

```
test an_irreversible_reaction_lands_in_both_rules ... ok
test horizon_is_before_the_earliest_pending_commit_and_the_limit ... ok
test only_an_irreversible_reaction_and_a_host_waiter_are_not_time_points ... ok
test each_clause_of_the_time_point_test_stops_advance ... ok
test the_landing_test_gives_no_cause_when_nothing_observable_fires ... ok
test the_landing_test_names_the_first_cause_that_holds ... ok
test tick_is_the_instants_tick ... ok
test two_commits_on_one_tick_land_once ... ok
test times_that_round_to_one_tick_are_one_instant ... ok
test a_free_running_clock_does_not_stop_advance ... ok

test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.04s
     Running tests/advance.rs (/var/tmp/piperine-target-d/debug/deps/advance-20cc610a4c5991b3)
```

`Run buckacc` - exit 0

```
test the_first_100_us_run_an_instant_only_where_something_is_due has been running for over 60 seconds
test the_digital_columns_equal_the_fine_reference_row_for_row ... ok
test the_first_100_us_land_at_most_1000_times ... ok
test the_first_100_us_run_an_instant_only_where_something_is_due ... ok
test the_first_100_us_match_the_fine_reference_within_0_104_mv ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 83.66s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/buck_accuracy.rs (/var/tmp/piperine-target-d/debug/deps/buck_accuracy-656f3942a9d7db4a)
buck accuracy: 31264 accepted steps (417 landings, 0 instants run for crossings, 668 scheduled) at max_step 1e-8, 1012195 at 1e-10
buck accuracy: 442 rejected by truncation, 0 by Newton, 0 retaken toward a crossing, 21320 ended on the controller's proposal, 9526 at max_step, 367 jump steps
buck landings: 417 in the first 100 us (limit 1000)
buck instants: 668 run in the first 100 us for 31264 accepted steps (0 for crossings, 668 scheduled, 417 landings; limit 1000)
buck accuracy: largest buck.vout difference 0.000136 mV at 9.999999999999999e-5 s (limit 0.104 mV)
```

`Run spectext` - exit 0

```
test the_step_that_reads_a_jump_is_first_order ... ok
test the_library_declares_no_net_nature ... ok
test tran_in_fences_is_the_module ... ok
test section_22_no_longer_says_std_is_in_the_earlier_spelling ... ok
test there_is_no_slice_and_no_borrow_in_the_grammar ... ok
test zero_bytes_are_the_default_value ... ok
test no_spec_text_says_stimulus_is_host_only ... ok
test every_error_code_is_defined_exactly_once ... ok
test the_removed_words_are_absent_from_the_specification ... ok
test part_v_the_step_to_t_solves_first_on_the_left_value_in_every_place_that_says_so ... ok

test result: ok. 128 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/spec_text.rs (/var/tmp/piperine-target-d/debug/deps/spec_text-909ae6bbf798e8b5)
```

`Run integration` - exit 0

```
test math::integration::tests::the_state_estimate_reads_chgtol ... ok
test math::integration::tests::trapezoid_degenerate_inputs_integrate_to_zero ... ok
test math::integration::tests::trapezoid_non_uniform_grid ... ok
test math::integration::tests::trapezoid_uniform_grid ... ok
test math::integration::tests::trbdf2_bdf2_phase_coeffs_hand_computed ... ok
test math::integration::tests::trbdf2_bdf2_phase_coeffs_match_nonuniform_formula ... ok
test math::integration::tests::trbdf2_gamma_is_two_minus_sqrt2 ... ok
test math::integration::tests::trbdf2_stage_coeffs_degrade_to_backward_euler_on_restart ... ok
test math::integration::tests::trbdf2_stage_coeffs_pass_through_with_history ... ok
test math::integration::tests::trbdf2_trapezoidal_phase_coeffs_match_formula ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.07s
     Running unittests src/lib.rs (/var/tmp/piperine-target-d/debug/deps/piperine_analog_solver-03fa06dcb9685482)
```

`Run scenarios` - exit 0

```
test s7_soft_start_at_the_sample_after_vin_rises_through_8_5_v ... ok
test s5_retry_comes_2_ms_after_the_hiccup_with_the_overload_on ... ok
test s5_retry_without_the_overload_regulates_and_clears_the_fault ... ok
test setpoint_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok
test s5_both_switches_stay_off_in_hiccup ... ok
test the_scenario_package_checks_with_no_diagnostic ... ok
test s9_every_corner_is_settled ... ok
test s9_dc_error_within_section_4_3_at_every_corner ... ok
test s9_no_idle_tone_above_1_code_at_every_corner ... ok
test s9_ripple_at_most_10_mv_at_every_corner ... ok

test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 423.96s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/buck_scenarios.rs (/var/tmp/piperine-target-d/debug/deps/buck_scenarios-909d9ca82de76bc5)
```

`Run abialloc` - exit 0

```
running 2 tests
test a_buffer_shorter_than_its_cell_is_refused_with_the_text_it_always_had ... ok
test the_second_eval_of_a_function_allocates_nothing ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.05s
     Running tests/eval_allocation.rs (/var/tmp/piperine-target-d/debug/deps/eval_allocation-5e542c50b7e65c1c)
```

`Run alloc` - exit 0

```
running 2 tests
test the_second_evaluation_at_a_changed_iterate_allocates_nothing ... ok
test the_second_stamping_of_each_stage_into_a_kept_buffer_allocates_nothing ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/allocation.rs (/var/tmp/piperine-target-d/debug/deps/allocation-52d1c8d264eb37b1)
```

`Run newton` - exit 0

```
running 4 tests
test math::newton_raphson::tests::a_non_finite_solution_of_finite_stamps_is_e0503 ... ok
test math::newton_raphson::tests::without_timing_a_solve_reads_no_clock ... ok
test math::newton_raphson::tests::with_timing_a_solve_reports_both_totals ... ok
test math::newton_raphson::tests::exhaustion_names_the_unknowns_that_failed ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 39 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.07s
     Running unittests src/lib.rs (/var/tmp/piperine-target-d/debug/deps/piperine_analog_solver-03fa06dcb9685482)
```

`Run parity` - exit 0

```
     Running tests/source_fixes.rs (/var/tmp/piperine-target-d/debug/deps/source_fixes-d1ffd97f22c995be)
     Running tests/source_host.rs (/var/tmp/piperine-target-d/debug/deps/source_host-480988613ba19c2b)
     Running tests/source_limexp.rs (/var/tmp/piperine-target-d/debug/deps/source_limexp-e6385f079ae4c5dd)
     Running tests/source_modules.rs (/var/tmp/piperine-target-d/debug/deps/source_modules-196f4d33439a0ca3)
     Running tests/source_natures.rs (/var/tmp/piperine-target-d/debug/deps/source_natures-0e3a717a399fc10c)
     Running tests/source_params.rs (/var/tmp/piperine-target-d/debug/deps/source_params-a40c966bba2b4980)
     Running tests/source_reach.rs (/var/tmp/piperine-target-d/debug/deps/source_reach-e3e0d3243e8f2da5)
     Running tests/source_records.rs (/var/tmp/piperine-target-d/debug/deps/source_records-9927e53ff9bcd459)
     Running tests/source_slew.rs (/var/tmp/piperine-target-d/debug/deps/source_slew-5ffa113f3c8c4d72)
     Running tests/source_switch.rs (/var/tmp/piperine-target-d/debug/deps/source_switch-f80a8a81fdfe14c7)
     Running tests/source_time.rs (/var/tmp/piperine-target-d/debug/deps/source_time-7d1a2d277816dcb9)
     Running tests/source_unknowns.rs (/var/tmp/piperine-target-d/debug/deps/source_unknowns-4723441ecc38021e)
     Running tests/source_value_semantics.rs (/var/tmp/piperine-target-d/debug/deps/source_value_semantics-a54b1ba10ea014c3)
     Running tests/twins.rs (/var/tmp/piperine-target-d/debug/deps/twins-e7c228ccf88fd189)
   Doc-tests piperine_parity
```

`Run docs` - exit 0

```
test state_records_the_derived_param_decision ... ok
test state_records_the_observable_landings_and_the_tick_epoch ... ok
test the_per_level_feature_is_recorded_as_dropped ... ok
test the_scenario_package_states_its_measured_step_count ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test transient_performance_closes_v12 ... ok
test state_holds_the_explicit_unknowns_decision ... ok
test the_roadmap_is_forward_looking ... ok
test abi_md_describes_the_unreleased_contract ... ok
test the_book_no_longer_says_settle_is_not_compiled ... ok

test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/docs.rs (/var/tmp/piperine-target-d/debug/deps/docs-93bbd657b5852db6)
```

`Run twins` - exit 0

```
running 6 tests
test the_three_op_twins_run_on_both_backends ... ok
test the_sweep_and_transient_twins_run_on_both_backends ... ok
test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok
test the_two_backends_print_the_same_bytes ... ok
test every_twin_prints_the_bytes_it_printed_before_the_move ... ok
test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.91s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/twins.rs (/var/tmp/piperine-target-d/debug/deps/twins-9b01bd7200d69219)
```

`Run grid` - exit 0

```
running 7 tests
test its_devices_pass_the_partials_verifier ... ok
test parity_digital_scheduler_snapshot ... ok
test parity_divider_op_dc ... ok
test parity_diode_dc_point ... ok
test parity_mixed_signal_dc_settle ... ok
test parity_rlc_ac_resonant_peak ... ok
test parity_rc_transient ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/parity_baseline.rs (/var/tmp/piperine-target-d/debug/deps/parity_baseline-2aced5317f54915e)
```

`Run epoch` - exit 0

```
running 3 tests
test a_step_of_one_tick_is_one_tick_long_past_2_52_ticks ... ok
test a_run_under_2_52_ticks_never_moves_its_epoch ... ok
test past_2_52_ticks_the_analog_side_is_told_small_times_since_the_epoch ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/protocol_epoch.rs (/var/tmp/piperine-target-d/debug/deps/protocol_epoch-a48eb6ba3368b0b8)
```

`Run options` - exit 0

```
running 1 test
test an_hour_long_transient_runs_at_the_default_resolution ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out; finished in 13.44s

    Finished `test` profile [unoptimized] target(s) in 0.10s
     Running tests/options.rs (/var/tmp/piperine-target-d/debug/deps/options-5b5980cc81321f7a)
```

`Run hostset` - exit 0

```
running 5 tests
test its_devices_pass_the_partials_verifier ... ok
test a_false_check_refuses_a_set ... ok
test a_structural_set_during_a_run_is_refused ... ok
test reshape_is_refused_in_a_transient_and_rebinds_between_analyses ... ok
test a_scheduled_host_write_lands_and_restarts_small ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.08s
     Running tests/host_set.rs (/var/tmp/piperine-target-d/debug/deps/host_set-5701f03124c1a69a)
```

`env PATH=/usr/bin/nonexistent:$HOME/.cargo/bin:/bin cargo test -p piperine-parity --test parity_tran -- --noca` - exit 0

```
SKIP uic_rc: ngspice comparison disabled by PIPERINE_NGSPICE=skip
test tran_floor ... ok
test tran_clipper ... ok
test uic_rc_runs_and_prints_the_skip_line_without_ngspice ... ok
test tran_corner ... ok
test tran_jump ... ok
test uic_rc ... ok
test tran_rc_with_a_scaled_derivative ... ok
test tran_rc ... ok
test tran_dq ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.09s

    Finished `test` profile [unoptimized] target(s) in 0.11s
     Running tests/parity_tran.rs (/var/tmp/piperine-target-d/debug/deps/parity_tran-1f68c93d92106e3e)
```

`git diff --quiet $(git merge-base origin/main HEAD) -- crates/piperine-parity/tests crates/piperine-parity/src` - exit 0

```
(no output)
```
