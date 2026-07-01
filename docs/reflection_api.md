# Piperine Reflection API — The Piperine Object Model (POM)

The reflection API exposes an elaborated Piperine design as a graph of typed runtime objects —
the **Piperine Object Model**. It is the single surface every host (Piperine `bench`, Python,
Rust) and the plugin ABI reference. The user navigates and queries POM nodes; the underlying
`ElabProgram` / `IrProgram` representation is never exposed.

POM is **read-first**. Querying a design has no effect. The one form of change — assigning a
parameter — does not mutate anything in place: it **stages an override** that a later
elaboration consumes purely (§3). Reproducibility and the no-magic guarantee of the language
hold across the reflection layer unchanged.

This document specifies the object model and every node interface. Two companion documents
build on it and are deliberately out of scope here: the **selector** ("XPath for POM") that
navigates the graph by string query, and the **extensibility** spec (plugins, `aspect`
schemas, annotations, the `bench` toolchain). Where they connect, this document forward-refers.

---

## 1. Core concepts

### 1.1 Distinct typed nodes

Each elaborated construct is its own node type with its own interface — `Module`, `Instance`,
`Net`, `Port`, `Param`, `Aspect`, `Behavior` — alongside the definition nodes `Bundle`, `Enum`,
`Discipline`, `Capability`, and the leaf nodes they contain (`Field`, `Variant`, `Nature`,
`Signature`). Distinct types are chosen over one uniform node so that each interface is precise
and type-safe in a host that wants it (Rust), while the shared protocol underneath (§7) still
gives one ABI and one selector.

Every node type follows the **two-axis model**:

