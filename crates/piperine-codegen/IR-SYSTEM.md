# Piperine Codegen IR — System Document

> **Status**: Reference document for the `piperine-codegen` Intermediate Representation.
> **Location**: `crates/piperine-codegen/src/ir.rs`
> **Scope**: Complete specification of all IR types, their semantics, and the lowering contract from both frontends (`piperine-ams` / Verilog-AMS and `piperine-lang` / PHDL).

## 1. Overview

The IR is the shared intermediate representation into which **both** Piperine frontends lower:

```
 Verilog-A/AMS (.va, .vams) ──┐
                               ├──▶ piperine-codegen::IrProgram ──▶ [codegen] ──▶ piperine-solver
 PHDL (.phdl)              ──┘
```

### Design principles

1. **Superset expressiveness**: the IR can represent anything either frontend can express. No construct is silently dropped — if a frontend emits it, the IR carries it.
2. **Simple**: the IR is intentionally low-cardinality. It has ~30 expression variants, ~25 statement variants, and ~11 state-variable kinds. This is small enough for a codegen to handle exhaustively.
3. **Untyped-by-default**: in analog context everything evaluates to `f64`. Type information is retained only where the codegen needs it (string params, integer vs real, 4-state logic).
4. **Opaque function calls**: `IrExpr::Call(name, args)` covers built-in math functions (`exp`, `ln`, ...), analog operators that are not state-allocated (`limexp`, `analysis`), and user-defined functions/tasks. The codegen resolves these at compile time. User function bodies live in `IrFunction` tables at module and program level.
5. **Analog + digital**: the IR models both continuous-time (analog) and event-driven (digital) behavior. A module may have both an `IrAnalogBody` and an `IrDigitalBody`.

### Source tracking

`IrProgram::source` is `"ams"` or `"ppr"`, set by the lowering entry points `ams_to_ir()` and `ppr_to_ir()` respectively. This is for diagnostics only — the IR structure is identical regardless of source.

---

## 2. Program structure

### `IrProgram`

```rust
pub struct IrProgram {
    pub source: String,              // "ams" or "ppr"
    pub modules: Vec<IrModule>,
    pub functions: Vec<IrFunction>,  // global (file-level) functions/tasks
}
```

The top-level container. `modules` is a flat list (hierarchy is expressed via `IrInstance`). `functions` holds functions declared at file scope (PHDL `fn` items); AMS has no file-level functions.

### `IrModule`

```rust
pub struct IrModule {
    pub name: String,
    pub ports: Vec<IrPort>,
    pub params: Vec<IrParam>,
    pub wires: Vec<IrWire>,
    pub branches: Vec<IrBranch>,
    pub events: Vec<IrEventDecl>,
    pub vars: Vec<IrVarDecl>,
    pub grounds: Vec<IrGroundDecl>,
    pub instances: Vec<IrInstance>,
    pub connections: Vec<IrConnectionDecl>,
    pub continuous_assigns: Vec<IrStmt>,
    pub analog: Option<IrAnalogBody>,
    pub digital: Option<IrDigitalBody>,
    pub functions: Vec<IrFunction>,
}
```

| Field | AMS source | PHDL source |
|-------|-----------|-------------|
| `ports` | `module m(p, n)` port list | `mod M(inout p: Electrical)` |
| `params` | `parameter real x = 1.0` | `param x: Real = 1.0` |
| `wires` | `electrical p, n;` net decls | `wire p: Electrical;` |
| `branches` | `branch (p, n) br;` | — (not in PHDL) |
| `events` | `event e;` | — (not in PHDL) |
| `vars` | `real x; integer i;` module-level | — (PHDL vars are block-local) |
| `grounds` | `ground gnd;` | — (PHDL uses `gnd` convention) |
| `instances` | `sub u1(.p(a), .n(b));` | `u1: Sub(a, b) { .r = 1k };` |
| `connections` | — (not in AMS) | `lhs = rhs;` net aliasing |
| `continuous_assigns` | `assign x = expr;` | — (not in PHDL) |
| `analog` | `analog begin ... end` | `analog M { ... }` |
| `digital` | `initial` / `always` (digital) | `digital M { ... }` |
| `functions` | `function ... endfunction` / `task ... endtask` | `fn f(...) -> T { ... }` (module-local) |

### `IrPort`

```rust
pub struct IrPort {
    pub name: String,
    pub direction: IrDirection,   // In, Out, Inout
    pub discipline: Option<String>, // "Electrical", "Logic", etc.
}
```

### `IrParam`

```rust
pub struct IrParam {
    pub name: String,
    pub ty: IrType,                // Real, Integer, String, Bool, Quad, Complex, Void
    pub default: Option<IrExpr>,
}
```

### `IrWire`

```rust
pub struct IrWire {
    pub name: String,
    pub discipline: Option<String>,
}
```

### `IrBranch`

```rust
pub struct IrBranch {
    pub name: String,
    pub plus: String,
    pub minus: String,
}
```

A named branch from `branch (p, n) br1;`. Allows `V(br1)` / `I(br1)` to resolve to `BranchV(br.plus, br.minus)`.

### `IrEventDecl`

```rust
pub struct IrEventDecl { pub name: String }
```

Declared with `event e;` in AMS. Used by `->e` triggers and `@(e)` event controls.

### `IrVarDecl`

