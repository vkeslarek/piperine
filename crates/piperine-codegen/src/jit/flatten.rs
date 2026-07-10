//! Analog body flattening: from the POM `Stmt` tree to pure per-branch
//! expressions. Operates entirely on POM `Expr`/`Stmt` — no `IrExpr`.
//!
//! - Variable assignments resolved symbolically (`Ident(name)` → scope value),
//! - `If`/`Match` path conditions folded into contribution expressions,
//! - User function calls inlined,
//! - `__ddt(id, x)` markers split into resistive (0) and charge (x) parts,
//! - `__idt(id, x, ic)` lowered to the implicit-Euler companion,
//! - `__ddx(id, x, node)` resolved to a compile-time derivative,
//! - `__state_load(id)` left as-is for runtime state reads.

use std::collections::HashMap;

use piperine_lang::parse::ast::{BinaryOp, BindOp, EventSpec, Expr as PomExpr, Literal, Stmt as PomStmt};

use crate::ir::{
    CrossDir, EventSource, AnalogEvent, LoweredBody,
    NatureKind, NodeId, StateId, VarId,
};

use super::CodegenError;

/// Convert an `EventSpec` (AST) to `EventSource`(s) for the flattener.
/// Expressions are NOT substituted here — `add_event` does that.
fn event_spec_to_sources(spec: &EventSpec) -> Vec<EventSource> {
    match spec {
        EventSpec::Initial => vec![EventSource::InitialStep],
        EventSpec::Final => vec![EventSource::FinalStep],
        EventSpec::Named { name, arg } => match name.as_str() {
            "cross" => vec![EventSource::Cross { expr: arg.clone(), dir: CrossDir::Either }],
            "above" => vec![EventSource::Above { expr: arg.clone() }],
            "timer" => vec![EventSource::Timer { period: arg.clone() }],
            // Digital events (posedge/negedge/change) don't appear in analog bodies.
            _ => vec![],
        },
        EventSpec::Or(specs) => {
            let mut all = Vec::new();
            for s in specs {
                all.extend(event_spec_to_sources(s));
            }
            all
        }
    }
}

/// A flattened flow contribution: current injected from `plus` to `minus`.
#[derive(Debug, Clone)]
pub struct FlatContrib {
    pub plus: NodeId,
    pub minus: NodeId,
    pub expr: PomExpr,
}

/// A flattened potential source.
#[derive(Debug, Clone)]
pub struct FlatForce {
    pub plus: NodeId,
    pub minus: NodeId,
    pub expr: PomExpr,
    pub ac_stim: Option<(PomExpr, PomExpr)>,
}

#[derive(Debug, Clone)]
pub struct FlatDiagnostic {
    pub severity: crate::ir::Severity,
    pub format: String,
}

#[derive(Debug, Clone)]
pub enum FlatEventTrigger {
    Initial,
    Cross { expr: PomExpr, dir: CrossDir },
    Above { expr: PomExpr },
    Timer { period: PomExpr },
}

#[derive(Debug, Clone)]
pub struct FlatEventAction {
    pub var: VarId,
    pub value: PomExpr,
}

#[derive(Debug, Clone)]
pub struct FlatEvent {
    pub trigger: FlatEventTrigger,
    pub actions: Vec<FlatEventAction>,
}

impl FlatEvent {
    pub fn trigger_expr(&self) -> Option<&PomExpr> {
        match &self.trigger {
            FlatEventTrigger::Initial => None,
            FlatEventTrigger::Cross { expr, .. } | FlatEventTrigger::Above { expr } => Some(expr),
            FlatEventTrigger::Timer { period } => Some(period),
        }
    }
    pub fn exprs(&self) -> impl Iterator<Item = &PomExpr> {
        self.trigger_expr().into_iter().chain(self.actions.iter().map(|a| &a.value))
    }
}

#[derive(Debug)]
pub struct FlatAcStim {
    pub plus: NodeId,
    pub minus: NodeId,
    pub mag: PomExpr,
    pub phase: PomExpr,
}

#[derive(Debug, Default)]
pub struct FlatAnalog {
    pub resistive: Vec<FlatContrib>,
    pub charge: Vec<FlatContrib>,
    pub forces: Vec<FlatForce>,
    pub ac_stims: Vec<FlatAcStim>,
    pub bound_steps: Vec<PomExpr>,
    pub noise: Vec<(NodeId, NodeId, PomExpr, Option<PomExpr>)>,
    pub diagnostics: Vec<FlatDiagnostic>,
    pub runtime_states: Vec<(StateId, PomExpr)>,
    pub events: Vec<FlatEvent>,
}

