# Shared IR Design

Both Piperine's native language (`.ppr`) and Verilog-AMS (`.va` / `.vams`) compile to a single `IrDesign`. Downstream codegen — Cranelift JIT for analog, tree-walking interpreter for digital, noise solver — sees only `IrDesign` and knows nothing about which frontend produced it.

---

## 1. Pipeline

```
.ppr source               .va / .vams source
      │                         │
      ▼                         ▼
 piperine-lang             piperine-ams
  parse::ast                model::Document
      │                         │
      ▼                         ▼
 PprLower::lower()         AmsLower::lower()
      │                         │
      └─────────┬───────────────┘
                ▼
           IrDesign
        [piperine-lang/src/elab/]
                │
       ┌────────┼────────────────────┐
       ▼        ▼                    ▼
  Cranelift  DigitalInterpreter   noise solver
  JIT        (tree-walking)       (AC analysis)
  [analog]   [digital]
       │        │                    │
       └────────┴────────────────────┘
                ▼
         piperine-solver
         Newton-Raphson DC/Tran/AC
```

---

## 2. Crate Responsibilities

| Crate | Role |
|-------|------|
| `piperine-lang` | `.ppr` parser + `ElabProgram` IR + Cranelift JIT + digital interpreter + `FrontendLower` trait |
| `piperine-ams` | Verilog-AMS parser + `Document` model |
| `piperine-solver` | Newton-Raphson DC/Tran/AC; digital event scheduler; noise analysis |
| `piperine-ngspice` | ngspice hardware definitions and backend |
| `piperine-cli` | Orchestrates frontend → IR → solver |

`AmsLower` lives in `piperine-ams/src/lower.rs` but depends on `piperine-lang` types via a feature flag or separate crate (`piperine-ams-lower`). This keeps `piperine-ams` usable as a standalone VA reader without pulling in Cranelift.

---

## 3. The Real IR: `ElabProgram`

The actual IR produced today is **`ElabProgram`** (in `piperine-lang/src/elab/ir.rs`), not a separate `IrDesign` type. Both frontends lower to `ElabProgram`.

Key types:

```rust
pub struct ElabProgram {
    pub modules:     HashMap<String, ElabMod>,
    pub behaviors:   Vec<ElabBehavior>,      // analog + digital blocks
    pub disciplines: HashMap<String, DisciplineDecl>,
    pub enums:       HashMap<String, EnumDecl>,
    pub functions:   HashMap<String, ElabFn>,
    pub impls:       Vec<ElabImpl>,
}

pub struct ElabBehavior {
    pub name: String,
    pub kind: BehaviorKind,       // Analog | Digital
    pub body: Vec<ElabBehaviorStmt>,
}

pub enum ElabBehaviorStmt {
    VarDecl { name: String, ty: ElabValueType, default: Option<Expr> },
    Bind    { dest: Expr, op: BindOp, src: Expr },
    If      { cond: Expr, then_body: Vec<ElabBehaviorStmt>, else_body: Option<Vec<ElabBehaviorStmt>> },
    Match   { expr: Expr, arms: Vec<ElabMatchArm> },
    Event   { spec: EventSpec, guard: Option<Expr>, body: Vec<ElabBehaviorStmt> },
    Diagnostic { sys: String, args: Vec<Expr> },
    Expr    (Expr),
}
```

`Bind` with `BindOp::Contrib` is the contribution statement (`I(p,n) <+ expr` or `V(p,n) <+ expr`).

---

## 4. Known Bugs in Current Analog Codegen

### 4.1 `if` flattening (critical)

`extract_contributions` in `analog.rs` walks `ElabBehaviorStmt::If` and unconditionally collects contributions from **both** branches:

```rust
ElabBehaviorStmt::If { then_body, else_body, .. } => {
    for s in then_body { extract_from_stmt(s, out); }    // ← always
    if let Some(eb) = else_body { for s in eb { extract_from_stmt(s, out); } }  // ← always
}
```

Effect: `if (cond) { I(p,n) <+ a; } else { I(p,n) <+ b; }` produces two unconditional contributions, stamping `a + b` always. Residual and Jacobian are both wrong.

### 4.2 Local variables dropped (critical)

`VarDecl` and `Assign`-type `Bind` statements are not matched in `extract_from_stmt`. When a contribution references a local variable, `emit_phdl_expr` sees `Ident("k")`, finds it in neither `param_values` nor `branch_voltages`, and returns 0.0. Example:

```
real k = V(p,n) / r;
I(p,n) <+ k;    →   compiled as   I(p,n) <+ 0.0
```

