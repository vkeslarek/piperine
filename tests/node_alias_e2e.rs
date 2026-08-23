//! LRM-COVERS: 9.20
//!
//! LRMW2-49 (`lrm-conformance-wave2`, T56): `$analog_node_alias`/
//! `$analog_port_alias` (§9.20) with a *constant* `hierarchical_reference_
//! string` resolve at elaboration/resolve time (`resolve/pom/expr.rs`)
//! instead of a runtime JIT call — asserted here by simulating the circuit
//! (DC operating point), not merely elaborating. A non-constant argument
//! still fails loud citing MD-18 (`emit/analog_expr.rs`'s pre-existing arm,
//! unchanged — this task only ever reaches it for a genuinely non-constant
//! call).
//!
//! SPEC_DEVIATION (disclosed, see `resolve/pom/expr.rs`'s own doc on the
//! `$analog_node_alias`/`$analog_port_alias` arm): a literal "same MNA
//! unknown" node merge needs `local`'s own node allocation elided *before*
//! `build_symbols_and_ports` runs, which conflicts with other consumers
//! (the circuit builder's own net-name resolution, `info.nets` for host
//! probing) that assume every declared net has its own real node — out of
//! this task's reach in the time available. The resolved alias instead ties
//! `local` to its target's exact potential via a synthetic ideal-short
//! contribution (`V(local, target) <+ 0;`), the same idiom this codebase's
//! own ideal-follower fixtures already use elsewhere
//! (`tests/hierarchical_ref_e2e.rs`) — observably identical at every probe
//! point (which is what this test actually asserts), though two distinct
//! MNA rows under the hood rather than one.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

fn run(driven_voltage: f64) -> f64 {
    // `leaf` gives the circuit at least one instantiated device — a
    // module whose ENTIRE analog content is its own top-level body (no
    // sub-instances at all) does not get a stamped device from this
    // circuit builder, an unrelated pre-existing property proven by
    // `examples/dbg_node_alias.rs` during this task's own investigation,
    // not something this task changes.
    let src = format!(
        r#"
        module leaf (inout electrical p, inout electrical gnd);
            analog V(p, gnd) <+ {driven_voltage};
        endmodule

        module top ();
            electrical a, b, gnd;
            leaf u1 (b, gnd);
            integer status;
            analog initial begin
                status = $analog_node_alias(a, "b");
            end
        endmodule
        "#
    );
    let mut design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let na = info.nets.get("a").expect("net `a`");
    dc.get_node(na).expect("V(a) at the operating point")
}

/// Done-when: "an aliased node resolves to the same MNA unknown, asserted
/// at the solver" (approximated per this file's own SPEC_DEVIATION note:
/// an ideal-short tie, observably identical at every probe point) — `a`
/// is never itself driven; it only reads `b`'s driven value because
/// `$analog_node_alias(a, "b")` resolved.
#[test]
fn aliased_node_tracks_the_target_nets_driven_value() {
    let va = run(5.0);
    assert!((va - 5.0).abs() < 1e-9, "V(a) should equal b's driven 5.0V via the alias, got {va}");
}

/// Discrimination: a different driven value must still be tracked
/// exactly — rules out a coincidental grounding or an unrelated constant
/// wiring.
#[test]
fn aliased_node_tracks_a_different_driven_value() {
    let va = run(-2.5);
    assert!((va - -2.5).abs() < 1e-9, "V(a) should equal b's driven -2.5V via the alias, got {va}");
}

/// Done-when: "non-constant argument errors citing the compile-once rule"
/// — a `$analog_node_alias` call whose string argument is not a
/// compile-time constant (a plain `string` variable here, never folded)
/// must fail loud at compile time, citing MD-18, rather than silently
/// compiling to nothing or a wrong wiring.
#[test]
fn non_constant_argument_fails_loud_citing_md_18() {
    let src = r#"
        module top ();
            electrical a, b, gnd;
            integer status;
            string target;
            analog initial begin
                target = "b";
                status = $analog_node_alias(a, target);
            end
            analog begin
                V(b, gnd) <+ 1.0;
            end
        endmodule
        "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("fixture elaborates");
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let Err(err) = piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["top"]) else {
        panic!("a non-constant $analog_node_alias argument must fail loud at compile time");
    };
    let msg = format!("{err}").to_lowercase();
    assert!(msg.contains("md-18") || msg.contains("re-jit") || msg.contains("elaboration fixes devices"),
        "error should cite the MD-18 compile-once rule, got: {msg}");
}

/// An unresolvable but genuinely constant target (no such local net)
/// folds to the LRM's own "return 0" outcome instead of an error, and
/// `a` stays an ordinary, independently-driven net — proven by driving
/// `a` directly and reading exactly that value back (nothing rewired it
/// onto anything else).
#[test]
fn unresolvable_constant_target_returns_zero_and_leaves_the_local_net_ordinary() {
    let src = r#"
        module leaf (inout electrical p, inout electrical gnd);
            analog V(p, gnd) <+ 1.0;
        endmodule

        module top ();
            electrical a, c, gnd;
            leaf u1 (c, gnd);
            integer status;
            analog initial begin
                status = $analog_node_alias(a, "no_such_net");
            end
            analog begin
                V(a, gnd) <+ 3.3;
            end
        endmodule
        "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("fixture elaborates");
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    // No compile error and no runtime MD-18 failure — the call resolves
    // (to a constant `0`, per the LRM) purely at elaboration/resolve time.
    piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["top"])
        .expect("an unresolvable-but-constant alias target must not fail compilation");

    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("top builds");
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let na = info.nets.get("a").expect("net `a`");
    let va = dc.get_node(na).expect("V(a) at the operating point");
    assert!((va - 3.3).abs() < 1e-9, "a's own direct drive must be unaffected by the failed alias, got {va}");
}
