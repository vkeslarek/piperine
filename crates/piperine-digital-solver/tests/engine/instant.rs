//! The microstep loop, the start and the settle phase (feature
//! `solver-abi-digital`, SAD-11, SAD-12 and SAD-16; `docs/spec/part_v_execution.md`
//! §15 and §15.1; abi.md §6, §9): apply, functions, reactions, repeat; the bound;
//! `QUIET`; `START`; `SETTLE`; what an instant reports to the owner.

use piperine_device_fixtures::digital::{And2, CallCounts, Counter, DataFlipFlop, Inverter, Odd, PulseGenerator, Sampler, Setter, SettleEvent, SettleRecorder, Settler, Timer};
use piperine_device_fixtures::binding::{Placed, PlanBinding};
use piperine_abi::layout::{BIT, HIGH, LOW};
use piperine_digital_solver::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::landing::TimePointRule;
use piperine_digital_solver::limits::DigitalLimits;

const NS: i64 = 1_000_000;

#[test]
fn start_commits_inits_and_fires_start() {
    // C28
    let mut d = InstantCases::engine(vec![
        ("clock", Timer::handle(), vec![("out", "clk")]),
        ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    // garbage before the start is dropped by it
    d.host_commit(InstantCases::net(&d, "d"), 0, &HIGH, Tick(3)).unwrap();
    d.start().unwrap();
    assert_eq!(d.read(InstantCases::net(&d, "q")), [0], "the store is zeroed until the first instant applies the inits");
    assert_eq!(d.next_due(), Some(Tick(0)), "inits and START are due at (0, 0); d's commit was dropped");
    let out = InstantCases::instant(&mut d, Tick(0));
    assert_eq!(d.read(InstantCases::net(&d, "q")), LOW, "q's init");
    assert_eq!(d.read(InstantCases::net(&d, "clk")), LOW, "the timer's out init");
    assert_eq!(d.read(InstantCases::net(&d, "clock.half")), 1e-8f64.to_le_bytes(), "the param's default");
    assert!(out.fired.contains(&InstantCases::net(&d, "global:7")), "START fired");
    assert_eq!(d.read(InstantCases::net(&d, "d")), [0], "the dropped commit never applied");
    // the timer reacted to START: its edge and its tick are due at 10 ns
    assert_eq!(d.next_due(), Some(Tick(10 * NS)));
}

#[test]
fn start_at_commits_init_at_its_tick() {
    // HYG-12 AC 2: the inits and START are due at the start tick, not at 0; the timer's reaction
    // (half = 10 ns) lands at the start plus its delay.
    let start = Tick(5 * NS);
    let mut d = InstantCases::engine(vec![
        ("clock", Timer::handle(), vec![("out", "clk")]),
        ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    d.start_at(start).unwrap();
    assert_eq!(d.next_due(), Some(start), "inits and START are due at the start tick");
    let out = InstantCases::instant(&mut d, start);
    assert_eq!(d.read(InstantCases::net(&d, "q")), LOW, "q's init");
    assert!(out.fired.contains(&InstantCases::net(&d, "global:7")), "START fired");
    assert_eq!(d.next_due(), Some(Tick(15 * NS)), "the timer commits at start plus its half period");
}

#[test]
fn a_plan_with_no_code_applies_host_commits_only() {
    let plan = piperine_digital_solver::plan::DigitalPlan::bind(
        vec![NetSpec::discrete("n", 1, 1)],
        vec![],
        Resolution::default(),
    )
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
    d.start().unwrap();
    assert_eq!(d.next_due(), None, "nothing to run");
    d.host_commit(NetId(0), 0, &HIGH, Tick(7)).unwrap();
    assert_eq!(d.next_due(), Some(Tick(7)));
    let out = InstantCases::instant(&mut d, Tick(7));
    assert_eq!(d.read(NetId(0)), HIGH);
    assert_eq!(out.microsteps, 1);
    assert_eq!(d.next_due(), None);
}

#[test]
fn the_traced_instant_of_section_15_1() {
    // C23: `Front` with vin crossing the upper threshold at 10 ns, exactly when clk rises
    let mut d = InstantCases::engine(vec![
        ("clock", Timer::handle(), vec![("out", "clk")]),
        ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
        ("buf", And2::handle(), vec![("a", "above"), ("b", "above"), ("y", "d")]),
        ("cmp", Setter::handle(), vec![("up", "cross"), ("out", "above")]),
    ]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    d.drain_observations();
    d.fire(InstantCases::net(&d, "cross"), Tick(10 * NS)).unwrap(); // the located crossing
    assert_eq!(d.next_due(), Some(Tick(10 * NS)));
    let out = InstantCases::instant(&mut d, Tick(10 * NS));

    let t = Tick(10 * NS);
    let seen = d.drain_observations();
    let at = |n: u32| (t, n);
    // microstep 0: applied, events fired, commits made
    let step0: Vec<&Observation> = seen
        .iter()
        .filter(|o| match o {
            Observation::Change { at: a, .. } | Observation::Fire { at: a, .. } => *a == at(0),
            Observation::Commit { made, .. } => *made == at(0),
            _ => false,
        })
        .collect();
    assert_eq!(
        step0,
        [
            &Observation::Change { net: "clk".into(), at: at(0), bytes: HIGH.to_vec() },
            &Observation::Fire { net: "clock.tick".into(), at: at(0) },
            &Observation::Fire { net: "cross".into(), at: at(0) },
            &Observation::Fire { net: "ff.rise".into(), at: at(0) },
            &Observation::Commit { instance: "clock".into(), reaction: 0, statement: 0, made: at(0), due: Some((Tick(20 * NS), 0)) },
            &Observation::Commit { instance: "clock".into(), reaction: 0, statement: 1, made: at(0), due: Some((Tick(20 * NS), 0)) },
            &Observation::Commit { instance: "cmp".into(), reaction: 0, statement: 0, made: at(0), due: Some(at(1)) },
            &Observation::Commit { instance: "ff".into(), reaction: 0, statement: 0, made: at(0), due: Some(at(1)) },
        ]
    );
    // microstep 1: q and above applied, then d defined; no event; nothing listens to d
    let step1: Vec<&Observation> = seen
        .iter()
        .filter(|o| matches!(o, Observation::Change { at: a, .. } | Observation::Fire { at: a, .. } if *a == at(1)))
        .collect();
    assert_eq!(
        step1,
        [
            &Observation::Change { net: "above".into(), at: at(1), bytes: HIGH.to_vec() },
            &Observation::Change { net: "d".into(), at: at(1), bytes: HIGH.to_vec() },
        ],
        "q took d from before the edge (still zero): no change; above and d changed after it"
    );
    assert!(seen.iter().all(|o| !matches!(o, Observation::Change { at: a, .. } | Observation::Fire { at: a, .. } if a.1 >= 2)));
    assert_eq!(out.microsteps, 2, "(10 ns, 2) has nothing pending: the loop exits");
}

#[test]
fn an_instant_that_never_goes_quiet_is_e0501_naming_the_nets() {
    // C24: a timer with a half period of zero toggles its own trigger forever
    let plan = PlanBinding::bind_by_name_with(vec![("t", Timer::handle(), vec![("out", "clk")])], Resolution::default(), |nets| {
        let half = nets.iter_mut().find(|n| n.name == "t.half").unwrap();
        half.init = 0.0f64.to_le_bytes().to_vec();
    })
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
    d.start().unwrap();
    match d.run_instant(Tick(0), &[], Horizon::uniform(Tick(1000)), Mode::default()) {
        Err(DigitalError::NotQuiet { code, tick, nets }) => {
            assert_eq!((code, tick), ("E0501", 0));
            assert_eq!(nets, ["clk", "t.tick"]);
        }
        other => panic!("expected E0501, got {other:?}"),
    }
}

#[test]
fn a_quiet_commit_does_not_start_a_microstep() {
    // C27: the counter's state is QUIET; a flip-flop's q is not
    let mut d = InstantCases::engine(vec![
        ("c", Counter::handle(), vec![("clk", "clk"), ("n", "n")]),
        ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &HIGH, Tick(2)).unwrap();
    let first = InstantCases::instant(&mut d, Tick(1));
    assert_eq!(first.microsteps, 1, "a low clock: no edge, nothing pending");
    let out = InstantCases::instant(&mut d, Tick(2));
    assert_eq!(d.read(InstantCases::net(&d, "n")), 1u32.to_le_bytes(), "the count applied as the instant exited");
    assert_eq!(out.microsteps, 2, "the flip-flop's q is a commit to a live net: microstep 1 ran, and the quiet n rode along");
    // the counter alone
    let mut d = InstantCases::engine(vec![("c", Counter::handle(), vec![("clk", "clk"), ("n", "n")])]);
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &HIGH, Tick(2)).unwrap();
    InstantCases::instant(&mut d, Tick(1));
    let out = InstantCases::instant(&mut d, Tick(2));
    assert_eq!(d.read(InstantCases::net(&d, "n")), 1u32.to_le_bytes());
    assert_eq!(out.microsteps, 1, "a quiet commit starts no microstep");
}

const SETTLING: Mode = Mode { is_static: false, settle: true };

#[test]
fn a_settle_commit_is_held_until_it_is_landed() {
    // §15 S1: a commit from the settle phase with no advancing delay is held, not queued and not
    // refused; the instant reports what the analog side will read (S2)
    let mut d = InstantCases::settler_engine(0.0);
    d.start().unwrap();
    let out = d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), SETTLING).expect("a settle commit is held");
    assert!(d.holds_settle_commits(), "held");
    assert_eq!(d.next_due(), None, "it is due at no instant until it lands");
    assert_eq!(d.read(InstantCases::net(&d, "out")), [0], "the net keeps its unset value (S4)");
    assert_eq!(out.settle_boundary, vec![(InstantCases::net(&d, "out"), HIGH.to_vec())], "the boundary the next solve reads (S2)");
    // without a settle phase no reaction runs and nothing is held
    let mut d = InstantCases::settler_engine(0.0);
    d.start().unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), Mode::default()).expect("SETTLE never fired");
    assert!(!d.holds_settle_commits());
}

#[test]
fn a_landed_settle_commit_fires_changed_at_microstep_0() {
    // §15 S1: landed at the next time point the commit is due at (t_next, 0), where it changes the net
    let mut d = InstantCases::settler_engine(0.0);
    d.start().unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), SETTLING).unwrap();
    d.land_settle_commits(Tick(5));
    assert!(!d.holds_settle_commits(), "landed commits are no longer held");
    assert_eq!(d.next_due(), Some(Tick(5)), "due at the time point");
    d.drain_observations();
    d.run_instant(Tick(5), &[], Horizon::uniform(Tick(10)), Mode::default()).unwrap();
    assert_eq!(d.read(InstantCases::net(&d, "out")), HIGH, "the commit applied");
    let seen = d.drain_observations();
    assert!(seen.contains(&Observation::Change { net: "out".into(), at: (Tick(5), 0), bytes: HIGH.to_vec() }), "changed at (t_next, 0): {seen:?}");
}

