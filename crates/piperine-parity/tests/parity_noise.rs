//! Scope: noise parity (analog-operators AO-06 and AO-12; design D9): the output density `onoise`
//! at every frequency, and the band total `onoise_total`, of the noise twins against ngspice's
//! `.noise` (`onoise_spectrum`, `onoise_total`) under `NOISE`, on the compiled backend only (the
//! interpreter refuses the noise sources). The sweep is read from ngspice's export, so both sides
//! stand on one grid. Without ngspice (`PIPERINE_NGSPICE=skip`) each twin still solves its sweep
//! and compares nothing.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::golden::Golden;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Lookup, Ngspice, Reference};
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier, NoiseAnalysisOptions, NoiseAnalysisResult};

/// A noise twin and what ngspice's run of its netlist exported: the output node, the input
/// instance, and the `wrdata` exports it names.
struct NoiseTwin {
    /// The twin's name: the model `tests/e2e/twins/<name>.msl`.
    name: &'static str,
    /// The netlist ngspice runs and names its exports after: `<export>_psd` and `<export>_total`.
    export: &'static str,
    /// Params set on the loaded twin before the sweep, as `(instance, param, value)`.
    settings: &'static [(&'static str, &'static str, f64)],
    /// The top module's node the density is measured at.
    output: &'static str,
    /// The top module's instance whose stimulus is the analysis input.
    input: Option<&'static str>,
    /// The rows of the density export (the points of the sweep).
    rows: usize,
    /// Whether the twin compares the band totals; a twin may hold only the densities.
    totals: bool,
    /// Whether the twin also compares the input-referred density and total, ngspice's
    /// `<export>_ipsd` and `<export>_itotal`.
    referred: bool,
}

impl NoiseTwin {
    /// The compiled backends that run a noise twin among the selected ones.
    fn backends() -> Vec<Backend> {
        Backend::selected().into_iter().filter(|backend| *backend == Backend::Rust).collect()
    }

    /// The run of the twin on `backend`: its sweep solved, held to `reference` when ngspice ran.
    fn compare(&self, reference: &Reference, backend: Backend) {
        let density = format!("{}_psd", self.export);
        let rows = match reference {
            Reference::Golden(golden) => {
                let rows = golden.real(&density);
                golden.expect_rows(&density, &rows, self.rows);
                rows
            }
            Reference::Skipped => Vec::new(),
        };
        let sweep = match (rows.first(), rows.last()) {
            (Some((start, _)), Some((stop, _))) => AcSweepAnalysisOptions { start_frequency: *start, stop_frequency: *stop, steps: rows.len(), logarithmic: true },
            _ => AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e6, steps: self.rows, logarithmic: true },
        };
        let result = self.solve(backend, sweep.clone());
        let Reference::Golden(golden) = reference else { return };
        golden.same_grid(&density, &result.frequencies);
        let band = Coordinate::Band(sweep.start_frequency, sweep.stop_frequency);
        for ((frequency, wanted), actual_squared) in rows.iter().zip(&result.out_noise_sq) {
            let at = Point::new(self.name, "onoise", Coordinate::Frequency(*frequency)).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::NOISE.assert(&at, *wanted, actual_squared.sqrt());
        }
        if self.totals {
            let wanted = golden.real(&format!("{}_total", self.export))[0].1;
            let at = Point::new(self.name, "onoise_total", band).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::NOISE.assert(&at, wanted, result.output_total);
        }
        if self.referred {
            self.compare_referred(golden, &result, band, backend);
        }
    }

    /// The input-referred density at every frequency and its band total against ngspice's
    /// `inoise_spectrum` and `inoise_total`.
    fn compare_referred(&self, golden: &Golden, result: &NoiseAnalysisResult, band: Coordinate, backend: Backend) {
        let export = format!("{}_ipsd", self.export);
        let rows = golden.real(&export);
        golden.expect_rows(&export, &rows, self.rows);
        let squared = result.input_noise_sq.as_ref().unwrap_or_else(|| panic!("{}: the run has an input and reports no input-referred density", self.name));
        for ((frequency, wanted), actual_squared) in rows.iter().zip(squared) {
            let at = Point::new(self.name, "inoise", Coordinate::Frequency(*frequency)).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::NOISE.assert(&at, *wanted, actual_squared.sqrt());
        }
        let wanted = golden.real(&format!("{}_itotal", self.export))[0].1;
        let total = result.input_total.unwrap_or_else(|| panic!("{}: the run has an input and reports no input-referred total", self.name));
        let at = Point::new(self.name, "inoise_total", band).with_ngspice(golden.version()).with_backend(backend.name());
        ToleranceContract::NOISE.assert(&at, wanted, total);
    }

    /// The noise analysis of the twin on `backend` over `sweep`.
    fn solve(&self, backend: Backend, sweep: AcSweepAnalysisOptions) -> NoiseAnalysisResult {
        let mut bench = Bench::load_on(self.name, backend);
        for (instance, param, value) in self.settings {
            bench.set(instance, param, *value);
        }
        let options = NoiseAnalysisOptions {
            sweep_options: sweep,
            output_node: bench.node(self.output),
            reference_node: NodeIdentifier::Gnd,
            input_source_name: self.input.map(|input| bench.path(input)),
            output_flow: None,
        };
        let mut noise = bench.circuit().noise(options, Context::default()).unwrap_or_else(|error| panic!("{}: the noise analysis does not start on {}: {error}", self.name, backend.name()));
        noise.solve().unwrap_or_else(|error| panic!("{}: the noise sweep does not solve on {}: {error}", self.name, backend.name()))
    }
}

