# Part IV — Reflection & Selector

The Piperine Object Model (POM) and the selector query language are one surface: the
selector's axes *are* POM relations, and its predicates *are* POM attributes. The POM
exposes an elaborated `.vams` design as a graph of typed runtime nodes — the interface
the Python host, the Rust host (Part VIII), the plugin ABI (Part VI), and the language
server all read the same design through.

POM is **read-first**: querying has no effect; assigning a parameter **stages an
override** consumed by a later pure re-elaboration (Part II). The selector adds no model
of its own — it is a query language layered over the POM's existing structure.

This Part describes the POM as it exists after the `verilog-ams-support` migration
(`crates/piperine-lang/src/pom/`). Two structural facts drive most of what changed from
the PHDL-era POM: **natures and branches are now first-class, top-level or per-module
nodes** (LRM §3.6.1, §3.12 — PHDL folded a nature inline inside a discipline and had no
branch type at all), and **the PHDL value layer that made bundles/enums/capabilities
first-class POM citizens is gone** (MD-45) — those `Kind` variants and node accessors
still exist as vestiges in the type definitions but are unreachable through the live
graph. Both facts are called out explicitly below rather than silently dropped.

## Contents

- §1 Core concepts
- §2 Design root
- §3 Staging and determinism
- §4 Elaborated-structure nodes
- §5 Natures, disciplines, branches
- §6 The `Node` graph and its gaps
- §7 One model, two languages, one ABI
- §8 Selector — model
- §9 Selector — grammar
- §10 Selector — axes
- §11 Selector — node tests
- §12 Selector — predicates
- §13 Selector — evaluation
- §14 Selector — integration and its own live gap

---

## §1 Core concepts

**Distinct typed nodes.** Each construct is its own Rust type with its own interface —
`Module` has `ports()`, `Instance` has `of()` (via `module_name()`/lookup), a `Nature`
has `abstol()`. The construct-level POM types, all in `crates/piperine-lang/src/pom/`:

- `Module`, `Instance`, `Port`, `Param`, `Wire`, `Var`, `Connection`, `Attribute`
  (`module.rs`) — the structural graph of one elaborated module.
- `Behavior` (`behavior.rs`) — one `analog`/`digital` block; its body is `BehaviorStmt`,
  a direct alias of `crate::ast::Stmt` (§4 below — no separate resolved-behavior layer).
- `Nature` (`nature.rs`) and `Branch`/`BranchKind` (`branch.rs`) — LRM §3.6.1 and §3.12,
  new in the VAMS-era POM (§5).
- `Design` (`design.rs`) — the root; owns every module plus the top-level
  natures/disciplines/paramsets/consts.

There is no generic node wrapper you must downcast into these — you reach a `Module`
straight from `Design::module(name)`, an `Instance` from `Module::instance(name)`, and
so on through plain typed accessors. The discriminated `Node<'a>` enum described in §6
exists *in addition* to this, purely as the selector's uniform navigation type; it is
narrower than the full set of POM node types (§6 explains the gap).

**Two-axis model.** Every construct offers two kinds of access:

- *Attributes* — scalar properties returning a value or a `Value`. Examples:
  `Module::name()`, `Nature::abstol()`, `Port::direction()`. A few are settable (params,
  via staged overrides); the rest are read-only accessor methods.
- *Relations* — accessor methods returning a slice or `Selection` of related nodes.
  Examples: `Module::ports()`, `Module::instances()`, `Design::modules()`.

This is what makes the selector work (§8 onward): its axes are these same relations,
its predicates test these same attributes.

**Value.** The value layer used by params, consts, and attribute data:
`crate::value::Value` — the LRM's value types (`Real`, `Integer`, `Boolean` (2-state),
a 4-state logic value, `String`) plus their aggregate forms. `UInt`/`SInt` bit-vector
convenience types that PHDL's stdlib defined as bundles do not exist in the VAMS POM;
VAMS parameters and `var`s carry LRM's own type set directly (LRM §3.2/§3.3, ch. 4).

