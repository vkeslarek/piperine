//! Parity baselines — the exact-value regression oracle for the
//! behaviour-preserving `solver-simplification` refactor (SS-06/SS-09/SS-16),
//! ported onto the device ABI (`solver-abi-integration` T17).
//!
//! Each test builds a representative circuit from hand-written `Device`s,
//! drives it through a real analysis driver (DC linear, DC nonlinear Newton +
//! homotopy, TR-BDF2 transient integration, complex AC assembly, and the
//! mixed-signal / digital settle through the owner protocol), and pins the
//! value(s) captured from the pre-refactor solver to a tight tolerance. A later
//! change that moves any pinned number is a defect, not a deviation. The
//! literals and tolerances are byte-identical to their pre-port originals.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::solver_devices::parity_baseline::{DigitalSource, FixedDiode, GatedISink, SineVsrc, ThresholdComparator};
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::digital;

use piperine_abi::layout::{HIGH, LOW};
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions};
use piperine_solver::prelude::AcSweepAnalysisOptions;

// ───────────────────────────── T1 analog baselines ───────────────────────────

/// Resistive divider operating point — DC linear solve.
#[test]
fn parity_divider_op_dc() {
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 10.0, &top, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", 2000.0, &top, &mid);
    CircuitBuilding::add_resistor(&mut b, "r2", 6000.0, &mid, &gnd);
    let mut solver = Solver::new(b.build().unwrap()).build();
    let res = solver.dc().unwrap().solve().unwrap();
    let vmid = res.get_node(&CircuitBuilding::node_id(&mid)).unwrap();

    // 10 V · 6k/(2k+6k) = 7.5 V.
    assert!((vmid - 7.5).abs() < 1e-12, "vmid = {vmid}");
}

/// Diode + series resistor DC operating point — nonlinear Newton exercised
/// through the convergence plan (gmin / source stepping homotopy).
#[test]
fn parity_diode_dc_point() {
    let mut b = CircuitBuilder::new("diode");
    let gnd = b.ground();
    let src = b.node("src");
    let anode = b.node("anode");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &src, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", 1000.0, &src, &anode);
    b.device(FixedDiode::module(), "d", &[], &[Port::Node(anode.clone()), Port::Node(gnd)]);
    let mut solver = Solver::new(b.build().unwrap()).build();
    let res = solver.dc().unwrap().solve().unwrap();
    let vd = res.get_node(&CircuitBuilding::node_id(&anode)).unwrap();

    // Captured operating point of the diode-resistor divider. Re-pinned under Q4 (no midpoint
    // damping, as ngspice): the iterate stops one Newton step nearer the root, 3e-10 V from the
    // earlier capture 0.629_146_879_798_922_5, far inside reltol.
    assert!((vd - 0.629_146_859_019_214_7).abs() < 1e-12, "vd = {vd}");
}

/// Sine-driven RC low-pass — TR-BDF2 transient integration + adaptive stepper.
#[test]
fn parity_rc_transient() {
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    b.device(SineVsrc::module(), "v1", &[], &[Port::Node(top.clone()), Port::Node(gnd.clone())]);
    CircuitBuilding::add_resistor(&mut b, "r1", 1000.0, &top, &mid);
    CircuitBuilding::add_capacitor(&mut b, "c1", 1e-7, &mid, &gnd);
    let opts = TransientAnalysisOptions::new(1e-3, 1e-5);
    let mut solver = Solver::new(b.build().unwrap()).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();

    let last = res.last().unwrap();
    let t_last = last.time();
    let v_last = last.get_node(&CircuitBuilding::node_id(&mid)).unwrap();

    assert!((t_last - 1e-3).abs() < 1e-12, "t_last = {t_last}");
    // Re-pinned when `PiController::propose_dt` was made monotone in the error
    // (bounded `de`, `p >= 0` — see `analyses/convergence.rs`). That fix changes
    // the adaptive grid, so this captured value moved by 4.8e-7.
    //
    // Re-pinned rather than treated as a regression, on evidence: driving this
    // same circuit onto a converged grid (dt_max swept 1e-6 → 1e-8, up to ~100k
    // steps) gives v_last = -0.450_456_791_8, stable to nine digits. Against
    // that reference the OLD baseline was off by 2.84e-5 and the new one by
    // 2.89e-5 — both dominated by the coarse ~386-step adaptive grid, and their
    // difference is 60x smaller than the error they share. The change is
    // accuracy-neutral here; the old number was the captured one, never the
    // accurate one.
    // Re-pinned again (HYG-07): the truncation test now measures the `ddt` states with ngspice's
    // tolerance instead of the node potentials. On the converged grid above (-0.450_456_791_8)
    // the previous pin was off by 2.89e-5 and this one is by 2.95e-5; the 161-step grid became
    // 259 steps. The twin `tran_rc` against ngspice is unmoved (0.5123 of its bound).
    assert!((v_last - (-0.450_486_166_452_110_26)).abs() < 1e-9, "v_last = {v_last}");
    // Adaptive stepper landed a stable, reproducible number of recorded steps.
    // 386 -> 161 with the monotone controller: the old law shrank `dt` whenever
    // the error improved sharply, so it paid for the same accuracy with 2.4x
    // the steps.
    assert_eq!(res.len(), 259, "step count = {}", res.len());
}

