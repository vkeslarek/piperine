//! Events and the heads of reactions (part IV §12.1, §13; DIG-09 AC 6 to 9).
//!
//! A head is put in disjunctive form, `d₁ || d₂ || …`, each `dₖ` an event source and its guard.
//! `c.changed()` is `Changed(c)`; `c.rise()` and `c.fall()` are `Changed(c)` guarded by the
//! two conditions of the stdlib text (§22.1):
//!
//! | Edge | Guard |
//! | --- | --- |
//! | `rise` | `!Previous(c) && c`: the previous value was false and the value is true |
//! | `fall` | `Previous(c) && !c`: the previous value was true and the value is false |
//!
//! `cross(e)` is `Cross(e)` and `at(t)` is `At(t)`. A guard `ev && g₁ && g₂` is split at its
//! top-level `&&` into conjuncts, each lowered on its own: one that reads a probe, `now()` or
//! `analysis(Static)` is **continuous** and becomes an `if` around the reaction's body (abi.md
//! F2: only the discrete part of a guard is a discrete function); the others stay on the event.
//!
//! | Head | Reactions |
//! | --- | --- |
//! | no disjunct has a continuous conjunct | one, on every disjunct's event |
//! | one disjunct, with continuous conjuncts | one, its body under `if` |
//! | several disjuncts, one with continuous conjuncts | one, on every disjunct's event, its body under `(fired(e₁) && c₁) \|\| (fired(e₂) && c₂) …` |
//!
//! A `ttol` or `etol` of `cross` (positional or by name) is `None`, a constant or a param, lowered
//! onto the event; one that reads a carrier, a probe or a local is `P0001`.
//!
//! Continuity is read off the lowered IR (an expression is continuous when it reads a probe, a
//! branch flow, `time()`, `static()`, `gmin()` or `source_scale()`, directly or through an operand
//! or a function), so a value input of a hoisted module is judged by what its caller gave.

use piperine_ir::{BinaryOp, Env, Event, EventSource, Expr, ExprId as LoweredExpr, Origin, Reaction, Stmt, UnaryOp, ValueType};

use super::elaborator::Elaboration;
use super::elements::{ElementRef, Lookup};
use super::lower::Scope;
use crate::pom::{BinOp, DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtId};
use crate::types::{Builtin, Callee};

/// One disjunct of a head: what fires it, and its guard split in two.
pub(super) struct Disjunct {
    pub(super) source: EventSource,
    /// The conjuncts the event holds.
    pub(super) discrete: Vec<LoweredExpr>,
    /// The conjuncts that read a probe, `now()` or `analysis(Static)`: the `if` around the body.
    pub(super) continuous: Vec<LoweredExpr>,
    pub(super) origin: Origin,
}

impl Elaboration<'_, '_> {
    /// Whether the lowered expression `expr` is continuous.
    pub(super) fn continuous(&self, expr: LoweredExpr) -> bool {
        self.expr_continuous.get(expr.index()).copied().unwrap_or(false)
    }