```rust
pub struct IrVarDecl {
    pub name: String,
    pub ty: IrType,
    pub init: Option<IrExpr>,
}
```

Module-level variables (AMS `real x;`) or block-local declarations. In analog bodies, local vars with known values are inlined into expressions by the lowering; `IrVarDecl` is retained for the codegen when the var is mutated across control-flow boundaries.

### `IrGroundDecl`

```rust
pub struct IrGroundDecl {
    pub name: String,
    pub discipline: Option<String>,
}
```

### `IrConnectionDecl`

```rust
pub struct IrConnectionDecl {
    pub lhs: String,
    pub rhs: String,
}
```

Net aliasing from PHDL `lhs = rhs;`. Both sides are concrete net names post-elaboration.

### `IrInstance`

```rust
pub struct IrInstance {
    pub label: String,
    pub module: String,
    pub connections: Vec<IrConnection>,
    pub params: Vec<(String, IrExpr)>,
}
```

### `IrConnection`

```rust
pub struct IrConnection {
    pub port: Option<String>,  // Some for named (.port(net)), None for positional
    pub net: String,
}
```

---

## 3. Types

```rust
pub enum IrType {
    Real,     // f64
    Integer,  // i64
    String,   // parameter strings
    Bool,     // boolean values
    Quad,     // 4-state logic: 0, 1, X, Z
    Complex,  // complex number (AC analysis)
    Void,     // tasks / void functions
}
```

The IR is mostly untyped — in analog evaluation everything is `f64`. Types are retained for:
- Parameter declarations (to distinguish string params from numeric)
- Variable declarations (for the codegen to allocate correct storage)
- 4-state logic in digital context

---

## 4. Expressions

```rust
pub enum IrExpr {
    // ── Literals ──
    Real(f64),
    Int(i64),
    String(String),
    Bool(bool),
    Quad(u8),              // 0=0, 1=1, 2=X, 3=Z
    Param(String),         // compile-time parameter
    Var(String),           // runtime variable (local var, function arg)

    // ── Branch access ──
    BranchAccess { access: String, plus: String, minus: String },
    // V(plus, minus), I(plus, minus), Pwr(plus, minus), Temp(plus, minus), ...
    // `access` is the nature's access function name ("V", "I", "Pwr", ...).
    // Single-arg form V(a) becomes BranchAccess { access: "V", plus: "a", minus: "0" }.

    // ── State ──
    StateRef(u32),            // reference to ddt/idt/transition/... slot

    // ── Simulator ──
    Sim(SimQuery),
    AcStim { mag: Box<IrExpr>, phase: Box<IrExpr> },

    // ── Calls ──
    Call(String, Vec<IrExpr>),  // exp, ln, user functions, ...

    // ── Operators ──
    Binary(IrBinOp, Box<IrExpr>, Box<IrExpr>),
    Unary(IrUnOp, Box<IrExpr>),
    Select(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),  // cond ? then : else

    // ── Vectors / arrays ──
    Concat(Vec<IrExpr>),           // {a, b, c}
    Replicate(Box<IrExpr>, Vec<IrExpr>),  // {n{a, b}}
    Array(Vec<IrExpr>),            // [a, b, c]
    ArrayRepeat(Box<IrExpr>, Box<IrExpr>),  // [value; N]
    Index(Box<IrExpr>, Box<IrExpr>),       // a[i]
    Slice(Box<IrExpr>, Box<IrRange>),      // a[lo..hi]
    PartSelect(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),  // a[msb:lsb]
    PartSelectIndexed { base, idx, width, up: bool },   // a[idx +: w] / a[idx -: w]
    Mintypmax(Box<IrExpr>, Box<IrExpr>, Box<IrExpr>),   // min:typ:max

    // ── Misc ──
    PortFlow(String),         // <port> flow access (AMS)
    BundleLit { ty: String, fields: Vec<(String, IrExpr)> },  // Type { .f = e }
    Lambda { params: Vec<String>, body: Box<IrExpr> },         // |a, b| body
}
```

### `Param` vs `Var`

- **`Param(name)`** — a compile-time parameter. The value comes from the module's parameter map (resolved at instantiation). The codegen reads it from the params array passed to the device.
- **`Var(name)`** — a runtime variable. The codegen must allocate storage (local stack slot or register) and read/write it during evaluation.

The lowering inlines local vars when their values are known (single assignment, no mutation). When a var is mutated (e.g. inside a loop or across if-branches), the lowering emits `IrStmt::VarDecl` + `IrExpr::Var(name)` references instead of inlining.

### `IrRange`

```rust
pub struct IrRange {
    pub start: IrExpr,
    pub end: IrExpr,
    pub inclusive: bool,  // true for ..=, false for ..
}
```

### `IrBinOp`

| Variant | Operators | Source |
|---------|-----------|--------|
| `Add` `Sub` `Mul` `Div` `Rem` `Pow` | `+ - * / % **` | AMS + PHDL |
| `Eq` `Ne` `Lt` `Le` `Gt` `Ge` | `== != < <= > >=` | AMS + PHDL |
| `And` `Or` | `&& \|\|` | AMS (logical) |
| `BitAnd` `BitOr` `BitXor` | `& \| ^` | AMS + PHDL |
| `Shl` `Shr` | `<< >>` | AMS (shifts) |
| `AShl` `AShr` | `<<< >>>` | AMS (arithmetic shifts) |

