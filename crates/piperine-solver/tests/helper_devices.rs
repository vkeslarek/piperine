//! The hand-written element library of `tests/helpers/devices.rs` against
//! analytic values: a resistor divider, an RC step, a diode fed through a
//! resistor, and every `param_stamps` against a central difference of the
//! element's own residual. This is the proof that the library is a faithful
//! stand-in before the five formerly codegen-driven targets are ported to it.

// Each target includes the library by path: it uses a different subset of it.
#[allow(dead_code)]
#[path = "helpers/devices.rs"]
mod devices;

use devices::{node_id, C, D, R, Vsrc};
use piperine_solver::abi::{
    AnalogVariable, CircularArrayBuffer2, DcAnalysisState, Element, InitialValue, Stamp,
};
use piperine_solver::prelude::{
    AnalogReference, CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions, Value,
};

fn dc_node(circuit: &mut CircuitInstance, node: &AnalogReference) -> f64 {
    let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
    result.get_node(&node_id(node)).expect("node solved")
}

#[test]
fn divider_reads_the_analytic_ratio() {
    let (vin, r1, r2) = (7.0, 1.5e3, 4.5e3);
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    b.element(Box::new(Vsrc::new("v1", vin, top.clone(), gnd.clone())));
    b.element(Box::new(R::new("r1", r1, top, mid.clone())));
    b.element(Box::new(R::new("r2", r2, mid.clone(), gnd)));
    let mut circuit = b.build().expect("build");

    let v = dc_node(&mut circuit, &mid);
    let analytic = vin * r2 / (r1 + r2);
    assert!((v - analytic).abs() < 1.0e-9, "v(mid) = {v} vs analytic {analytic}");
}

#[test]
fn rc_step_reads_one_minus_exp() {
    let (r, c) = (1.0e3, 1.0e-6);
    let tau = r * c;
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let out = b.node("out");
    b.element(Box::new(Vsrc::new("v1", 1.0, top.clone(), gnd.clone())));
    b.element(Box::new(R::new("r1", r, top, out.clone())));
    b.element(Box::new(C::new("c1", c, out.clone(), gnd)));
    let mut circuit = b.build().expect("build");

    let mut solver = circuit
        .transient(TransientAnalysisOptions::new(tau, tau / 100.0), Context::default())
        .expect("tran");
    solver.apply_initial_conditions(vec![InitialValue { reference: out.clone(), value: 0.0 }]);
    let result = solver.solve().expect("solve");
    let last = result.last().expect("steps");

    let v = last.get_node(&node_id(&out)).expect("v(out)");
    let analytic = 1.0 - (-1.0_f64).exp();
    assert!((last.time() - tau).abs() < 1.0e-12, "stopped at t = {}", last.time());
    assert!((v - analytic).abs() < 1.0e-3, "v(rc) = {v} vs 1 - 1/e = {analytic}");
}

#[test]
fn diode_satisfies_its_own_equation() {
    let (vin, r, is_sat, vt) = (1.0, 1.0e3, 1.0e-14, 0.02585);
    let mut b = CircuitBuilder::new("diode");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    b.element(Box::new(Vsrc::new("v1", vin, top.clone(), gnd.clone())));
    b.element(Box::new(R::new("r1", r, top, mid.clone())));
    b.element(Box::new(D::new("d1", is_sat, mid.clone(), gnd)));
    let mut circuit = b.build().expect("build");

    let vd = dc_node(&mut circuit, &mid);
    let i_resistor = (vin - vd) / r;
    let i_diode = is_sat * ((vd / vt).exp() - 1.0);
    assert!(
        (i_resistor - i_diode).abs() < 1.0e-9,
        "i(r) = {i_resistor} A vs i(d) = {i_diode} A at vd = {vd}"
    );
}

/// One element between two nodes, with the operating point to differentiate at.
struct Case {
    element: &'static str,
    param: &'static str,
    /// `(V(a), V(b), I(branch))`.
    op: (f64, f64, f64),
}

