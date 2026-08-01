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
- §8.1 Constraint elaboration
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

The output feeds the codegen (which lowers it to JIT-compiled analog kernels and
event-driven digital models); host APIs (Part VIII) drive analyses over it.

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

### §8.1 Constraint elaboration (dv-core DVC-04/05/07/09)

A `constraint Name { … }` block (Part I §10.6) is elaborated against the module
named `Name` with the same name-resolution machinery as a behavior body, then
**attached** to that module on the POM. Like a behavior body, the block stays
in its *authored* surface form on the POM — the parse AST's `ConstraintStmt`s
plus side tables the elaborator adds (resolved `var` value types, per-statement
classification). Nothing about the constraint crosses a flattening boundary
(MD-25): the POM holds exactly one authored block per authored module.

**Resolution order.** Constraint blocks register at step 2 of the pipeline
(§2) and elaborate at step 4, after their target module is fully lowered:

1. **Attach** — every registered `constraint Name { … }` resolves `Name`
   against the authored-module table. A constraint naming a non-existent
   module fails loud; a constraint on a module that has no `analog`/`digital`
   body is legal (a structural module can carry SOA intent).
2. **Resolve references** — every name inside the block (nets, ports,
   instance paths, `var`s, helpers, callables) resolves through the same scope
   as a behavior body, plus `headers/constraints.phdl`'s helpers (§10). An
   instance path `m1.d` reaches the *port* net of child instance `m1` through
   the authored tree; a child's *internal* node fails loud — it crosses
   encapsulation and depends on optional parameters (DVC-07 AC3).
3. **Type-check** — a `require` predicate must be `Boolean`; a `target`'s
   operands must be `Real`; a `var`'s annotation must resolve to a value type.
   Duplicate statement labels inside one block fail loud.
