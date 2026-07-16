// Comprehensive digital topology tests.
//
// Covers: DAG ordering, back-edge detection, zero-delay propagation,
// fan-out, diamond, RS latch, DFF pipeline, ring oscillator, disconnected
// subgraphs, D2A device, A2D state, and cosim integration.

use std::collections::BinaryHeap;

use piperine_solver::prelude::CircuitInstance;
fn make_instance(title: &str) -> CircuitInstance {
    CircuitInstance::from_devices_and_netlist(title, vec![], piperine_solver::abi::Netlist::new())
}
use piperine_solver::abi::{Element, ElementCapabilities};
use piperine_solver::abi::{LogicValue, DigitalNet, DigitalEvent};
use piperine_solver::abi::{DigitalPorts, EvalCtx, EventSink, QueueSink};
use piperine_solver::abi::{DigitalState, DigitalTopology};
use piperine_solver::prelude::TransientAnalysisOptions;
use piperine_solver::prelude::Context;

#[path = "helpers/mod.rs"]
mod helpers;
use helpers::{A2DState, D2ADevice};

// ===================================================================
// Pure Rust device implementations
// ===================================================================

struct Inverter { input: DigitalNet, output: DigitalNet, delay: f64, _id: usize }

impl Element for Inverter {
    fn name(&self) -> &str { "inverter" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: std::slice::from_ref(&self.input), outputs: std::slice::from_ref(&self.output) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out = match ctx.nets[self.input.0] {
            LogicValue::Zero => LogicValue::One,
            LogicValue::One  => LogicValue::Zero,
            _                => LogicValue::X,
        };
        sink.emit(self.output, out, self.delay);
    }

}



struct NorGate { inputs: [DigitalNet; 2], output: DigitalNet, delay: f64, _id: usize }

impl Element for NorGate {
    fn name(&self) -> &str { "nor_gate" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.output) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out = if self.inputs.iter().any(|n| ctx.nets[n.0] == LogicValue::One) { LogicValue::Zero } else { LogicValue::One };
        sink.emit(self.output, out, self.delay);
    }

}



struct AndGate { inputs: [DigitalNet; 2], output: DigitalNet, delay: f64, _id: usize }

impl Element for AndGate {
    fn name(&self) -> &str { "and_gate" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.output) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let out = if self.inputs.iter().all(|n| ctx.nets[n.0] == LogicValue::One) { LogicValue::One } else { LogicValue::Zero };
        sink.emit(self.output, out, self.delay);
    }

}



struct DFF {
    inputs: [DigitalNet; 2], // [clk, d]
    q: DigitalNet,
    last_clk: LogicValue,
    clk_to_q: f64,
    _id: usize,
}

impl DFF {
    fn new(_id: usize, clk: DigitalNet, d: DigitalNet, q: DigitalNet, clk_to_q: f64) -> Self {
        Self { inputs: [clk, d], q, last_clk: LogicValue::Zero, clk_to_q, _id: _id }
    }
}

impl Element for DFF {
    fn name(&self) -> &str { "dff" }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::DIGITAL }
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &self.inputs, outputs: std::slice::from_ref(&self.q) }
    }

    fn init(&mut self, _sink: &mut dyn EventSink) {}

    fn comb_phase(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        let clk = ctx.nets[self.inputs[0].0];
        let d   = ctx.nets[self.inputs[1].0];
        if self.last_clk == LogicValue::Zero && clk == LogicValue::One {
            sink.emit(self.q, d, self.clk_to_q);
        }
        self.last_clk = clk;
    }

}



// ===================================================================
// TOPOLOGY STRUCTURE TESTS
// ===================================================================

#[test]
fn test_topology_empty() {
    let devices: Vec<Box<dyn Element>> = vec![];
    let topo = DigitalTopology::build(&devices);
    assert!(topo.topo_order.is_empty());
    assert!(topo.back_edges.is_empty());
}

