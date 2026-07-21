// Mixed-signal integration tests.
//
// Tests the boundary between the analog (Newton-Raphson MNA) and digital
// (event-driven) simulation domains through:
//
// - **A→D**: Analog voltage drives digital output (comparators, ADCs).
// - **D→A**: Digital event changes analog stamp (switches, DACs, current sources).
// - **Loop**: Complete A→D→A feedback paths.
// - **Edge cases**: X-state propagation, simultaneous crossings, hysteresis.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_solver::abi::{AnalogReference, Netlist, NodeIdentifier};
use piperine_solver::abi::DcAnalysisState;
use piperine_solver::abi::{AnalogDevice, DigitalDevice, Element, ElementCapabilities, Introspect};
use piperine_solver::abi::{DigitalPorts, EvalCtx, EventSink, QueueSink};
use piperine_solver::abi::{DigitalEvent, DigitalNet, LogicValue};
use piperine_solver::abi::CircularArrayBuffer2;
use piperine_solver::abi::Stamp;
use piperine_solver::prelude::Context;
use piperine_solver::abi::DigitalState;

// ─────────────────────────────── Helpers ─────────────────────────────────────

fn empty_queue() -> BinaryHeap<Reverse<DigitalEvent>> { BinaryHeap::new() }

/// Build a one-row analog history buffer from a flat voltage vector. Pair with
/// [`DcAnalysisState::new`] (with an empty digital snapshot) at the call site.
fn dc_history_from_voltages(voltages: &[f64]) -> CircularArrayBuffer2<f64> {
    use ndarray::Array1;
    let mut st = CircularArrayBuffer2::new(1, voltages.len());
    let row = Array1::from_vec(voltages.to_vec());
    st.push(&row.view());
    st
}

// ─────────────────────────────── Element definitions ──────────────────────────

/// Voltage comparator: reads av[vp] - av[vn], drives digital output.
///
/// Called from the mixed-signal simulation loop on any potential analog crossing
/// (the simulator calls `comb_phase` with the current analog solution).
#[allow(dead_code)]
struct Comparator {
    vp_idx: usize,
    vn_idx: usize,
    out_net: DigitalNet,
    threshold: f64,
    id: usize,
    last_out: LogicValue,
}

impl Comparator {
    fn new(id: usize, vp: usize, vn: usize, out: DigitalNet, thresh: f64) -> Self {
        Self { vp_idx: vp, vn_idx: vn, out_net: out, threshold: thresh, id, last_out: LogicValue::X }
    }
}

impl AnalogDevice for Comparator {}

impl DigitalDevice for Comparator {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: std::slice::from_ref(&self.out_net) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let vdiff = ctx.analog.get(self.vp_idx).copied().unwrap_or(0.0)
            - ctx.analog.get(self.vn_idx).copied().unwrap_or(0.0);
        let out = if vdiff > self.threshold { LogicValue::One } else { LogicValue::Zero };
        if out != self.last_out {
            self.last_out = out;
            sink.emit(self.out_net, out, 0.0);
        }
    }
}

impl Introspect for Comparator {}

impl Element for Comparator {
    fn name(&self) -> &str { "comparator" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG | ElementCapabilities::DIGITAL }
    fn accept_timestep(&mut self, state: &CircularArrayBuffer2<f64>, t: f64, nets: &[LogicValue], sink: &mut dyn EventSink) {
        let latest = state.latest().unwrap();
        let eval_ctx = EvalCtx { time: t, nets, analog: latest.as_slice().unwrap() };
        self.comb_phase(&eval_ctx, sink);
    }
}



/// Schmitt trigger (hysteresis comparator).
#[allow(dead_code)]
struct SchmittTrigger {
    v_idx: usize,
    out_net: DigitalNet,
    thresh_high: f64,
    thresh_low: f64,
    state: LogicValue,
    id: usize,
}

impl SchmittTrigger {
    fn new(id: usize, v_idx: usize, out_net: DigitalNet, thresh_low: f64, thresh_high: f64) -> Self {
        Self { v_idx, out_net, thresh_high, thresh_low, state: LogicValue::Zero, id }
    }
}

impl AnalogDevice for SchmittTrigger {}

