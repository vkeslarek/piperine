//! Checkpoint, restore and the observation buffer (feature
//! `solver-abi-digital`, SAD-14 and SAD-20; abi.md §5 R6, §6 C5, §9): restore
//! returns store, queue and observations byte for byte with no device call, a
//! design runs the same whatever order it was assembled in, and every change,
//! commit and record is delivered in order.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_digital_solver::engine::{Digital, Horizon, Mode, Outcome};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::tick::{Resolution, Tick};
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
        .map(|i| Observation::Record { stream: "r.log".into(), reaction: 0, statement: 0, at: (Tick(3), 0), bytes: vec![i] })
        .collect();
    assert_eq!(records, want.iter().collect::<Vec<_>>(), "eight records, none lost to transport");
    let commits = seen.iter().filter(|o| matches!(o, Observation::Commit { instance, statement: 0, .. } if instance == "r")).count();
    assert_eq!(commits, 8);
    assert_eq!(d.read(net(&d, "log")), [8], "transport leaves the last value in the pin");
    let changes: Vec<&Observation> = seen.iter().filter(|o| matches!(o, Observation::Change { net, .. } if net == "log")).collect();
    assert_eq!(changes.len(), 1, "one applied change of log");
    assert_eq!(seen.first(), Some(&Observation::Fire { net: "go".into(), at: (Tick(3), 0) }));

    // the whole stream of a longer run never goes back in time
    let mut d = divider(&[0, 1, 2, 3, 4]);
    run_to_60ns(&mut d);
    let mut last = (Tick(i64::MIN), 0);
    for o in d.drain_observations() {
        let at = match o {
            Observation::Change { at, .. } | Observation::Fire { at, .. } | Observation::Record { at, .. } => at,
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

// ---- the journal (HYG-14) ---------------------------------------------------------------

use piperine_abi::layout::{HIGH, LOW};

/// A deterministic stream of small numbers (no dependency on a random crate).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % below
    }
}

/// One random step: a host commit on a bit net a few ns ahead, a host firing of the clock's own
/// `tick` event (the clock re-arms through a REPLACE commit and a transported commit, which remove
/// the commits of their statements that are still pending: the queue's `Removed` undo), or the
/// next due instant.
fn random_step(d: &mut Digital, rng: &mut Lcg) {
    match rng.next(4) {
        0 => {
            let net = net(d, ["q2", "dn"][rng.next(2) as usize]);
            let value = if rng.next(2) == 0 { HIGH } else { LOW };
            let at = Tick(d.next_due().map_or(0, |t| t.0) + rng.next(40) as i64 * NS / 4);
            d.host_commit(net, 0, &value, at).unwrap();
        }
        1 => {
            let at = Tick(d.next_due().map_or(0, |t| t.0) + rng.next(40) as i64 * NS / 4);
            let tick = net(d, "clock.tick");
            d.fire(tick, at).unwrap();
        }
        _ => {
            if let Some(t) = d.next_due() {
                instant(d, t);
            }
        }
    }
}

#[test]
fn a_rewind_equals_the_checkpoint_taken_at_the_mark() {
    // HYG-14 AC 4: random commit and instant sequences, each rewound to a mark and compared with
    // the checkpoint taken at it (store, queue, observations, bookkeeping), no device called.
    let mut d = divider(&[0, 1, 2, 3, 4]);
    d.start().unwrap();
    let mut rng = Lcg(7);
    let mut moved = 0;
    for _ in 0..60 {
        let cp = d.checkpoint();
        let observed = d.observations().to_vec();
        let mark = d.mark();
        for _ in 0..1 + rng.next(6) {
            random_step(&mut d, &mut rng);
        }
        moved += usize::from(d.checkpoint() != cp);
        reset_calls();
        d.rewind(&mark);
        for device in ["timer", "dff", "inverter", "counter"] {
            assert_eq!(calls(device), (0, 0), "rewind called {device}");
        }
        assert_eq!(d.checkpoint(), cp, "the rewound engine equals the checkpoint at the mark");
        assert_eq!(d.observations(), observed.as_slice(), "and so are the observations");
        d.release(mark);
        assert_eq!(d.journaled_nets(), 0, "the release empties the journal");
        // move the base on, so the next trial starts somewhere else
        random_step(&mut d, &mut rng);
    }
    assert!(moved > 40, "the random steps moved the engine ({moved} of 60)");
}

