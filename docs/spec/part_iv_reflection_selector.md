# Part IV — The POM and the Selector

The **POM** (Piperine Object Model) is the single elaborated object model:
`piperine_lang::pom::Design` and everything reachable from it. Codegen lowers
from it, the hosts navigate it, plugins read and stage into it, and the LSP
indexes it. There is no second model.

Sources: `crates/piperine-lang/src/pom/`, `crates/piperine-lang/src/value.rs`.

## Contents

- §1 The navigability rule
- §2 `Design`
- §3 `Module` and its members
- §4 Net types, branches, natures, disciplines
- §5 Values
- §6 Attributes
- §7 Serialization
- §8 Staging (the plugin write surface)
- §9 The selector language
- §10 Known divergences inside the POM

---

## §1 The navigability rule (unbreakable)

> **The POM's navigability reflects the structure of the original source, never
> the internal structure of elaboration.**

An author reads back their own modules, instances and hierarchy exactly as
written. Internal transforms — hierarchy flattening above all — are codegen
concerns and must never leak into `Design::modules`. Every pass only *adds* or
*validates*. A transform that needs a collapsed form produces a **separate side
artifact** (`Design::flat_modules`, `#[serde(skip)]`).

Consequences a reader can rely on:

- a `generate for` region reads back as the authored construct, not as its
  expansion;
- an auto-inserted connect module is not a member of the authored module;
- monomorphization may *name* a variant (`urc__5`) but the
  `instance → submodule → sub-instances` tree stays walkable.

---

## §2 `Design`

```rust
design.top() -> Option<&Module>            design.set_top(name)
design.module(name) -> Option<&Module>     design.modules() -> impl Iterator
design.flat_module(name) -> Option<&Module>   // derived, codegen's form
design.natures() / nature(name)            design.disciplines() / discipline(name)
design.paramsets() / paramset(name)        design.udp(name) / udps()
design.consts() / const_(name)             design.directives()
design.rfports(module) -> Vec<RfPort>      design.introspection_meta(module)
design.select(path) -> NodeSelection        // §9
design.fork() -> Design                    design.with_overrides_applied(root)
```

Plus the override/staging surface of §8. `Design` also carries provenance —
`origin_of(name)`, `item_file(name)` — so a diagnostic or an LSP jump lands in
the real declaring file, including through `` `include``.

---

## §3 `Module` and its members

```rust
module.name()      module.doc()          module.attributes()
module.ports()     module.params()       module.wires()     module.vars()
module.instances() module.connections()  module.behaviors() module.functions()
module.branches()  module.genvars()      module.events()    module.constraints()
module.ground_names()  module.is_ground(name)  module.timescale()
module.port(name)  module.param(name)    module.wire(name)  module.instance(label)
module.branch(name)
```

| Member | Carries |
|---|---|
| `Port` | name, `Direction`, `NetType`, attributes, doc |
| `Param` | name, `ValueType`, default `Value`, `is_local`, declared range (`contains(v)`), optional `Distribution` (the `tol` extension), attributes, doc |
| `Wire` | name, `NetType`, optional `nodeset` value, attributes, doc |
| `Var` | name, `ValueType`, optional initializer, attributes, doc |
| `Instance` | label, target module name, port `NetRef`s, parameter overrides, attributes, doc |
| `Connection` | an `lhs`/`rhs` `NetRef` pair |
| `Behavior` | kind (`Analog`, `AnalogInitial`, `Always`, `Initial`), sensitivity, statements |
| `Branch` | endpoints, optional name, `BranchKind` (including `Switch`) |
| `PomConstraintStmt` | the authored `require`/`var`/`target`/`cover` statements, in source order across blocks |

Implicit (unnamed) branches are not members: they are created by access-function
call sites and resolved during elaboration.

---

## §4 Net types, branches, natures, disciplines

```rust
pub enum NetType {
    Discipline(String),                 // an analog net, by discipline name
    Digital1364(DigitalNetKind),        // Wire | Tri | Reg | Wreal
    Array(Box<NetType>, u64),           // a vector — width() is the product
}
```

`NetType::Array` exists and `width()` is used by the §6.5.7.1 port-width check,
but **the front end has no way to construct a multi-bit net from source that
also lowers**: no range form parses, and the post-declarator form does not
resolve a bit at codegen time (Part III §5.1). Treat `Array` as present in the
type system and absent from the language until wave 3 lands.

`Nature` carries `units`, `access`, `abstol`, `ddt_nature`/`idt_nature`,
user attributes, and its derivation parent. `DisciplineDecl` binds potential
and/or flow natures and an optional `domain`. Compatibility follows the
derivation family, not name equality.

---

## §5 Values

`piperine_lang::Value` is the POM's value type: `Unit`, `Int`, `Nat`, `Real`,
`Bool`, `Str`, `Complex`, `Quad` (8-level logic), `Logic(LogicVec)` (the
four-state vector form), `EnumVariant`, `Tuple`, `List`, `Record`. Verilog-AMS
source produces `Int`/`Real`/`Str`/`Bool`/`Logic`/**`List`** (the `'{...}`
array assignment-pattern extension lowers to `List` — confirmed at T55, not a
host/attribute-only variant); the remaining three (`EnumVariant`, `Tuple`,
`Record`) exist for the host and attribute surfaces and, there, only as
retired-front-end survivors, proven unreachable from Verilog-AMS — §10.

---

## §6 Attributes

An `Attribute` is `{ schema, data: HashMap<String, Value> }`. Attribute
instances (`(* … *)`) attach to modules, ports, nets, parameters, variables,
instances and statements. Three schemas are recognized:

| Schema | Owner | Fields |
|---|---|---|
| `device` | plugin system | `device` (plugin name), `type` (device type id) |
| `port` | plugin system | `port`, `kind` |
| `rfport` | solver core | `num` (1-based), `z0` (positive) |

Any other schema name is a loud `UnknownAttrSchema` (`E2022`). Plugins
contribute **no** schemas of their own; the two plugin-facing schemas are
seeded once by the plugin system, so two plugins can never collide. The LRM's
four *standard* attributes (`desc`, `units`, `op`, `multiplicity`, §2.9.2) are
**not** registered today and are therefore rejected — Part III §5.1.

---

## §7 Serialization

`Design` implements serde and round-trips as itself, doc comments included
(`crates/piperine-lang/tests/pom_serde.rs`). `flat_modules` is
`#[serde(skip)]`: the wire form carries the authored design, never the derived
collapse.

---

## §8 Staging (the plugin write surface)

Plugins do not mutate the POM directly. They stage:

```rust
design.set_param(path, param, value)      design.get_override(path, param)
design.has_overrides()                    design.clear_overrides()
design.stage_instance(...)                design.stage_connection(...)
design.with_overrides_applied(root)  ->   Design
```

Staged instances and connections live in a separate staging area and are
applied into a derived design; the authored `Design::modules` is never
rewritten (§1). Type errors on a staged value are `E3003`; an unknown target is
`E3001`; a non-settable target is `E3002`. See Part VI §8.

---

## §9 The selector language

`Design::select(path)` evaluates a small XPath-like query over the POM and
returns a `NodeSelection`.

```text
selector  ::= ['/' ['/']] step { '/' step }
step      ::= [axis '::'] name_or_'*' { predicate }
predicate ::= '[' (index | 'last()' | expr) ']'
```

Axes (`pom::selector::ast::Axis`) and their status:

| Axis | Meaning | Implemented |
|---|---|---|
| `inst::` | child instances (the default axis) | yes |
| `net::` | nets/wires of the current module | yes |
| `port::` | ports | yes |
| `param::` | parameters | yes |
| `behavior::` | behavior blocks | yes |
| `attr::` | attributes | yes |
| `constraint::` | constraint statements (`Module::constraints()`, T49) | yes |
| `driver::`, `load::`, `parent::`, `ancestor::` | connectivity/upward traversal | **parse; fail loud** (`SelectorError::AxisNotImplemented`) rather than traverse — §10 |

Predicates support positional indexing, `last()`, and a boolean expression over
node properties (name, kind, attribute values) with `and`/`or`/comparison.

`Selection<T>` offers `len`/`is_empty`/`first`/`get`/`iter`/`one`/`filter`/`map`;
`one()` fails with a `ReflectError` when the match is not unique.

---

## §10 Known divergences inside the POM

Regenerated at T55 (`lrm-conformance-wave3`) against what T49/T50 actually
changed — three of the four items below are closed or corrected; only the
connectivity axes remain a real, now explicitly loud, gap.

1. **`Kind` is wider than the node graph — closed as "reserved, proven
   unreachable" (T50).** `pom::node::Kind` still lists `Enum`, `Bundle`, and
   `Capability` — value-layer concepts of the retired PHDL front end. No
   `Node` variant exists for any of the three, so no traversal can produce
   them; each now carries a `#[doc(hidden)]` "reserved — unreachable from
   Verilog-AMS" doc line, and
   `crates/piperine-lang/tests/pom_retired_variants_unreachable.rs` proves it
   both by a full-graph traversal and by a whole-crate reference scan.
   Removal is still deferred (a public enum variant, breaking change) but the
   claim is proven, not asserted.
2. **`constraint::` now dispatches for real (T49, closed).** The evaluator's
   `Axis::Constraint` arm was empty (`Axis::Constraint => {}`); it now walks
   `Module::constraints()` and returns each authored `require`/`var`/
   `target`/`cover` statement as a new `Node::Constraint`/`Kind::Constraint`
   node, proven by
   `crates/piperine-lang/tests/pom_selector_constraint_axis.rs`. This is no
   longer a divergence.
3. **Four axes are accepted and fail loud, not silently empty (T49
   investigation, no code change needed).** `driver`, `load`, `parent`,
   `ancestor` parse but connectivity traversal has not been rebuilt on the
   flattened form. Investigation for T49 found `walk_axis` already returns
   `SelectorError::AxisNotImplemented` for all four — propagated through
   `eval_step`'s `?`, failing the whole `select()` call — so this was never
   the silent "empty selection" this document previously described; that
   description was itself the stale artifact.
   `pom_selector_constraint_axis.rs::other_inert_axes_fail_loud_rather_than_
   return_an_empty_selection` pins the loud-error behavior so a regression
   that silently swallowed the error would be caught. The underlying gap —
   no real connectivity traversal — remains open; only its *failure mode* was
   ever a documentation defect, not a behavioral one.
4. **`Value` carries retired variants — narrowed to three, one corrected
   (T50).** `Record`, `Tuple`, and `EnumVariant` are genuinely unreachable —
   no Verilog-AMS construct produces them, proven by the same whole-crate
   reference scan as item 1. **`List` was miscategorized here** — it is
   reachable via the `'{...}` array assignment-pattern extension
   (`elab::const_eval::ConstEnv::eval_array`), proven live by
   `pom_retired_variants_unreachable.rs::list_is_reachable_via_array_
   assignment_pattern`, and is no longer listed as retired.

Every remaining item above is a documentation-visible divergence, not a
silent one: items 1 and 4's variants are unreachable but exist only as
reserved enum space (never surfaced through a live traversal), and item 3's
axes fail loud rather than returning an empty selection. There is no longer
a case in the selector surface where "nothing happened" passes as a silent,
unmarked empty result.
