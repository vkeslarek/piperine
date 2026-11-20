//! `Real` and `Dual<N>`: the numbers every analog function of generated code is generic over.
//! `f64` is a `Real`; `Dual<N>` carries a value and its derivatives against the `N`
//! differentiable reads of one function, a fixed array, with the formulas of the interpreter's
//! dual numbers component by component, so the two backends compute alike.

use core::ops::{Add, Div, Mul, Neg, Sub};

/// A number analog code computes with: a plain `f64`, or a value with its tangent.
pub trait Real:
    Copy + Default + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self>
{
    /// A value with no dependence on any read.
    fn constant(v: f64) -> Self;
    /// The value, its tangent dropped.
    fn value(self) -> f64;
    fn exp(self) -> Self;
    fn ln(self) -> Self;
    fn sqrt(self) -> Self;
    /// The derivative is `sign(v)`; at exactly zero it is zero.
    fn abs(self) -> Self;
    /// The smaller of the two; the tangent is that of the one chosen, `self` on a tie.
    fn min(self, other: Self) -> Self;
    /// The larger of the two; the tangent is that of the one chosen, `self` on a tie.
    fn max(self, other: Self) -> Self;
    /// `self` held in `lo..=hi`, as `self.max(lo).min(hi)`; panics when `lo > hi` or a bound is NaN,
    /// as Rust's `clamp` does. The tangent is that of the bound
    /// when the value is held at it, and of `self` inside.
    fn clamp(self, lo: Self, hi: Self) -> Self;
    /// The largest integer not above the value; its tangent is zero.
    fn floor(self) -> Self;
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn tan(self) -> Self;
    fn asin(self) -> Self;
    fn acos(self) -> Self;
    fn atan(self) -> Self;
    /// `atan2(self, x)`: the angle of the point `(x, self)`; the tangent is `(x·dy − y·dx) / (x² + y²)`.
    fn atan2(self, x: Self) -> Self;
    fn sinh(self) -> Self;
    fn cosh(self) -> Self;
    fn tanh(self) -> Self;
    /// `self` to the power `e`; the tangent is `e·x^(e−1)·dx + x^e·ln(x)·de`, a part whose
    /// tangent is zero contributing nothing (a constant exponent never meets `ln` of a negative).
    fn powf(self, e: Self) -> Self;
    /// `self` to the integer power `n`; the tangent is `n·x^(n−1)·dx`, zero for `n = 0`.
    fn powi(self, n: i32) -> Self;
    fn log10(self) -> Self;
    fn log2(self) -> Self;
    /// The smallest integer not below the value; its tangent is zero.
    fn ceil(self) -> Self;
    /// The nearest integer, halves away from zero; its tangent is zero.
    fn round(self) -> Self;
    /// The integer part; its tangent is zero.
    fn trunc(self) -> Self;
    /// `self - self.trunc()`; its tangent is the value's own.
    fn fract(self) -> Self;
    /// `1.0` for a positive value or `+0.0`, `-1.0` for a negative or `-0.0`, NaN for NaN, as
    /// Rust's; its tangent is zero.
    fn signum(self) -> Self;
    /// The Euclidean remainder of `self / d`, never negative; the tangent is
    /// `dx − q·dd` with `q = (self − r) / d` the integer quotient, `dx` alone for a constant `d`.
    fn rem_euclid(self, d: Self) -> Self;
    /// `sqrt(self² + o²)`; the tangent is `(x·dx + y·dy) / h`.
    fn hypot(self, o: Self) -> Self;
    fn is_nan(self) -> bool;
    fn is_finite(self) -> bool;
    fn is_infinite(self) -> bool;
    /// Whether `self` and `other` are the same number down to the bits, tangent included: how the
    /// merge of an unknown `if` decides that two paths agree (§3.3.3). A NaN is the same as itself.
    fn same_levels(self, other: Self) -> bool;
}

impl Real for f64 {
    fn constant(v: f64) -> f64 {
        v
    }

    fn value(self) -> f64 {
        self
    }

    fn exp(self) -> f64 {
        f64::exp(self)
    }

    fn ln(self) -> f64 {
        f64::ln(self)
    }

    fn sqrt(self) -> f64 {
        f64::sqrt(self)
    }

    fn abs(self) -> f64 {
        f64::abs(self)
    }

    fn min(self, other: f64) -> f64 {
        f64::min(self, other)
    }

