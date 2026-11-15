//! The one-way printer: `impl Display for Ir`. The text is deterministic and shows every body,
//! function, instance and node with every expression and statement reachable from them. Nothing
//! reads it back.
//!
//! The printer never panics on a broken IR: an id out of range prints as `<bad ...>`, which
//! `Ir::check` reports properly.

use std::fmt::{self, Display, Write};

use crate::ids::{ExprId, StmtId};
use crate::model::{
    BinaryOp, BodyData, BodyNode, BranchKind, Constant, Env, Expr, Ir, Local, MathFn, NodeRef,
    Operator, Origin, Param, Quantity, QuantityRole, Stmt, Terminal, UnaryOp, ValueType,
};

impl Display for Ir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut printer = Printer { ir: self, out: String::new() };
        printer.ir();
        f.write_str(&printer.out)
    }
}

impl Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file, self.line, self.col)
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
            Constant::Real(r) => write!(f, "{:?}", r.get()),
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

struct Printer<'a> {
    ir: &'a Ir,
    out: String,
}

impl<'a> Printer<'a> {
    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn ir(&mut self) {
        for nature in &self.ir.natures {
            self.line(0, &format!("nature {}", nature.name));
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
            let nature = self.ir.natures.get(node.nature.index()).map_or("<bad nature>", |n| n.name.as_str());
            let declarer = self.ir.instances.get(node.declared_by.index()).map_or("<bad instance>", |i| i.path.as_str());
            let init = node.init.map_or(String::new(), |r| format!("  init {:?}", r.get()));
            self.line(0, &format!("node {}: {nature}  declared by {declarer}{init}  @{}", node.path, node.origin));
        }
    }

    fn quantity(q: &Quantity) -> String {
        format!("{} {} {:?} abstol {:?}", q.name, q.access, q.units, q.abstol.get())
    }

    fn body(&mut self, body: &BodyData) {
        self.line(0, &format!("body {}  @{}", body.name, body.origin));
        let scope = Scope { locals: &body.locals, params: &body.params, nodes: &body.nodes };
        for param in &body.params {
            let mut text = format!("param {}: {}", param.name, param.ty);
            if let Some(default) = &param.default {
                let _ = write!(text, " = {default}");
            }
            if let Some(check) = param.check {
                let _ = write!(text, " where {}", self.expr(scope, check));
            }
            if param.structural {
                text.push_str(" structural");
            }
            let _ = write!(text, "  @{}", param.origin);
            self.line(1, &text);
        }
        for node in &body.nodes {
            let nature = self.ir.natures.get(node.nature.index()).map_or("<bad nature>", |n| n.name.as_str());
            let role = if node.role == crate::model::NodeRole::Port { "port" } else { "signal" };
            let init = node.init.map_or(String::new(), |r| format!(" init {:?}", r.get()));
            self.line(1, &format!("node {}: {nature} {role}{init}  @{}", node.name, node.origin));
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
            self.line(1, &format!("local {}: {}", local.name, local.ty));
        }
        for child in &body.children {
            let callee = self.ir.bodies.get(child.body.index());
            let name = callee.map_or("<bad body>", |b| b.name.as_str());
            let nodes: Vec<String> = child.nodes.iter().map(|t| Printer::terminal(scope, *t)).collect();
            let params: Vec<String> = child
                .params
                .iter()
                .map(|(i, v)| {
                    let pname = callee.and_then(|b| b.params.get(*i as usize)).map_or("<bad param>", |p| p.name.as_str());
                    format!("{pname} = {v}")
                })
                .collect();
            self.line(
                1,
                &format!("child {}: {name}  nodes ({})  params ({})  @{}", child.name, nodes.join(", "), params.join(", "), child.origin),
            );
        }
        if let Some(analog) = &body.analog {
            self.line(1, "analog");
            for &stmt in analog {
                let text = self.stmt(scope, stmt);
                self.line(2, &text);
            }
        }
    }

    fn function(&mut self, function: &crate::model::FunctionData) {
        let params: Vec<String> = function.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
        self.line(0, &format!("fn {}({}) -> {}  @{}", function.name, params.join(", "), function.ret, function.origin));
        for local in function.locals.iter().skip(function.params.len()) {
            self.line(1, &format!("local {}: {}", local.name, local.ty));
        }
        let scope = Scope { locals: &function.locals, params: &[], nodes: &[] };
        match self.ir.exprs.get(function.body.index()).map(|e| &e.kind) {
            Some(Expr::Block { stmts, value }) => {
                for &stmt in stmts {
                    let text = self.stmt(scope, stmt);
                    self.line(1, &text);
                }
                if let Some(value) = value {
                    let text = self.expr(scope, *value);
                    self.line(1, &text);
                }
            }
            _ => {
                let text = self.expr(scope, function.body);
                self.line(1, &text);
            }
        }
    }