Note: PHDL uses `&`/`|` for both bitwise and logical (no `&&`/`||`); the lowering maps them to `BitAnd`/`BitOr`. AMS has both `&&`/`||` (→ `And`/`Or`) and `&`/`|` (→ `BitAnd`/`BitOr`).

### `IrUnOp`

| Variant | Operator | Source |
|---------|----------|--------|
| `Neg` | `-` | AMS + PHDL |
| `Not` | `!` | AMS + PHDL |
| `BitNot` | `~` | AMS |
| `RedAnd` `RedNand` `RedOr` `RedNor` `RedXor` `RedXnor` | `& ~& \| ~\| ^ ~^` | AMS (reduction ops) |

### `SimQuery`

```rust
pub enum SimQuery {
    Temperature,                    // $temperature
    Vt(Option<Box<IrExpr>>),        // $vt or $vt(T)
    Abstime,                        // $abstime
    Mfactor,                        // $mfactor
    XPosition,                      // $xposition
    YPosition,                      // $yposition
    Angle,                          // $angle
    Simparam { key: String, default: Box<IrExpr> },  // $simparam("key", default)
    Analysis(String),               // analysis("tran"), analysis("ac"), ...
    ParamGiven(String),             // $param_given("name")
    PortConnected(String),          // $port_connected("name")
    Limit { kind: String, args: Vec<IrExpr> },  // $limit(x, "pnjlim", ...)
    Random { kind: String, args: Vec<IrExpr> }, // $random, $dist_normal, ...
}
```

### `IrNature`

```rust
pub enum IrNature {
    Potential(String),  // access fn: "V", "Pwr", "Temp", ...
    Flow(String),       // access fn: "I", ...
}
```

The nature of a branch access or contribution. In Verilog-AMS, natures define
access function names (`access = V;` for Voltage, `access = I;` for Current).
Custom disciplines may have custom access functions (`Pwr`, `Temp`, `Position`,
etc.). The IR carries the access function name and whether it's a potential
(across) or flow (through) nature.

- `Potential("V")` — voltage-like (across) contribution: `V(p,n) <+ expr`
- `Flow("I")` — current-like (through) contribution: `I(p,n) <+ expr`
- `Potential("Pwr")` — custom potential: `Pwr(p,n) <+ expr`

The codegen uses `is_potential()` to determine the MNA stamping pattern and
`access()` to resolve the branch reference.

Helper methods:
- `IrNature::access() -> &str` — returns the access function name
- `IrNature::is_potential() -> bool` — true for Potential variants

---

## 5. Statements

```rust
pub enum IrStmt {
    // ── Analog contributions ──
    Contrib { nature, plus, minus, expr, kind: ContribKind },
    Force { nature, plus, minus, expr },
    IndirectContrib { contrib_nature, contrib_plus, contrib_minus,
                      probe_nature, probe_plus, probe_minus, expr },

    // ── Control flow ──
    If { cond, then_, else_, label: Option<String> },
    Case { discriminant, arms, default, kind: CaseKind, label },
    For { var, start, end, step, body },
    While { cond, body },
    Repeat { count, body },
    Forever { body },
    Return(Option<IrExpr>),

    // ── Declarations ──
    VarDecl { name, ty: IrType, init: Option<IrExpr> },

    // ── Digital assignments ──
    NonBlocking { lval, expr, delay: Option<IrExpr>, event: Option<IrEventSpec> },
    Assign { lval, expr, delay: Option<IrExpr>, event: Option<IrEventSpec> },
    ContinuousAssign { lval, expr, delay: Option<IrExpr> },
    ProcAssign { lval, expr, is_force: bool },
    ProcDeassign { lval, is_release: bool },

    // ── Timing & events ──
    Delay { delay: IrExpr, body: Box<IrStmt> },
    EventControl { spec: IrEventSpec, body: Box<IrStmt> },
    Wait { cond, body: Box<IrStmt> },
    Fork { label: Option<String>, branches: Vec<Vec<IrStmt>>, join: JoinKind },
    Disable(String),
    Trigger(String),

    // ── Analog events ──
    AnalogEvent { kind: IrEventKind, body: Vec<IrStmt> },

    // ── Simulator control ──
    BoundStep(IrExpr),
    Finish,
    Discontinuity(i32),
    Diagnostic { severity: Severity, format: String, args: Vec<IrExpr> },
}
```

### Contribution statements

#### `Contrib`

```rust
Contrib { nature: IrNature, plus: String, minus: String, expr: IrExpr, kind: ContribKind }
```

The analog contribution operator `<+`:
- `I(p, n) <+ expr` → `Contrib { nature: Current, plus: "p", minus: "n", expr, kind }`
- `V(p, n) <+ expr` → `Contrib { nature: Voltage, plus: "p", minus: "n", expr, kind }`

`ContribKind`:
- `Resistive` — no state refs in `expr`; stamps directly into the DC Jacobian.
- `Reactive(id)` — `expr` contains `StateRef(id)` (a `ddt`/`idt`); the codegen must apply integration and stamp with `alpha = 1/dt`.

#### `Force`

```rust
Force { nature: IrNature, plus: String, minus: String, expr: IrExpr }
```

The force operator `<-` (ideal source). PHDL distinguishes `<+` (contribution, parallel) from `<-` (force, ideal). AMS uses `<+` for both.

#### `IndirectContrib`

