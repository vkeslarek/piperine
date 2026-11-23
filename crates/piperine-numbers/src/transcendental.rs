//! The correctly rounded transcendental methods of `Float128` (H.2): each evaluates an interval
//! that contains the exact value and lets [`ZivRounding`] raise the precision until both bounds
//! round to one binary128 pattern, so the result is the correctly rounded one. Special values
//! follow IEEE 754-2019 section 9.2 and H.2 rule 7 (a result too large is an infinity, a result
//! too small underflows to a subnormal or zero).

use super::constants::ReferenceConstants;
use super::elementary::ElementaryFunctions;
use super::limbs::{Limbs, ToLimbs};
use super::power::ExactPower;
use super::precision::{Interval, MultiPrecision};
use super::quad::Float128;
use super::reduction::ArgumentReduction;
use super::ziv::ZivRounding;

/// Shared plumbing of the transcendental methods.
struct Evaluation;

impl Evaluation {
    /// Bits kept beyond the Ziv precision so each interval is far narrower than it.
    const GUARD_BITS: u32 = 48;

    /// The exact point `x` as an interval carried at `precision` plus the guard bits.
    fn point(x: Float128, precision: u32) -> Option<Interval> {
        Some(Interval::point(MultiPrecision::from_quad_bits(x.to_bits())?, precision + Self::GUARD_BITS))
    }

    /// Runs the Ziv loop; a failure of the loop (never reached) is NaN.
    fn rounded<F>(exact: Option<Float128>, evaluation: F) -> Float128
    where
        F: FnMut(u32) -> Option<Interval>,
    {
        ZivRounding::round(exact, evaluation).unwrap_or(Float128::NAN)
    }

    /// The integer `value` as a binary128.
    fn integer(negative: bool, value: u64) -> Float128 {
        Float128::from_bits(super::format::BinaryFormat::QUAD.round(negative, &value.to_limbs(), 0, false))
    }

    /// `(sin, cos)` of the remainder of `x` modulo pi/2 and the quadrant, as intervals at `precision` plus the guard bits.
    fn circle(x: Float128, precision: u32) -> Option<(u8, Interval, Interval)> {
        let reduced = ArgumentReduction::quarter_turns(x, precision + Self::GUARD_BITS)?;
        let (sine, cosine) = ElementaryFunctions::sin_cos(&reduced.remainder)?;
        Some((reduced.quadrant, sine, cosine))
    }

    /// `pi / 2` as an interval carried at `precision` plus the guard bits.
    fn half_pi(precision: u32) -> Interval {
        ReferenceConstants::pi(precision + Self::GUARD_BITS).scaled(-1).with_precision(precision + Self::GUARD_BITS)
    }

    /// `pi` as an interval carried at `precision` plus the guard bits.
    fn pi(precision: u32) -> Interval {
        ReferenceConstants::pi(precision + Self::GUARD_BITS).with_precision(precision + Self::GUARD_BITS)
    }

    /// `log_base(x)` as `ln x / ln base` for a finite positive `x`, with the exact case supplied.
    fn logarithm(x: Float128, exact: Option<Float128>, divisor: fn(u32) -> Interval) -> Float128 {
        Self::rounded(exact, |precision| {
            let argument = Self::point(x, precision)?;
            ElementaryFunctions::ln(&argument)?.divide(&divisor(precision + Self::GUARD_BITS))
        })
    }

    /// `Some(k)` when `x` is `2^k` exactly.
    fn power_of_two(x: Float128) -> Option<i64> {
        let (negative, significand, exponent) = super::format::BinaryFormat::QUAD.decode(x.to_bits())?;
        (!negative && significand != 0 && significand.is_power_of_two()).then(|| exponent + i64::from(significand.trailing_zeros()))
    }

