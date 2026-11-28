//! `Float16` against exact rational arithmetic (`ExactOracle` over `num-bigint`): every unary
//! operation over all 65 536 patterns and the binary operations over the special values and
//! 2^22 deterministic pairs (CV-30; IEEE 754-2019 clause 5, H.2).

use crate::oracle;
use crate::random;

use oracle::ExactOracle;
use piperine_numbers::format::BinaryFormat;
use piperine_numbers::half::Float16;
use random::Xorshift;
use std::cmp::Ordering;

/// The scenario of one test: the oracle and the comparison of patterns.
struct HalfScenario {
    /// The reference operations.
    oracle: ExactOracle,
}

impl HalfScenario {
    /// A scenario over binary16.
    fn new() -> Self {
        HalfScenario { oracle: ExactOracle::new(BinaryFormat::HALF) }
    }

    /// Asserts two results are the same value: equal bits, or both NaN.
    fn same(&self, expected: u128, actual: Float16, context: &str) {
        let both_nan = self.oracle.is_nan(expected) && actual.is_nan();
        assert!(both_nan || expected == actual.to_bits() as u128, "{context}: expected {expected:#06x}, got {:#06x}", actual.to_bits());
    }

    /// Patterns of the special values: zeros, subnormal and normal extremes, ties, infinities, NaN.
    fn specials() -> Vec<u16> {
        let mut values = vec![0x0000, 0x0001, 0x0002, 0x03FF, 0x0400, 0x0401, 0x3400, 0x3800, 0x3C00, 0x3C01, 0x3E00, 0x4000, 0x4200, 0x4400, 0x5000, 0x6400, 0x6800, 0x6BFF, 0x7BFF, 0x7BFE, 0x7C00, 0x7E00, 0x7C01, 0x5BFF, 0x5C00, 0x0155, 0x3555, 0x2E66];
        let negatives: Vec<u16> = values.iter().map(|bits| bits | 0x8000).collect();
        values.extend(negatives);
        values
    }

    /// Checks every unary operation on one pattern.
    fn check_unary(&self, bits: u16) {
        let (pattern, value) = (bits as u128, Float16::from_bits(bits));
        let note = |name: &str| format!("{name}({bits:#06x})");
        self.same(self.oracle.square_root(pattern), value.sqrt(), &note("sqrt"));
        self.same(pattern ^ 0x8000, -value, &note("neg"));
        self.same(pattern & 0x7FFF, value.abs(), &note("abs"));
        self.same(self.oracle.floor(pattern), value.floor(), &note("floor"));
        self.same(self.oracle.ceiling(pattern), value.ceil(), &note("ceil"));
        self.same(self.oracle.round_half_away(pattern), value.round(), &note("round"));
        self.same(self.oracle.truncate(pattern), value.trunc(), &note("trunc"));
        self.same(self.oracle.fraction(pattern), value.fract(), &note("fract"));
        self.same(self.oracle.signum(pattern), value.signum(), &note("signum"));
        let infinite = self.oracle.is_infinite(pattern);
        let nan = self.oracle.is_nan(pattern);
        assert_eq!((value.is_nan(), value.is_infinite(), value.is_finite()), (nan, infinite, !nan && !infinite), "predicates {bits:#06x}");
        let exact = self.oracle.value(pattern);
        if let Some(exact) = exact.filter(|value| !value.is_zero()) {
            assert_eq!(Float16::from_f32(value.to_f32()).to_bits(), bits, "f32 round trip {bits:#06x}");
            assert_eq!(Float16::from_f64(value.to_f64()).to_bits(), bits, "f64 round trip {bits:#06x}");
            assert_eq!(exact.nearest(BinaryFormat::SINGLE), value.to_f32().to_bits() as u128, "widening is exact {bits:#06x}");
        }
    }

    /// Checks every binary operation on one pair.
    fn check_binary(&self, left: u16, right: u16, full: bool) {
        let (a, b) = (left as u128, right as u128);
        let (x, y) = (Float16::from_bits(left), Float16::from_bits(right));
        let note = |name: &str| format!("{name}({left:#06x}, {right:#06x})");
        self.same(self.oracle.add(a, b), x + y, &note("add"));
        self.same(self.oracle.subtract(a, b), x - y, &note("sub"));
        self.same(self.oracle.multiply(a, b), x * y, &note("mul"));
        self.same(self.oracle.divide(a, b), x / y, &note("div"));
        self.same(self.oracle.minimum(a, b), x.min(y), &note("min"));
        self.same(self.oracle.maximum(a, b), x.max(y), &note("max"));
        let order = self.oracle.compare(a, b);
        assert_eq!((x.partial_cmp(&y), x == y, x < y, x > y), (order, order == Some(Ordering::Equal), order == Some(Ordering::Less), order == Some(Ordering::Greater)), "{}", note("compare"));
        if full {
            self.same(self.oracle.remainder_euclid(a, b), x.rem_euclid(y), &note("rem_euclid"));
            self.same(self.oracle.hypot(a, b), x.hypot(y), &note("hypot"));
        }
    }
}

#[test]
fn every_unary_f16_operation_is_correctly_rounded_over_all_inputs() {
    let scenario = HalfScenario::new();
    for bits in 0..=u16::MAX {
        scenario.check_unary(bits);
    }
}

#[test]
fn binary_f16_operations_are_correctly_rounded() {
    let scenario = HalfScenario::new();
    let specials = HalfScenario::specials();
    for &left in &specials {
        for &right in &specials {
            scenario.check_binary(left, right, true);
            let low = Float16::from_bits(left.min(right));
            let high = Float16::from_bits(left.max(right));
            for &candidate in &specials {
                let expected = scenario.oracle.clamp(candidate as u128, left.min(right) as u128, left.max(right) as u128);
                let actual = Float16::from_bits(candidate).clamp_checked(low, high);
                assert_eq!(expected.is_some(), actual.is_some(), "clamp {candidate:#06x} {left:#06x} {right:#06x}");
                if let (Some(expected), Some(actual)) = (expected, actual) {
                    scenario.same(expected, actual, "clamp");
                }
            }
        }
    }
    let mut random = Xorshift::new(95);
    for index in 0..(1u32 << 22) {
        let word = random.next_u64();
        // `rem_euclid` and `hypot` cost two exact roots or divisions: one pair in eight.
        scenario.check_binary(word as u16, (word >> 32) as u16, index % 8 == 0);
    }
}
