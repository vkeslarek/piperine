//! Running the discrete functions (feature `solver-abi-digital`, SAD-06,
//! SAD-07 and SAD-13; abi.md §4 F3, §3.1 P2, §9 D1): dirty-driven evaluation
//! in the levelized order, relaxation of cycles through `RELAX` nets, and a
//! device fault carried with its code, tick and instance.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::layout::{HIGH, LOW};
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::observe::{Obs, Observation};
use piperine_digital_solver::plan::DigitalPlan;
use piperine_digital_solver::run::{Ambient, DirtySet, run_functions, write_net};
use piperine_digital_solver::store::Store;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::error::DigitalError;

/// A plan with the state `run_functions` needs, standing for a few microsteps of one instant.
struct Env {
    plan: DigitalPlan,
    store: Store,
    dirty: DirtySet,
    obs: Obs,
    step: u32,
}

impl Env {
    fn new(plan: DigitalPlan) -> Env {
        let store = Store::new(&plan.nets);
        let dirty = DirtySet::none(&plan);
        Env { plan, store, dirty, obs: Obs::new(), step: 0 }
    }

    fn all_dirty(mut self) -> Env {
        self.dirty = DirtySet::all(&self.plan);
        self
    }

    fn net(&self, name: &str) -> NetId {
        net_named(&self.plan, name)
    }

    /// The host writes `bytes` to the net called `name` in the current microstep.
    fn write(&mut self, name: &str, bytes: &[u8]) {
        let id = self.net(name);
        write_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, id, 0, bytes, (Tick(5), self.step));
    }

    fn run(&mut self) -> Result<(), DigitalError> {
        let amb = Ambient::at(Tick(5), 1e-15);
        run_functions(&self.plan, &mut self.store, &mut self.dirty, (Tick(5), self.step), &amb, &mut self.obs)?;
        self.obs.flush();
        Ok(())
    }

    fn next_microstep(&mut self) {
        self.store.begin_microstep();
        self.step += 1;
    }

    fn value(&self, name: &str) -> Vec<u8> {
        self.store.value(self.net(name)).to_vec()
    }
}

fn plan(devices: Vec<Placed<'_>>) -> DigitalPlan {
    bind_by_name(devices, Resolution::default()).expect("the fixture plan binds")
}

#[test]
fn a_function_runs_when_a_read_net_changed() {
    // C11
    reset_calls();
    let mut e = Env::new(plan(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])]));
    e.write("a", &HIGH);
    e.run().unwrap();
    assert_eq!(calls("inverter").0, 1);
    assert_eq!(e.value("y"), LOW, "a known high in, a known low out");
    e.next_microstep();
    e.write("a", &LOW);
    e.run().unwrap();
    assert_eq!(calls("inverter").0, 2);
    assert_eq!(e.value("y"), HIGH);
}

#[test]
fn functions_run_after_every_writer_of_their_reads() {
    // a chain a -> inv1 -> m -> inv2 -> z listed with the sink first runs in one pass
    reset_calls();
    let mut e = Env::new(plan(vec![
        ("sink", inverter(), vec![("a", "m"), ("y", "z")]),
        ("src", inverter(), vec![("a", "a"), ("y", "m")]),
    ]));
    e.write("a", &HIGH);
    e.run().unwrap();
    assert_eq!(e.value("m"), LOW);
    assert_eq!(e.value("z"), HIGH, "the sink saw the new m in the same microstep");
    assert_eq!(calls("inverter").0, 2, "each ran once");
}

#[test]
fn a_function_with_unchanged_inputs_is_not_called() {
    // C12
    reset_calls();
    let mut e = Env::new(plan(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])]));
    e.write("a", &HIGH);
    e.run().unwrap();
    e.next_microstep();
    e.run().unwrap();
    assert_eq!(calls("inverter").0, 1, "nothing changed in the second microstep");
    e.next_microstep();
    e.write("a", &HIGH); // the same bytes: not a change
    e.run().unwrap();
    assert_eq!(calls("inverter").0, 1);
}

#[test]
fn every_function_runs_once_at_the_first_instant() {
    reset_calls();
    let mut e = Env::new(plan(vec![
        ("inv", inverter(), vec![("a", "a"), ("y", "y")]),
        ("and", and2(), vec![("a", "a"), ("b", "b"), ("y", "z")]),
    ]))
    .all_dirty();
    e.run().unwrap();
    assert_eq!(calls("inverter").0, 1);
    assert_eq!(calls("and2").0, 1);
    assert_eq!(e.value("y"), [0, 0], "an unknown input gives an unknown output");
    e.next_microstep();
    e.run().unwrap();
    assert_eq!((calls("inverter").0, calls("and2").0), (1, 1), "and no more");
}

#[test]
fn an_absent_optional_pin_reads_zero_bytes() {
    // d1 of the resolver is OPTIONAL and unbound: it reads as unknown
    let mut e = Env::new(plan(vec![("r", resolver(), vec![("d0", "d0"), ("v", "v")])]));
    e.write("d0", &HIGH);
    e.run().unwrap();
    assert_eq!(e.value("v"), HIGH, "d0 alone decides");
    e.next_microstep();
    e.write("d0", &[0, 0]);
    e.run().unwrap();
    assert_eq!(e.value("v"), [0, 0], "neither drive is known");
}