- **Attributes** are scalar properties — a name, a direction, a value. They return a `Value`
  (§1.3). A few are *settable* (a parameter's value); the rest are read-only.
- **Relations** are named axes to other nodes — a module's instances, a net's drivers. They
  always return a `Selection` (§1.4), even when the result is one node or none.

This split is what makes the selector clean: a relation is a path *axis*, an attribute is a
path *predicate* (selector spec, separate).

### 1.2 Identity

Every node has a stable identity that survives re-elaboration as long as the source construct
is unchanged:

```
Node
  fn id(self) -> Id            // stable node identity
```

Nets additionally expose a `NetId`, the anchor that ties a POM net to layout extraction and LVS
correspondence (extensibility spec). Identity is what lets an external tool name *this* net
across the elaborate → extract → re-elaborate loop without a fragile textual path.

### 1.3 `Value`

Attribute accessors and collection elements are `Value`s — the value layer of the language:

- the primitives `Real`, `Natural`, `Integer`, `Boolean`, `Quad`, `String`, `Complex`;
- a reference to any POM node (so `port.net()` yields a `Net` value);
- the value-layer collections of §1.5.

`Value` is never a net or a piece of hardware. Hardware stays in the static net layer; POM
describes it but is itself value-level data.

### 1.4 `Selection<T>`

Navigation always returns a `Selection<T>` — an ordered, immutable set of nodes of type `T`. It
is the universal result type, so a relation that yields one node, many, or none has a single
shape, and bulk operations (filtering, overriding, attaching) fall out for free.

```
Selection<T>
  fn len(self) -> Natural
  fn is_empty(self) -> Boolean
  fn get(self, i: Natural) -> Option<T>      // bounds-checked
  fn first(self) -> Option<T>
  fn last(self) -> Option<T>
  fn iter(self) -> Vec<T>                     // for `for x in sel { ... }`
  fn filter(self, pred: fn(T) -> Boolean) -> Selection<T>
  fn map<U>(self, f: fn(T) -> U) -> Vec<U>
  fn where(self, path: String) -> Selection<Node>   // sub-select via the selector
  fn one(self) -> Result<T, ReflectError>     // exactly-one, else error
```

Indexing `sel[i]` is sugar for `get(i)` unwrapped; iterating a `Selection` in a `for` uses
`iter`. Settable attributes can be written across a whole selection (§3). `attach` (annotation)
is defined in the extensibility spec.

### 1.5 Value-layer collections

POM results use the language's dynamic collections, which live **only** in the value layer
(elaboration, `fn`, `bench`, and API results) — never in net-capable hardware:

```
Vec<T>      ordered, growable      push/pop, len, [i], iter, map/filter/fold
Map<K, V>   associative            get -> Option<V>, set, has, keys, values
Set<T>      unordered unique       add, has, len, union/intersect/diff
Option<T>   present or absent      is_some, unwrap_or, map
Result<T,E> success or error       is_ok, unwrap_or, map, ?
```

A value-only `bundle` may hold these (a plugin result is `{ parasitics: Vec<Annotation>,
metrics: Map<String, Real> }`); a net-capable `bundle` may not — its fields stay static.

---

## 2. The design root

```
Design
  fn top(self) -> Module                      // the elaborated top module
  fn module(self, name: String) -> Option<Module>
  fn modules(self) -> Selection<Module>        // every elaborated module (monomorphized)
  fn select(self, path: String) -> Selection<Node>   // run a selector from the root
  fn const_(self, name: String) -> Option<Value>       // evaluate a global constant by name
  fn consts(self) -> Map<String, Value>                // all elaborated global constants

  // definition reflection (first-class)
  fn bundles(self) -> Selection<Bundle>
  fn enums(self) -> Selection<Enum>
  fn disciplines(self) -> Selection<Discipline>
  fn capabilities(self) -> Selection<Capability>
```

`Design` is the handle a `bench` or host starts from. In a `bench Module { ... }` the design
root is the bench's module, already elaborated, so `select(...)` is rooted there implicitly
(extensibility spec). `modules()` returns monomorphized modules — `Foo[8]` appears as its
concrete instantiation.

---

## 3. Staging and determinism

A settable attribute is written through `set`, or through host-idiomatic assignment sugar:

```
Param
  fn set(self, v: Value) -> Result<Unit, ReflectError>
```

```piperine
select("//bias").r = 0.2e-2;          // sugar for the matched Param's set(...)
legs.set("w", 2.0);                    // bulk: set across a whole Selection
```

Writing a parameter **stages an override** in the POM's staging layer; it does not edit the
elaborated design. A subsequent `simulate`/`elaborate` (toolchain, extensibility spec)
re-elaborates from the staged overrides — purely and reproducibly. A parameter that affects
structure triggers re-elaboration; one that does not is patched into the netlist directly; the
engine decides and the user does not have to. The no-mutation invariant therefore holds: POM is
a read view plus a staging layer, never an in-place edit. Writing a read-only attribute is a
`ReflectError`.

---

## 4. Elaborated-structure nodes

### 4.1 `Module`

A monomorphized module: an interface, internal structure, and behavior.

```
Module
  fn name(self) -> String
  fn is_generic(self) -> Boolean               // false after monomorphization
  fn ports(self) -> Selection<Port>
  fn params(self) -> Selection<Param>          // declared params (defaults)
  fn nets(self) -> Selection<Net>              // port nets + internal wires
  fn instances(self) -> Selection<Instance>    // direct children only
  fn aspects(self) -> Selection<Aspect>
  fn behaviors(self) -> Selection<Behavior>    // its analog / digital blocks
  fn port(self, name: String) -> Option<Port>
  fn net(self, name: String) -> Option<Net>
  fn instance(self, name: String) -> Option<Instance>
  fn param(self, name: String) -> Option<Param>
  fn aspect(self, schema: String) -> Option<Aspect>
  fn select(self, path: String) -> Selection<Node>     // rooted at this module
```

`instances()` is direct children; full-depth traversal is a descendant selector or
`Instance.children()`. `nets()` includes both the nets bound to ports and the internal `wire`s.

### 4.2 `Instance`

A placed child of a module — the node a parent addresses to read a child's terminal or to
retune it.

```
Instance
  fn name(self) -> String
  fn path(self) -> Path                        // hierarchical identity from the root
  fn of(self) -> Module                        // the module instantiated
  fn ports(self) -> Selection<Port>            // this instance's port pins
  fn params(self) -> Selection<Param>          // bound params (settable → override)
  fn children(self) -> Selection<Instance>     // descends into of()
  fn port(self, name: String) -> Option<Port>
  fn param(self, name: String) -> Option<Param>
  fn net(self, port: String) -> Option<Net>    // the net wired to `port` (the dac.out access)
```

`net(port)` is the reflective form of the source-level `dac.out`: it returns the net wired to
that pin, which a parent's `analog` block may contribute to. Writing `instance.param("r")` (or
the sugar `instance.r = ...`) stages an override scoped to this instance's path.

