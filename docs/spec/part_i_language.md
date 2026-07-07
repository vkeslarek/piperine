# Part I — Language Specification

This Part is the normative core of PHDL: the lexical form, the type system, the module
and net model, behavior (analog/digital), functions, attributes, system tasks, the
extension model, and the phase model. It defines what every PHDL program must satisfy
to be well-formed.

The grammar defined here is LL(1) after one inline left-factoring in instance parsing.
Semantic distinctions that the parser cannot make on a single token of lookahead (value
vs net type, access vs call, `[Expr]` as const-arg vs array-dim) are deferred to the
type checker; the grammar produces a single ambiguous AST and the elaborator resolves
it. The full consolidated grammar is in Appendix B.

## Contents

- §1 Goals and governing rules
- §2 Core model: value vs net
- §3 Naming conventions
- §4 Lexical structure
- §5 Top-level items and packages
- §6 Types
- §7 Modules
- §8 Attributes (metadata)
- §9 Functions
- §10 Behavior
- §11 System tasks (`$`-syscalls) — overview
- §12 Phase model
- §13 No-Magic rule
- §14 Extension model
- §15 Rejected features
- §16 Validation rules (Part I consolidated)

---

## §1 Goals and governing rules

PHDL is governed by seven rules. Every language decision in this Part follows from one
or more of them; every rejected feature in §15 is rejected because it violates one.

**One mixed-signal model.** Continuous and discrete hardware share module, type, and
function constructs. The boundary between them is explicit and checked — you cannot
accidentally connect an analog node to a digital wire without a converter. There is one
language, not "an analog HDL" and "a digital HDL" glued together.

**No-Magic.** Type conversion, domain crossing, and driver resolution are never inserted
implicitly. The source states them. If you want to convert a `Real` voltage to a `Quad`
logic level, you write a converter module; the compiler does not invent one. If two
drivers contend on a single-driver net, that is an error, not an implicit resolution.

**Well-formed by construction.** A program that type-checks elaborates to a structurally
valid netlist: matched widths, single-driver where required, no implicit domain
crossings. The type checker is not advisory — if it accepts the program, the netlist is
sound.

**Compile-time by default.** Anything resolvable before the run — const folding, dead
branches, monomorphization, analysis specialization — is resolved then. The solver
receives a specialized, already-resolved device, not a generic interpreter.

**Tier independence.** Tier-1 code (leaf devices, RTL) is readable and writable with no
knowledge of the tier-3 machinery (capabilities, generics, higher-order functions). A
construct that forces generic or capability syntax into a leaf device model is a defect.
A resistor model should read like a resistor model, not like an exercise in type theory.

**No-Bloat (burden of proof).** A feature must demonstrate it cannot live in an
extension layer (§14) before it may touch the core grammar. The grammar is closed by
default; growth happens above it, not inside it.

**Machine-writable.** The grammar is LL(1) — one token of lookahead suffices to decide
every parse. `todo!` is a legal placeholder that type-checks, so code generators can
emit structure first and fill bodies later.

---

## §2 Core model: value vs net

PHDL separates two kinds of type: **value types** and **net types**. This separation is
the foundation of the language's mixed-signal discipline — it is what makes domain
crossings explicit (§13) and what lets the type checker prove structural well-formedness
at compile time (§1: well-formed by construction).

### 2.1 Value — pure data

A value is computed, passed, stored, and read. It has no direction, no driver, and no
physical meaning. Value types live in:

- `param` — an elaboration-time constant, settable by the parent module at instantiation.
- `var` — a mutable binding, scoped to its block.
- expressions — the operands and results of computation.
- `fn` results — the return value of a pure function.

The primitive value types are `Real`, `Natural` (indices, widths, counts), `Integer`,
`Boolean` (2-state), `Quad` (4-state: `0`/`1`/`X`/`Z`), and `String` (diagnostics).
Composite value types — tuples, lists (`Vec<T>`), maps (`Map<K,V>`), options (`T?`) —
exist in the value layer for interpreted computation (Part III) and const evaluation.
`UInt[N]`, `SInt[N]`, and `Complex` are standard-library bundles (§6.5), not primitives.

A value can be stored, indexed, mapped, reduced — all the ordinary operations of a
pure functional layer. What a value *cannot* do is carry a signal between modules, or
participate in Kirchhoff's laws. That is the job of a net.

### 2.2 Net — a signal carrier

A net carries a value through the simulation, but it also has **resolution** — the rule
for how multiple drivers combine. A net type is either a **discipline** (§6.2) or a
**net-capable bundle** (§6.5). Every net is defined by two things:

1. **Storage** — the value type(s) the net carries.
   - For a conservative discipline like `Electrical`, there are two quantities: a
     *potential* (`Real`, measured in volts) and a *flow* (`Real`, measured in amperes).
     Both are part of the net's storage; KCL relates them.
   - For a storage discipline like `Bit`, the storage is a single `Boolean`. For `Logic`,
     it is a `Quad`.
   - For a storage discipline like `Voltage`, the storage is a `Real` — a signal-flow
     potential without an associated flow.

