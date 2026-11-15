//! Regression tests for the round-1 verification findings of
//! `.specs/features/solver-abi-digital/verification.md` (probes of the
//! Verifier, made permanent): HORIZON after the reactions of the microstep,
//! a settled RELAX group leaves nothing due, the RELAX rule per cycle,
//! a never-due REPLACE, host writes range-checked, P3 on every write.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::layout::HIGH;
use piperine_digital_solver::abi::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::abi::net::NetId;
use piperine_digital_solver::abi::tick::{Resolution, Tick};
use piperine_digital_solver::limits::DigitalLimits;

const NS: i64 = 1_000_000;

fn engine(devices: Vec<Placed<'_>>) -> Digital {
    Digital::new(bind_by_name(devices, Resolution::default()).expect("binds"), DigitalLimits::default())
}

fn net(d: &Digital, name: &str) -> NetId {
    net_named(d.plan(), name)
}

fn instant(d: &mut Digital, at: Tick) {
    d.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).unwrap();
}

#[test]
fn a_settled_relax_group_leaves_nothing_due_after_its_instant() {
    // finding 2, F3: the group's own closing write must not re-dirty it
    reset_calls();
    let mut d = engine(vec![
        ("res", resolver(), vec![("d0", "d"), ("d1", "e"), ("v", "v")]),
        ("keep", and2(), vec![("a", "v"), ("b", "v"), ("y", "d")]),
    ]);
    d.start().unwrap();
    d.host_commit(net(&d, "e"), 0, &HIGH, Tick(0)).unwrap();
    instant(&mut d, Tick(0));
    assert_eq!(d.read(net(&d, "v")), HIGH);
    assert_eq!(d.next_due(), None, "nothing queued, nothing changed: no second instant at tick 0");
    let before = calls("resolver").0;
    d.advance(Tick(100)).unwrap();
    assert_eq!(calls("resolver").0, before, "and the group's functions are not called again");
}

#[test]
fn replace_with_an_infinite_delay_removes_the_pending_commits() {
    // finding 4, abi.md §6 C3: a REPLACE statement removes every pending commit of itself even when
    // its own commit never comes due (a statement that is not REPLACE keeps the earlier ones)
    let mut d = engine(vec![("clock", timer(), vec![("out", "clk"), ("tick", "t")])]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    assert_eq!(d.next_due(), Some(Tick(10 * NS)), "the start armed the edge and the tick");
    d.host_commit(net(&d, "clock.half"), 0, &f64::INFINITY.to_le_bytes(), Tick(5)).unwrap();
    d.fire(net(&d, "global:7"), Tick(5)).unwrap();
    instant(&mut d, Tick(5));
    d.drain_observations();
    d.advance(Tick(11 * NS)).unwrap();
    let seen = d.drain_observations();
    use piperine_digital_solver::abi::observe::Observation;
    let at_10 = |o: &&Observation| match o {
        Observation::Fire { at, .. } | Observation::Change { at, .. } => at.0 == Tick(10 * NS),
        _ => false,
    };
    assert!(
        seen.iter().filter(at_10).any(|o| matches!(o, Observation::Change { net, .. } if net == "clk")),
        "the edge (a statement without REPLACE) was not removed: {seen:?}"
    );
    assert!(
        !seen.iter().filter(at_10).any(|o| matches!(o, Observation::Fire { net, .. } if net == "t")),
        "the tick (a REPLACE statement) was removed by the never-due commit: {seen:?}"
    );
}

#[test]
fn a_host_write_outside_its_net_fails_loud_and_changes_nothing() {
    // finding 5: it used to be accepted and to panic inside the store at the instant
    use piperine_digital_solver::abi::net::NetSpec;
    use piperine_digital_solver::abi::plan::DigitalPlan;
    use piperine_digital_solver::error::DigitalError;
    let plan = DigitalPlan::bind(
        vec![NetSpec::discrete("n", 2, 1), NetSpec::event("ev")],
        vec![],
        Resolution::default(),
    )
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default());
    d.start().unwrap();
    let before = d.checkpoint();
    for (net, offset, bytes) in [(0u32, 1u32, &[1u8, 1, 1][..]), (0, 3, &[1][..]), (0, u32::MAX, &[1, 1][..]), (1, 0, &[1][..]), (9, 0, &[][..])] {
        match d.host_commit(NetId(net), offset, bytes, Tick(1)) {
            Err(DigitalError::HostWrite { .. }) => {}
            other => panic!("net {net} offset {offset} len {}: expected a refusal, got {other:?}", bytes.len()),
        }
    }
    assert_eq!(d.checkpoint(), before, "nothing was queued");
    d.host_commit(NetId(0), 1, &[1], Tick(1)).expect("the last byte of the net is in range");
    d.host_commit(NetId(0), 0, &[1, 1], Tick(1)).expect("the whole net is in range");
    instant(&mut d, Tick(1));
}

