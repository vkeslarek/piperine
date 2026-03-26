use num_complex::Complex;
use num_traits::{One, Zero};
use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, Sub, SubAssign};
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
    /// Differential voltage: `V(n1, n2)`.
    VoltageDiff(Node, Node),
    /// Branch current: `I(element)`.
    Current(ElementRef),
    /// Binary operation (arithmetic, comparison, or logical).
    BinOp(Box<Expr>, ExprBinOp, Box<Expr>),
    /// Unary negation: `-expr`.
    Neg(Box<Expr>),
    /// Logical NOT: `!expr`.
    Not(Box<Expr>),
    /// Ternary / conditional: `cond ? then_val : else_val`.
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    /// Built-in ngspice function call: `func(arg1, arg2, ...)`.
    Func(String, Vec<Expr>),
    /// Lookup table: `table(expr, x1,y1, x2,y2, ...)`.
    Table(Box<Expr>, Vec<(f64, f64)>),
    /// Special ngspice simulation variable (`time`, `frequency`, `temper` for temperature).
    SpecialVar(SpecialVar),
}

#[derive(Debug, Clone, Copy)]
pub enum ExprBinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    // Comparison (return 1.0 or 0.0 in ngspice)
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    // Logical (operate on 0/1 values)
    And,
    Or,
}

/// ngspice special simulation variables and physical constants.
#[derive(Debug, Clone, Copy)]
pub enum SpecialVar {
    /// Current simulation time (`time`).
    Time,
    /// Current AC frequency (`frequency`).
    Frequency,
    /// Circuit temperature in °C (renders as ngspice `temper`).
    Temp,
    /// Mathematical constant π (`pi` ≈ 3.14159).
    Pi,
    /// Boltzmann constant (`boltz` ≈ 1.38e-23 J/K).
    Boltz,
    /// Elementary charge (`echarge` ≈ 1.602e-19 C).
    Echarge,
    /// Planck constant (`planck` ≈ 6.626e-34 J·s).
    Planck,
}

impl Expr {
    pub fn param(name: impl Into<String>) -> Self {
        Expr::Param(name.into())
    }

    pub fn voltage(node: Node) -> Self {
        Expr::Voltage(node)
    }

    pub fn voltage_diff(plus: Node, minus: Node) -> Self {
        Expr::VoltageDiff(plus, minus)
    }

    pub fn current(elem: ElementRef) -> Self {
        Expr::Current(elem)
    }

    pub fn constant(v: f64) -> Self {
        Expr::Constant(v)
    }

    // ===== Special variables =====

    pub fn time() -> Self {
        Expr::SpecialVar(SpecialVar::Time)
    }

    pub fn frequency() -> Self {
        Expr::SpecialVar(SpecialVar::Frequency)
    }

    pub fn temp() -> Self {
        Expr::SpecialVar(SpecialVar::Temp)
    }

    // ===== Built-in math functions =====

    pub fn abs(e: Expr) -> Self   { Expr::Func("abs".into(), vec![e]) }
    pub fn sqrt(e: Expr) -> Self  { Expr::Func("sqrt".into(), vec![e]) }
    pub fn exp(e: Expr) -> Self   { Expr::Func("exp".into(), vec![e]) }
    pub fn ln(e: Expr) -> Self    { Expr::Func("ln".into(), vec![e]) }
    /// Base-10 logarithm. In ngspice `log()` is log10.
    pub fn log10(e: Expr) -> Self { Expr::Func("log".into(), vec![e]) }
    pub fn sin(e: Expr) -> Self   { Expr::Func("sin".into(), vec![e]) }
    pub fn cos(e: Expr) -> Self   { Expr::Func("cos".into(), vec![e]) }
    pub fn tan(e: Expr) -> Self   { Expr::Func("tan".into(), vec![e]) }
    pub fn asin(e: Expr) -> Self  { Expr::Func("asin".into(), vec![e]) }
    pub fn acos(e: Expr) -> Self  { Expr::Func("acos".into(), vec![e]) }
    pub fn atan(e: Expr) -> Self  { Expr::Func("atan".into(), vec![e]) }
    pub fn ceil(e: Expr) -> Self  { Expr::Func("ceil".into(), vec![e]) }
    pub fn floor(e: Expr) -> Self { Expr::Func("floor".into(), vec![e]) }
    pub fn min(a: Expr, b: Expr) -> Self { Expr::Func("min".into(), vec![a, b]) }
    pub fn max(a: Expr, b: Expr) -> Self { Expr::Func("max".into(), vec![a, b]) }

