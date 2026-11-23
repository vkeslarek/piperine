//! Body nodes: every statement, expression, pattern, type and path as an
//! id-addressed node, one variant per syntax-tree variant, nothing desugared.
//! Children are ids into the design's arenas; `owner` is the nearest
//! enclosing declaration.

use serde::{Deserialize, Serialize};

pub use crate::msl::ast::{BehaviorKind, BinOp, Dir, Lit, LitKind, UnOp};
// SPEC_DEVIATION: the design re-exports `ast::Quantity` under its own name, but the
// reflection handle `Quantity` (design, Reflection) takes that name in `pom`.
// Reason: one public name cannot be both; the enum is exported as `QuantityRole`.
pub use crate::msl::ast::Quantity as QuantityRole;

use super::decl::Name;
use super::design::Design;
use super::ids::{DeclId, ExprId, PatId, PathId, StmtId, TypeId};
use super::meta::Meta;
use super::resolution::{CallClass, Resolution, StructClass};
use crate::msl::Span;

// ---- paths and generic arguments ----------------------------------------------------------

/// `a::b<T>::c`. A path's resolution is its last segment's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathNode {
    pub segments: Vec<PathSegment>,
    pub span: Span,
    pub owner: DeclId,
}

/// One segment; `self`, `Self`, `super` and `crate` are segments too.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathSegment {
    pub name: Name,
    pub args: Option<GenericArgsNode>,
    pub res: Resolution,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenericArgsNode {
    /// `<A, B>` or the turbofish `::<A, B>`.
    Angle { args: Vec<GenericArgNode>, turbofish: bool, span: Span },
    /// The `Fn(P) -> R` bound sugar.
    Paren { inputs: Vec<TypeId>, output: Option<TypeId>, span: Span },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GenericArgNode {
    Type(TypeId),
    /// A const argument: a literal or a `{ block }`.
    Const(ExprId),
}

// ---- types ---------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeNode {
    pub kind: TypeKind,
    pub span: Span,
    pub owner: DeclId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeKind {
    Path(PathId),
    /// `[T; N]`: `len` is a const expression.
    Array { elem: TypeId, len: ExprId },
    /// `(A, B)`; the empty tuple is `()`.
    Tuple(Vec<TypeId>),
    /// `flip T`
    Flip(TypeId),
    /// `fn(A, B) -> R`; `ret` is `None` for `fn()`.
    FnPtr { params: Vec<TypeId>, ret: Option<TypeId> },
    /// `_`
    Infer,
}

// ---- patterns ------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatNode {
    pub kind: PatKind,
    pub span: Span,
    pub owner: DeclId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PatKind {
    /// `_`
    Wild,
    /// `..`
    Rest,
    /// `x`, `x @ p`. `binding` is the `Binding`
    /// declaration the identifier introduces (the lowering cannot tell a
    /// binder from a constant name; resolution can). `res` is what a plain
    /// name resolves to when it names a constant, a unit variant or a unit
    /// struct (§1): then the pattern is a path and binds nothing. It stays
    /// `Unresolved` for a binder.
    Ident { name: Name, sub: Option<PatId>, binding: DeclId, res: Resolution },
    /// A literal, possibly negated.
    Lit(ExprId),
    /// A path naming a constant or a unit variant.
    Path(PathId),
    Tuple(Vec<PatId>),
    TupleStruct { path: PathId, pats: Vec<PatId> },
    Struct { path: PathId, fields: Vec<FieldPatNode>, rest: bool },
    Or(Vec<PatId>),
    Range { lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool },
    /// `[a, b, ..]`: an array pattern.
    Array(Vec<PatId>),
}

/// `name: pat` in a struct pattern. The shorthand `Point { x }` has no `pat`
/// and introduces the `Binding` named by `binding`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldPatNode {
    pub name: Name,
    pub pat: Option<PatId>,
    pub binding: Option<DeclId>,
    /// The field of the struct or variant that the name stands for.
    pub res: Resolution,
    pub span: Span,
}

// ---- expressions ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExprNode {
    pub kind: ExprKind,
    pub span: Span,
    pub owner: DeclId,
}

/// An argument: positional, or `name: value` after the positional ones. The
/// name links to the port, param or field it names once the callee is
/// classified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arg {
    pub name: Option<Name>,
    pub value: ExprId,
    pub res: Resolution,
    pub span: Span,
}

