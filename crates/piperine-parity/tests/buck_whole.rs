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

use piperine_ir_fixtures::digital;
use piperine_parity::mixed;
use piperine_ir_fixtures::mixed::TopDesign;

use mixed::{BuiltDesign, Seen};
use piperine_parity::buck_fixture::{NetChanges, HIGH, LOW};

/// 10 switching periods of 2 us (SPEC §2: 500 kHz).
const STOP: f64 = 20e-6;

/// The whole buck package as the root crate, built and run with `en` not driven.
struct BuckWholeBench;

impl BuckWholeBench {
    /// The buck package as the root crate.
    fn project() -> Project {
        Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck/src/lib.msl"))
    }

    /// `Buck` compiled and run for 10 periods, `en` not driven.
    fn idle() -> (BuiltDesign, Seen) {
        let built = BuiltDesign::of(&BuckWholeBench::project(), "Buck");
        let seen = built.run(STOP, 1e-8);
        (built, seen)
    }
}

#[test]
fn the_buck_compiles_with_no_warning_and_valid_shapes() {
    // DIG-15 AC 3: `BuiltDesign::new` compiles the generated source and asserts `rustc` printed no
    // warning; every body that runs has a shape `piperine_abi::validate` accepts
    let built = BuiltDesign::of(&BuckWholeBench::project(), "Buck");
    assert_eq!(built.ir.check(), vec![], "the IR passes check()");
    let mut shapes = 0;
    for body in built.ir.bodies() {
        let runs = body.analog().is_some() || body.nodes().iter().any(|n| n.init.is_some()) || !body.carriers().is_empty();
        if !runs {
            continue;
        }
        let plan = piperine_plan::BodyPlan::build(body).expect("the body plans");
        assert_eq!(piperine_abi::validate::Validator::validate(plan.shape().shape()), Ok(()), "the shape of `{}`", body.name());
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
    let (_, seen) = BuckWholeBench::idle();
    // DIG-15 AC 6, SPEC §8: with `en` unknown the supervisor is `Off` (tag 0, its initial value)
    // at every sample, power-good stays low, and nothing commits another value
    let mode = NetChanges::series(&seen, "Buck.mode");
    assert_eq!(mode, vec![(0, vec![0])], "mode is Off from the first instant and never changes");
    let pgood = NetChanges::series(&seen, "Buck.pgood");
    assert_eq!(pgood, vec![(0, LOW.to_vec())], "pgood is false from the first instant and never changes");
    // the DPWM does not switch while `run` is low (SPEC §8: `Off` holds both switches off), so
    // both gates are false; each is a drive delayed by `tpd` = 10 ns (SPEC §5.3). A carrier has
    // its initial value, false, from (0, 0) on (part II §5.1.1, §5.3.8), and the commit of
    // false that comes due at `tpd` writes the value it already holds: "a commit of the value
    // the carrier already holds is not a change" (part II §5.3.5). So the one entry is the
    // initial value at 0, false to the end
    for gate in ["Buck.power.hs_gate", "Buck.power.ls_gate"] {
        let s = NetChanges::series(&seen, gate);
        assert_eq!(s, vec![(0, LOW.to_vec())], "{gate}: false from the first instant, never true");
    }
    // SPEC §8.1: Vin (12 V through 20 mohm, divided by 10: about 1.2 V) is above the UVLO
    // threshold 0.85 V at the operating point, so the `init` half of `above` (std `A2D` inside
    // `Sensing`, inside `Buck`) makes `vin_ok` true in the first instant, before time advances,
    // and it stays true
    let vin_ok = NetChanges::series(&seen, "Buck.vin_ok");
    assert_eq!(vin_ok.last(), Some(&(0, HIGH.to_vec())), "vin_ok is true at time 0 and never changes after: {vin_ok:?}");
    // the clock ran the whole time: 10 periods of 400 counts at 200 MHz
    let clk = NetChanges::series(&seen, "Buck.clk");
    assert!(clk.iter().any(|(t, _)| *t == TopDesign::rising_edge(4000)), "the clock's 4000th rising edge is at {} ticks", TopDesign::rising_edge(4000));
}

#[test]
fn no_never_record_fails_while_not_enabled() {
    let (built, seen) = BuckWholeBench::idle();
    // SPEC §5.3 and §10 S10: the `Never` monitor of the DPWM checks at each rising clock edge
    // that both commands are never on together: one record of `checks` per edge, `pass` true
    // while neither switches. `pass` is a `bool`, one byte (part VII §19 rule 3: a pass or a
    // fail, no third verdict)
    // `Never` has no result: it is a child instance of the DPWM, `no_shoot_through`
    let never_at = "Buck.ctl.pwm.no_shoot_through";
    let all = seen.all();
    // each record names its own site (abi.md C5), decoded with `Ir::record_at`
    let decoded: Vec<(i64, Vec<u8>, Vec<(String, String)>)> = all
        .iter()
        .filter(|o| o.analysis == AnalysisKind::Transient)
        // its record pin: the instance path and the stream
        .filter(|o| o.path == format!("{never_at}.checks"))
        .filter_map(|o| match &o.kind {
            ObservationKind::Record { site, bytes } => {
                let (reaction, statement) = site.expect("a record of an emit names its site");
                let (instance, site) = built.ir.record_at(&o.path, reaction, statement).expect("a record decodes to its emit");
                assert_eq!(instance.path(), never_at);
                let texts = site.texts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                Some(((o.time * 1e15).round() as i64, bytes.clone(), texts))
            }
            _ => None,
        })
        .collect();
    let records: Vec<(i64, Vec<u8>)> = decoded.iter().map(|(t, b, _)| (*t, b.clone())).collect();
    let sites: Vec<Vec<(String, String)>> = decoded.into_iter().map(|(_, _, s)| s).collect();
    // 20 us at 5 ns: rising edges 1 to 4000
    let edges: Vec<i64> = (1..=4000).map(rise).collect();
    assert_eq!(records.iter().map(|(t, _)| *t).collect::<Vec<_>>(), edges, "one record per rising edge");
    assert_eq!(sites.len(), records.len(), "each record decodes to its site");
    let never = vec![("kind".to_string(), "require".to_string()), ("name".to_string(), "never".to_string())];
    assert!(sites.iter().all(|t| *t == never), "every record is the `never` check");
    let failed: Vec<&(i64, Vec<u8>)> = records.iter().filter(|(_, b)| b[0] != 1).collect();
    assert!(failed.is_empty(), "no record has `pass` false: {failed:?}");
}
