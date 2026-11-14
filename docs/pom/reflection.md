# Reflection

The reflection face of the POM: `Copy` handles that borrow a `Design`, and the methods that
answer questions without touching an arena or an id. This page lists every handle and every
method with its result, and gives a runnable example for each group. The storage the handles
read is in [Model](model.md).

## Conventions

* **Handles borrow the design.** A handle is a pair of `&Design` and one id: `Copy`, cheap, and
  tied to the design's lifetime (`'d` in the signatures below). It never owns data.
* **Equality** holds exactly when two handles come from the same `Design` (by address) and name
  the same node. Handles of two builds of the same text are not equal.
* **Order.** Every list is in source order. A list that says *deep* (`Module::signals`,
  `lets`, `definitions`) includes items written inside generation; `items()` and
  `generations()` give the body as written, with a generation held as one item.
* **Options.** A method returns `Option` when the construct may be absent from the source (a
  default, a `where` clause) or when a link may be unresolved (the module of an instance).
  Methods never panic on any design, including a design of broken source.
* **Names.** `name()` is the declared identifier; it is `None` for an `impl`, a `use`, a
  behavior block, an error item and a file's root namespace.
* **Spans and text.** `span()` is a byte range of the declaring file's text; `text()` is that
  range. An item's span includes its attributes and doc comments.
* **Casts.** `Decl::as_*` return a typed handle exactly when the kind matches. The typed handles
  are separate types: ask one for `decl()` to get the generic `Decl` back.

### `Design`

The entry point. Lookups, whole-design listings, files, diagnostics and the raw arenas. `functions()`, `structs()` and `namespaces()` list the non-prelude files only; `modules()`, `natures()` and `decls()` include prelude sources.

| Method | Returns | Meaning |
| --- | --- | --- |
| `check(&self)` | `Vec<String>` | The internal inconsistencies of the arenas, one message each; empty when the design is well formed. |
| `builder()` | `DesignBuilder` | A builder with no sources. |
| `load(root: impl Into<PathBuf>)` | `Design` | Read the tree rooted at `root` from disk: `mod b;` follows Rust's layout. An unreadable root gives an empty root and one `E0161`. |
| `diagnostics(&self)` | `&[DesignDiagnostic]` | Every diagnostic of the build, every diagnostic sorted by file, offset and code. |
| `expr_node(&self, id: ExprId)` | `&ExprNode` | The expression with this id. Ids come from this design. |
| `stmt_node(&self, id: StmtId)` | `&StmtNode` | The statement node with this id. |
| `pat_node(&self, id: PatId)` | `&PatNode` | The pattern node with this id. |
| `type_node(&self, id: TypeId)` | `&TypeNode` | The type node with this id. |
| `path_node(&self, id: PathId)` | `&PathNode` | The path node with this id. |
| `expr_count(&self)` | `usize` | The number of expression nodes. |
| `stmt_count(&self)` | `usize` | The number of statement nodes. |
| `pat_count(&self)` | `usize` | The number of pattern nodes. |
| `type_count(&self)` | `usize` | The number of type nodes. |
| `path_count(&self)` | `usize` | The number of path nodes. |
| `decl_data(&self, id: DeclId)` | `&DeclData` | The declaration with this id. Ids come from this design. |
| `decl_count(&self)` | `usize` | The number of declarations. |
| `dump(&self)` | `String` | One line per declaration, depth first, roots in file order: indent (two spaces per level), kind, name (`-` when none), `start..end`. |
| `files(&self)` | `Vec<SourceFile<'_>>` | Every file, prelude sources first, then the root, then loaded files. |
| `expr(&self, id: ExprId)` | `Expr<'_>` | A handle on the expression with this id. |
| `stmt(&self, id: StmtId)` | `Stmt<'_>` | A handle on the statement with this id. |
| `pat(&self, id: PatId)` | `Pat<'_>` | A handle on the pattern with this id. |
| `ty(&self, id: TypeId)` | `Type<'_>` | A handle on the type with this id. |
| `path(&self, id: PathId)` | `Path<'_>` | A handle on the path with this id. |
| `root(&self)` | `Decl<'_>` | The root file's namespace. |
| `decl(&self, id: DeclId)` | `Decl<'_>` | The declaration with this id, as a handle: the way from a `Resolution::Decl(id)` to the declaration. Ids come from this design. |
| `decls(&self)` | `Vec<Decl<'_>>` | Every declaration of the design in arena (source) order, prelude files first. |
| `modules(&self)` | `Vec<Module<'_>>` | Every module of the design, prelude files first, in source order. |
| `module(&self, name: &str)` | `Option<Module<'_>>` | The first module named `name`: the first match in a non-prelude file in file order, else the first match in a prelude source. |
| `functions(&self)` | `Vec<Function<'_>>` | Every free function (a `fn` directly in a namespace) of the non-prelude files, in source order. |
| `structs(&self)` | `Vec<Struct<'_>>` | Every struct of the non-prelude files, in source order. |
| `namespaces(&self)` | `Vec<Namespace<'_>>` | Every named namespace (`mod a { .. }` or `mod a;`) of the non-prelude files; a file's unnamed root is not listed. |
| `natures(&self)` | `Vec<Nature<'_>>` | Every nature of the design, in source order. |
| `nature(&self, name: &str)` | `Option<Nature<'_>>` | The first nature named `name`. |
| `find(&self, path: &str)` | `Option<Decl<'_>>` | The declaration at a `::` path from the root, each segment a child name: `a::M` is `M` inside namespace `a`; or a name `qualified_name` printed (`a::E.A`, `impl S.X`), the first declaration with exactly it. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
mod a {
    pub nature Bit { value: bool }
    pub struct P { x: f64 }
    pub fn helper() -> f64 { 1.0 }
    module M(x: Bit) { signal w: Bit; }
}
module Top() {}
"#;
let design = Design::builder().source("top.msl", src).build();
assert!(design.diagnostics().is_empty());

// Lookups by name or by `::` path from the root.
assert_eq!(design.module("M").unwrap().qualified_name(), "a::M");
assert_eq!(design.find("a::helper").unwrap().name(), Some("helper"));
assert!(design.find("a::nope").is_none());
assert_eq!(design.nature("Bit").unwrap().qualified_name(), "a::Bit");

// Whole-design listings, in source order.
assert_eq!(design.modules().len(), 2);
assert_eq!(design.functions().len(), 1);
assert_eq!(design.structs().len(), 1);
assert_eq!(design.namespaces().len(), 1);
assert_eq!(design.files().len(), 1);

// A design checks itself and prints its declaration tree.
assert!(design.check().is_empty());
assert!(design.dump().starts_with("Namespace"));
```

A design is a plain value: it is `Clone`, comparable with `==`, and serializable. The retained
syntax tree is rebuilt from the text when a design is read back, so it is not part of the JSON.

```rust
use piperine_lang::pom::Design;

let design = Design::builder().source("top.msl", "module Top() {}").build();
let json = serde_json::to_string(&design).unwrap();
assert!(!json.contains("\"ast\""));
let back: Design = serde_json::from_str(&json).unwrap();
assert!(back == design);
assert_eq!(back.files()[0].ast(), design.files()[0].ast());
```

### `DesignBuilder`

Collects sources and builds. `build()` never fails.

| Method | Returns | Meaning |
| --- | --- | --- |
| `prelude(self, name: impl Into<String>, text: impl Into<String>)` | `Self` | A prelude source: its `pub` items are visible in every file. |
| `source(self, path: impl Into<String>, text: impl Into<String>)` | `Self` | A virtual source file. The first one is the root; the others are found by the `mod` items of the files that name them. |
| `root_file(self, path: impl Into<PathBuf>)` | `Self` | The root file, read from disk with the files its `mod` items name. A root file replaces the virtual sources. |
| `build(self)` | `Design` | Parse every file and assemble the design. Files are numbered prelude sources first, then the root. A parse error never stops the build: the recovered tree is kept and each diagnostic is listed with its file. |

Sources are virtual files; the first is the root and the others are found by the `mod` items
that name them (`mod b;` loads `b.msl`, or `b/mod.msl`, beside the declaring file). A prelude
source is parsed first and its `pub` items are visible everywhere.

```rust
use piperine_lang::pom::{Design, Reflect};

