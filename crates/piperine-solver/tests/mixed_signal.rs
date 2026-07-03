//! Mixed-signal integration tests.
//!
//! Tests the boundary between the analog (Newton-Raphson MNA) and digital
//! (event-driven) simulation domains through:
//!
//! - **A→D**: Analog voltage drives digital output (comparators, ADCs).
//! - **D→A**: Digital event changes analog stamp (switches, DACs, current sources).
//! - **Loop**: Complete A→D→A feedback paths.
//! - **Edge cases**: X-state propagation, simultaneous crossings, hysteresis.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use piperine_solver::analog::{AnalogReference, Netlist, NodeIdentifier};
use piperine_solver::analysis::dc::DcAnalysisState;
use piperine_solver::device::Device;
use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};
use piperine_solver::math::circular_array::CircularArrayBuffer2;
use piperine_solver::math::linear::Stamp;
use piperine_solver::solver::Context;
use piperine_solver::topology::DigitalState;

// ─────────────────────────────── Helpers ─────────────────────────────────────

fn empty_queue() -> BinaryHeap<Reverse<DigitalEvent>> { BinaryHeap::new() }

/// Build a one-row `DcAnalysisState` from a flat voltage vector.
fn dc_state_from_voltages(voltages: &[f64]) -> DcAnalysisState {
    use ndarray::Array1;
    let mut st = CircularArrayBuffer2::new(1, voltages.len());
    let row = Array1::from_vec(voltages.to_vec());
    st.push(&row.view());
    st
}

// ─────────────────────────────── Device definitions ──────────────────────────

/// Voltage comparator: reads av[vp] - av[vn], drives digital output.
///
/// Called from the mixed-signal simulation loop on any potential analog crossing
/// (the simulator calls `eval_discrete` with the current analog solution).
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

impl Device for Comparator {
    fn device_name(&self) -> &str { "comparator" }
    fn digital_output_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.out_net) }

    fn eval_discrete(
        &mut self, t: f64, _nets: &[LogicValue], av: &[f64],
        q: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let vdiff = av.get(self.vp_idx).copied().unwrap_or(0.0)
            - av.get(self.vn_idx).copied().unwrap_or(0.0);
        let out = if vdiff > self.threshold { LogicValue::One } else { LogicValue::Zero };
        if out != self.last_out {
            self.last_out = out;
            q.push(Reverse(DigitalEvent { time: t, net: self.out_net, value: out, source: self.id, seq: 0 }));
        }
    }
}

/// Schmitt trigger (hysteresis comparator).
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

impl Device for SchmittTrigger {
    fn device_name(&self) -> &str { "schmitt" }
    fn digital_output_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.out_net) }

    fn eval_discrete(
        &mut self, t: f64, _nets: &[LogicValue], av: &[f64],
        q: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let v = av.get(self.v_idx).copied().unwrap_or(0.0);
        let new_state = match self.state {
            LogicValue::Zero if v >= self.thresh_high => LogicValue::One,
            LogicValue::One  if v <= self.thresh_low  => LogicValue::Zero,
            _ => return,
        };
        self.state = new_state;
        q.push(Reverse(DigitalEvent { time: t, net: self.out_net, value: new_state, source: self.id, seq: 0 }));
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

impl Device for AnalogSwitch {
    fn device_name(&self) -> &str { "analog_switch" }
    fn digital_input_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.ctrl_net) }

    fn eval_discrete(
        &mut self, _t: f64, nets: &[LogicValue], _av: &[f64],
        _q: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        self.closed = nets[self.ctrl_net.0] == LogicValue::One;
    }

    fn load_dc(&mut self, _s: &DcAnalysisState, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
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

impl Device for GatedCurrentSource {
    fn device_name(&self) -> &str { "gated_isrc" }
    fn digital_input_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.enable_net) }

    fn eval_discrete(
        &mut self, _t: f64, nets: &[LogicValue], _av: &[f64],
        _q: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        self.enabled = nets[self.enable_net.0] == LogicValue::One;
    }

    fn load_dc(&mut self, _s: &DcAnalysisState, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        if !self.enabled { return Vec::new(); }
        let mut stamps = Vec::new();
        if let Some(p) = &self.node_p { stamps.push(Stamp::Rhs(p.clone(),  self.ibias)); }
        if let Some(n) = &self.node_n { stamps.push(Stamp::Rhs(n.clone(), -self.ibias)); }
        stamps
    }
}

/// Level-sensitive analog sample-and-hold. On posedge(clk), captures av[sample_idx].
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

