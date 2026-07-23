//! Rollback lifecycle contract tests (ABI-01..08): the checkpoint/restore
//! pair on `Element` is driven by the solver around every candidate step.
//!
//! - T1: the trait defaults (`checkpoint_state` → `None`, `restore_state` no-op).
//! - T2: transient reject path drives checkpoint before attempt + restore on
//!   rejection; accept discards.
//! - T3: DC homotopy retry drives checkpoint before each strategy + restore on
//!   strategy fallthrough.

use piperine_solver::abi::*;
use piperine_solver::prelude::Context;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A stateless stub element: inherits the checkpoint/restore defaults.
struct StatelessStub;

impl AnalogDevice for StatelessStub {}
impl DigitalDevice for StatelessStub {}
impl Introspect for StatelessStub {}

impl Element for StatelessStub {
    fn name(&self) -> &str { "StatelessStub" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
}

/// Spec ABI-06: an element with no mutable non-accept-gated state returns
/// `None` from `checkpoint_state` — the solver skips the restore entirely.
#[test]
fn default_checkpoint_state_is_none() {
    let dev = StatelessStub;
    assert!(dev.checkpoint_state().is_none());
}

/// Spec ABI-02 default: `restore_state` is a no-op on a stateless device —
/// feeding it an arbitrary checkpoint changes nothing and never panics.
#[test]
fn default_restore_state_is_a_noop() {
    let mut dev = StatelessStub;
    let checkpoint = ElementCheckpoint {
        int_state: vec![1, 2, 3],
        real_state: vec![1.5, -2.0],
    };
    dev.restore_state(&checkpoint);
}

/// A recording element that counts checkpoint/restore calls so the reject
/// path (T2) and homotopy retry (T3) can prove the hooks fire. Owns a small
/// piece of non-accept-gated mutable state it checkpoints + restores so the
/// "dirty rejected state must rewind" property is observable.
struct RecordingDevice {
    checkpoints: Arc<AtomicUsize>,
    restores: Arc<AtomicUsize>,
    state_value: f64,
}

impl AnalogDevice for RecordingDevice {}
impl DigitalDevice for RecordingDevice {}
impl Introspect for RecordingDevice {}

impl Element for RecordingDevice {
    fn name(&self) -> &str { "RecordingDevice" }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::SUPPORTS_ROLLBACK
    }

    fn checkpoint_state(&self) -> Option<ElementCheckpoint> {
        self.checkpoints.fetch_add(1, Ordering::SeqCst);
        Some(ElementCheckpoint {
            int_state: Vec::new(),
            real_state: vec![self.state_value],
        })
    }

    fn restore_state(&mut self, checkpoint: &ElementCheckpoint) {
        self.restores.fetch_add(1, Ordering::SeqCst);
        if let Some(&v) = checkpoint.real_state.first() {
            self.state_value = v;
        }
    }
}

impl RecordingDevice {
    fn new(checkpoints: Arc<AtomicUsize>, restores: Arc<AtomicUsize>) -> Self {
        Self { checkpoints, restores, state_value: 0.0 }
    }
}

/// Spec ABI-09/11: the default `limiting_report` returns `None` — a device
/// that does not limit inherits this (zero cost, no false convergence veto).
#[test]
fn default_limiting_report_is_none() {
    let dev = StatelessStub;
    assert!(dev.limiting_report().is_none());
}

/// A bare `Element` declares `SUPPORTS_ROLLBACK` and the trait surface
/// exposes the checkpoint/restore pair — the capability bit and the hook
/// exist together (ABI-01 wiring gate).
#[test]
fn supports_rollback_flag_is_declared_alongside_the_hooks() {
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let restores = Arc::new(AtomicUsize::new(0));
    let dev = RecordingDevice::new(checkpoints, restores);
    assert!(dev
        .capabilities()
        .contains(ElementCapabilities::SUPPORTS_ROLLBACK));
}

/// A device that overrides `checkpoint_state` returns `Some`, and a round-trip
/// through `restore_state` rewinds the mutated state to the checkpoint value.
#[test]
fn checkpoint_then_restore_round_trips_state() {
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let restores = Arc::new(AtomicUsize::new(0));
    let mut dev = RecordingDevice::new(checkpoints.clone(), restores.clone());
    dev.state_value = 4.2;
    let ckpt = dev.checkpoint_state().expect("recording device checkpoints");
    assert_eq!(checkpoints.load(Ordering::SeqCst), 1);

    // Mutate after the checkpoint — the rejected attempt dirties the state.
    dev.state_value = 99.0;
    // Restore rewinds to the checkpointed value.
    dev.restore_state(&ckpt);
    assert_eq!(restores.load(Ordering::SeqCst), 1);
    assert!((dev.state_value - 4.2).abs() < 1e-12);
}