    /// Whether an expression of kind `kind`, whose operands are lowered already, is continuous.
    pub(super) fn continuous_kind(&self, kind: &Expr) -> bool {
        let any = |es: &[LoweredExpr]| es.iter().any(|e| self.continuous(*e));
        match kind {
            Expr::Probe { .. } | Expr::Flow { .. } | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::Env(Env::Time | Env::Static | Env::Gmin | Env::SourceScale | Env::Freq) => true,
            Expr::Env(Env::Temperature | Env::TemperatureGiven | Env::Analysis(_)) | Expr::SessionParameter { .. } | Expr::Fired(_) | Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::ParamPrevious(_) | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Previous(_) => false,
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::Table { x: expr, .. } | Expr::TableSlope { x: expr, .. } => self.continuous(*expr),
            Expr::CarrierElement { index, .. } | Expr::ElementPrevious { index, .. } | Expr::ElementChanged { index, .. } => self.continuous(*index),
            Expr::Binary { lhs, rhs, .. } => any(&[*lhs, *rhs]),
            Expr::Unwrap { is, value, .. } => any(&[*is, *value]),
            Expr::Same { a, b } => any(&[*a, *b]),
            Expr::LevelCast { expr, .. } => self.continuous(*expr),
            Expr::If { cond, then, otherwise } => any(&[*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => stmts.iter().any(|s| self.stmt_continuous.get(s.index()).copied().unwrap_or(false)) || value.is_some_and(|v| self.continuous(v)),
            Expr::Call { function, args } => self.variants.continuous(*function) || any(args),
            Expr::Math { args, .. } => any(args),
            Expr::Method { receiver, args, .. } => self.continuous(*receiver) || any(args),
            Expr::Closure { body, .. } => self.continuous(*body),
            Expr::Limit { x, f, seed, .. } => any(&[*x, *f]) || seed.is_some_and(|s| self.continuous(s)),
            Expr::Stim { mag, phase, .. } => self.continuous(*mag) || phase.is_some_and(|p| self.continuous(p)),
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) => {
                any(&kind.operands()) || kind.statements().iter().any(|s| self.stmt_continuous.get(s.index()).copied().unwrap_or(false))
            }
        }
    }

    /// The `digital` blocks of `module`, read in `scope`: their reactions, in source order.
    pub(super) fn lower_digital(&mut self, scope: &mut Scope, module: DeclId) {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return };
        for block in data.behaviors.clone() {
            let decl = self.design.decl_data(block).clone();
            let DeclKind::Behavior(behavior) = &decl.kind else { continue };
            if behavior.kind != crate::pom::BehaviorKind::Digital {
                continue;
            }
            for (child, iteration) in self.digital_sequence(&decl.children) {
                if !matches!(self.design.decl_data(child).kind, DeclKind::Reaction(_)) {
                    continue;
                }
                // a reaction made by a generation `for` reads its loop variables as the constants of its iteration
                for (variable, value) in &iteration.values {
                    scope.unrolled.bind(*variable, piperine_ir::Constant::Int(*value));
                }
                scope.ordinals = iteration.ordinals.clone();
                self.lower_reaction(scope, child);
                scope.ordinals.clear();
                for (variable, _) in &iteration.values {
                    scope.unrolled.unbind(*variable);
                }
            }
        }
    }

    /// `on head { body }`: its events and one reaction, or one reaction per disjunct (see the
    /// table of this module).
    fn lower_reaction(&mut self, scope: &Scope, reaction: DeclId) {
        let data = self.design.decl_data(reaction).clone();
        let DeclKind::Reaction(r) = &data.kind else { return };
        let origin = self.locator.decl(reaction);
        let mut head = Self::sub_scope(scope);
        let Some(disjuncts) = self.disjuncts(&mut head, r.event) else { return };
        let split = disjuncts.len() > 1 && disjuncts.iter().any(|d| !d.continuous.is_empty());
        if !split {
            let mut on = Vec::new();
            let mut wrap = Vec::new();
            for d in disjuncts {
                wrap.extend(d.continuous.iter().copied());
                on.push(self.add_event(d.source, &d.discrete, d.origin));
            }
            if let Some(stmts) = self.reaction_body(&mut head, &r.body.stmts, &wrap) {
                self.structure.reactions.push(Reaction { on, stmts, locals: head.locals, origin });
            }
            return;
        }
        let span = self.design.expr_node(r.event).span;
        if !head.locals.is_empty() {
            self.unsupported(scope.file, span, "a `let` in the guard of a head split per disjunct");
            return;
        }
        // One reaction on every disjunct's event, run once per microstep whichever fired (abi.md
        // R1), its body under `(fired(e₁) && c₁) || (fired(e₂) && c₂) || …`: `cₖ` the disjunct's
        // continuous conjuncts.
        let mut on = Vec::new();
        let mut arms = Vec::new();
        for d in disjuncts {
            let event = self.add_event(d.source, &d.discrete, d.origin.clone());
            on.push(event);
            if let Some(arm) = self.fired_arm(event, d.continuous, d.origin) {
                arms.push(arm);
            }
        }
        let guard = self.disjunction(&arms);
        let mut body = Self::sub_scope(scope);
        if let Some(stmts) = self.reaction_body_under(&mut body, &r.body.stmts, |_| guard) {
            self.structure.reactions.push(Reaction { on, stmts, locals: body.locals, origin });
        }
    }

    /// Add the event of `source` guarded by the conjunction of `discrete`: its index.
    pub(super) fn add_event(&mut self, source: EventSource, discrete: &[LoweredExpr], origin: Origin) -> u32 {
        let guard = self.conjunction(discrete);
        self.structure.events.push(Event { source, guard, origin });
        self.structure.events.len() as u32 - 1
    }