impl Device for SampleAndHold {
    fn device_name(&self) -> &str { "sah" }
    fn digital_input_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.clk_net) }

    fn eval_discrete(
        &mut self, _t: f64, nets: &[LogicValue], av: &[f64],
        _q: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let clk = nets[self.clk_net.0];
        let posedge = self.last_clk != LogicValue::One && clk == LogicValue::One;
        self.last_clk = clk;
        if posedge {
            self.held_value = av.get(self.sample_idx).copied().unwrap_or(0.0);
        }
    }

    // Drives a Thevenin source (v_held with 0Ω) — simplified: just RHS stamp.
    fn load_dc(&mut self, _s: &DcAnalysisState, _ctx: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        if let Some(r) = &self.out_ref {
            vec![Stamp::Rhs(r.clone(), self.held_value)]
        } else {
            Vec::new()
        }
    }
}

/// Comparator with memory: drives output only when it changes.
/// Tracks `last_trigger_time` to test glitch suppression.
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

impl Device for GlitchTestDevice {
    fn device_name(&self) -> &str { "glitch_test" }
    fn digital_output_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.out_net) }

    fn eval_discrete(
        &mut self, t: f64, _nets: &[LogicValue], av: &[f64],
        q: &mut BinaryHeap<Reverse<DigitalEvent>>,
    ) {
        let v = av.get(self.v_idx).copied().unwrap_or(0.0);
        let out = if v > self.threshold { LogicValue::One } else { LogicValue::Zero };
        if out != self.last_out {
            self.last_out = out;
            self.event_count += 1;
            q.push(Reverse(DigitalEvent { time: t, net: self.out_net, value: out, source: self.id, seq: 0 }));
        }
    }
}

// ─────────────────────────────── Context stub ─────────────────────────────────

fn dummy_context() -> Context { Context { time: 0.0, ..Context::default() } }

// ─────────────────────────────── A → D Tests ─────────────────────────────────

/// Comparator fires when analog voltage crosses threshold.
#[test]
fn test_a2d_comparator_above_threshold() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    // vp=0.8, vn=0.0 → diff=0.8 > 0.5 → One
    cmp.eval_discrete(0.0, &[], &[0.8, 0.0], &mut q);
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
    cmp.eval_discrete(0.0, &[], &[0.3, 0.0], &mut q);
    assert_eq!(q.len(), 1, "initial X→Zero fires one event");
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::Zero);
    // Second call: same voltage — no new event (Zero stays Zero)
    cmp.eval_discrete(1e-9, &[], &[0.3, 0.0], &mut q);
    assert!(q.is_empty(), "no event when voltage stays below threshold");
}

/// Comparator outputs 0 after being above threshold and voltage drops below.
#[test]
fn test_a2d_comparator_hysteresis_less_crossing() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    // Rising: fires One
    cmp.eval_discrete(1e-9, &[], &[0.8, 0.0], &mut q);
    assert_eq!(q.pop().map(|Reverse(e)| e.value), Some(LogicValue::One));
    // Falling: fires Zero
    cmp.eval_discrete(2e-9, &[], &[0.2, 0.0], &mut q);
    assert_eq!(q.pop().map(|Reverse(e)| e.value), Some(LogicValue::Zero));
}

/// Calling comparator twice at same voltage level does not re-fire.
#[test]
fn test_a2d_comparator_no_repeat_fire() {
    let mut cmp = Comparator::new(0, 0, 1, DigitalNet(10), 0.5);
    let mut q = empty_queue();
    cmp.eval_discrete(1e-9, &[], &[0.9, 0.0], &mut q);
    assert_eq!(q.len(), 1); q.clear();
    // Same voltage, second call — no new event
    cmp.eval_discrete(2e-9, &[], &[0.9, 0.0], &mut q);
    assert!(q.is_empty(), "should not fire again at same level");
}

/// Schmitt trigger only fires when voltage exceeds thresh_high (rising).
#[test]
fn test_schmitt_rising_fires_above_high() {
    let mut st = SchmittTrigger::new(0, 0, DigitalNet(5), 0.3, 0.7);
    let mut q = empty_queue();
    // V=0.5: between thresholds → no fire (starting at Zero)
    st.eval_discrete(0.0, &[], &[0.5], &mut q);
    assert!(q.is_empty(), "0.5 < 0.7, no rising edge");
    // V=0.8: above thresh_high → fire One
    st.eval_discrete(1e-9, &[], &[0.8], &mut q);
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::One);
}

/// Schmitt trigger: voltage in hysteresis band after rising does not fire again.
#[test]
fn test_schmitt_hysteresis_suppresses_glitch() {
    let mut st = SchmittTrigger::new(0, 0, DigitalNet(5), 0.3, 0.7);
    let mut q = empty_queue();
    // Assert high
    st.eval_discrete(0.0, &[], &[0.9], &mut q); q.clear();
    // Voltage dips into band (below high, above low) — should NOT fire
    st.eval_discrete(1e-9, &[], &[0.5], &mut q);
    assert!(q.is_empty(), "in hysteresis band — no output change");
}

