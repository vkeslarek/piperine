# SOLVER_GAPS.md — ngspice features vs. the piperine solver

Audit of what ngspice-46 does that the piperine native solver
(`crates/piperine-solver/`) does **not** yet do, so nothing surprises us
later. Ordered by impact. Status: **DONE** (landed), **PARTIAL** (works in
some cases), **MISSING** (absent). Work items get checked off here as they
land — each entry names *where* in the solver and *why it matters*.

Cross-validation against ngspice lives in
`~/Git/plugins/piperine-spice/validation/` — run it after any solver change.

---

## 1. Solver/device ABI refactor

- [x] **Uniform simulation element ABI — DONE (2026-07-13).** The `Device`
  downcast wrapper and the separate `AnalogDevice`/`DigitalDevice` facet traits
  are gone. There is now **one** solver-facing contract, `Element`
  (`core/element.rs`): identity (`name`), a required `capabilities()`
  descriptor (`ElementCapabilities` bitflags), and every operation — analog
  load (`load_dc`/`load_ac`/`load_transient`/`noise_current_psd`), analog
  lifecycle (`limiting_active`/`bound_step_hint`/`initial_conditions`/`update`/
  `accept_timestep`), and digital evaluation (`boundary`/`init`/`seq_phase`/
  `comb_phase`/`evaluate`) — as methods with defaults. A resistor implements
  the analog subset, a gate the digital subset, a comparator both, over one
  object; there is no downcast. `AnalogInstance`/`DigitalInstance` are driven
  by the composite `PiperineDevice` (which delegates), and `DigitalNetwork` is
  itself a degenerate digital `Element`. Consumers (DC/AC/transient/noise/TF
  drivers, the scheduler) call the methods directly and gate on
  `capabilities()`. Everything below in this bullet is the original rationale.

  The old split (`Device` as a wrapper plus separate analog and digital
  facets) made mixed-signal models feel bolted together instead of native.
  A real mixed-signal element needs one coherent evaluation boundary: analog
  loading may depend on digital state, and digital event generation may depend
  on analog state/history. Today those data paths exist in ad-hoc places
  (`accept_timestep`, scheduler snapshots, D2A cached state), but the ABI does
  not state them as first-class inputs. Refactor toward a single solver-facing
  **Element** contract with optional capabilities, not a separate top-level
  `Device` plus per-domain downcasts.

  Proposed shape: every element receives a unified `SolveCtx` and exposes a
  capability descriptor. `SolveCtx` should include current analysis, time,
  tolerances, temperature, analog solution history, digital state snapshot,
  pending event sink, parameter/query access, and resolved terminal ids. The
  element then implements only the operations it supports: analog load,
  small-signal load, noise sources, digital eval, accept/commit, rollback, and
  queries. Pure analog/pure digital are degenerate cases of the same ABI; a
  comparator, DAC, sampled switch, or mixed Verilog-A/RTL peripheral is not a
  special bridge object.

  Why it matters: mixed-signal coupling is not one-directional. A switched-cap
  device stamps different analog equations from digital phase state; a
  comparator emits digital events from analog history/crossings; a timer or
  debounced edge detector may need both the analog state history and the digital
  net snapshot. The ABI should make these dependencies explicit so device
  authors do not hide them in side caches.

- [x] **Bidirectional mixed-signal state in every relevant callback — DONE
  (2026-07-13).** Analog stamping now sees the digital snapshot and digital
  evaluation sees the analog solution, both at the ABI level — no device-side
  cache. `DcAnalysisState`/`TransientAnalysisState` carry `digital:
  &[LogicValue]` (they deref to the analog history), so `load_dc`/`load_transient`
  read the exact digital state being solved against; `EvalCtx` already carries
  `analog: &[f64]` for A2D. `accept_timestep` still takes the accepted digital
  nets. Remaining nicety: unify the three views under one `SolveCtx` — deferred,
  not required for correctness.

- [x] **Uniform analog/digital naming and node designation — DONE
  (2026-07-13).** `core/net.rs::Net` is the unified public identity of any
  solved signal: a dense index paired with a `NetKind` (`Node`/`Branch`/
  `Digital`/`Pseudo`), a stable label, and — for analog nets — the originating
  `Arc<AnalogVariable>` so result types can look up solved values by `Net`
  without an extra index map. `From<&AnalogReference>` and `From<DigitalNet>`
  convert both fast-path types in; ground is a `Pseudo` net with no index.
  `Netlist::nets()` and `CircuitInstance::nets()` enumerate every analog and
  digital signal symmetrically as `Net`, and `Net` is exported in the prelude.
  **Hierarchical source labels for digital nets** land through
  `DigitalState::set_label` (the circuit builder injects; absent that,
  `label_or_default` returns `d{idx}`). The label survives `checkpoint` /
  `rollback`. **Result mapping via `Net`** lands through `DcAnalysisResult::
  get_net`, `AcAnalysisStep::get_net`, and `TransientStep::{get_net,
  digital_net}` — every result type now answers both the legacy
  `AnalogReference`/`DigitalNet` accessor and the new `&Net` accessor; the
  former stay as fast paths, the latter is the host-facing surface.

