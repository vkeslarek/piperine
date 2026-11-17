# Solver Hygiene Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/solver-hygiene/design.md`
**Status**: Approved
**Approved**: 2026-11-17 - user: "Pode fazer com Opus por agora, com Sonnet implementando e Opus verificando"
**Build**: `cargo build -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver -p piperine-parity -p piperine-cli --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
**Run deviationlib**: `cargo test -p piperine-parity --lib deviation`
**Run deviation**: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test deviation -- --nocapture`
**Run parity**: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test parity_op --test parity_dc --test parity_ac --test parity_tran --test parity_corpus`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run analogddt**: `cargo test -p piperine-analog-solver --test assembly_ddt`
**Run analoglib**: `cargo test -p piperine-analog-solver --lib`
**Run reentry**: `cargo test -p piperine-solver --test transient_reentry`
**Run failure**: `cargo test -p piperine-solver --test failure_rules`
**Run runerrors**: `cargo test -p piperine-parity --test run_errors`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run truncation**: `cargo test -p piperine-solver --test truncation`
**Run abitran**: `cargo test -p piperine-solver --test abi_tran`
**Run abidc**: `cargo test -p piperine-solver --test abi_dc`
**Run protocol**: `cargo test -p piperine-solver --test protocol_transient`
**Run hostset**: `cargo test -p piperine-solver --test host_set`
**Run boundary**: `cargo test -p piperine-parity --test boundary`
**Run removed**: `cargo test -p piperine-solver --test removed_surface`
**Run abidevices**: `cargo test -p piperine-solver --test abi_devices`
**Run digital**: `cargo test -p piperine-digital-solver --test checkpoint`
**Run instant**: `cargo test -p piperine-digital-solver --test instant`

The deviation table every numerics commit carries: `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test deviation -- --nocapture`, run before and after the change, both outputs pasted in the commit body.

Executors: **every task is Sonnet** (user, 2026-11-17: "Sonnet implementando e Opus verificando"). Opus verifies the feature.

**Answers.** Q0 to Q7 were answered on 2026-11-17, all option (a) ("Aceito todas (Recommended)", `context.md`); every task below follows them as written.

Rules every task follows:

- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; no tolerance is written outside it; no existing `.cir` or twin `.msl` changes. `tests/ngspice/devices.msl` changes only in T4.
- **Only the goldens and pins of design.md "Goldens, pins and twins that may move" may move,** under the task named there, and only when the commit body carries the deviation table before and after and every moved twin is at least as close to ngspice. A golden is re-captured from the real binary (`piperine run` with the flags of `crates/piperine-cli/tests/twins.rs`), never hand-edited. A re-pinned literal cites, in its comment, the value on a converged grid (the pattern of `parity_baseline.rs:211-223`).
- **A numerics change that moves a twin further from ngspice, breaks convergence of any target, or makes a buck target fail is not committed**: the task stops and reports the measured numbers.
- **Tests state the spec, not the code.** Expected values come from the spec's ACs, the analytic response, ngspice, or Part V; each cited at its assertion. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped; the existing tests that change are only those of design.md's table.
- **AGENTS.md idiom rules bind** (MD-13): no loose function, no macro, modules named after what they do, no `unwrap()`/`expect()` on a path user input reaches. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- **Cross-lane files** (design.md "Cross-lane touches"): `spec_text.rs` is appended to, never edited above; `GAPS.md` keeps its section numbering (`crates/piperine-cli/tests/docs.rs` reads it).
- Zero warnings on every crate the `**Build**` line names. Cargo with `-p <crate>` or the default members: never `--workspace`, never `cargo clean`. In a worktree sharing a target directory, `touch crates/<crate>/src/lib.rs` before a gate. Parity runs with `env -u PIPERINE_NGSPICE`.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (build and verify, test placement), `CLAUDE.md` ("Where the tests are", MD-28), `crates/piperine-lang/tests/suite_hygiene.rs` (the enforcing guard), `.specs/STATE.md` MD-38 (parity is measured) and MD-57 (spec edits with `spec_text.rs`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Analog sub-solver math and strategies (`piperine-analog-solver/src/{math,convergence.rs,error.rs}`) | unit | all branches; 1:1 to the ACs; coefficient-level values (lesson L-001) | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-analog-solver --lib` |
| Analog assembly (`piperine-analog-solver/src/device`) | integration | every AC with a hand-written device; the operating point and the first steps | `crates/piperine-analog-solver/tests/assembly_*.rs` | `cargo test -p piperine-analog-solver --test <target>` |
| Digital engine (`piperine-digital-solver/src`) | integration | every AC; the journal against `checkpoint()`, nested marks | `crates/piperine-digital-solver/tests/*.rs` | `cargo test -p piperine-digital-solver --test <target>` |
| Owner analyses, bind and protocol (`piperine-solver/src`) | integration | every AC on hand-written devices; error code and names asserted | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver --test <target>` |
| Parity and source-level runs (`piperine-parity`) | integration | ngspice at test time under `ToleranceContract`; source-level errors on both backends | `crates/piperine-parity/tests/*.rs`, `crates/piperine-parity/src` | `env -u PIPERINE_NGSPICE cargo test -p piperine-parity --test <target>` |
| Spec text (`docs/spec/`) | integration | one assertion per decided fact (MD-57) | `crates/piperine-lang/tests/spec_text.rs` | `cargo test -p piperine-lang --test spec_text` |
| Repository records (`GAPS.md`, `ROADMAP.md`, `.specs/STATE.md`) | none | - (build gate and the lone proofs of `## Checks`) | - | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After a task with unit tests or one integration target | the `Run` line(s) the task's `Done when` names |
| Full | After a task that changes numerics or a run's output | Quick, then `**Run parity**`, `**Run twins**`, `cargo test -p piperine-solver`, `cargo test -p piperine-analog-solver`, `cargo test -p piperine-digital-solver`, `env -u PIPERINE_NGSPICE cargo test -p piperine-parity` |
| Build | After each phase, and after a task with `Tests: none` | the `**Build**` line, then `cargo test` at the repository root (the default members) |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Evidence and the nonlinear-charge defect

The deviation table first, so every later commit has its "before"; then the silent defect the
`tran_dq` twin exposes, then the twin.

```
T1 → T2 → T3 → T4
```

### Phase 2: Exact re-entry

The lane's priority (N1), and its opt-out (N2).

```
T5 → T6 → T7 → T8
```

### Phase 3: Loud, named run errors

```
T9 → T10 → T11 → T12 → T13 → T14 → T15 → T16 → T17
```

### Phase 4: Spec text of the decided numerics

MD-57: the specification leads.

```
T18 → T19
```

### Phase 5: Truncation error and the step floor

```
T20 → T21 → T22 → T23 → T24
```

### Phase 6: Newton strategy and the restart step

```
T25 → T26 → T27 → T28 → T29
```

### Phase 7: Start time, dead code and guards

```
T30 → T31 → T32 → T33 → T34
```

### Phase 8: The digital journal

```
T35 → T36 → T37 → T38
```

### Phase 9: Records

```
T39 → T40
```

---

## Task Breakdown

### T1: The deviation of a twin against its contract

**What**: `Deviation { fixture, quantity, worst_ratio, at }` and `Deviation::of(contract, rows)`: the worst `|golden − actual| / (abstol + reltol·max(|golden|, |actual|))` over `(Coordinate, golden, actual)` rows, with `ToleranceContract::abstol()`/`reltol()`; `Deviation::table(&[Deviation]) -> String`, one aligned line per deviation.
**Where**: `crates/piperine-parity/src/deviation.rs`
**Depends on**: None
**Reuses**: `contract.rs` accessors and `Coordinate` (not edited)
**Requirement**: HYG-06
**Executor**: Sonnet

**Done when**:

- [x] `the_worst_ratio_is_the_largest_difference_over_its_bound` (non-uniform rows, the worst not first or last; lesson L-006) and `a_pair_at_the_bound_has_ratio_one` pass under `**Run deviationlib**`
- [x] `contract.rs` is unchanged (`git diff --stat` shows no line of it)
- [x] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(parity): the deviation of a twin against its contract`

---

### T2: The deviation table of every twin, and the baseline

**What**: A target that runs the 7 twins with each `parity_*` target's own policy and prints `Deviation::table`, plus, per transient, accepted and rejected steps and `dt_min_floor_hits`. Run it at this task's code; paste its output in the commit body as the feature's baseline. Measure, with a probe not committed, the `dt_min_floor_hits` of the transients of `buck_boundary`, `buck_time` and `buck_whole` and write the numbers on the Handoff's `**Settled mid-build:**` line (Q3's risk).
**Where**: `crates/piperine-parity/tests/deviation.rs`
**Depends on**: T1
**Reuses**: `Bench`, `Ngspice`, `Waveform`, the policies of `parity_{op,dc,ac,tran}.rs`
**Requirement**: HYG-06
**Executor**: Sonnet

**Done when**:

- [x] `the_table_covers_every_twin` passes (asserts one row per twin of `tests/ngspice/*.msl` but `devices`, no number) under `**Run deviation**`
- [x] The baseline table is in the commit body; the buck floor-hit numbers are on the Handoff
- [x] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): the deviation table of every twin`

---

### T3: A nonlinear charge through a limit keeps the operating point

**What**: Bisect the defect of design.md "The new defect" with a hand-written device (`LIMIT` on a junction voltage, `DDT` of `tt·is·(exp(vd/nvt) − 1)`), then fix it so each `DDT` history row is the term's argument at the converged solution, not its limit linearization. If the root cause lies outside the three solver crates, stop and report (lane A).
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs`
**Depends on**: T2
**Reuses**: `tests/support/devices.rs` (a limited device), `limit::linear`, `ddt::History`
**Requirement**: HYG-05
**Executor**: Sonnet

**Done when**:

- [x] `a_nonlinear_charge_through_a_limit_keeps_the_operating_point` and `the_ddt_history_holds_the_charge_at_the_solution` pass under `**Run analogddt**` and fail on the parent commit (shown in the commit body)
- [x] The deviation table before and after is in the commit body; no twin moves
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(analog-solver): a ddt history row is the charge at the solution`

---

### T4: The nonlinear-charge twin `tran_dq`

**What**: `Diode` in `tests/ngspice/devices.msl` gains `tt: f64 = 0.0 where tt >= 0.0` and `ddt(tt * id)`; new `tests/ngspice/tran_dq.cir` (`V1 vin 0 DC 1`, `R1 vin a 1000`, `C1 a 0 1.5915494309189535e-9`, `D1 a 0 DMOD`, `.model DMOD D (is=1e-14 n=1 tt=1e-6 cjo=0 rs=0)`, `.options interp reltol=1e-4 trtol=1`, `tran 0.1u 10u uic`, `wrdata tran_dq_tran v(a)`, a header stating why the options are set: design.md "Risks") and `tran_dq.msl` (`a` held at 0.0); `fn tran_dq()` beside `fn tran_rc()` under the same policy.
**Where**: `crates/piperine-parity/tests/parity_tran.rs`
**Depends on**: T3
**Reuses**: `TranRun::options`, `compare_on`
**Requirement**: HYG-06
**Executor**: Sonnet

**Done when**:

- [x] `tran_dq` passes under `**Run parity**` on both backends; `every_twin_has_a_cir_and_a_test` passes
- [x] `every_twin_prints_the_bytes_it_printed_before_the_long_run` passes under `**Run twins**` (the `tt` default moves no golden); if it does not, `tt` goes to a separate `DiodeTt` module instead
- [x] The deviation table after is in the commit body, with `tran_dq`'s row
- [x] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `test(parity): the nonlinear-charge twin tran_dq`

---

### T5: The step-size controller's memory can be captured

**What**: `StepperMemory { values: Vec<f64> }`; `StepperStrategy::memory(&self) -> StepperMemory` and `resume(&mut self, &StepperMemory)` with defaults (empty, no-op); `PiController` stores and restores `prev_error`.
**Where**: `crates/piperine-analog-solver/src/convergence.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `PiController`
**Requirement**: HYG-04
**Executor**: Sonnet

**Done when**:

- [x] `the_pi_controller_resumes_its_memory` (a controller resumed from another's memory proposes the same next step, bit for bit, after one and after several accepts) passes under `**Run analoglib**`
- [x] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(analog-solver): the step controller's memory is captured and resumed`

---

### T6: The re-entry state carries the analog side's resume state

**What**: `AnalogResume { newton, dt, prev_h, last_dt, last_step_accepted, restart_pending, stepper }` as a field `analog` of `ReentryState`, built and read through `pub(crate)` methods only.
**Where**: `crates/piperine-solver/src/core/result.rs`
**Depends on**: T5
**Reuses**: `CircularArrayBuffer2`, `StepperMemory`
**Requirement**: HYG-04
**Executor**: Sonnet

**Done when**:

- [ ] The field is filled at every capture site in the same commit (no unused field, zero warnings; lesson L-016 if `transient.rs` must change to compile)
- [ ] `**Run reentry**` passes unchanged
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): the re-entry state carries the analog resume state`

---

### T7: Re-entry continues the run bit for bit

**What**: `device_reentry_state` captures `AnalogResume` from the Newton buffer, `TimeLoop`, `prev_h`, `DeviceRun::last_dt` and the plan's stepper; `device_resume` restores the Newton buffer (no more two pushed rows), `TimeLoop::dt`, `prev_h`, `last_step_accepted`, `sets_just_applied` and the stepper.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T6
**Reuses**: `TransientSolver::with_initial_state`, `build_clocked_rc` of `transient_reentry.rs`
**Requirement**: HYG-04
**Executor**: Sonnet

**Done when**:

- [ ] `a_reentry_with_another_stop_lands_on_the_old_stop` passes under `**Run reentry**` (edge case 3, §16.4)
- [ ] `a_reentry_from_every_recorded_step_continues_bit_for_bit` (every recorded step of `build_clocked_rc`, same options and stop; times and every unknown compared by `to_bits`), `a_reentry_after_a_jump_takes_the_same_restart` and `the_reentry_state_holds_the_newton_history_and_the_controller` pass under `**Run reentry**`; the existing tests of the target still pass
- [ ] `cargo test -p piperine-solver --test pss` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(solver): re-entry continues the run bit for bit`

---

### T8: Re-entry state on the last step only, as an option

**What**: `TransientAnalysisOptions::reentry: Reentry { EveryStep (default), LastStep }` with `with_reentry`; under `LastStep`, `device_capture` and `device_mark_start` attach nothing, `DigitalSide::record_settled` is not installed, the final step keeps its state.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T7
**Reuses**: `Keep`, the final capture after the loop
**Requirement**: HYG-13
**Executor**: Sonnet

**Done when**:

- [ ] `reentry_last_step_attaches_state_to_the_last_step_only`, `the_default_reentry_keeps_every_step_state` and `a_middle_step_under_last_step_cannot_be_reentered` (edge case 4) pass under `**Run reentry**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): re-entry state per step is optional`

---

### T9: A typed run error with its spec code

**What**: `RunCode { E0503, E0504, E0505, E0507, E0508 }`, `RunError { code, domain, detail, time, unknowns, names }`, `Error::Run`, `Error::run`, `Error::code`, `with_time`, `with_unknowns`, `with_names`, and the `Display` of design.md.
**Where**: `crates/piperine-analog-solver/src/error.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `SolverDomain`
**Requirement**: HYG-01
**Executor**: Sonnet

**Done when**:

- [ ] `a_run_error_displays_its_code_time_and_names` and `a_run_error_without_names_shows_its_unknowns` pass under `**Run analoglib**`
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(analog-solver): a typed run error with its spec code`

---

### T10: Which unknowns make a Jacobian singular

**What**: `SingularityReport::of(size, &stamps)`: the dense matrix of the stamps, faer's dense rank-revealing factorization, the null vector's unknowns above 1e-6 of its largest component; `skipped: Some(size)` above `MAX_DENSE = 2000`.
**Where**: `crates/piperine-analog-solver/src/math/singular.rs`
**Depends on**: T9
**Reuses**: `Stamp`, faer
**Requirement**: HYG-01
**Executor**: Sonnet

**Done when**:

- [ ] `a_loop_of_two_potential_sources_is_singular_in_both_flows`, `a_node_reached_only_through_capacitors_is_singular_at_dc` and `a_singular_system_above_the_dense_bound_says_it_skipped` pass under `**Run analoglib**`
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(analog-solver): name the unknowns of a singular Jacobian`

---

### T11: Newton reports E0503 and the unknowns it could not converge

**What**: A failed symbolic or numeric factorization, or a non-finite solution of finite stamps, becomes `E0503` with `SingularityReport`'s unknowns (faer's text kept as the cause); iteration exhaustion carries the unknowns that failed the update or residual test (`Tolerances::failing`, same formulas as `has_converged`/`residual_test`) and the active limits.
**Where**: `crates/piperine-analog-solver/src/math/newton_raphson.rs`
**Depends on**: T10
**Reuses**: `Tolerances`, `NonLinearSystem::any_limiting_report`
**Requirement**: HYG-01, HYG-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_non_finite_solution_of_finite_stamps_is_e0503` and `exhaustion_names_the_unknowns_that_failed` pass under `**Run analoglib**`; the text `Linear solver returned NaN/Inf` no longer exists
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `feat(analog-solver): Newton reports E0503 and its failing unknowns`

---

### T12: The owner names the unknowns of a run error

**What**: `NameMap::describe(unknown)` (node by its bound pins, flow by instance and branch) and `Bound::name(&mut Error)`; every owner entry (`dc`, `transient`, `ac`, the protocol's operating point) names its error before returning.
**Where**: `crates/piperine-solver/src/core/bind.rs`
**Depends on**: T11
**Reuses**: `NameMap::pins`, the module shapes' pin names
**Requirement**: HYG-01
**Executor**: Sonnet

**Done when**:

- [ ] `a_singular_solve_names_the_pins_of_its_node`, `a_loop_of_sources_names_both_instances` and `an_instance_is_named_once_in_a_singular_report` (edge case 2) pass under `**Run failure**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): a run error names its nodes and branches`

---

### T13: A non-finite contribution is E0507

**What**: `device/assembly.rs:633` raises `Error::run(E0507, Device, …)` with the instance, cell and origin, citing §17.1 rule 4. Under Q7(a), `section_15_a_non_finite_stamp_is_a_convergence_failure` asserts `E0507` in place of `§7.3.2.1` (its other assertions unchanged).
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs`
**Depends on**: T12
**Reuses**: the existing naming of the cell's origin
**Requirement**: HYG-02
**Executor**: Sonnet

**Done when**:

- [ ] `a_non_finite_contribution_is_e0507_and_names_the_instance` passes under `**Run failure**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `fix(analog-solver): a non-finite contribution is E0507`

---

### T14: A non-finite stamp of the solver's own is E0507

**What**: `stamp_validation::non_finite_stamp_error` returns `E0507` with the row and column it found, the text citing §17.1 rule 4 instead of the LRM.
**Where**: `crates/piperine-analog-solver/src/stamp_validation.rs`
**Depends on**: T13
**Reuses**: its existing scan
**Requirement**: HYG-02
**Executor**: Sonnet

**Done when**:

- [ ] `a_non_finite_stamp_is_e0507_with_its_row` passes under `**Run analoglib**`; `grep -rn '7.3.2.1' crates/piperine-*solver*/src` finds nothing
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `fix(analog-solver): the stamp check cites E0507`

---

### T15: An equilibrium that never converges is E0505

**What**: When the plan's last strategy fails, `DcSolver` returns `E0505` with the failing unknowns or active limits of the last attempt, named.
**Where**: `crates/piperine-solver/src/analyses/dc.rs`
**Depends on**: T14
**Reuses**: `ConvergencePlan::solve`'s last error
**Requirement**: HYG-03
**Executor**: Sonnet

**Done when**:

- [ ] `an_equilibrium_that_never_converges_is_e0505_with_its_unknowns` passes under `**Run failure**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): an equilibrium that never converges is E0505`

---

### T16: A step that fails at the floor is E0508, or E0504 when it reads a jump

**What**: The Newton failure kept in `DeviceRun::failed` becomes `E0508` with the attempt's time and failing unknowns; when the failing step is the first after a `Restart::Integrator` delivery, `E0504` naming the delivered slots' nets (`NameMap::slot_nets` and the net names).
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T15
**Reuses**: `device_deliver`'s boundary, `TimeLoop::sets_just_applied`
**Requirement**: HYG-03
**Executor**: Sonnet

**Done when**:

- [ ] `a_step_that_fails_newton_at_the_floor_is_e0508_with_its_time` and `a_jump_step_that_fails_at_the_floor_is_e0504_naming_the_carrier` pass under `**Run failure**`
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): a step that fails at the floor is E0508 or E0504`

