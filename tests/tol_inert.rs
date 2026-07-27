//! dv-core T5 (DVC-03) — `tol` is inert at solve. A design whose params
//! carry `tol` clauses produces **bit-identical** `op()`/`tran()`/`ac()`
//! results to the same design without them; a staged override wins for the
//! nominal value while the distribution stays attached on the POM; and the
//! `Invalidation` class a live param write returns is unchanged by the
//! presence of a `tol`.

use piperine::{NetRef, Session, SolverConfig};
use piperine_lang::{SourceMap, Value};

/// The divider twin, with or without `tol` clauses on its params:
/// `mid = 5·2k/(3k+2k) = 2.0 V` nominal; staging `r_top.r = 2e3` gives 2.5 V.
fn divider_phdl(with_tol: bool) -> String {
    let tol_v = if with_tol { " tol gauss(sigma = 0.001) global" } else { "" };
    let tol_r = if with_tol { " tol uniform(rel = 0.01)" } else { "" };
    format!(
        "\
mod VoltageSource(inout p: Electrical, inout n: Electrical) {{
    param voltage: Real = 0.0{tol_v};
}}
analog VoltageSource {{ V(p, n) <- voltage; }}

mod Resistor(inout p: Electrical, inout n: Electrical) {{
    param r: Real = 1e3{tol_r};
}}
analog Resistor {{ I(p, n) <+ V(p, n) / r; }}

mod Divider() {{
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) {{ .voltage = 5.0 }};
    r_top : Resistor      (.p = vin, .n = mid) {{ .r = 3e3 }};
    r_bot : Resistor      (.p = mid, .n = gnd) {{ .r = 2e3 }};
}}
"
    )
}

/// The RC twin, with or without a `tol` clause — the `tran`/`ac` vehicle.
fn rc_phdl(with_tol: bool) -> String {
    let tol_r = if with_tol { " tol uniform(rel = 0.01)" } else { "" };
    format!(
        "\
mod V(inout p: Electrical, inout n: Electrical) {{ param dc: Real = 0.0; param acmag: Real = 0.0; }}
analog V {{ V(p, n) <+ dc + ac_stim(acmag, 0.0); }}

mod R(inout p: Electrical, inout n: Electrical) {{ param r: Real = 1e3{tol_r}; }}
analog R {{ I(p, n) <+ V(p, n) / r; }}

mod C(inout p: Electrical, inout n: Electrical) {{ param c: Real = 1e-6; }}
analog C {{ I(p, n) <+ c * ddt(V(p, n)); }}

mod Top() {{
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : V(.p = vin, .n = gnd) {{ .dc = 5.0, .acmag = 1.0 }};
    r1 : R(.p = vin, .n = out) {{}};
    c1 : C(.p = out, .n = gnd) {{}};
}}
"
    )
}

fn elaborate(src: &str) -> piperine_lang::Design {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    piperine_lang::parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &map)
        .expect("elaborates")
}

/// DVC-03 AC4: `op()` on the toleranced design is bit-identical to the
/// untoleranced twin.
#[test]
fn op_is_bit_identical_with_and_without_tol() {
    let plain = elaborate(&divider_phdl(false));
    let tol = elaborate(&divider_phdl(true));
    let mut plain_session = Session::compile(&plain, "Divider").expect("compiles");
    let mut tol_session = Session::compile(&tol, "Divider").expect("compiles");
    let plain_op = plain_session.op(&SolverConfig::default(), None).expect("op solves");
    let tol_op = tol_session.op(&SolverConfig::default(), None).expect("op solves");

    let mid = NetRef { name: "mid".into() };
    let vin = NetRef { name: "vin".into() };
    assert_eq!(
        plain_op.v(&mid).expect("v(mid)").to_bits(),
        tol_op.v(&mid).expect("v(mid)").to_bits(),
        "tol must not move v(mid)"
    );
    assert_eq!(
        plain_op.v(&vin).expect("v(vin)").to_bits(),
        tol_op.v(&vin).expect("v(vin)").to_bits(),
        "tol must not move v(vin)"
    );
    assert_eq!(
        plain_op.i((&vin, &mid)).expect("i(vin,mid)").to_bits(),
        tol_op.i((&vin, &mid)).expect("i(vin,mid)").to_bits(),
        "tol must not move i(vin,mid)"
    );
}

