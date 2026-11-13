# Language construct to POM

This page answers: *I wrote this in MSL; where is it in the model, and how do I get to it?*
Every production of the grammar's additions ([Appendix B.3.2](../spec/appendix_b_grammar.md)) has a
row, and so does every Rust item MSL inherits. Read a row left to right: the **construct**, its
**source form**, the **storage node** it becomes (see [Model](model.md)), the **handle** you read
it through and the **query** that reaches it (see [Reflection](reflection.md)).

Everywhere below `design` is a `Design`, `m` a `Module`, and `Decl::children()` is the authored
tree one level at a time.

## Productions of the grammar's additions

| Construct | Source form | Storage node | Handle | Query |
| --- | --- | --- | --- | --- |
| `item` | one top-level item of a file | `DeclKind` under the file's `NamespaceData` | `Decl` | `design.root().children()` |
| `nature_decl` | `pub nature Electrical<..> { .. }` | `NatureData`, with `QuantityData` children | `Nature` | `design.nature("Electrical")` |
| `nature_field` | `potential: Voltage { access: v, .. }`, `value: T`, `resolve: f` | `NatureFieldNode` (`Quantity`, `Value`, `Resolve`) | `Quantity` | `Nature::quantities`, `value_type`, `resolve_fn` |
| `struct_field` | `pub in name: Type` in a struct | `FieldData` | `Field` | `Struct::fields`, `Field::dir` |
| `impl_block` | `impl<..> Trait for Type { .. }` | `ImplData` | `Impl` | `Nature::impls`, `Impl::trait_path`, `self_ty` |
| `impl_item` | `pub fn ..` or `pub const ..` inside an `impl` | `FnData`, `ConstData` (children of the impl) | `Function`, `Const` | `Impl::items` |
| `fn_decl` | `const? extern "C"? fn name<..>(..) -> T where c { .. }` | `FnData` | `Function` | `design.functions()`, `Function::params`, `ret`, `body` |
| `fn_param` | `self` or `name: Type` | `FnParamData` | `FnParam` | `Function::params`, `FnParam::is_self` |
| `call` | `f(a, name: b)` | `ExprKind::Call` with `CallClass::Function` | `Expr` | `Expr::kind` |
| `method_call` | `recv.m::<G>(a)` | `ExprKind::MethodCall` | `Expr` | `Expr::kind` (`res` holds the member) |
| `instance` | `Callee(a, b) { p: v }` | `ExprKind::Call` with `CallClass::Instance` | `Instance` | `Module::instances`, `Module::instance(name)` |
| `callee` | the called path, call or `(expr)` | `ExprKind::Call::callee` | `Instance` | `Instance::callee` |
| `template` | `Path { f: v }` naming a module | `ExprKind::Struct` with `StructClass::Template` | `Expr` | `Instance::via`, `Instance::via_template` |
| `module_decl` | `pub extern "abi"? module M<..>(ports) -> T where c { .. }` | `ModuleData` | `Module` | `design.module("M")` |
| `port` | `in a: Bit = false` | `PortData` | `Port` | `Module::ports`, `Module::port(name)` |
| `dir` | `in` / `out` | `Dir` | `Port`, `Field` | `Port::direction` |
| `sigtype` | `flip Bit`, `[Bit; 4]` | `TypeKind` (`Path`, `Array`, `Flip`) | `Type` | `Port::ty`, `Signal::ty`, `Signal::nature` |
| `module_item` | a param, signal, `let`, definition or generation in a body | `DeclKind` under the `ModuleData` | `Decl` | `Module::items` |
| `param` | `r: f64 = 1k where r > 0.0;` | `ParamData` | `Param` | `Module::params`, `Module::param(name)` |
| `signal_decl` | `signal s: Bit = false;` | `SignalData` | `Signal` | `Module::signals`, `Module::signal(name)` |
| `behavior_block` | `analog for M { .. }`, `digital for M { .. }` | `BehaviorData` | `Behavior` | `Module::behaviors`, `analog_blocks`, `digital_blocks` |
| `names` | the `<N, M>` of `analog<N, M> for ..` | `BehaviorData::names` | `Behavior` | `Behavior::names` |
| `digital_item` | a reaction or a generation in a `digital` block | `ReactionData`, `GenerationData` | `Reaction`, `Generation` | `Behavior::reactions`, `Behavior::generations` |
| `gen<X>` | `for pat in e { .. }`, `if c { .. } else ..` over items | `GenerationData` | `Generation` | `Module::generations`, `Generation::arms` |
| `reaction` | `on event { .. }` | `ReactionData` | `Reaction` | `Behavior::reactions` |
| `contribution` | `i(p, n) +> expr;` | `StmtKind::Contribution` | `Contribution` | `Behavior::contributions` |
| `commit` | `q <- v after d;` | `StmtKind::Commit` | `Commit` | `Reaction::commits` |
| `place` | `s`, `s[i]`, `s.f`, `(s)` as the target of `=` or `<-` | `ExprKind` (`Path`, `Index`, `Field`, `Paren`) | `Expr` | `Definition::place`, `Commit::place` |
| `emit` | `emit!("stream", f = e)` | `ExprKind::Emit` | `Expr` | `Expr::kind` |
| `include_str` | `include_str!("file")` | `ExprKind::IncludeStr` | `Expr` | `Expr::kind` |
| `attr_derivative` | `#[derivative(f, g)]` | `AttrNode` | `Attr` | `Reflect::attr("derivative")`, `Attr::paths` |
| `attr_derive` | `#[derive(Repr)]` | `AttrNode` | `Attr` | `Reflect::attr("derive")`, `Attr::paths` |
| `attr_link` | `#[link("x.osdi", model = "m")]` | `AttrNode` | `Attr` | `Reflect::attr("link")`, `Attr::positional`, `Attr::named` |

