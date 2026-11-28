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

use piperine_api::{Cell, Overrides, Project, Session, Table};
use piperine_solver::protocol::{InstantCause, StepEnd};
use piperine_transpile::Settings;
use piperine_parity::sinks::Ignore;

/// The largest difference of `buck.vout` the run may have against the fine reference, in volts.
const VOUT_BOUND: f64 = 0.104e-3;

/// The most landings the first 100 us may have (TP-03 AC 6).
const LANDING_BOUND: u64 = 1000;

/// The most instants the first 100 us may run at accepted steps (TP-08 AC 2): the landings
/// (417 measured) and the instants scheduled at a step's end, never one per accepted step (127 611
/// before the idle rule, 31 264 steps now). 668 measured.
const INSTANT_BOUND: u64 = 1000;

/// One run of the first 100 us.
struct Run {
    /// The rows the run printed.
    table: Table,
    /// The accepted steps of the run.
    accepted_steps: u64,
    /// The accepted steps that ended on a landing, whatever its cause.
    landings: u64,
    /// The discrete instants the protocol ran at accepted steps, with a crossing fired there.
    crossing_instants: u64,
    /// The discrete instants it ran at accepted steps with no crossing, for what the discrete
    /// side held at the time.
    scheduled_instants: u64,
    /// The counts of `diagnosis.md` section 2, for the table of its "After".
    summary: String,
}

impl Run {
    /// The run of the first 100 us with `max_step`.
    fn with_max_step(max_step: f64) -> Run {
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
        let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        let steps = session.statistics().expect("a transient leaves its statistics").steps();
        let landings = StepEnd::ALL.iter().filter(|end| matches!(end, StepEnd::Landing(_))).map(|end| steps.ended(*end)).sum();
        let analog = session.statistics().expect("a transient leaves its statistics").analog().clone();
        let summary = format!(
            "{} rejected by truncation, {} by Newton, {} retaken toward a crossing, {} ended on the controller's proposal, {} at max_step, {} jump steps",
            analog.rejected_truncation, analog.rejected_newton, steps.retaken(), steps.ended(StepEnd::Proposal), steps.ended(StepEnd::MaxStep), analog.jump_steps
        );
        Run { table, accepted_steps: steps.accepted(), landings, crossing_instants: steps.instants_for(InstantCause::Crossing), scheduled_instants: steps.instants_for(InstantCause::Scheduled), summary }
    }

    /// The two runs, made once for every test of this target: the run as the buck is run, and the
    /// reference with a hundredth of the step.
    fn pair() -> &'static (Run, Run) {
        static RUNS: OnceLock<(Run, Run)> = OnceLock::new();
        RUNS.get_or_init(|| {
            let (run, reference) = (Run::with_max_step(1e-8), Run::with_max_step(1e-10));
            eprintln!("buck accuracy: {} accepted steps ({} landings, {} instants run for crossings, {} scheduled) at max_step 1e-8, {} at 1e-10", run.accepted_steps, run.landings, run.crossing_instants, run.scheduled_instants, reference.accepted_steps);
            eprintln!("buck accuracy: {}", run.summary);
            (run, reference)
        })
    }
}

/// The columns of a printed table.
struct Columns;

impl Columns {
    /// The index of the column whose label is `name` or ends in `.name`.
    fn index(table: &Table, name: &str) -> usize {
        let dotted = format!(".{name}");
        table.columns.iter().position(|c| c == name || c.ends_with(&dotted)).unwrap_or_else(|| panic!("no column `{name}` in {:?}", table.columns))
    }
}

#[test]
fn the_first_100_us_match_the_fine_reference_within_0_104_mv() {
    let (run, reference) = Run::pair();
    assert_eq!(run.table.rows.len(), reference.table.rows.len(), "the same 0.5 us grid");
    let at = Columns::index(&run.table, "vout");
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
    let (run, reference) = Run::pair();
    for name in ["reference", "mode"] {
        let at = Columns::index(&run.table, name);
        for (index, (row, fine)) in run.table.rows.iter().zip(&reference.table.rows).enumerate() {
            assert_eq!(row[at], fine[at], "{name} at row {index}");
        }
    }
}

#[test]
fn the_first_100_us_land_at_most_1000_times() {
    // TP-03 AC 6: only the instants something observes are landings
    let (run, _) = Run::pair();
    eprintln!("buck landings: {} in the first 100 us (limit {LANDING_BOUND})", run.landings);
    assert!(run.landings <= LANDING_BOUND, "{} landings in the first 100 us, limit {LANDING_BOUND}", run.landings);
}

#[test]
fn the_first_100_us_run_an_instant_only_where_something_is_due() {
    // TP-08 AC 2: an instant runs for a crossing or for a time the discrete side holds something
    // at, and a landing is the latter (a step that ends where nothing is due runs none)
    let (run, _) = Run::pair();
    let instants = run.crossing_instants + run.scheduled_instants;
    eprintln!(
        "buck instants: {instants} run in the first 100 us for {} accepted steps ({} for crossings, {} scheduled, {} landings; limit {INSTANT_BOUND})",
        run.accepted_steps, run.crossing_instants, run.scheduled_instants, run.landings
    );
    assert!(run.landings <= instants, "every landing runs its instant: {} landings, {instants} instants", run.landings);
    assert!(instants <= INSTANT_BOUND, "{instants} instants for {} accepted steps, limit {INSTANT_BOUND}", run.accepted_steps);
}
