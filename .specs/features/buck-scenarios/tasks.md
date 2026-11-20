# Buck Scenarios Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/buck-scenarios/design.md`
**Status**: Draft
**Approved**: 2026-11-20 - under the user's standing AFK rule; dispatched B1 (T1-T8) - Sonnet worker. B1 done (edc0c973). B2 (phases 3-5, T9-T15) - Sonnet worker, 2026-11-20.
**Build**: `cargo build -p piperine -p piperine-parity -p piperine-api -p piperine-cli --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape`
**Run harness**: `cargo test -p piperine-parity --lib scenario::`
**Run scenarios**: `cargo test -p piperine --test buck_scenarios -- --nocapture`
**Run supervisor**: `cargo test -p piperine-parity --test buck_supervisor`
**Run clibuck**: `cargo test -p piperine-cli --test buck the_buck_has_its_bench_and_two_runs`
**Run docs**: `cargo test -p piperine-cli --test docs`

Executors: every task is Sonnet (user rule); a different model verifies. The worktree is
`/home/keslarek/Git/piperine-buck-scenarios` on `feature/buck-scenarios`. Cargo runs with
`CARGO_TARGET_DIR=/var/tmp/piperine-target-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-a`,
in the foreground; stop when `df /` passes 92 %. Where worktrees share the target directory, `touch src/lib.rs`
(and the `src/lib.rs` of each crate the task edits) before a gate.

