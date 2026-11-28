//! `Limbs` and the wide integers against exact oracles: `num-bigint` for any width, `u128` and
//! `i128` wrapping arithmetic for widths up to 128 bits (CV-32; design-d.md "Tech Decisions").


use num_bigint::BigUint;
use piperine_numbers::limbs::{Limbs, ToLimbs};
use piperine_device_fixtures::random::ShiftGenerator;

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
    fn random_limbs(random: &mut ShiftGenerator, maximum: u64) -> Vec<u64> {
        let length = 1 + random.next_bits() % maximum;
        (0..length)
            .map(|_| match random.next_bits() % 8 {
                0 => 0,
                1 => 1,
                2 => u64::MAX,
                3 => 1 << 63,
                _ => random.next_bits(),
            })
            .collect()
    }
}

#[test]
fn limbs_agree_with_big_integers_on_every_operation() {
    let mut random = ShiftGenerator::new(92);
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
        let count = random.next_bits() % 300;
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
        let index = random.next_bits() % 200;
        assert_eq!(left.bit(index), (&big_left >> index) % 2u32 == BigUint::from(1u32), "bit {left:?} {index}");
        assert_eq!(left.any_bit_below(index), (&big_left % (BigUint::from(1u32) << index)) != BigUint::from(0u32), "below {left:?} {index}");
        let small = random.next_bits();
        let addend = random.next_bits();
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

use num_bigint::BigInt;
use piperine_numbers::wide::{WideInt, WideUInt};

/// The `u128`/`i128` oracle for an integer type of `N` bits (`N <= 128`): wrapping arithmetic
/// masked to `N` bits, as section 1 rule 6 states it.
struct NativeReference<const N: u32, const L: usize>;

impl<const N: u32, const L: usize> NativeReference<N, L> {
    /// The low `N` bits of `value`.
    fn mask(value: u128) -> u128 {
        if N == 128 { value } else { value & ((1u128 << N) - 1) }
    }

    /// The `N`-bit pattern `value` read as two's complement.
    fn signed(value: u128) -> i128 {
        let shift = 128 - N;
        ((Self::mask(value) << shift) as i128) >> shift
    }

    /// `value` wrapped into the signed range of `N` bits.
    fn wrap_signed(value: i128) -> i128 {
        Self::signed(value as u128)
    }

    /// The least signed value.
    fn minimum() -> i128 {
        Self::signed(1u128 << (N - 1))
    }

    /// The greatest signed value.
    fn maximum() -> i128 {
        Self::signed((1u128 << (N - 1)) - 1)
    }

    /// Operand patterns: the edge values, then deterministic random ones.
    fn operands(random: &mut ShiftGenerator, count: usize) -> Vec<u128> {
        let mut values: Vec<u128> = vec![0, 1, 2, Self::mask(u128::MAX), Self::mask(1u128 << (N - 1)), Self::mask((1u128 << (N - 1)) - 1), Self::mask(u128::MAX - 1)];
        for _ in 0..count {
            let pattern = (random.next_bits() as u128) << 64 | random.next_bits() as u128;
            values.push(Self::mask(pattern >> (random.next_bits() % 128)));
        }
        values
    }

    /// Checks every operator and H.1 method of `WideUInt<N, L>` on the operand pairs.
    fn check_unsigned(count: usize) {
        let mut random = ShiftGenerator::new(u64::from(N));
        let values = Self::operands(&mut random, count);
        let maximum = Self::mask(u128::MAX);
        for (index, &a) in values.iter().enumerate() {
            let b = values[(index * 7 + 3) % values.len()];
            let (x, y) = (WideUInt::<N, L>::from_u128(a), WideUInt::<N, L>::from_u128(b));
            let amount = random.next_bits() % (u64::from(N) + 8);
            let note = format!("N={N} a={a} b={b} amount={amount}");
            assert_eq!((x + y).low_u128(), Self::mask(a.wrapping_add(b)), "add {note}");
            assert_eq!((x - y).low_u128(), Self::mask(a.wrapping_sub(b)), "sub {note}");
            assert_eq!((x * y).low_u128(), Self::mask(a.wrapping_mul(b)), "mul {note}");
            assert_eq!((-x).low_u128(), Self::mask(0u128.wrapping_sub(a)), "neg {note}");
            assert_eq!((!x).low_u128(), Self::mask(!a), "not {note}");
            assert_eq!(((x & y).low_u128(), (x | y).low_u128(), (x ^ y).low_u128()), (a & b, a | b, a ^ b), "bit ops {note}");
            assert_eq!(x.shift_left(amount).low_u128(), if amount >= u64::from(N) { 0 } else { Self::mask(a << amount) }, "shl {note}");
            assert_eq!(x.shift_right(amount).low_u128(), if amount >= u64::from(N) { 0 } else { a >> amount }, "shr {note}");
            assert_eq!(x.cmp(&y), a.cmp(&b), "cmp {note}");
            assert_eq!(x.checked_divide(y).map(|v| v.low_u128()), a.checked_div(b), "div {note}");
            assert_eq!(x.checked_remainder(y).map(|v| v.low_u128()), a.checked_rem(b), "rem {note}");
            assert_eq!(x.rem_euclid_checked(y).map(|v| v.low_u128()), a.checked_rem_euclid(b), "rem_euclid {note}");
            assert_eq!((x.minimum(y).low_u128(), x.maximum_of(y).low_u128()), (a.min(b), a.max(b)), "min/max {note}");
            let (low, high) = (a.min(b), a.max(b));
            assert_eq!(x.clamp_checked(WideUInt::from_u128(low), WideUInt::from_u128(high)).map(|v| v.low_u128()), Some(a.clamp(low, high)), "clamp {note}");
            if a != b {
                assert!(x.clamp_checked(WideUInt::from_u128(high), WideUInt::from_u128(low)).is_none(), "clamp lo > hi {note}");
            }
            let exponent = (random.next_bits() % 9) as u32;
            assert_eq!(x.pow(exponent).low_u128(), Self::mask(a.wrapping_pow(exponent)), "pow {note}");
            let sum = a.checked_add(b).filter(|&v| v <= maximum);
            let difference = a.checked_sub(b);
            let product = a.checked_mul(b).filter(|&v| v <= maximum);
            assert_eq!(x.checked_add(y).map(|v| v.low_u128()), sum, "checked_add {note}");
            assert_eq!(x.checked_sub(y).map(|v| v.low_u128()), difference, "checked_sub {note}");
            assert_eq!(x.checked_mul(y).map(|v| v.low_u128()), product, "checked_mul {note}");
            assert_eq!(x.saturating_add(y).low_u128(), sum.unwrap_or(maximum), "saturating_add {note}");
            assert_eq!(x.saturating_sub(y).low_u128(), difference.unwrap_or(0), "saturating_sub {note}");
            assert_eq!(x.saturating_mul(y).low_u128(), product.unwrap_or(maximum), "saturating_mul {note}");
            assert_eq!((x.add_overflows(y), x.sub_overflows(y), x.mul_overflows(y)), (sum.is_none(), difference.is_none(), product.is_none()), "overflows {note}");
            assert_eq!(x.to_unsigned::<65, 2>().low_u128(), a & ((1u128 << 65) - 1), "as UInt<65> {note}");
            assert_eq!(x.to_unsigned::<200, 4>().low_u128(), a, "as UInt<200> {note}");
            assert_eq!(x.to_unsigned::<200, 4>().limbs()[2..], [0, 0], "zero extension {note}");
            assert_eq!(x.to_unsigned::<64, 1>().low_u64(), a as u64, "as UInt<64> {note}");
            assert_eq!(x.to_signed::<65, 2>().low_i128(), NativeReference::<65, 2>::signed(a), "as Int<65> {note}");
            assert_eq!(x.to_signed::<200, 4>().low_i128(), a as i128, "as Int<200> {note}");
        }
    }

    /// Checks every operator and H.1 method of `WideInt<N, L>` on the operand pairs.
    fn check_signed(count: usize) {
        let mut random = ShiftGenerator::new(1000 + u64::from(N));
        let values: Vec<i128> = Self::operands(&mut random, count).into_iter().map(Self::signed).chain([-1, Self::minimum(), Self::maximum(), Self::minimum() + 1]).collect();
        for (index, &a) in values.iter().enumerate() {
            let b = values[(index * 7 + 3) % values.len()];
            let (x, y) = (WideInt::<N, L>::from_i128(a), WideInt::<N, L>::from_i128(b));
            let amount = random.next_bits() % (u64::from(N) + 8);
            let note = format!("N={N} a={a} b={b} amount={amount}");
            assert_eq!(x.low_i128(), a, "round trip {note}");
            assert_eq!((x + y).low_i128(), Self::wrap_signed(a.wrapping_add(b)), "add {note}");
            assert_eq!((x - y).low_i128(), Self::wrap_signed(a.wrapping_sub(b)), "sub {note}");
            assert_eq!((x * y).low_i128(), Self::wrap_signed(a.wrapping_mul(b)), "mul {note}");
            assert_eq!((-x).low_i128(), Self::wrap_signed(a.wrapping_neg()), "neg {note}");
            assert_eq!((!x).low_i128(), !a, "not {note}");
            assert_eq!(((x & y).low_i128(), (x | y).low_i128(), (x ^ y).low_i128()), (a & b, a | b, a ^ b), "bit ops {note}");
            assert_eq!(x.shift_left(amount).low_i128(), if amount >= u64::from(N) { 0 } else { Self::wrap_signed(((a as u128) << amount) as i128) }, "shl {note}");
            assert_eq!(x.shift_right(amount).low_i128(), if amount >= u64::from(N) { if a < 0 { -1 } else { 0 } } else { a >> amount }, "shr {note}");
            assert_eq!(x.cmp(&y), a.cmp(&b), "cmp {note}");
            assert_eq!(x.checked_divide(y).map(|v| v.low_i128()), a.checked_div(b).or((b == -1).then(|| Self::wrap_signed(a.wrapping_neg()))), "div {note}");
            assert_eq!(x.checked_remainder(y).map(|v| v.low_i128()), (b != 0).then(|| a.wrapping_rem(b)), "rem {note}");
            assert_eq!(x.rem_euclid_checked(y).map(|v| v.low_i128()), (b != 0).then(|| a.wrapping_rem_euclid(b)), "rem_euclid {note}");
            assert_eq!(x.abs().low_i128(), Self::wrap_signed(a.wrapping_abs()), "abs {note}");
            assert_eq!(x.signum().low_i128(), a.signum(), "signum {note}");
            assert_eq!((x.minimum_of(y).low_i128(), x.maximum_of(y).low_i128()), (a.min(b), a.max(b)), "min/max {note}");
            let (low, high) = (a.min(b), a.max(b));
            assert_eq!(x.clamp_checked(WideInt::from_i128(low), WideInt::from_i128(high)).map(|v| v.low_i128()), Some(a.clamp(low, high)), "clamp {note}");
            if a != b {
                assert!(x.clamp_checked(WideInt::from_i128(high), WideInt::from_i128(low)).is_none(), "clamp lo > hi {note}");
            }
            let exponent = (random.next_bits() % 9) as u32;
            assert_eq!(x.pow(exponent).low_i128(), Self::wrap_signed(a.wrapping_pow(exponent)), "pow {note}");
            let in_range = |value: Option<i128>| value.filter(|v| (Self::minimum()..=Self::maximum()).contains(v));
            let (sum, difference, product) = (in_range(a.checked_add(b)), in_range(a.checked_sub(b)), in_range(a.checked_mul(b)));
            assert_eq!(x.checked_add(y).map(|v| v.low_i128()), sum, "checked_add {note}");
            assert_eq!(x.checked_sub(y).map(|v| v.low_i128()), difference, "checked_sub {note}");
            assert_eq!(x.checked_mul(y).map(|v| v.low_i128()), product, "checked_mul {note}");
            let saturate = |exact: Option<i128>, positive: bool| exact.map(|v| v.clamp(Self::minimum(), Self::maximum())).unwrap_or(if positive { Self::maximum() } else { Self::minimum() });
            assert_eq!(x.saturating_add(y).low_i128(), saturate(a.checked_add(b), a > 0), "saturating_add {note}");
            assert_eq!(x.saturating_sub(y).low_i128(), saturate(a.checked_sub(b), a >= 0), "saturating_sub {note}");
            assert_eq!(x.saturating_mul(y).low_i128(), saturate(a.checked_mul(b), (a < 0) == (b < 0)), "saturating_mul {note}");
            assert_eq!((x.add_overflows(y), x.sub_overflows(y), x.mul_overflows(y)), (sum.is_none(), difference.is_none(), product.is_none()), "overflows {note}");
            assert_eq!(x.to_signed::<65, 2>().low_i128(), NativeReference::<65, 2>::signed(a as u128), "as Int<65> {note}");
            assert_eq!(x.to_signed::<200, 4>().low_i128(), a, "as Int<200> sign-extends {note}");
            assert_eq!(x.to_signed::<200, 4>().sign_extended_limbs()[2], if a < 0 { u64::MAX } else { 0 }, "sign limbs {note}");
            assert_eq!(x.to_unsigned::<200, 4>().limbs()[2], if a < 0 { u64::MAX } else { 0 }, "as UInt<200> {note}");
            assert_eq!(x.to_unsigned::<64, 1>().low_u64(), a as u64, "as UInt<64> {note}");
            assert_eq!(x.to_unsigned::<65, 2>().low_u128(), (a as u128) & ((1u128 << 65) - 1), "as UInt<65> {note}");
        }
    }
}

#[test]
fn wide_integers_up_to_128_bits_agree_with_u128_and_i128() {
    NativeReference::<65, 2>::check_unsigned(10_000);
    NativeReference::<100, 2>::check_unsigned(10_000);
    NativeReference::<127, 2>::check_unsigned(10_000);
    NativeReference::<128, 2>::check_unsigned(10_000);
    NativeReference::<65, 2>::check_signed(10_000);
    NativeReference::<100, 2>::check_signed(10_000);
    NativeReference::<127, 2>::check_signed(10_000);
    NativeReference::<128, 2>::check_signed(10_000);
}

/// The `BigInt` oracle for any width: exact mathematics, then reduced modulo `2^N` or tested
/// against the range, as sections 1 rule 6 and H.1 state it.
struct ModularReference<const N: u32, const L: usize>;

impl<const N: u32, const L: usize> ModularReference<N, L> {
    /// `2^N`.
    fn modulus() -> BigInt {
        BigInt::from(1u32) << N
    }

    /// `value` reduced into `0..2^N`.
    fn wrap_unsigned(value: &BigInt) -> BigInt {
        ((value % Self::modulus()) + Self::modulus()) % Self::modulus()
    }

    /// `value` reduced into `-2^(N-1)..2^(N-1)`.
    fn wrap_signed(value: &BigInt) -> BigInt {
        let wrapped = Self::wrap_unsigned(value);
        if wrapped >= (BigInt::from(1u32) << (N - 1)) { wrapped - Self::modulus() } else { wrapped }
    }

    /// The pattern limbs of a value, as the library holds them.
    fn limbs(value: &BigInt) -> Vec<u64> {
        Self::wrap_unsigned(value).to_biguint().map(|magnitude| BigOracle::limbs(&magnitude)).unwrap_or_default()
    }

    /// Random operands of `N` bits, edge values first.
    fn operands(random: &mut ShiftGenerator, count: usize) -> Vec<BigInt> {
        let mut values: Vec<BigInt> = vec![BigInt::from(0), BigInt::from(1), BigInt::from(-1), Self::modulus() - 1, BigInt::from(1u32) << (N - 1), (BigInt::from(1u32) << (N - 1)) - 1];
        for _ in 0..count {
            let limbs = BigOracle::random_limbs(random, L as u64);
            let value = BigInt::from(BigOracle::value(&limbs));
            values.push(Self::wrap_unsigned(&(value >> (random.next_bits() % u64::from(N)))));
        }
        values
    }

    /// The unsigned operation set of `WideUInt<N, L>` against exact arithmetic.
    fn check_unsigned(count: usize) {
        let mut random = ShiftGenerator::new(u64::from(N) * 3);
        let values: Vec<BigInt> = Self::operands(&mut random, count).iter().map(Self::wrap_unsigned).collect();
        let maximum = Self::modulus() - 1;
        for (index, a) in values.iter().enumerate() {
            let b = &values[(index * 5 + 1) % values.len()];
            let x = WideUInt::<N, L>::from_limbs(&Self::limbs(a));
            let y = WideUInt::<N, L>::from_limbs(&Self::limbs(b));
            let amount = random.next_bits() % (u64::from(N) + 8);
            let note = format!("N={N} a={a} b={b} amount={amount}");
            let pattern = |value: WideUInt<N, L>| BigInt::from(BigOracle::value(value.limbs()));
            assert_eq!(pattern(x + y), Self::wrap_unsigned(&(a + b)), "add {note}");
            assert_eq!(pattern(x - y), Self::wrap_unsigned(&(a - b)), "sub {note}");
            assert_eq!(pattern(x * y), Self::wrap_unsigned(&(a * b)), "mul {note}");
            assert_eq!(pattern(-x), Self::wrap_unsigned(&(-a)), "neg {note}");
            assert_eq!(pattern(!x), &maximum - a, "not {note}");
            assert_eq!(pattern(x.shift_left(amount)), if amount >= u64::from(N) { BigInt::from(0) } else { Self::wrap_unsigned(&(a << amount)) }, "shl {note}");
            assert_eq!(pattern(x.shift_right(amount)), a >> amount, "shr {note}");
            assert_eq!(x.cmp(&y), a.cmp(b), "cmp {note}");
            assert_eq!(x.checked_divide(y).map(pattern), (b != &BigInt::from(0)).then(|| a / b), "div {note}");
            assert_eq!(x.checked_remainder(y).map(pattern), (b != &BigInt::from(0)).then(|| a % b), "rem {note}");
            let exponent = (random.next_bits() % 6) as u32;
            let mut expected_power = BigInt::from(1);
            for _ in 0..exponent {
                expected_power = Self::wrap_unsigned(&(expected_power * a));
            }
            assert_eq!(pattern(x.pow(exponent)), expected_power, "pow {note}");
            let in_range = |exact: BigInt| (exact >= BigInt::from(0) && exact <= maximum).then_some(exact);
            assert_eq!(x.checked_add(y).map(pattern), in_range(a + b), "checked_add {note}");
            assert_eq!(x.checked_sub(y).map(pattern), in_range(a - b), "checked_sub {note}");
            assert_eq!(x.checked_mul(y).map(pattern), in_range(a * b), "checked_mul {note}");
            let clamp = |exact: BigInt| if exact < BigInt::from(0) { BigInt::from(0) } else if exact > maximum { maximum.clone() } else { exact };
            assert_eq!(pattern(x.saturating_add(y)), clamp(a + b), "saturating_add {note}");
            assert_eq!(pattern(x.saturating_sub(y)), clamp(a - b), "saturating_sub {note}");
            assert_eq!(pattern(x.saturating_mul(y)), clamp(a * b), "saturating_mul {note}");
            assert_eq!((x.add_overflows(y), x.sub_overflows(y), x.mul_overflows(y)), (in_range(a + b).is_none(), in_range(a - b).is_none(), in_range(a * b).is_none()), "overflows {note}");
            assert_eq!(BigInt::from(BigOracle::value(x.to_unsigned::<130, 3>().limbs())), Self::wrap_unsigned_to(a, 130), "as UInt<130> {note}");
            assert_eq!(BigInt::from(BigOracle::value(x.to_unsigned::<70, 2>().limbs())), a % (BigInt::from(1u32) << 70), "as UInt<70> {note}");
            assert_eq!(x.to_decimal_string(), a.to_string(), "decimal {note}");
        }
    }

    /// The signed operation set of `WideInt<N, L>` against exact arithmetic.
    fn check_signed(count: usize) {
        let mut random = ShiftGenerator::new(u64::from(N) * 5);
        let values: Vec<BigInt> = Self::operands(&mut random, count).iter().map(Self::wrap_signed).collect();
        let (minimum, maximum) = (-(BigInt::from(1u32) << (N - 1)), (BigInt::from(1u32) << (N - 1)) - 1);
        for (index, a) in values.iter().enumerate() {
            let b = &values[(index * 5 + 1) % values.len()];
            let x = WideInt::<N, L>::from_limbs(&Self::limbs(a));
            let y = WideInt::<N, L>::from_limbs(&Self::limbs(b));
            let amount = random.next_bits() % (u64::from(N) + 8);
            let note = format!("N={N} a={a} b={b} amount={amount}");
            let value = |wide: WideInt<N, L>| Self::wrap_signed(&BigInt::from(BigOracle::value(wide.limbs())));
            assert_eq!(value(x), a.clone(), "round trip {note}");
            assert_eq!(value(x + y), Self::wrap_signed(&(a + b)), "add {note}");
            assert_eq!(value(x - y), Self::wrap_signed(&(a - b)), "sub {note}");
            assert_eq!(value(x * y), Self::wrap_signed(&(a * b)), "mul {note}");
            assert_eq!(value(-x), Self::wrap_signed(&(-a)), "neg {note}");
            assert_eq!(value(!x), -a - 1, "not {note}");
            assert_eq!(value(x.shift_left(amount)), if amount >= u64::from(N) { BigInt::from(0) } else { Self::wrap_signed(&(a << amount)) }, "shl {note}");
            assert_eq!(value(x.shift_right(amount)), a >> amount, "shr {note}");
            assert_eq!(x.cmp(&y), a.cmp(b), "cmp {note}");
            let zero = BigInt::from(0);
            assert_eq!(x.checked_divide(y).map(value), (b != &zero).then(|| Self::wrap_signed(&(a / b))), "div {note}");
            assert_eq!(x.checked_remainder(y).map(value), (b != &zero).then(|| a % b), "rem {note}");
            let euclid = |dividend: &BigInt, divisor: &BigInt| {
                let remainder = dividend % divisor;
                if remainder < zero { remainder + (if divisor < &zero { -divisor } else { divisor.clone() }) } else { remainder }
            };
            assert_eq!(x.rem_euclid_checked(y).map(value), (b != &zero).then(|| euclid(a, b)), "rem_euclid {note}");
            assert_eq!(value(x.abs()), Self::wrap_signed(&if a < &zero { -a } else { a.clone() }), "abs {note}");
            assert_eq!(value(x.signum()), BigInt::from(if a < &zero { -1 } else if a == &zero { 0 } else { 1 }), "signum {note}");
            let in_range = |exact: BigInt| (exact >= minimum && exact <= maximum).then_some(exact);
            assert_eq!(x.checked_add(y).map(value), in_range(a + b), "checked_add {note}");
            assert_eq!(x.checked_sub(y).map(value), in_range(a - b), "checked_sub {note}");
            assert_eq!(x.checked_mul(y).map(value), in_range(a * b), "checked_mul {note}");
            let clamp = |exact: BigInt| if exact < minimum { minimum.clone() } else if exact > maximum { maximum.clone() } else { exact };
            assert_eq!(value(x.saturating_add(y)), clamp(a + b), "saturating_add {note}");
            assert_eq!(value(x.saturating_sub(y)), clamp(a - b), "saturating_sub {note}");
            assert_eq!(value(x.saturating_mul(y)), clamp(a * b), "saturating_mul {note}");
            assert_eq!(BigInt::from(BigOracle::value(&x.to_unsigned::<1100, 18>().limbs()[..])), Self::wrap_unsigned_to(a, 1100), "as UInt<1100> {note}");
            assert_eq!(x.to_decimal_string(), a.to_string(), "decimal {note}");
        }
    }

    /// `value` reduced into `0..2^bits`.
    fn wrap_unsigned_to(value: &BigInt, bits: u32) -> BigInt {
        let modulus = BigInt::from(1u32) << bits;
        ((value % &modulus) + &modulus) % &modulus
    }
}

#[test]
fn wide_integers_above_128_bits_agree_with_big_integers_modulo_two_to_the_n() {
    ModularReference::<129, 3>::check_unsigned(2_000);
    ModularReference::<200, 4>::check_unsigned(2_000);
    ModularReference::<256, 4>::check_unsigned(2_000);
    ModularReference::<1000, 16>::check_unsigned(1_000);
    ModularReference::<129, 3>::check_signed(2_000);
    ModularReference::<200, 4>::check_signed(2_000);
    ModularReference::<256, 4>::check_signed(2_000);
    ModularReference::<1000, 16>::check_signed(1_000);
}
