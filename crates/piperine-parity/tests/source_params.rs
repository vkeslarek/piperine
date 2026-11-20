//! Param values, initial values and param history run from MSL source on the Rust backend (feature
//! `analog-structure`, AS-12, AS-15): each module is written in `tests/structure/params.msl`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is written from the criterion it cites, with its closed form beside the assertion.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier};

/// The project of `tests/structure/params.msl`.
fn project() -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structure/params.msl");
    Project::new(&path)
}

/// The node `<top>.<name>` of an elaborated design.
fn node(ir: &piperine_ir::Ir, top: &str, name: &str) -> NodeIdentifier {
    NodeIdentifier::Anonymous(ir.node_at(&format!("{top}.{name}")).expect("a node").id().index())
}

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use support::build;

/// `v(o)` of the bench `top` over a transient to `stop` with the step cap `step`, as a waveform.
fn transient(top: &str, stop: f64, step: f64) -> Waveform {
    let built = build(&project(), top);
    let out = node(&built.ir, top, "o");
    let (_, result) = built.run_with(stop, step);
    let steps: Vec<_> = result.iter().collect();
    Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(o)")).collect())
}

/// `|v(o)|` of the bench `top` at each frequency of a sweep of `steps` points from `start` to `stop`.
fn ac_magnitudes(top: &str, start: f64, stop: f64, steps: usize) -> Vec<(f64, f64)> {
    let mut running = project().compile(top).unwrap_or_else(|r| panic!("{r}"));
    let out = node(&running.ir, top, "o");
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: start, stop_frequency: stop, steps, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    sweep.iter().map(|step| (step.frequency, step.get_node(&out).expect("v(o)").norm())).collect()
}

#[test]
fn a_body_let_as_a_param_value_sets_the_corner() {
    // AS-12 AC 1: `c = 1 / (2 pi r fc)` with the defaults r = 1 kOhm and fc = 1 kHz is the corner of
    // an RC low-pass: |v(o) / v(a)| = 1 / sqrt(1 + (f / fc)^2), which is 1 / sqrt 2 at f = fc = 1 kHz.
    let magnitudes = ac_magnitudes("CornerBench", 1e2, 1e4, 3);
    for (frequency, magnitude) in magnitudes {
        let wanted = 1.0 / (1.0 + (frequency / 1e3).powi(2)).sqrt();
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("corner", "|v(o)|", Coordinate::Frequency(frequency)), wanted, magnitude);
    }
}

#[test]
fn a_node_initial_value_over_a_param_holds_the_operating_point() {
    // AS-15 AC 2 (§16): `signal o: Electrical = v0;` with v0 = 0.6 holds `o` at 0.6 V through the
    // transient's operating point, then the 1 uF into 1 V behind 1 kOhm (tau = 1 ms) charges it:
    // v(o) = 1 - 0.4 * exp(-t / tau): 0.6 at t = 0 and 1 - 0.4 / e = 0.8528 at 1 ms.
    let waveform = transient("StartedBench", 3e-3, 10e-6);
    for time in [0.0, 1e-3, 2e-3, 3e-3] {
        let wanted = 1.0 - 0.4 * (-time / 1e-3_f64).exp();
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("started", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}
