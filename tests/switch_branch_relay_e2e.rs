//! LRM-COVERS: 5.6.5
//!
//! LRMW2-40 (`lrm-conformance-wave2`, T45): a conditional potential
//! contribution on a branch that also carries a flow contribution from a
//! conditional's other arm models a switch (§5.6.5) — `CircuitCompiler`
//! used to fail this with `CodegenError::Unsupported("conditional
//! potential contribution")` regardless of whether the branch qualified
//! for the §5.6.5 exception. This is the AC15 end-to-end proof: the LRM's
//! own §5.6.5 relay example (same structure — `@(cross(V(cp,cn) - thresh,
//! 0)) ;` followed by `if (V(cp,cn) > thresh) V(p,n) <+ 0; else I(p,n) <+
//! 0;`; the LRM's own text names the branches `out`/`ctrl` via `branch
//! (p,n) out;` — the two-terminal form is used here instead because
//! resolving a contribution target by *branch name* (`V(out)`) is a
//! separate, pre-existing gap in `flatten::parse_dest`, unrelated to
//! switch-branch semantics; not this task's scope), simulated across a
//! transient that ramps the control node through the threshold, proving
//! the relay actually toggles between the shorted and open state — not
//! merely that the module elaborates/compiles.
//!
//! Disclosed approximation (documented at the fix site,
//! `flatten/analog.rs::add_switch_potential`): the "closed"
//! (potential-contribution) state is modelled as a very large Norton
//! conductance rather than a structural ideal-source branch-type change
//! (this fixed-shape MNA kernel has no such structural switch) — tight
//! enough that the divider fixture below reads within 1e-6 V of the exact
//! short. The "open" (flow-contribution) state is exact: `I(out) <+ 0;`
//! is an ordinary guarded flow contribution, unchanged by this fix.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// The LRM §5.6.5 relay, verbatim in structure, wired into a voltage
/// divider so its two states are observable: `outnode` is pulled toward
/// `vsup` through a resistor when the relay is open (no path to ground),
/// and toward `0` when the relay is closed (a near-short to ground). The
/// control node ramps linearly through `thresh = 0` at `t = 0.5s` over a
/// 1s transient.
const FIXTURE: &str = r#"
    module vsrc (inout electrical p, inout electrical n);
        parameter real dc = 5.0;
        analog V(p, n) <+ dc;
    endmodule

    module ramp (inout electrical p, inout electrical n);
        analog V(p, n) <+ -1.0 + 2.0 * $abstime;
    endmodule

    module res (inout electrical p, inout electrical n);
        parameter real r = 1e3;
        analog I(p, n) <+ V(p, n) / r;
    endmodule

    module relay (inout electrical p, n, cp, cn);
        parameter real thresh = 0;
        analog begin
            @(cross(V(cp, cn) - thresh, 0))
                ; // acts only to resolve threshold crossings
            if (V(cp, cn) > thresh)
                V(p, n) <+ 0;
            else
                I(p, n) <+ 0; // optional due to value retention
        end
    endmodule

    module top ();
        electrical gnd, vsup, ctrlnode, outnode;
        vsrc vs (vsup, gnd);
        ramp rp (ctrlnode, gnd);
        res r1 (vsup, outnode);
        relay rel1 (outnode, gnd, ctrlnode, gnd);
    endmodule
"#;

fn run() -> Vec<(f64, f64)> {
    let design = parse_and_elaborate(FIXTURE, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let opts = TransientAnalysisOptions::new(1.0, 0.005);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    let result = tran.solve().unwrap_or_else(|e| panic!("transient solves: {e:?}"));
    let outnode = info.nets.get("outnode").expect("net `outnode`");
    result.iter().filter_map(|step| step.get_node(outnode).map(|v| (step.time(), v))).collect()
}

/// AC15: the relay toggles across the transient — `outnode` sits near the
/// supply voltage while open (before the crossing) and near `0` while
/// closed (after it), not a single constant value throughout.
#[test]
fn relay_toggles_between_open_and_shorted_state_across_a_transient() {
    let trace = run();
    assert!(trace.len() > 10, "expected a real transient trace, got {} points", trace.len());

    let before = trace.iter().find(|(t, _)| *t > 0.1 && *t < 0.2).map(|&(_, v)| v)
        .expect("a sample before the t=0.5s crossing");
    let after = trace.iter().rev().find(|(t, _)| *t > 0.8 && *t < 0.9).map(|&(_, v)| v)
        .expect("a sample after the t=0.5s crossing");

    assert!(
        (before - 5.0).abs() < 1e-6,
        "open state (ctrl < thresh): outnode should float to the 5.0V supply through the resistor \
         (no current path, I(out)<+0 is exact), got {before}"
    );
    assert!(
        after.abs() < 1e-6,
        "closed state (ctrl > thresh): outnode should be pulled to ~0V by the near-ideal short, \
         got {after}"
    );
    assert!((before - after).abs() > 4.9, "the relay must produce two clearly distinct states, not one");
}
