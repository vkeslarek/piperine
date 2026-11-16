# Solver Crossings Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: inline, section "Design" below
**Status**: Done
**Approved**: 2026-11-15 - user: "Pode ir rodando os grandes passos" (standing delegation)
**Build**: `cargo build --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run digital**: `cargo test -p piperine-digital-solver --test instant`
**Run analog**: `cargo test -p piperine-analog-solver --test assembly_watch`
**Run solver**: `cargo test -p piperine-solver --test settled_value --test protocol_crossings --test crossing_rules`
**Run parity**: `cargo test -p piperine-solver --test parity_baseline`
**Run abi**: `cargo test -p piperine-abi --test validate`
**Run env**: `cargo test -p piperine-digital-solver --test functions`
**Run solverenv**: `cargo test -p piperine-solver --test environment`

Implementer: Opus, inline (11 tasks; Phase 4 was added mid-build at the coordinator's request); verifier: a different Opus, dispatched by the orchestrator. Worktree `/home/keslarek/Git/piperine-solver-crossings`, branch `feature/solver-crossings`; no push, no merge. Cargo runs with `CARGO_TARGET_DIR=/var/tmp/piperine-target`, plain `cargo test` at the root (default members), never `--workspace`.

Rules every task follows:

- ngspice parity is the top priority: `parity_baseline` and every parity test pass unedited; no tolerance is retuned; a design with no `CROSS` watch takes the same steps as before.
- Expected values come from the spec or from analytic computation in the test, never from the code under test.
- No test is weakened, skipped or deleted. The one exception is the assertion of `protocol_crossings.rs::a_fired_crossing_is_a_commit_at_the_landing` that encodes the unlocated crossing ("the first accepted time after the sign change"): T6 replaces it with the located behavior and its commit says so.
- No macro, no `unsafe`, no file-scope `#![allow]`, no `#[ignore]`; each new test target opens with a `//!` scope header.
- Zero warnings.

---

## Design

**Fix 1 (DSV-01).** `instant.rs` applies the commits due at a microstep in C2 order. Event-net commits fire as today. Discrete commits are grouped by net, in the order of their first commit: each net's settled bytes start from its current value and take each commit's bytes at its offset, in order; the net is then written once through `write_net` (one `Store::write` over the whole net, so `CHANGED` and `PREVIOUS` compare against the value before the microstep, and at most one `Change` is observed). The quiet-net exit path does the same. `host_param_write` is computed per net; `check_canonical` names the last writer.

**Fix 2 (SCX-01 to SCX-04).**

