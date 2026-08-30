//! LRM-COVERS: 3.12.1, 5.4.3
//!
//! T29 (`lrm-conformance-wave3`, LRMW3-13): `branch (<p>) probe_p;`
//! (§3.12.1) declares a *port branch* — the branch between a port's upper
//! (external) and lower (internal) connections — and `I(probe_p)` reads
//! the flow into the port (§5.4.3). Piperine's ports have no separate
//! upper/lower node identity by default (an instantiation binds a port
//! directly to the parent's connected node): `elab/lower/vams.rs::
//! apply_port_branches` materializes the split for real — a fresh
//! internal wire replaces every internal body reference to the port, and
//! a synthetic ideal `V(p, <fresh>) <+ 0;` contribution reconnects them,
//! reusing the existing force-branch/`HAS_INTERNAL_UNKNOWNS` machinery
//! (`flatten/analog.rs`'s `FlatForce`, unchanged) so the branch's current
//! *is* the port's flow.
//!
//! **Disclosed scope (documented in the T29 commit body)**: the port must
//! be scalar and a *local* port of the declaring module — the LRM's
//! `hierarchical_port_identifier` grammar alternative is not implemented.
//! An instance connection that passes a port-branch port straight through
//! to a child instance is not rewritten. A bare, direction-less port
//! (`module m(p); electrical p;`, the LRM's own §3.12.1 example) does not
//! elaborate in Piperine independently of this task (a pre-existing gap —
//! every port needs an explicit `input`/`output`/`inout`), so the fixtures
//! below spell the direction explicitly.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

/// AC1 (spec.md "P2: Branch and contribution completeness"): a port
/// branch reads a **nonzero** port current that matches the current the
/// connected network delivers — §3.12.1's own `current_sink` idea, adapted
/// with an explicit port direction (see the file header's disclosed gap)
/// and a resistor so the probed current is not trivially zero. `top`
/// forces `V(p) = 2.0` through an ideal source; `current_sink` internally
/// loads its (post-split) inner net with a `r`-ohm resistor to ground —
/// the only element touching that inner net besides the port branch
/// itself, so by KCL the entire `V(p)/r` resistor current must flow
/// through the port branch, which `I(probe_p)` reads back out onto the
/// externally visible `mon` net.
#[test]
fn port_branch_reads_a_nonzero_current_matching_the_connected_network() {
    let src = r#"
        module current_sink(inout p, output mon);
            electrical p, gnd, mon;
            ground gnd;
            branch (<p>) probe_p;
            parameter real r = 100;
            analog begin
                I(p, gnd) <+ V(p, gnd) / r;
                V(mon, gnd) <+ I(probe_p);
            end
        endmodule
        module top();
            electrical p, gnd, mon;
            ground gnd;
            current_sink cs(p, mon);
            analog begin
                V(p, gnd) <+ 2.0;
            end
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let mon = info.nets.get("mon").expect("net `mon`");
    let probed_current = dc.get_node(mon).expect("V(mon) == I(probe_p) at the operating point");
    let expected = 2.0 / 100.0;
    assert!(
        probed_current.abs() > 1e-6,
        "the probed current must be nonzero, got {probed_current}"
    );
    assert!(
        (probed_current - expected).abs() < 1e-9,
        "I(probe_p) must match the resistor current V(p)/r = {expected}, got {probed_current}"
    );
}

