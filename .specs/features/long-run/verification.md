# Long Run verification

**Verdict**: FAIL
**Proofs**: `proofs.md` at 16f7dbf4 (HEAD 5133f08b adds only spec records)
**Verifier**: independent sub-agent (author != verifier)

One silent defect against a spec edge case (F1), two proofs looser than their criterion (F2, F3). The streamed grid is bit-equal to `Table::on_grid` on every grid I tried, the hold rule is right at session level, `Keep::Last` is falsified by the memory test, and every ngspice comparison runs and passes with `PIPERINE_NGSPICE` unset.

## What was run at HEAD

- `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity_grid -- --nocapture`: 3 passed. The comparisons ran: `tran_rc on interp: 100 pairs` and `tran_rc on rust: 100 pairs`, worst deviation 8.60e-5 V. The only `SKIP` line came from `a_skipped_reference_still_runs_the_twin` (C47), which skips on purpose.
- `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity_tran --test parity_op --test parity_dc --test parity_ac --test parity_corpus -- --nocapture`: all passed, no `SKIP` line.
- C44 `git diff --quiet a5f23ea4 -- ...`: clean. `contract.rs` and the ngspice fixtures are unedited. No tolerance was retuned.
- I wrote throwaway tests and mutations in a scratch copy (`git archive HEAD`). Nothing was committed from them, and the worktree is clean.
  - Recorder against `Table::on_grid` on 7 grids (h 1e-4 to 1.7, coarser than `stop` included), each with steps snapped exactly to multiples and one ulp either side, 3 stops (one not a multiple) and 5 `from` values (`from` between steps among them): bit-equal everywhere, failures identical.
  - Session, rust backend: a counter with `init` 5 on grids of 0.5, 0.25, 0.1 and 1 µs. The 0.5 µs grid lands exactly on the clock edges. Row 0 shows 5, and every row equals `5 + edges at or before t`. A gridless run with nets is bit-equal to `Table::transient` over `Keep::Every`.
  - Mutation: the session keeps `Keep::Every`. `memory` fails (C15 is falsifiable).
  - Mutation: grid rows skip `apply(Some(at + slack))`. C20, C26, C62, C64, C63 and C27 fail. C25 still passes, as it should: its nets change only at steps.
  - Spec edge, change within slack after a step that sits on a row: fails at HEAD (F1).

## Checks

