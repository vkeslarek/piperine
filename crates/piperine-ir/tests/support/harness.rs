//! The differential harness (T14, LOV-11): runs one [`Fixture`] under two
//! [`Evaluator`]s and reports the first disagreeing observation by name.
//!
//! ## The second-evaluator judgment call
//!
//! `piperine-ir` ships exactly one evaluator so far: the reference
//! interpreter (`interpreter_eval::InterpreterEvaluator`, wrapping T13's
//! `piperine_ir::interp::Interpreter`). The JIT backend spec's AC6
//! ultimately compares against does not exist until Phase 4 (T18-22
//! build `piperine-codegen::backend::{analog,digital}`) — Phase 3 is
//! deliberately "the IR, its verifier, and its printer/parser", built and
//! proven BEFORE any backend (`ideal.md` §6, D6).
//!
//! Comparing the reference interpreter against a second call to itself
//! would prove nothing: two invocations of the same code can never
//! disagree, which is exactly the class of test the whole feature exists
//! to stop shipping (`ideal.md` §0 — "each test exercises one stage
//! against a hand-built fixture ... A test shaped like that can never
//! disagree with itself"). This module never does that.
//!
//! Instead this phase's second evaluator is `checked_eval::CheckedEvaluator`
//! — an INDEPENDENTLY-WRITTEN evaluator (dense-array value store instead
//! of `interp.rs`'s `HashMap`, per-op-category dispatch functions written
//! from the IR's own documentation rather than transcribed from
//! `interp.rs`) kept in `tests/`, never `src/`, so it can never be
//! mistaken for a shipped backend. It exists to prove the HARNESS
//! MECHANISM — fixture in, two evaluators, first-disagreement report out —
//! works end to end before there is a JIT to plug in. `T18` (Phase 4)
//! wires the real analog backend into `Evaluator` and re-runs the RC
//! fixture through this SAME `compare` function; that is where the
//! spec's literal AC6 claim (interpreter ≡ JIT) is finally earned. Noted
//! as a deviation in the batch report — this is not silently presented as
//! the JIT proof.

use super::fixture::Fixture;
use piperine_ir::interp::CellValue;

/// A running evaluator over a [`Fixture`] — an interpreter, a checked
/// evaluator, or (from T18 on) a JIT backend wrapper.
pub trait Evaluator {
    /// A short name for disagreement reports.
    fn name(&self) -> &'static str;
    /// Run the fixture's whole stimulus program, returning one value per
    /// `fixture.observe`, in the same order.
    fn run(&self, fixture: &Fixture) -> Vec<CellValue>;
}

/// The first point two evaluators disagreed on.
#[derive(Debug)]
pub struct Disagreement {
    pub label: &'static str,
    pub a_name: &'static str,
    pub a_value: CellValue,
    pub b_name: &'static str,
    pub b_value: CellValue,
}

impl std::fmt::Display for Disagreement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "disagreement at `{}`: {} = {:?}, {} = {:?}",
            self.label, self.a_name, self.a_value, self.b_name, self.b_value,
        )
    }
}

/// Run `fixture` under both evaluators and compare every declared
/// observation using the fixture's own tolerance. Returns the FIRST
/// disagreement found — a single named entity is enough to reproduce and
/// fix the bug (mirrors `Verifier::run`'s and `verify_derivatives`'s
/// "first offender" convention).
pub fn compare(fixture: &Fixture, a: &dyn Evaluator, b: &dyn Evaluator) -> Result<(), Disagreement> {
    let ra = a.run(fixture);
    let rb = b.run(fixture);
    assert_eq!(
        ra.len(),
        fixture.observe.len(),
        "{} produced the wrong number of observations for fixture `{}`",
        a.name(),
        fixture.name
    );
    assert_eq!(
        rb.len(),
        fixture.observe.len(),
        "{} produced the wrong number of observations for fixture `{}`",
        b.name(),
        fixture.name
    );
    for (i, obs) in fixture.observe.iter().enumerate() {
        if !agree(&ra[i], &rb[i], fixture.tolerance.abs) {
            return Err(Disagreement {
                label: obs.label,
                a_name: a.name(),
                a_value: ra[i].clone(),
                b_name: b.name(),
                b_value: rb[i].clone(),
            });
        }
    }
    Ok(())
}

fn agree(a: &CellValue, b: &CellValue, abs_tol: f64) -> bool {
    match (a, b) {
        (CellValue::Real(x), CellValue::Real(y)) => (x - y).abs() <= abs_tol,
        (CellValue::Bits(x), CellValue::Bits(y)) => x == y,
        (CellValue::Array(xs), CellValue::Array(ys)) => {
            xs.len() == ys.len() && xs.iter().zip(ys).all(|(x, y)| agree(x, y, abs_tol))
        }
        _ => false,
    }
}