### 4.3 `Net`

A signal carrier in the elaborated netlist.

```
Net
  fn id(self) -> NetId                         // stable; the LVS / extraction anchor
  fn name(self) -> String
  fn discipline(self) -> Discipline            // its net type's discipline
  fn resolution(self) -> Resolution            // Single | Tri | Or | And | Kcl
  fn width(self) -> Natural                    // 1 for a scalar net; N for a bus
  fn line(self, i: Natural) -> Option<Net>     // the i-th line of a bus
  fn is_port(self) -> Boolean
  fn drivers(self) -> Selection<Port>          // pins that drive it
  fn loads(self) -> Selection<Port>            // pins that read it
  fn connected(self) -> Selection<Port>        // all attached pins
```

A bus `Bit[N]` is one `Net` of `width N`; `line(i)` reaches an individual line. `drivers()` and
`loads()` give the connectivity an extractor or a checker walks.

### 4.4 `Port`

A typed, directional pin on a module or an instance.

```
Port
  fn name(self) -> String
  fn direction(self) -> Direction              // Input | Output | Inout
  fn net_type(self) -> Type                    // the discipline or net-bundle type
  fn net(self) -> Net                          // the net it is bound to
  fn owner(self) -> Node                        // the Module or Instance it belongs to
```

### 4.5 `Param`

A configuration value of a module (its default) or an instance (its binding).

```
Param
  fn name(self) -> String
  fn type(self) -> Type
  fn value(self) -> Value
  fn is_overridden(self) -> Boolean            // true if a binding/override set it
  fn set(self, v: Value) -> Result<Unit, ReflectError>   // stages an override (§3)
  fn owner(self) -> Node                        // Module or Instance
```

### 4.6 `Aspect`

Plugin-defined data attached to a module via an `aspect <schema> M { ... }` block. The data
conforms to the schema the owning plugin registered, so it is typed, not opaque (extensibility
spec).

```
Aspect
  fn schema(self) -> String                    // e.g. "layout"
  fn data(self) -> Value                        // a value conforming to the registered schema
  fn field(self, name: String) -> Option<Value>
  fn owner(self) -> Module
```

### 4.7 `Behavior`

One of a module's `analog` / `digital` blocks. v1 reflects the block's domain and ownership;
reflection into the block's contributions, drives, and events is a later refinement.

```
Behavior
  fn domain(self) -> Domain                    // Analog | Digital
  fn owner(self) -> Module
```

---

## 5. Definition nodes

These reflect declarations rather than placed structure. They are first-class so that tools and
patterns can reason about the type system itself — what a discipline carries, which types
implement a capability, an enum's encoding.

### 5.1 `Bundle`

```
Bundle
  fn name(self) -> String
  fn is_net_capable(self) -> Boolean
  fn type_params(self) -> Vec<String>
  fn const_params(self) -> Vec<String>
  fn fields(self) -> Selection<Field>
  fn capabilities(self) -> Selection<Capability>   // capabilities it implements
```