    fn instance(&mut self, index: usize, instance: &crate::model::InstanceData) {
        let body = self.ir.bodies.get(instance.body.index());
        let body_name = body.map_or("<bad body>", |b| b.name.as_str());
        let nodes: Vec<String> = instance
            .nodes
            .iter()
            .map(|n| match n {
                NodeRef::Ground => "gnd".to_string(),
                NodeRef::Node(id) => self.ir.nodes.get(id.index()).map_or(format!("<bad node {}>", id.0), |n| n.path.clone()),
            })
            .collect();
        let params: Vec<String> = instance
            .params
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let pname = body.and_then(|b| b.params.get(i)).map_or("<bad param>", |p| p.name.as_str());
                format!("{pname} = {v}")
            })
            .collect();
        let top = if self.ir.top.index() == index { "  top" } else { "" };
        self.line(
            0,
            &format!(
                "instance {}: {body_name}  nodes ({})  params ({}){top}  @{}",
                instance.path,
                nodes.join(", "),
                params.join(", "),
                instance.origin
            ),
        );
    }

    fn terminal(scope: Scope<'_>, t: Terminal) -> String {
        match t {
            Terminal::Ground => "gnd".to_string(),
            Terminal::Node(i) => scope.nodes.get(i as usize).map_or(format!("<bad node {i}>"), |n| n.name.clone()),
        }
    }

    fn stmt(&self, scope: Scope<'_>, id: StmtId) -> String {
        let Some(stmt) = self.ir.stmts.get(id.index()) else {
            return format!("<bad stmt {}>", id.0);
        };
        match stmt {
            Stmt::Let { local, init } => {
                format!("let {} = {}", Printer::local(scope, *local), self.expr(scope, *init))
            }
            Stmt::Expr(e) => self.expr(scope, *e),
            Stmt::Contribute { branch, quantity, addends } => {
                let q = if *quantity == QuantityRole::Flow { "flow" } else { "potential" };
                let parts: Vec<String> = addends
                    .iter()
                    .map(|a| {
                        let op = if a.operator == Operator::Direct { "direct" } else { "ddt" };
                        let neg = if a.negated { "neg " } else { "" };
                        format!("{neg}{op} {}", self.expr(scope, a.value))
                    })
                    .collect();
                format!("b{branch}.{q} +> {}", parts.join(", "))
            }
        }
    }

    fn local(scope: Scope<'_>, i: u32) -> String {
        scope.locals.get(i as usize).map_or(format!("<bad local {i}>"), |l| l.name.clone())
    }

    fn expr(&self, scope: Scope<'_>, id: ExprId) -> String {
        let Some(data) = self.ir.exprs.get(id.index()) else {
            return format!("<bad expr {}>", id.0);
        };
        match &data.kind {
            Expr::Real(r) => format!("{:?}", r.get()),
            Expr::Bool(b) => b.to_string(),
            Expr::Unit => "()".to_string(),
            Expr::Local(i) => Printer::local(scope, *i),
            Expr::Param(i) => scope.params.get(*i as usize).map_or(format!("<bad param {i}>"), |p| p.name.clone()),
            Expr::Unary { op, expr } => {
                let sym = if *op == UnaryOp::Neg { "-" } else { "!" };
                format!("({sym}{})", self.expr(scope, *expr))
            }
            Expr::Binary { op, lhs, rhs } => {
                format!("({} {} {})", self.expr(scope, *lhs), Printer::binary_op(*op), self.expr(scope, *rhs))
            }
            Expr::If { cond, then, otherwise } => format!(
                "if {} {} else {}",
                self.expr(scope, *cond),
                self.arm(scope, *then),
                self.arm(scope, *otherwise)
            ),
            Expr::Block { stmts, value } => {
                let mut parts: Vec<String> = stmts.iter().map(|s| self.stmt(scope, *s)).collect();
                if let Some(v) = value {
                    parts.push(self.expr(scope, *v));
                }
                format!("{{ {} }}", parts.join("; "))
            }
            Expr::Call { function, args } => {
                let name = self.ir.functions.get(function.index()).map_or("<bad function>", |f| f.name.as_str());
                format!("{name}({})", self.list(scope, args))
            }
            Expr::Math { f, args } => format!("{}({})", Printer::math_name(*f), self.list(scope, args)),
            Expr::Closure { params, body } => {
                let names: Vec<String> = params.iter().map(|p| Printer::local(scope, *p)).collect();
                format!("|{}| {}", names.join(", "), self.expr(scope, *body))
            }
            Expr::Probe { plus, minus } => {
                format!("probe({}, {})", Printer::terminal(scope, *plus), Printer::terminal(scope, *minus))
            }
            Expr::Env(env) => match env {
                Env::Temperature => "temperature()".to_string(),
                Env::Gmin => "gmin()".to_string(),
                Env::SourceScale => "source_scale()".to_string(),
            },
            Expr::Limit { site, x, f, seed } => {
                let seed = seed.map_or(String::new(), |s| format!(", seed {}", self.expr(scope, s)));
                format!("limit L{site} ({}, {}{seed})", self.expr(scope, *x), self.expr(scope, *f))
            }
            Expr::Stim { site, mag, phase } => {
                let phase = phase.map_or(String::new(), |p| format!(", phase {}", self.expr(scope, p)));
                format!("ac_stim S{site} ({}{phase})", self.expr(scope, *mag))
            }
        }
    }

    /// An arm of an `if`: a block already carries its braces, any other expression gets them.
    fn arm(&self, scope: Scope<'_>, id: ExprId) -> String {
        let text = self.expr(scope, id);
        match self.ir.exprs.get(id.index()).map(|e| &e.kind) {
            Some(Expr::Block { .. }) => text,
            _ => format!("{{ {text} }}"),
        }
    }

    fn list(&self, scope: Scope<'_>, args: &[ExprId]) -> String {
        let parts: Vec<String> = args.iter().map(|a| self.expr(scope, *a)).collect();
        parts.join(", ")
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

    fn math_name(f: MathFn) -> &'static str {
        match f {
            MathFn::Exp => "exp",
            MathFn::Ln => "ln",
            MathFn::Sqrt => "sqrt",
            MathFn::Abs => "abs",
        }
    }
}
