//! `Carried`: the types a pin carries (spec RS-11, abi.md P3), their byte form and their level
//! form. The commits of a reaction are `rt/commits.rs`.
//!
//! A pin holds the value bytes of its type, little-endian at offset 0, and nothing else (abi.md
//! P3: a value is its bytes). The level form (`to_levels`, `from_levels`) is what `same` compares
//! and `cast` moves between types of one level count (§22.1).

use super::int::{Int, UInt};

/// A type a pin carries: its value bytes little-endian at offset 0; the layouts of the plan (`piperine_plan::CarrierLayout`, spec RS-12 AC 1).
pub trait Carried: Copy + PartialEq {
    /// The width of the value bytes, which is the pin's size.
    const WIDTH: usize;
    /// Writes the value bytes into `out[..WIDTH]`.
    fn encode(self, out: &mut [u8]);
    /// The value of `bytes[..WIDTH]`.
    fn decode(bytes: &[u8]) -> Self;
    /// The value whose bytes are all zero: what the payload of a `None` unpacks to, for a type Rust gives no `Default`.
    fn zeroed() -> Self {
        Self::decode(&vec![0u8; Self::WIDTH])
    }
    /// How many levels the type has (§2): the `T::WIDTH` of MSL, not the byte width above.
    const LEVELS: u32;
    /// The levels as bits, level `k` being bit `k`: the form `same` compares and `cast` moves
    /// between types of the same level count (§22.1.3). A type of more than 64 levels has more
    /// than one word.
    fn to_levels(self) -> LevelWords;
    /// The value whose levels are the first `LEVELS` levels of `words`.
    fn from_levels(words: &LevelWords) -> Self;
    /// The value whose levels are the first `LEVELS` levels of `words`, or `None` when the type
    /// rejects them: an enum whose tag names no variant (`cast` is an `Option`, §22.1.3). A type that
    /// accepts every pattern of its levels is always `Some`.
    fn try_from_levels(words: &LevelWords) -> Option<Self> {
        Some(Self::from_levels(words))
    }

    /// The value of a pin's bytes.
    fn from_pin(bytes: &[u8]) -> Self {
        if bytes.len() < Self::WIDTH {
            panic!("a pin of {} bytes read as a value of {} bytes", bytes.len(), Self::WIDTH);
        }
        Self::decode(bytes)
    }

    /// The pin's bytes: the value.
    fn pin(self) -> Vec<u8> {
        let mut bytes = vec![0u8; Self::WIDTH];
        self.encode(&mut bytes);
        bytes
    }

    /// Writes the pin's bytes (`pin`) at the start of `out`, the buffer of a write cell.
    fn write_pin(self, out: &mut [u8]) {
        let bytes = self.pin();
        out[..bytes.len()].copy_from_slice(&bytes);
    }

    /// Writes the drive net's bytes into `out`, the buffer of a drive pin: the value's bytes, then the plain `driving`
    /// byte set to 1 (abi.md §9 D1: a driver writes `driving` 1 with every value it writes).
    fn write_drive(self, out: &mut [u8]) {
        self.write_pin(out);
        out[Self::WIDTH] = 1;
    }

    /// Whether the drive net `bytes` holds a drive present: its `driving` byte, right after the value's bytes, is 1.
    fn is_driving(bytes: &[u8]) -> bool {
        bytes.get(Self::WIDTH).is_some_and(|driving| *driving != 0)
    }
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

