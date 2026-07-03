//! POM behavioral nodes — [`Behavior`] (an `analog`/`digital` block), its
//! statements, and [`Function`]/[`ImplBlock`] (value-layer computation).

use crate::value::Value;
use crate::parse::ast::{BehaviorKind, BindOp, EventSpec, Pattern};
use crate::pom::net_type::TypeRef;
use crate::pom::node::Kind;
use crate::pom::traits::{Kinded, Named};

#[derive(Debug, Clone)]
pub enum BehaviorStmt {
    VarDecl { name: String, ty: crate::pom::ValueType, default: Option<crate::parse::ast::Expr> },
    Bind { dest: crate::parse::ast::Expr, op: BindOp, src: crate::parse::ast::Expr },
    If { cond: crate::parse::ast::Expr, then_body: Vec<BehaviorStmt>, else_body: Option<Vec<BehaviorStmt>> },
    Match { expr: crate::parse::ast::Expr, arms: Vec<MatchArm> },
    Event { spec: EventSpec, guard: Option<crate::parse::ast::Expr>, body: Vec<BehaviorStmt> },
    Return(crate::parse::ast::Expr),
    Diagnostic { sys: String, args: Vec<crate::parse::ast::Expr> },
    Expr(crate::parse::ast::Expr),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: Pattern,
    pub body: Vec<BehaviorStmt>,
}

impl MatchArm {
    pub fn pattern(&self) -> &Pattern { &self.pat }
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }
}

impl BehaviorStmt {
    /// Immutable visit of every expression in this statement, pre-order.
    /// See [`crate::parse::ast::Expr::walk`].
    pub fn walk_exprs(&self, f: &mut impl FnMut(&crate::parse::ast::Expr) -> crate::parse::ast::Walk) {
        
        match self {
            BehaviorStmt::VarDecl { default, .. } => {
                if let Some(e) = default { e.walk(f); }
            }
            BehaviorStmt::Bind { dest, src, .. } => { dest.walk(f); src.walk(f); }
            BehaviorStmt::If { cond, then_body, else_body } => {
                cond.walk(f);
                then_body.iter().for_each(|s| s.walk_exprs(f));
                if let Some(b) = else_body { b.iter().for_each(|s| s.walk_exprs(f)); }
            }
            BehaviorStmt::Match { expr, arms } => {
                expr.walk(f);
                arms.iter().for_each(|a| a.body.iter().for_each(|s| s.walk_exprs(f)));
            }
            BehaviorStmt::Event { spec, guard, body } => {
                spec.walk_exprs(f);
                if let Some(g) = guard { g.walk(f); }
                body.iter().for_each(|s| s.walk_exprs(f));
            }
            BehaviorStmt::Return(e) | BehaviorStmt::Expr(e) => e.walk(f),
            BehaviorStmt::Diagnostic { args, .. } => args.iter().for_each(|a| a.walk(f)),
        }
    }

    /// Mutable visit of every expression in this statement, pre-order.
    /// See [`crate::parse::ast::Expr::walk_mut`].
    pub fn walk_exprs_mut(&mut self, f: &mut impl FnMut(&mut crate::parse::ast::Expr) -> crate::parse::ast::Walk) {
        
        match self {
            BehaviorStmt::VarDecl { default, .. } => {
                if let Some(e) = default { e.walk_mut(f); }
            }
            BehaviorStmt::Bind { dest, src, .. } => { dest.walk_mut(f); src.walk_mut(f); }
            BehaviorStmt::If { cond, then_body, else_body } => {
                cond.walk_mut(f);
                then_body.iter_mut().for_each(|s| s.walk_exprs_mut(f));
                if let Some(b) = else_body { b.iter_mut().for_each(|s| s.walk_exprs_mut(f)); }
            }
            BehaviorStmt::Match { expr, arms } => {
                expr.walk_mut(f);
                arms.iter_mut().for_each(|a| a.body.iter_mut().for_each(|s| s.walk_exprs_mut(f)));
            }
            BehaviorStmt::Event { spec, guard, body } => {
                spec.walk_exprs_mut(f);
                if let Some(g) = guard { g.walk_mut(f); }
                body.iter_mut().for_each(|s| s.walk_exprs_mut(f));
            }
            BehaviorStmt::Return(e) | BehaviorStmt::Expr(e) => e.walk_mut(f),
            BehaviorStmt::Diagnostic { args, .. } => args.iter_mut().for_each(|a| a.walk_mut(f)),
        }
    }

