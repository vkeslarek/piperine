# ir-interp proofs

**HEAD**: 29f1369f
**Tree**: clean
**Shell**: /bin/sh
**Result**: 50 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run circuit / test one_module_per_body_with_behaviour_or_a_held_node ... ok` | pass |
| C2 | `run circuit / test a_circuit_binds_instances_nodes_and_params ... ok` | pass |
| C3 | `run circuit / test an_ir_that_fails_check_is_rejected ... ok` | pass |
| C4 | `run circuit / test the_crate_spawns_nothing_and_reads_no_file ... ok` | pass |
| C5 | `run shape / test a_held_node_gives_has_init ... ok` | pass |
| C6 | `run factory / test module_with_carries_the_factory ... ok` | pass |
| C7 | `run factory / test create_goes_through_the_factory_and_validates ... ok` | pass |
| C8 | `run factory / test eval_and_react_run_the_created_instance ... ok` | pass |
| C9 | `cargo test -p piperine-abi && cargo test -p piperine-solver` | pass |
| C10 | `run shape / test pins_come_in_the_declared_order ... ok` | pass |
| C11 | `run shape / test a_flow_branch_declares_flow_and_ddt_terms ... ok` | pass |
| C12 | `run shape / test a_potential_branch_declares_its_pin_and_two_terms ... ok` | pass |
| C13 | `run shape / test sites_declare_limit_and_stim_inputs ... ok` | pass |
| C14 | `run shape / test one_function_per_site_and_one_for_terms ... ok` | pass |
| C15 | `run shape / test reads_and_partials_are_the_dependency_sets ... ok` | pass |
| C16 | `run shape / test every_entry_carries_its_origin ... ok` | pass |
| C17 | `run shape / test every_m1_shape_validates ... ok` | pass |
| C18 | `run lib / test walk::tests::f64_arithmetic_and_methods_match_rust ... ok` | pass |
| C19 | `run lib / test walk::tests::division_by_zero_is_ieee ... ok` | pass |
| C20 | `run lib / test walk::tests::short_circuit_and_if_evaluate_one_side ... ok` | pass |
| C21 | `run lib / test walk::tests::calls_bind_parameters_and_closures_see_locals ... ok` | pass |
| C22 | `run terms / test evaluating_twice_writes_the_same_bytes ... ok` | pass |
| C23 | `run terms / test the_terms_function_sums_addends_with_signs ... ok` | pass |
| C24 | `run terms / test seeded_partials_are_the_dual_derivatives ... ok` | pass |
| C25 | `run terms / test environment_reads_come_from_pin_and_ambients ... ok` | pass |
| C26 | `run partials / test every_m1_function_passes_verify_partials ... ok` | pass |
| C27 | `run lib / test dual::tests::abs_has_the_derivative_of_sign ... ok` | pass |
| C28 | `run terms / test an_affine_term_is_constant_plus_folded_products ... ok` | pass |
| C29 | `run terms / test the_resistor_matches_the_hand_written_products_bit_for_bit ... ok` | pass |
| C30 | `run limit / test a_limit_writes_proposed_and_limited ... ok` | pass |
| C31 | `run limit / test old_is_seed_or_last_by_age_and_static ... ok` | pass |
| C32 | `run limit / test the_seed_is_kept_when_the_limited_value_falls_below ... ok` | pass |
| C33 | `run terms / test the_terms_function_reads_the_limit_value_untouched ... ok` | pass |
| C34 | `run shape / test no_function_writes_a_limit_and_a_term ... ok` | pass |
| C35 | `run stim / test a_stim_writes_its_phasor ... ok` | pass |
| C36 | `run terms / test the_terms_function_reads_the_stim_value ... ok` | pass |
| C37 | `run shape / test a_value_of_the_wrong_layout_faults_naming_the_param ... ok` | pass |
| C38 | `run lib / test walk::tests::an_unexecuted_form_faults_with_its_origin ... ok` | pass |
| C39 | `run shape / test a_where_declares_a_check_pin_and_function ... ok` | pass |
| C40 | `run circuit / test a_set_that_breaks_a_where_is_refused ... ok` | pass |
| C41 | `run shape / test a_block_without_contributions_gives_no_term ... ok` | pass |
| C42 | `run shape / test a_held_node_without_analog_is_ports_only ... ok` | pass |
| C43 | `run circuit / test an_unset_param_keeps_its_default ... ok` | pass |
| C44 | `run terms / test a_non_finite_value_is_written_as_is ... ok` | pass |
| C45 | `run terms / test an_undeclared_read_never_steers_the_walk ... ok` | pass |
| C46 | `run lib / test walk::tests::an_unread_value_steers_nothing ... ok` | pass |
| C47 | `run circuit / test a_held_node_starts_the_transient ... ok` | pass |
| Build | `cargo build -p piperine-abi --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `test ! -d crates/piperine-interp // cargo build -p piperine-interp --all-targets 2>&1 / tee /dev/stderr / (! g` | pass |