impl DigitalDevice for SchmittTrigger {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: std::slice::from_ref(&self.out_net) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let v = ctx.analog.get(self.v_idx).copied().unwrap_or(0.0);
        let new_state = match self.state {
            LogicValue::Zero if v >= self.thresh_high => LogicValue::One,
            LogicValue::One  if v <= self.thresh_low  => LogicValue::Zero,
            _ => return,
        };
        self.state = new_state;
        sink.emit(self.out_net, new_state, 0.0);
    }
}

impl Introspect for SchmittTrigger {}

impl Element for SchmittTrigger {
    fn name(&self) -> &str { "schmitt" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG | ElementCapabilities::DIGITAL }
    fn accept_timestep(&mut self, state: &CircularArrayBuffer2<f64>, t: f64, nets: &[LogicValue], sink: &mut dyn EventSink) {
        let latest = state.latest().unwrap();
        let eval_ctx = EvalCtx { time: t, nets, analog: latest.as_slice().unwrap() };
        self.comb_phase(&eval_ctx, sink);
    }
}



/// Analog switch: digital control input gates conductance between two analog nodes.
struct AnalogSwitch {
    ctrl_net: DigitalNet,
    node_a: Option<AnalogReference>,
    node_b: Option<AnalogReference>,
    conductance: f64,
    closed: bool,
}

impl AnalogSwitch {
    fn new(ctrl: DigitalNet, a: Option<AnalogReference>, b: Option<AnalogReference>, g: f64) -> Self {
        Self { ctrl_net: ctrl, node_a: a, node_b: b, conductance: g, closed: false }
    }
}

impl AnalogDevice for AnalogSwitch {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            if !self.closed { return Vec::new(); }
            let g = self.conductance;
            let (Some(a), Some(b)) = (self.node_a.clone(), self.node_b.clone()) else { return Vec::new(); };
            vec![
                Stamp::Matrix(a.clone(), a.clone(),  g),
                Stamp::Matrix(a.clone(), b.clone(), -g),
                Stamp::Matrix(b.clone(), a.clone(), -g),
                Stamp::Matrix(b.clone(), b.clone(),  g),
            ]
        }
}

impl DigitalDevice for AnalogSwitch {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.ctrl_net), outputs: &[] }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {
        self.closed = ctx.nets[self.ctrl_net.0] == LogicValue::One;
    }
}

impl Introspect for AnalogSwitch {}

impl Element for AnalogSwitch {
    fn name(&self) -> &str { "analog_switch" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG | ElementCapabilities::DIGITAL }
}



/// Digitally-gated current source: when enabled, injects Ibias into node_p / out of node_n.
struct GatedCurrentSource {
    enable_net: DigitalNet,
    node_p: Option<AnalogReference>,
    node_n: Option<AnalogReference>,
    ibias: f64,
    enabled: bool,
}

impl GatedCurrentSource {
    fn new(enable: DigitalNet, p: Option<AnalogReference>, n: Option<AnalogReference>, i: f64) -> Self {
        Self { enable_net: enable, node_p: p, node_n: n, ibias: i, enabled: false }
    }
}

impl AnalogDevice for GatedCurrentSource {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            if !self.enabled { return Vec::new(); }
            let mut stamps = Vec::new();
            if let Some(p) = &self.node_p { stamps.push(Stamp::Rhs(p.clone(),  self.ibias)); }
            if let Some(n) = &self.node_n { stamps.push(Stamp::Rhs(n.clone(), -self.ibias)); }
            stamps
        }
}

impl DigitalDevice for GatedCurrentSource {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.enable_net), outputs: &[] }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {
        self.enabled = ctx.nets[self.enable_net.0] == LogicValue::One;
    }
}

impl Introspect for GatedCurrentSource {}

impl Element for GatedCurrentSource {
    fn name(&self) -> &str { "gated_isrc" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG | ElementCapabilities::DIGITAL }
}



/// Level-sensitive analog sample-and-hold. On posedge(clk), captures av[sample_idx].
#[allow(dead_code)]
struct SampleAndHold {
    clk_net: DigitalNet,
    sample_idx: usize,
    last_clk: LogicValue,
    held_value: f64,
    out_ref: Option<AnalogReference>,
    id: usize,
}

impl SampleAndHold {
    fn new(id: usize, clk: DigitalNet, sample_idx: usize, out: Option<AnalogReference>) -> Self {
        Self { clk_net: clk, sample_idx, last_clk: LogicValue::Zero, held_value: 0.0, out_ref: out, id }
    }
}

