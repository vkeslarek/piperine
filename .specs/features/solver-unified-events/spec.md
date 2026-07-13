# Solver Unified Event Model

**Implements:** MD-12 (ABI vs policy)
**SOLVER_GAPS reference:** §1 unified event model

## Problem

Digital events, analog crossing events, timer events, breakpoints, and
`$bound_step`/step-limit hints are related scheduling constraints but live
in different places. The solver has no single queue/planner that decides:
next transient endpoint, zero-delay digital delta pending, analog
discontinuity forcing a step boundary, which elements need evaluation.

## Goals

- One event/breakpoint abstraction: kind, target signal, time, priority,
  source element, rollback behavior
- Digital value changes = one event kind; analog breakpoints and crossing
  guards = others
- Single queue/planner drives: next transient endpoint, digital delta,
  analog discontinuity, element evaluation set

## Acceptance Criteria

1. WHEN a digital event and an analog breakpoint are both pending THEN the planner SHALL pick the earliest across both kinds
2. WHEN a zero-delay digital delta is pending THEN the planner SHALL process it before advancing time
3. WHEN an analog discontinuity is declared THEN the planner SHALL force a step boundary
4. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| EVNT-01 | AC1 — cross-kind earliest | Pending |
| EVNT-02 | AC2 — zero-delay delta | Pending |
| EVNT-03 | AC3 — discontinuity boundary | Pending |
| EVNT-04 | AC4 — tests green | Pending |

**Coverage:** 4 total, 0 mapped to tasks ⚠️