```rust
IndirectContrib { contrib_nature, contrib_plus, contrib_minus,
                  probe_nature, probe_plus, probe_minus, expr }
```

The indirect branch contribution: `I(cp, cm) : V(pp, pm) = expr`. The current through `(cp, cm)` is controlled by the voltage across `(pp, pm)`.

### Control flow

#### `If` / `Case`

```rust
If { cond: IrExpr, then_: Vec<IrStmt>, else_: Vec<IrStmt>, label: Option<String> }
Case { discriminant: IrExpr, arms: Vec<(IrExpr, Vec<IrStmt>)>, default: Vec<IrStmt>,
       kind: CaseKind, label: Option<String> }
```

`CaseKind`: `Case` (exact), `CaseX` (x don't-care), `CaseZ` (z don't-care).

`label` is the block label from `begin : foo ... end` (AMS). Used by `disable foo;`.

#### Loops

```rust
For { var: String, start: IrExpr, end: IrExpr, step: IrExpr, body: Vec<IrStmt> }
While { cond: IrExpr, body: Vec<IrStmt> }
Repeat { count: IrExpr, body: Vec<IrStmt> }
Forever { body: Vec<IrStmt> }
```

- **`For`** is a runtime loop (var < end, step). Compile-time loops with constant bounds are unrolled during lowering and do not appear as `For` in the IR.
- Loops in analog blocks are restricted by the Verilog-AMS LRM: no contributions inside `while`/`forever` (the lowering enforces this). `for` with constant bounds is unrolled.
- Loops in digital blocks may contain contributions (non-blocking assigns) and run at event time.

#### `Return`

```rust
Return(Option<IrExpr>)
```

Returns from a function/task. `Return(None)` for tasks (void). `Return(Some(expr))` for functions.

### Declarations

#### `VarDecl`

```rust
VarDecl { name: String, ty: IrType, init: Option<IrExpr> }
```

A local variable declaration. In analog context, vars with known single-assignment values are inlined by the lowering; `VarDecl` appears only when the var is mutated or needs storage.

### Digital assignments

| Statement | Syntax | Semantics |
|-----------|--------|-----------|
| `NonBlocking` | `q <= d` | Non-blocking assign (digital, scheduled at end of timestep) |
| `Assign` | `x = expr` | Blocking assign (digital, immediate) |
| `ContinuousAssign` | `assign x = expr` | Continuous assignment (structural, always active) |
| `ProcAssign` | `assign x = expr` / `force x = expr` | Procedural assign/force (overrides continuous) |
| `ProcDeassign` | `deassign x` / `release x` | Procedural deassign/release |

All three of `NonBlocking`/`Assign`/`ContinuousAssign` carry optional `delay` and (for the first two) optional `event` timing controls.

### Timing & events

#### `Delay` / `EventControl` / `Wait`

```rust
Delay { delay: IrExpr, body: Box<IrStmt> }
EventControl { spec: IrEventSpec, body: Box<IrStmt> }
Wait { cond: IrExpr, body: Box<IrStmt> }
```

- `#delay stmt` → `Delay { delay, body }`
- `@(event) stmt` → `EventControl { spec, body }`
- `wait(cond) stmt` → `Wait { cond, body }`

#### `Fork`

```rust
Fork { label: Option<String>, branches: Vec<Vec<IrStmt>>, join: JoinKind }
```

`JoinKind`: `All` (join), `Any` (join_any), `None` (join_none).

#### `Disable` / `Trigger`

```rust
Disable(String)   // disable label / disable name
Trigger(String)   // ->event_name
```

### Analog events

#### `AnalogEvent`

```rust
AnalogEvent { kind: IrEventKind, body: Vec<IrStmt> }
```

```rust
pub enum IrEventKind {
    InitialStep,
    FinalStep,
    Cross { dir: i8, expr: Option<IrExpr> },   // dir: 0=either, 1=rising, -1=falling
    Above { expr: Option<IrExpr> },
    Timer { period: Option<IrExpr> },
}
```

- `@(initial_step)` → `AnalogEvent { kind: InitialStep, body }`
- `@(final_step)` → `AnalogEvent { kind: FinalStep, body }`
- `@(cross(expr, dir))` → `AnalogEvent { kind: Cross { dir, expr: Some(expr) }, body }`
- `@(above(expr))` → `AnalogEvent { kind: Above { expr: Some(expr) }, body }`
- `@(timer(period))` → `AnalogEvent { kind: Timer { period: Some(period) }, body }`

PHDL `@ cross(V(p,n)) when (guard) { ... }` lowers to an `AnalogEvent` whose body is wrapped in an `If { cond: guard, ... }`.

### `IrEventSpec` (digital event control)

```rust
pub enum IrEventSpec {
    Posedge(IrExpr),    // posedge(signal)
    Negedge(IrExpr),    // negedge(signal)
    Change(IrExpr),     // change(signal) — any edge
    Cross(IrExpr, i8),  // cross(expr, dir)
    Above(IrExpr),      // above(expr)
    Initial,            // initial
    Final,              // final
    Timer(IrExpr),      // timer(period)
    Named(String),      // named event / @* wildcard
    Or(Vec<IrEventSpec>),  // (spec | spec | ...)
}
```

### Simulator control

