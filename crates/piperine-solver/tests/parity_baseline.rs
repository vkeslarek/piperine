//! Parity baselines — the exact-value regression oracle for the
//! behaviour-preserving `solver-simplification` refactor (SS-06/SS-09/SS-16).
//!
//! Each test builds a representative circuit from hand-written [`Element`]
//! doubles, drives it through a real analysis driver (DC linear, DC nonlinear
//! Newton + homotopy, TR-BDF2 transient integration, complex AC assembly, and
//! the mixed-signal / digital settle seam), and pins the *current* solved
//! value(s) to a tight tolerance. A later refactor phase that changes any
//! pinned number is a defect, not a deviation. The values here were captured
//! from a run of the pre-refactor solver; they are the sharp net that
//! complements the broad existing suite.

use num_complex::Complex64;

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, CircularArrayBuffer2, DcAnalysisState,
    DigitalDevice, Element, ElementCapabilities, EvalCtx, EventSink, Introspect, Netlist,
    NodeIdentifier, Stamp, TransientAnalysisContext, TransientAnalysisState, TrBdf2, TrBdf2Phase,
    DigitalPorts, DigitalNet, LogicValue,
};
use piperine_solver::prelude::{
    Context, CircuitInstance, Solver, TransientAnalysisOptions,
};
use piperine_solver::prelude::AcSweepAnalysisOptions;

// ─────────────────────────── Analog element doubles ──────────────────────────

/// Linear resistor between two references; contributes the same conductance in
/// DC, AC, and transient.
struct Resistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl Resistor {
    fn stamps(&self, g: f64) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl AnalogDevice for Resistor {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps(1.0 / self.r)
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps(1.0 / self.r)
    }
    fn load_ac(
        &mut self,
        _dc: &piperine_solver::abi::DcAnalysisResult,
        _ac: &piperine_solver::abi::AcAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        let g = Complex64::new(1.0 / self.r, 0.0);
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl DigitalDevice for Resistor {}

impl Introspect for Resistor {}

impl Element for Resistor {
    fn name(&self) -> &str { "r" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_AC
            | ElementCapabilities::LOADS_TRAN
    }
}

/// Ideal DC voltage source with its own branch-current unknown. The forced
/// value tracks the source-stepping homotopy scale (`src_scale`).
struct Vdc {
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl Vdc {
    fn branch_stamps(&self, rhs: f64) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), rhs),
        ]
    }
}

impl AnalogDevice for Vdc {
    fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.branch_stamps(self.v * s.src_scale)
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.branch_stamps(self.v)
    }
}

impl DigitalDevice for Vdc {}

impl Introspect for Vdc {}

impl Element for Vdc {
    fn name(&self) -> &str { "v" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// Sine voltage source (branch unknown) whose forced value follows
/// `amp·sin(2π·freq·t)` in the transient; at the t=0 operating point it is
/// `sin(0) = 0`.
struct SineVsrc {
    amp: f64,
    freq: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl SineVsrc {
    fn branch_stamps(&self, rhs: f64) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), rhs),
        ]
    }
}

impl AnalogDevice for SineVsrc {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.branch_stamps(0.0)
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let v = self.amp * (2.0 * std::f64::consts::PI * self.freq * t.time).sin();
        self.branch_stamps(v)
    }
}

impl DigitalDevice for SineVsrc {}

impl Introspect for SineVsrc {}

impl Element for SineVsrc {
    fn name(&self) -> &str { "vsin" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// Shockley diode (`i = is·(exp(v/vt) − 1)`) between `n1` and `n2`, stamped as a
/// Norton companion linearised at the current Newton guess.
struct Diode {
    is: f64,
    vt: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl AnalogDevice for Diode {
    fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let v = |r: &AnalogReference| {
            r.idx()
                .and_then(|i| s.latest().and_then(|row| row.get(i).copied()))
                .unwrap_or(0.0)
        };
        let vd = v(&self.n1) - v(&self.n2);
        let ex = (vd / self.vt).exp();
        let id = self.is * (ex - 1.0);
        let gd = self.is / self.vt * ex;
        let ieq = id - gd * vd;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), gd),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), gd),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -gd),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -gd),
            Stamp::Rhs(self.n1.clone(), -ieq),
            Stamp::Rhs(self.n2.clone(), ieq),
        ]
    }
}

