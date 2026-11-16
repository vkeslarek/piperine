//! `Real` and `Dual<N>`: the numbers every analog function of generated code is generic over.
//! `f64` is a `Real`; `Dual<N>` carries a value and its derivatives against the `N`
//! differentiable reads of one function, a fixed array, with the formulas of the interpreter's
//! dual numbers component by component, so the two backends compute alike.

use core::ops::{Add, Div, Mul, Neg, Sub};

/// A number analog code computes with: a plain `f64`, or a value with its tangent.
pub trait Real:
    Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self>
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
    /// `self` held in `lo..=hi`, as `self.max(lo).min(hi)`: the tangent is that of the bound
    /// when the value is held at it, and of `self` inside.
    fn clamp(self, lo: Self, hi: Self) -> Self;
    /// The largest integer not above the value; its tangent is zero.
    fn floor(self) -> Self;
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
        f64::min(f64::max(self, lo), hi)
    }

    fn floor(self) -> f64 {
        f64::floor(self)
    }
}

/// A value and its tangent: `d[i]` is the derivative against differentiable read `i`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dual<const N: usize> {
    pub v: f64,
    pub d: [f64; N],
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
        Real::min(Real::max(self, lo), hi)
    }

    fn floor(self) -> Dual<N> {
        Dual { v: self.v.floor(), d: [0.0; N] }
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
}