let design = Design::builder()
    .prelude("std.msl", "pub nature Bit { value: bool }")
    .source("top.msl", "mod b;\nmodule Top(p: Bit) {}")
    .source("b.msl", "pub module Inner() {}")
    .build();
assert!(design.diagnostics().is_empty());

// Files: prelude sources first, then the root, then loaded files.
let names: Vec<_> = design.files().iter().map(|f| f.name().to_string()).collect();
assert_eq!(names, ["std.msl", "top.msl", "b.msl"]);
assert!(design.files()[0].is_prelude());

// `mod b;` is a namespace whose source file was loaded.
let b = design.find("b").unwrap().as_namespace().unwrap();
assert_eq!(b.source_file().unwrap().name(), "b.msl");
assert_eq!(design.module("Inner").unwrap().qualified_name(), "b::Inner");
assert_eq!(design.module("Top").unwrap().port("p").unwrap().nature(), design.nature("Bit"));
```

A tree on disk is read with `Design::load(path)`; an unreadable root still returns a design (an
empty root and one `E0161`).

```rust
use piperine_lang::pom::Design;

let design = Design::load("/definitely/not/here/top.msl");
assert_eq!(design.diagnostics().len(), 1);
assert_eq!(design.diagnostics()[0].diagnostic.code, "E0161");
assert!(design.modules().is_empty());
```

### `SourceFile`

One source file: its text, the syntax tree the parser returned, and the tree rebuilt from the POM.

| Method | Returns | Meaning |
| --- | --- | --- |
| `to_ast(self)` | `ast::File` | The syntax tree of this file, rebuilt from the declaration tree and the body arenas, spans included. |
| `id(self)` | `FileId` | The id of the file. |
| `name(self)` | `&'d str` | The name it was registered under, or its file name on disk. |
| `path(self)` | `Option<&'d FsPath>` | The path it was read from, virtual or on disk. |
| `text(self)` | `&'d str` | The text of the file, verbatim. |
| `ast(self)` | `&'d ast::File` | The syntax tree the parser returned, errors recovered. |
| `is_prelude(self)` | `bool` | Whether the file is a prelude source. |

```rust
use piperine_lang::pom::Design;

let src = "//! file docs\nmodule Top() {}\n";
let design = Design::builder().source("top.msl", src).build();
let file = &design.files()[0];
assert_eq!(file.name(), "top.msl");
assert_eq!(file.text(), src);
assert!(!file.is_prelude());

// The POM is lossless: the syntax tree rebuilt from it equals the one the parser returned.
assert_eq!(file.to_ast(), *file.ast());
```

### `Reflect`

The trait every declaration handle implements. Only `decl()` is required; everything else is provided.

