# Lowering Design to IrProgram

The lowering module in `src/lowering/` converts an elaborated `Design` (POM) into an `IrProgram` (piperine-codegen IR). This is the bridge between the PHDL frontend and the central IR consumed by the codegen crate.

---

## Entry Point: `ppr_to_ir`

```rust
pub fn ppr_to_ir(prog: &Design) -> IrProgram
```

The main entry point performs three passes:

1. **Structure conversion**: Each POM `Module` is converted to an `IrModule` via `convert_mod()` (`structure.rs`).
2. **Behavior attachment**: For each module, its `analog` and `digital` behavior blocks are lowered via `lower_stmts()` and attached as `IrAnalogBody` or `IrDigitalBody`.
3. **Function conversion**: Global functions are converted via `convert_fn()`.

The resulting `IrProgram` has `source: "ppr"` to distinguish it from Verilog-AMS–sourced IR.

---

## LowerCtx

`LowerCtx` is the mutable context carried through all lowering passes.

```rust
struct LowerCtx {
    env: HashMap<String, IrExpr>,      // identifier → expression bindings
    state_vars: Vec<IrStateVar>,        // allocated state variables
    noise_sources: Vec<IrNoiseSource>,  // extracted noise sources
    counter: u32,                       // unique state-variable ID counter
    is_digital: bool,                   // set true while lowering a digital block
}
```

### `alloc_state(kind: IrStateKind, arg: IrExpr) -> u32`

Allocates a unique state variable with the given kind and argument expression. Returns the state variable ID. The `counter` field is incremented on each allocation.

---

## Structure Conversion (`structure.rs`)

### `convert_mod(m: &Module) -> IrModule`

Maps each POM `Module` field to its IR equivalent:

| POM field | IR field | Transformation |
|-----------|----------|----------------|
| `ports` | `ports: Vec<IrPort>` | Direction mapped to `IrDirection`; discipline extracted via `discipline_name()` |
| `params` | `params: Vec<IrParam>` | Type mapped via `elab_value_type_to_ir()`; default mapped via `const_val_to_ir()` |
| `wires` | `wires: Vec<IrWire>` | Discipline extracted via `discipline_name()` |
| `instances` | `instances: Vec<IrInstance>` | Ports become `connection` strings; params lowered |
| `connections` | `connections: Vec<IrConnectionDecl>` | Net alias pairs |

### `discipline_name(ty: &NetType) -> Option<String>`

Extracts the base discipline name from a `NetType`, recursively unwrapping `Array` wrappers. Returns `None` only if the type is not a discipline.

### `elab_value_type_to_ir(ty: &ValueType) -> IrType`

Maps POM value types to IR types:

| POM ValueType | IrType |
|---------------|--------|
| `Real`, `Natural` | `Real` |
| `Integer` | `Integer` |
| `Complex` | `Complex` |
| `Boolean` | `Bool` |
| `Quad` | `Quad` |
| `Str` | `String` |
| `Enum(_)` | `Integer` |
| `Array(inner, _)` | Recurse into inner |
| `FnPtr(_, _)` | `Void` |

### `const_val_to_ir(v: &ConstVal) -> IrExpr`

Converts compile-time constant values to IR expressions: `Real` → `IrExpr::Real`, `Nat`/`Int` → `IrExpr::Int`, `Bool` → `IrExpr::Bool`, `Str` → `IrExpr::String`.

### `convert_fn(f: &Function) -> IrFunction`

Converts a POM `Function` to `IrFunction` by extracting parameter names and lowering the body statements via `lower_stmts()`.

---

## Expression Lowering (`expr.rs`)

### `lower_expr(expr: &Expr, ctx: &mut LowerCtx) -> IrExpr`

The main expression lowerer. Handles all parse-AST `Expr` variants:

