# Transient Performance Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/transient-performance/design.md`
**Status**: Draft
**Approved**: 2026-11-20 - under the user's standing AFK rule (2026-11-19: "Tu sempre assume o default mais razoável e depois me conta pois vou ficar afk"); the user asked for the feature on 2026-11-20 ("quero uma feature de melhora de performance"). Nothing is dispatched yet.
**Build**: `cargo build -p piperine -p piperine-abi -p piperine-analog-solver -p piperine-digital-solver -p piperine-solver -p piperine-api -p piperine-cli -p piperine-parity --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape`
**Run digital**: `cargo test -p piperine-digital-solver --test advance`
**Run protocol**: `cargo test -p piperine-solver --lib protocol::statistics::`
**Run solver**: `cargo test -p piperine-solver --test step_statistics --test jump_step --test landing_rule`
**Run integration**: `cargo test -p piperine-analog-solver --lib math::integration::`
**Run newton**: `cargo test -p piperine-analog-solver --lib math::newton_raphson::`
**Run alloc**: `cargo test -p piperine-analog-solver --test allocation`
**Run abialloc**: `cargo test -p piperine-abi --test eval_allocation`
**Run apistats**: `cargo test -p piperine-api --test session`
**Run bucklong**: `cargo test -p piperine-api --test buck_long -- --nocapture`
**Run buckacc**: `cargo test -p piperine-api --test buck_accuracy -- --nocapture`
**Run scenarios**: `cargo test -p piperine --test buck_scenarios`
**Run parity**: `cargo test -p piperine-parity`
**Run twins**: `cargo test -p piperine-cli --test twins`
**Run grid**: `cargo test -p piperine-solver --test parity_baseline`
**Run spectext**: `cargo test -p piperine-lang --test spec_text`
**Run docs**: `cargo test -p piperine-cli --test docs`

Executors: every task is Sonnet (user rule); a different model verifies. The worktree is
`/home/keslarek/Git/piperine-transient-performance` on `feature/transient-performance`. Cargo runs with
`CARGO_TARGET_DIR=/var/tmp/piperine-target-a CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-a`,
in the foreground, plain `cargo` at the root (never `--workspace`: `piperine-lang-server` is postponed); stop when
`df /` passes 90 %. Where worktrees share the target directory, `touch` the `src/lib.rs` of each crate the task
edits before a gate. No push.

**Sequencing.** `analog-operators` (T9: the step that reads a jump is first order in both stages; it adds
`protocol/retake.rs`) and `unknown-semantics-events` (`settle` is an MSL event; it edits `instant.rs`) must be
on main first; the worktree is rebased on that main before T1, and T1 checks it. Both edit the files of
Phases 1 to 3, and TP-05's estimate is defined on T9's two backward-Euler stages.

**The constraints of every task** (repeated in each Done when that can break them):

- **Parity is the top priority.** No ngspice parity value, fixture, twin's printed bytes, pinned time grid or
  tolerance moves, and none is edited. A task that would move one stops and reports (spec D5).
- **Tolerances are never retuned**, in the solver, in a test or in `contract.rs`.
- **Mixed-signal goldens** (buck listings, Book buck rows) move only by TP-04 and TP-05, and each moved value is
  listed in `moved.md` (old, new, requirement).
