//! LRM-COVERS: 4.5.3
//! LRMW2-37 (`lrm-conformance-wave2`, T33): `ddt(expr, abstol|nature)`'s
//! second argument used to be discarded entirely at lowering
//! (`resolve/pom/analog_ops.rs::Ddt::lower` only ever read `args[0]`) —
//! `StateKind::Ddt` carried no tolerance field at all. This target checks
//! the lowering step in isolation (the resolved `StateKind::Ddt { abstol
//! }` field), complementing the full-transient discriminating proof in
//! root `tests/ddt_abstol_lte.rs` (two different `abstol` values reaching
//! two different accepted-step counts through the LTE-driven stepper).

use piperine_codegen::resolve::{StateKind, StateVar};

fn ddt_state(src: &str) -> StateVar {
    let design = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::Ddt { .. }).then(|| s.clone()))
        .expect("a ddt state")
}

/// §4.5.3: an omitted second argument carries no tolerance.
#[test]
fn ddt_without_a_second_argument_has_no_abstol() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ ddt(V(p, n));\n\
        end\n\
        endmodule\n";
    let StateKind::Ddt { abstol } = ddt_state(src).kind else { unreachable!() };
    assert!(abstol.is_none(), "no second argument must leave abstol unset");
}

/// §4.5.3: an explicit numeric `abstol_expression` (`constant_expression`)
/// is carried, not discarded.
#[test]
fn ddt_with_an_explicit_abstol_carries_it() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ ddt(V(p, n), 1e-6);\n\
        end\n\
        endmodule\n";
    let StateKind::Ddt { abstol } = ddt_state(src).kind else { unreachable!() };
    assert!(abstol.is_some(), "an explicit abstol argument must be carried, not discarded");
}

/// §4.5.3: `abstol_expression ::= constant_expression | nature_identifier`
/// — a bare nature identifier resolves through its declared `abstol`
/// attribute at lowering time, literally, not left as an unresolved name.
#[test]
fn ddt_with_a_nature_argument_resolves_to_its_declared_abstol() {
    let src = "nature my_charge;\n\
        units = \"C\";\n\
        access = Q;\n\
        abstol = 2.5e-7;\n\
        endnature\n\
        module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ ddt(V(p, n), my_charge);\n\
        end\n\
        endmodule\n";
    let StateKind::Ddt { abstol } = ddt_state(src).kind else { unreachable!() };
    let piperine_lang::ast::Expr::Number { value, .. } = abstol.expect("nature abstol carried") else {
        panic!("nature abstol should resolve to a literal at lowering time");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => r,
        piperine_lang::lexer::NumberLit::Integer(i) => i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert!((v - 2.5e-7).abs() < 1e-15, "expected the nature's declared abstol 2.5e-7, got {v}");
}
