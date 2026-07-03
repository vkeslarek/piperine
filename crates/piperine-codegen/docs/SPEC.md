# Piperine IR — Specification

The Piperine IR is the post-elaboration, resolved intermediate representation that both
frontends lower into and the codegen consumes:

```
Verilog-A/AMS ─┐
               ├─▶ IrProgram ─▶ codegen (JIT / interpret) ─▶ piperine-solver
PHDL          ─┘
```

It is a **device-and-behavior codegen target**, not a simulation kernel model. It carries what a
compact-model / mixed-signal *device* needs — contributions, stamps, state operators, a clean
digital next-state model — and nothing from the SystemVerilog RTL/testbench world, which lives
in the `bench` layer, not here.

This document supersedes the previous `ir.rs` and `IR-SYSTEM.md`. §13 lists every construct
removed from that IR and why.

---

## 1. Goals

- **Post-elaboration and resolved.** The IR is emitted after elaboration. Generics, bundles,
  lambdas, higher-order functions, structural `for`/`if`, and const folding are already gone.
  Every module is monomorphic and flat. Names are resolved to interned ids (§3); the IR does not
  carry unresolved strings in hot positions.
- **Minimal and exhaustive.** Small enough for the codegen to handle *every* variant — no
  variant may lower to a silent `Real(0.0)` fallback. If the IR can express it, the codegen
  implements it; if the codegen cannot yet, the emitter must not produce it (fail-loud, §11).
- **Two engines, cleanly separated.** An analog body is a contribution/force stamp list; a
  digital body is combinational logic plus clocked registers (the PHDL model), not Verilog
  procedural timing. Nothing bridges the two implicitly (No-Magic carries through to the IR).
- **Frontend-neutral.** Anything either frontend can express, the IR represents — but by
  *lowering the frontend's construct into the IR's vocabulary*, never by widening the IR to
  mirror a frontend wart. Verilog-AMS's richer analog constructs lower in; its RTL/testbench
  constructs are rejected at ingestion (§13).

---

## 2. Position in the pipeline

The IR sits between elaboration and codegen. What each phase owns:

| Phase | Owns |
|-------|------|
| Elaboration (frontend) | generics, bundles, capabilities, HOF/lambda, structural control, const eval, monomorphization, bundle-field flattening, discipline resolution |
| **IR** | resolved netlist, contributions/forces, analog state operators, noise, analog/digital events, the digital next-state model, sim queries, functions |
| Codegen | JIT/interpret residual+Jacobian, reactive stamping, digital evaluator, mixed-signal bridges, `Device` synthesis |

Consequences: the IR has no `Lambda`, `BundleLit`, generic parameter, or structural `for`/`if` —
those are elaboration-only. It has no fork/join, `#delay`, or `wait` — those are RTL kernel
constructs the codegen does not model.

---

## 3. Resolution model (interned ids)

The previous IR referenced nodes, params, vars, and state by `String` in every expression. The
IR is post-elaboration, so all names are known; it interns them.

Per `IrModule`, an arena assigns dense ids and a side table maps id → source name (for display
and diagnostics only):

```
NodeId(u32)    // a resolved net / terminal (ground is NodeId(0))
ParamId(u32)   // a resolved parameter slot
VarId(u32)     // a resolved runtime variable slot
StateId(u32)   // an analog-operator state slot
FnId(u32)      // a resolved function
NatureId(u32)  // a discipline nature (its access name + potential/flow kind)

struct SymbolTable {
    nodes:   Vec<NodeInfo>,     // name, discipline
    params:  Vec<ParamInfo>,    // name, type, default
    vars:    Vec<VarInfo>,      // name, type
    states:  Vec<IrStateVar>,   // §7
    natures: Vec<NatureInfo>,   // access name, Potential | Flow
    fns:     Vec<IrFunction>,
}
```

Expressions carry ids, not strings. Codegen indexes arrays directly; display resolves through
the table. Ground is a reserved `NodeId(0)`; the single-argument access `V(a)` resolves to
`Branch { nature, plus: a, minus: NodeId(0) }`.

---

## 4. Types

