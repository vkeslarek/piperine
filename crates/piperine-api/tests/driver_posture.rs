//! dv-gradients T16 (DVG-14) — the policy-parameterized `Driver`: one
//! type, three policies (`optimize`/`center`/`high_sigma`), and the
//! structural invariant that `Driver::run` ALWAYS solves in `collect`
//! posture, regardless of the `checks` field on the `SolverConfig` it is
//! called with (T16 AC2/AC4).
//!
//! **Migrated** (constraints-on-vams): the fixtures run against the
//! `constraint begin … end` producer.

use piperine_api::prelude::CheckPosture;
use piperine_api::{Driver, HighSigmaSpec, OptimizeSpec, Policy, Session, SolverConfig};
use piperine_lang::SourceMap;

/// A divider whose NOMINAL point violates `require head` — under
/// `CheckPosture::Strict`, a plain `session.op(&strict_config, None)`
/// call on this design fails loud (the whole point of strict). If
/// `Driver::run` used the caller's `checks` field directly instead of
/// forcing `collect`, running the optimize policy against this fixture
/// with a `Strict` config would fail immediately on its very first
/// (infeasible) read. Succeeding proves the driver ignored the caller's
/// posture and forced `collect` internally (T16 AC2/AC4).
const INFEASIBLE_DIVIDER: &str = r#"`include "disciplines.vams"
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

/// The `SourceMap` reaching `piperine-lang`'s bundled headers (copied
/// verbatim from `margins_channel.rs`).
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn design(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates")
}

/// `Driver::posture()` always reports `Collect` — a static property of
/// the type, independent of any session or config.
#[test]
fn driver_posture_is_always_collect() {
    let spec = OptimizeSpec {
        objective: "head".to_string(),
        over: vec![("r1".to_string(), "r".to_string())],
        max_iter: 1,
        ..Default::default()
    };
    let driver = Driver::new(Policy::Optimize(spec));
    assert_eq!(driver.posture(), CheckPosture::Collect, "T16 AC2: the driver's posture is collect by construction");
}

/// T16 AC4: a fixture proves the posture is `collect` even when the
/// session's config default is `strict` — the infeasible-nominal divider,
/// run through `Driver::run` with a `Strict` `SolverConfig`, must succeed
/// rather than erroring on its first (infeasible) internal read.
#[test]
fn the_driver_forces_collect_posture_even_under_a_strict_config() {
    let d = design(INFEASIBLE_DIVIDER);
    let mut session = Session::compile(&d, "Top").expect("session compiles");

    // Sanity: the nominal point really is infeasible AND really does
    // fail loud under Strict — proving the fixture actually exercises the
    // invariant rather than trivially passing either way.
    let strict_config = SolverConfig { checks: CheckPosture::Strict, ..Default::default() };
    let strict_result = session.op(&strict_config, None);
    assert!(
        strict_result.is_err(),
        "sanity: an ordinary strict op on the infeasible nominal design must fail loud"
    );

    // Run the optimize policy with the SAME strict config. If the driver
    // used it directly, its first internal `session.op` read (nominal,
    // still infeasible) would fail identically. It must not.
    let spec = OptimizeSpec {
        objective: "head".to_string(),
        over: vec![("r1".to_string(), "r".to_string())],
        max_iter: 30,
        ..Default::default()
    };
    let driver = Driver::new(Policy::Optimize(spec));
    let outcome = driver.run(&mut session, &strict_config);
    assert!(
        outcome.is_ok(),
        "the driver must force collect posture internally, not error like a plain strict op: {:?}",
        outcome.err()
    );
}

/// T16 AC1: the driver exposes all three named policies as one type —
/// `Policy::Optimize`/`Policy::Center`/`Policy::HighSigma` — not three
/// separate engine types. (T20 gave `HighSigma` its spec payload;
/// `Policy` is still one type with three variants.)
#[test]
fn the_three_policies_are_variants_of_one_type() {
    fn accepts_policy(_: Policy) {}
    accepts_policy(Policy::Optimize(OptimizeSpec::default()));
    accepts_policy(Policy::Center(piperine_api::CenterSpec::default()));
    accepts_policy(Policy::HighSigma(HighSigmaSpec::default()));
}

/// T20 landed `Policy::HighSigma` (dedicated coverage in
/// `high_sigma_sampling.rs`); this file keeps only the loud-refusal shape
/// T16 established — a design with no declared `tol` distributions has
/// nothing to importance-sample, so it fails loud naming the reason
/// rather than reporting a meaningless yield.
#[test]
fn high_sigma_policy_fails_loud_with_no_tol_declarations() {
    let d = design(INFEASIBLE_DIVIDER);
    let mut session = Session::compile(&d, "Top").expect("session compiles");
    let driver = Driver::new(Policy::HighSigma(HighSigmaSpec::default()));
    let err = driver
        .run(&mut session, &SolverConfig::default())
        .expect_err("the fixture declares no `tol` distributions to sample");
    assert!(err.to_string().contains("tol"), "names the reason: {err}");
}
