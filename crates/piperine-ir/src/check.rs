//! `Ir::check`: the invariants a correct builder sequence guarantees, verified over the arenas.
//! The digital forms add their own: a body writes only the carriers it may (never an `in`
//! port), a carrier has a drive or commits, never both, a reaction waits on events of its body, a commit
//! sits in a reaction, a `cross` watches a real, and a carrier's initial value is a value of its
//! type (an integer within its `N`-bit range). A contribution may stand anywhere inside the
//! `analog` block (under an `if`, in a nested block) but in a closure, a function or a
//! reaction; a flow or potential branch takes its one quantity, a switch branch both.
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

use crate::contributions::Contributions;
use crate::ids::{ExprId, FunctionId, StmtId, TypeId};
use crate::model::{
    BinaryOp, BodyData, BranchKind, CarrierRole, Child, ChildValue, Constant, EmitField, Env, EventSource, Expr, Ir, NodeRef, NodeRole, Origin, Place, Projection, QuantityRole, ReachedItem, Stmt, Terminal, UnaryOp,
    FunctionData, ReprForm, TypeData, TypeKind, ValueType,
};
use crate::typing::Typing;

mod aggregates;
mod widths;
mod carrier_array;
mod resolved;
mod traits;

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
        checker.resolutions();
        checker.structure();
        checker.paths();
        checker.types();
        checker.type_arena();
        checker.aggregate_constants();
        checker.composite_params();
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
    /// Which locals the source declared `let mut`, parallel to `locals`: only these are assigned.
    mutable: Vec<bool>,
    /// The result type of the function being walked: what a `return` carries. `()` in a body.
    result: ValueType,
    params: Vec<ValueType>,
    nodes: usize,
    branches: Vec<BranchKind>,
    /// The branches the body's `analog` block contributes a potential to.
    driven: BTreeSet<u32>,
    limits: usize,
    /// For each `limit` site: whether it has an `enabled` (it is read under a condition).
    limit_guards: Vec<bool>,
    stims: usize,
    /// The carried types of the body's carriers.
    carriers: Vec<ValueType>,
    /// The element count of each carrier that is an array of carriers, parallel to `carriers`.
    counts: Vec<Option<u64>>,
    /// The driven type of each `Resolved` carrier, parallel to `carriers` (`None` for every other role): a drive or a
    /// commit of a resolved carrier writes this type, and only these carriers have `others()`.
    drive_types: Vec<Option<ValueType>>,
    /// The field types of each of the body's streams.
    streams: Vec<Vec<ValueType>>,
    /// The number of the body's events.
    events: usize,
    /// The number of the body's derivative sites.
    derivatives: usize,
    /// The number of the body's delay sites.
    delays: usize,
    /// The number of the body's noise sources.
    noise_sources: usize,
    /// The number of the body's session parameters.
    session_parameters: usize,
    /// The record stream of each of the body's trace sites.
    traces: Vec<u32>,
    /// The statements of a reaction: `fired` may appear.
    reaction: bool,
    /// The event whose guard is walked: `fired` of another event may appear there (§13 rule 9),
    /// never of itself.
    event: Option<u32>,
    /// A body (probes and params are readable) rather than a function.
    in_body: bool,
    /// The `analog` block of a body: sites and contributions may appear.
    analog: bool,
    /// A value a primitive evaluates on its own (a derivative state, a delay input, a noise density): it may read an
    /// `ac_stim` site, which a statement of the `analog` block writes.
    value_input: bool,
}

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
    /// The innermost loop expression the code sits in, `None` outside every loop and inside a
    /// closure (`break` and `continue` do not cross a closure).
    enclosing_loop: Option<ExprId>,
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
    fn holds(&self, ty: &ValueType, value: Constant) -> bool {
        match (ty, value) {
            (_, Constant::Aggregate(id)) => self.ir.aggregates.get(id.index()).is_some_and(|data| data.ty == *ty),
            (ValueType::Bool, Constant::Bool(_)) | (ValueType::F64, Constant::Real(_)) => true,
            // a type with a hand-written `Repr` is initialized by its levels, the word `encode` gives (GN-05)
            (ValueType::Struct(id) | ValueType::Enum(id), Constant::Int(levels)) => match self.ir.types.get(id.index()).map(|data| data.repr) {
                Some(ReprForm::HandWritten { width, .. }) => width < 128 && (0..1i128 << width).contains(&levels),
                _ => false,
            },
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Int(v)) => {
                ty.integer_range().is_some_and(|(lo, hi)| (lo..=hi).contains(&v))
            }
            (ValueType::F32 | ValueType::F16 | ValueType::F128, Constant::FloatBits(bits)) => self.float_bits_fit(ty, bits),
            (ValueType::UInt(_) | ValueType::Int(_), Constant::Wide(id)) => self.wide_fits(ty, id),
            _ => false,
        }
    }

    /// Whether a pin can hold a value of `ty`: a scalar, or a tuple, an array, a struct, an enum's
    /// payload or an `Option` whose parts a pin can hold (§2: every part has a `Repr`).
    fn carriable(&self, ty: &ValueType) -> bool {
        let mut stack = vec![ty];
        let mut seen = BTreeSet::new();
        while let Some(t) = stack.pop() {
            match t {
                ValueType::Tuple(items) => stack.extend(items),
                ValueType::Array(element, _) => stack.push(element),
                ValueType::Option(inner) => stack.push(inner),
                ValueType::Struct(id) | ValueType::Enum(id) => {
                    let Some(data) = self.ir.types.get(id.index()) else { return false };
                    if !seen.insert(id.index()) {
                        continue;
                    }
                    match &data.kind {
                        TypeKind::Struct { fields, .. } => stack.extend(fields.iter().map(|(_, field)| field)),
                        TypeKind::Enum { variants, .. } => stack.extend(variants.iter().flat_map(|v| v.fields.iter().map(|(_, field)| field))),
                    }
                }
                scalar if Typing::scalar(scalar) => {}
                _ => return false,
            }
        }
        true
    }

    /// The branches with a private flow unknown a flow probe may read: those `body`'s `analog`
    /// block contributes a potential to, wherever the contribution stands, and every switch
    /// branch (it has the unknown whichever kind a solve enables).
    fn potentials(ir: &Ir, body: &BodyData) -> BTreeSet<u32> {
        let potential = Contributions::new(ir).of(body.analog.as_deref().unwrap_or_default()).into_iter().filter(|c| c.quantity == QuantityRole::Potential).map(|c| c.branch);
        let switches = body.branches.iter().enumerate().filter(|(_, b)| b.kind == BranchKind::Switch).map(|(i, _)| i as u32);
        potential.chain(switches).collect()
    }

    fn expr_origin(&self, id: ExprId) -> Origin {
        self.ir.exprs.get(id.index()).map_or_else(Checker::unknown, |e| e.origin.clone())
    }

    fn stmt_origin(&self, stmt: &Stmt) -> Origin {
        let first = match stmt {
            Stmt::Let { init, .. } => Some(*init),
            Stmt::Expr(e) => Some(*e),
            Stmt::Contribute { addends, .. } => addends.first().map(|a| a.value),
            Stmt::Commit { value, .. } | Stmt::BoundStep { value, .. } | Stmt::Assign { value, .. } => Some(*value),
            Stmt::Emit { fields, .. } | Stmt::Trace { fields, .. } => fields.iter().find_map(|f| match f {
                EmitField::Value { value, .. } => Some(*value),
                EmitField::Text { .. } => None,
            }),
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
                    match t {
                        Terminal::Node(i) if i as usize >= body.nodes.len() => {
                            self.report(item.clone(), origin.clone(), format!("branch terminal node {i} out of range ({} nodes)", body.nodes.len()));
                        }
                        Terminal::Own(i) => self.report(item.clone(), origin.clone(), format!("branch terminal own{i}: an own terminal belongs in a child's node slot only")),
                        Terminal::Node(_) | Terminal::Ground => {}
                    }
                }
            }
            for &stmt in body.analog.iter().flatten() {
                self.stmt_id(&item, &origin, stmt);
            }
            for site in &body.derivatives {
                for id in [site.state].into_iter().chain(site.abstol) {
                    self.expr_id(&item, &site.origin, id);
                }
            }
            for site in &body.delays {
                for id in [site.input, site.time].into_iter().chain(site.maximum) {
                    self.expr_id(&item, &site.origin, id);
                }
            }
            for source in &body.noise_sources {
                self.expr_id(&item, &source.origin, source.density);
            }
            for site in &body.limits {
                if let Some(enabled) = site.enabled {
                    self.expr_id(&item, &site.origin, enabled);
                }
            }
            self.carrier_arrays(&item, body);
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
                if let EventSource::Cross { e, ttol, etol } = event.source {
                    for id in [e].into_iter().chain(ttol).chain(etol) {
                        self.expr_id(&item, &event.origin, id);
                    }
                }
                if let EventSource::At(e) = event.source {
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
                for (slot, t) in child.nodes.iter().enumerate() {
                    match t {
                        Terminal::Node(i) if *i as usize >= body.nodes.len() => {
                            self.report(item.clone(), child.origin.clone(), format!("child {}: terminal node {i} out of range", child.name));
                        }
                        Terminal::Own(k) => self.own_slot(&item, child, slot, *k, ir.bodies.get(child.body.index())),
                        Terminal::Node(_) | Terminal::Ground => {}
                    }
                }
            }
        }
        for body in &ir.bodies {
            if let Some(instance) = &body.instance
                && !instance.names_body(&body.name)
            {
                self.report(format!("body {}", body.name), body.origin.clone(), format!("a body of the generic module instance `{}` is named `{}`", instance.name(), body.name));
            }
            self.child_param_ids(body);
            self.child_carrier_ids(body);
        }
        for function in &ir.functions {
            let item = format!("function {}", function.name);
            self.expr_id(&item, &function.origin, function.body);
            if let Some(instance) = &function.instance
                && !instance.names(&function.name)
            {
                self.report(item, function.origin.clone(), format!("a generic instance of `{}` is named `{}`, not `{}`", instance.item, instance.name(), function.name));
            }
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
                Stmt::Assign { value, .. } => self.expr_id(&item, &origin, *value),
                Stmt::BoundStep { value, when, .. } => {
                    for id in [*value].into_iter().chain(*when) {
                        self.expr_id(&item, &origin, id);
                    }
                }
                Stmt::Contribute { addends, .. } => {
                    for a in addends {
                        self.expr_id(&item, &origin, a.value);
                    }
                }
                Stmt::Commit { value, after, target, .. } => {
                    self.expr_id(&item, &origin, *value);
                    if let Some(after) = after {
                        self.expr_id(&item, &origin, *after);
                    }
                    for index in target.iter().flat_map(crate::model::CommitTarget::index_operands) {
                        self.expr_id(&item, &origin, index);
                    }
                }
                Stmt::Emit { fields, .. } | Stmt::Trace { fields, .. } => {
                    for f in fields {
                        if let EmitField::Value { value, .. } = f {
                            self.expr_id(&item, &origin, *value);
                        }
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

    /// A child's own terminal `Own(k)` at `slot`: it must name its own slot, and that node of the
    /// child's body must be a `signal` or a node it reaches into a grandchild (a port is bound to the
    /// parent, never owned).
    fn own_slot(&mut self, item: &str, child: &Child, slot: usize, k: u32, callee: Option<&BodyData>) {
        if k as usize != slot {
            self.report(item.to_string(), child.origin.clone(), format!("child {}: slot {slot} holds own{k}, an own terminal names its own slot", child.name));
            return;
        }
        let Some(node) = callee.and_then(|b| b.nodes.get(slot)) else { return };
        if node.role == NodeRole::Port {
            self.report(item.to_string(), child.origin.clone(), format!("child {}: slot {slot} holds own{k}, but node {} of the child's body is a port", child.name, node.name));
        }
    }

    /// The values set beside the children: each names a child and a param of its body, once, and
    /// a constant is a value of the param's type.
    fn child_param_ids(&mut self, body: &BodyData) {
        let ir = self.ir;
        let item = format!("body {}", body.name);
        for (ci, list) in body.child_params.iter().enumerate() {
            let Some(child) = body.children.get(ci) else {
                if !list.is_empty() {
                    self.report(item.clone(), body.origin.clone(), format!("param values of child {ci}, out of range ({} children)", body.children.len()));
                }
                continue;
            };
            let callee = ir.bodies.get(child.body.index());
            let mut seen: BTreeSet<u32> = BTreeSet::new();
            for (p, value) in list {
                let Some(param) = callee.and_then(|b| b.params.get(*p as usize)) else {
                    if callee.is_some() {
                        self.report(item.clone(), child.origin.clone(), format!("child {}: param index {p} out of range", child.name));
                    }
                    continue;
                };
                if !seen.insert(*p) {
                    self.report(item.clone(), child.origin.clone(), format!("child {}: param {} is set twice", child.name, param.name));
                }
                match value {
                    ChildValue::Const(c) if !self.holds(&param.ty, *c) => {
                        self.report(item.clone(), child.origin.clone(), format!("child {}: the value {c} of param {} is not a value of its type {}", child.name, param.name, param.ty));
                    }
                    ChildValue::Const(_) => {}
                    ChildValue::Expr(e) => self.expr_id(&item, &child.origin, *e),
                }
            }
        }
    }

    /// The carriers each placement binds: one entry per carrier port of the child's body, each a
    /// carrier of the parent holding the port's value type, or open.
    fn child_carrier_ids(&mut self, body: &BodyData) {
        let ir = self.ir;
        let item = format!("body {}", body.name);
        self.child_windows(&item, body);
        for (ci, bound) in body.child_carriers.iter().enumerate() {
            if bound.is_empty() {
                continue;
            }
            let Some(child) = body.children.get(ci) else {
                self.report(item.clone(), body.origin.clone(), format!("carrier bindings of child {ci}, out of range ({} children)", body.children.len()));
                continue;
            };
            let Some(callee) = ir.bodies.get(child.body.index()) else { continue };
            let ports: Vec<_> = callee.carriers.iter().filter(|c| matches!(c.role, CarrierRole::In | CarrierRole::Out | CarrierRole::Reached | CarrierRole::Resolved { .. })).collect();
            if bound.len() != ports.len() {
                self.report(item.clone(), child.origin.clone(), format!("child {} binds {} carriers, body {} has {} carrier ports", child.name, bound.len(), callee.name, ports.len()));
            }
            for (port, binding) in ports.iter().zip(bound) {
                let Some(c) = binding else { continue };
                match body.carriers.get(*c as usize) {
                    None => self.report(item.clone(), child.origin.clone(), format!("child {}: carrier {c} out of range ({} carriers)", child.name, body.carriers.len())),
                    Some(parent) if parent.ty != port.ty => self.report(
                        item.clone(),
                        child.origin.clone(),
                        format!("child {} binds carrier {}: {} to the port {}: {}", child.name, parent.name, parent.ty, port.name, port.ty),
                    ),
                    Some(_) => {}
                }
            }
        }
    }

    fn expr_children(&mut self, item: &str, origin: &Origin, kind: &Expr) {
        let ir = self.ir;
        let mut exprs: Vec<ExprId> = Vec::new();
        match kind {
            Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) => {}
            Expr::Probe { .. } | Expr::Env(_) | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Flow { .. } | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::ParamPrevious(_) | Expr::Others(_) => {}
            Expr::Resolve { by_count, .. } => {
                for function in by_count {
                    if function.index() >= ir.functions.len() {
                        self.report(item.to_string(), origin.clone(), format!("function id {} out of range ({} functions)", function.0, ir.functions.len()));
                    }
                }
            }
            Expr::Unwrap { is, value, .. } => exprs.extend([*is, *value]),
            Expr::Same { a, b } => exprs.extend([*a, *b]),
            Expr::LevelCast { expr, .. } => exprs.push(*expr),
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
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => {
                for &s in kind.statements() {
                    self.stmt_id(item, origin, s);
                }
                exprs.extend(kind.operands());
            }
            Expr::FunctionRef(function) => {
                if function.index() >= ir.functions.len() {
                    self.report(item.to_string(), origin.clone(), format!("function id {} out of range ({} functions)", function.0, ir.functions.len()));
                }
            }
            Expr::Limit { x, f, seed, .. } => {
                exprs.extend([*x, *f]);
                exprs.extend(seed);
            }
            Expr::Stim { mag, phase, .. } => {
                exprs.push(*mag);
                exprs.extend(phase);
            }
            Expr::SessionParameter { .. } => {}
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

    /// The records of structure a body keeps (analog-structure): its merges, its reaches and its
    /// inflows, each in range and kept by every instance of the body.
    fn structure(&mut self) {
        let ir = self.ir;
        for body in &ir.bodies {
            let item = format!("body {}", body.name);
            self.merges_in_range(&item, body);
            self.reaches_in_range(&item, body);
            for inflow in &body.inflows {
                let port = body.nodes.get(inflow.port as usize);
                if port.is_none_or(|n| n.role != NodeRole::Port) {
                    self.report(item.clone(), body.origin.clone(), format!("inflow of node {}: not a port of the body", inflow.port));
                }
                match body.branches.get(inflow.branch as usize) {
                    Some(branch) if branch.kind == BranchKind::Potential => {}
                    Some(_) => self.report(item.clone(), body.origin.clone(), format!("inflow measured by branch {}, which is not a potential branch", inflow.branch)),
                    None => self.report(item.clone(), body.origin.clone(), format!("inflow measured by branch {}, out of range ({} branches)", inflow.branch, body.branches.len())),
                }
            }
            for (i, node) in body.nodes.iter().enumerate() {
                let listed = body.reaches.iter().any(|r| matches!(r.item, ReachedItem::Node { node, .. } if node as usize == i));
                if node.role == NodeRole::Reached && !listed {
                    self.report(item.clone(), node.origin.clone(), format!("reached node {} names no child node in `reaches`", node.name));
                }
            }
            for (i, carrier) in body.carriers.iter().enumerate() {
                let listed = body.reaches.iter().any(|r| matches!(r.item, ReachedItem::Carrier { carrier, .. } if carrier as usize == i));
                if carrier.role == CarrierRole::Reached && !listed {
                    self.report(item.clone(), carrier.origin.clone(), format!("reached carrier {} names no child carrier in `reaches`", carrier.name));
                }
            }
        }
        self.instances_keep_structure();
    }

    /// Each merge of `body` names nodes of the body, and two different ones.
    fn merges_in_range(&mut self, item: &str, body: &BodyData) {
        for merge in &body.merges {
            for end in [merge.plus, merge.minus] {
                match end {
                    Terminal::Node(i) if i as usize >= body.nodes.len() => {
                        self.report(item.to_string(), body.origin.clone(), format!("merge terminal node {i} out of range ({} nodes)", body.nodes.len()));
                    }
                    Terminal::Own(i) => self.report(item.to_string(), body.origin.clone(), format!("merge terminal own{i}: an own terminal belongs in a child's node slot only")),
                    Terminal::Node(_) | Terminal::Ground => {}
                }
            }
            if merge.plus == merge.minus {
                self.report(item.to_string(), body.origin.clone(), "a merge of a node with itself".to_string());
            }
        }
    }

    /// Each reach names a child of the body, a reached node or carrier of the body and the
    /// child's own.
    fn reaches_in_range(&mut self, item: &str, body: &BodyData) {
        let ir = self.ir;
        for reach in &body.reaches {
            let Some(child) = body.children.get(reach.child as usize) else {
                self.report(item.to_string(), body.origin.clone(), format!("reach into child {}, out of range ({} children)", reach.child, body.children.len()));
                continue;
            };
            let Some(callee) = ir.bodies.get(child.body.index()) else { continue };
            match reach.item {
                ReachedItem::Node { node, child_node } => {
                    if body.nodes.get(node as usize).is_none_or(|n| n.role != NodeRole::Reached) {
                        self.report(item.to_string(), child.origin.clone(), format!("reach of node {node}: not a node of role `reached`"));
                    }
                    if child_node as usize >= callee.nodes.len() {
                        self.report(item.to_string(), child.origin.clone(), format!("reach of node {child_node} of child {}, out of range ({} nodes)", child.name, callee.nodes.len()));
                    }
                }
                ReachedItem::Carrier { carrier, child_carrier } => {
                    if body.carriers.get(carrier as usize).is_none_or(|c| !matches!(c.role, CarrierRole::Reached | CarrierRole::Resolved { .. })) {
                        self.report(item.to_string(), child.origin.clone(), format!("reach of carrier {carrier}: not a carrier of role `reached` or `resolved`"));
                    }
                    if child_carrier as usize >= callee.carriers.len() {
                        self.report(item.to_string(), child.origin.clone(), format!("reach of carrier {child_carrier} of child {}, out of range ({} carriers)", child.name, callee.carriers.len()));
                    }
                }
            }
        }
    }

    /// What every instance of a body does with the body's structure records: a merged branch's two
    /// ends are one ref, and a reached node or carrier is the child instance's own.
    fn instances_keep_structure(&mut self) {
        let ir = self.ir;
        for instance in &ir.instances {
            let Some(body) = ir.bodies.get(instance.body.index()) else { continue };
            let item = format!("instance {}", instance.path);
            let end = |t: Terminal| match t {
                Terminal::Node(i) => instance.nodes.get(i as usize).copied(),
                Terminal::Ground => Some(NodeRef::Ground),
                Terminal::Own(_) => None,
            };
            for merge in &body.merges {
                if let (Some(plus), Some(minus)) = (end(merge.plus), end(merge.minus)) {
                    if plus != minus {
                        self.report(item.clone(), instance.origin.clone(), "binds the two ends of a merged branch to different nodes".to_string());
                    }
                }
            }
            for reach in &body.reaches {
                let Some(child) = instance.children.get(reach.child as usize).and_then(|c| ir.instances.get(c.index())) else { continue };
                match reach.item {
                    ReachedItem::Node { node, child_node } => {
                        let (own, theirs) = (instance.nodes.get(node as usize), child.nodes.get(child_node as usize));
                        if own.is_some() && theirs.is_some() && own != theirs {
                            self.report(item.clone(), instance.origin.clone(), format!("reached node {node} is not bound to node {child_node} of child {}", child.path));
                        }
                    }
                    ReachedItem::Carrier { carrier, child_carrier } => {
                        let (own, theirs) = (instance.nets.get(carrier as usize), child.nets.get(child_carrier as usize));
                        if own.is_some() && theirs.is_some() && own != theirs {
                            self.report(item.clone(), instance.origin.clone(), format!("reached carrier {carrier} is not bound to carrier {child_carrier} of child {}", child.path));
                        }
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

    /// Every type written in the IR is well formed: an integer width in 1..=2^32 - 1 (Q4 of the
    /// number widths, which supersedes RS-Q6's 1..=64: a width past 64 is a `U64` field of
    /// `ceil(N/64)` elements).
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
            for stream in &body.streams {
                let mut names: BTreeSet<&str> = BTreeSet::new();
                for f in &stream.fields {
                    let at = format!("{item} stream {}", stream.name);
                    if !self.carriable(&f.ty) {
                        self.report(at.clone(), body.origin.clone(), format!("a stream field holds `bool`, `f64` or an integer type, found {} for {}", f.ty, f.name));
                    }
                    if !names.insert(f.name.as_str()) {
                        self.report(at.clone(), body.origin.clone(), format!("stream {} has two fields named {}", stream.name, f.name));
                    }
                    found.push((at, body.origin.clone(), f.ty.clone()));
                }
            }
            for c in &body.carriers {
                if !self.carriable(&c.ty) {
                    self.report(
                        format!("{item} carrier {}", c.name),
                        c.origin.clone(),
                        format!("a carrier holds `bool`, `f64`, an integer type, an enum, or a tuple, array, struct or `Option` of them, found {}", c.ty),
                    );
                } else if let Some(init) = c.init
                    // a resolved carrier reads `ty` and is driven with `driven`: its initial value is its first drive's (§5.2.3)
                    && let held = match &c.role {
                        CarrierRole::Resolved { driven } => driven,
                        _ => &c.ty,
                    }
                    && !self.holds(held, init)
                    // an array carrier's initial value is one element (`[x; K]`) or every element (`[a, b, c]`)
                    && !c.count.is_some_and(|count| self.holds(&ValueType::Array(Box::new(c.ty.clone()), count), init))
                {
                    self.report(
                        format!("{item} carrier {}", c.name),
                        c.origin.clone(),
                        format!("the initial value {init} of carrier {} is not a value of its type {held}", c.name),
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
            if let Some(what) = self.malformed(&ty) {
                self.report(item, origin, what);
            }
        }
        for (index, data) in ir.types.iter().enumerate() {
            self.type_instance(data);
            self.hand_written_repr(index, data);
        }
    }

    /// A type that is a generic instance is named `item<arguments>`.
    fn type_instance(&mut self, data: &TypeData) {
        if let Some(instance) = &data.instance
            && data.name != instance.type_name()
        {
            self.report(format!("type {}", data.name), data.origin.clone(), format!("a generic instance of `{}` is named `{}`, not `{}`", instance.item, instance.type_name(), data.name));
        }
    }

    /// A hand-written `Repr` (GN-05): `encode` takes the type and returns `[bool; width]`, `decode`
    /// takes `[bool; width]` and returns `Option` of the type, and `width` is at least one level.
    fn hand_written_repr(&mut self, index: usize, data: &TypeData) {
        let ReprForm::HandWritten { width, encode, decode } = data.repr else { return };
        let item = format!("type {}", data.name);
        let own = match data.kind {
            TypeKind::Struct { .. } => ValueType::Struct(TypeId(index as u32)),
            TypeKind::Enum { .. } => ValueType::Enum(TypeId(index as u32)),
        };
        let levels = ValueType::Array(Box::new(ValueType::Bool), u64::from(width));
        if width == 0 {
            self.report(item.clone(), data.origin.clone(), "a hand-written Repr has at least one level".to_string());
        }
        let (Some(encode), Some(decode)) = (self.ir.functions.get(encode.index()), self.ir.functions.get(decode.index())) else {
            self.report(item, data.origin.clone(), "a hand-written Repr names a function outside the arena".to_string());
            return;
        };
        let takes = |function: &FunctionData, ty: &ValueType| function.params.len() == 1 && function.params[0].ty == *ty;
        if !takes(encode, &own) || encode.ret != levels {
            self.report(item.clone(), data.origin.clone(), format!("the encode of a hand-written Repr takes {own} and returns {levels}, `{}` does not", encode.name));
        }
        if !takes(decode, &levels) || decode.ret != ValueType::Option(Box::new(own.clone())) {
            self.report(item, data.origin.clone(), format!("the decode of a hand-written Repr takes {levels} and returns Option<{own}>, `{}` does not", decode.name));
        }
    }

    /// What is wrong with `ty`, read through function types and aggregate parts.
    fn malformed(&self, ty: &ValueType) -> Option<String> {
        let mut stack = vec![ty];
        while let Some(t) = stack.pop() {
            match t {
                ValueType::UInt(0) | ValueType::Int(0) => {
                    return Some(format!("type {t} has a width outside 1..=4294967295"));
                }
                ValueType::Fn(params, ret) => {
                    stack.extend(params);
                    stack.push(ret);
                }
                ValueType::Struct(id) | ValueType::Enum(id) if id.index() >= self.ir.types.len() => {
                    return Some(format!("type {t} names a type outside the arena ({} types)", self.ir.types.len()));
                }
                ValueType::Struct(id) if !matches!(self.ir.types[id.index()].kind, TypeKind::Struct { .. }) => {
                    return Some(format!("type {t} names an enum as a struct"));
                }
                ValueType::Enum(id) if !matches!(self.ir.types[id.index()].kind, TypeKind::Enum { .. }) => {
                    return Some(format!("type {t} names a struct as an enum"));
                }
                ValueType::Tuple(parts) => stack.extend(parts),
                ValueType::Array(element, _) | ValueType::Option(element) => stack.push(element),
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
        let ir = self.ir;
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
                if let Some(Expr::Env(env @ (Env::Time | Env::Static | Env::Freq))) = self.ir.exprs.get(e.index()).map(|d| &d.kind) {
                    let what = match env {
                        Env::Time => "time()",
                        Env::Freq => "freq()",
                        _ => "static()",
                    };
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

    /// A param value of a placement reads only params of the parent: no node, carrier, local,
    /// environment, site, flow or previous value, directly or through a function it calls.
    fn parent_reads(&mut self, item: &str, root: ExprId) {
        let ir = self.ir;
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
            self.report(item.to_string(), data.origin.clone(), format!("the value of a child param reads {what}: only the parent's params may be read"));
        }
    }

    /// The emits among `stmts`.
    fn emits(&self, stmts: &[StmtId]) -> Vec<StmtId> {
        stmts.iter().copied().filter(|s| matches!(self.ir.stmts.get(s.index()), Some(Stmt::Emit { .. }))).collect()
    }

    fn discrete(&mut self) {
        let ir = self.ir;
        for body in &ir.bodies {
            self.writes(body);
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
                if let EventSource::Cross { e, ttol, etol } = event.source {
                    if let Some(ty) = self.ty(e).filter(|t| **t != ValueType::F64) {
                        self.report(format!("{item} event {k}"), event.origin.clone(), format!("a `cross` watches a real, found {ty}"));
                    }
                    for (name, tol) in [("ttol", ttol), ("etol", etol)] {
                        let Some(tol) = tol else { continue };
                        if let Some(ty) = self.ty(tol).filter(|t| **t != ValueType::F64) {
                            self.report(format!("{item} event {k}"), event.origin.clone(), format!("the `{name}` of a `cross` is an `f64`, found {ty}"));
                        }
                        for read in self.reach_calls(&[tol]) {
                            if matches!(self.ir.exprs.get(read.index()).map(|d| &d.kind), Some(Expr::Probe { .. } | Expr::Carrier(_) | Expr::Env(Env::Time | Env::Freq))) {
                                let at = self.expr_origin(read);
                                self.report(format!("{item} event {k}"), at, format!("the `{name}` of a `cross` reads an input: it is a constant or a param expression"));
                            }
                        }
                    }
                }
                if let EventSource::ParamChanged(p) = event.source {
                    if p as usize >= body.params.len() {
                        self.report(format!("{item} event {k}"), event.origin.clone(), format!("`changed` of param {p}, out of range ({} params)", body.params.len()));
                    }
                }
                if let EventSource::At(e) = event.source {
                    if let Some(ty) = self.ty(e).filter(|t| **t != ValueType::F64) {
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
            // The `abstol` of a derivative site is a constant or a param expression, as the
            // tolerances of a `cross` are.
            for (k, site) in body.derivatives.iter().enumerate() {
                let Some(abstol) = site.abstol else { continue };
                for read in self.reach_calls(&[abstol]) {
                    if matches!(self.ir.exprs.get(read.index()).map(|d| &d.kind), Some(Expr::Probe { .. } | Expr::Carrier(_) | Expr::Env(Env::Time | Env::Freq))) {
                        let at = self.expr_origin(read);
                        self.report(format!("{item} derivative {k}"), at, "the `abstol` of a derivative site reads an input: it is a constant or a param expression".to_string());
                    }
                }
            }
            // A commit anywhere but in a reaction: the analog block, a `where`, a drive, an event.
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
            for (s, _) in self.commits(&stmts) {
                let origin = self.stmt_origin(&ir.stmts[s.index()]);
                self.report(item.clone(), origin, "a commit outside a reaction".to_string());
            }
            for s in self.emits(&stmts) {
                let origin = self.stmt_origin(&ir.stmts[s.index()]);
                self.report(item.clone(), origin, "an emit outside a reaction".to_string());
            }
        }
        for function in &ir.functions {
            let (_, stmts) = self.reach(&[], &[function.body]);
            for (s, _) in self.commits(&stmts) {
                let origin = self.stmt_origin(&ir.stmts[s.index()]);
                self.report(format!("function {}", function.name), origin, "a commit outside a reaction".to_string());
            }
            for s in self.emits(&stmts) {
                let origin = self.stmt_origin(&ir.stmts[s.index()]);
                self.report(format!("function {}", function.name), origin, "an emit outside a reaction".to_string());
            }
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
                self.report(format!("{item} drive {}", c.name), drive.origin.clone(), format!("drives the `in` port {}: an input is written outside the body", c.name));
            }
            if let Some(c) = reached(drive.carrier) {
                self.report(format!("{item} drive {}", c.name), drive.origin.clone(), format!("drives the reached carrier {}: a child's carrier is read only", c.name));
            }
        }
        let mut committed: Vec<(u32, ElementWritten)> = Vec::new();
        for (k, reaction) in body.reactions.iter().enumerate() {
            let (_, stmts) = self.reach(&reaction.stmts, &[]);
            for (s, carrier) in self.commits(&stmts) {
                committed.push((carrier, self.element_written(&self.ir.stmts[s.index()])));
                if let Some(c) = input(carrier) {
                    let origin = self.stmt_origin(&self.ir.stmts[s.index()]);
                    self.report(format!("{item} reaction {k}"), origin, format!("commits to the `in` port {}: an input is written outside the body", c.name));
                }
                if let Some(c) = reached(carrier) {
                    let origin = self.stmt_origin(&self.ir.stmts[s.index()]);
                    self.report(format!("{item} reaction {k}"), origin, format!("commits to the reached carrier {}: a child's carrier is read only", c.name));
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
                self.report(format!("{item} carrier {}", carrier.name), carrier.origin.clone(), format!("carrier {} has both a drive and a commit", carrier.name));
            }
        }
    }

    /// The element of its carrier a commit writes.
    fn element_written(&self, stmt: &Stmt) -> ElementWritten {
        let Stmt::Commit { target, .. } = stmt else { return ElementWritten::Whole };
        match target.as_ref().and_then(|t| t.element) {
            None => ElementWritten::Whole,
            Some(index) => match self.ir.exprs.get(index.index()).map(|d| &d.kind) {
                Some(Expr::Int(n)) if *n >= 0 => ElementWritten::Constant(*n as u64),
                _ => ElementWritten::Dynamic,
            },
        }
    }

    // ---- code ------------------------------------------------------------------------------

    fn code(&mut self) {
        let ir = self.ir;
        // The functions the `analog` blocks call, directly.
        let mut analog_calls: BTreeSet<FunctionId> = BTreeSet::new();
        for body in &ir.bodies {
            let owner = |analog: bool, item: String| Owner {
                item,
                origin: body.origin.clone(),
                locals: body.locals.iter().map(|l| l.ty.clone()).collect(),
                mutable: body.locals.iter().map(|l| l.mutable).collect(),
                result: ValueType::Unit,
                params: body.params.iter().map(|p| p.ty.clone()).collect(),
                nodes: body.nodes.len(),
                branches: body.branches.iter().map(|b| b.kind).collect(),
                driven: Checker::potentials(ir, body),
                limits: body.limits.len(),
                limit_guards: body.limits.iter().map(|s| s.enabled.is_some()).collect(),
                stims: body.stims.len(),
                carriers: body.carriers.iter().map(|c| c.ty.clone()).collect(),
                counts: body.carriers.iter().map(|c| c.count).collect(),
                drive_types: body.carriers.iter().map(|c| match &c.role {
                    CarrierRole::Resolved { driven } => Some(driven.clone()),
                    _ => None,
                }).collect(),
                streams: body.streams.iter().map(|s| s.fields.iter().map(|f| f.ty.clone()).collect()).collect(),
                events: body.events.len(),
                derivatives: body.derivatives.len(),
                delays: body.delays.len(),
                noise_sources: body.noise_sources.len(),
                session_parameters: body.session_parameters.len(),
                traces: body.traces.iter().map(|t| t.stream).collect(),
                reaction: false,
                event: None,
                in_body: true,
                analog,
                value_input: false,
            };
            for param in &body.params {
                let Some(check) = param.check else { continue };
                let mut w = Walk::new(owner(false, format!("body {} param {}", body.name, param.name)), 0);
                self.expect(&w, check, &ValueType::Bool);
                self.walk(&mut w, Task::Expr(check, Ctx::top()));
            }
            for (ci, list) in body.child_params.iter().enumerate() {
                let Some(child) = body.children.get(ci) else { continue };
                let callee = ir.bodies.get(child.body.index());
                for (p, value) in list {
                    let ChildValue::Expr(e) = value else { continue };
                    let Some(param) = callee.and_then(|b| b.params.get(*p as usize)) else { continue };
                    let item = format!("body {} child {} param {}", body.name, child.name, param.name);
                    let mut w = Walk::new(owner(false, item.clone()), 0);
                    self.expect(&w, *e, &param.ty);
                    self.walk(&mut w, Task::Expr(*e, Ctx::top()));
                    self.parent_reads(&item, *e);
                }
            }
            if let Some(analog) = &body.analog {
                let mut w = Walk::new(owner(true, format!("body {}", body.name)), 0);
                for &stmt in analog {
                    self.walk(&mut w, Task::Stmt(stmt, Ctx::top(), true));
                }
                self.site_uses(&w);
                analog_calls.extend(w.calls);
            }
            for drive in &body.drives {
                let name = body.carriers.get(drive.carrier as usize).map_or("<bad carrier>", |c| c.name.as_str());
                let mut w = Walk::new(owner(false, format!("body {} drive {name}", body.name)), 0);
                if let Some(c) = body.carriers.get(drive.carrier as usize) {
                    let held = match &c.role {
                        CarrierRole::Resolved { driven } => driven,
                        _ => &c.ty,
                    };
                    if let Some(written) = self.drive_type(&w, &drive.origin, drive.carrier, held, drive.element, &drive.path) {
                        self.carried(&w, drive.value, &written, "the value of a drive");
                    }
                }
                for step in &drive.path {
                    if let Projection::Index(index) = step {
                        self.walk(&mut w, Task::Expr(*index, Ctx::top()));
                    }
                }
                self.walk(&mut w, Task::Expr(drive.value, Ctx::top()));
                if let Some(after) = drive.after {
                    self.carried(&w, after, &ValueType::F64, "the delay of a drive");
                    self.walk(&mut w, Task::Expr(after, Ctx::top()));
                }
            }
            for (k, event) in body.events.iter().enumerate() {
                let mut guarded = owner(false, format!("body {} event {k}", body.name));
                guarded.event = Some(k as u32);
                let mut w = Walk::new(guarded, 0);
                match event.source {
                    EventSource::Cross { e, ttol, etol } => {
                        for id in [e].into_iter().chain(ttol).chain(etol) {
                            self.walk(&mut w, Task::Expr(id, Ctx::top()));
                        }
                    }
                    EventSource::At(e) => self.walk(&mut w, Task::Expr(e, Ctx::top())),
                    EventSource::ElementChanged { carrier, element } => {
                        self.element_event(&format!("body {} event {k}", body.name), &event.origin, body, carrier, element)
                    }
                    EventSource::Changed(_) | EventSource::ParamChanged(_) | EventSource::Settle => {}
                }
                if let Some(guard) = event.guard {
                    self.carried(&w, guard, &ValueType::Bool, "the guard of an event");
                    self.walk(&mut w, Task::Expr(guard, Ctx::top()));
                }
            }
            for (k, site) in body.limits.iter().enumerate() {
                let Some(enabled) = site.enabled else { continue };
                let mut w = Walk::new(owner(false, format!("body {} limit {k}", body.name)), 0);
                self.expect(&w, enabled, &ValueType::Bool);
                self.walk(&mut w, Task::Expr(enabled, Ctx::top()));
            }
            for (k, site) in body.derivatives.iter().enumerate() {
                let mut w = Walk::new(Owner { value_input: true, ..owner(false, format!("body {} derivative {k}", body.name)) }, 0);
                self.expect(&w, site.state, &ValueType::F64);
                self.walk(&mut w, Task::Expr(site.state, Ctx::top()));
                if let Some(abstol) = site.abstol {
                    self.expect(&w, abstol, &ValueType::F64);
                    self.walk(&mut w, Task::Expr(abstol, Ctx::top()));
                }
            }
            for (k, site) in body.delays.iter().enumerate() {
                let mut w = Walk::new(Owner { value_input: true, ..owner(false, format!("body {} delay {k}", body.name)) }, 0);
                for id in [site.input, site.time].into_iter().chain(site.maximum) {
                    self.expect(&w, id, &ValueType::F64);
                    self.walk(&mut w, Task::Expr(id, Ctx::top()));
                }
            }
            for (k, source) in body.noise_sources.iter().enumerate() {
                let mut w = Walk::new(Owner { value_input: true, ..owner(false, format!("body {} noise source {k}", body.name)) }, 0);
                self.expect(&w, source.density, &ValueType::F64);
                self.walk(&mut w, Task::Expr(source.density, Ctx::top()));
            }
            for (k, reaction) in body.reactions.iter().enumerate() {
                let item = format!("body {} reaction {k}", body.name);
                let mut owner = owner(false, item);
                owner.locals = reaction.locals.iter().map(|l| l.ty.clone()).collect();
                owner.mutable = reaction.locals.iter().map(|l| l.mutable).collect();
                owner.reaction = true;
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
                mutable: function.locals.iter().map(|l| l.mutable).collect(),
                result: function.ret.clone(),
                params: Vec::new(),
                nodes: 0,
                branches: Vec::new(),
                driven: BTreeSet::new(),
                limits: 0,
                limit_guards: Vec::new(),
                stims: 0,
                carriers: Vec::new(),
                counts: Vec::new(),
                drive_types: Vec::new(),
                streams: Vec::new(),
                events: 0,
                derivatives: 0,
                delays: 0,
                noise_sources: 0,
                session_parameters: 0,
                traces: Vec::new(),
                reaction: false,
                event: None,
                in_body: false,
                analog: false,
                value_input: false,
            };
            let mut w = Walk::new(owner, function.params.len());
            self.expect(&w, function.body, &function.ret);
            self.walk(&mut w, Task::Expr(function.body, Ctx::top()));
            graph.push(w.calls);
        }
        self.recursion(&graph, &analog_calls);
    }

    /// Every function that reaches itself through calls and is reached from an `analog` body
    /// (`analog_calls`, the functions that code calls, directly). Recursion elsewhere is allowed
    /// (§3 rule 12): the elaborator unrolls the recursion `analog` reaches, since a solve needs
    /// a bounded contribution.
    fn recursion(&mut self, graph: &[BTreeSet<FunctionId>], analog_calls: &BTreeSet<FunctionId>) {
        let reaches = |from: Vec<usize>| -> BTreeSet<usize> {
            let mut seen: BTreeSet<usize> = BTreeSet::new();
            let mut stack = from;
            while let Some(f) = stack.pop() {
                if f < graph.len() && seen.insert(f) {
                    stack.extend(graph[f].iter().map(|g| g.index()));
                }
            }
            seen
        };
        let from_analog = reaches(analog_calls.iter().map(|f| f.index()).collect());
        for (i, function) in self.ir.functions.iter().enumerate() {
            let recursive = reaches(graph[i].iter().map(|f| f.index()).collect()).contains(&i);
            if recursive && from_analog.contains(&i) {
                self.report(
                    format!("function {}", function.name),
                    function.origin.clone(),
                    format!("function {} calls itself, directly or through other functions, and `analog` reaches it (§3 rule 12)", function.name),
                );
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
            Stmt::Assign { place, op, value } => {
                self.assignment(w, &origin, place, *op, *value);
                next.push(Task::Expr(*value, ctx));
                next.extend(place.index_operands().map(|index| Task::Expr(index, ctx)));
            }
            Stmt::BoundStep { value, when, .. } => {
                if !top || !w.owner.analog {
                    self.report(w.owner.item.clone(), origin.clone(), "`bound_step` outside the top level of an analog block".to_string());
                }
                self.want(w, *value, &ValueType::F64, "the step of a `bound_step`");
                next.push(Task::Expr(*value, ctx));
                if let Some(when) = when {
                    self.want(w, *when, &ValueType::Bool, "the `when` of a `bound_step`");
                    next.push(Task::Expr(*when, ctx));
                }
            }
            Stmt::Contribute { branch, quantity, addends } => {
                // A contribution may stand under an `if` or in a nested block, anywhere inside
                // the analog block but a closure (analog-structure, mechanism 1).
                if !w.owner.analog {
                    self.report(w.owner.item.clone(), origin.clone(), "contribution outside the analog block of a body".to_string());
                } else if ctx.closure {
                    self.report(w.owner.item.clone(), origin.clone(), "contribution inside a closure".to_string());
                } else if ctx.enclosing_loop.is_some() {
                    // §11 rule 12: the elaborator unrolls a loop that contributes, so none reaches the IR.
                    self.report(w.owner.item.clone(), origin.clone(), "contribution inside a loop".to_string());
                }
                let kind = w.owner.branches.get(*branch as usize).copied();
                match kind {
                    None if w.owner.analog => {
                        self.report(w.owner.item.clone(), origin.clone(), format!("contribution to branch {branch}, out of range"));
                    }
                    Some(kind) => {
                        let flow = *quantity == QuantityRole::Flow;
                        let matches = match kind {
                            BranchKind::Flow => flow,
                            BranchKind::Potential => !flow,
                            BranchKind::Switch => true,
                        };
                        if !matches {
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
            Stmt::Commit { carrier, value, after, target } => {
                match w.owner.carriers.get(*carrier as usize).cloned() {
                    Some(held) => {
                        let ty = w.owner.drive_types.get(*carrier as usize).cloned().flatten().unwrap_or(held);
                        if let Some(written) = self.commit_type(w, &origin, *carrier, &ty, target.as_ref()) {
                            self.carried(w, *value, &written, "the value of a commit");
                        }
                    }
                    None => self.report(w.owner.item.clone(), origin.clone(), format!("commit to carrier {carrier}, out of range ({} carriers)", w.owner.carriers.len())),
                }
                next.push(Task::Expr(*value, ctx));
                if let Some(target) = target {
                    next.extend(target.index_operands().map(|index| Task::Expr(index, ctx)));
                }
                if let Some(after) = after {
                    self.carried(w, *after, &ValueType::F64, "the delay of a commit");
                    next.push(Task::Expr(*after, ctx));
                }
            }
            Stmt::Trace { site, fields } => {
                // A trace is written where the analog block executes it: not in a function, a
                // closure, a loop or a reaction.
                if !w.owner.analog {
                    self.report(w.owner.item.clone(), origin.clone(), "a trace outside the analog block of a body".to_string());
                } else if ctx.closure {
                    self.report(w.owner.item.clone(), origin.clone(), "a trace inside a closure".to_string());
                }
                let Some(stream) = w.owner.traces.get(*site as usize).copied() else {
                    self.report(w.owner.item.clone(), origin.clone(), format!("trace of site {site}, out of range ({} sites)", w.owner.traces.len()));
                    return;
                };
                let Some(types) = w.owner.streams.get(stream as usize).cloned() else {
                    self.report(w.owner.item.clone(), origin.clone(), format!("trace site {site} names stream {stream}, out of range ({} streams)", w.owner.streams.len()));
                    return;
                };
                for f in fields {
                    let EmitField::Value { field, value } = f else { continue };
                    match types.get(*field as usize) {
                        Some(ty) => self.carried(w, *value, ty, "the value of a trace field"),
                        None => self.report(w.owner.item.clone(), origin.clone(), format!("trace writes field {field} of stream {stream}, out of range ({} fields)", types.len())),
                    }
                    next.push(Task::Expr(*value, ctx));
                }
            }
            Stmt::Emit { stream, fields } => {
                // An emit outside a reaction is named by `discrete`; its stream is a body's.
                let types = if w.owner.in_body { w.owner.streams.get(*stream as usize).cloned() } else { None };
                if w.owner.in_body && types.is_none() {
                    self.report(w.owner.item.clone(), origin.clone(), format!("emit to stream {stream}, out of range ({} streams)", w.owner.streams.len()));
                }
                for f in fields {
                    let EmitField::Value { field, value } = f else { continue };
                    if let Some(types) = &types {
                        match types.get(*field as usize) {
                            Some(ty) => self.carried(w, *value, ty, "the value of an emit field"),
                            None => self.report(w.owner.item.clone(), origin.clone(), format!("emit writes field {field} of stream {stream}, out of range ({} fields)", types.len())),
                        }
                    }
                    next.push(Task::Expr(*value, ctx));
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

    /// `id`, a value written to a carrier of type `want` (or a delay, `want` being `f64`): its type
    /// is `want`.
    fn carried(&mut self, w: &Walk, id: ExprId, want: &ValueType, position: &str) {
        if let Some(got) = self.ty(id) {
            if got != want {
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
            Expr::Binary { .. } if self.operates_on_aggregates(&data.kind) => self.aggregate_comparison(w, id, first, ctx, next),
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
                    // typed by its operand (H.2): a float of any width gives a float of that width, `f64` otherwise
                    let operand = args.first().and_then(|a| self.ty(*a)).filter(|t| t.is_other_float()).cloned().unwrap_or(ValueType::F64);
                    self.own(w, &origin, ty, &operand);
                    if args.len() != 1 {
                        self.wrong(w, origin.clone(), format!("a math function takes 1 argument, found {}", args.len()));
                    }
                    for a in args {
                        self.want(w, *a, &operand, "argument of a math function");
                    }
                }
                next.extend(args.iter().map(|a| Task::Expr(*a, ctx)));
            }
            // `table_model` (§22.2.15): an `f64` read at an `f64`, of a table of at least two samples whose abscissae
            // strictly increase (elaboration reports `E0704`; the IR is held to the same rule).
            Expr::Table { x, table, .. } | Expr::TableSlope { x, table, .. } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    self.want(w, *x, &ValueType::F64, "argument of a table model");
                    if table.len() < 2 || table.windows(2).any(|pair| pair[0].0.get() >= pair[1].0.get()) {
                        self.wrong(w, origin.clone(), "a table model needs two or more samples with strictly increasing abscissae".to_string());
                    }
                }
                next.push(Task::Expr(*x, ctx));
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
                next.push(Task::Expr(*body, Ctx { closure: true, enclosing_loop: None, ..ctx }));
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
                        match t {
                            Terminal::Node(i) if w.owner.in_body && *i as usize >= w.owner.nodes => {
                                self.wrong(w, origin.clone(), format!("probe terminal node {i} out of range ({} nodes)", w.owner.nodes));
                            }
                            Terminal::Own(i) => self.wrong(w, origin.clone(), format!("probe terminal own{i}: an own terminal belongs in a child's node slot only")),
                            Terminal::Node(_) | Terminal::Ground => {}
                        }
                    }
                }
            }
            Expr::Flow { branch } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    if !w.owner.in_body {
                        self.wrong(w, origin.clone(), "a flow read outside a body".to_string());
                    } else if *branch as usize >= w.owner.branches.len() {
                        self.wrong(w, origin.clone(), format!("flow of branch {branch}, out of range ({} branches)", w.owner.branches.len()));
                    } else if !w.owner.driven.contains(branch) {
                        self.wrong(w, origin.clone(), format!("flow of branch {branch}, which the body does not drive with a potential contribution"));
                    }
                }
            }
            Expr::Env(env) => {
                if first {
                    let want = if matches!(env, Env::Static | Env::Analysis(_)) { ValueType::Bool } else { ValueType::F64 };
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
            Expr::FloatBits(bits) => {
                if first {
                    self.float_literal(w, &origin, ty, *bits);
                }
            }
            Expr::Wide(id) => {
                if first {
                    self.wide_literal(w, &origin, ty, *id);
                }
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
            Expr::Same { a, b } => {
                if first {
                    let want = match (self.ty(*a), self.ty(*b)) {
                        (Some(a), Some(b)) => Typing::same(a, b),
                        _ => None,
                    };
                    self.typed(w, &origin, ty, want, "`same`");
                }
                next.push(Task::Expr(*a, ctx));
                next.push(Task::Expr(*b, ctx));
            }
            Expr::LevelCast { expr, to, .. } => {
                if first {
                    let want = self.ty(*expr).and_then(|t| Typing::level_cast(t, to, &self.ir.types));
                    self.typed(w, &origin, ty, want, "`cast`");
                }
                next.push(Task::Expr(*expr, ctx));
            }
            Expr::ParamPrevious(p) => {
                if first {
                    if !w.owner.in_body {
                        self.wrong(w, origin, "the previous value of a param outside a body".to_string());
                    } else {
                        match w.owner.params.get(*p as usize) {
                            // A param's previous value has its type.
                            Some(param) => self.own(w, &origin, ty, &param.clone()),
                            None => self.wrong(w, origin, format!("previous value of param {p}, out of range ({} params)", w.owner.params.len())),
                        }
                    }
                }
            }
            Expr::Derivative(k) => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    if !w.owner.in_body {
                        self.wrong(w, origin, "a derivative read outside a body".to_string());
                    } else if *k as usize >= w.owner.derivatives {
                        self.wrong(w, origin, format!("derivative read of site {k}, out of range ({} sites)", w.owner.derivatives));
                    }
                }
            }
            Expr::Delay(k) => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    if !w.owner.in_body {
                        self.wrong(w, origin, "a delay read outside a body".to_string());
                    } else if *k as usize >= w.owner.delays {
                        self.wrong(w, origin, format!("delay read of site {k}, out of range ({} sites)", w.owner.delays));
                    }
                }
            }
            Expr::Noise(k) => {
                if first {
                    self.own(w, &origin, ty, &ValueType::F64);
                    if !w.owner.in_body {
                        self.wrong(w, origin, "a noise read outside a body".to_string());
                    } else if *k as usize >= w.owner.noise_sources {
                        self.wrong(w, origin, format!("noise read of source {k}, out of range ({} sources)", w.owner.noise_sources));
                    }
                }
            }
            Expr::SessionParameter { parameter } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::Option(Box::new(ValueType::F64)));
                    if !w.owner.in_body {
                        self.wrong(w, origin, "a session parameter read outside a body".to_string());
                    } else if *parameter as usize >= w.owner.session_parameters {
                        self.wrong(w, origin, format!("session parameter read {parameter}, out of range ({} parameters)", w.owner.session_parameters));
                    }
                }
            }
            Expr::Fired(e) => {
                if first {
                    self.own(w, &origin, ty, &ValueType::Bool);
                    if !w.owner.reaction && w.owner.event.is_none() {
                        self.wrong(w, origin, "`fired` outside a reaction or the guard of an event".to_string());
                    } else if w.owner.event == Some(*e) {
                        self.wrong(w, origin, format!("the guard of event {e} reads its own `fired`"));
                    } else if *e as usize >= w.owner.events {
                        self.wrong(w, origin, format!("`fired` of event {e}, out of range ({} events)", w.owner.events));
                    }
                }
            }
            Expr::Unwrap { is, value, .. } => {
                if first {
                    self.want(w, *is, &ValueType::Bool, "the presence flag of an `unwrap`");
                    if let Some(value_ty) = self.ty(*value).cloned() {
                        self.own(w, &origin, ty, &value_ty);
                    }
                }
                next.push(Task::Expr(*is, ctx));
                next.push(Task::Expr(*value, ctx));
            }
            Expr::Carrier(i) => {
                if first {
                    if !w.owner.in_body {
                        self.wrong(w, origin, "carrier read outside a body".to_string());
                    } else {
                        match w.owner.carriers.get(*i as usize) {
                            // an array of carriers read whole is the array of its elements' values (CV-24)
                            Some(c) => match w.owner.counts.get(*i as usize).copied().flatten() {
                                Some(count) => self.own(w, &origin, ty, &ValueType::Array(Box::new(c.clone()), count)),
                                None => self.own(w, &origin, ty, &Typing::read_of(c)),
                            },
                            None => self.wrong(w, origin, format!("carrier {i} out of range ({} carriers)", w.owner.carriers.len())),
                        }
                    }
                }
            }
            Expr::Others(i) => {
                if first {
                    match w.owner.carriers.get(*i as usize) {
                        _ if !w.owner.in_body => self.wrong(w, origin, "`others()` outside a body".to_string()),
                        Some(held) if w.owner.drive_types.get(*i as usize).is_some_and(Option::is_some) => self.own(w, &origin, ty, held),
                        Some(_) => self.wrong(w, origin, format!("`others()` of carrier {i}, which is not a resolved carrier")),
                        None => self.wrong(w, origin, format!("`others()` of carrier {i}, out of range ({} carriers)", w.owner.carriers.len())),
                    }
                }
            }
            Expr::Resolve { by_count, drives, except } => {
                if first {
                    self.resolve_form(w, &origin, ty, by_count, drives, *except);
                }
                w.calls.extend(by_count.iter().copied().filter(|f| f.index() < self.ir.functions.len()));
            }
            Expr::Previous(i) => {
                if first {
                    if !w.owner.in_body {
                        self.wrong(w, origin, "a previous-value read outside a body".to_string());
                    } else {
                        match w.owner.carriers.get(*i as usize) {
                            Some(_) if w.owner.counts.get(*i as usize).copied().flatten().is_some() => {
                                self.wrong(w, origin, format!("previous value of carrier {i}, an array of carriers: read one element's"));
                            }
                            Some(c) => self.own(w, &origin, ty, &Typing::read_of(c)),
                            None => self.wrong(w, origin, format!("previous value of carrier {i}, out of range ({} carriers)", w.owner.carriers.len())),
                        }
                    }
                }
            }
            Expr::CarrierElement { carrier, index } => {
                if first {
                    self.element_read(w, &origin, "an element read", *carrier, *index, ty, false);
                }
                next.push(Task::Expr(*index, ctx));
            }
            Expr::ElementPrevious { carrier, index } => {
                if first {
                    self.element_read(w, &origin, "an element's previous value", *carrier, *index, ty, false);
                }
                next.push(Task::Expr(*index, ctx));
            }
            Expr::ElementChanged { carrier, index } => {
                if first {
                    self.element_read(w, &origin, "an element's `changed()`", *carrier, *index, ty, true);
                }
                next.push(Task::Expr(*index, ctx));
            }
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } => self.looping(w, id, first, ctx, next),
            Expr::Break(value) => {
                self.leave_loop(w, &origin, ctx, *value, true);
                next.extend(value.iter().map(|v| Task::Expr(*v, ctx)));
            }
            Expr::Continue => self.leave_loop(w, &origin, ctx, None, false),
            Expr::Return(value) => {
                if w.owner.in_body {
                    self.wrong(w, origin.clone(), "`return` outside a function".to_string());
                } else if ctx.closure {
                    self.wrong(w, origin.clone(), "`return` inside a closure".to_string());
                } else {
                    let result = w.owner.result.clone();
                    match value {
                        Some(v) => self.want(w, *v, &result, "the value of a `return`"),
                        None if result != ValueType::Unit => self.wrong(w, origin.clone(), format!("`return` without a value in a function that returns {result}")),
                        None => {}
                    }
                }
                next.extend(value.iter().map(|v| Task::Expr(*v, ctx)));
            }
            Expr::FunctionRef(function) => {
                if let Some(f) = self.ir.functions.get(function.index()) {
                    w.calls.insert(*function);
                    if first {
                        let want = ValueType::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone()));
                        self.own(w, &origin, ty, &want);
                    }
                }
            }
            Expr::CallValue { callee, args } => {
                if first {
                    match self.ty(*callee) {
                        Some(ValueType::Fn(params, ret)) => {
                            self.own(w, &origin, ty, ret);
                            if params.len() != args.len() {
                                self.wrong(w, origin.clone(), format!("calls a function value with {} args, it takes {}", args.len(), params.len()));
                            }
                            for (a, p) in args.iter().zip(params) {
                                self.want(w, *a, p, "argument of a call through a function value");
                            }
                        }
                        Some(other) => self.wrong(w, origin.clone(), format!("the callee of a call through a value must have a function type, found {other}")),
                        None => {}
                    }
                }
                next.push(Task::Expr(*callee, ctx));
                next.extend(args.iter().map(|a| Task::Expr(*a, ctx)));
            }
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_)=> {
                self.aggregate_form(w, id, first, ctx, next);
            }
        }
        next.push(Task::Exit(id));
    }

    /// An assignment `place op= value` (§1.5.12): the target is a place of a `mutable` local, the
    /// value has the target's type, and `op`, when given, is a compound operator the type has.
    fn assignment(&mut self, w: &Walk, origin: &Origin, place: &Place, op: Option<BinaryOp>, value: ExprId) {
        let Some(target) = w.owner.locals.get(place.local as usize).cloned() else {
            self.wrong(w, origin.clone(), format!("assignment to local {}, out of range ({} locals)", place.local, w.owner.locals.len()));
            return;
        };
        if !w.owner.mutable.get(place.local as usize).copied().unwrap_or(false) {
            self.wrong(w, origin.clone(), format!("assignment to local {}, which is not `mutable`", place.local));
        }
        let Some(target) = self.place_type(w, origin, place, target) else { return };
        match op {
            None => self.want(w, value, &target, "the value of an assignment"),
            Some(op) => {
                let result = self.ty(value).and_then(|v| Typing::compound(op, &target, v));
                if self.ty(value).is_some() && result.as_ref() != Some(&target) {
                    self.wrong(w, origin.clone(), format!("`{op:?}` does not apply to a {target} target and this value"));
                }
            }
        }
    }

    /// The walk of the loop expression `id`: the checks of its own form (when `first`), then the
    /// tasks of its operands and its body, which run inside it (`break` and `continue` look the
    /// loop up through `Ctx::enclosing_loop`).
    fn looping(&mut self, w: &mut Walk, id: ExprId, first: bool, ctx: Ctx, next: &mut Vec<Task>) {
        let Some(data) = self.ir.exprs.get(id.index()) else { return };
        let (kind, ty, origin) = (&data.kind, &data.ty, data.origin.clone());
        let inside = Ctx { conditional: true, enclosing_loop: Some(id), ..ctx };
        let outside = w.bound.clone();
        match kind {
            Expr::While { cond, .. } => {
                if first {
                    self.carried(w, *cond, &ValueType::Bool, "the condition of a `while`");
                    self.own(w, &origin, ty, &ValueType::Unit);
                }
                next.push(Task::Expr(*cond, ctx));
            }
            Expr::ForRange { local, low, high, .. } => {
                if first {
                    self.own(w, &origin, ty, &ValueType::Unit);
                    if let (Some(l), Some(h)) = (self.ty(*low), self.ty(*high)) {
                        if l != h || !l.is_integer() {
                            self.wrong(w, origin.clone(), format!("the bounds of a `for` range must have one integer type, found {l} and {h}"));
                        } else if w.owner.locals.get(*local as usize) != Some(l) {
                            self.wrong(w, origin.clone(), format!("the variable of a `for` range must be a local of type {l}"));
                        }
                    }
                }
                next.extend([Task::Expr(*low, ctx), Task::Expr(*high, ctx), Task::Bind(*local)]);
            }
            _ => {}
        }
        next.extend(kind.statements().iter().map(|&s| Task::Stmt(s, inside, false)));
        // The body's `let`s, and a range's variable, end with the loop.
        next.push(Task::Restore(outside));
    }

    /// A `break` (`is_break`, with `value` when it carries one) or a `continue` at `origin`: it
    /// must sit in a loop, and a `break` value only out of a `loop`, of the type the loop gives.
    fn leave_loop(&mut self, w: &Walk, origin: &Origin, ctx: Ctx, value: Option<ExprId>, is_break: bool) {
        let word = if is_break { "break" } else { "continue" };
        let Some(enclosing) = ctx.enclosing_loop else {
            self.wrong(w, origin.clone(), format!("`{word}` outside a loop"));
            return;
        };
        if !is_break {
            return;
        }
        let loop_ty = self.ty(enclosing).cloned();
        match (self.ir.exprs.get(enclosing.index()).map(|d| &d.kind), value, loop_ty) {
            (Some(Expr::Loop { .. }), Some(v), Some(ty)) => self.want(w, v, &ty, "the value of a `break`"),
            (Some(Expr::Loop { .. }), None, Some(ty)) if ty != ValueType::Unit => {
                self.wrong(w, origin.clone(), format!("`break` without a value out of a `loop` of type {ty}"));
            }
            (Some(Expr::Loop { .. }), ..) => {}
            (_, Some(_), _) => self.wrong(w, origin.clone(), "`break` with a value out of a loop that is not `loop`".to_string()),
            _ => {}
        }
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
        if !w.owner.analog && !(kind == "ac_stim" && w.owner.value_input) {
            self.wrong(w, origin.clone(), format!("{kind} outside the analog block of a body"));
        } else if ctx.closure {
            self.wrong(w, origin.clone(), format!("{kind} inside a closure"));
        }
        if site as usize >= count {
            self.wrong(w, origin.clone(), format!("{kind} site {site} out of range ({count} sites)"));
            return;
        }
        // An `ac_stim` may stand under a condition: its terms in an arm not taken read no
        // stimulus. A `limit` under a condition says when it runs, through `enabled`; one at the
        // top has none (analog-structure, "Guarded core sites").
        if kind == "limit" && w.owner.analog && !ctx.closure {
            match (ctx.conditional, w.owner.limit_guards[site as usize]) {
                (true, false) => self.wrong(w, origin.clone(), format!("limit site {site} under an `if` or a short-circuit operator has no `enabled`")),
                (false, true) => self.wrong(w, origin.clone(), format!("limit site {site} has an `enabled` but is read outside every condition")),
                (true, true) | (false, false) => {}
            }
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
        Ctx { conditional: false, closure: false, enclosing_loop: None }
    }
}
