# Part IV — Reflection & Selector

The Piperine Object Model (POM) and the selector query language are one surface: the
selector's axes *are* POM relations, and its predicates *are* POM attributes. The POM
exposes an elaborated design as a graph of typed runtime nodes — the single interface
for the interpreted context (Part III), Python, Rust, and the plugin ABI.

POM is **read-first**: querying has no effect; assigning a parameter **stages an
override** consumed by a later pure elaboration (Part II). The selector adds no model of
its own — it is a query language layered over the POM's existing structure.

## Contents

- §1 Core concepts
- §2 Design root
- §3 Staging and determinism
- §4 Elaborated-structure nodes
- §5 Definition nodes
- §6 Leaf enums
- §7 One model, three languages, one ABI
- §8 Selector — model
- §9 Selector — grammar
- §10 Selector — axes
- §11 Selector — node tests
- §12 Selector — predicates
- §13 Selector — evaluation
- §14 Selector — integration

---

## §1 Core concepts

**Distinct typed nodes.** Each construct is its own node type with its own interface.
There is no generic "node" that you must downcast — a `Module` has `ports()`, an
`Instance` has `of()`, a `Net` has `discipline()`. The node types are:

- **Elaborated-structure:** `Module`, `Instance`, `Net`, `Port`, `Param`, `Attribute`,
  `Behavior`.
- **Definition:** `Bundle`, `Enum`, `Discipline`, `Capability`.
- **Leaf:** `Field` (of a bundle), `Variant` (of an enum), `Nature` (of a discipline),
  `Signature` (of a capability).

**Two-axis model.** Every node offers two kinds of access:

- *Attributes* — scalar properties returning a `Value`. Examples: `Module.name`,
  `Net.width`, `Port.direction`. A few attributes are settable (params); the rest are
  read-only.
- *Relations* — named axes returning a `Selection` of related nodes. Examples:
  `Module.ports()`, `Instance.children()`, `Net.drivers()`.

This is what makes the selector work: its axes are the POM's relations, its predicates
test the POM's attributes.

**Identity.** `Node.id()` returns a stable `Id` — stable across re-elaboration while the
source is unchanged. `Net` also exposes `NetId`, the anchor for parasitic extraction and
LVS across the design-closure loop.

**Value.** The value layer of the POM: primitives (`Real`, `Natural`, `Integer`,
`Boolean`, `Quad`, `String`), a node reference, or a value-layer collection (`Vec`,
`Map`, `Set`, `Option`, `Result`). `Complex` is a `Bundle`, not a primitive.

**Selection\<T\>** — the universal navigation result. An ordered, duplicate-free set:

```
len, is_empty
get(i) -> Option<T>     ; first / last -> Option<T>
iter() -> Vec<T>
filter(pred) -> Selection<T>
map(f) -> Vec<U>
where(path) -> Selection<Node>
one() -> Result<T, _>
```

`sel[i]` is sugar for `get(i)` unwrapped. Settable attributes write across a whole
selection (e.g., `select("//resistor").resistance = 2e6` stages the override on every
matched resistor).

---

## §2 Design root

The `Design` node is the root of the POM:

```
Design
  top() -> Module
  module(name) -> Option<Module>
  modules() -> Selection<Module>
  select(path) -> Selection<Node>
  const_(name) -> Option<Value>
  consts() -> Map<String, Value>
  bundles() / enums() / disciplines() / capabilities() -> Selection<...>
```

In a bench `fn`, the root is the bench's module; `select` is rooted there (Part III §2).

---

## §3 Staging and determinism

Writing a parameter stages an override; it does not edit the design. `Param.set(v)`
returns `Result<Unit, ReflectError>`; the sugar forms are `sel.r = 0.2e-2` and
`sel.set("w", 2.0)`. The next analysis re-elaborates purely from staged overrides.

| Code | Variant | Trigger |
|------|---------|---------|
| E3001 | `NotFound` | a name or path was not found |
| E3002 | `NotSettable` | the attribute is read-only |
| E3003 | `TypeMismatch` | the value's type does not match |
| E3004 | `OutOfRange` | the value is outside the accepted range |
| E3005 | `MultipleDrivers` | staging would create a multi-driver conflict |
| E3999 | `Other(String)` | catch-all |

No in-place mutation occurs. Writing a read-only attribute is `NotSettable` (E3002);
there is no escape hatch.

---

## §4 Elaborated-structure nodes

**Module** — `name`; `is_generic`; `ports()`, `params()`, `nets()`, `instances()`,
`behaviors()` → `Selection`; `attributes()` → `Selection<Attribute>`;
`port(n)`, `net(n)`, `instance(n)`, `param(n)` → `Option`; `attribute(schema)` →
`Option<Attribute>`; `select(path)`. `instances()` returns direct children; `nets()`
returns port nets plus internal wires.