#[test]
fn test_topology_single_device() {
    let devices: Vec<Box<dyn Element>> = vec![
        Box::new(Inverter { input: DigitalNet(0), output: DigitalNet(1), delay: 1e-9, _id: 0 }),
    ];
    let topo = DigitalTopology::build(&devices);
    assert_eq!(topo.topo_order, vec![0]);
    assert!(topo.back_edges.is_empty());
}

#[test]
fn test_topology_linear_chain() {
    // INV0→INV1→INV2→INV3
    let devices: Vec<Box<dyn Element>> = (0..4).map(|i| -> Box<dyn Element> {
        Box::new(Inverter { input: DigitalNet(i), output: DigitalNet(i + 1), delay: 1e-9, _id: i })
    }).collect();
    let topo = DigitalTopology::build(&devices);
    assert_eq!(topo.topo_order.len(), 4);
    assert!(topo.back_edges.is_empty());
    for i in 0..3 {
        let pi = topo.topo_order.iter().position(|&d| d == i).unwrap();
        let pj = topo.topo_order.iter().position(|&d| d == i + 1).unwrap();
        assert!(pi < pj, "INV{} must precede INV{}", i, i + 1);
    }
}

#[test]
fn test_topology_diamond() {
    // n0→INV0→n1→{INV1→n2, INV2→n3}; AND(n2,n3)→n4
    let devices: Vec<Box<dyn Element>> = vec![
        Box::new(Inverter { input: DigitalNet(0), output: DigitalNet(1), delay: 0.0, _id: 0 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(2), delay: 0.0, _id: 1 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(3), delay: 0.0, _id: 2 }),
        Box::new(AndGate  { inputs: [DigitalNet(2), DigitalNet(3)], output: DigitalNet(4), delay: 0.0, _id: 3 }),
    ];
    let topo = DigitalTopology::build(&devices);
    assert_eq!(topo.topo_order.len(), 4);
    assert!(topo.back_edges.is_empty());

    let p0 = topo.topo_order.iter().position(|&d| d == 0).unwrap();
    let p1 = topo.topo_order.iter().position(|&d| d == 1).unwrap();
    let p2 = topo.topo_order.iter().position(|&d| d == 2).unwrap();
    let p3 = topo.topo_order.iter().position(|&d| d == 3).unwrap();
    assert!(p0 < p1, "INV0 before INV1");
    assert!(p0 < p2, "INV0 before INV2");
    assert!(p1 < p3, "INV1 before AND");
    assert!(p2 < p3, "INV2 before AND");
}

#[test]
fn test_topology_ring_has_back_edge() {
    // 3-inverter ring: INV0→INV1→INV2→INV0
    let devices: Vec<Box<dyn Element>> = (0..3usize).map(|i| -> Box<dyn Element> {
        Box::new(Inverter { input: DigitalNet(i), output: DigitalNet((i + 1) % 3), delay: 1e-9, _id: i })
    }).collect();
    let topo = DigitalTopology::build(&devices);
    assert_eq!(topo.topo_order.len(), 3);
    assert!(!topo.back_edges.is_empty(), "Ring must have at least one back edge");
}

#[test]
fn test_topology_disconnected_subgraphs() {
    // Chain A: INV0→INV1; Chain B: INV2→INV3 (no shared nets)
    let devices: Vec<Box<dyn Element>> = vec![
        Box::new(Inverter { input: DigitalNet(0), output: DigitalNet(1), delay: 0.0, _id: 0 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(2), delay: 0.0, _id: 1 }),
        Box::new(Inverter { input: DigitalNet(3), output: DigitalNet(4), delay: 0.0, _id: 2 }),
        Box::new(Inverter { input: DigitalNet(4), output: DigitalNet(5), delay: 0.0, _id: 3 }),
    ];
    let topo = DigitalTopology::build(&devices);
    assert_eq!(topo.topo_order.len(), 4);
    assert!(topo.back_edges.is_empty());

    let p0 = topo.topo_order.iter().position(|&d| d == 0).unwrap();
    let p1 = topo.topo_order.iter().position(|&d| d == 1).unwrap();
    let p2 = topo.topo_order.iter().position(|&d| d == 2).unwrap();
    let p3 = topo.topo_order.iter().position(|&d| d == 3).unwrap();
    assert!(p0 < p1, "INV0 before INV1 in chain A");
    assert!(p2 < p3, "INV2 before INV3 in chain B");
}

