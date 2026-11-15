//! `UInt<N>` and `Int<N>`: the MSL integer types for `N` in 1..=64, held in a `u64` or an `i64`
//! (spec RS-10). Arithmetic wraps modulo `2^N` in every build, `Int<N>` in two's complement
//! (MSL §1 rule 6); an integer division or remainder by zero panics naming the operation
//! (§1 rule 11). `as` among integer types truncates to the target's width and extends by the
//! source's sign (§1 rule 1); from `f64` it saturates to the target's range, NaN giving 0
//! (ROADMAP G4); a shift by `N` or more gives the mathematical result truncated to `N` bits: 0
//! for `<<` and an unsigned `>>`, the sign for a signed `>>` (ROADMAP G5).
//!
//! A `UInt<N>` always holds a value below `2^N`; an `Int<N>` always holds its value sign-extended
//! to 64 bits. Every operation restores that form, so derived equality and order are the
//! integers' own.

use core::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

/// `UInt<N>` (`uN`): `N` bits, unsigned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UInt<const N: u32>(u64);

/// `Int<N>` (`iN`): `N` bits, two's complement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Int<const N: u32>(i64);

impl<const N: u32> UInt<N> {
    /// A width outside 1..=64 does not compile.
    const WIDTH: u32 = {
        assert!(N >= 1 && N <= 64, "UInt<N> holds 1 to 64 bits");
        N
    };
    /// The largest value, `2^N - 1`.
    pub const MAX: u64 = if N >= 64 { u64::MAX } else { (1u64 << N) - 1 };

    /// `v` reduced modulo `2^N`.
    pub fn new(v: u64) -> UInt<N> {
        let _ = Self::WIDTH;
        UInt(v & Self::MAX)
    }

    /// The value.
    pub fn get(self) -> u64 {
        self.0
    }

    pub fn min(self, other: UInt<N>) -> UInt<N> {
        if other.0 < self.0 { other } else { self }
    }

    pub fn max(self, other: UInt<N>) -> UInt<N> {
        if other.0 > self.0 { other } else { self }
    }

    /// `self` limited to `lo..=hi`; panics when `lo > hi`, as Rust's `clamp` does.
    pub fn clamp(self, lo: UInt<N>, hi: UInt<N>) -> UInt<N> {
        if lo.0 > hi.0 {
            panic!("`clamp` on UInt<{N}> with a lower bound {} above the upper bound {}", lo.0, hi.0);
        }
        self.max(lo).min(hi)
    }

    /// `self as UInt<M>`: the low `M` bits.
    pub fn to_uint<const M: u32>(self) -> UInt<M> {
        UInt::new(self.0)
    }

    /// `self as Int<M>`: the low `M` bits, read in two's complement.
    pub fn to_int<const M: u32>(self) -> Int<M> {
        Int::new(self.0 as i64)
    }

    /// `self as f64`, rounded to nearest as Rust's `as` does.
    pub fn to_f64(self) -> f64 {
        self.0 as f64
    }

    /// `x as UInt<N>`: truncated toward zero and saturated to `0..=2^N - 1`; NaN is 0.
    pub fn from_f64(x: f64) -> UInt<N> {
        // `as u64` truncates toward zero, saturates to the `u64` range and maps NaN to 0.
        UInt::new((x as u64).min(Self::MAX))
    }

    /// `b as UInt<N>`: 1 or 0.
    pub fn from_bool(b: bool) -> UInt<N> {
        UInt::new(u64::from(b))
    }

    /// `self << amount`: the low `N` bits of `self · 2^amount`.
    pub fn shl_by(self, amount: u64) -> UInt<N> {
        if amount >= u64::from(N) { UInt::new(0) } else { UInt::new(self.0 << amount) }
    }

    /// `self >> amount`: `floor(self / 2^amount)`.
    pub fn shr_by(self, amount: u64) -> UInt<N> {
        if amount >= u64::from(N) { UInt::new(0) } else { UInt::new(self.0 >> amount) }
    }

    /// The amount of a shift: never negative.
    fn amount(self, _op: &str) -> u64 {
        self.0
    }
}

impl<const N: u32> Int<N> {
    /// A width outside 1..=64 does not compile.
    const WIDTH: u32 = {
        assert!(N >= 1 && N <= 64, "Int<N> holds 1 to 64 bits");
        N
    };
    /// The largest value, `2^(N-1) - 1`.
    pub const MAX: i64 = if N >= 64 { i64::MAX } else { (1i64 << (N - 1)) - 1 };
    /// The smallest value, `-2^(N-1)`.
    pub const MIN: i64 = if N >= 64 { i64::MIN } else { -(1i64 << (N - 1)) };

