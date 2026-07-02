//! Analog body flattening: from the statement tree to pure per-branch
//! expressions.
//!
//! The analog JIT skeleton compiles *flat contributions* — one expression per
//! branch, symbolically differentiable. This pass gets there by:
//!
//! - resolving sequential variable assignments symbolically (an `Assign`
//!   under a guard becomes `Select(guard, new, old)`),
//! - folding `If`/`Match` path conditions into contribution expressions
//!   (`I <+ e` under guard `g` contributes `Select(g, e, 0)`),
//! - inlining user function calls ([`Inliner`]),
//! - expanding `ddx` states into their compile-time derivative, and
//! - splitting each contribution into a resistive part and a charge `Q(V)`
//!   for the reactive companion model.
//!
//! Anything it cannot express faithfully is a named [`CodegenError`].

use std::collections::HashMap;

use crate::ir::{
    ContribKind, EventSource, IrExpr, IrModule, IrStateKind, IrStmt, Lval, NatureId, NatureKind,
    NodeId, Pattern, Severity, StateId, Trit, VarId,
};

use super::CodegenError;

/// A flattened flow contribution: current injected from `plus` to `minus`.
#[derive(Debug, Clone)]
pub struct FlatContrib {
    pub plus: NodeId,
    pub minus: NodeId,
    pub expr: IrExpr,
}

/// A flattened potential source: `V(plus) − V(minus) = expr` (one MNA
/// branch-current unknown per force).
#[derive(Debug, Clone)]
pub struct FlatForce {
    pub plus: NodeId,
    pub minus: NodeId,
    pub expr: IrExpr,
}

/// A diagnostic statement carried through for tooling; analog diagnostics are
/// not executed by the JIT (SPEC §12).
#[derive(Debug, Clone)]
pub struct FlatDiagnostic {
    pub severity: Severity,
    pub format: String,
}

/// The flattened analog behavior, ready for the Cranelift skeleton.
#[derive(Debug, Default)]
pub struct FlatAnalog {
    /// Resistive current expressions (reactive states substituted to 0,
    /// runtime states left as `State(id)` reads).
    pub resistive: Vec<FlatContrib>,
    /// Charge expressions `Q(V)` whose `ddt` is the reactive current.
    pub charge: Vec<FlatContrib>,
    /// Ideal potential sources.
    pub forces: Vec<FlatForce>,
    /// `$bound_step` expressions (the device hint is their minimum).
    pub bound_steps: Vec<IrExpr>,
    /// Resolved noise PSDs, in `body.noise` order: `(plus, minus, psd)`.
    pub noise: Vec<(NodeId, NodeId, IrExpr)>,
    /// Diagnostics collected (not executed) from the analog body.
    pub diagnostics: Vec<FlatDiagnostic>,
    /// Runtime-state slots (`delay`/`slew`) the device must service, with
    /// their resolved input expressions.
    pub runtime_states: Vec<(StateId, IrExpr)>,
}

/// Inlines user function calls by symbolic substitution. Function bodies may
/// use `VarDecl`/`Assign`/`If`/`Match`/`Return`; every path must return.
pub struct Inliner<'m> {
    module: &'m IrModule,
    depth: u32,
}

impl<'m> Inliner<'m> {
    const MAX_DEPTH: u32 = 32;

    pub fn new(module: &'m IrModule) -> Self {
        Self { module, depth: 0 }
    }

    /// Expand `Call(id, args)` into the function's body expression with
    /// parameters substituted. `args` must already be resolved.
    pub fn expand(&mut self, id: crate::ir::FnId, args: Vec<IrExpr>) -> Result<IrExpr, CodegenError> {
        self.depth += 1;
        if self.depth > Self::MAX_DEPTH {
            self.depth -= 1;
            return Err(CodegenError::Function(format!(
                "function inlining exceeded depth {} — recursive function?",
                Self::MAX_DEPTH
            )));
        }
        let function = self
            .module
            .symbols
            .try_fn(id)
            .ok_or_else(|| CodegenError::Function(format!("dangling fn #{}", id.0)))?;
        if function.params.len() != args.len() {
            self.depth -= 1;
            return Err(CodegenError::Function(format!(
                "`{}` expects {} args, got {}",
                function.name,
                function.params.len(),
                args.len()
            )));
        }

        let mut scope = Scope::new();
        for (&param, arg) in function.params.iter().zip(args) {
            scope.assign_unconditional(param, arg);
        }
        let body = function.body.clone();
        let mut walker = FnWalker { inliner: self, scope, returned: None, name: function.name.clone() };
        walker.walk(&body, None)?;
        let result = walker
            .returned
            .ok_or_else(|| CodegenError::Function(format!("`{}` never returns a value", walker.name)))?;
        self.depth -= 1;
        Ok(result)
    }
}