- **Spec first.** T6 and T12 amend `docs/spec` and `abi.md` before the code that follows them.
- **Cleanliness** (the user's rules): no free function; names in full words; one purpose per type and per crate;
  no god method (`microsteps` and `device_attempt` shrink, they do not grow); the module tree says where a thing
  is (`landing.rs`, `protocol/statistics.rs`); the path of the data is evident; every struct, enum and method has
  a doc comment.
- **A gate that is missed is reported with its measured value; its limit is never moved** (300 000 steps,
  1 000 landings, 0.104 mV).

**Gate time.** `Run bucklong` is about 2.5 minutes before Phase 3 and under 1 after; `Run buckacc` runs a
1 000 000-step reference, about 3 minutes; `Run scenarios` is 15 to 25 minutes before Phase 3. Run each once per
task, never in a loop.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md`, `crates/piperine-lang/tests/suite_hygiene.rs`, `crates/piperine-solver/tests/workspace_shape.rs`, `.specs/STATE.md` MD-28 (a target lives in the crate it exercises), MD-33, MD-38 (tolerances only in the contract).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Digital engine (`piperine-digital-solver/src/{landing,plan,instant,advance}.rs`) | unit | one test per criterion of TP-02 and TP-03 and per listed edge case, on hand-built devices, in both rules | `crates/piperine-digital-solver/tests/advance.rs` | `cargo test -p piperine-digital-solver --test advance` |
| Pure types (`protocol/statistics.rs`, `math/integration.rs`, `math/newton_raphson.rs`) | unit | every method; closed-form values for the estimate | inline `#[cfg(test)] mod tests` | `cargo test -p piperine-solver --lib protocol::statistics::`; `cargo test -p piperine-analog-solver --lib math::` |
| Protocol and analog side (`protocol/transient.rs`, `analyses/transient.rs`) | integration | 1:1 to TP-01, TP-04, TP-05, TP-08, TP-09 on solver fixtures; rejection, floor and retake paths | `crates/piperine-solver/tests/{step_statistics,jump_step,landing_rule}.rs` | `cargo test -p piperine-solver --test step_statistics --test jump_step --test landing_rule` |
| Allocation (`piperine-abi/src/caller.rs`, `device/assembly.rs`) | unit / integration | 0 allocations on the second call, counted | `crates/piperine-abi/tests/eval_allocation.rs`, `crates/piperine-analog-solver/tests/allocation.rs` | `cargo test -p piperine-abi --test eval_allocation`; `cargo test -p piperine-analog-solver --test allocation` |
| Host surface (`piperine-api/src/session.rs`) | integration | the accessor before and after a run; no run key | `crates/piperine-api/tests/{session,options}.rs` | `cargo test -p piperine-api --test session` |
| The buck (`examples/buck`, unchanged) | integration | the step, landing and accuracy gates; SPEC limits unedited | `crates/piperine-api/tests/{buck_long,buck_accuracy}.rs`, `tests/buck_scenarios.rs` | `cargo test -p piperine-api --test buck_long`; `--test buck_accuracy`; `cargo test -p piperine --test buck_scenarios` |
| Parity (unchanged, never edited) | integration | every target passes as it is | `crates/piperine-parity/tests/*.rs`, `crates/piperine-cli/tests/twins.rs`, `crates/piperine-solver/tests/parity_baseline.rs` | `cargo test -p piperine-parity`; `cargo test -p piperine-cli --test twins`; `cargo test -p piperine-solver --test parity_baseline` |
| Spec and records (`docs/spec/*.md`, `abi.md`, `GAPS.md`, `ROADMAP.md`, `diagnosis.md`, `moved.md`) | integration | each amended sentence and each record claim read as text | `crates/piperine-lang/tests/spec_text.rs`, `crates/piperine-cli/tests/docs.rs` | `cargo test -p piperine-lang --test spec_text`; `cargo test -p piperine-cli --test docs` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | Tasks with unit tests only (T1, T2, T7, T13 to T15, T18, T19) | the task's `Run` line named in its Done when |
| Full | Tasks with integration tests (T3 to T5, T8 to T11, T16, T17, T20 to T22) | the task's `Run` lines, then the parity guard (`Run parity`, `Run twins`, `Run grid`) where the task names it |
| Build | Document tasks (T6, T12, T23, T24) and every phase end | the `**Build**:` line, then `Run spectext` and `Run docs` |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: The statistics (batch B1)

```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: Every step is tested (batch B2)

```
T5 → T6 → T7 → T8 → T9 → T10 → T11
```

### Phase 3: Observable landings (batch B3)

```
T11 → T12 → T13 → T14 → T15 → T16 → T17
```

### Phase 4: A cheaper step (batch B4)

```
T17 → T18 → T19 → T20 → T21 → T22
```

### Phase 5: Close (batch B4)

```
T22 → T23 → T24
```

Batches (about 7 tasks, whole phases, one Sonnet worker each, sequential): **B1** Phase 1 (T1 to T5); **B2** Phase 2 (T6 to T11); **B3** Phase 3 (T12 to T17); **B4** Phases 4 and 5 (T18 to T24).

Why this order: the statistics come first because every gate is stated in them. The tested steps come before the observable landings, because removing the landings while the two exemptions stand moves `buck.vout` by 1 to 5 mV (`diagnosis.md` §4 rows 3, 4, 14); with the tests in place first, the landings are removed without leaving the tree wrong at any commit.

---

## Task Breakdown

### T1: `LandingCause` and `TimePointRule`, and the cause on a landing

**What**: New module `landing.rs` with `LandingCause` (`AnalogReadChanged`, `ContinuousRead`, `DeclaredReach`, `Irreversible`, `HostWaiter`), `LandingCause::is_time_point` and `TimePointRule` (`Observable`, `Declared`; this task wires `Declared` only). `Stepped::Landing` and `Advance::Landing` carry `cause` in place of `time_point`; the adapter in `piperine-solver` maps it. Behaviour is unchanged.
**Where**: `crates/piperine-digital-solver/src/landing.rs`
**Depends on**: None
**Reuses**: `instant.rs` lines 163 to 187; `advance.rs`
**Requirement**: TP-01

**Done when**:

- [ ] the worktree is rebased on a main that holds `analog-operators` and `unknown-semantics-events` (`git log origin/main` shows both merges); if either is missing, STOP and report
- [ ] one test per cause in `tests/advance.rs`: a landing by a changed analog-read net, by a reaction that reads a continuous pin, by the reach flag only, by an `IRREVERSIBLE` reaction, by a host waiter, each asserting its `LandingCause`
- [ ] `is_time_point` is false for `Irreversible` and `HostWaiter` only
- [ ] Gate check passes: `Run digital`; every existing test of `piperine-digital-solver` and `piperine-solver` passes unedited except for the renamed field
- [ ] Test count: 6 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `feat(digital-solver): a landing carries its cause`

---

### T2: `StepStatistics` and `StepEnd`

**What**: New module `protocol/statistics.rs`: `StepEnd` (`Landing(LandingCause)`, `Crossing`, `MaxStep`, `Cap`, `Stop`, `Proposal`), the protocol's own `LandingCause` in `sides.rs`, and `StepStatistics` with `record_end`, `record_instant`, `record_retake`, `accepted`, `ended`, `landings`, `instants_run`, `sampling_share`.
**Where**: `crates/piperine-solver/src/protocol/statistics.rs`
**Depends on**: T1
**Reuses**: `protocol/sides.rs` (plain data at the seam)
**Requirement**: TP-01

**Done when**:

- [ ] `accepted()` equals the sum of `ended(e)` over every `StepEnd` (TP-01 AC 3), asserted on a hand-filled value
- [ ] `sampling_share()` is `landings(ContinuousRead) / accepted()` and 0.0 for an empty value
- [ ] Gate check passes: `Run protocol`
- [ ] Test count: 4 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `feat(solver): StepStatistics counts how each accepted step ended`

---

### T3: The protocol fills `StepStatistics`

**What**: `Protocol` owns a `StepStatistics`; `transient_loop` records one `StepEnd` per accepted step (from the `Advance`, the `Attempt` and the new `AnalogSide::proposal_bound() -> ProposalBound`), each instant run and each crossing retake; `Protocol::statistics()` returns it.
**Where**: `crates/piperine-solver/src/protocol/transient.rs`
**Depends on**: T2
**Reuses**: `tests/protocol_transient.rs`'s hand-built sides
**Requirement**: TP-01

**Done when**:

- [ ] `tests/step_statistics.rs` (new target, with a `//!` header): a hand-built pair of sides with one landing of each cause, one located crossing, one step at `max_step`, one capped step and the stop; each count asserted, and the sum equal to the accepted steps
- [ ] a rejected attempt and a retaken step are counted once each and add no `StepEnd`
- [ ] Gate check passes: `Run solver`
- [ ] Test count: 3 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): the protocol records why each step ended`

---

### T4: The analog side's counts

**What**: `SolverStats` gains `rejected_truncation`, `rejected_newton` (their sum stays `steps_rejected`), `jump_steps`, `jump_retakes`, `steps_at_max_step`; `device_attempt`, `cut_after_failure` and `device_accept` fill them; `TransientAnalysisResult::statistics()` returns the protocol's `StepStatistics`.
**Where**: `crates/piperine-solver/src/core/result.rs`
**Depends on**: T3
**Reuses**: `TimeLoop`'s existing counters
**Requirement**: TP-01

**Done when**:

- [ ] `tests/step_statistics.rs`: a real RC behind a digital switch reports 1 jump step per delivery; a step forced over the tolerance reports `rejected_truncation` 1 and `rejected_newton` 0
- [ ] `steps_rejected == rejected_truncation + rejected_newton` on every fixture of the target
- [ ] Gate check passes: `Run solver`
- [ ] Test count: 3 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): rejections by cause and jump steps in SolverStats`

