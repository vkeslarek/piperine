//! Four-state (`0`/`1`/`x`/`z`) bit-vector value type — LRMC-C1-02.
//!
//! [`LogicVec`] is a width-carrying vector of four-state bits, stored as two
//! parallel bitplanes (`val`, `unk`) so bulk queries (`has_unknown`,
//! equality) stay word-at-a-time instead of bit-at-a-time. Bit *i*: `unk`
//! bit 0 → `val` bit is the real 0/1 value; `unk` bit 1 → `val` bit selects
//! `X` (0) or `Z` (1). This is the frontend's only four-state
//! representation — [`crate::pom::net_type`]'s scalars and
//! `piperine_solver::abi::LogicValue` stay the 1-bit projection used by the
//! digital JIT/solver, never widened themselves (`design.md` P0.2).

/// One bit's worth of four-state logic — mirrors
/// `piperine_solver::abi::LogicValue`'s shape (`Zero`/`One`/`X`/`Z`,
/// `digital/events.rs:11`) so a future bridge between the two is a direct
/// discriminant match.
///
/// SPEC_DEVIATION: the design calls for `LogicVec::bit(i) -> LogicValue`
/// reusing `piperine_solver::abi::LogicValue` directly. `piperine-lang` has
/// no dependency on `piperine-solver` today and this task's file scope
/// (`crates/piperine-lang/...`) does not include `Cargo.toml` — adding a new
/// cross-crate dependency edge is a bigger architectural change than "add a
/// value type". This local type has the identical shape/discriminants, so
/// whichever later task wires `LogicVec` into the digital JIT/solver
/// boundary can bridge the two with a one-line `From` impl, or this type can
/// be re-exported from there instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LogicValue {
    Zero = 0,
    One = 1,
    X = 2,
    Z = 3,
}

/// The base a based literal's digits are written in — one bit-group size
/// per digit, except decimal (whole-literal `x`/`z` only, per §2.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Binary,
    Octal,
    Decimal,
    Hex,
}

impl Base {
    fn bits_per_digit(self) -> u32 {
        match self {
            Base::Binary => 1,
            Base::Octal => 3,
            Base::Decimal => 0, // no per-digit meaning; decimal is handled separately
            Base::Hex => 4,
        }
    }

    fn radix(self) -> u32 {
        match self {
            Base::Binary => 2,
            Base::Octal => 8,
            Base::Decimal => 10,
            Base::Hex => 16,
        }
    }
}

enum DigitKind {
    Concrete(u32),
    X,
    Z,
}

fn parse_digit(base: Base, c: char) -> Option<DigitKind> {
    match c {
        'x' | 'X' => Some(DigitKind::X),
        'z' | 'Z' | '?' => Some(DigitKind::Z),
        _ => c.to_digit(base.radix()).map(DigitKind::Concrete),
    }
}

const WORD_BITS: u32 = 64;

fn words_for(width: u32) -> usize {
    (width as usize).div_ceil(WORD_BITS as usize).max(1)
}

fn get_bit(plane: &[u64], i: u32) -> bool {
    let w = (i / WORD_BITS) as usize;
    let b = i % WORD_BITS;
    (plane[w] >> b) & 1 == 1
}

fn set_bit(plane: &mut [u64], i: u32, value: bool) {
    let w = (i / WORD_BITS) as usize;
    let b = i % WORD_BITS;
    if value {
        plane[w] |= 1u64 << b;
    } else {
        plane[w] &= !(1u64 << b);
    }
}

/// A width-carrying four-state bit vector — LRMC-C1-02.
///
/// Bit 0 is the least-significant bit. `unk`'s bits beyond `width` (padding
/// in the last word) are always 0, so [`LogicVec::has_unknown`] can scan the
/// whole bitplane without a width mask.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LogicVec {
    width: u32,
    val: Vec<u64>,
    unk: Vec<u64>,
}

impl LogicVec {
    /// The vector's bit width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// An all-zero (two-state `0`) vector, `width` bits wide.
    pub fn zero(width: u32) -> Self {
        let n = words_for(width);
        Self { width, val: vec![0; n], unk: vec![0; n] }
    }