    /// `Some(k)` when `x` is `10^k` exactly (`k` from 0 to 48: `10^k = 2^k 5^k` and `5^k` fits 113 bits).
    fn power_of_ten(x: Float128) -> Option<u64> {
        let (negative, significand, exponent) = super::format::BinaryFormat::QUAD.decode(x.to_bits())?;
        if negative || significand == 0 || exponent + i64::from(significand.trailing_zeros()) < 0 {
            return None;
        }
        let integer = if exponent >= 0 { significand.to_limbs().shifted_left(exponent as u64) } else { significand.to_limbs().shifted_right((-exponent) as u64) };
        let mut power = 1u64.to_limbs();
        for k in 0..=48u64 {
            if power.compare_magnitude(&integer) == core::cmp::Ordering::Equal {
                return Some(k);
            }
            power = power.times_small_plus(10, 0);
        }
        None
    }
}

impl Float128 {
    /// Whether the value is an integer, and whether that integer is odd.
    fn integer_parity(self) -> Option<bool> {
        let (_, significand, exponent) = super::format::BinaryFormat::QUAD.decode(self.to_bits())?;
        if significand == 0 {
            return Some(false);
        }
        let low = i64::from(significand.trailing_zeros());
        if exponent + low < 0 {
            return None;
        }
        Some(exponent + low == 0)
    }

    /// `e^x`, correctly rounded.
    pub fn exp(self) -> Float128 {
        if self.is_nan() {
            return Float128::NAN;
        }
        if self.is_infinite() {
            return if self.is_sign_negative() { Float128::ZERO } else { Float128::INFINITY };
        }
        if self.is_zero() {
            return Float128::ONE;
        }
        let eleven_thousand = Evaluation::integer(false, 11_357);
        if self >= eleven_thousand {
            return Float128::INFINITY;
        }
        if self <= -Evaluation::integer(false, 11_434) {
            return Float128::ZERO;
        }
        Evaluation::rounded(None, |precision| ElementaryFunctions::exp(&Evaluation::point(self, precision)?))
    }

    /// The natural logarithm, correctly rounded (NaN below zero, `-inf` at zero).
    pub fn ln(self) -> Float128 {
        match self.logarithm_domain() {
            Some(result) => result,
            None if self == Float128::ONE => Float128::ZERO,
            None => Evaluation::rounded(None, |precision| ElementaryFunctions::ln(&Evaluation::point(self, precision)?)),
        }
    }

    /// The special values of every logarithm: NaN, a negative, zero and infinity.
    fn logarithm_domain(self) -> Option<Float128> {
        if self.is_nan() || (self.is_sign_negative() && !self.is_zero()) {
            return Some(Float128::NAN);
        }
        if self.is_zero() {
            return Some(Float128::NEGATIVE_INFINITY);
        }
        self.is_infinite().then_some(Float128::INFINITY)
    }

    /// The base-2 logarithm, correctly rounded.
    pub fn log2(self) -> Float128 {
        if let Some(result) = self.logarithm_domain() {
            return result;
        }
        let exact = Evaluation::power_of_two(self).map(|k| Evaluation::integer(k < 0, k.unsigned_abs()));
        Evaluation::logarithm(self, exact, ReferenceConstants::ln_two)
    }

    /// The base-10 logarithm, correctly rounded.
    pub fn log10(self) -> Float128 {
        if let Some(result) = self.logarithm_domain() {
            return result;
        }
        let exact = Evaluation::power_of_ten(self).map(|k| Evaluation::integer(false, k));
        Evaluation::logarithm(self, exact, ReferenceConstants::ln_ten)
    }
}

