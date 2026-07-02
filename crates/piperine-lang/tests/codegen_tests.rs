//! Integration tests for Cranelift JIT codegen of PHDL analog modules.


use piperine_codegen::SimCtx;
use piperine_lang::{parse_and_elaborate, ppr_to_ir};
use piperine_codegen::CircuitCompiler;

// ── Resistor ──────────────────────────────────────────────────────────────────

const RESISTOR_SRC: &str = "
discipline Electrical {
    potential v : Real;
    flow      i : Real;
}

mod Resistor ( inout p : Electrical, inout n : Electrical ) {
    param R : Real = 1000.0;
}

analog Resistor {
    I(p, n) <+ V(p, n) / R;
}
";

#[test]
fn test_compile_resistor() {
    let prog = parse_and_elaborate(RESISTOR_SRC).expect("elab");
    let dev  = ir_analog_to_device(&ppr_to_ir(&prog), "Resistor").expect("codegen");

    assert_eq!(dev.num_terminals(), 2);
    assert_eq!(dev.num_params(),    1);
    assert_eq!(dev.num_params(), 1);
}

#[test]
fn test_resistor_residual_ohms_law() {
    let prog = parse_and_elaborate(RESISTOR_SRC).expect("elab");
    let dev  = ir_analog_to_device(&ppr_to_ir(&prog), "Resistor").expect("codegen");

    // V(p) = 1.0 V, V(n) = 0.0 V → V(p,n) = 1.0 V
    // R = 1000 Ω → I = 1.0 / 1000 = 0.001 A
    let node_voltages = [1.0_f64, 0.0];
    let params        = [1000.0_f64];
    let mut rhs       = [0.0_f64; 2];

    dev.eval_residual(&node_voltages, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);

    // KCL: I flows into p (+) and out of n (-)
    assert!((rhs[0] -  0.001).abs() < 1e-12, "rhs[p] = {}", rhs[0]);
    assert!((rhs[1] - -0.001).abs() < 1e-12, "rhs[n] = {}", rhs[1]);
}

#[test]
fn test_resistor_jacobian_conductance() {
    let prog = parse_and_elaborate(RESISTOR_SRC).expect("elab");
    let dev  = ir_analog_to_device(&ppr_to_ir(&prog), "Resistor").expect("codegen");

    // G = 1/R = 0.001 S
    // J = [[ G, -G], [-G,  G]]
    let node_voltages = [1.0_f64, 0.0];
    let params        = [1000.0_f64];
    let mut jac       = [0.0_f64; 4]; // 2×2

    dev.eval_jacobian(&node_voltages, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut jac);

    let g = 0.001;
    // jac is row-major: [J[p,p], J[p,n], J[n,p], J[n,n]]
    assert!((jac[0] -  g).abs() < 1e-12, "J[p,p] = {}", jac[0]);
    assert!((jac[1] - -g).abs() < 1e-12, "J[p,n] = {}", jac[1]);
    assert!((jac[2] - -g).abs() < 1e-12, "J[n,p] = {}", jac[2]);
    assert!((jac[3] -  g).abs() < 1e-12, "J[n,n] = {}", jac[3]);
}

// ── Voltage-controlled current source ─────────────────────────────────────────

const VCCS_SRC: &str = "
discipline Electrical {
    potential v : Real;
    flow      i : Real;
}

mod Vccs ( inout inp : Electrical, inout inn : Electrical,
           inout outp : Electrical, inout outn : Electrical ) {
    param gm : Real = 0.01;
}

analog Vccs {
    I(outp, outn) <+ gm * V(inp, inn);
}
";

#[test]
fn test_vccs_residual() {
    let prog = parse_and_elaborate(VCCS_SRC).expect("elab");
    let dev  = ir_analog_to_device(&ppr_to_ir(&prog), "Vccs").expect("codegen");

    // V(inp,inn)=0.5 V, gm=0.01 → I=0.005 A
    let node_voltages = [0.5_f64, 0.0, 0.0, 0.0]; // inp=0.5, inn=0, outp=0, outn=0
    let params        = [0.01_f64];
    let mut rhs       = [0.0_f64; 4];

    dev.eval_residual(&node_voltages, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);

    assert!((rhs[2] -  0.005).abs() < 1e-12, "rhs[outp] = {}", rhs[2]);
    assert!((rhs[3] - -0.005).abs() < 1e-12, "rhs[outn] = {}", rhs[3]);
    // Input ports: no current contribution
    assert!(rhs[0].abs() < 1e-12);
    assert!(rhs[1].abs() < 1e-12);
}