    /// Builds a two-state `LogicVec` from an exact integer value.
    pub fn from_int(value: i64, width: u32) -> Self {
        let mut v = Self::zero(width);
        for i in 0..width.min(63) {
            if (value >> i) & 1 != 0 {
                v.set_bit(i, LogicValue::One);
            }
        }
        v
    }

    /// Bit *i*'s four-state value. Panics if `i >= width` (an internal
    /// invariant violation, not a user-facing condition — callers index
    /// only within `0..width`).
    pub fn bit(&self, i: u32) -> LogicValue {
        assert!(i < self.width, "LogicVec::bit index {i} out of range for width {}", self.width);
        match (get_bit(&self.unk, i), get_bit(&self.val, i)) {
            (false, false) => LogicValue::Zero,
            (false, true) => LogicValue::One,
            (true, false) => LogicValue::X,
            (true, true) => LogicValue::Z,
        }
    }

    /// Sets bit *i* to the given four-state value.
    pub fn set_bit(&mut self, i: u32, lv: LogicValue) {
        assert!(i < self.width, "LogicVec::set_bit index {i} out of range for width {}", self.width);
        let (v, u) = match lv {
            LogicValue::Zero => (false, false),
            LogicValue::One => (true, false),
            LogicValue::X => (false, true),
            LogicValue::Z => (true, true),
        };
        set_bit(&mut self.val, i, v);
        set_bit(&mut self.unk, i, u);
    }

    /// True if any bit is `X` or `Z`.
    pub fn has_unknown(&self) -> bool {
        self.unk.iter().any(|&w| w != 0)
    }

    /// Converts to an exact `i64`, or `None` if the value has any unknown
    /// bit or does not fit (bit 63 and above must all be the two-state `0`).
    /// Callers at a context with no four-state semantics (array index, bit
    /// width, generate bound) treat `None` as a fail-loud condition citing
    /// the LRM clause that makes the usage illegal — never a truncation
    /// (spec Edge Case).
    pub fn to_i64_exact(&self) -> Option<i64> {
        if self.has_unknown() {
            return None;
        }
        if self.width == 0 {
            return Some(0);
        }
        for i in 63..self.width {
            if get_bit(&self.val, i) {
                return None;
            }
        }
        let mut result: i64 = 0;
        for i in 0..self.width.min(63) {
            if get_bit(&self.val, i) {
                result |= 1i64 << i;
            }
        }
        Some(result)
    }

    /// Case equality (`===`/`!==` is `!case_eq`) — Annex C.5-aware: compares
    /// `X`/`Z` bits literally instead of treating them as "don't care"
    /// (that's `==`'s job on the resolved two-state value, not this
    /// method's). Narrower operands are zero-extended to the wider width
    /// before comparison, matching the LRM's context-determined bit-length
    /// extension rule (§4.5.1) for unsized/differently-sized operands.
    pub fn case_eq(&self, other: &Self) -> bool {
        let width = self.width.max(other.width);
        for i in 0..width {
            let a = if i < self.width { self.bit(i) } else { LogicValue::Zero };
            let b = if i < other.width { other.bit(i) } else { LogicValue::Zero };
            if a != b {
                return false;
            }
        }
        true
    }

