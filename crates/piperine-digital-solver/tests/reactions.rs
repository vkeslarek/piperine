//! Running the reactions of one microstep and taking their commits (feature
//! `solver-abi-digital`, SAD-08, SAD-09 AC 8, SAD-10 and SAD-13; abi.md §5 R1
//! to R3, §6 C1 and C4): once per fired event, one snapshot, a commit due at
//! the next microstep or at a tick, a commit that breaks its statement's range
//! or `LATER` bound is a fault, `FINISH`, and a device fault with its site.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::layout::{HIGH, LOW};
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::observe::{Obs, Observation};
use piperine_digital_solver::plan::DigitalPlan;
use piperine_digital_solver::queue::Queue;
use piperine_digital_solver::run::{Ambient, DirtySet, Fired, fire_net, run_reactions, write_net};
use piperine_digital_solver::store::Store;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::error::DigitalError;

/// One microstep, (tick 5, step 0), of a bound plan.
struct Env {
    plan: DigitalPlan,
    store: Store,
    dirty: DirtySet,
    queue: Queue,
    obs: Obs,
    calls: u32,
}

const AT: (Tick, u32) = (Tick(5), 0);

impl Env {
    fn new(devices: Vec<Placed<'_>>) -> Env {
        let plan = bind_by_name(devices, Resolution::default()).expect("the fixture plan binds");
        let store = Store::new(&plan.nets);
        let dirty = DirtySet::none(&plan);
        Env { plan, store, dirty, queue: Queue::new(), obs: Obs::new(), calls: 0 }
    }

    fn net(&self, name: &str) -> NetId {
        net_named(&self.plan, name)
    }

    fn set(&mut self, name: &str, bytes: &[u8]) {
        let id = self.net(name);
        write_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, id, 0, bytes, AT);
    }

    fn fire(&mut self, name: &str) {
        let id = self.net(name);
        fire_net(&self.plan, &mut self.store, &mut self.dirty, &mut self.obs, id, AT);
    }

    fn react(&mut self) -> Result<Fired, DigitalError> {
        let amb = Ambient::at(AT.0, 1e-15);
        let r = run_reactions(&mut self.plan, &self.store, &[], &amb, &mut self.queue, &mut self.obs, AT, &mut self.calls);
        self.obs.flush();
        r
    }

    /// Every pending commit as (net name, bytes, due), in due order.
    fn pending(&mut self) -> Vec<(String, Vec<u8>, (Tick, u32))> {
        let mut out = Vec::new();
        while let Some(due) = self.queue.next_due() {
            for p in self.queue.take_due(due) {
                out.push((self.plan.nets[p.net.0 as usize].name.clone(), p.bytes, p.due));
            }
        }
        out
    }
}

