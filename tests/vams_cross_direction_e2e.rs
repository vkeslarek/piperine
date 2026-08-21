//! LRM-COVERS: 5.10.3.1
//! LRMC-C5-02 (`.specs/features/lrm-conformance-closure/`, T22):
//! `@(cross(expr, dir))`'s direction argument (§5.10.3.1) actually
//! restricts which edges fire, instead of every crossing always firing
//! regardless of `dir` — proven on the native solver with a linear ramp
//! (a single, unambiguous crossing per run) crossed by each direction.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const FIXTURE: &str = r#"
    module ramp_up (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ -1.0 + 2.0 * $abstime;
        end
    endmodule

    module ramp_down (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ 1.0 - 2.0 * $abstime;
        end
    endmodule

    module counter_rising (inout electrical p, inout electrical n, inout electrical mon);
        real count;
        analog begin
            @(cross(V(p, n), 1))
                count = count + 1.0;
            V(mon) <+ count;
        end
    endmodule

    module counter_falling (inout electrical p, inout electrical n, inout electrical mon);
        real count;
        analog begin
            @(cross(V(p, n), -1))
                count = count + 1.0;
            V(mon) <+ count;
        end
    endmodule

    module counter_either (inout electrical p, inout electrical n, inout electrical mon);
        real count;
        analog begin
            @(cross(V(p, n)))
                count = count + 1.0;
            V(mon) <+ count;
        end
    endmodule

    module top_rising_detects_rising ();
        electrical gnd, vp, vmon;
        ramp_up r1 (vp, gnd);
        counter_rising c1 (vp, gnd, vmon);
    endmodule

    module top_falling_ignores_rising ();
        electrical gnd, vp, vmon;
        ramp_up r1 (vp, gnd);
        counter_falling c1 (vp, gnd, vmon);
    endmodule

    module top_falling_detects_falling ();
        electrical gnd, vp, vmon;
        ramp_down r1 (vp, gnd);
        counter_falling c1 (vp, gnd, vmon);
    endmodule

    module top_rising_ignores_falling ();
        electrical gnd, vp, vmon;
        ramp_down r1 (vp, gnd);
        counter_rising c1 (vp, gnd, vmon);
    endmodule

    module top_either_detects_rising ();
        electrical gnd, vp, vmon;
        ramp_up r1 (vp, gnd);
        counter_either c1 (vp, gnd, vmon);
    endmodule

    module top_either_detects_falling ();
        electrical gnd, vp, vmon;
        ramp_down r1 (vp, gnd);
        counter_either c1 (vp, gnd, vmon);
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

#[test]
fn rising_direction_fires_on_a_rising_crossing() {
    assert!((final_count("top_rising_detects_rising") - 1.0).abs() < 1e-6);
}

#[test]
fn falling_direction_does_not_fire_on_a_rising_crossing() {
    assert!(final_count("top_falling_ignores_rising").abs() < 1e-6);
}

#[test]
fn falling_direction_fires_on_a_falling_crossing() {
    assert!((final_count("top_falling_detects_falling") - 1.0).abs() < 1e-6);
}

#[test]
fn rising_direction_does_not_fire_on_a_falling_crossing() {
    assert!(final_count("top_rising_ignores_falling").abs() < 1e-6);
}

#[test]
fn omitted_direction_defaults_to_either_and_fires_on_a_rising_crossing() {
    assert!((final_count("top_either_detects_rising") - 1.0).abs() < 1e-6);
}

#[test]
fn omitted_direction_defaults_to_either_and_fires_on_a_falling_crossing() {
    assert!((final_count("top_either_detects_falling") - 1.0).abs() < 1e-6);
}
