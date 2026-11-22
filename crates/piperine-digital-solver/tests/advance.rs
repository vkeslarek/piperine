//! Look-ahead (feature `solver-abi-digital`, SAD-15 and SAD-17; abi.md §5 R4
//! to R6 and protocol step 2): `advance` runs the instants before a target and
//! stops before the first one that touches the analog side, restoring that
//! instant whole; `HORIZON` and `TICK`; times that round to one tick.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::layout::{HIGH, LOW};
use piperine_digital_solver::advance::Advance;
use piperine_digital_solver::landing::LandingCause;
use piperine_digital_solver::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::limits::DigitalLimits;

const NS: i64 = 1_000_000; // ticks of 1 fs

/// An engine over `devices` in which each net of `analog` is read by the analog side.
fn engine_with(devices: Vec<Placed<'_>>, analog: &[&str]) -> Digital {
    let plan = bind_by_name_with(devices, Resolution::default(), |nets: &mut Vec<NetSpec>| {
        for n in nets.iter_mut().filter(|n| analog.contains(&n.name.as_str())) {
            n.analog_reads = true;
        }
    })
    .expect("binds");
    Digital::new(plan, DigitalLimits::default())
}

fn net(d: &Digital, name: &str) -> NetId {
    net_named(d.plan(), name)
}

/// Starts the engine and runs its first instant.
fn started(mut d: Digital) -> Digital {
    d.start().unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).unwrap();
    d
}

#[test]
fn advance_stops_before_the_first_time_point_and_leaves_it_untouched() {
    // C32: a host write to an analog-read net at 5 and a clock-free design before it
    let mut d = started(engine_with(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])], &["y"]));
    d.host_commit(net(&d, "a"), 0, &HIGH, Tick(5)).unwrap();
    d.host_commit(net(&d, "a"), 0, &LOW, Tick(9)).unwrap();
    let before = d.checkpoint();
    let observed = d.observations().to_vec();
    assert_eq!(d.advance(Tick(20)).unwrap(), Advance::Landing { tick: Tick(5), cause: LandingCause::AnalogReadChanged });
    assert_eq!(d.checkpoint(), before, "store, queue and bookkeeping are those before the instant");
    assert_eq!(d.observations(), observed.as_slice());
    // the owner then runs the instant whole
    let out = d.run_instant(Tick(5), &[], Horizon::uniform(Tick(20)), Mode::default()).unwrap();
    assert_eq!(out.boundary, [(net(&d, "y"), LOW.to_vec())]);
    assert_eq!(d.advance(Tick(20)).unwrap(), Advance::Landing { tick: Tick(9), cause: LandingCause::AnalogReadChanged }, "the next one is a landing too");
}

#[test]
fn each_clause_of_the_time_point_test_stops_advance() {
    // C34, first half: one case per clause of abi.md R4 (and the irreversible reaction of R6)
    // 1. a net the analog side reads changes
    let mut d = started(engine_with(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])], &["y"]));
    d.host_commit(net(&d, "a"), 0, &HIGH, Tick(4)).unwrap();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::AnalogReadChanged });

    // 2. a reaction that reads a continuous pin fires
    let mut d = started(engine_with(vec![("sm", sampler(), vec![("v", "node"), ("go", "g"), ("s", "out")])], &[]));
    d.fire(net(&d, "g"), Tick(4)).unwrap();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::ContinuousRead });

    // 3. a reaction with a statement without LATER whose net reaches an analog-read net fires
    let mut d = started(engine_with(
        vec![
            ("cmp", setter(), vec![("up", "cross"), ("out", "above")]),
            ("buf", and2(), vec![("a", "above"), ("b", "above"), ("y", "d")]),
        ],
        &["d"],
    ));
    d.fire(net(&d, "cross"), Tick(4)).unwrap();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::DeclaredReach });
    assert!(d.read(net(&d, "above")) != HIGH, "the reaction did not run ahead");

    // 4. an IRREVERSIBLE reaction fires: a landing that is not a time point
    let mut d = started(engine_with(vec![("st", stub(), vec![("go", "g"), ("out", "out")])], &[]));
    d.fire(net(&d, "g"), Tick(4)).unwrap();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::Irreversible });
}