// ===================================================================
// EVALUATION TESTS (standalone DigitalState + evaluate_dag_ordered)
// ===================================================================

#[test]
fn test_zero_delay_chain_propagates_in_one_pass() {
    // net0→INV0(delay=0)→net1→INV1(delay=0)→net2; all settle in one pass
    let mut state = DigitalState::new(3);
    state.nets[0] = LogicValue::One;
    state.schedule(DigitalEvent { time: 1e-9, net: DigitalNet(0), value: LogicValue::Zero, source: 99, seq: 0 });

    let mut devices: Vec<Box<dyn Element>> = vec![
        Box::new(Inverter { input: DigitalNet(0), output: DigitalNet(1), delay: 0.0, _id: 0 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(2), delay: 0.0, _id: 1 }),
    ];
    let topo = DigitalTopology::build(&devices);
    state.evaluate_dag_ordered(1e-9, &mut devices, &topo, Default::default(), &[]).unwrap();

    assert_eq!(state.nets[0], LogicValue::Zero);
    assert_eq!(state.nets[1], LogicValue::One);   // INV0: NOT(0)=1
    assert_eq!(state.nets[2], LogicValue::Zero);  // INV1: NOT(1)=0
    assert_eq!(state.peek_next_event_time(), f64::INFINITY, "No future events after zero-delay settle");
}

#[test]
fn test_fan_out_topology() {
    // net0→INV0→net1→{INV1→net2, INV2→net3}
    let mut state = DigitalState::new(4);
    state.nets[0] = LogicValue::Zero;
    state.schedule(DigitalEvent { time: 1e-9, net: DigitalNet(0), value: LogicValue::One, source: 99, seq: 0 });

    let mut devices: Vec<Box<dyn Element>> = vec![
        Box::new(Inverter { input: DigitalNet(0), output: DigitalNet(1), delay: 0.0, _id: 0 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(2), delay: 0.0, _id: 1 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(3), delay: 0.0, _id: 2 }),
    ];
    let topo = DigitalTopology::build(&devices);
    state.evaluate_dag_ordered(1e-9, &mut devices, &topo, Default::default(), &[]).unwrap();

    assert_eq!(state.nets[0], LogicValue::One);
    assert_eq!(state.nets[1], LogicValue::Zero); // INV0: NOT(1)=0
    assert_eq!(state.nets[2], LogicValue::One);  // INV1: NOT(0)=1
    assert_eq!(state.nets[3], LogicValue::One);  // INV2: NOT(0)=1
}

#[test]
fn test_diamond_propagation() {
    // n0→INV0→n1→{INV1→n2, INV2→n3}; AND(n2,n3)→n4
    // n0: 1→0, expect INV0→n1=1, INV1→n2=0, INV2→n3=0, AND→n4=0
    let mut state = DigitalState::new(5);
    state.nets[0] = LogicValue::One;
    state.schedule(DigitalEvent { time: 1e-9, net: DigitalNet(0), value: LogicValue::Zero, source: 99, seq: 0 });

    let mut devices: Vec<Box<dyn Element>> = vec![
        Box::new(Inverter { input: DigitalNet(0), output: DigitalNet(1), delay: 0.0, _id: 0 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(2), delay: 0.0, _id: 1 }),
        Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(3), delay: 0.0, _id: 2 }),
        Box::new(AndGate  { inputs: [DigitalNet(2), DigitalNet(3)], output: DigitalNet(4), delay: 0.0, _id: 3 }),
    ];
    let topo = DigitalTopology::build(&devices);
    state.evaluate_dag_ordered(1e-9, &mut devices, &topo, Default::default(), &[]).unwrap();

    assert_eq!(state.nets[0], LogicValue::Zero);
    assert_eq!(state.nets[1], LogicValue::One);
    assert_eq!(state.nets[2], LogicValue::Zero);
    assert_eq!(state.nets[3], LogicValue::Zero);
    assert_eq!(state.nets[4], LogicValue::Zero);
}

