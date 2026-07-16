# Solver ABI — Design

**Spec**: `.specs/features/solver-abi/spec.md`
**Status**: Draft
**Decisions honored**: MD-01 (one Element ABI), MD-04 (Tolerances/Policy — done),
MD-06 (init_global as Once), MD-11/12 (OSDI checklist / ABI-vs-policy),
MD-13 (idiom rules — esp. 2 "no loose functions", 4 "modules by system
function", 5 "no macros").

All file/line references verified 2026-07-16 on `feature/plugin-architecture`
(392 tests green). If a line number has drifted at execution time, locate the
named item by symbol, not by line.

---

## Architecture Overview

```
        HOST (bench, python, user code)             DEVICE AUTHOR (codegen, plugins, test doubles)
                    │                                            │
        use piperine_solver::prelude::*;             use piperine_solver::abi::*;
                    │                                            │
   CircuitBuilder ──build()──► CircuitInstance ◄──── impl Element (Stamp, states, EvalCtx, Noise)
                    │
   Solver::new(circuit).with_context(..).with_policy(..).build()
                    │
        .dc() .tran() .ac(..) .noise(..) .tf(..)
                    │
        DcAnalysisResult / TransientAnalysisResult / ... (+ SolverStats)

   Everything else in piperine-solver: pub(crate).
```

Two audiences, two modules, nothing else public. The element lifecycle
becomes explicit:

```
construct → allocate_unknowns(alloc)   [CircuitBuilder::build, pre-freeze]
          → setup(ctx)                 [first analysis constructor, once]
          → update/load_* loops        [existing, unchanged]
          → destroy()                  [CircuitInstance::drop, once]
```

---

## Current-State Inventory (what exists, verified)

| Item | Where today | State |
|------|-------------|-------|
| `lib.rs` modules | `crates/piperine-solver/src/lib.rs` | 9 `pub mod`: analysis, analog, core, digital, error, math, prelude, result, solver |
| `prelude.rs` | `src/prelude.rs` | host-oriented exports incl. `CircuitInstance`, `Context`, `Policy`, `Tolerances`, `ConvergenceHint`, results, options, `Net`, `Error` |
| `digital/scheduler.rs` | 403 lines | `DigitalTopology`+`build` (lines 19–99), `Checkpoint` (9–17), `DigitalState` data+lifecycle (101–183), `evaluate_until_stable` (194–268), `evaluate_dag_ordered` (270–end) — all one file |
| `as_iv` | `src/analysis/dc.rs` (`pub fn as_iv(&self, netlist: &Netlist)`) | one call site: `solver/transient.rs::compute_initial_conditions` (`dc_result.as_iv(netlist)`) |
| `Element` trait | `src/core/element.rs` | has: capabilities, limiting_active, convergence_hint, bound_step_hint, next_breakpoints, initial_conditions, read_opvars, list_params/get_param/set_param(→Invalidation), list_queries/query, list_terminals, set_temperature, update, accept_timestep(state, t, nets, sink), load_dc/ac/transient, noise_current_psd, digital methods, suggest_transient_step. **Missing:** setup, destroy, allocate_unknowns |
| `ElementCapabilities` | `src/core/element.rs` | 12 flags through `BYPASS_OK`. **Missing:** LINEAR, STAMPS_CHARGE, ANALYTIC_JACOBIAN |
| `TerminalDescriptor` | `src/core/introspect.rs:168-175` | name, domain, direction, required. **Missing:** discipline, sign |
| `Noise` | `src/analysis/noise.rs:6-9` | terminals + value only. **Missing:** name, kind |
| Noise per-source | `src/solver/noise.rs::solve` lines 97–123 | accumulates `step_density` only — per-source data is computed then thrown away (the loop already has element + per-source gain²·PSD in hand) |
| `Netlist` | `src/analog/netlist.rs` | `connect_node(NodeIdentifier) -> AnalogReference`, `connect_branch(BranchIdentifier) -> AnalogReference`, `max_index()`, gnd-family names normalize to ground |
| `Context::init_global` | `src/solver/mod.rs` | `static INIT: Once`; called by every analysis constructor (idempotent) |
| `CircuitInstance` | `src/core/circuit.rs` | `from_devices_and_netlist`, `rebuild_digital_topology`, `init_digital`, `update_all`, `apply_convergence_hints`, analysis entry methods `dc/ac/noise/transfer_function/transient` |
| Analysis solvers | `src/solver/{dc,transient,ac}.rs` | each carries `pub policy: Policy` (set by hosts post-construction) |
| Baseline tests | workspace | 392 passing; solver integration tests in `crates/piperine-solver/tests/{digital_topology,mixed_signal}.rs` + `tests/helpers/mod.rs` |

### Downstream import inventory (what `pub(crate)` breaks)

Verified by grep. These are ALL the external `piperine_solver::` paths in use:

| Consumer | Internal paths used today | Replacement |
|----------|--------------------------|-------------|
| piperine-codegen | `solver::Context`, `solver::{Context, Policy}`, `math::circular_array::CircularArrayBuffer2`, `math::linear::Stamp`, `math::linear::{AsIndex, Stamp}`, `math::integration::{TrBdf2, TrBdf2Phase}`, `digital::interface::{DigitalPorts, EvalCtx, EventSink}`, `digital::DigitalNet`, `digital::{DigitalNet, LogicValue}`, `digital::DigitalEvent`, `core::element::{Element, ElementCapabilities}`, `core::introspect::{…}`, `analysis::transient::{TransientAnalysisContext, TransientAnalysisState}`, `analysis::noise::Noise`, `analysis::dc::{DcAnalysisResult, DcAnalysisState}`, `analysis::ac::AcAnalysisContext`, `analog::{BranchIdentifier, NodeIdentifier}`, `analog::AnalogReference`, `analog::{AnalogReference, BranchIdentifier, Netlist, NodeIdentifier}`, `core::circuit::CircuitInstance` | `piperine_solver::abi::{…}` |
| piperine-bench | `solver::Context`, `solver::{Context, Policy}`, `analysis::*` results/options, `analog::{Netlist, NodeIdentifier}`, `core::circuit::CircuitInstance`, `digital::{DigitalNet, LogicValue}`, `analysis::transient::TransientAnalysisResult`, `analysis::dc::DcAnalysisResult` | `piperine_solver::prelude::{…}` (host role); `abi::` only where it touches `Netlist`/states |
| piperine-python | `result::SolverStats` | `piperine_solver::prelude::SolverStats` |
| piperine-solver `tests/` | `analog::{AnalogReference, Netlist, NodeIdentifier}`, `analysis::dc::DcAnalysisState`, `analysis::transient::TransientAnalysisOptions`, `core::circuit::CircuitInstance`, `core::element::{Element, ElementCapabilities}`, `digital::…` (incl. `interface::QueueSink`, `scheduler::{DigitalState, DigitalTopology}`), `math::circular_array::CircularArrayBuffer2`, `math::linear::Stamp`, `solver::Context` | `piperine_solver::abi::{…}` (tests are device authors + hosts) |
| piperine-plugin, piperine-lang | none (dependency exists, no source imports) | no change |

**Design consequence:** `abi` must export everything in the codegen + tests
rows. `QueueSink` and `DigitalState`/`DigitalTopology` are used by tests →
export them in `abi` (test doubles drive the scheduler directly; that is a
legitimate device-author/harness concern).

---

## Code Reuse Analysis

| Component | Location | How used |
|-----------|----------|----------|
| `Netlist::connect_node/connect_branch` | `analog/netlist.rs` | `CircuitBuilder` + `UnknownAllocator` wrap these — no new allocation logic |
| `CircuitInstance::from_devices_and_netlist` + `rebuild_digital_topology` + `init_digital` | `core/circuit.rs` | `CircuitBuilder::build` composes them |
| `static INIT: Once` in `solver/mod.rs` | existing | `Solver::build()` calls `Context::init_global()`; nothing new |
| `pub policy: Policy` fields on Dc/Transient/Ac solvers | delivered by solver-convergence-performance | `Solver` sets them from its own `Policy` |
| Adjoint loop in `solver/noise.rs::solve` | lines 97–123 | per-source accumulation slots into the existing inner loop (element name + per-source term already in scope) |
| Introspection test pattern (`Resistor` test double) | `core/introspect.rs` tests | template for lifecycle/allocator/noise test doubles |
| Existing prelude | `src/prelude.rs` | grows; nothing removed |

---

## Components

### 1. `abi` module (NEW: `src/abi.rs`)

- **Purpose**: The device-author surface — everything needed to implement
  `Element`, in one flat import.
- **Location**: `crates/piperine-solver/src/abi.rs`, declared `pub mod abi;`
  in `lib.rs`.
- **Contents** (pure re-exports, no new types):

```rust
//! The device-author surface: everything needed to implement [`Element`].
//! Hosts use [`crate::prelude`]; element implementors use this module.

// The contract
pub use crate::core::element::{ConvergenceHint, Element, ElementCapabilities};
pub use crate::core::circuit::CircuitInstance;
pub use crate::core::introspect::{
    Bounds, Direction, Domain, Invalidation, NoiseKind, ParamDescriptor, ParamError,
    ParamScope, QueryDescriptor, QueryKind, SignConvention, TerminalDescriptor,
    Value, ValueKind,
};
// Stamping + naming
pub use crate::math::linear::{AsIndex, Stamp};
pub use crate::analog::{
    AnalogReference, AnalogVariable, BranchIdentifier, Netlist, NodeIdentifier, GND,
};
// Solution history + per-analysis states/contexts
pub use crate::math::circular_array::CircularArrayBuffer2;
pub use crate::analysis::ac::AcAnalysisContext;
pub use crate::analysis::dc::{DcAnalysisResult, DcAnalysisState};
pub use crate::analysis::noise::Noise;
pub use crate::analysis::transient::{
    TransientAnalysisContext, TransientAnalysisOptions, TransientAnalysisState,
};
// Integration (kernels read phase/coeffs)
pub use crate::math::integration::{IntegrationMethod, TrBdf2, TrBdf2Phase};
pub use crate::math::unit::Second;
// Digital evaluation
pub use crate::digital::interface::{DigitalPorts, EvalCtx, EventSink, QueueSink};
pub use crate::digital::{DigitalEvent, DigitalNet, LogicValue};
pub use crate::digital::state::DigitalState;
pub use crate::digital::topology::DigitalTopology;
// Run config + results device code touches
pub use crate::solver::{Context, Policy, Tolerances};
pub use crate::result::{Result, SolverStats};
pub use crate::error::{Error, SolverDomain};
// Element lifecycle allocator (ABI-09)
pub use crate::core::builder::UnknownAllocator;
```

  (Exact list is normative — a task's done-when checks it compiles the
  `tests/abi_surface.rs` element. If an item is discovered missing during
  execution, ADD it to `abi.rs` and note it in the task's commit body — do
  not reach for an internal path.)

- **Dependencies**: none (re-exports only).

### 2. `prelude` additions (MODIFY: `src/prelude.rs`)

Adds (keeps everything already there): `CircuitBuilder`, `Solver`,
`SolverStats`, `NoiseKind` + noise result types
(`NoiseAnalysisResult` already exported — verify), `AcSweepAnalysisOptions`
(already there). Remove nothing.

### 3. `lib.rs` privacy flip (MODIFY)

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

Rust rule check: `pub use` re-exports FROM a `pub(crate)` module are legal
and elevate item visibility — the items themselves are `pub` in their
modules. No `private_interfaces` warnings expected; if one appears, the leaked
type gets added to `abi`/`prelude` explicitly.

### 4. Scheduler split (MODIFY `digital/`, MD-13 rules 2+4)

- **`digital/topology.rs`** (NEW): `DigitalTopology` + `impl` (`build`) moved
  verbatim from `scheduler.rs:19-99`.
- **`digital/state.rs`** (NEW): `Checkpoint` (private) + `DigitalState`
  struct + `impl DigitalState` with `new`, `with_labels`, `set_label`,
  `label_or_default`, `schedule`, `peek_next_event_time`, `checkpoint`,
  `rollback`, `commit` — moved verbatim from `scheduler.rs:9-17,101-183`.
- **`digital/scheduler.rs`** (SHRINKS): keeps ONLY
  `impl DigitalState { pub fn evaluate_until_stable(..) .. pub fn evaluate_dag_ordered(..) }`
  — a second impl block on a struct defined in `state.rs` (same crate: legal).
  No free functions. File ends < 250 lines.
- **`digital/mod.rs`**: `pub mod topology; pub mod state;` + re-export
  `DigitalState`, `DigitalTopology` at `crate::digital::{state,topology}` AND
  keep `crate::digital::scheduler::DigitalState` path working internally via
  `pub use` in `scheduler.rs`? **No** — simpler: fix the (few) internal
  `crate::digital::scheduler::DigitalState` references to
  `crate::digital::state::DigitalState`. Grep-driven, mechanical.

### 5. `as_iv` decoupling (MODIFY `analysis/dc.rs`, `solver/transient.rs`)

```rust
// analysis/dc.rs
pub fn as_iv(&self, circuit: &CircuitInstance) -> Vec<InitialValue<AnalogReference, f64>> {
    let netlist = circuit.netlist();
    // body unchanged
}
```

Call site `solver/transient.rs::compute_initial_conditions`: delete the local
`let netlist = self.system.circuit.netlist();` binding used for `as_iv` and
pass `&*self.system.circuit`. (The other `netlist` uses in that fn remain.)

### 6. `Solver` (NEW: `src/solver/solve.rs`)

```rust
/// The host entry point: owns the circuit + run configuration, initializes
/// the process globals once (MD-06), and hands out the five analyses.
pub struct Solver {
    circuit: CircuitInstance,
    context: Context,
    policy: Policy,
    tran_opts: TransientAnalysisOptions,   // default: TransientAnalysisOptions::new(1e-3, 1e-6) — verify ctor
    built: bool,
}

impl Solver {
    pub fn new(circuit: CircuitInstance) -> Self;              // defaults: Context/Policy::default()
    pub fn with_context(mut self, ctx: Context) -> Self;
    pub fn with_policy(mut self, policy: Policy) -> Self;
    pub fn with_tran_opts(mut self, opts: TransientAnalysisOptions) -> Self;
    /// Initializes process globals (tracing, faer) via `Context::init_global`
    /// — guarded by `Once`, so repeated builds are free. Returns self (moves on).
    pub fn build(mut self) -> Self;                            // sets built = true
    pub fn dc(&mut self) -> Result<DcSolver<'_>>;              // sets .policy = self.policy.clone()
    pub fn tran(&mut self) -> Result<TransientSolver<'_>>;     // uses self.tran_opts.clone(); sets policy
    pub fn ac(&mut self, sweep: AcSweepAnalysisOptions) -> Result<AcSweepRun<'_>>; // see note
    pub fn noise(&mut self, opts: NoiseAnalysisOptions) -> Result<NoiseSolver<'_>>;
    pub fn tf(&mut self, opts: TransferFunctionAnalysisOptions) -> Result<TransferFunctionSolver<'_>>;
    pub fn circuit(&self) -> &CircuitInstance;
    pub fn context(&self) -> &Context;
}
```

**Note on `ac`:** `AcSolver::new(circuit, context)` exists and
`solve_sweep(opts)` takes the sweep; `Solver::ac` simply constructs
`AcSolver` (policy set) and returns it — the `sweep` param is NOT needed at
construction. Final signature: `pub fn ac(&mut self) -> Result<AcSolver<'_>>`
(host calls `.solve_sweep(opts)` next, mirroring today's flow). Same for
noise/tf which take opts at construction — keep their existing constructor
shapes. **Rule for the implementing agent: mirror the existing
`CircuitInstance::{dc,transient,ac,noise,transfer_function}` signatures
exactly; `Solver` adds only ownership + policy threading + `build()`.**

`build()` before any analysis is NOT enforced by panic — analyses also call
`init_global` themselves (`Once`). `built` exists only for the builder-flow
test to assert; keep the field private with a `#[cfg(test)]` accessor or
drop the field if unused after tests are written (implementer's choice —
do not leave dead code).

### 7. `CircuitBuilder` + `UnknownAllocator` (NEW: `src/core/builder.rs`)

```rust
/// Safe, discoverable circuit assembly. Wraps the manual Netlist API.
pub struct CircuitBuilder {
    title: String,
    netlist: Netlist,
    nodes: HashMap<String, AnalogReference>,   // name → reference (incl. "gnd" → ground)
    elements: Vec<Box<dyn Element>>,
    digital_labels: Vec<Option<String>>,       // index = DigitalNet(i)
}

impl CircuitBuilder {
    pub fn new(title: impl Into<String>) -> Self;
    /// Ground reference. Idempotent.
    pub fn ground(&mut self) -> AnalogReference;
    /// Named analog node. Same name → same reference (idempotent lookup).
    /// gnd-family names ("gnd"/"GND"/"vss"/"VSS") route to ground.
    pub fn node(&mut self, name: &str) -> AnalogReference;
    /// Digital net with optional label. Returns DigitalNet(index), sequential.
    pub fn digital_net(&mut self, label: Option<&str>) -> DigitalNet;
    /// Store an element (insertion order preserved).
    pub fn element(&mut self, element: Box<dyn Element>) -> &mut Self;
    /// Freeze: run allocate_unknowns for every element (ABI-09, checks
    /// HAS_INTERNAL_UNKNOWNS), assemble CircuitInstance, build digital
    /// topology, init digital devices.
    pub fn build(self) -> crate::result::Result<CircuitInstance>;
}

/// Pre-freeze internal-unknown allocation seam handed to
/// `Element::allocate_unknowns`. Constructed ONLY by `CircuitBuilder::build`
/// (and `CircuitInstance` internal paths) — not exported in prelude, exported
/// in abi so elements can name the type in their signatures.
pub struct UnknownAllocator<'a> {
    netlist: &'a mut Netlist,
    allocated: usize,   // count for the HAS_INTERNAL_UNKNOWNS check
}

impl<'a> UnknownAllocator<'a> {
    /// Allocate an auxiliary branch unknown (component, name) → reference.
    pub fn branch(&mut self, component: &str, name: &str) -> AnalogReference;
    /// How many unknowns this element allocated (read by build()'s check).
    pub fn allocated(&self) -> usize;
}
```

`build()` sequence (normative):
1. For each element in insertion order: fresh `UnknownAllocator` over
   `&mut netlist` → `element.allocate_unknowns(&mut alloc)`; if
   `alloc.allocated() > 0 && !element.capabilities().contains(HAS_INTERNAL_UNKNOWNS)`
   → `Err(Error::simple(SolverDomain::…, "element `<name>` allocated internal unknowns without declaring HAS_INTERNAL_UNKNOWNS"))`
   (pick the closest existing `SolverDomain` variant — inspect the enum; do
   NOT add a variant unless none fits).
2. `CircuitInstance::from_devices_and_netlist(title, elements, netlist)`.
3. Apply digital labels via `digital_state.set_label` after sizing digital
   state to `digital_labels.len()` — **inspect how `DigitalState::new`
   receives net count in `from_devices_and_netlist` first**; if the count is
   currently hardwired to 0, extend `from_devices_and_netlist` or add a
   builder-only constructor that sizes it. Fail loud if labels don't fit.
4. `instance.rebuild_digital_topology(); instance.init_digital()?;`
5. Return instance.

### 8. Element lifecycle (MODIFY `core/element.rs`, `core/circuit.rs`, 4 analysis constructors)

```rust
// on Element (defaults preserve behavior):
/// One-time binding hook: called exactly once per circuit lifetime, after
/// unknown allocation, before the first analysis assembles. FFI wrappers
/// bind handles here. Errors abort analysis construction (fail loud).
fn setup(&mut self, _ctx: &Context) -> crate::result::Result<()> { Ok(()) }

/// Teardown: called exactly once when the CircuitInstance drops.
/// Must not panic.
fn destroy(&mut self) {}

/// Pre-freeze internal-unknown allocation (ABI-09). Elements that allocate
/// MUST declare `HAS_INTERNAL_UNKNOWNS`. Default: allocates nothing.
fn allocate_unknowns(&mut self, _alloc: &mut UnknownAllocator<'_>) {}
```

`CircuitInstance` gains:

```rust
is_set_up: bool,   // private field, false at construction

/// Run Element::setup once per circuit. Idempotent; called by every
/// analysis constructor before its Newton dry-run.
pub(crate) fn setup_all(&mut self, ctx: &Context) -> crate::result::Result<()> {
    if self.is_set_up { return Ok(()); }
    for d in self.devices.iter_mut() { d.setup(ctx)?; }
    self.is_set_up = true;
    Ok(())
}

impl Drop for CircuitInstance {
    fn drop(&mut self) {
        for d in self.devices.iter_mut() { d.destroy(); }
    }
}
```

Call sites for `setup_all(ctx)`: first statement (after `init_global`) in
`DcSolver::new`, `TransientSolver::new`, `AcSolver::new`, `NoiseSolver::new`,
`TransferFunctionSolver::new` — before any `NewtonRaphsonSolver::new`
(dry-run assemble). AC/noise/tf construct a `DcSolver` internally, which
would set up first — the `is_set_up` guard makes the outer call a no-op;
keep both for uniformity.

### 9. Introspection extensions (MODIFY `core/introspect.rs`)

```rust
/// OSDI terminal sign convention: which current direction is positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignConvention {
    /// Positive current flows into the terminal (the OSDI default).
    IntoTerminal,
    /// Positive current flows out of the terminal.
    OutOfTerminal,
}

pub struct TerminalDescriptor {
    pub name: String,
    pub domain: Domain,
    pub direction: Direction,
    pub required: bool,
    pub discipline: Option<String>,   // NEW — e.g. "electrical"
    pub sign: SignConvention,         // NEW — default IntoTerminal
}

impl TerminalDescriptor {
    /// name/domain/direction with defaults: required=true, discipline=None,
    /// sign=IntoTerminal.
    pub fn new(name: impl Into<String>, domain: Domain, direction: Direction) -> Self;
}
```

Migrate every existing `TerminalDescriptor { .. }` struct-literal site
(grep `TerminalDescriptor {`) to `::new(..)` + field overrides where needed.

### 10. Noise metadata + per-source reporting (MODIFY `analysis/noise.rs`, `solver/noise.rs`)

```rust
// analysis/noise.rs
/// What physical mechanism a noise source models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoiseKind { Thermal, Shot, Flicker, Other }

pub struct Noise {
    pub terminals: (AnalogReference, AnalogReference),
    pub value: AmpereSquaredSecond,
    pub name: Option<String>,   // NEW
    pub kind: NoiseKind,        // NEW
}

impl Noise {
    /// Anonymous source (name None, kind Other) — the shape existing
    /// emitters produce; they migrate to this constructor.
    pub fn new(terminals: (AnalogReference, AnalogReference), value: AmpereSquaredSecond) -> Self;
    pub fn named(mut self, name: impl Into<String>, kind: NoiseKind) -> Self; // builder-style
}

/// One source's identity + integrated contribution over the sweep.
#[derive(Debug, Clone)]
pub struct NoiseContribution {
    pub element: String,        // Element::name()
    pub source: String,         // Noise::name or the source's index as string
    pub kind: NoiseKind,
    /// Integrated output-referred contribution (same integration as the
    /// total: trapezoidal over the sweep, sqrt at the end NOT applied —
    /// store the mean-square integral so contributions sum to total²).
    pub integrated_sq: f64,
    /// Per-frequency output-referred PSD (same length as `frequencies`).
    pub psd: Vec<f64>,
}

pub struct NoiseAnalysisResult {
    pub frequencies: Vec<f64>,
    pub out_noise_sq: Vec<f64>,
    pub integrated_noise: f64,
    pub contributions: Vec<NoiseContribution>,   // NEW
}

impl NoiseAnalysisResult {
    pub fn contributions(&self) -> &[NoiseContribution];
}
```

`solver/noise.rs::solve` change (inside the existing frequency loop, lines
97–123): the inner `for n in noises` already computes
`gain_sq * n.value` per source — additionally accumulate it into a
`HashMap<(String, String), Vec<f64>>` keyed by
`(element.name().to_string(), n.name.clone().unwrap_or_else(|| idx.to_string()))`
where `idx` is the source's position in that element's returned vec. Each
key's vec grows one entry per frequency (push 0.0-filled up to the current
frequency index if a source appears late — sources may be
frequency-dependent; missing = 0.0). After the loop, integrate each source's
psd with the SAME `integrate_noise` trapezoid (but without sqrt — see
`integrated_sq` doc) and build `contributions`.

**Conservation invariant (test):** at every frequency index `i`,
`Σ_source psd[i] == out_noise_sq[i]` within reltol 1e-9.

Emitter migrations (grep `Noise {` across workspace — codegen's
`device/analog.rs` builds `Noise { terminals, value }`): switch to
`Noise::new(..)`. Codegen can name sources later; not this feature's job.

### 11. Capability flags (MODIFY `core/element.rs`)

```rust
/// Stamps are solution-independent (pure linear element). Declared, not yet
/// consumed — the solver-performance follow-up uses it for bypass/single-LU.
const LINEAR = 1 << 12;
/// Contributes a charge (`ddt`) part — has reactive stamps.
const STAMPS_CHARGE = 1 << 13;
/// The Jacobian is exact (analytic/symbolic), not finite-difference.
const ANALYTIC_JACOBIAN = 1 << 14;
```

Codegen `PiperineDevice::capabilities()`: always OR `ANALYTIC_JACOBIAN`
(symbolic diff); OR `STAMPS_CHARGE` when the compiled kernel has a Q part —
inspect `AnalogKernel`/`CompiledModule` for the existing "has charge"
predicate (the flattener splits resistive vs charge contributions; there is
a per-module charge list — locate it, e.g. the kernel's charge-slot count
> 0). If no clean predicate exists, set `STAMPS_CHARGE` conservatively
whenever the kernel was built with any `ddt` contribution and note the
predicate used in the commit body.

---

## Error Handling Strategy

| Error scenario | Handling | Caller sees |
|----------------|----------|-------------|
| `Element::setup` fails | Analysis constructor returns the `Error`; no assemble runs | `Err` from `circuit.dc(ctx)` / `Solver::dc()` with the element's message |
| Allocation without `HAS_INTERNAL_UNKNOWNS` | `CircuitBuilder::build` returns `Err` naming the element | Build-time failure, fail loud |
| Digital labels exceed digital net sizing | `CircuitBuilder::build` returns `Err` | Build-time failure |
| `destroy` panics in Drop | Documented "must not panic"; no catch_unwind | Standard panic-in-drop semantics |
| Missing item in `abi`/`prelude` discovered downstream | Add the re-export (never re-open module privacy) | Compile error during migration task, fixed in-place |

---

## Risks & Concerns

| Concern | Location | Impact | Mitigation |
|---------|----------|--------|------------|
| Privacy flip breaks unknown external users of `piperine_solver` internals | workspace-external | Compile breakage outside repo | Workspace grep is the source of truth; external crates (piperine-osdi) must migrate to `abi` — note in commit + STATE.md |
| `Drop for CircuitInstance` interacts with analysis solvers holding `&mut CircuitInstance` | `core/circuit.rs` | None at compile time (Drop runs when owner drops); but bench forks circuits per analysis — destroy runs per circuit instance, so per-fork FFI teardown must be safe to repeat across *different* instances | Document: `destroy` is per-instance; wrappers own per-instance handles |
| `setup_all` inside AC/noise/tf which nest a `DcSolver::new` | `solver/{ac,noise,tf}.rs` | Double-setup risk | `is_set_up` guard is on the circuit, not the solver — inherently idempotent |
| Scheduler split can silently change visibility of `Checkpoint` | `digital/state.rs` | none (private struct moves whole) | Move verbatim; grep `Checkpoint` after move |
| Per-source noise map allocates in the frequency loop | `solver/noise.rs` | Noise is not the hot path (bounded sweep); acceptable | Note in code; no premature optimization |
| `TerminalDescriptor`/`Noise` field additions break the external piperine-osdi repo | external | External churn | Constructors (`::new`) keep migration one-line; note in commit body |
| tests import `digital::scheduler::{DigitalState, DigitalTopology}` — paths move | `tests/digital_topology.rs` | Test compile breakage | Migration task updates test imports to `abi::` (assertions untouched — spec ABI-05 AC4) |

---

## Tech Decisions (non-obvious only)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Two-tier surface | `prelude` (host) + `abi` (device author) | Different audiences; keeps host prelude small; user-confirmed |
| Evaluation loops stay methods on `DigitalState` in a split impl block | `impl DigitalState` in `scheduler.rs` | Satisfies MD-13 rules 2 AND 4 with zero call-site churn |
| `UnknownAllocator` not in prelude | abi-only export | Hosts never allocate unknowns; compile-time prevention of post-freeze allocation |
| Per-source noise stores mean-square integral (`integrated_sq`) | not sqrt'd | Contributions must sum to the total; sqrt breaks additivity |
| Capability flags declared but unconsumed | documented on the flags | MD-11 checklist scope; consumption is solver-performance |
| `Solver::build` returns `Self` (not a distinct built type) | plain struct, no typestate | MD-13 rule 3 (simple > clever); `Once` already guarantees safety |

> Project-level: the two-tier surface is a new convention → record as AD
> entry in `.specs/STATE.md` on delivery ("public surface = prelude + abi;
> everything else pub(crate)").
