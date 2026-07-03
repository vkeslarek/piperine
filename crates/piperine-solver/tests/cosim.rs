//! Mixed-signal integration tests for piperine-solver.
//!
//! Tests:
//! 1. Pure digital ring oscillator (5 inverters, period=10ns)
//! 2. Analog RC transient
//! 3. D2A event scheduling in solver
//! 4. A2D event timing (ramp → threshold crossing)
//! 5. D2A cosim ramp
//! 6. Combinational chain zero-delay (DAG inline propagation)
//! 7. Multi-device topo order validation

use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::path::PathBuf;

use piperine_solver::osdi::model::AnalogModel;
use piperine_solver::analog::{GND, Netlist};
use piperine_solver::circuit::CircuitInstance;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Circuit {
    pub title: String,
    pub components: HashMap<String, OsdiDevice>,
    pub node_counter: AtomicUsize,
}
impl Circuit {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), components: HashMap::new(), node_counter: AtomicUsize::new(0) }
    }
    pub fn port(&self) -> piperine_solver::analog::NodeIdentifier {
        piperine_solver::analog::NodeIdentifier::Anonymous(self.node_counter.fetch_add(1, Ordering::Relaxed))
    }
    pub fn components_mut(&mut self) -> &mut HashMap<String, OsdiDevice> { &mut self.components }

    pub fn instantiate(&self) -> CircuitInstance {
        let mut netlist = Netlist::new();
        let ctx = piperine_solver::solver::Context::default();
        let devices = self.components.values()
            .map(|spec| Box::new(OsdiDevice::from_spec(spec, &mut netlist, &ctx)) as Box<dyn Device>)
            .collect();
        CircuitInstance::from_devices_and_netlist(self.title.clone(), devices, netlist)
    }
}
use piperine_solver::osdi::OsdiDevice;

use piperine_solver::device::Device;
use piperine_solver::topology::{DigitalState, DigitalTopology};

#[path = "helpers/mod.rs"]
mod helpers;
use helpers::{A2DState, D2ADevice};
use piperine_solver::digital::{LogicValue, DigitalNet, DigitalEvent};
use piperine_solver::analysis::transient::TransientAnalysisOptions;
use piperine_solver::solver::Context;

// ---------------------------------------------------------------------------
// Pure Rust device implementations for testing
// ---------------------------------------------------------------------------

struct Inverter {
    input: DigitalNet,
    output: DigitalNet,
    delay: f64,
    id: usize,
}

impl Device for Inverter {
    fn device_name(&self) -> &str { "inverter" }
    fn digital_input_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.input) }
    fn digital_output_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.output) }
    fn eval_discrete(&mut self, t: f64, nets: &[LogicValue], _av: &[f64], q: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        let out = match nets[self.input.0] {
            LogicValue::Zero => LogicValue::One,
            LogicValue::One  => LogicValue::Zero,
            _                => LogicValue::X,
        };
        q.push(Reverse(DigitalEvent { time: t + self.delay, net: self.output, value: out, source: self.id, seq: 0 }));
    }
}

struct DFF {
    inputs: [DigitalNet; 2], // [clk, d]
    q: DigitalNet,
    last_clk: LogicValue,
    clk_to_q: f64,
    id: usize,
}

impl DFF {
    fn new(id: usize, clk: DigitalNet, d: DigitalNet, q: DigitalNet, clk_to_q: f64) -> Self {
        Self { inputs: [clk, d], q, last_clk: LogicValue::Zero, clk_to_q, id }
    }
}

impl Device for DFF {
    fn device_name(&self) -> &str { "dff" }
    fn digital_input_nets(&self) -> &[DigitalNet] { &self.inputs }
    fn digital_output_nets(&self) -> &[DigitalNet] { std::slice::from_ref(&self.q) }
    fn eval_discrete(&mut self, t: f64, nets: &[LogicValue], _av: &[f64], q: &mut BinaryHeap<Reverse<DigitalEvent>>) {
        let clk = nets[self.inputs[0].0];
        let d   = nets[self.inputs[1].0];
        if self.last_clk == LogicValue::Zero && clk == LogicValue::One {
            q.push(Reverse(DigitalEvent { time: t + self.clk_to_q, net: self.q, value: d, source: self.id, seq: 0 }));
        }
        self.last_clk = clk;
    }
}

// ---------------------------------------------------------------------------
// Helpers — compile Verilog-A models
// ---------------------------------------------------------------------------

fn va_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("va")
}

