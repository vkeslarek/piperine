//! Regression tests for Part A + D of `docs/GAPS.md`.
//! PHDL-specific tests (parse_and_elaborate, ppr_to_ir, from_ir).

use piperine_codegen::{ir_analog_to_device, IrProgram, SimCtx};
use piperine_lang::{from_ir, ir_digital_to_interp, parse_and_elaborate, ppr_to_ir};

// ── A.4 — Digital Pow/Shl/Shr silently become Add ─────────────────────────

#[test]
fn a4_shift_in_digital_guard_is_rejected_not_silently_add() {
    use piperine_codegen::ir::{IrBinOp, IrExpr, IrStmt, IrDigitalBody, IrModule, IrPort};

    let mut prog = IrProgram {
        source: "test".into(),
        modules: Vec::new(),
        functions: Vec::new(),
    };
    prog.modules.push(IrModule {
        name: "shift_fsm".into(),
        ports: Vec::new(),
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: Vec::new(),
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: None,
        digital: Some(IrDigitalBody {
            inputs: Vec::new(),
            outputs: Vec::new(),
            state_vars: Vec::new(),
            stmts: vec![IrStmt::If {
                cond: IrExpr::Binary(
                    IrBinOp::Shl,
                    Box::new(IrExpr::Param("x".into())),
                    Box::new(IrExpr::Int(4)),
                ),
                then_: vec![],
                else_: vec![],
                label: None,
            }],
        }),
        functions: Vec::new(),
    });
    let err = ir_digital_to_interp(&prog, "shift_fsm")
        .err()
        .expect("shift in digital guard must fail");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("shl") || msg.contains("shift") || msg.contains("unsupported"),
        "A.4: expected shift rejection, got: {msg}"
    );
}

// ── A.5 — BitNot / reductions silently become Not ──────────────────────────

fn make_digital_ir_with_unary_op(
    module_name: &str,
    op: piperine_codegen::ir::IrUnOp,
) -> IrProgram {
    use piperine_codegen::ir::{IrExpr, IrModule, IrPort, IrStmt, IrDigitalBody};
    let mut prog = IrProgram {
        source: "test".into(),
        modules: Vec::new(),
        functions: Vec::new(),
    };
    prog.modules.push(IrModule {
        name: module_name.into(),
        ports: Vec::new(),
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: Vec::new(),
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: None,
        digital: Some(IrDigitalBody {
            inputs: Vec::new(),
            outputs: Vec::new(),
            state_vars: Vec::new(),
            stmts: vec![IrStmt::Assign {
                lval: "out".into(),
                expr: IrExpr::Unary(op, Box::new(IrExpr::Param("x".into()))),
                delay: None,
                event: None,
            }],
        }),
        functions: Vec::new(),
    });
    prog
}

#[test]
fn a5_bitnot_in_digital_is_rejected_not_silently_not() {
    use piperine_codegen::ir::IrUnOp;
    let prog = make_digital_ir_with_unary_op("bitnot_fsm", IrUnOp::BitNot);
    let err = ir_digital_to_interp(&prog, "bitnot_fsm")
        .err()
        .expect("BitNot in digital must fail");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("bitnot") || msg.contains("unary") || msg.contains("unsupported"),
        "A.5: expected BitNot rejection, got: {msg}"
    );
}

#[test]
fn a5_reduction_op_in_digital_is_rejected_not_silently_not() {
    use piperine_codegen::ir::IrUnOp;
    let prog = make_digital_ir_with_unary_op("redand_fsm", IrUnOp::RedAnd);
    let err = ir_digital_to_interp(&prog, "redand_fsm")
        .err()
        .expect("RedAnd in digital must fail");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("red") || msg.contains("unary") || msg.contains("unsupported"),
        "A.5: expected RedAnd rejection, got: {msg}"
    );
}

#[test]
fn a5_neg_in_digital_still_works() {
    use piperine_codegen::ir::IrUnOp;
    let prog = make_digital_ir_with_unary_op("neg_fsm", IrUnOp::Neg);
    ir_digital_to_interp(&prog, "neg_fsm").expect("Neg in digital must still work");
}

// ── A.6 — from_ir propagates child compile errors ──────────────────────────