#[test]
fn nested_marks_rewind_in_order() {
    let mut d = divider(&[0, 1, 2, 3, 4]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    let outer_cp = d.checkpoint();
    let outer = d.mark();
    instant(&mut d, Tick(10 * NS));
    let inner_cp = d.checkpoint();
    let inner = d.mark();
    instant(&mut d, Tick(20 * NS));
    instant(&mut d, Tick(30 * NS));
    assert_ne!(d.checkpoint(), inner_cp);
    d.rewind(&inner);
    assert_eq!(d.checkpoint(), inner_cp, "the inner rewind stops at the inner mark");
    d.release(inner);
    d.rewind(&outer);
    assert_eq!(d.checkpoint(), outer_cp, "the outer rewind goes on to the outer mark");
    d.release(outer);
    // and the run from there is the run it was
    let mut fresh = divider(&[0, 1, 2, 3, 4]);
    fresh.start().unwrap();
    instant(&mut fresh, Tick(0));
    instant(&mut d, Tick(10 * NS));
    instant(&mut fresh, Tick(10 * NS));
    assert_eq!(d.observations(), fresh.observations());
}

#[test]
fn a_mark_journals_only_the_nets_that_changed() {
    // HYG-14 AC 5: 1 of 1 000 nets changes between the mark and the rewind: one entry.
    let nets: Vec<NetSpec> = (0..1000).map(|i| NetSpec::discrete(&format!("n{i}"), 1, 1)).collect();
    let plan = piperine_digital_solver::plan::DigitalPlan::bind(nets, vec![], Resolution::default()).unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default());
    d.start().unwrap();
    let mark = d.mark();
    d.host_commit(NetId(417), 0, &HIGH, Tick(7)).unwrap();
    instant(&mut d, Tick(7));
    assert_eq!(d.read(NetId(417)), HIGH);
    assert_eq!(d.journaled_nets(), 1, "one net changed: one entry, not a thousand");
    d.rewind(&mark);
    assert_eq!(d.read(NetId(417)), [0]);
    assert_eq!(d.next_due(), None);
    d.release(mark);
}

/// A `settler` design whose `out` the analog side reads and whose commit has no delay.
fn settler_engine() -> Digital {
    let plan = bind_by_name_with(vec![("s", settler(), vec![("out", "out")])], Resolution::default(), |nets| {
        nets.iter_mut().find(|n| n.name == "out").unwrap().analog_reads = true;
    })
    .expect("binds");
    Digital::new(plan, DigitalLimits::default())
}

/// The settle phase of the instant at tick 0, which makes the held commit.
fn settle_instant(d: &mut Digital) {
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), Mode { is_static: false, settle: true }).expect("the instant runs");
}

#[test]
fn a_restore_brings_back_the_held_settle_commits() {
    // §15 S5: a step rejected after a settle commit was made restores the checkpoint taken after
    // it, which holds the commit for the retried time point; one taken before it holds none
    let mut d = settler_engine();
    d.start().unwrap();
    let before = d.checkpoint();
    settle_instant(&mut d);
    let held = d.checkpoint();
    assert!(d.holds_settle_commits());
    d.drop_settle_commits();
    assert!(!d.holds_settle_commits());
    d.restore(&held);
    assert!(d.holds_settle_commits(), "the commit is held again");
    d.land_settle_commits(Tick(5));
    instant(&mut d, Tick(5));
    assert_eq!(d.read(net(&d, "out")), HIGH, "and lands as it would have");
    d.restore(&before);
    assert!(!d.holds_settle_commits(), "a checkpoint from before the commit holds none");
    // the journal mark of the owner protocol does the same
    let mut d = settler_engine();
    d.start().unwrap();
    settle_instant(&mut d);
    let mark = d.mark();
    d.drop_settle_commits();
    d.rewind(&mark);
    assert!(d.holds_settle_commits(), "a rewind brings the held commits back");
    d.release(mark);
}

#[test]
fn a_restore_takes_back_a_landing() {
    // §15 S5: a step that landed the commits and was rejected takes the landing back: the commit is
    // held again and nothing is due at the time point
    let mut d = settler_engine();
    d.start().unwrap();
    settle_instant(&mut d);
    let cp = d.checkpoint();
    d.land_settle_commits(Tick(5));
    assert_eq!(d.next_due(), Some(Tick(5)));
    d.restore(&cp);
    assert!(d.holds_settle_commits(), "held again");
    assert_eq!(d.next_due(), None, "no commit is due at the time point");
    assert_eq!(d.checkpoint(), cp);
}