fn compile_va(name: &str) -> (PathBuf, tempfile::TempDir) {
    let va_path = va_dir().join(format!("{name}.va"));
    assert!(va_path.exists(), "VA file not found: {}", va_path.display());
    let tmp = tempfile::tempdir().expect("create tempdir");
    let osdi_path = tmp.path().join(format!("{name}.osdi"));

    let status = std::process::Command::new(env!("OPENVAF_BIN"))
        .arg(&va_path)
        .arg("-o")
        .arg(&osdi_path)
        .status()
        .expect("Failed to invoke openvaf executable. Make sure 'openvaf' is in your PATH.");

    assert!(status.success(), "openvaf compilation failed for {}", name);
    (osdi_path, tmp)
}

fn load_model(name: &str) -> (AnalogModel, tempfile::TempDir) {
    let (osdi_path, tmp) = compile_va(name);
    let model = AnalogModel::load(&osdi_path).expect("Model load failed");
    (model, tmp)
}

fn leak_tmp(tmp: tempfile::TempDir) {
    Box::leak(Box::new(tmp));
}

// ===================================================================
// TEST 1: Pure digital ring oscillator (5 inverters, period=10ns)
// ===================================================================

#[test]
fn test_pure_digital_ring_oscillator() {
    let circuit = Circuit::new("RingOsc");
    let mut instance = circuit.instantiate();

    let mut digital_state = DigitalState::new(5);
    digital_state.schedule(DigitalEvent {
        time: 0.0,
        net: DigitalNet(0),
        value: LogicValue::Zero,
        source: 999,
        seq: 0,
    });

    for i in 0..5 {
        instance.devices.push(Box::new(Inverter {
            input: DigitalNet(i),
            output: DigitalNet((i + 1) % 5),
            delay: 1e-9,
            id: i,
        }));
    }

    instance.digital_state = digital_state;

    let options = TransientAnalysisOptions::new(100e-9.into(), 1e-9.into());
    let mut solver = instance.transient(options, Context::default()).unwrap();
    let result = solver.solve().unwrap();

    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();

    assert!(times.len() >= 100, "Expected at least 100 steps, got {}", times.len());
    let hit_100ns = times.iter().any(|&t| (t - 100e-9).abs() < 1e-15);
    assert!(hit_100ns, "Solver should hit 100ns exactly");

    let ns_boundary_count = times
        .iter()
        .filter(|&&t| {
            let ns = t * 1e9;
            (ns - ns.round()).abs() < 1e-6 && ns >= 1.0
        })
        .count();
    assert!(
        ns_boundary_count >= 90,
        "Expected solver to step on most 1ns boundaries, got {} hits",
        ns_boundary_count
    );
}

// ===================================================================
// TEST 2: Analog RC transient
// ===================================================================

#[test]
fn test_analog_rc_transient() {
    let (resistor, t1) = load_model("resistor");
    let (capacitor, t2) = load_model("capacitor");
    let (isource, t3) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);
    leak_tmp(t3);

    let mut circuit = Circuit::new("RC_Transient");
    let n1 = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, n1.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n1.clone(), GND], vec![("r".to_string(), 1000.0)]));
    circuit.components_mut().insert("C1".to_string(), OsdiDevice::new_with_params("C1".to_string(), capacitor.lib.clone(), capacitor.descriptor_idx, vec![n1.clone(), GND], vec![("c".to_string(), 1e-9)]));

    let mut instance = circuit.instantiate();

    let stop_time = 5e-6;
    let dt = 50e-9;
    let options = TransientAnalysisOptions::new(stop_time.into(), dt.into());

    let mut solver = instance.transient(options, Context::default()).unwrap();
    let result = solver.solve().unwrap();

    let steps: Vec<_> = result.iter().collect();
    assert!(steps.len() >= 50, "Expected at least 50 steps over 5µs, got {}", steps.len());

    let voltages: Vec<(f64, f64)> = steps
        .iter()
        .filter_map(|s| s.get_node(&n1).map(|v| (s.time(), v)))
        .collect();

    assert!(!voltages.is_empty(), "Should have voltage readings");
    for &(t, v) in &voltages {
        assert!(
            (v - 1.0).abs() < 0.05,
            "Voltage should be near 1.0V at t={:.3}µs, got {:.6}V",
            t * 1e6, v
        );
    }

    let last_time = voltages.last().map(|&(t, _)| t).unwrap_or(0.0);
    assert!((last_time - stop_time).abs() < dt, "Should reach stop_time, last_time={}", last_time);
}

// ===================================================================
// TEST 3: D2A event scheduling in solver
// ===================================================================