### 4.3 Autodiff of local variables broken

`diff(Ident("k"), wrt)` always returns 0 (identifiers are treated as constants). If `k = V(p,n)/r`, the Jacobian entry for `I(p,n) <+ k` is 0 instead of `1/r`.

### 4.4 No Cranelift basic blocks

The current codegen emits a single linear block. Any `if` in the contribution tree requires branch/merge blocks in Cranelift SSA form. The current architecture has no mechanism for this.

### AMS impact

These bugs affect AMS lowering identically. `if` is universal across VA models — diodes, MOSFETs, BJTs, and virtually every model use conditional contributions. `if` inside AMS analog blocks (`IfStmt`) maps directly to `ElabBehaviorStmt::If` after lowering, and hits all four bugs above.

---

## 5. Correct Analog Lowering Architecture

### 5.1 Two-pass strategy

Instead of extracting a flat list of contributions and then compiling them, compile the **entire statement tree** into Cranelift in one pass. The key insight: Cranelift's `FunctionBuilder` is a structured builder that manages basic blocks and SSA values — use it for control flow too.

**Pass 1 — inline local variables (before codegen)**

Walk the `ElabBehaviorStmt` tree and substitute `VarDecl` definitions inline into all subsequent uses. This produces a statement tree where every `Bind { Contrib, src }` has a self-contained `src` expression containing no free local variables.

Why this before codegen: autodiff (`diff`) works purely on `Expr` trees. If local variables are not inlined, `diff(Ident("k"), wrt)` = 0 always. After inlining, `k` disappears from the contribution expr, and autodiff works correctly.

Inlining algorithm:

```rust
fn inline_vars(stmts: &[ElabBehaviorStmt]) -> Vec<InlinedStmt> {
    let mut env: HashMap<String, Expr> = HashMap::new();
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            ElabBehaviorStmt::VarDecl { name, default: Some(expr), .. } => {
                let inlined = substitute(expr, &env);
                env.insert(name.clone(), inlined);   // record for downstream
            }
            ElabBehaviorStmt::Bind { dest, op: BindOp::Assign, src } => {
                // Reassignment to local var: update env
                if let Expr::Ident(name) = dest {
                    let inlined = substitute(src, &env);
                    env.insert(name.clone(), inlined);
                }
            }
            ElabBehaviorStmt::Bind { dest, op: BindOp::Contrib, src } => {
                out.push(InlinedStmt::Contrib {
                    dest: dest.clone(),
                    src: substitute(src, &env),   // ← all local vars replaced
                });
            }
            ElabBehaviorStmt::If { cond, then_body, else_body } => {
                out.push(InlinedStmt::If {
                    cond: substitute(cond, &env),
                    then_: inline_vars(then_body),   // recurse, fresh env copy
                    else_: else_body.as_ref().map(|b| inline_vars(b)),
                });
            }
            _ => {}   // Diagnostic, Event, etc.
        }
    }
    out
}

fn substitute(expr: &Expr, env: &HashMap<String, Expr>) -> Expr {
    // Walk expr tree, replace Ident(name) with env[name] if present
    ...
}
```

**Result**: an `InlinedStmt` tree:

```rust
enum InlinedStmt {
    Contrib { plus: String, minus: String, expr: Expr },   // expr has no free local vars
    If      { cond: Expr, then_: Vec<InlinedStmt>, else_: Vec<InlinedStmt> },
    Case    { discriminant: Expr, arms: Vec<(Expr, Vec<InlinedStmt>)>, default: Vec<InlinedStmt> },
    // For/While only allowed if they contain no contributions (checked at lowering)
}
```

**Pass 2 — emit Cranelift with basic blocks**

Walk `InlinedStmt` recursively, building Cranelift IR:

```rust
fn emit_stmts_residual(
    stmts: &[InlinedStmt],
    builder: &mut FunctionBuilder,
    ctx: &mut EmitCtx,   // branch_voltages, param_values, libm, rhs_ptr, num_terminals
) {
    for stmt in stmts {
        match stmt {
            InlinedStmt::Contrib { plus, minus, expr } => {
                let current = emit_expr(builder, ctx, expr);
                if let Some(p) = ctx.port_index.get(plus) {
                    accumulate_f64(builder, current, ctx.rhs_ptr, *p);
                }
                if let Some(m) = ctx.port_index.get(minus) {
                    let neg = builder.ins().fneg(current);
                    accumulate_f64(builder, neg, ctx.rhs_ptr, *m);
                }
            }
            InlinedStmt::If { cond, then_, else_ } => {
                emit_if_residual(builder, ctx, cond, then_, else_);
            }
            InlinedStmt::Case { discriminant, arms, default } => {
                emit_case_residual(builder, ctx, discriminant, arms, default);
            }
        }
    }
}

fn emit_if_residual(
    builder: &mut FunctionBuilder,
    ctx: &mut EmitCtx,
    cond: &Expr,
    then_: &[InlinedStmt],
    else_: &[InlinedStmt],
) {
    let cond_val = emit_expr(builder, ctx, cond);
    let zero = builder.ins().f64const(0.0);
    let is_true = builder.ins().fcmp(FloatCC::NotEqual, cond_val, zero);

    let then_block  = builder.create_block();
    let else_block  = builder.create_block();
    let merge_block = builder.create_block();

    builder.ins().brif(is_true, then_block, &[], else_block, &[]);

    builder.switch_to_block(then_block);
    builder.seal_block(then_block);
    emit_stmts_residual(then_, builder, ctx);
    builder.ins().jump(merge_block, &[]);

    builder.switch_to_block(else_block);
    builder.seal_block(else_block);
    emit_stmts_residual(else_, builder, ctx);
    builder.ins().jump(merge_block, &[]);

    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);
    // Execution continues in merge_block
}
```

This correctly gates contributions: only the taken branch's contributions are stamped.

### 5.2 Jacobian with control flow

The Jacobian pass uses the same `InlinedStmt` tree and the same `emit_if` pattern, but differentiates `expr` before emitting:

```rust
fn emit_stmts_jacobian(
    stmts: &[InlinedStmt],
    builder: &mut FunctionBuilder,
    ctx: &mut EmitCtx,
    branch_pairs: &[(String, String)],
) {
    for stmt in stmts {
        match stmt {
            InlinedStmt::Contrib { plus, minus, expr } => {
                for (a, b) in branch_pairs {
                    let wrt = branch_key(a, b);
                    let dexpr = diff(expr, &wrt);        // symbolic diff on inlined expr
                    let g = emit_expr(builder, ctx, &dexpr);
                    stamp_jacobian(builder, ctx, g, plus, minus, a, b);
                }
            }
            InlinedStmt::If { cond, then_, else_ } => {
                // Same if/else blocks, but emit derivatives in each branch.
                // The condition is preserved — derivative of a gated contribution
                // is gated by the same condition.
                emit_if_jacobian(builder, ctx, cond, then_, else_, branch_pairs);
            }
            InlinedStmt::Case { discriminant, arms, default } => {
                emit_case_jacobian(builder, ctx, discriminant, arms, default, branch_pairs);
            }
        }
    }
}
```

**Why this is correct**: `d/dV [if(c) { f(V) } else { g(V) }]` = `if(c) { f'(V) } else { g'(V) }`. The condition is piecewise-constant with respect to voltage (at any given Newton step, `c` is fixed), so its derivative is zero and the chain rule does not require differentiating `c`.

**Caveat**: if `c` itself depends on the branch voltage (e.g. `if (V(p,n) > 0) { ... }`), the Jacobian is discontinuous at the threshold. This is standard for analog simulation — Newton-Raphson handles piecewise-smooth models correctly as long as the operating point is away from the discontinuity. Models that need smooth derivatives use `select`/ternary with `limexp`.

### 5.3 `case` lowering

VA `case(disc) { val1: stmt1; val2: stmt2; default: stmt_d; }` lowers to a chain of `if/else`:

```
if (disc == val1) { stmt1 }
else if (disc == val2) { stmt2 }
...
else { stmt_d }
```

In Cranelift this becomes a chain of conditional branches with `switch` or nested `brif`. Use `switch` for dense integer discriminants, nested `brif` for sparse or real-valued cases.

### 5.4 Local variable reassignment

The inlining approach above handles single-assignment locals perfectly (`real k = expr; I(p,n) <+ k * V(p,n)`). It also handles sequential reassignment since `inline_vars` updates `env` at each `Assign` statement.

**Limitation**: local variables reassigned inside an `if` branch cannot be inlined across the branch boundary. Example:

```verilog
real k = 1.0;
if (V(p,n) > 0)
    k = 2.0;
I(p,n) <+ k * V(p,n) / r;
```

After the `if`, `k` may be 1.0 or 2.0 depending on the condition. Inlining cannot resolve this without keeping the `if` — the inliner must detect that `k` was conditionally modified and replace the post-if use of `k` with a ternary:

```
k_after = V(p,n) > 0 ? 2.0 : 1.0
I(p,n) <+ k_after * V(p,n) / r
```

**Implementation**: the inliner tracks a "maybe-modified" flag. If a local var is assigned inside any `if` branch, its `env` entry after the `if` becomes `Expr::If { cond, then_: new_val, else_: old_val }` (phi-node emulation). This is correct for pure analog use cases.

**Alternative for complex cases**: use Cranelift stack slots (mutable memory) for reassigned locals. Store at assignment sites, load at use sites. This avoids the phi-node complexity but prevents autodiff from seeing through the variable. Since autodiff is done symbolically on `Expr` BEFORE codegen (in pass 1), stack-slot locals are post-inlining and do not appear in the autodiffed exprs.

Recommended strategy: use inlining for single-assignment locals (covers 95% of VA models), fall back to stack slots for multi-assignment locals (accept derivative accuracy loss at discontinuous points).

### 5.5 `for` loops in analog

VA `for` loops in analog blocks are legal but heavily restricted: loop bounds must be compile-time constants (or parameter-derived constants), and loops typically do NOT contain contributions.

Common usage:
```verilog
integer i;
for (i = 0; i < n_sections; i = i + 1) {
    // computes intermediate values, not contributions
}
```

**Lowering strategy**:

1. If loop bounds are constant at elaboration time (integer parameter): **unroll** at elaboration. This is already done in `piperine-lang`'s elaborator for PHDL `for` loops.
2. If bounds depend on a runtime parameter: **forbid** at compile time with an error message. Real VA compilers (OpenVAF) also reject this.
3. Loop body containing a contribution: **forbidden** in analog context — emit `ElabError::Other("loop containing <+ is not supported")`. Models use parameterized sections via sub-instances, not loops around contributions.

For `while`, `repeat`, `forever`: same prohibition if contributions appear inside. These are allowed in digital blocks only.

---

## 6. AMS-Specific Statement Lowering

`piperine_ams::ast::Stmt` has more variants than PHDL's `ElabBehaviorStmt`. The AMS lowering pass must map each to `ElabBehaviorStmt` or inline it.

### 6.1 Statement mapping

| AMS `Stmt` variant | Lowering |
|--------------------|---------|
| `Empty` | drop |
| `Assign { op: Contrib (<+) }` | `ElabBehaviorStmt::Bind { op: Contrib }` |
| `Assign { op: Assign (=) }` | `ElabBehaviorStmt::Bind { op: Assign }` |
| `If` | `ElabBehaviorStmt::If` |
| `Case` / `Casex` / `Casez` | chain of `ElabBehaviorStmt::If` (see §5.3) |
| `Block { items }` | flatten: VarDecls → `ElabBehaviorStmt::VarDecl`, Stmts → recurse |
| `For` | unroll if const bounds; error if runtime bounds with contrib inside |
| `While` | error if contrib inside; analog restriction |
| `Repeat` | unroll if const count; error if runtime count with contrib |
| `Forever` | error if contrib inside |
| `Event { event: @initial_step }` | `ElabBehaviorStmt::Event { spec: Initial }` |
| `Event { event: @final_step }` | `ElabBehaviorStmt::Event { spec: Final }` |
| `Event { event: @cross(...) }` | analog event — record state var, `ElabBehaviorStmt::Event { spec: Named("cross") }` |
| `Event { event: @timer(...) }` | analog event — record state var, `ElabBehaviorStmt::Event { spec: Named("timer") }` |
| `IndirectContrib` | `ElabBehaviorStmt::Bind { op: Contrib }` with companion equation |
| `NonBlockingAssign` | digital only; error in analog context |
| `TimingControl` | analog: inline timing hints; digital: schedule events |
| `Fork` | unsupported — emit diagnostic |
| `Disable` | unsupported — emit diagnostic |
| `Expr { expr: sys_call }` | `ElabBehaviorStmt::Diagnostic` for `$display`/`$warning`/etc. |
| `Expr { expr: $bound_step }` | new `ElabBehaviorStmt::BoundStep { expr }` |
| `Expr { expr: $finish }` | new `ElabBehaviorStmt::Finish` |

### 6.2 BlockStmt with VarDecl

AMS `begin ... end` blocks have mixed items:

```verilog
begin
    real k;
    k = V(p,n) / r;
    I(p,n) <+ k;
end
```

Lowering: iterate `BlockItem` in order — `VarDecl` → `ElabBehaviorStmt::VarDecl`, `Stmt` → recurse. The inliner (§5.1) then substitutes `k` into the contribution.

