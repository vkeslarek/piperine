//! Events and the heads of reactions (part IV §12.1, §13; DIG-09 AC 6 to 9).
//!
//! A head is put in disjunctive form, `d₁ || d₂ || …`, each `dₖ` an event source and its guard.
//! `c.changed()` is `Changed(c)`; `c.rise()` and `c.fall()` are `Changed(c)` guarded by the two
//! conditions of the stdlib text (§22.1), every part a flag-total read, so the guard is never
//! unknown:
//!
//! | Edge | Guard |
//! | --- | --- |
//! | `rise` | `!High(Previous(c)) & (!IsKnown(c) \| KnownOr(c, false))`: the previous value is not known-true and the value is not known-false |
//! | `fall` | `!(IsKnown(Previous(c)) & !KnownOr(Previous(c), true)) & !High(c)`: the previous value is not known-false and the value is not known-true |
//!
//! `cross(e)` is `Cross(e)` and `at(t)` is `At(t)`. A guard `ev && g₁ && g₂` is split at its
//! top-level `&&` into conjuncts, each lowered on its own: one that reads a probe, `now()` or
//! `analysis(Static)` is **continuous** and becomes an `if` around the reaction's body (abi.md
//! F2: only the discrete part of a guard is a discrete function); the others stay on the event.
//! A guard that may be unknown is kept as written: the generated code decides it.
//!
//! | Head | Reactions |
//! | --- | --- |
//! | no disjunct has a continuous conjunct | one, on every disjunct's event |
//! | one disjunct, with continuous conjuncts | one, its body under `if` |
//! | several disjuncts, one with continuous conjuncts | one per disjunct, in source order; `P0001` when the body emits a record |
//!
//! Continuity is read off the lowered IR (an expression is continuous when it reads a probe, a
//! branch flow, `time()`, `static()`, `gmin()` or `source_scale()`, directly or through an operand
//! or a function), so a value input of a hoisted module is judged by what its caller gave.