---

### T5: `Session::statistics` and the buck's baseline counts

**What**: `piperine-api` returns the last transient's statistics from the session (a `TransientStatistics` value: the protocol's counts and the analog side's); `buck_long` prints every count and the wall time on standard error. No run key, no flag (D8).
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T4
**Reuses**: `crates/piperine-api/tests/buck_long.rs::report`
**Requirement**: TP-01, TP-06

**Done when**:

- [ ] `tests/session.rs`: after a transient of `tran_rc` the session returns statistics whose accepted count equals the table's step count without `print_step`; before any run it returns `None`
- [ ] `buck_long` prints one line per count of TP-01 AC 1 and AC 2 and the wall time, asserting no time (TP-06 AC 4)
- [ ] the printed baseline (accepted steps, landings by cause, jump steps) is copied into `diagnosis.md` under a new heading `## 8. Baseline of the 600 µs start`
- [ ] Gate check passes: `Run apistats`, then `Run bucklong`
- [ ] Test count: 2 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `feat(api): a session returns its transient statistics`

---

### T6: Spec: the landing step is tested, the jump step is error-controlled

**What**: Amend `docs/spec/part_v_execution.md` §15.2 (rule 6; the "Before the solve of `t`" bullet; §15 S3) and Appendix C.2 ("the jump", "truncation error", "restart" rows; the constants row), Appendix D.4 and Appendix F "Restart" rows, exactly as design "Spec amendments" gives them. Pin each amended sentence in `spec_text.rs`.
**Where**: `docs/spec/part_v_execution.md`
**Depends on**: T5
**Reuses**: design.md "Spec amendments"
**Requirement**: TP-04, TP-05

**Done when**:

- [ ] §15.2 rule 6 states the first-order estimate and the retake; no sentence of §17.2 rule 2 changes (`git diff` shows none)
- [ ] `spec_text.rs` has one test per amended place (4 tests) and each fails on the old sentence
- [ ] Gate check passes: `Run spectext`, then `Run docs`
- [ ] Test count: 4 new tests

**Tests**: integration
**Gate**: build

**Commit**: `docs(spec): the landing step is tested and the jump step is error-controlled`

---

### T7: `TrBdf2::jump_lte`

**What**: The first-order estimate of design "The estimate": for each `DdtState`, `h·|s_1 − s_γ| / (2(1 − γ))` over the bound `state_lte` uses, the largest over the states.
**Where**: `crates/piperine-analog-solver/src/math/integration.rs`
**Depends on**: T6
**Reuses**: `TrBdf2::state_lte` and its bound
**Requirement**: TP-05

**Done when**:

- [ ] unit tests: a state linear in time gives 0; a state `q = t²` over `h` gives `h²` within 1 % before normalization (the closed form of backward Euler's error, `(h²/2)·q''`); an empty state list gives 0; the normalization equals `state_lte`'s on the same state
- [ ] Gate check passes: `Run integration`
- [ ] Test count: 4 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `feat(analog-solver): the first-order error estimate of a jump step`

---

### T8: `StepKind` replaces the two booleans

**What**: `StepPrediction` carries `kind: StepKind` (`Ordinary`, `ReadsJump`) and `on_tick: bool`; `assess_step` reads the kind. Behaviour is unchanged in this task: a step that ends on a landing keeps its exemption through a named, temporary branch that T9 deletes. `AnalogSide::attempt`'s second parameter is renamed `landing`.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T7
**Reuses**: `assess_step`, `device_attempt`, `PendingStep`
**Requirement**: TP-04

**Done when**:

- [ ] every existing test of `piperine-solver` passes unedited except for the renamed parameter
- [ ] the parity guard passes with nothing edited: `Run parity`, `Run twins`, `Run grid`; if a parity value, a twin's bytes or a pinned grid would move, STOP and report (D5) - no parity file, fixture or tolerance is edited
- [ ] Gate check passes: `Run solver`, then the parity guard
- [ ] Test count: 0 new tests (a refactor; the existing 3 targets `truncation`, `protocol_transient`, `parity_baseline` are its tests)
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `refactor(solver): a step is Ordinary or ReadsJump`

---

### T9: The step that ends on a landing is tested

**What**: Delete the exemption of T8's temporary branch: an `Ordinary` step is tested with `state_lte` whether or not it ends on a landing; a rejection goes back through the protocol's restore.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T8
**Reuses**: `cut_after_failure`, the protocol's `Rejected` arm
**Requirement**: TP-04

**Done when**:

- [ ] `tests/jump_step.rs` (new target, `//!` header): an RC driven by a ramp with a digital landing (a reaction that reads a continuous pin) in the middle of a step that fails the tolerance is rejected and retaken smaller (TP-04 AC 1, AC 2); the instants run ahead of the rejected step leave no record (TP-09 AC 2)
- [ ] the parity guard passes with nothing edited: `Run parity`, `Run twins`, `Run grid`; if a parity value, a twin's bytes or a pinned grid would move, STOP and report (D5) - no parity file, fixture or tolerance is edited
- [ ] a mixed-signal golden that moves (buck listings, Book buck rows, `tests/book/mixed.rs`) is updated and listed in `.specs/features/transient-performance/moved.md` with old value, new value and the requirement; no record and no digital column is in the list; no tolerance is retuned
- [ ] Gate check passes: `Run solver`, the parity guard, then `cargo test -p piperine --test book_mixed` and `Run bucklong`
- [ ] Test count: 2 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): a step that ends on a landing is tested for truncation error`

---

### T10: The step that reads a jump is tested with `jump_lte`

**What**: A `ReadsJump` step is assessed with `TrBdf2::jump_lte` against `trtol`; above the floor a failing step is rejected and retaken at the controller's `reject_dt`, still `ReadsJump`; at the floor it is accepted and counted. Its first size stays §15.2 rule 6's.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T9
**Reuses**: `is_at_floor`, `floor_accepts`, `DeviceRun::delivered`
**Requirement**: TP-05

**Done when**:

- [ ] `tests/jump_step.rs`: an ideal 1 V step into 1 kΩ and 1 nF reads `1 − e⁻¹` V within `reltol` one time constant later (TP-05 AC 4); a jump step over the tolerance is retaken and `jump_retakes` is 1 (AC 2); at `dt_min` it is accepted and `floor_accepts` is 1 (AC 3); a jump step within the tolerance is not retaken (`jump_retakes == 0`) and the pinned step count of `parity_baseline` is unmoved
- [ ] the worker records in `tasks.md` Handoff whether `DdtState::q[0]` of a jump step is the state with the delivered values (design "The estimate")
- [ ] the parity guard passes with nothing edited: `Run parity`, `Run twins`, `Run grid`; if a parity value, a twin's bytes or a pinned grid would move, STOP and report (D5) - no parity file, fixture or tolerance is edited
- [ ] a mixed-signal golden that moves (buck listings, Book buck rows, `tests/book/mixed.rs`) is updated and listed in `.specs/features/transient-performance/moved.md` with old value, new value and the requirement; no record and no digital column is in the list; no tolerance is retuned
- [ ] Gate check passes: `Run solver`, the parity guard, `cargo test -p piperine --test book_mixed`, `Run bucklong`
- [ ] Test count: 4 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): the step that reads a jump is error-controlled`

