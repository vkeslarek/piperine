# IR and JIT Compilation — Technical Specification

`piperine-lang` defines a language-neutral intermediate representation for analog/mixed-signal circuits and a Cranelift-based JIT compiler that lowers that IR directly to machine code without any external toolchain. It sits between the language front-ends (Verilog-AMS parser, future `.ptb` testbench) and the Newton-Raphson solver in `piperine-solver`. Each front-end implements `FrontendLower` to produce an `IrDesign`; `piperine-solver` consumes it via `CircuitInstance::from_design()`.

```
.ppr / .va
      │
      ▼
 Document::parse()          [piperine-parser]
      │
      ▼
 FrontendLower::lower()     [piperine-parser/src/lower.rs]
      │  impl for Document
      ▼
   IrDesign                 [piperine-lang/src/ir/]
      │
      ├─ disciplines, natures (IrDiscipline, IrNature)
      ├─ nets:  HashMap<NodeId, IrNet>
      │
      ├─ AnalogIrInstance  ──► AnalogBody::Source(IrAnalogBlock)
      │                                │
      │                        compile_analog_block()
      │                        [piperine-lang/src/codegen/analog.rs]
      │                                │
      │                         JitAnalogDevice
      │                         (Cranelift JIT)
      │                                │
      │                           JitDevice
      │                        impl AnalogDevice
      │                                │
      ├─ AnalogIrInstance ──► AnalogBody::Osdi{path}   (BSIM4, PSP …)
      ├─ AnalogIrInstance ──► AnalogBody::Primitive     (R/L/C/V/I …)
      ├─ DigitalIrInstance
      └─ ConnectIrInstance              │
                                 CircuitInstance
                                 [piperine-solver/src/circuit.rs]
                                        │
                                 Newton-Raphson DC/Tran/AC
```

---

## 1. IR Specification

### 1.1 Design Root: `IrDesign`

Defined in `crates/piperine-lang/src/ir/design.rs`.

```rust
pub struct IrDesign {
    pub meta:              IrMeta,
    pub disciplines:       HashMap<String, IrDiscipline>,
    pub natures:           HashMap<String, IrNature>,
    pub nets:              HashMap<NodeId, IrNet>,
    pub analog_instances:  Vec<AnalogIrInstance>,
    pub digital_instances: Vec<DigitalIrInstance>,
    pub connect_instances: Vec<ConnectIrInstance>,
}
```

**`meta: IrMeta`** — provenance: top module name, source language (`IrSource`), version string.

**`disciplines`** — all `discipline` declarations merged from the design hierarchy, keyed by name (e.g. `"electrical"`, `"logic"`). Each discipline carries `domain`, `potential`, `flow` nature names.

**`natures`** — all `nature` declarations, keyed by name (e.g. `"Voltage"`, `"Current"`). A nature carries `units`, `access` function name (e.g. `"V"`), `abstol`, and optional parent for inheritance.

**`nets`** — every net after flattening the instance hierarchy, keyed by `NodeId`. GND is always present as `NodeId(0)`.

**`analog_instances`, `digital_instances`, `connect_instances`** — flat lists of device instances, no hierarchy.

**Invariants after lowering:**
- Every `NodeId` referenced in any `PortBinding`, `IrBranch`, or `ConnectIrInstance` exists as a key in `design.nets`.
- `NodeId(0)` is always present, always `Domain::Analog`, name `"gnd"`.
- All parameters in `AnalogIrInstance.parameters` are fully folded to `f64` — no symbolic expressions remain.
- `IrAnalogBlock.branches` contains every branch name referenced in `IrAnalogStmt::Contribution`.
- All names (instance names, net paths) are globally unique — hierarchy is encoded in dot-separated paths.

---

### 1.2 Net Model

Defined in `crates/piperine-lang/src/ir/net.rs`.

#### `NodeId`

```rust
pub struct NodeId(pub u32);
impl NodeId {
    pub const GND: NodeId = NodeId(0);
}
```

Globally unique non-negative integer assigned during elaboration. Counters start at 1; `NodeId(0)` is reserved for the global ground net. Used as the index into the node-voltage array in JIT-compiled functions.

#### `Domain`

```rust
pub enum Domain {
    Analog,
    Digital,
    Wreal,
}
```

| Variant | Semantics |
|---------|-----------|
| `Analog` | KCL/KVL node. Participates in the MNA solver matrix. Voltage is a continuous `f64`. |
| `Digital` | Carries a `LogicValue` (0, 1, X, Z). Lives in the digital event queue. Never appears in the Jacobian. |
| `Wreal` | AMS `wreal` — continuous real value on a digital wire. Treated as `Analog` for solver purposes but carries no discipline. |

Domain assignment rules during lowering:
- A net's domain is derived from its discipline. The discipline's `domain` field (`Domain::Analog` or `Domain::Digital`) propagates to the net.
- `wreal` nets have `Domain::Wreal` and `discipline: None`.
- GND always has `Domain::Analog` regardless of discipline.
- Ports with no discipline default to `Domain::Analog`.

#### `IrNet`

```rust
pub struct IrNet {
    pub id:         NodeId,
    pub name:       String,   // leaf name, e.g. "drain"
    pub path:       String,   // hierarchical path, e.g. "top.u1.drain"
    pub domain:     Domain,
    pub discipline: Option<String>,
}
```