#[test]
fn a_free_running_clock_does_not_stop_advance() {
    // C34, second half: nothing on the analog side reads the clock, 1000 edges in one advance
    let mut d = started(engine_with(
        vec![
            ("clock", timer(), vec![("out", "clk")]),
            ("ff", dff(), vec![("clk", "clk"), ("d", "dn"), ("q", "q")]),
            ("inv", inverter(), vec![("a", "q"), ("y", "dn")]),
        ],
        &[],
    ));
    assert_eq!(d.advance(Tick(10_000 * NS + 5 * NS)).unwrap(), Advance::Reached);
    let edges = d
        .observations()
        .iter()
        .filter(|o| matches!(o, Observation::Change { net, at, .. } if net == "clk" && at.0 > Tick(0)))
        .count();
    assert_eq!(edges, 1000, "an edge every 10 ns up to 10 us");
    assert_eq!(d.next_due(), Some(Tick(10_010 * NS)));
}

#[test]
fn a_later_statement_does_not_make_its_instant_a_time_point() {
    // the LATER commit of `out` would reach the analog side, but it is due at another instant,
    // which is tested on its own
    let mut d = started(engine_with(
        vec![
            ("dl", delayed(), vec![("go", "go"), ("out", "x")]),
            ("buf", inverter(), vec![("a", "x"), ("y", "y")]),
        ],
        &["y"],
    ));
    d.fire(net(&d, "go"), Tick(NS)).unwrap();
    let advanced = d.advance(Tick(10 * NS)).unwrap();
    assert_eq!(advanced, Advance::Landing { tick: Tick(3 * NS), cause: LandingCause::AnalogReadChanged }, "the commit's own instant is the landing");
    assert!(
        d.observations().iter().any(|o| matches!(o, Observation::Commit { instance, .. } if instance == "dl")),
        "the instant that made the commit ran inside advance"
    );
    assert_eq!(d.next_due(), Some(Tick(3 * NS)), "and the commit waits for its instant");
}

