# Part II — Elaboration Phase

*Piperine HDL — Elaboration Phase*

Elaboration transforms a parsed `SourceFile` into a resolved `ElabProgram`: no generic
parameters, no unresolved constants, no structural `for`/`if`, no bundle references in port
lists. Consistency with the language spec (Complex/UInt/SInt as library bundles, two discipline
kinds, `@` attributes, unknown-event-is-error, latch warning) is assumed throughout.

### 1. Two phases

| Phase | Where | What |
|-------|-------|------|
| Elaboration | `mod` body, type annotations, structural control | resolved once into a fixed netlist |
| Solve | `analog`/`digital` | evaluated by the NR / event-driven engine |

A solve value never controls elaboration structure; runtime topology is a switch branch over a
static netlist.

### 2. Entry point

```rust
let source = parse_str(input)?;
let program = elaborate(source)?;   // inject stdlib → register items → validate → elaborate
```

Steps: prepend stdlib; register items (disciplines, bundles, enums, modules, behaviors, fns,
capabilities, impls, consts) into symbol tables; validate (§11); elaborate in dependency order.

### 3. Constant evaluation

Elaboration-position expressions (array dims, structural `for`/`if`, param defaults, const args,
`const`) must be compile-time constant.

`ConstVal = Int | Nat | Real | Bool | Str`. `ConstEnv` is a scoped name→ConstVal stack; each
`for` iteration pushes/pops the loop var. Evaluatable: literals (incl. SI-suffixed and
`_`-separated numerics), named bindings, unary `-`/`!`, binary arithmetic/comparison/bitwise,
`if/else`, block-with-trailing-value. Anything else (general calls, field access, comprehension)
is `NotConst`.

### 4. Type resolution

Resolves to `ElabNetType` or `ElabValueType`.

Primitive value types: `Real`, `Natural`, `Integer`, `Boolean`, `Quad`, `Str`. (`Complex`,
`UInt[N]`, `SInt[N]` are library bundles, resolved as bundles — not primitives.)

Disciplines resolve to `ElabNetType::Discipline(name)` of one of two kinds: **conservative**
(potential+flow, KCL) or **storage** (`storage T` + optional `resolve`). Enums →
`ElabValueType::Enum`. Arrays `Type[N]` evaluate `N` via `ConstEnv::eval_nat`; a free dimension
is an error.

A bundle is **net-capable** iff every field (recursively) is a net type; it may type a port and
is expanded (§7). A value-field bundle used as a net type is `NotNetCapable`. `Type`/`Net` are
root capabilities, satisfied implicitly.

### 5. Structural elaboration (mod body)

`for` unrolls (both bounds `Nat`); `if (const)` folds, emitting only the taken branch. `$assert`
in a `mod` body is an elaboration-time check. Errors: `<+`/`<-` in a `mod` body
(`ContribInModBody`/`ForceInModBody`).

V1: loop termination is not proven — the elaborator recurses with the loop var bound; a hard
depth limit is the intended backstop (else stack overflow on runaway const recursion).

### 6. Monomorphization

Const-param modules (`mod Dac[N]`) elaborate on instantiation: substitute `N`, elaborate, cache
under a mangled name (`Dac[8]`→`Dac__8`, `Grid[4,4]`→`Grid__4_4`). Type params (`<T: Add+Net>`)
substitute via a `type_subst` map; uninstantiated generics keep generic bodies, monomorphized on
demand. Generic `fn`s keep generic bodies; full inlining is the type checker's job.

### 7. Bundle expansion

A net-capable bundle port expands to one `ElabPort` per field, `{port}_{field}`:

```
input inp : DiffPair  →  input inp_p : Electrical,  input inp_n : Electrical
```

Field-access (`inp.p`) is preserved in behavior; the type checker rewrites to expanded names.

### 8. Behavior elaboration

Same const machinery, per-block rules. Behavioral `for` unrolls (elaboration-constant bound;
runtime loop is an error). Const `if` folds; runtime `if` is kept. `match` arms are kept
(type-checker does exhaustiveness). Latch inference (a `var` read unassigned on some path) raises
a warning by default; register inference (clocked `@`) is silent.

### 9. Event system

The parser emits `EventSpec::Named { name, arg }`; the elaborator resolves each against the
`EventRegistry` — events are extensible. Built-ins: `posedge`/`negedge`/`change` (digital),
`cross`/`above` (analog), `initial`/`final` (both), `timer` (both). Combine via `|`.

Domain validation: digital-edge in `analog` → `DigitalEventInAnalog`; analog-crossing in
`digital` → `AnalogEventInDigital`. **An unrecognized event name is `UnknownEvent` (a hard
error)** — no silent fallback.

Custom event (registry):

```rust
impl EventKind for SampleEvent { fn name(&self) -> &str { "sample" } fn is_digital_edge(&self) -> bool { true } }
elaborator.events_mut().register(SampleEvent);
```

### 10. Standard library (injected)

Capabilities: `Add Sub Mul Div` (one op method each), `Eq`, `Ord : Eq` (`lt`), `BitAnd BitOr
BitXor Not`, `Number : Add+Sub+Mul` (default `double`). Operator sugar: `+ - * / == < & | ^ !` →
`add sub mul div eq lt bitand bitor bitxor not`.

Collections: `map<T,U>(xs: T[N], f) -> U[N]`, `reduce<T>(xs: T[N], op) -> T` (divide-and-conquer,
base `N==1`), `concat`. Numeric bundles `UInt[N]`/`SInt[N]`/`Complex`. All are elaboration-time
generators: called with concrete `N`, they unroll to fixed structure.

### 11. Validation catalog

| Error | Trigger |
|-------|---------|
| `ContribInDigital` / `ContribInModBody` | `<+` in `digital` / `mod` body |
| `ForceInModBody` | `<-` in `mod` body |
| `AnalogEventInDigital` / `DigitalEventInAnalog` | event/block domain mismatch |
| `UnknownEvent(name)` | event name not registered |
| `UnknownAttrSchema(name)` | `@schema` not registered by any plugin |
| `NotNetCapable(name)` | value-field bundle used as net type |
| `UndefinedType` / `UndefinedModule` | name not found |
| `MissingConstParam { param, module }` | wrong const-arg count |
| `ConstEval { context, source }` | const expression failed |

Diagnostics (not errors): `InferredLatch(var)` — warning by default.

### 12. Future work

Import resolution (`use` stored, not resolved); full type-param monomorphization; bundle
field-access rewrite (`inp.p`→`inp_p`); operator-desugar pass; elaboration depth counter;
attribute-overlay merge from `bench`.

---

