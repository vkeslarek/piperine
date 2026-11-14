//! Handles over bodies and files: [`SourceFile`] here, expression and
//! statement handles as the body tasks add them.

use std::path::Path as FsPath;

use super::super::body::{Arg, ExprKind, FieldInit, ExprNode, PatKind, PatNode, PathNode, PathSegment, StmtKind, StmtNode, TypeKind, TypeNode};
use super::super::design::Design;
use super::super::ids::{ExprId, FileId, PatId, PathId, StmtId, TypeId};
use super::super::resolution::Resolution;
use super::{Decl, Reflect};
use crate::pom::meta::Doc;
use crate::msl::{Span, ast};

/// A source file of a [`Design`]: its name, text and retained syntax tree.
#[derive(Clone, Copy)]
pub struct SourceFile<'d> {
    design: &'d Design,
    id: FileId,
}

impl<'d> SourceFile<'d> {
    pub(crate) fn new(design: &'d Design, id: FileId) -> Self {
        SourceFile { design, id }
    }

    pub fn id(self) -> FileId {
        self.id
    }

    pub(crate) fn design(self) -> &'d Design {
        self.design
    }

    /// The name it was registered under, or its file name on disk.
    pub fn name(self) -> &'d str {
        &self.design.file_data(self.id).name
    }

    /// The path it was read from, virtual or on disk.
    pub fn path(self) -> Option<&'d FsPath> {
        self.design.file_data(self.id).path.as_deref()
    }

    pub fn text(self) -> &'d str {
        &self.design.file_data(self.id).text
    }

    /// The syntax tree the parser returned, errors recovered.
    pub fn ast(self) -> &'d ast::File {
        &self.design.file_data(self.id).ast
    }

    pub fn is_prelude(self) -> bool {
        self.design.file_data(self.id).prelude
    }
}

impl PartialEq for SourceFile<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for SourceFile<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SourceFile({:?}, {})", self.id, self.name())
    }
}

impl Design {
    /// Every file, prelude sources first, then the root, then loaded files.
    pub fn files(&self) -> Vec<SourceFile<'_>> {
        (0..self.file_count()).map(|i| SourceFile::new(self, FileId(i as u32))).collect()
    }
}

/// The authored text of `span` in the file of the declaration `owner`.
fn text_in<'d>(owner: Decl<'d>, span: Span) -> &'d str {
    owner.file().text().get(span.range()).unwrap_or("")
}

/// An expression node.
#[derive(Clone, Copy)]
pub struct Expr<'d> {
    design: &'d Design,
    id: ExprId,
}

impl<'d> Expr<'d> {
    pub(crate) fn new(design: &'d Design, id: ExprId) -> Self {
        Expr { design, id }
    }

    pub fn id(self) -> ExprId {
        self.id
    }

