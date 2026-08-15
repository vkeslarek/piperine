//! LRMC-C5-04 runtime half (`.specs/features/lrm-conformance-closure/`,
//! T26): `-> event_name;` actually wakes `@(event_name)` sensitivity
//! elsewhere, in the right order, and works across the analog/digital
//! domain boundary — §5.10.4's own text: "Named events can be triggered
//! from always and initial blocks, or from an analog event statement,"
//! synchronizing "two or more concurrently active processes." No literal
//! worked example exists in §5.10.4 itself; this fixture is the smallest
//! faithful instance of the clock-generator idiom the clause narrates,
//! driven from the digital domain (forced `posedge` events, the same
//! technique `tests/vams_vertical_slice.rs`'s T33 proof uses since VAMS
//! has no `.vams`-authored clock oscillator yet) into an analog `@(e)`
//! sensitivity.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const FIXTURE: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module dut (input logic clk, inout electrical mon);
        event e;
        real count;
        always @(posedge clk)
            -> e;
        analog begin
            @(e)
                count = count + 1.0;
            V(mon) <+ count;
        end
    endmodule

    module top ();
        logic clk_net;
        electrical mon_net;
        dut d1 (clk_net, mon_net);
    endmodule
"#;

#[test]
fn digital_trigger_wakes_an_analog_named_event_sensitivity_in_order() {
    let design = parse_and_elaborate(FIXTURE, &piperine_lang::SourceMap::dummy())
        .expect("the cross-domain named-event fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("top builds");

    let clk_idx = *info.digital_nets.get("clk_net").expect("`clk_net` mapped to a digital net");
    const EDGES: u32 = 4;
    for i in 0..EDGES {
        let base = (2 * i) as f64;
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 0.5,
            net: DigitalNet(clk_idx),
            value: LogicValue::Zero,
            source: usize::MAX,
            seq: (2 * i) as u64,
        });
        circuit.digital_state.schedule(DigitalEvent {
            time: base + 1.0,
            net: DigitalNet(clk_idx),
            value: LogicValue::One,
            source: usize::MAX,
            seq: (2 * i + 1) as u64,
        });
    }

    let opts = TransientAnalysisOptions::new(8.0, 0.05);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    let result = tran.solve().expect("the cross-domain transient run completes");

    let vmon = info.nets.get("mon_net").expect("net `mon_net`");
    let count = result.last().and_then(|s| s.get_node(vmon)).expect("v(mon) at the last step");

    // Each forced posedge triggers `-> e;` in the digital domain; the
    // analog `@(e)` sensitivity must wake and increment `count` --
    // proving the digital-domain trigger actually crossed into the
    // analog domain (not just that the digital side ran; before this
    // fix `count` stayed exactly 0 for the entire run).
    //
    // Not asserted as an exact `count == EDGES`: the digital scheduler's
    // comb-style (unclocked `always @(x)`) re-evaluation can fire more
    // than once per logical transition for *any* level-sensitive var
    // chain -- confirmed independently with a fixture using no named
    // events at all (`always @(posedge clk) x = 1 - x; always @(x)
    // count = count + 1;` shows the identical over-count). That
    // characteristic is pre-existing and out of LRMC-C5-04's scope; this
    // assertion only requires every real trigger to have registered (a
    // generous upper bound rules out the previous *silent-zero* failure
    // this task fixes, not a live regression).
    assert!(
        count >= f64::from(EDGES) && count <= f64::from(EDGES) * 2.0,
        "count must reflect the {EDGES} digital-domain triggers reaching the analog @(e) \
         sensitivity, got {count}"
    );

    // Monotonic: the analog @(e) action only ever increments `count`, so a
    // regression that re-fires wildly (e.g. every accepted step) would
    // blow well past the upper bound already asserted above, and one that
    // silently drops the cross-domain link would leave it at 0 -- both
    // already caught. This additionally confirms it never *decreases*,
    // i.e. the fired count monotonically accumulates as the run
    // progresses, not just at the final sample.
    let mut prev = 0.0;
    for step in result.iter() {
        let v = step.get_node(vmon).unwrap_or(0.0);
        assert!(v >= prev - 1e-9, "count must never decrease (t={}, {v} < {prev})", step.time());
        prev = v;
    }
}

/// Same-domain (analog-only) case: one `-> e;` chains into another
/// `@(e)`-sensitive action within a single analog block, over a deterministic
/// single crossing -- exact ordering, no digital-scheduler re-evaluation
/// ambiguity, so this asserts the precise count the cross-domain test above
/// only bounds.
const ANALOG_ONLY_FIXTURE: &str = r#"
    module vsrc (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ $abstime;
        end
    endmodule

    module dut (inout electrical p, inout electrical n, inout electrical mon);
        event e;
        real count;
        analog begin
            @(cross(V(p, n) - 0.5, 1))
                -> e;
            @(e)
                count = count + 1.0;
            V(mon) <+ count;
        end
    endmodule

    module top ();
        electrical gnd, vp, vmon;
        vsrc v1 (vp, gnd);
        dut d1 (vp, gnd, vmon);
    endmodule
"#;

#[test]
fn analog_only_event_trigger_wakes_its_own_sensitivity_exactly_once() {
    let design = parse_and_elaborate(ANALOG_ONLY_FIXTURE, &piperine_lang::SourceMap::dummy())
        .expect("the analog-only named-event fixture elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("top builds");
    let opts = TransientAnalysisOptions::new(2.0, 0.01);
    let mut tran = circuit.transient(opts, Context::default()).expect("transient starts");
    let result = tran.solve().expect("transient solves");
    let vmon = info.nets.get("vmon").expect("net `vmon`");
    let count = result.last().and_then(|s| s.get_node(vmon)).expect("v(vmon) at the last step");
    assert!(
        (count - 1.0).abs() < 1e-6,
        "the single crossing at t=0.5 must trigger -> e; exactly once, waking @(e) exactly \
         once, got count={count}"
    );
}
