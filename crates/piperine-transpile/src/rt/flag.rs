//! `Flagged<T>`: the values of digital code (spec RS-11, MSL §3.3). The commits of a reaction
//! are `rt/commits.rs`.
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
//! `Merge` combines two values level by level, as the merge of unknown control does (§3.3.3,
//! §22.1.2); `same`, `level`, `recast` and `known_or_flagged` are the rest of §22.1 on one value.
//!

use core::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub};

use super::control::Panic;
use super::int::{Int, UInt};
use super::real::Real;

/// A type a pin carries: its value bytes little-endian at offset 0, then one known byte; the
/// layouts of the plan (`piperine_plan::CarrierLayout`, spec RS-12 AC 1).
pub trait Carried: Copy + Default + PartialEq + Levelwise {
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
    /// between types of the same level count (§22.1.3). A type of more than 64 levels has more
    /// than one word.
    fn to_levels(self) -> LevelWords;
    /// The value whose levels are the first `LEVELS` levels of `words`.
    fn from_levels(words: &LevelWords) -> Self;
}

/// The levels of a value as bits, level `k` being bit `k`, in as many 64-bit words as the value's
/// level count needs (§2): the form a composite wider than 64 levels moves between types, reads
/// a level of and compares. Bits at and above `len` are always 0, so two values of one type are
/// equal as words exactly when their levels are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelWords {
    words: Vec<u64>,
    len: u32,
}

impl LevelWords {
    /// No levels: the start of a concatenation.
    pub fn empty() -> LevelWords {
        LevelWords { words: Vec::new(), len: 0 }
    }

    /// The low `len` bits of `bits` as `len` levels (`len` at most 64).
    pub fn of(bits: u64, len: u32) -> LevelWords {
        let kept = if len >= 64 { bits } else { bits & ((1u64 << len) - 1) };
        LevelWords { words: if len == 0 { Vec::new() } else { vec![kept] }, len }
    }

    /// `len` levels, all false: the padding of a narrower payload and the payload of `None` (§2 rule 6).
    pub fn zeros(len: u32) -> LevelWords {
        LevelWords { words: vec![0; len.div_ceil(64) as usize], len }
    }

    /// How many levels.
    pub fn len(&self) -> u32 {
        self.len
    }

    /// Whether there are no levels.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Level `k`; a level at or past `len` is false.
    pub fn get(&self, k: u32) -> bool {
        k < self.len && self.words[(k / 64) as usize] >> (k % 64) & 1 != 0
    }

    /// The lowest 64 levels as one word: the whole value when it has at most 64.
    pub fn low(&self) -> u64 {
        self.words.first().copied().unwrap_or(0)
    }

    /// `self` followed by `other`: `other`'s level 0 becomes level `self.len()`.
    pub fn append(&mut self, other: &LevelWords) {
        let start = self.len;
        self.len += other.len;
        self.words.resize(self.len.div_ceil(64) as usize, 0);
        for k in 0..other.len {
            if other.get(k) {
                let at = start + k;
                self.words[(at / 64) as usize] |= 1 << (at % 64);
            }
        }
    }

    /// The parts joined in order, the first lowest.
    pub fn concat<const N: usize>(parts: [LevelWords; N]) -> LevelWords {
        let mut joined = LevelWords::empty();
        for part in &parts {
            joined.append(part);
        }
        joined
    }

    /// The `count` levels starting at level `from`, as their own value.
    pub fn slice(&self, from: u32, count: u32) -> LevelWords {
        let mut part = LevelWords { words: vec![0; count.div_ceil(64) as usize], len: count };
        for k in 0..count {
            if self.get(from + k) {
                part.words[(k / 64) as usize] |= 1 << (k % 64);
            }
        }
        part
    }
}

/// One field of a composite value read or written in order: the cursor of the byte form and of the
/// level form of a tuple, an array or an `Option`.
struct Field;

impl Field {
    /// Writes `value`'s bytes at `*at`, then moves `*at` past them.
    fn write<T: Carried>(value: T, out: &mut [u8], at: &mut usize) {
        value.encode(&mut out[*at..*at + T::WIDTH]);
        *at += T::WIDTH;
    }

