//! # Parse AST
//!
//! Types produced by the [`Parser`][super::Parser] from a token stream.
//!
//! ## Phase contract
//!
//! **Input**: a sequence of [`Lexed`][super::lexer::Lexed] tokens.
//! **Output**: a [`SourceFile`] — a tree of raw syntactic forms.
//!
//! The AST is *intentionally unresolved*. The following are legal here and
//! expected to be resolved by the elaboration phase:
//!
//! - Type names (`Real`, `Electrical`, `UInt`, `MyBundle`) are plain strings;
//!   the AST does not distinguish disciplines from bundles from primitives.
//! - Array dimensions (`Bit[N]`) are arbitrary `Expr`s — `N` may be a free
//!   variable or a const parameter.
//! - Generic / const parameters (`mod Foo[N]`) are present as lists of
//!   identifiers and are not yet substituted.
//! - `EventSpec::Named` carries any identifier as an event name; the registry
//!   has not been consulted.
//! - `use` declarations are collected but not resolved.
//!
//! Semantic constraints (e.g. "contribution must not appear in a mod body")
//! are NOT checked here; they are checked by [`crate::elab::validate`].

// ─────────────────────────────── Compilation unit ────────────────────────────

/// A parsed source file — the root of the AST.
///
/// May contain any mix of top-level items in any order. The elaborator
/// processes them in dependency order after a first registration pass.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub items: Vec<Item>,
}

/// A single top-level declaration, optionally preceded by `pub`.
#[derive(Debug, Clone)]
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
    ConstDecl(ConstDecl),
}

/// A `::`-separated module path, e.g. `devices::passives::Resistor`.
#[derive(Debug, Clone)]
pub struct Path {
    pub segments: Vec<String>,
}

/// A global constant declaration `const Name : Type = Expr;`
#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    pub ty: Type,
    pub value: Expr,
}

// ─────────────────────────────── Modules ─────────────────────────────────────

/// `mod Name [CONST] <TYPE> ( PORTS ) { ... }`
///
/// `const_params` are compile-time Natural parameters (e.g. `[N]`).
/// `type_params` are generic type parameters (e.g. `<T: Add>`).
/// Both are unresolved here — the elaborator substitutes them at instantiation.
#[derive(Debug, Clone)]
pub struct ModDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    /// Compile-time Natural const parameters, e.g. `N` in `mod Foo[N]`.
    pub const_params: Vec<String>,
    /// Generic type parameters, e.g. `T: Add + Net`.
    pub type_params: Vec<TypeParam>,
    pub ports: Vec<Port>,
    pub body: Vec<ModStmt>,
}

/// A generic type parameter with optional capability bounds.
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    /// Capability names this type must satisfy, e.g. `["Add", "Net"]`.
    pub bounds: Vec<String>,
}

/// A module port declaration.
///
/// The type may be a discipline, a net-capable bundle, or an array of either.
/// The elaborator validates net-capability and expands bundles to flat fields.
#[derive(Debug, Clone)]
pub struct Port {
    pub attrs: Vec<Attribute>,
    pub direction: Direction,
    pub name: String,
    /// Unresolved type — may be a bundle name, discipline name, or parameterized type.
    pub ty: Type,
}

/// Port direction.
#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    /// Signal that flows into the module.
    Input,
    /// Signal that flows out of the module.
    Output,
    /// Bidirectional signal.
    Inout,
}

/// A statement inside a `mod` body.
///
/// Structural `For` and `If` are eliminated by the elaborator; they must have
/// elaboration-constant bounds/conditions. The remaining variants survive into
/// the [`Module`][crate::pom::Module].
#[derive(Debug, Clone)]
pub enum ModStmt {
    ParamDecl { attrs: Vec<Attribute>, name: String, ty: Type, default: Option<Expr> },
    WireDecl { attrs: Vec<Attribute>, name: String, ty: Type },
    VarDecl { attrs: Vec<Attribute>, name: String, ty: Type, default: Option<Expr> },
    StructuralFor { attrs: Vec<Attribute>, var: String, range: Range, body: Vec<ModStmt> },
    StructuralIf { attrs: Vec<Attribute>, cond: Expr, then_body: Vec<ModStmt>, else_body: Option<Vec<ModStmt>> },
    Instance {
        attrs: Vec<Attribute>,
        name: Option<String>,
        array_index: Option<Expr>,
        module: String,
        const_args: Vec<Expr>,
        type_args: Vec<Type>,
        ports: Vec<Expr>,
        params: Vec<ParamArg>,
    },
    Connection { attrs: Vec<Attribute>, lhs: Expr, rhs: Expr },
}

/// A named parameter override: `.name = expr`.
#[derive(Debug, Clone)]
pub struct ParamArg {
    pub name: String,
    pub expr: Expr,
}