/// Symbolic variable environment shared by function and analog flattening.
struct Scope {
    vars: HashMap<VarId, Option<IrExpr>>,
}

impl Scope {
    fn new() -> Self {
        Self { vars: HashMap::new() }
    }

    fn declare(&mut self, var: VarId, init: Option<IrExpr>) {
        self.vars.insert(var, init);
    }

    fn assign_unconditional(&mut self, var: VarId, value: IrExpr) {
        self.vars.insert(var, Some(value));
    }

    /// Bind `var` to `value` under `guard`; outside the guard it keeps its
    /// previous value.
    fn assign(&mut self, var: VarId, value: IrExpr, guard: Option<&IrExpr>) -> Result<(), CodegenError> {
        let merged = match guard {
            None => value,
            Some(g) => {
                let old = self.read_opt(var);
                match old {
                    Some(old) => IrExpr::select(g.clone(), value, old),
                    // Assigned only on one path and never before: reads after
                    // this point would be undefined outside the guard.
                    None => IrExpr::select(g.clone(), value, IrExpr::Real(0.0)),
                }
            }
        };
        self.vars.insert(var, Some(merged));
        Ok(())
    }

    fn read_opt(&self, var: VarId) -> Option<IrExpr> {
        self.vars.get(&var).cloned().flatten()
    }
}

/// Statement walker for function bodies (shared statement subset + `Return`).
struct FnWalker<'m, 'i> {
    inliner: &'i mut Inliner<'m>,
    scope: Scope,
    returned: Option<IrExpr>,
    name: String,
}

impl FnWalker<'_, '_> {
    fn walk(&mut self, stmts: &[IrStmt], guard: Option<&IrExpr>) -> Result<(), CodegenError> {
        for stmt in stmts {
            match stmt {
                IrStmt::VarDecl { var, init } => {
                    let init = init.as_ref().map(|e| self.resolve(e)).transpose()?;
                    self.scope.declare(*var, init);
                }
                IrStmt::Assign { lval: Lval::Var(var), expr } => {
                    let value = self.resolve(expr)?;
                    self.scope.assign(*var, value, guard)?;
                }
                IrStmt::Assign { lval, .. } => {
                    return Err(CodegenError::unsupported(format!(
                        "non-variable assignment target {lval:?} in function `{}`",
                        self.name
                    )));
                }
                IrStmt::If { cond, then_, else_ } => {
                    let cond = self.resolve(cond)?;
                    let then_guard = and_guards(guard, &cond);
                    self.walk(then_, Some(&then_guard))?;
                    let else_guard = and_guards(guard, &not(&cond));
                    self.walk(else_, Some(&else_guard))?;
                }
                IrStmt::Match { scrutinee, arms, default } => {
                    let scrutinee = self.resolve(scrutinee)?;
                    let mut no_prior = None::<IrExpr>;
                    for (pattern, body) in arms {
                        let cond = pattern_condition(&scrutinee, pattern)?;
                        let cond = self.resolve(&cond)?;
                        let arm_guard = chain_guards(guard, &no_prior, &cond);
                        self.walk(body, Some(&arm_guard))?;
                        no_prior = Some(match no_prior {
                            None => not(&cond),
                            Some(prev) => IrExpr::binary(crate::ir::IrBinOp::And, prev, not(&cond)),
                        });
                    }
                    let default_guard = match &no_prior {
                        None => guard.cloned(),
                        Some(none_matched) => Some(and_guards(guard, none_matched)),
                    };
                    self.walk(default, default_guard.as_ref())?;
                }
                IrStmt::Return(Some(expr)) => {
                    let value = self.resolve(expr)?;
                    self.returned = Some(match (&self.returned, guard) {
                        (None, _) => value,
                        (Some(prev), Some(g)) => IrExpr::select(g.clone(), value, prev.clone()),
                        // A second unconditional return is dead code; the
                        // first one wins.
                        (Some(prev), None) => prev.clone(),
                    });
                }
                IrStmt::Return(None) => {
                    return Err(CodegenError::Function(format!(
                        "`{}` returns no value where one is required",
                        self.name
                    )));
                }
                other => {
                    return Err(CodegenError::unsupported(format!(
                        "statement {other:?} in function `{}`",
                        self.name
                    )));
                }
            }
        }
        Ok(())
    }

