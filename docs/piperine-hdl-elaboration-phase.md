# Piperine HDL — Elaboration Phase

Elaboration is the compile-time pass that transforms a parsed PHDL `SourceFile` into a fully
resolved `ElabProgram`. After elaboration, the IR contains no generic parameters, no unresolved
constants, no structural for/if statements, and no bundle references in port lists.

---

## 1. Two-phase evaluation model

PHDL distinguishes two evaluation phases by location:

| Phase | Where | What happens |
|-------|-------|-------------|
| **Elaboration** | `mod` body, type annotations, structural control flow | Resolved once at compile time into a fixed netlist |
| **Solve** | `analog` / `digital` blocks | Evaluated by the Newton–Raphson or event-driven engine |

A solve-phase value never controls elaboration structure. Hardware is neither created nor
destroyed during the solve — runtime topology change is achieved via switch branches over a
static netlist.

---

## 2. Entry point

```rust
use piperine_lang_parser::{parse_str, elaborate};

let source = parse_str(input)?;
let program = elaborate(source)?;  // injects stdlib, then elaborates
```

The `elaborate` function:
1. Prepends the standard library items to the source.
2. Registers all items (disciplines, bundles, enums, modules, behaviors, functions, capabilities,
   impls) into symbol tables.
3. Validates semantic rules (§9).
4. Elaborates each item in dependency order.
5. Returns an `ElabProgram`.

---

## 3. Constant evaluation

Expressions that appear in elaboration positions (array dimensions, structural for/if
conditions, param defaults, instance const args) must be compile-time constants.

### 3.1 ConstVal

```
ConstVal = Int(i64) | Nat(u64) | Real(f64) | Bool(bool) | Str(String)
```

### 3.2 ConstEnv

`ConstEnv` is a scoped stack of name→ConstVal bindings. Each structural for iteration pushes
a new scope with the loop variable, then pops it after the body.

### 3.3 Evaluatable forms

| Expression | Result |
|-----------|--------|
| Integer literal | `ConstVal::Nat` |
| Real literal | `ConstVal::Real` |
| Bool literal (0/1) | `ConstVal::Nat`; resolved to `Bool` by context |
| Named binding | Look up in `ConstEnv`; error if absent |
| Unary `-` / `!` | Applied to inner value |
| Binary arithmetic / comparison / bitwise | Applied to matching ConstVal pairs |
| `if (cond) { … } else { … }` | Evaluate cond; take the matching branch |
| Block expression with trailing expr or `return` | Evaluate trailing value |

Anything else (calls, field access, array comprehensions) is **not const** — attempting to
evaluate it yields `ConstEvalError::NotConst`.

---

## 4. Type resolution

Types are resolved at elaboration to either `ElabNetType` or `ElabValueType`.

### 4.1 Primitive value types

| PHDL name | ElabValueType |
|-----------|--------------|
| `Real` | `Real` |
| `Natural` | `Natural` |
| `Integer` | `Integer` |
| `Complex` | `Complex` |
| `Boolean` | `Boolean` |
| `Quad` | `Quad` |
| `String` | `Str` |

### 4.2 Discipline (net) types

Any name declared with `discipline { … }` resolves to `ElabNetType::Discipline(name)`.

### 4.3 Bundle types and net-capability

A bundle is **net-capable** if every field (recursively) resolves to a net type. Net-capable
bundles may be used as port types; the elaborator expands them to flat fields (§6).

A bundle that contains any value-type field is a **value bundle** and cannot appear as a net
type. Attempting to use it as a port type yields `ElabError::NotNetCapable`.

The `NetType` capability is implicitly satisfied by all net-capable bundles and disciplines —
no explicit declaration needed.

### 4.4 Enum types

Names declared with `enum { … }` resolve to `ElabValueType::Enum(name)`.

### 4.5 Array types with concrete dimensions

Array dimensions (`Type[N]`) are evaluated via `ConstEnv::eval_nat`. An unevaluatable
dimension (e.g. a free type parameter) is a compile error.

```
Bit[8]       →  ElabNetType::Array(ElabNetType::Discipline("Bit"), 8)
Bit[N]       →  const param N must be in scope
Bit[2 * 4]   →  ElabNetType::Array(ElabNetType::Discipline("Bit"), 8)
```

