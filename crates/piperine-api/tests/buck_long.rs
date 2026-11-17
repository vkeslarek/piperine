//! The buck converter's whole start through the host library (feature `long-run`, LR-08 and
//! LR-12): `examples/buck`'s `startup` run to 600 us on a 10 ns grid, 60 001 rows, with the
//! monitors' `checks` records counted as they arrive. One run serves every test of the target;
//! the counting allocator measures its peak live heap and the run's wall time is printed. The
//! limits are `examples/buck/SPEC.md`'s and the run's own shape, never a value read off a run.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

#[path = "support/heap.rs"]
mod heap;

#[global_allocator]
static HEAP: heap::Counting = heap::Counting::new();

/// The `checks` records of the run: how many, and how many did not pass.
#[derive(Default)]
struct Checks {
    seen: usize,
    failed: usize,
}

impl RecordSink for Checks {
    fn record(&mut self, record: &Record) {
        if record.stream == "checks" {
            self.seen += 1;
            if !record.is_passing_check() {
                self.failed += 1;
            }
        }
    }
}

/// What the one run left.
struct Start {
    table: Table,
    checks: Checks,
    /// The peak of live heap during `Session::run`, less the heap live when it began.
    peak: usize,
    wall: Duration,
}

/// The one run of the whole start of this target.
fn start() -> &'static Start {
    static START: OnceLock<Start> = OnceLock::new();
    START.get_or_init(|| {
        let buck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck");
        let cache = tempfile::tempdir().expect("tmp");
        let project = Project::at(&buck).unwrap_or_else(|f| panic!("{f}"));
        let overrides = Overrides {
            print_step: Some(1e-8),
            print: ["buck.vout", "buck.ctl.reference", "mode", "pgood"].map(String::from).to_vec(),
            ..Overrides::default()
        };
        let spec = project.request(Some("startup"), &overrides).unwrap_or_else(|f| panic!("{f}"));
        let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
        let checks = Rc::new(RefCell::new(Checks::default()));
        let base = HEAP.begin();
        let began = Instant::now();
        let table = session.run(checks.clone()).unwrap_or_else(|f| panic!("{f}"));
        let wall = began.elapsed();
        let peak = HEAP.peak_since(base);
        let checks = std::mem::take(&mut *checks.borrow_mut());
        let start = Start { table, checks, peak, wall };
        eprintln!("{}", report(&start));
        start
    })
}

/// The line the run reports: its wall time and its peak live heap.
fn report(start: &Start) -> String {
    format!("buck whole start: {} rows, wall time {:.1} s, peak live heap {} B ({:.1} MiB)", start.table.rows.len(), start.wall.as_secs_f64(), start.peak, start.peak as f64 / (1024.0 * 1024.0))
}

#[test]
fn the_whole_start_is_60001_rows_on_a_10_ns_grid() {
    // LR-08 AC 1: 0 to 600 us on a 10 ns grid, the time of row k being k * 1e-8
    let table = &start().table;
    assert_eq!(table.rows.len(), 60_001, "{:?}", table.columns);
    for (k, row) in table.rows.iter().enumerate() {
        assert_eq!(row[0], Cell::Real(k as f64 * 1e-8), "row {k}");
    }
}

#[test]
fn every_check_of_the_whole_start_passes() {
    // LR-08 AC 2: the monitors check at every clock, so there are checks, and none fails
    let checks = &start().checks;
    assert!(checks.seen > 0, "no `checks` record in the whole start");
    assert_eq!(checks.failed, 0, "{} of {} checks failed", checks.failed, checks.seen);
}

#[test]
fn the_whole_start_stays_under_one_gibibyte() {
    // LR-12 AC 1
    let start = start();
    assert!(start.peak <= 1 << 30, "peak live heap {} B", start.peak);
}

#[test]
fn the_wall_time_and_the_peak_are_reported() {
    // LR-12 AC 2: the line holds both numbers and nothing asserts a time
    let start = start();
    let line = report(start);
    eprintln!("{line}");
    assert!(line.contains(&format!("{:.1} s", start.wall.as_secs_f64())), "{line}");
    assert!(line.contains(&format!("{} B", start.peak)), "{line}");
}
