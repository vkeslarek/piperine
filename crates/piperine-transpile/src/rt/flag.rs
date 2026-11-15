//! `Flagged<T>` and `Commits`: the values of digital code (spec RS-11, MSL §3.3) and the commits
//! of one reaction.
//!
//! A `Flagged<T>` is a value with one known flag, the flag of a carrier's pin (abi.md P3: one
//! known byte per field). An unknown value holds `T::default()`, so its bytes are the canonical
//! all-zero form. Arithmetic, comparisons, `as` and `^` are all-or-nothing (§3.3.2): one unknown
//! operand gives an unknown result, and the operation is not evaluated on it (an unknown divisor
//! is no division by zero). `&`, `|` and `!` work per level: a known all-false operand makes `&`
//! known all-false, a known all-true operand makes `|` known all-true, whatever the other
//! (`unknown & false == false`, `unknown | true == true`). A shift by a known amount fills with
//! known levels (§3.3.11): an unknown value shifted out entirely is known 0, except by a signed
//! `>>`, which fills with the sign. `known_or`, `is_known` and `high` read the flag (§3.3.4);
//! `decide` is the condition of an `if`, a `Fault` naming its origin when unknown (RS-11 AC 5).
//!
//! `Commits` collects the commits of one reaction and hands them to `React` in the order they
//! were made, after the body ran, so a fault makes none (design "Phase B: generated digital
//! code"). A commit whose delay is unknown commits `Unknown` at the next microstep (§3.3.3).

use core::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

use piperine_abi::device::{Fault, React};

use super::int::{Int, UInt};

/// A type a pin carries: its value bytes little-endian at offset 0, then one known byte; the
/// layouts of the plan (`piperine_plan::CarrierLayout`, spec RS-12 AC 1).
pub trait Carried: Copy + Default + PartialEq {
    /// The width of the value bytes.
    const WIDTH: usize;
    /// The pin's size: the known byte after the value, rounded to the value's alignment.
    const SIZE: usize;
    /// Writes the value bytes into `out[..WIDTH]`.
    fn encode(self, out: &mut [u8]);
    /// The value of `bytes[..WIDTH]`.
    fn decode(bytes: &[u8]) -> Self;
}

/// A carried type with levels, so with `&` and `|`: `bool` and the integers.
pub trait Levels: Carried {
    /// Every level true: the value `|` makes known whatever the other operand.
    fn ones() -> Self;
}

impl Carried for bool {
    const WIDTH: usize = 1;
    const SIZE: usize = 2;
    fn encode(self, out: &mut [u8]) {
        out[0] = u8::from(self);
    }
    fn decode(bytes: &[u8]) -> bool {
        bytes[0] != 0
    }
}

impl Levels for bool {
    fn ones() -> bool {
        true
    }
}

impl Carried for f64 {
    const WIDTH: usize = 8;
    const SIZE: usize = 16;
    fn encode(self, out: &mut [u8]) {
        out[..8].copy_from_slice(&self.to_le_bytes());
    }
    fn decode(bytes: &[u8]) -> f64 {
        let mut b = [0u8; 8];
        b.copy_from_slice(&bytes[..8]);
        f64::from_le_bytes(b)
    }
}

/// The value bytes of an integer pin.
struct Word;

impl Word {
    /// The value width of an `N`-bit integer: the narrowest of 1, 2, 4 and 8 bytes.
    const fn width(n: u32) -> usize {
        if n <= 8 {
            1
        } else if n <= 16 {
            2
        } else if n <= 32 {
            4
        } else {
            8
        }
    }

    /// The low `w` bytes of `bytes`, little-endian, zero-extended.
    fn read(bytes: &[u8], w: usize) -> u64 {
        let mut b = [0u8; 8];
        b[..w].copy_from_slice(&bytes[..w]);
        u64::from_le_bytes(b)
    }
}

impl<const N: u32> Carried for UInt<N> {
    const WIDTH: usize = Word::width(N);
    const SIZE: usize = 2 * Word::width(N);
    fn encode(self, out: &mut [u8]) {
        out[..Self::WIDTH].copy_from_slice(&self.get().to_le_bytes()[..Self::WIDTH]);
    }
    fn decode(bytes: &[u8]) -> UInt<N> {
        UInt::new(Word::read(bytes, Self::WIDTH))
    }
}

