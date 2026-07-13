# Part VII — Solver Specification

This Part defines the solver contract: the device ABI consumed by analyses, the
analog and digital variable namespaces, the numerical algorithms for DC, AC,
transient, noise, and transfer-function analysis, and the convergence aids that
make mixed-signal simulation deterministic.

The solver is below elaboration and device construction. It receives a fixed set
of devices, a fixed analog variable map, and a fixed digital net namespace. It
does not know source syntax or elaboration internals. A compiled PHDL module, a
plugin device, and an external model are equivalent once they present the device
ABI defined here.

## Contents

- §1 Position and governing rules
- §2 Circuit instance
- §3 Analog device ABI
- §4 Digital device ABI
- §5 Device loading and external models
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

---

## §1 Position and governing rules

The solver executes an already-built circuit. Structure is immutable during an
analysis: devices may change their internal state and stamp values, but they may
not create or delete devices, nodes, branches, or digital nets.

Three rules govern this Part.

**Device ABI only.** The solver communicates with every model through the
`Device` boundary and its analog and digital facets. A model's origin is not
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

The circuit instance offers analyses over the same topology. A DC analysis,
transient analysis, AC sweep, noise analysis, and transfer-function analysis all
consume the same device set and analog/digital namespaces.

---

## §3 Analog device ABI

An analog device participates in modified nodal analysis (MNA). It contributes
matrix and right-hand-side stamps for one or more analyses, may expose operating
variables, may emit noise sources, and may request convergence or timestep
controls.

### 3.1 Analog lifecycle methods

| Method | Contract |
|--------|----------|
| `set_temperature(t)` | Set the device temperature for temperature-dependent parameters. `t` is absolute temperature in kelvin. |
| `update(state, context)` | Refresh internal model state from the current analog solution history before loading stamps. |
| `accept_timestep(state, context, nets, sink)` | Commit an accepted solution point. A mixed-signal analog device may emit digital events through `sink`. |
| `initial_conditions()` | Return requested initial branch voltages as `(plus, minus, value)` tuples. A missing terminal means ground. |
| `read_opvars()` | Return named operating-point values for diagnostics and result extraction. |
| `limiting_active()` | Report that device-side limiting is still active; convergence must not be accepted while true. |
| `bound_step_hint()` | Return the maximum desirable next timestep. Infinity means no bound. |

`state` is the analog solution history. Row 0 is the current/latest solution;
additional rows are previous accepted solutions when the analysis requires
history. `context` carries tolerances, time, temperature, homotopy parameters,
and the integration method.

### 3.2 Analog loading methods

| Method | Analysis | Return |
|--------|----------|--------|
| `load_dc(state, context)` | DC operating point | Real MNA stamps for the nonlinear algebraic system. |
| `load_transient(state, transient_context, context)` | Time-domain analysis | Real MNA stamps for the implicit companion model at the current timestep. |
| `load_ac(dc_point, ac_context, context)` | Small-signal AC/noise | Complex MNA stamps linearized about the DC operating point. |
| `noise_current_psd(dc_point, ac_context)` | Noise | Current-noise sources as terminal pairs plus one-sided PSD values. |

An analog device that does not participate in an analysis may return an empty
stamp list only when the physical model genuinely has no contribution in that
analysis. An unsupported construct must fail before this ABI is reached or must
raise a loud device-construction/load error.

### 3.3 Analog ABI types

| Type | Meaning |
|------|---------|
| `AnalogReference` | Reference to one analog variable. Ground has no MNA index; every other solved variable has one dense index. |
| `Stamp<Ref, Scalar>` | Either `Matrix(row, col, value)` or `Rhs(row, value)`. The scalar is real for DC/transient and complex for AC/noise. |
| `Noise` | A current-noise source between two analog references with PSD in A²/Hz. |
| `Context` | Solver tolerances, temperature, time, homotopy state, and integration controls. |
| `TransientAnalysisContext` | Current time, current timestep, final time, previous timestep, and active integration order. |
| `AcAnalysisContext` | Current frequency. |

---

## §4 Digital device ABI

A digital device participates in event-driven simulation. It declares the nets it
reads and drives, initializes its outputs, and evaluates in two phases so
register chains have non-blocking semantics.

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
| `samples_analog()` | True when the model must be evaluated after an accepted analog solve even if no digital input net changed. |
| `has_input_on(changed)` | Convenience sensitivity test: true when any input net is in the changed set. |

The two-phase protocol is normative. All woken sequential phases observe the
same pre-settle net snapshot before any combinational output is recomputed.

