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
use piperine_digital_solver::landing::{LandingCause, LandingTest, TimePointRule};
use piperine_digital_solver::run::{Phase, fired_reactions};
use piperine_digital_solver::store::Store;
use piperine_digital_solver::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::limits::DigitalLimits;

const NS: i64 = 1_000_000; // ticks of 1 fs

/// An engine over `devices` under the declared rule, in which each net of `analog` is read by the
/// analog side.
fn engine_with(devices: Vec<Placed<'_>>, analog: &[&str]) -> Digital {
    Landings::engine_under(devices, analog, TimePointRule::Declared)
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

/// The engines and the walks of the landing tests.
struct Landings;

impl Landings {
    /// `engine_with` under `rule`.
    fn engine_under(devices: Vec<Placed<'_>>, analog: &[&str], rule: TimePointRule) -> Digital {
        let plan = bind_by_name_with(devices, Resolution::default(), |nets: &mut Vec<NetSpec>| {
            for n in nets.iter_mut().filter(|n| analog.contains(&n.name.as_str())) {
                n.analog_reads = true;
            }
        })
        .expect("binds");
        Digital::new(plan, DigitalLimits::default(), rule)
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

    /// Whether reaction 0 of instance `instance` is flagged `(probes, samples)` by the plan of `engine`.
    fn flags(engine: &Digital, instance: u32) -> (bool, bool) {
        let analysis = engine.plan().analysis();
        (analysis.probes(instance, 0), analysis.samples(instance, 0))
    }

    /// The cause `LandingTest` gives for `fire` fired in a fresh store of the design, with `waited`
    /// listened to by a host waiter and `analog_read_changed` as the caller says.
    fn cause_after_functions(devices: Vec<Placed<'_>>, analog: &[&str], fire: &str, waited: &[&str], analog_read_changed: bool) -> Option<LandingCause> {
        let d = engine_with(devices, analog);
        let waited: Vec<NetId> = waited.iter().map(|name| net(&d, name)).collect();
        let mut store = Store::new(&d.plan().nets);
        store.begin_microstep();
        store.fire(net(&d, fire));
        let firing = fired_reactions(&d.plan().instances, Phase::Microstep, &store);
        LandingTest::new(TimePointRule::Declared, d.plan(), &waited).after_functions(&store, &firing, analog_read_changed)
    }

    /// The ticks `advance` lands on before `to`, each instant then run whole as the owner does at the
    /// accepted time, so the walk goes on past it.
    fn landing_ticks(d: &mut Digital, to: Tick) -> Vec<Tick> {
        let mut ticks = Vec::new();
        loop {
            match d.advance(to).unwrap() {
                Advance::Landing { tick, .. } | Advance::Final(tick) => {
                    ticks.push(tick);
                    d.run_instant(tick, &[], Horizon::uniform(to), Mode::default()).unwrap();
                }
                Advance::Reached => return ticks,
            }
        }
    }

    /// Drives each of `names` to a defined LOW at tick 1, so that no later edge or copy is a change
    /// out of the unwritten state (whose bytes are not LOW).
    fn settle_inputs(d: &mut Digital, names: &[&str]) {
        for name in names {
            d.host_commit(net(d, name), 0, &LOW, Tick(1)).unwrap();
        }
        assert_eq!(d.advance(Tick(2)).unwrap(), Advance::Reached);
    }

    /// The setter that commits `above` HIGH, read by an `and2` whose output `d` the analog side reads.
    fn setter_into_analog() -> Vec<Placed<'static>> {
        vec![
            ("cmp", setter(), vec![("up", "cross"), ("out", "above")]),
            ("buf", and2(), vec![("a", "above"), ("b", "above"), ("y", "d")]),
        ]
    }
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
        let mut d = started(Digital::new(plan, DigitalLimits::default(), TimePointRule::default()));
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
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
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
fn a_reaction_reaching_the_analog_side_lands_by_declared_reach() {
    let cause = Landings::landing_cause(
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

#[test]
fn a_reaction_that_reads_a_continuous_pin_samples_and_probes() {
    let d = engine_with(vec![("sm", sampler(), vec![("v", "node"), ("go", "g"), ("s", "out")])], &[]);
    assert_eq!(Landings::flags(&d, 0), (true, true));
}

#[test]
fn a_reaction_that_only_writes_a_reaching_net_probes_and_does_not_sample() {
    let d = engine_with(
        vec![
            ("cmp", setter(), vec![("up", "cross"), ("out", "above")]),
            ("buf", and2(), vec![("a", "above"), ("b", "above"), ("y", "d")]),
        ],
        &["d"],
    );
    assert_eq!(Landings::flags(&d, 0), (true, false));
}

#[test]
fn a_reaction_that_reaches_nothing_neither_probes_nor_samples() {
    let d = engine_with(vec![("cmp", setter(), vec![("up", "cross"), ("out", "above")])], &[]);
    assert_eq!(Landings::flags(&d, 0), (false, false));
}

#[test]
fn the_landing_test_names_the_first_cause_that_holds() {
    let sampler_design = || vec![("sm", sampler(), vec![("v", "node"), ("go", "g"), ("s", "out")])];
    assert_eq!(Landings::cause_after_functions(sampler_design(), &[], "g", &[], true), Some(LandingCause::AnalogReadChanged), "a changed carrier outranks a continuous read");
    assert_eq!(Landings::cause_after_functions(sampler_design(), &[], "g", &[], false), Some(LandingCause::ContinuousRead));
    let stub_design = || vec![("st", stub(), vec![("go", "g"), ("out", "out")])];
    assert_eq!(Landings::cause_after_functions(stub_design(), &[], "g", &["g"], false), Some(LandingCause::Irreversible), "an irreversible reaction outranks a host waiter");
}

#[test]
fn the_landing_test_gives_no_cause_when_nothing_observable_fires() {
    let design = || vec![("cmp", setter(), vec![("up", "cross"), ("out", "above")])];
    assert_eq!(Landings::cause_after_functions(design(), &[], "cross", &[], false), None);
    assert_eq!(Landings::cause_after_functions(design(), &[], "cross", &["cross"], false), Some(LandingCause::HostWaiter));
}

const BOTH_RULES: [TimePointRule; 2] = [TimePointRule::Observable, TimePointRule::Declared];

#[test]
fn a_falling_clock_heard_only_by_a_rising_edge_reaction_makes_no_landing() {
    // TP-02 AC 1: the flip-flop's `q` reaches the analog side but `d` never moves it; its reaction
    // hears only rising edges, so the falling edges at 20 and 40 land under neither rule
    let design = || vec![("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])];
    let drive = |d: &mut Digital| {
        Landings::settle_inputs(d, &["clk", "d"]);
        for (tick, level) in [(10, HIGH), (20, LOW), (30, HIGH), (40, LOW)] {
            d.host_commit(net(d, "clk"), 0, &level, Tick(tick)).unwrap();
        }
    };
    let mut declared = started(Landings::engine_under(design(), &["q"], TimePointRule::Declared));
    drive(&mut declared);
    assert_eq!(Landings::landing_ticks(&mut declared, Tick(45)), [Tick(10), Tick(30)], "the rising edges only, under the declared rule");
    let mut observable = started(Landings::engine_under(design(), &["q"], TimePointRule::Observable));
    drive(&mut observable);
    assert_eq!(Landings::landing_ticks(&mut observable, Tick(45)), [], "q never changes, so nothing is observable");
}

#[test]
fn a_clock_whose_own_reaction_reaches_the_analog_side_lands_every_edge_only_when_declared() {
    // the buck's `Osc`: the timer's reaction commits `out` without LATER, `out` reaches the analog
    // side through the flip-flop, so the declared rule lands on every instant the timer runs
    let design = || vec![("clock", timer(), vec![("out", "clk")]), ("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])];
    let mut declared = started(Landings::engine_under(design(), &["q"], TimePointRule::Declared));
    Landings::settle_inputs(&mut declared, &["d"]);
    assert_eq!(Landings::landing_ticks(&mut declared, Tick(45 * NS)).len(), 4);
    let mut observable = started(Landings::engine_under(design(), &["q"], TimePointRule::Observable));
    Landings::settle_inputs(&mut observable, &["d"]);
    assert_eq!(Landings::landing_ticks(&mut observable, Tick(45 * NS)), []);
}

#[test]
fn a_commit_that_changes_a_carrier_the_analog_side_reads_lands_in_both_rules() {
    // TP-02 AC 2
    for rule in BOTH_RULES {
        let mut d = started(Landings::engine_under(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])], &["y"], rule));
        d.host_commit(net(&d, "a"), 0, &HIGH, Tick(4)).unwrap();
        assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::AnalogReadChanged }, "{rule:?}");
    }
}

#[test]
fn a_reaction_that_reads_a_continuous_pin_lands_in_both_rules() {
    // TP-02 AC 3
    for rule in BOTH_RULES {
        let mut d = started(Landings::engine_under(vec![("sm", sampler(), vec![("v", "node"), ("go", "g"), ("s", "out")])], &[], rule));
        d.fire(net(&d, "g"), Tick(4)).unwrap();
        assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::ContinuousRead }, "{rule:?}");
    }
}

#[test]
fn a_net_no_reaction_that_reaches_the_analog_side_reads_makes_no_landing_in_both_rules() {
    // TP-02 AC 4: the setter's `above` feeds nothing the analog side reads
    for rule in BOTH_RULES {
        let mut d = started(Landings::engine_under(vec![("cmp", setter(), vec![("up", "cross"), ("out", "above")])], &[], rule));
        d.fire(net(&d, "cross"), Tick(4)).unwrap();
        assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Reached, "{rule:?}");
        assert_eq!(d.read(net(&d, "above")), HIGH, "the instant ran ahead");
    }
}

