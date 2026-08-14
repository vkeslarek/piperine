//! Expression AST — LRM chapter 4, Annex A.8.3 `expression`.

use crate::error::Span;
use crate::lexer::NumberLit;

/// A binary operator, LRM §4.1.3/Table 4-1. Grouped by the precedence tier
/// `grammar::expr`'s Pratt table assigns them (highest binds tightest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Pow,
    Mul, Div, Mod,
    Add, Sub,
    Shl, Shr,
    Lt, Le, Gt, Ge,
    Eq, Ne, CaseEq, CaseNe,
    BitAnd,
    BitXor, BitXnor,
    BitOr,
    LogAnd,
    LogOr,
}

/// A unary (prefix) operator, LRM §4.1.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Plus, Neg, LogNot, BitNot,
    AndReduce, NandReduce, OrReduce, NorReduce, XorReduce, XnorReduce,
}

/// An expression, LRM Annex A.8.3.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A number literal — LRM §2.6.
    Number { span: Span, value: NumberLit },
    /// A string literal — LRM §2.7.
    Str { span: Span, value: String },
    /// A plain or escaped identifier used as a value (net/var/param
    /// reference, or a hierarchical/scoped name segment already joined by
    /// the grammar layer into one dotted string).
    Ident { span: Span, name: String },
    Unary { span: Span, op: UnOp, operand: Box<Expr> },
    Binary { span: Span, op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    /// `cond ? then_expr : else_expr` — §4.1.7.
    Ternary { span: Span, cond: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr> },
    /// `{expr, expr, ...}` concatenation — §4.6.1 (via Annex A borrows the
    /// 1364 `concatenation` production).
    Concat { span: Span, parts: Vec<Expr> },
    /// `{count{expr, ...}}` replication — §4.6.1.
    Replication { span: Span, count: Box<Expr>, parts: Vec<Expr> },
    /// `base[index]` — bit-select / array index.
    Index { span: Span, base: Box<Expr>, index: Box<Expr> },
    /// `base[msb:lsb]` — part-select.
    PartSelect { span: Span, base: Box<Expr>, msb: Box<Expr>, lsb: Box<Expr> },
    /// A plain function/task call `name(args)` — user function (§5.9), math
    /// function (§4.3), or time-domain function (`ddt`/`idt`/`transition`/
    /// `absdelay`/`slew`/`last_crossing`/`limexp`/`laplace_*`/`zi_*`, §4.5).
    /// Which of these `name` is gets resolved at elaboration, not here.
    Call { span: Span, name: String, args: Vec<Expr> },
    /// A branch access function — `V(p)`, `V(p,n)`, `I(br)`, `I(p,n)` —
    /// LRM §5.3/§5.4/Annex A.8.3 `branch_probe_function_call`.
    AccessFunction { span: Span, function: String, pos: Box<Expr>, neg: Option<Box<Expr>> },
    /// A `$`-prefixed system function call used as a value, e.g.
    /// `$abstime`, `$temperature` — LRM chapter 9.
    SystemFunctionCall { span: Span, name: String, args: Vec<Expr> },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number { span, .. }
            | Expr::Str { span, .. }
            | Expr::Ident { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Ternary { span, .. }
            | Expr::Concat { span, .. }
            | Expr::Replication { span, .. }
            | Expr::Index { span, .. }
            | Expr::PartSelect { span, .. }
            | Expr::Call { span, .. }
            | Expr::AccessFunction { span, .. }
            | Expr::SystemFunctionCall { span, .. } => *span,
        }
    }
}
