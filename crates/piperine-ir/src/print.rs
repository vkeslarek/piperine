//! The one-way printer: `impl Display for Ir`. The text is deterministic and nothing reads it
//! back. It has two parts:
//!
//! - the **tree**: every nature, body, function, instance and node, with every expression and
//!   statement reachable from them written out as source-like text, for a person to read;
//! - the **arenas**: one line per expression and statement (its id, type, form by ids and
//!   origin) and the id-level references of bodies, functions, instances and nodes, which the
//!   tree shows by name.
//!
//! Together they write every field `Ir`'s equality compares, so two unequal IRs print different
//! texts (IR-04 AC 4). A real prints as Rust's shortest round-trip form, a NaN by its bits; a
//! name prints verbatim when it is made of letters, digits, `_`, `.` and `$`, quoted and escaped
//! otherwise, so no name can pass for another line.
//!
//! The printer never panics on a broken IR and always terminates: an id out of range prints as
//! `<bad ...>`, an expression met again while it is being printed as `<cycle eN>`, and an
//! expression already printed in the tree as `eN`, its arena line. The walk is iterative, so
//! nesting depth costs no native stack.

use std::collections::HashSet;
use std::fmt::{self, Display, Write};

use crate::ids::{ExprId, StmtId};
use crate::model::{
    BinaryOp, BodyData, BodyNode, BranchKind, Constant, Env, Expr, FunctionData, InstanceData, Ir,
    Local, MathFn, NodeRef, Operator, Origin, Param, Quantity, QuantityRole, Stmt, Terminal,
    UnaryOp, ValueType,
};
use crate::real::Real;

impl Display for Ir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut printer = Printer { ir: self, out: String::new(), printed: HashSet::new(), on_stack: HashSet::new() };
        printer.ir();
        f.write_str(&printer.out)
    }
}

impl Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", Printer::name(&self.file), self.line, self.col)
    }
}

impl Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::F64 => f.write_str("f64"),
            ValueType::Bool => f.write_str("bool"),
            ValueType::Unit => f.write_str("()"),
            ValueType::Fn(params, ret) => {
                let params: Vec<String> = params.iter().map(ToString::to_string).collect();
                write!(f, "fn({}) -> {ret}", params.join(", "))
            }
        }
    }
}

impl Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Real(r) => f.write_str(&Printer::real(*r)),
            Constant::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// What names the locals, params and terminals of the code being printed.
#[derive(Clone, Copy)]
struct Scope<'a> {
    locals: &'a [Local],
    params: &'a [Param],
    nodes: &'a [BodyNode],
}

/// One step of the iterative tree walk.
enum Piece {
    Text(String),
    Expr(ExprId),
    /// An arm of an `if`: a block carries its own braces, any other expression gets them.
    Arm(ExprId),
    Stmt(StmtId),
    /// The expression's operands are printed: it leaves the stack.
    Exit(ExprId),
}

struct Printer<'a> {
    ir: &'a Ir,
    out: String,
    /// Expressions already written out in the tree; a later use prints `eN`.
    printed: HashSet<ExprId>,
    /// Expressions being written out; meeting one again is a cycle.
    on_stack: HashSet<ExprId>,
}

impl<'a> Printer<'a> {
    /// A real as its shortest round-trip form; a NaN, whose payload that form drops, by its bits.
    fn real(r: Real) -> String {
        let v = r.get();
        if v.is_nan() { format!("NaN(0x{:016x})", r.bits()) } else { format!("{v:?}") }
    }

    /// A name verbatim when it is made of letters, digits, `_`, `.` and `$`; quoted otherwise.
    fn name(s: &str) -> String {
        let plain = !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$'));
        if plain { s.to_string() } else { format!("{s:?}") }
    }

    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn ir(&mut self) {
        for nature in &self.ir.natures {
            self.line(0, &format!("nature {}", Printer::name(&nature.name)));
            self.line(1, &format!("potential {}", Printer::quantity(&nature.potential)));
            if let Some(flow) = &nature.flow {
                self.line(1, &format!("flow {}", Printer::quantity(flow)));
            }
        }
        for body in &self.ir.bodies {
            self.body(body);
        }
        for function in &self.ir.functions {
            self.function(function);
        }
        for (i, instance) in self.ir.instances.iter().enumerate() {
            self.instance(i, instance);
        }
        for node in &self.ir.nodes {
            let nature = self.nature_name(node.nature.index());
            let declarer = self.ir.instances.get(node.declared_by.index()).map_or("<bad instance>".to_string(), |i| Printer::name(&i.path));
            let init = node.init.map_or(String::new(), |r| format!("  init {}", Printer::real(r)));
            self.line(0, &format!("node {}: {nature}  declared by {declarer}{init}  @{}", Printer::name(&node.path), node.origin));
        }
        self.arenas();
    }

