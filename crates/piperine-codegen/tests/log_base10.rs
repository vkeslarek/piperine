//! `log` is base-10 (LRM Table 4-14), distinct from natural `ln` — end to
//! end through the JIT: a device that contributes `log(V)` must produce the
//! decimal logarithm's residual AND Jacobian, not the natural log's.
//!
//! LRM-COVERS: 4.3

use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

const LOG_SRC: &str = "
module LogDevice ( inout electrical p, inout electrical n );
    analog I(p, n) <+ log(V(p, n));
endmodule
";

const LN_SRC: &str = "
module LnDevice ( inout electrical p, inout electrical n );
    analog I(p, n) <+ ln(V(p, n));
endmodule
";

fn ir_analog_to_device(
    bodies: &std::collections::HashMap<String, piperine_codegen::resolve::LoweredBody>,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let module = bodies.get(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).cloned()
}

#[test]
fn log_call_produces_base_10_residual() {
    let prog = parse_and_elaborate(LOG_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "LogDevice").expect("codegen");

    let v = 100.0_f64;
    let node_voltages = [v, 0.0];
    let mut rhs = [0.0_f64; 2];
    dev.eval_residual(&node_voltages, &[], &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);

    // log10(100) = 2.0 exactly. ln(100) ≈ 4.6052 — the discriminating check
    // below proves this isn't accidentally still natural log.
    assert!((rhs[0] - 2.0).abs() < 1e-12, "rhs[p] = {} (expected log10(100) = 2.0)", rhs[0]);
    assert!((rhs[0] - v.ln()).abs() > 1.0, "log() must not equal ln() here");
}

#[test]
fn log_call_jacobian_matches_base_10_derivative() {
    // d/dV log10(V) = 1 / (V * ln(10)), evaluated at V = 100.
    let prog = parse_and_elaborate(LOG_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "LogDevice").expect("codegen");

    let v = 100.0_f64;
    let node_voltages = [v, 0.0];
    let mut jac = [0.0_f64; 4];
    dev.eval_jacobian(&node_voltages, &[], &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut jac);

    let expected = 1.0 / (v * std::f64::consts::LN_10);
    assert!((jac[0] - expected).abs() / expected.abs() < 1e-8, "J[p,p] = {} (expected {})", jac[0], expected);
}

#[test]
fn ln_call_stays_natural_log() {
    let prog = parse_and_elaborate(LN_SRC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let dev = ir_analog_to_device(&piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed"), "LnDevice").expect("codegen");

    let v = std::f64::consts::E;
    let node_voltages = [v, 0.0];
    let mut rhs = [0.0_f64; 2];
    dev.eval_residual(&node_voltages, &[], &vec![0.0; dev.num_state_slots()], &[], &SimCtx::default(), &mut rhs);

    assert!((rhs[0] - 1.0).abs() < 1e-10, "rhs[p] = {} (expected ln(e) = 1.0)", rhs[0]);
}
