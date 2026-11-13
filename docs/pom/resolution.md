# Name resolution

Name resolution runs inside `build()`, after lowering, in a fixed sequence of stages. It reads
the immutable arenas, computes answers, and writes them only into the *slots* the nodes
already have (a `Resolution`, a `CallClass`, a `StructClass`, a `PortKind`, the bundle flag of a
struct) and into the diagnostics list. It never adds, removes or moves a node, so the structure
you read (see [the rule](index.md#the-one-rule)) is the structure that was written.

A name that cannot be resolved is a hole in the navigation, not a failure: the slot stays
`Unresolved`, one `E0161` is reported at the path, and everything else still resolves.

## The stages

| Stage | What it does | Writes |
| --- | --- | --- |
| 0. Load | finds and reads the files of the source tree (`mod b;` follows Rust's layout) | files, `E0161` for a missing, ambiguous or cyclic `mod` file |
| 1. Lower | builds the declaration tree and the body arenas from the syntax tree, with every slot unset | all nodes |
| 2. Scopes | per-namespace type and value tables, per-module member tables | `E0457`, `E0330` |
| 3. Imports | `use` items, `self`/`super`/`crate`, visibility, prelude sources | import resolution |
| 4. Behaviors | binds each `analog`/`digital` block to its module | `BehaviorData::module`, `ModuleData::behaviors`, `E0401`, `E0402`, `E0403` |
| 5. Names | every path with its lexical scope stack | `PathSegment::res`, `E0161` |
| 6. Classification | calls, struct expressions, ports, bundles, access calls | `CallClass`, `StructClass`, `PortKind`, `StructData::bundle`, `E0130` |
| 7. Members | `e.name` and `e.m()` through declared types | `ExprKind::Field::res`, `MethodCall::res`, `E0343` |
| 8. Index | `references`, `writers`, `instantiated_by` | the [`Index`](model.md#index) |

Diagnostics are sorted by file, then span start, then code after the last stage, so their order
does not depend on the stage that found them.

## Scope order

A single-segment name is looked up in this order; the first hit wins.

1. **The lexical scope stack**, innermost first: block `let`s and block items, a binder's pattern
   (closure parameters, `match` arms, `for`), function parameters, the generics of every enclosing
   declaration, `Self`, and the members of the enclosing module (its ports, params, signals
   and body `let` names). An item nested in a block does not see the locals and generics around
   it, only the items of the blocks it sits in. A `use` in a block is one of those items: its
   names are visible in the whole block (also before the `use`), not outside it, and a path that
   does not resolve is one `E0161`, as at the top level.
2. **The items and imports of the enclosing namespace**: its own declarations, its `use` leaves
   and its glob imports.
3. **The prelude sources**: the `pub` items of every prelude source are visible in every file.
4. **The built-in table** (below).

A multi-segment path resolves one segment at a time: the first as above (or `crate`, `self`,
`super`), each next one inside what the previous named (a namespace's items, a module's
members, an enum's variants, the associated items of a type). Each segment keeps its own
`res`; the path's resolution is its last segment's.

Names live in two tables per namespace, **types** and **values**. A path in a type position
prefers the type table, a path in an expression prefers the values; a module is in both, so a
module can be named as a type (`p: Cell`) and called (`Cell(..)`).

**Visibility.** An item without `pub` is visible inside its own namespace and the namespaces
below it. A path that reaches a private item of another namespace still resolves (navigation
works) and reports `E0161` with the label "private item". A module's members are reached by
member access with `E0343` instead (see [Members](#members)).

**Repeated names.** A lookup of a name declared twice returns the first declaration. Both stay
in the tree; a repeated member name of a module is reported as `E0330`.

```msl
nature Bit { value: bool }
const K: f64 = 1.0;
module Top(a: Bit) {
    let K = 2.0;            // a body `let` shadows the namespace item
    signal s: Bit;
    s = a;                  // `a`: a port of the enclosing module
}
```

```rust
use piperine_lang::pom::{Design, Reference, Reflect};

let src = r#"
nature Bit { value: bool }
const K: f64 = 1.0;
module Top(a: Bit) {
    let K = 2.0;
    signal s: Bit;
    s = a;
}
"#;
let design = Design::builder().source("top.msl", src).build();
assert!(design.diagnostics().is_empty());
let top = design.module("Top").unwrap();
// `a` resolved to the port of the enclosing module.
assert_eq!(top.port("a").unwrap().references().len(), 1);
// The namespace-level `K` is not referenced: the body `let K` shadows it in `Top`.
let k = design.root().children().into_iter().find_map(|d| d.as_const()).unwrap();
assert!(k.decl().references().is_empty());
```

### Imports and the prelude

```rust
use piperine_lang::pom::{Design, Reflect};

let design = Design::builder()
    .prelude("std.msl", "pub nature Bit { value: bool }")
    .source("top.msl", "mod lib { pub module Cell(p: Bit) {} }\nuse lib::Cell;\nmodule Top(p: Bit) { let c = Cell(p); }")
    .build();
assert!(design.diagnostics().is_empty());
// `Bit` comes from the prelude, `Cell` through the `use`; the instance is linked.
let top = design.module("Top").unwrap();
assert_eq!(top.instance("c").unwrap().module(), design.module("Cell"));
assert_eq!(top.port("p").unwrap().nature(), design.nature("Bit"));
```

## The built-in table

The names the language provides without a declaration. `builtin(name)` finds a name;
`builtin_method(name)` finds a method row (methods are reached only as members, never as names).
A built-in resolves as `Resolution::Builtin(BuiltinId)`; `BuiltinId::info()` is its row and
`BuiltinId::name()` its exact spelling (`u8` for the width-8 entry of the `uN` family).

| Kind | Names (section) |
| --- | --- |
| `Type` | `uN` (§1), `iN` (§1), `fN` (§1), `bool` (§1), `usize` (§1), `isize` (§1), `UInt` (§1), `Int` (§1), `Float` (§1), `Option` (§1), `Event` (§13), `Analysis` (§11.1) |
| `Variant` | `Some` (§1), `None` (§1), `Dc` (§11.1.1), `Tran` (§11.1.1), `Ac` (§11.1.1), `Noise` (§11.1.1), `Static` (§11.1.1) |
| `Trait` | `Repr` (§2), `Fn` (§3), `Clone` (§3), `Copy` (§3) |
| `Const` | `Q` (§1), `K_B` (§1), `EPS0` (§1), `PI` (§1), `E` (§1), `inf` (§1) |
| `Fn` | `potential` (§14.1), `flow` (§14.1), `ddx` (§14.2), `ac_stim` (§14.3), `temperature` (§14.4), `simparam` (§14.4), `analysis` (§14.4), `now` (§14.4), `freq` (§14.4) |
| `Module` | `ddt` (§14.5), `delay` (§14.6), `limit` (§14.7), `noise` (§14.8), `cross` (§14.9), `at` (§14.9) |
| `Event` | `settle` (§14.9) |
| `Statement` | `bound_step` (§14.10) |
| `Method` | `changed` (§14.9), `prev` (§14.12), `others` (§14.12), `level` (§14.12), `inflow` (§14.1) |

The rows `uN`, `iN` and `fN` stand for every width (`u1`..`u128`, `i1`..`i128`, `f16`, `f32`,
`f64`, `f128`). Standard-library modules such as `idt`, `transition` and `above` are *not*
built in: they are declared in the standard library's prelude sources (Part VII of the
specification), and a design that does not load them leaves them `Unresolved`. A namespace item named like a core
primitive of §14 is `E0457`.

```rust
use piperine_lang::pom::{builtin, builtin_method, BuiltinKind, Design, Resolution};

assert_eq!(builtin("u8").unwrap().width(), Some(8));
assert_eq!(builtin("u8").unwrap().name(), "u8");
assert_eq!(builtin("Some").unwrap().info().kind, BuiltinKind::Variant);
assert_eq!(builtin("ddt").unwrap().info().kind, BuiltinKind::Module);
assert!(builtin("changed").is_none());                    // a method row is not a name
assert_eq!(builtin_method("changed").unwrap().info().kind, BuiltinKind::Method);
assert!(builtin("u0").is_none() && builtin("nothing").is_none());

// A path to a built-in type resolves to the table, with no declaration.
let design = Design::builder().source("top.msl", "const K: u12 = 1;").build();
let k = design.root().children()[0].as_const().unwrap();
let _ = k;
let id = design.type_node(piperine_lang::pom::TypeId(0));
match &id.kind {
    piperine_lang::pom::TypeKind::Path(p) => {
        let res = design.path_node(*p).segments[0].res;
        assert!(matches!(res, Resolution::Builtin(b) if b.name() == "u12"));
    }
    other => panic!("{other:?}"),
}
```

## Classification

Some shapes are decided by what a name resolves to, not by syntax ([Appendix B.3.3](../spec/appendix_b_grammar.md)). The POM
writes the decision into the node's slot, so the call, the struct expression or the port keeps
its authored shape.

| Shape | Slot | Value | When |
| --- | --- | --- | --- |
| `path(..)` | `ExprKind::Call::class` | `CallClass::Instance { module, via }` | the callee names a module, or a value that names one (`via` is that template, param, binding or function) |
| | | `CallClass::Probe` | the callee is an access name of a nature (`v`, `i`, ..) or `potential` / `flow` |
| | | `CallClass::Function` | otherwise |
| `Path { .. }` | `ExprKind::Struct::class` | `StructClass::Template(decl)` | the path names a module |
| | | `StructClass::BundleLiteral` | the path names a bundle |
| | | `StructClass::StructLiteral` | otherwise |
| `name: T` in a port list | `PortData::kind` | `PortKind::Carrier` | `T` resolves to a nature, or to a generic whose bound resolves to the prelude's `Nature` (or an array / `flip` of one) |
| | | `PortKind::Bundle` | `T` resolves to a bundle struct |
| | | `PortKind::ValueInput` | `T` is a value type |
| | | `PortKind::Unknown` | `T` does not resolve, or is a generic with a bound that does not resolve |
| `struct S { .. }` | `StructData::bundle` | `Some(true)` | a field is a carrier, an array of carriers or a bundle |
| | | `Some(false)` | no field is: a plain value struct |
| `nature N { .. }` | `NatureData::kind` | `NatureKind` | computed from the fields: `potential` + `flow` is `Conservative`; `potential` alone `SignalFlow`; `value` alone `Discrete`; `value` + `resolve` `Resolved`; any other mix `Invalid` |

`StructData::bundle` is `None` only on a design that has not been resolved; after `build()`
every struct carries `Some(_)`. A named argument of an instance (`n: b`) links to the port it
names (`Arg::res`), and a name in a param block links to the param (`FieldInit::res`).

```rust
use piperine_lang::pom::{CallClass, Design, ExprKind, Reflect};

let src = r#"
nature Electrical { potential: V { access: v, units: "V", abstol: 1u }, flow: I { access: i, units: "A", abstol: 1p } }
module Cell(p: Electrical) {}
fn half(x: f64) -> f64 { x / 2.0 }
module Top(a: Electrical) {
    let c = Cell(a);
    let h = half(1.0);
    let p = v(a);
}
"#;
let design = Design::builder().source("top.msl", src).build();
let top = design.module("Top").unwrap();
let class = |name: &str| match top.named(name).unwrap().value().unwrap().kind() {
    ExprKind::Call { class, .. } => *class,
    other => panic!("{other:?}"),
};
assert!(matches!(class("c"), CallClass::Instance { module: Some(_), .. }));
assert_eq!(class("h"), CallClass::Function);
assert_eq!(class("p"), CallClass::Probe);
```

## Members

`e.name` and `e.m()` are linked when the declarations alone decide the type of `e`:

| Receiver | `e.name` resolves to | `e.m()` resolves to |
| --- | --- | --- |
| an instance of a known module | the module's item of that name: a port, param, signal or `let` | |
| a value of a declared struct or bundle type (a port, signal or `let` with that annotated type) | the field | |
| a carrier of a nature | | in order: the carrier built-ins (`changed`, `prev`, `others`, `inflow`, `level`), the `impl` blocks of the nature (or bundle), then the `impl` blocks of the nature's value type |
| `self` in an `impl` | | a value of the impl's self type, then as above |
| anything else | `Deferred` | `Deferred` |

* A member item that is not `pub` still resolves, and `E0343` is reported at the member name. A
  member that the instance's module does not have stays `Unresolved` and is also `E0343` (§8.4.8).
* A field a declared struct does not have stays `Unresolved` with no diagnostic: the POM has no
  code for it (only the eight binding codes), and a type checker reports it.
* A method a known receiver does not have stays `Unresolved`, with no diagnostic.
* A receiver whose type needs inference is `Resolution::Deferred(DeferReason::ReceiverTypeUnknown)`
  with no diagnostic: a type checker fills it later.

```rust
use piperine_lang::pom::{DeferReason, Design, ExprKind, Reflect, Resolution};

let src = r#"
nature Bit { value: bool }
impl Bit { pub fn rise(self) -> bool { true } }
struct Bus { valid: Bit }
module Cell(q: Bit) { pub signal res: Bit; }
module Top(clk: Bit, bus: Bus) {
    let c = Cell(clk);
    signal r: Bit;
    r = c.res;
    let x = clk.rise();
    let y = bus.valid;
    let z = unknown().field;
}
"#;
let design = Design::builder().source("top.msl", src).build();
let top = design.module("Top").unwrap();
let member = |name: &str| match top.named(name).unwrap().value().unwrap().kind() {
    ExprKind::Field { res, .. } | ExprKind::MethodCall { res, .. } => *res,
    other => panic!("{other:?}"),
};
let cell = design.module("Cell").unwrap();
assert_eq!(member("x"), Resolution::Decl(design.nature("Bit").unwrap().impls()[0].items()[0].id()));
assert_eq!(member("y"), Resolution::Decl(design.structs()[0].fields()[0].decl().id()));
assert_eq!(member("z"), Resolution::Deferred(DeferReason::ReceiverTypeUnknown));
// `c.res` is a reference to the `pub signal res` of `Cell`.
assert_eq!(cell.signal("res").unwrap().references().len(), 1);
```

## Behavior blocks

A behavior block is bound to the module its target path names, by the module's file: the block and
its module share one file. The link is `Behavior::module()` (and the reverse, `Module::behaviors()`).
A block that cannot be bound stays in the tree with `module() == None`; the names inside it
are then resolved without the module's members, and a name that finds nothing there is not reported again.

| Code | When |
| --- | --- |
| `E0401` | the target names no module, names something that is not a module, or names a module declared in another file |
| `E0402` | the target is an `extern` module (it takes no behavior block); the block is still bound |
| `E0403` | the generic names of `analog<N, M>` are not those of the module |

## Diagnostics

Every diagnostic the POM reports, with the span it points at. Parse diagnostics (codes of the
parser, such as `E0900`) are copied into `Design::diagnostics()` first, with their file.

| Code | Stage | Meaning | Span |
| --- | --- | --- | --- |
| `E0130` | 6 | an `impl` block written for a module | the self type of the `impl` |
| `E0161` | 0, 3, 5 | a name that resolves to nothing, including a named connection, param binding, struct field or function argument the callee does not have; a private item reached through a path; an unreadable root file; a `mod b;` with no file, with two candidate files, or in a cycle | the path (or the `mod` item) |
| `E0330` | 2 | a name defined more than once in a module (the second is reported, with a label on the first) | the repeated name |
| `E0343` | 7 | a member of an instance that is missing or not `pub`, named from outside its module (§8.4.8) | the member name |
| `E0401` | 4 | a behavior block with no usable module | the target path |
| `E0402` | 4 | a behavior block for an `extern` module | the target path |
| `E0403` | 4 | the generic names of a behavior block differ from its module's | the head of the block |
| `E0457` | 2 | a namespace item named like a core primitive of §14 | the item's name |

```msl
module M() { signal s: Bit; signal s: Bit; }     // E0330
analog for Nowhere { }                           // E0401
```

```rust
use piperine_lang::pom::{Design, Reflect};

fn codes(src: &str) -> Vec<String> {
    let design = Design::builder().source("top.msl", src).build();
    design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
}

assert_eq!(codes("const K: f64 = missing;"), ["E0161"]);
assert_eq!(codes("mod a { fn hidden() {} }\nconst K: f64 = a::hidden;"), ["E0161"]);
assert_eq!(codes("nature Bit { value: bool }\nmodule M() { signal s: Bit; signal s: Bit; }"), ["E0330"]);
assert_eq!(codes("fn ddt() {}"), ["E0457"]);
assert_eq!(codes("module M() {}\nimpl M {}"), ["E0130"]);
assert_eq!(codes("analog for Nowhere {}"), ["E0401"]);
assert_eq!(codes("extern \"osdi\" module D() {}\nanalog for D {}"), ["E0402"]);
assert_eq!(codes("module M() {}\nanalog<N> for M {}"), ["E0403"]);

// A module and its block share a file; the diagnostic points at the target path.
let design = Design::builder().source("top.msl", "analog for Nowhere {}").build();
let span = design.diagnostics()[0].diagnostic.primary.0;
assert_eq!(&design.files()[0].text()[span.range()], "Nowhere");
```

### E0343 in practice

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module Cell(p: Bit) { signal hidden: Bit; pub signal shown: Bit; }
module Top(p: Bit) {
    let c = Cell(p);
    signal a: Bit;
    signal b: Bit;
    a = c.shown;
    b = c.hidden;
}
"#;
let design = Design::builder().source("top.msl", src).build();
let codes: Vec<_> = design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect();
assert_eq!(codes, ["E0343"]);
let span = design.diagnostics()[0].diagnostic.primary.0;
assert_eq!(&design.files()[0].text()[span.range()], "hidden");
// The member is still linked: the misuse is reported, the navigation works.
assert_eq!(design.module("Cell").unwrap().signal("hidden").unwrap().references().len(), 1);
```
