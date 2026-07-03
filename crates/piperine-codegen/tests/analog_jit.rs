//! Analog JIT end-to-end tests: hand-built IR modules compiled to kernels,
//! checked numerically and through full solver analyses.

use piperine_codegen::ir::*;
use piperine_codegen::jit::analog::AnalogKernel;
use piperine_codegen::{CircuitCompiler, CodegenError, SimCtx};
use piperine_solver::solver::Context;

// ─── IR construction helpers ──────────────────────────────────────────────────

/// A two-terminal module scaffold: ports `p`, `n`, natures `V`/`I`.
struct TwoTerminal {
    module: IrModule,
    p: NodeId,
    n: NodeId,
    v: NatureId,
    i: NatureId,
}

impl TwoTerminal {
    fn new(name: &str) -> Self {
        let mut module = IrModule::new(name);
        let p = module.symbols.add_node("p", Domain::Analog);
        let n = module.symbols.add_node("n", Domain::Analog);
        let v = module.symbols.add_nature("V", NatureKind::Potential);
        let i = module.symbols.add_nature("I", NatureKind::Flow);
        module.ports.push(IrPort { node: p, direction: IrDirection::Inout });
        module.ports.push(IrPort { node: n, direction: IrDirection::Inout });
        Self { module, p, n, v, i }
    }

    fn v_pn(&self) -> IrExpr {
        IrExpr::Branch { nature: self.v, plus: self.p, minus: self.n }
    }

