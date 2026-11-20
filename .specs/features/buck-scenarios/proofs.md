# buck-scenarios proofs

**HEAD**: 6bfe5454
**Tree**: clean
**Shell**: /bin/sh
**Result**: 79 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run scenarios / test the_package_declares_the_five_scenario_runs ... ok` | pass |
| C2 | `run scenarios / test every_run_samples_the_thirteen_signals_on_a_10_ns_grid ... ok` | pass |
| C3 | `run scenarios / test every_run_accepts_the_five_ports_and_three_params ... ok` | pass |
| C4 | `run scenarios / test the_scenario_package_checks_with_no_diagnostic ... ok` | pass |
| C5 | `run clibuck / test the_buck_has_its_bench_and_two_runs ... ok` | pass |
| C6 | `run scenarios / test opening_a_run_leaves_only_the_package_sources ... ok` | pass |
| C7 | `run harness / test scenario::column::tests::a_missing_column_panics_naming_the_name_and_the_columns ... ok` | pass |
| C8 | `run harness / test scenario::trace::tests::settled_into_is_the_first_row_from_which_every_row_stays_inside ...` | pass |
| C9 | `run harness / test scenario::trace::tests::deviation_is_the_largest_absolute_difference_with_its_time ... ok` | pass |
| C10 | `run harness / test scenario::trace::tests::peak_to_peak_is_the_largest_minus_the_smallest ... ok` | pass |
| C11 | `run harness / test scenario::trace::tests::mean_is_the_arithmetic_mean_of_the_rows ... ok` | pass |
| C12 | `run harness / test scenario::trace::tests::first_crossing_is_the_first_row_at_or_past_the_level_after_a_time .` | pass |
| C13 | `run harness / test scenario::discrete::tests::first_time_is_the_first_row_holding_the_value_after_a_time ... o` | pass |
| C14 | `run harness / test scenario::discrete::tests::holds_names_the_first_row_that_departs ... ok` | pass |
| C15 | `run harness / test scenario::stimulus::tests::a_ramp_sets_once_per_equal_step_and_ends_on_its_target ... ok` | pass |
| C16 | `run scenarios / test setpoint_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok` | pass |
| C17 | `run scenarios / test s2_reference_reaches_800_codes_0_544_ms_after_the_write ... ok` | pass |
| C18 | `run scenarios / test s2_vout_settles_into_5_v_within_1_percent_at_0_54_ms ... ok` | pass |
| C19 | `run scenarios / test s2_overshoot_is_at_most_15_mv ... ok` | pass |
| C20 | `run scenarios / test s2_pgood_stays_high ... ok` | pass |
| C21 | `run scenarios / test s2b_reference_reaches_160_codes_1_28_ms_after_the_write ... ok` | pass |
| C22 | `run scenarios / test s2b_vout_settles_into_1_v_within_1_percent_at_1_38_ms ... ok` | pass |
| C23 | `run scenarios / test s2b_inductor_current_goes_negative ... ok` | pass |
| C24 | `run scenarios / test s2b_sink_limit_never_trips ... ok` | pass |
| C25 | `run scenarios / test s2b_no_over_voltage_fault ... ok` | pass |
| C26 | `run scenarios / test setpoint_passes_every_check ... ok` | pass |
| C27 | `run scenarios / test the_families_run_at_once_in_their_own_build_directories ... ok` | pass |
| C28 | `run scenarios / test every_family_reports_its_wall_time_and_rows ... ok` | pass |
| C29 | `run harness / test scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run ... ok` | pass |
| C30 | `run scenarios / test load_line_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok` | pass |
| C31 | `run scenarios / test s3_load_step_up_stays_within_165_mv ... ok` | pass |
| C32 | `run scenarios / test s3_load_step_down_stays_within_165_mv ... ok` | pass |
| C33 | `run scenarios / test s3_load_step_up_recovers_within_200_us ... ok` | pass |
| C34 | `run scenarios / test s3_load_step_down_recovers_within_200_us ... ok` | pass |
| C35 | `run scenarios / test s3_pgood_stays_high ... ok` | pass |
| C36 | `run scenarios / test s3_no_current_limit ... ok` | pass |
| C37 | `run scenarios / test s4_line_step_up_stays_within_165_mv ... ok` | pass |
| C38 | `run scenarios / test s4_line_step_down_stays_within_165_mv ... ok` | pass |
| C39 | `run scenarios / test s4_line_step_up_recovers_within_200_us ... ok` | pass |
| C40 | `run scenarios / test s4_line_step_down_recovers_within_200_us ... ok` | pass |
| C41 | `run scenarios / test s4_no_fault ... ok` | pass |
| C42 | `run scenarios / test s8_off_at_the_sample_after_en_falls ... ok` | pass |
| C43 | `run scenarios / test s8_both_switches_stay_off ... ok` | pass |
| C44 | `run scenarios / test s8_pgood_low_from_off ... ok` | pass |
| C45 | `run scenarios / test s8_vout_decays_with_the_load_time_constant ... ok` | pass |
| C46 | `run scenarios / test load_line_passes_every_check ... ok` | pass |
| C47 | `run scenarios / test faults_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok` | pass |
| C48 | `run scenarios / test s5_every_period_is_cut_until_hiccup ... ok` | pass |
| C49 | `run scenarios / test s5_inductor_current_stays_at_or_below_7_6_a ... ok` | pass |
| C50 | `run scenarios / test s5_hiccup_with_over_current_26_5_to_30_5_us_after_the_step ... ok` | pass |
| C51 | `run scenarios / test s5_pgood_falls_within_8_us_of_the_deviation ... ok` | pass |
| C52 | `run scenarios / test s5_both_switches_stay_off_in_hiccup ... ok` | pass |
| C53 | `run scenarios / test s5_retry_comes_2_ms_after_the_hiccup_with_the_overload_on ... ok` | pass |
| C54 | `run scenarios / test s5_retry_without_the_overload_regulates_and_clears_the_fault ... ok` | pass |
| C55 | `run scenarios / test s6_sink_limit_trips ... ok` | pass |
| C56 | `run scenarios / test s6_inductor_current_is_held_at_minus_3_a ... ok` | pass |
| C57 | `run scenarios / test s6_over_voltage_within_two_samples_of_3_81_v ... ok` | pass |
| C58 | `run scenarios / test s6_reports_its_timing_and_rise_rate ... ok` | pass |
| C59 | `run scenarios / test s6_both_switches_stay_off_after_over_voltage ... ok` | pass |
| C60 | `run scenarios / test faults_passes_every_check ... ok` | pass |
| C61 | `run scenarios / test brownout_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok` | pass |
| C62 | `run scenarios / test s7_off_at_the_sample_after_vin_falls_through_8_v ... ok` | pass |
| C63 | `run scenarios / test s7_both_switches_off_while_off ... ok` | pass |
| C64 | `run scenarios / test s7_pgood_low_while_off ... ok` | pass |
| C65 | `run scenarios / test s7_fault_unchanged_while_off ... ok` | pass |
| C66 | `run scenarios / test s7_soft_start_at_the_sample_after_vin_rises_through_8_5_v ... ok` | pass |
| C67 | `run scenarios / test brownout_passes_every_check ... ok` | pass |
| C68 | `run scenarios / test s9_dc_error_within_section_4_3_at_every_corner ... ok` | pass |
| C69 | `run scenarios / test s9_ripple_at_most_10_mv_at_every_corner ... ok` | pass |
| C70 | `run scenarios / test s9_no_idle_tone_above_1_code_at_every_corner ... ok` | pass |
| C71 | `run scenarios / test s9_every_corner_is_settled ... ok` | pass |
| C72 | `run scenarios / test steady_state_passes_every_check ... ok` | pass |
| C73 | `run supervisor / test each_hiccup_lasts_1000_samples ... ok` | pass |
| C74 | `run docs / test the_buck_spec_holds_the_bench_bounds ... ok` | pass |
| C75 | `run docs / test the_buck_readme_points_at_the_scenarios ... ok` | pass |
| C76 | `run docs / test gaps_narrows_what_buck_scenarios_ran ... ok` | pass |
| Build | `cargo build -p piperine -p piperine-parity -p piperine-api -p piperine-cli --all-targets 2>&1 / tee /dev/stder` | pass |
| Build | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| Build | `cargo test -p piperine-solver --test workspace_shape` | pass |

## Output

`Run scenarios` - exit 0

```
S9 corner 4 (Vin 16 V, 3.3 V): mean error 2.94 mV (bound 14.7 mV), ripple 3.95 mV (10 mV), code swing 0 (1), half-window drift 0.17 mV (6.25 mV), window 3.458e-3 s to 3.522e-3 s
S9 corner 5 (Vin 12 V, 3.3 V): mean error 3.51 mV (bound 14.7 mV), ripple 6.18 mV (10 mV), code swing 1 (1), half-window drift 0.42 mV (6.25 mV), window 4.172e-3 s to 4.236e-3 s
S9 corner 6 (Vin 9 V, 3.3 V): mean error 1.61 mV (bound 14.7 mV), ripple 4.04 mV (10 mV), code swing 0 (1), half-window drift 0.62 mV (6.25 mV), window 4.886e-3 s to 4.9499999999999995e-3 s
S9 corner 7 (Vin 9 V, 5 V): mean error 0.38 mV (bound 20.0 mV), ripple 3.02 mV (10 mV), code swing 0 (1), half-window drift 0.00 mV (6.25 mV), window 5.945999999999999e-3 s to 6.009999999999999e-3 s
S9 corner 8 (Vin 12 V, 5 V): mean error 1.20 mV (bound 20.0 mV), ripple 5.09 mV (10 mV), code swing 0 (1), half-window drift 0.64 mV (6.25 mV), window 6.659999999999998e-3 s to 6.723999999999998e-3 s
S9 corner 9 (Vin 16 V, 5 V): mean error 1.42 mV (bound 20.0 mV), ripple 8.64 mV (10 mV), code swing 1 (1), half-window drift 2.78 mV (6.25 mV), window 7.373999999999998e-3 s to 7.4379999999999976e-3 s
S9 corner 1 (Vin 9 V, 0.8 V): mean error 0.70 mV (bound 6.5 mV), ripple 1.26 mV (10 mV), code swing 0 (1), half-window drift 0.05 mV (6.25 mV), window 7.14e-4 s to 7.78e-4 s
S9 corner 2 (Vin 12 V, 0.8 V): mean error 0.63 mV (bound 6.5 mV), ripple 1.47 mV (10 mV), code swing 0 (1), half-window drift 0.11 mV (6.25 mV), window 1.428e-3 s to 1.492e-3 s
S9 corner 3 (Vin 16 V, 0.8 V): mean error 1.36 mV (bound 6.5 mV), ripple 1.36 mV (10 mV), code swing 0 (1), half-window drift 0.01 mV (6.25 mV), window 2.1420000000000002e-3 s to 2.206e-3 s
S9 corner 4 (Vin 16 V, 3.3 V): mean error 2.94 mV (bound 14.7 mV), ripple 3.95 mV (10 mV), code swing 0 (1), half-window drift 0.17 mV (6.25 mV), window 3.458e-3 s to 3.522e-3 s
S9 corner 5 (Vin 12 V, 3.3 V): mean error 3.51 mV (bound 14.7 mV), ripple 6.18 mV (10 mV), code swing 1 (1), half-window drift 0.42 mV (6.25 mV), window 4.172e-3 s to 4.236e-3 s
S9 corner 6 (Vin 9 V, 3.3 V): mean error 1.61 mV (bound 14.7 mV), ripple 4.04 mV (10 mV), code swing 0 (1), half-window drift 0.62 mV (6.25 mV), window 4.886e-3 s to 4.9499999999999995e-3 s
S9 corner 7 (Vin 9 V, 5 V): mean error 0.38 mV (bound 20.0 mV), ripple 3.02 mV (10 mV), code swing 0 (1), half-window drift 0.00 mV (6.25 mV), window 5.945999999999999e-3 s to 6.009999999999999e-3 s
S9 corner 8 (Vin 12 V, 5 V): mean error 1.20 mV (bound 20.0 mV), ripple 5.09 mV (10 mV), code swing 0 (1), half-window drift 0.64 mV (6.25 mV), window 6.659999999999998e-3 s to 6.723999999999998e-3 s
S9 corner 9 (Vin 16 V, 5 V): mean error 1.42 mV (bound 20.0 mV), ripple 8.64 mV (10 mV), code swing 1 (1), half-window drift 2.78 mV (6.25 mV), window 7.373999999999998e-3 s to 7.4379999999999976e-3 s
```

`Run clibuck` - exit 0

```
running 1 test
test the_buck_has_its_bench_and_two_runs ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.27s
     Running tests/buck.rs (/var/tmp/piperine-target-a/debug/deps/buck-e0a5ced46b7d0d71)
