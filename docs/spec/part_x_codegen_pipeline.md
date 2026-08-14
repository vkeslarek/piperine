# Part X — Codegen Pipeline (`piperine-codegen`)

This Part defines the compilation contract between an elaborated design (Part II's
output, the Piperine Object Model) and the solver's `Element` ABI (Part VII). It
covers the stages that turn a POM `Design` into native code: the resolved
expression/statement form, symbolic differentiation, analog-body flattening,
Cranelift emission, compiled kernels, and the device/circuit wrapping that presents
kernels to the solver.

`piperine-codegen` sits strictly between elaboration and the solver. It does not
parse source syntax (that is Part I/II's job) and it does not run numerical
algorithms — Newton iteration, integration, convergence homotopy (that is Part
VII's job). Its one input is a `Design`: an immutable, monomorphic object model in
which every module's hierarchy, ports, and instances are already resolved (Part II
§9's structural unrolling and Part IV's POM are prerequisites). Its one output,
per module, is a pair of compiled kernels (analog and/or digital) plus the device
wiring that turns them into `Element`s a `CircuitInstance` can stamp.

There is no interpreted execution path anywhere in this pipeline. A
Verilog-AMS `analog`/`digital` body compiles to native code once, at elaboration/build time
(MD-18 — normatively restated in §11.1); simulation never re-invokes this
pipeline.

## Contents

- §1 Pipeline overview and stage contracts
- §2 Resolve — POM to resolved form
- §3 Symbolic differentiation
- §4 Flatten — analog body lowering
- §5 Emit — Cranelift emission, CSE, and the SimCtx ABI
- §6 Kernel — compiled analog and digital products
- §7 Device — kernels as solver Elements
- §8 Circuit compilation — instance structure and CircuitCompiler
- §9 The piperine-signal delegation boundary
- §10 Public surface and visibility
- §11 Error handling and construct coverage — the fail-loud contract

---

## §1 Pipeline overview and stage contracts

The pipeline is a fixed sequence of five named stages, each named for the
compilation stage it performs rather than the implementation technology it
happens to use:

```text
pom::Design ──resolve──▶ (resolved form) ──flatten──▶ FlatAnalog ──emit──▶ (native code) ──kernel──▶ AnalogKernel/DigitalKernel ──device──▶ solver Element
```

**Stage ownership.** Each stage SHALL own a single, named transformation and
SHALL NOT reach backward into an earlier stage's representation or forward into
a later stage's concerns:

| Stage | Input | Output | Owns |
|---|---|---|---|
| **resolve** | POM `Design`, one module at a time | `LoweredBody` (resolved expression/statement form) | Name interning, POM→resolved lowering, symbolic differentiation |
| **flatten** | resolved `AnalogBody` | `FlatAnalog` | Reducing one module's analog behavior to pure per-branch contribution expressions |
| **emit** | `FlatAnalog` / resolved digital body | native Cranelift functions | Expression-to-machine-code translation, common-subexpression elimination, the runtime ABI struct |
| **kernel** | emitted functions | `AnalogKernel` / `DigitalKernel` | Grouping compiled functions into a per-module, `Arc`-shared compiled artifact with a uniform calling convention |
| **device** | compiled kernel(s) | `Element` implementation | Per-instance state (parameters, operator history, register banks) and MNA/event wiring to the solver |

**Per-module scope, except device/circuit.** Every stage through `kernel`
operates on **one module's own behavior** — the analog/digital body as authored,
independent of how many times or where the module is instantiated. Instance
*structure* (which modules are wired to which nets, with which parameter
overrides) is a `device`-stage concern read directly from the POM at
circuit-build time (§8) — it is never resolved, flattened, or emitted, because it
does not vary per compilation, only per instantiation.

**Sharing.** A module compiles once. Its `AnalogKernel`/`DigitalKernel` is
`Arc`-shared across every instance of that module in the circuit; per-instance
data (parameter values, runtime-operator state, register banks) lives entirely
in the device layer (§7), never in the kernel.

*Rationale.* This is the same invariant Part VII §2 states from the solver's
side ("a compiled Verilog-AMS module, a plugin device, and an external model
are equivalent once they present the one `Element` ABI") — codegen is simply
the producer that makes a compiled Verilog-AMS module satisfy that
equivalence.

---

## §2 Resolve — POM to resolved form

The resolve stage lowers a POM `Design`'s per-module bodies into codegen's own
resolved form: names replaced by interned ids, control structures kept as POM
`Stmt`/`Expr` (no new AST is introduced), and every analog runtime operator
lowered to a uniform *marker call* convention.

### 2.1 The resolved form

The resolved form SHALL satisfy, for every module it covers:

- Every referenceable name (variable, port, node, parameter) SHALL resolve to
  an interned id from that module's own symbol table (Part IV's POM identifiers
  are not reused directly — resolve builds its own dense numbering per module).
- Ground SHALL be the one reserved node id, identical in meaning to Part VII
  §6's ground reference.
- No generic, lambda, bundle, or structural-control construct SHALL survive
  into the resolved form — Part II's elaboration passes (monomorphization,
  bundle expansion, structural unrolling) SHALL have already removed them. The
  resolved form is monomorphic by construction, mirroring the POM it reads.

