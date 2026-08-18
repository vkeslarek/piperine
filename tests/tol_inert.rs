//! dv-core T5 (DVC-03) — `tol` is inert at solve. A design whose params
//! carry `tol` clauses produces **bit-identical** `op()`/`tran()`/`ac()`
//! results to the same design without them; a staged override wins for the
//! nominal value while the distribution stays attached on the POM; and the
//! `Invalidation` class a live param write returns is unchanged by the
//! presence of a `tol`.
//!
//! **Migrated** (random-variable-catalogue): the fixtures run against the
//! live VAMS `tol gauss(…)`/`tol uniform(…)` parameter-distribution
//! surface — `tol`/`global` are contextual keywords, the keyed args are
//! const-evaluated in module scope, and the distribution lands on the POM
//! `Param::distribution` without ever reaching the solver.

use piperine::{NetRef, Session, SolverConfig};
use piperine_lang::{SourceMap, Value};

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// The divider twin, with or without `tol` clauses on its params:
/// `mid = 5·2k/(3k+2k) = 2.0 V` nominal; staging `r_top.r = 2e3` gives 2.5 V.
fn divider(with_tol: bool) -> String {
    let tol_v = if with_tol { " tol gauss(sigma = 0.001) global" } else { "" };
    let tol_r = if with_tol { " tol uniform(rel = 0.01)" } else { "" };
    format!(
        r#"`include "disciplines.vams"
module VoltageSource (inout electrical p, inout electrical n);
    parameter real voltage = 0.0{tol_v};
    analog begin
        V(p, n) <+ voltage;
    end
endmodule
module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3{tol_r};
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Divider ();
    electrical gnd, vin, mid;
    VoltageSource #(.voltage(5.0)) src (.p(vin), .n(gnd));
    Resistor #(.r(3e3)) r_top (.p(vin), .n(mid));
    Resistor #(.r(2e3)) r_bot (.p(mid), .n(gnd));
endmodule
"#
    )
}

/// The RC twin, with or without a `tol` clause — the `tran`/`ac` vehicle.
fn rc(with_tol: bool) -> String {
    let tol_r = if with_tol { " tol uniform(rel = 0.01)" } else { "" };
    format!(
        r#"`include "disciplines.vams"
module V (inout electrical p, inout electrical n);
    parameter real dc = 0.0;
    parameter real acmag = 0.0;
    analog begin
        V(p, n) <+ dc + ac_stim(acmag, 0.0);
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3{tol_r};
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module C (inout electrical p, inout electrical n);
    parameter real c = 1e-6;
    analog begin
        I(p, n) <+ c * ddt(V(p, n));
    end
endmodule
module Top ();
    electrical gnd, vin, out;
    V #(.dc(5.0), .acmag(1.0)) v1 (.p(vin), .n(gnd));
    R r1 (.p(vin), .n(out));
    C c1 (.p(out), .n(gnd));
endmodule
"#
    )
}

fn elaborate(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates")
}

/// DVC-03 AC4: `op()` on the toleranced design is bit-identical to the
/// untoleranced twin.
#[test]
fn op_is_bit_identical_with_and_without_tol() {
    let plain = elaborate(&divider(false));
    let tol = elaborate(&divider(true));
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
    let plain = elaborate(&rc(false));
    let tol = elaborate(&rc(true));
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
    let plain = elaborate(&rc(false));
    let tol = elaborate(&rc(true));
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
    let design = elaborate(&divider(true));
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
        let design = elaborate(&divider(with_tol));
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
