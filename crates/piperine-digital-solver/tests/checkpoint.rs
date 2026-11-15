//! Checkpoint, restore and the observation buffer (feature
//! `solver-abi-digital`, SAD-14 and SAD-20; abi.md §5 R6, §6 C5, §9): restore
//! returns store, queue and observations byte for byte with no device call, a
//! design runs the same whatever order it was assembled in, and every change,
//! commit and record is delivered in order.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_digital_solver::abi::engine::{Digital, Horizon, Mode, Outcome};
use piperine_digital_solver::abi::net::NetId;
use piperine_digital_solver::abi::observe::Observation;
use piperine_digital_solver::abi::tick::{Resolution, Tick};
use piperine_digital_solver::limits::DigitalLimits;

const NS: i64 = 1_000_000;

fn engine(devices: Vec<Placed<'_>>) -> Digital {
    Digital::new(bind_by_name(devices, Resolution::default()).expect("binds"), DigitalLimits::default())
}

fn net(d: &Digital, name: &str) -> NetId {
    net_named(d.plan(), name)
}

fn instant(d: &mut Digital, at: Tick) -> Outcome {
    d.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs")
}

/// A divide-by-two: a clock, a flip-flop and an inverter in a loop, and a counter on the clock.
fn divider(order: &[usize]) -> Digital {
    let mut parts: Vec<Option<Placed<'static>>> = vec![
        Some(("clock", timer(), vec![("out", "clk")])),
        Some(("ff0", dff(), vec![("clk", "clk"), ("d", "dn"), ("q", "q")])),
        Some(("inv", inverter(), vec![("a", "q"), ("y", "dn")])),
        Some(("cnt", counter(), vec![("clk", "clk"), ("n", "n")])),
        Some(("ff1", dff(), vec![("clk", "q"), ("d", "dn"), ("q", "q2")])),
    ];
    engine(order.iter().map(|i| parts[*i].take().unwrap()).collect())
}

/// Runs the start and every instant up to 60 ns.
fn run_to_60ns(d: &mut Digital) {
    d.start().unwrap();
    while let Some(t) = d.next_due() {
        if t > Tick(60 * NS) {
            break;
        }
        instant(d, t);
    }
}

#[test]
fn restore_returns_store_queue_and_observations_byte_for_byte() {
    // C30
    let mut d = divider(&[0, 1, 2, 3, 4]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    instant(&mut d, Tick(10 * NS));
    let cp = d.checkpoint();
    let observed = d.observations().to_vec();
    let q = net(&d, "q");
    let q_then = d.read(q).to_vec();
    assert!(!observed.is_empty());

    instant(&mut d, Tick(20 * NS));
    instant(&mut d, Tick(30 * NS));
    assert_ne!(d.checkpoint(), cp, "the run moved on");
    reset_calls();
    d.restore(&cp);
    for device in ["timer", "dff", "inverter", "counter"] {
        assert_eq!(calls(device), (0, 0), "restore called {device}");
    }
    assert_eq!(d.checkpoint(), cp, "store, queue and bookkeeping are as they were");
    assert_eq!(d.observations(), observed.as_slice(), "and so are the observations");
    assert_eq!(d.read(q), q_then.as_slice());

    // the run from the restored state is the run it was
    let mut fresh = divider(&[0, 1, 2, 3, 4]);
    fresh.start().unwrap();
    instant(&mut fresh, Tick(0));
    instant(&mut fresh, Tick(10 * NS));
    instant(&mut d, Tick(20 * NS));
    instant(&mut fresh, Tick(20 * NS));
    assert_eq!(d.observations(), fresh.observations());
}

#[test]
fn two_runs_give_identical_observations() {
    // C31: five instances assembled in two different insertion orders
    let mut a = divider(&[0, 1, 2, 3, 4]);
    let mut b = divider(&[4, 3, 2, 1, 0]);
    run_to_60ns(&mut a);
    run_to_60ns(&mut b);
    let (oa, ob) = (a.drain_observations(), b.drain_observations());
    assert!(oa.len() > 50, "a real run: {} observations", oa.len());
    assert_eq!(oa, ob);
    // and the same plan twice
    let mut c = divider(&[0, 1, 2, 3, 4]);
    run_to_60ns(&mut c);
    assert_eq!(c.drain_observations(), oa);
}

#[test]
fn every_change_commit_and_record_is_observed_in_order() {
    // C37: eight records through one statement
    let mut d = engine(vec![("r", recorder(), vec![("go", "go"), ("log", "log")])]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    d.drain_observations();
    d.fire(net(&d, "go"), Tick(3)).unwrap();
    instant(&mut d, Tick(3));
    let seen = d.drain_observations();
    let records: Vec<&Observation> = seen.iter().filter(|o| matches!(o, Observation::Record { .. })).collect();
    let want: Vec<Observation> = (1..=8u8)
        .map(|i| Observation::Record { stream: "r.log".into(), at: (Tick(3), 0), bytes: vec![i, 1] })
        .collect();
    assert_eq!(records, want.iter().collect::<Vec<_>>(), "eight records, none lost to transport");
    let commits = seen.iter().filter(|o| matches!(o, Observation::Commit { instance, statement: 0, .. } if instance == "r")).count();
    assert_eq!(commits, 8);
    assert_eq!(d.read(net(&d, "log")), [8, 1], "transport leaves the last value in the pin");
    let changes: Vec<&Observation> = seen.iter().filter(|o| matches!(o, Observation::Change { net, .. } if net == "log")).collect();
    assert_eq!(changes.len(), 1, "one applied change of log");
    assert_eq!(seen.first(), Some(&Observation::Fire { net: "go".into(), at: (Tick(3), 0) }));

    // the whole stream of a longer run never goes back in time
    let mut d = divider(&[0, 1, 2, 3, 4]);
    run_to_60ns(&mut d);
    let mut last = (Tick(i64::MIN), 0);
    for o in d.drain_observations() {
        let at = match o {
            Observation::Change { at, .. } | Observation::Fire { at, .. } | Observation::Record { at, .. } | Observation::Unsettled { at, .. } => at,
            Observation::Commit { made, .. } => made,
        };
        assert!(at >= last, "{at:?} after {last:?}");
        last = at;
    }
}

#[test]
fn observations_after_a_checkpoint_are_gone_after_restore() {
    // C38
    let mut d = divider(&[0, 1, 2, 3, 4]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    let cp = d.checkpoint();
    let before = d.observations().len();
    instant(&mut d, Tick(10 * NS));
    assert!(d.observations().len() > before);
    d.restore(&cp);
    assert_eq!(d.observations().len(), before);

    // what the host already drained cannot be taken back, and restore does not fail on it
    instant(&mut d, Tick(10 * NS));
    let drained = d.drain_observations();
    assert!(!drained.is_empty());
    d.restore(&cp);
    assert!(d.observations().is_empty(), "nothing is left to un-say beyond what was drained");
}