- [~] **Parameter and query ABI comparable to OSDI — PARTIAL (2026-07-13).**
  `core/introspect.rs` adds the OSDI-style metadata/query contract, exposed as
  optional (defaulted) `Element` methods:
  - **parameter descriptor** — `ParamDescriptor { name, kind, default, unit,
    bounds, scope (Model/Instance), invalidation }`; `Invalidation` is
    `None`/`Restamp`/`Temperature`/`OperatingPoint`/`Rebuild` so sweeps recompute
    the minimum correct amount;
  - **query descriptor** — `QueryDescriptor { name, kind, unit, description }`
    with `QueryKind` (operating variable, terminal voltage/current, internal
    state, event counter, limiting state);
  - **terminal descriptor** — `TerminalDescriptor { name, domain, direction,
    required }`;
  - **runtime access** — `list_params`/`get_param`/`set_param` (typed
    `ParamError`), `list_queries`/`query` (default-derived from `read_opvars` so
    any element with opvars is queryable), `list_terminals`; `Value` is
    real/integer/boolean/text.

  A reference `Element` (resistor with param `r` + opvar `g`) tests the contract
  end-to-end, and codegen's `PiperineDevice` now exposes its **real** JIT
  parameters: `list_params`/`get_param`/`set_param` delegate to `AnalogInstance`
  (param names from the kernel, values from the instance), so a bench/host reads
  and writes a compiled device's params at run time — a write restamps, no
  rebuild. Tested against a compiled PHDL resistor (`from_ir.rs`). Still to do:
  model descriptor (type id/version), noise-source metadata, real opvar/terminal
  catalogs from the kernel (the kernel exposes param and terminal *indices* but
  not opvar names yet).

  Why it matters: bench sweeps, plugin UIs, OSDI wrappers, and debugging should
  not special-case every device family. A BJT should expose `gm`, `gpi`, `vbe`,
  limiting state, and noise contributors through the same query path that a
  digital peripheral exposes event counts or register state.

- [ ] **OSDI-inspired ABI details still missing — MISSING.**
  The native ABI should not copy OSDI blindly, but OSDI has mature interface
  ideas that are worth absorbing because they solve real model-integration
  problems. Add these to the refactor target:

  - **Explicit lifecycle.** Define ordered hooks for model setup, instance setup,
    temperature preprocessing, load/evaluate, accept/commit, rollback, and
    destroy. Today setup, update, load, and accept semantics are spread across
    separate traits and callers. A model wrapper should be able to rely on one
    lifecycle chart per analysis.

  - **Richer terminal descriptors.** Terminals need declared domain,
    discipline/storage kind, direction, required/optional status, current sign
    convention, and whether they are external, internal, or auxiliary. This is
    necessary for automatic diagnostics, current queries, OSDI wrappers, and
    future internal-node allocation.

  - **Internal unknown allocation.** External models often require auxiliary
    branch currents, internal nodes, hidden states, or equation rows. The loader
    needs a pre-finalization allocation API so the matrix shape is still frozen
    before analysis starts, but models are not limited to source-level terminals.

  - **Operating-point variable catalog.** `read_opvars()` should become a
    declared catalog: name, type, unit, description, owner (model/instance), and
    query cost. This lets bench, CLI, plugins, and UIs discover `gm`, `gds`,
    `vbe`, register state, event counters, and limiter state without device
    family knowledge.

  - **Noise metadata.** Noise sources should carry stable names, type
    (`thermal`, `shot`, `flicker`, or custom), terminal pair, units, and whether
    the contribution is individually reportable. The noise solver should be able
    to return per-source contributions, not only total output PSD.

  - **Temperature protocol.** Separate nominal temperature, instance temperature,
    delta temperature, and temperature-dependent parameter preprocessing. The
    ABI should state whether a temperature change requires recomputing model
    constants, rebuilding matrix structure, or only restamping numeric values.

  - **Parameter invalidation rules.** Every parameter descriptor should say what
    changes when the parameter changes: no-op metadata, numeric restamp only,
    temperature recompute, operating-point restart, topology/matrix rebuild, or
    full device reconstruction. This makes bench sweeps and optimization loops
    fast and correct.

  - **Formal limiting API.** Device limiting should expose proposed values,
    limited values, limiter name, active/inactive state, and reason. The solver
    can then report why convergence is blocked and can make limiting part of the
    convergence contract rather than a hidden device cache.

  - **Discontinuity and breakpoint notifications.** A model should be able to
    request a breakpoint, force timestep reduction, mark a discontinuity, or
    request temporary integration-order reduction. `$bound_step` alone is too
    weak for piecewise sources, hard switches, and event-driven analog models.

  - **Jacobian/stamp capability declaration.** A model should declare whether it
    provides analytic derivatives, numeric derivatives, linear stamps, charge
    Jacobians, AC linearization, and noise derivatives. Missing derivative data
    should be a validation error for analyses that need it, not a late silent
    accuracy bug.

  - **Model/instance query separation.** Queries and parameters should state
    whether they belong to the shared model descriptor or one instance. This is
    important for compact-model libraries where thousands of instances share one
    model card but have different terminal state and instance geometry.

  Design rule: use OSDI as a checklist for integration maturity, not as the
  solver's native ABI. Piperine's ABI should remain mixed-signal-first and
  domain-uniform; OSDI wrappers should be one client of it.

- [ ] **Model/instance separation — MISSING.**
  External ABIs commonly distinguish model parameters shared by many instances
  from instance parameters and runtime state. The solver ABI should make this
  explicit: a `ModelHandle` owns immutable or rarely-changing model metadata;
  an `ElementInstance` owns terminals, instance params, cached state, and stamps.
  This avoids duplicating large descriptor data and gives parameter sweeps a
  clean rule for when a circuit rebuild is required.

