# Part II — Elaboration

Elaboration transforms a parsed source file into a resolved, monomorphic design: no
generic parameters, no unresolved constants, no structural `for`/`if`, no bundle
references in port lists. This Part defines how each kind of construct is resolved and
what is rejected along the way.

Elaboration is the first of PHDL's three execution contexts (§I-12). It is **pure and
total**: the same source plus the same staged overrides always produce the same design.
A solve value never controls elaboration structure — runtime topology is a switch
branch over a static netlist, not structural change.

## Contents

- §1 The elaboration contract
- §2 Entry point and pipeline
- §3 Constant evaluation
- §4 Type resolution
- §5 Structural elaboration (`mod` body)
- §6 Monomorphization
- §7 Bundle expansion
- §8 Behavior elaboration
- §9 Event system
- §10 Standard library (injected)
- §11 Validation catalog (master)
- §12 Diagnostics

---

## §1 The elaboration contract

Elaboration takes a parsed AST and produces a fully-resolved design graph. After
elaboration, every generic has been substituted with a concrete type, every structural
loop has been unrolled, every const expression has been evaluated, and every bundle
port has been expanded into individual nets. The result is a flat, concrete structure
that the codegen and the interpreted context (Part III) can consume without further
resolution.

The contract is one-directional: elaboration reads the source and produces structure;
it never reads runtime values. A voltage, a current, a digital state — none of these
exist at elaboration time. This is what makes the structure statically analyzable: the
netlist is fixed before any current flows.

## §2 Entry point and pipeline

The elaborator runs a fixed pipeline:

1. **Inject stdlib** — prepend the standard library (disciplines, capabilities, numeric
   bundles, math fns, prelude constants) so every source file sees the same base scope.
2. **Register items** — enter every top-level declaration (disciplines, bundles, enums,
   modules, behaviors, fns, capabilities, impls, consts) into symbol tables.
3. **Validate** — run the checks cataloged in §11.
4. **Elaborate in dependency order** — resolve types, unroll structure, monomorphize,
   expand bundles, elaborate behavior bodies.

The output feeds two consumers: the codegen (which lowers it to JIT-compiled analog
kernels and event-driven digital models) and the interpreted context (which tree-walks
bench fns over the design graph).

## §3 Constant evaluation

Expressions in elaboration position — array dimensions, structural `for`/`if` bounds,
parameter defaults, const arguments, `const` declarations — must be compile-time
constants.

A `ConstEnv` is a scoped name→value stack. Each `for` iteration pushes the loop variable
onto the stack and pops it when the iteration ends. The evaluator handles:

- Literals, including SI-suffixed and `_`-separated numerics.
- Named bindings (params, loop variables, consts).
- Unary `-` and `!`, binary arithmetic, comparison, bitwise.
- `if`/`else` (the condition must itself be const).
- Block with a trailing value.

Anything else — general function calls, field access, comprehensions, runtime values
— is rejected as `NotConst`. This keeps the elaboration phase a total pure evaluator:
no side effects, no unbounded computation, no dependence on solve-time state.

```
ConstExpr      ::= ConstOrExpr
ConstOrExpr    ::= ConstAndExpr { "or" ConstAndExpr }
ConstAndExpr   ::= ConstNotExpr { "and" ConstNotExpr }
ConstNotExpr   ::= "not" "(" ConstExpr ")" | ConstCompare
ConstCompare   ::= ConstUnary [ CmpOp ConstUnary ]
ConstUnary     ::= ("!"|"-") ConstUnary | ConstPostfix
ConstPostfix   ::= ConstPrimary { "[" ConstExpr "]" }
ConstPrimary   ::= Literal | Ident | "(" ConstExpr ")" | BlockWithValue
```

**Validation.** A const expression that cannot be evaluated → E2001 (`ConstEval`).

## §4 Type resolution

Every type annotation in the source is resolved to either a value type
(`ElabValueType`) or a net type (`ElabNetType`).

**Primitive value types:** `Real`, `Natural`, `Integer`, `Boolean`, `Quad`, `String`.

**Net types** resolve as disciplines of one of the two kinds (Part I §6.2):
conservative (potential + flow, KCL) or storage (`storage T` + optional `resolve`).

**Enums** resolve to `ElabValueType::Enum`.

**Arrays** `Type[N]` evaluate `N` via the const evaluator; a free (unset) dimension is
an error.

**Bundles** resolve recursively. A bundle is **net-capable** iff every field
(recursively) is a net type. A net-capable bundle may type a port or wire; a value-
field bundle used as a net type is rejected. `Type` and `Net` are root capabilities —
every value type satisfies `Type` implicitly, every net type satisfies `Net`.

**Validation.** E2002 (`UndefinedType`); E2003 (`UndefinedModule`); E2004
(`NotNetCapable`).

## §5 Structural elaboration (`mod` body)

The `mod` body is where structure is built. The elaborator processes each statement:

- **`for` loops unroll** — both bounds must evaluate to `Nat`. Each iteration emits its
  statements with the loop variable substituted by its concrete value. The result is
  as if the user had written every iteration by hand.