/// DVC-03 AC4: `tran()` on the toleranced design is bit-identical, sample
/// for sample, to the untoleranced twin.
#[test]
fn tran_is_bit_identical_with_and_without_tol() {
    let plain = elaborate(&rc_phdl(false));
    let tol = elaborate(&rc_phdl(true));
    let mut plain_session = Session::compile(&plain, "Top").expect("compiles");
    let mut tol_session = Session::compile(&tol, "Top").expect("compiles");
    let out = NetRef { name: "out".into() };
    let plain_tr = plain_session
        .tran(5e-3, Some(1e-4), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("tran solves");
    let tol_tr = tol_session
        .tran(5e-3, Some(1e-4), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("tran solves");
    let plain_w = plain_tr.v(&out).expect("v(out)");
    let tol_w = tol_tr.v(&out).expect("v(out)");
    assert_eq!(plain_w.len(), tol_w.len(), "tol must not change the accepted steps");
    for (i, (p, t)) in plain_w.points().iter().zip(tol_w.points().iter()).enumerate() {
        assert_eq!(p.0.to_bits(), t.0.to_bits(), "sample {i} time moved");
        assert_eq!(p.1.to_bits(), t.1.to_bits(), "sample {i} value moved");
    }
    assert!(!plain_w.is_empty(), "the trace carries samples");
}

/// DVC-03 AC4: `ac()` on the toleranced design is bit-identical, point for
/// point, to the untoleranced twin.
#[test]
fn ac_is_bit_identical_with_and_without_tol() {
    let plain = elaborate(&rc_phdl(false));
    let tol = elaborate(&rc_phdl(true));
    let mut plain_session = Session::compile(&plain, "Top").expect("compiles");
    let mut tol_session = Session::compile(&tol, "Top").expect("compiles");
    let out = NetRef { name: "out".into() };
    let plain_ac = plain_session.ac(1.0, 1e6, 8, true, &SolverConfig::default()).expect("ac solves");
    let tol_ac = tol_session.ac(1.0, 1e6, 8, true, &SolverConfig::default()).expect("ac solves");
    let plain_w = plain_ac.v(&out).expect("v(out)");
    let tol_w = tol_ac.v(&out).expect("v(out)");
    assert_eq!(plain_w.len(), tol_w.len());
    for (i, (p, t)) in plain_w.points().iter().zip(tol_w.points().iter()).enumerate() {
        assert_eq!(p.0.to_bits(), t.0.to_bits(), "point {i} frequency moved");
        assert_eq!(p.1.re.to_bits(), t.1.re.to_bits(), "point {i} real part moved");
        assert_eq!(p.1.im.to_bits(), t.1.im.to_bits(), "point {i} imaginary part moved");
    }
}

/// DVC-03 AC6: a param carrying both `tol` and a staged override — the
/// override wins for the nominal value (`v(mid) = 2.5 V`), and the
/// distribution stays attached on the POM param.
#[test]
fn staged_override_wins_and_the_distribution_stays_attached() {
    let design = elaborate(&divider_phdl(true));
    let mut session = Session::builder(&design, "Divider")
        .stage("r_top", "r", Value::Real(2e3))
        .compile()
        .expect("compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let mid = NetRef { name: "mid".into() };
    assert!(
        (op.v(&mid).expect("v(mid)") - 2.5).abs() < 1e-9,
        "the staged override, not the nominal default, sets the value"
    );

    let dist = design
        .module("Resistor")
        .and_then(|m| m.param("r"))
        .and_then(|p| p.distribution())
        .expect("the tol clause survives staging on the POM param");
    assert_eq!(dist.kind, "uniform");
    assert!(!dist.global);
}

/// DVC-03: a live write to a param carrying `tol` returns the same
/// `Invalidation` class as the same write on the untoleranced twin.
#[test]
fn invalidation_class_is_unchanged_by_tol() {
    use piperine_codegen::CircuitCompiler;
    use piperine_solver::prelude::{Invalidation, Value as SolverValue};

    let build = |with_tol: bool| {
        let design = elaborate(&divider_phdl(with_tol));
        let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
        CircuitCompiler::new(&design, &bodies).build_circuit("Divider").expect("circuit builds")
    };
    let mut plain = build(false);
    let mut tol = build(true);

    let set_on = |ci: &mut piperine_solver::prelude::CircuitInstance| {
        ci.all_devices_mut()
            .iter_mut()
            .find(|d| d.name() == "r_top")
            .expect("r_top device exists")
            .set_param("r", SolverValue::Real(4e3))
            .expect("set_param succeeds")
    };
    let plain_inv = set_on(&mut plain);
    let tol_inv = set_on(&mut tol);
    assert_eq!(plain_inv, tol_inv, "tol must not change the Invalidation class");
    assert_eq!(tol_inv, Invalidation::Restamp, "a scalar param write restamps (MD-18)");
}