- [ ] **ngspice-inspired solver architecture details still missing — MISSING.**
  ngspice has mature integration ideas that are worth absorbing alongside the
  OSDI-style metadata work. Each item below is tagged as either **ABI** (must
  live in the element/solver contract) or **solver policy** (the algorithm
  controls it, not individual devices).

  - **Run control / analysis state machine. (solver policy)** ngspice has a
    explicit setup → op → resume → accepted → rejected → restart loop with
    well-defined transitions. The transient/DC pipeline has those today but as
    inline logic. The solver should expose an analysis state machine so the
    same machinery backs DC, transient, homotopy, and future analyses.
  - **Analysis-specific element callbacks declared per analysis. (ABI)**
    `load_dc`, `load_tran`, `load_ac`, `load_noise` exist, but the ABI should
    state which callbacks run in which analysis and in which order. Plugins and
    external models should be able to advertise which analyses they support
    via the capability table.
  - **Per-device convergence tests. (ABI, optional)** A device that maintains
    internal hidden state (junction voltages, latch state, charge history) may
    report whether its own internal equations are consistent. This is opt-in
    data, not solver policy: the solver still owns global convergence, but a
    device can flag that its contribution is not yet trustworthy. Pair with the
    limiting API.
  - **Device-side bypass as an ABI capability. (ABI)** Devices that detect their
    terminal conditions barely changed should be able to tell the solver their
    last stamps remain valid for this Newton iteration. Today this is not an
    ABI concept; making it one gives a uniform speedup path for resistor-like
    through nonlinear models, without requiring each solver to reinvent it.
  - **Ask/set APIs for parameters and queries at run time. (ABI)** Comparable to
    OSDI's `evaluate`/`param_access`, but ABI-uniform: `get_param`, `set_param`,
    `query` go through the same metadata catalog. Bench sweeps, optimization
    loops, and CLI inspection then look the same.
  - **Save/probe selection protocol. (ABI)** Devices should declare which
    variables/opvars are observable and at what cost. The result layer can
    request only what the user/bench asked for. Without this, traces can be
    huge and noisy.
  - **Sparse matrix naming/debug hooks fed by the unified name layer. (ABI,
    solver policy)** Errors like “singular row: branch vsrc.x1” need names. The
    uniform naming layer should also surface those names in solver diagnostics;
    the solver itself is responsible for emitting the diagnostic message.
  - **Per-element hidden-state vector with rollback. (ABI, solver policy)**
    Elements may own a private state vector, sized at construction. The solver
    drives checkpoint/rollback/commit on that vector; the element just declares
    its size and owns the bytes. This unifies how internal state is restored on
    rejected steps.
  - **Convergence plan as explicit solver policy. (solver policy)** *Homotopy
    part DONE (2026-07-13):* gmin stepping and source stepping are now
    `HomotopyStrategy` implementations composed by a `ConvergencePlan`
    (`solver/convergence.rs`) that the DC driver runs via a `HomotopyDriver`
    it implements — the inline `match … Err => match …` cascade is gone.
    Still to fold in: Newton damping/limiting and transient step rejection as
    their own strategies (`NewtonStrategy`/`StepperStrategy`).
  - **Diagnostic verbosity hooks. (solver policy)** Tracing/debug options per
    analysis, per element, per homotopy phase. Today these are ad-hoc
    environment variables; the solver policy should formalize them so plugins,
    CLI, and embedded hosts can opt in.

  Classification rule: anything describing what the element *is* or *knows*
  goes in the ABI; anything the solver *decides* goes in solver policy. Per-
  device convergence tests stay in the ABI because the element must volunteer
  them, but the solver still gates the outer loop on global convergence.

- [x] **Capability discovery instead of downcast discovery — DONE
  (2026-07-13).** The `Device` downcast wrapper is now `Element`
  (`core/element.rs`), and `ElementCapabilities` is the first-class
  capability descriptor returned by `Element::capabilities()`. The bitflags
  cover coarse grain (`ANALOG`, `DIGITAL`, `SAMPLES_ANALOG`) and the finer
  per-analysis dependencies added in this commit:
  - **per-analysis participation** — `LOADS_DC`, `LOADS_AC`, `LOADS_TRAN`,
    `EMITS_NOISE` (subset of `ANALOG`);
  - **cross-domain dependencies** — `DEPENDS_ON_DIGITAL` (D2A bridges);
  - **loader / ABI capabilities** — `HAS_INTERNAL_UNKNOWNS`,
    `SUPPORTS_ROLLBACK`, `SUPPORTS_QUERIES`.
  **`SAMPLES_ANALOG` wire-up**: `CircuitInstance::accept_and_run_digital`
  now forwards the accepted analog solution to the digital scheduler via the
  new `run_digital_at_with_analog(t, solution)`; `EvalCtx.analog` carries
  it to elements that declared the flag (A2D bridges, comparators,
  threshold detectors). The transient driver still calls `run_digital_at`
  for events fired *before* the analog solve, where no analog solution
  exists yet. Real consumers today: `CircuitInstance::capabilities()`
  unions the descriptor (DC mixed-signal loop, scheduler skip-non-digital,
  `init_digital` filter); per-analysis routing of AC/Noise/TF still
  iterates by trial today (the next PR can replace the iterate-by-trial
  with a flag-gated loop). No call sites claim `LOADS_DC`/`LOADS_AC`/etc.
  yet — those land when JIT-compiled devices start declaring which
  analyses they ship for.

  Design rule: capabilities describe what the element *does*, not where it came
  from. A JIT-compiled PHDL block, a Rust plugin, an OSDI wrapper, and a future
  co-sim peripheral should all advertise through the same table.

- [ ] **Commit/rollback lifecycle for all mixed-signal state — MISSING.**
  Transient already checkpoints digital state around candidate steps, but mixed
  devices can also keep analog event detector state, D2A cached state, delayed
  digital outputs, random-source state, or co-sim state. Add lifecycle hooks for
  `checkpoint`, `rollback`, and `commit` at the element level. A rejected
  timestep must restore every stateful participant, not only the global digital
  net array.

  Why it matters: if an A2D model records a crossing, a D2A model updates a
  latch, or an external MCU advances its firmware during a rejected step, the
  retry is no longer deterministic unless that state rolls back too.