| Statement | Source | Semantics |
|-----------|--------|-----------|
| `BoundStep(expr)` | `$bound_step(dt)` | Limit the next timestep to `dt` |
| `Finish` | `$finish` / `$stop` | Terminate simulation |
| `Discontinuity(n)` | `$discontinuity(n)` | Notify the solver of an order-`n` discontinuity |
| `Diagnostic` | `$display`/`$warning`/`$error`/`$fatal` | Print a diagnostic message |

```rust
Diagnostic { severity: Severity, format: String, args: Vec<IrExpr> }
```

`Severity`: `Info`, `Warning`, `Error`, `Fatal`.

---

## 6. State variables (analog operators)

Analog operators that carry internal state are allocated as state slots. Each slot has an `id` (u32), a `kind`, and an `arg` (the input expression).

```rust
pub struct IrStateVar {
    pub id: u32,
    pub kind: IrStateKind,
    pub arg: IrExpr,
}
```

```rust
pub enum IrStateKind {
    Ddt,                                    // ddt(x)
    Idt { ic: IrExpr },                     // idt(x, ic)
    IdtMod { ic: IrExpr, modulus: IrExpr }, // idtmod(x, ic, mod)
    Ddx { node: String },                   // ddx(x, node)
    Delay { delay: IrExpr },                // delay(x, t) / absdelay(x, t)
    Transition { delay, rise, fall, tol },  // transition(x, td, tr, tf, tol)
    Slew { rise, fall },                    // slew(x, rise, fall)
    Laplace { variant, num, den },          // laplace_np/zp/pm/nm/npm(x, num, den)
    ZTransform { variant, num, den, sample_dt }, // zi_zd/zp/nd/np(x, num, den, dt)
    Cross { dir: i8 },                      // cross() as event-detector state
    Timer { period: IrExpr },               // timer() as event state
}
```

### Usage in expressions

When an analog operator appears in an expression, the lowering:
1. Allocates a state slot (`ctx.alloc_state(kind, arg)`)
2. Returns `IrExpr::StateRef(id)` to reference it

The `arg` (input expression) is stored in `IrStateVar.arg` and hoisted to `IrAnalogBody::state_vars`. The codegen evaluates `arg` at each Newton iteration and applies the operator's semantics to produce the output.

### `first_state_ref` helper

```rust
pub fn first_state_ref(expr: &IrExpr) -> Option<u32>
```

Walks an expression tree looking for the first `StateRef(id)`. Used by the lowering to classify a contribution as `Resistive` (no state ref) or `Reactive(id)` (contains a state ref).

### Operator semantics (codegen responsibility)

| Operator | Integration | Stamping |
|----------|-------------|----------|
| `ddt` | Backward Euler / Trapezoidal: `state_next = (x_new - x_old) / dt` | Reactive Jacobian × `alpha = 1/dt` |
| `idt` | `state_next = state_old + x * dt` | Reactive |
| `idtmod` | `idt` + modular wrap | Reactive |
| `ddx` | Symbolic derivative w.r.t. node voltage | Computed at compile time (autodiff) |
| `delay` | Ring buffer of past values | Resistive (reads delayed value) |
| `transition` | Waveform queue with rise/fall times | Resistive |
| `slew` | Rate limiter | Resistive |
| `laplace_*` | Continuous-time filter (state-space) | Reactive |
| `zi_*` | Discrete-time filter (sampled at `dt`) | Reactive |

---

## 7. Noise sources

```rust
pub struct IrNoiseSource {
    pub plus: String,
    pub minus: String,
    pub kind: IrNoise,
    pub label: Option<String>,
}

pub enum IrNoise {
    White { psd: IrExpr },                 // white_noise(psd, "label")
    Flicker { psd: IrExpr, exponent: IrExpr }, // flicker_noise(psd, exp, "label")
}
```

Noise sources are extracted from contribution expressions by `scan_noise()` during lowering. The noise source is registered in `IrAnalogBody::noise_sources`, and the `white_noise`/`flicker_noise` call itself returns `Real(0.0)` in the expression position (it contributes noise, not current).

The codegen implements `Device::noise_current_psd` to emit `Noise { terminals, value }` for each registered source.

---

## 8. Analog body

```rust
pub struct IrAnalogBody {
    pub state_vars: Vec<IrStateVar>,
    pub noise_sources: Vec<IrNoiseSource>,
    pub vars: Vec<IrVarDecl>,
    pub stmts: Vec<IrStmt>,
}
```

A module has at most one `IrAnalogBody` (multiple AMS `analog` blocks are merged; PHDL has one `analog` block per module).

- `state_vars` — all `ddt`/`idt`/`transition`/... slots allocated in the body
- `noise_sources` — all `white_noise`/`flicker_noise` sources
- `vars` — local variable declarations (for the codegen to allocate storage)
- `stmts` — the body statements (contributions, if/case, events, etc.)

---

## 9. Digital body

```rust
pub struct IrDigitalBody {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub state_vars: Vec<IrVarDecl>,  // regs / latches
    pub stmts: Vec<IrStmt>,
}
```

A module has at most one `IrDigitalBody` (PHDL `digital` block; AMS `initial`/`always` blocks — though AMS digital blocks are currently dropped by the AMS `convert_module` and would need a separate lowering path).

- `inputs` / `outputs` — digital nets the block reads/writes
- `state_vars` — `reg`/`var` declarations that persist across timesteps
- `stmts` — event controls, non-blocking assigns, if/case, loops

---

