//! LRM-COVERS: 5.6.2, 5.6.2.1
//!
//! Analog device numerics through the whole codegen path: a PHDL module is
//! elaborated, lowered, JIT-compiled, and its residual and Jacobian checked
//! against the closed form (resistor, VCCS, diode).
//!
//! T69 (`lrm-conformance-wave2`) re-reading, no new tests: `Vccs`
//! (`I(outp, outn) <+ gm * V(inp, inn);`) is §5.6.2.1's own "voltage
//! controlled current source" — one of the four controlled-source models
//! §5.6.2's worked examples walk through, with §5.6.2 itself the
//! containing "Examples" heading with no independent rule of its own.
//!
//! Renamed from `codegen_ir.rs` (P6 T12) — there is no IR crate; the stage is
//! `resolve` → `kernel` → `device`. Kernel-level numerics for one module live
//! in `analog_kernel.rs`; this file goes through `CircuitCompiler`.


use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;
use piperine_codegen::CircuitCompiler;

// ── Resistor ──────────────────────────────────────────────────────────────────

const RESISTOR_SRC: &str = "
module Resistor ( inout electrical p, inout electrical n );
    parameter real R = 1000.0;
    analog I(p, n) <+ V(p, n) / R;
endmodule
";

#[test]
fn test_compile_resistor() {
    let prog = parse_and_elaborate(RESISTOR_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev  = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "Resistor").expect("codegen");

    assert_eq!(dev.num_terminals(), 2);
    assert_eq!(dev.num_params(),    1);
    assert_eq!(dev.num_params(), 1);
}

#[test]
fn test_resistor_residual_ohms_law() {
    let prog = parse_and_elaborate(RESISTOR_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev  = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "Resistor").expect("codegen");

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
    let prog = parse_and_elaborate(RESISTOR_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev  = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "Resistor").expect("codegen");

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
module Vccs ( inout electrical inp, inout electrical inn,
              inout electrical outp, inout electrical outn );
    parameter real gm = 0.01;
    analog I(outp, outn) <+ gm * V(inp, inn);
endmodule
";

#[test]
fn test_vccs_residual() {
    let prog = parse_and_elaborate(VCCS_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev  = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "Vccs").expect("codegen");

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
    let prog = parse_and_elaborate(VCCS_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev  = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "Vccs").expect("codegen");

    let node_voltages = [0.5_f64, 0.0, 0.0, 0.0];
    let params        = [0.01_f64];
    let mut jac       = [0.0_f64; 16]; // 4×4

    dev.eval_jacobian(&node_voltages, &params, &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut jac);

    let gm = 0.01_f64;
    let n  = 4usize;

    // d(I_outp)/d(V_inp) = +gm, d(I_outp)/d(V_inn) = -gm
    assert!((jac[2*n] -  gm).abs() < 1e-12, "J[outp,inp] = {}", jac[2*n]);
    assert!((jac[2*n + 1] - -gm).abs() < 1e-12, "J[outp,inn] = {}", jac[2*n+1]);
    // d(I_outn)/d(V_inp) = -gm, d(I_outn)/d(V_inn) = +gm
    assert!((jac[3*n] - -gm).abs() < 1e-12, "J[outn,inp] = {}", jac[3*n]);
    assert!((jac[3*n + 1] -  gm).abs() < 1e-12, "J[outn,inn] = {}", jac[3*n+1]);
}

// ── Diode (nonlinear) ─────────────────────────────────────────────────────────

const DIODE_SRC: &str = "
module Diode ( inout electrical p, inout electrical n );
    parameter real Is = 1.0e-14;
    parameter real vt = 0.02585;
    analog I(p, n) <+ Is * (exp(V(p, n) / vt) - 1.0);
endmodule
";

#[test]
fn test_diode_residual_forward_bias() {
    let prog = parse_and_elaborate(DIODE_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev  = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "Diode").expect("codegen");

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
    let prog = parse_and_elaborate(DIODE_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev  = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "Diode").expect("codegen");

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
module Resistor ( inout electrical a, inout electrical b );
    parameter real R = 1000.0;
    analog I(a, b) <+ V(a, b) / R;
endmodule
module Top ( inout electrical vdd, inout electrical gnd );
    Resistor r1 ( vdd, gnd );
endmodule
";

#[test]
fn test_jit_compiles() {
    let prog = parse_and_elaborate(RESISTOR_IN_TOP_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let ir = piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed");
    let _ci = CircuitCompiler::new(&prog, &ir).build_circuit("Top").expect("from_ir");
}

#[test]
fn test_jit_two_resistors() {
    let src = "
module R1k ( inout electrical a, inout electrical b );
    parameter real R = 1000.0;
    analog I(a, b) <+ V(a, b) / R;
endmodule
module R2k ( inout electrical a, inout electrical b );
    parameter real R = 2000.0;
    analog I(a, b) <+ V(a, b) / R;
endmodule
module Top ( inout electrical vdd, inout electrical mid, inout electrical gnd );
    electrical m;
    R1k r1 ( vdd, m );
    R2k r2 ( m, gnd );
endmodule
";
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elab");
    let ir = piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed");
    let _ci = CircuitCompiler::new(&prog, &ir).build_circuit("Top").expect("from_ir");
}

fn ir_analog_to_device(
    bodies: &std::collections::HashMap<String, piperine_codegen::resolve::LoweredBody>,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let module = bodies.get(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).cloned()
}