    /// `I(p,n) <+ expr` as the whole analog body.
    fn with_flow_contrib(mut self, expr: IrExpr, kind: ContribKind) -> IrModule {
        self.module.analog = Some(IrAnalogBody {
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

fn bin(op: IrBinOp, a: IrExpr, b: IrExpr) -> IrExpr {
    IrExpr::binary(op, a, b)
}

/// `resistor(r = 1k)`: `I(p,n) <+ V(p,n) / r`.
fn resistor() -> IrModule {
    let mut t = TwoTerminal::new("resistor");
    let r = t.module.symbols.add_param("r", IrType::Real, Some(real(1000.0)));
    let expr = bin(IrBinOp::Div, t.v_pn(), param(r));
    t.with_flow_contrib(expr, ContribKind::Resistive)
}

/// `isource(dc = 1mA)`: `I(p,n) <+ -dc` (drives current from n to p).
fn isource() -> IrModule {
    let mut t = TwoTerminal::new("isource");
    let dc = t.module.symbols.add_param("dc", IrType::Real, Some(real(1e-3)));
    let expr = IrExpr::Unary(IrUnOp::Neg, Box::new(param(dc)));
    t.with_flow_contrib(expr, ContribKind::Resistive)
}

/// `vsource(dc = 1V)`: `V(p,n) <- dc`.
fn vsource() -> IrModule {
    let mut t = TwoTerminal::new("vsource");
    let dc = t.module.symbols.add_param("dc", IrType::Real, Some(real(1.0)));
    t.module.analog = Some(IrAnalogBody {
        states: Vec::new(),
        noise: Vec::new(),
        stmts: vec![IrStmt::Force { nature: t.v, plus: t.p, minus: t.n, expr: param(dc) }],
    });
    t.module
}

/// `diode(is = 1e-14)`: `I(p,n) <+ is * (exp(V/vt) - 1)` with vt from a var.
fn diode() -> IrModule {
    let mut t = TwoTerminal::new("diode");
    let is = t.module.symbols.add_param("is", IrType::Real, Some(real(1e-14)));
    let vt = t.module.symbols.add_var("vt", IrType::Real);
    let body = IrAnalogBody {
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
                    IrBinOp::Mul,
                    param(is),
                    bin(
                        IrBinOp::Sub,
                        IrExpr::MathCall(
                            "exp".into(),
                            vec![bin(IrBinOp::Div, t.v_pn(), IrExpr::Var(vt))],
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
fn capacitor() -> IrModule {
    let mut t = TwoTerminal::new("capacitor");
    let c = t.module.symbols.add_param("c", IrType::Real, Some(real(1e-6)));
    let arg = bin(IrBinOp::Mul, param(c), t.v_pn());
    let ddt = t.module.symbols.add_state(IrStateVar { kind: IrStateKind::Ddt, arg });
    t.module.analog = Some(IrAnalogBody {
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

/// A structural top: `modules` instanced once each with the given
/// connections into shared top nodes.
#[allow(dead_code)]
fn top_with(
    instances: Vec<(&str, Vec<NodeId>, Vec<(ParamId, IrExpr)>)>,
    num_nodes: u32,
) -> IrModule {
    let mut top = IrModule::new("top");
    for i in 0..num_nodes {
        top.symbols.add_node(format!("n{i}"), Domain::Analog);
    }
    for (index, (module, connections, params)) in instances.into_iter().enumerate() {
        top.instances.push(IrInstance {
            label: format!("x{index}"),
            module: module.to_string(),
            connections,
            params,
        });
    }
    top
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
    let r = t.module.symbols.add_param("r", IrType::Real, Some(real(100.0)));
    let cond = bin(IrBinOp::Gt, t.v_pn(), real(1.0));
    let contrib = IrStmt::Contrib {
        nature: t.i,
        plus: t.p,
        minus: t.n,
        expr: bin(IrBinOp::Div, t.v_pn(), param(r)),
        kind: ContribKind::Resistive,
    };
    t.module.analog = Some(IrAnalogBody {
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
    let x = t.module.symbols.add_var("x", IrType::Real);
    let double = t.module.symbols.add_fn(IrFunction {
        name: "double".into(),
        params: vec![x],
        returns: Some(IrType::Real),
        body: vec![IrStmt::Return(Some(bin(IrBinOp::Mul, IrExpr::Var(x), real(2.0))))],
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
    let state = t.module.symbols.add_state(IrStateVar {
        kind: IrStateKind::Transition {
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
        findings.iter().any(|d| d.kind == IrDiagnosticKind::Error),
        "resistive-marked reactive contribution must fail validation"
    );
}

// ─── Circuit-level tests through the solver ───────────────────────────────────

fn program(modules: Vec<IrModule>) -> IrProgram {
    let mut p = IrProgram::new(Source::Ppr);
    p.modules = modules;
    p
}

#[test]
fn dc_current_source_into_resistor() {
    // isource (1 mA) feeding a 1 kΩ resistor to ground → V(node) = 1 V.
    let mut top = IrModule::new("top");
    let node = top.symbols.add_node("out", Domain::Analog);
    top.instances.push(IrInstance {
        label: "i1".into(),
        module: "isource".into(),
        connections: vec![node, NodeId::GROUND],
        params: vec![],
    });
    top.instances.push(IrInstance {
        label: "r1".into(),
        module: "resistor".into(),
        connections: vec![node, NodeId::GROUND],
        params: vec![],
    });

    let program = program(vec![isource(), resistor(), top]);
    let mut compiler = CircuitCompiler::new(&program);
    let mut circuit = compiler.build_circuit("top").expect("build circuit");
    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let voltage = node_voltage(&circuit, 1);
    let v = result.get(voltage).expect("node voltage");
    assert!((v - 1.0).abs() < 1e-9, "V(out) = {v}");
}

#[test]
fn dc_voltage_divider_with_force_source() {
    // vsource 5 V across two series 1 kΩ resistors → middle at 2.5 V.
    let mut top = IrModule::new("top");
    let vin = top.symbols.add_node("vin", Domain::Analog);
    let mid = top.symbols.add_node("mid", Domain::Analog);
    let mut push = |label: &str, module: &str, conns: Vec<NodeId>, params: Vec<(ParamId, IrExpr)>| {
        top.instances.push(IrInstance {
            label: label.into(),
            module: module.into(),
            connections: conns,
            params,
        });
    };
    push("v1", "vsource", vec![vin, NodeId::GROUND], vec![(ParamId(0), real(5.0))]);
    push("r1", "resistor", vec![vin, mid], vec![]);
    push("r2", "resistor", vec![mid, NodeId::GROUND], vec![]);

    let program = program(vec![vsource(), resistor(), top]);
    let mut compiler = CircuitCompiler::new(&program);
    let mut circuit = compiler.build_circuit("top").expect("build circuit");
    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_in = result.get(node_voltage(&circuit, 1)).expect("vin");
    let v_mid = result.get(node_voltage(&circuit, 2)).expect("mid");
    assert!((v_in - 5.0).abs() < 1e-9, "V(vin) = {v_in}");
    assert!((v_mid - 2.5).abs() < 1e-9, "V(mid) = {v_mid}");
}

#[test]
fn dc_diode_resistor_operating_point() {
    // 5 V source through 1 kΩ into a diode: V_d ≈ 0.65–0.75 V and KCL holds.
    let mut top = IrModule::new("top");
    let vin = top.symbols.add_node("vin", Domain::Analog);
    let vd = top.symbols.add_node("vd", Domain::Analog);
    top.instances.push(IrInstance {
        label: "v1".into(),
        module: "vsource".into(),
        connections: vec![vin, NodeId::GROUND],
        params: vec![(ParamId(0), real(5.0))],
    });
    top.instances.push(IrInstance {
        label: "r1".into(),
        module: "resistor".into(),
        connections: vec![vin, vd],
        params: vec![],
    });
    top.instances.push(IrInstance {
        label: "d1".into(),
        module: "diode".into(),
        connections: vec![vd, NodeId::GROUND],
        params: vec![],
    });

    let program = program(vec![vsource(), resistor(), diode(), top]);
    let mut compiler = CircuitCompiler::new(&program);
    let mut circuit = compiler.build_circuit("top").expect("build circuit");
    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_d = result.get(node_voltage(&circuit, 2)).expect("vd");
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
    let mut top = IrModule::new("top");
    let vin = top.symbols.add_node("vin", Domain::Analog);
    let out = top.symbols.add_node("out", Domain::Analog);
    top.instances.push(IrInstance {
        label: "v1".into(),
        module: "vsource".into(),
        connections: vec![vin, NodeId::GROUND],
        params: vec![(ParamId(0), real(5.0))],
    });
    top.instances.push(IrInstance {
        label: "r1".into(),
        module: "resistor".into(),
        connections: vec![vin, out],
        params: vec![],
    });
    top.instances.push(IrInstance {
        label: "c1".into(),
        module: "capacitor".into(),
        connections: vec![out, NodeId::GROUND],
        params: vec![],
    });

    let program = program(vec![vsource(), resistor(), capacitor(), top]);
    let mut compiler = CircuitCompiler::new(&program);
    let mut circuit = compiler.build_circuit("top").expect("build circuit");

    let options = TransientAnalysisOptions::new(5e-3.into(), 1e-5.into());
    let result = circuit
        .transient(options, Context::default())
        .unwrap()
        .solve()
        .unwrap();

    // After 5 τ the capacitor is essentially charged.
    let final_v = result
        .last()
        .and_then(|step| {
            step.get_node(&piperine_solver::analog::NodeIdentifier::Anonymous(2))
        })
        .expect("final out voltage");
    assert!(
        (final_v - 5.0).abs() < 0.05,
        "V(out) after 5τ = {final_v}"
    );
}

/// The `AnalogVariable` for a top-level node id (as allocated by the
/// circuit compiler: `Anonymous(node_id)`).
fn node_voltage(
    _circuit: &piperine_solver::circuit::CircuitInstance,
    node_id: usize,
) -> piperine_solver::analog::AnalogVariable {
    piperine_solver::analog::AnalogVariable::Node(
        piperine_solver::analog::NodeIdentifier::Anonymous(node_id),
    )
}
