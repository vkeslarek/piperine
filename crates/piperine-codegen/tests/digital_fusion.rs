//! SC-13 — fused combinational digital network integration.
//!
//! `CircuitCompiler` pulls connected pure-combinational digital cones into
//! single `DigitalNetwork` elements (one fused JIT call per cone) with a
//! per-device fallback for clocked / analog-sampling members. These tests
//! prove (a) fusion is *active* (instrumentation, not timing) and (b) the
//! fused path is bit-identical to the per-device path — full digital state
//! vectors compared at every recorded transient step.

use piperine_codegen::{CircuitCompiler, CircuitBuildInfo};
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::{CircuitInstance, Context, LogicValue, TransientAnalysisOptions};

const PRELUDE: &str = "
    discipline Bit;
        domain discrete;
    enddiscipline
    module BitDriver ( output Bit q );
        parameter real level = 0.0;
        integer b;
        always @(*) begin
            b = level > 0.5;
            q = b;
        end
    endmodule
    module Inv ( input Bit a, output Bit y );
        integer t;
        always @(*) begin
            t = !a;
            y = t;
        end
    endmodule
    module ClockSrc ( inout electrical clk_a, inout electrical gnd );
        parameter real period = 1.0e-6;
        analog begin: body
            real ph;
            ph = $abstime - period * floor($abstime / period);
            V(clk_a, gnd) <+ (ph > period * 0.5) ? 1.0 : 0.0;
        end
    endmodule
    module Comparator ( input electrical a, input electrical n, output Bit y );
        always @(*) y = V(a, n) > 0.5;
    endmodule
    module Dff ( input Bit clk, input Bit d, output Bit q );
        integer st;
        always @(*) q = st;
        always @(posedge clk) st = d;
    endmodule
";

/// Build `top` with fusion on/off, run a transient, and return the full
/// digital state vector at every recorded step plus the final nets.
/// (`TransientSolver::new` performs the single `init_digital` — a second
/// manual init corrupts the per-device event scheduler's state.)
fn run(src: &str, top: &str, fuse: bool, stop: f64) -> (CircuitInstance, CircuitBuildInfo, Vec<Vec<Option<LogicValue>>>) {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.fuse_digital_cones = fuse;
    let (mut circuit, info) = compiler.build_circuit_mapped(top).expect("builds");
    let options = TransientAnalysisOptions::new(stop, stop / 100.0);
    let result = circuit.transient(options, Context::default()).unwrap().solve().unwrap();
    let net_count = circuit.digital_state.nets.len();
    let steps: Vec<Vec<Option<LogicValue>>> = result
        .iter()
        .map(|s| (0..net_count).map(|i| s.digital(i)).collect())
        .collect();
    (circuit, info, steps)
}

/// The fused and per-device runs must agree on the full digital state at
/// every recorded step and at the end of the run.
fn assert_bit_identical(src: &str, top: &str, stop: f64) -> CircuitBuildInfo {
    let (c_on, info, steps_on) = run(src, top, true, stop);
    let (c_off, _, steps_off) = run(src, top, false, stop);
    assert_eq!(
        steps_on.len(),
        steps_off.len(),
        "step counts diverge ({} vs {})",
        steps_on.len(),
        steps_off.len()
    );
    for (k, (a, b)) in steps_on.iter().zip(&steps_off).enumerate() {
        assert_eq!(a, b, "digital state diverges at recorded step {k}");
    }
    assert_eq!(
        c_on.digital_state.nets, c_off.digital_state.nets,
        "final digital nets diverge"
    );
    info
}