impl<const N: u32> Levels for UInt<N> {
    fn ones() -> UInt<N> {
        UInt::new(u64::MAX)
    }
}

impl<const N: u32> Carried for Int<N> {
    const WIDTH: usize = Word::width(N);
    const SIZE: usize = 2 * Word::width(N);
    fn encode(self, out: &mut [u8]) {
        out[..Self::WIDTH].copy_from_slice(&self.get().to_le_bytes()[..Self::WIDTH]);
    }
    fn decode(bytes: &[u8]) -> Int<N> {
        // the stored bytes hold the value sign-extended to their width: extend to 64 bits
        let shift = 64 - 8 * Self::WIDTH as u32;
        Int::new(((Word::read(bytes, Self::WIDTH) << shift) as i64) >> shift)
    }
}

impl<const N: u32> Levels for Int<N> {
    fn ones() -> Int<N> {
        Int::new(-1)
    }
}

/// A value that may be unknown. An unknown one holds `T::default()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flagged<T> {
    value: T,
    known: bool,
}

impl<T: Copy + Default> Flagged<T> {
    /// The fault code of an unknown `if` condition (RS-11 AC 5).
    pub const UNKNOWN_CONTROL: u32 = 3;

    /// A known value.
    pub fn known(value: T) -> Flagged<T> {
        Flagged { value, known: true }
    }

    /// `Unknown`.
    pub fn unknown() -> Flagged<T> {
        Flagged { value: T::default(), known: false }
    }

    /// `x.is_known()`: the flag, itself known.
    pub fn is_known(self) -> bool {
        self.known
    }

    /// `x.known_or(d)`: the value when known, `d` otherwise.
    pub fn known_or(self, default: T) -> T {
        if self.known { self.value } else { default }
    }

    /// `f` of the value, unknown when it is: a cast, a method of one operand.
    pub fn map<U: Copy + Default>(self, f: impl FnOnce(T) -> U) -> Flagged<U> {
        if self.known { Flagged::known(f(self.value)) } else { Flagged::unknown() }
    }

    /// `f` of three values, unknown when any is: `clamp` (§3.3.2).
    pub fn zip3<U: Copy + Default, V: Copy + Default, W: Copy + Default>(
        self,
        second: Flagged<U>,
        third: Flagged<V>,
        f: impl FnOnce(T, U, V) -> W,
    ) -> Flagged<W> {
        if self.known && second.known && third.known {
            Flagged::known(f(self.value, second.value, third.value))
        } else {
            Flagged::unknown()
        }
    }

    /// `f` of two values, unknown when either is: an all-or-nothing operator (§3.3.2).
    pub fn zip<U: Copy + Default, V: Copy + Default>(self, other: Flagged<U>, f: impl FnOnce(T, U) -> V) -> Flagged<V> {
        if self.known && other.known { Flagged::known(f(self.value, other.value)) } else { Flagged::unknown() }
    }
}

impl<T: Copy + Default + PartialEq> Flagged<T> {
    /// `a == b`: all-or-nothing (§3.3.10).
    pub fn equal(self, other: Flagged<T>) -> Flagged<bool> {
        self.zip(other, |a, b| a == b)
    }

    /// `a != b`: all-or-nothing.
    pub fn not_equal(self, other: Flagged<T>) -> Flagged<bool> {
        self.zip(other, |a, b| a != b)
    }
}

impl<T: Copy + Default + PartialOrd> Flagged<T> {
    /// `a < b`: all-or-nothing.
    pub fn less(self, other: Flagged<T>) -> Flagged<bool> {
        self.zip(other, |a, b| a < b)
    }

    /// `a <= b`: all-or-nothing.
    pub fn less_or_equal(self, other: Flagged<T>) -> Flagged<bool> {
        self.zip(other, |a, b| a <= b)
    }