// ===================================================================
// RS NOR LATCH (back-edge cycle that converges)
// ===================================================================

// NOR1: Q  = NOR(R, QB)
// NOR2: QB = NOR(S, Q)
// nets: 0=R, 1=S, 2=Q, 3=QB

fn make_rs_latch_instance(title: &str, r_val: LogicValue, s_val: LogicValue, q_val: LogicValue, qb_val: LogicValue) -> CircuitInstance {
    let mut state = DigitalState::new(4);
    state.nets[0] = r_val;
    state.nets[1] = s_val;
    state.nets[2] = q_val;
    state.nets[3] = qb_val;

    let mut instance = make_instance(title);
    instance.digital_state = state;
    instance.devices.push(Box::new(NorGate { inputs: [DigitalNet(0), DigitalNet(3)], output: DigitalNet(2), delay: 1e-10, _id: 0 }));
    instance.devices.push(Box::new(NorGate { inputs: [DigitalNet(1), DigitalNet(2)], output: DigitalNet(3), delay: 1e-10, _id: 1 }));
    instance.rebuild_digital_topology();
    instance
}

#[test]
fn test_rs_nor_latch_set() {
    let mut instance = make_rs_latch_instance("RSSet", LogicValue::Zero, LogicValue::Zero, LogicValue::X, LogicValue::X);
    // Apply S=1 at t=1ns
    instance.digital_state.schedule(DigitalEvent { time: 1e-9, net: DigitalNet(1), value: LogicValue::One, source: 99, seq: 0 });

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let _ = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    assert_eq!(instance.digital_state.nets[2], LogicValue::One,  "Q should be 1 after Set");
    assert_eq!(instance.digital_state.nets[3], LogicValue::Zero, "QB should be 0 after Set");
}

#[test]
fn test_rs_nor_latch_reset() {
    // Pre-set latch: Q=1, QB=0; then R=1
    let mut instance = make_rs_latch_instance("RSReset", LogicValue::Zero, LogicValue::Zero, LogicValue::One, LogicValue::Zero);
    instance.digital_state.schedule(DigitalEvent { time: 1e-9, net: DigitalNet(0), value: LogicValue::One, source: 99, seq: 0 });

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let _ = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    assert_eq!(instance.digital_state.nets[2], LogicValue::Zero, "Q should be 0 after Reset");
    assert_eq!(instance.digital_state.nets[3], LogicValue::One,  "QB should be 1 after Reset");
}

#[test]
fn test_rs_nor_latch_holds_state() {
    // Q=1, QB=0, no input events → latch holds
    let mut instance = make_rs_latch_instance("RSHold", LogicValue::Zero, LogicValue::Zero, LogicValue::One, LogicValue::Zero);
    // No events scheduled

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let _ = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    assert_eq!(instance.digital_state.nets[2], LogicValue::One,  "Q should hold at 1");
    assert_eq!(instance.digital_state.nets[3], LogicValue::Zero, "QB should hold at 0");
}

// ===================================================================
// DFF TESTS
// ===================================================================

#[test]
fn test_dff_rising_edge_capture() {
    let mut state = DigitalState::new(3);
    state.nets[0] = LogicValue::Zero; // CLK=0
    state.nets[1] = LogicValue::One;  // D=1
    state.schedule(DigitalEvent { time: 2e-9, net: DigitalNet(0), value: LogicValue::One, source: 99, seq: 0 });

    let mut instance = make_instance("DFFCapture");
    instance.digital_state = state;
    instance.devices.push(Box::new(DFF::new(0, DigitalNet(0), DigitalNet(1), DigitalNet(2), 0.5e-9)));
    instance.rebuild_digital_topology();

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let _ = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    assert_eq!(instance.digital_state.nets[2], LogicValue::One, "Q should capture D=1 on rising CLK");
}

