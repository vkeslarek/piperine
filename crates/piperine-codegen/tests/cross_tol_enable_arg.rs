//! LRM-COVERS: 5.10.3.1, 5.10.3.2
//! LRMW2-33 (`lrm-conformance-wave2`, T41): `cross(expr, dir[, time_tol[,
//! expr_tol[, enable]]])` / `above(expr[, time_tol[, expr_tol[, enable]]])`
//! (§5.10.3.1/§5.10.3.2) silently discarded `time_tol`/`expr_tol`/`enable`
//! entirely — `event_expr_to_sources` (`flatten/analog.rs`) only ever read
//! `cross`'s `args[0..=1]` / `above`'s `args[0]`. This target checks the
//! compiled trigger shape in isolation (`CompiledTrigger::Cross`/`Above`'s
//! `time_tol`/`expr_tol`/`enable` fields); the discriminating behavioral
//! proofs (spec AC14) live in root `tests/cross_enable_e2e.rs` (`enable = 0`
//! suppresses the event entirely) and `tests/cross_time_tol_lte.rs` (a
//! tighter `time_tol` forces more accepted transient steps).

use piperine_codegen::kernel::analog::{AnalogKernel, CompiledTrigger};
use piperine_lang::parse_and_elaborate;

fn compile_body(body: &str) -> AnalogKernel {
    let src = format!(
        "module dut (inout electrical p, inout electrical n);
            analog begin
                {body}
            end
        endmodule"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("dut elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    AnalogKernel::compile(&bodies["dut"]).expect("dut compiles")
}

#[test]
fn cross_without_tolerances_or_enable_carries_none_and_the_default_enable() {
    let kernel = compile_body("@(cross(V(p,n), 1)) V(p,n) <+ 0.0;");
    let CompiledTrigger::Cross { time_tol, expr_tol, enable, .. } = &kernel.events()[0].trigger else {
        panic!("expected a Cross trigger");
    };
    assert!(time_tol.is_none(), "no 3rd argument must leave time_tol unset");
    assert!(expr_tol.is_none(), "no 4th argument must leave expr_tol unset");
    let piperine_lang::ast::Expr::Number { value, .. } = enable.as_ref() else {
        panic!("expected a numeric enable default");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => *r,
        piperine_lang::lexer::NumberLit::Integer(i) => *i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert_eq!(v, 1.0, "an omitted enable must default to 1.0 (always active)");
}

#[test]
fn cross_with_explicit_tolerances_and_enable_carries_them() {
    let kernel = compile_body("@(cross(V(p,n), 1, 1e-9, 1e-6, 0.0)) V(p,n) <+ 0.0;");
    let CompiledTrigger::Cross { time_tol, expr_tol, enable, .. } = &kernel.events()[0].trigger else {
        panic!("expected a Cross trigger");
    };
    assert!(time_tol.is_some(), "an explicit time_tol argument must be carried, not discarded");
    assert!(expr_tol.is_some(), "an explicit expr_tol argument must be carried, not discarded");
    let piperine_lang::ast::Expr::Number { value, .. } = enable.as_ref() else {
        panic!("expected a numeric enable literal");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => *r,
        piperine_lang::lexer::NumberLit::Integer(i) => *i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert_eq!(v, 0.0, "an explicit enable=0 must be carried, not discarded");
}

#[test]
fn above_without_tolerances_or_enable_carries_none_and_the_default_enable() {
    let kernel = compile_body("@(above(V(p,n))) V(p,n) <+ 0.0;");
    let CompiledTrigger::Above { time_tol, expr_tol, enable } = &kernel.events()[0].trigger else {
        panic!("expected an Above trigger");
    };
    assert!(time_tol.is_none());
    assert!(expr_tol.is_none());
    let piperine_lang::ast::Expr::Number { value, .. } = enable.as_ref() else {
        panic!("expected a numeric enable default");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => *r,
        piperine_lang::lexer::NumberLit::Integer(i) => *i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert_eq!(v, 1.0);
}

#[test]
fn above_with_explicit_tolerances_and_enable_carries_them() {
    let kernel = compile_body("@(above(V(p,n), 1e-9, 1e-6, 0.0)) V(p,n) <+ 0.0;");
    let CompiledTrigger::Above { time_tol, expr_tol, enable } = &kernel.events()[0].trigger else {
        panic!("expected an Above trigger");
    };
    assert!(time_tol.is_some());
    assert!(expr_tol.is_some());
    let piperine_lang::ast::Expr::Number { value, .. } = enable.as_ref() else {
        panic!("expected a numeric enable literal");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => *r,
        piperine_lang::lexer::NumberLit::Integer(i) => *i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert_eq!(v, 0.0);
}