    /// `a && b && …` over lowered `bool`s, left to right. `None` for no operand.
    pub(super) fn conjunction(&mut self, operands: &[LoweredExpr]) -> Option<LoweredExpr> {
        let mut iter = operands.iter().copied();
        let first = iter.next()?;
        Some(iter.fold(first, |acc, next| {
            let origin = self.ir_origin(next);
            self.emit(Expr::Binary { op: BinaryOp::And, lhs: acc, rhs: next }, ValueType::Bool, origin)
        }))
    }

    /// `a || b || …` over lowered `bool`s, left to right. `None` for no operand.
    pub(super) fn disjunction(&mut self, operands: &[LoweredExpr]) -> Option<LoweredExpr> {
        let mut iter = operands.iter().copied();
        let first = iter.next()?;
        Some(iter.fold(first, |acc, next| {
            let origin = self.ir_origin(next);
            self.emit(Expr::Binary { op: BinaryOp::Or, lhs: acc, rhs: next }, ValueType::Bool, origin)
        }))
    }

    /// The origin an expression already added was given.
    fn ir_origin(&self, expr: LoweredExpr) -> Origin {
        self.expr_origins[expr.index()].clone()
    }

    /// The body of a reaction in `scope`, under `if c₁ && c₂ …` when `wrap` holds continuous
    /// conjuncts. `None` after a diagnostic.
    fn reaction_body(&mut self, scope: &mut Scope, stmts: &[StmtId], wrap: &[LoweredExpr]) -> Option<Vec<piperine_ir::StmtId>> {
        self.reaction_body_under(scope, stmts, |this| this.conjunction(wrap))
    }

    /// The body of a reaction in `scope`, under `if cond` when `condition`, run once the body is
    /// lowered, gives one. `None` after a diagnostic.
    fn reaction_body_under(&mut self, scope: &mut Scope, stmts: &[StmtId], condition: impl FnOnce(&mut Self) -> Option<LoweredExpr>) -> Option<Vec<piperine_ir::StmtId>> {
        let (mut lowered, value) = self.lower_statements(scope, stmts)?;
        if let Some(value) = value {
            lowered.push(self.stmt(Stmt::Expr(value)));
        }
        let Some(cond) = condition(self) else { return Some(lowered) };
        let origin = self.ir_origin(cond);
        let then = self.emit(Expr::Block { stmts: lowered, value: None }, ValueType::Unit, origin.clone());
        let otherwise = self.emit(Expr::Unit, ValueType::Unit, origin.clone());
        let whole = self.emit(Expr::If { cond, then, otherwise }, ValueType::Unit, origin);
        Some(vec![self.stmt(Stmt::Expr(whole))])
    }

