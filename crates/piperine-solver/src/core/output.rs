//! The simulation's runtime output path — `SimOutput` (the single sink
//! every executing body writes text through), its concrete sinks
//! (`StdoutSink`, `BufferSink`), and `FileTable` (the `$fopen`/`$fclose`/
//! `$fdisplay`/... descriptor table, §9.5). P0.1 of the
//! lrm-conformance-closure spine (design.md) — this file has no caller yet
//! (the JIT trampolines reaching it land in T5; the ch.9 task catalogue
//! wiring lands in P3) and closes no LRMC-ID by itself.
//!
//! **Never a silent zero descriptor.** Every `FileTable` operation returns
//! a typed [`FileError`] on failure instead of manufacturing a fake
//! success — matching design.md's Error Handling Strategy table.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

// ---------------------------------------------------------------------------
// SimOutput
// ---------------------------------------------------------------------------

/// An open file's handle, as returned by [`FileTable::open`] (`$fopen`,
/// §9.5) — an opaque id, never a raw index a caller could forge out of
/// range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub u32);

/// Which channel a write targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Stdout,
    Stderr,
    File(FileId),
}

/// The runtime output contract every executing body (analog JIT, digital
/// JIT, elaboration-time system tasks) writes through. One mechanism for
/// `$display`/`$write`/`$strobe`/`$monitor`, `vpi_printf`/`vpi_mcd_*`, and
/// file I/O alike (design.md's Tech Decisions table).
pub trait SimOutput {
    /// Writes `text` to `ch` (no implicit newline — callers format exactly
    /// what the LRM's format-specifier rendering produces).
    fn write(&self, ch: Channel, text: &str);
    /// Flushes any buffering. A no-op for sinks that don't buffer.
    fn flush(&self);
}

/// The default sink: writes go to the process's real stdout/stderr.
/// `Channel::File` isn't meaningful here — `FileTable`'s own `write`/`read`
/// methods are the file-I/O path (see below); a `SimOutput` implementation
/// is only ever asked to route `Stdout`/`Stderr`.
#[derive(Debug, Default)]
pub struct StdoutSink;

impl SimOutput for StdoutSink {
    fn write(&self, ch: Channel, text: &str) {
        match ch {
            Channel::Stdout => print!("{text}"),
            Channel::Stderr => eprint!("{text}"),
            Channel::File(id) => {
                // Reachable only if a caller routes a file-descriptor write
                // through the process-wide sink instead of `FileTable`
                // directly (a caller bug, not a runtime condition) — loud
                // in debug builds, harmlessly dropped in release rather
                // than panicking mid-simulation over an I/O routing detail.
                debug_assert!(
                    false,
                    "StdoutSink received a File({id:?}) channel write — file I/O routes through FileTable, not SimOutput"
                );
            }
        }
    }

    fn flush(&self) {
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
    }
}

/// A deterministic in-memory sink for tests — captures every write in
/// arrival order instead of touching the real stdout/stderr, so a
/// transcript can be asserted exactly.
#[derive(Debug, Default)]
pub struct BufferSink {
    stdout: RefCell<String>,
    stderr: RefCell<String>,
}

impl BufferSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything written to `Channel::Stdout` so far, in arrival order.
    pub fn stdout(&self) -> String {
        self.stdout.borrow().clone()
    }

    /// Everything written to `Channel::Stderr` so far, in arrival order.
    pub fn stderr(&self) -> String {
        self.stderr.borrow().clone()
    }
}

impl SimOutput for BufferSink {
    fn write(&self, ch: Channel, text: &str) {
        match ch {
            Channel::Stdout => self.stdout.borrow_mut().push_str(text),
            Channel::Stderr => self.stderr.borrow_mut().push_str(text),
            Channel::File(_) => {
                // Same routing note as `StdoutSink`: file writes are
                // `FileTable`'s concern, not a `SimOutput` impl's.
                debug_assert!(false, "BufferSink received a File channel write — route file I/O through FileTable");
            }
        }
    }

