//! T23 (DVT-27) — the dynamic digital subscriber, the one solver change
//! this batch makes: `Element::on_digital_settled` is dispatched once per
//! accepted step, strictly after `CircuitInstance::accept_and_run_digital`'s
//! scheduler reaches quiescence. Proven here at the solver level (the
//! sequencer-side edge-wait built on top lives in
//! `piperine-api/tests/sequencer_edge_waits.rs`):
//! - a classic AND-gate glitch fixture whose intermediate (mid-delta-cycle)
//!   value genuinely differs from its settled one — the subscriber must see
//!   only the settled value;
//! - the mechanism is not a new `EventKind` variant;
//! - a rejected step never dispatches the subscriber (reached only from the
//!   same call chain `accept_timestep` already proved reject-safe in T19).

use std::sync::{Arc, Mutex};

use piperine_solver::abi::{
    AnalogDevice, DigitalDevice, DigitalNet, DigitalPorts, DigitalState, Element, ElementCapabilities, EvalCtx,
    EventSink, Introspect, LogicValue, Netlist,
};
use piperine_solver::prelude::CircuitInstance;

/// Drives net `a` from `Zero` to `One` at a scripted instant via the
/// analog-accept hook's `EventSink` — the ordinary A2D seeding path,
/// unrelated to the new post-settle dispatch this test exercises.
struct InputDriver {
    a: DigitalNet,
    flip_at: f64,
    fired: bool,
}
impl AnalogDevice for InputDriver {}
impl DigitalDevice for InputDriver {}
impl Introspect for InputDriver {}
impl Element for InputDriver {
    fn name(&self) -> &str {
        "input_driver"
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
            sink.emit(self.a, LogicValue::One, 0.0);
        }
    }
}