/// Schmitt trigger fires falling only below thresh_low.
#[test]
fn test_schmitt_falling_fires_below_low() {
    let mut st = SchmittTrigger::new(0, 0, DigitalNet(5), 0.3, 0.7);
    let mut q = empty_queue();
    // Set high first
    st.eval_discrete(0.0, &[], &[0.9], &mut q); q.clear();
    // Voltage at 0.2 < 0.3 → fire Zero
    st.eval_discrete(1e-9, &[], &[0.2], &mut q);
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

    // vp=0.3, vn=0.8 → diff=-0.5 < 0 → Zero (from X, so fires)
    cmp.eval_discrete(0.0, &[], &[0.3, 0.8], &mut q);
    assert_eq!(q.len(), 1);
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::Zero, "negative diff → Zero");

    // vp=1.2, vn=0.4 → diff=0.8 > 0 → One
    cmp.eval_discrete(1e-9, &[], &[1.2, 0.4], &mut q);
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
    cmp.eval_discrete(0.0, &[], &[0.5, 0.0], &mut q);
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
    c1.eval_discrete(5e-9, &[], &av, &mut q);
    c2.eval_discrete(5e-9, &[], &av, &mut q);
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
    let state = dc_state_from_voltages(&[0.0]);
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

    // Force switch closed via eval_discrete
    let nets = [LogicValue::One];
    sw.eval_discrete(0.0, &nets, &[], &mut empty_queue());

    let state = dc_state_from_voltages(&[1.0, 0.0]);
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
    let state = dc_state_from_voltages(&[5.0, 0.0]);

    // Open → no stamps
    assert!(sw.load_dc(&state, &dummy_context()).is_empty());

    // Close
    sw.eval_discrete(0.0, &[LogicValue::One], &[], &mut empty_queue());
    assert_eq!(sw.load_dc(&state, &dummy_context()).len(), 4);

    // Re-open
    sw.eval_discrete(1e-9, &[LogicValue::Zero], &[], &mut empty_queue());
    assert!(sw.load_dc(&state, &dummy_context()).is_empty());
}

/// Gated current source: disabled → no stamps.
#[test]
fn test_d2a_gated_isrc_disabled_no_stamps() {
    let mut src = GatedCurrentSource::new(DigitalNet(0), None, None, 1e-3);
    let state = dc_state_from_voltages(&[]);
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
    src.eval_discrete(0.0, &[LogicValue::One], &[], &mut empty_queue());

    let state = dc_state_from_voltages(&[0.0, 0.0]);
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
    sw.eval_discrete(0.0, &[LogicValue::X], &[], &mut empty_queue());
    let state = dc_state_from_voltages(&[1.0, 0.0]);
    assert!(sw.load_dc(&state, &dummy_context()).is_empty(), "X state → open switch (safe)");
}

/// Z-state digital input to switch: also stays open.
#[test]
fn test_d2a_switch_z_state_stays_open() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(32));
    let b = netlist.connect_node(NodeIdentifier::Anonymous(33));
    let mut sw = AnalogSwitch::new(DigitalNet(0), Some(a), Some(b), 100.0);

    sw.eval_discrete(0.0, &[LogicValue::Z], &[], &mut empty_queue());
    let state = dc_state_from_voltages(&[1.0, 0.0]);
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
    sah.eval_discrete(1e-9, &nets, &av, &mut q);

    // Held value should be 1.23
    assert!((sah.held_value - 1.23).abs() < 1e-12);

    let state = dc_state_from_voltages(&[0.0, 0.0, 1.23]);
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
    // Capture 2.5 V on posedge
    sah.eval_discrete(1e-9, &[LogicValue::One], &[2.5], &mut q);
    assert!((sah.held_value - 2.5).abs() < 1e-12);

    // Clock goes low, analog changes — hold should not update
    sah.eval_discrete(2e-9, &[LogicValue::Zero], &[9.9], &mut q);
    assert!((sah.held_value - 2.5).abs() < 1e-12, "hold unchanged while clk=0");

    // Clock stays high (level, not edge) — no second posedge
    sah.eval_discrete(3e-9, &[LogicValue::One], &[9.9], &mut q);
    // last_clk was Zero (from prev call), so this IS a posedge — now captures 9.9
    assert!((sah.held_value - 9.9).abs() < 1e-12, "posedge again captures new value");
}