#[test]
fn test_dff_does_not_capture_on_falling_edge() {
    let mut state = DigitalState::new(3);
    state.nets[0] = LogicValue::One;  // CLK=1
    state.nets[1] = LogicValue::One;  // D=1
    state.nets[2] = LogicValue::Zero; // Q=0 initially
    state.schedule(DigitalEvent { time: 2e-9, net: DigitalNet(0), value: LogicValue::Zero, source: 99, seq: 0 });

    let mut instance = make_instance("DFFNoCapture");
    instance.digital_state = state;
    instance.devices.push(Box::new(DFF::new(0, DigitalNet(0), DigitalNet(1), DigitalNet(2), 0.5e-9)));
    instance.rebuild_digital_topology();

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let _ = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    assert_eq!(instance.digital_state.nets[2], LogicValue::Zero, "Q must not change on falling CLK edge");
}

#[test]
fn test_dff_pipeline_three_stages() {
    // nets: 0=CLK, 1=D_in, 2=Q1, 3=Q2, 4=Q3
    // Three rising edges push D_in=1 through all stages.
    let mut state = DigitalState::new(5);
    state.nets[0] = LogicValue::Zero;
    state.nets[1] = LogicValue::One;
    // Clock pulses: rise at 2,4,6ns; fall at 2.5,4.5,6.5ns
    for (i, &t) in [2e-9f64, 2.5e-9, 4e-9, 4.5e-9, 6e-9, 6.5e-9].iter().enumerate() {
        let val = if i % 2 == 0 { LogicValue::One } else { LogicValue::Zero };
        state.schedule(DigitalEvent { time: t, net: DigitalNet(0), value: val, source: 99, seq: i as u64 });
    }

    let mut instance = make_instance("DFFPipeline");
    instance.digital_state = state;
    instance.devices.push(Box::new(DFF::new(0, DigitalNet(0), DigitalNet(1), DigitalNet(2), 0.1e-9)));
    instance.devices.push(Box::new(DFF::new(1, DigitalNet(0), DigitalNet(2), DigitalNet(3), 0.1e-9)));
    instance.devices.push(Box::new(DFF::new(2, DigitalNet(0), DigitalNet(3), DigitalNet(4), 0.1e-9)));
    instance.rebuild_digital_topology();

    let options = TransientAnalysisOptions::new(20e-9.into(), 0.5e-9.into());
    let _ = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    assert_eq!(instance.digital_state.nets[4], LogicValue::One, "D=1 should propagate through all 3 DFF stages");
}

// ===================================================================
// RING OSCILLATOR — back-edge driven, verify period
// ===================================================================

#[test]
fn test_ring_oscillator_five_inv() {
    // 5 inverters in ring, 1ns delay each → period = 2*5*1ns = 10ns
    let mut state = DigitalState::new(5);
    state.schedule(DigitalEvent { time: 0.0, net: DigitalNet(0), value: LogicValue::Zero, source: 999, seq: 0 });

    let mut instance = make_instance("RingOsc5");
    for i in 0..5usize {
        instance.devices.push(Box::new(Inverter {
            input: DigitalNet(i), output: DigitalNet((i + 1) % 5), delay: 1e-9, _id: i,
        }));
    }
    instance.digital_state = state;
    instance.rebuild_digital_topology();

    // Ring has back edges
    let topo = instance.digital_topology.as_ref().unwrap();
    assert!(!topo.back_edges.is_empty(), "5-inverter ring must have back edges");

    let options = TransientAnalysisOptions::new(100e-9.into(), 0.5e-9.into());
    let mut solver = instance.transient(options, Context::default()).unwrap();
    let result = solver.solve().unwrap();

    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    assert!(times.iter().any(|&t| (t * 1e9 - 1.0).abs() < 0.01), "Should hit t=1ns");
    assert!(times.iter().any(|&t| (t * 1e9 - 10.0).abs() < 0.01), "Should hit t=10ns (full period)");
    // 100ns / 10ns period → 10 full cycles → at least ~20 events (each transition)
    let transition_count = times.iter().filter(|&&t| t > 0.0 && t < 100e-9).count();
    assert!(transition_count >= 90, "Expected ~100 events over 100ns, got {}", transition_count);
}

