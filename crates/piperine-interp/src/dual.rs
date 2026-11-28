//! Forward-mode dual numbers: a value and its derivatives against the slots of one function.
//! An empty tangent means every derivative is zero, so arithmetic among constants allocates
//! nothing.

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A value and its tangent, one entry per differentiable read of the running function.
#[derive(Debug, Clone, PartialEq)]
pub struct Dual {
    pub v: f64,
    pub d: Vec<f64>,
}

impl Dual {
    /// A value with no dependence on any read.
    pub fn constant(v: f64) -> Dual {
        Dual { v, d: Vec::new() }
    }

    /// The read in `slot` of `slots`: derivative 1 against itself, 0 against the rest.
    pub fn seeded(v: f64, slot: usize, slots: usize) -> Dual {
        let mut d = vec![0.0; slots];
        d[slot] = 1.0;
        Dual { v, d }
    }

    /// The tangent entry of `slot`; zero when the tangent is empty or shorter.
    pub fn tangent(&self, slot: usize) -> f64 {
        self.d.get(slot).copied().unwrap_or(0.0)
    }

    /// `f(v)` with derivative `scale · d`.
    fn chain(&self, v: f64, scale: f64) -> Dual {
        Dual { v, d: self.d.iter().map(|x| scale * x).collect() }
    }

    /// `a·self.d + b·other.d`, entry by entry; an empty side is zero.
    fn combine(&self, a: f64, other: &Dual, b: f64) -> Vec<f64> {
        if self.d.is_empty() && other.d.is_empty() {
            return Vec::new();
        }
        let n = self.d.len().max(other.d.len());
        (0..n).map(|i| a * self.tangent(i) + b * other.tangent(i)).collect()
    }

    pub fn exp(&self) -> Dual {
        let e = self.v.exp();
        self.chain(e, e)
    }

    pub fn ln(&self) -> Dual {
        self.chain(self.v.ln(), 1.0 / self.v)
    }

    pub fn sqrt(&self) -> Dual {
        let s = self.v.sqrt();
        self.chain(s, 0.5 / s)
    }

    /// The derivative is `sign(v)`; at exactly zero it is zero.
    pub fn abs(&self) -> Dual {
        let sign = if self.v > 0.0 {
            1.0
        } else if self.v < 0.0 {
            -1.0
        } else {
            0.0
        };
        self.chain(self.v.abs(), sign)
    }
}

impl Add for Dual {
    type Output = Dual;
    fn add(self, rhs: Dual) -> Dual {
        Dual { v: self.v + rhs.v, d: self.combine(1.0, &rhs, 1.0) }
    }
}

impl Sub for Dual {
    type Output = Dual;
    fn sub(self, rhs: Dual) -> Dual {
        Dual { v: self.v - rhs.v, d: self.combine(1.0, &rhs, -1.0) }
    }
}

impl Mul for Dual {
    type Output = Dual;
    fn mul(self, rhs: Dual) -> Dual {
        Dual { v: self.v * rhs.v, d: self.combine(rhs.v, &rhs, self.v) }
    }
}

impl Div for Dual {
    type Output = Dual;
    fn div(self, rhs: Dual) -> Dual {
        let q = self.v / rhs.v;
        Dual { v: q, d: self.combine(1.0 / rhs.v, &rhs, -q / rhs.v) }
    }
}

impl Neg for Dual {
    type Output = Dual;
    fn neg(self) -> Dual {
        self.chain(-self.v, -1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Dual;

    const POINTS: [(f64, f64); 3] = [(0.5, 2.0), (1.7, -0.3), (3.0, 4.5)];

    /// The dual-number arithmetic oracle: tolerant comparison and seeded operand pairs.
    struct DualFixture;

    impl DualFixture {
        fn close(a: f64, b: f64) -> bool {
            (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1.0)
        }

        /// `x` seeded in slot 0 and `y` in slot 1 of two.
        fn pair(x: f64, y: f64) -> (Dual, Dual) {
            (Dual::seeded(x, 0, 2), Dual::seeded(y, 1, 2))
        }
    }

    #[test]
    fn add_and_sub_follow_the_sum_rule() {
        for (x, y) in POINTS {
            let (a, b) = DualFixture::pair(x, y);
            let s = a.clone() + b.clone();
            assert_eq!((s.v, s.d), (x + y, vec![1.0, 1.0]));
            let t = a - b;
            assert_eq!((t.v, t.d), (x - y, vec![1.0, -1.0]));
        }
    }

    #[test]
    fn mul_follows_the_product_rule() {
        for (x, y) in POINTS {
            let (a, b) = DualFixture::pair(x, y);
            let p = a * b;
            assert!(DualFixture::close(p.v, x * y));
            assert!(DualFixture::close(p.d[0], y) && DualFixture::close(p.d[1], x));
        }
    }

    #[test]
    fn div_follows_the_quotient_rule() {
        for (x, y) in POINTS {
            let (a, b) = DualFixture::pair(x, y);
            let q = a / b;
            assert!(DualFixture::close(q.v, x / y));
            assert!(DualFixture::close(q.d[0], 1.0 / y) && DualFixture::close(q.d[1], -x / (y * y)));
        }
    }

    #[test]
    fn neg_flips_value_and_tangent() {
        for (x, _) in POINTS {
            let n = -Dual::seeded(x, 0, 1);
            assert_eq!((n.v, n.d), (-x, vec![-1.0]));
        }
    }

    #[test]
    fn exp_ln_sqrt_have_their_analytic_derivatives() {
        for (x, _) in POINTS {
            let a = Dual::seeded(x, 0, 1);
            let e = a.exp();
            assert!(DualFixture::close(e.v, x.exp()) && DualFixture::close(e.d[0], x.exp()));
            let l = a.ln();
            assert!(DualFixture::close(l.v, x.ln()) && DualFixture::close(l.d[0], 1.0 / x));
            let s = a.sqrt();
            assert!(DualFixture::close(s.v, x.sqrt()) && DualFixture::close(s.d[0], 0.5 / x.sqrt()));
        }
    }

    #[test]
    fn the_chain_rule_composes() {
        // d/dx exp(x * x) = 2x exp(x^2)
        for (x, _) in POINTS {
            let a = Dual::seeded(x, 0, 1);
            let r = (a.clone() * a).exp();
            assert!(DualFixture::close(r.d[0], 2.0 * x * (x * x).exp()));
        }
    }

    #[test]
    fn abs_has_the_derivative_of_sign() {
        for (x, _) in POINTS {
            let pos = Dual::seeded(x, 0, 1).abs();
            assert_eq!((pos.v, pos.d), (x, vec![1.0]));
            let neg = Dual::seeded(-x, 0, 1).abs();
            assert_eq!((neg.v, neg.d), (x, vec![-1.0]));
        }
        assert_eq!(Dual::seeded(0.0, 0, 1).abs().d, vec![0.0]);
    }

    #[test]
    fn constants_allocate_no_tangent() {
        let (a, b) = (Dual::constant(2.0), Dual::constant(3.0));
        for r in [a.clone() + b.clone(), a.clone() - b.clone(), a.clone() * b.clone(), a.clone() / b, -a.clone(), a.exp()] {
            assert!(r.d.is_empty() && r.d.capacity() == 0);
        }
    }

    #[test]
    fn a_constant_among_seeded_keeps_the_tangent_of_the_seeded() {
        let x = Dual::seeded(3.0, 1, 2);
        let r = Dual::constant(2.0) * x;
        assert_eq!((r.v, r.d.clone()), (6.0, vec![0.0, 2.0]));
        assert_eq!(r.tangent(5), 0.0);
    }
}