- `piperine-analog-solver` (`device/plan.rs`, `device/watch.rs`, `device/assembly.rs`): `WatchPlan` gains `ttol` and `etol`, the discrete slots of the watch's tolerance pins (`None` for `DEFAULT`). `accept_watches` reads them for every `CROSS` watch and fails with `E0510` on a value that is not finite or is `≤ 0`; a `CAP` value that is not finite or is `≤ 0` fails with `E0509`. `Assembly::rising(x, amb)` evaluates the watch functions at `x` (the same `Need::ACCEPT` evaluation `accept` makes, cached by F3) and returns each `CROSS` watch that went from `≤ 0` at the last accepted solution to `> 0` at `x`, with its value before and now and its tolerances, without accepting anything. `Assembly::has_crossings()` says whether any `CROSS` watch exists.
- `piperine-solver` (`analyses/transient.rs`): `TransientAnalysisOptions::ttol` (1 ps). In `device_attempt`, when the circuit has a `CROSS` watch, the step solves to `to` exactly (a tick), and after a converged attempt that reads no jump (`sets_just_applied` false), the locator runs: for each rising watch, satisfied when `e < etol` or `t1 − t_est ≤ ttol`, where `t_est` is the secant of the last two attempts of this step past the crossing when it falls inside the bracket, else the secant of the bracket `(t0, e0)`–`(t1, e1)` with `e0` halved once per earlier attempt past the crossing (Illinois; added by verification finding 4, where the plain bracket secant stalled on a concave `e`). All satisfied, or the earliest unsatisfied estimate rounding up to the attempt's own tick: `Converged`. Otherwise the attempt is recorded and `LandedEarlier(ceil_tick(min t_est))` sends the protocol back to retake. The record lives on `DeviceRun` (it survives the protocol's restore) and is dropped when the accepted time moves. Because such a step ends on the tick below the controller's proposal, the controller grows from the proposal, not from the floored step (a step of a few ticks would otherwise floor to one tick forever after a landing next to its accepted time). The protocol is not edited: its "landed earlier" path lands on the given tick and counts the retake toward `max_rejections`.
- An operating point and a later operating point already compare each `CROSS` watch with the last accepted solution (`Watched::cross` persists in the assembly); T7 proves it on the solver.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` (zero warnings; the numeric oracles; MD-28 placement), `.specs/features/solver-abi-analog/tasks.md` (expected values computed in the test), the orchestrator's rules (parity unchanged, never retune).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Digital engine (`src/instant.rs`) | unit | AC 1 to 3 of P1, values from §12.1 | `crates/piperine-digital-solver/tests/instant.rs` | `cargo test -p piperine-digital-solver` |
| Analog assembly (`src/device/*`) | unit | `E0509`, `E0510`, `rising` without accepting | `crates/piperine-analog-solver/tests/assembly_watch.rs` | `cargo test -p piperine-analog-solver` |
| Transient driver and owner (`src/analyses/transient.rs`) | integration | every AC of P2 on real circuits of devices, crossing times computed analytically | `crates/piperine-solver/tests/{protocol_crossings,crossing_rules,settled_value}.rs` | `cargo test -p piperine-solver` |
| Parity oracles | integration | unedited, green | `crates/piperine-solver/tests/parity_baseline.rs`, `crates/piperine-parity/tests/` | `cargo test` |
| Docs and specs | none | - | `docs/spec/`, `.specs/` | build gate only |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | the task's own target | `cargo test -p <crate> --test <target>` |
| Full | every code task | `cargo test -p <crate>` of each crate the task edits |
| Build | last task of each phase | the `**Build**` line, then `cargo test` at the root (default members) |

---

## Execution Plan

Phases are ordered and run sequentially.

### Phase 1: One settled value per microstep

```
T1 → T2
```

### Phase 2: The analog side of a crossing

```
T3 → T4 → T5
```

### Phase 3: The locator and its proofs

```
T6 → T7 → T8
```

### Phase 4: The environment in reactions (added mid-build)

```
T9 → T10 → T11
```

---

## Task Breakdown

### T1: Coalesce the commits of one microstep per net

**What**: the microstep applies the discrete commits due at it as one settled write per net.
**Where**: `crates/piperine-digital-solver/src/instant.rs` (tests in `tests/instant.rs`)
**Depends on**: None
**Reuses**: `run::write_net`, `run::check_canonical`
**Requirement**: DSV-01

**Done when**:

- [x] `two_commits_to_one_net_settle_to_one_write`, `a_commit_undone_in_its_microstep_is_no_change`, `a_settled_change_is_one_observation` pass
- [x] Gate check passes: `cargo test -p piperine-digital-solver`

**Tests**: unit
**Gate**: full

**Commit**: `fix(digital): settle the commits of one microstep to one write per net`

---

### T2: The T42 repro on the solver

**What**: the pure-ABI repro of `ir-rust` T42 as a solver test: `x <- HIGH; x <- LOW;` on a net holding `LOW` fires no `x.changed()`.
**Where**: `crates/piperine-solver/tests/settled_value.rs`
**Depends on**: T1
**Reuses**: `tests/helpers/abi_mixed.rs` (`delay`)
**Requirement**: DSV-01

**Done when**:

- [x] `x_changed_does_not_fire_when_the_settled_value_is_unchanged` passes
- [x] Gate check passes: the **Build** line, then `cargo test` at the root

**Tests**: integration
**Gate**: build

**Commit**: `test(solver): two commits that leave a net as it was fire no changed()`

---

### T3: `E0509` on a `CAP` watch

**What**: a `CAP` value that is not finite or is `≤ 0` fails the accept with `E0509`, naming instance and watch.
**Where**: `crates/piperine-analog-solver/src/device/watch.rs` (tests in `tests/assembly_watch.rs`)
**Depends on**: None
**Reuses**: `accept_watches`
**Requirement**: SCX-03

**Done when**:

- [x] `accept_fails_with_e0509_on_an_invalid_cap` passes
- [x] Gate check passes: `cargo test -p piperine-analog-solver`

**Tests**: unit
**Gate**: full

**Commit**: `feat(analog): an invalid step cap is E0509`

---

### T4: `ttol` and `etol` slots and `E0510`

**What**: `WatchPlan` takes the slots of a `CROSS` watch's tolerance pins; the accept fails with `E0510` on a tolerance that is not finite or is `≤ 0`.
**Where**: `crates/piperine-analog-solver/src/device/plan.rs` (with `watch.rs`; fixture in `tests/support/devices.rs`, tests in `tests/assembly_watch.rs`)
**Depends on**: T3
**Reuses**: `TermPlan::max`'s slot binding
**Requirement**: SCX-03

**Done when**:

- [x] `accept_fails_with_e0510_on_an_invalid_tolerance` passes
- [x] Gate check passes: `cargo test -p piperine-analog-solver`

**Tests**: unit
**Gate**: full

**Commit**: `feat(analog): crossing tolerances from their pins, E0510 on an invalid one`

---

### T5: `Assembly::rising`

**What**: the rising `CROSS` watches at a solution, with their values and tolerances, without accepting it.
**Where**: `crates/piperine-analog-solver/src/device/assembly.rs` (tests in `tests/assembly_watch.rs`)
**Depends on**: T4
**Reuses**: `evaluate_for(Need::ACCEPT)`
**Requirement**: SCX-01

**Done when**:

- [x] `rising_reports_a_sign_change_without_accepting` passes
- [x] Gate check passes: the **Build** line, then `cargo test` at the root

**Tests**: unit
**Gate**: build

**Commit**: `feat(analog): report rising crossings at a solution without accepting it`

---

### T6: The locator

**What**: `device_attempt` retakes a step until each rising crossing is within `ttol` or `etol`, landing on the earliest at a tick; `TransientAnalysisOptions::ttol`; tick-exact steps for a design with a `CROSS` watch.
**Where**: `crates/piperine-solver/src/analyses/transient.rs` (tests in `tests/protocol_crossings.rs`)
**Depends on**: None
**Reuses**: the protocol's `Attempt::LandedEarlier`
**Requirement**: SCX-01, SCX-04

**Done when**:

- [x] `a_ramp_lands_within_ttol_plus_one_tick`, `a_sine_crossing_is_located`, `an_rc_charge_trips_at_its_threshold`, `a_fired_crossing_is_a_commit_at_the_landing` (its unlocated assertion replaced), `the_earliest_of_several_crossings_lands_first`, `crossings_on_one_tick_fire_together`, `the_session_ttol_applies_to_a_watch_without_a_ttol_pin`, `a_watch_without_an_etol_pin_takes_the_potential_abstol`, `a_crossing_in_the_step_after_a_jump_is_not_located` pass
- [x] `parity_baseline` passes unedited
- [x] Gate check passes: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: full

**Commit**: `feat(solver): locate a crossing inside the step`

---

### T7: Crossings across operating points, `E0509` and `E0510` on the solver

**What**: solver-level proofs of SCX-02 and SCX-03: a crossing between the rounds of an operating point and between two operating points fires; an invalid tolerance or cap fails the analysis.
**Where**: `crates/piperine-solver/tests/crossing_rules.rs`
**Depends on**: T6
**Reuses**: `tests/helpers/abi_mixed.rs`, `tests/helpers/abi_devices.rs`
**Requirement**: SCX-02, SCX-03

**Done when**:

- [x] `a_crossing_between_operating_point_rounds_fires_in_the_later_round`, `a_crossing_between_two_operating_points_fires_at_the_later`, `an_invalid_ttol_or_etol_fails_the_analysis_with_e0510`, `an_invalid_step_cap_fails_the_analysis_with_e0509` pass
- [x] Gate check passes: `cargo test -p piperine-solver`

**Tests**: integration
**Gate**: full

**Commit**: `test(solver): crossings across operating points, E0509 and E0510`

---

### T8: Close the gap in the spec and the trackers

**What**: §17.3's closing note, Appendix C.2's crossings row and C.2.1's `ttol`/`etol` defaults state the located crossing; `solver-completeness` traceability marks SCX-01 to SCX-03.
**Where**: `docs/spec/appendix_c_implementation.md` (with `part_v_execution.md`, `.specs/features/solver-completeness/spec.md`)
**Depends on**: T7
**Reuses**: none
**Requirement**: SCX-01, SCX-02, SCX-03

**Done when**:

- [x] Gate check passes: the **Build** line, then `cargo test` at the root

**Tests**: none
**Gate**: build

**Commit**: `docs(spec): crossings are located; the default ttol and etol`

---

### T9: The validator accepts `GMIN` and `SOURCE_SCALE` in a reaction

**What**: the validator lets a reaction and a discrete function read `GMIN` and `SOURCE_SCALE`; abi.md §4.2 states the rule.
**Where**: `crates/piperine-abi/src/validate.rs` (abi.md §4.2; tests in `tests/validate.rs`)
**Depends on**: None
**Reuses**: the ambient table of `cell_bytes`
**Requirement**: ENV-01

**Done when**:

- [x] `a_reaction_may_read_gmin_and_source_scale` passes
- [x] Gate check passes: `cargo test -p piperine-abi`

**Tests**: unit
**Gate**: full

**Commit**: `feat(abi): a reaction may read gmin and source_scale`

---

### T10: The digital engine supplies the continuation values

**What**: `Ambient` carries `gmin` and `source_scale`; `Digital::set_continuation` sets them; reactions and discrete functions read them.
**Where**: `crates/piperine-digital-solver/src/run.rs` (with `engine.rs`, `instant.rs`; tests in `tests/functions.rs`)
**Depends on**: T9
**Reuses**: `cell_bytes`
**Requirement**: ENV-02

**Done when**:

- [x] `reactions_and_functions_read_the_continuation_values` passes
- [x] Gate check passes: `cargo test -p piperine-digital-solver`

**Tests**: unit
**Gate**: full

**Commit**: `feat(digital): reactions and discrete functions read gmin and source_scale`

---

### T11: The solver wires temperature, gmin and source_scale to reactions

**What**: the solver sets the engine's continuation values to the session `gmin` and 1.0 for each analysis; a reaction reads the host's temperature through its `GLOBAL TEMPERATURE` pin.
**Where**: `crates/piperine-solver/src/analyses/adapters.rs` (tests in `tests/environment.rs`)
**Depends on**: T10
**Reuses**: `DigitalSide::new`
**Requirement**: ENV-03

**Done when**:

- [x] `a_reaction_reads_temperature_gmin_and_source_scale` and `a_discrete_function_reads_gmin_and_source_scale` pass
- [x] Gate check passes: the **Build** line, then `cargo test` at the root

**Tests**: integration
**Gate**: build

**Commit**: `feat(solver): wire temperature, gmin and source_scale to reactions`

---

## Checks

**C1** - Two commits to one net in one microstep write it once: `HIGH` then a one-byte `0` at offset 0 settle to `LOW` with one `Change` (DSV-01, AC 1)
Proof: run digital | test two_commits_to_one_net_settle_to_one_write ... ok

**C2** - `LOW` then `HIGH` on a net holding `HIGH` reports no `Change` and fires no edge downstream (DSV-01, AC 2)
Proof: run digital | test a_commit_undone_in_its_microstep_is_no_change ... ok

**C3** - `HIGH`, `LOW`, `HIGH` on a net holding `LOW` is one `Change` to `HIGH`, with `PREVIOUS` `LOW` (DSV-01, AC 3)
Proof: run digital | test a_settled_change_is_one_observation ... ok

**C25** - An `F64` net holding 1.0 that takes 2.0 then 3.0 in one microstep reads `PREVIOUS` 1.0, the value before the microstep, not 2.0 (DSV-01, AC 3; verification finding)
Proof: run digital | test previous_is_the_value_before_the_microstep ... ok

**C26** - A concave crossing (`e = 1 − exp(−t/1 µs) − 0.999`, steps of 20 µs) is located within `etol` or `ttol` plus one tick; with the bracket secant alone (no `prev`) the step exceeds 64 consecutive retakes (SCX-01, AC 1; verification finding)
Proof: run solver | test a_curved_crossing_converges_from_both_sides ... ok

**C27** - After a located landing the next step starts from the controller's step before the retake: on a 10 kHz sine with 20 located crossings, at most 4 accepted steps per landing over the unlocated run (SCX-01; verification finding)
Proof: run solver | test a_landing_costs_a_few_steps_not_a_regrowth ... ok

**C28** - A `CROSS` design started at a 1 fs step grows back to the 1 µs grid within 60 extra steps; with the controller grown from the floored step it exhausts a 5000-step budget in under 2 s instead of hanging (SCX-01; verification finding)
Proof: run solver | test steps_of_a_few_ticks_grow_back ... ok

**C29** - An analog function and the reaction of a transient's operating point read one `GMIN`, the session's 3e-12 S: v(out) = gmin·1e12 = 3 V from t = 0 (1 V at t = 0 when the transient's operating point used the default context) (ENV-03, AC 3; verification finding)
Proof: run solverenv | test analog_and_reactions_read_one_gmin_at_the_transient_start ... ok

**C4** - On the solver, `x <- HIGH; x <- LOW;` on `x = LOW` fires `x.changed()` once in the run, at the initial value (DSV-01, AC 4)
Proof: run solver | test x_changed_does_not_fire_when_the_settled_value_is_unchanged ... ok

**C5** - A 1 kV/s ramp through 0.5 V, with `etol` out of play, lands no later than 0.5 ms plus `ttol` plus one tick and not before it, for initial steps of 10, 7 and 3 µs (SCX-01, AC 1)
Proof: run solver | test a_ramp_lands_within_ttol_plus_one_tick ... ok

**C6** - A 1 V 1 kHz sine against 0.5 V fires with `0 < e < 1 µV` or within 1 ps plus one tick of `asin(0.5)/(2π·1 kHz)` (SCX-01, AC 1)
Proof: run solver | test a_sine_crossing_is_located ... ok

**C7** - An RC charge (1 kΩ, 1 nF, from 0 to 1 V at the clock edge) trips a 0.625 V comparator with `0 < v − 0.625 < 1 µV` or within 1 ps plus one tick of the analytic time (SCX-01, AC 1)
Proof: run solver | test an_rc_charge_trips_at_its_threshold ... ok

**C8** - The located crossing is at a tick, an accepted step is at that exact time, and its event pin's commit changes `out` at that time, one microstep later (SCX-01, AC 2)
Proof: run solver | test a_fired_crossing_is_a_commit_at_the_landing ... ok

**C9** - Two crossings in one step (0.3 V and 0.7 V on the ramp) land in order, each at its own located time (SCX-01, AC 3)
Proof: run solver | test the_earliest_of_several_crossings_lands_first ... ok

**C10** - Two comparators with one threshold fire at one tick, together (SCX-01, AC 3)
Proof: run solver | test crossings_on_one_tick_fire_together ... ok

**C11** - A watch with no `ttol` pin takes `TransientAnalysisOptions::ttol`: at 1 s, the sine's crossing fires where `e` exceeds 1 µV (SCX-01, AC 4)
Proof: run solver | test the_session_ttol_applies_to_a_watch_without_a_ttol_pin ... ok

**C12** - A watch with no `etol` pin takes the session's potential `abstol`: at `vntol` 0.6 V the sine's crossing fires where `e` exceeds 1 µV, and at the default it fires under 1 µV (SCX-01, AC 4)
Proof: run solver | test a_watch_without_an_etol_pin_takes_the_potential_abstol ... ok

**C13** - A comparator on a clock edge fires at the end of the step that reads the jump, more than `ttol` plus one tick after the edge (SCX-01, AC 5)
Proof: run solver | test a_crossing_in_the_step_after_a_jump_is_not_located ... ok

**C14** - A source raised from 0 V to 1 V by a reaction of the operating point's first round makes a 0.5 V comparator fire in the next round, and `out` is `HIGH` after the operating point (SCX-02, AC 6)
Proof: run solver | test a_crossing_between_operating_point_rounds_fires_in_the_later_round ... ok

**C15** - A source set from 0.3 V to 0.7 V between two operating points makes a 0.5 V comparator fire at the second, not at the first (SCX-02, AC 7)
Proof: run solver | test a_crossing_between_two_operating_points_fires_at_the_later ... ok

**C16** - A `ttol` or `etol` pin of 0, −1 ps, NaN or +inf fails the transient with an error naming `E0510` and the watch (SCX-03, AC 8)
Proof: run solver | test an_invalid_ttol_or_etol_fails_the_analysis_with_e0510 ... ok

**C17** - A `CAP` of 0, −1 ns, NaN or +inf fails the transient with an error naming `E0509` and the watch (SCX-03, AC 9)
Proof: run solver | test an_invalid_step_cap_fails_the_analysis_with_e0509 ... ok

**C18** - The RC transient parity baseline still takes 161 steps and ends at its pinned value, unedited (SCX-04, AC 10)
Proof: run parity | test parity_rc_transient ... ok

**C19** - `Assembly::rising` reports a `CROSS` watch that went from `≤ 0` to `> 0` with its two values, and a later `accept` still fires it: nothing was accepted (SCX-01, AC 1)
Proof: run analog | test rising_reports_a_sign_change_without_accepting ... ok

**C20** - A reaction or a discrete function that reads `GMIN` or `SOURCE_SCALE` validates; `FREQ` in a reaction and `TIME` in a discrete function are still refused (ENV-01, AC 1)
Proof: run abi | test a_reaction_may_read_gmin_and_source_scale ... ok

**C21** - On the engine, a reaction and a discrete function read the values `set_continuation` gave (2e-12, 0.25), and 0 and 1 before (ENV-02, AC 2)
Proof: run env | test reactions_and_functions_read_the_continuation_values ... ok

**C22** - On the solver, a reaction commits `temperature()` = the host's 350 K, `gmin()` = the session's 3e-12 S and `source_scale()` = 1.0, in the operating point and the transient (ENV-03, AC 3 and 4)
Proof: run solverenv | test a_reaction_reads_temperature_gmin_and_source_scale ... ok

**C23** - On the solver, a discrete function reads `gmin()` = the session's and `source_scale()` = 1.0 (ENV-03, AC 3)
Proof: run solverenv | test a_discrete_function_reads_gmin_and_source_scale ... ok

**C24** - Parity is unchanged after the wiring: the RC baseline still takes 161 steps (ENV-03, AC 5)
Proof: run parity | test parity_rc_transient ... ok

---

## Handoff

**Settled mid-build:** user, via the coordinator, 2026-11-15: "temperature e gmin precisa estar wired..." — Fix 3 (ENV-01 to ENV-03, Phase 4): the ABI accepts ambient reads of `temperature`, `gmin`, `source_scale` by reactions and discrete functions, and the solver supplies their current values.
**Dispatched:** none (the orchestrator dispatches the Verifier, a different Opus)
**Walked:** no screen

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1:  T1 ------→ T2
Phase 2:  T3 ------→ T4 ------→ T5
Phase 3:  T6 ------→ T7 ------→ T8
Phase 4:  T9 ------→ T10 -----→ T11
```

Execution is strictly sequential, inline (8 tasks, one batch).