struct Inverter {
    input: DigitalNet,
    output: DigitalNet,
    delay: f64,
}
impl AnalogDevice for Inverter {}
impl DigitalDevice for Inverter {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.input), outputs: std::slice::from_ref(&self.output) }
    }
    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out = match ctx.nets[self.input.0] {
            LogicValue::Zero => LogicValue::One,
            LogicValue::One => LogicValue::Zero,
            _ => LogicValue::X,
        };
        sink.emit(self.output, out, self.delay);
    }
}
impl Introspect for Inverter {}
impl Element for Inverter {
    fn name(&self) -> &str {
        "inverter"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

struct AndGate {
    inputs: [DigitalNet; 2],
    output: DigitalNet,
}
impl AnalogDevice for AndGate {}
impl DigitalDevice for AndGate {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.output) }
    }
    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out =
            if self.inputs.iter().all(|n| ctx.nets[n.0] == LogicValue::One) { LogicValue::One } else { LogicValue::Zero };
        sink.emit(self.output, out, 0.0);
    }
}
impl Introspect for AndGate {}
impl Element for AndGate {
    fn name(&self) -> &str {
        "and_gate"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

struct OrGate {
    inputs: [DigitalNet; 2],
    output: DigitalNet,
}
impl AnalogDevice for OrGate {}
impl DigitalDevice for OrGate {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.output) }
    }
    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out =
            if self.inputs.iter().any(|n| ctx.nets[n.0] == LogicValue::One) { LogicValue::One } else { LogicValue::Zero };
        sink.emit(self.output, out, 0.0);
    }
}
impl Introspect for OrGate {}
impl Element for OrGate {
    fn name(&self) -> &str {
        "or_gate"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

/// Records every value `probed` takes on during **each `comb_phase`
/// evaluation** (mid-delta-cycle, before quiescence) — the ground truth for
/// "did a glitch actually occur," independent of the new subscriber seam.
struct Probe {
    probed: DigitalNet,
    other_input: DigitalNet, // present only so this device has SOME input to re-evaluate on
    mid_cycle_values: Arc<Mutex<Vec<LogicValue>>>,
}
impl AnalogDevice for Probe {}
impl DigitalDevice for Probe {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.probed), outputs: &[] }
    }
    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {
        let _ = self.other_input;
        self.mid_cycle_values.lock().unwrap().push(ctx.nets[self.probed.0]);
    }
}
impl Introspect for Probe {}
impl Element for Probe {
    fn name(&self) -> &str {
        "probe"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

/// The new hook under test: records the settled value it was handed, and
/// the count of times it fired (T23's "resumed only after settling" +
/// "a rejected step never dispatches" proofs).
struct Subscriber {
    watch: DigitalNet,
    calls: Arc<Mutex<Vec<(f64, LogicValue)>>>,
}
impl AnalogDevice for Subscriber {}
impl DigitalDevice for Subscriber {}
impl Introspect for Subscriber {}
impl Element for Subscriber {
    fn name(&self) -> &str {
        "subscriber"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
    fn on_digital_settled(&mut self, nets: &[LogicValue], t: f64) {
        self.calls.lock().unwrap().push((t, nets[self.watch.0]));
    }
}

/// Builds the classic AND-gate glitch: `c = a AND NOT(a)` (zero-delay
/// inverter, zero-delay AND) — driving `a` from 0 to 1 makes `c` read
/// `One` for one intermediate delta cycle (the AND still sees the stale
/// `b = NOT(old a) = 1` before the inverter's own zero-delay update
/// lands) before settling to its correct final value, `Zero`.
fn build_glitch_circuit(
    flip_at: f64,
    mid_cycle: Arc<Mutex<Vec<LogicValue>>>,
    settled_calls: Arc<Mutex<Vec<(f64, LogicValue)>>>,
) -> CircuitInstance {
    let a = DigitalNet(0);
    let b = DigitalNet(1); // b = NOT(a)
    let c = DigitalNet(2); // c = a AND b  (the glitching net)

    let mut circuit = CircuitInstance::from_devices_and_netlist("glitch", vec![], Netlist::new());
    circuit.digital_state = DigitalState::new(3);
    circuit.digital_state.nets[a.0] = LogicValue::Zero;
    circuit.digital_state.nets[b.0] = LogicValue::One; // NOT(Zero)
    circuit.digital_state.nets[c.0] = LogicValue::Zero; // a AND b = Zero AND One
    circuit.devices.push(Box::new(InputDriver { a, flip_at, fired: false }));
    circuit.devices.push(Box::new(Inverter { input: a, output: b, delay: 0.0 }));
    circuit.devices.push(Box::new(AndGate { inputs: [a, b], output: c }));
    circuit.devices.push(Box::new(Probe { probed: c, other_input: a, mid_cycle_values: mid_cycle }));
    circuit.devices.push(Box::new(Subscriber { watch: c, calls: settled_calls }));
    circuit
}

/// A two-stage double-inverter feeding an OR gate: `c = a OR NOT(NOT(a))`.
/// At rest this is consistent (`b2` always catches up to `NOT(NOT(a))`), so
/// `c` tracks `a` — but only after the two inverter stages settle, meaning
/// the **pre-accept snapshot of `c` differs from its post-settle value**
/// whenever `a` flips: before the flip, `c = a_old`; after the flip and full
/// settle, `c = a_new`. This is the fixture that catches a dispatch bug
/// where `on_digital_settled` is fed the pre-accept snapshot instead of the
/// post-settle nets — the AND-gate glitch fixture above cannot catch that
/// bug because its `c` is tautologically `Zero` both before and after every
/// settle (`a AND NOT(a)` is always `Zero`), so a pre- vs. post-settle mixup
/// is invisible to it.
fn build_before_after_differs_circuit(
    flip_at: f64,
    settled_calls: Arc<Mutex<Vec<(f64, LogicValue)>>>,
) -> CircuitInstance {
    let a = DigitalNet(0);
    let b1 = DigitalNet(1); // b1 = NOT(a)
    let b2 = DigitalNet(2); // b2 = NOT(b1) = NOT(NOT(a)), one delta cycle behind a
    let c = DigitalNet(3); // c = a OR b2 — settles to a's new value, differing from its own pre-flip value

    let mut circuit = CircuitInstance::from_devices_and_netlist("before_after", vec![], Netlist::new());
    circuit.digital_state = DigitalState::new(4);
    circuit.digital_state.nets[a.0] = LogicValue::Zero;
    circuit.digital_state.nets[b1.0] = LogicValue::One; // NOT(Zero)
    circuit.digital_state.nets[b2.0] = LogicValue::Zero; // NOT(One)
    circuit.digital_state.nets[c.0] = LogicValue::Zero; // Zero OR Zero
    circuit.devices.push(Box::new(InputDriver { a, flip_at, fired: false }));
    circuit.devices.push(Box::new(Inverter { input: a, output: b1, delay: 0.0 }));
    circuit.devices.push(Box::new(Inverter { input: b1, output: b2, delay: 0.0 }));
    circuit.devices.push(Box::new(OrGate { inputs: [a, b2], output: c }));
    circuit.devices.push(Box::new(Subscriber { watch: c, calls: settled_calls }));
    circuit
}

#[test]
fn the_subscriber_sees_the_post_settle_value_even_when_it_differs_from_the_pre_accept_snapshot() {
    let settled_calls = Arc::new(Mutex::new(Vec::new()));
    let mut circuit = build_before_after_differs_circuit(0.0, settled_calls.clone());

    // Read the pre-accept snapshot ourselves, exactly like the dispatch
    // site's own `before` local — proves the fixture's premise that this
    // snapshot genuinely differs from the settled value asserted below.
    let before_c = circuit.digital_state.nets[3];
    assert_eq!(before_c, LogicValue::Zero, "c starts at Zero (a=Zero OR b2=Zero) before the flip");

    let _ = circuit.accept_and_run_digital(&[], 0.0).expect("settles");

    let settled = settled_calls.lock().unwrap().clone();
    assert_eq!(settled.len(), 1, "on_digital_settled must fire exactly once for this one settle: {settled:?}");
    assert_eq!(
        settled[0],
        (0.0, LogicValue::One),
        "the subscriber must see c's SETTLED value (One, once a=One propagates through both inverters), \
         not the pre-accept snapshot (Zero) — a dispatch bug that fed the pre-accept snapshot instead of \
         the post-settle nets would report Zero here instead"
    );
}

#[test]
fn the_subscriber_sees_only_the_settled_value_never_the_mid_delta_cycle_glitch() {
    let mid_cycle = Arc::new(Mutex::new(Vec::new()));
    let settled_calls = Arc::new(Mutex::new(Vec::new()));
    let mut circuit = build_glitch_circuit(0.0, mid_cycle.clone(), settled_calls.clone());

    // Settle at t=0: the InputDriver's accept hook flips `a` to One,
    // cascading through the inverter/AND gate.
    let _ = circuit.accept_and_run_digital(&[], 0.0).expect("settles");

    let mid = mid_cycle.lock().unwrap().clone();
    assert!(
        mid.contains(&LogicValue::One),
        "the fixture must actually glitch (c reads One at some intermediate delta cycle): {mid:?}"
    );

    let settled = settled_calls.lock().unwrap().clone();
    assert_eq!(settled.len(), 1, "on_digital_settled must fire exactly once for this one settle: {settled:?}");
    assert_eq!(
        settled[0],
        (0.0, LogicValue::Zero),
        "the subscriber must see c's correct SETTLED value (Zero), never the transient glitch (One)"
    );
}

/// The mechanism is a subscriber dispatch on the existing `Element` ABI,
/// not a new `EventKind` variant — mechanical proof, not by inspection:
/// `EventKind`'s own declaration still names exactly its four original
/// variants.
#[test]
fn the_mechanism_adds_no_new_eventkind_variant() {
    let src = include_str!("../src/analyses/events.rs");
    let start = src.find("pub enum EventKind").expect("EventKind must still exist");
    let body_start = src[start..].find('{').unwrap() + start;
    let body_end = src[body_start..].find("\n}").unwrap() + body_start;
    let body = &src[body_start..body_end];
    let variants: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .filter(|l| l.ends_with(','))
        .collect();
    assert_eq!(
        variants,
        vec!["Digital,", "Breakpoint,", "StepHint,", "Crossing,"],
        "T23 must not add a new EventKind variant for the digital subscriber seam: {variants:?}"
    );
}

/// D11 holds for the subscriber too: `on_digital_settled` is reached only
/// from `accept_and_run_digital`, itself reachable only when a step is
/// already accepted — calling it directly here (as a solver-internal
/// integration proof) at a time nothing ever settles at is simply never
/// invoked by a rejected transient step in the real driver, the same
/// property `checkpoint_restore.rs`'s reject fixture already proves for
/// `accept_timestep`'s call site (identical call chain).
#[test]
fn calling_accept_and_run_digital_is_the_only_way_to_reach_the_new_hook() {
    // Structural proof: the call site added in this task lives inside
    // `accept_and_run_digital` only — grep the solver's own source for any
    // other call site of `on_digital_settled`.
    let src = include_str!("../src/core/circuit.rs");
    // Search for the functional call pattern specifically (`.on_digital_
    // settled(`), not any doc-comment mention of the method's name.
    let occurrences = src.matches(".on_digital_settled(").count();
    assert_eq!(occurrences, 1, "on_digital_settled must be dispatched from exactly one call site in circuit.rs");
}