---

### T17: Run errors from source

**What**: A target loading small `.msl` designs from `crates/piperine-parity/tests/run_errors/` through `Bench::load_file_on` on both backends: two `Vsource`s of 1 V and 2 V in parallel (`E0503`, both paths), a node joined to the rest only by `Capacitor`s in an `op` (`E0503`, the node's pins), a contribution that is NaN at the operating point (`E0507`, the instance).
**Where**: `crates/piperine-parity/tests/run_errors.rs`
**Depends on**: T16
**Reuses**: `tests/ngspice/devices.msl` by `mod` path or a copy under `run_errors/`
**Requirement**: HYG-01, HYG-02
**Executor**: Sonnet

**Done when**:

- [ ] `a_loop_of_two_vsources_from_source_is_e0503`, `a_floating_node_from_source_is_e0503` and `a_nan_contribution_from_source_is_e0507` pass under `**Run runerrors**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(parity): run errors from source carry their codes and names`

---

### T18: Part V states the decided rules

**What**: Per the answers: §15.2 rule 6 (the restart step, Q5), §16.4 or §16 (a later start, Q6), the §17.2 Resolution "Where rule 1 is measured" (the `ddt` states with `chgtol`, Q1), the `E0508` row (also a truncation failure at the floor, Q3), the X1 note of §15.2 left as it is. Each fact asserted in a new `spec_text.rs` test.
**Where**: `docs/spec/part_v_execution.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `spec_text.rs`'s existing helpers (appended at the end)
**Requirement**: HYG-17
**Executor**: Sonnet

**Done when**:

- [ ] `part_v_states_the_solver_hygiene_rules` passes under `**Run spectext**`; no existing test of the target is edited
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): Part V states the solver hygiene rules`

---

### T19: Appendix C states the decided numerics

**What**: C.2 rows "convergence" (no damping by default, `NodeDamping` opt-in, Q4), "truncation error" (Milne over the `ddt` states, ngspice's tolerance, Q1/Q2), "step control" (the floor fails `E0508` after one retry, Q3), "restart" (`0.1·min(last step, gap to the next time point)`, Q5); C.2.1 the rows that change; C.2.2 item 4 rewritten (the floor no longer relaxes §17.2.1).
**Where**: `docs/spec/appendix_c_implementation.md`
**Depends on**: T18
**Reuses**: the table layout of C.2
**Requirement**: HYG-17
**Executor**: Sonnet

**Done when**:

- [ ] `appendix_c_states_the_solver_hygiene_numerics` passes under `**Run spectext**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(spec): Appendix C states the solver hygiene numerics`

---

### T20: The truncation estimate over the `ddt` states

**What**: `DdtState { q: [f64; 3], i: [f64; 2], abstol: Option<f64> }` and `TrBdf2::state_lte(&[DdtState], &Tolerances, h)`: the Milne difference per state normalized by `max(h·(abstol + reltol·max|i|), reltol·max(|q_n|, |q_n+1|, chgtol))`, the worst returned, no skip.
**Where**: `crates/piperine-analog-solver/src/math/integration.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `TrBdf2::GAMMA`, the existing Milne tests' shape
**Requirement**: HYG-07, HYG-08
**Executor**: Sonnet

**Done when**:

- [ ] `the_state_estimate_reads_chgtol` (a value under `chgtol` and the same one above it give different normalized errors, hand-computed), `the_state_estimate_is_zero_for_a_linear_charge` and `a_rise_from_flat_is_measured_and_rejected` (flat over `γh`, a rise above the tolerance over the rest: error above `trtol`) pass under `**Run analoglib**`
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(analog-solver): the truncation estimate over the ddt states`

---

### T21: The assembly exposes its `ddt` states

**What**: `Assembly::ddt_states() -> Vec<DdtState>` from each `DDT` term's history and the companion current of the last two stages; a term's own `abstol` (not `DEFAULT`) is carried.
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs`
**Depends on**: T20
**Reuses**: `ddt::History`, `stamp_ddt`'s current
**Requirement**: HYG-07
**Executor**: Sonnet

**Done when**:

- [ ] `the_assembly_reports_each_ddt_state_after_a_step` passes under `**Run analogddt**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(analog-solver): the assembly reports its ddt states`

---

### T22: The step gate measures the states

**What**: `assess_step` takes `TrBdf2::state_lte(bound.analog.ddt_states(), …)` in place of the node Milne; `run.node_indices` goes. Re-pin `parity_rc_transient` (step count, `v_last`) under Q1 with converged-grid evidence; re-capture `goldens/tran_rc.txt` only if it moved.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T21
**Reuses**: `assess_step`, `propose_dt` (band unchanged)
**Requirement**: HYG-07
**Executor**: Sonnet

**Done when**:

- [ ] New target `crates/piperine-solver/tests/truncation.rs` (`//!` header) with `a_source_driven_nonlinear_capacitor_meets_the_current_tolerance` (1 V 100 kHz sine across `q = c0·v + c1·v³`, the source current against the analytic `dq/dt` at every accepted step of one period, bound `reltol·|i| + abstol·10³`) and `a_larger_chgtol_takes_fewer_steps` pass under `**Run truncation**`
- [ ] `tran_rc` and `tran_dq` pass under `**Run parity**`; `**Run twins**` passes (with a re-captured golden only as allowed)
- [ ] The deviation table before and after is in the commit body
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): the truncation test measures the ddt states`

---

### T23: The node Milne estimate loses its skip

**What**: Remove the asymmetric skip of `milne_lte_indexed` (Q2); if Q1(a) leaves it unused after T22, delete it and its tests, else keep it without the skip.
**Where**: `crates/piperine-analog-solver/src/math/integration.rs`
**Depends on**: T22
**Reuses**: none
**Requirement**: HYG-08
**Executor**: Sonnet

**Done when**:

- [ ] `grep -n '0.1 \* d1.max(d2)' crates/piperine-analog-solver/src` finds nothing
- [ ] `**Run analoglib**` and `**Run parity**` pass; the deviation table after is in the commit body
- [ ] Gate check passes: Full

**Tests**: unit
**Gate**: full

**Commit**: `fix(analog-solver): no truncation component is skipped`

---

### T24: A step that fails the error at the floor fails E0508

**What**: `device_reject_lte` retries once at `dt_min` and then returns `E0508` (or `E0504` after a delivery) with the time and the failing states as unknowns; `dt_min_floor_hits` counts the retries; the `tracing::warn!` goes.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T23
**Reuses**: T16's naming
**Requirement**: HYG-09
**Executor**: Sonnet

**Done when**:

- [ ] `an_unannounced_corner_fails_e0508_at_the_floor` (a hand-written source stepping 0 to 1 V at 1.03 µs computed from `TIME`, across 1 nF, `dt_min` = 1 ns) `the_floor_is_retried_once_before_failing` and `the_floor_at_the_stop_time_names_that_time` (edge case 6) pass under `**Run truncation**`
- [ ] `every_twin_prints_the_bytes_it_printed_before_the_long_run` passes under `**Run twins**`; T2's buck floor-hit numbers were 0 (else this task stopped)
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): a step that cannot meet the error at the floor fails E0508`

---

### T25: Newton strategies as ngspice

**What**: `NoDamping` (the new default, today's convergence test) and `NodeDamping` (ngspice `niiter.c:297-323`: static solves, node unknowns, a limit active, `maxΔv > 10` → `max(10/maxΔv, 0.1)`); `damp_update` takes a `DampContext`; `DampedNewton` and its `prelude.rs` export go; `ConvergencePlan::default()` takes `NoDamping`.
**Where**: `crates/piperine-analog-solver/src/convergence.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `NewtonStrategy`
**Requirement**: HYG-10
**Executor**: Sonnet

**Done when**:

- [ ] `node_damping_follows_ngspice` (each of its four conditions off leaves the update; on, the factor at maxΔv 20 V and 200 V) and `no_damping_leaves_the_update` pass under `**Run analoglib**`
- [ ] Gate check passes: Quick

**Tests**: unit
**Gate**: quick

**Commit**: `feat(analog-solver): Newton strategies as ngspice`

---

### T26: The DC driver iterates with its plan's strategy

**What**: `DcSolver` owns a `ConvergencePlan` (`with_plan`), `equilibrium` and `newton` use it and its `newton()`; re-pin `parity_diode_dc_point`/`parity_divider_op_dc` under Q4 if they moved; re-capture the DC goldens only as allowed.
**Where**: `crates/piperine-solver/src/analyses/dc.rs`
**Depends on**: T25
**Reuses**: `ConvergencePlan`
**Requirement**: HYG-10
**Executor**: Sonnet

**Done when**:

- [ ] `the_dc_driver_iterates_with_the_plans_strategy` (a plan with a counting strategy is called) passes under `**Run abidc**`
- [ ] `diode_iv`, `diode_rev`, `diode_series` pass under `**Run parity**`; `**Run twins**` passes; the deviation table before and after is in the commit body
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(solver): the DC driver iterates with its plan's Newton strategy`

---

### T27: The transient driver iterates with its plan's strategy

**What**: `execute_timestep` takes `self.plan.newton()` instead of the hard-coded strategy.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T26
**Reuses**: `ConvergencePlan::newton`
**Requirement**: HYG-10
**Executor**: Sonnet

**Done when**:

- [ ] `the_transient_driver_iterates_with_the_plans_strategy` passes under `**Run abitran**`
- [ ] `**Run parity**` and `**Run twins**` pass; the deviation table after is in the commit body
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(solver): the transient driver iterates with its plan's Newton strategy`

---

### T28: The step proposal sees the next scheduled time

**What**: `AnalogSide::propose(now, stop, next_due: Option<f64>)`; the protocol passes `DiscreteSide::next_due` in seconds; every implementer (`analyses/adapters.rs`, `tests/support/stub_sides.rs`, the scripted sides) changes in this commit (lesson L-016).
**Where**: `crates/piperine-solver/src/protocol/sides.rs`
**Depends on**: T27
**Reuses**: `DiscreteSide::next_due`
**Requirement**: HYG-11
**Executor**: Sonnet

**Done when**:

- [ ] `the_proposal_sees_the_next_due_time` passes under `**Run protocol**`; the other tests of the target pass unchanged
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(solver): the step proposal sees the next scheduled time`

---

### T29: The step after a jump is a tenth

**What**: `device_deliver` on `Restart::Integrator` sets a restart cap of `0.1·last_dt`; `device_propose` takes `min(controller, cap, 0.1·(next_due − now))`, floored at `dt_min`; `IntegratorSmall` unchanged. Under Q5(a)/Q7(a), `boundary.rs`'s `run_with(8.9e-6, 1e-9)` becomes `1e-8` and its F-B7 comment is replaced by this rule's.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T28
**Reuses**: `device_deliver`, `device_propose`
**Requirement**: HYG-11
**Executor**: Sonnet

**Done when**:

- [ ] `the_step_after_a_jump_is_capped_at_a_tenth_of_the_last` and `two_jumps_in_one_restart_step_restart_from_the_later` (edge case 5) pass under `**Run abitran**`
- [ ] `a_carrier_change_is_a_time_point_and_starts_the_response` passes at the 10 ns cap under `**Run boundary**`
- [ ] `a_scheduled_host_write_lands_and_restarts_small` passes under `**Run hostset**`
- [ ] The buck targets of `crates/piperine-parity` pass (else this task stops)
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `fix(solver): the step after a jump is a tenth of the last`

---

### T30: The digital engine starts at a tick

**What**: `Digital::start_at(tick)`: `init` commits and `START` at `(tick, 0)`; `start()` is `start_at(Tick(0))`.
**Where**: `crates/piperine-digital-solver/src/engine.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Digital::start`
**Requirement**: HYG-12
**Executor**: Sonnet

**Done when**:

- [ ] `start_at_commits_init_at_its_tick` passes under `**Run instant**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(digital-solver): the engine starts at a given tick`

---

### T31: A transient that starts later starts from an operating point

**What**: Drop the refusal of `start_time ≠ 0` without a captured step: the operating point runs at `(t_start, 0)` with the engine started at `tick(t_start)`; a non-finite or negative start fails first. Under Q6(a), `a_late_start_needs_a_step_a_transient_recorded` becomes `a_late_start_solves_the_operating_point_at_its_start`.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T30
**Reuses**: `Protocol::operating_point`, `empty_digital`
**Requirement**: HYG-12
**Executor**: Sonnet

**Done when**:

- [ ] `a_late_start_solves_the_operating_point_at_its_start`, `a_late_start_runs_no_instant_before_it` and `a_start_that_is_not_finite_fails_before_the_operating_point` pass under `**Run reentry**`
- [ ] `cargo test -p piperine-solver --test pss` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): a transient that starts later starts from an operating point`

---

### T32: The dead Newton hooks are gone

**What**: Remove `apply_limit`, `apply_limiting_reports`, their call sites and the stale comment (`newton_raphson.rs:46`, `:77`, `:208`, `:366-374`); add a `removed_surface.rs` row.
**Where**: `crates/piperine-analog-solver/src/math/newton_raphson.rs`
**Depends on**: T31
**Reuses**: `removed_surface.rs`'s `names_found`
**Requirement**: HYG-15
**Executor**: Sonnet

**Done when**:

- [ ] `the_dead_newton_hooks_are_gone` passes under `**Run removed**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `refactor(analog-solver): remove the dead Newton hooks`

---

### T33: The `.sp` documentation names the host's ports

**What**: Rewrite `analyses/sp.rs:5-9` and the `SpPort` doc (`:48`): ports are `SpPort` values the host gives the analysis; add a `removed_surface.rs` row for `PHDL` and `@rfport`.
**Where**: `crates/piperine-solver/src/analyses/sp.rs`
**Depends on**: T32
**Reuses**: none
**Requirement**: HYG-15
**Executor**: Sonnet

**Done when**:

- [ ] `the_phdl_port_text_is_gone` passes under `**Run removed**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `docs(solver): the sp analysis takes its ports from the host`

---

### T34: The partials guard matches the module

**What**: `every_hand_written_device_is_in_a_partials_check` accepts a device only when a check's module argument is that device (`module::<X>`, or the helper that returns it); a planted mismatch is caught by a new test.
**Where**: `crates/piperine-solver/tests/abi_devices.rs`
**Depends on**: T33
**Reuses**: `device_impls`
**Requirement**: HYG-16
**Executor**: Sonnet

**Done when**:

- [ ] `every_hand_written_device_is_in_a_partials_check` and `a_check_of_another_module_does_not_count` pass under `**Run abidevices**`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `test(solver): the partials guard matches the verified module`

---

### T35: The store journals its first writes

**What**: `Store::mark() -> StoreMark`, `Store::rewind(&StoreMark)`: between marks, the first previous `NetState` of each written net is journaled; marks nest.
**Where**: `crates/piperine-digital-solver/src/store.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `Store::restore_net`
**Requirement**: HYG-14
**Executor**: Sonnet

**Done when**:

- [ ] `a_store_rewind_restores_every_written_net` passes under `cargo test -p piperine-digital-solver --test store`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(digital-solver): the store journals its first writes`

---

### T36: The queue journals its operations

**What**: `Queue::mark()`/`rewind()`: pushes, pops and transport removals since the mark are undone in reverse; marks nest.
**Where**: `crates/piperine-digital-solver/src/queue.rs`
**Depends on**: T35
**Reuses**: `Queue::push`
**Requirement**: HYG-14
**Executor**: Sonnet

**Done when**:

- [ ] `a_queue_rewind_restores_its_pending_commits` passes under `cargo test -p piperine-digital-solver --test queue`
- [ ] Gate check passes: Quick

**Tests**: integration
**Gate**: quick

**Commit**: `feat(digital-solver): the queue journals its operations`

---

### T37: The engine marks and rewinds through the journal

**What**: `Digital::mark() -> Mark` and `Digital::rewind(&Mark)` over the store and queue journals, the dirty set and the counters; the look-ahead (`advance.rs`) uses them; `checkpoint()`/`restore()` stay.
**Where**: `crates/piperine-digital-solver/src/engine.rs`
**Depends on**: T36
**Reuses**: `checkpoint()` as the oracle
**Requirement**: HYG-14
**Executor**: Sonnet

**Done when**:

- [ ] `a_rewind_equals_the_checkpoint_taken_at_the_mark` (random commit and instant sequences, compared with a `checkpoint()` taken at the mark), `nested_marks_rewind_in_order` and `a_mark_journals_only_the_nets_that_changed` (1 of 1 000 nets changed: one entry) pass under `**Run digital**`
- [ ] `cargo test -p piperine-digital-solver` passes
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `perf(digital-solver): marks journal changes instead of copying every net`

---

### T38: The protocol's digital side uses the journal

**What**: `DigitalSide::mark`/`restore` (`adapters.rs:177`) use `Digital::mark`/`rewind`; the full copy and every net's bytes are taken only where a re-entry state is captured.
**Where**: `crates/piperine-solver/src/analyses/adapters.rs`
**Depends on**: T37
**Reuses**: T8's `Reentry`
**Requirement**: HYG-14
**Executor**: Sonnet

**Done when**:

- [ ] `**Run reentry**`, `**Run protocol**`, `cargo test -p piperine-solver --test checkpoint_restore` and the buck targets of `crates/piperine-parity` pass
- [ ] Gate check passes: Full

**Tests**: integration
**Gate**: full

**Commit**: `perf(solver): the protocol marks the digital side through its journal`

---

### T39: GAPS and ROADMAP say what closed

**What**: Remove `GAPS.md` §6 rows V1 to V11, V14, V15 and the first bullet of §4.8 (keep V12, V13, V16 and the section numbers; fix the counts line); add the HYG-05 defect nowhere (it is closed); in `ROADMAP.md`, close the N1 to N3 line and the non-zero-start line.
**Where**: `GAPS.md`
**Depends on**: None (its phase follows the previous one)
**Reuses**: none
**Requirement**: HYG-17
**Executor**: Sonnet

**Done when**:

- [ ] The lone proof of C49 exits 0; `cargo test -p piperine-cli --test docs` passes
- [ ] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs: GAPS and ROADMAP close the solver hygiene points`

---

### T40: MD-67 records the feature's decisions

**What**: `.specs/STATE.md` gains `MD-67`: run codes typed in the solver and named in the owner; re-entry exact and its opt-out; the truncation basis, damping, floor, restart and start as answered, each with the user's answer quoted.
**Where**: `.specs/STATE.md`
**Depends on**: T39
**Reuses**: the MD entry layout
**Requirement**: HYG-17
**Executor**: Sonnet

**Done when**:

- [ ] The lone proof of C50 exits 0
- [ ] Gate check passes: Build

**Tests**: none
**Gate**: build

**Commit**: `docs(specs): MD-67 records the solver hygiene decisions`

---

## Checks

**C1** - The deviation of a twin is the worst ratio of difference to contract bound, found anywhere in the rows. (HYG-06, AC 4; T1)
Proof: run deviationlib | test deviation::tests::the_worst_ratio_is_the_largest_difference_over_its_bound ... ok

**C2** - A pair exactly at the bound has ratio 1. (HYG-06, AC 4; T1)
Proof: run deviationlib | test deviation::tests::a_pair_at_the_bound_has_ratio_one ... ok

**C3** - The table has one row per twin. (HYG-06, AC 4; T2)
Proof: run deviation | test the_table_covers_every_twin ... ok

**C4** - A nonlinear charge through a limit leaves the operating point within `reltol·|v| + vntol` at the first step. (HYG-05, AC 1; T3)
Proof: run analogddt | test a_nonlinear_charge_through_a_limit_keeps_the_operating_point ... ok

**C5** - Each `DDT` history row is the argument's value at the converged solution. (HYG-05, AC 2; T3)
Proof: run analogddt | test the_ddt_history_holds_the_charge_at_the_solution ... ok

**C6** - `tran_dq` agrees with ngspice under `TRAN_VOLTAGE` on both backends. (HYG-06, AC 3; T4)
Proof: run parity | test tran_dq ... ok

**C7** - Every twin has a `.cir` and a test. (HYG-06, AC 3; T4)
Proof: run parity | test every_twin_has_a_cir_and_a_test ... ok

**C8** - A controller resumed from another's memory proposes the same step, bit for bit. (HYG-04, AC 3; T5)
Proof: run analoglib | test convergence::tests::the_pi_controller_resumes_its_memory ... ok

**C9** - A re-entry from every recorded step equals the continuous run bit for bit. (HYG-04, AC 1; T7)
Proof: run reentry | test a_reentry_from_every_recorded_step_continues_bit_for_bit ... ok

**C10** - A re-entry from the step after a delivery takes the same restart. (HYG-04, AC 2; T7)
Proof: run reentry | test a_reentry_after_a_jump_takes_the_same_restart ... ok

**C11** - The captured state holds the Newton history and the controller state. (HYG-04, AC 3; T7)
Proof: run reentry | test the_reentry_state_holds_the_newton_history_and_the_controller ... ok

**C12** - Under `Reentry::LastStep` only the last step carries a re-entry state. (HYG-13, AC 4; T8)
Proof: run reentry | test reentry_last_step_attaches_state_to_the_last_step_only ... ok

**C13** - The default still gives every step its re-entry state. (HYG-13, AC 4; T8)
Proof: run reentry | test the_default_reentry_keeps_every_step_state ... ok

**C14** - A run error displays code, time and names. (HYG-01, AC 1; T9)
Proof: run analoglib | test error::tests::a_run_error_displays_its_code_time_and_names ... ok

**C15** - A run error not yet named shows its unknown indices. (HYG-01, AC 1; T9)
Proof: run analoglib | test error::tests::a_run_error_without_names_shows_its_unknowns ... ok

**C16** - Two potential sources in a loop are singular in both flows. (HYG-01, AC 1; T10)
Proof: run analoglib | test singular::tests::a_loop_of_two_potential_sources_is_singular_in_both_flows ... ok

**C17** - A node reached only through capacitors is singular at DC. (HYG-01, AC 1; T10)
Proof: run analoglib | test singular::tests::a_node_reached_only_through_capacitors_is_singular_at_dc ... ok

**C18** - Above 2 000 unknowns the report says the diagnosis was skipped. (HYG-01, edge case 1; T10)
Proof: run analoglib | test singular::tests::a_singular_system_above_the_dense_bound_says_it_skipped ... ok

**C19** - A non-finite solution of finite stamps is `E0503`. (HYG-01, AC 2; T11)
Proof: run analoglib | test newton_raphson::tests::a_non_finite_solution_of_finite_stamps_is_e0503 ... ok

**C20** - Newton's exhaustion names the unknowns that failed. (HYG-03, AC 4; T11)
Proof: run analoglib | test newton_raphson::tests::exhaustion_names_the_unknowns_that_failed ... ok

**C21** - A singular solve names the pins of its node. (HYG-01, AC 1; T12)
Proof: run failure | test a_singular_solve_names_the_pins_of_its_node ... ok

**C22** - A loop of sources names both instances. (HYG-01, AC 1; T12)
Proof: run failure | test a_loop_of_sources_names_both_instances ... ok

**C23** - A non-finite contribution is `E0507`, names the instance, and cites no LRM section. (HYG-02, AC 3; T13)
Proof: run failure | test a_non_finite_contribution_is_e0507_and_names_the_instance ... ok

**C24** - The solver's own stamp check is `E0507` with its row. (HYG-02, AC 3; T14)
Proof: run analoglib | test stamp_validation::tests::a_non_finite_stamp_is_e0507_with_its_row ... ok

**C25** - An equilibrium that never converges is `E0505` with its unknowns. (HYG-03, AC 4; T15)
Proof: run failure | test an_equilibrium_that_never_converges_is_e0505_with_its_unknowns ... ok

**C26** - A Newton failure at the floor is `E0508` with its time. (HYG-03, AC 5; T16)
Proof: run failure | test a_step_that_fails_newton_at_the_floor_is_e0508_with_its_time ... ok

**C27** - The failing step after a jump is `E0504` naming the carrier. (HYG-03, AC 6; T16)
Proof: run failure | test a_jump_step_that_fails_at_the_floor_is_e0504_naming_the_carrier ... ok

**C28** - Two `Vsource`s in parallel from source fail `E0503` naming both, on both backends. (HYG-01, AC 7; T17)
Proof: run runerrors | test a_loop_of_two_vsources_from_source_is_e0503 ... ok

**C29** - A floating node from source fails `E0503` naming its pins. (HYG-01, AC 1; T17)
Proof: run runerrors | test a_floating_node_from_source_is_e0503 ... ok

**C30** - A NaN contribution from source fails `E0507` naming the instance. (HYG-02, AC 3; T17)
Proof: run runerrors | test a_nan_contribution_from_source_is_e0507 ... ok

**C31** - Part V states the restart step, the later start, the truncation basis and the `E0508` condition. (HYG-17, AC 2; T18)
Proof: run spectext | test part_v_states_the_solver_hygiene_rules ... ok

**C32** - Appendix C states the damping, truncation, floor and restart numerics. (HYG-17, AC 2; T19)
Proof: run spectext | test appendix_c_states_the_solver_hygiene_numerics ... ok

**C33** - The state estimate reads `chgtol`. (HYG-07, AC 1; T20)
Proof: run analoglib | test integration::tests::the_state_estimate_reads_chgtol ... ok

**C34** - The state estimate is zero for a linear charge. (HYG-07, AC 1; T20)
Proof: run analoglib | test integration::tests::the_state_estimate_is_zero_for_a_linear_charge ... ok

**C35** - A rise from flat is measured and rejected. (HYG-08, AC 4; T20)
Proof: run analoglib | test integration::tests::a_rise_from_flat_is_measured_and_rejected ... ok

**C36** - The assembly reports each `ddt` state after a step. (HYG-07, AC 1; T21)
Proof: run analogddt | test the_assembly_reports_each_ddt_state_after_a_step ... ok

**C37** - A source-driven nonlinear capacitor's current meets its tolerance at every step. (HYG-07, AC 2; T22)
Proof: run truncation | test a_source_driven_nonlinear_capacitor_meets_the_current_tolerance ... ok

**C38** - A larger `chgtol` takes fewer steps. (HYG-07, AC 3; T22)
Proof: run truncation | test a_larger_chgtol_takes_fewer_steps ... ok

**C39** - `tran_rc` stays within `TRAN_VOLTAGE`. (HYG-07, AC 5; T22)
Proof: run parity | test tran_rc ... ok

**C40** - An unannounced corner fails `E0508` at the floor. (HYG-09, AC 1; T24)
Proof: run truncation | test an_unannounced_corner_fails_e0508_at_the_floor ... ok

**C41** - The floor is retried once before failing. (HYG-09, AC 2; T24)
Proof: run truncation | test the_floor_is_retried_once_before_failing ... ok

**C42** - The twins print the same bytes (the floor rule moves nothing that never reaches it). (HYG-09, AC 3; T24)
Proof: run twins | test every_twin_prints_the_bytes_it_printed_before_the_long_run ... ok

**C43** - `NodeDamping` follows ngspice's four conditions and factor. (HYG-10, AC 2; T25)
Proof: run analoglib | test convergence::tests::node_damping_follows_ngspice ... ok

**C44** - The default strategy leaves the update. (HYG-10, AC 2; T25)
Proof: run analoglib | test convergence::tests::no_damping_leaves_the_update ... ok

**C45** - The DC driver iterates with its plan's strategy. (HYG-10, AC 1; T26)
Proof: run abidc | test the_dc_driver_iterates_with_the_plans_strategy ... ok

**C46** - The transient driver iterates with its plan's strategy. (HYG-10, AC 1; T27)
Proof: run abitran | test the_transient_driver_iterates_with_the_plans_strategy ... ok

**C47** - The diode sweep converges at every point with the default strategy, within `DC_CURRENT`. (HYG-10, AC 3; T26)
Proof: run parity | test diode_iv ... ok

**C48** - The proposal sees the next scheduled time. (HYG-11, AC 1; T28)
Proof: run protocol | test the_proposal_sees_the_next_due_time ... ok

**C49** - `GAPS.md` §6 lists none of V1 to V11, V14, V15 and keeps V12, V13, V16. (HYG-17, AC 1; T39)
Proof: `! grep -nE '^\| V(1|2|3|4|5|6|7|8|9|10|11|14|15) \|' GAPS.md && grep -cE '^\| V1[236] \|' GAPS.md | grep -qx 3`

**C50** - `.specs/STATE.md` records `MD-67`. (HYG-17, AC 3; T40)
Proof: `grep -q '^### MD-67' .specs/STATE.md`

**C51** - The step after a jump is at most a tenth of the last step. (HYG-11, AC 1; T29)
Proof: run abitran | test the_step_after_a_jump_is_capped_at_a_tenth_of_the_last ... ok

**C52** - The switch response meets `TRAN_VOLTAGE` at the harness's 10 ns cap. (HYG-11, AC 2; T29)
Proof: run boundary | test a_carrier_change_is_a_time_point_and_starts_the_response ... ok

**C53** - A host `set` keeps its restart of 10⁻³ of the last step. (HYG-11, AC 3; T29)
Proof: run hostset | test a_scheduled_host_write_lands_and_restarts_small ... ok

**C54** - The engine starts at a given tick. (HYG-12, AC 2; T30)
Proof: run instant | test start_at_commits_init_at_its_tick ... ok

**C55** - A late start solves the operating point at its start and records it there. (HYG-12, AC 1; T31)
Proof: run reentry | test a_late_start_solves_the_operating_point_at_its_start ... ok

**C56** - A late start runs no instant before it, and its timer commits at start plus delay. (HYG-12, AC 2; T31)
Proof: run reentry | test a_late_start_runs_no_instant_before_it ... ok

**C57** - A start that is not finite or negative fails before the operating point. (HYG-12, AC 3; T31)
Proof: run reentry | test a_start_that_is_not_finite_fails_before_the_operating_point ... ok

**C58** - The dead Newton hooks are gone and cannot return. (HYG-15, AC 1; T32)
Proof: run removed | test the_dead_newton_hooks_are_gone ... ok

**C59** - No solver source mentions `PHDL` or `@rfport`. (HYG-15, AC 2; T33)
Proof: run removed | test the_phdl_port_text_is_gone ... ok

**C60** - Every hand-written device is in a partials check of its own module. (HYG-16, AC 3; T34)
Proof: run abidevices | test every_hand_written_device_is_in_a_partials_check ... ok

**C61** - A check of another module does not count. (HYG-16, AC 3; T34)
Proof: run abidevices | test a_check_of_another_module_does_not_count ... ok

**C62** - A rewind equals the checkpoint taken at the mark. (HYG-14, AC 4; T37)
Proof: run digital | test a_rewind_equals_the_checkpoint_taken_at_the_mark ... ok

**C63** - Nested marks rewind in order. (HYG-14, AC 4; T37)
Proof: run digital | test nested_marks_rewind_in_order ... ok

**C64** - A mark journals only the nets that changed. (HYG-14, AC 5; T37)
Proof: run digital | test a_mark_journals_only_the_nets_that_changed ... ok

**C65** - The commit that changes the truncation basis carries the deviation table before and after. (HYG-07, AC 6; T22)
Proof: `git log --format=%B --grep='the truncation test measures the ddt states' | grep -c 'tran_rc' | grep -qE '^[2-9]'`

**C66** - A re-entry with another stop lands on the old stop (§16.4). (HYG-04, edge case 3; T7)
Proof: run reentry | test a_reentry_with_another_stop_lands_on_the_old_stop ... ok

**C67** - A middle step of a `LastStep` run cannot be re-entered. (HYG-13, edge case 4; T8)
Proof: run reentry | test a_middle_step_under_last_step_cannot_be_reentered ... ok

**C68** - An instance is named once in a singular report. (HYG-01, edge case 2; T12)
Proof: run failure | test an_instance_is_named_once_in_a_singular_report ... ok

**C69** - The floor reached at the stop time names that time. (HYG-09, edge case 6; T24)
Proof: run truncation | test the_floor_at_the_stop_time_names_that_time ... ok

**C70** - Two jumps within one restart step restart from the later. (HYG-11, edge case 5; T29)
Proof: run abitran | test two_jumps_in_one_restart_step_restart_from_the_later ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** (T1) `Deviation::of(fixture, quantity, contract, rows)` takes fixture and quantity too, since the bare `of(contract, rows)` cannot fill them. (T2) Q3 risk probe, `dt_min_floor_hits` at HEAD of T2: buck_boundary AdcBench 0 (140 accepted, 0 rejected), ComparatorBench 0 (897/0), HeldHighBench 0 (200/0); buck_time TimerBench/StampBench/EdgeBench 0 (no analog unknowns, 0 steps); buck_whole Buck 0 (8000/0). All zero: Q3's retry-at-floor changes none of them. (T3) Root cause: `Assembly::push_stage` stored the limit linearization `q(x̂) + g·(x − x̂)` as the DDT history row; at the transient's equilibrium rows the limit holds `x̂ = vcrit` (0.7303 V, the seed of the static age-1 evaluation) against x = 0.6294 V, and the tangent of the exponential charge at 0.73 evaluated at 0.63 is negative (−5.3e-8 vs +1.0e-9 expected). Fix: `push_stage` evaluates with every limit at `x̂ = x` (`Need { small_signal: true }`). It lay in `piperine-analog-solver`, not lane A's crates. The ChargeDiode fixture's limit returns the seed itself at age 0 / static age 1 (ngspice INITJCT), as the MSL `limit` lowering does.
**Dispatched:** Batch B1 (T1-T8) - Sonnet worker, 2026-11-17.
**Walked:** none (no screen)

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9

Phase 1:  T1 → T2 → T3 → T4
Phase 2:  T5 → T6 → T7 → T8
Phase 3:  T9 → T10 → T11 → T12 → T13 → T14 → T15 → T16 → T17
Phase 4:  T18 → T19
Phase 5:  T20 → T21 → T22 → T23 → T24
Phase 6:  T25 → T26 → T27 → T28 → T29
Phase 7:  T30 → T31 → T32 → T33 → T34
Phase 8:  T35 → T36 → T37 → T38
Phase 9:  T39 → T40
```

Batches (~7 tasks, whole phases, every worker Sonnet): **B1** Phases 1-2 (T1-T8, 8 tasks), **B2**
Phase 3 (T9-T17, 9), **B3** Phases 4-5 (T18-T24, 7), **B4** Phase 6 (T25-T29, 5), **B5** Phase 7
(T30-T34, 5), **B6** Phases 8-9 (T35-T40, 6). B1, B2 and B5's T32-T34 do not wait for the
answers to Q1 to Q6; B3, B4 and T30-T31 do.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1, T2 | one module, one target | ✅ Granular |
| T3 | one defect, one file (bisect first) | ✅ Granular |
| T4 | one twin and its test function | ✅ Granular (fixtures travel with their test) |
| T5-T8 | one capability each | ✅ Granular |
| T9-T17 | one code or one naming layer each | ✅ Granular |
| T18, T19 | one spec file each | ✅ Granular |
| T20-T24 | one estimate, one accessor, one gate, one removal, one floor rule | ✅ Granular |
| T25-T29 | one strategy file, one driver each, one trait, one rule | ✅ Granular (T28 updates implementers in the same commit: one compile unit) |
| T30-T34 | one function or one guard each | ✅ Granular |
| T35-T38 | one journal layer each | ✅ Granular |
| T39, T40 | one record each | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | first of Phase 1 | ✅ Match |
| T2-T4 | previous task | chain | ✅ Match |
| T5 | None | first of Phase 2 | ✅ Match |
| T6-T8 | previous task | chain | ✅ Match |
| T9 | None | first of Phase 3 | ✅ Match |
| T10-T17 | previous task | chain | ✅ Match |
| T18 | None | first of Phase 4 | ✅ Match |
| T19 | T18 | chain | ✅ Match |
| T20 | None | first of Phase 5 | ✅ Match |
| T21-T24 | previous task | chain | ✅ Match |
| T25 | None | first of Phase 6 | ✅ Match |
| T26-T29 | previous task | chain | ✅ Match |
| T30 | None | first of Phase 7 | ✅ Match |
| T31-T34 | previous task | chain | ✅ Match |
| T35 | None | first of Phase 8 | ✅ Match |
| T36-T38 | previous task | chain | ✅ Match |
| T39 | None | first of Phase 9 | ✅ Match |
| T40 | T39 | chain | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1 | parity src | integration (unit inline) | unit | ✅ OK |
| T2, T4, T17 | parity tests | integration | integration | ✅ OK |
| T3, T21 | analog assembly | integration | integration | ✅ OK |
| T5, T9, T10, T11, T14, T20, T23, T25 | analog math and strategies | unit | unit | ✅ OK |
| T13 | analog assembly | integration | integration | ✅ OK |
| T6-T8, T12, T15, T16, T22, T24, T26-T29, T31, T33, T34, T38 | owner | integration | integration | ✅ OK |
| T32 | analog math (removal) | unit | integration (`removed_surface`) | ✅ OK (the guard is the proof of a removal) |
| T30, T35-T37 | digital engine | integration | integration | ✅ OK |
| T18, T19 | spec text | integration | integration | ✅ OK |
| T39, T40 | records | none | none | ✅ OK |
