//! Margin-gradient differentiability (dv-gradients T10, DVG-11/12): how
//! (or whether) a declared [`crate::margins::MarginEntry`]'s gradient can
//! be computed. The loud refusals are the heart of this module — a metric
//! the driver cannot differentiate through the solver fails loud with the
//! reason, never a silent finite-difference substitution.
//!
//! Three refusal shapes (DVG-11/12):
//!
//! - **AC-defined** metrics (`ac_gain`, `ac_phase`, `ac_unity_gain_freq`,
//!   `ac_phase_margin`, `ac_gain_margin`, …) — the AC adjoint is a
//!   documented gap (D6). The host may finite-difference explicitly, but
//!   the driver never substitutes silently.
//! - **Event-detected** metrics (settling time, overshoot instant) —
//!   differentiable only through the event, which has no adjoint today.
//! - **Non-differentiable reductions** — a reduced metric whose defining
//!   reduction is not differentiable through the implicit function
//!   theorem in this delivery.
//!
//! The one differentiable case today is a **pointwise DC** margin whose
//! expression reduces to a `V(a, b)` probe (the common `require head :
//! V(p, n) <= hi;` shape). The DC adjoint (DVG-08) computes its gradient
//! at the located argmin (the single DC operating point — `argmin = 0.0`,
//! the result states this rather than implying a sweep).
//!
//! # Host sensitivity surface (T11, DVG-13)
//!
//! [`Session::sensitivity`](crate::Session::sensitivity) and
//! [`Session::gradient`](crate::Session::gradient) expose the adjoint path
//! at the host boundary with engine attribution: each result names
//! [`SensitivityMethod::Adjoint`] (one transpose solve) or
//! [`SensitivityMethod::FiniteDifference`] (the `.sens` baseline) and
//! carries the per-element fallback list (DVG-07). A `metric` argument
//! resolves to either a solved analog net name (`"mid"` → `V(mid)`) or a
//! declared margin name (`"head"` → the `require`'s expression); anything
//! else fails loud listing the candidates.

use std::collections::{HashMap, HashSet};

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::pom::Design;
use piperine_solver::prelude::NodeIdentifier;

use crate::error::Error;

/// How (or whether) a declared margin's gradient can be computed.
#[derive(Debug, Clone)]
pub enum MarginDifferentiability {
    /// A pointwise DC margin whose expression reduces to a `V(a, b)`
    /// probe, differentiable at the DC argmin (the single DC operating
    /// point — `argmin = 0.0`).
    PointwiseDc {
        /// The instance label as the adjoint addresses it (the leaf of
        /// the authored instance path).
        label: String,
        /// The output node the margin's expression reads (`V(a)` or the
        /// `a` in `V(a, b)`).
        output: NodeIdentifier,
        /// The optional reference node (`b` in `V(a, b)`). `None` for a
        /// single-ended probe.
        output_ref: Option<NodeIdentifier>,
        /// The DC coordinate — the located argmin for a pointwise DC
        /// margin is the single DC operating point. The result states
        /// this rather than implying a sweep.
        argmin: f64,
    },
    /// The metric is AC-defined and the AC adjoint is a documented gap
    /// (D6). The driver refuses loud; the host may finite-difference
    /// explicitly, never silently.
    AcDefined,
    /// The metric is event-detected (settling time, overshoot instant) —
    /// differentiable only through the event, which has no adjoint today.
    EventDetected,
    /// A reduced margin whose defining reduction is not differentiable
    /// through the implicit function theorem in this delivery.
    NonDifferentiableReduction,
}

/// Classify one declared margin's gradient differentiability from the
/// authored design + build info. Walks the flat top module's instances
/// and constraint blocks for a leaf whose label (or `target#<index>`)
/// matches `margin_name`, then reads its `StmtClass` to decide.
///
/// The lookup mirrors [`MarginsResult::get`](crate::margins::MarginsResult::get)'s
/// shape: a bare name resolves when it is unique across instances;
/// otherwise the caller passes the qualified `"instance.label"` form.
///
/// **MD-45 regression**: this always fails loud. The dv-core `constraint`
/// block surface (`Module::constraints()`, `ConstraintStmt`,
/// `QuantityClass`/`StmtClass`) was gutted with the PHDL AST at T21 — no
/// VAMS Annex A production carries it, so there is no declared-margin
/// tree left to walk or classify. `dv-gradients`' margin-gradient surface
/// is dark until a future task rebuilds constraint declarations on VAMS
/// `(* attribute *)` syntax (`.specs/STATE.md` MD-45). Callers (e.g.
/// [`crate::session::entry::Session::gradient`]) already fall through to
/// the plain net-name path on `Err`, so this stub composes correctly with
/// existing call sites rather than requiring a call-site rewrite.
pub fn classify_margin_gradient(
    _design: &Design,
    _info: &CircuitBuildInfo,
    top: &str,
    margin_name: &str,
) -> Result<MarginDifferentiability, Error> {
    Err(Error::Measurement(format!(
        "margin gradient: no declared constraint named `{margin_name}` on top module `{top}` — \
         constraint-block declarations are unavailable post-MD-45 (dv-gradients regressed until \
         a future task rebuilds them on VAMS attribute syntax)"
    )))
}

