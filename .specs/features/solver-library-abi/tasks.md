# Solver Library ABI / Prelude — Tasks

**Design:** `.specs/features/solver-library-abi/design.md`
**Status:** Draft

---

## Test Coverage Matrix

> Generated from codebase. Guidelines found: `AGENTS.md` (zero warnings, `cargo test --workspace`).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|------------|-------------------|---------------------|-------------------|-------------|
| Solver digital (scheduler/topology/state) | unit | All branches; existing digital_topology tests pass | `tests/digital_topology.rs` | `cargo test -p piperine-solver` |
| Solver analysis (dc, transient) | integration | All analyses run; results match baseline | `tests/mixed_signal.rs` | `cargo test -p piperine-solver` |
| Codegen device | integration | Compiled PHDL devices still stamp correctly | `tests/{from_ir,codegen_ir}.rs` | `cargo test -p piperine-codegen` |
| Bench e2e | e2e | Examples still pass | `tests/{bench,run_examples}.rs` | `cargo test -p piperine-bench` |
| CircuitBuilder / Solver | unit | Builder produces valid CircuitInstance | `src/solver/solve.rs` (inline) | `cargo test -p piperine-solver` |

## Gate Check Commands

| Gate Level | When to Use | Command |
|------------|-------------|---------|
| Quick | After digital-only changes | `cargo test -p piperine-solver` |
| Full | After codegen/bench import changes | `cargo test --workspace` |
| Build | After struct/signature changes | `cargo build --workspace` |

---

## Execution Plan

### Phase 1: Scheduler split (pure move, no behavior change)

```
T1 → T2 → T3
```

### Phase 2: Internals crate-private (import updates)

```
T4 → T5
```

### Phase 3: as_iv + Solver

```
T6 → T7
```

### Phase 4: CircuitBuilder

```
T8
```

---

## Task Breakdown

### T1: Extract `DigitalTopology` into `digital/topology.rs`

**What**: Move `DigitalTopology` struct + `build` method from `digital/scheduler.rs` to new `digital/topology.rs`.
**Where**: `crates/piperine-solver/src/digital/topology.rs` (NEW), `scheduler.rs` (MODIFY)
**Depends on**: None
**Requirement**: LIB-03

**Done when**:
- [ ] `digital/topology.rs` exists with `DigitalTopology` + `build` + `Default` (if needed)
- [ ] `digital/scheduler.rs` imports from `super::topology` instead of defining it
- [ ] `digital/mod.rs` declares `pub mod topology;`
- [ ] `cargo build -p piperine-solver` passes

**Tests**: none (pure move)
**Gate**: quick

---

### T2: Extract `DigitalState` lifecycle into `digital/state.rs`

**What**: Move `DigitalState` struct + `new`/`with_labels`/`set_label`/`label_or_default`/`schedule`/`peek_next_event_time`/`checkpoint`/`rollback`/`commit` from `scheduler.rs` to new `digital/state.rs`. Evaluation loops stay in `scheduler.rs` but become `pub(crate) fn` that take `&mut DigitalState`.

**Where**: `crates/piperine-solver/src/digital/state.rs` (NEW), `scheduler.rs` (MODIFY)
**Depends on**: T1
**Requirement**: LIB-03

**Done when**:
- [ ] `digital/state.rs` exists with `DigitalState` struct + lifecycle methods
- [ ] `digital/scheduler.rs` contains only `evaluate_until_stable` and `evaluate_dag_ordered` as `pub(crate) fn`
- [ ] `digital/scheduler.rs` is <200 lines
- [ ] `digital/mod.rs` declares `pub mod state;`
- [ ] `cargo test -p piperine-solver` passes (digital_topology tests still green)

**Tests**: unit (existing tests exercise the split methods)
**Gate**: quick

---

### T3: Clean `digital/` exports

**What**: Update `digital/mod.rs` re-exports to use the new split modules. Remove any dead re-exports.
**Where**: `crates/piperine-solver/src/digital/mod.rs`
**Depends on**: T2
**Requirement**: LIB-03

**Done when**:
- [ ] `digital/mod.rs` re-exports `DigitalTopology` from `topology`, state items from `state`, scheduler fns from `scheduler`
- [ ] No compilation warnings
- [ ] `cargo test -p piperine-solver` green

**Tests**: none
**Gate**: quick

---

### T4: Make solver internals `pub(crate)`

**What**: Change `lib.rs` from `pub mod` to `pub(crate) mod` for `analysis`, `analog`, `core`, `digital`, `error`, `math`, `result`, `solver`, `util`. Keep `pub mod prelude`. Add crate-root re-exports for what codegen needs.
**Where**: `crates/piperine-solver/src/lib.rs`
**Depends on**: T3
**Requirement**: LIB-02

**Done when**:
- [ ] `lib.rs` has `pub(crate) mod` for all internal modules
- [ ] `lib.rs` has `pub use prelude::*;` at crate root
- [ ] `cargo build -p piperine-solver` passes (no external callers break inside the crate)

**Tests**: none (crate-internal)
**Gate**: build

---

### T5: Update codegen/bench imports to public paths

**What**: Change codegen (`piperine-codegen`) and bench (`piperine-bench`) imports from internal paths like `piperine_solver::core::element::Element` to public paths like `piperine_solver::Element` or `piperine_solver::prelude::*`.
**Where**: `piperine-codegen/src/`, `piperine-bench/src/`
**Depends on**: T4
**Requirement**: LIB-02