#[test]
fn a_write_to_a_reaching_net_that_changes_nothing_lands_only_under_the_declared_rule() {
    // TP-03 AC 1: the second firing commits the value `above` already holds
    let run = |rule| {
        let mut d = started(Landings::engine_under(Landings::setter_into_analog(), &["d"], rule));
        d.fire(net(&d, "cross"), Tick(4)).unwrap();
        d.fire(net(&d, "cross"), Tick(8)).unwrap();
        (Landings::landing_ticks(&mut d, Tick(10)), d)
    };
    assert_eq!(run(TimePointRule::Declared).0, [Tick(4), Tick(8)]);
    assert_eq!(run(TimePointRule::Observable).0, [Tick(4)], "only the first firing changes d");
}

#[test]
fn a_reaction_on_settle_keeps_every_declared_landing() {
    // TP-03 AC 2: a pin bound to SETTLE (here a settler's) puts the design under the declared rule
    let mut design = Landings::setter_into_analog();
    design.push(("st", settler(), vec![("out", "settled")]));
    let mut d = started(Landings::engine_under(design, &["d"], TimePointRule::Observable));
    d.fire(net(&d, "cross"), Tick(4)).unwrap();
    d.run_instant(Tick(4), &[], Horizon::uniform(Tick(10)), Mode::default()).unwrap();
    d.fire(net(&d, "cross"), Tick(8)).unwrap();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(8), cause: LandingCause::DeclaredReach });
}