/// Parallel RLC resonator (current-driven) — complex AC assembly; pins the
/// transfer peak magnitude and the frequency it lands on.
#[test]
fn parity_rlc_ac_resonant_peak() {
    let mut b = CircuitBuilder::new("rlc");
    let gnd = b.ground();
    let node = b.node("node");
    b.device(circuit::AcCurrentSource::module(), "iac", &[("i", ParamValue::Real(1.0))], &[Port::Node(node.clone())]);
    CircuitBuilding::add_resistor(&mut b, "r1", 10_000.0, &node, &gnd);
    CircuitBuilding::add_capacitor(&mut b, "c1", 1e-9, &node, &gnd);
    b.device(circuit::Inductor::module(), "l1", &[("l", ParamValue::Real(1e-3))], &[Port::Node(node.clone()), Port::Node(gnd)]);
    let mut solver = Solver::new(b.build().unwrap()).build();
    let opts = AcSweepAnalysisOptions {
        start_frequency: 1e4,
        stop_frequency: 1e6,
        steps: 200,
        logarithmic: true,
    };
    let res = solver.ac().unwrap().solve_sweep(opts).unwrap();

    // Find the peak-magnitude point.
    let mut peak_mag = 0.0_f64;
    let mut peak_freq = 0.0_f64;
    for i in 0..res.len() {
        let step = res.get(i).unwrap();
        let v = step.get_node(&CircuitBuilding::node_id(&node)).unwrap();
        if v.norm() > peak_mag {
            peak_mag = v.norm();
            peak_freq = step.frequency;
        }
    }
    // Near resonance the parallel L and C nearly cancel → V ≈ I·R = 10 kΩ · 1 A;
    // the log grid lands just off the exact resonance, so pin the captured peak.
    assert!((peak_mag - 9_817.187_756_927_691).abs() < 1e-6, "peak_mag = {peak_mag}");
    assert!((peak_freq - 160_705.281_826_163_85).abs() < 1e-3, "peak_freq = {peak_freq}");
}

/// Mixed-signal DC settle: an analog divider whose mid node is watched by a
/// comparator; the comparator's digital output gates an extra conductance back
/// onto the mid node (D2A→A2D loop), through the operating point's rounds.
/// Pins the settled analog node and the digital net value.
#[test]
fn parity_mixed_signal_dc_settle() {
    let mut b = CircuitBuilder::new("ms");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 5.0, &top, &gnd);
    CircuitBuilding::add_resistor(&mut b, "r1", 1000.0, &top, &mid);
    CircuitBuilding::add_resistor(&mut b, "r2", 1000.0, &mid, &gnd);
    let ctrl = || Port::Net("ctrl".to_string());
    b.device(ThresholdComparator::module(), "cmp", &[], &[Port::Node(mid.clone()), ctrl()]);
    b.device(GatedISink::module(), "gsink", &[], &[ctrl(), Port::Node(mid.clone())]);
    let mut solver = Solver::new(b.build().unwrap()).build();
    let res = solver.dc().unwrap().solve().unwrap();
    let vmid = res.get_node(&CircuitBuilding::node_id(&mid)).unwrap();

    // Without the gate the divider sits at 2.5 V (> 1 V threshold); the
    // comparator fires One, the gate adds 1 mS to ground, and the node settles
    // lower. Pin the converged value + the latched digital decision.
    assert!((vmid - 1.666_666_666_666_666_7).abs() < 1e-9, "vmid = {vmid}");
    let bound = solver.circuit().bound();
    assert_eq!(bound.digital.read(bound.names.nets["net:ctrl"]), HIGH, "ctrl latched");
}

/// Pure-digital combinational snapshot: two inverters feeding an AND gate.
/// net0 = 1 (driven), inv0: net0→net1, inv1: net1→net2, and: (net1,net2)→net3.
/// Exercises the digital engine's settle to a stable state at an operating point.
#[test]
fn parity_digital_scheduler_snapshot() {
    let mut b = CircuitBuilder::new("dig");
    let n = |i: usize| Port::Net(format!("net{i}"));
    b.device(DigitalSource::module(), "src", &[], &[n(0)]);
    b.device(digital::Inverter::module(), "inv0", &[], &[n(0), n(1)]);
    b.device(digital::Inverter::module(), "inv1", &[], &[n(1), n(2)]);
    b.device(digital::And2::module(), "and", &[], &[n(1), n(2), n(3)]);
    let mut solver = Solver::new(b.build().unwrap()).build();
    solver.dc().unwrap().solve().unwrap();

    let bound = solver.circuit().bound();
    let nets = |i: usize| bound.digital.read(bound.names.nets[&format!("net:net{i}")]).to_vec();
    // net0=1 → inv0 net1=0 → inv1 net2=1 → AND(net1,net2)=AND(0,1)=0.
    assert_eq!(nets(0), HIGH, "net0");
    assert_eq!(nets(1), LOW, "net1");
    assert_eq!(nets(2), HIGH, "net2");
    assert_eq!(nets(3), LOW, "net3");
}

/// The helpers of the parity_baseline tests.
pub struct ParityBaselineCases;

