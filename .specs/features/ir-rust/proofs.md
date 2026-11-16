# ir-rust proofs

**HEAD**: c2234656
**Tree**: clean
**Shell**: /bin/sh
**Result**: 117 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run compile / test the_m1_design_compiles_with_no_error_and_no_warning ... ok` | pass |
| C2 | `run golden / test each_function_is_a_generic_fn_and_each_body_a_device_mod ... ok` | pass |
| C3 | `run golden / test transpiling_twice_gives_the_same_bytes ... ok` | pass |
| C4 | `run lib / test print::value::tests::an_unsupported_form_is_named_with_path_and_origin ... ok` | pass |
| C5 | `run reject / test an_ir_that_fails_check_gives_no_source ... ok` | pass |
| C6 | `run golden / test every_item_sits_under_its_path_and_origin ... ok` | pass |
| C7 | `run lib / test print::text::tests::keywords_become_raw_identifiers ... ok` | pass |
| C8 | `run lib / test print::text::tests::reals_round_trip_in_the_shortest_form ... ok` | pass |
| C8 | `run lib / test print::text::tests::lines_are_indented_by_four_one_statement_each ... ok` | pass |
| C9 | `run compile / test the_source_is_kept_beside_the_library_and_reported ... ok` | pass |
| C10 | `run golden / test the_five_m1_bodies_print_as_the_golden_files ... ok` | pass |
| C11 | `run lib / test settings::tests::the_compiler_is_the_variable_then_the_path ... ok` | pass |
| C12 | `run rustc / test the_compiler_is_run_directly_and_cargo_never ... ok` | pass |
| C13 | `run rustc / test a_missing_compiler_is_named_and_the_interpreter_is_offered ... ok` | pass |
| C14 | `run compile / test a_compile_error_keeps_the_source_and_shows_stderr ... ok` | pass |
| C15 | `run rustc / test a_compiler_below_1_85_is_refused_with_both_versions ... ok` | pass |
| C16 | `run compile / test a_second_build_runs_no_compiler ... ok` | pass |
| C17 | `run compile / test a_changed_ir_is_another_entry ... ok` | pass |
| C18 | `run compile / test another_compiler_version_is_another_entry ... ok` | pass |
| C19 | `run cache / test two_racing_builds_leave_one_entry_and_no_staging ... ok` | pass |
| C20 | `run load / test a_library_whose_bytes_changed_is_not_loaded ... ok` | pass |
| C21 | `run compile / test a_corrupted_library_is_rebuilt ... ok` | pass |
| C22 | `run session / test no_analysis_runs_the_compiler ... ok` | pass |
| C23 | `run session / test a_set_between_analyses_runs_no_compiler_and_applies ... ok` | pass |
| C24 | `run cache / test an_unwritable_directory_is_named ... ok` | pass |
| C25 | `run lib / test settings::tests::the_cache_directory_follows_the_four_rules ... ok` | pass |
| C26 | `run compiled / test modules_come_through_the_symbol_and_bind_like_the_interpreter ... ok` | pass |
| C27 | `run compiled / test loaded_shapes_equal_the_plans_entry_by_entry ... ok` | pass |
| C28 | `run plan / test the_interpreter_and_the_plan_crate_share_one_body_plan ... ok` | pass |
| C29 | `run load / test a_file_that_is_no_library_is_named ... ok` | pass |
| C30 | `run load / test a_library_without_the_symbol_is_named ... ok` | pass |
| C31 | `run load / test a_library_of_a_newer_minor_is_refused ... ok` | pass |
| C31 | `run load / test a_library_smaller_than_the_host_reads_is_refused ... ok` | pass |
| C32 | `run load / test unsafe_lives_in_load_only ... ok` | pass |
| C33 | `run differential / test every_m1_function_gives_the_interpreters_bytes ... ok` | pass |
| C34 | `run lib / test rt::real::tests::dual_gives_the_interpreters_values_and_tangents ... ok` | pass |
| C35 | `run differential / test every_compiled_function_passes_verify_partials ... ok` | pass |
| C36 | `run residue / test the_compiled_resistor_matches_the_hand_written_products_bit_for_bit ... ok` | pass |
| C37 | `run lib / test print::analog::tests::an_affine_term_is_printed_as_constant_plus_fold ... ok` | pass |
| C38 | `run residue / test a_compiled_affine_constant_leaves_at_most_one_ulp ... ok` | pass |
| C39 | `run backends / test both_backends_agree_on_the_m1_circuits ... ok` | pass |
| C40 | `run reaction / test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok` | pass |
| C41 | `run differential / test a_non_finite_value_is_written_as_is ... ok` | pass |
| C42 | `run parity / test divider ... ok` | pass |
| C43 | `run parity / test diode_series ... ok` | pass |
| C44 | `run parity / test isrc_r ... ok` | pass |
| C45 | `run parity / test diode_iv ... ok` | pass |
| C46 | `run parity / test diode_rev ... ok` | pass |
| C47 | `run parity / test ac_rc ... ok` | pass |
| C48 | `run parity / test tran_rc ... ok` | pass |
| C49 | `run backend / test a_rejection_names_the_backend ... ok` | pass |
| C50 | `run backend / test the_rust_bench_runs_loaded_modules ... ok` | pass |
| C51 | `run contract / test contract::tests::the_five_contracts_carry_bounds_and_rationale ... ok` | pass |
| C52 | `run backend / test the_variable_selects_one_backend ... ok` | pass |
| C53 | `run backend / test an_unknown_backend_is_refused_naming_both ... ok` | pass |
| C54 | `run compile / test a_build_reports_its_cost ... ok` | pass |
| C55 | `run irdigital / test integers_and_their_operators_print ... ok` | pass |
| C56 | `run irdigital / test unknown_values_and_their_reads_print ... ok` | pass |
| C57 | `run irdigital / test carriers_and_drives_print ... ok` | pass |
| C58 | `run irdigital / test events_and_reactions_print ... ok` | pass |
| C59 | `run irdigital / test a_defective_digital_body_gives_one_entry_per_defect ... ok` | pass |
| C60 | `run irdigital / test the_m1_prints_are_unchanged ... ok` | pass |
| C61 | `run irdigital / test the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts ... ok` | pass |
| C62 | `run frozen / test a_digital_body_is_rejected_by_name ... ok` | pass |
| C63 | `run lib / test rt::int::tests::arithmetic_wraps_at_every_width ... ok` | pass |
| C64 | `run lib / test rt::int::tests::casts_truncate_and_extend_by_the_source_sign ... ok` | pass |
| C65 | `run lib / test rt::int::tests::a_float_cast_saturates_and_nan_is_zero ... ok` | pass |
| C66 | `run lib / test rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result ... ok` | pass |
| C67 | `run lib / test print::value::tests::a_type_rust_lacks_is_refused_by_name ... ok` | pass |
| C68 | `run lib / test rt::flag::tests::an_unknown_operand_gives_an_unknown_result ... ok` | pass |
| C69 | `run lib / test rt::flag::tests::bool_operators_follow_the_level_rules ... ok` | pass |
| C70 | `run lib / test rt::flag::tests::known_or_is_known_and_high ... ok` | pass |
| C71 | `run reaction / test a_known_or_over_an_unwritten_input_gives_the_default ... ok` | pass |
| C72 | `run lib / test rt::flag::tests::an_unknown_value_is_written_as_zero_bytes ... ok` | pass |
| C73 | `run reaction / test an_unknown_condition_faults_and_commits_nothing ... ok` | pass |
| C74 | `run reaction / test an_unknown_delay_commits_unknown_at_the_next_microstep ... ok` | pass |
| C75 | `run planshape / test carriers_are_discrete_pins_with_a_known_byte ... ok` | pass |
| C76 | `run planshape / test a_drive_is_a_discrete_function ... ok` | pass |
| C77 | `run planshape / test an_event_is_an_event_pin_and_its_function ... ok` | pass |
| C78 | `run planshape / test a_reaction_lists_its_events_reads_and_statements ... ok` | pass |
| C78 | `run planshape / test an_if_lists_its_then_statement_before_its_else ... ok` | pass |
| C79 | `run planshape / test only_a_literal_positive_delay_is_later ... ok` | pass |
| C80 | `run planshape / test a_delayed_drive_is_a_pin_an_event_and_a_reaction ... ok` | pass |
| C81 | `run planshape / test a_cross_event_is_a_watch_with_partials ... ok` | pass |
| C82 | `run planshape / test an_analog_read_of_a_carrier_is_a_read_of_terms ... ok` | pass |
| C83 | `run planshape / test a_probing_reaction_reads_the_continuous_pin ... ok` | pass |
| C84 | `run planshape / test every_buck_slice_shape_validates ... ok` | pass |
| C85 | `run dpwm / test the_oscillator_toggles_every_half_period ... ok` | pass |
| C86 | `run commits / test two_commits_exchange_two_carriers ... ok` | pass |
| C87 | `run commits / test the_later_statement_wins ... ok` | pass |
| C88 | `run commits / test two_statements_make_a_pulse ... ok` | pass |
| C89 | `run commits / test an_unchanged_commit_fires_nothing ... ok` | pass |
| C90 | `run dpwm / test the_dpwm_dithers_keeps_dead_time_and_never_overlaps ... ok` | pass |
| C91 | `run pid / test the_ramp_and_the_pid_follow_their_difference_equations ... ok` | pass |
| C92 | `run boundary / test the_adc_commits_the_rounded_code_after_tconv ... ok` | pass |
| C93 | `run boundary / test the_comparator_trips_at_its_thresholds ... ok` | pass |
| C94 | `run boundary / test a_carrier_change_is_a_time_point_and_starts_the_response ... ok` | pass |
| C95 | `run dpwm / test two_runs_give_the_same_changes ... ok` | pass |
| C96 | `run abisrc / test the_embedded_sources_are_the_abi_crate ... ok` | pass |
| C97 | `run abisrc / test the_abi_builds_with_rustc_alone ... ok` | pass |
| C98 | `run nets / test carrier_ports_bind_to_design_nets ... ok` | pass |
| C99 | `run nets / test every_buck_slice_body_compiles_with_no_warning ... ok` | pass |
| C100 | `run golden / test a_design_with_nothing_to_run_has_no_module ... ok` | pass |
| C101 | `run golden / test a_body_without_analog_prints_no_terms_function ... ok` | pass |
| C102 | `run cache / test two_racing_builds_leave_one_entry_and_no_staging ... ok` | pass |
| C103 | `run golden / test a_keyword_identifier_is_raw_and_the_abi_name_is_not ... ok` | pass |
| C104 | `run differential / test msl_names_that_equal_generated_names_give_the_interpreters_bytes ... ok` | pass |
| C105 | `run compiled / test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok` | pass |
| C106 | `run nets / test a_parent_signal_its_children_meet_on_is_refused ... ok` | pass |
| C107 | `run nets / test a_net_with_its_own_init_is_refused ... ok` | pass |
| C108 | `run irdigital / test a_carrier_initial_value_outside_its_type_is_an_entry ... ok` | pass |
| C109 | `run reaction / test reactions_and_drives_read_the_environment_as_analog_code_does ... ok` | pass |
| C111 | `run environment / test a_compiled_reaction_and_drive_read_the_hosts_temperature_gmin_and_source_scale ... ok` | pass |
| C110 | `run reject / test a_limit_or_stim_function_reading_a_carrier_is_refused_by_name ... ok` | pass |
| Build | `cargo build -p piperine-ir -p piperine-interp -p piperine-parity --all-targets 2>&1 / tee /dev/stderr / (! gre` | pass |
| Build | `test ! -d crates/piperine-plan // cargo build -p piperine-plan --all-targets 2>&1 / tee /dev/stderr / (! grep ` | pass |
| Build | `test ! -d crates/piperine-transpile // cargo build -p piperine-transpile --all-targets 2>&1 / tee /dev/stderr ` | pass |

## Output

`Run compile` - exit 0

```
test a_second_build_runs_no_compiler ... ok
test another_compiler_version_is_another_entry ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.97s

   Compiling piperine-abi v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-abi)
   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-ir)
   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-analog-solver)
   Compiling piperine-digital-solver v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-digital-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-plan)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.47s
     Running tests/compile.rs (/var/tmp/piperine-target/debug/deps/compile-a7556a234f8f9139)
```

`Run golden` - exit 0

```
running 7 tests
test a_keyword_identifier_is_raw_and_the_abi_name_is_not ... ok
test a_body_without_analog_prints_no_terms_function ... ok
test the_five_m1_bodies_print_as_the_golden_files ... ok
test each_function_is_a_generic_fn_and_each_body_a_device_mod ... ok
test every_item_sits_under_its_path_and_origin ... ok
test transpiling_twice_gives_the_same_bytes ... ok
test a_design_with_nothing_to_run_has_no_module ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.23s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.46s
     Running tests/golden.rs (/var/tmp/piperine-target/debug/deps/golden-15c7969774360076)
```

`Run lib` - exit 0

```
test print::value::tests::integer_and_flagged_expressions_print ... ok
test print::cross::tests::terms_reads_a_carrier_as_a_plain_value ... ok
test settings::tests::the_compiler_is_the_variable_then_the_path ... ok
test print::digital::tests::drives_and_events_print_as_discrete_functions ... ok
test shape_check::tests::equal_shapes_have_no_difference_and_a_changed_entry_is_named ... ok
test print::text::tests::reals_round_trip_in_the_shortest_form ... ok
test rt::real::tests::dual_gives_the_interpreters_values_and_tangents ... ok
test rt::int::tests::a_shift_by_the_width_or_more_is_the_truncated_result ... ok
test rt::int::tests::arithmetic_wraps_at_every_width ... ok

test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.73s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_transpile-61235144fc5b0573)
```

`Run reject` - exit 0

```
running 2 tests
test an_ir_that_fails_check_gives_no_source ... ok
test a_limit_or_stim_function_reading_a_carrier_is_refused_by_name ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.46s
     Running tests/reject.rs (/var/tmp/piperine-target/debug/deps/reject-27030a75f846b1f3)
```

`Run rustc` - exit 0

```
running 3 tests
test a_missing_compiler_is_named_and_the_interpreter_is_offered ... ok
test a_compiler_below_1_85_is_refused_with_both_versions ... ok
test the_compiler_is_run_directly_and_cargo_never ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.44s
     Running tests/rustc.rs (/var/tmp/piperine-target/debug/deps/rustc-4060ea5b9a6eab7e)
```

`Run cache` - exit 0

```
running 4 tests
test a_directory_is_created_private ... ok
test an_unwritable_directory_is_named ... ok
test the_key_changes_with_any_part ... ok
test two_racing_builds_leave_one_entry_and_no_staging ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.62s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.48s
     Running tests/cache.rs (/var/tmp/piperine-target/debug/deps/cache-0719567aeb394303)
```

`Run load` - exit 0

```
running 6 tests
test unsafe_lives_in_load_only ... ok
test a_file_that_is_no_library_is_named ... ok
test a_library_without_the_symbol_is_named ... ok
test a_library_whose_bytes_changed_is_not_loaded ... ok
test a_library_smaller_than_the_host_reads_is_refused ... ok
test a_library_of_a_newer_minor_is_refused ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.31s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.46s
     Running tests/load.rs (/var/tmp/piperine-target/debug/deps/load-7431600d063cdccc)
```

`Run session` - exit 0

```
running 2 tests
test no_analysis_runs_the_compiler ... ok
test a_set_between_analyses_runs_no_compiler_and_applies ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.70s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.55s
     Running tests/session.rs (/var/tmp/piperine-target/debug/deps/session-584c45c22bc847d4)
```

`Run compiled` - exit 0

```
test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok
test the_compiled_divider_solves ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.86s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.54s
     Running tests/compiled.rs (/var/tmp/piperine-target/debug/deps/compiled-c9b43439a9e305fb)

thread '<unnamed>' (2719023) panicked at /tmp/.tmpBz8xB2/cache/rust/.staging-8446884de102eeb936cda7b70e243925-2719022-7/mutated.rs:79:48:
boom at 200
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

thread '<unnamed>' (2719023) panicked at /tmp/.tmpBz8xB2/cache/rust/.staging-8446884de102eeb936cda7b70e243925-2719022-7/mutated.rs:79:48:
boom at 300
```

`Run plan` - exit 0

```
running 2 tests
test the_interpreter_and_the_plan_crate_share_one_body_plan ... ok
test the_netlist_binds_whatever_supplies_the_modules ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-analog-solver)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-solver)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-plan)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.19s
     Running tests/shared.rs (/var/tmp/piperine-target/debug/deps/shared-b08238b521f56231)
```

`Run differential` - exit 0

```
running 4 tests
test msl_names_that_equal_generated_names_give_the_interpreters_bytes ... ok
test every_compiled_function_passes_verify_partials ... ok
test a_non_finite_value_is_written_as_is ... ok
test every_m1_function_gives_the_interpreters_bytes ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.81s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.49s
     Running tests/differential.rs (/var/tmp/piperine-target/debug/deps/differential-f27b3e14e59bc3f9)
```

`Run residue` - exit 0

```
running 2 tests
test the_compiled_resistor_matches_the_hand_written_products_bit_for_bit ... ok
test a_compiled_affine_constant_leaves_at_most_one_ulp ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.95s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running tests/residue.rs (/var/tmp/piperine-target/debug/deps/residue-5947c2ffaf862839)
```

`Run backends` - exit 0

```
running 1 test
test both_backends_agree_on_the_m1_circuits ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.71s

   Compiling piperine-analog-solver v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-analog-solver)
   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-ir)
   Compiling piperine-solver v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-solver)
   Compiling piperine-lang v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-lang)
   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-plan)
   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-interp)
   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-parity)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.50s
     Running tests/backends.rs (/var/tmp/piperine-target/debug/deps/backends-d9fdbea001f91861)
```

`Run reaction` - exit 0

```
test a_known_or_over_an_unwritten_input_gives_the_default ... ok
test an_unknown_delay_commits_unknown_at_the_next_microstep ... ok
test a_reaction_commits_in_source_order ... ok
test an_unknown_condition_faults_and_commits_nothing ... ok
test a_panic_in_compiled_code_is_a_fault_and_the_instance_survives ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.93s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.49s
     Running tests/reaction.rs (/var/tmp/piperine-target/debug/deps/reaction-99e5abda0f2c23e7)

thread '<unnamed>' (2722957) panicked at /tmp/.tmp9Trg5L/rust/.staging-aa30e791cdce162a4ca6d1165937f1de-2722955-2/rt/int.rs:209:13:
integer division by zero: UInt<8> 8 / 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

`Run parity` - exit 0

```
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.37s


running 2 tests
test the_transient_step_cap_is_a_tenth_of_the_print_step ... ok
test tran_rc ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.71s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-parity)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.85s
     Running tests/parity_ac.rs (/var/tmp/piperine-target/debug/deps/parity_ac-26be4076edef9079)
     Running tests/parity_dc.rs (/var/tmp/piperine-target/debug/deps/parity_dc-c1f3d9ac312fbb08)
     Running tests/parity_op.rs (/var/tmp/piperine-target/debug/deps/parity_op-158610f75b7f80db)
     Running tests/parity_tran.rs (/var/tmp/piperine-target/debug/deps/parity_tran-200c854a70195100)