    fn resolve(&mut self, expr: &IrExpr) -> Result<IrExpr, CodegenError> {
        resolve_expr(expr, &self.scope, self.inliner)
    }
}

/// Substitute variables from `scope` and inline user calls, recursively.
fn resolve_expr(
    expr: &IrExpr,
    scope: &Scope,
    inliner: &mut Inliner<'_>,
) -> Result<IrExpr, CodegenError> {
    match expr {
        IrExpr::Var(id) => scope.read_opt(*id).ok_or_else(|| {
            CodegenError::Invalid(format!(
                "variable `{}` read before assignment",
                inliner.module.symbols.var(*id).name
            ))
        }),
        IrExpr::Call(id, args) => {
            let args = args
                .iter()
                .map(|a| resolve_expr(a, scope, inliner))
                .collect::<Result<Vec<_>, _>>()?;
            inliner.expand(*id, args)
        }
        other => {
            let mut error = None;
            let out = other.map_children(&mut |child| {
                match resolve_expr(child, scope, inliner) {
                    Ok(v) => v,
                    Err(e) => {
                        error.get_or_insert(e);
                        IrExpr::Real(0.0)
                    }
                }
            });
            match error {
                Some(e) => Err(e),
                None => Ok(out),
            }
        }
    }
}

/// The boolean condition under which `pattern` matches `scrutinee`.
fn pattern_condition(scrutinee: &IrExpr, pattern: &Pattern) -> Result<IrExpr, CodegenError> {
    use crate::ir::IrBinOp::Eq;
    match pattern {
        Pattern::Wildcard => Ok(IrExpr::Bool(true)),
        Pattern::Value(e) => Ok(IrExpr::binary(Eq, scrutinee.clone(), e.clone())),
        Pattern::BitPattern(trits) => match trits.as_slice() {
            [Trit::DontCare] => Ok(IrExpr::Bool(true)),
            [Trit::Zero] => Ok(IrExpr::binary(Eq, scrutinee.clone(), IrExpr::Int(0))),
            [Trit::One] => Ok(IrExpr::binary(Eq, scrutinee.clone(), IrExpr::Int(1))),
            _ => Err(CodegenError::unsupported(
                "multi-bit patterns in an analog/function `match`",
            )),
        },
    }
}

fn not(expr: &IrExpr) -> IrExpr {
    IrExpr::Unary(crate::ir::IrUnOp::Not, Box::new(expr.clone()))
}

fn and_guards(guard: Option<&IrExpr>, cond: &IrExpr) -> IrExpr {
    match guard {
        None => cond.clone(),
        Some(g) => IrExpr::binary(crate::ir::IrBinOp::And, g.clone(), cond.clone()),
    }
}

/// `guard ∧ no-prior-arm ∧ cond` for `match` arms.
fn chain_guards(guard: Option<&IrExpr>, no_prior: &Option<IrExpr>, cond: &IrExpr) -> IrExpr {
    let with_prior = match no_prior {
        None => cond.clone(),
        Some(prev) => IrExpr::binary(crate::ir::IrBinOp::And, prev.clone(), cond.clone()),
    };
    and_guards(guard, &with_prior)
}

// ─── Analog body flattening ───────────────────────────────────────────────────

/// Flattens an analog body into [`FlatAnalog`]. One-shot: construct, call
/// [`Self::flatten`].
pub struct AnalogFlattener<'m> {
    module: &'m IrModule,
    inliner: Inliner<'m>,
    scope: Scope,
    out: FlatAnalog,
    /// Potential contributions accumulate per branch before becoming forces.
    potential_acc: Vec<(NodeId, NodeId, IrExpr)>,
}

impl<'m> AnalogFlattener<'m> {
    pub fn new(module: &'m IrModule) -> Self {
        let mut scope = Scope::new();
        // Pre-populate the scope with module-level persistent vars (SPEC
        // §I.15, §9): these survive across evaluations. In a mixed-signal
        // module the analog body reads digital register values through
        // this path (the D2A bridge). Each var maps to `IrExpr::Var(id)`
        // — an external read the JIT services from the vars bank. If the
        // analog body assigns the var (sequential binding), that
        // assignment overwrites this entry.
        for (id, _) in module.symbols.vars() {
            scope.declare(id, Some(IrExpr::Var(id)));
        }
        Self {
            module,
            inliner: Inliner::new(module),
            scope,
            out: FlatAnalog::default(),
            potential_acc: Vec::new(),
        }
    }

