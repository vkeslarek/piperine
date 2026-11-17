//! Scope: temperature parity (host-ii HII-03): the operating point of the twin `temp_res`, a
//! divider whose lower resistor has a first-order temperature coefficient, solved at a run's
//! `temp` (the context's temperature in kelvin) agrees with ngspice's `.options temp` operating
//! point under `DC_VOLTAGE`, on each selected backend. The same twin at the default temperature
//! agrees with ngspice at 27 C.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::Context;

/// The nominal temperature of the twin and of ngspice's `tnom`, kelvin.
const NOMINAL: f64 = 300.15;

/// Solves `temp_res` on `backend` in a context at `kelvin` (the default context when `None`), and
/// compares `v(mid)` with what ngspice printed as `printed`, when ngspice ran.
fn compare(reference: &Reference, printed: &str, kelvin: Option<f64>, backend: Backend) {
    let mut bench = Bench::load_on("temp_res", backend);
    let mid = bench.node("mid");
    let mut context = Context::default();
    if let Some(kelvin) = kelvin {
        context.tolerances.temperature = kelvin;
    }
    let result = bench.circuit().dc(context).expect("dc").solve().unwrap_or_else(|error| panic!("temp_res: the operating point does not solve on {}: {error}", backend.name()));
    let actual = result.get_node(&mid).unwrap_or_else(|| panic!("temp_res: the solve holds no v(mid) on {}", backend.name()));
    let Reference::Golden(golden) = reference else { return };
    let at = Point::new("temp_res", "v(mid)", Coordinate::OperatingPoint).with_ngspice(golden.version()).with_backend(backend.name());
    ToleranceContract::DC_VOLTAGE.assert(&at, golden.printed(printed), actual);
}

#[test]
fn temp_res() {
    // 75 C: ngspice's `.options temp=75`, the context's 348.15 K
    let reference = Ngspice::from_environment().run("temp_res");
    for backend in Backend::selected() {
        compare(&reference, "v(mid)", Some(75.0 + 273.15), backend);
    }
}

#[test]
fn temp_res_at_the_default_temperature() {
    // 27 C: ngspice's `option temp=27` (the `cold` it prints), the solver's default context
    assert_eq!(Context::default().tolerances.temperature, NOMINAL, "the default temperature is ngspice's 27 C");
    let reference = Ngspice::from_environment().run("temp_res");
    for backend in Backend::selected() {
        compare(&reference, "cold", None, backend);
    }
}
