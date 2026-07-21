//! Regression tests for Part A + D of `docs/GAPS.md`.
//! PHDL-specific tests (parse_and_elaborate, lower_bodies, CircuitCompiler).

use std::collections::HashMap;

use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::{CircuitCompiler, DigitalKernel};

// ── A.5 — Neg in digital still works (POM path) ─────────────────────────────

#[test]
fn a5_neg_in_digital_still_works() {
    use piperine_codegen::resolve::{DigitalBody, Domain, Type};
    use piperine_lang::parse::ast::{BindOp, Expr, Stmt, UnaryOp};

    let mut module = LoweredBody::new("neg_fsm");
    let param_x = module.symbols.add_param("x", Type::Integer, None);
    let _ = param_x;
    let node_out = module.symbols.add_node("out", Domain::Digital);
    module.digital = Some(DigitalBody {
        inputs: Vec::new(),
        outputs: vec![node_out],
        regs: Vec::new(),
        stmts: vec![Stmt::Bind {
            dest: Expr::Ident("out".into()),
            op: BindOp::Assign,
            src: Expr::Unary(UnaryOp::Neg, Box::new(Expr::Ident("x".into()))),
        }],
    });
    DigitalKernel::compile(&module).expect("Neg in digital must still work");
}

// ── A.6 — CircuitCompiler propagates child compile errors ──────────────────

#[test]
fn a6_from_ir_propagates_child_compile_error_not_silent_skip() {
    use piperine_codegen::resolve::{AnalogBody, NatureKind};
    use piperine_lang::parse::ast::{BindOp, Expr, Stmt};

    // `vsource` exists in the POM (so the top's instance connection/port
    // count resolves normally); its *resolved body* is then swapped for a
    // hand-corrupted one — deliberately invalid: references a nonexistent
    // identifier — to check the error names both the instance and the
    // module when CircuitCompiler tries to compile it.
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod vsource (inout p: Electrical, inout n: Electrical) {}
        mod top (inout a: Electrical, inout b: Electrical) { u1 : vsource(.p = a, .n = b); }
    ";
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elab");
    let mut bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering failed");

    let vsource = bodies.get_mut("vsource").expect("vsource lowered");
    let _nature_i = vsource.symbols.add_nature("I", NatureKind::Flow);
    vsource.analog = Some(AnalogBody {
        states: Vec::new(),
        noise: Vec::new(),
        stmts: vec![Stmt::Bind {
            dest: Expr::Call(
                Box::new(Expr::Ident("I".into())),
                vec![Expr::Ident("p".into()), Expr::Ident("n".into())],
            ),
            op: BindOp::Contrib,
            src: Expr::Ident("nonexistent_var_xyz".into()),
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
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Cap ( inout p : Electrical, inout n : Electrical ) { param c : Real = 1.0e-9; }
        analog Cap { I(p, n) <+ c * ddt(V(p, n)); }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "Cap").expect("capacitor compiles through IR path");
    assert!(dev.has_reactive(), "capacitor must have reactive contributions");
}

// ── D.5 — User fn inlined at call site ─────────────────────────────────────

#[test]
fn d5_user_fn_inlined_at_call_site_in_contribution() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        fn scale_v (x : Real) -> Real { return x * 2.0; }
        mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
        analog Resistor { I(p, n) <+ scale_v(V(p, n)) / r; }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "Resistor").expect("D.5: user-fn inlining must compile");
    let params = [1.0e3_f64];
    let v = [0.5_f64, 0.0_f64];
    let mut rhs = [0.0_f64; 2];
    dev.eval_residual(&v, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);
    let expected = 2.0 * 0.5 / 1.0e3;
    assert!((rhs[0] - expected).abs() < 1e-9_f64);
}

#[test]
fn d5_user_fn_call_to_nonbuiltin_is_inlined_not_silently_zero() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        fn amp (x : Real, g : Real) -> Real { return g * x; }
        mod Gain ( inout p : Electrical, inout n : Electrical ) { param g : Real = 2.0; }
        analog Gain { I(p, n) <+ amp(V(p, n), g); }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "Gain").expect("D.5: user fn must compile");
    let params = [2.0_f64];
    let v = [0.5_f64, 0.0_f64];
    let mut rhs = [0.0_f64; 2];
    dev.eval_residual(&v, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);
    assert!((rhs[0] - 1.0_f64).abs() < 1e-9_f64);
}

#[test]
fn d5_user_fn_missing_still_errors() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Bad ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
        analog Bad { I(p, n) <+ no_such_fn(V(p, n)); }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let result = ir_analog_to_device(&bodies, "Bad");
    let err = result.err().expect("D.5: missing fn must fail loudly");
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("no_such_fn") || msg.contains("unknown") || msg.contains("unsupported"));
}

#[test]
fn d5_spec_diode_with_user_fn_compiles() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        fn thermal_voltage(t: Real) -> Real { return 8.617e-5 * t; }
        mod Diode ( inout a : Electrical, inout c : Electrical ) {
            param is_sat : Real = 1.0e-14; param temp : Real = 300.0;
        }
        analog Diode { I(a, c) <+ is_sat * (exp(V(a, c) / thermal_voltage(temp)) - 1.0); }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("Diode model parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let dev = ir_analog_to_device(&bodies, "Diode").expect("Diode compiles");
    let params = [1.0e-14_f64, 300.0_f64];
    let v = [0.5_f64, 0.0_f64];
    let mut rhs = [0.0_f64; 2];
    dev.eval_residual(&v, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);
    let vt: f64 = 8.617e-5 * 300.0;
    let expected: f64 = 1.0e-14_f64 * ((0.5_f64 / vt).exp() - 1.0);
    assert!((rhs[0] - expected).abs() < 1e-9);
}

// ── D.2 — idt integration operator ─────────────────────────────────────────

#[test]
fn d2_idt_in_contribution_lowers_to_integrator() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Inductor ( inout p : Electrical, inout n : Electrical ) { param L : Real = 1.0e-6; }
        analog Inductor { I(p, n) <+ idt(V(p, n)) / L; }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses");
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
    let mut sim = SimCtx::default();
    sim.step = 1.0e-3;
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

// ── D.1 — voltage force V(p,n) <- expr ─────────────────────────────────────

#[test]
fn d1_voltage_force_compiles_with_force_residual() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod VSource ( inout p : Electrical, inout n : Electrical ) { param dc : Real = 1.5; }
        analog VSource { V(p, n) <- dc; }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("VSource parses");
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
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod OpAmp ( input inp : Electrical, input inn : Electrical, inout out : Electrical ) {
            param gain : Real = 1.0e6;
        }
        analog OpAmp { V(out) <- gain * V(inp, inn); }
    "#;
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("OpAmp parses");
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
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).map(|a| a.clone())
}
