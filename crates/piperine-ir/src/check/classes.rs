//! The class check: what each class of position may read and write. A body writes only the carriers
//! it may (never an `in` port or a reached carrier), a carrier has a drive or commits, never both, a
//! reaction waits on events of its body, a `cross` and an `at` watch a real, a tolerance is a constant
//! or a param expression, a commit and an emit sit in a reaction, a drive, an event guard or an `at`
//! argument reads no input, and a placement's param value reads only the parent's params.

use std::collections::{BTreeSet, HashSet};

use super::ClassCheck;
use crate::ids::{ExprId, StmtId};
use crate::model::{BodyData, CarrierRole, EmitField, Env, Event, EventSource, Expr, Origin, Stmt, ValueType};

/// Which element of its carrier a write names, as far as the IR says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElementWritten {
    /// The whole carrier (or its only value).
    Whole,
    /// One element, known.
    Constant(u64),
    /// An element chosen at run time: any of them.
    Dynamic,
}

impl ElementWritten {
    /// Whether two writes of one carrier can reach one element.
    fn overlaps(self, other: ElementWritten) -> bool {
        match (self, other) {
            (ElementWritten::Constant(a), ElementWritten::Constant(b)) => a == b,
            _ => true,
        }
    }
}

impl ClassCheck<'_> {
    /// Runs the class rules of every body, then of every function.
    pub(super) fn run(&mut self) {
        let ir = self.reader.ir;
        for body in &ir.bodies {
            self.writes(body);
            self.ambient_reads(body);
            let item = format!("body {}", body.name);
            for (k, reaction) in body.reactions.iter().enumerate() {
                for &e in &reaction.on {
                    if e as usize >= body.events.len() {
                        self.findings.report(format!("{item} reaction {k}"), reaction.origin.clone(), format!("waits on event {e}, out of range ({} events)", body.events.len()));
                    }
                }
            }
            for (k, event) in body.events.iter().enumerate() {
                self.event_source(&format!("{item} event {k}"), body, event);
            }
            self.derivative_tolerances(&item, body);
            self.writes_outside_reactions(&item, body);
        }
        for function in &ir.functions {
            let (_, stmts) = self.reach(&[], &[function.body]);
            self.report_writes(&format!("function {}", function.name), &stmts);
        }
    }

    /// The source of an event named `item`: a `cross` watches an `f64` with `f64` tolerances that
    /// read no input, a `changed` of a param names a param of `body`, an `at` waits for an `f64` time
    /// and reads no probe.
    fn event_source(&mut self, item: &str, body: &BodyData, event: &Event) {
        if let EventSource::Cross { e, ttol, etol } = event.source {
            if let Some(ty) = self.reader.type_of(e).filter(|t| **t != ValueType::F64) {
                self.findings.report(item.to_string(), event.origin.clone(), format!("a `cross` watches a real, found {ty}"));
            }
            for (name, tol) in [("ttol", ttol), ("etol", etol)] {
                let Some(tol) = tol else { continue };
                if let Some(ty) = self.reader.type_of(tol).filter(|t| **t != ValueType::F64) {
                    self.findings.report(item.to_string(), event.origin.clone(), format!("the `{name}` of a `cross` is an `f64`, found {ty}"));
                }
                self.constant_reads(item, tol, &format!("the `{name}` of a `cross` reads an input: it is a constant or a param expression"));
            }
        }
        if let EventSource::ParamChanged(p) = event.source {
            if p as usize >= body.params.len() {
                self.findings.report(item.to_string(), event.origin.clone(), format!("`changed` of param {p}, out of range ({} params)", body.params.len()));
            }
        }
        if let EventSource::At(e) = event.source {
            if let Some(ty) = self.reader.type_of(e).filter(|t| **t != ValueType::F64) {
                self.findings.report(item.to_string(), event.origin.clone(), format!("an `at` waits for a time, an `f64`, found {ty}"));
            }
            for read in self.reach_calls(&[e]) {
                if matches!(self.reader.ir.exprs.get(read.index()).map(|d| &d.kind), Some(Expr::Probe { .. })) {
                    let at = self.reader.expression_origin(read);
                    self.findings.report(item.to_string(), at, "an `at` argument reads a probe: a discrete function reads no input (abi.md §4.2)".to_string());
                }
            }
        }
    }

    /// The `abstol` of a derivative site is a constant or a param expression, as the tolerances of a
    /// `cross` are.
    fn derivative_tolerances(&mut self, item: &str, body: &BodyData) {
        for (k, site) in body.derivatives.iter().enumerate() {
            let Some(abstol) = site.abstol else { continue };
            self.constant_reads(&format!("{item} derivative {k}"), abstol, "the `abstol` of a derivative site reads an input: it is a constant or a param expression");
        }
    }

    /// Reports `what`, against `item`, at each probe, carrier, `time()` or `freq()` the value of
    /// `root` reads: a value that must be a constant or a param expression.
    fn constant_reads(&mut self, item: &str, root: ExprId, what: &str) {
        for read in self.reach_calls(&[root]) {
            if matches!(self.reader.ir.exprs.get(read.index()).map(|d| &d.kind), Some(Expr::Probe { .. } | Expr::Carrier(_) | Expr::Env(Env::Time | Env::Freq))) {
                let at = self.reader.expression_origin(read);
                self.findings.report(item.to_string(), at, what.to_string());
            }
        }
    }

    /// A commit or an emit anywhere in `body` but in a reaction: the analog block, a `where`, a
    /// drive, an event.
    fn writes_outside_reactions(&mut self, item: &str, body: &BodyData) {
        let mut roots: Vec<ExprId> = body.params.iter().filter_map(|p| p.check).collect();
        roots.extend(body.derivatives.iter().flat_map(|d| [d.state].into_iter().chain(d.abstol)));
        roots.extend(body.noise_sources.iter().map(|n| n.density));
        roots.extend(body.delays.iter().flat_map(|d| [d.input, d.time].into_iter().chain(d.maximum)));
        roots.extend(body.drives.iter().flat_map(|d| [d.value].into_iter().chain(d.after)));
        roots.extend(body.events.iter().flat_map(|e| {
            let cross = match e.source {
                EventSource::Cross { e: x, ttol, etol } => [Some(x), ttol, etol],
                EventSource::At(x) => [Some(x), None, None],
                EventSource::Changed(_) | EventSource::ParamChanged(_) | EventSource::Settle | EventSource::ElementChanged { .. } => [None; 3],
            };
            cross.into_iter().flatten().chain(e.guard)
        }));
        let (_, stmts) = self.reach(body.analog.as_deref().unwrap_or(&[]), &roots);
        self.report_writes(item, &stmts);
    }

    /// Reports, against `item`, each commit and then each emit among `stmts`: they sit outside a
    /// reaction.
    fn report_writes(&mut self, item: &str, stmts: &[StmtId]) {
        let ir = self.reader.ir;
        for (s, _) in self.commits(stmts) {
            let origin = self.reader.statement_origin(&ir.stmts[s.index()]);
            self.findings.report(item.to_string(), origin, "a commit outside a reaction".to_string());
        }
        for s in self.emits(stmts) {
            let origin = self.reader.statement_origin(&ir.stmts[s.index()]);
            self.findings.report(item.to_string(), origin, "an emit outside a reaction".to_string());
        }
    }

    /// Who writes each carrier: an `in` port is written by its driver outside, never by the body;
    /// any other carrier has drives or commits, never both.
    fn writes(&mut self, body: &BodyData) {
        let item = format!("body {}", body.name);
        let input = |c: u32| body.carriers.get(c as usize).filter(|c| c.role == CarrierRole::In);
        let reached = |c: u32| body.carriers.get(c as usize).filter(|c| c.role == CarrierRole::Reached);
        for drive in &body.drives {
            if let Some(c) = input(drive.carrier) {
                self.findings.report(format!("{item} drive {}", c.name), drive.origin.clone(), format!("drives the `in` port {}: an input is written outside the body", c.name));
            }
            if let Some(c) = reached(drive.carrier) {
                self.findings.report(format!("{item} drive {}", c.name), drive.origin.clone(), format!("drives the reached carrier {}: a child's carrier is read only", c.name));
            }
        }
        let mut committed: Vec<(u32, ElementWritten)> = Vec::new();
        for (k, reaction) in body.reactions.iter().enumerate() {
            let (_, stmts) = self.reach(&reaction.stmts, &[]);
            for (s, carrier) in self.commits(&stmts) {
                let stmt = &self.reader.ir.stmts[s.index()];
                committed.push((carrier, self.element_written(stmt)));
                if let Some(c) = input(carrier) {
                    let origin = self.reader.statement_origin(stmt);
                    self.findings.report(format!("{item} reaction {k}"), origin, format!("commits to the `in` port {}: an input is written outside the body", c.name));
                }
                if let Some(c) = reached(carrier) {
                    let origin = self.reader.statement_origin(stmt);
                    self.findings.report(format!("{item} reaction {k}"), origin, format!("commits to the reached carrier {}: a child's carrier is read only", c.name));
                }
            }
        }
        // a definition and a commit of one carrier, or of one element of an array carrier (CV-25 AC 3: two elements
        // are two carriers), never both; a commit whose element is not constant may reach any element
        let mut clashing: BTreeSet<u32> = BTreeSet::new();
        for drive in &body.drives {
            let written = drive.element.map_or(ElementWritten::Whole, ElementWritten::Constant);
            if committed.iter().any(|(carrier, other)| *carrier == drive.carrier && written.overlaps(*other)) {
                clashing.insert(drive.carrier);
            }
        }
        for c in clashing {
            if let Some(carrier) = body.carriers.get(c as usize) {
                self.findings.report(format!("{item} carrier {}", carrier.name), carrier.origin.clone(), format!("carrier {} has both a drive and a commit", carrier.name));
            }
        }
    }

    /// The element of its carrier a commit writes.
    fn element_written(&self, stmt: &Stmt) -> ElementWritten {
        let Stmt::Commit { target, .. } = stmt else { return ElementWritten::Whole };
        match target.as_ref().and_then(|t| t.element) {
            None => ElementWritten::Whole,
            Some(index) => match self.reader.ir.exprs.get(index.index()).map(|d| &d.kind) {
                Some(Expr::Int(n)) if *n >= 0 => ElementWritten::Constant(*n as u64),
                _ => ElementWritten::Dynamic,
            },
        }
    }

    /// abi.md §4.2: no discrete function reads an input, so `time()` and `static()` are read by
    /// `analog` and by reactions, never by a drive, an event guard or an `at` argument.
    fn ambient_reads(&mut self, body: &BodyData) {
        let item = format!("body {}", body.name);
        let mut positions: Vec<(String, Origin, ExprId)> = Vec::new();
        for drive in &body.drives {
            let name = body.carriers.get(drive.carrier as usize).map_or("<bad carrier>", |c| c.name.as_str());
            for e in [drive.value].into_iter().chain(drive.after) {
                positions.push((format!("{item} drive {name}"), drive.origin.clone(), e));
            }
        }
        for (k, event) in body.events.iter().enumerate() {
            let at = match event.source {
                EventSource::At(e) => Some(e),
                EventSource::Changed(_) | EventSource::Cross { .. } | EventSource::ParamChanged(_) | EventSource::Settle | EventSource::ElementChanged { .. } => None,
            };
            for e in event.guard.into_iter().chain(at) {
                positions.push((format!("{item} event {k}"), event.origin.clone(), e));
            }
        }
        for (position, origin, root) in positions {
            for e in self.reach_calls(&[root]) {
                if let Some(Expr::Env(env @ (Env::Time | Env::Static | Env::Freq))) = self.reader.ir.exprs.get(e.index()).map(|d| &d.kind) {
                    let what = match env {
                        Env::Time => "time()",
                        Env::Freq => "freq()",
                        _ => "static()",
                    };
                    let at = self.reader.ir.exprs.get(e.index()).map_or_else(|| origin.clone(), |d| d.origin.clone());
                    self.findings.report(position.clone(), at, format!("reads {what}: a drive, an event guard and an `at` argument read no input (abi.md §4.2)"));
                }
            }
        }
    }

    /// A param value of a placement reads only params of the parent: no node, carrier, local,
    /// environment, site, flow or previous value, directly or through a function it calls.
    pub(super) fn parent_reads(&mut self, item: &str, root: ExprId) {
        let ir = self.reader.ir;
        let (direct, _) = self.reach(&[], &[root]);
        let direct: BTreeSet<ExprId> = direct.into_iter().collect();
        for e in self.reach_calls(&[root]) {
            let Some(data) = ir.exprs.get(e.index()) else { continue };
            let what = match &data.kind {
                Expr::Probe { .. } => "a probe",
                Expr::Env(_) => "an environment read",
                Expr::Carrier(_) | Expr::Previous(_) => "a carrier",
                Expr::Flow { .. } => "a branch flow",
                Expr::Limit { .. } | Expr::Stim { .. } => "a `limit` or `ac_stim` site",
                Expr::Local(_) | Expr::Block { .. } | Expr::Closure { .. } if direct.contains(&e) => "a local, a block or a closure",
                _ => continue,
            };
            self.findings.report(item.to_string(), data.origin.clone(), format!("the value of a child param reads {what}: only the parent's params may be read"));
        }
    }

    // ---- what code reaches ---------------------------------------------------------------------

    /// The commits among `stmts`, with the carrier each writes.
    fn commits(&self, stmts: &[StmtId]) -> Vec<(StmtId, u32)> {
        stmts
            .iter()
            .filter_map(|s| match self.reader.ir.stmts.get(s.index()) {
                Some(Stmt::Commit { carrier, .. }) => Some((*s, *carrier)),
                _ => None,
            })
            .collect()
    }

    /// The emits among `stmts`.
    fn emits(&self, stmts: &[StmtId]) -> Vec<StmtId> {
        stmts.iter().copied().filter(|s| matches!(self.reader.ir.stmts.get(s.index()), Some(Stmt::Emit { .. }))).collect()
    }

    /// The expressions and statements reachable from `stmts` and `exprs`, through operands and
    /// blocks, each once. Ids out of range are skipped (the reference check names them).
    fn reach(&self, stmts: &[StmtId], exprs: &[ExprId]) -> (Vec<ExprId>, Vec<StmtId>) {
        let ir = self.reader.ir;
        let (mut seen_e, mut seen_s) = (HashSet::new(), HashSet::new());
        let (mut out_e, mut out_s) = (Vec::new(), Vec::new());
        let mut stack_e: Vec<ExprId> = exprs.to_vec();
        let mut stack_s: Vec<StmtId> = stmts.to_vec();
        loop {
            if let Some(s) = stack_s.pop() {
                let Some(stmt) = ir.stmts.get(s.index()) else { continue };
                if !seen_s.insert(s) {
                    continue;
                }
                out_s.push(s);
                match stmt {
                    Stmt::Let { init, .. } => stack_e.push(*init),
                    Stmt::Expr(e) => stack_e.push(*e),
                    Stmt::Assign { value, .. } => stack_e.push(*value),
                    Stmt::Contribute { addends, .. } => stack_e.extend(addends.iter().map(|a| a.value)),
                    Stmt::Commit { value, after, target, .. } => stack_e.extend([*value].into_iter().chain(*after).chain(target.iter().flat_map(crate::model::CommitTarget::index_operands))),
                    Stmt::BoundStep { value, when, .. } => stack_e.extend([*value].into_iter().chain(*when)),
                    Stmt::Emit { fields, .. } | Stmt::Trace { fields, .. } => stack_e.extend(fields.iter().filter_map(|f| match f {
                        EmitField::Value { value, .. } => Some(*value),
                        EmitField::Text { .. } => None,
                    })),
                }
                continue;
            }
            let Some(e) = stack_e.pop() else { break };
            let Some(data) = ir.exprs.get(e.index()) else { continue };
            if !seen_e.insert(e) {
                continue;
            }
            out_e.push(e);
            match &data.kind {
                Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::Probe { .. } | Expr::Env(_) => {}
                Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::ParamPrevious(_) | Expr::Others(_) | Expr::Resolve { .. } => {}
                Expr::Unwrap { is, value, .. } => stack_e.extend([*is, *value]),
                Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => stack_e.push(*expr),
                Expr::Binary { lhs, rhs, .. } => stack_e.extend([*lhs, *rhs]),
                Expr::Same { a, b } => stack_e.extend([*a, *b]),
                Expr::LevelCast { expr, .. } => stack_e.push(*expr),
                Expr::If { cond, then, otherwise } => stack_e.extend([*cond, *then, *otherwise]),
                Expr::Block { stmts, value } => {
                    stack_s.extend(stmts);
                    stack_e.extend(value);
                }
                Expr::Call { args, .. } | Expr::Math { args, .. } => stack_e.extend(args),
                Expr::Method { receiver, args, .. } => stack_e.extend([*receiver].into_iter().chain(args.iter().copied())),
                Expr::Closure { body, .. } => stack_e.push(*body),
                Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => {
                    stack_s.extend(data.kind.statements());
                    stack_e.extend(data.kind.operands());
                }
                Expr::Limit { x, f, seed, .. } => stack_e.extend([*x, *f].into_iter().chain(*seed)),
                Expr::Stim { mag, phase, .. } => stack_e.extend([*mag].into_iter().chain(*phase)),
                Expr::SessionParameter { .. } => {}
            }
        }
        (out_e, out_s)
    }

    /// `reach`, followed into the body of every function an expression calls: what a value
    /// computed from `exprs` evaluates.
    fn reach_calls(&self, exprs: &[ExprId]) -> BTreeSet<ExprId> {
        let ir = self.reader.ir;
        let mut out = BTreeSet::new();
        let mut called = BTreeSet::new();
        let mut roots = exprs.to_vec();
        while !roots.is_empty() {
            let (found, _) = self.reach(&[], &roots);
            roots.clear();
            for e in found {
                if let Some(Expr::Call { function, .. } | Expr::FunctionRef(function)) = ir.exprs.get(e.index()).map(|d| &d.kind)
                    && called.insert(*function)
                    && let Some(f) = ir.functions.get(function.index())
                {
                    roots.push(f.body);
                }
                out.insert(e);
            }
        }
        out
    }
}
