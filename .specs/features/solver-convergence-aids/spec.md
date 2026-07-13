# Solver Convergence Aids

**SOLVER_GAPS reference:** §2 gshunt, §2 damping/fetlim/limvds

## Problem

Models add `gmin·v` at their own junctions, but there is no circuit-wide
`gshunt` option. `apply_damping` halves the whole update vector past 0.5 V;
ngspice also has per-device voltage limiting (`fetlim`/`limvds`). pnjlim is
in (`$limit`); `fetlim`/`DEVlimvds` are identity.

## Goals

- Circuit-wide `gshunt` option (diagonal GMIN the user can raise)
- `fetlim`/`limvds` limiters for tight ngspice parity

## Acceptance Criteria

1. WHEN `gshunt` is set THEN every node SHALL receive an added conductance to ground
2. WHEN a MOS device runs in transient THEN `fetlim` SHALL clamp gate-source voltage excursions
3. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| CONV-01 | AC1 — gshunt | Pending |
| CONV-02 | AC2 — fetlim | Pending |
| CONV-03 | AC3 — tests green | Pending |

**Coverage:** 3 total, 0 mapped to tasks ⚠️