```
IrType = Real | Integer | Bool | Quad
```

Everything in analog evaluation is `Real`. `Integer`/`Bool` distinguish storage and control
flow; `Quad` is 4-state digital (0/1/X/Z). Removed from the prior IR: `Complex` (a library
bundle, expanded to two `Real`s at elaboration — the IR never sees it), `String` (diagnostics
carry a literal format `String` on the `Diagnostic` node, not a value type), and `Void`
(a function without a return is simply `returns: None`).

---

## 5. Expressions

```
IrExpr =
  // literals
  | Real(f64) | Int(i64) | Bool(bool) | Quad(u8)          // Quad: 0=0 1=1 2=X 3=Z
  // resolved references
  | Param(ParamId)
  | Var(VarId)
  | Branch { nature: NatureId, plus: NodeId, minus: NodeId }   // V(p,n), I(p,n), Pwr(p,n)…
  | Net(NodeId)                                                 // digital net read (quad)
  | State(StateId)                                              // an analog-operator result
  // queries and stimulus
  | Sim(SimQuery)
  | AcStim { mag: Box<IrExpr>, phase: Box<IrExpr> }             // .ac only
  // computation
  | MathCall(String, Vec<IrExpr>)                               // built-in math (libm intrinsic)
  | Call(FnId, Vec<IrExpr>)                                     // user function
  | Binary(IrBinOp, Box<IrExpr>, Box<IrExpr>)
  | Unary(IrUnOp, Box<IrExpr>)
  | Select(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>)              // cond ? a : b
  // vectors (buses)
  | Array(Vec<IrExpr>)
  | Index(Box<IrExpr>, Box<IrExpr>)                            // a[i]
  | Slice(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>, bool)         // a[lo..hi], inclusive flag
```

Built-in math (`exp ln sqrt pow tanh …`) is `MathCall`, resolved by name to a libm/JIT
intrinsic; user functions are `Call(FnId, …)`. Analog operators never appear as calls —
they are `State` (§7). `Net` is the digital-side dual of `Branch`: a 4-state net read,
valid only in digital bodies. Node references are always resolved `NodeId`s.

```
IrBinOp  = Add Sub Mul Div Rem Pow
         | Eq Ne Lt Le Gt Ge
         | And Or                       // logical (short-circuit)
         | BitAnd BitOr BitXor
         | Shl Shr                       // logical shifts

IrUnOp   = Neg | Not | BitNot
         | RedAnd RedOr RedXor           // bus reductions (digital)

SimQuery = Temperature | Vt(Option<Box<IrExpr>>) | Abstime | Mfactor
         | Position(Axis) | Angle
         | Simparam { key: String, default: Box<IrExpr> }
         | Analysis(Analysis)            // resolved enum: Dc Ac Tran Noise
         | ParamGiven(ParamId) | PortConnected(NodeId)
         | Limit  { kind: String, args: Vec<IrExpr> }
         | Random { kind: String, args: Vec<IrExpr> }
```

`Analysis` is a resolved enum, not a string (matches the language `$analysis` returning an
`Analysis` enum). Arithmetic shifts, mintypmax, part-selects, concat, replicate, port-flow,
bundle literals, and lambdas are gone (§13).

---

## 6. Analog behavior

```
IrAnalogBody {
    states: Vec<StateId>,           // operator slots referenced by this body
    noise:  Vec<IrNoiseSource>,
    stmts:  Vec<IrStmt>,            // analog statement subset (§8)
}
```

Two contribution statements, plus structured control:

```
Contrib { nature: NatureId, plus: NodeId, minus: NodeId, expr: IrExpr, kind: ContribKind }
Force   { nature: NatureId, plus: NodeId, minus: NodeId, expr: IrExpr }

ContribKind = Resistive | Reactive(StateId)
```

- `Contrib` is `<+` (accumulates on the branch). `kind` is `Reactive(s)` iff `expr` contains a
  reactive `State` (`ddt`/`idt`/`laplace`/`zi`), else `Resistive`; classification is a structural
  property computed at emit time, not a guess.
