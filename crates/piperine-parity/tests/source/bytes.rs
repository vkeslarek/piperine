//! The bytes of a known carrier value as the plan lays it out (abi.md §3.1): the value
//! little-endian, then the known byte, padded to the alignment of the value.

/// The bytes a known carrier of each scalar type holds.
pub struct CarrierBytes;

impl CarrierBytes {
    /// A known `f64` carrier value: the value, then the known byte.
    pub fn real(v: f64) -> Vec<u8> {
        Self::eight_bytes(&v.to_le_bytes())
    }

    /// A known `u64` carrier value: the value, then the known byte.
    pub fn unsigned_64(v: u64) -> Vec<u8> {
        Self::eight_bytes(&v.to_le_bytes())
    }

    /// A known one-byte carrier value (`u8`, `bool`).
    pub fn byte(v: u8) -> Vec<u8> {
        vec![v, 1]
    }

    /// A known `u16` carrier value: two bytes little-endian, the known byte, one byte of padding.
    pub fn word(v: u16) -> Vec<u8> {
        Self::two_bytes(&v.to_le_bytes())
    }

    /// A known `i16` carrier value: two bytes little-endian, the known byte, one byte of padding.
    pub fn short(v: i16) -> Vec<u8> {
        Self::two_bytes(&v.to_le_bytes())
    }

    /// `value`, eight bytes, then the known byte, in the 16 bytes of an eight-byte carrier.
    fn eight_bytes(value: &[u8; 8]) -> Vec<u8> {
        let mut bytes = vec![0u8; 16];
        bytes[..8].copy_from_slice(value);
        bytes[8] = 1;
        bytes
    }

    /// `value`, two bytes, then the known byte and one byte of padding.
    fn two_bytes(value: &[u8; 2]) -> Vec<u8> {
        let mut bytes = value.to_vec();
        bytes.extend([1, 0]);
        bytes
    }
}