```

`Run backend` - exit 0

```
running 4 tests
test an_unknown_backend_is_refused_naming_both ... ok
test the_variable_selects_one_backend ... ok
test a_rejection_names_the_backend ... ok
test the_rust_bench_runs_loaded_modules ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.92s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-parity)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.66s
     Running tests/backend.rs (/var/tmp/piperine-target/debug/deps/backend-bbd0c0c934d2618f)
```

`Run contract` - exit 0

```
test golden::tests::the_print_precision_is_read_from_the_token ... ok
test golden::tests::a_wrong_row_count_names_both_counts ... ok
test golden::tests::a_grid_mismatch_names_the_point ... ok
test ngspice::tests::a_missing_binary_fails_naming_the_opt_out ... ok
test ngspice::tests::skip_passes_and_prints_one_line ... ok
test waveform::tests::a_time_outside_the_result_is_none ... ok
test ngspice::tests::the_binary_comes_from_the_variable_or_the_path ... ok
test golden::tests::an_unparsable_row_names_file_and_row ... ok
test golden::tests::an_export_is_found_with_or_without_data_suffix ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-parity)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.63s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_parity-8e9b94bdfaac5c62)
```

`Run irdigital` - exit 0

```
running 8 tests
test a_carrier_initial_value_outside_its_type_is_an_entry ... ok
test a_defective_digital_body_gives_one_entry_per_defect ... ok
test carriers_and_drives_print ... ok
test the_m1_prints_are_unchanged ... ok
test events_and_reactions_print ... ok
test unknown_values_and_their_reads_print ... ok
test integers_and_their_operators_print ... ok
test the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-ir v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-ir)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.66s
     Running tests/digital.rs (/var/tmp/piperine-target/debug/deps/digital-00f1b28ac249b265)
