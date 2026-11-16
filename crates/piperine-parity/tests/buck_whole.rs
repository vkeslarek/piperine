//! The whole buck converter runs from its MSL source (feature `msl-digital`, T56; checks C145,
//! C149 and C150, DIG-15 AC 3, 6 and 7): `Buck` of `examples/buck` is the top of a project whose
//! root crate is the buck package itself, elaborated, transpiled with no `rustc` warning, every
//! body's shape valid (abi.md §3), and run on the solver for 10 switching periods with `en` not
//! driven. Its in-ports are nets of the top that no one writes, so `en` stays unknown, which the
//! supervisor reads as not enabled (`examples/buck/SPEC.md` §8: `Off` until enabled). Expected
//! values are written from SPEC §2, §5.3, §8 and §10 S10.
#![cfg(unix)]

use std::path::Path;

use piperine_parity::project::Project;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::{Built, Seen, rise};
use support::{HIGH, LOW, build, series};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;
/// 10 switching periods of 2 us (SPEC §2: 500 kHz).
const STOP: f64 = 20e-6;

/// The buck package as the root crate.
fn project() -> Project {
    Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck/src/lib.msl"))
}

/// `Buck` compiled and run for 10 periods, `en` not driven.
fn idle() -> (Built, Seen) {
    let built = build(&project(), "Buck");
    let seen = built.run(STOP, 1e-8);
    (built, seen)
}

#[test]
fn the_buck_compiles_with_no_warning_and_valid_shapes() {
    // DIG-15 AC 3: `Built::new` compiles the generated source and asserts `rustc` printed no
    // warning; every body that runs has a shape `piperine_abi::validate` accepts
    let built = build(&project(), "Buck");
    assert_eq!(built.ir.check(), vec![], "the IR passes check()");
    let mut shapes = 0;
    for body in built.ir.bodies() {
        let runs = body.analog().is_some() || body.nodes().iter().any(|n| n.init.is_some()) || !body.carriers().is_empty();
        if !runs {
            continue;
        }
        let plan = piperine_plan::BodyPlan::new(body);
        assert_eq!(piperine_abi::validate::validate(plan.shape().shape()), Ok(()), "the shape of `{}`", body.name());
        shapes += 1;
    }
    // every body of the buck runs: the power stage, the sensing, the controller and the top
    let names: Vec<&str> = built.ir.bodies().map(|b| b.name()).collect();
    for name in ["Buck", "Controller", "Supervisor", "Dpwm", "Pid", "RefRamp", "SetpointReg", "Osc", "PowerStage", "PowerSwitch", "GateDriver", "Inductor", "Capacitor", "CurrentSense", "Supply", "Load", "Sensing", "Adc", "A2D", "Divider", "Resistor"] {
        assert!(names.contains(&name), "a body `{name}` in {names:?}");
    }
    assert!(shapes >= 20, "{shapes} shapes validated");
    // and the circuit binds on the solver
    built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_buck_that_is_not_enabled_stays_off_with_vin_ok_high() {
    let (_, seen) = idle();
    // DIG-15 AC 6, SPEC §8: with `en` unknown the supervisor is `Off` (tag 0, its initial value)
    // at every sample, power-good stays low, and nothing commits another value
    let mode = series(&seen, "Buck.mode");
    assert_eq!(mode, vec![(0, vec![0, 1])], "mode is Off, known, from the first instant and never changes");
    let pgood = series(&seen, "Buck.pgood");
    assert_eq!(pgood, vec![(0, LOW.to_vec())], "pgood is false from the first instant and never changes");
    // the DPWM does not switch while `run` is low (SPEC §8: `Off` holds both switches off), so
    // both gates are false; each is a drive delayed by `tpd` = 10 ns (SPEC §5.3), unknown until
    // its first commit is due (part I §5.1.1), false from then to the end
    let tpd = 10 * NS;
    for gate in ["Buck.power.hs_gate", "Buck.power.ls_gate"] {
        let s = series(&seen, gate);
        assert_eq!(s, vec![(tpd, LOW.to_vec())], "{gate}: false from tpd, never true");
    }
    // SPEC §8.1: Vin (12 V through 20 mohm, divided by 10: about 1.2 V) is above the UVLO
    // threshold 0.85 V at the operating point, so the `init` half of `above` (std `A2D` inside
    // `Sensing`, inside `Buck`) makes `vin_ok` true in the first instant, before time advances,
    // and it stays true
    let vin_ok = series(&seen, "Buck.vin_ok");
    assert_eq!(vin_ok.last(), Some(&(0, HIGH.to_vec())), "vin_ok is true at time 0 and never changes after: {vin_ok:?}");
    // the clock ran the whole time: 10 periods of 400 counts at 200 MHz
    let clk = series(&seen, "Buck.clk");
    assert!(clk.iter().any(|(t, _)| *t == rise(4000)), "the clock's 4000th rising edge is at {} ticks", rise(4000));
}

#[test]
fn no_never_record_fails_while_not_enabled() {
    let (built, seen) = idle();
    // SPEC §5.3 and §10 S10: the `Never` monitor of the DPWM checks at each rising clock edge
    // that both commands are never on together: one record of `checks` per edge, `pass` known
    // and true while neither switches
    // `Never` has no result: it is a child instance of the DPWM, `no_shoot_through`
    let never_at = "Buck.ctl.pwm.no_shoot_through";
    let never_body = built.ir.instance_at(never_at).expect("the monitor instance").body();
    let all = seen.all();
    let transient = all.iter().filter(|o| o.analysis == AnalysisKind::Transient);
    let sites: Vec<Vec<(String, String)>> = transient
        .clone()
        .filter(|o| o.path == never_at)
        .filter_map(|o| match &o.kind {
            ObservationKind::Commit { reaction, statement, .. } => never_body.record_site(*reaction, *statement),
            _ => None,
        })
        .map(|site| site.texts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
        .collect();
    let records: Vec<(i64, Vec<u8>)> = transient
        // its record pin: the instance path and the stream
        .filter(|o| o.path == format!("{never_at}.checks"))
        .filter_map(|o| match &o.kind {
            ObservationKind::Record { bytes } => Some(((o.time * 1e15).round() as i64, bytes.clone())),
            _ => None,
        })
        .collect();
    // 20 us at 5 ns: rising edges 1 to 4000
    let edges: Vec<i64> = (1..=4000).map(rise).collect();
    assert_eq!(records.iter().map(|(t, _)| *t).collect::<Vec<_>>(), edges, "one record per rising edge");
    assert_eq!(sites.len(), records.len(), "each record has its commit");
    let never = vec![("kind".to_string(), "require".to_string()), ("name".to_string(), "never".to_string())];
    assert!(sites.iter().all(|t| *t == never), "every record is the `never` check");
    let failed: Vec<&(i64, Vec<u8>)> = records.iter().filter(|(_, b)| b[0] != 1 || b[1] != 1).collect();
    assert!(failed.is_empty(), "no record has `pass` false or unknown: {failed:?}");
}
