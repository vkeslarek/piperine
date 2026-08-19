//! Analog body flattening: from the POM `Stmt` tree
//! (`piperine_lang::ast::Stmt`) to pure per-branch expressions. Operates
//! entirely on POM `Expr`/`Stmt`.
//!
//! - Variable assignments resolved symbolically (`Ident(name)` → scope value),
//! - `If`/`Case` path conditions folded into contribution expressions,
//! - User function calls inlined,
//! - `__ddt(id, x)` markers split into resistive (0) and charge (x) parts,
//! - `__idt(id, x, ic)` lowered to the implicit-Euler companion,
//! - `__ddx(id, x, node)` resolved to a compile-time derivative,
//! - `__state_load(id)` left as-is for runtime state reads.
//!
//! **MD-45 note**: PHDL's `BindOp::Force` (`dest <- src`, an analog "force"
//! distinct from `<+` contribution — used for ideal-source guarded/switch
//! branches and `@initial` UIC seeding) has no VAMS Annex A production —
//! Verilog-AMS has only `<+`. Since no `.vams` source can ever construct
//! one, `add_force`/`collect_initial_conditions`/the guarded switch-branch
//! trick are deleted, not ported (documented, not silent) — this is *not*
//! a regression for anything expressible in VAMS: a genuinely
//! guarded/conditional potential contribution was **already** an error via
//! `<+` before this retarget (see `add_contrib`'s Potential arm, unchanged),
//! and no VAMS source ever had `<-` to reach the deleted path in the first
//! place. `FlatForce`/the `forces` pipeline itself stays — every ordinary
//! `V(p,n) <+ expr;` (LRM's ideal-source idiom, e.g. `V(p,n) <+ V0;`)
//! still flows through `add_contrib`'s potential accumulator into
//! `split_flux` exactly as before; that pipeline was never Force-specific.

use std::collections::HashMap;

use piperine_lang::ast::{BinOp, Expr as PomExpr, EventExpr, Stmt as PomStmt, UnOp};
use piperine_lang::lexer::NumberLit;
use piperine_lang::error::PreprocessedSpan;

use crate::resolve::{
    CrossDir, EventSource, AnalogEvent, LoweredBody,
    NatureKind, NodeId, StateId, VarId,
};

use crate::error::CodegenError;

const NOSPAN: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn call(name: &str, args: Vec<PomExpr>) -> PomExpr {
    PomExpr::Call { span: NOSPAN, name: name.to_string(), args }
}

/// Convert an event-control's `events` list (LRM §5.6 analog event
/// functions, seen as ordinary `Call` expressions inside an `EventExpr`) to
/// `EventSource`s for the flattener. Expressions are NOT substituted here —
/// `add_event` does that.
fn event_exprs_to_sources(events: &[EventExpr]) -> Result<Vec<EventSource>, CodegenError> {
    let mut out = Vec::new();
    for event in events {
        out.extend(event_expr_to_sources(event)?);
    }
    Ok(out)
}

fn event_expr_to_sources(event: &EventExpr) -> Result<Vec<EventSource>, CodegenError> {
    // §5.10.5 (LRMC-C5-05): "analog behavior can be made sensitive to
    // digital events, including posedge events, negedge events" — no
    // longer rejected by Annex C (`elab::typecheck`'s own T27 narrowing),
    // so this is a live path, not defensive. A digital signal reads as
    // 0/1 through the D2A bridge (the same var bank `Named`/T26 uses), so
    // `posedge sig`/`negedge sig` is exactly a rising/falling crossing of
    // that value through zero — reuses `Cross` rather than inventing a
    // separate digital-edge detector.
    let e = match event {
        EventExpr::Posedge(e) => {
            return Ok(vec![EventSource::Cross { expr: e.clone(), dir: CrossDir::Rising }]);
        }
        EventExpr::Negedge(e) => {
            return Ok(vec![EventSource::Cross { expr: e.clone(), dir: CrossDir::Falling }]);
        }
        // `@*`/`@(*)` (ImplicitAll) has no fixed watched-expression shape
        // (it means "every signal read in the body") — a distinct,
        // larger feature (needs body-wide signal collection), genuinely
        // out of LRMC-C5-05's scope, still a defensive empty list.
        EventExpr::ImplicitAll => return Ok(vec![]),
        EventExpr::Level(e) => e,
    };
    // `initial_step`/`final_step` take no mandatory args, so the parser
    // accepts the bare-identifier spelling (`@(initial_step)`, no parens)
    // as well as the zero/multi-arg call form (`@(initial_step("dc"))`) —
    // LRM §5.6's analog event functions. `cross`/`above`/`timer`/`absdelta`
    // always carry at least one arg and so only ever parse as `Call`.
    // `is_call` distinguishes a genuine (mis-typed or unimplemented)
    // function call, which must fail loud below, from a bare signal-name
    // event (`@(clk)`, no parens — a legitimate LRM form this codegen
    // doesn't implement yet, defensively empty, not this task's scope).
    let (name, args, is_call): (&str, &[PomExpr], bool) = match e {
        PomExpr::Call { name, args, .. } => (name.as_str(), args.as_slice(), true),
        PomExpr::Ident { name, .. } => (name.as_str(), &[], false),
        _ => return Ok(vec![]),
    };
    let sources = match name {
        "initial_step" => vec![EventSource::InitialStep],
        "final_step" => vec![EventSource::FinalStep],
        // First arg is the watched expression; a missing arg defaults to a
        // zero literal (the elaborator rejects genuinely arg-less events).
        // Second arg is the direction (§5.10.3.1): a LRM-mandated
        // compile-time constant, `+1`/`-1`/`0` -> rising/falling/either,
        // omitted -> either (LRMC-C5-02).
        "cross" => vec![EventSource::Cross {
            expr: args.first().cloned().unwrap_or_else(|| lit(0.0)),
            dir: args.get(1).map(cross_dir_from_arg).unwrap_or(CrossDir::Either),
        }],
        "above" => vec![EventSource::Above {
            expr: args.first().cloned().unwrap_or_else(|| lit(0.0)),
        }],
        // `@timer(period)` or `@timer(period, phase)`. `phase` (default 0)
        // offsets the fire times: fires at phase, phase+period, … instead
        // of period, 2·period, … — so a source can declare both its rise
        // and fall edges with two phased timers.
        "timer" => {
            let period = args.first().cloned().unwrap_or_else(|| lit(0.0));
            let phase = args.get(1).cloned().unwrap_or_else(|| lit(0.0));
            vec![EventSource::Timer { period, phase }]
        }
        // §5.10.3.4: expr/delta required; time_tol/expr_tol/enable
        // optional, tool-chosen when omitted — 0 tolerances and always-on
        // (1) are the LRM's own stated fallback ("if the tolerances are
        // not specified, then the tool sets them"; an absent enable means
        // unconditionally active, matching the LRM's own example, which
        // never passes one).
        "absdelta" => vec![EventSource::AbsDelta {
            expr: args.first().cloned().unwrap_or_else(|| lit(0.0)),
            delta: args.get(1).cloned().unwrap_or_else(|| lit(0.0)),
            time_tol: args.get(2).cloned().unwrap_or_else(|| lit(0.0)),
            expr_tol: args.get(3).cloned().unwrap_or_else(|| lit(0.0)),
            enable: args.get(4).cloned().unwrap_or_else(|| lit(1.0)),
        }],
        _ if is_call => {
            return Err(CodegenError::unsupported(format!(
                "unknown analog event function `{name}(...)` — LRM Annex A.6.5 defines only \
                 cross/above/timer/absdelta (plus initial_step/final_step)"
            )));
        }
        // A bare signal/named-event sensitivity (§5.6/§5.10.4, LRMC-C5-04):
        // fires on any change of `name`'s value — a named event (declared
        // `event e;`, elaborated as a hidden integer var T25/T26 toggles
        // on `-> e;`) or an ordinary signal used the same way. Resolution
        // of whether `name` actually exists happens downstream (`finish_expr`/
        // emission), same as every other bare identifier in this codegen.
        _ => vec![EventSource::Named { expr: PomExpr::Ident { span: NOSPAN, name: name.to_string() } }],
    };
    Ok(sources)
}

