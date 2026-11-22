//! The operators of the place models (`let_mut.msl`, `places.msl`), written once in Rust: the
//! sequence each model applies to a local, which the run is compared with.

/// The compound-assignment sequences of the place models.
pub struct PlaceOperators;

impl PlaceOperators {
    /// The sequence of the integer operators, from `start`.
    pub fn integer(start: u8) -> u8 {
        let mut n = start;
        n += 3;
        n -= 1;
        n *= 2;
        n /= 3;
        n %= 5;
        n &= 7;
        n |= 8;
        n ^= 5;
        n <<= 1;
        n >>= 1;
        n
    }

    /// The sequence of the real operators, from `start`.
    pub fn real(start: f64) -> f64 {
        let mut x = start;
        x += 1.5;
        x -= 0.25;
        x *= 3.0;
        x /= 2.0;
        x
    }
}
