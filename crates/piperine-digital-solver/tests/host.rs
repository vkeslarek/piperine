//! Host writes, the parameter cone and the shadow
//! (feature `solver-abi-digital`, SAD-18 and SAD-19; abi.md §3.1 P5
//! and P6, §6 C6): a host write is a commit at microstep 0, a `STRUCTURAL`
//! write mid-run is refused, a param write is checked on a copy of the store
//! first.

#[path = "support/devices.rs"]
#[allow(dead_code)] // each test target uses the devices it needs
mod devices;

use devices::*;
use piperine_abi::layout::{HIGH, LOW};
use piperine_digital_solver::advance::Advance;
use piperine_digital_solver::engine::{Digital, Horizon, Mode, Shadow};
use piperine_digital_solver::net::{NetId, NetSpec};
use piperine_digital_solver::observe::Observation;
use piperine_digital_solver::tick::{Resolution, Tick};
use piperine_digital_solver::error::DigitalError;
use piperine_digital_solver::limits::DigitalLimits;

fn engine_with(devices: Vec<Placed<'_>>, adjust: impl FnOnce(&mut Vec<NetSpec>)) -> Digital {
    Digital::new(bind_by_name_with(devices, Resolution::default(), adjust).expect("binds"), DigitalLimits::default())
}

fn net(d: &Digital, name: &str) -> NetId {
    net_named(d.plan(), name)
}

fn started(mut d: Digital) -> Digital {
    d.start().unwrap();
    d.run_instant(Tick(0), &[], Horizon::uniform(Tick(i64::MAX)), Mode::default()).unwrap();
    d
}

fn f64_bytes(v: f64) -> [u8; 8] {
    v.to_le_bytes()
}

#[test]
fn a_host_write_is_a_commit_at_microstep_zero() {
    // C39
    let mut d = started(engine_with(vec![("inv", inverter(), vec![("a", "a"), ("y", "y")])], |_| {}));
    d.host_commit(net(&d, "a"), 0, &HIGH, Tick(5)).unwrap();
    assert_eq!(d.read(net(&d, "a")), [0, 0], "queued, not applied");
    assert_eq!(d.next_due(), Some(Tick(5)));
    d.run_instant(Tick(5), &[], Horizon::uniform(Tick(10)), Mode::default()).unwrap();
    assert_eq!(d.read(net(&d, "a")), HIGH);
    assert_eq!(d.read(net(&d, "y")), LOW, "and its readers ran in the same microstep");
    let changes: Vec<_> = d
        .observations()
        .iter()
        .filter_map(|o| match o {
            Observation::Change { net, at, .. } if at.0 == Tick(5) => Some((net.as_str(), at.1)),
            _ => None,
        })
        .collect();
    assert_eq!(changes, [("a", 0), ("y", 0)], "both at (5, 0)");
}