    fn max(self, other: f64) -> f64 {
        f64::max(self, other)
    }

    fn clamp(self, lo: f64, hi: f64) -> f64 {
        // as Rust's `clamp` and H.2: a crossed pair of bounds, or a NaN bound, panics
        if !(lo <= hi) {
            panic!("`clamp` with a lower bound {lo:?} above the upper bound {hi:?}, or a NaN bound");
        }
        f64::min(f64::max(self, lo), hi)
    }

    fn floor(self) -> f64 {
        f64::floor(self)
    }

    fn sin(self) -> f64 {
        f64::sin(self)
    }

    fn cos(self) -> f64 {
        f64::cos(self)
    }

    fn tan(self) -> f64 {
        f64::tan(self)
    }

    fn asin(self) -> f64 {
        f64::asin(self)
    }

    fn acos(self) -> f64 {
        f64::acos(self)
    }

    fn atan(self) -> f64 {
        f64::atan(self)
    }

    fn atan2(self, x: f64) -> f64 {
        f64::atan2(self, x)
    }

    fn sinh(self) -> f64 {
        f64::sinh(self)
    }

    fn cosh(self) -> f64 {
        f64::cosh(self)
    }

    fn tanh(self) -> f64 {
        f64::tanh(self)
    }

    fn powf(self, e: f64) -> f64 {
        f64::powf(self, e)
    }

    fn powi(self, n: i32) -> f64 {
        f64::powi(self, n)
    }

    fn log10(self) -> f64 {
        f64::log10(self)
    }

    fn log2(self) -> f64 {
        f64::log2(self)
    }

    fn ceil(self) -> f64 {
        f64::ceil(self)
    }

    fn round(self) -> f64 {
        f64::round(self)
    }

    fn trunc(self) -> f64 {
        f64::trunc(self)
    }

    fn fract(self) -> f64 {
        f64::fract(self)
    }

    fn signum(self) -> f64 {
        f64::signum(self)
    }

    fn rem_euclid(self, d: f64) -> f64 {
        f64::rem_euclid(self, d)
    }

    fn hypot(self, o: f64) -> f64 {
        f64::hypot(self, o)
    }

    fn is_nan(self) -> bool {
        f64::is_nan(self)
    }

    fn is_finite(self) -> bool {
        f64::is_finite(self)
    }

    fn is_infinite(self) -> bool {
        f64::is_infinite(self)
    }

    fn same_levels(self, other: f64) -> bool {
        self.to_bits() == other.to_bits()
    }
}

/// A value and its tangent: `d[i]` is the derivative against differentiable read `i`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dual<const N: usize> {
    pub v: f64,
    pub d: [f64; N],
}

impl<const N: usize> Default for Dual<N> {
    /// Zero, with a zero tangent (what a `Flagged<Dual<N>>` holds when unknown).
    fn default() -> Dual<N> {
        Dual { v: 0.0, d: [0.0; N] }
    }
}

impl<const N: usize> Dual<N> {
    /// The read in `slot`: derivative 1 against itself, 0 against the rest.
    pub fn seeded(v: f64, slot: usize) -> Dual<N> {
        let mut d = [0.0; N];
        d[slot] = 1.0;
        Dual { v, d }
    }

    /// `f(v)` with derivative `scale · d`.
    fn chain(self, v: f64, scale: f64) -> Dual<N> {
        let mut d = self.d;
        for x in &mut d {
            *x = scale * *x;
        }
        Dual { v, d }
    }

    /// `a·self.d + b·other.d`, entry by entry, an entry whose tangent is zero contributing
    /// nothing: `inf·0` and `NaN·0` of a constant operand do not reach the result.
    fn combine_known(self, a: f64, other: Dual<N>, b: f64) -> [f64; N] {
        let mut d = [0.0; N];
        for (i, x) in d.iter_mut().enumerate() {
            let left = if self.d[i] == 0.0 { 0.0 } else { a * self.d[i] };
            let right = if other.d[i] == 0.0 { 0.0 } else { b * other.d[i] };
            *x = left + right;
        }
        d
    }

    /// `a·self.d + b·other.d`, entry by entry.
    fn combine(self, a: f64, other: Dual<N>, b: f64) -> [f64; N] {
        let mut d = [0.0; N];
        for (i, x) in d.iter_mut().enumerate() {
            *x = a * self.d[i] + b * other.d[i];
        }
        d
    }
}

