//! The `arenas` section: one line per expression and statement, and the id-level references.

use crate::ids::{ExprId, StmtId};
use crate::model::{
    BinaryOp, BodyData, ChildValue, EmitField, Env, EventSource, Expr, LevelPart, MathFn, Method, NodeRef, Operator, Projection, QuantityRole, Stmt, Terminal, UnaryOp,
};
use crate::real::Real;

use super::Printer;

impl<'a> Printer<'a> {
    /// The id-level part of the text: what the tree shows by name or not at all.
    pub(super) fn arenas(&mut self) {
        self.line(0, "arenas");
        self.line(1, &format!("top {}", self.ir.top.0));
        self.bodies_arena();
        self.functions_arena();
        self.instances_arena();
        self.nodes_and_nets_arena();
        self.expressions_arena();
        self.statements_arena();
    }

    /// One `body` line per body, and its `discrete` line when it has drives, events or reactions.
    fn bodies_arena(&mut self) {
        let ir = self.ir;
        for (i, body) in ir.bodies.iter().enumerate() {
            self.body_arena_line(i, body);
            self.discrete_arena_line(i, body);
        }
    }

    /// The `body` line: where checks, node natures, branches, children and the analog block by id.
    fn body_arena_line(&mut self, i: usize, body: &BodyData) {
        let terminal = |t: &Terminal| match t {
            Terminal::Node(i) => i.to_string(),
            Terminal::Ground => "gnd".to_string(),
            Terminal::Own(k) => format!("own{k}"),
        };
            let checks: Vec<String> = body.params.iter().map(|p| p.check.map_or("-".to_string(), |e| format!("e{}", e.0))).collect();
            let natures: Vec<String> = body.nodes.iter().map(|n| n.nature.0.to_string()).collect();
            let branches: Vec<String> = body.branches.iter().map(|b| format!("({}, {})", terminal(&b.plus), terminal(&b.minus))).collect();
            let children: Vec<String> = body
                .children
                .iter()
                .enumerate()
                .map(|(ci, c)| {
                    let nodes: Vec<String> = c.nodes.iter().map(terminal).collect();
                    let params: Vec<String> = c.params.iter().map(|(p, _)| p.to_string()).collect();
                    let values: Vec<String> = body
                        .child_params
                        .get(ci)
                        .map_or(&[][..], Vec::as_slice)
                        .iter()
                        .map(|(p, v)| match v {
                            ChildValue::Const(k) => format!("p{p}={k}"),
                            ChildValue::Expr(e) => format!("p{p}=e{}", e.0),
                        })
                        .collect();
                    let values = if values.is_empty() { String::new() } else { format!(" values ({})", values.join(", ")) };
                    let bound = body.child_carriers.get(ci).map_or(&[][..], Vec::as_slice);
                    let carriers = if bound.is_empty() {
                        String::new()
                    } else {
                        let ids: Vec<String> = bound.iter().map(|c| c.map_or("-".to_string(), |c| format!("c{c}"))).collect();
                        format!(" carriers ({})", ids.join(", "))
                    };
                    format!("(body {} nodes ({}) params ({}){values}{carriers})", c.body.0, nodes.join(", "), params.join(", "))
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

    /// The `discrete` line of a body: its drives, events and reactions by id; none for a body with none.
    fn discrete_arena_line(&mut self, i: usize, body: &BodyData) {
            if !(body.drives.is_empty() && body.events.is_empty() && body.reactions.is_empty()) {
                let opt = |e: Option<ExprId>| e.map_or("-".to_string(), |a| format!("e{}", a.0));
                let drives: Vec<String> = body.drives.iter().map(|d| format!("(c{} e{} {}{})", d.carrier, d.value.0, opt(d.after), Printer::drive_target_key(d))).collect();
                let events: Vec<String> = body
                    .events
                    .iter()
                    .map(|ev| match &ev.source {
                        EventSource::Changed(c) => format!("(changed c{c} {})", opt(ev.guard)),
                        EventSource::ParamChanged(p) => format!("(param_changed p{p} {})", opt(ev.guard)),
                        EventSource::Cross { e, ttol: None, etol: None } => format!("(cross e{} {})", e.0, opt(ev.guard)),
                        EventSource::Cross { e, ttol, etol } => format!("(cross e{} {} ttol {} etol {})", e.0, opt(ev.guard), opt(*ttol), opt(*etol)),
                        EventSource::At(e) => format!("(at e{} {})", e.0, opt(ev.guard)),
                        EventSource::Settle => format!("(settle {})", opt(ev.guard)),
                        EventSource::ElementChanged { carrier, element } => format!("(element_changed c{carrier} {element} {})", opt(ev.guard)),
                    })
                    .collect();
                let reactions: Vec<String> = body
                    .reactions
                    .iter()
                    .map(|r| {
                        let on: Vec<String> = r.on.iter().map(u32::to_string).collect();
                        let stmts: Vec<String> = r.stmts.iter().map(|s| format!("s{}", s.0)).collect();
                        format!("(on ({}) ({}))", on.join(", "), stmts.join(", "))
                    })
                    .collect();
                self.line(
                    1,
                    &format!("body {i} discrete: drives ({})  events ({})  reactions ({})", drives.join(", "), events.join(", "), reactions.join(", ")),
                );
            }
    }

    /// One `fn` line per function.
    fn functions_arena(&mut self) {
        let ir = self.ir;
        for (i, function) in ir.functions.iter().enumerate() {
            self.line(1, &format!("fn {i}: body e{}", function.body.0));
        }
    }

    /// One `instance` line per instance.
    fn instances_arena(&mut self) {
        let ir = self.ir;
        for (i, instance) in ir.instances.iter().enumerate() {
            let parent = instance.parent.map_or("-".to_string(), |p| p.0.to_string());
            let children: Vec<String> = instance.children.iter().map(|c| c.0.to_string()).collect();
            let nets = if instance.nets.is_empty() {
                String::new()
            } else {
                let ids: Vec<String> = instance.nets.iter().map(|n| n.0.to_string()).collect();
                format!("  nets ({})", ids.join(", "))
            };
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
                    "instance {i}: name {}  body {}  parent {parent}  children ({})  nodes ({}){nets}",
                    Printer::name(&instance.name),
                    instance.body.0,
                    children.join(", "),
                    nodes.join(", ")
                ),
            );
        }
    }

    /// One `node` line per node and one `net` line per net.
    fn nodes_and_nets_arena(&mut self) {
        let ir = self.ir;
        for (i, node) in ir.nodes.iter().enumerate() {
            self.line(1, &format!("node {i}: nature {}  declared by {}", node.nature.0, node.declared_by.0));
        }
        for (i, net) in ir.nets.iter().enumerate() {
            self.line(1, &format!("net {i}: declared by {}", net.declared_by.0));
        }
    }

    /// One `e` line per expression: its type, form by ids and origin.
    fn expressions_arena(&mut self) {
        let ir = self.ir;
        for (i, data) in ir.exprs.iter().enumerate() {
            self.line(1, &format!("e{i}: {} = {}  @{}", data.ty, Printer::form(&data.kind), data.origin));
        }
    }

    /// One `s` line per statement.
    fn statements_arena(&mut self) {
        let ir = self.ir;
        for (i, stmt) in ir.stmts.iter().enumerate() {
            self.line(1, &format!("s{i}: {}", Printer::stmt_form(stmt)));
        }
    }

    /// An expression's form with its operands as ids.
    pub(super) fn form(kind: &Expr) -> String {
        let e = |id: &ExprId| format!("e{}", id.0);
        let opt = |id: &Option<ExprId>| id.as_ref().map_or("-".to_string(), e);
        let ids = |list: &[ExprId]| list.iter().map(e).collect::<Vec<_>>().join(", ");
        let terminal = |t: &Terminal| match t {
            Terminal::Node(i) => format!("n{i}"),
            Terminal::Ground => "gnd".to_string(),
            Terminal::Own(k) => format!("own{k}"),
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
            Expr::Table { x, table, control } => format!("table {} \"{}\" {}", e(x), control.text(), Printer::samples(table)),
            Expr::TableSlope { x, table, control } => format!("table_slope {} \"{}\" {}", e(x), control.text(), Printer::samples(table)),
            Expr::Closure { params, body } => {
                let params: Vec<String> = params.iter().map(u32::to_string).collect();
                format!("closure ({}) {}", params.join(", "), e(body))
            }
            Expr::Probe { plus, minus } => format!("probe {} {}", terminal(plus), terminal(minus)),
            Expr::Env(Env::Analysis(kind)) => format!("env analysis {kind:?}"),
            Expr::Env(env) => format!("env {}", Printer::env_name(*env)),
            Expr::Limit { site, x, f, seed } => format!("limit L{site} {} {} {}", e(x), e(f), opt(seed)),
            Expr::Stim { site, mag, phase } => format!("ac_stim S{site} {} {}", e(mag), opt(phase)),
            Expr::Int(v) => format!("int {v}"),
            Expr::FloatBits(bits) => format!("bits 0x{bits:x}"),
            Expr::Wide(id) => format!("wide {}", id.0),
            Expr::Cast { expr, to } => format!("cast {} to {to}", e(expr)),
            Expr::Method { method, receiver, args } => format!("{} {} ({})", Printer::method_name(*method), e(receiver), ids(args)),
            Expr::Carrier(i) => format!("carrier {i}"),
            Expr::Flow { branch } => format!("flow b{branch}"),
            Expr::Previous(i) => format!("previous {i}"),
            Expr::Others(i) => format!("others {i}"),
            Expr::Resolve { by_count, drives, except } => {
                let functions: Vec<String> = by_count.iter().map(|f| f.0.to_string()).collect();
                format!("resolve fns [{}] drives {drives:?} except {except:?}", functions.join(" "))
            }
            Expr::CarrierElement { carrier, index } => format!("carrier_element {carrier} {}", e(index)),
            Expr::ElementPrevious { carrier, index } => format!("element_previous {carrier} {}", e(index)),
            Expr::ElementChanged { carrier, index } => format!("element_changed {carrier} {}", e(index)),
            Expr::Derivative(k) => format!("derivative D{k}"),
            Expr::Delay(k) => format!("delay {k}"),
            Expr::Noise(k) => format!("noise {k}"),
            Expr::SessionParameter { parameter } => format!("simparam {parameter}"),
            Expr::ParamPrevious(i) => format!("param_previous {i}"),
            Expr::Fired(i) => format!("fired {i}"),
            Expr::Same { a, b } => format!("same {} {}", e(a), e(b)),
            Expr::LevelCast { expr, to, tags } => match tags {
                Some(tags) => format!("level_cast {} to {to} tags {tags:?}", e(expr)),
                None => format!("level_cast {} to {to}", e(expr)),
            },
            Expr::Unwrap { is, value, message } => match message {
                Some(message) => format!("expect {} {} {message:?}", e(is), e(value)),
                None => format!("unwrap {} {}", e(is), e(value)),
            },
            Expr::Loop { body } => format!("loop ({})", Printer::stmt_ids(body)),
            Expr::While { cond, body } => format!("while {} ({})", e(cond), Printer::stmt_ids(body)),
            Expr::ForRange { local, low, high, inclusive, reverse, body } => {
                let flags = format!("{}{}", if *inclusive { " inclusive" } else { "" }, if *reverse { " reverse" } else { "" });
                format!("for local {local} {} {}{flags} ({})", e(low), e(high), Printer::stmt_ids(body))
            }
            Expr::Break(value) => format!("break {}", opt(value)),
            Expr::Continue => "continue".to_string(),
            Expr::Return(value) => format!("return {}", opt(value)),
            Expr::FunctionRef(function) => format!("function_ref fn {}", function.0),
            Expr::CallValue { callee, args } => format!("call_value {} ({})", e(callee), ids(args)),
            Expr::Tuple(parts) => format!("tuple ({})", ids(parts)),
            Expr::Array(parts) => format!("array ({})", ids(parts)),
            Expr::ArrayRepeat { element, count } => format!("array_repeat {} x{count}", e(element)),
            Expr::StructOf { ty, fields } => format!("struct_of t{} ({})", ty.0, ids(fields)),
            Expr::VariantOf { ty, variant, fields } => format!("variant_of t{} v{variant} ({})", ty.0, ids(fields)),
            Expr::Field { expr, index } => format!("field {} .{index}", e(expr)),
            Expr::Index { expr, index } => format!("index {} {}", e(expr), e(index)),
            Expr::Length(expr) => format!("length {}", e(expr)),
            Expr::IsVariant { expr, variant } => format!("is_variant {} v{variant}", e(expr)),
            Expr::VariantField { expr, variant, field } => format!("variant_field {} v{variant} .{field}", e(expr)),
            Expr::OptionPack { is, value } => format!("option_pack {} {}", e(is), e(value)),
            Expr::OptionPart { option, part } => format!("option_part {} {}", e(option), if *part == LevelPart::Is { "is" } else { "value" }),
            Expr::Default(ty) => format!("default {ty}"),
        }
    }

    /// Statement ids as `s0, s1`.
    pub(super) fn stmt_ids(stmts: &[StmtId]) -> String {
        stmts.iter().map(|s| format!("s{}", s.0)).collect::<Vec<_>>().join(", ")
    }

    /// A statement's form with its operands as ids.
    pub(super) fn stmt_form(stmt: &Stmt) -> String {
        match stmt {
            Stmt::Let { local, init } => format!("let {local} e{}", init.0),
            Stmt::Expr(e) => format!("expr e{}", e.0),
            Stmt::Assign { place, op, value } => {
                let fields: String = place
                    .path
                    .iter()
                    .map(|step| match step {
                        Projection::Field(k) => format!(".{k}"),
                        Projection::Index(index) => format!("[e{}]", index.0),
                    })
                    .collect();
                let op = op.map_or("-".to_string(), |op| Printer::binary_name(op).to_string());
                format!("assign {}{fields} {op} e{}", place.local, value.0)
            }
            Stmt::BoundStep { site, value, when } => match when {
                Some(when) => format!("bound_step B{site} e{} when e{}", value.0, when.0),
                None => format!("bound_step B{site} e{}", value.0),
            },
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
            Stmt::Commit { carrier, value, after, target } => {
                let target = target.as_ref().map_or(String::new(), |t| format!(" {}", Printer::target_key(t)));
                format!("commit c{carrier} e{} {}{target}", value.0, after.map_or("-".to_string(), |a| format!("e{}", a.0)))
            }
            Stmt::Trace { site, fields } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|f| match f {
                        EmitField::Value { field, value } => format!("f{field} e{}", value.0),
                        EmitField::Text { name, text } => format!("{} {text:?}", Printer::name(name)),
                    })
                    .collect();
                format!("trace t{site} ({})", parts.join(", "))
            }
            Stmt::Emit { stream, fields } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|f| match f {
                        EmitField::Value { field, value } => format!("f{field} e{}", value.0),
                        EmitField::Text { name, text } => format!("{} {text:?}", Printer::name(name)),
                    })
                    .collect();
                format!("emit s{stream} ({})", parts.join(", "))
            }
        }
    }

    /// The operator symbol of a binary operator.
    pub(super) fn binary_op(op: BinaryOp) -> &'static str {
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
            BinaryOp::Rem => "%",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
        }
    }

    /// The name of a binary operator in the arena form.
    pub(super) fn binary_name(op: BinaryOp) -> &'static str {
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
            BinaryOp::Rem => "rem",
            BinaryOp::BitAnd => "bitand",
            BinaryOp::BitOr => "bitor",
            BinaryOp::BitXor => "bitxor",
            BinaryOp::Shl => "shl",
            BinaryOp::Shr => "shr",
        }
    }

    /// The name of a method in the arena form.
    pub(super) fn method_name(method: Method) -> &'static str {
        method.name()
    }

    /// The samples of a table model as `[(a, v), ..]`, each number as `real` writes it.
    pub(super) fn samples(table: &[(Real, Real)]) -> String {
        let pairs: Vec<String> = table.iter().map(|(abscissa, value)| format!("({}, {})", Printer::real(*abscissa), Printer::real(*value))).collect();
        format!("[{}]", pairs.join(", "))
    }

    /// The name of a math function.
    pub(super) fn math_name(f: MathFn) -> &'static str {
        match f {
            MathFn::Exp => "exp",
            MathFn::Ln => "ln",
            MathFn::Sqrt => "sqrt",
            MathFn::Abs => "abs",
        }
    }

    /// The name of an environment query.
    pub(super) fn env_name(env: Env) -> &'static str {
        match env {
            Env::Temperature => "temperature",
            Env::Gmin => "gmin",
            Env::SourceScale => "source_scale",
            Env::Time => "time",
            Env::Static => "static",
            Env::Freq => "freq",
            Env::Analysis(_) => "analysis",
            Env::TemperatureGiven => "temperature_given",
        }
    }
}