    pub fn flatten(mut self) -> Result<FlatAnalog, CodegenError> {
        let body = self
            .module
            .analog
            .as_ref()
            .ok_or_else(|| CodegenError::Invalid(format!("`{}` has no analog body", self.module.name)))?;
        self.walk(&body.stmts, None)?;

        // Accumulated potential contributions become force rows.
        let potentials = std::mem::take(&mut self.potential_acc);
        for (plus, minus, expr) in potentials {
            let expr = self.finish_expr(expr)?;
            self.out.forces.push(FlatForce { plus, minus, expr });
        }

        // Noise PSDs resolve against the final variable environment.
        for source in &body.noise {
            let psd = match &source.kind {
                crate::ir::IrNoise::White { psd } => psd.clone(),
                crate::ir::IrNoise::Flicker { psd, .. } => psd.clone(),
            };
            let psd = resolve_expr(&psd, &self.scope, &mut self.inliner)?;
            let psd = self.finish_expr(psd)?;
            self.out.noise.push((source.plus, source.minus, psd));
        }
        Ok(self.out)
    }

    fn walk(&mut self, stmts: &[IrStmt], guard: Option<&IrExpr>) -> Result<(), CodegenError> {
        for stmt in stmts {
            match stmt {
                IrStmt::Contrib { nature, plus, minus, expr, kind } => {
                    self.add_contrib(*nature, *plus, *minus, expr, *kind, guard)?;
                }
                IrStmt::Force { nature, plus, minus, expr } => {
                    self.add_force(*nature, *plus, *minus, expr, guard)?;
                }
                IrStmt::Assign { lval: Lval::Var(var), expr } => {
                    let value = resolve_expr(expr, &self.scope, &mut self.inliner)?;
                    self.scope.assign(*var, value, guard)?;
                }
                IrStmt::Assign { lval, .. } => {
                    return Err(CodegenError::unsupported(format!(
                        "non-variable assignment target {lval:?} in an analog body"
                    )));
                }
                IrStmt::VarDecl { var, init } => {
                    let init = init
                        .as_ref()
                        .map(|e| resolve_expr(e, &self.scope, &mut self.inliner))
                        .transpose()?;
                    self.scope.declare(*var, init);
                }
                IrStmt::If { cond, then_, else_ } => {
                    let cond = resolve_expr(cond, &self.scope, &mut self.inliner)?;
                    let then_guard = and_guards(guard, &cond);
                    self.walk(then_, Some(&then_guard))?;
                    let else_guard = and_guards(guard, &not(&cond));
                    self.walk(else_, Some(&else_guard))?;
                }
                IrStmt::Match { scrutinee, arms, default } => {
                    let scrutinee = resolve_expr(scrutinee, &self.scope, &mut self.inliner)?;
                    let mut no_prior = None::<IrExpr>;
                    for (pattern, body) in arms {
                        let cond = pattern_condition(&scrutinee, pattern)?;
                        let arm_guard = chain_guards(guard, &no_prior, &cond);
                        self.walk(body, Some(&arm_guard))?;
                        no_prior = Some(match no_prior {
                            None => not(&cond),
                            Some(prev) => IrExpr::binary(crate::ir::IrBinOp::And, prev, not(&cond)),
                        });
                    }
                    let default_guard = match &no_prior {
                        None => guard.cloned(),
                        Some(none_matched) => Some(and_guards(guard, none_matched)),
                    };
                    self.walk(default, default_guard.as_ref())?;
                }
                IrStmt::BoundStep(expr) => {
                    let expr = resolve_expr(expr, &self.scope, &mut self.inliner)?;
                    let expr = self.finish_expr(expr)?;
                    self.out.bound_steps.push(expr);
                }
                IrStmt::Diagnostic { severity, format, .. } => {
                    self.out
                        .diagnostics
                        .push(FlatDiagnostic { severity: *severity, format: format.clone() });
                }
                IrStmt::Discontinuity(_) => {}
                IrStmt::AnalogEvent(event) => {
                    match &event.source {
                        EventSource::InitialStep => {
                            // `@ initial` — the body sets initial conditions
                            // (forces, contributions) that apply at t=0.
                            // Walk the body as regular statements so the
                            // forces/contributions are collected.
                            self.walk(&event.body, None)?;
                        }
                        EventSource::FinalStep => {
                            // `@ final` — the body runs at simulation end.
                            // Nothing to compile into the continuous kernel
                            // (no effect on the residual/Jacobian).
                        }
                        EventSource::Cross { .. } | EventSource::Above { .. } | EventSource::Timer { .. } => {
                            // Cross/above/timer events are runtime-triggered:
                            // the device detects the crossing/threshold and
                            // executes the body. An empty body is a no-op
                            // (common in tests that just declare the event).
                            // A non-empty body needs runtime event support
                            // (not yet implemented); for now we skip it so
                            // the module compiles, and the event's effect is
                            // absent at runtime.
                            if !event.body.is_empty() {
                                self.out.diagnostics.push(FlatDiagnostic {
                                    severity: Severity::Warn,
                                    format: "analog cross/above/timer event body is not yet executed at runtime".into(),
                                });
                            }
                        }
                    }
                }
                IrStmt::Finish => {
                    return Err(CodegenError::unsupported("$finish in an analog body"));
                }
                IrStmt::ClockedBlock { .. } | IrStmt::Return(_) => {
                    return Err(CodegenError::Invalid(format!(
                        "statement {stmt:?} is not allowed in an analog body"
                    )));
                }
            }
        }
        Ok(())
    }