- [ ] **Unified event model — MISSING.**
  Digital events, analog crossing events, timer events, breakpoints, and
  `$bound_step`/step-limit hints are related scheduling constraints but live in
  different places. Introduce one event/breakpoint abstraction with event kind,
  target signal, time, priority, source element, and rollback behavior. Digital
  value changes are one event kind; analog breakpoints and crossing guards are
  others.

  The solver should use the same queue/planner to decide: the next transient
  endpoint, whether a zero-delay digital delta is pending, whether an analog
  discontinuity forces a step boundary, and which elements need evaluation.

- [ ] **Refactor plan.**
  1. Define the new names/descriptors first: terminal ids, solver variable ids,
     labels, parameter descriptors, query descriptors, and capability table.
  2. Introduce the unified context and element traits alongside the old ABI.
     Adapter-wrap existing analog and digital devices so behavior stays green.
  3. Move mixed-signal bridges to the unified ABI first; they exercise both
     directions and will expose missing context fields immediately.
  4. Move plugin/external model construction to resolved device specs with
     parameter/query descriptors.
  5. Replace scheduler and solver planning from trait downcasts to capability
     descriptors.
  6. Remove the old `Device` downcast layer once all in-tree devices and plugin
     wrappers use the new ABI.

### 1.1 Architectural decisions (locked)

These are the binding decisions for §1. They are recorded here so the refactor
is not relitigated in every PR.

- **Naming layer is unified as `Net`.** A `Net` is the public identity of a
  solved signal: a node, a branch current, a digital net, an operating
  variable, or a pseudo variable. The `dense: usize` is the fast path; a
  `kind: NetKind` and a stable `label` are paired for diagnostics, queries,
  and result mapping. `Ground` is a `Net::Pseudo` with `dense = usize::MAX`.
  `Net` replaces both `AnalogReference` (at the public boundary) and
  `DigitalNet(usize)` (at the public boundary). The two remain available as
  ergonomic aliases that delegate to `Net`.
- **Per-analysis context, shared `Context`.** `Context` (the global one) only
  carries what every analysis shares: `Tolerances`, `IntegrationMethod`,
  `Temperature { nominal, instance, delta }`, `Verbosity`. Everything analysis-
  specific — `dt_min`, `dt_max`, `adaptive`, `record_from`, `breakpoints`,
  `sweep` config, `initial_guess` — lives in an `AnalysisContext` enum:
  `DcContext`, `AcContext`, `TransientContext`, `NoiseContext`, `TfContext`.
  This mirrors how OSDI/ngspice pass an analysis-specific struct to the
  model.
- **`Context` is split into `Tolerances` and `Policy`.** The shared `Context`
  holds `Tolerances` (immutable for a run). `Policy` — homotopy scales, step
  bounds, retry counters, transient state, `src_scale`, `gmin_extra` — is
  mutable state held by the active `ConvergencePlan` and strategies, not by
  the global `Context`. Strategies do not reach into a magic `Context.gmin_extra`
  field; they own their policy.
- **`init_global` stays as a `Once`.** `tracing` and `faer` need a one-time
  process-wide initialization. `Context::default` does not trigger it; the
  first `Solver::build()` (or equivalent) does. The global init function is
  the documented entry point. This is OSDI/ngspice convention.
- **State machine is solver policy.** The analysis state machine
  (setup → op → resume → accepted → rejected → restart) is a `SolverPolicy`
  composed of `NewtonStrategy`, `HomotopyStrategy`, and `StepperStrategy`
  capabilities. Each analysis picks the strategies it needs. The current
  `if-else` chain in `DcSolver::solve` and the literal `MAX_MS_ITER` go
  away.
- **TF keeps the explicit error for current-source input.** It is not a gap
  to fix; it is a documented limit. Replace the prefix heuristic with a
  clearer error message; do not introduce a new capability.
- **`Device` wrapper is removed in one pass.** Adapter-wrap existing analog
  and digital devices, move `core/circuit.rs` to the new `Element` ABI, and
  remove `core/device.rs::Device` once nothing references it. Big-bang is
  acceptable.

### 1.2 Phased plan (macro)

Six phases. Each phase ends in a green test run. Phases are not skippable.

- **Phase 0 — Decisions (this section).** Lock `Net`, the `Context` split,
  and the strategy composition. Decide the prelude surface.
- **Phase 1 — Minor refactors.** Remove dead code, simplify math layer,
  split `Context` (no behavior change), add `prelude`. (Details in §7.)
- **Phase 2 — Naming layer.** Introduce `Net`, replace public `AnalogReference`
  / `DigitalNet` at the boundary, route diagnostics through it.
- **Phase 3 — `Element` ABI.** New `Element` trait with `ElementCapabilities`
  descriptor, unified load/eval/accept/rollback/commit contexts, capability-
  based scheduler, `SignalBridge` internal component. Adapter-wrap existing
  devices.
- **Phase 4 — Strategy composition.** `ConvergencePlan`, `NewtonStrategy`,
  `HomotopyStrategy`, `StepperStrategy` traits. DC and transient drivers
  become thin strategies; per-analysis `AnalysisContext` enum carries
  analysis-specific tunables.
- **Phase 5 — Library ABI / Prelude.** `Circuit` builder, `Solver::build()`,
  public analyses, result types. Internals become crate-private. The
  prelude exposes exactly what a host needs to build, run, and query.