impl AnalogDevice for SampleAndHold {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            if let Some(r) = &self.out_ref {
                vec![Stamp::Rhs(r.clone(), self.held_value)]
            } else {
                Vec::new()
            }
        }
}

impl DigitalDevice for SampleAndHold {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.clk_net), outputs: &[] }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {
        let clk = ctx.nets[self.clk_net.0];
        let posedge = self.last_clk != LogicValue::One && clk == LogicValue::One;
        self.last_clk = clk;
        if posedge {
            self.held_value = ctx.analog.get(self.sample_idx).copied().unwrap_or(0.0);
        }
    }

        // Drives a Thevenin source (v_held with 0Ω) — simplified: just RHS stamp.

}

impl Introspect for SampleAndHold {}

impl Element for SampleAndHold {
    fn name(&self) -> &str { "sah" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG | ElementCapabilities::DIGITAL }
}



/// Comparator with memory: drives output only when it changes.
/// Tracks `last_trigger_time` to test glitch suppression.
#[allow(dead_code)]
struct GlitchTestDevice {
    v_idx: usize,
    out_net: DigitalNet,
    threshold: f64,
    last_out: LogicValue,
    event_count: usize,
    id: usize,
}

impl GlitchTestDevice {
    fn new(id: usize, v_idx: usize, out: DigitalNet, thresh: f64) -> Self {
        Self { v_idx, out_net: out, threshold: thresh, last_out: LogicValue::X, event_count: 0, id }
    }
}

impl AnalogDevice for GlitchTestDevice {}

impl DigitalDevice for GlitchTestDevice {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: std::slice::from_ref(&self.out_net) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let v = ctx.analog.get(self.v_idx).copied().unwrap_or(0.0);
        let out = if v > self.threshold { LogicValue::One } else { LogicValue::Zero };
        if out != self.last_out {
            self.last_out = out;
            self.event_count += 1;
            sink.emit(self.out_net, out, 0.0);
        }
    }
}

impl Introspect for GlitchTestDevice {}

impl Element for GlitchTestDevice {
    fn name(&self) -> &str { "glitch_test" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
}



// ─────────────────────────────── Context stub ─────────────────────────────────

fn dummy_context() -> Context { Context::default() }

// ─────────────────────────────── A → D Tests ─────────────────────────────────

/// Comparator fires when analog voltage crosses threshold.
#[test]
fn test_a2d_comparator_above_threshold() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    // vp=0.8, vn=0.0 → diff=0.8 > 0.5 → One
    let ctx = EvalCtx { time: 0.0, nets: &[], analog: &[0.8, 0.0] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::One);
    assert_eq!(ev.net, DigitalNet(10));
}

/// Comparator: initial call below threshold fires X→Zero, then stays silent.
#[test]
fn test_a2d_comparator_below_threshold() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    // First call: X → Zero (initial state transition)
    let ctx = EvalCtx { time: 0.0, nets: &[], analog: &[0.3, 0.0] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 1, "initial X→Zero fires one event");
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::Zero);
    // Second call: same voltage — no new event (Zero stays Zero)
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[0.3, 0.0] };
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert!(q.is_empty(), "no event when voltage stays below threshold");
}

/// Comparator outputs 0 after being above threshold and voltage drops below.
#[test]
fn test_a2d_comparator_hysteresis_less_crossing() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    // Rising: fires One
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[0.8, 0.0] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert_eq!(q.pop().map(|Reverse(e)| e.value), Some(LogicValue::One));
    // Falling: fires Zero
    let ctx = EvalCtx { time: 2e-9, nets: &[], analog: &[0.2, 0.0] };
    let mut sink = QueueSink::new(&mut q, 2e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert_eq!(q.pop().map(|Reverse(e)| e.value), Some(LogicValue::Zero));
}

/// Calling comparator twice at same voltage level does not re-fire.
#[test]
fn test_a2d_comparator_no_repeat_fire() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[0.9, 0.0] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 1); q.clear();
    // Same voltage, second call — no new event
    let ctx = EvalCtx { time: 2e-9, nets: &[], analog: &[0.9, 0.0] };
    let mut sink = QueueSink::new(&mut q, 2e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert!(q.is_empty(), "should not fire again at same level");
}

