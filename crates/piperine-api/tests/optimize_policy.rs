//! dv-gradients T17 (DVG-14) — the optimize policy: a feasibility phase
//! (climb until every `require` margin ≥ 0) followed by an objective
//! phase (descend the objective while projecting onto the feasible set),
//! using `Session::gradient` (T11) and the `Driver`/`Policy::Optimize`
//! skeleton (T16).
//!
//! **Migrated** (constraints-on-vams): the fixtures run against the
//! `constraint begin … end` producer.

use piperine_api::{Driver, DriverOutcome, OptimizeSpec, Policy, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// A resistive divider whose output voltage `V(out) = dc·G1/(G1+G2+Gd)`
/// (G1 = 1/r1, G2 = 1/r2, Gd = 1e-3 fixed) is DECREASING in `r2` and
/// INCREASING in `r1` — an unconstrained minimum of `V(out)` runs away
/// toward 0 (r2 → 0 or r1 → ∞), so `require lb : V(out) >= lo` (lo=0.2)
/// gives the problem a WELL-POSED, analytically known constrained
/// optimum: `V(out) = 0.2` exactly, at the active boundary. Nominal
/// (r1=r2=1e3) is already feasible (`V(out) = 1/3 ≈ 0.333 >= 0.2`), so
/// the objective phase does the work.
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

fn design(phdl: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(phdl, &source_map()).expect("elaborates")
}

fn optimize_spec() -> OptimizeSpec {
    OptimizeSpec {
        objective: "out".to_string(),
        minimize: true,
        over: vec![("r1".to_string(), "r".to_string()), ("r2".to_string(), "r".to_string())],
        max_iter: 60,
        step: 0.2,
    }
}

/// T17 AC1/AC3: a two-parameter fixture with a known optimum
/// (`V(out) = 0.2`, the active `require lb` boundary) converges the
/// objective value to that analytically known optimum, staying feasible
/// throughout (the objective phase projects onto the feasible set).
#[test]
fn a_two_parameter_fixture_converges_to_its_known_optimum_and_stays_feasible() {
    let phdl_design = design(DIVIDER_BOUND);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");

    let driver = Driver::new(Policy::Optimize(optimize_spec()));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("optimize runs");
    let DriverOutcome::Optimize(result) = outcome else { panic!("expected Optimize outcome") };

    assert!(result.feasible, "the final point must satisfy `require lb` (projected onto the feasible set)");
    assert!(
        (result.objective_value - 0.2).abs() < 5e-3,
        "the minimized V(out) must converge to the known constrained optimum 0.2, got {}",
        result.objective_value
    );
    assert!(
        result.iterations > 0 && result.iterations <= 60,
        "iterations must be bounded by max_iter, got {}",
        result.iterations
    );
    assert!(
        result.engines.contains(&piperine_api::Engine::Gradient),
        "the objective is differentiable — the analytic gradient engine must be used"
    );
}

/// T17 AC2: an objective naming an unknown `var`/net fails loud at the
/// host boundary — before any restamp happens.
#[test]
fn an_objective_naming_an_unknown_var_fails_loud() {
    let phdl_design = design(DIVIDER_BOUND);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");

    let spec = OptimizeSpec {
        objective: "nonexistent_metric".to_string(),
        over: vec![("r1".to_string(), "r".to_string())],
        max_iter: 5,
        ..Default::default()
    };
    let driver = Driver::new(Policy::Optimize(spec));
    let err = driver
        .run(&mut session, &SolverConfig::default())
        .expect_err("an unknown objective must fail loud");
    assert!(
        err.to_string().contains("nonexistent_metric"),
        "the error names the unknown objective: {err}"
    );
}

/// T17 AC1: the feasibility phase actually climbs when the nominal point
/// starts infeasible — proven by an independent fixture whose nominal
/// violates `head`, run through the SAME optimize policy the known-optimum
/// test uses, and confirmed feasible by the end of the run.
#[test]
fn the_feasibility_phase_climbs_an_infeasible_nominal_to_a_feasible_point() {
    const INFEASIBLE: &str = r#"`include "disciplines.vams"
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
    let phdl_design = design(INFEASIBLE);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");

    // Sanity: nominal really is infeasible.
    let nominal = session.op(&piperine_api::SolverConfig { checks: piperine_api::prelude::CheckPosture::Collect, ..Default::default() }, None).expect("collect op solves");
    assert!(!nominal.requires_ok(), "sanity: nominal violates `head`");

    let spec = OptimizeSpec {
        objective: "out".to_string(),
        minimize: true,
        over: vec![("r1".to_string(), "r".to_string())],
        max_iter: 60,
        step: 0.2,
    };
    let driver = Driver::new(Policy::Optimize(spec));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("optimize runs");
    let DriverOutcome::Optimize(result) = outcome else { panic!("expected Optimize outcome") };
    assert!(result.feasible, "the feasibility phase must have climbed to a feasible point");
}
