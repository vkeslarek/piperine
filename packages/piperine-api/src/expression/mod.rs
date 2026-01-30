pub mod function;

use crate::expression::function::Function;
use num_complex::Complex;
use std::fmt::Write;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Sub};

#[derive(Debug, Clone, PartialEq)]
pub enum Number {
    Real(f64),
    Complex(Complex<f64>),
}

impl Into<Number> for Complex<f64> {
    fn into(self) -> Number {
        Number::Complex(self)
    }
}

impl Into<Number> for f64 {
    fn into(self) -> Number {
        Number::Real(self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Atom {
    Constant(Number),
    Identifier(String),
}

impl Into<Atom> for Number {
    fn into(self) -> Atom {
        Atom::Constant(self)
    }
}

impl Into<Atom> for Complex<f64> {
    fn into(self) -> Atom {
        Atom::Constant(Number::Complex(self))
    }
}

impl Into<Atom> for f64 {
    fn into(self) -> Atom {
        Atom::Constant(Number::Real(self))
    }
}

impl From<String> for Atom {
    fn from(val: String) -> Self {
        Atom::Identifier(val)
    }
}

impl From<&str> for Atom {
    fn from(val: &str) -> Self {
        Atom::Identifier(val.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperation {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperation {
    Pow,
    Mul,
    Div,
    Mod,
    IntDiv,
    Add,
    Sub,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Atom(Atom),

    Unary {
        op: UnaryOperation,
        expr: Box<Expr>,
    },

    Binary {
        op: BinaryOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    Ternary {
        cond: Box<Expr>,
        true_val: Box<Expr>,
        false_val: Box<Expr>,
    },

    Call(Function),
}

impl Expr {
    pub fn eq(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::Eq,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn ne(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::Ne,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn lt(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::Lt,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn le(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::Le,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn gt(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::Gt,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn ge(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::Ge,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn and(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::And,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn or(self, rhs: impl Into<Expr>) -> Expr {
        Expr::Binary {
            op: BinaryOperation::Or,
            lhs: Box::new(self),
            rhs: Box::new(rhs.into()),
        }
    }

    pub fn ternary(self, true_val: impl Into<Expr>, false_val: impl Into<Expr>) -> Expr {
        Expr::Ternary {
            cond: Box::new(self),
            true_val: Box::new(true_val.into()),
            false_val: Box::new(false_val.into()),
        }
    }
}

#[macro_export]
macro_rules! eq {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).eq($rhs)
    };
}

#[macro_export]
macro_rules! ne {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).ne($rhs)
    };
}

#[macro_export]
macro_rules! lt {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).lt($rhs)
    };
}

#[macro_export]
macro_rules! le {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).le($rhs)
    };
}

#[macro_export]
macro_rules! gt {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).gt($rhs)
    };
}

#[macro_export]
macro_rules! ge {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).ge($rhs)
    };
}

#[macro_export]
macro_rules! and {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).and($rhs)
    };
}

#[macro_export]
macro_rules! or {
    ($lhs:expr, $rhs:expr) => {
        $crate::expression::Expr::from($lhs).or($rhs)
    };
}

impl Into<Expr> for Atom {
    fn into(self) -> Expr {
        Expr::Atom(self)
    }
}

impl Into<Expr> for Complex<f64> {
    fn into(self) -> Expr {
        Expr::Atom(self.into())
    }
}

impl Into<Expr> for f64 {
    fn into(self) -> Expr {
        Expr::Atom(self.into())
    }
}

impl From<String> for Expr {
    fn from(val: String) -> Self {
        Expr::Atom(val.into())
    }
}

impl From<&str> for Expr {
    fn from(val: &str) -> Self {
        Expr::Atom(val.into())
    }
}
impl Neg for Expr {
    type Output = Expr;

    fn neg(self) -> Self::Output {
        Expr::Unary {
            op: UnaryOperation::Neg,
            expr: Box::new(self),
        }
    }
}

impl Not for Expr {
    type Output = Expr;

    fn not(self) -> Self::Output {
        Expr::Unary {
            op: UnaryOperation::Not,
            expr: Box::new(self),
        }
    }
}

macro_rules! impl_binary_op {
    ($trait:ident, $method:ident, $op:expr) => {
        impl $trait<Expr> for Expr {
            type Output = Expr;
            fn $method(self, rhs: Expr) -> Self::Output {
                Expr::Binary {
                    op: $op,
                    lhs: Box::new(self),
                    rhs: Box::new(rhs),
                }
            }
        }

        impl $trait<f64> for Expr {
            type Output = Expr;
            fn $method(self, rhs: f64) -> Self::Output {
                Expr::Binary {
                    op: $op,
                    lhs: Box::new(self),
                    rhs: Box::new(rhs.into()),
                }
            }
        }

        impl $trait<Complex<f64>> for Expr {
            type Output = Expr;
            fn $method(self, rhs: Complex<f64>) -> Self::Output {
                Expr::Binary {
                    op: $op,
                    lhs: Box::new(self),
                    rhs: Box::new(rhs.into()),
                }
            }
        }

        impl $trait<&str> for Expr {
            type Output = Expr;
            fn $method(self, rhs: &str) -> Self::Output {
                Expr::Binary {
                    op: $op,
                    lhs: Box::new(self),
                    rhs: Box::new(rhs.into()),
                }
            }
        }
    };
}

impl_binary_op!(Add, add, BinaryOperation::Add);
impl_binary_op!(Sub, sub, BinaryOperation::Sub);
impl_binary_op!(Mul, mul, BinaryOperation::Mul);
impl_binary_op!(Div, div, BinaryOperation::Div);
impl_binary_op!(Rem, rem, BinaryOperation::Mod);
impl_binary_op!(BitAnd, bitand, BinaryOperation::And);
impl_binary_op!(BitOr, bitor, BinaryOperation::Or);
impl_binary_op!(BitXor, bitxor, BinaryOperation::Pow);
