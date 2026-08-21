//! T16/LRMW2-12 (§5.10.3): `@(expr)` level events on an arbitrary
//! expression (not a bare identifier and not a recognized event function
//! call — a comparison, an access-function read, ...) fire when the
//! sampled value changes, instead of the body vanishing. Before this fix
//! `flatten/analog.rs`'s catch-all arm for this expression shape returned
//! `Ok(vec![])` — the event compiled with zero triggers and its body never
//! ran.
//!
//! LRM-COVERS: 5.10.3

use piperine_codegen::resolve::lower_bodies;
use piperine_codegen::{CircuitCompiler, CodegenError};
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// `V(p, gnd)` ramps linearly from 0V to 5V over the 1s run
/// (`5.0 * $abstime`); `@(V(p, gnd) > 2.5) count = count + 1.0;` watches
/// the *boolean* value of that comparison, which is 0.0 for the first half
/// of the run and 1.0 for the second — exactly one change, at t=0.5s.
/// `count` is mirrored onto `mon` so the result is host-observable.
const RAMP_LEVEL_EVENT: &str = r#"
module dut (inout electrical p, inout electrical gnd, inout electrical mon);
    real count;
    analog begin
        V(p, gnd) <+ 5.0 * $abstime;
        @(V(p, gnd) > 2.5)
            count = count + 1.0;
        V(mon, gnd) <+ count;
    end
endmodule

module top ();
    electrical p, gnd, mon;
    dut d1 (p, gnd, mon);
endmodule
"#;

/// `@('{1.0, 2.0}) ...` — an assignment-pattern literal is not a legal
/// scalar value to watch; it must fail loud (naming the construct),
/// never silently compile with a dropped event.
const UNSUPPORTED_LEVEL_EVENT: &str = r#"
module dut (inout electrical p, inout electrical gnd);
    real count;
    analog begin
        V(p, gnd) <+ 1.0;
        @('{1.0, 2.0})
            count = count + 1.0;
    end
endmodule

module top ();
    electrical p, gnd;
    dut d1 (p, gnd);
endmodule
"#;

/// `@(expr)` on a comparison expression runs its body exactly once, when
/// the predicate's value changes from false (0.0) to true (1.0) — not
/// zero times (the pre-fix silent-drop) and not once per accepted
/// timestep (which would overshoot 1).
#[test]
fn level_event_on_a_comparison_expression_fires_on_change() {
    let design = parse_and_elaborate(RAMP_LEVEL_EVENT, &piperine_lang::SourceMap::dummy())
        .expect("the ramp/level-event fixture elaborates");
    let bodies = lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("top builds");

    let opts = TransientAnalysisOptions::new(1.0, 0.01);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    let result = tran.solve().expect("transient completes");

    let mon = info.nets.get("mon").expect("net `mon`");
    let count = result.last().and_then(|s| s.get_node(mon)).expect("v(mon) at the last step");
    assert_eq!(
        count, 1.0,
        "the crossing at t=0.5s must fire the event exactly once, got count={count}"
    );
}

/// An event expression emission genuinely cannot compile (an
/// assignment-pattern literal) still fails loud, naming the construct —
/// not a silently-dropped event.
#[test]
fn unsupported_level_event_expression_fails_loud_naming_the_construct() {
    let design = parse_and_elaborate(UNSUPPORTED_LEVEL_EVENT, &piperine_lang::SourceMap::dummy())
        .expect("the fixture elaborates (the failure is at codegen, not elaboration)");
    let bodies = lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let err = match compiler.build_circuit_mapped("top") {
        Ok(_) => panic!("an assignment-pattern event expression must not silently compile"),
        Err(e) => e,
    };
    match err {
        CodegenError::Invalid(msg) | CodegenError::Unsupported(msg) => {
            assert!(
                msg.contains("assignment pattern"),
                "error must name the unsupported construct: {msg}"
            );
        }
        other => panic!("expected CodegenError::Unsupported (wrapped or not), got {other}"),
    }
}