    pub fn node(self) -> &'d ExprNode {
        self.design.expr_node(self.id)
    }

    pub fn kind(self) -> &'d ExprKind {
        &self.node().kind
    }

    pub fn span(self) -> Span {
        self.node().span
    }

    /// The nearest declaration that contains this expression.
    pub fn enclosing(self) -> Decl<'d> {
        Decl::new(self.design, self.node().owner)
    }

    /// The authored text of the expression.
    pub fn text(self) -> &'d str {
        text_in(self.enclosing(), self.span())
    }

    /// The direct sub-expressions, in source order. A form that holds a block
    /// lists the expressions of the block's statements; types, patterns and
    /// generic arguments are not expressions.
    pub fn children(self) -> Vec<Expr<'d>> {
        crate::pom::body::expr_children(self.kind(), self.design).into_iter().map(|c| Expr::new(self.design, c)).collect()
    }

    /// The expression that lists this one among its [`Expr::children`]; none
    /// when no expression contains it (a statement's own expression, a
    /// declaration's value).
    pub fn parent(self) -> Option<Expr<'d>> {
        self.design.expr_parent.get(self.id.index()).copied().flatten().map(|p| Expr::new(self.design, p))
    }

    /// The nearest statement that holds this expression or an expression
    /// above it: its own statement when a statement lists it among its
    /// [`Stmt::children`], else the one of its nearest ancestor that has one.
    pub fn stmt(self) -> Option<Stmt<'d>> {
        let mut at = Some(self);
        while let Some(e) = at {
            if let Some(s) = self.design.expr_stmt.get(e.id.index()).copied().flatten() {
                return Some(Stmt::new(self.design, s));
            }
            at = e.parent();
        }
        None
    }

    /// The call arguments of a call or a method call, in source order; empty
    /// for every other expression.
    pub fn args(self) -> Vec<Argument<'d>> {
        match self.kind() {
            ExprKind::Call { args, .. } | ExprKind::MethodCall { args, .. } => args.iter().map(|node| Argument { design: self.design, node }).collect(),
            _ => Vec::new(),
        }
    }

    /// The `name: value` initializers of a struct expression, of an `emit!`
    /// and of a call's param block (`R(a, b) { r: 1k }`), in source order;
    /// empty for every other expression.
    pub fn fields(self) -> Vec<FieldBinding<'d>> {
        match self.kind() {
            ExprKind::Struct { fields, .. } | ExprKind::Emit { fields, .. } => fields.iter().map(|node| FieldBinding { design: self.design, node }).collect(),
            ExprKind::Call { params: Some(fields), .. } => fields.iter().map(|node| FieldBinding { design: self.design, node }).collect(),
            _ => Vec::new(),
        }
    }

    /// The path this expression is, when it is a path expression (`s`, `a::X`).
    pub fn as_path(self) -> Option<Path<'d>> {
        match self.kind() {
            ExprKind::Path(p) => Some(Path::new(self.design, *p)),
            _ => None,
        }
    }

    /// What this expression names: a path's last segment, a struct literal's
    /// path, a field access or method call's member, a call's callee. Every
    /// other expression names nothing and answers `Unresolved`.
    pub fn resolution(self) -> Resolution {
        match self.kind() {
            ExprKind::Path(p) | ExprKind::Struct { path: p, .. } => Path::new(self.design, *p).resolution(),
            ExprKind::Field { res, .. } | ExprKind::MethodCall { res, .. } => *res,
            ExprKind::Call { callee, .. } => Expr::new(self.design, *callee).resolution(),
            _ => Resolution::Unresolved,
        }
    }

    /// The declaration [`Expr::resolution`] names, as a handle; none for an
    /// unresolved name, a built-in or a deferred one.
    pub fn target(self) -> Option<Decl<'d>> {
        match self.resolution() {
            Resolution::Decl(id) => Some(self.design.decl(id)),
            _ => None,
        }
    }

    /// The instance this call is, when it is classified as one.
    pub fn as_instance(self) -> Option<super::Instance<'d>> {
        match self.kind() {
            ExprKind::Call { class: crate::pom::resolution::CallClass::Instance { .. }, .. } => Some(super::Instance::new(self.design, self.id)),
            _ => None,
        }
    }
}

/// A call argument: positional, or `name: value`.
#[derive(Clone, Copy)]
pub struct Argument<'d> {
    design: &'d Design,
    node: &'d Arg,
}

impl<'d> Argument<'d> {
    /// The raw storage node.
    pub fn node(self) -> &'d Arg {
        self.node
    }

    /// The written name; none for a positional argument.
    pub fn name(self) -> Option<&'d str> {
        self.node.name.as_ref().map(|n| n.text.as_str())
    }

    pub fn value(self) -> Expr<'d> {
        self.design.expr(self.node.value)
    }

    /// The port, param or field the name links to, as classification set it;
    /// none for a positional argument and for a name nothing declares.
    pub fn target(self) -> Option<Decl<'d>> {
        match self.node.res {
            Resolution::Decl(id) => Some(self.design.decl(id)),
            _ => None,
        }
    }

    pub fn span(self) -> Span {
        self.node.span
    }
}

/// `name: value` of a struct expression, an `emit!` or an instance's param
/// block.
#[derive(Clone, Copy)]
pub struct FieldBinding<'d> {
    design: &'d Design,
    node: &'d FieldInit,
}

