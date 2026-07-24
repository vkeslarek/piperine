//! host-library T20 (HOST-18): fluent `Session::sweep` + `SweepPoint` as a
//! `Session` view.
//!
//! Fixture (mirrors `piperine-codegen/tests/live_params.rs`'s
//! presence-flipping oracle): a symmetric conductance divider `G`/`G` with
//! `r2` carrying `param ns: Real? = none` — an *optional* param never given
//! at build. Writing `ns` at all is a presence flip
//! (`Invalidation::Rebuild`, LIVE-14): the divider is `mid = 10·g1/(g1+g2)`,
//! `g2 = 1e-3 + ns·1e-3`.
//!
//! Every expected value below is computed by an independent ground-truth
//! path: a **fresh** `Session::compile` per sweep point with `ns` supplied
//! directly in the PHDL source (never touching `Sweep`/`SweepPoint`) — the
//! spec's own "values match per-point fresh builds" acceptance criterion.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const DIVIDER_TEMPLATE: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod G(inout p: Electrical, inout n: Electrical) {
    param g: Real = 1.0e-3;
    param ns: Real? = none;
}
analog G { I(p, n) <+ (g + ns.get_or(0.0) * 1.0e-3) * V(p, n); }

mod Vsrc(inout p: Electrical, inout n: Electrical) {
    param dc: Real = 10.0;
}
analog Vsrc { V(p, n) <- dc; }

mod Top() {
    wire gnd : Electrical;
    wire top : Electrical;
    wire mid : Electrical;
    v1 : Vsrc(.p = top, .n = gnd) {};
    r1 : G(.p = top, .n = mid) {};
    r2 : G(.p = mid, .n = gnd) { {r2_override} };
}
";

fn elaborate(r2_override: &str) -> piperine_lang::Design {
    let src = DIVIDER_TEMPLATE.replace("{r2_override}", r2_override);
    piperine_lang::parse_and_elaborate(&src, &SourceMap::dummy()).expect("divider fixture elaborates")
}

fn mid_voltage(session: &mut Session) -> f64 {
    session.op(&SolverConfig::default(), None).expect("op solves").v("mid").expect("v(mid)")
}

/// A fresh, independent `Session::compile` with `ns` supplied directly in
/// the source — the ground-truth path the sweep's restamped/rebuilt values
/// are checked against.
fn fresh_build_mid(ns: f64) -> f64 {
    let design = elaborate(&format!(".ns = {ns}"));
    let mut session = Session::compile(&design, "Top").expect("fresh session compiles");
    mid_voltage(&mut session)
}

/// HOST-18 AC1/AC4: sweeping `r2.ns` (an optional param never given at
/// build — a structural presence flip) rebuilds on the first sweep point
/// (`rebuilds` goes 0 -> 1) and each point's value matches an independent
/// fresh-build `Session::compile` with `ns` given directly in the source.
#[test]
fn structural_sweep_rebuilds_once_and_matches_fresh_builds() {
    let design = elaborate(""); // r2.ns never given
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    assert_eq!(session.rebuilds(), 0);

    let values = [1.0_f64, 2.0, 3.0];
    let mut seen = Vec::new();
    {
        let mut sweep = session.sweep("r2", "ns", &values);
        assert_eq!(sweep.len(), values.len());
        while let Some(point) = sweep.next() {
            let mut point = point.expect("sweep point restamps/rebuilds");
            assert!(values.contains(&point.value));
            let mid = mid_voltage(&mut point);
            seen.push((point.index, point.value, mid));
        }
    }
    assert_eq!(seen.len(), 3);

    // The presence flip lands once (the first point, ns: none -> given);
    // subsequent points on the same, now-given `ns` are plain restamps.
    assert_eq!(session.rebuilds(), 1, "exactly one structural rebuild for the whole sweep");

    for (i, (idx, value, mid)) in seen.iter().enumerate() {
        assert_eq!(*idx, i);
        let expected = fresh_build_mid(*value);
        let rel_err = (mid - expected).abs() / expected;
        assert!(
            rel_err < 1e-9,
            "point {i} (ns={value}): sweep mid={mid}, fresh-build mid={expected} (rel {rel_err:.3e})"
        );
    }
}

/// HOST-18 AC1: sweeping a plain (non-structural) numeric param never
/// rebuilds — the sweep restamps every point on the one compilation
/// (`rebuilds` stays 0), and every point's `op()` value matches an
/// independent fresh build.
#[test]
fn non_structural_sweep_never_rebuilds_and_matches_fresh_builds() {
    let design = elaborate(".ns = 0.0"); // ns given at build -> plain Restamp on write
    let mut session = Session::compile(&design, "Top").expect("session compiles");

    let values = [0.5_f64, 1.5, 4.0];
    let mut seen = Vec::new();
    {
        let mut sweep = session.sweep("r2", "ns", &values);
        while let Some(point) = sweep.next() {
            let mut point = point.expect("sweep point restamps");
            let mid = mid_voltage(&mut point);
            seen.push((point.value, mid));
        }
    }
    assert_eq!(session.rebuilds(), 0, "a non-structural sweep never rebuilds");

    for (value, mid) in seen {
        let expected = fresh_build_mid(value);
        let rel_err = (mid - expected).abs() / expected;
        assert!(rel_err < 1e-9, "ns={value}: sweep mid={mid}, fresh-build mid={expected}");
    }
}
