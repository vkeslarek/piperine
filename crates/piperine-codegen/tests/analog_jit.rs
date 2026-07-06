//! Analog JIT end-to-end tests: hand-built IR modules compiled to kernels,
//! checked numerically and through full solver analyses.

use piperine_codegen::ir::*;
use piperine_codegen::jit::analog::AnalogKernel;
use piperine_codegen::{CircuitCompiler, CodegenError, SimCtx};
use piperine_solver::solver::Context;

// ─── IR construction helpers ──────────────────────────────────────────────────

/// A two-terminal module scaffold: ports `p`, `n`, natures `V`/`I`.
struct TwoTerminal {
    module: LoweredBody,
    p: NodeId,
    n: NodeId,
    v: NatureId,
    i: NatureId,
}

impl TwoTerminal {
    fn new(name: &str) -> Self {
        let mut module = LoweredBody::new(name);
        let p = module.symbols.add_node("p", Domain::Analog);
        let n = module.symbols.add_node("n", Domain::Analog);
        let v = module.symbols.add_nature("V", NatureKind::Potential);
        let i = module.symbols.add_nature("I", NatureKind::Flow);
        module.ports.push(Port { node: p, direction: Direction::Inout });
        module.ports.push(Port { node: n, direction: Direction::Inout });
        Self { module, p, n, v, i }
    }

    fn v_pn(&self) -> IrExpr {
        IrExpr::Branch { nature: self.v, plus: self.p, minus: self.n }
    }

    /// `I(p,n) <+ expr` as the whole analog body.
    fn with_flow_contrib(mut self, expr: IrExpr, kind: ContribKind) -> LoweredBody {
        self.module.analog = Some(AnalogBody {
            states: Vec::new(),
            noise: Vec::new(),
            stmts: vec![IrStmt::Contrib { nature: self.i, plus: self.p, minus: self.n, expr, kind }],
        });
        self.module
    }
}

fn real(v: f64) -> IrExpr {
    IrExpr::Real(v)
}

fn param(id: ParamId) -> IrExpr {
    IrExpr::Param(id)
}

fn bin(op: BinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::binary(op, a, b)
}

/// `resistor(r = 1k)`: `I(p,n) <+ V(p,n) / r`.
fn resistor() -> LoweredBody {
    let mut t = TwoTerminal::new("resistor");
    let r = t.module.symbols.add_param("r", Type::Real, Some(real(1000.0)));
    let expr = bin(BinOp::Div, t.v_pn(), param(r));
    t.with_flow_contrib(expr, ContribKind::Resistive)
}

/// `diode(is = 1e-14)`: `I(p,n) <+ is * (exp(V/vt) - 1)` with vt from a var.
fn diode() -> LoweredBody {
    let mut t = TwoTerminal::new("diode");
    let is = t.module.symbols.add_param("is", Type::Real, Some(real(1e-14)));
    let vt = t.module.symbols.add_var("vt", Type::Real);
    let body = AnalogBody {
        states: Vec::new(),
        noise: Vec::new(),
        stmts: vec![
            IrStmt::Assign {
                lval: Lval::Var(vt),
                expr: IrExpr::Sim(SimQuery::Vt(None)),
            },
            IrStmt::Contrib {
                nature: t.i,
                plus: t.p,
                minus: t.n,
                expr: bin(
                    BinOp::Mul,
                    param(is),
                    bin(
                        BinOp::Sub,
                        IrExpr::MathCall(
                            "exp".into(),
                            vec![bin(BinOp::Div, t.v_pn(), IrExpr::Var(vt))],
                        ),
                        real(1.0),
                    ),
                ),
                kind: ContribKind::Resistive,
            },
        ],
    };
    t.module.analog = Some(body);
    t.module
}

/// `capacitor(c = 1u)`: `I(p,n) <+ ddt(c * V(p,n))`.
fn capacitor() -> LoweredBody {
    let mut t = TwoTerminal::new("capacitor");
    let c = t.module.symbols.add_param("c", Type::Real, Some(real(1e-6)));
    let arg = bin(BinOp::Mul, param(c), t.v_pn());
    let ddt = t.module.symbols.add_state(StateVar { kind: StateKind::Ddt, arg });
    t.module.analog = Some(AnalogBody {
        states: vec![ddt],
        noise: Vec::new(),
        stmts: vec![IrStmt::Contrib {
            nature: t.i,
            plus: t.p,
            minus: t.n,
            expr: IrExpr::State(ddt),
            kind: ContribKind::Reactive(ddt),
        }],
    });
    t.module
}

