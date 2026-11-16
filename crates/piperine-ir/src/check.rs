//! `Ir::check`: the invariants a correct builder sequence guarantees, verified over the arenas.
//! The digital forms add their own: a body writes only the carriers it may (never an `in`
//! port), a carrier has a drive or commits, never both, `analog` reads a carrier only through
//! `known_or`, `is_known` and `high` (§3.3.5), a reaction waits on events of its body, a commit
//! sits in a reaction, a `cross` watches a real, and a carrier's initial value is a value of its
//! type (an integer within its `N`-bit range).
//! Every id is in range, instances agree with their bodies, paths are unique, `limit` and
//! `ac_stim` sites sit unconditionally at the top of the `analog` block and are used once, and
//! every expression has the type its position requires, every local is bound before it is read,
//! no function reaches itself through calls, and no expression reaches itself. It lists what is
//! broken and never panics.
//!
//! The code walk is iterative (an explicit stack, so nesting depth costs no native stack) and
//! visits each expression once per placement context and set of bound locals, so an id shared by
//! many parents costs one visit, not one per path. An expression met again while it is still
//! being walked is a cycle, whatever its depth.
//!
//! A local is bound by scope, not by evaluation order: a `let` binds its local from the next
//! statement to the end of its block (of the `analog` block at the top level), a closure binds
//! its parameters in its body only, and a function's parameters are bound in its whole body. A
//! `let` in one arm of an `if` sits in that arm's block, so the other arm and what follows the
//! `if` do not see it.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::ids::{ExprId, FunctionId, StmtId};
use crate::model::{
    BinaryOp, BodyData, BranchKind, CarrierRole, Constant, Env, EventSource, Expr, Ir, Origin, QuantityRole, Stmt, Terminal, UnaryOp,
    ValueType,
};
use crate::typing::Typing;

/// One broken invariant: the item, where it was written, and what is wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Broken {
    pub item: String,
    pub origin: Origin,
    pub what: String,
}

impl Ir {
    /// The broken invariants of this IR; empty when it is well formed.
    pub fn check(&self) -> Vec<Broken> {
        let mut checker = Checker { ir: self, out: Vec::new(), cycles: BTreeSet::new() };
        checker.ids();
        checker.instances();
        checker.paths();
        checker.types();
        checker.code();
        checker.discrete();
        checker.out
    }
}

/// What names and bounds the code being walked.
struct Owner {
    item: String,
    origin: Origin,
    locals: Vec<ValueType>,
    params: Vec<ValueType>,
    nodes: usize,
    branches: Vec<BranchKind>,
    limits: usize,
    stims: usize,
    /// The carried types of the body's carriers.
    carriers: Vec<ValueType>,
    /// A body (probes and params are readable) rather than a function.
    in_body: bool,
    /// The `analog` block of a body: sites and contributions may appear.
    analog: bool,
}

/// The state of one walk: one owner's code, every root of it sharing the memo.
struct Walk {
    owner: Owner,
    /// The distinct expressions that use each `limit` / `ac_stim` site.
    limit_users: Vec<BTreeSet<ExprId>>,
    stim_users: Vec<BTreeSet<ExprId>>,
    /// Locals bound in the scope being walked (a `let` after its init until its block ends,
    /// closure parameters in the closure's body, function parameters throughout).
    bound: Vec<bool>,
    /// The id of the current `bound` set, interned: part of a visit's key.
    scope: u32,
    scopes: HashMap<Vec<bool>, u32>,
    /// Local reads already reported as unbound.
    unbound: HashSet<ExprId>,
    /// Expressions whose own checks ran.
    seen: HashSet<ExprId>,
    /// Expression and statement visits already made, per placement and scope.
    done: HashSet<(ExprId, Ctx, u32)>,
    stmts_done: HashSet<(StmtId, Ctx, bool, u32)>,
    /// Expressions being walked: meeting one again is a cycle.
    on_stack: HashSet<ExprId>,
    /// The functions this owner's code calls.
    calls: BTreeSet<FunctionId>,
}

/// Where in the code an expression sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Ctx {
    conditional: bool,
    closure: bool,
}

/// One step of the iterative walk.
enum Task {
    Expr(ExprId, Ctx),
    /// The walk of an expression's operands is over: it leaves the stack.
    Exit(ExprId),
    /// A statement, and whether it sits at the top of an `analog` block.
    Stmt(StmtId, Ctx, bool),
    /// A `let` has evaluated its init: its local is bound.
    Bind(u32),
    /// A block or a closure ends: the locals it bound go out of scope.
    Restore(Vec<bool>),
}

struct Checker<'a> {
    ir: &'a Ir,
    out: Vec<Broken>,
    /// Expressions already reported as reaching themselves.
    cycles: BTreeSet<ExprId>,
}

impl<'a> Checker<'a> {
    fn report(&mut self, item: String, origin: Origin, what: String) {
        self.out.push(Broken { item, origin, what });
    }

    fn unknown() -> Origin {
        Origin { file: "<ir>".to_string(), line: 0, col: 0 }
    }