impl<'d> FieldBinding<'d> {
    /// The raw storage node.
    pub fn node(self) -> &'d FieldInit {
        self.node
    }

    pub fn name(self) -> &'d str {
        self.node.name.text.as_str()
    }

    pub fn value(self) -> Expr<'d> {
        self.design.expr(self.node.value)
    }

    /// The param, field or port the name links to, as classification set it;
    /// none for a name nothing declares.
    pub fn target(self) -> Option<Decl<'d>> {
        match self.node.res {
            Resolution::Decl(id) => Some(self.design.decl(id)),
            _ => None,
        }
    }

    /// Whether the field was written `name` for `name: name`.
    pub fn is_shorthand(self) -> bool {
        self.node.shorthand
    }

    pub fn span(self) -> Span {
        self.node.span
    }
}

impl PartialEq for Argument<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && std::ptr::eq(self.node, other.node)
    }
}

impl std::fmt::Debug for Argument<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Argument({:?})", self.node.span)
    }
}

impl PartialEq for FieldBinding<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && std::ptr::eq(self.node, other.node)
    }
}

impl std::fmt::Debug for FieldBinding<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FieldBinding({})", self.node.name.text)
    }
}

/// A statement node.
#[derive(Clone, Copy)]
pub struct Stmt<'d> {
    design: &'d Design,
    id: StmtId,
}

impl<'d> Stmt<'d> {
    pub(crate) fn new(design: &'d Design, id: StmtId) -> Self {
        Stmt { design, id }
    }

    pub fn id(self) -> StmtId {
        self.id
    }

    pub fn node(self) -> &'d StmtNode {
        self.design.stmt_node(self.id)
    }

    pub fn kind(self) -> &'d StmtKind {
        &self.node().kind
    }

    pub fn span(self) -> Span {
        self.node().span
    }

    pub fn enclosing(self) -> Decl<'d> {
        Decl::new(self.design, self.node().owner)
    }

    pub fn text(self) -> &'d str {
        text_in(self.enclosing(), self.span())
    }

    /// The doc comments written before the statement, one per comment.
    pub fn docs(self) -> &'d [Doc] {
        &self.node().meta.docs
    }

    /// The expressions this statement holds with no expression above them, in
    /// source order: a `let`'s initializer (and the expressions of its `else`
    /// block's statements), an expression statement's expression, both sides
    /// of a contribution, the place, value and delay of a commit. An item, a
    /// signal, a reaction and a lone `;` hold none; a nested block's
    /// statements are reached through the expression that owns the block.
    pub fn children(self) -> Vec<Expr<'d>> {
        crate::pom::body::stmt_children(self.design, self.id).into_iter().map(|e| Expr::new(self.design, e)).collect()
    }
}

/// A block of statements: the body of a function or a reaction.
// SPEC_DEVIATION: design.md names this handle `Block<'d>`, but `pom::Block` is already the
// storage type (`pub use body::*`). Reason: one public name cannot be both; the handle is
// exported as `BlockHandle`, as `QuantityRole` did for the other clash.
#[derive(Clone, Copy)]
pub struct BlockHandle<'d> {
    design: &'d Design,
    node: &'d crate::pom::body::Block,
}

impl<'d> BlockHandle<'d> {
    pub(crate) fn new(design: &'d Design, node: &'d crate::pom::body::Block) -> Self {
        BlockHandle { design, node }
    }

    /// The raw storage block.
    pub fn node(self) -> &'d crate::pom::body::Block {
        self.node
    }

    /// The statements in source order.
    pub fn stmts(self) -> Vec<Stmt<'d>> {
        self.node.stmts.iter().map(|s| Stmt::new(self.design, *s)).collect()
    }

    /// The block's value: the expression of a last statement written without
    /// a `;`.
    pub fn value(self) -> Option<Expr<'d>> {
        match self.node.stmts.last().map(|s| &self.design.stmt_node(*s).kind) {
            Some(StmtKind::Expr { expr, semi: false }) => Some(Expr::new(self.design, *expr)),
            _ => None,
        }
    }

    /// The range of the braces and everything between them.
    pub fn span(self) -> Span {
        self.node.span
    }
}

impl PartialEq for BlockHandle<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && std::ptr::eq(self.node, other.node)
    }
}

impl std::fmt::Debug for BlockHandle<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlockHandle({:?})", self.node.span)
    }
}

/// A pattern node.
#[derive(Clone, Copy)]
pub struct Pat<'d> {
    design: &'d Design,
    id: PatId,
}