#[test]
fn a_dropped_settle_commit_never_applies() {
    // §15 S6: at the stop time the held commits are dropped
    let mut d = InstantCases::settler_engine(0.0);
    d.start().unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), SETTLING).unwrap();
    d.drop_settle_commits();
    assert!(!d.holds_settle_commits());
    d.land_settle_commits(Tick(5));
    assert_eq!(d.next_due(), None, "nothing to land");
    assert_eq!(d.read(InstantCases::net(&d, "out")), [0], "it never applied");
}

#[test]
fn a_settle_commit_with_an_advancing_delay_is_an_ordinary_commit() {
    // §15 S7: a delay that advances time is queued at its time, never held
    let mut d = InstantCases::settler_engine(1e-9);
    d.start().unwrap();
    let out = d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), SETTLING).expect("a delayed settle commit");
    assert!(!d.holds_settle_commits());
    assert!(out.settle_boundary.is_empty());
    assert_eq!(d.next_due(), Some(Tick(NS)));
}

#[test]
fn a_guarded_settle_event_fires_in_the_settle_phase() {
    // §15 step 4 and §13 rules 1, 14, 17: the settle phase fires `SETTLE`, runs the discrete
    // functions of its cone (V2), so `settle && en.high()` fires its event pin, and its reaction
    // runs; the guard decides
    let settle = Mode { is_static: false, settle: true };
    let run = |en: &[u8], mode: Mode| {
        let mut d = InstantCases::settle_event_engine(1e-9, vec![]);
        d.start().unwrap();
        d.host_commit(InstantCases::net(&d, "en"), 0, en, Tick(0)).unwrap();
        d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10 * NS)), mode).expect("the instant runs");
        (d.next_due(), CallCounts::of("settle_event").1)
    };
    CallCounts::reset();
    assert_eq!(run(&HIGH, settle), (Some(Tick(NS)), 1), "the guard holds: the event fired and its commit is due 1 ns on");
    CallCounts::reset();
    assert_eq!(run(&LOW, settle), (None, 0), "the guard is false: the event did not fire");
    CallCounts::reset();
    assert_eq!(run(&HIGH, Mode::default()), (None, 0), "no settle phase: SETTLE never fired");
}