// ===================================================================
// D2A DEVICE UNIT TESTS
// ===================================================================

#[test]
fn test_d2a_voltage_ramp() {
    let mut d = D2ADevice::new(DigitalNet(0));
    let mut q = BinaryHeap::new();
    let ctx = EvalCtx { time: 0.0, nets: &[LogicValue::One], analog: &[] };
    let mut seq = 0u64;
    let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
    d.comb_phase(&ctx, &mut sink);
    assert!((d.voltage_at(0.0)    - 0.0).abs() < 1e-12, "At t=0, output=0 (v_from)");
    assert!((d.voltage_at(50e-12) - 0.9).abs() < 1e-12, "Midpoint of 100ps rise = 0.9V");
    assert!((d.voltage_at(100e-12)- 1.8).abs() < 1e-12, "End of rise = 1.8V");
    assert!((d.voltage_at(200e-12)- 1.8).abs() < 1e-12, "After rise, holds at 1.8V");
}

#[test]
fn test_d2a_no_restart_on_same_value() {
    let mut d = D2ADevice::new(DigitalNet(0));
    let mut q = BinaryHeap::new();
    let mut seq = 0u64;
    {
        let ctx = EvalCtx { time: 0.0, nets: &[LogicValue::One], analog: &[] };
        let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
        d.comb_phase(&ctx, &mut sink);
    }
    let ts = d.transition_start_time;
    {
        let ctx = EvalCtx { time: 5e-9, nets: &[LogicValue::One], analog: &[] };
        let mut sink = QueueSink::new(&mut q, 5e-9, 0, &mut seq);
        d.comb_phase(&ctx, &mut sink); // same value
    }
    assert_eq!(d.transition_start_time, ts, "No transition restart on same value");
}

#[test]
fn test_d2a_x_holds_voltage() {
    let mut d = D2ADevice::new(DigitalNet(0));
    let mut q = BinaryHeap::new();
    let mut seq = 0u64;
    {
        let ctx = EvalCtx { time: 0.0, nets: &[LogicValue::One], analog: &[] };  // start rising
        let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
        d.comb_phase(&ctx, &mut sink);
    }
    {
        let ctx = EvalCtx { time: 5e-9, nets: &[LogicValue::X], analog: &[] };   // X: hold
        let mut sink = QueueSink::new(&mut q, 5e-9, 0, &mut seq);
        d.comb_phase(&ctx, &mut sink);
    }
    assert!((d.target_voltage - 1.8).abs() < 1e-12, "X should hold at 1.8V");
}

#[test]
fn test_d2a_interrupted_ramp() {
    let mut d = D2ADevice::new(DigitalNet(0));
    let mut q = BinaryHeap::new();
    let mut seq = 0u64;
    {
        let ctx = EvalCtx { time: 0.0, nets: &[LogicValue::One], analog: &[] };    // start rising 0→1.8V
        let mut sink = QueueSink::new(&mut q, 0.0, 0, &mut seq);
        d.comb_phase(&ctx, &mut sink);
    }
    {
        let ctx = EvalCtx { time: 50e-12, nets: &[LogicValue::Zero], analog: &[] }; // interrupt at midpoint
        let mut sink = QueueSink::new(&mut q, 50e-12, 0, &mut seq);
        d.comb_phase(&ctx, &mut sink);
    }
    assert!((d.v_from - 0.9).abs() < 1e-9, "v_from should be midpoint 0.9V");
    assert_eq!(d.target_voltage, 0.0);
}

// ===================================================================
// A2D STATE UNIT TESTS
// ===================================================================

#[test]
fn test_a2d_rising_crossing() {
    let mut a2d = A2DState::default(); // threshold=0.9
    let result = a2d.check_crossing(0.0, 1.8, 0.0, 10e-9);
    let (t, v) = result.expect("Should detect crossing");
    assert_eq!(v, LogicValue::One);
    assert_eq!(t, 10e-9); // t_cross < t_now → clamped
}