#[test]
fn test_d2a_event_scheduling_in_solver() {
    let circuit = Circuit::new("D2A_EventScheduling");
    let mut instance = circuit.instantiate();

    let mut digital_state = DigitalState::new(1);
    digital_state.schedule(DigitalEvent { time: 2e-9, net: DigitalNet(0), value: LogicValue::One,  source: 0, seq: 0 });
    digital_state.schedule(DigitalEvent { time: 7e-9, net: DigitalNet(0), value: LogicValue::Zero, source: 0, seq: 1 });

    let mut d2a = Box::new(D2ADevice::new(DigitalNet(0)));
    let d2a_ptr: *mut D2ADevice = &mut *d2a;

    instance.digital_state = digital_state;
    instance.devices.push(d2a as Box<dyn Device>);

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let result = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    assert!(times.iter().any(|&t| (t - 2e-9).abs() < 1e-15), "Solver should clamp to 2ns. Times: {:?}", times);
    assert!(times.iter().any(|&t| (t - 7e-9).abs() < 1e-15), "Solver should clamp to 7ns. Times: {:?}", times);

    let final_d2a = unsafe { &*d2a_ptr };
    assert_eq!(final_d2a.current_value, LogicValue::Zero);
    assert!((final_d2a.target_voltage - 0.0).abs() < 1e-12);

    let v_mid_rise = final_d2a.voltage_at(7e-9 + 50e-12);
    assert!(
        (v_mid_rise - 0.9).abs() < 0.1,
        "Expected ramp midpoint ~0.9, got {}",
        v_mid_rise
    );
}

// ===================================================================
// TEST 4: A2D event timing (OSDI ramp → threshold crossing)
// ===================================================================

#[test]
fn test_a2d_event_timing() {
    let (vramp, t1) = load_model("vramp");
    leak_tmp(t1);

    let mut circuit = Circuit::new("A2D_Ramp");
    let n_ramp = circuit.port();

    circuit.components_mut().insert("Vramp".to_string(), OsdiDevice::new_with_params(
        "Vramp".to_string(), vramp.lib.clone(), vramp.descriptor_idx,
        vec![n_ramp.clone(), GND],
        vec![("v_start".to_string(), 0.0), ("v_end".to_string(), 2.0), ("ramp_time".to_string(), 10e-9)],
    ));

    let mut instance = circuit.instantiate();
    instance.digital_state = DigitalState::new(1);

    let stop_time = 15e-9;
    let dt = 0.5e-9;
    let options = TransientAnalysisOptions::new(stop_time.into(), dt.into());

    let mut solver = instance.transient(options, Context::default()).unwrap();
    let result = solver.solve().unwrap();

    let steps: Vec<_> = result.iter().collect();
    let voltages: Vec<(f64, f64)> = steps
        .iter()
        .filter_map(|s| s.get_node(&n_ramp).map(|v| (s.time(), v)))
        .collect();

    assert!(voltages.len() >= 20, "Expected at least 20 voltage samples, got {}", voltages.len());

    let (_, v_first) = voltages.first().unwrap();
    let (_, v_last)  = voltages.last().unwrap();
    assert!(v_first.abs() < 0.5, "First voltage should be near 0, got {}", v_first);
    assert!(*v_last > 1.5,       "Last voltage should be near 2V, got {}", v_last);

    let mut a2d = A2DState { last_value: LogicValue::Zero, ..A2DState::new(1.0, 0.0) };
    let mut crossings: Vec<(f64, LogicValue)> = Vec::new();

    for i in 1..voltages.len() {
        let (t_prev, v_prev) = voltages[i - 1];
        let (t_now, v_now)   = voltages[i];
        if let Some(cross) = a2d.check_crossing(v_prev, v_now, t_prev, t_now) {
            crossings.push(cross);
        }
    }

    assert_eq!(crossings.len(), 1, "Expected exactly 1 crossing, got {}", crossings.len());
    assert_eq!(crossings[0].1, LogicValue::One);
    assert_eq!(a2d.last_value, LogicValue::One);
}

// ===================================================================
// TEST 5: D2A cosim ramp
// ===================================================================