/// `cross(expr, dir, ...)`'s `dir` argument (§5.10.3.1, a required
/// compile-time constant) to `CrossDir` — `+1` rising, `-1` falling, `0`
/// (or any other/non-literal shape) either, matching this file's own
/// permissive-default style for a constant it cannot statically read.
fn cross_dir_from_arg(expr: &PomExpr) -> CrossDir {
    // A negative literal (`-1`) lexes as `Unary{Neg, Number(1)}`, not a
    // single negative `Number` token — unwrap that one level before
    // reading the magnitude, or `-1` silently falls through as `Either`.
    let (expr, negate) = match expr {
        PomExpr::Unary { op: UnOp::Neg, operand, .. } => (operand.as_ref(), true),
        other => (other, false),
    };
    let n = match expr {
        PomExpr::Number { value: NumberLit::Integer(n), .. } => *n,
        PomExpr::Number { value: NumberLit::Real(r), .. } => *r as i64,
        _ => return CrossDir::Either,
    };
    match if negate { -n } else { n } {
        1 => CrossDir::Rising,
        -1 => CrossDir::Falling,
        _ => CrossDir::Either,
    }
}

/// A flattened flow contribution: current injected from `plus` to `minus`.
#[derive(Debug, Clone)]
pub struct FlatContrib {
    pub plus: NodeId,
    pub minus: NodeId,
    pub expr: PomExpr,
}

/// A flattened potential source (every `V(p,n) <+ expr;` — LRM's ideal-
/// source idiom).
#[derive(Debug, Clone)]
pub struct FlatForce {
    pub plus: NodeId,
    pub minus: NodeId,
    pub expr: PomExpr,
    pub ac_stim: Option<(PomExpr, PomExpr)>,
    /// Inductor flux terms: each `(branch_plus, branch_minus, coeff)` is a
    /// `coeff·ddt(I(branch))` piece of this force's value, i.e. the flux
    /// `Φ = Σ coeff·I(branch)` linear in one or more branch currents. A term
    /// over the force's *own* branch is a self-inductance `L`; a term over a
    /// *different* branch is mutual inductance `M` (transformer `K`). Empty
    /// for a non-reactive force. The transient companion adds `dΦ/dt` on the
    /// branch equation (`force_flux_stamps`); a short in DC/AC-op (`dt = 0`).
    pub flux_terms: Vec<(NodeId, NodeId, PomExpr)>,
    /// Series-impedance terms: each `(branch_plus, branch_minus, coeff)` is a
    /// linear `coeff·I(branch)` piece of this force's *resistive* value —
    /// `V(p,n) <+ R·I(p,n)` is an exact series resistor over the force's own
    /// branch (a perfect short at `R = 0`). Split out because the JIT reads a
    /// bare `I(a,b)` in an expression as a branch *voltage*; the runtime
    /// stamps `−coeff` on the target branch-current column instead. The
    /// target must itself be a forced branch (its current is an MNA unknown).
    pub current_terms: Vec<(NodeId, NodeId, PomExpr)>,
}

#[derive(Debug, Clone)]
pub struct FlatDiagnostic {
    pub severity: crate::resolve::Severity,
    pub format: String,
}

#[derive(Debug, Clone)]
pub enum FlatEventTrigger {
    Initial,
    Cross { expr: PomExpr, dir: CrossDir },
    Above { expr: PomExpr },
    Timer { period: PomExpr, phase: PomExpr },
    /// §5.10.3.4 — `expr` is the per-step trigger row (same role as
    /// `Cross`/`Above`'s own `expr`); `delta`/`time_tol`/`expr_tol`/
    /// `enable` are evaluated once at elaboration (`kernel/analog/compile.rs`
    /// mirrors `Timer`'s `period`/`phase` treatment).
    AbsDelta { expr: PomExpr, delta: PomExpr, time_tol: PomExpr, expr_tol: PomExpr, enable: PomExpr },
    /// A bare signal/named-event sensitivity (§5.6/§5.10.4, LRMC-C5-04) —
    /// fires on any change of `expr`'s value.
    Named { expr: PomExpr },
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
            FlatEventTrigger::Cross { expr, .. }
            | FlatEventTrigger::Above { expr }
            | FlatEventTrigger::AbsDelta { expr, .. }
            | FlatEventTrigger::Named { expr } => Some(expr),
            FlatEventTrigger::Timer { period, .. } => Some(period),
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
    /// The value tape: `temps[id]` is the expression for temporary `id`,
    /// referencing params/nodes/`V`/`I`/`$limit` and *earlier* temps via
    /// `__temp(id)` leaves. Each `var` (and each guarded reassignment)
    /// produces one entry; contributions reference them instead of inlining,
    /// keeping every downstream expression linear in body size. Codegen
    /// evaluates each temp once (value) and differentiates each once
    /// (derivative tape).
    pub temps: Vec<PomExpr>,
    pub resistive: Vec<FlatContrib>,
    pub charge: Vec<FlatContrib>,
    pub forces: Vec<FlatForce>,
    pub ac_stims: Vec<FlatAcStim>,
    pub bound_steps: Vec<PomExpr>,
    /// `$discontinuity(...)` call sites (LRMC-C7-11, T42): each entry is
    /// `guard ? 1.0 : 0.0` (unconditional `1.0` when `guard` is `None`) —
    /// `kernel/analog/compile.rs` folds them via `max` into one
    /// fired-this-evaluation flag.
    pub discontinuities: Vec<PomExpr>,
    pub noise: Vec<(NodeId, NodeId, PomExpr, Option<PomExpr>)>,
    pub diagnostics: Vec<FlatDiagnostic>,
    pub runtime_states: Vec<(StateId, PomExpr)>,
    pub events: Vec<FlatEvent>,
    /// UIC seeds: `(plus, minus, value)`. Always empty post-MD-45 — the
    /// sole PHDL producer (`BindOp::Force` inside `@initial`) has no VAMS
    /// source; a VAMS UIC idiom (`@(initial_step) V(p,n) <+ ic;`, a
    /// contribution nested inside an event-action body) has no lowering
    /// path yet either (event actions currently only lower `Assign`, see
    /// `collect_event_actions`) — tracked as a follow-up, not invented
    /// speculatively here. The field/consumer stay so a future task adding
    /// that lowering doesn't also need a new `FlatAnalog` field.
    pub initial_conditions: Vec<(NodeId, NodeId, PomExpr)>,
    /// Module-level analog opvars (ABI-30): `(var_name, final_temp_id)` for
    /// each non-shadow module `var` reachable at the end of flattening. The
    /// compiler emits an opvar-evaluation `AnalogFn` from these — one row per
    /// var reading its final temp (`__temp(id)`). A device without analog
    /// vars leaves this empty (the opvar path is `None`, zero overhead).
    pub module_var_temps: Vec<(String, u32)>,
}

