//! Host-side reductions for reduced constraint quantities (DVC-09, D7):
//! the reduced helpers declared in `headers/constraints.phdl`
//! (`ac_unity_gain_freq`, `ac_phase_margin`, `ac_gain_margin`) exist once
//! per completed analysis — no per-point kernel can compute them (design
//! §C2), so the solver stays pointwise-pure and the host applies each
//! declared reduction over the points the solver already returns. The
//! declaration lives in PHDL (the constraint block names the quantity);
//! its computation is one pass over the AC sweep's complex points.
//!
//! Every reduced `var`/`target` of every instance's constraint block is
//! evaluated here, producing the [`MarginValue::Reduced`] entries of the
//! margins channel — one value, **no** argmin, never an invented `t = 0`.

use num_complex::Complex64;
use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::parse::ast::{BinaryOp, ConstraintStmt, Expr, Literal, TargetCmp, UnaryOp};
use piperine_lang::pom::{Design, QuantityClass};
use piperine_solver::prelude::{AcAnalysisResult, MarginKind, NodeIdentifier};

use crate::error::Error;
use crate::margins::{MarginEntry, MarginValue};

/// The evaluation scope one reduced statement reads: the `var`s evaluated
/// before it (in authored order), the instance its module was compiled
/// from, and the sweep being reduced.
struct EvalScope<'a> {
    env: &'a [(String, f64)],
    instance: &'a piperine_codegen::device::BuiltInstanceInfo,
    result: &'a AcAnalysisResult,
}

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
    pub fn evaluate(&self, result: &AcAnalysisResult) -> Result<Vec<MarginEntry>, Error> {
        let mut entries = Vec::new();
        for instance in &self.info.instances {
            let Some(module) = self.design.module(&instance.module) else { continue };
            for block in module.constraints() {
                let mut env: Vec<(String, f64)> = Vec::new();
                for (leaf_index, (stmt, class)) in block.leaves().enumerate() {
                    if class.class != QuantityClass::Reduced {
                        // Pointwise vars never reach the host reduction
                        // (the kernel evaluates them); skip all
                        // non-reduced leaves entirely here.
                        continue;
                    }
                    match stmt {
                        ConstraintStmt::Var { name, value, .. } => {
                            let scope = EvalScope { env: &env, instance, result };
                            let v = self.scalar(value, &scope)?;
                            env.push((name.clone(), v));
                            entries.push(MarginEntry {
                                label: name.clone(),
                                instance_path: instance.label.clone(),
                                kind: MarginKind::Var,
                                value: MarginValue::Reduced { value: v },
                            });
                        }
                        ConstraintStmt::Target { expr, cmp, level, tol, .. } => {
                            let scope = EvalScope { env: &env, instance, result };
                            let margin = self.target_margin(expr, *cmp, level, tol.as_ref(), &scope)?;
                            entries.push(MarginEntry {
                                label: format!("target#{leaf_index}"),
                                instance_path: instance.label.clone(),
                                kind: MarginKind::Target,
                                value: MarginValue::Reduced { value: margin },
                            });
                        }
                        ConstraintStmt::Require { .. } | ConstraintStmt::Event { .. } => {}
                    }
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
        expr: &Expr,
        cmp: TargetCmp,
        level: &Expr,
        tol: Option<&Expr>,
        scope: &EvalScope<'_>,
    ) -> Result<f64, Error> {
        let value = self.scalar(expr, scope)?;
        let level = self.constant(level, scope.env)?;
        let scale = match tol {
            None => 1.0,
            Some(scale) => self.constant(scale, scope.env)?,
        };
        let diff = match cmp {
            TargetCmp::Le => level - value,
            TargetCmp::Ge => value - level,
        };
        Ok(diff / scale)
    }

    /// A target's level/scale must be an elaboration constant the host can
    /// read directly — a literal, or a reduced `var` already evaluated.
    /// Anything else (a param reference, a probe) fails loud rather than
    /// silently reading a stale or zero value.
    fn constant(&self, expr: &Expr, env: &[(String, f64)]) -> Result<f64, Error> {
        match expr {
            Expr::Literal(lit) => Self::literal(lit),
            Expr::Ident(name) => env
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .ok_or_else(|| Error::Measurement(format!(
                    "reduced target constant `{name}` is not a literal or an evaluated reduced var"
                ))),
            Expr::Unary(UnaryOp::Neg, e) => Ok(-self.constant(e, env)?),
            Expr::Binary(l, op, r) => {
                let l = self.constant(l, env)?;
                let r = self.constant(r, env)?;
                Self::arith(l, op, r)
            }
            other => Err(Error::Measurement(format!(
                "unsupported reduced target constant `{other:?}` — a literal or reduced var is required"
            ))),
        }
    }

    /// Evaluate a reduced scalar expression: literals, earlier reduced
    /// `var`s, real arithmetic, and the declared reduced helpers over a
    /// probe. Anything outside that vocabulary fails loud.
    fn scalar(&self, expr: &Expr, scope: &EvalScope<'_>) -> Result<f64, Error> {
        match expr {
            Expr::Literal(lit) => Self::literal(lit),
            Expr::Ident(name) => scope
                .env
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .ok_or_else(|| Error::Measurement(format!(
                    "unknown reduced quantity `{name}` — reduced vars evaluate in authored order"
                ))),
            Expr::Unary(UnaryOp::Neg, e) => Ok(-self.scalar(e, scope)?),
            Expr::Binary(l, op, r) => {
                let l = self.scalar(l, scope)?;
                let r = self.scalar(r, scope)?;
                Self::arith(l, op, r)
            }
            Expr::Call(func, args) => {
                let Expr::Ident(helper) = func.as_ref() else {
                    return Err(Error::Measurement("unsupported reduced expression (not a helper call)".into()));
                };
                let [arg] = args.as_slice() else {
                    return Err(Error::Measurement(format!(
                        "reduced helper `{helper}` takes exactly one probe argument"
                    )));
                };
                let probe = self.probe(arg, scope.instance, scope.result)?;
                Self::reduce(helper, &probe)
            }
            other => Err(Error::Measurement(format!(
                "unsupported reduced expression `{other:?}` — the host reduction handles \
                 literals, reduced vars, arithmetic, and helper(probe) calls"
            ))),
        }
    }

    /// The one reduction per declared helper (D7): each reads the probe's
    /// phasor series over the sweep.
    fn reduce(helper: &str, pts: &[(f64, Complex64)]) -> Result<f64, Error> {
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
    fn unity_gain_freq(pts: &[(f64, Complex64)]) -> Result<f64, Error> {
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
    fn phase_margin(pts: &[(f64, Complex64)]) -> Result<f64, Error> {
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
    fn gain_margin(pts: &[(f64, Complex64)]) -> Result<f64, Error> {
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
        pts: &[(f64, Complex64)],
        measure: impl Fn(Complex64) -> f64,
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
    fn interp_log(pts: &[(f64, Complex64)], f_at: f64, measure: impl Fn(Complex64) -> f64) -> f64 {
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
        expr: &Expr,
        instance: &piperine_codegen::device::BuiltInstanceInfo,
        result: &AcAnalysisResult,
    ) -> Result<Vec<(f64, Complex64)>, Error> {
        let Expr::Call(func, args) = expr else {
            return Err(Error::Measurement(
                "a reduced helper's argument must be a probe (`V(a, b)`)".into(),
            ));
        };
        let Expr::Ident(probe_fn) = func.as_ref() else {
            return Err(Error::Measurement(
                "a reduced helper's argument must be a probe (`V(a, b)`)".into(),
            ));
        };
        if probe_fn != "V" || args.len() != 2 {
            return Err(Error::Measurement(format!(
                "unsupported probe `{probe_fn}` — the host reduction reads `V(a, b)` probes"
            )));
        }
        let node_a = self.probe_node(&args[0], instance)?;
        let node_b = self.probe_node(&args[1], instance)?;
        let zero = Complex64::default();
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
        expr: &Expr,
        instance: &piperine_codegen::device::BuiltInstanceInfo,
    ) -> Result<NodeIdentifier, Error> {
        let Expr::Ident(name) = expr else {
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
        instance.terminals.get(position).cloned().ok_or_else(|| {
            Error::Measurement(format!(
                "instance `{}` has no terminal at port position {position}",
                instance.label
            ))
        })
    }

    fn literal(lit: &Literal) -> Result<f64, Error> {
        match lit {
            Literal::Real(v) => Ok(*v),
            Literal::Int(v) => Ok(*v as f64),
            other => Err(Error::Measurement(format!(
                "unsupported literal `{other:?}` in a reduced expression — a real constant is required"
            ))),
        }
    }

    fn arith(l: f64, op: &BinaryOp, r: f64) -> Result<f64, Error> {
        match op {
            BinaryOp::Add => Ok(l + r),
            BinaryOp::Sub => Ok(l - r),
            BinaryOp::Mul => Ok(l * r),
            BinaryOp::Div => Ok(l / r),
            other => Err(Error::Measurement(format!(
                "unsupported operator `{other:?}` in a reduced expression — arithmetic only"
            ))),
        }
    }
}