/// Schmitt trigger only fires when voltage exceeds thresh_high (rising).
#[test]
fn test_schmitt_rising_fires_above_high() {
    let mut st = SchmittTrigger::new(0, 0, DigitalNet(5), 0.3, 0.7);
    let mut q = empty_queue();
    let mut seq = 0u64;
    // V=0.5: between thresholds → no fire (starting at Zero)
    let ctx = EvalCtx { time: 0.0, nets: &[], analog: &[0.5] };
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    st.comb_phase(&ctx, &mut sink);
    assert!(q.is_empty(), "0.5 < 0.7, no rising edge");
    // V=0.8: above thresh_high → fire One
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[0.8] };
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    st.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::One);
}

/// Schmitt trigger: voltage in hysteresis band after rising does not fire again.
#[test]
fn test_schmitt_hysteresis_suppresses_glitch() {
    let mut st = SchmittTrigger::new(0, 0, DigitalNet(5), 0.3, 0.7);
    let mut q = empty_queue();
    let mut seq = 0u64;
    // Assert high
    let ctx = EvalCtx { time: 0.0, nets: &[], analog: &[0.9] };
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    st.comb_phase(&ctx, &mut sink); q.clear();
    // Voltage dips into band (below high, above low) — should NOT fire
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[0.5] };
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    st.comb_phase(&ctx, &mut sink);
    assert!(q.is_empty(), "in hysteresis band — no output change");
}

/// Schmitt trigger fires falling only below thresh_low.
#[test]
fn test_schmitt_falling_fires_below_low() {
    let mut st = SchmittTrigger::new(0, 0, DigitalNet(5), 0.3, 0.7);
    let mut q = empty_queue();
    let mut seq = 0u64;
    // Set high first
    let ctx = EvalCtx { time: 0.0, nets: &[], analog: &[0.9] };
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    st.comb_phase(&ctx, &mut sink); q.clear();
    // Voltage at 0.2 < 0.3 → fire Zero
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[0.2] };
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    st.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::Zero);
}

/// Comparator with differential inputs: vn > vp → output Zero.
/// Then vp swings above vn → output One. Tests both transitions.
#[test]
fn test_a2d_differential_input() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.0);
    let mut q = empty_queue();
    let mut seq = 0u64;

    // vp=0.3, vn=0.8 → diff=-0.5 < 0 → Zero (from X, so fires)
    let ctx = EvalCtx { time: 0.0, nets: &[], analog: &[0.3, 0.8] };
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::Zero, "negative diff → Zero");

    // vp=1.2, vn=0.4 → diff=0.8 > 0 → One
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[1.2, 0.4] };
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::One, "positive diff → One");
}

/// Comparator at exact threshold: below (exclusive).
#[test]
fn test_a2d_comparator_exact_threshold() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    // Exactly at threshold — 0.5 > 0.5 is false → Zero
    let ctx = EvalCtx { time: 0.0, nets: &[], analog: &[0.5, 0.0] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    // X → Zero: should fire
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::Zero, "strict > : 0.5 is not above 0.5");
}

/// Multiple comparators triggered by the same analog vector at the same time.
#[test]
fn test_a2d_multiple_comparators_simultaneous() {
    let mut c1 = Comparator::new(1, 0, 1, DigitalNet(10), 0.2);
    let mut c2 = Comparator::new(2, 0, 1, DigitalNet(11), 0.8);
    let mut q = empty_queue();
    let av = [1.0_f64, 0.0]; // diff = 1.0
    let ctx = EvalCtx { time: 5e-9, nets: &[], analog: &av };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 5e-9, 0, &mut seq);
    c1.comb_phase(&ctx, &mut sink);
    c2.comb_phase(&ctx, &mut sink);
    assert_eq!(q.len(), 2);
    // Both should output One (1.0 > 0.2 and 1.0 > 0.8)
    let events: Vec<_> = q.into_iter().map(|Reverse(e)| e.value).collect();
    assert!(events.iter().all(|&v| v == LogicValue::One));
}

// ─────────────────────────────── D → A Tests ─────────────────────────────────

