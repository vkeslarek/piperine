# Solver Convergence & Performance Specification

**Merges:** `solver-convergence-aids`, `solver-performance`,
`solver-strategy-composition` (all subsumed — their standalone specs are
deleted after this feature lands).
**Audit basis:** solver crate audit (2026-07-15) — every requirement traces to
a concrete `file:line` finding.

## Problem Statement

The solver has three critical holes discovered by code audit:

1. **User tolerances are silently ignored.** Every DC and transient solve
   hardcodes `Policy::default()` (`dc_damp_tolerance=0.5`, `max_iter=500`)
   regardless of what `Context` carries (`dc.rs:101,185,222`;
   `transient.rs:84,206`). The user cannot tune convergence.

2. **The linear system is rebuilt every Newton iteration.** `reset()` exists,
   is tested, and its docstring says "call this instead of reconstructing" —
   but it's never called (`newton_raphson.rs:154,266`). Two heap Vecs per
   iteration (`residual`, `scale`) amplify the waste.

3. **No device bypass or convergence hints.** Every device is re-evaluated
   every iteration even if its terminals barely changed (`BYPASS_OK` capability
   declared, never consulted). Damping is a single global L2 midpoint with no
   per-element feedback. The diode clipper converges but slowly.

## Goals

- [ ] User-facing tolerances (`reltol`, `abstol`, `max_iter`, `dc_damp_tolerance`)
      actually reach the Newton loop — no silent `Policy::default()`.
- [ ] Zero per-iteration heap allocations in the Newton inner loop (matrix
      reuse via `reset()`, hoisted work vectors).
- [ ] Device bypass: skip re-evaluation of nonlinear devices whose terminal
      voltages are unchanged within tolerance (`BYPASS_OK`).
- [ ] Per-element convergence hints: evolve `limiting_active() -> bool` into a
      structured hint the solver can act on.
- [ ] Newton predictor in transient: seed `x̂ₙ₊₁` from `(xₙ₋₁, xₙ)`.
- [ ] MD-13 compliance: no free `pub(crate) fn`; `Tolerances`/`Policy` split;
      dead `alpha` parameter removed.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Commit/rollback lifecycle hooks | Separate feature `solver-commit-rollback` |
| OSDI-inspired ABI metadata | Separate feature `solver-osdi-abi-completion` |
| Circuit builder / prelude / scheduler split | Separate feature `solver-library-abi` |
| Parallel element evaluation (rayon) | Follow-up after bypass + reuse land |
| Line search / trust region | P3 — deferred unless bypass + hints are insufficient |
| `gshunt` (circuit-wide diagonal) | Rolled in as P2; small scope |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| `BYPASS_OK` threshold | `reltol · \|V\| + abstol` per terminal | SPICE-standard bypass criterion | y |
| Convergence hint shape | `Option<(NetRef, f64)>` — "limit this net to this value" | Minimal evolution of `limiting_active`; device says what + where | y |
| Predictor order | First-order linear extrapolation `x̂ = xₙ + (xₙ − xₙ₋₁) · dtₙ₊₁/dtₙ` | Simplest meaningful predictor; ngspice default | y |
| `gshunt` default | `0.0` (off — no behavioral change unless user raises it) | Backward-compatible | y |
| Strategy trait extraction scope | Extract `NewtonStrategy` + `StepperStrategy` traits but keep the existing `DampedNewton` + PI controller as the sole impls | Traits for composition; no new algorithms this feature | y |

**Open questions:** none — all resolved above.

---

## User Stories

### P1: Solver statistics + tolerances reach the solver + zero-alloc Newton ⭐ MVP

**User Story**: As a circuit designer and solver developer, I want the solver
to report per-analysis statistics (Newton iterations, step rejections, bypass
hits, timing), I want my tolerance settings to actually reach the Newton loop,
and I want zero per-iteration allocations, so that I can diagnose convergence
problems, tune the solver, and run fast.

**Why P1**: Statistics are the measuring stick for every other requirement in
this feature. Without iteration counts and bypass hits, "30% fewer iterations"
is unprovable. Tolerances and zero-alloc are the two critical bugs (C1, P1).

**Acceptance Criteria**:

1. WHEN a DC analysis completes THEN `DcAnalysisResult` SHALL carry a
   `SolverStats` struct reporting: `newton_iterations`, `converged`,
   `homotopy_strategy` (if used), `homotopy_levels`, and wall-clock
   `assembly_time_ns` + `solve_time_ns`.
2. WHEN a transient analysis completes THEN `TransientAnalysisResult` SHALL
   carry a `SolverStats` reporting: `steps_accepted`, `steps_rejected`,
   `dt_min_floor_hits`, `dt_min`, `dt_max`, plus the DC stats for the initial
   operating point.
3. WHEN the solver bypasses a device (P2 feature) THEN `SolverStats` SHALL
   count `bypass_hits` and `bypass_misses` per analysis.
4. WHEN `Context` carries `dc_damp_tolerance=0.3` THEN the Newton midpoint
   damping SHALL use `0.3`, not the hardcoded `0.5`.
5. WHEN `Context` carries `max_iter=50` THEN the Newton loop SHALL stop after
   50 iterations, not 500.
6. WHEN the Newton loop runs 100 iterations THEN zero heap allocations SHALL
   occur after the first iteration (matrix via `reset()`, work vectors hoisted).
7. WHEN `DcSystem::apply_limit` or `TransientSystem::apply_limit` is searched
   for THEN neither SHALL exist (dead code deleted).
8. WHEN a transient step is LTE-rejected AND `dt` is at `dt_min` THEN the
   solver SHALL accept the step, increment `dt_min_floor_hits`, AND emit a
   `tracing::warn!` documenting the accuracy concession.
9. WHEN the Python binding reads `op.stats` THEN it SHALL expose every
   `SolverStats` field as a typed attribute (e.g.,
   `op.stats.newton_iterations`, `trace.stats.steps_rejected`).
10. WHEN `cargo test --workspace` runs THEN all targets SHALL pass and the
    existing example circuits SHALL produce identical DC operating points
    (±1e-9).

**Independent Test**: Run the diode clipper; print `op.stats.newton_iterations`
— it SHALL be a positive integer. Set `max_iter=3`; confirm non-convergence
after exactly 3. Set `max_iter=200`; confirm convergence + the stat shows < 200.

---

### P2: Device bypass + convergence hints + per-device LTE ⭐ MVP

**User Story**: As a circuit designer working with large circuits, I want the
solver to skip re-evaluating devices whose terminals haven't moved, and to
listen to per-device convergence hints, so that each Newton iteration is fast
and nonlinear devices (diodes, MOSFETs) converge in fewer iterations.

**Why P2**: The diode clipper takes many iterations because every device is
re-evaluated every time and the global damping can't target the specific
junction that's oscillating.

**Acceptance Criteria**:

7. WHEN a device declares `BYPASS_OK` AND its terminal voltages changed by
   less than `reltol·\|V\| + abstol` since last evaluation THEN the solver
   SHALL reuse its previous stamps and skip `evaluate`.
8. WHEN a device's `convergence_hint` returns `Some((net, limited_value))`
   THEN the solver SHALL apply that limit to the corresponding unknown before
   the convergence test (replacing the boolean-only `limiting_active` gate).
9. WHEN a reactive device's `suggest_transient_step` returns a dt floor smaller
   than the PI controller's proposal THEN the stepper SHALL shrink `dt` to the
   device's floor.
10. WHEN `gshunt > 0.0` is set THEN every analog node SHALL receive an added
    diagonal conductance of that value in the Jacobian.
11. WHEN `cargo test --workspace` runs THEN all targets SHALL pass.

**Independent Test**: Run the diode clipper with bypass enabled; count Newton
iterations — it SHALL be fewer than without bypass for the same circuit.

---

### P3: Newton predictor + MD-13 architecture cleanup

**User Story**: As a solver developer, I want the transient Newton seed to be
extrapolated from history (predictor), and I want the convergence/performance
machinery organized as strategy traits with no free functions (MD-13), so that
adding new algorithms is clean and the code is maintainable.

**Why P3**: Predictor is a performance win (fewer iterations per step). The
MD-13 cleanup (Tolerances/Policy split, trait extraction, dead `alpha` removal)
is architecture debt that compounds with every new feature.

**Acceptance Criteria**:

