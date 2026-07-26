# Part VII — Solver Specification

This Part defines the solver contract: the element ABI consumed by analyses, the
analog and digital variable namespaces, the numerical algorithms for DC, AC,
transient, noise, transfer-function, sensitivity, and periodic-steady-state
analysis, and the convergence aids that
make mixed-signal simulation deterministic.

The solver is below elaboration and device construction. It receives a fixed set
of elements, a fixed analog variable map, and a fixed digital net namespace. It
does not know source syntax or elaboration internals. A compiled PHDL module, a
plugin device, and an external model are equivalent once they present the one
`Element` ABI defined here.

## Contents

- §1 Position and governing rules
- §2 Circuit instance
- §3 Element ABI — analog operations
- §4 Element ABI — digital operations
- §5 Element loading and external models
- §6 Analog variable and node labels
- §7 Digital net labels and logic values
- §8 Stamping and MNA
- §9 DC operating point
- §10 Transient analysis
- §11 AC analysis
- §12 Noise analysis
- §13 Transfer-function analysis
- §14 Mixed-signal execution
- §15 Convergence aids
- §16 Validation and failure rules
- §17 Sensitivity analysis
- §18 Periodic steady state
- §19 Lifecycle contract — per-analysis hook chart

---

## §1 Position and governing rules

The solver executes an already-built circuit. Structure is immutable during an
analysis: devices may change their internal state and stamp values, but they may
not create or delete devices, nodes, branches, or digital nets.

Three rules govern this Part.

**Element ABI only.** The solver communicates with every model through the one
`Element` contract and its declared capabilities. A model's origin is not
observable by the solver.

**Fail loud.** A model or analysis that cannot produce a faithful stamp or event
must report an error. It must not emit a silent zero stamp, ignore an unmodeled
effect, or continue with a no-op substitute.

**Deterministic mixed signal.** Analog convergence and digital event settling are
ordered by a fixed protocol. Zero-delay digital logic settles by delta cycles;
analog steps are accepted only after the analog solve succeeds and the coupled
digital boundary has been serviced according to §14.

---

## §2 Circuit instance

A circuit instance is the complete solver input:

| Field | Meaning |
|-------|---------|
| title | Human-readable circuit identity. |
| devices | Ordered list of all analog, digital, and mixed-signal devices. |
| analog netlist | Mapping from analog variables to dense MNA unknown indices. |
| digital state | Current logic value of every digital net and pending digital events. |
| digital topology | Optional topological order over digital devices, with back-edge restart data for cyclic combinational dependencies. |

The device order is stable for the lifetime of the circuit. Event provenance may
refer to this order, and deterministic ties use a monotonic sequence number.

The circuit instance (`CircuitInstance`) exposes its surface grouped into five
contracted responsibilities; every public method belongs to exactly one:

| Responsibility | Contents |
|----------------|----------|
| Circuit state | Read-only views of the built circuit: the analog netlist, the unified net list, digital labels, the capability union (the OR of every element's `ElementCapabilities`), and device access. |
| Analysis entry | One uniform entry point per analysis — `dc`, `ac`, `transient`, `noise`, `transfer_function`, `sens`, `pss` — each handing a driver a borrow of the circuit plus a `Context`. |
| Mixed-signal seam | The one place analog acceptance seeds digital events and the scheduler runs (§14): `init_digital`, `run_digital_at[_with_analog]`, `accept_and_run_digital`, `rebuild_digital_topology`. |
| Live mutation | The restamp path (`set_element_param`, §10.5) plus the per-solve hooks (`setup_all`, `update_all`, `apply_limiting_reports`). |
| Construction | None — construction stays in the `CircuitBuilder`. |

Construction is the builder's job. `CircuitBuilder::build` runs each element's
`allocate_unknowns` pre-freeze allocation seam (an element that allocates
internal unknowns without declaring `HAS_INTERNAL_UNKNOWNS` fails the build),
assembles the instance, sizes and labels the digital state, rebuilds the
digital topology, and initializes the digital devices at time zero. After
construction, re-entry goes through the analysis drivers (e.g. a transient
restart from a captured step) and the restamp path — never through a new
constructor.

The circuit instance offers analyses over the same topology. A DC analysis,
transient analysis, AC sweep, noise analysis, transfer-function analysis,
sensitivity analysis, and periodic-steady-state analysis all consume the same
device set and analog/digital namespaces. The `Solver` facade is the host entry
point: it owns the circuit plus the shared run configuration (`Context` and
`Policy`) and hands out each analysis driver with that configuration applied.

---

## §3 Element ABI — analog operations

There is **one** solver-facing object, the **element**. Every participant — a
pure resistor, a logic gate, a comparator, a JIT-compiled PHDL block, a plugin,
a wrapped external model — implements the same `Element` contract and
implements only the operations it needs. There is no downcast and no `Any`.

The contract's surface is grouped by concern: `Element` is the conjunction of
three supertraits, each independently documented with every method defaulted.

```text
Element = AnalogDevice + DigitalDevice + Introspect
          + identity & cross-cutting lifecycle
```

| Supertrait | Concern |
|------------|---------|
| `AnalogDevice` | MNA loading (`load_dc`/`load_ac`/`load_transient`/`noise_current_psd`) plus the analog lifecycle and convergence/timestep hooks (this section). |
| `DigitalDevice` | The two-phase delta cycle and digital hidden-state round-trip (§4). |
| `Introspect` | OSDI-style parameters, queries, terminals, and operating variables (§3.4). |

`Element` itself keeps only identity and the cross-cutting lifecycle that is
not purely one concern:

| Method | Contract |
|--------|----------|
| `name()` | Source-level identity, for diagnostics and result mapping. |
| `capabilities()` | Required. A capability descriptor (`ElementCapabilities`) declaring what the element participates in, so the solver and scheduler plan without probing. |
| `setup(context)` | One-time initialization before the first solve, with the run context. |
| `destroy()` | Teardown when the circuit instance is dropped. |
| `accept_timestep(state, t, nets, sink)` | The analog→digital bridge hook: called after each accepted solution point at time `t`; a mixed-signal element may emit digital events through `sink`. |
| `runtime_banks()` | Runtime state/var banks for opt-in per-step recording; default empty. |

All supertrait methods default to a no-op, so a pure-analog element overrides
only its analog methods and inherits the inert digital and introspection
surfaces (the empty impl blocks are explicit — their presence documents that
the element is deliberately inert in the other concerns). The object is not
split — only its surface is grouped — and the solver never names a supertrait
to select behavior: capability flags gate, as before.

`ElementCapabilities` is a bit set:

| Flag | Meaning |
|------|---------|
| `ANALOG` | Contributes to the analog system (MNA stamps in DC/AC/transient/noise). |
| `DIGITAL` | Participates in the digital scheduler (drives/reads logic nets). |
| `SAMPLES_ANALOG` | Its digital logic reads analog node voltages, so it must be evaluated on every accepted analog solve even without a pending digital event. |
| `LOADS_DC` | `load_dc` contributes to the DC operating point. |
| `LOADS_AC` | `load_ac` contributes to the small-signal AC sweep. |
| `LOADS_TRAN` | `load_transient` contributes to time-domain integration. |
| `EMITS_NOISE` | `noise_current_psd` returns non-empty sources. |
| `DEPENDS_ON_DIGITAL` | Analog load reads the digital net snapshot (D2A). Implies `ANALOG`; a D2A-ordering descriptor for the DC and transient drivers. |
| `HAS_INTERNAL_UNKNOWNS` | The element allocated internal MNA unknowns (auxiliary branch currents, hidden states) through the `allocate_unknowns` seam during circuit construction. |
| `BYPASS_OK` | Reserved: stamp bypass is owned by the `solver-performance` follow-up. The DC driver's stamp cache (§9) is global — gated on solution movement and limiting, not on this flag. |
| `SUPPORTS_ROLLBACK` | The element overrides `checkpoint_state`/`restore_state` to snapshot/restore its mutable non-accept-gated state around rejected timesteps and DC homotopy retries (§3.1, §15.8). Default `checkpoint_state() = None` = stateless = zero cost. |

An element must declare its capabilities accurately; the solver gates analysis
and scheduling on this descriptor rather than on which methods are overridden.
Two flags gate solver control flow directly — `DIGITAL` (the mixed-signal loop,
the scheduler, digital initialization) and `HAS_INTERNAL_UNKNOWNS` (the
builder's allocation check). The remaining live flags are participation
descriptors consumed by the loaders and result mapping; the three reserved bits
name their owning follow-up and promise nothing today.

The analog operations in this section and the digital operations in §4 are all
methods of the one element. Analog methods default to contributing no stamps;
digital methods default to an element that drives no nets. A pure-analog
element leaves the digital methods at their defaults and vice versa.

An element that contributes to MNA declares `ANALOG` and implements the
`AnalogDevice` methods below: it contributes matrix and right-hand-side stamps
for one or more analyses, may expose operating variables, may emit noise
sources, and may request convergence or timestep controls.

### 3.1 Analog lifecycle methods

| Method | Contract |
|--------|----------|
| `setup(context)` | One-time initialization before the first solve (identity-resolved cross-cutting hook; also on `Element`). |
| `allocate_unknowns(alloc)` | Pre-freeze internal-unknown allocation, called once per element by the circuit builder before the matrix shape freezes (§5.2). Elements that allocate must declare `HAS_INTERNAL_UNKNOWNS`. Default no-op. |
| `set_temperature(t)` | Set the device temperature for temperature-dependent parameters. `t` is absolute temperature in kelvin. Called once during circuit construction (default nominal temperature), re-seeded during `setup_all` with the run's actual `Tolerances.temperature` (§9), and re-driven on a temperature sweep (invalidation path). Default no-op — backward compatible for elements that read `$temperature` at eval time. |
| `update(state, context)` | Refresh internal model state from the current analog solution history before loading stamps. |
| `initial_conditions()` | Return requested initial branch voltages as `(plus, minus, value)` tuples. A missing terminal means ground. |
| `limiting_report()` | Structured limiting feedback: `Option<LimitingReport { net, proposed, limited_value, limiter_name, reason }>`. `is_some()` gates Newton convergence (replaces the old `limiting_active: bool`); the solver applies `limited_value` to `net` in the Newton guess before the convergence test (replaces the old `convergence_hint`). Default none. |
| `checkpoint_state()` | Return an opaque `ElementCheckpoint { int_state, real_state }` snapshot of the element's mutable non-accept-gated state (limiter active/seeds/vold, digital registers), taken before each transient attempt and each DC homotopy attempt. Restored on rejection; discarded on acceptance. Default `None` = stateless = zero cost. |
| `restore_state(ckpt)` | Restore a snapshot previously produced by `checkpoint_state`. Called on every rejected transient step (LTE or Newton failure) and on every DC homotopy strategy fallthrough, before the retry. Default no-op. |
| `bound_step_hint()` | Return the maximum desirable next timestep (`$bound_step` lineage). Infinity means no bound. |
| `next_breakpoints(from, horizon)` | Absolute landing times this element requires the integrator to hit within `(from, from + horizon]` — `@timer` fires, source edges, PWL corners. Absolute times, so they survive step rollback. Default empty. |
| `suggest_transient_step(state, time_history, context)` | LTE-driven timestep suggestion, consulted by the transient stepper after each accepted step; the proposal is clamped to the minimum over all suggestions. Default none (no bound). |
| `accept_timestep(state, t, nets, sink)` | The analog→digital bridge hook (also on `Element`): called after each accepted solution point at time `t`; a mixed-signal element may emit digital events through `sink`. Advances accept-gated state (operators, event detectors, `last_volts`). |
| `destroy()` | Teardown when the circuit instance is dropped (also on `Element`). |

`context` carries only the immutable `Tolerances` (§3.3) — gmin, the
convergence tolerances, temperature, and the circuit-wide shunt. Simulation
time reaches an element through its analysis context (§3.3) or as an explicit
argument (`accept_timestep`), never through `Context`. `Context` carries **no**
mutable homotopy state — the source-stepping scale reaches an element through
the analysis state (below), and the gmin-stepping conductance is owned by the
DC driver (§15). Per-analysis convergence tunables (iteration cap, damping
threshold, trace toggles) live on the separate driver-owned `Policy`.

### 3.2 Analog loading methods

| Method | Analysis | Return |
|--------|----------|--------|
| `load_dc(state, context)` | DC operating point | Real MNA stamps for the nonlinear algebraic system. |
| `load_transient(state, transient_context, context)` | Time-domain analysis | Real MNA stamps for the implicit companion model at the current timestep. |
| `load_ac(dc_point, ac_context, context)` | Small-signal AC/noise | Complex MNA stamps linearized about the DC operating point. |
| `noise_current_psd(dc_point, ac_context)` | Noise | Current-noise sources as terminal pairs plus one-sided PSD values. |

The DC and transient `state` is **bidirectional**: it is the analog solution
history *and* the digital net snapshot being solved against. A mixed-signal
element whose analog stamps depend on digital logic (D2A) reads the exact digital
state here, with no device-side cache. Symmetrically, the digital evaluation
context (§4) carries the sampled analog voltages (A2D). Mixed-signal coupling is
thus native in both directions rather than routed through side state.

An element that does not participate in an analysis may return an empty stamp
list only when the physical model genuinely has no contribution in that analysis.
An unsupported construct must fail before this ABI is reached or must raise a loud
element-construction/load error.

### 3.3 Analog ABI types

All times, frequencies, values, and step sizes crossing this ABI are plain
`f64` — times are `f64` seconds; there is no typed-units layer.

| Type | Meaning |
|------|---------|
| `AnalogReference` | Reference to one analog variable. Ground has no MNA index; every other solved variable has one dense index. |
| `Stamp<Ref, Scalar>` | Either `Matrix(row, col, value)` or `Rhs(row, value)`. The scalar is real for DC/transient and complex for AC/noise. |
| `Noise` | A current-noise source between two analog references with PSD in A²/Hz. |
| `Context` | The shared, immutable run context: only the `Tolerances` (gmin, reltol, vntol, abstol, min_res, trtol, chgtol, temperature, tnom, gshunt). Immutable for a run; carries no time, no integration controls, and no per-solve homotopy or convergence state. |
| `Policy` | The driver-owned convergence tunables: the Newton iteration cap, the damping threshold, and the diagnostic trace toggles. Each analysis driver carries its own. |
| `DcAnalysisState` | The DC loading state: the analog solution history (row 0 latest), the digital net snapshot (D2A), and the source-stepping scale. Derefs to the history. |
| `TransientAnalysisState` | The transient loading state: the analog solution history and the digital net snapshot. Derefs to the history. |
| `TransientAnalysisContext` | Current time, the final time, the TR-BDF2 phase being stamped (Trapezoidal over `γh` or BDF2 over `(1−γ)h`), the full step `h`, and the previous accepted step size (so the TR stage can re-derive the previous capacitor current). No integration-method field — TR-BDF2 is the sole scheme. |
| `AcAnalysisContext` | Current frequency. |

### 3.4 Introspection: parameters, queries, terminals

Introspection is the third supertrait, `Introspect`. An element may expose
OSDI-style metadata so hosts — bench sweeps, optimization
loops, plugins, CLI/UI — discover and poke a model without knowing its family.
Every method here is optional; an element exposes as much or as little as it has.

**Parameters.**

| Method | Contract |
|--------|----------|
| `list_params()` | Declared parameters as descriptors: name, value kind, default, unit, bounds, model-vs-instance scope, and the invalidation a write forces. |
| `get_param(name)` | Current value, or none if there is no such parameter. |
| `set_param(name, value)` | Write a parameter. On success, return the invalidation the change forces; on failure, a typed error (unknown, read-only, out of range, type mismatch). |

The **invalidation** a parameter write reports is normative for sweep/optimization
correctness. It is one of: none (metadata only), restamp (numeric only),
temperature (recompute temperature-dependent constants), operating-point
(restart the DC solve), or rebuild (matrix structure / element reconstruction).
A caller recomputes exactly as much as the reported invalidation requires.

**Queries.**

| Method | Contract |
|--------|----------|
| `list_queries()` | Declared queries as descriptors: name, kind, unit, description. |
| `query(name)` | Read one query value, or none. |

A query kind is one of: operating variable, terminal voltage, terminal current,
internal state, event counter, or limiting/convergence state. The default
`list_queries`/`query` expose each `read_opvars` entry as an operating variable,
so any element with operating variables is queryable without extra code.

**Terminals.** `list_terminals()` returns terminal descriptors (name, domain,
direction, required) for diagnostics, current queries, and external-model
wrapping.

Values carried by parameters and queries are real, integer, boolean, or text.

---

## §4 Element ABI — digital operations

An element that declares `DIGITAL` participates in event-driven simulation. It
declares the nets it reads and drives, initializes its outputs, and evaluates in
two phases so register chains have non-blocking semantics. These are the methods
of the `DigitalDevice` supertrait of the one element contract (§3); there is no
separate digital device type.

### 4.1 Digital boundary

| Type | Meaning |
|------|---------|
| `DigitalNet` | Dense integer identifier for a digital net. |
| `LogicValue` | Four-state value: `0`, `1`, `X`, or `Z`. |
| `DigitalPorts` | Borrowed lists of input nets and output nets. Inputs are the sensitivity list; outputs are driven by the device. |
| `EvalCtx` | Read-only evaluation snapshot: time, all digital net values, and optional sampled analog values. |
| `EventSink` | Write-only event emitter. A device schedules changes through this facade, never by mutating the queue directly. |
| `DigitalEvent` | Value change on one digital net at one simulation time, with source and sequence provenance. |

`LogicValue` resolution for multiple four-state drivers is tri-state style:
`Z` yields to the other value, equal strong values preserve that value, and all
other conflicts produce `X`.

### 4.2 Digital methods

| Method | Contract |
|--------|----------|
| `boundary()` | Return stable input and output net lists. The lists must not change during an analysis. |
| `init(sink)` | Emit initial output events, normally at time zero. |
| `seq_phase(ctx)` | Phase 1: detect clock/event edges against internal prior state and commit register banks from the pre-settle snapshot. It returns whether a clocked block fired. It must not emit output events. |
| `comb_phase(ctx, sink)` | Phase 2: recompute driven outputs from current nets and internal state, emitting value-change events. |
| `evaluate(ctx, sink)` | Fused one-shot evaluation for models that do not participate in the scheduler's two-phase protocol. It is equivalent to `seq_phase` followed by `comb_phase`. |
| `has_input_on(changed)` | Convenience sensitivity test: true when any input net is in the changed set. |
| `digital_hidden_snapshot()` | Hidden digital state (module vars, edge-detection memory) as an opaque `(int, real)` carrier, snapshotted into each recorded transient step. `None` means stateless (pure combinational). |
| `digital_hidden_restore(state)` | Restore a state previously produced by `digital_hidden_snapshot`. Called on full-state re-entry (periodic-steady-state shots, transient restart from a captured step) after `init`, before the first settle — register state round-trips with the digital nets. |

An element whose logic samples analog voltages declares the `SAMPLES_ANALOG`
capability (§3) rather than a separate predicate method; the scheduler evaluates
such elements after an accepted analog solve even when no digital input changed.

The two-phase protocol is normative. All woken sequential phases observe the
same pre-settle net snapshot before any combinational output is recomputed.

### 4.3 Digital event ordering

Digital events are ordered by `(time, sequence)`. All events at the current time
or within the scheduler equality tolerance of the current time are drained into
the current delta cycle. Zero-delay events emitted during combinational
evaluation are applied in the same simulation time and may trigger another delta
iteration.

---

## §5 Element loading and external models

Element loading is outside the numerical algorithms but inside the solver ABI
contract. A loader constructs values that implement the `Element` trait, each
declaring its `ElementCapabilities`:

| Element kind | Declared capabilities |
|--------------|-----------------------|
| Pure analog | `ANALOG`. |
| Pure digital | `DIGITAL`. |
| Mixed signal | `ANALOG | DIGITAL` (plus `SAMPLES_ANALOG` if it reads analog voltages). |

The coarse flags are refined by the per-analysis flags: an analog element also
declares which analyses it contributes to (`LOADS_DC`/`LOADS_AC`/`LOADS_TRAN`/
`EMITS_NOISE`), and `DEPENDS_ON_DIGITAL` marks an analog load that reads the
digital net snapshot.

A loader receives already-resolved terminal bindings: analog terminals as analog
references and digital terminals as digital nets. Parameter values are already
elaborated. The loader must either construct a faithful element or fail loud with
a diagnostic naming the model and missing capability.

Native PHDL-compiled elements, native plugin elements, and wrapped external model
ABIs all lower into this same one `Element` boundary. An OSDI v0.4 model is not a
solver-native object; an OSDI loader must parse the model descriptor, bind its
terminals and parameters, and wrap the compiled model as an element declaring
`ANALOG`. The solver core does not require an OSDI loader to exist, and an
unavailable OSDI feature is a plugin/device load error rather than a silent
solver behavior.

An element that declares no capability is invalid for solve and must not be
admitted into a circuit instance.

### 5.1 Device specification

A device factory receives a resolved specification:

| Field | Meaning |
|-------|---------|
| owner | Device-library identity that owns the factory. |
| type | Device type identifier registered by the loader. |
| ports | Logical port names, directions, and resolved terminal bindings. |
| params | Elaborated instance parameter values after defaults and overrides. |
| attributes | Validated attributes attached to the module, instance, and ports. |

Each terminal binding is one of:

| Binding | Meaning |
|---------|---------|
| Analog reference | A conservative terminal or analog storage quantity that participates in the analog variable namespace. |
| Digital net | A storage digital terminal that participates in the event scheduler. |
| Unconnected optional terminal | Permitted only when the declared port/loader contract says the terminal is optional. |

The factory must declare whether the produced element is analog, digital, or
mixed-signal. The returned element must declare the corresponding
`ElementCapabilities` and implement the matching operations described in §3
and §4.

The language and elaboration layers own the surface syntax and rules that decide
which module or instance requests an external factory. The solver ABI begins only
after that decision has been resolved into the specification above.

### 5.2 Factory obligations

A factory must either return a faithful device or fail loud. It must not admit a
device with missing required terminals, unsupported parameter values, unknown
attributes that affect model semantics, or an unsupported analysis mode that will
later be silently ignored.

An analog factory may consume analog references and may use branch variables
allocated for that model during device construction. It may not allocate new MNA
unknowns after analysis begins. A digital factory may consume digital nets and
must provide a stable digital boundary for the lifetime of the analysis. A
mixed-signal factory must satisfy both contracts.

If an external ABI requires internal unknowns or auxiliary branches, the loader
must allocate those unknowns before the circuit instance is finalized, through
the one allocation seam: the builder calls each element's `allocate_unknowns`
with an `UnknownAllocator` before the matrix shape freezes, and an element that
allocates must declare `HAS_INTERNAL_UNKNOWNS` (the build fails loud otherwise).
If allocation is impossible, loading fails loud with a diagnostic naming the
model and the missing allocation capability.

### 5.3 Device-loading validation

| Rule | Failure |
|------|---------|
| Required terminal is unbound | Device-construction error. |
| Terminal domain does not match the factory's declared binding | Device-construction error. |
| Required parameter is absent or has an unsupported value | Device-construction error. |
| Factory returns an element that declares no capability | Device-construction error. |
| Factory needs internal analog variables but no allocation seam is available | Device-construction error. |

---

## §6 Analog variable and node labels

The analog namespace (§6) and the digital namespace (§7) are named uniformly at
the public boundary by one identity, the **net**. A net pairs the fast dense
solve index with a kind — analog node, analog branch current, digital net, or a
pseudo signal with no unknown (ground) — and a stable label. The domain-specific
fast-path types (`AnalogReference` over an `AnalogVariable`, and `DigitalNet`)
remain for the hot loops and both convert into a net, so diagnostics, queries,
and result mapping treat `v(out)`, `i(vsrc)`, a digital net, and `GND`
symmetrically. Enumerating every solved signal of a circuit as nets is a single
operation over both domains.

The analog namespace contains node variables, branch-current variables, and
analysis pseudo-variables.

| Variable kind | Label form | MNA index |
|---------------|------------|-----------|
| Ground node | `GND` | None. Ground is the reference potential and is not an unknown. |
| Non-ground node | Anonymous labels display as `n<N>` | Dense zero-based index. |
| Branch variable | Component label plus optional branch name | Dense zero-based index. |
| Time | `time` pseudo-variable | No ordinary MNA index. |
| Frequency | `frequency` pseudo-variable | No ordinary MNA index. |
| Iteration | `iteration` pseudo-variable | No ordinary MNA index. |

Ground spellings in the language elaborate to the single ground reference. A
device may use a missing analog reference to mean ground only where the ABI
explicitly permits it, such as initial-condition tuples.

Branch variables represent currents introduced by ideal voltage constraints,
force branches, inductive companion models, and any other MNA equation that
requires an extra unknown. A branch label has a component identity and may have a
device-local branch name. The component identity is stable within one circuit
instance and should be human-readable in diagnostics.

Analog indices are dense over all non-ground node and branch variables. The
matrix dimension is one plus the maximum assigned index, or zero for an empty
analog system. Ground is never allocated a row or column; stamps targeting
ground are ignored by index-based matrix application because ground contributes a
known zero potential.

---

## §7 Digital net labels and logic values

The digital namespace is a dense array of digital nets. A `DigitalNet` label is
an integer index into the digital state vector. All nets initialize to `X` before
device initialization events are applied.

Digital net labels are local to a circuit instance. Source-level names are
resolved before solve; the solver requires only the dense index.

The four logic values are:

| Value | Meaning |
|-------|---------|
| `0` | Strong logical false. |
| `1` | Strong logical true. |
| `X` | Unknown or contention. |
| `Z` | High impedance. |

Result objects that expose digital traces read values by digital-net index.
Mapping those indices back to source names is a reflection/result-layer
responsibility, not part of the solver's numerical contract; a digital net
converts into the unified net identity of §6 with an anonymous label until the
circuit builder attaches the hierarchical source name it owns.

---

## §8 Stamping and MNA

The analog solver forms systems from stamps:

```text
A · x = b
```

`x` is the vector of non-ground node voltages and branch currents. A matrix stamp
adds to `A[row, col]`; an RHS stamp adds to `b[row]`. Multiple stamps to the same
entry accumulate.

For nonlinear analyses, devices stamp the local linearization of their residuals
at the current iterate. For a node row, the residual is KCL current imbalance. For
a branch row, the residual is the branch equation imbalance. Reactive devices in
transient analysis use implicit companion models and stamp the conductance-like
Jacobian terms plus history-dependent RHS terms.

Potential forces are represented as branch equations with an associated branch
current unknown. Flow contributions stamp directly into node KCL rows. A device
may introduce branch variables only during circuit construction; analysis-time
loading may not change the variable set.

The linear backend may cache the symbolic sparsity pattern. Numeric values may
change every iteration or frequency point; the set of possible matrix positions
is fixed after the circuit instance is built.

---

## §9 DC operating point

DC analysis solves the nonlinear algebraic operating point at time zero.

The DC algorithm is:

1. Allocate the analog system from the fixed analog variable map.
2. Seed the Newton state from explicit node-set or initial-condition hints when
   supplied; otherwise start from zero or the previous accepted state.
3. For each Newton iteration:
   - Ask elements to update from the current state.
   - Collect DC stamps from every element's `load_dc`.
   - Add any active homotopy conductances (§15.5).
   - Solve the linearized system.
   - Apply solver-side damping/limiting (§15.2).
   - Accept convergence only if both the update test and residual test pass and
     no device reports active limiting.
4. If plain Newton fails, attempt gmin stepping. If that fails, attempt source
   stepping.
5. Run the mixed-signal DC settle loop (§14.1) until digital state stops changing
   or the mixed-signal iteration cap is reached.
6. Return a mapping from every indexed analog variable to its solved value.

Two assembly-level details are normative:

- **Stamp cache.** Between Newton iterations the DC driver reuses the previous
  iteration's assembled stamps when every unknown moved less than
  `vntol + reltol·max(|x|, |x_old|)` (ngspice bypass semantics), suppressed
  while any device reports a non-`None` `limiting_report()`. The cache is dropped whenever
  the stamps depend on anything besides the solution vector — a homotopy scale
  change or a digital settle — and on any parameter write.
- **Shunt conductances.** The homotopy conductance (§15.5) and the optional
  circuit-wide `gshunt` tolerance stamp a diagonal conductance to ground on
  every non-ground node — never on branch-current unknowns.

DC ignores dynamic charge history except where a device's DC model explicitly
depends on its internally updated operating point. Time-varying sources are
evaluated at the DC context defined by the source model.

---

## §10 Transient analysis

Transient analysis integrates from a start time (default `t = 0`) to
`stop_time` over a fixed circuit topology. A non-zero start time is the host
restart form (§10.5): the integrator's clock is absolute — `$abstime`,
breakpoints, and scheduled sets all read it — and the initial state is the
start-time operating point overlaid with the host's carried initial
conditions.

### 10.1 Initial state

The transient initial state is built from a DC operating point. Device
initial-condition requests and user initial-condition seeds overlay that DC
point. For a branch voltage initial condition `(plus, minus, value)`, the
initial value is:

```text
V(plus) = V(minus) + value
```

where a missing `minus` terminal means ground.

Device initial conditions are **enforced**, not merely seeded (the ngspice
`CKTsetIC` analogue): each `@initial` branch seed becomes a UIC hold clamp — a
large conductance (`G = 10¹²`) across the seeded branch carrying `G·ic` — stamped
through the t=0 operating-point solve and the first accepted step, so the seed
value is the *consistent* t=0 solution the rest of the circuit solves against.
The clamp releases after the first accepted step.

User node-voltage seeds and the solved history populate enough solution history
for the companion model to start without an artificial first-step
discontinuity: the initial values are pushed into both history rows. A host
restart instead re-enters from a previously captured step — the analog solution
and the digital snapshot (including hidden register state, §4.2) — with no DC
solve and no device/user seeds.

### 10.2 Step algorithm

The time loop is a thin loop over named phase methods, with its mutable state
(the clock, the current step size, acceptance bookkeeping) carried in one
`TimeLoop` state:

1. **Predict** (`predict_step`). Take the PI controller's proposed timestep
   (§10.3) and clamp the target time to the analysis stop time, to the next
   pending digital event time, to the next declared **breakpoint** (analog
   `@timer` fires and source edges — see §15.9), and to the next scheduled
   live-set time (§10.5). Digital-var/enum `if`s in analog bodies switch at
   digital events, which are themselves breakpoints, so landing here covers
   them. A target landing on a declared discontinuity is marked exempt from
   the LTE gate.
2. **Attempt** (`attempt_step`). Checkpoint the digital state and the analog
   solution history, apply digital events exactly at the target time before
   the analog solve, then solve the analog implicit companion system for the
   interval ending at the target time — TR-BDF2 runs two Newton sub-steps
   (Trapezoidal → `x_{n+γ}`, BDF2 → `x_{n+1}`; γ = 2−√2).
3. **Assess** (`assess_step`). If both sub-steps converged, estimate the
   global LTE (Milne's device over node-voltage unknowns) and compare against
   `trtol`. Steps exempted in predict (and the first step after a live set,
   whose LTE window spans the intentional value jump) skip this gate.
4. **Accept** (`accept_step`). On a pass:
   - Service analog-to-digital acceptance hooks and run digital evaluation to
     quiescence at the target time (`settle_digital`), then commit the digital
     checkpoint.
   - Record the step if it is at or after `record_from` (`record_step`).
   - Apply any scheduled live sets due at the accepted point (§10.5); a write
     of operating-point strength or stronger re-solves the landing point so
     the recorded state there is the post-set consistent solution.
   - Advance the clock and integration history; reset the previous-step size
     across a discontinuity so the next TR stage restarts first-order.
   - Propose the next timestep (`propose_dt`) — the PI controller from the
     global error, then clamped by every element's `suggest_transient_step`.
5. **Reject.** On an LTE failure (`reject_lte_step`): roll back the digital
   checkpoint and the analog history, call `restore_state` on every
   checkpointed element (limiter/digital registers rewound to the pre-attempt
   snapshot), roll back the unified `EventQueue` (honoring each entry's
   rollback behavior — digital events restored, breakpoints re-polled, hints
   discarded), reduce the proposed timestep via the convergence plan's stepper
   (÷8 backtracking), and reset the PI memory. At the minimum-timestep floor
   the step is **accepted as-is** rather than stalling — the accuracy
   concession is warned and counted in the run statistics. On a Newton failure
   in either sub-step (`reject_step`): roll back the digital + analog +
   device-internal checkpoints, roll back the event queue, reduce the
   timestep, and retry; reaching the minimum timestep without convergence
   fails the analysis loud.

The solver is **always adaptive** (SPICE has been adaptive since v2); the
user's `.step` is the initial timestep, grown/shrunk from there. The recorded
waveform is the adaptive time grid; waveform statistics weight by the timestep
so they stay correct on the uneven grid. Output interpolation onto a fixed
print grid is a roadmap follow-up; point queries (`Waveform::at(t)`) already
interpolate.

### 10.3 Integration method

The transient companion uses **TR-BDF2** (Trapezoidal Rule / Backward
Differentiation Formula 2) as the sole integration scheme. Each step advances
`[t_n, t_{n+1}]` in two stages with γ = 2−√2: a Trapezoidal stage over `γh`
produces the intermediate point `x_{n+γ}`, then a BDF2 stage over `(1−γ)h`
produces `x_{n+1}` from `x_{n+γ}` and `x_n`. The BDF2 stage is a native
low-pass filter, giving L-stability (no trapezoidal ringing on stiff/switched
nodes). There is no method-selection surface.

The Trapezoidal stage's companion carries the previous capacitor current
`i_{C,n}` (the trapezoidal companion is `i_{C,n+γ} = (2/(γh))(Q_{n+γ}−Q_n) −
i_{C,n}`), which the kernel re-derives from the prior step's BDF2 formula
(coeffs at the previous step size, charges at the three history points). The
BDF2 stage uses the pure-derivative companion.

**Restart convention.** Across a declared discontinuity — a breakpoint edge, a
scheduled live set, or a host restart — the previous-derivative term is
unavailable (the history spans the jump) and the Trapezoidal stage degrades to
backward Euler over the `γh` sub-step: `i_{C,n+γ} = (1/(γh))(Q_{n+γ}−Q_n)`,
no previous-current term. Keeping the full trapezoid weight with an assumed
zero previous current would double the derivative estimate for the first step,
an O(h)·i error scaling with the post-edge current. The same applies to the
inductor flux companion's previous branch voltage. After a scheduled live set
the run additionally restarts small (`1e-3` of the accepted step) and the PI
controller regrows from clean error readings; a plain breakpoint edge resets
only the previous-step history, leaving the PI proposal intact.

The timestep controller is a **Proportional-Integral (PI) controller**: after
each accepted step the global local-truncation error is estimated via Milne's
device (a linear extrapolation of the node voltages at `t_n` and `t_{n+γ}`
differenced from `x_{n+1}`, normalized per node by `reltol·|v| + vntol`), and
the next timestep follows `dt_{n+1} = dt_n · (target/lte)^p` with `p = kp +
ki·(lte − lte_prev)/lte` (defaults `kp = 0.7`, `ki = 0.4`). A node whose
consecutive differences straddle a discontinuity (one side flat, the other
large) is skipped — its predictor residual is the intentional jump, not
truncation error. The growth factor is clamped to a safe per-step range
(`[0.2, 1.5]`) and the result to `[dt_min, dt_max]`. A rejected step divides
the failed step by 8 and resets the PI memory. With no usable error signal
(non-reactive step or short history) the controller grows `dt` by 1.5× toward
`dt_max`. All of these gains live in the typed `StepperGains` config (§15.4),
not in the controller body. The Milne estimate is computed over node-voltage
unknowns only (branch currents are KCL-derived).

The PI controller is owned by the convergence plan (`ConvergencePlan::stepper`,
§15.4) — the transient driver delegates `propose_dt` and `reject_dt` to the
plan's stepper rather than holding its own stepper. The plan is the single
strategy owner across analyses (Newton + Homotopy + Stepper). The unified
event queue (§15.9) is the predict-step read path; `predict_step` peeks
`EventQueue::peek_next_time()` instead of merging the four ad-hoc sources.

### 10.4 Results

Each recorded transient point contains:

| Field | Meaning |
|-------|---------|
| time | Accepted simulation time. |
| analog values | Solved value of each indexed analog variable. |
| digital snapshot | Logic value of every digital net after digital evaluation at that time. |

`record_from` affects recording only. The solver still integrates from the
start time because skipped early states influence later history.

### 10.5 Live parameter sets and the host surface

A host may write parameters on the **compiled** circuit — no re-elaboration,
no re-JIT (the MD-18 boundary): elaboration fixes devices; simulation
restamps. Addressing is the PHDL scheme — the same flat instance labels and
flattened `{param}_{field}` bundle names the POM's `Design::set_param`
accepts. A write routes to the element's `set_param` (§3.4) and the caller
recomputes exactly what the reported invalidation requires; a successful
write through a held DC analysis also invalidates the driver's bypass stamp
cache. Unknown labels or parameters fail loud (the parameter
error lists the element's candidates); an out-of-bounds value is rejected
with no partial apply.

**Scheduled sets.** A write may be scheduled for a simulation time `t` on a
running transient. Each scheduled time is a declared discontinuity: it feeds
the unified breakpoint table, so the integrator lands exactly on `t`, applies
the write there (scheduling order — last write wins per parameter), and the
new value takes effect from the next accepted step under the §10.3 restart
convention (LTE skipped at the edge, previous-derivative history discarded,
small resume step). A write of operating-point strength or stronger re-solves
the landing point so the recorded state at `t` is the post-set consistent
solution. Sets scheduled at or before the start time apply before the initial
operating point — an idle set.

**Structural writes.** A write whose invalidation is *rebuild* (matrix
structure / element reconstruction — e.g. an optional-parameter presence
flip) is beyond the solver: it has no POM. The restamp path reports the
rebuild invalidation to the caller, and a structural set scheduled
mid-transient fails the run loud with the typed outcome. The **host layer**
(the Python `LiveSession`: compile once, `set`, re-run analyses on the held
circuit) re-elaborates and
recompiles automatically, reports it visibly, and carries the solved node
voltages by net name as the next solve's initial guess — dropped nets are
discarded, new nets start cold. At the host layer a structural set scheduled
mid-transient splits the run at `t`: the session rebuilds there and the
transient restarts
from `t` (absolute start time, carried node state as initial conditions), and
the recorded segments stitch into one continuous trace. A failed
re-elaboration surfaces the error and keeps the previous compiled circuit
usable.

---

## §11 AC analysis

AC analysis computes the small-signal frequency response around a DC operating
point.

The AC algorithm is:

1. Solve the DC operating point.
2. For each frequency in the requested sweep:
   - Build the AC context for that frequency.
   - Ask each analog device for complex small-signal stamps linearized at the DC
     point.
   - Solve the complex linear system.
   - Record complex values for every indexed analog variable.

Frequency sweeps may be linear or logarithmic. A sweep with one or fewer points
contains the start frequency only. AC analysis is linear at each frequency; it
does not run the mixed-signal event scheduler during the sweep except through
state already captured in the DC operating point and device small-signal model.

Reactive contributions are represented by frequency-domain admittances such as
`jω · dQ/dV`. Independent AC stimuli are represented as complex RHS terms with
their configured magnitude and phase.

---

## §12 Noise analysis

Noise analysis computes output noise over an AC frequency sweep using the
small-signal operating point.

The noise algorithm is:

1. Solve the DC operating point.
2. Resolve the output node and reference node.
3. Build the linearized small-signal matrix pattern.
4. For each frequency:
   - Assemble complex AC stamps at that frequency.
   - Solve the adjoint system by transposing the linearized matrix and applying
     a unit current excitation at the output/reference pair.
   - Ask each analog device for current-noise PSD sources at the DC point and
     current frequency.
   - For each source, multiply the source PSD by the squared transfer magnitude
     from the adjoint solution and accumulate output PSD.
5. Integrate the output PSD over frequency with trapezoidal integration and
   return the RMS output noise.

Noise source values are one-sided power spectral densities in A²/Hz. The output
PSD is reported in V²/Hz for voltage outputs.

---

## §13 Transfer-function analysis

Transfer-function analysis computes DC small-signal quantities around the
operating point.

The algorithm is:

1. Solve the DC operating point.
2. Resolve the input source branch and output variable.
3. Assemble the DC linearized Jacobian at the operating point.
4. Apply a unit input excitation and solve for gain.
5. Derive input resistance from the same input-excitation solution.
6. Apply a unit output test excitation and solve for output resistance.

The transfer type is determined by whether the input is a voltage or current
source and whether the output variable is a voltage or current:

| Input | Output | Transfer type |
|-------|--------|---------------|
| Voltage | Voltage | Voltage gain. |
| Voltage | Current | Transconductance. |
| Current | Voltage | Transresistance. |
| Current | Current | Current gain. |

Unsupported input-source forms must fail loud. Returning an arbitrary infinite
or zero resistance for an unsupported case is not permitted unless it is the
physically correct result of the solved linear system.

---

## §14 Mixed-signal execution

Mixed-signal behavior is expressed by an element that declares both `ANALOG` and
`DIGITAL` and implements both sets of operations, or by paired elements that
communicate through explicit analog and digital nets. There is no implicit
converter insertion.

The analog↔digital crossing has exactly one owner: the circuit instance's
mixed-signal-seam methods (§2). `accept_and_run_digital` turns an accepted
analog solution into digital events (every element's `accept_timestep` hook
seeds the event queue) and runs the scheduler to quiescence; the DC settle loop
(§14.1) and the transient accept path (§14.2) both go through it. There is no
separate bridge object — any element is natively mixed-signal.

### 14.1 DC mixed-signal settle loop

After an analog DC solve converges, the solver lets analog acceptance hooks emit
digital events and runs digital evaluation at time zero. If any digital net
changes, D2A state may have changed the analog stamps, so the analog DC solve is
repeated. This alternation continues until digital state is unchanged or the
mixed-signal iteration cap is reached.

The loop order is:

```text
analog Newton solve → analog accept hooks → digital settle → repeat if digital changed
```

### 14.2 Transient mixed-signal ordering

At a transient target time, digital events scheduled for that time are applied
before the analog solve. This lets D2A bridges update their analog stamp state
for the interval endpoint. After analog convergence, A2D bridges inspect the
accepted analog solution and may emit digital events; the digital scheduler then
settles at the same time.

If the analog solve for the target time fails, digital state is rolled back to
the checkpoint taken before applying that time's events, the timestep is reduced,
and the step is retried.

### 14.3 Digital delta-cycle algorithm

At a digital evaluation time:

1. Drain all events due at that time into the changed-net set.
2. Run `seq_phase` for every woken device in topological order. All sequential
   phases observe the same pre-combinational snapshot.
3. Run `comb_phase` for woken devices in topological order.
4. Apply zero-delay emitted events immediately. Future events remain queued.
5. If a back edge changes or new same-time events exist, restart from the
   earliest affected topological position.
6. Stop when no same-time event or back-edge restart remains.

When no topology is available, the scheduler uses a fixed-point delta-cycle loop
over all woken devices. Both modes have a finite iteration cap. Exceeding the cap
is a convergence failure of the digital network. A production analysis that
depends on the value must fail loud rather than silently accept an oscillating
combinational loop.

---

## §15 Convergence aids

### 15.1 Update and residual convergence

Newton convergence requires both:

1. **Update convergence.** For every indexed variable, the absolute update must
   satisfy:

   ```text
   |x_new - x_old| <= reltol · max(|x_new|, |x_old|) + abstol_kind
   ```

   Node-voltage rows use voltage tolerance; branch-current rows use current
   tolerance.

2. **Residual convergence.** For every row, the assembled residual magnitude
   must satisfy:

   ```text
   |A · x_old - b| <= abstol_kind + reltol · row_scale
   ```

   The tolerance kind follows the unknown the row belongs to: node-voltage
   rows use voltage tolerance; branch-current rows use current tolerance.

Device-side limiting is an additional gate: if any analog device reports a
non-`None` `limiting_report()`, Newton convergence is false even when the numeric
tests pass. Before the tests run, the solver applies every device's structured
limiting report (`limiting_report()`, §3.1) to the Newton guess — the clamped
unknown is set to `limited_value`, so the iteration continues from the clamped
point instead of oscillating around it.

### 15.2 Damping

If a Newton update is larger than the configured damping threshold in vector
norm, the solver replaces the candidate solution by the midpoint between the
previous state and the candidate state. This reduces oscillation in stiff
nonlinear systems. Damping is applied before convergence tests.

### 15.3 Device limiting

Devices may internally limit state changes, such as PN junction voltage changes
or MOS operating-region transitions. A limited device must report active
limiting until the limited quantities are consistent with the converged solution.
The solver must not accept convergence while any device reports active limiting.

### 15.4 Convergence plan

Homotopy escalation and transient stepping are **solver policy**, expressed as a
composable convergence plan rather than inline branches in the DC or transient
driver. The plan owns three strategies:

- **NewtonStrategy** (e.g. `DampedNewton`) — the inner loop's damping/limiting
  policy.
- **HomotopyStrategy** (`GminStepping`, `SourceStepping`) — the DC fallback
  cascade; runs plain Newton, then falls through an ordered list of homotopy
  strategies until one converges, returning the first converged solution or the
  last failure. The default plan is gmin stepping followed by source stepping.
  Each strategy is stateless: it drives the plain-Newton solve and sets the
  homotopy scales through a driver interface, and never reaches into the
  solver's internals.
- **StepperStrategy** (`PiController`) — the transient timestep proposal/reject
  policy. The transient driver delegates `propose_dt` and `reject_dt` to
  `plan.stepper()`; the PI controller's behavior is unchanged (bit-identical
  step sequence on the parity baselines). A custom stepper is plugged in by
  constructing the plan with `with_stepper` — the driver routes accept/reject
  callbacks through the plan without reimplementing the transient loop.

This is the seam at which an analysis or host selects a different escalation or
a different timestep policy. The plan is the single strategy owner across
analyses (Newton + Homotopy + Stepper).

Every behavior-affecting numeric lives in the solver's **config home** as a
typed, defaulted, documented field — no magic literals in strategy or driver
bodies. The plan owns the two homotopy schedules (`GminSchedule`,
`SourceSchedule`, one `Schedules` family); the transient stepper owns its
`StepperGains` (§10.3); the diagnostic trace toggles are `TraceFlags` fields
seeded once from the `PIPERINE_TRACE_{GMIN,SRC,TRAN}` environment variables and
read as typed fields thereafter. Cross-driver numerical caps (the mixed-signal
settle cap, the digital delta-cycle cap, the scheduler time-equality epsilon)
live beside them in `PlanLimits`.

### 15.5 Gmin and gmin stepping

The solver context contains a normal `gmin`, used by device models for weak
conductance stabilization. Gmin stepping adds an extra homotopy conductance,
owned by the DC driver (not the shared context).

During gmin stepping, every non-ground node receives an added conductance to
ground. The strategy starts from an easy, strongly shunted problem
(`start_g = 0.1` S) and reduces the extra conductance toward zero,
warm-starting each step from the previous solution: one decade per converged
solve (`decade_factor = 0.1`), the step factor relaxing after each success
(`relax_growth = 1.3`, capped at `relax_cap = 0.5`) and backing off after each
failure (`backoff_growth = 3.0`, capped at `backoff_cap = 0.7`), for at most
`max_steps = 200` iterations, stopping once the conductance is below
`floor_margin = 10` × the gmin floor. A failure before any step converged gives
up immediately. The final accepted operating point is always solved with the
extra conductance at zero. The extra conductance is applied only to
node-voltage unknowns, never to branch current unknowns.

### 15.6 Source stepping

Source stepping scales independent forced source values from zero to full
strength. It runs after plain Newton and gmin stepping fail. Each scale point
warm-starts from the previous point: the ramp starts fully off
(`start_step = 0.1`), grows the step after each converged solve
(`step_growth = 1.5`, capped at `step_cap = 0.25`), halves the step after a
failure (`backoff_factor = 0.5`) and gives up when the step falls below
`min_step = 1e-6`, for at most `max_steps = 300` iterations. A temporary shunt
(`knee_gmin = 1e-6` S) conditions the exponential turn-on knee; it is held
through the source ramp and then itself ramped out (one decade per converged
solve, `knee_decay = 0.1`, until below `floor_margin = 10` × the gmin floor),
so the final solve is exact.

An element whose source value is affected by source stepping multiplies that
source by the source-stepping scale carried in `DcAnalysisState`. Elements that
do not represent independent sources ignore it.

### 15.7 Initial guesses, node sets, and device initial conditions

Node-set values and user initial conditions seed Newton history; they are not
themselves constraints. Device initial conditions in transient are enforced by
the UIC hold clamps of §10.1 through the t=0 solve and the first accepted step.

The solver may push the same initial condition into multiple history rows when a
multistep integration method needs a consistent starting history.

### 15.8 Timestep rejection and rollback

Transient convergence failure rejects the candidate step. The reject path is
symmetric with the attempt's checkpoints — every snapshot taken before the
candidate endpoint is restored:

1. **Digital state** — `DigitalState::rollback()` rewinds the net snapshot and
   the digital scheduler's pending events to the pre-attempt checkpoint.
2. **Device-internal state** — `Element::restore_state(&checkpoint)` is called
   on every element that produced a non-`None` `checkpoint_state()` snapshot:
   the limiter (`active`, `seeds`, vold slots) and digital register banks
   (`vars_int`, `vars_real`, `prev_watch`) rewind to the pre-attempt values.
3. **Analog history** — the Newton solver's solution history (the rows the
   rejected attempt left behind) is restored to the pre-attempt buffer.
4. **Unified event queue** — `EventQueue::rollback()` honors each drained
   entry's `RollbackBehavior`: `Restore` for digital events (re-fired on the
   retry), `RePoll` for breakpoints (dropped — re-declared by `next_breakpoints`
   next predict), `Discard` for step hints and crossings (re-emitted by the
   device).

The timestep is then reduced through the convergence plan's stepper
(`ConvergencePlan::stepper_mut().reject_dt`). A step is committed only after
the analog solve succeeds and same-time digital acceptance has run; the
checkpoints are discarded on acceptance (no `restore_state` call —
`accept_timestep` advances accept-gated state).

The same checkpoint/restore pair guards DC homotopy retries (§9): before each
homotopy strategy attempt the DC solver calls `checkpoint_state()` on every
checkpointed element; on strategy fallthrough (failed attempt → next strategy)
it calls `restore_state()` before the next strategy starts, so a failed
gmin-stepping attempt does not leave a dirty limiter for the source-stepping
attempt.

### 15.9 Timestep bounds and breakpoints

The step size is bounded from three directions. Elements declare
**breakpoints** — absolute landing times — through
`Element::next_breakpoints(from, horizon)`; reactive elements report the
largest step their charge/flux history tolerates through
`suggest_transient_step`, consulted after every accepted step, with the
proposal clamped to the minimum over all suggestions; and the PI proposal
itself is clamped to the analysis options' `[dt_min, dt_max]`.

The four time-discontinuity sources (digital events, analog breakpoints,
scheduled live sets, `$bound_step` hints) flow through one **unified event
queue** (`EventQueue<EventEntry>`). Each entry carries its kind
(`Digital`/`Breakpoint`/`StepHint`/`Crossing`), its target, its time, its
priority (`Exact`/`Advisory`), its source, and its `RollbackBehavior`
(`Restore`/`RePoll`/`Discard`). `predict_step` reads the earliest time from
`EventQueue::peek_next_time()` rather than merging the four sources by hand;
the queue is checkpointed in `attempt_step` and rolled back on rejection so
each entry's rollback behavior is honored (§15.8). The target time is the
minimum of the PI-proposed timestep, the queue's next event time, and the stop
time. Breakpoints are absolute, so they survive step rollback.

Breakpoints come from two unified sources: (a) **analog** — each element's
`@timer` fires (a phased `@timer(period, phase)` lets a source declare both
its rise and fall edges, so the integrator lands on each switching edge
instead of stepping over it); (b) **digital** — the digital event queue's
future value-change times, which are when digital-var/enum `if`s in analog
bodies switch. Landing on a digital event thus covers analog contributions
that branch on a digital variable. If no hook is available, the solver still
must honor digital event times and the global minimum/maximum timestep
limits.

### 15.10 Linear-solver safety

If the linear solve returns a non-finite value, the nonlinear solve fails loud.
The solver must not continue from NaN or infinity.

---

## §16 Validation and failure rules

Every failure in this Part is an analysis or device-load error. These errors are
not parse or elaboration errors unless the invalid condition is detectable before
device construction.

| Section | Rule | Failure |
|---------|------|---------|
| §2 | Circuit contains an element that declares no capability | Device-load error. |
| §3 | Unsupported analog behavior reaches the ABI | Device-load or analysis error; never an empty fake stamp. |
| §4 | Digital boundary changes during an analysis | Analysis error. |
| §4 | Digital event targets a nonexistent net | Analysis error. |
| §5 | External model or plugin cannot bind required terminals/params | Device-load error. |
| §6 | Stamp references an unmapped non-ground/non-branch variable | Analysis error. |
| §8 | Analysis-time loading changes matrix dimension or sparsity contract | Analysis error. |
| §9 | DC fails plain Newton, gmin stepping, and source stepping | Convergence failure. |
| §10 | Transient Newton solve reaches the minimum timestep without converging | Convergence failure. (An LTE rejection at the minimum timestep is instead accepted with a warning and counted in the run statistics.) |
| §11 | AC frequency point cannot solve its linear system | Analysis error for that sweep. |
| §12 | Noise output/reference node cannot be resolved | Analysis error. |
| §13 | Unsupported transfer-function source form is requested | Analysis error. |
| §14 | Digital delta cycle does not settle within the iteration cap | Digital convergence failure. |
| §15 | Linear solve returns NaN or infinity | Convergence failure. |
| §17 | Sensitivity parameter is unknown, unreadable, non-real, or rebuild-strength | Analysis error. |
| §18 | Non-positive period or negative pre-roll requested; digital state not periodic after convergence | Analysis error. |

---

## §17 Sensitivity analysis

DC sensitivity analysis (`.sens`) computes `∂(output)/∂(param)` at the
operating point over the restamp path (§10.5).

The algorithm is:

1. Validate the whole request up front (no partial writes on a bad request):
   each `(element label, parameter)` pair must name an existing element with a
   declared, real, readable parameter whose write does not invalidate the
   compiled structure (a *rebuild*-strength
   parameter fails loud — a finite difference across a rebuild boundary is not
   a sensitivity), and each requested output must be a solved analog net.
2. For each pair, perturb the parameter by a central difference step
   (`±dp`, relative with default `dp_rel = 1e-6`, absolute fallback when the
   parameter value is zero), re-solving the DC operating point at each side on
   the same compiled circuit.
3. Difference the requested outputs (node voltages and branch currents,
   addressed as nets) across the two operating points.

The result is keyed by `(output label, "element.param")`. Sensitivities are
defined only for restampable parameters; every addressing failure is loud.

---

## §18 Periodic steady state

Periodic-steady-state analysis (PSS) finds a periodic orbit of a driven circuit
by **single shooting**: Newton iteration on `g(x₀) = x(t₀+T) − x₀`, where each
evaluation of `g` is an ordinary transient over one period re-entered from `x₀`
(§10.1 full-state re-entry). Mixed-signal circuits run unchanged inside every
shot — scheduler, breakpoints, and bridges all active; Newton sees only the
continuous unknowns.

The algorithm is:

1. Validate the request: the period `T` must be positive and the optional
   pre-roll `tstab` non-negative (autonomous period detection is out of scope;
   the drive period is supplied).
2. Optionally integrate `[0, tstab]` once to move the starting state near the
   orbit before shooting begins.
3. Shooting Newton: from the current `x₀`, run one period and form the
   periodicity residual. The first Jacobian is built by finite difference (one
   extra shot per unknown); later iterations reuse it with Broyden rank-1
   updates. The Newton update is damped — each component's move is clamped to a
   physically plausible magnitude so an exponential nonlinearity cannot throw
   `x₀` to hundreds of volts — and a singular Jacobian fails loud. Converge
   when `max_i |x_i(T) − x_i(0)|` is within the shooting
   tolerance (default `1e-6`, bounded below by the adaptive integrator's
   per-period reproducibility of ~`1e-7`); iteration count is capped
   (default 40) and exhausting it fails loud.
4. Verify the orbit *repeats*: one extra shot over the second period must land
   where the first did (within integration tolerance) — a fixed point of the
   period map under a non-periodic drive is not a steady state. Then verify
   digital periodicity: a mismatch fails loud, and
   when the digital state closes only after `k` periods (checked up to `k = 4`)
   the error names the circuit period as `k·T` (the divider case).

The result is one period of transient samples (`t ∈ [t₀, t₀+T]`) plus the
shooting diagnostics: iteration count, the final periodicity residual, and —
when a Jacobian was computed and the orbit is stable — the estimated natural
settling time from the dominant monodromy eigenvalue (power iteration on the
shooting Jacobian).

---

## §19 Lifecycle contract — per-analysis hook chart

This section is the **normative lifecycle contract**: for each analysis
(`.dc`, `.ac`, `.tran`, `.noise`, `.pss`, `.sens`), the ordered sequence of
`Element`/`AnalogDevice`/`DigitalDevice`/`Introspect` hooks with their
preconditions and postconditions, plus a structured algorithm description
covering the main loop, the phases within one iteration, the
convergence/rejection criteria, and where each hook sits in that flow. An
external device author reads this chart to know exactly **when** and **why**
every hook fires — without reading the solver source.

An executable contract test (`piperine-solver/tests/lifecycle.rs`)
instruments a recording `Element` and asserts the hook ordering documented
here for each analysis.

### 19.1 Hook ordering legend

The chart below uses these abbreviations for the hook methods documented in
§3.1 and §4.2:

| Abbreviation | Hook | Supertrait |
|--------------|------|------------|
| `setup` | `setup(context)` | `Element` |
| `alloc` | `allocate_unknowns(alloc)` | `Element` (builder phase) |
| `temp` | `set_temperature(t)` | `AnalogDevice` |
| `update` | `update(state, context)` | `AnalogDevice` |
| `load_dc` | `load_dc(state, context)` | `AnalogDevice` |
| `load_ac` | `load_ac(dc_point, ac_ctx, context)` | `AnalogDevice` |
| `load_tran` | `load_transient(state, tran_ctx, context)` | `AnalogDevice` |
| `limit` | `limiting_report()` | `AnalogDevice` |
| `ckpt` | `checkpoint_state()` | `Element` |
| `restore` | `restore_state(ckpt)` | `Element` |
| `accept` | `accept_timestep(state, t, nets, sink)` | `Element` |
| `seq` | `seq_phase(ctx)` | `DigitalDevice` |
| `comb` | `comb_phase(ctx, sink)` | `DigitalDevice` |
| `init` | `init(sink)` | `DigitalDevice` |
| `noise_psd` | `noise_current_psd(dc_point, ac_ctx)` | `AnalogDevice` |
| `bp` | `next_breakpoints(from, horizon)` | `AnalogDevice` |
| `suggest_dt` | `suggest_transient_step(...)` | `AnalogDevice` |
| `opvars` | `read_opvars()` | `Introspect` |
| `destroy` | `destroy()` | `Element` |

Hooks with a default no-op are listed in brackets `[ ]`; they fire but a
device that does not override them does nothing. Hooks that never fire in a
given analysis are omitted from that analysis's chart.

### 19.2 DC operating point (`.dc`)

**Algorithm flow.** DC solves the nonlinear algebraic operating point at time
zero. The driver runs a homotopy cascade through the convergence plan
(§15.4): plain Newton first, then gmin stepping, then source stepping. Each
homotopy strategy attempt is one Newton loop. Between strategy attempts —
when one strategy falls through to the next — the checkpoint/restore pair
fires so a failed attempt does not leave a dirty limiter for the next
strategy. After the analog operating point converges, the mixed-signal DC
settle loop (§14.1) alternates analog re-solve with digital evaluation until
the digital state stops changing.

**set_temperature position.** `set_temperature(tolerances.temperature)` is
called once per element inside `setup_all` — after `allocate_unknowns`
(builder phase, already done) and before the first `load_dc`. A temperature
sweep re-calls `set_temperature(t_new)` and honors the returned
`Invalidation::Temperature` (recompute constants → restamp).

**limiting_report position.** Inside each Newton iteration: after `load_dc`
assembles the stamps and the linear solve produces a candidate, the solver
calls `limiting_report()` on every analog device. If any returns `Some`, the
report's `limited_value` is applied to its `net` in the Newton guess
(`apply_limiting_reports`) **before** the convergence test; convergence is
then false (the limiter is still active). The DC bypass (stamp cache, §9) is
also suppressed while any `limiting_report()` is non-`None`.

**Checkpoint/restore position.** `checkpoint_state()` is called on every
checkpointed element before each homotopy strategy's `ConvergencePlan::solve`;
`restore_state(&ckpt)` is called on strategy fallthrough (failed attempt →
next strategy) before the next strategy starts.

**Hook ordering table:**

| # | Hook | Precondition | Postcondition |
|---|------|--------------|---------------|
| 1 | `setup(ctx)` | Circuit built; `allocate_unknowns` done | Element initialized |
| 2 | `temp(t_nom)` | After `setup`, before first load | Temperature constants seeded |
| 3 | `init(sink)` | After `setup`; digital devices only | Initial digital events emitted |
| — | **homotopy loop start** | | |
| 4 | `ckpt()` | Before each strategy attempt | Per-element checkpoint stored |
| 5 | `update(state, ctx)` | Each Newton iteration, before load | Internal state refreshed |
| 6 | `load_dc(state, ctx)` | After `update` | DC stamps contributed |
| 7 | `limit()` | After linear solve, before convergence test | Limiting report applied to guess; convergence gated |
| — | *(converged? yes → settle; no → next Newton iter or strategy fallthrough)* | | |
| 8 | `restore(ckpt)` | On strategy fallthrough, before next strategy | Pre-attempt state rewound |
| — | **mixed-signal settle loop** (§14.1) | | |
| 9 | `accept(state, t, nets, sink)` | Analog converged | A2D events seeded |
| 10 | `seq(ctx)` / `comb(ctx, sink)` | After accept, per delta cycle | Digital nets updated |
| 11 | `opvars()` | After final operating point | Operating variables readable |
| 12 | `destroy()` | Circuit dropped | Element torn down |

### 19.3 AC analysis (`.ac`)

**Algorithm flow.** AC solves the DC operating point once (§19.2), then
freezes the linearization. For each frequency in the sweep, the driver builds
the AC context, asks each analog device for complex small-signal stamps
linearized at the DC point, and solves the complex linear system. AC is
linear at each frequency; the mixed-signal scheduler does not run during the
sweep.

**set_temperature position.** Inherited from the DC operating-point solve
(§19.2); AC does not re-call `set_temperature`.

**limiting_report position.** AC does not consult `limiting_report` — the
operating point is already converged and frozen; small-signal analysis is
linear.

**Checkpoint/restore position.** None. AC has no iteration that can dirty
state; the DC operating point was already accepted.

**Hook ordering table:**

| # | Hook | Precondition | Postcondition |
|---|------|--------------|---------------|
| 1–12 | *(DC operating point — §19.2)* | | |
| 13 | `load_ac(dc_point, ac_ctx, ctx)` | DC OP converged; once per frequency | Complex small-signal stamps contributed |
| 14 | `destroy()` | Circuit dropped | Element torn down |

### 19.4 Transient analysis (`.tran`)

**Algorithm flow.** The transient driver integrates from the start time to
`stop_time` over a fixed circuit topology, using TR-BDF2 (§10.3) as the sole
integration scheme. The time loop is a thin loop over named phase methods
(predict → attempt → assess → accept/reject), each with a specific hook
contract. The convergence plan owns the stepper (`propose_dt`/`reject_dt`
delegate to `plan.stepper()`); the unified event queue (`EventQueue`) is the
predict-step read path.

**Main loop structure.** Per accepted step:

1. **predict_step** — read the PI-proposed `dt` from the plan's stepper; build
   the unified `EventQueue` from the four sources (digital peek, analog
   `next_breakpoints`, scheduled sets, `$bound_step` hints); peek the earliest
   event time and clamp the landing point.
2. **attempt_step** — checkpoint the digital state, snapshot each element's
   device-internal state (`checkpoint_state`), snapshot the analog history,
   checkpoint the event queue; drain due events and run the digital settle
   (`seq_phase`/`comb_phase`) at the target time; then `execute_timestep`
   (TR-BDF2: TR Newton solve → BDF2 Newton solve).
3. **assess_step** — Milne LTE over node-voltage unknowns; skip if the step
   landed on a breakpoint or is the first step after a live set.
4. **accept_step** (on pass) — run `accept_timestep` (A2D bridge) + digital
   settle; commit the digital + event-queue checkpoints; **discard** the
   device-internal checkpoints (no `restore_state`); record the step; apply
   scheduled sets; advance the clock; `propose_dt` via the plan's stepper.
5. **reject_lte_step** / **reject_step** (on failure) — restore the digital
   checkpoint, **call `restore_state` on every checkpointed element**, restore
   the analog history, **roll back the event queue** (per-entry
   `RollbackBehavior`), reduce `dt` via the plan's stepper; at the dt floor,
   accept as-is with a warning.

**Phases within one iteration (TR-BDF2).** Each `execute_timestep` runs two
Newton sub-steps:
- **TR phase** — Trapezoidal over `γ·h` → `x_{n+γ}`. Each Newton iteration
  calls `update` → `load_transient` → `limit` (limiting_report gates
  convergence; `apply_limiting_reports` applies the clamped value before the
  test). A first-order predictor warm-starts from the two newest accepted
  rows when the previous step was accepted.
- **BDF2 phase** — Backward differentiation over `(1−γ)·h` → `x_{n+1}`,
  warm-started from `x_{n+γ}`. Same `update` → `load_transient` → `limit`
  cycle. Either phase failing rejects the whole step.

**Convergence/rejection criteria.** Newton convergence requires the update
test + residual test + no active `limiting_report()` (§15.1). LTE rejection
requires `milne > trtol` (and the step is not breakpoint-exempt). Newton
failure in either TR or BDF2 phase rejects the whole step. At the dt floor,
an LTE rejection is accepted as-is (warned); a Newton failure fails the run
loud.

**set_temperature position.** Called once per element inside `setup_all`,
before the first `load_transient`. Not re-called during the run unless a
temperature sweep is in flight.

**limiting_report position.** Inside **both** Newton sub-steps (TR and BDF2):
after each iteration's `load_transient` + linear solve, `limiting_report()` is
consulted; `apply_limiting_reports` applies the clamped value to the guess
before the convergence test; convergence is false while any report is `Some`.

**Checkpoint/restore position.** `checkpoint_state()` is called on every
checkpointed element in `attempt_step` before the digital settle and the
TR-BDF2 solve. `restore_state(&ckpt)` is called in **both** `reject_lte_step`
and `reject_step` before the retry. The checkpoints are discarded on
acceptance (`accept_step`).

**Hook ordering table:**

| # | Hook | Precondition | Postcondition |
|---|------|--------------|---------------|
| 1 | `setup(ctx)` | Circuit built | Element initialized |
| 2 | `temp(t_nom)` | After `setup`, before first load | Temperature constants seeded |
| 3 | `init(sink)` | After `setup`; digital devices | Initial digital events emitted |
| — | **initial operating point** (§19.2, steps 4–11) | | |
| — | **time loop** (per step) | | |
| 4 | `bp(from, horizon)` | In `predict_step` | Breakpoint times contributed to `EventQueue` |
| 5 | `ckpt()` | In `attempt_step`, before settle + solve | Per-element checkpoint stored |
| 6 | `seq(ctx)` | After due events drained, at target time | Register banks committed; clock edges detected |
| 7 | `comb(ctx, sink)` | After `seq_phase` | Output events emitted |
| 8 | `update(state, ctx)` | Each Newton iter (TR + BDF2), before load | Internal state refreshed |
| 9 | `load_tran(state, tran_ctx, ctx)` | After `update` | Companion stamps contributed |
| 10 | `limit()` | After linear solve, before convergence test | Limiting applied to guess; convergence gated |
| — | *(TR + BDF2 converged? yes → assess; no → reject)* | | |
| 11 | `restore(ckpt)` | On reject (LTE or Newton), before retry | Pre-attempt state rewound |
| 12 | `accept(state, t, nets, sink)` | On accept, after settle | A2D events seeded; accept-gated state advanced |
| 13 | `suggest_dt(...)` | After accept, before next predict | Per-element dt floor contributed |
| 14 | `destroy()` | Circuit dropped | Element torn down |

### 19.5 Noise analysis (`.noise`)

**Algorithm flow.** Noise solves the DC operating point once (§19.2), builds
the linearized small-signal matrix pattern, then sweeps frequency. For each
frequency: assemble complex AC stamps, solve the adjoint system (transpose +
unit excitation at the output), ask each analog device for current-noise PSD
sources, and accumulate per-source output PSD. Finally integrate the output
PSD over frequency (trapezoidal) for the RMS output noise.

**set_temperature position.** Inherited from the DC operating-point solve;
noise does not re-call `set_temperature`.

**limiting_report position.** None. Noise is a small-signal analysis around
the converged operating point; no Newton loop runs during the sweep.

**Checkpoint/restore position.** None. Noise does not iterate; the operating
point is frozen.

**Hook ordering table:**

| # | Hook | Precondition | Postcondition |
|---|------|--------------|---------------|
| 1–12 | *(DC operating point — §19.2)* | | |
| 13 | `load_ac(dc_point, ac_ctx, ctx)` | DC OP converged; once per frequency | Complex small-signal stamps contributed |
| 14 | `noise_psd(dc_point, ac_ctx)` | Once per frequency | Per-source current-noise PSD returned |
| 15 | `destroy()` | Circuit dropped | Element torn down |

### 19.6 Periodic steady state (`.pss`)

**Algorithm flow.** PSS finds a periodic orbit by **single shooting**: Newton
iteration on `g(x₀) = x(t₀+T) − x₀`, where each evaluation of `g` is one
transient over one period re-entered from `x₀`. Per shot: construct a
`TransientSolver` with full-state re-entry (`digital_hidden_restore` seeds
hidden register state + analog solution + digital snapshot), run one period,
compare the endpoint to `x₀`. The first Jacobian is finite-differenced (one
extra shot per unknown); later iterations reuse it with Broyden updates. The
Newton update is damped. Converge when `max_i |x_i(T) − x_i(0)|` is within the
shooting tolerance. Then verify the orbit repeats and digital state is
periodic.

**set_temperature position.** Inherited from the operating-point solve that
seeds the shooting (or the pre-roll transient); PSS itself does not re-call
`set_temperature`.

**limiting_report position.** Inside every shot's transient Newton sub-steps
(TR + BDF2) — same as transient (§19.4). The shooting Newton loop itself does
not consult `limiting_report` directly; it sees only the endpoint residual.

**Checkpoint/restore position.** Inside every shot's transient run — same as
transient (§19.4): `checkpoint_state` per attempt, `restore_state` per reject.
The shooting re-entry uses `digital_hidden_restore` (full-state re-entry
contract, §4.2) to seed each shot — that is a separate mechanism from
per-step rollback (PSS records every step; per-step rollback fires on reject
only).

**Hook ordering table:**

| # | Hook | Precondition | Postcondition |
|---|------|--------------|---------------|
| 1 | `setup(ctx)` | Circuit built | Element initialized |
| 2 | `temp(t_nom)` | After `setup`, before first shot | Temperature constants seeded |
| 3 | `init(sink)` | After `setup`; digital devices | Initial digital events emitted |
| 4 | `restore_hidden(state)` | Per shot, before transient run | Hidden register state + snapshot seeded for re-entry |
| — | **per-shot transient run** (§19.4 steps 4–13, one period) | | |
| — | *(monodromy converged? yes → verify; no → damped Newton update → next shot)* | | |
| 5 | `destroy()` | Circuit dropped | Element torn down |

### 19.7 Sensitivity analysis (`.sens`)

**Algorithm flow.** DC sensitivity computes `∂(output)/∂(param)` at the
operating point over the restamp path (§10.5). The whole request is validated
up front. Then for each `(element, parameter)` pair: perturb the parameter by
a central-difference step (`±dp`), re-solve the DC operating point at each
side on the same compiled circuit (each re-solve runs the full DC homotopy
cascade, §19.2), and difference the requested outputs across the two points.

**set_temperature position.** Inherited from each DC re-solve; `.sens` does
not re-call `set_temperature` itself (the DC driver does, inside each
operating-point solve).

**limiting_report position.** Inside each DC re-solve's Newton iterations —
same as DC (§19.2). `.sens` itself does not consult `limiting_report`.

**Checkpoint/restore position.** Inside each DC re-solve's homotopy cascade —
same as DC (§19.2). `.sens` itself does not checkpoint; it re-solves from
scratch on each side of the central difference.

**Hook ordering table:**

| # | Hook | Precondition | Postcondition |
|---|------|--------------|---------------|
| 1 | `setup(ctx)` | Circuit built | Element initialized |
| 2 | `temp(t_nom)` | After `setup`, before first DC solve | Temperature constants seeded |
| — | **central-difference loop** (per param) | | |
| 3 | `set_param(name, v+dp)` | Before `+dp` DC re-solve | Parameter restamped; invalidation returned |
| 4 | *(DC operating point — §19.2 steps 4–11)* | | |
| 5 | `set_param(name, v−dp)` | Before `−dp` DC re-solve | Parameter restamped |
| 6 | *(DC operating point — §19.2 steps 4–11)* | | |
| 7 | `set_param(name, v)` | After differencing | Parameter restored to nominal |
| 8 | `opvars()` | After final operating point | Operating variables readable |
| 9 | `destroy()` | Circuit dropped | Element torn down |