*Note.* The resolved form is **codegen-private**. Nothing outside this crate
depends on its shape; Verilog-AMS (`piperine-lang`, whose parser/elaborator
modules absorbed the former standalone `piperine-vams` crate, MD-47) is the
pipeline's sole frontend (PHDL retired, MD-44/MD-45), so there is no
cross-crate IR contract to keep stable (a former standalone IR crate existed
when PHDL and an earlier Verilog-AMS frontend attempt both fed it — that
dual-frontend era is gone, and the resolved form retreated to being this
crate's internal stage, now fed by the one surviving frontend).

### 2.2 The lowering pass — one resolved body per module

Lowering SHALL produce exactly one `LoweredBody` per module, covering that
module's:

- symbol table,
- resolved port list,
- resolved analog body (if the module has an `analog` block),
- resolved digital body (if the module has a `digital` block).

Lowering SHALL NOT resolve instance wiring (connections, parameter overrides)
— that is left entirely to the device/circuit stage (§8), which reads the POM
directly at build time. This split exists because instance wiring is not a
property of a module's own behavior; the same `LoweredBody` is correct
regardless of how many instances of the module exist or how they are
connected.

### 2.3 Analog runtime operators — the marker-call convention

Every analog runtime operator (`ddt`, `idt`, `idtmod`, `ddx`, `delay`, `slew`,
`transition`, `table`, and the `laplace_*`/`zi_*` family — all 5 `laplace_*`
and 4 `zi_*` LRM variants are registered in
`AnalogOpRegistry::with_builtins`, Part I's extension surface, MD-24) SHALL
lower to a POM `Expr::Call` of the fixed shape:

```text
__<op>(state_id, resolved_args...)
```

where `state_id` is an integer literal identifying the operator's allocated
runtime-state slot. This is a **uniform dispatch convention**, not a new
expression variant: the flattener (§4) and the emission `Builder` (§5) both
dispatch on the marker's name string, so adding a new analog operator requires
extending the marker table, never adding a new `Expr` case to the resolved
form itself.

**Operator registry.** Each operator is one small type implementing a
one-method `AnalogOp` trait (`lower(args, ctx) -> Expr`) that allocates the
runtime-state slot and emits the marker call; the registry maps operator names
to their `AnalogOp` impl. This SHALL remain a data table plus small per-operator
types — never a macro-generated dispatch (the project's binding Rust-idiom
rules forbid `macro_rules!`/proc-macro codegen for repeated patterns; see the
project's binding conventions on data-table dispatch).

**State allocation.** An operator that carries runtime state across
timesteps (`ddt`, `idt`, `idtmod`, `delay`, `slew`, `transition`) SHALL
allocate exactly one state slot per call site at lowering time, tagged with
the operator's `StateKind` (e.g. `StateKind::Idt { ic }` carries the initial
condition expression). This slot id is what the device layer (§7) later
services between timesteps — resolve only allocates the slot, it never
services it.

---

## §3 Symbolic differentiation

Symbolic differentiation is the correctness-critical core of the analog path:
every Jacobian entry the solver's Newton iteration consumes (Part VII §3.2's
`load_transient`/`load_ac` contract) is a differentiated expression, not a
finite-difference approximation. It SHALL NOT be reimplemented per device, and
a construct that reaches this stage without a differentiation rule SHALL fail
loud (§11) rather than silently contribute a zero derivative.

### 3.1 What is differentiated, and by what rule

Differentiation operates on the flattened contribution expressions (§4) — one
Jacobian entry per contribution, per branch voltage the contribution reads.
Three differentiation variables are defined, each its own entry point sharing
one chain-rule walk:

| Entry point | Differentiates w.r.t. | Seed rule | Consumer |
|---|---|---|---|
| `d_dv` (branch voltage) | `V(plus, minus)` for a specific branch | the differentiated branch's own `V(p,m)` read seeds `1`, every other branch read seeds `0` | The residual's Jacobian (Part VII §3.2, §8) |
| `d_dnode` (node potential) | a single node's potential | a `V` read touching that node seeds `±1` by polarity, `0` otherwise | The `ddx` operator (Part V analog operators) |
| `d_dp` (parameter) | a named parameter | an `Ident` leaf matching the parameter name seeds `1`, every other ident and every branch read seeds `0` | The `∂F/∂p` sensitivity kernel (Part VII §17.2) |

All three share one differentiation walk; only the leaf-seeding closures
differ. This SHALL remain true of any future differentiation entry point —
one differentiator, parameterized by its seed rule, never a second
hand-written pass.

### 3.2 Transparency rules

Two classes of sub-expression are differentiation boundaries by design, not by
omission:

- **State reads are constants within a Newton iteration.** `__state_load`
  reads (runtime-operator state serviced by the device between steps, §2.3)
  and `__ddt` reads (the reactive/charge half, differentiated separately by
  the charge Jacobian, §6.2) SHALL differentiate to zero within the residual's
  own Jacobian pass — their contribution to the solve enters through the
  companion-model stamp (Part VII §8), not through this differentiation.
- **`$limit` is a value site, not a differentiated function.** The limiter
  (§9) computes a clamped voltage `vlim` used *in place of* the raw branch
  voltage; differentiation treats the emitted `$limit` call as an ordinary
  value (its own derivative flows through normally as `d(vnew)/dV`), and does
  **not** differentiate through the limiter formula itself. A refactor that
  moves where `$limit`'s formula is evaluated (§9, §11) SHALL NOT require any
  change to this stage — this is the explicit boundary the delegation in §9
  was designed to preserve.

### 3.3 The temp-tape mechanism

A flattened body may share sub-expressions across multiple contributions
(common in junction devices with several controlling terminals). Rather than
re-inlining a shared sub-expression at every use — which would explode a
many-branch device's Jacobian into a combinatorially large expression tree —
shared work is recorded once on a **temp tape** (`__temp(id)` reads the
recorded value, `__dtemp(id)` reads its already-differentiated counterpart)
and every use references the tape entry.

**Self-consistency requirement.** A differentiation pass over an expression
containing `__temp(id)` reads SHALL install a matching `__dtemp`-equivalent
tape (the derivative of each `__temp` entry, in the same order) before
emitting the result. Higher-order differentiation (e.g. `.disto`'s
second/third derivative kernels) SHALL use a distinctly-named tape marker per
derivative order so that a multi-level temp chain resolves against the
correct order's tape, never a lower order's.

---

## §4 Flatten — analog body lowering

Flattening reduces one module's resolved analog body to `FlatAnalog`: a fixed
set of pure, per-branch contribution expressions ready for emission. This is
**not** the same pass as hierarchy flattening (Part II's structural-unrolling
family, which collapses instance nesting at elaboration time — Part II §9);
this flatten stage operates entirely within one already-monomorphic module's
own behavior, turning its statement-shaped body into expression-shaped
contributions.

### 4.1 What flattening resolves

Flattening SHALL, for the analog body it is given:

- resolve every variable assignment symbolically — an `Ident(name)` reference
  becomes the assigned value's expression, not a runtime variable read (analog
  bodies have no mutable state outside runtime operators and the persistent
  var bank, Part VII §3.4);
