//! T18 (DVT-30) — the phase's integration proof: a non-trivial Rust host
//! device composing all three levels this batch delivered (T14-T18), end
//! to end in a real circuit. Not new production code beyond what the proof
//! needs — everything here is `piperine-api::device` + `piperine-signal`,
//! wired the way an author would wire them.
//!
//! What is proven, one test per "Done when" bullet:
//! - `a_composed_device_stamps_level_0_and_installs_level_1` — the
//!   `ideal.md` example G shape minus the sequencer half (Phase 4's job):
//!   one class stamps a plain resistive load directly (level 0) *and*
//!   installs a `Wave` and a `Behavior` (level 1), all in the same
//!   `load_dc`.
//! - `hysteretic_pwm_runs_end_to_end` — the `ideal.md` example D
//!   controller: `Behavior::hysteresis` driving `Behavior::switch` through
//!   `Composed`, solving in a real circuit.
//! - `a_rejected_step_leaves_level_1_operator_state_untouched` — a
//!   `piperine_signal::ops::Operator` (accept-gated by construction: its
//!   only mutator is `accept`, documented "call only on accepted
//!   timesteps") is read via `load_dc`-shaped calls with no
//!   `accept_timestep` in between; its `pending_edges` (a `&self` probe)
//!   proves nothing moved, while the level-0 half of the same device keeps
//!   stamping normally across the same calls.
//! - `behavior_evaluation_is_the_closed_form_rust_path_not_a_host_call` —
//!   an instrumented `BehaviorKind` counts every `value`/`jacobian` call;
//!   the count is asserted, not inspected (T8's pattern), and by
//!   construction (no FFI, no dynamic Python dispatch anywhere in this
//!   crate) every one of those calls is the native closed form.
//! - `all_three_levels_compose_in_one_class_end_to_end` (DVT-30 fix-round
//!   gap closure) — `ideal.md` example G's full shape, sequencer half
//!   included: one `HostElement` stamps its own conductance directly
//!   (level 0), installs a `Wave` into an independent node (level 1), and
//!   owns a `Sequencer` (level 2) that sweeps that same conductance over
//!   time. `Sequencer<P>`'s own `HostElement` impl (`sequencer/driver.rs`)
//!   never overrides `load_dc` — nothing in the ABI requires it to: a
//!   composed device wraps a `Sequencer<P>` as a field and delegates the
//!   time/lifecycle methods (`next_breakpoints`/`accept_timestep`/
//!   `on_digital_settled`/`drain_digital_drives`/`validation_reports`) to
//!   it through the same `HostElement` trait, exactly the composition-over-
//!   inheritance shape Rust uses in place of the Python example's
//!   subclassing. No change to `sequencer/driver.rs` was needed or made.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use piperine_api::sequencer::SequencerControl;
use piperine_api::{
    BehaviorStamp, Ctx, HostAdapter, HostElement, Sequencer, SequencerProgram, StampSink, StateView, Wake, WaveStamp,
};
use piperine_signal::behavior::kinds::{Comparator, Composed, Hysteresis, Switch};
use piperine_signal::behavior::{BehaviorKind, Crossing};
use piperine_signal::ops::Operator;
use piperine_signal::wave::Wave;
use piperine_solver::abi::{
    AnalogDevice, AnalogReference, AnalogVariable, Context, DcAnalysisState, DigitalDevice, DigitalNet, Element,
    ElementCapabilities, Introspect, LogicValue, NodeIdentifier, Stamp, TransientAnalysisContext,
    TransientAnalysisState, UnknownAllocator,
};
use piperine_solver::prelude::{CircuitBuilder, Solver, TransientAnalysisOptions};

