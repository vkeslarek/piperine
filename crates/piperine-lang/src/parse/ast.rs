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
    ModuleDeclaration(ModuleDeclaration),
    BehaviorDecl(BehaviorDecl),
    DisciplineDecl(DisciplineDecl),
    BundleDecl(BundleDecl),
    EnumDecl(EnumDecl),
    CapabilityDecl(CapabilityDecl),
    ImplDecl(ImplDecl),
    FnDecl(FnDecl),
    ConstDecl(ConstDecl),
    /// An `extern` declaration — see [`ExternDecl`]. Signature-only by
    /// construction (SPEC "declared language surface" P2).
    ExternDecl(ExternDecl),
}

impl Item {
    /// Whether this item is declared `pub` (public, visible from other
    /// packages via `use`). `UseDecl` itself has no visibility.
    pub fn is_pub(&self) -> bool {
        match self {
            Item::UseDecl(_) => true,
            Item::ModuleDeclaration(m) => m.is_pub,
            Item::BehaviorDecl(b) => b.is_pub,
            Item::DisciplineDecl(d) => d.is_pub,
            Item::BundleDecl(b) => b.is_pub,
            Item::EnumDecl(e) => e.is_pub,
            Item::CapabilityDecl(c) => c.is_pub,
            Item::ImplDecl(i) => i.is_pub,
            Item::FnDecl(f) => f.is_pub,
            Item::ConstDecl(c) => c.is_pub,
            // `extern` declarations have no `pub` modifier (SPEC P2) — a
            // header's extern decls are always visible where the header is used.
            Item::ExternDecl(_) => false,
        }
    }

    /// The declared name of this item, if it has one (`use` and `impl`
    /// declarations don't). A behavior names the module it attaches to.
    pub fn name(&self) -> Option<&str> {
        match self {
            Item::UseDecl(_) | Item::ImplDecl(_) => None,
            Item::ModuleDeclaration(m) => Some(&m.name),
            Item::BehaviorDecl(b) => Some(&b.name),
            Item::DisciplineDecl(d) => Some(&d.name),
            Item::BundleDecl(b) => Some(&b.name),
            Item::EnumDecl(e) => Some(&e.name),
            Item::CapabilityDecl(c) => Some(&c.name),
            Item::FnDecl(f) => Some(&f.sig.name),
            Item::ConstDecl(c) => Some(&c.name),
            Item::ExternDecl(e) => Some(e.name()),
        }
    }
}

// ─────────────────────────────── Extern declarations ─────────────────────────

/// An `extern <kind> ...;` declaration — the textual home for a name whose
/// *implementation* is a native Rust registry entry (math functions, system
/// tasks, runtime operators, attribute schemas, primitive types, and native
/// type methods). Every variant is signature-only: giving any `extern`
/// declaration (or an individual method inside `Impl`) a body is a parse
/// error (SPEC "declared language surface" P2-AC7).
///
/// See `.specs/features/declared-language-surface/spec.md` P2 and
/// `design.md`'s "Grammar: extern modifier" component.
#[derive(Debug, Clone)]
pub enum ExternDecl {
    /// `extern type Name;` — a primitive value type, no body.
    Type { span: Option<miette::SourceSpan>, name: String },
    /// `extern fn name(params) -> RetType;` — a native function, signature
    /// only (the body is a compiler-side registry entry, e.g. `math.rs`).
    Fn(ExternSig),
    /// `extern task $name(params) -> RetType;` — a system task; `sig.name`
    /// retains the `$`-prefixed form.
    Task(ExternSig),
    /// `extern operator name(params) -> RetType;` — a runtime operator
    /// (`ddt`, `delay`, `slew`, …).
    Operator(ExternSig),
    /// `extern attribute name { field: Type, ... }` — an attribute schema
    /// (`@device`, `@port`, plugin-contributed ones).
    Attribute { span: Option<miette::SourceSpan>, name: String, fields: Vec<ExternAttrField> },
}

impl ExternDecl {
    /// The `decl_span` covering the whole declaration — the LSP
    /// go-to-definition target for the declaration itself.
    pub fn span(&self) -> Option<miette::SourceSpan> {
        match self {
            ExternDecl::Type { span, .. } | ExternDecl::Attribute { span, .. } => *span,
            ExternDecl::Fn(sig) | ExternDecl::Task(sig) | ExternDecl::Operator(sig) => sig.span,
        }
    }

