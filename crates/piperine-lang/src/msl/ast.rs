//! The MSL syntax tree: one node per Appendix B.3 production, plus the Rust
//! 2024 value forms the language inherits (`docs/spec/index.md` §0.4).
//!
//! Every node carries its source range in a [`Loc`]. `Loc` compares equal to
//! any other `Loc`, so `==` on nodes is *structural*: that is what the
//! printer's round-trip test needs (a printed tree cannot reproduce the
//! original spans). Compare spans explicitly through `span.0`.
//!
//! # Production to node
//!
//! | B.3 production | Node |
//! | --- | --- |
//! | `item` | [`Item`] / [`ItemKind`] |
//! | `nature_decl` | [`NatureDecl`] |
//! | `nature_field` | [`NatureField`] / [`NatureFieldKind`] |
//! | `struct_field` | [`Field`] (carrier fields make a bundle: the type is a [`Type`]) |
//! | `impl_block`, `impl_item` | [`ImplBlock`], [`ImplItem`] |
//! | `fn_decl`, `fn_param` | [`FnDecl`], [`FnParam`] |
//! | `call`, `method_call`, `instance`, `callee` | [`ExprKind::Call`] (`params` is `Some` for an instance), [`ExprKind::MethodCall`]; the callee is any [`Expr`] |
//! | `template` | [`ExprKind::Struct`] (name resolution tells a template from a struct expression) |
//! | `module_decl`, `port`, `dir` | [`ModuleDecl`], [`Port`], [`Dir`] |
//! | `sigtype` | [`Type`] with [`TypeKind::Flip`] and [`TypeKind::Array`] |
//! | `module_item`, `param`, `signal_decl` | [`ModuleItem`] / [`ModuleItemKind`] |
//! | `behavior_block`, `names` | [`BehaviorBlock`], [`BehaviorKind`] |
//! | `digital_item`, `reaction` | [`DigitalItem`] / [`DigitalItemKind`], [`Reaction`] |
//! | `gen<X>` | [`Gen`] / [`GenKind`] / [`GenElse`] |
//! | `contribution`, `commit` | [`StmtKind::Contribution`], [`StmtKind::Commit`] |
//! | `place` | an [`Expr`] built of paths, fields and indexes |
//! | `emit`, `include_str` | [`ExprKind::Emit`], [`ExprKind::IncludeStr`] |
//! | `attr_*` | [`Attr`] / [`AttrInput`] |
//! | `stmt`, `expr`, `type`, `pat`, `block`, `generics`, `generic_args`, `path`, `const_item` | [`Stmt`], [`Expr`], [`Type`], [`Pat`], [`Block`], [`Generics`], [`GenericArgs`], [`Path`], [`ConstItem`] |

use serde::{Deserialize, Serialize};

use super::span::Span;

/// A node's source range. Equality ignores it; see the module docs.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Loc(pub Span);

impl PartialEq for Loc {
    fn eq(&self, _: &Loc) -> bool {
        true
    }
}

impl Eq for Loc {}

impl From<Span> for Loc {
    fn from(s: Span) -> Loc {
        Loc(s)
    }
}

// ---- file, metadata -------------------------------------------------------------------------

/// A whole source file.
#[derive(Debug, Clone, PartialEq)]
pub struct File {
    /// `//!` comments at the top of the file.
    pub inner_docs: Vec<Doc>,
    pub items: Vec<Item>,
    pub span: Loc,
}

/// A doc comment: `///` or `//!` (the text after the marker) or, with
/// `block`, `/** */` or `/*! */` (the text between the markers), verbatim.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub text: String,
    pub block: bool,
    pub span: Loc,
}

/// An identifier with its range. Contextual keywords are identifiers.
#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    /// The name, without any `r#` prefix.
    pub name: String,
    /// Written `r#name`.
    pub raw: bool,
    pub span: Loc,
}

