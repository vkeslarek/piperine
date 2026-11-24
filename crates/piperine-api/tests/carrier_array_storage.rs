//! An array of carriers costs what is written (composite-values CV-27 AC 1, part II §5.1.8): `signal mem: [Bit<u8>; 1 << 32]`
//! with a thousand distinct elements committed holds the same peak live heap as `1 << 16` elements, measured by a counting
//! allocator around `Session::run`, and a positive control shows the meter sees the thousand written elements.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use piperine_api::{Overrides, Project, Record, RecordSink, Session};
use piperine_transpile::Settings;

#[path = "support/heap.rs"]
mod heap;

#[global_allocator]
static HEAP: heap::Counting = heap::Counting::new();

/// A sink that drops every record: the table is not what is measured.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// One design of an array of `length` carriers that commits `writes` distinct elements, one per rising edge.
struct Store {
    length: u64,
    writes: u32,
}

impl Store {
    /// The MSL source: a 1 GHz oscillator, a counter and the memory written at element `n * 61 % 65536` for `n < writes`.
    fn source(&self) -> String {
        format!(
            "pub module Osc(out clk: Bit = false) {{ f: f64 = 200M where f > 0.0; }}\n\
             digital for Osc {{ on clk.changed() {{ clk <- !clk after 0.5 / f; }} }}\n\
             module Memory(in clk: Bit) {{\n    signal n: Bit<u16> = 0;\n    signal mem: [Bit<u8>; {length}] = [0; {length}];\n}}\n\
             digital for Memory {{ on clk.rise() {{ n <- n + 1; if n < {writes} {{ mem[(n as u32) * 61 % 65536] <- 1; }} }} }}\n\
             module Bench() {{ signal clk: Bit; let osc = Osc(clk) {{ f: 1G }}; let memory = Memory(clk); }}\n",
            length = self.length,
            writes = self.writes
        )
    }

    /// The peak live heap of the transient of this design, 1.1 us long.
    fn peak(&self, directory: &Path, cache: &Path) -> usize {
        let file = directory.join("store.msl");
        std::fs::write(&file, self.source()).expect("write the model");
        let project = Project::file(&file).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides { analysis: Some("transient".into()), top: Some("Bench".into()), stop: Some(1.1e-6), print_step: Some(1e-7), max_step: Some(1e-9), ..Overrides::default() };
        let spec = project.request(None, &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
        let base = HEAP.begin();
        session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        HEAP.peak_since(base)
    }
}

#[test]
fn peak_heap_does_not_depend_on_the_declared_length() {
    // CV-27 AC 1: the peak of `1 << 32` elements and of `1 << 16`, a thousand distinct elements written in each, differ by
    // at most 256 KiB (a million-fold longer array that allocated per element would differ by gigabytes)
    let (directory, cache) = (tempfile::tempdir().expect("tmp"), tempfile::tempdir().expect("tmp"));
    let huge = Store { length: 1 << 32, writes: 1000 }.peak(directory.path(), cache.path());
    let small = Store { length: 1 << 16, writes: 1000 }.peak(directory.path(), cache.path());
    eprintln!("peak live heap: 1<<32 {huge} B, 1<<16 {small} B, difference {} B", huge.abs_diff(small));
    assert!(huge.abs_diff(small) <= 256 * 1024, "peaks {huge} and {small} differ by {} B", huge.abs_diff(small));
    // the positive control: with nothing written the same array holds less, so the meter sees the thousand elements (at least 64 B each)
    let idle = Store { length: 1 << 32, writes: 0 }.peak(directory.path(), cache.path());
    assert!(huge > idle + 64 * 1000, "a thousand written elements raise the peak by at least 64 B each: {huge} B written, {idle} B idle");
}