4. **Classify** — every leaf statement (§10 of the helpers' classes) is
   classified `Pointwise` or `Reduced` from the **declarations** in
   `headers/constraints.phdl` (§10), never inferred from a use site.

**The pointwise/reduced classification (DVC-09).** A constraint quantity
exists either at every evaluation point of an analysis (*pointwise*) or only
once the analysis has run to completion (*reduced*) — design §C2. The class is
a property of the **declaration**, carried by `headers/constraints.phdl`'s
`@quantity(class, analysis)` marker; the elaborator reads it transitively over
every expression. The rules:

- A `var`/`target` expression's class is the class of its subexpressions:
  pointwise arithmetic over pointwise terms stays pointwise; **any** reduced
  subexpression makes the whole expression reduced.
- A `require` is always classified pointwise — a reduced predicate is a loud
  elaboration error ("a reduced quantity is checkable once per analysis, not
  per point"), because `require` is a per-point check by definition.
- The analysis a helper belongs to (`ac`, `dc`, `tran`) is recorded on the
  statement, so a later pass can refuse a cross-analysis read.

Only pointwise quantities lower to a per-point margin kernel (Part VII §3.5);
reduced ones are computed host-side as reductions over the points the solver
returns (Part VIII §7).

**Monomorphized carriage (DVC-05).** A const-param module (`mod Dac[N]`)
elaborates on each instantiation, and a `constraint Dac { … }` block attaches
to every monomorphized variant (`Dac[8]` → `Dac__8`) exactly as an
`analog`/`digital` body does — the block is authored once, carried by every
variant. The POM holds exactly one authored block per authored module; a
selector walk (Part IV) reaches it on both. No accessor reads a flattened
structure.

**`tol` distribution resolution (DVC-02).** A `param`'s optional `tol` clause
(Part I §7.2) is resolved against `headers/statistics.phdl` (§10): the
distribution name must match a declared `extern fn`, the keyed arguments
evaluate as elaboration constants in the param's scope (sibling params
already bound), and the `global` marker is recorded on the POM as the
process-vs-mismatch kind. A `tol` on a non-`Real` param, or an undeclared
distribution name, fails loud at elaboration. The clause is inert at solve —
sampling is `dv-gradients`' scope.

**`cover` bin resolution and the elaboration-time cross cap (dv-tester
DVT-42, D10).** A `cover <name> : <expr> bins […]` / `… in {…}` statement
resolves and classifies exactly like a `var`/`target` expression (steps 2–4
above): `<expr>` must be `Real` and **pointwise** — the same reduced-quantity
restriction `require` carries, for the same reason ("a reduced quantity is
checkable once per analysis, not per point"). Its `CoverBins` (a literal-edged
range `[lo:step:hi]` or an explicit literal set `{e1, e2, …}`) resolve to a
`ResolvedCoverBins` and a concrete **bin count**, both computed at
elaboration since every bin edge is a literal — nothing about a `cover`
point's own bin count needs a solve. The point's name and bin count are
recorded in the block's local scope so a later `cover cross` in the same
block can look them up by label.

A `cover cross <name> : <point1>, <point2>, …;` names two or more
previously-declared `cover` points, in the same block, by label — naming an
undeclared point fails loud, listing the undeclared label and the points
that must be declared first. Its **joint bin count is the product of each
named point's own bin count**, and because every factor is already known
statically, the product is checked **at elaboration, before anything
allocates**: exceeding `ElabContext::cover_bin_cap` (a field on the
elaboration context an embedding host may raise, defaulting to 4096 — an
agent-decided default, since `spec.md`/`ideal.md` D10 left the exact number
open) is a loud elaboration error naming the declared joint count and the
cap, rather than a runtime allocation of however many bins the product
turned out to be. This is the whole point of resolving bin counts at
elaboration rather than at the first solve: a combinatorially oversized
`cover cross` (crossing several fine-grained ranges) is refused before a
single coverage counter exists, not discovered as a multi-gigabyte
allocation mid-run.

**Event-block scoping (DVC-19/20).** A `@ … { … }` block inside a constraint
reuses the behavior-body `EventSpec` production and `EventRegistry` (§9). The
elaborator resolves each term and rejects names that are not analysis-scope
kinds inside a constraint scope (`posedge`/`cross`/`timer` belong in a
behavior body, not a constraint scope). `after = …`/`dur = …` window named-args
evaluate as elaboration-constant `f64`s; a non-constant window expression fails
loud. The scope's analyses and window are unioned into every enclosed leaf's
classification, so a `require` inside `@ tran(after = 5u, dur = 10u) { … }`
records `["tran"]` and the resolved window — the runtime evaluator gates each
evaluation point on both.

**Validation.** Every constraint-elaboration failure surfaces today as a
descriptive `ElabErrorKind::Other` (E2999 in the catalog below) — the
constraint block is the one place where the parse/elaboration surface grew
without an assigned E20xx code, deliberately, because every check names the
constraint, the module, and the offending site in its message. The full list
of fail-loud clauses is in §11.1.

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
| `timer(period)` / `timer(period, phase)` | analog | periodic, every `period` seconds; optional `phase` offsets the first fire (see Part V) |
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

**Statistical distributions (DVC-02):** `headers/statistics.phdl` declares the
four distribution spellings a `tol` clause (Part I §7.2) may name — `gauss`
and `uniform`, each with absolute and relative argument forms distinguished by
the keyed-argument name. The full table is Part V §6.2.

**Constraint helpers (DVC-09):** `headers/constraints.phdl` declares the
helpers a `var`/`target` expression may call, each carrying its
pointwise/reduced class and its analysis in a `@quantity(class, analysis)`
marker the elaborator reads. The full table is Part V §6.2.

All stdlib items are elaboration-time generators — called with concrete `N`,
they unroll to fixed structure.

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

### §11.1 Constraint-elaboration fail-loud catalog (dv-core)

Every constraint-related failure surfaces as `E2999 Other(…)` today. Each
message names the constraint, the module, and the offending site explicitly,
so the catch-all carries the descriptive load an E20xx code would. Gathered
in one place for cross-reference (the rule column points to the spec section
that defines the rule):

| Rule | Trigger wording | Section |
|------|-----------------|---------|
| `tol` on a non-`Real` param | `` `tol` on param `{p}` requires a `Real` param, found `{ty}` `` | I §7.2, II §8.1 |
| `tol` on a bundle-typed param | `` `tol` on param `{p}` … found bundle-typed `{ty}` `` | I §7.2 |
| `tol` names an undeclared distribution | `` `tol` on param `{p}` names undeclared distribution `{d}` … `` | I §7.2, II §8.1 |
| `constraint` on a non-existent module | `` constraint `{name}` names a module that does not exist in the design `` | II §8.1 |
| unknown net/port/var inside a constraint | `` constraint `{c}` on module `{m}` (site): unknown name `{n}` `` | I §10.6, II §8.1 |
| unknown instance-path segment | `` constraint `{c}` on module `{m}` (site): unknown instance `{i}` `` | I §10.6 |
| child internal node read | `` `{inst}.{field}` is an internal node of `{child}` — a constraint reads a child port, not its internals `` | II §8.1 |
| instance path deeper than one level | `` unsupported instance path — a constraint reaches one level: `instance.port` `` | II §8.1 |
| undeclared helper or function | `` undeclared helper or function `{n}` — constraint helpers are declared in headers/constraints.phdl, math functions in headers/math.phdl `` | I §10.6, V §6.2 |
| missing `@quantity` argument | `` constraint helper `{h}`: `@quantity` requires a `{key} = "…"` string argument `` | V §6.2 |
| bad `@quantity` class | `` constraint helper `{h}`: `@quantity` class must be `pointwise` or `reduced`, found `{x}` `` | V §6.2 |
| duplicate helper declaration | `` constraint helper `{n}` is declared more than once `` | V §6.2 |
| non-`Boolean` `require` predicate | `` constraint `{c}` on module `{m}`: `require` predicate must be `Boolean`, found `{ty}` `` | I §10.6 |
| non-`Real` `target` operand | `` constraint `{c}` on module `{m}`: `target` operand must be `Real`, found `{ty}` `` | I §10.6 |
| missing `var` annotation | `` constraint `{c}` on module `{m}`: `var` `{n}` requires a type annotation `` | I §10.6 |
| duplicate statement label | `` constraint `{c}` on module `{m}`: duplicate statement label `{n}` `` | I §10.6 |
| reduced quantity in a pointwise scope | `` constraint `{c}` on module `{m}`: a reduced quantity is checkable once per analysis, not per point `` | I §10.6, II §8.1 |
| `cover cross` names an undeclared point | `` `cover cross `{name}`` names undeclared cover point `{point}` — every crossed point must be declared with `cover {point} : … bins […]` or `… in {…}` earlier in the same block `` | I §10.6, II §8.1 (DVT-42) |
| `cover cross` joint bin count exceeds the cap | `` `cover cross {name}`'s joint bin count ({n}) exceeds the elaboration cap ({cap}) — cross fewer points, coarsen a range, or raise `ElabContext::cover_bin_cap` `` | I §10.6, II §8.1 (DVT-42, D10) |
| bad cast or call form in a constraint | `` unsupported {form} `{x}` in a constraint `` | II §8.1 |
| `none` literal where a value type is required | `` `none` has no value type in a constraint `` | II §8.1 |
| `@` block names no analyses | `` constraint `{c}` on module `{m}`: an `@` event block must name at least one analysis `` | I §10.6 |
| `@` block term illegal in a constraint scope | `` `{term}` is valid only inside a `constraint` block's `@` event block, not a runtime behavior body `` (and the reverse) | I §10.6 |
| non-const `after`/`dur` window expression | `` constraint `{c}` on module `{m}` (@ event block window): {e} `` | I §10.6 |

A constraint block that survives elaboration has cleared every check above;
the device-compile stage assumes a well-classified, well-typed block.

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

## §12 Diagnostics

Diagnostics are warnings, not errors — they report potential issues but do not stop
elaboration. The one diagnostic defined today:

- **`InferredLatch(var)`** — a `var` in a `digital` block is read on a path where it
  was not assigned, meaning it retains its previous value (unintended storage). Raised
  as a warning by default; can be promoted to an error per-project or via an attribute.
