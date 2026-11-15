//! `Ir::check`: the invariants a correct builder sequence guarantees, verified over the arenas.
//! Every id is in range, instances agree with their bodies, paths are unique, `limit` and
//! `ac_stim` sites sit unconditionally at the top of the `analog` block and are used once, and
//! every expression has the type its position requires. It lists what is broken and never panics.

use std::collections::BTreeMap;

use crate::ids::{ExprId, StmtId};
use crate::model::{
    BinaryOp, BranchKind, Expr, Ir, Origin, QuantityRole, Stmt, Terminal, UnaryOp, ValueType,
};

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
        let mut checker = Checker { ir: self, out: Vec::new() };
        checker.ids();
        checker.instances();
        checker.paths();
        checker.code();
        checker.out
    }
}

/// How deep an expression may nest before the walk calls it a cycle.
const MAX_DEPTH: usize = 256;

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
    /// A body (probes and params are readable) rather than a function.
    in_body: bool,
    /// The `analog` block of a body: sites and contributions may appear.
    analog: bool,
}

/// The state of one walk.
struct Walk {
    owner: Owner,
    limit_uses: Vec<u32>,
    stim_uses: Vec<u32>,
    too_deep: bool,
}

/// Where in the code an expression sits.
#[derive(Clone, Copy)]
struct Ctx {
    conditional: bool,
    closure: bool,
    depth: usize,
}

struct Checker<'a> {
    ir: &'a Ir,
    out: Vec<Broken>,
}

impl<'a> Checker<'a> {
    fn report(&mut self, item: String, origin: Origin, what: String) {
        self.out.push(Broken { item, origin, what });
    }

    fn unknown() -> Origin {
        Origin { file: "<ir>".to_string(), line: 0, col: 0 }
    }

    fn expr_origin(&self, id: ExprId) -> Origin {
        self.ir.exprs.get(id.index()).map_or_else(Checker::unknown, |e| e.origin.clone())
    }