    fn add_contrib(
        &mut self,
        nature: NatureId,
        plus: NodeId,
        minus: NodeId,
        expr: &IrExpr,
        kind: ContribKind,
        guard: Option<&IrExpr>,
    ) -> Result<(), CodegenError> {
        let resolved = resolve_expr(expr, &self.scope, &mut self.inliner)?;
        let guarded = match guard {
            None => resolved,
            Some(g) => IrExpr::select(g.clone(), resolved, IrExpr::Real(0.0)),
        };
        match self.module.symbols.nature(nature).kind {
            NatureKind::Flow => self.add_flow(guarded, plus, minus, kind),
            NatureKind::Potential => {
                if guard.is_some() {
                    return Err(CodegenError::unsupported(
                        "conditional potential contribution (`V(p,n) <+ …` under if/match)",
                    ));
                }
                match self
                    .potential_acc
                    .iter_mut()
                    .find(|(p, m, _)| *p == plus && *m == minus)
                {
                    Some((_, _, acc)) => {
                        *acc = IrExpr::binary(crate::ir::IrBinOp::Add, acc.clone(), guarded);
                    }
                    None => self.potential_acc.push((plus, minus, guarded)),
                }
                Ok(())
            }
        }
    }

    fn add_force(
        &mut self,
        nature: NatureId,
        plus: NodeId,
        minus: NodeId,
        expr: &IrExpr,
        guard: Option<&IrExpr>,
    ) -> Result<(), CodegenError> {
        let resolved = resolve_expr(expr, &self.scope, &mut self.inliner)?;
        match self.module.symbols.nature(nature).kind {
            NatureKind::Potential => {
                if let Some(g) = guard {
                    // Conditional potential force — a switch branch (SPEC
                    // §10.2). The ideal `V(a,b) <- expr` under guard `g`
                    // cannot conditionally add/remove an MNA branch.
                    // Use the finite-parameter approximation: model the
                    // switch as a variable conductance (Thevenin equiv).
                    //
                    //   I(a,b) <+ Select(g, G_LARGE, G_MIN) * (V(a,b) − expr)
                    //
                    // g=true:  I = G_LARGE * (V − expr) ≈ V = expr (closed)
                    // g=false: I = G_MIN * (V − expr)   ≈ I = 0    (open)
                    const GMIN: f64 = 1e-12;
                    const G_LARGE: f64 = 1.0 / GMIN;
                    let branch = IrExpr::Branch { nature, plus, minus };
                    let v_minus_expr = IrExpr::binary(crate::ir::IrBinOp::Sub, branch, resolved);
                    let conductance = IrExpr::Select(
                        Box::new(g.clone()),
                        Box::new(IrExpr::Real(G_LARGE)),
                        Box::new(IrExpr::Real(GMIN)),
                    );
                    let switch_expr =
                        IrExpr::binary(crate::ir::IrBinOp::Mul, conductance, v_minus_expr);
                    return self.add_flow(switch_expr, plus, minus, ContribKind::Resistive);
                }
                let expr = self.finish_expr(resolved)?;
                self.out.forces.push(FlatForce { plus, minus, expr });
                Ok(())
            }
            NatureKind::Flow => {
                if let Some(g) = guard {
                    let guarded = IrExpr::select(g.clone(), resolved, IrExpr::Real(0.0));
                    return self.add_flow(guarded, plus, minus, ContribKind::Resistive);
                }
                self.add_flow(resolved, plus, minus, ContribKind::Resistive)
            }
        }
    }