`path` is preserved from the pre-flattening hierarchy for waveform output. `discipline` is `None` for synthetic nodes (A2D/D2A split nodes, GND).

---

### 1.3 Type System

Defined in `crates/piperine-lang/src/ir/types.rs`.

#### `IrScalarType`

```rust
pub enum IrScalarType {
    Logic,
    Integer,
    Real,
    Reg { bits: u32 },
}
```

Type of a variable or local declaration inside a behavioral block.

| Variant | Verilog-AMS | Usage |
|---------|-------------|-------|
| `Logic` | `wire`, `logic` | Digital signals |
| `Integer` | `integer` | Loop variables, counters |
| `Real` | `real` | Analog variables |
| `Reg { bits }` | `reg [n-1:0]` | Bit vectors |

#### `IrValue`

```rust
pub enum IrValue {
    Real(f64),
    Int(i64),
    Str(String),
}
```

Constant value after parameter folding. All numeric parameters are `Real` or `Int` after elaboration; `Str` is used only for string parameters (e.g. model names in BSIM: `.MODEL = "nmos_lvt"`).

`IrValue::as_real()` coerces `Int` to `f64`.

#### `IrDiscipline`

```rust
pub struct IrDiscipline {
    pub name:      String,
    pub domain:    Domain,
    pub potential: Option<String>,  // e.g. "Voltage"
    pub flow:      Option<String>,  // e.g. "Current"
}
```

Maps directly from the `discipline ... enddiscipline` block. `potential` and `flow` are nature names that can be looked up in `IrDesign.natures`.

#### `IrNature`

```rust
pub struct IrNature {
    pub name:    String,
    pub units:   Option<String>,  // e.g. "V", "A"
    pub access:  Option<String>,  // e.g. "V" for V(node)
    pub abstol:  f64,
    pub parent:  Option<String>,
}
```

`abstol` is the convergence tolerance for this nature. `access` is the function name used in expressions (`V(...)`, `I(...)`). `parent` supports nature inheritance chains.

#### `IrBranch`

```rust
pub struct IrBranch {
    pub name:  String,   // e.g. "(a, b)" or declared name "rb"
    pub plus:  NodeId,
    pub minus: NodeId,
}
```

A directed edge. Current flows from `plus` to `minus` when positive. `name` is the key used in `IrContribution` and `IrExpr::BranchVoltage`. Implicit branches (from `I(a,b) <+ ...`) get auto-assigned names of the form `"(a, b)"`.

#### `PortBinding`

```rust
pub struct PortBinding {
    pub port_name: String,
    pub net_id:    NodeId,
}
```

Connects a module port name to a net in the flat design. Used in both `AnalogIrInstance.terminals` and `DigitalIrInstance.{input,output,inout}_ports`.

#### `IrVariable`

```rust
pub struct IrVariable {
    pub name: String,
    pub ty:   IrScalarType,
}
```

Declared variable inside a digital block. Analog blocks use `IrAnalogStmt::LocalVar` instead.

#### `IrSource`

```rust
pub enum IrSource {
    VerilogAms { path: PathBuf },
    PiperineTb { path: PathBuf },
    Synthetic,
}
```

Provenance tag for diagnostics. `Synthetic` is used in tests and when `IrDesign` is constructed programmatically.

---

### 1.4 Expression IR: `IrExpr`

Defined in `crates/piperine-lang/src/ir/expr.rs`.

All expressions produce `f64` unless noted. Boolean comparisons produce `1.0` (true) or `0.0` (false).

```rust
pub enum IrExpr {
    Literal(f64),
    Var(String),
    BranchVoltage(String),
    BranchCurrent(String),
    NodeVoltage(NodeId),
    AnalogFn { func: IrAnalogFn, args: Vec<IrExpr> },
    BinaryOp { op: IrBinOp, lhs: Box<IrExpr>, rhs: Box<IrExpr> },
    UnaryOp  { op: IrUnOp, operand: Box<IrExpr> },
    Select   { cond: Box<IrExpr>, then_: Box<IrExpr>, else_: Box<IrExpr> },
}
```

| Variant | Verilog-AMS syntax | Notes |
|---------|-------------------|-------|
| `Literal(f64)` | `1.0`, `1k`, `1e-9` | SI suffixes folded at parse time |
| `Var(name)` | parameter or local variable name | Resolved to name after elaboration; no scope lookup at runtime |
| `BranchVoltage(name)` | `V(a,b)`, `V(br)` | `name` is branch name, e.g. `"(a, b)"` or declared `"rb"` |
| `BranchCurrent(name)` | `I(a,b)`, `I(br)` | Same naming convention; not yet emitted by JIT |
| `NodeVoltage(id)` | `V(net)` single-arg | Voltage relative to GND; not yet emitted by JIT |
| `AnalogFn{func,args}` | `sin(x)`, `exp(x)`, etc. | See `IrAnalogFn` table |
| `BinaryOp{op,lhs,rhs}` | `a + b`, `a / b`, etc. | See `IrBinOp` table |
| `UnaryOp{op,operand}` | `-x`, `!x`, `~x` | See `IrUnOp` table |
| `Select{cond,then_,else_}` | `c ? t : e` | `cond == 0.0` → false, nonzero → true |

#### `IrAnalogFn`