impl Ident {
    /// The name as written: `r#out` for a raw identifier.
    pub fn spelled(&self) -> String {
        if self.raw { format!("r#{}", self.name) } else { self.name.clone() }
    }
}

/// Doc comments and attributes in front of an item, field, port or variant.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meta {
    pub docs: Vec<Doc>,
    pub attrs: Vec<Attr>,
}

/// `#[path]`, `#[path(args)]` or `#[path = expr]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub path: Path,
    pub input: AttrInput,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttrInput {
    None,
    /// `#[doc = "text"]`
    Eq(Expr),
    /// `#[derive(Repr, Clone)]`, `#[link("x.osdi", model = "bsim4")]`
    List(Vec<AttrArg>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttrArg {
    Path(Path),
    Lit(Lit),
    NameValue(Ident, Lit),
}

// ---- paths, generics, types -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub segments: Vec<PathSeg>,
    pub span: Loc,
}

/// One path segment; `self`, `Self`, `super` and `crate` are segments too.
#[derive(Debug, Clone, PartialEq)]
pub struct PathSeg {
    pub ident: Ident,
    /// `Bit<u8>`, `idt::<Heat>` (a turbofish prints the same) or `Fn(Cell)`.
    pub args: Option<GenericArgs>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenericArgs {
    Angle { args: Vec<GenericArg>, turbofish: bool, span: Loc },
    /// The `Fn(P) -> R` bound sugar.
    Paren { inputs: Vec<Type>, output: Option<Box<Type>>, span: Loc },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenericArg {
    Type(Type),
    /// A const argument: a literal or a `{ block }`.
    Const(Expr),
}

/// `<T: Repr = bool, const A: u32>` on a declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Generics {
    pub params: Vec<GenericParam>,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: Ident,
    pub kind: GenericParamKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenericParamKind {
    /// A type or nature parameter: bounds and default.
    Type { bounds: Vec<Path>, default: Option<Type> },
    /// `const A: u32 = 4`.
    Const { ty: Type, default: Option<Expr> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Path(Path),
    /// `[T; N]`: `len` is a const expression.
    Array { elem: Box<Type>, len: Box<Expr> },
    /// `[T]`
    Slice(Box<Type>),
    /// `(A, B)`; the empty tuple is `()`.
    Tuple(Vec<Type>),
    /// `&T` or `&'static T`.
    Ref { static_lifetime: bool, inner: Box<Type> },
    /// `flip T`: reverses every direction of a bundle type.
    Flip(Box<Type>),
    /// `fn(A, B) -> R`: a function-pointer type (`ret` is `None` for `fn()`).
    FnPtr { params: Vec<Type>, ret: Option<Box<Type>> },
    /// `_`
    Infer,
}

// ---- patterns ---------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Pat {
    pub kind: PatKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatKind {
    /// `_`
    Wild,
    /// `..` inside a tuple, slice or tuple-struct pattern.
    Rest,
    /// `x`, `mut x`, `ref x`, `x @ p`.
    Ident { by_ref: bool, mutable: bool, name: Ident, sub: Option<Box<Pat>> },
    /// A literal, possibly negated: the expression is a `Lit` or a `Unary` of one.
    Lit(Box<Expr>),
    /// A path naming a constant or a unit variant: `Phase::Liquid`.
    Path(Path),
    Tuple(Vec<Pat>),
    /// `Some(x)`, `Phase::Data(n)`.
    TupleStruct { path: Path, pats: Vec<Pat> },
    /// `Data { n }`, `Point { x: 0, .. }`.
    Struct { path: Path, fields: Vec<FieldPat>, rest: bool },
    /// `a | b`
    Or(Vec<Pat>),
    /// `lo..hi`, `lo..=hi`
    Range { lo: Option<Box<Expr>>, hi: Option<Box<Expr>>, inclusive: bool },
    /// `&p`
    Ref(Box<Pat>),
    Slice(Vec<Pat>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldPat {
    pub name: Ident,
    /// `None` is the shorthand `Point { x }`.
    pub pat: Option<Pat>,
    pub span: Loc,
}

// ---- expressions ------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lit {
    pub kind: LitKind,
    pub span: Loc,
}

impl Lit {
    /// The value of a string literal: escapes processed, a raw string verbatim.
    /// `None` for another kind of literal or a string with an undefined escape.
    pub fn str_value(&self) -> Option<String> {
        let LitKind::Str { text, .. } = &self.kind else { return None };
        let (value, bad) = super::lexer::unescape(text);
        bad.is_empty().then_some(value)
    }

    /// The value of an integer literal (any radix, underscores and suffix
    /// removed); `None` for another kind of literal or one `E0909` rejects.
    pub fn int_value(&self) -> Option<u128> {
        match &self.kind {
            LitKind::Int(text) => super::lexer::int_value(text),
            _ => None,
        }
    }

    /// The suffix of an integer or float literal: a type suffix, or a float's
    /// SI letter; `None` when there is none.
    pub fn suffix(&self) -> Option<&str> {
        let suffix = match &self.kind {
            LitKind::Int(text) => super::lexer::int_suffix_text(text),
            LitKind::Float { raw, .. } => super::lexer::float_suffix(raw),
            _ => "",
        };
        (!suffix.is_empty()).then_some(suffix)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LitKind {
    /// The text, type suffix included.
    Int(String),
    /// The value (SI suffix applied) and the original text.
    Float { value: f64, raw: String },
    /// The text between the quotes, escapes unprocessed; a raw string holds
    /// its body with backslash and quote escaped, and its number of `#`.
    Str { text: String, raw_hashes: Option<u8> },
    Bool(bool),
    /// The `Unknown` literal (A.3).
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    Neg,
    Not,
    Deref,
    /// `&e`: a shared borrow (`&mut` is not in the language).
    Ref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add, Sub, Mul, Div, Rem, BitXor, BitAnd, BitOr, Shl, Shr,
    And, Or, Eq, Ne, Lt, Le, Gt, Ge,
}

/// An argument: positional, or `name: value` after the positional ones.
#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<Ident>,
    pub value: Expr,
    pub span: Loc,
}

/// `name: value` in a struct expression, a template or an instance's param block.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    pub name: Ident,
    pub value: Expr,
    /// Written `{ r }` for `{ r: r }`: the authored form is kept.
    pub shorthand: bool,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub meta: Meta,
    pub pat: Pat,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosureParam {
    pub pat: Pat,
    pub ty: Option<Type>,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Lit(Lit),
    Path(Path),
    Unary { op: UnOp, expr: Box<Expr> },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    /// `lhs = rhs` (`op: None`) or `lhs += rhs`.
    Assign { op: Option<BinOp>, lhs: Box<Expr>, rhs: Box<Expr> },
    Cast { expr: Box<Expr>, ty: Type },
    /// `callee(args)`. `params` is the `{ name: value }` block of an instance;
    /// whether the callee names a module is decided by name resolution.
    Call { callee: Box<Expr>, args: Vec<Arg>, params: Option<Vec<FieldInit>> },
    MethodCall { recv: Box<Expr>, method: Ident, generics: Option<GenericArgs>, args: Vec<Arg> },
    /// `e.name`; a tuple index `e.0` has the digits as its name.
    Field { expr: Box<Expr>, name: Ident },
    Index { expr: Box<Expr>, index: Box<Expr> },
    Range { lo: Option<Box<Expr>>, hi: Option<Box<Expr>>, inclusive: bool },
    /// `(e)`, kept so the printer reproduces the grouping.
    Paren(Box<Expr>),
    /// `(a, b)`; the unit value is `()`.
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    ArrayRepeat { elem: Box<Expr>, len: Box<Expr> },
    /// `Path { name: value, .. }`: a struct expression, a bundle literal or a template.
    Struct { path: Path, fields: Vec<FieldInit> },
    Block(Block),
    If { cond: Box<Expr>, then: Block, else_: Option<Box<Expr>> },
    /// `let pat = expr` in a condition.
    Let { pat: Pat, expr: Box<Expr> },
    Match { scrutinee: Box<Expr>, arms: Vec<Arm> },
    While { cond: Box<Expr>, body: Block },
    Loop { body: Block },
    For { pat: Pat, iter: Box<Expr>, body: Block },
    Break(Option<Box<Expr>>),
    Continue,
    Return(Option<Box<Expr>>),
    Closure { is_move: bool, params: Vec<ClosureParam>, ret: Option<Type>, body: Box<Expr> },
    /// `e?`
    Try(Box<Expr>),
    /// `emit!("stream", name = value, ...)` (B.3 `emit`).
    Emit { stream: Lit, fields: Vec<FieldInit> },
    /// `include_str!("path")` (B.3 `include_str`).
    IncludeStr(Lit),
    /// A malformed expression already reported by a diagnostic.
    Error,
}

// ---- statements -------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    /// Outer doc comments and attributes before the statement (an item
    /// statement keeps its own on the item).
    pub meta: Meta,
    pub kind: StmtKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `let pat: ty = init else { diverge };` (`else_` only with an `init`).
    Let { pat: Pat, ty: Option<Type>, init: Option<Expr>, else_: Option<Block> },
    /// An expression statement. `semi: false` on the last statement of a
    /// block is the block's value.
    Expr { expr: Expr, semi: bool },
    /// A Rust item inside a block (`const`, `fn`, `use`, ...).
    Item(Box<Item>),
    /// `probe +> value;` (B.3 `contribution`).
    Contribution { lhs: Expr, rhs: Expr },
    /// `place <- value [after delay];` (B.3 `commit`).
    Commit { place: Expr, value: Expr, after: Option<Expr> },
    /// A lone `;`.
    Empty,
    /// `signal name: type = init;`, a module-body form. Parsed in any block
    /// so a context check can report it where it is not allowed.
    Signal { name: Ident, ty: Type, init: Option<Expr> },
    /// `on event { .. }`, a `digital` form, parsed for the same reason.
    Reaction(Reaction),
}

// ---- items ------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub meta: Meta,
    pub public: bool,
    pub kind: ItemKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    Fn(FnDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Impl(ImplBlock),
    Trait(TraitDecl),
    Const(ConstItem),
    Use(UseTree),
    Mod(ModItem),
    Nature(NatureDecl),
    Module(ModuleDecl),
    Behavior(BehaviorBlock),
    /// A malformed item already reported by a diagnostic (a macro definition).
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub is_const: bool,
    /// `extern "C" fn`: the ABI string.
    pub abi: Option<Lit>,
    pub name: Ident,
    pub generics: Option<Generics>,
    pub params: Vec<FnParam>,
    pub ret: Option<Type>,
    /// `where expr`: a boolean const expression.
    pub where_: Option<Expr>,
    /// `None` for a declaration ending in `;` (an `extern` function).
    pub body: Option<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FnParam {
    /// `self`
    SelfValue(Loc),
    Named { name: Ident, ty: Type, span: Loc },
    /// A malformed parameter already reported by a diagnostic (`E0143`).
    Error(Loc),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dir {
    In,
    Out,
}

/// A `struct` or `enum` variant body.
#[derive(Debug, Clone, PartialEq)]
pub enum Fields {
    Named(Vec<Field>),
    Tuple(Vec<Field>),
    Unit,
}

/// `struct_field`: with a carrier type it makes the struct a bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub meta: Meta,
    pub public: bool,
    pub dir: Option<Dir>,
    /// `None` in a tuple field.
    pub name: Option<Ident>,
    pub ty: Type,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub fields: Fields,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub meta: Meta,
    pub name: Ident,
    pub fields: Fields,
    pub discriminant: Option<Expr>,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    pub generics: Option<Generics>,
    /// `impl Trait for Type`: the trait, when `for` is present.
    pub trait_: Option<Path>,
    pub self_ty: Type,
    pub items: Vec<ImplItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplItem {
    pub meta: Meta,
    pub public: bool,
    pub kind: ImplItemKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImplItemKind {
    Fn(FnDecl),
    Const(ConstItem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraitDecl {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub supertraits: Vec<Path>,
    pub items: Vec<ImplItem>,
}

/// `const NAME: ty = value;`; the value is absent in a trait.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstItem {
    pub name: Ident,
    pub ty: Type,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseTree {
    pub path: Vec<Ident>,
    pub kind: UseKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UseKind {
    Simple { alias: Option<Ident> },
    Glob,
    Group(Vec<UseTree>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModItem {
    pub name: Ident,
    /// `None` for `mod name;`.
    pub items: Option<Vec<Item>>,
}

// ---- natures, modules, behavior ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct NatureDecl {
    pub name: Ident,
    pub generics: Option<Generics>,
    pub fields: Vec<NatureField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NatureField {
    pub meta: Meta,
    pub kind: NatureFieldKind,
    pub span: Loc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quantity {
    Potential,
    Flow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NatureFieldKind {
    /// `potential: Voltage { access: v, units: "V" }`
    Quantity { which: Quantity, name: Ident, attrs: Vec<FieldInit> },
    /// `value: T`
    Value(Type),
    /// `resolve: expr`
    Resolve(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleDecl {
    /// `extern "osdi" module`: the ABI string.
    pub abi: Option<Lit>,
    pub name: Ident,
    pub generics: Option<Generics>,
    pub ports: Vec<Port>,
    /// `-> T`
    pub ret: Option<Type>,
    pub where_: Option<Expr>,
    pub body: Vec<ModuleItem>,
    /// A trailing expression: the module's result.
    pub result: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Port {
    pub meta: Meta,
    pub dir: Option<Dir>,
    pub name: Ident,
    pub ty: Type,
    pub default: Option<Expr>,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleItem {
    pub meta: Meta,
    pub public: bool,
    pub kind: ModuleItemKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModuleItemKind {
    /// `name: ty = default where cond;`
    Param { name: Ident, ty: Type, default: Option<Expr>, where_: Option<Expr> },
    /// `signal name: sigtype = init;`
    Signal { name: Ident, ty: Type, init: Option<Expr> },
    Let { pat: Pat, value: Expr },
    /// `place = value after delay;`
    Def { place: Expr, value: Expr, after: Option<Expr> },
    Gen(Gen<ModuleItem>),
    /// A malformed item already reported by a diagnostic.
    Error,
}

/// `gen<X>`: generation `for`/`if` over params, repeating a production `X`.
#[derive(Debug, Clone, PartialEq)]
pub struct Gen<X> {
    pub kind: GenKind<X>,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenKind<X> {
    For { pat: Pat, iter: Expr, body: Vec<X> },
    If { cond: Expr, then: Vec<X>, else_: Option<Box<GenElse<X>>> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenElse<X> {
    Block(Vec<X>),
    If(Gen<X>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BehaviorKind {
    Analog,
    Digital,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BehaviorBlock {
    pub kind: BehaviorKind,
    /// `analog<N, M>`: the generic names of the module, in order.
    pub names: Vec<Ident>,
    /// The module the block belongs to, with its generic arguments.
    pub target: Path,
    pub body: BehaviorBody,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BehaviorBody {
    Analog(Vec<Stmt>),
    Digital(Vec<DigitalItem>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DigitalItem {
    pub meta: Meta,
    pub kind: DigitalItemKind,
    pub span: Loc,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DigitalItemKind {
    Reaction(Reaction),
    Gen(Gen<DigitalItem>),
    /// A malformed item already reported by a diagnostic (`E0434`).
    Error,
}

/// `on event { statements and commits }`
#[derive(Debug, Clone, PartialEq)]
pub struct Reaction {
    pub event: Expr,
    pub body: Block,
}
