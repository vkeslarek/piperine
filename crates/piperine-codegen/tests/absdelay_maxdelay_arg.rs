//! LRM-COVERS: 4.5.7
//! LRMW2-31 (`lrm-conformance-wave2`, T36): `absdelay(input, td,
//! maxdelay)`'s third argument was silently discarded at lowering
//! (`resolve/pom/analog_ops.rs::Delay::lower` only ever read `args[0..1]`)
//! — `StateKind::Delay.delay` always carried the bare `td` expression.
//! Checks the lowering step in isolation, complementing the full-transient
//! discriminating proof in root `tests/absdelay_maxdelay.rs`.

use piperine_codegen::resolve::{StateKind, StateVar};
use piperine_lang::ast::{BinOp, Expr};

fn delay_state(src: &str) -> StateVar {
    let design = piperine_lang::parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::Delay { .. }).then(|| s.clone()))
        .expect("a delay state")
}

/// §4.5.7: `absdelay(x, td)` with no `maxdelay` keeps the bare `td`
/// expression — the pre-existing no-`maxdelay` behavior is unchanged.
#[test]
fn absdelay_without_maxdelay_carries_the_bare_td() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ absdelay(V(p, n), 0.5);\n\
        end\n\
        endmodule\n";
    let StateKind::Delay { delay } = delay_state(src).kind else { unreachable!() };
    assert!(
        matches!(delay, Expr::Number { .. }),
        "no maxdelay: delay must stay the bare td literal, got {delay:?}"
    );
}

/// §4.5.7: `absdelay(x, td, maxdelay)` lowers to a `td > maxdelay ?
/// maxdelay : td` clamp — not a bare `td`, not an error marker.
#[test]
fn absdelay_with_maxdelay_lowers_to_a_clamp_ternary() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ absdelay(V(p, n), 0.5, 0.2);\n\
        end\n\
        endmodule\n";
    let StateKind::Delay { delay } = delay_state(src).kind else { unreachable!() };
    let Expr::Ternary { cond, .. } = &delay else {
        panic!("with maxdelay: delay must lower to a clamp ternary, got {delay:?}");
    };
    let Expr::Binary { op, .. } = cond.as_ref() else {
        panic!("clamp condition must be a comparison, got {cond:?}");
    };
    assert_eq!(*op, BinOp::Gt, "clamp condition must be `td > maxdelay`");
}