| Variant | AMS name | Args | Derivative support |
|---------|----------|------|--------------------|
| `Sin` | `sin` | 1 | `cos(u) * u'` |
| `Cos` | `cos` | 1 | `-sin(u) * u'` |
| `Tan` | `tan` | 1 | `u' / cos²(u)` |
| `Asin` | `asin` | 1 | `u' / sqrt(1-u²)` |
| `Acos` | `acos` | 1 | `-u' / sqrt(1-u²)` |
| `Atan` | `atan` | 1 | `u' / (1 + u²)` |
| `Atan2` | `atan2` | 2 | 0 (simplified) |
| `Exp` | `exp` | 1 | `exp(u) * u'` |
| `Ln` | `ln` | 1 | `u' / u` |
| `Log` | `log10` | 1 | `u' / (u * ln10)` |
| `Sqrt` | `sqrt` | 1 | `u' / (2*sqrt(u))` |
| `Pow` | `pow` | 2 | `v*u^(v-1)*u'` |
| `Abs` | `abs` | 1 | `sign(u) * u'` |
| `Min` | `min` | 2 | 0 a.e. |
| `Max` | `max` | 2 | 0 a.e. |
| `Floor` | `floor` | 1 | 0 a.e. |
| `Ceil` | `ceil` | 1 | 0 a.e. |
| `LimExp` | `limexp` | 1 | `exp(min(u,80)) * u'` |
| `Cross` | `$cross` | 2 | 0 (event) |
| `Above` | `$above` | 1 | 0 (event) |
| `Timer` | `$timer` | 1+ | 0 (event) |
| `WhiteNoise` | `white_noise` | 1 | 0 (noise) |
| `FlickerNoise` | `flicker_noise` | 2 | 0 (noise) |

#### `IrBinOp`

| Variant | Operator | Codegen |
|---------|----------|---------|
| `Add` | `+` | `fadd` |
| `Sub` | `-` | `fsub` |
| `Mul` | `*` | `fmul` |
| `Div` | `/` | `fdiv` |
| `Rem` | `%` | 0 (not emitted) |
| `Pow` | `**` | libm `pow` |
| `Eq` | `==` | `fcmp Equal` → 1.0/0.0 |
| `Ne` | `!=` | `fcmp NotEqual` |
| `Lt` | `<` | `fcmp LessThan` |
| `Le` | `<=` | `fcmp LessThanOrEqual` |
| `Gt` | `>` | `fcmp GreaterThan` |
| `Ge` | `>=` | `fcmp GreaterThanOrEqual` |
| `BitAnd` | `&` | 0 (integer-only) |
| `BitOr` | `\|` | 0 |
| `BitXor` | `^` | 0 |
| `Shl` | `<<` | 0 |
| `Shr` | `>>` | 0 |
| `LogAnd` | `&&` | 0 |
| `LogOr` | `\|\|` | 0 |

Integer/bitwise operators on `f64` emit `0.0` — they are valid in digital blocks (via `IrStmt`) but are not meaningful in the analog codegen path.

#### `IrUnOp`

| Variant | Operator | Codegen |
|---------|----------|---------|
| `Neg` | `-` (unary) | `fneg` |
| `LogNot` | `!` | passthrough (0 emitted for boolean context) |
| `BitNot` | `~` | passthrough |
| `RedAnd` | `&` (reduction) | passthrough |
| `RedOr` | `\|` (reduction) | passthrough |
| `RedXor` | `^` (reduction) | passthrough |
| `RedNand` | `~&` | passthrough |
| `RedNor` | `~\|` | passthrough |
| `RedXnor` | `~^` | passthrough |

Only `Neg` is lowered to a real instruction in the analog codegen. The rest pass through (the Cranelift value of the operand is returned unchanged).

---

### 1.5 Statement IR

Two statement enums exist because analog and digital behavioral code have fundamentally different semantics.

#### `IrStmt` — digital procedural code

Used in `IrAlwaysBlock.body` and `IrInitialBlock.body`.

```rust
pub enum IrStmt {
    Block(Vec<IrStmt>),
    If { cond: IrExpr, then_: Box<IrStmt>, else_: Option<Box<IrStmt>> },
    Case { expr: IrExpr, arms: Vec<IrCaseArm>, default: Option<Box<IrStmt>> },
    Assign          { target: String, expr: IrExpr },           // blocking =
    NonBlockingAssign { target: String, expr: IrExpr, delay: Option<f64> }, // <=
    EventControl    { sensitivity: IrSensitivity, body: Box<IrStmt> },
    ForLoop  { init: Box<IrStmt>, cond: IrExpr, step: Box<IrStmt>, body: Box<IrStmt> },
    Repeat   { count: IrExpr, body: Box<IrStmt> },
    Forever  (Box<IrStmt>),
    Wait     { cond: IrExpr },
    LocalVar { name: String, ty: IrScalarType, init: Option<IrExpr> },
    Display  { format: String, args: Vec<IrExpr> },
    Finish,
    Empty,
}
```

`IrSensitivity` for `EventControl`:

```rust
pub enum IrSensitivity {
    Posedge(NodeId),
    Negedge(NodeId),
    Level(Vec<NodeId>),   // @(a or b or ...)
    Star,                 // @* / @(*)
}
```

#### `IrAnalogStmt` — analog behavioral code

Used in `IrAnalogBlock.statements`. These are the statements the JIT compiler directly lowers.