#[test]
fn test_a2d_falling_crossing() {
    let mut a2d = A2DState { last_value: LogicValue::One, ..Default::default() };
    let result = a2d.check_crossing(1.8, 0.0, 0.0, 10e-9);
    let (_, v) = result.expect("Should detect falling crossing");
    assert_eq!(v, LogicValue::Zero);
}

#[test]
fn test_a2d_hysteresis_blocks_small_swing() {
    let mut a2d = A2DState { last_value: LogicValue::Zero, ..A2DState::new(1.0, 0.2) };
    // thresh_high = 1.1; v_now = 1.05 doesn't reach it
    let result = a2d.check_crossing(0.95, 1.05, 0.0, 10e-9);
    assert!(result.is_none(), "Swing below hysteresis threshold should not trigger");
}

#[test]
fn test_a2d_no_duplicate_crossings() {
    let mut a2d = A2DState::default();
    a2d.check_crossing(0.0, 1.8, 0.0, 10e-9); // first crossing → One
    let result = a2d.check_crossing(1.5, 1.9, 10e-9, 20e-9); // already One
    assert!(result.is_none(), "Duplicate rising crossing should be suppressed");
}

#[test]
fn test_a2d_rising_then_falling() {
    let mut a2d = A2DState::new(1.0, 0.0);
    let r1 = a2d.check_crossing(0.0, 2.0, 0.0, 10e-9);
    assert_eq!(r1.unwrap().1, LogicValue::One);
    // No second rise
    assert!(a2d.check_crossing(1.5, 2.0, 10e-9, 20e-9).is_none());
    // Falling
    let r3 = a2d.check_crossing(2.0, 0.0, 20e-9, 30e-9);
    assert_eq!(r3.unwrap().1, LogicValue::Zero);
}

// ===================================================================
// D2A COSIM INTEGRATION
// ===================================================================

#[test]
fn test_cosim_d2a_event_at_correct_time() {
    let mut instance = make_instance("D2ACosim");
    let mut state = DigitalState::new(1);
    state.schedule(DigitalEvent { time: 5e-9, net: DigitalNet(0), value: LogicValue::One, source: 0, seq: 0 });

    let mut d2a = Box::new(D2ADevice::new(DigitalNet(0)));
    let d2a_ptr: *mut D2ADevice = &mut *d2a;
    instance.devices.push(d2a as Box<dyn Element>);
    instance.digital_state = state;
    instance.rebuild_digital_topology();

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let result = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    assert!(times.iter().any(|&t| (t - 5e-9).abs() < 1e-15), "Solver must clamp to 5ns event");

    let final_d2a = unsafe { &*d2a_ptr };
    assert_eq!(final_d2a.current_value, LogicValue::One);
    assert!((final_d2a.target_voltage - 1.8).abs() < 1e-12);
    assert!((final_d2a.voltage_at(5e-9 + 50e-12) - 0.9).abs() < 1e-6);
}

#[test]
fn test_cosim_d2a_multiple_events() {
    let mut instance = make_instance("D2AMulti");
    let mut state = DigitalState::new(1);
    state.schedule(DigitalEvent { time: 2e-9, net: DigitalNet(0), value: LogicValue::One,  source: 0, seq: 0 });
    state.schedule(DigitalEvent { time: 7e-9, net: DigitalNet(0), value: LogicValue::Zero, source: 0, seq: 1 });

    let mut d2a = Box::new(D2ADevice::new(DigitalNet(0)));
    let d2a_ptr: *mut D2ADevice = &mut *d2a;
    instance.devices.push(d2a as Box<dyn Element>);
    instance.digital_state = state;
    instance.rebuild_digital_topology();

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let result = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    assert!(times.iter().any(|&t| (t - 2e-9).abs() < 1e-15), "Must hit 2ns");
    assert!(times.iter().any(|&t| (t - 7e-9).abs() < 1e-15), "Must hit 7ns");

    let final_d2a = unsafe { &*d2a_ptr };
    assert_eq!(final_d2a.current_value, LogicValue::Zero);
    let v = final_d2a.voltage_at(7e-9 + 50e-12);
    assert!((v - 0.9).abs() < 0.1, "Ramp midpoint ≈0.9V, got {}", v);
}