**Instance** — `name`; `path` → `Path`; `of()` → `Module`; `ports()`, `params()`,
`children()` → `Selection`; `port(n)`, `param(n)` → `Option`; `net(port)` →
`Option<Net>` (the `dac.out` access from Part I §7.3). `param.set` / `inst.r = ...`
stages an override scoped to the instance path.

**Net** — `id` → `NetId`; `name`; `discipline()` → `Discipline`; `resolution()` →
`Resolution`; `width()` → `Natural`; `line(i)` → `Option<Net>`; `is_port`;
`drivers()`, `loads()`, `connected()` → `Selection<Port>`. A bus is one `Net` of
`width N`; `line(i)` returns a single line.

**Port** — `name`; `direction()` → `Direction`; `net_type()` → `Type`; `net()` →
`Net`; `owner()` → `Node`.

**Param** — `name`; `type()` → `Type`; `value()` → `Value`; `is_overridden`; `set(v)`;
`owner()` → `Node`.

**Attribute** — plugin metadata from `@schema(...)`: `schema()` → `String`; `data()` →
`Value`; `field(name)` → `Option<Value>`; `owner()` → `Node`. Inline (source) and
overlay (selector-applied) attributes appear identically here; overlay wins on conflict.

**Behavior** — `domain()` → `Domain` (`Analog` | `Digital`); `owner()` → `Module`.

---

## §5 Definition nodes

**Bundle** — `name`; `is_net_capable`; `type_params()`, `const_params()` →
`Vec<String>`; `fields()` → `Selection<Field>`; `capabilities()` →
`Selection<Capability>`.

**Field** — `name`; `type()` → `Type`; `default()` → `Option<Value>`.

**Enum** — `name`; `repr()` → `Type`; `variants()` → `Selection<Variant>`.

**Variant** — `name`; `value()` → `Value`.

**Discipline** — `name`; `kind()` → `DisciplineKind` (`Conservative` | `Storage`);
`storage()` → `Option<Type>`; `resolution()` → `Resolution`; `natures()` →
`Selection<Nature>` (conservative only).

**Nature** — `name`; `kind()` → `NatureKind` (`Potential` | `Flow`); `value_type()` →
`Type`; `unit()` → `Option`; `abstol()` → `Option<Real>`.

**Capability** — `name`; `supers()` → `Selection<Capability>`; `signatures()` →
`Selection<Signature>`; `implementors()` → `Selection<Node>`.

**Signature** — `name`; `params()` → `Vec<Type>`; `returns()` → `Type`; `has_default`
→ `Boolean`.

---

## §6 Leaf enums

```
Direction      ::= Input | Output | Inout
Domain         ::= Analog | Digital
DisciplineKind ::= Conservative | Storage
Resolution     ::= Single | Tri | Or | And | Sum | Avg | Max | Min | Kcl
NatureKind     ::= Potential | Flow
ReflectError   ::= NotFound | NotSettable | TypeMismatch | OutOfRange
                 | MultipleDrivers | Other
```

`Node` is the supertype of every node kind; `Node.kind()` discriminates it, allowing an
untyped host to recover the concrete type.

---

## §7 One model, three languages, one ABI

The POM is uniform underneath and idiomatic on top. The wire protocol is a serialized
node carrying `kind`, `id`, scalar attributes, and relation axes. Every host rebuilds
the same typed objects from this one ABI:

- **Piperine (bench)** — built-in node objects with assignment sugar (`inst.r = ...`).
- **Python** — classes with `__getattr__` / `__setattr__`.
- **Rust** — structs and traits with explicit calls (`inst.param("r")?.set(...)?`).

Compiled plugins (Rust or Python) load like OSDI compact models — a shared library
exposing a descriptor that conforms to the ABI.

---

## §8 Selector — model

The selector is the "XPath of the circuit." It evaluates against a design and returns a
`Selection<Node>`. It adds no model of its own:

> **Axes are POM relations. Predicates are POM attributes.**

A path is a sequence of **steps**. Each step moves along an **axis**, keeps nodes
matching a **node test**, and filters through **predicates**. Results union across
context nodes, deduplicate by identity, and preserve first-seen order.

A selector is evaluated against a context node: `design.select(path)` (rooted at the
design), `module.select(path)` (rooted at a module), or `selection.where(path)`
(rooted at each node in a selection).

---

## §9 Selector — grammar