### 4.3 Digital event ordering

Digital events are ordered by `(time, sequence)`. All events at the current time
or within the scheduler equality tolerance of the current time are drained into
the current delta cycle. Zero-delay events emitted during combinational
evaluation are applied in the same simulation time and may trigger another delta
iteration.

---

## §5 Device loading and external models

Device loading is outside the numerical algorithms but inside the solver ABI
contract. A loader constructs values that implement the `Device` boundary:

| Device kind | Required facets |
|-------------|-----------------|
| Pure analog | `Device` + analog facet. |
| Pure digital | `Device` + digital facet. |
| Mixed signal | `Device` + both analog and digital facets. |

A device loader receives already-resolved terminal bindings: analog terminals as
analog references and digital terminals as digital nets. Parameter values are
already elaborated. The loader must either construct a faithful device or fail
loud with a diagnostic naming the model and missing capability.

Native PHDL-compiled devices, native plugin devices, and wrapped external model
ABIs all lower into this same boundary. An OSDI v0.4 model is not a solver-native
object; an OSDI loader must parse the model descriptor, bind its terminals and
parameters, and wrap the compiled model as an analog device. The solver core
does not require an OSDI loader to exist, and an unavailable OSDI feature is a
plugin/device load error rather than a silent solver behavior.

The `Device` boundary is a downcastable capability set. A model that returns both
facets is mixed-signal; a model that returns neither facet is invalid for solve
and must not be admitted into a circuit instance.

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

The factory must declare whether the produced device is analog, digital, or
mixed-signal. The returned device must expose the corresponding facets described
in §3 and §4.

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
must allocate those unknowns before the circuit instance is finalized. If it
cannot, loading fails loud with a diagnostic naming the model and the missing
allocation capability.

### 5.3 Device-loading validation

| Rule | Failure |
|------|---------|
| Required terminal is unbound | Device-construction error. |
| Terminal domain does not match the factory's declared binding | Device-construction error. |
| Required parameter is absent or has an unsupported value | Device-construction error. |
| Factory returns a device with no analog or digital facet | Device-construction error. |
| Factory needs internal analog variables but no allocation seam is available | Device-construction error. |

---

## §6 Analog variable and node labels

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
responsibility, not part of the solver's numerical contract.

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
   - Ask devices to update from the current state.
   - Collect DC stamps from all analog facets.
   - Add any active homotopy conductances (§15.4).
   - Solve the linearized system.
   - Apply solver-side damping/limiting (§15.2).
   - Accept convergence only if both the update test and residual test pass and
     no device reports active limiting.
4. If plain Newton fails, attempt gmin stepping. If that fails, attempt source
   stepping.
5. Run the mixed-signal DC settle loop (§14.1) until digital state stops changing
   or the mixed-signal iteration cap is reached.
6. Return a mapping from every indexed analog variable to its solved value.

DC ignores dynamic charge history except where a device's DC model explicitly
depends on its internally updated operating point. Time-varying sources are
evaluated at the DC context defined by the source model.

---

## §10 Transient analysis

Transient analysis integrates from `t = 0` to `stop_time` over a fixed circuit
topology.

### 10.1 Initial state

The transient initial state is built from a DC operating point. Device
initial-condition requests and user initial-condition seeds overlay that DC
point. For a branch voltage initial condition `(plus, minus, value)`, the
initial value is:

```text
V(plus) = V(minus) + value
```

where a missing `minus` terminal means ground.

Initial-condition seeds populate enough solution history for the companion model
to start without an artificial first-step discontinuity. They are seeds, not a
guaranteed hold constraint unless the device model stamps such a constraint.

### 10.2 Step algorithm

For each step:

1. Choose a proposed timestep from the current timestep controller.
2. Clamp the target time to the analysis stop time and to the next pending
   digital event time.
3. Checkpoint the digital state.
4. Apply digital events exactly at the target time before the analog solve.
5. Solve the analog implicit companion system for the interval ending at the
   target time.
6. If the analog solve succeeds:
   - Service analog-to-digital acceptance hooks and run digital evaluation at
     the target time.
   - Commit the digital checkpoint.
   - Record the step if it is at or after `record_from`.
   - Advance integration history and grow the proposed timestep within bounds.
7. If the analog solve fails:
   - Roll back the digital checkpoint.
   - Reduce the proposed timestep and retry.
   - If the minimum timestep is reached and the solve still fails, the analysis
     fails loud.

### 10.3 Integration method

