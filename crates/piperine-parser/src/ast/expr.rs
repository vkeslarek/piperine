//! Expressions and operators.
//!
//! Mirrors `veriloga.ungram` from OpenVAF-Reloaded. Extensions beyond the ungram:
//! - `Index` / `PartSelect`: array indexing and bit-selects used in real models.
//! - `PortFlow`: `<port>` syntax (present in ungram as `PortFlow = '<' port:Path '>'`).

use super::*;

/// ungram: `Expr = Literal | PrefixExpr | BinExpr | ParenExpr | ArrayExpr
///                | Call | SelectExpr | PathExpr | PortFlow`
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Prefix(PrefixOp, Box<Expr>),
    Binary(Box<Expr>, BinOp, Box<Expr>),
    Paren(Box<Expr>),
    Array(Vec<Expr>),
    Call(FunctionRef, Vec<Expr>),
    /// Ternary: `condition ? then_val : else_val`
    Select(Box<Expr>, Box<Expr>, Box<Expr>),
    /// Array index: `base[idx]`
    Index(Box<Expr>, Box<Expr>),
    /// Part-select: `base[msb:lsb]`
    PartSelect(Box<Expr>, Box<Expr>, Box<Expr>),
    Path(Path),
    PortFlow(Path),
}

/// ungram: `Literal = 'int_number' | 'str_lit' | 'std_real_number' | 'si_real_number' | 'inf'`
#[derive(Debug, Clone)]
pub enum Literal {
    IntNumber(String),
    StrLit(String),
    StdRealNumber(String),
    SiRealNumber(String),
    Inf,
}

/// ungram: `PrefixExpr op: ('-' | '!' | '~' | '+')`
#[derive(Debug, Clone)]
pub enum PrefixOp {
    Neg,
    Not,
    BitNot,
    Pos,
}

/// ungram: `BinExpr op: ('||' | '&&' | '==' | '!=' | '<=' | '>=' | '<' | '>'
///                      | '+' | '*' | '-' | '/' | '**' | '%' | '<<' | '>>'
///                      | '^' | '^~' | '~^' | '|' | '&')`
#[derive(Debug, Clone)]
pub enum BinOp {
    OrOr,
    AndAnd,
    Eq,
    Neq,
    Le,
    Ge,
    Lt,
    Gt,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    Shl,
    Shr,
    Xor,
    XNor1,
    XNor2,
    BitOr,
    BitAnd,
}

/// ungram: `Assign = lval:Expr op:('<+' | '=') rval:Expr`
#[derive(Debug, Clone)]
pub enum AssignOp {
    Contrib,
    Eq,
}

#[derive(Debug, Clone)]
pub struct Assign {
    pub lval: Expr,
    pub op: AssignOp,
    pub rval: Expr,
}

/// ungram: `FunctionRef = Path | SysFun`
#[derive(Debug, Clone)]
pub enum FunctionRef {
    Path(Path),
    SysFun(String),
}
