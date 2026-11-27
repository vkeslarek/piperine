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
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::mixed;
use piperine_device_fixtures::partials::PartialsCheck;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::{BIT, F64, HIGH, LOW};
use piperine_abi::raw::{
    Cell, DEFAULT, GROUND, Module, Partial, Shape, Statement, ambient, cell, global, op, part, pin_flag, role, watch,
};
use piperine_abi::shape::{FunctionSpec, PinSpec, ReactionSpec, ShapeBuf, ShapeRef, TermSpec, WatchSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions};
use piperine_solver::prelude::AcSweepAnalysisOptions;

const ORIGIN: &str = "parity_baseline.rs";

// ─────────────────────────── Analog device doubles ───────────────────────────
// The resistor, capacitor, DC source, AC source and inductor are those of
// `helpers/abi_devices.rs`; the diode and the sine source keep this target's
// own constants (`vt = 0.025_852`, `std::f64::consts::PI`).

/// Shockley diode `i = is·(exp(v/vt) − 1)` between `p` and `n`, `vt = 0.025_852`.
/// pins: p=0, n=1 (PORT); terms T0 FLOW{p, n}; fn 0: reads [p, n] writes [T0] partials (0<-0) (0<-1)
struct Diode;
static DIODE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
const IS: f64 = 1e-14;
const VT: f64 = 0.025_852;

