//! The fixture devices of the ABI engine's tests (feature `solver-abi-digital`,
//! SAD-06 and SAD-08): each passes the ABI's shape validation, and a timer, a
//! flip-flop and an inverter bind into a plan by net name.

use piperine_device_fixtures::digital::{And2, Checked, ClockWatch, Counter, DataFlipFlop, Delayed, Faulty, Inverter, Odd, Rearm, Recorder, Resolver, Rogue, Sampler, Setter, Settler, Stub, Successor, Timer};
use piperine_device_fixtures::binding::{PlanBinding};
use piperine_abi::validate::validate;
use piperine_digital_solver::dag::Step;
use piperine_digital_solver::plan::NetWriter;
use piperine_digital_solver::tick::Resolution;

#[test]
fn every_fixture_shape_is_valid() {
    let all = [
        ("inverter", Inverter::handle()),
        ("and2", And2::handle()),
        ("dff", DataFlipFlop::handle()),
        ("timer", Timer::handle()),
        ("counter", Counter::handle()),
        ("resolver", Resolver::handle()),
        ("succ", Successor::handle()),
        ("recorder", Recorder::handle()),
        ("checked", Checked::handle()),
        ("faulty", Faulty::handle()),
        ("sampler", Sampler::handle()),
        ("stub", Stub::handle()),
        ("odd", Odd::handle()),
        ("rogue", Rogue::handle()),
        ("rearm", Rearm::handle()),
        ("setter", Setter::handle()),
        ("settler", Settler::handle()),
        ("delayed", Delayed::handle()),
        ("clockwatch", ClockWatch::handle()),
    ];
    for (name, handle) in &all {
        validate(handle.shape()).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn fixtures_bind_into_a_plan() {
    // timer -> clk; dff(clk, d, q); inverter feeds d from q: a divide-by-two
    let plan = PlanBinding::bind_by_name(
        vec![
            ("clock", Timer::handle(), vec![("out", "clk")]),
            ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "dn"), ("q", "q")]),
            ("inv", Inverter::handle(), vec![("a", "q"), ("y", "dn")]),
        ],
        Resolution::default(),
    )
    .expect("a divider binds");
    let a = plan.analysis();
    assert_eq!(a.writer(PlanBinding::net_named(&plan, "clk")), NetWriter::Statements { instance: 0 });
    assert_eq!(a.writer(PlanBinding::net_named(&plan, "q")), NetWriter::Statements { instance: 1 });
    assert_eq!(a.writer(PlanBinding::net_named(&plan, "dn")), NetWriter::Function { instance: 2, k: 0 });
    assert_eq!(a.writer(PlanBinding::net_named(&plan, "global:7")), NetWriter::Host, "the START net is written by the owner");
    assert_eq!(plan.dag().order().len(), 2, "the flip-flop's edge detector and the inverter");
    assert!(plan.dag().order().iter().all(|s| matches!(s, Step::Run(_))));
    assert!(!plan.analysis().later(0, 0, 0), "the timer promises no LATER");
    // the flip-flop's q starts LOW through its init
    assert_eq!(plan.nets[PlanBinding::net_named(&plan, "q").0 as usize].init, [0]);
}