#[test]
fn test_cosim_d2a_to_analog_ramp() {
    let circuit = Circuit::new("CosimD2ARamp");
    let mut instance = circuit.instantiate();

    let mut digital_state = DigitalState::new(1);
    digital_state.schedule(DigitalEvent { time: 5e-9, net: DigitalNet(0), value: LogicValue::One, source: 0, seq: 0 });

    let mut d2a = Box::new(D2ADevice::new(DigitalNet(0)));
    let d2a_ptr: *mut D2ADevice = &mut *d2a;

    instance.digital_state = digital_state;
    instance.devices.push(d2a as Box<dyn Device>);

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let result = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    assert!(times.iter().any(|&t| (t - 5e-9).abs() < 1e-15), "Solver should clamp to 5ns. Times: {:?}", times);

    let final_d2a = unsafe { &*d2a_ptr };
    assert_eq!(final_d2a.current_value, LogicValue::One);
    assert!((final_d2a.target_voltage - 1.8).abs() < 1e-12);

    let v_mid = final_d2a.voltage_at(5e-9 + 50e-12);
    assert!((v_mid - 0.9).abs() < 1e-6, "Expected ramp midpoint voltage=0.9, got {}", v_mid);
}

// ===================================================================
// TEST 6: Combinational chain — zero-delay inline DAG propagation
// ===================================================================

#[test]
fn test_combinational_chain_zero_delay() {
    let circuit = Circuit::new("CombChain");
    let mut instance = circuit.instantiate();

    let mut digital_state = DigitalState::new(3);
    digital_state.nets[0] = LogicValue::One;
    digital_state.nets[1] = LogicValue::X;
    digital_state.nets[2] = LogicValue::X;

    digital_state.schedule(DigitalEvent {
        time: 1e-9, net: DigitalNet(0), value: LogicValue::Zero, source: 99, seq: 0,
    });

    instance.devices.push(Box::new(Inverter { input: DigitalNet(0), output: DigitalNet(1), delay: 0.0, id: 0 }));
    instance.devices.push(Box::new(Inverter { input: DigitalNet(1), output: DigitalNet(2), delay: 0.0, id: 1 }));

    instance.digital_state = digital_state;
    instance.rebuild_digital_topology();

    let topo = instance.digital_topology.as_ref().unwrap();
    assert!(topo.back_edges.is_empty(), "Acyclic chain must have no back edges");
    let pos0 = topo.topo_order.iter().position(|&d| d == 0).unwrap();
    let pos1 = topo.topo_order.iter().position(|&d| d == 1).unwrap();
    assert!(pos0 < pos1, "INV0 must precede INV1 in topo order");

    let options = TransientAnalysisOptions::new(2e-9.into(), 1e-9.into());
    let _ = {
        let mut solver = instance.transient(options, Context::default()).unwrap();
        solver.solve().unwrap()
    };

    let nets = &instance.digital_state.nets;
    assert_eq!(nets[0], LogicValue::Zero, "net0 should be Zero");
    assert_eq!(nets[1], LogicValue::One,  "net1 = INV(Zero) = One");
    assert_eq!(nets[2], LogicValue::Zero, "net2 = INV(One) = Zero");
}

// ===================================================================
// TEST 7: Multi-device chain — topo order + DAG topology validation
// ===================================================================

#[test]
fn test_multi_digital_device_topo_order() {
    let circuit = Circuit::new("MultiInvChain");
    let mut instance = circuit.instantiate();

    let mut digital_state = DigitalState::new(6);
    digital_state.nets[0] = LogicValue::One;
    digital_state.schedule(DigitalEvent {
        time: 1e-9, net: DigitalNet(0), value: LogicValue::Zero, source: 99, seq: 0,
    });

    for i in 0..5usize {
        instance.devices.push(Box::new(Inverter {
            input: DigitalNet(i), output: DigitalNet(i + 1), delay: 1e-9, id: i,
        }));
    }

    instance.digital_state = digital_state;
    instance.rebuild_digital_topology();

    let topo = instance.digital_topology.as_ref().unwrap();
    assert_eq!(topo.topo_order.len(), 5);
    assert!(topo.back_edges.is_empty(), "Linear chain has no back edges");

    for i in 0..4 {
        let pi = topo.topo_order.iter().position(|&d| d == i).unwrap();
        let pn = topo.topo_order.iter().position(|&d| d == i + 1).unwrap();
        assert!(pi < pn, "INV{} must precede INV{} in topo order", i, i + 1);
    }

    // Standalone topology build from unified device vec
    let standalone = DigitalTopology::build(&instance.devices);
    assert_eq!(standalone.topo_order.len(), 5);
    assert!(standalone.back_edges.is_empty());

    let options = TransientAnalysisOptions::new(10e-9.into(), 1e-9.into());
    let mut solver = instance.transient(options, Context::default()).unwrap();
    let result = solver.solve().unwrap();

    let times: Vec<f64> = result.iter().map(|s| s.time()).collect();
    assert!(times.iter().any(|&t| (t - 1e-9).abs() < 1e-15), "Should hit 1ns");
    assert!(times.iter().any(|&t| (t - 5e-9).abs() < 1e-15), "Should hit 5ns");
}
