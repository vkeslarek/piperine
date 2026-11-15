# solver-abi-analog proofs

**HEAD**: b1b86c81
**Tree**: clean
**Shell**: /bin/sh
**Result**: 25 pass, 0 fail

| Check | Proof | Result |
| --- | --- | --- |
| C1 | `run analog / test device_modules_name_no_digital_or_owner_item ... ok` | pass |
| C2 | `run analog / test a_flow_term_stamps_its_norton_form ... ok` | pass |
| C3 | `run analog / test a_ddt_term_is_silent_in_an_equilibrium_solve ... ok` | pass |
| C4 | `run analog / test a_ddt_term_stamps_the_capacitor_companion ... ok` | pass |
| C5 | `run analog / test a_restore_returns_histories_and_limit_state ... ok` | pass |
| C6 | `run analog / test a_limit_reader_is_linearized_at_the_held_iterate ... ok` | pass |
| C7 | `run analog / test an_active_limit_vetoes_convergence ... ok` | pass |
| C8 | `run analog / test owns_correction_gets_no_correction_and_keeps_the_veto ... ok` | pass |
| C9 | `run analog / test a_delay_term_reads_its_history_at_t_minus_td ... ok` | pass |
| C10 | `run analog / test a_delay_beyond_its_max_fails_naming_the_term ... ok` | pass |
| C11 | `run analog / test a_clean_function_is_not_called ... ok` | pass |
| C12 | `run analog / test a_non_finite_value_names_instance_cell_and_origin ... ok` | pass |
| C13 | `run analog / test the_pattern_is_terms_times_partials ... ok` | pass |
| C14 | `run analog / test small_signal_weights_each_operator ... ok` | pass |
| C15 | `run analog / test one_noise_input_on_two_terms_is_correlated ... ok` | pass |
| C16 | `run analog / test a_var_watch_is_readable_at_an_accepted_solution ... ok` | pass |
| C17 | `run analog / test a_cross_watch_fires_on_a_rising_sign_change ... ok` | pass |
| C18 | `run analog / test accept_reports_the_smallest_cap ... ok` | pass |
| C19 | `run analog / test a_trace_record_survives_a_rejected_step ... ok` | pass |
| C20 | `run analog / test ambient_inputs_carry_the_solve_coordinates ... ok` | pass |
| C21 | `run analog / test setting_a_discrete_slot_makes_its_readers_dirty ... ok` | pass |
| C22 | `run analog / test the_facade_offers_what_an_analog_side_needs ... ok` | pass |
| C23 | `cargo test -p piperine-analog-solver --lib 2>&1 / grep -E '^test result: ok\. 24 passed'` | pass |
| Build | `cargo build -p piperine-analog-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |
| Build | `cargo build -p piperine-solver --all-targets 2>&1 / tee /dev/stderr / (! grep -qE '^(warning/error)')` | pass |

## Output

`Run analog` - exit 0

```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.07s
     Running unittests src/lib.rs (/var/tmp/piperine-target/debug/deps/piperine_analog_solver-d85a2eecb8c4fbb0)
     Running tests/assembly_ddt.rs (/var/tmp/piperine-target/debug/deps/assembly_ddt-120b3b4b98c81008)
     Running tests/assembly_delay.rs (/var/tmp/piperine-target/debug/deps/assembly_delay-7d354577901f8e40)
     Running tests/assembly_eval.rs (/var/tmp/piperine-target/debug/deps/assembly_eval-dca1c3c5fda0f1f7)
     Running tests/assembly_flow.rs (/var/tmp/piperine-target/debug/deps/assembly_flow-dc8380098aff5732)
     Running tests/assembly_limit.rs (/var/tmp/piperine-target/debug/deps/assembly_limit-e9cbc38044bbb0c2)
     Running tests/assembly_noise.rs (/var/tmp/piperine-target/debug/deps/assembly_noise-9178d4b18ce8c5ee)
     Running tests/assembly_small_signal.rs (/var/tmp/piperine-target/debug/deps/assembly_small_signal-3de5a9d671f6a652)
     Running tests/assembly_watch.rs (/var/tmp/piperine-target/debug/deps/assembly_watch-78f836e6cfd00fff)
     Running tests/boundary.rs (/var/tmp/piperine-target/debug/deps/boundary-b6ca8e349cc211bd)
     Running tests/plan.rs (/var/tmp/piperine-target/debug/deps/plan-d18bcd9d8c5637d2)
   Doc-tests piperine_analog_solver
```

`cargo test -p piperine-analog-solver --lib 2>&1 / grep -E '^test result: ok\. 24 passed'` - exit 0

```
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
