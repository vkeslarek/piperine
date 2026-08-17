//! Regression tests for Part A + D of `docs/GAPS.md`: the silent-zero /
//! silent-skip failure modes (`parse_and_elaborate`, `lower_bodies`,
//! `CircuitCompiler`), re-expressed against the Verilog-AMS frontend.
//!
//! Part D's user-`fn` inlining cases (D.5) are reduced to the fail-loud half:
//! VAMS `function`/`analog function` declarations are not lowered into the
//! POM at all today (`resolve/pom/body.rs` Pass 1.5, MD-45), so a *calling*
//! test would assert against a frontend that cannot produce the callee. The
//! "an unknown callee must fail loudly, never emit a silent zero" guard —
//! the actual regression this section exists for — is unchanged.

use std::collections::HashMap;

use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::SimCtx;
use piperine_codegen::{CircuitCompiler, DigitalKernel};
use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::parse_and_elaborate;

const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

const DISCIPLINES: &str = "`include \"disciplines.vams\"";

/// The `SourceMap` reaching `piperine-lang`'s bundled headers from this crate
/// (`SourceMap::dummy`'s relative paths only resolve from that crate's dir).
fn source_map() -> piperine_lang::SourceMap {
    let headers =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn ident(name: &str) -> Expr {
    Expr::Ident { span: SP, name: name.to_string() }
}

// ── A.5 — Neg in digital still works (POM path) ─────────────────────────────

#[test]
fn a5_neg_in_digital_still_works() {
    use piperine_codegen::resolve::{DigitalBody, Domain, Type};
    use piperine_lang::ast::UnOp;

    let mut module = LoweredBody::new("neg_fsm");
    let param_x = module.symbols.add_param("x", Type::Integer, None);
    let _ = param_x;
    let node_out = module.symbols.add_node("out", Domain::Digital);
    module.digital = Some(DigitalBody {
        inputs: Vec::new(),
        outputs: vec![node_out],
        regs: Vec::new(),
        reg_inits: Vec::new(),
        stmts: vec![Stmt::Assign {
            span: SP,
            target: ident("out"),
            value: Expr::Unary { span: SP, op: UnOp::Neg, operand: Box::new(ident("x")) },
        }],
    });
    DigitalKernel::compile(&module).expect("Neg in digital must still work");
}

// ── A.6 — CircuitCompiler propagates child compile errors ──────────────────

#[test]
fn a6_from_ir_propagates_child_compile_error_not_silent_skip() {
    use piperine_codegen::resolve::{AnalogBody, NatureKind};

    // `vsource` exists in the POM (so the top's instance connection/port
    // count resolves normally); its *resolved body* is then swapped for a
    // hand-corrupted one — deliberately invalid: references a nonexistent
    // identifier — to check the error names both the instance and the
    // module when CircuitCompiler tries to compile it.
    let src = format!(
        "{DISCIPLINES}
        module vsource (p, n);
            inout electrical p, n;
        endmodule
        module top (a, b);
            inout electrical a, b;
            vsource u1 (.p(a), .n(b));
        endmodule
    "
    );
    let design = parse_and_elaborate(&src, &source_map()).expect("elab");
    let mut bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering failed");

    let vsource = bodies.get_mut("vsource").expect("vsource lowered");
    let _nature_i = vsource.symbols.add_nature("I", NatureKind::Flow);
    vsource.analog = Some(AnalogBody {
        states: Vec::new(),
        noise: Vec::new(),
        stmts: vec![Stmt::Contribution {
            span: SP,
            target: Expr::AccessFunction {
                span: SP,
                function: "I".into(),
                pos: Box::new(ident("p")),
                neg: Some(Box::new(ident("n"))),
            },
            value: ident("nonexistent_var_xyz"),
        }],
    });

    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let err = compiler.build_circuit("top").err().expect("top with bad child must bubble error");
    let msg = err.to_string();
    assert!(msg.contains("u1"), "A.6: error should name instance u1, got: {msg}");
    assert!(msg.contains("vsource"), "A.6: error should name module vsource, got: {msg}");
}

// ── A.7 — from_elab ddt rejection (now tests IR path handles ddt) ──────────

#[test]
fn a7_capacitor_compiles_through_ir_path() {
    let src = format!(
        "{DISCIPLINES}
        module Cap (p, n);
            inout electrical p, n;
            parameter real c = 1.0e-9;
            analog I(p, n) <+ c * ddt(V(p, n));
        endmodule
    "
    );
    let elab = parse_and_elaborate(&src, &source_map()).expect("VAMS parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "Cap").expect("capacitor compiles through IR path");
    assert!(dev.has_reactive(), "capacitor must have reactive contributions");
}

// ── D.5 — an unknown callee fails loudly, never emits a silent zero ─────────

#[test]
fn d5_user_fn_missing_still_errors() {
    let src = format!(
        "{DISCIPLINES}
        module Bad (p, n);
            inout electrical p, n;
            parameter real r = 1.0e3;
            analog I(p, n) <+ no_such_fn(V(p, n));
        endmodule
    "
    );
    let elab = parse_and_elaborate(&src, &source_map()).expect("VAMS parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let result = ir_analog_to_device(&bodies, "Bad");
    let err = result.err().expect("D.5: missing fn must fail loudly");
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("no_such_fn") || msg.contains("unknown") || msg.contains("unsupported"));
}

// ── D.2 — idt integration operator ─────────────────────────────────────────

#[test]
fn d2_idt_in_contribution_lowers_to_integrator() {
    let src = format!(
        "{DISCIPLINES}
        module Inductor (p, n);
            inout electrical p, n;
            parameter real L = 1.0e-6;
            analog I(p, n) <+ idt(V(p, n)) / L;
        endmodule
    "
    );
    let elab = parse_and_elaborate(&src, &source_map()).expect("VAMS parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "Inductor").expect("D.2: idt must compile");
    // idt is a runtime-serviced integrator (state + dt·x), not a charge.
    assert_eq!(dev.runtime_states().len(), 1, "D.2: idt allocates one integrator state");
    assert!(!dev.has_reactive(), "D.2: idt is not a charge contribution");
    let params = [1.0e-6_f64];
    let v = [0.5_f64, 0.0_f64];
    // DC (step = 0, state = 0): I = state/L = 0.
    let mut rhs = [0.0_f64; 2];
    dev.eval_residual(&v, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);
    assert!(rhs[0].abs() < 1e-12_f64, "D.2: DC residual near 0, got {}", rhs[0]);
    // In-step (dt = 1e-3, accumulated y = 2e-6): I = (y + dt·V)/L.
    let sim = SimCtx { step: 1.0e-3, ..Default::default() };
    let state = vec![2.0e-6_f64; dev.num_state_slots()];
    let mut rhs = [0.0_f64; 2];
    dev.eval_residual(&v, &params, &state, &[], &sim, &mut rhs);
    let expected = (2.0e-6 + 1.0e-3 * 0.5) / 1.0e-6;
    assert!(
        (rhs[0] - expected).abs() < 1e-9_f64 * expected,
        "D.2: I = (y + dt·V)/L, got {}",
        rhs[0]
    );
}

// ── D.1 — voltage force V(p,n) <+ expr ─────────────────────────────────────

#[test]
fn d1_voltage_force_compiles_with_force_residual() {
    let src = format!(
        "{DISCIPLINES}
        module VSource (p, n);
            inout electrical p, n;
            parameter real dc = 1.5;
            analog V(p, n) <+ dc;
        endmodule
    "
    );
    let elab = parse_and_elaborate(&src, &source_map()).expect("VSource parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "VSource").expect("D.1: VSource compiles");
    assert!(dev.num_forces() > 0, "D.1: must have force function");
    let params = [1.5_f64];
    let v = [1.2_f64, 0.4_f64];
    let mut rhs = [0.0_f64; 1];
    dev.eval_force(&v, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);
    assert!((rhs[0] - 1.5_f64).abs() < 1e-12_f64, "D.1: force E = dc = {}", rhs[0]);
}

#[test]
fn d1_op_amp_with_force_compiles() {
    let src = format!(
        "{DISCIPLINES}
        module OpAmp (inp, inn, out);
            input electrical inp, inn;
            inout electrical out;
            parameter real gain = 1.0e6;
            analog V(out) <+ gain * V(inp, inn);
        endmodule
    "
    );
    let elab = parse_and_elaborate(&src, &source_map()).expect("OpAmp parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "OpAmp").expect("D.1: OpAmp compiles");
    assert!(dev.num_forces() > 0, "D.1: OpAmp must have force function");
}

fn ir_analog_to_device(
    bodies: &HashMap<String, LoweredBody>,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let module = bodies.get(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).cloned()
}
