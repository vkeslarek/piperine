//! Scope: T54 (`lrm-conformance-wave3`, PSW3-08/B10) — proves
//! `examples/24_constraint_tol_centering.vams` (the new example exercising
//! the `constraint`/`tol`/optimizer extension surface) genuinely runs a
//! centering pass with a numeric improvement, on the one host where that
//! surface exists (`Driver`/`Policy::Center` — Rust-only, the declared
//! asymmetry `tests/host_parity.rs::host_parity_declared_rust_only_surface_
//! has_no_python_binding` pins, spec.md's B5). The example's own `.py`
//! twin exercises the half that IS on both hosts (the `require`/`tol`
//! declarations and the margins channel they produce); this file completes
//! the proof for the half that only the Rust host carries — mirrors
//! `crates/piperine-api/tests/center_policy.rs`'s fixture/assertions
//! exactly, but against the real example file rather than an inline
//! string, so a regression in the example itself would be caught here too.
//!
//! SPEC-COVERS: appendix_a#A.3

use piperine::prelude::CheckPosture;
use piperine::{CenterSpec, Driver, DriverOutcome, Policy, SamplePlan, Session, SolverConfig};
use piperine_lang::SourceMap;

fn example_source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn load_example() -> piperine_lang::Design {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/24_constraint_tol_centering.vams");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    piperine_lang::parse_and_elaborate(&src, &example_source_map())
        .unwrap_or_else(|e| panic!("example elaborates: {e:?}"))
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

/// PSW3-08/B10: the example's nominal point sits at the `head_lo` boundary
/// (the `.py` twin proves this directly: `V(out) == 0.4`, `head_lo`'s own
/// floor) — a real, non-contrived off-center starting point — and a
/// `Policy::Center` pass over it raises the worst normalized margin, a
/// concrete numeric outcome, not merely "the driver ran without error".
#[test]
fn centering_the_example_improves_the_worst_normalized_margin() {
    let design = load_example();
    let mut session = Session::compile(&design, "Top").expect("session compiles");

    let plan = SamplePlan::collect(&design, "Top").expect("plan collects");
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
        "sanity: the example's nominal point must be near/at the head_lo boundary, got worst \
         normalized margin {nominal_worst}"
    );

    let driver = Driver::new(Policy::Center(center_spec()));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("center runs");
    let DriverOutcome::Center(result) = outcome else { panic!("expected a Center outcome") };

    assert!(result.outer_iterations > 0, "at least one outer iteration ran");
    assert!(!result.params.is_empty(), "the centered parameters are reported");
    assert!(
        result.worst_normalized_margin > nominal_worst,
        "centering the example must raise the worst normalized margin above nominal: \
         centered={} nominal={}",
        result.worst_normalized_margin,
        nominal_worst
    );
}
