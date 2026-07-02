//! IR → Device adapter for digital modules.
//!
//! Phase 1.5: takes an [`IrProgram`] + module name, converts the
//! [`piperine_codegen::ir::IrDigitalBody`] into an `BehaviorStmt` sequence inside a
//! synthetic [`Design`] and hands it off to `compile_digital_module`,
//! which walks the standard scanner + interpreter.

use crate::runtime::digital::{compile_digital_module, DigitalInterpreter};
use piperine_codegen::codegen::inline::inline_user_calls;
use piperine_codegen::ir::{IrModule, IrProgram, IrStmt};
use piperine_codegen::CodegenError;
use crate::pom::{Behavior, BehaviorStmt, Design, Module};
use crate::parse::ast::{BehaviorKind, EventSpec};

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
        .ok_or_else(|| CodegenError::NoDigitalBody(module_name.to_string()))?;

    // GAPS §D.5 — inline user fn calls in the body before validation +
    // lowering so that user-defined helpers like `fn next_state(...)` are
    // substituted into the body. The interpreter then sees a body with
    // no opaque user-call nodes.
    let mut stmts: Vec<IrStmt> = body.stmts.clone();
    for stmt in &mut stmts {
        inline_stmt_user_calls(stmt, program, module).map_err(CodegenError::InlineError)?;
    }

    // GAPS §A.4 + §A.5 — validate the (now-inlined) body up-front so the
    // wrong-op fallbacks in `ir_expr_to_phdl` cannot be silently relied
    // on. The interpreter cannot currently execute shifts / powers /
    // reductions correctly, so reject them loudly.
    for stmt in &stmts {
        validate_ir_digital_stmt(stmt)?;
    }

    let elab_stmts = lower_stmts(&stmts);

    let mut elab = Design::new();
    // Build a synthetic Module with the behavior attached, so
    // `compile_digital_module` can find it.
    let behavior = Behavior::new(
        module_name.to_string(),
        BehaviorKind::Digital,
        elab_stmts,
    );
    let module = Module::new(
        module_name.to_string(),
        vec![], vec![], vec![], vec![], vec![],
        vec![behavior],
    );
    elab.insert_module(module_name.to_string(), module);

    compile_digital_module(&elab, module_name, 0)
}

/// Walk an IrStmt and inline user calls into every expression it
/// carries. Owned-mutates the stmt in place so the IR's recursive
/// structure is preserved.
fn inline_stmt_user_calls(
    stmt: &mut IrStmt,
    program: &IrProgram,
    module: &IrModule,
) -> Result<(), String> {
    use piperine_codegen::ir::IrStmt;
    match stmt {
        IrStmt::Assign { expr, .. }
        | IrStmt::NonBlocking { expr, .. }
        | IrStmt::Contrib { expr, .. }
        | IrStmt::Force { expr, .. }
        | IrStmt::ContinuousAssign { expr, .. }
        | IrStmt::ProcAssign { expr, .. }
        | IrStmt::Return(Some(expr)) => {
            *expr = inline_user_calls(program, module, expr)?;
        }
        IrStmt::If { cond, then_, else_, .. } => {
            *cond = inline_user_calls(program, module, cond)?;
            for s in then_.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
            for s in else_.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
        }
        IrStmt::Case { discriminant, arms, default, .. } => {
            *discriminant = inline_user_calls(program, module, discriminant)?;
            for (e, body) in arms.iter_mut() {
                *e = inline_user_calls(program, module, e)?;
                for s in body.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
            }
            for s in default.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
        }
        IrStmt::For { start, end, step, body, .. } => {
            *start = inline_user_calls(program, module, start)?;
            *end = inline_user_calls(program, module, end)?;
            *step = inline_user_calls(program, module, step)?;
            for s in body.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
        }
        IrStmt::While { cond, body, .. } => {
            *cond = inline_user_calls(program, module, cond)?;
            for s in body.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
        }
        IrStmt::Repeat { count, body, .. } => {
            *count = inline_user_calls(program, module, count)?;
            for s in body.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
        }
        IrStmt::AnalogEvent { body, .. } => {
            for s in body.iter_mut() { inline_stmt_user_calls(s, program, module)?; }
        }
        IrStmt::EventControl { body, .. } => {
            inline_stmt_user_calls(body.as_mut(), program, module)?;
        }
        IrStmt::Delay { body, .. } => {
            inline_stmt_user_calls(body.as_mut(), program, module)?;
        }
        IrStmt::Wait { cond, body, .. } => {
            *cond = inline_user_calls(program, module, cond)?;
            inline_stmt_user_calls(body.as_mut(), program, module)?;
        }
        // No expression payloads to inline in:
        // VarDecl, BoundStep, Finish, Discontinuity, Diagnostic, Return(None),
        // Fork, Disable, Trigger, ProcDeassign.
        _ => {}
    }
    Ok(())
}