impl DigitalDevice for Diode {}

impl Introspect for Diode {}

impl Element for Diode {
    fn name(&self) -> &str { "d" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

/// Linear capacitor to ground (`Q = C·v`). Transient companion mirrors the
/// codegen kernel's TR-BDF2 stamping (`TrBdf2::stage_coeffs`, MD-07).
struct CapGnd {
    c: f64,
    node: AnalogReference,
}

impl AnalogDevice for CapGnd {
    fn load_transient(
        &mut self,
        states: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let dt = t.h;
        let Some(idx) = self.node.idx() else { return Vec::new(); };
        if dt <= 0.0 { return Vec::new(); }
        let (c0, c1, c2) = TrBdf2::stage_coeffs(t.phase, t.h, t.prev_h);
        let vget = |lb: usize| -> f64 {
            let row = if lb == 0 { states.latest() } else { states.view(lb) };
            row.and_then(|r| r.get(idx).copied()).unwrap_or(0.0)
        };
        let q_now = self.c * vget(0);
        let q_prev = self.c * vget(1);
        let q_prev2 = self.c * vget(2);
        let mut i_c = c0 * q_now + c1 * q_prev + c2 * q_prev2;
        if matches!(t.phase, TrBdf2Phase::Trapezoidal) && t.prev_h > 0.0 {
            let (d0, d1, d2) = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, t.prev_h);
            let q_prev3 = self.c * vget(3);
            i_c -= d0 * q_prev + d1 * q_prev2 + d2 * q_prev3;
        }
        let g_eq = c0 * self.c;
        let rhs = c0 * q_now - i_c;
        vec![
            Stamp::Matrix(self.node.clone(), self.node.clone(), g_eq),
            Stamp::Rhs(self.node.clone(), rhs),
        ]
    }
    fn load_ac(
        &mut self,
        _dc: &piperine_solver::abi::DcAnalysisResult,
        ac: &piperine_solver::abi::AcAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        let w = 2.0 * std::f64::consts::PI * ac.frequency;
        let y = Complex64::new(0.0, w * self.c);
        vec![Stamp::Matrix(self.node.clone(), self.node.clone(), y)]
    }
}

impl DigitalDevice for CapGnd {}

impl Introspect for CapGnd {}

impl Element for CapGnd {
    fn name(&self) -> &str { "c" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN | ElementCapabilities::LOADS_AC
    }
}

/// Ideal inductor to ground modelled as a node-to-ground admittance
/// `Y = 1/(jωL)` for the AC resonator (no DC/transient participation here).
struct AcIndGnd {
    l: f64,
    node: AnalogReference,
}

impl AnalogDevice for AcIndGnd {
    fn load_ac(
        &mut self,
        _dc: &piperine_solver::abi::DcAnalysisResult,
        ac: &piperine_solver::abi::AcAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        let w = 2.0 * std::f64::consts::PI * ac.frequency;
        let y = Complex64::new(1.0, 0.0) / Complex64::new(0.0, w * self.l);
        vec![Stamp::Matrix(self.node.clone(), self.node.clone(), y)]
    }
}

impl DigitalDevice for AcIndGnd {}

impl Introspect for AcIndGnd {}

impl Element for AcIndGnd {
    fn name(&self) -> &str { "l" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_AC
    }
}

/// AC current source injecting `i` amps into `node` (small-signal stimulus).
struct AcISrc {
    i: f64,
    node: AnalogReference,
}

impl AnalogDevice for AcISrc {
    fn load_ac(
        &mut self,
        _dc: &piperine_solver::abi::DcAnalysisResult,
        _ac: &piperine_solver::abi::AcAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        vec![Stamp::Rhs(self.node.clone(), Complex64::new(self.i, 0.0))]
    }
}

impl DigitalDevice for AcISrc {}

impl Introspect for AcISrc {}

impl Element for AcISrc {
    fn name(&self) -> &str { "iac" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_AC
    }
}

// ───────────────────────────── T1 analog baselines ───────────────────────────

/// Resistive divider operating point — DC linear solve.
#[test]
fn parity_divider_op_dc() {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(2));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { v: 10.0, n1: top.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(Resistor { r: 2000.0, n1: top.clone(), n2: mid.clone() }),
        Box::new(Resistor { r: 6000.0, n1: mid.clone(), n2: gnd.clone() }),
    ];
    let circuit = CircuitInstance::from_devices_and_netlist("divider", elements, netlist);
    let mut solver = Solver::new(circuit).build();
    let res = solver.dc().unwrap().solve().unwrap();
    let vmid = res.get_node(&NodeIdentifier::Anonymous(2)).unwrap();

