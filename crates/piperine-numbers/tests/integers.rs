//! `Limbs` and the wide integers against exact oracles: `num-bigint` for any width, `u128` and
//! `i128` wrapping arithmetic for widths up to 128 bits (CV-32; design-d.md "Tech Decisions").

mod support;

use num_bigint::BigUint;
use piperine_numbers::limbs::{Limbs, ToLimbs};
use support::random::Xorshift;

/// The big-integer oracle: conversions between limbs and `BigUint`, and random limb vectors.
struct BigOracle;

impl BigOracle {
    /// The `BigUint` of a little-endian limb slice.
    fn value(limbs: &[u64]) -> BigUint {
        let mut value = BigUint::from(0u32);
        for limb in limbs.iter().rev() {
            value = (value << 64u32) | BigUint::from(*limb);
        }
        value
    }

    /// The normalized limbs of a `BigUint`.
    fn limbs(value: &BigUint) -> Vec<u64> {
        value.to_u64_digits()
    }

    /// A random limb vector of 1 to `maximum` limbs, biased to edge limbs (0, 1, `u64::MAX`).
    fn random_limbs(random: &mut Xorshift, maximum: u64) -> Vec<u64> {
        let length = 1 + random.next_u64() % maximum;
        (0..length)
            .map(|_| match random.next_u64() % 8 {
                0 => 0,
                1 => 1,
                2 => u64::MAX,
                3 => 1 << 63,
                _ => random.next_u64(),
            })
            .collect()
    }
}

#[test]
fn limbs_agree_with_big_integers_on_every_operation() {
    let mut random = Xorshift::new(92);
    let edge: Vec<Vec<u64>> = vec![vec![], vec![1], vec![u64::MAX], vec![0, 1], vec![u64::MAX, u64::MAX], vec![1 << 63], vec![0, 1 << 63], vec![1, 1 << 63, u64::MAX]];
    let mut pairs: Vec<(Vec<u64>, Vec<u64>)> = Vec::new();
    for left in &edge {
        for right in &edge {
            pairs.push((left.clone(), right.clone()));
        }
    }
    for _ in 0..10_000 {
        pairs.push((BigOracle::random_limbs(&mut random, 40), BigOracle::random_limbs(&mut random, 40)));
    }
    for (left, right) in pairs {
        let big_left = BigOracle::value(&left);
        let big_right = BigOracle::value(&right);
        assert_eq!(BigOracle::value(&left.plus(&right)), &big_left + &big_right, "add {left:?} {right:?}");
        assert_eq!(left.minus(&right).map(|value| BigOracle::value(&value)), (big_left >= big_right).then(|| &big_left - &big_right), "sub {left:?} {right:?}");
        assert_eq!(BigOracle::value(&left.times(&right)), &big_left * &big_right, "mul {left:?} {right:?}");
        assert_eq!(left.compare_magnitude(&right), big_left.cmp(&big_right), "cmp {left:?} {right:?}");
        match left.divide_with_remainder(&right) {
            None => assert!(big_right == BigUint::from(0u32)),
            Some((quotient, remainder)) => {
                assert_eq!(BigOracle::value(&quotient), &big_left / &big_right, "div {left:?} {right:?}");
                assert_eq!(BigOracle::value(&remainder), &big_left % &big_right, "rem {left:?} {right:?}");
                assert_eq!(quotient, quotient.trimmed());
            }
        }
        let count = random.next_u64() % 300;
        assert_eq!(BigOracle::value(&left.shifted_left(count)), &big_left << count, "shl {left:?} {count}");
        assert_eq!(BigOracle::value(&left.shifted_right(count)), &big_left >> count, "shr {left:?} {count}");
        assert_eq!(left.bit_length(), big_left.bits(), "bits {left:?}");
        assert_eq!(left.to_decimal_string(), big_left.to_string(), "decimal {left:?}");
        let (root, exact) = left.integer_square_root();
        assert_eq!(BigOracle::value(&root), big_left.sqrt(), "sqrt {left:?}");
        assert_eq!(exact, big_left.sqrt().pow(2) == big_left);
        if big_left != BigUint::from(0u32) {
            assert_eq!(left.trailing_zero_bits(), big_left.trailing_zeros().unwrap_or(0));
        }
        let index = random.next_u64() % 200;
        assert_eq!(left.bit(index), (&big_left >> index) % 2u32 == BigUint::from(1u32), "bit {left:?} {index}");
        assert_eq!(left.any_bit_below(index), (&big_left % (BigUint::from(1u32) << index)) != BigUint::from(0u32), "below {left:?} {index}");
        let small = random.next_u64();
        let addend = random.next_u64();
        assert_eq!(BigOracle::value(&left.times_small_plus(small, addend)), &big_left * small + addend);
        if small != 0 {
            let (quotient, remainder) = left.divide_small(small).unwrap_or_default();
            assert_eq!(BigOracle::value(&quotient), &big_left / small);
            assert_eq!(BigUint::from(remainder), &big_left % small);
        }
        let mut fixed = left.clone();
        fixed.resize(left.len().max(1), 0);
        let width = fixed.len() as u64 * 64;
        let modulus = BigUint::from(1u32) << width;
        let mut shifted = fixed.clone();
        shifted.shift_left_assign(count);
        assert_eq!(BigOracle::value(&shifted), (&big_left << count) % &modulus, "fixed shl");
        let mut shifted = fixed.clone();
        shifted.shift_right_assign(count, true);
        let expected = ((&big_left >> count) | (&modulus - (BigUint::from(1u32) << (width.saturating_sub(count))))) % &modulus;
        let expected = if count >= width { &modulus - 1u32 } else { expected };
        assert_eq!(BigOracle::value(&shifted), expected, "fixed shr fill {left:?} {count}");
        assert_eq!(BigOracle::value(&left.to_u128().map(|value| value.to_limbs()).unwrap_or_default()), if big_left.bits() <= 128 { big_left.clone() } else { BigUint::from(0u32) });
        assert_eq!(BigOracle::limbs(&big_left), left.trimmed());
    }
}