```

`Run frozen` - exit 0

```
running 1 test
test a_digital_body_is_rejected_by_name ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.43s
     Running tests/frozen.rs (/var/tmp/piperine-target/debug/deps/frozen-7080003fc9c2119c)
```

`Run planshape` - exit 0

```
test an_if_lists_its_then_statement_before_its_else ... ok
test an_analog_read_of_a_carrier_is_a_read_of_terms ... ok
test only_a_literal_positive_delay_is_later ... ok
test an_event_is_an_event_pin_and_its_function ... ok
test a_probing_reaction_reads_the_continuous_pin ... ok
test a_reaction_lists_its_events_reads_and_statements ... ok
test a_drive_is_a_discrete_function ... ok
test carriers_are_discrete_pins_with_a_known_byte ... ok
test every_buck_slice_shape_validates ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-plan v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-plan)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.48s
     Running tests/digital_shape.rs (/var/tmp/piperine-target/debug/deps/digital_shape-bedd664d53623fe3)
```

`Run dpwm` - exit 0

```
running 3 tests
test the_oscillator_toggles_every_half_period ... ok
test the_dpwm_dithers_keeps_dead_time_and_never_overlaps ... ok
test two_runs_give_the_same_changes ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.68s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.56s
     Running tests/dpwm.rs (/var/tmp/piperine-target/debug/deps/dpwm-13649b813078249c)