    fn flush(&self) {}
}

// ---------------------------------------------------------------------------
// FileTable — §9.5 file I/O
// ---------------------------------------------------------------------------

/// A typed file-I/O failure — every `FileTable` operation that can fail
/// returns one instead of a silent zero descriptor or a swallowed error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FileError {
    #[error("failed to open `{path}`: {reason}")]
    OpenFailed { path: String, reason: String },
    #[error("unknown file descriptor {0:?} (not open, or already closed)")]
    UnknownDescriptor(FileId),
    #[error("write to `{path}` failed: {reason}")]
    WriteFailed { path: String, reason: String },
    #[error("read from `{path}` failed: {reason}")]
    ReadFailed { path: String, reason: String },
    #[error("no free multi-channel-descriptor bit available (all {0} channel slots in use)")]
    TooManyChannels(u32),
    #[error("`{path}`: {reason}")]
    MemParse { path: String, reason: String },
}

struct FileEntry {
    path: String,
    file: File,
    /// `$ungetc` (§9.5) pushback: at most one byte, returned by the next
    /// `getc`/`fgets`/`fread` before the file itself is touched again.
    pushback: Option<u8>,
}

/// The `$fopen`/`$fclose`/`$fdisplay`/`$fwrite`/`$fscanf`/... descriptor
/// table (LRM §9.5). Descriptors are opaque [`FileId`]s handed out in
/// allocation order — never reused while still open, so a stale id from a
/// prior close is always a typed [`FileError::UnknownDescriptor`], never a
/// silent write to the wrong file.
#[derive(Default)]
pub struct FileTable {
    next_id: u32,
    open: HashMap<FileId, FileEntry>,
}

impl fmt::Debug for FileTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTable").field("open_count", &self.open.len()).finish()
    }
}

