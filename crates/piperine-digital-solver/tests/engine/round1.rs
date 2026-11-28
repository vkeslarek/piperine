//! Regression tests for the round-1 verification findings of
//! `.specs/features/solver-abi-digital/verification.md` (probes of the
//! Verifier, made permanent): HORIZON after the reactions of the microstep,
//! a settled RELAX group leaves nothing due, the RELAX rule per cycle,
//! a never-due REPLACE, host writes range-checked, P3 on every write.

use super::engine_binding::EngineBinding;
use piperine_device_fixtures::digital::{And2, CallCounts, ClockWatch, Rearm, DriveDisjunction, Timer};
use piperine_device_fixtures::binding::Placed;
use piperine_abi::layout::HIGH;
use piperine_digital_solver::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::net::NetId;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::landing::TimePointRule;
use piperine_digital_solver::limits::DigitalLimits;

const NS: i64 = 1_000_000;

#[test]
fn a_settled_relax_group_leaves_nothing_due_after_its_instant() {
    // finding 2, F3: the group's own closing write must not re-dirty it
    CallCounts::reset();
    let mut d = EngineBinding::engine(vec![
        ("res", DriveDisjunction::handle(), vec![("d0", "d"), ("d1", "e"), ("v", "v")]),
        ("keep", And2::handle(), vec![("a", "v"), ("b", "v"), ("y", "d")]),
    ]);
    d.start().unwrap();
    d.host_commit(EngineBinding::net(&d, "e"), 0, &HIGH, Tick(0)).unwrap();
    RearmingTimer::instant(&mut d, Tick(0));
    assert_eq!(d.read(EngineBinding::net(&d, "v")), HIGH);
    assert_eq!(d.next_due(), None, "nothing queued, nothing changed: no second instant at tick 0");
    let before = CallCounts::of("resolver").0;
    d.advance(Tick(100)).unwrap();
    assert_eq!(CallCounts::of("resolver").0, before, "and the group's functions are not called again");
}

#[test]
fn replace_with_an_infinite_delay_removes_the_pending_commits() {
    // finding 4, abi.md §6 C3: a REPLACE statement removes every pending commit of itself even when
    // its own commit never comes due (a statement that is not REPLACE keeps the earlier ones)
    let mut d = EngineBinding::engine(vec![("clock", Timer::handle(), vec![("out", "clk"), ("tick", "t")])]);
    d.start().unwrap();
    RearmingTimer::instant(&mut d, Tick(0));
    assert_eq!(d.next_due(), Some(Tick(10 * NS)), "the start armed the edge and the tick");
    d.host_commit(EngineBinding::net(&d, "clock.half"), 0, &f64::INFINITY.to_le_bytes(), Tick(5)).unwrap();
    d.fire(EngineBinding::net(&d, "global:7"), Tick(5)).unwrap();
    RearmingTimer::instant(&mut d, Tick(5));
    d.drain_observations();
    d.advance(Tick(11 * NS)).unwrap();
    let seen = d.drain_observations();
    use piperine_digital_solver::observe::Observation;
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
    use piperine_digital_solver::net::NetSpec;
    use piperine_digital_solver::plan::DigitalPlan;
    use piperine_digital_solver::error::DigitalError;
    let plan = DigitalPlan::bind(
        vec![NetSpec::discrete("n", 1, 1), NetSpec::event("ev")],
        vec![],
        Resolution::default(),
    )
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
    d.start().unwrap();
    let before = d.checkpoint();
    for (net, offset, bytes) in [(0u32, 1u32, &[1u8, 1][..]), (0, 2, &[1][..]), (0, u32::MAX, &[1, 1][..]), (1, 0, &[1][..]), (9, 0, &[][..])] {
        match d.host_commit(NetId(net), offset, bytes, Tick(1)) {
            Err(DigitalError::HostWrite { .. }) => {}
            other => panic!("net {net} offset {offset} len {}: expected a refusal, got {other:?}", bytes.len()),
        }
    }
    assert_eq!(d.checkpoint(), before, "nothing was queued");
    d.host_commit(NetId(0), 0, &[1], Tick(1)).expect("the last byte of the net is in range");
    d.host_commit(NetId(0), 0, &[1], Tick(1)).expect("the whole net is in range");
    RearmingTimer::instant(&mut d, Tick(1));
}

