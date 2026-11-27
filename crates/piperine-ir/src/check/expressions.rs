//! The expressions of the code walk, one method per expression form: each checks, on the first visit
//! of its expression, the type its form gives and that it stands where its form may, then queues the
//! steps of its operands. An expression met again while it is being walked is a cycle.

use std::collections::BTreeSet;

use super::TypeCheck;
use super::walk::{CodeWalk, ExpressionVisit, WalkPosition, WalkStep};
use crate::ids::{ExprId, FunctionId};
use crate::model::{BinaryOp, Env, Expr, Origin, Terminal, UnaryOp, ValueType};
use crate::real::Real;
use crate::typing::Typing;

impl<'a> TypeCheck<'a> {
    /// The visit of expression `id` at `position`: a cycle when it is being walked, nothing when
    /// it was visited at this position in this scope, its form's checks and steps otherwise.
    pub(super) fn expression(&mut self, walk: &mut CodeWalk, id: ExprId, position: WalkPosition) {
        let ir = self.reader.ir;
        let Some(data) = ir.exprs.get(id.index()) else { return };
        if walk.on_stack.contains(&id) {
            if self.cycles.insert(id) {
                self.wrong(walk, data.origin.clone(), format!("expr {} reaches itself through its operands: a cycle", id.0));
            }
            return;
        }
        if !walk.done.insert((id, position, walk.scope)) {
            return;
        }
        walk.on_stack.insert(id);
        // The checks of the expression itself run once; its placement (sites) once per context.
        let first = walk.seen.insert(id);
        let visit = ExpressionVisit { id, origin: data.origin.clone(), recorded_type: &data.ty, position, first };
        self.expression_form(walk, &visit, &data.kind);
        walk.pending.push(WalkStep::Exit(id));
    }