    /// `a > b`: all-or-nothing.
    pub fn greater(self, other: Flagged<T>) -> Flagged<bool> {
        self.zip(other, |a, b| a > b)
    }

    /// `a >= b`: all-or-nothing.
    pub fn greater_or_equal(self, other: Flagged<T>) -> Flagged<bool> {
        self.zip(other, |a, b| a >= b)
    }
}

impl<T: Carried> Flagged<T> {
    /// The value of a pin's bytes: unknown when the known byte is 0 (abi.md P3).
    pub fn decode(bytes: &[u8]) -> Flagged<T> {
        if bytes.len() < T::SIZE {
            panic!("a pin of {} bytes read as a value of {} bytes", bytes.len(), T::SIZE);
        }
        if bytes[T::WIDTH] == 0 { Flagged::unknown() } else { Flagged::known(T::decode(bytes)) }
    }

    /// The pin's bytes: the value and the known byte 1, or every byte 0 when unknown (RS-11
    /// AC 4).
    pub fn encode(self) -> Vec<u8> {
        let mut bytes = vec![0u8; T::SIZE];
        if self.known {
            self.value.encode(&mut bytes);
            bytes[T::WIDTH] = 1;
        }
        bytes
    }
}

impl Flagged<bool> {
    /// `x.high()`: known and true.
    pub fn high(self) -> bool {
        self.known && self.value
    }

    /// The condition of an `if` at `origin`: its value, or a `Fault` naming the origin when it
    /// is unknown (RS-11 AC 5; the merge of §3.3.3 is out of scope).
    pub fn decide(self, origin: &str) -> Result<bool, Fault> {
        if self.known {
            Ok(self.value)
        } else {
            Err(Fault::new(Self::UNKNOWN_CONTROL, format!("{origin}: the condition of an `if` is unknown")))
        }
    }
}

impl<T: Copy + Default + Add<Output = T>> Add for Flagged<T> {
    type Output = Flagged<T>;
    fn add(self, rhs: Flagged<T>) -> Flagged<T> {
        self.zip(rhs, |a, b| a + b)
    }
}

impl<T: Copy + Default + Sub<Output = T>> Sub for Flagged<T> {
    type Output = Flagged<T>;
    fn sub(self, rhs: Flagged<T>) -> Flagged<T> {
        self.zip(rhs, |a, b| a - b)
    }
}

impl<T: Copy + Default + Mul<Output = T>> Mul for Flagged<T> {
    type Output = Flagged<T>;
    fn mul(self, rhs: Flagged<T>) -> Flagged<T> {
        self.zip(rhs, |a, b| a * b)
    }
}

impl<T: Copy + Default + Div<Output = T>> Div for Flagged<T> {
    type Output = Flagged<T>;
    fn div(self, rhs: Flagged<T>) -> Flagged<T> {
        self.zip(rhs, |a, b| a / b)
    }
}

impl<T: Copy + Default + Rem<Output = T>> Rem for Flagged<T> {
    type Output = Flagged<T>;
    fn rem(self, rhs: Flagged<T>) -> Flagged<T> {
        self.zip(rhs, |a, b| a % b)
    }
}

impl<T: Copy + Default + Neg<Output = T>> Neg for Flagged<T> {
    type Output = Flagged<T>;
    fn neg(self) -> Flagged<T> {
        self.map(|a| -a)
    }
}

impl<T: Copy + Default + Not<Output = T>> Not for Flagged<T> {
    type Output = Flagged<T>;
    fn not(self) -> Flagged<T> {
        self.map(|a| !a)
    }
}

impl<T: Levels + BitAnd<Output = T>> BitAnd for Flagged<T> {
    type Output = Flagged<T>;
    /// A known all-false operand gives known all-false, whatever the other (§3.3.2).
    fn bitand(self, rhs: Flagged<T>) -> Flagged<T> {
        let none = T::default();
        if (self.known && self.value == none) || (rhs.known && rhs.value == none) {
            return Flagged::known(none);
        }
        self.zip(rhs, |a, b| a & b)
    }
}