impl FlatAnalog {
    /// Every expression in the flattened body (for terminal discovery).
    pub fn exprs(&self) -> impl Iterator<Item = &PomExpr> {
        self.resistive.iter().chain(&self.charge).map(|c| &c.expr)
            .chain(self.forces.iter().map(|f| &f.expr))
            .chain(self.ac_stims.iter().flat_map(|s| [&s.mag, &s.phase]))
            .chain(self.bound_steps.iter())
            .chain(self.noise.iter().map(|(_, _, psd, _)| psd))
            .chain(self.runtime_states.iter().map(|(_, input)| input))
            .chain(self.events.iter().flat_map(FlatEvent::exprs))
    }

    /// How far into the `params`/`state`/`vars` banks the compiled code reads.
    pub fn read_bounds(&self, module: &LoweredBody) -> (usize, usize, usize) {
        // Function param VarIds are NOT module-level vars — exclude them.
        let fn_param_ids: std::collections::HashSet<crate::ir::VarId> = module
            .symbols
            .fns()
            .flat_map(|(_, f)| f.params.iter().copied())
            .collect();
        let (mut params, mut state, mut vars) = (0usize, 0usize, 0usize);
        for expr in self.exprs() {
            visit_all(expr, &mut |e| {
                if let PomExpr::Call(func, args) = e
                    && let PomExpr::Ident(name) = func.as_ref()
                        && name == "__state_load"
                            && let Some(PomExpr::Literal(Literal::Int(id))) = args.first() {
                                state = state.max(*id as usize + 1);
                            }
                if let PomExpr::Ident(name) = e {
                    if let Some(id) = module_param_id(module, name) {
                        params = params.max(id.0 as usize + 1);
                    }
                    if let Some(id) = module_var_id(module, name)
                        && !fn_param_ids.contains(&id) {
                            vars = vars.max(id.0 as usize + 1);
                        }
                }
            });
        }
        (params, state, vars)
    }
}

fn module_param_id(module: &LoweredBody, name: &str) -> Option<crate::ir::ParamId> {
    module.symbols.params().find(|(_, p)| p.name == name).map(|(id, _)| id)
}

fn module_var_id(module: &LoweredBody, name: &str) -> Option<crate::ir::VarId> {
    module.symbols.vars().find(|(_, v)| v.name == name).map(|(id, _)| id)
}

fn visit_all<F: FnMut(&PomExpr)>(expr: &PomExpr, f: &mut F) {
    use piperine_lang::parse::ast::Walk;
    expr.walk(&mut |e| { f(e); Walk::Continue });
}

/// Symbolic variable environment: maps var name → bound expression.
struct Scope {
    vars: HashMap<String, PomExpr>,
}

impl Scope {
    fn new() -> Self { Self { vars: HashMap::new() } }

    fn declare(&mut self, name: String, init: Option<PomExpr>) {
        self.vars.insert(name, init.unwrap_or(PomExpr::Literal(Literal::Real(0.0))));
    }

    fn assign(&mut self, name: String, value: PomExpr, guard: Option<&PomExpr>) {
        let merged = match guard {
            None => value,
            Some(g) => {
                let old = self.vars.get(&name).cloned()
                    .unwrap_or(PomExpr::Literal(Literal::Real(0.0)));
                select(g.clone(), value, old)
            }
        };
        self.vars.insert(name, merged);
    }

    fn get(&self, name: &str) -> Option<&PomExpr> {
        self.vars.get(name)
    }
}

fn select(cond: PomExpr, then_: PomExpr, else_: PomExpr) -> PomExpr {
    PomExpr::If {
        cond: Box::new(cond),
        then_body: piperine_lang::parse::ast::Block { stmts: vec![], expr: Some(Box::new(then_)) },
        else_body: piperine_lang::parse::ast::Block { stmts: vec![], expr: Some(Box::new(else_)) },
    }
}

fn binary(op: BinaryOp, a: PomExpr, b: PomExpr) -> PomExpr {
    PomExpr::Binary(Box::new(a), op, Box::new(b))
}

fn lit(v: f64) -> PomExpr {
    PomExpr::Literal(Literal::Real(v))
}

fn not_expr(e: PomExpr) -> PomExpr {
    PomExpr::Unary(piperine_lang::parse::ast::UnaryOp::Not, Box::new(e))
}

fn and_guards(guard: Option<&PomExpr>, cond: &PomExpr) -> PomExpr {
    match guard {
        None => cond.clone(),
        Some(g) => binary(BinaryOp::And, g.clone(), cond.clone()),
    }
}

