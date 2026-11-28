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

use super::DigitalLowering;
use piperine_ir::{BinaryOp, Event, EventSource, Expr, ExprId as LoweredExpr, Origin, Reaction, Stmt, UnaryOp, ValueType};

use super::elements::{ElementRef, Lookup};
use crate::elab::elaborator::Scope;
use crate::refusal::forms::AnalogForm;
use crate::pom::{BinOp, DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtId};
use crate::types::{Builtin, Callee};

/// One disjunct of a head: what fires it, and its guard split in two.
pub(in crate::elab) struct Disjunct {
    pub(in crate::elab) source: EventSource,
    /// The conjuncts the event holds.
    pub(in crate::elab) discrete: Vec<LoweredExpr>,
    /// The conjuncts that read a probe, `now()` or `analysis(Static)`: the `if` around the body.
    pub(in crate::elab) continuous: Vec<LoweredExpr>,
    pub(in crate::elab) origin: Origin,
}

impl DigitalLowering<'_, '_, '_> {
    /// The `digital` blocks of `module`, read in `scope`: their reactions, in source order.
    pub(in crate::elab) fn lower_digital(&mut self, scope: &mut Scope, module: DeclId) {
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return };
        for block in data.behaviors.clone() {
            let decl = self.state.design.decl_data(block).clone();
            let DeclKind::Behavior(behavior) = &decl.kind else { continue };
            if behavior.kind != crate::pom::BehaviorKind::Digital {
                continue;
            }
            for (child, iteration) in self.structure().digital_sequence(&decl.children) {
                if !matches!(self.state.design.decl_data(child).kind, DeclKind::Reaction(_)) {
                    continue;
                }
                // a reaction made by a generation `for` reads its loop variables as the constants of its iteration
                for (variable, value) in &iteration.values {
                    scope.values.unrolled.bind(*variable, piperine_ir::Constant::Int(*value));
                }
                scope.digital.ordinals = iteration.ordinals.clone();
                self.lower_reaction(scope, child);
                scope.digital.ordinals.clear();
                for (variable, _) in &iteration.values {
                    scope.values.unrolled.unbind(*variable);
                }
            }
        }
    }

    /// `on head { body }`: its events and one reaction, or one reaction per disjunct (see the
    /// table of this module).
    fn lower_reaction(&mut self, scope: &Scope, reaction: DeclId) {
        let data = self.state.design.decl_data(reaction).clone();
        let DeclKind::Reaction(r) = &data.kind else { return };
        let origin = self.state.locator.decl(reaction);
        let mut head = Scope::sub_scope(scope);
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
                self.state.digital.structure.reactions.push(Reaction { on, stmts, locals: head.values.locals, origin });
            }
            return;
        }
        let span = self.state.design.expr_node(r.event).span;
        if !head.values.locals.is_empty() {
            self.refuse(scope.values.file, span, AnalogForm::LetInGuardOfHeadSplitPer);
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
        let mut body = Scope::sub_scope(scope);
        if let Some(stmts) = self.reaction_body_under(&mut body, &r.body.stmts, |_| guard) {
            self.state.digital.structure.reactions.push(Reaction { on, stmts, locals: body.values.locals, origin });
        }
    }

    /// Add the event of `source` guarded by the conjunction of `discrete`: its index.
    pub(super) fn add_event(&mut self, source: EventSource, discrete: &[LoweredExpr], origin: Origin) -> u32 {
        let guard = self.conjunction(discrete);
        self.state.digital.structure.events.push(Event { source, guard, origin });
        self.state.digital.structure.events.len() as u32 - 1
    }

    /// `a && b && …` over lowered `bool`s, left to right. `None` for no operand.
    pub(in crate::elab) fn conjunction(&mut self, operands: &[LoweredExpr]) -> Option<LoweredExpr> {
        let mut iter = operands.iter().copied();
        let first = iter.next()?;
        Some(iter.fold(first, |acc, next| {
            let origin = self.ir_origin(next);
            self.emit(Expr::Binary { op: BinaryOp::And, lhs: acc, rhs: next }, ValueType::Bool, origin)
        }))
    }

    /// `a || b || …` over lowered `bool`s, left to right. `None` for no operand.
    pub(in crate::elab) fn disjunction(&mut self, operands: &[LoweredExpr]) -> Option<LoweredExpr> {
        let mut iter = operands.iter().copied();
        let first = iter.next()?;
        Some(iter.fold(first, |acc, next| {
            let origin = self.ir_origin(next);
            self.emit(Expr::Binary { op: BinaryOp::Or, lhs: acc, rhs: next }, ValueType::Bool, origin)
        }))
    }

    /// The origin an expression already added was given.
    fn ir_origin(&self, expr: LoweredExpr) -> Origin {
        self.state.values.expr_origins[expr.index()].clone()
    }

    /// The body of a reaction in `scope`, under `if c₁ && c₂ …` when `wrap` holds continuous
    /// conjuncts. `None` after a diagnostic.
    fn reaction_body(&mut self, scope: &mut Scope, stmts: &[StmtId], wrap: &[LoweredExpr]) -> Option<Vec<piperine_ir::StmtId>> {
        self.reaction_body_under(scope, stmts, |this| this.conjunction(wrap))
    }

    /// The body of a reaction in `scope`, under `if cond` when `condition`, run once the body is
    /// lowered, gives one. `None` after a diagnostic.
    fn reaction_body_under(&mut self, scope: &mut Scope, stmts: &[StmtId], condition: impl FnOnce(&mut Self) -> Option<LoweredExpr>) -> Option<Vec<piperine_ir::StmtId>> {
        let (mut lowered, value) = self.values().lower_statements(scope, stmts)?;
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
    pub(in crate::elab) fn disjuncts(&mut self, scope: &mut Scope, expr: ExprId) -> Option<Vec<Disjunct>> {
        let node = self.state.design.expr_node(expr).clone();
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
            ExprKind::MethodCall { recv, .. } => match self.state.types.builtin(expr) {
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
                _ if self.functions().is_event_method(expr) => self.functions().event_method_disjuncts(scope, expr),
                _ => {
                    self.refuse(scope.values.file, node.span, AnalogForm::MethodAsEvent);
                    None
                }
            },
            ExprKind::Call { args, .. } => match self.state.types.callee(expr) {
                // A function that returns `Event` (§13 rule 7): inlined, its body's event standing here (`event_functions.rs`).
                Some(Callee::Function(function)) if self.functions().is_event_function(function) => self.functions().event_function_disjuncts(scope, expr, function),
                // The same through a method: `Trait::fires(x)` and a call through a bound.
                Some(Callee::Method(_) | Callee::Bound { .. }) if self.functions().is_event_method(expr) => self.functions().event_method_disjuncts(scope, expr),
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
                Some(Callee::Module(_)) | None if self.structure().result_callee(expr).is_some() => {
                    let frame = self.structure().frame_of(scope, expr)?;
                    let module = self.state.structure.frames[frame].module;
                    let DeclKind::Module(m) = &self.state.design.decl_data(module).kind else { return None };
                    let result = m.result?;
                    let entry = self.structure().enter_frame(scope, frame);
                    let lowered = self.disjuncts(scope, result);
                    self.structure().leave_frame(scope, entry);
                    lowered
                }
                _ => {
                    self.refuse(scope.values.file, node.span, AnalogForm::CallAsEvent);
                    None
                }
            },
            // A module-body `let` of an event (`let tick = at(..); tick`): its value, in place;
            // and `settle`, the event of the settle phase (§13 rules 10, 14, 17).
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) if id.info().name == "settle" => {
                    Some(vec![Disjunct { source: EventSource::Settle, discrete: Vec::new(), continuous: Vec::new(), origin }])
                }
                Some(Resolution::Decl(decl)) if self.body_let_value(decl).is_some() => {
                    let value = self.body_let_value(decl)?;
                    self.disjuncts(scope, value)
                }
                _ => {
                    self.refuse(scope.values.file, node.span, AnalogForm::NameAsEvent);
                    None
                }
            },
            _ => {
                self.refuse(scope.values.file, node.span, AnalogForm::FormOfEvent);
                None
            }
        }
    }

    /// The guard `expr` of the disjunct `d`, split at its top-level `&&` into conjuncts, each
    /// added to the discrete or the continuous part. `None` after a diagnostic.
    fn guard(&mut self, scope: &mut Scope, expr: ExprId, d: &mut Disjunct) -> Option<()> {
        match &self.state.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.guard(scope, *inner, d),
            ExprKind::Binary { op: BinOp::And, lhs, rhs } => {
                let (lhs, rhs) = (*lhs, *rhs);
                self.guard(scope, lhs, d)?;
                self.guard(scope, rhs, d)
            }
            _ => {
                let before = scope.values.locals.len();
                let lowered = self.values().lower_expr(scope, expr)?;
                let span = self.state.design.expr_node(expr).span;
                self.add_conjunct(scope, lowered, before, span, d)
            }
        }
    }

    /// The lowered `bool` `conjunct` joins the guard of `d`: the continuous part when it reads a probe or a local, the
    /// discrete part otherwise. `locals` is how many locals the scope held before `conjunct` was lowered, and a `let`
    /// it added to the discrete part is refused. `None` after a diagnostic.
    pub(in crate::elab) fn add_conjunct(&mut self, scope: &mut Scope, conjunct: LoweredExpr, locals: usize, span: crate::msl::Span, d: &mut Disjunct) -> Option<()> {
        if self.continuous(conjunct) || self.reads_local(conjunct) {
            d.continuous.push(conjunct);
        } else if scope.values.locals.len() > locals {
            self.refuse(scope.values.file, span, AnalogForm::LetInDiscretePartOfGuard);
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
            let Some(kind) = self.state.builder.expr_kind(next) else { continue };
            if matches!(kind, Expr::Local(_)) {
                return true;
            }
            pending.extend(kind.operands());
        }
        false
    }

    /// The argument of `cross` or `at`, which belongs to the event: it may bind no local.
    fn event_argument(&mut self, scope: &mut Scope, arg: ExprId) -> Option<LoweredExpr> {
        let before = scope.values.locals.len();
        let lowered = self.values().lower_expr(scope, arg)?;
        if scope.values.locals.len() > before {
            let span = self.state.design.expr_node(arg).span;
            self.refuse(scope.values.file, span, AnalogForm::LetInArgumentOfEvent);
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
    pub(in crate::elab) fn optional_constant(&mut self, scope: &mut Scope, arg: Option<ExprId>, what: &str) -> Option<Option<LoweredExpr>> {
        let Some(arg) = arg else { return Some(None) };
        let node = self.state.design.expr_node(arg).clone();
        match &node.kind {
            ExprKind::Paren(inner) => return self.optional_constant(scope, Some(*inner), what),
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) if id.info().name == "None" => return Some(None),
                Some(Resolution::Decl(decl)) => match self.structure().input_of(scope, decl) {
                    Some(None) => return Some(None),
                    Some(Some(given)) => return self.structure().in_caller(scope, |this, scope| this.digital().optional_constant(scope, Some(given), what)),
                    None => {}
                },
                _ => {}
            },
            ExprKind::Call { args, .. } if self.values().is_some_call(arg) => {
                let payload = args.first().map(|a| a.value);
                return self.optional_constant(scope, payload, what);
            }
            _ => {}
        }
        if !self.constant_or_param(scope, arg) {
            self.refuse_with(scope.values.file, node.span, AnalogForm::ThatIsNotConstantOrParam, [what.to_string()]);
            return None;
        }
        Some(Some(self.event_argument(scope, arg)?))
    }

    /// Whether the source expression `expr` reads only literals, constants and params (so no
    /// carrier, probe or local), through operators; an input of a hoisted module is judged by
    /// what its caller gave.
    fn constant_or_param(&mut self, scope: &mut Scope, expr: ExprId) -> bool {
        let node = self.state.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Lit(_) => true,
            ExprKind::Paren(inner) | ExprKind::Unary { expr: inner, .. } | ExprKind::Cast { expr: inner, .. } => self.constant_or_param(scope, *inner),
            ExprKind::Binary { lhs, rhs, .. } => self.constant_or_param(scope, *lhs) && self.constant_or_param(scope, *rhs),
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(_)) => true,
                Some(Resolution::Decl(decl)) => match self.structure().input_of(scope, decl) {
                    Some(None) => false,
                    Some(Some(given)) => self.structure().in_caller(scope, |this, scope| Some(this.digital().constant_or_param(scope, given))).unwrap_or(false),
                    None => matches!(self.state.design.decl_data(decl).kind, DeclKind::Param(_) | DeclKind::Const(_)),
                },
                _ => false,
            },
            _ => false,
        }
    }

    /// The index among the params of the body of the param a receiver names, when it is one of the
    /// body's own (frame 0): `r.changed()` and `r.prev()` read its pin (§5.3.1).
    pub(super) fn param_of(&self, scope: &Scope, expr: ExprId) -> Option<u32> {
        let ExprKind::Path(path) = &self.state.design.expr_node(self.peel_parens(expr)).kind else { return None };
        let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
        if scope.values.frame != 0 || !matches!(self.state.design.decl_data(decl).kind, DeclKind::Param(_)) {
            return None;
        }
        scope.values.params.get(&decl).copied()
    }

    /// The carrier of the body a receiver names, in the frame of `scope`.
    pub(super) fn carrier_of(&mut self, scope: &Scope, expr: ExprId) -> Option<u32> {
        let node = self.state.design.expr_node(self.peel_parens(expr)).clone();
        if let ExprKind::Field { .. } = &node.kind {
            let peeled = self.peel_parens(expr);
            return self.elaboration.structure().reached_carrier(scope, peeled);
        }
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(bound) = scope.digital.handles.get(&decl).copied()
        {
            return self.carrier_of(scope, bound);
        }
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.state.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(index) = self.structure().frame_carrier(scope.values.frame, decl)
        {
            return Some(index);
        }
        self.refuse(scope.values.file, node.span, AnalogForm::ReceiverThatIsNotCarrierOfThis);
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
        matches!(self.state.digital.structure.carriers[c as usize].ty, ValueType::Struct(_))
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
    pub(in crate::elab) fn carrier_read(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let carrier = &self.state.digital.structure.carriers[c as usize];
        // an array of carriers read whole is the array of its elements' values (CV-24 AC 1)
        let ty = match carrier.count {
            Some(count) => ValueType::Array(Box::new(carrier.ty.clone()), count),
            None => carrier.ty.clone(),
        };
        self.emit(Expr::Carrier(c), ty, origin)
    }

    /// `c.prev()`: the value of carrier `c` before its most recent change.
    pub(in crate::elab) fn previous(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let ty = self.state.digital.structure.carriers[c as usize].ty.clone();
        self.emit(Expr::Previous(c), ty, origin)
    }

    /// The read a built-in method is as a value: `c.prev()` is the carrier's previous value and `p.inflow()`
    /// the flow entering at a port; an edge is an event (`events.rs`), never a value.
    pub(in crate::elab) fn lower_builtin_read(&mut self, scope: &mut Scope, expr: ExprId, builtin: Builtin, recv: ExprId, base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(expr).span;
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
            Builtin::Inflow => self.analog().lower_inflow(scope, recv, origin),
            // `s.others()`: the resolution of every drive but the body's own (§5.2.7)
            Builtin::Others => {
                let carrier = self.carrier_of(scope, recv)?;
                let value = self.state.digital.structure.carriers[carrier as usize].ty.clone();
                self.state.digital.structure.others_read.insert(carrier);
                Some(self.emit(Expr::Others(carrier), value, origin))
            }
            // An event read is `fired(k)` (`event_reads.rs`, §13 rule 2) before it gets here; an
            // edge that reaches this arm stands where no event read does.
            Builtin::Changed | Builtin::Rise | Builtin::Fall => {
                self.refuse(scope.values.file, span, AnalogForm::EventThatIsNotEventRead);
                None
            }
        }
    }
}
