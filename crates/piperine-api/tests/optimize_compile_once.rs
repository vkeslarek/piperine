//! dv-gradients T17 (DVG-14 AC5 / MD-18): an entire `optimize` run — the
//! feasibility phase's climbing steps plus the objective phase's gradient
//! descent and line search — restamps on the one build; the compile
//! count is unchanged across the whole run. Lives in its own test binary
//! (a single `#[test]`) so the process-global
//! [`AnalogKernel::compile_count`] delta is not polluted by concurrent
//! tests in the same process (the `compile_once_sweep.rs` pattern).
//!
//! **BLOCKED — fixtures left in PHDL syntax.** This target's fixture needs
//! PHDL's `constraint` block and its `require` clause.
//! Verilog-AMS Annex A has no production for any of them: MD-45 deleted
//! `pom::ConstraintBlock`/`QuantityClass`/`Module::constraints()` with the
//! PHDL AST (T21), `lower_bodies` now always sets `LoweredBody::constraints`
//! and `::cover` to `None`, `elab/lower/vams.rs` always writes
//! `Param::distribution = None`, and `MarginsCatalog::compute` returns an
//! empty catalog unconditionally. A future phase re-anchors the dv-core
//! surface on VAMS `(* attribute *)` syntax (`.specs/STATE.md` MD-45,
//! `resolve/pom/constraints.rs`'s module doc). Until then the fixtures below
//! cannot be retargeted and the tests are left failing rather than weakened,
//! deleted, or `#[ignore]`d (repo-banned by `tests/suite_hygiene.rs`).

use piperine_api::{Driver, DriverOutcome, OptimizeSpec, Policy, Session, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::SourceMap;

const DIVIDER_BOUND_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param lo: Real = 0.2; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut { require lb : V(p, n) >= lo; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    r2   : R(.p = out,    .n = gnd);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

#[test]
fn the_whole_optimization_run_costs_zero_additional_compiles() {
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_BOUND_PHDL, &SourceMap::dummy()).expect("elaborates");
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