/// The feedback loop `v = d | e`, `d = v` through a resolver and an AND, with `d` a `RELAX` net.
fn keeper(e_value: &[u8]) -> Env {
    let mut env = Env::new(plan(vec![
        ("res", resolver(), vec![("d0", "d"), ("d1", "e"), ("v", "v")]),
        ("keep", and2(), vec![("a", "v"), ("b", "v"), ("y", "d")]),
    ]));
    env.write("e", e_value);
    env
}

#[test]
fn a_relax_group_settles_within_its_bound() {
    // C16: one RELAX net in the cycle, so at most two passes
    reset_calls();
    let mut e = keeper(&HIGH);
    e.run().unwrap();
    assert_eq!(e.value("v"), HIGH);
    assert_eq!(e.value("d"), HIGH);
    assert_eq!(calls("resolver").0, 2, "one pass that moves, one that does not");
    assert!(e.obs.list().iter().all(|o| !matches!(o, Observation::Unsettled { .. })), "it settled");
    // the net `d` changed once for the run, from nothing to high
    let changes: Vec<_> = e
        .obs
        .list()
        .iter()
        .filter(|o| matches!(o, Observation::Change { net, .. } if net == "d"))
        .collect();
    assert_eq!(changes.len(), 1, "one change of d, not one per pass: {changes:?}");
}

#[test]
fn a_moving_known_field_is_zeroed_and_observed() {
    // C17: v = d | LOW, d = not v never settles; the known field is zeroed
    let mut e = Env::new(plan(vec![
        ("res", resolver(), vec![("d0", "d"), ("d1", "e"), ("v", "v")]),
        ("inv", inverter(), vec![("a", "v"), ("y", "d")]),
    ]));
    e.write("e", &LOW);
    e.run().unwrap();
    assert_eq!(e.value("d"), [0, 0], "zeroed: no value");
    assert_eq!(e.value("v"), [0, 0]);
    let unsettled: Vec<&Observation> =
        e.obs.list().iter().filter(|o| matches!(o, Observation::Unsettled { net, .. } if net == "d")).collect();
    assert_eq!(unsettled.len(), 1, "one observation naming d: {:?}", e.obs.list());
}

#[test]
fn a_moving_field_without_a_known_byte_fails() {
    // C17: a counter field that always moves has nothing to fall back to
    let mut e = Env::new(plan(vec![("s", succ(), vec![("a", "x"), ("y", "x")])])).all_dirty();
    match e.run() {
        Err(DigitalError::Fault { message, instance, .. }) => {
            assert!(message.contains("`x`"), "the net is named: {message}");
            assert!(instance.contains("s"), "the cycle is named: {instance}");
        }
        other => panic!("expected a fault, got {other:?}"),
    }
}

#[test]
fn a_function_fault_carries_code_tick_and_instance() {
    // C25: Faulty sits behind an inverter, outside the parameter cone
    let mut e = Env::new(plan(vec![
        ("front", inverter(), vec![("a", "x"), ("y", "m")]),
        ("f", faulty(), vec![("a", "m"), ("y", "out")]),
    ]));
    e.write("x", &LOW); // m = high: a FAULT
    match e.run() {
        Err(DigitalError::Fault { code, message, tick, instance }) => {
            assert_eq!((code, message.as_str(), tick, instance.as_str()), (7, "boom", 5, "f"));
        }
        other => panic!("expected a fault, got {other:?}"),
    }
    // RESHAPE outside the cone is the same error
    let mut e = Env::new(plan(vec![
        ("front", inverter(), vec![("a", "x"), ("y", "m")]),
        ("f", faulty(), vec![("a", "m"), ("y", "out")]),
    ]));
    e.write("x", &HIGH); // m = low: a RESHAPE
    match e.run() {
        Err(DigitalError::Fault { tick, instance, message, .. }) => {
            assert_eq!((tick, instance.as_str()), (5, "f"));
            assert!(message.contains("another shape"), "{message}");
        }
        other => panic!("expected a fault, got {other:?}"),
    }
}

#[test]
fn changed_is_zero_one_microstep_later() {
    // the flip-flop's edge function reads CHANGED and fires `rise`
    let mut e = Env::new(plan(vec![("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]));
    let rise = e.net("ff.rise");
    e.write("clk", &LOW); // unknown to low is no edge
    e.run().unwrap();
    assert!(!e.store.fired(rise));
    e.next_microstep();
    e.write("clk", &HIGH);
    e.run().unwrap();
    assert!(e.store.fired(rise), "a rising edge in the microstep of the change");
    e.next_microstep();
    assert!(!e.store.changed(e.net("clk")));
    e.dirty = DirtySet::all(&e.plan); // force the function to run again
    e.run().unwrap();
    assert!(!e.store.fired(rise), "the same clk, CHANGED read 0: no edge");
}
