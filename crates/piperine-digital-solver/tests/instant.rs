//! The microstep loop, the start and the settle phase (feature
//! `solver-abi-digital`, SAD-11, SAD-12 and SAD-16; `docs/spec/part_v_execution.md`
//! §15 and §15.1; abi.md §6, §9): apply, functions, reactions, repeat; the bound;
//! `QUIET`; `START`; `SETTLE`; what an instant reports to the owner.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::layout::{HIGH, LOW};
use piperine_digital_solver::engine::{Digital, Horizon, Mode};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::limits::DigitalLimits;

const NS: i64 = 1_000_000; // ticks of 1 fs

fn engine(devices: Vec<Placed<'_>>) -> Digital {
    Digital::new(bind_by_name(devices, Resolution::default()).expect("binds"), DigitalLimits::default())
}

fn net(d: &Digital, name: &str) -> NetId {
    net_named(d.plan(), name)
}

fn instant(d: &mut Digital, at: Tick) -> piperine_digital_solver::engine::Outcome {
    d.run_instant(at, &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).expect("the instant runs")
}

fn real(v: f64) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&v.to_le_bytes());
    b[8] = 1;
    b
}

#[test]
fn start_commits_inits_and_fires_start() {
    // C28
    let mut d = engine(vec![
        ("clock", timer(), vec![("out", "clk")]),
        ("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    // garbage before the start is dropped by it
    d.host_commit(net(&d, "d"), 0, &HIGH, Tick(3)).unwrap();
    d.start().unwrap();
    assert_eq!(d.read(net(&d, "q")), [0, 0], "the store is zeroed until the first instant applies the inits");
    assert_eq!(d.next_due(), Some(Tick(0)), "inits and START are due at (0, 0); d's commit was dropped");
    let out = instant(&mut d, Tick(0));
    assert_eq!(d.read(net(&d, "q")), LOW, "q's init");
    assert_eq!(d.read(net(&d, "clk")), LOW, "the timer's out init");
    assert_eq!(d.read(net(&d, "clock.half")), 1e-8f64.to_le_bytes(), "the param's default");
    assert!(out.fired.contains(&net(&d, "global:7")), "START fired");
    assert_eq!(d.read(net(&d, "d")), [0, 0], "the dropped commit never applied");
    // the timer reacted to START: its edge and its tick are due at 10 ns
    assert_eq!(d.next_due(), Some(Tick(10 * NS)));
}

#[test]
fn start_at_commits_init_at_its_tick() {
    // HYG-12 AC 2: the inits and START are due at the start tick, not at 0; the timer's reaction
    // (half = 10 ns) lands at the start plus its delay.
    let start = Tick(5 * NS);
    let mut d = engine(vec![
        ("clock", timer(), vec![("out", "clk")]),
        ("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    d.start_at(start).unwrap();
    assert_eq!(d.next_due(), Some(start), "inits and START are due at the start tick");
    let out = instant(&mut d, start);
    assert_eq!(d.read(net(&d, "q")), LOW, "q's init");
    assert!(out.fired.contains(&net(&d, "global:7")), "START fired");
    assert_eq!(d.next_due(), Some(Tick(15 * NS)), "the timer commits at start plus its half period");
}

#[test]
fn a_plan_with_no_code_applies_host_commits_only() {
    let plan = piperine_digital_solver::plan::DigitalPlan::bind(
        vec![NetSpec::discrete("n", 2, 1)],
        vec![],
        Resolution::default(),
    )
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default());
    d.start().unwrap();
    assert_eq!(d.next_due(), None, "nothing to run");
    d.host_commit(NetId(0), 0, &HIGH, Tick(7)).unwrap();
    assert_eq!(d.next_due(), Some(Tick(7)));
    let out = instant(&mut d, Tick(7));
    assert_eq!(d.read(NetId(0)), HIGH);
    assert_eq!(out.microsteps, 1);
    assert_eq!(d.next_due(), None);
}

#[test]
fn the_traced_instant_of_section_15_1() {
    // C23: `Front` with vin crossing the upper threshold at 10 ns, exactly when clk rises
    let mut d = engine(vec![
        ("clock", timer(), vec![("out", "clk")]),
        ("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
        ("buf", and2(), vec![("a", "above"), ("b", "above"), ("y", "d")]),
        ("cmp", setter(), vec![("up", "cross"), ("out", "above")]),
    ]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    d.drain_observations();
    d.fire(net(&d, "cross"), Tick(10 * NS)).unwrap(); // the located crossing
    assert_eq!(d.next_due(), Some(Tick(10 * NS)));
    let out = instant(&mut d, Tick(10 * NS));

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
            &Observation::Change { net: "q".into(), at: at(1), bytes: vec![0, 0] },
        ],
        "q took d from before the edge (still unknown), above and d changed after it"
    );
    assert!(seen.iter().all(|o| !matches!(o, Observation::Change { at: a, .. } | Observation::Fire { at: a, .. } if a.1 >= 2)));
    assert_eq!(out.microsteps, 2, "(10 ns, 2) has nothing pending: the loop exits");
}

#[test]
fn an_instant_that_never_goes_quiet_is_e0501_naming_the_nets() {
    // C24: a timer with a half period of zero toggles its own trigger forever
    let plan = bind_by_name_with(vec![("t", timer(), vec![("out", "clk")])], Resolution::default(), |nets| {
        let half = nets.iter_mut().find(|n| n.name == "t.half").unwrap();
        half.init = 0.0f64.to_le_bytes().to_vec();
    })
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default());
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
    let mut d = engine(vec![
        ("c", counter(), vec![("clk", "clk"), ("n", "n")]),
        ("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")]),
    ]);
    d.host_commit(net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    d.host_commit(net(&d, "clk"), 0, &HIGH, Tick(2)).unwrap();
    let first = instant(&mut d, Tick(1));
    assert_eq!(first.microsteps, 1, "a low clock: no edge, nothing pending");
    let out = instant(&mut d, Tick(2));
    assert_eq!(d.read(net(&d, "n")), 1u32.to_le_bytes(), "the count applied as the instant exited");
    assert_eq!(out.microsteps, 2, "the flip-flop's q is a commit to a live net: microstep 1 ran, and the quiet n rode along");
    // the counter alone
    let mut d = engine(vec![("c", counter(), vec![("clk", "clk"), ("n", "n")])]);
    d.host_commit(net(&d, "clk"), 0, &LOW, Tick(1)).unwrap();
    d.host_commit(net(&d, "clk"), 0, &HIGH, Tick(2)).unwrap();
    instant(&mut d, Tick(1));
    let out = instant(&mut d, Tick(2));
    assert_eq!(d.read(net(&d, "n")), 1u32.to_le_bytes());
    assert_eq!(out.microsteps, 1, "a quiet commit starts no microstep");
}

#[test]
fn a_settle_commit_with_no_delay_is_refused() {
    // C29
    let mut d = engine(vec![("s", settler(), vec![("out", "out")])]);
    d.start().unwrap();
    match d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), Mode { is_static: false, settle: true }) {
        Err(DigitalError::Unsupported { what }) => assert!(what.contains("settle"), "{what}"),
        other => panic!("expected an unsupported error, got {other:?}"),
    }
    // no settle phase, no reaction, no error
    let mut d = engine(vec![("s", settler(), vec![("out", "out")])]);
    d.start().unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), Mode::default()).expect("SETTLE never fired");
    // a delay that advances is a commit like any other
    let plan = bind_by_name_with(vec![("s", settler(), vec![("out", "out")])], Resolution::default(), |nets| {
        nets.iter_mut().find(|n| n.name == "s.delay").unwrap().init = 1e-9f64.to_le_bytes().to_vec();
    })
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default());
    d.start().unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(10)), Mode { is_static: false, settle: true }).expect("a delayed settle commit");
    assert_eq!(d.next_due(), Some(Tick(NS)));
}