#[test]
fn a_reaction_in_the_settle_phase_is_on_settle_whatever_its_head() {
    // §13 rule 14, D4: the reaction listens to the private event pin `seen`, not to `SETTLE`, yet it
    // runs in the settle phase, so a commit with no advancing delay is a settle commit (S1): held
    let mut d = InstantCases::settle_event_engine(0.0, vec![]);
    d.start().unwrap();
    d.host_commit(InstantCases::net(&d, "en"), 0, &HIGH, Tick(0)).unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), SETTLING).expect("the commit is held");
    assert!(d.holds_settle_commits(), "held, not applied and not refused");
    assert_eq!(d.next_due(), None);
}

#[test]
fn settle_records_carry_the_last_microstep_index() {
    // §19 Resolution, V1, D3: the settle phase records under (t, n), n the last microstep the
    // instant ran. A rising clock makes the flip-flop commit `q` into microstep 1, so n = 1
    let mut d = InstantCases::settle_event_engine(1e-9, vec![("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0)); // the inits, `delay` included, apply at (0, 0)
    d.host_commit(InstantCases::net(&d, "en"), 0, &HIGH, Tick(1)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &HIGH, Tick(2)).unwrap();
    InstantCases::instant(&mut d, Tick(1));
    d.drain_observations();
    let out = d.run_instant(Tick(2), &[], Horizon::uniform(Tick(10 * NS)), Mode { is_static: false, settle: true }).expect("runs");
    assert_eq!(out.microsteps, 2, "the loop ran microsteps 0 and 1");
    let seen = d.drain_observations();
    let at = (Tick(2), 1);
    assert!(seen.contains(&Observation::Fire { net: "global:8".into(), at }), "SETTLE fires at (t, n): {seen:?}");
    assert!(seen.contains(&Observation::Fire { net: "s.seen".into(), at }), "the settle event fires at (t, n): {seen:?}");
    assert!(
        seen.iter().any(|o| matches!(o, Observation::Commit { instance, made, .. } if instance == "s" && *made == at)),
        "its commit is made at (t, n): {seen:?}"
    );
    assert!(seen.iter().all(|o| !matches!(o, Observation::Fire { at: (_, n), .. } if *n > 1)), "nothing is stamped past n: {seen:?}");
}

#[test]
fn a_record_from_the_settle_phase_is_stamped_with_the_last_microstep() {
    // §19 rules 7 and 8: a record from the settle phase carries the time point's `t` and the index
    // of its last microstep. It is observed where it is made and is no settle commit (§15 S1), so
    // `on settle { emit!(..) }` neither fails nor waits for the next time point. A rising clock makes
    // the instant run microsteps 0 and 1, so the record is at `(t, 1)`.
    let mut d = InstantCases::engine(vec![
        ("sr", SettleRecorder::handle(), vec![("log", "log")]),
        ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &HIGH, Tick(2)).unwrap();
    InstantCases::instant(&mut d, Tick(1));
    d.drain_observations();
    d.run_instant(Tick(2), &[], Horizon::uniform(Tick(10 * NS)), Mode { is_static: false, settle: true }).expect("a record is no settle commit");
    let seen = d.drain_observations();
    let records: Vec<&Observation> = seen.iter().filter(|o| matches!(o, Observation::Record { .. })).collect();
    assert_eq!(records, [&Observation::Record { stream: "sr.log".into(), reaction: 0, statement: 0, at: (Tick(2), 1), bytes: vec![7] }], "{seen:?}");
    assert_eq!(d.next_due(), None, "the record applied as the instant exited: nothing stays due at this tick");
}

#[test]
fn an_instant_reports_boundary_fired_and_final() {
    // C33: the sampler reads its sample; `out` is read by the analog side; the stub finishes
    let plan = PlanBinding::bind_by_name_with(
        vec![
            ("sm", Sampler::handle(), vec![("v", "node"), ("go", "g"), ("s", "out")]),
            ("o", Odd::handle(), vec![("e1", "fin")]),
        ],
        Resolution::default(),
        |nets| nets.iter_mut().find(|n| n.name == "out").unwrap().analog_reads = true,
    )
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
    d.start().unwrap();
    d.fire(InstantCases::net(&d, "g"), Tick(7)).unwrap();
    d.fire(InstantCases::net(&d, "fin"), Tick(7)).unwrap();
    let out = d.run_instant(Tick(7), &[2.5], Horizon::uniform(Tick(100)), Mode::default()).unwrap();
    assert_eq!(out.boundary, [(InstantCases::net(&d, "out"), InstantCases::real(2.5).to_vec())], "the sample went through the reaction");
    let mut want = vec![InstantCases::net(&d, "g"), InstantCases::net(&d, "fin")];
    want.sort();
    assert_eq!(out.fired, want);
    assert!(out.finish, "the Odd reaction on e1 returned FINISH");
    assert!(!out.host_param_write);
    // a host write to a PARAM net is reported
    let mut d = InstantCases::engine(vec![("s", Settler::handle(), vec![("out", "out")])]);
    d.host_commit(InstantCases::net(&d, "s.delay"), 0, &2e-9f64.to_le_bytes(), Tick(3)).unwrap();
    assert!(InstantCases::instant(&mut d, Tick(3)).host_param_write);
}

#[test]
fn two_commits_to_one_net_settle_to_one_write() {
    // DSV-01 AC 1 (§12.1.1, §12.1.2): the commits of a microstep apply together, the later one
    // winning each byte it writes. `d` holds [0]; LOW ([0]) then HIGH ([1]) settle to HIGH: one
    // write, one change
    let mut d = InstantCases::engine(vec![("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    assert_eq!(d.read(InstantCases::net(&d, "d")), [0]);
    d.drain_observations();
    let t = Tick(5 * NS);
    d.host_commit(InstantCases::net(&d, "d"), 0, &LOW, t).unwrap();
    d.host_commit(InstantCases::net(&d, "d"), 0, &HIGH, t).unwrap();
    InstantCases::instant(&mut d, t);
    assert_eq!(d.read(InstantCases::net(&d, "d")), HIGH, "the later commit wins its byte");
    let seen = d.drain_observations();
    assert_eq!(InstantCases::changes(&seen, "d", t, 0), vec![HIGH.to_vec()], "one change, to the settled value: {seen:?}");
}

#[test]
fn a_commit_undone_in_its_microstep_is_no_change() {
    // DSV-01 AC 2 (§8.2.3, §12.1.18): clk holds HIGH; LOW then HIGH in one microstep settle to
    // HIGH, the value it had: no change, CHANGED reads false, so the flip-flop's rising edge (it
    // reads VALUE, PREVIOUS and CHANGED) does not fire and q keeps the d sampled before
    CallCounts::reset();
    let mut d = InstantCases::engine(vec![("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    d.host_commit(InstantCases::net(&d, "d"), 0, &LOW, Tick(NS)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, Tick(NS)).unwrap();
    InstantCases::instant(&mut d, Tick(NS));
    d.host_commit(InstantCases::net(&d, "clk"), 0, &HIGH, Tick(2 * NS)).unwrap();
    InstantCases::instant(&mut d, Tick(2 * NS));
    assert_eq!(d.read(InstantCases::net(&d, "q")), LOW, "the edge at 2 ns sampled d = LOW");
    let sampled = CallCounts::of("dff").1;
    assert_eq!(sampled, 1, "one rising edge so far");
    d.host_commit(InstantCases::net(&d, "d"), 0, &HIGH, Tick(3 * NS)).unwrap();
    InstantCases::instant(&mut d, Tick(3 * NS));
    d.drain_observations();

    let t = Tick(4 * NS);
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, t).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &HIGH, t).unwrap();
    InstantCases::instant(&mut d, t);
    let seen = d.drain_observations();
    assert_eq!(d.read(InstantCases::net(&d, "clk")), HIGH);
    assert!(InstantCases::changes(&seen, "clk", t, 0).is_empty(), "the settled clk is the one before: no change: {seen:?}");
    assert!(
        !seen.iter().any(|o| matches!(o, Observation::Fire { net, .. } if net == "ff.rise")),
        "no edge fired: {seen:?}"
    );
    assert_eq!(CallCounts::of("dff").1, sampled, "the flip-flop did not sample again");
    assert_eq!(d.read(InstantCases::net(&d, "q")), LOW, "q did not take d = HIGH");
}

#[test]
fn a_settled_change_is_one_observation() {
    // DSV-01 AC 3: clk holds LOW; HIGH, LOW, HIGH in one microstep settle to HIGH: one change,
    // to HIGH, with PREVIOUS = LOW (the value before the microstep), so the rising edge fires
    // once and q samples d
    CallCounts::reset();
    let mut d = InstantCases::engine(vec![("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    d.host_commit(InstantCases::net(&d, "d"), 0, &HIGH, Tick(NS)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, Tick(NS)).unwrap();
    InstantCases::instant(&mut d, Tick(NS));
    d.drain_observations();

    let t = Tick(2 * NS);
    for bytes in [HIGH, LOW, HIGH] {
        d.host_commit(InstantCases::net(&d, "clk"), 0, &bytes, t).unwrap();
    }
    InstantCases::instant(&mut d, t);
    let seen = d.drain_observations();
    assert_eq!(InstantCases::changes(&seen, "clk", t, 0), vec![HIGH.to_vec()], "one change, to HIGH: {seen:?}");
    let rises = seen.iter().filter(|o| matches!(o, Observation::Fire { net, .. } if net == "ff.rise")).count();
    assert_eq!(rises, 1, "PREVIOUS is LOW, VALUE HIGH: one rising edge");
    assert_eq!(CallCounts::of("dff").1, 1);
    assert_eq!(d.read(InstantCases::net(&d, "q")), HIGH, "q sampled d");
}

/// Copies the `PREVIOUS` of its input `x` to `p`.
/// pins: x=0, p=1 (PORT F64);  fn 0 "prev": reads [x.VALUE, x.PREVIOUS] writes [p]
struct Previous;
static PREVIOUS_SHAPE: std::sync::OnceLock<piperine_abi::shape::ShapeRef> = std::sync::OnceLock::new();
static PREVIOUS_MODULE: std::sync::OnceLock<piperine_abi::raw::Module> = std::sync::OnceLock::new();

impl piperine_abi::device::Device for Previous {
    fn prototype() -> &'static piperine_abi::raw::Shape<'static> {
        PREVIOUS_SHAPE.get_or_init(InstantCases::previous_shape).shape()
    }
    fn create(
        _: &piperine_abi::device::Values<'_>,
    ) -> Result<(Self, piperine_abi::shape::ShapeRef), piperine_abi::device::Fault> {
        Ok((Previous, std::sync::Arc::clone(PREVIOUS_SHAPE.get_or_init(InstantCases::previous_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut piperine_abi::device::Eval) -> Result<(), piperine_abi::device::Fault> {
        let previous = cx.f64(1);
        cx.set(0, previous);
        Ok(())
    }
    fn react(
        &self,
        _k: u32,
        _cx: &mut piperine_abi::device::React,
    ) -> Result<piperine_abi::device::Flow, piperine_abi::device::Fault> {
        Ok(piperine_abi::device::Flow::Continue)
    }
}

#[test]
fn previous_is_the_value_before_the_microstep() {
    // DSV-01 AC 3 (§8.2.3): x holds 1.0; 2.0 then 3.0 in one microstep settle to 3.0, and
    // PREVIOUS reads 1.0, the value before the microstep, not 2.0, the commit before the last
    let handle = piperine_abi::caller::Handle::create(
        PREVIOUS_MODULE.get_or_init(|| piperine_abi::thunk::module::<Previous>("Previous")),
        &[],
    )
    .expect("creates");
    let mut d = InstantCases::engine(vec![("pv", handle, vec![("x", "x"), ("p", "p")])]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    let f = |d: &Digital, name: &str| f64::from_le_bytes(d.read(InstantCases::net(d, name)).try_into().expect("an F64"));
    d.host_commit(InstantCases::net(&d, "x"), 0, &1.0f64.to_le_bytes(), Tick(NS)).unwrap();
    InstantCases::instant(&mut d, Tick(NS));
    assert_eq!(f(&d, "p"), 0.0, "x went from 0.0 to 1.0");
    d.drain_observations();
    let t = Tick(2 * NS);
    for v in [2.0f64, 3.0] {
        d.host_commit(InstantCases::net(&d, "x"), 0, &v.to_le_bytes(), t).unwrap();
    }
    InstantCases::instant(&mut d, t);
    assert_eq!(f(&d, "x"), 3.0, "the later commit wins");
    assert_eq!(f(&d, "p"), 1.0, "PREVIOUS is the value before the microstep");
    let seen = d.drain_observations();
    assert_eq!(InstantCases::changes(&seen, "x", t, 0), vec![3.0f64.to_le_bytes().to_vec()], "one change, to 3.0: {seen:?}");
}

/// Commits when `go` fires: statement `i` does `out <- value` one microstep later with a plain
/// `commit`. One device, so the commits are the statements of one reaction (a net has one
/// writer, P1) and C2 orders them by statement.
/// pins: go=0 (PORT event); out=1 (PORT BIT)   reaction 0: on [go] writes [S0 {out}, S1 {out}, ..]
struct Driver {
    commits: Vec<Vec<u8>>,
    shape: piperine_abi::shape::ShapeRef,
}

impl Driver {
    /// A driver of `commits` to a `BIT`, leaked so its module can borrow it.
    fn handle(commits: &[[u8; 1]]) -> piperine_abi::caller::Handle {
        Driver::on(&BIT, commits.iter().map(|value| value.to_vec()).collect())
    }

    /// A driver of `commits` to a pin of `layout`.
    fn on(layout: &'static piperine_abi::raw::Layout<'static>, commits: Vec<Vec<u8>>) -> piperine_abi::caller::Handle {
        use piperine_abi::raw::{Statement, role};
        use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf};
        let mut shape = ShapeBuf::new();
        let go = shape.pin(PinSpec::event("go", role::PORT));
        let out = shape.pin(PinSpec::discrete("out", role::PORT, layout));
        shape.reaction(ReactionSpec {
            name: "drive",
            on: vec![go],
            writes: commits.iter().map(|value| Statement { pin: out, offset: 0, len: value.len() as u32, flags: 0, min_after: 0.0 }).collect(),
            ..ReactionSpec::default()
        });
        let driver: &'static Driver = Box::leak(Box::new(Driver { commits, shape: shape.freeze() }));
        let module: &'static piperine_abi::raw::Module = Box::leak(Box::new(piperine_abi::thunk::module_with("Driver", driver)));
        piperine_abi::caller::Handle::create(module, &[]).expect("creates")
    }
}

impl piperine_abi::device::Factory for Driver {
    type Instance = Vec<Vec<u8>>;
    fn prototype(&self) -> &piperine_abi::raw::Shape<'_> {
        self.shape.shape()
    }
    fn create(&self, _: &piperine_abi::device::Values<'_>) -> Result<(Self::Instance, piperine_abi::shape::ShapeRef), piperine_abi::device::Fault> {
        Ok((self.commits.clone(), std::sync::Arc::clone(&self.shape)))
    }
    fn eval(_: &Self::Instance, _: u32, _: &mut piperine_abi::device::Eval<'_>) -> Result<(), piperine_abi::device::Fault> {
        Ok(())
    }
    fn react(commits: &Self::Instance, _: u32, cx: &mut piperine_abi::device::React<'_>) -> Result<piperine_abi::device::Flow, piperine_abi::device::Fault> {
        for (statement, value) in commits.iter().enumerate() {
            cx.commit(statement as u32, value, 0.0)?;
        }
        Ok(piperine_abi::device::Flow::Continue)
    }
}

#[test]
fn the_last_plain_commit_of_a_microstep_is_the_value() {
    // abi.md §6 C2: the commits of one microstep apply in statement order and the later one wins
    assert_eq!(InstantCases::driven(HIGH, &[LOW]).0, LOW);
    assert_eq!(InstantCases::driven(LOW, &[LOW, HIGH]).0, HIGH, "the later commit wins");
    assert_eq!(InstantCases::driven(LOW, &[HIGH, LOW]).0, LOW, "the later commit wins");
}

#[test]
fn a_commit_equal_to_the_value_fires_no_change() {
    // §8.2.3: a commit equal to the net's value is no change, so `changed()` stays false and no
    // change is observed; one that differs is one change
    let (value, seen) = InstantCases::driven(HIGH, &[HIGH]);
    assert_eq!(value, HIGH);
    assert!(InstantCases::changes(&seen, "q", Tick(2 * NS), 1).is_empty(), "no change observed: {seen:?}");
    let (value, seen) = InstantCases::driven(HIGH, &[LOW]);
    assert_eq!(value, LOW);
    assert_eq!(InstantCases::changes(&seen, "q", Tick(2 * NS), 1), vec![LOW.to_vec()], "{seen:?}");
}

#[test]
fn a_plain_commit_to_a_layout_without_a_known_byte_writes_its_bytes() {
    // abi.md §3.1: an `F64` pin holds the value only; a commit writes its eight bytes, as every
    // commit does
    let value = 1.0f64.to_le_bytes().to_vec();
    let handle = Driver::on(&piperine_abi::layout::F64, vec![value.clone()]);
    let mut d = InstantCases::engine(vec![("d", handle, vec![("go", "go"), ("out", "q")])]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    let t = Tick(NS);
    d.host_commit(InstantCases::net(&d, "go"), 0, &[], t).unwrap();
    InstantCases::instant(&mut d, t);
    assert_eq!(d.read(InstantCases::net(&d, "q")).to_vec(), value);
}

#[test]
fn a_reopened_instant_settles_at_its_last_microstep() {
    // §19 Resolution, §15.2 rule 4: the owner may run an instant without its settle phase and run the
    // phase afterwards. `reopen(at, false)` resumes at the last microstep the instant ran, so the
    // record of the settle phase is stamped `(t, 1)` as in one run, not `(t, 0)` of a fresh loop.
    let mut d = InstantCases::engine(vec![
        ("sr", SettleRecorder::handle(), vec![("log", "log")]),
        ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    d.host_commit(InstantCases::net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    d.host_commit(InstantCases::net(&d, "clk"), 0, &HIGH, Tick(2)).unwrap();
    InstantCases::instant(&mut d, Tick(1));
    d.drain_observations();
    let quiet = d.run_instant(Tick(2), &[], Horizon::uniform(Tick(10 * NS)), Mode { is_static: false, settle: false }).expect("runs");
    assert_eq!(quiet.microsteps, 2, "microsteps 0 and 1");
    d.reopen(Tick(2), false);
    d.run_instant(Tick(2), &[], Horizon::uniform(Tick(10 * NS)), Mode { is_static: false, settle: true }).expect("the settle phase");
    let seen = d.drain_observations();
    let records: Vec<&Observation> = seen.iter().filter(|o| matches!(o, Observation::Record { .. })).collect();
    assert_eq!(records, [&Observation::Record { stream: "sr.log".into(), reaction: 0, statement: 0, at: (Tick(2), 1), bytes: vec![7] }], "{seen:?}");
}

#[test]
fn a_firing_after_a_reopened_instant_is_due_at_the_next_microstep() {
    // §15.2 rule 4: a crossing only a retaken solution makes fires at the next microstep of `t`. The
    // first crossing fires at (t, 0) and its commit applies at (t, 1), so the next one fires at (t, 2).
    let mut d = InstantCases::engine(vec![("cmp", Setter::handle(), vec![("up", "cross"), ("out", "above")])]);
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    d.fire(InstantCases::net(&d, "cross"), Tick(2)).unwrap();
    let first = InstantCases::instant(&mut d, Tick(2));
    assert_eq!(first.microsteps, 2, "microsteps 0 and 1");
    d.drain_observations();
    d.reopen(Tick(2), true);
    d.fire(InstantCases::net(&d, "cross"), Tick(2)).unwrap();
    d.run_instant(Tick(2), &[], Horizon::uniform(Tick(10 * NS)), Mode { is_static: false, settle: false }).expect("the next round");
    let seen = d.drain_observations();
    assert!(seen.contains(&Observation::Fire { net: "cross".into(), at: (Tick(2), 2) }), "{seen:?}");
    assert!(!seen.iter().any(|o| matches!(o, Observation::Fire { at: (_, 0), .. })), "nothing restarts at microstep 0: {seen:?}");
}

/// A pulser on `seat` (read by the analog side) and a settler on `late`, for the passing values of
/// an instant.
struct PulseBench;

impl PulseBench {
    /// The engine, started and past its first instant; `go` fires the pulse at tick 7.
    fn engine() -> Digital {
        let plan = PlanBinding::bind_by_name_with(
            vec![("p", PulseGenerator::handle(), vec![("up", "go"), ("out", "seat")]), ("s", Settler::handle(), vec![("out", "late")])],
            Resolution::default(),
            |nets| nets.iter_mut().find(|n| n.name == "seat").unwrap().analog_reads = true,
        )
        .expect("binds");
        let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
        d.start().unwrap();
        InstantCases::instant(&mut d, Tick(0));
        d.fire(InstantCases::net(&d, "go"), Tick(7)).unwrap();
        d
    }
}

#[test]
fn a_pulse_of_a_net_the_analog_side_reads_is_reported_as_passing() {
    // docs/spec §15.2 rule 4: `seat` is high at the end of microstep 1 and low again at microstep
    // 2. The instant leaves it where it began, so the high it passed through is reported for the
    // owner to retake the step on, and the settle phase is left to follow that retake
    let mut d = PulseBench::engine();
    let out = d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), SETTLING).expect("the pulse runs");
    let seat = InstantCases::net(&d, "seat");
    assert_eq!(d.read(seat), LOW, "the pulse ends where it began");
    assert_eq!(out.microsteps, 3, "fire, high, low");
    assert_eq!(out.boundary, [(seat, LOW.to_vec())], "what the instant leaves behind");
    assert_eq!(out.passing, [(seat, HIGH.to_vec())], "the value it passed through");
    assert!(!d.holds_settle_commits(), "the settle phase has not run");
    // in an equilibrium nothing is retaken: nothing passes and the settle phase runs
    let mut d = PulseBench::engine();
    let out = d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), Mode { is_static: true, settle: true }).unwrap();
    assert!(out.passing.is_empty(), "{:?}", out.passing);
    assert!(d.holds_settle_commits(), "the settle phase ran");
}

#[test]
fn a_pulse_of_an_element_of_an_array_net_is_passing_as_its_windows() {
    // docs/spec §15.2 rule 4 on an array net: `p` pulses element 1 of `lines` (4 elements). The analog side reads the
    // net as its windows, so the value passed through is those windows with element 1 high, and an element outside
    // every window passes nothing the analog side sees.
    let run = |windows: Vec<(u64, u64)>| {
        let lines = NetSpec { analog_reads: true, analog_windows: windows, ..NetSpec::array("lines", 1, 1, 4) };
        let nets = vec![lines, NetSpec::event("go"), NetSpec::event("p.rise")];
        let (lines, go, rise) = (NetId(0), NetId(1), NetId(2));
        let pulser = piperine_digital_solver::plan::InstancePlan {
            path: "p".into(),
            handle: PulseGenerator::handle(),
            pins: vec![piperine_digital_solver::plan::Binding::Net(go), piperine_digital_solver::plan::Binding::ArrayElement { net: lines, element: 1 }, piperine_digital_solver::plan::Binding::Net(rise)],
        };
        let plan = piperine_digital_solver::plan::DigitalPlan::bind(nets, vec![pulser], Resolution::default()).expect("binds");
        let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
        d.start().unwrap();
        InstantCases::instant(&mut d, Tick(0));
        d.fire(go, Tick(7)).unwrap();
        let out = d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), SETTLING).expect("the pulse runs");
        assert_eq!(d.read_element(lines, 1), LOW, "the pulse ends where it began");
        (lines, out)
    };
    // the windows' bytes with element 1 at `value`: one byte per element of each window, in window order
    let windows_with = |windows: &[(u64, u64)], value: u8| -> Vec<u8> {
        windows.iter().flat_map(|&(first, length)| (first..first + length).map(move |element| if element == 1 { value } else { LOW[0] })).collect()
    };
    for windows in [vec![(0, 4)], vec![(1, 1)], vec![(2, 2), (0, 2)]] {
        let (lines, out) = run(windows.clone());
        assert_eq!(out.boundary, [(lines, windows_with(&windows, LOW[0]))], "what the instant leaves behind, windows {windows:?}");
        assert_eq!(out.passing, [(lines, windows_with(&windows, HIGH[0]))], "the high element 1 passed through, windows {windows:?}");
    }
    let (_, out) = run(vec![(2, 2)]);
    assert!(out.passing.is_empty(), "element 1 is in no window: {:?}", out.passing);
}

#[test]
fn a_net_that_changes_once_passes_through_nothing() {
    // docs/spec §15.2 rule 4: a change the instant keeps is read by the first step after it, and
    // only that step; the settle phase runs in the instant
    let plan = PlanBinding::bind_by_name_with(
        vec![("u", Setter::handle(), vec![("up", "go"), ("out", "seat")]), ("s", Settler::handle(), vec![("out", "late")])],
        Resolution::default(),
        |nets| nets.iter_mut().find(|n| n.name == "seat").unwrap().analog_reads = true,
    )
    .expect("binds");
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    d.fire(InstantCases::net(&d, "go"), Tick(7)).unwrap();
    let out = d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), SETTLING).unwrap();
    assert_eq!(out.boundary, [(InstantCases::net(&d, "seat"), HIGH.to_vec())]);
    assert!(out.passing.is_empty(), "{:?}", out.passing);
    assert!(d.holds_settle_commits(), "the settle phase ran");
}

/// Two pulsers whose instant a host waiter suspends, for the passing record of a whole instant.
struct SuspendedPulse;

impl SuspendedPulse {
    /// `p` pulses `seat` (read by the analog side) from `go`, `q` pulses `seat2` from `go2`, and a
    /// settler holds a commit when the settle phase runs. `q_up` is the net `q` rises from.
    fn engine(q_up: &str) -> Digital {
        let plan = PlanBinding::bind_by_name_with(
            vec![
                ("p", PulseGenerator::handle(), vec![("up", "go"), ("out", "seat")]),
                ("q", PulseGenerator::handle(), vec![("up", q_up), ("out", "seat2")]),
                ("s", Settler::handle(), vec![("out", "late")]),
            ],
            Resolution::default(),
            |nets| nets.iter_mut().find(|n| n.name == "seat").unwrap().analog_reads = true,
        )
        .expect("binds");
        let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
        d.start().unwrap();
        InstantCases::instant(&mut d, Tick(0));
        d
    }

    /// Runs one part of the instant at tick 7.
    fn part(d: &mut Digital) -> piperine_digital_solver::engine::Outcome {
        d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), SETTLING).expect("the part runs")
    }
}

#[test]
fn a_pulse_split_by_a_host_suspension_is_still_passing() {
    // docs/spec §15.2 rule 4: the host waits on `go` and `go2`. The instant stops after microstep 0
    // (go fired), the host fires `go2` there, and it stops again after microstep 1, where `seat` is
    // high; `seat` falls in the third part. The record spans the parts, so the last one reports the
    // high it passed through, and the settle phase is left to the owner
    let mut d = SuspendedPulse::engine("go2");
    let (go, go2, seat) = (InstantCases::net(&d, "go"), InstantCases::net(&d, "go2"), InstantCases::net(&d, "seat"));
    d.wait(go);
    d.wait(go2);
    d.fire(go, Tick(7)).unwrap();
    let first = SuspendedPulse::part(&mut d);
    assert_eq!((first.host_events.clone(), first.passing.clone()), (vec![go], vec![]), "suspended after go");
    d.fire(go2, Tick(7)).unwrap();
    let second = SuspendedPulse::part(&mut d);
    assert_eq!(second.host_events, [go2], "suspended after go2");
    assert_eq!(second.boundary, [(seat, HIGH.to_vec())], "seat is high at the end of microstep 1");
    let third = SuspendedPulse::part(&mut d);
    assert!(third.host_events.is_empty());
    assert_eq!(third.boundary, [(seat, LOW.to_vec())], "seat falls in the last part");
    assert_eq!(third.passing, [(seat, HIGH.to_vec())], "the high it passed through, across the suspension");
    assert!(!d.holds_settle_commits(), "the settle phase has not run in any part");
}

#[test]
fn a_part_after_the_pulse_leaves_the_settle_phase_to_the_owner() {
    // docs/spec §15.2 rule 4 ("the settle phase runs once"): `seat` rises and falls in the first
    // part, which stops after microstep 2 for the host's waiter on `q.rise`; the second part passes
    // nothing of its own. It still reports the pulse of the instant and does not run the settle
    // phase, which the owner runs once after reading it
    let mut d = SuspendedPulse::engine("p.rise");
    let (go, seat) = (InstantCases::net(&d, "go"), InstantCases::net(&d, "seat"));
    d.wait(InstantCases::net(&d, "q.rise"));
    d.fire(go, Tick(7)).unwrap();
    let first = SuspendedPulse::part(&mut d);
    assert_eq!(first.host_events, [InstantCases::net(&d, "q.rise")], "suspended after microstep 2");
    assert_eq!(first.passing, [(seat, HIGH.to_vec())], "seat rose and fell in the first part");
    let second = SuspendedPulse::part(&mut d);
    assert!(second.host_events.is_empty());
    assert_eq!(second.passing, [(seat, HIGH.to_vec())], "the instant's pulse, reported by its last part");
    assert!(!d.holds_settle_commits(), "the settle phase did not run in the instant");
}

#[test]
fn a_reopened_instant_reports_only_what_passes_after_the_reading() {
    // docs/spec §15.2 rule 4: a Pulser on `go` clocks a Dff whose `d` is `!q`; both `clk` and `q`
    // are read by the analog side. Round 1 pulses `clk` (passed through high) and toggles `q` to 1.
    // The owner reads that and reopens the instant; in round 2 `go` fires again, `clk` pulses again
    // and `q` toggles back to 0, once. Round 2 reports `clk`'s high and `q` at 0: never `q` at 1,
    // which round 1's reading already read
    let plan = PlanBinding::bind_by_name_with(
        vec![
            ("p", PulseGenerator::handle(), vec![("up", "go"), ("out", "clk")]),
            ("ff", DataFlipFlop::handle(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
            ("i", Inverter::handle(), vec![("a", "q"), ("y", "d")]),
        ],
        Resolution::default(),
        |nets| {
            for name in ["clk", "q"] {
                nets.iter_mut().find(|n| n.name == name).unwrap().analog_reads = true;
            }
        },
    )
    .expect("binds");
    let mut d = Digital::new(plan, DigitalLimits::default(), TimePointRule::default());
    d.start().unwrap();
    InstantCases::instant(&mut d, Tick(0));
    let (go, clk, q) = (InstantCases::net(&d, "go"), InstantCases::net(&d, "clk"), InstantCases::net(&d, "q"));
    d.fire(go, Tick(7)).unwrap();
    let first = d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), Mode::default()).unwrap();
    assert_eq!(d.read(q), HIGH, "q toggled to 1");
    assert_eq!(first.passing, [(clk, HIGH.to_vec()), (q, HIGH.to_vec())], "round 1: clk passed high, q at 1");
    d.reopen(Tick(7), true);
    d.fire(go, Tick(7)).unwrap();
    let second = d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), Mode::default()).unwrap();
    assert_eq!(d.read(q), LOW, "q toggled back to 0");
    assert_eq!(second.passing, [(clk, HIGH.to_vec()), (q, LOW.to_vec())], "round 2: clk passed high again, q read at 0");
    // without the second pulse, round 2's single change of q passes through nothing
    d.reopen(Tick(7), true);
    d.host_commit(q, 0, &HIGH, Tick(7)).unwrap();
    let third = d.run_instant(Tick(7), &[], Horizon::uniform(Tick(100)), Mode::default()).unwrap();
    assert!(third.passing.is_empty(), "a single change after a reading: {:?}", third.passing);
}

/// The engines the microstep tests run.
pub struct InstantCases;

impl InstantCases {
     // ticks of 1 fs

    /// The engine helper `engine`.
    fn engine(devices: Vec<Placed<'_>>) -> Digital {
        Digital::new(PlanBinding::bind_by_name(devices, Resolution::default()).expect("binds"), DigitalLimits::default(), TimePointRule::default())
    }

    /// The net helper `net`.
    fn net(d: &Digital, name: &str) -> NetId {
        PlanBinding::net_named(d.plan(), name)
    }

    /// The instant helper `instant`.
    fn instant(d: &mut Digital, at: Tick) -> piperine_digital_solver::engine::Outcome {
        d.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs")
    }

    /// The real helper `real`.
    fn real(v: f64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// A `settler` design whose `out` the analog side reads, with its `delay` set to `delay` seconds.
    fn settler_engine(delay: f64) -> Digital {
        let plan = PlanBinding::bind_by_name_with(vec![("s", Settler::handle(), vec![("out", "out")])], Resolution::default(), |nets| {
            nets.iter_mut().find(|n| n.name == "s.delay").unwrap().init = delay.to_le_bytes().to_vec();
            nets.iter_mut().find(|n| n.name == "out").unwrap().analog_reads = true;
        })
        .expect("binds");
        Digital::new(plan, DigitalLimits::default(), TimePointRule::default())
    }

    /// The nets of a settle-event design whose delay is `delay` seconds (the shape's default is 0).
    fn settle_event_engine(delay: f64, extra: Vec<Placed<'_>>) -> Digital {
        let mut devices = vec![("s", SettleEvent::handle(), vec![("en", "en"), ("out", "out")])];
        devices.extend(extra);
        let plan = PlanBinding::bind_by_name_with(devices, Resolution::default(), |nets| {
            nets.iter_mut().find(|n| n.name == "s.delay").expect("the delay net").init = delay.to_le_bytes().to_vec();
        })
        .expect("binds");
        Digital::new(plan, DigitalLimits::default(), TimePointRule::default())
    }

    /// The `Change` observations of net `name` at instant `(t, n)`.
    fn changes(seen: &[Observation], name: &str, t: Tick, n: u32) -> Vec<Vec<u8>> {
        seen.iter()
            .filter_map(|o| match o {
                Observation::Change { net, at, bytes } if net == name && *at == (t, n) => Some(bytes.clone()),
                _ => None,
            })
            .collect()
    }

    /// The previous shape helper `previous_shape`.
    fn previous_shape() -> piperine_abi::shape::ShapeRef {
        use piperine_abi::layout::F64;
        use piperine_abi::raw::{Cell, cell, part, role};
        use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf};
        let mut s = ShapeBuf::new();
        let x = s.pin(PinSpec::discrete("x", role::PORT, &F64));
        let p = s.pin(PinSpec::discrete("p", role::PORT, &F64));
        s.function(FunctionSpec {
            name: "prev",
            origin: "instant.rs",
            reads: vec![Cell { kind: cell::PIN, index: x, part: part::VALUE }, Cell { kind: cell::PIN, index: x, part: part::PREVIOUS }],
            writes: vec![Cell { kind: cell::PIN, index: p, part: part::VALUE }],
            ..FunctionSpec::default()
        });
        s.freeze()
    }

    /// The value of net `q` after one reaction made `commits` to it, in statement order, in one
    /// microstep, when `q` held `held` before; and what the run observed.
    fn driven(held: [u8; 1], commits: &[[u8; 1]]) -> (Vec<u8>, Vec<Observation>) {
        let mut d = InstantCases::engine(vec![("d", Driver::handle(commits), vec![("go", "go"), ("out", "q")])]);
        d.start().unwrap();
        InstantCases::instant(&mut d, Tick(0));
        d.host_commit(InstantCases::net(&d, "q"), 0, &held, Tick(NS)).unwrap();
        InstantCases::instant(&mut d, Tick(NS));
        d.drain_observations();
        let t = Tick(2 * NS);
        d.host_commit(InstantCases::net(&d, "go"), 0, &[], t).unwrap();
        InstantCases::instant(&mut d, t);
        (d.read(InstantCases::net(&d, "q")).to_vec(), d.drain_observations())
    }
}