impl<T: Levels + BitOr<Output = T>> BitOr for Flagged<T> {
    type Output = Flagged<T>;
    /// A known all-true operand gives known all-true, whatever the other (§3.3.2).
    fn bitor(self, rhs: Flagged<T>) -> Flagged<T> {
        let all = T::ones();
        if (self.known && self.value == all) || (rhs.known && rhs.value == all) {
            return Flagged::known(all);
        }
        self.zip(rhs, |a, b| a | b)
    }
}

impl<T: Copy + Default + BitXor<Output = T>> BitXor for Flagged<T> {
    type Output = Flagged<T>;
    fn bitxor(self, rhs: Flagged<T>) -> Flagged<T> {
        self.zip(rhs, |a, b| a ^ b)
    }
}

/// A shift amount: any integer type, known or not.
pub trait Amount: Copy + Default {
    /// The amount, `None` when negative.
    fn amount(self) -> Option<u64>;
}

impl<const M: u32> Amount for UInt<M> {
    fn amount(self) -> Option<u64> {
        Some(self.get())
    }
}

impl<const M: u32> Amount for Int<M> {
    fn amount(self) -> Option<u64> {
        u64::try_from(self.get()).ok()
    }
}

impl<const N: u32, A: Amount> Shl<Flagged<A>> for Flagged<UInt<N>>
where
    UInt<N>: Shl<A, Output = UInt<N>>,
{
    type Output = Flagged<UInt<N>>;
    /// An unknown value shifted by `N` or more is known 0 (§3.3.11).
    fn shl(self, rhs: Flagged<A>) -> Flagged<UInt<N>> {
        match (self.known, rhs.known, rhs.value.amount()) {
            (false, true, Some(k)) if k >= u64::from(N) => Flagged::known(UInt::new(0)),
            _ => self.zip(rhs, |a, k| a << k),
        }
    }
}

impl<const N: u32, A: Amount> Shr<Flagged<A>> for Flagged<UInt<N>>
where
    UInt<N>: Shr<A, Output = UInt<N>>,
{
    type Output = Flagged<UInt<N>>;
    /// An unknown value shifted by `N` or more is known 0 (§3.3.11).
    fn shr(self, rhs: Flagged<A>) -> Flagged<UInt<N>> {
        match (self.known, rhs.known, rhs.value.amount()) {
            (false, true, Some(k)) if k >= u64::from(N) => Flagged::known(UInt::new(0)),
            _ => self.zip(rhs, |a, k| a >> k),
        }
    }
}

impl<const N: u32, A: Amount> Shl<Flagged<A>> for Flagged<Int<N>>
where
    Int<N>: Shl<A, Output = Int<N>>,
{
    type Output = Flagged<Int<N>>;
    /// An unknown value shifted by `N` or more is known 0 (§3.3.11).
    fn shl(self, rhs: Flagged<A>) -> Flagged<Int<N>> {
        match (self.known, rhs.known, rhs.value.amount()) {
            (false, true, Some(k)) if k >= u64::from(N) => Flagged::known(Int::new(0)),
            _ => self.zip(rhs, |a, k| a << k),
        }
    }
}

impl<const N: u32, A: Amount> Shr<Flagged<A>> for Flagged<Int<N>>
where
    Int<N>: Shr<A, Output = Int<N>>,
{
    type Output = Flagged<Int<N>>;
    /// A signed `>>` fills with the sign level and its flag: unknown stays unknown.
    fn shr(self, rhs: Flagged<A>) -> Flagged<Int<N>> {
        self.zip(rhs, |a, k| a >> k)
    }
}

/// The commits of one reaction, in the order they were made.
#[derive(Debug, Default)]
pub struct Commits {
    pending: Vec<(u32, Vec<u8>, f64)>,
}

impl Commits {
    pub fn new() -> Commits {
        Commits::default()
    }

    /// `carrier <- value after delay` through statement `statement`. An unknown delay commits
    /// `Unknown` at the next microstep (§3.3.3, RS-11 AC 6).
    pub fn commit<T: Carried>(&mut self, statement: u32, value: Flagged<T>, after: Flagged<f64>) {
        let (value, after) = if after.is_known() { (value, after.known_or(0.0)) } else { (Flagged::unknown(), 0.0) };
        self.pending.push((statement, value.encode(), after));
    }

