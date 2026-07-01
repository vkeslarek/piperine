# Compile-time Constant Evaluation

The constant evaluator (`src/elab/const_eval.rs`) evaluates expressions at elaboration
time for array dimensions, structural for-loop bounds, structural if conditions, instance
const args, and parameter defaults.

---

## `ConstVal` enum

```rust
pub enum ConstVal {
    Int(i64),
    Nat(u64),
    Real(f64),
    Bool(bool),
    Str(String),
}
```

Five types of compile-time values. `Nat` is used for natural numbers (non-negative
unsigned). `Int` is used for signed integers. `Quad` is never a const value — quad
literals always produce a `NotConst` error.

---

## `ConstEvalError` variants

```rust
pub enum ConstEvalError {
    NotConst(String),       // expression is not a compile-time constant
    DivByZero,              // division or remainder by zero
    Undefined(String),      // undefined name in environment
    TypeMismatch,           // type mismatch in constant expression
}
```

---

## `ConstEnv`

A scoped environment with a stack of `HashMap<String, ConstVal>` bindings.

### Construction

```rust
pub fn new() -> Self
```

Creates a single top-level scope containing an empty `HashMap`.

`Default for ConstEnv` calls `new()`.

### Scope management

```rust
pub fn push(&mut self)    // push a new scope onto the stack
pub fn pop(&mut self)     // pop the current scope from the stack
```

Used for `for`-loop body unrolling: each iteration pushes a new scope with the loop
variable binding, then pops it after the body is unrolled.

### Binding

```rust
pub fn define(&mut self, name: String, val: ConstVal)
```

Inserts a binding into the current (innermost) scope.

### Lookup

```rust
pub fn lookup(&self, name: &str) -> Option<&ConstVal>
```

Searches scopes from innermost to outermost (reverse iteration over the `bindings`
stack). Returns the first match.

---

## `ConstEnv::eval(expr)` — expression evaluation

Evaluates a parse AST `Expr` to a `ConstVal`. Supported forms:

### Literals

| AST literal | Result |
|---|---|
| `Literal::Int(n)` | `Ok(ConstVal::Nat(*n))` |
| `Literal::Real(r)` | `Ok(ConstVal::Real(*r))` |
| `Literal::Bool(b)` | `Ok(ConstVal::Bool(*b))` |
| `Literal::String(s)` | `Ok(ConstVal::Str(s.clone()))` |
| `Literal::Quad(q)` | `Err(NotConst("quad literal 0q..."))` |

### `Ident(name)`

Looks up `name` in the scoped environment via `lookup()`. Returns `Undefined` if not
found.

### `Unary(op, inner)`

| Operator | Input | Result |
|---|---|---|
| `Neg` | `Nat(n)` | `Int(-(n as i64))` |
| `Neg` | `Int(n)` | `Int(-n)` |
| `Neg` | `Real(r)` | `Real(-r)` |
| `Not` | `Bool(b)` | `Bool(!b)` |
| `Not` | `Nat(n)` | `Nat(!n)` |

All other combinations produce `TypeMismatch`.

### `Binary(lhs, op, rhs)`

Dispatches to `eval_binary()` (see below).

### `If { cond, then_body, else_body }`

Evaluates `cond`. If the result is `Bool(true)`, `Nat(1)`, or any non-zero `Nat`,
the `then_body` block is evaluated. Otherwise the `else_body` block is evaluated.

### `Block(block)`

Delegates to `eval_block()`.

### Everything else

Returns `NotConst` with a debug format of the expression.

---

## `eval_binary()` — binary evaluation

Type-dispatches on the operator and operand types:

### Natural arithmetic

`Add`, `Sub`, `Mul` (wrapping), `Div`, `Rem` — all on `(Nat, Nat)` operand pairs.
Division/remainder by zero returns `DivByZero`.

### Integer arithmetic

Same ops as Natural, on `(Int, Int)` operand pairs. Also wrapping for `Add`, `Sub`, `Mul`.

### Mixed `Nat` / `Int`

`Nat + Int`, `Int + Nat`, `Nat - Int`, `Int - Nat`, `Nat * Int`, `Int * Nat` — all
promote to `Int`. Division and remainder on mixed types are **not** supported
(TypeMismatch).

### Real arithmetic

`Add`, `Sub`, `Mul`, `Div` on `(Real, Real)` pairs. Division by zero is silently
permitted (produces NaN/Infinity per IEEE 754).

### Comparisons — `Nat`

`Eq`, `Neq`, `Lt`, `Le`, `Gt`, `Ge` on `(Nat, Nat)` produce `Bool`.

### Comparisons — `Int`

`Eq`, `Neq`, `Lt`, `Le`, `Gt`, `Ge` on `(Int, Int)` produce `Bool`.

### Comparisons — `Bool`

`Eq`, `Neq` on `(Bool, Bool)` produce `Bool`.

### Bitwise — `Nat`

`BitAnd`, `BitOr`, `BitXor` on `(Nat, Nat)` produce `Nat`.

### Bitwise — `Bool`

`BitAnd`, `BitOr`, `BitXor` on `(Bool, Bool)` produce `Bool`.

### Unsupported operand combinations

All other type/operator combinations return `TypeMismatch`.

---

## `eval_block()` — block evaluation

Only evaluates blocks that consist solely of a trailing expression. `return expr`
statements are supported (the return value is evaluated). Blocks with other kinds
of statements or without a trailing expression return `NotConst`.

---

## Typed eval helpers

### `eval_nat(expr) -> Result<u64, ConstEvalError>`

Evaluates the expression and coerces the result to `u64`. Accepts `Nat(n)` directly
and non-negative `Int(n)` values. All other values produce `TypeMismatch`.

### `eval_int(expr) -> Result<i64, ConstEvalError>`

Evaluates the expression and coerces the result to `i64`. Accepts `Int(n)` directly
and `Nat(n)` (via `n as i64`). All other values produce `TypeMismatch`.

---

## Where `ConstEnv` is used in elaboration

The constant evaluator is invoked throughout the elaboration pipeline:

| Usage site | File | Purpose |
|---|---|---|
| Array dimension evaluation | `resolve.rs` | Converts `Type::dimensions` expressions to concrete `u64` values |
| Structural for-loop bounds | `module.rs` | Evaluates start/end of `StructuralFor` range to unroll the body |
| Structural if conditions | `module.rs` | Evaluates `StructuralIf` condition to select the taken branch |
| Behavioral for-loop bounds | `behavior.rs` | Evaluates start/end of behavioral `for` loop to unroll |
| Instance const args | `module.rs` | Evaluates `const_args` to concrete `u64` values for monomorphization |
| Instance param overrides | `module.rs` | Evaluates `ParamArg` expressions to `ConstVal` |
| Parameter defaults | `module.rs` | Evaluates `ParamDecl` default expressions |
| Elaboration-constant `if` folding | `behavior.rs` | Constant-folds `if` conditions in behavior blocks |