```rust
pub enum IrAnalogStmt {
    Contribution(IrContribution),
    IndirectContribution(IrIndirectContribution),
    IfElse { cond: IrExpr, then_: Vec<IrAnalogStmt>, else_: Vec<IrAnalogStmt> },
    Assign  { var: String, expr: IrExpr },
    LocalVar { name: String, ty: IrScalarType, init: Option<IrExpr> },
    Display  { format: String, args: Vec<IrExpr> },
}
```

| Variant | AMS syntax | Codegen treatment |
|---------|-----------|-------------------|
| `Contribution(Current{branch,expr})` | `I(a,b) <+ f(V)` | Stamps `f` into RHS; stamps `df/dV` into Jacobian |
| `Contribution(Voltage{branch,expr})` | `V(a,b) <+ f(...)` | Registered but not yet stamped in Jacobian |
| `IndirectContribution` | `V(a,b) : I(a,b) == f` | Registered but not yet in codegen |
| `IfElse` | `if (c) ... else ...` | Both branches visited unconditionally in codegen (conservative) |
| `Assign` | `x = expr` | Not yet emitted in JIT (local vars not threaded through) |
| `LocalVar` | `real x = ...` | Not yet emitted |
| `Display` | `$strobe(...)` | Ignored by codegen |

---

### 1.6 Instance Model

#### `AnalogIrInstance`

```rust
pub struct AnalogIrInstance {
    pub instance_name:  String,          // e.g. "top.u1.R1"
    pub model_name:     String,          // e.g. "res", "nmos"
    pub terminals:      Vec<PortBinding>,
    pub parameters:     Vec<(String, f64)>,
    pub str_parameters: Vec<(String, String)>,
    pub body:           AnalogBody,
}

pub enum AnalogBody {
    Source(IrAnalogBlock),   // → compile_analog_block() → JIT
    Osdi { path: PathBuf },  // pre-compiled .osdi (BSIM4, PSP, HiSIM …)
    Primitive,               // R, L, C, V, I, diode, MOSFET — solver built-in
}
```

`terminals` order matches the module port declaration order. The i-th `PortBinding` has `net_id` pointing to the connected net.

#### `DigitalIrInstance`

```rust
pub struct DigitalIrInstance {
    pub instance_name: String,
    pub model_name:    String,
    pub input_ports:   Vec<PortBinding>,
    pub output_ports:  Vec<PortBinding>,
    pub inout_ports:   Vec<PortBinding>,
    pub parameters:    HashMap<String, IrValue>,
    pub body:          DigitalBody,
}

pub enum DigitalBody {
    Source(IrDigitalBlock),  // → future digital codegen → DigitalDevice impl
    Primitive,               // AND, NAND, OR, NOR, XOR, NOT, BUF …
}
```

Bidirectional ports appear in both `input_ports` and `output_ports` AND in `inout_ports`. The digital simulator queries all three lists.

`IrDigitalBlock` holds `variables: Vec<IrVariable>`, `always_blocks: Vec<IrAlwaysBlock>`, and `initial_blocks: Vec<IrInitialBlock>`. Each `IrAlwaysBlock` has a `sensitivity` and `body: IrStmt`.

#### `ConnectIrInstance`

Auto-inserted at each analog↔digital port crossing during lowering.

```rust
pub struct ConnectIrInstance {
    pub instance_name: String,
    pub kind:          ConnectKind,
    pub analog_net:    NodeId,
    pub digital_net:   NodeId,
}

pub enum ConnectKind {
    A2D { threshold: f64, hysteresis: f64 },
    D2A { v_high: f64, v_low: f64, rise_time: f64 },
    Custom { model_name: String, parameters: HashMap<String, f64> },
}
```

`ConnectKind::default_a2d()` — threshold 0.9 V, zero hysteresis.  
`ConnectKind::default_d2a()` — v_high 1.8 V, v_low 0 V, rise_time 100 ps.

---

## 2. JIT Compilation Specification

### 2.1 Architecture

`IrAnalogBlock` → Cranelift IR → machine code in JIT memory. No subprocess, no file system, no shared library. Two `extern "C"` functions are compiled per module: `residual` and `jacobian`. Both are embedded in a `JITModule` that is kept alive by `JitAnalogDevice` for the duration of simulation.

Cranelift is a pure-Rust compiler backend (`cranelift_codegen`, `cranelift_frontend`, `cranelift_jit`, `cranelift_module`) — no LLVM dependency. It emits native code via SSA-form IR and produces output comparable to `-O1` C for float-heavy code.

All math functions (sin, cos, exp, etc.) are resolved to Rust wrapper functions in `libm_wrappers` (a module-private block) registered directly with the `JITBuilder`. These wrappers call Rust's `f64` methods (`x.sin()`, `x.exp()`, etc.), so no C libm is needed at link time.

### 2.2 Entry Point: `compile_analog_block`

Defined in `crates/piperine-lang/src/codegen/analog.rs`.

```rust
pub fn compile_analog_block(
    name:        &str,
    block:       &IrAnalogBlock,
    terminals:   &[PortBinding],
    param_names: &[String],
) -> Result<JitAnalogDevice, CodegenError>
```

Steps:

1. **Build `JITBuilder`** — register 17 math symbols from `libm_wrappers` (sin, cos, tan, asin, acos, atan, atan2, exp, log, log10, sqrt, pow, fabs, fmin, fmax, floor, ceil).

