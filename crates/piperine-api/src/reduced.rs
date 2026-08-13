//! Host-side reductions for reduced constraint quantities (DVC-09, D7):
//! the reduced helpers declared in `headers/constraints.phdl`
//! (`ac_unity_gain_freq`, `ac_phase_margin`, `ac_gain_margin`) exist once
//! per completed analysis — no per-point kernel can compute them (design
//! §C2), so the solver stays pointwise-pure and the host applies each
//! declared reduction over the points the solver already returns.
//!
//! **MD-45 regression**: the declaration vehicle — a PHDL/dv-core
//! `constraint` block naming the reduced quantity — was gutted with the
//! PHDL AST at T21 (`Module::constraints()`, `ConstraintStmt`,
//! `QuantityClass` no longer exist; VAMS Annex A has no equivalent
//! production). [`ReducedEvaluation::evaluate`] therefore always returns
//! an empty list — there is no constraint block left to walk. The math
//! this module used to apply (`ac_unity_gain_freq`/`ac_phase_margin`/
//! `ac_gain_margin` over an AC phasor sweep, plus the log-log bracket/
//! interpolation helpers and the constraint-expression evaluator they sat
//! on) is deleted here rather than kept as unreachable dead code — a
//! future task rebuilding the constraint-declaration surface on VAMS
//! `(* attribute *)` syntax (`.specs/STATE.md` MD-45) is expected to
//! reintroduce the reduction math alongside a real walk.

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::pom::Design;
use piperine_solver::prelude::AcAnalysisResult;

use crate::error::Error;
use crate::margins::MarginEntry;

/// The reduction engine: walks every instance's constraint block on the
/// compiled design and evaluates each reduced statement over one AC
/// sweep's points. No state escapes an [`evaluate`](Self::evaluate) call
/// — sweep-shaped state never enters the solver (D7), and none lingers
/// here either.
pub struct ReducedEvaluation<'a> {
    design: &'a Design,
    info: &'a CircuitBuildInfo,
}

impl<'a> ReducedEvaluation<'a> {
    /// The reduction engine over one compiled design + build info. Public
    /// so hosts driving a `CircuitInstance` directly (the Python live
    /// session) run the identical reductions the session does (D7).
    pub fn new(design: &'a Design, info: &'a CircuitBuildInfo) -> Self {
        Self { design, info }
    }

    /// Evaluate every reduced `var`/`target` declared on every instance's
    /// module over the sweep's points, in catalog order.
    ///
    /// Always returns an empty list post-MD-45 — see the module doc.
    pub fn evaluate(&self, _result: &AcAnalysisResult) -> Result<Vec<MarginEntry>, Error> {
        let _ = (self.design, self.info);
        Ok(Vec::new())
    }
}