| Check | Test | Holds |
| --- | --- | --- |
| C1 | `step_observer.rs` `the_observer_is_called_once_per_accepted_step_in_time_order` | yes, exact times [1,2,3] and accept count |
| C2 | `step_observer.rs` `each_observed_step_holds_the_values_of_the_kept_step` | yes, every unknown and every net's bytes against the kept step, and the clock is shown to toggle |
| C3 | `step_observer.rs` `a_transient_observes_its_start_and_every_accepted_step` | yes, calls == `steps_accepted + 1`, first at 0, strictly increasing |
| C4 | `step_observer.rs` `a_resumed_transient_observes_its_start` | yes, first call's time, v(c) and every net equal the captured step |
| C5 | `step_observer.rs` `the_sink_has_every_observation_up_to_the_step_before_the_observer` | yes, sink length [1,3,3] at each call |
| C6 | `step_observer.rs` `a_rejected_or_retaken_attempt_is_never_observed` | yes, a rejection and a landing earlier, times [0.5, 1.0] only |
| C7 | `step_observer.rs` `without_an_observer_the_result_and_the_sink_stream_are_unchanged` | yes for with/without an observer at HEAD. "As before the feature" is held by C40 and C43 |
| C8 | `step_observer.rs` `keep_last_holds_only_the_last_step_with_its_reentry_state` | yes, one step, contents equal to a full run's last step, re-entry goes on |
| C9 | `step_observer.rs` `reentry_from_the_kept_step_equals_reentry_from_a_full_run` | yes, continuation compared step by step, plus stats |
| C10 | `step_observer.rs` `the_default_keeps_every_accepted_step` | yes |
| C11 | `grid.rs` `a_gridless_transient_equals_the_table_of_a_run_that_kept_every_step` | yes, bit-equal on both backends, with and without `from`. My own test adds nets (rust) |
| C12 | `recorder.rs` `a_from_past_the_start_drops_the_start_row` | yes |
| C13 | `grid.rs` `a_printed_signal_with_no_value_fails_before_the_first_step` | yes at `Probe::column`. "Before the first step" holds by reading `Session::analysis` (columns resolve before `Solver::new`); `print` cannot reach the case |
| C14 | `session.rs` `a_transient_keeps_only_the_last_step` | yes, the session builds its options only through `transient_options` |
| C15 | `memory.rs` `a_run_ten_times_longer_at_equal_rows_holds_no_more_memory` | yes, fails under a `Keep::Every` mutation. Retention under about 60 B per step stays under the criterion's 256 KiB |
| C16 | `recorder.rs` `grid_rows_run_from_the_first_multiple_not_before_from_to_stop` | yes |
| C17 | `grid.rs` `a_gridded_run_with_from_starts_at_the_first_multiple_not_before_it` | yes, first row 3 µs, 8 rows, bit-equal to `on_grid` |
| C18 | `recorder.rs` `a_real_column_is_bit_equal_to_on_grid` | yes, by bits on 6 grids. My wider sweep (step on multiple, ±1 ulp, coarse, fine, stop not a multiple) agrees |
| C19 | `recorder.rs` `a_row_on_an_accepted_step_takes_its_value` | yes, picks a case where interpolation would differ |
| C20 | `recorder.rs` `a_discrete_column_holds_the_value_after_the_last_instant` | yes, two microsteps at 0.4, mutation fails it |
| C21 | `recorder.rs` `each_grid_row_is_made_when_its_step_arrives` | yes, row counts after each step, one previous step held |
| C22 | `recorder.rs` `a_row_past_the_last_step_fails` | yes, message asserted |
| C23 | `run.rs` `a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot` | yes, target unedited by the feature |
| C24 | `grid.rs` `the_gridded_tran_rc_is_bit_equal_to_on_grid_on_both_backends` | yes, by bits, two grids, both backends |
| C25 | `recorder.rs` `a_discrete_column_that_changes_only_at_steps_equals_on_grid` | yes, bool and int, 3 grids |
| C26 | `recorder.rs` `a_change_between_two_steps_reaches_the_rows_after_it` | yes, mutation fails it |
| C27 | `grid.rs` `a_counter_that_changes_between_analog_steps_is_exact_on_the_grid` | yes, exact count per row, mutation fails it. My own test adds `init` and edges exactly on grid times |
| C28 | `cli.rs` `every_run_key_has_its_flag` | yes, `cli.rs` unedited, so no key or flag was added |
| C29 | `grid.rs` `a_run_table_and_flags_give_equal_tables` | yes |
| C30 | `buck_long.rs` `the_whole_start_is_60001_rows_on_a_10_ns_grid` | yes, every time `== k as f64 * 1e-8` |
| C31 | `buck_long.rs` `every_check_of_the_whole_start_passes` | yes, seen > 0 and failed == 0 |
| C32 | `buck_long.rs` `the_overshoot_is_within_15_mv` | yes, the maximum over the whole table against 0.815 V (stricter than after-the-ramp) |
| C33 | `buck_long.rs` `the_output_settles_into_10_mv_within_the_ramp_window` | yes, settling = the row after the last exit from the band. Never-out (t=0) and ends-out (inf) both fail |
| C34 | `buck_long.rs` `the_start_ends_in_regulation_with_power_good` | yes |
| C35 | `buck_long.rs` `the_ripple_is_within_10_mv_peak_to_peak` | yes, rows t >= 560 µs, > 0 and <= 10 mV |
| C36 | `buck_long.rs` `the_whole_start_stays_under_one_gibibyte` | yes, peak since `begin` inside `Session::run`, sink O(1) |
| C37 | `buck_long.rs` `the_wall_time_and_the_peak_are_reported` | yes, no time asserted (by reading) |
| C38 | `buck.rs` `startup_prints_the_whole_start_on_its_grid` | yes, 1 201 rows, k*0.5 µs within print precision |
| C39 | `buck.rs` `piperine_test_passes_on_the_buck` | yes |
| C40 | `twins.rs` `tran_rc_prints_the_bytes_it_printed_before_the_long_run` | yes, golden pinned in 4ad365aa, which touched no source |
| C41 | `twins.rs` `the_two_backends_print_the_same_bytes` | no, it asserts interp == rust, not "the bytes printed before" (F2) |
| C42 | `parity_tran.rs` `tran_rc` | yes, ran with ngspice, no SKIP |
| C43 | `parity_baseline.rs` `parity_rc_transient` | yes, target unedited (C44) |
| C44 | `git diff --quiet a5f23ea4 -- <parity sources, baseline, tests/ngspice>` | yes, clean |
| C45 | `parity_grid.rs` `the_grid_of_tran_rc_has_a_row_at_every_time_ngspice_exports` | yes when ngspice runs (100 rows, both backends). The test returns early, asserting nothing, under `skip`, and `proofs.md` cannot show which happened (F3) |
| C46 | `parity_grid.rs` `tran_rc_on_the_grid_agrees_with_ngspice` | yes when ngspice runs (worst 8.6e-5 V under `TRAN_VOLTAGE`), vacuous under `skip` (F3) |
| C47 | `parity_grid.rs` `a_skipped_reference_still_runs_the_twin` | yes, `Reference::Skipped` and 101 rows per backend |
| C48 | `verdict.rs` `a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive` | yes |
| C49 | `commands.rs` `test_prints_pass_with_the_check_count` | yes, target unedited, so the output is the same as before |
| C50 | `commands.rs` `test_prints_fail_and_the_failing_records` | yes, target unedited |
| C51 | `verdict.rs` `a_record_of_another_stream_never_decides` | yes |
| C52 | `options.rs` `a_plot_of_a_gridded_run_draws_its_rows` | yes, points == 6 rows for both series |
| C53 | `options.rs` `a_plot_of_a_gridless_run_draws_every_accepted_step` | yes, points == table rows (one per step via C11/C67) |
| C54 | `book.rs` `every_cli_listing_of_the_book_is_what_the_binary_prints` | yes |
| C55 | `book.rs` `the_buck_block_is_what_the_binary_prints` | yes |
| C56 | `docs.rs` `gaps_says_a_host_run_is_bounded` | yes (text) |
| C57 | `docs.rs` `the_roadmap_lists_what_the_long_run_leaves` | yes (text) |
| C58 | `docs.rs` `the_buck_readme_says_how_to_run_it` | yes (text) |
| C59 | `docs.rs` `state_records_the_observation_decision` | yes for the record. LR-07 AC 3 as behaviour is held by C29 and by reading |
| C60 | `recorder.rs` `a_print_step_past_stop_gives_the_row_at_zero` | yes |
| C61 | `recorder.rs` `a_row_within_rounding_of_the_last_step_takes_its_values` | yes |
| C62 | `recorder.rs` `a_change_within_rounding_of_a_row_counts_at_it` | no, only when the change and the row fall between the same two steps. A row already made at an earlier step misses it (F1) |
| C63 | `recorder.rs` `an_unknown_net_value_is_an_unknown_cell` | yes |
| C64 | `recorder.rs` `a_real_typed_net_is_held_not_interpolated` | yes |
| C65 | `recorder.rs` `a_net_that_changed_unobserved_fails_naming_it` | yes, grid and gridless, the net and time named. The cross-check runs at every step and never takes the reader's value |
| C66 | `recorder.rs` `a_signal_named_twice_gives_two_equal_columns` | yes |
| C67 | `analyses.rs` `a_transient_has_one_row_per_accepted_step` | yes, target unedited |
| C68 | `probes.rs` `a_probe_with_no_value_in_the_circuit_is_a_failure_naming_it` | yes |