impl FileTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens `path` for append+read (`$fopen`'s common mode — write
    /// descriptors append, matching the LRM's own worked example of
    /// repeated `$fdisplay` calls to the same handle). Creates the file if
    /// it doesn't exist. Typed [`FileError::OpenFailed`] on any I/O
    /// failure (permissions, missing directory, ...) — never a silent
    /// zero descriptor.
    pub fn open(&mut self, path: &str) -> Result<FileId, FileError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)
            .map_err(|e| FileError::OpenFailed { path: path.to_string(), reason: e.to_string() })?;
        let id = FileId(self.next_id);
        self.next_id += 1;
        self.open.insert(id, FileEntry { path: path.to_string(), file, pushback: None });
        Ok(id)
    }

    /// Closes `id`, freeing its descriptor. `FileError::UnknownDescriptor`
    /// if `id` isn't currently open.
    pub fn close(&mut self, id: FileId) -> Result<(), FileError> {
        self.open.remove(&id).map(|_| ()).ok_or(FileError::UnknownDescriptor(id))
    }

    /// Writes `text` to the file behind `id`.
    pub fn write(&mut self, id: FileId, text: &str) -> Result<(), FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        entry
            .file
            .write_all(text.as_bytes())
            .map_err(|e| FileError::WriteFailed { path: entry.path.clone(), reason: e.to_string() })
    }

    /// Reads the file behind `id` from its start to a `String` (`$fscanf`/
    /// `$sscanf`'s underlying data source).
    pub fn read_to_string(&mut self, id: FileId) -> Result<String, FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        entry
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|e| FileError::ReadFailed { path: entry.path.clone(), reason: e.to_string() })?;
        let mut s = String::new();
        entry
            .file
            .read_to_string(&mut s)
            .map_err(|e| FileError::ReadFailed { path: entry.path.clone(), reason: e.to_string() })?;
        Ok(s)
    }

    /// True if `id` is currently open.
    pub fn is_open(&self, id: FileId) -> bool {
        self.open.contains_key(&id)
    }

    /// `id`'s open path, if it's currently open — `vpi_mcd_name()`'s
    /// underlying data (`McdTable::name`).
    pub fn path(&self, id: FileId) -> Option<&str> {
        self.open.get(&id).map(|e| e.path.as_str())
    }

    /// `$fflush` (§9.5): flushes buffered writes to disk.
    pub fn flush(&mut self, id: FileId) -> Result<(), FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        entry.file.flush().map_err(|e| FileError::WriteFailed { path: entry.path.clone(), reason: e.to_string() })
    }

    /// `$ftell` (§9.5): the current read/write position, bytes from start.
    pub fn tell(&mut self, id: FileId) -> Result<u64, FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        entry.file.stream_position().map_err(|e| FileError::ReadFailed { path: entry.path.clone(), reason: e.to_string() })
    }

    /// `$fseek` (§9.5): repositions to `offset` bytes from start.
    pub fn seek(&mut self, id: FileId, offset: u64) -> Result<(), FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        entry
            .file
            .seek(SeekFrom::Start(offset))
            .map(|_| ())
            .map_err(|e| FileError::ReadFailed { path: entry.path.clone(), reason: e.to_string() })
    }

    /// `$rewind` (§9.5): repositions to the start — `seek(id, 0)`.
    pub fn rewind(&mut self, id: FileId) -> Result<(), FileError> {
        self.seek(id, 0)
    }

    /// `$feof` (§9.5): true once a read has consumed the file to its end.
    /// Probes by attempting a zero-effect read: reads one byte, then seeks
    /// back if it succeeded (never consumes past what the caller sees).
    pub fn eof(&mut self, id: FileId) -> Result<bool, FileError> {
        let pos = self.tell(id)?;
        let mut buf = [0u8; 1];
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        let n = entry.file.read(&mut buf).map_err(|e| FileError::ReadFailed { path: entry.path.clone(), reason: e.to_string() })?;
        self.seek(id, pos)?;
        Ok(n == 0)
    }

    /// `$fgetc` (§9.5): reads and consumes one byte, `None` at EOF. A
    /// pending [`Self::ungetc`] pushback is returned first, before the
    /// file is touched again.
    pub fn getc(&mut self, id: FileId) -> Result<Option<u8>, FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        if let Some(b) = entry.pushback.take() {
            return Ok(Some(b));
        }
        let mut buf = [0u8; 1];
        let n = entry.file.read(&mut buf).map_err(|e| FileError::ReadFailed { path: entry.path.clone(), reason: e.to_string() })?;
        Ok((n == 1).then_some(buf[0]))
    }

    /// `$ungetc(c, fd)` (§9.5): pushes `byte` back so the next
    /// `getc`/`fgets`/`fread` on `id` sees it before any further file
    /// data. Only one byte of pushback is held (a second call before a
    /// consuming read overwrites the first) — matching the LRM's own
    /// C-`ungetc`-derived "at least one character" contract.
    pub fn ungetc(&mut self, id: FileId, byte: u8) -> Result<(), FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        entry.pushback = Some(byte);
        Ok(())
    }

    /// `$fgets(str, fd)` (§9.5.4.1): reads through the next `\n`
    /// (inclusive) or EOF. `None` if already at EOF with nothing to
    /// return (mirrors `$fgets`'s own "returns 0" empty-read signal at
    /// the caller's typed level instead of a silent empty string).
    pub fn fgets(&mut self, id: FileId) -> Result<Option<String>, FileError> {
        let mut bytes = Vec::new();
        loop {
            match self.getc(id)? {
                Some(b) => {
                    bytes.push(b);
                    if b == b'\n' {
                        break;
                    }
                }
                None => break,
            }
        }
        if bytes.is_empty() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
    }

    /// `$fread(mem, fd)` (§9.5.4.2): reads up to `n` raw bytes, stopping
    /// early at EOF (the returned `Vec` is shorter than `n` in that
    /// case — never padded with fabricated data).
    pub fn fread(&mut self, id: FileId, n: usize) -> Result<Vec<u8>, FileError> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            match self.getc(id)? {
                Some(b) => out.push(b),
                None => break,
            }
        }
        Ok(out)
    }

    /// `$readmemb`/`$readmemh` (§9.5): reads `id`'s full contents and
    /// parses it into `mem` per [`parse_memory_text`], with this file's
    /// real path in any [`FileError::MemParse`] instead of the
    /// placeholder that function uses when given raw text directly.
    pub fn readmem(&mut self, id: FileId, radix: MemRadix, mem: &mut [u64], start_addr: usize) -> Result<(), FileError> {
        let path = self.path(id).ok_or(FileError::UnknownDescriptor(id))?.to_string();
        let text = self.read_to_string(id)?;
        parse_memory_text(&text, radix, mem, start_addr).map_err(|e| match e {
            FileError::MemParse { reason, .. } => FileError::MemParse { path: path.clone(), reason },
            other => other,
        })
    }

    /// `$fscanf`'s underlying data source: reads every remaining byte from
    /// the current position (unlike [`Self::read_to_string`], which always
    /// rewinds to the start first) — the incremental-parse counterpart.
    pub fn read_remaining(&mut self, id: FileId) -> Result<String, FileError> {
        let entry = self.open.get_mut(&id).ok_or(FileError::UnknownDescriptor(id))?;
        let mut s = String::new();
        entry.file.read_to_string(&mut s).map_err(|e| FileError::ReadFailed { path: entry.path.clone(), reason: e.to_string() })?;
        Ok(s)
    }
}

