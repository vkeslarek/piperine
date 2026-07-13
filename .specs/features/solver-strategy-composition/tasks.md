# Solver Strategy Composition — Tasks

**Design:** `.specs/features/solver-strategy-composition/design.md`
**Status:** Draft

---

## Test Coverage Matrix

> Generated from codebase. Guidelines found: `AGENTS.md` (zero warnings, `cargo test --workspace`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|------------|-------------------|---------------------|-------------------|-------------|
| Solver math (newton_raphson) | unit | All branches; existing tests stay green | `src/math/newton_raphson.rs` (inline `#[cfg(test)]`) | `cargo test -p piperine-solver` |
| Solver drivers (dc, transient, ac, noise, tf) | integration | All analyses run; results match baseline | `tests/{mixed_signal,digital_topology}.rs` | `cargo test -p piperine-solver` |
| Codegen device (analog, digital, mod) | integration | Compiled PHDL devices still stamp correctly | `tests/{from_ir,codegen_ir}.rs` | `cargo test -p piperine-codegen` |
| Bench e2e | e2e | Examples still pass | `tests/{bench,run_examples}.rs` | `cargo test -p piperine-bench` |

## Gate Check Commands

| Gate Level | When to Use | Command |
|------------|-------------|---------|
| Quick | After solver-only changes | `cargo test -p piperine-solver` |
| Full | After codegen/bench changes | `cargo test --workspace` |
| Build | After struct/signature changes | `cargo build --workspace` |

---

## Execution Plan

### Phase 1: Tolerances/Policy split (pure refactor, no behavior change)

```
T1 → T2 → T3
```

### Phase 2: NewtonStrategy (move free fns into trait)

```
T4 → T5 → T6
```

### Phase 3: StepperStrategy + AnalysisContext

```
T7 → T8 → T9
```

### Phase 4: SignalBridge + cleanup

```
T10 → T11
```

---

## Task Breakdown

### T1: Create `Tolerances` and `Policy` structs

**What**: Extract `Tolerances` (immutable, `Copy`) and `Policy` (mutable) from `Context` fields.
**Where**: `crates/piperine-solver/src/solver/mod.rs`
**Depends on**: None
**Requirement**: STRAT-03

**Done when**:
- [ ] `Tolerances` struct with `reltol`, `vntol`, `abstol`, `gmin`, `min_res`, `trtol`, `chgtol`, `temperature`, `tnom`, `integration` — `Copy`
- [ ] `Policy` struct with `max_iter`, `dc_damp_tolerance`, `time`
- [ ] `Context` holds `tolerances: Tolerances` — no flat tolerance fields remain
- [ ] `Context::default()` constructs default `Tolerances`
- [ ] `has_converged` moves to `Tolerances` method
- [ ] `init_global` stays on `Context`
- [ ] `cargo build -p piperine-solver` passes

**Tests**: none (struct rearrangement)
**Gate**: build

---

### T2: Update all callers to read `ctx.tolerances.*`

**What**: Replace every `ctx.reltol` / `ctx.abstol` / etc. with `ctx.tolerances.reltol` / `ctx.tolerances.abstol`.
**Where**: `solver/dc.rs`, `solver/transient.rs`, `solver/ac.rs`, `solver/noise.rs`, `solver/tf.rs`, `solver/mod.rs`, `analysis/*.rs`, `core/element.rs`, `core/circuit.rs`
**Depends on**: T1
**Requirement**: STRAT-03

**Done when**:
- [ ] No `ctx.reltol` / `ctx.vntol` / `ctx.abstol` / `ctx.gmin` / `ctx.trtol` / `ctx.chgtol` / `ctx.temperature` / `ctx.tnom` / `ctx.integration` remain — all go through `ctx.tolerances.*`
- [ ] `ctx.max_iter` / `ctx.dc_damp_tolerance` / `ctx.time` accessed from a local `Policy` where the driver owns it
- [ ] `cargo build --workspace` passes, zero warnings
- [ ] `cargo test --workspace` green

**Tests**: integration (existing tests verify no behavior change)
**Gate**: full

---

### T3: Export `Tolerances` and `Policy` in prelude

**What**: Add `Tolerances`, `Policy` to `prelude.rs`.
**Where**: `crates/piperine-solver/src/prelude.rs`
**Depends on**: T2
**Requirement**: STRAT-03

**Done when**:
- [ ] `use piperine_solver::prelude::*;` includes `Tolerances` and `Policy`
- [ ] `cargo build --workspace` passes

**Tests**: none
**Gate**: build

---

### T4: Define `NewtonStrategy` trait + `DampedNewton` default impl

**What**: Create `NewtonStrategy` trait with `damp_update`, `is_converged`, `max_iter`. Implement `DampedNewton` wrapping today's `apply_damping` + `check_convergence` + `residual_converged` logic.
**Where**: `crates/piperine-solver/src/solver/convergence.rs`
**Depends on**: T2
**Requirement**: STRAT-01, STRAT-06

**Done when**:
- [ ] `NewtonStrategy` trait defined with the three methods
- [ ] `DampedNewton` struct implements it using the existing logic (moved from free fns)
- [ ] Free fns `check_convergence`, `residual_converged`, `apply_damping` removed from `solver/mod.rs`
- [ ] `cargo build -p piperine-solver` passes

**Tests**: unit (verify `DampedNewton::is_converged` matches old `check_convergence` + `residual_converged`)
**Gate**: quick

---

### T5: Wire `NewtonRaphsonSolver` to use `NewtonStrategy`

**What**: `NewtonRaphsonSolver::solve` takes `&dyn NewtonStrategy` instead of calling `system.apply_limit`/`converged`/`residual_converged`. Remove those methods from `NonLinearSystem`. Remove `alpha` parameter from `assemble`.
**Where**: `crates/piperine-solver/src/math/newton_raphson.rs`, `crates/piperine-solver/src/solver/{dc,transient,ac,noise,tf}.rs`
**Depends on**: T4
**Requirement**: STRAT-01, STRAT-02

**Done when**:
- [ ] `NonLinearSystem::assemble` takes only `&CircularArrayBuffer2<E>` (no `alpha`)
- [ ] `NonLinearSystem` no longer has `apply_limit`, `converged`, `residual_converged`
- [ ] `NewtonRaphsonSolver::solve` takes `strategy: &dyn NewtonStrategy`
- [ ] All `DcSystem`/`TransientSystem`/`AcSystem` impls updated
- [ ] All `.solve(system, alpha, max_iter)` call sites updated to `.solve(system, strategy)`
- [ ] `cargo build --workspace` passes, zero warnings
- [ ] `cargo test --workspace` green

**Tests**: integration (existing tests verify results match)
**Gate**: full

---

### T6: Add `NewtonStrategy` to `ConvergencePlan`

**What**: `ConvergencePlan` holds `newton: Box<dyn NewtonStrategy>`. `HomotopyDriver::newton` uses the plan's strategy. Default plan uses `DampedNewton`.
**Where**: `crates/piperine-solver/src/solver/convergence.rs`, `crates/piperine-solver/src/solver/dc.rs`
**Depends on**: T5
**Requirement**: STRAT-01

**Done when**:
- [ ] `ConvergencePlan` has `newton: Box<dyn NewtonStrategy>` field
- [ ] `ConvergencePlan::default()` uses `DampedNewton`
- [ ] DC driver reads `plan.newton()` instead of constructing strategy locally
- [ ] `cargo test --workspace` green

**Tests**: integration
**Gate**: full

---

### T7: Define `StepperStrategy` trait + `LteStepper` default impl

**What**: Create `StepperStrategy` trait with `propose_dt`, `reject_dt`. Implement `LteStepper` wrapping today's inline transient logic.
**Where**: `crates/piperine-solver/src/solver/convergence.rs`
**Depends on**: T6
**Requirement**: STRAT-01

**Done when**:
- [ ] `StepperStrategy` trait defined
- [ ] `LteStepper` implements it using today's LTE + 2× growth + halve-on-reject logic
- [ ] `cargo build -p piperine-solver` passes

**Tests**: unit
**Gate**: quick

---

### T8: Define `AnalysisContext` enum + per-analysis structs

**What**: Create `DcContext`, `AcContext`, `TransientContext`, `NoiseContext`, `TfContext`. `TransientAnalysisOptions` becomes a constructor for `TransientContext`.
**Where**: `crates/piperine-solver/src/analysis/transient.rs`, `crates/piperine-solver/src/analysis/{dc,ac,noise,tf}.rs`
**Depends on**: T7
**Requirement**: STRAT-04

**Done when**:
- [ ] `TransientContext` carries `dt`, `dt_min`, `dt_max`, `adaptive`, `record_from`, `stop_time`
- [ ] `DcContext` carries `max_iter`, `dc_damp_tolerance` (from `Policy`)
- [ ] `TransientSolver::new` takes `TransientContext` instead of `TransientAnalysisOptions`
- [ ] `cargo build --workspace` passes
- [ ] `cargo test --workspace` green

**Tests**: integration
**Gate**: full

---

### T9: Wire `TransientSolver` to use `StepperStrategy`

**What**: `TransientSolver::solve` delegates timestep selection to `StepperStrategy`. Remove inline `dt * 2.0` / `dt * 0.5` / LTE loop.
**Where**: `crates/piperine-solver/src/solver/transient.rs`
**Depends on**: T8
**Requirement**: STRAT-01

**Done when**:
- [ ] No inline `dt_proposed * 2.0` or `dt_proposed * 0.5` in transient driver
- [ ] `stepper.propose_dt(...)` and `stepper.reject_dt(...)` called instead
- [ ] `cargo test --workspace` green — transient results match baseline

**Tests**: integration
**Gate**: full

---

### T10: Extract `SignalBridge`

**What**: Move `accept_and_run_digital` logic into `SignalBridge` struct owned by `CircuitInstance`.
**Where**: `crates/piperine-solver/src/core/circuit.rs`
**Depends on**: T9
**Requirement**: STRAT-05

**Done when**:
- [ ] `SignalBridge` struct defined with `accept_and_settle` method
- [ ] `CircuitInstance` holds `bridge: SignalBridge`
- [ ] `accept_and_run_digital` delegates to `self.bridge.accept_and_settle(...)`
- [ ] `cargo test --workspace` green

**Tests**: integration
**Gate**: full

---

### T11: Final cleanup — prelude, dead code, docs

**What**: Export new types in prelude. Remove any orphaned code. Update `SOLVER_GAPS.md` checkboxes.
**Where**: `prelude.rs`, `SOLVER_GAPS.md`
**Depends on**: T10
**Requirement**: STRAT-07

**Done when**:
- [ ] `NewtonStrategy`, `StepperStrategy`, `AnalysisContext` in prelude
- [ ] No free `pub(crate) fn` in `solver/mod.rs`
- [ ] `SOLVER_GAPS.md` updated with DONE markers
- [ ] `cargo build --workspace` zero warnings
- [ ] `cargo test --workspace` green

**Tests**: none
**Gate**: build

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1:  T1 ──→ T2 ──→ T3
Phase 2:  T4 ──→ T5 ──→ T6
Phase 3:  T7 ──→ T8 ──→ T9
Phase 4:  T10 ─→ T11
```

11 tasks, 4 phases. Single batch (≤ ~8 tasks per worker is the budget; 11 is
one over — but phases 1-2 are the heavy ones, 3-4 are mechanical. Fits a
single inline execution.)

---

## Task Granularity Check

| Task | Scope | Status |
|------|-------|--------|
| T1: Tolerances/Policy structs | 1 struct rearrangement | ✅ |
| T2: Update callers | 1 mechanical sweep | ✅ |
| T3: Prelude export | 1 file | ✅ |
| T4: NewtonStrategy trait | 1 trait + 1 impl | ✅ |
| T5: Wire NewtonRaphsonSolver | 1 signature change + callers | ✅ |
| T6: ConvergencePlan owns Newton | 1 struct field + DC driver | ✅ |
| T7: StepperStrategy trait | 1 trait + 1 impl | ✅ |
| T8: AnalysisContext enum | 1 enum + structs | ✅ |
| T9: Wire TransientSolver | 1 driver rewrite | ✅ |
| T10: SignalBridge | 1 struct extraction | ✅ |
| T11: Cleanup | prelude + docs | ✅ |

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram Shows | Status |
|------|-------------------|---------------|--------|
| T1 | None | Phase 1 start | ✅ |
| T2 | T1 | T1→T2 | ✅ |
| T3 | T2 | T2→T3 | ✅ |
| T4 | T2 | T2→T4 (phase boundary) | ✅ |
| T5 | T4 | T4→T5 | ✅ |
| T6 | T5 | T5→T6 | ✅ |
| T7 | T6 | T6→T7 (phase boundary) | ✅ |
| T8 | T7 | T7→T8 | ✅ |
| T9 | T8 | T8→T9 | ✅ |
| T10 | T9 | T9→T10 (phase boundary) | ✅ |
| T11 | T10 | T10→T11 | ✅ |

---

## Test Co-location Validation

| Task | Code Layer | Matrix Requires | Task Says | Status |
|------|-----------|----------------|-----------|--------|
| T1 | solver/mod.rs | none (struct only) | none | ✅ |
| T2 | all drivers | integration | integration | ✅ |
| T3 | prelude | none | none | ✅ |
| T4 | convergence.rs | unit | unit | ✅ |
| T5 | newton_raphson + drivers | integration | integration | ✅ |
| T6 | convergence + dc | integration | integration | ✅ |
| T7 | convergence.rs | unit | unit | ✅ |
| T8 | analysis/* | integration | integration | ✅ |
| T9 | transient.rs | integration | integration | ✅ |
| T10 | circuit.rs | integration | integration | ✅ |
| T11 | prelude + docs | none | none | ✅ |