/// A range expression `start .. end` or `start ..= end`.
#[derive(Debug, Clone)]
pub struct Range {
    pub start: Box<Expr>,
    pub end: Box<Expr>,
    pub inclusive: bool,
}

// ─────────────────────────────── Types ───────────────────────────────────────

/// A syntactic type reference.
///
/// `name` is looked up in the type namespace at elaboration. `dimensions` are
/// array extents — each must evaluate to a `Natural` at elaboration. `args`
/// are generic type arguments (for `<T>`-style generics).
///
/// The elaborator replaces this with either [`NetType`][crate::pom::NetType]
/// or [`ValueType`][crate::pom::ValueType].
#[derive(Debug, Clone)]
pub struct Type {
    pub name: String,
    /// Generic type arguments, e.g. `T` in `Pair<T>`.
    pub args: Vec<Type>,
    /// Array dimensions, e.g. `[8]` in `Bit[8]`. May be non-literal expressions.
    pub dimensions: Vec<Expr>,
}

// ─────────────────────────────── Disciplines ─────────────────────────────────

/// `discipline Name { ... }`
#[derive(Debug, Clone)]
pub struct DisciplineDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    pub items: Vec<DisciplineItem>,
}

#[derive(Debug, Clone)]
pub enum DisciplineItem {
    Nature { kind: NatureKind, name: String, ty: Type, attrs: Vec<AttrArg> },
    Storage(Type),
    Resolve(ResolveKind),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NatureKind {
    Potential,
    Flow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResolveKind {
    Tri,
    Or,
    And,
}

/// A named attribute in a nature declaration, e.g. `unit = "V"`.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<AttrArg>,
}

#[derive(Debug, Clone)]
pub struct AttrArg {
    pub name: String,
    pub expr: Expr,
}

// ─────────────────────────────── Bundles ─────────────────────────────────────

/// `bundle Name [CONST] <TYPE> { ... }`
///
/// A bundle is **net-capable** if every field resolves to a net type. The
/// elaborator determines this and grants the implicit `NetType` capability.
/// Net-capable bundles used as ports are expanded to flat fields.
#[derive(Debug, Clone)]
pub struct BundleDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    pub const_params: Vec<String>,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<FieldDecl>,
}

/// A single field inside a bundle.
#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub attrs: Vec<Attribute>,
    pub name: String,
    pub ty: Type,
    /// Optional default value (for value-type bundles).
    pub default: Option<Expr>,
}

// ─────────────────────────────── Enums ───────────────────────────────────────

/// `enum Name [: ReprType] { Variant [= Expr], ... }`
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    /// Optional explicit underlying type, e.g. `Bit[2]`.
    pub repr: Option<Type>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    /// Explicit discriminant; auto-increments from zero when absent.
    pub value: Option<Expr>,
}

// ─────────────────────────────── Capabilities ────────────────────────────────

/// `capability Name [: Super, ...] { fn sig; | fn decl { } }`
#[derive(Debug, Clone)]
pub struct CapabilityDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    /// Names of required super-capabilities.
    pub supers: Vec<String>,
    pub items: Vec<CapItem>,
}

/// An item inside a capability — either an abstract signature or a default body.
#[derive(Debug, Clone)]
pub enum CapItem {
    FnSig(FnSig),
    FnDecl(FnDecl),
}

/// `impl [Capability for] TypeRef { fn ... }`
#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    /// `Some` for capability impls; `None` for inherent method impls.
    pub capability: Option<String>,
    pub ty: String,
    pub const_args: Vec<Expr>,
    pub type_args: Vec<Type>,
    pub methods: Vec<FnDecl>,
}

// ─────────────────────────────── Functions ───────────────────────────────────

/// A function signature (without body).
#[derive(Debug, Clone)]
pub struct FnSig {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<FnParam>,
    pub ret: Type,
}

/// `fn Name<TYPE>(PARAMS) -> RetType { BODY }`
///
/// Functions are pure value computations. They may be generic — the elaborator
/// retains the body as-is and monomorphizes at call sites.
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub sig: FnSig,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub enum FnParam {
    SelfParam,
    Typed(String, Type),
}

// ─────────────────────────────── Blocks & statements ─────────────────────────

/// A `{ stmts... [trailing_expr] }` block.
///
/// The optional trailing expression is the block's value (Rust-style).
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Box<Expr>>,
}

/// A statement inside a function body or event block.
#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl { name: String, ty: Type, default: Option<Expr> },
    Return(Expr),
    If { cond: Expr, then_body: Block, else_body: Option<Block> },
    Match { expr: Expr, arms: Vec<StmtMatchArm> },
    For { var: String, range: Range, body: Block },
    Bind { dest: Expr, op: BindOp, src: Expr },
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct StmtMatchArm {
    pub pat: Pattern,
    pub body: Block,
}

// ─────────────────────────────── Behavior ────────────────────────────────────

