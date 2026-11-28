//! A run's memory does not depend on its length (feature `long-run`, LR-04): `examples/rc`
//! to 5 us and to 50 us, each on 101 rows, peak live heap measured by a counting allocator
//! around `Session::run`. The two peaks differ by at most 256 KiB, a bound that a run keeping
//! every accepted step would break (ten times the steps).
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use piperine_api::{Overrides, Project, Session};
use piperine_transpile::Settings;
use piperine_parity::sinks::Ignore;

use crate::HEAP;

/// Associated test helpers of `memory`, grouped on one fixture type.
struct MemoryFixture;

impl MemoryFixture {
    /// The rows and the peak live heap of `examples/rc`'s `step` run to `stop` on `print_step`.
    fn measured(stop: f64, print_step: f64, cache: &Path) -> (usize, usize) {
        let rc = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/rc");
        let project = Project::at(&rc).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides {
            stop: Some(stop),
            print_step: Some(print_step),
            max_step: Some(1e-8),
            print: vec!["vin".into(), "vout".into()],
            ..Overrides::default()
        };
        let spec = project.request(Some("step"), &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
        let base = HEAP.begin();
        let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        let peak = HEAP.peak_since(base);
        (table.rows.len(), peak)
    }
}

#[test]
fn a_run_ten_times_longer_at_equal_rows_holds_no_more_memory() {
    let _turn = HEAP.alone();
    // LR-04 AC 2: both tables have 101 rows; AC 1: the peaks differ by at most 256 KiB
    let cache = tempfile::tempdir().expect("tmp");
    let (short_rows, short) = MemoryFixture::measured(5e-6, 5e-8, cache.path());
    let (long_rows, long) = MemoryFixture::measured(50e-6, 5e-7, cache.path());
    eprintln!("peak live heap: 5 us {short} B, 50 us {long} B, difference {} B", long.abs_diff(short));
    assert_eq!(short_rows, 101, "the 5 us table");
    assert_eq!(long_rows, 101, "the 50 us table");
    assert!(long.abs_diff(short) <= 256 * 1024, "peaks {short} and {long} differ by {} B", long.abs_diff(short));
}