    /// The declared name (the type/attribute-schema name for `extern
    /// type`/`extern attribute`; the function/task/operator name —
    /// `$`-prefixed for `extern task` — otherwise).
    pub fn name(&self) -> &str {
        match self {
            ExternDecl::Type { name, .. } | ExternDecl::Attribute { name, .. } => name,
            ExternDecl::Fn(sig) | ExternDecl::Task(sig) | ExternDecl::Operator(sig) => &sig.name,
        }
    }
}

/// One field of an `extern attribute` schema — same name/type shape as a
/// bundle field, with its own `decl_span` so a field name (e.g. `plugin`
/// inside `@device(plugin = ...)`) resolves independently of the schema name.
#[derive(Debug, Clone)]
pub struct ExternAttrField {
    pub span: Option<miette::SourceSpan>,
    pub name: String,
    pub ty: Type,
}

/// A signature-only declaration shared by `extern fn`/`extern task`/
/// `extern operator` and each individual method inside `extern impl` — no
/// body, `decl_span` covers the declaration line. `name` retains any
/// `$`-prefix for `extern task` (system-task identifier form).
#[derive(Debug, Clone)]
pub struct ExternSig {
    pub span: Option<miette::SourceSpan>,
    pub name: String,
    pub params: Vec<FnParam>,
    pub ret: Type,
}

/// A `::`-separated module path, e.g. `devices::passives::Resistor`.
#[derive(Debug, Clone)]
pub struct Path {
    pub segments: Vec<String>,
}