The transient companion model uses an implicit integration method selected by the
solver context. The default method is Gear/BDF order 2. The first accepted steps
use order 1 until sufficient history exists; then the method may use the
configured order, capped by available history.

Trapezoidal integration is permitted as a second-order implicit method. Gear
orders outside the supported range clamp to a valid conservative coefficient;
they must not panic or produce a non-finite timestep calculation.

### 10.4 Results

Each recorded transient point contains:

| Field | Meaning |
|-------|---------|
| time | Accepted simulation time. |
| analog values | Solved value of each indexed analog variable. |
| digital snapshot | Logic value of every digital net after digital evaluation at that time. |

`record_from` affects recording only. The solver still integrates from `t = 0`
because skipped early states influence later history.

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

Mixed-signal behavior is expressed by devices that implement both analog and
digital facets, or by paired devices that communicate through explicit analog
and digital nets. There is no implicit converter insertion.

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

   Node rows use current tolerance. Branch-equation rows use voltage tolerance.

Device-side limiting is an additional gate: if any analog device reports
`limiting_active()`, Newton convergence is false even when the numeric tests
pass.

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

### 15.4 Gmin and gmin stepping

The solver context contains a normal `gmin`, used by device models for weak
conductance stabilization, and an extra homotopy conductance `gmin_extra`.

During gmin stepping, every non-ground node receives an added conductance to
ground. The solver starts from an easy, strongly shunted problem and reduces the
extra conductance toward zero, warm-starting each step from the previous
solution. The final accepted operating point is always solved with
`gmin_extra = 0`.

The extra conductance is applied only to node-voltage unknowns, never to branch
current unknowns.

### 15.5 Source stepping

Source stepping scales independent forced source values from zero to full
strength. It is attempted after plain Newton and gmin stepping fail. Each scale
point warm-starts from the previous point. A temporary shunt may be held during
the source ramp and then ramped out so the final solve is exact.

A model whose source value is affected by source stepping must multiply that
source by the context's source scale. Models that do not represent independent
sources ignore the scale.

### 15.6 Initial guesses, node sets, and device initial conditions

Node-set values and user initial conditions seed Newton history; they are not
themselves constraints. Device initial conditions seed transient history and may
become constraints only when the device stamps a constraint.

The solver may push the same initial condition into multiple history rows when a
multistep integration method needs a consistent starting history.

### 15.7 Timestep rejection and rollback

Transient convergence failure rejects the candidate step. Rejection restores the
digital state to the checkpoint taken before the candidate endpoint, reduces the
timestep, and retries. A step is committed only after the analog solve succeeds
and same-time digital acceptance has run.

### 15.8 Timestep bounds and breakpoints

Devices may request a maximum timestep, and sources may expose breakpoints where
the solver should land exactly. A timestep controller that supports these hooks
must take the minimum positive bound that does not overshoot the stop time or the
next digital event. If no hook is available, the solver still must honor digital
event times and the global minimum/maximum timestep limits.

### 15.9 Linear-solver safety

If the linear solve returns a non-finite value, the nonlinear solve fails loud.
The solver must not continue from NaN or infinity.

---

## §16 Validation and failure rules

Every failure in this Part is an analysis or device-load error. These errors are
not parse or elaboration errors unless the invalid condition is detectable before
device construction.

| Section | Rule | Failure |
|---------|------|---------|
| §2 | Circuit contains a device with neither analog nor digital facet | Device-load error. |
| §3 | Unsupported analog behavior reaches the ABI | Device-load or analysis error; never an empty fake stamp. |
| §4 | Digital boundary changes during an analysis | Analysis error. |
| §4 | Digital event targets a nonexistent net | Analysis error. |
| §5 | External model or plugin cannot bind required terminals/params | Device-load error. |
| §6 | Stamp references an unmapped non-ground/non-branch variable | Analysis error. |
| §8 | Analysis-time loading changes matrix dimension or sparsity contract | Analysis error. |
| §9 | DC fails plain Newton, gmin stepping, and source stepping | Convergence failure. |
| §10 | Transient reaches minimum timestep without convergence | Convergence failure. |
| §11 | AC frequency point cannot solve its linear system | Analysis error for that sweep. |
| §12 | Noise output/reference node cannot be resolved | Analysis error. |
| §13 | Unsupported transfer-function source form is requested | Analysis error. |
| §14 | Digital delta cycle does not settle within the iteration cap | Digital convergence failure. |
| §15 | Linear solve returns NaN or infinity | Convergence failure. |