```
Field
  fn name(self) -> String
  fn type(self) -> Type
  fn default(self) -> Option<Value>
```

### 5.2 `Enum`

```
Enum
  fn name(self) -> String
  fn repr(self) -> Type                        // the underlying Bit[N] representation
  fn variants(self) -> Selection<Variant>
```

```
Variant
  fn name(self) -> String
  fn value(self) -> Value
```

### 5.3 `Discipline`

```
Discipline
  fn name(self) -> String
  fn domain(self) -> Domain                    // Continuous | Discrete
  fn storage(self) -> Option<Type>             // digital: the stored value type
  fn resolution(self) -> Resolution
  fn natures(self) -> Selection<Nature>        // continuous: potential + flow
```

```
Nature
  fn name(self) -> String
  fn kind(self) -> NatureKind                  // Potential | Flow
  fn value_type(self) -> Type
  fn unit(self) -> Option<String>
  fn abstol(self) -> Option<Real>
```

### 5.4 `Capability`

```
Capability
  fn name(self) -> String
  fn supers(self) -> Selection<Capability>     // required (supertrait) capabilities
  fn signatures(self) -> Selection<Signature>  // the methods it declares
  fn implementors(self) -> Selection<Node>     // types (Bundle / primitive) that implement it
```

```
Signature
  fn name(self) -> String
  fn params(self) -> Vec<Type>
  fn returns(self) -> Type
  fn has_default(self) -> Boolean
```

---

## 6. Enumerations and leaf value types

Returned by the accessors above:

```
Direction    ::= Input | Output | Inout
Domain       ::= Analog | Digital | Continuous | Discrete
Resolution   ::= Single | Tri | Or | And | Kcl
NatureKind   ::= Potential | Flow
Kind         ::= Module | Instance | Net | Port | Param | Aspect | Behavior
               | Bundle | Field | Enum | Variant | Discipline | Nature
               | Capability | Signature
ReflectError ::= NotFound | NotSettable | TypeMismatch | OutOfRange
```

`Node` is the supertype of all node types; `Node.kind()` discriminates it, which is how an
untyped host (a generic selector result) recovers the concrete type.

---

## 7. One model, three languages, one ABI

The node interfaces are uniform underneath and idiomatic on top. The wire protocol is a single
shape — a serialized node carries its `kind`, `id`, scalar attributes, and relation axes — so
every host reconstructs the same typed objects from one ABI, and the selector navigates one
graph.

- **Piperine (`bench`).** Nodes are built-in types. Attribute reads and the assignment sugar
  (`inst.r = 0.2e-2`) compile to the accessors and `set` above.
- **Python.** Each node type is a class wrapping the C-API; `__getattr__` / `__setattr__`
  present the same `inst.r = 0.2e-2` surface.
- **Rust.** Each node type is a struct/trait. Without dynamic property assignment, the write is
  explicit — `inst.param("r")?.set(0.2e-2.into())?` — over the identical protocol.

The ABI is therefore part of the language definition, not an attachment: it *is* this API,
expressed as the serialized node protocol plus the per-type method tables. Compiled plugins
(Rust or Python) are loaded the way the solver already loads OSDI compact models — a shared
library exposing a descriptor — and bind these same node types (extensibility spec).

---

## 8. Out of scope (companion documents)

- **Selector spec** — the string query language ("XPath for POM"): axes (`inst::`, `net::`,
  `port::`, `param::`, `aspect::`), `/` and `//`, and `[...]` predicates over attributes. POM's
  relations are its axes and POM's attributes are its predicates; the selector is a thin
  navigation grammar over this model.
- **Extensibility spec** — `aspect` schemas, the plugin descriptor and three verbs
  (`aspects` / `reflect` / `emit`), annotations (`Selection.attach`), and the `bench` toolchain
  (`simulate`, `extract`, `Results`, `Waveform`). These may still reshape POM; this document is
  the stake in the ground they build on.