impl Float128 {
    /// `self` raised to `exponent`, correctly rounded, with the special cases of C's `pow` (and
    /// Rust's `powf`): `x^0 = 1` and `1^y = 1` even for NaN, a negative base with a non-integer
    /// exponent is NaN (H.2 rule 7), signed zeros and infinities by IEEE 754 section 9.2.1.
    pub fn powf(self, exponent: Float128) -> Float128 {
        if exponent.is_zero() || self == Float128::ONE {
            return Float128::ONE;
        }
        if self.is_nan() || exponent.is_nan() {
            return Float128::NAN;
        }
        let parity = exponent.integer_parity();
        let odd = parity == Some(true);
        let negative_result = self.is_sign_negative() && odd;
        if self.is_zero() || self.is_infinite() || exponent.is_infinite() {
            return self.power_of_extreme(exponent, odd, negative_result);
        }
        if self.is_sign_negative() && parity.is_none() {
            return Float128::NAN;
        }
        let magnitude = self.abs();
        let exact = ExactPower::of(magnitude, exponent);
        let result = Evaluation::rounded(exact, |precision| {
            let logarithm = ElementaryFunctions::ln(&Evaluation::point(magnitude, precision)?)?;
            ElementaryFunctions::exp(&logarithm.multiply(&Evaluation::point(exponent, precision)?))
        });
        if negative_result { -result } else { result }
    }

    /// `self^exponent` when the base is a zero or an infinity or the exponent is infinite.
    fn power_of_extreme(self, exponent: Float128, odd: bool, negative_result: bool) -> Float128 {
        let exponent_negative = exponent.is_sign_negative();
        if self.is_zero() {
            let magnitude = if exponent_negative { Float128::INFINITY } else { Float128::ZERO };
            return if negative_result { -magnitude } else { magnitude };
        }
        if self.is_infinite() {
            let magnitude = if exponent_negative { Float128::ZERO } else { Float128::INFINITY };
            return if self.is_sign_negative() && odd { -magnitude } else { magnitude };
        }
        let size = self.abs();
        if size == Float128::ONE {
            return Float128::ONE;
        }
        let grows = (size > Float128::ONE) != exponent_negative;
        if grows { Float128::INFINITY } else { Float128::ZERO }
    }

    /// `self` raised to an `i32`, correctly rounded; the same values as `powf` of that integer.
    pub fn powi(self, exponent: i32) -> Float128 {
        self.powf(Evaluation::integer(exponent < 0, u64::from(exponent.unsigned_abs())))
    }
}

impl Float128 {
    /// The sine, correctly rounded (NaN for an infinity).
    pub fn sin(self) -> Float128 {
        if self.is_nan() || self.is_infinite() {
            return Float128::NAN;
        }
        if self.is_zero() {
            return self;
        }
        Evaluation::rounded(None, |precision| {
            let (quadrant, sine, cosine) = Evaluation::circle(self, precision)?;
            Some(match quadrant {
                0 => sine,
                1 => cosine,
                2 => sine.negated(),
                _ => cosine.negated(),
            })
        })
    }

    /// The cosine, correctly rounded (NaN for an infinity).
    pub fn cos(self) -> Float128 {
        if self.is_nan() || self.is_infinite() {
            return Float128::NAN;
        }
        if self.is_zero() {
            return Float128::ONE;
        }
        Evaluation::rounded(None, |precision| {
            let (quadrant, sine, cosine) = Evaluation::circle(self, precision)?;
            Some(match quadrant {
                0 => cosine,
                1 => sine.negated(),
                2 => cosine.negated(),
                _ => sine,
            })
        })
    }

    /// The tangent, correctly rounded (NaN for an infinity).
    pub fn tan(self) -> Float128 {
        if self.is_nan() || self.is_infinite() {
            return Float128::NAN;
        }
        if self.is_zero() {
            return self;
        }
        Evaluation::rounded(None, |precision| {
            let (quadrant, sine, cosine) = Evaluation::circle(self, precision)?;
            if quadrant % 2 == 0 { sine.divide(&cosine) } else { Some(cosine.divide(&sine)?.negated()) }
        })
    }
}

