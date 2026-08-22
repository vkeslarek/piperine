//! LRM-COVERS: 4.5.5
//! LRMW2-30 (`lrm-conformance-wave2`, T35): `idtmod(expr, ic, modulus,
//! offset)`'s fourth argument was silently discarded — the compiled
//! integrator always wrapped into `[0, modulus)`, never the shifted
//! `[offset, offset+modulus)` §4.5.5 requires. Checks lowering
//! (`StateKind::IdtMod.offset`) plus the same const-eval `AnalogInstance::
//! new` runs to seed the runtime `Integrator`'s slot (`ic + offset`,
//! `SlottedOperator::initial_output`'s formula) against two different
//! `offset` parameter values.

use piperine_codegen::kernel::analog::{AnalogKernel, RuntimeState};
use piperine_codegen::resolve::{StateKind, StateVar};
use piperine_lang::SourceMap;

fn idtmod_state(src: &str) -> StateVar {
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::IdtMod { .. }).then(|| s.clone()))
        .expect("an idtmod state")
}

/// §4.5.5: an omitted fourth argument defaults `offset` to the
/// pre-existing `[0, modulus)` window — carried as a literal `0`, not
/// left unresolved.
#[test]
fn idtmod_without_an_offset_argument_defaults_the_lowered_offset_to_zero() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ idtmod(V(p, n), 0, 10);\n\
        end\n\
        endmodule\n";
    let StateKind::IdtMod { offset, .. } = idtmod_state(src).kind else { unreachable!() };
    let piperine_lang::ast::Expr::Number { value, .. } = offset else {
        panic!("expected a literal default offset");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => r,
        piperine_lang::lexer::NumberLit::Integer(i) => i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert_eq!(v, 0.0, "an omitted offset must default to 0.0, not be left unresolved");
}

/// AC14 discrimination: the compiled kernel's `RuntimeState::Integrator`
/// carries `offset` as a live (param-dependent) expression, not folded
/// away or dropped — const-evaluating it (`pom_eval_const`, the exact
/// function `AnalogInstance::new` itself calls to seed the runtime state,
/// via `value(offset)` in `device/analog/instance.rs`) against two
/// different `offset` parameter values must give two different results.
/// (The `ic + offset` combination this then feeds into is
/// `SlottedOperator::initial_output`'s own formula, proven directly by
/// `device/analog/operators.rs`'s `offset_seeds_the_initial_output_too`
/// unit test — this test's job is only proving the compiled kernel
/// actually carries a *live*, per-instance `offset`.)
#[test]
fn two_different_offset_params_const_eval_to_two_different_values() {
    let src = "
module dut (inout electrical out);
    parameter real offset = 0.0;
    analog begin
        V(out) <+ idtmod(0.0, 0.0, 10.0, offset);
    end
endmodule
";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let module = bodies.get("dut").expect("dut lowered");
    let kernel: AnalogKernel = AnalogKernel::compile(module).expect("compiles");

    let spec = kernel.runtime_states().first().expect("one runtime integrator state");
    let RuntimeState::Integrator { offset, .. } = &spec.kind else {
        panic!("expected an Integrator runtime state");
    };
    assert!(kernel.param_names().iter().any(|p| p == "offset"), "offset must be a real param");

    let offset_at = |offset_value: f64| -> f64 {
        let resolve = |name: &str| -> Option<f64> { (name == "offset").then_some(offset_value) };
        piperine_codegen::resolve::pom_eval_const(offset, &resolve).expect("offset const-evals")
    };
    let a = offset_at(3.0);
    let b = offset_at(7.0);
    assert_eq!(a, 3.0);
    assert_eq!(b, 7.0);
    assert!(a != b, "two different offset params must const-eval to two different values");
}
