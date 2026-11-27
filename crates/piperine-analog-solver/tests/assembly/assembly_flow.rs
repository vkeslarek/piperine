//! `FLOW` terms (feature `solver-abi-analog`, SAN-02, T3): a term stamps the
//! Norton form of its linearization (C2), a potential source written as
//! abi.md A1 says stamps what `branch_force` stamps today, and analytic
//! circuits (a divider, a diode fed through a resistor) solve to their known
//! values with `assemble_dc` driving this crate's own Newton engine.
//!
//! Numerically sensitive: stamps are compared with values computed here by
//! hand, never by the code under test.

use piperine_device_fixtures::analog::{Modules, VT};
use piperine_device_fixtures::assembly::Circuits;
use piperine_device_fixtures::assembly::{AssemblySolves};
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly};

#[test]
fn a_flow_term_stamps_its_norton_form() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let is = 1e-14;
    let d = Circuits::instance("d1", m.plain_diode, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(is));

    let (xa, xk) = (0.65, 0.05);
    let stamps = assembly.assemble_dc(&[xa, xk], AssemblySolves::at_rest()).expect("assemble");
    // By hand: id = is·(e − 1), gd = is·e/VT at vd = 0.6; the Norton current
    // id − gd·vd leaves the anode.
    let e = ((xa - xk) / VT).exp();
    let (id, gd) = (is * (e - 1.0), is * e / VT);
    let ieq = id - gd * (xa - xk);
    let (a, b) = AssemblySolves::dense(&stamps, 2);
    let rel = 1e-14;
    AssemblySolves::close(a[0][0], gd, rel, "(a, a)");
    AssemblySolves::close(a[0][1], -gd, rel, "(a, k)");
    AssemblySolves::close(a[1][0], -gd, rel, "(k, a)");
    AssemblySolves::close(a[1][1], gd, rel, "(k, k)");
    AssemblySolves::close(b[0], -ieq, rel, "rhs(a)");
    AssemblySolves::close(b[1], ieq, rel, "rhs(k)");
    assert_eq!(AssemblySolves::matrix_positions(&stamps).len(), 4);

    // A cathode at ground has no row: only (a, a) and rhs(a).
    let d = Circuits::instance("d2", m.plain_diode, vec![Discrete(0), Circuits::at(&n[0]), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![d], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(is));
    let stamps = assembly.assemble_dc(&[xa, 0.0], AssemblySolves::at_rest()).expect("assemble");
    assert_eq!(AssemblySolves::matrix_positions(&stamps), [(0, 0)].into());
    let (a, b) = AssemblySolves::dense(&stamps, 2);
    let e = (xa / VT).exp();
    AssemblySolves::close(a[0][0], is * e / VT, rel, "(a, a) to ground");
    AssemblySolves::close(b[0], -(is * (e - 1.0) - is * e / VT * xa), rel, "rhs(a) to ground");
    assert_eq!(b[1], 0.0);
}

#[test]
fn a_potential_source_stamps_through_its_private_pin() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &["v1.f"]);
    let (p, q, f) = (0, 1, 2);
    let v = Circuits::instance("v1", m.voltage_source, vec![Discrete(0), Circuits::at(&n[p]), Circuits::at(&n[q]), Circuits::at(&n[f])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![v], &net).expect("bind"));
    let volts = 5.0;
    assembly.set_discrete(0, &Circuits::real_bytes(volts));

    let stamps = assembly.assemble_dc(&[3.0, 1.0, 2e-3], AssemblySolves::at_rest()).expect("assemble");
    // branch_force(p, n, f, V): (p, f) 1, (f, p) 1, (n, f) −1, (f, n) −1, rhs(f) V.
    let mut want = vec![vec![0.0; 3]; 3];
    want[p][f] = 1.0;
    want[f][p] = 1.0;
    want[q][f] = -1.0;
    want[f][q] = -1.0;
    let (a, b) = AssemblySolves::dense(&stamps, 3);
    assert_eq!(a, want);
    assert_eq!(b, vec![0.0, 0.0, volts]);
    assert_eq!(AssemblySolves::matrix_positions(&stamps), [(p, f), (f, p), (q, f), (f, q)].into());
}

#[test]
fn a_divider_solves_to_its_ratio() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &["v1.f"]);
    let (top, mid, f) = (&n[0], &n[1], &n[2]);
    let v = Circuits::instance("v1", m.voltage_source, vec![Discrete(0), Circuits::at(top), Ground, Circuits::at(f)]);
    let r1 = Circuits::instance("r1", m.resistor, vec![Discrete(1), Circuits::at(top), Circuits::at(mid)]);
    let r2 = Circuits::instance("r2", m.resistor, vec![Discrete(2), Circuits::at(mid), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![v, r1, r2], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(10.0));
    assembly.set_discrete(1, &Circuits::real_bytes(1e3));
    assembly.set_discrete(2, &Circuits::real_bytes(3e3));

    let x = AssemblySolves::solve_dc(&mut assembly, &net, &vec![0.0; AssemblySolves::size(&net)]);
    let got = x[mid.idx().expect("mid")];
    assert!((got - 7.5).abs() <= 1e-12, "v(mid) = {got} vs 7.5");
}

#[test]
fn a_diode_satisfies_its_own_equation() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &["v1.f"]);
    let (vin, r, is) = (1.0, 1.0e3, 1.0e-14);
    let (top, mid, f) = (&n[0], &n[1], &n[2]);
    let v = Circuits::instance("v1", m.voltage_source, vec![Discrete(0), Circuits::at(top), Ground, Circuits::at(f)]);
    let r1 = Circuits::instance("r1", m.resistor, vec![Discrete(1), Circuits::at(top), Circuits::at(mid)]);
    let d1 = Circuits::instance("d1", m.plain_diode, vec![Discrete(2), Circuits::at(mid), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![v, r1, d1], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(vin));
    assembly.set_discrete(1, &Circuits::real_bytes(r));
    assembly.set_discrete(2, &Circuits::real_bytes(is));

    let x = AssemblySolves::solve_dc(&mut assembly, &net, &vec![0.0; AssemblySolves::size(&net)]);
    let vd = x[mid.idx().expect("mid")];
    let i_resistor = (vin - vd) / r;
    let i_diode = is * ((vd / VT).exp() - 1.0);
    assert!((i_resistor - i_diode).abs() < 1.0e-9, "i(r) = {i_resistor} A vs i(d) = {i_diode} A at vd = {vd}");
}