---

### T11: The buck's accuracy gate

**What**: New target `buck_accuracy.rs`: the first 100 µs of `startup` at `max_step = 1e-8` and at `max_step = 1e-10`, `buck.vout` compared on the 0.5 µs grid; the largest difference at most 0.104 mV; the digital columns equal row for row. It prints the difference and both step counts.
**Where**: `crates/piperine-api/tests/buck_accuracy.rs`
**Depends on**: T10
**Reuses**: `buck_long.rs` (`start`, `column`, `series`)
**Requirement**: TP-05

**Done when**:

- [ ] the target opens with a `//!` header naming TP-05 AC 5 and the origin of 0.104 mV (`diagnosis.md` §4 row 0)
- [ ] `the_first_100_us_match_the_fine_reference_within_0_104_mv` passes and prints the measured difference
- [ ] if the difference exceeds 0.104 mV, STOP and report: the bound is never moved
- [ ] Gate check passes: `Run buckacc`
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test(api): the buck's first 100 us against a fine reference`

---

### T12: Spec: the time points as implemented

**What**: Amend Appendix C.2's "time points (§17.2)" row and add the paragraph "R4 as implemented" to `abi.md` §5, as design "Spec amendments" gives them; pin both in `spec_text.rs`.
**Where**: `docs/spec/appendix_c_implementation.md`
**Depends on**: T11
**Reuses**: design.md "Spec amendments"
**Requirement**: TP-10

**Done when**:

- [ ] the row names the dynamic test, the three exceptions (`settle` reaction, host waiter, host probe) and the reference mode (TP-10 AC 2)
- [ ] §17.2 rule 2 is unchanged
- [ ] Gate check passes: `Run spectext`, then `Run docs`
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: build

**Commit**: `docs(spec): the time points the solver lands on`

---

### T13: `Analysis::samples`

**What**: The plan keeps, per reaction, whether it reads a continuous pin itself, beside `probes`.
**Where**: `crates/piperine-digital-solver/src/plan.rs`
**Depends on**: T12
**Reuses**: the `samples` local of `reachability`
**Requirement**: TP-03

**Done when**:

- [ ] tests in `tests/advance.rs`: a reaction that reads a continuous pin has `samples` and `probes`; a reaction that only writes a reaching net has `probes` and not `samples`; a reaction that reaches nothing has neither
- [ ] Gate check passes: `Run digital`
- [ ] Test count: 3 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `feat(digital-solver): the plan knows which reactions read a continuous pin`

---

### T14: `LandingTest`

**What**: The landing test of the look-ahead moves out of `Digital::microsteps` into `LandingTest::after_functions`; `Declared` behaviour only, identical to today's. `microsteps` calls it once.
**Where**: `crates/piperine-digital-solver/src/landing.rs`
**Depends on**: T13
**Reuses**: `instant.rs`'s `if lookahead` block
**Requirement**: TP-03

**Done when**:

- [ ] `microsteps` holds no landing condition of its own (the block is one call)
- [ ] every test of `piperine-digital-solver` and `piperine-solver` passes unedited
- [ ] Gate check passes: `Run digital`, then `Run solver`
- [ ] Test count: 2 new unit tests of `LandingTest` on a hand-built store
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `refactor(digital-solver): LandingTest decides a landing`

---

### T15: `TimePointRule::Observable`

**What**: `LandingTest` under `Observable`: a landing only when an analog-read net changed (`AnalogReadChanged`) or a firing reaction has `samples` (`ContinuousRead`), plus `Irreversible` and `HostWaiter` as today. The effective rule is `Declared` when a reaction listens to a `settle` net or a host waiter is registered. `Digital::new` takes the rule; the default of `DigitalLimits`' sibling option is `Observable`.
**Where**: `crates/piperine-digital-solver/src/landing.rs`
**Depends on**: T14
**Reuses**: `Analysis::settle_nets`, `Digital::waited`
**Requirement**: TP-02, TP-03

**Done when**:

- [ ] `tests/advance.rs`, one test per criterion: TP-02 AC 1 to AC 4 in both rules; TP-03 AC 1 (no landing), AC 2 (a `settle` listener keeps every declared landing), AC 3 (a host waiter keeps them), AC 4 (a change at microstep 1 rewinds and lands, the undone run leaves no observation), AC 5 (a commit with a param-expression delay of one tick or more makes no landing at its making)
- [ ] the edge cases: two commits on one tick land once; an instant due at the stop; a `FINISH` at an instant run ahead; an `IRREVERSIBLE` reaction in both rules
- [ ] Gate check passes: `Run digital`
- [ ] Test count: 13 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `feat(digital-solver): land only where something observable happens`

---

### T16: The rule reaches the solver, and the open-loop fixture

