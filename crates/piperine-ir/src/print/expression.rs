//! The code of the tree: statements and expressions, written by an iterative walk.

use crate::ids::{ExprId, StmtId};
use crate::model::{
    BinaryOp, CommitTarget, Drive, EmitField, Env, Expr, ExprData, Ir, LevelPart, Operator, Place, Projection, QuantityRole, Stmt, Terminal, UnaryOp, ValueType,
};
use crate::typing::Typing;

use super::{Piece, Printer, Scope};

impl<'a> Printer<'a> {
    /// A terminal by the name the scope gives it.
    pub(super) fn terminal(scope: Scope<'_>, t: Terminal) -> String {
        match t {
            Terminal::Ground => "gnd".to_string(),
            Terminal::Node(i) => scope.nodes.get(i as usize).map_or(format!("<bad node {i}>"), |n| Printer::name(&n.name)),
            Terminal::Own(k) => format!("own{k}"),
        }
    }

    /// A carrier by the name the scope gives it.
    pub(super) fn carrier(scope: Scope<'_>, i: u32) -> String {
        scope.carriers.get(i as usize).map_or(format!("<bad carrier {i}>"), |c| Printer::name(&c.name))
    }

    /// What a commit names inside its carrier, as pieces after the carrier's name: `[element]`, then `.k` for a field
    /// and `[index]` for an element of a value array.
    pub(super) fn target(next: &mut Vec<Piece>, target: &CommitTarget) {
        if let Some(element) = target.element {
            next.extend([Piece::Text("[".into()), Piece::Expr(element), Piece::Text("]".into())]);
        }
        for step in &target.path {
            match step {
                Projection::Field(k) => next.push(Piece::Text(format!(".{k}"))),
                Projection::Index(index) => next.extend([Piece::Text("[".into()), Piece::Expr(*index), Piece::Text("]".into())]),
            }
        }
    }

    /// What a definition names inside its carrier, for the structural form: ` at N` for the element, then the steps.
    pub(super) fn drive_target_key(drive: &Drive) -> String {
        let mut out = drive.element.map_or(String::new(), |e| format!(" at {e}"));
        for step in &drive.path {
            match step {
                Projection::Field(k) => out.push_str(&format!(".{k}")),
                Projection::Index(index) => out.push_str(&format!("[e{}]", index.0)),
            }
        }
        out
    }

    /// `target`, as the structural form writes it: `at eN` for the element, `.k` and `[eN]` for the steps.
    pub(super) fn target_key(target: &CommitTarget) -> String {
        let mut out = target.element.map_or(String::new(), |e| format!("at e{}", e.0));
        for step in &target.path {
            match step {
                Projection::Field(k) => out.push_str(&format!(".{k}")),
                Projection::Index(index) => out.push_str(&format!("[e{}]", index.0)),
            }
        }
        out
    }

    /// An assignment target as pieces: the local, then `.k` for a field and `[index]` for an element.
    pub(super) fn place(scope: Scope<'_>, place: &Place) -> Vec<Piece> {
        let mut pieces = vec![Piece::Text(Printer::local(scope, place.local))];
        for step in &place.path {
            match step {
                Projection::Field(k) => pieces.push(Piece::Text(format!(".{k}"))),
                Projection::Index(index) => pieces.extend([Piece::Text("[".into()), Piece::Expr(*index), Piece::Text("]".into())]),
            }
        }
        pieces
    }

    /// A local by the name the scope gives it.
    pub(super) fn local(scope: Scope<'_>, i: u32) -> String {
        scope.locals.get(i as usize).map_or(format!("<bad local {i}>"), |l| Printer::name(&l.name))
    }

    // ---- the tree of one statement or expression -------------------------------------------