```

`Run harness` - exit 0

```
test scenario::trace::tests::first_crossing_is_the_first_row_at_or_past_the_level_after_a_time ... ok
test scenario::trace::tests::between_keeps_half_open_windows_and_an_empty_window_panics_naming_it ... ok
test scenario::trace::tests::peak_to_peak_is_the_largest_minus_the_smallest ... ok
test scenario::trace::tests::scaled_multiplies_every_value ... ok
test scenario::trace::tests::settled_into_is_the_first_row_from_which_every_row_stays_inside ... ok
test scenario::trace::tests::decay_time_constant_recovers_an_exponential ... ok
test scenario::stimulus::tests::a_refused_action_is_returned_naming_it ... ok
test scenario::stimulus::tests::a_stimulus_is_applied_to_the_session_and_its_drives_and_sets_land ... ok
test scenario::run::tests::each_run_builds_in_its_own_directory ... ok
test scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 25 filtered out; finished in 4.08s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running unittests src/lib.rs (/var/tmp/piperine-target-a/debug/deps/piperine_parity-6187a033365891a7)
```

`Run supervisor` - exit 0

```
running 3 tests
test power_good_asserts_after_64_and_drops_after_4 ... ok
test the_supervisor_follows_its_state_table ... ok
test each_hiccup_lasts_1000_samples ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.66s

    Finished `test` profile [unoptimized] target(s) in 0.12s
     Running tests/buck_supervisor.rs (/var/tmp/piperine-target-a/debug/deps/buck_supervisor-ef1d5af335e10bba)
```

`Run docs` - exit 0

```
test state_records_the_cli_decision ... ok
test the_roadmap_and_state_record_msl_fixes ... ok
test the_instruction_files_say_the_host_crates_build ... ok
test the_roadmap_and_state_record_analog_structure ... ok
test state_records_the_unknown_semantics_decision ... ok
test state_records_the_derived_param_decision ... ok
test state_records_the_observation_decision ... ok
test the_roadmap_marks_the_cli_item_done ... ok
test gaps_drops_what_msl_fixes_closed ... ok
test the_roadmap_and_state_record_root_e2e ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized] target(s) in 0.16s
     Running tests/docs.rs (/var/tmp/piperine-target-a/debug/deps/docs-93bbd657b5852db6)
```