    // 10 V · 6k/(2k+6k) = 7.5 V.
    assert!((vmid - 7.5).abs() < 1e-12, "vmid = {vmid}");
}

/// Diode + series resistor DC operating point — nonlinear Newton exercised
/// through the convergence plan (gmin / source stepping homotopy).
#[test]
fn parity_diode_dc_point() {
    let mut netlist = Netlist::new();
    let src = netlist.connect_node(NodeIdentifier::Anonymous(10));
    let anode = netlist.connect_node(NodeIdentifier::Anonymous(11));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { v: 1.0, n1: src.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(Resistor { r: 1000.0, n1: src.clone(), n2: anode.clone() }),
        Box::new(Diode { is: 1e-14, vt: 0.025_852, n1: anode.clone(), n2: gnd.clone() }),
    ];
    let circuit = CircuitInstance::from_devices_and_netlist("diode", elements, netlist);
    let mut solver = Solver::new(circuit).build();
    let res = solver.dc().unwrap().solve().unwrap();
    let vd = res.get_node(&NodeIdentifier::Anonymous(11)).unwrap();

    // Captured operating point of the diode-resistor divider.
    assert!((vd - 0.629_146_879_798_922_5).abs() < 1e-12, "vd = {vd}");
}

/// Sine-driven RC low-pass — TR-BDF2 transient integration + adaptive stepper.
#[test]
fn parity_rc_transient() {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(20));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(21));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(SineVsrc { amp: 1.0, freq: 1000.0, n1: top.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(Resistor { r: 1000.0, n1: top.clone(), n2: mid.clone() }),
        Box::new(CapGnd { c: 1e-7, node: mid.clone() }),
    ];
    let circuit = CircuitInstance::from_devices_and_netlist("rc", elements, netlist);
    let opts = TransientAnalysisOptions::new(1e-3, 1e-5);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();

    let last = res.last().unwrap();
    let t_last = last.time();
    let v_last = last.get_node(&NodeIdentifier::Anonymous(21)).unwrap();

    assert!((t_last - 1e-3).abs() < 1e-12, "t_last = {t_last}");
    assert!((v_last - (-0.450_485_218_772_388_9)).abs() < 1e-9, "v_last = {v_last}");
    // Adaptive stepper landed a stable, reproducible number of recorded steps.
    assert_eq!(res.len(), 386, "step count = {}", res.len());
}

/// Parallel RLC resonator (current-driven) — complex AC assembly; pins the
/// transfer peak magnitude and the frequency it lands on.
#[test]
fn parity_rlc_ac_resonant_peak() {
    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(30));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let _ = gnd;

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(AcISrc { i: 1.0, node: node.clone() }),
        Box::new(Resistor { r: 10_000.0, n1: node.clone(), n2: netlist_gnd(&mut netlist) }),
        Box::new(CapGnd { c: 1e-9, node: node.clone() }),
        Box::new(AcIndGnd { l: 1e-3, node: node.clone() }),
    ];
    let circuit = CircuitInstance::from_devices_and_netlist("rlc", elements, netlist);
    let mut solver = Solver::new(circuit).build();
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
        let v = step.get_node(&NodeIdentifier::Anonymous(30)).unwrap();
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