    /// The text of `root`, written by an explicit stack.
    pub(super) fn render(&mut self, scope: Scope<'_>, root: Piece) -> String {
        if self.structural {
            // A statement is written out on its own: its numbering starts afresh.
            self.printed.clear();
            self.on_stack.clear();
            self.numbers.clear();
        }
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

    /// Pushes the pieces of statement `id` onto the walk; one arm per `Stmt` form.
    pub(super) fn stmt(&self, scope: Scope<'_>, id: StmtId, next: &mut Vec<Piece>) {
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
            Stmt::Assign { place, op, value } => {
                let op = op.map_or(String::new(), |op| Printer::binary_op(op).to_string());
                next.extend(Printer::place(scope, place));
                next.push(Piece::Text(format!(" {op}= ")));
                next.push(Piece::Expr(*value));
            }
            Stmt::BoundStep { value, when, .. } => {
                next.extend([Piece::Text("bound_step(".into()), Piece::Expr(*value), Piece::Text(")".into())]);
                if let Some(when) = when {
                    next.extend([Piece::Text(" when ".into()), Piece::Expr(*when)]);
                }
            }
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
            Stmt::Commit { carrier, value, after, target } => {
                next.push(Piece::Text(Printer::carrier(scope, *carrier)));
                if let Some(target) = target {
                    Printer::target(next, target);
                }
                next.push(Piece::Text(" <- ".into()));
                next.push(Piece::Expr(*value));
                if let Some(after) = after {
                    next.extend([Piece::Text(" after ".into()), Piece::Expr(*after)]);
                }
            }
            Stmt::Trace { site, fields } => {
                next.push(Piece::Text(format!("trace{site} {{")));
                Printer::emit_fields(next, fields, &|field| format!("f{field}"));
                next.push(Piece::Text("}".into()));
            }
            Stmt::Emit { stream, fields } => {
                let stream_plan = scope.streams.get(*stream as usize);
                let name = stream_plan.map_or(format!("<bad stream {stream}>"), |s| Printer::name(&s.name));
                next.push(Piece::Text(format!("emit {name}(")));
                let field_name = |field: u32| {
                    stream_plan.and_then(|s| s.fields.get(field as usize)).map_or("<bad field>".to_string(), |f| Printer::name(&f.name))
                };
                Printer::emit_fields(next, fields, &field_name);
                next.push(Piece::Text(")".into()));
            }
        }
    }