/// `name: value` in a struct expression, a template or an instance's param
/// block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldInit {
    pub name: Name,
    pub value: ExprId,
    /// Written `{ r }` for `{ r: r }`.
    pub shorthand: bool,
    pub res: Resolution,
    pub span: Span,
}

impl FieldInit {
    /// Whether the field was written `name` for `name: name`.
    pub fn is_shorthand(&self) -> bool {
        self.shorthand
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arm {
    pub meta: Meta,
    pub pat: PatId,
    pub guard: Option<ExprId>,
    pub body: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClosureParam {
    pub pat: PatId,
    pub ty: Option<TypeId>,
    pub span: Span,
}

/// `{ stmts }`, held by value by the node that owns it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockNode {
    pub stmts: Vec<StmtId>,
    pub span: Span,
}

/// One variant per `ast::ExprKind` variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExprKind {
    Lit(Lit),
    Path(PathId),
    Unary { op: UnOp, expr: ExprId },
    Binary { op: BinOp, lhs: ExprId, rhs: ExprId },
    /// `lhs = rhs` (`op: None`) or `lhs += rhs`.
    Assign { op: Option<BinOp>, lhs: ExprId, rhs: ExprId },
    Cast { expr: ExprId, ty: TypeId },
    /// `params` is the `{ name: value }` block of an instance.
    Call { callee: ExprId, args: Vec<Arg>, params: Option<Vec<FieldInit>>, class: CallClass },
    MethodCall { recv: ExprId, method: Name, generics: Option<GenericArgsNode>, args: Vec<Arg>, res: Resolution },
    /// `e.name`; a tuple index `e.0` has the digits as its name.
    Field { expr: ExprId, name: Name, res: Resolution },
    Index { expr: ExprId, index: ExprId },
    Range { lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool },
    Paren(ExprId),
    Tuple(Vec<ExprId>),
    Array(Vec<ExprId>),
    ArrayRepeat { elem: ExprId, len: ExprId },
    Struct { path: PathId, fields: Vec<FieldInit>, class: StructClass },
    Block(BlockNode),
    If { cond: ExprId, then: BlockNode, else_: Option<ExprId> },
    /// `let pat = expr` in a condition.
    Let { pat: PatId, expr: ExprId },
    Match { scrutinee: ExprId, arms: Vec<Arm> },
    While { cond: ExprId, body: BlockNode },
    Loop { body: BlockNode },
    For { pat: PatId, iter: ExprId, body: BlockNode },
    Break(Option<ExprId>),
    Continue,
    Return(Option<ExprId>),
    Closure { params: Vec<ClosureParam>, ret: Option<TypeId>, body: ExprId },
    /// `e?`
    Try(ExprId),
    Emit { stream: Lit, fields: Vec<FieldInit> },
    IncludeStr(Lit),
    /// A malformed expression already reported by a diagnostic.
    Error,
}

// ---- statements ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StmtNode {
    /// Outer doc comments and attributes before the statement.
    pub meta: Meta,
    pub kind: StmtKind,
    pub span: Span,
    pub owner: DeclId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StmtKind {
    /// `let pat: ty = init else { diverge };`
    Let { pat: PatId, ty: Option<TypeId>, init: Option<ExprId>, else_: Option<BlockNode> },
    /// `semi: false` on the last statement of a block is the block's value.
    Expr { expr: ExprId, semi: bool },
    /// An item written inside a block; the declaration is a child of the
    /// enclosing declaration.
    Item(DeclId),
    /// `lhs +> rhs;`
    Contribution { lhs: ExprId, rhs: ExprId },
    /// `place <- value [after delay];`
    Commit { place: ExprId, value: ExprId, after: Option<ExprId> },
    /// A lone `;`.
    Empty,
    /// `signal name: type = init;`: the `Signal` declaration.
    Signal(DeclId),
    /// `on event { ... }`: the `Reaction` declaration.
    Reaction(DeclId),
}

// ---- navigation ----------------------------------------------------------------------------

/// The direct sub-expressions of an expression, in source order: the one
/// enumeration of an `ExprKind`'s children that [`Expr::children`], the
/// parent table and `check()` all read. A form that holds a block lists the
/// expressions of the block's statements (see [`stmt_children`]) where the
/// block was written. Types, patterns and generic arguments are not
/// expressions and are not entered.
pub(crate) fn expr_children(kind: &ExprKind, design: &Design) -> Vec<ExprId> {
    let mut out = Vec::new();
    match kind {
        ExprKind::Lit(_) | ExprKind::Path(_) | ExprKind::Continue | ExprKind::Error | ExprKind::IncludeStr(_) => {}
        ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) | ExprKind::Try(expr) | ExprKind::Cast { expr, .. } | ExprKind::Field { expr, .. } => out.push(*expr),
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Assign { lhs, rhs, .. } => out.extend([*lhs, *rhs]),
        ExprKind::Call { callee, args, params, .. } => {
            out.push(*callee);
            out.extend(args.iter().map(|a| a.value));
            out.extend(params.iter().flatten().map(|f| f.value));
        }
        ExprKind::MethodCall { recv, args, .. } => {
            out.push(*recv);
            out.extend(args.iter().map(|a| a.value));
        }
        ExprKind::Index { expr, index } => out.extend([*expr, *index]),
        ExprKind::Range { lo, hi, .. } => out.extend(lo.iter().chain(hi)),
        ExprKind::Tuple(items) | ExprKind::Array(items) => out.extend(items),
        ExprKind::ArrayRepeat { elem, len } => out.extend([*elem, *len]),
        ExprKind::Struct { fields, .. } | ExprKind::Emit { fields, .. } => out.extend(fields.iter().map(|f| f.value)),
        ExprKind::Block(b) | ExprKind::Loop { body: b } => out.extend(block_exprs(design, b)),
        ExprKind::If { cond, then, else_ } => {
            out.push(*cond);
            out.extend(block_exprs(design, then));
            out.extend(else_);
        }
        ExprKind::Let { expr, .. } => out.push(*expr),
        ExprKind::Match { scrutinee, arms } => {
            out.push(*scrutinee);
            for a in arms {
                out.extend(a.guard);
                out.push(a.body);
            }
        }
        ExprKind::While { cond, body } => {
            out.push(*cond);
            out.extend(block_exprs(design, body));
        }
        ExprKind::For { iter, body, .. } => {
            out.push(*iter);
            out.extend(block_exprs(design, body));
        }
        ExprKind::Break(v) | ExprKind::Return(v) => out.extend(v),
        ExprKind::Closure { body, .. } => out.push(*body),
    }
    out
}