- **`if (const)` folds** — the condition is evaluated; only the taken branch is emitted.
  A runtime-dependent condition in a `mod` body is an error.
- **`$assert(cond, msg)`** is evaluated immediately. If `cond` is false, elaboration
  fails with the message.

After structural elaboration, the `mod` body contains no loops and no `if` — only
concrete declarations and instances.

**Validation.** E2006 (`ContribInModBody`); E2007 (`ForceInModBody`); E2001 (`ConstEval`
on a non-const range or condition).

## §6 Monomorphization

A const-param module (`mod Dac[N]`) is elaborated on each instantiation with concrete
arguments. The elaborator substitutes the const params, elaborates the body, and caches
the result under a mangled name: `Dac[8]` → `Dac__8`, `Grid[4,4]` → `Grid__4_4`. Two
instantiations with the same arguments share one monomorphization.

Type-param modules (`mod Adder <T: Add + Net>`) substitute via a type map. Uninstantiated
generics keep their generic bodies (for reflection); monomorphization happens on demand
when a concrete instantiation is needed.

Generic `fn`s keep their generic signatures for reflection; the type checker inlines
them at each call site with the concrete types substituted.

## §7 Bundle expansion

A net-capable bundle port expands to one individual port per field. The expanded ports
are named `{port}_{field}`:

```
input inp : DiffPair   →   input inp_p : Electrical,  input inp_n : Electrical
```

Field-access expressions (`inp.p`) in behavior bodies are preserved in the AST; the type
checker rewrites them to the expanded names during lowering. From the solver's
perspective, every net is scalar — there are no "bundle nets," only individual nets
that happened to be declared together.

## §8 Behavior elaboration

`analog` and `digital` bodies are elaborated with the same const machinery, but with
per-block rules:

- **Behavioral `for` unrolls** — the bound must be an elaboration constant. A runtime
  loop (iterating a value that depends on solve state) is an error.
- **Const `if` folds** — a condition that can be evaluated at elaboration time is
  folded; only the taken branch is kept. A runtime `if` is preserved as a branch in the
  lowered body.
- **`match` arms are kept** — the type checker performs exhaustiveness analysis but
  does not fold the match (the scrutinee is a runtime value).
- **Latch inference** — a `var` read on a path where it was not assigned raises a
  warning by default (`InferredLatch`). Register inference (a `var` updated in a
  clocked `@` block) is silent — it is the intended idiom.

## §9 Event system

Events are resolved against an extensible registry. The parser emits an `EventSpec`
node for each `@` expression; the elaborator resolves the event name against the
registry and validates that the event class is legal in the enclosing block's domain.

**Built-in events:**

| Name | Class | Fires |
|------|-------|-------|
| `posedge(sig)` | digital edge | rising edge of `sig` |
| `negedge(sig)` | digital edge | falling edge of `sig` |
| `change(sig)` | digital edge | any change of `sig` |
| `cross(expr)` | analog crossing | zero crossing of `expr` |
| `above(expr)` | analog crossing | one-shot level crossing |
| `timer(period)` | analog | periodic, every `period` seconds |
| `initial` | lifecycle | once, at the start of the analysis |
| `final` | lifecycle | once, at the end (diagnostics only) |

Events combine via OR: `@ (posedge(a) | posedge(b)) { ... }`. The `when (cond)` clause
gates the event body on a level condition.

**Custom events** can be registered by plugins through the layer-2 extension mechanism
(Part I §14). A custom event implements the `EventKind` trait, declaring its name and
its class (digital edge, analog crossing, etc.); the elaborator validates it the same
way as built-ins.

**Validation.** E2008 (`UnknownEvent` — name not registered); E2009
(`AnalogEventInDigital` — an analog event like `cross` inside a `digital` block); E2010
(`DigitalEventInAnalog` — a digital edge like `posedge` inside an `analog` block). An
unrecognized event name is always a hard error — there is no silent fallback.

## §10 Standard library (injected)

Every compilation unit is prepended with the standard library, so every source file
sees the same base scope. The stdlib provides:

**Disciplines:** `Ground` (reference). Storage-digital: `Bit`, `Logic`, `DDiscrete`.
Conservative: `Electrical`, `Magnetic`, `Thermal`, `Kinematic`, `KinematicV`,
`Rotational`, `RotationalOmega`. Storage-`Real`: `Voltage`, `Current`.

**Constants:** math constants (`M_PI`, `M_E`, `M_SQRT2`, ...); physical constants
(`P_Q` elementary charge, `P_K` Boltzmann, `P_C` speed of light, ...).

**Capabilities:** `Type`, `Net` (root markers); `Add`, `Sub`, `Mul`, `Div`, `Eq`,
`Ord : Eq`, `BitAnd`, `BitOr`, `BitXor`, `Not`, `Number : Add+Sub+Mul`.

**Collections:** `map<T,U>(xs: T[N], f: fn(T)->U) -> U[N]`,
`reduce<T>(xs: T[N], op: fn(T,T)->T) -> T`, `concat(...)`.

**Numeric bundles:** `UInt[N]`, `SInt[N]`, `Complex`.

All stdlib items are elaboration-time generators — called with concrete `N`, they
unroll to fixed structure.