2. **Create `JITModule`** from the builder.

3. **Declare libm imports** — for each math function, declare a `Linkage::Import` function with the correct arity signature (`(F64) → F64` or `(F64, F64) → F64`). Returns `HashMap<&str, FuncId>`.

4. **Build index maps**:
   - `branch_index: HashMap<String, usize>` — branch name → position in `block.branches`
   - `param_index: HashMap<String, usize>` — param name → position in `param_names`

5. **Build `node_to_local`** — `HashMap<NodeId, usize>` mapping each terminal's `net_id` to its 0-based local index. GND is absent from this map (it has no local terminal index).

6. **Compute `voltage_array_size`** — `max(NodeId.0) + 1` across all branch endpoints and terminal `net_id`s. The caller must allocate at least this many `f64`s for the voltage array.

7. **Build `branch_endpoints`** — `HashMap<String, (Option<usize>, Option<usize>)>` mapping each branch name to its `(local_plus, local_minus)` indices. `None` means the node is GND — skip that stamp entry.

8. **Compile `residual` function** (`compile_residual`).

9. **Compile `jacobian` function** (`compile_jacobian`).

10. **`module.finalize_definitions()`** — commits machine code into executable memory.

11. **Extract function pointers** via `module.get_finalized_function()` + `mem::transmute`.

12. **Return `JitAnalogDevice`** — owns the `JITModule`, keeping code alive.

### 2.3 Function ABI

Both compiled functions share the same C signature:

```c
void fn(
    const double *node_voltages,  // indexed by NodeId.0; length >= voltage_array_size
    const double *params,         // indexed by parameter position; length >= num_params
    double       *output          // rhs[num_terminals] or jac[num_terminals²]
);
```

Caller responsibilities:
- `output` must be zero-initialized before each call.
- `node_voltages` must have `voltage_array_size` valid elements.
- `params` must have `num_params` valid elements.

The functions are purely functional — they accumulate into `output` and touch no other memory.

#### Residual function

For each `IrAnalogStmt::Contribution(IrContribution::Current { branch, expr })`:

```
i = emit_expr(expr, node_voltages, params)
if plus  != GND: output[local_plus]  += i
if minus != GND: output[local_minus] -= i
```

Output is indexed by **local terminal index** (0-based position in `terminals`), not by `NodeId`.

#### Jacobian function

For each `IrContribution::Current { branch, expr }`:

```
g = emit_expr(diff(expr, branch), node_voltages, params)
// Conductance stamp, row-major, size N×N where N = num_terminals:
if plus  != GND: jac[p*N + p] += g
if plus  != GND and minus != GND: jac[p*N + m] -= g
if minus != GND and plus  != GND: jac[m*N + p] -= g
if minus != GND: jac[m*N + m] += g
```

The derivative is taken symbolically with respect to the branch voltage name (not the node voltage). This works because branch voltages are defined as `V_plus - V_minus` — the Jacobian entries with respect to V_plus are the same as with respect to the branch voltage, and entries with respect to V_minus are negated.

### 2.4 Prologue: Branch Voltage Computation

At the start of both compiled functions, before any statement is processed, all branch voltages are pre-computed into a local `Value` array:

```
for each branch b at index i:
    vp = node_voltages[b.plus.0]
    vm = node_voltages[b.minus.0]
    branch_values[i] = vp - vm
```

This is the **only place `NodeId.0` is used as an array index**. Everywhere else — RHS stamping, Jacobian stamping, terminal references — uses local terminal indices.

GND (`NodeId(0)`) is valid here: `node_voltages[0]` is always 0.0, so `V_plus - 0 = V_plus` and `0 - V_minus = -V_minus` are correct.

### 2.5 Parameter Preload

Immediately after the prologue, all parameters are loaded from the `params` pointer into SSA `Value`s:

```
for i in 0..num_params:
    param_values[i] = params[i]
```

This is done once at function entry, not per-use.

### 2.6 Expression Emission: `emit_expr`

Defined in `crates/piperine-lang/src/codegen/expr.rs`. Recursively maps `IrExpr` → Cranelift `Value`:

| `IrExpr` variant | Cranelift output |
|------------------|-----------------|
| `Literal(v)` | `ins.f64const(v)` |
| `Var(name)` | `param_values[param_index[name]]`; 0.0 if unknown |
| `BranchVoltage(name)` | `branch_values[branch_index[name]]`; 0.0 if unknown |
| `BranchCurrent(_)` | `ins.f64const(0.0)` — not available in this context |
| `NodeVoltage(_)` | `ins.f64const(0.0)` — not available in this context |
| `BinaryOp{Add,...}` | `ins.fadd(l, r)` |
| `BinaryOp{Sub,...}` | `ins.fsub(l, r)` |
| `BinaryOp{Mul,...}` | `ins.fmul(l, r)` |
| `BinaryOp{Div,...}` | `ins.fdiv(l, r)` |
| `BinaryOp{Pow,...}` | `emit_libm2("pow", l, r)` |
| `BinaryOp{Eq/Ne/Lt/...}` | `ins.fcmp(cc, l, r)` → `ins.select(b, 1.0, 0.0)` |
| `BinaryOp{BitAnd/...}` | `ins.f64const(0.0)` |
| `UnaryOp{Neg,...}` | `ins.fneg(o)` |
| `UnaryOp{other}` | passthrough `o` |
| `AnalogFn{Sin,...}` | `emit_libm1("sin", a0)` |
| `AnalogFn{LimExp,...}` | `emit_libm2("fmin", a0, 80.0)` → `emit_libm1("exp", ...)` |
| `AnalogFn{Cross/Above/Timer/...}` | `ins.f64const(0.0)` |
| `Select{cond,then_,else_}` | `fcmp NotEqual (c, 0.0)` → `ins.select(b, t, e)` |