/// Reject constructs the digital interpreter cannot currently handle
/// correctly. The old path silently mapped them to `Add`/`Not`, which gave
/// wrong values for non-trivial digital logic.
///
/// Fail-loud list (GAPS §A.4 + §A.5):
/// - `Pow`, `Shl`, `Shr`, `AShl`, `AShr` — no PHDL `BinaryOp` mapping.
/// - `BitNot`, `RedAnd`, `RedNand`, `RedOr`, `RedNor`, `RedXor`, `RedXnor`
///   — reductions need bitwidth (GAPS Part B/I).
fn validate_ir_digital_stmt(stmt: &piperine_codegen::ir::IrStmt) -> Result<(), CodegenError> {
    use piperine_codegen::ir::IrStmt;
    match stmt {
        IrStmt::Assign { expr, .. } | IrStmt::NonBlocking { expr, .. } => {
            validate_ir_digital_expr(expr)
        }
        IrStmt::If { cond, then_, else_, .. } => {
            validate_ir_digital_expr(cond)?;
            for s in then_ { validate_ir_digital_stmt(s)?; }
            for s in else_ { validate_ir_digital_stmt(s)?; }
            Ok(())
        }
        IrStmt::Case { discriminant, arms, default, .. } => {
            validate_ir_digital_expr(discriminant)?;
            for (_, body) in arms {
                for s in body { validate_ir_digital_stmt(s)?; }
            }
            for s in default { validate_ir_digital_stmt(s)?; }
            Ok(())
        }
        IrStmt::AnalogEvent { body, .. } => {
            for s in body { validate_ir_digital_stmt(s)?; }
            Ok(())
        }
        IrStmt::EventControl { body, .. } => validate_ir_digital_stmt(body),
        _ => Ok(()),
    }
}

/// Reject constructs the digital interpreter cannot handle correctly
/// (shifts, powers, bitwise reductions). Returns an error for unsupported ops.
fn validate_ir_digital_expr(e: &piperine_codegen::ir::IrExpr) -> Result<(), CodegenError> {
    use piperine_codegen::ir::{IrBinOp, IrExpr, IrUnOp};
    match e {
        IrExpr::Real(_) | IrExpr::Int(_) | IrExpr::Bool(_) | IrExpr::String(_)
        | IrExpr::Quad(_) | IrExpr::Param(_) | IrExpr::Var(_) | IrExpr::StateRef(_)
        | IrExpr::Sim(_) => Ok(()),
        IrExpr::BranchAccess { .. } => Ok(()),
        IrExpr::Unary(op, x) => {
            match op {
                IrUnOp::Neg | IrUnOp::Not => validate_ir_digital_expr(x),
                // BitNot and all reduction operators need width / bit-level
                // semantics; the interpreter cannot lower them correctly
                // today (GAPS §A.5, §B/I).
                _ => Err(CodegenError::Unsupported(format!(
                    "unary operator {op:?} in digital block (GAPS §A.5)"
                ))),
            }
        }
        IrExpr::Binary(op, a, b) => {
            match op {
                IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr
                | IrBinOp::AShl | IrBinOp::AShr => {
                    Err(CodegenError::Unsupported(format!(
                        "operator {op:?} in digital block (GAPS §A.4)"
                    )))
                }
                _ => {
                    validate_ir_digital_expr(a)?;
                    validate_ir_digital_expr(b)
                }
            }
        }
        IrExpr::Select(c, t, f) => {
            validate_ir_digital_expr(c)?;
            validate_ir_digital_expr(t)?;
            validate_ir_digital_expr(f)
        }
        IrExpr::Call(_, args) => {
            for a in args { validate_ir_digital_expr(a)?; }
            Ok(())
        }
        _ => Err(CodegenError::Unsupported(format!(
            "{e:?} in digital block"
        ))),
    }
}

/// Lower a slice of IR statements to elaborated [`BehaviorStmt`]s, dropping
/// unsupported construct variants.
fn lower_stmts(stmts: &[piperine_codegen::ir::IrStmt]) -> Vec<BehaviorStmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for s in stmts {
        if let Some(es) = lower_stmt(s) {
            out.push(es);
        }
    }
    out
}

