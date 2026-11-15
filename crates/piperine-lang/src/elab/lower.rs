//! Value code: POM expressions lowered to the IR node of the same form.
//!
//! One `Scope` per owner (a module body, later a function) names the params, nodes and locals
//! an expression may read. A `const` read becomes its folded value; a param read stays a
//! `Param` read, never a number (a host `set` must reach the solver without re-elaboration).

use std::collections::BTreeMap;

use piperine_ir::{BinaryOp, Constant, Expr, ExprId as LoweredExpr, Local, Origin, Site, UnaryOp, ValueType};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::pom::{BinOp, DeclId, DeclKind, ExprId, ExprKind, FileId, Resolution, UnOp};
use crate::types::Ty;

/// What names the code being lowered: the owner's params, nodes and locals.
pub(super) struct Scope {
    pub(super) file: FileId,
    /// Param declaration to its index in the body's params.
    pub(super) params: BTreeMap<DeclId, u32>,
    /// Port or signal declaration to its index in the body's nodes.
    pub(super) nodes: BTreeMap<DeclId, u32>,
    pub(super) locals: Vec<Local>,
    pub(super) local_index: BTreeMap<DeclId, u32>,
    pub(super) limits: Vec<Site>,
    pub(super) stims: Vec<Site>,
}

impl Scope {
    pub(super) fn new(file: FileId) -> Self {
        Scope { file, params: BTreeMap::new(), nodes: BTreeMap::new(), locals: Vec::new(), local_index: BTreeMap::new(), limits: Vec::new(), stims: Vec::new() }
    }
}

/// The IR type of a type of the subset.
pub(super) struct ValueTypes;

impl ValueTypes {
    /// `None` for a type no value has in the IR: a node, a module.
    pub(super) fn of(ty: &Ty) -> Option<ValueType> {
        match ty {
            Ty::F64 => Some(ValueType::F64),
            Ty::Bool => Some(ValueType::Bool),
            Ty::Unit => Some(ValueType::Unit),
            Ty::Fn(params, ret) => {
                let params: Option<Vec<ValueType>> = params.iter().map(ValueTypes::of).collect();
                Some(ValueType::Fn(params?, Box::new(ValueTypes::of(ret)?)))
            }
            Ty::Node(_) | Ty::AnyNode | Ty::Module(_) => None,
        }
    }
}

impl Elaboration<'_, '_> {
    /// The origin of an expression: where its construct starts.
    pub(super) fn expr_origin(&self, expr: ExprId) -> Origin {
        let node = self.design.expr_node(expr);
        self.locator.origin(self.design.decl_data(node.owner).file, node.span)
    }

    /// `expr` lowered, or `None` after the diagnostics of what could not be.
    pub(super) fn lower_expr(&mut self, scope: &mut Scope, expr: ExprId) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let file = self.design.decl_data(node.owner).file;
        let origin = self.expr_origin(expr);
        let ty = self.types.ty(expr).and_then(ValueTypes::of);
        let Some(ty) = ty else {
            self.unsupported(file, node.span, "an expression the type checker gave no value type");
            return None;
        };
        match &node.kind {
            ExprKind::Lit(_) => match self.consts.value(expr, &Bindings::new()) {
                Ok(value) => Some(self.constant_expr(value, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a literal");
                    None
                }
            },
            ExprKind::Paren(inner) => self.lower_expr(scope, *inner),
            ExprKind::Path(_) => self.lower_path(scope, expr, ty, origin),
            ExprKind::Unary { op, expr: operand } => {
                let op = match op {
                    UnOp::Neg => UnaryOp::Neg,
                    UnOp::Not => UnaryOp::Not,
                    UnOp::Deref | UnOp::Ref => {
                        self.unsupported(file, node.span, "a reference or a dereference");
                        return None;
                    }
                };
                let operand = self.lower_expr(scope, *operand)?;
                Some(self.ir.expr(Expr::Unary { op, expr: operand }, ty, origin))
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let (lhs, rhs) = (self.lower_expr(scope, *lhs), self.lower_expr(scope, *rhs));
                let (lhs, rhs) = (lhs?, rhs?);
                let Some(op) = Self::binary_op(*op) else {
                    self.unsupported(file, node.span, "this operator");
                    return None;
                };
                Some(self.ir.expr(Expr::Binary { op, lhs, rhs }, ty, origin))
            }
            _ => {
                self.unsupported(file, node.span, "this expression form");
                None
            }
        }
    }

    fn binary_op(op: BinOp) -> Option<BinaryOp> {
        Some(match op {
            BinOp::Add => BinaryOp::Add,
            BinOp::Sub => BinaryOp::Sub,
            BinOp::Mul => BinaryOp::Mul,
            BinOp::Div => BinaryOp::Div,
            BinOp::Lt => BinaryOp::Lt,
            BinOp::Le => BinaryOp::Le,
            BinOp::Gt => BinaryOp::Gt,
            BinOp::Ge => BinaryOp::Ge,
            BinOp::Eq => BinaryOp::Eq,
            BinOp::Ne => BinaryOp::Ne,
            BinOp::And => BinaryOp::And,
            BinOp::Or => BinaryOp::Or,
            BinOp::Rem | BinOp::BitXor | BinOp::BitAnd | BinOp::BitOr | BinOp::Shl | BinOp::Shr => return None,
        })
    }

    /// A folded value as the IR expression of its type.
    pub(super) fn constant_expr(&mut self, value: Constant, origin: Origin) -> LoweredExpr {
        match value {
            Constant::Real(r) => self.ir.expr(Expr::Real(r), ValueType::F64, origin),
            Constant::Bool(b) => self.ir.expr(Expr::Bool(b), ValueType::Bool, origin),
        }
    }

    /// A name: a `const` (folded), a param of the body (a read), a local.
    fn lower_path(&mut self, scope: &mut Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(decl) = res else {
            self.unsupported(scope.file, node.span, "a built-in name used as a value");
            return None;
        };
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(_) => match self.consts.constant(decl) {
                Ok(value) => Some(self.constant_expr(value, origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `const`");
                    None
                }
            },
            DeclKind::Param(_) => match scope.params.get(&decl) {
                Some(index) => Some(self.ir.expr(Expr::Param(*index), ty, origin)),
                None => {
                    self.unsupported(scope.file, node.span, "a param of another module");
                    None
                }
            },
            DeclKind::FnParam(_) | DeclKind::Binding(_) => match scope.local_index.get(&decl) {
                Some(index) => Some(self.ir.expr(Expr::Local(*index), ty, origin)),
                None => {
                    self.unsupported(scope.file, node.span, "a name with no value here");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, node.span, "this name as a value");
                None
            }
        }
    }
}
