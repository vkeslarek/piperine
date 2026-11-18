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
//! `Merge` combines two values level by level, as the merge of unknown control does (§3.3.3,
//! §22.1.2); `same`, `level`, `recast` and `known_or_flagged` are the rest of §22.1 on one value.
//!
//! `Commits` collects the commits of one reaction and hands them to `React` in the order they
//! were made, after the body ran, so a fault makes none (design "Phase B: generated digital
//! code"). A commit whose delay is unknown commits `Unknown` at the next microstep (§3.3.3).

use core::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

use piperine_abi::device::{Fault, React};

use super::control::Panic;
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
    /// How many levels the type has (§2): the `T::WIDTH` of MSL, not the byte width above.
    const LEVELS: u32;
    /// The levels as bits, level `k` being bit `k`: the form `level` reads and `cast` moves
    /// between types of the same level count (§22.1.3).
    fn to_levels(self) -> u64;
    /// The value whose levels are the low `LEVELS` bits of `bits`.
    fn from_levels(bits: u64) -> Self;
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
    const LEVELS: u32 = 1;
    fn to_levels(self) -> u64 {
        u64::from(self)
    }
    fn from_levels(bits: u64) -> bool {
        bits & 1 != 0
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
    const LEVELS: u32 = 64;
    fn to_levels(self) -> u64 {
        self.to_bits()
    }
    fn from_levels(bits: u64) -> f64 {
        f64::from_bits(bits)
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
    const LEVELS: u32 = N;
    fn to_levels(self) -> u64 {
        self.get()
    }
    fn from_levels(bits: u64) -> UInt<N> {
        UInt::new(bits)
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
    const LEVELS: u32 = N;
    fn to_levels(self) -> u64 {
        // the low `N` bits of the two's complement value
        (self.get() as u64) & UInt::<N>::MAX
    }
    fn from_levels(bits: u64) -> Int<N> {
        Int::new(bits as i64)
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

    /// Writes the pin's bytes (`encode`) at the start of `out`, the buffer of a write cell.
    pub fn write(self, out: &mut [u8]) {
        let bytes = self.encode();
        out[..bytes.len()].copy_from_slice(&bytes);
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

/// Two values that merge level by level (§3.3.3, §22.1.2): the value of an unknown `if` is the
/// merge of what its arms yield.
pub trait Merge: Sized {
    /// Equal known levels kept, the rest unknown. Commutative and associative, and
    /// `a.merge(a)` is `a` (§22.1.2 rules 1 and 2).
    fn merge(self, other: Self) -> Self;
    /// The value every level of which is unknown: what a path that panicked yields under
    /// unknown control (§1 rule 13).
    fn unknown() -> Self;
}

impl<T: Carried> Merge for Flagged<T> {
    /// One flag per value: known only when both are known and hold the same levels.
    fn merge(self, other: Flagged<T>) -> Flagged<T> {
        if self.known && other.known && self.value.to_levels() == other.value.to_levels() { self } else { Flagged::unknown() }
    }

    fn unknown() -> Flagged<T> {
        Flagged { value: T::default(), known: false }
    }
}

impl Merge for () {
    /// A statement yields nothing to merge.
    fn merge(self, _other: ()) {}

    fn unknown() {}
}

impl<T: Carried> Flagged<T> {
    /// `same(a, b)` (§22.1.4): every level has the same flag and, where known, the same value.
    /// Always known, so a plain `bool`; two unknown values are the same, unlike `==` (§3.3.10).
    pub fn same(self, other: Flagged<T>) -> bool {
        match (self.known, other.known) {
            (true, true) => self.value.to_levels() == other.value.to_levels(),
            (false, false) => true,
            _ => false,
        }
    }

    /// `x.level(k)` as the pair `(is, value)` of the `Option<bool>` it yields (§3.3.4): `is`
    /// says the level is known, `value` is its value (`false` when it is not, the form `None`
    /// takes). With a known `k` both are known (§3.3.14) and a `k` outside the type's levels
    /// is the panic of §1 rule 11 naming `origin` and `k`. An unknown `k` is an unknown index:
    /// the merge of the pair of every level (§3.3.13).
    pub fn level(self, index: Flagged<UInt<32>>, origin: &str) -> (Flagged<bool>, Flagged<bool>) {
        if !index.known {
            return (0..T::LEVELS)
                .map(|k| self.level_at(k))
                .reduce(|(is, value), (next_is, next_value)| (is.merge(next_is), value.merge(next_value)))
                .unwrap_or((Flagged::unknown(), Flagged::unknown()));
        }
        let k = index.value.get();
        if k >= u64::from(T::LEVELS) {
            Panic::raise(format!("{origin}: level {k} is out of range for a value of {} levels", T::LEVELS));
        }
        self.level_at(k as u32)
    }

    /// Level `k` (below `T::LEVELS`) as the known pair of `level`.
    fn level_at(self, k: u32) -> (Flagged<bool>, Flagged<bool>) {
        (Flagged::known(self.known), Flagged::known(self.known && (self.value.to_levels() >> k) & 1 != 0))
    }

    /// `cast::<T, U>(x)` (§22.1.3): the same levels under another type of the same level count,
    /// with their flag. `tags` lists the tags `U` decodes when it is an enum: any other
    /// pattern has no variant, and the result is `Unknown` (rule 2).
    pub fn recast<U: Carried>(self, tags: Option<&[i128]>) -> Flagged<U> {
        if !self.known {
            return Flagged::unknown();
        }
        let bits = self.value.to_levels();
        match tags {
            Some(tags) if !tags.contains(&i128::from(bits)) => Flagged::unknown(),
            _ => Flagged::known(U::from_levels(bits)),
        }
    }
}

impl<T: Copy + Default> Flagged<T> {
    /// `x.known_or(d)` where `d` may itself be unknown (§22.1.1 rule 4): `x` when known,
    /// otherwise `d` with its own flag.
    pub fn known_or_flagged(self, default: Flagged<T>) -> Flagged<T> {
        if self.known { self } else { default }
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

    /// A record through statement `statement`: `bytes`, the whole layout of its stream's
    /// `RECORD` pin, due at the next microstep (abi.md C5: a record is a commit to that pin).
    pub fn record(&mut self, statement: u32, bytes: &[u8]) {
        self.pending.push((statement, bytes.to_vec(), 0.0));
    }

    /// Arms a timer through statement `statement`, a `REPLACE` statement on an event pin (abi.md
    /// R1, C3), for the time `at` seen at the instant `now`: a later time is due `at - now`
    /// from now, the time of the instant itself at the next microstep (a delay of 0, C1), and an
    /// unknown or past time never (`+inf`, C1). Since the statement is `REPLACE`, each arming
    /// removes the firing an earlier one left pending, so a timer never fires for a time it no
    /// longer holds.
    pub fn arm(&mut self, statement: u32, at: Flagged<f64>, now: f64) {
        let after = match at.is_known() {
            true if at.value > now => at.value - now,
            true if at.value == now => 0.0,
            _ => f64::INFINITY,
        };
        self.pending.push((statement, Vec::new(), after));
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
    use super::{Commits, Flagged, Merge};
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

    /// The message of the panic `f` raises, `None` when it does not panic.
    fn panic_message(f: impl FnOnce() + std::panic::UnwindSafe) -> Option<String> {
        std::panic::catch_unwind(f).err().and_then(|e| e.downcast_ref::<String>().cloned())
    }

    #[test]
    fn merge_follows_the_level_rule() {
        // §22.1.2 rule 1 (one flag per value): known exactly when both are known and equal,
        // and then that value; any other pair is unknown
        assert_eq!(k(true).merge(k(true)), k(true));
        assert_eq!(k(true).merge(k(false)), u());
        assert_eq!(k(true).merge(u()), u());
        assert_eq!(u::<bool>().merge(u()), u());
        assert_eq!(k(UInt::<8>::new(5)).merge(k(UInt::<8>::new(5))), k(UInt::<8>::new(5)));
        assert_eq!(k(UInt::<8>::new(5)).merge(k(UInt::<8>::new(6))), u());
        assert_eq!(k(Int::<4>::new(-1)).merge(k(Int::<4>::new(-1))), k(Int::<4>::new(-1)));
        // rule 2: commutative, and `merge(a, a)` has the levels and flags of `a`, a NaN
        // included (it is one known value, §3.3.9)
        let values = [k(1.5f64), k(2.5), u(), k(f64::NAN)];
        for a in values {
            for b in values {
                assert_eq!(a.merge(b).is_known(), b.merge(a).is_known(), "{a:?} {b:?}");
            }
            assert_eq!(a.merge(a).is_known(), a.is_known(), "{a:?}");
        }
        assert!(k(f64::NAN).merge(k(f64::NAN)).is_known());
        // rule 2: associative
        let (a, b, c) = (k(true), k(true), k(false));
        assert_eq!(a.merge(b).merge(c), a.merge(b.merge(c)));
        // a statement yields nothing, so there is nothing to merge
        assert_eq!(().merge(()), ());
        assert_eq!(<Flagged<bool> as Merge>::unknown(), u());
    }

    #[test]
    fn same_compares_flags() {
        // §22.1.4 rule 1: `same` is always known and compares flags; `==` is unknown on an
        // unknown operand (§3.3.10)
        assert!(u::<UInt<8>>().same(u()), "same(Unknown, Unknown) is true");
        assert_eq!(u::<UInt<8>>().equal(u()), u::<bool>(), "Unknown == Unknown is unknown");
        assert!(!k(UInt::<8>::new(0)).same(u()));
        assert!(!u::<bool>().same(k(false)));
        assert!(k(UInt::<8>::new(9)).same(k(UInt::new(9))));
        assert!(!k(UInt::<8>::new(9)).same(k(UInt::new(8))));
        assert!(k(Int::<8>::new(-3)).same(k(Int::new(-3))));
        assert!(k(f64::NAN).same(k(f64::NAN)), "a NaN is one known value");
    }

    #[test]
    fn level_reads_a_level_and_merges_an_unknown_index() {
        let at = |x: Flagged<UInt<8>>, i: u64| x.level(k(UInt::<32>::new(i)), "t.msl:1:1");
        // §3.3.4, US-02 AC 1: `Some(b)` for a known level, `None` for an unknown value, and the
        // pair itself known whatever the flags of `x` (§3.3.14, AC 2)
        let x = k(UInt::<8>::new(0b1010_0101));
        for i in 0..8 {
            assert_eq!(at(x, i), (k(true), k((0b1010_0101 >> i) & 1 == 1)), "level {i}");
            assert_eq!(at(u(), i), (k(false), k(false)), "level {i} of an unknown value is None");
        }
        assert_eq!(k(true).level(k(UInt::new(0)), "t"), (k(true), k(true)));
        assert_eq!(u::<bool>().level(k(UInt::new(0)), "t"), (k(false), k(false)));
        assert_eq!(k(Int::<4>::new(-8)).level(k(UInt::new(3)), "t"), (k(true), k(true)), "the sign level");
        assert_eq!(k(-1.0f64).level(k(UInt::new(63)), "t"), (k(true), k(true)), "f64 has 64 levels");
        // AC 4, §1 rule 11: a known `k` not below the level count is a panic naming the
        // origin and `k`
        let message = panic_message(|| {
            let _ = at(k(UInt::<8>::new(1)), 8);
        })
        .expect("level 8 of a u8 panics");
        assert!(message.contains("t.msl:1:1") && message.contains('8'), "{message}");
        assert!(panic_message(|| {
            let _ = k(true).level(k(UInt::new(1)), "t");
        })
        .is_some());
        // AC 5, §3.3.13: an unknown index is the merge of every level: known only when every
        // level agrees
        let any = u::<UInt<32>>();
        assert_eq!(k(UInt::<8>::new(0xFF)).level(any, "t"), (k(true), k(true)));
        assert_eq!(k(UInt::<8>::new(0)).level(any, "t"), (k(true), k(false)));
        assert_eq!(k(UInt::<8>::new(1)).level(any, "t"), (k(true), u()));
        assert_eq!(u::<UInt<8>>().level(any, "t"), (k(false), k(false)), "every level is None");
    }

    #[test]
    fn recast_keeps_levels_and_decodes_tags() {
        // §22.1.3 rule 1: level `k` of the result is level `k` of the operand, flag included
        assert_eq!(k(UInt::<4>::new(0b1011)).recast::<Int<4>>(None), k(Int::<4>::new(-5)));
        assert_eq!(k(Int::<4>::new(-1)).recast::<UInt<4>>(None), k(UInt::<4>::new(15)));
        assert_eq!(k(true).recast::<UInt<1>>(None), k(UInt::<1>::new(1)));
        assert_eq!(k(UInt::<1>::new(0)).recast::<bool>(None), k(false));
        assert_eq!(k(1.0f64).recast::<UInt<64>>(None), k(UInt::<64>::new(1.0f64.to_bits())));
        assert_eq!(u::<UInt<4>>().recast::<Int<4>>(None), u(), "an unknown value stays unknown");
        // rule 2: a tag the target has no variant for is `Unknown`
        let tags = [0, 1, 2];
        assert_eq!(k(UInt::<2>::new(2)).recast::<UInt<2>>(Some(&tags)), k(UInt::<2>::new(2)));
        assert_eq!(k(UInt::<2>::new(3)).recast::<UInt<2>>(Some(&tags)), u());
        assert_eq!(u::<UInt<2>>().recast::<UInt<2>>(Some(&tags)), u());
    }

    #[test]
    fn known_or_flagged_gives_the_default_with_its_flag() {
        // §22.1.1 rule 4: `x` when known, otherwise `d` with its own flag
        assert_eq!(k(1.0f64).known_or_flagged(k(2.0)), k(1.0));
        assert_eq!(k(1.0f64).known_or_flagged(u()), k(1.0));
        assert_eq!(u::<f64>().known_or_flagged(k(2.0)), k(2.0));
        assert_eq!(u::<f64>().known_or_flagged(u()), u());
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

    #[test]
    fn a_timer_is_armed_for_a_later_time_only() {
        // msl-digital A3 on abi.md C1, C4: an arming fires its event pin (no bytes) `t - now`
        // later, at the next microstep for `t == now`, and never (`+inf`) for a past or unknown
        // time
        let made = react_with(|cx| {
            let mut commits = Commits::new();
            commits.arm(0, k(15e-9), 5e-9);
            commits.arm(1, k(5e-9), 5e-9);
            commits.arm(2, k(0.0), 5e-9);
            commits.arm(3, u::<f64>(), 5e-9);
            commits.flush(cx).expect("every arming accepted");
        });
        assert_eq!(made, [(0, vec![], 15e-9 - 5e-9), (1, vec![], 0.0), (2, vec![], f64::INFINITY), (3, vec![], f64::INFINITY)]);
    }
}