#[test]
fn a_structural_param_write_during_a_run_is_refused() {
    // C40
    let structural = |nets: &mut Vec<NetSpec>| nets.iter_mut().find(|n| n.name == "c.r").unwrap().structural = true;
    let mut d = engine_with(vec![("c", checked(), vec![])], structural);
    // between runs the write is a plain commit
    d.host_commit(net(&d, "c.r"), 0, &f64_bytes(2.0), Tick(0)).expect("no run in progress");
    d.start().unwrap();
    let before = d.checkpoint();
    match d.host_commit(net(&d, "c.r"), 0, &f64_bytes(3.0), Tick(4)) {
        Err(DigitalError::HostWrite { net, .. }) => assert_eq!(net, "c.r"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(d.checkpoint(), before, "nothing changed");
    // a net that is not structural still takes the write
    d.host_commit(net(&d, "c.ok"), 0, &HIGH, Tick(4)).expect("not structural");
}

#[test]
fn the_cone_is_the_functions_fed_by_params_and_globals_only() {
    let d = engine_with(
        vec![
            ("c", checked(), vec![]),                                           // reads a PARAM: in
            ("chain", inverter(), vec![("a", "c.ok"), ("y", "chained")]),       // through a cone function: in
            ("port", inverter(), vec![("a", "driven"), ("y", "p")]),            // a net the host drives as a port: out
            ("set", setter(), vec![("up", "up"), ("out", "stmt")]),
            ("stmt", inverter(), vec![("a", "stmt"), ("y", "s")]),              // fed by a reaction's statement: out
            ("after", inverter(), vec![("a", "p"), ("y", "downstream")]),       // fed by an outside function: out
        ],
        |_| {},
    );
    let plan = d.plan();
    let in_cone = |path: &str| {
        let i = plan.instances.iter().position(|i| i.path == path).unwrap() as u32;
        plan.in_cone(i, 0)
    };
    assert!(in_cone("c"));
    assert!(in_cone("chain"), "fed through a function of the cone");
    assert!(!in_cone("port"), "a port bound to a net something may write");
    assert!(!in_cone("stmt"), "an input reachable from a reaction's statement");
    assert!(!in_cone("after"), "an input from a function outside the cone");
}

#[test]
fn a_false_check_refuses_the_write_and_changes_nothing() {
    // C41
    let d = started(engine_with(vec![("c", checked(), vec![])], |_| {}));
    let r = net(&d, "c.r");
    assert_eq!(d.read(r), f64_bytes(1.0), "the default");
    assert_eq!(d.read(net(&d, "c.ok")), HIGH);
    let before = d.checkpoint();
    assert_eq!(d.shadow(r, 0, &f64_bytes(-1.0)).unwrap(), Shadow::Refused { check: "c.ok".into() });
    assert_eq!(d.checkpoint(), before, "the shadow changed nothing");
    assert_eq!(d.read(r), f64_bytes(1.0));
    assert_eq!(d.shadow(r, 0, &f64_bytes(2.0)).unwrap(), Shadow::Accept, "a good value passes");
    // a net outside the cone is an ordinary write
    assert_eq!(d.shadow(net(&d, "c.ok"), 0, &LOW).unwrap(), Shadow::Accept);
}

#[test]
fn reshape_on_the_shadow_names_the_instance_and_changes_nothing() {
    // C42
    let d = started(engine_with(vec![("c", checked(), vec![])], |_| {}));
    let before = d.checkpoint();
    assert_eq!(d.shadow(net(&d, "c.r"), 0, &f64_bytes(500.0)).unwrap(), Shadow::Reshape { instance: "c".into() });
    assert_eq!(d.checkpoint(), before);
    assert_eq!(d.read(net(&d, "c.r")), f64_bytes(1.0));
}

#[test]
fn a_waited_event_suspends_its_instant_until_the_host_answers() {
    // abi.md §5 R7: a host waiter is an IRREVERSIBLE reaction on its event net.
    let mut d = started(engine_with(
        vec![
            ("set", setter(), vec![("up", "up"), ("out", "out")]),
            ("inv", inverter(), vec![("a", "a"), ("y", "y")]),
        ],
        |_| {},
    ));
    let up = net(&d, "up");
    d.wait(up);
    d.fire(up, Tick(5)).unwrap();
    // The look-ahead stops before the instant, untouched: a landing that is not a time point.
    assert_eq!(d.advance(Tick(10)).unwrap(), Advance::Landing { tick: Tick(5), time_point: false });
    assert_eq!(d.read(net(&d, "out")), [0, 0]);

    let limit = Horizon::uniform(Tick(10));
    let first = d.run_instant(Tick(5), &[], limit, Mode::default()).unwrap();
    assert_eq!(first.host_events, vec![up], "suspended after the microstep `up` fired in");
    assert_eq!(d.read(net(&d, "out")), [0, 0], "the setter's commit waits for the next microstep");
    // The host answers: its write is due at the microstep the instant continues at.
    d.host_commit(net(&d, "a"), 0, &HIGH, Tick(5)).unwrap();
    let rest = d.run_instant(Tick(5), &[], limit, Mode::default()).unwrap();
    assert!(rest.host_events.is_empty());
    assert_eq!(d.read(net(&d, "out")), HIGH);
    assert_eq!(d.read(net(&d, "a")), HIGH);
    assert_eq!(d.read(net(&d, "y")), LOW, "the host's write ran its readers");
    let at_a: Vec<_> = d
        .observations()
        .iter()
        .filter_map(|o| match o {
            Observation::Change { net, at, .. } if net.as_str() == "a" => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(at_a, vec![(Tick(5), 1)], "applied at the next microstep of the instant");
    assert_eq!(d.next_due(), None, "nothing left over");
}
