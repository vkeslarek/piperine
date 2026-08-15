//! T63 (LRMC-C10-14): `PluginHost::mcd_open`/`mcd_close`/`mcd_printf`/
//! `mcd_flush` — `vpi_mcd_*`'s native equivalents over T4's `FileTable`
//! (`McdTable`, `piperine-solver/src/core/output.rs`) — and
//! `PluginHost::vpi_printf` routing through the same `SimOutput` sink
//! `mcd_printf`'s stdout bit uses.

use piperine_plugin::{BufferSink, FileTable, McdTable, PluginHost};

#[test]
fn vpi_printf_routes_to_the_same_sink_as_mcd_printfs_stdout_bit() {
    let host = PluginHost::empty();
    let sink = BufferSink::new();

    host.vpi_printf(&sink, "from vpi_printf\n");

    let mut files = FileTable::new();
    let mcd = McdTable::new();
    host.mcd_printf(&mcd, &mut files, &sink, McdTable::STDOUT, "from mcd_printf\n");

    assert_eq!(sink.stdout(), "from vpi_printf\nfrom mcd_printf\n", "both must land in the same sink stream");
}

/// `mcd_open`/`mcd_printf`/`mcd_flush`/`mcd_close` write to every channel
/// selected by a combined mask — stdout *and* an open file, in one call.
#[test]
fn mcd_printf_writes_to_every_selected_channel() {
    let host = PluginHost::empty();
    let sink = BufferSink::new();
    let mut files = FileTable::new();
    let mut mcd = McdTable::new();

    let path = std::env::temp_dir().join("piperine_t63_mcd_test.txt");
    let _ = std::fs::remove_file(&path);

    let file_bit = host.mcd_open(&mut mcd, &mut files, path.to_str().unwrap()).expect("mcd_open");
    let combined = McdTable::STDOUT | file_bit;

    host.mcd_printf(&mcd, &mut files, &sink, combined, "line one\n");
    host.mcd_printf(&mcd, &mut files, &sink, combined, "line two\n");
    host.mcd_flush(&mcd, &mut files, &sink, combined);

    assert_eq!(sink.stdout(), "line one\nline two\n", "stdout channel must receive every write");
    let contents = std::fs::read_to_string(&path).expect("file readable after flush");
    assert_eq!(contents, "line one\nline two\n", "file channel must receive every write too");

    host.mcd_close(&mut mcd, &mut files, file_bit).expect("mcd_close");
    let _ = std::fs::remove_file(&path);
}

/// `mcd_close` deregisters the channel — a write to a mask naming a
/// closed channel silently skips it (1364's own MCD semantics: an
/// invalid/closed bit is ignored, not an error), while the file itself
/// remains closed (no further growth).
#[test]
fn mcd_close_stops_further_writes_to_that_channel() {
    let host = PluginHost::empty();
    let sink = BufferSink::new();
    let mut files = FileTable::new();
    let mut mcd = McdTable::new();

    let path = std::env::temp_dir().join("piperine_t63_mcd_close_test.txt");
    let _ = std::fs::remove_file(&path);

    let file_bit = host.mcd_open(&mut mcd, &mut files, path.to_str().unwrap()).expect("mcd_open");
    host.mcd_printf(&mcd, &mut files, &sink, file_bit, "before close\n");
    host.mcd_flush(&mcd, &mut files, &sink, file_bit);
    host.mcd_close(&mut mcd, &mut files, file_bit).expect("mcd_close");

    // A write against the now-closed bit is silently skipped, not an error.
    host.mcd_printf(&mcd, &mut files, &sink, file_bit, "after close\n");

    let contents = std::fs::read_to_string(&path).expect("file still readable");
    assert_eq!(contents, "before close\n", "post-close write must not have reached the file");

    let _ = std::fs::remove_file(&path);
}

/// `mcd_name` returns the path bound to a file channel, and `None` for the
/// stdout bit.
#[test]
fn mcd_name_returns_the_bound_path() {
    let host = PluginHost::empty();
    let mut files = FileTable::new();
    let mut mcd = McdTable::new();

    let path = std::env::temp_dir().join("piperine_t63_mcd_name.txt");
    let _ = std::fs::remove_file(&path);
    let file_bit = host.mcd_open(&mut mcd, &mut files, path.to_str().unwrap()).unwrap();

    assert_eq!(host.mcd_name(&mcd, &files, file_bit).as_deref(), Some(path.to_str().unwrap()));
    assert_eq!(host.mcd_name(&mcd, &files, McdTable::STDOUT), None, "the stdout bit has no path");

    host.mcd_close(&mut mcd, &mut files, file_bit).unwrap();
    let _ = std::fs::remove_file(&path);
}

/// Two `mcd_open` calls get distinct, independently addressable bits.
#[test]
fn distinct_mcd_open_calls_get_distinct_channels() {
    let host = PluginHost::empty();
    let sink = BufferSink::new();
    let mut files = FileTable::new();
    let mut mcd = McdTable::new();

    let path_a = std::env::temp_dir().join("piperine_t63_mcd_a.txt");
    let path_b = std::env::temp_dir().join("piperine_t63_mcd_b.txt");
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);

    let a = host.mcd_open(&mut mcd, &mut files, path_a.to_str().unwrap()).unwrap();
    let b = host.mcd_open(&mut mcd, &mut files, path_b.to_str().unwrap()).unwrap();
    assert_ne!(a, b);

    host.mcd_printf(&mcd, &mut files, &sink, a, "only a\n");
    host.mcd_flush(&mcd, &mut files, &sink, a | b);

    assert_eq!(std::fs::read_to_string(&path_a).unwrap(), "only a\n");
    assert_eq!(std::fs::read_to_string(&path_b).unwrap(), "", "channel b received nothing");

    host.mcd_close(&mut mcd, &mut files, a | b).unwrap();
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
}
