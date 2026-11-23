//! The bytes of a carrier value as the plan lays it out (abi.md §3.1 P3): the value
//! little-endian and nothing else.

/// The bytes a carrier of each scalar type holds.
pub struct CarrierBytes;

impl CarrierBytes {
    /// An `f64` carrier value: its eight bytes.
    pub fn real(v: f64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    /// A `u64` carrier value: its eight bytes.
    pub fn unsigned_64(v: u64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    /// A one-byte carrier value (`u8`, `bool`).
    pub fn byte(v: u8) -> Vec<u8> {
        vec![v]
    }

    /// A `u16` carrier value: two bytes little-endian.
    pub fn word(v: u16) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    /// An `i16` carrier value: two bytes little-endian.
    pub fn short(v: i16) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }
}
