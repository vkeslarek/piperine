//! Scope: `piperine_api::model::Module` (CLA-17) — the reflected children
//! (ports/nets/instances/params/behaviors), one analysis per family
//! (op/tran/ac/noise), staging isolation from the parent design, and
//! `compile()` returning a live, working `Session`.

use piperine_api::model::Design;
use piperine_api::SolverConfig;
use piperine_lang::parse::ast::{BehaviorKind, Direction};
use piperine_lang::{Value, ValueType};

/// Self-contained divider (own discipline + devices, no prelude):
/// `mid = 5·2k/(3k+2k) = 2.0 V` by default; staging `r_top.r = 2e3`
/// balances the divider to `2.5 V`.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

fn divider() -> Design {
    Design::load_str(DIVIDER_PHDL).expect("divider elaborates")
}

#[test]
fn navigation_reads_the_authored_declarations() {
    let design = divider();

    let resistor = design.module("Resistor").expect("Resistor present");
    let ports = resistor.ports().expect("ports resolve");
    let mut port_names: Vec<&str> = ports.iter().map(|p| p.name()).collect();
    port_names.sort();
    assert_eq!(port_names, vec!["n", "p"], "Resistor's two authored ports");
    assert!(
        ports.iter().all(|p| matches!(p.direction(), Direction::Inout) && p.ty() == "Electrical"),
        "both ports are `inout` on the Electrical discipline"
    );

    let params = resistor.params().expect("params resolve");
    assert_eq!(params.len(), 1, "Resistor declares exactly one param");
    assert_eq!(params[0].name(), "r");
    assert!(matches!(params[0].ty(), ValueType::Real), "`r` is a Real param");
    assert_eq!(params[0].default(), Some(&Value::Real(1e3)), "the authored default folds to 1e3");

    let behaviors = resistor.behaviors().expect("behaviors resolve");
    assert_eq!(behaviors.len(), 1, "Resistor has one behavior block");
    assert!(matches!(behaviors[0].kind(), BehaviorKind::Analog), "the block is `analog`");

    let divider_mod = design.module("Divider").expect("Divider present");
    let nets = divider_mod.nets().expect("nets resolve");
    let mut net_names: Vec<&str> = nets.iter().map(|n| n.name()).collect();
    net_names.sort();
    assert_eq!(net_names, vec!["gnd", "mid", "vin"], "Divider's three authored wires");

    let instances = divider_mod.instances().expect("instances resolve");
    let mut pairs: Vec<(&str, &str)> = instances.iter().map(|i| (i.name(), i.module())).collect();
    pairs.sort();
    assert_eq!(
        pairs,
        vec![("r_bot", "Resistor"), ("r_top", "Resistor"), ("src", "VoltageSource")],
        "Divider's three authored instances, each naming its submodule"
    );
}

#[test]
fn op_solves_the_operating_point() {
    let design = divider();
    let module = design.module("Divider").expect("Divider present");
    let op = module.op(None, None).expect("op solves");
    let mid = op.v("mid").expect("v(mid)");
    assert!((mid - 2.0).abs() < 1e-9, "5 V over 3k/2k reads 2.0 V at mid, got {mid}");
}

#[test]
fn tran_records_a_waveform() {
    let design = divider();
    let module = design.module("Divider").expect("Divider present");
    let trace = module.tran(5e-3, Some(1e-5), 0.0, None, None, false, &[]).expect("tran runs");
    let mid = trace.v("mid").expect("v(mid) recorded");
    assert!(!mid.is_empty(), "the transient recorded points");
    let last = mid.points().last().expect("at least one point").1;
    assert!((last - 2.0).abs() < 1e-6, "a resistive divider sits at its op value, got {last}");
}

#[test]
fn ac_sweeps_the_requested_points() {
    let design = divider();
    let module = design.module("Divider").expect("Divider present");
    let ac = module.ac(1.0, 1e6, 10, true, None).expect("ac runs");
    assert_eq!(ac.axis().len(), 10, "one sweep point per requested step");
}

#[test]
fn noise_returns_a_psd_over_the_sweep() {
    let design = divider();
    let module = design.module("Divider").expect("Divider present");
    let noise = module.noise("mid", "gnd", (1.0, 1e6), 5, true, None).expect("noise runs");
    assert_eq!(noise.psd().len(), 5, "one PSD sample per requested step");
}

#[test]
fn staged_overrides_apply_without_mutating_the_parent_design() {
    let design = divider();
    let module = design.module("Divider").expect("Divider present");

    module.set("r_top", "r", 2e3);
    let staged = module.op(None, None).expect("staged op solves");
    let mid = staged.v("mid").expect("v(mid)");
    assert!((mid - 2.5).abs() < 1e-9, "r_top = 2k balances the divider to 2.5 V, got {mid}");

    // A fresh view over the SAME shared design sees the authored value: the
    // staged override lived in the module's isolated map and never touched
    // the parent design.
    let fresh = design.module("Divider").expect("Divider still present");
    let unstaged = fresh.op(None, None).expect("unstaged op solves");
    let mid = unstaged.v("mid").expect("v(mid)");
    assert!((mid - 2.0).abs() < 1e-9, "the parent design still solves to 2.0 V, got {mid}");
}

#[test]
fn compile_returns_a_live_session_that_restamps_without_recompiling() {
    let design = divider();
    let module = design.module("Divider").expect("Divider present");
    let mut session = module.compile().expect("compile returns a session");

    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let mid = op.v("mid").expect("v(mid)");
    assert!((mid - 2.0).abs() < 1e-9, "the compiled session solves the authored divider, got {mid}");

    session.set("r_top", "r", 2e3).expect("live set restamps");
    let op = session.op(&SolverConfig::default(), None).expect("op re-solves");
    let mid = op.v("mid").expect("v(mid)");
    assert!((mid - 2.5).abs() < 1e-9, "the live set took effect, got {mid}");
    assert_eq!(session.rebuilds(), 0, "a live set never recompiles (MD-18)");
}
