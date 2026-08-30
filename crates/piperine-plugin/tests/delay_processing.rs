//! T61 (LRMC-C10-12, surface half): `PluginHost::get_delays`/`put_delays`
//! — `vpi_get_delays()`/`vpi_put_delays()`'s native equivalents over T60's
//! `DelayTable` model.
//!
//! LRM-COVERS: 11.3.2, 12.11, 12.29

use std::sync::{Arc, Mutex};

use piperine_plugin::{
    AnalogDevice, CircuitInstance, DelayTable, DigitalDevice, DigitalEvent, DigitalNet,
    DigitalPorts, Element, ElementCapabilities, EvalCtx, EventSink, Introspect, LogicValue,
    Netlist, PathDelay, PluginHost,
};

/// A minimal inverter that consults a *shared* `DelayTable` on every
/// evaluation instead of a fixed `delay` field — the demonstration device
/// this test needs to prove a `put_delays` write actually changes
/// subsequently scheduled event timing (no shipped gate/model in this
/// tree consults `DelayTable` yet; wiring `piperine-codegen`'s
/// `GateElement` to do so is future work outside this task's `Where`,
/// `piperine-plugin/src/{host.rs,vpi.rs}` only).
struct TimedInverter {
    input: DigitalNet,
    output: DigitalNet,
    delays: Arc<Mutex<DelayTable>>,
}

impl AnalogDevice for TimedInverter {}
impl Introspect for TimedInverter {}

impl DigitalDevice for TimedInverter {
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.input), outputs: std::slice::from_ref(&self.output) }
    }

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out = match ctx.nets[self.input.0] {
            LogicValue::Zero => LogicValue::One,
            LogicValue::One => LogicValue::Zero,
            _ => LogicValue::X,
        };
        let delay = self.delays.lock().unwrap().get(self.output).rise;
        sink.emit(self.output, out, delay);
    }
}

impl Element for TimedInverter {
    fn name(&self) -> &str {
        "timed_inverter"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::DIGITAL
    }
}

/// `get_delays` reads what `put_delays` wrote — the plain data round-trip
/// half.
#[test]
fn put_delays_then_get_delays_round_trips() {
    let host = PluginHost::empty();
    let mut table = DelayTable::new();
    let net = DigitalNet(0);

    assert_eq!(host.get_delays(&table, net), PathDelay::default());

    let delay = PathDelay { rise: 1.0e-9, fall: 2.0e-9, turn_off: 3.0e-9 };
    host.put_delays(&mut table, net, delay);
    assert_eq!(host.get_delays(&table, net), delay);
}

/// The live-run half: a `put_delays` write changes the timing of a
/// subsequently scheduled event, observed through a real
/// `CircuitInstance::run_digital_at` run (not a hand-rolled scheduler
/// loop) — `PluginHost::put_delays`/`get_delays` are the only entry
/// points this test uses to touch the delay table.
#[test]
fn put_delays_changes_observed_event_timing_in_a_live_run() {
    let host = PluginHost::empty();
    let input = DigitalNet(0);
    let output = DigitalNet(1);

    let delays = Arc::new(Mutex::new(DelayTable::new()));
    host.put_delays(&mut delays.lock().unwrap(), output, PathDelay::uniform(2.0e-6));

    let gate: Box<dyn Element> =
        Box::new(TimedInverter { input, output, delays: delays.clone() });
    let mut circuit = CircuitInstance::from_devices_and_netlist("t", vec![gate], Netlist::new());
    let mut state = piperine_solver::abi::DigitalState::new(2);

    let t_input = 1.0e-6;
    state.schedule(DigitalEvent {
        time: t_input,
        net: input,
        value: LogicValue::One,
        source: 0,
        seq: 0,
        region: piperine_solver::abi::Region::Active, strength: None });
    circuit.digital_state = state;

    circuit.run_digital_at(t_input).expect("settles at t_input");
    assert_eq!(
        circuit.digital_state.nets[output.0],
        LogicValue::X,
        "output must not flip before the delay set by put_delays elapses"
    );

    circuit.run_digital_at(t_input + 2.0e-6).expect("settles at t_input + delay");
    assert_eq!(
        circuit.digital_state.nets[output.0],
        LogicValue::Zero,
        "output flips exactly at t_input + the delay written by put_delays"
    );

    // Now change the delay for a *second* run and prove the new value
    // takes effect (not sticky to the first observed timing).
    let output2 = DigitalNet(3);
    host.put_delays(&mut delays.lock().unwrap(), output2, PathDelay::uniform(5.0e-6));
    let gate2: Box<dyn Element> =
        Box::new(TimedInverter { input: DigitalNet(2), output: output2, delays });
    let mut circuit2 = CircuitInstance::from_devices_and_netlist("t2", vec![gate2], Netlist::new());
    let mut state2 = piperine_solver::abi::DigitalState::new(4);
    state2.schedule(DigitalEvent {
        time: t_input,
        net: DigitalNet(2),
        value: LogicValue::One,
        source: 0,
        seq: 0,
        region: piperine_solver::abi::Region::Active, strength: None });
    circuit2.digital_state = state2;
    circuit2.run_digital_at(t_input).expect("settles");
    circuit2.run_digital_at(t_input + 2.0e-6).expect("settles"); // old delay: nothing yet
    assert_eq!(circuit2.digital_state.nets[output2.0], LogicValue::X, "the new 5us delay must apply, not the old 2us one");
    circuit2.run_digital_at(t_input + 5.0e-6).expect("settles at the new delay");
    assert_eq!(circuit2.digital_state.nets[output2.0], LogicValue::Zero);
}