## Rust items MSL keeps

| Construct | Source form | Storage node | Handle | Query |
| --- | --- | --- | --- | --- |
| file | one source text | `SourceFileData` and its root `NamespaceData` | `SourceFile`, `Namespace` | `design.files()`, `SourceFile::to_ast` |
| `mod` | `mod a { .. }` or `mod a;` | `NamespaceData` with a `NamespaceOrigin` | `Namespace` | `design.namespaces()`, `Namespace::origin`, `source_file` |
| `use` | `use a::{b, c as d, e::*};` | `UseData` (the tree and its `Import` leaves) | `Use` | `Use::tree`, `Use::imports` |
| `const` | `const K: T = v;` | `ConstData` | `Const` | `Decl::as_const` |
| `struct` | `struct S<..> { f: T }` | `StructData`, `FieldData` children | `Struct` | `design.structs()`, `Struct::fields`, `is_bundle` |
| `enum` | `enum E { A = 1, B(T), C { f: T } }` | `EnumData`, `VariantData` children | `Enum` | `Decl::as_enum`, `Enum::variants` |
| `trait` | `trait T: Super { fn f(self); }` | `TraitData` | `Trait` | `Decl::as_trait`, `Trait::items` |
| generic parameter | `<T: Bound = D>`, `<const N: u32 = 4>` | `GenericParamData` | `GenericParam` | `Function::generics`, `Module::generics`, ... |
| body `let` | `let pat = expr;` in a module body | `LetData` with `BindingData` children | `Let`, `Binding` | `Module::lets`, `Module::named(name)` |
| block `let` | `let pat: T = e else { .. };` | `StmtKind::Let` with `BindingData` declarations | `Stmt`, `Binding` | `Stmt::kind` |
| expression | every other expression | `ExprKind` | `Expr` | `Expr::kind`, `Expr::text` |
| pattern | `(a, b)`, `S { x, .. }`, `a \| b` | `PatKind` | `Pat` | `Let::pattern` |
| type | `[T; N]`, `&T`, `(A, B)`, `flip T` | `TypeKind` | `Type` | `Port::ty`, `Field::ty` |
| path | `a::b<T>::c` | `PathNode` with `PathSegment`s | `Path` | `Path::segments` |
| doc comment | `/// text`, `//! text` | `Doc` in `Meta` | `Reflect` | `Reflect::docs` |
| attribute | `#[a(b)]` | `AttrNode` in `Meta` | `Attr` | `Reflect::attrs`, `Reflect::attr(name)` |
| malformed item or expression | any text the parser rejected | `DeclKind::Error`, `ExprKind::Error` | `Decl`, `Expr` | `Decl::kind` |

## Worked examples

Each pair is the same source twice: first as MSL, then read back through the POM.

### A nature

```msl
pub nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
```