    /// The value whose bytes start at `*at`; moves `*at` past them.
    fn read<T: Carried>(bytes: &[u8], at: &mut usize) -> T {
        let value = T::decode(&bytes[*at..*at + T::WIDTH]);
        *at += T::WIDTH;
        value
    }

    /// The value whose levels start at level `*at`; moves `*at` past them.
    fn levels<T: Carried>(words: &LevelWords, at: &mut u32) -> T {
        let value = T::from_levels(&words.slice(*at, T::LEVELS));
        *at += T::LEVELS;
        value
    }
}

/// A value two paths of an unknown `if` can be compared on (§3.3.3): the carried types, and the
/// reals of analog code (`f64`, `Dual<N>`), whose `Flagged` form a `cross` argument computes on.
pub trait Levelwise: Copy + Default {
    /// Whether `self` and `other` hold the same levels, a NaN being the same as itself (§3.3.9,
    /// §22.1.2 rule 2).
    fn same_levels(self, other: Self) -> bool;
}

/// A real is compared as `Real` says, its tangent included.
impl<R: Real> Levelwise for R {
    fn same_levels(self, other: R) -> bool {
        Real::same_levels(self, other)
    }
}

impl Levelwise for bool {
    fn same_levels(self, other: bool) -> bool {
        self == other
    }
}

impl<const N: u32> Levelwise for UInt<N> {
    fn same_levels(self, other: UInt<N>) -> bool {
        self.get() == other.get()
    }
}

impl<const N: u32> Levelwise for Int<N> {
    fn same_levels(self, other: Int<N>) -> bool {
        self.get() == other.get()
    }
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
    fn to_levels(self) -> LevelWords {
        LevelWords::of(u64::from(self), 1)
    }
    fn from_levels(words: &LevelWords) -> bool {
        words.get(0)
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
    fn to_levels(self) -> LevelWords {
        LevelWords::of(self.to_bits(), 64)
    }
    fn from_levels(words: &LevelWords) -> f64 {
        f64::from_bits(words.low())
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
    fn to_levels(self) -> LevelWords {
        LevelWords::of(self.get(), N)
    }
    fn from_levels(words: &LevelWords) -> UInt<N> {
        UInt::new(words.low())
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
    fn to_levels(self) -> LevelWords {
        // the low `N` bits of the two's complement value
        LevelWords::of((self.get() as u64) & UInt::<N>::MAX, N)
    }
    fn from_levels(words: &LevelWords) -> Int<N> {
        Int::new(words.low() as i64)
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

impl<T: Levelwise> Merge for Flagged<T> {
    /// One flag per value: known only when both are known and hold the same levels.
    fn merge(self, other: Flagged<T>) -> Flagged<T> {
        if self.known && other.known && self.value.same_levels(other.value) { self } else { Flagged::unknown() }
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

/// The written-local tuples of a merged `if` or loop merge field by field (§3.3.3: the effects of
/// a path are its value and each local it writes). One impl per arity, 1 to 12, written out: no
/// macro (AGENTS.md rule 5).
impl<A: Merge> Merge for (A,) {
    /// One field: its merge.
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0),)
    }

    fn unknown() -> Self {
        (A::unknown(),)
    }
}

impl<A: Merge, B: Merge> Merge for (A, B) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge> Merge for (A, B, C) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge> Merge for (A, B, C, D) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge> Merge for (A, B, C, D, E) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge, F: Merge> Merge for (A, B, C, D, E, F) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4), self.5.merge(other.5))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown(), F::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge, F: Merge, G: Merge> Merge for (A, B, C, D, E, F, G) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4), self.5.merge(other.5), self.6.merge(other.6))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown(), F::unknown(), G::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge, F: Merge, G: Merge, H: Merge> Merge for (A, B, C, D, E, F, G, H) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4), self.5.merge(other.5), self.6.merge(other.6), self.7.merge(other.7))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown(), F::unknown(), G::unknown(), H::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge, F: Merge, G: Merge, H: Merge, I: Merge> Merge for (A, B, C, D, E, F, G, H, I) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4), self.5.merge(other.5), self.6.merge(other.6), self.7.merge(other.7), self.8.merge(other.8))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown(), F::unknown(), G::unknown(), H::unknown(), I::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge, F: Merge, G: Merge, H: Merge, I: Merge, J: Merge> Merge for (A, B, C, D, E, F, G, H, I, J) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4), self.5.merge(other.5), self.6.merge(other.6), self.7.merge(other.7), self.8.merge(other.8), self.9.merge(other.9))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown(), F::unknown(), G::unknown(), H::unknown(), I::unknown(), J::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge, F: Merge, G: Merge, H: Merge, I: Merge, J: Merge, K: Merge> Merge for (A, B, C, D, E, F, G, H, I, J, K) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4), self.5.merge(other.5), self.6.merge(other.6), self.7.merge(other.7), self.8.merge(other.8), self.9.merge(other.9), self.10.merge(other.10))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown(), F::unknown(), G::unknown(), H::unknown(), I::unknown(), J::unknown(), K::unknown())
    }
}