/// Lower a single IR statement to an optional [`BehaviorStmt`], returning
/// `None` for constructs that are not representable in the digital interpreter.
fn lower_stmt(s: &piperine_codegen::ir::IrStmt) -> Option<BehaviorStmt> {
    use piperine_codegen::ir::{IrEventKind, IrStmt};
    use crate::parse::ast::{BindOp, Expr as PExpr};

    match s {
        IrStmt::Assign { lval, expr, .. } | IrStmt::NonBlocking { lval, expr, .. } => {
            let dest = PExpr::Ident(lval.clone());
            let src = ir_expr_to_phdl(expr);
            Some(BehaviorStmt::Bind { dest, op: BindOp::Force, src })
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
            Some(BehaviorStmt::Event {
                spec: elab_spec,
                guard: None,
                body: lowered,
            })
        }
        IrStmt::EventControl { spec, body } => {
            let elab_spec = match spec {
                piperine_codegen::ir::IrEventSpec::Posedge(e) => EventSpec::Named {
                    name: "posedge".into(),
                    arg: ir_expr_to_phdl(e),
                },
                piperine_codegen::ir::IrEventSpec::Negedge(e) => EventSpec::Named {
                    name: "negedge".into(),
                    arg: ir_expr_to_phdl(e),
                },
                piperine_codegen::ir::IrEventSpec::Change(e) => EventSpec::Named {
                    name: "change".into(),
                    arg: ir_expr_to_phdl(e),
                },
                _ => return None,
            };
            if let Some(es) = lower_stmt(&body) {
                Some(BehaviorStmt::Event {
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

/// Convert an IR expression back to a PHDL AST expression for the
/// digital interpreter to evaluate.
fn ir_expr_to_phdl(ir: &piperine_codegen::ir::IrExpr) -> crate::parse::ast::Expr {
    use crate::parse::ast::{BinaryOp, Expr as PExpr, Literal, UnaryOp};

    match ir {
        piperine_codegen::ir::IrExpr::Real(v)         => PExpr::Literal(Literal::Real(*v)),
        piperine_codegen::ir::IrExpr::Int(v)          => PExpr::Literal(Literal::Int(*v as u64)),
        piperine_codegen::ir::IrExpr::Bool(b)         => PExpr::Literal(Literal::Bool(*b)),
        piperine_codegen::ir::IrExpr::Param(name) | piperine_codegen::ir::IrExpr::Var(name) => PExpr::Ident(name.clone()),
        piperine_codegen::ir::IrExpr::BranchAccess { access, plus, minus } => PExpr::Call(
            Box::new(PExpr::Ident(access.clone())),
            vec![PExpr::Ident(plus.clone()), PExpr::Ident(minus.clone())],
        ),
        piperine_codegen::ir::IrExpr::Call(name, args) => PExpr::Call(
            Box::new(PExpr::Ident(name.clone())),
            args.iter().map(ir_expr_to_phdl).collect(),
        ),
        piperine_codegen::ir::IrExpr::Binary(op, a, b) => {
            use piperine_codegen::ir::IrBinOp;
            let phdl_op = match op {
                IrBinOp::Add => BinaryOp::Add,
                IrBinOp::Sub => BinaryOp::Sub,
                IrBinOp::Mul => BinaryOp::Mul,
                IrBinOp::Div => BinaryOp::Div,
                IrBinOp::Rem => BinaryOp::Rem,
                IrBinOp::Eq => BinaryOp::Eq,
                IrBinOp::Ne => BinaryOp::Neq,
                IrBinOp::Lt => BinaryOp::Lt,
                IrBinOp::Le => BinaryOp::Le,
                IrBinOp::Gt => BinaryOp::Gt,
                IrBinOp::Ge => BinaryOp::Ge,
                IrBinOp::BitAnd | IrBinOp::And => BinaryOp::BitAnd,
                IrBinOp::BitOr | IrBinOp::Or => BinaryOp::BitOr,
                IrBinOp::BitXor => BinaryOp::BitXor,
                // Pow/Shl/Shr/AShl/AShr are validated-out in
                // `validate_ir_digital_expr` (GAPS §A.4) — they cannot reach
                // this match. Keeping a defensive fall-through to `Add`
                // would silently re-introduce the wrong-code bug.
                IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr
                | IrBinOp::AShl | IrBinOp::AShr => {
                    unreachable!("validated out by validate_ir_digital_expr (GAPS §A.4)")
                }
            };
            PExpr::Binary(
                Box::new(ir_expr_to_phdl(a)),
                phdl_op,
                Box::new(ir_expr_to_phdl(b)),
            )
        }
        piperine_codegen::ir::IrExpr::Unary(op, e) => {
            let phdl_op = match op {
                piperine_codegen::ir::IrUnOp::Neg => UnaryOp::Neg,
                _ => UnaryOp::Not,
            };
            PExpr::Unary(phdl_op, Box::new(ir_expr_to_phdl(e)))
        }
        _ => PExpr::Literal(Literal::Bool(false)),
    }
}