    /// `self` followed by false levels up to `len` levels: the payload of a variant narrower than the
    /// widest (§2 rule 6). Already `len` levels or more, `self` as it is.
    pub fn padded(mut self, len: u32) -> LevelWords {
        if self.len < len {
            self.append(&LevelWords::zeros(len - self.len));
        }
        self
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
/// level form of a tuple, an array, an `Option` or a derived struct or enum (the generated `Carried`
/// impls of `print/types.rs` call it too).
pub struct Field;

impl Field {
    /// Writes `value`'s bytes at `*at`, then moves `*at` past them.
    pub fn write<T: Carried>(value: T, out: &mut [u8], at: &mut usize) {
        value.encode(&mut out[*at..*at + T::WIDTH]);
        *at += T::WIDTH;
    }

    /// The value whose bytes start at `*at`; moves `*at` past them.
    pub fn read<T: Carried>(bytes: &[u8], at: &mut usize) -> T {
        let value = T::decode(&bytes[*at..*at + T::WIDTH]);
        *at += T::WIDTH;
        value
    }

    /// The value whose levels start at level `*at`; moves `*at` past them.
    pub fn levels<T: Carried>(words: &LevelWords, at: &mut u32) -> T {
        let value = T::from_levels(&words.slice(*at, T::LEVELS));
        *at += T::LEVELS;
        value
    }

    /// Like [`Field::levels`], `None` when the type rejects the levels.
    pub fn try_levels<T: Carried>(words: &LevelWords, at: &mut u32) -> Option<T> {
        let value = T::try_from_levels(&words.slice(*at, T::LEVELS));
        *at += T::LEVELS;
        value
    }
}

impl Carried for bool {
    const WIDTH: usize = 1;
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

impl Carried for f64 {
    const WIDTH: usize = 8;
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

impl<const N: u32> Carried for Int<N> {
    const WIDTH: usize = Word::width(N);
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

/// The tag of a derived enum in the byte form: its value in the low `width` bytes, little-endian
/// (none for an enum of one variant), and the widths a derived enum's layout takes the widest of.
pub struct Tag;

impl Tag {
    /// Writes `tag` into `out[..width]`.
    pub fn put(tag: u64, out: &mut [u8], width: usize) {
        out[..width].copy_from_slice(&tag.to_le_bytes()[..width]);
    }

    /// The tag in `bytes[..width]`.
    pub fn get(bytes: &[u8], width: usize) -> u64 {
        let mut word = [0u8; 8];
        word[..width].copy_from_slice(&bytes[..width]);
        u64::from_le_bytes(word)
    }

    /// The widest of `widths`: the payload of a derived enum is as wide as its widest variant's.
    pub const fn widest(widths: &[usize]) -> usize {
        let mut widest = 0;
        let mut at = 0;
        while at < widths.len() {
            if widths[at] > widest {
                widest = widths[at];
            }
            at += 1;
        }
        widest
    }
}

// ---- composite carried types (§2): tuples, arrays and `Option` ----------------------------------------------
//
// The value bytes of a composite are those of its parts one after another, the first lowest, then
// its levels are the parts' levels in the same order.

impl<A: Carried> Carried for (A,) {
    const WIDTH: usize = A::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A,)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?,))
    }
}

impl<A: Carried, B: Carried> Carried for (A, B) {
    const WIDTH: usize = A::WIDTH + B::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried> Carried for (A, B, C) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried> Carried for (A, B, C, D) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried> Carried for (A, B, C, D, E) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried> Carried for (A, B, C, D, E, F) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E, F)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried> Carried for (A, B, C, D, E, F, G) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E, F, G)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried> Carried for (A, B, C, D, E, F, G, H) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E, F, G, H)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried> Carried for (A, B, C, D, E, F, G, H, I) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E, F, G, H, I)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried, J: Carried> Carried for (A, B, C, D, E, F, G, H, I, J) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH + J::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E, F, G, H, I, J)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried, J: Carried, K: Carried> Carried for (A, B, C, D, E, F, G, H, I, J, K) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH + J::WIDTH + K::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E, F, G, H, I, J, K)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

impl<A: Carried, B: Carried, C: Carried, D: Carried, E: Carried, F: Carried, G: Carried, H: Carried, I: Carried, J: Carried, K: Carried, L: Carried> Carried for (A, B, C, D, E, F, G, H, I, J, K, L) {
    const WIDTH: usize = A::WIDTH + B::WIDTH + C::WIDTH + D::WIDTH + E::WIDTH + F::WIDTH + G::WIDTH + H::WIDTH + I::WIDTH + J::WIDTH + K::WIDTH + L::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<(A, B, C, D, E, F, G, H, I, J, K, L)> {
        let mut at = 0;
        Some((Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?, Field::try_levels(words, &mut at)?))
    }
}

/// An array of any length is carried (`Default` is no part of `Carried`: Rust gives it up to 32 elements, and `encode`
/// of a 64-level layout is a `[bool; 64]`): element 0 is the lowest.
impl<T: Carried, const N: usize> Carried for [T; N] {
    const WIDTH: usize = N * T::WIDTH;
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
    fn try_from_levels(words: &LevelWords) -> Option<[T; N]> {
        let mut at = 0;
        let mut items = Vec::with_capacity(N);
        for _ in 0..N {
            items.push(Field::try_levels(words, &mut at)?);
        }
        items.try_into().ok()
    }
}

/// `Option<T>`: level 0 is the tag, true for `Some`, then `T`'s levels (all false for `None`); the
/// bytes are a tag byte, then `T`'s value bytes (all zero for `None`).
impl<T: Carried> Carried for Option<T> {
    const WIDTH: usize = 1 + T::WIDTH;
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

#[cfg(test)]
mod tests {
    use super::{Carried, LevelWords};
    use crate::rt::int::{Int, UInt};
    use piperine_abi::layout::{HIGH, LOW};
    use piperine_ir::{Constant, Real, ValueType};
    use piperine_plan::CarrierLayout;

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
        assert_ne!(wide.to_levels(), (-2.5f64, UInt::<8>::new(0b1000_0011)).to_levels(), "the levels of both words are compared");
    }

