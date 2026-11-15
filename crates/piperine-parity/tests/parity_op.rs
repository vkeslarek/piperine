//! Scope: operating-point parity (requirements PAR-06 AC 1 and 2, PAR-08 AC 2): the node voltages
//! of a `Dc` solve of each twin agree with the values ngspice prints for its `.cir`, under
//! `DC_VOLTAGE`. One test per fixture, named after it.

use piperine_parity::ngspice::Ngspice;
use piperine_parity::operating_point::OperatingPoint;

#[test]
fn divider() {
    OperatingPoint { fixture: "divider", nodes: &["vin", "mid"] }.compare(&Ngspice::from_environment());
}

#[test]
fn diode_series() {
    OperatingPoint { fixture: "diode_series", nodes: &["a", "b"] }.compare(&Ngspice::from_environment());
}

#[test]
fn isrc_r() {
    OperatingPoint { fixture: "isrc_r", nodes: &["a"] }.compare(&Ngspice::from_environment());
}