## 10. Functions

```rust
pub struct IrFunction {
    pub name: String,
    pub params: Vec<String>,   // positional parameter names
    pub body: Vec<IrStmt>,
}
```

Functions and tasks are unified into `IrFunction`:
- **AMS `function`** — returns a value (the function name variable); the lowering adds an implicit `Return(env[function_name])` at the end.
- **AMS `task`** — void return; body is stmts without a `Return`.
- **PHDL `fn`** — returns via `Return(expr)`; the elaborator lowers `Stmt::Return(e)` to `ElabBehaviorStmt::Expr(e)`, and the codegen-level lowering converts it back to `IrStmt::Return`.

Calls to functions are opaque `IrExpr::Call(name, args)`. The codegen resolves user functions by looking up `IrProgram::functions` and `IrModule::functions`, then either:
1. **Inlines** the function body at the call site (alpha-substitute params with args, use `Return` value), or
2. **Compiles as a helper** (Cranelift function, emit a `call` instruction).

Functions live at two levels:
- `IrProgram::functions` — file-level (PHDL `fn` items)
- `IrModule::functions` — module-level (AMS `function`/`task` inside `module`)

---

## 11. Lowering contract

### From AMS (`from_ams.rs`)

Entry point: `pub fn ams_to_ir(doc: &Document) -> IrProgram`

| AMS construct | IR target |
|---------------|-----------|
| `module m(p, n)` | `IrModule { name, ports, ... }` |
| `parameter real x = 1.0` | `IrParam { name: "x", ty: Real, default: Some(Real(1.0)) }` |
| `electrical p, n;` | `IrWire { name: "p", discipline: "Electrical" }` per member |
| `branch (p, n) br;` | `IrBranch { name: "br", plus: "p", minus: "n" }` |
| `event e;` | `IrEventDecl { name: "e" }` |
| `real x;` | `IrVarDecl { name: "x", ty: Real, init: None }` |
| `ground gnd;` | `IrGroundDecl { name: "gnd", ... }` |
| `sub u1(.p(a), .n(b));` | `IrInstance { label: "u1", connections: [IrConnection{port:"p",net:"a"}, ...] }` |
| `assign x = expr;` | `IrStmt::ContinuousAssign { lval: "x", expr, delay: None }` |
| `analog begin ... end` | `IrAnalogBody { stmts, ... }` |
| `analog initial begin ... end` | `IrAnalogBody { stmts: [AnalogEvent { kind: InitialStep, body }] }` |
| `I(p,n) <+ expr` | `IrStmt::Contrib { nature: Current, plus: "p", minus: "n", expr, kind }` |
| `V(p,n) : I(src) == expr` | `IrStmt::IndirectContrib { ... }` |
| `if (c) s1 else s2` | `IrStmt::If { cond, then_, else_, label: None }` |
| `case (x) ...` | `IrStmt::Case { ..., kind: Case/CaseX/CaseZ }` |
| `for (i=0; i<N; i++) s` | Unrolled if constant; else `IrStmt::For { ... }` |
| `while (c) s` | `IrStmt::While { cond, body }` |
| `@(cross(V,1)) s` | `IrStmt::AnalogEvent { kind: Cross { dir: 1, expr: Some(V) }, body }` |
| `ddt(x)` | `StateRef(id)` + `IrStateVar { id, kind: Ddt, arg: x }` |
| `transition(x, td, tr, tf)` | `StateRef(id)` + `IrStateVar { id, kind: Transition { ... }, arg: x }` |
| `$vt` | `IrExpr::Sim(SimQuery::Vt(None))` |
| `$display("fmt", args)` | `IrStmt::Diagnostic { severity: Info, format: "fmt", args }` |
| `$bound_step(dt)` | `IrStmt::BoundStep(dt)` |
| `$discontinuity(n)` | `IrStmt::Discontinuity(n)` |
| `function real f(input real x); ... endfunction` | `IrFunction { name: "f", params: ["x"], body }` |
| `task t; ... endtask` | `IrFunction { name: "t", params: [...], body }` |
| `q <= d` | `IrStmt::NonBlocking { lval: "q", expr: d, delay, event }` |
| `#5 x = 1;` | `IrStmt::Delay { delay: 5, body: Assign { ... } }` |
| `@(posedge(clk)) s` | `IrStmt::EventControl { spec: Posedge(clk), body }` |
| `fork ... join` | `IrStmt::Fork { label, branches, join: All }` |
| `disable blk;` | `IrStmt::Disable("blk")` |
| `->e;` | `IrStmt::Trigger("e")` |

### From PHDL (`from_ppr.rs`)

Entry point: `pub fn ppr_to_ir(prog: &ElabProgram) -> IrProgram`

