//! dv-gradients T21 (DVG-21) — aging as parameter drift: the host computes
//! a stress proxy from an OP (or transient) run, restamps declared drift
//! rates through the ordinary restamp path, re-verifies, and reports the
//! margin delta — which constraint goes negative first, and after how
//! long. Also covers the edge case (spec + DVG-21): a drift that would
//! cross a rebuild-class (`Invalidation::Rebuild`) boundary fails loud
//! rather than silently re-elaborating.
//!
//! **Migrated** (constraints-on-vams): the fixtures run against the
//! `constraint begin … end` producer.

use piperine_api::{age, AgingSpec, DriftRate, Session, SolverConfig, StressSource};
use piperine_lang::SourceMap;

/// A resistive divider whose `r1` drifts UPWARD with stress
/// (`rate_per_stress > 0`), lowering `V(out)` over lifetime until the
/// `require head_lo : V(p, n) >= lo` margin goes negative — a real
/// margin-erosion path driven purely by this module's linear placeholder
/// model (no calibrated device physics involved).
///
/// The `SourceMap` reaching `piperine-lang`'s bundled headers (copied
/// verbatim from `margins_channel.rs`).
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

const AGING_DIVIDER: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real lo = 0.45;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head_lo : V(p, n) >= lo;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

/// A fixture whose `R` module carries a PRESENCE-QUERIED optional param
/// (`parameter real ns = 0.0` guarded by `$param_given("ns")`, never
/// given) alongside the ordinary `r` — exactly the shape
/// `live_params.rs`'s
/// `presence_flipping_set_returns_typed_rebuild_without_applying` test
/// uses to prove `Session::set` on such a param returns
/// `Invalidation::Rebuild`. Aging `r1.ns` must hit that SAME refusal.
const AGING_REBUILD: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    // given => extra ns*1e-6 S in parallel
    parameter real ns = 0.0;
    analog begin
        I(p, n) <+ V(p, n) / r + ($param_given("ns") ? ns : 0.0) * 1.0e-6 * V(p, n);
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real lo = 0.45;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head_lo : V(p, n) >= lo;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

fn design(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates")
}

/// T21 AC1/AC2: aging computes a stress proxy from an OP, restamps the
/// drift, re-verifies, and reports which constraint went negative first
/// and after how long — a genuine margin-erosion path, not a trivial
/// always-passes run.
#[test]
fn aging_reports_the_first_violated_constraint_and_when() {
    let d = design(AGING_DIVIDER);
    let mut session = Session::compile(&d, "Top").expect("session compiles");

    let spec = AgingSpec {
        stress: StressSource::Op { metric: "in".to_string() },
        drifts: vec![DriftRate {
            label: "r1".to_string(),
            param: "r".to_string(),
            from: 1e3,
            rate_per_stress: 50.0,
        }],
        constraints: vec!["dut1.head_lo".to_string()],
        lifetime: 20.0,
        steps: 200,
    };
    let result = age(&mut session, &spec, &SolverConfig::default()).expect("aging runs");

    let (name, at_time) = result.first_violation.clone().expect("the drift must eventually violate head_lo");
    assert_eq!(name, "dut1.head_lo", "the only watched constraint is the one reported");
    assert!(at_time > 0.0 && at_time <= 20.0, "the violation time lies within the scanned lifetime");

    let delta = result.delta("dut1.head_lo").expect("delta reads both channels");
    assert!(delta < 0.0, "the margin must have shrunk (gone negative) under drift, got delta={delta}");
    assert!(
        result.baseline.get("head_lo").expect("baseline head_lo").scalar() >= 0.0,
        "the baseline (t=0) margin must be satisfied — the erosion is real, not pre-existing"
    );
    assert!(
        result.aged.get("head_lo").expect("aged head_lo").scalar() < 0.0,
        "the aged margin at the reported violation time must actually be negative"
    );

    // No residue: a nominal analysis after aging returns to declared
    // values (T14's discipline, reused).
    let nominal = session.op(&SolverConfig::default(), None).expect("nominal op solves after aging");
    assert!(nominal.requires_ok(), "the design returns to its satisfied nominal state after aging");
}

/// T21 edge case: no drift reaches the watched constraint within the
/// scanned lifetime → `first_violation` is `None`, not a fabricated time.
#[test]
fn no_violation_within_the_scanned_lifetime_reports_none() {
    let d = design(AGING_DIVIDER);
    let mut session = Session::compile(&d, "Top").expect("session compiles");

    let spec = AgingSpec {
        stress: StressSource::Op { metric: "in".to_string() },
        drifts: vec![DriftRate {
            label: "r1".to_string(),
            param: "r".to_string(),
            from: 1e3,
            rate_per_stress: 50.0,
        }],
        constraints: vec!["dut1.head_lo".to_string()],
        // A much shorter horizon than the previous test's 20.0 — the
        // same drift rate must not yet have crossed the boundary.
        lifetime: 0.001,
        steps: 5,
    };
    let result = age(&mut session, &spec, &SolverConfig::default()).expect("aging runs");
    assert!(result.first_violation.is_none(), "a short-enough horizon must report no violation");
    assert!(result.delta("dut1.head_lo").expect("delta") < 0.0, "the margin still eroded, just not to zero");
}

/// T21 edge case (spec + DVG-21): a drift that would cross a
/// rebuild-class boundary fails loud rather than silently re-elaborating.
/// `r1.ns` is a presence-queried optional param never given
/// (`$param_given("ns")`-guarded `parameter real ns = 0.0`) — the SAME
/// shape `live_params.rs` proves classifies as
/// `Invalidation::Rebuild` on its very first `Session::set`. `age` must
/// propagate that refusal rather than swallowing it or re-elaborating
/// around it.
#[test]
fn a_drift_crossing_a_rebuild_class_boundary_fails_loud() {
    let d = design(AGING_REBUILD);
    let mut session = Session::compile(&d, "Top").expect("session compiles");

    // Sanity: `Session::set` on `r1.ns` really is a structural (presence-
    // flipping) write and really does fail loud on its own — proving the
    // fixture actually exercises a rebuild-class param, not a
    // restamp-class one.
    let direct = session.set("r1", "ns", 2.0);
    assert!(direct.is_err(), "sanity: r1.ns is a rebuild-class param on this fixture (never given)");
    assert!(direct.unwrap_err().to_string().to_lowercase().contains("rebuild"));

    // A fresh session — the sanity probe above errored (no partial
    // apply), but compile fresh anyway so the aging call below starts
    // from an unambiguous, never-touched circuit.
    let mut session = Session::compile(&d, "Top").expect("fresh session compiles");

    let spec = AgingSpec {
        stress: StressSource::Op { metric: "in".to_string() },
        drifts: vec![DriftRate {
            label: "r1".to_string(),
            param: "ns".to_string(),
            from: 0.0,
            rate_per_stress: 1.0,
        }],
        constraints: vec!["dut1.head_lo".to_string()],
        lifetime: 10.0,
        steps: 10,
    };
    let err = age(&mut session, &spec, &SolverConfig::default())
        .expect_err("a rebuild-class drift target must fail loud, not silently re-elaborate");
    assert!(
        err.to_string().to_lowercase().contains("rebuild"),
        "the failure names the rebuild-class refusal (Session::set's own message): {err}"
    );
}