/// SC-13 — fusion-active proof: a 4-bit ripple adder (8 drivers + 4
/// FullAdders, one multi-module combinational cone) collapses to a single
/// fused `DigitalNetwork` element, and the computed sum is correct.
#[test]
fn fusion_active_on_ripple_adder_cone() {
    let src = format!("{PRELUDE}
        module FullAdder ( input Bit a, input Bit b, input Bit cin, output Bit sum, output Bit cout );
            integer s, c;
            always @(*) begin
                s = (a != b) != cin;
                c = (a && b) || (cin && (a != b));
                sum = s; cout = c;
            end
        endmodule
        module Adder4 ();
            Bit a0, a1, a2, a3;
            Bit b0, b1, b2, b3;
            Bit zero;
            Bit s0, s1, s2, s3, s4;
            Bit c1, c2, c3;
            BitDriver #(.level(0.0)) dz (.q(zero));
            BitDriver #(.level(1.0)) da0 (.q(a0));
            BitDriver #(.level(0.0)) da1 (.q(a1));
            BitDriver #(.level(1.0)) da2 (.q(a2));
            BitDriver #(.level(0.0)) da3 (.q(a3));
            BitDriver #(.level(1.0)) db0 (.q(b0));
            BitDriver #(.level(1.0)) db1 (.q(b1));
            BitDriver #(.level(0.0)) db2 (.q(b2));
            BitDriver #(.level(0.0)) db3 (.q(b3));
            FullAdder fa0 (.a(a0), .b(b0), .cin(zero), .sum(s0), .cout(c1));
            FullAdder fa1 (.a(a1), .b(b1), .cin(c1), .sum(s1), .cout(c2));
            FullAdder fa2 (.a(a2), .b(b2), .cin(c2), .sum(s2), .cout(c3));
            FullAdder fa3 (.a(a3), .b(b3), .cin(c3), .sum(s3), .cout(s4));
        endmodule
    ");
    let (circuit, info, _) = run(&src, "Adder4", true, 1e-6);
    assert_eq!(info.fused_networks, 1, "the whole cone fuses into one network");
    assert_eq!(circuit.all_devices().len(), 1, "13 devices replaced by one fused network");
    // a = 0101, b = 0011 → s = 01000 (s0..s4 low-first: 0,0,0,1,0).
    let bit = |name: &str| {
        let idx = info.digital_nets[name];
        circuit.digital_state.nets[idx]
    };
    assert_eq!(bit("s0"), LogicValue::Zero);
    assert_eq!(bit("s1"), LogicValue::Zero);
    assert_eq!(bit("s2"), LogicValue::Zero);
    assert_eq!(bit("s3"), LogicValue::One);
    assert_eq!(bit("s4"), LogicValue::Zero, "5 + 3 = 8");
}

/// SC-13 — bit-equality differential on a static combinational cone
/// (multi-module ripple adder): every recorded step's digital vector and
/// the final nets are identical fused vs per-device.
#[test]
fn fusion_bit_identical_static_adder() {
    let src = format!("{PRELUDE}
        module FullAdder ( input Bit a, input Bit b, input Bit cin, output Bit sum, output Bit cout );
            integer s, c;
            always @(*) begin
                s = (a != b) != cin;
                c = (a && b) || (cin && (a != b));
                sum = s; cout = c;
            end
        endmodule
        module Adder4 ();
            Bit a0, a1, a2, a3;
            Bit b0, b1, b2, b3;
            Bit zero;
            Bit s0, s1, s2, s3, s4;
            Bit c1, c2, c3;
            BitDriver #(.level(0.0)) dz (.q(zero));
            BitDriver #(.level(1.0)) da0 (.q(a0));
            BitDriver #(.level(1.0)) da1 (.q(a1));
            BitDriver #(.level(0.0)) da2 (.q(a2));
            BitDriver #(.level(1.0)) da3 (.q(a3));
            BitDriver #(.level(1.0)) db0 (.q(b0));
            BitDriver #(.level(1.0)) db1 (.q(b1));
            BitDriver #(.level(0.0)) db2 (.q(b2));
            BitDriver #(.level(0.0)) db3 (.q(b3));
            FullAdder fa0 (.a(a0), .b(b0), .cin(zero), .sum(s0), .cout(c1));
            FullAdder fa1 (.a(a1), .b(b1), .cin(c1), .sum(s1), .cout(c2));
            FullAdder fa2 (.a(a2), .b(b2), .cin(c2), .sum(s2), .cout(c3));
            FullAdder fa3 (.a(a3), .b(b3), .cin(c3), .sum(s3), .cout(s4));
        endmodule
    ");
    let info = assert_bit_identical(&src, "Adder4", 2e-6);
    assert_eq!(info.fused_networks, 1);
}

/// SC-13 — clock-driven cone: the comparator samples analog (per-device
/// fallback) while the two-inverter cone fuses. Transient propagation over
/// clock edges stays bit-identical.
#[test]
fn fusion_bit_identical_clocked_cone() {
    let src = format!("{PRELUDE}
        module Top ();
            electrical gnd, clk_a;
            Bit clk, y1, y2;
            ClockSrc #(.period(1.0e-6)) clkgen (.clk_a(clk_a), .gnd(gnd));
            Comparator ccmp (.a(clk_a), .n(gnd), .y(clk));
            Inv i1 (.a(clk), .y(y1));
            Inv i2 (.a(y1), .y(y2));
        endmodule
    ");
    let info = assert_bit_identical(&src, "Top", 4e-6);
    assert_eq!(info.fused_networks, 1, "the inverter pair fuses; the comparator stays per-device");
}

/// SC-13 — cross-module NBA with a fused cone between flops: the register
/// pipeline (clocked, per-device) samples pre-edge values while the
/// inter-stage inverter cone settles combinationally — fused and per-device
/// runs agree at every recorded step.
#[test]
fn fusion_bit_identical_nba_pipeline() {
    let src = format!("{PRELUDE}
        module Top ();
            electrical gnd, clk_a, high_a;
            Bit clk, din;
            Bit q0, q0n, q0nn, q1;
            ClockSrc #(.period(1.0e-6)) clkgen (.clk_a(clk_a), .gnd(gnd));
            ClockSrc #(.period(2.7e-6)) dgen (.clk_a(high_a), .gnd(gnd));
            Comparator ccmp (.a(clk_a), .n(gnd), .y(clk));
            Comparator dcmp (.a(high_a), .n(gnd), .y(din));
            Dff f0 (.clk(clk), .d(din), .q(q0));
            Inv i1 (.a(q0), .y(q0n));
            Inv i2 (.a(q0n), .y(q0nn));
            Dff f1 (.clk(clk), .d(q0nn), .q(q1));
        endmodule
    ");
    let info = assert_bit_identical(&src, "Top", 6e-6);
    assert_eq!(info.fused_networks, 1, "the inter-stage inverter pair fuses; flops stay per-device");
}