| PHDL construct | IR target |
|----------------|-----------|
| `mod M(inout p: Electrical)` | `IrModule { name: "M", ports, ... }` |
| `param x: Real = 1.0` | `IrParam { name: "x", ty: Real, default: Some(Real(1.0)) }` |
| `wire p: Electrical;` | `IrWire { name: "p", discipline: "Electrical" }` |
| `u1: Sub(a, b) { .r = 1k }` | `IrInstance { label: "u1", connections: [positional], params: [("r", 1000)] }` |
| `lhs = rhs;` (net conn) | `IrConnectionDecl { lhs, rhs }` |
| `analog M { ... }` | `IrAnalogBody { stmts, ... }` |
| `digital M { ... }` | `IrDigitalBody { stmts, ... }` |
| `I(p,n) <+ expr` | `IrStmt::Contrib { nature: Current, ... }` |
| `V(p,n) <- expr` | `IrStmt::Force { nature: Voltage, ... }` |
| `if (c) { t } else { e }` | `IrStmt::If { cond, then_, else_, label: None }` |
| `match x { A => { ... }, _ => { ... } }` | Desugared to `IrStmt::If` chain (eq checks) |
| `@ cross(V(p,n)) when (guard) { ... }` | `IrStmt::AnalogEvent { kind: Cross { .. }, body: [If { cond: guard, ... }] }` |
| `@ change(clk) { ... }` | `IrStmt::AnalogEvent` (analog) or digital event handling |
| `ddt(V(p,n))` | `StateRef(id)` + `IrStateVar { id, kind: Ddt, arg: V(p,n) }` |
| `transition(x, td, tr, tf)` | `StateRef(id)` + `IrStateVar { id, kind: Transition { ... } }` |
| `white_noise(psd, "label")` | Side-effect: `IrNoiseSource { ..., kind: White { psd }, label }`; expr → `Real(0.0)` |
| `$simparam("key", default)` | `IrExpr::Sim(SimQuery::Simparam { key, default })` |
| `$bound_step(dt)` | `IrStmt::BoundStep(dt)` (from Diagnostic stmt) |
| `fn f(x: Real) -> Real { return x * 2.0; }` | `IrFunction { name: "f", params: ["x"], body: [Return(Binary(Mul, Var("x"), Real(2.0)))] }` |
| `match` (in fn body) | Desugared to `If` chain |
| `if (c) { t } else { e }` (expr) | `IrExpr::Select(c, t, e)` |
| `[a, b, c]` | `IrExpr::Array([a, b, c])` |
| `[value; N]` | `IrExpr::ArrayRepeat(value, N)` |
| `a[i]` | `IrExpr::Index(a, i)` |
| `a[lo..hi]` | `IrExpr::Slice(a, IrRange { start: lo, end: hi, inclusive: false })` |
| `a.field` (bundle) | `IrExpr::Param("a_field")` (flattened by elaborator) |
| `Type { .f = e }` (bundle lit) | `IrExpr::BundleLit { ty: "Type", fields: [("f", e)] }` |
| `\|a, b\| a + b` (lambda) | `IrExpr::Lambda { params: ["a", "b"], body }` |

### Inlining vs retention

The lowering inlines local variables when safe (single static assignment, value known at lowering time). This produces compact IR but loses variable identity. When a variable is:
- Mutated after assignment → retained as `IrStmt::VarDecl` + `IrExpr::Var(name)`
- Assigned inside a loop → retained (the loop body may execute multiple times)
- Assigned in one branch of an if/else and read after → phi-merged via `Select(cond, then_val, else_val)` and inlined

---

## 12. Codegen contract (planned)

The codegen lowers `IrProgram` → `Vec<Box<dyn piperine_solver::Device>>` + `Netlist`.

### Analog device generation

For each module with an `IrAnalogBody`:
1. Allocate nodes/branches in the `Netlist`
2. JIT-compile (Cranelift) or interpret two functions:
   - `residual(node_voltages, params, sim_ctx, state, rhs)` — evaluates all contributions
   - `jacobian(node_voltages, params, sim_ctx, state, jac)` — symbolic/numeric derivative
3. Wrap in a `Device` impl that:
   - `load_dc` — calls `residual` + `jacobian`, stamps Norton equivalent
   - `load_transient` — same + reactive stamping (`react_jac * alpha = 1/dt`)
   - `noise_current_psd` — emits `Noise` for each `IrNoiseSource`
   - `update` / `accept_timestep` — manage state vector

### Digital device generation

For each module with an `IrDigitalBody`:
1. Collect `digital_input_nets` / `digital_output_nets`
2. Compile (interpreter or JIT) the body as an event-driven evaluator
3. Wrap in a `Device` impl that:
   - `eval_discrete` — processes events, updates `LogicValue` outputs
   - `digital_init` — seeds initial events

### Mixed-signal bridges

For modules with both analog and digital bodies, or for discipline crossings:
- A2D: `eval_discrete` reads analog voltages, thresholds to `LogicValue`
- D2A: `load_dc`/`load_transient` stamps a Thevenin source based on digital state

---

## 13. File map

| File | Role |
|------|------|
| `crates/piperine-codegen/src/ir.rs` | IR type definitions (this document) |
| `crates/piperine-codegen/src/lib.rs` | Public API: `ams_to_ir`, `ppr_to_ir`, `pub use ir::*` |
| `crates/piperine-codegen/src/from_ams.rs` | AMS `Document` → `IrProgram` lowering |
| `crates/piperine-codegen/src/from_ppr.rs` | PHDL `ElabProgram` → `IrProgram` lowering |
| `crates/piperine-codegen/src/display.rs` | Debug pseudo-language printer for `IrProgram` |
| `crates/piperine-codegen/tests/ams_ir_test.rs` | AMS → IR tests (31 tests) |
| `crates/piperine-codegen/tests/ppr_ir_test.rs` | PHDL → IR tests (23 tests) |

---

## 14. Test coverage

### AMS tests (31)