**Gate time.** `Run scenarios` simulates every family present so far, all at once: from T9 on a gate takes 15 to
25 minutes. Run it once per task, never in a loop; a red family prints its message in every test of the family.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md`, `crates/piperine-lang/tests/suite_hygiene.rs`, `crates/piperine-solver/tests/workspace_shape.rs`, `.specs/STATE.md` MD-13, MD-28, MD-38, MD-72.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Harness library (`piperine-parity/src/scenario/*.rs`) | unit | every method 1:1 to BS-02's criteria on hand-built tables; every panic and `None` path; a batch with a failing run | inline `#[cfg(test)] mod tests` in each file | `cargo test -p piperine-parity --lib scenario::` |
| Scenario package and root target (`tests/e2e/buck/`, `tests/buck_scenarios.rs`) | integration | one test per criterion of BS-01, BS-03 to BS-08, each printing the value it measured; every family's S10 | `tests/buck_scenarios.rs` | `cargo test -p piperine --test buck_scenarios -- --nocapture` |
| The buck model (`examples/buck/src/*.msl`) | integration | the fixed behaviour counted against SPEC, not against a model of the code | `crates/piperine-parity/tests/buck_*.rs` | `cargo test -p piperine-parity --test buck_supervisor` |
| Pinned prints (`crates/piperine-lang/tests/fixtures/elab/buck.txt`) | integration | the pin moves only by the intended line | `crates/piperine-lang/tests/elab_buck.rs` | `cargo test -p piperine-lang --test elab_buck` |
| Records (`examples/buck/SPEC.md`, `README.md`, `GAPS.md`) | integration | each record claim read as text | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |
| `examples/buck/Piperine.toml` (unchanged) | integration | its two runs stay | `crates/piperine-cli/tests/buck.rs` | `cargo test -p piperine-cli --test buck the_buck_has_its_bench_and_two_runs` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | Harness tasks (T1 to T5) | `cargo test -p piperine-parity --lib scenario::` |
| Full | Package, model and family tasks (T6 to T13) | the task's `Run` line(s) named in its Done when, then the Build line |
| Build | Record tasks (T8, T14, T15) and every phase end | the `**Build**:` line, then `cargo test -p piperine-cli --test docs` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

**Batches** (~7 tasks, whole phases): B1 = Phases 1-2 (T1-T8, 8 tasks, no long run), B2 = Phases 3-5 (T9-T15, 7 tasks, every gate a long run).

### Phase 1: The harness

Measurement and stimulus types in `piperine-parity::scenario`, each proven on hand-built data.

```
T1 → T2
T1 → T3
T4 → T5
```

### Phase 2: The package, and the buck agrees with its SPEC

The scenario package with its cheap tests, the hiccup fix, and the SPEC edits the scenarios cite.

```
T6
T7
T8
```

### Phase 3: Families I

```
T2 → T9
T3 → T9
T5 → T9
T6 → T9
T8 → T9
T9 → T10 → T11
T7 → T11
```

### Phase 4: Families II

```
T11 → T12 → T13
```

### Phase 5: The records

```
T13 → T14
T13 → T15
```

---

## Task Breakdown

### T1: `NamedColumn` and the `scenario` module

**What**: `crates/piperine-parity/src/scenario/column.rs` with `NamedColumn::find`, `index`, `label` (design "Components"); `crates/piperine-parity/src/scenario/mod.rs` (a `//!` header naming MD-73 and the five files) and `pub mod scenario;` in `crates/piperine-parity/src/lib.rs`. Every struct and method has a doc comment.
**Where**: `crates/piperine-parity/src/scenario/column.rs`
**Depends on**: None
**Reuses**: `crates/piperine-api/tests/buck_long.rs::column`
**Requirement**: BS-02

**Done when**:

- [x] A label equal to the name and a label ending in `.name` are both found; two matching labels panic naming both
- [x] `a_missing_column_panics_naming_the_name_and_the_columns` asserts the panic text through `std::panic::catch_unwind` (no `should_panic`)
- [x] Gate check passes: `cargo test -p piperine-parity --lib scenario::`
- [x] Test count: 3 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): NamedColumn finds a table column by label or suffix`

**Status**: Complete

---

### T2: `Trace`

**What**: `crates/piperine-parity/src/scenario/trace.rs`: `Trace`, `Sample`, `Crossing` with the methods of design "Components" (`of`, `scaled`, `between`, `maximum`, `minimum`, `deviation_from`, `mean`, `peak_to_peak`, `settled_into`, `first_crossing`, `first_rise_in_any_row`, `decay_time_constant`). An empty window panics naming the label and the window.
**Where**: `crates/piperine-parity/src/scenario/trace.rs`
**Depends on**: T1
**Reuses**: `buck_long.rs::series` and its settling rule
**Requirement**: BS-02

**Done when**:

- [x] One test per BS-02 AC 1 to AC 5 on hand-built tables whose answers are worked by hand in the test's comment; AC 1's test also asserts `None` when the last row is outside
- [x] A test for `first_rise_in_any_row` and `decay_time_constant` on an exact exponential (τ recovered to 1e-9 relative)
- [x] Gate check passes: `cargo test -p piperine-parity --lib scenario::`
- [x] Test count: 7 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): Trace measures a real column over a window`

**Status**: Complete

---

### T3: `DiscreteTrace`

**What**: `crates/piperine-parity/src/scenario/discrete.rs`: `DiscreteTrace` and `Departure` (`of`, `between`, `first_time`, `holds`, `value_at`, `peak_to_peak`); `Bool` cells read as 0 or 1, `Unknown` as `None`.
**Where**: `crates/piperine-parity/src/scenario/discrete.rs`
**Depends on**: T1
**Reuses**: `piperine_api::Cell`
**Requirement**: BS-02

**Done when**:

- [x] One test each for BS-02 AC 6 and AC 7; one for `value_at` between rows; one for `peak_to_peak` panicking on an unknown row (asserted through `catch_unwind`)
- [x] Gate check passes: `cargo test -p piperine-parity --lib scenario::`
- [x] Test count: 4 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): DiscreteTrace reads a mode, a code or a bit over a window`

**Status**: Complete

---

### T4: `Stimulus` and `Ramp`

**What**: `crates/piperine-parity/src/scenario/stimulus.rs`: `Stimulus` (`new`, `drive`, `set`, `ramp`, `apply`) and `Ramp` (`points`), with the private `Action`. `apply` calls `Session::drive` and `Session::set_at` in the order given and returns the first refusal with the action named.
**Where**: `crates/piperine-parity/src/scenario/stimulus.rs`
**Depends on**: None
**Reuses**: `piperine_api::{Session, Failure, ParamValue}`
**Requirement**: BS-02

**Done when**:

- [x] `a_ramp_sets_once_per_equal_step_and_ends_on_its_target` (BS-02 AC 8) covers a rising and a falling ramp (12 → 16 V in 10 µs: 40 points 0.25 µs apart; 12 → 7.5 V in 90 µs: 45 points 2 µs apart)
- [x] `apply` is tested on a planted one-module package (a tempdir: `Piperine.toml`, `src/lib.msl` with one discrete `in` port and one `f64` param): a drive and a timed set are accepted, a drive of an unknown port returns `E0801` naming the action
- [x] Gate check passes: `cargo test -p piperine-parity --lib scenario::`
- [x] Test count: 3 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): Stimulus schedules a host's drives and timed sets`

**Status**: Complete

---

### T5: `ScenarioRun`, `ScenarioBatch`, `Ran`, `CheckTally`

**What**: `crates/piperine-parity/src/scenario/run.rs` (design "Components"): each run in a build directory of its own, its session dropped before that directory; the batch runs one thread per run in input order and turns a panic into `Err`.
**Where**: `crates/piperine-parity/src/scenario/run.rs`
**Depends on**: T4
**Reuses**: `crates/piperine-parity/src/host.rs` (`HostRun`), `buck_long.rs::Checks`
**Requirement**: BS-08

**Done when**:

- [x] `a_failed_run_is_kept_as_its_message_and_the_others_still_run` (BS-08 AC 2): a batch of a short planted run and a run of a name the package lacks gives `Ok` then `Err` naming the missing run
- [x] `the_tally_counts_checks_and_the_failed_ones` on hand-made `Record`s
- [x] `each_run_builds_in_its_own_directory`: two runs of the planted package report different `build` paths and overlapping `started..ended` intervals
- [x] Gate check passes: `cargo test -p piperine-parity --lib scenario::`
- [x] Test count: 3 new tests

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): ScenarioBatch runs scenario runs once each, at once`

**Status**: Complete

---

### T6: The scenario package `tests/e2e/buck/`

**What**: `tests/e2e/buck/Piperine.toml` (package `buck_scenarios`, `buck = { path = "../../../examples/buck" }`, the five runs of design "Components" with the stops of design "Timelines") and `tests/e2e/buck/src/lib.msl` (`//!` header; `Bench` with the five `in` ports, `pgood`, `fault`, `mode`, `pub let buck`). `tests/buck_scenarios.rs` (new root target, `//!` header naming SPEC §10 and this feature) holds the package's cheap tests only.
**Where**: `tests/e2e/buck/Piperine.toml`
**Depends on**: None
**Reuses**: `examples/buck/benches/startup.msl` (declarations), `crates/piperine-api/tests/buck_long.rs` (opening a session)
**Requirement**: BS-01

**Done when**:

- [x] `the_package_declares_the_five_scenario_runs`, `every_run_samples_the_thirteen_signals_on_a_10_ns_grid`, `every_run_accepts_the_five_ports_and_three_params` (opens every run, drives the five ports at 0 and sets the three params at 1 µs, does not run), `the_scenario_package_checks_with_no_diagnostic`, `opening_a_run_leaves_only_the_package_sources` (BS-01 AC 1 to 4, 6)
- [x] `Run clibuck` passes (BS-01 AC 5) and `cargo test -p piperine-parity --test parity_corpus` passes (no twin added)
- [x] `git status --porcelain` is clean after the gate but for the task's files
- [x] Gate check passes: `Run scenarios` and the Build line
- [x] Test count: 5 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the buck's scenario package with five host-driven runs`

**Status**: Complete

---

### T7: The hiccup lasts 1000 samples

**What**: `examples/buck/src/supervisor.msl` enters `Hiccup` with `retry <- HICCUP_SAMPLES - 1`, so `SoftStart` comes at the 1000th sample after the entry (SPEC §8.5, design "Risks"); the reference model in `crates/piperine-parity/tests/buck_supervisor.rs` sets `hiccup_left = 999` with a comment citing §8.5; the new test `each_hiccup_lasts_1000_samples` counts, in the observed run, the samples from each entry into `Hiccup` to the next `SoftStart` and asserts 1000 against SPEC's number, not against the model; the pinned print `crates/piperine-lang/tests/fixtures/elab/buck.txt` changes by that one constant.
**Where**: `examples/buck/src/supervisor.msl`
**Depends on**: None
**Reuses**: `buck_supervisor.rs` (its script reaches two hiccups)
**Requirement**: BS-09

**Done when**:

- [x] `each_hiccup_lasts_1000_samples` fails before the model change and passes after it (run once each way, both outcomes in the handoff)
- [x] `the_supervisor_follows_its_state_table` passes with the reference model at 999
- [x] `cargo test -p piperine-lang --test elab_buck` passes with the pin's one line changed (`git diff --stat` shows one line in `buck.txt`)
- [x] `cargo test -p piperine-parity --test buck_closed_loop --test buck_whole` pass
- [x] Gate check passes: `Run supervisor` and the Build line
- [x] Test count: 1 new test

**Tests**: integration
**Gate**: full

**Commit**: `fix(buck): a hiccup lasts 1000 samples, as SPEC 8.5 says`

**Status**: Complete

---

### T8: SPEC agrees with the bench

**What**: `examples/buck/SPEC.md`: §4.3 gains the 3.3 V column (spec BS-09 AC 2, with the exact total row `| **Total, worst case** | **±6.5 mV** (target ±10 mV) | **±14.7 mV = 0.45 %** (target ±1 %) | **±20 mV = 0.41 %** (target ±1 %) |`); §10 S2b reads "within ±1 % of 1.0 V at 1.29 ms ±0.05 ms"; a new §10.1 "How the bench measures" holds the stimulus shapes (setpoint write, 0.1 V staircase, line step in 10 µs, brown-out at 50 mV/µs), the settling time and deviation definitions, the 10 ns grid, the next-sample bound 2.02 µs and two-sample bound 4.52 µs, the current-limit reaction bound 0.1 A (one 5 ns clock, 10 ns driver, 5 ns channel at ≤ 3.4 A/µs), S8's decay bound ± 2 % (τ = (6.6 Ω ∥ 40 kΩ) · 220 µF = 1.452 ms), and the "≈" rule of D8; §8.5's fault codes read (`Clear`, `OverCurrent`, `OverVoltage`). `crates/piperine-cli/tests/docs.rs` gains `the_buck_spec_holds_the_bench_bounds`, one assertion per BS-09 AC 2 to AC 5.
**Where**: `examples/buck/SPEC.md`
**Depends on**: None
**Reuses**: SPEC §4.3's own rows and rules
**Requirement**: BS-09

**Done when**:

- [x] The new test fails before the edit and passes after it
- [x] Gate check passes: `Run docs` and the Build line
- [x] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs(buck): SPEC holds the 3.3 V budget, the S2b tolerance and how the bench measures`

**Status**: Complete

---

### T9: The suite and the setpoint family

**What**: In `tests/buck_scenarios.rs`: `Supervisor`, `Timing`, `BuckHost`, `Waveforms`, `Suite` (one `OnceLock<Suite>` holding one `Result<Ran, String>` per family, filled by one `ScenarioBatch`; `report()` prints each family's wall time, rows and interval) and `SetpointFamily` (timeline of design "Timelines"); one test per criterion of BS-03, the family's S10, BS-08 AC 1 and AC 3. Every measuring test prints its value and the SPEC bound.
**Where**: `tests/buck_scenarios.rs`
**Depends on**: T2, T3, T5, T6, T8
**Reuses**: `piperine_parity::scenario`
**Requirement**: BS-03, BS-08

**Done when**:

- [x] `setpoint_regulates_at_3_3_v_with_pgood_before_its_first_stimulus`, `s2_reference_reaches_800_codes_0_544_ms_after_the_write`, `s2_vout_settles_into_5_v_within_1_percent_at_0_54_ms`, `s2_overshoot_is_at_most_15_mv`, `s2_pgood_stays_high`, `s2b_reference_reaches_160_codes_1_28_ms_after_the_write`, `s2b_vout_settles_into_1_v_within_1_percent_at_1_29_ms`, `s2b_inductor_current_goes_negative`, `s2b_sink_limit_never_trips`, `s2b_no_over_voltage_fault`, `setpoint_passes_every_check`, `the_families_run_at_once_in_their_own_build_directories`, `every_family_reports_its_wall_time_and_rows`
- [x] Gate check passes: `Run scenarios` and the Build line (≈ 15 min) (17 of 18 pass; `s2b_vout_settles_into_1_v_within_1_percent_at_1_29_ms` is red, SPEC bound unmoved)
- [x] Test count: 13 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the buck's setpoint scenarios S2 and S2b`

**Status**: Complete (C22, the S2b settling at 1.29 ms, is red on the model: measured 1.360 ms, see Handoff)

---

### T10: The load and line family

**What**: `LoadLineFamily` in `tests/buck_scenarios.rs` (S3, S4, S8 on one run; design "Timelines"), added to `Suite`; one test per criterion of BS-04 and of BS-06 AC 6 to AC 9, its precondition and its S10.
**Where**: `tests/buck_scenarios.rs`
**Depends on**: T9
**Reuses**: `BuckHost::line`, `BuckHost::load`
**Requirement**: BS-04, BS-06

**Done when**:

- [x] `load_line_regulates_at_3_3_v_with_pgood_before_its_first_stimulus`, `s3_load_step_up_stays_within_165_mv`, `s3_load_step_down_stays_within_165_mv`, `s3_load_step_up_recovers_within_200_us`, `s3_load_step_down_recovers_within_200_us`, `s3_pgood_stays_high`, `s3_no_current_limit`, `s4_line_step_up_stays_within_165_mv`, `s4_line_step_down_stays_within_165_mv`, `s4_line_step_up_recovers_within_200_us`, `s4_line_step_down_recovers_within_200_us`, `s4_no_fault`, `s8_off_at_the_sample_after_en_falls`, `s8_both_switches_stay_off`, `s8_pgood_low_from_off`, `s8_vout_decays_with_the_load_time_constant`, `load_line_passes_every_check`
- [x] Gate check passes: `Run scenarios` and the Build line (34 of 35 pass; the one red is T9's `s2b_vout_settles_into_1_v_within_1_percent_at_1_29_ms`)
- [x] Test count: 17 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the buck's load, line and disable scenarios S3, S4 and S8 at 3.3 V`

**Status**: Complete (all 17 pass; the 18th test of the target, C22 of T9, stays red)

---

### T11: The faults family

**What**: `FaultsFamily` in `tests/buck_scenarios.rs` (S5 then S6 on one run; design "Timelines"), added to `Suite`, with `over_current_entry()`, `retry()` and `over_voltage_entry()` shared by its tests; one test per criterion of BS-05, its precondition and its S10.
**Where**: `tests/buck_scenarios.rs`
**Depends on**: T10, T7
**Reuses**: `BuckHost::step_resistance`, `BuckHost::inject`
**Requirement**: BS-05

**Done when**:

- [ ] `faults_regulates_at_3_3_v_with_pgood_before_its_first_stimulus`, `s5_every_period_is_cut_until_hiccup`, `s5_inductor_current_stays_at_or_below_7_6_a`, `s5_hiccup_with_over_current_16_to_20_us_after_the_step`, `s5_pgood_falls_within_8_us_of_the_deviation`, `s5_both_switches_stay_off_in_hiccup`, `s5_retry_comes_2_ms_after_the_hiccup_with_the_overload_on`, `s5_retry_without_the_overload_regulates_and_clears_the_fault`, `s6_sink_limit_trips`, `s6_inductor_current_is_held_at_minus_3_a`, `s6_over_voltage_within_two_samples_of_3_81_v`, `s6_reports_its_timing_and_rise_rate`, `s6_both_switches_stay_off_after_over_voltage`, `faults_passes_every_check`
- [ ] A criterion the model misses (design "Risks": S5's 16 to 20 µs) stays red and is reported in the handoff with its measured value; no bound moves
- [ ] Gate check passes: `Run scenarios` and the Build line (≈ 24 min)
- [ ] Test count: 14 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the buck's overcurrent and over-voltage scenarios S5 and S6`

---

### T12: The brown-out family

**What**: `BrownoutFamily` in `tests/buck_scenarios.rs` (S7; design "Timelines"), added to `Suite`, with the two `buck.vin` crossings shared by its tests; one test per criterion of BS-06 AC 1 to AC 5, its precondition and its S10.
**Where**: `tests/buck_scenarios.rs`
**Depends on**: T11
**Reuses**: `BuckHost::line` (the 2 µs staircase)
**Requirement**: BS-06

**Done when**:

- [ ] `brownout_regulates_at_3_3_v_with_pgood_before_its_first_stimulus`, `s7_off_at_the_sample_after_vin_falls_through_8_v`, `s7_both_switches_off_while_off`, `s7_pgood_low_while_off`, `s7_fault_unchanged_while_off`, `s7_soft_start_at_the_sample_after_vin_rises_through_8_5_v`, `brownout_passes_every_check`
- [ ] Gate check passes: `Run scenarios` and the Build line
- [ ] Test count: 7 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the buck's brown-out scenario S7`

---

### T13: The steady-state family

**What**: `SteadyStateFamily` and `Corner` in `tests/buck_scenarios.rs` (S9; the nine corners of design "Timelines", their windows computed from `SETTLE`, `WINDOW`, `LINE` and SPEC §7.2's ramp lengths), added to `Suite`; one test per criterion of BS-07 and its S10. Every corner's values are printed, failing or not.
**Where**: `tests/buck_scenarios.rs`
**Depends on**: T12
**Reuses**: `BuckHost::write_setpoint`, `BuckHost::load`, `BuckHost::line`
**Requirement**: BS-07

**Done when**:

- [ ] `s9_every_corner_is_settled`, `s9_dc_error_within_section_4_3_at_every_corner`, `s9_ripple_at_most_10_mv_at_every_corner`, `s9_no_idle_tone_above_1_code_at_every_corner`, `steady_state_passes_every_check`
- [ ] The DC-error test checks the corner's settling first and names an unsettled corner as such (BS-07 AC 4)
- [ ] Gate check passes: `Run scenarios` and the Build line (all five families, ≈ 24 min)
- [ ] Test count: 5 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test: the buck's steady state at nine corners, S9`

---

### T14: The buck's README points at the scenarios

**What**: `examples/buck/README.md`: "Status" and "Driving it from a host" say S2 to S10 run from `tests/buck_scenarios.rs` over `tests/e2e/buck/` through `Session::drive` and `Session::set_at`; the sentence that `piperine-api` has no drives, waits or mid-run `set` goes; the Python sketch keeps "No such host exists yet" for Python (B4). `crates/piperine-cli/tests/docs.rs` gains `the_buck_readme_points_at_the_scenarios`; `the_buck_readme_says_how_to_run_it` stays as it is.
**Where**: `examples/buck/README.md`
**Depends on**: T13
**Reuses**: `docs.rs` (the read-as-text pattern)
**Requirement**: BS-10

**Done when**:

- [ ] The new test fails before the edit and passes after it; `the_buck_readme_says_how_to_run_it` still passes
- [ ] Gate check passes: `Run docs` and the Build line
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs(buck): the README points at the host scenarios`

---

### T15: `GAPS.md` narrows U3

**What**: `GAPS.md`: U3 reads that S2 to S10 run (`buck-scenarios`) and only S1 at 3.3 V and S1b stay unrun; H1's sentence "The buck's scenarios S1 to S9 have no host yet" becomes "the buck's scenarios S2 to S10 run on the Rust host (`buck-scenarios`)", keeping "`Session::drive`" and "`force` and `release`"; the U-section and summary rows that name U3 follow. `crates/piperine-cli/tests/docs.rs` gains `gaps_narrows_what_buck_scenarios_ran`; `gaps_drops_what_host_ii_closed` stays as it is.
**Where**: `GAPS.md`
**Depends on**: T13
**Reuses**: `docs.rs::gaps_drops_what_root_e2e_closed` (the form)
**Requirement**: BS-10

**Done when**:

- [ ] The new test fails before the edit and passes after it; every other `docs.rs` test passes
- [ ] Gate check passes: `Run docs` and the Build line
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `docs(gaps): U3 keeps only S1 at 3.3 V and S1b`

---

## Checks

**C1** - The package `tests/e2e/buck/` depends on `examples/buck` by path and declares exactly `brownout`, `faults`, `load_line`, `setpoint`, `steady_state`. (BS-01, AC 1; T6)
Proof: run scenarios | test the_package_declares_the_five_scenario_runs ... ok

**C2** - Every run has top `Bench`, `max_step` and `print_step` 1e-8, `streams = ["checks"]`, `max_steps` 15000000 and the thirteen print entries. (BS-01, AC 2; T6)
Proof: run scenarios | test every_run_samples_the_thirteen_signals_on_a_10_ns_grid ... ok

**C3** - Every run accepts a drive of the five ports at 0 and a timed set of `buck.vin_nom`, `buck.r_load`, `buck.r_step`. (BS-01, AC 3; T6)
Proof: run scenarios | test every_run_accepts_the_five_ports_and_three_params ... ok

**C4** - The scenario package checks with no diagnostic. (BS-01, AC 4; T6)
Proof: run scenarios | test the_scenario_package_checks_with_no_diagnostic ... ok

**C5** - `examples/buck/Piperine.toml` keeps exactly `idle` and `startup`. (BS-01, AC 5; T6)
Proof: run clibuck | test the_buck_has_its_bench_and_two_runs ... ok

**C6** - Opening a run leaves `tests/e2e/buck/` holding only `Piperine.toml` and `src/lib.msl`. (BS-01, AC 6; T6)
Proof: run scenarios | test opening_a_run_leaves_only_the_package_sources ... ok

**C7** - A missing column panics naming the name and every label. (BS-02, AC 9; T1)
Proof: run harness | test scenario::column::tests::a_missing_column_panics_naming_the_name_and_the_columns ... ok

**C8** - The settling time is the first row from which every later row is inside the band, and none when the last row is outside. (BS-02, AC 1; T2)
Proof: run harness | test scenario::trace::tests::settled_into_is_the_first_row_from_which_every_row_stays_inside ... ok

**C9** - The deviation is the largest absolute difference from the nominal, with its row's time. (BS-02, AC 2; T2)
Proof: run harness | test scenario::trace::tests::deviation_is_the_largest_absolute_difference_with_its_time ... ok

**C10** - The peak-to-peak is the largest row minus the smallest. (BS-02, AC 3; T2)
Proof: run harness | test scenario::trace::tests::peak_to_peak_is_the_largest_minus_the_smallest ... ok

**C11** - The mean is the arithmetic mean of the rows. (BS-02, AC 4; T2)
Proof: run harness | test scenario::trace::tests::mean_is_the_arithmetic_mean_of_the_rows ... ok

**C12** - The first crossing is the first row at or past the level, in the direction asked, after the time asked. (BS-02, AC 5; T2)
Proof: run harness | test scenario::trace::tests::first_crossing_is_the_first_row_at_or_past_the_level_after_a_time ... ok

**C13** - The first time of a value is the first row holding it after the time asked. (BS-02, AC 6; T3)
Proof: run harness | test scenario::discrete::tests::first_time_is_the_first_row_holding_the_value_after_a_time ... ok

**C14** - `holds` names the first departing row with its time and value, and passes when none departs. (BS-02, AC 7; T3)
Proof: run harness | test scenario::discrete::tests::holds_names_the_first_row_that_departs ... ok

**C15** - A ramp sets once per equal step, the last at the end of the duration and equal to the target. (BS-02, AC 8; T4)
Proof: run harness | test scenario::stimulus::tests::a_ramp_sets_once_per_equal_step_and_ends_on_its_target ... ok

**C16** - At 1.25 ms the `setpoint` run is in `Regulate` with `pgood` high and `vout` within 3.3 V ± 1 %. (BS-03, AC 1; T9)
Proof: run scenarios | test setpoint_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok

**C17** - After the write of 168 the reference first reads 800 codes 0.544 ms ± 2 µs later. (BS-03, AC 2; T9)
Proof: run scenarios | test s2_reference_reaches_800_codes_0_544_ms_after_the_write ... ok

**C18** - `vout` settles into 5.0 V ± 1 % at 0.54 ms ± 0.05 ms after the write. (BS-03, AC 3; T9)
Proof: run scenarios | test s2_vout_settles_into_5_v_within_1_percent_at_0_54_ms ... ok

**C19** - `vout` stays at most 15 mV above 5.0 V from 1.25 ms to 2.00 ms. (BS-03, AC 4; T9)
Proof: run scenarios | test s2_overshoot_is_at_most_15_mv ... ok

**C20** - `pgood` is high in every row from 1.25 ms to 2.00 ms. (BS-03, AC 5; T9)
Proof: run scenarios | test s2_pgood_stays_high ... ok

**C21** - After the write of 8 the reference first reads 160 codes 1.28 ms ± 2 µs later. (BS-03, AC 6; T9)
Proof: run scenarios | test s2b_reference_reaches_160_codes_1_28_ms_after_the_write ... ok

**C22** - `vout` settles into 1.0 V ± 1 % at 1.29 ms ± 0.05 ms after the write. (BS-03, AC 7; T9)
Proof: run scenarios | test s2b_vout_settles_into_1_v_within_1_percent_at_1_29_ms ... ok

**C23** - The inductor current goes below 0 A during S2b, its minimum printed. (BS-03, AC 8; T9)
Proof: run scenarios | test s2b_inductor_current_goes_negative ... ok

**C24** - `buck.sink_oc` is low in every row of S2b. (BS-03, AC 9; T9)
Proof: run scenarios | test s2b_sink_limit_never_trips ... ok

**C25** - `fault` is `Clear` and `mode` is `Regulate` in every row of S2b. (BS-03, AC 10; T9)
Proof: run scenarios | test s2b_no_over_voltage_fault ... ok

**C26** - The `setpoint` run's `checks` hold records and none failed. (BS-08, AC 4; T9)
Proof: run scenarios | test setpoint_passes_every_check ... ok

**C27** - The families' run intervals all overlap and each built in its own directory. (BS-08, AC 1; T9)
Proof: run scenarios | test the_families_run_at_once_in_their_own_build_directories ... ok

**C28** - Each family's wall time and rows are printed. (BS-08, AC 3; T9)
Proof: run scenarios | test every_family_reports_its_wall_time_and_rows ... ok

**C29** - A failed run is kept as its message and the other runs still run. (BS-08, AC 2; T5)
Proof: run harness | test scenario::run::tests::a_failed_run_is_kept_as_its_message_and_the_others_still_run ... ok

**C30** - At 1.25 ms the `load_line` run is in `Regulate` with `pgood` high and `vout` within 3.3 V ± 1 %. (BS-03, AC 1; T10)
Proof: run scenarios | test load_line_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok

**C31** - After the load step up `vout` stays within 165 mV of 3.3 V until 1.55 ms. (BS-04, AC 1; T10)
Proof: run scenarios | test s3_load_step_up_stays_within_165_mv ... ok

**C32** - After the load step down `vout` stays within 165 mV of 3.3 V until 1.85 ms. (BS-04, AC 2; T10)
Proof: run scenarios | test s3_load_step_down_stays_within_165_mv ... ok

**C33** - After the load step up `vout` settles into 3.3 V ± 1 % within 200 µs. (BS-04, AC 3; T10)
Proof: run scenarios | test s3_load_step_up_recovers_within_200_us ... ok

**C34** - After the load step down `vout` settles into 3.3 V ± 1 % within 200 µs. (BS-04, AC 4; T10)
Proof: run scenarios | test s3_load_step_down_recovers_within_200_us ... ok

**C35** - `pgood` is high in every row from 1.25 ms to 1.85 ms. (BS-04, AC 5; T10)
Proof: run scenarios | test s3_pgood_stays_high ... ok

**C36** - `buck.oc` is low in every row from 1.25 ms to 1.85 ms. (BS-04, AC 6; T10)
Proof: run scenarios | test s3_no_current_limit ... ok

**C37** - After the line step up `vout` stays within 165 mV of 3.3 V until 2.35 ms. (BS-04, AC 7; T10)
Proof: run scenarios | test s4_line_step_up_stays_within_165_mv ... ok

**C38** - After the line step down `vout` stays within 165 mV of 3.3 V until 2.65 ms. (BS-04, AC 8; T10)
Proof: run scenarios | test s4_line_step_down_stays_within_165_mv ... ok

**C39** - After the line step up `vout` settles into 3.3 V ± 1 % within 200 µs of the step's start. (BS-04, AC 9; T10)
Proof: run scenarios | test s4_line_step_up_recovers_within_200_us ... ok

**C40** - After the line step down `vout` settles into 3.3 V ± 1 % within 200 µs of the step's start. (BS-04, AC 10; T10)
Proof: run scenarios | test s4_line_step_down_recovers_within_200_us ... ok

**C41** - `fault` is `Clear` and `mode` is `Regulate` in every row from 2.05 ms to 2.65 ms. (BS-04, AC 11; T10)
Proof: run scenarios | test s4_no_fault ... ok

**C42** - After `en` falls at 2.80 ms `mode` first reads `Off` within 2.02 µs. (BS-06, AC 6; T10)
Proof: run scenarios | test s8_off_at_the_sample_after_en_falls ... ok

**C43** - Both gate commands are low in every row from one grid row after the entry into `Off` to the end. (BS-06, AC 7; T10)
Proof: run scenarios | test s8_both_switches_stay_off ... ok

**C44** - `pgood` is low from the entry into `Off` on. (BS-06, AC 8; T10)
Proof: run scenarios | test s8_pgood_low_from_off ... ok

**C45** - From 10 µs after the entry into `Off`, `vout` decreases in every row with a time constant within 1.452 ms ± 2 %. (BS-06, AC 9; T10)
Proof: run scenarios | test s8_vout_decays_with_the_load_time_constant ... ok

**C46** - The `load_line` run's `checks` hold records and none failed. (BS-08, AC 4; T10)
Proof: run scenarios | test load_line_passes_every_check ... ok

**C47** - At 1.25 ms the `faults` run is in `Regulate` with `pgood` high and `vout` within 3.3 V ± 1 %. (BS-03, AC 1; T11)
Proof: run scenarios | test faults_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok

**C48** - From the first rise of `buck.ctl.cut` to the entry into `Hiccup`, `cut` is high in every row. (BS-05, AC 1; T11)
Proof: run scenarios | test s5_every_period_is_cut_until_hiccup ... ok

**C49** - The inductor current stays at or below 7.6 A while current-limited. (BS-05, AC 2; T11)
Proof: run scenarios | test s5_inductor_current_stays_at_or_below_7_6_a ... ok

**C50** - `Hiccup` with `OverCurrent` first appears 16 µs to 20 µs after the load step. (BS-05, AC 3; T11)
Proof: run scenarios | test s5_hiccup_with_over_current_16_to_20_us_after_the_step ... ok

**C51** - `pgood` is low no later than 8 µs after the first ADC code outside `reference ± reference/16`. (BS-05, AC 4; T11)
Proof: run scenarios | test s5_pgood_falls_within_8_us_of_the_deviation ... ok

**C52** - Both gate commands are low in every row of the hiccup. (BS-05, AC 5; T11)
Proof: run scenarios | test s5_both_switches_stay_off_in_hiccup ... ok

**C53** - `SoftStart` first appears 2.000 ms after the entry into `Hiccup`, within one grid row, with `load_step` still high. (BS-05, AC 6; T11)
Proof: run scenarios | test s5_retry_comes_2_ms_after_the_hiccup_with_the_overload_on ... ok

**C54** - With the overload removed during the retry, `mode` reaches `Regulate` and `fault` reads `Clear` from that row on. (BS-05, AC 7; T11)
Proof: run scenarios | test s5_retry_without_the_overload_regulates_and_clears_the_fault ... ok

**C55** - `buck.sink_oc` rises at least once between `inject`'s edge and the entry into `Hiccup`. (BS-05, AC 8; T11)
Proof: run scenarios | test s6_sink_limit_trips ... ok

**C56** - The minimum inductor current during the back-drive is within −3.0 A ± 0.1 A. (BS-05, AC 9; T11)
Proof: run scenarios | test s6_inductor_current_is_held_at_minus_3_a ... ok

**C57** - `Hiccup` with `OverVoltage` first appears no earlier than the first row of `vout` above 3.81 V and no later than 4.52 µs after it. (BS-05, AC 10; T11)
Proof: run scenarios | test s6_over_voltage_within_two_samples_of_3_81_v ... ok

**C58** - The time from `inject`'s edge to the OVP entry and the rise rate of `vout` are printed. (BS-05, AC 11; T11)
Proof: run scenarios | test s6_reports_its_timing_and_rise_rate ... ok

**C59** - Both gate commands are low in every row from one grid row after the OVP entry to the end. (BS-05, AC 12; T11)
Proof: run scenarios | test s6_both_switches_stay_off_after_over_voltage ... ok

**C60** - The `faults` run's `checks` hold records and none failed. (BS-08, AC 4; T11)
Proof: run scenarios | test faults_passes_every_check ... ok

**C61** - At 1.25 ms the `brownout` run is in `Regulate` with `pgood` high and `vout` within 3.3 V ± 1 %. (BS-03, AC 1; T12)
Proof: run scenarios | test brownout_regulates_at_3_3_v_with_pgood_before_its_first_stimulus ... ok

**C62** - `mode` first reads `Off` within 2.02 µs after `buck.vin` falls through 8.0 V, and not before. (BS-06, AC 1; T12)
Proof: run scenarios | test s7_off_at_the_sample_after_vin_falls_through_8_v ... ok

**C63** - Both gate commands are low in every `Off` row from one grid row after its entry. (BS-06, AC 2; T12)
Proof: run scenarios | test s7_both_switches_off_while_off ... ok

**C64** - `pgood` is low in every `Off` row. (BS-06, AC 3; T12)
Proof: run scenarios | test s7_pgood_low_while_off ... ok

**C65** - `fault` keeps its pre-brown-out value (`Clear`) in every `Off` row. (BS-06, AC 4; T12)
Proof: run scenarios | test s7_fault_unchanged_while_off ... ok

**C66** - `mode` first reads `SoftStart` within 2.02 µs after `buck.vin` rises through 8.5 V, and reads `Off` until that crossing. (BS-06, AC 5; T12)
Proof: run scenarios | test s7_soft_start_at_the_sample_after_vin_rises_through_8_5_v ... ok

**C67** - The `brownout` run's `checks` hold records and none failed. (BS-08, AC 4; T12)
Proof: run scenarios | test brownout_passes_every_check ... ok

**C68** - The mean of `vout` in each corner's window is within ± 6.5, ± 14.7 or ± 20 mV of 0.8, 3.3 or 5.0 V. (BS-07, AC 1; T13)
Proof: run scenarios | test s9_dc_error_within_section_4_3_at_every_corner ... ok

**C69** - The peak-to-peak of `vout` in each corner's window is at most 10 mV. (BS-07, AC 2; T13)
Proof: run scenarios | test s9_ripple_at_most_10_mv_at_every_corner ... ok

**C70** - The peak-to-peak of `buck.code` in each corner's window is at most 1 code. (BS-07, AC 3; T13)
Proof: run scenarios | test s9_no_idle_tone_above_1_code_at_every_corner ... ok

**C71** - The means of each window's two halves differ by at most 6.25 mV; an unsettled corner fails named as such. (BS-07, AC 4; T13)
Proof: run scenarios | test s9_every_corner_is_settled ... ok

**C72** - The `steady_state` run's `checks` hold records and none failed. (BS-08, AC 4; T13)
Proof: run scenarios | test steady_state_passes_every_check ... ok

**C73** - Each hiccup of the supervisor lasts exactly 1000 samples, counted against SPEC §8.5. (BS-09, AC 1; T7)
Proof: run supervisor | test each_hiccup_lasts_1000_samples ... ok

**C74** - SPEC §4.3 has the 3.3 V column with the ± 14.7 mV total, S2b reads "±0.05 ms", §10.1 exists with its bounds, and the fault codes read `Clear`. (BS-09, AC 2 to AC 5; T8)
Proof: run docs | test the_buck_spec_holds_the_bench_bounds ... ok

**C75** - The README points at `tests/buck_scenarios.rs` and `tests/e2e/buck/` and no longer says `piperine-api` has no drives or mid-run `set`. (BS-10, AC 1; T14)
Proof: run docs | test the_buck_readme_points_at_the_scenarios ... ok

**C76** - `GAPS.md` U3 keeps only S1 at 3.3 V and S1b, and H1 no longer says the buck's scenarios have no host. (BS-10, AC 2; T15)
Proof: run docs | test gaps_narrows_what_buck_scenarios_ran ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** B1 (worker defaults, user AFK): (1) `Stimulus::apply` returns `Result<(), Refusal>` (new type: action text plus the session's `Failure`, Display) instead of `Failure`, because `Failure` cannot carry the action's name; (2) `DiscreteTrace::value_at` panics when no row is at or before the time; (3) `scenario::planted` (cfg(test)) plants the one-module package of the harness tests; (4) extra harness tests beyond the counts: `between`/`scaled` in trace, split apply tests; (5) SPEC §4.3 gains a short paragraph deriving the 3.3 V column (aliasing = half the ESR ripple at 16 V: 1.7 mV) and §10.1 derives 2.02 us = Ts + 20 ns, 4.52 us = 2 Ts + 500 ns conversion + 20 ns, 0.1 A reaction = 20 ns x 3.4 A/us = 0.068 A; (6) hiccup fix is `retry <- HICCUP_SAMPLES - 1` (pin line folds to `999u11`), reference model `hiccup_left = 999`; `each_hiccup_lasts_1000_samples` measured 1001 before the fix and 1000 after; (7) the print list's non-`pub` signals (`buck.code`, `buck.oc`, `buck.ctl.cut`...) resolve with no change to the converter.
**Dispatched:** none
**Walked:** none

---

## Phase Execution Map

```
Phase 1, then Phase 2, then Phase 3, then Phase 4, then Phase 5

Phase 1:  T1, T2, T3, T4, T5     (harness, quick gates)
Phase 2:  T6, T7, T8             (package, hiccup fix, SPEC; no long run)
Phase 3:  T9, T10, T11           (each gate 15 to 25 min)
Phase 4:  T12, T13               (each gate about 25 min)
Phase 5:  T14, T15               (records)
```

Execution is strictly sequential. B1 = Phases 1-2 (8 tasks), B2 = Phases 3-5 (7 tasks).
