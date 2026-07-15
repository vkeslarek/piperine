# Solver Strategy Composition

**Implements:** MD-03 (per-analysis context), MD-04 (Tolerances/Policy), MD-05 (strategy composition)
**SOLVER_GAPS reference:** §1 Phase 4, §7.3, §7.4 (alpha), §7.9 (SignalBridge)

## Problem

DC and transient drivers have inline logic for Newton damping, homotopy
escalation, mixed-signal settle, and timestep selection. The homotopy part
is already a `ConvergencePlan` with `HomotopyStrategy` traits, but Newton
damping/limiting and transient step rejection are still inline. `Context`
still carries flat mutable fields (`max_iter`, `dc_damp_tolerance`, `time`)
that belong on a `Tolerances`/`Policy` split. The dead `alpha` parameter on
`NonLinearSystem::assemble` is still passed and ignored. Three free functions
in `solver/mod.rs` (`check_convergence`, `residual_converged`,
`apply_damping`) violate MD-13 rule 1 (every method has an owner).

## What's already done

| Component | Status | Where |
|-----------|--------|-------|
| `HomotopyStrategy` trait | ✅ Done | `solver/convergence.rs` |
| `HomotopyDriver` trait | ✅ Done | `solver/convergence.rs` |
| `ConvergencePlan` (compose + solve) | ✅ Done | `solver/convergence.rs` |
| `GminStepping` strategy | ✅ Done | `solver/convergence.rs` |
| `SourceStepping` strategy | ✅ Done | `solver/convergence.rs` |
| `PlanLimits` (caps extracted from literals) | ✅ Done | `solver/convergence.rs` |
| DC driver uses `plan.solve(self)` via `HomotopyDriver` | ✅ Done | `solver/dc.rs` |
| `gmin_extra` / `src_scale` out of `Context` | ✅ Done | `solver/mod.rs`, `solver/dc.rs` |
| LTE-driven timestep (`suggest_transient_step`) | ✅ Done | `solver/transient.rs`, `core/element.rs` |
| `dt_min`/`dt_max` from `TransientAnalysisOptions` (not literals) | ✅ Done | `solver/transient.rs` |
| PI controller always-adaptive (Milne LTE, asymmetric-diff exclusion) | ✅ Done | `solver/trbdf2.rs` (delivered by `solver-trbdf2-engine`) |
| TR-BDF2 two-phase sole scheme | ✅ Done | `solver/trbdf2.rs` |

## What's not done

| Component | Status | Where |
|-----------|--------|-------|
| `NewtonStrategy` trait (damping + convergence policy) | ❌ | `check_convergence`/`residual_converged`/`apply_damping` are free fns in `solver/mod.rs` |
| `StepperStrategy` trait (timestep accept/reject/grow) | ❌ | Inline in `TransientSolver::solve()` |
| `Tolerances` sub-struct (immutable, `Copy`) | ❌ | `Context` has flat fields |
| `Policy` sub-struct (mutable, owned by plan) | ❌ | `max_iter`, `dc_damp_tolerance` still on `Context` |
| `AnalysisContext` enum (`DcContext`, `AcContext`, …) | ❌ | Tunables split between `Context` and `*AnalysisOptions` |
| Dead `alpha` parameter on `NonLinearSystem::assemble` | ❌ | `newton_raphson.rs:13` |
| `SignalBridge` component | ❌ | `CircuitInstance::accept_and_run_digital` does 3 jobs inline |
| Free fns → trait/struct methods (MD-13 rule 1) | ❌ | `solver/mod.rs:18-77` |

## Goals

- `NewtonStrategy` trait replaces free functions; `ConvergencePlan` composes it
- `StepperStrategy` trait replaces inline transient logic
- `Context` split: `Tolerances` (immutable, `Copy`) + `Policy` (mutable, owned by plan)
- `AnalysisContext` enum carries per-analysis tunables
- Dead `alpha` parameter removed
- `SignalBridge` extracted from `CircuitInstance`

## Out of Scope

| Feature | Reason |
|---------|--------|
| OSDI-inspired ABI details | `solver-osdi-abi-completion` |
| Device bypass / matrix reuse | `solver-performance` |
| Niche analyses | On demand |

---

## Acceptance Criteria

1. WHEN `ConvergencePlan::default()` runs DC THEN results SHALL match today's
2. WHEN `ConvergencePlan::default()` runs transient THEN results SHALL match today's
3. WHEN `NonLinearSystem::assemble` is called THEN it SHALL NOT receive `alpha`
4. WHEN `Context::default()` is constructed THEN it SHALL contain a `Tolerances` sub-struct and NO mutable policy fields
5. WHEN a transient analysis is configured THEN `TransientContext` SHALL carry `dt_min`, `dt_max`, `adaptive`, `record_from`
6. WHEN `accept_and_run_digital` is called THEN a `SignalBridge` SHALL handle the buffer/seed/schedule logic
7. WHEN `solver/mod.rs` is examined THEN there SHALL be no free `pub(crate) fn` — every function has an owner (trait or struct)
8. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| STRAT-01 | AC1, AC2 — default plan reproduces results | Pending |
| STRAT-02 | AC3 — alpha removed | Pending |
| STRAT-03 | AC4 — Tolerances/Policy split | Pending |
| STRAT-04 | AC5 — AnalysisContext enum | Pending |
| STRAT-05 | AC6 — SignalBridge | Pending |
| STRAT-06 | AC7 — no free fns (MD-13) | Pending |
| STRAT-07 | AC8 — tests green | Pending |

**Coverage:** 7 total, 0 mapped to tasks ⚠️
