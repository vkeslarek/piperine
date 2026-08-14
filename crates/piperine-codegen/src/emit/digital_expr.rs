//! The `Codegen` trait: dispatch from POM `Expr` (`piperine_lang::ast::Expr`)
//! to Cranelift `Value` via the [`Builder`]. Adding a new expression variant
//! = one match arm here.

use cranelift_codegen::ir::InstBuilder;
use piperine_lang::ast::{Expr, UnOp as VUnOp};
use piperine_lang::lexer::NumberLit;

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
            Expr::Number { value, .. } => match value {
                NumberLit::Real(v) => Ok(Typed::real(b.builder_f64(*v))),
                NumberLit::Integer(v) => Ok(Typed::int(b.builder_i64(*v))),
                NumberLit::Based(based) => {
                    // A based literal's digits may carry 4-state bits
                    // (x/z) — LRM §2.6.1. `Based` digits are decoded
                    // radix-first; any non-numeric digit (x/z/?) makes
                    // the whole literal a Quad `X`, matching real
                    // Verilog-1364 "any unknown bit makes the whole
                    // value unknown for a same-radix read" behavior for
                    // the common case this digital codegen covers today.
                    let digits: String = based.digits.chars().filter(|c| *c != '_').collect();
                    if digits.chars().any(|c| matches!(c, 'x' | 'X' | 'z' | 'Z' | '?')) {
                        return Ok(Typed::quad(b.builder_i64(2))); // X
                    }
                    let radix = match based.base {
                        piperine_lang::lexer::NumberBase::Decimal => 10,
                        piperine_lang::lexer::NumberBase::Binary => 2,
                        piperine_lang::lexer::NumberBase::Octal => 8,
                        piperine_lang::lexer::NumberBase::Hex => 16,
                    };
                    let v = i64::from_str_radix(&digits, radix).unwrap_or(0);
                    Ok(Typed::int(b.builder_i64(v)))
                }
            },
            Expr::Str { .. } => Err(CodegenError::unsupported("string literal in digital expression")),
            Expr::Ident { name, .. } => b.load_ident(name),
            Expr::Binary { op, lhs, rhs, .. } => {
                let l = lhs.emit(b)?;
                let r = rhs.emit(b)?;
                b.emit_binary(crate::resolve::BinOp::from_pom(*op), l, r)
            }
            Expr::Unary { op, operand, .. } => {
                if *op == VUnOp::Plus {
                    return operand.emit(b);
                }
                let v = operand.emit(b)?;
                b.emit_unary(UnOp::from_pom(*op), v)
            }
            Expr::Call { name, args, .. } => b.call_expr(name, args),
            Expr::AccessFunction { pos, neg, .. } => b.branch_access(pos, neg.as_deref()),
            Expr::SystemFunctionCall { name, args, .. } => b.syscall(name, args),
            Expr::Ternary { cond, then_branch, else_branch, .. } => {
                // Expression-level conditional: both branches must produce
                // a value. Cranelift `select` (no side effects either way).
                let c = cond.emit(b)?;
                let flag = b.truthy(c)?;
                let then_val = then_branch.emit(b)?;
                let else_val = else_branch.emit(b)?;
                let (then_val, else_val) = b.unify(then_val, else_val)?;
                let value = b.builder.ins().select(flag, then_val.value, else_val.value);
                Ok(Typed { value, ty: then_val.ty })
            }
            Expr::Index { .. } | Expr::PartSelect { .. } => Err(CodegenError::unsupported(format!(
                "bit-select/part-select in digital codegen: {self:?}"
            ))),
            Expr::Concat { .. } | Expr::Replication { .. } => Err(CodegenError::unsupported(format!(
                "vector concatenation/replication in digital codegen: {self:?}"
            ))),
            Expr::AssignmentPattern { .. } => Err(CodegenError::unsupported(format!(
                "assignment pattern in digital codegen: {self:?}"
            ))),
            // Never reaches codegen — see emit/analog_expr.rs's own
            // PortAccess arm.
            Expr::PortAccess { .. } => Err(CodegenError::unsupported(format!(
                "unresolved port access in digital codegen: {self:?}"
            ))),
        }
    }
}