#[test]
fn a_non_canonical_write_is_a_fault_through_a_function_a_commit_and_the_host() {
    // finding 6, abi.md P3: a field whose known byte is 0 holds no value, so its value bytes are zero.
    // Checked on every write, in release builds too.
    use piperine_digital_solver::error::DigitalError;
    let fault = |r: Result<_, DigitalError>, what: &str| match r {
        Err(DigitalError::Fault { message, .. }) => assert!(message.contains("P3") && message.contains(what), "{message}"),
        other => panic!("expected a P3 fault naming {what}, got {other:?}"),
    };
    let mut d = engine(vec![("s", sloppy(), vec![("a", "a"), ("y", "y"), ("z", "z"), ("go", "go")])]);
    d.start().unwrap();
    d.host_commit(net(&d, "a"), 0, &HIGH, Tick(1)).unwrap();
    fault(d.run_instant(Tick(1), &[], Horizon::uniform(Tick(9)), Mode::default()), "y");

    let mut d = engine(vec![("s", sloppy(), vec![("a", "a"), ("y", "y"), ("z", "z"), ("go", "go")])]);
    d.start().unwrap();
    d.fire(net(&d, "go"), Tick(1)).unwrap();
    fault(d.run_instant(Tick(1), &[], Horizon::uniform(Tick(9)), Mode::default()), "z");

    // a host commit is a write too
    let mut d = engine(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])]);
    d.start().unwrap();
    d.host_commit(net(&d, "a"), 0, &[1, 0], Tick(1)).unwrap();
    fault(d.run_instant(Tick(1), &[], Horizon::uniform(Tick(9)), Mode::default()), "a");

    // canonical bytes pass: known high, and no value at all
    let mut d = engine(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])]);
    d.start().unwrap();
    d.host_commit(net(&d, "a"), 0, &HIGH, Tick(1)).unwrap();
    d.host_commit(net(&d, "a"), 0, &[0, 0], Tick(2)).unwrap();
    instant(&mut d, Tick(1));
    instant(&mut d, Tick(2));
}

/// What the clockwatch of the instant at 10 ns read as `HORIZON`, with a timer re-arming the event it
/// listens to in the same microstep. `watch_first` puts the clockwatch first among the instances.
fn horizon_with_a_rearming_timer(watch_first: bool) -> i64 {
    let clock: Placed<'_> = ("clock", timer(), vec![("out", "clk"), ("tick", "t")]);
    let watch: Placed<'_> = ("cw", clockwatch(), vec![("go", "t")]);
    let mut d = engine(if watch_first { vec![watch, clock] } else { vec![clock, watch] });
    d.start().unwrap();
    instant(&mut d, Tick(0));
    assert_eq!(d.next_due(), Some(Tick(10 * NS)));
    instant(&mut d, Tick(10 * NS));
    let seen = d.read(net(&d, "cw.seen")).to_vec();
    assert_eq!(i64::from_le_bytes(seen[..8].try_into().unwrap()), 10 * NS, "TICK");
    i64::from_le_bytes(seen[8..].try_into().unwrap())
}

#[test]
fn horizon_counts_the_commits_other_reactions_make_in_the_same_microstep() {
    // finding 1, abi.md R5: the timer fires `t` again at 20 ns in the microstep in which the clockwatch,
    // listening to `t`, runs; the clockwatch may not run its clock through 20 ns, whatever the order
    // the two reactions are called in
    assert_eq!(horizon_with_a_rearming_timer(false), 20 * NS - 1);
    assert_eq!(horizon_with_a_rearming_timer(true), 20 * NS - 1);
}