    /// Hands every commit to `cx`, in order; the first refusal is the fault.
    pub fn flush(self, cx: &mut React<'_>) -> Result<(), Fault> {
        for (statement, bytes, after) in &self.pending {
            cx.commit(*statement, bytes, *after)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Commits, Flagged};
    use crate::rt::int::{Int, UInt};
    use piperine_abi::device::React;
    use piperine_abi::layout::{HIGH, LOW};
    use piperine_abi::raw::status;
    use piperine_ir::{Constant, Real, ValueType};
    use piperine_plan::CarrierLayout;

    type B = Flagged<bool>;

    fn k<T: Copy + Default>(v: T) -> Flagged<T> {
        Flagged::known(v)
    }

    fn u<T: Copy + Default>() -> Flagged<T> {
        Flagged::unknown()
    }

    #[test]
    fn an_unknown_operand_gives_an_unknown_result() {
        // RS-11 AC 1, §3.3.2: arithmetic, comparisons and `as` are all-or-nothing; one unknown
        // operand, on either side, makes the result unknown
        let (a, x) = (k(UInt::<9>::new(5)), u::<UInt<9>>());
        for (what, r) in [("+", a + x), ("+", x + a), ("-", a - x), ("*", x * a), ("/", a / x), ("%", x % a), ("^", a ^ x), ("unary -", -x), ("!", !x)] {
            assert_eq!(r, u(), "u9 {what}");
        }
        let (a, x) = (k(Int::<16>::new(-3)), u::<Int<16>>());
        for (what, r) in [("+", x + a), ("-", x - a), ("*", a * x), ("/", x / a), ("%", a % x), ("unary -", -x)] {
            assert_eq!(r, u(), "i16 {what}");
        }
        let (a, x) = (k(2.5f64), u::<f64>());
        for (what, r) in [("+", a + x), ("-", x - a), ("*", a * x), ("/", x / a), ("unary -", -x)] {
            assert_eq!(r, u(), "f64 {what}");
        }
        // an unknown operand is not evaluated: an unknown divisor is no division by zero, and a
        // known zero divisor under an unknown dividend is unknown, not a panic
        assert_eq!(k(UInt::<9>::new(5)) / u(), u());
        assert_eq!(u::<UInt<9>>() / k(UInt::<9>::new(0)), u());
        assert_eq!(u::<Int<8>>() % k(Int::<8>::new(0)), u());
        // comparisons (§3.3.10): unknown even when the known levels already differ
        let (a, x) = (k(UInt::<9>::new(5)), u::<UInt<9>>());
        for (what, r) in [("==", a.equal(x)), ("!=", x.not_equal(a)), ("<", a.less(x)), ("<=", x.less_or_equal(a)), (">", a.greater(x)), (">=", x.greater_or_equal(a))] {
            assert_eq!(r, u::<bool>(), "{what}");
        }
        // `as` and the methods of one operand
        assert_eq!(u::<UInt<9>>().map(|v| v.to_int::<4>()), u::<Int<4>>());
        assert_eq!(u::<f64>().map(UInt::<10>::from_f64), u::<UInt<10>>());
        assert_eq!(k(UInt::<9>::new(3)).zip(u::<UInt<9>>(), |a, b| a.min(b)), u());
        // with every operand known, the values compute as the integers do
        assert_eq!(k(UInt::<9>::new(511)) + k(UInt::<9>::new(1)), k(UInt::<9>::new(0)));
        assert_eq!(k(Int::<16>::new(-3)) * k(Int::<16>::new(4)), k(Int::<16>::new(-12)));
        assert_eq!(k(UInt::<9>::new(5)).less(k(UInt::<9>::new(7))), k(true));
        assert_eq!(k(2.5f64).map(Int::<8>::from_f64), k(Int::<8>::new(2)));
        // a shift by an unknown amount is all-or-nothing; by a known amount it moves levels,
        // so an unknown value shifted entirely out is known 0 (§3.3.11), and a signed `>>`
        // fills with the unknown sign
        assert_eq!(k(UInt::<9>::new(1)) << u::<Int<32>>(), u());
        assert_eq!(u::<UInt<9>>() << k(Int::<32>::new(2)), u());
        assert_eq!(u::<UInt<9>>() << k(Int::<32>::new(9)), k(UInt::<9>::new(0)));
        assert_eq!(u::<UInt<9>>() >> k(UInt::<8>::new(9)), k(UInt::<9>::new(0)));
        assert_eq!(u::<Int<8>>() << k(UInt::<8>::new(8)), k(Int::<8>::new(0)));
        assert_eq!(u::<Int<8>>() >> k(UInt::<8>::new(8)), u());
        assert_eq!(k(UInt::<3>::new(1)) << k(Int::<32>::new(2)), k(UInt::<3>::new(4)));
        // per level on integers: a known all-false `&` operand and a known all-true `|` operand
        // decide the result; anything else with an unknown operand is unknown
        assert_eq!(u::<UInt<9>>() & k(UInt::<9>::new(0)), k(UInt::<9>::new(0)));
        assert_eq!(k(UInt::<9>::new(511)) | u(), k(UInt::<9>::new(511)));
        assert_eq!(u::<Int<4>>() | k(Int::<4>::new(-1)), k(Int::<4>::new(-1)));
        assert_eq!(u::<UInt<9>>() & k(UInt::<9>::new(1)), u());
        assert_eq!(u::<UInt<9>>() | k(UInt::<9>::new(1)), u());
    }

    #[test]
    fn bool_operators_follow_the_level_rules() {
        // §3.3.2: `unknown & false == false`, `unknown | true == true`, every other case with an
        // unknown operand is unknown; the nine cases of `&` and `|`, the three of `!`
        let (f, t, x): (B, B, B) = (k(false), k(true), u());
        let and = [
            (f, f, f),
            (f, t, f),
            (f, x, f),
            (t, f, f),
            (t, t, t),
            (t, x, x),
            (x, f, f),
            (x, t, x),
            (x, x, x),
        ];
        for (a, b, want) in and {
            assert_eq!(a & b, want, "{a:?} & {b:?}");
        }
        let or = [
            (f, f, f),
            (f, t, t),
            (f, x, x),
            (t, f, t),
            (t, t, t),
            (t, x, t),
            (x, f, x),
            (x, t, t),
            (x, x, x),
        ];
        for (a, b, want) in or {
            assert_eq!(a | b, want, "{a:?} | {b:?}");
        }
        assert_eq!(!f, t);
        assert_eq!(!t, f);
        assert_eq!(!x, x);
        // `^` is all-or-nothing
        assert_eq!(x ^ f, x);
        assert_eq!(t ^ f, t);
    }

    #[test]
    fn known_or_is_known_and_high() {
        // RS-11 AC 3, §3.3.4: `known_or(d)` the value when known and `d` otherwise, `is_known()`
        // the flag, `high()` known and true
        assert_eq!(k(UInt::<8>::new(42)).known_or(UInt::new(7)), UInt::new(42));
        assert_eq!(u::<UInt<8>>().known_or(UInt::new(7)), UInt::new(7), "an uninitialised Bit<u8> read as known_or(7)");
        assert_eq!(k(0.0f64).known_or(1.0), 0.0);
        assert_eq!(u::<f64>().known_or(1.0), 1.0);
        assert!(k(false).is_known());
        assert!(!u::<bool>().is_known());
        assert!(k(Int::<4>::new(0)).is_known());
        assert!(k(true).high());
        assert!(!k(false).high());
        assert!(!u::<bool>().high());
        // the condition of an `if`: its value when known; unknown, a fault naming the origin
        // (RS-11 AC 5)
        assert_eq!(k(true).decide("control.msl:28:12"), Ok(true));
        assert_eq!(k(false).decide("control.msl:28:12"), Ok(false));
        let fault = u::<bool>().decide("control.msl:28:12").expect_err("an unknown condition is a fault");
        assert!(fault.message.contains("control.msl:28:12"), "{}", fault.message);
        assert!(!fault.reshape);
    }

    /// A reaction frame whose commits are recorded.
    fn react_with(f: impl FnOnce(&mut React<'_>)) -> Vec<(u32, Vec<u8>, f64)> {
        let mut made = Vec::new();
        {
            let mut sink = |statement: u32, bytes: &[u8], after: f64| {
                made.push((statement, bytes.to_vec(), after));
                status::OK
            };
            let mut cx = React::new(vec![], &mut sink);
            f(&mut cx);
        }
        made
    }

    #[test]
    fn an_unknown_value_is_written_as_zero_bytes() {
        // RS-11 AC 4: an unknown value is written with its known byte 0 and its value bytes 0,
        // in the layout of RS-12 AC 1
        assert_eq!(u::<bool>().encode(), [0, 0]);
        assert_eq!(u::<f64>().encode(), [0; 16]);
        assert_eq!(u::<UInt<1>>().encode(), [0, 0]);
        assert_eq!(u::<UInt<9>>().encode(), [0; 4]);
        assert_eq!(u::<Int<32>>().encode(), [0; 8]);
        assert_eq!(u::<UInt<64>>().encode(), [0; 16]);
        // a known value is the plan's canonical bytes for the same type and value
        assert_eq!(k(true).encode(), HIGH);
        assert_eq!(k(false).encode(), LOW);
        let plan = |ty: ValueType, c: Constant| CarrierLayout::bytes(&ty, c).expect("a value of the type");
        assert_eq!(k(1.5f64).encode(), plan(ValueType::F64, Constant::Real(Real::new(1.5))));
        assert_eq!(k(UInt::<9>::new(300)).encode(), plan(ValueType::UInt(9), Constant::Int(300)));
        assert_eq!(k(Int::<9>::new(-2)).encode(), plan(ValueType::Int(9), Constant::Int(-2)));
        assert_eq!(k(Int::<32>::new(-70000)).encode(), plan(ValueType::Int(32), Constant::Int(-70000)));
        assert_eq!(k(UInt::<64>::new(u64::MAX)).encode(), plan(ValueType::UInt(64), Constant::Int(i128::from(u64::MAX))));
        assert_eq!(k(Int::<1>::new(-1)).encode(), plan(ValueType::Int(1), Constant::Int(-1)));
        // and read back: zero bytes are unknown (§2 rule 11), the canonical bytes their value
        assert_eq!(Flagged::<UInt<9>>::decode(&[0, 0, 0, 0]), u());
        assert_eq!(Flagged::<UInt<9>>::decode(&[44, 1, 1, 0]), k(UInt::new(300)));
        assert_eq!(Flagged::<Int<9>>::decode(&[254, 255, 1, 0]), k(Int::new(-2)));
        assert_eq!(Flagged::<bool>::decode(&HIGH), k(true));
        assert_eq!(Flagged::<bool>::decode(&[0, 0]), u());
        assert_eq!(Flagged::<f64>::decode(&k(-0.25f64).encode()), k(-0.25));

        // through `Commits`: every commit reaches `React` in the order made, an unknown value as
        // zero bytes; an unknown delay commits `Unknown` at the next microstep (§3.3.3, RS-11
        // AC 6); nothing reaches `React` before `flush`
        let made = react_with(|cx| {
            let mut commits = Commits::new();
            commits.commit(2, k(UInt::<9>::new(300)), k(5e-9));
            commits.commit(0, u::<bool>(), k(0.0));
            commits.commit(1, k(true), u::<f64>());
            commits.commit(0, k(false), k(1e-9));
            commits.flush(cx).expect("every commit accepted");
        });
        assert_eq!(
            made,
            [(2, vec![44, 1, 1, 0], 5e-9), (0, vec![0, 0], 0.0), (1, vec![0, 0], 0.0), (0, LOW.to_vec(), 1e-9)]
        );
        let made = react_with(|_| {
            let mut commits = Commits::new();
            commits.commit(0, k(true), k(0.0));
            drop(commits);
        });
        assert_eq!(made, [], "a dropped `Commits`, as after a fault, commits nothing");
    }
}
