//! dv-gradients T19 (DVG-19) — the center policy: maximize
//! `min_i (m_i / σ_i)` over the design parameters, using `dv-core`'s
//! normalized margins and T15's `σ_i` (`SamplePlan::monte_carlo`). The
//! `driver/center.rs` logic itself landed structurally in T16's commit
//! (see its SPEC_DEVIATION note); this file validates it against T19's
//! acceptance criteria.
//!
//! **Migrated** (random-variable-catalogue): the fixture runs against the
//! live VAMS `tol gauss(sigma_rel = …)` parameter-distribution surface
//! and the in-module `constraint begin … end` producer.
//!
//! SPEC-COVERS: part_viii#15

use std::collections::HashMap;

use piperine_api::prelude::CheckPosture;
use piperine_api::{CenterSpec, Driver, DriverOutcome, Policy, SamplePlan, Session, SigmaScheme, SolverConfig};
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// Two competing `require`s bracket `V(out)`: `head_hi : V <= 0.6` and
/// `head_lo : V >= 0.4`. `r1` carries mismatch `tol`, so both margins have
/// comparable `σ` (same underlying `V(out)` spread, opposite sign). The
/// nominal `r1 = 1500` puts `V(out) = 1000/(1500+1000) = 0.4` — exactly
/// on the `head_lo` boundary (zero margin, the worst-case point) — known,
/// off-center geometry for the centering algorithm to correct.
const COMPETING: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1500.0 tol gauss(sigma_rel = 0.05);
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.6;
    parameter real lo = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head_hi : V(p, n) <= hi;
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

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

fn center_spec() -> CenterSpec {
    CenterSpec {
        constraints: vec!["dut1.head_hi".to_string(), "dut1.head_lo".to_string()],
        over: vec![("r1".to_string(), "r".to_string())],
        samples_per_outer: 200,
        seed: 17,
        outer_iters: 4,
        inner_iters: 15,
        step: 0.03,
    }
}

/// T19 AC1/AC4: `center` returns the centered parameters and the
/// worst-case distance in sigmas, and that distance is an IMPROVEMENT
/// over the nominal (off-center, at a zero-margin boundary) point.
#[test]
fn center_raises_the_worst_normalized_margin_above_nominal() {
    let competing_design = design(COMPETING);
    let mut session = Session::compile(&competing_design, "Top").expect("session compiles");

    // Nominal worst normalized margin: min(margin_hi, margin_lo) / sigma,
    // sigma estimated the same way the driver estimates it (a Monte
    // Carlo at the nominal point).
    let plan = SamplePlan::collect(&competing_design, "Top").expect("plan collects");
    let nominal_mc = plan
        .monte_carlo(&mut session, 200, 17, &collect_config())
        .expect("nominal monte carlo runs");
    let sigma_hi = nominal_mc.sigma("dut1.head_hi").expect("sigma(head_hi)");
    let sigma_lo = nominal_mc.sigma("dut1.head_lo").expect("sigma(head_lo)");
    let nominal_op = session.op(&collect_config(), None).expect("nominal op solves");
    let m_hi = nominal_op.margins().get("head_hi").expect("head_hi").scalar();
    let m_lo = nominal_op.margins().get("head_lo").expect("head_lo").scalar();
    let nominal_worst = (m_hi / sigma_hi).min(m_lo / sigma_lo);
    assert!(
        nominal_worst < 0.5,
        "sanity: the nominal point (r1=1500, V(out)=0.4) must be near/at the head_lo boundary, \
         got worst normalized margin {nominal_worst}"
    );

    // Center.
    let driver = Driver::new(Policy::Center(center_spec()));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("center runs");
    let DriverOutcome::Center(result) = outcome else { panic!("expected Center outcome") };

    assert_eq!(
        result.sigma_scheme,
        SigmaScheme::FixedWithinOuterIteration,
        "T19 AC2: the result states which sigma scheme it used"
    );
    assert!(result.outer_iterations > 0, "at least one outer iteration ran");
    assert!(
        !result.params.is_empty(),
        "the centered parameters are reported"
    );
    assert!(
        result.worst_normalized_margin > nominal_worst,
        "centering must raise the worst normalized margin above nominal: centered={} nominal={}",
        result.worst_normalized_margin,
        nominal_worst
    );
}

/// T19 AC4 (independent test criterion): a Monte Carlo run AT the
/// centered point shows higher yield than a Monte Carlo at nominal — the
/// practical payoff of centering.
#[test]
fn a_monte_carlo_at_the_centered_point_shows_higher_yield_than_nominal() {
    let competing_design = design(COMPETING);
    let mut session = Session::compile(&competing_design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&competing_design, "Top").expect("plan collects");

    let nominal_mc = plan
        .monte_carlo(&mut session, 300, 5, &collect_config())
        .expect("nominal monte carlo runs");
    let nominal_yield = nominal_mc.yield_fraction();

    let driver = Driver::new(Policy::Center(center_spec()));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("center runs");
    let DriverOutcome::Center(result) = outcome else { panic!("expected Center outcome") };

    // Apply the centered parameters to the session (the driver already
    // leaves `session` there, but apply explicitly from the reported
    // params for a test that does not rely on internal driver state).
    let params: HashMap<String, f64> = result.params.clone();
    let (r1_label, r1_param) = ("r1", "r");
    let centered_r = params
        .get(&format!("{r1_label}.{r1_param}"))
        .copied()
        .expect("centered r1.r is reported");
    session.set(r1_label, r1_param, centered_r).expect("apply centered r1.r");

    // Sample AROUND the centered point (`monte_carlo_around`), not the
    // design's original declared nominal — `monte_carlo` alone would
    // ignore the centering entirely and always perturb around 1500.
    let centered_mc = plan
        .monte_carlo_around(&mut session, 300, 5, &collect_config(), &params)
        .expect("centered monte carlo runs");
    let centered_yield = centered_mc.yield_fraction();

    assert!(
        centered_yield > nominal_yield,
        "the centered point must show higher Monte Carlo yield than nominal: centered={} nominal={}",
        centered_yield,
        nominal_yield
    );
}
