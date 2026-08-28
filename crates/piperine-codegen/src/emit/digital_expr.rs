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
            // §4.2.1 bit-select (T9, LRMW3-01/LRMW3-03): scalarized vectors
            // (MD-52) mean `q[2]` is simply the independently-named net
            // `q[2]` — `structure.rs`'s element-node registration loop
            // already put it in the symbol table, so this is one
            // `load_ident` call through the same constant-fold rule every
            // other widened terminal resolver in this crate uses
            // (`resolve/pom/expr.rs::terminal_name_of` and its
            // `flatten`/`emit`/`diff` twins). An undriven `q[2]` yields
            // `X` (Table 7-3) for free: it is its own net, defaulting the
            // same way any other undriven digital net already does — no
            // special case needed.
            Expr::Index { base, index, .. } => {
                let Expr::Ident { name, .. } = base.as_ref() else {
                    return Err(CodegenError::unsupported(format!(
                        "digital bit-select base must be a plain vector identifier: {self:?}"
                    )));
                };
                let i = piperine_lang::elab::const_eval::ConstEnv::new().eval_int(index).map_err(|e| {
                    CodegenError::unsupported(format!(
                        "digital bit-select index `{name}[...]` must be a constant expression \
                         (LRM §5.5.2: the index shall be a constant expression, though it may \
                         include genvar variables): {e}"
                    ))
                })?;
                if i < 0 {
                    return Err(CodegenError::unsupported(format!(
                        "digital bit-select index `{name}[{i}]` must be non-negative"
                    )));
                }
                let elem = crate::resolve::pom::structure::element_node_name(name, i as u64);
                b.load_ident(&elem)
            }
            Expr::PartSelect { .. } => Err(CodegenError::unsupported(format!(
                "part-select in digital codegen: {self:?}"
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
            // §6.7/§6.2.1 hierarchical references (T50, LRMW2-42) are
            // resolved for the analog path only (`resolve/pom/expr.rs`);
            // the digital path keeps the POM `Stmt` tree unresolved (this
            // file's own doc comment), so a `Hier` here is a genuine,
            // disclosed scope gap rather than a broken invariant — fails
            // loud rather than silently misreading the wrong net.
            Expr::Hier { .. } => Err(CodegenError::unsupported(format!(
                "hierarchical reference (LRM §6.7) in a digital expression is not yet supported: {self:?}"
            ))),
        }
    }
}
