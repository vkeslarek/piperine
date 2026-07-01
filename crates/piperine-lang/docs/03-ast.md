# Parse AST Structure

The parse AST is the output of the parser and the input to elaboration. It is
**intentionally unresolved** — types are plain strings, array dimensions are
arbitrary expressions, event names are unvalidated, and `use` declarations are
not yet expanded. All semantic validation is deferred to elaboration.

## Root type: `SourceFile`

```rust
pub struct SourceFile {
    pub items: Vec<Item>,
}
```

A parsed source file is a flat list of top-level items. Order is preserved but
the elaborator processes items in dependency order after a registration pass.

## `Item` enum

```rust
pub enum Item {
    UseDecl(Path),
    ModDecl(ModDecl),
    BehaviorDecl(BehaviorDecl),
    DisciplineDecl(DisciplineDecl),
    BundleDecl(BundleDecl),
    EnumDecl(EnumDecl),
    CapabilityDecl(CapabilityDecl),
    ImplDecl(ImplDecl),
    FnDecl(FnDecl),
}
```

## `Path`

```rust
pub struct Path {
    pub segments: Vec<String>,
}
```

A `::`-separated module path (e.g. `devices::passives::Resistor`).

---

## Modules

### `ModDecl`

```rust
pub struct ModDecl {
    pub is_pub: bool,
    pub name: String,
    pub const_params: Vec<String>,      // compile-time Natural params, e.g. [N]
    pub type_params: Vec<TypeParam>,    // generic type params, e.g. <T: Add>
    pub ports: Vec<Port>,
    pub body: Vec<ModStmt>,
}
```

`const_params` are compile-time Natural parameters (e.g. `[N]`). `type_params` are
generic type parameters with optional capability bounds. Both are unresolved at
parse time — the elaborator substitutes them at instantiation.

### `TypeParam`

```rust
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<String>,  // capability names, e.g. ["Add", "Net"]
}
```

### `Port`

```rust
pub struct Port {
    pub direction: Direction,
    pub name: String,
    pub ty: Type,              // unresolved type
}
```

### `Direction`

```rust
pub enum Direction { Input, Output, Inout }
```

### `ModStmt` enum

```rust
pub enum ModStmt {
    ParamDecl { name: String, ty: Type, default: Option<Expr> },
    WireDecl { name: String, ty: Type },
    VarDecl { name: String, ty: Type, default: Option<Expr> },
    StructuralFor { var: String, range: Range, body: Vec<ModStmt> },
    StructuralIf { cond: Expr, then_body: Vec<ModStmt>, else_body: Option<Vec<ModStmt>> },
    Instance {
        name: Option<String>,        // None for anonymous instances
        array_index: Option<Expr>,   // index on label, e.g. r[i]
        module: String,
        const_args: Vec<Expr>,       // e.g. [N] in Dac[N](...)
        type_args: Vec<Type>,
        ports: Vec<Expr>,
        params: Vec<ParamArg>,
    },
    Connection { lhs: Expr, rhs: Expr },
}
```

`StructuralFor` is unrolled at elaboration with concrete range bounds. `StructuralIf`
is evaluated at elaboration. `Instance` with `name: Some(n)` is a named instantiation
(`r: Resistor(...)`); `name: None` is anonymous.

### `ParamArg`

```rust
pub struct ParamArg {
    pub name: String,
    pub expr: Expr,
}
```

A named parameter override (`.name = expr`).

### `Range`

```rust
pub struct Range {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub inclusive: bool,
}
```

---

## Types

### `Type`

```rust
pub struct Type {
    pub name: String,           // type name, looked up at elaboration
    pub args: Vec<Type>,        // generic type arguments
    pub dimensions: Vec<Expr>,  // array extents (must eval to Natural at elaboration)
}
```

A syntactic type reference. `name` is resolved in the type namespace. `dimensions`
are array extents that must evaluate to `Natural` at elaboration time. The elaborator
replaces `Type` with either `NetType` or `ValueType`.

