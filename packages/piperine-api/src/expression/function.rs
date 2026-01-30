use crate::expression::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum Function {
    Voltage(Box<Expr>),
    VoltageDiff(Box<Expr>, Box<Expr>),
    Current(Box<Expr>),

    Sqrt(Box<Expr>),
    Abs(Box<Expr>),
    Exp(Box<Expr>),
    Ln(Box<Expr>),
    Log10(Box<Expr>),

    Sin(Box<Expr>),
    Cos(Box<Expr>),
    Tan(Box<Expr>),
    Asin(Box<Expr>),
    Acos(Box<Expr>),
    Atan(Box<Expr>),

    Sinh(Box<Expr>),
    Cosh(Box<Expr>),
    Tanh(Box<Expr>),
    Asinh(Box<Expr>),
    Acosh(Box<Expr>),
    Atanh(Box<Expr>),

    Int(Box<Expr>),
    Nint(Box<Expr>),
    Floor(Box<Expr>),
    Ceil(Box<Expr>),

    Sgn(Box<Expr>),
    Min(Box<Expr>, Box<Expr>),
    Max(Box<Expr>, Box<Expr>),

    Pow(Box<Expr>, Box<Expr>),
    Pwr(Box<Expr>, Box<Expr>),

    Gauss(Box<Expr>, Box<Expr>, Box<Expr>),
    Agauss(Box<Expr>, Box<Expr>, Box<Expr>),
    Unif(Box<Expr>, Box<Expr>),
    Aunif(Box<Expr>, Box<Expr>),
    Limit(Box<Expr>, Box<Expr>),

    Var(Box<Expr>),
    Vec(Box<Expr>),
}

#[allow(non_snake_case)]
#[macro_export]
macro_rules! V {
    ($node:expr) => {
        Function::Voltage(Box::new($node.into()))
    };
    ($pos:expr, $neg:expr) => {
        Function::VoltageDiff(Box::new($pos.into()), Box::new($neg.into()))
    };
}

#[allow(non_snake_case)]
#[macro_export]
macro_rules! I {
    ($source:expr) => {
        Function::Current(Box::new($source.into()))
    };
}

#[macro_export]
macro_rules! sqrt {
    ($x:expr) => {
        Function::Sqrt(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! abs {
    ($x:expr) => {
        Function::Abs(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! exp {
    ($x:expr) => {
        Function::Exp(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! ln {
    ($x:expr) => {
        Function::Ln(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! log10 {
    ($x:expr) => {
        Function::Log10(Box::new($x.into()))
    };
}

// Trigonometry
#[macro_export]
macro_rules! sin {
    ($x:expr) => {
        Function::Sin(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! cos {
    ($x:expr) => {
        Function::Cos(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! tan {
    ($x:expr) => {
        Function::Tan(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! asin {
    ($x:expr) => {
        Function::Asin(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! acos {
    ($x:expr) => {
        Function::Acos(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! atan {
    ($x:expr) => {
        Function::Atan(Box::new($x.into()))
    };
}

// Hyperbolic
#[macro_export]
macro_rules! sinh {
    ($x:expr) => {
        Function::Sinh(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! cosh {
    ($x:expr) => {
        Function::Cosh(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! tanh {
    ($x:expr) => {
        Function::Tanh(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! asinh {
    ($x:expr) => {
        Function::Asinh(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! acosh {
    ($x:expr) => {
        Function::Acosh(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! atanh {
    ($x:expr) => {
        Function::Atanh(Box::new($x.into()))
    };
}

// Rounding
#[macro_export]
macro_rules! int {
    ($x:expr) => {
        Function::Int(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! nint {
    ($x:expr) => {
        Function::Nint(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! floor {
    ($x:expr) => {
        Function::Floor(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! ceil {
    ($x:expr) => {
        Function::Ceil(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! sgn {
    ($x:expr) => {
        Function::Sgn(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! var {
    ($x:expr) => {
        Function::Var(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! vec {
    ($x:expr) => {
        Function::Vec(Box::new($x.into()))
    };
}

#[macro_export]
macro_rules! min {
    ($x:expr, $y:expr) => {
        Function::Min(Box::new($x.into()), Box::new($y.into()))
    };
}

#[macro_export]
macro_rules! max {
    ($x:expr, $y:expr) => {
        Function::Max(Box::new($x.into()), Box::new($y.into()))
    };
}

#[macro_export]
macro_rules! pow {
    ($base:expr, $exponent:expr) => {
        Function::Pow(Box::new($base.into()), Box::new($exponent.into()))
    };
}

#[macro_export]
macro_rules! pwr {
    ($base:expr, $exponent:expr) => {
        Function::Pwr(Box::new($base.into()), Box::new($exponent.into()))
    };
}

#[macro_export]
macro_rules! unif {
    ($nom:expr, $var:expr) => {
        Function::Unif(Box::new($nom.into()), Box::new($var.into()))
    };
}

#[macro_export]
macro_rules! aunif {
    ($nom:expr, $var:expr) => {
        Function::Aunif(Box::new($nom.into()), Box::new($var.into()))
    };
}

#[macro_export]
macro_rules! limit {
    ($nom:expr, $var:expr) => {
        Function::Limit(Box::new($nom.into()), Box::new($var.into()))
    };
}

#[macro_export]
macro_rules! gauss {
    ($nom:expr, $rvar:expr, $sigma:expr) => {
        Function::Gauss(
            Box::new($nom.into()),
            Box::new($rvar.into()),
            Box::new($sigma.into()),
        )
    };
}

#[macro_export]
macro_rules! agauss {
    ($nom:expr, $avar:expr, $sigma:expr) => {
        Function::Agauss(
            Box::new($nom.into()),
            Box::new($avar.into()),
            Box::new($sigma.into()),
        )
    };
}