**Done when**:
- [ ] No codegen or bench file imports from `piperine_solver::core::*`, `piperine_solver::solver::*`, `piperine_solver::math::*`, `piperine_solver::digital::*`, `piperine_solver::analog::*`, `piperine_solver::analysis::*` directly (only through prelude or crate root)
- [ ] `cargo build --workspace` passes, zero warnings
- [ ] `cargo test --workspace` green

**Tests**: integration + e2e
**Gate**: full

---

### T6: Decouple `as_iv` from `Netlist`

**What**: Change `DcAnalysisResult::as_iv(&self, netlist: &Netlist)` to `as_iv(&self, circuit: &CircuitInstance)`. Update the single callsite in `solver/transient.rs`.
**Where**: `analysis/dc.rs`, `solver/transient.rs`
**Depends on**: T5
**Requirement**: LIB-04

**Done when**:
- [ ] `DcAnalysisResult::as_iv` takes `&CircuitInstance`, reads `circuit.netlist()` internally
- [ ] `solver/transient.rs::compute_initial_conditions` passes `self.system.circuit` instead of `netlist`
- [ ] `cargo build --workspace` passes
- [ ] `cargo test --workspace` green

**Tests**: integration
**Gate**: full

---

### T7: Create `Solver` struct with `build()` triggering `init_global`

**What**: New `Solver` struct in `solver/solve.rs` (or `solver/mod.rs`). Wraps `CircuitInstance`, `Context`, `ConvergencePlan`. `build()` calls `Context::init_global()` once.
**Where**: `crates/piperine-solver/src/solver/solve.rs` (NEW), `prelude.rs`
**Depends on**: T6
**Requirement**: LIB-01

**Done when**:
- [ ] `Solver` struct has `new(circuit)`, `with_context`, `with_plan`, `with_tran_opts`, `build`
- [ ] `build()` calls `Context::init_global()` via `std::sync::Once`
- [ ] `Solver` has analysis methods: `dc()`, `tran()`, `ac()`, `noise()`, `tf()`
- [ ] `Solver` exported in prelude
- [ ] `cargo test --workspace` green

**Tests**: unit (smoke test: construct Solver, run DC, read result)
**Gate**: quick

---

### T8: Create `CircuitBuilder`

**What**: New `CircuitBuilder` in `core/builder.rs`. Accumulates nodes, elements, wires. `build()` produces `CircuitInstance` with topology prebuilt and digital initialized.
**Where**: `crates/piperine-solver/src/core/builder.rs` (NEW), `prelude.rs`
**Depends on**: T7
**Requirement**: LIB-05

**Done when**:
- [ ] `CircuitBuilder` has `new`, `add_ground`, `add_node`, `add_digital_net`, `add_element`, `node`
- [ ] `build()` calls `from_devices_and_netlist` + `rebuild_digital_topology` + `init_digital`
- [ ] Exported in prelude
- [ ] `cargo test --workspace` green

**Tests**: unit (smoke test: build circuit with resistor, run DC, verify voltage)
**Gate**: quick

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1:  T1 ──→ T2 ──→ T3
Phase 2:  T4 ──→ T5
Phase 3:  T6 ──→ T7
Phase 4:  T8
```

8 tasks, 4 phases. Fits a single inline execution.

---

## Task Granularity Check

| Task | Scope | Status |
|------|-------|--------|
| T1: Extract DigitalTopology | 1 struct move | ✅ |
| T2: Extract DigitalState | 1 struct move | ✅ |
| T3: Clean digital exports | 1 file | ✅ |
| T4: pub(crate) mod | 1 file | ✅ |
| T5: Update imports | mechanical sweep | ✅ |
| T6: as_iv decoupled | 1 signature change | ✅ |
| T7: Solver struct | 1 new file | ✅ |
| T8: CircuitBuilder | 1 new file | ✅ |

---

## Diagram-Definition Cross-Check

| Task | Depends On | Diagram Shows | Status |
|------|-----------|---------------|--------|
| T1 | None | Phase 1 start | ✅ |
| T2 | T1 | T1→T2 | ✅ |
| T3 | T2 | T2→T3 | ✅ |
| T4 | T3 | T3→T4 (phase boundary) | ✅ |
| T5 | T4 | T4→T5 | ✅ |
| T6 | T5 | T5→T6 (phase boundary) | ✅ |
| T7 | T6 | T6→T7 | ✅ |
| T8 | T7 | T7→T8 (phase boundary) | ✅ |

---

## Test Co-location Validation

| Task | Code Layer | Matrix Requires | Task Says | Status |
|------|-----------|----------------|-----------|--------|
| T1 | topology.rs | unit | none (structural) | ✅ |
| T2 | state.rs + scheduler.rs | unit | unit (existing tests) | ✅ |
| T3 | mod.rs | none | none | ✅ |
| T4 | lib.rs | none | none | ✅ |
| T5 | codegen + bench | integration + e2e | integration + e2e | ✅ |
| T6 | dc.rs + transient.rs | integration | integration | ✅ |
| T7 | solve.rs | unit | unit | ✅ |
| T8 | builder.rs | unit | unit | ✅ |
