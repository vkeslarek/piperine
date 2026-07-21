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
    discipline Electrical { potential v : Real; flow i : Real; }
    discipline Bit { storage Boolean; }
    mod BitDriver ( output q : Bit ) { param level : Real = 0.0; var b : Bit = 0; }
    digital BitDriver { b = level > 0.5; q <- b; }
    mod Inv ( input a : Bit, output y : Bit ) { var t : Bit = 0; }
    digital Inv { t = !a; y <- t; }
    mod ClockSrc ( inout clk_a : Electrical, inout gnd : Electrical ) { param period : Real = 1.0e-6; }
    analog ClockSrc {
        var ph : Real = $abstime - period * floor($abstime / period);
        V(clk_a, gnd) <- if (ph > period * 0.5) { 1.0 } else { 0.0 };
    }
    mod Comparator ( input a : Electrical, input n : Electrical, output y : Bit ) { }
    digital Comparator { y <- V(a, n) > 0.5; }
    mod Dff ( input clk : Bit, input d : Bit, output q : Bit ) { var st : Bit = 0; }
    digital Dff { q <- st; @ (posedge(clk)) { st = d; } }
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
    let options = TransientAnalysisOptions::new(stop.into(), (stop / 100.0).into());
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
        mod FullAdder ( input a : Bit, input b : Bit, input cin : Bit, output sum : Bit, output cout : Bit ) {{
            var s : Bit = 0; var c : Bit = 0;
        }}
        digital FullAdder {{
            s = (a != b) != cin;
            c = (a && b) || (cin && (a != b));
            sum <- s; cout <- c;
        }}
        mod Adder4 () {{
            wire a0 : Bit; wire a1 : Bit; wire a2 : Bit; wire a3 : Bit;
            wire b0 : Bit; wire b1 : Bit; wire b2 : Bit; wire b3 : Bit;
            wire zero : Bit;
            wire s0 : Bit; wire s1 : Bit; wire s2 : Bit; wire s3 : Bit; wire s4 : Bit;
            wire c1 : Bit; wire c2 : Bit; wire c3 : Bit;
            dz : BitDriver ( .q = zero ) {{ .level = 0.0 }};
            da0 : BitDriver ( .q = a0 ) {{ .level = 1.0 }};
            da1 : BitDriver ( .q = a1 ) {{ .level = 0.0 }};
            da2 : BitDriver ( .q = a2 ) {{ .level = 1.0 }};
            da3 : BitDriver ( .q = a3 ) {{ .level = 0.0 }};
            db0 : BitDriver ( .q = b0 ) {{ .level = 1.0 }};
            db1 : BitDriver ( .q = b1 ) {{ .level = 1.0 }};
            db2 : BitDriver ( .q = b2 ) {{ .level = 0.0 }};
            db3 : BitDriver ( .q = b3 ) {{ .level = 0.0 }};
            fa0 : FullAdder ( .a = a0, .b = b0, .cin = zero, .sum = s0, .cout = c1 );
            fa1 : FullAdder ( .a = a1, .b = b1, .cin = c1, .sum = s1, .cout = c2 );
            fa2 : FullAdder ( .a = a2, .b = b2, .cin = c2, .sum = s2, .cout = c3 );
            fa3 : FullAdder ( .a = a3, .b = b3, .cin = c3, .sum = s3, .cout = s4 );
        }}
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
        mod FullAdder ( input a : Bit, input b : Bit, input cin : Bit, output sum : Bit, output cout : Bit ) {{
            var s : Bit = 0; var c : Bit = 0;
        }}
        digital FullAdder {{
            s = (a != b) != cin;
            c = (a && b) || (cin && (a != b));
            sum <- s; cout <- c;
        }}
        mod Adder4 () {{
            wire a0 : Bit; wire a1 : Bit; wire a2 : Bit; wire a3 : Bit;
            wire b0 : Bit; wire b1 : Bit; wire b2 : Bit; wire b3 : Bit;
            wire zero : Bit;
            wire s0 : Bit; wire s1 : Bit; wire s2 : Bit; wire s3 : Bit; wire s4 : Bit;
            wire c1 : Bit; wire c2 : Bit; wire c3 : Bit;
            dz : BitDriver ( .q = zero ) {{ .level = 0.0 }};
            da0 : BitDriver ( .q = a0 ) {{ .level = 1.0 }};
            da1 : BitDriver ( .q = a1 ) {{ .level = 1.0 }};
            da2 : BitDriver ( .q = a2 ) {{ .level = 0.0 }};
            da3 : BitDriver ( .q = a3 ) {{ .level = 1.0 }};
            db0 : BitDriver ( .q = b0 ) {{ .level = 1.0 }};
            db1 : BitDriver ( .q = b1 ) {{ .level = 1.0 }};
            db2 : BitDriver ( .q = b2 ) {{ .level = 0.0 }};
            db3 : BitDriver ( .q = b3 ) {{ .level = 0.0 }};
            fa0 : FullAdder ( .a = a0, .b = b0, .cin = zero, .sum = s0, .cout = c1 );
            fa1 : FullAdder ( .a = a1, .b = b1, .cin = c1, .sum = s1, .cout = c2 );
            fa2 : FullAdder ( .a = a2, .b = b2, .cin = c2, .sum = s2, .cout = c3 );
            fa3 : FullAdder ( .a = a3, .b = b3, .cin = c3, .sum = s3, .cout = s4 );
        }}
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
        mod Top () {{
            wire gnd : Electrical;
            wire clk_a : Electrical;
            wire clk : Bit;
            wire y1 : Bit; wire y2 : Bit;
            clkgen : ClockSrc ( .clk_a = clk_a, .gnd = gnd ) {{ .period = 1.0e-6 }};
            ccmp : Comparator ( .a = clk_a, .n = gnd, .y = clk );
            i1 : Inv ( .a = clk, .y = y1 );
            i2 : Inv ( .a = y1, .y = y2 );
        }}
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
        mod Top () {{
            wire gnd : Electrical;
            wire clk_a : Electrical;
            wire high_a : Electrical;
            wire clk : Bit; wire din : Bit;
            wire q0 : Bit; wire q0n : Bit; wire q0nn : Bit; wire q1 : Bit;
            clkgen : ClockSrc ( .clk_a = clk_a, .gnd = gnd ) {{ .period = 1.0e-6 }};
            dgen : ClockSrc ( .clk_a = high_a, .gnd = gnd ) {{ .period = 2.7e-6 }};
            ccmp : Comparator ( .a = clk_a, .n = gnd, .y = clk );
            dcmp : Comparator ( .a = high_a, .n = gnd, .y = din );
            f0 : Dff ( .clk = clk, .d = din, .q = q0 );
            i1 : Inv ( .a = q0, .y = q0n );
            i2 : Inv ( .a = q0n, .y = q0nn );
            f1 : Dff ( .clk = clk, .d = q0nn, .q = q1 );
        }}
    ");
    let info = assert_bit_identical(&src, "Top", 6e-6);
    assert_eq!(info.fused_networks, 1, "the inter-stage inverter pair fuses; flops stay per-device");
}