```

`Run commits` - exit 0

```
running 4 tests
test two_commits_exchange_two_carriers ... ok
test the_later_statement_wins ... ok
test two_statements_make_a_pulse ... ok
test an_unchanged_commit_fires_nothing ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.24s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.52s
     Running tests/commits.rs (/var/tmp/piperine-target/debug/deps/commits-39e041361bc207ce)
```

`Run pid` - exit 0

```
running 1 test
test the_ramp_and_the_pid_follow_their_difference_equations ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.79s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.57s
     Running tests/pid.rs (/var/tmp/piperine-target/debug/deps/pid-8bde7ace945c63da)
```

`Run boundary` - exit 0

```
running 3 tests
test the_adc_commits_the_rounded_code_after_tconv ... ok
test the_comparator_trips_at_its_thresholds ... ok
test a_carrier_change_is_a_time_point_and_starts_the_response ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.87s

   Compiling piperine-parity v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-parity)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.70s
     Running tests/boundary.rs (/var/tmp/piperine-target/debug/deps/boundary-0752cf5c76837b1e)
```

`Run abisrc` - exit 0

```
running 2 tests
test the_embedded_sources_are_the_abi_crate ... ok
test the_abi_builds_with_rustc_alone ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.78s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.43s
     Running tests/abi_source.rs (/var/tmp/piperine-target/debug/deps/abi_source-04b7c9196ab45390)
```

`Run nets` - exit 0

```
running 4 tests
test a_net_with_its_own_init_is_refused ... ok
test a_parent_signal_its_children_meet_on_is_refused ... ok
test every_buck_slice_body_compiles_with_no_warning ... ok
test carrier_ports_bind_to_design_nets ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.19s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.54s
     Running tests/digital_circuit.rs (/var/tmp/piperine-target/debug/deps/digital_circuit-8ab2381c7c350e55)
```

`Run environment` - exit 0

```
running 1 test
test a_compiled_reaction_and_drive_read_the_hosts_temperature_gmin_and_source_scale ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.53s

   Compiling piperine-transpile v0.2.0 (/home/keslarek/Git/piperine-ir-rust/crates/piperine-transpile)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.53s
     Running tests/environment.rs (/var/tmp/piperine-target/debug/deps/environment-3680573813a632ad)
```