```rust
use piperine_lang::pom::{Design, NatureKind, Reflect};

let src = r#"
pub nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
"#;
let design = Design::builder().source("top.msl", src).build();
let e = design.nature("Electrical").unwrap();
assert_eq!(e.kind(), NatureKind::Conservative);          // potential + flow
assert!(e.is_pub());
// Each quantity is its own declaration, a child of the nature, with its access name.
let names: Vec<_> = e.decl().children().iter().filter_map(|c| c.name()).collect();
assert_eq!(names, ["Voltage", "Current"]);
assert_eq!(e.potential().unwrap().access().unwrap().text(), "v");
```

### A module head: ports and parameters

```msl
module Vco(in vctl: Electrical, out clk: Bit, f0: f64 = 1M, kvco: f64 = 1M) {
    signal phase: Electrical;
}
```

```rust
use piperine_lang::pom::{Design, Dir, PortKind, Reflect};

let src = r#"
nature Electrical { potential: V { access: v, units: "V", abstol: 1u }, flow: I { access: i, units: "A", abstol: 1p } }
nature Bit { value: bool }
module Vco(in vctl: Electrical, out clk: Bit, f0: f64 = 1M, kvco: f64 = 1M) {
    signal phase: Electrical;
}
"#;
let design = Design::builder().source("top.msl", src).build();
let vco = design.module("Vco").unwrap();
// The three first are ports (carriers); `f0` and `kvco` are value ports: parameters.
assert_eq!(vco.ports().len(), 4);
assert_eq!(vco.port("vctl").unwrap().direction(), Some(Dir::In));
assert_eq!(vco.port("vctl").unwrap().kind(), PortKind::Carrier);
assert_eq!(vco.port("f0").unwrap().kind(), PortKind::ValueInput);
assert_eq!(vco.port("f0").unwrap().default().unwrap().text(), "1M");
assert_eq!(vco.signal("phase").unwrap().nature(), design.nature("Electrical"));
```

### Parameters and signals inside the body

```msl
module Osc() {
    gain: f64 = 2.0 where gain > 0.0;
    signal s: Bit = false;
    signal bus: [Bit; 4];
}
```

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module Osc() {
    gain: f64 = 2.0 where gain > 0.0;
    signal s: Bit = false;
    signal bus: [Bit; 4];
}
"#;
let design = Design::builder().source("top.msl", src).build();
let osc = design.module("Osc").unwrap();
let gain = osc.param("gain").unwrap();
assert_eq!(gain.default().unwrap().text(), "2.0");
assert_eq!(gain.where_clause().unwrap().text(), "gain > 0.0");
assert_eq!(osc.signal("s").unwrap().init().unwrap().text(), "false");
assert_eq!(osc.signal("bus").unwrap().ty().text(), "[Bit; 4]");
// Both signals are in module `Osc`, nowhere else.
assert!(osc.signals().iter().all(|s| s.declared_in().unwrap() == osc.decl()));
```

### A definition, a `let` and an instance

```msl
module Top(a: Bit, y: Bit) {
    signal n: Bit;
    n = !a after 1n;
    let inv = Inverter(n, y) { tpd: 2n };
}
```

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module Inverter(a: Bit, y: Bit) { tpd: f64 = 1n; }
module Top(a: Bit, y: Bit) {
    signal n: Bit;
    n = !a after 1n;
    let inv = Inverter(n, y) { tpd: 2n };
}
"#;
let design = Design::builder().source("top.msl", src).build();
let top = design.module("Top").unwrap();

// `n = ..` is a definition: place, value, delay.
let def = top.definitions()[0];
assert_eq!((def.place().text(), def.value().text(), def.after().unwrap().text()), ("n", "!a", "1n"));

// `let inv = ..` is a `let`, the name `inv` is a binding, and its value is an instance.
assert_eq!(top.lets().len(), 1);
let inv = top.named("inv").unwrap().instance().unwrap();
assert_eq!(inv.module(), design.module("Inverter"));
assert_eq!(inv.param_bindings()[0].name.text, "tpd");
assert_eq!(inv.connection(design.module("Inverter").unwrap().port("a").unwrap()).unwrap().text(), "n");
```

### Generation is not unrolled

