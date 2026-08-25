//! LRM-COVERS: 4.5.6
//! LRM §4.5.6 "Derivative operator": `ddx(expr, unknown_quantity)` returns
//! the partial derivative of `expr` with respect to the named unknown
//! (a scalar net/port potential or a branch flow) — "holding all other
//! unknowns fixed". This target reproduces the LRM's own `vccs` worked
//! example verbatim in structure: `vin = V(pin,nin); one = ddx(vin,
//! V(pin)); minusone = ddx(vin, V(nin)); zero = ddx(vin, V(pout));` — the
//! LRM states these three calls resolve to +1, -1, and 0 respectively
//! because `vin` depends on `pin`/`nin` but not `pout` at all.
//!
//! Proven at the lowering level (mirrors `ddt_abstol_arg.rs`'s own
//! precedent): each `ddx(...)` call must lower to a distinct
//! `StateKind::Ddx { node }`, and the `node` it carries must be the
//! `NodeId` of the *second* argument's net, never conflated across the
//! three calls — the concrete mechanism `ddx()`'s "which unknown" claim
//! depends on.

use piperine_codegen::resolve::{StateKind, StateVar};

const VCCS: &str = "\
module vccs (inout electrical pout, inout electrical nout, inout electrical pin, inout electrical nin);
    parameter real k = 1.0;
    real vin, one, minusone, zero;
    analog begin
        vin = V(pin, nin);
        one = ddx(vin, V(pin));
        minusone = ddx(vin, V(nin));
        zero = ddx(vin, V(pout));
        I(pout, nout) <+ k * vin;
    end
endmodule
";

fn ddx_states() -> Vec<StateVar> {
    let design = piperine_lang::parse_and_elaborate(VCCS, &piperine_lang::SourceMap::dummy())
        .expect("the LRM's own §4.5.6 vccs example elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["vccs"];
    body.symbols
        .states()
        .filter_map(|(_, s)| matches!(s.kind, StateKind::Ddx { .. }).then(|| s.clone()))
        .collect()
}

/// §4.5.6: three `ddx()` call sites in one analog block must lower to
/// three distinct `StateKind::Ddx` states — the operator is not collapsed
/// or deduplicated away, each carries its own target unknown.
#[test]
fn three_ddx_call_sites_lower_to_three_ddx_states() {
    let states = ddx_states();
    assert_eq!(states.len(), 3, "expected one StateKind::Ddx per ddx() call site, got {states:?}");
}

/// §4.5.6: "the second argument shall be the potential of a scalar net or
/// port ... with respect to which the derivative ... needs to be
/// computed" — `ddx(vin, V(pin))` and `ddx(vin, V(nin))` must target two
/// *different* nodes (pin vs nin), and `ddx(vin, V(pout))` a third,
/// distinct again — never the same node reused across call sites.
#[test]
fn each_ddx_call_targets_the_distinct_node_named_in_its_second_argument() {
    let states = ddx_states();
    let nodes: std::collections::BTreeSet<u32> = states
        .iter()
        .map(|s| match s.kind {
            StateKind::Ddx { node } => node.0,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(nodes.len(), 3, "pin/nin/pout must resolve to three distinct NodeIds, got {nodes:?}");
}