fn netlist_gnd(netlist: &mut Netlist) -> AnalogReference {
    netlist.connect_node(NodeIdentifier::Gnd)
}

// ────────────────────── T2 mixed-signal / digital baselines ──────────────────

/// Analog comparator that samples an analog node voltage and drives a digital
/// net (A2D). Emits its output on the analog accept hook (`accept_timestep`).
struct Comparator {
    sense_idx: usize,
    threshold: f64,
    out: DigitalNet,
    last: LogicValue,
}

impl AnalogDevice for Comparator {}

impl DigitalDevice for Comparator {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: std::slice::from_ref(&self.out) }
    }
}

impl Introspect for Comparator {}

impl Element for Comparator {
    fn name(&self) -> &str { "cmp" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL | ElementCapabilities::SAMPLES_ANALOG
    }
    fn accept_timestep(
        &mut self,
        state: &CircularArrayBuffer2<f64>,
        _t: f64,
        _nets: &[LogicValue],
        sink: &mut dyn EventSink,
    ) {
        let v = state.latest().and_then(|r| r.get(self.sense_idx).copied()).unwrap_or(0.0);
        let new = if v >= self.threshold { LogicValue::One } else { LogicValue::Zero };
        if new != self.last {
            self.last = new;
            sink.emit(self.out, new, 0.0);
        }
    }
}

/// Digital-gated current sink: when its control net is `One`, it pulls a fixed
/// current out of `node` (a resistive load re-routing), otherwise contributes
/// nothing (D2A).
struct GatedISink {
    ctrl: DigitalNet,
    node: AnalogReference,
    g: f64,
}

impl AnalogDevice for GatedISink {
    fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let on = s.digital.get(self.ctrl.0).copied() == Some(LogicValue::One);
        if on {
            vec![Stamp::Matrix(self.node.clone(), self.node.clone(), self.g)]
        } else {
            Vec::new()
        }
    }
}

impl DigitalDevice for GatedISink {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.ctrl), outputs: &[] }
    }
}

impl Introspect for GatedISink {}

impl Element for GatedISink {
    fn name(&self) -> &str { "gsink" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::DIGITAL
            | ElementCapabilities::DEPENDS_ON_DIGITAL
    }
}

/// Mixed-signal DC settle: an analog divider whose mid node is watched by a
/// comparator; the comparator's digital output gates an extra conductance back
/// onto the mid node (D2A→A2D loop). Exercises the `SignalBridge` seam that T6
/// folds. Pins the settled analog node and the digital net value.
#[test]
fn parity_mixed_signal_dc_settle() {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(40));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(41));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));
    let mid_idx = mid.idx().unwrap();
    let ctrl = DigitalNet(0);

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { v: 5.0, n1: top.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(Resistor { r: 1000.0, n1: top.clone(), n2: mid.clone() }),
        Box::new(Resistor { r: 1000.0, n1: mid.clone(), n2: gnd.clone() }),
        Box::new(Comparator { sense_idx: mid_idx, threshold: 1.0, out: ctrl, last: LogicValue::X }),
        Box::new(GatedISink { ctrl, node: mid.clone(), g: 1e-3 }),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("ms", elements, netlist);
    circuit.digital_state = piperine_solver::abi::DigitalState::new(1);
    let mut solver = Solver::new(circuit).build();
    let res = solver.dc().unwrap().solve().unwrap();
    let vmid = res.get_node(&NodeIdentifier::Anonymous(41)).unwrap();

    // Without the gate the divider sits at 2.5 V (> 1 V threshold); the
    // comparator fires One, the gate adds 1 mS to ground, and the node settles
    // lower. Pin the converged value + the latched digital decision.
    assert!((vmid - 1.666_666_666_666_666_7).abs() < 1e-9, "vmid = {vmid}");
}