- **Phase 6 — Legacy removal.** Delete `Device`, the analog/digital facets,
  dead analyses, `FaerDenseLinearSystem`, `AcAnalysisSolver`,
  `AcFrequencyAnalysisOptions`, `truncation.rs` traits, `Scalar`, `UnitExt`.
  `Context::gmin_extra` and `Context::src_scale` are gone (moved into the
  policy of the active strategy).

### 1.3 Order of work (dependencies)

| Phase | Depends on | Validation |
|-------|-----------|------------|
| 0 | — | Decisions locked in this document |
| 1 | 0 | `cargo build --workspace` zero warnings; `cargo test --workspace` green |
| 2 | 1 | Tests that use `AnalogReference`/`DigitalNet` still green via aliases |
| 3 | 2 | Bridges continue converging; new tests for capability-based load |
| 4 | 3 | `ConvergencePlan::default()` reproduces today's results for DC and transient |
| 5 | 4 | Host-level smoke test (build + run + query) |
| 6 | 5 | Legacy deleted; full test suite still green |

---

## 7. Minor refactors (Rust idiomático, dead code, cleanup)

These are small, mechanical, behavior-preserving changes. They ship as one or
two PRs before the ABI refactor in §1. None of them change simulation
semantics. None of them is a gap in solver features — they are about
removing items that violate the Rust idiom rules (every method has an owner,
every API is a contract or a capability, the code reads at a glance).

### 7.1 Dead code to delete

- [x] `math/faer.rs::FaerDenseLinearSystem` — **DONE (2026-07-13).** Deleted the
  type and both trait impls; production only ever used `FaerSparseLinearSystem`.
- [x] `math/faer.rs::FaerToNdarray` — **KEEP.** The extension trait (owned by
  `Col`) is the idiomatic form; both `Col` and `Array1` are foreign so a `From`
  impl is impossible (orphan rule). Resolved, no action.
- [x] `math/linear.rs::NoSymbolic` — **DONE (2026-07-13).** Deleted with
  `FaerDenseLinearSystem`, its only user.
- [x] `analysis/ac.rs::AcAnalysisSolver` — **DONE (2026-07-13).** Deleted; no
  implementor existed.
- [x] `analysis/ac.rs::AcFrequencyAnalysisOptions` — **DONE (2026-07-13).**
  Deleted; no caller.
- [x] `analysis/truncation.rs::TruncationError` and `BreakpointProvider` traits —
  **MOVED to `math/integration.rs` (2026-07-13).** The file is gone; the traits
  and `IntegrationMethod` now live under `math/` alongside the companion
  coefficient formula. Wire-up into the transient stepper is still Phase 4.
- [x] `math/faer.rs::FaerSymbolicMatrix::size` field made private — **DONE
  (2026-07-13).** Trait method kept as the only accessor; both call sites
  (`solver/noise.rs`, `solver/tf.rs`) already use `symbolic_matrix.size()`.

### 7.2 Math layer simplifications

- [x] `math/linear.rs::AsIndexGetExt` — **DONE (2026-07-13).** Deleted; it had
  no callers at all.
- [x] `math/faer.rs::FaerSparseLinearSystem::solve` — **DONE (2026-07-13).**
  Removed the orphan `solve` (no non-backend callers) and dropped `solve` from
  the `LinearSystem` trait; only `solve_with_backend` remains.
- [ ] `math/linear.rs::SymbolicMatrix` — `size` is a method on the trait, but
  `FaerSymbolicMatrix` also exposes `pub size: usize`. Make the field private;
  keep the method. (Pending.)
- [x] `math/num.rs::Scalar` — **KEEP.** `Scalar` is implemented for both `f64`
  and `Complex<f64>` and carries `faer::ComplexField`; a `num_traits::Float`
  bound would exclude `Complex`. The trait is correct as written. Resolved.
- [x] `math/unit.rs` `UnitExt` — **DONE (2026-07-13).** Deleted the
  `paste!`-generated `UnitExt` trait and dropped the `paste` dependency; the 4
  call sites (`.pS()`, `.Hz()`) were inlined to literals. **Deviation:** kept
  the plain `pub type` SI aliases (`Ohm`, `Siemens`, `Second`, …) — they are
  not macro magic and they keep solver signatures readable at a glance
  (rule 4). Inlining ~100 alias sites to bare `f64` is pure churn deferred
  to Phase 5. `constant.rs` never used `UnitExt`, only the aliases.

### 7.3 Solver policy struct (`Context` split)

- [x] **Homotopy state out of `Context` — DONE (2026-07-13).** `gmin_extra` and
  `src_scale` no longer live on the shared `Context`. `gmin_extra` is a field of
  `DcSystem` (the gmin-stepping homotopy owns it); `src_scale` travels to
  elements through `DcAnalysisState::src_scale`. `Context` now carries only
  immutable per-run settings (tolerances, temperature, integration method,
  `time`). Full `Tolerances`/`Policy` sub-structs land with the strategy FSM
  (§1 phase 4), which becomes the owner of retry counters and step bounds.
- `Context::default` does not call `init_global`. `Solver::build` does. (Pending
  the public API in §1 phase 5.)

### 7.4 Magic numbers and shared tunables

- [x] `solver/dc.rs::solve` `MAX_MS_ITER` literal — **DONE (2026-07-13).** The
  cap moved into `ConvergencePlan::PlanLimits::max_mixed_signal_iter`
  (default 20). Hosts can override via
  `ConvergencePlan::default().with_limits(...)`. The DC driver reads
  `plan.limits().max_mixed_signal_iter`.
- [x] `solver/transient.rs::execute_timestep` dead `alpha` parameter — **PARTIAL
  (deferred).** The `alpha` is still passed and ignored by `assemble`; the
  trait parameter stays until the `NewtonStrategy`/`StepperStrategy` split
  in Phase 4 removes the `NonLinearSystem` trait as the cross-analysis
  abstraction. Literals like `1e-15`/`min_step` remain inline; they'll
  follow `PlanLimits` once the transient stepper becomes a strategy.
