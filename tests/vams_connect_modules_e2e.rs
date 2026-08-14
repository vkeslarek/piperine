//! T52 (VAMS-20) — "mixed analog/digital fixture simulates end-to-end"
//! done-when: a single `.vams` design containing an analog subcircuit
//! (an ideal source, a resistor, and an explicitly-instantiated
//! `connectmodule` bridge — LRM ch. 7's own module kind, elaborated
//! uniformly with `module`/`macromodule` per `crates/piperine-lang/src/
//! elab/lower/connect.rs`'s survey finding) *and* a digital register
//! (the same clocked-counter pattern `tests/vams_vertical_slice.rs`'s
//! T33 already proved), compiled into **one** circuit and simulated
//! together on the unmodified native solver.
//!
//! Placement note (MD-28): this is a cross-crate host-surface proof
//! (`piperine_lang` → `piperine_codegen` → `piperine_solver`), so it lives
//! in root `tests/`, alongside `vams_vertical_slice.rs` — not inside
//! `piperine-lang`'s own `tests/`, which cannot reach codegen/solver.
//! `crates/piperine-lang/tests/vams_connect_modules.rs` covers the
//! elaboration-only claims (T52's stated `Where`); this file is the
//! necessary scope-widening for the "simulates end-to-end" claim, in the
//! same spirit T46's own progress-log entry documented for its CLI fixes.
//!
//! **Scope note on "mixed"**: the connectmodule here bridges two
//! same-hierarchy `electrical` nets (an identity/zero-volt bridge) rather
//! than an automatically-inserted `electrical<->logic` bridge — true
//! automatic cross-domain insertion is the SPEC_DEVIATION documented in
//! `connect.rs`'s own module doc (blocked upstream by
//! `ResolveDisciplines`'s current hard-fail-on-mismatch behavior). What
//! this test proves instead, honestly: a `connectmodule`-kind instance
//! coexists in the same compiled circuit as genuine analog *and* digital
//! content, and all three simulate together without any special-casing
//! anywhere in the pipeline.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

const MIXED_FIXTURE: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

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

    connectmodule wire_bridge (inout electrical a, inout electrical b);
        analog begin
            V(a, b) <+ 0;
        end
    endmodule

    module counter (input logic clk, output logic q);
        integer count;
        always @(posedge clk) count <= count + 1;
    endmodule

    module top ();
        electrical gnd, vin, vmid;
        logic clk_net, q_net;
        vsrc #(.dc(1.0)) v1 (vin, gnd);
        resistor #(.r(1.0e3)) r1 (vin, vmid);
        wire_bridge b1 (vmid, gnd);
        counter c1 (clk_net, q_net);
    endmodule
"#;

#[test]
fn mixed_analog_digital_fixture_with_a_connectmodule_bridge_simulates_end_to_end() {
    let design = parse_and_elaborate(MIXED_FIXTURE, &piperine_lang::SourceMap::dummy())
        .expect("a mixed analog/digital design with an explicit connectmodule instance elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").expect("CircuitCompiler builds one circuit spanning both domains");

    // Analog half: the connectmodule bridges vmid to gnd at 0V, so the
    // resistor sees the full source voltage across it and vmid settles at
    // ground potential (the bridge, not the resistor, sets the DC node).
    let dc_result = circuit.dc(Context::default()).expect("dc analysis starts").solve().expect("dc converges");
    let vin = info.nets.get("vin").expect("net `vin`");
    let vmid = info.nets.get("vmid").expect("net `vmid`");
    let v_in = dc_result.get_node(vin).expect("v(vin)");
    let v_mid = dc_result.get_node(vmid).expect("v(vmid)");
    assert!((v_in - 1.0).abs() < 1e-6, "v(vin) = 1.0 V (the ideal source), got {v_in}");
    assert!(v_mid.abs() < 1e-6, "the connectmodule bridge holds v(vmid) at ground, got {v_mid}");

    // Digital half, in the SAME compiled circuit: force two posedges on
    // clk_net and confirm the counter's register increments — the same
    // proof T33 (`tests/vams_vertical_slice.rs`) established, now
    // alongside live analog content and a connectmodule instance rather
    // than in isolation.
    let counter_dev_index = circuit
        .all_devices()
        .iter()
        .position(|d| d.name() == "c1")
        .expect("the `c1` counter instance is a device in the built circuit");
    let count_before =
        circuit.all_devices()[counter_dev_index].digital_hidden_snapshot().expect("hidden register state").0[0];

    let clk_idx = *info.digital_nets.get("clk_net").expect("`clk_net` mapped to a digital net");
    const EDGES: u32 = 2;
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

    let opts = TransientAnalysisOptions::new(4.0, 0.05);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    let result = tran.solve().expect("the mixed analog/digital circuit's transient run completes");
    assert!(result.len() > 5, "the run recorded a meaningful number of steps");

    let count_after =
        circuit.all_devices()[counter_dev_index].digital_hidden_snapshot().expect("hidden register state").0[0];
    assert_eq!(
        count_after - count_before,
        i64::from(EDGES),
        "the digital register still increments correctly alongside live analog content \
         (before={count_before}, after={count_after})"
    );

    // And the analog node is still tracked correctly through the same
    // transient run that also advanced the digital register.
    let last_vmid = result.iter().last().expect("at least one step").get_node(vmid).expect("v(vmid) recorded");
    assert!(last_vmid.abs() < 1e-3, "v(vmid) stays near ground through the transient run, got {last_vmid}");
}