## Findings

| Finding | Evidence | Fix | Closed |
| --- | --- | --- | --- |
| F1 (silent, minor): a net change within `1e-9 * print_step` after a row's time is lost when an accepted step at or after that row came first. The spec edge case "a discrete net changes at a time within 1e-9*print_step of a row's time THEN the row SHALL give the changed value" fails, and the grid row shows the old value | `Recorder::on_grid` emits every row with `at <= t1` at step `t1` and applies only the changes already buffered. A change at `t > t1` with `t <= at + slack` arrives with the next step, after the row is pushed. Throwaway test: steps 0, 0.5, 1.0, grid 0.5, change at 0.5+1e-10 gives row 0.5 = `false` (want `true`). It is reachable with the 1 fs tick once `print_step >= 1 µs` (slack >= 1 tick), e.g. a step landing on a breakpoint at `k*h` and a digital instant a few fs later | Keep the indices of the rows emitted at the last step whose `at + slack > t1`. On a later `change` with `time <= at + slack`, update those rows' net cells in order. Add the throwaway case as a test under C62 | |
| F2 (low, assertion looser than LR-14 AC 1): "each of the 7 twins prints the bytes it printed before" is proven only for `tran_rc`. C41 asserts interp == rust, which a change common to both backends passes | `twins.rs` `the_two_backends_print_the_same_bytes` compares the backends with each other; only `goldens/tran_rc.txt` holds pre-feature bytes. The other six run op/dc/ac arms the diff does not touch, so the risk is low | Pin goldens for the six other twins from the `a5f23ea4` binary and compare each backend to them, or restate C41 as backend agreement and add a golden check | |
| F3 (silent proof, low): C45 and C46 pass vacuously under `PIPERINE_NGSPICE=skip`. Their tests `return` before any assertion, the SKIP line goes to captured stdout, and `proofs.md` shows only `... ok`, so a skipped run proves nothing about the grid against ngspice | `parity_grid.rs:72` and `:86` `let Some(..) = export(&reference) else { return };`. `proofs.md` has no `pairs` or `SKIP` text. I reran with the variable unset: both compare 100 pairs per backend and pass (worst 8.6e-5 V) | Make the `paritygrid` Run line `env -u PIPERINE_NGSPICE cargo test ... -- --nocapture`, and set the C46 proof to a line that only a real comparison prints (e.g. `tran_rc on rust: 100 pairs`) | |

Observations, not findings: the memory test's first run carries one-time global allocations (short 61 kB against long 26 kB). The 256 KiB bound, fixed by the spec, does not catch retention below about 60 B per step.
