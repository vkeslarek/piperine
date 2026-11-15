# solver-restructure proofs

**HEAD**: 21f77c4f
**Tree**: clean
**Shell**: /bin/sh
**Result**: 46 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run solver / test solver_manifest_has_no_in_tree_dev_dependency ... ok` | pass |
| C2 | `cargo test -p piperine-solver` | pass |
| C3 | `cargo test -p piperine-solver --test sens --test adjoint_dc --test adjoint_vs_sens --test transient_reentry --` | pass |
| C4 | `run solver / test divider_reads_the_analytic_ratio ... ok` | pass |
| C5 | `run solver / test rc_step_reads_one_minus_exp ... ok` | pass |
| C6 | `run solver / test diode_satisfies_its_own_equation ... ok` | pass |
| C7 | `run solver / test retired_crates_are_gone ... ok` | pass |
| C8 | `run solver / test no_manifest_names_a_retired_crate ... ok` | pass |
| C9 | `cargo metadata --no-deps --format-version 1 > /dev/null` | pass |
| C10 | `run solver / test root_tests_hold_no_end_to_end_target ... ok` | pass |
| C11 | `run solver / test examples_hold_no_vams_or_python ... ok` | pass |
| C12 | `run solver / test default_members_are_the_green_crates ... ok` | pass |
| C13 | `run solver / test workflows_name_no_retired_crate ... ok` | pass |
| C14 | `run solver / test disto_is_gone ... ok` | pass |
| C15 | `run solver / test text_io_is_gone ... ok` | pass |
| C16 | `run solver / test verification_channels_are_gone ... ok` | pass |
| C17 | `run solver / test specify_timing_is_gone ... ok` | pass |
| C18 | `run solver / test regions_are_gone ... ok` | pass |
| C19 | `run solver / test simprobe_is_gone ... ok` | pass |
| C20 | `run solver / test the_scan_catches_a_planted_name ... ok` | pass |
| C21 | `run solver / test the_three_solver_crates_exist ... ok` | pass |
| C22 | `run solver / test sub_solvers_depend_on_no_workspace_crate ... ok` | pass |
| C23 | `run solver / test owner_depends_on_both_sub_solvers_only ... ok` | pass |
| C24 | `run solver / test analog_sources_name_no_digital_type ... ok` | pass |
| C25 | `run solver / test digital_sources_name_no_analog_type ... ok` | pass |
| C26 | `run solver / test each_sub_solver_holds_its_modules ... ok` | pass |
| C27 | `run analog / test singular_system_fails_loud_through_the_existing_error_family ... ok` | pass |
| C28 | `run digital / test external_model_emits_through_the_sink ... ok` | pass |
| C29 | `run solver / test same_time_events_order_by_seq ... ok` | pass |
| C30 | `git diff --quiet e129b869 -- crates/piperine-solver/tests/abi_surface.rs crates/piperine-solver/tests/prelude_` | pass |
| C31 | `git diff --quiet e129b869 -- crates/piperine-solver/tests/parity_baseline.rs && cargo test -p piperine-solver ` | pass |
| C32 | `now=$(mktemp); cargo test -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver 2>&1 / grep ` | pass |
| C33 | `! cargo build -p piperine-lang -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver --all-t` | pass |
| C34 | `run solver / test instructions_list_the_kept_crates ... ok` | pass |
| C35 | `run solver / test instructions_have_no_entry_for_a_retired_crate ... ok` | pass |
| C36 | `run solver / test state_records_the_solver_structure_decision ... ok` | pass |
| C37 | `! git diff --diff-filter=M e129b869 -- crates/piperine-solver/tests ':(exclude)crates/piperine-solver/tests/se` | pass |
| C38 | `test "$(wc -l < .specs/features/solver-restructure/baseline.txt)" -ge 250` | pass |
| C39 | `cargo test -p piperine-lang --test suite_hygiene` | pass |
| C40 | `a=$(mktemp); b=$(mktemp); git show e129b869:tests/suite_hygiene.rs / grep -A1 '^#\[test\]' / grep -oE '^fn [a-` | pass |
| C41 | `! grep -nE 'crates/piperine-(codegen/ir/python/plugin/plugin-macros)/' crates/piperine-lang/tests/suite_hygien` | pass |
| C42 | `run hygiene / test the_walk_covers_the_three_solver_crates ... ok` | pass |
| C43 | `run solver / test hygiene_guard_lives_in_piperine_lang ... ok` | pass |
| C44 | `run hygiene / test the_detectors_recognise_what_they_forbid ... ok` | pass |
| Build | `cargo build -p piperine-lang --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-solver 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run solver` - exit 0

```
     Running tests/nature_abstol_inheritance.rs (/var/tmp/piperine-target/debug/deps/nature_abstol_inheritance-c8cb96b4addc8d6f)
     Running tests/parity_baseline.rs (/var/tmp/piperine-target/debug/deps/parity_baseline-8c30d9e988a59aee)
     Running tests/prelude_surface.rs (/var/tmp/piperine-target/debug/deps/prelude_surface-2373959a57aa03f9)
     Running tests/probe_selection.rs (/var/tmp/piperine-target/debug/deps/probe_selection-92d77a64c01028b4)
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

`Run analog` - exit 0

```
test math::integration::tests::trbdf2_trapezoidal_phase_coeffs_match_formula ... ok
test math::faer::tests::real_transpose_solve_matches_a_hand_computed_lambda ... ok
test math::faer::tests::one_symbolic_factorization_serves_both_forward_and_transpose_solves ... ok
test math::faer::tests::singular_system_fails_loud_through_the_existing_error_family ... ok

