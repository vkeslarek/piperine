//! Scope: noise parity (analog-operators AO-06 and AO-12; design D9): the output density `onoise`
//! at every frequency, and the band total `onoise_total`, of the noise twins against ngspice's
//! `.noise` (`onoise_spectrum`, `onoise_total`) under `NOISE`, on the compiled backend only (the
//! interpreter refuses the noise sources). The sweep is read from ngspice's export, so both sides
//! stand on one grid. Without ngspice (`PIPERINE_NGSPICE=skip`) each twin still solves its sweep
//! and compares nothing.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Lookup, Ngspice, Reference};
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier, NoiseAnalysisOptions, NoiseAnalysisResult};

/// A noise twin and what ngspice's run of its netlist exported: the output node, the input
/// instance (for the input-referred columns), and the `wrdata` exports it names.
struct NoiseTwin {
    /// The twin's name, which names its exports `<name>_psd`, `<name>_total` and, with an input,
    /// `<name>_ipsd` and `<name>_itotal`.
    name: &'static str,
    /// The top module's node the density is measured at.
    output: &'static str,
    /// The top module's instance whose stimulus is the analysis input, when the twin compares `inoise`.
    input: Option<&'static str>,
    /// The rows of the density export (the points of the sweep).
    rows: usize,
    /// Whether the twin compares the band totals; a twin may hold only the densities.
    totals: bool,
}

impl NoiseTwin {
    /// The compiled backends that run a noise twin among the selected ones.
    fn backends() -> Vec<Backend> {
        Backend::selected().into_iter().filter(|backend| *backend == Backend::Rust).collect()
    }

    /// The run of the twin on `backend`: its sweep solved, held to `reference` when ngspice ran.
    fn compare(&self, reference: &Reference, backend: Backend) {
        let density = format!("{}_psd", self.name);
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
            let wanted = golden.real(&format!("{}_total", self.name))[0].1;
            let at = Point::new(self.name, "onoise_total", band).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::NOISE.assert(&at, wanted, result.output_total);
        }
    }

    /// The noise analysis of the twin on `backend` over `sweep`.
    fn solve(&self, backend: Backend, sweep: AcSweepAnalysisOptions) -> NoiseAnalysisResult {
        let mut bench = Bench::load_on(self.name, backend);
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

/// The thermal noise of the resistor of an RC pole at 100 kHz: `onoise` over 301 points of
/// `dec 100 1k 1Meg` and its total, which agree only on a fine grid (the comment of the `.cir`).
const NOISE_RC: NoiseTwin = NoiseTwin { name: "noise_rc", output: "out", input: Some("v1"), rows: 301, totals: true };

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
