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
    AnalogReference, BranchIdentifier, DcAnalysisState, Element, ElementCapabilities,
    Netlist, NodeIdentifier, Stamp, TransientAnalysisContext, TransientAnalysisState,
    TrBdf2, TrBdf2Phase,
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

impl Element for Resistor {
    fn name(&self) -> &str { "r" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_AC
            | ElementCapabilities::LOADS_TRAN
    }
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

impl Element for Vdc {
    fn name(&self) -> &str { "v" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
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

impl Element for SineVsrc {
    fn name(&self) -> &str { "vsin" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
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

/// Shockley diode (`i = is·(exp(v/vt) − 1)`) between `n1` and `n2`, stamped as a
/// Norton companion linearised at the current Newton guess.
struct Diode {
    is: f64,
    vt: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl Element for Diode {
    fn name(&self) -> &str { "d" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
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

/// Linear capacitor to ground (`Q = C·v`). Transient companion mirrors the
/// codegen kernel's TR-BDF2 stamping (`TrBdf2::stage_coeffs`, MD-07).
struct CapGnd {
    c: f64,
    node: AnalogReference,
}

impl Element for CapGnd {
    fn name(&self) -> &str { "c" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN | ElementCapabilities::LOADS_AC
    }
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

/// Ideal inductor to ground modelled as a node-to-ground admittance
/// `Y = 1/(jωL)` for the AC resonator (no DC/transient participation here).
struct AcIndGnd {
    l: f64,
    node: AnalogReference,
}

impl Element for AcIndGnd {
    fn name(&self) -> &str { "l" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_AC
    }
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

/// AC current source injecting `i` amps into `node` (small-signal stimulus).
struct AcISrc {
    i: f64,
    node: AnalogReference,
}

impl Element for AcISrc {
    fn name(&self) -> &str { "iac" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_AC
    }
    fn load_ac(
        &mut self,
        _dc: &piperine_solver::abi::DcAnalysisResult,
        _ac: &piperine_solver::abi::AcAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        vec![Stamp::Rhs(self.node.clone(), Complex64::new(self.i, 0.0))]
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