    #[test]
    fn a_cast_keeps_the_levels_under_another_type() {
        // §22.1.3 rule 1: level `k` of the result is level `k` of the operand
        assert_eq!(Int::<4>::from_levels(&UInt::<4>::new(0b1011).to_levels()), Int::<4>::new(-5));
        assert_eq!(UInt::<4>::from_levels(&Int::<4>::new(-1).to_levels()), UInt::<4>::new(15));
        assert_eq!(UInt::<1>::from_levels(&true.to_levels()), UInt::<1>::new(1));
        assert_eq!(bool::from_levels(&UInt::<1>::new(0).to_levels()), false);
        assert_eq!(UInt::<64>::from_levels(&1.0f64.to_levels()), UInt::<64>::new(1.0f64.to_bits()));
        let pair = (UInt::<3>::new(0b101), true);
        assert_eq!(UInt::<4>::from_levels(&pair.to_levels()), UInt::<4>::new(0b1101));
        assert_eq!(<(UInt<3>, bool)>::from_levels(&UInt::<4>::new(0b1101).to_levels()), pair);
    }

    #[test]
    fn same_compares_every_level() {
        // §22.1.4: a NaN is the same as itself, `0.0` and `-0.0` are two values
        assert_eq!(UInt::<8>::new(9).to_levels(), UInt::<8>::new(9).to_levels());
        assert_ne!(UInt::<8>::new(9).to_levels(), UInt::<8>::new(8).to_levels());
        assert_eq!(f64::NAN.to_levels(), f64::NAN.to_levels());
        assert_ne!(0.0f64.to_levels(), (-0.0f64).to_levels());
        assert_ne!(Some(UInt::<3>::new(2)).to_levels(), None::<UInt<3>>.to_levels(), "Some and None differ in the tag level");
    }

    #[test]
    fn a_pin_holds_the_value_and_nothing_else() {
        // RS-12 AC 1: the plan's canonical bytes for the same type and value
        assert_eq!(true.pin(), HIGH);
        assert_eq!(false.pin(), LOW);
        let plan = |ty: ValueType, c: Constant| CarrierLayout::bytes(&ty, c).expect("a value of the type");
        assert_eq!(1.5f64.pin(), plan(ValueType::F64, Constant::Real(Real::new(1.5))));
        assert_eq!(UInt::<9>::new(300).pin(), plan(ValueType::UInt(9), Constant::Int(300)));
        assert_eq!(Int::<9>::new(-2).pin(), plan(ValueType::Int(9), Constant::Int(-2)));
        assert_eq!(Int::<32>::new(-70000).pin(), plan(ValueType::Int(32), Constant::Int(-70000)));
        assert_eq!(UInt::<64>::new(u64::MAX).pin(), plan(ValueType::UInt(64), Constant::Int(i128::from(u64::MAX))));
        assert_eq!(Int::<1>::new(-1).pin(), plan(ValueType::Int(1), Constant::Int(-1)));
        // and read back
        assert_eq!(UInt::<9>::from_pin(&[44, 1]), UInt::new(300));
        assert_eq!(Int::<9>::from_pin(&[254, 255]), Int::new(-2));
        assert_eq!(bool::from_pin(&HIGH), true);
        assert_eq!(f64::from_pin(&(-0.25f64).pin()), -0.25);
        // the bytes of a composite: the parts' value bytes one after another
        let pair = (UInt::<3>::new(0b101), true);
        assert_eq!(<(UInt<3>, bool)>::WIDTH, 2);
        assert_eq!(pair.pin(), vec![0b101, 1]);
        assert_eq!(<(UInt<3>, bool)>::from_pin(&[0b101, 1]), pair);
        assert_eq!([UInt::<2>::new(1), UInt::<2>::new(2), UInt::<2>::new(3)].pin(), vec![1, 2, 3]);
        assert_eq!(<Option<UInt<3>>>::from_pin(&Some(UInt::<3>::new(6)).pin()), Some(UInt::<3>::new(6)));
        assert_eq!(None::<UInt<3>>.pin(), vec![0, 0]);
        let mut padded = [9u8, 9];
        None::<UInt<3>>.encode(&mut padded);
        assert_eq!(padded, [0, 0], "None encodes its payload as zeros");
    }
}
