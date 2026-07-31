//! T23 (DVT-27) — the sequencer-level half of the dynamic digital
//! subscriber: `Wake::WaitEdge` resumes a `Sequencer` on a settled digital
//! transition, built on `HostElement::on_digital_settled` (mirrored from
//! the new `Element` hook, `piperine-solver`'s side proven in
//! `piperine-solver/tests/digital_settle_subscriber.rs`).

use std::sync::{Arc, Mutex};

use piperine_api::sequencer::wait::EdgeDirection;
use piperine_api::sequencer::SequencerControl;
use piperine_api::{HostAdapter, Sequencer, SequencerProgram, Wake};
use piperine_solver::abi::{
    AnalogDevice, DigitalDevice, DigitalNet, DigitalPorts, DigitalState, Element, ElementCapabilities, EvalCtx,
    EventSink, Introspect, LogicValue, Netlist,
};
use piperine_solver::prelude::CircuitInstance;

/// Drives net `a` from `Zero` to `One` at a scripted instant.
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
        sink.emit(self.output, out, 0.0);
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

/// `t.wait(edge(net, Rising))` once, then finishes — recording the exact
/// `(t, settled_value)` it was resumed with.
struct WaitOnce {
    net: DigitalNet,
    direction: EdgeDirection,
    log: Arc<Mutex<Vec<f64>>>,
}
impl SequencerProgram for WaitOnce {
    fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
        Wake::WaitEdge(self.net, self.direction)
    }
    fn resume(&mut self, t: f64, _ctl: &mut SequencerControl) -> Wake {
        self.log.lock().unwrap().push(t);
        Wake::Done
    }
}

/// The direct case: `c` (an AND of `a` and a zero-delay-inverted `a`,
/// i.e. the same glitching net from the solver-level fixture) rising is
/// watched. Its true, SETTLED behavior when `a` flips 0->1 is: `c` glitches
/// to One for one delta cycle, then settles back to Zero — `c` never truly
/// rises in the settled sense at this transition. A waiter on `edge(c,
/// Rising)` must NOT fire here (proving it saw only the settled value,
/// never the transient glitch) — it must still be waiting after the
/// settle.
#[test]
fn a_waiter_on_a_glitching_net_is_not_fooled_by_the_transient_value() {
    let a = DigitalNet(0);
    let b = DigitalNet(1);
    let c = DigitalNet(2);

    let mut circuit = CircuitInstance::from_devices_and_netlist("edge_wait_glitch", vec![], Netlist::new());
    circuit.digital_state = DigitalState::new(3);
    circuit.digital_state.nets[a.0] = LogicValue::Zero;
    circuit.digital_state.nets[b.0] = LogicValue::One;
    circuit.digital_state.nets[c.0] = LogicValue::Zero;

    let log = Arc::new(Mutex::new(Vec::new()));
    circuit.devices.push(Box::new(InputDriver { a, flip_at: 0.0, fired: false }));
    circuit.devices.push(Box::new(Inverter { input: a, output: b }));
    circuit.devices.push(Box::new(AndGate { inputs: [a, b], output: c }));
    circuit.devices.push(Box::new(HostAdapter(Sequencer::new(
        "waiter",
        WaitOnce { net: c, direction: EdgeDirection::Rising, log: log.clone() },
        0.0,
        1.0,
    ))));

    let _ = circuit.accept_and_run_digital(&[], 0.0).expect("settles");
    assert_eq!(circuit.digital_state.nets[c.0], LogicValue::Zero, "c must settle back to Zero (a AND NOT(a))");
    assert!(
        log.lock().unwrap().is_empty(),
        "a waiter on c's rising edge must NOT resume from the transient mid-cycle glitch"
    );
}

/// The positive case: a genuine settled rising edge on `b` (the inverter's
/// output, which really does settle high once `a` falls) resumes the
/// waiter with the correct instant, exactly once.
#[test]
fn a_waiter_resumes_on_a_genuine_settled_rising_edge() {
    let a = DigitalNet(0);
    let b = DigitalNet(1); // b = NOT(a)

    let mut circuit = CircuitInstance::from_devices_and_netlist("edge_wait_genuine", vec![], Netlist::new());
    circuit.digital_state = DigitalState::new(2);
    circuit.digital_state.nets[a.0] = LogicValue::One;
    circuit.digital_state.nets[b.0] = LogicValue::Zero;

    let log = Arc::new(Mutex::new(Vec::new()));
    circuit.devices.push(Box::new(FlipToZero { a, flip_at: 5e-6, fired: false }));
    circuit.devices.push(Box::new(Inverter { input: a, output: b }));
    circuit.devices.push(Box::new(HostAdapter(Sequencer::new(
        "waiter",
        WaitOnce { net: b, direction: EdgeDirection::Rising, log: log.clone() },
        0.0,
        1e-3,
    ))));

    // Settle once at t=0 to establish the waiter's baseline (no edge yet —
    // `a` has not moved).
    let _ = circuit.accept_and_run_digital(&[], 0.0).expect("initial settle establishes the baseline");
    assert!(log.lock().unwrap().is_empty(), "no edge yet at t=0 — a has not moved");

    // `a` genuinely falls at t=5e-6, raising `b`.
    let _ = circuit.accept_and_run_digital(&[], 5e-6).expect("settles the flip");
    assert_eq!(circuit.digital_state.nets[b.0], LogicValue::One, "b settles high once a falls");
    assert_eq!(
        log.lock().unwrap().clone(),
        vec![5e-6],
        "the waiter resumed exactly once, at the settle instant of the genuine rising edge"
    );
}

struct FlipToZero {
    a: DigitalNet,
    flip_at: f64,
    fired: bool,
}
impl AnalogDevice for FlipToZero {}
impl DigitalDevice for FlipToZero {}
impl Introspect for FlipToZero {}
impl Element for FlipToZero {
    fn name(&self) -> &str {
        "flip_to_zero"
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
            sink.emit(self.a, LogicValue::Zero, 0.0);
        }
    }
}
