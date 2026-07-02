//! IR → Device adapter for digital modules.
//!
//! Phase 1.5: takes an [`IrProgram`] + module name, converts the
//! [`piperine_codegen::ir::IrDigitalBody`] into an `BehaviorStmt` sequence inside a
//! synthetic [`Design`] and hands it off to `compile_digital_module`,
//! which walks the standard scanner + interpreter.

use crate::runtime::digital::{compile_digital_module, DigitalInterpreter};

use piperine_codegen::ir::{IrModule, IrProgram, IrStmt, SymbolTable};
use piperine_codegen::CodegenError;
use crate::pom::{Behavior, BehaviorStmt, Design, Module};
use crate::parse::ast::{BehaviorKind, EventSpec};

pub fn find_module<'a>(program: &'a IrProgram, name: &str) -> Option<&'a IrModule> {
    program.modules.iter().find(|m| m.name == name)
}

pub fn ir_digital_to_interp(
    program: &IrProgram,
    module_name: &str,
) -> Result<DigitalInterpreter, CodegenError> {
    let module = find_module(program, module_name)
        .ok_or_else(|| CodegenError::ModuleNotFound(module_name.to_string()))?;
    let body = module
        .digital
        .as_ref()
        .ok_or_else(|| CodegenError::Unsupported(module_name.to_string()))?;



    for stmt in &body.stmts {
        validate_ir_digital_stmt(stmt)?;
    }

    let elab_stmts = lower_stmts(&body.stmts, &module.symbols);

    let mut elab = Design::new();
    let behavior = Behavior::new(
        module_name.to_string(),
        BehaviorKind::Digital,
        elab_stmts,
    );
    let synth_module = Module::new(
        module_name.to_string(),
        vec![], vec![], vec![], vec![], vec![],
        vec![behavior],
    );
    elab.insert_module(module_name.to_string(), synth_module);

    compile_digital_module(&elab, module_name, 0)
}