/// Flattens an analog body into [`FlatAnalog`].
pub struct AnalogFlattener<'m> {
    module: &'m LoweredBody,
    scope: Scope,
    out: FlatAnalog,
    potential_acc: Vec<(NodeId, NodeId, PomExpr)>,
}

impl<'m> AnalogFlattener<'m> {
    pub fn new(module: &'m LoweredBody) -> Self {
        let mut scope = Scope::new();
        for (_, v) in module.symbols.vars() {
            scope.declare(v.name.clone(), Some(PomExpr::Ident(v.name.clone())));
        }
        Self {
            module,
            scope,
            out: FlatAnalog::default(),
            potential_acc: Vec::new(),
        }
    }

    pub fn flatten(mut self) -> Result<FlatAnalog, CodegenError> {
        let body = self.module.analog.as_ref()
            .ok_or_else(|| CodegenError::Invalid(format!(
                "`{}` has no analog body", self.module.name)))?;
        self.walk(&body.stmts, None)?;

        let potentials = std::mem::take(&mut self.potential_acc);
        for (plus, minus, expr) in potentials {
            let (without, stim) = split_ac_stim(expr)?;
            let expr = self.finish_expr(without)?;
            let ac_stim = match stim {
                Some((mag, phase)) => Some((self.finish_expr(mag)?, self.finish_expr(phase)?)),
                None => None,
            };
            self.out.forces.push(FlatForce { plus, minus, expr, ac_stim });
        }

        for source in &body.noise {
            let (psd_src, exponent_src) = match &source.kind {
                crate::ir::NoiseKind::White { psd } => (psd.clone(), None),
                crate::ir::NoiseKind::Flicker { psd, exponent } => (psd.clone(), Some(exponent.clone())),
            };
            let psd = self.subst(&psd_src)?;
            let psd = self.finish_expr(psd)?;
            let exponent = match exponent_src {
                Some(e) => Some(self.finish_expr(self.subst(&e)?)?),
                None => None,
            };
            self.out.noise.push((source.plus, source.minus, psd, exponent));
        }
        Ok(self.out)
    }

