//! Handles over bodies and files: [`SourceFile`] here, expression and
//! statement handles as the body tasks add them.

use std::path::Path as FsPath;

use super::super::body::{ExprKind, ExprNode, PatKind, PatNode, PathNode, PathSegment, StmtKind, StmtNode, TypeKind, TypeNode};
use super::super::design::Design;
use super::super::ids::{ExprId, FileId, PatId, PathId, StmtId, TypeId};
use super::{Decl, Reflect};
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