fn validate_ir_digital_stmt(stmt: &piperine_codegen::ir::IrStmt) -> Result<(), CodegenError> {
    use piperine_codegen::ir::IrStmt;
    match stmt {
        IrStmt::Assign { expr, .. } => {
            validate_ir_digital_expr(expr)
        }
        IrStmt::If { cond, then_, else_, .. } => {
            validate_ir_digital_expr(cond)?;
            for s in then_ { validate_ir_digital_stmt(s)?; }
            for s in else_ { validate_ir_digital_stmt(s)?; }
            Ok(())
        }
        IrStmt::AnalogEvent(e) => {
            for s in &e.body { validate_ir_digital_stmt(s)?; }
            Ok(())
        }
        IrStmt::ClockedBlock { body, .. } => {
            for s in body { validate_ir_digital_stmt(s)?; }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_ir_digital_expr(e: &piperine_codegen::ir::IrExpr) -> Result<(), CodegenError> {
    use piperine_codegen::ir::{IrBinOp, IrExpr, IrUnOp};
    match e {
        IrExpr::Real(_) | IrExpr::Int(_) | IrExpr::Bool(_)
        | IrExpr::Quad(_) | IrExpr::Param(_) | IrExpr::Var(_) | IrExpr::State(_)
        | IrExpr::Sim(_) | IrExpr::Net(_) => Ok(()),
        IrExpr::Branch { .. } => Ok(()),
        IrExpr::Unary(op, x) => {
            match op {
                IrUnOp::Neg | IrUnOp::Not => validate_ir_digital_expr(x),
                _ => Err(CodegenError::Unsupported(format!(
                    "unary operator {op:?} in digital block"
                ))),
            }
        }
        IrExpr::Binary(op, a, b) => {
            match op {
                IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr => {
                    Err(CodegenError::Unsupported(format!(
                        "operator {op:?} in digital block"
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
        IrExpr::Call(_, args) | IrExpr::MathCall(_, args) => {
            for a in args { validate_ir_digital_expr(a)?; }
            Ok(())
        }
        _ => Err(CodegenError::Unsupported(format!(
            "{e:?} in digital block"
        ))),
    }
}

fn lower_stmts(stmts: &[piperine_codegen::ir::IrStmt], symbols: &SymbolTable) -> Vec<BehaviorStmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for s in stmts {
        if let Some(es) = lower_stmt(s, symbols) {
            out.push(es);
        }
    }
    out
}

fn lower_stmt(s: &piperine_codegen::ir::IrStmt, symbols: &SymbolTable) -> Option<BehaviorStmt> {
    use piperine_codegen::ir::{IrStmt, Lval};
    use crate::parse::ast::{BindOp, Expr as PExpr};

    match s {
        IrStmt::Assign { lval, expr, .. } => {
            let dest = match lval {
                Lval::Var(id) => {
                    let v = symbols.var(*id);
                    PExpr::Ident(v.name.clone())
                }
                Lval::Net(id) => {
                    let n = symbols.node(*id);
                    PExpr::Ident(n.name.clone())
                }
                _ => return None,
            };
            let src = ir_expr_to_phdl(expr, symbols);
            Some(BehaviorStmt::Bind { dest, op: BindOp::Force, src })
        }
        IrStmt::If { cond, then_, else_, .. } => {
            let c = ir_expr_to_phdl(cond, symbols);
            let tb = lower_stmts(then_, symbols);
            let eb = lower_stmts(else_, symbols);
            Some(BehaviorStmt::If {
                cond: c,
                then_body: tb,
                else_body: if eb.is_empty() { None } else { Some(eb) },
            })
        }
        IrStmt::AnalogEvent(e) => {
            // We cannot map AnalogEvent meaningfully in digital lowering right now,
            // we'll just return None or map it generically.
            // Digital block doesn't usually contain AnalogEvent.
            None
        }
        IrStmt::ClockedBlock { event, body } => {
            use piperine_codegen::ir::DigitalEvent;
            let elab_spec = match event {
                DigitalEvent::Posedge(e) => EventSpec::Named {
                    name: "posedge".into(),
                    arg: ir_expr_to_phdl(e, symbols),
                },
                DigitalEvent::Negedge(e) => EventSpec::Named {
                    name: "negedge".into(),
                    arg: ir_expr_to_phdl(e, symbols),
                },
                DigitalEvent::Change(e) => EventSpec::Named {
                    name: "change".into(),
                    arg: ir_expr_to_phdl(e, symbols),
                },
                _ => return None,
            };
            let lowered = lower_stmts(body, symbols);
            if lowered.is_empty() {
                return None;
            }
            Some(BehaviorStmt::Event {
                spec: elab_spec,
                guard: None,
                body: lowered,
            })
        }
        _ => None,
    }
}

fn ir_expr_to_phdl(ir: &piperine_codegen::ir::IrExpr, symbols: &SymbolTable) -> crate::parse::ast::Expr {
    use crate::parse::ast::{BinaryOp, Expr as PExpr, Literal, UnaryOp};
    use piperine_codegen::ir::IrExpr;

    match ir {
        IrExpr::Real(v)         => PExpr::Literal(Literal::Real(*v)),
        IrExpr::Int(v)          => PExpr::Literal(Literal::Int(*v as u64)),
        IrExpr::Bool(b)         => PExpr::Literal(Literal::Bool(*b)),
        IrExpr::Param(id) => {
            let p = symbols.param(*id);
            PExpr::Ident(p.name.clone())
        }
        IrExpr::Var(id) => {
            let v = symbols.var(*id);
            PExpr::Ident(v.name.clone())
        }
        IrExpr::Net(id) => {
            let n = symbols.node(*id);
            PExpr::Ident(n.name.clone())
        }
        IrExpr::Branch { nature: _, plus, minus } => {
            let pn = symbols.node(*plus).name.clone();
            let mn = symbols.node(*minus).name.clone();
            PExpr::Call(
                Box::new(PExpr::Ident("V".into())),
                vec![PExpr::Ident(pn), PExpr::Ident(mn)],
            )
        }
        IrExpr::MathCall(name, args) => PExpr::Call(
            Box::new(PExpr::Ident(name.clone())),
            args.iter().map(|a| ir_expr_to_phdl(a, symbols)).collect(),
        ),
        IrExpr::Call(id, args) => {
            let name = symbols.function(*id).name.clone();
            PExpr::Call(
                Box::new(PExpr::Ident(name)),
                args.iter().map(|a| ir_expr_to_phdl(a, symbols)).collect(),
            )
        }
        IrExpr::Binary(op, a, b) => {
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
                IrBinOp::Pow | IrBinOp::Shl | IrBinOp::Shr => {
                    unreachable!("validated out by validate_ir_digital_expr")
                }
            };
            PExpr::Binary(
                Box::new(ir_expr_to_phdl(a, symbols)),
                phdl_op,
                Box::new(ir_expr_to_phdl(b, symbols)),
            )
        }
        IrExpr::Unary(op, e) => {
            let phdl_op = match op {
                piperine_codegen::ir::IrUnOp::Neg => UnaryOp::Neg,
                _ => UnaryOp::Not,
            };
            PExpr::Unary(phdl_op, Box::new(ir_expr_to_phdl(e, symbols)))
        }
        _ => PExpr::Literal(Literal::Bool(false)),
    }
}