**What**: `ProtocolOptions` and the solver's configuration carry `TimePointRule` to `Digital::new` through the adapter; `piperine-api` exposes it as a session option with no run key. New target `landing_rule.rs` with the open-loop fixture of TP-09 AC 1.
**Where**: `crates/piperine-solver/tests/landing_rule.rs`
**Depends on**: T15
**Reuses**: `analyses/adapters.rs`, `tests/mixed_signal.rs`'s device builders
**Requirement**: TP-06, TP-09

**Done when**:

- [ ] the fixture (200 MHz oscillator, a counter that commits every edge, a `D2A` whose input changes once per 400 edges, into an RC, no probing reaction) gives the same records in both rules, field for field (TP-09 AC 1), and instants in increasing `(t, n)` in both (AC 3)
- [ ] under `Declared` the fixture's landing count equals its clock edges; under `Observable` it equals the `D2A` changes (TP-06 AC 3)
- [ ] a `cross` is located in both rules and a `bound_step` cap applies in both (edge cases)
- [ ] `piperine-api` accepts the option and `crates/piperine-api/tests/options.rs` asserts no run key or CLI flag names it
- [ ] the parity guard passes with nothing edited: `Run parity`, `Run twins`, `Run grid`; if a parity value, a twin's bytes or a pinned grid would move, STOP and report (D5) - no parity file, fixture or tolerance is edited
- [ ] Gate check passes: `Run solver`, `cargo test -p piperine-api --test options`, the parity guard
- [ ] Test count: 6 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): the time-point rule is a solver option`

---

### T17: The buck's step and landing gates

**What**: `buck_long` asserts at most 300 000 accepted steps for the 600 µs start (TP-06 AC 1); `buck_accuracy` asserts at most 1 000 landings in the first 100 µs (TP-03 AC 6). The SPEC limits of `buck_long` stay as they are.
**Where**: `crates/piperine-api/tests/buck_long.rs`
**Depends on**: T16
**Reuses**: T5's printed counts
**Requirement**: TP-03, TP-06

**Done when**:

- [ ] `the_whole_start_takes_at_most_300000_steps` and `the_first_100_us_land_at_most_1000_times` pass and print the measured counts
- [ ] every existing test of `buck_long` passes with its limits unedited (TP-06 AC 2)
- [ ] if a gate is missed, STOP and report the measured count: the limit is never moved
- [ ] a mixed-signal golden that moves (buck listings, Book buck rows, `tests/book/mixed.rs`) is updated and listed in `.specs/features/transient-performance/moved.md` with old value, new value and the requirement; no record and no digital column is in the list; no tolerance is retuned
- [ ] Gate check passes: `Run bucklong`, then `Run buckacc`
- [ ] Test count: 2 new tests

**Tests**: integration
**Gate**: full

**Commit**: `test(api): the buck's start within 300 000 steps and 1 000 landings per 100 us`

---

### T18: `SolveTimer`

**What**: The Newton solver reads the clock around assembly and the linear solve only when `TraceFlags::timing` is set (`PIPERINE_TRACE_TIMING`, read by `TraceFlags::from_env`); otherwise both totals stay 0.
**Where**: `crates/piperine-analog-solver/src/math/newton_raphson.rs`
**Depends on**: T17
**Reuses**: `assembly_ns`, `solve_ns`, `TraceFlags`
**Requirement**: TP-07

**Done when**:

- [ ] unit tests: with timing off a solve leaves `assembly_time_ns()` and `solve_time_ns()` at 0 (TP-07 AC 3); with it on both are greater than 0 (AC 4)
- [ ] Gate check passes: `Run newton`
- [ ] Test count: 2 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `perf(analog-solver): Newton reads the clock only when timing is asked for`

---

### T19: `FunctionFrame`: cell lengths computed once

**What**: `Handle` computes each function's read and write cell lengths at construction (`FunctionFrame`); `Handle::eval` checks the buffers against them and takes its two pointer lists from an `EvalBuffers` the caller owns. The errors keep their text.
**Where**: `crates/piperine-abi/src/caller.rs`
**Depends on**: T18
**Reuses**: `device::cell_lens`, `check_cells`
**Requirement**: TP-07

**Done when**:

- [ ] `tests/eval_allocation.rs` (new target, `//!` header, a counting allocator local to the target): the second `eval` of a function allocates 0 times (TP-07 AC 1)
- [ ] a buffer shorter than its cell is refused with today's message, asserted as text; every test of `tests/caller.rs` passes unedited except for the new argument
- [ ] the generated side (`thunk.rs`, `device.rs`) is not edited; `cargo test -p piperine-transpile` passes
- [ ] Gate check passes: `Run abialloc`, then `cargo test -p piperine-abi`
- [ ] Test count: 2 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: unit
**Gate**: quick

**Commit**: `perf(abi): a Handle validates a function's cells once`

---

### T20: `Assembly::run` reuses its buffers

**What**: The per-call `Vec<&[u8]>` and `Vec<&mut [u8]>` of `run` are replaced by offsets into `scratch` and the `EvalBuffers` the `Assembly` owns.
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs`
**Depends on**: T19
**Reuses**: `Assembly::scratch`
**Requirement**: TP-07

**Done when**:

- [ ] `tests/allocation.rs` (new target, `//!` header): the second `Assembly::evaluate` at a changed iterate allocates 0 times
- [ ] every `assembly_*` target passes unedited
- [ ] Gate check passes: `Run alloc`, then `cargo test -p piperine-analog-solver`
- [ ] Test count: 1 new test
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `perf(analog-solver): a function call reuses the assembly's buffers`

---

### T21: The stamps and the node list are reused

**What**: `Assembly::assemble_tran_into` and `NonLinearSystem::assemble_into` fill a buffer the Newton solver owns; the `nodes` list is built once in `NewtonRaphsonSolver::new`; `TransientSystem::assemble_device` stops copying `x`.
**Where**: `crates/piperine-analog-solver/src/math/newton_raphson.rs`
**Depends on**: T20
**Reuses**: `assemble_tran`, `compute_residual`
**Requirement**: TP-07

**Done when**:

- [ ] `tests/allocation.rs`: the second `assemble_tran_into` for the same stage shape allocates 0 times (TP-07 AC 2)
- [ ] `assemble_tran` and `assemble` stay and return what they returned (their tests pass unedited)
- [ ] the parity guard passes with nothing edited: `Run parity`, `Run twins`, `Run grid`; if a parity value, a twin's bytes or a pinned grid would move, STOP and report (D5) - no parity file, fixture or tolerance is edited
- [ ] Gate check passes: `Run alloc`, `Run solver`, the parity guard
- [ ] Test count: 1 new test
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `perf(analog-solver): Newton reuses its stamp buffer`

