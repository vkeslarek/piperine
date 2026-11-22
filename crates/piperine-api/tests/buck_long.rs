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

/// The index of the column whose label is `name` or ends in `.name`.
fn column(table: &Table, name: &str) -> usize {
    let dotted = format!(".{name}");
    table.columns.iter().position(|c| c == name || c.ends_with(&dotted)).unwrap_or_else(|| panic!("no column `{name}` in {:?}", table.columns))
}

/// The real cells of a column, with the time of each row.
fn series(table: &Table, name: &str) -> Vec<(f64, f64)> {
    let at = column(table, name);
    table
        .rows
        .iter()
        .map(|row| match (&row[0], &row[at]) {
            (Cell::Real(t), Cell::Real(v)) => (*t, *v),
            other => panic!("a real `{name}` at a real time, found {other:?}"),
        })
        .collect()
}

#[test]
fn the_overshoot_is_within_15_mv() {
    // LR-09 AC 1, SPEC §10 S1: the output never exceeds 0.8 V by more than 15 mV
    let vout = series(&start().table, "vout");
    let (time, peak) = vout.iter().copied().fold((0.0, f64::MIN), |best, (t, v)| if v > best.1 { (t, v) } else { best });
    eprintln!("overshoot: largest buck.vout {peak:.6} V at {time:e} s (limit 0.815 V)");
    assert!(peak <= 0.815, "the largest buck.vout is {peak} V at {time:e} s, limit 0.815 V");
}

#[test]
fn the_output_settles_into_10_mv_within_the_ramp_window() {
    // LR-10 AC 1, SPEC §2 (regulation of 10 mV) and §10 S1: `en` rises at 1.5 us
    // (`benches/startup.msl`) and the reference ramps over 0.256 ms (§7.2), so the output is
    // inside 0.8 V +- 10 mV from 0.2575 ms +- 0.05 ms; the settling time is the time of the first
    // row from which every later row is inside the band
    let vout = series(&start().table, "vout");
    let inside = |v: f64| (v - 0.8).abs() <= 10e-3;
    let last_outside = vout.iter().rposition(|(_, v)| !inside(*v));
    let settled = match last_outside {
        None => vout[0].0,
        Some(k) if k + 1 < vout.len() => vout[k + 1].0,
        Some(_) => f64::INFINITY,
    };
    eprintln!("settling: buck.vout inside 0.8 V +- 10 mV from {settled:e} s (window 207.5 to 307.5 us)");
    assert!(settled >= 207.5e-6, "settled at {settled:e} s, before 207.5 us");
    assert!(settled <= 307.5e-6, "settled at {settled:e} s, after 307.5 us");
}

#[test]
fn the_start_ends_in_regulation_with_power_good() {
    // LR-10 AC 2: the last row holds reference code 128 (SPEC §7.2: 0.8 V is code 0, reference
    // 128), supervisor mode `Regulate` (§8, variant 2) and `pgood` true (§8.6)
    let table = &start().table;
    let last = table.rows.last().expect("rows");
    let (reference, mode, pgood) = (&last[column(table, "reference")], &last[column(table, "mode")], &last[column(table, "pgood")]);
    eprintln!("end state: reference {reference:?}, mode {mode:?}, pgood {pgood:?} at {:?}", last[0]);
    assert_eq!(reference, &Cell::Int(128), "reference at the end");
    assert_eq!(mode, &Cell::Text("Regulate".into()), "mode at the end (read by its variant name, CV-22)");
    assert_eq!(pgood, &Cell::Bool(true), "pgood at the end");
}

#[test]
fn the_ripple_is_within_10_mv_peak_to_peak() {
    // LR-11 AC 1, SPEC §2: over the last 40 us the output moves, and by at most 10 mV peak to peak
    let vout = series(&start().table, "vout");
    let window: Vec<f64> = vout.iter().filter(|(t, _)| *t >= 560e-6).map(|(_, v)| *v).collect();
    let (low, high) = window.iter().fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
    let ripple = high - low;
    eprintln!("ripple: {:.6} mV peak to peak over {} rows of the last 40 us (limit 10 mV)", ripple * 1e3, window.len());
    assert!(ripple > 0.0, "the output does not move over the last 40 us: {ripple} V");
    assert!(ripple <= 10e-3, "ripple {ripple} V peak to peak over the last 40 us, limit 10 mV");
}
