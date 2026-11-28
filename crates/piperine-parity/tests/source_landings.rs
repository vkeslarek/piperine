//! The landing of a located crossing (docs/spec §17.3 rules 1 and 3, Appendix C.2 "crossings"),
//! run from MSL source on the Rust backend at the default 1 fs tick: a float switch on a tank
//! that fills at 0.025 V/s (`tests/landings/float_switch.msl`). A step of a few ticks moves the
//! level by less than half a unit in the last place of the threshold, so a landing whose
//! solution sits exactly on it does not cross; the crossing is then landed again, never one
//! tick at a time from a solution that cannot move (the stall of the pump-action project, where
//! the step fell to 1 fs and the 2e6-step budget ran out).
#![cfg(unix)]

use std::path::{Path, PathBuf};

use std::cell::RefCell;
use std::rc::Rc;

use piperine_parity::project::Project;
use piperine_solver::prelude::{NodeIdentifier, Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::{HIGH, LOW, build};

/// Keeps the value changes of one net in the transient: the time and the bytes of each.
struct NetTape {
    /// The net's path.
    path: &'static str,
    /// The changes kept so far, shared with the run that reads them back.
    changes: Rc<RefCell<Vec<(f64, Vec<u8>)>>>,
}

impl Sink for NetTape {
    /// Keeps `observation` when it is a change of the net in the transient.
    fn observe(&mut self, observation: &Observation) {
        if observation.path != self.path || observation.analysis != AnalysisKind::Transient {
            return;
        }
        if let ObservationKind::Change { bytes } = &observation.kind {
            self.changes.borrow_mut().push((observation.time, bytes.clone()));
        }
    }
}

/// The bench of `landings/float_switch.msl`, run to 8 s with a step cap of 50 ms.
struct FloatSwitchBench;

/// One run of the bench: each change of the float with the level at its time point.
struct FloatRun {
    /// Each change of `full`, as its time in seconds, whether it closed, and `v(lvl)` there.
    changes: Vec<(f64, bool, f64)>,
}

impl FloatSwitchBench {
    /// The float closes above this level.
    const TOP: f64 = 0.9;
    /// It opens below this level.
    const BOTTOM: f64 = 0.88;
    /// The run's stop time and step cap.
    const STOP: f64 = 8.0;
    const STEP: f64 = 0.05;
    /// A budget of attempts far above what the run needs (a few hundred), far below the
    /// default 2e6 a stall spends: a stall fails in seconds, naming the budget.
    const BUDGET: usize = 20_000;
    /// How far the level may lie past a threshold where the float changes: the slope times
    /// the default `ttol` of 1 ps plus one tick, or the `etol` of 1 uV, with room for rounding.
    const LEVEL_TOLERANCE: f64 = 1e-9;

    /// Runs the bench at the default tick and reads the float's changes.
    fn run() -> FloatRun {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/landings/float_switch.msl");
        let built = build(&Project::new(&path), "FloatSwitch");
        let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
        let changes = Rc::new(RefCell::new(Vec::new()));
        let tape = NetTape { path: "FloatSwitch.full", changes: changes.clone() };
        let mut options = TransientAnalysisOptions::new(Self::STOP, Self::STEP).with_max_steps(Self::BUDGET);
        options.dt_max = Self::STEP;
        let mut solver = Solver::new(circuit).with_tran_opts(options).with_sink(Box::new(tape)).build();
        let result = solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}"));
        let node = NodeIdentifier::Anonymous(built.ir.node_at("FloatSwitch.tank.lvl").expect("the tank's level").id().index());
        let level = |time: f64| {
            let point = result.iter().find(|point| point.time() == time).unwrap_or_else(|| panic!("no time point at {time:e} s"));
            point.get_node(&node).expect("v(lvl)")
        };
        let changes = changes
            .take()
            .into_iter()
            .map(|(time, bytes)| {
                let closed = bytes == HIGH.to_vec();
                assert!(closed || bytes == LOW.to_vec(), "a Bit is high or low: {bytes:?}");
                let at = result.iter().map(|point| point.time()).min_by(|a, b| (a - time).abs().total_cmp(&(b - time).abs())).expect("a point");
                (at, closed, level(at))
            })
            .collect();
        FloatRun { changes }
    }
}

#[test]
fn a_crossing_whose_landing_does_not_cross_is_located_again_and_fires() {
    // §17.3 rule 1: every crossing of the level is located and fires, so the run ends at its stop
    // within the budget, the float closing at 2 s (0.85 V + 2 s * 0.025 V/s) and then once per
    // cycle of the pump; rule 3: at each landing the level lies past its threshold, within the
    // level the landing tolerance allows
    let run = FloatSwitchBench::run();
    let closings: Vec<&(f64, bool, f64)> = run.changes.iter().filter(|(_, closed, _)| *closed).collect();
    let openings: Vec<&(f64, bool, f64)> = run.changes.iter().filter(|(_, closed, _)| !*closed).collect();
    assert!(closings.len() >= 4 && openings.len() >= 4, "the float cycles to 8 s: {:?}", run.changes);
    assert!((closings[0].0 - 2.0).abs() < 1e-9, "the float first closes at 2 s: {:?}", closings[0]);
    for (time, _, level) in closings {
        let past = level - FloatSwitchBench::TOP;
        assert!((0.0..FloatSwitchBench::LEVEL_TOLERANCE).contains(&past), "closed at {time:e} s with the level at {level}");
    }
    for (time, _, level) in openings {
        let past = FloatSwitchBench::BOTTOM - level;
        assert!((0.0..FloatSwitchBench::LEVEL_TOLERANCE).contains(&past), "opened at {time:e} s with the level at {level}");
    }
}