/// Analog switch: open by default, no stamps.
#[test]
fn test_d2a_switch_open_no_stamps() {
    let mut sw = AnalogSwitch::new(DigitalNet(0), None, None, 0.1);
    let __hist = dc_history_from_voltages(&[0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    let stamps = sw.load_dc(&state, &dummy_context());
    assert!(stamps.is_empty(), "open switch: no stamps");
}

/// Analog switch: closing it makes load_dc return 4 stamps (2×2 G matrix).
#[test]
fn test_d2a_switch_closed_stamps_conductance() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let b = netlist.connect_node(NodeIdentifier::Anonymous(2));

    let mut sw = AnalogSwitch::new(DigitalNet(0), Some(a), Some(b), 1e3);

    // Force switch closed via comb_phase
    let nets = [LogicValue::One];
    let mut eq = empty_queue();
    let ctx = EvalCtx { time: 0.0, nets: &nets, analog: &[] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);

    let __hist = dc_history_from_voltages(&[1.0, 0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    let stamps = sw.load_dc(&state, &dummy_context());

    assert_eq!(stamps.len(), 4, "closed switch: 4 matrix stamps");

    // Verify diagonal stamps are +G and off-diagonal are -G
    let values: Vec<f64> = stamps.iter().map(|s| match s {
        Stamp::Matrix(_, _, v) => *v,
        Stamp::Rhs(_, v) => *v,
    }).collect();
    let pos: Vec<f64> = values.iter().filter(|&&v| v > 0.0).cloned().collect();
    let neg: Vec<f64> = values.iter().filter(|&&v| v < 0.0).cloned().collect();
    assert_eq!(pos.len(), 2, "two positive conductance stamps");
    assert_eq!(neg.len(), 2, "two negative conductance stamps");
    assert!((pos[0] - 1e3).abs() < 1e-9, "G = 1e3");
    assert!((neg[0] + 1e3).abs() < 1e-9, "-G = -1e3");
}

/// Switch: toggle open → closed → open. Stamps appear and disappear.
#[test]
fn test_d2a_switch_toggle_stamps() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(10));
    let b = netlist.connect_node(NodeIdentifier::Anonymous(11));
    let mut sw = AnalogSwitch::new(DigitalNet(0), Some(a), Some(b), 100.0);
    let __hist = dc_history_from_voltages(&[5.0, 0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);

    // Open → no stamps
    assert!(sw.load_dc(&state, &dummy_context()).is_empty());

    let mut eq = empty_queue();
    let mut seq = 0u64;

    // Close
    let nets = [LogicValue::One];
    let ctx = EvalCtx { time: 0.0, nets: &nets, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);
    assert_eq!(sw.load_dc(&state, &dummy_context()).len(), 4);

    // Re-open
    let nets = [LogicValue::Zero];
    let ctx = EvalCtx { time: 1e-9, nets: &nets, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 1e-9, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);
    assert!(sw.load_dc(&state, &dummy_context()).is_empty());
}

/// Gated current source: disabled → no stamps.
#[test]
fn test_d2a_gated_isrc_disabled_no_stamps() {
    let mut src = GatedCurrentSource::new(DigitalNet(0), None, None, 1e-3);
    let __hist = dc_history_from_voltages(&[]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    assert!(src.load_dc(&state, &dummy_context()).is_empty());
}

/// Gated current source: enabled → Ibias injected into p, extracted from n.
#[test]
fn test_d2a_gated_isrc_enabled_stamps() {
    let mut netlist = Netlist::new();
    let p = netlist.connect_node(NodeIdentifier::Anonymous(20));
    let n = netlist.connect_node(NodeIdentifier::Anonymous(21));
    let ibias = 5e-3;
    let mut src = GatedCurrentSource::new(DigitalNet(0), Some(p.clone()), Some(n.clone()), ibias);

    // Enable
    let nets = [LogicValue::One];
    let mut eq = empty_queue();
    let ctx = EvalCtx { time: 0.0, nets: &nets, analog: &[] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    src.comb_phase(&ctx, &mut sink);

    let __hist = dc_history_from_voltages(&[0.0, 0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    let stamps = src.load_dc(&state, &dummy_context());
    assert_eq!(stamps.len(), 2);

    // p gets +ibias, n gets -ibias
    let rhs_vals: Vec<f64> = stamps.iter().map(|s| match s { Stamp::Rhs(_, v) => *v, _ => 0.0 }).collect();
    assert!(rhs_vals.contains(&ibias), "+Ibias on p");
    assert!(rhs_vals.contains(&-ibias), "-Ibias on n");
}

/// X-state digital input to switch: switch remains open (safe fail).
#[test]
fn test_d2a_switch_x_state_stays_open() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(30));
    let b = netlist.connect_node(NodeIdentifier::Anonymous(31));
    let mut sw = AnalogSwitch::new(DigitalNet(0), Some(a), Some(b), 100.0);

    // X input — switch should remain open (conservative)
    let nets = [LogicValue::X];
    let mut eq = empty_queue();
    let ctx = EvalCtx { time: 0.0, nets: &nets, analog: &[] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);
    let __hist = dc_history_from_voltages(&[1.0, 0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    assert!(sw.load_dc(&state, &dummy_context()).is_empty(), "X state → open switch (safe)");
}

/// Z-state digital input to switch: also stays open.
#[test]
fn test_d2a_switch_z_state_stays_open() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(32));
    let b = netlist.connect_node(NodeIdentifier::Anonymous(33));
    let mut sw = AnalogSwitch::new(DigitalNet(0), Some(a), Some(b), 100.0);

    let nets = [LogicValue::Z];
    let mut eq = empty_queue();
    let ctx = EvalCtx { time: 0.0, nets: &nets, analog: &[] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);
    let __hist = dc_history_from_voltages(&[1.0, 0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    assert!(sw.load_dc(&state, &dummy_context()).is_empty(), "Z state → open switch (safe)");
}

// ─────────────────────────────── Sample-and-hold ────────────────────────────

/// SAH: captures correct voltage on posedge.
#[test]
fn test_sah_captures_on_posedge() {
    let mut netlist = Netlist::new();
    let out = netlist.connect_node(NodeIdentifier::Anonymous(40));
    let mut sah = SampleAndHold::new(0, DigitalNet(0), 2, Some(out));

    let mut q = empty_queue();

    // clk 0→1, analog[2] = 1.23 V
    let nets = [LogicValue::One];
    let av   = [0.0, 0.0, 1.23];
    let ctx = EvalCtx { time: 1e-9, nets: &nets, analog: &av };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    sah.comb_phase(&ctx, &mut sink);

    // Held value should be 1.23
    assert!((sah.held_value - 1.23).abs() < 1e-12);

    let __hist = dc_history_from_voltages(&[0.0, 0.0, 1.23]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    let stamps = sah.load_dc(&state, &dummy_context());
    assert_eq!(stamps.len(), 1);
    if let Stamp::Rhs(_, v) = &stamps[0] {
        assert!((v - 1.23).abs() < 1e-12);
    }
}

/// SAH: stays at previous value when clock is low.
#[test]
fn test_sah_holds_when_clock_low() {
    let mut netlist = Netlist::new();
    let out = netlist.connect_node(NodeIdentifier::Anonymous(41));
    let mut sah = SampleAndHold::new(0, DigitalNet(0), 0, Some(out));

    let mut q = empty_queue();
    let mut seq = 0u64;
    // Capture 2.5 V on posedge
    let nets = [LogicValue::One];
    let ctx = EvalCtx { time: 1e-9, nets: &nets, analog: &[2.5] };
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    sah.comb_phase(&ctx, &mut sink);
    assert!((sah.held_value - 2.5).abs() < 1e-12);

    // Clock goes low, analog changes — hold should not update
    let nets = [LogicValue::Zero];
    let ctx = EvalCtx { time: 2e-9, nets: &nets, analog: &[9.9] };
    let mut sink = QueueSink::new(&mut q, 2e-9, 0, &mut seq);
    sah.comb_phase(&ctx, &mut sink);
    assert!((sah.held_value - 2.5).abs() < 1e-12, "hold unchanged while clk=0");

    // Clock stays high (level, not edge) — no second posedge
    let nets = [LogicValue::One];
    let ctx = EvalCtx { time: 3e-9, nets: &nets, analog: &[9.9] };
    let mut sink = QueueSink::new(&mut q, 3e-9, 0, &mut seq);
    sah.comb_phase(&ctx, &mut sink);
    // last_clk was Zero (from prev call), so this IS a posedge — now captures 9.9
    assert!((sah.held_value - 9.9).abs() < 1e-12, "posedge again captures new value");
}

/// SAH: multiple posedges each capture the latest analog value.
#[test]
fn test_sah_sequential_samples() {
    let mut sah = SampleAndHold::new(0, DigitalNet(0), 0, None);
    let mut q = empty_queue();
    let mut seq = 0u64;
    let voltages = [0.1, 0.5, 0.9, 1.3, 1.7];

    for &v in &voltages {
        let av = [v];
        let nets_lo = [LogicValue::Zero];
        let ctx = EvalCtx { time: 0.0, nets: &nets_lo, analog: &av };
        let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
        sah.comb_phase(&ctx, &mut sink);  // low — no sample
        let nets_hi = [LogicValue::One];
        let ctx = EvalCtx { time: 0.0, nets: &nets_hi, analog: &av };
        let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
        sah.comb_phase(&ctx, &mut sink);  // posedge — sample
        assert!((sah.held_value - v).abs() < 1e-12, "captured {}", v);
    }
}

// ─────────────────────────────── A→D→A Loop Tests ────────────────────────────

/// Full loop: analog voltage → comparator → switch → analog back.
///
/// Verifies that when analog rises above threshold:
/// 1. Comparator fires `One`.
/// 2. Switch closes (processes the event).
/// 3. `load_dc` returns stamps (feedback path active).
#[test]
fn test_loop_comparator_closes_switch() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(50));
    let b = netlist.connect_node(NodeIdentifier::Anonymous(51));
    let ctrl_net = DigitalNet(0);

    let mut cmp = Comparator::new(0, 0, 1, ctrl_net, 0.5);
    let mut sw  = AnalogSwitch::new(ctrl_net, Some(a), Some(b), 100.0);

    // Analog: vp=0.8, vn=0.0 → diff > threshold
    let av = [0.8_f64, 0.0];
    let mut q = empty_queue();
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &av };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);

    // Dequeue event, apply to digital state, forward to switch
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::One);

    let nets = [ev.value];
    let mut eq = empty_queue();
    let ctx = EvalCtx { time: 1e-9, nets: &nets, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 1e-9, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);

    // Verify switch now contributes analog stamps
    let __hist = dc_history_from_voltages(&[0.8, 0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    let stamps = sw.load_dc(&state, &dummy_context());
    assert_eq!(stamps.len(), 4, "feedback: switch closed, analog path active");
}

/// Loop: voltage drops → comparator fires Zero → switch opens → stamps gone.
#[test]
fn test_loop_comparator_opens_switch() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(52));
    let b = netlist.connect_node(NodeIdentifier::Anonymous(53));
    let ctrl_net = DigitalNet(0);

    let mut cmp = Comparator::new(0, 0, 1, ctrl_net, 0.5);
    let mut sw  = AnalogSwitch::new(ctrl_net, Some(a), Some(b), 100.0);

    let mut q = empty_queue();
    let mut eq = empty_queue();
    let mut seq = 0u64;

    // First: raise voltage → close switch
    let ctx = EvalCtx { time: 1e-9, nets: &[], analog: &[0.9, 0.0] };
    let mut sink = QueueSink::new(&mut q, 1e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    let Reverse(e1) = q.pop().unwrap();
    let nets1 = [e1.value];
    let ctx = EvalCtx { time: 1e-9, nets: &nets1, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 1e-9, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);
    let __hist = dc_history_from_voltages(&[0.9, 0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);
    assert_eq!(sw.load_dc(&state, &dummy_context()).len(), 4);

    // Then: lower voltage → open switch
    let ctx = EvalCtx { time: 2e-9, nets: &[], analog: &[0.1, 0.0] };
    let mut sink = QueueSink::new(&mut q, 2e-9, 0, &mut seq);
    cmp.comb_phase(&ctx, &mut sink);
    let Reverse(e2) = q.pop().unwrap();
    assert_eq!(e2.value, LogicValue::Zero);
    let nets2 = [e2.value];
    let ctx = EvalCtx { time: 2e-9, nets: &nets2, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 2e-9, 0, &mut seq);
    sw.comb_phase(&ctx, &mut sink);
    assert!(sw.load_dc(&state, &dummy_context()).is_empty(), "switch re-opened");
}

/// Glitch test: rapid voltage swings around threshold → only real transitions fire events.
#[test]
fn test_glitch_suppression_rapid_transitions() {
    let mut dev = GlitchTestDevice::new(0, 0, DigitalNet(5), 0.5);
    let mut q = empty_queue();
    let mut seq = 0u64;

    // Sequence: 0.8 → 0.3 → 0.9 → 0.2 → 0.7 (5 samples, 4 threshold crossings)
    let voltages = [0.8, 0.3, 0.9, 0.2, 0.7];
    for &v in &voltages {
        let av = [v];
        let ctx = EvalCtx { time: 0.0, nets: &[], analog: &av };
        let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
        dev.comb_phase(&ctx, &mut sink);
    }

    // Events: X→1, 1→0, 0→1, 1→0, 0→1 = 5 crossings (including initial X→)
    let expected = 5;
    assert_eq!(dev.event_count, expected, "expected {} threshold crossings", expected);
    assert_eq!(q.len(), expected);
}

/// DigitalState integration: comparator drives inverter chain via DigitalState.
#[test]
fn test_a2d_drives_digital_chain() {
    // Comparator fires One on net 0.
    // Inverter inverts net 0 → net 1.
    // Verify net 1 becomes Zero after propagation.

    #[allow(dead_code)]
    struct SimpleInverter { input: DigitalNet, output: DigitalNet, id: usize }
    impl AnalogDevice for SimpleInverter {}
    impl DigitalDevice for SimpleInverter {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.input), outputs: std::slice::from_ref(&self.output) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
                let out = match ctx.nets[self.input.0] {
                    LogicValue::One  => LogicValue::Zero,
                    LogicValue::Zero => LogicValue::One,
                    _                => LogicValue::X,
                };
                sink.emit(self.output, out, 0.0);
            }

}
    impl Introspect for SimpleInverter {}
    impl Element for SimpleInverter {
    fn name(&self) -> &str { "inv" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
}



    // Comparator fires via analog: schedule its output event manually
    // (simulating what the simulator does after comb_phase with av)
    let mut state = DigitalState::new(2);

    // Inject comparator result: net 0 = One at t=1ns
    state.schedule(DigitalEvent { time: 1e-9, net: DigitalNet(0), value: LogicValue::One, source: 99, seq: 0 });

    let mut devices: Vec<Box<dyn Element>> = vec![
        Box::new(SimpleInverter { input: DigitalNet(0), output: DigitalNet(1), id: 0 }),
    ];

    state.evaluate_until_stable(1e-9, &mut devices, Default::default(), &[]).unwrap();

    assert_eq!(state.nets[0], LogicValue::One,  "comparator output = 1");
    assert_eq!(state.nets[1], LogicValue::Zero, "inverter output = 0");
}

/// D2A: multiple current sources, only enabled ones contribute stamps.
#[test]
fn test_d2a_selective_current_sources() {
    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(60));

    let mut src0 = GatedCurrentSource::new(DigitalNet(0), Some(node.clone()), None, 1e-3);
    let mut src1 = GatedCurrentSource::new(DigitalNet(1), Some(node.clone()), None, 2e-3);
    let mut src2 = GatedCurrentSource::new(DigitalNet(2), Some(node.clone()), None, 4e-3);

    let __hist = dc_history_from_voltages(&[0.0]);
    let state = DcAnalysisState::new(&__hist, &[], 1.0);

    // Enable only src0 and src2
    let mut eq = empty_queue();
    let mut seq = 0u64;
    let nets0 = [LogicValue::One,  LogicValue::X, LogicValue::X];
    let ctx = EvalCtx { time: 0.0, nets: &nets0, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    src0.comb_phase(&ctx, &mut sink);
    let nets1 = [LogicValue::X, LogicValue::Zero, LogicValue::X];
    let ctx = EvalCtx { time: 0.0, nets: &nets1, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    src1.comb_phase(&ctx, &mut sink);
    let nets2 = [LogicValue::X,  LogicValue::X, LogicValue::One];
    let ctx = EvalCtx { time: 0.0, nets: &nets2, analog: &[] };
    let mut sink = QueueSink::new(&mut eq, 0.0, 0, &mut seq);
    src2.comb_phase(&ctx, &mut sink);

    let total_stamps = src0.load_dc(&state, &dummy_context()).len()
        + src1.load_dc(&state, &dummy_context()).len()
        + src2.load_dc(&state, &dummy_context()).len();

    // src0: 1 stamp (Rhs on node, None for gnd side)
    // src1: 0 stamps (disabled)
    // src2: 1 stamp
    assert_eq!(total_stamps, 2, "only enabled sources contribute stamps");
}
