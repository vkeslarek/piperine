//! LRM-COVERS: 4.5.15
//! LRMW2-35 (`lrm-conformance-wave2`, T39): `laplace_*(expr, num, den[, ε])`'s
//! trailing `abstol_expression` (§4.5.15's table: `abstol` for every one of
//! `laplace_zp`/`laplace_zd`/`laplace_np`/`laplace_nd`) used to be silently
//! discarded — `Laplace::lower` (`resolve/pom/analog_ops.rs`) only ever read
//! `args[0..=2]`, and `StateKind::Laplace` carried no tolerance field at
//! all. This target checks the lowering step in isolation (the resolved
//! `StateKind::Laplace { abstol }` field), complementing the full-transient
//! discriminating proof in root `tests/laplace_abstol_lte.rs` (two
//! different `ε` values reaching two different accepted-step counts
//! through the LTE-driven stepper — the same mechanism `ddt`'s own abstol
//! already uses, `tests/ddt_abstol_lte.rs`, T33).

use piperine_codegen::resolve::{StateKind, StateVar};

fn laplace_state(src: &str) -> StateVar {
    let design = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::Laplace { .. }).then(|| s.clone()))
        .expect("a laplace state")
}

/// §4.5.15: an omitted trailing `ε` carries no tolerance.
#[test]
fn laplace_without_an_abstol_argument_has_none() {
    let src = "module dut (inout electrical p, inout electrical n);
        analog begin
        I(p, n) <+ laplace_nd(V(p, n), '{0,1}, '{-1,0,1});
        end
        endmodule\n";
    let StateKind::Laplace { abstol, .. } = laplace_state(src).kind else { unreachable!() };
    assert!(abstol.is_none(), "no trailing argument must leave abstol unset");
}

/// §4.5.15: an explicit `ε` (`abstol_expression`) is carried, not discarded.
#[test]
fn laplace_with_an_explicit_abstol_carries_it() {
    let src = "module dut (inout electrical p, inout electrical n);
        analog begin
        I(p, n) <+ laplace_nd(V(p, n), '{0,1}, '{-1,0,1}, 1e-6);
        end
        endmodule\n";
    let StateKind::Laplace { abstol, .. } = laplace_state(src).kind else { unreachable!() };
    let piperine_lang::ast::Expr::Number { value, .. } = abstol.expect("abstol carried") else {
        panic!("a numeric abstol should resolve to a literal at lowering time");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => r,
        piperine_lang::lexer::NumberLit::Integer(i) => i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert!((v - 1e-6).abs() < 1e-15, "expected the declared abstol 1e-6, got {v}");
}