#[test]
fn a_host_waiter_keeps_every_declared_landing() {
    // TP-03 AC 3: the waiter listens to another net; its presence is what keeps the rule
    let mut d = started(Landings::engine_under(Landings::setter_into_analog(), &["d"], TimePointRule::Observable));
    d.fire(net(&d, "cross"), Tick(4)).unwrap();
    d.run_instant(Tick(4), &[], Horizon::uniform(Tick(10)), Mode::default()).unwrap();
    d.fire(net(&d, "cross"), Tick(8)).unwrap();
    let elsewhere = net(&d, "above");
    d.wait(elsewhere);
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(8), cause: LandingCause::DeclaredReach });
}

#[test]
fn a_change_found_at_a_later_microstep_rewinds_the_instant_and_leaves_no_record() {
    // TP-03 AC 4: the setter's reaction runs ahead at microstep 0; `d` changes at microstep 1
    let mut d = started(Landings::engine_under(Landings::setter_into_analog(), &["d"], TimePointRule::Observable));
    d.fire(net(&d, "cross"), Tick(4)).unwrap();
    let before = d.checkpoint();
    let observed = d.observations().to_vec();
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::AnalogReadChanged });
    assert_eq!(d.checkpoint(), before, "microstep 0 ran and was undone");
    assert_eq!(d.observations(), observed.as_slice(), "the undone run leaves no observation");
}