    /// Whether `value` is a value of the scalar type `ty`: a `bool` for `bool`, a real for `f64`,
    /// an integer within the `N`-bit range for `UInt<N>`/`Int<N>`.
    fn holds(ty: &ValueType, value: Constant) -> bool {
        match (ty, value) {
            (ValueType::Bool, Constant::Bool(_)) | (ValueType::F64, Constant::Real(_)) => true,
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Int(v)) => {
                ty.integer_range().is_some_and(|(lo, hi)| (lo..=hi).contains(&v))
            }
            _ => false,
        }
    }

    fn expr_origin(&self, id: ExprId) -> Origin {
        self.ir.exprs.get(id.index()).map_or_else(Checker::unknown, |e| e.origin.clone())
    }

    fn stmt_origin(&self, stmt: &Stmt) -> Origin {
        let first = match stmt {
            Stmt::Let { init, .. } => Some(*init),
            Stmt::Expr(e) => Some(*e),
            Stmt::Contribute { addends, .. } => addends.first().map(|a| a.value),
            Stmt::Commit { value, .. } => Some(*value),
        };
        first.map_or_else(Checker::unknown, |e| self.expr_origin(e))
    }

    // ---- ids in range -------------------------------------------------------------------

    fn expr_id(&mut self, item: &str, origin: &Origin, id: ExprId) {
        if id.index() >= self.ir.exprs.len() {
            self.report(item.to_string(), origin.clone(), format!("expr id {} out of range ({} exprs)", id.0, self.ir.exprs.len()));
        }
    }

    fn stmt_id(&mut self, item: &str, origin: &Origin, id: StmtId) {
        if id.index() >= self.ir.stmts.len() {
            self.report(item.to_string(), origin.clone(), format!("stmt id {} out of range ({} stmts)", id.0, self.ir.stmts.len()));
        }
    }

    fn ids(&mut self) {
        let ir = self.ir;
        if ir.top.index() >= ir.instances.len() {
            self.report("top".into(), Checker::unknown(), format!("top instance id {} out of range ({} instances)", ir.top.0, ir.instances.len()));
        }
        for body in &ir.bodies {
            let item = format!("body {}", body.name);
            let origin = body.origin.clone();
            for param in &body.params {
                if let Some(check) = param.check {
                    self.expr_id(&item, &param.origin, check);
                }
            }
            for node in &body.nodes {
                if node.nature.index() >= ir.natures.len() {
                    self.report(item.clone(), node.origin.clone(), format!("node {}: nature id {} out of range", node.name, node.nature.0));
                }
            }
            for branch in &body.branches {
                for t in [branch.plus, branch.minus] {
                    if let Terminal::Node(i) = t {
                        if i as usize >= body.nodes.len() {
                            self.report(item.clone(), origin.clone(), format!("branch terminal node {i} out of range ({} nodes)", body.nodes.len()));
                        }
                    }
                }
            }
            for &stmt in body.analog.iter().flatten() {
                self.stmt_id(&item, &origin, stmt);
            }
            for drive in &body.drives {
                if drive.carrier as usize >= body.carriers.len() {
                    self.report(item.clone(), drive.origin.clone(), format!("drive of carrier {}, out of range ({} carriers)", drive.carrier, body.carriers.len()));
                }
                self.expr_id(&item, &drive.origin, drive.value);
                if let Some(after) = drive.after {
                    self.expr_id(&item, &drive.origin, after);
                }
            }
            for event in &body.events {
                if let EventSource::Cross(e) | EventSource::At(e) = event.source {
                    self.expr_id(&item, &event.origin, e);
                }
                if let Some(guard) = event.guard {
                    self.expr_id(&item, &event.origin, guard);
                }
            }
            for reaction in &body.reactions {
                for &stmt in &reaction.stmts {
                    self.stmt_id(&item, &reaction.origin, stmt);
                }
            }
            for child in &body.children {
                match ir.bodies.get(child.body.index()) {
                    None => self.report(item.clone(), child.origin.clone(), format!("child {}: body id {} out of range", child.name, child.body.0)),
                    Some(callee) => {
                        if child.nodes.len() != callee.nodes.len() {
                            self.report(item.clone(), child.origin.clone(), format!("child {} binds {} nodes, body {} has {}", child.name, child.nodes.len(), callee.name, callee.nodes.len()));
                        }
                        for (i, _) in &child.params {
                            if *i as usize >= callee.params.len() {
                                self.report(item.clone(), child.origin.clone(), format!("child {}: param index {i} out of range", child.name));
                            }
                        }
                    }
                }
                for t in &child.nodes {
                    if let Terminal::Node(i) = t {
                        if *i as usize >= body.nodes.len() {
                            self.report(item.clone(), child.origin.clone(), format!("child {}: terminal node {i} out of range", child.name));
                        }
                    }
                }
            }
        }
        for function in &ir.functions {
            let item = format!("function {}", function.name);
            self.expr_id(&item, &function.origin, function.body);
        }
        for (i, data) in ir.exprs.iter().enumerate() {
            let item = format!("expr {i}");
            let origin = data.origin.clone();
            self.expr_children(&item, &origin, &data.kind);
        }
        for (i, stmt) in ir.stmts.iter().enumerate() {
            let item = format!("stmt {i}");
            let origin = self.stmt_origin(stmt);
            match stmt {
                Stmt::Let { init, .. } => self.expr_id(&item, &origin, *init),
                Stmt::Expr(e) => self.expr_id(&item, &origin, *e),
                Stmt::Contribute { addends, .. } => {
                    for a in addends {
                        self.expr_id(&item, &origin, a.value);
                    }
                }
                Stmt::Commit { value, after, .. } => {
                    self.expr_id(&item, &origin, *value);
                    if let Some(after) = after {
                        self.expr_id(&item, &origin, *after);
                    }
                }
            }
        }
        for instance in &ir.instances {
            let item = format!("instance {}", instance.path);
            let origin = instance.origin.clone();
            if instance.body.index() >= ir.bodies.len() {
                self.report(item.clone(), origin.clone(), format!("body id {} out of range", instance.body.0));
            }
            for id in instance.parent.iter().chain(&instance.children) {
                if id.index() >= ir.instances.len() {
                    self.report(item.clone(), origin.clone(), format!("instance id {} out of range", id.0));
                }
            }
            for n in &instance.nodes {
                if let crate::model::NodeRef::Node(id) = n {
                    if id.index() >= ir.nodes.len() {
                        self.report(item.clone(), origin.clone(), format!("node id {} out of range", id.0));
                    }
                }
            }
            for net in &instance.nets {
                if net.index() >= ir.nets.len() {
                    self.report(item.clone(), origin.clone(), format!("net id {} out of range", net.0));
                }
            }
        }
        for net in &ir.nets {
            if net.declared_by.index() >= ir.instances.len() {
                self.report(format!("net {}", net.path), net.origin.clone(), format!("declaring instance id {} out of range", net.declared_by.0));
            }
        }
        for node in &ir.nodes {
            let item = format!("node {}", node.path);
            if node.nature.index() >= ir.natures.len() {
                self.report(item.clone(), node.origin.clone(), format!("nature id {} out of range", node.nature.0));
            }
            if node.declared_by.index() >= ir.instances.len() {
                self.report(item, node.origin.clone(), format!("declaring instance id {} out of range", node.declared_by.0));
            }
        }
    }

    fn expr_children(&mut self, item: &str, origin: &Origin, kind: &Expr) {
        let ir = self.ir;
        let mut exprs: Vec<ExprId> = Vec::new();
        match kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) => {}
            Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) | Expr::Unknown | Expr::Carrier(_) => {}
            Expr::KnownOr { value, default } => exprs.extend([*value, *default]),
            Expr::IsKnown(value) | Expr::High(value) => exprs.push(*value),
            Expr::Cast { expr, .. } => exprs.push(*expr),
            Expr::Method { receiver, args, .. } => {
                exprs.push(*receiver);
                exprs.extend(args);
            }
            Expr::Unary { expr, .. } => exprs.push(*expr),
            Expr::Binary { lhs, rhs, .. } => exprs.extend([*lhs, *rhs]),
            Expr::If { cond, then, otherwise } => exprs.extend([*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => {
                for &s in stmts {
                    self.stmt_id(item, origin, s);
                }
                exprs.extend(value);
            }
            Expr::Call { function, args } => {
                if function.index() >= ir.functions.len() {
                    self.report(item.to_string(), origin.clone(), format!("function id {} out of range ({} functions)", function.0, ir.functions.len()));
                }
                exprs.extend(args);
            }
            Expr::Math { args, .. } => exprs.extend(args),
            Expr::Closure { body, .. } => exprs.push(*body),
            Expr::Limit { x, f, seed, .. } => {
                exprs.extend([*x, *f]);
                exprs.extend(seed);
            }
            Expr::Stim { mag, phase, .. } => {
                exprs.push(*mag);
                exprs.extend(phase);
            }
        }
        for e in exprs {
            self.expr_id(item, origin, e);
        }
    }

    // ---- instances and paths -------------------------------------------------------------

    fn instances(&mut self) {
        let ir = self.ir;
        for (i, instance) in ir.instances.iter().enumerate() {
            let Some(body) = ir.bodies.get(instance.body.index()) else { continue };
            let item = format!("instance {}", instance.path);
            if instance.nodes.len() != body.nodes.len() {
                self.report(item.clone(), instance.origin.clone(), format!("binds {} nodes, body {} has {}", instance.nodes.len(), body.name, body.nodes.len()));
            }
            if instance.params.len() != body.params.len() {
                self.report(item.clone(), instance.origin.clone(), format!("binds {} params, body {} has {}", instance.params.len(), body.name, body.params.len()));
            }
            if instance.nets.len() != body.carriers.len() {
                self.report(item.clone(), instance.origin.clone(), format!("binds {} nets, body {} has {} carriers", instance.nets.len(), body.name, body.carriers.len()));
            }
            for (carrier, net) in body.carriers.iter().zip(&instance.nets) {
                if let Some(net) = ir.nets.get(net.index()) {
                    if net.ty != carrier.ty {
                        self.report(item.clone(), instance.origin.clone(), format!("binds carrier {}: {} to net {}: {}", carrier.name, carrier.ty, net.path, net.ty));
                    }
                }
            }
            match instance.parent.and_then(|p| ir.instances.get(p.index())) {
                Some(parent) => {
                    if !parent.children.iter().any(|c| c.index() == i) {
                        self.report(item.clone(), instance.origin.clone(), format!("parent {} does not list it as a child", parent.path));
                    }
                    if instance.parent.is_some_and(|p| p.index() >= i) {
                        self.report(item.clone(), instance.origin.clone(), "stored before its parent (not pre-order)".to_string());
                    }
                }
                None => {
                    if instance.parent.is_none() && ir.top.index() != i {
                        self.report(item.clone(), instance.origin.clone(), "has no parent and is not the top".to_string());
                    }
                }
            }
            for child in &instance.children {
                if let Some(c) = ir.instances.get(child.index()) {
                    if c.parent.map(|p| p.index()) != Some(i) {
                        self.report(item.clone(), instance.origin.clone(), format!("child {} does not name it as its parent", c.path));
                    }
                }
            }
        }
    }

    fn paths(&mut self) {
        let ir = self.ir;
        let mut instances: BTreeMap<&str, (u32, &Origin)> = BTreeMap::new();
        for i in &ir.instances {
            instances.entry(i.path.as_str()).or_insert((0, &i.origin)).0 += 1;
        }
        let mut nodes: BTreeMap<&str, (u32, &Origin)> = BTreeMap::new();
        for n in &ir.nodes {
            nodes.entry(n.path.as_str()).or_insert((0, &n.origin)).0 += 1;
        }
        let mut nets: BTreeMap<&str, (u32, &Origin)> = BTreeMap::new();
        for n in &ir.nets {
            nets.entry(n.path.as_str()).or_insert((0, &n.origin)).0 += 1;
        }
        let mut found = Vec::new();
        for (kind, table) in [("instance", instances), ("node", nodes), ("net", nets)] {
            for (path, (count, origin)) in table {
                if count > 1 {
                    found.push((format!("{kind} {path}"), origin.clone(), format!("{count} {kind}s share the path {path}")));
                }
            }
        }
        for (item, origin, what) in found {
            self.report(item, origin, what);
        }
    }

    // ---- types -----------------------------------------------------------------------------

    /// Every type written in the IR is well formed: an integer width in 1..=64 (RS-Q6: the ABI's
    /// leaves end at 64 bits).
    fn types(&mut self) {
        let ir = self.ir;
        let mut found: Vec<(String, Origin, ValueType)> = Vec::new();
        for body in &ir.bodies {
            let item = format!("body {}", body.name);
            for p in &body.params {
                found.push((format!("{item} param {}", p.name), p.origin.clone(), p.ty.clone()));
            }
            for l in &body.locals {
                found.push((format!("{item} local {}", l.name), body.origin.clone(), l.ty.clone()));
            }
            for r in &body.reactions {
                for l in &r.locals {
                    found.push((format!("{item} reaction local {}", l.name), r.origin.clone(), l.ty.clone()));
                }
            }
            for c in &body.carriers {
                if !Typing::scalar(&c.ty) {
                    self.report(format!("{item} carrier {}", c.name), c.origin.clone(), format!("a carrier holds `bool`, `f64` or an integer type, found {}", c.ty));
                } else if let Some(init) = c.init
                    && !Self::holds(&c.ty, init)
                {
                    self.report(
                        format!("{item} carrier {}", c.name),
                        c.origin.clone(),
                        format!("the initial value {init} of carrier {} is not a value of its type {}", c.name, c.ty),
                    );
                }
                found.push((format!("{item} carrier {}", c.name), c.origin.clone(), c.ty.clone()));
            }
        }
        for net in &ir.nets {
            found.push((format!("net {}", net.path), net.origin.clone(), net.ty.clone()));
        }
        for function in &ir.functions {
            let item = format!("function {}", function.name);
            for l in &function.locals {
                found.push((format!("{item} local {}", l.name), function.origin.clone(), l.ty.clone()));
            }
            found.push((format!("{item} result"), function.origin.clone(), function.ret.clone()));
        }
        for (i, data) in ir.exprs.iter().enumerate() {
            found.push((format!("expr {i}"), data.origin.clone(), data.ty.clone()));
            if let Expr::Cast { to, .. } = &data.kind {
                found.push((format!("expr {i}"), data.origin.clone(), to.clone()));
            }
        }
        for (item, origin, ty) in found {
            if let Some(what) = Checker::malformed(&ty) {
                self.report(item, origin, what);
            }
        }
    }

    /// What is wrong with `ty`, read through function types.
    fn malformed(ty: &ValueType) -> Option<String> {
        let mut stack = vec![ty];
        while let Some(t) = stack.pop() {
            match t {
                ValueType::UInt(n) | ValueType::Int(n) if !(1..=64).contains(n) => {
                    return Some(format!("type {t} has a width outside 1..=64"));
                }
                ValueType::Flagged(inner) if !Typing::scalar(inner) => {
                    return Some(format!("type {t}: only `bool`, `f64` and an integer type may be unknown"));
                }
                ValueType::Flagged(inner) => stack.push(inner),
                ValueType::Fn(params, ret) => {
                    stack.extend(params);
                    stack.push(ret);
                }
                _ => {}
            }
        }
        None
    }

    // ---- the digital rules -----------------------------------------------------------------

    /// The expressions and statements reachable from `stmts` and `exprs`, through operands and
    /// blocks, each once. Ids out of range are skipped (`ids` names them).
    fn reach(&self, stmts: &[StmtId], exprs: &[ExprId]) -> (Vec<ExprId>, Vec<StmtId>) {
        let ir = self.ir;
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
                    Stmt::Contribute { addends, .. } => stack_e.extend(addends.iter().map(|a| a.value)),
                    Stmt::Commit { value, after, .. } => stack_e.extend([*value].into_iter().chain(*after)),
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
                Expr::Int(_) | Expr::Unknown | Expr::Carrier(_) => {}
                Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => stack_e.push(*expr),
                Expr::Binary { lhs, rhs, .. } => stack_e.extend([*lhs, *rhs]),
                Expr::KnownOr { value, default } => stack_e.extend([*value, *default]),
                Expr::If { cond, then, otherwise } => stack_e.extend([*cond, *then, *otherwise]),
                Expr::Block { stmts, value } => {
                    stack_s.extend(stmts);
                    stack_e.extend(value);
                }
                Expr::Call { args, .. } | Expr::Math { args, .. } => stack_e.extend(args),
                Expr::Method { receiver, args, .. } => stack_e.extend([*receiver].into_iter().chain(args.iter().copied())),
                Expr::Closure { body, .. } => stack_e.push(*body),
                Expr::Limit { x, f, seed, .. } => stack_e.extend([*x, *f].into_iter().chain(*seed)),
                Expr::Stim { mag, phase, .. } => stack_e.extend([*mag].into_iter().chain(*phase)),
            }
        }
        (out_e, out_s)
    }

    /// `reach`, followed into the body of every function an expression calls: what a value
    /// computed from `exprs` evaluates.
    fn reach_calls(&self, exprs: &[ExprId]) -> BTreeSet<ExprId> {
        let ir = self.ir;
        let mut out = BTreeSet::new();
        let mut called = BTreeSet::new();
        let mut roots = exprs.to_vec();
        while !roots.is_empty() {
            let (found, _) = self.reach(&[], &roots);
            roots.clear();
            for e in found {
                if let Some(Expr::Call { function, .. }) = ir.exprs.get(e.index()).map(|d| &d.kind)
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
                EventSource::Changed(_) | EventSource::Cross(_) => None,
            };
            for e in event.guard.into_iter().chain(at) {
                positions.push((format!("{item} event {k}"), event.origin.clone(), e));
            }
        }
        for (position, origin, root) in positions {
            for e in self.reach_calls(&[root]) {
                if let Some(Expr::Env(env @ (Env::Time | Env::Static))) = self.ir.exprs.get(e.index()).map(|d| &d.kind) {
                    let what = if *env == Env::Time { "time()" } else { "static()" };
                    let at = self.ir.exprs.get(e.index()).map_or_else(|| origin.clone(), |d| d.origin.clone());
                    self.report(position.clone(), at, format!("reads {what}: a drive, an event guard and an `at` argument read no input (abi.md §4.2)"));
                }
            }
        }
    }

    /// The commits among `stmts`, with the carrier each writes.
    fn commits(&self, stmts: &[StmtId]) -> Vec<(StmtId, u32)> {
        stmts
            .iter()
            .filter_map(|s| match self.ir.stmts.get(s.index()) {
                Some(Stmt::Commit { carrier, .. }) => Some((*s, *carrier)),
                _ => None,
            })
            .collect()
    }

    fn discrete(&mut self) {
        let ir = self.ir;
        for body in &ir.bodies {
            self.writes(body);
            self.analog_reads(body);
            self.ambient_reads(body);
            let item = format!("body {}", body.name);
            for (k, reaction) in body.reactions.iter().enumerate() {
                for &e in &reaction.on {
                    if e as usize >= body.events.len() {
                        self.report(format!("{item} reaction {k}"), reaction.origin.clone(), format!("waits on event {e}, out of range ({} events)", body.events.len()));
                    }
                }
            }
            for (k, event) in body.events.iter().enumerate() {
                if let EventSource::Cross(e) = event.source {
                    if let Some(ty) = self.ty(e).filter(|t| *Typing::base(t) != ValueType::F64) {
                        self.report(format!("{item} event {k}"), event.origin.clone(), format!("a `cross` watches a real, found {ty}"));
                    }
                }
                if let EventSource::At(e) = event.source {
                    if let Some(ty) = self.ty(e).filter(|t| *Typing::base(t) != ValueType::F64) {
                        self.report(format!("{item} event {k}"), event.origin.clone(), format!("an `at` waits for a time, an `f64`, found {ty}"));
                    }
                    for read in self.reach_calls(&[e]) {
                        if matches!(self.ir.exprs.get(read.index()).map(|d| &d.kind), Some(Expr::Probe { .. })) {
                            let at = self.expr_origin(read);
                            self.report(format!("{item} event {k}"), at, "an `at` argument reads a probe: a discrete function reads no input (abi.md §4.2)".to_string());
                        }
                    }
                }
            }
            // A commit anywhere but in a reaction: the analog block, a `where`, a drive, an event.
            let mut roots: Vec<ExprId> = body.params.iter().filter_map(|p| p.check).collect();
            roots.extend(body.drives.iter().flat_map(|d| [d.value].into_iter().chain(d.after)));
            roots.extend(body.events.iter().flat_map(|e| {
                let cross = match e.source {
                    EventSource::Cross(x) | EventSource::At(x) => Some(x),
                    EventSource::Changed(_) => None,
                };
                cross.into_iter().chain(e.guard)
            }));
            let (_, stmts) = self.reach(body.analog.as_deref().unwrap_or(&[]), &roots);
            for (s, _) in self.commits(&stmts) {
                let origin = self.stmt_origin(&ir.stmts[s.index()]);
                self.report(item.clone(), origin, "a commit outside a reaction".to_string());
            }
        }
        for function in &ir.functions {
            let (_, stmts) = self.reach(&[], &[function.body]);
            for (s, _) in self.commits(&stmts) {
                let origin = self.stmt_origin(&ir.stmts[s.index()]);
                self.report(format!("function {}", function.name), origin, "a commit outside a reaction".to_string());
            }
        }
    }

    /// Who writes each carrier: an `in` port is written by its driver outside, never by the body;
    /// any other carrier has drives or commits, never both.
    fn writes(&mut self, body: &BodyData) {
        let item = format!("body {}", body.name);
        let input = |c: u32| body.carriers.get(c as usize).filter(|c| c.role == CarrierRole::In);
        for drive in &body.drives {
            if let Some(c) = input(drive.carrier) {
                self.report(format!("{item} drive {}", c.name), drive.origin.clone(), format!("drives the `in` port {}: an input is written outside the body", c.name));
            }
        }
        let mut committed: BTreeSet<u32> = BTreeSet::new();
        for (k, reaction) in body.reactions.iter().enumerate() {
            let (_, stmts) = self.reach(&reaction.stmts, &[]);
            for (s, carrier) in self.commits(&stmts) {
                committed.insert(carrier);
                if let Some(c) = input(carrier) {
                    let origin = self.stmt_origin(&self.ir.stmts[s.index()]);
                    self.report(format!("{item} reaction {k}"), origin, format!("commits to the `in` port {}: an input is written outside the body", c.name));
                }
            }
        }
        let driven: BTreeSet<u32> = body.drives.iter().map(|d| d.carrier).collect();
        for c in driven.intersection(&committed) {
            if let Some(carrier) = body.carriers.get(*c as usize) {
                self.report(format!("{item} carrier {}", carrier.name), carrier.origin.clone(), format!("carrier {} has both a drive and a commit", carrier.name));
            }
        }
    }

    /// §3.3.5: `analog` reads a carrier only as the receiver of `known_or`, `is_known` or `high`.
    fn analog_reads(&mut self, body: &BodyData) {
        let Some(analog) = &body.analog else { return };
        let (exprs, _) = self.reach(analog, &[]);
        let ir = self.ir;
        let mut allowed: HashSet<ExprId> = HashSet::new();
        for e in &exprs {
            if let Expr::KnownOr { value: read, .. } | Expr::IsKnown(read) | Expr::High(read) = &ir.exprs[e.index()].kind {
                if matches!(ir.exprs.get(read.index()).map(|d| &d.kind), Some(Expr::Carrier(_))) {
                    allowed.insert(*read);
                }
            }
        }
        for e in exprs {
            if let Expr::Carrier(c) = ir.exprs[e.index()].kind {
                if !allowed.contains(&e) {
                    let name = body.carriers.get(c as usize).map_or_else(|| format!("{c}"), |c| c.name.clone());
                    let origin = self.expr_origin(e);
                    self.report(format!("body {}", body.name), origin, format!("`analog` reads carrier {name} outside `known_or`, `is_known` and `high` (§3.3.5)"));
                }
            }
        }
    }

    // ---- code ------------------------------------------------------------------------------

    fn code(&mut self) {
        let ir = self.ir;
        for body in &ir.bodies {
            let owner = |analog: bool, item: String| Owner {
                item,
                origin: body.origin.clone(),
                locals: body.locals.iter().map(|l| l.ty.clone()).collect(),
                params: body.params.iter().map(|p| p.ty.clone()).collect(),
                nodes: body.nodes.len(),
                branches: body.branches.iter().map(|b| b.kind).collect(),
                limits: body.limits.len(),
                stims: body.stims.len(),
                carriers: body.carriers.iter().map(|c| c.ty.clone()).collect(),
                in_body: true,
                analog,
            };
            for param in &body.params {
                let Some(check) = param.check else { continue };
                let mut w = Walk::new(owner(false, format!("body {} param {}", body.name, param.name)), 0);
                self.expect(&w, check, &ValueType::Bool);
                self.walk(&mut w, Task::Expr(check, Ctx::top()));
            }
            if let Some(analog) = &body.analog {
                let mut w = Walk::new(owner(true, format!("body {}", body.name)), 0);
                for &stmt in analog {
                    self.walk(&mut w, Task::Stmt(stmt, Ctx::top(), true));
                }
                self.site_uses(&w);
            }
            for drive in &body.drives {
                let name = body.carriers.get(drive.carrier as usize).map_or("<bad carrier>", |c| c.name.as_str());
                let mut w = Walk::new(owner(false, format!("body {} drive {name}", body.name)), 0);
                if let Some(c) = body.carriers.get(drive.carrier as usize) {
                    self.carried(&w, drive.value, &c.ty, "the value of a drive");
                }
                self.walk(&mut w, Task::Expr(drive.value, Ctx::top()));
                if let Some(after) = drive.after {
                    self.carried(&w, after, &ValueType::F64, "the delay of a drive");
                    self.walk(&mut w, Task::Expr(after, Ctx::top()));
                }
            }
            for (k, event) in body.events.iter().enumerate() {
                let mut w = Walk::new(owner(false, format!("body {} event {k}", body.name)), 0);
                if let EventSource::Cross(e) | EventSource::At(e) = event.source {
                    self.walk(&mut w, Task::Expr(e, Ctx::top()));
                }
                if let Some(guard) = event.guard {
                    self.carried(&w, guard, &ValueType::Bool, "the guard of an event");
                    self.walk(&mut w, Task::Expr(guard, Ctx::top()));
                }
            }
            for (k, reaction) in body.reactions.iter().enumerate() {
                let item = format!("body {} reaction {k}", body.name);
                let mut owner = owner(false, item);
                owner.locals = reaction.locals.iter().map(|l| l.ty.clone()).collect();
                let mut w = Walk::new(owner, 0);
                for &stmt in &reaction.stmts {
                    self.walk(&mut w, Task::Stmt(stmt, Ctx::top(), false));
                }
            }
        }
        let mut graph: Vec<BTreeSet<FunctionId>> = Vec::new();
        for function in &ir.functions {
            let owner = Owner {
                item: format!("function {}", function.name),
                origin: function.origin.clone(),
                locals: function.locals.iter().map(|l| l.ty.clone()).collect(),
                params: Vec::new(),
                nodes: 0,
                branches: Vec::new(),
                limits: 0,
                stims: 0,
                carriers: Vec::new(),
                in_body: false,
                analog: false,
            };
            let mut w = Walk::new(owner, function.params.len());
            self.expect(&w, function.body, &function.ret);
            self.walk(&mut w, Task::Expr(function.body, Ctx::top()));
            graph.push(w.calls);
        }
        self.recursion(&graph);
    }

    /// Every function that reaches itself through calls.
    fn recursion(&mut self, graph: &[BTreeSet<FunctionId>]) {
        for (i, function) in self.ir.functions.iter().enumerate() {
            let mut seen: BTreeSet<usize> = BTreeSet::new();
            let mut stack: Vec<usize> = graph[i].iter().map(|f| f.index()).collect();
            let mut recursive = false;
            while let Some(f) = stack.pop() {
                if f == i {
                    recursive = true;
                    break;
                }
                if f < graph.len() && seen.insert(f) {
                    stack.extend(graph[f].iter().map(|g| g.index()));
                }
            }
            if recursive {
                self.report(format!("function {}", function.name), function.origin.clone(), format!("function {} calls itself, directly or through other functions", function.name));
            }
        }
    }

    fn site_uses(&mut self, w: &Walk) {
        for (kind, users) in [("limit", &w.limit_users), ("ac_stim", &w.stim_users)] {
            for (i, set) in users.iter().enumerate() {
                if set.len() > 1 {
                    self.report(w.owner.item.clone(), w.owner.origin.clone(), format!("{kind} site {i} is used by {} expressions, once is allowed", set.len()));
                }
            }
        }
    }

    /// Runs the walk from `root` to its end.
    fn walk(&mut self, w: &mut Walk, root: Task) {
        let mut stack = vec![root];
        let mut next = Vec::new();
        while let Some(task) = stack.pop() {
            match task {
                Task::Exit(id) => {
                    w.on_stack.remove(&id);
                }
                Task::Bind(local) => {
                    if let Some(slot) = w.bound.get_mut(local as usize) {
                        *slot = true;
                    }
                    w.rescope();
                }
                Task::Restore(bound) => {
                    w.bound = bound;
                    w.rescope();
                }
                Task::Stmt(id, ctx, top) => self.stmt(w, id, ctx, top, &mut next),
                Task::Expr(id, ctx) => self.expr(w, id, ctx, &mut next),
            }
            // `next` holds the follow-up tasks in evaluation order.
            stack.extend(next.drain(..).rev());
        }
    }

    fn stmt(&mut self, w: &mut Walk, id: StmtId, ctx: Ctx, top: bool, next: &mut Vec<Task>) {
        let Some(stmt) = self.ir.stmts.get(id.index()) else { return };
        if !w.stmts_done.insert((id, ctx, top, w.scope)) {
            return;
        }
        let origin = self.stmt_origin(stmt);
        match stmt {
            Stmt::Let { local, init } => {
                match w.owner.locals.get(*local as usize).cloned() {
                    Some(expected) => self.expect(w, *init, &expected),
                    None => self.report(w.owner.item.clone(), origin, format!("let binds local {local}, out of range ({} locals)", w.owner.locals.len())),
                }
                next.push(Task::Expr(*init, ctx));
                next.push(Task::Bind(*local));
            }
            Stmt::Expr(e) => next.push(Task::Expr(*e, ctx)),
            Stmt::Contribute { branch, quantity, addends } => {
                if !top || !w.owner.analog {
                    self.report(w.owner.item.clone(), origin.clone(), "contribution outside the top level of an analog block".to_string());
                }
                let kind = w.owner.branches.get(*branch as usize).copied();
                match kind {
                    None if w.owner.analog => {
                        self.report(w.owner.item.clone(), origin.clone(), format!("contribution to branch {branch}, out of range"));
                    }
                    Some(kind) => {
                        let flow = *quantity == QuantityRole::Flow;
                        if flow != (kind == BranchKind::Flow) {
                            self.report(w.owner.item.clone(), origin.clone(), format!("contribution quantity does not match the kind of branch {branch}"));
                        }
                    }
                    None => {}
                }
                for a in addends {
                    self.expect(w, a.value, &ValueType::F64);
                    next.push(Task::Expr(a.value, ctx));
                }
            }
            Stmt::Commit { carrier, value, after } => {
                match w.owner.carriers.get(*carrier as usize).cloned() {
                    Some(ty) => self.carried(w, *value, &ty, "the value of a commit"),
                    None => self.report(w.owner.item.clone(), origin.clone(), format!("commit to carrier {carrier}, out of range ({} carriers)", w.owner.carriers.len())),
                }
                next.push(Task::Expr(*value, ctx));
                if let Some(after) = after {
                    self.carried(w, *after, &ValueType::F64, "the delay of a commit");
                    next.push(Task::Expr(*after, ctx));
                }
            }
        }
    }

    fn wrong(&mut self, w: &Walk, origin: Origin, what: String) {
        self.report(w.owner.item.clone(), origin, what);
    }

    /// The recorded type of `id`, when it exists.
    fn ty(&self, id: ExprId) -> Option<&'a ValueType> {
        self.ir.exprs.get(id.index()).map(|e| &e.ty)
    }

    /// `id`, placed where `want` is required (a `let`, an addend, a `where`, a function body),
    /// must have that type.
    fn expect(&mut self, w: &Walk, id: ExprId, want: &ValueType) {
        if let Some(got) = self.ty(id) {
            if got != want {
                let origin = self.expr_origin(id);
                self.wrong(w, origin, format!("expression must be {want}, found {got}"));
            }
        }
    }

    /// `id`, a value written to a carrier of type `want` (or a delay, `want` being `f64`), may be
    /// known or unknown: its type is `want` or `want` flagged.
    fn carried(&mut self, w: &Walk, id: ExprId, want: &ValueType, position: &str) {
        if let Some(got) = self.ty(id) {
            if Typing::base(got) != want {
                let origin = self.expr_origin(id);
                self.wrong(w, origin, format!("{position} must be {want}, found {got}"));
            }
        }
    }

    /// `id` must have type `want`; an entry at its origin otherwise.
    fn want(&mut self, w: &Walk, id: ExprId, want: &ValueType, position: &str) {
        if let Some(got) = self.ty(id) {
            if got != want {
                let origin = self.expr_origin(id);
                self.wrong(w, origin, format!("{position} must be {want}, found {got}"));
            }
        }
    }

    fn expr(&mut self, w: &mut Walk, id: ExprId, ctx: Ctx, next: &mut Vec<Task>) {
        let Some(data) = self.ir.exprs.get(id.index()) else { return };
        let origin = data.origin.clone();
        if w.on_stack.contains(&id) {
            if self.cycles.insert(id) {
                self.wrong(w, origin, format!("expr {} reaches itself through its operands: a cycle", id.0));
            }
            return;
        }
        if !w.done.insert((id, ctx, w.scope)) {
            return;
        }
        w.on_stack.insert(id);
        // The checks of the expression itself run once; its placement (sites) once per context.
        let first = w.seen.insert(id);
        let ty = &data.ty;
        match &data.kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit if !first => {}
            Expr::Real(_) => self.own(w, &origin, ty, &ValueType::F64),
            Expr::Bool(_) => self.own(w, &origin, ty, &ValueType::Bool),
            Expr::Unit => self.own(w, &origin, ty, &ValueType::Unit),
            Expr::Local(i) => match w.owner.locals.get(*i as usize) {
                Some(local) => {
                    if first {
                        self.own(w, &origin, ty, &local.clone());
                    }
                    // Checked in every scope the read is visited in; reported once.
                    if !w.bound.get(*i as usize).copied().unwrap_or(false) && w.unbound.insert(id) {
                        self.wrong(w, origin.clone(), format!("local {i} read before its `let`"));
                    }
                }
                None if first => self.wrong(w, origin, format!("local {i} out of range ({} locals)", w.owner.locals.len())),
                None => {}
            },
            Expr::Param(_) if !first => {}
            Expr::Param(i) => {
                if !w.owner.in_body {
                    self.wrong(w, origin, "param read outside a body".to_string());
                } else {
                    match w.owner.params.get(*i as usize) {
                        Some(p) => self.own(w, &origin, ty, &p.clone()),
                        None => self.wrong(w, origin, format!("param {i} out of range ({} params)", w.owner.params.len())),
                    }
                }
            }
            Expr::Unary { op, expr } if self.ty(*expr).is_some_and(Typing::digital) => {
                if first {
                    self.typed(w, &origin, ty, self.ty(*expr).and_then(|t| Typing::unary(*op, t)), "a unary operator");
                }
                next.push(Task::Expr(*expr, ctx));
            }
            Expr::Unary { op, expr } => {
                if first {
                    let operand = if *op == UnaryOp::Neg { ValueType::F64 } else { ValueType::Bool };
                    self.own(w, &origin, ty, &operand);
                    self.want(w, *expr, &operand, "operand of a unary operator");
                }
                next.push(Task::Expr(*expr, ctx));
            }
            Expr::Binary { op, lhs, rhs }
                if Typing::digital_op(*op) || [*lhs, *rhs].iter().any(|e| self.ty(*e).is_some_and(Typing::digital)) =>
            {
                if first {
                    let want = match (self.ty(*lhs), self.ty(*rhs)) {
                        (Some(l), Some(r)) => Typing::binary(*op, l, r),
                        _ => None,
                    };
                    self.typed(w, &origin, ty, want, "a binary operator");
                }
                next.push(Task::Expr(*lhs, ctx));
                let short = matches!(op, BinaryOp::And | BinaryOp::Or);
                next.push(Task::Expr(*rhs, Ctx { conditional: ctx.conditional || short, ..ctx }));
            }
            Expr::Binary { op, lhs, rhs } => {
                if first {
                    let (operands, result) = match op {
                        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => (Some(ValueType::F64), ValueType::F64),
                        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => (Some(ValueType::F64), ValueType::Bool),
                        BinaryOp::And | BinaryOp::Or => (Some(ValueType::Bool), ValueType::Bool),
                        BinaryOp::Eq | BinaryOp::Ne => (None, ValueType::Bool),
                        // Taken by the digital arm above.
                        BinaryOp::Rem | BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Shl | BinaryOp::Shr => {
                            (None, ValueType::Bool)
                        }
                    };
                    self.own(w, &origin, ty, &result);
                    match operands {
                        Some(t) => {
                            self.want(w, *lhs, &t, "operand of an operator");
                            self.want(w, *rhs, &t, "operand of an operator");
                        }
                        None => {
                            if let (Some(l), Some(r)) = (self.ty(*lhs), self.ty(*rhs)) {
                                if l != r {
                                    self.wrong(w, origin.clone(), format!("compares {l} with {r}"));
                                }
                            }
                        }
                    }
                }
                next.push(Task::Expr(*lhs, ctx));
                // `&&` and `||` evaluate their right side conditionally.
                let short = matches!(op, BinaryOp::And | BinaryOp::Or);
                next.push(Task::Expr(*rhs, Ctx { conditional: ctx.conditional || short, ..ctx }));
            }
            Expr::If { cond, then, otherwise } if [*cond, *then, *otherwise].iter().any(|e| self.ty(*e).is_some_and(|t| matches!(t, ValueType::Flagged(_)))) => {
                if first {
                    let want = match (self.ty(*cond), self.ty(*then), self.ty(*otherwise)) {
                        (Some(c), Some(t), Some(o)) => Typing::if_(c, t, o),
                        _ => None,
                    };
                    self.typed(w, &origin, ty, want, "an `if`");
                }
                next.push(Task::Expr(*cond, ctx));
                let arm = Ctx { conditional: true, ..ctx };
                next.push(Task::Expr(*then, arm));
                next.push(Task::Expr(*otherwise, arm));
            }
            Expr::If { cond, then, otherwise } => {
                if first {
                    self.want(w, *cond, &ValueType::Bool, "condition of an `if`");
                    self.want(w, *then, ty, "arm of an `if`");
                    self.want(w, *otherwise, ty, "arm of an `if`");
                }
                next.push(Task::Expr(*cond, ctx));
                let arm = Ctx { conditional: true, ..ctx };
                next.push(Task::Expr(*then, arm));
                next.push(Task::Expr(*otherwise, arm));
            }
            Expr::Block { stmts, value } => {
                if first {
                    match value {
                        Some(v) => self.want(w, *v, ty, "value of a block"),
                        None => self.own(w, &origin, ty, &ValueType::Unit),
                    }
                }
                for &s in stmts {
                    next.push(Task::Stmt(s, ctx, false));
                }
                if let Some(v) = value {
                    next.push(Task::Expr(*v, ctx));
                }
                // The block's `let`s end with it.
                next.push(Task::Restore(w.bound.clone()));
            }
            Expr::Call { function, args } => {
                if let Some(f) = self.ir.functions.get(function.index()) {
                    w.calls.insert(*function);
                    if first {
                        self.own(w, &origin, ty, &f.ret);
                        if args.len() != f.params.len() {
                            self.wrong(w, origin.clone(), format!("calls {} with {} args, it takes {}", f.name, args.len(), f.params.len()));
                        }
                        for (a, p) in args.iter().zip(&f.params) {
                            self.want(w, *a, &p.ty, "argument of a call");
                        }
                    }
                }
                next.extend(args.iter().map(|a| Task::Expr(*a, ctx)));
            }
            Expr::Math { args, .. } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    if args.len() != 1 {
                        self.wrong(w, origin.clone(), format!("a math function takes 1 argument, found {}", args.len()));
                    }
                    for a in args {
                        self.want(w, *a, &ValueType::F64, "argument of a math function");
                    }
                }
                next.extend(args.iter().map(|a| Task::Expr(*a, ctx)));
            }
            Expr::Closure { params, body } => {
                let outside = w.bound.clone();
                for p in params {
                    match w.bound.get_mut(*p as usize) {
                        Some(slot) => *slot = true,
                        None if first => self.wrong(w, origin.clone(), format!("closure parameter local {p} out of range")),
                        None => {}
                    }
                }
                if first {
                    match ty {
                        ValueType::Fn(ps, ret) => {
                            if ps.len() != params.len() {
                                self.wrong(w, origin.clone(), format!("closure takes {} parameters, its type {}", params.len(), ps.len()));
                            }
                            self.want(w, *body, ret, "body of a closure");
                        }
                        other => self.wrong(w, origin.clone(), format!("a closure must have a function type, found {other}")),
                    }
                }
                w.rescope();
                next.push(Task::Expr(*body, Ctx { closure: true, ..ctx }));
                // The parameters end with the closure.
                next.push(Task::Restore(outside));
            }
            Expr::Probe { plus, minus } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    if !w.owner.in_body {
                        self.wrong(w, origin.clone(), "probe outside a body".to_string());
                    }
                    for t in [plus, minus] {
                        if let Terminal::Node(i) = t {
                            if w.owner.in_body && *i as usize >= w.owner.nodes {
                                self.wrong(w, origin.clone(), format!("probe terminal node {i} out of range ({} nodes)", w.owner.nodes));
                            }
                        }
                    }
                }
            }
            Expr::Env(env) => {
                if first {
                    let want = if *env == Env::Static { ValueType::Bool } else { ValueType::F64 };
                    self.own(w, &origin, ty, &want);
                }
            }
            Expr::Limit { site, x, f, seed } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    self.want(w, *x, &ValueType::F64, "input of a `limit`");
                    if let Some(got) = self.ty(*f) {
                        if !matches!(got, ValueType::Fn(..)) {
                            let o = self.expr_origin(*f);
                            self.wrong(w, o, format!("the function of a `limit` must have a function type, found {got}"));
                        }
                    }
                    if let Some(s) = seed {
                        self.want(w, *s, &ValueType::F64, "seed of a `limit`");
                    }
                }
                self.site(w, id, &origin, "limit", *site, ctx);
                next.push(Task::Expr(*x, ctx));
                next.push(Task::Expr(*f, ctx));
                next.extend(seed.iter().map(|s| Task::Expr(*s, ctx)));
            }
            Expr::Stim { site, mag, phase } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    self.want(w, *mag, &ValueType::F64, "magnitude of an `ac_stim`");
                    if let Some(p) = phase {
                        self.want(w, *p, &ValueType::F64, "phase of an `ac_stim`");
                    }
                }
                self.site(w, id, &origin, "ac_stim", *site, ctx);
                next.push(Task::Expr(*mag, ctx));
                next.extend(phase.iter().map(|p| Task::Expr(*p, ctx)));
            }
            Expr::Int(value) => {
                if first {
                    match ty.integer_range() {
                        Some((lo, hi)) if !(lo..=hi).contains(value) => {
                            self.wrong(w, origin, format!("integer literal {value} is outside the range of {ty}"));
                        }
                        Some(_) => {}
                        None if ty.is_integer() => {}
                        None => self.wrong(w, origin, format!("an integer literal must have an integer type, found {ty}")),
                    }
                }
            }
            Expr::Cast { expr, to } => {
                if first {
                    self.typed(w, &origin, ty, self.ty(*expr).and_then(|t| Typing::cast(t, to)), "a cast");
                }
                next.push(Task::Expr(*expr, ctx));
            }
            Expr::Method { method, receiver, args } => {
                if first {
                    let arg_types: Option<Vec<&ValueType>> = args.iter().map(|a| self.ty(*a)).collect();
                    let want = match (self.ty(*receiver), arg_types) {
                        (Some(r), Some(a)) => Typing::method(*method, r, &a),
                        _ => None,
                    };
                    self.typed(w, &origin, ty, want, "a method");
                }
                next.push(Task::Expr(*receiver, ctx));
                next.extend(args.iter().map(|a| Task::Expr(*a, ctx)));
            }
            Expr::Unknown => {
                if first && !matches!(ty, ValueType::Flagged(_)) {
                    self.wrong(w, origin, format!("the literal `Unknown` must have a flagged type, found {ty}"));
                }
            }
            Expr::KnownOr { value, default } => {
                if first {
                    let want = match (self.ty(*value), self.ty(*default)) {
                        (Some(v), Some(d)) => Typing::known_or(v, d),
                        _ => None,
                    };
                    self.typed(w, &origin, ty, want, "`known_or`");
                }
                next.push(Task::Expr(*value, ctx));
                next.push(Task::Expr(*default, ctx));
            }
            Expr::Carrier(i) => {
                if first {
                    if !w.owner.in_body {
                        self.wrong(w, origin, "carrier read outside a body".to_string());
                    } else {
                        match w.owner.carriers.get(*i as usize) {
                            Some(c) => self.own(w, &origin, ty, &ValueType::Flagged(Box::new(c.clone()))),
                            None => self.wrong(w, origin, format!("carrier {i} out of range ({} carriers)", w.owner.carriers.len())),
                        }
                    }
                }
            }
            Expr::IsKnown(value) | Expr::High(value) => {
                if first {
                    let high = matches!(data.kind, Expr::High(_));
                    let what = if high { "`high`" } else { "`is_known`" };
                    self.typed(w, &origin, ty, self.ty(*value).and_then(|v| Typing::flag_read(high, v)), what);
                }
                next.push(Task::Expr(*value, ctx));
            }
        }
        next.push(Task::Exit(id));
    }

    /// The recorded type `got` of a digital form must be `want`, the type its form gives its
    /// operands; `None` when the operands fit no rule of `what`.
    fn typed(&mut self, w: &Walk, origin: &Origin, got: &ValueType, want: Option<ValueType>, what: &str) {
        match want {
            Some(want) => self.own(w, origin, got, &want),
            None => self.wrong(w, origin.clone(), format!("the operands of {what} do not fit its types")),
        }
    }

    /// The recorded type `got` of an expression must be what its form produces.
    fn own(&mut self, w: &Walk, origin: &Origin, got: &ValueType, want: &ValueType) {
        if got != want {
            self.wrong(w, origin.clone(), format!("expression is typed {got}, its form gives {want}"));
        }
    }

    /// A `limit` (`kind` "limit") or `ac_stim` site used by expression `user`: placement in this
    /// context, range, and the distinct expressions that use it.
    fn site(&mut self, w: &mut Walk, user: ExprId, origin: &Origin, kind: &str, site: u32, ctx: Ctx) {
        let count = if kind == "limit" { w.owner.limits } else { w.owner.stims };
        if !w.owner.analog {
            self.wrong(w, origin.clone(), format!("{kind} outside the analog block of a body"));
        } else if ctx.conditional {
            self.wrong(w, origin.clone(), format!("{kind} under an `if` or a short-circuit operator"));
        } else if ctx.closure {
            self.wrong(w, origin.clone(), format!("{kind} inside a closure"));
        }
        if site as usize >= count {
            self.wrong(w, origin.clone(), format!("{kind} site {site} out of range ({count} sites)"));
            return;
        }
        let users = if kind == "limit" { &mut w.limit_users } else { &mut w.stim_users };
        if users.len() < count {
            users.resize(count, BTreeSet::new());
        }
        users[site as usize].insert(user);
    }
}

impl Walk {
    /// A walk of `owner`'s code with its first `bound` locals (a function's parameters) bound.
    fn new(owner: Owner, bound: usize) -> Self {
        let mut slots = vec![false; owner.locals.len()];
        for slot in slots.iter_mut().take(bound) {
            *slot = true;
        }
        let mut walk = Walk {
            owner,
            limit_users: Vec::new(),
            stim_users: Vec::new(),
            bound: slots,
            scope: 0,
            scopes: HashMap::new(),
            unbound: HashSet::new(),
            seen: HashSet::new(),
            done: HashSet::new(),
            stmts_done: HashSet::new(),
            on_stack: HashSet::new(),
            calls: BTreeSet::new(),
        };
        walk.rescope();
        walk
    }

    /// Interns the current `bound` set as the scope id of the visits that follow.
    fn rescope(&mut self) {
        let next = self.scopes.len() as u32;
        self.scope = *self.scopes.entry(self.bound.clone()).or_insert(next);
    }
}

impl Ctx {
    fn top() -> Self {
        Ctx { conditional: false, closure: false }
    }
}
