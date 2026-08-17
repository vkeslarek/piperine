//! Constraints end to end over the host surface (constraints-on-vams,
//! spec P1–P3 Independent Tests + edge cases): a declared `require`
//! aborts under `strict` naming the constraint, the instance path, and
//! the margin; raising the limit passes with worst margins reported; the
//! worst margin changes sign at the expected swept coordinate with
//! `argmin()` naming it; the three postures behave as LRM §9.7.3's
//! severities ($error aborts, $warning records, off is silent); multiple
//! instances report per-instance margins by authored path; a constrained
//! module two levels deep in the hierarchy still lowers and reports.

use piperine::prelude::CheckPosture;
use piperine::{MarginValue, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The divider: `dc` volts across r1 + dut1 (both 1 kΩ) puts `dc/2`
/// across `dut1`. With dc = 1.0 and `require head : V(p, n) <= hi`
/// (hi = 0.4) the margin is −0.1 at the operating point.
const DIVIDER: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1.0e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers from this
/// crate.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn compile(src: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn config(posture: CheckPosture) -> SolverConfig {
    SolverConfig { checks: posture, ..Default::default() }
}

/// P1's Independent Test: a divider driving a node past a declared limit
/// fails under `strict`, loudly, by name — and raising the limit makes
/// the same fixture pass, reporting each constraint satisfied with its
/// worst margin.
#[test]
fn p1_violation_aborts_strict_and_raising_the_limit_passes() {
    let mut session = compile(DIVIDER);
    let err = session
        .op(&config(CheckPosture::Strict), None)
        .expect_err("strict + violated require aborts the run");
    let msg = err.to_string();
    assert!(msg.contains("head"), "names the constraint: {msg}");
    assert!(msg.contains("dut1"), "names the instance path: {msg}");

    // Same fixture, limit raised past the operating point: passes and
    // reports the worst margin.
    session.set("dut1", "hi", 0.6).expect("restamp");
    let op = session.op(&config(CheckPosture::Strict), None).expect("passes at hi = 0.6");
    let head = op.margins().get("head").expect("the require is in the channel");
    assert_eq!(head.value, MarginValue::Pointwise { worst: 0.6 - 0.5, argmin: Some(0.0) });
    assert!(head.is_satisfied());
    assert!(op.requires_ok(), "all requires hold at every accepted point");
}

/// P2's Independent Test: sweep the source across the limit and the worst
/// margin changes sign exactly where the sweep crosses it — `dc/2 > hi`
/// at `dc > 0.8` — with `argmin()` naming the swept coordinate.
#[test]
fn p2_worst_margin_changes_sign_at_the_crossing_with_the_swept_argmin() {
    let mut session = compile(DIVIDER);
    let trace = session
        .dc("v1", "dc", &[0.6, 0.8, 1.0], &config(CheckPosture::Collect), None)
        .expect("sweep completes");
    let margins = trace.margins();
    let head = margins.get("head").expect("the require is in the channel");
    assert!(
        (head.scalar() - (0.4 - 0.5)).abs() < 1e-9,
        "worst = hi − dc_max/2 = −0.1, got {}",
        head.scalar()
    );
    assert_eq!(head.argmin(), Some(1.0), "the argmin is the swept coordinate");
    assert!(!margins.requires_ok(), "violated at the far end of the sweep");

    // Shrinking the sweep to the satisfied side flips the sign.
    let ok = session
        .dc("v1", "dc", &[0.6, 0.7], &config(CheckPosture::Collect), None)
        .expect("sweep completes");
    let margins = ok.margins();
    let head = margins.get("head").expect("the require is in the channel");
    assert!(head.scalar() > 0.0, "satisfied everywhere: dc/2 ≤ 0.35 < hi");
    assert_eq!(head.argmin(), Some(0.7), "worst at the largest satisfied dc");
    assert!(ok.requires_ok());
}

/// P3's Independent Test: one violating fixture, three postures —
/// `strict` aborts ($error), `collect` completes and records every
/// violation ($warning), `off` completes silent at no cost.
#[test]
fn p3_the_three_postures_abort_record_and_stay_silent() {
    // strict: aborts.
    let mut strict = compile(DIVIDER);
    let err = strict
        .op(&config(CheckPosture::Strict), None)
        .expect_err("strict aborts on the violated require");
    assert!(err.to_string().contains("head"), "the abort names the constraint");

    // collect: completes, every violation recorded and readable.
    let mut collect = compile(DIVIDER);
    let op = collect.op(&config(CheckPosture::Collect), None).expect("collect completes");
    assert!(!op.requires_ok());
    let violation = op.first_violation().expect("the violation is recorded");
    assert_eq!(violation.label, "head");
    assert_eq!(violation.instance_path, "dut1");

    // off: completes, evaluates nothing — the channel reports no margins
    // at all ("nothing was evaluated, so there is nothing honest to
    // report"), and no violation is raised.
    let mut off = compile(DIVIDER);
    let op = off.op(&config(CheckPosture::Off), None).expect("off completes");
    assert!(
        op.margins().get("head").is_err(),
        "off evaluates nothing — the channel carries no margin to read"
    );
}

/// Spec edge case: the same constrained module instantiated many times —
/// each instance reports its own margin, distinguished by authored
/// instance path (a divider and a direct drive put different voltages on
/// the two instances).
#[test]
fn multiple_instances_report_per_instance_margins_by_path() {
    let src = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1.0e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Top ();
    electrical gnd, in, out, mid;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
    Dut dut2 (.p(mid), .n(gnd));
endmodule
"#;
    // dut2's terminals float to the same node — both instance paths
    // appear in the channel with their own entries.
    let mut session = compile(src);
    let op = session.op(&config(CheckPosture::Collect), None).expect("collect completes");
    let margins = op.margins();
    let d1 = margins.get("dut1.head").expect("dut1's own margin by qualified path");
    assert_eq!(d1.instance_path, "dut1");
    assert!((d1.scalar() - (0.4 - 0.5)).abs() < 1e-9, "dut1 at 0.5 V: −0.1");
    let _ = margins.get("dut2.head").expect("dut2's own margin, distinguished by path");
}

/// Spec edge case (the design's flatten risk): a constrained module two
/// levels deep in the hierarchy — `Top → seg → dut` — still lowers,
/// evaluates, and reports through its authored spliced path.
#[test]
fn a_mid_hierarchy_constrained_module_reports_through_its_spliced_path() {
    let src = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 0.5;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Seg (inout electrical p, inout electrical n);
    Dut d (.p(p), .n(n));
endmodule
module Top ();
    electrical gnd, in;
    Vsrc v1 (.p(in), .n(gnd));
    Seg seg (.p(in), .n(gnd));
endmodule
"#;
    let mut session = compile(src);
    let op = session.op(&config(CheckPosture::Collect), None).expect("collect completes");
    let head = op.margins().get("head").expect("the deep require is in the channel");
    assert_eq!(head.instance_path, "seg.d", "the authored spliced path");
    assert!(
        (head.scalar() - (0.4 - 0.5)).abs() < 1e-9,
        "0.5 V across the nested dut: margin −0.1, got {}",
        head.scalar()
    );
    assert!(!op.requires_ok());
}
