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
    Call(FunctionRef, Vec<CallArg>),
    /// Ternary: `condition ? then_val : else_val`
    Select(Box<Expr>, Box<Expr>, Box<Expr>),
    /// Array index: `base[idx]`
    Index(Box<Expr>, Box<Expr>),
    /// Part-select: `base[msb:lsb]`
    PartSelect(Box<Expr>, Box<Expr>, Box<Expr>),
    Path(Path),
    PortFlow(Path),
    Concat(Vec<Expr>),
    Replicate(Box<Expr>, Vec<Expr>),
    Mintypmax(Box<Expr>, Box<Expr>, Box<Expr>),
    PartSelectUp(Box<Expr>, Box<Expr>, Box<Expr>),
    PartSelectDown(Box<Expr>, Box<Expr>, Box<Expr>),
}

/// ungram: `Literal = 'int_number' | 'str_lit' | 'std_real_number' | 'si_real_number' | 'inf'`
#[derive(Debug, Clone)]
pub enum Literal {
    IntNumber(String),
    StrLit(String),
    StdRealNumber(String),
    SiRealNumber(String),
    Inf,
    SizedLit(String),
}

/// ungram: `PrefixExpr op: ('-' | '!' | '~' | '+')`
#[derive(Debug, Clone)]
pub enum PrefixOp {
    Neg,
    Not,
    BitNot,
    Pos,
    ReduceAnd,
    ReduceNand,
    ReduceOr,
    ReduceNor,
    ReduceXor,
    ReduceXnor1,
    ReduceXnor2,
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
    CaseEq,
    CaseNeq,
    ArithShl,
    ArithShr,
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

/// An argument in a function/task call.
///
/// Positional: `$func(val)`.
/// Named: `$func(name = val)` — optional/override parameters.
#[derive(Debug, Clone)]
pub enum CallArg {
    Positional(Expr),
    Named(String, Expr),
}

impl CallArg {
    /// The expression value of this argument.
    pub fn expr(&self) -> &Expr {
        match self { Self::Positional(e) | Self::Named(_, e) => e }
    }
    /// The parameter name, if this is a named arg.
    pub fn name(&self) -> Option<&str> {
        match self { Self::Named(n, _) => Some(n.as_str()), Self::Positional(_) => None }
    }
    pub fn is_named(&self) -> bool { matches!(self, Self::Named(_, _)) }
    pub fn is_positional(&self) -> bool { matches!(self, Self::Positional(_)) }
}

/// ungram: `FunctionRef = Path | SysFun`
#[derive(Debug, Clone)]
pub enum FunctionRef {
    Path(Path),
    SysFun(String),
}
