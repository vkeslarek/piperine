//! Scope: the lifted object model driven end-to-end from the **Rust** host
//! (CLA-19): `load` → module navigation → analysis → instance view → opvar →
//! `compile` → live `set`, all through the root `piperine` crate with no
//! Python in the loop. The twin of the Python binding's object-model tests —
//! same model, other host (MD-22).

use std::rc::Rc;

use piperine::SolverConfig;
use piperine::model::{Design, InstanceReadout, InstanceResolver, InstanceView};

/// The divider fixture: `r_bot` computes opvar `cond = 1/r` (named via
/// `@name`); `r_top` is a plain resistor. `mid = 5·2k/(3k+2k) = 2.0 V` by
/// default; staging `r_top.r = 2e3` balances the divider to 2.5 V.
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

mod OpvarResistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
    @name(value = \"cond\") var g : Real = 0.0;
}
analog OpvarResistor {
    g = 1.0 / r;
    I(p, n) <+ g * V(p, n);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : OpvarResistor (.p = mid, .n = gnd) { .r = 2e3 };
}
";

#[test]
fn rust_host_drives_the_full_lifted_model_path() {
    // load → module navigation.
    let design = Design::load_str(DIVIDER_PHDL).expect("divider elaborates");
    let top = design.top().expect("the unique root is inferred");
    assert_eq!(top.name().expect("top resolves"), "Top");
    let module = design.module("Top").expect("Top present");
    assert_eq!(module.instances().expect("instances").len(), 3, "src/r_top/r_bot");

    // analysis through the model.
    let op = Rc::new(module.op(None, None).expect("op solves"));
    let mid = op.v("mid").expect("v(mid)");
    assert!((mid - 2.0).abs() < 1e-9, "default divider reads 2.0 V, got {mid}");

    // instance view → opvar + terminal readout.
    let resolver = InstanceResolver::new(design.shared(), "Top".to_string());
    let view = InstanceView::new_op(op, resolver, "r_bot").expect("r_bot binds");
    let cond = view.opvar("cond").expect("r_bot declares cond");
    assert!((cond - 1.0 / 2e3).abs() < 1e-12, "cond = 1/r_bot, got {cond}");
    let vp = match view.v("p", None).expect("v(p)") {
        InstanceReadout::Scalar(v) => v,
        other => panic!("expected a scalar over an op result, got {other:?}"),
    };
    assert!((vp - 2.0).abs() < 1e-9, "r_bot.p sits on mid = 2.0 V, got {vp}");

    // compile → live set, no recompile (MD-18).
    let mut session = module.compile().expect("compile returns a session");
    session.set("r_top", "r", 2e3).expect("live set restamps");
    let op = session.op(&SolverConfig::default(), None).expect("op re-solves");
    let mid = op.v("mid").expect("v(mid)");
    assert!((mid - 2.5).abs() < 1e-9, "the live set balanced the divider, got {mid}");
    assert_eq!(session.rebuilds(), 0, "a live set never recompiles");
}

#[test]
fn staged_overrides_stay_isolated_from_the_parent_design() {
    let design = Design::load_str(DIVIDER_PHDL).expect("divider elaborates");
    let module = design.module("Top").expect("Top present");

    module.set("r_top", "r", 2e3);
    let staged = module.op(None, None).expect("staged op solves");
    let mid = staged.v("mid").expect("v(mid)");
    assert!((mid - 2.5).abs() < 1e-9, "staged r_top = 2k reads 2.5 V, got {mid}");

    let fresh = design.module("Top").expect("Top still present");
    let unstaged = fresh.op(None, None).expect("unstaged op solves");
    let mid = unstaged.v("mid").expect("v(mid)");
    assert!((mid - 2.0).abs() < 1e-9, "the parent design still reads 2.0 V, got {mid}");
}

#[test]
fn select_and_navigation_round_trip_through_the_root_crate() {
    let design = Design::load_str(DIVIDER_PHDL).expect("divider elaborates");

    let selection = design.select("/r_top").expect("the selector resolves");
    assert_eq!(selection.len(), 1);
    assert_eq!(selection.nodes()[0].kind(), "instance");
    assert_eq!(selection.nodes()[0].name(), "r_top");

    let mut names: Vec<String> =
        design.modules().iter().map(|m| m.name().expect("resolves").to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["OpvarResistor", "Resistor", "Top", "VoltageSource"],
        "every authored module, named as written"
    );
}
