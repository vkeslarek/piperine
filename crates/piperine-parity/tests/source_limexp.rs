//! `limexp` from MSL source on the compiled backend (feature `unknown-semantics-events`, UE-05):
//! the standard library's `limexp` (§22.2.4) elaborates with its closure, a diode written with it
//! and one written with `exp` under `pnjlim` agree at the operating point, and the `diode_series`
//! twin written with `limexp` agrees with ngspice's `diode_series.cir`, both under `DC_VOLTAGE`.
//! Models are in `tests/limexp/`; the tolerance is the existing contract, never a new one.

use std::path::{Path, PathBuf};

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_parity::project::Project;
use piperine_solver::prelude::Context;

/// The nodes of `diode_series` the operating point is read at.
const NODES: [&str; 2] = ["a", "b"];

/// The backends that run a diode written with `limit`: the compiled one.
fn compiled_backends() -> Vec<Backend> {
    Backend::selected().into_iter().filter(|backend| *backend == Backend::Rust).collect()
}

/// One `diode_series` model of `tests/limexp/`.
struct SeriesModel {
    /// The model's file under `tests/limexp/`.
    file: &'static str,
}

impl SeriesModel {
    /// The path of the model.
    fn path(&self) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/limexp").join(self.file)
    }

    /// The operating point of the model on `backend`: the voltage of each of [`NODES`].
    fn operating_point(&self, backend: Backend) -> Vec<f64> {
        let mut bench = Bench::load_file_on(self.file, self.path(), backend);
        let ids: Vec<_> = NODES.iter().map(|node| bench.node(node)).collect();
        let solved = bench.circuit().dc(Context::default()).expect("dc").solve().unwrap_or_else(|error| panic!("{}: the operating point does not solve on {}: {error}", self.file, backend.name()));
        ids.iter().zip(NODES).map(|(id, node)| solved.get_node(id).unwrap_or_else(|| panic!("{}: no v({node})", self.file))).collect()
    }
}

#[test]
fn limexp_elaborates_from_the_stdlib_with_its_closure() {
    // §22.2.4: `limexp` is a standard library module whose body hands `limit` a closure (D9); the
    // series model with `limexp` type checks and elaborates with no stage refusing it.
    let ir = Project::new(&SeriesModel { file: "series_limexp.msl" }.path()).elaborate("DiodeSeries").unwrap_or_else(|rejection| panic!("{rejection}"));
    assert!(ir.instance_at("DiodeSeries.d1").is_some(), "the diode with limexp is an instance of the elaborated design");
}

#[test]
fn limexp_and_pnjlim_diodes_agree_at_the_operating_point() {
    // The same circuit with the junction's exponential written two ways reaches one operating
    // point: `limexp` only changes how Newton gets there (§22.2.4 rule 3), never where it lands.
    for backend in compiled_backends() {
        let limexp = SeriesModel { file: "series_limexp.msl" }.operating_point(backend);
        let pnjlim = SeriesModel { file: "series_pnjlim.msl" }.operating_point(backend);
        for ((node, limited), exact) in NODES.iter().zip(&limexp).zip(&pnjlim) {
            let quantity = format!("v({node})");
            let at = Point::new("diode_series limexp against pnjlim", &quantity, Coordinate::OperatingPoint).with_backend(backend.name());
            ToleranceContract::DC_VOLTAGE.assert(&at, *exact, *limited);
        }
    }
}

#[test]
fn the_limexp_diode_series_twin_agrees_with_ngspice() {
    // D11: the twin written with `limexp` against `tests/e2e/twins/diode_series.cir`, the same
    // netlist the pnjlim twin is held to in `parity_op`, under the same `DC_VOLTAGE`.
    let reference = Ngspice::from_environment().run("diode_series");
    let Reference::Golden(golden) = &reference else { panic!("ngspice is required for this comparison, it is not skipped") };
    for backend in compiled_backends() {
        let solved = SeriesModel { file: "series_limexp.msl" }.operating_point(backend);
        for (node, actual) in NODES.iter().zip(solved) {
            let quantity = format!("v({node})");
            let at = Point::new("diode_series limexp", &quantity, Coordinate::OperatingPoint).with_ngspice(golden.version()).with_backend(backend.name());
            ToleranceContract::DC_VOLTAGE.assert(&at, golden.printed(&quantity), actual);
        }
    }
}