    /// `v` reduced modulo `2^N` into `MIN..=MAX`.
    pub fn new(v: i64) -> Int<N> {
        let shift = 64 - Self::WIDTH;
        Int((v << shift) >> shift)
    }

    /// The value.
    pub fn get(self) -> i64 {
        self.0
    }

    pub fn min(self, other: Int<N>) -> Int<N> {
        if other.0 < self.0 { other } else { self }
    }

    pub fn max(self, other: Int<N>) -> Int<N> {
        if other.0 > self.0 { other } else { self }
    }

    /// `self` limited to `lo..=hi`; panics when `lo > hi`, as Rust's `clamp` does.
    pub fn clamp(self, lo: Int<N>, hi: Int<N>) -> Int<N> {
        if lo.0 > hi.0 {
            panic!("`clamp` on Int<{N}> with a lower bound {} above the upper bound {}", lo.0, hi.0);
        }
        self.max(lo).min(hi)
    }

    /// `self as UInt<M>`: sign-extended, then the low `M` bits.
    pub fn to_uint<const M: u32>(self) -> UInt<M> {
        UInt::new(self.0 as u64)
    }

    /// `self as Int<M>`: sign-extended, then the low `M` bits in two's complement.
    pub fn to_int<const M: u32>(self) -> Int<M> {
        Int::new(self.0)
    }

    /// `self as f64`, rounded to nearest as Rust's `as` does.
    pub fn to_f64(self) -> f64 {
        self.0 as f64
    }

    /// `x as Int<N>`: truncated toward zero and saturated to `MIN..=MAX`; NaN is 0.
    pub fn from_f64(x: f64) -> Int<N> {
        // `as i64` truncates toward zero, saturates to the `i64` range and maps NaN to 0.
        Int::new((x as i64).clamp(Self::MIN, Self::MAX))
    }

    /// `b as Int<N>`: 1 or 0, so `true as i1` is -1, as two's complement has it.
    pub fn from_bool(b: bool) -> Int<N> {
        Int::new(i64::from(b))
    }

    /// `self << amount`: the low `N` bits of `self · 2^amount`.
    pub fn shl_by(self, amount: u64) -> Int<N> {
        if amount >= u64::from(N) { Int::new(0) } else { Int::new(((self.0 as u64) << amount) as i64) }
    }

    /// `self >> amount`: `floor(self / 2^amount)`, filling with the sign.
    pub fn shr_by(self, amount: u64) -> Int<N> {
        if amount >= u64::from(N) { Int::new(if self.0 < 0 { -1 } else { 0 }) } else { Int::new(self.0 >> amount) }
    }

    /// The amount of a shift: a negative one panics naming the operation.
    fn amount(self, op: &str) -> u64 {
        if self.0 < 0 {
            panic!("a shift by a negative amount: `{op}` by Int<{N}> {}", self.0);
        }
        self.0 as u64
    }
}

impl<const N: u32> Add for UInt<N> {
    type Output = UInt<N>;
    fn add(self, rhs: UInt<N>) -> UInt<N> {
        UInt::new(self.0.wrapping_add(rhs.0))
    }
}

impl<const N: u32> Sub for UInt<N> {
    type Output = UInt<N>;
    fn sub(self, rhs: UInt<N>) -> UInt<N> {
        UInt::new(self.0.wrapping_sub(rhs.0))
    }
}

impl<const N: u32> Mul for UInt<N> {
    type Output = UInt<N>;
    fn mul(self, rhs: UInt<N>) -> UInt<N> {
        UInt::new(self.0.wrapping_mul(rhs.0))
    }
}

impl<const N: u32> Div for UInt<N> {
    type Output = UInt<N>;
    fn div(self, rhs: UInt<N>) -> UInt<N> {
        if rhs.0 == 0 {
            panic!("integer division by zero: UInt<{N}> {} / 0", self.0);
        }
        UInt::new(self.0 / rhs.0)
    }
}

impl<const N: u32> Rem for UInt<N> {
    type Output = UInt<N>;
    fn rem(self, rhs: UInt<N>) -> UInt<N> {
        if rhs.0 == 0 {
            panic!("integer remainder by zero: UInt<{N}> {} % 0", self.0);
        }
        UInt::new(self.0 % rhs.0)
    }
}

