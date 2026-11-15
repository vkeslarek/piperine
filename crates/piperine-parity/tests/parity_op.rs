//! Scope: operating-point parity (requirements PAR-06 AC 1 and 2, PAR-08 AC 2, RS-07): the node voltages
//! of a `Dc` solve of each twin, on each selected backend, agree with the values ngspice prints for
//! its `.cir`, under `DC_VOLTAGE`. One test per fixture, named after it; ngspice runs once per twin.

use piperine_parity::backend::Backend;
use piperine_parity::ngspice::Ngspice;
use piperine_parity::operating_point::OperatingPoint;

/// Runs ngspice once for the twin and holds every selected backend to it.
fn compare(twin: OperatingPoint) {
    let reference = Ngspice::from_environment().run(twin.fixture);
    for backend in Backend::selected() {
        twin.compare_on(&reference, backend);
    }
}

#[test]
fn divider() {
    compare(OperatingPoint { fixture: "divider", nodes: &["vin", "mid"] });
}

#[test]
fn diode_series() {
    compare(OperatingPoint { fixture: "diode_series", nodes: &["a", "b"] });
}

#[test]
fn isrc_r() {
    compare(OperatingPoint { fixture: "isrc_r", nodes: &["a"] });
}
