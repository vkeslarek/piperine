//! T24 (DVT-26) — analog crossing waits: `wait(above(net, level))` resumes
//! at the predicted tier-2 crossing, reporting its honest error bound; all
//! three wake forms (time, digital edge, analog crossing) compose in one
//! coroutine; a crossing that never occurs before stop time ends the run
//! cleanly.

use std::sync::{Arc, Mutex};

use piperine_api::sequencer::install::InstallSite;
use piperine_api::sequencer::wait::EdgeDirection;
use piperine_api::sequencer::SequencerControl;
use piperine_api::{HostAdapter, HostElement, Sequencer, SequencerProgram, Wake};
use piperine_ir::events::CrossDir;
use piperine_ir::wave::Wave;
use piperine_solver::abi::{AnalogReference, Context, ElementCapabilities};
use piperine_solver::prelude::{CircuitBuilder, TransientAnalysisOptions};

/// A `drive_voltage`-shaped source (T21's `InstallSite`, reused): a ramp
/// with a known slope, so the analytic crossing instant is known exactly.
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

/// `wait(above(vout, 1.0))` on a 0V -> 2V ramp over `dur` crosses 1.0V at
/// exactly `dur / 2` — the analytic instant this test checks the tier-2
/// prediction against.
#[test]
fn wait_above_resumes_at_the_predicted_crossing_with_an_honest_bound() {
    struct WaitAbove {
        net: AnalogReference,
        level: f64,
        result: Arc<Mutex<Option<(f64, f64, f64)>>>, // (resume_t, t_cross, error_bound)
    }
    impl SequencerProgram for WaitAbove {
        fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
            Wake::WaitCrossing(self.net.clone(), self.level, CrossDir::Rising)
        }
        fn resume(&mut self, t: f64, ctl: &mut SequencerControl) -> Wake {
            let pred = ctl.last_crossing().expect("a crossing wake must carry a Prediction");
            *self.result.lock().unwrap() = Some((t, pred.t_cross, pred.error_bound));
            Wake::Done
        }
    }

    let mut builder = CircuitBuilder::new("crossing");
    let gnd = builder.ground();
    let vout = builder.node("vout");
    let dur = 100e-6;

    let mut site = InstallSite::new(vout.clone(), gnd.clone(), 1e-3);
    site.install(Wave::ramp(0.0, 2.0, dur), 0.0);
    builder.element(Box::new(HostAdapter(RampSource { site })));

    let result = Arc::new(Mutex::new(None));
    builder.element(Box::new(HostAdapter(Sequencer::new(
        "waiter",
        WaitAbove { net: vout, level: 1.0, result: result.clone() },
        0.0,
        dur,
    ))));

    let mut circuit = builder.build().expect("builds");
    let opts = TransientAnalysisOptions::new(dur, dur / 500.0);
    let mut tran = circuit.transient(opts, Context::default()).expect("tran setup");
    let _ = tran.solve().expect("tran solves");

    let (resume_t, t_cross, error_bound) = result.lock().unwrap().expect("the waiter must have resumed");
    let analytic = dur / 2.0;
    assert!(
        (t_cross - analytic).abs() <= error_bound,
        "the predicted crossing {t_cross:.3e} must be within its own published bound {error_bound:.3e} of the \
         analytic instant {analytic:.3e}"
    );
    assert!(resume_t >= t_cross - error_bound, "the sequencer resumes no earlier than it could have detected the crossing");
    assert!(error_bound > 0.0, "the bound must be a real bracket width, never a claimed exact zero");
}

/// A crossing that never occurs before the stop time: the run completes
/// cleanly (no panic, no hang, no error) and the waiter simply never
/// resumes.
#[test]
fn a_crossing_that_never_occurs_ends_the_run_cleanly() {
    struct WaitAboveUnreachable {
        net: AnalogReference,
        resumed: Arc<Mutex<bool>>,
    }
    impl SequencerProgram for WaitAboveUnreachable {
        fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
            // The ramp only reaches 2.0V; asking for 5.0V never happens.
            Wake::WaitCrossing(self.net.clone(), 5.0, CrossDir::Rising)
        }
        fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
            *self.resumed.lock().unwrap() = true;
            Wake::Done
        }
    }

    let mut builder = CircuitBuilder::new("crossing_unreachable");
    let gnd = builder.ground();
    let vout = builder.node("vout");
    let dur = 100e-6;
    let mut site = InstallSite::new(vout.clone(), gnd.clone(), 1e-3);
    site.install(Wave::ramp(0.0, 2.0, dur), 0.0);
    builder.element(Box::new(HostAdapter(RampSource { site })));

    let resumed = Arc::new(Mutex::new(false));
    builder.element(Box::new(HostAdapter(Sequencer::new(
        "waiter",
        WaitAboveUnreachable { net: vout, resumed: resumed.clone() },
        0.0,
        dur,
    ))));

    let mut circuit = builder.build().expect("builds");
    let opts = TransientAnalysisOptions::new(dur, dur / 100.0);
    let mut tran = circuit.transient(opts, Context::default()).expect("tran setup");
    let result = tran.solve();
    assert!(result.is_ok(), "a never-satisfied crossing wait must not fail or hang the analysis: {result:?}");
    assert!(!*resumed.lock().unwrap(), "the waiter must never resume for a crossing that never occurred");
}

// ── All three wake forms compose in one coroutine ───────────────────────────

