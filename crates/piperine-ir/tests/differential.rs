//! T14 — the differential harness (LOV-11): a fixture runs under TWO
//! evaluators and the harness reports the first disagreeing observation
//! by name.
//!
//! ## The second-evaluator judgment call (read `support/harness.rs` first)
//!
//! At this point in the plan (Phase 3, before Phase 4's backends exist)
//! there is no JIT to compare the reference interpreter against. This
//! target proves the HARNESS MECHANISM itself: it takes a fixture plus
//! two evaluators, and correctly (a) confirms agreement between two
//! independently-implemented evaluators on real fixtures, and (b) detects
//! and names a disagreement when one is injected. Neither test compares
//! the reference interpreter against a second call to itself — see
//! `support/harness.rs`'s module doc for the full reasoning and the
//! explicit note that this is a deviation from a literal JIT-vs-
//! interpreter proof, closed at T18.

mod support;

use piperine_ir::core::{
    Block, Cell, Const, Domain, EdgeKind, Module, Net, NatureInfo, NatureKind, Op, PureOp,
    SigRef, Trigger, Ty,
};
use piperine_ir::interp::CellValue;
use piperine_ir::verify::Verifier;
use support::checked_eval::CheckedEvaluator;
use support::fixture::{Fixture, Observation, ObservePoint, Step, Tolerance};
use support::harness::{compare, Evaluator};
use support::interpreter_eval::InterpreterEvaluator;

/// A tiny analog fixture: `y = (Va - Vb)^2` via `ReadAcross` + `Pure::Mul`.
/// Exercises the analog net-read path both evaluators share.
fn real_arithmetic_fixture() -> Fixture {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let a = m.push_net(Net::node(v, i));
    let b = m.push_net(Net::node(v, i));
    let x = m.push_value(Ty::Real);
    let y = m.push_value(Ty::Real);
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    m.block_at_mut(block).push(Op::ReadAcross { a, b, out: x });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Mul, args: vec![x, x], out: y });
    Verifier::new(&m).run().expect("fixture must verify");

    Fixture {
        name: "real_arithmetic",
        module: m,
        steps: vec![Step { time: None, node_values: vec![(a, 5.0, 0.0), (b, 2.0, 0.0)], ..Default::default() }],
        observe: vec![Observation { label: "y", point: ObservePoint::Value(y) }],
        tolerance: Tolerance { abs: 1e-9 },
    }
}

/// A tiny digital fixture: a 2-bit counter incremented on every rising
/// edge of an externally-driven clock value. Exercises Edge triggers,
/// CellRead/CellWrite, and Bits arithmetic (with width-masking wraparound)
/// both evaluators share.
fn digital_edge_fixture(cycles: u32) -> Fixture {
    let mut m = Module::new();
    let clk_v = m.push_value(Ty::Bits(1));
    let one = m.push_value(Ty::Bits(2));
    let init = m.push_const(Const::Bits(0));
    let block = m.push_block(Block::new(
        Domain::Digital,
        Trigger::Edge(SigRef(vec![piperine_ir::core::Chunk::Slice { value: clk_v, lo: 0, hi: 1 }]), EdgeKind::Rising),
    ));
    let count = m.push_cell(Cell::new(Ty::Bits(2), init, block));
    let old = m.push_value(Ty::Bits(2));
    let next = m.push_value(Ty::Bits(2));
    m.block_at_mut(block).push(Op::CellRead { cell: count, addr: None, out: old });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Add, args: vec![old, one], out: next });
    m.block_at_mut(block).push(Op::CellWrite { cell: count, addr: None, value: next });
    Verifier::new(&m).run().expect("fixture must verify");

    let mut steps = Vec::new();
    for cycle in 0..cycles {
        let _ = cycle;
        steps.push(Step {
            seed_values: vec![(one, CellValue::Bits(1))],
            signal_values: vec![],
            node_values: vec![],
            time: None,
        }
        .with_clk(clk_v, 0));
        steps.push(Step { seed_values: vec![(one, CellValue::Bits(1))], ..Default::default() }.with_clk(clk_v, 1));
    }

    Fixture {
        name: "digital_edge_counter",
        module: m,
        steps,
        observe: vec![Observation { label: "count", point: ObservePoint::Cell(count) }],
        tolerance: Tolerance::EXACT,
    }
}

