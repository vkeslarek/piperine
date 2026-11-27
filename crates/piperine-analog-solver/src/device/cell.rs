//! The `F64` cell: eight little-endian bytes at the start of a slot or a write buffer.

/// Reads of the `F64` cells the device plan lays out.
pub(crate) struct RealCell;

impl RealCell {
    /// The value at byte 0 of `bytes`. A buffer shorter than a cell is read zero-extended, so
    /// the read cannot fail: the validated plan sizes every `F64` cell to eight bytes.
    pub(crate) fn read(bytes: &[u8]) -> f64 {
        let mut raw = [0u8; 8];
        let width = bytes.len().min(raw.len());
        raw[..width].copy_from_slice(&bytes[..width]);
        f64::from_le_bytes(raw)
    }
}
