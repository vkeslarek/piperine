//! dv-gradients T15 (DVG-18 / MD-18): a Monte Carlo run of 1000 samples
//! costs exactly the compiles the session already paid for building — the
//! sampling loop restamps, it never recompiles. Lives in its own test
//! binary (a single `#[test]`) so the process-global
//! [`AnalogKernel::compile_count`] delta is not polluted by concurrent
//! tests in the same process (the `compile_once_sweep.rs` / `urc_compile_count.rs`
//! pattern).

use piperine_api::prelude::CheckPosture;
use piperine_api::{SamplePlan, Session, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::SourceMap;

const DIVIDER_TOL_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3 tol gauss(sigma_rel = 0.3);
}
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.55; }
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

/// DVG-18 / MD-18: a 1000-sample Monte Carlo run costs exactly the
/// compiles the session already paid for building — the loop restamps,
/// it never recompiles.
#[test]
fn a_thousand_samples_cost_zero_additional_compiles() {
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_TOL_PHDL, &SourceMap::dummy()).expect("elaborates");
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