| Method | Returns | Meaning |
| --- | --- | --- |
| `decl(self)` | `Decl<'d>` | The generic handle; the one method an implementor provides. |
| `name(self)` | `Option<&'d str>` | The declared name; none for an `impl`, a `use`, a behavior block, an error item or a file's root namespace. |
| `span(self)` | `Span` | The range of the whole declaration (an item's span includes its attributes and docs). |
| `file(self)` | `SourceFile<'d>` | The file the declaration is written in. |
| `text(self)` | `&'d str` | The authored text of the declaration. |
| `docs(self)` | `&'d [Doc]` | The doc comments in order, one per comment. On a file's root namespace these are the file's `//!` comments. |
| `attrs(self)` | `Vec<Attr<'d>>` | The attributes in order. |
| `attr(self, name: &str)` | `Option<Attr<'d>>` | The first attribute whose path is written `name`. |
| `is_pub(self)` | `bool` | Whether the item is written `pub`. |
| `module(self)` | `Option<Module<'d>>` | The module this declaration belongs to: itself for a module, else the nearest enclosing module (through generation). None outside any module, and for a `signal` written outside a module body (the parser reported it). |
| `declared_in(self)` | `Option<Decl<'d>>` | The declaration that lexically contains this one; none for a file's root namespace. |
| `references(self)` | `Vec<Reference<'d>>` | Every path with a segment, every member expression and every identifier pattern that resolved to this declaration, in source order. |
| `qualified_name(self)` | `String` | The namespace path joined by `::`, then the member chain joined by `.`: signal `w` of module `M` in `mod a` is `a::M.w`. An `impl` adds its header (`impl S`, `impl Tr for S`) and a behavior block its own (`analog for M`), so their items do not share a name with another impl's or the module's: `impl Tr for S.X`, `analog for M.z`. A generation, a `use` and a file's root namespace add no segment. |

Every declaration handle (`Module`, `Port`, `Signal`, ..., and the generic `Decl`) implements
`Reflect`, so these methods are available on all of them.

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
mod a {
    /// A cell.
    #[derive(Repr)]
    pub module Cell(p: Bit) {
        pub signal q: Bit;
    }
}
"#;
let design = Design::builder().source("top.msl", src).build();
let cell = design.module("Cell").unwrap();
let q = cell.signal("q").unwrap();

assert_eq!(cell.name(), Some("Cell"));
assert!(cell.is_pub() && q.is_pub());
assert_eq!(cell.docs()[0].text.trim(), "A cell.");
assert_eq!(cell.attr("derive").unwrap().paths()[0].text(), "Repr");

// Where a declaration lives, and its qualified name.
assert_eq!(q.declared_in().unwrap().name(), Some("Cell"));
assert_eq!(q.module(), Some(cell));
assert_eq!(q.qualified_name(), "a::Cell.q");
assert_eq!(q.text(), "pub signal q: Bit;");
assert_eq!(q.file().name(), "top.msl");
```

### `Decl`

The generic declaration handle, over any kind. The `as_*` casts return the typed handle exactly when the kind matches.

| Method | Returns | Meaning |
| --- | --- | --- |
| `as_nature(self)` | `Option<Nature<'d>>` | The declaration as a `Nature` handle when its kind matches; none otherwise. |
| `as_quantity(self)` | `Option<Quantity<'d>>` | The declaration as a `Quantity` handle when its kind matches; none otherwise. |
| `as_module(self)` | `Option<Module<'d>>` | The declaration as a `Module` handle when its kind matches; none otherwise. |
| `as_port(self)` | `Option<Port<'d>>` | The declaration as a `Port` handle when its kind matches; none otherwise. |
| `as_param(self)` | `Option<Param<'d>>` | The declaration as a `Param` handle when its kind matches; none otherwise. |
| `as_signal(self)` | `Option<Signal<'d>>` | The declaration as a `Signal` handle when its kind matches; none otherwise. |
| `as_let(self)` | `Option<Let<'d>>` | The declaration as a `Let` handle when its kind matches; none otherwise. |
| `as_binding(self)` | `Option<Binding<'d>>` | The declaration as a `Binding` handle when its kind matches; none otherwise. |
| `as_definition(self)` | `Option<Definition<'d>>` | The declaration as a `Definition` handle when its kind matches; none otherwise. |
| `as_generation(self)` | `Option<Generation<'d>>` | The declaration as a `Generation` handle when its kind matches; none otherwise. |
| `as_behavior(self)` | `Option<Behavior<'d>>` | The declaration as a `Behavior` handle when its kind matches; none otherwise. |
| `as_reaction(self)` | `Option<Reaction<'d>>` | The declaration as a `Reaction` handle when its kind matches; none otherwise. |
| `as_const(self)` | `Option<Const<'d>>` | The declaration as a `Const` handle when its kind matches; none otherwise. |
| `as_function(self)` | `Option<Function<'d>>` | The declaration as a `Function` handle when its kind matches; none otherwise. |
| `as_fn_param(self)` | `Option<FnParam<'d>>` | The declaration as a `FnParam` handle when its kind matches; none otherwise. |
| `as_generic_param(self)` | `Option<GenericParam<'d>>` | The declaration as a `GenericParam` handle when its kind matches; none otherwise. |
| `as_struct(self)` | `Option<Struct<'d>>` | The declaration as a `Struct` handle when its kind matches; none otherwise. |
| `as_field(self)` | `Option<Field<'d>>` | The declaration as a `Field` handle when its kind matches; none otherwise. |
| `as_enum(self)` | `Option<Enum<'d>>` | The declaration as a `Enum` handle when its kind matches; none otherwise. |
| `as_variant(self)` | `Option<Variant<'d>>` | The declaration as a `Variant` handle when its kind matches; none otherwise. |
| `as_trait(self)` | `Option<Trait<'d>>` | The declaration as a `Trait` handle when its kind matches; none otherwise. |
| `as_impl(self)` | `Option<Impl<'d>>` | The declaration as a `Impl` handle when its kind matches; none otherwise. |
| `as_use(self)` | `Option<Use<'d>>` | The declaration as a `Use` handle when its kind matches; none otherwise. |
| `as_namespace(self)` | `Option<Namespace<'d>>` | The declaration as a `Namespace` handle when its kind matches; none otherwise. |
| `id(self)` | `DeclId` | The id of the node in its arena. |
| `design(self)` | `&'d Design` | The design the handle borrows. |
| `data(self)` | `&'d DeclData` | The raw storage of this declaration. |
| `kind(self)` | `&'d DeclKind` | The kind and payload of the declaration. |
| `children(self)` | `Vec<Decl<'d>>` | The declarations lexically inside this one, at the first level, in source order. |

`Decl` is the generic handle. Every typed handle is one `as_*` away, and the cast succeeds
only when the kind matches. `children()` is the authored tree, one level.

```rust
use piperine_lang::pom::{Design, Reflect};

let design = Design::builder()
    .source("top.msl", "nature Bit { value: bool }\nmodule Top(p: Bit) { signal s: Bit; }")
    .build();
let top = design.find("Top").unwrap();
assert!(top.as_module().is_some());
assert!(top.as_signal().is_none());
let kinds: Vec<_> = top.children().iter().map(|c| c.kind().label()).collect();
assert_eq!(kinds, ["Port", "Signal"]);

// The root is the file's namespace; `id()` is stable for the life of the design.
assert!(design.root().as_namespace().is_some());
assert_eq!(top.design().decl_data(top.id()).name.as_ref().unwrap().text, "Top");
```

### `Attr`

One attribute: `#[path]`, `#[path(args)]` or `#[path = expr]`.

| Method | Returns | Meaning |
| --- | --- | --- |
| `node(self)` | `&'d AttrNode` | The raw attribute node. |
| `name(self)` | `String` | The path as written, segments joined by `::`. |
| `path(self)` | `Path<'d>` | The attribute's path node; its resolution stays `Unresolved`. |
| `span(self)` | `Span` | The byte range of the attribute. |
| `text(self)` | `&'d str` | The authored text, `#[...]` included. |
| `input(self)` | `&'d AttrInputNode` | The raw arguments of the attribute. |
| `value(self)` | `Option<Expr<'d>>` | The `= expr` value of `#[path = expr]`. |
| `positional(self)` | `Vec<&'d Lit>` | The literal arguments written without a name, in order. |
| `paths(self)` | `Vec<Path<'d>>` | The path arguments, as in `#[derive(Repr, Clone)]`, in order. |
| `named(self, name: &str)` | `Option<&'d Lit>` | The value of the argument `name = lit`. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
#[link("x.osdi", model = "bsim4")]
#[derive(Repr, Clone)]
#[doc = "hello"]
extern "osdi" module Dev() {}
"#;
let design = Design::builder().source("top.msl", src).build();
let dev = design.module("Dev").unwrap();
let attrs = dev.attrs();
assert_eq!(attrs.len(), 3);

let link = dev.attr("link").unwrap();
assert_eq!(link.name(), "link");
assert_eq!(link.positional().len(), 1);          // "x.osdi"
assert!(link.named("model").is_some());          // model = "bsim4"
assert_eq!(link.text(), "#[link(\"x.osdi\", model = \"bsim4\")]");

let paths: Vec<_> = dev.attr("derive").unwrap().paths().iter().map(|p| p.text()).collect();
assert_eq!(paths, ["Repr", "Clone"]);
assert_eq!(dev.attr("doc").unwrap().value().unwrap().text(), "\"hello\"");
assert!(dev.is_extern());
assert_eq!(dev.abi(), Some("osdi"));
```

### `Namespace`

A namespace: a file's top, `mod a { }` or `mod a;`.

| Method | Returns | Meaning |
| --- | --- | --- |
| `origin(self)` | `&'d NamespaceOrigin` | Whether it is a file root, an inline `mod`, or an external `mod a;` and the file loaded. |
| `items(self)` | `Vec<Decl<'d>>` | Every declaration directly inside, in source order. |
| `submodules(self)` | `Vec<Namespace<'d>>` | The namespaces directly inside, in source order. |
| `source_file(self)` | `Option<SourceFile<'d>>` | The file this namespace is the top of: a file's root namespace, or the file `mod a;` loaded. None for an inline module and for a `mod a;` whose file was not loaded. |

```rust
use piperine_lang::pom::{Design, NamespaceOrigin, Reflect};

let src = r#"
mod a { pub fn f() -> f64 { 1.0 } mod b {} }
use a::f;
"#;
let design = Design::builder().source("top.msl", src).build();
let a = design.find("a").unwrap().as_namespace().unwrap();
assert_eq!(*a.origin(), NamespaceOrigin::Inline);
assert_eq!(a.items().len(), 2);
assert_eq!(a.submodules().len(), 1);
assert!(a.source_file().is_none());               // an inline module has no file of its own

let root = design.root().as_namespace().unwrap();
assert_eq!(*root.origin(), NamespaceOrigin::Root);
assert!(root.source_file().is_some());
```

### `Use`

`use a::{b, c as d};`

| Method | Returns | Meaning |
| --- | --- | --- |
| `tree(self)` | `&'d UseTreeNode` | The tree as written. |
| `imports(self)` | `Vec<ImportRef<'d>>` | One entry per leaf, in source order. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = "mod a { pub fn f() {} pub fn g() {} }\nuse a::{f, g as h};\n";
let design = Design::builder().source("top.msl", src).build();
let u = design.root().children().into_iter().find_map(|d| d.as_use()).unwrap();
let imports = u.imports();
assert_eq!(imports.len(), 2);
assert_eq!(imports[0].path.segments().last().unwrap().name.text, "f");
assert_eq!(imports[1].alias, Some("h"));
assert!(!imports[0].glob);
```

### `Const`

`const NAME: ty = value;`

| Method | Returns | Meaning |
| --- | --- | --- |
| `ty(self)` | `Type<'d>` | The declared type. |
| `value(self)` | `Option<Expr<'d>>` | The value expression; none for a trait's constant declaration. |

### `Function`

`fn name<..>(params) -> ret where cond { body }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `is_const(self)` | `bool` | Whether the function is `const`. |
| `abi(self)` | `Option<&'d str>` | The ABI string of `extern "C" fn`, without quotes. |
| `generics(self)` | `Vec<GenericParam<'d>>` | The generic parameters of the function. |
| `params(self)` | `Vec<FnParam<'d>>` | The parameters in source order; a method's `self` comes first. |
| `ret(self)` | `Option<Type<'d>>` | The `-> ty` return type. |
| `where_clause(self)` | `Option<Expr<'d>>` | The `where` condition. |
| `body(self)` | `Option<&'d Block>` | The body block; none for a declaration ending in `;`. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
const fn twice<T>(x: f64, y: f64) -> f64 where x > 0.0 { x + y }
fn declared(self);
"#;
let design = Design::builder().source("top.msl", src).build();
let f = design.functions()[0];
assert_eq!(f.name(), Some("twice"));
assert!(f.is_const());
assert_eq!(f.generics().len(), 1);
assert_eq!(f.params().len(), 2);
assert_eq!(f.params()[0].name(), Some("x"));
assert_eq!(f.ret().unwrap().text(), "f64");
assert!(f.where_clause().is_some());
assert!(f.body().is_some());
// A declaration ending in `;` has no body, and `self` is a parameter.
let g = design.functions()[1];
assert!(g.body().is_none());
assert!(g.params()[0].is_self());
```

### `FnParam`

One parameter of a function.

| Method | Returns | Meaning |
| --- | --- | --- |
| `is_self(self)` | `bool` | Whether the parameter is `self`. |
| `ty(self)` | `Option<Type<'d>>` | The declared type; none for `self` and for a malformed parameter. |

### `GenericParam`

`<T: Bound = Default>` or `<const N: u32 = 4>`.

| Method | Returns | Meaning |
| --- | --- | --- |
| `is_const(self)` | `bool` | Whether it is `const`. |
| `bounds(self)` | `Vec<Path<'d>>` | The bounds of a type parameter, in source order. |
| `const_type(self)` | `Option<Type<'d>>` | The type of a const parameter. |
| `default_type(self)` | `Option<Type<'d>>` | The default of a type parameter. |
| `default_value(self)` | `Option<Expr<'d>>` | The default of a const parameter. |

### `Struct`

`struct S<..> { fields }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `form(self)` | `FieldsForm` | Named, tuple or unit, as written. |
| `bundle(self)` | `Option<bool>` | Whether the struct is a bundle; none until classification. |
| `is_bundle(self)` | `bool` | Whether a field is a carrier, an array of carriers or a bundle. |
| `generics(self)` | `Vec<GenericParam<'d>>` | The generic parameters of the struct. |
| `fields(self)` | `Vec<Field<'d>>` | The fields in source order. |

A struct whose field is a carrier (a nature, an array of one, or another bundle) is a *bundle*;
the classification is written into `bundle()` after name resolution.

```rust
use piperine_lang::pom::{Design, FieldsForm, Reflect};

let src = r#"
nature Bit { value: bool }
struct Point { x: f64, y: f64 }
struct Bus { valid: Bit, data: [Bit; 4] }
struct Pair(f64, f64);
"#;
let design = Design::builder().source("top.msl", src).build();
let structs = design.structs();
let (point, bus, pair) = (structs[0], structs[1], structs[2]);
assert_eq!(point.form(), FieldsForm::Named);
assert_eq!(pair.form(), FieldsForm::Tuple);
assert_eq!(point.fields().len(), 2);
assert_eq!(point.bundle(), Some(false));
assert!(bus.is_bundle());
assert_eq!(bus.fields()[1].ty().text(), "[Bit; 4]");
```

### `Field`

A field of a struct or of a variant.

| Method | Returns | Meaning |
| --- | --- | --- |
| `dir(self)` | `Option<Dir>` | The direction word written before the name. |
| `ty(self)` | `Type<'d>` | The declared type. |

### `Enum`

`enum E<..> { variants }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `generics(self)` | `Vec<GenericParam<'d>>` | The generic parameters of the enum. |
| `variants(self)` | `Vec<Variant<'d>>` | The variants in source order. |

```rust
use piperine_lang::pom::{Design, FieldsForm, Reflect};

let src = "enum Logic { L = 0, H = 1, Pair(f64, f64), Named { a: f64 } }";
let design = Design::builder().source("top.msl", src).build();
let e = design.root().children()[0].as_enum().unwrap();
let variants = e.variants();
assert_eq!(variants.len(), 4);
assert_eq!(variants[0].discriminant().unwrap().text(), "0");
assert_eq!(variants[2].form(), FieldsForm::Tuple);
assert_eq!(variants[3].fields()[0].name(), Some("a"));
```

### `Variant`

A variant of an enum.

| Method | Returns | Meaning |
| --- | --- | --- |
| `form(self)` | `FieldsForm` | How the variant lists its fields. |
| `fields(self)` | `Vec<Field<'d>>` | The fields of the variant, in source order. |
| `discriminant(self)` | `Option<Expr<'d>>` | The `= value` after the variant. |

### `Trait`

`trait Name<..>: Super { items }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `generics(self)` | `Vec<GenericParam<'d>>` | The generic parameters of the trait. |
| `supertraits(self)` | `Vec<Path<'d>>` | The `: Super` bounds, as paths. |
| `items(self)` | `Vec<Decl<'d>>` | The functions and constants, in source order. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
trait Shape: Clone { fn area(self) -> f64; const SIDES: u32; }
struct Sq { s: f64 }
impl Shape for Sq { fn area(self) -> f64 { 1.0 } const SIDES: u32 = 4; }
"#;
let design = Design::builder().source("top.msl", src).build();
let kids = design.root().children();
let tr = kids[0].as_trait().unwrap();
assert_eq!(tr.supertraits().len(), 1);
assert_eq!(tr.items().len(), 2);
let im = kids[2].as_impl().unwrap();
assert_eq!(im.trait_path().unwrap().text(), "Shape");
assert_eq!(im.self_ty().text(), "Sq");
assert_eq!(im.items().len(), 2);
assert!(kids[2].as_trait().is_none());
```

### `Impl`

`impl<..> Trait for Type { items }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `generics(self)` | `Vec<GenericParam<'d>>` | The generic parameters of the impl. |
| `trait_path(self)` | `Option<Path<'d>>` | The trait of `impl Trait for Type`; none for an inherent impl. |
| `self_ty(self)` | `Type<'d>` | The type the impl is for. |
| `items(self)` | `Vec<Decl<'d>>` | The functions and constants, in source order. |

### `Nature`

`nature Name { potential: .., flow: .., value: .., resolve: .. }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `kind(self)` | `NatureKind` | `Conservative`, `SignalFlow`, `Discrete`, `Resolved` or `Invalid`, by the fields declared. |
| `generics(self)` | `Vec<GenericParam<'d>>` | The generic parameters of the nature. |
| `quantities(self)` | `Vec<Quantity<'d>>` | The quantities in source order, repeated ones included. |
| `potential(self)` | `Option<Quantity<'d>>` | The first `potential` quantity. |
| `flow(self)` | `Option<Quantity<'d>>` | The first `flow` quantity. |
| `impls(self)` | `Vec<Impl<'d>>` | The `impl` blocks written for this nature, in source order. |
| `value_type(self)` | `Option<Type<'d>>` | The type of the first `value: T` field. |
| `resolve_fn(self)` | `Option<Expr<'d>>` | The expression of the first `resolve: f` field. |

```rust
use piperine_lang::pom::{Design, NatureKind, QuantityRole, Reflect};

let src = r#"
nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
nature Bit { value: bool }
nature Wire { value: bool, resolve: pick }
nature Sig { potential: Level { access: lvl, units: "1", abstol: 1u } }
"#;
let design = Design::builder().source("top.msl", src).build();
let e = design.nature("Electrical").unwrap();
assert_eq!(e.kind(), NatureKind::Conservative);
assert_eq!(e.quantities().len(), 2);
let v = e.potential().unwrap();
assert_eq!(v.name(), Some("Voltage"));
assert_eq!(v.role(), QuantityRole::Potential);
assert_eq!(v.access().unwrap().text(), "v");
assert_eq!(v.units().unwrap().text(), "\"V\"");
assert_eq!(e.flow().unwrap().name(), Some("Current"));

assert_eq!(design.nature("Bit").unwrap().kind(), NatureKind::Discrete);
assert_eq!(design.nature("Bit").unwrap().value_type().unwrap().text(), "bool");
assert_eq!(design.nature("Wire").unwrap().kind(), NatureKind::Resolved);
assert_eq!(design.nature("Wire").unwrap().resolve_fn().unwrap().text(), "pick");
assert_eq!(design.nature("Sig").unwrap().kind(), NatureKind::SignalFlow);
```

### `Quantity`

`potential: Voltage { access: v, units: "V", abstol: 1u }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `role(self)` | `QuantityRole` | `Potential` or `Flow`. |
| `access(self)` | `Option<Expr<'d>>` | The `access: v` attribute: the name of the access function. |
| `units(self)` | `Option<Expr<'d>>` | The `units:` attribute. |
| `abstol(self)` | `Option<Expr<'d>>` | The `abstol:` attribute. |

### `Module`

`[extern "abi"] module Name<..>(ports) -> ret where cond { body }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `generics(self)` | `Vec<GenericParam<'d>>` | The generic parameters of the head. |
| `behaviors(self)` | `Vec<Behavior<'d>>` | The behavior blocks of this module (in its file), in source order. |
| `instances(self)` | `Vec<Instance<'d>>` | Every instance placed by this module's body and blocks, in source order: named ones written as `let name = Callee(..)` and anonymous ones (a built-in or declared module called in a block). Generation is not unrolled: an instance in a loop body is listed once. |
| `instance(self, name: &str)` | `Option<Instance<'d>>` | The instance named `name` by its `let`. |
| `analog_blocks(self)` | `Vec<Behavior<'d>>` | The `analog` blocks of this module, in source order. |
| `digital_blocks(self)` | `Vec<Behavior<'d>>` | The `digital` blocks of this module, in source order. |
| `ports(self)` | `Vec<Port<'d>>` | The ports in source order. |
| `port(self, name: &str)` | `Option<Port<'d>>` | The port named `name`. |
| `items(self)` | `Vec<Decl<'d>>` | The body's items in source order, generation held as one item. |
| `params(self)` | `Vec<Param<'d>>` | The params in source order. |
| `param(self, name: &str)` | `Option<Param<'d>>` | The param named `name`. |
| `signals(self)` | `Vec<Signal<'d>>` | The signals in source order, including those written inside generation. |
| `signal(self, name: &str)` | `Option<Signal<'d>>` | The signal named `name`. |
| `lets(self)` | `Vec<Let<'d>>` | The `let` items in source order, including those inside generation. |
| `named(self, name: &str)` | `Option<Binding<'d>>` | The named expression `name` introduced by a body `let`. |
| `definitions(self)` | `Vec<Definition<'d>>` | The definitions in source order, including those inside generation. |
| `generations(self)` | `Vec<Generation<'d>>` | The generation items written directly in the body. |
| `member(self, name: &str)` | `Option<Decl<'d>>` | What the name `name` denotes in the module: a port, param, signal or `let`-named expression, as a declaration. |
| `result(self)` | `Option<Expr<'d>>` | The trailing expression: the module's result. |
| `result_type(self)` | `Option<Type<'d>>` | The `-> T` type. |
| `where_clause(self)` | `Option<Expr<'d>>` | The `where` condition of the head. |
| `is_extern(self)` | `bool` | Whether the module is written `extern "abi"`. |
| `abi(self)` | `Option<&'d str>` | The ABI string of `extern "abi" module`, without quotes. |
| `instantiated_by(self)` | `Vec<Instance<'d>>` | Every instance, in the whole design, whose `module()` is this module, in source order. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module Cell(p: Bit) {}
module Top(in a: Bit, out y: Bit, g: f64 = 1.0) -> f64 where g > 0.0 {
    gain: f64 = 2.0;
    signal s: Bit;
    let u = Cell(s);
    y = a;
    if true { signal t: Bit; }
    g
}
"#;
let design = Design::builder().source("top.msl", src).build();
let top = design.module("Top").unwrap();

// Head: ports in order, the result type, the `where` condition.
assert_eq!(top.ports().len(), 3);
assert_eq!(top.port("y").unwrap().index(), 1);
assert_eq!(top.result_type().unwrap().text(), "f64");
assert!(top.where_clause().is_some());
assert_eq!(top.result().unwrap().text(), "g");

// Body: params, signals, lets, definitions. The listings are deep: they include the
// items written inside generation (`t` is inside the `if`).
assert_eq!(top.params().len(), 1);
assert_eq!(top.param("gain").unwrap().default().unwrap().text(), "2.0");
let signals: Vec<_> = top.signals().iter().map(|s| s.name().unwrap()).collect();
assert_eq!(signals, ["s", "t"]);
assert_eq!(top.lets().len(), 1);
assert_eq!(top.definitions().len(), 1);

// `items()` is the body as written; a generation is one item.
assert_eq!(top.items().len(), 5);
assert_eq!(top.generations().len(), 1);
assert!(top.signal("t").unwrap().generation().is_some());

// `member(name)` finds any named thing the module declares.
assert!(top.member("a").unwrap().as_port().is_some());
assert!(top.member("s").unwrap().as_signal().is_some());
assert!(top.member("u").unwrap().as_binding().is_some());

// A named `let` introduces a binding, and a call of a module is an instance.
assert_eq!(top.named("u").unwrap().instance().unwrap().module(), design.module("Cell"));
assert_eq!(top.instances().len(), 1);
assert_eq!(top.instance("u").unwrap().callee().text(), "Cell");
```

### `Port`

One port of a module.

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The 0-based position among the module's ports. |
| `direction(self)` | `Option<Dir>` | `In`, `Out` or none when no word is written. |
| `ty(self)` | `Type<'d>` | The declared type, as written. |
| `default(self)` | `Option<Expr<'d>>` | The `= expr` default of the port (a driven carrier or value input). |
| `kind(self)` | `PortKind` | The classification of the port (`Unknown` until classified). |
| `nature(self)` | `Option<Nature<'d>>` | The nature of the port's type, through arrays and `flip`. |
| `writers(self)` | `Vec<Writer<'d>>` | Every definition and commit whose place is rooted at this port, in source order. |

```rust
use piperine_lang::pom::{Design, Dir, PortKind, Reflect};

let src = r#"
nature Bit { value: bool }
struct Bus { valid: Bit }
module M(in a: Bit, out y: Bit = false, bus: Bus, k: f64) {}
"#;
let design = Design::builder().source("top.msl", src).build();
let m = design.module("M").unwrap();
let a = m.port("a").unwrap();
assert_eq!(a.direction(), Some(Dir::In));
assert_eq!(a.kind(), PortKind::Carrier);
assert_eq!(a.nature(), design.nature("Bit"));
assert_eq!(a.ty().text(), "Bit");
assert_eq!(m.port("y").unwrap().default().unwrap().text(), "false");
assert_eq!(m.port("bus").unwrap().kind(), PortKind::Bundle);
assert_eq!(m.port("k").unwrap().kind(), PortKind::ValueInput);
assert_eq!(m.port("k").unwrap().nature(), None);
```

### `Param`

`name: ty = default where cond;` in a module body.

| Method | Returns | Meaning |
| --- | --- | --- |
| `ty(self)` | `Type<'d>` | The declared type. |
| `default(self)` | `Option<Expr<'d>>` | The default; none makes the param required (unless the type is an `Option`). |
| `where_clause(self)` | `Option<Expr<'d>>` | The `where` condition. |
| `is_required(self)` | `bool` | Syntactic: no default, and a type that is not a path ending in `Option`. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = "module M() { r: f64 = 1k; n: f64; m: Option<f64>; }";
let design = Design::builder().source("top.msl", src).build();
let m = design.module("M").unwrap();
assert!(!m.param("r").unwrap().is_required());   // has a default
assert!(m.param("n").unwrap().is_required());    // no default
assert!(!m.param("m").unwrap().is_required());   // an Option may be left out
assert_eq!(m.param("r").unwrap().ty().text(), "f64");
```

### `Signal`

`signal name: ty = init;`

| Method | Returns | Meaning |
| --- | --- | --- |
| `ty(self)` | `Type<'d>` | The declared type. |
| `init(self)` | `Option<Expr<'d>>` | The `= init` value. |
| `nature(self)` | `Option<Nature<'d>>` | The nature of the signal's type, through arrays and `flip`. |
| `generation(self)` | `Option<Generation<'d>>` | The generation the signal is written in, if its parent is one. |
| `writers(self)` | `Vec<Writer<'d>>` | Every definition and commit whose place is rooted at this signal (`s`, `s[i]`, `s.f`, `(s)`), in source order. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module M() {
    signal a: Bit = false;
    signal buf: [Bit; 4];
    for k in 0..4 { signal s: Bit; }
}
"#;
let design = Design::builder().source("top.msl", src).build();
let m = design.module("M").unwrap();
let a = m.signal("a").unwrap();
assert_eq!(a.init().unwrap().text(), "false");
assert_eq!(a.nature(), design.nature("Bit"));
assert!(a.generation().is_none());
// Through an array type the nature is still found.
assert_eq!(m.signal("buf").unwrap().nature(), design.nature("Bit"));
// The loop body is ONE declaration, whatever the bound.
let s = m.signal("s").unwrap();
assert!(s.generation().unwrap().is_for());
assert_eq!(m.signals().len(), 3);
```

### `Let`

`let pat = value;` in a module body.

| Method | Returns | Meaning |
| --- | --- | --- |
| `pattern(self)` | `Pat<'d>` | The pattern of the `let`. |
| `value(self)` | `Expr<'d>` | The whole value expression. |
| `bindings(self)` | `Vec<Binding<'d>>` | The names the pattern introduces, in source order. |

```rust
use piperine_lang::pom::{Design, BindingOrigin, Reflect};

let src = "module M() { let (a, b) = pair(); let c = 1.0; }";
let design = Design::builder().source("top.msl", src).build();
let m = design.module("M").unwrap();
let lets = m.lets();
assert_eq!(lets[0].pattern().text(), "(a, b)");
assert_eq!(lets[0].value().text(), "pair()");
let names: Vec<_> = lets[0].bindings().iter().map(|b| b.name().unwrap()).collect();
assert_eq!(names, ["a", "b"]);
let c = m.named("c").unwrap();
assert_eq!(c.origin(), BindingOrigin::BodyLet);
assert_eq!(c.value().unwrap().text(), "1.0");
assert_eq!(c.let_item(), Some(lets[1]));
```

### `Binding`

One identifier a pattern introduces.

| Method | Returns | Meaning |
| --- | --- | --- |
| `origin(self)` | `BindingOrigin` | The syntactic place the binding comes from. |
| `is_binding(self)` | `bool` | Does the name bind a value? `false` when the pattern is a plain identifier that names a constant, a unit variant or a unit struct: it is a path then (§1), and this declaration stays for navigation. |
| `pattern(self)` | `Pat<'d>` | The pattern that introduces the name. |
| `let_item(self)` | `Option<Let<'d>>` | The `let` item the name comes from, for a body `let`. |
| `instance(self)` | `Option<Instance<'d>>` | The instance this name names: the `let` value is a call classified as an instance and the pattern is this one identifier. |
| `value(self)` | `Option<Expr<'d>>` | The value of the `let` the name comes from: the whole expression, the pattern is not matched against it. |

### `Definition`

`place = value after delay;`

| Method | Returns | Meaning |
| --- | --- | --- |
| `place(self)` | `Expr<'d>` | The assigned place. |
| `value(self)` | `Expr<'d>` | The value. |
| `after(self)` | `Option<Expr<'d>>` | The `after` delay. |

```rust
use piperine_lang::pom::{Design, Reflect};

let src = "nature Bit { value: bool }\nmodule M(a: Bit) { signal q: Bit; q = !a after 1n; }";
let design = Design::builder().source("top.msl", src).build();
let d = design.module("M").unwrap().definitions()[0];
assert_eq!(d.place().text(), "q");
assert_eq!(d.value().text(), "!a");
assert_eq!(d.after().unwrap().text(), "1n");
```

### `Generation`

A generation `for` or `if` over module or digital items.

| Method | Returns | Meaning |
| --- | --- | --- |
| `is_for(self)` | `bool` | Whether the generation is a `for` (else an `if`). |
| `pattern(self)` | `Option<Pat<'d>>` | The pattern of a `for`. |
| `iter(self)` | `Option<Expr<'d>>` | The iterated expression of a `for`. |
| `items(self)` | `Vec<Decl<'d>>` | The items of the `for` body, or of the first arm of an `if`. |
| `arms(self)` | `Vec<GenArm<'d>>` | The arms of an `if` chain in order: each `if` and `else if` condition, then the final `else` with none. A `for` has no arms. |
| `generations(self)` | `Vec<Generation<'d>>` | The generations written directly inside this one (an `else if` is one of them). |

Generation is held as written: nothing is unrolled. A `for` has an iterator and a body; an `if`
chain is a list of arms.

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Bit { value: bool }
module M() {
    for k in 0..1000 { signal s: Bit; }
    if true { signal a: Bit; } else if false { signal b: Bit; } else { signal c: Bit; }
}
"#;
let design = Design::builder().source("top.msl", src).build();
let m = design.module("M").unwrap();
let gens = m.generations();
assert_eq!(gens.len(), 2);

let f = gens[0];
assert!(f.is_for());
assert_eq!(f.pattern().unwrap().text(), "k");
assert_eq!(f.iter().unwrap().text(), "0..1000");
assert_eq!(f.items().len(), 1);                   // one declaration for a thousand iterations

let i = gens[1];
assert!(!i.is_for());
let arms = i.arms();
assert_eq!(arms.len(), 3);
assert_eq!(arms[0].condition().unwrap().text(), "true");
assert_eq!(arms[1].condition().unwrap().text(), "false");
assert!(arms[2].condition().is_none());            // the final `else`
assert_eq!(arms[2].items()[0].name(), Some("c"));
```

### `GenArm`

One arm of a generation `if` chain.

| Method | Returns | Meaning |
| --- | --- | --- |
| `condition(self)` | `Option<Expr<'d>>` | The arm's condition; none for the final `else`. |
| `items(self)` | `Vec<Decl<'d>>` | The items of the arm. |

### `Behavior`

`analog<N> for Target { .. }` or `digital for Target { .. }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `kind(self)` | `BehaviorKind` | `Analog` or `Digital`. |
| `names(self)` | `Vec<&'d str>` | The generic names of `analog<N, M>`, in order. |
| `module(self)` | `Option<Module<'d>>` | The module this block belongs to: the one its target names, when that module is declared in the same file and is not `extern`. |
| `target(self)` | `Path<'d>` | The target path as written, generic arguments included. |
| `statements(self)` | `Vec<Stmt<'d>>` | The statements of an `analog` block; none for `digital`. |
| `contributions(self)` | `Vec<Contribution<'d>>` | Every contribution of an `analog` block at any depth, in source order. |
| `reactions(self)` | `Vec<Reaction<'d>>` | The reactions of a `digital` block in source order, including those inside generation. |
| `generations(self)` | `Vec<Generation<'d>>` | The generation items written directly in a `digital` block. |

```rust
use piperine_lang::pom::{BehaviorKind, Design, Reflect};

let src = r#"
nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
nature Bit { value: bool }
module R(p: Electrical, n: Electrical, in clk: Bit, out q: Bit) { r: f64 = 1k; }
analog for R { i(p, n) +> v(p, n) / r; }
digital for R { on clk { q <- true after 1n; } }
analog for Missing {}
"#;
let design = Design::builder().source("top.msl", src).build();
let r = design.module("R").unwrap();
assert_eq!(r.behaviors().len(), 2);

let analog = &r.analog_blocks()[0];
assert_eq!(analog.kind(), BehaviorKind::Analog);
assert_eq!(analog.module(), Some(r));
assert_eq!(analog.target().text(), "R");
let c = analog.contributions()[0];
assert_eq!(c.lhs().text(), "i(p, n)");
assert_eq!(c.rhs().text(), "v(p, n) / r");

let digital = &r.digital_blocks()[0];
let reaction = digital.reactions()[0];
assert_eq!(reaction.event().text(), "clk");
assert_eq!(reaction.commits()[0].place().text(), "q");
assert_eq!(reaction.commits()[0].after().unwrap().text(), "1n");

// A block that names no module stays in the tree with `module() == None`, and E0401 is reported.
let missing = design.root().children().into_iter().filter_map(|d| d.as_behavior()).last().unwrap();
assert!(missing.module().is_none());
assert!(design.diagnostics().iter().any(|d| d.diagnostic.code == "E0401"));
```

### `Reaction`

`on event { .. }`

| Method | Returns | Meaning |
| --- | --- | --- |
| `event(self)` | `Expr<'d>` | The event expression after `on`. |
| `body(self)` | `&'d Block` | The block of statements and commits. |
| `commits(self)` | `Vec<Commit<'d>>` | Every commit of the body at any depth, in source order. |

### `Contribution`

`lhs +> rhs;`

| Method | Returns | Meaning |
| --- | --- | --- |
| `stmt(self)` | `Stmt<'d>` | The statement this is a view of. |
| `lhs(self)` | `Expr<'d>` | The left side: the probe (`i(p, n)`). |
| `rhs(self)` | `Expr<'d>` | The contributed expression. |

### `Commit`

`place <- value [after delay];`

| Method | Returns | Meaning |
| --- | --- | --- |
| `stmt(self)` | `Stmt<'d>` | The statement this is a view of. |
| `place(self)` | `Expr<'d>` | The written place. |
| `value(self)` | `Expr<'d>` | The committed value. |
| `after(self)` | `Option<Expr<'d>>` | The `after` delay. |

### `Instance`

A placement: a view over a call expression classified as an instance. It is not a declaration and not a node of its own.

| Method | Returns | Meaning |
| --- | --- | --- |
| `expr(self)` | `Expr<'d>` | The call expression. |
| `name(self)` | `Option<&'d str>` | The name of the `let` whose whole value is this call, when its pattern is one identifier; none for an anonymous instance. |
| `is_anonymous(self)` | `bool` | Whether the instance has no name (no `let name =` around the call). |
| `module(self)` | `Option<Module<'d>>` | The module placed, when a declaration names it. |
| `via(self)` | `Option<Decl<'d>>` | The template (param, value input, constant, binding or function) the call was written through. |
| `via_template(self)` | `bool` | Whether the call was written through a template. |
| `callee(self)` | `Expr<'d>` | The called expression. |
| `declared_in(self)` | `Decl<'d>` | The declaration the placement is written in: the module body or generation for a `let`, the block or reaction otherwise. |
| `connections(self)` | `Vec<Expr<'d>>` | The connection arguments, in order. |
| `connection(self, port: Port<'d>)` | `Option<Expr<'d>>` | The argument written for `port`: the connection named after it, else the unnamed one at its position. None when the instance's module is unknown, is not the port's module, or no argument is written there. |
| `param_bindings(self)` | `&'d [FieldInit]` | The `{ name: value }` param bindings as written. |

An instance is a view over a call classified as a module instantiation. It is never a node of its
own: `Instance::expr()` is the call expression, and its name is the `let` around it.

```rust
use piperine_lang::pom::{Design, Reflect};

let src = r#"
nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
module Resistor(p: Electrical, n: Electrical, tc: Option<f64>) { r: f64 = 1.0; }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let r1 = Resistor(a, b) { r: 1k };
    let r2 = Resistor(a, b, tc: 0.5);
}
"#;
let design = Design::builder().source("top.msl", src).build();
let resistor = design.module("Resistor").unwrap();
let top = design.module("Top").unwrap();

let r1 = top.instance("r1").unwrap();
assert_eq!(r1.name(), Some("r1"));
assert_eq!(r1.module(), Some(resistor));
assert_eq!(r1.callee().text(), "Resistor");
assert_eq!(r1.declared_in(), top.decl());
let conns: Vec<_> = r1.connections().iter().map(|c| c.text()).collect();
assert_eq!(conns, ["a", "b"]);
assert_eq!(r1.param_bindings()[0].name.text, "r");

// `connection(port)` finds the argument for a port by name or by position.
let (p, tc) = (resistor.port("p").unwrap(), resistor.port("tc").unwrap());
assert_eq!(r1.connection(p).unwrap().text(), "a");
let r2 = top.instance("r2").unwrap();
assert_eq!(r2.connection(tc).unwrap().text(), "0.5");   // written `tc: 0.5`

// An instance is a view of its call, and the call knows it.
assert_eq!(r1.expr().as_instance(), Some(r1));
assert!(!r1.is_anonymous() && !r1.via_template());
assert_eq!(resistor.instantiated_by().len(), 2);
```

### `Expr`

An expression node.

| Method | Returns | Meaning |
| --- | --- | --- |
| `id(self)` | `ExprId` | The id of the node in its arena. |
| `node(self)` | `&'d ExprNode` | The raw storage node. |
| `kind(self)` | `&'d ExprKind` | What it is, with its payload. |
| `span(self)` | `Span` | The byte range in the file text. |
| `enclosing(self)` | `Decl<'d>` | The nearest declaration that contains this expression. |
| `text(self)` | `&'d str` | The authored text of the expression. |
| `as_instance(self)` | `Option<Instance<'d>>` | The instance this call is, when it is classified as one. |
| `as_path(self)` | `Option<Path<'d>>` | The path this expression is, when it is a path expression (`s`, `a::X`). |
| `resolution(self)` | `Resolution` | What this expression names: a path's last segment, a struct literal's path, a field access or method call's member, a call's callee. Every other expression names nothing and answers `Unresolved`. |
| `target(self)` | `Option<Decl<'d>>` | The declaration `resolution()` names, as a handle; none for an unresolved name, a built-in or a deferred one. |

Body nodes are read through `Expr`, `Stmt`, `Pat`, `Type` and `Path`. Each gives the node
(`kind()`), its range (`span()`), its authored text and the declaration that owns it.

```rust
use piperine_lang::pom::{Design, ExprKind, Reflect};

let src = "const K: f64 = 1.5 + 2.0 * 3.0;";
let design = Design::builder().source("top.msl", src).build();
let k = design.root().children()[0].as_const().unwrap();
let value = k.value().unwrap();
assert_eq!(value.text(), "1.5 + 2.0 * 3.0");
assert!(matches!(value.kind(), ExprKind::Binary { .. }));
assert_eq!(value.enclosing(), k.decl());
assert_eq!(&src[value.span().range()], value.text());
assert!(value.as_instance().is_none());
assert_eq!(k.ty().text(), "f64");
```

### `Stmt`

A statement node.

| Method | Returns | Meaning |
| --- | --- | --- |
| `id(self)` | `StmtId` | The id of the node in its arena. |
| `node(self)` | `&'d StmtNode` | The raw storage node. |
| `kind(self)` | `&'d StmtKind` | What it is, with its payload. |
| `span(self)` | `Span` | The byte range in the file text. |
| `enclosing(self)` | `Decl<'d>` | The nearest declaration that contains it. |
| `text(self)` | `&'d str` | The authored text. |

```rust
use piperine_lang::pom::{Design, Reflect, StmtKind};

let src = "nature Bit { value: bool }\nmodule M(a: Bit, out q: Bit) {}\ndigital for M { on a { q <- true; let x = 1; } }";
let design = Design::builder().source("top.msl", src).build();
let reaction = design.module("M").unwrap().digital_blocks()[0].reactions()[0];
let stmts = reaction.body().stmts.clone();
let first = design.stmt(stmts[0]);
assert!(matches!(first.kind(), StmtKind::Commit { .. }));
assert_eq!(first.text(), "q <- true;");
assert_eq!(first.enclosing(), reaction.decl());
assert!(matches!(design.stmt(stmts[1]).kind(), StmtKind::Let { .. }));
```

### `Pat`

A pattern node.

| Method | Returns | Meaning |
| --- | --- | --- |
| `id(self)` | `PatId` | The id of the node in its arena. |
| `node(self)` | `&'d PatNode` | The raw storage node. |
| `kind(self)` | `&'d PatKind` | What it is, with its payload. |
| `span(self)` | `Span` | The byte range in the file text. |
| `enclosing(self)` | `Decl<'d>` | The nearest declaration that contains it. |
| `text(self)` | `&'d str` | The authored text. |

```rust
use piperine_lang::pom::{Design, PatKind, Reflect};

let design = Design::builder().source("top.msl", "module M() { let (a, b) = pair(); }").build();
let l = design.module("M").unwrap().lets()[0];
let pat = l.pattern();
assert!(matches!(pat.kind(), PatKind::Tuple(items) if items.len() == 2));
assert_eq!(pat.text(), "(a, b)");
```

### `Type`

A type node.

| Method | Returns | Meaning |
| --- | --- | --- |
| `id(self)` | `TypeId` | The id of the node in its arena. |
| `node(self)` | `&'d TypeNode` | The raw storage node. |
| `kind(self)` | `&'d TypeKind` | What it is, with its payload. |
| `span(self)` | `Span` | The byte range in the file text. |
| `enclosing(self)` | `Decl<'d>` | The nearest declaration that contains it. |
| `text(self)` | `&'d str` | The authored text. |

```rust
use piperine_lang::pom::{Design, Reflect, TypeKind};

let src = "nature Bit { value: bool }\nmodule M(a: [flip Bit; 4]) {}";
let design = Design::builder().source("top.msl", src).build();
let ty = design.module("M").unwrap().port("a").unwrap().ty();
assert_eq!(ty.text(), "[flip Bit; 4]");
assert!(matches!(ty.kind(), TypeKind::Array { .. }));
```

### `Path`

A path node.

| Method | Returns | Meaning |
| --- | --- | --- |
| `id(self)` | `PathId` | The id of the node in its arena. |
| `node(self)` | `&'d PathNode` | The raw storage node. |
| `segments(self)` | `&'d [PathSegment]` | The path segments in order. |
| `span(self)` | `Span` | The byte range in the file text. |
| `enclosing(self)` | `Decl<'d>` | The nearest declaration that contains it. |
| `text(self)` | `&'d str` | The authored text. |
| `resolution(self)` | `Resolution` | What the path names: the resolution of its last segment. |
| `target(self)` | `Option<Decl<'d>>` | The declaration the path names, as a handle; none when the last segment is unresolved, a built-in or deferred. |

A path stores one resolution per segment (`segments()[i].res`); the path's own resolution is that
of its last segment. `target()` turns it into the declaration handle.

```rust
use piperine_lang::pom::{Design, Reflect, Resolution};

let src = "mod a { pub const X: f64 = 1.0; }\nconst Y: f64 = a::X;\n";
let design = Design::builder().source("top.msl", src).build();
let y = design.find("Y").unwrap().as_const().unwrap();
let kinds = y.value().unwrap();
let a_x = design.find("a::X").unwrap();
// The value of `Y` is the path `a::X`: two segments, the last resolved to the const `X`.
let path = match kinds.kind() {
    piperine_lang::pom::ExprKind::Path(p) => design.path(*p),
    other => panic!("{other:?}"),
};
assert_eq!(path.text(), "a::X");
assert_eq!(path.segments().len(), 2);
assert_eq!(path.segments()[1].res, Resolution::Decl(a_x.id()));
assert_eq!(path.enclosing(), y.decl());
```

### `Reference`

A node that names a declaration: see `Reflect::references`.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Path` | `(Path<'d>)` | A path (a segment of it resolved to the declaration). |
| `Member` | `(Expr<'d>)` | A `.name` or `.method()` expression. |
| `Pattern` | `(Pat<'d>)` | An identifier pattern that names the declaration (a constant, a unit variant, a unit struct). |

```rust
use piperine_lang::pom::{Design, Reference, Reflect};

let src = "nature Bit { value: bool }\nmodule M(a: Bit) { signal s: Bit; s = a; let u = a; }";
let design = Design::builder().source("top.msl", src).build();
let a = design.module("M").unwrap().port("a").unwrap();
let refs = a.references();
assert_eq!(refs.len(), 2);                              // `s = a;` and `let u = a;`
assert!(refs.iter().all(|r| matches!(r, Reference::Path(p) if p.text() == "a")));
// The nature `Bit` is referenced by the type of `a` and of `s`.
assert_eq!(design.nature("Bit").unwrap().references().len(), 2);
```

Navigating from a use to its definition: a signal read in a reaction, the path it is, the
declaration it names, and the module that declares it.

```rust
use piperine_lang::pom::{Design, Reflect, Resolution};

let src = "nature Bit { value: bool }\nmodule M(a: Bit, out q: Bit) { signal s: Bit; s = a; }\ndigital for M { on a { q <- s; } }";
let design = Design::builder().source("top.msl", src).build();
let m = design.module("M").unwrap();
let commit = m.digital_blocks()[0].reactions()[0].commits()[0];
let read = commit.value();                        // the `s` of `q <- s;`
let signal = read.target().unwrap().as_signal().unwrap();
assert_eq!(signal, m.signal("s").unwrap());
assert_eq!(signal.declared_in(), Some(m.decl()));
// The same through the id a resolution holds.
let Resolution::Decl(id) = read.resolution() else { panic!("resolved") };
assert_eq!(design.decl(id), signal.decl());
// From the declaration back to its uses: the read is among them.
assert!(signal.references().iter().any(|r| matches!(r, piperine_lang::pom::Reference::Path(p) if p.target() == Some(signal.decl()))));
```

### `Writer`

Something that writes a place: see `Signal::writers`.

Something that writes a place: see `Signal::writers`.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Definition` | `(Definition<'d>)` | `place = value;` in a module body. |
| `Commit` | `(Commit<'d>)` | `place <- value;` in a reaction. |

```rust
use piperine_lang::pom::{Design, Reflect, Writer};

let src = r#"
nature Bit { value: bool }
module M(a: Bit, out q: Bit) { signal s: Bit; s = a; }
digital for M { on a { q <- s; s[0] <- false; } }
"#;
let design = Design::builder().source("top.msl", src).build();
let m = design.module("M").unwrap();
let s = m.signal("s").unwrap().writers();
assert_eq!(s.len(), 2);
assert!(matches!(s[0], Writer::Definition(d) if d.place().text() == "s"));
assert!(matches!(s[1], Writer::Commit(c) if c.place().text() == "s[0]"));   // rooted at `s`
assert_eq!(m.port("q").unwrap().writers().len(), 1);
assert!(m.port("a").unwrap().writers().is_empty());                         // reads are not writes
```

## Helpers on storage types

A few public functions live on storage types rather than handles.

#### `BuiltinId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `info(self)` | `&'static BuiltinInfo` | The table row; for a sized type, the row of its family. |
| `width(self)` | `Option<u8>` | The width of a `uN`, `iN` or `fN` entry. |
| `name(self)` | `String` | The exact spelling: `u8` for the entry the family row `uN` gives width 8. |

#### `Index`

| Method | Returns | Meaning |
| --- | --- | --- |
| `references(&self, decl: DeclId)` | `&[ReferenceId]` | The paths and members that name `decl`, in source order. |
| `writers(&self, decl: DeclId)` | `&[WriterId]` | The definitions and commits whose place is rooted at `decl`, in source order. |
| `instantiated_by(&self, decl: DeclId)` | `&[ExprId]` | The instance calls that place module `decl`, in source order. |

#### `DeclKind`

| Method | Returns | Meaning |
| --- | --- | --- |
| `label(&self)` | `&'static str` | The variant's name, as `dump()` prints it. |

#### `MemorySources`

| Method | Returns | Meaning |
| --- | --- | --- |
| `new()` | `Self` | An empty set of files. |
| `insert(&self, path: impl AsRef<Path>, text: impl Into<String>)` | `()` | Add or replace the file at `path`. |

#### `SourceProvider`

| Method | Returns | Meaning |
| --- | --- | --- |
| `read(&self, path: &Path)` | `Option<Vec<u8>>` | The bytes of the file at `path`; none when it cannot be read. |
| `canonical(&self, path: &Path)` | `PathBuf` | The path in a form that is equal for two spellings of the same file; the cycle check compares these. |

#### `FileId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The id as a `usize`. |

#### `DeclId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The id as a `usize`. |

#### `ExprId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The id as a `usize`. |

#### `StmtId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The id as a `usize`. |

#### `PatId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The id as a `usize`. |

#### `TypeId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The id as a `usize`. |

#### `PathId`

| Method | Returns | Meaning |
| --- | --- | --- |
| `index(self)` | `usize` | The id as a `usize`. |