    /// The checks and steps of the form `kind` of the visited expression.
    fn expression_form(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit<'a>, kind: &'a Expr) {
        match kind {
            Expr::Real(_) => self.literal(walk, visit, &ValueType::F64),
            Expr::Bool(_) => self.literal(walk, visit, &ValueType::Bool),
            Expr::Unit => self.literal(walk, visit, &ValueType::Unit),
            Expr::Local(i) => self.local_read(walk, visit, *i),
            Expr::Param(i) => self.param_read(walk, visit, *i),
            Expr::Unary { op, expr } => self.unary(walk, visit, *op, *expr),
            Expr::Binary { .. } => self.binary(walk, visit, kind),
            Expr::If { cond, then, otherwise } => self.if_form(walk, visit, *cond, [*then, *otherwise]),
            Expr::Block { stmts, value } => self.block(walk, visit, stmts, *value),
            Expr::Call { function, args } => self.call(walk, visit, *function, args),
            Expr::Math { args, .. } => self.math(walk, visit, args),
            Expr::Table { x, table, .. } | Expr::TableSlope { x, table, .. } => self.table_model(walk, visit, *x, table),
            Expr::Closure { params, body } => self.closure(walk, visit, params, *body),
            Expr::Probe { plus, minus } => self.probe(walk, visit, [*plus, *minus]),
            Expr::Flow { branch } => self.flow_read(walk, visit, *branch),
            Expr::Env(env) => self.environment_read(walk, visit, *env),
            Expr::Limit { site, x, f, seed } => self.limit(walk, visit, *site, [*x, *f], *seed),
            Expr::Stim { site, mag, phase } => self.stimulus(walk, visit, *site, *mag, *phase),
            Expr::FloatBits(bits) => self.float_literal(walk, visit, *bits),
            Expr::Wide(id) => self.wide_literal(walk, visit, *id),
            Expr::Int(value) => self.integer_literal(walk, visit, *value),
            Expr::Cast { expr, to } => self.cast(walk, visit, *expr, to),
            Expr::Method { method, receiver, args } => self.method_call(walk, visit, *method, *receiver, args),
            Expr::Same { a, b } => self.same(walk, visit, *a, *b),
            Expr::LevelCast { expr, to, .. } => self.level_cast(walk, visit, *expr, to),
            Expr::ParamPrevious(p) => self.param_previous(walk, visit, *p),
            Expr::Derivative(k) => self.derivative_read(walk, visit, *k),
            Expr::Delay(k) => self.delay_read(walk, visit, *k),
            Expr::Noise(k) => self.noise_read(walk, visit, *k),
            Expr::SessionParameter { parameter } => self.session_parameter_read(walk, visit, *parameter),
            Expr::Fired(e) => self.fired(walk, visit, *e),
            Expr::Unwrap { is, value, .. } => self.unwrap_form(walk, visit, *is, *value),
            Expr::Carrier(i) => self.carrier_read(walk, visit, *i),
            Expr::Others(i) => self.others_read(walk, visit, *i),
            Expr::Resolve { by_count, drives, except } => self.resolve(walk, visit, by_count, drives, *except),
            Expr::Previous(i) => self.previous_read(walk, visit, *i),
            Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => self.element(walk, visit, kind),
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } => self.looping(walk, visit, kind),
            Expr::Break(value) => self.break_form(walk, visit, *value),
            Expr::Continue => self.leave_loop(walk, &visit.origin, visit.position, None, false),
            Expr::Return(value) => self.return_form(walk, visit, *value),
            Expr::FunctionRef(function) => self.function_reference(walk, visit, *function),
            Expr::CallValue { callee, args } => self.call_value(walk, visit, *callee, args),
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } => self.aggregate_form(walk, visit),
            Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) => self.aggregate_form(walk, visit),
        }
    }

    /// A literal `true`, `1.0` or `()`: typed `form_type`.
    fn literal(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, form_type: &ValueType) {
        if visit.first {
            self.own(walk, &visit.origin, visit.recorded_type, form_type);
        }
    }

    /// A read of local `i`: a local of the owner, of its type, bound in this scope (reported once).
    fn local_read(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, i: u32) {
        let origin = &visit.origin;
        match walk.owner.locals.get(i as usize) {
            Some(local) => {
                if visit.first {
                    self.own(walk, origin, visit.recorded_type, &local.clone());
                }
                // Checked in every scope the read is visited in; reported once.
                if !walk.bound.get(i as usize).copied().unwrap_or(false) && walk.unbound.insert(visit.id) {
                    self.wrong(walk, origin.clone(), format!("local {i} read before its `let`"));
                }
            }
            None if visit.first => self.wrong(walk, origin.clone(), format!("local {i} out of range ({} locals)", walk.owner.locals.len())),
            None => {}
        }
    }

    /// A read of param `i`: in a body, of the param's type.
    fn param_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, i: u32) {
        if !visit.first {
            return;
        }
        let origin = visit.origin.clone();
        if !walk.owner.in_body {
            self.wrong(walk, origin, "param read outside a body".to_string());
        } else {
            match walk.owner.params.get(i as usize) {
                Some(p) => self.own(walk, &origin, visit.recorded_type, &p.clone()),
                None => self.wrong(walk, origin, format!("param {i} out of range ({} params)", walk.owner.params.len())),
            }
        }
    }

    /// A unary operator: of a digital operand, the type `Typing` gives; a negation of an `f64` or a
    /// `!` of a `bool` otherwise.
    fn unary(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, op: UnaryOp, expr: ExprId) {
        if visit.first {
            match self.reader.type_of(expr) {
                Some(operand) if Typing::digital(operand) => self.typed(walk, &visit.origin, visit.recorded_type, Typing::unary(op, operand), "a unary operator"),
                _ => {
                    let operand = if op == UnaryOp::Neg { ValueType::F64 } else { ValueType::Bool };
                    self.own(walk, &visit.origin, visit.recorded_type, &operand);
                    self.want(walk, expr, &operand, "operand of a unary operator");
                }
            }
        }
        walk.pending.push(WalkStep::Expression(expr, visit.position));
    }

    /// A binary operator: a comparison of aggregates, a digital operator, or an operator of `f64`s
    /// and `bool`s. `&&` and `||` evaluate their right side conditionally.
    fn binary(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, kind: &Expr) {
        if self.operates_on_aggregates(kind) {
            self.aggregate_comparison(walk, visit);
            return;
        }
        let Expr::Binary { op, lhs, rhs } = *kind else { return };
        if visit.first {
            if Typing::digital_op(op) || [lhs, rhs].iter().any(|e| self.reader.type_of(*e).is_some_and(Typing::digital)) {
                let want = match (self.reader.type_of(lhs), self.reader.type_of(rhs)) {
                    (Some(l), Some(r)) => Typing::binary(op, l, r),
                    _ => None,
                };
                self.typed(walk, &visit.origin, visit.recorded_type, want, "a binary operator");
            } else {
                self.analog_binary(walk, visit, op, lhs, rhs);
            }
        }
        walk.pending.push(WalkStep::Expression(lhs, visit.position));
        let short = matches!(op, BinaryOp::And | BinaryOp::Or);
        walk.pending.push(WalkStep::Expression(rhs, WalkPosition { conditional: visit.position.conditional || short, ..visit.position }));
    }

    /// An operator of `f64`s or `bool`s: its operands have the type it takes, and it gives its own.
    fn analog_binary(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, op: BinaryOp, lhs: ExprId, rhs: ExprId) {
        let (operands, result) = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => (Some(ValueType::F64), ValueType::F64),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => (Some(ValueType::F64), ValueType::Bool),
            BinaryOp::And | BinaryOp::Or => (Some(ValueType::Bool), ValueType::Bool),
            BinaryOp::Eq | BinaryOp::Ne => (None, ValueType::Bool),
            // Taken by the digital operators.
            BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => (None, ValueType::Bool),
        };
        self.own(walk, &visit.origin, visit.recorded_type, &result);
        match operands {
            Some(t) => {
                self.want(walk, lhs, &t, "operand of an operator");
                self.want(walk, rhs, &t, "operand of an operator");
            }
            None => {
                if let (Some(l), Some(r)) = (self.reader.type_of(lhs), self.reader.type_of(rhs)) {
                    if l != r {
                        self.wrong(walk, visit.origin.clone(), format!("compares {l} with {r}"));
                    }
                }
            }
        }
    }

    /// An `if`: a `bool` condition, two arms of the expression's type, each evaluated conditionally.
    fn if_form(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, cond: ExprId, arms: [ExprId; 2]) {
        if visit.first {
            self.want(walk, cond, &ValueType::Bool, "condition of an `if`");
            for arm in arms {
                self.want(walk, arm, visit.recorded_type, "arm of an `if`");
            }
        }
        walk.pending.push(WalkStep::Expression(cond, visit.position));
        let arm = WalkPosition { conditional: true, ..visit.position };
        walk.pending.extend(arms.map(|id| WalkStep::Expression(id, arm)));
    }

    /// A block: its value has its type (`()` without one), and the `let`s of its statements end with it.
    fn block(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, stmts: &[crate::ids::StmtId], value: Option<ExprId>) {
        if visit.first {
            match value {
                Some(v) => self.want(walk, v, visit.recorded_type, "value of a block"),
                None => self.own(walk, &visit.origin, visit.recorded_type, &ValueType::Unit),
            }
        }
        for &s in stmts {
            walk.pending.push(WalkStep::Statement(s, visit.position, false));
        }
        if let Some(v) = value {
            walk.pending.push(WalkStep::Expression(v, visit.position));
        }
        // The block's `let`s end with it.
        let bound = walk.bound.clone();
        walk.pending.push(WalkStep::Restore(bound));
    }

    /// A call of a function of the arena: its result type, one argument of each parameter's type.
    fn call(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, function: FunctionId, args: &[ExprId]) {
        if let Some(f) = self.reader.ir.functions.get(function.index()) {
            walk.calls.insert(function);
            if visit.first {
                self.own(walk, &visit.origin, visit.recorded_type, &f.ret);
                if args.len() != f.params.len() {
                    self.wrong(walk, visit.origin.clone(), format!("calls {} with {} args, it takes {}", f.name, args.len(), f.params.len()));
                }
                for (a, p) in args.iter().zip(&f.params) {
                    self.want(walk, *a, &p.ty, "argument of a call");
                }
            }
        }
        walk.pending.extend(args.iter().map(|a| WalkStep::Expression(*a, visit.position)));
    }

    /// A math function: typed by its operand (H.2), a float of any width giving a float of that
    /// width, `f64` otherwise; it takes one argument.
    fn math(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, args: &[ExprId]) {
        if visit.first {
            let operand = args.first().and_then(|a| self.reader.type_of(*a)).filter(|t| t.is_other_float()).cloned().unwrap_or(ValueType::F64);
            self.own(walk, &visit.origin, visit.recorded_type, &operand);
            if args.len() != 1 {
                self.wrong(walk, visit.origin.clone(), format!("a math function takes 1 argument, found {}", args.len()));
            }
            for a in args {
                self.want(walk, *a, &operand, "argument of a math function");
            }
        }
        walk.pending.extend(args.iter().map(|a| WalkStep::Expression(*a, visit.position)));
    }

    /// `table_model` (§22.2.15): an `f64` read at an `f64`, of a table of at least two samples whose
    /// abscissae strictly increase (elaboration reports `E0704`; the IR is held to the same rule).
    fn table_model(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, x: ExprId, table: &[(Real, Real)]) {
        if visit.first {
            self.own(walk, &visit.origin, visit.recorded_type, &ValueType::F64);
            self.want(walk, x, &ValueType::F64, "argument of a table model");
            if table.len() < 2 || table.windows(2).any(|pair| pair[0].0.get() >= pair[1].0.get()) {
                self.wrong(walk, visit.origin.clone(), "a table model needs two or more samples with strictly increasing abscissae".to_string());
            }
        }
        walk.pending.push(WalkStep::Expression(x, visit.position));
    }

    /// A closure: a function type of as many parameters, a body of its result type, its parameters
    /// bound in its body only.
    fn closure(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, params: &[u32], body: ExprId) {
        let outside = walk.bound.clone();
        for p in params {
            match walk.bound.get_mut(*p as usize) {
                Some(slot) => *slot = true,
                None if visit.first => self.wrong(walk, visit.origin.clone(), format!("closure parameter local {p} out of range")),
                None => {}
            }
        }
        if visit.first {
            match visit.recorded_type {
                ValueType::Fn(ps, ret) => {
                    if ps.len() != params.len() {
                        self.wrong(walk, visit.origin.clone(), format!("closure takes {} parameters, its type {}", params.len(), ps.len()));
                    }
                    self.want(walk, body, ret, "body of a closure");
                }
                other => self.wrong(walk, visit.origin.clone(), format!("a closure must have a function type, found {other}")),
            }
        }
        walk.intern_scope();
        walk.pending.push(WalkStep::Expression(body, WalkPosition { closure: true, enclosing_loop: None, ..visit.position }));
        // The parameters end with the closure.
        walk.pending.push(WalkStep::Restore(outside));
    }

    /// A probe: an `f64` of a body, between nodes of the body or the ground.
    fn probe(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, terminals: [Terminal; 2]) {
        if !visit.first {
            return;
        }
        let origin = &visit.origin;
        self.own(walk, origin, visit.recorded_type, &ValueType::F64);
        if !walk.owner.in_body {
            self.wrong(walk, origin.clone(), "probe outside a body".to_string());
        }
        for t in terminals {
            match t {
                Terminal::Node(i) if walk.owner.in_body && i as usize >= walk.owner.nodes => {
                    self.wrong(walk, origin.clone(), format!("probe terminal node {i} out of range ({} nodes)", walk.owner.nodes));
                }
                Terminal::Own(i) => self.wrong(walk, origin.clone(), format!("probe terminal own{i}: an own terminal belongs in a child's node slot only")),
                Terminal::Node(_) | Terminal::Ground => {}
            }
        }
    }

    /// A flow read: an `f64` of a branch of the body that the body drives with a potential.
    fn flow_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, branch: u32) {
        if !visit.first {
            return;
        }
        let origin = &visit.origin;
        self.own(walk, origin, visit.recorded_type, &ValueType::F64);
        if !walk.owner.in_body {
            self.wrong(walk, origin.clone(), "a flow read outside a body".to_string());
        } else if branch as usize >= walk.owner.branches.len() {
            self.wrong(walk, origin.clone(), format!("flow of branch {branch}, out of range ({} branches)", walk.owner.branches.len()));
        } else if !walk.owner.driven.contains(&branch) {
            self.wrong(walk, origin.clone(), format!("flow of branch {branch}, which the body does not drive with a potential contribution"));
        }
    }

    /// An environment read: a `bool` for `static()`, an analysis test and `temperature_given`, an
    /// `f64` otherwise.
    fn environment_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, env: Env) {
        if visit.first {
            let want = if matches!(env, Env::Static | Env::Analysis(_) | Env::TemperatureGiven) { ValueType::Bool } else { ValueType::F64 };
            self.own(walk, &visit.origin, visit.recorded_type, &want);
        }
    }

    /// A `limit` of site `site`: an `f64` of an `f64` input through a function value, with an `f64`
    /// seed; the site is placed as a `limit` may be.
    fn limit(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, site: u32, [x, f]: [ExprId; 2], seed: Option<ExprId>) {
        if visit.first {
            self.own(walk, &visit.origin, visit.recorded_type, &ValueType::F64);
            self.want(walk, x, &ValueType::F64, "input of a `limit`");
            if let Some(got) = self.reader.type_of(f) {
                if !matches!(got, ValueType::Fn(..)) {
                    let o = self.reader.expression_origin(f);
                    self.wrong(walk, o, format!("the function of a `limit` must have a function type, found {got}"));
                }
            }
            if let Some(s) = seed {
                self.want(walk, s, &ValueType::F64, "seed of a `limit`");
            }
        }
        self.site(walk, visit.id, &visit.origin, "limit", site, visit.position);
        walk.pending.push(WalkStep::Expression(x, visit.position));
        walk.pending.push(WalkStep::Expression(f, visit.position));
        walk.pending.extend(seed.iter().map(|s| WalkStep::Expression(*s, visit.position)));
    }

    /// An `ac_stim` of site `site`: an `f64` of an `f64` magnitude and phase; the site is placed as an
    /// `ac_stim` may be.
    fn stimulus(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, site: u32, mag: ExprId, phase: Option<ExprId>) {
        if visit.first {
            self.own(walk, &visit.origin, visit.recorded_type, &ValueType::F64);
            self.want(walk, mag, &ValueType::F64, "magnitude of an `ac_stim`");
            if let Some(p) = phase {
                self.want(walk, p, &ValueType::F64, "phase of an `ac_stim`");
            }
        }
        self.site(walk, visit.id, &visit.origin, "ac_stim", site, visit.position);
        walk.pending.push(WalkStep::Expression(mag, visit.position));
        walk.pending.extend(phase.iter().map(|p| WalkStep::Expression(*p, visit.position)));
    }

    /// An integer literal: of an integer type, within its range.
    fn integer_literal(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, value: i128) {
        if !visit.first {
            return;
        }
        let ty = visit.recorded_type;
        match ty.integer_range() {
            Some((lo, hi)) if !(lo..=hi).contains(&value) => {
                self.wrong(walk, visit.origin.clone(), format!("integer literal {value} is outside the range of {ty}"));
            }
            Some(_) => {}
            None if ty.is_integer() => {}
            None => self.wrong(walk, visit.origin.clone(), format!("an integer literal must have an integer type, found {ty}")),
        }
    }

    /// A cast `expr as to`: the type `Typing` gives the cast.
    fn cast(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, expr: ExprId, to: &ValueType) {
        if visit.first {
            self.typed(walk, &visit.origin, visit.recorded_type, self.reader.type_of(expr).and_then(|t| Typing::cast(t, to)), "a cast");
        }
        walk.pending.push(WalkStep::Expression(expr, visit.position));
    }

    /// A method call: the type `Typing` gives the method on its receiver and arguments.
    fn method_call(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, method: crate::model::Method, receiver: ExprId, args: &[ExprId]) {
        if visit.first {
            let arg_types: Option<Vec<&ValueType>> = args.iter().map(|a| self.reader.type_of(*a)).collect();
            let want = match (self.reader.type_of(receiver), arg_types) {
                (Some(r), Some(a)) => Typing::method(method, r, &a),
                _ => None,
            };
            self.typed(walk, &visit.origin, visit.recorded_type, want, "a method");
        }
        walk.pending.push(WalkStep::Expression(receiver, visit.position));
        walk.pending.extend(args.iter().map(|a| WalkStep::Expression(*a, visit.position)));
    }

    /// `same(a, b)`: the type `Typing` gives it.
    fn same(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, a: ExprId, b: ExprId) {
        if visit.first {
            let want = match (self.reader.type_of(a), self.reader.type_of(b)) {
                (Some(a), Some(b)) => Typing::same(a, b),
                _ => None,
            };
            self.typed(walk, &visit.origin, visit.recorded_type, want, "`same`");
        }
        walk.pending.push(WalkStep::Expression(a, visit.position));
        walk.pending.push(WalkStep::Expression(b, visit.position));
    }

    /// A level `cast`: the type `Typing` gives it over the type arena.
    fn level_cast(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, expr: ExprId, to: &ValueType) {
        if visit.first {
            let want = self.reader.type_of(expr).and_then(|t| Typing::level_cast(t, to, &self.reader.ir.types));
            self.typed(walk, &visit.origin, visit.recorded_type, want, "`cast`");
        }
        walk.pending.push(WalkStep::Expression(expr, visit.position));
    }

    /// The previous value of param `p`: in a body, of the param's type.
    fn param_previous(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, p: u32) {
        if !visit.first {
            return;
        }
        let origin = visit.origin.clone();
        if !walk.owner.in_body {
            self.wrong(walk, origin, "the previous value of a param outside a body".to_string());
        } else {
            match walk.owner.params.get(p as usize) {
                // A param's previous value has its type.
                Some(param) => self.own(walk, &origin, visit.recorded_type, &param.clone()),
                None => self.wrong(walk, origin, format!("previous value of param {p}, out of range ({} params)", walk.owner.params.len())),
            }
        }
    }

    /// A read of derivative site `k`: an `f64` of a site of the body.
    fn derivative_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, k: u32) {
        if !visit.first {
            return;
        }
        self.own(walk, &visit.origin, visit.recorded_type, &ValueType::F64);
        if !walk.owner.in_body {
            self.wrong(walk, visit.origin.clone(), "a derivative read outside a body".to_string());
        } else if k as usize >= walk.owner.derivatives {
            self.wrong(walk, visit.origin.clone(), format!("derivative read of site {k}, out of range ({} sites)", walk.owner.derivatives));
        }
    }

    /// A read of delay site `k`: an `f64` of a site of the body.
    fn delay_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, k: u32) {
        if !visit.first {
            return;
        }
        self.own(walk, &visit.origin, visit.recorded_type, &ValueType::F64);
        if !walk.owner.in_body {
            self.wrong(walk, visit.origin.clone(), "a delay read outside a body".to_string());
        } else if k as usize >= walk.owner.delays {
            self.wrong(walk, visit.origin.clone(), format!("delay read of site {k}, out of range ({} sites)", walk.owner.delays));
        }
    }

    /// A read of noise source `k`: an `f64` of a source of the body.
    fn noise_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, k: u32) {
        if !visit.first {
            return;
        }
        self.own(walk, &visit.origin, visit.recorded_type, &ValueType::F64);
        if !walk.owner.in_body {
            self.wrong(walk, visit.origin.clone(), "a noise read outside a body".to_string());
        } else if k as usize >= walk.owner.noise_sources {
            self.wrong(walk, visit.origin.clone(), format!("noise read of source {k}, out of range ({} sources)", walk.owner.noise_sources));
        }
    }

    /// A read of session parameter `parameter`: an `Option<f64>` of a parameter of the body.
    fn session_parameter_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, parameter: u32) {
        if !visit.first {
            return;
        }
        self.own(walk, &visit.origin, visit.recorded_type, &ValueType::Option(Box::new(ValueType::F64)));
        if !walk.owner.in_body {
            self.wrong(walk, visit.origin.clone(), "a session parameter read outside a body".to_string());
        } else if parameter as usize >= walk.owner.session_parameters {
            self.wrong(walk, visit.origin.clone(), format!("session parameter read {parameter}, out of range ({} parameters)", walk.owner.session_parameters));
        }
    }

    /// `fired` of event `e`: a `bool`, in a reaction or the guard of another event of the body.
    fn fired(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, e: u32) {
        if !visit.first {
            return;
        }
        let origin = visit.origin.clone();
        self.own(walk, &origin, visit.recorded_type, &ValueType::Bool);
        if !walk.owner.reaction && walk.owner.event.is_none() {
            self.wrong(walk, origin, "`fired` outside a reaction or the guard of an event".to_string());
        } else if walk.owner.event == Some(e) {
            self.wrong(walk, origin, format!("the guard of event {e} reads its own `fired`"));
        } else if e as usize >= walk.owner.events {
            self.wrong(walk, origin, format!("`fired` of event {e}, out of range ({} events)", walk.owner.events));
        }
    }

    /// An `unwrap`: a `bool` presence flag, and the type of its value.
    fn unwrap_form(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, is: ExprId, value: ExprId) {
        if visit.first {
            self.want(walk, is, &ValueType::Bool, "the presence flag of an `unwrap`");
            if let Some(value_ty) = self.reader.type_of(value).cloned() {
                self.own(walk, &visit.origin, visit.recorded_type, &value_ty);
            }
        }
        walk.pending.push(WalkStep::Expression(is, visit.position));
        walk.pending.push(WalkStep::Expression(value, visit.position));
    }

    /// A read of carrier `i`: in a body, the value it carries; an array of carriers read whole is the
    /// array of its elements' values (CV-24).
    fn carrier_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, i: u32) {
        if !visit.first {
            return;
        }
        let origin = visit.origin.clone();
        if !walk.owner.in_body {
            self.wrong(walk, origin, "carrier read outside a body".to_string());
            return;
        }
        match walk.owner.carriers.get(i as usize) {
            Some(c) => match walk.owner.counts.get(i as usize).copied().flatten() {
                Some(count) => self.own(walk, &origin, visit.recorded_type, &ValueType::Array(Box::new(c.clone()), count)),
                None => self.own(walk, &origin, visit.recorded_type, &Typing::read_of(c)),
            },
            None => self.wrong(walk, origin, format!("carrier {i} out of range ({} carriers)", walk.owner.carriers.len())),
        }
    }

    /// `others()` of carrier `i`: in a body, of a resolved carrier, of its type.
    fn others_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, i: u32) {
        if !visit.first {
            return;
        }
        let origin = visit.origin.clone();
        match walk.owner.carriers.get(i as usize) {
            _ if !walk.owner.in_body => self.wrong(walk, origin, "`others()` outside a body".to_string()),
            Some(held) if walk.owner.drive_types.get(i as usize).is_some_and(Option::is_some) => self.own(walk, &origin, visit.recorded_type, held),
            Some(_) => self.wrong(walk, origin, format!("`others()` of carrier {i}, which is not a resolved carrier")),
            None => self.wrong(walk, origin, format!("`others()` of carrier {i}, out of range ({} carriers)", walk.owner.carriers.len())),
        }
    }

    /// A `resolve` form: its own checks, and the functions it calls.
    fn resolve(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, by_count: &[FunctionId], drives: &[u32], except: Option<u32>) {
        if visit.first {
            self.resolve_form(walk, &visit.origin, visit.recorded_type, by_count, drives, except);
        }
        walk.calls.extend(by_count.iter().copied().filter(|f| f.index() < self.reader.ir.functions.len()));
    }

    /// The previous value of carrier `i`: in a body, of a carrier that is no array of carriers.
    fn previous_read(&mut self, walk: &CodeWalk, visit: &ExpressionVisit, i: u32) {
        if !visit.first {
            return;
        }
        let origin = visit.origin.clone();
        if !walk.owner.in_body {
            self.wrong(walk, origin, "a previous-value read outside a body".to_string());
            return;
        }
        match walk.owner.carriers.get(i as usize) {
            Some(_) if walk.owner.counts.get(i as usize).copied().flatten().is_some() => {
                self.wrong(walk, origin, format!("previous value of carrier {i}, an array of carriers: read one element's"));
            }
            Some(c) => self.own(walk, &origin, visit.recorded_type, &Typing::read_of(c)),
            None => self.wrong(walk, origin, format!("previous value of carrier {i}, out of range ({} carriers)", walk.owner.carriers.len())),
        }
    }

    /// An element of an array of carriers: read, its previous value, or its `changed()`.
    fn element(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, kind: &Expr) {
        let (form, carrier, index, want_bool) = match *kind {
            Expr::CarrierElement { carrier, index } => ("an element read", carrier, index, false),
            Expr::ElementPrevious { carrier, index } => ("an element's previous value", carrier, index, false),
            Expr::ElementChanged { carrier, index } => ("an element's `changed()`", carrier, index, true),
            _ => return,
        };
        if visit.first {
            self.element_read(walk, &visit.origin, form, carrier, index, visit.recorded_type, want_bool);
        }
        walk.pending.push(WalkStep::Expression(index, visit.position));
    }

    /// The walk of a loop expression: the checks of its own form (on the first visit), then the
    /// steps of its operands and its body, which run inside it (`break` and `continue` look the loop
    /// up through `WalkPosition::enclosing_loop`).
    fn looping(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, kind: &Expr) {
        let (ty, origin, position) = (visit.recorded_type, &visit.origin, visit.position);
        let inside = WalkPosition { conditional: true, enclosing_loop: Some(visit.id), ..position };
        let outside = walk.bound.clone();
        match kind {
            Expr::While { cond, .. } => {
                if visit.first {
                    self.carried(walk, *cond, &ValueType::Bool, "the condition of a `while`");
                    self.own(walk, origin, ty, &ValueType::Unit);
                }
                walk.pending.push(WalkStep::Expression(*cond, position));
            }
            Expr::ForRange { local, low, high, .. } => {
                if visit.first {
                    self.own(walk, origin, ty, &ValueType::Unit);
                    if let (Some(l), Some(h)) = (self.reader.type_of(*low), self.reader.type_of(*high)) {
                        if l != h || !l.is_integer() {
                            self.wrong(walk, origin.clone(), format!("the bounds of a `for` range must have one integer type, found {l} and {h}"));
                        } else if walk.owner.locals.get(*local as usize) != Some(l) {
                            self.wrong(walk, origin.clone(), format!("the variable of a `for` range must be a local of type {l}"));
                        }
                    }
                }
                walk.pending.extend([WalkStep::Expression(*low, position), WalkStep::Expression(*high, position), WalkStep::Bind(*local)]);
            }
            _ => {}
        }
        walk.pending.extend(kind.statements().iter().map(|&s| WalkStep::Statement(s, inside, false)));
        // The body's `let`s, and a range's variable, end with the loop.
        walk.pending.push(WalkStep::Restore(outside));
    }

    /// A `break`, with the value it carries.
    fn break_form(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, value: Option<ExprId>) {
        self.leave_loop(walk, &visit.origin, visit.position, value, true);
        walk.pending.extend(value.iter().map(|v| WalkStep::Expression(*v, visit.position)));
    }

    /// A `break` (`is_break`, with `value` when it carries one) or a `continue` at `origin`: it
    /// must sit in a loop, and a `break` value only out of a `loop`, of the type the loop gives.
    fn leave_loop(&mut self, walk: &CodeWalk, origin: &Origin, position: WalkPosition, value: Option<ExprId>, is_break: bool) {
        let word = if is_break { "break" } else { "continue" };
        let Some(enclosing) = position.enclosing_loop else {
            self.wrong(walk, origin.clone(), format!("`{word}` outside a loop"));
            return;
        };
        if !is_break {
            return;
        }
        let loop_ty = self.reader.type_of(enclosing).cloned();
        match (self.reader.ir.exprs.get(enclosing.index()).map(|d| &d.kind), value, loop_ty) {
            (Some(Expr::Loop { .. }), Some(v), Some(ty)) => self.want(walk, v, &ty, "the value of a `break`"),
            (Some(Expr::Loop { .. }), None, Some(ty)) if ty != ValueType::Unit => {
                self.wrong(walk, origin.clone(), format!("`break` without a value out of a `loop` of type {ty}"));
            }
            (Some(Expr::Loop { .. }), ..) => {}
            (_, Some(_), _) => self.wrong(walk, origin.clone(), "`break` with a value out of a loop that is not `loop`".to_string()),
            _ => {}
        }
    }

    /// A `return`: in a function, outside every closure, with a value of the function's result type.
    fn return_form(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, value: Option<ExprId>) {
        let origin = &visit.origin;
        if walk.owner.in_body {
            self.wrong(walk, origin.clone(), "`return` outside a function".to_string());
        } else if visit.position.closure {
            self.wrong(walk, origin.clone(), "`return` inside a closure".to_string());
        } else {
            let result = walk.owner.result.clone();
            match value {
                Some(v) => self.want(walk, v, &result, "the value of a `return`"),
                None if result != ValueType::Unit => self.wrong(walk, origin.clone(), format!("`return` without a value in a function that returns {result}")),
                None => {}
            }
        }
        walk.pending.extend(value.iter().map(|v| WalkStep::Expression(*v, visit.position)));
    }

    /// A reference to a function of the arena: the function type of its parameters and result.
    fn function_reference(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, function: FunctionId) {
        if let Some(f) = self.reader.ir.functions.get(function.index()) {
            walk.calls.insert(function);
            if visit.first {
                let want = ValueType::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone()));
                self.own(walk, &visit.origin, visit.recorded_type, &want);
            }
        }
    }

    /// A call through a function value: the callee has a function type, the call its result type, and
    /// one argument of each parameter's type.
    fn call_value(&mut self, walk: &mut CodeWalk, visit: &ExpressionVisit, callee: ExprId, args: &[ExprId]) {
        if visit.first {
            match self.reader.type_of(callee) {
                Some(ValueType::Fn(params, ret)) => {
                    self.own(walk, &visit.origin, visit.recorded_type, ret);
                    if params.len() != args.len() {
                        self.wrong(walk, visit.origin.clone(), format!("calls a function value with {} args, it takes {}", args.len(), params.len()));
                    }
                    for (a, p) in args.iter().zip(params) {
                        self.want(walk, *a, p, "argument of a call through a function value");
                    }
                }
                Some(other) => self.wrong(walk, visit.origin.clone(), format!("the callee of a call through a value must have a function type, found {other}")),
                None => {}
            }
        }
        walk.pending.push(WalkStep::Expression(callee, visit.position));
        walk.pending.extend(args.iter().map(|a| WalkStep::Expression(*a, visit.position)));
    }

    /// A `limit` (`kind` "limit") or `ac_stim` site used by expression `user`: placement in this
    /// position, range, and the distinct expressions that use it.
    fn site(&mut self, walk: &mut CodeWalk, user: ExprId, origin: &Origin, kind: &str, site: u32, position: WalkPosition) {
        let count = if kind == "limit" { walk.owner.limits } else { walk.owner.stimulus_sites };
        if !walk.owner.analog && !(kind == "ac_stim" && walk.owner.value_input) {
            self.wrong(walk, origin.clone(), format!("{kind} outside the analog block of a body"));
        } else if position.closure {
            self.wrong(walk, origin.clone(), format!("{kind} inside a closure"));
        }
        if site as usize >= count {
            self.wrong(walk, origin.clone(), format!("{kind} site {site} out of range ({count} sites)"));
            return;
        }
        // An `ac_stim` may stand under a condition: its terms in an arm not taken read no
        // stimulus. A `limit` under a condition says when it runs, through `enabled`; one at the
        // top has none (analog-structure, "Guarded core sites").
        if kind == "limit" && walk.owner.analog && !position.closure {
            match (position.conditional, walk.owner.limit_guards[site as usize]) {
                (true, false) => self.wrong(walk, origin.clone(), format!("limit site {site} under an `if` or a short-circuit operator has no `enabled`")),
                (false, true) => self.wrong(walk, origin.clone(), format!("limit site {site} has an `enabled` but is read outside every condition")),
                (true, true) | (false, false) => {}
            }
        }
        let users = if kind == "limit" { &mut walk.limit_users } else { &mut walk.stimulus_users };
        if users.len() < count {
            users.resize(count, BTreeSet::new());
        }
        users[site as usize].insert(user);
    }
}