impl Float128 {
    /// The arcsine, correctly rounded (NaN outside `[-1, 1]`): `2 atan(x / (1 + sqrt(1 - x^2)))`.
    pub fn asin(self) -> Float128 {
        if self.is_nan() || self.abs() > Float128::ONE {
            return Float128::NAN;
        }
        if self.is_zero() {
            return self;
        }
        let negative = self.is_sign_negative();
        let result = Evaluation::rounded(None, |precision| {
            if self.abs() == Float128::ONE {
                return Some(Evaluation::half_pi(precision));
            }
            let x = Evaluation::point(self.abs(), precision)?;
            let one = Interval::integer(false, 1, precision + Evaluation::GUARD_BITS);
            // 1 - x^2 as (1 - x)(1 + x): both factors are exact for a binary128 x.
            let root = one.subtract(&x).multiply(&one.add(&x)).square_root()?;
            Some(ElementaryFunctions::atan(&x.divide(&one.add(&root))?)?.scaled(1))
        });
        if negative { -result } else { result }
    }

    /// The arccosine, correctly rounded (NaN outside `[-1, 1]`): `2 atan(sqrt((1 - x) / (1 + x)))`.
    pub fn acos(self) -> Float128 {
        if self.is_nan() || self.abs() > Float128::ONE {
            return Float128::NAN;
        }
        if self == Float128::ONE {
            return Float128::ZERO;
        }
        Evaluation::rounded(None, |precision| {
            if self == -Float128::ONE {
                return Some(Evaluation::pi(precision));
            }
            let x = Evaluation::point(self, precision)?;
            let one = Interval::integer(false, 1, precision + Evaluation::GUARD_BITS);
            let ratio = one.subtract(&x).divide(&one.add(&x))?;
            Some(ElementaryFunctions::atan(&ratio.square_root()?)?.scaled(1))
        })
    }

    /// The arctangent, correctly rounded.
    pub fn atan(self) -> Float128 {
        if self.is_nan() {
            return Float128::NAN;
        }
        if self.is_zero() {
            return self;
        }
        let negative = self.is_sign_negative();
        let result = Evaluation::rounded(None, |precision| {
            if self.is_infinite() {
                return Some(Evaluation::half_pi(precision));
            }
            ElementaryFunctions::atan(&Evaluation::point(self.abs(), precision)?)
        });
        if negative { -result } else { result }
    }

    /// `self.atan2(x)`: the angle of the point `(x, self)` from `-pi` to `pi` (H.2 rule 8), with the
    /// signed zeros and infinities of IEEE 754 section 9.2.1.
    pub fn atan2(self, x: Float128) -> Float128 {
        if self.is_nan() || x.is_nan() {
            return Float128::NAN;
        }
        let negative = self.is_sign_negative();
        let signed = |magnitude: Float128| if negative { -magnitude } else { magnitude };
        let left = x.is_sign_negative();
        if self.is_zero() {
            return if left { signed(Evaluation::rounded(None, |precision| Some(Evaluation::pi(precision)))) } else { self };
        }
        if self.is_infinite() {
            let angle = Evaluation::rounded(None, |precision| {
                let pi = Evaluation::pi(precision);
                Some(match (x.is_infinite(), left) {
                    (false, _) => pi.scaled(-1),
                    (true, false) => pi.scaled(-2),
                    (true, true) => pi.scaled(-2).multiply(&Interval::integer(false, 3, pi.precision())),
                })
            });
            return signed(angle);
        }
        if x.is_infinite() {
            return if left { signed(Evaluation::rounded(None, |precision| Some(Evaluation::pi(precision)))) } else { signed(Float128::ZERO) };
        }
        if x.is_zero() {
            return signed(Evaluation::rounded(None, |precision| Some(Evaluation::half_pi(precision))));
        }
        let result = Evaluation::rounded(None, |precision| {
            let quotient = Evaluation::point(self.abs(), precision)?.divide(&Evaluation::point(x.abs(), precision)?)?;
            let angle = ElementaryFunctions::atan(&quotient)?;
            Some(if left { Evaluation::pi(precision).subtract(&angle) } else { angle })
        });
        signed(result)
    }
}