    /// The disjuncts of the head `expr`. `None` after a diagnostic.
    pub(super) fn disjuncts(&mut self, scope: &mut Scope, expr: ExprId) -> Option<Vec<Disjunct>> {
        let node = self.design.expr_node(expr).clone();
        let origin = self.expr_origin(expr);
        match &node.kind {
            ExprKind::Paren(inner) => self.disjuncts(scope, *inner),
            ExprKind::Binary { op: BinOp::Or, lhs, rhs } => {
                let left = self.disjuncts(scope, *lhs);
                let right = self.disjuncts(scope, *rhs);
                let mut all = left?;
                all.extend(right?);
                Some(all)
            }
            ExprKind::Binary { op: BinOp::And, lhs, rhs } => {
                let mut all = self.disjuncts(scope, *lhs)?;
                // Each disjunct gets its own lowering of the guard: an IR expression has one place.
                for d in &mut all {
                    self.guard(scope, *rhs, d)?;
                }
                Some(all)
            }
            ExprKind::MethodCall { recv, .. } => match self.types.builtin(expr) {
                Some(Builtin::Changed) if let Some(param) = self.param_of(scope, *recv) => Some(vec![Disjunct { source: EventSource::ParamChanged(param), discrete: Vec::new(), continuous: Vec::new(), origin }]),
                Some(edge @ (Builtin::Changed | Builtin::Rise | Builtin::Fall)) if let Lookup::Found(element) = self.element_lookup(scope, *recv) => {
                    // `on lines[k].rise()`: the event of that element only; `k` folds (QC4)
                    let at = self.folded_element(scope, *recv, element)?;
                    let discrete = match edge {
                        Builtin::Rise => vec![self.element_rise_guard(scope, element, origin.clone())],
                        Builtin::Fall => vec![self.element_fall_guard(scope, element, origin.clone())],
                        _ => Vec::new(),
                    };
                    Some(vec![Disjunct { source: EventSource::ElementChanged { carrier: element.carrier, element: at }, discrete, continuous: Vec::new(), origin }])
                }
                Some(edge @ (Builtin::Changed | Builtin::Rise | Builtin::Fall)) => {
                    let carrier = self.carrier_of(scope, *recv)?;
                    let discrete = match edge {
                        Builtin::Rise => vec![self.rise_guard(carrier, origin.clone())],
                        Builtin::Fall => vec![self.fall_guard(carrier, origin.clone())],
                        _ => Vec::new(),
                    };
                    Some(vec![Disjunct { source: EventSource::Changed(carrier), discrete, continuous: Vec::new(), origin }])
                }
                _ if self.is_event_method(expr) => self.event_method_disjuncts(scope, expr),
                _ => {
                    self.unsupported(scope.file, node.span, "this method as an event");
                    None
                }
            },
            ExprKind::Call { args, .. } => match self.types.callee(expr) {
                // A function that returns `Event` (§13 rule 7): inlined, its body's event standing here (`event_functions.rs`).
                Some(Callee::Function(function)) if self.is_event_function(function) => self.event_function_disjuncts(scope, expr, function),
                // The same through a method: `Trait::fires(x)` and a call through a bound.
                Some(Callee::Method(_) | Callee::Bound { .. }) if self.is_event_method(expr) => self.event_method_disjuncts(scope, expr),
                Some(Callee::Core(id)) if id.info().name == "cross" => {
                    // `cross(e, ttol, etol)`: positionally or by name (§3.2)
                    let (mut watched, mut ttol, mut etol) = (None, None, None);
                    for (i, a) in args.iter().enumerate() {
                        let slot = match a.name.as_ref().map(|n| n.text.as_str()) {
                            Some("e") => 0,
                            Some("ttol") => 1,
                            Some("etol") => 2,
                            Some(_) => continue,
                            None => i,
                        };
                        match slot {
                            0 => watched = Some(a.value),
                            1 => ttol = Some(a.value),
                            2 => etol = Some(a.value),
                            _ => {}
                        }
                    }
                    let watched = self.event_argument(scope, watched?)?;
                    let ttol = self.tolerance(scope, ttol)?;
                    let etol = self.tolerance(scope, etol)?;
                    Some(vec![Disjunct { source: EventSource::Cross { e: watched, ttol, etol }, discrete: Vec::new(), continuous: Vec::new(), origin }])
                }
                Some(Callee::Core(id)) if id.info().name == "at" => {
                    let time = self.event_argument(scope, args[0].value)?;
                    Some(vec![Disjunct { source: EventSource::At(time), discrete: Vec::new(), continuous: Vec::new(), origin }])
                }
                // A module with an `Event` result (`above`, `init`): its result, in its frame.
                Some(Callee::Module(_)) if self.result_callee(expr).is_some() => {
                    let frame = self.frame_of(scope, expr)?;
                    let module = self.frames[frame].module;
                    let DeclKind::Module(m) = &self.design.decl_data(module).kind else { return None };
                    let result = m.result?;
                    let entry = self.enter_frame(scope, frame);
                    let lowered = self.disjuncts(scope, result);
                    self.leave_frame(scope, entry);
                    lowered
                }
                _ => {
                    self.unsupported(scope.file, node.span, "this call as an event");
                    None
                }
            },
            // A module-body `let` of an event (`let tick = at(..); tick`): its value, in place;
            // and `settle`, the event of the settle phase (§13 rules 10, 14, 17).
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) if id.info().name == "settle" => {
                    Some(vec![Disjunct { source: EventSource::Settle, discrete: Vec::new(), continuous: Vec::new(), origin }])
                }
                Some(Resolution::Decl(decl)) if self.body_let_value(decl).is_some() => {
                    let value = self.body_let_value(decl)?;
                    self.disjuncts(scope, value)
                }
                _ => {
                    self.unsupported(scope.file, node.span, "this name as an event");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, node.span, "this form of an event");
                None
            }
        }
    }

    /// The guard `expr` of the disjunct `d`, split at its top-level `&&` into conjuncts, each
    /// added to the discrete or the continuous part. `None` after a diagnostic.
    fn guard(&mut self, scope: &mut Scope, expr: ExprId, d: &mut Disjunct) -> Option<()> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.guard(scope, *inner, d),
            ExprKind::Binary { op: BinOp::And, lhs, rhs } => {
                let (lhs, rhs) = (*lhs, *rhs);
                self.guard(scope, lhs, d)?;
                self.guard(scope, rhs, d)
            }
            _ => {
                let before = scope.locals.len();
                let lowered = self.lower_expr(scope, expr)?;
                let span = self.design.expr_node(expr).span;
                self.add_conjunct(scope, lowered, before, span, d)
            }
        }
    }

    /// The lowered `bool` `conjunct` joins the guard of `d`: the continuous part when it reads a probe or a local, the
    /// discrete part otherwise. `locals` is how many locals the scope held before `conjunct` was lowered, and a `let`
    /// it added to the discrete part is refused. `None` after a diagnostic.
    pub(super) fn add_conjunct(&mut self, scope: &mut Scope, conjunct: LoweredExpr, locals: usize, span: crate::msl::Span, d: &mut Disjunct) -> Option<()> {
        if self.continuous(conjunct) || self.reads_local(conjunct) {
            d.continuous.push(conjunct);
        } else if scope.locals.len() > locals {
            self.unsupported(scope.file, span, "a `let` in the discrete part of a guard");
            return None;
        } else {
            d.discrete.push(conjunct);
        }
        Some(())
    }

    /// Whether the lowered expression `expr` reads a local of the body being lowered: a conjunct that
    /// does is evaluated where the local lives, in the body, and never by the event's own guard.
    fn reads_local(&self, expr: LoweredExpr) -> bool {
        let mut pending = vec![expr];
        while let Some(next) = pending.pop() {
            let Some(kind) = self.ir.expr_kind(next) else { continue };
            if matches!(kind, Expr::Local(_)) {
                return true;
            }
            pending.extend(kind.operands());
        }
        false
    }

    /// The argument of `cross` or `at`, which belongs to the event: it may bind no local.
    fn event_argument(&mut self, scope: &mut Scope, arg: ExprId) -> Option<LoweredExpr> {
        let before = scope.locals.len();
        let lowered = self.lower_expr(scope, arg)?;
        if scope.locals.len() > before {
            let span = self.design.expr_node(arg).span;
            self.unsupported(scope.file, span, "a `let` in the argument of an event");
            return None;
        }
        Some(lowered)
    }

    /// The `ttol` or `etol` argument `arg` of `cross`, lowered: `Some(None)` for one left out or
    /// given `None` (the watch keeps the solver's default), `Some(Some(value))` for a constant or
    /// a param, `None` after a diagnostic. A value reaching a hoisted module's input (`above`'s
    /// `cross(e, ttol, etol)`) is read in the caller's frame.
    fn tolerance(&mut self, scope: &mut Scope, arg: Option<ExprId>) -> Option<Option<LoweredExpr>> {
        self.optional_constant(scope, arg, "a `cross` tolerance")
    }

    /// The optional argument `arg`, lowered as [`tolerance`](Self::tolerance) reads one: `Some(None)`
    /// for one left out or given `None`, `Some(Some(value))` for a constant or a param, `None` after a
    /// diagnostic naming `what` (`delay`'s `max` is read the same way, §14.6).
    pub(super) fn optional_constant(&mut self, scope: &mut Scope, arg: Option<ExprId>, what: &str) -> Option<Option<LoweredExpr>> {
        let Some(arg) = arg else { return Some(None) };
        let node = self.design.expr_node(arg).clone();
        match &node.kind {
            ExprKind::Paren(inner) => return self.optional_constant(scope, Some(*inner), what),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) if id.info().name == "None" => return Some(None),
                Some(Resolution::Decl(decl)) => match self.input_of(scope, decl) {
                    Some(None) => return Some(None),
                    Some(Some(given)) => return self.in_caller(scope, |this, scope| this.optional_constant(scope, Some(given), what)),
                    None => {}
                },
                _ => {}
            },
            ExprKind::Call { args, .. } if self.is_some_call(arg) => {
                let payload = args.first().map(|a| a.value);
                return self.optional_constant(scope, payload, what);
            }
            _ => {}
        }
        if !self.constant_or_param(scope, arg) {
            self.unsupported(scope.file, node.span, &format!("{what} that is not a constant or a param"));
            return None;
        }
        Some(Some(self.event_argument(scope, arg)?))
    }

    /// Whether the source expression `expr` reads only literals, constants and params (so no
    /// carrier, probe or local), through operators; an input of a hoisted module is judged by
    /// what its caller gave.
    fn constant_or_param(&mut self, scope: &mut Scope, expr: ExprId) -> bool {
        let node = self.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Lit(_) => true,
            ExprKind::Paren(inner) | ExprKind::Unary { expr: inner, .. } | ExprKind::Cast { expr: inner, .. } => self.constant_or_param(scope, *inner),
            ExprKind::Binary { lhs, rhs, .. } => self.constant_or_param(scope, *lhs) && self.constant_or_param(scope, *rhs),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(_)) => true,
                Some(Resolution::Decl(decl)) => match self.input_of(scope, decl) {
                    Some(None) => false,
                    Some(Some(given)) => self.in_caller(scope, |this, scope| Some(this.constant_or_param(scope, given))).unwrap_or(false),
                    None => matches!(self.design.decl_data(decl).kind, DeclKind::Param(_) | DeclKind::Const(_)),
                },
                _ => false,
            },
            _ => false,
        }
    }

    /// The index among the params of the body of the param a receiver names, when it is one of the
    /// body's own (frame 0): `r.changed()` and `r.prev()` read its pin (§5.3.1).
    pub(super) fn param_of(&self, scope: &Scope, expr: ExprId) -> Option<u32> {
        let ExprKind::Path(path) = &self.design.expr_node(self.peel_parens(expr)).kind else { return None };
        let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
        if scope.frame != 0 || !matches!(self.design.decl_data(decl).kind, DeclKind::Param(_)) {
            return None;
        }
        scope.params.get(&decl).copied()
    }

    /// The carrier of the body a receiver names, in the frame of `scope`.
    pub(super) fn carrier_of(&mut self, scope: &Scope, expr: ExprId) -> Option<u32> {
        let node = self.design.expr_node(self.peel_parens(expr)).clone();
        if let ExprKind::Field { .. } = &node.kind {
            return self.reached_carrier(scope, self.peel_parens(expr));
        }
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(bound) = scope.handles.get(&decl).copied()
        {
            return self.carrier_of(scope, bound);
        }
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(index) = self.frame_carrier(scope.frame, decl)
        {
            return Some(index);
        }
        self.unsupported(scope.file, node.span, "a receiver that is not a carrier of this module");
        None
    }

    /// `c.rise()`'s guard (§22.1): `!Previous(c) && c` on a `Bit` of `bool`; on a `Bit<Logic>` the level `c` is high and
    /// `Previous(c)` is not (§22.1.5), so an `X` or `Z` before the change counts as not high.
    fn rise_guard(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        if self.is_four_state(c) {
            return self.four_state_edge(c, (1, 1), origin);
        }
        let previous = self.previous(c, origin.clone());
        let was_low = self.not(previous, origin.clone());
        let read = self.carrier_read(c, origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::And, lhs: was_low, rhs: read }, ValueType::Bool, origin)
    }

    /// `c.fall()`'s guard (§22.1): `Previous(c) && !c` on a `Bit` of `bool`; on a `Bit<Logic>` the level `c` is low and
    /// `Previous(c)` is not (§22.1.5).
    fn fall_guard(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        if self.is_four_state(c) {
            return self.four_state_edge(c, (1, 0), origin);
        }
        let previous = self.previous(c, origin.clone());
        let read = self.carrier_read(c, origin.clone());
        let low = self.not(read, origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::And, lhs: previous, rhs: low }, ValueType::Bool, origin)
    }

    /// `x[k].rise()`'s guard: `!ElementPrevious(x, k) && x[k]`.
    fn element_rise_guard(&mut self, scope: &Scope, element: ElementRef, origin: Origin) -> LoweredExpr {
        let previous = self.element_previous(scope, element, origin.clone());
        let was_low = self.not(previous, origin.clone());
        let read = self.carrier_element_read(scope, element, origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::And, lhs: was_low, rhs: read }, ValueType::Bool, origin)
    }

    /// `x[k].fall()`'s guard: `ElementPrevious(x, k) && !x[k]`.
    fn element_fall_guard(&mut self, scope: &Scope, element: ElementRef, origin: Origin) -> LoweredExpr {
        let previous = self.element_previous(scope, element, origin.clone());
        let read = self.carrier_element_read(scope, element, origin.clone());
        let low = self.not(read, origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::And, lhs: previous, rhs: low }, ValueType::Bool, origin)
    }

    /// Whether carrier `c` holds a `Logic`: the only struct the checker lets reach an edge (`edge_read`).
    fn is_four_state(&self, c: u32) -> bool {
        matches!(self.structure.carriers[c as usize].ty, ValueType::Struct(_))
    }

    /// The edge of a `Logic<1>` carrier into the level whose planes are `(binary, value)`: `c` is at that level and
    /// `Previous(c)` is not (`impl Bit<Logic>`, §22.1.5).
    fn four_state_edge(&mut self, c: u32, planes: (i128, i128), origin: Origin) -> LoweredExpr {
        let previous = self.previous(c, origin.clone());
        let before = self.level_is(previous, planes, origin.clone());
        let not_before = self.not(before, origin.clone());
        let read = self.carrier_read(c, origin.clone());
        let now = self.level_is(read, planes, origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::And, lhs: not_before, rhs: now }, ValueType::Bool, origin)
    }

    /// Whether the `Logic<1>` `logic` has `binary` and `value` planes equal to `planes`.
    fn level_is(&mut self, logic: LoweredExpr, planes: (i128, i128), origin: Origin) -> LoweredExpr {
        let plane = ValueType::UInt(1);
        let mut checks = Vec::new();
        for (index, wanted) in [(0u32, planes.0), (1, planes.1)] {
            let field = self.emit(Expr::Field { expr: logic, index }, plane.clone(), origin.clone());
            let wanted = self.emit(Expr::Int(wanted), plane.clone(), origin.clone());
            checks.push(self.emit(Expr::Binary { op: BinaryOp::Eq, lhs: field, rhs: wanted }, ValueType::Bool, origin.clone()));
        }
        self.emit(Expr::Binary { op: BinaryOp::And, lhs: checks[0], rhs: checks[1] }, ValueType::Bool, origin)
    }

    fn not(&mut self, operand: LoweredExpr, origin: Origin) -> LoweredExpr {
        self.emit(Expr::Unary { op: UnaryOp::Not, expr: operand }, ValueType::Bool, origin)
    }

    /// A read of carrier `c`: its value.
    pub(super) fn carrier_read(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let carrier = &self.structure.carriers[c as usize];
        // an array of carriers read whole is the array of its elements' values (CV-24 AC 1)
        let ty = match carrier.count {
            Some(count) => ValueType::Array(Box::new(carrier.ty.clone()), count),
            None => carrier.ty.clone(),
        };
        self.emit(Expr::Carrier(c), ty, origin)
    }

    /// `c.prev()`: the value of carrier `c` before its most recent change.
    pub(super) fn previous(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let ty = self.structure.carriers[c as usize].ty.clone();
        self.emit(Expr::Previous(c), ty, origin)
    }

    /// The read a built-in method is as a value: `c.prev()` is the carrier's previous value and `p.inflow()`
    /// the flow entering at a port; an edge is an event (`events.rs`), never a value.
    pub(super) fn lower_builtin_read(&mut self, scope: &mut Scope, expr: ExprId, builtin: Builtin, recv: ExprId, base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.design.expr_node(expr).span;
        match builtin {
            Builtin::Prev if let Some(param) = self.param_of(scope, recv) => Some(self.emit(Expr::ParamPrevious(param), base, origin)),
            Builtin::Prev => match self.element_lookup(scope, recv) {
                Lookup::Found(element) => Some(self.element_previous(scope, element, origin)),
                Lookup::Failed => None,
                Lookup::No => {
                    let carrier = self.carrier_of(scope, recv)?;
                    Some(self.previous(carrier, origin))
                }
            },
            Builtin::Inflow => self.lower_inflow(scope, recv, origin),
            // An event read is `fired(k)` (`event_reads.rs`, §13 rule 2) before it gets here; an
            // edge that reaches this arm stands where no event read does.
            Builtin::Changed | Builtin::Rise | Builtin::Fall => {
                self.unsupported(scope.file, span, "an event that is not an event read");
                None
            }
        }
    }
}