impl<A: Merge, B: Merge, C: Merge, D: Merge, E: Merge, F: Merge, G: Merge, H: Merge, I: Merge, J: Merge, K: Merge, L: Merge> Merge for (A, B, C, D, E, F, G, H, I, J, K, L) {
    /// Field by field: each is the merge of the two fields (§3.3.3, a path's effects are each local
    /// it writes).
    fn merge(self, other: Self) -> Self {
        (self.0.merge(other.0), self.1.merge(other.1), self.2.merge(other.2), self.3.merge(other.3), self.4.merge(other.4), self.5.merge(other.5), self.6.merge(other.6), self.7.merge(other.7), self.8.merge(other.8), self.9.merge(other.9), self.10.merge(other.10), self.11.merge(other.11))
    }

    fn unknown() -> Self {
        (A::unknown(), B::unknown(), C::unknown(), D::unknown(), E::unknown(), F::unknown(), G::unknown(), H::unknown(), I::unknown(), J::unknown(), K::unknown(), L::unknown())
    }
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

    /// `a.merge(b)` on the bytes of two pins of type `T`: how `Commits` merges two committed
    /// values without knowing their type.
    pub fn merge_bytes(a: &[u8], b: &[u8]) -> Vec<u8> {
        Flagged::<T>::decode(a).merge(Flagged::<T>::decode(b)).encode()
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
        (Flagged::known(self.known), Flagged::known(self.known && self.value.to_levels().get(k)))
    }