12. WHEN a transient step begins THEN the Newton seed SHALL be the first-order
    predictor `x̂ = xₙ + (xₙ − xₙ₋₁) · dtₙ₊₁/dtₙ` (falling back to `xₙ` when
    no history is available).
13. WHEN `Context::default()` is constructed THEN it SHALL contain a `Tolerances`
    sub-struct (immutable, `Copy`) and NO mutable policy fields (`max_iter`,
    `dc_damp_tolerance` move to `Policy`).
14. WHEN `NonLinearSystem::assemble` is called THEN it SHALL NOT receive the
    dead `alpha` parameter.
15. WHEN `solver/mod.rs` is examined THEN there SHALL be no free `pub(crate) fn`
    — `check_convergence`/`residual_converged`/`apply_damping` are methods on
    their owning trait or struct.
16. WHEN `cargo test --workspace` runs THEN all targets SHALL pass and DC/tran
    results SHALL be identical to pre-refactor (±1e-9).

**Independent Test**: Run a transient on the RC lowpass; compare iteration
counts per step with and without the predictor — predictor SHALL reduce the
average.

---

## Edge Cases

- WHEN a device is bypassed for many iterations THEN its stamps SHALL be
  re-evaluated at least once per accepted timestep (stale stamps across steps
  would be wrong).
- WHEN `gshunt` is very large (1e-3 S) THEN the DC operating point SHALL shift
  noticeably (the diagonal loads every node) — documented, not a bug.
- WHEN the predictor overshoots on a stiff circuit THEN Newton SHALL still
  converge (the predictor is a seed, not a constraint; damping + homotopy
  remain the safety net).
- WHEN no history exists (first step, or after a breakpoint) THEN the predictor
  SHALL fall back to the previous accepted point (no extrapolation).

---

## Requirement Traceability

| ID | Story | Status |
|----|-------|--------|
| CP-01 | P1 AC1 — DC SolverStats (iterations, homotopy, timing) | Pending |
| CP-02 | P1 AC2 — Transient SolverStats (steps, rejections, dt range) | Pending |
| CP-03 | P1 AC3 — bypass hit/miss counters | Pending |
| CP-04 | P1 AC4 — dc_damp_tolerance reaches Newton | Pending |
| CP-05 | P1 AC5 — max_iter reaches Newton | Pending |
| CP-06 | P1 AC6 — zero per-iter heap allocs | Pending |
| CP-07 | P1 AC7 — dead apply_limit deleted | Pending |
| CP-08 | P1 AC8 — dt_min floor hit counted + warned | Pending |
| CP-09 | P1 AC9 — Python exposes SolverStats | Pending |
| CP-10 | P1 AC10 — tests green + identical results | Pending |
| CP-11 | P2 AC7 — device bypass (BYPASS_OK) | Pending |
| CP-12 | P2 AC8 — convergence_hint replaces boolean | Pending |
| CP-13 | P2 AC9 — suggest_transient_step consulted | Pending |
| CP-14 | P2 AC10 — gshunt diagonal | Pending |
| CP-15 | P2 AC11 — tests green | Pending |
| CP-16 | P3 AC12 — Newton predictor | Pending |
| CP-17 | P3 AC13 — Tolerances/Policy split | Pending |
| CP-18 | P3 AC14 — dead alpha removed | Pending |
| CP-19 | P3 AC15 — no free fns (MD-13) | Pending |
| CP-20 | P3 AC16 — tests green + identical results | Pending |

**Coverage:** 20 total, 0 mapped to tasks ⚠️

**Status values:** Pending → In Design → In Tasks → Implementing → Verified

---

## Success Criteria

- [ ] `op.stats.newton_iterations` and `trace.stats.steps_rejected` return
      meaningful integers from Python (the measuring stick).
- [ ] User-set `max_iter` / `dc_damp_tolerance` / `reltol` / `abstol` reach the
      Newton loop (provable by setting extreme values and observing behavior).
- [ ] Zero heap allocations per Newton iteration after the first.
- [ ] Device bypass reduces Newton iterations on the diode clipper by ≥30%
      (measurable via `op.stats.newton_iterations` before/after).
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` green;
      21/21 `examples/*.py` pass unchanged.
- [ ] No free `pub(crate) fn` in `solver/mod.rs` (MD-13 rule 2).
