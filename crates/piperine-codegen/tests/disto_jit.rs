//! `.disto` derivative-kernel tests (DISTO-03/04): the JIT'd second
//! derivatives are checked value-for-value against hand-derived symbolic
//! references on polynomial devices, and the unlowerable path fails loud.

use piperine_codegen::ir::NodeId;
use piperine_codegen::{CompiledModule, SimCtx};
use piperine_lang::parse_and_elaborate;

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }\n";

/// Compile `src` (module defs only) and return the kernel + the named
/// module's node-id resolver.
fn compile_module(
    src: &str,
    name: &str,
) -> (std::sync::Arc<piperine_codegen::AnalogKernel>, impl Fn(&str) -> NodeId) {
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elab");
    let bodies = piperine_codegen::ir::lower_bodies(&prog).expect("lowering failed");
    let module = bodies.get(name).expect("module present");
    let nodes: Vec<(NodeId, String)> = module
        .symbols
        .nodes()
        .map(|(id, info)| (id, info.name.clone()))
        .collect();
    let compiled = CompiledModule::compile(module).expect("compile");
    let kernel = compiled.analog().expect("analog body").clone();
    let resolve = move |name: &str| -> NodeId {
        nodes
            .iter()
            .find(|(_, n)| n == name)
            .map(|(id, _)| *id)
            .unwrap_or(NodeId::GROUND)
    };
    (kernel, resolve)
}

/// Compile `src`, expecting failure.
fn compile_err(src: &str, name: &str) -> piperine_codegen::CodegenError {
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elab");
    let bodies = piperine_codegen::ir::lower_bodies(&prog).expect("lowering failed");
    match CompiledModule::compile(bodies.get(name).expect("module present")) {
        Err(e) => e,
        Ok(_) => panic!("device `{name}` must not compile"),
    }
}

/// `i = g1·v + g2·v² + g3·v³` from (inp,inn) into (outp,outn), with the
/// controlling voltage held in a `var` so every contribution routes through
/// the shared value tape (DISTO-03): `f''(v) = 2·g2 + 6·g3·v` is the only
/// nonzero second derivative.
#[test]
fn disto2_polynomial_vccs_matches_symbolic_second_derivative() {
    let src = format!(
        "{DISCIPLINE}
mod PolyVccs ( inout inp : Electrical, inout inn : Electrical,
               inout outp : Electrical, inout outn : Electrical ) {{
    param g1 : Real = 0.1;
    param g2 : Real = 0.02;
    param g3 : Real = 0.003;
}}
analog PolyVccs {{
    var v_in : Real;
    v_in = V(inp, inn);
    I(outp, outn) <+ g1 * v_in + g2 * v_in * v_in + g3 * v_in * v_in * v_in;
}}
"
    );
    let (kernel, node) = compile_module(&src, "PolyVccs");
    assert!(kernel.has_disto2(), "nonlinear device must carry a disto2 kernel");

    // Only the ((inp,inn),(inp,inn)) pair has a nonzero row.
    let expected_pair = ((node("inp"), node("inn")), (node("inp"), node("inn")));
    assert_eq!(kernel.disto2_pairs(), &[expected_pair]);
    assert_eq!(kernel.disto2_contribs(), &[(node("outp"), node("outn"))]);
    assert_eq!(kernel.disto2_charge_start(), 1);

    let (g2, g3) = (0.02_f64, 0.003_f64);
    let v_in = 0.4_f64;
    let volts = [v_in, 0.0, 0.0, 0.0];
    let params = [0.1, g2, g3];
    let mut out = [0.0_f64; 1];
    kernel.eval_disto2(&volts, &params, &[], &[], &SimCtx::default(), &mut out);

    let expected = 2.0 * g2 + 6.0 * g3 * v_in;
    assert!(
        (out[0] - expected).abs() < 1e-15,
        "f''(0.4) = {} vs symbolic {}",
        out[0],
        expected
    );
}

