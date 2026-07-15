# Solver Convergence Aids

**SOLVER_GAPS reference:** §2 gshunt

## Problem

Models add `gmin·v` at their own junctions, but there is no circuit-wide
`gshunt` option — a user-raisable diagonal conductance to ground on every
node. This helps topologies that are otherwise floating or poorly damped
converge. Per-device voltage limiters (`pnjlim`/`fetlim`/`limvds`) are NOT
solver aids — they are device-internal knowledge (MD-12: element "knows"),
implemented inside each model's `evaluate`. `pnjlim` already exists as
`$limit`; `fetlim`/`limvds` will land when a MOS model is written.

## Goals

- Circuit-wide `gshunt` option (diagonal GMIN the user can raise per-analysis)

## Acceptance Criteria

1. WHEN `gshunt` is set THEN every analog node SHALL receive an added
   conductance to ground of that value
2. WHEN `gshunt` is left at default (0) THEN the solver SHALL behave as today
3. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| CONV-01 | AC1 — gshunt adds diagonal | Pending |
| CONV-02 | AC2 — default unchanged | Pending |
| CONV-03 | AC3 — tests green | Pending |

**Coverage:** 3 total, 0 mapped to tasks ⚠️
