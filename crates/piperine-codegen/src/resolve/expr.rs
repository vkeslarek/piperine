//! Operator types shared by digital and analog codegen. Codegen dispatches on
//! POM `Expr` (`piperine_vams::ast::Expr`) directly; these are the operator
//! enums that form carries.

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
    BitXnor,
    Shl,
    Shr,
}

impl BinOp {
    /// Map a VAMS AST `BinOp` (LRM §4.1.3/Table 4-1) to this enum.
    ///
    /// `CaseEq`/`CaseNe` (`===`/`!==`, 4-state case (in)equality) have no
    /// distinct slot here — this resolved form carries `Real`/`Int`/`Bool`
    /// values, not 4-state (0/1/X/Z) ones, so a genuine X/Z-aware case
    /// comparison cannot be expressed; they fall back to ordinary
    /// (in)equality, a documented, non-silent approximation (not a `0.0`
    /// no-op) until the resolved form gains a 4-state value representation.
    pub fn from_pom(op: piperine_vams::ast::BinOp) -> Self {
        use piperine_vams::ast::BinOp as P;
        match op {
            P::Add => BinOp::Add,
            P::Sub => BinOp::Sub,
            P::Mul => BinOp::Mul,
            P::Div => BinOp::Div,
            P::Mod => BinOp::Rem,
            P::Pow => BinOp::Pow,
            P::Eq | P::CaseEq => BinOp::Eq,
            P::Ne | P::CaseNe => BinOp::Ne,
            P::Lt => BinOp::Lt,
            P::Le => BinOp::Le,
            P::Gt => BinOp::Gt,
            P::Ge => BinOp::Ge,
            P::BitAnd => BinOp::BitAnd,
            P::BitOr => BinOp::BitOr,
            P::BitXor => BinOp::BitXor,
            P::BitXnor => BinOp::BitXnor,
            P::LogAnd => BinOp::And,
            P::LogOr => BinOp::Or,
            P::Shl => BinOp::Shl,
            P::Shr => BinOp::Shr,
        }
    }

    /// Map this enum back to a VAMS AST `BinOp` (used by `diff.rs` to
    /// rebuild an `Expr` node after differentiating).
    pub fn to_pom(self) -> piperine_vams::ast::BinOp {
        use piperine_vams::ast::BinOp as P;
        match self {
            BinOp::Add => P::Add,
            BinOp::Sub => P::Sub,
            BinOp::Mul => P::Mul,
            BinOp::Div => P::Div,
            BinOp::Rem => P::Mod,
            BinOp::Pow => P::Pow,
            BinOp::Eq => P::Eq,
            BinOp::Ne => P::Ne,
            BinOp::Lt => P::Lt,
            BinOp::Le => P::Le,
            BinOp::Gt => P::Gt,
            BinOp::Ge => P::Ge,
            BinOp::BitAnd => P::BitAnd,
            BinOp::BitOr => P::BitOr,
            BinOp::BitXor => P::BitXor,
            BinOp::BitXnor => P::BitXnor,
            BinOp::And => P::LogAnd,
            BinOp::Or => P::LogOr,
            BinOp::Shl => P::Shl,
            BinOp::Shr => P::Shr,
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
    RedNand,
    RedNor,
    RedXnor,
}

impl UnOp {
    /// Map a VAMS AST `UnOp` (LRM §4.1.3) to this enum. `Plus` (`+x`) is a
    /// no-op at the value level — callers should drop the wrapping node
    /// entirely rather than call this for it (see `pom_eval_const`/`diff.rs`).
    pub fn from_pom(op: piperine_vams::ast::UnOp) -> Self {
        use piperine_vams::ast::UnOp as P;
        match op {
            P::Neg => UnOp::Neg,
            P::LogNot => UnOp::Not,
            P::BitNot => UnOp::BitNot,
            P::AndReduce => UnOp::RedAnd,
            P::OrReduce => UnOp::RedOr,
            P::XorReduce => UnOp::RedXor,
            P::NandReduce => UnOp::RedNand,
            P::NorReduce => UnOp::RedNor,
            P::XnorReduce => UnOp::RedXnor,
            // Handled by the caller before reaching here (identity).
            P::Plus => UnOp::Neg,
        }
    }
}

/// Evaluate a compile-time-constant VAMS `Expr` (LRM ch. 4). `param`
/// resolves parameter references by name. Anything runtime-dependent
/// (branch access functions, system functions) is an error.
pub fn pom_eval_const(
    expr: &piperine_vams::ast::Expr,
    resolve_param: &impl Fn(&str) -> Option<f64>,
) -> Result<f64, String> {
    use piperine_vams::ast::{BinOp as P, Expr, UnOp as U};
    let eval = |e: &Expr| pom_eval_const(e, resolve_param);
    match expr {
        Expr::Number { value, .. } => Ok(number_lit_to_f64(value)),
        Expr::Ident { name, .. } => {
            resolve_param(name).ok_or_else(|| format!("parameter `{name}` has no value"))
        }
        Expr::Unary { op: U::Plus, operand, .. } => eval(operand),
        Expr::Unary { op: U::Neg, operand, .. } => Ok(-eval(operand)?),
        Expr::Unary { op: U::LogNot, operand, .. } => Ok(f64::from(eval(operand)? == 0.0)),
        Expr::Unary { op, .. } => {
            Err(format!("unary operator {op:?} is not const-evaluable"))
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let (a, b) = (eval(lhs)?, eval(rhs)?);
            Ok(match op {
                P::Add => a + b,
                P::Sub => a - b,
                P::Mul => a * b,
                P::Div => a / b,
                P::Mod => a % b,
                P::Pow => a.powf(b),
                P::Eq | P::CaseEq => f64::from(a == b),
                P::Ne | P::CaseNe => f64::from(a != b),
                P::Lt => f64::from(a < b),
                P::Le => f64::from(a <= b),
                P::Gt => f64::from(a > b),
                P::Ge => f64::from(a >= b),
                P::LogAnd => f64::from(a != 0.0 && b != 0.0),
                P::LogOr => f64::from(a != 0.0 || b != 0.0),
                _ => return Err(format!("operator {op:?} is not const-evaluable")),
            })
        }
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            if eval(cond)? != 0.0 { eval(then_branch) } else { eval(else_branch) }
        }
        Expr::Call { name, args, .. } => {
            let vals: Vec<f64> = args.iter().map(eval).collect::<Result<_, _>>()?;
            piperine_lang::math::eval_const_math(name, &vals)
                .ok_or_else(|| format!("`{name}` is not a const-evaluable math builtin"))
        }
        other => Err(format!("expression is not compile-time constant: {other:?}")),
    }
}

fn number_lit_to_f64(lit: &piperine_vams::lexer::NumberLit) -> f64 {
    use piperine_vams::lexer::NumberLit;
    match lit {
        NumberLit::Integer(i) => *i as f64,
        NumberLit::Real(r) => *r,
        NumberLit::Based(b) => {
            let digits: String = b.digits.chars().filter(|c| *c != '_').collect();
            let radix = match b.base {
                piperine_vams::lexer::NumberBase::Decimal => 10,
                piperine_vams::lexer::NumberBase::Binary => 2,
                piperine_vams::lexer::NumberBase::Octal => 8,
                piperine_vams::lexer::NumberBase::Hex => 16,
            };
            i64::from_str_radix(&digits, radix).map(|v| v as f64).unwrap_or(0.0)
        }
    }
}
