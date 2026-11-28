//! Field operations on `ExactRational`: negation, sums, products, quotients and the exact order.

use super::ExactRational;
use num_bigint::{BigInt, Sign};
use std::cmp::Ordering;

impl ExactRational {
    /// `self + other`.
    pub fn plus(&self, other: &Self) -> Self {
        let denominator = &self.denominator * &other.denominator;
        let numerator = &self.numerator * BigInt::from(other.denominator.clone()) + &other.numerator * BigInt::from(self.denominator.clone());
        ExactRational { numerator, denominator }
    }

    /// `-self`.
    pub fn negated(&self) -> Self {
        ExactRational { numerator: -self.numerator.clone(), denominator: self.denominator.clone() }
    }

    /// `self - other`.
    pub fn minus(&self, other: &Self) -> Self {
        self.plus(&other.negated())
    }

    /// `self * other`.
    pub fn times(&self, other: &Self) -> Self {
        ExactRational { numerator: &self.numerator * &other.numerator, denominator: &self.denominator * &other.denominator }
    }

    /// `self / other`, `None` for a zero divisor.
    pub fn divided_by(&self, other: &Self) -> Option<Self> {
        if other.numerator.sign() == Sign::NoSign {
            return None;
        }
        let sign = if other.is_negative() { BigInt::from(-1) } else { BigInt::from(1) };
        Some(ExactRational { numerator: &self.numerator * BigInt::from(other.denominator.clone()) * sign, denominator: self.denominator.clone() * other.magnitude() })
    }

    /// The exact order of two rationals.
    pub fn compare(&self, other: &Self) -> Ordering {
        (&self.numerator * BigInt::from(other.denominator.clone())).cmp(&(&other.numerator * BigInt::from(self.denominator.clone())))
    }
}