    /// The fields of a `trace` or an `emit`, separated by `, `: a value as `<name> = <expr>`, a text as
    /// `<name> = "<text>"`; `value_name` names the field a value is written to.
    fn emit_fields(next: &mut Vec<Piece>, fields: &[EmitField], value_name: &dyn Fn(u32) -> String) {
        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                next.push(Piece::Text(", ".into()));
            }
            match f {
                EmitField::Value { field, value } => {
                    next.push(Piece::Text(format!("{} = ", value_name(*field))));
                    next.push(Piece::Expr(*value));
                }
                EmitField::Text { name, text } => next.push(Piece::Text(format!("{} = {text:?}", Printer::name(name)))),
            }
        }
    }

    /// `items` separated by `, `.
    pub(super) fn list(next: &mut Vec<Piece>, items: &[ExprId]) {
        for (i, &item) in items.iter().enumerate() {
            if i > 0 {
                next.push(Piece::Text(", ".into()));
            }
            next.push(Piece::Expr(item));
        }
    }

    /// A loop body: `head { s0; s1 }`, the statements written in order.
    pub(super) fn statements(next: &mut Vec<Piece>, head: &str, stmts: &[StmtId]) {
        next.push(Piece::Text(if head.is_empty() { "{ ".to_string() } else { format!("{head} {{ ") }));
        for (i, &s) in stmts.iter().enumerate() {
            if i > 0 {
                next.push(Piece::Text("; ".into()));
            }
            next.push(Piece::Stmt(s));
        }
        next.push(Piece::Text(" }".into()));
    }

    /// Pushes the pieces of expression `id` onto the walk; one arm per `Expr` form.
    pub(super) fn expr(&mut self, scope: Scope<'_>, id: ExprId, next: &mut Vec<Piece>) {
        let Some(data) = self.ir.exprs.get(id.index()) else {
            next.push(Piece::Text(format!("<bad expr {}>", id.0)));
            return;
        };
        if self.on_stack.contains(&id) {
            next.push(Piece::Text(format!("<cycle {}>", self.reference(id))));
            return;
        }
        if !self.printed.insert(id) {
            next.push(Piece::Text(self.reference(id)));
            return;
        }
        let number = self.numbers.len();
        self.numbers.insert(id, number);
        self.on_stack.insert(id);
        let text = |t: String| Piece::Text(t);
        // The structural form writes a type its form does not imply.
        let note = self.structural && self.implied(scope, data).as_ref() != Some(&data.ty);
        if note {
            next.push(text("(".into()));
        }
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
            Expr::Tuple(parts) => {
                next.push(text("(".into()));
                Printer::list(next, parts);
                next.push(text(if parts.len() == 1 { ",)" } else { ")" }.into()));
            }
            Expr::Array(parts) => {
                next.push(text("[".into()));
                Printer::list(next, parts);
                next.push(text("]".into()));
            }
            Expr::ArrayRepeat { element, count } => next.extend([text("[".into()), Piece::Expr(*element), text(format!("; {count}]"))]),
            Expr::StructOf { ty, fields } => {
                next.push(text(format!("{}(", self.type_name(*ty))));
                Printer::list(next, fields);
                next.push(text(")".into()));
            }
            Expr::VariantOf { ty, variant, fields } => {
                next.push(text(format!("{}(", self.variant_name(*ty, *variant))));
                Printer::list(next, fields);
                next.push(text(")".into()));
            }
            Expr::Field { expr, index } => next.extend([Piece::Expr(*expr), text(format!(".{index}"))]),
            Expr::Index { expr, index } => next.extend([Piece::Expr(*expr), text("[".into()), Piece::Expr(*index), text("]".into())]),
            Expr::Length(expr) => next.extend([Piece::Expr(*expr), text(".len()".into())]),
            Expr::IsVariant { expr, variant } => {
                let ty = self.ir.exprs.get(expr.index()).map(|e| &e.ty);
                let name = match ty {
                    Some(ValueType::Enum(id)) => self.variant_name(*id, *variant),
                    _ => format!("<variant {variant}>"),
                };
                next.extend([text("matches!(".into()), Piece::Expr(*expr), text(format!(", {name})"))]);
            }
            Expr::VariantField { expr, variant, field } => next.extend([Piece::Expr(*expr), text(format!(" as variant {variant} .{field}"))]),
            Expr::OptionPack { is, value } => next.extend([text("option(".into()), Piece::Expr(*is), text(", ".into()), Piece::Expr(*value), text(")".into())]),
            Expr::OptionPart { option, part } => next.extend([Piece::Expr(*option), text(if *part == LevelPart::Is { ".is" } else { ".value" }.into())]),
            Expr::Default(ty) => next.push(text(format!("{ty}::default()"))),
            Expr::Table { x, table, control } | Expr::TableSlope { x, table, control } => {
                let name = if matches!(data.kind, Expr::Table { .. }) { "table" } else { "table_slope" };
                next.extend([text(format!("{name}(")), Piece::Expr(*x), text(format!(", \"{}\", {})", control.text(), Printer::samples(table)))]);
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
            Expr::Env(Env::Analysis(kind)) => next.push(text(format!("analysis({kind:?})"))),
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
            // An integer literal carries its type as a suffix: `5u9`, `-3i16`.
            Expr::Int(v) => {
                let suffix = if data.ty.is_integer() { data.ty.to_string() } else { String::new() };
                next.push(text(format!("{v}{suffix}")));
            }
            Expr::FloatBits(bits) => next.push(text(Printer::float_bits_text(&data.ty, *bits))),
            Expr::Wide(id) => next.push(text(self.wide_text(*id))),
            Expr::Cast { expr, to } => next.extend([text("(".into()), Piece::Expr(*expr), text(format!(" as {to})"))]),
            Expr::Method { method, receiver, args } => {
                next.extend([Piece::Expr(*receiver), text(format!(".{}(", Printer::method_name(*method)))]);
                Printer::list(next, args);
                next.push(text(")".into()));
            }
            Expr::Carrier(i) => next.push(text(Printer::carrier(scope, *i))),
            Expr::Flow { branch } => next.push(text(format!("flow(b{branch})"))),
            Expr::Previous(i) => next.push(text(format!("{}.prev()", Printer::carrier(scope, *i)))),
            Expr::Others(i) => next.push(text(format!("{}.others()", Printer::carrier(scope, *i)))),
            Expr::Resolve { by_count, drives, except } => {
                let functions: Vec<String> = by_count.iter().map(|f| self.ir.functions.get(f.index()).map_or("<bad function>".to_string(), |f| Printer::name(&f.name))).collect();
                let drives: Vec<String> = drives.iter().map(|c| Printer::carrier(scope, *c)).collect();
                let except = except.map_or(String::new(), |k| format!(" except {k}"));
                next.push(text(format!("resolve [{}] of ({}){except}", functions.join(", "), drives.join(", "))));
            }
            Expr::CarrierElement { carrier, index } => {
                next.extend([text(format!("{}[", Printer::carrier(scope, *carrier))), Piece::Expr(*index), text("]".into())])
            }
            Expr::ElementPrevious { carrier, index } => {
                next.extend([text(format!("{}[", Printer::carrier(scope, *carrier))), Piece::Expr(*index), text("].prev()".into())])
            }
            Expr::ElementChanged { carrier, index } => {
                next.extend([text(format!("{}[", Printer::carrier(scope, *carrier))), Piece::Expr(*index), text("].changed()".into())])
            }
            Expr::Derivative(k) => next.push(text(format!("rate(D{k})"))),
            Expr::Delay(k) => next.push(text(format!("delay{k}.value"))),
            Expr::Noise(k) => next.push(text(format!("noise{k}.value"))),
            Expr::SessionParameter { parameter } => {
                let name = scope.session_parameters.get(*parameter as usize).map_or(format!("<bad session parameter {parameter}>"), |p| format!("{:?}", p.name));
                next.push(text(format!("simparam({name})")));
            }
            Expr::ParamPrevious(i) => next.push(text(format!("{}.prev()", scope.params.get(*i as usize).map_or(format!("<bad param {i}>"), |p| Printer::name(&p.name))))),
            Expr::Fired(e) => next.push(text(format!("fired(E{e})"))),
            Expr::Same { a, b } => next.extend([text("same(".into()), Piece::Expr(*a), text(", ".into()), Piece::Expr(*b), text(")".into())]),
            Expr::LevelCast { expr, to, tags } => {
                let tags = tags.as_ref().map(|t| format!(" in {t:?}")).unwrap_or_default();
                next.extend([text(format!("cast::<{to}{tags}>(")), Piece::Expr(*expr), text(")".into())]);
            }
            Expr::Loop { body } => Printer::statements(next, "loop", body),
            Expr::While { cond, body } => {
                next.extend([text("while ".into()), Piece::Expr(*cond), text(" ".into())]);
                Printer::statements(next, "", body);
            }
            Expr::ForRange { local, low, high, inclusive, reverse, body } => {
                let range = if *inclusive { "..=" } else { ".." };
                let (open, close) = if *reverse { ("(", ").rev()") } else { ("", "") };
                next.extend([text(format!("for {} in {open}", Printer::local(scope, *local))), Piece::Expr(*low), text(range.into()), Piece::Expr(*high), text(format!("{close} "))]);
                Printer::statements(next, "", body);
            }
            Expr::Break(value) => {
                next.push(text("break".into()));
                if let Some(value) = value {
                    next.extend([text(" ".into()), Piece::Expr(*value)]);
                }
            }
            Expr::Continue => next.push(text("continue".into())),
            Expr::Return(value) => {
                next.push(text("return".into()));
                if let Some(value) = value {
                    next.extend([text(" ".into()), Piece::Expr(*value)]);
                }
            }
            Expr::FunctionRef(function) => next.push(text(self.ir.functions.get(function.index()).map_or("<bad function>".to_string(), |f| Printer::name(&f.name)))),
            Expr::CallValue { callee, args } => {
                next.extend([Piece::Expr(*callee), text("(".into())]);
                Printer::list(next, args);
                next.push(text(")".into()));
            }
            Expr::Unwrap { is, value, message } => {
                let name = if message.is_some() { "expect" } else { "unwrap" };
                next.extend([text(format!("{name}(")), Piece::Expr(*is), text(", ".into()), Piece::Expr(*value)]);
                if let Some(message) = message {
                    next.push(text(format!(", {message:?}")));
                }
                next.push(text(")".into()));
            }
        }
        if note {
            next.push(text(format!(" : {})", data.ty)));
        }
        next.push(Piece::Exit(id));
    }

    /// How a later use of `id` prints: its arena id in the full text, its number in this
    /// statement in the structural form.
    pub(super) fn reference(&self, id: ExprId) -> String {
        match self.numbers.get(&id) {
            Some(k) if self.structural => format!("^{k}"),
            _ => format!("e{}", id.0),
        }
    }

    /// The type of a block with `value`: the value's recorded type, `()` without one.
    pub(super) fn block_type(ir: &Ir, value: &Option<ExprId>) -> ValueType {
        match value {
            Some(v) => ir.exprs.get(v.index()).map_or(ValueType::Unit, |e| e.ty.clone()),
            None => ValueType::Unit,
        }
    }

    /// The type an expression's form implies, given the recorded types of its operands; `None`
    /// when an id it needs is out of range or the operands fit no rule. An operand outside `f64`
    /// and `bool`, or an operator M1 does not have, selects the digital rules of `Typing`.
    pub(super) fn implied(&self, scope: Scope<'_>, data: &ExprData) -> Option<ValueType> {
        let ir = self.ir;
        let ty = |id: &ExprId| ir.exprs.get(id.index()).map(|e| e.ty.clone());
        match &data.kind {
            Expr::Int(_) => data.ty.is_integer().then(|| data.ty.clone()),
            Expr::FloatBits(_) => data.ty.is_other_float().then(|| data.ty.clone()),
            Expr::Wide(_) => data.ty.is_wide_integer().then(|| data.ty.clone()),
            Expr::Cast { expr, to } => Typing::cast(&ty(expr)?, to),
            Expr::Method { method, receiver, args } => {
                let args: Option<Vec<ValueType>> = args.iter().map(ty).collect();
                let args = args?;
                Typing::method(*method, &ty(receiver)?, &args.iter().collect::<Vec<_>>())
            }
            Expr::Same { a, b } => Typing::same(&ty(a)?, &ty(b)?),
            Expr::LevelCast { expr, to, .. } => Typing::level_cast(&ty(expr)?, to, &ir.types),
            Expr::Carrier(i) | Expr::Previous(i) => scope.carriers.get(*i as usize).map(|c| Typing::read_of_carrier(c)),
            Expr::Others(i) => scope.carriers.get(*i as usize).map(|c| Typing::read_of(&c.ty)),
            Expr::Resolve { by_count, .. } => by_count.first().and_then(|f| ir.functions.get(f.index())).map(|f| f.ret.clone()),
            Expr::CarrierElement { carrier, .. } | Expr::ElementPrevious { carrier, .. } => {
                scope.carriers.get(*carrier as usize).map(|c| Typing::read_of(&c.ty))
            }
            Expr::ElementChanged { .. } => Some(ValueType::Bool),
            Expr::Unary { op, expr } if ty(expr).is_some_and(|t| Typing::digital(&t)) => Typing::unary(*op, &ty(expr)?),
            Expr::Binary { op, lhs, rhs }
                if Typing::digital_op(*op) || [lhs, rhs].iter().any(|e| ty(e).is_some_and(|t| Typing::digital(&t))) =>
            {
                Typing::binary(*op, &ty(lhs)?, &ty(rhs)?)
            }
            kind => self.implied_m1(scope, kind),
        }
    }

    /// The M1 rule of `implied`, unchanged: every M1 golden print keeps its bytes.
    pub(super) fn implied_m1(&self, scope: Scope<'_>, kind: &Expr) -> Option<ValueType> {
        let ir = self.ir;
        let ty = |id: &ExprId| ir.exprs.get(id.index()).map(|e| e.ty.clone());
        match kind {
            Expr::Env(Env::Static | Env::Analysis(_) | Env::TemperatureGiven) | Expr::Fired(_) => Some(ValueType::Bool),
            Expr::Real(_) | Expr::Math { .. } | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::Probe { .. } | Expr::Env(_) | Expr::Limit { .. } | Expr::Stim { .. } | Expr::Flow { .. } | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) => {
                Some(ValueType::F64)
            }
            Expr::SessionParameter { .. } => Some(ValueType::Option(Box::new(ValueType::F64))),
            Expr::Bool(_) => Some(ValueType::Bool),
            Expr::Unit => Some(ValueType::Unit),
            Expr::Local(i) => scope.locals.get(*i as usize).map(|l| l.ty.clone()),
            Expr::Param(i) | Expr::ParamPrevious(i) => scope.params.get(*i as usize).map(|p| p.ty.clone()),
            Expr::Unary { op: UnaryOp::Neg, .. } => Some(ValueType::F64),
            Expr::Unary { op: UnaryOp::Not, .. } => Some(ValueType::Bool),
            Expr::Binary { op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div, .. } => Some(ValueType::F64),
            Expr::Binary { .. } => Some(ValueType::Bool),
            Expr::If { then, .. } => ty(then),
            Expr::Block { value, .. } => Some(Printer::block_type(ir, value)),
            Expr::Call { function, .. } => ir.functions.get(function.index()).map(|f| f.ret.clone()),
            Expr::Closure { params, body } => {
                let params: Option<Vec<ValueType>> = params.iter().map(|p| scope.locals.get(*p as usize).map(|l| l.ty.clone())).collect();
                Some(ValueType::Fn(params?, Box::new(ty(body)?)))
            }
            Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Cast { .. } | Expr::Method { .. } => None,
            Expr::Carrier(_) | Expr::Previous(_) | Expr::Others(_) | Expr::Resolve { .. } | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => None,
            Expr::Same { .. } | Expr::LevelCast { .. } => None,
            Expr::Unwrap { value, .. } => ty(value),
            Expr::While { .. } | Expr::ForRange { .. } => Some(ValueType::Unit),
            // A diverging form fits any type, and a loop is typed by its breaks: both write theirs.
            Expr::Loop { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) => None,
            Expr::FunctionRef(function) => ir.functions.get(function.index()).map(|f| ValueType::Fn(f.params.iter().map(|p| p.ty.clone()).collect(), Box::new(f.ret.clone()))),
            Expr::CallValue { callee, .. } => match ty(callee)? {
                ValueType::Fn(_, ret) => Some(*ret),
                _ => None,
            },
            // The aggregate forms are typed by their arena entry: the structural form writes the type.
            Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } => None,
            Expr::Length(_) => Some(ValueType::UInt(64)),
            Expr::IsVariant { .. } | Expr::OptionPart { part: LevelPart::Is, .. } => Some(ValueType::Bool),
            Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } => None,
            Expr::Default(ty) => Some(ty.clone()),
        }
    }
}
