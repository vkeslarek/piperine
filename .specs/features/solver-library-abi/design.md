# Solver Library ABI / Prelude — Design

**Spec:** `.specs/features/solver-library-abi/spec.md`
**Decisions:** MD-06, MD-13

## Architecture

```
Host code:
  use piperine_solver::prelude::*;

  let circuit = CircuitBuilder::new("top")
      .add_ground()
      .add_node("out")
      .add_element("r1", resistor)
      .connect("out", "gnd", resistor_resistance)
      .build()?;

  let mut solver = Solver::new(circuit)
      .with_context(Context::default())
      .with_plan(ConvergencePlan::default())
      .build();

  let op = solver.dc()?.solve()?;
  let vout = op.get_net(&circuit.net("out"))?;
```

## Components

### 1. `CircuitBuilder` (new file: `core/builder.rs`)

A safe, discoverable builder for circuit assembly. Wraps the manual
`Netlist::connect_node`/`Netlist::connect_branch` API.

```rust
pub struct CircuitBuilder {
    title: String,
    nodes: HashMap<String, NodeIdentifier>,
    branches: Vec<BranchIdentifier>,
    elements: Vec<Box<dyn Element>>,
    netlist: Netlist,
    has_ground: bool,
    digital_net_count: usize,
}

impl CircuitBuilder {
    pub fn new(title: impl Into<String>) -> Self;

    /// Allocate the ground node. Idempotent — second call is no-op.
    pub fn add_ground(&mut self) -> NodeIdentifier;

    /// Allocate a named analog node. Returns the NodeIdentifier.
    pub fn add_node(&mut self, name: &str) -> NodeIdentifier;

    /// Allocate a digital net with an optional source label.
    pub fn add_digital_net(&mut self, label: Option<&str>) -> DigitalNet;

    /// Add a compiled element. The element binds its terminals via
    /// `DigitalPorts` (digital) or netlist references (analog) — the
    /// caller provides the resolved bindings.
    pub fn add_element(&mut self, element: Box<dyn Element>);

    /// Look up a previously registered node by name.
    pub fn node(&self, name: &str) -> Option<&NodeIdentifier>;

    /// Build the `CircuitInstance`. Consumes the builder.
    pub fn build(mut self) -> crate::result::Result<CircuitInstance>;
}
```

`build()` calls `CircuitInstance::from_devices_and_netlist`, then
`rebuild_digital_topology()` and `init_digital()` automatically.

### 2. `Solver` (new file: `solver/solve.rs`)

Thin wrapper around `CircuitInstance` that triggers `Context::init_global` once.

```rust
pub struct Solver {
    circuit: CircuitInstance,
    context: Context,
    plan: ConvergencePlan,
    options: TransientAnalysisOptions, // for transient
}

impl Solver {
    pub fn new(circuit: CircuitInstance) -> Self;
    pub fn with_context(mut self, ctx: Context) -> Self;
    pub fn with_plan(mut self, plan: ConvergencePlan) -> Self;
    pub fn with_tran_opts(mut self, opts: TransientAnalysisOptions) -> Self;

    /// Build the solver. Calls `init_global` on first call. Idempotent.
    pub fn build(self) -> Self;

    // Analysis entry points
    pub fn dc(&mut self) -> crate::result::Result<DcSolver<'_>>;
    pub fn tran(&mut self) -> crate::result::Result<TransientSolver<'_>>;
    pub fn ac(&mut self, sweep: AcSweepAnalysisOptions) -> crate::result::Result<AcSolver<'_>>;
    pub fn noise(&mut self, opts: NoiseAnalysisOptions) -> crate::result::Result<NoiseSolver<'_>>;
    pub fn tf(&mut self, opts: TransferFunctionAnalysisOptions) -> crate::result::Result<TransferFunctionSolver<'_>>;

    // Accessors
    pub fn context(&self) -> &Context;
    pub fn circuit(&self) -> &CircuitInstance;
}
```

### 3. Internals crate-private

`lib.rs` today:
```rust
pub mod analysis;
pub mod analog;
pub mod core;
pub mod digital;
pub mod error;
pub mod math;
pub mod prelude;
pub mod result;
pub mod solver;
pub mod util;
```

After:
```rust
pub mod prelude;                // the only pub module
pub(crate) mod analysis;
pub(crate) mod analog;
pub(crate) mod core;
pub(crate) mod digital;
pub(crate) mod error;
pub(crate) mod math;
pub(crate) mod result;
pub(crate) mod solver;
pub(crate) mod util;

// Re-export the types a host needs at the crate root (for convenience,
// identical to prelude)
pub use prelude::*;
```

Wait — making `core` `pub(crate)` would break `piperine-codegen` which imports
from `piperine_solver::core::element::Element`. The codegen crate needs
`Element`, `ElementCapabilities`, and the introspection types. These are
already in the prelude.

Fix: add `pub use` at the `lib.rs` level for what codegen needs. Codegen uses
`use piperine_solver::core::element::Element` today — changing to `pub(crate)`
breaks it. But we can change codegen's imports to use `piperine_solver::prelude::*`
or `piperine_solver::Element`.

Decision: codegen's imports are updated to use the public paths (prelude or
crate-root re-exports). All internal paths become `pub(crate)`.

### 4. Scheduler split (3 files from 1)

