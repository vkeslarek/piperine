//! LRMC-C5-03 wiring half (`.specs/features/lrm-conformance-closure/`,
//! T24): an analog block sensitive to `@(absdelta(expr, delta))` actually
//! re-evaluates at the points §5.10.3.4 specifies (initialization, plus
//! whenever `expr` has moved by more than `delta` since the last event) —
//! not silently swallowed by the `_ => vec![]` fallthrough that used to
//! discard every unrecognized/unwired analog event function.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const FIXTURE: &str = r#"
    module ramp (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ $abstime;
        end
    endmodule

    module sampler (inout electrical p, inout electrical n, inout electrical mon, inout electrical cnt);
        real sampled;
        real count;
        analog begin
            @(absdelta(V(p, n), 0.5)) begin
                sampled = V(p, n);
                count = count + 1.0;
            end
            V(mon) <+ sampled;
            V(cnt) <+ count;
        end
    endmodule

    module top ();
        electrical gnd, vp, vmon, vcnt;
        ramp r1 (vp, gnd);
        sampler s1 (vp, gnd, vmon, vcnt);
    endmodule
"#;

#[test]
fn absdelta_event_re_evaluates_at_the_lrm_specified_points() {
    let design =
        parse_and_elaborate(FIXTURE, &piperine_lang::SourceMap::dummy()).expect("the fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("top builds");

    let opts = TransientAnalysisOptions::new(4.0, 0.01);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    let result = tran.solve().expect("transient solves");

    let vmon = info.nets.get("vmon").expect("net `vmon`");
    let vcnt = info.nets.get("vcnt").expect("net `vcnt`");
    let last = result.last().expect("at least one recorded step");
    let sampled = last.get_node(vmon).expect("v(vmon)");
    let count = last.get_node(vcnt).expect("v(vcnt)");

    // Over a 4s ramp with delta=0.5, the event must fire many times, not
    // just the single initialization event -- proving it re-evaluates on
    // the *value-change* rule, not only once.
    assert!(count > 3.0, "absdelta must fire repeatedly as the ramp advances past delta each time, got count={count}");
    // The exact count is timestep-grid-dependent (tier-1 reactive
    // detection, no sub-step interpolation -- that refinement is out of
    // this task's scope), but it cannot exceed roughly one fire per delta
    // plus the init fire over the 4s window.
    assert!(count < 15.0, "an event firing every accepted step would be a distinct (silently-fired) bug, got count={count}");
    // `sampled` must track the ramp near the end of the run, not be stuck
    // at its very first (t=0) value -- proving later events actually ran.
    assert!(sampled > 2.5, "sampled must have advanced with the ramp via later absdelta events, got sampled={sampled}");
}
