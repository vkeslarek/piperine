//! dv-gradients T17 (DVG-14 AC5 / MD-18): an entire `optimize` run — the
//! feasibility phase's climbing steps plus the objective phase's gradient
//! descent and line search — restamps on the one build; the compile
//! count is unchanged across the whole run. Lives in its own test binary
//! (a single `#[test]`) so the process-global
//! [`AnalogKernel::compile_count`] delta is not polluted by concurrent
//! tests in the same process (the `compile_once_sweep.rs` pattern).
//!
//! **Migrated** (constraints-on-vams): the fixture runs against the
//! `constraint begin … end` producer.

use piperine_api::{Driver, DriverOutcome, OptimizeSpec, Policy, Session, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers (same
/// shape as `margins_channel.rs`'s).
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

const DIVIDER_BOUND: &str = r#"`include "disciplines.vams"
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
    parameter real lo = 0.2;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require lb : V(p, n) >= lo;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    R r2 (.p(out), .n(gnd));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

#[test]
fn the_whole_optimization_run_costs_zero_additional_compiles() {
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_BOUND, &source_map()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");

    let spec = OptimizeSpec {
        objective: "out".to_string(),
        minimize: true,
        over: vec![("r1".to_string(), "r".to_string()), ("r2".to_string(), "r".to_string())],
        max_iter: 60,
        step: 0.2,
    };
    let driver = Driver::new(Policy::Optimize(spec));

    let before = AnalogKernel::compile_count();
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("optimize runs");
    let delta = AnalogKernel::compile_count() - before;

    let DriverOutcome::Optimize(result) = outcome else { panic!("expected Optimize outcome") };
    assert!(result.feasible);
    assert_eq!(delta, 0, "MD-18: the whole optimize run must restamp on the one build, got {delta} recompiles");
}