use piperine_solver::abi::{
    AnalogDevice, DcAnalysisState, DigitalDevice, DigitalNet, DigitalState, Element, EventSink, Introspect,
    LogicValue, Netlist, NodeIdentifier, TransientAnalysisContext, TransientAnalysisState,
};
use piperine_solver::prelude::CircuitInstance;
use std::sync::Arc as StdArc;

/// Flips `net` to `value` at `flip_at`, once.
struct FlipAt {
    net: DigitalNet,
    value: LogicValue,
    flip_at: f64,
    fired: bool,
}
impl AnalogDevice for FlipAt {}
impl DigitalDevice for FlipAt {}
impl Introspect for FlipAt {}
impl Element for FlipAt {
    fn name(&self) -> &str {
        "flip_at"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
    fn accept_timestep(
        &mut self,
        _state: &piperine_solver::abi::CircularArrayBuffer2<f64>,
        t: f64,
        _nets: &[LogicValue],
        sink: &mut dyn EventSink,
    ) {
        if !self.fired && t >= self.flip_at {
            self.fired = true;
            sink.emit(self.net, self.value, 0.0);
        }
    }
}

/// A ramp source over the raw `Element` ABI (level 0, not `HostElement`) so
/// this fixture can live in one `CircuitInstance::from_devices_and_netlist`
/// call alongside the digital `FlipAt` device — mixing analog and digital
/// devices at this construction level needs the raw ABI on both sides.
struct RawRamp {
    node: AnalogReference,
    v0: f64,
    v1: f64,
    dur: f64,
}
impl AnalogDevice for RawRamp {
    fn load_dc(
        &mut self,
        _s: &DcAnalysisState<'_>,
        _c: &Context,
    ) -> Vec<piperine_solver::abi::Stamp<AnalogReference, f64>> {
        self.stamps(0.0)
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<piperine_solver::abi::Stamp<AnalogReference, f64>> {
        self.stamps(t.time)
    }
}
impl RawRamp {
    fn stamps(&self, t: f64) -> Vec<piperine_solver::abi::Stamp<AnalogReference, f64>> {
        let frac = (t / self.dur).clamp(0.0, 1.0);
        let v = self.v0 + (self.v1 - self.v0) * frac;
        let g = 1e3;
        vec![
            piperine_solver::abi::Stamp::Matrix(self.node.clone(), self.node.clone(), g),
            piperine_solver::abi::Stamp::Rhs(self.node.clone(), g * v),
        ]
    }
}
impl DigitalDevice for RawRamp {}
impl Introspect for RawRamp {}
impl Element for RawRamp {
    fn name(&self) -> &str {
        "raw_ramp"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
    }
}

/// `Advance` -> `WaitEdge` -> `WaitCrossing` -> `Done`, recording which wake
/// kind resumed it at each step (by tag) — proving all three sources
/// compose in one coroutine, mapped to `Breakpoint`/`Digital`/`Crossing`
/// respectively.
#[test]
fn all_three_wake_forms_compose_in_one_coroutine() {
    #[derive(Debug, PartialEq)]
    enum Kind {
        Advance,
        Edge,
        Crossing,
    }
    struct ThreeStage {
        clk: DigitalNet,
        vout: AnalogReference,
        stage: usize,
        log: Arc<Mutex<Vec<Kind>>>,
    }
    impl SequencerProgram for ThreeStage {
        fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
            Wake::Advance(10e-6)
        }
        fn resume(&mut self, _t: f64, ctl: &mut SequencerControl) -> Wake {
            match self.stage {
                0 => {
                    self.log.lock().unwrap().push(Kind::Advance);
                    self.stage = 1;
                    Wake::WaitEdge(self.clk, EdgeDirection::Rising)
                }
                1 => {
                    self.log.lock().unwrap().push(Kind::Edge);
                    self.stage = 2;
                    Wake::WaitCrossing(self.vout.clone(), 1.0, CrossDir::Rising)
                }
                _ => {
                    let _ = ctl.last_crossing();
                    self.log.lock().unwrap().push(Kind::Crossing);
                    Wake::Done
                }
            }
        }
    }

    let mut netlist = Netlist::new();
    let vout = netlist.connect_node(NodeIdentifier::Anonymous(0));
    let clk = DigitalNet(0);

    let dur = 100e-6;
    let log = StdArc::new(Mutex::new(Vec::new()));
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(RawRamp { node: vout.clone(), v0: 0.0, v1: 2.0, dur }),
        Box::new(FlipAt { net: clk, value: LogicValue::One, flip_at: 30e-6, fired: false }),
        Box::new(HostAdapter(Sequencer::new(
            "three_stage",
            ThreeStage { clk, vout, stage: 0, log: log.clone() },
            0.0,
            dur,
        ))),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("three_stage", elements, netlist);
    circuit.digital_state = DigitalState::new(1);
    circuit.digital_state.nets[clk.0] = LogicValue::Zero;
    circuit.rebuild_digital_topology();

    let opts = TransientAnalysisOptions::new(dur, dur / 500.0);
    let mut tran = circuit.transient(opts, Context::default()).expect("tran setup");
    let _ = tran.solve().expect("tran solves");

    assert_eq!(
        *log.lock().unwrap(),
        vec![Kind::Advance, Kind::Edge, Kind::Crossing],
        "all three wake sources must fire, in the order the coroutine asked for them"
    );
}
