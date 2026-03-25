use num_complex::Complex;
use num_traits::{One, Zero};
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};
use crate::node::Node;
use crate::spice::ElementRef;

pub trait Scalar:
    Copy
    + Clone
    + PartialEq
    + Zero
    + One
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + DivAssign
{
}

impl Scalar for f64 {}

impl Scalar for Complex<f64> {}

// ===== Expr =====

/// An ngspice expression tree.
///
/// Serialises to the ngspice expression syntax used in:
/// - device parameters: `{expr}` (curly-brace notation)
/// - meas PARAM:        `'expr'` (single-quote notation)
///
/// Call `to_ngspice()` to obtain the inner expression string (without delimiters).
#[derive(Debug, Clone)]
pub enum Expr {
    /// Literal numeric constant.
    Constant(f64),
    /// Reference to a `.param` variable (e.g. `rval`).
    Param(String),
    /// Reference to a completed `meas` result (e.g. `ppm_0`).
    MeasResult(String),
    /// Node voltage: `V(node)`.
    Voltage(Node),
    /// Branch current: `I(element)`.
    Current(ElementRef),
    /// Binary operation.
    BinOp(Box<Expr>, ExprBinOp, Box<Expr>),
    /// Unary negation.
    Neg(Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum ExprBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

impl Expr {
    pub fn param(name: impl Into<String>) -> Self {
        Expr::Param(name.into())
    }

    pub fn voltage(node: Node) -> Self {
        Expr::Voltage(node)
    }

    pub fn current(elem: ElementRef) -> Self {
        Expr::Current(elem)
    }

    /// Renders the inner expression string (no wrapping delimiters).
    pub fn to_ngspice(&self) -> String {
        match self {
            Expr::Constant(v) => format!("{v}"),
            Expr::Param(s) => s.clone(),
            Expr::MeasResult(s) => s.clone(),
            Expr::Voltage(n) => format!("V({})", n.spice_name()),
            Expr::Current(e) => format!("I({})", e.spice_name()),
            Expr::BinOp(l, op, r) => {
                let op_str = match op {
                    ExprBinOp::Add => "+",
                    ExprBinOp::Sub => "-",
                    ExprBinOp::Mul => "*",
                    ExprBinOp::Div => "/",
                    ExprBinOp::Pow => "^",
                };
                format!("({}{}{})", l.to_ngspice(), op_str, r.to_ngspice())
            }
            Expr::Neg(e) => format!("(-{})", e.to_ngspice()),
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{}}}", self.to_ngspice()) // device param: {expr}
    }
}

// Arithmetic operator overloads (Expr op Expr)
impl Add for Expr {
    type Output = Expr;
    fn add(self, rhs: Expr) -> Expr {
        Expr::BinOp(Box::new(self), ExprBinOp::Add, Box::new(rhs))
    }
}

impl Sub for Expr {
    type Output = Expr;
    fn sub(self, rhs: Expr) -> Expr {
        Expr::BinOp(Box::new(self), ExprBinOp::Sub, Box::new(rhs))
    }
}

impl Mul for Expr {
    type Output = Expr;
    fn mul(self, rhs: Expr) -> Expr {
        Expr::BinOp(Box::new(self), ExprBinOp::Mul, Box::new(rhs))
    }
}

impl Div for Expr {
    type Output = Expr;
    fn div(self, rhs: Expr) -> Expr {
        Expr::BinOp(Box::new(self), ExprBinOp::Div, Box::new(rhs))
    }
}

impl Neg for Expr {
    type Output = Expr;
    fn neg(self) -> Expr {
        Expr::Neg(Box::new(self))
    }
}

// f64 convenience conversions
impl From<f64> for Expr {
    fn from(v: f64) -> Self {
        Expr::Constant(v)
    }
}

impl Mul<Expr> for f64 {
    type Output = Expr;
    fn mul(self, rhs: Expr) -> Expr {
        Expr::Constant(self) * rhs
    }
}

impl Mul<f64> for Expr {
    type Output = Expr;
    fn mul(self, rhs: f64) -> Expr {
        self * Expr::Constant(rhs)
    }
}

impl Div<f64> for Expr {
    type Output = Expr;
    fn div(self, rhs: f64) -> Expr {
        self / Expr::Constant(rhs)
    }
}

impl Add<f64> for Expr {
    type Output = Expr;
    fn add(self, rhs: f64) -> Expr {
        self + Expr::Constant(rhs)
    }
}

impl Sub<f64> for Expr {
    type Output = Expr;
    fn sub(self, rhs: f64) -> Expr {
        self - Expr::Constant(rhs)
    }
}

#[derive(Debug, Clone)]
pub enum Dynamic<T: Scalar> {
    Literal(T),
    Expression(Expr),
}

impl<T: Scalar + Display> Display for Dynamic<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Dynamic::Literal(scalar) => Display::fmt(&scalar, f),
            Dynamic::Expression(expr) => Display::fmt(&expr, f),
        }
    }
}

impl<T: Scalar> From<T> for Dynamic<T> {
    fn from(val: T) -> Self {
        Dynamic::Literal(val)
    }
}

impl<T: Scalar> From<Expr> for Dynamic<T> {
    fn from(expr: Expr) -> Self {
        Dynamic::Expression(expr)
    }
}