// ---------------------------------------------------------------------------
// $readmemb / $readmemh (§9.5, inherited-base-1364, T55/LRMW2-48)
// ---------------------------------------------------------------------------

/// Which radix a `$readmemb`/`$readmemh` source file's data tokens are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemRadix {
    /// `$readmemb`: binary digits (`0`/`1`), `x`/`X`/`z`/`z` and `_`
    /// separators allowed per token.
    Binary,
    /// `$readmemh`: hex digits, same `x`/`X`/`z`/`Z`/`_` allowance.
    Hex,
}

/// Parses `$readmemb`/`$readmemh` source text into `mem`, starting at
/// `start_addr` (mirrors the LRM's own `$readmemh(file, mem[, start[,
/// finish]])` form — `finish` isn't separately enforced here beyond
/// `mem`'s own length, which is the same bound a `finish` argument would
/// ultimately clamp to). `//` starts a line comment; an `@hexaddr` token
/// jumps the write cursor (always hex, independent of `radix` — matching
/// the base standard). An unrecognized digit or an address past `mem`'s
/// end is a typed [`FileError::MemParse`], never a silently-dropped or
/// zero-filled word — an `x`/`z` digit in the source *is* representable
/// (this table stores `u64`s, so `x`/`z` digits fold to `0`, matching
/// this crate's existing `LogicValue`-to-numeric convention elsewhere;
/// SPEC_DEVIATION: a true 4-state memory word is future work, not
/// this task's stated scope).
pub fn parse_memory_text(text: &str, radix: MemRadix, mem: &mut [u64], start_addr: usize) -> Result<(), FileError> {
    let mut addr = start_addr;
    for raw_line in text.lines() {
        let line = match raw_line.find("//") {
            Some(idx) => &raw_line[..idx],
            None => raw_line,
        };
        for token in line.split_whitespace() {
            if let Some(hex) = token.strip_prefix('@') {
                addr = usize::from_str_radix(hex, 16)
                    .map_err(|e| FileError::MemParse { path: "<memory text>".to_string(), reason: format!("bad @address `{hex}`: {e}") })?;
                continue;
            }
            let cleaned: String = token.chars().filter(|&c| c != '_').collect();
            let base = match radix {
                MemRadix::Binary => 2,
                MemRadix::Hex => 16,
            };
            let value = if cleaned.chars().all(|c| matches!(c, 'x' | 'X' | 'z' | 'Z')) {
                0u64
            } else {
                u64::from_str_radix(&cleaned.replace(['x', 'X', 'z', 'Z'], "0"), base)
                    .map_err(|e| FileError::MemParse { path: "<memory text>".to_string(), reason: format!("bad {radix:?} token `{token}`: {e}") })?
            };
            if addr >= mem.len() {
                return Err(FileError::MemParse {
                    path: "<memory text>".to_string(),
                    reason: format!("address {addr} is past the target memory's length {}", mem.len()),
                });
            }
            mem[addr] = value;
            addr += 1;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// McdTable
// ---------------------------------------------------------------------------

/// `vpi_mcd_open`/`vpi_mcd_close`/`vpi_mcd_printf`/`vpi_mcd_name`'s native
/// model (LRM §11.4 Table 11-10, T63/LRMC-C10-14): a multi-channel
/// descriptor is a 32-bit mask where each set bit names one channel — bit
/// 0 is always the standard-output channel (permanently open, never
/// allocated or closed, matching 1364's own MCD convention), the
/// remaining 31 bits are allocated one-per-open-file over the *same*
/// [`FileTable`] every `$fopen`-family task already uses (no separate
/// file-handle universe). Writing to a combined mask writes to every
/// channel named by its set bits — the multi-channel part of "multi-
/// channel descriptor".
#[derive(Debug, Default)]
pub struct McdTable {
    /// bit index (1..=30) -> the `FileId` bound to that channel.
    channels: HashMap<u32, FileId>,
    next_bit: u32,
}

impl McdTable {
    /// The standard-output channel's mask — always open, always includes
    /// bit 0.
    pub const STDOUT: u32 = 1;

    /// The highest allocatable bit (31 total channel bits: bit 0 is
    /// stdout, bits 1..=30 are `open`-allocatable, matching the 1364
    /// convention that the top bit unused so file-mask values stay
    /// positive as a 31-bit `int`).
    const MAX_BIT: u32 = 30;

    pub fn new() -> Self {
        Self { channels: HashMap::new(), next_bit: 1 }
    }

    /// `vpi_mcd_open()`: opens `path` (via `files`) and allocates it the
    /// next free channel bit. Returns the single-bit mask naming that new
    /// channel — combine with other masks (bitwise OR) to write to
    /// several channels with one `mcd_printf`.
    pub fn open(&mut self, files: &mut FileTable, path: &str) -> Result<u32, FileError> {
        if self.next_bit > Self::MAX_BIT {
            return Err(FileError::TooManyChannels(Self::MAX_BIT));
        }
        let id = files.open(path)?;
        let bit = self.next_bit;
        self.next_bit += 1;
        self.channels.insert(bit, id);
        Ok(1u32 << bit)
    }

    /// `vpi_mcd_close()`: closes every file-backed channel named by a set
    /// bit in `mcd` (bit 0/stdout is silently skipped — it can never be
    /// closed). Fails loud with the first `FileError` a real close hits,
    /// same as `FileTable::close`'s own contract.
    pub fn close(&mut self, files: &mut FileTable, mcd: u32) -> Result<(), FileError> {
        for bit in 1..=Self::MAX_BIT {
            if mcd & (1 << bit) != 0 {
                if let Some(id) = self.channels.remove(&bit) {
                    files.close(id)?;
                }
            }
        }
        Ok(())
    }

    /// `vpi_mcd_printf()`: writes `text` to every channel named by a set
    /// bit in `mcd` — bit 0 through `sink` (the same `SimOutput` a plain
    /// `vpi_printf()`/`$display` write goes through), every other set bit
    /// through its bound `FileTable` entry. A bit with no bound channel
    /// (never allocated, or already closed) is silently skipped — matching
    /// 1364's own "an invalid bit in the mask is ignored" MCD semantics,
    /// not a per-bit error return.
    pub fn printf(&self, files: &mut FileTable, sink: &dyn SimOutput, mcd: u32, text: &str) {
        if mcd & Self::STDOUT != 0 {
            sink.write(Channel::Stdout, text);
        }
        for bit in 1..=Self::MAX_BIT {
            if mcd & (1 << bit) != 0 {
                if let Some(&id) = self.channels.get(&bit) {
                    let _ = files.write(id, text);
                }
            }
        }
    }

    /// `vpi_mcd_name()`: the path bound to `mcd`'s lowest set file-backed
    /// bit (a real MCD is usually a single-bit mask in practice — 1364's
    /// own routine takes one descriptor's name at a time). `None` for the
    /// stdout bit (no path) or a mask with no allocated file bit set.
    pub fn name(&self, mcd: u32) -> Option<(u32, FileId)> {
        for bit in 1..=Self::MAX_BIT {
            if mcd & (1 << bit) != 0 {
                if let Some(&id) = self.channels.get(&bit) {
                    return Some((bit, id));
                }
            }
        }
        None
    }

    /// `vpi_mcd_flush()`-equivalent (1364 also exposes this as
    /// `$fflush(mcd)` over a channel mask): flushes every channel named by
    /// a set bit in `mcd`.
    pub fn flush(&self, files: &mut FileTable, sink: &dyn SimOutput, mcd: u32) {
        if mcd & Self::STDOUT != 0 {
            sink.flush();
        }
        for bit in 1..=Self::MAX_BIT {
            if mcd & (1 << bit) != 0 {
                if let Some(&id) = self.channels.get(&bit) {
                    let _ = files.flush(id);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_sink_captures_stdout_and_stderr_deterministically() {
        let sink = BufferSink::new();
        sink.write(Channel::Stdout, "hello ");
        sink.write(Channel::Stdout, "world\n");
        sink.write(Channel::Stderr, "warn: x\n");
        assert_eq!(sink.stdout(), "hello world\n");
        assert_eq!(sink.stderr(), "warn: x\n");
    }

    #[test]
    fn buffer_sink_preserves_arrival_order_across_interleaved_writes() {
        let sink = BufferSink::new();
        sink.write(Channel::Stdout, "1");
        sink.write(Channel::Stdout, "2");
        sink.write(Channel::Stdout, "3");
        assert_eq!(sink.stdout(), "123");
    }

    #[test]
    fn mcd_printf_writes_to_stdout_bit_through_the_shared_sink() {
        let sink = BufferSink::new();
        let mut files = FileTable::new();
        let mcd = McdTable::new();
        mcd.printf(&mut files, &sink, McdTable::STDOUT, "hello\n");
        assert_eq!(sink.stdout(), "hello\n");
    }

    #[test]
    fn mcd_open_allocates_a_distinct_bit_per_channel() {
        let mut files = FileTable::new();
        let mut mcd = McdTable::new();
        let dir = std::env::temp_dir();
        let a = mcd.open(&mut files, dir.join("mcd_a.txt").to_str().unwrap()).unwrap();
        let b = mcd.open(&mut files, dir.join("mcd_b.txt").to_str().unwrap()).unwrap();
        assert_ne!(a, b, "each mcd_open call gets its own bit");
        assert_eq!(a & b, 0, "distinct channels never share a bit");
        assert_eq!(a & McdTable::STDOUT, 0, "an allocated channel never reuses the stdout bit");

        mcd.close(&mut files, a | b).unwrap();
        let _ = std::fs::remove_file(dir.join("mcd_a.txt"));
        let _ = std::fs::remove_file(dir.join("mcd_b.txt"));
    }

    #[test]
    fn mcd_printf_writes_to_every_bit_in_a_combined_mask() {
        let sink = BufferSink::new();
        let mut files = FileTable::new();
        let mut mcd = McdTable::new();
        let dir = std::env::temp_dir();
        let path = dir.join("mcd_combined.txt");
        let _ = std::fs::remove_file(&path);
        let file_bit = mcd.open(&mut files, path.to_str().unwrap()).unwrap();

        let combined = McdTable::STDOUT | file_bit;
        mcd.printf(&mut files, &sink, combined, "both channels\n");
        mcd.flush(&mut files, &sink, combined);

        assert_eq!(sink.stdout(), "both channels\n");
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "both channels\n");

        mcd.close(&mut files, file_bit).unwrap();
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mcd_close_never_touches_the_stdout_bit() {
        let mut files = FileTable::new();
        let mut mcd = McdTable::new();
        // Closing bit 0 (stdout) alone must succeed as a no-op — no
        // FileTable entry exists for it to fail against.
        assert!(mcd.close(&mut files, McdTable::STDOUT).is_ok());
    }

    #[test]
    fn file_table_open_failure_is_typed_not_a_silent_descriptor() {
        let mut table = FileTable::new();
        // A path inside a directory that does not exist can never open.
        let result = table.open("/nonexistent/deeply/nested/path/for/lrmc/test.txt");
        match result {
            Err(FileError::OpenFailed { .. }) => {}
            other => panic!("expected FileError::OpenFailed, got {other:?}"),
        }
    }

    #[test]
    fn file_table_round_trips_write_and_read() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_filetable_test_{}.txt", std::process::id()));
        let path_str = path.to_str().unwrap();
        let mut table = FileTable::new();
        let id = table.open(path_str).expect("open succeeds");
        table.write(id, "hello file table\n").expect("write succeeds");
        let contents = table.read_to_string(id).expect("read succeeds");
        assert_eq!(contents, "hello file table\n");
        table.close(id).expect("close succeeds");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_table_operations_on_unknown_descriptor_are_typed() {
        let mut table = FileTable::new();
        let bogus = FileId(9999);
        assert!(matches!(table.close(bogus), Err(FileError::UnknownDescriptor(_))));
        assert!(matches!(table.write(bogus, "x"), Err(FileError::UnknownDescriptor(_))));
        assert!(matches!(table.read_to_string(bogus), Err(FileError::UnknownDescriptor(_))));
    }

    #[test]
    fn ungetc_pushback_is_returned_before_the_file_resumes() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_ungetc_test_{}.txt", std::process::id()));
        let mut table = FileTable::new();
        let id = table.open(path.to_str().unwrap()).expect("open");
        table.write(id, "AB").expect("write");
        table.rewind(id).expect("rewind");
        assert_eq!(table.getc(id).unwrap(), Some(b'A'));
        table.ungetc(id, b'Z').expect("ungetc");
        assert_eq!(table.getc(id).unwrap(), Some(b'Z'), "pushback comes back before the file resumes");
        assert_eq!(table.getc(id).unwrap(), Some(b'B'), "file position was untouched by the pushback");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fgets_reads_one_line_including_the_newline() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_fgets_test_{}.txt", std::process::id()));
        let mut table = FileTable::new();
        let id = table.open(path.to_str().unwrap()).expect("open");
        table.write(id, "line one\nline two\nno trailing newline").expect("write");
        table.rewind(id).expect("rewind");
        assert_eq!(table.fgets(id).unwrap(), Some("line one\n".to_string()));
        assert_eq!(table.fgets(id).unwrap(), Some("line two\n".to_string()));
        assert_eq!(table.fgets(id).unwrap(), Some("no trailing newline".to_string()));
        assert_eq!(table.fgets(id).unwrap(), None, "EOF with nothing left returns None, not an empty string");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fread_reads_up_to_n_bytes_and_stops_short_at_eof() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_fread_test_{}.txt", std::process::id()));
        let mut table = FileTable::new();
        let id = table.open(path.to_str().unwrap()).expect("open");
        table.write(id, "abc").expect("write");
        table.rewind(id).expect("rewind");
        assert_eq!(table.fread(id, 2).unwrap(), b"ab".to_vec());
        assert_eq!(table.fread(id, 10).unwrap(), b"c".to_vec(), "fewer than n bytes returned at EOF, never padded");
        let _ = std::fs::remove_file(&path);
    }

    /// `$readmemh`: a hex-radix memory file loads into a real array a
    /// "model" (here, a plain `Vec<u64>` standing in for a `reg` array)
    /// would read from.
    #[test]
    fn readmemh_loads_a_dense_array_in_hex() {
        let mut mem = vec![0u64; 4];
        parse_memory_text("DE AD be ef", MemRadix::Hex, &mut mem, 0).expect("parses");
        assert_eq!(mem, vec![0xDE, 0xAD, 0xbe, 0xef]);
    }

    /// `$readmemb`: binary radix, `//` comments, and an `@addr` directive
    /// that jumps the write cursor — the LRM's own worked-example shape.
    #[test]
    fn readmemb_honors_comments_and_address_directives() {
        let mut mem = vec![0xFFu64; 8];
        let text = "// header comment\n101 // three\n@4\n110\n111\n";
        parse_memory_text(text, MemRadix::Binary, &mut mem, 0).expect("parses");
        assert_eq!(mem[0], 0b101, "first token loads at the start address");
        assert_eq!(mem[1], 0xFF, "untouched slot between the dense run and the @4 jump stays as-is");
        assert_eq!(mem[4], 0b110, "@4 jumps the write cursor");
        assert_eq!(mem[5], 0b111);
    }

    /// End to end (Done-when: "`$readmemh` loads an array used by the
    /// model"): a real file on disk loads through `FileTable::readmem`
    /// into a `Vec<u64>` standing in for a model's declared memory array
    /// — the same array shape a `reg [7:0] mem [0:3];` declaration would
    /// back.
    #[test]
    fn file_table_readmem_loads_a_real_file_into_a_model_style_array() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_readmemh_test_{}.txt", std::process::id()));
        std::fs::write(&path, "CA FE // two words\nBA BE\n").expect("write fixture");

        let mut table = FileTable::new();
        let id = table.open(path.to_str().unwrap()).expect("open");
        let mut mem = vec![0u64; 4]; // the model's array
        table.readmem(id, MemRadix::Hex, &mut mem, 0).expect("readmem");
        assert_eq!(mem, vec![0xCA, 0xFE, 0xBA, 0xBE], "the model's array now holds the file's contents");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn readmem_out_of_range_address_is_a_typed_error_not_a_panic() {
        let mut mem = vec![0u64; 2];
        let err = parse_memory_text("1 2 3", MemRadix::Hex, &mut mem, 0).unwrap_err();
        assert!(matches!(err, FileError::MemParse { .. }), "expected FileError::MemParse, got {err:?}");
    }

    #[test]
    fn readmem_malformed_digit_is_a_typed_error_not_a_panic() {
        let mut mem = vec![0u64; 2];
        let err = parse_memory_text("not_hex", MemRadix::Hex, &mut mem, 0).unwrap_err();
        assert!(matches!(err, FileError::MemParse { .. }), "expected FileError::MemParse, got {err:?}");
    }

    #[test]
    fn file_table_descriptor_is_unusable_after_close() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("lrmc_filetable_close_test_{}.txt", std::process::id()));
        let path_str = path.to_str().unwrap();
        let mut table = FileTable::new();
        let id = table.open(path_str).expect("open succeeds");
        table.close(id).expect("close succeeds");
        assert!(!table.is_open(id));
        assert!(matches!(table.write(id, "x"), Err(FileError::UnknownDescriptor(_))));
        let _ = std::fs::remove_file(&path);
    }
}