`emit_libm1` / `emit_libm2` look up the pre-declared `FuncRef` in the `libm` map and emit a call instruction.

### 2.7 Symbolic Differentiation: `autodiff::diff`

Defined in `crates/piperine-lang/src/codegen/autodiff.rs`.

```rust
pub fn diff(expr: &IrExpr, wrt: &str) -> IrExpr
```

`wrt` is a branch voltage name (e.g. `"(a, b)"`). Returns a new `IrExpr` representing `∂expr/∂V_branch`. The result is fed directly back through `emit_expr` — no separate IR pass.

Rules:

| Expression | Derivative |
|-----------|-----------|
| `Literal(_)` | `0` |
| `Var(name)` | `1` if `name == wrt`, else `0` |
| `BranchVoltage(name)` | `1` if `name == wrt`, else `0` |
| `BranchCurrent(_)`, `NodeVoltage(_)` | `0` |
| `Add(u, v)` | `u' + v'` |
| `Sub(u, v)` | `u' - v'` |
| `Mul(u, v)` | `u'v + uv'` |
| `Div(u, v)` | `(u'v - uv') / v²` |
| `Pow(u, v)` | `v * u^(v-1) * u'` |
| Comparisons | `0` (piecewise constant) |
| `Neg(u)` | `-u'` |
| `Exp(u)` | `exp(u) * u'` |
| `Ln(u)` | `u' / u` |
| `Log(u)` | `u' / (u * ln10)` |
| `Sqrt(u)` | `u' / (2*sqrt(u))` |
| `Sin(u)` | `cos(u) * u'` |
| `Cos(u)` | `-sin(u) * u'` |
| `Tan(u)` | `u' / cos²(u)` |
| `Asin(u)` | `u' / sqrt(1 - u²)` |
| `Acos(u)` | `-u' / sqrt(1 - u²)` |
| `Atan(u)` | `u' / (1 + u²)` |
| `Atan2(y,x)` | `0` |
| `Abs(u)` | `(u >= 0 ? 1 : -1) * u'` |
| `Pow(u,v)` (AnalogFn) | `v * u^(v-1) * u'` |
| `LimExp(u)` | `exp(min(u,80)) * u'` |
| `Floor`, `Ceil`, `Min`, `Max` | `0` a.e. |
| `Cross`, `Above`, `Timer`, `WhiteNoise`, `FlickerNoise` | `0` |
| `Select{cond,t,e}` | `Select{cond, diff(t), diff(e)}` |

**Constant folding smart constructors** are applied immediately during diff construction:
- `0 + x → x`, `x + 0 → x`
- `x - 0 → x`
- `0 * x → 0`, `x * 0 → 0`, `1 * x → x`, `x * 1 → x`
- `neg(Literal(v)) → Literal(-v)`

These prevent exponential blowup in the expression tree for models with many terms.

### 2.8 Compiled Output Types

```rust
pub struct CompiledAnalogBlock {
    pub residual:           unsafe extern "C" fn(*const f64, *const f64, *mut f64),
    pub jacobian:           unsafe extern "C" fn(*const f64, *const f64, *mut f64),
    pub num_terminals:      usize,
    pub num_params:         usize,
    pub voltage_array_size: usize,
    _module:                JITModule,   // keeps machine code alive
}

pub struct JitAnalogDevice {
    pub name:        String,
    pub param_names: Vec<String>,
    pub compiled:    CompiledAnalogBlock,
}
```

`JITModule` is `!Send + !Sync` by default. After `finalize_definitions()`, no further mutation occurs, so `unsafe impl Send for JitAnalogDevice` and `unsafe impl Sync for JitAnalogDevice` are applied. The device is shared across threads via `Arc<JitAnalogDevice>`.

High-level evaluation methods on `JitAnalogDevice`:

```rust
fn eval_residual(&self, node_voltages: &[f64], params: &[f64], rhs:  &mut [f64])
fn eval_jacobian(&self, node_voltages: &[f64], params: &[f64], jac:  &mut [f64])
```

Both require `node_voltages.len() >= voltage_array_size` and zero-initialized output slices.

### 2.9 Solver Integration: `JitDevice`

Defined in `crates/piperine-solver/src/analog/jit_device.rs`.

```rust
pub struct JitDevice {
    pub jit:          Arc<JitAnalogDevice>,
    pub terminal_ids: Vec<NodeId>,   // NodeId of each terminal in port order
}
```

`terminal_ids` is the key bridge between the solver's `NodeIdentifier` world and the JIT's `NodeId` world.

`JitDevice` implements `AnalogDevice`:

| `AnalogDevice` method | `JitDevice` behavior |
|----------------------|----------------------|
| `name()` | `jit.name` |
| `num_nodes()` | `jit.num_terminals()` |
| `num_terminals()` | `jit.num_terminals()` |
| `num_states()` | 0 |
| `num_resistive_jacobian_entries()` | `N²` (dense) |
| `num_reactive_jacobian_entries()` | 0 |
| `setup_model()` | resize `model.params` to `num_params`, fill with 0 |
| `setup_instance()` | resize `instance.residual` to N, `instance.jacobian` to N² |
| `allocate_nodes()` | `netlist.connect_node()` per terminal → `node_refs` |
| `bind_nodes()` | no-op |
| `set_params()` | fill `model.params` by name lookup in `param_names` |
| `eval()` | call `eval_residual` + `eval_jacobian`, cache in `instance` |
| `load_residual_resist()` | copy `instance.residual` into `rhs` |
| `load_jacobian_resist()` | copy `instance.jacobian` into `jacobian` |
| `load_residual_react()` | no-op |
| `load_jacobian_react()` | no-op |
| `bound_step_hint()` | `f64::INFINITY` |
| `read_opvars()` | `[]` |
| `num_noise_sources()` | 0 |
| `load_noise()` | no-op |

**`build_prev_solve()`** is the NodeId bridge:

```rust
fn build_prev_solve(&self, instance, node_refs, state_fn) -> [f64; SCRATCH] {
    let mut prev_solve = [0.0; SCRATCH];
    for (i, net_id) in self.terminal_ids.iter().enumerate() {
        let node_idx = net_id.0 as usize;   // ← NodeId used as array index
        if node_idx < SCRATCH {
            if let Some(cref) = &node_refs[i] {
                if let Some(k) = cref.idx() {
                    prev_solve[node_idx] = state_fn(k);
                }
            }
        }
    }
    // GND stays 0.0
    prev_solve
}
```

This populates a `NodeId`-indexed voltage array from the solver's local circuit state — the input format that the JIT-compiled functions expect.

**`load_spice_rhs_dc()`** — for each terminal `i`: `rhs[i] = Σ_j jac[i*N+j]*v[j] - res[i]`, where `v[j]` is the local terminal voltage extracted from `prev_solve` via `terminal_ids`.

### 2.10 `CircuitInstance::from_design`

Defined in `crates/piperine-solver/src/circuit.rs`.

```rust
pub fn from_design(design: &IrDesign) -> Result<Self, String>
```

Steps:

1. **Allocate nodes** — iterate `design.nets`, map each `IrNet` to a `NodeIdentifier`:
   - net named `"gnd"` or `"GND"` or `NodeId::GND` → `NodeIdentifier::Gnd`
   - all others → `NodeIdentifier::Anonymous(atomic_counter++)`
   - Store in `node_id_map: HashMap<NodeId, NodeIdentifier>`.

2. **JIT compile and instantiate** — for each `AnalogIrInstance` where `body == AnalogBody::Source(block)`:
   - Call `compile_analog_block(model_name, block, &terminals, &param_names)`
   - Build `terminal_ids: Vec<NodeId>` from `inst.terminals`
   - Build `terminals: Vec<NodeIdentifier>` via `node_id_map`
   - Construct `JitDevice::new(Arc::new(jit), terminal_ids)`
   - Call `device.allocate_nodes()` to register in `Netlist`
   - Wrap in `DeviceRuntime::new(...)` → push to `runtimes`

3. **Skip other body variants** — `AnalogBody::Osdi` and `AnalogBody::Primitive` are `continue`d with no panic. Osdi support can be added later; primitives are handled by a separate path.

4. **Return `CircuitInstance`** — `runtimes` holds all JIT devices; `digital_runtimes`, `digital_topology`, `digital_state` are empty/default; `netlist` is fully allocated.

---

## 3. `FrontendLower` Contract

Defined in `crates/piperine-lang/src/lowering/mod.rs`.

```rust
pub trait FrontendLower {
    type Error: std::error::Error;
    fn lower(&self, top_module: &str) -> Result<IrDesign, Self::Error>;
}
```

### What an implementor must produce

An `IrDesign` satisfying all IR invariants from §1.1:

- `design.nets` must contain `NodeId::GND` with `domain: Analog`, `name: "gnd"`.
- Every `PortBinding.net_id` in every instance must be in `design.nets`.
- Every branch name in any `IrAnalogStmt::Contribution` must appear in the instance's `IrAnalogBlock.branches`.
- All numeric parameters in `AnalogIrInstance.parameters` must be `f64` — no strings for numeric values.
- All names globally unique — use hierarchical dot-paths for scoping.

### Verilog-AMS lowering (`Document` implementation)

Implemented in `crates/piperine-parser/src/lower.rs` via `struct Elaborator<'d>`.

**Elaboration order:**
1. Build `discipline_domains` from all `doc.disciplines`.
2. Insert GND net at `NodeId(0)`.
3. Lower all disciplines and natures into `design.disciplines` / `design.natures`.
4. Find the top-level module by name.
5. Allocate nets for all ports and net-declarations in the module (`node_counter` starts at 1).
6. Build `net_map: HashMap<String, NodeId>` for the current scope.
7. Build `branch_map` from explicit `branch` declarations + implicit branch registration.
8. Lower all `analog` blocks in the module → `IrAnalogBlock` → push `AnalogIrInstance`.
9. Recurse into sub-instances: resolve connections, fold parameters, repeat.