impl NoiseTwin {
    /// The twin's output density at every point of a sweep of `rows` points from `start` to `stop`
    /// is the flat `density` (V per root hertz) of a closed form, held under `NOISE` whether or not
    /// ngspice ran: the form needs no reference.
    fn compare_flat(&self, density: f64, start: f64, stop: f64, backend: Backend) {
        let sweep = AcSweepAnalysisOptions { start_frequency: start, stop_frequency: stop, steps: self.rows, logarithmic: true };
        let result = self.solve(backend, sweep);
        assert_eq!(result.frequencies.len(), self.rows, "{}: one point per step", self.name);
        for (frequency, actual_squared) in result.frequencies.iter().zip(&result.out_noise_sq) {
            let at = Point::new(self.name, "onoise", Coordinate::Frequency(*frequency)).with_backend(backend.name());
            ToleranceContract::NOISE.assert(&at, density, actual_squared.sqrt());
        }
    }
}

/// Boltzmann's constant as `std` declares it, J/K.
const BOLTZMANN: f64 = 1.380649e-23;

/// ngspice's default temperature, 27 C, in kelvin: what the twins' `--temp 27` fixes.
const ROOM: f64 = 300.15;

/// The thermal noise of the resistor of an RC pole at 100 kHz: `onoise` over 301 points of
/// `dec 100 1k 1Meg` and its total, which agree only on a fine grid (the comment of the `.cir`).
const NOISE_RC: NoiseTwin = NoiseTwin { name: "noise_rc", export: "noise_rc", settings: &[], output: "out", input: Some("v1"), rows: 301, totals: true, referred: false };

#[test]
fn noise_rc() {
    let reference = Ngspice::from_environment().run("noise_rc");
    for backend in NoiseTwin::backends() {
        NOISE_RC.compare(&reference, backend);
    }
}

#[test]
fn noise_twins_run_and_print_the_skip_line_without_ngspice() {
    // AO-12: with the comparison disabled `Ngspice::run` prints its skip line and returns
    // `Skipped`, and each twin still solves its sweep on the compiled backend and compares nothing
    let reference = Ngspice::new(Lookup::Skip).run("noise_rc");
    assert!(matches!(reference, Reference::Skipped));
    assert_eq!(Ngspice::skip_line("noise_rc"), "SKIP noise_rc: ngspice comparison disabled by PIPERINE_NGSPICE=skip");
    for backend in NoiseTwin::backends() {
        NOISE_RC.compare(&reference, backend);
    }
}

/// The switch twin `noise_switch` with its control at 5 V, above the threshold: the switch is 1 ohm.
const NOISE_SWITCH: NoiseTwin = NoiseTwin { name: "noise_switch", export: "noise_switch", settings: &[], output: "a", input: Some("vin"), rows: 11, totals: false, referred: false };

/// The same twin with its control at 0 V: the switch is open, 10 kohm (`noise_switch_off.cir`).
const NOISE_SWITCH_OPEN: NoiseTwin = NoiseTwin { name: "noise_switch", export: "noise_switch_off", settings: &[("vc", "dc", 0.0)], output: "a", input: Some("vin"), rows: 11, totals: false, referred: false };