    fn stmt_origin(&self, stmt: &Stmt) -> Origin {
        let first = match stmt {
            Stmt::Let { init, .. } => Some(*init),
            Stmt::Expr(e) => Some(*e),
            Stmt::Contribute { addends, .. } => addends.first().map(|a| a.value),
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
            Expr::Probe { .. } | Expr::Env(_) => {}
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
        let mut found = Vec::new();
        for (kind, table) in [("instance", instances), ("node", nodes)] {
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
                in_body: true,
                analog,
            };
            for param in &body.params {
                let Some(check) = param.check else { continue };
                let mut w = Walk::new(owner(false, format!("body {} param {}", body.name, param.name)));
                self.expr(&mut w, check, Ctx::top(), Some(&ValueType::Bool));
            }
            if let Some(analog) = &body.analog {
                let mut w = Walk::new(owner(true, format!("body {}", body.name)));
                for &stmt in analog {
                    self.stmt(&mut w, stmt, Ctx::top(), true);
                }
                self.site_uses(&w);
            }
        }
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
                in_body: false,
                analog: false,
            };
            let mut w = Walk::new(owner);
            self.expr(&mut w, function.body, Ctx::top(), Some(&function.ret));
        }
    }

    fn site_uses(&mut self, w: &Walk) {
        for (kind, uses) in [("limit", &w.limit_uses), ("ac_stim", &w.stim_uses)] {
            for (i, n) in uses.iter().enumerate() {
                if *n > 1 {
                    self.report(w.owner.item.clone(), w.owner.origin.clone(), format!("{kind} site {i} is used by {n} expressions, once is allowed"));
                }
            }
        }
    }

    fn stmt(&mut self, w: &mut Walk, id: StmtId, ctx: Ctx, top: bool) {
        let Some(stmt) = self.ir.stmts.get(id.index()) else { return };
        if w.too_deep {
            return;
        }
        let origin = self.stmt_origin(stmt);
        match stmt {
            Stmt::Let { local, init } => {
                let expected = w.owner.locals.get(*local as usize).cloned();
                if expected.is_none() {
                    self.report(w.owner.item.clone(), origin, format!("let binds local {local}, out of range ({} locals)", w.owner.locals.len()));
                }
                self.expr(w, *init, ctx, expected.as_ref());
            }
            Stmt::Expr(e) => self.expr(w, *e, ctx, None),
            Stmt::Contribute { branch, quantity, addends } => {
                if !top || !w.owner.analog {
                    self.report(w.owner.item.clone(), origin.clone(), "contribution outside the top level of an analog block".to_string());
                }
                let kind = self.body_branch(w, *branch);
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
                    self.expr(w, a.value, ctx, Some(&ValueType::F64));
                }
            }
        }
    }

    /// The kind of branch `i` of the body being walked.
    fn body_branch(&self, w: &Walk, i: u32) -> Option<BranchKind> {
        w.owner.branches.get(i as usize).copied()
    }

    fn wrong(&mut self, w: &Walk, origin: Origin, what: String) {
        self.report(w.owner.item.clone(), origin, what);
    }

    /// The recorded type of `id`, when it exists.
    fn ty(&self, id: ExprId) -> Option<&'a ValueType> {
        self.ir.exprs.get(id.index()).map(|e| &e.ty)
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

    fn expr(&mut self, w: &mut Walk, id: ExprId, ctx: Ctx, expected: Option<&ValueType>) {
        let Some(data) = self.ir.exprs.get(id.index()) else { return };
        if w.too_deep {
            return;
        }
        let origin = data.origin.clone();
        if ctx.depth > MAX_DEPTH {
            if !w.too_deep {
                w.too_deep = true;
                self.wrong(w, origin, format!("expressions nest deeper than {MAX_DEPTH}: a cycle"));
            }
            return;
        }
        if let Some(want) = expected {
            if &data.ty != want {
                self.wrong(w, origin.clone(), format!("expression must be {want}, found {}", data.ty));
            }
        }
        let ty = &data.ty;
        let below = Ctx { depth: ctx.depth + 1, ..ctx };
        match &data.kind {
            Expr::Real(_) => self.own(w, &origin, ty, &ValueType::F64),
            Expr::Bool(_) => self.own(w, &origin, ty, &ValueType::Bool),
            Expr::Unit => self.own(w, &origin, ty, &ValueType::Unit),
            Expr::Local(i) => match w.owner.locals.get(*i as usize) {
                Some(local) => self.own(w, &origin, ty, &local.clone()),
                None => self.wrong(w, origin, format!("local {i} out of range ({} locals)", w.owner.locals.len())),
            },
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
            Expr::Unary { op, expr } => {
                let operand = if *op == UnaryOp::Neg { ValueType::F64 } else { ValueType::Bool };
                self.own(w, &origin, ty, &operand);
                self.want(w, *expr, &operand, "operand of a unary operator");
                self.expr(w, *expr, below, None);
            }
            Expr::Binary { op, lhs, rhs } => {
                let (operands, result) = match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => (Some(ValueType::F64), ValueType::F64),
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => (Some(ValueType::F64), ValueType::Bool),
                    BinaryOp::And | BinaryOp::Or => (Some(ValueType::Bool), ValueType::Bool),
                    BinaryOp::Eq | BinaryOp::Ne => (None, ValueType::Bool),
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
                self.expr(w, *lhs, below, None);
                // `&&` and `||` evaluate their right side conditionally.
                let short = matches!(op, BinaryOp::And | BinaryOp::Or);
                self.expr(w, *rhs, Ctx { conditional: ctx.conditional || short, ..below }, None);
            }
            Expr::If { cond, then, otherwise } => {
                self.want(w, *cond, &ValueType::Bool, "condition of an `if`");
                self.want(w, *then, ty, "arm of an `if`");
                self.want(w, *otherwise, ty, "arm of an `if`");
                self.expr(w, *cond, below, None);
                let arm = Ctx { conditional: true, ..below };
                self.expr(w, *then, arm, None);
                self.expr(w, *otherwise, arm, None);
            }
            Expr::Block { stmts, value } => {
                match value {
                    Some(v) => self.want(w, *v, ty, "value of a block"),
                    None => self.own(w, &origin, ty, &ValueType::Unit),
                }
                for &s in stmts {
                    self.stmt(w, s, below, false);
                }
                if let Some(v) = value {
                    self.expr(w, *v, below, None);
                }
            }
            Expr::Call { function, args } => {
                if let Some(f) = self.ir.functions.get(function.index()) {
                    self.own(w, &origin, ty, &f.ret);
                    if args.len() != f.params.len() {
                        self.wrong(w, origin.clone(), format!("calls {} with {} args, it takes {}", f.name, args.len(), f.params.len()));
                    }
                    for (a, p) in args.iter().zip(&f.params) {
                        self.want(w, *a, &p.ty, "argument of a call");
                    }
                }
                for a in args {
                    self.expr(w, *a, below, None);
                }
            }
            Expr::Math { args, .. } => {
                self.own(w, &origin, ty, &ValueType::F64);
                if args.len() != 1 {
                    self.wrong(w, origin.clone(), format!("a math function takes 1 argument, found {}", args.len()));
                }
                for a in args {
                    self.want(w, *a, &ValueType::F64, "argument of a math function");
                    self.expr(w, *a, below, None);
                }
            }
            Expr::Closure { params, body } => {
                for p in params {
                    if *p as usize >= w.owner.locals.len() {
                        self.wrong(w, origin.clone(), format!("closure parameter local {p} out of range"));
                    }
                }
                match ty {
                    ValueType::Fn(ps, ret) => {
                        if ps.len() != params.len() {
                            self.wrong(w, origin.clone(), format!("closure takes {} parameters, its type {}", params.len(), ps.len()));
                        }
                        self.want(w, *body, ret, "body of a closure");
                    }
                    other => self.wrong(w, origin.clone(), format!("a closure must have a function type, found {other}")),
                }
                self.expr(w, *body, Ctx { closure: true, ..below }, None);
            }
            Expr::Probe { plus, minus } => {
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
            Expr::Env(_) => self.own(w, &origin, ty, &ValueType::F64),
            Expr::Limit { site, x, f, seed } => {
                self.own(w, &origin, ty, &ValueType::F64);
                self.site(w, &origin, "limit", *site, ctx);
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
                self.expr(w, *x, below, None);
                self.expr(w, *f, below, None);
                if let Some(s) = seed {
                    self.expr(w, *s, below, None);
                }
            }
            Expr::Stim { site, mag, phase } => {
                self.own(w, &origin, ty, &ValueType::F64);
                self.site(w, &origin, "ac_stim", *site, ctx);
                self.want(w, *mag, &ValueType::F64, "magnitude of an `ac_stim`");
                if let Some(p) = phase {
                    self.want(w, *p, &ValueType::F64, "phase of an `ac_stim`");
                }
                self.expr(w, *mag, below, None);
                if let Some(p) = phase {
                    self.expr(w, *p, below, None);
                }
            }
        }
    }

    /// The recorded type `got` of an expression must be what its form produces.
    fn own(&mut self, w: &Walk, origin: &Origin, got: &ValueType, want: &ValueType) {
        if got != want {
            self.wrong(w, origin.clone(), format!("expression is typed {got}, its form gives {want}"));
        }
    }

    /// A `limit` (`kind` "limit") or `ac_stim` site use: placement, range, count.
    fn site(&mut self, w: &mut Walk, origin: &Origin, kind: &str, site: u32, ctx: Ctx) {
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
        let uses = if kind == "limit" { &mut w.limit_uses } else { &mut w.stim_uses };
        if uses.len() < count {
            uses.resize(count, 0);
        }
        uses[site as usize] += 1;
    }
}

impl Walk {
    fn new(owner: Owner) -> Self {
        Walk { owner, limit_uses: Vec::new(), stim_uses: Vec::new(), too_deep: false }
    }
}

impl Ctx {
    fn top() -> Self {
        Ctx { conditional: false, closure: false, depth: 0 }
    }
}
