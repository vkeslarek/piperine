//! dv-gradients T15 (DVG-18 / MD-18): a Monte Carlo run of 1000 samples
//! costs exactly the compiles the session already paid for building — the
//! sampling loop restamps, it never recompiles. Lives in its own test
//! binary (a single `#[test]`) so the process-global
//! [`AnalogKernel::compile_count`] delta is not polluted by concurrent
//! tests in the same process (the `compile_once_sweep.rs` / `urc_compile_count.rs`
//! pattern).
//!
//! **Migrated** (random-variable-catalogue): the fixture runs against the
//! live VAMS `tol gauss(…)` parameter-distribution surface and the
//! in-module `constraint begin … end` producer.

use piperine_api::prelude::CheckPosture;
use piperine_api::{SamplePlan, Session, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

const DIVIDER_TOL: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3 tol gauss(sigma_rel = 0.3);
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.55;
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

/// DVG-18 / MD-18: a 1000-sample Monte Carlo run costs exactly the
/// compiles the session already paid for building — the loop restamps,
/// it never recompiles.
#[test]
fn a_thousand_samples_cost_zero_additional_compiles() {
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_TOL, &source_map()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let plan = SamplePlan::collect(&design, "Top").expect("plan collects");
    let config = SolverConfig { checks: CheckPosture::Collect, ..Default::default() };

    let before = AnalogKernel::compile_count();
    let result = plan.monte_carlo(&mut session, 1000, 5, &config).expect("monte carlo runs");
    let delta = AnalogKernel::compile_count() - before;

    assert_eq!(result.n, 1000);
    assert_eq!(result.outcomes.len(), 1000);
    assert_eq!(delta, 0, "MD-18: 1000 samples must restamp on the one build, got {delta} recompiles");
}
