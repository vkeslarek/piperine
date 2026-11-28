//! `slew` limits an ideal step, run from MSL source on the Rust backend (analog-operators AO-13):
//! the fixture `tests/operators/slew_step.msl` sets a `Bit<f64>` source from 0 to 1 at 1 us, with no
//! edge, into the stdlib `slew(v(inp), 1M, 1M)`. The expected values are the criterion's, with
//! the closed form beside each assertion, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::NodeIdentifier;

use piperine_parity::mixed::BuiltDesign;

/// The bench of `operators/slew_step.msl`: an ideal step into `slew`.
struct SlewStepBench;

impl SlewStepBench {
    /// `v(o)` over a transient to `stop` with the step cap `step`.
    fn output(stop: f64, step: f64) -> Waveform {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/operators/slew_step.msl");
        let built = BuiltDesign::of(&Project::new(&path), "SlewStepBench");
        let output_node = NodeIdentifier::Anonymous(built.ir.node_at("SlewStepBench.o").expect("node o").id().index());
        let (_, result) = built.run_with(stop, step);
        let accepted: Vec<_> = result.iter().collect();
        Waveform::new(accepted.iter().map(|point| point.time()).collect(), accepted.iter().map(|point| point.get_node(&output_node).expect("v(o)")).collect())
    }
}

#[test]
fn slew_limits_an_ideal_step_to_its_rate() {
    // AO-13: the step 0 to 1 V at 1 us has no edge; at 1 V/us the output is 0 V before 1 us,
    // `(t - 1 us) * 1e6` V between 1 us and 2 us, and 1 V from 2 us.
    let waveform = SlewStepBench::output(3e-6, 0.01e-6);
    for (time, wanted) in [(0.5e-6, 0.0), (0.9e-6, 0.0), (1.25e-6, 0.25), (1.5e-6, 0.5), (1.75e-6, 0.75), (2.5e-6, 1.0), (3.0e-6, 1.0)] {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("slew step", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}
