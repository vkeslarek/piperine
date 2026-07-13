# Solver Library ABI / Prelude

**Implements:** MD-06 (init_global as Once), MD-13 (Rust idiom rules 2+4)
**SOLVER_GAPS reference:** §1 Phase 5, §7.5 (layout split), §7.7 (as_iv, Integrator)

## Problem Statement

A host library user today must assemble `CircuitInstance` manually — there is
no builder pattern. The `Context::init_global` must be called explicitly before
any analysis. All crate-internal modules are `pub mod` in `lib.rs`, leaking
implementation details to downstream crates. `digital/scheduler.rs` is 395
lines mixing `DigitalTopology` (DFS-based DAG builder), `DigitalState`
(event queue + checkpoint/rollback), and the two evaluation loops
(`evaluate_until_stable`, `evaluate_dag_ordered`) in one file — violating
MD-13 rule 4 (modules organized by system function). `DcAnalysisResult::as_iv`
still takes `&Netlist`, coupling the analysis type to the netlist structure.

## Goals

- [ ] `Circuit` builder: a safe, discoverable builder for circuit assembly
- [ ] `Solver` struct: `Solver::build()` triggers `Context::init_global` exactly once
- [ ] Internals crate-private: only `prelude` and `lib` are the public API surface
- [ ] `digital/scheduler.rs` split into `topology.rs`, `state.rs`, `scheduler.rs`
- [ ] `DcAnalysisResult::as_iv` takes `&CircuitInstance` or `&SolverContext` instead of `&Netlist`

## Out of Scope

| Feature | Reason |
|---------|--------|
| Strategy traits | `solver-strategy-composition` |
| OSDI metadata | `solver-osdi-abi-completion` |
| Integrator trait | `solver-performance` |

---

## Assumptions & Open Questions

| Assumption | Chosen default | Rationale | Confirmed? |
|------------|---------------|-----------|------------|
| `Circuit` builder adds analog elements first, then wires them | `add_analog_node`, `add_element`, `connect`, `build()` | Mirrors SPICE netlist building | n |
| `Solver` owns `ConvergencePlan` by default | `Solver::new(circuit).with_plan(plan).build()` | Builder pattern; plan is optional | n |
| `as_iv` target type | Takes `&CircuitInstance` (has both netlist + context) | Most natural for callers (transient already has circuit) | n |

---

## User Stories

### P1: `Solver::build()` triggers init_global ⭐ MVP

**User Story**: As a host library user, I want `Solver::build()` to automatically
initialize tracing and faer parallelism so that I never need to call
`Context::init_global()` manually.

**Acceptance Criteria**:

1. WHEN `Solver::build()` is called for the first time THEN it SHALL call `Context::init_global()` exactly once
2. WHEN `Solver::build()` is called a second time THEN `init_global` SHALL NOT run again
3. WHEN the host calls `Solver::new(circuit).build()` THEN the solver SHALL be ready to run analyses

**Independent Test**: Construct two solvers from two circuits, verify the second doesn't panic.

---

### P2: `prelude` is the only public surface

**User Story**: As a host library user, I want `piperine_solver::prelude::*` to
contain everything I need to build circuits, configure analyses, run simulations,
and read results — without needing to know internal module paths.

**Acceptance Criteria**:

1. WHEN a host writes `use piperine_solver::prelude::*;` THEN they SHALL be able to call `circuit.dc(ctx)` / `circuit.tran(opts, ctx)` / `circuit.ac(ctx)` / `circuit.noise(opts, ctx)` / `circuit.tf(opts, ctx)`
2. WHEN a host writes `use piperine_solver::prelude::*;` THEN they SHALL be able to read results without importing from `analysis::*` or `solver::*` directly
3. WHEN a downstream crate uses `use piperine_solver::*;` (not prelude) THEN it SHALL NOT compile — internals are no longer `pub`

**Independent Test**: Write a smoke test that only imports `prelude` and constructs a circuit, runs DC, reads a voltage.

---

### P3: `scheduler.rs` split by system function

**User Story**: As a solver developer, I want `DigitalTopology`, `DigitalState`,
and the scheduler evaluation loops in separate files so that I can find each
concept at a glance.

**Acceptance Criteria**:

1. WHEN `digital/scheduler.rs` is opened THEN it SHALL contain only the evaluation loops (`evaluate_until_stable`, `evaluate_dag_ordered`) and be <200 lines
2. WHEN `digital/topology.rs` is opened THEN it SHALL contain `DigitalTopology` and its `build` method
3. WHEN `digital/state.rs` is opened THEN it SHALL contain `DigitalState` and its methods

**Independent Test**: Each file compiles independently; imports are clean.

---

### P4: `DcAnalysisResult::as_iv` decoupled from `Netlist`

**User Story**: As a solver developer, I want analysis types to not depend on
`Netlist` directly so that the analysis layer is purely about results and options.

**Acceptance Criteria**:

1. WHEN `as_iv` is called THEN it SHALL take `&CircuitInstance` (or a smaller context type) — not `&Netlist`
2. WHEN the analysis layer is examined THEN it SHALL not import `crate::analog::Netlist` directly

---

### P5: `Circuit` builder

**User Story**: As a host library user, I want a builder pattern for circuit
assembly so that I can create circuits without calling `Netlist::connect_node`
manually.

**Acceptance Criteria**:

1. WHEN `CircuitBuilder::new("top")` is called THEN it SHALL return a builder with an empty netlist, no devices, no ground
2. WHEN `.add_ground()` is called THEN the builder SHALL allocate a ground node
3. WHEN `.add_node("out")` is called THEN the builder SHALL allocate an analog node and return its `NodeIdentifier`
4. WHEN `.add_element(name, element)` is called THEN the builder SHALL store the element and its terminal bindings
5. WHEN `.build()` is called THEN the builder SHALL return a `CircuitInstance` with digital topology prebuilt and digital devices initialized

---

## Edge Cases

- WHEN `build()` is called without `add_ground()` THEN the circuit SHALL still be valid (ground is optional for pure-digital circuits)
- WHEN the scheduler split is done THEN all existing tests in `digital_topology.rs` and `mixed_signal.rs` SHALL pass without modification
- WHEN `as_iv` signature changes THEN the transient solver (`compute_initial_conditions`) SHALL continue to work

---

## Requirement Traceability

| ID | Story | AC | Status |
|----|-------|----|--------|
| LIB-01 | P1 | AC1-AC3 — Solver::build triggers init | Pending |
| LIB-02 | P2 | AC1-AC3 — prelude is complete | Pending |
| LIB-03 | P3 | AC1-AC3 — scheduler split | Pending |
| LIB-04 | P4 | AC1-AC2 — as_iv decoupled | Pending |
| LIB-05 | P5 | AC1-AC5 — Circuit builder | Pending |
| LIB-06 | — | `cargo test --workspace` green | Pending |

**Coverage:** 6 total, 0 mapped to tasks ⚠️

---

## Success Criteria

- [ ] `use piperine_solver::prelude::*;` is the only import a host needs
- [ ] `lib.rs` only `pub mod` is `prelude` and select re-exports
- [ ] `cargo build --workspace` zero warnings
- [ ] `cargo test --workspace` 51 targets green