// ─── Kernel-level numeric tests ───────────────────────────────────────────────

#[test]
fn resistor_residual_and_jacobian_match_ohms_law() {
    let module = resistor();
    let kernel = AnalogKernel::compile(&module).expect("compile resistor");
    assert_eq!(kernel.num_terminals(), 2);

    let volts = [2.0, 0.5];
    let params = [1000.0];
    let sim = SimCtx::default();
    let mut res = [0.0; 2];
    kernel.eval_residual(&volts, &params, &[], &[], &sim, &mut res);
    let i = (2.0 - 0.5) / 1000.0;
    assert!((res[0] - i).abs() < 1e-15, "res[0] = {}", res[0]);
    assert!((res[1] + i).abs() < 1e-15, "res[1] = {}", res[1]);

    let mut jac = [0.0; 4];
    kernel.eval_jacobian(&volts, &params, &[], &[], &sim, &mut jac);
    let g = 1.0 / 1000.0;
    assert!((jac[0] - g).abs() < 1e-15);
    assert!((jac[1] + g).abs() < 1e-15);
    assert!((jac[2] + g).abs() < 1e-15);
    assert!((jac[3] - g).abs() < 1e-15);
}

#[test]
fn diode_uses_thermal_voltage_from_sim_ctx() {
    let module = diode();
    let kernel = AnalogKernel::compile(&module).expect("compile diode");

    let volts = [0.6, 0.0];
    let params = [1e-14];
    let sim = SimCtx::at_temperature(300.0);
    let vt = 300.0 * SimCtx::K_B_OVER_Q;

    let mut res = [0.0; 2];
    kernel.eval_residual(&volts, &params, &[], &[], &sim, &mut res);
    let expected = 1e-14 * ((0.6 / vt).exp() - 1.0);
    assert!(
        (res[0] - expected).abs() < expected.abs() * 1e-12,
        "diode current {} vs {}",
        res[0],
        expected
    );

    // dI/dV = is/vt * exp(V/vt)
    let mut jac = [0.0; 4];
    kernel.eval_jacobian(&volts, &params, &[], &[], &sim, &mut jac);
    let g = 1e-14 / vt * (0.6 / vt).exp();
    assert!((jac[0] - g).abs() < g * 1e-12, "g = {} vs {}", jac[0], g);
}

#[test]
fn capacitor_charge_and_charge_jacobian() {
    let module = capacitor();
    let kernel = AnalogKernel::compile(&module).expect("compile capacitor");
    assert!(kernel.has_reactive());

    let volts = [3.0, 1.0];
    let params = [1e-6];
    let sim = SimCtx::default();

    // Q = C·V, resistive residual must be zero.
    let mut res = [0.0; 2];
    kernel.eval_residual(&volts, &params, &[], &[], &sim, &mut res);
    assert_eq!(res, [0.0; 2]);

    let mut q = [0.0; 2];
    kernel.eval_charge(&volts, &params, &[], &[], &sim, &mut q);
    let expected = 1e-6 * 2.0;
    assert!((q[0] - expected).abs() < 1e-18);
    assert!((q[1] + expected).abs() < 1e-18);

    let mut qjac = [0.0; 4];
    kernel.eval_charge_jacobian(&volts, &params, &[], &[], &sim, &mut qjac);
    assert!((qjac[0] - 1e-6).abs() < 1e-18);
    assert!((qjac[3] - 1e-6).abs() < 1e-18);
}

#[test]
fn guarded_contribution_folds_into_select() {
    // I(p,n) <+ if (V > 1) V/r else 0 — via an If statement.
    let mut t = TwoTerminal::new("clipper");
    let r = t.module.symbols.add_param("r", Type::Real, Some(real(100.0)));
    let cond = bin(BinOp::Gt, t.v_pn(), real(1.0));
    let contrib = IrStmt::Contrib {
        nature: t.i,
        plus: t.p,
        minus: t.n,
        expr: bin(BinOp::Div, t.v_pn(), param(r)),
        kind: ContribKind::Resistive,
    };
    t.module.analog = Some(AnalogBody {
        states: Vec::new(),
        noise: Vec::new(),
        stmts: vec![IrStmt::If { cond, then_: vec![contrib], else_: vec![] }],
    });
    let kernel = AnalogKernel::compile(&t.module).expect("compile clipper");

    let params = [100.0];
    let sim = SimCtx::default();
    let mut res = [0.0; 2];
    kernel.eval_residual(&[2.0, 0.0], &params, &[], &[], &sim, &mut res);
    assert!((res[0] - 0.02).abs() < 1e-15, "above threshold conducts");

    res = [0.0; 2];
    kernel.eval_residual(&[0.5, 0.0], &params, &[], &[], &sim, &mut res);
    assert_eq!(res, [0.0; 2], "below threshold is off");
}