impl FlatAnalog {
    /// Every expression in the flattened body (for terminal/param/state
    /// discovery and `$limit` collection). Includes the temp tape — the
    /// `V`/`I`/param/`$limit` references now live inside temps, not the
    /// contributions (which hold only `__temp` leaves).
    pub fn exprs(&self) -> impl Iterator<Item = &PomExpr> {
        self.temps.iter()
            .chain(self.resistive.iter().chain(&self.charge).map(|c| &c.expr))
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
        let fn_param_ids: std::collections::HashSet<crate::resolve::VarId> = module
            .symbols
            .fns()
            .flat_map(|(_, f)| f.params.iter().copied())
            .collect();
        let (mut params, mut state, mut vars) = (0usize, 0usize, 0usize);
        for expr in self.exprs() {
            visit_all(expr, &mut |e| {
                if let PomExpr::Call { name, args, .. } = e
                    && name == "__state_load"
                    && let Some(PomExpr::Number { value: NumberLit::Integer(id), .. }) = args.first() {
                        state = state.max(*id as usize + 1);
                    }
                if let PomExpr::Ident { name, .. } = e {
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

fn module_param_id(module: &LoweredBody, name: &str) -> Option<crate::resolve::ParamId> {
    module.symbols.params().find(|(_, p)| p.name == name).map(|(id, _)| id)
}

fn module_var_id(module: &LoweredBody, name: &str) -> Option<crate::resolve::VarId> {
    module.symbols.vars().find(|(_, v)| v.name == name).map(|(id, _)| id)
}

/// Pre-order walk over a VAMS `Expr`. No shared `Walk` trait exists
/// post-MD-45 — every `resolve`/`flatten` module that needs one carries its
/// own (T29's `diff.rs` precedent).
pub(crate) fn visit_all<F: FnMut(&PomExpr)>(expr: &PomExpr, f: &mut F) {
    f(expr);
    match expr {
        // Never reaches codegen — see this file's other PortAccess arm
        // (rewrite_expr) for why.
        PomExpr::Number { .. } | PomExpr::Str { .. } | PomExpr::Ident { .. } | PomExpr::PortAccess { .. } => {}
        PomExpr::Unary { operand, .. } => visit_all(operand, f),
        PomExpr::Binary { lhs, rhs, .. } => {
            visit_all(lhs, f);
            visit_all(rhs, f);
        }
        PomExpr::Ternary { cond, then_branch, else_branch, .. } => {
            visit_all(cond, f);
            visit_all(then_branch, f);
            visit_all(else_branch, f);
        }
        PomExpr::Concat { parts, .. } | PomExpr::Replication { parts, .. } => {
            for p in parts {
                visit_all(p, f);
            }
        }
        PomExpr::AssignmentPattern { replicate, parts, .. } => {
            if let Some(r) = replicate {
                visit_all(r, f);
            }
            for p in parts {
                visit_all(p, f);
            }
        }
        PomExpr::Index { base, index, .. } => {
            visit_all(base, f);
            visit_all(index, f);
        }
        PomExpr::PartSelect { base, msb, lsb, .. } => {
            visit_all(base, f);
            visit_all(msb, f);
            visit_all(lsb, f);
        }
        PomExpr::Call { args, .. } | PomExpr::SystemFunctionCall { args, .. } => {
            for a in args {
                visit_all(a, f);
            }
        }
        PomExpr::AccessFunction { pos, neg, .. } => {
            visit_all(pos, f);
            if let Some(n) = neg {
                visit_all(n, f);
            }
        }
    }
}

/// Variable environment: maps a `var` name to its current **temporary id**.
///
/// A `var` is never inlined into the expressions that use it — that turns a
/// value reused N times into N copies of its whole subtree and blows up
/// multiplicatively (`vd = $limit(…, vd)` reused ~30× in a diode). Instead
/// each `var` definition becomes one entry in [`FlatAnalog::temps`] and every
/// use is a `__temp(id)` leaf. Codegen evaluates each temp **once** (value
/// tape) and differentiates each **once** (derivative tape), so both the
/// residual and the Jacobian stay linear in body size.
struct Scope {
    ids: HashMap<String, u32>,
}

impl Scope {
    fn new() -> Self { Self { ids: HashMap::new() } }

    fn set(&mut self, name: String, id: u32) {
        self.ids.insert(name, id);
    }

    fn get(&self, name: &str) -> Option<u32> {
        self.ids.get(name).copied()
    }
}

/// A `__temp(id)` leaf — a reference to temporary `id` in [`FlatAnalog::temps`].
pub(crate) fn temp_ref(id: u32) -> PomExpr {
    call("__temp", vec![int(id as i64)])
}

fn select(cond: PomExpr, then_: PomExpr, else_: PomExpr) -> PomExpr {
    PomExpr::Ternary { span: NOSPAN, cond: Box::new(cond), then_branch: Box::new(then_), else_branch: Box::new(else_) }
}

fn binary(op: BinOp, a: PomExpr, b: PomExpr) -> PomExpr {
    PomExpr::Binary { span: NOSPAN, op, lhs: Box::new(a), rhs: Box::new(b) }
}

fn lit(v: f64) -> PomExpr {
    PomExpr::Number { span: NOSPAN, value: NumberLit::Real(v) }
}

fn int(v: i64) -> PomExpr {
    PomExpr::Number { span: NOSPAN, value: NumberLit::Integer(v) }
}

fn str_lit(s: impl Into<String>) -> PomExpr {
    PomExpr::Str { span: NOSPAN, value: s.into() }
}

fn not_expr(e: PomExpr) -> PomExpr {
    PomExpr::Unary { span: NOSPAN, op: UnOp::LogNot, operand: Box::new(e) }
}

fn and_guards(guard: Option<&PomExpr>, cond: &PomExpr) -> PomExpr {
    match guard {
        None => cond.clone(),
        Some(g) => binary(BinOp::LogAnd, g.clone(), cond.clone()),
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
        let mut f = Self {
            module,
            scope: Scope::new(),
            out: FlatAnalog::default(),
            potential_acc: Vec::new(),
        };
        // Every module var starts as a temp reading its own persistent-bank
        // slot (`Ident(name)`); an assignment later shadows it with a new temp.
        // A node with the same name wins (a bare name in a `V()`/`I()` position
        // is a node): the lowering registers even block-local `var`s as module
        // symbols, so a depletion-charge temp named like a terminal (`var s`
        // vs the source node `s`) must not shadow the node here — the
        // block-local one is resolved through `locals` inside its own block.
        let is_node = |name: &str| module.symbols.nodes().any(|(_, n)| n.name == name);
        for (_, v) in module.symbols.vars() {
            if is_node(&v.name) {
                continue;
            }
            let id = f.alloc_temp(PomExpr::Ident { span: NOSPAN, name: v.name.clone() });
            f.scope.set(v.name.clone(), id);
        }
        f
    }

    /// Append a temporary and return its id. The expression may reference
    /// earlier temps via `__temp` leaves.
    fn alloc_temp(&mut self, expr: PomExpr) -> u32 {
        let id = self.out.temps.len() as u32;
        self.out.temps.push(expr);
        id
    }

    pub fn flatten(mut self) -> Result<FlatAnalog, CodegenError> {
        let body = self.module.analog.as_ref()
            .ok_or_else(|| CodegenError::Invalid(format!(
                "`{}` has no analog body", self.module.name)))?;
        self.walk(&body.stmts, None)?;

        let potentials = std::mem::take(&mut self.potential_acc);
        for (plus, minus, expr) in potentials {
            let (without, stim) = split_ac_stim(expr)?;
            let (expr, flux_terms, current_terms) = self.split_flux(without, plus, minus)?;
            let ac_stim = match stim {
                Some((mag, phase)) => Some((self.finish_expr(mag)?, self.finish_expr(phase)?)),
                None => None,
            };
            self.out.forces.push(FlatForce { plus, minus, expr, ac_stim, flux_terms, current_terms });
        }

        // A series-impedance term reads a branch current, which is an MNA
        // unknown only for forced branches — anything else would silently
        // stamp against a column that does not exist (fail loud, GAPS).
        let force_branches: Vec<(NodeId, NodeId)> =
            self.out.forces.iter().map(|f| (f.plus, f.minus)).collect();
        for force in &self.out.forces {
            for &(p, m, _) in &force.current_terms {
                let is_force = force_branches
                    .iter()
                    .any(|&(a, b)| (a, b) == (p, m) || (a, b) == (m, p));
                if !is_force {
                    return Err(CodegenError::unsupported(
                        "branch-current term in a potential force must probe a forced branch (`V(...) <+ ...`)",
                    ));
                }
            }
        }

        for source in &body.noise {
            let (psd_src, exponent_src) = match &source.kind {
                crate::resolve::NoiseKind::White { psd } => (psd.clone(), None),
                crate::resolve::NoiseKind::Flicker { psd, exponent } => (psd.clone(), Some(exponent.clone())),
            };
            let psd = self.subst(&psd_src)?;
            let psd = self.finish_expr(psd)?;
            let exponent = match exponent_src {
                Some(e) => Some(self.finish_expr(self.subst(&e)?)?),
                None => None,
            };
            self.out.noise.push((source.plus, source.minus, psd, exponent));
        }

        // Capture the final module-var → temp-id mapping for the opvar
        // compilation path (ABI-30). Only non-shadow module vars: a
        // `__shadow_*` var is a D2A bridge plumbing leak the host never
        // queries. The seeding loop in `new` placed every module var in
        // scope; an analog-body reassignment shadows it with a later temp
        // — so `scope.get` here reads the var's value at end of evaluation.
        let is_node = |name: &str| self.module.symbols.nodes().any(|(_, n)| n.name == name);
        for (_, v) in self.module.symbols.vars() {
            if v.name.starts_with("__shadow_") || is_node(&v.name) {
                continue;
            }
            if let Some(id) = self.scope.get(&v.name) {
                self.out.module_var_temps.push((v.name.clone(), id));
            }
        }

        Ok(self.out)
    }

    fn walk(&mut self, stmts: &[PomStmt], guard: Option<&PomExpr>) -> Result<(), CodegenError> {
        for stmt in stmts {
            match stmt {
                PomStmt::Contribution { target, value, .. } => {
                    self.add_contrib(target, value, guard)?;
                }
                PomStmt::Assign { target: PomExpr::Ident { name, .. }, value, .. }
                | PomStmt::NonBlockingAssign { target: PomExpr::Ident { name, .. }, value, .. } => {
                    // A guarded reassignment reads the previous temp as its
                    // else-branch: `x = g ? new : __temp(old)`.
                    let resolved = self.subst(value)?;
                    let resolved = match guard {
                        None => resolved,
                        Some(g) => {
                            let old = self.scope.get(name)
                                .map(temp_ref)
                                .unwrap_or_else(|| lit(0.0));
                            select(g.clone(), resolved, old)
                        }
                    };
                    let id = self.alloc_value_temp(resolved)?;
                    self.scope.set(name.clone(), id);
                }
                // A non-identifier assign target has no meaning in a
                // flattened analog body — nothing to record.
                PomStmt::Assign { .. } | PomStmt::NonBlockingAssign { .. } => {}
                PomStmt::Block { stmts, .. } => {
                    self.walk(stmts, guard)?;
                }
                PomStmt::If { cond, then_branch, else_branch, .. } => {
                    let cond = self.subst(cond)?;
                    let then_guard = and_guards(guard, &cond);
                    self.walk(std::slice::from_ref(then_branch.as_ref()), Some(&then_guard))?;
                    let else_guard = and_guards(guard, &not_expr(cond.clone()));
                    if let Some(eb) = else_branch {
                        self.walk(std::slice::from_ref(eb.as_ref()), Some(&else_guard))?;
                    }
                }
                PomStmt::Case { selector, arms, .. } => {
                    let scrutinee = self.subst(selector)?;
                    let mut no_prior = None::<PomExpr>;
                    for arm in arms {
                        let cond = case_arm_cond(&scrutinee, arm);
                        let arm_guard = chain_guards(guard, &no_prior, &cond);
                        self.walk(std::slice::from_ref(arm.body.as_ref()), Some(&arm_guard))?;
                        no_prior = Some(match no_prior {
                            None => not_expr(cond),
                            Some(prev) => binary(BinOp::LogAnd, prev, not_expr(cond)),
                        });
                    }
                }
                PomStmt::TaskCall { name, args, .. } => {
                    match name.trim_start_matches('$') {
                        "bound_step" => {
                            let val = match args.first() {
                                Some(a) => self.inline_temps(&self.subst(a)?),
                                None => lit(0.0),
                            };
                            let finished = self.finish_expr(val)?;
                            self.out.bound_steps.push(finished);
                        }
                        "finish" | "stop" => {
                            return Err(CodegenError::unsupported("$finish in an analog body"));
                        }
                        // §8.4.4/§9.5.3 (LRMC-C7-11, T42): folded into a
                        // fired-this-evaluation flag by `compile.rs`, not
                        // discarded. `degree` (LRM: which derivative order
                        // is discontinuous) is accepted but not consulted
                        // — every fire forces the same restart
                        // (`prev_h = 0`, drop to backward Euler for the
                        // next TR stage) regardless of degree; a
                        // degree-sensitive order selection is a documented
                        // scope limit (mirrors `$bound_step`'s own
                        // unconditional-collection precedent this arm
                        // otherwise follows exactly).
                        "discontinuity" => {
                            let _ = args;
                            let flag = match guard {
                                None => lit(1.0),
                                Some(g) => select(g.clone(), lit(1.0), lit(0.0)),
                            };
                            self.out.discontinuities.push(flag);
                        }
                        n @ ("display" | "write" | "strobe" | "monitor"
                            | "warning" | "warn" | "error" | "fatal" | "info") =>
                        {
                            let severity = match n {
                                "warning" | "warn" => crate::resolve::Severity::Warn,
                                "error" => crate::resolve::Severity::Error,
                                "fatal" => crate::resolve::Severity::Fatal,
                                _ => crate::resolve::Severity::Info,
                            };
                            let fmt = match args.first() {
                                Some(PomExpr::Str { value, .. }) => value.clone(),
                                _ => String::new(),
                            };
                            self.out.diagnostics.push(FlatDiagnostic { severity, format: fmt });
                        }
                        _ => {}
                    }
                }
                PomStmt::EventControl { events, body, .. } => {
                    // `@(initial_step) V(p, n) <+ value;` — the UIC seed
                    // (SPICE `.ic`): never a live contribution (its branch
                    // keeps the kind the unconditional contributions give
                    // it), routed to `FlatAnalog::initial_conditions` for
                    // the t=0 solve + UIC clamps. Anything else under an
                    // `initial_step` guard (an `Assign`, the PWM/thermostat
                    // idiom) stays an ordinary guarded event.
                    if events.iter().any(|e| {
                        matches!(e, piperine_lang::ast::EventExpr::Level(piperine_lang::ast::Expr::Ident { name, .. }) if name == "initial_step")
                    }) {
                        if let PomStmt::Contribution { target, value, .. } = body.as_ref() {
                            let (nature_kind, plus, minus) = self.parse_dest(target)?;
                            match nature_kind {
                                NatureKind::Potential => {
                                    let resolved = self.subst(value)?;
                                    self.out.initial_conditions.push((plus, minus, resolved));
                                    continue;
                                }
                                NatureKind::Flow => {
                                    return Err(CodegenError::unsupported(
                                        "`@(initial_step)` seed: only the potential form `V(a, b) <+ value;` seeds today",
                                    ));
                                }
                            }
                        }
                    }
                    for source in event_exprs_to_sources(events)? {
                        let event_body = match body.as_ref() {
                            PomStmt::Block { stmts, .. } => stmts.clone(),
                            other => vec![other.clone()],
                        };
                        let event = AnalogEvent { source, body: event_body };
                        self.add_event(&event, guard)?;
                    }
                }
                PomStmt::Null { .. } | PomStmt::Disable { .. } => {}
                PomStmt::For { .. } | PomStmt::While { .. } | PomStmt::Repeat { .. }
                | PomStmt::Forever { .. } | PomStmt::DelayControl { .. } => {
                    return Err(CodegenError::unsupported(
                        "runtime loop/delay-control statement in an analog body",
                    ));
                }
                // `-> e;` (§5.10.4, LRMC-C5-04): `e` is the hidden integer
                // var `elab::lower::vams` allocates per declared `event`
                // (T25/T26) — triggering toggles it (`e = 1 - e;`, 0/1
                // alternating) so any `Trigger::AnyChange` watcher (`@(e)`
                // here or in the digital domain, T26's own cross-domain
                // proof) sees a real value change. A toggle rather than an
                // unbounded increment: only one bit ever differs between 0
                // and 1, so a single trigger can't fan out into more than
                // one net-level change event on the digital side (see
                // `emit/stmt.rs`'s matching arm for the observed failure
                // mode this avoids). Desugars to an ordinary `Assign` and
                // recurses through the exact same var-write path — no
                // second mechanism.
                PomStmt::EventTrigger { span, name } => {
                    let target = PomExpr::Ident { span: *span, name: name.clone() };
                    let value = PomExpr::Binary {
                        span: *span,
                        op: BinOp::Sub,
                        lhs: Box::new(PomExpr::Number { span: *span, value: NumberLit::Integer(1) }),
                        rhs: Box::new(target.clone()),
                    };
                    self.walk(&[PomStmt::Assign { span: *span, target, value }], guard)?;
                }
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
                        *acc = binary(BinOp::Add, acc.clone(), guarded);
                    }
                    None => self.potential_acc.push((plus, minus, guarded)),
                }
                Ok(())
            }
        }
    }

    /// Split a potential contribution's value into `(resistive, flux_terms,
    /// current_terms)`. `… + L·ddt(I(p,n))` over the branch's *own* current
    /// is the inductor flux `Φ = L·I`: `L` is returned as a flux term and
    /// the branch equation gains `dΦ/dt`. A bare linear `R·I(branch)` read
    /// is a series-impedance term (`FlatForce::current_terms`) — exact
    /// `V = R·I`, never a penalty conductance. `ddt` of a *different*
    /// branch's current (mutual inductance) stays supported; a nonlinear
    /// flux/impedance or mixing `R·I` with `ddt` in one contribution is a
    /// loud error for now.
    #[allow(clippy::type_complexity)]
    fn split_flux(
        &mut self,
        value: PomExpr,
        _plus: NodeId,
        _minus: NodeId,
    ) -> Result<(PomExpr, Vec<(NodeId, NodeId, PomExpr)>, Vec<(NodeId, NodeId, PomExpr)>), CodegenError> {
        if !has_marker(&value, &["__ddt", "__laplace", "__ztransform"]) {
            let (rest, current_terms) = self.split_current_terms(value)?;
            return Ok((self.finish_expr(rest)?, Vec::new(), current_terms));
        }
        let resistive = substitute_marker(&value, &["__ddt"], false)?;
        if !collect_branch_current_pairs(&resistive).is_empty() {
            return Err(CodegenError::unsupported(
                "a branch-current term (`R·I(...)`) combined with `ddt` in one potential contribution",
            ));
        }
        let flux = substitute_marker(&value, &["__ddt"], true)?;
        // The flux `Φ` must be linear in branch currents: `Φ = Σ coeff_k·I(k)`.
        // Extract each `coeff_k` by setting that branch's current to 1 and every
        // other branch's to 0.
        let branches = collect_branch_current_pairs(&flux);
        if branches.is_empty() {
            return Err(CodegenError::unsupported(
                "ddt in a potential contribution must integrate a branch current (`ddt(I(p,n))`)",
            ));
        }
        let mut terms = Vec::new();
        for (a, b) in &branches {
            let coeff = isolate_branch_coeff(&flux, a, b, &branches);
            if has_branch_current(&coeff) {
                return Err(CodegenError::unsupported(
                    "nonlinear flux (branch current inside the flux coefficient) is not supported",
                ));
            }
            let (pa, pb) = (self.resolve_node(a), self.resolve_node(b));
            terms.push((pa, pb, self.finish_expr(coeff)?));
        }
        Ok((self.finish_expr(resistive)?, terms, Vec::new()))
    }

    /// Split the linear `coeff·I(branch)` (series-impedance) terms out of a
    /// non-reactive potential-contribution value. Returns the remainder
    /// (every listed branch current set to 0) and one `(plus, minus,
    /// coeff)` term per distinct branch, with `coeff = value@[I_k=1, others
    /// 0] − value@[all 0]` — exact for a value affine in the branch
    /// currents; a coefficient that still reads a branch current
    /// (nonlinear impedance) is a loud error.
    #[allow(clippy::type_complexity)]
    fn split_current_terms(
        &mut self,
        value: PomExpr,
    ) -> Result<(PomExpr, Vec<(NodeId, NodeId, PomExpr)>), CodegenError> {
        let pairs = collect_branch_current_pairs(&value);
        if pairs.is_empty() {
            return Ok((value, Vec::new()));
        }
        let rest = zero_branch_currents(&value, &pairs);
        let mut terms = Vec::new();
        for (a, b) in &pairs {
            let at_one = isolate_branch_coeff(&value, a, b, &pairs);
            let coeff = binary(BinOp::Sub, at_one, rest.clone());
            if has_branch_current(&coeff) {
                return Err(CodegenError::unsupported(
                    "nonlinear branch-current impedance (I(...) inside its own coefficient) in a potential contribution",
                ));
            }
            let (pa, pb) = (self.resolve_node(a), self.resolve_node(b));
            terms.push((pa, pb, self.finish_expr(coeff)?));
        }
        Ok((rest, terms))
    }

    fn parse_dest(&self, dest: &PomExpr) -> Result<(NatureKind, NodeId, NodeId), CodegenError> {
        if let PomExpr::AccessFunction { function, pos, neg, .. } = dest {
            let nature_kind = match function.as_str() {
                "V" => NatureKind::Potential,
                _ => NatureKind::Flow,
            };
            let plus_name = ident_str(Some(pos)).unwrap_or_else(|| "?".into());
            let minus_name = neg.as_deref().and_then(|n| ident_str(Some(n))).unwrap_or_else(|| "0".into());
            let plus = self.resolve_node(&plus_name);
            let minus = self.resolve_node(&minus_name);
            return Ok((nature_kind, plus, minus));
        }
        // Anything that is not an access-function call. A bare-identifier
        // target is the natural spelling for driving a digital net from an
        // analog body — but analog contributions only ever target branches
        // (LRM §5.4.1), so this is a hard error, not a silent no-op.
        let spelled = match dest {
            PomExpr::Ident { name, .. } => {
                format!("`{}`", name.strip_prefix("__shadow_").unwrap_or(name))
            }
            other => format!("{other:?}"),
        };
        Err(CodegenError::unsupported(format!(
            "analog contribution target {spelled} is not an access function. \
             `V(a, b) <+ …` / `I(a, b) <+ …` are the analog contribution forms; \
             driving a net by bare name from an `analog` body is not lowered. \
             To drive a digital net from analog state, put the comparison in \
             the module's digital body and read the analog side there with \
             `V(...)` (the A2D bridge)."
        )))
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
            let charge = binary(BinOp::Sub, with_arg, with_zero);
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
            if let PomExpr::Call { name, args, .. } = e {
                match name.as_str() {
                    "__ddx" => {
                        // __ddx(id, x, node_id) → d_dnode(x, node)
                        if args.len() >= 3 {
                            let x = &args[1];
                            let node_id = match &args[2] {
                                PomExpr::Number { value: NumberLit::Integer(n), .. } => NodeId(*n as u32),
                                _ => NodeId::GROUND,
                            };
                            let module = self.module;
                            let resolve = |n: &str| -> Option<NodeId> {
                                module.symbols.nodes().find(|(_, info)| info.name == n).map(|(id, _)| id)
                            };
                            return crate::resolve::diff::d_dnode(x, node_id, &resolve);
                        }
                        return lit(0.0);
                    }
                    "__idt" | "__idtmod" => {
                        // __idt(id, x, ic[, modulus]) → __state_load(id) + step * x
                        if args.len() >= 2 {
                            let id = match &args[0] {
                                PomExpr::Number { value: NumberLit::Integer(n), .. } => StateId(*n as u32),
                                _ => return e.clone(),
                            };
                            let x = &args[1];
                            if !self.out.runtime_states.iter().any(|(s, _)| *s == id) {
                                self.out.runtime_states.push((id, x.clone()));
                            }
                            let state_load = call("__state_load", vec![int(id.0 as i64)]);
                            let step = PomExpr::SystemFunctionCall {
                                span: NOSPAN,
                                name: "$simparam".to_string(),
                                args: vec![str_lit("step"), lit(0.0)],
                            };
                            return binary(BinOp::Add, state_load, binary(BinOp::Mul, step, x.clone()));
                        }
                        return e.clone();
                    }
                    "__delay" | "__slew" | "__transition" => {
                        // Runtime state: __op(id, x, ...) → __state_load(id), register input.
                        if let Some(PomExpr::Number { value: NumberLit::Integer(id), .. }) = args.first() {
                            let sid = StateId(*id as u32);
                            if !self.out.runtime_states.iter().any(|(s, _)| *s == sid) {
                                let x = args.get(1).cloned().unwrap_or_else(|| lit(0.0));
                                self.out.runtime_states.push((sid, x));
                            }
                            return call("__state_load", vec![int(*id)]);
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

    /// Resolve `var` references to `__temp` leaves. Never inlines — the
    /// output is linear in the size of `expr`.
    fn subst(&self, expr: &PomExpr) -> Result<PomExpr, CodegenError> {
        Ok(subst_scope(expr, &self.scope))
    }

    /// Introduce a value temporary from an already-substituted expression:
    /// expand runtime-operator markers (`finish_expr`), reject reactive
    /// markers (`ddt`/`ac_stim` belong in a contribution, not a `var`), and
    /// append it to the temp tape.
    fn alloc_value_temp(&mut self, value: PomExpr) -> Result<u32, CodegenError> {
        if has_marker(&value, &["__ddt", "__laplace", "__ztransform"]) {
            return Err(CodegenError::unsupported(
                "ddt/laplace inside a `var` — write the reactive term directly in the contribution",
            ));
        }
        let value = self.finish_expr(value)?;
        Ok(self.alloc_temp(value))
    }

    /// Fully expand `__temp` leaves back into an inlined expression. Used
    /// only for analog-event action values, which are evaluated outside the
    /// contribution temp tape (against the persistent var bank). Event
    /// expressions are small, so this stays bounded.
    fn inline_temps(&self, expr: &PomExpr) -> PomExpr {
        rewrite_expr(expr, &mut |e| {
            if let PomExpr::Call { name, args, .. } = e
                && name == "__temp"
                && let Some(PomExpr::Number { value: NumberLit::Integer(id), .. }) = args.first()
            {
                return self.inline_temps(&self.out.temps[*id as usize]);
            }
            e.clone()
        })
    }

    fn add_event(&mut self, event: &AnalogEvent, guard: Option<&PomExpr>) -> Result<(), CodegenError> {
        let trigger = match &event.source {
            EventSource::InitialStep => FlatEventTrigger::Initial,
            EventSource::FinalStep => {
                self.out.diagnostics.push(FlatDiagnostic {
                    severity: crate::resolve::Severity::Info,
                    format: String::new(),
                });
                return Ok(());
            }
            EventSource::Cross { expr, dir } => {
                let e = self.inline_temps(&self.subst(expr)?);
                FlatEventTrigger::Cross { expr: self.finish_expr(e)?, dir: *dir }
            }
            EventSource::Above { expr } => {
                let e = self.inline_temps(&self.subst(expr)?);
                FlatEventTrigger::Above { expr: self.finish_expr(e)? }
            }
            EventSource::Timer { period, phase } => {
                let ep = self.inline_temps(&self.subst(period)?);
                let ph = self.inline_temps(&self.subst(phase)?);
                FlatEventTrigger::Timer {
                    period: self.finish_expr(ep)?,
                    phase: self.finish_expr(ph)?,
                }
            }
            EventSource::AbsDelta { expr, delta, time_tol, expr_tol, enable } => {
                let e = self.inline_temps(&self.subst(expr)?);
                let d = self.inline_temps(&self.subst(delta)?);
                let tt = self.inline_temps(&self.subst(time_tol)?);
                let et = self.inline_temps(&self.subst(expr_tol)?);
                let en = self.inline_temps(&self.subst(enable)?);
                FlatEventTrigger::AbsDelta {
                    expr: self.finish_expr(e)?,
                    delta: self.finish_expr(d)?,
                    time_tol: self.finish_expr(tt)?,
                    expr_tol: self.finish_expr(et)?,
                    enable: self.finish_expr(en)?,
                }
            }
            EventSource::Named { expr } => {
                let e = self.inline_temps(&self.subst(expr)?);
                FlatEventTrigger::Named { expr: self.finish_expr(e)? }
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
                PomStmt::Assign { target: PomExpr::Ident { name, .. }, value, .. }
                | PomStmt::NonBlockingAssign { target: PomExpr::Ident { name, .. }, value, .. } => {
                    if let Some(var_id) = self.module.symbols.vars()
                        .find(|(_, v)| &v.name == name).map(|(id, _)| id)
                    {
                        // Event action values run against the persistent
                        // var bank, not the contribution temp tape, so
                        // inline temps back to bank-referencing form.
                        let value = self.inline_temps(&self.subst(value)?);
                        let value = self.finish_expr(value)?;
                        let value = match guard {
                            None => value,
                            Some(g) => select(self.inline_temps(g), value,
                                PomExpr::Ident { span: NOSPAN, name: name.clone() }),
                        };
                        actions.push(FlatEventAction { var: var_id, value });
                    }
                }
                PomStmt::Block { stmts, .. } => self.collect_event_actions(stmts, guard, actions)?,
                PomStmt::If { cond, then_branch, else_branch, .. } => {
                    let cond = self.subst(cond)?;
                    let then_guard = and_guards(guard, &cond);
                    self.collect_event_actions(std::slice::from_ref(then_branch.as_ref()), Some(&then_guard), actions)?;
                    let else_guard = and_guards(guard, &not_expr(cond));
                    if let Some(eb) = else_branch {
                        self.collect_event_actions(std::slice::from_ref(eb.as_ref()), Some(&else_guard), actions)?;
                    }
                }
                // `-> e;` inside an event action body (e.g. `@(cross(...))
                // -> e;`, chaining one event into another) — same toggle
                // desugaring as the main-body `walk` arm, routed through
                // this function's own Assign handling instead of
                // recursing into `walk` (this function's own scope).
                PomStmt::EventTrigger { span, name } => {
                    let target = PomExpr::Ident { span: *span, name: name.clone() };
                    let value = PomExpr::Binary {
                        span: *span,
                        op: BinOp::Sub,
                        lhs: Box::new(PomExpr::Number { span: *span, value: NumberLit::Integer(1) }),
                        rhs: Box::new(target.clone()),
                    };
                    self.collect_event_actions(
                        &[PomStmt::Assign { span: *span, target, value }],
                        guard,
                        actions,
                    )?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Substitute a `var` reference `Ident(name)` with its `__temp(id)` leaf.
/// Non-var idents (params, nodes) pass through untouched. Crucially this
/// does **not** inline module-level vars — that is what keeps the tree
/// linear. Block-local `var`s (VAMS `BlockItemDecl`, name-only — no
/// initializer syntax) are not separately tracked: since a local
/// declaration carries no default expression, its first use is always
/// preceded by an ordinary `Assign`, which the caller (`walk`) already
/// allocates a temp for exactly like a module-level reassignment — there is
/// no PHDL-style "block-local inlined value" case left to handle here.
fn subst_scope(expr: &PomExpr, scope: &Scope) -> PomExpr {
    match expr {
        PomExpr::Ident { name, .. } => match scope.get(name) {
            Some(id) => temp_ref(id),
            None => expr.clone(),
        },
        PomExpr::Unary { span, op, operand } => {
            PomExpr::Unary { span: *span, op: *op, operand: Box::new(subst_scope(operand, scope)) }
        }
        PomExpr::Binary { span, op, lhs, rhs } => PomExpr::Binary {
            span: *span,
            op: *op,
            lhs: Box::new(subst_scope(lhs, scope)),
            rhs: Box::new(subst_scope(rhs, scope)),
        },
        PomExpr::Ternary { span, cond, then_branch, else_branch } => PomExpr::Ternary {
            span: *span,
            cond: Box::new(subst_scope(cond, scope)),
            then_branch: Box::new(subst_scope(then_branch, scope)),
            else_branch: Box::new(subst_scope(else_branch, scope)),
        },
        PomExpr::Call { span, name, args } => PomExpr::Call {
            span: *span,
            name: name.clone(),
            args: args.iter().map(|a| subst_scope(a, scope)).collect(),
        },
        PomExpr::SystemFunctionCall { span, name, args } => PomExpr::SystemFunctionCall {
            span: *span,
            name: name.clone(),
            args: args.iter().map(|a| subst_scope(a, scope)).collect(),
        },
        PomExpr::AccessFunction { span, function, pos, neg } => PomExpr::AccessFunction {
            span: *span,
            function: function.clone(),
            pos: Box::new(subst_scope(pos, scope)),
            neg: neg.as_ref().map(|n| Box::new(subst_scope(n, scope))),
        },
        PomExpr::Index { span, base, index } => PomExpr::Index {
            span: *span,
            base: Box::new(subst_scope(base, scope)),
            index: Box::new(subst_scope(index, scope)),
        },
        PomExpr::PartSelect { span, base, msb, lsb } => PomExpr::PartSelect {
            span: *span,
            base: Box::new(subst_scope(base, scope)),
            msb: Box::new(subst_scope(msb, scope)),
            lsb: Box::new(subst_scope(lsb, scope)),
        },
        PomExpr::Concat { span, parts } => {
            PomExpr::Concat { span: *span, parts: parts.iter().map(|p| subst_scope(p, scope)).collect() }
        }
        PomExpr::Replication { span, count, parts } => PomExpr::Replication {
            span: *span,
            count: Box::new(subst_scope(count, scope)),
            parts: parts.iter().map(|p| subst_scope(p, scope)).collect(),
        },
        PomExpr::AssignmentPattern { span, replicate, parts } => PomExpr::AssignmentPattern {
            span: *span,
            replicate: replicate.as_ref().map(|r| Box::new(subst_scope(r, scope))),
            parts: parts.iter().map(|p| subst_scope(p, scope)).collect(),
        },
        // Never reaches codegen — see this file's other PortAccess arms.
        PomExpr::Number { .. } | PomExpr::Str { .. } | PomExpr::PortAccess { .. } => expr.clone(),
    }
}

/// Check if an expression contains any of the named marker calls.
/// The distinct branch-current node pairs `(a,b)` a flux expression reads
/// (orientation from the first occurrence; `I(b,a)` folds onto `(a,b)`).
fn collect_branch_current_pairs(expr: &PomExpr) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    visit_all(expr, &mut |e| {
        if let PomExpr::AccessFunction { function, pos, neg: Some(neg), .. } = e
            && function == "I"
            && let (PomExpr::Ident { name: a, .. }, PomExpr::Ident { name: b, .. }) = (pos.as_ref(), neg.as_ref())
            && !pairs.iter().any(|(x, y)| (x == a && y == b) || (x == b && y == a))
        {
            pairs.push((a.clone(), b.clone()));
        }
    });
    pairs
}

/// Isolate the coefficient of `I(a,b)` in a flux linear in branch currents:
/// set that branch's current to 1 (`I(b,a) → −1`) and every *other* listed
/// branch's current to 0.
fn isolate_branch_coeff(expr: &PomExpr, a: &str, b: &str, all: &[(String, String)]) -> PomExpr {
    rewrite_expr(expr, &mut |e| {
        if let PomExpr::AccessFunction { function, pos, neg: Some(neg), .. } = e
            && function == "I"
            && let (PomExpr::Ident { name: x, .. }, PomExpr::Ident { name: y, .. }) = (pos.as_ref(), neg.as_ref())
        {
            if x == a && y == b {
                return lit(1.0);
            }
            if x == b && y == a {
                return lit(-1.0);
            }
            if all.iter().any(|(p, q)| (p == x && q == y) || (p == y && q == x)) {
                return lit(0.0);
            }
        }
        e.clone()
    })
}

/// Set every listed branch's current `I(a,b)` (either orientation) to 0 —
/// the "remainder" of an expression affine in those branch currents.
fn zero_branch_currents(expr: &PomExpr, all: &[(String, String)]) -> PomExpr {
    rewrite_expr(expr, &mut |e| {
        if let PomExpr::AccessFunction { function, pos, neg: Some(neg), .. } = e
            && function == "I"
            && let (PomExpr::Ident { name: x, .. }, PomExpr::Ident { name: y, .. }) = (pos.as_ref(), neg.as_ref())
            && all.iter().any(|(p, q)| (p == x && q == y) || (p == y && q == x))
        {
            return lit(0.0);
        }
        e.clone()
    })
}

/// True if the expression still reads any branch current `I(...)`.
fn has_branch_current(expr: &PomExpr) -> bool {
    let mut found = false;
    visit_all(expr, &mut |e| {
        if let PomExpr::AccessFunction { function, .. } = e
            && function == "I"
        {
            found = true;
        }
    });
    found
}

fn has_marker(expr: &PomExpr, names: &[&str]) -> bool {
    let mut found = false;
    visit_all(expr, &mut |e| {
        if let PomExpr::Call { name, .. } = e
            && names.contains(&name.as_str()) {
                found = true;
            }
    });
    found
}

/// Replace marker calls: `__ddt(id, x)` → `x` (with_arg=true) or `0.0` (false).
fn substitute_marker(expr: &PomExpr, names: &[&str], with_arg: bool) -> Result<PomExpr, CodegenError> {
    Ok(rewrite_expr(expr, &mut |e| {
        if let PomExpr::Call { name, args, .. } = e
            && names.contains(&name.as_str()) {
                if with_arg {
                    return args.get(1).cloned().unwrap_or_else(|| lit(0.0));
                } else {
                    return lit(0.0);
                }
            }
        e.clone()
    }))
}

/// Bottom-up rewrite: children first, then `f` on the rebuilt node.
pub(crate) fn rewrite_expr(expr: &PomExpr, f: &mut impl FnMut(&PomExpr) -> PomExpr) -> PomExpr {
    let rewritten = match expr {
        // PortAccess never reaches codegen: elab::lower::vams rewrites it
        // into a sum of AccessFunction reads before a Module leaves the
        // elaborator (LRMC-C1-10).
        PomExpr::Number { .. } | PomExpr::Str { .. } | PomExpr::Ident { .. } | PomExpr::PortAccess { .. } => {
            expr.clone()
        }
        PomExpr::SystemFunctionCall { span, name, args } => PomExpr::SystemFunctionCall {
            span: *span,
            name: name.clone(),
            args: args.iter().map(|a| rewrite_expr(a, f)).collect(),
        },
        PomExpr::Unary { span, op, operand } => {
            PomExpr::Unary { span: *span, op: *op, operand: Box::new(rewrite_expr(operand, f)) }
        }
        PomExpr::Binary { span, op, lhs, rhs } => PomExpr::Binary {
            span: *span,
            op: *op,
            lhs: Box::new(rewrite_expr(lhs, f)),
            rhs: Box::new(rewrite_expr(rhs, f)),
        },
        PomExpr::Call { span, name, args } => {
            let args: Vec<_> = args.iter().map(|a| rewrite_expr(a, f)).collect();
            PomExpr::Call { span: *span, name: name.clone(), args }
        }
        PomExpr::AccessFunction { span, function, pos, neg } => PomExpr::AccessFunction {
            span: *span,
            function: function.clone(),
            pos: Box::new(rewrite_expr(pos, f)),
            neg: neg.as_ref().map(|n| Box::new(rewrite_expr(n, f))),
        },
        PomExpr::Ternary { span, cond, then_branch, else_branch } => PomExpr::Ternary {
            span: *span,
            cond: Box::new(rewrite_expr(cond, f)),
            then_branch: Box::new(rewrite_expr(then_branch, f)),
            else_branch: Box::new(rewrite_expr(else_branch, f)),
        },
        PomExpr::Index { span, base, index } => PomExpr::Index {
            span: *span,
            base: Box::new(rewrite_expr(base, f)),
            index: Box::new(rewrite_expr(index, f)),
        },
        PomExpr::PartSelect { span, base, msb, lsb } => PomExpr::PartSelect {
            span: *span,
            base: Box::new(rewrite_expr(base, f)),
            msb: Box::new(rewrite_expr(msb, f)),
            lsb: Box::new(rewrite_expr(lsb, f)),
        },
        PomExpr::Concat { span, parts } => {
            PomExpr::Concat { span: *span, parts: parts.iter().map(|p| rewrite_expr(p, f)).collect() }
        }
        PomExpr::Replication { span, count, parts } => PomExpr::Replication {
            span: *span,
            count: Box::new(rewrite_expr(count, f)),
            parts: parts.iter().map(|p| rewrite_expr(p, f)).collect(),
        },
        PomExpr::AssignmentPattern { span, replicate, parts } => PomExpr::AssignmentPattern {
            span: *span,
            replicate: replicate.as_ref().map(|r| Box::new(rewrite_expr(r, f))),
            parts: parts.iter().map(|p| rewrite_expr(p, f)).collect(),
        },
    };
    f(&rewritten)
}

fn chain_guards(guard: Option<&PomExpr>, no_prior: &Option<PomExpr>, cond: &PomExpr) -> PomExpr {
    let with_prior = match no_prior {
        None => cond.clone(),
        Some(prev) => binary(BinOp::LogAnd, prev.clone(), cond.clone()),
    };
    and_guards(guard, &with_prior)
}

/// The path condition for one `case` arm: `false` for an empty label list
/// (unreachable — the grammar requires at least one label or `default`),
/// the OR of `scrutinee == label` for a labeled arm, `true` for the
/// `default` arm (`labels: None`).
fn case_arm_cond(scrutinee: &PomExpr, arm: &piperine_lang::ast::CaseArm) -> PomExpr {
    match &arm.labels {
        None => lit(1.0),
        Some(labels) => labels.iter().fold(lit(0.0), |acc, label| {
            let eq = binary(BinOp::Eq, scrutinee.clone(), label.clone());
            if matches!(acc, PomExpr::Number { value: NumberLit::Real(v), .. } if v == 0.0) {
                eq
            } else {
                binary(BinOp::LogOr, acc, eq)
            }
        }),
    }
}

fn ident_str(e: Option<&PomExpr>) -> Option<String> {
    match e? {
        PomExpr::Ident { name, .. } => Some(name.clone()),
        _ => None,
    }
}

/// Split a contribution expression around its `$ac_stim` SysCalls.
///
/// A single stimulus keeps the direct path: magnitude extracted as
/// `with_mag − without` (exact for the linear-in-stimulus embeddings
/// contributions use), phase taken from the call. Multiple stimuli sum as
/// phasors: each occurrence's magnitude is extracted the same way (that
/// occurrence at its mag, the rest at 0), then `(re, im) = Σ mag_k·(cos φ_k,
/// sin φ_k)` and `(mag, phase) = (hypot(re, im), atan2(im, re))`.
fn split_ac_stim(expr: PomExpr) -> Result<(PomExpr, Option<(PomExpr, PomExpr)>), CodegenError> {
    let mut count = 0usize;
    visit_all(&expr, &mut |e| {
        if let PomExpr::SystemFunctionCall { name, .. } = e
            && name == "$ac_stim" {
                count += 1;
            }
    });
    if count == 0 {
        return Ok((expr, None));
    }
    let without = rewrite_expr(&expr, &mut |e| {
        if let PomExpr::SystemFunctionCall { name, .. } = e
            && name == "$ac_stim" {
                return lit(0.0);
            }
        e.clone()
    });
    let math1 = |name: &str, a: PomExpr| call(name, vec![a]);
    let math2 = |name: &str, a: PomExpr, b: PomExpr| call(name, vec![a, b]);
    if count == 1 {
        let mut phase_expr = None;
        let with_mag = rewrite_expr(&expr, &mut |e| {
            if let PomExpr::SystemFunctionCall { name, args, .. } = e
                && name == "$ac_stim" {
                    phase_expr = args.get(1).cloned();
                    return args.first().cloned().unwrap_or_else(|| lit(1.0));
                }
            e.clone()
        });
        let mag = binary(BinOp::Sub, with_mag, without.clone());
        let phase = phase_expr.unwrap_or_else(|| lit(0.0));
        return Ok((without, Some((mag, phase))));
    }
    // Phasor sum: one magnitude extraction per occurrence, then rectangular
    // accumulation so differing phases cancel/reinforce correctly.
    let mut re = lit(0.0);
    let mut im = lit(0.0);
    for k in 0..count {
        let mut seen = 0usize;
        let mut phase_k = lit(0.0);
        let with_k = rewrite_expr(&expr, &mut |e| {
            if let PomExpr::SystemFunctionCall { name, args, .. } = e
                && name == "$ac_stim" {
                    let this = seen == k;
                    if this {
                        phase_k = args.get(1).cloned().unwrap_or_else(|| lit(0.0));
                    }
                    seen += 1;
                    return if this { args.first().cloned().unwrap_or_else(|| lit(1.0)) } else { lit(0.0) };
                }
            e.clone()
        });
        let mag_k = binary(BinOp::Sub, with_k, without.clone());
        re = binary(BinOp::Add, re, binary(BinOp::Mul, mag_k.clone(), math1("cos", phase_k.clone())));
        im = binary(BinOp::Add, im, binary(BinOp::Mul, mag_k, math1("sin", phase_k)));
    }
    let mag = math2("hypot", re.clone(), im.clone());
    let phase = math2("atan2", im, re);
    Ok((without, Some((mag, phase))))
}