| Expr variant | IR result |
|-------------|-----------|
| `Literal::Real(f)` | `IrExpr::Real(f)` |
| `Literal::Int(n)` | `IrExpr::Int(n)` |
| `Literal::Bool(b)` | `IrExpr::Bool(b)` |
| `Literal::String(s)` | `IrExpr::String(s)` |
| `Literal::Quad(s)` | `IrExpr::Quad(val)` (parsed 0q-notation) |
| `Ident(name)` | Looked up in env, or `IrExpr::Param(name)` |
| `Path(p)` | Joined segments looked up in env, or `IrExpr::Param` |
| `Unary(Neg, inner)` | `IrExpr::Unary(Neg, ...)` |
| `Unary(Not, inner)` | `IrExpr::Unary(Not, ...)` |
| `Binary(lhs, op, rhs)` | `IrExpr::Binary(lower_binop(op), ...)` |
| `Call(func, args)` | Delegates to `lower_call()` |
| `SysCall(name, args)` | Delegates to `lower_syscall()` |
| `If { cond, then, else }` | `IrExpr::Select(cond, then, else)` |
| `Block(b)` | Evaluates via `block_value()` |
| `Index(base, idx)` | `IrExpr::Index(base, idx)` |
| `Slice(base, range)` | `IrExpr::Slice(base, range)` |
| `Field(base, field)` | Flattened: `a.field` → `IrExpr::Param("a_field")` |
| `Array(body)` | Delegates to `lower_array()` |
| `BundleLit`, `Lambda` | `IrExpr::Real(0.0)` (unsupported in analog scalar context) |

### `lower_call(func, args, ctx) -> IrExpr`

Handles function calls and analog operators:

| Function | Behavior |
|----------|----------|
| `V(n1, n2)` / `I(n1, n2)` | `IrExpr::BranchAccess` with Potential/Flow nature and plus/minus nodes |
| `ddt(expr)` | Allocates `IrStateKind::Ddt` state variable, returns `IrExpr::StateRef(id)` |
| `idt(expr, ic?)` | Allocates `IrStateKind::Idt { ic }` state variable |
| `idtmod(expr, ic?, modulus?)` | Allocates `IrStateKind::IdtMod { ic, modulus }` state variable |
| `ddx(expr, node)` | Allocates `IrStateKind::Ddx { node }` state variable |
| `delay(expr, delay?)` / `absdelay(expr, delay?)` | Allocates `IrStateKind::Delay { delay }` state variable |
| `transition(expr, delay?, rise?, fall?, tol?)` | Allocates `IrStateKind::Transition { delay, rise, fall, tol }` state variable |
| `slew(expr, rise?, fall?)` | Allocates `IrStateKind::Slew { rise, fall }` state variable |
| `laplace_*(expr, num, den)` | Allocates `IrStateKind::Laplace { variant, num, den }` state variable |
| `zi_*(expr, num, den, sample_dt)` | Allocates `IrStateKind::ZTransform { variant, num, den, sample_dt }` state variable |
| `ac_stim(mag?, phase?)` | Returns `IrExpr::AcStim { mag, phase }` |
| `white_noise(...)` / `flicker_noise(...)` | Returns `IrExpr::Real(0.0)` — noise tracked separately by `scan_noise()` |
| `analysis(kind)` | Returns `IrExpr::Sim(SimQuery::Analysis(kind))` |
| Any other | `IrExpr::Call(name, args)` — user-defined function call |

### `lower_syscall(name, args, ctx) -> IrExpr`

Handles system queries (`$`-prefixed calls):

| Syscall | IR result |
|---------|-----------|
| `$temperature` | `SimQuery::Temperature` |
| `$vt(args?)` | `SimQuery::Vt(optional_arg)` |
| `$abstime` | `SimQuery::Abstime` |
| `$mfactor` | `SimQuery::Mfactor` |
| `$xposition` | `SimQuery::XPosition` |
| `$yposition` | `SimQuery::YPosition` |
| `$angle` | `SimQuery::Angle` |
| `$simparam(key, default)` | `SimQuery::Simparam { key, default }` |
| `$param_given(name)` | `SimQuery::ParamGiven(name)` |
| `$port_connected(name)` | `SimQuery::PortConnected(name)` |
| `$limit(kind, args...)` | `SimQuery::Limit { kind, args }` |
| `$random` | `SimQuery::Random { kind: "random", args: [] }` |
| `$dist_*` | `SimQuery::Random { kind, args }` |
| `$analysis` | `SimQuery::Analysis(kind)` |
| `$discontinuity` | Falls through to `IrExpr::Real(0.0)` (handled as a statement) |

### `parse_contrib_dest(dest: &Expr) -> (IrNature, String, String)`

Parses a contribution left-hand side `V(n1, n2)` or `I(n1, n2)` into a nature and two node names. Falls back to `(Flow("I"), "?", "?")` if the LHS is not a recognized access call.

### `access_to_nature(name: &str) -> IrNature`

Maps access function names to natures: `"V"` → `Potential`, `"I"` → `Flow`.

### Noise Extraction