- `Force` is `<-` (single-driver ideal source / short); the elaborator has already enforced the
  single-driver rule, so codegen stamps it as a voltage/flow source unconditionally.

Each maps to a solver stamp: a flow contribution is an injected current, a potential is a
voltage-defined branch (internal branch-current unknown). Reactive contributions are stamped
with `alpha = 1/dt` (or the integration companion coefficient).

Analog control flow is `If` and `Match` (§8); loops are already unrolled by elaboration (the IR
has no analog loop). Indirect branch assignment is **not** representable (§13).

### 6.1 Analog events

```
IrAnalogEvent { source: EventSource, body: Vec<IrStmt> }

EventSource = InitialStep | FinalStep
            | Cross { expr: IrExpr, dir: CrossDir }   // dir: Either | Rising | Falling
            | Above { expr: IrExpr }
            | Timer { period: IrExpr }
```

An `@ initial` initial condition, a threshold `cross`/`above`, or a periodic `timer`. A guarded
PHDL event (`@ cross(...) when (g) { ... }`) lowers to an event whose body is a single `If { g }`.
This is the **one** event representation; the prior IR's separate `IrEventSpec` (digital),
`IrEventKind` (analog), and `IrStateKind::Cross/Timer` are unified here and in §9.

Runtime semantics (codegen): an event body is a list of persistent-variable updates
(`if`/`match` fold into `Select` on each value; diagnostics are collected; anything else fails
loud). `initial` actions run once at instance creation (zero volts, parameters visible);
`cross`/`above`/`timer` trigger expressions are evaluated at each *accepted* solution and the
device detects the transition — a fired event writes its actions into the vars bank, visible
from the next step (the ngspice `sw` hysteresis idiom). `final` admits diagnostics only (there
is no end-of-run device hook yet).

### 6.2 Noise

```
IrNoiseSource { plus: NodeId, minus: NodeId, kind: IrNoise, label: Option<String> }
IrNoise = White { psd: IrExpr } | Flicker { psd: IrExpr, exponent: IrExpr }
```

Extracted from contribution expressions at emit time; the `white_noise`/`flicker_noise` call
contributes to `noise`, and its expression position is `Real(0.0)`.

---

## 7. Analog state operators

Operators with internal state are slots, referenced by `IrExpr::State(id)`:

```
IrStateVar { id: StateId, kind: IrStateKind, arg: IrExpr }

IrStateKind =
  | Ddt                                        // reactive
  | Idt    { ic: IrExpr }                      // reactive
  | IdtMod { ic: IrExpr, modulus: IrExpr }     // reactive
  | Ddx    { node: NodeId }                     // compile-time derivative
  | Delay  { delay: IrExpr }                    // resistive (ring buffer)
  | Transition { delay: IrExpr, rise: IrExpr, fall: IrExpr, tol: IrExpr }
  | Slew   { rise: IrExpr, fall: IrExpr }
  | Table  { data: TableRef, mode: InterpMode }         // measured-data lookup (new)
  | Laplace    { variant: LaplaceKind, num: Vec<IrExpr>, den: Vec<IrExpr> }
  | ZTransform { variant: ZKind, num: Vec<IrExpr>, den: Vec<IrExpr>, sample_dt: IrExpr }
```

`arg` is the operator input; the codegen evaluates it each Newton iteration and applies the
operator. Reactivity (for `ContribKind`) is a property of the kind: `Ddt`/`Idt`/`IdtMod`/
`Laplace`/`ZTransform` are reactive; `Delay`/`Transition`/`Slew`/`Table`/`Ddx` are resistive.
`Table` is added for measured-data devices; `variant` fields are resolved enums, not strings.
Current lowering status: `Ddt` (charge/companion model), `Idt`/`IdtMod` (implicit-Euler
runtime integrator: the kernel reads `state + dt·x`, the device accumulates — and wraps, for
`idtmod` — each accepted step; at DC the value is the initial condition; the AC small-signal
`1/jω` admittance is not yet stamped), `Ddx` (symbolic), and `Delay`/`Slew` (runtime-serviced
state) are implemented. `Transition`/`Table`/`Laplace`/`ZTransform` are recognised and fail
loud at codegen (named errors) — each is its own follow-up.
`Cross`/`Timer` are **not** state kinds here — they are event sources (§6.1); detector state, if
any, is the codegen's concern.