    /// Parses a based literal's digit string (post-base-letter, e.g. `"1F"`
    /// for `` 'h1F ``, `"z"` for `` 'bz ``) into a `LogicVec`. `width` is
    /// `None` for an unsized literal (defaults to 32 bits, the LRM's own
    /// unsized-integer default) or `Some(w)` for a sized literal (`8'hFF`).
    /// Underscores must already be stripped by the caller (a lexer-level
    /// concern, LRMC-C1-04). Returns `None` for a digit illegal in the
    /// given base — the caller surfaces that as the typed lexical error
    /// (LRMC-C1-01), this function does not itself classify the failure.
    pub fn from_based_literal(base: Base, digits: &str, width: Option<u32>) -> Option<LogicVec> {
        if base == Base::Decimal {
            return Self::from_decimal_literal(digits, width);
        }
        let bpd = base.bits_per_digit();
        let chars: Vec<char> = digits.chars().collect();
        if chars.is_empty() {
            return None;
        }
        let total_bits = bpd * chars.len() as u32;
        // Unsized literals default to (at least) 32 bits per the LRM's own
        // unsized-integer rule — wider only if the digit string needs more.
        let w = width.unwrap_or(total_bits.max(32));
        let mut v = Self::zero(w);
        let mut bit_pos = 0u32;
        // The most-significant (leftmost) digit's kind — used below to
        // extend a sized literal wider than its digit string per §2.6.1
        // ("the unfilled left-most bits are filled with the value of the
        // left-most character": 0 for a concrete digit, x/z if that digit
        // was x/z).
        let mut msd_kind = DigitKind::Concrete(0);
        for &c in chars.iter().rev() {
            let kind = parse_digit(base, c)?;
            for b in 0..bpd {
                if bit_pos >= w {
                    break;
                }
                let lv = match kind {
                    DigitKind::X => LogicValue::X,
                    DigitKind::Z => LogicValue::Z,
                    DigitKind::Concrete(val) => {
                        if (val >> b) & 1 != 0 {
                            LogicValue::One
                        } else {
                            LogicValue::Zero
                        }
                    }
                };
                v.set_bit(bit_pos, lv);
                bit_pos += 1;
            }
            msd_kind = kind;
        }
        let extension = match msd_kind {
            DigitKind::X => LogicValue::X,
            DigitKind::Z => LogicValue::Z,
            DigitKind::Concrete(_) => LogicValue::Zero,
        };
        while bit_pos < w {
            v.set_bit(bit_pos, extension);
            bit_pos += 1;
        }
        Some(v)
    }