impl<const N: usize> Real for Dual<N> {
    fn constant(v: f64) -> Dual<N> {
        Dual { v, d: [0.0; N] }
    }

    fn value(self) -> f64 {
        self.v
    }

    fn exp(self) -> Dual<N> {
        let e = self.v.exp();
        self.chain(e, e)
    }

    fn ln(self) -> Dual<N> {
        self.chain(self.v.ln(), 1.0 / self.v)
    }

    fn sqrt(self) -> Dual<N> {
        let s = self.v.sqrt();
        self.chain(s, 0.5 / s)
    }

    fn abs(self) -> Dual<N> {
        let sign = if self.v > 0.0 {
            1.0
        } else if self.v < 0.0 {
            -1.0
        } else {
            0.0
        };
        self.chain(self.v.abs(), sign)
    }

    fn min(self, other: Dual<N>) -> Dual<N> {
        if self.v <= other.v { self } else { other }
    }

    fn max(self, other: Dual<N>) -> Dual<N> {
        if self.v >= other.v { self } else { other }
    }

    fn clamp(self, lo: Dual<N>, hi: Dual<N>) -> Dual<N> {
        if !(lo.v <= hi.v) {
            panic!("`clamp` with a lower bound {:?} above the upper bound {:?}, or a NaN bound", lo.v, hi.v);
        }
        Real::min(Real::max(self, lo), hi)
    }

    fn floor(self) -> Dual<N> {
        Dual { v: self.v.floor(), d: [0.0; N] }
    }

    fn sin(self) -> Dual<N> {
        self.chain(self.v.sin(), self.v.cos())
    }

    fn cos(self) -> Dual<N> {
        self.chain(self.v.cos(), -self.v.sin())
    }

    fn tan(self) -> Dual<N> {
        let t = self.v.tan();
        self.chain(t, 1.0 + t * t)
    }

    fn asin(self) -> Dual<N> {
        self.chain(self.v.asin(), 1.0 / (1.0 - self.v * self.v).sqrt())
    }

    fn acos(self) -> Dual<N> {
        self.chain(self.v.acos(), -1.0 / (1.0 - self.v * self.v).sqrt())
    }

    fn atan(self) -> Dual<N> {
        self.chain(self.v.atan(), 1.0 / (1.0 + self.v * self.v))
    }

    fn atan2(self, x: Dual<N>) -> Dual<N> {
        let r2 = x.v * x.v + self.v * self.v;
        Dual { v: self.v.atan2(x.v), d: self.combine_known(x.v / r2, x, -self.v / r2) }
    }

    fn sinh(self) -> Dual<N> {
        self.chain(self.v.sinh(), self.v.cosh())
    }

    fn cosh(self) -> Dual<N> {
        self.chain(self.v.cosh(), self.v.sinh())
    }

    fn tanh(self) -> Dual<N> {
        let t = self.v.tanh();
        self.chain(t, 1.0 - t * t)
    }

    fn powf(self, e: Dual<N>) -> Dual<N> {
        let p = self.v.powf(e.v);
        Dual { v: p, d: self.combine_known(e.v * self.v.powf(e.v - 1.0), e, p * self.v.ln()) }
    }

    fn powi(self, n: i32) -> Dual<N> {
        let scale = if n == 0 { 0.0 } else { f64::from(n) * self.v.powi(n - 1) };
        self.chain(self.v.powi(n), scale)
    }

    fn log10(self) -> Dual<N> {
        self.chain(self.v.log10(), 1.0 / (self.v * core::f64::consts::LN_10))
    }

    fn log2(self) -> Dual<N> {
        self.chain(self.v.log2(), 1.0 / (self.v * core::f64::consts::LN_2))
    }

    fn ceil(self) -> Dual<N> {
        Dual { v: self.v.ceil(), d: [0.0; N] }
    }

    fn round(self) -> Dual<N> {
        Dual { v: self.v.round(), d: [0.0; N] }
    }

    fn trunc(self) -> Dual<N> {
        Dual { v: self.v.trunc(), d: [0.0; N] }
    }

    fn fract(self) -> Dual<N> {
        self.chain(self.v.fract(), 1.0)
    }

    fn signum(self) -> Dual<N> {
        Dual { v: self.v.signum(), d: [0.0; N] }
    }

    fn rem_euclid(self, d: Dual<N>) -> Dual<N> {
        let r = self.v.rem_euclid(d.v);
        let q = (self.v - r) / d.v;
        Dual { v: r, d: self.combine_known(1.0, d, -q) }
    }

