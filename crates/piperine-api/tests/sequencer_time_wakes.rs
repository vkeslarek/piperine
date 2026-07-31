//! T19 (DVT-25/26) — the sequencer coroutine driver's linchpin proof: a
//! `Sequencer` is resumed **exactly once** per accepted breakpoint, and a
//! genuinely rejected step never resumes it. Reuses the sine-driven RC
//! low-pass fixture `piperine-solver/tests/checkpoint_restore.rs` uses to
//! force real LTE rejections (`transient_reject_drives_restore_accept_discards`)
//! — under default tolerances it naturally overshoots its LTE budget on a
//! handful of early steps, so `res.stats.steps_rejected > 0` here is a real
//! property of the run, not a contrived stub. The sequencer's own
//! breakpoints are scheduled well after that early window (50us+) so they
//! do not themselves suppress the natural early rejections (a step landed
//! on a declared breakpoint bypasses the LTE gate).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use piperine_api::{HostAdapter, Sequencer, SequencerProgram, Wake};
use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, Context, DcAnalysisState, DigitalDevice, Element,
    ElementCapabilities, Introspect, Netlist, NodeIdentifier, Stamp, TransientAnalysisContext,
    TransientAnalysisState, TrBdf2, TrBdf2Phase,
};
use piperine_solver::prelude::{Solver, TransientAnalysisOptions};

// ── The sine-RC fixture (mirrors piperine-solver/tests/checkpoint_restore.rs) ──

struct TResistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}
impl AnalogDevice for TResistor {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
}
impl TResistor {
    fn stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
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
    fn name(&self) -> &str {
        "r"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN
    }
}

struct TSineVsrc {
    amp: f64,
    freq: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}
impl AnalogDevice for TSineVsrc {
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
    fn name(&self) -> &str {
        "vsin"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

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
        let Some(idx) = self.node.idx() else { return Vec::new() };
        if dt <= 0.0 {
            return Vec::new();
        }
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
        vec![Stamp::Matrix(self.node.clone(), self.node.clone(), g_eq), Stamp::Rhs(self.node.clone(), rhs)]
    }
}
impl DigitalDevice for TCapGnd {}
impl Introspect for TCapGnd {}
impl Element for TCapGnd {
    fn name(&self) -> &str {
        "c"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN
    }
}

fn build_sine_rc(mut extra: Vec<Box<dyn Element>>) -> piperine_solver::prelude::CircuitInstance {
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(20));
    let mid = netlist.connect_node(NodeIdentifier::Anonymous(21));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let mut elements: Vec<Box<dyn Element>> = vec![
        Box::new(TSineVsrc { amp: 1.0, freq: 1000.0, n1: top.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(TResistor { r: 1000.0, n1: top.clone(), n2: mid.clone() }),
        Box::new(TCapGnd { c: 1e-7, node: mid.clone() }),
    ];
    elements.append(&mut extra);
    piperine_solver::prelude::CircuitInstance::from_devices_and_netlist("rc-reject", elements, netlist)
}

// ── The recording program ───────────────────────────────────────────────────

/// A program that advances by fixed steps a fixed number of times, then
/// finishes, recording the exact `t` it was resumed at on every call.
struct Recording {
    first_dt: f64,
    dt: f64,
    remaining: usize,
    log: Arc<Mutex<Vec<f64>>>,
    resumes: Arc<AtomicUsize>,
}
impl SequencerProgram for Recording {
    fn start(&mut self) -> Wake {
        Wake::Advance(self.first_dt)
    }
    fn resume(&mut self, t: f64) -> Wake {
        self.resumes.fetch_add(1, Ordering::SeqCst);
        self.log.lock().unwrap().push(t);
        if self.remaining == 0 {
            Wake::Done
        } else {
            self.remaining -= 1;
            Wake::Advance(self.dt)
        }
    }
}

/// The linchpin proof (T19): run the sine-RC fixture — which really does
/// reject steps under default tolerances — with a recording `Sequencer`
/// scripted for exactly 3 resumes at known instants. If `accept_timestep`
/// (and thus resume) were ever wired to fire during a *candidate* attempt
/// rather than strictly after acceptance, the dozens of real rejected
/// attempts in this fixture would inflate the resume count far past 3; if
/// it fired more than once per accepted landing, the count would likewise
/// exceed 3. Either bug is caught by the exact-count assertion below.
#[test]
fn a_sequencer_resumes_exactly_once_per_accepted_breakpoint_never_on_rejection() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let resumes = Arc::new(AtomicUsize::new(0));
    let program =
        Recording { first_dt: 50e-6, dt: 5e-6, remaining: 2, log: log.clone(), resumes: resumes.clone() };
    let sequencer = Sequencer::new("recorder", program, 0.0, 1e-4);

    let circuit = build_sine_rc(vec![Box::new(HostAdapter(sequencer))]);

    let opts = TransientAnalysisOptions::new(1e-4, 1e-5);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();

    // This run really does reject steps (the same property the solver-side
    // fixture proves) — otherwise this test would not discriminate the bug
    // it exists to catch.
    assert!(res.stats.steps_rejected > 0, "the fixture must force real rejections for this test to mean anything");

    // Exactly 3 resumes: the script asked for 3 (remaining starts at 2, plus
    // the initial resume = 3 total), landing at 50us, 55us, 60us.
    assert_eq!(resumes.load(Ordering::SeqCst), 3, "resumed more or fewer times than the accepted breakpoints it declared");
    let recorded = log.lock().unwrap().clone();
    assert_eq!(recorded, vec![50e-6, 55e-6, 60e-6], "each resume landed at exactly its declared instant, in order");
}

/// A simpler, deterministic companion: `next_breakpoints` after each
/// resume matches the newly pending wake, and disappears once `Done`.
#[test]
fn next_breakpoints_tracks_the_coroutines_pending_wake_across_resumes() {
    struct TwoStep {
        first: bool,
    }
    impl SequencerProgram for TwoStep {
        fn start(&mut self) -> Wake {
            Wake::Advance(1e-6)
        }
        fn resume(&mut self, _t: f64) -> Wake {
            if self.first {
                self.first = false;
                Wake::Advance(2e-6)
            } else {
                Wake::Done
            }
        }
    }
    let circuit = build_sine_rc(vec![Box::new(HostAdapter(Sequencer::new(
        "two_step",
        TwoStep { first: true },
        0.0,
        1e-3,
    )))]);
    let opts = TransientAnalysisOptions::new(5e-6, 1e-6);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().unwrap().solve().unwrap();
    // The two declared breakpoints (1e-6, 3e-6) must both have been landed
    // on exactly — i.e. present among the accepted step times.
    let times: Vec<f64> = res.iter().map(|s| s.time()).collect();
    assert!(times.iter().any(|&t| (t - 1e-6).abs() < 1e-12), "1us breakpoint landed on: {times:?}");
    assert!(times.iter().any(|&t| (t - 3e-6).abs() < 1e-12), "3us breakpoint landed on: {times:?}");
}