/// A global constant declaration `const Name : Type = Expr;`
#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub span: Option<miette::SourceSpan>,
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
pub struct ModuleDeclaration {
    pub span: Option<miette::SourceSpan>,
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub name: String,
    /// Compile-time Natural const parameters, e.g. `N` in `mod Foo[N]`.
    pub const_params: Vec<String>,
    /// Generic type parameters, e.g. `T: Add + Net`.
    pub type_params: Vec<TypeParam>,
    pub ports: Vec<Port>,
    pub body: Vec<ModuleStatement>,
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
impl ModuleStatement {
    pub fn span(&self) -> Option<miette::SourceSpan> {
        match self {
            Self::ParamDecl { span, .. } => *span,
            Self::WireDecl { span, .. } => *span,
            Self::VarDecl { span, .. } => *span,
            Self::StructuralFor { span, .. } => *span,
            Self::StructuralIf { span, .. } => *span,
            Self::Instance { span, .. } => *span,
            Self::Connection { span, .. } => *span,
            Self::Assert { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModuleStatement {
    ParamDecl { span: Option<miette::SourceSpan>, attrs: Vec<Attribute>, name: String, ty: Type, default: Option<Expr> },
    WireDecl { span: Option<miette::SourceSpan>, attrs: Vec<Attribute>, name: String, ty: Type },
    VarDecl { span: Option<miette::SourceSpan>, attrs: Vec<Attribute>, name: String, ty: Type, default: Option<Expr> },
    StructuralFor { span: Option<miette::SourceSpan>, attrs: Vec<Attribute>, var: String, range: Range, body: Vec<ModuleStatement> },
    StructuralIf { span: Option<miette::SourceSpan>, attrs: Vec<Attribute>, cond: Expr, then_body: Vec<ModuleStatement>, else_body: Option<Vec<ModuleStatement>> },
    Instance {
        span: Option<miette::SourceSpan>,
        attrs: Vec<Attribute>,
        name: Option<String>,
        array_index: Option<Expr>,
        module: String,
        const_args: Vec<Expr>,
        type_args: Vec<Type>,
        ports: Vec<PortConnection>,
        params: Vec<ParamArg>,
    },
    Connection { span: Option<miette::SourceSpan>, attrs: Vec<Attribute>, lhs: Expr, rhs: Expr },
    /// `$assert(cond, msg);` — an elaboration-time check (SPEC §7.4).
    Assert { span: Option<miette::SourceSpan>, attrs: Vec<Attribute>, cond: Expr, msg: Expr },
}

/// One instance port connection: positional (`a`) or named (`.p = a`).
#[derive(Debug, Clone)]
pub enum PortConnection {
    Positional(Expr),
    Named { port: String, expr: Expr },
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
    /// Trailing `?`: an optional value that may be absent (`none`). A `param`
    /// or binding of an optional type must be read through `.is_present()` /
    /// `.get_or(default)`; `none` inhabits any optional type.
    pub optional: bool,
}

// ─────────────────────────────── Disciplines ─────────────────────────────────

/// `discipline Name { ... }`
#[derive(Debug, Clone)]
pub struct DisciplineDecl {
    pub span: Option<miette::SourceSpan>,
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
    Sum,
    Avg,
    Max,
    Min,
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
    pub span: Option<miette::SourceSpan>,
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
    pub span: Option<miette::SourceSpan>,
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
    pub span: Option<miette::SourceSpan>,
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
    pub span: Option<miette::SourceSpan>,
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
    pub span: Option<miette::SourceSpan>,
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    /// Whether this is an `extern fn` — signature-only, body provided by
    /// the compiler or a plugin (SPEC Part I §8, ROADMAP "extern").
    pub is_extern: bool,
    pub sig: FnSig,
    pub body: Block,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum FnParam {
    SelfParam,
    /// A typed parameter, optionally with a default value (the language spec Part I §9.1
    /// — trailing parameters may carry a default; a call may omit them).
    /// Defaults are elaboration constants. The default is `None` for
    /// non-defaulted (leading) params.
    Typed { name: String, ty: Type, default: Option<Expr> },
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
    /// `var name [: Type] = expr;` — the type annotation is optional here
    /// (unlike a `mod`-body `var`): an omitted `ty` infers from `default`
    /// at runtime.
    /// A statically-elaborated fn/method body (an `impl` or global `fn`)
    /// still requires an explicit type (SPEC Part I §9) — the elaborator
    /// enforces that, not the parser.
    VarDecl { name: String, ty: Option<Type>, default: Option<Expr> },
    Return(Expr),
    If { cond: Expr, then_body: Block, else_body: Option<Block> },
    Match { expr: Expr, arms: Vec<StmtMatchArm> },
    For { var: String, iter: ForIter, body: Block },
    Bind { dest: Expr, op: BindOp, src: Expr },
    /// An event block `@ EventSpec [when (guard)] { ... }` — behavior
    /// bodies only (validated by `elab`, not the type system: one `Stmt`
    /// serves fn bodies and `analog`/`digital` bodies alike,
    /// SIMPLIFICATION.md P3).
    Event { spec: EventSpec, guard: Option<Expr>, body: Block },
    /// A `$display`-family diagnostic call in statement position.
    Diagnostic { sys: String, args: Vec<Expr> },
    Expr(Expr),
}

/// What a fn-body `for` loops over: an elaboration-time range (as in a
/// module/behavior body) or a runtime value-layer list (SPEC Part I §9 —
/// `for rl in [1e3, 1e4, ...]`, only valid where the interpreter runs).
#[derive(Debug, Clone)]
pub enum ForIter {
    Range(Range),
    Expr(Expr),
}

/// Traversal control returned by a walker closure: keep descending into the
/// current node's children, or skip them (the walk continues with the next
/// sibling either way). See [`Expr::walk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Walk {
    Continue,
    SkipChildren,
}

impl Block {
    /// Visit every expression in this block, pre-order (statements in
    /// source order, then the trailing expression). See [`Expr::walk`].
    pub fn walk_exprs(&self, f: &mut impl FnMut(&Expr) -> Walk) {
        self.stmts.iter().for_each(|s| s.walk_exprs(f));
        if let Some(e) = &self.expr {
            e.walk(f);
        }
    }

    /// Mutable counterpart of [`Block::walk_exprs`].
    pub fn walk_exprs_mut(&mut self, f: &mut impl FnMut(&mut Expr) -> Walk) {
        self.stmts.iter_mut().for_each(|s| s.walk_exprs_mut(f));
        if let Some(e) = &mut self.expr {
            e.walk_mut(f);
        }
    }

}

impl Stmt {
    /// Visit every expression in this statement, pre-order. See
    /// [`Expr::walk`].
    pub fn walk_exprs(&self, f: &mut impl FnMut(&Expr) -> Walk) {
        match self {
            Stmt::VarDecl { default, .. } => {
                if let Some(e) = default {
                    e.walk(f);
                }
            }
            Stmt::Return(e) | Stmt::Expr(e) => e.walk(f),
            Stmt::If { cond, then_body, else_body } => {
                cond.walk(f);
                then_body.walk_exprs(f);
                if let Some(b) = else_body {
                    b.walk_exprs(f);
                }
            }
            Stmt::Match { expr, arms } => {
                expr.walk(f);
                arms.iter().for_each(|a| a.body.walk_exprs(f));
            }
            Stmt::For { iter, body, .. } => {
                match iter {
                    ForIter::Range(r) => {
                        r.start.walk(f);
                        r.end.walk(f);
                    }
                    ForIter::Expr(e) => e.walk(f),
                }
                body.walk_exprs(f);
            }
            Stmt::Bind { dest, src, .. } => {
                dest.walk(f);
                src.walk(f);
            }
            Stmt::Event { spec, guard, body } => {
                spec.walk_exprs(f);
                if let Some(g) = guard {
                    g.walk(f);
                }
                body.walk_exprs(f);
            }
            Stmt::Diagnostic { args, .. } => args.iter().for_each(|a| a.walk(f)),
        }
    }

    /// Mutable counterpart of [`Stmt::walk_exprs`].
    pub fn walk_exprs_mut(&mut self, f: &mut impl FnMut(&mut Expr) -> Walk) {
        match self {
            Stmt::VarDecl { default, .. } => {
                if let Some(e) = default {
                    e.walk_mut(f);
                }
            }
            Stmt::Return(e) | Stmt::Expr(e) => e.walk_mut(f),
            Stmt::If { cond, then_body, else_body } => {
                cond.walk_mut(f);
                then_body.walk_exprs_mut(f);
                if let Some(b) = else_body {
                    b.walk_exprs_mut(f);
                }
            }
            Stmt::Match { expr, arms } => {
                expr.walk_mut(f);
                arms.iter_mut().for_each(|a| a.body.walk_exprs_mut(f));
            }
            Stmt::For { iter, body, .. } => {
                match iter {
                    ForIter::Range(r) => {
                        r.start.walk_mut(f);
                        r.end.walk_mut(f);
                    }
                    ForIter::Expr(e) => e.walk_mut(f),
                }
                body.walk_exprs_mut(f);
            }
            Stmt::Bind { dest, src, .. } => {
                dest.walk_mut(f);
                src.walk_mut(f);
            }
            Stmt::Event { spec, guard, body } => {
                spec.walk_exprs_mut(f);
                if let Some(g) = guard {
                    g.walk_mut(f);
                }
                body.walk_exprs_mut(f);
            }
            Stmt::Diagnostic { args, .. } => args.iter_mut().for_each(|a| a.walk_mut(f)),
        }
    }

    /// Substitute `var → value` in all expressions of this statement.
    /// See [`Expr::subst_const`] for the lambda-body exception.
    pub fn subst_const(&mut self, var: &str, value: u64) {
        self.walk_exprs_mut(&mut Expr::subst_visitor(var, value));
    }

    /// Visit this statement and every nested statement, pre-order
    /// (if/match/for/event bodies, in source order).
    pub fn walk_stmts(&self, f: &mut impl FnMut(&Stmt)) {
        f(self);
        match self {
            Stmt::If { then_body, else_body, .. } => {
                then_body.stmts.iter().for_each(|s| s.walk_stmts(f));
                if let Some(b) = else_body {
                    b.stmts.iter().for_each(|s| s.walk_stmts(f));
                }
            }
            Stmt::Match { arms, .. } => arms
                .iter()
                .for_each(|a| a.body.stmts.iter().for_each(|s| s.walk_stmts(f))),
            Stmt::For { body, .. } | Stmt::Event { body, .. } => {
                body.stmts.iter().for_each(|s| s.walk_stmts(f));
            }
            Stmt::VarDecl { .. }
            | Stmt::Return(_)
            | Stmt::Bind { .. }
            | Stmt::Diagnostic { .. }
            | Stmt::Expr(_) => {}
        }
    }
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
    pub span: Option<miette::SourceSpan>,
    pub attrs: Vec<Attribute>,
    pub is_pub: bool,
    pub kind: BehaviorKind,
    pub name: String,
    pub body: Vec<Stmt>,
}

/// The kind of a behavior block: analog (continuous-time) or digital (event-driven).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BehaviorKind {
    /// Continuous-time analog behavior.
    Analog,
    /// Event-driven digital behavior.
    Digital,
}

impl EventSpec {
    /// Immutable visit of every expression in this event spec (`Named` args,
    /// recursively through `Or`).
    pub fn walk_exprs(&self, f: &mut impl FnMut(&Expr) -> Walk) {
        match self {
            EventSpec::Named { args, .. } => args.iter().for_each(|a| a.walk(f)),
            EventSpec::Initial | EventSpec::Final => {}
            EventSpec::Or(specs) => specs.iter().for_each(|s| s.walk_exprs(f)),
        }
    }

    /// Mutable visit of every expression in this event spec (`Named` args,
    /// recursively through `Or`).
    pub fn walk_exprs_mut(&mut self, f: &mut impl FnMut(&mut Expr) -> Walk) {
        match self {
            EventSpec::Named { args, .. } => args.iter_mut().for_each(|a| a.walk_mut(f)),
            EventSpec::Initial | EventSpec::Final => {}
            EventSpec::Or(specs) => specs.iter_mut().for_each(|s| s.walk_exprs_mut(f)),
        }
    }
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
pub enum Pattern {
    Path(Path),
    Wildcard,
    /// A numeric literal pattern (e.g. `0b1100`, `3`).
    Literal(u64),
    /// A bit pattern with don't-cares (`0b1??0`); one char per bit,
    /// MSB first, each `0`, `1`, or `?`.
    BitPattern(String),
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
/// `name(arg)` or `name(arg, arg, ...)` — any identifier looked up in the
/// event registry at elaboration. Multiple arguments support events like
/// `@timer(period, phase)`; single-arg events (`cross`, `above`) use
/// `args[0]`.
#[derive(Debug, Clone)]
pub enum EventSpec {
    Named { name: String, args: Vec<Expr> },
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
    /// `(a, b, ...)` — a value-layer tuple literal (SPEC Part I §6.1).
    /// `(e)` with no comma is a parenthesized group, not a 1-tuple.
    Tuple(Vec<Expr>),
    BundleLit { ty: Type, fields: Vec<(String, Expr)> },
    /// A `Map { k: v, ... }` literal (`Map<K, V>`, used for `ic`/`nodeset`
    /// per-node hints). `Map {}` is the empty map.
    MapLit(Vec<(Expr, Expr)>),
    /// A `Set { a, b, c }` literal. `Set {}` is the empty set.
    SetLit(Vec<Expr>),
    Lambda { params: Vec<String>, body: Box<Expr> },
}

impl Expr {
    /// Visit this expression and every sub-expression, pre-order: `f` sees
    /// the node first; return [`Walk::SkipChildren`] to prune the subtree.
    /// Descends through blocks (their statements' expressions in source
    /// order), lambda bodies, event args — everything.
    ///
    /// This and [`Expr::walk_mut`] are the **only** exhaustive child
    /// enumerations over `Expr` outside the transformers (eval, to-IR,
    /// formatter, predict): a new variant is added here once and every
    /// search/rewrite in the crate follows (SIMPLIFICATION.md P4).
    pub fn walk(&self, f: &mut impl FnMut(&Expr) -> Walk) {
        if f(self) == Walk::SkipChildren {
            return;
        }
        match self {
            Expr::Literal(_) | Expr::Ident(_) | Expr::Path(_) => {}
            Expr::SysCall(_, args) | Expr::Tuple(args) => {
                args.iter().for_each(|a| a.walk(f));
            }
            Expr::Unary(_, inner) | Expr::Cast(_, inner) | Expr::Field(inner, _) | Expr::Lambda { body: inner, .. } => {
                inner.walk(f);
            }
            Expr::Binary(l, _, r) | Expr::Index(l, r) => {
                l.walk(f);
                r.walk(f);
            }
            Expr::Call(callee, args) => {
                callee.walk(f);
                args.iter().for_each(|a| a.walk(f));
            }
            Expr::Slice(base, range) => {
                base.walk(f);
                range.start.walk(f);
                range.end.walk(f);
            }
            Expr::Block(b) => b.walk_exprs(f),
            Expr::If { cond, then_body, else_body } => {
                cond.walk(f);
                then_body.walk_exprs(f);
                else_body.walk_exprs(f);
            }
            Expr::Array(ArrayBody::List(items)) => items.iter().for_each(|e| e.walk(f)),
            Expr::Array(ArrayBody::Repeat(v, n)) => {
                v.walk(f);
                n.walk(f);
            }
            Expr::Array(ArrayBody::Comprehension(e, _, range)) => {
                e.walk(f);
                range.start.walk(f);
                range.end.walk(f);
            }
            Expr::BundleLit { fields, .. } => fields.iter().for_each(|(_, e)| e.walk(f)),
            Expr::MapLit(entries) => entries.iter().for_each(|(k, v)| {
                k.walk(f);
                v.walk(f);
            }),
            Expr::SetLit(items) => items.iter().for_each(|e| e.walk(f)),
        }
    }

    /// Mutable counterpart of [`Expr::walk`]: `f` may replace the node in
    /// place; the walk then descends into the (possibly new) node's
    /// children.
    pub fn walk_mut(&mut self, f: &mut impl FnMut(&mut Expr) -> Walk) {
        if f(self) == Walk::SkipChildren {
            return;
        }
        match self {
            Expr::Literal(_) | Expr::Ident(_) | Expr::Path(_) => {}
            Expr::SysCall(_, args) | Expr::Tuple(args) => {
                args.iter_mut().for_each(|a| a.walk_mut(f));
            }
            Expr::Unary(_, inner) | Expr::Cast(_, inner) | Expr::Field(inner, _) | Expr::Lambda { body: inner, .. } => {
                inner.walk_mut(f);
            }
            Expr::Binary(l, _, r) | Expr::Index(l, r) => {
                l.walk_mut(f);
                r.walk_mut(f);
            }
            Expr::Call(callee, args) => {
                callee.walk_mut(f);
                args.iter_mut().for_each(|a| a.walk_mut(f));
            }
            Expr::Slice(base, range) => {
                base.walk_mut(f);
                range.start.walk_mut(f);
                range.end.walk_mut(f);
            }
            Expr::Block(b) => b.walk_exprs_mut(f),
            Expr::If { cond, then_body, else_body } => {
                cond.walk_mut(f);
                then_body.walk_exprs_mut(f);
                else_body.walk_exprs_mut(f);
            }
            Expr::Array(ArrayBody::List(items)) => items.iter_mut().for_each(|e| e.walk_mut(f)),
            Expr::Array(ArrayBody::Repeat(v, n)) => {
                v.walk_mut(f);
                n.walk_mut(f);
            }
            Expr::Array(ArrayBody::Comprehension(e, _, range)) => {
                e.walk_mut(f);
                range.start.walk_mut(f);
                range.end.walk_mut(f);
            }
            Expr::BundleLit { fields, .. } => fields.iter_mut().for_each(|(_, e)| e.walk_mut(f)),
            Expr::MapLit(entries) => entries.iter_mut().for_each(|(k, v)| {
                k.walk_mut(f);
                v.walk_mut(f);
            }),
            Expr::SetLit(items) => items.iter_mut().for_each(|e| e.walk_mut(f)),
        }
    }

    /// The `walk_mut` visitor implementing loop-variable substitution:
    /// `Ident(var)` → `Literal::Int(value)`, skipping lambda bodies (their
    /// parameters may shadow `var`; capture is not substitution).
    pub(crate) fn subst_visitor(var: &str, value: u64) -> impl FnMut(&mut Expr) -> Walk + '_ {
        move |e| match e {
            Expr::Ident(name) if name == var => {
                *e = Expr::Literal(Literal::Int(value));
                Walk::Continue
            }
            Expr::Lambda { .. } => Walk::SkipChildren,
            _ => Walk::Continue,
        }
    }


    /// Substitute every `Ident(name)` matching `var` with `Literal::Int(value)`.
    /// Used during behavioral `for` unrolling to replace the loop variable
    /// with its concrete iteration value (the `for` is syntactic sugar —
    /// same as `if` const-folding, the bound must be an elaboration constant).
    pub fn subst_const(&mut self, var: &str, value: u64) {
        self.walk_mut(&mut Expr::subst_visitor(var, value));
    }
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
    /// `none` — the absent value of any optional type (`Real?`). Read through
    /// `.is_present()` / `.get_or(default)`.
    None,
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
