# Part IV — Reflection API (Object Model)

*Piperine Reflection API — The Piperine Object Model (POM)*

POM exposes an elaborated design as a graph of typed runtime nodes — the single surface for
Piperine `bench`, Python, Rust, and the plugin ABI. The `ElabProgram`/`IrProgram` behind it is
never exposed. POM is **read-first**: querying has no effect; assigning a parameter **stages an
override** consumed by a later pure elaboration (§3). Companion docs (selector, extensibility)
build on it.

### 1. Core concepts

**Distinct typed nodes.** Each construct is its own node type with its own interface — `Module`,
`Instance`, `Net`, `Port`, `Param`, `Attribute`, `Behavior`; definition nodes `Bundle`, `Enum`,
`Discipline`, `Capability`; leaf nodes `Field`, `Variant`, `Nature`, `Signature`. Distinct types
give precise, type-safe interfaces per host; one wire protocol underneath (§7) gives one ABI and
one selector.

**Two-axis model.** *Attributes* are scalar properties returning a `Value`; a few are settable,
the rest read-only. *Relations* are named axes to other nodes, always returning a `Selection`.
(This is what makes the selector's axes = relations, predicates = attributes.)

**Identity.** `Node.id() -> Id`, stable across re-elaboration while the source is unchanged.
`Net` also exposes `NetId` — the anchor for extraction/LVS across the closure loop.

**Value** = the value layer: primitives `Real Natural Integer Boolean Quad String`; a node
reference; or a value-layer collection. (`Complex` is a `Bundle`, not a primitive.)

**Selection\<T\>** — ordered, duplicate-free set; the universal navigation result:

```
len, is_empty ; get(i)->Option<T> ; first/last->Option<T> ; iter->Vec<T>
filter(pred)->Selection<T> ; map(f)->Vec<U> ; where(path)->Selection<Node> ; one()->Result<T,_>
```

`sel[i]` = `get(i)` unwrapped. Settable attributes write across a whole selection (§3); `attach`
(annotation) and `meta` (attribute overlay) are in the extensibility spec.

**Value-layer collections** (value layer only, never net-capable hardware): `Vec Map Set Option
Result`. A value-only bundle may hold them; a net-capable bundle may not.

### 2. Design root

```
Design
  top()->Module ; module(name)->Option<Module> ; modules()->Selection<Module>
  select(path)->Selection<Node>
  const_(name)->Option<Value> ; consts()->Map<String,Value>
  bundles()/enums()/disciplines()/capabilities()->Selection<...>
```

In `bench Module { ... }` the root is the bench's module; `select` is rooted there.

### 3. Staging and determinism

`Param.set(v)->Result<Unit,ReflectError>`, or sugar `sel.r = 0.2e-2` / `sel.set("w", 2.0)`.
Writing a parameter **stages an override**; it does not edit the design. The next
`simulate`/`elaborate` re-elaborates purely from staged overrides (structural param →
re-elaborate; non-structural → netlist patch; the engine decides). No in-place mutation; writing
a read-only attribute is a `ReflectError`.

### 4. Elaborated-structure nodes

**Module** — `name ; is_generic ; ports()/params()/nets()/instances()/behaviors()->Selection ;
attributes()->Selection<Attribute> ; port(n)/net(n)/instance(n)/param(n)->Option ;
attribute(schema)->Option<Attribute> ; select(path)`. `instances()` is direct children; `nets()`
= port nets + internal wires.

**Instance** — `name ; path->Path ; of()->Module ; ports()/params()/children()->Selection ;
port(n)/param(n)->Option ; net(port)->Option<Net>` (the `dac.out` access). `param.set` /
`inst.r = ...` stages an override scoped to the instance path.

**Net** — `id->NetId ; name ; discipline()->Discipline ; resolution()->Resolution ;
width()->Natural ; line(i)->Option<Net> ; is_port ; drivers()/loads()/connected()->Selection<Port>`.
A bus is one Net of `width N`; `line(i)` is a line.

**Port** — `name ; direction()->Direction ; net_type()->Type ; net()->Net ; owner()->Node`.

**Param** — `name ; type()->Type ; value()->Value ; is_overridden ; set(v) ; owner()->Node`.

**Attribute** — plugin metadata from `@schema(...)` on any declaration (§8 of the language spec):
`schema()->String ; data()->Value ; field(name)->Option<Value> ; owner()->Node`. Inline and
selector-overlay attributes appear identically here (overlay wins on conflict).

**Behavior** — `domain()->Domain (Analog|Digital) ; owner()->Module`. (Body reflection is a later
refinement.)

### 5. Definition nodes

**Bundle** — `name ; is_net_capable ; type_params()/const_params()->Vec<String> ;
fields()->Selection<Field> ; capabilities()->Selection<Capability>`.
**Field** — `name ; type()->Type ; default()->Option<Value>`.

**Enum** — `name ; repr()->Type ; variants()->Selection<Variant>`. **Variant** — `name ;
value()->Value`.

**Discipline** — `name ; kind()->DisciplineKind (Conservative|Storage) ; storage()->Option<Type>
; resolution()->Resolution ; natures()->Selection<Nature>` (conservative).
**Nature** — `name ; kind()->NatureKind (Potential|Flow) ; value_type()->Type ; unit()->Option ;
abstol()->Option<Real>`.

**Capability** — `name ; supers()->Selection<Capability> ; signatures()->Selection<Signature> ;
implementors()->Selection<Node>`. **Signature** — `name ; params()->Vec<Type> ; returns()->Type ;
has_default->Boolean`.

### 6. Leaf enums

```
Direction     ::= Input | Output | Inout
Domain        ::= Analog | Digital
DisciplineKind::= Conservative | Storage
Resolution    ::= Single | Tri | Or | And | Sum | Avg | Max | Min | Kcl
NatureKind    ::= Potential | Flow
ReflectError  ::= NotFound | NotSettable | TypeMismatch | OutOfRange | UnknownSchema
```

`Node` is the supertype; `Node.kind()` discriminates it (how an untyped host recovers the type).

### 7. One model, three languages, one ABI

Uniform underneath, idiomatic on top. Wire protocol = a serialized node carrying `kind`, `id`,
scalar attributes, relation axes; every host rebuilds the same typed objects from one ABI.
Piperine (`bench`) built-in nodes + assignment sugar; Python classes with `__getattr__`/
`__setattr__`; Rust structs/traits with explicit `inst.param("r")?.set(...)?`. The ABI *is* this
API (serialized-node protocol + per-type method tables). Compiled plugins (Rust/Python) load like
OSDI compact models — a shared library exposing a descriptor.

### 8. Out of scope

Selector spec (the query language; POM relations are its axes, attributes its predicates).
Extensibility spec (`@` schema registration, plugin verbs, annotations `Selection.attach`,
attribute overlay `Selection.meta`, the `bench` toolchain). These may still reshape POM.

---