---

## 5. Structural elaboration (mod body)

### 5.1 For loop unrolling

Structural `for` loops in a `mod` body are unrolled at elaboration time. Both bounds must
evaluate to `ConstVal::Nat`.

```phdl
for i in 0..N {
    Resistor(node[i], node[i+1]) { .r = r };
}
// becomes N separate instance statements with i substituted
```

V1 note: Loop termination is not proven — the elaborator recurses into the body with `i`
bound. A deeply nested or very large `N` will exhaust the call stack. Bounded loops with
small ranges are the intended use case.

### 5.2 Structural if/else

`if (cond) { … } else { … }` in a `mod` body is evaluated at elaboration. The condition must
be a compile-time constant. Only the taken branch is emitted.

```phdl
if (N > 1) { … }   // N must be a const param or literal
```

### 5.3 Validation

The following are errors in `mod` body context:

| Construct | Error |
|-----------|-------|
| `<+` contribution | `ElabError::ContribInModBody` |
| `<-` force | `ElabError::ForceInModBody` |

---

## 6. Generic monomorphization

### 6.1 Const parameters

A module with const params (e.g. `mod Dac[N]`) is not elaborated until it is instantiated
with a concrete value. When `Dac[8]` appears, the elaborator:

1. Substitutes `N → 8` in the `ConstEnv`.
2. Elaborates the body with that binding in scope.
3. Caches the result under the mangled name `Dac__8`.

**Name mangling scheme**: `{ModuleName}__{arg0}_{arg1}_...`

```
Dac[8]     →  Dac__8
Driver[4]  →  Driver__4
Grid[4, 4] →  Grid__4_4
```

### 6.2 Type parameters

Type parameters (e.g. `mod Adder<T: Add + Net>`) are substituted via a `type_subst` map
(string → string) propagated during elaboration. For V1, type-parameterized modules with no
concrete instantiation site are stored with their generic bodies and monomorphized on demand
(future work).

### 6.3 Generic functions

Functions with type params (`fn map<T, U>(…)`) keep their generic bodies in `ElabFn`. The
elaborator resolves what it can; type-param-dependent return types fall back to `Real` as a
placeholder. Full generic inlining is deferred to the type checker.

---

## 7. Bundle expansion

When a port has a net-capable bundle type, it is expanded to one `ElabPort` per field.
Naming convention: `{port_name}_{field_name}`.

```phdl
bundle DiffPair { p : Electrical, n : Electrical }
mod Amp ( input inp : DiffPair, output out : Electrical ) { … }
// elaborates to:
//   input inp_p : Electrical
//   input inp_n : Electrical
//   output out  : Electrical
```

Field access expressions (`inp.p`) are preserved as-is in behavior bodies; a future type
checker will validate and rewrite them to the expanded names.

---

## 8. Behavior elaboration

Behavior blocks (`analog` / `digital`) are elaborated with the same const-evaluation
machinery but with different rules.

### 8.1 For loop unrolling in behaviors

Per the spec: "A `for` in either block is unrolled into hardware, so its bound must be an
elaboration constant." The elaborator unrolls behavioral for loops identically to structural
ones. Runtime loops are not allowed.

### 8.2 If/else constant folding

`if` conditions in behavior blocks that evaluate to a ConstVal are folded — the dead branch
is dropped. Runtime conditions are kept as `ElabBehaviorStmt::If`.

### 8.3 Match arms

Match statements are kept as-is. The type checker is responsible for exhaustiveness checks.

---

## 9. Event system

### 9.1 Open event model

The parser emits `EventSpec::Named { name: String, arg: Expr }` for all event calls. The
elaborator looks up each name in the `EventRegistry`. This makes events extensible: any
identifier can be an event name; the registry defines which ones are valid and what they mean.

### 9.2 Built-in events

| Name | Fires on | Domain |
|------|----------|--------|
| `posedge` | rising edge of a digital signal | digital |
| `negedge` | falling edge of a digital signal | digital |
| `change` | any value change of a digital signal | digital |
| `cross` | analog expression crossing zero | analog |
| `above` | analog expression exceeding zero | analog |
| `initial` | once at simulation start | both |
| `final` | once at simulation end | both |

### 9.3 Adding a custom event