    fn nature_name(&self, i: usize) -> String {
        self.ir.natures.get(i).map_or("<bad nature>".to_string(), |n| Printer::name(&n.name))
    }

    fn quantity(q: &Quantity) -> String {
        format!("{} {} {:?} abstol {}", Printer::name(&q.name), Printer::name(&q.access), q.units, Printer::real(q.abstol))
    }

    fn body(&mut self, body: &BodyData) {
        self.line(0, &format!("body {}  @{}", Printer::name(&body.name), body.origin));
        let scope = Scope { locals: &body.locals, params: &body.params, nodes: &body.nodes };
        for param in &body.params {
            let mut text = format!("param {}: {}", Printer::name(&param.name), param.ty);
            if let Some(default) = &param.default {
                let _ = write!(text, " = {default}");
            }
            if let Some(check) = param.check {
                let _ = write!(text, " where {}", self.render(scope, Piece::Expr(check)));
            }
            if param.structural {
                text.push_str(" structural");
            }
            let _ = write!(text, "  @{}", param.origin);
            self.line(1, &text);
        }
        for node in &body.nodes {
            let nature = self.nature_name(node.nature.index());
            let role = if node.role == crate::model::NodeRole::Port { "port" } else { "signal" };
            let init = node.init.map_or(String::new(), |r| format!(" init {}", Printer::real(r)));
            self.line(1, &format!("node {}: {nature} {role}{init}  @{}", Printer::name(&node.name), node.origin));
        }
        for (i, branch) in body.branches.iter().enumerate() {
            let kind = if branch.kind == BranchKind::Flow { "flow" } else { "potential" };
            let plus = Printer::terminal(scope, branch.plus);
            let minus = Printer::terminal(scope, branch.minus);
            self.line(1, &format!("branch b{i} ({plus}, {minus}) {kind}"));
        }
        for (i, site) in body.limits.iter().enumerate() {
            self.line(1, &format!("limit L{i} {:?}  @{}", site.name, site.origin));
        }
        for (i, site) in body.stims.iter().enumerate() {
            self.line(1, &format!("stim S{i} {:?}  @{}", site.name, site.origin));
        }
        for local in &body.locals {
            self.line(1, &format!("local {}: {}", Printer::name(&local.name), local.ty));
        }
        for child in &body.children {
            let callee = self.ir.bodies.get(child.body.index());
            let name = callee.map_or("<bad body>".to_string(), |b| Printer::name(&b.name));
            let nodes: Vec<String> = child.nodes.iter().map(|t| Printer::terminal(scope, *t)).collect();
            let params: Vec<String> = child
                .params
                .iter()
                .map(|(i, v)| {
                    let pname = callee.and_then(|b| b.params.get(*i as usize)).map_or("<bad param>".to_string(), |p| Printer::name(&p.name));
                    format!("{pname} = {v}")
                })
                .collect();
            self.line(
                1,
                &format!(
                    "child {}: {name}  nodes ({})  params ({})  @{}",
                    Printer::name(&child.name),
                    nodes.join(", "),
                    params.join(", "),
                    child.origin
                ),
            );
        }
        if let Some(analog) = &body.analog {
            self.line(1, "analog");
            for &stmt in analog {
                let text = self.render(scope, Piece::Stmt(stmt));
                self.line(2, &text);
            }
        }
    }

