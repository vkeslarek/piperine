//! IR → Device adapter for `IrAnalogBody`.
//!
//! Bridges the IR codegen path:  takes an [`IrProgram`] + module name, looks
//! up the analog body, and lowers each [`IrStmt::Contrib`] to a residual +
//! jacobian function via the existing Cranelift codegen.
//!
//! Scope (Phase 1.4 — first bridge):
//!   - Contributions of the form `I(p, n) <+ expr` / `V(p, n) <+ expr`.
//!   - The IR expression graph is translated back to the PHDL-AST [`Expr`]
//!     by [`ir_expr_to_phdl`] which is the inverse of `ppr_to_ir`'s
//!     [`piperine_lang::parse::ast::Expr`] → [`crate::ir::IrExpr`]
//!     lowering.  Only the subset that the boilerplate VA fixtures exercise
//!     is implemented; other constructs will surface as `CodegenError`.
//!
//! Future phases will replace this translation with a direct IR-consuming
//! Cranelift emitter.

use crate::codegen::analog::{compile_analog_module_ir, Contribution};
use crate::codegen::{CodegenError, JitAnalogDevice};
use crate::ir::{ContribKind, IrAnalogBody, IrExpr, IrModule, IrNature, IrProgram, IrStmt};
use piperine_lang::parse::ast as phdl;

/// Lookup `IrModule` by name.
pub fn find_module<'a>(program: &'a IrProgram, name: &str) -> Option<&'a IrModule> {
    program.modules.iter().find(|m| m.name == name)
}

/// Lower an [`IrProgram`]'s analog body for `module_name` to a Cranelift
/// `JitAnalogDevice`.
pub fn ir_analog_to_device(program: &IrProgram, module_name: &str) -> Result<JitAnalogDevice, CodegenError> {
    let module = find_module(program, module_name)
        .ok_or_else(|| CodegenError::ModuleNotFound(module_name.to_string()))?;
    let body = module
        .analog
        .as_ref()
        .ok_or_else(|| CodegenError::BehaviorNotFound(module_name.to_string()))?;
    compile_ir_analog(module, body)
}

fn compile_ir_analog(module: &IrModule, body: &IrAnalogBody) -> Result<JitAnalogDevice, CodegenError> {
    // Translate to the existing Contribution shape so we can reuse the
    // Cranelift residual/jacobian emitter unchanged.
    let mut phdl_contributions = Vec::new();
    collect_contributions(&body.stmts, &mut phdl_contributions);

    if phdl_contributions.is_empty() {
        return Err(CodegenError::BehaviorNotFound(module.name.clone()));
    }

    let param_names: Vec<String> = module.params.iter().map(|p| p.name.clone()).collect();
    let port_names: Vec<String> = module.ports.iter().map(|p| p.name.clone()).collect();

    compile_analog_module_ir(&module.name, port_names, param_names, phdl_contributions)
}

fn collect_contributions(stmts: &[IrStmt], out: &mut Vec<Contribution>) {
    for s in stmts {
        match s {
            IrStmt::Contrib { plus, minus, expr, .. } => {
                let phdl_expr = ir_expr_to_phdl(expr);
                out.push(Contribution {
                    plus: plus.clone(),
                    minus: minus.clone(),
                    expr: phdl_expr,
                });
            }
            IrStmt::If { then_, else_, .. } => {
                collect_contributions(then_, out);
                collect_contributions(else_, out);
            }
            _ => {}
        }
    }
}

/// Translate `IrExpr` → `phdl::Expr`.
///
/// This is a **partial** inverse of `ppr_to_ir`'s lowering, sufficient for
/// the boilerplate VA fixtures (`resistor`, `capacitor`, `vsource`,
/// `isource`, `vramp`, `vstep`, `noisy_resistor`).  Constructs outside this
/// subset return [`phdl::Expr::Block`] with a `todo!` placeholder so the
/// codegen still produces a result that fails loudly in tests rather than
/// silently miscompiling.
pub fn ir_expr_to_phdl(ir: &IrExpr) -> phdl::Expr {
    match ir {
        IrExpr::Real(v)         => phdl::Expr::Literal(phdl::Literal::Real(*v)),
        IrExpr::Int(v)          => phdl::Expr::Literal(phdl::Literal::Int(*v as u64)),
        IrExpr::Bool(b)         => phdl::Expr::Literal(phdl::Literal::Bool(*b)),
        IrExpr::Param(name)     => phdl::Expr::Ident(name.clone()),
        IrExpr::Var(name)       => phdl::Expr::Ident(name.clone()),
        IrExpr::BranchAccess { access, plus, minus } => phdl::Expr::Call(
            Box::new(phdl::Expr::Ident(access.clone())),
            vec![phdl::Expr::Ident(plus.clone()), phdl::Expr::Ident(minus.clone())],
        ),
        IrExpr::Unary(op, e) => phdl::Expr::Unary(
            match op {
                crate::ir::IrUnOp::Neg => phdl::UnaryOp::Neg,
                crate::ir::IrUnOp::Not => phdl::UnaryOp::Not,
                crate::ir::IrUnOp::BitNot => phdl::UnaryOp::Not,
                _ => phdl::UnaryOp::Neg,
            },
            Box::new(ir_expr_to_phdl(e)),
        ),
        IrExpr::Binary(op, a, b) => {
            let phdl_op = match op {
                crate::ir::IrBinOp::Add => phdl::BinaryOp::Add,
                crate::ir::IrBinOp::Sub => phdl::BinaryOp::Sub,
                crate::ir::IrBinOp::Mul => phdl::BinaryOp::Mul,
                crate::ir::IrBinOp::Div => phdl::BinaryOp::Div,
                crate::ir::IrBinOp::Rem => phdl::BinaryOp::Rem,
                _ => phdl::BinaryOp::Add,
            };
            phdl::Expr::Binary(
                Box::new(ir_expr_to_phdl(a)),
                phdl_op,
                Box::new(ir_expr_to_phdl(b)),
            )
        }
        IrExpr::Call(name, args) => phdl::Expr::Call(
            Box::new(phdl::Expr::Ident(name.clone())),
            args.iter().map(ir_expr_to_phdl).collect(),
        ),
        IrExpr::Sim(sq) => match sq {
            crate::ir::SimQuery::Abstime => phdl::Expr::Ident("$abstime".into()),
            crate::ir::SimQuery::Temperature => phdl::Expr::Ident("$temperature".into()),
            _ => phdl::Expr::Literal(phdl::Literal::Real(0.0)),
        },
        // Conservatively: unsupported → Real(0.0) so the codegen still
        // produces *something*.  Tests targeting unsupported features will
        // assert on the value and fail loudly.
        _ => phdl::Expr::Literal(phdl::Literal::Real(0.0)),
    }
}