**Selection\<T\>** — the universal navigation result returned by relation accessors
(`crate::pom::selection::Selection<T>`, `crate::pom::selection::NodeSelection<'a>` is
its `Node<'a>` specialization):

```
len, is_empty
first() -> Option<&T>   ; get, iter, etc.
where_path(design, path) -> Result<NodeSelection, SelectorError>   (NodeSelection only)
```

`where_path` is how a selection re-enters the selector grammar (§8) for a further step.

---

## §2 Design root

`Design` (`crate::pom::Design`) is the root, returned by
`piperine_lang::SourceFile::elaborate` / `parse_and_elaborate` (Part VIII §2):

```
Design
  top() -> Option<&Module>
  module(name) -> Option<&Module>
  flat_module(name) -> Option<&Module>     // FlattenHierarchy's derived form, codegen-only
  modules() -> impl Iterator<Item = &Module>
  module_count() -> usize
  select(path) -> Result<NodeSelection, SelectorError>
  const_(name) -> Option<&Value>
  consts() -> impl Iterator<Item = (&String, &Value)>
  discipline(name) / disciplines() -> Option<&DisciplineDecl> / iterator
  nature(name) / natures() -> Option<&Nature> / iterator
  paramset(name) / paramsets() -> Option<&ParamsetDecl> / iterator
  project() -> &Project
```

There is no `bundles()`/`enums()`/`capabilities()` on `Design` — PHDL's value-layer
definition nodes (§6). `flat_module` reaches `Design::flat_modules`, the codegen-only
derived artifact `FlattenHierarchy` builds (Part II §5); `module`/`modules()` always
return the **authored** hierarchy (the POM-navigability unbreakable rule, CLAUDE.md).

`design.select(path)` is the entry point for the selector language (§8); a returned
`NodeSelection` re-roots further steps via `where_path`.

---

## §3 Staging and determinism

Writing a parameter stages an override rather than editing the design in place:
`Design::set_param(path, param, value)` records the override; `Design::with_overrides_
applied(root_module)` produces a fresh, purely re-elaborated `Design` — Part II's
elaboration contract (pure, total) holds across this boundary. `get_override`/
`has_overrides`/`clear_overrides`/`fork` round out the staging surface. Structural
staging (`stage_instance`/`stage_connection`) is the mechanism plugin `transform_design`
hooks use (Part VI §8.2) — additive only, never a mutation of authored structure.

Selector-level "reflection" errors, `ReflectError` (`pom/error.rs`):

| Code | Variant | Trigger |
|------|---------|---------|
| E3001 | `NotFound` | a name or path was not found |
| E3002 | `NotSettable` | the attribute is read-only |
| E3003 | `TypeMismatch` | the value's type does not match |
| E3004 | `OutOfRange` | the value is outside the accepted range |
| E3005 | `MultipleDrivers` | staging would create a multi-driver conflict on a discipline that does not resolve |
| E3999 | `Other(String)` | catch-all |

No in-place mutation occurs. There is no escape hatch around `NotSettable`.

---

## §4 Elaborated-structure nodes

**Module** (`pom::Module`) — `name()`; `doc()`; `attributes()`; `ports()`, `params()`,
`wires()`, `vars()`, `instances()`, `connections()`, `behaviors()`, `branches()`
→ `&[T]`; `genvars()` → `&[String]` (LRM §3.5 loop-index names declared in this module,
generated instances themselves land only in `Design::flat_modules`, never here — MD-25);
`port(n)`, `param(n)`, `wire(n)`, `instance(n)`, `branch(n)` → `Option<&T>`. There is no
combined `nets()` accessor merging ports and wires, and **no `constraints()`** — see the
callout below.

**Instance** (`pom::Instance`) — `name()`, `attributes()`, `module_name()` (the callee
module's name — `of()` in the old PHDL-era phrasing no longer exists as a method that
returns the resolved `Module`; a caller resolves it via `design.module(inst.module_
name())`), `ports()` → `&[NetRef]` (positional/named connection list as authored),
`params()` → `&[(String, Value)]` (param overrides given at the instantiation site),
`label()`, `doc()`.

**Port** — `name()`, `direction()` → `Direction` (`Input`/`Output`/`Inout`, LRM §3.7),
`net_type()` → `NetType`, `doc()`, `attributes()`.

