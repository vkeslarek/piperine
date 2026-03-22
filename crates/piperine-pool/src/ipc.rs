//! Length-prefixed bincode IPC transport.

use std::io::{self, Read, Write};

/// Read a length-prefixed bincode message.
pub fn read_msg<T: serde::de::DeserializeOwned>(r: &mut impl Read) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    bincode::deserialize(&buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write a length-prefixed bincode message.
pub fn write_msg<T: serde::Serialize>(w: &mut impl Write, msg: &T) -> io::Result<()> {
    let bytes = bincode::serialize(msg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}
