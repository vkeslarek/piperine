# Solver ABI — Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill files
by filesystem path. The skill is the source of truth for the full flow
(per-task cycle, sub-agent delegation, adequacy review, Verifier,
discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Spec**: `.specs/features/solver-abi/spec.md`
**Design**: `.specs/features/solver-abi/design.md`
**Status**: Draft

**Baseline (2026-07-16, branch `feature/plugin-architecture`):**
`cargo build --workspace` zero warnings; `cargo test --workspace` green (391
tests); 21/21 `examples/*.py` pass. All file/line references below were
verified against this baseline. **If a line number has drifted, locate the
named item by symbol, not by line.**

**Reading order before T1:** open `spec.md` (acceptance criteria) and
`design.md` (exact struct definitions + the normative `abi.rs` re-export list
in Component 1, the `CircuitBuilder` build sequence in Component 7, the
lifecycle wiring in Component 8, the noise loop change in Component 10).
Every task below references the design section it implements — read that
section before editing.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec. Guidelines found:
> `AGENTS.md` (Test placement table + build/verify bar: zero warnings,
> `cargo test --workspace`), `CLAUDE.md`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Solver ABI public surface (`prelude`/`abi`) | integration | Both surfaces compile importing ONLY their module; `prelude` runs full host flow (build→run→read); `abi` implements a full `Element` | `crates/piperine-solver/tests/{prelude_surface,abi_surface}.rs` | `cargo test -p piperine-solver` |
| Solver core types (`Element` trait, `CircuitInstance`, `CircuitBuilder`, `UnknownAllocator`, `introspect`, `Noise`, `Solver`) | unit | 1:1 to spec ACs; every lifecycle/allocator/noise AC has a test; every listed edge case has a test | `crates/piperine-solver/src/**/*.rs` (`#[cfg(test)] mod tests`) + `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver` |
| Digital scheduler split | integration | Existing `digital_topology` + `mixed_signal` pass unchanged — assertions untouched, import adjustments only (ABI-05 AC4) | `crates/piperine-solver/tests/{digital_topology,mixed_signal}.rs` | `cargo test -p piperine-solver` |
| Codegen device capabilities (`PiperineDevice`) | unit | New flags declared + set correctly: `ANALYTIC_JACOBIAN` always; `STAMPS_CHARGE` iff kernel has charge | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Downstream migration (privacy flip) | integration (build gate) | Zero internal-path `piperine_solver::(core|math|digital|analog|analysis|solver|result|error)::` matches in codegen/bench/python/solver-tests; workspace builds zero warnings | grep gate (see Success Criteria) + `cargo build --workspace` | `cargo build --workspace` |
| Noise per-source reporting | unit + integration | Conservation: Σ per-source psd == total `out_noise_sq` per frequency ±1e-9 reltol; total PSD unchanged ±1e-12 vs baseline | `crates/piperine-solver/tests/*.rs` + Johnson-noise example | `cargo test -p piperine-solver` + examples |
| Examples (e2e) | e2e | 21/21 `examples/*.py` pass via `piperine run` (rebuild root binary first) | `examples/*.py` (driven by `piperine-bench/tests/run_examples.rs`) | `cargo build -p piperine` then run examples |

## Gate Check Commands

> Generated from codebase (`AGENTS.md` build/verify bar). Confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | After tasks touching only solver unit tests | `cargo test -p piperine-solver` |
| Codegen | After tasks touching `piperine-codegen` | `cargo build -p piperine-codegen && cargo test -p piperine-codegen` |
| Full | After migration / integration / noise tasks | `cargo build --workspace && cargo test --workspace` |
| Build | After struct/config-only tasks (no behavior) | `cargo build --workspace` (MUST be zero warnings) |
| Examples | After final task + Verifier | `cargo build -p piperine` then run 21 `examples/*.py` via `piperine run` (or `cargo test -p piperine-bench --test run_examples`) |
| Grep gate | After privacy flip (T6) + final (T15) | `grep -rn "piperine_solver::\(core\|math\|digital\|analog\|analysis\|solver\|result\|error\)::" crates/piperine-codegen crates/piperine-bench crates/piperine-python crates/piperine-solver/tests` → must be empty |

---

## Execution Plan

Phases are ordered and run sequentially — each phase completes before the next
begins, and tasks within a phase execute in order. The ordering is dependency-
driven: additive/non-breaking changes first, then the load-bearing privacy
flip, then the ABI additions that depend on it.

### Phase 1: Scheduler split (mechanical, no API change)

```
T1
```

### Phase 2: `abi` module (additive re-exports)

```
T2
```

### Phase 3: Downstream migration + privacy flip (load-bearing)

```
T3 → T4 → T5 → T6
```

### Phase 4: `as_iv` decoupling

```
T7
```

### Phase 5: Element lifecycle (setup/destroy)

```
T8
```

### Phase 6: CircuitBuilder + internal-unknown allocation

```
T9
```

### Phase 7: Rich terminal descriptors

```
T10
```

### Phase 8: Noise metadata + per-source reporting

```
T11 → T12
```

### Phase 9: Stamp-capability declaration

```
T13
```

### Phase 10: `Solver` entry point

```
T14
```

### Phase 11: Final verification

```
T15
```

---

## Task Breakdown

### T1: Split `digital/scheduler.rs` into `topology.rs` + `state.rs` + `scheduler.rs`

**What**: Split the 403-line `scheduler.rs` by system function (MD-13 rules 2+4).
Three files result; no free functions; evaluation loops stay methods on
`DigitalState` in a second `impl` block inside the (shrunk) `scheduler.rs`.

**Where**:
- NEW `crates/piperine-solver/src/digital/topology.rs`
- NEW `crates/piperine-solver/src/digital/state.rs`
- MODIFY `crates/piperine-solver/src/digital/scheduler.rs` (shrinks to <250 lines)
- MODIFY `crates/piperine-solver/src/digital/mod.rs` (re-export paths)

**Depends on**: None
**Reuses**: design.md Component 4 (§"Scheduler split")
**Requirement**: ABI-05 (AC1–AC4)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. Create `digital/topology.rs`. Move VERBATIM (no edits to bodies) from
   `scheduler.rs`: the `DigitalTopology` struct + its `impl DigitalTopology`
   block containing `build` (currently lines 19–95). Keep the imports it needs
   (`std::collections::{HashMap, HashSet}`, `std::cmp::Reverse`,
   `crate::digital::{LogicValue, DigitalNet, DigitalEvent}`,
   `crate::core::element::{Element, ElementCapabilities}` — keep only the ones
   `topology.rs` actually references; remove unused imports).
2. Create `digital/state.rs`. Move VERBATIM from `scheduler.rs`: the `Checkpoint`
   struct (currently lines 8–13) + the `DigitalState` struct + its data/lifecycle
   `impl DigitalState` block with methods `new`, `with_labels`, `set_label`,
   `label_or_default`, `schedule`, `peek_next_event_time`, `checkpoint`,
   `rollback`, `commit` (currently lines 101–183). Keep needed imports.
3. Shrink `digital/scheduler.rs` to contain ONLY a second
   `impl DigitalState { ... }` block holding `evaluate_until_stable` and
   `evaluate_dag_ordered` (currently lines 194–402). Add
   `use crate::digital::state::DigitalState;` at the top (the impl block needs
   the type in scope). Keep imports the two methods use
   (`HashSet`, `Reverse`, `EvalCtx`, `QueueSink`, `Element`, `ElementCapabilities`,
   `DigitalNet`, `DigitalEvent`, `LogicValue`). **No free functions** (MD-13
   rule 2). The file MUST end under 250 lines.
4. Update `digital/mod.rs`:
   ```rust
   pub mod events;
   pub mod interface;
   pub mod scheduler;
   pub mod state;
   pub mod topology;

   pub use events::{DigitalEvent, DigitalNet, LogicValue};
   pub use interface::{DigitalPorts, EvalCtx, EventSink, QueueSink};
   pub use state::DigitalState;
   pub use topology::DigitalTopology;
   ```
5. Grep the whole crate for `digital::scheduler::DigitalState` and
   `digital::scheduler::DigitalTopology` and rewrite each to
   `digital::state::DigitalState` / `digital::topology::DigitalTopology`
   (or to the `crate::digital::{DigitalState, DigitalTopology}` re-export).
   Expected sites: `crates/piperine-solver/src/core/circuit.rs`,
   `crates/piperine-solver/tests/digital_topology.rs`,
   `crates/piperine-solver/tests/mixed_signal.rs`. Adjust imports only — do
   NOT touch any test assertion (ABI-05 AC4).

**Done when**:
- [ ] `digital/topology.rs` contains exactly `DigitalTopology` + `build`
- [ ] `digital/state.rs` contains `Checkpoint` + `DigitalState` + its 9 data/lifecycle methods
- [ ] `digital/scheduler.rs` contains ONLY the `impl DigitalState` evaluation block, < 250 lines, no free functions
- [ ] `digital/mod.rs` re-exports `DigitalState` from `state` and `DigitalTopology` from `topology`
- [ ] `cargo build -p piperine-solver` succeeds with zero warnings
- [ ] `cargo test -p piperine-solver` passes; `digital_topology.rs` + `mixed_signal.rs` assertions are untouched
- [ ] Test count: ≥ 391 pass (no silent deletions)

**Tests**: integration (existing `digital_topology.rs` + `mixed_signal.rs` — import adjustments only)
**Gate**: Quick
**Commit**: `refactor(solver): split digital/scheduler.rs by system function (topology/state/scheduler)`

---

### T2: Create the `abi` module (additive re-exports)

**What**: Add `crates/piperine-solver/src/abi.rs` — the device-author surface,
pure re-exports, no new types. Declare it `pub mod abi;` in `lib.rs` (NO privacy
flip yet — that is T6). Add a smoke test that implements `Element` importing
ONLY `piperine_solver::abi::*`.

**Where**:
- NEW `crates/piperine-solver/src/abi.rs`
- MODIFY `crates/piperine-solver/src/lib.rs` (add one line: `pub mod abi;`)
- NEW `crates/piperine-solver/tests/abi_surface.rs`

**Depends on**: T1 (abi exports `digital::state::DigitalState` /
`digital::topology::DigitalTopology` — the split must land first so the paths exist)
**Reuses**: design.md Component 1 (normative re-export list)
**Requirement**: ABI-03

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. Create `src/abi.rs` with EXACTLY the re-export list from design.md Component 1.
   The list is normative — copy it verbatim. It exports:
   - Contract: `Element, ElementCapabilities, ConvergenceHint, CircuitInstance`
   - Introspection: `Bounds, Direction, Domain, Invalidation, ParamDescriptor,
     ParamError, ParamScope, QueryDescriptor, QueryKind, SignConvention,
     TerminalDescriptor, Value, ValueKind` (note: `SignConvention` lands in T10;
     until then this line will not compile — see step 2)
   - Stamping/naming: `AsIndex, Stamp, AnalogReference, AnalogVariable,
     BranchIdentifier, Netlist, NodeIdentifier, GND`
   - Solution history/states: `CircularArrayBuffer2, AcAnalysisContext,
     DcAnalysisResult, DcAnalysisState, Noise, TransientAnalysisContext,
     TransientAnalysisOptions, TransientAnalysisState`
   - Integration: `IntegrationMethod, TrBdf2, TrBdf2Phase, Second`
   - Digital: `DigitalPorts, EvalCtx, EventSink, QueueSink, DigitalEvent,
     DigitalNet, LogicValue, DigitalState, DigitalTopology`
   - Run config/results: `Context, Policy, Tolerances, Result, SolverStats,
     Error, SolverDomain`
   - Lifecycle allocator: `UnknownAllocator` (lands in T9)
2. **Forward-dependency handling**: `SignConvention` (T10) and
   `UnknownAllocator` (T9) do not exist yet. To keep T2 green, COMMENT OUT the
   two lines that re-export them (`SignConvention` in the introspection block;
   `UnknownAllocator` in the lifecycle block) with a `// TODO(T10):` and
   `// TODO(T9):` marker. T9 and T10 will uncomment their respective lines.
   Verify `GND`, `AnalogVariable`, `TrBdf2`, `TrBdf2Phase`, `Second`,
   `SolverStats`, `SolverDomain` exist (grep `pub struct`/`pub enum`/`pub use`
   for each); if any is missing, omit it and note it in the commit body — do
   NOT invent a type.
3. Add `pub mod abi;` to `lib.rs` (keep all other modules `pub mod` for now).
4. Create `tests/abi_surface.rs`. It imports ONLY `use piperine_solver::abi::*;`
   (no other `piperine_solver::` path). Define a `Resistor` test double
   (template: `core/introspect.rs` tests `Resistor`, lines 215–256) that:
   - declares `ANALOG | LOADS_DC`,
   - implements `name`, `capabilities`, `load_dc` (stamps a conductance between
     two `AnalogReference`s via `Stamp::Matrix`),
   - implements `list_params`/`get_param`/`set_param` (one param `r`).
   Add one `#[test] fn abi_compiles_an_element()` that constructs the double,
   checks `capabilities()` and `get_param`, and asserts `load_dc` returns one
   stamp. Build a minimal circuit with `CircuitInstance::from_devices_and_netlist`
   and run `circuit.dc(Context::default())?.solve()?` to prove the abi types
   thread through the solver.

**Done when**:
- [ ] `src/abi.rs` exists with the normative re-export list (T9/T10 items commented)
- [ ] `lib.rs` declares `pub mod abi;`
- [ ] `tests/abi_surface.rs` imports ONLY `piperine_solver::abi::*` and compiles + passes
- [ ] `cargo build --workspace` zero warnings
- [ ] Test count: ≥ 392 pass (391 baseline + 1 new)

**Tests**: integration (`tests/abi_surface.rs`)
**Gate**: Full
**Commit**: `feat(solver): add abi module — device-author re-export surface`

---

### T3: Migrate `piperine-codegen` imports to `piperine_solver::abi::*`

**What**: Grep-driven mechanical rewrite of every `piperine_solver::` internal
path in `piperine-codegen` to the `abi::` equivalent. Modules are still `pub`
(T6 flips them), so old paths still compile — this just switches to the new
paths. Build stays green throughout.

**Where**: `crates/piperine-codegen/src/**/*.rs`
**Depends on**: T2 (abi must exist with the re-exports)
**Reuses**: design.md "Downstream import inventory" table (codegen row — exact paths)
**Requirement**: ABI-04

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. Grep `crates/piperine-codegen/src` for `piperine_solver::`. The design
   inventory lists every path used today (e.g. `solver::Context`,
   `math::circular_array::CircularArrayBuffer2`, `math::linear::Stamp`,
   `math::integration::{TrBdf2, TrBdf2Phase}`,
   `digital::interface::{DigitalPorts, EvalCtx, EventSink}`,
   `digital::{DigitalNet, LogicValue}`, `digital::DigitalEvent`,
   `core::element::{Element, ElementCapabilities}`,
   `core::introspect::{…}`, `analysis::transient::{…}`,
   `analysis::noise::Noise`, `analysis::dc::{…}`,
   `analysis::ac::AcAnalysisContext`, `analog::{…}`,
   `core::circuit::CircuitInstance`).
2. For each match, replace the internal path with `piperine_solver::abi::…`
   (collapse to a single `use piperine_solver::abi::{…};` per file where
   practical). If an item is not yet in `abi` (e.g. `SignConvention`,
   `UnknownAllocator` — not used by codegen today), it stays un-exported; do
   not add it.
3. Run `cargo build -p piperine-codegen` — zero warnings. Then
   `cargo test -p piperine-codegen`.

**Done when**:
- [ ] `grep -rn "piperine_solver::\(core\|math\|digital\|analog\|analysis\|solver\|result\|error\)::" crates/piperine-codegen/src` returns ZERO matches
- [ ] `cargo build -p piperine-codegen` zero warnings
- [ ] `cargo test -p piperine-codegen` passes (no test count drop)

**Tests**: integration (codegen existing tests, untouched)
**Gate**: Codegen
**Commit**: `refactor(codegen): import solver surface via abi module`

---

### T4: Migrate `piperine-bench` imports to `prelude::*` / `abi::*`

**What**: Same mechanical migration for `piperine-bench`. Host-role imports go
to `prelude::*`; the few that touch `Netlist`/states go to `abi::*`.

**Where**: `crates/piperine-bench/src/**/*.rs`
**Depends on**: T2
**Reuses**: design.md "Downstream import inventory" table (bench row)
**Requirement**: ABI-02, ABI-04

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. Grep `crates/piperine-bench/src` for `piperine_solver::`.
2. Host-role items (`Context`, `Policy`, analysis results/options,
   `CircuitInstance`, `DigitalNet`, `LogicValue`,
   `DcAnalysisResult`, `TransientAnalysisResult`, `Netlist`, `NodeIdentifier`,
   `Error`) → `piperine_solver::prelude::{…}`.
3. Device-author items (anything touching `Element`, `Stamp`, states,
   `EvalCtx`) → `piperine_solver::abi::{…}`. Inspect each use to decide the
   right tier; when in doubt, `prelude` for host flow, `abi` for element
   implementation.
4. Build + test bench.

**Done when**:
- [ ] `grep -rn "piperine_solver::\(core\|math\|digital\|analog\|analysis\|solver\|result\|error\)::" crates/piperine-bench/src` returns ZERO matches
- [ ] `cargo build -p piperine-bench` zero warnings
- [ ] `cargo test -p piperine-bench` passes (incl. `bench.rs` + `run_examples.rs`)

**Tests**: integration (bench existing tests, untouched)
**Gate**: Full
**Commit**: `refactor(bench): import solver surface via prelude/abi`

---

### T5: Migrate `piperine-python` + `piperine-solver/tests/` imports to `abi`/`prelude`

**What**: Finish the downstream migration: `piperine-python` (one import:
`result::SolverStats` → `prelude::SolverStats`) and every file under
`crates/piperine-solver/tests/` (they are device-author harnesses → `abi::*`).

**Where**:
- `crates/piperine-python/src/**/*.rs`
- `crates/piperine-solver/tests/**/*.rs` (incl. `helpers/mod.rs`,
  `digital_topology.rs`, `mixed_signal.rs`)

**Depends on**: T2
**Reuses**: design.md "Downstream import inventory" table (python + solver-tests rows)
**Requirement**: ABI-02, ABI-03, ABI-04

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. Grep `crates/piperine-python/src` for `piperine_solver::result::SolverStats`
   → replace with `piperine_solver::prelude::SolverStats`.
2. Grep `crates/piperine-solver/tests/` for `piperine_solver::`. Replace every
   internal path (`analog::`, `analysis::`, `core::`, `digital::`, `math::`,
   `solver::`) with `piperine_solver::abi::{…}`. The test doubles in
   `helpers/mod.rs` use `Element`, `ElementCapabilities`, `LogicValue`,
   `DigitalNet`, `DigitalPorts`, `EvalCtx`, `EventSink` — all in `abi`.
   `digital_topology.rs` / `mixed_signal.rs` use
   `digital::scheduler::{DigitalState, DigitalTopology}` (after T1 these moved)
   → `piperine_solver::abi::{DigitalState, DigitalTopology}`.
3. Build + test. Test assertions stay untouched (only `use` lines change).

**Done when**:
- [ ] `grep -rn "piperine_solver::\(core\|math\|digital\|analog\|analysis\|solver\|result\|error\)::" crates/piperine-python/src crates/piperine-solver/tests` returns ZERO matches
- [ ] `cargo build --workspace` zero warnings
- [ ] `cargo test --workspace` passes (≥ 392)

**Tests**: integration (existing tests, import adjustments only)
**Gate**: Full
**Commit**: `refactor(python,solver): import solver surface via prelude/abi`

---

### T6: Flip `lib.rs` privacy + add `tests/prelude_surface.rs`

**What**: The load-bearing change. Flip every module except `prelude` and `abi`
to `pub(crate)`. Re-export `pub use prelude::*;` at the crate root. Add the
host-flow surface test importing ONLY `prelude::*`. After this, the grep gate
must be empty.

**Where**:
- MODIFY `crates/piperine-solver/src/lib.rs`
- MODIFY `crates/piperine-solver/src/prelude.rs` (add `CircuitBuilder` re-export
  IF T9 has landed; if not, skip — `CircuitBuilder` is not yet built. Add
  `Solver` re-export IF T14 has landed; skip otherwise. Keep everything else.)
- NEW `crates/piperine-solver/tests/prelude_surface.rs`

**Depends on**: T3, T4, T5 (all downstream consumers must already be off internal paths, or the flip breaks them)
**Reuses**: design.md Component 3 (lib.rs privacy flip)
**Requirement**: ABI-01 (AC1), ABI-02, ABI-04

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. Rewrite `lib.rs` to:
   ```rust
   pub mod abi;
   pub mod prelude;
   pub(crate) mod analysis;
   pub(crate) mod analog;
   pub(crate) mod core;
   pub(crate) mod digital;
   pub(crate) mod error;
   pub(crate) mod math;
   pub(crate) mod result;
   pub(crate) mod solver;

   pub use prelude::*;
   ```
2. Build the workspace. If a `private_interfaces` warning appears (a `pub` item
   in a `pub(crate)` module leaks a private type), add the leaked type to
   `abi.rs` or `prelude.rs` explicitly — do NOT re-open module privacy.
3. Create `tests/prelude_surface.rs`. It imports ONLY
   `use piperine_solver::prelude::*;`. It must compile code that: builds a
   circuit via `CircuitInstance::from_devices_and_netlist` (CircuitBuilder not
   yet available — use the existing constructor), constructs `Context`,
   runs `circuit.dc(Context::default())?.solve()?`, reads
   `DcAnalysisResult::get_net` (or `get_node`) + `SolverStats`, and references
   `Net`, `Error`. Use the `Resistor`-style double from `abi_surface.rs` copied
   inline (the test cannot import `abi`, so it re-declares a minimal double OR
   uses the existing test helpers — but helpers import `abi::` now; so the
   prelude test must define its own tiny double inline). One `#[test]` that
   builds a voltage divider and asserts the midpoint ±1e-9.

**Done when**:
- [ ] `lib.rs` has only `pub mod abi;` + `pub mod prelude;`; all others `pub(crate) mod`; `pub use prelude::*;` present
- [ ] `tests/prelude_surface.rs` imports ONLY `piperine_solver::prelude::*` and compiles + passes
- [ ] Grep gate empty: `grep -rn "piperine_solver::\(core\|math\|digital\|analog\|analysis\|solver\|result\|error\)::" crates/piperine-codegen crates/piperine-bench crates/piperine-python crates/piperine-solver/tests` → zero matches
- [ ] `cargo build --workspace` zero warnings
- [ ] `cargo test --workspace` passes (≥ 393: +prelude_surface)

**Tests**: integration (`tests/prelude_surface.rs`)
**Gate**: Full
**Commit**: `feat(solver): two-tier public surface — prelude(host)+abi(device-author), internals pub(crate)`

---

### T7: Decouple `as_iv` from `Netlist`

**What**: Change `DcAnalysisResult::as_iv` to take `&CircuitInstance` (reads
`circuit.netlist()` internally) so the analysis layer no longer depends on a
`Netlist` argument. Update BOTH call sites (transient + tf — the design listed
only transient; tf.rs:175 is a second site verified in the baseline).

**Where**:
- MODIFY `crates/piperine-solver/src/analysis/dc.rs` (`as_iv`, currently line 99)
- MODIFY `crates/piperine-solver/src/solver/transient.rs` (call site ~line 162)
- MODIFY `crates/piperine-solver/src/solver/tf.rs` (call site ~line 175)

**Depends on**: T6 (CircuitInstance is public via prelude; the signature change is a public-API touch)
**Reuses**: design.md Component 5
**Requirement**: ABI-06 (AC1–AC3)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. In `analysis/dc.rs`, change the signature and first line:
   ```rust
   pub fn as_iv(&self, circuit: &CircuitInstance) -> Vec<InitialValue<AnalogReference, f64>> {
       let netlist = circuit.netlist();
       // body unchanged
   }
   ```
   Add `use crate::core::circuit::CircuitInstance;` if not already in scope.
2. In `solver/transient.rs::compute_initial_conditions` (~line 161-162): the
   current code is `let netlist = self.system.circuit.netlist();` then
   `let iv_dc = dc_result.as_iv(netlist);`. Change to
   `let iv_dc = dc_result.as_iv(self.system.circuit);` (pass the circuit). If
   `netlist` is used elsewhere in that fn (it is — for other purposes), keep a
   separate `let netlist = self.system.circuit.netlist();` binding for those
   other uses; only the `as_iv` call changes.
3. In `solver/tf.rs::assemble_dc_stamps` (~line 171-175): current code is
   `let netlist = circuit.netlist();` ... `let dc_values_iv = dc_point.as_iv(netlist);`.
   Change the `as_iv` call to `dc_point.as_iv(circuit)`. Keep the `netlist`
   binding if used for `max_index()` (it is, line 172) — only the `as_iv` call
   changes.
4. Grep `\.as_iv\(` across the workspace for any other call site; update all.

**Done when**:
- [ ] `as_iv` signature is `(&self, &CircuitInstance) -> Vec<InitialValue<AnalogReference, f64>>`
- [ ] `grep -rn "\.as_iv(" crates/piperine-solver/src` shows only the definition + the 2 updated call sites, all passing `circuit`
- [ ] `grep -rn "use crate::analog::.*Netlist" crates/piperine-solver/src/analysis/dc.rs` — no `Netlist` type in a public signature (only via `circuit.netlist()` internally)
- [ ] `cargo test --workspace` passes; transient + tf results identical to baseline (±1e-9)
- [ ] Test count: ≥ 393 (no drop)

**Tests**: integration (existing tran + tf tests prove results unchanged ±1e-9)
**Gate**: Full
**Commit**: `refactor(solver): as_iv takes &CircuitInstance, decouples analysis from Netlist`

---

### T8: Element lifecycle — `setup` / `destroy` + `CircuitInstance` teardown

**What**: Add `setup` and `destroy` hooks to the `Element` trait (defaults
preserve behavior). Add `is_set_up` field + `setup_all` method to
`CircuitInstance`, and `impl Drop for CircuitInstance` calling `destroy`.
Insert `setup_all(ctx)` as the first statement (after `init_global`) in all
five analysis constructors. (Does NOT include `allocate_unknowns` — that is T9.)

**Where**:
- MODIFY `crates/piperine-solver/src/core/element.rs` (add 2 trait methods)
- MODIFY `crates/piperine-solver/src/core/circuit.rs` (field + method + Drop)
- MODIFY `crates/piperine-solver/src/solver/{dc,transient,ac,noise,tf}.rs` (one line each)

**Depends on**: T6
**Reuses**: design.md Component 8 (lifecycle portion — `setup`/`destroy` only)
**Requirement**: ABI-08 (AC1–AC5)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. In `core/element.rs`, add these two methods to `trait Element` (after
   `set_temperature` or near the analog lifecycle section; defaults preserve
   behavior — copy the doc comments from design.md Component 8):
   ```rust
   fn setup(&mut self, _ctx: &Context) -> crate::result::Result<()> { Ok(()) }
   fn destroy(&mut self) {}
   ```
2. In `core/circuit.rs`, add a private field `is_set_up: bool` to
   `CircuitInstance` (init `false` in `from_devices_and_netlist`). Add:
   ```rust
   pub(crate) fn setup_all(&mut self, ctx: &Context) -> crate::result::Result<()> {
       if self.is_set_up { return Ok(()); }
       for d in self.devices.iter_mut() { d.setup(ctx)?; }
       self.is_set_up = true;
       Ok(())
   }
   ```
   And:
   ```rust
   impl Drop for CircuitInstance {
       fn drop(&mut self) {
           for d in self.devices.iter_mut() { d.destroy(); }
       }
   }
   ```
3. Insert `circuit.setup_all(&context)?;` (or the equivalent borrowing shape —
   `setup_all` takes `&mut self`; the constructors hold
   `&'a mut CircuitInstance`) immediately AFTER `Context::init_global();` in:
   - `DcSolver::new` (`solver/dc.rs`, init_global at line 174)
   - `TransientSolver::new` (`solver/transient.rs`, init_global at line 119)
   - `AcSolver::new` (`solver/ac.rs`, init_global at line 86)
   - `NoiseSolver::new` (`solver/noise.rs`, init_global at line 55)
   - `TransferFunctionSolver::new` (`solver/tf.rs`, init_global at line 64)
   NOTE: AC/noise/tf construct a `DcSolver` internally, which sets up first;
   the outer `setup_all` is then a no-op via the `is_set_up` guard. Keep all
   five for uniformity (design.md Component 8).
4. Add a unit test (co-located in `circuit.rs` `#[cfg(test)]` OR a new
   `tests/lifecycle.rs`): a test double with `AtomicUsize` call counters for
   `setup` and `destroy`. Construct DC then AC over one circuit →
   `setup_calls == 1`; drop circuit → `destroy_calls == 1`. Also test: when
   `setup` returns `Err`, the analysis constructor returns that `Err` and no
   `assemble` runs (assert by checking the error propagates — `circuit.dc(...)`
   returns `Err`).

**Done when**:
- [ ] `Element` has `setup` (default `Ok(())`) and `destroy` (default `{}`)
- [ ] `CircuitInstance` has `is_set_up` + `setup_all` + `impl Drop`
- [ ] All 5 analysis constructors call `setup_all` after `init_global`
- [ ] `setup` called exactly once per circuit lifetime even across multiple analyses (test proves it)
- [ ] `destroy` called exactly once on drop (test proves it)
- [ ] `setup` error propagates from analysis constructor (test proves it)
- [ ] `cargo test --workspace` passes; existing results unchanged (±1e-9)
- [ ] Test count: ≥ 394 (+lifecycle test)

**Tests**: unit (lifecycle call-counter test) — co-located or `tests/lifecycle.rs`
**Gate**: Quick
**Commit**: `feat(solver): element lifecycle hooks setup/destroy with CircuitInstance Drop`

---

### T9: `CircuitBuilder` + `UnknownAllocator` + `allocate_unknowns` trait method

**What**: Add `core/builder.rs` with `CircuitBuilder` (safe assembly) and
`UnknownAllocator` (pre-freeze internal-unknown seam). Add the
`allocate_unknowns` trait method (default no-op). Wire `CircuitBuilder::build`
to: run `allocate_unknowns` per element (checking `HAS_INTERNAL_UNKNOWNS`),
assemble `CircuitInstance`, size + label digital state, rebuild topology, init
digital. Export `CircuitBuilder` in `prelude`, `UnknownAllocator` in `abi`
(uncomment the T9 line in `abi.rs`).

**Where**:
- NEW `crates/piperine-solver/src/core/builder.rs`
- MODIFY `crates/piperine-solver/src/core/mod.rs` (declare `pub mod builder;`)
- MODIFY `crates/piperine-solver/src/core/element.rs` (add `allocate_unknowns`)
- MODIFY `crates/piperine-solver/src/abi.rs` (uncomment `UnknownAllocator`)
- MODIFY `crates/piperine-solver/src/prelude.rs` (add `CircuitBuilder`)

**Depends on**: T8 (build calls `setup_all` indirectly via analyses later, and reuses the `is_set_up` field; the allocator check uses `HAS_INTERNAL_UNKNOWNS` which exists)
**Reuses**: design.md Component 7 (full `CircuitBuilder` + `UnknownAllocator` definitions + build sequence)
**Requirement**: ABI-07 (AC1–AC7), ABI-09 (AC1–AC4)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. In `core/element.rs`, add the trait method (default no-op):
   ```rust
   fn allocate_unknowns(&mut self, _alloc: &mut crate::core::builder::UnknownAllocator<'_>) {}
   ```
2. Create `core/builder.rs` with `CircuitBuilder` + `UnknownAllocator` exactly
   as in design.md Component 7. Key implementation details:
   - `UnknownAllocator::branch(component, name)` calls
     `self.netlist.connect_branch(...)` — inspect `analog/netlist.rs` for the
     exact `connect_branch` signature (`BranchIdentifier` construction); the
     branch name combines `component` + `name` so it is unique. Increment
     `self.allocated`. Return the `AnalogReference`.
   - `CircuitBuilder::ground()` returns the netlist's ground reference; use the
     existing netlist ground accessor (inspect `analog/netlist.rs` for
     `ground()` / `GND`). Idempotent (store in `nodes["gnd"]`).
   - `CircuitBuilder::node(name)`: gnd-family names (`gnd`/`GND`/`vss`/`VSS` —
     match the netlist's existing gnd-family set) route to ground. Others:
     `netlist.connect_node(NodeIdentifier::named(name))` (inspect the exact
     `NodeIdentifier` constructor). Cache in `nodes` HashMap for idempotent
     lookup.
   - `CircuitBuilder::digital_net(label)`: push label into `digital_labels`,
     return `DigitalNet(index)` where index = `digital_labels.len() - 1`.
   - `CircuitBuilder::build()` sequence (design.md Component 7, normative):
     1. For each element in insertion order: fresh
        `UnknownAllocator<'_> { netlist: &mut netlist, allocated: 0 }` →
        `element.allocate_unknowns(&mut alloc)`. If
        `alloc.allocated() > 0 && !element.capabilities().contains(HAS_INTERNAL_UNKNOWNS)`
        → `Err(Error::simple(SolverDomain::Element, format!("element `{}` allocated internal unknowns without declaring HAS_INTERNAL_UNKNOWNS", element.name())))`.
     2. `let mut instance = CircuitInstance::from_devices_and_netlist(title, elements, netlist);`
     3. Size + label digital state: build a `Vec<String>` of length
        `digital_labels.len()` (each `Some(s)` → `s`, each `None` →
        `format!("d{i}")`), then `instance.digital_state =
        DigitalState::with_labels(count, labels);` (the field is `pub`).
        If `digital_labels.is_empty()`, leave the default `DigitalState::new(0)`.
     4. `instance.rebuild_digital_topology();`
     5. `instance.init_digital()?;`
     6. Return `Ok(instance)`.
3. In `core/mod.rs` add `pub mod builder;` (the module is `pub(crate)` via the
   flip in T6 — `core` is `pub(crate)`, so `builder` is crate-internal; items
   are re-exported via `abi`/`prelude`).
4. In `abi.rs`, uncomment the `pub use crate::core::builder::UnknownAllocator;`
   line (marked `// TODO(T9):` in T2).
5. In `prelude.rs`, add `pub use crate::core::builder::CircuitBuilder;`.
6. Tests (co-locate in `builder.rs` `#[cfg(test)]` OR `tests/circuit_builder.rs`):
   - AC1: `CircuitBuilder::new("top")` → empty netlist, 0 elements, 0 digital nets, no ground.
   - AC2: `.ground()` idempotent — two calls return equal references.
   - AC3: `.node("out")` twice → same reference; matrix size grows by 1 per unique node.
   - AC4: `.digital_net(Some("clk"))` → sequential indices, label registered.
   - AC6: `.build()` returns a `CircuitInstance` where `init_digital` ran and digital net count == digital_net calls.
   - AC7: `.build()` without `.ground()` succeeds.
   - Edge: `.build()` with zero elements → valid empty instance (DC yields empty result, no panic).
   - Edge: `.node("gnd")` routes to ground.
   - ABI-09: a test double allocating one branch via `allocate_unknowns` +
     declaring `HAS_INTERNAL_UNKNOWNS`; build; assert `netlist.max_index()` ==
     nodes + 1; DC solves with the branch row present.
   - ABI-09 AC3: the same double WITHOUT `HAS_INTERNAL_UNKNOWNS` → `build()`
     returns `Err` naming the element.
   - Independent test (spec ABI-07): build a two-resistor divider via
     `CircuitBuilder` + a V source element (use test doubles), run DC through
     `Solver` or `circuit.dc(...)`, assert midpoint ±1e-9.

**Done when**:
- [ ] `core/builder.rs` defines `CircuitBuilder` + `UnknownAllocator` per design
- [ ] `Element::allocate_unknowns` exists (default no-op)
- [ ] `abi` exports `UnknownAllocator`; `prelude` exports `CircuitBuilder`
- [ ] All ABI-07 AC1–AC7 + edge cases have passing tests
- [ ] ABI-09 AC1–AC4 have passing tests (allocation grows matrix; missing-flag errors loud)
- [ ] `cargo test --workspace` passes; existing tests unchanged (±1e-9)
- [ ] Test count: ≥ 395 (+builder/allocator tests)

**Tests**: unit (co-located builder tests + `tests/circuit_builder.rs`)
**Gate**: Quick
**Commit**: `feat(solver): CircuitBuilder + UnknownAllocator + allocate_unknowns seam`

---

### T10: Rich terminal descriptors (`discipline` + `sign`)

**What**: Add `SignConvention` enum and `discipline`/`sign` fields to
`TerminalDescriptor`. Add a `TerminalDescriptor::new` constructor with OSDI
defaults. Export `SignConvention` in `abi` + `prelude`. Migrate any struct-literal
construction sites to the constructor.

**Where**:
- MODIFY `crates/piperine-solver/src/core/introspect.rs` (TerminalDescriptor at line 168)
- MODIFY `crates/piperine-solver/src/abi.rs` (uncomment `SignConvention`, line marked `// TODO(T10):`)
- MODIFY `crates/piperine-solver/src/prelude.rs` (add `SignConvention`)

**Depends on**: T2 (abi structure); independent of T3–T9
**Reuses**: design.md Component 9
**Requirement**: ABI-10 (AC1–AC3)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. In `core/introspect.rs`, add the enum (before `TerminalDescriptor`):
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub enum SignConvention { IntoTerminal, OutOfTerminal }
   ```
2. Add two fields to `TerminalDescriptor`:
   ```rust
   pub discipline: Option<String>,
   pub sign: SignConvention,
   ```
3. Add the constructor:
   ```rust
   impl TerminalDescriptor {
       pub fn new(name: impl Into<String>, domain: Domain, direction: Direction) -> Self {
           Self {
               name: name.into(), domain, direction,
               required: true, discipline: None, sign: SignConvention::IntoTerminal,
           }
       }
   }
   ```
4. Grep `TerminalDescriptor {` across the whole workspace (verified: only the
   struct definition in `introspect.rs` — no struct-literal construction sites
   exist in the baseline). If any are found, migrate them to `::new(..)` +
   field overrides. None expected.
5. In `abi.rs`, uncomment `SignConvention` in the introspection re-export block
   (marked `// TODO(T10):` in T2). In `prelude.rs`, add `SignConvention` to the
   introspection `pub use` list.
6. Tests (co-located in `introspect.rs` `#[cfg(test)]`): assert
   `TerminalDescriptor::new("p", Domain::Analog, Direction::Inout)` has
   `required == true`, `discipline == None`, `sign == IntoTerminal`; assert a
   custom descriptor with `discipline: Some("electrical".into())` +
   `sign: OutOfTerminal` builds.

**Done when**:
- [ ] `TerminalDescriptor` carries `discipline: Option<String>` + `sign: SignConvention`
- [ ] `::new` defaults `required=true, discipline=None, sign=IntoTerminal`
- [ ] `abi` + `prelude` export `SignConvention`
- [ ] `cargo build --workspace` zero warnings (no `private_interfaces` leak)
- [ ] `cargo test --workspace` passes (existing introspect tests + new)
- [ ] Test count: ≥ 396

**Tests**: unit (co-located introspect tests)
**Gate**: Quick
**Commit**: `feat(solver): terminal descriptors carry discipline + sign convention`

---

### T11: Noise metadata — `NoiseKind` + named sources + emitter migration

**What**: Add `NoiseKind` enum + `name`/`kind` fields to `Noise`. Add
`Noise::new` (anonymous default) + `Noise::named` (builder). Migrate the single
codegen emitter (`device/analog.rs:1112`) to `Noise::new`. Existing noise tests
stay green with total PSD unchanged (±1e-12). Does NOT add per-source reporting
yet (that is T12).

**Where**:
- MODIFY `crates/piperine-solver/src/analysis/noise.rs` (`Noise` struct, line 6)
- MODIFY `crates/piperine-codegen/src/device/analog.rs` (emitter, line 1112)
- MODIFY `crates/piperine-solver/src/abi.rs` + `prelude.rs` (export `NoiseKind`)

**Depends on**: T6 (abi/prelude exports)
**Reuses**: design.md Component 10 (Noise struct portion)
**Requirement**: ABI-11 (AC1, AC4)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. In `analysis/noise.rs`, add the enum + extend the struct + constructors
   exactly as design.md Component 10:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
   pub enum NoiseKind { Thermal, Shot, Flicker, Other }

   pub struct Noise {
       pub terminals: (AnalogReference, AnalogReference),
       pub value: AmpereSquaredSecond,
       pub name: Option<String>,
       pub kind: NoiseKind,
   }

   impl Noise {
       pub fn new(terminals: (AnalogReference, AnalogReference), value: AmpereSquaredSecond) -> Self {
           Self { terminals, value, name: None, kind: NoiseKind::Other }
       }
       pub fn named(mut self, name: impl Into<String>, kind: NoiseKind) -> Self {
           self.name = Some(name.into()); self.kind = kind; self
       }
   }
   ```
2. In `device/analog.rs:1112`, change `Noise { terminals: (plus, minus), value }`
   to `Noise::new((plus, minus), value)`.
3. Grep `Noise {` across the workspace for any other construction site; migrate
   all to `Noise::new(..)`.
4. Export `NoiseKind` in `abi.rs` (introspection or noise block) and `prelude.rs`.
5. Tests (co-located in `analysis/noise.rs` `#[cfg(test)]`): assert
   `Noise::new(..)` has `name == None`, `kind == Other`; assert
   `Noise::new(..).named("rn", NoiseKind::Thermal)` sets both. Run the Johnson
   noise example + existing noise tests; assert total PSD unchanged (±1e-12).

**Done when**:
- [ ] `Noise` carries `name: Option<String>` + `kind: NoiseKind`
- [ ] `Noise::new` defaults `name=None, kind=Other`; `named` sets both
- [ ] Codegen emitter uses `Noise::new`
- [ ] `abi` + `prelude` export `NoiseKind`
- [ ] `cargo test --workspace` passes; total PSD values identical to baseline (±1e-12)
- [ ] Test count: ≥ 397

**Tests**: unit (co-located noise struct tests) + integration (Johnson noise example unchanged)
**Gate**: Full
**Commit**: `feat(solver): Noise carries name + kind; emitters migrate to Noise::new`

---

### T12: Per-source noise reporting + conservation

**What**: Add `NoiseContribution` struct + `contributions: Vec<NoiseContribution>`
to `NoiseAnalysisResult` + `contributions()` accessor. Extend
`solver/noise.rs::solve` to accumulate per-source PSD inside the existing
frequency loop. Add the conservation test (Σ per-source psd == total
`out_noise_sq` per frequency ±1e-9).

**Where**:
- MODIFY `crates/piperine-solver/src/analysis/noise.rs` (`NoiseAnalysisResult`, `NoiseContribution`)
- MODIFY `crates/piperine-solver/src/solver/noise.rs` (`solve`, lines 93–131; inner loop 104–121)
- MODIFY `crates/piperine-solver/src/abi.rs` + `prelude.rs` (export `NoiseContribution`)

**Depends on**: T11 (Noise has `name`/`kind` to key contributions on)
**Reuses**: design.md Component 10 (per-source portion + conservation invariant)
**Requirement**: ABI-11 (AC2, AC3)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. In `analysis/noise.rs`, add `NoiseContribution` and extend
   `NoiseAnalysisResult` exactly as design.md Component 10:
   ```rust
   #[derive(Debug, Clone)]
   pub struct NoiseContribution {
       pub element: String,
       pub source: String,
       pub kind: NoiseKind,
       pub integrated_sq: f64,
       pub psd: Vec<f64>,
   }
   ```
   Add `pub contributions: Vec<NoiseContribution>` to `NoiseAnalysisResult` +
   `pub fn contributions(&self) -> &[NoiseContribution]`.
2. In `solver/noise.rs::solve` (lines 93–131): inside the `for &f in &frequencies`
   loop, the inner `for source in &mut self.circuit.devices` → `for n in noises`
   already computes `gain_sq * n.value` per source. Add a per-source accumulator:
   a `HashMap<(String, String), Vec<f64>>` keyed by
   `(element.name().to_string(), n.name.clone().unwrap_or_else(|| idx.to_string()))`
   where `idx` is the source's position in that element's returned vec. For
   each source, push `gain_sq * n.value` into the key's psd vec (one entry per
   frequency). Handle late-appearing sources by padding earlier frequencies with
   0.0 (a source may appear only at some frequencies — missing = 0.0). After
   the frequency loop, integrate each source's psd with the SAME trapezoidal
   rule as `integrate_noise` BUT without the final `.sqrt()` (store mean-square
   so contributions sum to total² — see `integrated_sq` doc). Build the
   `contributions` vec. Add `contributions` to the returned `NoiseAnalysisResult`.
3. Export `NoiseContribution` in `abi.rs` + `prelude.rs`.
4. Tests (`tests/noise_sources.rs` or co-located):
   - AC2: a circuit with two named noise sources; after `solve()`,
     `result.contributions()` returns one entry per (element, source) pair;
     unnamed sources use index strings `"0"`, `"1"`.
   - AC3 conservation: for every frequency index `i`,
     `Σ contribution.psd[i] ≈ result.out_noise_sq[i]` within reltol 1e-9.
   - AC4: total `integrated_noise` + `out_noise_sq` identical to baseline (±1e-12).

**Done when**:
- [ ] `NoiseAnalysisResult` has `contributions: Vec<NoiseContribution>` + accessor
- [ ] `solve` populates per-source psd + integrated_sq without changing total PSD
- [ ] `abi` + `prelude` export `NoiseContribution`
- [ ] Conservation test passes (Σ psd == out_noise_sq ±1e-9 per frequency)
- [ ] Total PSD unchanged vs baseline (±1e-12)
- [ ] `cargo test --workspace` passes
- [ ] Test count: ≥ 398

**Tests**: unit + integration (conservation + Johnson noise unchanged)
**Gate**: Full
**Commit**: `feat(solver): per-source noise contributions with conservation invariant`

---

### T13: Stamp-capability declaration (`LINEAR` / `STAMPS_CHARGE` / `ANALYTIC_JACOBIAN`)

**What**: Add three capability flags to `ElementCapabilities` (declaration +
doc only — MD-11 checklist; NOT consumed by solver code). Set
`ANALYTIC_JACOBIAN` always + `STAMPS_CHARGE` when the kernel has a charge part
in `PiperineDevice::capabilities`.

**Where**:
- MODIFY `crates/piperine-solver/src/core/element.rs` (`ElementCapabilities`, after `BYPASS_OK` = 1<<11)
- MODIFY `crates/piperine-codegen/src/device/mod.rs` (`PiperineDevice::capabilities`, line 148)

**Depends on**: T6
**Reuses**: design.md Component 11
**Requirement**: ABI-12 (AC1–AC3)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. In `core/element.rs`, add three flags to the `bitflags!` block after
   `BYPASS_OK` (1 << 11), with the doc comments from design.md Component 11
   (each doc states "declaration consumed by solver-performance follow-up"):
   ```rust
   const LINEAR = 1 << 12;
   const STAMPS_CHARGE = 1 << 13;
   const ANALYTIC_JACOBIAN = 1 << 14;
   ```
2. In `device/mod.rs::PiperineDevice::capabilities` (line 148): when
   `self.analog.is_some()` (the device has an analog kernel), OR in
   `ANALYTIC_JACOBIAN` always (symbolic differentiation). OR in
   `STAMPS_CHARGE` when the kernel has a charge part. The predicate:
   `AnalogKernel::has_charge()` (verified: `jit/analog.rs:384` —
   `self.charge.is_some()`). Inspect the `AnalogInstance` → `AnalogKernel`
   accessor (`.kernel()`); set `STAMPS_CHARGE` iff
   `self.analog.as_ref().is_some_and(|a| a.kernel().has_charge())`. If the
   accessor is named differently, use the actual one; note it in the commit
   body. Do NOT set `LINEAR` (codegen devices are generally nonlinear; the
   flag is for future linear-classification).
3. Tests (co-located in codegen OR a new `tests/capability_flags.rs`): build a
   PHDL device with a reactive (`ddt`) contribution → assert
   `capabilities().contains(STAMPS_CHARGE | ANALYTIC_JACOBIAN)`; build a
   purely resistive device → assert `contains(ANALYTIC_JACOBIAN)` but
   `!contains(STAMPS_CHARGE)`.

**Done when**:
- [ ] Three flags declared with doc comments noting "consumed by solver-performance follow-up"
- [ ] `PiperineDevice` sets `ANALYTIC_JACOBIAN` whenever analog; `STAMPS_CHARGE` iff kernel has charge
- [ ] `cargo build --workspace` zero warnings
- [ ] `cargo test --workspace` passes (no solver code consumes the flags — MD-11)
- [ ] Test count: ≥ 399

**Tests**: unit (codegen capability test — reactive vs resistive)
**Gate**: Codegen
**Commit**: `feat(solver): declare LINEAR/STAMPS_CHARGE/ANALYTIC_JACOBIAN capability flags`

---

### T14: `Solver` entry point (`build()` triggers `init_global`)

**What**: Add `src/solver/solve.rs` with the `Solver` struct — owns
`CircuitInstance` + `Context` + `Policy` + tran opts; `build()` calls
`Context::init_global()` (Once-guarded); hands out all five analyses with the
`Solver`'s policy threaded in. Export in `prelude`.

**Where**:
- NEW `crates/piperine-solver/src/solver/solve.rs`
- MODIFY `crates/piperine-solver/src/solver/mod.rs` (declare `pub mod solve;`)
- MODIFY `crates/piperine-solver/src/prelude.rs` (add `Solver`)

**Depends on**: T8 (setup_all must exist so analyses work through Solver), T6 (prelude exports)
**Reuses**: design.md Component 6 (full `Solver` definition + the `ac` signature note)
**Requirement**: ABI-01 (AC1–AC4)

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. Create `solver/solve.rs` with the `Solver` struct exactly as design.md
   Component 6. Mirror the existing
   `CircuitInstance::{dc,transient,ac,noise,transfer_function}` signatures —
   `Solver` adds only ownership + policy threading + `build()`. Key rules:
   - `Solver::new(circuit)` → defaults `Context::default()`, `Policy::default()`,
     `tran_opts = TransientAnalysisOptions::new(1e-3, 1e-6)` (verify the ctor
     signature in `analysis/transient.rs`; adjust if it differs).
   - `build(mut self) -> Self`: calls `Context::init_global()`; sets
     `self.built = true`; returns `self`. (init_global is `Once`-guarded —
     repeat calls are free.)
   - `dc(&mut self) -> Result<DcSolver<'_>>`: sets
     `self.circuit.dc(self.context.clone())?` then sets
     `.policy = self.policy.clone()` on the returned solver (the `policy` field
     is `pub` per design). Return it.
   - `tran(&mut self)`: `self.circuit.transient(self.tran_opts.clone(), self.context.clone())?`
     then set `.policy`.
   - `ac(&mut self) -> Result<AcSolver<'_>>`: NO sweep arg (design note — host
     calls `.solve_sweep(opts)` next). Construct `AcSolver`, set policy, return.
   - `noise(&mut self, opts)` / `tf(&mut self, opts)`: mirror the existing
     `CircuitInstance` constructors (they take opts at construction).
   - `circuit(&self)` / `context(&self)` accessors.
   - `built` field: keep private. If no test needs the accessor, drop the field
     to avoid dead code (design says implementer's choice — do NOT leave dead
     code; MD-13 rule 3). Recommended: drop `built`; `build()` just calls
     `init_global` and returns `self`.
2. In `solver/mod.rs` add `pub mod solve;`.
3. In `prelude.rs` add `pub use crate::solver::solve::Solver;`.
4. Tests (`tests/solver_entry.rs`): build two `Solver`s from two circuits in
   one test process (spec ABI-01 independent test); run DC on both; assert the
   second build does not panic and both results are correct (voltage-divider
   expected values, tolerance ±1e-9). Test AC2: a second `Solver::build()` in
   the same process does not panic (Once guard). Test AC4: an element whose
   `setup` returns `Err` → `Solver::dc()` returns that `Err`.

**Done when**:
- [ ] `Solver` struct + builder + 5 analysis methods exist per design
- [ ] `build()` calls `Context::init_global()` (Once-guarded)
- [ ] Second `Solver::build()` in same process does not panic
- [ ] All 5 analyses return their solvers with policy threaded
- [ ] `setup` error propagates from `Solver::dc()` (AC4)
- [ ] `prelude` exports `Solver`
- [ ] `cargo build --workspace` zero warnings (no dead code)
- [ ] `cargo test --workspace` passes; divider DC correct ±1e-9
- [ ] Test count: ≥ 400

**Tests**: integration (`tests/solver_entry.rs`)
**Gate**: Full
**Commit**: `feat(solver): Solver entry point — build() triggers init_global, hands out 5 analyses`

---

### T15: Final workspace verification + example gallery

**What**: The closing verification task. Confirm the full success criteria from
`spec.md`: zero warnings, all tests green (≥ baseline + new), 21/21 examples,
grep gate empty, noise conservation. This is the author's final pass BEFORE the
Verifier sub-agent runs.

**Where**: whole workspace (no source edits expected; if a gap surfaces, add a
fix commit and re-run)

**Depends on**: T1–T14 all complete
**Reuses**: spec.md "Success Criteria" section
**Requirement**: ABI-13

**Tools**:
- MCP: NONE
- Skill: NONE

**Exact steps**:

1. `cargo build --workspace` — MUST be zero warnings. Fix any warning (dead
   code, unused import, `private_interfaces` leak).
2. `cargo test --workspace` — all green. Record the test count (must be ≥ 391
   baseline + the new tests added in T2/T6/T8/T9/T10/T11/T12/T13/T14).
3. Rebuild the root CLI binary: `cargo build -p piperine`.
4. Run the 21 `examples/*.py` via `cargo test -p piperine-bench --test run_examples`
   (or `piperine run examples/<each>.py`). All 21 pass.
5. Grep gate: `grep -rn "piperine_solver::\(core\|math\|digital\|analog\|analysis\|solver\|result\|error\)::" crates/piperine-codegen crates/piperine-bench crates/piperine-python crates/piperine-solver/tests`
   → ZERO matches.
6. Noise conservation spot-check: run the Johnson-noise example; confirm
   per-source contributions sum to total PSD (±1e-9).
7. Record the AD entry per design.md Tech Decisions: "public surface = prelude
   + abi; everything else pub(crate)" → append to `.specs/STATE.md` Decisions
   as AD-NNN (next free ID).

**Done when**:
- [ ] `cargo build --workspace` zero warnings
- [ ] `cargo test --workspace` green (record count: ≥ 400)
- [ ] 21/21 examples pass
- [ ] Grep gate empty
- [ ] Noise conservation holds (±1e-9)
- [ ] AD entry recorded in `.specs/STATE.md`

**Tests**: none (verification-only; uses all prior gates)
**Gate**: Examples
**Commit**: `chore(solver): final verification — solver-abi feature complete` (only if a fix was needed; otherwise no commit)

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10 → Phase 11

Phase 1:   T1
Phase 2:   T2
Phase 3:   T3 ──→ T4 ──→ T5 ──→ T6
Phase 4:   T7
Phase 5:   T8
Phase 6:   T9
Phase 7:   T10
Phase 8:   T11 ──→ T12
Phase 9:   T13
Phase 10:  T14
Phase 11:  T15
```

Execution is strictly sequential — there is no intra-phase parallelism. A
single agent (or batch worker) works one task at a time, in order.

**How phase-based execution works:**

At Execute, the agent counts total tasks (15) and packs phases into
**task-budgeted batches** (~7 tasks per worker, whole phases — the benchmarked
sweet spot is ~20 tasks → ~3 workers). A **phase** is the semantic/dependency
unit; a **batch** is one or more *consecutive whole phases* assigned to one
worker. The cut only ever lands on a phase boundary — a phase is never split
across workers. When packing yields more than one batch (> ~8 tasks), the agent
offers to dispatch batch sub-agents. Batches run sequentially: each worker
executes ALL its tasks in order, then reports a compact summary before the next
batch starts.

**Suggested packing for this feature (15 tasks → 3 batches):**

| Batch | Phases | Tasks | Count |
|-------|--------|-------|-------|
| 1 | Phase 1 + 2 + 3 | T1, T2, T3, T4, T5, T6 | 6 |
| 2 | Phase 4 + 5 + 6 + 7 | T7, T8, T9, T10 | 4 |
| 3 | Phase 8 + 9 + 10 + 11 | T11, T12, T13, T14, T15 | 5 |

When the whole feature fits a single batch (≤ ~8 tasks), execution happens
inline in the main window with no sub-agents spawned. Here 15 > 8 → **offer
sub-agents** (offer-then-confirm; the user must accept before dispatch).

**The orchestrating agent's role during Execute:**
1. Count total tasks and pack phases into ~7-task batches — offer batch sub-agents if that yields more than one batch and the user accepts
2. Dispatch the next batch (to a worker, or execute inline)
3. Receive the compact batch summary
4. Update tasks.md with results
5. If the batch summary shows all tasks complete: proceed to the next batch
6. If a task failed: decide fix/escalate before dispatching the next batch

After the final task (T15) is committed, a fresh **Verifier** sub-agent runs
automatically (author ≠ verifier) — spec-anchored outcome check + discrimination
sensor. It writes `.specs/features/solver-abi/validation.md` and distills
lessons. This is never optional and never prompted.

---

## Task Granularity Check

| Task | Scope | Status |
|------|-------|--------|
| T1: Scheduler split | 1 file split (3 files) + re-export update — cohesive mechanical refactor | ✅ Granular |
| T2: `abi` module | 1 new module + 1 smoke test | ✅ Granular |
| T3: Migrate codegen imports | 1 crate, mechanical grep→replace | ✅ Granular |
| T4: Migrate bench imports | 1 crate, mechanical grep→replace | ✅ Granular |
| T5: Migrate python + solver-tests imports | 2 crates, mechanical grep→replace | ✅ Granular |
| T6: Privacy flip + prelude_surface | 1 file rewrite + 1 test | ✅ Granular |
| T7: `as_iv` decoupling | 1 signature + 2 call sites | ✅ Granular |
| T8: Lifecycle setup/destroy | 1 trait + 1 struct + 5 one-line inserts + test | ✅ Granular |
| T9: CircuitBuilder + allocator | 1 new module + 1 trait method + tests | ✅ Granular |
| T10: Terminal descriptors | 1 enum + 2 fields + 1 constructor + test | ✅ Granular |
| T11: Noise metadata | 1 enum + struct fields + 1 emitter migration | ✅ Granular |
| T12: Per-source noise | 1 struct + 1 loop extension + conservation test | ✅ Granular |
| T13: Capability flags | 3 flags + 1 capabilities() edit + test | ✅ Granular |
| T14: `Solver` entry point | 1 new module + prelude export + test | ✅ Granular |
| T15: Final verification | verification-only, no source edits | ✅ Granular |

**Granularity check:**
- ✅ 1 component / 1 function / 1 file change = Good
- ⚠️ T8 touches element.rs + circuit.rs + 5 solver files — but it is ONE concept (lifecycle) with a uniform one-line insert per constructor; cohesive, not multi-concept. OK.
- ⚠️ T9 bundles CircuitBuilder + UnknownAllocator + allocate_unknowns — but they are one seam (the allocator is the build()'s pre-freeze step; the trait method is its caller). Splitting would leave untestable fragments. OK.
- ❌ None exceed the threshold.

---

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
|------|------------------------|---------------|--------|
| T1 | None | (Phase 1, no inbound arrows) | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T2 | T3 → T4 (Phase 3 chain) | ✅ Match (T4 depends on T2; ordering T3→T4 is phase-internal) |
| T5 | T2 | T4 → T5 | ✅ Match (T5 depends on T2; phase-internal ordering) |
| T6 | T3, T4, T5 | T5 → T6 | ✅ Match (T6 closes Phase 3; depends on all migrations) |
| T7 | T6 | T6 → T7 (Phase 3 → Phase 4) | ✅ Match |
| T8 | T6 | T7 → T8 | ✅ Match (Phase boundary; T8 depends on T6, ordered after T7) |
| T9 | T8 | T8 → T9 | ✅ Match |
| T10 | T2 | (Phase 7, after T9 in ordering) | ✅ Match (T10 depends on T2 only; placed after T9 by phase plan) |
| T11 | T6 | T10 → T11 | ✅ Match (T11 depends on T6; phase-internal) |
| T12 | T11 | T11 → T12 | ✅ Match |
| T13 | T6 | T12 → T13 | ✅ Match (T13 depends on T6; phase boundary) |
| T14 | T8, T6 | T13 → T14 | ✅ Match (T14 depends on T8+T6; ordered after T13) |
| T15 | T1–T14 | T14 → T15 | ✅ Match |

**Rules verified:**
- Every `Depends on` in a task body has a corresponding arrow in the diagram (directly or via phase ordering).
- Every arrow in the diagram corresponds to a `Depends on` (or phase-internal ordering).
- No task depends on a task in a later phase — all dependencies point backward or within the same phase.

---

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
|------|-----------------------------|-----------------|-----------|--------|
| T1 | Digital scheduler split | integration | integration (existing digital_topology + mixed_signal, import-only) | ✅ OK |
| T2 | Solver ABI public surface | integration | integration (`tests/abi_surface.rs`) | ✅ OK |
| T3 | Downstream migration (codegen) | integration (build gate) | integration (codegen existing tests, untouched) | ✅ OK |
| T4 | Downstream migration (bench) | integration (build gate) | integration (bench existing tests, untouched) | ✅ OK |
| T5 | Downstream migration (python+tests) | integration (build gate) | integration (existing tests, import-only) | ✅ OK |
| T6 | Solver ABI public surface | integration | integration (`tests/prelude_surface.rs`) | ✅ OK |
| T7 | Solver core (analysis) | unit | integration (existing tran+tf tests prove ±1e-9) | ✅ OK (existing integration covers the AC; no new unit needed — behavior is "results unchanged") |
| T8 | Solver core (Element, CircuitInstance) | unit | unit (lifecycle call-counter test) | ✅ OK |
| T9 | Solver core (CircuitBuilder, UnknownAllocator) | unit | unit (co-located builder tests + `tests/circuit_builder.rs`) | ✅ OK |
| T10 | Solver core (introspect) | unit | unit (co-located introspect tests) | ✅ OK |
| T11 | Solver core (Noise) + Codegen device | unit | unit (co-located noise tests) + integration (Johnson example) | ✅ OK (highest = unit; integration is the ±1e-12 gate) |
| T12 | Solver core (noise result + solver loop) | unit + integration | unit + integration (conservation + Johnson) | ✅ OK |
| T13 | Codegen device capabilities | unit | unit (codegen capability test) | ✅ OK |
| T14 | Solver core (Solver) | integration | integration (`tests/solver_entry.rs`) | ✅ OK |
| T15 | (verification only) | none | none | ✅ OK (no code layer created) |

**Rules verified:**
- No task defers tests to another task ("tested in another task" is not a justification).
- `Tests: none` appears only for T15 (verification-only — no code layer created).
- Where a task touches multiple layers, the highest test type is used (T11/T12: unit + integration).
- Every task that creates/modifies a code layer with a required test type includes co-located tests in the same task + the matching gate.

---

## Notes for the implementing agent

- **Read design.md before each task.** Every task references the design section
  that holds the exact struct definitions and code. The task body is the
  *step list*; the design is the *spec*.
- **Line numbers drift.** Locate items by symbol when a line number has moved.
- **Never weaken/skip/delete a test to make a gate pass.** If a test fails, the
  implementation is wrong — fix the implementation. (Critical Rule 3.)
- **One atomic commit per task.** Commit message format is given per task.
- **`abi.rs` re-export list is normative.** If an item is missing at compile
  time, ADD the re-export and note it in the commit body — never reach for an
  internal `piperine_solver::<module>::` path from outside the solver crate.
- **MD-13 rule 2 (no loose functions) + rule 5 (no macros)** govern every line.
  T1's evaluation loops stay methods on `DigitalState` (second `impl` block),
  not free functions.