---

### T22: No instant at an idle step

**What**: `DiscreteSide::is_idle_at` (true when no commit is due at the tick, no instant is suspended, no function is dirty and no reaction listens to `settle`); the protocol runs no instant at an accepted step when it is true and no crossing fired.
**Where**: `crates/piperine-solver/src/protocol/transient.rs`
**Depends on**: T21
**Reuses**: `Digital::next_due`, the dirty set, `Analysis::settle_nets`
**Requirement**: TP-08

**Done when**:

- [ ] `tests/step_statistics.rs`: a pure analog RC of 100 accepted steps runs 0 instants; with a reaction on `settle` it runs 100; with one commit due it runs 1 (TP-08 AC 1)
- [ ] `buck_accuracy` asserts `instants_run` at most scheduled instants + crossings + landings for the first 100 µs (TP-08 AC 2)
- [ ] the open-loop fixture's records are still equal in both rules (`landing_rule`)
- [ ] the parity guard passes with nothing edited: `Run parity`, `Run twins`, `Run grid`; if a parity value, a twin's bytes or a pinned grid would move, STOP and report (D5) - no parity file, fixture or tolerance is edited
- [ ] a mixed-signal golden that moves (buck listings, Book buck rows, `tests/book/mixed.rs`) is updated and listed in `.specs/features/transient-performance/moved.md` with old value, new value and the requirement; no record and no digital column is in the list; no tolerance is retuned
- [ ] Gate check passes: `Run solver`, `Run buckacc`, the parity guard
- [ ] Test count: 4 new tests
- [ ] every new struct, enum and method has a doc comment; no free function is added; no abbreviation in a new name

**Tests**: integration
**Gate**: full

**Commit**: `perf(solver): an accepted step with nothing due runs no instant`

---

### T23: The whole regression and the scenario package

**What**: Run the five scenario families, the parity targets, the twins and the root suite on the finished tree; update the comment of `tests/e2e/buck/Piperine.toml` to the measured step count of 5.2 ms (`max_steps` and every run key unchanged, D9).
**Where**: `tests/e2e/buck/Piperine.toml`
**Depends on**: T22
**Reuses**: `tests/buck_scenarios.rs`
**Requirement**: TP-06, TP-09, TP-10

**Done when**:

- [ ] `Run scenarios` passes with `tests/buck_scenarios.rs` unedited (TP-06 AC 5)
- [ ] `git diff --quiet origin/main -- crates/piperine-parity/tests crates/piperine-parity/src/contract.rs crates/piperine-cli/tests/twins.rs crates/piperine-solver/tests/parity_baseline.rs` exits 0 (TP-09 AC 4)
- [ ] with `ngspice` taken off `PATH` the parity targets print the harness's skip line and the twins still run (TP-09 AC 5)
- [ ] `moved.md` exists and lists every moved golden, or says "none" (TP-09 AC 6); `crates/piperine-cli/tests/docs.rs` gains a test that the comment states a step count and that `moved.md` has its three columns
- [ ] Gate check passes: `Run scenarios`, the parity guard, `Run docs`, then `cargo test` at the root
- [ ] Test count: 1 new test

**Tests**: integration
**Gate**: build

**Commit**: `test(buck): the scenario package states its measured step count`

---

### T24: The records

**What**: `GAPS.md` loses V12; `ROADMAP.md` marks `multi-rate` delivered by `transient-performance` with its commit; `.specs/features/multi-rate/spec.md` gains one line under its title pointing here; `diagnosis.md` gains `## 9. After` with the counts of its section 2 and the wall time `buck_long` printed; `.specs/STATE.md` gains the next `MD-NN` (the solver lands by the observable rule; the two exemptions are gone).
**Where**: `GAPS.md`
**Depends on**: T23
**Reuses**: `crates/piperine-cli/tests/docs.rs`
**Requirement**: TP-10

**Done when**:

- [ ] `docs.rs` tests: `GAPS.md` names no V12 row; `ROADMAP.md`'s `multi-rate` row names `transient-performance` (TP-10 AC 1); `diagnosis.md` has the heading `## 9. After` with the six counts (TP-10 AC 3)
- [ ] Gate check passes: `Run docs`, then the `**Build**:` line
- [ ] Test count: 3 new tests

**Tests**: integration
**Gate**: build

**Commit**: `docs: transient-performance closes V12`

---

## Checks

**C1** - Each landing cause is counted: analog-read change, continuous read, declared reach only, irreversible or host waiter, finish (TP-01, AC 1; T1, T3)
Proof: run solver | test the_protocol_counts_one_landing_of_each_cause ... ok

**C2** - Accepted steps, rejections by cause, crossing retakes, jump steps, steps at max_step and instants run are reported (TP-01, AC 2; T4)
Proof: run solver | test the_analog_side_reports_rejections_by_cause_and_jump_steps ... ok

**C3** - Every accepted step is counted under exactly one end (TP-01, AC 3; T2)
Proof: run protocol | test the_ends_sum_to_the_accepted_steps ... ok

**C4** - A session returns the counts and the sampling share is computable (TP-01, AC 4; T5)
Proof: run apistats | test a_session_returns_the_statistics_of_its_last_transient ... ok

**C5** - `buck_long` prints every count on standard error (TP-01, AC 5; T5)
Proof: run bucklong | test the_statistics_of_the_whole_start_are_reported ... ok

**C6** - A falling `clk` heard only by `rise()` makes no landing, in both rules (TP-02, AC 1; T15)
Proof: run digital | test a_falling_clock_heard_by_rise_is_no_landing_in_either_rule ... ok

**C7** - A commit due at `(t, 0)` that changes an analog-read carrier lands on `t` (TP-02, AC 2; T15)
Proof: run digital | test a_due_change_of_an_analog_read_net_lands_in_either_rule ... ok

**C8** - A firing reaction that reads a continuous pin lands on `t` (TP-02, AC 3; T15)
Proof: run digital | test a_reaction_that_reads_a_continuous_pin_lands_in_either_rule ... ok

