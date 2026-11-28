//! `Float128` against exact rational arithmetic (`ExactOracle` over `num-bigint`): `+ - * /` and
//! `sqrt` rounded once, and every exact H.2 method, on special values, subnormals, ties,
//! overflow, cancellation and deterministic random operands (CV-31; IEEE 754-2019 clause 5, H.2).


use piperine_device_fixtures::oracle::ExactOracle;
use piperine_numbers::format::BinaryFormat;
use piperine_numbers::quad::Float128;
use piperine_device_fixtures::random::ShiftGenerator;
use std::cmp::Ordering;

/// Operand generation and comparison for binary128 tests.
struct QuadScenario {
    /// The reference operations.
    oracle: ExactOracle,
    /// The deterministic operand source.
    random: ShiftGenerator,
}

impl QuadScenario {
    /// A scenario with a seeded generator.
    fn new(seed: u64) -> Self {
        QuadScenario { oracle: ExactOracle::new(BinaryFormat::QUAD), random: ShiftGenerator::new(seed) }
    }

    /// 128 random bits.
    fn word(&mut self) -> u128 {
        (self.random.next_bits() as u128) << 64 | self.random.next_bits() as u128
    }

    /// Asserts two results are the same value: equal bits, or both NaN.
    fn same(&self, expected: u128, actual: Float128, context: &str) {
        let both_nan = self.oracle.is_nan(expected) && actual.is_nan();
        assert!(both_nan || expected == actual.to_bits(), "{context}: expected {expected:#034x}, got {:#034x}", actual.to_bits());
    }

    /// Patterns of the special values: zeros, extremes, ties, infinities, NaN, both signs.
    fn specials() -> Vec<u128> {
        let mut values: Vec<u128> = vec![0, 1, 2, (1u128 << 112) - 1, 1u128 << 112, (1u128 << 112) + 1, 0x3FFF_0000_0000_0000_0000_0000_0000_0000, 0x3FFE_0000_0000_0000_0000_0000_0000_0000, 0x4000_0000_0000_0000_0000_0000_0000_0000, 0x3FFF_0000_0000_0000_0000_0000_0000_0001, 0x7FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF, 0x7FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFE, 0x7FFF_0000_0000_0000_0000_0000_0000_0000, 0x7FFF_8000_0000_0000_0000_0000_0000_0000, 0x7FFF_0000_0000_0000_0000_0000_0000_0001, 0x4000_8000_0000_0000_0000_0000_0000_0000, 0x3FFF_8000_0000_0000_0000_0000_0000_0000, 0x406F_0000_0000_0000_0000_0000_0000_0000];
        let negatives: Vec<u128> = values.iter().map(|bits| bits | 1u128 << 127).collect();
        values.extend(negatives);
        values
    }

    /// A random pattern of a random kind: any bits, a nearby exponent, sparse bits (ties),
    /// subnormal, near overflow, or a short significand.
    fn pattern(&mut self, anchor: u128) -> u128 {
        let sign = (self.word() & 1) << 127;
        let fraction = self.word() & ((1u128 << 112) - 1);
        let anchor_field = ((anchor >> 112) & 0x7FFF) as u64;
        let field = match self.random.next_bits() % 8 {
            0 => (self.word() as u128) % 0x7FFF,
            1 | 2 => ((anchor_field + (self.random.next_bits() % 5)).saturating_sub(2).min(0x7FFE)) as u128,
            3 => ((anchor_field + (self.random.next_bits() % 240)).saturating_sub(120).min(0x7FFE)) as u128,
            4 => (self.random.next_bits() % 4) as u128,
            5 => 0x7FFE - (self.random.next_bits() % 4) as u128,
            _ => ((anchor_field + (self.random.next_bits() % 120)).saturating_sub(60).min(0x7FFE)) as u128,
        };
        let kept = match self.random.next_bits() % 4 {
            0 => fraction & !((1u128 << (self.random.next_bits() % 112)) - 1),
            1 => fraction >> (self.random.next_bits() % 112) << (self.random.next_bits() % 4),
            _ => fraction,
        } & ((1u128 << 112) - 1);
        sign | field << 112 | kept
    }

    /// Checks `+ - * /` on one pair.
    fn check_arithmetic(&self, left: u128, right: u128) {
        let (x, y) = (Float128::from_bits(left), Float128::from_bits(right));
        let note = |name: &str| format!("{name}({left:#034x}, {right:#034x})");
        self.same(self.oracle.add(left, right), x + y, &note("add"));
        self.same(self.oracle.subtract(left, right), x - y, &note("sub"));
        self.same(self.oracle.multiply(left, right), x * y, &note("mul"));
        self.same(self.oracle.divide(left, right), x / y, &note("div"));
    }