#[test]
fn a_landing_found_at_a_later_microstep_restores_the_whole_instant() {
    // abi.md §5 step 2: the setter's reaction (microstep 0) is harmless; its commit raises ff's clk at
    // microstep 1, the edge fires an event the irreversible stub listens to
    let mut d = started(engine_with(
        vec![
            ("set", setter(), vec![("up", "up"), ("out", "clk")]),
            ("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
            ("st", stub(), vec![("go", "ff.rise"), ("out", "sout")]),
        ],
        &[],
    ));
    d.host_commit(net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    assert_eq!(d.advance(Tick(3)).unwrap(), Advance::Reached);
    d.fire(net(&d, "up"), Tick(5)).unwrap();
    let before = d.checkpoint();
    let observed = d.observations().to_vec();
    assert_eq!(d.advance(Tick(9)).unwrap(), Advance::Landing { tick: Tick(5), cause: LandingCause::Irreversible });
    assert_eq!(d.checkpoint(), before, "microstep 0 ran and was undone with the rest");
    assert_eq!(d.observations(), observed.as_slice());
    assert_eq!(d.next_due(), Some(Tick(5)), "the fire is pending again");
}

#[test]
fn horizon_is_before_the_earliest_pending_commit_and_the_limit() {
    // C35
    let mut d = started(engine_with(vec![("w", clockwatch(), vec![("go", "go"), ("seen", "seen")])], &[]));
    let seen = |d: &Digital| {
        let b = d.read(net(d, "seen"));
        (i64::from_le_bytes(b[..8].try_into().unwrap()), i64::from_le_bytes(b[8..].try_into().unwrap()))
    };
    // a commit pending at 500 and a limit of 1000: before the commit
    d.fire(net(&d, "go"), Tick(100)).unwrap();
    d.host_commit(net(&d, "seen"), 0, &[0; 16], Tick(500)).unwrap();
    d.run_instant(Tick(100), &[], Horizon::uniform(Tick(1000)), Mode::default()).unwrap();
    assert_eq!(seen(&d), (100, 499));

    // a limit below the pending commit wins
    d.fire(net(&d, "go"), Tick(200)).unwrap();
    d.run_instant(Tick(200), &[], Horizon::uniform(Tick(300)), Mode::default()).unwrap();
    assert_eq!(seen(&d), (200, 299));

    // nothing pending: the tick before the limit, and never before the instant itself
    let mut d = started(engine_with(vec![("w", clockwatch(), vec![("go", "go"), ("seen", "seen")])], &[]));
    d.fire(net(&d, "go"), Tick(50)).unwrap();
    d.run_instant(Tick(50), &[], Horizon::uniform(Tick(80)), Mode::default()).unwrap();
    assert_eq!(seen(&d), (50, 79));
    d.fire(net(&d, "go"), Tick(90)).unwrap();
    d.run_instant(Tick(90), &[], Horizon::uniform(Tick(90)), Mode::default()).unwrap();
    assert_eq!(seen(&d), (90, 90));
}

#[test]
fn a_reaction_a_crossing_can_reach_gets_the_crossing_horizon() {
    // abi.md R5: `crossing` bounds a reaction a CROSS watch can reach, `stop` the others
    let horizon = |crossing_net: bool| {
        let plan = bind_by_name_with(
            vec![("w", clockwatch(), vec![("go", "go"), ("seen", "seen")])],
            Resolution::default(),
            |nets| nets.iter_mut().find(|n| n.name == "go").unwrap().crossing = crossing_net,
        )
        .unwrap();
        let mut d = started(Digital::new(plan, DigitalLimits::default()));
        d.fire(net(&d, "go"), Tick(10)).unwrap();
        let limit = Horizon { stop: Tick(1000), crossing: Tick(10) };
        d.run_instant(Tick(10), &[], limit, Mode::default()).unwrap();
        let b = d.read(net(&d, "seen"));
        i64::from_le_bytes(b[8..].try_into().unwrap())
    };
    assert_eq!(horizon(true), 10, "reachable by a crossing: the current instant");
    assert_eq!(horizon(false), 999, "not reachable: the stop time");
}

#[test]
fn tick_is_the_instants_tick() {
    let mut d = started(engine_with(vec![("w", clockwatch(), vec![("go", "go"), ("seen", "seen")])], &[]));
    for t in [7, 123_456_789] {
        d.fire(net(&d, "go"), Tick(t)).unwrap();
        d.run_instant(Tick(t), &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).unwrap();
        let b = d.read(net(&d, "seen"));
        assert_eq!(i64::from_le_bytes(b[..8].try_into().unwrap()), t);
    }
}

#[test]
fn a_commit_due_after_the_target_stays_pending() {
    // C36
    let mut d = started(engine_with(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])], &[]));
    for (t, bytes) in [(5, HIGH), (10, LOW), (20, HIGH)] {
        d.host_commit(net(&d, "a"), 0, &bytes, Tick(t)).unwrap();
    }
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Reached);
    assert_eq!(d.read(net(&d, "a")), HIGH, "the commit at 5 ran");
    assert_eq!(d.next_due(), Some(Tick(10)), "the one due at the target did not");
    assert_eq!(d.advance(Tick(15)).unwrap(), Advance::Reached);
    assert_eq!(d.read(net(&d, "a")), LOW);
    assert_eq!(d.next_due(), Some(Tick(20)));
}