impl<const N: u32> Neg for UInt<N> {
    type Output = UInt<N>;
    fn neg(self) -> UInt<N> {
        UInt::new(self.0.wrapping_neg())
    }
}

impl<const N: u32> Not for UInt<N> {
    type Output = UInt<N>;
    fn not(self) -> UInt<N> {
        UInt::new(!self.0)
    }
}

impl<const N: u32> BitAnd for UInt<N> {
    type Output = UInt<N>;
    fn bitand(self, rhs: UInt<N>) -> UInt<N> {
        UInt::new(self.0 & rhs.0)
    }
}

impl<const N: u32> BitOr for UInt<N> {
    type Output = UInt<N>;
    fn bitor(self, rhs: UInt<N>) -> UInt<N> {
        UInt::new(self.0 | rhs.0)
    }
}

impl<const N: u32> BitXor for UInt<N> {
    type Output = UInt<N>;
    fn bitxor(self, rhs: UInt<N>) -> UInt<N> {
        UInt::new(self.0 ^ rhs.0)
    }
}

impl<const N: u32, const M: u32> Shl<UInt<M>> for UInt<N> {
    type Output = UInt<N>;
    fn shl(self, rhs: UInt<M>) -> UInt<N> {
        self.shl_by(rhs.amount("<<"))
    }
}

impl<const N: u32, const M: u32> Shl<Int<M>> for UInt<N> {
    type Output = UInt<N>;
    fn shl(self, rhs: Int<M>) -> UInt<N> {
        self.shl_by(rhs.amount("<<"))
    }
}

impl<const N: u32, const M: u32> Shr<UInt<M>> for UInt<N> {
    type Output = UInt<N>;
    fn shr(self, rhs: UInt<M>) -> UInt<N> {
        self.shr_by(rhs.amount(">>"))
    }
}

impl<const N: u32, const M: u32> Shr<Int<M>> for UInt<N> {
    type Output = UInt<N>;
    fn shr(self, rhs: Int<M>) -> UInt<N> {
        self.shr_by(rhs.amount(">>"))
    }
}

impl<const N: u32> Add for Int<N> {
    type Output = Int<N>;
    fn add(self, rhs: Int<N>) -> Int<N> {
        Int::new(self.0.wrapping_add(rhs.0))
    }
}

impl<const N: u32> Sub for Int<N> {
    type Output = Int<N>;
    fn sub(self, rhs: Int<N>) -> Int<N> {
        Int::new(self.0.wrapping_sub(rhs.0))
    }
}

impl<const N: u32> Mul for Int<N> {
    type Output = Int<N>;
    fn mul(self, rhs: Int<N>) -> Int<N> {
        Int::new(self.0.wrapping_mul(rhs.0))
    }
}

impl<const N: u32> Div for Int<N> {
    type Output = Int<N>;
    /// Truncated toward zero, as Rust's; `MIN / -1` wraps to `MIN`.
    fn div(self, rhs: Int<N>) -> Int<N> {
        if rhs.0 == 0 {
            panic!("integer division by zero: Int<{N}> {} / 0", self.0);
        }
        Int::new(self.0.wrapping_div(rhs.0))
    }
}

impl<const N: u32> Rem for Int<N> {
    type Output = Int<N>;
    /// The sign of the dividend, as Rust's; `MIN % -1` is 0.
    fn rem(self, rhs: Int<N>) -> Int<N> {
        if rhs.0 == 0 {
            panic!("integer remainder by zero: Int<{N}> {} % 0", self.0);
        }
        Int::new(self.0.wrapping_rem(rhs.0))
    }
}

impl<const N: u32> Neg for Int<N> {
    type Output = Int<N>;
    fn neg(self) -> Int<N> {
        Int::new(self.0.wrapping_neg())
    }
}

impl<const N: u32> Not for Int<N> {
    type Output = Int<N>;
    fn not(self) -> Int<N> {
        Int::new(!self.0)
    }
}

impl<const N: u32> BitAnd for Int<N> {
    type Output = Int<N>;
    fn bitand(self, rhs: Int<N>) -> Int<N> {
        Int::new(self.0 & rhs.0)
    }
}

impl<const N: u32> BitOr for Int<N> {
    type Output = Int<N>;
    fn bitor(self, rhs: Int<N>) -> Int<N> {
        Int::new(self.0 | rhs.0)
    }
}

impl<const N: u32> BitXor for Int<N> {
    type Output = Int<N>;
    fn bitxor(self, rhs: Int<N>) -> Int<N> {
        Int::new(self.0 ^ rhs.0)
    }
}