#[test]
fn user_function_is_inlined() {
    // fn double(x) = x * 2; I(p,n) <+ double(V(p,n))
    let mut t = TwoTerminal::new("doubler");
    let x = t.module.symbols.add_var("x", Type::Real);
    let double = t.module.symbols.add_fn(Function {
        name: "double".into(),
        params: vec![x],
        defaults: vec![None],
        returns: Some(Type::Real),
        body: vec![IrStmt::Return(Some(bin(BinOp::Mul, IrExpr::Var(x), real(2.0))))],
    });
    let expr = IrExpr::Call(double, vec![t.v_pn()]);
    let module = t.with_flow_contrib(expr, ContribKind::Resistive);
    let kernel = AnalogKernel::compile(&module).expect("compile doubler");

    let mut res = [0.0; 2];
    kernel.eval_residual(&[1.5, 0.0], &[], &[], &[], &SimCtx::default(), &mut res);
    assert!((res[0] - 3.0).abs() < 1e-15);
}

#[test]
fn unsupported_operator_fails_loud_with_name() {
    // transition() has no lowering yet — must be a named error.
    let mut t = TwoTerminal::new("bad");
    let state = t.module.symbols.add_state(StateVar {
        kind: StateKind::Transition {
            delay: real(0.0),
            rise: real(1e-9),
            fall: real(1e-9),
            tol: real(1e-12),
        },
        arg: real(1.0),
    });
    let module = t.with_flow_contrib(IrExpr::State(state), ContribKind::Resistive);
    let Err(err) = AnalogKernel::compile(&module) else {
        panic!("transition must not compile");
    };
    match err {
        CodegenError::Unsupported(msg) => assert!(msg.contains("transition"), "{msg}"),
        other => panic!("expected Unsupported, got {other}"),
    }
}

#[test]
fn validation_rejects_mismatched_contrib_kind() {
    let module = capacitor();
    // Corrupt the kind: mark the reactive contribution resistive.
    let mut corrupted = module.clone();
    if let Some(body) = &mut corrupted.analog {
        if let IrStmt::Contrib { kind, .. } = &mut body.stmts[0] {
            *kind = ContribKind::Resistive;
        }
    }
    let findings = corrupted.validate();
    assert!(
        findings.iter().any(|d| d.kind == DiagnosticKind::Error),
        "resistive-marked reactive contribution must fail validation"
    );
}

// ─── Circuit-level tests through the solver ───────────────────────────────────

// These circuit-level tests exercise the full pipeline (parse → elaborate →
// lower_bodies → CircuitCompiler) instead of hand-built resolved bodies —
// there is no way to hand-build a multi-module POM `Design` from outside
// `piperine-lang` (its structural fields are crate-private by design), and
// a tiny PHDL snippet is a more faithful fixture for "does the compiler
// wire instances correctly" than a hand-rolled structural twin ever was.
const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }\n";
const ISOURCE: &str = "mod Isource(inout p : Electrical, inout n : Electrical) { param dc : Real = 1e-3; }\nanalog Isource { I(p, n) <+ -dc; }\n";
const VSOURCE: &str = "mod Vsource(inout p : Electrical, inout n : Electrical) { param dc : Real = 1.0; }\nanalog Vsource { V(p, n) <- dc; }\n";
const RESISTOR: &str = "mod Resistor(inout p : Electrical, inout n : Electrical) { param r : Real = 1e3; }\nanalog Resistor { I(p, n) <+ V(p, n) / r; }\n";
const DIODE: &str = "mod Diode(inout p : Electrical, inout n : Electrical) { param is : Real = 1e-14; }\nanalog Diode { I(p, n) <+ is * (exp(V(p, n) / $vt) - 1.0); }\n";
const CAPACITOR: &str = "mod Capacitor(inout p : Electrical, inout n : Electrical) { param c : Real = 1e-6; }\nanalog Capacitor { I(p, n) <+ ddt(c * V(p, n)); }\n";

/// Elaborate `body` (module defs, no top) with a `Top` module wired from
/// `wires`/`instances`, then build the circuit.
fn build_top(
    body: &str,
    top_src: &str,
) -> (piperine_solver::core::circuit::CircuitInstance, piperine_codegen::CircuitBuildInfo) {
    let src = format!("{DISCIPLINE}{body}\nmod Top() {{\n{top_src}\n}}\n");
    let design = piperine_lang::parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .expect("parse_and_elaborate");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("lower_bodies");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit_mapped("Top").expect("build circuit")
}