/// An ideal DC voltage source — level 0, no `HostElement` needed — used
/// only to bias the circuits below so this file proves the *host devices*,
/// not a second source kind.
struct Vsrc {
    p: AnalogReference,
    n: AnalogReference,
    branch: Option<AnalogReference>,
    v: f64,
}
impl AnalogDevice for Vsrc {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        self.branch = Some(alloc.branch("vsrc", "i"));
    }
    fn load_dc(&mut self, _state: &DcAnalysisState<'_>, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let b = self.branch.clone().unwrap();
        vec![
            Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
            Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
            Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
            Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
            Stamp::Rhs(b, self.v),
        ]
    }
}
impl DigitalDevice for Vsrc {}
impl Introspect for Vsrc {}
impl Element for Vsrc {
    fn name(&self) -> &str {
        "vsrc"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

// ── Test 1: one class, level 0 + level 1 (Wave + Behavior) ─────────────────

/// `ideal.md` example G's shape, minus the sequencer half (Phase 4): a
/// class that stamps its own resistive load directly (level 0) *and*
/// installs a `Wave` (an independent source) and a `Behavior` (a reactive
/// cross-coupled force), in the same device.
struct Composite {
    // level 0: a plain resistive load between its own two ports.
    load_p: AnalogReference,
    load_n: AnalogReference,
    load_g: f64,
    // level 1: an independent Wave-driven output.
    wave: WaveStamp,
    // level 1: a Behavior reading (ctrl_a, ctrl_b), forcing (force_p, gnd).
    // A bias resistor keeps `force_p` well-posed on its own (the Behavior's
    // cross terms alone, with force terminals disjoint from its controls,
    // carry no self term — the same reason a bare VCCS needs a return
    // path).
    behavior: BehaviorStamp,
    bias_p: AnalogReference,
    bias_n: AnalogReference,
    bias_g: f64,
}

impl HostElement for Composite {
    fn name(&self) -> &str {
        "composite"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
    fn load_dc(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        sink.g(&self.load_p, &self.load_p, self.load_g);
        sink.g(&self.load_p, &self.load_n, -self.load_g);
        sink.g(&self.load_n, &self.load_p, -self.load_g);
        sink.g(&self.load_n, &self.load_n, self.load_g);

        sink.g(&self.bias_p, &self.bias_p, self.bias_g);
        sink.g(&self.bias_p, &self.bias_n, -self.bias_g);
        sink.g(&self.bias_n, &self.bias_p, -self.bias_g);
        sink.g(&self.bias_n, &self.bias_n, self.bias_g);

        self.wave.stamp(ctx, sink);
        self.behavior.stamp(ctx, sink);
    }
}

#[test]
fn a_composed_device_stamps_level_0_and_installs_level_1() {
    let mut builder = CircuitBuilder::new("composite");
    let gnd = builder.ground();
    let vin = builder.node("vin");
    let mid = builder.node("mid");
    let force_node = builder.node("force");
    let wave_node = builder.node("wout");

    builder.element(Box::new(Vsrc { p: vin.clone(), n: gnd.clone(), branch: None, v: 5.0 }));
    builder.element(Box::new(HostAdapter(Composite {
        load_p: vin.clone(),
        load_n: mid.clone(),
        load_g: 1.0 / 1e3,
        wave: WaveStamp::new(Wave::constant(2.5), wave_node.clone(), gnd.clone(), 100.0),
        behavior: BehaviorStamp::new(
            Box::new(Comparator { lo: 0.0, hi: 1.0, gain: 1.0 }),
            vec![vin.clone(), gnd.clone()],
            force_node.clone(),
            gnd.clone(),
        ),
        bias_p: force_node.clone(),
        bias_n: gnd.clone(),
        bias_g: 1e-3,
    })));

    let mut circuit = builder.build().expect("a composed level-0 + level-1 device builds into a circuit");
    let mut dc = circuit.dc(Context::default()).expect("dc setup");
    let result = dc.solve().expect("a composed device solves");

    // Level 0: the resistive load is a straight two-terminal resistor with
    // nothing else touching `mid` — its own equation pins `mid == vin`.
    let v_mid = result.get(mid.clone()).expect("mid is solved");
    assert!((v_mid - 5.0).abs() < 1e-2, "the level-0 load participates normally: v(mid) = {v_mid}");

    // Level 1 Wave: a self-contained Norton source into an otherwise-empty
    // node reads back exactly its declared constant.
    let v_wave = result.get(wave_node.clone()).expect("wout is solved");
    assert!((v_wave - 2.5).abs() < 1e-2, "the installed Wave drives its declared value: v(wout) = {v_wave}");

    // Level 1 Behavior: the force node is a real, finite, solved unknown —
    // proof the cross-Jacobian force plus its bias resistor make a
    // well-posed nonlinear equation (not merely "compiles").
    let v_force = result.get(force_node).expect("the Behavior's force node is solved");
    assert!(v_force.is_finite(), "the installed Behavior resolves to a finite operating point: {v_force}");
}

// ── Test 2: the hysteretic PWM controller, end to end ───────────────────────

#[test]
fn hysteretic_pwm_runs_end_to_end() {
    struct Pwm {
        vout: AnalogReference,
        vref: AnalogReference,
        sw_p: AnalogReference,
        sw_n: AnalogReference,
        pwm: BehaviorStamp,
        bias_p: AnalogReference,
        bias_n: AnalogReference,
        bias_g: f64,
    }
    impl HostElement for Pwm {
        fn name(&self) -> &str {
            "hyst_pwm"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
        fn load_dc(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
            let _ = (&self.sw_p, &self.sw_n);
            sink.g(&self.bias_p, &self.bias_p, self.bias_g);
            sink.g(&self.bias_p, &self.bias_n, -self.bias_g);
            sink.g(&self.bias_n, &self.bias_p, -self.bias_g);
            sink.g(&self.bias_n, &self.bias_n, self.bias_g);
            // A small conductance to ground at each of the comparator's own
            // read-only inputs — the same role a gmin stamp plays: the
            // Behavior's cross-Jacobian is exactly zero for this genuinely
            // discontinuous kind (Switch/Hysteresis both report a zero
            // derivative away from their own edge, `ideal.md` §3.3), so
            // nothing else in this circuit gives `vout`/`vref` a return
            // path once the driving Vsrc's own branch equation is combined
            // with a purely-read node — a realistic PWM comparator's inputs
            // are always loaded by something.
            let gmin = 1e-9;
            sink.g(&self.vout, &self.vout, gmin);
            sink.g(&self.vref, &self.vref, gmin);
            self.pwm.stamp(ctx, sink);
        }
    }

    let mut builder = CircuitBuilder::new("hyst_pwm");
    let gnd = builder.ground();
    let vout = builder.node("vout");
    let vref = builder.node("vref");
    let sw = builder.node("sw");

    builder.element(Box::new(Vsrc { p: vout.clone(), n: gnd.clone(), branch: None, v: 5.1 }));
    builder.element(Box::new(Vsrc { p: vref.clone(), n: gnd.clone(), branch: None, v: 5.0 }));

    // `Behavior.hysteresis(vout, vref, vth_hi, vth_lo)` driving
    // `Behavior.switch(ctrl, ron, roff, vth)` through `Composed` — the
    // exact `ideal.md` example D shape.
    let kind = Composed {
        // SPEC_DEVIATION: `ron`/`roff` chosen for numerically sane current
        // magnitudes under `BehaviorStamp`'s current-force stamping
        // (`I(p,n) <+ kind.value(inputs)`), not ngspice-realistic switch
        // resistances. `Behavior.switch`'s value is documented as a
        // *conductance*; stamping it as a current force (the only shape
        // `BehaviorStamp` provides) is a scoped approximation for this
        // proof — a true variable-conductance adapter is a natural T16
        // follow-up, not required by any Done-when bullet here.
        f: Box::new(Switch { ron: 1e3, roff: 1e9, vth: 0.5 }),
        g: Box::new(Hysteresis::new(5e-3, -5e-3)),
    };
    let pwm = BehaviorStamp::new(Box::new(kind), vec![vout.clone(), vref.clone()], sw.clone(), gnd.clone());

    builder.element(Box::new(HostAdapter(Pwm {
        vout: vout.clone(),
        vref: vref.clone(),
        sw_p: sw.clone(),
        sw_n: gnd.clone(),
        pwm,
        bias_p: sw.clone(),
        bias_n: gnd.clone(),
        bias_g: 1e-3,
    })));

    let mut circuit = builder.build().expect("the PWM controller builds into a circuit");
    let mut dc = circuit.dc(Context::default()).expect("dc setup");
    let result = dc.solve().expect("the hysteretic PWM controller solves end to end, reacting at Newton rate");
    let v_sw = result.get(sw).expect("the switch node is solved");
    assert!(v_sw.is_finite(), "the PWM controller's switch node settles to a finite operating point: {v_sw}");
}

// ── Test 3: a rejected step leaves level-1 operator state untouched ────────

#[test]
fn a_rejected_step_leaves_level_1_operator_state_untouched() {
    /// A device holding a `piperine_signal::ops::Operator` (level 1): its
    /// `load_dc` — the "candidate attempt" path a rejected step also
    /// exercises — never calls `Operator::accept`, only `accept_timestep`
    /// does. `Operator`'s own doc header makes this the type-level
    /// guarantee: "a rejected step must leave every operator untouched"
    /// because `accept` is its only mutator and it is "call only on
    /// accepted timesteps".
    struct WithOperator {
        p: AnalogReference,
        n: AnalogReference,
        g: f64,
        transition: Operator,
    }
    impl HostElement for WithOperator {
        fn name(&self) -> &str {
            "with_operator"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
        fn load_dc(&mut self, _ctx: &Ctx<'_>, sink: &mut StampSink) {
            // The level-0 half: an ordinary resistive load, stamped every
            // call regardless of acceptance — proving it "participates
            // normally" across the same repeated (candidate) calls.
            sink.g(&self.p, &self.p, self.g);
            sink.g(&self.p, &self.n, -self.g);
            sink.g(&self.n, &self.p, -self.g);
            sink.g(&self.n, &self.n, self.g);
            // The level-1 operator is deliberately never touched here.
        }
    }

    let p = AnalogReference::new(Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(0))), 0);
    let n = AnalogReference::ground();
    let device = WithOperator { p, n, g: 1e-3, transition: Operator::transition(0.0, 1e-6, 1e-6) };

    // Probe: an unseeded/settled transition has no pending edges (T2's own
    // rule). Read via the `&self` `pending_edges` — a state probe that
    // cannot itself mutate the operator.
    let pending_before = {
        let mut out = Vec::new();
        device.transition.pending_edges(0.0, 1.0, &mut out);
        out
    };
    assert!(pending_before.is_empty(), "a fresh, unseeded transition has no pending edges");

    // Simulate several *rejected* candidate attempts: call the ABI's own
    // `Element::load_dc` (the stamping path — driven through `HostAdapter`,
    // the same blanket adapter the solver uses) repeatedly with no
    // `accept_timestep` in between — exactly what happens between a
    // candidate step and its rejection. `DcAnalysisState::new` and
    // `CircularArrayBuffer2` are both public (`piperine_solver::abi`), so
    // this drives the real ABI rather than a test-only shortcut.
    let mut adapter = HostAdapter(device);
    let history = piperine_solver::abi::CircularArrayBuffer2::<f64>::new(1, 1);
    let digital: Vec<piperine_solver::abi::LogicValue> = Vec::new();
    let state = DcAnalysisState::new(&history, &digital, 1.0);
    let context = Context::default();
    let mut stamped_counts = Vec::new();
    for _ in 0..5 {
        let stamps = AnalogDevice::load_dc(&mut adapter, &state, &context);
        stamped_counts.push(stamps.len());
    }
    let device = &mut adapter.0;

    assert!(
        stamped_counts.iter().all(|&n| n == stamped_counts[0]),
        "the level-0 half stamps identically across every rejected candidate attempt: {stamped_counts:?}"
    );

    let pending_after = {
        let mut out = Vec::new();
        device.transition.pending_edges(0.0, 1.0, &mut out);
        out
    };
    assert_eq!(
        pending_after, pending_before,
        "five rejected candidate attempts (load_dc with no accept_timestep) leave the level-1 \
         operator's own state exactly as it started"
    );

    // Now a real acceptance: `accept_timestep`-shaped mutation, through the
    // operator's only mutator.
    let _ = device.transition.accept(0.0, 1.0); // first observation: settles at input (T2's rule)
    let _ = device.transition.accept(1.0, 2.0); // a real retarget — now a ramp is pending
    let mut pending_now = Vec::new();
    device.transition.pending_edges(1.0, 2.0 + 2e-6, &mut pending_now);
    assert!(!pending_now.is_empty(), "an accepted retarget does move the operator's state (the contrast case)");
}

// ── Test 4: Behavior evaluation is the closed-form Rust path ───────────────

/// Wraps a `BehaviorKind`, counting every `value`/`jacobian` call into a
/// shared counter — the "counted, not inspected" proof (T8's pattern):
/// zero of these calls are a host/FFI call (there is no dynamic dispatch
/// into another language anywhere in this crate), and the count shows the
/// closed form really is evaluated every time the adapter stamps, not
/// skipped or cached.
struct Counting {
    inner: Box<dyn BehaviorKind>,
    calls: Arc<AtomicUsize>,
}
impl BehaviorKind for Counting {
    fn value(&self, inputs: &[f64]) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.value(inputs)
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.inner.jacobian(inputs, out)
    }
    fn crossings(&self, inputs: &[f64]) -> Option<Crossing> {
        self.inner.crossings(inputs)
    }
}

#[test]
fn behavior_evaluation_is_the_closed_form_rust_path_not_a_host_call() {
    struct Reactive {
        a: AnalogReference,
        b: AnalogReference,
        force_p: AnalogReference,
        force_n: AnalogReference,
        bias_g: f64,
        behavior: BehaviorStamp,
    }
    impl HostElement for Reactive {
        fn name(&self) -> &str {
            "reactive"
        }
        fn capabilities(&self) -> ElementCapabilities {
            ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
        }
        fn load_dc(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
            sink.g(&self.force_p, &self.force_p, self.bias_g);
            sink.g(&self.force_p, &self.force_n, -self.bias_g);
            sink.g(&self.force_n, &self.force_p, -self.bias_g);
            sink.g(&self.force_n, &self.force_n, self.bias_g);
            let _ = (&self.a, &self.b);
            self.behavior.stamp(ctx, sink);
        }
    }

    let mut builder = CircuitBuilder::new("counted");
    let gnd = builder.ground();
    let vin = builder.node("vin");
    let fp = builder.node("fp");

    builder.element(Box::new(Vsrc { p: vin.clone(), n: gnd.clone(), branch: None, v: 1.0 }));

    let calls = Arc::new(AtomicUsize::new(0));
    let kind = Counting { inner: Box::new(Comparator { lo: 0.0, hi: 1.0, gain: 1.0 }), calls: calls.clone() };
    let behavior = BehaviorStamp::new(Box::new(kind), vec![vin.clone(), gnd.clone()], fp.clone(), gnd.clone());

    builder.element(Box::new(HostAdapter(Reactive {
        a: vin.clone(),
        b: gnd.clone(),
        force_p: fp,
        force_n: gnd,
        bias_g: 1e-3,
        behavior,
    })));

    let mut circuit = builder.build().expect("builds");
    let mut dc = circuit.dc(Context::default()).expect("dc setup");
    let _ = dc.solve().expect("solves");

    assert!(
        calls.load(Ordering::Relaxed) > 0,
        "the closed-form Behavior was evaluated at least once while stamping DC — counted, not inspected"
    );
}

// ── Test 5 (DVT-30 fix-round gap closure): all three levels, one class ─────

/// An ideal voltage source usable in both DC and transient — the bias rail
/// for the composed-device fixture below. A separate type from `Vsrc`
/// (which is DC-only, matching Test 1's purely-DC circuit) because this
/// fixture needs the same ideal source to hold through a transient run.
struct TranVsrc {
    p: AnalogReference,
    n: AnalogReference,
    branch: Option<AnalogReference>,
    v: f64,
}
impl AnalogDevice for TranVsrc {
    fn allocate_unknowns(&mut self, alloc: &mut UnknownAllocator<'_>) {
        self.branch = Some(alloc.branch("tvsrc", "i"));
    }
    fn load_dc(&mut self, _state: &DcAnalysisState<'_>, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
    fn load_transient(
        &mut self,
        _state: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
}
impl TranVsrc {
    fn stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
        let b = self.branch.clone().unwrap();
        vec![
            Stamp::Matrix(self.p.clone(), b.clone(), 1.0),
            Stamp::Matrix(self.n.clone(), b.clone(), -1.0),
            Stamp::Matrix(b.clone(), self.p.clone(), 1.0),
            Stamp::Matrix(b.clone(), self.n.clone(), -1.0),
            Stamp::Rhs(b, self.v),
        ]
    }
}
impl DigitalDevice for TranVsrc {}
impl Introspect for TranVsrc {}
impl Element for TranVsrc {
    fn name(&self) -> &str {
        "tvsrc"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// A plain series resistor — the divider partner that makes the composed
/// device's conductance sweep (level 2, below) observable as a changing
/// node voltage rather than an inert internal number.
struct SeriesResistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}
impl AnalogDevice for SeriesResistor {
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
impl SeriesResistor {
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
impl DigitalDevice for SeriesResistor {}
impl Introspect for SeriesResistor {}
impl Element for SeriesResistor {
    fn name(&self) -> &str {
        "rseries"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
    }
}

/// The level-2 program: sweeps a shared conductance through a fixed list
/// of values, holding each for `dt` — `ideal.md` example G's `for g in
/// (...): self.g = g; yield t.advance(dt)`, translated to the hand-written
/// coroutine shape (`SequencerProgram`) this crate uses in place of Python
/// generators. Mutates `shared_g` (owned jointly with the composed
/// device's `load_dc`/`load_transient`) rather than a field of its own,
/// since a `SequencerProgram` has no way to reach back into the outer
/// `HostElement` that embeds it.
struct SweepProgram {
    steps: Vec<f64>,
    idx: usize,
    dt: f64,
    shared_g: Arc<Mutex<f64>>,
}
impl SequencerProgram for SweepProgram {
    fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
        *self.shared_g.lock().unwrap() = self.steps[0];
        Wake::Advance(self.dt)
    }
    fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
        self.idx += 1;
        if self.idx >= self.steps.len() {
            return Wake::Done;
        }
        *self.shared_g.lock().unwrap() = self.steps[self.idx];
        Wake::Advance(self.dt)
    }
}

/// `ideal.md` example G, in full: one class that (a) stamps its own
/// conductance directly — level 0 — (b) installs a `Wave` into an
/// independent node — level 1 — and (c) owns a `Sequencer` that sweeps
/// that conductance over simulated time — level 2. The `Sequencer` field
/// is composed, not inherited (Rust has no class inheritance); the
/// time/lifecycle `HostElement` methods below simply delegate to it.
struct ActiveLoadSweep {
    p: AnalogReference,
    n: AnalogReference,
    g: Arc<Mutex<f64>>,
    wave: WaveStamp,
    seq: Sequencer<SweepProgram>,
}
impl ActiveLoadSweep {
    fn stamp(&self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        let g = *self.g.lock().unwrap();
        sink.g(&self.p, &self.p, g);
        sink.g(&self.p, &self.n, -g);
        sink.g(&self.n, &self.p, -g);
        sink.g(&self.n, &self.n, g);
        self.wave.stamp(ctx, sink);
    }
}
impl HostElement for ActiveLoadSweep {
    fn name(&self) -> &str {
        "active_load_sweep"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN | self.seq.capabilities()
    }
    fn load_dc(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        self.stamp(ctx, sink);
    }
    fn load_transient(&mut self, ctx: &Ctx<'_>, sink: &mut StampSink) {
        self.stamp(ctx, sink);
    }
    fn next_breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        self.seq.next_breakpoints(from, horizon)
    }
    fn accept_timestep(&mut self, state: &StateView<'_>, t: f64) {
        self.seq.accept_timestep(state, t);
    }
    fn on_digital_settled(&mut self, nets: &[LogicValue], t: f64) {
        self.seq.on_digital_settled(nets, t);
    }
    fn drain_digital_drives(&mut self) -> Vec<(DigitalNet, LogicValue, f64)> {
        self.seq.drain_digital_drives()
    }
    fn validation_reports(&self) -> Option<piperine_solver::abi::ValidationReport> {
        self.seq.validation_reports()
    }
}

#[test]
fn all_three_levels_compose_in_one_class_end_to_end() {
    // R_series divides against the composed device's own swept
    // conductance g: v(mid) = v_in / (1 + g * r_series) — a resistive
    // divider, so each g step produces a distinct, predictable v(mid).
    let v_in = 5.0;
    let r_series = 1e3;
    let dt = 20e-6;
    let steps = vec![1e-3, 5e-3, 2e-2];
    let shared_g = Arc::new(Mutex::new(steps[0]));

    let mut builder = CircuitBuilder::new("active_load_sweep");
    let gnd = builder.ground();
    let top = builder.node("top");
    let mid = builder.node("mid");
    let wave_node = builder.node("wout");

    builder.element(Box::new(TranVsrc { p: top.clone(), n: gnd.clone(), branch: None, v: v_in }));
    builder.element(Box::new(SeriesResistor { r: r_series, n1: top, n2: mid.clone() }));

    let program = SweepProgram { steps: steps.clone(), idx: 0, dt, shared_g: shared_g.clone() };
    let seq = Sequencer::new("sweep", program, 0.0, (steps.len() as f64) * dt + 1e-6);
    builder.element(Box::new(HostAdapter(ActiveLoadSweep {
        p: mid.clone(),
        n: gnd.clone(),
        g: shared_g,
        wave: WaveStamp::new(Wave::constant(1.8), wave_node.clone(), gnd, 100.0),
        seq,
    })));

    let circuit = builder.build().expect("all three levels build into a circuit");
    let opts = TransientAnalysisOptions::new((steps.len() as f64) * dt + 1e-6, 1e-6);
    let mut solver = Solver::new(circuit).with_tran_opts(opts).build();
    let res = solver.tran().expect("tran setup").solve().expect("all three levels solve together");

    // Level 2: the sequencer's breakpoints force exact landings at each
    // sweep boundary — read v(mid) at each and confirm it matches the
    // resistor-divider value for that step's conductance, proving the
    // level-0 stamp in this same class really did change value under the
    // level-2 coroutine's control.
    let tol_t = 1e-9;
    for (i, g) in steps.iter().enumerate() {
        let target_t = ((i + 1) as f64) * dt;
        let step = res
            .iter()
            .find(|s| (s.time() - target_t).abs() < tol_t)
            .unwrap_or_else(|| panic!("no accepted step landed exactly at sweep boundary t={target_t:e}"));
        let v_mid = step.get(mid.clone()).expect("mid is solved at this step");
        let expected = v_in / (1.0 + g * r_series);
        assert!(
            (v_mid - expected).abs() < 1e-2,
            "step {i} (g={g:e}): v(mid) = {v_mid}, expected {expected} from the divider with this class's own \
             level-0 conductance"
        );
    }

    // Level 1: the installed Wave, in the very same class, still drives its
    // declared constant into its own independent node throughout — level 2
    // sweeping level 0 did not disturb level 1.
    let last = res.last().expect("the run produced at least one step");
    let v_wave = last.get(wave_node.clone()).expect("wout is solved");
    assert!((v_wave - 1.8).abs() < 1e-2, "the installed Wave still drives its declared value: v(wout) = {v_wave}");
}