    fn function(&mut self, function: &FunctionData) {
        let params: Vec<String> = function.params.iter().map(|p| format!("{}: {}", Printer::name(&p.name), p.ty)).collect();
        self.line(
            0,
            &format!("fn {}({}) -> {}  @{}", Printer::name(&function.name), params.join(", "), function.ret, function.origin),
        );
        for local in &function.locals {
            self.line(1, &format!("local {}: {}", Printer::name(&local.name), local.ty));
        }
        let scope = Scope { locals: &function.locals, params: &[], nodes: &[] };
        let body = function.body;
        // A block body prints one statement per line; its id is printed and on the stack meanwhile.
        let fresh = !self.printed.contains(&body);
        match self.ir.exprs.get(body.index()).map(|e| &e.kind) {
            Some(Expr::Block { stmts, value }) if fresh => {
                self.printed.insert(body);
                self.on_stack.insert(body);
                for &stmt in stmts {
                    let text = self.render(scope, Piece::Stmt(stmt));
                    self.line(1, &text);
                }
                if let Some(value) = value {
                    let text = self.render(scope, Piece::Expr(*value));
                    self.line(1, &text);
                }
                self.on_stack.remove(&body);
            }
            _ => {
                let text = self.render(scope, Piece::Expr(body));
                self.line(1, &text);
            }
        }
    }

