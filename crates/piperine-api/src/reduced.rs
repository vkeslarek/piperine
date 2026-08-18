//! Host-side reductions for reduced constraint quantities (DVC-09, D7):
//! the reduced helpers declared in a `constraint begin` block
//! (`ac_unity_gain_freq`, `ac_phase_margin`, `ac_gain_margin`) exist once
//! per completed analysis — no per-point kernel can compute them (design
//! §C2), so the solver stays pointwise-pure and the host applies each
//! declared reduction over the points the solver already returns.

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::pom::Design;
use piperine_lang::pom::PomConstraintStmt;
use piperine_solver::prelude::{AcAnalysisResult, MarginKind, NodeIdentifier};
use piperine_solver::abi::AnalysisKind;

use crate::error::Error;
use crate::margins::{MarginEntry, MarginValue};

/// The reduced-helper names — the classification rule shared with the
/// codegen lowering (`resolve/pom/constraints.rs::is_reduced`): an
/// expression containing one of these calls is a reduced quantity.
pub(crate) const REDUCED_HELPERS: &[&str] = &["ac_unity_gain_freq", "ac_phase_margin", "ac_gain_margin"];

/// Whether a constraint statement's expressions involve a reduced helper
/// — the `ConstraintCatalog`'s half: such a statement exists once per
/// completed AC sweep, so a cross-analysis read is "not defined in this
/// analysis", never "unknown constraint".
pub(crate) fn is_reduced_stmt(stmt: &PomConstraintStmt) -> bool {
    fn contains(expr: &piperine_lang::ast::Expr) -> bool {
        use piperine_lang::ast::Expr;
        match expr {
            Expr::Call { name, args, .. } => {
                REDUCED_HELPERS.contains(&name.as_str()) || args.iter().any(contains)
            }
            Expr::Unary { operand, .. } => contains(operand),
            Expr::Binary { lhs, rhs, .. } => contains(lhs) || contains(rhs),
            Expr::Ternary { cond, then_branch, else_branch, .. } => {
                contains(cond) || contains(then_branch) || contains(else_branch)
            }
            _ => false,
        }
    }
    match stmt {
        PomConstraintStmt::Var { expr, .. } => contains(expr),
        PomConstraintStmt::Target { expr, level, tol, .. } => {
            contains(expr) || contains(level) || tol.as_ref().is_some_and(contains)
        }
        _ => false,
    }
}

/// Whether `expr` mentions a reduced quantity — a helper call, or a name
/// bound by an earlier reduced `var` (the through-the-binding half of
/// the classification: the codegen lowering sees the helper by
/// substituting the binding, the host by the name, and both must agree
/// on which statements are reduced or a target would be kept out of the
/// kernel yet never evaluated).
fn mentions_reduced(expr: &piperine_lang::ast::Expr, reduced: &[String]) -> bool {
    use piperine_lang::ast::Expr;
    match expr {
        Expr::Call { name, args, .. } => {
            REDUCED_HELPERS.contains(&name.as_str()) || args.iter().any(|a| mentions_reduced(a, reduced))
        }
        Expr::Ident { name, .. } => reduced.iter().any(|n| n == name),
        Expr::Unary { operand, .. } => mentions_reduced(operand, reduced),
        Expr::Binary { lhs, rhs, .. } => mentions_reduced(lhs, reduced) || mentions_reduced(rhs, reduced),
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            mentions_reduced(cond, reduced)
                || mentions_reduced(then_branch, reduced)
                || mentions_reduced(else_branch, reduced)
        }
        _ => false,
    }
}