## Output

`Run circuit` - exit 0

```
test the_crate_spawns_nothing_and_reads_no_file ... ok
test one_module_per_body_with_behaviour_or_a_held_node ... ok
test an_rc_low_pass_is_at_minus_three_db_at_its_corner ... ok
test the_divider_reads_seven_and_a_half_volts ... ok
test a_circuit_binds_instances_nodes_and_params ... ok
test an_unset_param_keeps_its_default ... ok
test a_diode_converges_to_the_shockley_point ... ok
test a_checked_ir_is_accepted ... ok
test a_set_that_breaks_a_where_is_refused ... ok
test a_held_node_starts_the_transient ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/circuit.rs (/var/tmp/piperine-target/debug/deps/circuit-c6bcde5cd512f815)
```

`Run shape` - exit 0

```
test every_entry_carries_its_origin ... ok
test pins_come_in_the_declared_order ... ok
test one_function_per_site_and_one_for_terms ... ok
test a_flow_branch_declares_flow_and_ddt_terms ... ok
test a_where_declares_a_check_pin_and_function ... ok
test every_m1_shape_validates ... ok
test sites_declare_limit_and_stim_inputs ... ok
test a_potential_branch_declares_its_pin_and_two_terms ... ok
test no_function_writes_a_limit_and_a_term ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-interp/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.57s
     Running tests/shape.rs (/var/tmp/piperine-target/debug/deps/shape-f513b0dadbd7d664)
```

`Run factory` - exit 0

```
running 3 tests
test module_with_carries_the_factory ... ok
test create_goes_through_the_factory_and_validates ... ok
test eval_and_react_run_the_created_instance ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
     Running tests/factory.rs (/var/tmp/piperine-target/debug/deps/factory-f56ceb0559a7ddd5)
```

`Run lib` - exit 0

```
test dual::tests::mul_follows_the_product_rule ... ok
test dual::tests::the_chain_rule_composes ... ok
test dual::tests::neg_flips_value_and_tangent ... ok
test walk::tests::an_unexecuted_form_faults_with_its_origin ... ok
test walk::tests::an_unread_value_steers_nothing ... ok
test walk::tests::division_by_zero_is_ieee ... ok
test walk::tests::short_circuit_and_if_evaluate_one_side ... ok
test walk::tests::calls_bind_parameters_and_closures_see_locals ... ok
test walk::tests::f64_arithmetic_and_methods_match_rust ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-interp/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.54s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_interp-55a54c1fd80ff7e7)
```

`Run terms` - exit 0

```
test seeded_partials_are_the_dual_derivatives ... ok
test a_non_finite_value_is_written_as_is ... ok
test the_terms_function_reads_the_stim_value ... ok
test evaluating_twice_writes_the_same_bytes ... ok
test environment_reads_come_from_pin_and_ambients ... ok
test the_terms_function_reads_the_limit_value_untouched ... ok
test the_terms_function_sums_addends_with_signs ... ok
test an_affine_term_is_constant_plus_folded_products ... ok
test the_resistor_matches_the_hand_written_products_bit_for_bit ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-interp/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.55s
     Running tests/terms.rs (/var/tmp/piperine-target/debug/deps/terms-b3d925e9d3116a06)
```

`Run partials` - exit 0

```
running 1 test
test every_m1_function_passes_verify_partials ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-interp/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.47s
     Running tests/partials.rs (/var/tmp/piperine-target/debug/deps/partials-d7ff87ea58d22805)
```

`Run limit` - exit 0

```
running 3 tests
test old_is_seed_or_last_by_age_and_static ... ok
test a_limit_writes_proposed_and_limited ... ok
test the_seed_is_kept_when_the_limited_value_falls_below ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-interp/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.53s
     Running tests/limit.rs (/var/tmp/piperine-target/debug/deps/limit-5a2d264f5712ddb9)
```

`Run stim` - exit 0

```
running 1 test
test a_stim_writes_its_phasor ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Compiling piperine-interp v0.2.0 (/home/keslarek/Git/piperine-ir-interp/crates/piperine-interp)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.51s
     Running tests/stim.rs (/var/tmp/piperine-target/debug/deps/stim-a1188d05a8407da1)
```

`cargo test -p piperine-abi && cargo test -p piperine-solver` - exit 0

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
