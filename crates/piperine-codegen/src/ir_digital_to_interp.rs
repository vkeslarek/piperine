//! IR → Device adapter for digital modules.
//!
//! Phase 1.5: takes an [`IrProgram`] + module name, converts the
//! [`crate::ir::IrDigitalBody`] into an `ElabBehaviorStmt` sequence inside a
//! synthetic [`ElabProgram`] and hands it off to `compile_digital_module`,
//! which walks the standard scanner + interpreter.

use crate::codegen::digital::{compile_digital_module, DigitalInterpreter};
use crate::codegen::CodegenError;
use crate::ir::IrModule;
use crate::ir::IrProgram;
use piperine_lang::elab::ir::{ElabBehavior, ElabBehaviorStmt, ElabProgram};
use piperine_lang::parse::ast::{BehaviorKind, EventSpec};

/// Lookup IrModule by name (mirrors analog version).
pub fn find_module<'a>(program: &'a IrProgram, name: &str) -> Option<&'a IrModule> {
    program.modules.iter().find(|m| m.name == name)
}

/// Lower a digital module body from the IR to a [`DigitalInterpreter`].
pub fn ir_digital_to_interp(
    program: &IrProgram,
    module_name: &str,
) -> Result<DigitalInterpreter, CodegenError> {
    let module = find_module(program, module_name)
        .ok_or_else(|| CodegenError::ModuleNotFound(module_name.to_string()))?;
    let body = module
        .digital
        .as_ref()
        .ok_or_else(|| CodegenError::BehaviorNotFound(module_name.to_string()))?;

    let elab_stmts = lower_stmts(&body.stmts);

    let mut elab = ElabProgram::new();
    elab.behaviors.push(ElabBehavior {
        kind: BehaviorKind::Digital,
        name: module_name.to_string(),
        body: elab_stmts,
    });

    compile_digital_module(&elab, module_name, 0)
}

fn lower_stmts(stmts: &[crate::ir::IrStmt]) -> Vec<ElabBehaviorStmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for s in stmts {
        if let Some(es) = lower_stmt(s) {
            out.push(es);
        }
    }
    out
}

fn lower_stmt(s: &crate::ir::IrStmt) -> Option<ElabBehaviorStmt> {
    use crate::ir::{IrEventKind, IrStmt};
    use piperine_lang::parse::ast::{BindOp, Expr as PExpr};

    match s {
        IrStmt::Assign { lval, expr, .. } | IrStmt::NonBlocking { lval, expr, .. } => {
            let dest = PExpr::Ident(lval.clone());
            let src = ir_expr_to_phdl(expr);
            Some(ElabBehaviorStmt::Bind { dest, op: BindOp::Force, src })
        }
        IrStmt::AnalogEvent { kind, body } => {
            let elab_spec = match kind {
                IrEventKind::Posedge(e) => EventSpec::Named {
                    name: "posedge".into(),
                    arg: ir_expr_to_phdl(e),
                },
                IrEventKind::Negedge(e) => EventSpec::Named {
                    name: "negedge".into(),
                    arg: ir_expr_to_phdl(e),
                },
                IrEventKind::Change(e) => EventSpec::Named {
                    name: "change".into(),
                    arg: ir_expr_to_phdl(e),
                },
                _ => return None,
            };
            let lowered = lower_stmts(body);
            if lowered.is_empty() {
                return None;
            }
            Some(ElabBehaviorStmt::Event {
                spec: elab_spec,
                guard: None,
                body: lowered,
            })
        }
        IrStmt::EventControl { spec, body } => {
            let elab_spec = match spec {
                crate::ir::IrEventSpec::Posedge(e) => EventSpec::Named {
                    name: "posedge".into(),
                    arg: ir_expr_to_phdl(e),
                },
                crate::ir::IrEventSpec::Negedge(e) => EventSpec::Named {
                    name: "negedge".into(),
                    arg: ir_expr_to_phdl(e),
                },
                crate::ir::IrEventSpec::Change(e) => EventSpec::Named {
                    name: "change".into(),
                    arg: ir_expr_to_phdl(e),
                },
                _ => return None,
            };
            if let Some(es) = lower_stmt(&body) {
                Some(ElabBehaviorStmt::Event {
                    spec: elab_spec,
                    guard: None,
                    body: vec![es],
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn ir_expr_to_phdl(ir: &crate::ir::IrExpr) -> piperine_lang::parse::ast::Expr {
    use piperine_lang::parse::ast::{BinaryOp, Expr as PExpr, Literal, UnaryOp};

    match ir {
        crate::ir::IrExpr::Real(v)         => PExpr::Literal(Literal::Real(*v)),
        crate::ir::IrExpr::Int(v)          => PExpr::Literal(Literal::Int(*v as u64)),
        crate::ir::IrExpr::Bool(b)         => PExpr::Literal(Literal::Bool(*b)),
        crate::ir::IrExpr::Param(name) | crate::ir::IrExpr::Var(name) => PExpr::Ident(name.clone()),
        crate::ir::IrExpr::BranchAccess { access, plus, minus } => PExpr::Call(
            Box::new(PExpr::Ident(access.clone())),
            vec![PExpr::Ident(plus.clone()), PExpr::Ident(minus.clone())],
        ),
        crate::ir::IrExpr::Call(name, args) => PExpr::Call(
            Box::new(PExpr::Ident(name.clone())),
            args.iter().map(ir_expr_to_phdl).collect(),
        ),
        crate::ir::IrExpr::Binary(op, a, b) => {
            let phdl_op = match op {
                crate::ir::IrBinOp::Add => BinaryOp::Add,
                crate::ir::IrBinOp::Sub => BinaryOp::Sub,
                crate::ir::IrBinOp::Mul => BinaryOp::Mul,
                crate::ir::IrBinOp::Div => BinaryOp::Div,
                crate::ir::IrBinOp::Rem => BinaryOp::Rem,
                _ => BinaryOp::Add,
            };
            PExpr::Binary(
                Box::new(ir_expr_to_phdl(a)),
                phdl_op,
                Box::new(ir_expr_to_phdl(b)),
            )
        }
        crate::ir::IrExpr::Unary(op, e) => {
            let phdl_op = match op {
                crate::ir::IrUnOp::Neg => UnaryOp::Neg,
                _ => UnaryOp::Not,
            };
            PExpr::Unary(phdl_op, Box::new(ir_expr_to_phdl(e)))
        }
        _ => PExpr::Literal(Literal::Bool(false)),
    }
}
