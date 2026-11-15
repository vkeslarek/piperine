//! Scope: DC sweep parity (requirements PAR-06 AC 3 and PAR-08 AC 3): the current through `v1` of
//! a diode twin, solved at each sweep value of ngspice's export, agrees with ngspice's `i(v1)`
//! under `DC_CURRENT`, sign unchanged. `diode_iv` covers -1 V to 0.8 V; `diode_rev` covers
//! -1 V to -20 V, where the `gmin` shunt (1e-12 S times the bias) clears the current floor.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::Context;

/// One DC sweep comparison: the fixture, ngspice's export and the rows it prints.
struct Sweep {
    fixture: &'static str,
    export: &'static str,
    rows: usize,
}

impl Sweep {
    fn compare(&self) {
        let fixture = self.fixture;
        let reference = Ngspice::from_environment().run(fixture);
        for backend in Backend::selected() {
            self.compare_on(&reference, backend);
        }
    }

    /// The sweep on `backend` against the reference run already made.
    fn compare_on(&self, reference: &Reference, backend: Backend) {
        let fixture = self.fixture;
        let mut bench = Bench::load_on(fixture, backend);
        let source = bench.source("v1");
        let rows = match reference {
            Reference::Golden(golden) => {
                let rows = golden.real(self.export);
                golden.expect_rows(self.export, &rows, self.rows);
                rows
            }
            Reference::Skipped => vec![(0.8, 0.0)],
        };
        for (sweep, wanted) in rows {
            bench.set("v1", "dc", sweep);
            let result = bench.circuit().dc(Context::default()).expect("dc").solve().unwrap_or_else(|error| panic!("{fixture}: solve at {sweep} on {}: {error}", backend.name()));
            let actual = result.get_branch(source.clone()).unwrap_or_else(|| panic!("{fixture}: no flow of v1 at {sweep} on {}", backend.name()));
            if let Reference::Golden(golden) = reference {
                let at = Point::new(fixture, "i(v1)", Coordinate::Sweep(sweep)).with_ngspice(golden.version()).with_backend(backend.name());
                ToleranceContract::DC_CURRENT.assert(&at, wanted, actual);
            }
        }
    }
}

#[test]
fn diode_iv() {
    Sweep { fixture: "diode_iv", export: "diode_iv_sweep", rows: 37 }.compare();
}

#[test]
fn diode_rev() {
    Sweep { fixture: "diode_rev", export: "diode_rev_sweep", rows: 20 }.compare();
}
