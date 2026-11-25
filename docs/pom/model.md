# The storage model

This page is the reference for what a `Design` stores: every id, node, payload and resolution
slot, with its fields. It is generated from the types of `crates/piperine-lang/src/pom/` and
checked against them: every `pub struct`, `pub enum` and `pub trait` of that tree has a
``### `Name` `` heading here (or, for the handles, below in [Handles](#handles)).

Use it to understand what the data *is*. To read it, prefer the handles of
[Reflection](reflection.md): you will rarely touch a `*Data` or `*Node` value directly.

## How the storage is organised

```text
Design
 ├─ files:  Vec<SourceFileData>      text, retained syntax tree, root namespace
 ├─ decls:  Vec<DeclData>            every item/port/param/signal/binding; the authored tree
 │            └─ kind: DeclKind      one variant per handle, with its payload (ModuleData, ..)
 ├─ exprs / stmts / pats / types / paths      body nodes, addressed by ExprId, StmtId, ..
 ├─ diagnostics: Vec<DesignDiagnostic>
 └─ index:  Index                    references / writers / instantiated_by
```

* **Arenas and ids.** Each arena is a `Vec`; a node is addressed by a `u32` newtype id. Nodes
  never move or disappear after the build, so an id is a stable identity: handle equality is
  "same design, same id".
* **Naming.** Storage types end in `Data` (a declaration payload) or `Node` (a body node);
  handles have the plain names (`Module` over `ModuleData`).
* **The authored tree.** `DeclData::parent` and `children` give the lexical tree exactly as
  written. Body nodes point to their children by id and to the declaration that owns them by
  `owner`.
* **Links are ids, never ownership.** `Resolution::Decl`, `BehaviorData::module`,
  `ModuleData::behaviors` and `CallClass::Instance` refer across the tree by `DeclId`.
* **Slots.** A resolution slot (`Resolution`, `CallClass`, `StructClass`, `PortKind`,
  `StructData::bundle`, ..) starts in its first state at lowering and is written once by name
  resolution. Nothing else changes after lowering.
* **An `Instance` is not stored.** It is a view over a `Call` expression classified as an
  instance, so classifying a call creates no node. Its name is the binding of the module-body
  `let` whose whole value is that call, when the pattern is one identifier.
* **Serialization.** Every storage type is `Serialize + Deserialize`. The retained syntax tree
  is not serialized: it is rebuilt from the file text on load. The derived `Deserialize` is unvalidated; `Design::from_json` is the entry point for text that did not come from `build`.

## Identifiers

Every arena index is a `u32` newtype. Ids are plain data: they are `Copy`, ordered, hashable, serializable, and meaningful only for the `Design` that issued them.

### `FileId`

*struct* in `src/pom/ids.rs`.

A source file of the design.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the file list. |

### `CrateId`

*struct* in `src/pom/ids.rs`.

A crate of a design, or of a crate graph.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the crate list. |

### `DeclId`

*struct* in `src/pom/ids.rs`.

A declaration of any kind.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the declaration arena. |

### `ExprId`

*struct* in `src/pom/ids.rs`.

An expression node.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the expression arena. |

### `StmtId`

*struct* in `src/pom/ids.rs`.

A statement node.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the statement arena. |

### `PatId`

*struct* in `src/pom/ids.rs`.

A pattern node.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the pattern arena. |

### `TypeId`

*struct* in `src/pom/ids.rs`.

A type node.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the type arena. |

### `PathId`

*struct* in `src/pom/ids.rs`.

A path node.

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u32` | Index into the path arena. |

## The design and its builder

`Design` owns every arena; its fields are private and read through the methods of [Reflection](reflection.md). `DesignBuilder` collects sources and builds.

### `SourceFileData`

*struct* in `src/pom/design.rs`.

A source file as the design stores it: its text and the retained syntax tree. The tree is not part of the serialized form; it is rebuilt from the text (see `SourceFileRepr`).

Read through the handle `SourceFile` (see [Reflection](reflection.md#sourcefile)).

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `String` | As registered, or the file name on disk. |
| `path` | `Option<PathBuf>` | The path it was read from (virtual or on disk). |
| `krate` | `CrateId` | The crate the file belongs to. |
| `text` | `String` | The text, verbatim. |
| `ast` | `ast::File` | The retained syntax tree; not serialized, rebuilt from `text`. |
| `root` | `DeclId` | The file's `Namespace` declaration. |

### `SourceFileRepr`

*struct* in `src/pom/design.rs`.

The serialized shape of a `SourceFileData`: every field but the tree.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `String` | As in `SourceFileData`. |
| `path` | `Option<PathBuf>` | As in `SourceFileData`. |
| `krate` | `CrateId` | As in `SourceFileData`. |
| `text` | `String` | As in `SourceFileData`. |
| `root` | `DeclId` | As in `SourceFileData`. |

### `DesignDiagnostic`

*struct* in `src/pom/design.rs`.

A diagnostic and the file it was found in.

| Field | Type | Meaning |
| --- | --- | --- |
| `file` | `FileId` | The file the diagnostic was found in. |
| `diagnostic` | `Diagnostic` | The diagnostic: code, message, span, label. |

### `Design`

*struct* in `src/pom/design.rs`.

The owning, immutable object model of one set of sources. Build it with `Design::builder()` (or `Design::load` for a tree on disk); nothing mutates it afterwards. It is `Send + Sync`, `Clone`, comparable with `==` and serializable.

| Storage (private) | Type | Meaning |
| --- | --- | --- |
| `files` | `Vec<SourceFileData>` | Every file: the project's (the root, then files loaded by `mod name;`), then the standard library's. Read with `files()`. |
| `crates` | `Vec<CrateData>` | The crates in graph order, `std` last. Read with `crates()`, `root_crate()`, `std_crate()` and `Crate`. |
| `root_crate / std_crate` | `CrateId` | The crate the design was built from, and the standard library. |
| `decls` | `Vec<DeclData>` | The declaration arena. Read with `decl_data`, `decls`, `Decl`. |
| `exprs / stmts / pats / types / paths` | `Vec<..Node>` | The body arenas. Read with `expr_node`, `stmt_node`, `pat_node`, `type_node`, `path_node` or the handles `expr`, `stmt`, `pat`, `ty`, `path`. |
| `diagnostics` | `Vec<DesignDiagnostic>` | Parse, loading and resolution diagnostics, in file then source order. Read with `diagnostics()`. |
| `index` | `Index` | The reverse tables. |
| `names` | by-name tables | Sorted maps (name to declaration ids in source order; module and name to ports, params and signals) behind `Design::module`, `nature`, `struct_named`, `function` and `Module::port`, `param`, `signal`. Built on first use from the finished arenas, never serialized, never part of equality, so a design read from JSON has them too. |
| `expr_parent / expr_stmt` | `Vec<Option<ExprId>>`, `Vec<Option<StmtId>>` | Side tables by `ExprId`, filled once after lowering from the one child enumeration: the expression that contains each one, and the statement that holds it directly. Read with `Expr::parent` and `Expr::stmt`; `check()` verifies them against `Expr::children`. |

### `DesignBuilder`

*struct* in `src/pom/design.rs`.

Collects virtual sources and the standard library's, then builds the [`Design`](#design). `build` never fails: a parse error keeps the recovered tree and is listed in `diagnostics()`.

| Storage (private) | Type | Meaning |
| --- | --- | --- |
| `std_sources` | `Vec<(String, String)>` | Virtual files of `std`, replacing the embedded ones; the first is its root. |
| `graph` | `CrateGraph` | The crates to build; empty means one root crate from `sources` or `root_file`. |
| `sources` | `Vec<(String, String)>` | Virtual files; the first is the root. |
| `root_file` | `Option<PathBuf>` | A root file on disk; replaces the virtual sources. |

## Source providers

Where the files of a source tree come from. `Design::builder()` uses `MemorySources`; `Design::load` uses `DiskSources`.

### `SourceProvider`

*trait* in `src/pom/source.rs`.

Where the files of a tree come from.

| Method | Meaning |
| --- | --- |
| `read(&self, path: &Path) -> Option<Vec<u8>>` | The bytes of the file at `path`; none when it cannot be read. |
| `canonical(&self, path: &Path) -> PathBuf` | The path in a form that is equal for two spellings of the same file; the cycle check of `mod` loading compares these. |

### `MemorySources`

*struct* in `src/pom/source.rs`.

Files held in memory, keyed by their normalized path (`.` removed, `..` folded).

| Storage (private) | Type | Meaning |
| --- | --- | --- |
| `files` | `BTreeMap<PathBuf, String>` | Text by normalized path. |

### `DiskSources`

*struct* in `src/pom/source.rs`.

The filesystem. `canonical` is `std::fs::canonicalize`, falling back to the path itself.

## Crates

A design is built over a crate graph: the root crate first, its dependencies in graph order and
`std` last. The storage keeps the crates as plain data beside the arenas, and every file knows
its crate; no declaration of one crate is a child of another's (the
[rule](index.md#the-one-rule)), so each crate keeps its authored tree.

```text
Design
 ├─ crates:      Vec<CrateData>      name, root file, root namespace, declared dependencies
 ├─ root_crate:  CrateId             the crate the design was built from
 ├─ std_crate:   CrateId             the standard library, last in `crates`
 └─ files:       Vec<SourceFileData> each with its `krate`; numbered crate by crate