    fn instance(&mut self, index: usize, instance: &InstanceData) {
        let body = self.ir.bodies.get(instance.body.index());
        let body_name = body.map_or("<bad body>".to_string(), |b| Printer::name(&b.name));
        let nodes: Vec<String> = instance
            .nodes
            .iter()
            .map(|n| match n {
                NodeRef::Ground => "gnd".to_string(),
                NodeRef::Node(id) => self.ir.nodes.get(id.index()).map_or(format!("<bad node {}>", id.0), |n| Printer::name(&n.path)),
            })
            .collect();
        let params: Vec<String> = instance
            .params
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let pname = body.and_then(|b| b.params.get(i)).map_or("<bad param>".to_string(), |p| Printer::name(&p.name));
                format!("{pname} = {v}")
            })
            .collect();
        let top = if self.ir.top.index() == index { "  top" } else { "" };
        self.line(
            0,
            &format!(
                "instance {}: {body_name}  nodes ({})  params ({}){top}  @{}",
                Printer::name(&instance.path),
                nodes.join(", "),
                params.join(", "),
                instance.origin
            ),
        );
    }

    fn terminal(scope: Scope<'_>, t: Terminal) -> String {
        match t {
            Terminal::Ground => "gnd".to_string(),
            Terminal::Node(i) => scope.nodes.get(i as usize).map_or(format!("<bad node {i}>"), |n| Printer::name(&n.name)),
        }
    }

    fn local(scope: Scope<'_>, i: u32) -> String {
        scope.locals.get(i as usize).map_or(format!("<bad local {i}>"), |l| Printer::name(&l.name))
    }

    // ---- the tree of one statement or expression -------------------------------------------

    /// The text of `root`, written by an explicit stack.
    fn render(&mut self, scope: Scope<'_>, root: Piece) -> String {
        let mut text = String::new();
        let mut stack = vec![root];
        let mut next = Vec::new();
        while let Some(piece) = stack.pop() {
            match piece {
                Piece::Text(t) => text.push_str(&t),
                Piece::Exit(id) => {
                    self.on_stack.remove(&id);
                }
                Piece::Stmt(id) => self.stmt(scope, id, &mut next),
                Piece::Expr(id) => self.expr(scope, id, &mut next),
                Piece::Arm(id) => {
                    let block = matches!(self.ir.exprs.get(id.index()).map(|e| &e.kind), Some(Expr::Block { .. }));
                    if block && !self.printed.contains(&id) {
                        next.push(Piece::Expr(id));
                    } else {
                        next.extend([Piece::Text("{ ".into()), Piece::Expr(id), Piece::Text(" }".into())]);
                    }
                }
            }
            // `next` holds the pieces in the order they are written.
            stack.extend(next.drain(..).rev());
        }
        text
    }

    fn stmt(&self, scope: Scope<'_>, id: StmtId, next: &mut Vec<Piece>) {
        let Some(stmt) = self.ir.stmts.get(id.index()) else {
            next.push(Piece::Text(format!("<bad stmt {}>", id.0)));
            return;
        };
        match stmt {
            Stmt::Let { local, init } => {
                next.push(Piece::Text(format!("let {} = ", Printer::local(scope, *local))));
                next.push(Piece::Expr(*init));
            }
            Stmt::Expr(e) => next.push(Piece::Expr(*e)),
            Stmt::Contribute { branch, quantity, addends } => {
                let q = if *quantity == QuantityRole::Flow { "flow" } else { "potential" };
                next.push(Piece::Text(format!("b{branch}.{q} +> ")));
                for (i, a) in addends.iter().enumerate() {
                    let sep = if i == 0 { "" } else { ", " };
                    let op = if a.operator == Operator::Direct { "direct" } else { "ddt" };
                    let neg = if a.negated { "neg " } else { "" };
                    next.push(Piece::Text(format!("{sep}{neg}{op} ")));
                    next.push(Piece::Expr(a.value));
                }
            }
        }
    }

    /// `items` separated by `, `.
    fn list(next: &mut Vec<Piece>, items: &[ExprId]) {
        for (i, &item) in items.iter().enumerate() {
            if i > 0 {
                next.push(Piece::Text(", ".into()));
            }
            next.push(Piece::Expr(item));
        }
    }

    fn expr(&mut self, scope: Scope<'_>, id: ExprId, next: &mut Vec<Piece>) {
        let Some(data) = self.ir.exprs.get(id.index()) else {
            next.push(Piece::Text(format!("<bad expr {}>", id.0)));
            return;
        };
        if self.on_stack.contains(&id) {
            next.push(Piece::Text(format!("<cycle e{}>", id.0)));
            return;
        }
        if !self.printed.insert(id) {
            next.push(Piece::Text(format!("e{}", id.0)));
            return;
        }
        self.on_stack.insert(id);
        let text = |t: String| Piece::Text(t);
        match &data.kind {
            Expr::Real(r) => next.push(text(Printer::real(*r))),
            Expr::Bool(b) => next.push(text(b.to_string())),
            Expr::Unit => next.push(text("()".into())),
            Expr::Local(i) => next.push(text(Printer::local(scope, *i))),
            Expr::Param(i) => {
                next.push(text(scope.params.get(*i as usize).map_or(format!("<bad param {i}>"), |p| Printer::name(&p.name))))
            }
            Expr::Unary { op, expr } => {
                let sym = if *op == UnaryOp::Neg { "(-" } else { "(!" };
                next.extend([text(sym.into()), Piece::Expr(*expr), text(")".into())]);
            }
            Expr::Binary { op, lhs, rhs } => next.extend([
                text("(".into()),
                Piece::Expr(*lhs),
                text(format!(" {} ", Printer::binary_op(*op))),
                Piece::Expr(*rhs),
                text(")".into()),
            ]),
            Expr::If { cond, then, otherwise } => next.extend([
                text("if ".into()),
                Piece::Expr(*cond),
                text(" ".into()),
                Piece::Arm(*then),
                text(" else ".into()),
                Piece::Arm(*otherwise),
            ]),
            Expr::Block { stmts, value } => {
                next.push(text("{ ".into()));
                for (i, &s) in stmts.iter().enumerate() {
                    if i > 0 {
                        next.push(text("; ".into()));
                    }
                    next.push(Piece::Stmt(s));
                }
                if let Some(v) = value {
                    if !stmts.is_empty() {
                        next.push(text("; ".into()));
                    }
                    next.push(Piece::Expr(*v));
                }
                next.push(text(" }".into()));
            }
            Expr::Call { function, args } => {
                let name = self.ir.functions.get(function.index()).map_or("<bad function>".to_string(), |f| Printer::name(&f.name));
                next.push(text(format!("{name}(")));
                Printer::list(next, args);
                next.push(text(")".into()));
            }
            Expr::Math { f, args } => {
                next.push(text(format!("{}(", Printer::math_name(*f))));
                Printer::list(next, args);
                next.push(text(")".into()));
            }
            Expr::Closure { params, body } => {
                let names: Vec<String> = params.iter().map(|p| Printer::local(scope, *p)).collect();
                next.extend([text(format!("|{}| ", names.join(", "))), Piece::Expr(*body)]);
            }
            Expr::Probe { plus, minus } => {
                next.push(text(format!("probe({}, {})", Printer::terminal(scope, *plus), Printer::terminal(scope, *minus))))
            }
            Expr::Env(env) => next.push(text(format!("{}()", Printer::env_name(*env)))),
            Expr::Limit { site, x, f, seed } => {
                next.extend([text(format!("limit L{site} (")), Piece::Expr(*x), text(", ".into()), Piece::Expr(*f)]);
                if let Some(s) = seed {
                    next.extend([text(", seed ".into()), Piece::Expr(*s)]);
                }
                next.push(text(")".into()));
            }
            Expr::Stim { site, mag, phase } => {
                next.extend([text(format!("ac_stim S{site} (")), Piece::Expr(*mag)]);
                if let Some(p) = phase {
                    next.extend([text(", phase ".into()), Piece::Expr(*p)]);
                }
                next.push(text(")".into()));
            }
        }
        next.push(Piece::Exit(id));
    }

    // ---- the arenas -------------------------------------------------------------------------

    /// The id-level part of the text: what the tree shows by name or not at all.
    fn arenas(&mut self) {
        let ir = self.ir;
        self.line(0, "arenas");
        self.line(1, &format!("top {}", ir.top.0));
        let terminal = |t: &Terminal| match t {
            Terminal::Node(i) => i.to_string(),
            Terminal::Ground => "gnd".to_string(),
        };
        for (i, body) in ir.bodies.iter().enumerate() {
            let checks: Vec<String> = body.params.iter().map(|p| p.check.map_or("-".to_string(), |e| format!("e{}", e.0))).collect();
            let natures: Vec<String> = body.nodes.iter().map(|n| n.nature.0.to_string()).collect();
            let branches: Vec<String> = body.branches.iter().map(|b| format!("({}, {})", terminal(&b.plus), terminal(&b.minus))).collect();
            let children: Vec<String> = body
                .children
                .iter()
                .map(|c| {
                    let nodes: Vec<String> = c.nodes.iter().map(terminal).collect();
                    let params: Vec<String> = c.params.iter().map(|(p, _)| p.to_string()).collect();
                    format!("(body {} nodes ({}) params ({}))", c.body.0, nodes.join(", "), params.join(", "))
                })
                .collect();
            let analog = body.analog.as_ref().map_or("-".to_string(), |stmts| {
                let ids: Vec<String> = stmts.iter().map(|s| format!("s{}", s.0)).collect();
                format!("({})", ids.join(", "))
            });
            self.line(
                1,
                &format!(
                    "body {i}: where ({})  natures ({})  branches ({})  children ({})  analog {analog}",
                    checks.join(", "),
                    natures.join(", "),
                    branches.join(", "),
                    children.join(", ")
                ),
            );
        }
        for (i, function) in ir.functions.iter().enumerate() {
            self.line(1, &format!("fn {i}: body e{}", function.body.0));
        }
        for (i, instance) in ir.instances.iter().enumerate() {
            let parent = instance.parent.map_or("-".to_string(), |p| p.0.to_string());
            let children: Vec<String> = instance.children.iter().map(|c| c.0.to_string()).collect();
            let nodes: Vec<String> = instance
                .nodes
                .iter()
                .map(|n| match n {
                    NodeRef::Node(id) => id.0.to_string(),
                    NodeRef::Ground => "gnd".to_string(),
                })
                .collect();
            self.line(
                1,
                &format!(
                    "instance {i}: name {}  body {}  parent {parent}  children ({})  nodes ({})",
                    Printer::name(&instance.name),
                    instance.body.0,
                    children.join(", "),
                    nodes.join(", ")
                ),
            );
        }
        for (i, node) in ir.nodes.iter().enumerate() {
            self.line(1, &format!("node {i}: nature {}  declared by {}", node.nature.0, node.declared_by.0));
        }
        for (i, data) in ir.exprs.iter().enumerate() {
            self.line(1, &format!("e{i}: {} = {}  @{}", data.ty, Printer::form(&data.kind), data.origin));
        }
        for (i, stmt) in ir.stmts.iter().enumerate() {
            self.line(1, &format!("s{i}: {}", Printer::stmt_form(stmt)));
        }
    }

    /// An expression's form with its operands as ids.
    fn form(kind: &Expr) -> String {
        let e = |id: &ExprId| format!("e{}", id.0);
        let opt = |id: &Option<ExprId>| id.as_ref().map_or("-".to_string(), e);
        let ids = |list: &[ExprId]| list.iter().map(e).collect::<Vec<_>>().join(", ");
        let terminal = |t: &Terminal| match t {
            Terminal::Node(i) => format!("n{i}"),
            Terminal::Ground => "gnd".to_string(),
        };
        match kind {
            Expr::Real(r) => format!("real {}", Printer::real(*r)),
            Expr::Bool(b) => format!("bool {b}"),
            Expr::Unit => "unit".to_string(),
            Expr::Local(i) => format!("local {i}"),
            Expr::Param(i) => format!("param {i}"),
            Expr::Unary { op, expr } => format!("{} {}", if *op == UnaryOp::Neg { "neg" } else { "not" }, e(expr)),
            Expr::Binary { op, lhs, rhs } => format!("{} {} {}", Printer::binary_name(*op), e(lhs), e(rhs)),
            Expr::If { cond, then, otherwise } => format!("if {} {} {}", e(cond), e(then), e(otherwise)),
            Expr::Block { stmts, value } => {
                let stmts: Vec<String> = stmts.iter().map(|s| format!("s{}", s.0)).collect();
                format!("block ({}) {}", stmts.join(", "), opt(value))
            }
            Expr::Call { function, args } => format!("call fn {} ({})", function.0, ids(args)),
            Expr::Math { f, args } => format!("{} ({})", Printer::math_name(*f), ids(args)),
            Expr::Closure { params, body } => {
                let params: Vec<String> = params.iter().map(u32::to_string).collect();
                format!("closure ({}) {}", params.join(", "), e(body))
            }
            Expr::Probe { plus, minus } => format!("probe {} {}", terminal(plus), terminal(minus)),
            Expr::Env(env) => format!("env {}", Printer::env_name(*env)),
            Expr::Limit { site, x, f, seed } => format!("limit L{site} {} {} {}", e(x), e(f), opt(seed)),
            Expr::Stim { site, mag, phase } => format!("ac_stim S{site} {} {}", e(mag), opt(phase)),
        }
    }

    /// A statement's form with its operands as ids.
    fn stmt_form(stmt: &Stmt) -> String {
        match stmt {
            Stmt::Let { local, init } => format!("let {local} e{}", init.0),
            Stmt::Expr(e) => format!("expr e{}", e.0),
            Stmt::Contribute { branch, quantity, addends } => {
                let q = if *quantity == QuantityRole::Flow { "flow" } else { "potential" };
                let parts: Vec<String> = addends
                    .iter()
                    .map(|a| {
                        let op = if a.operator == Operator::Direct { "direct" } else { "ddt" };
                        let neg = if a.negated { "neg " } else { "" };
                        format!("{neg}{op} e{}", a.value.0)
                    })
                    .collect();
                format!("contribute b{branch} {q} ({})", parts.join(", "))
            }
        }
    }

    fn binary_op(op: BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }

    fn binary_name(op: BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "add",
            BinaryOp::Sub => "sub",
            BinaryOp::Mul => "mul",
            BinaryOp::Div => "div",
            BinaryOp::Lt => "lt",
            BinaryOp::Le => "le",
            BinaryOp::Gt => "gt",
            BinaryOp::Ge => "ge",
            BinaryOp::Eq => "eq",
            BinaryOp::Ne => "ne",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
        }
    }

    fn math_name(f: MathFn) -> &'static str {
        match f {
            MathFn::Exp => "exp",
            MathFn::Ln => "ln",
            MathFn::Sqrt => "sqrt",
            MathFn::Abs => "abs",
        }
    }

    fn env_name(env: Env) -> &'static str {
        match env {
            Env::Temperature => "temperature",
            Env::Gmin => "gmin",
            Env::SourceScale => "source_scale",
        }
    }
}
