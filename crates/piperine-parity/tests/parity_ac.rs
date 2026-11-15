//! Scope: AC parity (requirement PAR-06 AC 4 and 5): the magnitude and phase of `v(out)` of the RC
//! low-pass twin over the frequencies of ngspice's export agree with ngspice under `AC_MAGNITUDE`
//! and `AC_PHASE`, on a frequency grid that matches the export's.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context};

#[test]
fn ac_rc() {
    let reference = Ngspice::from_environment().run("ac_rc");
    for backend in Backend::selected() {
        compare_on(&reference, backend);
    }
}

/// The sweep of the RC low-pass on `backend` against the reference run already made.
fn compare_on(reference: &Reference, backend: Backend) {
    let mut bench = Bench::load_on("ac_rc", backend);
    let out = bench.node("out");
    let rows = match reference {
        Reference::Golden(golden) => {
            let rows = golden.complex("ac_rc_ac");
            golden.expect_rows("ac_rc_ac", &rows, 41);
            rows
        }
        Reference::Skipped => Vec::new(),
    };
    let options = match (rows.first(), rows.last()) {
        (Some((start, _)), Some((stop, _))) => AcSweepAnalysisOptions { start_frequency: *start, stop_frequency: *stop, steps: rows.len(), logarithmic: true },
        _ => AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 10e6, steps: 41, logarithmic: true },
    };
    let mut ac = bench.circuit().ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(options).unwrap_or_else(|error| panic!("ac_rc: the sweep does not solve on {}: {error}", backend.name()));
    let Reference::Golden(golden) = reference else { return };
    let actual_grid: Vec<f64> = sweep.iter().map(|step| step.frequency).collect();
    golden.same_grid("ac_rc_ac", &actual_grid);
    for ((frequency, wanted), step) in rows.iter().zip(sweep.iter()) {
        let actual = step.get_node(&out).unwrap_or_else(|| panic!("ac_rc: no v(out) at {frequency} on {}", backend.name()));
        let at = Point::new("ac_rc", "|v(out)|", Coordinate::Frequency(*frequency)).with_ngspice(golden.version()).with_backend(backend.name());
        ToleranceContract::AC_MAGNITUDE.assert(&at, wanted.magnitude(), actual.norm());
        if ToleranceContract::phase_applies(wanted.magnitude()) {
            let at = Point::new("ac_rc", "arg v(out)", Coordinate::Frequency(*frequency)).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::AC_PHASE.assert(&at, wanted.phase(), actual.arg());
        }
    }
}
