//! The operating-point comparison of one twin, shared by the `parity_op` target and by the tests
//! that prove what a skipped reference does: the MSL side is always built and solved, the
//! comparison happens only against a real ngspice run.

use piperine_solver::prelude::Context;

use crate::bench::Bench;
use crate::contract::{Coordinate, Point, ToleranceContract};
use crate::ngspice::{Ngspice, Reference};

/// One operating-point comparison: the fixture and the nodes ngspice prints.
pub struct OperatingPoint {
    pub fixture: &'static str,
    pub nodes: &'static [&'static str],
}

/// What a comparison did: the nodes read from the MSL solve and the nodes compared with ngspice.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub solved: usize,
    pub compared: usize,
}

impl OperatingPoint {
    /// Builds and solves the twin, then compares each node under `DC_VOLTAGE` when `ngspice`
    /// gave a golden run. Panics on any failed stage or any disagreement.
    pub fn compare(&self, ngspice: &Ngspice) -> Outcome {
        let reference = ngspice.run(self.fixture);
        let mut bench = Bench::load(self.fixture);
        let ids: Vec<_> = self.nodes.iter().map(|node| bench.node(node)).collect();
        let result = bench.circuit().dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        let mut outcome = Outcome { solved: 0, compared: 0 };
        for (node, id) in self.nodes.iter().zip(&ids) {
            let actual = result.get_node(id).unwrap_or_else(|| panic!("{}: the solve holds no v({node})", self.fixture));
            outcome.solved += 1;
            let Reference::Golden(golden) = &reference else { continue };
            let wanted = golden.printed(&format!("v({node})"));
            let quantity = format!("v({node})");
            let at = Point::new(self.fixture, &quantity, Coordinate::OperatingPoint).with_ngspice(golden.version());
            ToleranceContract::DC_VOLTAGE.assert(&at, wanted, actual);
            outcome.compared += 1;
        }
        outcome
    }
}