- [x] `digital/scheduler.rs::evaluate_dag_ordered` `MAX_ITERS` and `log::warn!`
  — **DONE (2026-07-13).** Both `evaluate_dag_ordered` and
  `evaluate_until_stable` now take a `PlanLimits` and return
  `Result<(), Error>` (`SolverDomain::Digital`). The cap is named
  `max_delta_cycles`; the time-equality epsilon is `digital_time_epsilon`.
  `CircuitInstance::run_digital_at` and `accept_and_run_digital` propagate
  the result; DC and transient drivers `?`-propagate.
- [x] `math/iv.rs::InitialValueApplyExt::apply_iv` panic — **DONE
  (2026-07-13).** Replaced `panic!` with `debug_assert!`. Out-of-bounds is a
  programming invariant (the circuit builder cannot ship an unknown
  reference), not user input. Loud in tests, no-op in release per
  AGENTS.md.

### 7.5 Layout and module boundaries

- [ ] `digital/scheduler.rs` mixes `DigitalTopology`, `DigitalState`, and the
  scheduler. Split into `digital/topology.rs`, `digital/state.rs`,
  `digital/scheduler.rs`. (Deferred to Phase 5; the file is currently the
  single owner of all three concepts and splitting it is best done alongside
  the prelude clean-up that follows.)
- [x] Glob reexports removed — **DONE (2026-07-13).** `analog/mod.rs`,
  `digital/mod.rs`, `core/mod.rs` now export explicit named items instead of
  `pub use …::*;`. `solver/mod.rs`, `analysis/mod.rs`, `math/mod.rs` never
  used glob reexports. `lib.rs` exposes a `prelude` module that hosts the
  full public surface.
- [x] `port.rs` moved into `core/` — **DONE (2026-07-13).** `crates/piperine-solver/src/core/port.rs`
  is now the home of `Port`; re-exported as `piperine_solver::core::Port`
  (and via the prelude once Phase 5 finalises the surface).
- [x] `lib.rs` exports a **`prelude`** module — **DONE (2026-07-13).**
  `src/prelude.rs` re-exports the host-facing surface: `CircuitInstance`,
  `Context`, `Element`/`ElementCapabilities`, the option and result types, the
  naming types (`AnalogReference`/`Netlist`/`DigitalNet`/`LogicValue`),
  `ConvergencePlan`/`HomotopyStrategy`, and `Error`/`Result`. Additive (existing
  paths still work); removing the glob re-exports and making internals private
  is the remaining Phase-5 step.

### 7.6 Error model

- [x] `SolverDomain` enum — **DONE (2026-07-13).** `error.rs::SolverDomain`
  enumerates `Dc`, `Ac`, `Transient`, `Noise`, `Tf`, `Digital`, `Bridge`,
  `Newton`, `Linear`, `SpaceMatrix`, `Element`. `Error::simple`/`Error::cause`
  now take a `SolverDomain` as the title type; tyops are compile errors. Every
  solver callsite migrated (DC, AC, TF, noise, transient scheduler, faer
  linear, Newton).
- [x] `crate::result::Result` consistent — **DONE (2026-07-13).** Solver
  modules use `crate::result::Result` everywhere; the only remaining
  `std::result::Result` in the crate is the definition of the alias itself in
  `result.rs`.

### 7.7 Result and analysis layering

