//! A deterministic pseudo-random generator for the tests (xorshift64*), so every run sees the
//! same operands.

/// xorshift64* with a fixed seed.
pub struct ShiftGenerator {
    /// The generator state; never zero.
    state: u64,
}

impl ShiftGenerator {
    /// A generator seeded with `seed` (a zero seed is replaced by a fixed constant).
    pub fn new(seed: u64) -> Self {
        ShiftGenerator { state: if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed } }
    }

    /// The next 64 pseudo-random bits.
    pub fn next_bits(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}