| Category | Tests |
|----------|-------|
| Basic contrib | `conductor_module_parsed`, `conductor_has_resistive_contrib`, `conductor_param_g` |
| Resistor | `resistor_two_modules`, `resistor_res1_current_contrib`, `resistor_res2_voltage_contrib`, `resistor_res3_if_structure` |
| Capacitor | `capacitor_cap1_has_ddt`, `capacitor_cap1_reactive_contrib`, `capacitor_cap2_has_if` |
| Diode | `snap_diode`, `inline_diode_nonlinear_contrib` |
| Printer | `snap_*`, `printer_produces_source_header` |
| Shifts | `shift_operator_preserved` |
| Reductions | `reduction_operator_preserved` |
| Named ports | `named_port_connection_preserved` |
| Strings | `string_literal_preserved` |
| Functions | `function_lowered` |
| Noise | `noise_source_registered_ams` |
| Discontinuity | `discontinuity_stmt` |
| Transition | `transition_state_var_ams` |
| Simparam | `simparam_query_ams` |
| Delay | `delay_state_var_ams` |
| Laplace | `laplace_state_var_ams` |
| Timer event | `timer_event_ams` |
| Cross event | `cross_event_with_expr_ams` |
| Branches | `branch_declaration_preserved` |
| Param types | `parameter_types_preserved` |

### PHDL tests (23)

| Category | Tests |
|----------|-------|
| Basic contrib | `resistor_resistive_contrib`, `resistor_printer_smoke` |
| Capacitor | `capacitor_reactive_contrib_with_state_var`, `capacitor_printer_reactive` |
| Local var | `local_var_inlined_into_contrib` |
| Diode | `diode_nonlinear_contrib` |
| If | `if_stmt_both_branches_preserved`, `nested_if_structure_preserved` |
| Module | `module_ports_and_params_present` |
| Noise | `noise_source_registered`, `flicker_noise_source_registered` |
| idtmod | `idtmod_state_var` |
| Single-arg I | `single_arg_current_access` |
| Force | `force_contribution` |
| Match | `match_desugars_to_if_chain` |
| Event guard | `event_guard_wraps_body` |
| Above event | `above_event` |
| Simparam | `simparam_query` |
| Bound step | `bound_step_stmt` |
| Digital | `digital_behavior_lowered` |
| Functions | `global_function_lowered` |
| String param | `string_param_preserved` |
| Transition | `transition_state_var` |

---

## 15. Design decisions & trade-offs

### Why no type system in the IR?

Analog evaluation is purely `f64`. Adding a full type system would bloat the IR without benefit. The minimal `IrType` enum exists only where the codegen needs to make decisions (string vs numeric params, 4-state logic storage).

### Why are functions opaque `Call(name, args)`?

The IR does not distinguish `exp(x)` from `my_func(x)` — both are `Call("exp", [x])` / `Call("my_func", [x])`. The codegen resolves this at compile time:
- Built-in math (`exp`, `ln`, `sqrt`, ...) → direct libm call
- Analog operators (`ddt`, `idt`, ...) → state-allocated (handled during lowering, never appears as `Call`)
- User functions → inline or compile as helper

This keeps the IR simple while preserving all information.

### Why is `match` desugared to `if` chains?

PHDL's `match` with only `Path`/`Wildcard` patterns is equivalent to an if/else-if chain comparing the discriminant to each pattern. Desugaring at lowering time avoids a separate `IrStmt::Match` variant. If PHDL adds literal patterns or destructuring in the future, a dedicated `Match` variant can be added.

### Why are `Param` and `Var` separate?

- `Param` is read-only, set at instantiation time, stored in the device's params array.
- `Var` is read-write, stored in the device's local storage or stack.
The codegen needs to know which array to index, hence the distinction.

### Why inlining?

Inlining local vars produces compact IR and enables the codegen to generate efficient straight-line code without local variable management. The downside is code duplication if a var is used many times, but analog blocks are typically small. The lowering retains `VarDecl` + `Var` when inlining is unsafe (mutation, loops).

## 16. From IR to solver (current state)

The IR → Device path lives in `crates/piperine-codegen/src/`:

| Entry point | Returns |
|------------|---------|
| `ams_to_ir(doc)` | `IrProgram` from `piperine_ams::Document` |
| `ppr_to_ir(prog)` | `IrProgram` from `piperine_lang::elab::ElabProgram` |
| `ir_analog_to_device(prog, module_name)` | `JitAnalogDevice` (Cranelift) |
| `ir_digital_to_interp(prog, module_name)` | `DigitalInterpreter` |
| `from_ir(prog, top)` | `CircuitInstance` ready for the solver |

### Limitations (current)

- The `IrExpr` → PHDL-AST translation inside `ir_analog_to_device.rs` is
  intentionally narrow — it covers `Real/Int/Bool/Param/Var/Call`,
  `Binary` (add/sub/mul/div/rem), `Unary`, and `BranchAccess`.  Anything
  else falls back to `Real(0.0)`, so the test suite targets the
  boilerplate VA fixtures and simple PHDL devices.
- `from_ir` assumes positional port connections; named-port support is
  in place but the layout for AMS has minor edge cases.
- OSDI-fed resistor fixtures (`piperine-solver/tests/va/*.va`) are
  the canonical numeric baseline; the in-house IR-built resistors
  exercise the same Newton-Raphson path but the IR → Device lowering
  may produce slightly different stamps.
