//! `ddx` run from MSL source on the Rust backend (analog-operators AO-01, Part IV §14.2): each
//! module is written in `tests/operators/partial.msl`, elaborated by the front end, compiled on
//! the transpiled backend and run on the solver. Every module forces its output to the value of
//! one `ddx`, so the operating point reads the derivative as `v(o)`; every expected value is
//! written from the closed form its criterion gives, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier};

/// The project of `tests/operators/partial.msl`, and what a bench's operating point reads.
struct Partial;

impl Partial {
    fn project() -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/operators/partial.msl");
        Project::new(&path)
    }

    /// `v(o)` of the bench `top` at its operating point.
    fn output(top: &str) -> f64 {
        let mut running = Self::project().compile(top).unwrap_or_else(|r| panic!("{r}"));
        let o = NodeIdentifier::Anonymous(running.ir.node_at(&format!("{top}.o")).expect("node o").id().index());
        let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        solved.get_node(&o).expect("v(o)")
    }

    /// Whether `actual` is `wanted` within `relative`.
    fn within(wanted: f64, actual: f64, relative: f64) -> bool {
        (actual - wanted).abs() <= relative * wanted.abs().max(f64::MIN_POSITIVE)
    }
}

#[test]
fn ddx_of_the_diode_current_is_its_conductance() {
    // AO-01 AC 1: `ddx(id, v(a, k))` of `is * (exp(v/nvt) - 1)` at 0.6 V is `is/nvt * exp(0.6/nvt)`,
    // to 1e-9 relative.
    let (is, nvt) = (1e-14, 25.85e-3);
    let wanted = is / nvt * (0.6f64 / nvt).exp();
    let actual = Partial::output("DiodeConductanceBench");
    assert!(Partial::within(wanted, actual, 1e-9), "g = {actual:e}, closed form {wanted:e}");
}

#[test]
fn ddx_differentiates_through_a_fn_and_a_fn_it_calls() {
    // AO-01 AC 1 (§14.2.3, the derivative is symbolic over pure functions): `shockley` (a `let` and a
    // value) plus `twice` (2 * a call of `shockley`) is three junctions: three times the conductance.
    let (is, nvt) = (1e-14, 25.85e-3);
    let wanted = 3.0 * is / nvt * (0.6f64 / nvt).exp();
    let actual = Partial::output("FnConductanceBench");
    assert!(Partial::within(wanted, actual, 1e-9), "g = {actual:e}, closed form {wanted:e}");
}

#[test]
fn ddx_of_two_calls_of_one_fn_reads_each_calls_arguments() {
    // `shockley(v, is) + shockley(v, 5 is)`: the second call binds the parameters again, so the
    // derivative is six times `is / nvt * exp(0.6 / nvt)`, not twice.
    let (is, nvt) = (1e-14, 25.85e-3);
    let wanted = 6.0 * is / nvt * (0.6f64 / nvt).exp();
    let actual = Partial::output("FnTwiceConductanceBench");
    assert!(Partial::within(wanted, actual, 1e-9), "g = {actual:e}, closed form {wanted:e}");
}

#[test]
fn ddx_against_the_flow_of_a_branch_with_no_potential_contribution_is_zero() {
    // §14.2.3: the flow of a branch the module only gives a flow contribution is no unknown, so the
    // expression is held constant against it: `ddx(3 v(a), i(a))` is 0.
    assert!(Partial::output("FlowOfAFlowBranchBench").abs() <= 1e-12);
}

#[test]
fn ddx_against_the_reversed_branch_is_minus_one() {
    // AO-01 AC 2: `ddx(v(k, a), v(a, k))`: the reversed branch contributes -1.
    assert!(Partial::within(-1.0, Partial::output("ReversedBranchBench"), 1e-12));
}

#[test]
fn ddx_of_single_node_probes_against_a_branch_is_zero() {
    // AO-01 AC 3: `ddx(v(g) - v(s), v(g, s))` is 0, the two single-node probes being other branches
    // (§14.2.3's resolution).
    assert!(Partial::output("OtherBranchesBench").abs() <= 1e-12);
}

#[test]
fn ddx_against_a_flow_probe_gives_its_factor() {
    // AO-01 AC 4: `ddx(x * i(a, b), i(a, b))` is `x` = 3.
    assert!(Partial::within(3.0, Partial::output("FlowFactorBench"), 1e-12));
}