test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_analog_solver-4b654c63946687cc)
   Doc-tests piperine_analog_solver
```

`Run digital` - exit 0

```
test interface::tests::non_crossing_samples_yield_no_interpolation ... ok
test interface::tests::predict_crossing_interpolates_a_linear_bracket_exactly ... ok
test interface::tests::predict_crossing_rejects_a_non_bracketing_pair ... ok
test limits::tests::digital_limits_defaults_are_the_literals ... ok

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_digital_solver-1d0eee47bdaa1c7d)
   Doc-tests piperine_digital_solver
```

`Run hygiene` - exit 0

```
test root_targets_match_the_naming_rule ... ok
test the_walk_covers_the_three_solver_crates ... ok
test every_integration_target_declares_its_scope ... ok
test mod_rs_declares_only ... ok
test every_ignored_doc_example_is_registered ... ok
test no_file_scope_lint_suppression ... ok
test no_dead_architecture_identifiers ... ok
test no_ignored_tests ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_disabled_test_code ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target/debug/deps/suite_hygiene-cfb978c38d167293)
```

`cargo test -p piperine-solver` - exit 0

```
     Running tests/nature_abstol_inheritance.rs (/var/tmp/piperine-target/debug/deps/nature_abstol_inheritance-c8cb96b4addc8d6f)
     Running tests/parity_baseline.rs (/var/tmp/piperine-target/debug/deps/parity_baseline-8c30d9e988a59aee)
     Running tests/prelude_surface.rs (/var/tmp/piperine-target/debug/deps/prelude_surface-2373959a57aa03f9)
     Running tests/probe_selection.rs (/var/tmp/piperine-target/debug/deps/probe_selection-92d77a64c01028b4)
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

`cargo test -p piperine-solver --test sens --test adjoint_dc --test adjoint_vs_sens --test transient_reentry --` - exit 0

```
(no output)
```

`cargo metadata --no-deps --format-version 1 > /dev/null` - exit 0

```
(no output)
```

`git diff --quiet e129b869 -- crates/piperine-solver/tests/abi_surface.rs crates/piperine-solver/tests/prelude_` - exit 0

```
running 1 test
test abi_compiles_an_element ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 1 test
test prelude_voltage_divider ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/abi_surface.rs (/var/tmp/piperine-target/debug/deps/abi_surface-0849349ee130faf8)
     Running tests/prelude_surface.rs (/var/tmp/piperine-target/debug/deps/prelude_surface-2373959a57aa03f9)
```

`git diff --quiet e129b869 -- crates/piperine-solver/tests/parity_baseline.rs && cargo test -p piperine-solver ` - exit 0

```
running 6 tests
test parity_digital_scheduler_snapshot ... ok
test parity_diode_dc_point ... ok
test parity_mixed_signal_dc_settle ... ok
test parity_divider_op_dc ... ok
test parity_rlc_ac_resonant_peak ... ok
test parity_rc_transient ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/parity_baseline.rs (/var/tmp/piperine-target/debug/deps/parity_baseline-8c30d9e988a59aee)
```

`now=$(mktemp); cargo test -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver 2>&1 / grep ` - exit 0

```
(no output)
```

`! cargo build -p piperine-lang -p piperine-solver -p piperine-analog-solver -p piperine-digital-solver --all-t` - exit 0

```
(no output)
```

`! git diff --diff-filter=M e129b869 -- crates/piperine-solver/tests ':(exclude)crates/piperine-solver/tests/se` - exit 0

```
(no output)
```

`test "$(wc -l < .specs/features/solver-restructure/baseline.txt)" -ge 250` - exit 0

```
(no output)
```

`cargo test -p piperine-lang --test suite_hygiene` - exit 0

```
test root_targets_match_the_naming_rule ... ok
test the_walk_covers_the_three_solver_crates ... ok
test every_integration_target_declares_its_scope ... ok
test mod_rs_declares_only ... ok
test every_ignored_doc_example_is_registered ... ok
test no_file_scope_lint_suppression ... ok
test no_dead_architecture_identifiers ... ok
test no_ignored_tests ... ok
test lrm_covers_declarations_are_well_formed ... ok
test no_disabled_test_code ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running tests/suite_hygiene.rs (/var/tmp/piperine-target/debug/deps/suite_hygiene-cfb978c38d167293)
```

`a=$(mktemp); b=$(mktemp); git show e129b869:tests/suite_hygiene.rs / grep -A1 '^#\[test\]' / grep -oE '^fn [a-` - exit 0

```
(no output)
```

`! grep -nE 'crates/piperine-(codegen/ir/python/plugin/plugin-macros)/' crates/piperine-lang/tests/suite_hygien` - exit 0

```
(no output)
```