    /// `cast::<T, U>(x)` (§22.1.3): the same levels under another type of the same level count,
    /// with their flag. `tags` lists the tags `U` decodes when it is an enum: any other
    /// pattern has no variant, and the result is `Unknown` (rule 2).
    pub fn recast<U: Carried>(self, tags: Option<&[i128]>) -> Flagged<U> {
        if !self.known {
            return Flagged::unknown();
        }
        let words = self.value.to_levels();
        match tags {
            // a tag is at most 64 levels: a value of more levels is read through its low word
            Some(tags) if !tags.contains(&i128::from(words.low())) => Flagged::unknown(),
            _ => Flagged::known(U::from_levels(&words)),
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

// ---- composite carried types (§2): tuples, arrays and `Option` ----------------------------------------------
//
// The value bytes of a composite are those of its parts one after another, the first lowest, then
// the one known byte of the whole value; its levels are the parts' levels in the same order.

impl<A: Carried> Carried for (A,) {
    const WIDTH: usize = A::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A,) {
        let mut at = 0;
        (Field::read(bytes, &mut at),)
    }
    const LEVELS: u32 = A::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A,) {
        let mut at = 0;
        (Field::levels(words, &mut at),)
    }
}

impl<A: Carried, B: Carried> Carried for (A, B) {
    const WIDTH: usize = A::WIDTH + B::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried> Carried for (A, B, C) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried> Carried for (A, B, C, D) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried> Carried for (A, B, C, D, E) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried> Carried for (A, B, C, D, E, F) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
        Field::write(self.5, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E, F) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS + F::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels(), self.5.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E, F) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried> Carried for (A, B, C, D, E, F, G) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
        Field::write(self.5, out, &mut at);
        Field::write(self.6, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E, F, G) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS + F::LEVELS + G::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels(), self.5.to_levels(), self.6.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E, F, G) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried> Carried for (A, B, C, D, E, F, G, H) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
        Field::write(self.5, out, &mut at);
        Field::write(self.6, out, &mut at);
        Field::write(self.7, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E, F, G, H) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS + F::LEVELS + G::LEVELS + H::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels(), self.5.to_levels(), self.6.to_levels(), self.7.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E, F, G, H) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried> Carried for (A, B, C, D, E, F, G, H, I) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
        Field::write(self.5, out, &mut at);
        Field::write(self.6, out, &mut at);
        Field::write(self.7, out, &mut at);
        Field::write(self.8, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E, F, G, H, I) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS + F::LEVELS + G::LEVELS + H::LEVELS + I::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels(), self.5.to_levels(), self.6.to_levels(), self.7.to_levels(), self.8.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E, F, G, H, I) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried, J: Carried> Carried for (A, B, C, D, E, F, G, H, I, J) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH + J::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
        Field::write(self.5, out, &mut at);
        Field::write(self.6, out, &mut at);
        Field::write(self.7, out, &mut at);
        Field::write(self.8, out, &mut at);
        Field::write(self.9, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E, F, G, H, I, J) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS + F::LEVELS + G::LEVELS + H::LEVELS + I::LEVELS + J::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels(), self.5.to_levels(), self.6.to_levels(), self.7.to_levels(), self.8.to_levels(), self.9.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E, F, G, H, I, J) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried, J: Carried, K: Carried> Carried for (A, B, C, D, E, F, G, H, I, J, K) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH + J::WIDTH + K::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
        Field::write(self.5, out, &mut at);
        Field::write(self.6, out, &mut at);
        Field::write(self.7, out, &mut at);
        Field::write(self.8, out, &mut at);
        Field::write(self.9, out, &mut at);
        Field::write(self.10, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E, F, G, H, I, J, K) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS + F::LEVELS + G::LEVELS + H::LEVELS + I::LEVELS + J::LEVELS + K::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels(), self.5.to_levels(), self.6.to_levels(), self.7.to_levels(), self.8.to_levels(), self.9.to_levels(), self.10.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E, F, G, H, I, J, K) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried, J: Carried, K: Carried, L: Carried> Carried for (A, B, C, D, E, F, G, H, I, J, K, L) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH + J::WIDTH + K::WIDTH + L::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        Field::write(self.0, out, &mut at);
        Field::write(self.1, out, &mut at);
        Field::write(self.2, out, &mut at);
        Field::write(self.3, out, &mut at);
        Field::write(self.4, out, &mut at);
        Field::write(self.5, out, &mut at);
        Field::write(self.6, out, &mut at);
        Field::write(self.7, out, &mut at);
        Field::write(self.8, out, &mut at);
        Field::write(self.9, out, &mut at);
        Field::write(self.10, out, &mut at);
        Field::write(self.11, out, &mut at);
    }
    fn decode(bytes: &[u8]) -> (A, B, C, D, E, F, G, H, I, J, K, L) {
        let mut at = 0;
        (Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at), Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = A::LEVELS + B::LEVELS + C::LEVELS + D::LEVELS + E::LEVELS + F::LEVELS + G::LEVELS + H::LEVELS + I::LEVELS + J::LEVELS + K::LEVELS + L::LEVELS;
    fn to_levels(self) -> LevelWords {
        LevelWords::concat([self.0.to_levels(), self.1.to_levels(), self.2.to_levels(), self.3.to_levels(), self.4.to_levels(), self.5.to_levels(), self.6.to_levels(), self.7.to_levels(), self.8.to_levels(), self.9.to_levels(), self.10.to_levels(), self.11.to_levels()])
    }
    fn from_levels(words: &LevelWords) -> (A, B, C, D, E, F, G, H, I, J, K, L) {
        let mut at = 0;
        (Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at), Field::levels(words, &mut at))
    }
}

impl<A: Levelwise> Levelwise for (A,) {
    fn same_levels(self, other: (A,)) -> bool {
        self.0.same_levels(other.0)
    }
}

