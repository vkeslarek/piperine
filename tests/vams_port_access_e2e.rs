//! LRM-COVERS: 4.4, 5.4.3
//! LRMC-C1-10 (`.specs/features/lrm-conformance-closure/`, T18): the port
//! access function `I(<port>)` (§4.4/§5.4.3) simulates end-to-end on the
//! native solver, and the value it reads genuinely differs from any single
//! branch's own flow — the KCL-sum rewrite `elab::lower::vams` performs
//! (`crates/piperine-lang/tests/vams_lower_analog.rs` proves the rewrite's
//! *shape* at the elaboration level; this file proves its *numeric* result
//! on a real compiled circuit, per MD-28's cross-crate placement rule).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

/// `leaf`'s port `a` carries two internal *potential-forced* branches to
/// `b` and `c` (`V(a,b) <+ vab`, `V(a,c) <+ vac`) — a branch current is
/// only an MNA unknown for a forced branch (`flatten/analog.rs`'s own
/// "branch-current term in a potential force must probe a forced branch"
/// rule), so `I(<a>)`'s KCL-sum rewrite can only reference these two
/// currents from inside the same module if they're forced, not the
/// natural `I(p,n) <+ V(p,n)/r` resistor form. `mon` is driven (a
/// controlled source, a standard VAMS idiom) to the port-access reading
/// itself, so a DC solve can read "the current the LRM says flows into
/// this module through port a" directly off `mon`'s node voltage.
/// External resistors on `b`/`c` give each forced branch a real,
/// *different* nonzero current to sum (an isolated forced branch with no
/// other connection would trivially carry zero current).
const LEAF_WITH_PORT_PROBE: &str = r#"
    module vsrc (inout electrical p, inout electrical n);
        parameter real dc = 1.0;
        analog begin
            V(p, n) <+ dc;
        end
    endmodule

    module resistor (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog begin
            I(p, n) <+ V(p, n) / r;
        end
    endmodule

    module leaf (inout electrical a, inout electrical b, inout electrical c, inout electrical mon);
        parameter real vab = 0.4;
        parameter real vac = 0.7;
        analog begin
            V(a, b) <+ vab;
            V(a, c) <+ vac;
            V(mon) <+ I(<a>);
        end
    endmodule

    module top ();
        electrical gnd, va, vb, vc, vmon;
        vsrc #(.dc(1.0)) v1 (va, gnd);
        resistor #(.r(1.0e3)) rb (vb, gnd);
        resistor #(.r(2.0e3)) rc (vc, gnd);
        leaf #(.vab(0.4), .vac(0.7)) l1 (va, vb, vc, vmon);
    endmodule
"#;

#[test]
fn port_access_current_is_the_kcl_sum_and_differs_from_either_branch_flow() {
    let design = parse_and_elaborate(LEAF_WITH_PORT_PROBE, &piperine_lang::SourceMap::dummy())
        .expect("the port-probe fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("CircuitCompiler builds the circuit");

    let dc_result = circuit.dc(Context::default()).expect("dc analysis starts").solve().expect("dc converges");

    // v1 holds va at exactly 1.0 V; leaf's own forced branches then fix
    // vb = va - vab = 0.6 V and vc = va - vac = 0.3 V. Each forced
    // branch's current is set by KCL at its far node, whose only other
    // connection is the external resistor to gnd: I(a,b) = vb/rb,
    // I(a,c) = vc/rc.
    let va = 1.0;
    let vab = 0.4;
    let vac = 0.7;
    let vb = va - vab;
    let vc = va - vac;
    let i_ab = vb / 1.0e3; // 6.0e-4
    let i_ac = vc / 2.0e3; // 1.5e-4
    let expected_port_flow = i_ab + i_ac; // 7.5e-4

    let vmon = info.nets.get("vmon").expect("net `vmon`");
    let v_mon = dc_result.get_node(vmon).expect("v(vmon)");

    assert!(
        (v_mon - expected_port_flow).abs() < 1e-9,
        "I(<a>) must equal the KCL sum of the two branches touching port a \
         (I(a,b) + I(a,c) = {i_ab} + {i_ac} = {expected_port_flow}), got {v_mon}"
    );
    assert!(
        (v_mon - i_ab).abs() > 1e-6,
        "port flow ({v_mon}) must differ from the (a,b) branch's own flow ({i_ab}) alone"
    );
    assert!(
        (v_mon - i_ac).abs() > 1e-6,
        "port flow ({v_mon}) must differ from the (a,c) branch's own flow ({i_ac}) alone"
    );
}