// ───────────────────── Digital scheduler snapshot (topology) ──────────────────

/// Constant digital source: drives `out` to `One` at power-on.
struct DigitalSource {
    out: DigitalNet,
}

impl AnalogDevice for DigitalSource {}

impl DigitalDevice for DigitalSource {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: std::slice::from_ref(&self.out) }
    }
    fn init(&mut self, sink: &mut dyn EventSink) {
        sink.emit(self.out, LogicValue::One, 0.0);
    }
}

impl Introspect for DigitalSource {}

impl Element for DigitalSource {
    fn name(&self) -> &str { "src" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
}

/// Inverter with zero delay for a purely digital combinational chain.
struct Inv {
    input: DigitalNet,
    output: DigitalNet,
}

impl AnalogDevice for Inv {}

impl DigitalDevice for Inv {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts {
            inputs: std::slice::from_ref(&self.input),
            outputs: std::slice::from_ref(&self.output),
        }
    }
    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let v = ctx.nets[self.input.0];
        let out = match v {
            LogicValue::One => LogicValue::Zero,
            LogicValue::Zero => LogicValue::One,
            other => other,
        };
        sink.emit(self.output, out, 0.0);
    }
}

impl Introspect for Inv {}

impl Element for Inv {
    fn name(&self) -> &str { "inv" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
}

/// Two-input AND gate.
struct And2 {
    a: DigitalNet,
    b: DigitalNet,
    output: DigitalNet,
    inputs: [DigitalNet; 2],
}

impl And2 {
    fn new(a: DigitalNet, b: DigitalNet, output: DigitalNet) -> Self {
        Self { a, b, output, inputs: [a, b] }
    }
}

impl AnalogDevice for And2 {}

impl DigitalDevice for And2 {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.output) }
    }
    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let a = ctx.nets[self.a.0];
        let b = ctx.nets[self.b.0];
        let out = match (a, b) {
            (LogicValue::One, LogicValue::One) => LogicValue::One,
            (LogicValue::Zero, _) | (_, LogicValue::Zero) => LogicValue::Zero,
            _ => LogicValue::X,
        };
        sink.emit(self.output, out, 0.0);
    }
}

impl Introspect for And2 {}

impl Element for And2 {
    fn name(&self) -> &str { "and" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
}

/// Pure-digital combinational snapshot: two inverters feeding an AND gate.
/// net0 = 1 (driven), inv0: net0→net1, inv1: net1→net2, and: (net1,net2)→net3.
/// Exercises the digital scheduler / topology and the settle to a stable state.
#[test]
fn parity_digital_scheduler_snapshot() {
    let netlist = Netlist::new();
    let n0 = DigitalNet(0);
    let n1 = DigitalNet(1);
    let n2 = DigitalNet(2);
    let n3 = DigitalNet(3);

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(DigitalSource { out: n0 }),
        Box::new(Inv { input: n0, output: n1 }),
        Box::new(Inv { input: n1, output: n2 }),
        Box::new(And2::new(n1, n2, n3)),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("dig", elements, netlist);
    circuit.digital_state = piperine_solver::abi::DigitalState::new(4);
    circuit.rebuild_digital_topology();
    circuit.init_digital().unwrap();

    let nets = &circuit.digital_state.nets;
    // net0=1 → inv0 net1=0 → inv1 net2=1 → AND(net1,net2)=AND(0,1)=0.
    assert_eq!(nets[0], LogicValue::One, "net0");
    assert_eq!(nets[1], LogicValue::Zero, "net1");
    assert_eq!(nets[2], LogicValue::One, "net2");
    assert_eq!(nets[3], LogicValue::Zero, "net3");
}