```

`Design::check()` verifies the crate table: every link in range, each crate rooted at a file of
its own whose root namespace is the crate's root declaration, and no dependency cycle. Read it
through `Design::crates()`, `krate(name)` and the `Crate` handle; see [Reflection](reflection.md).

## The crate graph

A build over several crates takes a `CrateGraph`: plain data, built by the host (the package loader or a test), with no manifest and no file read. A rejected call leaves the graph unchanged.

### `CrateGraph`

*struct* in `src/pom/crates.rs`.

The crates of a build in insertion order; the first is the root crate.

| Method | Meaning |
| --- | --- |
| `new() -> CrateGraph` | An empty graph. |
| `add_crate(&mut self, name: &str, root: impl Into<PathBuf>) -> Result<CrateId, GraphError>` | Add a crate; `InvalidName` unless `is_crate_name(name)`. |
| `add_dependency(&mut self, from: CrateId, name: &str, to: CrateId) -> Result<(), GraphError>` | Give `from` a dependency called `name` on `to`; `UnknownCrate`, `InvalidName`, `DuplicateDependency` or `Cycle`. |
| `crates(&self) -> &[CrateNode]` | The crates in insertion order. |

`is_crate_name(name: &str) -> bool` is true for one identifier that is no keyword and not `std`.

### `CrateNode`

*struct* in `src/pom/crates.rs`.

One crate of a graph.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `String` | The crate's name. |
| `root` | `PathBuf` | The crate root file. |
| `dependencies` | `Vec<DependencyData>` | Its named dependencies, in the order added. |

### `DependencyData`

*struct* in `src/pom/crates.rs`.

An edge of the graph.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `String` | The name the depending crate gives it. |
| `krate` | `CrateId` | The crate it names. |

### `CrateData`

*struct* in `src/pom/crates.rs`.

A crate as the design stores it. Read through the handle `Crate` (see [Reflection](reflection.md#crate)).

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `String` | The crate's name. |
| `root_file` | `FileId` | The file the crate is read from. |
| `root` | `DeclId` | The root `Namespace` declaration: the root file's. |
| `dependencies` | `Vec<DependencyData>` | As declared; `std` is implicit and not listed. |

### `GraphError`

*enum* in `src/pom/crates.rs`.

Why a call on a `CrateGraph` was refused.

| Variant | Meaning |
| --- | --- |
| `InvalidName { name }` | Not an identifier, a keyword, or `std`. |
| `DuplicateDependency { krate, name }` | The crate already has a dependency of this name. |
| `UnknownCrate { id }` | The id does not belong to the graph. |
| `Cycle { names }` | The edge would close a cycle; `names` lists it, the first crate repeated last. |

## Declarations

Every item, field, port, parameter, signal and binding is a `DeclData` in the declaration arena; `kind` carries the kind-specific payload. `parent` and `children` form the authored tree, exactly as written.

### `Name`

*struct* in `src/pom/decl.rs`.

An identifier as written, with its range.

| Field | Type | Meaning |
| --- | --- | --- |
| `text` | `String` | The identifier text, without any `r#` prefix. |
| `raw` | `bool` | Written `r#text` (one of the seven added keywords); `to_ast` restores the prefix. |
| `span` | `Span` | Byte range in the file text. |

### `DeclData`

*struct* in `src/pom/decl.rs`.

One declaration. `children` is in source order and complete: every declaration lexically inside this one, at the first level.