---

## 8. Statements

One statement set; each body admits a subset, enforced at emit (§11).

```
IrStmt =
  // analog only
  | Contrib { … } | Force { … }                 // §6
  | AnalogEvent(IrAnalogEvent)                   // §6.1
  // digital, or analog-sequential (see below)
  | Assign { lval: Lval, expr: IrExpr }          // combinational or register (context, §9)
  | ClockedBlock { event: DigitalEvent, body: Vec<IrStmt> }   // §9
  // shared control
  | If    { cond: IrExpr, then_: Vec<IrStmt>, else_: Vec<IrStmt> }
  | Match { scrutinee: IrExpr, arms: Vec<(Pattern, Vec<IrStmt>)>, default: Vec<IrStmt> }
  | VarDecl { var: VarId, init: Option<IrExpr> }
  | Return(Option<IrExpr>)                        // function bodies
  // simulator control (shared)
  | BoundStep(IrExpr) | Finish | Discontinuity(u8)
  | Diagnostic { severity: Severity, format: String, args: Vec<IrExpr> }

Lval    = Var(VarId) | Net(NodeId) | Index(Box<Lval>, IrExpr) | Slice(Box<Lval>, …)
Pattern = Value(IrExpr) | BitPattern(Vec<Trit>) | Wildcard   // Trit: 0 | 1 | DontCare
Severity = Info | Warn | Error | Fatal
```

In an **analog** body, `Assign { lval: Var(v), … }` is a *sequential variable binding*
(`x = …; I(p,n) <+ x;`), the idiom every Verilog-A compact model uses. The codegen resolves
these symbolically: assignments substitute forward in source order, and an assignment under
`if`/`match` merges as `Select(guard, new, old)`. Net assignment stays digital-only.

`Match` replaces the prior `Case`/`CaseX`/`CaseZ` trio; don't-care is a `BitPattern` trit
(`?`), distinct from the `Quad` value X. There is no `For`/`While`/`Repeat`/`Forever` in the IR:
analog loops are unrolled at elaboration, digital loops are unrolled or expressed as clocked
iteration. `Diagnostic` format strings interpolate `{}` from `args`.

---

## 9. Digital behavior

The digital body is the PHDL model — combinational logic with inferred memory, plus clocked
registers — **not** the Verilog procedural kernel.

```
IrDigitalBody {
    inputs:  Vec<NodeId>,
    outputs: Vec<NodeId>,
    regs:    Vec<VarId>,          // state held across timesteps
    stmts:   Vec<IrStmt>,         // combinational + ClockedBlock
}

DigitalEvent = Posedge(IrExpr) | Negedge(IrExpr) | Change(IrExpr)
             | Or(Vec<DigitalEvent>)
```

Semantics carried by structure, not by assignment operator:

- An `Assign` **outside** a `ClockedBlock` is combinational (dataflow, read-after-write in
  order). A `VarId` read on a path where it was not assigned infers a **latch** (the emitter
  flags it; §11).
- An `Assign` **inside** a `ClockedBlock` is a **register** update: within the block, reads see
  the pre-edge value; a chain is a pipeline. Overlapping writes: last in source order wins.

There is no `NonBlocking` vs `Assign` distinction, no inline `#delay`/`@event` on assignments,
no `ContinuousAssign`/`ProcAssign`/`Deassign` — register-ness is positional (inside a clocked
block), matching the language. `initial`/`final` on the digital side are `ClockedBlock`s with an
`InitialStep`/`FinalStep`-equivalent source when needed; combinational reset is ordinary logic.

---

## 10. Module and program

