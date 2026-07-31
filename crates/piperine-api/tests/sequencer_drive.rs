//! T21 (DVT-31/33/34/35) — the two "Done when" bullets that need a real
//! circuit: a ramp drive into an RC follows the analytic segment response,
//! and host calls between breakpoints are counted and asserted zero (the
//! core invariant the whole sequencer design exists to protect).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use piperine_api::sequencer::install::InstallSite;
use piperine_api::sequencer::SequencerControl;
use piperine_api::{HostAdapter, HostElement, Sequencer, SequencerProgram, Wake};
use piperine_signal::wave::Wave;
use piperine_solver::abi::{
    AnalogDevice, AnalogReference, Context, DcAnalysisState, DigitalDevice, ElementCapabilities, Introspect,
    Stamp, TransientAnalysisContext, TransientAnalysisState, TrBdf2, TrBdf2Phase,
};
use piperine_solver::prelude::{CircuitBuilder, TransientAnalysisOptions};

/// A resistor between two analog nets (transient participation).
struct TResistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
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
impl DigitalDevice for TResistor {}
impl Introspect for TResistor {}
impl piperine_solver::abi::Element for TResistor {
    fn name(&self) -> &str {
        "r"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
    }
}

/// A capacitor to ground (TR-BDF2 companion, mirrors the same shape used in
/// `crates/piperine-solver/tests/checkpoint_restore.rs`).
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
impl piperine_solver::abi::Element for TCapGnd {
    fn name(&self) -> &str {
        "c"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_TRAN
    }
}

/// An ideal-ish ramp source (small declared `rout`) wrapping an
/// `InstallSite` — exactly `drive_voltage`'s shape, exercised as a
/// `HostElement` stamping every transient call.
struct RampSource {
    site: InstallSite,
}
impl HostElement for RampSource {
    fn name(&self) -> &str {
        "ramp_source"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
    }
    fn load_dc(&mut self, ctx: &piperine_api::Ctx<'_>, sink: &mut piperine_api::StampSink) {
        self.site.stamp(ctx, sink);
    }
    fn load_transient(&mut self, ctx: &piperine_api::Ctx<'_>, sink: &mut piperine_api::StampSink) {
        self.site.stamp(ctx, sink);
    }
}

/// DVT-31/35: a ramp drive into a series-R / shunt-C low-pass follows the
/// closed-form segment response `Vc(t') = m*t' - m*tau + (v0 + m*tau) *
/// exp(-t'/tau)` (t' measured from the ramp's install instant, m the
/// ramp's slope, tau = R*C, v0 the capacitor's voltage at install).
#[test]
fn a_ramp_drive_into_an_rc_follows_the_analytic_segment_response() {
    let r = 1e3;
    let c = 1e-9;
    let tau = r * c;
    let dur = 20.0 * tau; // stay well inside the ramp's linear region
    let m = 1.0 / dur; // slope: 0 -> 1V over `dur`
    let rout = 1e-3; // declared, near-ideal series impedance

    let mut builder = CircuitBuilder::new("ramp_rc");
    let gnd = builder.ground();
    let vin = builder.node("vin");
    let vout = builder.node("vout");

    let mut site = InstallSite::new(vin.clone(), gnd.clone(), rout);
    site.install(Wave::ramp(0.0, 1.0, dur), 0.0);
    builder.element(Box::new(HostAdapter(RampSource { site })));
    builder.element(Box::new(TResistor { r, n1: vin.clone(), n2: vout.clone() }));
    builder.element(Box::new(TCapGnd { c, node: vout.clone() }));

    let mut circuit = builder.build().expect("the ramp-RC circuit builds");
    let opts = TransientAnalysisOptions::new(dur, tau / 200.0);
    let mut tran = circuit.transient(opts, Context::default()).expect("tran setup");
    let result = tran.solve().expect("tran solves");

    let v0 = 0.0; // capacitor starts at 0V (no @initial / UIC here)
    let analytic = |tp: f64| m * tp - m * tau + (v0 + m * tau) * (-tp / tau).exp();

    let mut checked = 0;
    for step in result.iter() {
        let t = step.time();
        if t < 3.0 * tau || t > dur - tau {
            continue; // stay inside the well-conditioned middle of the ramp
        }
        let got = step.get(vout.clone()).expect("vout is a solved unknown");
        let want = analytic(t);
        assert!(
            (got - want).abs() < 0.02,
            "t={t:.3e}: rc response {got:.6} vs analytic {want:.6} (tau={tau:.3e})"
        );
        checked += 1;
    }
    assert!(checked > 5, "the analytic comparison must actually run over multiple accepted points, got {checked}");
}

/// DVT-31 core invariant: no host call happens between declared
/// breakpoints. A counting `SequencerProgram` proves `resume` fires
/// exactly at its own declared instants, never at any of the many
/// intermediate accepted points the adaptive stepper takes between them.
#[test]
fn host_calls_between_breakpoints_are_counted_and_zero() {
    struct Counting {
        calls: Arc<AtomicUsize>,
        step: usize,
    }
    impl SequencerProgram for Counting {
        fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Wake::Advance(2e-6)
        }
        fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.step += 1;
            if self.step >= 3 {
                Wake::Done
            } else {
                Wake::Advance(2e-6)
            }
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let program = Counting { calls: calls.clone(), step: 0 };
    // start() itself counts as call #1; resume fires 3 more times (steps
    // 0,1,2) before returning Done at step 3, so 4 declared breakpoints
    // total — every OTHER accepted point the adaptive stepper takes along
    // the way must not increment the counter.
    let sequencer = Sequencer::new("counted", program, 0.0, 100e-6);

    let mut builder = CircuitBuilder::new("counted_rc");
    let gnd = builder.ground();
    let vin = builder.node("vin");
    // A plain RC settling from a step, forcing MANY more accepted points
    // than the 3 declared sequencer breakpoints (a tight dt_max keeps the
    // adaptive stepper from just landing on the 3 breakpoints alone).
    let mut site = InstallSite::new(vin.clone(), gnd.clone(), 1e-3);
    site.install(Wave::constant(1.0), 0.0);
    builder.element(Box::new(HostAdapter(RampSource { site })));
    builder.element(Box::new(TResistor { r: 1e3, n1: vin.clone(), n2: gnd.clone() }));
    builder.element(Box::new(HostAdapter(sequencer)));

    let mut circuit = builder.build().expect("builds");
    let opts = TransientAnalysisOptions::new(100e-6, 1e-8);
    let mut tran = circuit.transient(opts, Context::default()).expect("tran setup");
    let result = tran.solve().expect("tran solves");

    // Many more accepted points exist than sequencer breakpoints — proof
    // this is a real discriminating test, not a run with only 3 steps.
    assert!(result.len() > 10, "the run must take more accepted steps than the 3 declared breakpoints: {}", result.len());

    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "resume/start must fire exactly at the 4 declared breakpoints, never at any of the {} other accepted points",
        result.len()
    );
}