/// `i = k·V(outp,outn)·V(inp,inn)`: the cross derivatives
/// `∂²i/∂V(out)∂V(in) = ∂²i/∂V(in)∂V(out) = k` are the only nonzero rows.
#[test]
fn disto2_cross_derivative_matches_symbolic_reference() {
    let src = format!(
        "{DISCIPLINE}
mod CrossDev ( inout inp : Electrical, inout inn : Electrical,
               inout outp : Electrical, inout outn : Electrical ) {{
    param k : Real = 0.5;
}}
analog CrossDev {{
    I(outp, outn) <+ k * V(outp, outn) * V(inp, inn);
}}
"
    );
    let (kernel, node) = compile_module(&src, "CrossDev");
    assert!(kernel.has_disto2());

    let out_branch = (node("outp"), node("outn"));
    let in_branch = (node("inp"), node("inn"));
    let mut want = [(out_branch, in_branch), (in_branch, out_branch)];
    want.sort();
    let mut got = kernel.disto2_pairs().to_vec();
    got.sort();
    assert_eq!(got, want, "only the two cross pairs survive");

    let volts = [0.3, 0.1, 0.7, 0.2];
    let params = [0.5_f64];
    let mut out = [0.0_f64; 2];
    kernel.eval_disto2(&volts, &params, &[], &[], &SimCtx::default(), &mut out);
    for (row, value) in out.iter().enumerate() {
        assert!(
            (value - 0.5).abs() < 1e-15,
            "cross derivative row {row} = {value} vs k = 0.5"
        );
    }
}

/// Nonlinear charge `q = c1·v + c2·v²`: `q'' = 2·c2` lands in the charge
/// row; the (literal-zero) resistive row stays zero.
#[test]
fn disto2_nonlinear_charge_second_derivative() {
    let src = format!(
        "{DISCIPLINE}
mod NlCap ( inout p : Electrical, inout n : Electrical ) {{
    param c1 : Real = 1e-6;
    param c2 : Real = 1e-7;
}}
analog NlCap {{
    I(p, n) <+ ddt(c1 * V(p, n) + c2 * V(p, n) * V(p, n));
}}
"
    );
    let (kernel, node) = compile_module(&src, "NlCap");
    assert!(kernel.has_disto2(), "nonlinear charge must carry a disto2 kernel");

    let branch = (node("p"), node("n"));
    assert_eq!(kernel.disto2_pairs(), &[(branch, branch)]);
    // Resistive row (literal 0) then the charge row.
    assert_eq!(kernel.disto2_contribs(), &[branch, branch]);
    assert_eq!(kernel.disto2_charge_start(), 1);

    let volts = [0.9, 0.1];
    let params = [1e-6_f64, 1e-7];
    let mut out = [0.0_f64; 2];
    kernel.eval_disto2(&volts, &params, &[], &[], &SimCtx::default(), &mut out);
    assert_eq!(out[0], 0.0, "resistive row folds to zero");
    assert!(
        (out[1] - 2.0e-7).abs() < 1e-19,
        "q'' = {} vs 2·c2 = 2e-7",
        out[1]
    );
}

/// DISTO-04: a contribution reading a branch current `I(...)` has no
/// voltage-pair second derivative — fail loud, naming the device.
#[test]
fn disto2_branch_current_read_fails_loud_naming_device() {
    let src = format!(
        "{DISCIPLINE}
mod ProbeDev ( inout p : Electrical, inout n : Electrical, inout q : Electrical ) {{
    param k : Real = 1.0;
}}
analog ProbeDev {{
    I(p, n) <+ k * V(p, n) * I(q, 0);
}}
"
    );
    let err = compile_err(&src, "ProbeDev");
    match err {
        piperine_codegen::CodegenError::Unsupported(msg) => {
            assert!(msg.contains("ProbeDev"), "error must name the device: {msg}")
        }
        other => panic!("expected CodegenError::Unsupported, got {other}"),
    }
}
