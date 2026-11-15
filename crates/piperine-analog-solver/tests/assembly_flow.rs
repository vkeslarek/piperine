//! `FLOW` terms (feature `solver-abi-analog`, SAN-02, T3): a term stamps the
//! Norton form of its linearization (C2), a potential source written as
//! abi.md A1 says stamps what `branch_force` stamps today, and analytic
//! circuits (a divider, a diode fed through a resistor) solve to their known
//! values with `assemble_dc` driving this crate's own Newton engine.
//!
//! Numerically sensitive: stamps are compared with values computed here by
//! hand, never by the code under test.

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
use devices::{VT, modules};
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly};
use system::{at_rest, close, dense, matrix_positions, size, solve_dc};

#[test]
fn a_flow_term_stamps_its_norton_form() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let is = 1e-14;
    let d = instance("d1", m.plain_diode, vec![Discrete(0), at(&n[0]), at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(is));

    let (xa, xk) = (0.65, 0.05);
    let stamps = asm.assemble_dc(&[xa, xk], at_rest()).expect("assemble");
    // By hand: id = is·(e − 1), gd = is·e/VT at vd = 0.6; the Norton current
    // id − gd·vd leaves the anode.
    let e = ((xa - xk) / VT).exp();
    let (id, gd) = (is * (e - 1.0), is * e / VT);
    let ieq = id - gd * (xa - xk);
    let (a, b) = dense(&stamps, 2);
    let rel = 1e-14;
    close(a[0][0], gd, rel, "(a, a)");
    close(a[0][1], -gd, rel, "(a, k)");
    close(a[1][0], -gd, rel, "(k, a)");
    close(a[1][1], gd, rel, "(k, k)");
    close(b[0], -ieq, rel, "rhs(a)");
    close(b[1], ieq, rel, "rhs(k)");
    assert_eq!(matrix_positions(&stamps).len(), 4);

    // A cathode at ground has no row: only (a, a) and rhs(a).
    let d = instance("d2", m.plain_diode, vec![Discrete(0), at(&n[0]), Ground]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(is));
    let stamps = asm.assemble_dc(&[xa, 0.0], at_rest()).expect("assemble");
    assert_eq!(matrix_positions(&stamps), [(0, 0)].into());
    let (a, b) = dense(&stamps, 2);
    let e = (xa / VT).exp();
    close(a[0][0], is * e / VT, rel, "(a, a) to ground");
    close(b[0], -(is * (e - 1.0) - is * e / VT * xa), rel, "rhs(a) to ground");
    assert_eq!(b[1], 0.0);
}

#[test]
fn a_potential_source_stamps_through_its_private_pin() {
    let m = modules();
    let (net, n) = netlist(2, &["v1.f"]);
    let (p, q, f) = (0, 1, 2);
    let v = instance("v1", m.vsource, vec![Discrete(0), at(&n[p]), at(&n[q]), at(&n[f])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![v], &net).expect("bind"));
    let volts = 5.0;
    asm.set_discrete(0, &f64_bytes(volts));

    let stamps = asm.assemble_dc(&[3.0, 1.0, 2e-3], at_rest()).expect("assemble");
    // branch_force(p, n, f, V): (p, f) 1, (f, p) 1, (n, f) −1, (f, n) −1, rhs(f) V.
    let mut want = vec![vec![0.0; 3]; 3];
    want[p][f] = 1.0;
    want[f][p] = 1.0;
    want[q][f] = -1.0;
    want[f][q] = -1.0;
    let (a, b) = dense(&stamps, 3);
    assert_eq!(a, want);
    assert_eq!(b, vec![0.0, 0.0, volts]);
    assert_eq!(matrix_positions(&stamps), [(p, f), (f, p), (q, f), (f, q)].into());
}

#[test]
fn a_divider_solves_to_its_ratio() {
    let m = modules();
    let (net, n) = netlist(2, &["v1.f"]);
    let (top, mid, f) = (&n[0], &n[1], &n[2]);
    let v = instance("v1", m.vsource, vec![Discrete(0), at(top), Ground, at(f)]);
    let r1 = instance("r1", m.resistor, vec![Discrete(1), at(top), at(mid)]);
    let r2 = instance("r2", m.resistor, vec![Discrete(2), at(mid), Ground]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![v, r1, r2], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(10.0));
    asm.set_discrete(1, &f64_bytes(1e3));
    asm.set_discrete(2, &f64_bytes(3e3));

    let x = solve_dc(&mut asm, &net, &vec![0.0; size(&net)]);
    let got = x[mid.idx().expect("mid")];
    assert!((got - 7.5).abs() <= 1e-12, "v(mid) = {got} vs 7.5");
}

#[test]
fn a_diode_satisfies_its_own_equation() {
    let m = modules();
    let (net, n) = netlist(2, &["v1.f"]);
    let (vin, r, is) = (1.0, 1.0e3, 1.0e-14);
    let (top, mid, f) = (&n[0], &n[1], &n[2]);
    let v = instance("v1", m.vsource, vec![Discrete(0), at(top), Ground, at(f)]);
    let r1 = instance("r1", m.resistor, vec![Discrete(1), at(top), at(mid)]);
    let d1 = instance("d1", m.plain_diode, vec![Discrete(2), at(mid), Ground]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![v, r1, d1], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(vin));
    asm.set_discrete(1, &f64_bytes(r));
    asm.set_discrete(2, &f64_bytes(is));

    let x = solve_dc(&mut asm, &net, &vec![0.0; size(&net)]);
    let vd = x[mid.idx().expect("mid")];
    let i_resistor = (vin - vd) / r;
    let i_diode = is * ((vd / VT).exp() - 1.0);
    assert!((i_resistor - i_diode).abs() < 1.0e-9, "i(r) = {i_resistor} A vs i(d) = {i_diode} A at vd = {vd}");
}
