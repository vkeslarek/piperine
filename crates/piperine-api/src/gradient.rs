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
use piperine_lang::parse::ast::{BinaryOp, ConstraintStmt, Expr};
use piperine_lang::pom::{Design, QuantityClass};
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
pub fn classify_margin_gradient(
    design: &Design,
    info: &CircuitBuildInfo,
    top: &str,
    margin_name: &str,
) -> Result<MarginDifferentiability, Error> {
    let Some(flat) = design.flat_module(top) else {
        return Err(Error::Measurement(format!(
            "margin gradient: top module `{top}` not found"
        )));
    };

    // Collect every leaf matching `margin_name` (bare or qualified), then
    // resolve ambiguities exactly the way `MarginsResult::get` does — a
    // unique bare match wins; otherwise the qualified `"instance.label"`
    // form is required.
    let mut matches: Vec<(String, &ConstraintStmt, &piperine_lang::pom::StmtClass)> =
        Vec::new();
    for instance in flat.instances() {
        let Some(module) = design.module(instance.module_name()) else { continue };
        let analog_instance = info.instances.iter().find(|i| i.label == instance.name());
        for block in module.constraints() {
            for (leaf_index, (stmt, class)) in block.leaves().enumerate() {
                let label = match stmt {
                    ConstraintStmt::Require { name, .. } | ConstraintStmt::Var { name, .. } => {
                        name.clone()
                    }
                    ConstraintStmt::Target { .. } => format!("target#{leaf_index}"),
                    // `leaves()` never yields these (they classify to
                    // their own `StmtClassNode` kind, not `Leaf`) —
                    // exhaustiveness only.
                    ConstraintStmt::Event { .. } | ConstraintStmt::Cover { .. } | ConstraintStmt::CoverCross { .. } => {
                        continue
                    }
                };
                if label != margin_name
                    && format!("{}.{}", instance.name(), label) != margin_name
                {
                    continue;
                }
                // A constraint on a digital-only instance has no DC
                // gradient — the catalog (DVC-15) calls this out as
                // "margins are an analog notion". Surface it as
                // NonDifferentiableReduction so the refusal is loud
                // rather than a silent zero.
                if analog_instance.is_none() {
                    return Ok(MarginDifferentiability::NonDifferentiableReduction);
                }
                matches.push((instance.name().to_string(), stmt, class));
            }
        }
    }

    let (instance_name, stmt, class) = match matches.as_slice() {
        [] => return Err(Error::Measurement(format!(
            "margin gradient: no declared constraint named `{margin_name}` on top module `{top}`"
        ))),
        [one] => one,
        _ => return Err(Error::Measurement(format!(
            "margin gradient: constraint name `{margin_name}` is ambiguous across instances — \
             qualify it as `instance.{margin_name}`"
        ))),
    };
    let instance_name = instance_name.clone();

    // AC scoping wins over everything else: an AC-defined metric cannot
    // be differentiated without the AC adjoint (D6).
    if class.analyses.iter().any(|a| a == "ac") {
        return Ok(MarginDifferentiability::AcDefined);
    }
    // Transient scoping → event-detected. A settling-time / overshoot
    // metric lives here until a transient adjoint lands.
    if class.analyses.iter().any(|a| a == "tran") {
        return Ok(MarginDifferentiability::EventDetected);
    }
    // Reduced class without an AC analysis (e.g. a future DC reduction)
    // → still not differentiable through the IFT in this delivery.
    if class.class == QuantityClass::Reduced {
        return Ok(MarginDifferentiability::NonDifferentiableReduction);
    }

    // Pointwise DC. Extract a V() probe from the margin's expression and
    // resolve it against the instance's terminals. Anything more complex
    // than `V(...)` (or a `cmp V(...)`) is refused loud as a
    // non-differentiable expression — the surface grows as more shapes
    // are wired, never silently.
    let expr = margin_value_expr(stmt).ok_or_else(|| Error::Measurement(format!(
        "margin gradient: `{margin_name}` has no extractable expression to differentiate"
    )))?;
    let probe = extract_voltage_probe(expr)?;
    let analog = info
        .instances
        .iter()
        .find(|i| i.label == instance_name)
        .expect("analog presence was checked above");
    let output = resolve_probe_end(&probe.0, design, analog)?;
    let output_ref = probe
        .1
        .as_ref()
        .map(|n| resolve_probe_end(n, design, analog))
        .transpose()?;

    Ok(MarginDifferentiability::PointwiseDc {
        label: instance_name,
        output,
        output_ref,
        argmin: 0.0,
    })
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

// ── internal helpers ──────────────────────────────────────────────────────

/// For a `require name : pred;` the differentiable expression is `pred`'s
/// left or right side (the `V(...)` half of the comparison); for a `var`
/// it is the value; for a `target` it is `expr`.
fn margin_value_expr(stmt: &ConstraintStmt) -> Option<&Expr> {
    match stmt {
        ConstraintStmt::Require { pred, .. } => {
            // `pred` is `expr cmp level` or `level cmp expr` — pick the
            // side that is not a literal/ident (the probe half).
            if let Expr::Binary(l, op, r) = pred
                && matches!(op, BinaryOp::Le | BinaryOp::Ge | BinaryOp::Lt | BinaryOp::Gt)
            {
                return Some(if !is_literal_or_ident(l) { l } else { r });
            }
            // A bare predicate (no comparison) — the whole expression.
            Some(pred)
        }
        ConstraintStmt::Var { value, .. } => Some(value),
        ConstraintStmt::Target { expr, .. } => Some(expr),
        ConstraintStmt::Event { .. } | ConstraintStmt::Cover { .. } | ConstraintStmt::CoverCross { .. } => None,
    }
}

fn is_literal_or_ident(e: &Expr) -> bool {
    matches!(e, Expr::Literal(_) | Expr::Ident(_))
}

/// Extract a `V(a, b)` or `V(a)` probe from `expr`. Returns the two probe
/// ends (the second is `None` for a single-ended probe). Anything else
/// (a parameter reference, a complex expression, a different probe
/// function) is refused loud — the surface grows as more shapes land.
fn extract_voltage_probe(expr: &Expr) -> Result<(Expr, Option<Expr>), Error> {
    let inner = match expr {
        Expr::Binary(l, _, r) => {
            // The comparison's probe half — recurse into whichever side
            // carries the V() call.
            if let Ok(p) = extract_voltage_probe(l) {
                return Ok(p);
            }
            return extract_voltage_probe(r);
        }
        other => other,
    };
    let Expr::Call(callee, args) = inner else {
        return Err(Error::Measurement(format!(
            "margin gradient: expression `{expr:?}` is not a V() probe — only V(a) or V(a, b) is differentiable today"
        )));
    };
    let Expr::Ident(name) = callee.as_ref() else {
        return Err(Error::Measurement(format!(
            "margin gradient: expression `{expr:?}` is not a V() probe"
        )));
    };
    if name != "V" {
        return Err(Error::Measurement(format!(
            "margin gradient: probe `{name}()` is not V() — only V(a) or V(a, b) is differentiable today"
        )));
    }
    match args.as_slice() {
        [a] => Ok((a.clone(), None)),
        [a, b] => Ok((a.clone(), Some(b.clone()))),
        _ => Err(Error::Measurement(format!(
            "margin gradient: V() probe expects 1 or 2 arguments, got {}",
            args.len()
        ))),
    }
}

/// Resolve a probe end (`p`, `n`, `gnd`, …) against the instance's module
/// ports — exactly the mapping `reduced.rs`'s `probe_node` does. A ground
/// name resolves to the reference node; a port name maps through the
/// module's port declaration order to the instance's terminal
/// `NodeIdentifier`. Anything else fails loud.
fn resolve_probe_end(
    expr: &Expr,
    design: &Design,
    instance: &piperine_codegen::device::BuiltInstanceInfo,
) -> Result<NodeIdentifier, Error> {
    let Expr::Ident(name) = expr else {
        return Err(Error::Measurement(format!(
            "margin gradient: probe end `{expr:?}` must be a port name"
        )));
    };
    if piperine_lang::pom::is_ground(name) {
        return Ok(NodeIdentifier::Gnd);
    }
    let module = design.module(&instance.module).ok_or_else(|| {
        Error::Measurement(format!(
            "margin gradient: module `{}` not found for instance `{}`",
            instance.module, instance.label
        ))
    })?;
    let position = module.ports().iter().position(|p| p.name == *name).ok_or_else(|| {
        Error::Measurement(format!(
            "margin gradient: probe end `{name}` is not a port of `{}` — an internal net crosses encapsulation",
            instance.module
        ))
    })?;
    instance.terminals.get(position).cloned().ok_or_else(|| {
        Error::Measurement(format!(
            "margin gradient: instance `{}` has no terminal at port position {position}",
            instance.label
        ))
    })
}
