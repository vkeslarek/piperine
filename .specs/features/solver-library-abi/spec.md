# Solver Library ABI / Prelude

**Implements:** MD-06 (init_global as Once), MD-13 (Rust idiom rules)
**SOLVER_GAPS reference:** §1 Phase 5, §7.5 (layout split), §7.7 (as_iv, Integrator)

## Problem

There is no `Circuit` builder or `Solver::build()` — hosts must assemble
`CircuitInstance` manually. Internals are not crate-private (glob reexports
removed but modules still `pub`). `init_global` is not wired to `Solver::build`.
`digital/scheduler.rs` mixes topology, state, and scheduler in one file.

## Goals

- `Circuit` builder: `Circuit::new()`, `add_element()`, `connect()`, `build()`
- `Solver::build()` triggers `init_global` (MD-06)
- Internals become crate-private; `prelude` is the only public surface
- `digital/scheduler.rs` split into `topology.rs`, `state.rs`, `scheduler.rs`
- `DcAnalysisResult::as_iv` no longer takes `&Netlist` directly

## Out of Scope

| Feature | Reason |
|---------|--------|
| Strategy traits | `solver-strategy-composition` |
| OSDI metadata | `solver-osdi-abi-completion` |

---

## Acceptance Criteria

1. WHEN a host writes `use piperine_solver::prelude::*;` THEN they SHALL be able to build, run, and query a circuit without reaching into internal modules
2. WHEN `Solver::build()` is called THEN `init_global` SHALL run exactly once
3. WHEN `digital/scheduler.rs` is examined THEN it SHALL contain only the scheduler loop, not `DigitalTopology` or `DigitalState`
4. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| LIB-01 | AC1 — prelude is complete public surface | Pending |
| LIB-02 | AC2 — Solver::build triggers init | Pending |
| LIB-03 | AC3 — scheduler split | Pending |
| LIB-04 | AC4 — tests green | Pending |

**Coverage:** 4 total, 0 mapped to tasks ⚠️
