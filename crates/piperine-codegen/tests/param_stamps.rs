//! LRM-COVERS: 5.6.3
//!
//! `param_stamps` — the analog device ABI for `∂F/∂p` (dv-gradients T4,
//! DVG-04): one method on [`AnalogDevice`], defaulted to write nothing,
//! that hands parameter-derivative stamps to a caller-owned sink. The PHDL
//! device evaluates the JIT kernel; plugin/OSDI elements inherit the no-op
//! default. No allocation per call — the contract is the `&mut Vec` shape.
//!
//! T69 (`lrm-conformance-wave2`) re-reading, no new tests: the fixture's
//! `G` module (`I(p, n) <+ g * V(p, n);`) is §5.6.3's own linear
//! conductor model (Figure 5-3, `I(cond) <+ G * V(cond);`) verbatim.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{AnalogDevice, Stamp};
use piperine_solver::prelude::Context;


/// A 0.7 V source across a parametric conductor `g` (1 mA at g = 1e-3). At the
/// DC operating point V(a) = 0.7, so ∂(g·V)/∂g = V(a) = 0.7.
fn circuit() -> piperine_solver::prelude::CircuitInstance {
    let src = format!(
        "module Vsrc(inout electrical p, inout electrical n);
    parameter real dc = 0.7;
    analog V(p, n) <+ dc;
endmodule
module G(inout electrical p, inout electrical n);
    parameter real g = 1e-3;
    analog I(p, n) <+ g * V(p, n);
endmodule
module Top();
    electrical gnd, a;
    Vsrc v (.p(a), .n(gnd));
    G r (.p(a), .n(gnd));
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    // Cache the converged operating point into each device's `last_volts`.
    // A pure-analog DC solve does not call `accept` itself (that path is
    // mixed-signal), so the adjoint seeds the OP the same way before reading
    // `param_stamps`. Collect (idx, voltage) pairs first to release the
    // immutable netlist borrow before mutating.
    let pairs: Vec<(usize, f64)> = circuit
        .netlist()
        .all_references()
        .iter()
        .filter_map(|r| Some((r.idx()?, dc.get(r.variable().clone())?)))
        .collect();
    let n = pairs.iter().map(|(i, _)| i + 1).max().unwrap_or(0);
    let mut sol = vec![0.0; n];
    for (i, v) in pairs {
        sol[i] = v;
    }
    let _ = circuit.accept_and_run_digital(&sol, 0.0);
    circuit
}

fn g_index(device: &dyn piperine_solver::abi::Element) -> usize {
    device
        .list_params()
        .iter()
        .position(|p| p.name == "g")
        .expect("G exposes param `g`")
}

// ── the PHDL device pushes the analytic derivative at the operating point ────

#[test]
fn param_stamps_pushes_the_analytic_derivative_into_a_caller_sink() {
    let mut circuit = circuit();
    let r = circuit
        .devices
        .iter_mut()
        .find(|d| d.name() == "r")
        .expect("device `r`");
    let idx = g_index(r.as_ref());

    let mut sink: Vec<Stamp<piperine_solver::abi::AnalogReference, f64>> = Vec::new();
    r.param_stamps(idx, &mut sink);

    // ∂(g·V(a,gnd))/∂g = V(a) = 0.7, stamped + at `a`. The `gnd` terminal is
    // ground (not an MNA unknown) so its −0.7 stamp is dropped — one entry.
    assert_eq!(sink.len(), 1, "one stamp (the gnd terminal is ground)");
    match &sink[0] {
        Stamp::Rhs(_, v) => assert!((v - 0.7).abs() < 1e-9, "∂F/∂g = V(a) = 0.7, got {v}"),
        other => panic!("expected a Rhs stamp, got {other:?}"),
    }
}

// ── a parameter the element does not own writes nothing (not an error) ───────

#[test]
fn param_stamps_writes_nothing_for_a_parameter_the_element_does_not_own() {
    let mut circuit = circuit();
    // An out-of-range index: no dispatch block, the default writes nothing.
    let r = circuit.devices.iter_mut().find(|d| d.name() == "r").unwrap();
    let mut sink = Vec::new();
    r.param_stamps(999, &mut sink);
    assert!(sink.is_empty(), "an out-of-range index writes nothing and is not an error");

    // The voltage source has no ∂F/∂p kernel (its body is a force, not a
    // contribution) — any index writes nothing (the default path).
    let v = circuit.devices.iter_mut().find(|d| d.name() == "v").unwrap();
    v.param_stamps(0, &mut sink);
    assert!(sink.is_empty(), "a device without the kernel writes nothing");
}

// ── the sink is caller-owned and reused (no allocation per call, by shape) ───

#[test]
fn param_stamps_reuses_one_caller_owned_sink_across_calls() {
    // The signature `(&mut self, idx, &mut Vec<Stamp>) -> ()` returns nothing —
    // the caller owns the sink and reuses it across parameters. Two calls into
    // the same (cleared) sink both deliver their stamps.
    let mut circuit = circuit();
    let r = circuit.devices.iter_mut().find(|d| d.name() == "r").unwrap();
    let idx = g_index(r.as_ref());
    let mut sink: Vec<Stamp<piperine_solver::abi::AnalogReference, f64>> = Vec::new();

    r.param_stamps(idx, &mut sink);
    let first = sink.len();
    sink.clear();
    r.param_stamps(idx, &mut sink);
    assert_eq!(sink.len(), first, "the reused sink holds the second call's stamps");
    assert!(!sink.is_empty(), "the analytic path delivered stamps both times");
}

// ── the default implementation writes nothing (plugin / OSDI inherit this) ───

struct Inert;

impl piperine_solver::abi::AnalogDevice for Inert {}

#[test]
fn the_default_param_stamps_writes_nothing() {
    // A minimal AnalogDevice (e.g. a plugin or OSDI element) that does not
    // override `param_stamps` inherits the no-op default — the driver falls
    // back to stamp perturbation for it (DVG-06, T6).
    let mut inert = Inert;
    let mut sink: Vec<Stamp<piperine_solver::abi::AnalogReference, f64>> = Vec::new();
    inert.param_stamps(0, &mut sink);
    assert!(sink.is_empty(), "the default writes nothing (plugin/OSDI default clear)");
}