**C9** - A net nothing reaching the analog side reads makes no landing (TP-02, AC 4; T15)
Proof: run digital | test a_change_nothing_analog_reaches_is_no_landing_in_either_rule ... ok

**C10** - An instant that changes no analog-read carrier and fires no sampling reaction is not landed on (TP-03, AC 1; T15)
Proof: run digital | test an_instant_nothing_observes_is_not_landed_on ... ok

**C11** - A design with a reaction on `settle` keeps every declared landing (TP-03, AC 2; T15)
Proof: run digital | test a_settle_listener_keeps_every_declared_landing ... ok

**C12** - A host waiter keeps the declared landings (TP-03, AC 3; T15)
Proof: run digital | test a_host_waiter_keeps_every_declared_landing ... ok

**C13** - A change at a later microstep rewinds the instant and lands on it, leaving no record (TP-03, AC 4; T15)
Proof: run digital | test a_change_at_a_later_microstep_rewinds_and_lands ... ok

**C14** - A commit made with a param-expression delay of a tick or more makes no landing at its making (TP-03, AC 5; T15)
Proof: run digital | test a_delayed_commit_with_a_param_delay_is_no_landing_at_its_making ... ok

**C15** - The buck's first 100 µs land at most 1 000 times (TP-03, AC 6; T17)
Proof: run buckacc | test the_first_100_us_land_at_most_1000_times ... ok

**C16** - A step that ends on a landing and reads no jump is tested for truncation error (TP-04, AC 1; T9)
Proof: run solver | test a_step_that_ends_on_a_landing_is_tested ... ok

**C17** - A failing landing step is rejected, restored and retaken smaller, with the instants replayed (TP-04, AC 2; T9)
Proof: run solver | test a_rejected_landing_step_replays_its_instants_and_leaves_no_record ... ok

**C18** - Appendix C.2, D.4 and F say the landing step is tested (TP-04, AC 3; T6)
Proof: run spectext | test the_landing_step_is_tested_in_appendix_c_d_and_f ... ok

**C19** - A jump step is sized by §15.2 rule 6 and tested with the first-order estimate (TP-05, AC 1; T7, T10)
Proof: run integration | test the_jump_estimate_of_a_quadratic_state_is_h_squared ... ok

**C20** - A failing jump step is retaken smaller, still as the step that reads the jump (TP-05, AC 2; T10)
Proof: run solver | test a_jump_step_over_the_tolerance_is_retaken ... ok

**C21** - A jump step failing at the floor is accepted and counted (TP-05, AC 3; T10)
Proof: run solver | test a_jump_step_at_the_floor_is_accepted_and_counted ... ok

**C22** - An ideal 1 V step into 1 kΩ and 1 nF reads `1 − e⁻¹` V within `reltol` after one time constant (TP-05, AC 4; T10)
Proof: run solver | test an_ideal_step_into_an_rc_reads_one_minus_e_inverse ... ok

**C23** - The buck's first 100 µs are within 0.104 mV of the `max_step = 1e-10` run (TP-05, AC 5; T11)
Proof: run buckacc | test the_first_100_us_match_the_fine_reference_within_0_104_mv ... ok

**C24** - §15.2 rule 6 and Appendix C.2 state the estimate and the retake (TP-05, AC 6; T6)
Proof: run spectext | test rule_6_states_the_first_order_estimate_and_the_retake ... ok

**C25** - The buck's 600 µs start takes at most 300 000 accepted steps (TP-06, AC 1; T17)
Proof: run bucklong | test the_whole_start_takes_at_most_300000_steps ... ok

**C26** - Overshoot at most 0.815 V (TP-06, AC 2; T17)
Proof: run bucklong | test the_overshoot_is_within_15_mv ... ok

**C27** - Settling inside ±10 mV within the ramp window, ripple at most 10 mV, checks passing, the end in regulation (TP-06, AC 2; T17)
Proof: run bucklong | test the_start_ends_in_regulation_with_power_good ... ok

**C28** - The reference mode lands on every clock edge of the fixture; the default on the `D2A` changes only (TP-06, AC 3; T16)
Proof: run solver | test the_declared_rule_lands_on_every_edge_and_the_observable_rule_on_the_changes ... ok

**C29** - `buck_long` prints the wall time and asserts none (TP-06, AC 4; T5)
Proof: run bucklong | test the_wall_time_and_the_peak_are_reported ... ok

**C30** - The five scenario families pass unedited (TP-06, AC 5; T23)
Proof: run scenarios | test the_package_declares_the_five_scenario_runs ... ok

**C31** - The second `eval` of a function allocates nothing and a short buffer is still refused (TP-07, AC 1; T19)
Proof: run abialloc | test the_second_eval_of_a_function_allocates_nothing ... ok

**C32** - The second `assemble_tran_into` allocates nothing (TP-07, AC 2; T21)
Proof: run alloc | test the_second_assembly_of_a_stage_allocates_nothing ... ok

**C33** - With timing off the Newton solver's two totals are 0 (TP-07, AC 3; T18)
Proof: run newton | test a_solve_reads_no_clock_when_timing_is_off ... ok

**C34** - With timing on both totals are reported (TP-07, AC 4; T18)
Proof: run newton | test a_solve_reports_both_totals_when_timing_is_on ... ok

**C35** - An accepted step with nothing due, no crossing and no `settle` listener runs no instant (TP-08, AC 1; T22)
Proof: run solver | test an_idle_step_runs_no_instant ... ok

**C36** - The buck's instants run are at most scheduled instants plus crossings plus landings (TP-08, AC 2; T22)
Proof: run buckacc | test the_first_100_us_run_no_idle_instant ... ok

**C37** - The open-loop fixture's records are equal in both rules (TP-09, AC 1; T16)
Proof: run solver | test the_open_loop_fixture_records_the_same_in_both_rules ... ok

**C38** - A rejected step undoes every instant run ahead of it (TP-09, AC 2; T9)
Proof: run solver | test a_rejected_landing_step_replays_its_instants_and_leaves_no_record ... ok

**C39** - Instants run in increasing `(t, n)` in both rules (TP-09, AC 3; T16)
Proof: run solver | test instants_run_in_order_in_both_rules ... ok

**C40** - The parity sources, fixtures, contract, twins and pinned grid are unedited and `parity_tran`'s `tran_rc` passes (TP-09, AC 4; T23)
Proof: run parity | test tran_rc ... ok