#[test]
fn times_that_round_to_one_tick_are_one_instant() {
    // two clocks whose first edges are due 10.2 fs and 9.9 fs after the start: both are tick 10
    let plan = bind_by_name_with(
        vec![("a", timer(), vec![("out", "ca")]), ("b", timer(), vec![("out", "cb")])],
        Resolution::default(),
        |nets| {
            nets.iter_mut().find(|n| n.name == "a.half").unwrap().init = 10.2e-15f64.to_le_bytes().to_vec();
            nets.iter_mut().find(|n| n.name == "b.half").unwrap().init = 9.9e-15f64.to_le_bytes().to_vec();
        },
    )
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default());
    d.start().unwrap();
    assert_eq!(d.advance(Tick(11)).unwrap(), Advance::Reached);
    let edges: Vec<_> = d
        .observations()
        .iter()
        .filter_map(|o| match o {
            Observation::Change { net, at, .. } if at.0 > Tick(0) => Some((net.clone(), *at)),
            _ => None,
        })
        .collect();
    assert_eq!(edges, [("ca".to_string(), (Tick(10), 0)), ("cb".to_string(), (Tick(10), 0))], "one instant, tick 10, microstep 0");
}

#[test]
fn advance_returns_final_at_a_finish() {
    let mut d = started(engine_with(vec![("o", odd(), vec![])], &[]));
    d.fire(net(&d, "o.e1"), Tick(6)).unwrap();
    let before = d.checkpoint();
    assert_eq!(d.advance(Tick(50)).unwrap(), Advance::Final(Tick(6)));
    assert_eq!(d.checkpoint(), before, "the instant is untouched: the owner runs it at the accepted time");
}

/// The landing `advance` finds for a design whose analog-read nets are `analog`, with `fire` fired at tick 4.
fn landing_cause(devices: Vec<Placed<'_>>, analog: &[&str], fire: &str) -> LandingCause {
    let mut d = started(engine_with(devices, analog));
    d.fire(net(&d, fire), Tick(4)).unwrap();
    match d.advance(Tick(10)).unwrap() {
        Advance::Landing { cause, .. } => cause,
        other => panic!("expected a landing, got {other:?}"),
    }
}

#[test]
fn a_changed_analog_read_net_lands_by_analog_read_changed() {
    let mut d = started(engine_with(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])], &["y"]));
    d.host_commit(net(&d, "a"), 0, &HIGH, Tick(4)).unwrap();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::AnalogReadChanged });
}

#[test]
fn a_reaction_that_reads_a_continuous_pin_lands_by_continuous_read() {
    let cause = landing_cause(vec![("sm", sampler(), vec![("v", "node"), ("go", "g"), ("s", "out")])], &[], "g");
    assert_eq!(cause, LandingCause::ContinuousRead);
}

#[test]
fn a_reaction_reaching_the_analog_side_lands_by_declared_reach() {
    let cause = landing_cause(
        vec![
            ("cmp", setter(), vec![("up", "cross"), ("out", "above")]),
            ("buf", and2(), vec![("a", "above"), ("b", "above"), ("y", "d")]),
        ],
        &["d"],
        "cross",
    );
    assert_eq!(cause, LandingCause::DeclaredReach, "the reach flag alone: the reaction reads no continuous pin");
}

#[test]
fn an_irreversible_reaction_lands_by_irreversible() {
    let cause = landing_cause(vec![("st", stub(), vec![("go", "g"), ("out", "out")])], &[], "g");
    assert_eq!(cause, LandingCause::Irreversible);
}

#[test]
fn a_host_waiter_lands_by_host_waiter() {
    let mut d = started(engine_with(vec![("set", setter(), vec![("up", "up"), ("out", "out")])], &[]));
    let up = net(&d, "up");
    d.wait(up);
    d.fire(up, Tick(4)).unwrap();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::HostWaiter });
}

#[test]
fn only_an_irreversible_reaction_and_a_host_waiter_are_not_time_points() {
    use LandingCause::*;
    for (cause, time_point) in [
        (AnalogReadChanged, true),
        (ContinuousRead, true),
        (DeclaredReach, true),
        (Irreversible, false),
        (HostWaiter, false),
    ] {
        assert_eq!(cause.is_time_point(), time_point, "{cause:?}");
    }
}
