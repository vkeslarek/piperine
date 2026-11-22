//! The buck converter's accuracy against a fine reference (feature `transient-performance`,
//! TP-05 AC 5): the first 100 us of `examples/buck`'s `startup` run at `max_step = 1e-8` and at
//! `max_step = 1e-10`, `buck.vout` compared on the 0.5 us print grid. The largest difference is
//! at most 0.104 mV, the difference of the solver before this feature (`diagnosis.md` section 4,
//! row 0), so the speed of the feature is not bought with accuracy; the digital columns are equal
//! row for row. The run lands at most 1 000 times (TP-03 AC 6; 39 999 before the observable rule)
//! and runs at most 1 000 instants at its accepted steps (TP-08 AC 2).
//! The bounds are never moved to fit a run.
#![cfg(unix)]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_solver::protocol::StepEnd;
use piperine_transpile::Settings;

/// The largest difference of `buck.vout` the run may have against the fine reference, in volts.
const VOUT_BOUND: f64 = 0.104e-3;

/// The most landings the first 100 us may have (TP-03 AC 6).
const LANDING_BOUND: u64 = 1000;

/// The most instants the first 100 us may run at accepted steps (TP-08 AC 2): the landings
/// (417 measured) and the instants scheduled at a step's end, never one per accepted step (127 611
/// before the idle rule, 31 264 steps now). 668 measured.
const INSTANT_BOUND: u64 = 1000;

/// A sink that drops every record: the comparison reads the table only.
struct Discard;

impl RecordSink for Discard {
    fn record(&mut self, _record: &Record) {}
}

/// One run of the first 100 us.
struct Run {
    table: Table,
    accepted_steps: u64,
    /// The accepted steps that ended on a landing, whatever its cause.
    landings: u64,
    /// The discrete instants the protocol ran at accepted steps.
    instants: u64,
}

/// The run of the first 100 us with `max_step`.
fn run_with(max_step: f64) -> Run {
    let buck = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck");
    let cache = tempfile::tempdir().expect("tmp");
    let project = Project::at(&buck).unwrap_or_else(|f| panic!("{f}"));
    let overrides = Overrides {
        stop: Some(100e-6),
        max_step: Some(max_step),
        print_step: Some(0.5e-6),
        print: ["buck.vout", "buck.ctl.reference", "mode"].map(String::from).to_vec(),
        ..Overrides::default()
    };
    let spec = project.request(Some("startup"), &overrides).unwrap_or_else(|f| panic!("{f}"));
    let mut session = Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|f| panic!("{f}"));
    let table = session.run(Rc::new(RefCell::new(Discard))).unwrap_or_else(|f| panic!("{f}"));
    let steps = session.statistics().expect("a transient leaves its statistics").steps();
    let landings = StepEnd::ALL.iter().filter(|end| matches!(end, StepEnd::Landing(_))).map(|end| steps.ended(*end)).sum();
    Run { table, accepted_steps: steps.accepted(), landings, instants: steps.instants_run() }
}

/// The two runs, made once for every test of this target: the run as the buck is run, and the
/// reference with a hundredth of the step.
fn runs() -> &'static (Run, Run) {
    static RUNS: OnceLock<(Run, Run)> = OnceLock::new();
    RUNS.get_or_init(|| {
        let (run, reference) = (run_with(1e-8), run_with(1e-10));
        eprintln!("buck accuracy: {} accepted steps ({} landings, {} instants run) at max_step 1e-8, {} at 1e-10", run.accepted_steps, run.landings, run.instants, reference.accepted_steps);
        (run, reference)
    })
}

/// The index of the column whose label is `name` or ends in `.name`.
fn column(table: &Table, name: &str) -> usize {
    let dotted = format!(".{name}");
    table.columns.iter().position(|c| c == name || c.ends_with(&dotted)).unwrap_or_else(|| panic!("no column `{name}` in {:?}", table.columns))
}

#[test]
fn the_first_100_us_match_the_fine_reference_within_0_104_mv() {
    let (run, reference) = runs();
    assert_eq!(run.table.rows.len(), reference.table.rows.len(), "the same 0.5 us grid");
    let at = column(&run.table, "vout");
    let mut largest = (0.0_f64, 0.0_f64);
    for (row, fine) in run.table.rows.iter().zip(&reference.table.rows) {
        match (&row[0], &row[at], &fine[at]) {
            (Cell::Real(time), Cell::Real(value), Cell::Real(fine_value)) => {
                let difference = (value - fine_value).abs();
                if difference > largest.0 {
                    largest = (difference, *time);
                }
            }
            other => panic!("a real buck.vout at a real time, found {other:?}"),
        }
    }
    eprintln!("buck accuracy: largest buck.vout difference {:.6} mV at {:e} s (limit {} mV)", largest.0 * 1e3, largest.1, VOUT_BOUND * 1e3);
    assert!(largest.0 <= VOUT_BOUND, "buck.vout differs by {} V at {:e} s, limit {VOUT_BOUND} V", largest.0, largest.1);
}

#[test]
fn the_digital_columns_equal_the_fine_reference_row_for_row() {
    let (run, reference) = runs();
    for name in ["reference", "mode"] {
        let at = column(&run.table, name);
        for (index, (row, fine)) in run.table.rows.iter().zip(&reference.table.rows).enumerate() {
            assert_eq!(row[at], fine[at], "{name} at row {index}");
        }
    }
}

#[test]
fn the_first_100_us_land_at_most_1000_times() {
    // TP-03 AC 6: only the instants something observes are landings
    let (run, _) = runs();
    eprintln!("buck landings: {} in the first 100 us (limit {LANDING_BOUND})", run.landings);
    assert!(run.landings <= LANDING_BOUND, "{} landings in the first 100 us, limit {LANDING_BOUND}", run.landings);
}

#[test]
fn the_first_100_us_run_an_instant_only_where_something_is_due() {
    // TP-08 AC 2: a step that ends where nothing is due runs no instant
    let (run, _) = runs();
    eprintln!("buck instants: {} run in the first 100 us for {} accepted steps (limit {INSTANT_BOUND})", run.instants, run.accepted_steps);
    assert!(run.instants >= run.landings, "every landing runs its instant");
    assert!(run.instants <= INSTANT_BOUND, "{} instants for {} accepted steps, limit {INSTANT_BOUND}", run.instants, run.accepted_steps);
}