impl<A: Levelwise, B: Levelwise> Levelwise for (A, B) {
    fn same_levels(self, other: (A, B)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise> Levelwise for (A, B, C) {
    fn same_levels(self, other: (A, B, C)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise> Levelwise for (A, B, C, D) {
    fn same_levels(self, other: (A, B, C, D)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise> Levelwise for (A, B, C, D, E) {
    fn same_levels(self, other: (A, B, C, D, E)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise, F: Levelwise> Levelwise for (A, B, C, D, E, F) {
    fn same_levels(self, other: (A, B, C, D, E, F)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4) && self.5.same_levels(other.5)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise, F: Levelwise, G: Levelwise> Levelwise for (A, B, C, D, E, F, G) {
    fn same_levels(self, other: (A, B, C, D, E, F, G)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4) && self.5.same_levels(other.5) && self.6.same_levels(other.6)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise, F: Levelwise, G: Levelwise, H: Levelwise> Levelwise for (A, B, C, D, E, F, G, H) {
    fn same_levels(self, other: (A, B, C, D, E, F, G, H)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4) && self.5.same_levels(other.5) && self.6.same_levels(other.6) && self.7.same_levels(other.7)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise, F: Levelwise, G: Levelwise, H: Levelwise, I: Levelwise> Levelwise for (A, B, C, D, E, F, G, H, I) {
    fn same_levels(self, other: (A, B, C, D, E, F, G, H, I)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4) && self.5.same_levels(other.5) && self.6.same_levels(other.6) && self.7.same_levels(other.7) && self.8.same_levels(other.8)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise, F: Levelwise, G: Levelwise, H: Levelwise, I: Levelwise, J: Levelwise> Levelwise for (A, B, C, D, E, F, G, H, I, J) {
    fn same_levels(self, other: (A, B, C, D, E, F, G, H, I, J)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4) && self.5.same_levels(other.5) && self.6.same_levels(other.6) && self.7.same_levels(other.7) && self.8.same_levels(other.8) && self.9.same_levels(other.9)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise, F: Levelwise, G: Levelwise, H: Levelwise, I: Levelwise, J: Levelwise, K: Levelwise> Levelwise for (A, B, C, D, E, F, G, H, I, J, K) {
    fn same_levels(self, other: (A, B, C, D, E, F, G, H, I, J, K)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4) && self.5.same_levels(other.5) && self.6.same_levels(other.6) && self.7.same_levels(other.7) && self.8.same_levels(other.8) && self.9.same_levels(other.9) && self.10.same_levels(other.10)
    }
}

impl<A: Levelwise, B: Levelwise, C: Levelwise, D: Levelwise, E: Levelwise, F: Levelwise, G: Levelwise, H: Levelwise, I: Levelwise, J: Levelwise, K: Levelwise, L: Levelwise> Levelwise for (A, B, C, D, E, F, G, H, I, J, K, L) {
    fn same_levels(self, other: (A, B, C, D, E, F, G, H, I, J, K, L)) -> bool {
        self.0.same_levels(other.0) && self.1.same_levels(other.1) && self.2.same_levels(other.2) && self.3.same_levels(other.3) && self.4.same_levels(other.4) && self.5.same_levels(other.5) && self.6.same_levels(other.6) && self.7.same_levels(other.7) && self.8.same_levels(other.8) && self.9.same_levels(other.9) && self.10.same_levels(other.10) && self.11.same_levels(other.11)
    }
}

/// An array is carried where Rust gives it a `Default` (up to 32 elements): element 0 is the
/// lowest.
impl<T: Carried, const N: usize> Carried for [T; N]
where
    [T; N]: Default,
{
    const WIDTH: usize = N * T::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        let mut at = 0;
        for element in self {
            Field::write(element, out, &mut at);
        }
    }
    fn decode(bytes: &[u8]) -> [T; N] {
        let mut at = 0;
        std::array::from_fn(|_| Field::read(bytes, &mut at))
    }
    const LEVELS: u32 = N as u32 * T::LEVELS;
    fn to_levels(self) -> LevelWords {
        let mut joined = LevelWords::empty();
        for element in self {
            joined.append(&element.to_levels());
        }
        joined
    }
    fn from_levels(words: &LevelWords) -> [T; N] {
        let mut at = 0;
        std::array::from_fn(|_| Field::levels(words, &mut at))
    }
}

impl<T: Levelwise, const N: usize> Levelwise for [T; N]
where
    [T; N]: Default,
{
    fn same_levels(self, other: [T; N]) -> bool {
        self.iter().zip(other).all(|(a, b)| a.same_levels(b))
    }
}

/// `Option<T>`: level 0 is the tag, true for `Some`, then `T`'s levels (all false for `None`); the
/// bytes are a tag byte, then `T`'s value bytes (all zero for `None`).
impl<T: Carried> Carried for Option<T> {
    const WIDTH: usize = 1 + T::WIDTH;
    const SIZE: usize = Self::WIDTH + 1;
    fn encode(self, out: &mut [u8]) {
        out[..Self::WIDTH].fill(0);
        if let Some(value) = self {
            out[0] = 1;
            value.encode(&mut out[1..Self::WIDTH]);
        }
    }
    fn decode(bytes: &[u8]) -> Option<T> {
        (bytes[0] != 0).then(|| T::decode(&bytes[1..Self::WIDTH]))
    }
    const LEVELS: u32 = 1 + T::LEVELS;
    fn to_levels(self) -> LevelWords {
        match self {
            Some(value) => LevelWords::concat([LevelWords::of(1, 1), value.to_levels()]),
            None => LevelWords::concat([LevelWords::of(0, 1), LevelWords::zeros(T::LEVELS)]),
        }
    }
    fn from_levels(words: &LevelWords) -> Option<T> {
        words.get(0).then(|| T::from_levels(&words.slice(1, T::LEVELS)))
    }
}

impl<T: Levelwise> Levelwise for Option<T> {
    fn same_levels(self, other: Option<T>) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.same_levels(b),
            (None, None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Carried, Flagged, LevelWords, Merge};
    use crate::rt::commits::{Commits, tests::react_with};
    use crate::rt::int::{Int, UInt};
    use piperine_abi::layout::{HIGH, LOW};
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
    fn composite_levels_follow_the_section_2_table() {
        // §2 table: a tuple concatenates its fields, `.0` lowest; an array its elements, element 0
        // lowest; `Option<T>` has the tag at level 0 (true for `Some`), then `T`
        let pair = (UInt::<3>::new(0b101), true);
        assert_eq!(<(UInt<3>, bool)>::LEVELS, 4);
        assert_eq!(pair.to_levels(), LevelWords::of(0b1101, 4), "the tuple: .0 in levels 0 to 2, then the bool");
        assert_eq!(<(UInt<3>, bool)>::from_levels(&LevelWords::of(0b1101, 4)), pair);
        let elements = [UInt::<2>::new(1), UInt::<2>::new(2), UInt::<2>::new(3)];
        assert_eq!(elements.to_levels(), LevelWords::of(0b11_10_01, 6), "the array: element 0 lowest");
        assert_eq!(<[UInt<2>; 3]>::from_levels(&LevelWords::of(0b11_10_01, 6)), elements);
        assert_eq!(Some(UInt::<3>::new(0b101)).to_levels(), LevelWords::of(0b1011, 4), "Some: the tag true, then the payload");
        assert_eq!(None::<UInt<3>>.to_levels(), LevelWords::zeros(4), "None: the tag false and the payload all false");
        assert_eq!(<Option<UInt<3>>>::from_levels(&LevelWords::of(0b1011, 4)), Some(UInt::<3>::new(0b101)));
        assert_eq!(<Option<UInt<3>>>::from_levels(&LevelWords::of(0b1010, 4)), None, "a clear tag is None whatever the payload levels");

        // more than 64 levels: a real and a byte are 72 levels in two words, read and moved by level
        let wide = (-2.5f64, UInt::<8>::new(0b1000_0001));
        let levels = wide.to_levels();
        assert_eq!(levels.len(), 72);
        assert_eq!(levels.low(), (-2.5f64).to_bits());
        assert!(levels.get(64) && !levels.get(65) && levels.get(71), "the byte sits in levels 64 to 71");
        assert_eq!(<(f64, UInt<8>)>::from_levels(&levels), wide);
        let (is, value) = k(wide).level(k(UInt::<32>::new(71)), "t.msl:1:1");
        assert_eq!((is, value), (k(true), k(true)), "`level` reads past the first word");
        assert_eq!(k(wide).level(k(UInt::<32>::new(65)), "t.msl:1:1").1, k(false));
        assert!(k(wide).same(k(wide)) && !k(wide).same(k((-2.5f64, UInt::<8>::new(0b1000_0011)))), "`same` compares both words");

        // the bytes: the parts' value bytes one after another, then the known byte of the whole value
        assert_eq!(<(UInt<3>, bool)>::WIDTH, 2);
        assert_eq!(k(pair).encode(), vec![0b101, 1, 1]);
        assert_eq!(Flagged::<(UInt<3>, bool)>::decode(&[0b101, 1, 1]), k(pair));
        assert_eq!(Flagged::<(UInt<3>, bool)>::decode(&[0, 0, 0]), u());
        assert_eq!(k(elements).encode(), vec![1, 2, 3, 1]);
        assert_eq!(Flagged::<Option<UInt<3>>>::decode(&k(Some(UInt::<3>::new(6))).encode()), k(Some(UInt::<3>::new(6))));
        assert_eq!(k(None::<UInt<3>>).encode(), vec![0, 0, 1]);
        let mut padded = [9u8, 9];
        None::<UInt<3>>.encode(&mut padded);
        assert_eq!(padded, [0, 0], "None encodes its payload as zeros");

        // a composite is one value with one flag: a merge keeps it only when both are known and equal; `cast`
        // moves the levels under another type of four levels
        assert_eq!(k(pair).merge(k(pair)), k(pair));
        assert_eq!(k(pair).merge(k((UInt::<3>::new(5), false))), u());
        assert_eq!(k(pair).recast::<UInt<4>>(None), k(UInt::<4>::new(0b1101)));
        assert_eq!(k(UInt::<4>::new(0b1101)).recast::<(UInt<3>, bool)>(None), k(pair));
        assert_eq!(k(Some(UInt::<3>::new(2))).merge(k(None)), u(), "Some and None differ in the tag level");
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
    fn a_real_with_its_tangent_merges_as_one_value() {
        // §3.3.3, §22.1.2: the arms of an unknown `if` in a `cross` argument yield `Flagged<Dual>`,
        // which merges as `Real::same_levels` says: the value and every partial must agree, a NaN
        // being the same as itself
        use crate::rt::real::Dual;
        let d = |v: f64, d0: f64| k(Dual::<2> { v, d: [d0, 1.0] });
        assert_eq!(d(2.0, 3.0).merge(d(2.0, 3.0)), d(2.0, 3.0), "equal value and tangent are kept");
        assert_eq!(d(2.0, 3.0).merge(d(2.0, 4.0)), u(), "a different tangent is a different value");
        assert_eq!(d(2.0, 3.0).merge(d(5.0, 3.0)), u());
        assert!(d(f64::NAN, 3.0).merge(d(f64::NAN, 3.0)).is_known(), "NaN merges with itself");
        assert_eq!(d(2.0, 3.0).merge(u()), u());
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
    fn tuples_merge_field_by_field() {
        // §3.3.3: the effects of a path are each local it writes; two tuples merge as each of their
        // fields does. Arity 1, 2 and 12 here; the impls of the others are the same text
        let (five, six): (Flagged<UInt<8>>, Flagged<UInt<8>>) = (k(UInt::new(5)), k(UInt::new(6)));
        let unknown = Flagged::<UInt<8>>::unknown();
        assert_eq!((five,).merge((five,)), (five,));
        assert_eq!((five,).merge((six,)), (unknown,));
        assert_eq!((five, five).merge((five, six)), (five, unknown));
        let a = (five, five, five, five, five, five, five, five, five, five, five, five);
        let b = (five, six, five, six, five, six, five, six, five, six, five, six);
        let merged = a.merge(b);
        assert_eq!(merged, (five, unknown, five, unknown, five, unknown, five, unknown, five, unknown, five, unknown));
        assert_eq!(<(Flagged<UInt<8>>, Flagged<bool>)>::unknown(), (unknown, Flagged::<bool>::unknown()));
    }
}