**Scoping**: AMS block labels (`begin: label ... end`) create a named scope. Lowering flattens scoped blocks, appending the label to all local var names to prevent collisions (`label.k` instead of `k`).

### 6.3 Case with real discriminant

```verilog
case (mode)
    1: I(p,n) <+ linear_current;
    2: I(p,n) <+ nonlinear_current;
    default: I(p,n) <+ 0;
endcase
```

Lower to:
```
if (mode == 1.0) { Contrib linear_current }
else if (mode == 2.0) { Contrib nonlinear_current }
else { Contrib 0 }
```

### 6.4 Event statements in analog

`@(initial_step) stmt` — execute `stmt` only at the first simulation step (DC or the first transient step). Maps to `ElabBehaviorStmt::Event { spec: Named("initial_step") }`.

`@(final_step) stmt` — execute at last step. Maps to `spec: Named("final_step")`.

`@(cross(expr, dir)) stmt` — fire when `expr` crosses zero. This introduces a **state variable** (see §7 below) to detect the crossing. The `stmt` typically sets a flag or bounds the step.

Analog events cannot contain contributions — they are for side effects (display, step control) only.

### 6.5 `for` loops in AMS analog

```verilog
integer i;
real sum = 0;
for (i = 0; i < n; i = i + 1)
    sum = sum + V(node[i], gnd);
I(p,n) <+ sum / n;
```

Strategy:
1. Check if `n` is a constant parameter — if yes, unroll at lowering.
2. If no: the loop body does NOT contain a direct contribution, so the loop can be emitted as a Cranelift loop (use an integer SSA counter + loop block + back edge).
3. If the loop body contains `<+` and bounds are not constant: emit `LowerError::LoopWithContrib`.

**Cranelift loop emission** (for runtime-bound loops without contributions):

```
header_block:  phi(i = 0)
  if i < n → body_block, else → exit_block
body_block:
  compute loop body (pure computation, no stamping)
  i' = i + 1
  jump header_block(i')
exit_block:
  use accumulated sum in contribution
```

---

## 7. State Variables

State variables are allocated by the lowering pass for analog operators that require persistent state across time steps.

### 7.1 Operators requiring state

| Operator | State contents |
|----------|---------------|
| `ddt(x)` | previous value of `x`, for `(x_new - x_old) / h` |
| `idt(x, ic)` | accumulated integral value |
| `idtmod(x, ic, mod)` | modular integral value |
| `$limit(v, "pnjlim", vt, vcrit)` | previous Newton-step value of `v` |
| `cross(expr, dir)` | previous value of `expr`, for crossing detection |
| `timer(start, period)` | next fire time |
| `transition(x, td, tr, tf)` | waveform queue (complex: multiple pending edges) |
| `slew(x, rise, fall)` | current output value |

### 7.2 Allocation

During the inline-variables pass (§5.1), when a call to `ddt`, `idt`, etc. is encountered:
1. Allocate a new `StateVarId` (sequential counter).
2. Replace the call with a synthetic expr `IrStateRef(StateVarId)`.
3. Record `(StateVarId, kind, argument_expr)` in the block's `state_vars` list.

`IrStateRef(id)` in the residual function reads `state[id]` (passed via `SimContext`). In the reactive contribution path, `state_next[id]` is written after `eval`.

### 7.3 `ddt` reactive stamping

`I(p,n) <+ C * ddt(V(p,n))` lowers to:

- **Residual (resistive)**: stamp 0 (reactive current does not appear in resistive residual).
- **Residual (reactive)**: stamp `C * (V(p,n) - state[id]) / h` where `h` = timestep.
- **Jacobian (resistive)**: 0.
- **Jacobian (reactive)**: stamp `C / h`.
- **State update**: `state_next[id] = V(p,n)` (written after Newton convergence).

The lowering annotates the contribution as reactive vs. resistive based on whether `ddt`/`idt` appears in its expr:

```rust
enum ContribKind { Resistive, Reactive { state_id: StateVarId } }
```

### 7.4 `idt` and `idtmod`

`I(p,n) <+ idt(current_src, ic)` lowers using backward Euler:
- `integral_new = integral_old + h * current_src`
- Reactive stamp: `integral_new` as a voltage contribution (V-mode).
- State update: `state_next[id] = integral_new`.

Initial condition `ic` is the value at `t=0`. The solver handles initial conditions during DC operating-point analysis.

---

## 8. PHDL-Specific Lowering

PHDL's `ElabBehaviorStmt` is already the IR. The elaborator (`piperine-lang/src/elab/lower.rs`) handles PHDL. The analog codegen (`analog.rs`) consumes `ElabBehaviorStmt` directly.

