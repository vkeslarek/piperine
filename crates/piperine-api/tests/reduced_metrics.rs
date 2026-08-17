//! Host-side reductions for reduced constraint metrics (dv-core T22,
//! DVC-09, D7): `ac_unity_gain_freq` / `ac_phase_margin` /
//! `ac_gain_margin` are computed host-side over the sweep's points — the
//! solver stays pointwise-pure. The analytic oracle is a known
//! single-pole fixture: a VCCS gain stage (gm·r = 100) into an RC pole
//! (fc = 1/(2π·1k·1µ) ≈ 159.155 Hz), so |H(f)| = 100/√(1+(f/fc)²), the
//! unity crossing is fc·√(100²−1) ≈ 15.9 kHz, the phase margin is
//! 180° − atan(f_u/fc) ≈ 90.57°, and the gain margin is infinite (a
//! single pole never reaches −180°).
//!
//! **BLOCKED — fixtures left in PHDL syntax.** This target's fixture needs
//! PHDL's `constraint` block with its reduced `var` declarations and
//! `target … tol …` clauses.
//! Verilog-AMS Annex A has no production for any of them: MD-45 deleted
//! `pom::ConstraintBlock`/`QuantityClass`/`Module::constraints()` with the
//! PHDL AST (T21), `lower_bodies` now always sets `LoweredBody::constraints`
//! and `::cover` to `None`, `elab/lower/vams.rs` always writes
//! `Param::distribution = None`, and `MarginsCatalog::compute` returns an
//! empty catalog unconditionally. A future phase re-anchors the dv-core
//! surface on VAMS `(* attribute *)` syntax (`.specs/STATE.md` MD-45,
//! `resolve/pom/constraints.rs`'s module doc). Until then the fixtures below
//! cannot be retargeted and the tests are left failing rather than weakened,
//! deleted, or `#[ignore]`d (repo-banned by `tests/suite_hygiene.rs`).

use piperine_api::prelude::CheckPosture;
use piperine_api::{MarginShape, Session, SolverConfig};
use piperine_lang::SourceMap;
use piperine_solver::prelude::MarginKind;

const STAGE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vac(inout p: Electrical, inout n: Electrical) { param acmag: Real = 1.0; }
analog Vac { V(p, n) <+ ac_stim(acmag, 0.0); }

mod Vcvs(inout p: Electrical, inout n: Electrical, inout cp: Electrical, inout cn: Electrical) {
    param gain: Real = 100.0;
}
analog Vcvs { V(p, n) <- gain * V(cp, cn); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Stage(inout inp: Electrical, inout out: Electrical, inout gnd: Electrical) {
    param gm: Real = 0.1;
    param r: Real = 1e3;
    param c: Real = 1e-6;
}
analog Stage {
    I(gnd, out) <+ gm * V(inp, gnd);
    I(out, gnd) <+ V(out, gnd) / r;
    I(out, gnd) <+ c * ddt(V(out, gnd));
}
constraint Stage {
    var ugbw : Real = ac_unity_gain_freq(V(out, gnd));
    var pm   : Real = ac_phase_margin(V(out, gnd));
    var gm   : Real = ac_gain_margin(V(out, gnd));
    target ugbw >= 1e4 tol 1e3;
}

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1 : Vac(.p = in, .n = gnd);
    x1 : Stage(.inp = in, .out = out, .gnd = gnd);
}
";

fn compile() -> Session {
    let design = piperine_lang::parse_and_elaborate(STAGE_PHDL, &SourceMap::dummy()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// The analytic single-pole values.
const FC: f64 = 1.0 / (2.0 * std::f64::consts::PI * 1e3 * 1e-6);
const A0: f64 = 100.0;

fn analytic_ugbw() -> f64 {
    FC * (A0 * A0 - 1.0).sqrt()
}

fn analytic_phase_margin() -> f64 {
    180.0 - (analytic_ugbw() / FC).atan().to_degrees()
}

/// DVC-09/D7: each reduced helper declared in `headers/constraints.phdl`
/// has a host implementation, and the reduction over the AC sweep matches
/// the single-pole analytic values (log-log interpolation between sweep
/// points, so the match is within interpolation error).
#[test]
fn reduced_metrics_match_the_single_pole_analytic_values() {
    let mut session = compile();
    let trace = session.ac(1.0, 1e6, 61, true, &collect_config()).expect("ac completes");
    let margins = trace.margins();

    let ugbw = margins.get("ugbw").expect("ugbw in the channel");
    assert_eq!(ugbw.kind, MarginKind::Var, "a reduced var is kind var");
    assert_eq!(ugbw.shape(), MarginShape::Reduced, "reduced shape, no argmin");
    assert_eq!(ugbw.instance_path, "x1");
    let expected_u = analytic_ugbw();
    assert!(
        (ugbw.scalar() - expected_u).abs() / expected_u < 1e-2,
        "ugbw = {expected_u} Hz analytically, got {} (log-log interpolation)",
        ugbw.scalar()
    );

    let pm = margins.get("pm").expect("pm in the channel");
    let expected_pm = analytic_phase_margin();
    assert!(
        (pm.scalar() - expected_pm).abs() < 0.5,
        "phase margin = {expected_pm}° analytically, got {}",
        pm.scalar()
    );

    let gm = margins.get("gm").expect("gm in the channel");
    assert!(
        gm.scalar().is_infinite() && gm.scalar().is_sign_positive(),
        "a single pole never reaches −180° — infinite gain margin, got {}",
        gm.scalar()
    );
}

/// DVC-08 on the reduced row: a reduced `target` becomes a normalized
/// margin — `(ugbw − 1e4)/1e3` — one value, no argmin.
#[test]
fn a_reduced_target_is_a_normalized_margin_without_argmin() {
    let mut session = compile();
    let trace = session.ac(1.0, 1e6, 61, true, &collect_config()).expect("ac completes");
    let margins = trace.margins();
    let target = margins.get("target#3").expect("the target is in the channel");
    assert_eq!(target.kind, MarginKind::Target);
    assert_eq!(target.shape(), MarginShape::Reduced);
    let expected = (analytic_ugbw() - 1e4) / 1e3;
    assert!(
        (target.scalar() - expected).abs() < 0.1,
        "target margin = (ugbw − 1e4)/1e3 ≈ {expected}, got {}",
        target.scalar()
    );
    assert_eq!(target.argmin(), None, "no argmin is invented for a reduced margin");
}

/// D7 edge case: a reduction over too few points fails loud rather than
/// extrapolating.
#[test]
fn a_reduction_over_too_few_points_fails_loud() {
    let mut session = compile();
    let err = session
        .ac(1.0, 1e6, 1, true, &collect_config())
        .expect_err("one sweep point cannot support a crossing search");
    let msg = err.to_string();
    assert!(
        msg.contains("at least two sweep points"),
        "fails loud instead of extrapolating: {msg}"
    );
}