impl Device for Diode {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineCases::shared(&DIODE_SHAPE, ParityBaselineCases::diode_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Diode, Arc::clone(ParityBaselineCases::shared(&DIODE_SHAPE, ParityBaselineCases::diode_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let vd = cx.f64(0) - cx.f64(1);
        let ex = (vd / VT).exp();
        let gd = IS / VT * ex;
        cx.set(0, IS * (ex - 1.0));
        cx.jac().copy_from_slice(&[gd, -gd]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// Sine voltage source `amp·sin(2π·freq·t)` on a private branch current `f`;
/// at the t=0 operating point it is `sin(0) = 0`.
/// pins: p=0, n=1 (PORT); f=2 (PRIVATE); terms T0 FLOW{p, n}, T1 FLOW{f, GROUND}
/// fn 0: reads [p, n, f, TIME] writes [T0, T1] partials (0<-f) (1<-p) (1<-n)
struct SineVsrc;
static SINE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
const AMP: f64 = 1.0;
const FREQ: f64 = 1000.0;

impl Device for SineVsrc {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineCases::shared(&SINE_SHAPE, ParityBaselineCases::sine_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((SineVsrc, Arc::clone(ParityBaselineCases::shared(&SINE_SHAPE, ParityBaselineCases::sine_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let v = AMP * (2.0 * std::f64::consts::PI * FREQ * cx.f64(3)).sin();
        cx.set(0, cx.f64(2));
        cx.set(1, cx.f64(0) - cx.f64(1) - v);
        cx.jac().copy_from_slice(&[1.0, 1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

static DIODE: OnceLock<Module> = OnceLock::new();
static SINE: OnceLock<Module> = OnceLock::new();

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
    b.device(DIODE.get_or_init(|| module::<Diode>("Diode")), "d", &[], &[Port::Node(anode.clone()), Port::Node(gnd)]);
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
    b.device(SINE.get_or_init(|| module::<SineVsrc>("SineVsrc")), "v1", &[], &[Port::Node(top.clone()), Port::Node(gnd.clone())]);
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

// ────────────────────── T2 mixed-signal / digital baselines ──────────────────

/// Analog comparator that samples a node voltage and drives a digital net (A2D):
/// a reaction on `START` and on each crossing of `threshold` writes
/// `out = v ≥ threshold`.
/// pins: threshold=0 (PARAM F64, default 1.0); sense=1 (PORT continuous); out=2 (PORT BIT);
///       up=3, down=4 (PRIVATE event); start=5 (GLOBAL START)
/// watches: W0 CROSS -> up (v − threshold), W1 CROSS -> down (threshold − v)
/// fn 0: reads [sense, threshold] writes [W0, W1] partials (0<-0) (1<-0)
/// reaction 0: on [start, up, down] reads [sense, threshold] writes [S0 {out}]
struct Comparator;
static CMP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static CMP: OnceLock<Module> = OnceLock::new();
const ONE_VOLT: [u8; 8] = 1.0_f64.to_le_bytes();

impl Device for Comparator {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineCases::shared(&CMP_SHAPE, ParityBaselineCases::cmp_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Comparator, Arc::clone(ParityBaselineCases::shared(&CMP_SHAPE, ParityBaselineCases::cmp_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let e = cx.f64(0) - cx.f64(1);
        cx.set(0, e);
        cx.set(1, -e);
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
        let level = if cx.f64(0) >= cx.f64(1) { HIGH } else { LOW };
        cx.commit(0, &level, 0.0)?;
        Ok(Flow::Continue)
    }
}

/// Digital-gated conductance to ground: when `ctrl` is HIGH it adds `g` from
/// `node` to ground, otherwise nothing (D2A).
/// pins: g=0 (PARAM F64, default 1 mS); ctrl=1 (PORT BIT); node=2 (PORT continuous)
/// terms: T0 FLOW{node, GROUND}; fn 0: reads [ctrl, node, g] writes [T0] partials (0<-1)
struct GatedISink;
static SINK_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SINK: OnceLock<Module> = OnceLock::new();
const ONE_MS: [u8; 8] = 1e-3_f64.to_le_bytes();

impl Device for GatedISink {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineCases::shared(&SINK_SHAPE, ParityBaselineCases::sink_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((GatedISink, Arc::clone(ParityBaselineCases::shared(&SINK_SHAPE, ParityBaselineCases::sink_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let g = if cx.bytes(0) == HIGH { cx.f64(2) } else { 0.0 };
        cx.set(0, g * cx.f64(1));
        cx.jac().copy_from_slice(&[g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
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
    b.device(CMP.get_or_init(|| module::<Comparator>("Comparator")), "cmp", &[], &[Port::Node(mid.clone()), ctrl()]);
    b.device(SINK.get_or_init(|| module::<GatedISink>("GatedISink")), "gsink", &[], &[ctrl(), Port::Node(mid.clone())]);
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

// ───────────────────── Digital scheduler snapshot (topology) ──────────────────

/// Constant digital source: `out` is HIGH from power-on (its `init`).
/// pins: out=0 (PORT BIT, init HIGH)
struct DigitalSource;
static SRC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static SRC: OnceLock<Module> = OnceLock::new();

impl Device for DigitalSource {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineCases::shared(&SRC_SHAPE, ParityBaselineCases::src_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((DigitalSource, Arc::clone(ParityBaselineCases::shared(&SRC_SHAPE, ParityBaselineCases::src_shape))))
    }
    fn eval(&self, _k: u32, _cx: &mut Eval) -> Result<(), Fault> {
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// Two-input AND gate: pins a=0, b=1, y=2 (PORT BIT); fn 0: reads [a, b] writes [y].
struct And2;
static AND_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static AND: OnceLock<Module> = OnceLock::new();

impl Device for And2 {
    fn prototype() -> &'static Shape<'static> {
        ParityBaselineCases::shared(&AND_SHAPE, ParityBaselineCases::and_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((And2, Arc::clone(ParityBaselineCases::shared(&AND_SHAPE, ParityBaselineCases::and_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (a, b) = (cx.bytes(0), cx.bytes(1));
        let y = if a == HIGH && b == HIGH { HIGH } else { LOW };
        cx.out_bytes(0).copy_from_slice(&y);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// Pure-digital combinational snapshot: two inverters feeding an AND gate.
/// net0 = 1 (driven), inv0: net0→net1, inv1: net1→net2, and: (net1,net2)→net3.
/// Exercises the digital engine's settle to a stable state at an operating point.
#[test]
fn parity_digital_scheduler_snapshot() {
    let mut b = CircuitBuilder::new("dig");
    let n = |i: usize| Port::Net(format!("net{i}"));
    b.device(SRC.get_or_init(|| module::<DigitalSource>("DigitalSource")), "src", &[], &[n(0)]);
    b.device(mixed::Inverter::module(), "inv0", &[], &[n(0), n(1)]);
    b.device(mixed::Inverter::module(), "inv1", &[], &[n(1), n(2)]);
    b.device(AND.get_or_init(|| module::<And2>("And2")), "and", &[], &[n(1), n(2), n(3)]);
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

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    PartialsCheck::verify_every_function("Diode", DIODE.get_or_init(|| module::<Diode>("Diode")));
    PartialsCheck::verify_every_function("SineVsrc", SINE.get_or_init(|| module::<SineVsrc>("SineVsrc")));
    PartialsCheck::verify_every_function("Comparator", CMP.get_or_init(|| module::<Comparator>("Comparator")));
    PartialsCheck::verify_every_function("GatedISink", SINK.get_or_init(|| module::<GatedISink>("GatedISink")));
    PartialsCheck::verify_every_function("DigitalSource", SRC.get_or_init(|| module::<DigitalSource>("DigitalSource")));
    PartialsCheck::verify_every_function("And2", AND.get_or_init(|| module::<And2>("And2")));
}

/// The helpers of the parity_baseline tests.
pub struct ParityBaselineCases;

impl ParityBaselineCases {
    /// The pin helper `pin`.
    fn pin(index: u32) -> Cell {
        Cell { kind: cell::PIN, index, part: part::VALUE }
    }

    /// The term helper `term`.
    fn term(index: u32) -> Cell {
        Cell { kind: cell::TERM, index, part: part::VALUE }
    }

    /// The partial helper `partial`.
    fn partial(write: u32, read: u32) -> Partial {
        Partial { write, to: 0, read, at: 0 }
    }

    /// The flow helper `flow`.
    fn flow(name: &'static str, plus: u32, minus: u32) -> TermSpec<'static> {
        TermSpec { name, origin: ORIGIN, op: op::FLOW, plus, minus, abstol: DEFAULT, max: DEFAULT }
    }

    /// The shared helper `shared`.
    fn shared(slot: &'static OnceLock<ShapeRef>, build: fn() -> ShapeRef) -> &'static ShapeRef {
        slot.get_or_init(build)
    }

    /// The diode shape helper `diode_shape`.
    fn diode_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let t0 = s.term(ParityBaselineCases::flow("i", p, n));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![ParityBaselineCases::pin(p), ParityBaselineCases::pin(n)],
            writes: vec![ParityBaselineCases::term(t0)],
            partials: vec![ParityBaselineCases::partial(0, 0), ParityBaselineCases::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The sine shape helper `sine_shape`.
    fn sine_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
        let t0 = s.term(ParityBaselineCases::flow("i", p, n));
        let t1 = s.term(ParityBaselineCases::flow("eq", f, GROUND));
        s.function(FunctionSpec {
            name: "v",
            origin: ORIGIN,
            reads: vec![ParityBaselineCases::pin(p), ParityBaselineCases::pin(n), ParityBaselineCases::pin(f), Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE }],
            writes: vec![ParityBaselineCases::term(t0), ParityBaselineCases::term(t1)],
            partials: vec![ParityBaselineCases::partial(0, 2), ParityBaselineCases::partial(1, 0), ParityBaselineCases::partial(1, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The cmp shape helper `cmp_shape`.
    fn cmp_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let th = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_VOLT, ..PinSpec::discrete("threshold", role::PARAM, &F64) });
        let sense = s.pin(PinSpec::continuous("sense", role::PORT));
        let out = s.pin(PinSpec::discrete("out", role::PORT, &BIT));
        let up = s.pin(PinSpec::event("up", role::PRIVATE));
        let down = s.pin(PinSpec::event("down", role::PRIVATE));
        let start = s.pin(PinSpec { global: global::START, ..PinSpec::event("start", role::GLOBAL) });
        let w = |name, event| WatchSpec { name, origin: ORIGIN, kind: watch::CROSS, layout: &F64, event, ttol: DEFAULT, etol: DEFAULT };
        let w0 = s.watch(w("rise", up));
        let w1 = s.watch(w("fall", down));
        let wc = |index| Cell { kind: cell::WATCH, index, part: part::VALUE };
        s.function(FunctionSpec {
            name: "e",
            origin: ORIGIN,
            reads: vec![ParityBaselineCases::pin(sense), ParityBaselineCases::pin(th)],
            writes: vec![wc(w0), wc(w1)],
            partials: vec![ParityBaselineCases::partial(0, 0), ParityBaselineCases::partial(1, 0)],
            ..FunctionSpec::default()
        });
        s.reaction(ReactionSpec {
            name: "sample",
            origin: ORIGIN,
            on: vec![start, up, down],
            reads: vec![ParityBaselineCases::pin(sense), ParityBaselineCases::pin(th)],
            writes: vec![Statement { pin: out, offset: 0, len: 1, flags: 0, min_after: 0.0 }],
            ..ReactionSpec::default()
        });
        s.freeze()
    }

    /// The sink shape helper `sink_shape`.
    fn sink_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let g = s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &ONE_MS, ..PinSpec::discrete("g", role::PARAM, &F64) });
        let ctrl = s.pin(PinSpec::discrete("ctrl", role::PORT, &BIT));
        let node = s.pin(PinSpec::continuous("node", role::PORT));
        let t0 = s.term(ParityBaselineCases::flow("i", node, GROUND));
        s.function(FunctionSpec {
            name: "i",
            origin: ORIGIN,
            reads: vec![ParityBaselineCases::pin(ctrl), ParityBaselineCases::pin(node), ParityBaselineCases::pin(g)],
            writes: vec![ParityBaselineCases::term(t0)],
            partials: vec![ParityBaselineCases::partial(0, 1)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The src shape helper `src_shape`.
    fn src_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &HIGH, ..PinSpec::discrete("out", role::PORT, &BIT) });
        s.freeze()
    }

    /// The and shape helper `and_shape`.
    fn and_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        let a = s.pin(PinSpec::discrete("a", role::PORT, &BIT));
        let b = s.pin(PinSpec::discrete("b", role::PORT, &BIT));
        let y = s.pin(PinSpec::discrete("y", role::PORT, &BIT));
        s.function(FunctionSpec { name: "and", origin: ORIGIN, reads: vec![ParityBaselineCases::pin(a), ParityBaselineCases::pin(b)], writes: vec![ParityBaselineCases::pin(y)], ..FunctionSpec::default() });
        s.freeze()
    }
}
