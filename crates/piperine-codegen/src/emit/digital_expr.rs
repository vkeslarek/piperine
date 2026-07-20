//! The `Codegen` trait: dispatch from POM `Expr` to Cranelift `Value` via
//! the [`Builder`]. Adding a new expression variant = one match arm here.

use cranelift_codegen::ir::InstBuilder;
use piperine_lang::parse::ast::{Expr, Literal, UnaryOp};

use crate::resolve::UnOp;
use crate::error::CodegenError;

use super::builder::Builder;
use super::resolver::Typed;

/// Code generation for POM expressions.
///
/// Implement this trait for a new expression variant to teach the codegen
/// how to emit it — that's the ONLY place you need to touch.
pub trait Codegen {
    fn emit(&self, b: &mut Builder) -> Result<Typed, CodegenError>;
}

impl Codegen for Expr {
    fn emit(&self, b: &mut Builder) -> Result<Typed, CodegenError> {
        match self {
            Expr::Literal(lit) => match lit {
                Literal::Real(v) => Ok(Typed::real(b.builder_f64(*v))),
                Literal::Int(v) => Ok(Typed::int(b.builder_i64(*v as i64))),
                Literal::Bool(v) => Ok(Typed::int(b.builder_i64(i64::from(*v)))),
                Literal::Quad(s) => {
                    // Parse quad literal: "0"=0, "1"=1, "x"/"X"=2, "z"/"Z"=3
                    let q: u8 = match s.as_str() {
                        "0" => 0,
                        "1" => 1,
                        "x" | "X" => 2,
                        "z" | "Z" => 3,
                        _ => 2, // default to X
                    };
                    Ok(Typed::quad(b.builder_i64(i64::from(q))))
                }
                Literal::String(_) => {
                    Err(CodegenError::unsupported("string literal in digital expression"))
                }
                Literal::None => Err(CodegenError::unsupported("none literal in digital expression")),
            },
            Expr::Ident(name) => b.load_ident(name),
            Expr::Binary(lhs, op, rhs) => {
                let l = lhs.emit(b)?;
                let r = rhs.emit(b)?;
                b.emit_binary(crate::resolve::BinOp::from_pom(op.clone()), l, r)
            }
            Expr::Unary(op, x) => {
                let v = x.emit(b)?;
                let ir_op = match op {
                    UnaryOp::Neg => UnOp::Neg,
                    UnaryOp::Not => UnOp::Not,
                };
                b.emit_unary(ir_op, v)
            }
            Expr::Call(func, args) => b.call_expr(func, args),
            Expr::SysCall(name, args) => b.syscall(name, args),
            // Casts are resolved at lowering time; emit the inner expression.
            Expr::Cast(_, inner) => inner.emit(b),
            Expr::If { cond, then_body, else_body } => {
                // Expression-level if: both branches must produce a value.
                // Use Cranelift select for simple cases (no side effects).
                let c = cond.emit(b)?;
                let flag = b.truthy(c)?;
                let then_val = match &then_body.expr {
                    Some(e) => e.emit(b)?,
                    None => Typed::int(b.builder_i64(0)),
                };
                let else_val = match &else_body.expr {
                    Some(e) => e.emit(b)?,
                    None => Typed::int(b.builder_i64(0)),
                };
                let (then_val, else_val) = b.unify(then_val, else_val)?;
                let value = b.builder.ins().select(flag, then_val.value, else_val.value);
                Ok(Typed { value, ty: then_val.ty })
            }
            Expr::Field(base, field) => {
                // Bundle field access — resolved to flattened params at elaboration.
                // Try "base_field" as a combined name.
                if let Expr::Ident(base_name) = base.as_ref() {
                    let combined = format!("{base_name}_{field}");
                    // Try as a var
                    if let Some(&id) = b.resolver.vars.get(&combined) {
                        return Ok(b.load_var(id));
                    }
                    if let Some(&id) = b.resolver.params.get(&combined) {
                        return b.load_param(id);
                    }
                }
                Err(CodegenError::unsupported(format!("unresolved field access: {self:?}")))
            }
            Expr::Index(_, _)
            | Expr::Slice(_, _)
            | Expr::Array(_)
            | Expr::Tuple(_)
            | Expr::BundleLit { .. }
            | Expr::MapLit(_)
            | Expr::SetLit(_)
            | Expr::Lambda { .. }
            | Expr::Path(_)
            | Expr::Block(_) => Err(CodegenError::unsupported(format!(
                "expression form not supported in digital codegen: {self:?}"
            ))),
        }
    }
}
