# Long Run Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/long-run/design.md`
**Status**: Approved
**Approved**: 2026-11-17 - user: "Pode ir rodando os grandes passos"
**Build**: `cargo build -p piperine-solver -p piperine-api -p piperine -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
**Run step**: `cargo test -p piperine-solver --test step_observer`
**Run baseline**: `cargo test -p piperine-solver --test parity_baseline`
**Run apilib**: `cargo test -p piperine-api --lib`
**Run probes**: `cargo test -p piperine-api --test probes`
**Run analyses**: `cargo test -p piperine-api --test analyses`
**Run grid**: `cargo test -p piperine-api --test grid`
**Run memory**: `cargo test -p piperine-api --test memory`
**Run bucklong**: `cargo test -p piperine-api --test buck_long`
**Run parity**: `cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran --test parity_corpus`
**Run paritygrid**: `cargo test -p piperine-parity --test parity_grid`
**Run clilib**: `cargo test -p piperine-cli --lib`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run run**: `cargo test -p piperine-cli --test run`
**Run options**: `cargo test -p piperine-cli --test options`
**Run commands**: `cargo test -p piperine-cli --test commands`
**Run buck**: `cargo test -p piperine-cli --test buck`
**Run clibook**: `cargo test -p piperine-cli --test book`
**Run docs**: `cargo test -p piperine-cli --test docs`

The approval reached the planner through the orchestrator on 2026-11-17, as a standing permission to dispatch the batches; it is recorded in `context.md`.

Executors: Sonnet implements; Opus implements the delicate tasks directly, those where a mistake gives a wrong number with no error or moves a parity result: T1, T3, T4, T5, T8, T9, T10, T14, T15, T18, T19, T20. A different Opus verifies the feature. The worktree, the branch and the shared cargo target directory are told at dispatch; where parallel worktrees share one target directory, `touch crates/<crate>/src/lib.rs` before a gate so cargo rebuilds this tree's sources.

Rules every task follows:

- **The solver's default does not move.** With no step observer and `Keep::Every`, a transient gives the result and the sink stream it gave at `a5f23ea4`. No existing target of `piperine-solver`, `piperine-analog-solver`, `piperine-digital-solver`, `piperine-transpile`, `piperine-interp` or `piperine-parity` is edited; a task that needs to edit one stops and reports.
- **Parity is untouched.** `crates/piperine-parity/src/contract.rs`, `tests/ngspice/` and the five `parity_*` targets are not edited. No tolerance is written outside `ToleranceContract`, and none is changed.
- **The command line is a shell** (MD-65). Every rule of what a row holds is in `piperine-api` with a Rust test. No flag and no run key is added (Q3).
- **No MSL changes.** `crates/piperine-lang`, `docs/spec/` and `examples/buck/src` are not edited. `piperine-project` is not edited.
- **`Table::on_grid` and `Table::transient` are not edited**: they are the oracle of the differential tests.
- **Fail loud.** A value that cannot be read, a net that changed unobserved, a grid past the last step: a `Failure`, never a default, a skipped row or a stale cell.
- **A SPEC limit is never moved** (Q4). If the buck misses a limit of LR-09, LR-10 or LR-11, the test stays as written, the task stops, and the measured value is reported to the orchestrator for the user.
- **Tests state what the spec says, not what the code does.** Expected values come from the spec of this feature, `examples/buck/SPEC.md`, the MSL source or the oracle functions named above, each cited at its assertion. Test functions have exactly the names `## Checks` cites.
- **Three existing tests change, and only these**, each under an amendment the user accepted on 2026-11-17: `a_plot_draws_every_accepted_step_whatever_the_print_step` in `crates/piperine-cli/tests/options.rs` (CLI-22 AC 7, T11), the `startup` assertions of `crates/piperine-cli/tests/buck.rs` (Q2, T14), and `the_buck_readme_says_how_to_run_it` in `crates/piperine-cli/tests/docs.rs` (T16). No other test is removed, weakened or skipped.
- **AGENTS.md idiom rules bind**: no loose function, no macro of your own, modules named after what they do, no `unwrap()`/`expect()` on a path a user's input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`. The one attribute this feature needs is `#[global_allocator]` on the counting allocator's `static`.
- Zero warnings on every crate the `**Build**` line names. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- **`lessons.py` is not run** during this feature: its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`. If it is run, `git status` afterwards and restore both files.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md` ("Where the tests are", MD-28 placement), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Solver protocol and analyses (`piperine-solver/src/protocol`, `src/analyses`) | integration | 1:1 to LR-01 and LR-02 ACs; the default path proven unchanged by `parity_baseline` unedited | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver --test step_observer` |
| Host library rules (`piperine-api/src/*.rs`: recorder, probe, verdict) | unit | all branches; 1:1 to the spec's ACs and every listed edge case | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-api --lib` |
| Host library runs (`piperine-api` `Session`) | integration | every AC of LR-03 to LR-08, on both backends where the twin runs on both; differential against `Table::transient` and `Table::on_grid` | `crates/piperine-api/tests/*.rs` | `cargo test -p piperine-api --test <target>` |
| Command line (`piperine-cli/src/commands`) | e2e | the real binary on a project on disk: happy path, the failed run, the unchanged output | `crates/piperine-cli/tests/*.rs` | `cargo test -p piperine-cli --test <target>` |
| Parity (`piperine-parity/tests`) | integration | ngspice at test time under `ToleranceContract`; the skip rule | `crates/piperine-parity/tests/*.rs` | `cargo test -p piperine-parity --test <target>` |
| Example manifests and the Book (`examples/buck/Piperine.toml`, `docs/book/content.html`) | e2e | every listing equals the binary's output | `crates/piperine-cli/tests/{buck,book}.rs` | `cargo test -p piperine-cli --test buck --test book` |
| Repository records (`GAPS.md`, `ROADMAP.md`, `README.md`, `.specs/STATE.md`) | integration | each claim of LR-18 read as text | `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-cli --test docs` |
| Feature records (`.specs/features/long-run/contract.md`) | none | - (build gate only) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes what a run prints | Quick, then `cargo test -p piperine-api`; `cargo test -p piperine-cli --test run --test options --test twins --test commands` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: The baseline and the solver seam

The contract and the byte golden come before any code changes; then the observer and `Keep::Last`.

```
T1 → T3 → T4 → T5
T2
```

### Phase 2: The recorder

`piperine-api` records rows from the seam; the session's transient keeps no step.

```
T6 → T7 → T8 → T9 → T10 → T11
T5 → T10
```

### Phase 3: The command line and the buck's manifest

The streaming verdict, then `startup` as the whole start with the Book and the README reconciled.

```
T12 → T13
T10 → T14
T14 → T15
T14 → T16
```

### Phase 4: The proofs of the long run

Memory, the grid against ngspice, and the buck measured.

```
T10 → T17
T10 → T18
T14 → T19
T19 → T20
```

### Phase 5: The records

```
T21
T22
T23
```

---

## Task Breakdown

### T1: Contract: the merged code against the design, and the net writers audited

**What**: `contract.md`: confirm every file, line and name `design.md` cites against the tree at this task's `HEAD` (the branch was rebased onto `a5f23ea4`), and audit every place the digital engine writes a net (`init`, host commits, resolved nets, reaction commits, event firings) for whether it yields an `ObservationKind::Change` with the net's name and bytes at the sink. Each difference gets the task that absorbs it; a writer with no observation is reported to the orchestrator before T9 and listed for T9's fail-loud check.
**Where**: `.specs/features/long-run/contract.md`
**Depends on**: None
**Reuses**: `design.md` "What the code does today"; `crates/piperine-digital-solver/src/run.rs`, `observe.rs`
**Requirement**: LR-01, LR-05
**Executor**: Sonnet

**Done when**:

- [x] `contract.md` lists each cited location as confirmed or moved, and each net writer with its observation or its absence
- [x] No code is edited
- [x] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): long-run contract against the merged tree`

---

### T2: The byte golden of `tran_rc`, captured before any code changes

**What**: Run the `tran_rc` twin through the real binary with the flags `TWINS` gives it, on this task's unchanged code; store the standard output after the header line as `crates/piperine-cli/tests/goldens/tran_rc.txt`; add a test that compares both backends' output after the header with that file, byte for byte.
**Where**: `crates/piperine-cli/tests/twins.rs`
**Depends on**: None
**Reuses**: `all()`, `TWINS` and the `after_header` closure of the same target
**Requirement**: LR-14
**Executor**: Sonnet

**Done when**:

- [x] The golden file is written from a run made before any source of `piperine-solver` or `piperine-api` is changed; the commit holds the test and the golden only
- [x] The test `tran_rc_prints_the_bytes_it_printed_before_the_long_run` passes in `cargo test -p piperine-cli --test twins`
- [x] Gate check passes: Quick
- [x] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: e2e
**Gate**: quick

**Commit**: `test(cli): pin the bytes of the tran_rc twin`

---

### T3: `StepObserver` and `StepView` on the owner protocol

**What**: `StepObserver`, `StepView` (`time`, `unknown(index)`, `net(id)`), `Protocol::with_step_observer`, and the call after `delivery.flush(t_c)` in `transient_loop`; the default methods `AnalogSide::solution` (empty) and `DiscreteSide::net` (`None`); exports from `protocol` and the prelude. A crate-visible `Protocol::observe_step` makes the same call for the start (T4 calls it).
**Where**: `crates/piperine-solver/src/protocol/step.rs`
**Depends on**: T1
**Reuses**: `Delivery` and `with_sink` in `protocol/transient.rs`; the side doubles of `crates/piperine-solver/tests/protocol_transient.rs`
**Requirement**: LR-01
**Executor**: Sonnet

**Done when**:

- [x] New target `crates/piperine-solver/tests/step_observer.rs` (with its `//!` header) holds, with exactly these names: `the_observer_is_called_once_per_accepted_step_in_time_order`, `the_sink_has_every_observation_up_to_the_step_before_the_observer`, `a_rejected_or_retaken_attempt_is_never_observed`
- [x] Every existing target of `piperine-solver` passes unedited: `cargo test -p piperine-solver`
- [x] Gate check passes: Quick (`Run step`), then `cargo test -p piperine-solver`
- [x] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): the protocol hands each accepted step to an observer`

---

### T4: The observer on a transient of devices

**What**: `HostHooks` carries the observer and `install` puts it on the run's protocol; `Solver::with_step_observer`; `TransientSide::solution` (the Newton solver's current guess) and `DigitalSide::net` (`Digital::read`); `TransientSolver::solve` observes the start after `mark_start`, from an operating point and from a captured step.
**Where**: `crates/piperine-solver/src/analyses/adapters.rs`
**Depends on**: T3
**Reuses**: `HostHooks::install`, `Solver::with_sink`; the helper devices of `crates/piperine-solver/tests/support`
**Requirement**: LR-01
**Executor**: Sonnet

**Done when**:

- [x] `step_observer.rs` gains, with exactly these names: `a_transient_observes_its_start_and_every_accepted_step` (calls equal `stats.steps_accepted + 1`, the first at the start time), `each_observed_step_holds_the_values_of_the_kept_step` (every unknown and every net's bytes against the steps of a `Keep::Every` result), `a_resumed_transient_observes_its_start`, `without_an_observer_the_result_and_the_sink_stream_are_unchanged`
- [x] `cargo test -p piperine-solver --test parity_baseline` passes unedited
- [x] Gate check passes: Quick (`Run step`, `Run baseline`), then `cargo test -p piperine-solver`
- [x] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): a transient of devices observes its steps`

---

### T5: `Keep::Last`

**What**: `Keep { Every, Last }`, `TransientAnalysisOptions::keep` (default `Every`) and `with_keep`. Under `Last`: no step is pushed, `record_settled` is not installed, `device_capture` and `device_mark_start` do nothing, a converged attempt carries an empty snapshot; after the loop `solve` builds the one step from the final solution and attaches the final re-entry state. `Last` with `record_device_state` or a non-empty `probe_selection` is refused at setup.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T4
**Reuses**: the end-of-run re-entry code of `solve`; `crates/piperine-solver/tests/transient_reentry.rs` for the re-entry fixture
**Requirement**: LR-02
**Executor**: Sonnet

**Done when**:

- [ ] `step_observer.rs` gains, with exactly these names: `keep_last_holds_only_the_last_step_with_its_reentry_state`, `reentry_from_the_kept_step_equals_reentry_from_a_full_run` (every step of the two continued runs bit-equal), `the_default_keeps_every_accepted_step`, `keep_last_refuses_device_state_recording`
- [ ] `cargo test -p piperine-solver` passes with every existing target unedited
- [ ] Gate check passes: Quick (`Run step`, `Run baseline`), then Build (end of phase 1)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `feat(solver): a transient can keep only its last step`

---

### T6: `Probe::column`

**What**: `Column { Unknown { index }, Net { id, ty } }` and `Probe::column(&self, circuit) -> Result<Column, Failure>`: a node or a flow to its index in the solution (`Netlist::reference_for`, `idx`), a discrete net to its id in the digital plan; a probe with neither is a `Failure::Backend` naming its label.
**Where**: `crates/piperine-api/src/probe.rs`
**Depends on**: None
**Reuses**: `Session::discrete`'s lookup by name; `Probe::label`
**Requirement**: LR-03
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-api/tests/probes.rs` gains, with exactly these names: `a_node_a_flow_and_a_net_resolve_to_their_columns`, `a_probe_with_no_value_in_the_circuit_is_a_failure_naming_it`
- [ ] Gate check passes: Quick (`Run probes`)
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `feat(api): a probe resolves to its column before the run`

---

### T7: `Recorder`: the rows of a run with no grid

**What**: `Recorder` (crate-private) with `new`, `step` and `finish`, for a run with no `print_step`: each step whose time is not before `from` is a row of the time and each column's cell; `finish` gives the `Table` with the column `time` and the labels.
**Where**: `crates/piperine-api/src/recorder.rs`
**Depends on**: T6
**Reuses**: `Cell::decode`, `Table`
**Requirement**: LR-03
**Executor**: Sonnet

**Done when**:

- [ ] Unit tests with exactly these names pass in `cargo test -p piperine-api --lib`: `recorder::tests::without_a_grid_each_step_at_or_after_from_is_a_row`, `recorder::tests::a_from_past_the_start_drops_the_start_row`, `recorder::tests::a_signal_named_twice_gives_two_equal_columns`, `recorder::tests::a_column_that_reads_nothing_fails_naming_it`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): a recorder makes the rows of a transient as it goes`

---

### T8: `Recorder`: nodes and flows on the print grid

**What**: The grid of a recorder made with `print_step`: `kmin`, `kmax` and the slack as `Table::on_grid` computes them; on each step a row for every pending `k*h` not past it, a node or flow being the step's value when `k*h` equals its time and else `a + (b - a) * (k*h - t0) / (t1 - t0)`; `finish` gives a `k*h` within the slack of the last step that step's values and fails for one further past, with `on_grid`'s message. Only the previous step is held.
**Where**: `crates/piperine-api/src/recorder.rs`
**Depends on**: T7
**Reuses**: the expressions of `Table::on_grid` (`crates/piperine-api/src/table.rs`), copied, not called
**Requirement**: LR-05, LR-06
**Executor**: Sonnet

**Done when**:

- [ ] Unit tests with exactly these names pass in `cargo test -p piperine-api --lib`: `recorder::tests::grid_rows_run_from_the_first_multiple_not_before_from_to_stop`, `recorder::tests::a_real_column_is_bit_equal_to_on_grid` (irregular steps, with and without `from`, compared with `to_bits`), `recorder::tests::a_row_on_an_accepted_step_takes_its_value`, `recorder::tests::each_grid_row_is_made_when_its_step_arrives` (the row count after each step, and one previous step held), `recorder::tests::a_row_within_rounding_of_the_last_step_takes_its_values`, `recorder::tests::a_row_past_the_last_step_fails`, `recorder::tests::a_print_step_past_stop_gives_the_row_at_zero`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: the 7 tests named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): the recorder applies the print grid while the run goes`

---

### T9: `Recorder`: discrete nets on the grid

**What**: `Recorder::change(path, time, bytes)` buffers a change of a printed net; a grid row gives a discrete column the previous step's value with every buffered change whose time is at most `k*h` plus the slack applied in order; a net column is held whatever its type; a clear known byte is `Cell::Unknown`. At each step every discrete column built from the changes must equal the net reader's bytes, else the run fails naming the net and the time. The writers T1 found with no observation are covered by this check or reported.
**Where**: `crates/piperine-api/src/recorder.rs`
**Depends on**: T8
**Reuses**: `Cell::decode`; `contract.md` (T1) for the net writers
**Requirement**: LR-05, LR-06
**Executor**: Sonnet

**Done when**:

- [ ] Unit tests with exactly these names pass in `cargo test -p piperine-api --lib`: `recorder::tests::a_discrete_column_holds_the_value_after_the_last_instant`, `recorder::tests::a_change_between_two_steps_reaches_the_rows_after_it`, `recorder::tests::a_discrete_column_that_changes_only_at_steps_equals_on_grid`, `recorder::tests::a_change_within_rounding_of_a_row_counts_at_it`, `recorder::tests::a_real_typed_net_is_held_not_interpolated`, `recorder::tests::an_unknown_net_value_is_an_unknown_cell`, `recorder::tests::a_net_that_changed_unobserved_fails_naming_it`
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: the 7 tests named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): the recorder holds a discrete net at its last commit`

---

### T10: A session's transient records rows and keeps no step

**What**: The `transient` arm of `Session::analysis`: each probe to its `Column` before the solver is built; a `Recorder` shared by `Delivery` (which now forwards an `ObservationKind::Change` with bytes on a printed net, in `record.rs`) and by the step observer; `TransientAnalysisOptions` with `Keep::Last` and no `record_from`, built by one method a unit test reads; the "ended before its stop" check on the one kept step; the table from `Recorder::finish`. `Table::transient` and `Table::on_grid` are no longer called by the session and are not edited.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T9, T5
**Reuses**: `Delivery`; `tests/ngspice/tran_rc.msl`; the `run`, `twin` and `transient` helpers of `crates/piperine-api/tests/analyses.rs`
**Requirement**: LR-03, LR-05, LR-06, LR-07
**Executor**: Sonnet

**Done when**:

- [ ] New target `crates/piperine-api/tests/grid.rs` (with its `//!` header) holds, with exactly these names: `a_gridless_transient_equals_the_table_of_a_run_that_kept_every_step` (against `Table::transient` over a `Keep::Every` run made in the test, both backends), `the_gridded_tran_rc_is_bit_equal_to_on_grid_on_both_backends`, `a_gridded_run_with_from_starts_at_the_first_multiple_not_before_it`, `a_counter_that_changes_between_analog_steps_is_exact_on_the_grid` (a small clocked design written in the test's own project directory, expected values computed from its clock period), `a_run_table_and_flags_give_equal_tables`, `a_printed_signal_with_no_value_fails_before_the_first_step`
- [ ] The unit test `session::tests::a_transient_keeps_only_the_last_step` passes in `cargo test -p piperine-api --lib`
- [ ] `a_transient_has_one_row_per_accepted_step` and `the_rc_step_follows_one_minus_exp_on_both_backends` pass unedited in `cargo test -p piperine-api --test analyses`
- [ ] `tran_rc_prints_the_bytes_it_printed_before_the_long_run` (T2) and `a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot` pass unedited
- [ ] Known and expected until T14 and T15: `crates/piperine-cli/tests/buck.rs` and the buck listing of `tests/book.rs` may differ on the first row of `startup` (a gridded run with `from` gains the row CLI-11 AC 4 names). They are not this task's gate; any other failing test stops the task
- [ ] Gate check passes: Quick (`Run grid`, `Run apilib`, `Run analyses`), then `cargo test -p piperine-api`; `cargo test -p piperine-cli --test run --test twins`
- [ ] Test count: the 7 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a transient records its printed signals and keeps no step`

---

### T11: The plot draws the run's rows

**What**: `Session::plot` draws from the table the run gave; `Session::accepted` no longer holds a second, per-step table. The test of the amended CLI-22 AC 7 is rewritten: `a_plot_draws_every_accepted_step_whatever_the_print_step` becomes `a_plot_of_a_gridded_run_draws_its_rows`, and `a_plot_of_a_gridless_run_draws_every_accepted_step` is added beside it.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T10
**Reuses**: `Plot::of`; `crates/piperine-cli/tests/options.rs` (`plot_writes_an_svg_and_prints_wrote`)
**Requirement**: LR-17
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/options.rs` holds `a_plot_of_a_gridded_run_draws_its_rows` (as many points as the table has rows: 6 for `examples/rc` `step`) and `a_plot_of_a_gridless_run_draws_every_accepted_step`; the old test name is gone and no other test of the target is edited
- [ ] `the_plot_in_the_chapter_is_the_file_the_binary_writes` passes in `cargo test -p piperine-cli --test book`; if the Book's SVG differs because it drew every step, the Book's file is regenerated from the binary in this task and the commit says so
- [ ] Gate check passes: Full, then Build (end of phase 2; the two `startup` differences T10 names are still expected)
- [ ] Test count: the 2 tests named above; 1 existing test replaced under the amendment, none other removed or edited

**Tests**: e2e
**Gate**: build

**Commit**: `feat(api): a plot draws the rows of its run`

---

### T12: `Verdict` as a record sink

**What**: `Verdict::new()` and `impl RecordSink for Verdict`: a record of the `checks` stream is counted, and kept when its `pass` is not true; every other record is dropped. `Verdict::judge(&[Record])` stays, written over the sink.
**Where**: `crates/piperine-api/src/verdict.rs`
**Depends on**: None
**Reuses**: the existing tests of `verdict.rs`, unedited
**Requirement**: LR-16
**Executor**: Sonnet

**Done when**:

- [ ] Unit tests with exactly these names pass in `cargo test -p piperine-api --lib`: `verdict::tests::a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive`, `verdict::tests::a_record_of_another_stream_never_decides`
- [ ] `verdict::tests::no_false_and_no_unknown_pass_is_a_pass` and `verdict::tests::an_unknown_pass_fails` pass unedited
- [ ] Gate check passes: Quick (`Run apilib`)
- [ ] Test count: the 2 tests named above, and no existing test removed or edited

**Tests**: unit
**Gate**: quick

**Commit**: `feat(api): a verdict judges records as they arrive`

---

### T13: `piperine test` judges with the streaming verdict

**What**: `TestCommand::judge` hands `Session::run` a shared `Verdict` and drops `Collected`; nothing it prints changes.
**Where**: `crates/piperine-cli/src/commands/test.rs`
**Depends on**: T12
**Reuses**: `Verdict` (T12)
**Requirement**: LR-16
**Executor**: Sonnet

**Done when**:

- [ ] `test_prints_pass_with_the_check_count`, `test_prints_fail_and_the_failing_records`, `test_prints_no_table_and_no_other_record`, `a_run_that_does_not_complete_is_a_fail` and `the_last_line_counts_and_the_exit_code_follows` pass unedited in `cargo test -p piperine-cli --test commands`
- [ ] `commands/test.rs` holds no `Vec<Record>` of a run's records
- [ ] Gate check passes: Quick (`Run commands`)
- [ ] Test count: no test added (the output is unchanged and already held); no existing test removed or edited

**Tests**: e2e
**Gate**: quick

**Commit**: `refactor(cli): piperine test keeps no passing record`

---

### T14: `startup` is the whole start

**What**: `[run.startup]` of `examples/buck/Piperine.toml` drops `from`; its comment says the run prints the whole start on a 0.5 µs grid. The `startup` assertions of `crates/piperine-cli/tests/buck.rs` follow (Q2): the table has 1 201 rows from time 0 to 600 µs, the last row holds reference 128, mode 2 and `vout` within the bound the target already asserts.
**Where**: `examples/buck/Piperine.toml`
**Depends on**: T10
**Reuses**: the shared `startup()` fixture and `table`, `column` helpers of `crates/piperine-cli/tests/buck.rs`
**Requirement**: LR-13
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/buck.rs` holds `startup_prints_the_whole_start_on_its_grid` (1 201 rows; first time 0, last 600 µs; times `k * 0.5e-6`), and `the_buck_has_its_bench_and_two_runs`, `startup_prints_no_failed_check`, `idle_keeps_the_mode_off` and `piperine_test_passes_on_the_buck` pass, edited only where they named `from` or the two-row table
- [ ] `examples/buck/Piperine.toml` has no `from` in `[run.startup]`
- [ ] Gate check passes: Quick (`Run buck`)
- [ ] Test count: the 1 test named above; the `startup` assertions edited under Q2, no test removed

**Tests**: e2e
**Gate**: quick

**Commit**: `feat(examples): the buck's startup run prints the whole start`

---

### T15: The Book's `startup` listing follows the binary

**What**: The Book's `[run.startup]` text and its `piperine run startup` output block follow the manifest and the binary: the table of the whole start, shown through the block's own line-range attribute where the Book does not show 1 201 rows, with the sentence beside it saying what the run prints. No other chapter text changes.
**Where**: `docs/book/content.html`
**Depends on**: T14
**Reuses**: `data-cli-output`, `data-cli-project`, `data-cli-lines` as `crates/piperine-cli/tests/book.rs` reads them; `docs/book/build.py`
**Requirement**: LR-18
**Executor**: Sonnet

**Done when**:

- [ ] `the_buck_block_is_what_the_binary_prints`, `every_cli_listing_of_the_book_is_what_the_binary_prints` and `the_chapter_lists_commands_keys_flags_codes` pass unedited in `cargo test -p piperine-cli --test book`
- [ ] `cargo test -p piperine-lang --test book` passes (the Book's `msl` blocks still parse)
- [ ] Gate check passes: Quick (`Run clibook`)
- [ ] Test count: no test added (the Book is held by the existing targets); no test removed or edited

**Tests**: e2e
**Gate**: quick

**Commit**: `docs(book): the buck's startup listing is the whole start`

---

### T16: The buck's README describes the runs as they are

**What**: `examples/buck/README.md` "Status": `piperine run startup` prints the whole start (1 201 rows on a 0.5 µs grid), the sentence about keeping only the last microsecond goes, and a line shows the fine-grid waveform from flags (`--print-step 1e-8 --csv ... --plot ...`). `the_buck_readme_says_how_to_run_it` in `crates/piperine-cli/tests/docs.rs` follows the new text.
**Where**: `examples/buck/README.md`
**Depends on**: T14
**Reuses**: `crates/piperine-cli/tests/docs.rs`
**Requirement**: LR-18
**Executor**: Sonnet

**Done when**:

- [ ] `the_buck_readme_says_how_to_run_it` asserts the README names the whole start and no longer names `from = 599e-6`, and passes in `cargo test -p piperine-cli --test docs`
- [ ] Gate check passes: Quick (`Run docs`), then Build (end of phase 3: every test of the default members green, the `startup` differences of T10 closed)
- [ ] Test count: 1 existing test edited to the new text; none removed

**Tests**: integration
**Gate**: build

**Commit**: `docs(examples): the buck's README describes the whole-start run`

---

### T17: Memory does not grow with the run's length

**What**: New target `memory.rs` with a counting allocator (a `struct` over `std::alloc::System` holding live and peak bytes in two atomics, in `crates/piperine-api/tests/support/heap.rs`, installed by one `#[global_allocator]` static) and one test: `examples/rc` to 5 µs with `print_step = 5e-8` and to 50 µs with `print_step = 5e-7`, both with `max_step = 1e-8` and the same printed signals; each table has 101 rows; the two peaks of live heap during `Session::run`, less the heap live when each run starts, differ by at most 256 KiB.
**Where**: `crates/piperine-api/tests/memory.rs`
**Depends on**: T10
**Reuses**: `examples/rc`; `Project::request` with `Overrides`
**Requirement**: LR-04
**Executor**: Sonnet

**Done when**:

- [ ] The target holds exactly one test, `a_run_ten_times_longer_at_equal_rows_holds_no_more_memory`, asserting both row counts (LR-04 AC 2) and the 256 KiB bound (AC 1), and prints the two peaks on standard error
- [ ] The test fails when the session is run with `Keep::Every` (tried once by hand, reverted, and noted in the commit body): the bound is falsifiable
- [ ] Gate check passes: Quick (`Run memory`)
- [ ] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `test(api): a run's memory does not depend on its length`

---

### T18: The grid against ngspice's TSTEP rows

**What**: `piperine-api` as a dev-dependency of `piperine-parity`, and new target `parity_grid.rs`: `tran_rc` through `Session` to 10 µs with `print_step` equal to the difference of the first two times of ngspice's export and `max_step` a tenth of it, on each selected backend; every row of the export has a table row whose time is within `1e-9 * print_step` of it; `v(out)` of each pair agrees under `ToleranceContract::TRAN_VOLTAGE`; the test interpolates nothing. With the comparison disabled the twin still runs and the harness's skip line is printed.
**Where**: `crates/piperine-parity/tests/parity_grid.rs`
**Depends on**: T10
**Reuses**: `Ngspice`, `Reference`, `Point`, `Coordinate`, `ToleranceContract`, `Backend::selected` of `piperine-parity`; `crates/piperine-parity/tests/parity_tran.rs` as the pattern
**Requirement**: LR-15
**Executor**: Sonnet

**Done when**:

- [ ] The target (with its `//!` header) holds, with exactly these names: `the_grid_of_tran_rc_has_a_row_at_every_time_ngspice_exports`, `tran_rc_on_the_grid_agrees_with_ngspice`, `a_skipped_reference_still_runs_the_twin`
- [ ] `crates/piperine-parity/src/contract.rs` and the five `parity_*` targets are unedited: `git diff --quiet a5f23ea4 -- crates/piperine-parity/src crates/piperine-parity/tests/parity_op.rs crates/piperine-parity/tests/parity_dc.rs crates/piperine-parity/tests/parity_ac.rs crates/piperine-parity/tests/parity_tran.rs crates/piperine-parity/tests/parity_corpus.rs tests/ngspice`
- [ ] `cargo test -p piperine-parity --test parity_corpus` and `cargo test -p piperine-solver --test crate_boundaries --test workspace_shape` pass unedited with the new dev-dependency; if a guard refuses it, the task stops and reports
- [ ] Gate check passes: Quick (`Run paritygrid`, `Run parity`)
- [ ] Test count: the 3 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the print grid against ngspice's own`

---

### T19: The buck's whole start through the host library

**What**: New target `buck_long.rs`: one shared run (`OnceLock`) of `examples/buck`'s `startup` through `Project::request` with `print_step` overridden to `1e-8` and `print` to `buck.vout`, `buck.ctl.reference`, `mode`, `pgood`; a record sink that counts the `checks` records and the failed ones; the counting allocator of T17 measuring the peak inside the initializer; the wall time of `Session::run`. Tests of the run's shape, its checks, the memory cap and the report.
**Where**: `crates/piperine-api/tests/buck_long.rs`
**Depends on**: T14
**Reuses**: `crates/piperine-api/tests/support/heap.rs` (T17); the `OnceLock` pattern of `crates/piperine-cli/tests/buck.rs`; `Settings` with a temporary cache directory
**Requirement**: LR-08, LR-12
**Executor**: Sonnet

**Done when**:

- [ ] The target (with its `//!` header) holds, with exactly these names: `the_whole_start_is_60001_rows_on_a_10_ns_grid` (times `k * 1e-8`, `k` from 0 to 60 000), `every_check_of_the_whole_start_passes` (at least one `checks` record, none failed), `the_whole_start_stays_under_one_gibibyte`, `the_wall_time_and_the_peak_are_reported` (the line printed on standard error holds both numbers; no time is asserted)
- [ ] The run happens once for the target
- [ ] Gate check passes: Quick (`Run bucklong`)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `test(api): the buck's whole start runs through the host library`

---

### T20: Overshoot, settling and ripple against the SPEC

**What**: Four measurements over the shared run's table, each computed in the test from the rows and each limit cited to `examples/buck/SPEC.md` at its assertion: the largest `buck.vout` at most 0.815 V (§10 S1); the settling time, the time of the first row from which every later row is inside 0.8 V ± 10 mV (§2), at least 207.5 µs and at most 307.5 µs (`en` at 1.5 µs from `benches/startup.msl`, the 0.256 ms ramp of §7.2, the ±0.05 ms of S1); the last row with reference 128, mode 2 and `pgood` true (§7.1, §8, §8.6); the peak-to-peak `buck.vout` of the rows of the last 40 µs greater than 0 and at most 10 mV (§2). Each failure message holds the measured value. A limit that is missed is not moved: the task stops and the value goes to the orchestrator for the user.
**Where**: `crates/piperine-api/tests/buck_long.rs`
**Depends on**: T19
**Reuses**: the shared run of T19
**Requirement**: LR-09, LR-10, LR-11
**Executor**: Sonnet

**Done when**:

- [ ] The target gains, with exactly these names: `the_overshoot_is_within_15_mv`, `the_output_settles_into_10_mv_within_the_ramp_window`, `the_start_ends_in_regulation_with_power_good`, `the_ripple_is_within_10_mv_peak_to_peak`
- [ ] The four measured values are printed on standard error and copied into the commit body
- [ ] Gate check passes: Quick (`Run bucklong`), then Build (end of phase 4)
- [ ] Test count: the 4 tests named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `test(api): the buck's start is measured against its spec`

---

### T21: `GAPS.md` says a host run is bounded

**What**: `GAPS.md` §6: the entry "A transient keeps every accepted step" is rewritten to what is left (the solver's default result still keeps every step with its re-entry state, N2; a host run through `piperine-api` records only its printed signals; the step count and the wall time are unchanged), §1's row for the command line names the whole-start run, and §4 still lists `on above(..) { emit!(..) }`. A test in `crates/piperine-cli/tests/docs.rs` reads the file.
**Where**: `GAPS.md`
**Depends on**: None
**Reuses**: `gaps_drops_what_closed_and_lists_what_is_left` as the pattern, unedited
**Requirement**: LR-18
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/docs.rs` holds `gaps_says_a_host_run_is_bounded`: `GAPS.md` no longer holds "a host\n  needs a bounded recording" nor "a 600 µs run held 25 GB" as an open gap of host runs, and still holds "`on above(..) { emit!(..) }` is `P0001`"
- [ ] `gaps_drops_what_closed_and_lists_what_is_left` passes unedited
- [ ] Gate check passes: Quick (`Run docs`)
- [ ] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `docs(gaps): a host run is bounded by what it prints`

---

### T22: `ROADMAP.md` lists what the long run leaves

**What**: `ROADMAP.md`: an entry for `long-run` as delivered, and the three futures of decision 6 by name: multi-rate stepping (digital edges that do not cut analog steps), surrogate or averaged models, periodic steady state on the device ABI as the next feature. A test in `crates/piperine-cli/tests/docs.rs` reads the file.
**Where**: `ROADMAP.md`
**Depends on**: None
**Reuses**: `the_roadmap_marks_the_cli_item_done` as the pattern, unedited
**Requirement**: LR-18
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/docs.rs` holds `the_roadmap_lists_what_the_long_run_leaves`, asserting the three futures are named
- [ ] Gate check passes: Quick (`Run docs`)
- [ ] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: integration
**Gate**: quick

**Commit**: `docs(roadmap): the long run and what follows it`

---

### T23: MD-66 in `.specs/STATE.md`

**What**: Append MD-66 to `.specs/STATE.md` `## Macro Decisions`, as `design.md` words it: observation is the run's, not the language's; a transient hands each accepted step to a host observer and a host keeps rows, never steps; a new observation option is a key of `[run.<name>]` with its flag; measurements over a waveform are host code. It extends MD-65 item 1 and supersedes nothing. A test in `crates/piperine-cli/tests/docs.rs` reads the file.
**Where**: `.specs/STATE.md`
**Depends on**: None
**Reuses**: `state_records_the_cli_decision` as the pattern, unedited
**Requirement**: LR-07, LR-18
**Executor**: Sonnet

**Done when**:

- [ ] `crates/piperine-cli/tests/docs.rs` holds `state_records_the_observation_decision`, asserting `### MD-66` and the sentence that observation is the run's
- [ ] `every_run_key_has_its_flag` passes unedited in `cargo test -p piperine-cli --lib` (no key and no flag was added)
- [ ] Gate check passes: Quick (`Run docs`, `Run clilib`), then Build (end of the feature)
- [ ] Test count: the 1 test named above, and no existing test removed or edited

**Tests**: integration
**Gate**: build

**Commit**: `docs(state): observation is the run's (MD-66)`

---

## Checks

**C1** - The observer is called once per accepted step, with the step's time, in increasing order. (LR-01, AC 1; T3)
Proof: run step | test the_observer_is_called_once_per_accepted_step_in_time_order ... ok

**C2** - Each observed step holds every unknown and every net's bytes of the step a full run keeps. (LR-01, AC 1; T4)
Proof: run step | test each_observed_step_holds_the_values_of_the_kept_step ... ok

**C3** - A transient observes its start once before any step: calls equal `steps_accepted + 1`. (LR-01, AC 2; T4)
Proof: run step | test a_transient_observes_its_start_and_every_accepted_step ... ok

**C4** - A transient resumed from a captured step observes its start. (LR-01, AC 2; T4)
Proof: run step | test a_resumed_transient_observes_its_start ... ok

**C5** - At each observer call the sink already holds every observation whose time is not after the call's. (LR-01, AC 3; T3)
Proof: run step | test the_sink_has_every_observation_up_to_the_step_before_the_observer ... ok

**C6** - A rejected or retaken attempt is never observed. (LR-01, AC 4; T3)
Proof: run step | test a_rejected_or_retaken_attempt_is_never_observed ... ok

**C7** - With no observer the result and the sink stream are what they were. (LR-01, AC 5; T4)
Proof: run step | test without_an_observer_the_result_and_the_sink_stream_are_unchanged ... ok

**C8** - `Keep::Last` gives a result of exactly one step, the last, with its re-entry state. (LR-02, AC 1; T5)
Proof: run step | test keep_last_holds_only_the_last_step_with_its_reentry_state ... ok

**C9** - Re-entry from the kept step gives the same steps as re-entry from a full run's last step. (LR-02, AC 2; T5)
Proof: run step | test reentry_from_the_kept_step_equals_reentry_from_a_full_run ... ok

**C10** - The default keeps every accepted step. (LR-02, AC 3; T5)
Proof: run step | test the_default_keeps_every_accepted_step ... ok

**C11** - A gridless transient has `time`, one column per printed signal and one row per accepted step, with the cells `Table::transient` gives over a full run. (LR-03, AC 1 and 2; T10)
Proof: run grid | test a_gridless_transient_equals_the_table_of_a_run_that_kept_every_step ... ok

**C12** - A gridless run with `from` past the start has no row for the start. (LR-03, AC 1; edge case; T7)
Proof: run apilib | test recorder::tests::a_from_past_the_start_drops_the_start_row ... ok

**C13** - A printed signal with no value in the bound circuit fails before the first step, naming the signal. (LR-03, AC 3; T10)
Proof: run grid | test a_printed_signal_with_no_value_fails_before_the_first_step ... ok

**C14** - A session's transient asks the solver to keep only the last step. (LR-03, AC 4; T10)
Proof: run apilib | test session::tests::a_transient_keeps_only_the_last_step ... ok

**C15** - `examples/rc` to 5 µs and to 50 µs at 101 rows each: the two peaks of live heap differ by at most 256 KiB. (LR-04, AC 1 and 2; T17)
Proof: run memory | test a_run_ten_times_longer_at_equal_rows_holds_no_more_memory ... ok

**C16** - Grid rows run from the first `k*h` not before `from` to the last not past `stop`, the first cell being `k*h`. (LR-05, AC 1; T8)
Proof: run apilib | test recorder::tests::grid_rows_run_from_the_first_multiple_not_before_from_to_stop ... ok

**C17** - Through a session, a gridded run with `from` starts at the first multiple not before it. (LR-05, AC 1; T10)
Proof: run grid | test a_gridded_run_with_from_starts_at_the_first_multiple_not_before_it ... ok

**C18** - A node or a flow on the grid is linear between the two accepted steps around the row, bit-equal to `Table::on_grid`. (LR-05, AC 2; LR-06, AC 1; T8)
Proof: run apilib | test recorder::tests::a_real_column_is_bit_equal_to_on_grid ... ok

**C19** - A row whose time is an accepted step's takes that step's value. (LR-05, AC 3; T8)
Proof: run apilib | test recorder::tests::a_row_on_an_accepted_step_takes_its_value ... ok

**C20** - A discrete net on the grid holds the value after the last instant at or before the row. (LR-05, AC 4; T9)
Proof: run apilib | test recorder::tests::a_discrete_column_holds_the_value_after_the_last_instant ... ok

**C21** - Each grid row is made when its step arrives, from one previous step. (LR-05, AC 5; T8)
Proof: run apilib | test recorder::tests::each_grid_row_is_made_when_its_step_arrives ... ok

**C22** - A grid time past the last step by more than `1e-9 * h` gives no row and fails the run. (LR-05, AC 6; T8)
Proof: run apilib | test recorder::tests::a_row_past_the_last_step_fails ... ok

**C23** - A transient that ends before its `stop` gives no table, CSV or plot. (LR-05, AC 7; T10)
Proof: run run | test a_transient_that_ends_before_its_stop_fails_for_table_csv_and_plot ... ok

**C24** - `tran_rc` on the grid through a session is bit-equal to `Table::on_grid` over the per-step table, on both backends. (LR-06, AC 1; T10)
Proof: run grid | test the_gridded_tran_rc_is_bit_equal_to_on_grid_on_both_backends ... ok

**C25** - A discrete column that changes only at accepted steps equals `Table::on_grid`'s. (LR-06, AC 2; T9)
Proof: run apilib | test recorder::tests::a_discrete_column_that_changes_only_at_steps_equals_on_grid ... ok

**C26** - A change strictly between two accepted steps reaches the rows after it. (LR-06, AC 3; T9)
Proof: run apilib | test recorder::tests::a_change_between_two_steps_reaches_the_rows_after_it ... ok

**C27** - Through a session, a counter that changes between analog steps is exact on the grid. (LR-05, AC 4; LR-06, AC 3; T10)
Proof: run grid | test a_counter_that_changes_between_analog_steps_is_exact_on_the_grid ... ok

**C28** - Every key of a run table has its flag, and no key or flag was added. (LR-07, AC 1; T23)
Proof: run clilib | test every_run_key_has_its_flag ... ok

**C29** - The same `print`, `print_step`, `from` and `stop` from a run table and from flags give equal tables: the policy is the `RunSpec`'s. (LR-07, AC 2 and 3; T10)
Proof: run grid | test a_run_table_and_flags_give_equal_tables ... ok

**C30** - The buck's `startup` on a 10 ns grid is 60 001 rows at `k * 1e-8`. (LR-08, AC 1; T19)
Proof: run bucklong | test the_whole_start_is_60001_rows_on_a_10_ns_grid ... ok

**C31** - Every `checks` record of the whole start passes, and there is at least one. (LR-08, AC 2; T19)
Proof: run bucklong | test every_check_of_the_whole_start_passes ... ok

**C32** - The largest `buck.vout` is at most 0.815 V. (LR-09, AC 1; T20)
Proof: run bucklong | test the_overshoot_is_within_15_mv ... ok

**C33** - The output settles inside 0.8 V ± 10 mV between 207.5 and 307.5 µs. (LR-10, AC 1; T20)
Proof: run bucklong | test the_output_settles_into_10_mv_within_the_ramp_window ... ok

**C34** - The last row holds reference 128, mode 2 and `pgood` true. (LR-10, AC 2; T20)
Proof: run bucklong | test the_start_ends_in_regulation_with_power_good ... ok

**C35** - The ripple over the last 40 µs is greater than 0 and at most 10 mV peak to peak. (LR-11, AC 1; T20)
Proof: run bucklong | test the_ripple_is_within_10_mv_peak_to_peak ... ok

**C36** - The peak live heap of the whole start is at most 1 GiB. (LR-12, AC 1; T19)
Proof: run bucklong | test the_whole_start_stays_under_one_gibibyte ... ok

**C37** - The wall time and the peak are printed, and no time is asserted. (LR-12, AC 2; T19)
Proof: run bucklong | test the_wall_time_and_the_peak_are_reported ... ok

**C38** - `piperine run startup` prints 1 201 rows from 0 to 600 µs on its 0.5 µs grid. (LR-13, AC 1; T14)
Proof: run buck | test startup_prints_the_whole_start_on_its_grid ... ok

**C39** - `piperine test` in `examples/buck` prints `PASS idle`, `PASS startup` and `2 runs, 2 passed, 0 failed`. (LR-13, AC 2; T14)
Proof: run buck | test piperine_test_passes_on_the_buck ... ok

**C40** - The `tran_rc` twin prints, on each backend, the bytes it printed before this feature. (LR-14, AC 1; T2)
Proof: run twins | test tran_rc_prints_the_bytes_it_printed_before_the_long_run ... ok

**C41** - The seven twins still print the same bytes on both backends. (LR-14, AC 1; T10)
Proof: run twins | test the_two_backends_print_the_same_bytes ... ok

**C42** - The transient parity of `tran_rc` passes with its target unedited. (LR-14, AC 2; T10)
Proof: run parity | test tran_rc ... ok

**C43** - The solver's pinned transient baseline passes with its target unedited. (LR-14, AC 2; T5)
Proof: run baseline | test parity_rc_transient ... ok

**C44** - The parity sources, the tolerance contract and the twins are unedited since `a5f23ea4`. (LR-14, AC 2; T18)
Proof: `git diff --quiet a5f23ea4 -- crates/piperine-parity/src crates/piperine-parity/tests/parity_op.rs crates/piperine-parity/tests/parity_dc.rs crates/piperine-parity/tests/parity_ac.rs crates/piperine-parity/tests/parity_tran.rs crates/piperine-parity/tests/parity_corpus.rs crates/piperine-solver/tests/parity_baseline.rs tests/ngspice`

**C45** - Every row of ngspice's export of `tran_rc` has a grid row within `1e-9 * print_step` of its time. (LR-15, AC 1; T18)
Proof: run paritygrid | test the_grid_of_tran_rc_has_a_row_at_every_time_ngspice_exports ... ok

**C46** - `v(out)` of each paired row agrees with ngspice under `TRAN_VOLTAGE`, with no interpolation in the test. (LR-15, AC 2; T18)
Proof: run paritygrid | test tran_rc_on_the_grid_agrees_with_ngspice ... ok

**C47** - With the comparison disabled the twin still runs. (LR-15, AC 3; T18)
Proof: run paritygrid | test a_skipped_reference_still_runs_the_twin ... ok

**C48** - A verdict fed one record at a time counts the checks and keeps only the failed. (LR-16, AC 1; T12)
Proof: run apilib | test verdict::tests::a_verdict_counts_checks_and_keeps_only_the_failed_as_they_arrive ... ok

**C49** - `piperine test` prints the same `PASS` line with the check count. (LR-16, AC 2; T13)
Proof: run commands | test test_prints_pass_with_the_check_count ... ok

**C50** - `piperine test` prints the same `FAIL` line and failed records. (LR-16, AC 2; T13)
Proof: run commands | test test_prints_fail_and_the_failing_records ... ok

**C51** - A record of another stream never decides a verdict. (LR-16, AC 3; T12)
Proof: run apilib | test verdict::tests::a_record_of_another_stream_never_decides ... ok

**C52** - A plot of a gridded run draws the rows of its table. (LR-17, AC 1; T11)
Proof: run options | test a_plot_of_a_gridded_run_draws_its_rows ... ok

**C53** - A plot of a gridless run draws every accepted step. (LR-17, AC 2; T11)
Proof: run options | test a_plot_of_a_gridless_run_draws_every_accepted_step ... ok

**C54** - Every command-line listing of the Book is what the binary prints. (LR-18, AC 1; T15)
Proof: run clibook | test every_cli_listing_of_the_book_is_what_the_binary_prints ... ok

**C55** - The Book's buck block is what the binary prints. (LR-18, AC 1; T15)
Proof: run clibook | test the_buck_block_is_what_the_binary_prints ... ok

**C56** - `GAPS.md` no longer says a host run keeps every accepted step, and still lists `on above(..) { emit!(..) }`. (LR-18, AC 2; T21)
Proof: run docs | test gaps_says_a_host_run_is_bounded ... ok

**C57** - `ROADMAP.md` names multi-rate stepping, surrogate models and periodic steady state as what follows. (LR-18, AC 2; T22)
Proof: run docs | test the_roadmap_lists_what_the_long_run_leaves ... ok

**C58** - The buck's README describes the whole-start run. (LR-18, AC 3; T16)
Proof: run docs | test the_buck_readme_says_how_to_run_it ... ok

**C59** - `.specs/STATE.md` holds MD-66: observation is the run's. (LR-07, AC 3; T23)
Proof: run docs | test state_records_the_observation_decision ... ok

**C60** - A `print_step` greater than `stop` gives the single row at time 0. (edge case; T8)
Proof: run apilib | test recorder::tests::a_print_step_past_stop_gives_the_row_at_zero ... ok

**C61** - A row within `1e-9 * print_step` past the last step takes the last step's values. (edge case; T8)
Proof: run apilib | test recorder::tests::a_row_within_rounding_of_the_last_step_takes_its_values ... ok

**C62** - A net change within `1e-9 * print_step` of a row's time counts at it. (edge case; T9)
Proof: run apilib | test recorder::tests::a_change_within_rounding_of_a_row_counts_at_it ... ok

**C63** - A net whose known byte is clear is an unknown cell. (edge case; T9)
Proof: run apilib | test recorder::tests::an_unknown_net_value_is_an_unknown_cell ... ok

**C64** - A discrete net of a real type is held, not interpolated. (LR-05, AC 4; Assumptions; T9)
Proof: run apilib | test recorder::tests::a_real_typed_net_is_held_not_interpolated ... ok

**C65** - A net that changed with no observation fails the run, naming the net. (design, Error Handling; T9)
Proof: run apilib | test recorder::tests::a_net_that_changed_unobserved_fails_naming_it ... ok

**C66** - A signal printed twice gives two equal columns. (edge case; T7)
Proof: run apilib | test recorder::tests::a_signal_named_twice_gives_two_equal_columns ... ok

**C67** - A gridless transient still has one row per accepted step, its existing test unedited. (LR-03, AC 1; `msl-cli` CLI-05 AC 2; T10)
Proof: run analyses | test a_transient_has_one_row_per_accepted_step ... ok

**C68** - A probe resolves to its column, and one with no value in the circuit is a failure naming it. (LR-03, AC 3; T6)
Proof: run probes | test a_probe_with_no_value_in_the_circuit_is_a_failure_naming_it ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** (T4) `StepView::time()` is the analog side's own time of the step (`AnalogSide::step_time`, the time the kept `TransientStep` records, a sum of steps), falling back to the protocol's tick time `t_c` when a side keeps none: the two differ in the last bit (3e-5 against 3.0000000000000004e-5) and the recorder's grid arithmetic must read what `Table::on_grid` reads off the kept steps (LR-06 bit-equality). T1: no net writer lacks a `Change` observation (see `contract.md`). The four questions of the spec were answered before the tasks were written (spec.md, "Questions answered").
**Dispatched:** Executors all Sonnet (user 2026-11-17: "Implementação sempre em Sonnet e verificação em Opus"). Batch 1 (phase 1, T1-T5) - Sonnet worker, 2026-11-17.
**Walked:**

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5

Phase 1:  T1, T2, T3, T4, T5
Phase 2:  T6, T7, T8, T9, T10, T11
Phase 3:  T12, T13, T14, T15, T16
Phase 4:  T17, T18, T19, T20
Phase 5:  T21, T22, T23
```

Execution is strictly sequential, in task order.

**Batches** (about 7 tasks each, whole phases):

| Batch | Phases | Tasks | Count |
| ----- | ------ | ----- | ----- |
| 1 | Phase 1 | T1 to T5 | 5 |
| 2 | Phase 2 | T6 to T11 | 6 |
| 3 | Phase 3 | T12 to T16 | 5 |
| 4 | Phases 4 and 5 | T17 to T23 | 7 |

Batch 1 is all solver work and mostly Opus; batch 2 holds the three delicate recorder tasks; batch 4 holds the two long runs of the buck (about two and a half minutes each time `buck_long` runs).

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1 | 1 document | ✅ Granular |
| T2 | 1 test and its golden | ✅ Granular |
| T3 | 1 module (the observer) and its call site | ✅ Granular |
| T4 | the wiring of one seam in one file, two one-line side methods | ✅ Granular |
| T5 | 1 option and its branches in one file | ✅ Granular |
| T6 | 1 function | ✅ Granular |
| T7 | 1 type, one mode | ✅ Granular |
| T8 | 1 mode of the same type | ✅ Granular |
| T9 | 1 mode of the same type | ✅ Granular |
| T10 | 1 arm of one function, plus the one branch in `Delivery` that feeds it | ⚠️ Cohesive: neither half runs without the other |
| T11 | 1 function | ✅ Granular |
| T12 | 1 trait implementation | ✅ Granular |
| T13 | 1 function | ✅ Granular |
| T14 | 1 run table and the assertions that read it | ✅ Granular |
| T15 | 1 listing | ✅ Granular |
| T16 | 1 section of a README | ✅ Granular |
| T17 | 1 test target and its allocator | ✅ Granular |
| T18 | 1 test target | ✅ Granular |
| T19 | 1 test target: the run | ✅ Granular |
| T20 | 4 measurements in the same target | ✅ Granular |
| T21, T22, T23 | 1 document each | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | none | ✅ Match |
| T2 | None | none | ✅ Match |
| T3 | T1 | T1 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | None | none | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T8 | T8 → T9 | ✅ Match |
| T10 | T9, T5 (phase 1) | T9 → T10, T5 → T10 | ✅ Match |
| T11 | T10 | T10 → T11 | ✅ Match |
| T12 | None | none | ✅ Match |
| T13 | T12 | T12 → T13 | ✅ Match |
| T14 | T10 (phase 2) | T10 → T14 | ✅ Match |
| T15 | T14 | T14 → T15 | ✅ Match |
| T16 | T14 | T14 → T16 | ✅ Match |
| T17 | T10 (phase 2) | T10 → T17 | ✅ Match |
| T18 | T10 (phase 2) | T10 → T18 | ✅ Match |
| T19 | T14 (phase 3) | T14 → T19 | ✅ Match |
| T20 | T19 | T19 → T20 | ✅ Match |
| T21, T22, T23 | None | none | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1 | feature records | none | none | ✅ OK |
| T2 | command line (a test target) | e2e | e2e | ✅ OK |
| T3, T4, T5 | solver protocol and analyses | integration | integration | ✅ OK |
| T6 | host library runs (a probe against a bound circuit) | integration | integration | ✅ OK |
| T7, T8, T9 | host library rules | unit | unit | ✅ OK |
| T10 | host library runs | integration | integration | ✅ OK |
| T11 | host library runs, proven from the command line | e2e | e2e | ✅ OK |
| T12 | host library rules | unit | unit | ✅ OK |
| T13 | command line | e2e | e2e | ✅ OK |
| T14, T15 | example manifests and the Book | e2e | e2e | ✅ OK |
| T16 | repository records | integration | integration | ✅ OK |
| T17, T19, T20 | host library runs | integration | integration | ✅ OK |
| T18 | parity | integration | integration | ✅ OK |
| T21, T22, T23 | repository records | integration | integration | ✅ OK |