**Param** — `name()`, `value_type()` → `ValueType`, `default()` → `Option<&Value>`,
`doc()`, `is_local()`, `distribution()` → `Option<&Distribution>` (the dv-core `tol`
surface — currently dark, §5's note applies equally here: `Distribution::kind` used to
resolve against a PHDL statistics header deleted at T40 and has no VAMS Annex A
production yet).

**Wire** — `name()`, `net_type()`, `doc()`, `attributes()`.

**Connection** — `lhs()`/`rhs()` → `&NetRef` (a `connect a, b;`-shaped structural tie).

**Attribute** (`pom::module::Attribute`) — the `(* schema = ..., field = ... *)` carrier
(Appendix B §B.2, Part I §8): `schema()`, `data()` → `&HashMap<String, Value>`,
`field(name)` → `Option<&Value>`. Every attribute instance on a module or port that does
not match one of the plugin system's seeded `device`/`port` schemas fails loud at
elaboration (`ElabErrorKind::UnknownAttrSchema`) — it never reaches the POM as an inert,
un-typed blob.

**Behavior** — `name()`, `doc()`, `kind()` → `crate::ast::BehaviorKind` (`Analog` |
`Digital`), `body()` → `&[BehaviorStmt]`, `var_types()` — the resolved value types of
the body's `var` declarations, a side table elaboration builds on top of the surface
AST. **`BehaviorStmt` is `pub use crate::ast::Stmt as BehaviorStmt` — a direct type
alias, not a distinct resolved-behavior AST layer** (confirmed against
`crates/piperine-lang/src/pom/behavior.rs`'s own doc comment: "Behavior bodies are the
surface `Stmt` type directly (design F1, preserved across the MD-44 frontend swap —
only the surface AST's identity changed, `piperine_lang::parse::ast` → `crate::ast`)").
Where PHDL's POM once carried an independently-typed resolved-statement form, VAMS
behavior bodies are the same `crate::ast::Stmt`/`Expr` that the parser produced,
structurally unrolled where elaboration folds `generate` regions but otherwise
unchanged; `piperine-codegen`'s own `resolve` layer (Part X) is where a second,
codegen-private resolved form is built, downstream of the POM.

> **Removed: `ConstraintBlock`.** PHDL's `constraint Name { ... }` node and its
> `Module::constraints()` accessor do not exist in the current POM. T21/MD-45 deleted
> `pom::ConstraintBlock` outright along with the rest of the PHDL value layer; T35
> explicitly scoped the VAMS migration's attribute work to *only* the `device`/`port`
> schemas, leaving the general constraint/capability rebuild as declared future work.
> `.specs/features/verilog-ams-support/validation.md` Gap 3 records this precisely: the
> spec's own P2 story for an attribute-instance-carried constraint block (VAMS-14) was
> never delivered, and no later task picked it back up despite the plan's own "must add
> a task before Phase 7 closes" gate. There is currently no way to author a
> `ConstraintBlock`-equivalent through `.vams` source at all — this is real,
> undelivered functionality, not merely an undocumented POM shape.

---

## §5 Natures, disciplines, branches

Two constructs the PHDL-era POM had no equivalent for at all:

**Nature** (`pom::Nature`, LRM §3.6.1) — a stand-alone, top-level, independently
declared continuous-quantity kind (`Voltage`, `Current`, …), looked up by name from a
discipline's `potential`/`flow` binding and from `idt_nature`/`ddt_nature` references.
Fields: `name`, `parent` (`Option<String>` — a derived nature per §3.6.1.1), and the
five LRM-standard attributes (§3.6.1.2) as **named fields, not a map** — `abstol`
(`Option<f64>`, drives solver per-net tolerance), `access` (`Option<String>`, the
access-function name, e.g. `V`/`I`), `units` (`Option<String>`), `idt_nature`,
`ddt_nature` (both `Option<String>`, default to the nature itself when unset). Named
fields, deliberately, over a stringly-keyed map — a typo'd key in a map would fail
silently, violating the project's fail-loud rule. User-defined attributes (§3.6.1.3)
live in a separate map, preserving the LRM-standard/author-defined split. Natures do not
nest inside modules; `Design::natures()` is the only place to look them up.

**Discipline** — still represented by the AST's own `DisciplineDecl` type
(`crate::ast::DisciplineDecl`), reached via `Design::discipline(name)`/`disciplines()`,
rather than by a dedicated POM `Discipline` struct with a `Kind`/`storage()`/
`resolution()` reflection surface. A discipline binds a nature to `potential` and/or
`flow` (LRM §3.6.2) — this binding is read directly off the AST node today; there is no
richer POM-native `Discipline` reflection type analogous to `Nature`.

**Branch** (`pom::Branch`/`BranchKind`, LRM §3.12/§5) — a named or implicit path between
two nets. PHDL had no first-class branch type at all; contributions resolved through
net/terminal references directly. VAMS's branches are explicit and nameable
(`branch (p,n) name;`) or created implicitly by an access-function call site's first
use (§5.4.1). `BranchKind` tracks how a branch is driven: `Probe` (read but never
contributed — the default state), `Potential`, `Flow`, or `Switch` (the one legal
exception to "a branch cannot be contributed as both potential and flow," LRM §5.6.5,
for a branch that legitimately toggles kind at runtime through conditional contribution
statements). A branch contributed as both potential and flow outside the switch case is
`ElabErrorKind::BranchKindConflict` (E4001) — a fail-loud elaboration error, not a
silent last-write-wins resolution. `Module::branches()` reaches a module's own declared
and implicit branches.

---

## §6 The `Node` graph and its gaps

`crate::pom::node::Kind` — the discriminant every generic-node API switches on — still
lists **fourteen** variants: `Module`, `Instance`, `Port`, `Param`, `Wire`, `Var`,
`Behavior`, `Discipline`, `Nature`, `Branch`, `Enum`, `Bundle`, `Capability`,
`Attribute`. But `crate::pom::node::Node<'a>` — the actual enum every selector-facing
API (`select`, `NodeSelection`) constructs and returns — carries only **seven**:

```rust
pub enum Node<'a> {
    Module(&'a Module), Instance(&'a Instance), Port(&'a Port), Param(&'a Param),
    Wire(&'a Wire), Behavior(&'a Behavior), Attribute(&'a Attribute),
}
```

`Var`, `Discipline`, `Nature`, `Branch`, `Enum`, `Bundle`, and `Capability` are `Kind`
variants with **no corresponding `Node` case** — they exist as POM struct types (`Var`,
`Nature`, `Branch` are real and populated; `Enum`/`Bundle`/`Capability` are PHDL-value-
layer vestiges with no VAMS producer at all) but are unreachable through the generic
`Node`/selector graph walk. This is confirmed at `crates/piperine-lang-server/src/
symbol_index.rs`'s own comment: PHDL's bundle/enum/capability symbol kinds "are gone,
not retargeted." Concretely: `select("//nature::Voltage")` or any selector path trying
to reach a `Nature`/`Branch` node has no axis to do so (§10's axis table below has no
`nature::`/`discipline::` entry either) — reaching a nature or branch today means going
through the typed `Design::nature(name)`/`Module::branch(name)` accessors directly, not
through the selector or the generic `Node` graph. This is a real, load-bearing gap in
the reflection surface for two constructs the VAMS migration itself introduced, not an
inherited PHDL-era limitation.

---

## §7 One model, two languages, one ABI

The POM is uniform underneath and idiomatic on top. Every host reaches the same typed
objects, in the host's own idiom:

- **Python** — classes with `__getattr__`/`__setattr__`, with assignment sugar
  (`inst.r = ...`) — Part VIII, Appendix C.
- **Rust** — the structs and typed accessors described above, with explicit calls
  (`inst.param("r")`, `param.set(...)` via the staging API of §3).

Compiled device plugins (native or embedded-Python) reach the POM through the same
reflection surface the hosts use — Part VI.

---

## §8 Selector — model

The selector is the "XPath of the circuit." It evaluates against a design and returns a
`NodeSelection`. It adds no model of its own:

> **Axes are POM relations. Predicates are POM attributes.**

A path is a sequence of **steps**. Each step moves along an **axis**, keeps nodes
matching a **node test**, and filters through **predicates**. Results union across
context nodes and preserve first-seen order (`crate::pom::selector::{ast, parse, eval}`).

A selector is evaluated against a context node: `design.select(path)` (rooted at the
design), or `node_selection.where_path(design, path)` (rooted at each node currently
selected).

---

## §9 Selector — grammar

```
Selector  ::= [ "/" | "//" ] Step { ( "/" | "//" ) Step }
Step      ::= [ Axis "::" ] NodeTest { Predicate }
Axis      ::= "inst"|"net"|"port"|"param"|"attr"|"behavior"|"constraint"
             |"driver"|"load"|"parent"|"ancestor"
NodeTest  ::= Name | "*"
Predicate ::= "[" ( Index | PredExpr ) "]"
```

(`crate::pom::selector::ast::Axis` — see §10 for which of these eleven names actually
dispatch to something today.)

---

## §10 Selector — axes

| Axis | Dispatches to | Status |
|------|----------------|--------|
| `inst::` *(default)* | `Module::instances()` | live |
| `net::` | port/wire lookup by name | live |
| `port::` | `Module::ports()` | live |
| `param::` | `Module::params()` | live |
| `attr::` | `Module::attributes()` | live |
| `behavior::` | `Module::behaviors()` | live |
| `constraint::` | nothing — `Axis::Constraint => {}` in `selector/eval.rs`, a no-op arm | **dead**: `Module::constraints()` does not exist (§4's `ConstraintBlock` removal); a selector step on this axis always returns empty, silently, not a parse error |
| `driver::` | net drivers | live |
| `load::` | net loads | live |
| `parent::` | reverse of `instances()` | live |
| `ancestor::` | transitive closure of `parent::` | live |

There is no `nature::`/`discipline::`/`branch::` axis — §6's gap, not merely an
oversight in this table.

---

## §11 Selector — node tests

`*` matches any node. A name matches by the node's own name; on `inst::`, a name may
match either by instance label or by the callee module's name, mirroring how a source
file itself names an instantiation (`crate::pom::selector::eval` resolves this).

---

## §12 Selector — predicates

A predicate filters the step's result set:

- **Positional:** `[i]` (0-based index) or `[last()]`.
- **Expressional:** a boolean expression over `@name`/`@schema.field` attribute
  references, axis existence/comparison (`[net::clk]`, `[param::r > 1k]`), string glob
  match (`[@name ~ "cmp*"]`), and the boolean combinators `and`/`or`/`not(...)`.

---

## §13 Selector — evaluation

Per step over a node set S: for each node n in S, follow the axis from n, keep nodes
matching the node test, apply predicates left-to-right, then union all results.

An empty result is valid, not an error. Evaluation is a pure function of the elaborated
`Design` plus staged overrides.

| Variant | Trigger |
|---------|---------|
| `EmptySelector` | empty selector string |
| `ExpectedDoubleColon` | missing `::` after an axis |
| `ExpectedNodeTest` | missing node test after an axis |
| `UnknownAxis(String)` | axis name not in the table |
| `AxisNotImplemented(Axis)` | axis exists in the enum but has no lowering |

---

## §14 Selector — integration and its own live gap

```python
for r in design.select("//Resistor").nodes():
    print(r.param("r").value())

design.select("//dac/param::vref").set(1.8)
```

A plugin's `transform_design` hook stages structural additions over the same model
(`staging.add_instance(...)` — Part VI §8.2); the selector itself is read-only apart
from the staged parameter writes of §3.

The one concrete, load-bearing limitation to carry forward from this Part: a `.vams`
design's own new POM constructs — `Nature` and `Branch` — are **not** selector-
addressable (§6, §10), and the `constraint::` axis is silently dead (§10). Extending
`Node<'a>` to cover `Var`/`Nature`/`Branch` and wiring matching axes is open work, not
speculative — it is the natural next step given the VAMS migration already built the
underlying `Nature`/`Branch` POM types.
