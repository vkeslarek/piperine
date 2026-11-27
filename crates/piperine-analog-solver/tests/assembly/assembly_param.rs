//! `∂F/∂param` of an instance (feature `solver-abi-integration`, T8; abi.md
//! §10 "Host procedures"): the declared partial of a `FLOW` term against a
//! discrete pin, scaled by the derivative of the pin's bytes with respect to
//! the param, as residual-shaped right-side stamps. Values are computed here
//! by hand.

use piperine_device_fixtures::analog::{Modules};
use piperine_device_fixtures::assembly::Circuits;
use piperine_device_fixtures::assembly::{AssemblySolves};
use piperine_analog_solver::device::AnalogBinding::Discrete;
use piperine_analog_solver::device::{AnalogPlan, Assembly};
use piperine_analog_solver::math::linear::Stamp;

#[test]
fn a_flow_term_has_its_declared_derivative_against_a_param() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let r = 2.0e3;
    let res = Circuits::instance("r1", m.resistor, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![res], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(r));
    let x = [0.7, 0.2];

    // F_p = (V_p - V_n)/r, so dF_p/dr = -(V_p - V_n)/r^2 and the opposite at n.
    let stamps = assembly.param_stamps(0, &[(0, 1.0)], &x, AssemblySolves::at_rest()).expect("stamps");
    let want = -(x[0] - x[1]) / (r * r);
    let b = ParamCircuits::rhs(&stamps, 2);
    AssemblySolves::close(b[0], want, 1e-14, "rhs(p)");
    AssemblySolves::close(b[1], -want, 1e-14, "rhs(n)");

    // The pin's bytes depend on the param with derivative 3: the chain scales the stamp.
    let chained = assembly.param_stamps(0, &[(0, 3.0)], &x, AssemblySolves::at_rest()).expect("stamps");
    AssemblySolves::close(ParamCircuits::rhs(&chained, 2)[0], 3.0 * want, 1e-14, "chained rhs(p)");

    // A continuous pin is never a param: no stamp.
    assert!(assembly.param_stamps(0, &[(1, 1.0)], &x, AssemblySolves::at_rest()).expect("stamps").is_empty());
}

#[test]
fn a_ddt_term_has_no_equilibrium_derivative() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let cap = Circuits::instance("c1", m.capacitor, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![cap], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(1e-6));
    assert!(assembly.param_stamps(0, &[(0, 1.0)], &[0.4, 0.1], AssemblySolves::at_rest()).expect("stamps").is_empty());
}

/// The circuits the param tests build.
pub struct ParamCircuits;

impl ParamCircuits {
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
}