### 8.1 What needs fixing in `analog.rs`

The current `extract_contributions` + flat loop approach must be replaced entirely. The new flow:

```
ElabBehaviorStmt tree
        │
        ▼
   inline_vars()        → InlinedStmt tree (no free local vars in contrib exprs)
        │
        ▼
  emit_stmts_residual() → Cranelift residual function (with basic blocks for if/else)
  emit_stmts_jacobian() → Cranelift jacobian function (same structure, diff applied)
```

### 8.2 PHDL `match` in analog

PHDL `match` lowers identically to VA `case`: chain of `if/else` blocks in Cranelift. Each arm becomes an `fcmp Equal` on the discriminant.

### 8.3 PHDL `for` unrolling

PHDL `for` loops with const bounds are already unrolled by the elaborator (the `ElabBehaviorStmt` tree has no `For` variant). For loops in the elab IR are therefore already gone when codegen sees them — no issue here.

### 8.4 PHDL `@event` in analog

PHDL event blocks (`@ cross(...) when (guard) { body }`) map to analog events. Same treatment as AMS `@cross`: state variable for crossing detection; body cannot contain `<+`.

---

## 9. Digital Codegen (No Changes Needed)

The `DigitalInterpreter` in `digital.rs` correctly handles `ElabBehaviorStmt::If` — it evaluates the condition at runtime and executes only the taken branch. Control flow in digital is correct because:

1. Digital is a tree-walking interpreter, not a Cranelift compiler.
2. `exec_one` for `If` correctly branches:
   ```rust
   ElabBehaviorStmt::If { cond, then_body, else_body } => {
       if self.eval_expr(cond, nets).as_bool() {
           self.exec_stmts(&tb, t, nets, queue);
       } else if let Some(else_b) = eb {
           self.exec_stmts(&else_b, t, nets, queue);
       }
   }
   ```
3. State variables in digital are in `HashMap<String, DigitalVal>` — mutable and shared across branches correctly.

No changes needed to digital codegen for control flow.

---

## 10. JIT ABI

Current signature: `fn(node_voltages: *const f64, params: *const f64, output: *mut f64)`.

Extended for state variables and sim queries (Wave A):

```c
typedef struct {
    double  temperature;    // K, default 300.15
    double  abstime;        // s, 0 in DC/AC
    double  timestep;       // h, infinity in DC
    double *simparam;       // indexed by SimparamId enum
    double *state;          // state_vars[id].value (read)
    double *state_next;     // state_vars[id].value (written after eval)
} SimContext;

void residual(const double *voltages, const double *params,
              const SimContext *ctx, double *rhs);
void jacobian(const double *voltages, const double *params,
              const SimContext *ctx, double *jac);
```

This is one additional pointer argument — forward-compatible as `SimContext` grows. All sim query expressions (`$temperature`, `$abstime`, etc.) read from `ctx`. State reads use `ctx->state[id]`; state writes go to `ctx->state_next[id]`.

**Reactive vs. resistive**: in the current ABI, `rhs` is the combined output. With reactive contributions, two output arrays are needed:

```c
void residual_resist(const double *voltages, const double *params,
                     const SimContext *ctx, double *rhs_r);
void residual_react (const double *voltages, const double *params,
                     const SimContext *ctx, double *rhs_c);
void jacobian_resist(const double *voltages, const double *params,
                     const SimContext *ctx, double *jac_r);
void jacobian_react (const double *voltages, const double *params,
                     const SimContext *ctx, double *jac_c);
```

Or, alternatively, compile four separate functions per module. Preferred: four separate functions — each is simpler, and the solver calls whichever is needed for the current analysis type (DC only needs resist; AC needs both).

---

## 11. `FrontendLower` Trait

```rust
pub trait FrontendLower {
    type Error: std::error::Error;
    fn lower(&self, top_module: &str) -> Result<ElabProgram, Self::Error>;
}
```

Two implementors:
- `PprLower<'a>(&'a piperine_lang::parse::SourceFile)` — uses the existing `Elaborator`
- `AmsLower<'a>(&'a piperine_ams::Document)` — new, in `piperine-ams/src/lower.rs`

Both produce `ElabProgram`. The CLI selects by file extension.

---

## 12. `AmsLower` Responsibilities

Located in `piperine-ams/src/lower.rs`, depends on `piperine-lang`.

### 12.1 Module → `ElabMod`