#[test]
fn a_commit_delayed_by_an_expression_makes_no_landing_at_its_making() {
    // TP-03 AC 5: the statement carries no LATER, so the declared rule lands where the commit is made
    let design = || {
        vec![
            ("lag", lagged(), vec![("go", "go"), ("out", "x")]),
            ("buf", inverter(), vec![("a", "x"), ("y", "y")]),
        ]
    };
    let mut declared = started(Landings::engine_under(design(), &["y"], TimePointRule::Declared));
    declared.fire(net(&declared, "go"), Tick(NS)).unwrap();
    assert_eq!(declared.advance(Tick(10 * NS)).unwrap(), Advance::Landing { tick: Tick(NS), cause: LandingCause::DeclaredReach });
    let mut observable = started(Landings::engine_under(design(), &["y"], TimePointRule::Observable));
    observable.fire(net(&observable, "go"), Tick(NS)).unwrap();
    assert_eq!(
        observable.advance(Tick(10 * NS)).unwrap(),
        Advance::Landing { tick: Tick(3 * NS), cause: LandingCause::AnalogReadChanged },
        "the commit's own instant is the landing"
    );
}

#[test]
fn two_commits_on_one_tick_land_once() {
    // edge case: `above` changes at microstep 1 of tick 4 and `d` at microstep 2; one landing
    for rule in BOTH_RULES {
        let mut d = started(Landings::engine_under(Landings::setter_into_analog(), &["d"], rule));
        d.fire(net(&d, "cross"), Tick(4)).unwrap();
        assert_eq!(Landings::landing_ticks(&mut d, Tick(10)), [Tick(4)], "{rule:?}");
    }
}

#[test]
fn an_instant_due_at_the_stop_is_left_for_the_step_that_lands_on_it() {
    // edge case: the look-ahead runs instants strictly before the target, so the one due at the
    // stop is the instant of the step that ends there (§17.2: the stop is a time point)
    for rule in BOTH_RULES {
        let mut d = started(Landings::engine_under(vec![("cmp", setter(), vec![("up", "cross"), ("out", "above")])], &[], rule));
        d.fire(net(&d, "cross"), Tick(10)).unwrap();
        assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Reached, "{rule:?}");
        assert_eq!(d.next_due(), Some(Tick(10)), "{rule:?}");
    }
}

#[test]
fn a_finish_at_an_instant_run_ahead_is_final_in_both_rules() {
    // edge case: the instant that returns FINISH is untouched and the step lands on it
    for rule in BOTH_RULES {
        let mut d = started(Landings::engine_under(vec![("o", odd(), vec![])], &[], rule));
        d.fire(net(&d, "o.e1"), Tick(6)).unwrap();
        let before = d.checkpoint();
        assert_eq!(d.advance(Tick(50)).unwrap(), Advance::Final(Tick(6)), "{rule:?}");
        assert_eq!(d.checkpoint(), before, "the instant is untouched: the owner runs it at the accepted time ({rule:?})");
    }
}

#[test]
fn an_irreversible_reaction_lands_in_both_rules() {
    // edge case (abi.md R6)
    for rule in BOTH_RULES {
        let mut d = started(Landings::engine_under(vec![("st", stub(), vec![("go", "g"), ("out", "out")])], &[], rule));
        d.fire(net(&d, "g"), Tick(4)).unwrap();
        assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(4), cause: LandingCause::Irreversible }, "{rule:?}");
    }
}