#[test]
fn noise_switch() {
    // AO-06: the closed switch carries the thermal noise of its 1 ohm. ngspice (46 and 47) cannot be
    // the oracle for it: its noise source reads the switch state vector at 0 and so takes the open
    // switch's noise and the open switch's transfer for a switch the operating point closed
    // (`noise_switch.cir` prints 3.881900e-9, the open value of `noise_switch_off.cir`, with
    // `v(a)` = 0.999 mV, the closed divider, at the same operating point). The closed form is the
    // reference: both resistors are noise currents `4 k T / r` into the node's impedance
    // `1 / (1/1k + 1/ron)`, so `v^2 = 4 k T (1k || ron)`: 1.286838e-10 V/sqrt(Hz) at 27 C.
    let parallel = 1000.0 * 1.0 / (1000.0 + 1.0);
    let density = (4.0 * BOLTZMANN * ROOM * parallel).sqrt();
    for backend in NoiseTwin::backends() {
        NOISE_SWITCH.compare_flat(density, 1e3, 1e4, backend);
    }
}

#[test]
fn noise_switch_open_agrees_with_ngspice() {
    // the same twin with the control below the threshold is `noise_switch_off.cir`, which ngspice
    // computes correctly: `roff` = 10k across the 1k source resistance
    let reference = Ngspice::from_environment().run("noise_switch_off");
    for backend in NoiseTwin::backends() {
        NOISE_SWITCH_OPEN.compare(&reference, backend);
    }
}

/// The junction twin: shot and flicker noise at 1 mA, 41 points of `dec 10 10 100k`, densities and
/// totals, at the node and referred to the source.
const NOISE_DIODE: NoiseTwin = NoiseTwin { name: "noise_diode", export: "noise_diode", settings: &[], output: "a", input: Some("i1"), rows: 41, totals: true, referred: true };

#[test]
fn noise_diode() {
    let reference = Ngspice::from_environment().run("noise_diode");
    for backend in NoiseTwin::backends() {
        NOISE_DIODE.compare(&reference, backend);
    }
}

#[test]
fn noise_diode_at_one_kilohertz_is_the_hand_derived_density() {
    // AO-05 AC 1 and AO-06: the 1 mA source forces the junction current (`I1` less `gmin * vd`, a
    // part in 1e9), so the shot density is `2 q I = 3.204353e-22 A^2/Hz` and the flicker density is
    // `kf I^af / f = 1e-15 * (1e-3)^1.2 / 1e3 = 2.511886e-22 A^2/Hz`; the node sees them across the
    // junction's resistance `nvt / I` (nvt = k T / q at 300.15 K, 25.8648 mV, so 25.8648 ohm), and the
    // output density is `sqrt((shot + flicker)) * nvt / I`; referred to the source (whose own
    // transfer is that same resistance) it is `sqrt(shot + flicker)`, the current density itself.
    let current = 1e-3_f64;
    let thermal_voltage = BOLTZMANN * ROOM / 1.602176634e-19;
    let shot = 2.0 * 1.602176634e-19 * current;
    let flicker = 1e-15 * current.powf(1.2) / 1e3;
    let resistance = thermal_voltage / current;
    let sweep = AcSweepAnalysisOptions { start_frequency: 10.0, stop_frequency: 1e5, steps: 41, logarithmic: true };
    for backend in NoiseTwin::backends() {
        let result = NOISE_DIODE.solve(backend, sweep.clone());
        let at_one_kilohertz = 20;
        assert!(ToleranceContract::NOISE.holds(1e3, result.frequencies[at_one_kilohertz]), "1 kHz is the twenty-first point of dec 10 from 10 Hz");
        let onoise = result.out_noise_sq[at_one_kilohertz].sqrt();
        let inoise = result.input_noise_sq.as_ref().expect("an input run")[at_one_kilohertz].sqrt();
        let point = Point::new("noise_diode", "onoise", Coordinate::Frequency(1e3)).with_backend(backend.name());
        ToleranceContract::NOISE.assert(&point, ((shot + flicker).sqrt()) * resistance, onoise);
        let point = Point::new("noise_diode", "inoise", Coordinate::Frequency(1e3)).with_backend(backend.name());
        ToleranceContract::NOISE.assert(&point, (shot + flicker).sqrt(), inoise);
    }
}