- Ports: `piperine_ams::Port` → `ElabPort { direction, name, ty: ElabNetType::Discipline(disc) }`.
- Params: `piperine_ams::Parameter` → `ElabParam { name, ty: Real, default: eval_const_expr(default) }`.
- Wires (internal nets): `piperine_ams::Net` → `ElabWire { name, ty: ElabNetType::Discipline(disc) }`.
  - Use `net_discipline()` fallback (same logic as `to_phdl`) when `discipline` is None but `ty` is `Type::Custom`.
- Instances: `piperine_ams::Instance` → `ElabInstance { module, ports, params }`.
- Sub-module connections: bind port names to net names via `connections`.

### 12.2 Analog block → `ElabBehavior`

For each `AnalogBlock` in the module:
- `is_initial = true` → `ElabBehavior { kind: Analog, body: [Event { spec: Initial, body: ... }] }`
- `is_initial = false` → `ElabBehavior { kind: Analog, body: lower_stmts(block.stmt) }`

### 12.3 `lower_stmts(stmt: &piperine_ams::ast::Stmt)` → `Vec<ElabBehaviorStmt>`

Recursive. Follows the table in §6.1. Key mappings:

```rust
Stmt::Assign(a) if a.op == Contrib =>
    ElabBehaviorStmt::Bind { dest: lower_expr(a.lvalue), op: Contrib, src: lower_expr(a.rvalue) }

Stmt::If(s) =>
    ElabBehaviorStmt::If {
        cond: lower_expr(s.condition),
        then_body: lower_stmts(s.then_branch),
        else_body: s.else_branch.map(|b| lower_stmts(b)),
    }

Stmt::Block(b) =>
    b.items.flat_map(|item| match item {
        BlockItem::VarDecl(v) => lower_var_decl(v),
        BlockItem::Stmt(s)    => lower_stmts(s),
    })

Stmt::For(f) =>
    if bounds_are_const(f) {
        unroll_for(f, lower_stmts)
    } else if contains_contrib(f.for_body) {
        return Err(LowerError::LoopWithContrib)
    } else {
        lower_runtime_for(f)   // Cranelift loop, no contributions inside
    }
```

### 12.4 Expression lowering

AMS `Expr` → PHDL `Expr` (they share a common structure; the PHDL `Expr` type is reused):

| AMS expression | PHDL `Expr` |
|----------------|------------|
| `Expr::Real(v)` | `Expr::Literal(Literal::Real(v))` |
| `Expr::Int(n)` | `Expr::Literal(Literal::Int(n))` |
| `Expr::Ident(s)` | `Expr::Ident(s)` |
| `Expr::Binary(l, op, r)` | `Expr::Binary(lower(l), lower_binop(op), lower(r))` |
| `Expr::Unary(op, x)` | `Expr::Unary(lower_unop(op), lower(x))` |
| `Expr::Call("V", [a, b])` | `Expr::Call("V", [lower(a), lower(b)])` — branch voltage |
| `Expr::Call("I", [a, b])` | `Expr::Call("I", [lower(a), lower(b)])` — branch current |
| `Expr::Call("ddt", [x])` | `Expr::Call("ddt", [lower(x)])` — state var allocated by inliner |
| `Expr::Call("$temperature", [])` | `Expr::SysCall("temperature", [])` |
| `Expr::Call("$vt", [])` | `Expr::SysCall("vt", [])` |
| `Expr::Call("$abstime", [])` | `Expr::SysCall("abstime", [])` |
| `Expr::Call("$simparam", [key, def])` | `Expr::SysCall("simparam", [key, lower(def)])` |
| `Expr::Ternary(c, t, f)` | `Expr::If { cond: lower(c), then: block(lower(t)), else: block(lower(f)) }` |
| `Expr::Call(f, args)` (math) | `Expr::Call(f, args.map(lower))` |

SI literal suffixes (`1k`, `10n`, `70K`) are folded to `f64` during lexing in `piperine-ams` — they arrive in `AmsLower` already as `Expr::Real(f64)`.

---

## 13. Invariants Post-Lowering

| Invariant | Enforced by |
|-----------|------------|
| No `ElabBehaviorStmt::Bind { Contrib }` with free local variable names in `src` | `inline_vars` pass |
| No `For`/`While`/`Repeat` containing `<+` in analog | `lower_stmts` error check |
| Every analog contribution has `dest = Call("I"|"V", [Ident, Ident])` | validated in `inline_vars` |
| `VarDecl` inside `If`/`Case` arms handled by phi-inlining or stack slots | `inline_vars` |
| `@(initial_step)` / `@(final_step)` events contain no `<+` | validated in `lower_stmts` |
| State var IDs are sequential from 0 | allocated by counter in `inline_vars` |
| `SimContext.state.len() >= max(StateVarId) + 1` | `JitDevice` allocates from `state_vars.len()` |