    /// Power: `self ^ exp`. Convenience method (same as `Expr ^ Expr`).
    pub fn pow(self, exp: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Pow, Box::new(exp.into()))
    }

    /// Escape hatch: arbitrary ngspice function by name.
    pub fn func(name: impl Into<String>, args: Vec<Expr>) -> Self {
        Expr::Func(name.into(), args)
    }

    // ===== Hyperbolic trig =====

    pub fn sinh(e: Expr) -> Self  { Expr::Func("sinh".into(), vec![e]) }
    pub fn cosh(e: Expr) -> Self  { Expr::Func("cosh".into(), vec![e]) }
    pub fn tanh(e: Expr) -> Self  { Expr::Func("tanh".into(), vec![e]) }
    pub fn asinh(e: Expr) -> Self { Expr::Func("asinh".into(), vec![e]) }
    pub fn acosh(e: Expr) -> Self { Expr::Func("acosh".into(), vec![e]) }
    pub fn atanh(e: Expr) -> Self { Expr::Func("atanh".into(), vec![e]) }
    pub fn atan2(y: Expr, x: Expr) -> Self { Expr::Func("atan2".into(), vec![y, x]) }

    // ===== Signal / behavioral functions =====

    /// Unit step function: `u(x)` = 1 if x > 0, else 0.
    pub fn u(e: Expr) -> Self    { Expr::Func("u".into(), vec![e]) }
    /// Ramp function: `uramp(x)` = x if x > 0, else 0.
    pub fn uramp(e: Expr) -> Self { Expr::Func("uramp".into(), vec![e]) }
    /// Sign function: `sgn(x)` = -1, 0, or 1.
    pub fn sgn(e: Expr) -> Self  { Expr::Func("sgn".into(), vec![e]) }
    /// Round to nearest integer: `nint(x)`.
    pub fn nint(e: Expr) -> Self { Expr::Func("nint".into(), vec![e]) }
    /// Signed power: `pwr(x, y)` = sign(x) * |x|^y.
    pub fn pwr(x: Expr, y: Expr) -> Self { Expr::Func("pwr".into(), vec![x, y]) }
    /// Clamp: `limit(x, lo, hi)` = max(lo, min(hi, x)).
    pub fn limit(x: Expr, lo: Expr, hi: Expr) -> Self {
        Expr::Func("limit".into(), vec![x, lo, hi])
    }

    // ===== Physical constants =====

    pub fn pi() -> Self      { Expr::SpecialVar(SpecialVar::Pi) }
    pub fn boltz() -> Self   { Expr::SpecialVar(SpecialVar::Boltz) }
    pub fn echarge() -> Self { Expr::SpecialVar(SpecialVar::Echarge) }
    pub fn planck() -> Self  { Expr::SpecialVar(SpecialVar::Planck) }

    /// Angular frequency: `2 * pi * frequency` (convenience, not a primitive).
    pub fn omega() -> Self {
        Expr::Constant(2.0) * Expr::pi() * Expr::frequency()
    }

    // ===== Lookup table =====

    /// Piecewise-linear lookup table: `table(expr, x1,y1, x2,y2, ...)`.
    ///
    /// `points` must have at least 2 entries and x values must be monotonically increasing.
    pub fn table(expr: Expr, points: Vec<(f64, f64)>) -> Self {
        Expr::Table(Box::new(expr), points)
    }

    // ===== Comparison operators (return 1.0 or 0.0) =====

    pub fn equal(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Eq, Box::new(rhs.into()))
    }
    pub fn neq(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Ne, Box::new(rhs.into()))
    }
    pub fn gt(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Gt, Box::new(rhs.into()))
    }
    pub fn gte(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Ge, Box::new(rhs.into()))
    }
    pub fn lt(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Lt, Box::new(rhs.into()))
    }
    pub fn lte(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Le, Box::new(rhs.into()))
    }

    // ===== Logical operators =====

    pub fn and(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::And, Box::new(rhs.into()))
    }
    pub fn or(self, rhs: impl Into<Expr>) -> Self {
        Expr::BinOp(Box::new(self), ExprBinOp::Or, Box::new(rhs.into()))
    }
    pub fn not(self) -> Self {
        Expr::Not(Box::new(self))
    }

    // ===== Conditional =====

    /// Ternary expression: `cond ? then_val : else_val`.
    ///
    /// In ngspice, `cond` is evaluated as a float: 0.0 = false, non-zero = true.
    /// Typically built from comparison operators: `voltage!(out).gt(val!(5.0))`.
    pub fn if_then_else(cond: Expr, then_val: Expr, else_val: Expr) -> Self {
        Expr::Ternary(Box::new(cond), Box::new(then_val), Box::new(else_val))
    }

    /// Renders the inner expression string (no wrapping delimiters).
    pub fn to_ngspice(&self) -> String {
        match self {
            Expr::Constant(v) => format!("{v}"),
            Expr::Param(s) => s.clone(),
            Expr::MeasResult(s) => s.clone(),
            Expr::Voltage(n) => format!("V({})", n.spice_name()),
            Expr::VoltageDiff(p, n) => format!("V({},{})", p.spice_name(), n.spice_name()),
            Expr::Current(e) => format!("I({})", e.spice_name()),
            Expr::BinOp(l, op, r) => {
                let op_str = match op {
                    ExprBinOp::Add => "+",
                    ExprBinOp::Sub => "-",
                    ExprBinOp::Mul => "*",
                    ExprBinOp::Div => "/",
                    ExprBinOp::Pow => "^",
                    ExprBinOp::Mod => "%",
                    ExprBinOp::Eq  => "==",
                    ExprBinOp::Ne  => "!=",
                    ExprBinOp::Gt  => ">",
                    ExprBinOp::Ge  => ">=",
                    ExprBinOp::Lt  => "<",
                    ExprBinOp::Le  => "<=",
                    ExprBinOp::And => "&&",
                    ExprBinOp::Or  => "||",
                };
                format!("({}{}{})", l.to_ngspice(), op_str, r.to_ngspice())
            }
            Expr::Neg(e) => format!("(-{})", e.to_ngspice()),
            Expr::Not(e) => format!("(!{})", e.to_ngspice()),
            Expr::Ternary(cond, then_val, else_val) => format!(
                "({} ? {} : {})",
                cond.to_ngspice(),
                then_val.to_ngspice(),
                else_val.to_ngspice()
            ),
            Expr::Func(name, args) => {
                let arg_strs: Vec<String> = args.iter().map(|a| a.to_ngspice()).collect();
                format!("{}({})", name, arg_strs.join(","))
            }
            Expr::Table(expr, points) => {
                let mut parts = vec![expr.to_ngspice()];
                for (x, y) in points {
                    parts.push(format!("{x}"));
                    parts.push(format!("{y}"));
                }
                format!("table({})", parts.join(","))
            }
            Expr::SpecialVar(sv) => match sv {
                SpecialVar::Time      => "time".to_string(),
                SpecialVar::Frequency => "frequency".to_string(),
                SpecialVar::Temp      => "temper".to_string(),
                SpecialVar::Pi        => "pi".to_string(),
                SpecialVar::Boltz     => "boltz".to_string(),
                SpecialVar::Echarge   => "echarge".to_string(),
                SpecialVar::Planck    => "planck".to_string(),
            },
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

impl Rem for Expr {
    type Output = Expr;
    fn rem(self, rhs: Expr) -> Expr {
        Expr::BinOp(Box::new(self), ExprBinOp::Mod, Box::new(rhs))
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

// ===== Ergonomic expression macros =====
//
// These mirror the `Expr::*` constructors but read more naturally in composed expressions:
//
//   sqrt!(voltage!(out).pow(2.0) + param!("offset"))
//   meas_max!(Probe::voltage(n))
//   min!(voltage!(a), voltage!(b))

/// Reference a `.param` variable: `param!("rval")` → `Expr::Param("rval")`.
#[macro_export]
macro_rules! param {
    ($name:expr) => {
        $crate::num::Expr::param($name)
    };
}

/// Node voltage leaf: `voltage!(n)` → `Expr::Voltage(n)`.
#[macro_export]
macro_rules! voltage {
    ($node:expr) => {
        $crate::num::Expr::voltage($node)
    };
}

/// Differential voltage: `vdiff!(p, n)` → `Expr::VoltageDiff(p, n)`.
#[macro_export]
macro_rules! vdiff {
    ($plus:expr, $minus:expr) => {
        $crate::num::Expr::voltage_diff($plus, $minus)
    };
}

/// Branch current leaf: `current!(elem_ref)` → `Expr::Current(elem_ref)`.
#[macro_export]
macro_rules! current {
    ($elem:expr) => {
        $crate::num::Expr::current($elem)
    };
}

/// Literal constant: `val!(3.14)` → `Expr::Constant(3.14)`.
#[macro_export]
macro_rules! val {
    ($v:expr) => {
        $crate::num::Expr::constant($v as f64)
    };
}

/// Current simulation time variable: `time!()` → `Expr::SpecialVar(Time)`.
#[macro_export]
macro_rules! time {
    () => {
        $crate::num::Expr::time()
    };
}

/// AC frequency variable: `freq!()` → `Expr::SpecialVar(Frequency)`.
#[macro_export]
macro_rules! freq {
    () => {
        $crate::num::Expr::frequency()
    };
}

/// Circuit temperature variable (°C): `temp!()` → renders as ngspice `temper`.
#[macro_export]
macro_rules! temp {
    () => {
        $crate::num::Expr::temp()
    };
}

// ===== Math function macros =====

/// `sqrt!(e)` — square root.
#[macro_export]
macro_rules! sqrt {
    ($e:expr) => {
        $crate::num::Expr::sqrt($e)
    };
}

/// `abs!(e)` — absolute value.
#[macro_export]
macro_rules! abs {
    ($e:expr) => {
        $crate::num::Expr::abs($e)
    };
}

/// `exp!(e)` — eˣ.
#[macro_export]
macro_rules! exp {
    ($e:expr) => {
        $crate::num::Expr::exp($e)
    };
}

/// `ln!(e)` — natural logarithm.
#[macro_export]
macro_rules! ln {
    ($e:expr) => {
        $crate::num::Expr::ln($e)
    };
}

/// `log10!(e)` — base-10 logarithm (ngspice `log()`).
#[macro_export]
macro_rules! log10 {
    ($e:expr) => {
        $crate::num::Expr::log10($e)
    };
}

/// `sin!(e)`, `cos!(e)`, `tan!(e)` — trig functions (radians).
#[macro_export]
macro_rules! sin {
    ($e:expr) => {
        $crate::num::Expr::sin($e)
    };
}
#[macro_export]
macro_rules! cos {
    ($e:expr) => {
        $crate::num::Expr::cos($e)
    };
}
#[macro_export]
macro_rules! tan {
    ($e:expr) => {
        $crate::num::Expr::tan($e)
    };
}

/// `asin!(e)`, `acos!(e)`, `atan!(e)` — inverse trig.
#[macro_export]
macro_rules! asin {
    ($e:expr) => {
        $crate::num::Expr::asin($e)
    };
}
#[macro_export]
macro_rules! acos {
    ($e:expr) => {
        $crate::num::Expr::acos($e)
    };
}
#[macro_export]
macro_rules! atan {
    ($e:expr) => {
        $crate::num::Expr::atan($e)
    };
}

/// `ceil!(e)` — ceiling (round up to nearest integer).
#[macro_export]
macro_rules! ceil {
    ($e:expr) => {
        $crate::num::Expr::ceil($e)
    };
}

/// `floor!(e)` — floor (round down to nearest integer).
#[macro_export]
macro_rules! floor {
    ($e:expr) => {
        $crate::num::Expr::floor($e)
    };
}

/// `min!(a, b)` — minimum of two expressions.
#[macro_export]
macro_rules! min {
    ($a:expr, $b:expr) => {
        $crate::num::Expr::min($a, $b)
    };
}

/// `max!(a, b)` — maximum of two expressions.
#[macro_export]
macro_rules! max {
    ($a:expr, $b:expr) => {
        $crate::num::Expr::max($a, $b)
    };
}

/// Arbitrary ngspice function: `ngfunc!("nint", e)` or `ngfunc!("atan2", a, b)`.
#[macro_export]
macro_rules! ngfunc {
    ($name:expr, $($arg:expr),+) => {
        $crate::num::Expr::func($name, vec![$($arg),+])
    };
}

// ===== Hyperbolic trig macros =====

#[macro_export]
macro_rules! sinh  { ($e:expr) => { $crate::num::Expr::sinh($e)  }; }
#[macro_export]
macro_rules! cosh  { ($e:expr) => { $crate::num::Expr::cosh($e)  }; }
#[macro_export]
macro_rules! tanh  { ($e:expr) => { $crate::num::Expr::tanh($e)  }; }
#[macro_export]
macro_rules! asinh { ($e:expr) => { $crate::num::Expr::asinh($e) }; }
#[macro_export]
macro_rules! acosh { ($e:expr) => { $crate::num::Expr::acosh($e) }; }
#[macro_export]
macro_rules! atanh { ($e:expr) => { $crate::num::Expr::atanh($e) }; }
#[macro_export]
macro_rules! atan2 { ($y:expr, $x:expr) => { $crate::num::Expr::atan2($y, $x) }; }

// ===== Signal / behavioral function macros =====

/// Unit step: `u!(x)` = 1 if x > 0, else 0.
#[macro_export]
macro_rules! u     { ($e:expr) => { $crate::num::Expr::u($e)    }; }
/// Ramp: `uramp!(x)` = x if x > 0, else 0.
#[macro_export]
macro_rules! uramp { ($e:expr) => { $crate::num::Expr::uramp($e) }; }
/// Sign: `sgn!(x)` = -1, 0, or 1.
#[macro_export]
macro_rules! sgn   { ($e:expr) => { $crate::num::Expr::sgn($e)  }; }
/// Round to nearest integer: `nint!(x)`.
#[macro_export]
macro_rules! nint  { ($e:expr) => { $crate::num::Expr::nint($e) }; }
/// Signed power: `pwr!(x, y)` = sign(x) * |x|^y.
#[macro_export]
macro_rules! pwr   { ($x:expr, $y:expr) => { $crate::num::Expr::pwr($x, $y) }; }
/// Clamp: `limit!(x, lo, hi)`.
#[macro_export]
macro_rules! limit { ($x:expr, $lo:expr, $hi:expr) => { $crate::num::Expr::limit($x, $lo, $hi) }; }

// ===== Physical constant macros =====

/// Mathematical constant π.
#[macro_export]
macro_rules! pi      { () => { $crate::num::Expr::pi()      }; }
/// Boltzmann constant.
#[macro_export]
macro_rules! boltz   { () => { $crate::num::Expr::boltz()   }; }
/// Elementary charge.
#[macro_export]
macro_rules! echarge { () => { $crate::num::Expr::echarge() }; }
/// Planck constant.
#[macro_export]
macro_rules! planck  { () => { $crate::num::Expr::planck()  }; }
/// Angular frequency: `2 * pi * freq`.
#[macro_export]
macro_rules! omega   { () => { $crate::num::Expr::omega()   }; }

// ===== Conditional / logical macros =====

/// Ternary: `ternary!(cond, then_val, else_val)` → `(cond ? then_val : else_val)`.
#[macro_export]
macro_rules! ternary {
    ($cond:expr, $then:expr, $else:expr) => {
        $crate::num::Expr::if_then_else($cond, $then, $else)
    };
}

/// Logical NOT: `not!(e)` → `(!e)`.
#[macro_export]
macro_rules! not {
    ($e:expr) => { $crate::num::Expr::not($e) };
}

// ===== Table macro =====

/// Lookup table: `table!(expr, [(x1,y1), (x2,y2), ...])`.
#[macro_export]
macro_rules! table {
    ($expr:expr, [$( ($x:expr, $y:expr) ),+ $(,)?]) => {
        $crate::num::Expr::table($expr, vec![$( ($x as f64, $y as f64) ),+])
    };
}