#[test]
fn test_vccs_jacobian_transconductance() {
    let prog = parse_and_elaborate(VCCS_SRC).expect("elab");
    let dev  = ir_analog_to_device(&ppr_to_ir(&prog), "Vccs").expect("codegen");

    let node_voltages = [0.5_f64, 0.0, 0.0, 0.0];
    let params        = [0.01_f64];
    let mut jac       = [0.0_f64; 16]; // 4×4

    dev.eval_jacobian(&node_voltages, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut jac);

    let gm = 0.01_f64;
    let n  = 4usize;

    // d(I_outp)/d(V_inp) = +gm, d(I_outp)/d(V_inn) = -gm
    assert!((jac[2*n + 0] -  gm).abs() < 1e-12, "J[outp,inp] = {}", jac[2*n+0]);
    assert!((jac[2*n + 1] - -gm).abs() < 1e-12, "J[outp,inn] = {}", jac[2*n+1]);
    // d(I_outn)/d(V_inp) = -gm, d(I_outn)/d(V_inn) = +gm
    assert!((jac[3*n + 0] - -gm).abs() < 1e-12, "J[outn,inp] = {}", jac[3*n+0]);
    assert!((jac[3*n + 1] -  gm).abs() < 1e-12, "J[outn,inn] = {}", jac[3*n+1]);
}

// ── Diode (nonlinear) ─────────────────────────────────────────────────────────

const DIODE_SRC: &str = "
discipline Electrical {
    potential v : Real;
    flow      i : Real;
}

mod Diode ( inout p : Electrical, inout n : Electrical ) {
    param Is  : Real = 1.0e-14;
    param vt  : Real = 0.02585;
}

analog Diode {
    I(p, n) <+ Is * (exp(V(p, n) / vt) - 1.0);
}
";

#[test]
fn test_diode_residual_forward_bias() {
    let prog = parse_and_elaborate(DIODE_SRC).expect("elab");
    let dev  = ir_analog_to_device(&ppr_to_ir(&prog), "Diode").expect("codegen");

    let vd   = 0.5_f64;
    let is_  = 1.0e-14_f64;
    let vt   = 0.02585_f64;
    let expected = is_ * ((vd / vt).exp() - 1.0);

    let node_voltages = [vd, 0.0];
    let params        = [is_, vt];
    let mut rhs       = [0.0_f64; 2];

    dev.eval_residual(&node_voltages, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);

    assert!((rhs[0] - expected).abs() / expected.abs() < 1e-10,
        "diode residual mismatch: {} vs {}", rhs[0], expected);
}

#[test]
fn test_diode_jacobian_forward_bias() {
    let prog = parse_and_elaborate(DIODE_SRC).expect("elab");
    let dev  = ir_analog_to_device(&ppr_to_ir(&prog), "Diode").expect("codegen");

    let vd  = 0.5_f64;
    let is_ = 1.0e-14_f64;
    let vt  = 0.02585_f64;
    // d(I)/d(V(p,n)) = Is/vt * exp(V/vt)
    let g_expected = is_ / vt * (vd / vt).exp();

    let node_voltages = [vd, 0.0];
    let params        = [is_, vt];
    let mut jac       = [0.0_f64; 4];

    dev.eval_jacobian(&node_voltages, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut jac);

    assert!((jac[0] - g_expected).abs() / g_expected.abs() < 1e-10,
        "diode J[p,p] mismatch: {} vs {}", jac[0], g_expected);
}

// ── Circuit round-trip tests ──────────────────────────────────────────────────

const RESISTOR_IN_TOP_SRC: &str = "
discipline Electrical {
    potential v : Real;
    flow      i : Real;
}
mod Resistor ( inout a : Electrical, inout b : Electrical ) {
    param R : Real = 1000.0;
}
analog Resistor {
    I(a, b) <+ V(a, b) / R;
}
mod Top ( inout vdd : Electrical, inout gnd : Electrical ) {
    Resistor ( vdd, gnd );
}
";

#[test]
fn test_jit_compiles() {
    let prog = parse_and_elaborate(RESISTOR_IN_TOP_SRC).expect("elab");
    let ir = ppr_to_ir(&prog);
    let _ci = CircuitCompiler::new(&ir).build_circuit("Top").expect("from_ir");
}

#[test]
fn test_jit_two_resistors() {
    let src = "
discipline Electrical {
    potential v : Real;
    flow      i : Real;
}
mod R1k ( inout a : Electrical, inout b : Electrical ) { param R : Real = 1000.0; }
analog R1k { I(a, b) <+ V(a, b) / R; }
mod R2k ( inout a : Electrical, inout b : Electrical ) { param R : Real = 2000.0; }
analog R2k { I(a, b) <+ V(a, b) / R; }
mod Top ( inout vdd : Electrical, inout mid : Electrical, inout gnd : Electrical ) {
    wire m : Electrical;
    R1k ( vdd, m );
    R2k ( m, gnd );
}
";
    let prog = parse_and_elaborate(src).expect("elab");
    let ir = ppr_to_ir(&prog);
    let _ci = CircuitCompiler::new(&ir).build_circuit("Top").expect("from_ir");
}

fn ir_analog_to_device(
    prog: &piperine_codegen::ir::IrProgram,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let module = prog.module(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).map(|a| a.clone())
}