    /// Checks `sqrt` on one pattern.
    fn check_square_root(&self, bits: u128) {
        self.same(self.oracle.square_root(bits), Float128::from_bits(bits).sqrt(), &format!("sqrt({bits:#034x})"));
    }

    /// Checks the unary exact methods on one pattern.
    fn check_unary(&self, bits: u128) {
        let value = Float128::from_bits(bits);
        let note = |name: &str| format!("{name}({bits:#034x})");
        self.same(self.oracle.floor(bits), value.floor(), &note("floor"));
        self.same(self.oracle.ceiling(bits), value.ceil(), &note("ceil"));
        self.same(self.oracle.round_half_away(bits), value.round(), &note("round"));
        self.same(self.oracle.truncate(bits), value.trunc(), &note("trunc"));
        self.same(self.oracle.fraction(bits), value.fract(), &note("fract"));
        self.same(self.oracle.signum(bits), value.signum(), &note("signum"));
        self.same(bits & !(1u128 << 127), value.abs(), &note("abs"));
        self.same(bits ^ 1u128 << 127, -value, &note("neg"));
        let infinite = self.oracle.is_infinite(bits);
        let nan = self.oracle.is_nan(bits);
        assert_eq!((value.is_nan(), value.is_infinite(), value.is_finite()), (nan, infinite, !nan && !infinite), "{}", note("predicates"));
    }

    /// Checks the binary exact methods on one pair.
    fn check_binary(&self, left: u128, right: u128, with_division: bool) {
        let (x, y) = (Float128::from_bits(left), Float128::from_bits(right));
        let note = |name: &str| format!("{name}({left:#034x}, {right:#034x})");
        self.same(self.oracle.minimum(left, right), x.min(y), &note("min"));
        self.same(self.oracle.maximum(left, right), x.max(y), &note("max"));
        let order = self.oracle.compare(left, right);
        assert_eq!((x.partial_cmp(&y), x == y, x < y, x > y), (order, order == Some(Ordering::Equal), order == Some(Ordering::Less), order == Some(Ordering::Greater)), "{}", note("compare"));
        let (low, high) = if self.oracle.compare(left, right) == Some(Ordering::Greater) { (right, left) } else { (left, right) };
        let clamped = x.clamp_checked(Float128::from_bits(low), Float128::from_bits(high));
        let expected = self.oracle.clamp(left, low, high);
        assert_eq!(expected.is_some(), clamped.is_some(), "{}", note("clamp"));
        if let (Some(expected), Some(actual)) = (expected, clamped) {
            self.same(expected, actual, &note("clamp"));
        }
        if with_division {
            self.same(self.oracle.remainder_euclid(left, right), x.rem_euclid(y), &note("rem_euclid"));
            self.same(self.oracle.hypot(left, right), x.hypot(y), &note("hypot"));
        }
    }
}

#[test]
fn f128_arithmetic_and_square_root_are_correctly_rounded() {
    let mut scenario = QuadScenario::new(96);
    let specials = QuadScenario::specials();
    for &left in &specials {
        for &right in &specials {
            scenario.check_arithmetic(left, right);
        }
        scenario.check_square_root(left);
    }
    for _ in 0..50_000 {
        let left = scenario.pattern(0x3FFF << 112);
        let anchor = if scenario.random.next_bits() & 1 == 0 { left } else { 0x3FFF << 112 };
        let right = scenario.pattern(anchor);
        scenario.check_arithmetic(left, right);
        scenario.check_square_root(left & !(1u128 << 127));
        let root = scenario.pattern(0x3FFF << 112) & !(1u128 << 127) & !((1u128 << 56) - 1);
        let square = scenario.oracle.multiply(root, root);
        scenario.check_square_root(square);
    }
}

#[test]
fn f128_exact_methods_follow_h2() {
    let mut scenario = QuadScenario::new(961);
    let specials = QuadScenario::specials();
    for &left in &specials {
        scenario.check_unary(left);
        for &right in &specials {
            scenario.check_binary(left, right, true);
        }
    }
    let largest = 0x7FFE_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFFu128;
    scenario.check_binary(largest, largest, true);
    scenario.check_binary(largest, 1, true);
    scenario.check_binary(largest | 1u128 << 127, largest, true);
    for _ in 0..20_000 {
        let left = scenario.pattern(0x3FFF << 112);
        scenario.check_unary(left);
    }
    for index in 0..4_000 {
        let left = scenario.pattern(0x3FFF << 112);
        let right = scenario.pattern(left);
        scenario.check_binary(left, right, index % 4 == 0);
    }
}
