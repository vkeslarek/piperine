//! Scope: transient parity (requirement PAR-06 AC 6 and 7, edge case 3): `v(out)` of the RC
//! twin, run to 10 us with a step cap of a tenth of ngspice's print step and read at the exported
//! times by linear interpolation, agrees with ngspice under `TRAN_VOLTAGE`.

use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// How the transient of a twin is configured from ngspice's export.
struct TranRun;

impl TranRun {
    const STOP: f64 = 10e-6;

    /// The options: the stop time, and a step cap of a tenth of the export's print step (the
    /// difference of its first two times), which keeps the interpolation error a decade under
    /// the contract.
    fn options(times: &[f64]) -> TransientAnalysisOptions {
        let print_step = times[1] - times[0];
        let mut options = TransientAnalysisOptions::new(Self::STOP, print_step / 10.0);
        options.dt_max = print_step / 10.0;
        options
    }
}

#[test]
fn the_transient_step_cap_is_a_tenth_of_the_print_step() {
    let times = [0.0, 0.1e-6, 0.2e-6];
    let options = TranRun::options(&times);
    assert_eq!(options.dt_max, (times[1] - times[0]) / 10.0);
    assert_eq!(options.stop_time, 10e-6);
    let shifted = TranRun::options(&[1e-7, 3e-7]);
    assert_eq!(shifted.dt_max, 2e-8);
}

#[test]
fn tran_rc() {
    let reference = Ngspice::from_environment().run("tran_rc");
    let mut bench = Bench::load("tran_rc");
    let out = bench.node("out");
    let rows = match &reference {
        Reference::Golden(golden) => {
            let rows = golden.real("tran_rc_tran");
            golden.expect_rows("tran_rc_tran", &rows, 100);
            rows
        }
        Reference::Skipped => vec![(0.0, 0.0), (0.1e-6, 0.0)],
    };
    let times: Vec<f64> = rows.iter().map(|(t, _)| *t).collect();
    let options = TranRun::options(&times);
    let result = bench.circuit().transient(options, Context::default()).expect("transient").solve().expect("the transient solves");
    let steps: Vec<_> = result.iter().collect();
    let waveform = Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(out)")).collect());
    let Reference::Golden(golden) = &reference else { return };
    for (time, wanted) in rows {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("tran_rc: the time {time:e} s lies outside the solver's result"));
        let at = Point::new("tran_rc", "v(out)", Coordinate::Time(time)).with_ngspice(golden.version());
        ToleranceContract::TRAN_VOLTAGE.assert(&at, wanted, actual);
    }
}