---

## Disciplines

### `DisciplineDecl`

```rust
pub struct DisciplineDecl {
    pub is_pub: bool,
    pub name: String,
    pub items: Vec<DisciplineItem>,
}
```

### `DisciplineItem`

```rust
pub enum DisciplineItem {
    Nature { kind: NatureKind, name: String, ty: Type, attrs: Vec<Attr> },
    Storage(Type),
    Resolve(ResolveKind),
}
```

### `NatureKind`

```rust
pub enum NatureKind { Potential, Flow }
```

### `ResolveKind`

```rust
pub enum ResolveKind { Tri, Or, And }
```

### `Attr`

```rust
pub struct Attr {
    pub name: String,
    pub expr: Expr,
}
```

A named attribute in a nature declaration (e.g. `unit = "V"`).

---

## Bundles

### `BundleDecl`

```rust
pub struct BundleDecl {
    pub is_pub: bool,
    pub name: String,
    pub const_params: Vec<String>,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<FieldDecl>,
}
```

A bundle is **net-capable** if every field resolves to a net type. Net-capable bundles
used as ports are expanded to flat fields by the elaborator.

### `FieldDecl`

```rust
pub struct FieldDecl {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,  // optional default for value-type bundles
}
```

---

## Enums

### `EnumDecl`

```rust
pub struct EnumDecl {
    pub is_pub: bool,
    pub name: String,
    pub repr: Option<Type>,           // optional explicit underlying type
    pub variants: Vec<EnumVariant>,
}
```

### `EnumVariant`

```rust
pub struct EnumVariant {
    pub name: String,
    pub value: Option<Expr>,  // explicit discriminant; auto-increments when absent
}
```

---

## Capabilities

### `CapabilityDecl`

```rust
pub struct CapabilityDecl {
    pub is_pub: bool,
    pub name: String,
    pub supers: Vec<String>,      // required super-capabilities
    pub items: Vec<CapItem>,
}
```

### `CapItem`

```rust
pub enum CapItem {
    FnSig(FnSig),    // abstract signature
    FnDecl(FnDecl),  // default body
}
```

### `ImplDecl`

```rust
pub struct ImplDecl {
    pub is_pub: bool,
    pub capability: Option<String>,  // Some for capability impls; None for inherent
    pub ty: String,
    pub const_args: Vec<Expr>,
    pub type_args: Vec<Type>,
    pub methods: Vec<FnDecl>,
}
```

---

## Functions

### `FnSig`

```rust
pub struct FnSig {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<FnParam>,
    pub ret: Type,
}
```

### `FnDecl`

```rust
pub struct FnDecl {
    pub is_pub: bool,
    pub sig: FnSig,
    pub body: Block,
}
```

### `FnParam`

```rust
pub enum FnParam {
    SelfParam,
    Typed(String, Type),
}
```

---

## Blocks and statements

### `Block`

```rust
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Box<Expr>>,  // trailing expression as block value (Rust-style)
}
```

The optional trailing expression (no semicolon) is the block's value.

### `Stmt` enum

```rust
pub enum Stmt {
    VarDecl { name: String, ty: Type, default: Option<Expr> },
    Return(Expr),
    If { cond: Expr, then_body: Block, else_body: Option<Block> },
    Match { expr: Expr, arms: Vec<StmtMatchArm> },
    For { var: String, range: Range, body: Block },
    Bind { dest: Expr, op: BindOp, src: Expr },
    Expr(Expr),
}
```

### `StmtMatchArm`

```rust
pub struct StmtMatchArm {
    pub pat: Pattern,
    pub body: Block,
}
```

---

## Behaviors

### `BehaviorDecl`

```rust
pub struct BehaviorDecl {
    pub is_pub: bool,
    pub kind: BehaviorKind,
    pub name: String,
    pub body: Vec<BehaviorStmt>,
}
```

### `BehaviorKind`