/// The expressions a statement holds without any expression above them: its
/// own, and those of the statements of a `let .. else` block.
pub(crate) fn stmt_children(design: &Design, id: StmtId) -> Vec<ExprId> {
    let mut out = stmt_own_exprs(design, id);
    if let StmtKind::Let { else_: Some(b), .. } = &design.stmt_node(id).kind {
        out.extend(block_exprs(design, b));
    }
    out
}

/// The expressions a statement itself holds, in source order; the statements
/// of a `let .. else` block hold their own.
pub(crate) fn stmt_own_exprs(design: &Design, id: StmtId) -> Vec<ExprId> {
    match &design.stmt_node(id).kind {
        StmtKind::Let { init, .. } => init.iter().copied().collect(),
        StmtKind::Expr { expr, .. } => vec![*expr],
        StmtKind::Contribution { lhs, rhs } => vec![*lhs, *rhs],
        StmtKind::Commit { place, value, after } => [*place, *value].into_iter().chain(*after).collect(),
        StmtKind::Item(_) | StmtKind::Empty | StmtKind::Signal(_) | StmtKind::Reaction(_) => Vec::new(),
    }
}

/// The children of every statement of a block, in order.
pub(crate) fn block_exprs(design: &Design, block: &BlockNode) -> Vec<ExprId> {
    block.stmts.iter().flat_map(|s| stmt_children(design, *s)).collect()
}

/// `id` and everything below it, pre-order.
pub(crate) fn preorder(design: &Design, id: ExprId, out: &mut Vec<ExprId>) {
    crate::stack::Stack::deep(|| {
        out.push(id);
        for c in expr_children(&design.expr_node(id).kind, design) {
            preorder(design, c, out);
        }
    })
}
