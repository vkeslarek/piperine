//! Scope: transient parity (requirement PAR-06 AC 6 and 7, edge case 3; HYG-06): `v(out)` of the RC
//! twin and `v(a)` of `tran_dq`, the RC charge through a diode whose transit-time charge `tt * id`
//! is nonlinear in the limited junction voltage, each run to 10 us with a step cap of a tenth of
//! ngspice's print step and read at the exported times by linear interpolation, agree with ngspice
//! under `TRAN_VOLTAGE`. Three twins whose source is computed from the time (nothing announces its
//! corner) pin the step floor (HYG-09, the floor rule of ngspice's `dctran.c`): `tran_jump` (a hard
//! step) and `tran_corner` (a 5 ns tanh corner with a 1 ns floor) complete in ngspice and agree with
//! it; `tran_floor` is a node no step can settle, where ngspice stops with "Timestep too small" and
//! piperine fails `E0508`. They run on the compiled backend only: the interpreter does not run
//! `now()`.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Lookup, Ngspice, Reference};
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
        Self::options_to(times, Self::STOP)
    }

    /// [`options`](Self::options) for a run to `stop`.
    fn options_to(times: &[f64], stop: f64) -> TransientAnalysisOptions {
        let print_step = times[1] - times[0];
        let mut options = TransientAnalysisOptions::new(stop, print_step / 10.0);
        options.dt_max = print_step / 10.0;
        options
    }
}

/// A twin whose source reads the time: the compiled backend only (the interpreter runs the M1
/// analog subset, which has no `now()`).
fn compiled_backends() -> Vec<Backend> {
    Backend::selected().into_iter().filter(|backend| *backend == Backend::Rust).collect()
}

/// The stop of the three floor twins and the print rows ngspice exports for it (a print step of
/// 0.1 us).
const FLOOR_STOP: f64 = 2e-6;
const FLOOR_ROWS: usize = 20;

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
    for backend in Backend::selected() {
        compare_on("tran_rc", "out", &reference, backend);
    }
}

#[test]
fn tran_dq() {
    let reference = Ngspice::from_environment().run("tran_dq");
    for backend in Backend::selected() {
        compare_on("tran_dq", "a", &reference, backend);
    }
}

#[test]
fn tran_jump() {
    // A hard 0 to 1 V step at 1.03 us across 1 nF, with the default floor: ngspice completes the
    // run (61 points), and so does piperine, accepting the step at the floor.
    let reference = Ngspice::from_environment().run("tran_jump");
    for backend in compiled_backends() {
        let options = |times: &[f64]| TranRun::options_to(times, FLOOR_STOP);
        compare_with("tran_jump", "top", &reference, backend, FLOOR_ROWS, options);
    }
}

#[test]
fn tran_corner() {
    // A 5 ns tanh corner at 1.03 us with a 1 ns floor (ngspice's `delmin=1n`): both complete.
    let reference = Ngspice::from_environment().run("tran_corner");
    for backend in compiled_backends() {
        let options = |times: &[f64]| {
            let mut options = TranRun::options_to(times, FLOOR_STOP);
            options.dt_min = 1e-9;
            options
        };
        compare_with("tran_corner", "top", &reference, backend, FLOOR_ROWS, options);
    }
}

#[test]
fn tran_floor() {
    // A node no step can settle after 1 us: ngspice stops with "Timestep too small" at 1 us,
    // piperine fails E0508 at the same time, on a Newton failure at the floor.
    let reference = Ngspice::from_environment().run("tran_floor");
    if let Reference::Golden(golden) = &reference {
        assert!(golden.said("Timestep too small"), "ngspice stops at the floor:\n{}", golden.captured());
    }
    for backend in compiled_backends() {
        let mut bench = Bench::load_on("tran_floor", backend);
        let options = TranRun::options_to(&[0.0, 0.1e-6], FLOOR_STOP);
        let error = bench.circuit().transient(options, Context::default()).and_then(|mut tran| tran.solve()).map(|_| ()).expect_err("the step no Newton iteration settles must fail");
        let text = error.to_string();
        assert!(text.contains("E0508"), "{}: {text}", backend.name());
        let time = text.split("at t = ").nth(1).and_then(|t| t.split(' ').next()).and_then(|t| t.parse::<f64>().ok()).expect("the time of the failing step");
        assert!((1e-6..1e-6 + 1e-8).contains(&time), "{}: the failing step follows ngspice's 1 us: {text}", backend.name());
    }
}