```rust
pub enum BehaviorKind { Analog, Digital }
```

### `BehaviorStmt` enum

```rust
pub enum BehaviorStmt {
    VarDecl { name: String, ty: Type, default: Option<Expr> },
    Bind { dest: Expr, op: BindOp, src: Expr },
    If { cond: Expr, then_body: Vec<BehaviorStmt>, else_body: Option<Vec<BehaviorStmt>> },
    Match { expr: Expr, arms: Vec<MatchArm> },
    For { var: String, range: Range, body: Vec<BehaviorStmt> },
    Event { spec: EventSpec, guard: Option<Expr>, body: Block },
    Diagnostic { sys: String, args: Vec<Expr> },
    Expr(Expr),
}
```

### `BindOp`

```rust
pub enum BindOp {
    Contrib,  // <+
    Force,    // <-
    Assign,   // =
}
```

### `MatchArm`

```rust
pub struct MatchArm {
    pub pat: Pattern,
    pub body: Vec<BehaviorStmt>,
}
```

### `Pattern`

```rust
pub enum Pattern {
    Path(Path),
    Wildcard,
}
```

---

## Events

### `EventSpec`

```rust
pub enum EventSpec {
    Named { name: String, arg: Expr },  // any identifier, resolved at elaboration
    Initial,                             // fires once at simulation start
    Final,                               // fires once at simulation end
    Or(Vec<EventSpec>),                  // fires when any constituent event fires
}
```

The event model is extensible: `Named { name, arg }` carries any identifier as the
event name. The elaborator resolves `name` against the `EventRegistry`. Built-in names
include `posedge`, `negedge`, `change`, `cross`, `above`.

---

## Expressions

### `Expr` enum

```rust
pub enum Expr {
    Literal(Literal),
    SysCall(String, Vec<Expr>),
    Ident(String),
    Path(Path),
    Unary(UnaryOp, Box<Expr>),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Range),
    Field(Box<Expr>, String),
    Block(Block),
    If { cond: Box<Expr>, then_body: Block, else_body: Block },
    Array(ArrayBody),
    BundleLit { ty: Type, fields: Vec<(String, Expr)> },
    Lambda { params: Vec<String>, body: Box<Expr> },
}
```

### `ArrayBody` enum

```rust
pub enum ArrayBody {
    Repeat(Box<Expr>, Box<Expr>),                     // [value; count]
    Comprehension(Box<Expr>, String, Range),           // [expr | var in range]
    List(Vec<Expr>),                                    // [a, b, c]
}
```

### `Literal` enum

```rust
pub enum Literal {
    Real(f64),
    Int(u64),
    Bool(bool),
    Quad(String),
    String(String),
}
```

### `UnaryOp` enum

```rust
pub enum UnaryOp { Not, Neg }
```

### `BinaryOp` enum

```rust
pub enum BinaryOp {
    Add, Sub, Mul, Div, Rem,
    Eq, Neq, Lt, Le, Gt, Ge,
    BitAnd, BitOr, BitXor,
}
```

Precedence (lowest to highest): `BitOr` < `BitAnd` < `Eq,Neq` < `Lt,Le,Gt,Ge` < `BitXor` < `Add,Sub` < `Mul,Div,Rem`.

---

## Intentional lack of resolution

The AST is deliberately unresolved. The following are legal at this stage and are
resolved by the elaboration phase:

- **Type names** (`Real`, `Electrical`, `MyBundle`) are plain strings; the AST does
  not distinguish disciplines from bundles from primitives.
- **Array dimensions** (`Bit[N]`) are arbitrary `Expr`s — `N` may be a free variable
  or a const parameter.
- **Generic/const parameters** are present as lists of identifiers and are not yet
  substituted.
- **`EventSpec::Named`** carries any identifier as an event name; the registry has
  not been consulted.
- **`use` declarations** are collected but not resolved.
- **Semantic constraints** (e.g. contribution must not appear in a `mod` body) are
  not checked here.
