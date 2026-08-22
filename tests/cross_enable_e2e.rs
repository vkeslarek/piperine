//! LRM-COVERS: 5.10.3.1
//! LRMW2-33 (`lrm-conformance-wave2`, T41): `cross(expr, dir, time_tol,
//! expr_tol, enable)`'s `enable` argument used to be silently discarded —
//! `cross()` always generated an event regardless. §5.10.3.1: "If enable
//! argument is specified and it is zero, then cross() is inactive, meaning
//! that it does not generate an event at threshold crossings and does not
//! act to control the timestep." The LRM's own disabled-sample-and-hold
//! idiom (§5.10.3.1's second example, `en === 1'b1`) motivates this test's
//! fixture shape: a module parameter directly gates `enable`.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const FIXTURE: &str = r#"
    module ramp_up (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ -1.0 + 2.0 * $abstime;
        end
    endmodule

    module counter (inout electrical p, inout electrical n, inout electrical mon);
        parameter real en = 1.0;
        real count;
        analog begin
            @(cross(V(p, n), 1, 0, 0, en))
                count = count + 1.0;
            V(mon) <+ count;
        end
    endmodule

    module top_enabled ();
        electrical gnd, vp, vmon;
        ramp_up r1 (vp, gnd);
        counter #(.en(1.0)) c1 (vp, gnd, vmon);
    endmodule

    module top_disabled ();
        electrical gnd, vp, vmon;
        ramp_up r1 (vp, gnd);
        counter #(.en(0.0)) c1 (vp, gnd, vmon);
    endmodule
"#;

/// Runs `top` over one second (the ramp's own zero-crossing window) and
/// returns the final `count`, read off `vmon`'s node voltage.
fn final_count(top: &str) -> f64 {
    let design = parse_and_elaborate(FIXTURE, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped(top).unwrap_or_else(|e| panic!("{top} builds: {e:?}"));
    let opts = TransientAnalysisOptions::new(1.0, 0.01);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    let result = tran.solve().unwrap_or_else(|e| panic!("{top} transient solves: {e:?}"));
    let vmon = info.nets.get("vmon").expect("net `vmon`");
    result.last().and_then(|step| step.get_node(vmon)).expect("v(vmon) at the last step")
}

/// AC14 discrimination: `enable = 1` (the default LRM behavior) must fire
/// on the rising crossing; `enable = 0` must suppress it entirely — an
/// ignored `enable` argument would give the same count either way.
#[test]
fn enabled_cross_fires_on_the_rising_crossing() {
    assert!((final_count("top_enabled") - 1.0).abs() < 1e-6, "enable=1 must fire once");
}

#[test]
fn disabled_cross_never_fires() {
    assert!(final_count("top_disabled").abs() < 1e-6, "enable=0 must suppress the event entirely");
}