/// SAH: multiple posedges each capture the latest analog value.
#[test]
fn test_sah_sequential_samples() {
    let mut sah = SampleAndHold::new(0, DigitalNet(0), 0, None);
    let mut q = empty_queue();
    let voltages = [0.1, 0.5, 0.9, 1.3, 1.7];

    for &v in &voltages {
        sah.eval_discrete(0.0, &[LogicValue::Zero], &[v], &mut q);  // low — no sample
        sah.eval_discrete(0.0, &[LogicValue::One],  &[v], &mut q);  // posedge — sample
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
    cmp.eval_discrete(1e-9, &[], &av, &mut q);

    // Dequeue event, apply to digital state, forward to switch
    let Reverse(ev) = q.pop().unwrap();
    assert_eq!(ev.value, LogicValue::One);

    let nets = [ev.value];
    sw.eval_discrete(1e-9, &nets, &[], &mut empty_queue());

    // Verify switch now contributes analog stamps
    let state = dc_state_from_voltages(&[0.8, 0.0]);
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

    // First: raise voltage → close switch
    let mut q = empty_queue();
    cmp.eval_discrete(1e-9, &[], &[0.9, 0.0], &mut q);
    let Reverse(e1) = q.pop().unwrap();
    sw.eval_discrete(1e-9, &[e1.value], &[], &mut empty_queue());
    let state = dc_state_from_voltages(&[0.9, 0.0]);
    assert_eq!(sw.load_dc(&state, &dummy_context()).len(), 4);

    // Then: lower voltage → open switch
    cmp.eval_discrete(2e-9, &[], &[0.1, 0.0], &mut q);
    let Reverse(e2) = q.pop().unwrap();
    assert_eq!(e2.value, LogicValue::Zero);
    sw.eval_discrete(2e-9, &[e2.value], &[], &mut empty_queue());
    assert!(sw.load_dc(&state, &dummy_context()).is_empty(), "switch re-opened");
}

/// Glitch test: rapid voltage swings around threshold → only real transitions fire events.
#[test]
fn test_glitch_suppression_rapid_transitions() {
    let mut dev = GlitchTestDevice::new(0, 0, DigitalNet(5), 0.5);
    let mut q = empty_queue();

    // Sequence: 0.8 → 0.3 → 0.9 → 0.2 → 0.7 (5 samples, 4 threshold crossings)
    let voltages = [0.8, 0.3, 0.9, 0.2, 0.7];
    for &v in &voltages {
        dev.eval_discrete(0.0, &[], &[v], &mut q);
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

    struct SimpleInverter { input: DigitalNet, output: DigitalNet, id: usize }
    impl Device for SimpleInverter {
        fn device_name(&self) -> &str { "inv" }
        fn digital_input_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.input) }
        fn digital_output_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.output) }
        fn eval_discrete(&mut self, t: f64, nets: &[LogicValue], _av: &[f64], q: &mut BinaryHeap<Reverse<DigitalEvent>>) {
            let out = match nets[self.input.0] {
                LogicValue::One  => LogicValue::Zero,
                LogicValue::Zero => LogicValue::One,
                _                => LogicValue::X,
            };
            q.push(Reverse(DigitalEvent { time: t, net: self.output, value: out, source: self.id, seq: 0 }));
        }
    }

    // Comparator fires via analog: schedule its output event manually
    // (simulating what the simulator does after eval_discrete with av)
    let mut state = DigitalState::new(2);

    // Inject comparator result: net 0 = One at t=1ns
    state.schedule(DigitalEvent { time: 1e-9, net: DigitalNet(0), value: LogicValue::One, source: 99, seq: 0 });

    let mut devices: Vec<Box<dyn Device>> = vec![
        Box::new(SimpleInverter { input: DigitalNet(0), output: DigitalNet(1), id: 0 }),
    ];

    state.evaluate_until_stable(1e-9, &mut devices);

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

    let state = dc_state_from_voltages(&[0.0]);

    // Enable only src0 and src2
    src0.eval_discrete(0.0, &[LogicValue::One,  LogicValue::X, LogicValue::X], &[], &mut empty_queue());
    src1.eval_discrete(0.0, &[LogicValue::X, LogicValue::Zero, LogicValue::X], &[], &mut empty_queue());
    src2.eval_discrete(0.0, &[LogicValue::X,  LogicValue::X, LogicValue::One], &[], &mut empty_queue());

    let total_stamps = src0.load_dc(&state, &dummy_context()).len()
        + src1.load_dc(&state, &dummy_context()).len()
        + src2.load_dc(&state, &dummy_context()).len();

    // src0: 1 stamp (Rhs on node, None for gnd side)
    // src1: 0 stamps (disabled)
    // src2: 1 stamp
    assert_eq!(total_stamps, 2, "only enabled sources contribute stamps");
}