```
IrModule {
    name: String,
    symbols: SymbolTable,          // §3: nodes, params, vars, states, natures, fns
    ports: Vec<IrPort>,            // (NodeId, direction)
    instances: Vec<IrInstance>,    // resolved children
    analog:  Option<IrAnalogBody>,
    digital: Option<IrDigitalBody>,
}

IrPort     { node: NodeId, direction: In | Out | Inout }   // domain lives on NodeInfo
IrInstance { label: String, module: String,
             connections: Vec<NodeId>,   // parent node per child port, positional
             params: Vec<(ParamId, IrExpr)> }              // child ParamId, parent-scope expr
IrFunction { name: String, params: Vec<VarId>, returns: Option<IrType>, body: Vec<IrStmt> }

IrProgram  { source: Source /* Ams | Ppr */, modules: Vec<IrModule> }
```

Removed module fields (§13): `branches` (resolved to node pairs at emit — `V(br)` becomes
`Branch{plus,minus}`), `events` (named-event decls — Verilog-only), `grounds` (a node attribute,
`NodeId(0)` is the reference), `connections`/`continuous_assigns` (net aliasing is resolved into
node identity by elaboration; a structural continuous assign is a combinational `Assign` in the
digital body). Hierarchy is `instances`; the top module is elaborated flat for the device path.

---

## 11. Emit and validation contract

The emitter (frontend → IR) must produce only what the codegen implements. There is no silent
fallback; a construct the codegen cannot lower must be rejected at emit with a diagnostic. Rules:

- Analog bodies contain only `Contrib`/`Force`/`AnalogEvent`/`If`/`Match`/`VarDecl`/`Diagnostic`/
  `BoundStep`/`Discontinuity`. A `<+`/`<-` outside an analog body, or a digital `Assign`/
  `ClockedBlock` inside one, is an error.
- Digital bodies contain only `Assign`/`ClockedBlock`/`If`/`Match`/`VarDecl`/`Diagnostic`. A
  digital-edge event in an analog body (or analog crossing in a clocked digital block) is an
  error.
- Every `Branch` nature/node, `Param`, `Var`, `State`, `Fn` id resolves in the module's
  `SymbolTable`.
- `ContribKind` matches the presence of a reactive `State` in the expression (checked, not
  assumed).
- `$bound_step`/`$discontinuity` are analog-only; `Finish` fails loud at codegen in both
  bodies (no runtime hook yet).
- Inferred digital latches emit a warning (a combinational variable read on a path where it
  was not definitely assigned); registers are silent.

`first_reactive_state(expr) -> Option<StateId>` classifies contributions; unlike the prior
string-walking `first_state_ref`, it walks resolved `State(id)` nodes.

---

## 12. Codegen contract

The codegen owns the full IR → solver bridge: it compiles kernels (`jit`), wraps them in
`Device` implementations (`device::PiperineDevice`), and builds circuits from a structural
top module (`device::CircuitCompiler`). Frontends emit IR and stop.

Per module:

- **Analog** (`AnalogKernel`): flatten the body (sequential vars substitute forward, path
  conditions fold into `Select`, user calls inline, `ddx` expands symbolically), then JIT
  residual, Jacobian (symbolic differentiation), charge `Q(V)` + charge Jacobian (companion
  model for `ddt`), force rows + force Jacobian (ideal potential sources get MNA
  branch-current unknowns), noise PSDs, runtime-state inputs (`delay`/`slew`/`idt`/`idtmod`,
  serviced by the device each accepted step), event trigger/action rows (§6.1), and
  `$bound_step`. Everything either lowers faithfully or is a
  named `CodegenError::Unsupported` — no `Real(0.0)` fallback. `Diagnostic` statements are
  collected as kernel metadata for tooling, not executed.
- **Digital** (`DigitalKernel`): compiled, not interpreted. Each digital body JITs to three
  native functions — `comb` (combinational, source order, latches hold through the variable
  banks), `seq` (register updates for fired clocked blocks, reading pre-edge bank copies),
  and `watch` (event-term values; the device derives edges by comparing against the previous
  values). Kernels are per *module* and shared across instances; per-instance state
  (register banks, previous watch values) lives in the device. The circuit-level boundary
  stays the solver's event queue — devices exchange events, each device's evaluation is
  native code.
- **Mixed-signal**: a boundary device with both bodies bridges explicitly — A2D thresholds an
  analog potential to a digital value in the digital evaluator; D2A stamps a source from digital
  state in the analog loader. No implicit crossing (No-Magic).

