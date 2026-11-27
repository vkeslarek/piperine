//! Scope: a host write of a value to a discrete net, between analyses and during a transient
//! (host-ii HII-12): `CircuitInstance::write_net` is an initial value of every later analysis,
//! and `TransientSolver::schedule_write` commits at its time. The nets are an inverter's, whose
//! output is unknown while its input is, so a write is seen at the output.

use piperine_device_fixtures::mixed;
use piperine_digital_solver::net::NetId;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions};

#[test]
fn write_net_before_a_run_is_an_initial_value() {
    // HII-12 AC 1 and 5: a net the host never writes holds its zero bytes after the operating
    // point, and one written before the run holds the value there, the inverter's output
    // following it
    let mut untouched = HostWritesCases::inverter_circuit();
    untouched.dc(Context::default()).expect("dc").solve().expect("op");
    assert_eq!(HostWritesCases::read(&untouched, "a"), [0], "unwritten input: its zero bytes");

    let mut circuit = HostWritesCases::inverter_circuit();
    circuit.write_net("a", ParamValue::Bool(true)).expect("write");
    circuit.dc(Context::default()).expect("dc").solve().expect("op");
    assert_eq!(HostWritesCases::read(&circuit, "a"), [1], "the written value");
    assert_eq!(HostWritesCases::read(&circuit, "y"), [0], "the inverter of a high input is low");

    // a later write of the same net replaces the earlier one, and a name that is no net is refused
    circuit.write_net("a", ParamValue::Bool(false)).expect("rewrite");
    circuit.dc(Context::default()).expect("dc").solve().expect("op");
    assert_eq!(HostWritesCases::read(&circuit, "y"), [1]);
    let refused = circuit.write_net("nothing", ParamValue::Bool(true)).expect_err("no such net");
    assert!(format!("{refused}").contains("nothing"), "{refused}");
}

#[test]
fn a_scheduled_write_commits_at_its_time() {
    // HII-12 AC 2: a write scheduled at 1 us enters that instant, so the rising edge it makes is
    // counted by a run that reaches 1 us, and by none that stops before it
    assert_eq!(HostWritesCases::counted(2.0e-6, 1.0e-6), 1, "the edge at 1 us is counted by a run to 2 us");
    assert_eq!(HostWritesCases::counted(5.0e-7, 1.0e-6), 0, "a run that stops at 0.5 us never sees it");
    // a write at the start is a write before the run: the clock starts high, so no edge rises
    assert_eq!(HostWritesCases::counted(2.0e-6, 0.0), 0, "a write at 0 is an initial value, not an edge");
}

/// The helpers of the host_writes tests.
pub struct HostWritesCases;

impl HostWritesCases {
    /// The net helper `net`.
    fn net(name: &str) -> Port {
        Port::Net(name.to_string())
    }

    /// The inverter circuit helper `inverter_circuit`.
    fn inverter_circuit() -> CircuitInstance {
        let mut b = CircuitBuilder::new("inverter");
        b.device(mixed::Inverter::module(), "inv", &[], &[HostWritesCases::net("a"), HostWritesCases::net("y")]);
        b.build().expect("build")
    }

    /// The value bytes of the discrete net `name` of the plan.
    fn read(circuit: &CircuitInstance, name: &str) -> Vec<u8> {
        let bound = circuit.bound();
        let at = bound.digital.plan().nets.iter().position(|n| n.name == name).expect("the net is in the plan");
        bound.digital.read(NetId(at as u32)).to_vec()
    }

    /// A rising-edge counter on `clk`, held low by the host from the start; `n` counts the edges.
    fn counted(stop: f64, rise_at: f64) -> u32 {
        let mut b = CircuitBuilder::new("counter");
        b.device(mixed::Counter::module(), "cnt", &[], &[HostWritesCases::net("clk"), HostWritesCases::net("n")]);
        let mut circuit = b.build().expect("build");
        circuit.write_net("clk", ParamValue::Bool(false)).expect("clk low");
        let mut solver = circuit.transient(TransientAnalysisOptions::new(stop, 1.0e-7), Context::default()).expect("tran");
        solver.schedule_write(rise_at, "clk", ParamValue::Bool(true));
        solver.solve().expect("solve");
        mixed::MixedParts::count_of(&HostWritesCases::read(&circuit, "n"))
    }
}
