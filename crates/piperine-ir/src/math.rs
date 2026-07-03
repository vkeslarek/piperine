//! Built-in math: the table of libm intrinsics available to JIT code, plus
//! the matching compile-time evaluator used by `IrExpr::eval_const`.

/// One built-in math function: IR name, arity, and the native symbol the JIT
/// links against.
#[derive(Debug, Clone, Copy)]
pub struct MathFn {
    pub name: &'static str,
    pub arity: usize,
    pub symbol: *const u8,
}

// The table is read-only function pointers; sharing across threads is safe.
unsafe impl Sync for MathFn {}

mod wrappers {
    pub extern "C" fn sin(x: f64) -> f64 { x.sin() }
    pub extern "C" fn cos(x: f64) -> f64 { x.cos() }
    pub extern "C" fn tan(x: f64) -> f64 { x.tan() }
    pub extern "C" fn asin(x: f64) -> f64 { x.asin() }
    pub extern "C" fn acos(x: f64) -> f64 { x.acos() }
    pub extern "C" fn atan(x: f64) -> f64 { x.atan() }
    pub extern "C" fn atan2(y: f64, x: f64) -> f64 { y.atan2(x) }
    pub extern "C" fn sinh(x: f64) -> f64 { x.sinh() }
    pub extern "C" fn cosh(x: f64) -> f64 { x.cosh() }
    pub extern "C" fn tanh(x: f64) -> f64 { x.tanh() }
    pub extern "C" fn asinh(x: f64) -> f64 { x.asinh() }
    pub extern "C" fn acosh(x: f64) -> f64 { x.acosh() }
    pub extern "C" fn atanh(x: f64) -> f64 { x.atanh() }
    pub extern "C" fn exp(x: f64) -> f64 { x.exp() }
    pub extern "C" fn ln(x: f64) -> f64 { x.ln() }
    pub extern "C" fn log10(x: f64) -> f64 { x.log10() }
    pub extern "C" fn sqrt(x: f64) -> f64 { x.sqrt() }
    pub extern "C" fn pow(b: f64, e: f64) -> f64 { b.powf(e) }
    pub extern "C" fn hypot(a: f64, b: f64) -> f64 { a.hypot(b) }
    pub extern "C" fn abs(x: f64) -> f64 { x.abs() }
    pub extern "C" fn min(a: f64, b: f64) -> f64 { a.min(b) }
    pub extern "C" fn max(a: f64, b: f64) -> f64 { a.max(b) }
    pub extern "C" fn floor(x: f64) -> f64 { x.floor() }
    pub extern "C" fn ceil(x: f64) -> f64 { x.ceil() }
    /// `limexp`: exp with the exponent clamped to 80 for Newton robustness.
    pub extern "C" fn limexp(x: f64) -> f64 { x.min(80.0).exp() }
}

/// All built-in math functions, keyed by their IR (`MathCall`) name.
pub const MATH_FNS: &[MathFn] = &[
    MathFn { name: "sin", arity: 1, symbol: wrappers::sin as *const u8 },
    MathFn { name: "cos", arity: 1, symbol: wrappers::cos as *const u8 },
    MathFn { name: "tan", arity: 1, symbol: wrappers::tan as *const u8 },
    MathFn { name: "asin", arity: 1, symbol: wrappers::asin as *const u8 },
    MathFn { name: "acos", arity: 1, symbol: wrappers::acos as *const u8 },
    MathFn { name: "atan", arity: 1, symbol: wrappers::atan as *const u8 },
    MathFn { name: "atan2", arity: 2, symbol: wrappers::atan2 as *const u8 },
    MathFn { name: "sinh", arity: 1, symbol: wrappers::sinh as *const u8 },
    MathFn { name: "cosh", arity: 1, symbol: wrappers::cosh as *const u8 },
    MathFn { name: "tanh", arity: 1, symbol: wrappers::tanh as *const u8 },
    MathFn { name: "asinh", arity: 1, symbol: wrappers::asinh as *const u8 },
    MathFn { name: "acosh", arity: 1, symbol: wrappers::acosh as *const u8 },
    MathFn { name: "atanh", arity: 1, symbol: wrappers::atanh as *const u8 },
    MathFn { name: "exp", arity: 1, symbol: wrappers::exp as *const u8 },
    MathFn { name: "limexp", arity: 1, symbol: wrappers::limexp as *const u8 },
    MathFn { name: "ln", arity: 1, symbol: wrappers::ln as *const u8 },
    MathFn { name: "log10", arity: 1, symbol: wrappers::log10 as *const u8 },
    MathFn { name: "sqrt", arity: 1, symbol: wrappers::sqrt as *const u8 },
    MathFn { name: "pow", arity: 2, symbol: wrappers::pow as *const u8 },
    MathFn { name: "hypot", arity: 2, symbol: wrappers::hypot as *const u8 },
    MathFn { name: "abs", arity: 1, symbol: wrappers::abs as *const u8 },
    MathFn { name: "min", arity: 2, symbol: wrappers::min as *const u8 },
    MathFn { name: "max", arity: 2, symbol: wrappers::max as *const u8 },
    MathFn { name: "floor", arity: 1, symbol: wrappers::floor as *const u8 },
    MathFn { name: "ceil", arity: 1, symbol: wrappers::ceil as *const u8 },
];

/// Look up a built-in by IR name. `log`/`ln` are aliases.
pub fn math_fn(name: &str) -> Option<&'static MathFn> {
    let name = if name == "log" { "ln" } else { name };
    MATH_FNS.iter().find(|f| f.name == name)
}

/// Compile-time evaluation of a built-in math call (for `eval_const`).
pub fn eval_const_math(name: &str, args: &[f64]) -> Option<f64> {
    let f = math_fn(name)?;
    if args.len() != f.arity {
        return None;
    }
    // Calling through the same wrappers the JIT links keeps compile-time and
    // runtime results bit-identical.
    let value = unsafe {
        match f.arity {
            1 => std::mem::transmute::<*const u8, extern "C" fn(f64) -> f64>(f.symbol)(args[0]),
            2 => std::mem::transmute::<*const u8, extern "C" fn(f64, f64) -> f64>(f.symbol)(
                args[0], args[1],
            ),
            _ => return None,
        }
    };
    Some(value)
}