impl<'d> Pat<'d> {
    pub(crate) fn new(design: &'d Design, id: PatId) -> Self {
        Pat { design, id }
    }

    pub fn id(self) -> PatId {
        self.id
    }

    pub fn node(self) -> &'d PatNode {
        self.design.pat_node(self.id)
    }

    pub fn kind(self) -> &'d PatKind {
        &self.node().kind
    }

    pub fn span(self) -> Span {
        self.node().span
    }

    pub fn enclosing(self) -> Decl<'d> {
        Decl::new(self.design, self.node().owner)
    }

    pub fn text(self) -> &'d str {
        text_in(self.enclosing(), self.span())
    }
}

/// A type node.
#[derive(Clone, Copy)]
pub struct Type<'d> {
    design: &'d Design,
    id: TypeId,
}

impl<'d> Type<'d> {
    pub(crate) fn new(design: &'d Design, id: TypeId) -> Self {
        Type { design, id }
    }

    pub fn id(self) -> TypeId {
        self.id
    }

    pub fn node(self) -> &'d TypeNode {
        self.design.type_node(self.id)
    }

    pub fn kind(self) -> &'d TypeKind {
        &self.node().kind
    }

    pub fn span(self) -> Span {
        self.node().span
    }

    pub fn enclosing(self) -> Decl<'d> {
        Decl::new(self.design, self.node().owner)
    }

    pub fn text(self) -> &'d str {
        text_in(self.enclosing(), self.span())
    }
}

/// A path node.
#[derive(Clone, Copy)]
pub struct Path<'d> {
    design: &'d Design,
    id: PathId,
}

impl<'d> Path<'d> {
    pub(crate) fn new(design: &'d Design, id: PathId) -> Self {
        Path { design, id }
    }

    pub fn id(self) -> PathId {
        self.id
    }

    pub fn node(self) -> &'d PathNode {
        self.design.path_node(self.id)
    }

    pub fn segments(self) -> &'d [PathSegment] {
        &self.node().segments
    }

    pub fn span(self) -> Span {
        self.node().span
    }

    pub fn enclosing(self) -> Decl<'d> {
        Decl::new(self.design, self.node().owner)
    }

    pub fn text(self) -> &'d str {
        text_in(self.enclosing(), self.span())
    }

    /// What the path names: the resolution of its last segment.
    pub fn resolution(self) -> Resolution {
        self.segments().last().map_or(Resolution::Unresolved, |s| s.res)
    }

    /// The declaration the path names, as a handle; none when the last
    /// segment is unresolved, a built-in or deferred.
    pub fn target(self) -> Option<Decl<'d>> {
        match self.resolution() {
            Resolution::Decl(id) => Some(self.design.decl(id)),
            _ => None,
        }
    }
}

impl PartialEq for Expr<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Expr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Expr({:?})", self.id)
    }
}

impl PartialEq for Stmt<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Stmt<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Stmt({:?})", self.id)
    }
}

impl PartialEq for Pat<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Pat<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pat({:?})", self.id)
    }
}

impl PartialEq for Type<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Type<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Type({:?})", self.id)
    }
}

impl PartialEq for Path<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Path<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Path({:?})", self.id)
    }
}


impl Design {
    /// A handle on the expression with this id.
    pub fn expr(&self, id: ExprId) -> Expr<'_> {
        Expr::new(self, id)
    }

    pub fn stmt(&self, id: StmtId) -> Stmt<'_> {
        Stmt::new(self, id)
    }

    pub fn pat(&self, id: PatId) -> Pat<'_> {
        Pat::new(self, id)
    }

    pub fn ty(&self, id: TypeId) -> Type<'_> {
        Type::new(self, id)
    }

    pub fn path(&self, id: PathId) -> Path<'_> {
        Path::new(self, id)
    }
}

/// The statements under `stmts`, each followed by those nested in it
/// (blocks, `if`, `match`, loops), in source order. Items inside a block are
/// declarations of their own and are not entered.
pub(crate) fn nested_stmts(design: &Design, stmts: &[StmtId]) -> Vec<StmtId> {
    let mut out = Vec::new();
    for s in stmts {
        walk_stmt(design, *s, &mut out);
    }
    out
}