#[test]
fn a6_from_ir_propagates_child_compile_error_not_silent_skip() {
    use piperine_codegen::ir::{
        ContribKind, IrAnalogBody, IrConnection, IrDirection, IrExpr, IrInstance,
        IrModule, IrNature, IrPort, IrStmt,
    };

    let mut prog = IrProgram {
        source: "test".into(),
        modules: Vec::new(),
        functions: Vec::new(),
    };
    prog.modules.push(IrModule {
        name: "vsource".into(),
        ports: vec![
            IrPort { name: "p".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
            IrPort { name: "n".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
        ],
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: Vec::new(),
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: Some(IrAnalogBody {
            state_vars: Vec::new(),
            noise_sources: Vec::new(),
            vars: Vec::new(),
            stmts: vec![IrStmt::Contrib {
                nature: IrNature::Flow("I".into()),
                plus: "p".into(),
                minus: "n".into(),
                expr: IrExpr::BranchAccess {
                    access: "I".into(),
                    plus: "p".into(),
                    minus: "n".into(),
                },
                kind: ContribKind::Resistive,
            }],
        }),
        digital: None,
        functions: Vec::new(),
    });
    prog.modules.push(IrModule {
        name: "top".into(),
        ports: vec![
            IrPort { name: "a".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
            IrPort { name: "b".into(), direction: IrDirection::Inout, discipline: Some("Electrical".into()) },
        ],
        params: Vec::new(),
        wires: Vec::new(),
        branches: Vec::new(),
        events: Vec::new(),
        vars: Vec::new(),
        grounds: Vec::new(),
        instances: vec![IrInstance {
            label: "u1".into(),
            module: "vsource".into(),
            connections: vec![
                IrConnection { port: Some("p".into()), net: "a".into() },
                IrConnection { port: Some("n".into()), net: "b".into() },
            ],
            params: Vec::new(),
        }],
        connections: Vec::new(),
        continuous_assigns: Vec::new(),
        analog: None,
        digital: None,
        functions: Vec::new(),
    });
    let err = from_ir(&prog, "top").err().expect("top with bad child must fail");
    assert!(err.contains("u1"), "A.6: error should name instance u1, got: {err}");
    assert!(err.contains("vsource"), "A.6: error should name module vsource, got: {err}");
}

// ── A.7 — from_elab ddt rejection (now tests IR path handles ddt) ──────────

#[test]
fn a7_capacitor_compiles_through_ir_path() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Cap ( inout p : Electrical, inout n : Electrical ) { param c : Real = 1.0e-9; }
        analog Cap { I(p, n) <+ c * ddt(V(p, n)); }
    "#;
    let elab = parse_and_elaborate(src).expect("PHDL parses + elaborates");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "Cap").expect("capacitor compiles through IR path");
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
    let elab = parse_and_elaborate(src).expect("PHDL parses + elaborates");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "Resistor").expect("D.5: user-fn inlining must compile");
    let params = [1.0e3_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    let expected = 2.0 * 0.5 / 1.0e3;
    assert!((rhs[0] - expected).abs() < 1e-9);
}

#[test]
fn d5_user_fn_call_to_nonbuiltin_is_inlined_not_silently_zero() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        fn amp (x : Real, g : Real) -> Real { return g * x; }
        mod Gain ( inout p : Electrical, inout n : Electrical ) { param g : Real = 2.0; }
        analog Gain { I(p, n) <+ amp(V(p, n), g); }
    "#;
    let elab = parse_and_elaborate(src).expect("PHDL parses");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "Gain").expect("D.5: user fn must compile");
    let params = [2.0_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    assert!((rhs[0] - 1.0).abs() < 1e-9);
}

#[test]
fn d5_user_fn_missing_still_errors() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Bad ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
        analog Bad { I(p, n) <+ no_such_fn(V(p, n)); }
    "#;
    let elab = parse_and_elaborate(src).expect("PHDL parses");
    let ir = ppr_to_ir(&elab);
    let result = ir_analog_to_device(&ir, "Bad");
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
    let elab = parse_and_elaborate(src).expect("Diode model parses");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "Diode").expect("Diode compiles");
    let params = [1.0e-14_f64, 300.0_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    let vt: f64 = 8.617e-5 * 300.0;
    let expected: f64 = 1.0e-14_f64 * ((0.5_f64 / vt).exp() - 1.0);
    assert!((rhs[0] - expected).abs() < 1e-9);
}

// ── D.2 — idt integration operator ─────────────────────────────────────────

#[test]
fn d2_idt_in_contribution_compiles_with_reactive_support() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Inductor ( inout p : Electrical, inout n : Electrical ) { param L : Real = 1.0e-6; }
        analog Inductor { I(p, n) <+ idt(V(p, n)) / L; }
    "#;
    let elab = parse_and_elaborate(src).expect("PHDL parses");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "Inductor").expect("D.2: idt must compile");
    assert!(dev.has_reactive(), "D.2: idt must produce reactive contributions");
    let params = [1.0e-6_f64];
    let v = [0.5, 0.0];
    let mut rhs = [0.0; 2];
    dev.eval_residual(&v, &params, &SimCtx::default(), &mut rhs);
    assert!(rhs[0].abs() < 1e-12, "D.2: DC residual near 0, got {}", rhs[0]);
    let mut q = [0.0; 2];
    dev.eval_charge(&v, &params, &SimCtx::default(), &mut q);
    let expected = 0.5 / 1.0e-6;
    assert!((q[0] - expected).abs() < 1e-3 * expected, "D.2: charge Q = V/L");
}

// ── D.1 — voltage force V(p,n) <- expr ─────────────────────────────────────

#[test]
fn d1_voltage_force_compiles_with_force_residual() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod VSource ( inout p : Electrical, inout n : Electrical ) { param dc : Real = 1.5; }
        analog VSource { V(p, n) <- dc; }
    "#;
    let elab = parse_and_elaborate(src).expect("VSource parses");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "VSource").expect("D.1: VSource compiles");
    assert!(dev.has_force(), "D.1: must have force function");
    let params = [1.5_f64];
    let v = [1.2, 0.4];
    let mut rhs = [0.0; 1];
    dev.eval_force(&v, &params, &SimCtx::default(), &mut rhs);
    assert!((rhs[0] - (-0.7)).abs() < 1e-12, "D.1: force residual = {}", rhs[0]);
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
    let elab = parse_and_elaborate(src).expect("OpAmp parses");
    let ir = ppr_to_ir(&elab);
    let dev = ir_analog_to_device(&ir, "OpAmp").expect("D.1: OpAmp compiles");
    assert!(dev.has_force(), "D.1: OpAmp must have force function");
}