    fn walk(&mut self, stmts: &[PomStmt], guard: Option<&PomExpr>) -> Result<(), CodegenError> {
        for stmt in stmts {
            match stmt {
                PomStmt::Bind { dest, op: BindOp::Contrib, src } => {
                    self.add_contrib(dest, src, guard)?;
                }
                PomStmt::Bind { dest, op: BindOp::Force, src } => {
                    self.add_force(dest, src, guard)?;
                }
                PomStmt::Bind { dest: PomExpr::Ident(name), op: BindOp::Assign, src } => {
                    let value = self.subst(src)?;
                    self.scope.assign(name.clone(), value, guard);
                }
                // A non-identifier assign target has no meaning in a
                // flattened analog body — nothing to record.
                PomStmt::Bind { op: BindOp::Assign, .. } => {}
                PomStmt::VarDecl { name, default, .. } => {
                    let init = match default {
                        Some(e) => Some(self.subst(e)?),
                        None => None,
                    };
                    self.scope.declare(name.clone(), init);
                }
                PomStmt::If { cond, then_body, else_body } => {
                    let cond = self.subst(cond)?;
                    let then_guard = and_guards(guard, &cond);
                    self.walk(&then_body.stmts, Some(&then_guard))?;
                    let else_guard = and_guards(guard, &not_expr(cond.clone()));
                    if let Some(eb) = else_body {
                        self.walk(&eb.stmts, Some(&else_guard))?;
                    }
                }
                PomStmt::Match { expr, arms } => {
                    let scrutinee = self.subst(expr)?;
                    let mut no_prior = None::<PomExpr>;
                    for arm in arms {
                        let cond = pattern_cond(&scrutinee, &arm.pat);
                        let arm_guard = chain_guards(guard, &no_prior, &cond);
                        self.walk(&arm.body.stmts, Some(&arm_guard))?;
                        no_prior = Some(match no_prior {
                            None => not_expr(cond),
                            Some(prev) => binary(BinaryOp::And, prev, not_expr(cond)),
                        });
                    }
                }
                PomStmt::Expr(PomExpr::SysCall(name, args)) => {
                    match name.trim_start_matches('$') {
                        "bound_step" => {
                            let val = args.first().map(|a| self.subst(a))
                                .unwrap_or(Ok(lit(0.0)))?;
                            let finished = self.finish_expr(val)?;
                            self.out.bound_steps.push(finished);
                        }
                        "finish" | "stop" => {
                            return Err(CodegenError::unsupported("$finish in an analog body"));
                        }
                        "discontinuity" => {}
                        n @ ("display" | "write" | "strobe" | "monitor"
                            | "warning" | "warn" | "error" | "fatal" | "info") =>
                        {
                            let severity = match n {
                                "warning" | "warn" => crate::ir::Severity::Warn,
                                "error" => crate::ir::Severity::Error,
                                "fatal" => crate::ir::Severity::Fatal,
                                _ => crate::ir::Severity::Info,
                            };
                            let fmt = match args.first() {
                                Some(PomExpr::Literal(Literal::String(s))) => s.clone(),
                                _ => String::new(),
                            };
                            self.out.diagnostics.push(FlatDiagnostic { severity, format: fmt });
                        }
                        _ => {}
                    }
                }
                PomStmt::Expr(_) => {}
                PomStmt::Diagnostic { sys, .. } => {
                    let bare = sys.trim_start_matches('$');
                    let severity = match bare {
                        "warning" | "warn" => crate::ir::Severity::Warn,
                        "error" => crate::ir::Severity::Error,
                        "fatal" => crate::ir::Severity::Fatal,
                        _ => crate::ir::Severity::Info,
                    };
                    self.out.diagnostics.push(FlatDiagnostic {
                        severity,
                        format: String::new(),
                    });
                }
                PomStmt::Event { spec, guard: event_guard, body } => {
                    // Combine the event's `when` guard with the outer path guard.
                    let combined_guard = match event_guard {
                        Some(eg) => {
                            let resolved_eg = self.subst(eg)?;
                            match guard {
                                Some(pg) => Some(Box::new(PomExpr::Binary(
                                    Box::new(resolved_eg), BinaryOp::And, Box::new(pg.clone()),
                                ))),
                                None => Some(Box::new(resolved_eg)),
                            }
                        }
                        None => guard.map(|g| Box::new(g.clone())),
                    };
                    for source in event_spec_to_sources(spec) {
                        let event = AnalogEvent {
                            source,
                            body: body.stmts.clone(),
                        };
                        self.add_event(&event, combined_guard.as_deref())?;
                    }
                }
                PomStmt::Return(_) => {
                    return Err(CodegenError::Invalid("`return` in an analog body".into()));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn add_contrib(&mut self, dest: &PomExpr, expr: &PomExpr, guard: Option<&PomExpr>) -> Result<(), CodegenError> {
        let (nature_kind, plus, minus) = self.parse_dest(dest)?;
        let resolved = self.subst(expr)?;
        let guarded = match guard {
            None => resolved,
            Some(g) => select(g.clone(), resolved, lit(0.0)),
        };
        match nature_kind {
            NatureKind::Flow => self.add_flow(guarded, plus, minus),
            NatureKind::Potential => {
                if guard.is_some() {
                    return Err(CodegenError::unsupported("conditional potential contribution"));
                }
                match self.potential_acc.iter_mut().find(|(p, m, _)| *p == plus && *m == minus) {
                    Some((_, _, acc)) => {
                        *acc = binary(BinaryOp::Add, acc.clone(), guarded);
                    }
                    None => self.potential_acc.push((plus, minus, guarded)),
                }
                Ok(())
            }
        }
    }

    fn add_force(&mut self, dest: &PomExpr, expr: &PomExpr, guard: Option<&PomExpr>) -> Result<(), CodegenError> {
        let (nature_kind, plus, minus) = self.parse_dest(dest)?;
        let resolved = self.subst(expr)?;
        match nature_kind {
            NatureKind::Potential => {
                if let Some(g) = guard {
                    let branch = PomExpr::Call(
                        Box::new(PomExpr::Ident("V".into())),
                        vec![PomExpr::Ident(self.module.symbols.node(plus).name.clone()),
                             PomExpr::Ident(self.module.symbols.node(minus).name.clone())],
                    );
                    let v_minus_e = binary(BinaryOp::Sub, branch, resolved);
                    let conductance = select(g.clone(), lit(1e12), lit(1e-12));
                    let switch = binary(BinaryOp::Mul, conductance, v_minus_e);
                    return self.add_flow(switch, plus, minus);
                }
                let expr = self.finish_expr(resolved)?;
                self.out.forces.push(FlatForce { plus, minus, expr, ac_stim: None });
                Ok(())
            }
            NatureKind::Flow => {
                if let Some(g) = guard {
                    let guarded = select(g.clone(), resolved, lit(0.0));
                    return self.add_flow(guarded, plus, minus);
                }
                self.add_flow(resolved, plus, minus)
            }
        }
    }

    fn parse_dest(&self, dest: &PomExpr) -> Result<(NatureKind, NodeId, NodeId), CodegenError> {
        if let PomExpr::Call(func, args) = dest
            && let PomExpr::Ident(name) = func.as_ref() {
            let nature_kind = match name.as_str() {
                "V" => NatureKind::Potential,
                _ => NatureKind::Flow,
            };
            let plus_name = ident_str(args.first()).unwrap_or_else(|| "?".into());
            let minus_name = ident_str(args.get(1)).unwrap_or_else(|| "0".into());
            let plus = self.resolve_node(&plus_name);
            let minus = self.resolve_node(&minus_name);
            return Ok((nature_kind, plus, minus));
        }
        Ok((NatureKind::Flow, NodeId::GROUND, NodeId::GROUND))
    }

    fn resolve_node(&self, name: &str) -> NodeId {
        if piperine_lang::pom::is_ground(name) {
            return NodeId::GROUND;
        }
        self.module.symbols.nodes()
            .find(|(_, n)| n.name == name)
            .map(|(id, _)| id)
            .unwrap_or(NodeId::GROUND)
    }

    fn add_flow(&mut self, expr: PomExpr, plus: NodeId, minus: NodeId) -> Result<(), CodegenError> {
        let expr = self.extract_ac_stim(expr, plus, minus)?;
        let has_ddt = has_marker(&expr, &["__ddt", "__laplace", "__ztransform"]);

        if has_ddt {
            let with_arg = substitute_marker(&expr, &["__ddt", "__laplace", "__ztransform"], true)?;
            let with_zero = substitute_marker(&expr, &["__ddt", "__laplace", "__ztransform"], false)?;
            let charge = binary(BinaryOp::Sub, with_arg, with_zero);
            let charge = self.finish_expr(charge)?;
            self.out.charge.push(FlatContrib { plus, minus, expr: charge });
        }

        let resistive = substitute_marker(&expr, &["__ddt", "__laplace", "__ztransform"], false)?;
        let resistive = self.finish_expr(resistive)?;
        self.out.resistive.push(FlatContrib { plus, minus, expr: resistive });
        Ok(())
    }

    fn extract_ac_stim(&mut self, expr: PomExpr, plus: NodeId, minus: NodeId) -> Result<PomExpr, CodegenError> {
        let (without, stim) = split_ac_stim(expr)?;
        if let Some((mag, phase)) = stim {
            let mag = self.finish_expr(mag)?;
            let phase = self.finish_expr(phase)?;
            self.out.ac_stims.push(FlatAcStim { plus, minus, mag, phase });
        }
        Ok(without)
    }

    /// Final expression pass: expand `__ddx`, lower `__idt`/`__idtmod` to
    /// the companion model, register runtime states.
    fn finish_expr(&mut self, expr: PomExpr) -> Result<PomExpr, CodegenError> {
        let error: Option<CodegenError> = None;
        let out = rewrite_expr(&expr, &mut |e| {
            if let PomExpr::Call(func, args) = e
                && let PomExpr::Ident(name) = func.as_ref() {
                    match name.as_str() {
                        "__ddx" => {
                            // __ddx(id, x, node_id) → d_dnode(x, node)
                            if args.len() >= 3 {
                                let x = &args[1];
                                let node_id = match &args[2] {
                                    PomExpr::Literal(Literal::Int(n)) => NodeId(*n as u32),
                                    _ => NodeId::GROUND,
                                };
                                let module = self.module;
                                let resolve = |n: &str| -> Option<NodeId> {
                                    module.symbols.nodes().find(|(_, info)| info.name == n).map(|(id, _)| id)
                                };
                                return crate::lower::diff::d_dnode(x, node_id, &resolve);
                            }
                            return lit(0.0);
                        }
                        "__idt" | "__idtmod" => {
                            // __idt(id, x, ic[, modulus]) → __state_load(id) + step * x
                            if args.len() >= 2 {
                                let id = match &args[0] {
                                    PomExpr::Literal(Literal::Int(n)) => StateId(*n as u32),
                                    _ => return e.clone(),
                                };
                                let x = &args[1];
                                if !self.out.runtime_states.iter().any(|(s, _)| *s == id) {
                                    self.out.runtime_states.push((id, x.clone()));
                                }
                                let state_load = PomExpr::Call(
                                    Box::new(PomExpr::Ident("__state_load".into())),
                                    vec![PomExpr::Literal(Literal::Int(id.0 as u64))],
                                );
                                let step = PomExpr::SysCall(
                                    "$simparam".to_string(),
                                    vec![PomExpr::Literal(Literal::String("step".into())), lit(0.0)],
                                );
                                return binary(BinaryOp::Add, state_load, binary(BinaryOp::Mul, step, x.clone()));
                            }
                            return e.clone();
                        }
                        "__delay" | "__slew" | "__transition" => {
                            // Runtime state: __op(id, x, ...) → __state_load(id), register input.
                            if let Some(PomExpr::Literal(Literal::Int(id))) = args.first() {
                                let sid = StateId(*id as u32);
                                if !self.out.runtime_states.iter().any(|(s, _)| *s == sid) {
                                    let x = args.get(1).cloned().unwrap_or(lit(0.0));
                                    self.out.runtime_states.push((sid, x));
                                }
                                return PomExpr::Call(
                                    Box::new(PomExpr::Ident("__state_load".into())),
                                    vec![PomExpr::Literal(Literal::Int(*id))],
                                );
                            }
                            return e.clone();
                        }
                        _ => {}
                    }
                }
            e.clone()
        });
        match error {
            Some(e) => Err(e),
            None => Ok(out),
        }
    }

    /// Substitute variables from scope and inline user calls.
    fn subst(&self, expr: &PomExpr) -> Result<PomExpr, CodegenError> {
        Ok(subst_scope(expr, &self.scope))
    }

    fn add_event(&mut self, event: &AnalogEvent, guard: Option<&PomExpr>) -> Result<(), CodegenError> {
        let trigger = match &event.source {
            EventSource::InitialStep => FlatEventTrigger::Initial,
            EventSource::FinalStep => {
                self.out.diagnostics.push(FlatDiagnostic {
                    severity: crate::ir::Severity::Info,
                    format: String::new(),
                });
                return Ok(());
            }
            EventSource::Cross { expr, dir } => {
                FlatEventTrigger::Cross { expr: self.finish_expr(self.subst(expr)?)?, dir: *dir }
            }
            EventSource::Above { expr } => {
                FlatEventTrigger::Above { expr: self.finish_expr(self.subst(expr)?)? }
            }
            EventSource::Timer { period } => {
                FlatEventTrigger::Timer { period: self.finish_expr(self.subst(period)?)? }
            }
        };
        let mut actions = Vec::new();
        self.collect_event_actions(&event.body, guard, &mut actions)?;
        self.out.events.push(FlatEvent { trigger, actions });
        Ok(())
    }

    fn collect_event_actions(
        &mut self,
        stmts: &[PomStmt],
        guard: Option<&PomExpr>,
        actions: &mut Vec<FlatEventAction>,
    ) -> Result<(), CodegenError> {
        for stmt in stmts {
            match stmt {
                PomStmt::Bind { dest, op: BindOp::Assign, src } => {
                    if let PomExpr::Ident(name) = dest
                        && let Some(var_id) = self.module.symbols.vars()
                            .find(|(_, v)| &v.name == name).map(|(id, _)| id)
                        {
                            let value = self.finish_expr(self.subst(src)?)?;
                            let value = match guard {
                                None => value,
                                Some(g) => select(g.clone(), value,
                                    PomExpr::Ident(name.clone())),
                            };
                            actions.push(FlatEventAction { var: var_id, value });
                        }
                }
                PomStmt::If { cond, then_body, else_body } => {
                    let cond = self.subst(cond)?;
                    let then_guard = and_guards(guard, &cond);
                    self.collect_event_actions(&then_body.stmts, Some(&then_guard), actions)?;
                    let else_guard = and_guards(guard, &not_expr(cond));
                    if let Some(eb) = else_body {
                        self.collect_event_actions(&eb.stmts, Some(&else_guard), actions)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Substitute `Ident(name)` with scope values.
fn subst_scope(expr: &PomExpr, scope: &Scope) -> PomExpr {
    match expr {
        PomExpr::Ident(name) => scope.get(name).cloned().unwrap_or_else(|| expr.clone()),
        PomExpr::Unary(op, x) => PomExpr::Unary(op.clone(), Box::new(subst_scope(x, scope))),
        PomExpr::Binary(l, op, r) => PomExpr::Binary(
            Box::new(subst_scope(l, scope)), op.clone(), Box::new(subst_scope(r, scope))),
        PomExpr::Call(f, args) => {
            let f = subst_scope(f, scope);
            let args: Vec<_> = args.iter().map(|a| subst_scope(a, scope)).collect();
            PomExpr::Call(Box::new(f), args)
        }
        PomExpr::SysCall(name, args) => {
            let args: Vec<_> = args.iter().map(|a| subst_scope(a, scope)).collect();
            PomExpr::SysCall(name.clone(), args)
        }
        PomExpr::If { cond, then_body, else_body } => PomExpr::If {
            cond: Box::new(subst_scope(cond, scope)),
            then_body: subst_block(then_body, scope),
            else_body: subst_block(else_body, scope),
        },
        PomExpr::Block(b) => PomExpr::Block(subst_block(b, scope)),
        PomExpr::Cast(t, x) => PomExpr::Cast(t.clone(), Box::new(subst_scope(x, scope))),
        PomExpr::Field(base, field) => {
            PomExpr::Field(Box::new(subst_scope(base, scope)), field.clone())
        }
        PomExpr::Index(base, idx) => PomExpr::Index(
            Box::new(subst_scope(base, scope)), Box::new(subst_scope(idx, scope))),
        _ => expr.clone(),
    }
}

fn subst_block(block: &piperine_lang::parse::ast::Block, scope: &Scope) -> piperine_lang::parse::ast::Block {
    piperine_lang::parse::ast::Block {
        stmts: block.stmts.iter().map(|s| subst_stmt(s, scope)).collect(),
        expr: block.expr.as_ref().map(|e| Box::new(subst_scope(e, scope))),
    }
}

fn subst_stmt(stmt: &PomStmt, scope: &Scope) -> PomStmt {
    use piperine_lang::parse::ast::Stmt as S;
    match stmt {
        S::Bind { dest, op, src } => S::Bind {
            dest: subst_scope(dest, scope), op: op.clone(), src: subst_scope(src, scope),
        },
        S::VarDecl { name, ty, default } => S::VarDecl {
            name: name.clone(), ty: ty.clone(),
            default: default.as_ref().map(|e| subst_scope(e, scope)),
        },
        S::If { cond, then_body, else_body } => S::If {
            cond: subst_scope(cond, scope),
            then_body: subst_block(then_body, scope),
            else_body: else_body.as_ref().map(|b| subst_block(b, scope)),
        },
        S::Expr(e) => S::Expr(subst_scope(e, scope)),
        S::Return(e) => S::Return(subst_scope(e, scope)),
        other => other.clone(),
    }
}

/// Check if an expression contains any of the named marker calls.
fn has_marker(expr: &PomExpr, names: &[&str]) -> bool {
    use piperine_lang::parse::ast::Walk;
    let mut found = false;
    expr.walk(&mut |e| {
        if let PomExpr::Call(func, _) = e
            && let PomExpr::Ident(name) = func.as_ref()
                && names.contains(&name.as_str()) {
                    found = true;
                    return Walk::SkipChildren;
                }
        Walk::Continue
    });
    found
}

/// Replace marker calls: `__ddt(id, x)` → `x` (with_arg=true) or `0.0` (false).
fn substitute_marker(expr: &PomExpr, names: &[&str], with_arg: bool) -> Result<PomExpr, CodegenError> {
    Ok(rewrite_expr(expr, &mut |e| {
        if let PomExpr::Call(func, args) = e
            && let PomExpr::Ident(name) = func.as_ref()
                && names.contains(&name.as_str()) {
                    if with_arg {
                        return args.get(1).cloned().unwrap_or(lit(0.0));
                    } else {
                        return lit(0.0);
                    }
                }
        e.clone()
    }))
}

/// Bottom-up rewrite: children first, then `f` on the rebuilt node.
fn rewrite_expr(expr: &PomExpr, f: &mut impl FnMut(&PomExpr) -> PomExpr) -> PomExpr {
    let rewritten = match expr {
        PomExpr::Literal(_) | PomExpr::Ident(_) | PomExpr::Path(_) => expr.clone(),
        PomExpr::SysCall(name, args) => PomExpr::SysCall(
            name.clone(),
            args.iter().map(|a| rewrite_expr(a, f)).collect(),
        ),
        PomExpr::Unary(op, x) => PomExpr::Unary(op.clone(), Box::new(rewrite_expr(x, f))),
        PomExpr::Binary(l, op, r) => PomExpr::Binary(
            Box::new(rewrite_expr(l, f)), op.clone(), Box::new(rewrite_expr(r, f))),
        PomExpr::Call(func, args) => {
            let func = rewrite_expr(func, f);
            let args: Vec<_> = args.iter().map(|a| rewrite_expr(a, f)).collect();
            PomExpr::Call(Box::new(func), args)
        }
        PomExpr::If { cond, then_body, else_body } => PomExpr::If {
            cond: Box::new(rewrite_expr(cond, f)),
            then_body: rewrite_block(then_body, f),
            else_body: rewrite_block(else_body, f),
        },
        PomExpr::Block(b) => PomExpr::Block(rewrite_block(b, f)),
        PomExpr::Cast(t, x) => PomExpr::Cast(t.clone(), Box::new(rewrite_expr(x, f))),
        PomExpr::Field(base, field) => PomExpr::Field(Box::new(rewrite_expr(base, f)), field.clone()),
        PomExpr::Index(base, idx) => PomExpr::Index(
            Box::new(rewrite_expr(base, f)), Box::new(rewrite_expr(idx, f))),
        _ => expr.clone(),
    };
    f(&rewritten)
}

fn rewrite_block(block: &piperine_lang::parse::ast::Block, f: &mut impl FnMut(&PomExpr) -> PomExpr) -> piperine_lang::parse::ast::Block {
    piperine_lang::parse::ast::Block {
        stmts: block.stmts.iter().map(|s| rewrite_stmt(s, f)).collect(),
        expr: block.expr.as_ref().map(|e| Box::new(rewrite_expr(e, f))),
    }
}

fn rewrite_stmt(stmt: &PomStmt, f: &mut impl FnMut(&PomExpr) -> PomExpr) -> PomStmt {
    use piperine_lang::parse::ast::Stmt as S;
    match stmt {
        S::Bind { dest, op, src } => S::Bind {
            dest: rewrite_expr(dest, f), op: op.clone(), src: rewrite_expr(src, f),
        },
        S::Expr(e) => S::Expr(rewrite_expr(e, f)),
        S::Return(e) => S::Return(rewrite_expr(e, f)),
        other => other.clone(),
    }
}

fn chain_guards(guard: Option<&PomExpr>, no_prior: &Option<PomExpr>, cond: &PomExpr) -> PomExpr {
    let with_prior = match no_prior {
        None => cond.clone(),
        Some(prev) => binary(BinaryOp::And, prev.clone(), cond.clone()),
    };
    and_guards(guard, &with_prior)
}

fn pattern_cond(scrutinee: &PomExpr, pattern: &piperine_lang::parse::ast::Pattern) -> PomExpr {
    use piperine_lang::parse::ast::Pattern as P;
    match pattern {
        P::Wildcard => lit(1.0),
        P::Literal(lit_v) => binary(BinaryOp::Eq, scrutinee.clone(),
            PomExpr::Literal(Literal::Int(*lit_v))),
        P::Path(p) => {
            let name = p.segments.join("::");
            binary(BinaryOp::Eq, scrutinee.clone(), PomExpr::Ident(name))
        }
        P::BitPattern(bits) => {
            let mut mask = 0i64;
            let mut value = 0i64;
            for c in bits.chars() {
                mask <<= 1; value <<= 1;
                match c {
                    '0' => mask |= 1,
                    '1' => { mask |= 1; value |= 1; }
                    _ => {}
                }
            }
            binary(BinaryOp::Eq,
                binary(BinaryOp::BitAnd, scrutinee.clone(), PomExpr::Literal(Literal::Int(mask as u64))),
                PomExpr::Literal(Literal::Int(value as u64)))
        }
    }
}

fn ident_str(e: Option<&PomExpr>) -> Option<String> {
    match e? {
        PomExpr::Ident(s) => Some(s.clone()),
        PomExpr::Field(base, field) => match base.as_ref() {
            PomExpr::Ident(base_name) => Some(format!("{base_name}.{field}")),
            _ => None,
        },
        _ => None,
    }
}

/// Split a contribution expression around a single `$ac_stim` SysCall.
fn split_ac_stim(expr: PomExpr) -> Result<(PomExpr, Option<(PomExpr, PomExpr)>), CodegenError> {
    let mut count = 0usize;
    let mut phase_expr = None;
    visit_all(&expr, &mut |e| {
        if let PomExpr::SysCall(name, args) = e
            && name == "$ac_stim" {
                count += 1;
                phase_expr = args.get(1).cloned();
            }
    });
    if count == 0 {
        return Ok((expr, None));
    }
    if count > 1 {
        return Err(CodegenError::unsupported("multiple `ac_stim` calls in one contribution"));
    }
    let with_mag = rewrite_expr(&expr, &mut |e| {
        if let PomExpr::SysCall(name, args) = e
            && name == "$ac_stim" {
                return args.first().cloned().unwrap_or(lit(1.0));
            }
        e.clone()
    });
    let without = rewrite_expr(&expr, &mut |e| {
        if let PomExpr::SysCall(name, _) = e
            && name == "$ac_stim" {
                return lit(0.0);
            }
        e.clone()
    });
    let mag = binary(BinaryOp::Sub, with_mag, without.clone());
    let phase = phase_expr.unwrap_or(lit(0.0));
    Ok((without, Some((mag, phase))))
}
