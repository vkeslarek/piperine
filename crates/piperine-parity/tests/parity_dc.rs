//! Scope: DC sweep parity (requirement PAR-06 AC 3): the current through `v1` of the diode twin,
//! solved at each sweep value of ngspice's export, agrees with ngspice's `i(v1)` under
//! `DC_CURRENT`, sign unchanged.

use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::Context;

#[test]
fn diode_iv() {
    let reference = Ngspice::from_environment().run("diode_iv");
    let mut bench = Bench::load("diode_iv");
    let source = bench.source("v1");
    let rows = match &reference {
        Reference::Golden(golden) => {
            let rows = golden.real("diode_iv_sweep");
            golden.expect_rows("diode_iv_sweep", &rows, 37);
            rows
        }
        Reference::Skipped => vec![(0.8, 0.0)],
    };
    for (sweep, wanted) in rows {
        bench.set("v1", "dc", sweep);
        let result = bench.circuit().dc(Context::default()).expect("dc").solve().unwrap_or_else(|error| panic!("diode_iv: solve at {sweep}: {error}"));
        let actual = result.get_branch(source.clone()).unwrap_or_else(|| panic!("diode_iv: no flow of v1 at {sweep}"));
        if let Reference::Golden(golden) = &reference {
            let at = Point::new("diode_iv", "i(v1)", Coordinate::Sweep(sweep)).with_ngspice(golden.version());
            ToleranceContract::DC_CURRENT.assert(&at, wanted, actual);
        }
    }
}
