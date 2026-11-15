//! `∂F/∂param` of an instance (feature `solver-abi-integration`, T8; abi.md
//! §10 "Host procedures"): the declared partial of a `FLOW` term against a
//! discrete pin, scaled by the derivative of the pin's bytes with respect to
//! the param, as residual-shaped right-side stamps. Values are computed here
//! by hand.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use circuit::{at, f64_bytes, instance, netlist};
use devices::modules;
use piperine_analog_solver::device::AnalogBinding::Discrete;
use piperine_analog_solver::device::{AnalogPlan, Assembly};
use piperine_analog_solver::math::linear::Stamp;
use system::{at_rest, close};

#[test]
fn a_flow_term_has_its_declared_derivative_against_a_param() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let r = 2.0e3;
    let res = instance("r1", m.resistor, vec![Discrete(0), at(&n[0]), at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![res], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(r));
    let x = [0.7, 0.2];

    // F_p = (V_p - V_n)/r, so dF_p/dr = -(V_p - V_n)/r^2 and the opposite at n.
    let stamps = asm.param_stamps(0, &[(0, 1.0)], &x, at_rest()).expect("stamps");
    let want = -(x[0] - x[1]) / (r * r);
    let b = rhs(&stamps, 2);
    close(b[0], want, 1e-14, "rhs(p)");
    close(b[1], -want, 1e-14, "rhs(n)");

    // The pin's bytes depend on the param with derivative 3: the chain scales the stamp.
    let chained = asm.param_stamps(0, &[(0, 3.0)], &x, at_rest()).expect("stamps");
    close(rhs(&chained, 2)[0], 3.0 * want, 1e-14, "chained rhs(p)");

    // A continuous pin is never a param: no stamp.
    assert!(asm.param_stamps(0, &[(1, 1.0)], &x, at_rest()).expect("stamps").is_empty());
}

#[test]
fn a_ddt_term_has_no_equilibrium_derivative() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let cap = instance("c1", m.capacitor, vec![Discrete(0), at(&n[0]), at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![cap], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(1e-6));
    assert!(asm.param_stamps(0, &[(0, 1.0)], &[0.4, 0.1], at_rest()).expect("stamps").is_empty());
}

/// The right-side stamps as a dense vector.
fn rhs(stamps: &[Stamp<piperine_analog_solver::netlist::AnalogReference, f64>], size: usize) -> Vec<f64> {
    let mut b = vec![0.0; size];
    for s in stamps {
        match s {
            Stamp::Rhs(r, v) => b[r.idx().expect("a row")] += v,
            Stamp::Matrix(..) => panic!("a parameter derivative stamps only the right side"),
        }
    }
    b
}