`scan_noise(expr, plus, minus, ctx)` walks a contribution's RHS looking for `white_noise` and `flicker_noise` calls. Found noise sources are collected as `IrNoiseSource` with the contribution's plus/minus node pair. The noise call expressions themselves return `IrExpr::Real(0.0)` in expression position.

### Array Lowering

`lower_array(body, ctx)` handles three array literal forms:
- **List**: `[a, b, c]` → `IrExpr::Array(elems)`
- **Repeat**: `[val; n]` → `IrExpr::ArrayRepeat(val, n)`
- **Comprehension**: `[expr for var in range]` — attempted const-bounds unrolling; falls back to empty array if bounds are non-constant.

### `block_value(block, ctx) -> IrExpr`

Processes a block expression: executes `VarDecl` side effects for variable bindings, then evaluates the trailing expression or the last `Expr` statement.

---

## Statement Lowering (`stmt.rs`)

### `lower_stmts(stmts, ctx) -> Vec<IrStmt>`

Top-level statement lowering loop. Delegates each `BehaviorStmt` to `lower_stmt()` and flattens the results.

### `lower_stmt(stmt, ctx) -> Vec<IrStmt>`

Converts each `BehaviorStmt` variant to zero or more `IrStmt`:

| BehaviorStmt | Lowered result |
|--------------|----------------|
| `VarDecl { name, default: Some(e) }` | Stores lowered expression in env, returns `[]` |
| `VarDecl { name, default: None }` | Stores `IrExpr::Real(0.0)` in env, returns `[]` |
| `Bind { dest: Ident(name), op: Assign, src }` | Updates env with lowered src, returns `[]` |
| `Bind { dest, op: Contrib, src }` | Parses `V(n1,n2)`/`I(n1,n2)`, scans noise, lowers rhs. Determines `Reactive` (has state ref) or `Resistive` contribution kind. Returns `[IrStmt::Contrib]` |
| `Bind { dest, op: Force, src }` | Dual semantics: `IrStmt::Force` in analog blocks, `IrStmt::Assign` in digital blocks (controlled by `ctx.is_digital`) |
| `If { cond, then_body, else_body }` | Creates `IrStmt::If` with branch phi-node merging |
| `Match { expr, arms }` | Desugared into if/else-if chain via `lower_match()` |
| `Event { spec, guard, body }` | Event spec converted via `convert_event_spec()`, body lowered, optional guard wrapped in `IrStmt::If` |
| `Diagnostic { sys, args }` | System tasks: `$display`/`$write`/`$strobe`/`$monitor`/`$error`/`$fatal`/`$warning`/`$info` → `IrStmt::Diagnostic`. Special tasks: `$bound_step` → `IrStmt::BoundStep`, `$finish`/`$stop` → `IrStmt::Finish`, `$discontinuity` → `IrStmt::Discontinuity` |
| `Expr(e)` | Expression statement lowered via `lower_expr_stmt()` (handles syscall-side-effect stmts) |
| `Return(e)` | `IrStmt::Return(Some(e))` — preserved for fn body inlining per GAPS D.5 |

### `lower_match(expr, arms, ctx) -> Vec<IrStmt>`

Desugars a `match` expression into an if/else-if chain. Each `Pattern::Path` arm becomes an equality check (`cond == pat`). `Pattern::Wildcard` becomes the trailing else body. Builds the chain inside-out from the last arm.

### Phi-Node Merge: `merge_branch_ctx(pre_env, then_ctx, else_ctx, cond, ctx)`

After an if/else, variables modified in branches are merged into `IrExpr::Select(cond, then_val, else_val)`. State variables and noise sources from both branches are collected. The counter is advanced past both branches' maximum.

---

## Event Spec Conversion (`event.rs`)

### `convert_event_spec(spec, ctx) -> Vec<IrEventKind>`

Maps PHDL `EventSpec` to `IrEventKind` vectors:

| EventSpec | IrEventKind |
|-----------|-------------|
| `Initial` | `[InitialStep]` |
| `Final` | `[FinalStep]` |
| `Named { "cross", arg }` | `[Cross { dir: 0, expr: Some(arg) }]` |
| `Named { "above", arg }` | `[Above { expr: Some(arg) }]` |
| `Named { "timer", arg }` | `[Timer { period: Some(arg) }]` |
| `Named { "posedge", arg }` | `[Posedge(arg)]` |
| `Named { "negedge", arg }` | `[Negedge(arg)]` |
| `Named { "change", arg }` | `[Change(arg)]` |
| `Or(specs)` | Flat-map over sub-specs |
| Any other named | `[InitialStep]` (unknown event) |
