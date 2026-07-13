# Solver Performance

**SOLVER_GAPS reference:** §5 device bypass, §5 matrix reuse, §5 predictor

## Problem

piperine rebuilds the linear system each Newton iteration (`L::new` +
`apply_stamps`). ngspice reuses the symbolic factorization (KLU) and only
refactors numerically. No device bypass (skip re-evaluating a nonlinear
device whose terminal voltages barely changed). No predictor for the
transient initial guess.

## Goals

- Device bypass: per-element "inputs unchanged within tol → reuse last stamps"
- Matrix reuse: confirm faer's symbolic reuse is exploited; avoid full
  `L::new` per iteration
- Predictor: extrapolate next timepoint from history as Newton seed

## Acceptance Criteria

1. WHEN a nonlinear device's terminals barely changed THEN the solver SHALL skip re-evaluation and reuse last stamps
2. WHEN the symbolic factorization is available THEN the solver SHALL NOT rebuild it per Newton iteration
3. WHEN a transient step starts THEN the Newton seed SHALL be extrapolated from history
4. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| PERF-01 | AC1 — device bypass | Pending |
| PERF-02 | AC2 — matrix reuse | Pending |
| PERF-03 | AC3 — predictor | Pending |
| PERF-04 | AC4 — tests green | Pending |

**Coverage:** 4 total, 0 mapped to tasks ⚠️
