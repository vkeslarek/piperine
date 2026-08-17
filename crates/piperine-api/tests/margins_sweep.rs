//! Worst-across-sweep margins (dv-core T20, DVC-11, design §C3's sweep
//! row): a `Session::dc` sweep result carries the worst margin across the
//! swept points **with the swept coordinate** in its argmin, and the same
//! reduction is available over the fluent `Session::sweep` driver via
//! [`MarginsResult::worst_across`] — the shape the optimizer, corner runs,
//! and Monte Carlo all consume.
//!
//! **BLOCKED — fixtures left in PHDL syntax.** This target's fixture needs
//! PHDL's `constraint` block and its `require` clause.
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
use piperine_api::{MarginsResult, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The divider fixture: `dc` volts across r1 + dut1 (both 1 kΩ) puts
/// `dc/2` across `dut1`; its `require head : V(p, n) <= hi` (hi = 0.4)
/// has margin 0.4 − dc/2 — satisfied at dc = 0.6 (+0.1), borderline at
/// 0.8 (0.0), violated at 1.0 (−0.1).
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.4; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut { require head : V(p, n) <= hi; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

fn compile() -> Session {
    let design = piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &SourceMap::dummy())
        .expect("divider elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// DVC-11: the `Session::dc` sweep's margin channel is the
/// worst-across-sweep reduction — worst = 0.4 − 1.0/2 = −0.1, with the
/// swept coordinate dc = 1.0 as its argmin.
#[test]
fn dc_sweep_carries_the_worst_margin_with_the_swept_coordinate() {
    let mut session = compile();
    let trace = session
        .dc("v1", "dc", &[0.6, 0.8, 1.0], &collect_config(), None)
        .expect("collect sweep completes");
    let margins = trace.margins();
    let head = margins.get("head").expect("the require is in the channel");
    assert!(
        (head.scalar() - (0.4 - 0.5)).abs() < 1e-9,
        "worst margin across the sweep = 0.4 − 0.5 = −0.1, got {}",
        head.scalar()
    );
    assert_eq!(
        head.value,
        piperine_api::MarginValue::Pointwise { worst: 0.4 - 0.5, argmin: Some(1.0) },
        "the argmin is the swept coordinate, not a time"
    );
    assert!(!margins.requires_ok(), "a violated require gates requires_ok");
    let violation = margins.first_violation().expect("the violation is readable");
    assert_eq!(violation.label, "head");
    assert_eq!(violation.instance_path, "dut1");
}

/// DVC-11: the same reduction over the fluent `Session::sweep` driver —
/// collect each swept point's channel and fold with
/// [`MarginsResult::worst_across`], pairing each point with its swept
/// coordinate.
#[test]
fn fluent_sweep_reduces_to_worst_across_with_coordinates() {
    let mut session = compile();
    let config = collect_config();
    let mut points = Vec::new();
    let mut sweep = session.sweep("v1", "dc", &[0.6, 0.8, 1.0]);
    while let Some(point) = sweep.next() {
        let mut point = point.expect("point restamps");
        let op = point.op(&config, None).expect("op solves");
        points.push((
            point.value,
            MarginsResult::from_reported(op.stats().margins.clone()),
        ));
    }
    let worst = MarginsResult::worst_across(&points);
    let head = worst.get("head").expect("the require is in the channel");
    assert!((head.scalar() - (0.4 - 0.5)).abs() < 1e-9, "worst = −0.1, got {}", head.scalar());
    assert_eq!(
        head.value,
        piperine_api::MarginValue::Pointwise { worst: 0.4 - 0.5, argmin: Some(1.0) },
        "the argmin is the swept coordinate"
    );
}

/// DVC-15 across a sweep: in `strict`, the first swept point whose
/// require is violated aborts the sweep loud — the sweep does not
/// silently skip the failing corner.
#[test]
fn strict_sweep_aborts_at_the_violated_point() {
    let mut session = compile();
    let err = session
        .dc("v1", "dc", &[0.6, 1.0], &SolverConfig::default(), None)
        .expect_err("strict aborts at the dc = 1.0 point");
    let msg = err.to_string();
    assert!(msg.contains("head"), "names the constraint: {msg}");
    assert!(msg.contains("dut1"), "names the instance path: {msg}");
}