```msl
module Chain(a: Bit) {
    for k in 0..1000 { signal s: Bit; let u = Cell(s); }
}
```

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module Cell(p: Bit) {}
module Chain(a: Bit) {
    for k in 0..1000 { signal s: Bit; let u = Cell(s); }
}
"#;
let design = Design::builder().source("top.msl", src).build();
let chain = design.module("Chain").unwrap();
assert_eq!(chain.generations().len(), 1);
assert_eq!(chain.signals().len(), 1);          // one declaration, not a thousand
assert_eq!(chain.instances().len(), 1);        // one placement
let generation = chain.generations()[0];
assert_eq!(generation.iter().unwrap().text(), "0..1000");
assert_eq!(chain.signal("s").unwrap().generation(), Some(generation));
```

### Analog behavior

```msl
module Res(p: Electrical, n: Electrical) { r: f64 = 1k; }
analog for Res { i(p, n) +> v(p, n) / r; }
```

```rust
use piperine_lang::pom::{BehaviorKind, Design, Reflect};

let src = r#"
nature Electrical { potential: V { access: v, units: "V", abstol: 1u }, flow: I { access: i, units: "A", abstol: 1p } }
module Res(p: Electrical, n: Electrical) { r: f64 = 1k; }
analog for Res { i(p, n) +> v(p, n) / r; }
"#;
let design = Design::builder().source("top.msl", src).build();
let res = design.module("Res").unwrap();
let block = &res.analog_blocks()[0];
assert_eq!(block.kind(), BehaviorKind::Analog);
assert_eq!(block.module(), Some(res));            // the block is linked to its module
let c = block.contributions()[0];
assert_eq!(c.lhs().text(), "i(p, n)");
assert_eq!(c.rhs().text(), "v(p, n) / r");
```

### Digital behavior

```msl
module Reg(in clk: Bit, in d: Bit, out q: Bit) {}
digital for Reg { on clk.rise() { q <- d after 1n; } }
```

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module Reg(in clk: Bit, in d: Bit, out q: Bit) {}
digital for Reg { on clk.rise() { q <- d after 1n; } }
"#;
let design = Design::builder().source("top.msl", src).build();
let reg = design.module("Reg").unwrap();
let reaction = reg.digital_blocks()[0].reactions()[0];
assert_eq!(reaction.event().text(), "clk.rise()");
let commit = reaction.commits()[0];
assert_eq!(commit.place().text(), "q");
assert_eq!(commit.value().text(), "d");
assert_eq!(commit.after().unwrap().text(), "1n");
// The reverse view: this commit is a writer of the port `q`.
assert_eq!(reg.port("q").unwrap().writers().len(), 1);
```

### A bundle

```msl
struct Stream { valid: Bit, in ready: Bit, data: [Bit; 8] }
```

```rust
use piperine_lang::pom::{Design, Dir, Reflect};

let src = r#"
nature Bit { value: bool }
struct Stream { valid: Bit, in ready: Bit, data: [Bit; 8] }
struct Plain { x: f64 }
"#;
let design = Design::builder().source("top.msl", src).build();
let stream = design.structs()[0];
assert!(stream.is_bundle());                       // it has carrier fields
assert_eq!(stream.fields()[1].dir(), Some(Dir::In));
assert!(!design.structs()[1].is_bundle());         // only values
```

### Modules and `use`

```msl
mod analog_lib {
    pub module Cell(p: Electrical) {}
}
use analog_lib::Cell;
```

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Electrical { potential: V { access: v, units: "V", abstol: 1u }, flow: I { access: i, units: "A", abstol: 1p } }
mod analog_lib {
    pub module Cell(p: Electrical) {}
}
"#;
let design = Design::builder().source("top.msl", src).build();
// The hierarchy is the one written: `Cell` is inside `analog_lib`.
let cell = design.module("Cell").unwrap();
assert_eq!(cell.qualified_name(), "analog_lib::Cell");
assert_eq!(cell.declared_in().unwrap().name(), Some("analog_lib"));
```

### Attributes and docs

```msl
/// A transistor model.
#[link("bsim4.osdi", model = "bsim4")]
extern "osdi" module Nmos(d: Electrical, g: Electrical, s: Electrical, b: Electrical) {}
```

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
/// A transistor model.
#[link("bsim4.osdi", model = "bsim4")]
extern "osdi" module Nmos() {}
"#;
let design = Design::builder().source("top.msl", src).build();
let nmos = design.module("Nmos").unwrap();
assert_eq!(nmos.docs()[0].text.trim(), "A transistor model.");
let link = nmos.attr("link").unwrap();
assert_eq!(link.positional().len(), 1);
assert!(link.named("model").is_some());
assert_eq!(nmos.abi(), Some("osdi"));
```
