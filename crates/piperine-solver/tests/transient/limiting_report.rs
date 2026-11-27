//! Limiting convergence-gate tests (ABI-10) on the `Device` trait: an active
//! `LIMIT` input vetoes Newton convergence — the solver does not declare
//! convergence at a clamped non-solution — and a device that limits and then
//! settles reaches the operating point the unlimited device reaches.
//!
//! The guess is no longer steered by the solver (the held iterate is the
//! device's own `LIMITED` value), so the old claim that a reported
//! `limited_value` overwrites the Newton guess is gone with its hook.

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::solver_devices::limiting_report::{AlwaysLimited, LimitedDiode};

use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Context, Solver};


/// Spec ABI-10: an always-active limiter vetoes Newton convergence — the
/// solver does NOT declare convergence at a clamped non-solution. A voltage
/// source forcing a node to 1 V while a limiter keeps its iterate 0.3 V away
/// cannot settle, so the solve fails to converge (the limiter keeps vetoing).
/// This proves the gate reads the limit's activity, not a stale boolean.
#[test]
fn active_limiter_vetoes_newton_convergence() {
    let mut b = CircuitBuilder::new("veto");
    let gnd = b.ground();
    let a = b.node("a");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &a, &gnd);
    b.device(AlwaysLimited::module(), "alwolim", &[], &[Port::Node(a)]);
    let circuit = b.build().expect("build");
    let mut solver = Solver::new(circuit).build();
    let res = solver.dc().unwrap().solve();

    // The limiter vetoes convergence permanently → the DC plan fails.
    assert!(res.is_err(), "an always-active limiter must veto convergence");
}

/// A limiter that limits the early iterates and then releases reaches the
/// operating point the unlimited diode reaches: the limit steers the path,
/// never the fixed point.
#[test]
fn a_limited_diode_reaches_the_unlimited_operating_point() {
    let solve = |limited: bool| {
        let mut b = CircuitBuilder::new("diode");
        let gnd = b.ground();
        let top = b.node("top");
        let mid = b.node("mid");
        CircuitBuilding::add_voltage_source(&mut b, "v1", 5.0, &top, &gnd);
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
        if limited {
            b.device(
                LimitedDiode::module(),
                "d1",
                &[("is_sat", ParamValue::Real(1.0e-14))],
                &[Port::Node(mid.clone()), Port::Node(gnd.clone())],
            );
        } else {
            CircuitBuilding::add_diode(&mut b, "d1", 1.0e-14, &mid, &gnd);
        }
        let circuit = b.build().expect("build");
        let mut solver = Solver::new(circuit).with_context(Context::default()).build();
        solver.dc().unwrap().solve().unwrap().get_node(&CircuitBuilding::node_id(&mid)).unwrap()
    };
    let (limited, plain) = (solve(true), solve(false));
    assert!(plain > 0.5 && plain < 0.9, "the diode conducts: {plain}");
    // Both stop inside Newton's own criterion (vntol + reltol·|v| ≈ 7e-4 V).
    assert!((limited - plain).abs() < 1e-4, "limited {limited} vs unlimited {plain}");
}