use piperine_ir::{BinaryOp, Env, Event, EventSource, Expr, ExprId as LoweredExpr, Origin, Reaction, Stmt, UnaryOp, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::{BinOp, DeclId, DeclKind, ExprId, ExprKind, Resolution, StmtId};
use crate::types::{Builtin, Callee};

/// One disjunct of a head: what fires it, and its guard split in two.
pub(super) struct Disjunct {
    source: EventSource,
    /// The conjuncts the event holds.
    discrete: Vec<LoweredExpr>,
    /// The conjuncts that read a probe, `now()` or `analysis(Static)`: the `if` around the body.
    continuous: Vec<LoweredExpr>,
    origin: Origin,
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
            Expr::Probe { .. } | Expr::Flow { .. } | Expr::Env(Env::Time | Env::Static | Env::Gmin | Env::SourceScale) => true,
            Expr::Env(Env::Temperature) | Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::Int(_) | Expr::Unknown | Expr::Carrier(_) | Expr::Previous(_) => false,
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => self.continuous(*expr),
            Expr::Binary { lhs, rhs, .. } => any(&[*lhs, *rhs]),
            Expr::KnownOr { value, default } => any(&[*value, *default]),
            Expr::If { cond, then, otherwise } => any(&[*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => stmts.iter().any(|s| self.stmt_continuous.get(s.index()).copied().unwrap_or(false)) || value.is_some_and(|v| self.continuous(v)),
            Expr::Call { function, args } => self.variants.continuous(*function) || any(args),
            Expr::Math { args, .. } => any(args),
            Expr::Method { receiver, args, .. } => self.continuous(*receiver) || any(args),
            Expr::Closure { body, .. } => self.continuous(*body),
            Expr::Limit { x, f, seed, .. } => any(&[*x, *f]) || seed.is_some_and(|s| self.continuous(s)),
            Expr::Stim { mag, phase, .. } => self.continuous(*mag) || phase.is_some_and(|p| self.continuous(p)),
        }
    }

    /// The `digital` blocks of `module`, read in `scope`: their reactions, in source order.
    pub(super) fn lower_digital(&mut self, scope: &Scope, module: DeclId) {
        let DeclKind::Module(data) = &self.design.decl_data(module).kind else { return };
        for block in data.behaviors.clone() {
            let decl = self.design.decl_data(block).clone();
            let DeclKind::Behavior(behavior) = &decl.kind else { continue };
            if behavior.kind != crate::pom::BehaviorKind::Digital {
                continue;
            }
            for child in decl.children {
                if matches!(self.design.decl_data(child).kind, DeclKind::Reaction(_)) {
                    self.lower_reaction(scope, child);
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
        if self.emits_in(&r.body) {
            self.unsupported(scope.file, span, "a head whose disjuncts each need a reaction, over a body that emits a record,");
            return;
        }
        if !head.locals.is_empty() {
            self.unsupported(scope.file, span, "a `let` in the guard of a head split per disjunct");
            return;
        }
        for d in disjuncts {
            let on = vec![self.add_event(d.source, &d.discrete, d.origin)];
            let mut body = Self::sub_scope(scope);
            if let Some(stmts) = self.reaction_body(&mut body, &r.body.stmts, &d.continuous) {
                self.structure.reactions.push(Reaction { on, stmts, locals: body.locals, origin: origin.clone() });
            }
        }
    }

    /// Add the event of `source` guarded by the conjunction of `discrete`: its index.
    fn add_event(&mut self, source: EventSource, discrete: &[LoweredExpr], origin: Origin) -> u32 {
        let guard = self.conjunction(discrete);
        self.structure.events.push(Event { source, guard, origin });
        self.structure.events.len() as u32 - 1
    }

    /// `a && b && …` over lowered `bool`s, left to right: `&` when an operand may be unknown
    /// (§3.3.12), `&&` otherwise. `None` for no operand.
    fn conjunction(&mut self, operands: &[LoweredExpr]) -> Option<LoweredExpr> {
        let mut iter = operands.iter().copied();
        let first = iter.next()?;
        Some(iter.fold(first, |acc, next| {
            let flag = self.flags(&[acc, next]);
            let op = if flag == Flag::Flagged { BinaryOp::BitAnd } else { BinaryOp::And };
            let origin = self.ir_origin(next);
            self.emit(Expr::Binary { op, lhs: acc, rhs: next }, flag.wrap(ValueType::Bool), origin)
        }))
    }

    /// The origin an expression already added was given.
    fn ir_origin(&self, expr: LoweredExpr) -> Origin {
        self.expr_origins[expr.index()].clone()
    }

    /// The body of a reaction in `scope`, under `if c₁ && c₂ …` when `wrap` holds continuous
    /// conjuncts. `None` after a diagnostic.
    fn reaction_body(&mut self, scope: &mut Scope, stmts: &[StmtId], wrap: &[LoweredExpr]) -> Option<Vec<piperine_ir::StmtId>> {
        let (mut lowered, value) = self.lower_statements(scope, stmts)?;
        if let Some(value) = value {
            lowered.push(self.stmt(Stmt::Expr(value)));
        }
        let Some(cond) = self.conjunction(wrap) else { return Some(lowered) };
        let origin = self.ir_origin(cond);
        let then = self.emit(Expr::Block { stmts: lowered, value: None }, ValueType::Unit, origin.clone());
        let otherwise = self.emit(Expr::Unit, ValueType::Unit, origin.clone());
        let whole = self.emit(Expr::If { cond, then, otherwise }, ValueType::Unit, origin);
        Some(vec![self.stmt(Stmt::Expr(whole))])
    }

    /// Whether the statements reach `emit!`, directly or through a function that emits.
    fn emits_in(&self, block: &crate::pom::BlockNode) -> bool {
        let mut stack = crate::pom::block_exprs(self.design, block);
        while let Some(e) = stack.pop() {
            let kind = &self.design.expr_node(e).kind;
            match kind {
                ExprKind::Emit { .. } => return true,
                ExprKind::Call { .. } if matches!(self.types.callee(e), Some(Callee::Function(f)) if self.types.emits(f).is_some()) => return true,
                _ => {}
            }
            stack.extend(crate::pom::expr_children(kind, self.design));
        }
        false
    }

    /// The disjuncts of the head `expr`. `None` after a diagnostic.
    fn disjuncts(&mut self, scope: &mut Scope, expr: ExprId) -> Option<Vec<Disjunct>> {
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
                Some(edge @ (Builtin::Changed | Builtin::Rise | Builtin::Fall)) => {
                    let carrier = self.carrier_of(scope, *recv)?;
                    let discrete = match edge {
                        Builtin::Rise => vec![self.rise_guard(carrier, origin.clone())],
                        Builtin::Fall => vec![self.fall_guard(carrier, origin.clone())],
                        _ => Vec::new(),
                    };
                    Some(vec![Disjunct { source: EventSource::Changed(carrier), discrete, continuous: Vec::new(), origin }])
                }
                _ => {
                    self.unsupported(scope.file, node.span, "this method as an event");
                    None
                }
            },
            ExprKind::Call { args, .. } => match self.types.callee(expr) {
                Some(Callee::Core(id)) if id.info().name == "cross" => {
                    let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                    let watched = self.event_argument(scope, args[0])?;
                    for tolerance in &args[1..] {
                        self.no_tolerance(scope, *tolerance)?;
                    }
                    Some(vec![Disjunct { source: EventSource::Cross(watched), discrete: Vec::new(), continuous: Vec::new(), origin }])
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
                    let outer = std::mem::replace(&mut scope.frame, frame);
                    let lowered = self.disjuncts(scope, result);
                    scope.frame = outer;
                    lowered
                }
                _ => {
                    self.unsupported(scope.file, node.span, "this call as an event");
                    None
                }
            },
            // A module-body `let` of an event (`let tick = at(..); tick`): its value, in place.
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
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
                if self.continuous(lowered) {
                    d.continuous.push(lowered);
                } else if scope.locals.len() > before {
                    let span = self.design.expr_node(expr).span;
                    self.unsupported(scope.file, span, "a `let` in the discrete part of a guard");
                    return None;
                } else {
                    d.discrete.push(lowered);
                }
                Some(())
            }
        }
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

    /// A `ttol` or `etol` argument of `cross`: only `None` is accepted (the `Cross` event carries
    /// no tolerance), written as such or reached through a value input of a hoisted module
    /// (`above`'s `cross(e, ttol, etol)`) left out or given `None`.
    fn no_tolerance(&mut self, scope: &mut Scope, arg: ExprId) -> Option<()> {
        let node = self.design.expr_node(arg).clone();
        if let ExprKind::Path(path) = &node.kind {
            match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) if id.info().name == "None" => return Some(()),
                Some(Resolution::Decl(decl)) => match self.input_of(scope, decl) {
                    Some(None) => return Some(()),
                    Some(Some(given)) => return self.in_caller(scope, |this, scope| this.no_tolerance(scope, given)),
                    None => {}
                },
                _ => {}
            }
        }
        self.unsupported(scope.file, node.span, "a `ttol` or `etol` given to `cross`");
        None
    }

    /// The carrier of the body a receiver names, in the frame of `scope`.
    pub(super) fn carrier_of(&mut self, scope: &Scope, expr: ExprId) -> Option<u32> {
        let node = self.design.expr_node(self.peel_parens(expr)).clone();
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(decl)) = self.design.path_node(*path).segments.last().map(|s| s.res)
            && let Some(index) = self.frames.get(scope.frame).and_then(|f| f.carriers.get(&decl))
        {
            return Some(*index);
        }
        self.unsupported(scope.file, node.span, "a receiver that is not a carrier of this module");
        None
    }

    /// `c.rise()`'s guard (§22.1): `!High(Previous(c)) & (!IsKnown(c) | KnownOr(c, false))`.
    fn rise_guard(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let previous = self.previous(c, origin.clone());
        let was_high = self.emit(Expr::High(previous), ValueType::Bool, origin.clone());
        let not_was_high = self.not(was_high, origin.clone());
        let read = self.carrier_read(c, origin.clone());
        let known = self.emit(Expr::IsKnown(read), ValueType::Bool, origin.clone());
        let unknown = self.not(known, origin.clone());
        let read = self.carrier_read(c, origin.clone());
        let fallback = self.emit(Expr::Bool(false), ValueType::Bool, origin.clone());
        let level = self.emit(Expr::KnownOr { value: read, default: fallback }, ValueType::Bool, origin.clone());
        let not_low = self.emit(Expr::Binary { op: BinaryOp::BitOr, lhs: unknown, rhs: level }, ValueType::Bool, origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::BitAnd, lhs: not_was_high, rhs: not_low }, ValueType::Bool, origin)
    }

    /// `c.fall()`'s guard (§22.1): `!(IsKnown(Previous(c)) & !KnownOr(Previous(c), true)) & !High(c)`.
    fn fall_guard(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let previous = self.previous(c, origin.clone());
        let known = self.emit(Expr::IsKnown(previous), ValueType::Bool, origin.clone());
        let previous = self.previous(c, origin.clone());
        let fallback = self.emit(Expr::Bool(true), ValueType::Bool, origin.clone());
        let level = self.emit(Expr::KnownOr { value: previous, default: fallback }, ValueType::Bool, origin.clone());
        let low = self.not(level, origin.clone());
        let was_low = self.emit(Expr::Binary { op: BinaryOp::BitAnd, lhs: known, rhs: low }, ValueType::Bool, origin.clone());
        let not_was_low = self.not(was_low, origin.clone());
        let read = self.carrier_read(c, origin.clone());
        let high = self.emit(Expr::High(read), ValueType::Bool, origin.clone());
        let not_high = self.not(high, origin.clone());
        self.emit(Expr::Binary { op: BinaryOp::BitAnd, lhs: not_was_low, rhs: not_high }, ValueType::Bool, origin)
    }

    fn not(&mut self, operand: LoweredExpr, origin: Origin) -> LoweredExpr {
        self.emit(Expr::Unary { op: UnaryOp::Not, expr: operand }, ValueType::Bool, origin)
    }

    /// A read of carrier `c`: flagged, as every carrier read (§3.3).
    pub(super) fn carrier_read(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let ty = Flag::Flagged.wrap(self.structure.carriers[c as usize].ty.clone());
        self.emit(Expr::Carrier(c), ty, origin)
    }

    /// `c.prev()`: the value of carrier `c` before its most recent change, flagged like a read.
    pub(super) fn previous(&mut self, c: u32, origin: Origin) -> LoweredExpr {
        let ty = Flag::Flagged.wrap(self.structure.carriers[c as usize].ty.clone());
        self.emit(Expr::Previous(c), ty, origin)
    }
}
