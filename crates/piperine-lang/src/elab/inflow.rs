//! `p.inflow()` (part III §6 rule 5, G.59): the flow entering the module at its port `p`, its
//! children's branches included, measured by an ammeter.
//!
//! A module that reads `p.inflow()` gets an inside node `p.inflow` (role `Signal`) and a branch
//! `(p, p.inflow)` that carries the constant potential 0. Every end of a branch and every child
//! connection the module writes on `p` is the inside node (`Frame::nodes` maps `p` to it), so all
//! the flow the module and its children draw at `p` goes through the ammeter. The read is the
//! flow of that branch: `Expr::Flow`, which differentiates like any probe. The branch is made here,
//! not written in the source, so the merge of zero-potential branches (`structural.rs`) never
//! sees it: its flow is read.

use piperine_ir::{Addend, Branch, BranchKind, BodyNode, Constant, Expr, ExprId as LoweredExpr, Inflow, Operator, Origin, QuantityRole, Real, Stmt, StmtId as LoweredStmt, Terminal, ValueType};

use super::body::NodePlan;
use super::elaborator::Elaboration;
use super::lower::Scope;
use super::selectors::ContributionRecord;
use crate::pom::{DeclId, DeclKind, ExprKind, Resolution};
use crate::types::Builtin;

impl Elaboration<'_, '_> {
    /// The ammeter of each port of `module` whose `inflow()` the module reads: its inside node and
    /// its zero-potential branch, added to the body, and the statements that drive the branches.
    /// Run right after the nodes are lowered, before any name is read.
    pub(super) fn lower_ammeters(&mut self, scope: &mut Scope, module: DeclId) -> Vec<LoweredStmt> {
        self.ammeters.clear();
        let mut stmts = Vec::new();
        for port in self.ports_read_by_inflow(module) {
            let Some(Terminal::Node(port_node)) = self.frames[0].nodes.get(&port).copied() else { continue };
            let (spec, plan) = {
                let outer = &self.structure.nodes[port_node as usize];
                let name = format!("{}.inflow", outer.0.name);
                let origin = outer.0.origin.clone();
                let nature = outer.0.nature;
                (BodyNode { name: name.clone(), nature, role: piperine_ir::NodeRole::Signal, init: None, origin: origin.clone() }, NodePlan { name, role: piperine_ir::NodeRole::Signal, nature, init: None, origin })
            };
            let origin = spec.origin.clone();
            self.structure.nodes.push((spec, plan));
            let inside = Terminal::Node(self.structure.nodes.len() as u32 - 1);
            scope.branches.push(Branch { plus: Terminal::Node(port_node), minus: inside, kind: BranchKind::Potential });
            let branch = scope.branches.len() as u32 - 1;
            let data = self.design.decl_data(port);
            scope.contributions.push(ContributionRecord { branch, quantity: QuantityRole::Potential, conditions: Vec::new(), file: data.file, span: data.span });
            let zero = self.constant_expr(Constant::Real(Real::new(0.0)), ValueType::F64, origin);
            stmts.push(self.stmt(Stmt::Contribute { branch, quantity: QuantityRole::Potential, addends: vec![Addend { operator: Operator::Direct, negated: false, value: zero }] }));
            self.structure.inflows.push(Inflow { port: port_node, branch });
            self.frames[0].nodes.insert(port, inside);
            self.ammeters.insert(port, branch);
        }
        stmts
    }

    /// `p.inflow()` as a value: the flow of the ammeter branch of `p`.
    pub(super) fn lower_inflow(&mut self, scope: &Scope, receiver: crate::pom::ExprId, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(receiver).span;
        let branch = match &self.design.expr_node(self.peel_parens(receiver)).kind {
            ExprKind::Path(path) if scope.frame == 0 => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(port)) => self.ammeters.get(&port).copied(),
                _ => None,
            },
            _ => None,
        };
        let Some(branch) = branch else {
            self.unsupported(scope.file, span, "`inflow()` of a port of a module that is placed in an expression");
            return None;
        };
        Some(self.emit(Expr::Flow { branch }, ValueType::F64, origin))
    }

    /// The ports of `module` that an `inflow()` the module writes names, in first-read order.
    fn ports_read_by_inflow(&self, module: DeclId) -> Vec<DeclId> {
        let mut ports = Vec::new();
        for expr in self.module_exprs(module) {
            let ExprKind::MethodCall { recv, .. } = &self.design.expr_node(expr).kind else { continue };
            if self.types.builtin(expr) != Some(Builtin::Inflow) {
                continue;
            }
            let ExprKind::Path(path) = &self.design.expr_node(self.peel_parens(*recv)).kind else { continue };
            if let Some(Resolution::Decl(port)) = self.design.path_node(*path).segments.last().map(|s| s.res)
                && matches!(self.design.decl_data(port).kind, DeclKind::Port(_))
                && !ports.contains(&port)
            {
                ports.push(port);
            }
        }
        ports
    }
}