```rust
use piperine_lang_parser::elab::event::{EventKind, EventRegistry};

struct SampleEvent;

impl EventKind for SampleEvent {
    fn name(&self) -> &str { "sample" }
    fn is_digital_edge(&self) -> bool { true }
}

// Register before elaborating:
let mut elaborator = Elaborator::new();
elaborator.events_mut().register(SampleEvent);
```

### 9.4 Domain validation

| Event kind | In analog block | In digital block |
|-----------|----------------|-----------------|
| `is_digital_edge()` | `ElabError::DigitalEventInAnalog` | ✓ |
| `is_analog_crossing()` | ✓ | `ElabError::AnalogEventInDigital` |
| `initial` / `final` | ✓ | ✓ |

---

## 10. Standard library

The stdlib is injected before elaboration — its items are available in every compilation unit
without an explicit `use`.

### 10.1 Capabilities

| Capability | Supers | Methods |
|-----------|--------|---------|
| `Add` | — | `fn add(self, o: Self) -> Self` |
| `Sub` | — | `fn sub(self, o: Self) -> Self` |
| `Mul` | — | `fn mul(self, o: Self) -> Self` |
| `Div` | — | `fn div(self, o: Self) -> Self` |
| `Eq` | — | `fn eq(self, o: Self) -> Boolean` |
| `Ord` | `Eq` | `fn lt(self, o: Self) -> Boolean` |
| `BitAnd` | — | `fn bitand(self, o: Self) -> Self` |
| `BitOr` | — | `fn bitor(self, o: Self) -> Self` |
| `BitXor` | — | `fn bitxor(self, o: Self) -> Self` |
| `Not` | — | `fn not(self) -> Self` |
| `Number` | `Add + Sub + Mul` | `fn double(self) -> Self` (default body) |

Operator sugar (§6.6 of the spec) maps binary operators to capability methods:

| Operator | Desugars to |
|----------|------------|
| `a + b` | `a.add(b)` |
| `a - b` | `a.sub(b)` |
| `a * b` | `a.mul(b)` |
| `a / b` | `a.div(b)` |
| `a == b` | `a.eq(b)` |
| `a < b` | `a.lt(b)` |
| `a & b` | `a.bitand(b)` |
| `a \| b` | `a.bitor(b)` |
| `a ^ b` | `a.bitxor(b)` |
| `!a` | `a.not()` |

### 10.2 Higher-order collection functions

```phdl
fn map<T, U>(xs: T[N], f: fn(T) -> U) -> U[N]
fn reduce<T>(xs: T[N], op: fn(T, T) -> T) -> T
```

`map` applies `f` to each element via array comprehension. `reduce` splits the array in half
recursively and combines with `op` (balanced tree). Both are elaboration-time generators —
when called with a concrete `N`, they unroll to a fixed structure.

---

## 11. Validation rules catalog

| Error | Trigger |
|-------|---------|
| `ContribInDigital` | `<+` inside a `digital` block |
| `ContribInModBody` | `<+` inside a `mod` body |
| `ForceInModBody` | `<-` inside a `mod` body |
| `AnalogEventInDigital(name)` | `cross`/`above` inside a `digital` block |
| `DigitalEventInAnalog(name)` | `posedge`/`negedge`/`change` inside an `analog` block |
| `UnknownEvent(name)` | Event name not found in `EventRegistry` |
| `NotNetCapable(name)` | Bundle used as net type but contains value fields |
| `UndefinedType(name)` | Type name not found in any symbol table |
| `UndefinedModule(name)` | Module name not found during monomorphization |
| `MissingConstParam { param, module }` | Module instantiated with wrong number of const args |
| `ConstEval { context, source }` | Const expression failed to evaluate (wraps `ConstEvalError`) |

---

## 12. Future work

- **Import resolution**: `use pkg::item` — currently stored but not resolved.
- **Type parameter monomorphization**: full demand-driven generic inlining.
- **Bundle field-access rewriting**: rewrite `inp.p` → `inp_p` after bundle expansion.
- **Operator desugar pass**: replace `a + b` with `a.add(b)` using capability lookup.
- **Elaboration depth limit**: explicit recursion counter to give a clean error instead of
  stack overflow on non-terminating const computations.
