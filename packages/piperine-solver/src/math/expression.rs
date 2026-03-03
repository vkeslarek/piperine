use crate::circuit::netlist::{BranchIdentifier, NodeIdentifier};
use crate::devices::ask::Ask;
use crate::math::rand::Distribution;
use num_complex::Complex64;
use std::f64::consts::PI;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Sub};

#[derive(Debug, Clone, PartialEq)]
pub enum Quantity {
    Scalar(f64),
    Complex(Complex64),
    Vector(Vec<f64>),
    Boolean(bool),
    Stochastic(Distribution),
    Undefined,
}

impl Into<Expr> for Quantity {
    fn into(self) -> Expr {
        Expr::Constant(self)
    }
}

impl Quantity {
    pub fn to_complex(&self) -> Option<Complex64> {
        match self {
            Quantity::Scalar(s) => Some(Complex64::new(*s, 0.0)),
            Quantity::Complex(c) => Some(*c),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Function {
    Sin(Box<Expr>),
    Cos(Box<Expr>),
    Tan(Box<Expr>),
    Asin(Box<Expr>),
    Acos(Box<Expr>),
    Atan(Box<Expr>),
    Sinh(Box<Expr>),
    Cosh(Box<Expr>),
    Tanh(Box<Expr>),

    Exp(Box<Expr>),
    Ln(Box<Expr>),
    Log10(Box<Expr>),

    Sqrt(Box<Expr>),
    Abs(Box<Expr>),
    Signum(Box<Expr>),
    Floor(Box<Expr>),
    Ceil(Box<Expr>),

    Pow(Box<Expr>, Box<Expr>),
    Min(Box<Expr>, Box<Expr>),
    Max(Box<Expr>, Box<Expr>),
    Atan2(Box<Expr>, Box<Expr>),

    Ddt(Box<Expr>),
    Idt(Box<Expr>),

    Conj(Box<Expr>),
    Real(Box<Expr>),
    Imag(Box<Expr>),
    Arg(Box<Expr>),

    Prev(Box<Expr>, f64),
}

#[macro_export]
macro_rules! sin {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Sin(Box::new(
            $val.into(),
        )))
    };
}

#[macro_export]
macro_rules! cos {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Cos(Box::new(
            $val.into(),
        )))
    };
}

#[macro_export]
macro_rules! tan {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Tan(Box::new(
            $val.into(),
        )))
    };
}

