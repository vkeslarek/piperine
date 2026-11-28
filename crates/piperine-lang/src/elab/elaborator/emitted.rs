//! What the run records of each IR node it adds: its type, its continuity and its origin.

use piperine_ir::{Env, Expr, ExprId as LoweredExpr, Origin, Stmt, StmtId as LoweredStmt, ValueType};
use crate::pom::{ExprId};
use super::Elaboration;

impl<'d, 't> Elaboration<'d, 't> {
    /// Add an expression to the IR, remembering its type (the builder does not hand it back) and
    /// whether it is continuous (`events.rs`).
    pub(in crate::elab) fn emit(&mut self, kind: Expr, ty: ValueType, origin: Origin) -> LoweredExpr {
        self.state.values.expr_types.push(ty.clone());
        let continuous = self.continuous_kind(&kind);
        self.state.values.expr_continuous.push(continuous);
        self.state.values.expr_origins.push(origin.clone());
        self.state.builder.expr(kind, ty, origin)
    }

    /// Add a statement to the IR, remembering whether it reads a continuous value.
    pub(in crate::elab) fn stmt(&mut self, stmt: Stmt) -> LoweredStmt {
        let continuous = stmt.operands().iter().any(|e| self.continuous(*e));
        self.state.values.stmt_continuous.push(continuous);
        let id = self.state.builder.stmt(stmt.clone());
        self.state.values.divergences.note_statement(id, &stmt);
        id
    }

    /// The type of an expression already added.
    pub(in crate::elab) fn type_of(&self, expr: LoweredExpr) -> ValueType {
        self.state.values.expr_types[expr.index()].clone()
    }

    /// The origin of an expression: where its construct starts.
    pub(in crate::elab) fn expr_origin(&self, expr: ExprId) -> Origin {
        let node = self.state.design.expr_node(expr);
        self.state.locator.origin(self.state.design.decl_data(node.owner).file, node.span)
    }

    /// Whether the lowered expression `expr` is continuous.
    pub(in crate::elab) fn continuous(&self, expr: LoweredExpr) -> bool {
        self.state.values.expr_continuous.get(expr.index()).copied().unwrap_or(false)
    }

    /// Whether an expression of kind `kind`, whose operands are lowered already, is continuous.
    pub(in crate::elab) fn continuous_kind(&self, kind: &Expr) -> bool {
        let any = |es: &[LoweredExpr]| es.iter().any(|e| self.continuous(*e));
        match kind {
            Expr::Probe { .. } | Expr::Flow { .. } | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::Env(Env::Time | Env::Static | Env::Gmin | Env::SourceScale | Env::Freq) => true,
            Expr::Env(Env::Temperature | Env::TemperatureGiven | Env::Analysis(_)) | Expr::SessionParameter { .. } | Expr::Fired(_) | Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::ParamPrevious(_) | Expr::Others(_) | Expr::Resolve { .. } | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Previous(_) => false,
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::Table { x: expr, .. } | Expr::TableSlope { x: expr, .. } => self.continuous(*expr),
            Expr::CarrierElement { index, .. } | Expr::ElementPrevious { index, .. } | Expr::ElementChanged { index, .. } => self.continuous(*index),
            Expr::Binary { lhs, rhs, .. } => any(&[*lhs, *rhs]),
            Expr::Unwrap { is, value, .. } => any(&[*is, *value]),
            Expr::Same { a, b } => any(&[*a, *b]),
            Expr::LevelCast { expr, .. } => self.continuous(*expr),
            Expr::If { cond, then, otherwise } => any(&[*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => stmts.iter().any(|s| self.state.values.stmt_continuous.get(s.index()).copied().unwrap_or(false)) || value.is_some_and(|v| self.continuous(v)),
            Expr::Call { function, args } => self.state.functions.variants.continuous(*function) || any(args),
            Expr::Math { args, .. } => any(args),
            Expr::Method { receiver, args, .. } => self.continuous(*receiver) || any(args),
            Expr::Closure { body, .. } => self.continuous(*body),
            Expr::Limit { x, f, seed, .. } => any(&[*x, *f]) || seed.is_some_and(|s| self.continuous(s)),
            Expr::Stim { mag, phase, .. } => self.continuous(*mag) || phase.is_some_and(|p| self.continuous(p)),
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) => {
                any(&kind.operands()) || kind.statements().iter().any(|s| self.state.values.stmt_continuous.get(s.index()).copied().unwrap_or(false))
            }
        }
    }
}