Read through the handle `Decl` (see [Reflection](reflection.md#decl)).

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | `DeclKind` | The kind and its payload. |
| `name` | `Option<Name>` | The declared name; none for an `impl`, a `use`, a behavior block, an error item and a file root. |
| `parent` | `Option<DeclId>` | The declaration that lexically contains this one; none for a file root. |
| `children` | `Vec<DeclId>` | The declarations lexically inside this one, in source order. |
| `file` | `FileId` | The file the declaration is written in. |
| `span` | `Span` | Byte range of the whole item, attributes and docs included. |
| `generics_span` | `Option<Span>` | The range of the `<..>` list when the declaration wrote one (even an empty one); its parameters are `GenericParam` children. |
| `public` | `bool` | Whether the item is written `pub`. |
| `meta` | `Meta` | Doc comments and attributes. |

### `DeclKind`

*enum* in `src/pom/decl.rs`.

What a declaration is, with its payload.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Namespace` | `(NamespaceData)` | A file root or a `mod`. |
| `Use` | `(UseData)` | A `use` item. |
| `Const` | `(ConstData)` | A `const` item. |
| `Fn` | `(FnData)` | A `fn` item. |
| `FnParam` | `(FnParamData)` | A function parameter. |
| `Struct` | `(StructData)` | A `struct`. |
| `Field` | `(FieldData)` | A field of a struct or variant. |
| `Enum` | `(EnumData)` | An `enum`. |
| `Variant` | `(VariantData)` | A variant of an enum. |
| `Trait` | `(TraitData)` | A `trait`. |
| `Impl` | `(ImplData)` | An `impl` block. |
| `GenericParam` | `(GenericParamData)` | A generic parameter of any generic item. |
| `Nature` | `(NatureData)` | A `nature`. |
| `Quantity` | `(QuantityData)` | A `potential` or `flow` field of a nature. |
| `Module` | `(ModuleData)` | A `module`. |
| `Port` | `(PortData)` | A port in a module head. |
| `Param` | `(ParamData)` | A parameter of a module body (`name: ty = default;`). |
| `Signal` | `(SignalData)` | A `signal` declaration. |
| `Let` | `(LetData)` | A module-body `let`. |
| `Binding` | `(BindingData)` | One identifier a pattern introduces. |
| `Definition` | `(DefinitionData)` | A `place = value;` definition. |
| `Generation` | `(GenerationData)` | A generation `for` or `if` over module or digital items. |
| `Behavior` | `(BehaviorData)` | An `analog` or `digital` block. |
| `Reaction` | `(ReactionData)` | An `on event { }` reaction. |
| `Error` | — | The span of a malformed item. |

### `NamespaceOrigin`

*enum* in `src/pom/decl.rs`.

Where a namespace comes from.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Root` | — | The top of a file. |
| `Inline` | — | `mod a { ... }` |
| `External` | `{ file: Option<FileId> }` | `mod a;`: `file` is the loaded file, none when no file was loaded. |

### `NamespaceData`

*struct* in `src/pom/decl.rs`.

Payload of a `Namespace` declaration (a file root or a `mod`).

Read through the handle `Namespace` (see [Reflection](reflection.md#namespace)).

| Field | Type | Meaning |
| --- | --- | --- |
| `origin` | `NamespaceOrigin` | Where the namespace comes from. |

### `UseData`

*struct* in `src/pom/decl.rs`.

`use a::{b, c as d, e::*};`: the tree as written, and the same imports flattened to one entry per leaf.

Read through the handle `Use` (see [Reflection](reflection.md#use)).

| Field | Type | Meaning |
| --- | --- | --- |
| `tree` | `UseTreeNode` | The tree as written. |
| `imports` | `Vec<Import>` | The leaves, one per imported name, in order. |

### `UseTreeNode`

*struct* in `src/pom/decl.rs`.

One node of a `use` tree: the path written at this level and what follows.

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | `Vec<Name>` | The path written at this level. |
| `kind` | `UseTreeKind` | What follows the path. |
| `span` | `Span` | Byte range in the file text. |

### `UseTreeKind`

*enum* in `src/pom/decl.rs`.

What follows the path at one level of a `use` tree.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Simple` | `{ alias: Option<Name> }` | A leaf, optionally `as alias`. |
| `Glob` | — | `path::*`. |
| `Group` | `(Vec<UseTreeNode>)` | `path::{a, b}`. |

### `Import`

*struct* in `src/pom/decl.rs`.

One leaf of a `use` tree with its full path (every segment from the root of the tree; a `self` leaf names its prefix, so it has no `self` segment). The path node spans only the segments the leaf itself writes, never the braces or the other leaves around it.

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | `PathId` | The full path of the leaf. |
| `alias` | `Option<Name>` | The `as` name. |
| `glob` | `bool` | Whether the leaf is a `*` import. |

### `ConstData`

*struct* in `src/pom/decl.rs`.

`const NAME: ty = value;`; the value is absent in a trait.

Read through the handle `Const` (see [Reflection](reflection.md#const)).

| Field | Type | Meaning |
| --- | --- | --- |
| `ty` | `TypeId` | The declared type. |
| `value` | `Option<ExprId>` | The value. |

### `FnData`

*struct* in `src/pom/decl.rs`.

`fn name<..>(params) -> ret where cond { body }`.

Read through the handle `Function` (see [Reflection](reflection.md#function)).

| Field | Type | Meaning |
| --- | --- | --- |
| `is_const` | `bool` | Whether the function is `const`. |
| `abi` | `Option<Lit>` | `extern "C" fn`: the ABI string. |
| `ret` | `Option<TypeId>` | The `-> ty`. |
| `where_` | `Option<ExprId>` | The `where` condition. |
| `body` | `Option<BlockNode>` | None for a declaration ending in `;`. |

### `FnParamData`

*enum* in `src/pom/decl.rs`.

One parameter of a function.

Read through the handle `FnParam` (see [Reflection](reflection.md#fnparam)).

| Variant | Payload | Meaning |
| --- | --- | --- |
| `SelfValue` | — | `self` |
| `Named` | `{ ty: TypeId }` | A parameter `name: ty`. |
| `Error` | — | A malformed parameter already reported by a diagnostic. |

### `FieldsForm`

*enum* in `src/pom/decl.rs`.

How a struct or a variant lists its fields.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Named` | — | `{ a: T }` |
| `Tuple` | — | `(T, U)` |
| `Unit` | — | no fields |

### `StructData`

*struct* in `src/pom/decl.rs`.

`struct S<..> { fields }`; the fields are children after the generics.

Read through the handle `Struct` (see [Reflection](reflection.md#struct)).

| Field | Type | Meaning |
| --- | --- | --- |
| `form` | `FieldsForm` | How the fields are listed. |
| `bundle` | `Option<bool>` | Classification slot: none until the classification pass sets it. |

### `FieldData`

*struct* in `src/pom/decl.rs`.

`dir name: ty = init`; the name is absent in a tuple field.

Read through the handle `Field` (see [Reflection](reflection.md#field)).

| Field | Type | Meaning |
| --- | --- | --- |
| `dir` | `Option<Dir>` | The direction written before the name. |
| `ty` | `TypeId` | The declared type. |
| `init` | `Option<ExprId>` | The `= value` after the type of an `out` field of a bundle: the initial value of the carrier the field becomes at a producer (§7 rule 10). |

### `EnumData`

*struct* in `src/pom/decl.rs`.

`enum E<..> { variants }`; the variants are children after the generics.

Read through the handle `Enum` (see [Reflection](reflection.md#enum)).

### `VariantData`

*struct* in `src/pom/decl.rs`.

`Name`, `Name(T)`, `Name { f: T }`, with an optional `= discriminant`; the fields are children.

Read through the handle `Variant` (see [Reflection](reflection.md#variant)).

| Field | Type | Meaning |
| --- | --- | --- |
| `form` | `FieldsForm` | How the fields are listed. |
| `discriminant` | `Option<ExprId>` | The `= expr` value. |

### `TraitData`

*struct* in `src/pom/decl.rs`.

`trait Name<..>: Super { items }`; generics and items are children.

Read through the handle `Trait` (see [Reflection](reflection.md#trait)).

| Field | Type | Meaning |
| --- | --- | --- |
| `supertraits` | `Vec<PathId>` | The `: Super` bounds. |

### `ImplData`

*struct* in `src/pom/decl.rs`.

`impl<..> Trait for Type { items }`; generics and items are children.

Read through the handle `Impl` (see [Reflection](reflection.md#impl)).

| Field | Type | Meaning |
| --- | --- | --- |
| `trait_` | `Option<PathId>` | The trait of `impl Trait for Type`; none for an inherent impl. |
| `self_ty` | `TypeId` | The type the impl is for. |

### `GenericParamData`

*enum* in `src/pom/decl.rs`.

`<T: Bound = Default>` or `<const N: u32 = 4>`.

Read through the handle `GenericParam` (see [Reflection](reflection.md#genericparam)).

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Type` | `{ bounds: Vec<PathId>, default: Option<TypeId> }` | `T: Bound = Default`. |
| `Const` | `{ ty: TypeId, default: Option<ExprId> }` | `const N: ty = default`. |

### `NatureData`

*struct* in `src/pom/decl.rs`.

`nature Name<..> { potential: Voltage { .. }, flow: Current { .. }, value: T }`; generics and quantities are children.

Read through the handle `Nature` (see [Reflection](reflection.md#nature)).

| Field | Type | Meaning |
| --- | --- | --- |
| `fields` | `Vec<NatureFieldNode>` | The fields in source order, repeated and mixed ones included. |
| `kind` | `NatureKind` | Computed from the declared fields. |

### `NatureFieldNode`

*enum* in `src/pom/decl.rs`.

One field of a nature, as written.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Quantity` | `(DeclId)` | `potential: Voltage { .. }` or `flow: ..`: the `Quantity` declaration. |
| `Value` | `{ meta: Meta, ty: TypeId, span: Span }` | `value: T`; `span` covers the whole field; `meta` holds its docs and attributes (a quantity keeps them on its declaration). |

### `QuantityData`

*struct* in `src/pom/decl.rs`.

`potential: Voltage { access: v, units: "V" }`: the attribute list as written.

Read through the handle `Quantity` (see [Reflection](reflection.md#quantity)).

| Field | Type | Meaning |
| --- | --- | --- |
| `role` | `QuantityRole` | `Potential` or `Flow`. |
| `attrs` | `Vec<FieldInit>` | The `{ access: v, units: "V", abstol: 1u }` entries as written. |

### `ModuleData`

*struct* in `src/pom/decl.rs`.

`[extern "abi"] module Name<..>(ports) -> ret where cond { items result }`; generics, ports and body items are children, in that order.

Read through the handle `Module` (see [Reflection](reflection.md#module)).

| Field | Type | Meaning |
| --- | --- | --- |
| `abi` | `Option<Lit>` | The ABI string of `extern "abi" module`. |
| `ret` | `Option<TypeId>` | The `-> ty` result type. |
| `where_` | `Option<ExprId>` | The `where` condition. |
| `result` | `Option<ExprId>` | The trailing expression: the module's result. |
| `behaviors` | `Vec<DeclId>` | The behavior blocks bound to this module, in source order; filled by name resolution. |
| `instances` | `Vec<ExprId>` | Every call of this module's body and blocks classified as an instance, in source order; filled by classification. An instance is a view over the call, not a node. |

### `PortData`

*struct* in `src/pom/decl.rs`.

`dir name: ty = default`.

Read through the handle `Port` (see [Reflection](reflection.md#port)).

| Field | Type | Meaning |
| --- | --- | --- |
| `dir` | `Option<Dir>` | The direction word (`in`, `out`, `inout`) when written. |
| `ty` | `TypeId` | The declared type. |
| `default` | `Option<ExprId>` | The default expression. |
| `kind` | `PortKind` | Classification slot: unknown until the classification pass sets it. |

### `ParamData`

*struct* in `src/pom/decl.rs`.

`name: ty = default where cond;`

Read through the handle `Param` (see [Reflection](reflection.md#param)).

| Field | Type | Meaning |
| --- | --- | --- |
| `ty` | `TypeId` | The declared type. |
| `default` | `Option<ExprId>` | The default; a param with none is required. |
| `where_` | `Option<ExprId>` | The `where` condition. |

### `SignalData`

*struct* in `src/pom/decl.rs`.

`signal name: ty = init;`

Read through the handle `Signal` (see [Reflection](reflection.md#signal)).

| Field | Type | Meaning |
| --- | --- | --- |
| `ty` | `TypeId` | The declared type. |
| `init` | `Option<ExprId>` | The initial value. |

### `LetData`

*struct* in `src/pom/decl.rs`.

`let pat = value;` in a module body; the bindings of the pattern are children.

Read through the handle `Let` (see [Reflection](reflection.md#let)).

| Field | Type | Meaning |
| --- | --- | --- |
| `pat` | `PatId` | The pattern. |
| `value` | `ExprId` | The whole right-hand side. |

### `BindingData`

*struct* in `src/pom/decl.rs`.

One identifier a pattern introduces.

Read through the handle `Binding` (see [Reflection](reflection.md#binding)).

| Field | Type | Meaning |
| --- | --- | --- |
| `pat` | `PatId` | The pattern that introduces it (the whole struct pattern for the shorthand `S { x }`). |
| `origin` | `BindingOrigin` | The syntactic place the binding comes from. |

### `BindingOrigin`

*enum* in `src/pom/decl.rs`.

The syntactic place a binding comes from.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `BodyLet` | — | A module-body `let`. |
| `Local` | — | A `let` statement in a block. |
| `ClosureParam` | — | A closure parameter. |
| `MatchArm` | — | A match arm pattern. |
| `For` | — | A `for` loop pattern in a body. |
| `IfLet` | — | `if let` / `while let`. |
| `GenFor` | — | The pattern of a generation `for`. |

### `DefinitionData`

*struct* in `src/pom/decl.rs`.

`place = value after delay;`

Read through the handle `Definition` (see [Reflection](reflection.md#definition)).

| Field | Type | Meaning |
| --- | --- | --- |
| `place` | `ExprId` | The assigned place. |
| `value` | `ExprId` | The value. |
| `after` | `Option<ExprId>` | The `after` delay. |

### `GenerationData`

*enum* in `src/pom/decl.rs`.

`for pat in iter { items }` or `if cond { items } else ..` over module or digital items, held as written: nothing is unrolled. The items are children.

Read through the handle `Generation` (see [Reflection](reflection.md#generation)).

| Variant | Payload | Meaning |
| --- | --- | --- |
| `For` | `{ pat: PatId, iter: ExprId, body: Vec<DeclId> }` | The bindings of `pat` are children before the items. |
| `If` | `{ cond: ExprId, then: Vec<DeclId>, else_: Option<GenElseNode> }` | `if cond { items } else ..`. |

### `GenElseNode`

*enum* in `src/pom/decl.rs`.

What follows `else` in a generation `if`.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Block` | `(Vec<DeclId>)` | `else { items }`. |
| `If` | `(DeclId)` | `else if ..`: the nested `Generation`, a child of this one. |

### `BehaviorData`

*struct* in `src/pom/decl.rs`.

`analog<N> for Target { .. }` or `digital for Target { .. }`; an analog block's bindings and items, a digital block's reactions and generations, are children.

Read through the handle `Behavior` (see [Reflection](reflection.md#behavior)).

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | `BehaviorKind` | `Analog` or `Digital`. |
| `names` | `Vec<Name>` | The module names written after `for`. |
| `target` | `PathId` | The path written after `for`. |
| `body` | `Vec<StmtId>` | The statements of an `analog` block; empty for `digital`. |
| `module` | `Option<DeclId>` | The module of the same file the target names; filled by name resolution, none when the target names no usable module. |

### `ReactionData`

*struct* in `src/pom/decl.rs`.

`on event { ... }`

Read through the handle `Reaction` (see [Reflection](reflection.md#reaction)).

| Field | Type | Meaning |
| --- | --- | --- |
| `event` | `ExprId` | The event expression. |
| `body` | `BlockNode` | The block. |

## Body nodes

Expressions, statements, patterns, types and paths live in their own arenas. A node names its children by id and its owner by `DeclId`. Nothing is desugared: one node kind per syntax-tree variant.

### `PathNode`

*struct* in `src/pom/body.rs`.

`a::b<T>::c`. A path's resolution is its last segment's.

| Field | Type | Meaning |
| --- | --- | --- |
| `segments` | `Vec<PathSegment>` | The segments in order; `self`, `Self`, `super` and `crate` are segments too. |
| `span` | `Span` | Byte range in the file text. |
| `owner` | `DeclId` | The nearest enclosing declaration; ties the node to the authored tree. |

### `PathSegment`

*struct* in `src/pom/body.rs`.

One segment; `self`, `Self`, `super` and `crate` are segments too.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `Name` | The identifier of the segment. |
| `args` | `Option<GenericArgsNode>` | The `<..>` arguments written on this segment. |
| `res` | `Resolution` | What the name refers to (`Unresolved` until name resolution). |

### `GenericArgsNode`

*enum* in `src/pom/body.rs`.

The generic arguments of one path segment.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Angle` | `{ args: Vec<GenericArgNode>, turbofish: bool, span: Span }` | `<A, B>` or the turbofish `::<A, B>`. |
| `Paren` | `{ inputs: Vec<TypeId>, output: Option<TypeId>, span: Span }` | The `Fn(P) -> R` bound sugar. |

### `GenericArgNode`

*enum* in `src/pom/body.rs`.

One generic argument.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Type` | `(TypeId)` | A type argument. |
| `Const` | `(ExprId)` | A const argument: a literal or a `{ block }`. |

### `TypeNode`

*struct* in `src/pom/body.rs`.

A type expression node. Types live in their own arena (`TypeId`).

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | `TypeKind` | What the type is. |
| `span` | `Span` | Byte range in the file text. |
| `owner` | `DeclId` | The nearest enclosing declaration; ties the node to the authored tree. |

### `TypeKind`

*enum* in `src/pom/body.rs`.

The forms of a type expression.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Path` | `(PathId)` | A named type, `a::T<U>`. |
| `Array` | `{ elem: TypeId, len: ExprId }` | `[T; N]`: `len` is a const expression. |
| `Tuple` | `(Vec<TypeId>)` | `(A, B)`; the empty tuple is `()`. |
| `Flip` | `(TypeId)` | `flip T` |
| `FnPtr` | `{ params: Vec<TypeId>, ret: Option<TypeId> }` | `fn(A, B) -> R`; `ret` is `None` for `fn()`. |
| `Infer` | — | `_` |

### `PatNode`

*struct* in `src/pom/body.rs`.

A pattern node. Patterns live in their own arena (`PatId`).

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | `PatKind` | What the pattern is. |
| `span` | `Span` | Byte range in the file text. |
| `owner` | `DeclId` | The nearest enclosing declaration; ties the node to the authored tree. |

### `PatKind`

*enum* in `src/pom/body.rs`.

The forms of a pattern.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Wild` | — | `_` |
| `Rest` | — | `..` |
| `Ident` | `{ name: Name, sub: Option<PatId>, binding: DeclId, res: Resolution }` | `x`, `x @ p`. `binding` is the `Binding` declaration the identifier introduces (the lowering cannot tell a binder from a constant name; resolution can). `res` is what a plain name resolves to when it names a constant, a unit variant, a unit struct or a built-in variant but `Some`: the pattern is a path then and binds nothing; it stays `Unresolved` for a binder. |
| `Lit` | `(ExprId)` | A literal, possibly negated. |
| `Path` | `(PathId)` | A path naming a constant or a unit variant. |
| `Tuple` | `(Vec<PatId>)` | `(a, b)` |
| `TupleStruct` | `{ path: PathId, pats: Vec<PatId> }` | `Path(a, b)` |
| `Struct` | `{ path: PathId, fields: Vec<FieldPatNode>, rest: bool }` | `Path { a, b: p, .. }`; `rest` is the trailing `..`. |
| `Or` | `(Vec<PatId>)` | `a \| b` |
| `Range` | `{ lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool }` | `lo..hi` or `lo..=hi`. |
| `Array` | `(Vec<PatId>)` | `[a, b]`, `[first, .., last]` |

### `FieldPatNode`

*struct* in `src/pom/body.rs`.

`name: pat` in a struct pattern. The shorthand `Point { x }` has no `pat` and introduces the `Binding` named by `binding`.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `Name` | The field name. |
| `pat` | `Option<PatId>` | The sub-pattern; none in the shorthand. |
| `binding` | `Option<DeclId>` | The `Binding` the shorthand introduces. |
| `res` | `Resolution` | The field of the struct or variant the name stands for; `Unresolved` when it has none. |
| `span` | `Span` | Byte range in the file text. |

### `ExprNode`

*struct* in `src/pom/body.rs`.

An expression node. Expressions live in their own arena (`ExprId`); children are ids.

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | `ExprKind` | What the expression is. |
| `span` | `Span` | Byte range in the file text. |
| `owner` | `DeclId` | The nearest enclosing declaration; ties the node to the authored tree. |

### `Arg`

*struct* in `src/pom/body.rs`.

An argument: positional, or `name: value` after the positional ones. The name links to the port, param or field it names once the callee is classified.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `Option<Name>` | The `name:` of a named argument. |
| `value` | `ExprId` | The argument expression. |
| `res` | `Resolution` | What the name refers to (`Unresolved` until name resolution). |
| `span` | `Span` | Byte range in the file text. |

### `FieldInit`

*struct* in `src/pom/body.rs`.

`name: value` in a struct expression, a template or an instance's param block.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `Name` | The field, param or port name. |
| `value` | `ExprId` | The value expression. |
| `shorthand` | `bool` | Written `{ r }` for `{ r: r }` (`is_shorthand()`); `to_ast` restores the form. |
| `res` | `Resolution` | What the name refers to (`Unresolved` until name resolution). |
| `span` | `Span` | Byte range in the file text. |

### `Arm`

*struct* in `src/pom/body.rs`.

One arm of a `match`: `#[attrs] pat if guard => body`.

| Field | Type | Meaning |
| --- | --- | --- |
| `meta` | `Meta` | Docs and attributes of the arm. |
| `pat` | `PatId` | The pattern. |
| `guard` | `Option<ExprId>` | The `if` guard. |
| `body` | `ExprId` | The arm expression. |
| `span` | `Span` | Byte range in the file text. |

### `ClosureParam`

*struct* in `src/pom/body.rs`.

One parameter of a closure: `pat: ty`.

| Field | Type | Meaning |
| --- | --- | --- |
| `pat` | `PatId` | The parameter pattern. |
| `ty` | `Option<TypeId>` | The annotated type. |
| `span` | `Span` | Byte range in the file text. |

### `BlockNode`

*struct* in `src/pom/body.rs`.

`{ stmts }`, held by value by the node that owns it.

| Field | Type | Meaning |
| --- | --- | --- |
| `stmts` | `Vec<StmtId>` | The statements in order. |
| `span` | `Span` | Byte range of the braces and everything between them. |

### `ExprKind`

*enum* in `src/pom/body.rs`.

One variant per `ast::ExprKind` variant.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Lit` | `(Lit)` | A literal. |
| `Path` | `(PathId)` | A path used as a value: `x`, `a::b`. |
| `Unary` | `{ op: UnOp, expr: ExprId }` | `-x`, `!x`, `*x` ... |
| `Binary` | `{ op: BinOp, lhs: ExprId, rhs: ExprId }` | `a + b` and every other binary operator. |
| `Assign` | `{ op: Option<BinOp>, lhs: ExprId, rhs: ExprId }` | `lhs = rhs` (`op: None`) or `lhs += rhs`. |
| `Cast` | `{ expr: ExprId, ty: TypeId }` | `expr as ty`. |
| `Call` | `{ callee: ExprId, args: Vec<Arg>, params: Option<Vec<FieldInit>>, class: CallClass }` | `params` is the `{ name: value }` block of an instance. |
| `MethodCall` | `{ recv: ExprId, method: Name, generics: Option<GenericArgsNode>, args: Vec<Arg>, res: Resolution }` | `recv.method::<G>(args)`; `res` is the member the resolver found. |
| `Field` | `{ expr: ExprId, name: Name, res: Resolution }` | `e.name`; a tuple index `e.0` has the digits as its name. |
| `Index` | `{ expr: ExprId, index: ExprId }` | `expr[index]`. |
| `Range` | `{ lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool }` | `lo..hi`, `lo..=hi`, `..hi`, `lo..`. |
| `Paren` | `(ExprId)` | `(expr)`; parentheses are kept. |
| `Tuple` | `(Vec<ExprId>)` | `(a, b)`. |
| `Array` | `(Vec<ExprId>)` | `[a, b]`. |
| `ArrayRepeat` | `{ elem: ExprId, len: ExprId }` | `[elem; len]`. |
| `Struct` | `{ path: PathId, fields: Vec<FieldInit>, class: StructClass }` | `Path { f: v }`: a struct literal, a bundle literal or a template (see `StructClass`). |
| `Block` | `(BlockNode)` | `{ stmts }`. |
| `If` | `{ cond: ExprId, then: BlockNode, else_: Option<ExprId> }` | `if cond { then } else ..`; `else_` is a block expression or a nested `if`. |
| `Let` | `{ pat: PatId, expr: ExprId }` | `let pat = expr` in a condition. |
| `Match` | `{ scrutinee: ExprId, arms: Vec<Arm> }` | `match scrutinee { arms }`. |
| `While` | `{ cond: ExprId, body: BlockNode }` | `while cond { body }`. |
| `Loop` | `{ body: BlockNode }` | `loop { body }`. |
| `For` | `{ pat: PatId, iter: ExprId, body: BlockNode }` | `for pat in iter { body }`. |
| `Break` | `(Option<ExprId>)` | `break value?`. |
| `Continue` | — | `continue`. |
| `Return` | `(Option<ExprId>)` | `return value?`. |
| `Closure` | `{ params: Vec<ClosureParam>, ret: Option<TypeId>, body: ExprId }` | `\|p: T\| -> R body`. |
| `Try` | `(ExprId)` | `e?` |
| `Emit` | `{ stream: Lit, fields: Vec<FieldInit> }` | `emit "stream" { fields }`. |
| `IncludeStr` | `(Lit)` | `include_str!("file")`. |
| `Error` | — | A malformed expression already reported by a diagnostic. |

### `StmtNode`

*struct* in `src/pom/body.rs`.

A statement node. Statements live in their own arena (`StmtId`).

| Field | Type | Meaning |
| --- | --- | --- |
| `meta` | `Meta` | The outer doc comments and attributes written before the statement, in order (an item statement keeps its own on the item). |
| `kind` | `StmtKind` | What the statement is. |
| `span` | `Span` | Byte range in the file text. |
| `owner` | `DeclId` | The nearest enclosing declaration; ties the node to the authored tree. |

### `StmtKind`

*enum* in `src/pom/body.rs`.

The forms of a statement.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Let` | `{ pat: PatId, ty: Option<TypeId>, init: Option<ExprId>, else_: Option<BlockNode> }` | `let pat: ty = init else { diverge };` |
| `Expr` | `{ expr: ExprId, semi: bool }` | `semi: false` on the last statement of a block is the block's value. |
| `Item` | `(DeclId)` | An item written inside a block; the declaration is a child of the enclosing declaration. |
| `Contribution` | `{ lhs: ExprId, rhs: ExprId }` | `lhs +> rhs;` |
| `Commit` | `{ place: ExprId, value: ExprId, after: Option<ExprId> }` | `place <- value [after delay];` |
| `Empty` | — | A lone `;`. |
| `Signal` | `(DeclId)` | `signal name: type = init;`: the `Signal` declaration. |
| `Reaction` | `(DeclId)` | `on event { ... }`: the `Reaction` declaration. |

## Resolution slots

Every slot that name resolution writes starts in its first state (`Unresolved`, `Unclassified`, `Unknown`). Resolution only fills these slots and appends diagnostics; it never adds, removes or moves a node.

### `Resolution`

*enum* in `src/pom/resolution.rs`.

What a name refers to.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Unresolved` | — | Not looked at yet, or nothing has that name (then `E0161`). |
| `Decl` | `(DeclId)` | A declaration of this design. |
| `Builtin` | `(BuiltinId)` | An entry of the built-in table. |
| `Deferred` | `(DeferReason)` | Needs type inference; no diagnostic. |

### `DeferReason`

*enum* in `src/pom/resolution.rs`.

Why a member was left `Deferred` instead of resolved.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `ReceiverTypeUnknown` | — | The receiver type cannot be read from a declaration. |
| `AmbiguousAccess` | — | An access call (`v(x)`) whose access name several natures declare and the argument does not decide. |
| `InferredGenerics` | — | Reserved: needs inference of generic arguments. The resolver does not produce it yet. |
| `BuiltinTraitMember` | — | An associated name written through a generic bounded by a catalogue trait of Appendix H (`T: PartialOrd`, `T::foo`). |

### `CallClass`

*enum* in `src/pom/resolution.rs`.

What a call expression is.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Unclassified` | — | Not classified (before resolution, or in a place it does not run). |
| `Function` | — | A call of a function. |
| `Instance` | `{ module: Option<DeclId>, via: Option<DeclId> }` | A module instantiation; `via` is the template it was written through. |
| `Probe` | — | A call whose callee is a nature quantity (`v(p, n)`) or the built-in `potential` / `flow`. |

### `StructClass`

*enum* in `src/pom/resolution.rs`.

What a `Path { ... }` expression is.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Unclassified` | — | Not classified. |
| `StructLiteral` | — | A literal of a struct whose fields are values. |
| `BundleLiteral` | — | A literal of a bundle (a struct with carrier fields). |
| `Template` | `(DeclId)` | A module template; the id is the template declaration. |

### `PortKind`

*enum* in `src/pom/resolution.rs`.

The kind of a module port, from its declared type.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Unknown` | — | Not classified. |
| `Carrier` | — | The port type resolves to a nature (a carrier of that nature). |
| `Bundle` | — | The port type resolves to a bundle struct. |
| `ValueInput` | — | The port type is a value type (a built-in or a declared non-bundle type): the port carries a value, not a signal. |

### `BundleView`

*enum* in `src/pom/bundles.rs`.

The way one module sees a bundle (spec part II §7): `Port::bundle_view` reads it from the port as written. The checker and the elaborator
read the fields the port stands for from a side table of the design, never from `children`.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Producer` | — | `tx: Stream`: drives the `out` fields, reads the `in` fields. |
| `Consumer` | — | `rx: flip Stream`: every direction of a one-driver field reversed. |
| `Observer` | — | `in s: Stream`: every one-driver field an input. |

### `NatureKind`

*enum* in `src/pom/resolution.rs`.

The kind of a nature, from the fields it declares.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Discrete` | — | Only a `value` field (resolved when an `impl Resolve` is written for it). |
| `Conservative` | — | `potential` and `flow`. |
| `SignalFlow` | — | `potential` only. |
| `Invalid` | — | Any other combination. |

## Documentation and attributes

What precedes an item: doc comments and attributes, kept in order and verbatim.

### `Doc`

*struct* in `src/pom/meta.rs`.

A doc comment: the text after the marker, or between the markers of a block comment, verbatim.

| Field | Type | Meaning |
| --- | --- | --- |
| `text` | `String` | The text after the marker (or between the block markers), verbatim. |
| `block` | `bool` | Whether it is a block comment. |
| `inner` | `bool` | Whether it was written `//!` or `/*! */`: it documents the namespace around it. |
| `file` | `FileId` | The file the comment was written in; `span` is an offset in that file. |
| `span` | `Span` | Byte range of the comment. |

### `Meta`

*struct* in `src/pom/meta.rs`.

What precedes an item, field, port, variant or match arm, in order.

| Field | Type | Meaning |
| --- | --- | --- |
| `docs` | `Vec<Doc>` | The doc comments in order. |
| `attrs` | `Vec<AttrNode>` | The attributes in order. |

### `AttrNode`

*struct* in `src/pom/meta.rs`.

`#[path]`, `#[path(args)]` or `#[path = expr]`. The path is stored as written and is not resolved.

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | `PathId` | The attribute path as written. |
| `input` | `AttrInputNode` | Its arguments. |
| `span` | `Span` | Byte range in the file text. |

### `AttrInputNode`

*enum* in `src/pom/meta.rs`.

The arguments of an attribute.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `None` | — | `#[path]`. |
| `Eq` | `(ExprId)` | `#[doc = "text"]` |
| `List` | `(Vec<AttrArgNode>)` | `#[derive(Repr, PartialEq)]`, `#[link("x.osdi", model = "bsim4")]` |

### `AttrArgNode`

*enum* in `src/pom/meta.rs`.

One argument inside an attribute list.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Path` | `(PathId)` | A path argument. |
| `Lit` | `(Lit)` | A literal argument. |
| `NameValue` | `(Name, Lit)` | `name = lit`. |

## Reverse index

Tables computed once after resolution: where each declaration is named, who writes it, who instantiates it. They are read through `references()`, `writers()` and `instantiated_by()`.

### `ReferenceId`

*enum* in `src/pom/index.rs`.

A node that names a declaration.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Path` | `(PathId)` | A path with a segment that resolved to the declaration. |
| `Member` | `(ExprId)` | A `.name` or `.method()` expression that resolved to it. |
| `Pattern` | `(PatId)` | An identifier pattern that names it (a constant, a unit variant, a unit struct). |

### `WriterId`

*enum* in `src/pom/index.rs`.

A statement or item that writes a place.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Definition` | `(DeclId)` | `place = value;` in a module body. |
| `Commit` | `(StmtId)` | `place <- value;` in a reaction. |

### `Index`

*struct* in `src/pom/index.rs`.

The three reverse tables, each indexed by `DeclId`. Read them through `Reflect::references`, `Signal::writers`, `Port::writers` and `Module::instantiated_by`.

| Storage (private) | Type | Meaning |
| --- | --- | --- |
| `references` | `Vec<Vec<ReferenceId>>` | Per declaration: the paths and members that name it, in source order. |
| `writers` | `Vec<Vec<WriterId>>` | Per declaration: the definitions and commits whose place is rooted at it or ends in it as a member of an instance (`c.w`; the instance's own name is then not written), in source order. |
| `instantiated_by` | `Vec<Vec<ExprId>>` | Per module: the instance calls that place it. |

## The built-in table

The closed table of names the language provides without a declaration.

### `BuiltinKind`

*enum* in `src/pom/builtin.rs`.

What a built-in is.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Type` | — | A primitive or built-in type. |
| `Trait` | — | A built-in bound. |
| `Variant` | — | A built-in enum variant (`Some`, `Transient`, ...). |
| `Fn` | — | A built-in function. |
| `Module` | — | A built-in module placed as an instance (`ddt`, `delay`, `limit`, `noise`, `cross`, `at`). |
| `Method` | — | A built-in method (`changed`, `prev`, `others`, `inflow`); reached only as a member, never as a name. |
| `Event` | — | A built-in event (`settle`). |
| `Statement` | — | A built-in statement (`bound_step`). |
| `Node` | — | A reference node (`gnd`). |
| `Member` | — | An associated item of a built-in trait (`Repr::WIDTH`, `Repr::encode`, `Repr::decode`); reached only through a generic bounded by the trait (`builtin_member`), never as a name. |

### `BuiltinInfo`

*struct* in `src/pom/builtin.rs`.

One row of `BUILTINS`.

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `&'static str` | The spelling; `uN`, `iN` and `fN` stand for every width of the family. |
| `kind` | `BuiltinKind` | What the built-in is. |
| `section` | `&'static str` | The spec section that defines it. |

### `BuiltinId`

*struct* in `src/pom/builtin.rs`.

An entry of the built-in table. The low byte is the row of `BUILTINS`; the high byte is the width of a `uN`, `iN` or `fN` type (0 otherwise).

| Field | Type | Meaning |
| --- | --- | --- |
| `0` | `u16` | The row index. |

## Handles

The reflection handles are `Copy` views over a `&Design`: a pair of the design and one id, equal exactly when they come from the same design and name the same node. Their methods are listed in [Reflection](reflection.md); this section records only what each handle is.

### `SourceFile`

*struct* in `src/pom/reflect/body.rs`. A source file of a `Design`: its name, text and retained syntax tree.

Methods: [Reflection](reflection.md#sourcefile).

### `Expr`

*struct* in `src/pom/reflect/body.rs`. An expression node.

Methods: [Reflection](reflection.md#expr).

### `Stmt`

*struct* in `src/pom/reflect/body.rs`. A statement node.

Methods: [Reflection](reflection.md#stmt).

### `Pat`

*struct* in `src/pom/reflect/body.rs`. A pattern node.

Methods: [Reflection](reflection.md#pat).

### `Type`

*struct* in `src/pom/reflect/body.rs`. A type node.

Methods: [Reflection](reflection.md#type).

### `Path`

*struct* in `src/pom/reflect/body.rs`. A path node.

Methods: [Reflection](reflection.md#path).

### `Contribution`

*struct* in `src/pom/reflect/body.rs`. `lhs +> rhs;`

Methods: [Reflection](reflection.md#contribution).

### `Commit`

*struct* in `src/pom/reflect/body.rs`. `place <- value [after delay];`

Methods: [Reflection](reflection.md#commit).

### `Nature`

*struct* in `src/pom/reflect/hardware.rs`. `nature Name { potential: .., flow: .., value: .. }`

Methods: [Reflection](reflection.md#nature).

### `Quantity`

*struct* in `src/pom/reflect/hardware.rs`. `potential: Voltage { access: v, units: "V", abstol: 1u }`

Methods: [Reflection](reflection.md#quantity).

### `Module`

*struct* in `src/pom/reflect/hardware.rs`. `[extern "abi"] module Name<..>(ports) -> ret where cond { body }`

Methods: [Reflection](reflection.md#module).

### `Port`

*struct* in `src/pom/reflect/hardware.rs`. One port of a module.

Methods: [Reflection](reflection.md#port).

### `Param`

*struct* in `src/pom/reflect/hardware.rs`. `name: ty = default where cond;` in a module body.

Methods: [Reflection](reflection.md#param).

### `Signal`

*struct* in `src/pom/reflect/hardware.rs`. `signal name: ty = init;`

Methods: [Reflection](reflection.md#signal).

### `Let`

*struct* in `src/pom/reflect/hardware.rs`. `let pat = value;` in a module body.

Methods: [Reflection](reflection.md#let).

### `Binding`

*struct* in `src/pom/reflect/hardware.rs`. One identifier a pattern introduces.

Methods: [Reflection](reflection.md#binding).

### `Definition`

*struct* in `src/pom/reflect/hardware.rs`. `place = value after delay;`

Methods: [Reflection](reflection.md#definition).

### `Generation`

*struct* in `src/pom/reflect/hardware.rs`. A generation `for` or `if` over module or digital items.

Methods: [Reflection](reflection.md#generation).

### `GenArm`

*struct* in `src/pom/reflect/hardware.rs`. One arm of a generation `if` chain.

Methods: [Reflection](reflection.md#genarm).

### `Behavior`

*struct* in `src/pom/reflect/hardware.rs`. `analog<N> for Target { .. }` or `digital for Target { .. }`

Methods: [Reflection](reflection.md#behavior).

### `Reaction`

*struct* in `src/pom/reflect/hardware.rs`. `on event { .. }`

Methods: [Reflection](reflection.md#reaction).

### `Instance`

*struct* in `src/pom/reflect/hardware.rs`. A placement: a view over a call expression classified as an instance. It is not a declaration and not a node of its own.

Methods: [Reflection](reflection.md#instance).

### `Reference`

*enum* in `src/pom/reflect/index.rs`.

A node that names a declaration: see `Reflect::references`.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Path` | `(Path<'d>)` | A path (a segment of it resolved to the declaration). |
| `Member` | `(Expr<'d>)` | A `.name` or `.method()` expression. |

### `Writer`

*enum* in `src/pom/reflect/index.rs`.

Something that writes a place: see `Signal::writers`.

| Variant | Payload | Meaning |
| --- | --- | --- |
| `Definition` | `(Definition<'d>)` | `place = value;` in a module body. |
| `Commit` | `(Commit<'d>)` | `place <- value;` in a reaction. |

### `Const`

*struct* in `src/pom/reflect/items.rs`. `const NAME: ty = value;`

Methods: [Reflection](reflection.md#const).

### `Function`

*struct* in `src/pom/reflect/items.rs`. `fn name<..>(params) -> ret where cond { body }`

Methods: [Reflection](reflection.md#function).

### `FnParam`

*struct* in `src/pom/reflect/items.rs`. One parameter of a function.

Methods: [Reflection](reflection.md#fnparam).

### `GenericParam`

*struct* in `src/pom/reflect/items.rs`. `<T: Bound = Default>` or `<const N: u32 = 4>`.

Methods: [Reflection](reflection.md#genericparam).

### `Struct`

*struct* in `src/pom/reflect/items.rs`. `struct S<..> { fields }`

Methods: [Reflection](reflection.md#struct).

### `Field`

*struct* in `src/pom/reflect/items.rs`. A field of a struct or of a variant.

Methods: [Reflection](reflection.md#field).

### `Enum`

*struct* in `src/pom/reflect/items.rs`. `enum E<..> { variants }`

Methods: [Reflection](reflection.md#enum).

### `Variant`

*struct* in `src/pom/reflect/items.rs`. A variant of an enum.

Methods: [Reflection](reflection.md#variant).

### `Trait`

*struct* in `src/pom/reflect/items.rs`. `trait Name<..>: Super { items }`

Methods: [Reflection](reflection.md#trait).

### `Impl`

*struct* in `src/pom/reflect/items.rs`. `impl<..> Trait for Type { items }`

Methods: [Reflection](reflection.md#impl).

### `Use`

*struct* in `src/pom/reflect/items.rs`. `use a::{b, c as d};`

Methods: [Reflection](reflection.md#use).

### `ImportRef`

*struct* in `src/pom/reflect/items.rs`. One flattened leaf of a `use` tree.

Methods: [Reflection](reflection.md#importref).

### `Namespace`

*struct* in `src/pom/reflect/items.rs`. A namespace: a file's top, `mod a { }` or `mod a;`.

Methods: [Reflection](reflection.md#namespace).

### `Decl`

*struct* in `src/pom/reflect/mod.rs`. The generic declaration handle: any kind, read through `Decl::kind`.

Methods: [Reflection](reflection.md#decl).

### `Reflect`

*trait* in `src/pom/reflect/mod.rs`. What every declaration handle answers. Implementors give `Reflect::decl`; the rest are plain provided methods.

Methods: [Reflection](reflection.md#reflect).

### `Attr`

*struct* in `src/pom/reflect/mod.rs`. An attribute: `#[path]`, `#[path(args)]` or `#[path = expr]`.

Methods: [Reflection](reflection.md#attr).

## Syntax types the POM re-exports

These come from the syntax tree and appear in POM payloads unchanged.

### `Lit`

*struct*. A literal with its position. Re-exported from the syntax tree (`msl::ast`).

Shape: `pub kind: LitKind`, `pub span: Loc`.

### `LitKind`

*enum*. The literal forms. `Int` keeps the text with its type suffix; `Float` keeps the SI-scaled value and the raw text (serialized as the raw text only: the value is recomputed from it on read, so `1e999` round-trips through JSON); `Str` keeps escapes unprocessed.

Shape: `Int(String)`, `Float { value: f64, raw: String }`, `Str(String)`, `Bool(bool)`.

### `UnOp`

*enum*. Unary operators.

Shape: `Neg`, `Not`.

### `BinOp`

*enum*. Binary operators.

Shape: `Add`, `Sub`, `Mul`, `Div`, `Rem`, `BitXor`, `BitAnd`, `BitOr`, `Shl`, `Shr`, `And`, `Or`, `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`.

### `Dir`

*enum*. A port or field direction.

Shape: `In`, `Out`.

### `BehaviorKind`

*enum*. Which behavior block a `Behavior` is.

Shape: `Analog`, `Digital`.

### `QuantityRole`

*enum*. The role of a nature quantity (the syntax tree's `Quantity`, renamed because the handle `Quantity` takes the name).

Shape: `Potential`, `Flow`.

### `Span`

*struct*. A byte range in a file's text. `range()` gives it as `start..end`.

Shape: `pub start: usize`, `pub end: usize`.

### `Diagnostic`

*struct*. A reported problem: a stable code (`E0161`), a message, a primary label and optional secondary labels and help. Defined by the parser crate module `msl` and carried unchanged by `DesignDiagnostic`.

Shape: `code`, `message`, `primary: (Span, String)`, `secondary: Vec<(Span, String)>`, `help: Option<String>`.