/// The transient of the twin `fixture` on `backend`, reading `v(node)`, against the reference run
/// already made.
fn compare_on(fixture: &str, node: &str, reference: &Reference, backend: Backend) {
    compare_with(fixture, node, reference, backend, 100, TranRun::options);
}

/// [`compare_on`] with the export's `expected_rows` and the options `options_of` makes from its times.
fn compare_with(fixture: &str, node: &str, reference: &Reference, backend: Backend, expected_rows: usize, options_of: impl Fn(&[f64]) -> TransientAnalysisOptions) {
    let export = format!("{fixture}_tran");
    let quantity = format!("v({node})");
    let mut bench = Bench::load_on(fixture, backend);
    let out = bench.node(node);
    let rows = match reference {
        Reference::Golden(golden) => {
            let rows = golden.real(&export);
            golden.expect_rows(&export, &rows, expected_rows);
            rows
        }
        Reference::Skipped => vec![(0.0, 0.0), (0.1e-6, 0.0)],
    };
    let times: Vec<f64> = rows.iter().map(|(t, _)| *t).collect();
    let options = options_of(&times);
    let result = bench.circuit().transient(options, Context::default()).expect("transient").solve().unwrap_or_else(|error| panic!("{fixture}: the transient does not solve on {}: {error}", backend.name()));
    let steps: Vec<_> = result.iter().collect();
    let waveform = Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).unwrap_or_else(|| panic!("{fixture}: no {quantity}"))).collect());
    let Reference::Golden(golden) = reference else { return };
    for (time, wanted) in rows {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("{fixture}: the time {time:e} s lies outside the solver's result on {}", backend.name()));
        let at = Point::new(fixture, &quantity, Coordinate::Time(time)).with_ngspice(golden.version()).with_backend(backend.name());
        ToleranceContract::TRAN_VOLTAGE.assert(&at, wanted, actual);
    }
}

/// The capacitor of the `tran_rc` twin written `2.0 * ddt(0.5 * c * v)` (`tests/structure/`), run on
/// the Rust backend against ngspice's reference of the unchanged circuit.
#[test]
fn tran_rc_with_a_scaled_derivative() {
    let reference = Ngspice::from_environment().run("tran_rc");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structure/tran_rc_scaled.msl");
    let mut bench = Bench::load_file_on("tran_rc", path, Backend::Rust);
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
    let result = bench.circuit().transient(TranRun::options(&times), Context::default()).expect("transient").solve().unwrap_or_else(|error| panic!("tran_rc_scaled: the transient does not solve: {error}"));
    let steps: Vec<_> = result.iter().collect();
    let waveform = Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("v(out)")).collect());
    let Reference::Golden(golden) = &reference else { return };
    for (time, wanted) in rows {
        let actual = waveform.at(time).unwrap_or_else(|| panic!("tran_rc_scaled: the time {time:e} s lies outside the result"));
        let at = Point::new("tran_rc_scaled", "v(out)", Coordinate::Time(time)).with_ngspice(golden.version()).with_backend(Backend::Rust.name());
        ToleranceContract::TRAN_VOLTAGE.assert(&at, wanted, actual);
    }
}

/// The RC charge of a capacitor that starts at 2 V: `CapIc` (§16 H7, `tests/e2e/twins/uic_rc.msl`)
/// against ngspice's `uic` run with `IC=2.0`, on the Rust backend (the interpreter refuses the
/// `analysis` environment reads, T15).
#[test]
fn uic_rc() {
    let reference = Ngspice::from_environment().run("uic_rc");
    for backend in compiled_backends() {
        compare_on("uic_rc", "out", &reference, backend);
    }
}

#[test]
fn uic_rc_runs_and_prints_the_skip_line_without_ngspice() {
    // AS-18 AC 3: with the comparison disabled `Ngspice::run` prints its skip line and returns
    // `Skipped`, and the twin still solves its transient on the Rust backend (`compare_on` runs it
    // and compares nothing).
    let reference = Ngspice::new(Lookup::Skip).run("uic_rc");
    assert!(matches!(reference, Reference::Skipped));
    assert_eq!(Ngspice::skip_line("uic_rc"), "SKIP uic_rc: ngspice comparison disabled by PIPERINE_NGSPICE=skip");
    for backend in compiled_backends() {
        compare_on("uic_rc", "out", &reference, backend);
    }
}