#[test]
fn ddx_differentiates_through_limit() {
    // AO-01 AC 5 (§18 rule 3): `ddx(limit(v(a), f) * 2.0, v(a))` is 2: `limit` is transparent.
    assert!(Partial::within(2.0, Partial::output("ThroughLimitBench"), 1e-12));
}

#[test]
fn ddx_of_an_expression_that_reads_no_probe_is_zero() {
    // `ddx(k * 2.0, v(a))`: the expression does not move with the probe.
    assert!(Partial::output("NoProbeBench").abs() <= 1e-12);
}

#[test]
fn an_ac_sweep_takes_ddx_at_the_operating_point() {
    // AO-01 AC 9: `i(o) +> ddx(id, v(a, k)) * v(in)` into 1 kohm, `in` a unit stimulus: |v(o)| is
    // the operating point's `ddx` (the conductance) times the load, whatever the frequency.
    let (is, nvt) = (1e-14, 25.85e-3);
    let g = is / nvt * (0.6f64 / nvt).exp();
    let mut running = Partial::project().compile("ConductanceAcBench").unwrap_or_else(|r| panic!("{r}"));
    let o = NodeIdentifier::Anonymous(running.ir.node_at("ConductanceAcBench.o").expect("node o").id().index());
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e5, steps: 3, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    assert_eq!(sweep.iter().count(), 3);
    for step in sweep.iter() {
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("ddx ac", "|v(o)|", Coordinate::Frequency(step.frequency)), g * 1e3, step.get_node(&o).expect("v(o)").norm());
    }
}

#[test]
fn an_ac_stim_read_by_a_ddx_is_a_constant_of_the_derivative() {
    // §14.3.1, §14.2.3: `ddx(ac_stim(2) * x + 3 x, x)` is `ac_stim(2) + 3`: the stimulus is no function of the probe, so its
    // derivative is 0 and the product rule leaves `ac_stim`. The operating point reads 3 (the stimulus is 0 there),
    // the sweep reads the stimulus 2 at the output, at every frequency.
    assert!(Partial::within(3.0, Partial::output("StimulusDdxBench"), 1e-12));
    let mut running = Partial::project().compile("StimulusDdxBench").unwrap_or_else(|r| panic!("{r}"));
    let o = NodeIdentifier::Anonymous(running.ir.node_at("StimulusDdxBench.o").expect("node o").id().index());
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e5, steps: 3, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    for step in sweep.iter() {
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("ddx ac_stim", "|v(o)|", Coordinate::Frequency(step.frequency)), 2.0, step.get_node(&o).expect("v(o)").norm());
    }
}

#[test]
fn ddx_of_an_ac_stim_alone_is_zero_and_the_site_still_runs() {
    // §14.2.3: the derivative of the stimulus against a probe is 0 (it reads no probe), at the operating
    // point and in the sweep, where the stimulus site is still evaluated without a statement that reads it.
    assert!(Partial::output("StimulusAloneDdxBench").abs() <= 1e-12);
    let mut running = Partial::project().compile("StimulusAloneDdxBench").unwrap_or_else(|r| panic!("{r}"));
    let o = NodeIdentifier::Anonymous(running.ir.node_at("StimulusAloneDdxBench.o").expect("node o").id().index());
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e3, steps: 1, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    for step in sweep.iter() {
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("ddx of ac_stim", "|v(o)|", Coordinate::Frequency(step.frequency)), 0.0, step.get_node(&o).expect("v(o)").norm());
    }
}

#[test]
fn an_ac_stim_read_by_a_ddt_state_compiles_and_reads_zero_at_the_operating_point() {
    // §14.3.1: a derivative site's state is a value input that only `analog` reads: `ddt(ac_stim(2) + v(a))` runs, and
    // is 0 at the operating point (the stimulus is 0 there and `v(a)` is held).
    assert!(Partial::output("StimulusStateBench").abs() <= 1e-12);
}

#[test]
fn ddx_against_a_flow_with_no_potential_contribution_reads_the_expression() {
    // AO-01 AC 11 (§14.2.3): a probe of the same flow contributes 1, so `ddx(5 i(a), i(a))` is 5 of a flow
    // branch that is no unknown, -5 against the reversed branch, and still 5 when no contribution of the
    // module reaches the branch: the derivative is the expression's, never a literal.
    assert!(Partial::within(5.0, Partial::output("FlowLeafFactorBench"), 1e-12));
    assert!(Partial::within(-5.0, Partial::output("FlowLeafReversedBench"), 1e-12));
    assert!(Partial::within(5.0, Partial::output("FlowLeafAloneBench"), 1e-12));
}