    fn from_decimal_literal(digits: &str, width: Option<u32>) -> Option<LogicVec> {
        let trimmed = digits.trim();
        let w = width.unwrap_or(32);
        if trimmed.eq_ignore_ascii_case("x") {
            let mut v = Self::zero(w);
            for i in 0..w {
                v.set_bit(i, LogicValue::X);
            }
            return Some(v);
        }
        if trimmed.eq_ignore_ascii_case("z") || trimmed == "?" {
            let mut v = Self::zero(w);
            for i in 0..w {
                v.set_bit(i, LogicValue::Z);
            }
            return Some(v);
        }
        let value: i128 = trimmed.parse().ok()?;
        let mut v = Self::zero(w);
        for i in 0..w.min(127) {
            if (value >> i) & 1 != 0 {
                v.set_bit(i, LogicValue::One);
            }
        }
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_vector_has_no_unknowns_and_converts_exactly() {
        let v = LogicVec::zero(8);
        assert!(!v.has_unknown());
        assert_eq!(v.to_i64_exact(), Some(0));
        for i in 0..8 {
            assert_eq!(v.bit(i), LogicValue::Zero);
        }
    }

    #[test]
    fn from_int_round_trips_through_to_i64_exact() {
        let v = LogicVec::from_int(0b1010_1100, 8);
        assert_eq!(v.to_i64_exact(), Some(0b1010_1100));
        assert!(!v.has_unknown());
    }

    #[test]
    fn set_bit_covers_every_four_state_combination() {
        let mut v = LogicVec::zero(4);
        v.set_bit(0, LogicValue::Zero);
        v.set_bit(1, LogicValue::One);
        v.set_bit(2, LogicValue::X);
        v.set_bit(3, LogicValue::Z);
        assert_eq!(v.bit(0), LogicValue::Zero);
        assert_eq!(v.bit(1), LogicValue::One);
        assert_eq!(v.bit(2), LogicValue::X);
        assert_eq!(v.bit(3), LogicValue::Z);
        assert!(v.has_unknown());
        assert_eq!(v.to_i64_exact(), None);
    }

    #[test]
    fn width_edge_one_bit() {
        let mut v = LogicVec::zero(1);
        assert_eq!(v.width(), 1);
        v.set_bit(0, LogicValue::One);
        assert_eq!(v.to_i64_exact(), Some(1));
    }

    #[test]
    fn width_edge_sixty_three_bits_fits_i64() {
        let mut v = LogicVec::zero(63);
        v.set_bit(62, LogicValue::One);
        v.set_bit(0, LogicValue::One);
        assert_eq!(v.to_i64_exact(), Some((1i64 << 62) | 1));
        assert!(!v.has_unknown());
    }

    #[test]
    fn width_edge_sixty_four_bits_word_boundary() {
        let mut v = LogicVec::zero(64);
        assert_eq!(v.width(), 64);
        // bit 63 set: does not fit in the exact-i64 conversion (bits >= 63 must be zero).
        v.set_bit(63, LogicValue::One);
        assert_eq!(v.to_i64_exact(), None);
        // clearing it back to zero makes it representable again.
        v.set_bit(63, LogicValue::Zero);
        assert_eq!(v.to_i64_exact(), Some(0));
    }

    #[test]
    fn width_edge_sixty_five_bits_spans_two_words() {
        let mut v = LogicVec::zero(65);
        assert_eq!(v.width(), 65);
        v.set_bit(64, LogicValue::X);
        assert!(v.has_unknown());
        assert_eq!(v.bit(64), LogicValue::X);
        // an unknown high bit still fails exact conversion.
        assert_eq!(v.to_i64_exact(), None);
        v.set_bit(64, LogicValue::One);
        // a concrete but out-of-range high bit also fails (doesn't fit i64).
        assert_eq!(v.to_i64_exact(), None);
    }

    #[test]
    fn case_eq_distinguishes_x_from_zero_unlike_ordinary_equality() {
        let mut a = LogicVec::zero(4);
        let mut b = LogicVec::zero(4);
        a.set_bit(0, LogicValue::X);
        b.set_bit(0, LogicValue::Zero);
        assert!(!a.case_eq(&b));
        b.set_bit(0, LogicValue::X);
        assert!(a.case_eq(&b));
    }

    #[test]
    fn case_eq_zero_extends_narrower_operand() {
        let a = LogicVec::from_int(0b101, 3);
        let b = LogicVec::from_int(0b101, 8);
        assert!(a.case_eq(&b));
    }

    #[test]
    fn from_based_literal_hex_expands_x_and_z_digits_to_four_bits() {
        let v = LogicVec::from_based_literal(Base::Hex, "x", Some(4)).unwrap();
        for i in 0..4 {
            assert_eq!(v.bit(i), LogicValue::X);
        }
        let v = LogicVec::from_based_literal(Base::Hex, "z", Some(4)).unwrap();
        for i in 0..4 {
            assert_eq!(v.bit(i), LogicValue::Z);
        }
        let v = LogicVec::from_based_literal(Base::Hex, "?", Some(4)).unwrap();
        for i in 0..4 {
            assert_eq!(v.bit(i), LogicValue::Z);
        }
    }

    #[test]
    fn from_based_literal_hex_concrete_value() {
        let v = LogicVec::from_based_literal(Base::Hex, "FF", Some(8)).unwrap();
        assert_eq!(v.to_i64_exact(), Some(0xFF));
    }

    #[test]
    fn from_based_literal_binary_mixed_known_and_unknown() {
        let v = LogicVec::from_based_literal(Base::Binary, "01x", Some(3)).unwrap();
        assert_eq!(v.bit(0), LogicValue::X);
        assert_eq!(v.bit(1), LogicValue::One);
        assert_eq!(v.bit(2), LogicValue::Zero);
        assert!(v.has_unknown());
    }

    #[test]
    fn from_based_literal_octal_z_digit() {
        let v = LogicVec::from_based_literal(Base::Octal, "z", Some(3)).unwrap();
        for i in 0..3 {
            assert_eq!(v.bit(i), LogicValue::Z);
        }
    }

    #[test]
    fn from_based_literal_decimal_whole_literal_x() {
        let v = LogicVec::from_based_literal(Base::Decimal, "x", Some(12)).unwrap();
        assert!(v.has_unknown());
        for i in 0..12 {
            assert_eq!(v.bit(i), LogicValue::X);
        }
    }

    #[test]
    fn from_based_literal_decimal_concrete_value() {
        let v = LogicVec::from_based_literal(Base::Decimal, "42", Some(8)).unwrap();
        assert_eq!(v.to_i64_exact(), Some(42));
    }

    #[test]
    fn from_based_literal_rejects_illegal_digit() {
        assert!(LogicVec::from_based_literal(Base::Binary, "2", Some(4)).is_none());
        assert!(LogicVec::from_based_literal(Base::Octal, "9", Some(4)).is_none());
    }

    #[test]
    fn from_based_literal_unsized_defaults_to_thirty_two_bits() {
        let v = LogicVec::from_based_literal(Base::Hex, "F", None).unwrap();
        assert_eq!(v.width(), 32);
    }
}