    /// Split a flow contribution into its resistive and charge parts and
    /// register any runtime states it references.
    fn add_flow(
        &mut self,
        expr: IrExpr,
        plus: NodeId,
        minus: NodeId,
        _declared: ContribKind,
    ) -> Result<(), CodegenError> {
        let has_reactive = expr
            .find_state(&|id| self.module.symbols.state(id).kind.is_reactive())
            .is_some();

        if has_reactive {
            // Q = expr[reactive → arg] − expr[reactive → 0] isolates the
            // reactive part; the resistive terms cancel. Exact for `ddt`;
            // for `idt` the companion coefficient is the solver's concern.
            let with_arg = self.substitute_reactive(&expr, true)?;
            let with_zero = self.substitute_reactive(&expr, false)?;
            let charge = IrExpr::binary(crate::ir::IrBinOp::Sub, with_arg, with_zero);
            let charge = self.finish_expr(charge)?;
            self.out.charge.push(FlatContrib { plus, minus, expr: charge });
        }

        let resistive = self.substitute_reactive(&expr, false)?;
        let resistive = self.finish_expr(resistive)?;
        self.out.resistive.push(FlatContrib { plus, minus, expr: resistive });
        Ok(())
    }

    /// Replace reactive `State(id)` reads with the operator's input (`arg`)
    /// or with 0.
    fn substitute_reactive(&mut self, expr: &IrExpr, with_arg: bool) -> Result<IrExpr, CodegenError> {
        let mut error = None;
        let out = expr.rewrite(&mut |e| {
            if let IrExpr::State(id) = &e {
                let state = self.module.symbols.state(*id);
                if state.kind.is_reactive() {
                    if !with_arg {
                        return IrExpr::Real(0.0);
                    }
                    return match resolve_expr(&state.arg.clone(), &self.scope, &mut self.inliner) {
                        Ok(arg) => arg,
                        Err(err) => {
                            error.get_or_insert(err);
                            IrExpr::Real(0.0)
                        }
                    };
                }
            }
            e
        });
        match error {
            Some(e) => Err(e),
            None => Ok(out),
        }
    }

    /// Final expression pass: expand `ddx`, register runtime states, and
    /// reject operators without a lowering.
    fn finish_expr(&mut self, expr: IrExpr) -> Result<IrExpr, CodegenError> {
        let mut error: Option<CodegenError> = None;
        let out = expr.rewrite(&mut |e| {
            let IrExpr::State(id) = &e else { return e };
            let id = *id;
            let state = self.module.symbols.state(id);
            match &state.kind {
                // Reactive states were substituted away before this pass.
                kind if kind.is_reactive() => {
                    error.get_or_insert(CodegenError::Invalid(format!(
                        "reactive `{}` state survived the reactive split",
                        kind.name()
                    )));
                    e
                }
                IrStateKind::Ddx { node } => {
                    match resolve_expr(&state.arg.clone(), &self.scope, &mut self.inliner)
                        .map(|arg| arg.d_dnode(*node))
                    {
                        Ok(derivative) => derivative,
                        Err(err) => {
                            error.get_or_insert(err);
                            e
                        }
                    }
                }
                IrStateKind::Delay { .. } | IrStateKind::Slew { .. } => {
                    match resolve_expr(&state.arg.clone(), &self.scope, &mut self.inliner) {
                        Ok(arg) => {
                            if !self.out.runtime_states.iter().any(|(s, _)| *s == id) {
                                self.out.runtime_states.push((id, arg));
                            }
                            e
                        }
                        Err(err) => {
                            error.get_or_insert(err);
                            e
                        }
                    }
                }
                kind @ (IrStateKind::Transition { .. } | IrStateKind::Table { .. }) => {
                    error.get_or_insert(CodegenError::unsupported(format!(
                        "analog operator `{}` lowering is not implemented yet",
                        kind.name()
                    )));
                    e
                }
                _ => e,
            }
        });
        match error {
            Some(e) => Err(e),
            None => Ok(out),
        }
    }
}