/// The reduction engine: walks every instance's constraint statements on
/// the compiled design and evaluates each reduced `var`/`target` over one
/// AC sweep's points. No state escapes an [`evaluate`](Self::evaluate)
/// call — sweep-shaped state never enters the solver (D7), and none
/// lingers here either.
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
    /// module over the sweep's points, in catalog order (`target#<n>` is
    /// the statement index across all of the module's constraint
    /// statements, matching the lowering's naming).
    pub fn evaluate(&self, result: &AcAnalysisResult) -> Result<Vec<MarginEntry>, Error> {
        let _ = AnalysisKind::Ac;
        let mut entries = Vec::new();
        for instance in &self.info.instances {
            let Some(module) = self.design.module(&instance.module) else { continue };
            let mut env: Vec<(String, f64)> = Vec::new();
            let mut reduced: Vec<String> = Vec::new();
            for (index, stmt) in module.constraints().iter().enumerate() {
                match stmt {
                    PomConstraintStmt::Var { name, expr, .. } if mentions_reduced(expr, &reduced) => {
                        let v = self.scalar(expr, &env, instance, result)?;
                        env.push((name.clone(), v));
                        reduced.push(name.clone());
                        entries.push(MarginEntry {
                            label: name.clone(),
                            instance_path: instance.label.clone(),
                            kind: MarginKind::Var,
                            value: MarginValue::Reduced { value: v },
                        });
                    }
                    PomConstraintStmt::Target { expr, cmp, level, tol, .. }
                        if mentions_reduced(expr, &reduced)
                            || mentions_reduced(level, &reduced)
                            || tol.as_ref().is_some_and(|t| mentions_reduced(t, &reduced)) =>
                    {
                        let margin = self.target_margin(
                            expr,
                            *cmp,
                            level,
                            tol.as_ref(),
                            &env,
                            instance,
                            result,
                        )?;
                        entries.push(MarginEntry {
                            label: format!("target#{index}"),
                            instance_path: instance.label.clone(),
                            kind: MarginKind::Target,
                            value: MarginValue::Reduced { value: margin },
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(entries)
    }

    /// A reduced target's signed margin: `expr <= level` →
    /// `(level − expr)/scale`, `expr >= level` → `(expr − level)/scale`,
    /// `scale = 1` absent a `tol` (the same convention the pointwise
    /// lowering emits, DVC-08).
    fn target_margin(
        &self,
        expr: &piperine_lang::ast::Expr,
        cmp: piperine_lang::ast::TargetCmp,
        level: &piperine_lang::ast::Expr,
        tol: Option<&piperine_lang::ast::Expr>,
        env: &[(String, f64)],
        instance: &piperine_codegen::device::BuiltInstanceInfo,
        result: &AcAnalysisResult,
    ) -> Result<f64, Error> {
        let value = self.scalar(expr, env, instance, result)?;
        let level = self.constant(level, env)?;
        let scale = match tol {
            None => 1.0,
            Some(scale) => self.constant(scale, env)?,
        };
        let diff = match cmp {
            piperine_lang::ast::TargetCmp::Le => level - value,
            piperine_lang::ast::TargetCmp::Ge => value - level,
        };
        Ok(diff / scale)
    }

    /// A target's level/scale must be an elaboration constant the host can
    /// read directly — a literal, or a reduced `var` already evaluated.
    /// Anything else (a param reference, a probe) fails loud rather than
    /// silently reading a stale or zero value.
    fn constant(
        &self,
        expr: &piperine_lang::ast::Expr,
        env: &[(String, f64)],
    ) -> Result<f64, Error> {
        use piperine_lang::ast::{Expr, UnOp};
        match expr {
            Expr::Number { value, .. } => Ok(number_to_f64(value)),
            Expr::Ident { name, .. } => env
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .ok_or_else(|| Error::Measurement(format!(
                    "reduced target constant `{name}` is not a literal or an evaluated reduced var"
                ))),
            Expr::Unary { op: UnOp::Neg, operand, .. } => Ok(-self.constant(operand, env)?),
            Expr::Unary { op: UnOp::Plus, operand, .. } => self.constant(operand, env),
            Expr::Binary { op, lhs, rhs, .. } => {
                let l = self.constant(lhs, env)?;
                let r = self.constant(rhs, env)?;
                Self::arith(l, *op, r)
            }
            other => Err(Error::Measurement(format!(
                "unsupported reduced target constant `{other:?}` — a literal or reduced var is required"
            ))),
        }
    }

    /// Evaluate a reduced scalar expression: literals, earlier reduced
    /// `var`s, real arithmetic, and the declared reduced helpers over a
    /// probe. Anything outside that vocabulary fails loud.
    fn scalar(
        &self,
        expr: &piperine_lang::ast::Expr,
        env: &[(String, f64)],
        instance: &piperine_codegen::device::BuiltInstanceInfo,
        result: &AcAnalysisResult,
    ) -> Result<f64, Error> {
        use piperine_lang::ast::{Expr, UnOp};
        match expr {
            Expr::Number { value, .. } => Ok(number_to_f64(value)),
            Expr::Ident { name, .. } => env
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .ok_or_else(|| Error::Measurement(format!(
                    "unknown reduced quantity `{name}` — reduced vars evaluate in authored order"
                ))),
            Expr::Unary { op: UnOp::Neg, operand, .. } => {
                Ok(-self.scalar(operand, env, instance, result)?)
            }
            Expr::Unary { op: UnOp::Plus, operand, .. } => self.scalar(operand, env, instance, result),
            Expr::Binary { op, lhs, rhs, .. } => {
                let l = self.scalar(lhs, env, instance, result)?;
                let r = self.scalar(rhs, env, instance, result)?;
                Self::arith(l, *op, r)
            }
            Expr::Call { name, args, .. } => {
                let [arg] = args.as_slice() else {
                    return Err(Error::Measurement(format!(
                        "reduced helper `{name}` takes exactly one probe argument"
                    )));
                };
                let probe = self.probe(arg, instance, result)?;
                Self::reduce(name, &probe)
            }
            other => Err(Error::Measurement(format!(
                "unsupported reduced expression `{other:?}` — the host reduction handles \
                 literals, reduced vars, arithmetic, and helper(probe) calls"
            ))),
        }
    }

    /// The one reduction per declared helper (D7): each reads the probe's
    /// phasor series over the sweep.
    fn reduce(helper: &str, pts: &[(f64, num_complex::Complex64)]) -> Result<f64, Error> {
        match helper {
            "ac_unity_gain_freq" => Self::unity_gain_freq(pts),
            "ac_phase_margin" => Self::phase_margin(pts),
            "ac_gain_margin" => Self::gain_margin(pts),
            other => Err(Error::Measurement(format!(
                "no host reduction for `{other}` — the reduced helpers are \
                 ac_unity_gain_freq, ac_phase_margin, ac_gain_margin"
            ))),
        }
    }

    /// The frequency at which the probe's magnitude crosses unity:
    /// log-log interpolation between the bracketing points (a power-law
    /// magnitude between adjacent sweep points). Too few points fails
    /// loud rather than extrapolating; no crossing fails loud rather than
    /// returning a plausible wrong number.
    fn unity_gain_freq(pts: &[(f64, num_complex::Complex64)]) -> Result<f64, Error> {
        if pts.len() < 2 {
            return Err(Error::Measurement(
                "ac_unity_gain_freq needs at least two sweep points — it will not extrapolate".into(),
            ));
        }
        Self::bracket(pts, |w| w.norm() - 1.0).ok_or_else(|| {
            Error::Measurement(
                "ac_unity_gain_freq: the probe's magnitude never crosses unity over the sweep".into(),
            )
        })
    }

    /// The loop's phase margin: 180° + the probe's phase (degrees) at the
    /// unity-gain crossing, the phase linearly interpolated in log
    /// frequency between the bracketing points.
    fn phase_margin(pts: &[(f64, num_complex::Complex64)]) -> Result<f64, Error> {
        if pts.len() < 2 {
            return Err(Error::Measurement(
                "ac_phase_margin needs at least two sweep points — it will not extrapolate".into(),
            ));
        }
        let f_u = Self::unity_gain_freq(pts)?;
        let phase = Self::interp_log(pts, f_u, |w| w.arg().to_degrees());
        Ok(180.0 + phase)
    }

    /// The loop's gain margin: −20·log10 of the probe's magnitude at the
    /// −180° phase crossing. A probe whose phase never reaches −180° has
    /// infinite gain margin (the standard convention — not a silent 0).
    fn gain_margin(pts: &[(f64, num_complex::Complex64)]) -> Result<f64, Error> {
        if pts.len() < 2 {
            return Err(Error::Measurement(
                "ac_gain_margin needs at least two sweep points — it will not extrapolate".into(),
            ));
        }
        let mut prev: Option<(f64, f64)> = None;
        for &(f, w) in pts {
            let phase = w.arg().to_degrees();
            if let Some((f0, p0)) = prev
                && (p0 + 180.0) * (phase + 180.0) <= 0.0
                && p0 != phase
            {
                let t = (-180.0 - p0) / (phase - p0);
                let f_pi = (f0.ln() + t * (f.ln() - f0.ln())).exp();
                let mag = Self::interp_log(pts, f_pi, |w| w.norm());
                return Ok(-20.0 * mag.log10());
            }
            prev = Some((f, phase));
        }
        Ok(f64::INFINITY)
    }

    /// The first point pair whose `measure` brackets zero (either
    /// direction), with the crossing position log-log interpolated —
    /// shared by the unity-gain searches.
    fn bracket(
        pts: &[(f64, num_complex::Complex64)],
        measure: impl Fn(num_complex::Complex64) -> f64,
    ) -> Option<f64> {
        if pts.len() < 2 {
            return None;
        }
        let mut prev: Option<(f64, f64)> = None;
        for &(f, w) in pts {
            let m = measure(w);
            if let Some((f0, m0)) = prev
                && m0 * m <= 0.0
                && m0 != m
            {
                // log|w| linear in log f between the bracketing points.
                let t = (0.0 - m0) / (m - m0);
                return Some((f0.ln() + t * (f.ln() - f0.ln())).exp());
            }
            prev = Some((f, m));
        }
        None
    }

    /// Linear interpolation of `measure` in log frequency at `f_at`.
    fn interp_log(
        pts: &[(f64, num_complex::Complex64)],
        f_at: f64,
        measure: impl Fn(num_complex::Complex64) -> f64,
    ) -> f64 {
        let mut prev: Option<(f64, f64)> = None;
        for &(f, w) in pts {
            let m = measure(w);
            if let Some((f0, m0)) = prev
                && f_at <= f
            {
                let t = (f_at.ln() - f0.ln()) / (f.ln() - f0.ln());
                return m0 + t * (m - m0);
            }
            prev = Some((f, m));
        }
        measure(pts[pts.len() - 1].1)
    }

    /// The probe's phasor series over the sweep: `V(a, b)` — each end a
    /// port of the instance's module (anything deeper crosses encapsulation
    /// and fails loud, mirroring the elaborator's internal-node rule).
    fn probe(
        &self,
        expr: &piperine_lang::ast::Expr,
        instance: &piperine_codegen::device::BuiltInstanceInfo,
        result: &AcAnalysisResult,
    ) -> Result<Vec<(f64, num_complex::Complex64)>, Error> {
        use piperine_lang::ast::Expr;
        let Expr::AccessFunction { function, pos, neg, .. } = expr else {
            return Err(Error::Measurement(
                "a reduced helper's argument must be a probe (`V(a, b)`)".into(),
            ));
        };
        if function != "V" {
            return Err(Error::Measurement(format!(
                "unsupported probe `{function}` — the host reduction reads `V(a, b)` probes"
            )));
        }
        let node_a = self.probe_node(pos, instance)?;
        let node_b = match neg.as_deref() {
            None => Ok(NodeIdentifier::Gnd),
            Some(e) => self.probe_node(e, instance),
        }?;
        let zero = num_complex::Complex64::default();
        let pts = result
            .iter()
            .map(|step| {
                let va = if node_a == NodeIdentifier::Gnd { zero } else { step.get_node(&node_a).copied().unwrap_or(zero) };
                let vb = if node_b == NodeIdentifier::Gnd { zero } else { step.get_node(&node_b).copied().unwrap_or(zero) };
                (step.frequency, va - vb)
            })
            .collect();
        Ok(pts)
    }

    /// One probe end: a ground name maps to the reference node; a port
    /// name maps through the instance's terminals (declaration order,
    /// matching the kernel's terminal order); anything else — an internal
    /// net of the module — crosses encapsulation and fails loud.
    fn probe_node(
        &self,
        expr: &piperine_lang::ast::Expr,
        instance: &piperine_codegen::device::BuiltInstanceInfo,
    ) -> Result<NodeIdentifier, Error> {
        use piperine_lang::ast::Expr;
        let Expr::Ident { name, .. } = expr else {
            return Err(Error::Measurement("probe ends must be net names".into()));
        };
        if piperine_lang::pom::is_ground(name) {
            return Ok(NodeIdentifier::Gnd);
        }
        let module = self.design.module(&instance.module).ok_or_else(|| {
            Error::Measurement(format!("module `{}` not found for reduced evaluation", instance.module))
        })?;
        let position = module
            .ports()
            .iter()
            .position(|p| p.name == *name)
            .ok_or_else(|| Error::Measurement(format!(
                "probe end `{name}` is not a port of `{}` — an internal net crosses \
                 encapsulation and cannot be probed from the host",
                instance.module
            )))?;
        instance.terminals.get(position).cloned().ok_or_else(|| Error::Measurement(format!(
            "instance `{}` has no terminal at port position {position}",
            instance.label
        )))
    }

    fn arith(l: f64, op: piperine_lang::ast::BinOp, r: f64) -> Result<f64, Error> {
        use piperine_lang::ast::BinOp;
        match op {
            BinOp::Add => Ok(l + r),
            BinOp::Sub => Ok(l - r),
            BinOp::Mul => Ok(l * r),
            BinOp::Div => Ok(l / r),
            other => Err(Error::Measurement(format!(
                "unsupported operator `{other:?}` in a reduced expression — arithmetic only"
            ))),
        }
    }
}

/// A number literal's real value (real or integer — the reduction math is
/// real-valued throughout).
fn number_to_f64(value: &piperine_lang::lexer::NumberLit) -> f64 {
    use piperine_lang::lexer::NumberLit;
    match value {
        NumberLit::Real(v) => *v,
        NumberLit::Integer(v) => *v as f64,
        // A based literal in a reduced expression is unusual but legal —
        // fold it through the codegen const-eval's own rule.
        NumberLit::Based(_) => piperine_codegen::resolve::pom_eval_const(
            &piperine_lang::ast::Expr::Number {
                span: piperine_lang::error::PreprocessedSpan::new(0, 0),
                value: value.clone(),
            },
            &|_| None,
        )
        .unwrap_or(0.0),
    }
}
