//! LRM §9.15: `$simparam` with an unknown key and no default is a loud
//! error naming the key and the valid set — not a silent `0.0`
//! (SPEC_DEVIATION.md 2.15).
//!
//! LRM-COVERS: 9.15

use std::sync::Arc;

use piperine_codegen::{AnalogKernel, CodegenError, SimCtx};
use piperine_lang::parse_and_elaborate;

fn compile(module_name: &str, src: &str) -> Result<Arc<AnalogKernel>, CodegenError> {
    let prog = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("{module_name} elaborates: {e}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&prog).expect("lowering failed");
    let module = bodies.get(module_name).expect("module lowered");
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    Ok(compiled.analog().expect("analog body").clone())
}

const NO_DEFAULT_SRC: &str = "
module SimparamNoDefault ( inout electrical p, inout electrical n );
    analog I(p, n) <+ $simparam(\"nope\") * V(p, n);
endmodule
";

/// spec AC12: `$simparam(\"nope\")` (no default) is a loud error naming
/// the key and the valid set.
#[test]
fn unknown_key_without_default_fails_loud_by_typed_kind() {
    let result = compile("SimparamNoDefault", NO_DEFAULT_SRC);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("SimparamNoDefault must fail to compile"),
    };
    match err {
        CodegenError::Unsupported(msg) => {
            assert!(msg.contains("nope"), "names the offending key: {msg}");
            assert!(
                msg.contains("gmin") && msg.contains("temperature") && msg.contains("step")
                    && msg.contains("tfinal") && msg.contains("sourceScaleFactor"),
                "names the valid set: {msg}"
            );
        }
        other => panic!("expected CodegenError::Unsupported, got {other:?}"),
    }
}

const WITH_DEFAULT_SRC: &str = "
module SimparamWithDefault ( inout electrical p, inout electrical n );
    analog I(p, n) <+ $simparam(\"nope\", 1.5) * V(p, n);
endmodule
";

/// spec AC12: `$simparam(\"nope\", 1.5)` still returns the explicit
/// default — the escape hatch stays working.
#[test]
fn unknown_key_with_default_still_returns_it() {
    let kernel = compile("SimparamWithDefault", WITH_DEFAULT_SRC).expect("compiles with an explicit default");
    let params: [f64; 0] = [];
    let node_voltages = [1.0_f64, 0.0];
    let mut rhs = [0.0_f64; 2];
    kernel.eval_residual(&node_voltages, &params, &vec![0.0; kernel.num_state_slots()], &[], &SimCtx::default(), &mut rhs);
    // I(p, n) <+ $simparam("nope", 1.5) * V(p, n) = 1.5 * 1.0 = 1.5
    assert!((rhs[0] - 1.5).abs() < 1e-12, "must use the explicit default 1.5: rhs[0] = {}", rhs[0]);
}