```
Selector  ::= [ "/" | "//" ] Step { ( "/" | "//" ) Step }
Step      ::= [ Axis "::" ] NodeTest { Predicate }
Axis      ::= "inst"|"net"|"port"|"param"|"attr"|"behavior"|"driver"|"load"|"parent"|"ancestor"
NodeTest  ::= Name | "*"
Predicate ::= "[" ( Index | PredExpr ) "]"
Index     ::= NatLit | "last" "(" ")"
PredExpr  ::= OrExpr
OrExpr    ::= AndExpr { "or" AndExpr }
AndExpr   ::= NotExpr { "and" NotExpr }
NotExpr   ::= "not" "(" PredExpr ")" | Compare
Compare   ::= Operand [ CmpOp Operand ]
CmpOp     ::= "=="|"!="|"<"|"<="|">"|">="|"~"
Operand   ::= AttrRef | AxisRef | Func | Literal
AttrRef   ::= "@" Name [ "." Name ]
AxisRef   ::= Axis "::" NodeTest
Func      ::= "of" "(" StringLit ")" | "count" "(" AxisRef ")"
Literal   ::= NumberLit | StringLit | BoolLit | Ident
```

`/` is absolute from the context node; `//` is the descendant-or-self closure over
`inst::`. The default axis (when none is written) is `inst::`.

---

## §10 Selector — axes

| Axis | POM relation |
|------|--------------|
| `inst::` *(default)* | `instances()` / `children()` |
| `net::` | `nets()` |
| `port::` | `ports()` |
| `param::` | `params()` |
| `attr::` | `attributes()` |
| `behavior::` | `behaviors()` |
| `driver::` | net `drivers()` |
| `load::` | net `loads()` |
| `parent::` | reverse of `instances()` — the containing instance |
| `ancestor::` | transitive closure of `parent::` |

`//X` is equivalent to "X at any instance depth." A step after `//` may switch axis:
`//*/net::clk` selects every `clk` net at any depth.

---

## §11 Selector — node tests

`*` matches any node. A name matches by the node's name:

- On `inst::`, **PascalCase** matches by module type (the `of()` source name — `Dac`
  matches the monomorphized `Dac__8`), while **snake_case** matches by instance name
  (the `name :` given in the parent).
- On other axes, a name matches by the node's own name.

An instance array `leg[N]` shares the base name `leg`; a bare `leg` matches all
replicas, and an index predicate `[i]` picks one.

---

## §12 Selector — predicates

A predicate filters the step's result set. Two kinds:

**Positional:** `[i]` (0-based index) or `[last()]`.

**Expressional:** a boolean expression over attributes and axis-references.

- Attribute reference: `@name` or `@schema.field` — e.g., `[@direction == Input]`,
  `[@width > 1]`, `[@layout.min_width > 1u]`.
- String match: `[@name ~ "cmp*"]` (glob, case-sensitive).
- Axis existence: `[attr::layout]` (the node has a `layout` attribute),
  `[net::clk]` (the node has a `clk` net).
- Axis comparison: `[param::r > 1k]` (the node's `r` param exceeds 1k).
- Boolean combinators: `and` (binds tighter), `or`, `not(...)`.
- Functions: `of("TypeName")` (tests the node's type), `count(axis::test)` (counts
  matches along an axis).

Enum and node-type names are bare identifiers (`Input`, `Resistor`); string values are
quoted (`"m3"`). Sequential predicates are conjoined left-to-right: `[@width > 1][@name
~ "data*"]` means width > 1 AND name starts with "data".

---

## §13 Selector — evaluation

Per step over a node set S: for each node n in S, follow the axis from n, keep nodes
matching the node test, apply predicates left-to-right (boolean filters narrow the set;
positional predicates keep the ordinal), then union all results and deduplicate by
identity.

An empty result is valid (not an error) — use `is_empty()` or `one()` to handle it.
Evaluation is a pure function of the elaborated design plus staged overrides, so the
same selector on the same state always returns the same nodes.

| Variant | Trigger |
|---------|---------|
| `EmptySelector` | empty selector string |
| `ExpectedDoubleColon` | missing `::` after an axis |
| `ExpectedNodeTest` | missing node test after an axis |
| `UnknownAxis(String)` | axis name not in the table |
| `AxisNotImplemented(Axis)` | axis exists in the enum but has no lowering |

---

## §14 Selector — integration

```piperine
// Read every resistor's value
for r in select("//Resistor") { $info("{}", r.param("r").value()); }

// Set a param on every matched instance
select("//dac/param::vref").set(1.8);
select("//leg").set("w", 2.0);

// Filter into a local
var big = select("//Resistor").where("[param::r > 1k]");

// Attach a parasitic (extensibility layer 4)
select("//*/net::*[@layout.layer == \"m3\"]").attach( Capacitor { .c = 4.2f } );
```

Adding a POM node type or attribute extends what the selector addresses, with no
grammar change — the new relation or attribute is immediately available as an axis or
predicate.
