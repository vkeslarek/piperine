//! Operator types shared by digital and analog codegen. The old `IrExpr`
//! type is gone — the codegen now dispatches on POM `Expr` directly.

/// A resolved analysis kind returned by `$analysis`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Analysis {
    Dc,
    Ac,
    Tran,
    Noise,
}

/// A position axis for `$xposition` / `$yposition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinOp {
    /// Map a POM `BinaryOp` to this enum.
    pub fn from_pom(op: piperine_lang::parse::ast::BinaryOp) -> Self {
        use piperine_lang::parse::ast::BinaryOp as P;
        match op {
            P::Add => BinOp::Add,
            P::Sub => BinOp::Sub,
            P::Mul => BinOp::Mul,
            P::Div => BinOp::Div,
            P::Rem => BinOp::Rem,
            P::Eq => BinOp::Eq,
            P::Neq => BinOp::Ne,
            P::Lt => BinOp::Lt,
            P::Le => BinOp::Le,
            P::Gt => BinOp::Gt,
            P::Ge => BinOp::Ge,
            P::BitAnd => BinOp::BitAnd,
            P::BitOr => BinOp::BitOr,
            P::BitXor => BinOp::BitXor,
            P::And => BinOp::And,
            P::Or => BinOp::Or,
        }
    }

    /// Map this enum to a POM `BinaryOp`.
    pub fn to_pom(self) -> piperine_lang::parse::ast::BinaryOp {
        use piperine_lang::parse::ast::BinaryOp as P;
        match self {
            BinOp::Add => P::Add,
            BinOp::Sub => P::Sub,
            BinOp::Mul => P::Mul,
            BinOp::Div => P::Div,
            BinOp::Rem => P::Rem,
            BinOp::Eq => P::Eq,
            BinOp::Ne => P::Neq,
            BinOp::Lt => P::Lt,
            BinOp::Le => P::Le,
            BinOp::Gt => P::Gt,
            BinOp::Ge => P::Ge,
            BinOp::BitAnd => P::BitAnd,
            BinOp::BitOr => P::BitOr,
            BinOp::BitXor => P::BitXor,
            BinOp::And => P::And,
            BinOp::Or => P::Or,
            BinOp::Pow | BinOp::Shl | BinOp::Shr => P::BitXor, // fallback (unused in analog)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
    RedAnd,
    RedOr,
    RedXor,
}

/// Evaluate a compile-time-constant POM `Expr`. `param` resolves parameter
/// references by name. Anything runtime-dependent is an error.
pub fn pom_eval_const(
    expr: &piperine_lang::parse::ast::Expr,
    resolve_param: &impl Fn(&str) -> Option<f64>,
) -> Result<f64, String> {
    use piperine_lang::parse::ast::{Expr, Literal, UnaryOp};
    let eval = |e: &Expr| pom_eval_const(e, resolve_param);
    match expr {
        Expr::Literal(Literal::Real(v)) => Ok(*v),
        Expr::Literal(Literal::Int(v)) => Ok(*v as f64),
        Expr::Literal(Literal::Bool(b)) => Ok(f64::from(*b)),
        Expr::Ident(name) => {
            resolve_param(name).ok_or_else(|| format!("parameter `{name}` has no value"))
        }
        Expr::Unary(UnaryOp::Neg, x) => Ok(-eval(x)?),
        Expr::Unary(UnaryOp::Not, x) => Ok(f64::from(eval(x)? == 0.0)),
        Expr::Binary(lhs, op, rhs) => {
            let (a, b) = (eval(lhs)?, eval(rhs)?);
            use piperine_lang::parse::ast::BinaryOp as P;
            Ok(match op {
                P::Add => a + b,
                P::Sub => a - b,
                P::Mul => a * b,
                P::Div => a / b,
                P::Rem => a % b,
                P::Eq => f64::from(a == b),
                P::Neq => f64::from(a != b),
                P::Lt => f64::from(a < b),
                P::Le => f64::from(a <= b),
                P::Gt => f64::from(a > b),
                P::Ge => f64::from(a >= b),
                P::And => f64::from(a != 0.0 && b != 0.0),
                P::Or => f64::from(a != 0.0 || b != 0.0),
                _ => return Err(format!("operator {op:?} is not const-evaluable")),
            })
        }
        Expr::Call(func, args) => {
            if let Expr::Ident(name) = func.as_ref() {
                let vals: Vec<f64> = args.iter().map(eval).collect::<Result<_, _>>()?;
                return piperine_lang::math::eval_const_math(name, &vals)
                    .ok_or_else(|| format!("`{name}` is not a const-evaluable math builtin"));
            }
            Err("non-identifier call in const context".into())
        }
        Expr::Cast(_, inner) => eval(inner),
        Expr::If { cond, then_body, else_body } => {
            if eval(cond)? != 0.0 {
                if let Some(e) = &then_body.expr { eval(e) } else { Ok(0.0) }
            } else {
                if let Some(e) = &else_body.expr { eval(e) } else { Ok(0.0) }
            }
        }
        Expr::Block(b) => {
            if let Some(e) = &b.expr { return eval(e); }
            for s in b.stmts.iter().rev() {
                if let piperine_lang::parse::ast::Stmt::Expr(e) = s {
                    return eval(e);
                }
            }
            Ok(0.0)
        }
        other => Err(format!("expression is not compile-time constant: {other:?}")),
    }
}