---

## 14. Roadmap

Tasks in dependency order:

| Task | Crate | Blocks |
|------|-------|--------|
| `inline_vars()` pass — local variable inlining | `piperine-lang` | — |
| `InlinedStmt` enum | `piperine-lang` | `inline_vars` |
| `emit_stmts_residual()` with Cranelift basic blocks | `piperine-lang/codegen/analog.rs` | `InlinedStmt` |
| `emit_stmts_jacobian()` with same structure | `piperine-lang/codegen/analog.rs` | `InlinedStmt` |
| Replace `extract_contributions` + flat loop | `piperine-lang/codegen/analog.rs` | above |
| `SimContext` in JIT ABI | `piperine-lang/codegen/analog.rs` | — |
| Four-function split (resist/react × residual/jacobian) | `piperine-lang/codegen/analog.rs` | SimContext |
| State variable allocation in `inline_vars` | `piperine-lang` | `InlinedStmt` |
| `ddt` reactive stamping | `piperine-lang/codegen/analog.rs` | state vars |
| `AmsLower::lower()` — `piperine_ams::Document` → `ElabProgram` | `piperine-ams` | piperine-lang types |
| `AmsLower` expression lowering | `piperine-ams` | AmsLower |
| `AmsLower` statement lowering with control flow | `piperine-ams` | AmsLower |
| Noise pass (`white_noise`/`flicker_noise` → `load_noise`) | `piperine-lang` + `piperine-solver` | — |
| `$temperature` / `$vt` wired from solver SimContext | `piperine-solver` | SimContext ABI |
| `cross` / `timer` event detection in solver | `piperine-solver` | state vars |

---

## 15. Example: Diode with `if`

```verilog
// diode.va
module diode(a, k);
  inout electrical a, k;
  parameter real is = 1e-14;
  parameter real n  = 1.0;
  analog begin
    real vd, id;
    vd = V(a, k);
    if (vd > 0.7)
      id = is * (exp(0.7 / (n * $vt)) + (vd - 0.7) / 0.026);
    else
      id = is * (exp(vd / (n * $vt)) - 1.0);
    I(a, k) <+ id;
  end
endmodule
```

**After `inline_vars`**:

```
Contrib {
    plus: "a", minus: "k",
    expr: If {
        cond: Binary(V(a,k), Gt, 0.7),
        then_: is * (exp(0.7 / (n * $vt)) + (V(a,k) - 0.7) / 0.026),
        else_: is * (exp(V(a,k) / (n * $vt)) - 1.0),
    }
}
```

Note: `vd` and `id` are fully inlined — they do not appear in the contribution expr.

**Cranelift residual** (pseudocode):

```
vd = voltages[0] - voltages[1]     // V(a,k)

is_val = params[0]
n_val  = params[1]
vt_val = ctx.temperature * 8.617e-5  // $vt

cond = fcmp(vd, >, 0.7)
brif cond → then_block, else_block

then_block:
    id_then = is_val * (exp(0.7 / (n_val * vt_val)) + (vd - 0.7) / 0.026)
    jump merge_block(id_then)

else_block:
    id_else = is_val * (exp(vd / (n_val * vt_val)) - 1.0)
    jump merge_block(id_else)

merge_block(id):
    rhs[0] += id
    rhs[1] -= id
```

**After `diff(expr, "V(a,k)")`** (symbolic, on the inlined expr):

```
If {
    cond: V(a,k) > 0.7,
    then_: is * (1/0.026),                          // d/dV of linear part
    else_: is * exp(V(a,k)/(n*vt)) / (n*vt),        // d/dV of exp part
}
```

**Cranelift jacobian** (same if/else structure, derivative expressions in each branch):

```
cond = fcmp(vd, >, 0.7)
brif cond → then_jac, else_jac

then_jac:
    g_then = is_val / 0.026
    jump merge_jac(g_then)

else_jac:
    g_else = is_val * exp(vd / (n_val * vt_val)) / (n_val * vt_val)
    jump merge_jac(g_else)

merge_jac(g):
    jac[0*2+0] += g;  jac[0*2+1] -= g
    jac[1*2+0] -= g;  jac[1*2+1] += g
```

Both residual and Jacobian correctly gate computation on the condition. Newton-Raphson converges correctly because the Jacobian matches the residual's derivative in each branch.