/// Sanity: a `Context` default exists so the recording device can later be
/// driven through a real analysis (used by T2/T3).
#[test]
fn _context_default_compiles_for_later_tests() {
    let _ctx = Context::default();
}

// ── T2: transient reject path drives checkpoint/restore ────────────────────
//
// Spec ABI-02/03/08: a rejected transient step calls restore_state on every
// checkpointed element before the retry; an accepted step discards the
// checkpoint. A circuit is forced through repeated LTE rejections by a tight
// `trtol`, and the recording element counts the hooks.

use piperine_solver::abi as solver_abi;
use piperine_solver::prelude::{Solver, TransientAnalysisOptions};

/// Linear resistor between two references (transient participation).
struct TResistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl AnalogDevice for TResistor {
    fn load_dc(
        &mut self,
        _s: &solver_abi::DcAnalysisState<'_>,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl DigitalDevice for TResistor {}
impl Introspect for TResistor {}
impl Element for TResistor {
    fn name(&self) -> &str { "r" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN
    }
}

/// Sine voltage source with its own branch-current unknown.
struct TSineVsrc {
    amp: f64,
    freq: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl AnalogDevice for TSineVsrc {
    fn load_dc(
        &mut self,
        _s: &solver_abi::DcAnalysisState<'_>,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
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

impl TSineVsrc {
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

impl DigitalDevice for TSineVsrc {}
impl Introspect for TSineVsrc {}
impl Element for TSineVsrc {
    fn name(&self) -> &str { "vsin" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// Linear capacitor to ground — the reactive element whose charge history
/// makes the Milne-LTE gate fire (and thus force rejections) under a tight
/// `trtol`. Companion model mirrors the codegen TR-BDF2 stamping.
struct TCapGnd {
    c: f64,
    node: AnalogReference,
}

impl AnalogDevice for TCapGnd {
    fn load_transient(
        &mut self,
        states: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let dt = t.h;
        let Some(idx) = self.node.idx() else { return Vec::new(); };
        if dt <= 0.0 { return Vec::new(); }
        let (c0, c1, c2) = solver_abi::TrBdf2::stage_coeffs(t.phase, t.h, t.prev_h);
        let vget = |lb: usize| -> f64 {
            let row = if lb == 0 { states.latest() } else { states.view(lb) };
            row.and_then(|r| r.get(idx).copied()).unwrap_or(0.0)
        };
        let q_now = self.c * vget(0);
        let q_prev = self.c * vget(1);
        let q_prev2 = self.c * vget(2);
        let mut i_c = c0 * q_now + c1 * q_prev + c2 * q_prev2;
        if matches!(t.phase, solver_abi::TrBdf2Phase::Trapezoidal) && t.prev_h > 0.0 {
            let (d0, d1, d2) =
                solver_abi::TrBdf2::phase_coeffs(solver_abi::TrBdf2Phase::Bdf2, t.prev_h);
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
}

impl DigitalDevice for TCapGnd {}
impl Introspect for TCapGnd {}
impl Element for TCapGnd {
    fn name(&self) -> &str { "c" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN
    }
}

/// Spec ABI-02/03/08: the sine-driven RC low-pass naturally overshoots its
/// LTE budget on roughly two dozen of its early steps, exercising the reject
/// path. Each rejected step calls `restore_state` on the recording element;
/// each accepted step discards its checkpoint (no restore). Multiple rejects
/// each take a fresh checkpoint and each restores from it.
#[test]
fn transient_reject_drives_restore_accept_discards() {
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let restores = Arc::new(AtomicUsize::new(0));

    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(20));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(21));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(TSineVsrc { amp: 1.0, freq: 1000.0, n1: top.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(TResistor { r: 1000.0, n1: top.clone(), n2: mid.clone() }),
        Box::new(TCapGnd { c: 1e-7, node: mid.clone() }),
        Box::new(RecordingDevice::new(checkpoints.clone(), restores.clone())),
    ];
    let circuit = CircuitInstance::from_devices_and_netlist("rc-reject", elements, netlist);

    // The sine-driven RC low-pass naturally overshoots its LTE budget on
    // ~26 of its first steps under default tolerances, driving the reject
    // path without an artificial tolerance clamp (which would loop forever).
    let opts = TransientAnalysisOptions::new(1e-4, 1e-5);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();

    let n_restores = restores.load(Ordering::SeqCst);
    let n_checkpoints = checkpoints.load(Ordering::SeqCst);

    // ABI-02: at least one rejected step restored the device state.
    assert!(n_restores > 0, "expected rejections to call restore_state");
    // ABI-08: every recorded reject restored exactly once.
    assert_eq!(
        n_restores, res.stats.steps_rejected,
        "restores must match rejected-step count"
    );
    // ABI-03: accepted steps checkpoint but do NOT restore — so the
    // checkpoint count strictly exceeds the restore count.
    assert!(
        n_checkpoints > n_restores,
        "checkpoints ({n_checkpoints}) must exceed restores ({n_restores}) — \
         accepted steps discard their checkpoint"
    );
    // ABI-01: every attempt took a checkpoint.
    assert!(n_checkpoints >= res.stats.steps_accepted + res.stats.steps_rejected);
}

// ── T3: DC homotopy retry drives checkpoint/restore ────────────────────────
//
// Spec ABI-07: when a DC homotopy strategy falls through (failed attempt →
// next strategy), the DC solver calls restore_state on checkpointed elements
// before retrying. A stiff diode whose plain-Newton attempt is starved of
// iterations (max_iter=4) falls through to gmin stepping, which converges.

use piperine_solver::prelude::Policy;

/// Ideal DC voltage source with its own branch-current unknown.
struct TDcVsrc {
    v: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}

impl AnalogDevice for TDcVsrc {
    fn load_dc(
        &mut self,
        _s: &solver_abi::DcAnalysisState<'_>,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), self.v),
        ]
    }
}

impl DigitalDevice for TDcVsrc {}
impl Introspect for TDcVsrc {}
impl Element for TDcVsrc {
    fn name(&self) -> &str { "v" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// Shockley diode — a stiff nonlinear load that plain Newton (starved of
/// iterations) fails on, exercising the homotopy fallthrough.
struct TDiode {
    is: f64,
    vt: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl AnalogDevice for TDiode {
    fn load_dc(
        &mut self,
        s: &solver_abi::DcAnalysisState<'_>,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
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

impl DigitalDevice for TDiode {}
impl Introspect for TDiode {}
impl Element for TDiode {
    fn name(&self) -> &str { "d" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

/// Spec ABI-07: plain Newton fails on the starved stiff diode and falls
/// through to gmin stepping, which converges. The fallthrough fires
/// `restore_state` on the recording element before the gmin retry — so the
/// retry starts from the pre-attempt checkpoint, not the dirty failed-Newton
/// limiter state.
#[test]
fn dc_homotopy_fallthrough_drives_restore_between_strategies() {
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let restores = Arc::new(AtomicUsize::new(0));

    let mut netlist = Netlist::new();
    let src = netlist.connect_node(NodeIdentifier::Anonymous(10));
    let anode = netlist.connect_node(NodeIdentifier::Anonymous(11));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(TDcVsrc { v: 1.0, n1: src.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(TResistor { r: 1000.0, n1: src.clone(), n2: anode.clone() }),
        Box::new(TDiode { is: 1e-14, vt: 0.025_852, n1: anode.clone(), n2: gnd.clone() }),
        Box::new(RecordingDevice::new(checkpoints.clone(), restores.clone())),
    ];
    let circuit = CircuitInstance::from_devices_and_netlist("diode-homotopy", elements, netlist);

    // max_iter=4 starves plain Newton on the stiff diode so it falls through
    // to gmin stepping (which converges from the easy high-gmin start).
    let policy = Policy { max_iter: 4, ..Policy::default() };
    let mut solver = Solver::new(circuit).with_policy(policy).build();
    let res = solver.dc().unwrap().solve().unwrap();

    // Plain Newton fell through to gmin stepping (the homotopy took over).
    assert_eq!(
        res.stats.homotopy_strategy.as_deref(),
        Some("gmin-stepping"),
        "plain Newton must fall through so the homotopy restore fires"
    );
    // ABI-07: the plain-Newton → gmin fallthrough restored the device state
    // before the gmin retry. checkpoint fired once per attempt; the successful
    // gmin attempt is NOT restored.
    let n_restores = restores.load(Ordering::SeqCst);
    let n_checkpoints = checkpoints.load(Ordering::SeqCst);
    assert!(n_restores >= 1, "homotopy fallthrough must call restore_state");
    assert!(
        n_checkpoints > n_restores,
        "the converging gmin attempt checkpoints but is not restored"
    );
}