// Small builder-style helper kept local to this test file (not shared
// support machinery — it just seeds `clk_v` into `seed_values`).
trait WithClk {
    fn with_clk(self, clk_v: piperine_ir::core::ValueId, bit: u128) -> Step;
}
impl WithClk for Step {
    fn with_clk(mut self, clk_v: piperine_ir::core::ValueId, bit: u128) -> Step {
        self.seed_values.push((clk_v, CellValue::Bits(bit)));
        self
    }
}

#[test]
fn harness_confirms_agreement_on_a_real_arithmetic_fixture() {
    let fixture = real_arithmetic_fixture();
    let result = compare(&fixture, &InterpreterEvaluator, &CheckedEvaluator);
    assert!(result.is_ok(), "the two evaluators must agree: {:?}", result.err());
}

#[test]
fn harness_confirms_agreement_on_a_digital_edge_fixture_over_multiple_cycles() {
    let fixture = digital_edge_fixture(5);
    let result = compare(&fixture, &InterpreterEvaluator, &CheckedEvaluator);
    assert!(result.is_ok(), "the two evaluators must agree: {:?}", result.err());
}

/// A hand-computed sanity check alongside the differential proof: 5 rising
/// edges of a 2-bit counter wrap once (0,1,2,3,0,1) — final value 1.
#[test]
fn digital_edge_fixture_hand_computed_value_is_one_after_five_edges() {
    let fixture = digital_edge_fixture(5);
    let observed = InterpreterEvaluator.run(&fixture);
    assert_eq!(observed, vec![CellValue::Bits(1)]);
}

/// MD-31: the harness must be able to fail, and name what disagreed. A
/// deliberately-wrong evaluator (always reports zero) proves it.
struct AlwaysZeroEvaluator;
impl Evaluator for AlwaysZeroEvaluator {
    fn name(&self) -> &'static str {
        "always-zero (deliberately wrong, proves the harness can fail)"
    }
    fn run(&self, fixture: &Fixture) -> Vec<CellValue> {
        fixture
            .observe
            .iter()
            .map(|_| CellValue::Real(0.0))
            .collect()
    }
}

#[test]
fn harness_detects_and_names_a_disagreement() {
    let fixture = real_arithmetic_fixture(); // y = 9.0, not 0.0
    let err = compare(&fixture, &InterpreterEvaluator, &AlwaysZeroEvaluator)
        .expect_err("the always-zero evaluator must disagree with the real one");
    assert_eq!(err.label, "y");
    assert_eq!(err.a_value, CellValue::Real(9.0));
    assert_eq!(err.b_value, CellValue::Real(0.0));
}

/// The declared tolerance is honoured both ways: a disagreement within
/// tolerance passes, the same disagreement outside tolerance fails.
struct PerturbedEvaluator(f64);
impl Evaluator for PerturbedEvaluator {
    fn name(&self) -> &'static str {
        "perturbed reference interpreter"
    }
    fn run(&self, fixture: &Fixture) -> Vec<CellValue> {
        InterpreterEvaluator
            .run(fixture)
            .into_iter()
            .map(|v| match v {
                CellValue::Real(x) => CellValue::Real(x + self.0),
                other => other,
            })
            .collect()
    }
}

#[test]
fn declared_tolerance_absorbs_a_small_perturbation_but_not_a_large_one() {
    let mut within = real_arithmetic_fixture();
    within.tolerance = Tolerance { abs: 0.01 };
    let ok = compare(&within, &InterpreterEvaluator, &PerturbedEvaluator(0.001));
    assert!(ok.is_ok(), "a perturbation within tolerance must agree: {:?}", ok.err());

    let mut outside = real_arithmetic_fixture();
    outside.tolerance = Tolerance { abs: 0.01 };
    let err = compare(&outside, &InterpreterEvaluator, &PerturbedEvaluator(1.0))
        .expect_err("a perturbation outside tolerance must disagree");
    assert_eq!(err.label, "y");
}
