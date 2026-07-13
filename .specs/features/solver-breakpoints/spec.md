# Solver Breakpoints

**Implements:** MD-07 (IntegrationMethod in math/), MD-08 (LTE drives timestep)
**SOLVER_GAPS reference:** §3 breakpoints

## Problem

ngspice forces a timepoint exactly at every source discontinuity (pulse
edges, PWL corners) so the integrator never steps across a kink. Piperine
relies on adaptive stepping + `$bound_step` hints. The `BreakpointProvider`
trait exists in `math/integration.rs` but is never called.

## Goals

- Breakpoint table fed by source models (`BreakpointProvider::get_breakpoints`)
- Stepper clamps `t_next` to the next breakpoint
- Breakpoints survive rollback (they are absolute times, not state)

## Acceptance Criteria

1. WHEN a pulse source has a rising edge at t=10ns THEN the stepper SHALL land exactly at t=10ns
2. WHEN a PWL source has corners THEN the stepper SHALL land on each corner
3. WHEN no breakpoints exist THEN the stepper SHALL behave as today
4. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| BRK-01 | AC1 — pulse edge | Pending |
| BRK-02 | AC2 — PWL corners | Pending |
| BRK-03 | AC3 — no breakpoints, unchanged | Pending |
| BRK-04 | AC4 — tests green | Pending |

**Coverage:** 4 total, 0 mapped to tasks ⚠️