## §11 Validation catalog (master)

This is the master catalog of elaboration errors. Each error carries a code (`ENNNT`),
a variant name, and a trigger. The section column points to where the error is
explained.

### Elaboration errors

| Code | Variant | Trigger | Section |
|------|---------|---------|---------|
| E2001 | `ConstEval { context, source }` | a const expression failed to evaluate | I §7.4, II §3 |
| E2002 | `UndefinedType(String)` | a type name was not found | I §6, II §4 |
| E2003 | `UndefinedModule(String)` | a module name was not found | I §7, II §4 |
| E2004 | `NotNetCapable(String)` | a value-field bundle was used as a net type | I §6.5 |
| E2005 | `ContribInDigital` | a contribution `<+` appeared in a `digital` block | I §10.2 |
| E2006 | `ContribInModBody` | a contribution `<+` appeared in a `mod` body | I §10.2 |
| E2007 | `ForceInModBody` | a force `<-` appeared in a `mod` body | I §10.2 |
| E2008 | `UnknownEvent(String)` | an event name was not registered | I §10.4 |
| E2009 | `AnalogEventInDigital(String)` | an analog event in a `digital` block | I §10.4 |
| E2010 | `DigitalEventInAnalog(String)` | a digital event in an `analog` block | I §10.4 |
| E2011 | `MissingConstParam { param, module }` | wrong const-argument count | I §7.3 |
| E2012 | `NotANetRef(String)` | a name does not resolve to a net | I §7.3 |
| E2013 | `WidthMismatch { module, lhs, rhs, lhs_w, rhs_w }` | net width mismatch on connection | I §7.3, I §13 |
| E2014 | `DisciplineCrossing { module, lhs, rhs }` | incompatible disciplines connected | I §13 |
| E2015 | `UnknownBundle(String)` | a bundle name was not found | I §6.5 |
| E2016 | `BundleFieldUnknown { bundle, field }` | an unknown bundle field was referenced | I §6.5 |
| E2017 | `BundleParamDefault { param, expected, found }` | wrong type for a bundle param | I §6.5 |
| E2018 | `BundleFieldNoDefault { param, bundle, field }` | a required field was omitted with no default | I §6.5 |
| E2019 | `BundleParamNameCollision(String)` | two params with the same name | I §6.5 |
| E2020 | `MultipleDrivers { module, net, discipline }` | a second driver on a single-driver net | I §6.3 |
| E2021 | `PrivateItem { item, owner_pkg }` | a private item accessed from another package | I §5.3 |
| E2022 | `UnknownAttrSchema(String)` | attribute schema name not a registered bundle | I §8 |
| E2023 | `AttrSchemaField { schema, field, reason }` | attribute argument does not match the schema's bundle field | I §8 |
| E2024 | `NonExhaustiveMatch { module, missing }` | a `match` does not cover every variant | I §10.3 |
| E2999 | `Other(String)` | catch-all for custom diagnostics | — |

### Reflection errors

| Code | Variant | Trigger |
|------|---------|---------|
| E3001 | `NotFound` | a name or path was not found in the POM |
| E3002 | `NotSettable` | an attribute is read-only |
| E3003 | `TypeMismatch` | a value's type does not match the expected type |
| E3004 | `OutOfRange` | a value is outside the accepted range |
| E3005 | `MultipleDrivers` | staging an override would create a multi-driver conflict |
| E3999 | `Other(String)` | catch-all |

(Covered in Part IV §3.)

### Selector errors

| Variant | Trigger |
|---------|---------|
| `EmptySelector` | empty selector string |
| `ExpectedDoubleColon` | missing `::` after an axis name |
| `ExpectedNodeTest` | missing node test after an axis |
| `UnknownAxis(String)` | axis name not in the table |
| `AxisNotImplemented(Axis)` | axis exists but is not yet lowered |

(Covered in Part IV §13.)

### Codegen errors

| Variant | Trigger |
|---------|---------|
| `ModuleNotFound(String)` | module name not found in the lowered program |
| `Invalid(String)` | IR validation failed |
| `Module(String)` | Cranelift module error |
| `Unsupported(String)` | an IR construct that has no lowering yet (fail-loud) |
| `ConstEval(String)` | constant evaluation failed at lowering |
| `Function(String)` | function lowering failed |

Every unimplemented lowering is a named `Unsupported` error — nothing silently compiles
to `0.0` or a no-op. This is the fail-loud contract.

### Bench task enforcement

A `bench` fn may call only tasks in the `bench_task_implemented` allowlist; otherwise
the call is an elaboration error before any analysis runs. The allowlist contains:
`assert`, `info`, `warn`, `error`, `fatal`, `display`, `op`, `tran`, `ac`, `noise`,
`write`. (Covered in Part III §5.)

## §12 Diagnostics

Diagnostics are warnings, not errors — they report potential issues but do not stop
elaboration. The one diagnostic defined today:

- **`InferredLatch(var)`** — a `var` in a `digital` block is read on a path where it
  was not assigned, meaning it retains its previous value (unintended storage). Raised
  as a warning by default; can be promoted to an error per-project or via an attribute.