#[macro_export]
macro_rules! asin {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Asin(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! acos {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Acos(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! atan {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Atan(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! sinh {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Sinh(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! cosh {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Cosh(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! tanh {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Tanh(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! exp {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Exp(Box::new(
            $val.into(),
        )))
    };
}

#[macro_export]
macro_rules! ln {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Ln(Box::new(
            $val.into(),
        )))
    };
}

#[macro_export]
macro_rules! log10 {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Log10(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! sqrt {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Sqrt(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! abs {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Abs(Box::new(
            $val.into(),
        )))
    };
}

#[macro_export]
macro_rules! signum {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Signum(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! floor {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Floor(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! ceil {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Ceil(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! pow {
    ($base:expr, $exp:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Pow(
            Box::new($base.into()),
            Box::new($exp.into()),
        ))
    };
}

#[macro_export]
macro_rules! max {
    ($val:expr) => {
        $val.into()
    };

    ($head:expr, $($tail:expr),+) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Max(
            Box::new($head.into()),
            Box::new($crate::max!($($tail),+))
        ))
    };
}

#[macro_export]
macro_rules! min {
    ($val:expr) => {
        $val.into()
    };

    ($head:expr, $($tail:expr),+) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Min(
            Box::new($head.into()),
            Box::new($crate::min!($($tail),+))
        ))
    };
}

#[macro_export]
macro_rules! atan2 {
    ($y:expr, $x:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Atan2(
            Box::new($y.into()),
            Box::new($x.into()),
        ))
    };
}

#[macro_export]
macro_rules! ddt {
    ($fn:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Ddt(Box::new(
            $fn.into(),
        )))
    };
}

#[macro_export]
macro_rules! idt {
    ($fn:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Idt(Box::new(
            $fn.into(),
        )))
    };
}

#[macro_export]
macro_rules! conj {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Conj(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! imag {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Imag(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! real {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Real(
            Box::new($val.into()),
        ))
    };
}

#[macro_export]
macro_rules! arg {
    ($val:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Arg(Box::new(
            $val.into(),
        )))
    };
}

#[macro_export]
macro_rules! prev {
    ($exp:expr, $time:expr) => {
        $crate::math::expression::Expr::Function($crate::math::expression::Function::Prev(
            Box::new($val.into()),
            $time.into(),
        ))
    };
}

#[derive(Debug, Clone)]
pub enum Parameter {
    Time,
    Frequency,
    Temperature,
    Gmin,
}

#[macro_export]
macro_rules! param {
    (time) => {
        $crate::math::expression::Expr::Parameter($crate::math::expression::Parameter::Time)
    };
    (freq) => {
        $crate::math::expression::Expr::Parameter($crate::math::expression::Parameter::Frequency)
    };
    (frequency) => {
        $crate::math::expression::Expr::Parameter($crate::math::expression::Parameter::Frequency)
    };
    (temp) => {
        $crate::math::expression::Expr::Parameter($crate::math::expression::Parameter::Temperature)
    };
    (temperature) => {
        $crate::math::expression::Expr::Parameter($crate::math::expression::Parameter::Temperature)
    };
    (gmin) => {
        $crate::math::expression::Expr::Parameter($crate::math::expression::Parameter::Gmin)
    };
}

impl Into<Expr> for Parameter {
    fn into(self) -> Expr {
        Expr::Parameter(self)
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Constant(Quantity),
    Voltage(NodeIdentifier, NodeIdentifier),
    Current(BranchIdentifier),
    Parameter(Parameter),
    Ask {
        component: String,
        param: Ask,
    },

    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Rem(Box<Expr>, Box<Expr>),

    Gt(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),

    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    BitAnd(Box<Expr>, Box<Expr>),
    BitOr(Box<Expr>, Box<Expr>),
    BitXor(Box<Expr>, Box<Expr>),

    IfElse {
        cond: Box<Expr>,
        true_branch: Box<Expr>,
        false_branch: Box<Expr>,
    },

    Function(Function),
}

macro_rules! impl_binary_op {
    ($trait:ident, $method:ident, $variant:ident) => {
        impl $trait<Expr> for Expr {
            type Output = Expr;
            fn $method(self, rhs: Expr) -> Self::Output {
                Expr::$variant(Box::new(self), Box::new(rhs))
            }
        }

        impl $trait<Expr> for f64 {
            type Output = Expr;
            fn $method(self, rhs: Expr) -> Self::Output {
                let lhs: Expr = self.into();
                Expr::$variant(Box::new(lhs), Box::new(rhs))
            }
        }

        impl $trait<f64> for Expr {
            type Output = Expr;
            fn $method(self, rhs: f64) -> Self::Output {
                let rhs_expr: Expr = rhs.into();
                Expr::$variant(Box::new(self), Box::new(rhs_expr))
            }
        }

        impl $trait<Expr> for i32 {
            type Output = Expr;
            fn $method(self, rhs: Expr) -> Self::Output {
                let lhs: Expr = self.into();
                Expr::$variant(Box::new(lhs), Box::new(rhs))
            }
        }

        impl $trait<i32> for Expr {
            type Output = Expr;
            fn $method(self, rhs: i32) -> Self::Output {
                let rhs_expr: Expr = rhs.into();
                Expr::$variant(Box::new(self), Box::new(rhs_expr))
            }
        }
    };
}

impl_binary_op!(Add, add, Add);
impl_binary_op!(Sub, sub, Sub);
impl_binary_op!(Mul, mul, Mul);
impl_binary_op!(Div, div, Div);
impl_binary_op!(Rem, rem, Rem);
impl_binary_op!(BitAnd, bitand, BitAnd);
impl_binary_op!(BitOr, bitor, BitOr);
impl_binary_op!(BitXor, bitxor, BitXor);

impl Neg for Expr {
    type Output = Expr;
    fn neg(self) -> Self::Output {
        Expr::Neg(Box::new(self))
    }
}

impl Not for Expr {
    type Output = Expr;
    fn not(self) -> Self::Output {
        Expr::Not(Box::new(self))
    }
}

impl From<f64> for Expr {
    fn from(val: f64) -> Self {
        Expr::Constant(Quantity::Scalar(val))
    }
}

impl From<f64> for Quantity {
    fn from(val: f64) -> Self {
        Quantity::Scalar(val)
    }
}

impl From<i32> for Expr {
    fn from(val: i32) -> Self {
        Expr::Constant(Quantity::Scalar(val as f64))
    }
}

impl From<i32> for Quantity {
    fn from(val: i32) -> Self {
        Quantity::Scalar(val as f64)
    }
}

impl From<bool> for Expr {
    fn from(val: bool) -> Self {
        Expr::Constant(Quantity::Boolean(val))
    }
}

impl From<bool> for Quantity {
    fn from(val: bool) -> Self {
        Quantity::Boolean(val)
    }
}

impl From<Complex64> for Expr {
    fn from(val: Complex64) -> Self {
        Expr::Constant(Quantity::Complex(val))
    }
}

impl From<Complex64> for Quantity {
    fn from(val: Complex64) -> Self {
        Quantity::Complex(val)
    }
}

#[allow(non_snake_case)]
#[macro_export]
macro_rules! V {
    ($node:expr) => {
        $crate::math::expression::Expr::Voltage(
            $node.into(),
            $crate::circuit::netlist::NodeIdentifier::Gnd,
        )
    };

    ($pos:expr, $neg:expr) => {
        $crate::math::expression::Expr::Voltage($pos.into(), $neg.into())
    };
}

#[allow(non_snake_case)]
#[macro_export]
macro_rules! I {
    ($comp:expr) => {
        $crate::math::expression::Expr::Current($crate::circuit::netlist::BranchIdentifier {
            component: $comp.into(),
            name: None,
        })
    };

    ($comp:expr, $term:expr) => {
        $crate::math::expression::Expr::Current($crate::circuit::netlist::BranchIdentifier {
            component: $comp.into(),
            name: Some($term.into()),
        })
    };
}

#[macro_export]
macro_rules! ask {
    ($comp:expr, $param:expr) => {
        $crate::math::expression::Expr::Ask {
            component: $comp.into(),
            param: $param,
        }
    };
}

#[macro_export]
macro_rules! constant {
    ($con:expr) => {
        $crate::math::expression::Expr::Constant($con.into())
    };
}

#[allow(non_snake_case)]
#[macro_export]
macro_rules! If {
    ($cond:expr, $t:expr, $f:expr) => {
        $crate::math::expression::Expr::IfElse {
            cond: Box::new($cond.into()),
            true_branch: Box::new($t.into()),
            false_branch: Box::new($f.into()),
        }
    };

    ($cond:expr, $t:expr) => {
        $crate::math::expression::Expr::IfElse {
            cond: Box::new($cond.into()),
            true_branch: Box::new($t.into()),
            false_branch: Box::new($crate::math::expression::Expr::Constant(
                $crate::math::expression::Quantity::Undefined,
            )),
        }
    };

    ($cond:expr => $t:expr; else $f:expr) => {
        $crate::If!($cond, $t, $f)
    };

    ($cond:expr => $t:expr) => {
        $crate::If!($cond, $t)
    };
}

impl Expr {
    pub fn gt(self, rhs: impl Into<Expr>) -> Self {
        Expr::Gt(Box::new(self), Box::new(rhs.into()))
    }
    pub fn lt(self, rhs: impl Into<Expr>) -> Self {
        Expr::Lt(Box::new(self), Box::new(rhs.into()))
    }
    pub fn eq(self, rhs: impl Into<Expr>) -> Self {
        Expr::Eq(Box::new(self), Box::new(rhs.into()))
    }
    pub fn and(self, rhs: impl Into<Expr>) -> Self {
        Expr::And(Box::new(self), Box::new(rhs.into()))
    }
    pub fn or(self, rhs: impl Into<Expr>) -> Self {
        Expr::Or(Box::new(self), Box::new(rhs.into()))
    }

    pub fn pow(self, exp: impl Into<Expr>) -> Self {
        Expr::Function(Function::Pow(Box::new(self), Box::new(exp.into())))
    }
    pub fn sqrt(self) -> Self {
        Expr::Function(Function::Sqrt(Box::new(self)))
    }
    pub fn abs(self) -> Self {
        Expr::Function(Function::Abs(Box::new(self)))
    }
    pub fn floor(self) -> Self {
        Expr::Function(Function::Floor(Box::new(self)))
    }
    pub fn ceil(self) -> Self {
        Expr::Function(Function::Ceil(Box::new(self)))
    }

    pub fn sin(self) -> Self {
        Expr::Function(Function::Sin(Box::new(self)))
    }
    pub fn cos(self) -> Self {
        Expr::Function(Function::Cos(Box::new(self)))
    }
    pub fn tan(self) -> Self {
        Expr::Function(Function::Tan(Box::new(self)))
    }
    pub fn asin(self) -> Self {
        Expr::Function(Function::Asin(Box::new(self)))
    }
    pub fn acos(self) -> Self {
        Expr::Function(Function::Acos(Box::new(self)))
    }
    pub fn atan(self) -> Self {
        Expr::Function(Function::Atan(Box::new(self)))
    }
    pub fn sinh(self) -> Self {
        Expr::Function(Function::Sinh(Box::new(self)))
    }
    pub fn cosh(self) -> Self {
        Expr::Function(Function::Cosh(Box::new(self)))
    }
    pub fn tanh(self) -> Self {
        Expr::Function(Function::Tanh(Box::new(self)))
    }

    pub fn real(self) -> Self {
        Expr::Function(Function::Real(Box::new(self)))
    }
    pub fn imag(self) -> Self {
        Expr::Function(Function::Imag(Box::new(self)))
    }
    pub fn conj(self) -> Self {
        Expr::Function(Function::Conj(Box::new(self)))
    }

    pub fn ternary(self, t: impl Into<Expr>, f: impl Into<Expr>) -> Self {
        Expr::IfElse {
            cond: Box::new(self),
            true_branch: Box::new(t.into()),
            false_branch: Box::new(f.into()),
        }
    }
}

// NOTE: This example function is commented out because it uses integer node references
// which are no longer supported after removing NodeIdentifier::Indexed.
// Nodes must now be created with Circuit::port() instead.
//
// pub fn example_usage() -> Expr {
//     let base_eq = V!(1) - 12.0 * I!("L1") + ask!("C1", Ask::ModelParam("Capacitance".into()));
//
//     let ac_source = sin!(param!(time) * 2.0 * PI * 60.0);
//
//     If!(V!(1).gt(5.0) => 10.0; else max!(base_eq, ac_source))
// }