2. **Resolution** — how drivers combine.
   - A **conservative** discipline resolves by KCL (Kirchhoff's current law), always and
     implicitly. Every connected terminal contributes its flow to the node; the solver
     finds the potential where they balance.
   - A **storage** discipline resolves by a single-driver rule by default: one driver
     writes the value, and a second driver is an error. An optional `resolve` clause
     (§6.3) permits multiple drivers: `tri` (tri-state, needs high-impedance `Z`),
     `wired-or`, `sum`, `average`, etc.

A net lives in **ports** (the module's external interface) and **wires** (internal
connections). It does not live in a `param` or a pure `var` — those carry values, not
signals.

### 2.3 Reading and driving

The boundary between value and net is bridged in two directions, both explicit:

- **Reading a net into a value** — access functions like `V(a,b)` read a voltage as a
  `Real`; a digital net is read directly as a `Boolean` or `Quad`. This is ordinary
  sampling and does not cross a discipline boundary.
- **Driving a value onto a net** — contributions (`<+`) and forces (`<-`) write a
  computed value onto a net. A contribution accumulates (multiple `<+` on the same
  branch add up); a force overwrites (single-driver).

What is forbidden is *connecting* two nets of incompatible disciplines without an
explicit converter module. Reading and driving are value/net interactions; connecting
is net/net, and that is where No-Magic (§13) applies.

### 2.4 Construct kinds

PHDL has seven construct kinds, each with a distinct role:

| Construct | Role |
|-----------|------|
| `mod` | Module shape: identity, ports, parameters, instances, structural body. The unit of hierarchy. |
| `bundle` | An aggregate of named fields, each of value or net type. May be net-capable (if all fields are net types) and type a port, or value-only and type a `param`/`var`. |
| `fn` | A pure value computation. Inlines at the call site; serves every context uniformly (elaboration, analog, digital, interpreted). |
| `capability` | A named contract of function signatures. Operators desugar to capabilities. Satisfied via `impl Cap for T`. |
| `impl` | Provides method bodies for a bundle, or an implementation of a capability. |
| `analog` / `digital` | Behavior blocks: the continuous and discrete engines that run during solve. |
| `bench` | An interpreted testbench attached to a module. Uses the same `fn`-body grammar; covered in Part III. |

Metadata attaches to any declaration via `@` attributes (§8). Attributes are inert —
they do not affect elaboration or simulation. They carry tool-specific intent (layout,
routing, floorplan, matching) for downstream tools.

### 2.5 Two phases by location

The location of a construct determines when it runs:

- A **`mod` body** runs at **elaboration time** — once, producing a fixed netlist.
  Structural `for`/`if`, parameter substitution, monomorphization, and const evaluation
  all happen here. The result is static structure: no hardware is created or destroyed
  after elaboration.
- An **`analog` or `digital` body** runs at **solve time** — evaluated by the
  Newton-Raphson or event-driven engine during each analysis. A solve value never
  controls elaboration structure: you cannot write a `for` loop whose bound depends on
  a voltage. Runtime topology (a switch opening and closing) is expressed as a switch
  branch over a fixed node set, not as structural change.
- A **`bench` body** runs at **interpretation time** — after elaboration, tree-walked
  over the elaborated design. Part III covers this context.

This phase separation is what makes PHDL statically analyzable: the structure is known
before any current flows.

---

## §3 Naming conventions

PascalCase: modules, bundles, value types, net types, disciplines, enums, capabilities.

snake_case: functions, methods.

lowercase/snake_case: ports, params, vars, fields, instances.

This convention is not merely stylistic — the selector (Part IV §8) relies on it to
disambiguate instance-type matches (PascalCase) from instance-name matches
(snake_case). A name's casing carries semantic information that tools depend on.

---

## §4 Lexical structure

### 4.1 Lexical grammar (EBNF)

```
Ident      ::= (letter|"_") { letter|digit|"_" }
RealLit    ::= Digits "." Digits [ ("e"|"E") ["+"|"-"] Digits ] [ SiSuffix ]
             | Digits ("e"|"E") ["+"|"-"] Digits [ SiSuffix ]
NatLit     ::= Digits [ SiSuffix ] | "0b" BinDigits | "0x" HexDigits
Digits     ::= digit { digit | "_" }                     -- '_' separators anywhere between digits
SiSuffix   ::= "T"|"G"|"M"|"k" | "m"|"u"|"n"|"p"|"f"|"a" -- case-sensitive; M=mega, m=milli
QuadLit    ::= "0q" ("0"|"1"|"X"|"Z")
StringLit  ::= '"' {char} '"'
SysCall    ::= "$" Ident
```

### 4.2 Comments, whitespace, and tokens

Comments: `//` line, `/* */` block. Whitespace is not significant beyond separating
tokens.

System names are a distinct token class: `$name` introduces a syscall (§11, Part V).
The `@` sigil prefixes both attributes (§8) and events (§10.4); the parser disambiguates
by position — an `@` at declaration-start is an attribute, an `@` at statement-start
inside a behavior body is an event.

### 4.3 Reserved spellings

The following identifiers are reserved at the parser level — they cannot be used as
variable, field, or type names because the parser interprets them as keywords in the
positions where they are expected:

`above`, `and`, `analog`, `bundle`, `capability`, `change`, `const`, `cross`,
`digital`, `discipline`, `else`, `enum`, `final`, `flow`, `fn`, `for`, `if`, `impl`,
`in`, `initial`, `inout`, `input`, `mod`, `negedge`, `none`, `or`, `output`, `param`,
`posedge`, `potential`, `pub`, `resolve`, `return`, `self`, `Self`, `storage`, `tri`,
`use`, `var`, `when`, `wire`, `bench`.

Contextual keywords (reserved only inside a `discipline` body): the resolve kinds `tri`,
`or`, `and`, `sum`, `avg`, `max`, `min`.

Lexer-level sigils: `$`, `@`, `<+` (contribution), `<-` (force), `?` (optional type;
pattern wildcard), `..` / `..=` (ranges), `::` (path), `=>` (match arm), `->` (fn
return), `|` (lambda; event OR), `=` (assignment).

### 4.4 Literals

- **`Real`** — `1.0`, `1.0e3`, `2u` (= 2e-6). SI suffixes are case-sensitive: `T G M k`
  multiply by 1e12…1e3; `m u n p f a` multiply by 1e-3…1e-18. `M` is mega, `m` is milli.
  Digit separators `_` may appear anywhere between digits: `1_000_000`, `4_2u`.
- **`Boolean`** — `0` (false) or `1` (true).
- **`Quad`** — `0q0`, `0q1`, `0qX`, `0qZ` (4-state logic values).
- **`String`** — `"text"`, used in diagnostics and attribute arguments.
- **Arrays** — element list `[a, b, c]`, repeat `[v; N]`, comprehension
  `[expr | i in 0..N]`; index `a[i]`, slice `a[lo..hi]`; nesting `Bit[8][16]` = 16 words
  of 8 bits.

`Boolean` widens to `Quad` implicitly (a 2-state value is a special case of 4-state);
all other casts are explicit (`real(x)`, `int(x)`, `bit(x)`).

---

## §5 Top-level items and packages

### 5.1 Grammar (EBNF)

```
CompilationUnit ::= { UseDecl | Item }
UseDecl   ::= "use" Path ";"
Path      ::= Ident { "::" Ident }
Item      ::= { Attribute } [ "pub" ] ItemKind
ItemKind  ::= ModDecl | BehaviorDecl | DisciplineDecl | BundleDecl | EnumDecl
             | CapabilityDecl | ImplDecl | FnDecl | ConstDecl | BenchDecl
ConstDecl ::= "const" Ident ":" Type "=" Expr ";"
```

### 5.2 Items

| Item | Purpose |
|------|---------|
| `discipline` | net type: storage + resolution (§6.2) |
| `bundle` | value/net aggregate (§6.5) |
| `enum` | enumerated value over a digital repr (§6.4) |
| `capability` | type contract, operator sugar (§6.6) |
| `fn` | pure value function (§9) |
| `const` | global compile-time constant |
| `mod` | module shape (§7) |
| `analog`/`digital` | module behavior (§10) |
| `impl` | bundle methods / capability impl (§6.5–§6.6) |
| `bench` | interpreted testbench (Part III) |

### 5.3 Packages and visibility

Packages are file/directory-based: a file or directory is a package; there is no
namespace declaration, no index file, no re-export. Items are private unless declared
`pub`; `use pkg::item` imports public items from another package — non-`pub` items
from a used file are not visible to the importer.

```phdl
// devices/passives.phdl → package devices::passives
pub mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1k; }
mod InternalHelper ( ... );   // private — not visible to importers

// top.phdl
use devices::passives;        // brings Resistor into scope; InternalHelper stays private
```

The standard library (`piperine::` namespace) is exempt from privacy filtering — its
items are always exported regardless of `pub`, matching the prelude injection model.

A `const NAME : T = expr;` declares a global compile-time constant, evaluated at
elaboration and usable wherever a param or literal is expected.

**Validation.** An unresolved `use` import is stored and deferred; a reference to an
unknown item at a use-site is E2002 (`UndefinedType`) or E2003 (`UndefinedModule`).
Privacy is enforced during `use` resolution: non-`pub` items from a used file are not
inlined, so referencing one from the importer yields E2002/E2003 (the item is not in
scope).

---

## §6 Types

### 6.1 Value types

The primitive value types:

| Type | Values | Notes |
|------|--------|-------|
| `Real` | floating-point | default numeric type in analog contexts |
| `Natural` | non-negative integer | indices, widths, counts; const-position |
| `Integer` | signed integer | |
| `Boolean` | `0` / `1` | 2-state logic |
| `Quad` | `0q0` / `0q1` / `0qX` / `0qZ` | 4-state logic, standard propagation |
| `String` | text | diagnostics only |

`UInt[N]`, `SInt[N]`, and `Complex` are **standard-library bundles** (§6.5), not
primitives — they are built from the same `bundle` + `capability` + `impl` machinery
available to any user. Primitives carry built-in operators via the capability system
(§6.6).

**Collections and composites** (interpreted `fn`-body grammar — bench and const-eval):

- **Tuples** `(a, b, ...)` — indexed `.0`/`.1`/...; `(e)` with no comma is a
  parenthesized group, not a 1-tuple.
- **Lists** `[a, b, ...]` / `[v; N]` / `[expr | i in a..b]` — a runtime `Vec<T>` in the
  value layer, with `.push(v)`, `.len()`, `.get(i) -> Option<T>`. The same array-literal
  syntax produces a fixed-size elaboration-constant `Array` in a `mod`/`analog`/
  `digital` context — which form applies follows from context.
- **Options** — `Option<T>` with `.is_some()`, `.is_none()`, `.unwrap()`,
  `.unwrap_or(default)`. The optional type `T?` is sugar: a trailing `?` marks a value
  that may be absent; `none` is the absent value. Read through `.is_present()` /
  `.get_or(default)`. On a scalar `param`, this lowers onto parameter-presence
  (`is_present` ≡ `$param_given`), so the choice is per-instance. Prefer `T?` over a
  sentinel default (`1e99`, `0`) + `$param_given`.
- **Sets** — `Set<T>` with literal `Set { a, b, c }` (`Set {}` is empty), `.insert(x)`
  (adds if not present), `.contains(x) -> Boolean`, `.len() -> Natural`, `.remove(x)`.
  Backed by a list with linear search (N is small; `Value` is not `Hash`/`Eq`-clean).
- **Results** — `Result<T, E>` represents either `Ok(T)` or `Err(E)`. No literal syntax —
  result values are produced by fallible operations (e.g. `Selection.one()` returns
  `Result` — `Ok` when exactly one match, `Err` when empty or ambiguous). Methods:
  `.is_ok() -> Boolean`, `.is_err() -> Boolean`, `.unwrap() -> T` (errors on `Err`),
  `.unwrap_or(default) -> T`, `.ok() -> Option<T>`, `.err() -> Option<E>`.
- **Maps** — `Map<K, V>` with literal `Map { key: value, ... }` (`Map {}` is empty),
  `.insert(k, v)`, `.get(k) -> Option<V>`, `.len()`. Keys compare by value (structural
  equality, small-N association list). Backs the `ic:`/`nodeset:` fields of analysis
  config bundles (Part III §7.1).

**Validation.** Using a collection literal in an `analog`/`digital` body where lowering
is not implemented fails loud at device-compile (never a silent stub).

### 6.2 Disciplines — net types

A discipline is one of exactly two kinds:

**Conservative** — declares a `potential` and a `flow` (each with optional named
attributes like unit and abstol). Resolves by KCL, always and implicitly. Read through
accessors: `V(a,b)` for potential, `I(a,b)` for flow, plus the declared nature names
(`Temp(th)`, `Pwr(th)`, etc.). A conservative net is a physical terminal pair.

**Storage** — declares one `storage` value type. Single-driver by default; read/driven
by name. An optional `resolve` clause (§6.3) permits multiple drivers.

```phdl
discipline Electrical { potential v : Real (unit = "V", abstol = 1e-6);
                        flow      i : Real (unit = "A", abstol = 1e-12); }
discipline Voltage  { storage Real; }        // signal-flow potential
discipline Bit      { storage Boolean; }
discipline Logic    { storage Quad; resolve tri; }
```

`Ground` is the predefined conservative reference node — the universal KCL sink.

```
DisciplineDecl ::= "discipline" Ident "{" { DisciplineItem } "}"
DisciplineItem ::= NatureDecl | StorageDecl | ResolveDecl
NatureDecl     ::= ("potential"|"flow") Ident ":" Type [ AttrList ] ";"
AttrList       ::= "(" NamedAttr {"," NamedAttr} ")"
NamedAttr      ::= Ident "=" Expr
StorageDecl    ::= "storage" Type ";"
```

**Validation.** A value-field bundle used as a net type → E2004 (`NotNetCapable`).
Multiple drivers on a single-driver storage net → E2020 (`MultipleDrivers`).

### 6.3 Resolution

```
ResolveDecl ::= "resolve" ("tri"|"or"|"and"|"sum"|"avg"|"max"|"min") ";"
```

- Conservative → KCL, implicit. No `resolve` clause applies.
- Storage default → single-driver. A second driver is E2020.
- `resolve` clause on `Quad` storage: `tri` (tri-state, needs `Z`), `or` (wired-or),
  `and` (wired-and).
- `resolve` clause on `Real` storage: `sum`, `avg`, `max`, `min` (numeric bus
  resolution).
- `Boolean` storage is single-driver only — it has no `resolve` clause.
- A vector net resolves per line, independently.

### 6.4 Enums

```
EnumDecl    ::= "enum" Ident [ ":" Type ] "{" EnumVariant {"," EnumVariant} [","] "}"
EnumVariant ::= Ident [ "=" Expr ]
```

An enum is an enumerated value over a digital representation. The optional `: Repr`
fixes the underlying digital net type; the default is `Bit[ceil(log2(count))]`. Values
default sequential from zero, continuing from the last explicit value.

```phdl
enum SwState { Open, Closed }                          // sequential → Bit[1]
enum Phase   { Idle = 0b00, P1, P2, P3 }               // explicit / continuing
enum OpCode : Bit[32] { Mov = 0, Add, Sub, Jmp = 16 }  // explicit repr
```

### 6.5 Bundles

A bundle is a named aggregate of fields, each of value or net type, with optional
defaults. A bundle is the "struct" of PHDL — it groups related data.

**Net-capable** iff every field (recursively) is a net type. A net-capable bundle may
type a port or wire; it is expanded field-by-field at elaboration (Part II §7).
Otherwise the bundle is value-only: it types a `param`, `var`, or `fn` argument.

A bundle port is **direction-agnostic**: one direction applies to the whole bundle.
Mixed-direction interfaces (some fields in, some out) are expressed as two bundles, not
one. Same-type net bundles connect field-by-field by name; a field is read or driven as
`b.field`.

Methods and constructors live in `impl`. A named constructor is an associated
`fn -> Self`. The literal syntax is `Name { .field = v }`; omitted fields take their
defaults.

```phdl
bundle Complex { re : Real = 0.0, im : Real = 0.0 }
impl Complex {
    fn polar(mag: Real, ang: Real) -> Self {
        return Complex { .re = mag*cos(ang), .im = mag*sin(ang) };
    }
}
```

```
BundleDecl ::= "bundle" Ident [ConstParams] [TypeParams] "{" [ Field {"," Field} [","] ] "}"
Field      ::= { Attribute } Ident ":" Type [ "=" Expr ]
```

**Validation.** E2004 (`NotNetCapable`); E2015 (`UnknownBundle`); E2016
(`BundleFieldUnknown`); E2017 (`BundleParamDefault` — wrong type for a bundle param);
E2018 (`BundleFieldNoDefault` — required field omitted); E2019
(`BundleParamNameCollision`).

### 6.6 Capabilities and generics

A `capability` is a named contract of `fn` signatures. A type satisfies a capability via
`impl Cap for T`. `Self` refers to the implementing type. A capability may require
super-capabilities and supply default method bodies.

Operators are sugar for capability methods: `a + b` desugars to `a.add(b)`. The standard
operator capabilities are:

| Operator | Capability | Method |
|----------|------------|--------|
| `+` `-` `*` `/` | `Add` `Sub` `Mul` `Div` | `add` `sub` `mul` `div` |
| `==` `!=` | `Eq` | `eq` |
| `<` `<=` `>` `>=` | `Ord : Eq` | `lt` |
| `&` `\|` `^` | `BitAnd` `BitOr` `BitXor` | `bitand` `bitor` `bitxor` |
| `!` | `Not` | `not` |

`Number : Add+Sub+Mul` is a convenience capability with a default `double` method.
Primitive types satisfy the relevant capabilities intrinsically.

**Generics.** Type parameters appear in `<>`, const parameters (of type `Natural`) in
`[]`. A bound is a `+`-separated set of capabilities. `Type` and `Net` are root
capabilities — every value type satisfies `Type`, every net type satisfies `Net`.

```phdl
capability Add { fn add(self, o: Self) -> Self; }
mod Adder <T: Add + Net> ( input a : T, input b : T, output y : T );
digital Adder { y <- a + b; }
```

`UInt`/`SInt`/`Complex` are library bundles built this way — fixed-width arithmetic in
PHDL, not compiler magic. `Bit`-vector concatenation is the library `fn concat`, not an
operator.

```
CapabilityDecl ::= "capability" Ident [ ":" Ident {"," Ident} ] "{" { FnSig | FnDecl } "}"
FnSig          ::= "fn" Ident [TypeParams] ParamList "->" Type ";"
ImplDecl       ::= "impl" [ Ident "for" ] TypeRef "{" { FnDecl } "}"
TypeRef        ::= Ident [ConstArgs] [TypeArgs]
TypeParams     ::= "<" TypeParam {"," TypeParam} ">"
TypeParam      ::= Ident [ ":" Bound ]
Bound          ::= Ident { "+" Ident }
```

**Parsing note.** `impl Cap for T` vs `impl T` is disambiguated by peeking for `for`
after the first identifier — the parser looks one token ahead.

---

## §7 Modules

A module is the unit of hierarchy. It declares ports (its external interface), params
(elaboration constants settable by the parent), internal wires and vars, child
instances, and structural control (`for`/`if`). Behavior is attached separately via
`analog`/`digital` blocks (§10).

```
ModDecl     ::= "mod" Ident [ConstParams] [TypeParams] PortList [ModBody]
ConstParams ::= "[" Ident {"," Ident} "]"
PortList    ::= "(" [ Port {"," Port} [","] ] ")"
ModBody     ::= "{" { ModStmt } "}"
ModStmt     ::= { Attribute } ( ParamDecl | WireDecl | VarDecl | StructuralFor
                                | StructuralIf | AssertStmt | InstanceOrConnect )
```

Const params `[N]` scale an architecture without threading widths through every call;
type params `<T>` parameterize over types per §6.6. Braces are omitted when a module
has only ports (a leaf with no internal structure).

### 7.1 Ports

```
Port      ::= { Attribute } Direction Ident ":" Type
Direction ::= "input" | "output" | "inout"
```

- **`input`** — directional in; high-impedance analog sense (reads without loading).
- **`output`** — single-driver out; one driver per net.
- **`inout`** — bidirectional; a conservative terminal participating in KCL.

Vectors: `NetType[N]` for a bus of width N.

### 7.2 Storage classes

```
ParamDecl ::= "param" Ident ":" Type [ "=" Expr ] ";"
WireDecl  ::= "wire" Ident ":" Type ";"
VarDecl   ::= "var" Ident [ ":" Type ] [ "=" Expr ] ";"
AssertStmt::= "$assert" "(" Expr "," Expr ")" ";"
```

- **`param`** — an elaboration constant. Settable by the parent via `{ .name = value }`.
  Always a value type. Requires an explicit type.
- **`wire`** — an internal net (or net array). Connects instances; participates in KCL.
- **`var`** — a mutable binding. In `digital`, combinational by default; if it must hold
  a value across cycles (read on a path where it was not assigned), it infers memory
  (§10.3). An initialized `var` infers its type (`var acc = 0.0;` → `Real`); `param`,
  ports, and fields require explicit types.

### 7.3 Instances and connectivity

```
InstanceOrConnect ::= Ident { Indexer | Field } InstTail
InstTail  ::= ":" ModuleRef PortArgs [ParamArgs] ";"     -- named instance
             | ConstArgs PortArgs [ParamArgs] ";"        -- anon w/ const args
             | PortArgs [ParamArgs] ";"                   -- anon
             | "=" Expr ";"                               -- net connection
ModuleRef ::= Ident [ConstArgs] [TypeArgs]
ConstArgs ::= "[" Expr {"," Expr} "]"
TypeArgs  ::= "<" Type {"," Type} ">"
PortArgs  ::= "(" [ PortArg {"," PortArg} ] ")"
PortArg   ::= Expr | "." Ident "=" Expr
ParamArgs ::= "{" [ ParamArg {"," ParamArg} ] "}"
ParamArg  ::= "." Ident "=" Expr
Indexer   ::= "[" Expr "]"   ;   Field ::= "." Ident
```

Ports connect positionally in `()` or by name `.p = net`. Params bind by name in `{}`.
An instance may be named (`name : Module`) — a named instance exposes its ports as nets
`name.port`, which the parent may connect, probe, or contribute to from its own `analog`
block. This is the KCL-accumulation mechanism: contributing `I(load.p, gnd) <+ expr`
adds current to the node that `load.p` is connected to, without an extra component.
Anonymous instances cannot be addressed afterward.

```phdl
r1 : Resistor ( .p = a, .n = b ) { .r = 50 };
load : Capacitor ( out, gnd ) { .c = 1p };
analog Tile { I(load.p, gnd) <+ cpar * ddt(V(load.p, gnd)); }
```

A `for` instance is an array `name[i]`; `name[i].port` reaches each replica. After
behavioral `for` unrolling (§10), `name[i].port` becomes `name_0.port`, `name_1.port`,
etc. — the loop variable is substituted by its concrete value, same as `if` const-
folding.

**Validation.** E2011 (`MissingConstParam` — wrong const-arg count); E2013
(`WidthMismatch` — net width mismatch on connection); E2014 (`DisciplineCrossing` —
incompatible disciplines connected); E2020 (`MultipleDrivers`).

### 7.4 Structural control

```
StructuralFor ::= "for" Ident "in" Range ModBody
StructuralIf  ::= "if" "(" Expr ")" ModBody [ "else" (ModBody|StructuralIf) ]
Range         ::= Expr (".."|"..=") Expr
```

`for i in lo..hi` / `lo..=hi` over a constant range builds parametric structure — the
loop is fully unrolled at elaboration, each iteration emitting its instances. `if
(const) {} else {}` selects which instances exist — the condition is evaluated at
elaboration and only the taken branch is emitted. `$assert(cond, msg)` in a `mod` body
is an elaboration-time check.

**Validation.** A non-const range or condition → E2001 (`ConstEval`). The loop bound
must be an elaboration constant; a runtime-dependent bound is an error (a solve value
never controls elaboration structure, §2.5).

---

## §8 Attributes (metadata)

`@SchemaName(field = value, ...)` prefixes any declaration — a `wire`, a port, a
`param`, a `mod`, a `bundle`, an instance. It attaches typed metadata for tools:
layout intent, routing constraints, floorplan placement, matching requirements.
Attributes are stackable:

```phdl
@Layout(min_width = 2u, layer = "m3") @Route(priority = 1) wire clk : Electrical;
@Floorplan(x = 0, y = 0) mod Cpu ( ... ) { ... }
```

```
Attribute ::= "@" Ident "(" [ AttrArg { "," AttrArg } ] ")"
AttrArg   ::= Ident "=" Expr
```

Three governing rules govern every attribute. Violation of any is a defect:

1. **Inert.** The core compiler never reads attribute content for semantics. It
   validates against the schema and attaches. Attributes do not affect elaboration or
   simulation.
2. **Removable.** Deleting every `@` yields an identical elaborated netlist and identical
   simulation results. (Contrast Verilog `full_case`/`parallel_case` — attributes that
   changed synthesis semantics and caused sim/synth mismatch; forbidden here by rule 1.)
3. **Schema-typed.** Each schema is a registered bundle declaration. The attribute's
   arguments are validated against the bundle's fields (names, types, defaults) during
   elaboration. An unregistered schema name is an error.

### Schema registration

A bundle becomes an attribute schema by marking it with
`@attribute(schema = "BundleName")`:

```phdl
bundle Layout { min_width : Real = 0.0, layer : String, spacing : Real = 0.0 }
@attribute(schema = "Layout")
```

Once registered, `@Layout(...)` is usable on any declaration. The arguments are
type-checked against the bundle's fields: provided values must match declared types,
required fields (no default) must be supplied, and omitted fields with defaults are
filled in automatically.

**Two entry paths, one store.** Attributes enter through two paths but live in one
metadata store:

- **Inline** — design intent, written in source next to the declaration.
- **Overlay** — flow intent, applied from the interpreted context (Part III §9) via the
  selector: `select("//pll//net::*").meta(layout, spacing = 2u)`. This annotates in bulk
  without touching source, like SDC/UPF constraints in ASIC flow. Overlay wins over
  inline on conflict, with a diagnostic.

Reflection (Part IV) exposes both uniformly through the `aspect::` axis. A module-level
attribute replaces the former "aspect block"; there is one metadata mechanism.

**Validation.** An attribute whose schema name is not a registered bundle is
`UnknownAttrSchema(name)` (E2022). An attribute argument that names a field not in
the bundle, or whose value type does not match the field's declared type, or a
required field that is omitted, is `AttrSchemaField { schema, field, reason }`
(E2023). Attributes are now populated into the POM — every node's `attributes()`
returns the validated `Attribute { schema, data }` entries from source.

---

## §9 Functions

A function is a pure value computation: `fn name(args) -> T`. Pure means no
contributions (`<+`), no forces (`<-`), no state, no events — just inputs in, a result
out. Because it is pure, a `fn` inlines at the call site, and so it serves every
context uniformly:

- **Elaboration** — compute a param default, a width, a const expression.
- **`digital`** — combinational logic (a `fn` called in a `digital` body inlines into
  the dataflow).
- **`analog`** — a `Real`-valued `fn` inlines into a contribution and is differentiated
  for the Jacobian (the Verilog-A analog function, gated by type: `Real` → analog,
  discrete → digital).

```
FnDecl    ::= "fn" Ident [TypeParams] ParamList "->" Type Block
ParamList ::= "(" [ Param {"," Param} ] ")"
Param     ::= "self" | Ident ":" Type [ "=" Expr ]      -- trailing defaults allowed
Block     ::= "{" { Stmt } [ Expr ] "}"
```

Arguments pass by value (basic types) or read-only reference (bundles). `mod` is the
unit of reusable structure; `fn` is the unit of reusable value computation.

This `fn`-body grammar — `var`, `if`/`else`, `match`, `for`, `return`, expressions,
lambdas — is **the same grammar** used everywhere in PHDL: bundle `impl` methods,
`bench` fns (Part III), and analog/digital behavior bodies. The statements and
expressions are uniform; what changes by context is purity, effect availability, and
the system-task set.

### 9.1 Default parameter values

A parameter may carry a default value: `fn v(self, a: Net, b: Net = gnd) -> Real`.
Defaults are **trailing only** — a defaulted parameter followed by a non-defaulted one
is a parse error. A call may omit trailing defaulted arguments: `r.v(a)` ≡
`r.v(a, gnd)`. Arity checking counts only the non-defaulted prefix.

Defaults are elaboration constants, evaluated in the callee's scope against the
already-bound earlier parameters. This applies uniformly — bundle `impl` methods, global
`fn`s, interpreted helpers, and analog `fn`s used in contributions. It replaces
overloading-by-arity (which PHDL does not have) and makes optional config expressible:
`op(cfg: OpConfig = OpConfig {})`.

### 9.2 Higher-order functions and generation

A function is a value: type `fn(T, U) -> R`. Lambdas `|a, b| a + b` are pure and capture
only elaboration constants. Collection operators are library functions, not syntax:

```phdl
fn map<T, U>(xs: T[N], f: fn(T) -> U) -> U[N] { return [ f(xs[i]) | i in 0..N ]; }
fn reduce<T>(xs: T[N], op: fn(T, T) -> T) -> T {
    if (N == 1) { return xs[0]; }
    return op( reduce(xs[0..N/2], op), reduce(xs[N/2..N], op) );
}
```

With a net `T` and a combinational `op`, `reduce(parts, |a,b| a+b)` emits a balanced
adder tree. (A mux tree, priority encoder, or prefix network is the same pattern.)

**Generation** is the elaboration phase evaluating pure values/types to emit hardware —
not macros over syntax. Recursion is elaboration-only and must terminate (each call
reduces a const param; a hard depth limit is the backstop). The elaboration phase stays
a total pure evaluator, never a Turing-complete macro stage.

```
Lambda ::= "|" [ Ident {"," Ident} ] "|" Expr
```

**Validation.** An effectful syscall inside a pure `fn` body is rejected
(`TaskUnavailable`, Part III §3) — purity is enforced by the interpreter's effect-
gating mechanism.

---

## §10 Behavior

`analog` and `digital` blocks are named after the module they belong to and run on
different engines under one statement grammar:

- **`analog`** builds the continuous system. Contributions `<+` and forces `<-` are
  stamped and resolved by Newton-Raphson each iteration (a blocking instruction list).
  Reads analog quantities and digital values.
- **`digital`** computes next state. Drives `<-`, assignments `=`, events `@`, on the
  event-driven kernel (combinational dataflow; inferred memory). Reads digital values
  and samples analog quantities.

A leaf device has one behavior block. A boundary device takes the block of the domain it
*drives*: a `Comparator` is `digital` (samples `V`, drives `Bit`); a 1-bit DAC is
`analog` (reads `Bit`, forces `V`). A behavioral `for` is unrolled — the bound must be
an elaboration constant. After unrolling, `rseg[i].n` becomes `rseg_0.n`, `rseg_1.n`,
etc. Behavior may branch on `$analysis` (§11), specialized per analysis at compile time.

```
BehaviorDecl ::= ("analog"|"digital") Ident "{" { BehaviorStmt } "}"
BehaviorStmt ::= VarDecl | BindStmt | IfStmt | MatchStmt | ForStmt
                | EventBlock | Diagnostic | ExprStmt
BindStmt     ::= Expr BindOp Expr ";"    ;   BindOp ::= "<+" | "<-" | "="
```

### 10.1 Access functions

Conservative quantities are read through accessors. A node pair is a branch:
`V(a,b)` reads the potential difference, `I(a,b)` reads the flow through the branch,
`V(n)` reads a node's potential relative to ground. The declared nature names are also
available: `Temp(th)`, `Pwr(th)`, etc.

Built-in analog operators include `ddt` (time derivative), `idt` (time integral), the
full math catalog (Part V §1), and explicit casts (`real`, `int`, `bit`). The analog-
operator set is open — new operators register through the layer-2 extension mechanism
(§14, Part V §2).

### 10.2 Analog behavior

`<+` is a **contribution**: it accumulates on a branch. Multiple contributions to the
same branch add up (KCL). `<-` is a **force**: it sets a single-driver value or a
controlled expression. One force per quantity per branch.

Each statement is a stamp: a flow contribution injects current into a node; a potential
force becomes a voltage source with an internal branch-current unknown. The solver
resolves all stamps together in each Newton iteration. Ideal constraint elements (ideal
op-amp, ideal switch) use a finite-parameter approximation (large-but-finite gain),
keeping every statement a direct stamp rather than a singular system.

A **switch branch** toggles which quantity it forces at runtime — this is runtime
topology over a static node set. An open ideal switch is stabilized by a small
conductance (Gmin). `@ initial` sets an analog initial condition; `$bound_step(dt)`
caps the next timestep.

```phdl
analog Switch { if (ctrl == Closed) { V(a,b) <- 0.0; } else { I(a,b) <- 0.0; } }
```

**Validation.** `<+` in a `digital` block → E2005 (`ContribInDigital`). `<+` in a `mod`
body → E2006 (`ContribInModBody`). `<-` in a `mod` body → E2007 (`ForceInModBody`).

### 10.3 Digital behavior

`<-` drives a net; `=` assigns a `var`. Combinational by default: an assignment is
dataflow — a later statement reads the value just assigned. Memory inference follows
two patterns:

- **Latch inference** — a `var` read on a path where it was not assigned retains its
  previous value. This raises a **warning by default** (deny-able per project or via
  attribute). It is the Verilog/VHDL idiom for accidental storage.
- **Register inference** — a `var` updated in a clocked `@` block is an edge-triggered
  register. Within the block, reads see the pre-edge value (a chain of register writes
  is a pipeline). Register inference is silent (it is the intended idiom, not an
  accident).

Overlapping writes: last in source order wins.

Control flow is `if`/`else` and `match`. A `match` over an enum is exhaustiveness-
checked. Patterns: enum variants, `_` (wildcard), and bit-pattern wildcards (`0b1??0`;
`?` is pattern-only, distinct from the `Quad` value `X`).

```phdl
digital SarAdc {
    result <- code;
    @ posedge(clk) {
        match state {
            Idle    => { if (start == 1) { state = Convert; idx = N-1; code = 0; code[N-1] = 1; } }
            Convert => { if (cmp == 0) { code[idx] = 0; }
                         if (idx == 0) { state = Done; } else { idx = idx - 1; code[idx-1] = 1; } }
            Done    => { if (start == 0) { state = Idle; } }
        }
    }
}
```

```
MatchStmt ::= "match" Expr "{" { MatchArm } "}"
MatchArm  ::= Pattern "=>" Block [","]
Pattern   ::= Path | "_" | BitPattern            -- BitPattern: "0b" {"0"|"1"|"?"}
```

**Validation.** Inferred latch → diagnostic `InferredLatch(var)` (warning). Non-
exhaustive `match` → elaboration error.

### 10.4 Events

`@ EVENT [ when (cond) ] { ... }` — the only place a `var` becomes state. Events are
how the digital kernel decides when to update registers, and how the analog kernel
detects crossings.

```
EventBlock  ::= "@" EventSpec [ "when" "(" Expr ")" ] Block
EventSpec   ::= EventTerm | "(" EventTerm { ("|"|"or") EventTerm } ")"
EventTerm   ::= Ident "(" [Expr] ")" | "initial" | "final"
```

Event sources (Part V §5 for the full table):
- **Digital edges:** `posedge(sig)`, `negedge(sig)`, `change(sig)`.
- **Analog crossings:** `cross(expr)` (zero crossing), `above(expr)` (one-shot level
  crossing).
- **Lifecycle:** `initial` (fires once at start), `final` (fires once at end;
  diagnostics only).
- **Periodic:** `timer(period)` (analog only).

Combine events with OR via `|`: `@ (posedge(a) | posedge(b))`. The `when (cond)` clause
gates on a level. An analog crossing may drive digital state (domain coupling — the
ngspice switch idiom). An unrecognized event name is a compile error.

**Validation.** E2008 (`UnknownEvent` — name not registered); E2009
(`AnalogEventInDigital`); E2010 (`DigitalEventInAnalog`).

### 10.5 Diagnostics

```
Diagnostic ::= SysCall "(" [ Expr {"," Expr} ] ")" ";"
```

`$info` / `$warn` / `$error` / `$fatal` report at increasing severity. `$assert(cond,
msg)` reports when `cond` is false. Format strings interpolate arguments:
`$info("vout = {}", V(out))`. `$finish` ends the run. `$display` / `$write` print at
Info severity. `$fatal` does not auto-`$finish`.

The full `$`-syscall catalog — including `$temperature`, `$vt`, `$abstime`,
`$analysis`, `$limit`, `$random`/`$dist_*`, `$simparam`, `$port_connected`, and more —
is normative in Part V §3.

---

## §11 System tasks (`$`-syscalls) — overview

A `$`-syscall is the surface syntax for every runtime-valued or effectful operation.
The `$` prefix makes it visually distinct from user-defined functions and signals that
the call may depend on simulation state, not just its arguments.

PHDL has three layers of `$`-syscall availability, depending on context (full tables in
Part V):

1. **Everywhere (pure tasks):** diagnostics (`$assert`, `$info`, `$warn`, `$error`,
   `$fatal`, `$display`) and the math catalog (`$sqrt`, `$ln`, ...). Available in any
   `fn` body, including pure functions.

2. **Solve-time (analog/digital):** runtime quantities (`$temperature`, `$vt`,
   `$abstime`, `$analysis`, `$random`, ...) and control (`$bound_step`, `$finish`,
   `$discontinuity`). Available in `analog`/`digital` bodies only.

3. **Interpreted context (bench):** analyses (`$op`, `$tran`, `$ac`, `$noise`) and
   artifact writers (`$write`, `$plot`). Available in `bench` fns only.

The syscall set is open — new syscalls register through the layer-2 extension mechanism
(§14). This section gives the grammar; Part V gives the exhaustive tables.

---

## §12 Phase model

| Phase | Where | What |
|-------|-------|------|
| Elaboration | `mod` body, type annotations, structural control | resolved once into a fixed netlist |
| Solve | `analog` / `digital` | evaluated by the NR / event-driven engine |
| Interpreted | `bench` | tree-walked over the elaborated design |

Hardware is neither created nor destroyed during solve or interpretation. Runtime
topology — a switch opening, a MOSFET entering cutoff — is a switch branch over a
static node set, not structural change. This is the fundamental contract: the structure
is fixed before any current flows, and the solver specializes itself to that structure.

---

## §13 No-Magic

Connecting incompatible disciplines is a compile error. Crossing a discipline or domain
boundary requires an explicit converter `mod` — the compiler never inserts one
implicitly.

This rule governs **net connections only**. Reading a net into a value (sampling a
voltage into a `Real`) or driving a value onto a net (forcing a computed voltage) is
ordinary and unchecked. A device that couples two disciplines *internally* (an
electrothermal resistor with both `Electrical` and `Thermal` ports) needs no converter,
because no single net crosses a boundary — the coupling happens inside the device's own
`analog` block.

**Validation.** E2013 (`WidthMismatch`); E2014 (`DisciplineCrossing`).

---

## §14 Extension model

The core grammar (layer 0) is closed. Growth happens above it, through five layers:

| Layer | Mechanism | Adds |
|-------|-----------|------|
| 1 | Standard library (capabilities, generics, HOF) | Value types (`UInt`, `Complex`), `map`/`reduce`/`concat` |
| 2 | Compiler registries (trait + registry) | Analog operators, `$`-syscalls, `@`-event kinds |
| 3 | Attribute schemas | Typed per-declaration metadata (layout, routing, ...) |
| 4 | POM + selector + plugins | Reflection, overrides, annotations — the design-closure loop |
| 5 | Interpreted context (bench) | Orchestration, sweeps, verification, closure loops |

New value types (layer 1) and new operators/syscalls/events (layer 2) never touch the
grammar — they register at startup and the existing grammar productions (function
calls, `$`-prefixed names, `@`-prefixed event specs) absorb them.

Layers 3–4 carry the physical-design loop: attribute metadata + a plugin that reflects
over the POM, runs placement/extraction, and returns **annotations** (parasitics keyed
by `NetId`, fused by KCL) and **overrides** (staged, consumed by pure re-elaboration).
The loop is `reflect → emit → re-elaborate → simulate`, deterministic at every step.
Verification and timing live in layer 5, outside the hardware language.

Layer 0 stays closed by the No-Bloat rule (§1): a feature must demonstrate it cannot
live in a layer above before it may touch the core grammar.

---

## §15 Rejected features

These are decisions, not omissions — each was considered and rejected for a stated
reason.

**Digital `#` delays.** Rejected because they are a source of race semantics and delta
cycles. RTL in PHDL is zero-delay combinational logic plus registers. Timing belongs to
analog (the `transition` and `absdelay` operators) or to verification (the interpreted
context), not to the RTL itself.

**Connect modules / connectrules (auto-insertion).** Rejected by No-Magic (§13). If you
need to cross a discipline boundary, you write an explicit converter module. The
compiler never inserts one for you.

**Indirect branch assignment (`V(x): f(x)==0`).** Rejected because it produces singular
systems (the Jacobian has no useful entry for an algebraic constraint with no residual).
Use the finite-parameter idiom instead — a large-but-finite gain VCVS.

**`mod`/`bundle` unification.** Rejected because it reproduces the SystemVerilog
`interface` — a construct that blurs the line between "shape with identity and
behavior" (a module) and "valued aggregate" (a bundle). They are distinct concepts in
PHDL; their field syntax rhymes, and that suffices.

---

## §16 Validation rules (Part I consolidated)

Every validation rule in this Part, gathered for cross-reference. The master catalog
with error codes is Part II §11.

| Section | Rule | Error |
|---------|------|-------|
| §2 | value type used as net | E2004 `NotNetCapable` |
| §2 | `<+` / `<-` in a `mod` body | E2006 / E2007 |
| §5.3 | accessing a non-`pub` item from outside its package | E2021 `PrivateItem` |
| §6.1 | collection literal in unlowered context | fail-loud at device-compile |
| §6.2 | multiple drivers on single-driver storage | E2020 `MultipleDrivers` |
| §6.5 | bundle type/field errors | E2015–E2019 |
| §7.3 | const-arg count mismatch | E2011 `MissingConstParam` |
| §7.3 | width mismatch on connection | E2013 `WidthMismatch` |
| §7.3 | discipline crossing | E2014 `DisciplineCrossing` |
| §7.4 | non-const structural control | E2001 `ConstEval` |
| §8 | unknown attribute schema | E2022 `UnknownAttrSchema` |
| §8 | attribute field mismatch | E2023 `AttrSchemaField` |
| §9.2 | effectful syscall inside pure `fn` | `TaskUnavailable` |
| §10.2 | `<+` in `digital` or `mod` body | E2005 / E2006 |
| §10.3 | inferred latch | diagnostic `InferredLatch` (warning) |
| §10.3 | non-exhaustive `match` | E2024 `NonExhaustiveMatch` |
| §10.4 | unknown event name | E2008 `UnknownEvent` |
| §10.4 | event/block domain mismatch | E2009 / E2010 |
