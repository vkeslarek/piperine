//! The fixture devices of the ABI engine's tests (feature `solver-abi-digital`,
//! SAD-06 and SAD-08): each passes the ABI's shape validation, and a timer, a
//! flip-flop and an inverter bind into a plan by net name.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::validate::validate;
use piperine_digital_solver::dag::Step;
use piperine_digital_solver::plan::NetWriter;
use piperine_digital_solver::tick::Resolution;

#[test]
fn every_fixture_shape_is_valid() {
    let all = [
        ("inverter", inverter()),
        ("and2", and2()),
        ("dff", dff()),
        ("timer", timer()),
        ("counter", counter()),
        ("resolver", resolver()),
        ("succ", succ()),
        ("recorder", recorder()),
        ("checked", checked()),
        ("faulty", faulty()),
        ("sampler", sampler()),
        ("stub", stub()),
        ("odd", odd()),
        ("rogue", rogue()),
        ("sloppy", sloppy()),
        ("rearm", rearm()),
        ("setter", setter()),
        ("settler", settler()),
        ("delayed", delayed()),
        ("clockwatch", clockwatch()),
    ];
    for (name, handle) in &all {
        validate(handle.shape()).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn fixtures_bind_into_a_plan() {
    // timer -> clk; dff(clk, d, q); inverter feeds d from q: a divide-by-two
    let plan = bind_by_name(
        vec![
            ("clock", timer(), vec![("out", "clk")]),
            ("ff", dff(), vec![("clk", "clk"), ("d", "dn"), ("q", "q")]),
            ("inv", inverter(), vec![("a", "q"), ("y", "dn")]),
        ],
        Resolution::default(),
    )
    .expect("a divider binds");
    let a = plan.analysis();
    assert_eq!(a.writer(net_named(&plan, "clk")), NetWriter::Statements { instance: 0 });
    assert_eq!(a.writer(net_named(&plan, "q")), NetWriter::Statements { instance: 1 });
    assert_eq!(a.writer(net_named(&plan, "dn")), NetWriter::Function { instance: 2, k: 0 });
    assert_eq!(a.writer(net_named(&plan, "global:7")), NetWriter::Host, "the START net is written by the owner");
    assert_eq!(plan.dag().order().len(), 2, "the flip-flop's edge detector and the inverter");
    assert!(plan.dag().order().iter().all(|s| matches!(s, Step::Run(_))));
    assert!(!plan.analysis().later(0, 0, 0), "the timer promises no LATER");
    // the flip-flop's q starts LOW through its init
    assert_eq!(plan.nets[net_named(&plan, "q").0 as usize].init, [0, 1]);
}
