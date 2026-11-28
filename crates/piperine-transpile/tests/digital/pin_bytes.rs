//! The bytes of a pin, as the compiled reactions and drives read and write them.

/// The bytes of a pin holding a value, and the value a pin's bytes hold.
pub struct PinBytes;

impl PinBytes {
    /// The bytes of a `Bit<T>` pin holding `value`: the value and nothing else (abi.md P3).
    pub fn pin_bytes(value: &[u8]) -> Vec<u8> {
        value.to_vec()
    }

    /// The bytes of a `Bit<f64>` pin holding `value`: its eight bytes (abi.md P3, `REAL`: the value at 0).
    pub fn real_pin_bytes(value: f64) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    /// The `f64` in the first eight bytes of a pin.
    pub fn real_at(bytes: &[u8]) -> f64 {
        f64::from_le_bytes(bytes[..8].try_into().expect("eight bytes"))
    }
}