#[test]
fn horizon_counts_the_commits_other_reactions_make_in_the_same_microstep() {
    // finding 1, abi.md R5: the timer fires `t` again at 20 ns in the microstep in which the clockwatch,
    // listening to `t`, runs; the clockwatch may not run its clock through 20 ns, whatever the order
    // the two reactions are called in
    assert_eq!(RearmingTimer::horizon_with_a_rearming_timer(false), 20 * NS - 1);
    assert_eq!(RearmingTimer::horizon_with_a_rearming_timer(true), 20 * NS - 1);

    // round-2 finding 7: an IRREVERSIBLE reaction runs last and once, so its commits cannot be seen by
    // the others; when one fires with others in a microstep every reaction of it gets the current tick.
    // Here it re-fires `a` 5 ticks on, while the clockwatch also listens to `a`.
    for emulator_first in [true, false] {
        let emu: Placed<'_> = ("emu", Rearm::handle(), vec![("a", "a")]);
        let cw: Placed<'_> = ("cw", ClockWatch::handle(), vec![("go", "a")]);
        let mut d = EngineBinding::engine(if emulator_first { vec![emu, cw] } else { vec![cw, emu] });
        d.start().unwrap();
        RearmingTimer::instant(&mut d, Tick(0));
        d.fire(EngineBinding::net(&d, "a"), Tick(100)).unwrap();
        RearmingTimer::instant(&mut d, Tick(100));
        let seen = d.read(EngineBinding::net(&d, "cw.seen")).to_vec();
        assert_eq!(i64::from_le_bytes(seen[8..].try_into().unwrap()), 100, "emulator_first = {emulator_first}");
        assert_eq!(d.next_due(), Some(Tick(105)), "the re-fire is pending");
    }
    // alone, an irreversible reaction keeps the horizon the queue and the limit give it
    CallCounts::reset();
    let mut d = EngineBinding::engine(vec![("emu", Rearm::handle(), vec![("a", "a")])]);
    d.start().unwrap();
    d.fire(EngineBinding::net(&d, "a"), Tick(100)).unwrap();
    RearmingTimer::instant(&mut d, Tick(100));
    assert_eq!(CallCounts::of("rearm").1, 1);
}

/// An instant that returns nothing, and the horizon read with a timer re-arming its own event.
struct RearmingTimer;

impl RearmingTimer {
    /// The instant helper `instant`.
    fn instant(d: &mut Digital, at: Tick) {
        d.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).unwrap();
    }

    /// What the clockwatch of the instant at 10 ns read as `HORIZON`, with a timer re-arming the event it
    /// listens to in the same microstep. `watch_first` puts the clockwatch first among the instances.
    fn horizon_with_a_rearming_timer(watch_first: bool) -> i64 {
        let clock: Placed<'_> = ("clock", Timer::handle(), vec![("out", "clk"), ("tick", "t")]);
        let watch: Placed<'_> = ("cw", ClockWatch::handle(), vec![("go", "t")]);
        let mut d = EngineBinding::engine(if watch_first { vec![watch, clock] } else { vec![clock, watch] });
        d.start().unwrap();
        RearmingTimer::instant(&mut d, Tick(0));
        assert_eq!(d.next_due(), Some(Tick(10 * NS)));
        RearmingTimer::instant(&mut d, Tick(10 * NS));
        let seen = d.read(EngineBinding::net(&d, "cw.seen")).to_vec();
        assert_eq!(i64::from_le_bytes(seen[..8].try_into().unwrap()), 10 * NS, "TICK");
        i64::from_le_bytes(seen[8..].try_into().unwrap())
    }
}