/// Discrimination companion: changing the driving voltage (and therefore
/// the current the connected network actually delivers) must change what
/// `I(probe_p)` reports — a resolution that accidentally wired the probe
/// to a constant (e.g. always 0, or the driving voltage itself) would
/// pass the fixed-value case above by coincidence but fail this one.
#[test]
fn port_branch_current_tracks_a_different_driving_voltage() {
    let src = |v: f64| {
        format!(
            r#"
        module current_sink(inout p, output mon);
            electrical p, gnd, mon;
            ground gnd;
            branch (<p>) probe_p;
            parameter real r = 50;
            analog begin
                I(p, gnd) <+ V(p, gnd) / r;
                V(mon, gnd) <+ I(probe_p);
            end
        endmodule
        module top();
            electrical p, gnd, mon;
            ground gnd;
            current_sink cs(p, mon);
            analog begin
                V(p, gnd) <+ {v};
            end
        endmodule
        "#
        )
    };
    let run = |v: f64| -> f64 {
        let mut design = parse_and_elaborate(&src(v), &piperine_lang::SourceMap::dummy())
            .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
        design.set_top("top");
        let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
        let mut compiler = CircuitCompiler::new(&design, &bodies);
        let (mut circuit, info) =
            compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
        let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
        let mon = info.nets.get("mon").expect("net `mon`");
        dc.get_node(mon).expect("V(mon)")
    };
    let a = run(3.0);
    let b = run(-1.5);
    assert!((a - 3.0 / 50.0).abs() < 1e-9, "expected 3.0/50 == 0.06, got {a}");
    assert!((b - (-1.5 / 50.0)).abs() < 1e-9, "expected -1.5/50 == -0.03, got {b}");
}

/// A module **without** a port branch is completely unaffected —
/// `apply_port_branches` is a no-op when no `branch (<p>) ...;` is
/// declared: the flattened net set is exactly the ports/wires the module
/// authored, no synthetic internal wire, no extra branch bookkeeping.
#[test]
fn module_without_a_port_branch_has_an_unchanged_node_count() {
    let src = r#"
        module plain(inout p, inout q);
            electrical p, q, gnd;
            ground gnd;
            analog V(p, q) <+ 1.0;
        endmodule
        module top();
            electrical a, b;
            plain u1(a, b);
        endmodule
    "#;
    let mut design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    // `plain` has exactly its declared nodes (p, q, gnd) plus the
    // `SymbolTable`'s own pre-interned `NodeId::GROUND` slot (`gnd`, always
    // node 0) — 4 total. No synthetic wire was added by the
    // (never-triggered) port-branch machinery.
    let plain = &bodies["plain"];
    assert_eq!(
        plain.symbols.nodes().count(),
        4,
        "a module with no port branch must carry exactly its own declared nodes (p, q, gnd) \
         plus the pre-interned ground slot"
    );
    assert!(
        !design.module("plain").unwrap().branches().iter().any(|b| b.is_port),
        "no `branch (<p>) ...;` was declared — no port branch may appear in the catalog"
    );
}

/// AC/Done-when bullet 3: a potential access function on a port branch
/// (`V(probe_p)`) is meaningless (§5.4.3: a port branch represents the
/// flow into a port, not a potential) and fails loud citing the clause.
#[test]
fn potential_access_on_a_port_branch_fails_loud() {
    let src = r#"
        module current_sink(inout p);
            electrical p, gnd;
            ground gnd;
            branch (<p>) probe_p;
            analog begin
                V(gnd, gnd) <+ V(probe_p);
            end
        endmodule
    "#;
    let err = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect_err("V(probe_p) on a port branch must fail loud");
    let msg = format!("{err:?}");
    assert!(msg.contains("§5.4.3"), "expected the §5.4.3 clause cited, got: {msg}");
    assert!(msg.contains("probe_p"), "expected the port branch named, got: {msg}");
}

/// The port a port branch names must be a real, local port of the
/// declaring module (§3.12.1) — a typo'd or nonexistent port name fails
/// loud naming it, rather than silently doing nothing.
#[test]
fn port_branch_on_a_nonexistent_port_fails_loud() {
    let src = r#"
        module m(inout p);
            electrical p;
            branch (<bogus>) probe_p;
        endmodule
    "#;
    let err = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy())
        .expect_err("`bogus` is not a port of `m`");
    let msg = format!("{err:?}");
    assert!(msg.contains("§3.12.1"), "expected §3.12.1 cited, got: {msg}");
    assert!(msg.contains("bogus"), "expected the offending name `bogus`, got: {msg}");
}