/// The result of a margin gradient request: the located argmin, the
/// gradient values, and the engine attribution propagated from the
/// underlying adjoint.
#[derive(Debug, Clone)]
pub struct MarginGradientResult {
    /// The margin name this gradient is for.
    pub name: String,
    /// The located argmin (D1) — for a pointwise DC margin this is the
    /// single DC operating point (`0.0`). The result **states this**
    /// rather than implying a sweep.
    pub argmin: f64,
    /// `∂(output)/∂(label.param)` keyed by `"label.param"`. The values
    /// are the gradients of the underlying `V()` quantity the margin
    /// reads — the caller combines them with the margin's own
    /// `(level − expr)` sign convention if it needs the margin's signed
    /// gradient directly.
    pub d: std::collections::HashMap<String, f64>,
    /// Which elements used the stamp-perturbation fallback (DVG-07),
    /// propagated from the underlying adjoint.
    pub fallback_users: std::collections::HashSet<String>,
}

// ── host sensitivity surface (T11, DVG-13) ────────────────────────────────

/// How a host-requested sensitivity was computed (DVG-13). The result
/// names the engine so a reader never has to guess whether a number came
/// from the analytic adjoint (one transpose solve, exact on linear
/// circuits) or the central finite-difference baseline (`.sens`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitivityMethod {
    /// Analytic adjoint (DVG-08): one transpose solve of `Jᵀλ = ∂f/∂x`,
    /// combined with per-element `∂F/∂p`. Per-element stamps are analytic
    /// where the element declares `HAS_SENSITIVITY` (DVG-04/05) and
    /// stamp-perturbation (DVG-06) otherwise — see the result's
    /// [`Sensitivity::fallback_users`].
    Adjoint,
    /// Central finite difference (`.sens`, two re-solves per parameter) —
    /// the verifiable oracle the adjoint is cross-checked against (T9).
    FiniteDifference,
}

impl SensitivityMethod {
    /// The stable, host-visible name (Python and Rust agree — MD-22).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Adjoint => "adjoint",
            Self::FiniteDifference => "finite-difference",
        }
    }
}

impl std::fmt::Display for SensitivityMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One sensitivity value with engine attribution (DVG-13).
#[derive(Debug, Clone)]
pub struct Sensitivity {
    /// `∂(metric)/∂(label.param)` at the operating point.
    pub value: f64,
    /// The engine that produced the value — adjoint or finite-difference.
    pub method: SensitivityMethod,
    /// Elements that used the stamp-perturbation fallback (DVG-07).
    /// Empty for [`SensitivityMethod::FiniteDifference`] (no fallback
    /// path) and for an all-analytic adjoint.
    pub fallback_users: HashSet<String>,
}

/// A multi-parameter gradient with engine attribution (DVG-13) —
/// [`Session::gradient`](crate::Session::gradient)'s result type. One
/// engine for the whole call (the adjoint serves every parameter from
/// one transpose solve); the per-element fallback list is shared.
#[derive(Debug, Clone)]
pub struct Gradient {
    /// `∂(metric)/∂(label.param)` keyed by `"label.param"`.
    pub d: HashMap<String, f64>,
    /// The engine that produced the gradient.
    pub method: SensitivityMethod,
    /// Elements that used the stamp-perturbation fallback (DVG-07).
    pub fallback_users: HashSet<String>,
}

impl Gradient {
    /// `∂(metric)/∂(label.param)`, if computed.
    pub fn get(&self, label: &str, param: &str) -> Option<f64> {
        self.d.get(&format!("{label}.{param}")).copied()
    }
}