    /// Visit every sub-statement in this statement's children (pre-order,
    /// recursive). The callback fires for each direct child statement —
    /// `If` bodies, `Match` arms, `Event` bodies. The callback does *not*
    /// fire for `self`; callers that want to visit the root too should call
    /// `f(self)` before `self.walk_stmts(f)`.
    pub fn walk_stmts(&self, f: &mut impl FnMut(&BehaviorStmt)) {
        match self {
            BehaviorStmt::If { then_body, else_body, .. } => {
                for s in then_body { f(s); s.walk_stmts(f); }
                if let Some(b) = else_body {
                    for s in b { f(s); s.walk_stmts(f); }
                }
            }
            BehaviorStmt::Match { arms, .. } => {
                for arm in arms {
                    for s in &arm.body { f(s); s.walk_stmts(f); }
                }
            }
            BehaviorStmt::Event { body, .. } => {
                for s in body { f(s); s.walk_stmts(f); }
            }
            _ => {}
        }
    }
}

/// A behavior block inside a module (analog or digital).
#[derive(Debug, Clone)]
pub struct Behavior {
    /// Behavior block name.
    pub name: String,
    /// Whether this is an analog or digital block.
    pub kind: BehaviorKind,
    /// The statements making up the behavior body.
    pub body: Vec<BehaviorStmt>,
}

impl Behavior {
    /// Construct a new Behavior (used by the elaborator and codegen).
    #[doc(hidden)]
    pub fn new(name: String, kind: BehaviorKind, body: Vec<BehaviorStmt>) -> Self {
        Self { name, kind, body }
    }

    /// The behavior block name.
    pub fn name(&self) -> &str { &self.name }
    /// The behavior kind (analog or digital).
    pub fn kind(&self) -> &BehaviorKind { &self.kind }
    /// The statements inside the behavior block.
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }

    /// Returns `true` if this is an `analog` behavior block.
    pub fn is_analog(&self) -> bool { matches!(self.kind, BehaviorKind::Analog) }
    /// Returns `true` if this is a `digital` behavior block.
    pub fn is_digital(&self) -> bool { matches!(self.kind, BehaviorKind::Digital) }
}

impl Named for Behavior { fn name(&self) -> &str { self.name() } }
impl Kinded for Behavior { fn kind(&self) -> Kind { Kind::Behavior } }

// ─────────────────────────────── Function ────────────────────────────────────

/// A value-layer function definition.
#[derive(Debug, Clone)]
pub struct Function {
    /// Function name.
    pub name: String,
    /// Parameter names and types.
    pub params: Vec<(String, TypeRef)>,
    /// Return type.
    pub ret: TypeRef,
    /// Function body statements.
    pub body: Vec<BehaviorStmt>,
    /// Whether this function is generic (has type or const parameters).
    /// Generic functions are retained for reflection but not lowered into
    /// the IR until monomorphized at a call site.
    pub is_generic: bool,
}

impl Function {
    /// The function name.
    pub fn name(&self) -> &str { &self.name }
    /// The function parameters (name, type).
    pub fn params(&self) -> &[(String, TypeRef)] { &self.params }
    /// The function return type.
    pub fn ret(&self) -> &TypeRef { &self.ret }
    /// The function body statements.
    pub fn body(&self) -> &[BehaviorStmt] { &self.body }
    /// Whether this function is generic (not lowerable until monomorphized).
    pub fn is_generic(&self) -> bool { self.is_generic }
}

impl Named for Function { fn name(&self) -> &str { self.name() } }

// ─────────────────────────────── ImplBlock ───────────────────────────────────

/// An `impl` block — associates methods with a type, optionally gated by a capability.
#[derive(Debug, Clone)]
pub struct ImplBlock {
    /// Optional capability gate (e.g. `analog`, `digital`).
    pub capability: Option<String>,
    /// The type being implemented.
    pub ty: String,
    /// Constant generic arguments of the type.
    pub const_args: Vec<Value>,
    /// Methods defined in this impl block.
    pub methods: Vec<Function>,
}

impl ImplBlock {
    /// The capability gate, if any.
    pub fn capability(&self) -> Option<&str> { self.capability.as_deref() }
    /// The type being implemented.
    pub fn ty(&self) -> &str { &self.ty }
    /// All methods in this impl block.
    pub fn methods(&self) -> &[Function] { &self.methods }
}
