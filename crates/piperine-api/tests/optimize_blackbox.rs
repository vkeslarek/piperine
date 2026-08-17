//! dv-gradients T18 (DVG-15) — the black-box fallback engine, composed
//! with the gradient path: a mixed fixture where the feasibility phase's
//! `require` (a plain `V()` probe) is analytically differentiable, but
//! the OBJECTIVE (`I()`-probed, refused by the adjoint's probe extractor
//! today) is not — "gradient inside a discrete shell", with every result
//! naming which engine(s) produced it.
//!
//! **Migrated** (constraints-on-vams): the fixtures run against the
//! `constraint begin … end` producer.

use piperine_api::{Driver, DriverOutcome, Engine, OptimizeSpec, Policy, Session, SolverConfig};
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// `head` (a `V(p, n) <= hi` require) is a plain-voltage-probe margin —
/// differentiable through the adjoint. `ilim_c` (an `I(p, n) <= ilim`
/// require) probes a branch CURRENT, which the adjoint's probe extractor
/// refuses today (`extract_voltage_probe` only accepts `V()`) — a
/// genuinely non-differentiable-through-this-delivery objective that
/// still has a real, param-dependent value at every point. Nominal
/// (r1=1e3) VIOLATES `head` (V(out)=0.5 > hi=0.4), forcing the
/// feasibility phase to actually climb.
const MIXED: &str = r#"`include "disciplines.vams"
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
    parameter real ilim = 0.001;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
        require ilim_c : I(p, n) <= ilim;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

fn design(phdl: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(phdl, &source_map()).expect("elaborates")
}

/// T18 AC1/AC2/AC3: the black-box engine handles the non-differentiable
/// `ilim_c` objective; it composes with the gradient-based feasibility
/// climb that `head` forces; and the result names BOTH engines.
#[test]
fn a_mixed_fixture_composes_gradient_feasibility_with_black_box_objective() {
    let phdl_design = design(MIXED);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");

    // Sanity: `Session::gradient` really does refuse `ilim_c` (an I()
    // probe) — this run's black-box path is genuinely exercised, not a
    // false negative.
    let refuse = session.gradient("ilim_c", &[("r1".to_string(), "r".to_string())], &SolverConfig::default());
    assert!(refuse.is_err(), "sanity: an I()-probed require must not be analytically differentiable today");

    let spec = OptimizeSpec {
        objective: "ilim_c".to_string(),
        minimize: false, // maximize the margin -> push I(p,n) down, away from the limit
        over: vec![("r1".to_string(), "r".to_string())],
        max_iter: 40,
        step: 0.2,
    };
    let driver = Driver::new(Policy::Optimize(spec));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("optimize runs");
    let DriverOutcome::Optimize(result) = outcome else { panic!("expected Optimize outcome") };

    assert!(result.feasible, "the feasibility phase must have climbed `head` to satisfied");
    assert!(
        result.engines.contains(&Engine::Gradient),
        "the feasibility climb used the gradient engine: {:?}",
        result.engines
    );
    assert!(
        result.engines.contains(&Engine::BlackBox),
        "the non-differentiable objective phase used the black-box engine: {:?}",
        result.engines
    );
    assert_eq!(
        result.engines.len(),
        2,
        "a mixed run names BOTH engines, no more no fewer: {:?}",
        result.engines
    );
}

/// T18 AC1: a purely-differentiable run (no I()-probed objective) uses
/// ONLY the gradient engine — the black-box engine is never spuriously
/// attributed when it did not run.
#[test]
fn a_purely_differentiable_run_never_attributes_black_box() {
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
    let phdl_design = design(DIVIDER_BOUND);
    let mut session = Session::compile(&phdl_design, "Top").expect("session compiles");
    let spec = OptimizeSpec {
        objective: "out".to_string(),
        minimize: true,
        over: vec![("r1".to_string(), "r".to_string()), ("r2".to_string(), "r".to_string())],
        max_iter: 60,
        step: 0.2,
    };
    let driver = Driver::new(Policy::Optimize(spec));
    let outcome = driver.run(&mut session, &SolverConfig::default()).expect("optimize runs");
    let DriverOutcome::Optimize(result) = outcome else { panic!("expected Optimize outcome") };
    assert_eq!(result.engines, std::collections::HashSet::from([Engine::Gradient]));
}
