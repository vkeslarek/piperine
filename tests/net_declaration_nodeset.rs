//! T14/LRMW2-10 (§3.6.3.2): a net declaration's nodeset value
//! (`electrical n = 0.9;`) reaches the DC solve as a Newton initial-guess
//! hint (`OpConfig.nodeset`) — before this fix `NetDecl::nodeset` was
//! parsed and then dropped on the floor (never read past the parser).
//!
//! The discriminating circuit is the textbook tri-stable nonlinear
//! resistor: a single device with `I(p, gnd) <+ V(p)^3 - V(p)`, no other
//! source — KCL at `p` requires the device current alone to be zero, so
//! `v ∈ {-1, 0, +1}` are all valid DC solutions and the *starting guess*
//! (not the physics) picks which one Newton lands on. An ignored nodeset
//! cannot pick a branch, so this circuit cannot pass by accident.
//!
//! LRM-COVERS: 3.6.3.2

use std::collections::HashMap;

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// `NLR`'s port `p` connects to a declared net `n` whose own nodeset value
/// is `{nodeset}` — the only knob that differs between the two fixtures
/// below.
fn tristable_source(nodeset: f64) -> String {
    format!(
        "
module NLR (inout electrical p);
    analog begin
        I(p, gnd) <+ V(p) * V(p) * V(p) - V(p);
    end
endmodule

module Top ();
    electrical n = {nodeset};
    NLR nlr (.p(n));
endmodule
"
    )
}

fn compile(src: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy())
        .expect("tristable circuit elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

/// A declared nodeset near `+1` must converge to the `+1` branch, not the
/// `-1` or `0` branch.
#[test]
fn declared_nodeset_positive_selects_the_positive_branch() {
    let mut session = compile(&tristable_source(0.9));
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let v = op.v("n").expect("v(n)");
    assert!((v - 1.0).abs() < 0.05, "declared nodeset=0.9 must land near v=+1, got {v}");
}

/// The mirror case: a declared nodeset near `-1` must converge to the `-1`
/// branch — same circuit, only the declared nodeset differs, proving the
/// branch selection tracks the nodeset rather than some fixed default.
#[test]
fn declared_nodeset_negative_selects_the_negative_branch() {
    let mut session = compile(&tristable_source(-0.9));
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let v = op.v("n").expect("v(n)");
    assert!((v + 1.0).abs() < 0.05, "declared nodeset=-0.9 must land near v=-1, got {v}");
}

/// A host-supplied `OpConfig.nodeset` entry for the same net overrides the
/// declared one: the module declares `-0.9` (would select the `-1` branch
/// per the previous test) but the host passes `+0.9` for `n` — the result
/// must land on the `+1` branch, proving host precedence over the
/// declaration.
#[test]
fn host_supplied_nodeset_overrides_the_declared_one() {
    let mut session = compile(&tristable_source(-0.9));
    let host_nodeset = HashMap::from([("n".to_string(), 0.9_f64)]);
    let op = session
        .op(&SolverConfig::default(), Some(&host_nodeset))
        .expect("op solves");
    let v = op.v("n").expect("v(n)");
    assert!(
        (v - 1.0).abs() < 0.05,
        "host nodeset=0.9 must win over the declared -0.9 and land near v=+1, got {v}"
    );
}