#[test]
fn a_reaction_runs_once_when_its_event_fired() {
    // C19
    reset_calls();
    let mut e = Env::new(vec![("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    e.set("d", &HIGH);
    let fired = e.react().unwrap();
    assert_eq!((calls("dff").1, fired.ran), (0, 0), "no event fired, no reaction");
    e.fire("ff.rise");
    let fired = e.react().unwrap();
    assert_eq!((calls("dff").1, fired.ran), (1, 1));
    assert_eq!(e.pending(), [("q".to_string(), HIGH.to_vec(), (Tick(5), 1))]);

    // two of its events in one microstep still run the reaction once
    reset_calls();
    let mut e = Env::new(vec![("t", timer(), vec![("out", "clk")])]);
    e.set("t.half", &1e-8f64.to_le_bytes());
    e.fire("global:7");
    e.fire("t.tick");
    e.react().unwrap();
    assert_eq!(calls("timer").1, 1, "start and tick fired together: one run");
}

#[test]
fn reactions_of_one_microstep_read_one_snapshot() {
    // C20: a ring of two flip-flops swaps its values
    let mut e = Env::new(vec![
        ("a", dff(), vec![("clk", "clk"), ("d", "qb"), ("q", "qa")]),
        ("b", dff(), vec![("clk", "clk"), ("d", "qa"), ("q", "qb")]),
    ]);
    e.set("qa", &HIGH);
    e.set("qb", &LOW);
    e.fire("a.rise");
    e.fire("b.rise");
    e.react().unwrap();
    let mut got = e.pending();
    got.sort();
    assert_eq!(
        got,
        [("qa".to_string(), LOW.to_vec(), (Tick(5), 1)), ("qb".to_string(), HIGH.to_vec(), (Tick(5), 1))],
        "each read the other's value from before the microstep"
    );
}

#[test]
fn a_zero_delay_commit_applies_at_the_next_microstep() {
    // C21: after <= 0, and a delay that rounds to no advance
    for half in [0.0f64, -1e-9, 0.4e-15] {
        let mut e = Env::new(vec![("t", timer(), vec![("out", "clk")])]);
        e.set("t.half", &half.to_le_bytes());
        e.fire("t.tick");
        e.react().unwrap();
        let due: Vec<_> = e.pending().into_iter().map(|p| p.2).collect();
        assert_eq!(due, [(Tick(5), 1), (Tick(5), 1)], "half = {half}");
    }
    // a positive delay is due at the nearest tick, at microstep 0
    let mut e = Env::new(vec![("t", timer(), vec![("out", "clk")])]);
    e.set("t.half", &3e-9f64.to_le_bytes());
    e.fire("t.tick");
    e.react().unwrap();
    let due: Vec<_> = e.pending().into_iter().map(|p| p.2).collect();
    assert_eq!(due, [(Tick(5 + 3_000_000), 0), (Tick(5 + 3_000_000), 0)]);
}

#[test]
fn an_infinite_delay_is_never_stored_and_a_nan_delay_is_e0435() {
    let mut e = Env::new(vec![("t", timer(), vec![("out", "clk")])]);
    e.set("t.half", &f64::INFINITY.to_le_bytes());
    e.fire("t.tick");
    e.react().unwrap();
    assert!(e.queue.is_empty(), "+inf never comes due");
    let commits = e.obs.list().iter().filter(|o| matches!(o, Observation::Commit { due: None, .. })).count();
    assert_eq!(commits, 2, "but the commits were made, and observed");

    let mut e = Env::new(vec![("t", timer(), vec![("out", "clk")])]);
    e.set("t.half", &f64::NAN.to_le_bytes());
    e.fire("t.tick");
    match e.react() {
        Err(DigitalError::NanDelay { code, instance, statement }) => assert_eq!((code, instance.as_str(), statement), ("E0435", "t", 0)),
        other => panic!("expected E0435, got {other:?}"),
    }
}

#[test]
fn a_commit_outside_its_statement_range_is_a_fault() {
    // C22: three bytes through a two-byte statement
    let mut e = Env::new(vec![("o", rogue(), vec![])]);
    e.fire("o.go0");
    match e.react() {
        Err(DigitalError::Fault { message, instance, tick, .. }) => {
            assert_eq!((instance.as_str(), tick), ("o", 5));
            assert!(message.contains("statement 0") && message.contains("reaction 0"), "{message}");
        }
        other => panic!("expected a fault, got {other:?}"),
    }
    // a statement the reaction does not have
    let mut e = Env::new(vec![("o", rogue(), vec![])]);
    e.fire("o.go1");
    match e.react() {
        Err(DigitalError::Fault { message, .. }) => assert!(message.contains("statement 9"), "{message}"),
        other => panic!("expected a fault, got {other:?}"),
    }
}

#[test]
fn a_later_commit_below_its_bound_is_a_fault() {
    // C22: the statement promised 1 ns and committed after 0.5 ns
    let mut e = Env::new(vec![("o", odd(), vec![])]);
    e.fire("o.e0");
    match e.react() {
        Err(DigitalError::Fault { message, instance, .. }) => {
            assert_eq!(instance, "o");
            assert!(message.contains("statement 0") && message.contains("LATER"), "{message}");
        }
        other => panic!("expected a fault, got {other:?}"),
    }
    // at a resolution of 1 us the flag is dropped at bind, and the same commit is no fault
    let plan = bind_by_name(vec![("o", odd(), vec![])], Resolution { seconds_per_tick: 1e-6 }).unwrap();
    let mut store = Store::new(&plan.nets);
    let (mut queue, mut obs, mut dirty, mut calls) = (Queue::new(), Obs::new(), DirtySet::none(&plan), 0);
    let mut plan = plan;
    let id = net_named(&plan, "o.e0");
    fire_net(&plan, &mut store, &mut dirty, &mut obs, id, AT);
    let amb = Ambient::at(AT.0, 1e-6);
    run_reactions(&mut plan, &store, &[], &amb, &mut queue, &mut obs, AT, &mut calls).expect("no LATER, no bound");
}

#[test]
fn finish_marks_the_instant_final() {
    // C26
    let mut e = Env::new(vec![("o", odd(), vec![])]);
    e.fire("o.e1");
    let fired = e.react().unwrap();
    assert!(fired.finish);
    let mut e = Env::new(vec![("o", odd(), vec![])]);
    assert!(!e.react().unwrap().finish, "nothing fired");
}

#[test]
fn a_reaction_fault_carries_code_tick_and_instance() {
    let mut e = Env::new(vec![("o", odd(), vec![])]);
    e.fire("o.e2");
    match e.react() {
        Err(DigitalError::Fault { code, message, tick, instance }) => {
            assert_eq!((code, message.as_str(), tick, instance.as_str()), (9, "bang", 5, "o"));
        }
        other => panic!("expected a fault, got {other:?}"),
    }
}

#[test]
fn an_irreversible_reaction_is_reported() {
    let mut e = Env::new(vec![("s", stub(), vec![])]);
    e.fire("s.go");
    let fired = e.react().unwrap();
    assert!(fired.irreversible);
    assert_eq!(e.pending(), [("s.out".to_string(), HIGH.to_vec(), (Tick(5), 1))]);
}
