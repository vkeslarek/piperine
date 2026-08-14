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
}

struct FileEntry {
    path: String,
    file: File,
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
        self.open.insert(id, FileEntry { path: path.to_string(), file });
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
