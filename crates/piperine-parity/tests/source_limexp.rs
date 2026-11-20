//! `limexp` from MSL source on the compiled backend (feature `unknown-semantics-events`, UE-05):
//! the standard library's `limexp` (§22.2.4) elaborates with its closure, a diode written with it
//! and one written with `exp` under `pnjlim` agree at the operating point, and the `diode_series`
//! twin written with `limexp` agrees with ngspice's `diode_series.cir`, both under `DC_VOLTAGE`.
//! The limit instance of the compiled diode is called through the device ABI, where §22.2.4
//! rules 3 and 5 are read: its argument moves by at most 1.0 and its derivative is `limexp`.
//! Models are in `tests/limexp/`; the tolerance is the existing contract, never a new one.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_abi::caller::Handle;
use piperine_abi::device::{Access, cell_lens};
use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_parity::project::{Project, Running};
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

/// The compiled `DiodeLimexp`, called through the device ABI at its default `is` and `n`. The
/// project is kept so the loaded library outlives the handle.
struct CompiledDiode {
    handle: Handle,
    _running: Running,
}

/// What one call of a function wrote (each write as an `f64`) and the partials it declares.
struct Call {
    writes: Vec<f64>,
    partials: Vec<f64>,
}

impl CompiledDiode {
    /// The default `is` of `DiodeLimexp` (its model text, `tests/limexp/series_limexp.msl`).
    const SATURATION: f64 = 1e-14;
    /// The thermal voltage constants of `std` (`K_B`, `Q`, section 1) and the temperature fed to
    /// the limit function (kelvin).
    const BOLTZMANN: f64 = 1.380649e-23;
    const CHARGE: f64 = 1.602176634e-19;
    const TEMPERATURE: f64 = 300.15;

    /// The diode of `series_limexp.msl`, compiled, instantiated at its defaults.
    fn compile() -> CompiledDiode {
        let running = Project::new(&SeriesModel { file: "series_limexp.msl" }.path()).compile("DiodeSeries").unwrap_or_else(|rejection| panic!("{rejection}"));
        let module = running.compiled().loaded().module("DiodeLimexp").expect("the compiled library holds DiodeLimexp");
        let handle = Handle::create(module, &[]).unwrap_or_else(|error| panic!("create: {error}"));
        CompiledDiode { handle, _running: running }
    }

    /// `n * vt()` at [`TEMPERATURE`] for `n = 1`: the scale of the exponent (`vt = K_B T / Q`).
    fn scale() -> f64 {
        Self::BOLTZMANN * Self::TEMPERATURE / Self::CHARGE
    }

    /// Runs the function `name` on `reads`, one byte string per read cell.
    fn call(&self, name: &str, reads: &[&[u8]]) -> Call {
        let shape = self.handle.shape();
        let k = shape.functions.iter().position(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        let function = shape.functions.get(k).expect("the function");
        let lens = cell_lens(shape, &function.writes, Access::FunctionWrite).expect("write lens");
        let mut cells: Vec<Vec<u8>> = lens.iter().map(|len| vec![0u8; *len]).collect();
        let mut writes: Vec<&mut [u8]> = cells.iter_mut().map(Vec::as_mut_slice).collect();
        let wanted = vec![1u8; function.partials.len()];
        let mut partials = vec![0.0; function.partials.len()];
        self.handle.eval(k as u32, reads, &mut writes, &wanted, &mut partials).unwrap_or_else(|error| panic!("{name}: {error}"));
        Call { writes: cells.iter().map(|cell| f64::from_le_bytes(cell[..8].try_into().expect("an f64 cell"))).collect(), partials }
    }

    /// The limit instance (`limit:limit0`) with `a - k = exponent * nvt`, `n = 1` and the last
    /// limited value `last`: it reads `a`, `k`, `n`, the temperature, `LAST`, `AGE` (an iteration
    /// after the first) and `STATIC` (not a small-signal sweep), and writes `PROPOSED`, `LIMITED`.
    fn limit(&self, exponent: f64, last: f64) -> Call {
        let (a, k, n, temperature) = (exponent * Self::scale(), 0.0f64, 1.0f64, Self::TEMPERATURE);
        self.call("limit:limit0", &[&a.to_le_bytes(), &k.to_le_bytes(), &n.to_le_bytes(), &temperature.to_le_bytes(), &last.to_le_bytes(), &[1u8], &[0u8]])
    }

    /// The current (`terms`) with `a - k = exponent * nvt` and the limited exponent `limited`: it
    /// reads `a`, `k`, the `LIMITED` value, `is` and `gmin`; its third partial is the derivative
    /// of the current against the limited exponent.
    fn terms(&self, exponent: f64, limited: f64) -> Call {
        let (a, k, gmin) = (exponent * Self::scale(), 0.0f64, 1e-12f64);
        self.call("terms", &[&a.to_le_bytes(), &k.to_le_bytes(), &limited.to_le_bytes(), &Self::SATURATION.to_le_bytes(), &gmin.to_le_bytes()])
    }
}

#[test]
fn limexp_moves_its_argument_at_most_one_per_iteration() {
    // §22.2.4 rule 3 with `deltalim(new, old, 1.0) = new.clamp(old - 1.0, old + 1.0)`: from `old = 0`
    // an argument of 30 comes out as 1.0 and -30 as -1.0; one inside the bound passes unchanged.
    let diode = CompiledDiode::compile();
    for (new, old) in [(30.0, 0.0), (-30.0, 0.0), (0.25, 0.0), (30.0, 12.5)] {
        let moved = diode.limit(new, old);
        let wanted = f64::clamp(new, old - 1.0, old + 1.0);
        assert!((moved.writes[1] - wanted).abs() <= 1e-12, "limit from {old} for {new}: {} against {wanted}", moved.writes[1]);
        assert!((moved.writes[0] - new).abs() <= 1e-9 * new.abs().max(1.0), "the proposed value is the argument unlimited: {}", moved.writes[0]);
    }
    assert_eq!(diode.limit(30.0, 0.0).writes[1], 1.0, "UE-05 AC 3: old = 0, new = 30 gives 1.0");
}

#[test]
fn the_derivative_of_limexp_is_limexp() {
    // §22.2.4 rule 5 and §18 rule 3: `limit` is transparent, so at a converged point (last = new =
    // x, the limited value x) the current `is * (limexp(x) - 1)` has the partial `is * limexp(x)`
    // against the limited exponent, and the argument's own partial against the node is 1 / nvt:
    // d i / d v = is * exp(x) / nvt, with the exponent `x` of a forward bias.
    let diode = CompiledDiode::compile();
    let x = 0.7 / CompiledDiode::scale();
    let limited = diode.limit(x, x);
    assert!((limited.writes[1] - x).abs() <= 1e-12, "converged: the limited value is x");
    let current = diode.terms(x, limited.writes[1]);
    let against_exponent = current.partials[2];
    let wanted_against_exponent = CompiledDiode::SATURATION * x.exp();
    assert!((against_exponent - wanted_against_exponent).abs() <= 1e-9 * wanted_against_exponent, "d i / d(limexp argument): {against_exponent} against {wanted_against_exponent}");
    let against_node = against_exponent * limited.partials[0];
    let wanted_against_node = CompiledDiode::SATURATION * x.exp() / CompiledDiode::scale();
    assert!((against_node - wanted_against_node).abs() <= 1e-9 * wanted_against_node, "d i / d v: {against_node} against {wanted_against_node}");
}
