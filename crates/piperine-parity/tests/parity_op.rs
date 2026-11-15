//! Scope: operating-point parity (requirements PAR-06 AC 1 and 2, PAR-08 AC 2): the node voltages
//! of a `Dc` solve of each twin agree with the values ngspice prints for its `.cir`, under
//! `DC_VOLTAGE`. One test per fixture, named after it.

use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_solver::prelude::Context;

/// One operating-point comparison: the fixture and the nodes ngspice prints.
struct OperatingPoint {
    fixture: &'static str,
    nodes: &'static [&'static str],
}

impl OperatingPoint {
    fn compare(&self) {
        let reference = Ngspice::from_environment().run(self.fixture);
        let mut bench = Bench::load(self.fixture);
        let ids: Vec<_> = self.nodes.iter().map(|node| bench.node(node)).collect();
        let result = bench.circuit().dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        let Reference::Golden(golden) = reference else { return };
        for (node, id) in self.nodes.iter().zip(&ids) {
            let actual = result.get_node(id).unwrap_or_else(|| panic!("{}: the solve holds no v({node})", self.fixture));
            let wanted = golden.printed(&format!("v({node})"));
            let quantity = format!("v({node})");
            let at = Point::new(self.fixture, &quantity, Coordinate::OperatingPoint).with_ngspice(golden.version());
            ToleranceContract::DC_VOLTAGE.assert(&at, wanted, actual);
        }
    }
}

#[test]
fn divider() {
    OperatingPoint { fixture: "divider", nodes: &["vin", "mid"] }.compare();
}

#[test]
fn diode_series() {
    OperatingPoint { fixture: "diode_series", nodes: &["a", "b"] }.compare();
}

#[test]
fn isrc_r() {
    OperatingPoint { fixture: "isrc_r", nodes: &["a"] }.compare();
}