/// `analog Name { ... }` or `digital Name { ... }`
///
/// Behavior blocks describe the continuous or event-driven semantics of a
/// module. Validation rules differ by `kind` (§9 of elaboration spec).
#[derive(Debug, Clone)]
pub struct BehaviorDecl {
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub kind: BehaviorKind,
    pub name: String,
    pub body: Vec<BehaviorStmt>,
}

/// The kind of a behavior block: analog (continuous-time) or digital (event-driven).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BehaviorKind {
    /// Continuous-time analog behavior.
    Analog,
    /// Event-driven digital behavior.
    Digital,
}

/// A statement inside an `analog` or `digital` block.
///
/// `For` loops in behavior blocks are unrolled at elaboration — their bounds
/// must be elaboration constants. The unrolled form appears in
/// [`BehaviorStmt`][crate::pom::BehaviorStmt].
#[derive(Debug, Clone)]
pub enum BehaviorStmt {
    VarDecl { name: String, ty: Type, default: Option<Expr> },
    Bind { dest: Expr, op: BindOp, src: Expr },
    If { cond: Expr, then_body: Vec<BehaviorStmt>, else_body: Option<Vec<BehaviorStmt>> },
    Match { expr: Expr, arms: Vec<MatchArm> },
    For { var: String, range: Range, body: Vec<BehaviorStmt> },
    /// An event block: `@ EventSpec [when (guard)] { ... }`.
    Event { spec: EventSpec, guard: Option<Expr>, body: Block },
    Diagnostic { sys: String, args: Vec<Expr> },
    Expr(Expr),
}

/// The bind operator: contribution `<+`, force `<-`, or assignment `=`.
#[derive(Debug, Clone, PartialEq)]
pub enum BindOp {
    /// Analog contribution: `dest <+ src`.
    Contrib,
    /// Digital force: `dest <- src`.
    Force,
    /// Value assignment: `dest = src`.
    Assign,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: Pattern,
    pub body: Vec<BehaviorStmt>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Path(Path),
    Wildcard,
}

// ─────────────────────────────── Events ──────────────────────────────────────

/// An event specification attached to an `@` block.
///
/// ## Extensible event model
///
/// `Named { name, arg }` carries any identifier as the event name. The
/// elaborator resolves `name` against the [`EventRegistry`][crate::elab::event::EventRegistry],
/// which means new event kinds can be added at any time without changing the
/// parser or AST. Built-in names: `posedge`, `negedge`, `change`, `cross`,
/// `above`.
#[derive(Debug, Clone)]
pub enum EventSpec {
    /// `name(arg)` — any identifier looked up in the event registry at elaboration.
    Named { name: String, arg: Expr },
    /// `initial` — fires once at simulation start.
    Initial,
    /// `final` — fires once at simulation end.
    Final,
    /// `(spec | spec | ...)` — fires when any constituent event fires.
    Or(Vec<EventSpec>),
}

// ─────────────────────────────── Expressions ─────────────────────────────────

/// An expression in the PHDL grammar.
///
/// Expressions appear at multiple levels: elaboration-position (array dims,
/// structural for bounds, param defaults) and solve-position (behavior bodies).
/// The elaborator distinguishes these by context, not by a separate type.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    SysCall(String, Vec<Expr>),
    Ident(String),
    Path(Path),
    Unary(UnaryOp, Box<Expr>),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Cast(String, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Range),
    Field(Box<Expr>, String),
    Block(Block),
    If { cond: Box<Expr>, then_body: Block, else_body: Block },
    Array(ArrayBody),
    BundleLit { ty: Type, fields: Vec<(String, Expr)> },
    Lambda { params: Vec<String>, body: Box<Expr> },
}

/// The body of an array expression `[ ... ]`.
#[derive(Debug, Clone)]
pub enum ArrayBody {
    /// `[value; N]` — repeat expression.
    Repeat(Box<Expr>, Box<Expr>),
    /// `[expr | i in 0..N]` — array comprehension.
    Comprehension(Box<Expr>, String, Range),
    /// `[a, b, c]` — element list.
    List(Vec<Expr>),
}

/// A literal value appearing in an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Floating-point literal (f64).
    Real(f64),
    /// Unsigned integer literal.
    Int(u64),
    /// Boolean literal (`true` / `false`).
    Bool(bool),
    /// Four-valued logic literal (`0q...`).
    Quad(String),
    /// Double-quoted string literal.
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Not,
    Neg,
}

/// Binary operators in precedence order (lowest to highest when parsing):
/// `||` < `&&` < `|` < `&` < `==` `!=` < `<` `<=` `>` `>=` < `^` < `+` `-` < `*` `/` `%`
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Neq,
    Lt,
    Le,
    Gt,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    /// Logical AND (`&&`).
    And,
    /// Logical OR (`||`).
    Or,
}