---

## 13. Discarded from the previous IR (with rationale)

Removed because they are rejected by the language, dead (unimplemented, fell to `Real(0.0)`), or
redundant. This is the diff from `ir.rs`/`IR-SYSTEM.md`.

**Rejected by the language spec:**
- `IrStmt::IndirectContrib` — indirect branch assignment `V(x): I==expr` (singular systems;
  language §14). Devices use the finite-parameter idiom.
- `IrStmt::Delay`, inline `delay`/`event` on `Assign`/`NonBlocking` — digital `#delay` (language
  rejects delay-based RTL timing; timing is analog `transition`/`delay` or the `bench` layer).

**SystemVerilog RTL / testbench (belong to `bench`, not a device IR):**
- `IrStmt::Fork`/`JoinKind`, `Wait`, `Disable`, `Trigger`, `IrEventDecl`, named events,
  `IrEventSpec::Named`/wildcard.
- `IrStmt::ProcAssign`/`ProcDeassign` (force/release/deassign), `ContinuousAssign` as a distinct
  procedural form.
- `IrStmt::While`/`Repeat`/`Forever` — no unbounded runtime loops in a device body; analog is
  unrolled, digital is combinational + clocked.

**Redundant / folded away:**
- `NonBlocking` vs `Assign` — register-ness is positional (inside `ClockedBlock`), not an
  operator (§9).
- `IrEventSpec` (digital) + `IrEventKind` (analog) + `IrStateKind::Cross/Timer` — unified into
  `EventSource` (analog, §6.1) and `DigitalEvent` (§9).
- `CaseX`/`CaseZ` → one `Match` with `BitPattern` trits.
- `Concat`, `Replicate` → `Array` / library `concat` (a function, not an IR node).
- `PartSelect`, `PartSelectIndexed` → `Slice`/`Index`.
- `PortFlow` → `Branch`.
- `Mintypmax` → the typical value (Verilog spec minutiae; no device use).
- `Shl`/`Shr` arithmetic variants (`AShl`/`AShr`) → the two logical shifts suffice for the
  language.
- `RedNand`/`RedNor`/`RedXnor` → `Not(RedAnd|RedOr|RedXor)`.

**Elaboration-only (never reaches the IR):**
- `IrExpr::BundleLit`, `IrExpr::Lambda` — bundles are field-flattened and lambdas/HOF are
  inlined/monomorphized at elaboration.
- `IrType::Complex` (library bundle → two `Real`s), `IrType::Void`, `IrType::String` as a value
  type.

**Structural, resolved by elaboration:**
- `IrModule::branches` (→ node pairs), `events`, `grounds` (→ `NodeId(0)`),
  `connections`/`continuous_assigns` (→ node identity / combinational `Assign`).

**Stringly-typed → interned (§3):** every `plus`/`minus`/`access`/`Param(String)`/`Var(String)`/
node name in an expression is now a resolved id, with a per-module symbol table for display.

Functions live only in `SymbolTable.fns` (no separate module/program lists); the emitter
copies any file-level function into each module that calls it, so `FnId` is always
module-local. A port's domain (analog vs digital) lives on its node's `NodeInfo`, not on
the port.

---

## 14. Expressiveness validation

- **Verilog-AMS analog**: contributions, natures/branches, all analog operators (incl. `table`
  now), events (`cross`/`above`/`timer`/`initial`/`final`), noise, sim functions, analog
  functions, `if`/`case`, bounded `for` — all representable. Indirect branch assignment is the
  only analog feature intentionally dropped (language decision).
- **Verilog-AMS / SystemVerilog digital**: `fork`/`join`, `#delay`, `wait`, named events are
  intentionally out — they are RTL/testbench simulation constructs, not compact-model behavior,
  and Piperine's digital is the combinational-plus-register model.
- **PHDL**: analog and digital behavior, functions; generics/bundles/capabilities/HOF/generation
  are elaborated away before the IR. Fully covered.

The trimmed IR covers everything the language can express and everything Verilog-AMS *device*
modeling needs, while dropping the constructs that were aspirational, redundant, or rejected.