**C41** - Without ngspice the parity targets print the skip line and the twins still run (TP-09, AC 5; T23)
Proof: `env PATH=/usr/bin/nonexistent:$HOME/.cargo/bin:/bin cargo test -p piperine-parity --test parity_tran -- --nocapture`

**C42** - `moved.md` lists every moved mixed golden with old value, new value and requirement (TP-09, AC 6; T23)
Proof: run docs | test moved_md_lists_each_moved_golden_with_its_requirement ... ok

**C43** - `GAPS.md` has no V12 and `ROADMAP.md` marks `multi-rate` delivered here (TP-10, AC 1; T24)
Proof: run docs | test v12_is_closed_by_transient_performance ... ok

**C44** - Appendix C.2's time-points row describes the as-if rule, its exceptions and the reference mode (TP-10, AC 2; T12)
Proof: run spectext | test the_time_points_row_describes_the_observable_rule ... ok

**C45** - The scenario package's comment states the measured step count and `diagnosis.md` has its After table (TP-10, AC 3; T23, T24)
Proof: run docs | test the_diagnosis_has_its_after_table ... ok

**C46** - The two backends print the same bytes for the twins (TP-09, AC 4; T23)
Proof: run twins | test the_two_backends_print_the_same_bytes ... ok

**C47** - The pinned time grid of the RC transient is unmoved (TP-09, AC 4; T23)
Proof: run grid | test parity_rc_transient ... ok

**C48** - No parity file is edited on the branch (TP-09, AC 4; T23)
Proof: `git diff --quiet origin/main -- crates/piperine-parity/tests crates/piperine-parity/src/contract.rs crates/piperine-cli/tests/twins.rs crates/piperine-solver/tests/parity_baseline.rs`

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** none - planning only (Opus, 2026-11-20). Implementation waits for `analog-operators` and `unknown-semantics-events` on main.
**Walked:** not yet

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5

Phase 1:  T1 ------→ T2 ------→ T3 ------→ T4 ------→ T5
Phase 2:  T6 ------→ T7 ------→ T8 ------→ T9 ------→ T10 ------→ T11
Phase 3:  T12 ------→ T13 ------→ T14 ------→ T15 ------→ T16 ------→ T17
Phase 4:  T18 ------→ T19 ------→ T20 ------→ T21 ------→ T22
Phase 5:  T23 ------→ T24
```

Execution is strictly sequential - there is no intra-phase parallelism.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: `LandingCause` and `TimePointRule`, and the cause on a landing | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T2: `StepStatistics` and `StepEnd` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T3: The protocol fills `StepStatistics` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T4: The analog side's counts | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T5: `Session::statistics` and the buck's baseline counts | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T6: Spec: the landing step is tested, the jump step is error-controlled | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T7: `TrBdf2::jump_lte` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T8: `StepKind` replaces the two booleans | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T9: The step that ends on a landing is tested | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T10: The step that reads a jump is tested with `jump_lte` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T11: The buck's accuracy gate | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T12: Spec: the time points as implemented | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T13: `Analysis::samples` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T14: `LandingTest` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T15: `TimePointRule::Observable` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T16: The rule reaches the solver, and the open-loop fixture | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T17: The buck's step and landing gates | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T18: `SolveTimer` | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T19: `FunctionFrame`: cell lengths computed once | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T20: `Assembly::run` reuses its buffers | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T21: The stamps and the node list are reused | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T22: No instant at an idle step | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T23: The whole regression and the scenario package | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |
| T24: The records | 1 type, 1 function or 1 document change, with its tests | ✅ Granular |

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | None | ✅ Match |
| T2 | T1 | T1 | ✅ Match |
| T3 | T2 | T2 | ✅ Match |
| T4 | T3 | T3 | ✅ Match |
| T5 | T4 | T4 | ✅ Match |
| T6 | T5 | T5 | ✅ Match |
| T7 | T6 | T6 | ✅ Match |
| T8 | T7 | T7 | ✅ Match |
| T9 | T8 | T8 | ✅ Match |
| T10 | T9 | T9 | ✅ Match |
| T11 | T10 | T10 | ✅ Match |
| T12 | T11 | T11 | ✅ Match |
| T13 | T12 | T12 | ✅ Match |
| T14 | T13 | T13 | ✅ Match |
| T15 | T14 | T14 | ✅ Match |
| T16 | T15 | T15 | ✅ Match |
| T17 | T16 | T16 | ✅ Match |
| T18 | T17 | T17 | ✅ Match |
| T19 | T18 | T18 | ✅ Match |
| T20 | T19 | T19 | ✅ Match |
| T21 | T20 | T20 | ✅ Match |
| T22 | T21 | T21 | ✅ Match |
| T23 | T22 | T22 | ✅ Match |
| T24 | T23 | T23 | ✅ Match |

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1 | Digital engine | unit | unit | ✅ OK |
| T2 | Pure types | unit | unit | ✅ OK |
| T3 | Protocol and analog side | integration | integration | ✅ OK |
| T4 | Protocol and analog side | integration | integration | ✅ OK |
| T5 | Host surface | integration | integration | ✅ OK |
| T6 | Spec and records | integration | integration | ✅ OK |
| T7 | Pure types | unit | unit | ✅ OK |
| T8 | Protocol and analog side | integration | integration | ✅ OK |
| T9 | Protocol and analog side | integration | integration | ✅ OK |
| T10 | Protocol and analog side | integration | integration | ✅ OK |
| T11 | The buck | integration | integration | ✅ OK |
| T12 | Spec and records | integration | integration | ✅ OK |
| T13 | Digital engine | unit | unit | ✅ OK |
| T14 | Digital engine | unit | unit | ✅ OK |
| T15 | Digital engine | unit | unit | ✅ OK |
| T16 | Protocol and analog side | integration | integration | ✅ OK |
| T17 | The buck | integration | integration | ✅ OK |
| T18 | Pure types | unit | unit | ✅ OK |
| T19 | Allocation | unit / integration | unit | ✅ OK |
| T20 | Allocation | unit / integration | integration | ✅ OK |
| T21 | Allocation | unit / integration | integration | ✅ OK |
| T22 | Protocol and analog side | integration | integration | ✅ OK |
| T23 | Spec and records | integration | integration | ✅ OK |
| T24 | Spec and records | integration | integration | ✅ OK |