fn voltage(
    result: &piperine_solver::analysis::dc::DcAnalysisResult,
    info: &piperine_codegen::CircuitBuildInfo,
    net: &str,
) -> f64 {
    let node = info.nets.get(net).unwrap_or_else(|| panic!("no net `{net}`"));
    result
        .get(piperine_solver::analog::AnalogVariable::Node(node.clone()))
        .unwrap_or_else(|| panic!("no voltage for `{net}`"))
}

#[test]
fn dc_current_source_into_resistor() {
    // isource (1 mA) feeding a 1 kΩ resistor to ground → V(node) = 1 V.
    let body = format!("{ISOURCE}{RESISTOR}");
    let top = "wire out : Electrical;\ni1 : Isource(.p = out, .n = gnd);\nr1 : Resistor(.p = out, .n = gnd);";
    let (mut circuit, info) = build_top(&body, top);
    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v = voltage(&result, &info, "out");
    assert!((v - 1.0).abs() < 1e-9, "V(out) = {v}");
}

#[test]
fn dc_voltage_divider_with_force_source() {
    // vsource 5 V across two series 1 kΩ resistors → middle at 2.5 V.
    let body = format!("{VSOURCE}{RESISTOR}");
    let top = "wire vin : Electrical;\nwire mid : Electrical;\nv1 : Vsource(.p = vin, .n = gnd) { .dc = 5.0 };\nr1 : Resistor(.p = vin, .n = mid);\nr2 : Resistor(.p = mid, .n = gnd);";
    let (mut circuit, info) = build_top(&body, top);
    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_in = voltage(&result, &info, "vin");
    let v_mid = voltage(&result, &info, "mid");
    assert!((v_in - 5.0).abs() < 1e-9, "V(vin) = {v_in}");
    assert!((v_mid - 2.5).abs() < 1e-9, "V(mid) = {v_mid}");
}

#[test]
fn dc_diode_resistor_operating_point() {
    // 5 V source through 1 kΩ into a diode: V_d ≈ 0.65–0.75 V and KCL holds.
    let body = format!("{VSOURCE}{RESISTOR}{DIODE}");
    let top = "wire vin : Electrical;\nwire vd : Electrical;\nv1 : Vsource(.p = vin, .n = gnd) { .dc = 5.0 };\nr1 : Resistor(.p = vin, .n = vd);\nd1 : Diode(.p = vd, .n = gnd);";
    let (mut circuit, info) = build_top(&body, top);
    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_d = voltage(&result, &info, "vd");
    assert!(v_d > 0.5 && v_d < 0.9, "diode drop {v_d}");
    // KCL: resistor current equals diode current at the operating point.
    let context = Context::default();
    let vt = context.temperature * SimCtx::K_B_OVER_Q;
    let i_r = (5.0 - v_d) / 1000.0;
    let i_d = 1e-14 * ((v_d / vt).exp() - 1.0);
    // Match to the solver's Newton tolerance (vntol on an exponential device).
    assert!(
        (i_r - i_d).abs() < i_r * 1e-3,
        "KCL: resistor {i_r} vs diode {i_d}"
    );
}

#[test]
fn transient_rc_charges_toward_source() {
    use piperine_solver::analysis::transient::TransientAnalysisOptions;

    // 5 V step into R = 1 kΩ, C = 1 µF (τ = 1 ms), simulate 5 ms.
    let body = format!("{VSOURCE}{RESISTOR}{CAPACITOR}");
    let top = "wire vin : Electrical;\nwire out : Electrical;\nv1 : Vsource(.p = vin, .n = gnd) { .dc = 5.0 };\nr1 : Resistor(.p = vin, .n = out);\nc1 : Capacitor(.p = out, .n = gnd);";
    let (mut circuit, info) = build_top(&body, top);

    let options = TransientAnalysisOptions::new(5e-3.into(), 1e-5.into());
    let result = circuit
        .transient(options, Context::default())
        .unwrap()
        .solve()
        .unwrap();

    // After 5 τ the capacitor is essentially charged.
    let out_node = info.nets.get("out").expect("out net");
    let final_v = result
        .last()
        .and_then(|step| step.get_node(out_node))
        .expect("final out voltage");
    assert!(
        (final_v - 5.0).abs() < 0.05,
        "V(out) after 5τ = {final_v}"
    );
}
