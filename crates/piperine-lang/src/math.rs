//! Built-in math: the table of libm intrinsics available to JIT code, plus
//! the matching compile-time evaluator [`eval_const_math`], called by the const
//! interpreter (`crate::eval`) and by codegen's constant folder.

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
    // `limexp` is NOT in this table: LRM §4.5.13 gives it Newton-iteration
    // state (a `yold` slot), so it can't be a stateless wrapper here — it is
    // dispatched specially in `piperine-codegen`'s `emit_analog_limexp`
    // (`kernel/analog/limexp.rs`), which also means it can never be
    // elaboration-constant-folded (deliberate: folding would silently drop
    // the iteration-history behavior that motivates the function).
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

/// Look up a built-in by IR name. Per LRM Table 4-14, `log` is the decimal
/// (base-10) logarithm — it aliases `log10`, not `ln`.
pub fn math_fn(name: &str) -> Option<&'static MathFn> {
    let name = if name == "log" { "log10" } else { name };
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

#[cfg(test)]
mod tests {
    //! LRM-COVERS: 4.3

    use super::*;

    #[test]
    fn log_is_base_10() {
        assert_eq!(eval_const_math("log", &[100.0]), Some(2.0));
    }

    #[test]
    fn ln_is_natural() {
        let v = eval_const_math("ln", &[std::f64::consts::E.powf(1.0)]).unwrap();
        assert!((v - 1.0).abs() < 1e-12, "ln(exp(1.0)) = {v}");
    }

    // LRMW2-54 (T47): the audit's "missing from the catalogue" finding for
    // these six does not reproduce — they were already in `MATH_FNS`.
    // Table 4-14/4-15 round trip through the same `eval_const_math` path
    // `piperine-codegen`'s constant folder calls; the JITted-device round
    // trip lives in `crates/piperine-codegen/tests/math_intrinsics_lrm.rs`
    // (Done-when's own "not just the Rust wrapper" bar).

    #[test]
    fn ceil_rounds_toward_positive_infinity() {
        assert_eq!(eval_const_math("ceil", &[2.3]), Some(3.0));
        assert_eq!(eval_const_math("ceil", &[-2.3]), Some(-2.0));
    }

    #[test]
    fn atan2_is_arc_tangent_of_y_over_x() {
        let v = eval_const_math("atan2", &[1.0, 1.0]).unwrap();
        assert!((v - std::f64::consts::FRAC_PI_4).abs() < 1e-12, "atan2(1,1) = {v}");
    }

    #[test]
    fn hypot_is_the_euclidean_norm() {
        assert_eq!(eval_const_math("hypot", &[3.0, 4.0]), Some(5.0));
    }

    #[test]
    fn asinh_inverts_sinh() {
        let v = eval_const_math("asinh", &[1.0_f64.sinh()]).unwrap();
        assert!((v - 1.0).abs() < 1e-12, "asinh(sinh(1.0)) = {v}");
    }

    #[test]
    fn acosh_inverts_cosh() {
        let v = eval_const_math("acosh", &[2.0_f64.cosh()]).unwrap();
        assert!((v - 2.0).abs() < 1e-12, "acosh(cosh(2.0)) = {v}");
    }

    #[test]
    fn atanh_inverts_tanh() {
        let v = eval_const_math("atanh", &[0.5_f64.tanh()]).unwrap();
        assert!((v - 0.5).abs() < 1e-12, "atanh(tanh(0.5)) = {v}");
    }

    /// Arity errors are loud (Done-when) — `eval_const_math` returns
    /// `None` (the const-evaluator's own not-a-constant signal, which its
    /// caller turns into a fail-loud diagnostic) rather than silently
    /// truncating/padding the argument list.
    #[test]
    fn wrong_arity_is_not_silently_truncated() {
        assert_eq!(eval_const_math("hypot", &[3.0, 4.0, 5.0]), None);
        assert_eq!(eval_const_math("ceil", &[]), None);
    }
}