fn walk_stmt(design: &Design, id: StmtId, out: &mut Vec<StmtId>) {
    out.push(id);
    match &design.stmt_node(id).kind {
        StmtKind::Expr { expr, .. } => walk_expr(design, *expr, out),
        StmtKind::Let { init, else_, .. } => {
            if let Some(e) = init {
                walk_expr(design, *e, out);
            }
            if let Some(b) = else_ {
                walk_block(design, b, out);
            }
        }
        StmtKind::Item(_)
        | StmtKind::Contribution { .. }
        | StmtKind::Commit { .. }
        | StmtKind::Empty
        | StmtKind::Signal(_)
        | StmtKind::Reaction(_) => {}
    }
}

fn walk_block(design: &Design, block: &crate::pom::body::Block, out: &mut Vec<StmtId>) {
    for s in &block.stmts {
        walk_stmt(design, *s, out);
    }
}

fn walk_expr(design: &Design, id: ExprId, out: &mut Vec<StmtId>) {
    crate::stack::deep(|| walk_expr_inner(design, id, out))
}

fn walk_expr_inner(design: &Design, id: ExprId, out: &mut Vec<StmtId>) {
    match &design.expr_node(id).kind {
        ExprKind::Block(b) | ExprKind::Loop { body: b } | ExprKind::While { body: b, .. } | ExprKind::For { body: b, .. } => {
            walk_block(design, b, out)
        }
        ExprKind::If { then, else_, .. } => {
            walk_block(design, then, out);
            if let Some(e) = else_ {
                walk_expr(design, *e, out);
            }
        }
        ExprKind::Match { arms, .. } => {
            for a in arms {
                walk_expr(design, a.body, out);
            }
        }
        ExprKind::Paren(inner) => walk_expr(design, *inner, out),
        _ => {}
    }
}

/// `lhs +> rhs;`
#[derive(Clone, Copy)]
pub struct Contribution<'d> {
    stmt: Stmt<'d>,
}

impl<'d> Contribution<'d> {
    pub(crate) fn new(design: &'d Design, id: StmtId) -> Self {
        Contribution { stmt: Stmt::new(design, id) }
    }

    pub fn stmt(self) -> Stmt<'d> {
        self.stmt
    }

    pub fn lhs(self) -> Expr<'d> {
        match self.stmt.kind() {
            StmtKind::Contribution { lhs, .. } => self.stmt.design.expr(*lhs),
            _ => unreachable!("Contribution handle over a non-contribution statement"),
        }
    }

    pub fn rhs(self) -> Expr<'d> {
        match self.stmt.kind() {
            StmtKind::Contribution { rhs, .. } => self.stmt.design.expr(*rhs),
            _ => unreachable!("Contribution handle over a non-contribution statement"),
        }
    }
}

/// `place <- value [after delay];`
#[derive(Clone, Copy)]
pub struct Commit<'d> {
    stmt: Stmt<'d>,
}

impl<'d> Commit<'d> {
    pub(crate) fn new(design: &'d Design, id: StmtId) -> Self {
        Commit { stmt: Stmt::new(design, id) }
    }

    pub fn stmt(self) -> Stmt<'d> {
        self.stmt
    }

    pub fn place(self) -> Expr<'d> {
        match self.stmt.kind() {
            StmtKind::Commit { place, .. } => self.stmt.design.expr(*place),
            _ => unreachable!("Commit handle over a non-commit statement"),
        }
    }

    pub fn value(self) -> Expr<'d> {
        match self.stmt.kind() {
            StmtKind::Commit { value, .. } => self.stmt.design.expr(*value),
            _ => unreachable!("Commit handle over a non-commit statement"),
        }
    }

    pub fn after(self) -> Option<Expr<'d>> {
        match self.stmt.kind() {
            StmtKind::Commit { after, .. } => after.map(|e| self.stmt.design.expr(e)),
            _ => unreachable!("Commit handle over a non-commit statement"),
        }
    }
}

impl PartialEq for Contribution<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.stmt == other.stmt
    }
}

impl std::fmt::Debug for Contribution<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Contribution({:?})", self.stmt.id())
    }
}

impl PartialEq for Commit<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.stmt == other.stmt
    }
}

impl std::fmt::Debug for Commit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Commit({:?})", self.stmt.id())
    }
}