fn element_for(kind: &str, a: AnalogReference, b: AnalogReference) -> Box<dyn Element> {
    match kind {
        "R" => Box::new(R::new("x", 2.0e3, a, b)),
        "D" => Box::new(D::new("x", 1.0e-14, a, b)),
        "C" => Box::new(C::new("x", 1.0e-6, a, b)),
        "Vsrc" => Box::new(Vsrc::new("x", 3.0, a, b)),
        other => panic!("no element `{other}`"),
    }
}

/// `F = A·V - b` at the fixed operating point `op`, from the element's own
/// `load_dc` stamps — the residual `param_stamps` is the parameter-derivative of.
fn residual(circuit: &mut CircuitInstance, op: &[f64]) -> Vec<f64> {
    let mut history = CircularArrayBuffer2::new(1, op.len());
    history.push(&ndarray::Array1::from_vec(op.to_vec()).view());
    let state = DcAnalysisState::new(&history, &[], 1.0);
    let ctx = Context::default();
    let stamps = circuit.all_devices_mut()[0].load_dc(&state, &ctx);
    let mut f = vec![0.0; op.len()];
    for stamp in stamps {
        match stamp {
            Stamp::Matrix(row, col, v) => {
                if let (Some(r), Some(c)) = (row.idx(), col.idx()) {
                    f[r] += v * op[c];
                }
            }
            Stamp::Rhs(row, v) => {
                if let Some(r) = row.idx() {
                    f[r] -= v;
                }
            }
        }
    }
    f
}

#[test]
fn param_stamps_match_a_central_difference() {
    let cases = [
        Case { element: "R", param: "r", op: (0.7, 0.2, 0.0) },
        Case { element: "D", param: "is_sat", op: (0.55, 0.0, 0.0) },
        Case { element: "C", param: "c", op: (0.4, 0.1, 0.0) },
        Case { element: "Vsrc", param: "dc", op: (3.0, 0.5, 0.01) },
    ];
    for case in cases {
        let mut b = CircuitBuilder::new("case");
        let a = b.node("a");
        let n = b.node("b");
        b.element(element_for(case.element, a.clone(), n.clone()));
        let mut circuit = b.build().expect("build");

        let size = circuit.netlist().max_index().map(|i| i + 1).unwrap_or(0);
        let mut op = vec![0.0; size];
        op[a.idx().expect("a is solved")] = case.op.0;
        op[n.idx().expect("b is solved")] = case.op.1;
        for r in circuit.netlist().all_references() {
            if matches!(r.variable().as_ref(), AnalogVariable::Branch(_)) {
                op[r.idx().expect("branch is solved")] = case.op.2;
            }
        }

        let mut history = CircularArrayBuffer2::new(1, size);
        history.push(&ndarray::Array1::from_vec(op.clone()).view());
        circuit.update_all(&history, &Context::default());

        let device = &mut circuit.all_devices_mut()[0];
        let index = device
            .list_params()
            .iter()
            .position(|p| p.name == case.param)
            .expect("param declared");
        let p0 = device.get_param(case.param).and_then(|v| v.as_real()).expect("real");
        let mut sink = Vec::new();
        device.param_stamps(index, &mut sink);
        let mut analytic = vec![0.0; size];
        for stamp in sink {
            if let Stamp::Rhs(row, v) = stamp
                && let Some(r) = row.idx()
            {
                analytic[r] += v;
            }
        }

        let dp = 1.0e-6 * p0.abs();
        let write = |circuit: &mut CircuitInstance, value: f64| {
            circuit.all_devices_mut()[0]
                .set_param(case.param, Value::Real(value))
                .expect("write param");
        };
        write(&mut circuit, p0 + dp);
        let plus = residual(&mut circuit, &op);
        write(&mut circuit, p0 - dp);
        let minus = residual(&mut circuit, &op);
        write(&mut circuit, p0);

        for row in 0..size {
            let fd = (plus[row] - minus[row]) / (2.0 * dp);
            let scale = analytic[row].abs().max(fd.abs());
            assert!(
                (analytic[row] - fd).abs() <= 1.0e-5 * scale + 1.0e-12,
                "{}.{} row {row}: analytic {} vs central difference {fd}",
                case.element,
                case.param,
                analytic[row]
            );
        }
    }
}