    fn hypot(self, o: Dual<N>) -> Dual<N> {
        let h = self.v.hypot(o.v);
        Dual { v: h, d: self.combine_known(self.v / h, o, o.v / h) }
    }

    fn is_nan(self) -> bool {
        self.v.is_nan()
    }

    fn is_finite(self) -> bool {
        self.v.is_finite()
    }

    fn is_infinite(self) -> bool {
        self.v.is_infinite()
    }

    fn same_levels(self, other: Dual<N>) -> bool {
        self.v.to_bits() == other.v.to_bits() && self.d.iter().zip(other.d).all(|(a, b)| a.to_bits() == b.to_bits())
    }
}

impl<const N: usize> Add for Dual<N> {
    type Output = Dual<N>;
    fn add(self, rhs: Dual<N>) -> Dual<N> {
        Dual { v: self.v + rhs.v, d: self.combine(1.0, rhs, 1.0) }
    }
}

impl<const N: usize> Sub for Dual<N> {
    type Output = Dual<N>;
    fn sub(self, rhs: Dual<N>) -> Dual<N> {
        Dual { v: self.v - rhs.v, d: self.combine(1.0, rhs, -1.0) }
    }
}

impl<const N: usize> Mul for Dual<N> {
    type Output = Dual<N>;
    fn mul(self, rhs: Dual<N>) -> Dual<N> {
        Dual { v: self.v * rhs.v, d: self.combine(rhs.v, rhs, self.v) }
    }
}

impl<const N: usize> Div for Dual<N> {
    type Output = Dual<N>;
    fn div(self, rhs: Dual<N>) -> Dual<N> {
        let q = self.v / rhs.v;
        Dual { v: q, d: self.combine(1.0 / rhs.v, rhs, -q / rhs.v) }
    }
}