impl<const N: u32, const M: u32> Shl<UInt<M>> for Int<N> {
    type Output = Int<N>;
    fn shl(self, rhs: UInt<M>) -> Int<N> {
        self.shl_by(rhs.amount("<<"))
    }
}

impl<const N: u32, const M: u32> Shl<Int<M>> for Int<N> {
    type Output = Int<N>;
    fn shl(self, rhs: Int<M>) -> Int<N> {
        self.shl_by(rhs.amount("<<"))
    }
}

impl<const N: u32, const M: u32> Shr<UInt<M>> for Int<N> {
    type Output = Int<N>;
    fn shr(self, rhs: UInt<M>) -> Int<N> {
        self.shr_by(rhs.amount(">>"))
    }
}

impl<const N: u32, const M: u32> Shr<Int<M>> for Int<N> {
    type Output = Int<N>;
    fn shr(self, rhs: Int<M>) -> Int<N> {
        self.shr_by(rhs.amount(">>"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Int, UInt};

    /// The references: the operation done on the integers themselves in `i128` (wrapping only
    /// where a product of two 64-bit values would leave `i128`, which a reduction modulo
    /// `2^N <= 2^64` does not see), then reduced modulo `2^N` into the type's range.
    struct Reference;

    impl Reference {
        fn modulus(n: u32) -> i128 {
            1i128 << n
        }

        /// `v` modulo `2^n` in `0..2^n`.
        fn unsigned(v: i128, n: u32) -> i128 {
            v.rem_euclid(Self::modulus(n))
        }

        /// `v` modulo `2^n` in `-2^(n-1)..2^(n-1)`.
        fn signed(v: i128, n: u32) -> i128 {
            let r = Self::unsigned(v, n);
            if r >= Self::modulus(n) / 2 { r - Self::modulus(n) } else { r }
        }

        /// `a op b` on the integers; `None` for a division or remainder by zero.
        fn apply(op: &str, a: i128, b: i128) -> Option<i128> {
            Some(match op {
                "+" => a + b,
                "-" => a - b,
                "*" => a.wrapping_mul(b),
                "/" if b == 0 => return None,
                "%" if b == 0 => return None,
                "/" => a / b,
                "%" => a % b,
                "&" => a & b,
                "|" => a | b,
                "^" => a ^ b,
                _ => unreachable!("no operator {op}"),
            })
        }
    }

    /// A fixed linear congruential sequence of 64-bit words.
    struct Words(u64);

    impl Iterator for Words {
        type Item = u64;
        fn next(&mut self) -> Option<u64> {
            self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            Some(self.0 ^ (self.0 >> 29))
        }
    }

    const OPS: [&str; 8] = ["+", "-", "*", "/", "%", "&", "|", "^"];

    /// Runs `f` and returns its panic message, `None` when it does not panic.
    fn panic_of(f: impl FnOnce() + std::panic::UnwindSafe) -> Option<String> {
        std::panic::catch_unwind(f).err().map(|e| {
            e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
        })
    }

    /// Every operator of `UInt<N>` against the reference, at the boundaries and 1000 random
    /// pairs.
    fn unsigned<const N: u32>() {
        let max = UInt::<N>::MAX;
        let mut values = vec![0, 1, 2, max / 2, max / 2 + 1, max.saturating_sub(1), max];
        let mut words = Words(u64::from(N) * 7919);
        let pairs: Vec<(u64, u64)> = (0..1000).map(|_| (words.next().unwrap_or(0) & max, words.next().unwrap_or(0) & max)).collect();
        values.retain(|v| *v <= max);
        values.sort_unstable();
        values.dedup();
        let edges: Vec<(u64, u64)> = values.iter().flat_map(|a| values.iter().map(move |b| (*a, *b))).collect();
        for (a, b) in edges.into_iter().chain(pairs) {
            let (x, y) = (UInt::<N>::new(a), UInt::<N>::new(b));
            assert_eq!((x.get(), y.get()), (a, b), "UInt<{N}> holds {a} and {b}");
            for op in OPS {
                let got = match op {
                    "/" | "%" if b == 0 => {
                        let message = panic_of(move || {
                            let _ = if op == "/" { x / y } else { x % y };
                        });
                        let what = if op == "/" { "division" } else { "remainder" };
                        let message = message.unwrap_or_else(|| panic!("UInt<{N}> {a} {op} 0 did not panic"));
                        assert!(message.contains(what) && message.contains(&format!("UInt<{N}>")), "the panic names the operation: {message}");
                        continue;
                    }
                    "+" => x + y,
                    "-" => x - y,
                    "*" => x * y,
                    "/" => x / y,
                    "%" => x % y,
                    "&" => x & y,
                    "|" => x | y,
                    _ => x ^ y,
                };
                let want = Reference::apply(op, i128::from(a), i128::from(b)).map(|v| Reference::unsigned(v, N));
                assert_eq!(Some(i128::from(got.get())), want, "UInt<{N}>: {a} {op} {b}");
            }
            assert_eq!(i128::from((-x).get()), Reference::unsigned(-i128::from(a), N), "UInt<{N}>: -{a}");
            assert_eq!(i128::from((!x).get()), Reference::unsigned(!i128::from(a), N), "UInt<{N}>: !{a}");
            assert_eq!(x < y, a < b, "UInt<{N}>: {a} < {b}");
            assert_eq!(x == y, a == b, "UInt<{N}>: {a} == {b}");
            assert_eq!(x.min(y).get(), a.min(b));
            assert_eq!(x.max(y).get(), a.max(b));
        }
        // `u9` 511 + 1 is 0 (spec RS-10, Independent Test)
        if N == 9 {
            assert_eq!(UInt::<9>::new(511) + UInt::<9>::new(1), UInt::<9>::new(0));
        }
    }

    /// Every operator of `Int<N>` against the reference, at the boundaries and 1000 random
    /// pairs.
    fn signed<const N: u32>() {
        let (min, max) = (Int::<N>::MIN, Int::<N>::MAX);
        let mut values = vec![min, min + 1, -2, -1, 0, 1, 2, max - 1, max];
        values.retain(|v| (min..=max).contains(v));
        values.sort_unstable();
        values.dedup();
        let mut words = Words(u64::from(N) * 104_729);
        let mut draw = || Int::<N>::new(words.next().unwrap_or(0) as i64).get();
        let pairs: Vec<(i64, i64)> = (0..1000).map(|_| (draw(), draw())).collect();
        let edges: Vec<(i64, i64)> = values.iter().flat_map(|a| values.iter().map(move |b| (*a, *b))).collect();
        for (a, b) in edges.into_iter().chain(pairs) {
            let (x, y) = (Int::<N>::new(a), Int::<N>::new(b));
            assert_eq!((x.get(), y.get()), (a, b), "Int<{N}> holds {a} and {b}");
            for op in OPS {
                let got = match op {
                    "/" | "%" if b == 0 => {
                        let message = panic_of(move || {
                            let _ = if op == "/" { x / y } else { x % y };
                        });
                        let what = if op == "/" { "division" } else { "remainder" };
                        let message = message.unwrap_or_else(|| panic!("Int<{N}> {a} {op} 0 did not panic"));
                        assert!(message.contains(what) && message.contains(&format!("Int<{N}>")), "the panic names the operation: {message}");
                        continue;
                    }
                    "+" => x + y,
                    "-" => x - y,
                    "*" => x * y,
                    "/" => x / y,
                    "%" => x % y,
                    "&" => x & y,
                    "|" => x | y,
                    _ => x ^ y,
                };
                let want = Reference::apply(op, i128::from(a), i128::from(b)).map(|v| Reference::signed(v, N));
                assert_eq!(Some(i128::from(got.get())), want, "Int<{N}>: {a} {op} {b}");
            }
            assert_eq!(i128::from((-x).get()), Reference::signed(-i128::from(a), N), "Int<{N}>: -{a}");
            assert_eq!(i128::from((!x).get()), Reference::signed(!i128::from(a), N), "Int<{N}>: !{a}");
            assert_eq!(x < y, a < b, "Int<{N}>: {a} < {b}");
            assert_eq!(x.min(y).get(), a.min(b));
            assert_eq!(x.max(y).get(), a.max(b));
        }
    }

    #[test]
    fn arithmetic_wraps_at_every_width() {
        // spec RS-10 AC 1, MSL §1 rule 6 and rule 11: `+ - * / %` and unary `-` modulo `2^N`,
        // `Int<N>` in two's complement, a zero divisor a panic; the bit operators with them.
        unsigned::<1>();
        unsigned::<3>();
        unsigned::<8>();
        unsigned::<9>();
        unsigned::<12>();
        unsigned::<16>();
        unsigned::<31>();
        unsigned::<32>();
        unsigned::<63>();
        unsigned::<64>();
        signed::<1>();
        signed::<3>();
        signed::<8>();
        signed::<9>();
        signed::<12>();
        signed::<16>();
        signed::<31>();
        signed::<32>();
        signed::<63>();
        signed::<64>();
        // signed edge cases written out: MIN / -1 and -MIN wrap to MIN, MIN % -1 is 0, division
        // truncates toward zero and the remainder takes the dividend's sign (as Rust)
        let (min, minus_one) = (Int::<8>::new(-128), Int::<8>::new(-1));
        assert_eq!((min / minus_one).get(), -128);
        assert_eq!((-min).get(), -128);
        assert_eq!((min % minus_one).get(), 0);
        assert_eq!((Int::<8>::new(-7) / Int::<8>::new(2)).get(), -3);
        assert_eq!((Int::<8>::new(-7) % Int::<8>::new(2)).get(), -1);
        assert_eq!((Int::<8>::new(7) % Int::<8>::new(-2)).get(), 1);
        assert_eq!((Int::<64>::new(i64::MIN) / Int::<64>::new(-1)).get(), i64::MIN);
        assert_eq!((Int::<1>::new(-1) * Int::<1>::new(-1)).get(), -1, "i1: (-1)·(-1) = 1 = -1 mod 2");
        assert_eq!((UInt::<9>::new(0) - UInt::<9>::new(1)).get(), 511);
        assert_eq!((UInt::<64>::new(u64::MAX) + UInt::<64>::new(1)).get(), 0);
    }

    #[test]
    fn casts_truncate_and_extend_by_the_source_sign() {
        // spec RS-10 AC 2, MSL §1 rule 1 ("as in Rust"): at Rust's own widths every cast equals
        // Rust's `as`, over 1000 random words.
        for w in Words(42).take(1000) {
            let (u8_, i8_, u16_, i16_, u32_, i32_, u64_, i64_) = (w as u8, w as i8, w as u16, w as i16, w as u32, w as i32, w, w as i64);
            let x = UInt::<8>::new(u64::from(u8_));
            assert_eq!(x.to_int::<8>().get(), i64::from(u8_ as i8));
            assert_eq!(x.to_uint::<16>().get(), u64::from(u8_ as u16));
            assert_eq!(x.to_int::<64>().get(), u8_ as i64);
            let x = Int::<8>::new(i64::from(i8_));
            assert_eq!(x.to_uint::<8>().get(), u64::from(i8_ as u8));
            assert_eq!(x.to_uint::<16>().get(), u64::from(i8_ as u16), "sign-extended, then truncated");
            assert_eq!(x.to_int::<32>().get(), i64::from(i8_ as i32));
            assert_eq!(x.to_uint::<64>().get(), i8_ as u64);
            let x = UInt::<16>::new(u64::from(u16_));
            assert_eq!(x.to_uint::<8>().get(), u64::from(u16_ as u8));
            assert_eq!(x.to_int::<8>().get(), i64::from(u16_ as i8));
            let x = Int::<16>::new(i64::from(i16_));
            assert_eq!(x.to_int::<8>().get(), i64::from(i16_ as i8));
            assert_eq!(x.to_uint::<32>().get(), u64::from(i16_ as u32));
            let x = UInt::<32>::new(u64::from(u32_));
            assert_eq!(x.to_int::<32>().get(), i64::from(u32_ as i32));
            assert_eq!(x.to_int::<16>().get(), i64::from(u32_ as i16));
            let x = Int::<32>::new(i64::from(i32_));
            assert_eq!(x.to_uint::<64>().get(), i32_ as u64);
            assert_eq!(x.to_uint::<16>().get(), u64::from(i32_ as u16));
            let x = UInt::<64>::new(u64_);
            assert_eq!(x.to_int::<64>().get(), u64_ as i64);
            assert_eq!(x.to_uint::<32>().get(), u64::from(u64_ as u32));
            let x = Int::<64>::new(i64_);
            assert_eq!(x.to_uint::<64>().get(), i64_ as u64);
            assert_eq!(x.to_int::<16>().get(), i64::from(i64_ as i16));
        }
        // widths Rust lacks: truncate to the target's bits, extend by the source's sign
        assert_eq!(UInt::<9>::new(300).to_uint::<8>().get(), 44, "300 = 0x12C: low 8 bits 0x2C");
        assert_eq!(UInt::<9>::new(300).to_int::<9>().get(), -212, "0x12C read as 9-bit two's complement");
        assert_eq!(UInt::<3>::new(7).to_int::<32>().get(), 7, "an unsigned source extends with zeros");
        assert_eq!(Int::<3>::new(-1).to_int::<32>().get(), -1, "a signed source extends with its sign");
        assert_eq!(Int::<3>::new(-1).to_uint::<9>().get(), 511);
        assert_eq!(Int::<9>::new(-2).to_uint::<3>().get(), 6);
        assert_eq!(UInt::<1>::new(1).to_int::<1>().get(), -1);
        assert_eq!(Int::<12>::new(-2048).to_int::<9>().get(), 0, "-2048 = 0x800: low 9 bits 0");
        assert_eq!(UInt::<10>::new(1023).to_int::<16>().get(), 1023);
        assert_eq!(UInt::<10>::new(1023).to_int::<10>().get(), -1);
        // `bool as` integer: 1 or 0
        assert_eq!(UInt::<4>::from_bool(true).get(), 1);
        assert_eq!(Int::<4>::from_bool(false).get(), 0);
        // `new` reduces a literal modulo 2^N, as the cast of the wider literal does
        assert_eq!(UInt::<9>::new(512 + 3).get(), 3);
        assert_eq!(Int::<9>::new(256).get(), -256);
    }

    #[test]
    fn a_float_cast_saturates_and_nan_is_zero() {
        // spec RS-10 AC 3, ROADMAP G4: saturate to the N-bit range, NaN to 0; inside the range,
        // truncation toward zero (Rust's `as`)
        assert_eq!(UInt::<9>::from_f64(600.0).get(), 511);
        assert_eq!(UInt::<9>::from_f64(511.99).get(), 511);
        assert_eq!(UInt::<9>::from_f64(3.9).get(), 3);
        assert_eq!(UInt::<9>::from_f64(-3.5).get(), 0);
        assert_eq!(UInt::<9>::from_f64(f64::NAN).get(), 0);
        assert_eq!(UInt::<9>::from_f64(f64::INFINITY).get(), 511);
        assert_eq!(UInt::<9>::from_f64(f64::NEG_INFINITY).get(), 0);
        assert_eq!(UInt::<1>::from_f64(0.99).get(), 0);
        assert_eq!(UInt::<1>::from_f64(7.0).get(), 1);
        assert_eq!(UInt::<10>::from_f64(1023.0).get(), 1023, "the Adc's top code");
        assert_eq!(UInt::<64>::from_f64(1e30).get(), u64::MAX);
        assert_eq!(UInt::<64>::from_f64(-1e30).get(), 0);
        assert_eq!(Int::<8>::from_f64(200.0).get(), 127);
        assert_eq!(Int::<8>::from_f64(-200.0).get(), -128);
        assert_eq!(Int::<8>::from_f64(-3.9).get(), -3);
        assert_eq!(Int::<8>::from_f64(127.5).get(), 127);
        assert_eq!(Int::<8>::from_f64(f64::NAN).get(), 0);
        assert_eq!(Int::<1>::from_f64(5.0).get(), 0, "i1 holds -1..=0");
        assert_eq!(Int::<1>::from_f64(-5.0).get(), -1);
        assert_eq!(Int::<12>::from_f64(1e9).get(), 2047);
        assert_eq!(Int::<64>::from_f64(-1e30).get(), i64::MIN);
        assert_eq!(Int::<64>::from_f64(1e30).get(), i64::MAX);
        assert_eq!(Int::<64>::from_f64(f64::NAN).get(), 0);
        // and back: the value as f64
        assert_eq!(UInt::<9>::new(511).to_f64(), 511.0);
        assert_eq!(Int::<8>::new(-128).to_f64(), -128.0);
        // over the width and around it, against the saturated mathematical truncation
        for n_bits in [3u32, 9, 16, 31] {
            let (umax, imin, imax) = ((1i128 << n_bits) - 1, -(1i128 << (n_bits - 1)), (1i128 << (n_bits - 1)) - 1);
            for k in -40i32..40 {
                let x = f64::from(k) * (umax as f64) / 17.0 + 0.25;
                let t = x.trunc() as i128;
                let (u, i) = (t.clamp(0, umax), t.clamp(imin, imax));
                let (gu, gi) = match n_bits {
                    3 => (i128::from(UInt::<3>::from_f64(x).get()), i128::from(Int::<3>::from_f64(x).get())),
                    9 => (i128::from(UInt::<9>::from_f64(x).get()), i128::from(Int::<9>::from_f64(x).get())),
                    16 => (i128::from(UInt::<16>::from_f64(x).get()), i128::from(Int::<16>::from_f64(x).get())),
                    _ => (i128::from(UInt::<31>::from_f64(x).get()), i128::from(Int::<31>::from_f64(x).get())),
                };
                assert_eq!((gu, gi), (u, i), "N = {n_bits}, x = {x}");
            }
        }
    }

    /// Shifts of `UInt<N>`/`Int<N>` by `0..=N + 70` against `floor(a · 2^k)` and `floor(a / 2^k)`
    /// reduced to `N` bits.
    fn shifts<const N: u32>() {
        let mut words = Words(u64::from(N) + 5);
        for _ in 0..50 {
            let w = words.next().unwrap_or(0);
            let (u, i) = (UInt::<N>::new(w), Int::<N>::new(w as i64));
            for k in 0..=u64::from(N) + 70 {
                let (ua, ia) = (i128::from(u.get()), i128::from(i.get()));
                let shl = |a: i128| if k >= 64 { 0 } else { a.wrapping_mul(1i128 << k) };
                let shr = |a: i128| if k >= 127 { if a < 0 { -1 } else { 0 } } else { a >> k };
                assert_eq!(i128::from(u.shl_by(k).get()), Reference::unsigned(shl(ua), N), "UInt<{N}> {ua} << {k}");
                assert_eq!(i128::from(u.shr_by(k).get()), shr(ua), "UInt<{N}> {ua} >> {k}");
                assert_eq!(i128::from(i.shl_by(k).get()), Reference::signed(shl(ia), N), "Int<{N}> {ia} << {k}");
                assert_eq!(i128::from(i.shr_by(k).get()), shr(ia), "Int<{N}> {ia} >> {k}");
            }
        }
    }

    #[test]
    fn a_shift_by_the_width_or_more_is_the_truncated_result() {
        // spec RS-10 AC 4, ROADMAP G5: 0 for `<<` and an unsigned `>>`, the sign for a signed `>>`
        let amount = |k: u64| UInt::<8>::new(k);
        assert_eq!((UInt::<9>::new(1) << amount(8)).get(), 256);
        assert_eq!((UInt::<9>::new(1) << amount(9)).get(), 0);
        assert_eq!((UInt::<9>::new(511) << amount(1)).get(), 510);
        assert_eq!((UInt::<9>::new(511) >> amount(9)).get(), 0);
        assert_eq!((UInt::<9>::new(511) >> amount(200)).get(), 0);
        assert_eq!((Int::<8>::new(-128) >> amount(8)).get(), -1);
        assert_eq!((Int::<8>::new(-128) >> amount(200)).get(), -1);
        assert_eq!((Int::<8>::new(64) >> amount(8)).get(), 0);
        assert_eq!((Int::<8>::new(1) << amount(7)).get(), -128);
        assert_eq!((Int::<8>::new(1) << amount(8)).get(), 0);
        assert_eq!((Int::<8>::new(-1) << amount(100)).get(), 0);
        assert_eq!((UInt::<64>::new(1) << amount(63)).get(), 1 << 63);
        assert_eq!((UInt::<64>::new(1) << amount(64)).get(), 0);
        assert_eq!((Int::<64>::new(-1) >> amount(64)).get(), -1);
        assert_eq!((Int::<64>::new(i64::MAX) >> amount(64)).get(), 0);
        // the amount may be any integer type: `(k & 1) << 2i32` of `rev3`
        assert_eq!((UInt::<3>::new(1) << Int::<32>::new(2)).get(), 4);
        assert_eq!((UInt::<3>::new(4) >> Int::<32>::new(2)).get(), 1);
        assert_eq!((Int::<32>::new(-8) >> UInt::<64>::new(u64::MAX)).get(), -1);
        // a negative amount has no mathematical meaning here: a panic naming the shift
        let message = std::panic::catch_unwind(|| UInt::<8>::new(1) << Int::<8>::new(-1)).err().map(|e| {
            e.downcast_ref::<String>().cloned().unwrap_or_default()
        });
        assert!(message.is_some_and(|m| m.contains("negative") && m.contains("<<")));
        shifts::<1>();
        shifts::<3>();
        shifts::<8>();
        shifts::<9>();
        shifts::<12>();
        shifts::<16>();
        shifts::<31>();
        shifts::<32>();
        shifts::<63>();
        shifts::<64>();
    }
}