- fold every `if`/`else` and `match` path condition into the resulting
  contribution expression (a `select`-shaped ternary at emission, §5), so a
  contribution's value is a single unconditional expression per branch, never
  a control-flow graph;
- inline every user function call at its call site, rather than emitting a
  separate compiled function per `fn` (an analog `fn` is a macro-like
  expansion, not a call target — Part I's function model, if the language
  spec distinguishes analog and non-analog `fn` bodies);
- split every `ddt(x)` marker into its resistive part (contributes `0` to the
  residual) and its charge part (contributes `x` to the reactive/charge
  kernel, §6.2) — a `ddt` contribution is never a residual term directly;
- lower every `idt(x, ic)` marker to its implicit-Euler companion-model form
  (the state-update relation the device services between steps, Part VII
  §10.3);
- resolve every `ddx(x, node)` marker to its compile-time derivative via §3's
  `d_dnode`, since `ddx` is a *static* differentiation request, not a runtime
  operator;
- leave every `__state_load(id)` marker as-is — it is a placeholder for a
  value the device supplies at load time (runtime-operator output, event
  state), not something flattening can resolve statically.

### 4.2 FlatAnalog's shape

The flattened output SHALL separate, as independent lists, every category of
effect a module's analog body may produce:

| Category | Meaning |
|---|---|
| Contributions | Resistive per-branch residual terms |
| Forces | `V`/`I`-source branch unknowns and their stamps (Part VII §3.3's force-branch allocation) |
| Events | Runtime analog event triggers (`cross`/`above`/`@timer`) and their fired-actions |
| Diagnostics | `$strobe`/`$display`-class statements reached during evaluation |
| `ac_stim` rows | Per-source AC stimulus magnitude/phase contributions (Part VII §11) |

A category with no entries contributes nothing to compilation — an empty
category SHALL NOT force a compiled function to exist for it (§6.1's
capability-gating pattern is downstream of this: flatten's emptiness is what
`kernel` turns into `None`).

---

## §5 Emit — Cranelift emission, CSE, and the SimCtx ABI

Emission is the one stage that produces actual machine code: it walks
`FlatAnalog` (or the resolved digital body) and a POM `Expr`/`Stmt` tree,
translating each node to Cranelift IR via a shared `Builder`.

### 5.1 The Codegen trait — one dispatch point per expression variant

Expression-to-Cranelift translation SHALL be reached through one trait with
one method per POM expression *category* (not one method per node kind
combinatorially) — adding a new expression variant to the language SHALL
require exactly one new match arm in this dispatch, never a second
independent lowering path. Analog- and digital-context emission share the
same `Builder` type; analog-only fields (branch voltages, parameter/state
pointers, the CSE cache) SHALL be present only when the `Builder` is
constructed for analog emission, and analog-context emission methods SHALL
NOT be reachable from a digital-context `Builder`.

### 5.2 Common-subexpression elimination

Analog emission SHALL cache every emitted sub-expression under a **structural
key** and SHALL reuse the cached Cranelift value for any later
structurally-equal sub-expression within the same compiled function, rather
than re-emitting it. The key space SHALL distinguish, at minimum:

- literal constants (by bit pattern),
- bank loads (by bank + byte offset — state, persistent-var, or `SimCtx`
  bank),
- unary/binary/ternary operations (by operator tag plus operand keys),
- math-builtin calls (by function name plus argument keys),
- limiter calls (by the `$limit` call's allocated slot index — §9).

**The CSE invariant.** A repeated, structurally-equal analog expression
(including a `$limit` call at the same slot) SHALL emit **exactly one**
computation per distinct key within a compiled function, regardless of how
many source-level positions reference it — including positions reached
through function inlining (§4.1) or a compiled parameter-Jacobian pass (Part
VII §17.2). This is a correctness-adjacent performance guarantee: without it,
a Newton iteration's hot path could silently duplicate an expensive
computation (e.g. a junction limiter formula) per use rather than per
distinct value.

### 5.3 The runtime ABI — SimCtx

Every compiled analog function receives, in addition to its
voltage/parameter/state arrays, one read-only struct carrying the live
simulator state that is not otherwise addressable as a node/branch/parameter:
simulation temperature, absolute time (`$abstime`), element multiplicity
(`$mfactor`), matrix-regularization conductance (`$simparam("gmin")`), the
current solver step and stop time, the bitmask of user-supplied parameters
(`$param_given`), the running analysis identity (`$analysis`), the AC
analysis frequency (used by frequency-dependent noise sources), and the DC
source-stepping scale factor (`$simparam("sourceScaleFactor")`, consulted
only while a source-stepping homotopy is active, Part VII §15.6).

**Layout stability.** This struct's field layout SHALL be `#[repr(C)]` and
SHALL be treated as an ABI contract: emission reads its fields by fixed byte
offset, so a field addition SHALL be appended, never inserted, to avoid
invalidating already-compiled offset constants — though in practice every
compilation happens fresh per build (§11.1's fail-loud-on-recompile
invariant), so this is a within-build consistency concern rather than a
cross-version binary-compatibility one.

**External symbol linkage.** A compiled kernel MAY link against a fixed table
of externally-defined native functions rather than emitting a formula as
inline Cranelift IR. Two such tables exist: the math-intrinsic table (Part V
§1's `extern fn` declarations, backed by libm) and, for the three `$limit`
kinds, `piperine-signal`'s limiter formulas (§9). A linked symbol's emission
SHALL be an ordinary function call (subject to the same CSE rule as any other
expression, §5.2) — this is not a separate code path from ordinary
expression emission, only a different value for "what does the call target."

---

## §6 Kernel — compiled analog and digital products

The kernel stage groups a module's emitted functions into one compiled,
`Arc`-shared artifact with a uniform calling convention. There is no
interpreted fallback: every module with an `analog` and/or `digital` block
compiles fully to native code, once, before any analysis runs.

### 6.1 AnalogKernel — capability-gated compiled functions

`AnalogKernel` SHALL expose one *always-present* core (residual and Jacobian
functions, terminal ordering, parameter/state layout) plus a fixed set of
*optional* capabilities, each present exactly when the module's analog body
requires it:

| Capability | Present when | Compiles |
|---|---|---|
| Reactive (`ddt`) | at least one `ddt`/charge-bearing contribution | charge `Q(V)` and its Jacobian |
| Forces | at least one `V`/`I`-source branch | branch-force stamps, inductor-flux and series-impedance companions |
| Limits | at least one `$limit` call | vold-slot update/seed/raw-voltage functions (§9) |
| Noise | at least one noise contribution | per-source PSD rows and flicker exponents |
| `ac_stim` | at least one AC-stimulus contribution | per-source magnitude/phase rows |
| Constraint margins | at least one declared pointwise `require`/`target` | one function per declared margin (never per instance — the kernel is instance-shared) |
| Coverage | at least one declared `cover` point | one function per declared point |
| `∂F/∂p` (sensitivity) | at least one differentiable parameter | one function, parameter-indexed (one compiled function regardless of parameter count — Part VII §17.2) |
| `.disto` 2nd/3rd order | opt-in AND at least one nonzero higher derivative | one function per nonzero ordered branch pair/triple (never one function unrolling every combination — a many-branch device would overwhelm Cranelift's own compilation, Part VII §17) |

**The capability-gating contract.** Presence of a capability SHALL be
represented as `Option<Capability>` (or an emptiness check on the
capability's own inner collection), never as a separately-tracked boolean
flag alongside always-allocated capability data. A "does the module have
capability X" query SHALL be exactly `self.x.is_some()` (or the equivalent
emptiness check) — this is a correctness property, not a style preference:
two independently-maintained flags for the same fact are two places a future
change can disagree.

**Uniform function signature.** Every compiled analog function — regardless
of which capability it belongs to — SHALL share one calling convention:
terminal voltages, parameter values, runtime-state values, the `SimCtx`
struct (§5.3) as inputs, and an output array. Terminal order is fixed per
kernel (ports first, then module-internal nodes); state-slot order is fixed
per kernel (assigned at resolve time, §2.3). This uniformity is what lets the
device layer (§7) call any capability's function through one dispatch shape.

### 6.2 DigitalKernel — comb/seq/watch

A module's digital body compiles to exactly three native functions, sharing
one per-instance ABI struct:

- **`comb`** — evaluates every combinational statement in source order: reads
  inputs and the live per-instance variable banks, writes outputs and the
  banks. A variable read before any assignment in the same evaluation holds
  its previous value (a latch), never an undefined read.
- **`seq`** — for each clocked block whose `fired` flag is set (computed by
  `watch`, below), runs that block's register updates. Reads SHALL see the
  *pre-edge* bank copies; writes go to the live banks (Part I's clocked-block
  semantics govern the exact same-edge read/write ordering this implements).
- **`watch`** — evaluates each atomic event term (the signal expression under
  a `posedge`/`negedge`/`change`); the device layer (§7) compares the
  evaluated value against the previous sample to derive each block's `fired`
  flag using `piperine-signal`'s edge-direction rule (§9).

**Value encoding.** Digital signals SHALL be four-state, encoded as a
64-bit integer with the fixed mapping `0 = Zero`, `1 = One`, `2 = X`,
`3 = Z` (Part VII §7's `LogicValue` — codegen's encoding and the solver's
`LogicValue` share this numbering as a stated convention, not a coincidence,
so conversions between them are a plain match, never a reinterpret). Integer
and boolean values are plain 64-bit integers; real values are 64-bit floats.
Variables live in two per-instance banks (integer, real), addressed by
compile-time-assigned slots — there is no dynamic variable lookup at
simulation time.

### 6.3 Fused digital-cone evaluation

A connected group of *purely combinational* digital instances MAY compile
into a single evaluator function that runs every member's combinational body,
in dependency-rank order, over shared arrays — rather than one solver
`Element` per instance dispatching individually. Such a fused cone SHALL
present to the solver's event scheduler (Part VII §14.3) as a single
`Element`: it consumes the cone's boundary input nets and emits events only
for driven nets whose value actually changed, exactly as an unfused
instance-per-gate circuit would, but without a per-gate dispatch or
intra-cone event round-trip for connections that are, semantically, just
wires between combinational gates.

This is an optimization of *evaluation granularity*, not a semantic change:
fusing or not fusing a given cone SHALL produce identical settled values and
identical emitted events at the cone's boundary. Register/clocked-block
fusion is explicitly out of this stage's current scope — a cone containing
any clocked block or analog sampling stays per-instance.

---

## §7 Device — kernels as solver Elements

The device stage wraps a compiled kernel, plus per-instance state, as one
`Element` (Part VII §3/§4) implementation. Per Part VII's MD-01, there is
exactly one device type presenting both the analog and digital halves as a
single flat object — never a separate analog/digital wrapper pair, never a
downcast.

### 7.1 Per-instance state

Everything a kernel's compiled functions treat as an input array (§6.1's
uniform signature) is device-owned, per instance:

- **Parameter values** — resolved once at instance construction (POM
  `Instance` overrides applied over the module's declared defaults), never
  re-resolved during a simulation (MD-18, §11.1).
- **Runtime-operator state** — the state-array slots resolve (§2.3) allocated
  per operator call site, serviced between accepted timesteps.
- **Register banks** — the digital int/real variable banks (§6.2), persisted
  across `comb`/`seq`/`watch` calls.
- **Netlist references** — which solver node/branch/net index each terminal
  and internal analog node, and each digital net, maps to.

### 7.2 Analog loading

The analog device SHALL implement Part VII §3.2's loading contract
(`load_dc`/`load_transient`/`load_ac`/noise) by calling the kernel's
compiled functions with the instance's current state and packing the results
into the Norton-companion stamp shape Part VII §8 defines. Three families of
per-instance behavior are delegated to `piperine-signal` rather than
reimplemented device-side — runtime operators, analog event detection, and
`$limit` bookkeeping — named precisely in §9; this section states only that
the device layer is where that delegated state lives and is serviced, not
what the delegated semantics are.

### 7.3 Digital evaluation

The digital device SHALL evaluate its kernel's `comb`/`seq`/`watch` functions
at the points Part VII §14.3's delta-cycle algorithm calls for, using
`piperine-signal`'s edge-direction rule (§9) to turn a `watch` sample pair
into each clocked block's `fired` flag.

### 7.4 What never delegates

Two per-instance concerns are permanently device-owned, not candidates for
delegation, because they are MNA-shaped rather than runtime-semantic:

- **Branch-unknown allocation** for `V`/`I`-source forces — this is a
  structural property of how the instance is stamped into the matrix, not a
  computable value.
- **The LTE step-size hint** (`suggest_transient_step`, Part VII §15.9) — the
  device supplies a per-charge local hint; the solver's own estimator (Part
  VII §10.3) is a separate, global computation. These are deliberately two
  different things computed two different ways, not duplication.

---

## §8 Circuit compilation — instance structure and CircuitCompiler

`CircuitCompiler` is the one stage that reads instance *structure* — which
module each instance is, what its ports connect to, what parameter overrides
it carries — directly from the POM `Design`/`Module`/`Instance` objects, at
build time, rather than from any per-module compiled artifact.

### 8.1 Why this is a separate stage

Every stage through `kernel` (§2–§6) is scoped to one module's *own*
behavior, independent of instantiation. Instance wiring is exactly the part
of a design that is **not** a property of a module's own behavior — the same
compiled `AnalogKernel` is correct for every instance of its module,
regardless of how many instances exist or how they are connected. Resolving
wiring at the resolve stage would force a wiring-dependent recompilation per
instance; reading it directly from the POM at build time instead means:

- a module compiles exactly once regardless of instance count (§1's sharing
  invariant),
- instance wiring changes (a different top-level netlist using the same
  library modules) never trigger recompilation of the modules themselves.

### 8.2 The build walk

`CircuitCompiler` SHALL, for a given top module:

1. compile (or retrieve from cache) each instantiated module's
   `CompiledModule` — the `Arc`-shared analog/digital kernel pair;
2. for each instance in the top module's instance list, read its POM
   connections and parameter overrides directly;
3. wrap each instance as one `PiperineDevice` (§7), sharing the module's
   compiled kernel and owning the instance's own per-instance state;
4. accumulate every device, plus the analog netlist and digital net map,
   into a `CircuitInstance` ready for the solver (Part VII §2).

**Plugin devices.** An instance annotated `@device` (Part VI's plugin
extensibility model) SHALL be delegated to a registered device provider
instead of being compiled from a Verilog-AMS body — the provider returns an
`Element` directly, and the solver SHALL NOT be able to distinguish a
plugin-provided device from a Verilog-AMS-compiled one (Part VI §7's stated
invariant). This crate SHALL NOT depend on the plugin-loading crate to
implement this: the provider contract is a trait defined here, implemented
by the plugin host, preserving the dependency direction plugins→codegen never
codegen→plugins.

---

## §9 The piperine-signal delegation boundary

Five families of **runtime semantics** — computations over already-elaborated
values, independent of Cranelift, independent of the solver's matrix — are
owned by `piperine-signal`, a pure crate with no in-tree dependencies, rather
than by this crate. Codegen calls into them; it holds no second
implementation of any of the five. Their normative semantics (exact formulas,
edge-case rules, the IEEE 1364 edge convention, etc.) are specified in **Part
IX**, not repeated here — this section states only what crosses the boundary
and how.

| Family | What crosses the boundary | How it crosses |
|---|---|---|
| Runtime operators (`delay`/`slew`/`transition`/`idt`/`idtmod`) | Per-call-site value/derivative computation | The device layer (§7) constructs and advances `piperine-signal`'s operator type directly; codegen's own state-array slot (§2.3) wraps it, since the pure type deliberately carries no slot index |
| Event detectors (`cross`/`above`/`@timer`'s fire decision) | The fire decision itself | The device layer maps its compiled trigger representation to `piperine-signal`'s trigger type and asks it whether a fire occurred; `@initial` dispatch and the timer period table remain codegen's — they are not detector logic |
| The junction limiter (`pnjlim`/`fetlim`/`limvds`) | Both the formula **and** the seed-preservation/convergence-veto bookkeeping | The formula is linked as an external native symbol (§5.3) and called from **compiled JIT code** — this is the one family where the call crosses into emitted machine code, not just device-layer Rust; the bookkeeping calls a `piperine-signal` method that accepts the JIT-computed limited value and an externally-owned state slice, so the formula is never computed twice |
| `table` | Validation (strictly increasing breakpoints, matching-length arrays, supported interpolation mode) and segment geometry | The resolve stage (§2) constructs a validated table value from `piperine-signal` at lowering time and reads its segment data to emit the equivalent piecewise expression — the *validated data* crosses the boundary, not a runtime call, because `table`'s value must remain a differentiable compiled expression (§3), not an opaque function call |
| Digital edge detection (IEEE 1364 rising/falling/either) | The edge-fire predicate | Both the compiled digital kernel's `watch`-derived comparison (§6.2, §7.3) and the host sequencer's wait predicate (Part VIII) call the same predicate — this is the one family whose two call sites span two different crates (`piperine-codegen` and `piperine-api`), unified so a host tester's edge wait cannot silently disagree with what a compiled Verilog-AMS clocked block would observe |

**What stays in codegen, and why.** Compile-time machinery — symbolic
differentiation (§3), Cranelift emission and CSE (§5), the `SimCtx` ABI
(§5.3), and POM→resolved lowering (§2) — is not a candidate for this
delegation. `piperine-signal` computes values, derivatives, event
predicates, and running reductions; it never stamps a matrix, never sees an
`Element`, and depends on nothing else in the tree (Part IX §1's purity
invariant). A pass that needs a JIT builder or a matrix stays here, by
construction.

**The guard.** The absence of a second copy of any of the five families is
mechanically checked, not merely documented: a dedicated test walks this
crate's (and, for the digital-edge family, `piperine-api`'s) own sources for
each family's forbidden identifiers and fails naming the offending location
if one reappears. A future change that reintroduces a local copy — of a
formula, a bookkeeping struct, or an inline edge comparison — SHALL be
caught by this guard, not discovered by drift.

---

## §10 Public surface and visibility

This crate presents **one** deliverable, not a two-tier host/plugin split
(contrast Part VII's solver, which exposes a `prelude`/`abi` split for two
different audiences). Its visibility rule:

- `resolve` and `kernel` and `device` SHALL be `pub` — hosts and tests
  address them by deep path (e.g. a test constructing a `LoweredBody`
  directly, or a host inspecting `kernel::digital::network`'s fused-cone
  type).
- `emit`, `flatten`, and the crate's error module SHALL be crate-private —
  nothing outside this crate has a legitimate reason to construct a
  `Builder` or a `FlatAnalog` directly; any item from these modules that a
  host genuinely needs SHALL be re-exported through the crate's single
  `lib.rs` façade instead of widening the module's own visibility.

A module SHALL earn `pub` visibility only by an evidenced external
dependency on its deep path — not by default, and not because a sibling
module happens to be `pub`.

---

## §11 Error handling and construct coverage — the fail-loud contract

### 11.1 One named error type

Every failure this pipeline can produce — an unimplemented lowering, a
Cranelift validation failure, a module-build error, a failed constant
evaluation, a failed function-inlining — SHALL surface as one of a small,
named set of error variants. There SHALL be no code path in which a
construct that cannot be faithfully compiled instead produces a silently
substituted value (a `0.0` residual, a no-op stamp, an ignored contribution).

### 11.2 The coverage guarantee

"100% coverage" is a specific, checkable claim about this pipeline: **every
Verilog-AMS construct that reaches codegen either lowers to executable device code,
or fails loud with a named, construct-identifying error** at the point it is
first found unsupported. This SHALL hold uniformly across every emission
site a construct might be reached through — the primary residual path, a
compiled parameter-Jacobian pass, and an inlined user-function body (§4.1)
SHALL all apply the same coverage rule to the same construct; a construct
handled on the primary path but silently dropped inside an inlined `fn` body
would violate this guarantee just as surely as never handling it at all.

*Rationale.* A silent zero is worse than a compile failure: it produces a
circuit that appears to simulate successfully while quietly omitting a
device's declared behavior. This pipeline treats "I don't know how to
compile this" and "I compiled this wrong" as the same severity of bug, and
refuses to let the first degrade into the second.

### 11.3 Elaboration fixes devices; simulation never recompiles

Once a `CircuitInstance` is built (§8), its devices' compiled kernels are
fixed for the remainder of that build — a parameter sweep, a live-session
parameter update, or any other in-simulation change SHALL be serviced by
restamping the already-compiled circuit (Part VII §10.5's live-parameter
surface), never by re-invoking any stage of this pipeline. Re-running
resolve/flatten/emit/kernel inside a simulation loop is a pipeline-boundary
violation, not a performance tuning question — this is the same invariant
Part VII §10.5 states from the solver-facing side.