- [ ] `analysis/dc.rs::DcAnalysisResult::as_iv(&Netlist)` exposes the netlist
  through the analysis API. Replace with `as_iv(&SolverContext)` or move
  the helper into the solver crate. Analysis types should not depend on
  `Netlist` directly. (Deferred: no caller in the workspace yet; lands with
  Phase 5 when the prelude decides the analysis type's surface.)
- [ ] `solver/noise.rs::integrate_noise` is a trapezoidal integration inlined
  into the noise driver. Add an `Integrator` trait (already partly exists
  as `IntegrationMethod` in `analysis/truncation.rs`) and reuse it for
  transient, noise, and any future `.four` / `.disto` post-processing.
  (Deferred: the trapezoidal transient is not wired yet; an `Integrator`
  trait shared across analyses lands once both call sites exist.)

### 7.8 Heuristics in solver code

- [x] `solver/tf.rs::is_voltage_source` — **DONE (2026-07-13).** Replaced by
  `input_is_voltage_source() -> Option<bool>` which returns `None` for any
  branch label that doesn't start with `V` or `I`. `calculate_gain` now
  errors loud (`SolverDomain::Tf`) for any non-`V` input — including an
  explicit "TF: current-source input is not supported (D5)" message instead
  of the old "not yet fully implemented". `calculate_input_resistance` no
  longer reads the heuristic (voltage-only is guaranteed by `calculate_gain`).
  No `SourceKind::Voltage` capability yet — that lands with the model
  descriptor work in Phase 4.

### 7.9 Bridge ownership

- `core/circuit.rs::CircuitInstance::accept_and_run_digital` does three jobs:
  builds a `CircularArrayBuffer2` from the current solution, seeds a digital
  event queue, and runs the digital scheduler. Extract a `SignalBridge`
  component owned by `CircuitInstance`. It is internal, not part of the
  prelude. Phases 3 and 4 land this; for now, this is the file to watch.

---

## 2. Nonlinear convergence

- [x] **Current-residual convergence test (ngspice `NIconvTest`) — DONE
  (2026-07-12).** ngspice requires BOTH a small voltage step AND a small
  per-node current residual. piperine used to check only the voltage step,
  so stiff exponential devices (BJT/MOS) stopped at non-solutions (the BJT
  settled in the active region where ngspice saturates; MOS drain current
  ~1.5× off). Added `NonLinearSystem::residual_converged` +
  `DcSystem::residual_converged` (`solver/mod.rs`, `solver/dc.rs`): the
  Newton loop computes `(A·v − b)` = the KCL imbalance from the assembled
  companion stamps and ANDs a per-row tolerance (`abstol` for node rows,
  `vntol` for branch rows, plus `reltol·scale`) into the convergence test.

- [~] **Source stepping — PARTIAL (implemented 2026-07-12; BJT amplifier
  blocked on the BJT *model*, not the homotopy).**
  `solver/dc.rs::solve_source_stepping` ramps `Context.src_scale` 0 → 1
  (forced-voltage values scale in `force_stamps`) with a 1 µS knee shunt that
  is then itself ramped out — a nested source+gmin homotopy. It converges more
  circuits, but the BJT common-emitter (`validation/bjt_ce`) still stalls at
  the exponential turn-on knee (scale ≈ 0.375, base ≈ 0.75 V) regardless of
  the homotopy. **Root cause is the BJT model, not the solver:** the pnjlim
  limiter is not engaging (`vnew == vlim`, i.e. never clamps — its `vcrit`
  seed is too high for `is=1e-16`), so the base-emitter exponential jumps
  uncontrolled and Newton diverges through the knee. Fix belongs with the BJT
  model work (pnjlim `vcrit` / limiting engagement) — pairs with the MOS1
  model bug. The homotopy machinery is in place and correct; it just needs a
  working device limiter under it.

- [x] **gmin stepping — DONE (2026-07-11).** `solver/dc.rs::solve_gmin_stepping`
  — node-to-ground conductance ramped 0.1 S → 0 with adaptive back-off, on
  plain-Newton failure. Makes the coupled-junction devices converge (to the
  gmin-homotopy solution; source stepping is still needed for the correct
  branch on some circuits).

- [ ] **Junction/device GMIN and `gshunt` — PARTIAL.** Models add `gmin·v`
  at their own junctions (e.g. the diode/BJT leakage terms), but there is no
  circuit-wide `gshunt` option or a diagonal GMIN the user can raise. Low
  priority once source stepping lands.

- [ ] **Convergence step limiting / `damping` beyond pnjlim — PARTIAL.**
  `apply_damping` halves the whole update vector past a 0.5 V step; ngspice
  also has per-device voltage limiting (pnjlim/fetlim/limvds). pnjlim is in
  (`$limit`); `fetlim`/`DEVlimvds` are identity (MOS converges via gmin
  stepping without them, but tight ngspice parity may want them). See ROADMAP
  B5.

---

## 3. Transient integration

- [x] **Gear (BDF) integration — DONE (2026-07-12).** The reactive companion
  (`device/analog.rs`) now uses a BDF formula
  `dQ/dt ≈ c0·Q_n + c1·Q_{n-1} + c2·Q_{n-2}` with non-uniform-step
  coefficients (`bdf_coeffs`), selectable via `Context.integration`
  (default **Gear order 2**). Order ramps 1 → 2 over the first steps
  (`TransientSystem.step_index`, `dt_prev`). Backward-Euler over-damped
  ringing; Gear-2 preserves it — an ideal LC tank holds amplitude
  (v after one period 0.9986 vs ideal 1.0; RC discharge 1.1062 vs analytic
  1.1036). **Trapezoidal wired (2026-07-13):** `IntegrationMethod::coeffs`
  now returns `(2/dt, −2/dt, 0)` for Trapezoidal, and the solver kernel
  (`codegen/device/analog.rs`) calls the centralised `bdf_coeffs(method, order,
  dt, dt_prev)` via `tran_ctx.integration`. A host selects it with
  `Context.integration = IntegrationMethod::Trapezoidal`. Order is 2 always;
  history depth is not needed (two-point formula). Gear-1/2 uniform and
  non-uniform paths are unchanged. A user can still choose
  `IntegrationMethod::Gear { order: 2 }` (the default).

- [ ] **Local truncation error timestep control — PARTIAL.** `math/integration.rs`
  contains the `TruncationError` trait and `IntegrationMethod` with its LTE
  coefficient; verify it uses the charge/LTE estimate ngspice does (`trtol`,
  `chgtol`) and that it interacts correctly once trapezoidal lands (trap needs
  the DD2 estimate, Euler a different one).

- [ ] **Breakpoints — MISSING.** ngspice forces a timepoint exactly at every
  source discontinuity (pulse edges, PWL corners) so the integrator never
  steps across a kink. piperine relies on adaptive stepping + `$bound_step`
  hints; add a breakpoint table fed by source models.

- [x] **`@initial` / UIC device initial conditions — DONE (2026-07-12).**
  `@initial { V(p,n) <- ic; }` now seeds the t=0 branch voltage
  (cap/ind/dio `.ic`). Flattener collects instance-constant potential forces
  in the `@initial` event (`FlatAnalog.initial_conditions`), the kernel
  compiles the values, `AnalogDevice::initial_conditions` reports them, and
  `solver/transient.rs::compute_initial_conditions` seeds `v(plus) =
  v(minus) + ic`. (Milestone-1 seed, matching the existing user-`ic` path;
  a fully *enforced* hold via a t=0 clamp branch is a follow-up.)

- [ ] **Enforced UIC hold (`.ic` + `uic`) — PARTIAL.** The seed above sets
  the starting point; ngspice with `UIC` also *holds* the node at `ic`
  through the first solve via a large-conductance clamp (`CKTsetIC`). Add the
  clamp branch for the first timepoint, released after t=0.

---

- [x] **`ddt(I)` inductor flux companion — DONE (2026-07-12), transient AND
  AC.** A potential force `V(p,n) <- L·ddt(I(p,n))` over its own branch current
  compiles and runs: the flattener peels the flux coefficient `L`
  (`split_flux`), the kernel evaluates it, and `force_flux_stamps` adds the
  transient companion `−c0·L·ib` on the branch diagonal plus the flux history
  `L·(c1·ib_{n-1} + c2·ib_{n-2})`. `load_ac` adds the small-signal admittance
  `−jω·L·ib` on the branch equation. A short in DC (`dt = 0`). Verified: RL
  high-pass hits −3 dB at its corner (0.705 vs 0.707). Unblocks the `ind`
  model in transient and AC.
- [x] **Mutual inductance `ddt(I(other_branch))` — DONE (2026-07-12).** The
  flux is decomposed into per-branch-current terms (`split_flux` /
  `isolate_branch_coeff`); `force_flux_stamps` couples force branch `i`'s
  equation to any partner branch current (`Matrix(branch_i, branch_j,
  −c0·M)` + the partner's flux history). A single-device transformer
  (`V(p1,n1) <- L1·ddt(I(p1,n1)) + M·ddt(I(p2,n2))` and the symmetric row)
  couples energy correctly (coupled-LC test: secondary reaches 0.083 V from
  a primary tank). **Constraint:** the two windings must be *one* device —
  ngspice/piperine's separate `ind` + `mut` devices each force the same node
  pair (two ideal sources on one branch → singular). The `piperine-spice`
  `mut`/`ind` models therefore need a combined transformer block to use this
  (tracked in the model gaps).

## 4. Analyses ngspice has that piperine lacks

Present: `.op`, `.ac`, `.noise`, `.tf`, `.tran` (`analysis/` + `solver/`).

- [ ] **`.dc` sweep — MISSING as a native analysis.** ngspice sweeps a
  source/param and reports the operating point at each step. piperine does
  parameter sweeps at the *bench* layer (staging + repeated `$op`), not as a
  solver analysis. Probably fine to keep at bench level — confirm it covers
  nested sweeps and source sweeps.
- [ ] **`.four` (Fourier of a transient) — MISSING.** Post-processing of a
  `$tran` waveform; belongs as a bench task (like `extract`), not the solver.
- [ ] **`.disto` (small-signal distortion) — MISSING.** Rarely used; low
  priority.
- [ ] **`.pz` (pole-zero) — MISSING.** Needs an eigenvalue solve on the MNA
  matrix. Niche; low priority.
- [ ] **`.sens` (DC/AC sensitivity) — MISSING.** Derivative of outputs w.r.t.
  params. Could reuse the symbolic-diff infrastructure; medium value.
- [ ] **`.sp` (S-parameters, ngspice-46) — MISSING.** Port-based AC; niche.

---

## 5. Numerics / performance

- [ ] **Device bypass — MISSING.** ngspice skips re-evaluating a nonlinear
  device whose terminal voltages barely changed (`CKTbypass`). Pure speed;
  matters for large circuits. Add a per-device "inputs unchanged within
  tol → reuse last stamps" check.
- [ ] **Matrix reuse / incremental refactor — check.** piperine rebuilds the
  linear system each Newton iteration (`L::new` + `apply_stamps`). ngspice
  reuses the symbolic factorization (KLU) and only refactors numerically.
  Confirm faer's symbolic reuse is exploited (`self.symbolic` is kept, but
  `self.linear_system = L::new(...)` each iter looks like a full rebuild).
- [ ] **Predictor for the transient initial guess — PARTIAL/check.** ngspice
  extrapolates the next timepoint from history as the Newton seed (fewer
  iterations). Confirm the transient warm-starts from the previous step (the
  companion history buffer suggests yes) and whether a polynomial predictor
  would help.
- [ ] **Temperature (`.temp` / per-instance `temp`) — PARTIAL.** Models read
  `temp`/`dtemp`; confirm a global `.temp` sweep and `tnom` rescaling flow
  through every model consistently (the spice models do temperature
  preprocessing; the *analysis*-level temperature sweep is bench-side).

---

## 6. Model-equation correctness (not solver, but found by the harness)

The current-residual test proved these *converge to a KCL-consistent point*,
so they are **model-equation** bugs, not solver bugs (tracked here because the
validation harness surfaced them):

- [ ] **MOS1 drain current ~1.5× too high** (`validation/nmos_load`:
  ngspice v(d)=3.0 V, piperine 1.92 V). Likely the Shichman-Hodges
  `β = kp·W/L` / effective-width or the `kp` vs `u0·cox` default path in
  `piperine-spice/src/mos.phdl`. Check against `mos1load.c`.
- [ ] **JFET off by ~15 mV / ~1 %** (`validation/jfet_bias`: 1.382 vs
  1.397 V). Minor — a small model-detail discrepancy in `jfet.phdl`.

---

## Priority order (recommended)

1. **Solver/device ABI refactor** (§1) — make mixed signal first-class,
   unify analog/digital state visibility, naming, parameters, queries,
   capabilities, and rollback before adding more device families.
2. **Source stepping** (§2) — unblocks BJT/MOS amplifier operating points;
   pairs with the residual check already landed.
3. **Trapezoidal integration** (§3) — transient accuracy; backward-Euler
   over-damps every reactive circuit.
4. **Enforced UIC hold** (§3) — completes the `@initial` work.
5. **Breakpoints** (§3) — transient correctness at source edges.
6. **Device bypass / matrix reuse** (§5) — performance, once correctness is
   solid.
7. The niche analyses (`.pz`, `.disto`, `.sens`, `.sp`) — on demand.