impl<const N: usize> Neg for Dual<N> {
    type Output = Dual<N>;
    fn neg(self) -> Dual<N> {
        self.chain(-self.v, -1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Dual, Real};
    use piperine_interp::Dual as Reference;

    /// 200 points in (-8, 8), from a fixed linear congruential sequence.
    fn points() -> Vec<f64> {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        (0..200)
            .map(|_| {
                state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                ((state >> 11) as f64 / (1u64 << 53) as f64) * 16.0 - 8.0
            })
            .collect()
    }

    /// The value's bits equal, every tangent entry equal as `f64`; an entry the reference does
    /// not hold is zero there (its tangent is empty or shorter).
    fn same(what: &str, x: f64, ours: Dual<2>, reference: &Reference) {
        assert_eq!(ours.v.to_bits(), reference.v.to_bits(), "{what} at {x}: value {} vs {}", ours.v, reference.v);
        for (i, d) in ours.d.iter().enumerate() {
            assert_eq!(*d, reference.tangent(i), "{what} at {x}: d[{i}]");
        }
    }

    #[test]
    fn dual_gives_the_interpreters_values_and_tangents() {
        let xs = points();
        for (k, &x) in xs.iter().enumerate() {
            let y = xs[(k + 1) % xs.len()];
            let (a, b) = (Dual::<2>::seeded(x, 0), Dual::<2>::seeded(y, 1));
            let (ra, rb) = (Reference::seeded(x, 0, 2), Reference::seeded(y, 1, 2));
            // the four operators and negation, both operands seeded
            same("a + b", x, a + b, &(ra.clone() + rb.clone()));
            same("a - b", x, a - b, &(ra.clone() - rb.clone()));
            same("a * b", x, a * b, &(ra.clone() * rb.clone()));
            same("a / b", x, a / b, &(ra.clone() / rb.clone()));
            same("-a", x, -a, &(-ra.clone()));
            // a constant on either side: the reference holds an empty tangent there
            let (c, rc) = (Dual::<2>::constant(y), Reference::constant(y));
            same("c * a", x, c * a, &(rc.clone() * ra.clone()));
            same("a / c", x, a / c, &(ra.clone() / rc.clone()));
            same("c - a", x, c - a, &(rc.clone() - ra.clone()));
            same("c + c", x, c + c, &(rc.clone() + rc.clone()));
            // the methods; `ln` and `sqrt` over |x|, where they are real
            same("exp", x, a.exp(), &ra.exp());
            same("abs", x, a.abs(), &ra.abs());
            let (p, rp) = (Dual::<2>::seeded(x.abs(), 0), Reference::seeded(x.abs(), 0, 2));
            same("ln", x, p.ln(), &rp.ln());
            same("sqrt", x, p.sqrt(), &rp.sqrt());
            // a composition, as a diode's current reads: is * (exp(v / nvt) - 1), nvt kept from 0
            let ours = c * ((a / (b.abs() + Dual::constant(1.0))).exp() - Dual::constant(1.0));
            let one = Reference::constant(1.0);
            let theirs = rc.clone() * ((ra.clone() / (rb.abs() + one.clone())).exp() - one);
            same("is * (exp(a / (|b| + 1)) - 1)", x, ours, &theirs);
            assert_eq!(a.value().to_bits(), x.to_bits());
        }
    }

    #[test]
    fn abs_has_the_derivative_of_sign() {
        for x in [0.5, 1.7, 3.0] {
            let pos = Dual::<1>::seeded(x, 0).abs();
            assert_eq!((pos.v, pos.d), (x, [1.0]));
            let neg = Dual::<1>::seeded(-x, 0).abs();
            assert_eq!((neg.v, neg.d), (x, [-1.0]));
        }
        assert_eq!(Dual::<1>::seeded(0.0, 0).abs().d, [0.0]);
    }

    #[test]
    fn min_max_clamp_and_floor_carry_the_tangent_of_the_value_chosen() {
        let (x, lo, hi) = (Dual::<3>::seeded(0.4, 0), Dual::<3>::seeded(0.0, 1), Dual::<3>::seeded(1.0, 2));
        // inside the bounds the tangent is the value's
        let inside = x.clamp(lo, hi);
        assert_eq!((inside.v, inside.d), (0.4, [1.0, 0.0, 0.0]));
        // below the low bound, the bound's; above the high bound, the bound's
        let below = Dual::<3>::seeded(-2.0, 0).clamp(lo, hi);
        assert_eq!((below.v, below.d), (0.0, [0.0, 1.0, 0.0]));
        let above = Dual::<3>::seeded(5.0, 0).clamp(lo, hi);
        assert_eq!((above.v, above.d), (1.0, [0.0, 0.0, 1.0]));
        // a tie keeps `self`
        let tie = Dual::<3>::seeded(1.0, 0).min(hi);
        assert_eq!(tie.d, [1.0, 0.0, 0.0]);
        assert_eq!(Dual::<3>::seeded(0.0, 0).max(lo).d, [1.0, 0.0, 0.0]);
        // floor has no tangent
        let f = Dual::<3>::seeded(2.7, 0).floor();
        assert_eq!((f.v, f.d), (2.0, [0.0; 3]));
        // the plain real is the standard library's
        for v in points() {
            assert_eq!(Real::clamp(v, -1.0, 2.0).to_bits(), v.max(-1.0).min(2.0).to_bits());
            assert_eq!(Real::floor(v).to_bits(), v.floor().to_bits());
            assert_eq!(Real::min(v, 1.0).to_bits(), v.min(1.0).to_bits());
            assert_eq!(Real::max(v, 1.0).to_bits(), v.max(1.0).to_bits());
        }
    }

    /// One expression written once over `Real`.
    fn diode<R: Real>(v: R, is: R, nvt: R) -> R {
        is * ((v / nvt).exp() - R::constant(1.0)) + (v * v).sqrt().ln().abs() - -v
    }

    #[test]
    fn f64_is_a_real() {
        for x in points() {
            let (is, nvt) = (1e-14, 0.025852);
            let generic = diode(x, is, nvt);
            let plain = is * ((x / nvt).exp() - 1.0) + (x * x).sqrt().ln().abs() - -x;
            assert_eq!(generic.to_bits(), plain.to_bits(), "at {x}");
            assert_eq!(<f64 as Real>::constant(x).to_bits(), x.to_bits());
            assert_eq!(Real::value(x).to_bits(), x.to_bits());
            // the value of the dual instantiation is the plain one, bit for bit
            let dual = diode(Dual::<1>::seeded(x, 0), Dual::constant(is), Dual::constant(nvt));
            assert_eq!(dual.value().to_bits(), plain.to_bits(), "at {x}");
        }
    }

    /// The rows of Appendix H.2 by their MSL name, once over `Real` and once over Rust's `f64`.
    struct Rows;

    impl Rows {
        /// Every row that returns a number.
        const NAMES: [&'static str; 24] = [
            "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "powf", "powi", "log10", "log2", "ceil", "round", "trunc",
            "fract", "signum", "rem_euclid", "hypot", "floor", "abs", "sqrt",
        ];

        fn real<R: Real>(name: &str, x: R, y: R, n: i32) -> R {
            match name {
                "sin" => x.sin(),
                "cos" => x.cos(),
                "tan" => x.tan(),
                "asin" => x.asin(),
                "acos" => x.acos(),
                "atan" => x.atan(),
                "atan2" => x.atan2(y),
                "sinh" => x.sinh(),
                "cosh" => x.cosh(),
                "tanh" => x.tanh(),
                "powf" => x.powf(y),
                "powi" => x.powi(n),
                "log10" => x.log10(),
                "log2" => x.log2(),
                "ceil" => x.ceil(),
                "round" => x.round(),
                "trunc" => x.trunc(),
                "fract" => x.fract(),
                "signum" => x.signum(),
                "rem_euclid" => x.rem_euclid(y),
                "hypot" => x.hypot(y),
                "floor" => x.floor(),
                "abs" => x.abs(),
                "sqrt" => x.sqrt(),
                other => panic!("no row {other}"),
            }
        }

        /// Rust's own method of the row on plain `f64`.
        fn rust(name: &str, x: f64, y: f64, n: i32) -> f64 {
            match name {
                "sin" => f64::sin(x),
                "cos" => f64::cos(x),
                "tan" => f64::tan(x),
                "asin" => f64::asin(x),
                "acos" => f64::acos(x),
                "atan" => f64::atan(x),
                "atan2" => f64::atan2(x, y),
                "sinh" => f64::sinh(x),
                "cosh" => f64::cosh(x),
                "tanh" => f64::tanh(x),
                "powf" => f64::powf(x, y),
                "powi" => f64::powi(x, n),
                "log10" => f64::log10(x),
                "log2" => f64::log2(x),
                "ceil" => f64::ceil(x),
                "round" => f64::round(x),
                "trunc" => f64::trunc(x),
                "fract" => f64::fract(x),
                "signum" => f64::signum(x),
                "rem_euclid" => f64::rem_euclid(x, y),
                "hypot" => f64::hypot(x, y),
                "floor" => f64::floor(x),
                "abs" => f64::abs(x),
                "sqrt" => f64::sqrt(x),
                other => panic!("no row {other}"),
            }
        }

        /// The bits equal, or both NaN.
        fn same(a: f64, b: f64) -> bool {
            a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
        }
    }

    #[test]
    fn each_row_has_the_value_of_rusts_method() {
        let xs = points();
        for (k, &x) in xs.iter().enumerate() {
            let y = xs[(k + 1) % xs.len()];
            for name in Rows::NAMES {
                for n in [-3, 0, 2, 5] {
                    let want = Rows::rust(name, x, y, n);
                    // the plain real, and the dual's value, in the domain and outside it (NaN rows)
                    let plain = Rows::real::<f64>(name, x, y, n);
                    assert!(Rows::same(plain, want), "{name}({x}, {y}, {n}) on f64: {plain} vs {want}");
                    let dual = Rows::real(name, Dual::<2>::seeded(x, 0), Dual::<2>::seeded(y, 1), n).v;
                    assert!(Rows::same(dual, want), "{name}({x}, {y}, {n}) on Dual: {dual} vs {want}");
                }
            }
        }
        // is_nan, is_finite and is_infinite of a finite value, NaN and both infinities
        for (v, nan, finite, infinite) in [(1.5, false, true, false), (f64::NAN, true, false, false), (f64::INFINITY, false, false, true), (f64::NEG_INFINITY, false, false, true)] {
            assert_eq!((Real::is_nan(v), Real::is_finite(v), Real::is_infinite(v)), (nan, finite, infinite), "f64 {v}");
            let d = Dual::<1>::seeded(v, 0);
            assert_eq!((d.is_nan(), d.is_finite(), d.is_infinite()), (nan, finite, infinite), "Dual {v}");
        }
    }

    #[test]
    fn the_differentiable_rows_agree_with_finite_differences() {
        // Central differences with step H. The truncation error is H²/6 times the third
        // derivative and the rounding error is about ε·|f|/H; for the arguments below the
        // third derivative is under 1e3 and |f| under 1e2, so both stay under 1e-9. Every
        // partial agrees with the quotient to BOUND, relative to 1 + |partial|.
        const H: f64 = 1e-6;
        const BOUND: f64 = 1e-6;
        let one = |xs: &[f64]| xs.iter().map(|&x| (x, 0.0)).collect::<Vec<_>>();
        let rows: [(&str, Vec<(f64, f64)>); 15] = [
            ("sin", one(&[-2.5, -0.7, 0.3, 1.9])),
            ("cos", one(&[-2.5, -0.7, 0.3, 1.9])),
            ("tan", one(&[-1.2, -0.4, 0.3, 1.1])),
            ("asin", one(&[-0.8, -0.3, 0.2, 0.7])),
            ("acos", one(&[-0.8, -0.3, 0.2, 0.7])),
            ("atan", one(&[-2.5, -0.7, 0.3, 1.9])),
            ("sinh", one(&[-2.5, -0.7, 0.3, 1.9])),
            ("cosh", one(&[-2.5, -0.7, 0.3, 1.9])),
            ("tanh", one(&[-2.5, -0.7, 0.3, 1.9])),
            ("log10", one(&[0.3, 1.7, 4.2])),
            ("log2", one(&[0.3, 1.7, 4.2])),
            ("atan2", vec![(0.7, 1.3), (-1.1, 0.4), (2.0, -0.9)]),
            ("powf", vec![(1.7, 2.3), (0.4, -1.2), (3.1, 0.5)]),
            ("hypot", vec![(0.7, 1.3), (-1.1, 0.4), (2.0, -0.9)]),
            ("powi", one(&[-1.9, 0.6, 2.4])),
        ];
        for (name, points) in &rows {
            let exponents = if *name == "powi" { vec![3, -2, 0, 1] } else { vec![0] };
            for n in exponents {
                for &(x, y) in points {
                    let at = |x: f64, y: f64| Rows::real::<f64>(name, x, y, n);
                    let out = Rows::real(name, Dual::<2>::seeded(x, 0), Dual::<2>::seeded(y, 1), n);
                    let fd = [(at(x + H, y) - at(x - H, y)) / (2.0 * H), (at(x, y + H) - at(x, y - H)) / (2.0 * H)];
                    // a one-argument row does not depend on `y`: its second partial is zero
                    let want = if matches!(*name, "atan2" | "powf" | "hypot") { fd } else { [fd[0], 0.0] };
                    for (i, (got, want)) in out.d.iter().zip(want).enumerate() {
                        assert!((got - want).abs() <= BOUND * (1.0 + got.abs()), "{name}({x}, {y}, n = {n}) d[{i}]: {got} vs finite difference {want}");
                    }
                }
            }
        }
        // a constant exponent meets no `ln` of a negative base: d/dx x^3 at -2 is 3 * 4 = 12
        let cube = Dual::<1>::seeded(-2.0, 0).powf(Dual::constant(3.0));
        assert_eq!((cube.v, cube.d), (-8.0, [12.0]));
    }

    #[test]
    fn the_piecewise_rows_have_derivative_zero_or_one() {
        for x in [-2.7, -0.4, 0.3, 1.5, 2.2] {
            let a = Dual::<2>::seeded(x, 0);
            // floor, ceil, round, trunc and signum are constant between their jumps: tangent 0
            for name in ["floor", "ceil", "round", "trunc", "signum"] {
                let out = Rows::real(name, a, Dual::constant(0.0), 0);
                assert_eq!(out.d, [0.0, 0.0], "{name}({x})");
            }
            // fract and rem_euclid by a constant move one for one with the receiver
            assert_eq!(a.fract().d, [1.0, 0.0], "fract({x})");
            assert_eq!(a.rem_euclid(Dual::constant(3.0)).d, [1.0, 0.0], "rem_euclid({x}, 3)");
        }
        // the divisor of rem_euclid enters with minus the integer quotient: 7.5 = 3 * 2 + 1.5
        // and -7.5 = -4 * 2 + 0.5
        let up = Dual::<2>::seeded(7.5, 0).rem_euclid(Dual::seeded(2.0, 1));
        assert_eq!((up.v, up.d), (1.5, [1.0, -3.0]));
        let down = Dual::<2>::seeded(-7.5, 0).rem_euclid(Dual::seeded(2.0, 1));
        assert_eq!((down.v, down.d), (0.5, [1.0, 4.0]));
    }
}