Current `digital/scheduler.rs` (395 lines):
- `DigitalTopology` struct + `build` method (~90 lines)
- `DigitalState` struct + `new`/`schedule`/`checkpoint`/`rollback`/`commit` + `label_or_default` etc. (~130 lines)
- `evaluate_until_stable` method (~75 lines)
- `evaluate_dag_ordered` method (~100 lines)

Split into:

**`digital/topology.rs`:**
```rust
pub struct DigitalTopology {
    pub topo_order: Vec<usize>,
    pub back_edges: Vec<(usize, usize)>,
}

impl DigitalTopology {
    pub fn build(devices: &[Box<dyn Element>]) -> Self;
}
```

**`digital/state.rs`:**
```rust
pub struct DigitalState {
    pub nets: Vec<LogicValue>,
    pub event_queue: BinaryHeap<Reverse<DigitalEvent>>,
    labels: Vec<String>,
    checkpoint: Option<(...)>,
}

impl DigitalState {
    pub fn new(num_nets: usize) -> Self;
    pub fn with_labels(num_nets: usize, labels: Vec<String>) -> Self;
    pub fn set_label(&mut self, net: DigitalNet, label: impl Into<String>);
    pub fn label_or_default(&self, net: DigitalNet) -> String;
    pub fn schedule(&mut self, event: DigitalEvent);
    pub fn peek_next_event_time(&self) -> f64;
    pub fn checkpoint(&mut self);
    pub fn rollback(&mut self);
    pub fn commit(&mut self);
    pub fn evaluate_until_stable(...);
    pub fn evaluate_dag_ordered(...);
}
```

**`digital/scheduler.rs`** (kept, renamed concept): Actually, since both evaluation
loops are methods on `DigitalState`, the split results in `state.rs` holding all the
state + evaluation, and `topology.rs` holding only the DAG builder.

Better split: evaluation loops go in `digital/scheduler.rs` (they orchestrate
state + topology), and they take `&mut DigitalState` + `&DigitalTopology` as
parameters. This way `scheduler.rs` is the orchestrator, `state.rs` is the
data + lifecycle, and `topology.rs` is the DAG structure.

But `evaluate_until_stable` is currently a method on `DigitalState`. Making it
a free function requires refactoring. Let me keep it simple:

Split:
- `digital/topology.rs` — `DigitalTopology` + `build`
- `digital/state.rs` — `DigitalState` + all methods except evaluation loops
- `digital/scheduler.rs` — `evaluate_until_stable` and `evaluate_dag_ordered`
  as `pub(crate) fn` that take `(&mut DigitalState, &mut [Box<dyn Element>],
  &DigitalTopology, PlanLimits, &[f64])`

Wait, MD-13 rule 2 says no loose functions! These free fns would violate.
Better: make them methods on a new `Scheduler` struct, or keep them on
`DigitalState` in `state.rs`.

Let me just do the minimum: `DigitalTopology` → `topology.rs`. `DigitalState`
and evaluation loops stay in `scheduler.rs` (but it's named after the primary
concept it holds — the state). Or rename `scheduler.rs` to `state.rs` and keep
the evaluation loops there.

Best approach per MD-13 rule 4 (modules by system function):
- `digital/topology.rs` — DAG structure
- `digital/state.rs` — `DigitalState` (data + lifecycle + evaluation)
- `digital/scheduler.rs` — thin re-export, or the module that ties them together

Actually, let me not overthink this. The design says "split into separate files"
and the spec AC says "scheduler.rs shall contain only the evaluation loops".
Let me do exactly that:

- `digital/topology.rs` — `DigitalTopology` + `build`
- `digital/state.rs` — `DigitalState` (new, checkpoint, rollback, commit, labels, schedule, peek)
- `digital/scheduler.rs` — evaluation loops: `evaluate_until_stable` and `evaluate_dag_ordered`

The evaluation loops become `pub(crate) fn` in `scheduler.rs`. MD-13 rule 2 says
no loose functions — but these are the scheduler's methods. In the next phase,
they become methods on a `Scheduler` struct. For now, they're `pub(crate)` fns
in a module called `scheduler`. This is acceptable as an incremental step.

### 5. `as_iv` decoupled

Change `DcAnalysisResult::as_iv(&self, netlist: &Netlist)` to
`as_iv(&self, circuit: &CircuitInstance)`. The implementation walks
`circuit.netlist()` internally.

### Files touched

| File | Change |
|------|--------|
| `lib.rs` | `pub(crate) mod` for internals; re-export prelude at root |
| `core/builder.rs` | NEW — CircuitBuilder |
| `solver/solve.rs` | NEW — Solver (or add to `solver/mod.rs`) |
| `prelude.rs` | Add `CircuitBuilder`, `Solver` |
| `digital/topology.rs` | NEW — extract `DigitalTopology` + `build` |
| `digital/state.rs` | NEW — extract `DigitalState` lifecycle |
| `digital/scheduler.rs` | Keep evaluation loops only |
| `digital/mod.rs` | Update re-exports |
| `analysis/dc.rs` | `as_iv(&CircuitInstance)` |
| `solver/transient.rs` | Update `as_iv` call |
| `piperine-codegen/src/device/*` | Update imports to public paths |
| `piperine-bench/src/*` | Update imports to public paths |

### Migration path

1. **Scheduler split** — pure move, no behavior change
2. **Internals crate-private** — mechanical import updates in codegen/bench
3. **as_iv decoupled** — one signature change
4. **Solver struct** — new entry point
5. **CircuitBuilder** — new builder (can be last, it's additive)