**Implicit branch registration** (`register_implicit_branch`): when a contribution statement `I(a,b) <+ expr` appears with `(a,b)` not explicitly declared as a branch, one is created with `name = "(a, b)"`, `plus = net_map["a"]`, `minus = net_map["b"]`.

**Discipline → domain**: `"electrical"` → `Domain::Analog`, `"logic"` / `"discrete"` → `Domain::Digital`, `"wreal"` → `Domain::Wreal`. Unknown disciplines default to `Domain::Analog`.

**Parameter folding** (`eval_const_expr`): evaluates constant expressions including SI suffix handling (`k=1e3`, `m=1e-3`, `u=1e-6`, `n=1e-9`, `p=1e-12`, `f=1e-15`).

---

## 4. Invariants and Contracts Summary

### IR invariants (enforced post-lowering)

| Invariant | Where checked |
|-----------|--------------|
| `NodeId::GND` in `design.nets` | `Elaborator::new()` inserts it unconditionally |
| All branch endpoint `NodeId`s exist in `design.nets` | Lowering uses `net_map` to look up names |
| All contribution branch names in `IrAnalogBlock.branches` | `register_implicit_branch()` ensures this |
| No symbolic params in `AnalogIrInstance.parameters` | `eval_const_expr` folds all constants |
| Globally unique instance names | Elaborator prefixes with `instance_name.` on recursion |

### JIT invariants

| Invariant | Contract |
|-----------|----------|
| `output` zero-initialized before call | Caller (JitDevice::eval) allocates `vec![0.0; N]` |
| `node_voltages.len() >= voltage_array_size` | `build_prev_solve` allocates `[f64; SCRATCH]` with `SCRATCH >= voltage_array_size` |
| `params.len() >= num_params` | `set_params` ensures `model.params.len() == num_params` |
| `JitAnalogDevice` valid for simulation lifetime | Held in `Arc` inside `JitDevice` |
| Local terminal index i maps to `terminals[i].net_id` | Established in `compile_analog_block`, matches `JitDevice.terminal_ids` |

---

## 5. Extension Points

### Adding a new `IrAnalogFn`

1. Add variant to `IrAnalogFn` in `crates/piperine-lang/src/ir/expr.rs`.
2. Add Cranelift emission in `emit_analog_fn()` in `crates/piperine-lang/src/codegen/expr.rs`.
3. Add derivative rule in `autodiff::diff` in `crates/piperine-lang/src/codegen/autodiff.rs`.
4. Add AMS name mapping in `lower.rs` `builtin_fn()` if the function has a Verilog-AMS name.

### Adding a new `IrBinOp`

1. Add variant to `IrBinOp` in `ir/expr.rs`.
2. Add Cranelift instruction mapping in `emit_expr`'s `BinaryOp` match in `codegen/expr.rs`.
3. Add derivative case in `autodiff::diff`.

### Voltage-mode contributions

`IrContribution::Voltage { branch, expr }` is currently registered but not stamped. To implement:
- In `emit_residual_stmt`: compute `v = emit_expr(expr)`, stamp `rhs[p] += v`, `rhs[m] -= v`.
- In `emit_jacobian_stmt`: differentiate w.r.t. branch voltage → KVL stamp entries.

### Indirect contributions

`IrIndirectContribution` is currently a no-op in codegen. To implement, it requires an implicit internal node allocation and an additional KCL row.

### OSDI in `from_design`

`AnalogBody::Osdi { path }` — load `.osdi` via `OsdiLib::load(path)` → construct `OsdiDevice` → wrap in `DeviceRuntime` → push to `runtimes`. The OSDI path is already implemented for the `Circuit::instantiate()` path.

### Digital codegen

`DigitalBody::Source(IrDigitalBlock)` — compile `IrDigitalBlock` (always/initial blocks as `IrStmt`) into a Rust struct implementing `DigitalDevice`. The `eval()` method would process `IrStmt::EventControl` blocks on each `DigitalTopology` evaluation pass.

---

## 6. Known Limitations

| Limitation | Impact | Location |
|-----------|--------|----------|
| `V(a,b) <+ expr` not stamped in Jacobian | Voltage sources without explicit conductance may not converge | `codegen/analog.rs:emit_jacobian_stmt` |
| `IndirectContribution` no-op in codegen | `V: I == expr` constraints silently ignored | `codegen/analog.rs` |
| `$temperature` not wired to params | Temperature-dependent models use constant 300.15 K | `jit_device.rs::set_params` |
| `WhiteNoise`/`FlickerNoise` → 0 | JIT path is noiseless; noise analysis needs a separate stamp pass | `codegen/expr.rs:emit_analog_fn` |
| `AnalogBody::Primitive` skipped in `from_design` | R/L/C/V/I from IR design not instantiated | `circuit.rs:from_design` |
| `IfElse` branches visited unconditionally in codegen | Both branches always stamp; wrong for `if (param > 0)` gating | `codegen/analog.rs:emit_residual_stmt` |
| `Assign` / `LocalVar` in analog blocks not emitted | Local variable state not threaded through compiled functions | `codegen/analog.rs` |
| `BranchCurrent` / `NodeVoltage` in expressions → 0 | Probes inside math expressions return 0 at JIT level | `codegen/expr.rs` |
| `voltage_array_size` bounded by `SCRATCH` | NodeIds must fit in `SCRATCH`-element array in `build_prev_solve` | `jit_device.rs` |