#[test]
fn an_instant_reports_boundary_fired_and_final() {
    // C33: the sampler reads its sample; `out` is read by the analog side; the stub finishes
    let plan = bind_by_name_with(
        vec![
            ("sm", sampler(), vec![("v", "node"), ("go", "g"), ("s", "out")]),
            ("o", odd(), vec![("e1", "fin")]),
        ],
        Resolution::default(),
        |nets| nets.iter_mut().find(|n| n.name == "out").unwrap().analog_reads = true,
    )
    .unwrap();
    let mut d = Digital::new(plan, DigitalLimits::default());
    d.start().unwrap();
    d.fire(net(&d, "g"), Tick(7)).unwrap();
    d.fire(net(&d, "fin"), Tick(7)).unwrap();
    let out = d.run_instant(Tick(7), &[2.5], Horizon::uniform(Tick(100)), Mode::default()).unwrap();
    assert_eq!(out.boundary, [(net(&d, "out"), real(2.5).to_vec())], "the sample went through the reaction");
    let mut want = vec![net(&d, "g"), net(&d, "fin")];
    want.sort();
    assert_eq!(out.fired, want);
    assert!(out.finish, "the Odd reaction on e1 returned FINISH");
    assert!(!out.host_param_write);
    // a host write to a PARAM net is reported
    let mut d = engine(vec![("s", settler(), vec![("out", "out")])]);
    d.host_commit(net(&d, "s.delay"), 0, &2e-9f64.to_le_bytes(), Tick(3)).unwrap();
    assert!(instant(&mut d, Tick(3)).host_param_write);
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

#[test]
fn two_commits_to_one_net_settle_to_one_write() {
    // DSV-01 AC 1 (§12.1.1, §12.1.2): the commits of a microstep apply together, the later one
    // winning each byte it writes. `d` holds [0, 0]; HIGH ([1, 1]) then a one-byte 0 at offset 0
    // settle to [0, 1], LOW: one write, one change
    let mut d = engine(vec![("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    assert_eq!(d.read(net(&d, "d")), [0, 0]);
    d.drain_observations();
    let t = Tick(5 * NS);
    d.host_commit(net(&d, "d"), 0, &HIGH, t).unwrap();
    d.host_commit(net(&d, "d"), 0, &[0], t).unwrap();
    instant(&mut d, t);
    assert_eq!(d.read(net(&d, "d")), LOW, "the later commit wins its byte, the earlier keeps the other");
    let seen = d.drain_observations();
    assert_eq!(changes(&seen, "d", t, 0), vec![LOW.to_vec()], "one change, to the settled value: {seen:?}");
}

#[test]
fn a_commit_undone_in_its_microstep_is_no_change() {
    // DSV-01 AC 2 (§8.2.3, §12.1.18): clk holds HIGH; LOW then HIGH in one microstep settle to
    // HIGH, the value it had: no change, CHANGED reads false, so the flip-flop's rising edge (it
    // reads VALUE, PREVIOUS and CHANGED) does not fire and q keeps the d sampled before
    reset_calls();
    let mut d = engine(vec![("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    d.host_commit(net(&d, "d"), 0, &LOW, Tick(NS)).unwrap();
    d.host_commit(net(&d, "clk"), 0, &LOW, Tick(NS)).unwrap();
    instant(&mut d, Tick(NS));
    d.host_commit(net(&d, "clk"), 0, &HIGH, Tick(2 * NS)).unwrap();
    instant(&mut d, Tick(2 * NS));
    assert_eq!(d.read(net(&d, "q")), LOW, "the edge at 2 ns sampled d = LOW");
    let sampled = calls("dff").1;
    assert_eq!(sampled, 1, "one rising edge so far");
    d.host_commit(net(&d, "d"), 0, &HIGH, Tick(3 * NS)).unwrap();
    instant(&mut d, Tick(3 * NS));
    d.drain_observations();

    let t = Tick(4 * NS);
    d.host_commit(net(&d, "clk"), 0, &LOW, t).unwrap();
    d.host_commit(net(&d, "clk"), 0, &HIGH, t).unwrap();
    instant(&mut d, t);
    let seen = d.drain_observations();
    assert_eq!(d.read(net(&d, "clk")), HIGH);
    assert!(changes(&seen, "clk", t, 0).is_empty(), "the settled clk is the one before: no change: {seen:?}");
    assert!(
        !seen.iter().any(|o| matches!(o, Observation::Fire { net, .. } if net == "ff.rise")),
        "no edge fired: {seen:?}"
    );
    assert_eq!(calls("dff").1, sampled, "the flip-flop did not sample again");
    assert_eq!(d.read(net(&d, "q")), LOW, "q did not take d = HIGH");
}

#[test]
fn a_settled_change_is_one_observation() {
    // DSV-01 AC 3: clk holds LOW; HIGH, LOW, HIGH in one microstep settle to HIGH: one change,
    // to HIGH, with PREVIOUS = LOW (the value before the microstep), so the rising edge fires
    // once and q samples d
    reset_calls();
    let mut d = engine(vec![("ff", dff(), vec![("clk", "clk"), ("d", "d"), ("q", "q")])]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    d.host_commit(net(&d, "d"), 0, &HIGH, Tick(NS)).unwrap();
    d.host_commit(net(&d, "clk"), 0, &LOW, Tick(NS)).unwrap();
    instant(&mut d, Tick(NS));
    d.drain_observations();

    let t = Tick(2 * NS);
    for bytes in [HIGH, LOW, HIGH] {
        d.host_commit(net(&d, "clk"), 0, &bytes, t).unwrap();
    }
    instant(&mut d, t);
    let seen = d.drain_observations();
    assert_eq!(changes(&seen, "clk", t, 0), vec![HIGH.to_vec()], "one change, to HIGH: {seen:?}");
    let rises = seen.iter().filter(|o| matches!(o, Observation::Fire { net, .. } if net == "ff.rise")).count();
    assert_eq!(rises, 1, "PREVIOUS is LOW, VALUE HIGH: one rising edge");
    assert_eq!(calls("dff").1, 1);
    assert_eq!(d.read(net(&d, "q")), HIGH, "q sampled d");
}

/// Copies the `PREVIOUS` of its input `x` to `p`.
/// pins: x=0, p=1 (PORT F64);  fn 0 "prev": reads [x.VALUE, x.PREVIOUS] writes [p]
struct Previous;
static PREVIOUS_SHAPE: std::sync::OnceLock<piperine_abi::shape::ShapeRef> = std::sync::OnceLock::new();
static PREVIOUS_MODULE: std::sync::OnceLock<piperine_abi::raw::Module> = std::sync::OnceLock::new();

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

impl piperine_abi::device::Device for Previous {
    fn prototype() -> &'static piperine_abi::raw::Shape<'static> {
        PREVIOUS_SHAPE.get_or_init(previous_shape).shape()
    }
    fn create(
        _: &piperine_abi::device::Values<'_>,
    ) -> Result<(Self, piperine_abi::shape::ShapeRef), piperine_abi::device::Fault> {
        Ok((Previous, std::sync::Arc::clone(PREVIOUS_SHAPE.get_or_init(previous_shape))))
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
    let mut d = engine(vec![("pv", handle, vec![("x", "x"), ("p", "p")])]);
    d.start().unwrap();
    instant(&mut d, Tick(0));
    let f = |d: &Digital, name: &str| f64::from_le_bytes(d.read(net(d, name)).try_into().expect("an F64"));
    d.host_commit(net(&d, "x"), 0, &1.0f64.to_le_bytes(), Tick(NS)).unwrap();
    instant(&mut d, Tick(NS));
    assert_eq!(f(&d, "p"), 0.0, "x went from 0.0 to 1.0");
    d.drain_observations();
    let t = Tick(2 * NS);
    for v in [2.0f64, 3.0] {
        d.host_commit(net(&d, "x"), 0, &v.to_le_bytes(), t).unwrap();
    }
    instant(&mut d, t);
    assert_eq!(f(&d, "x"), 3.0, "the later commit wins");
    assert_eq!(f(&d, "p"), 1.0, "PREVIOUS is the value before the microstep");
    let seen = d.drain_observations();
    assert_eq!(changes(&seen, "x", t, 0), vec![3.0f64.to_le_bytes().to_vec()], "one change, to 3.0: {seen:?}");
}
