//! Scope: periodic-steady-state parity (host-ii HII-07 AC 4): the orbit of the twin `pss_rc` (an
//! RC driven by a square wave of period 4 us, `tstab` two periods) agrees, row for row, under
//! `TRAN_VOLTAGE`, with the last period of ngspice's ten-period transient at the same phase, and
//! the end of its high half agrees with the closed form `1 / (1 + exp(-T / (2 RC)))`. ngspice's own
//! `.pss` is not a reference for a driven circuit, so a long `.tran` is the oracle.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::{Context, PssAnalysisOptions};

/// The twin's drive and the run's configuration.
struct PssRun;

impl PssRun {
    const PERIOD: f64 = 4e-6;
    const TSTAB: f64 = 8e-6;
    /// The step cap: the source has 1 ps edges and no breakpoint, so a step that spans one
    /// misplaces the edge by up to the step; at 1 ns the orbit is within the contract.
    const STEP: f64 = 1e-9;
    /// `R * C` of the twin.
    const TAU: f64 = 1e-6;
    /// The number of periods of ngspice's transient.
    const PERIODS: f64 = 10.0;

    fn options() -> PssAnalysisOptions {
        let mut options = PssAnalysisOptions::new(Self::PERIOD).with_tstab(Self::TSTAB);
        options.dt = Some(Self::STEP);
        options.dt_max = Some(Self::STEP);
        options
    }
}

fn compare(reference: &Reference, backend: Backend) {
    let mut bench = Bench::load_on("pss_rc", backend);
    let out = bench.node("vout");
    let orbit = bench
        .circuit()
        .pss(PssRun::options(), Context::default())
        .and_then(|solver| solver.solve())
        .unwrap_or_else(|error| panic!("pss_rc: no orbit on {}: {error}", backend.name()));
    let steps: Vec<_> = orbit.trace.iter().collect();
    let waveform = Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(vout)")).collect());
    let mut worst = (0.0_f64, 0.0_f64);
    let mut note = |wanted: f64, actual: f64, time: f64| {
        let deviation = (wanted - actual).abs();
        if deviation > worst.0 {
            worst = (deviation, time);
        }
    };

    // the closed form: the end of the high half, 1 / (1 + exp(-T / (2 RC)))
    let half = PssRun::TSTAB + PssRun::PERIOD / 2.0;
    let closed = 1.0 / (1.0 + (-PssRun::PERIOD / (2.0 * PssRun::TAU)).exp());
    let actual = waveform.at(half).expect("the orbit holds the end of the high half");
    note(closed, actual, half);
    let at = Point::new("pss_rc", "v(vout), end of the high half", Coordinate::Time(half)).with_backend(backend.name());
    ToleranceContract::TRAN_VOLTAGE.assert(&at, closed, actual);

    if let Reference::Golden(golden) = reference {
        let rows = golden.real("pss_rc_tran");
        golden.expect_rows("pss_rc_tran", &rows, 1001);
        // the last period of the transient: [(N - 1) T, N T], at the phase of the orbit's [t0, t0 + T]
        let last_from = (PssRun::PERIODS - 1.0) * PssRun::PERIOD;
        let mut compared = 0;
        for (time, wanted) in rows.iter().filter(|(t, _)| *t >= last_from - 1e-12) {
            let phase = (time - last_from).clamp(0.0, PssRun::PERIOD);
            let actual = waveform.at(PssRun::TSTAB + phase).unwrap_or_else(|| panic!("pss_rc: phase {phase:e} s lies outside the orbit on {}", backend.name()));
            note(*wanted, actual, phase);
            let at = Point::new("pss_rc", "v(vout)", Coordinate::Time(*time)).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::TRAN_VOLTAGE.assert(&at, *wanted, actual);
            compared += 1;
        }
        assert_eq!(compared, 101, "pss_rc: the last period holds 101 grid rows");
    }
    eprintln!("pss_rc on {}: worst deviation {:e} V at {:e} s (closed form {closed:e}, orbit {actual:e}), {} shooting iterations, residual {:e}", backend.name(), worst.0, worst.1, orbit.stats.shoot_iterations, orbit.stats.residual);
}

#[test]
fn pss_rc() {
    let reference = Ngspice::from_environment().run("pss_rc");
    // the source reads `now()`, which the interpreter (the M1 analog subset) does not run: the
    // Rust backend is the one that runs the twin
    for backend in Backend::selected().into_iter().filter(|backend| *backend == Backend::Rust) {
        compare(&reference, backend);
    }
}
