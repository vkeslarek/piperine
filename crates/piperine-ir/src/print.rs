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
//!
//! [`Ir::print_structural`] prints the **structural form** (IR-11): the tree alone, with no
//! origin and no arena id, and the type of every expression whose form does not imply it. It is
//! the form two producers of one design compare (the hand-built fixtures and `msl-elab`), so
//! nothing in it depends on the order of an arena: natures, bodies, functions, the locals of an
//! owner and the node table print sorted by their text; instances keep the pre-order of
//! IR-08 AC 2; params, nodes, branches, sites, children and statements keep declaration order.
//! An expression is written out in full wherever it is reached; one reached again inside the same
//! statement prints as `^k`, the `k`-th expression written out in that statement (`<cycle ^k>`
//! when it is still being written), so the numbering follows the tree, not the arena.

use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display, Write};

use crate::ids::{AggregateId, WideId, ExprId, FunctionId, StmtId, TypeId};
use crate::model::{
    Drive,
    BinaryOp, BodyData, BodyNode, BranchKind, Carrier, CarrierRole, ChildValue, Constant, EmitField, Env, EventSource, Expr, ExprData, FieldsForm, FunctionData, InstanceData, Ir,
    CommitTarget, LevelPart, Local, MathFn, Method, NodeRef, Operator, Origin, Param, Place, Projection, Quantity, QuantityRole, Stmt, Stream, ReprForm, Terminal, TypeData, TypeKind,
    UnaryOp, ValueType,
};
use crate::real::Real;
use crate::typing::Typing;

impl Display for Ir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&Printer::new(self, false).text())
    }
}

impl Ir {
    /// The structural form of this IR (IR-11): the tree of the printed text with every
    /// expression's type where its form does not imply it, and no origin or arena id. Two IRs
    /// that differ only in origins or in the order of their arenas print it byte-identically.
    pub fn print_structural(&self) -> String {
        Printer::new(self, true).text()
    }
}

impl Ir {
    /// The `where` expression of param `param` of `body`, as the printer writes it after the
    /// word `where`; `None` for a param with no check or an index out of range.
    pub(crate) fn param_check_text(&self, body: &BodyData, param: usize) -> Option<String> {
        let check = body.params.get(param)?.check?;
        let scope = Scope { locals: &body.locals, params: &body.params, nodes: &body.nodes, carriers: &body.carriers, streams: &body.streams };
        Some(Printer::new(self, false).render(scope, Piece::Expr(check)))
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
            ValueType::F32 => f.write_str("f32"),
            ValueType::F16 => f.write_str("f16"),
            ValueType::F128 => f.write_str("f128"),
            ValueType::Bool => f.write_str("bool"),
            ValueType::Unit => f.write_str("()"),
            ValueType::Fn(params, ret) => {
                let params: Vec<String> = params.iter().map(ToString::to_string).collect();
                write!(f, "fn({}) -> {ret}", params.join(", "))
            }
            ValueType::UInt(n) => write!(f, "u{n}"),
            ValueType::Int(n) => write!(f, "i{n}"),
            ValueType::Tuple(parts) if parts.len() == 1 => write!(f, "({},)", parts[0]),
            ValueType::Tuple(parts) => {
                let parts: Vec<String> = parts.iter().map(ToString::to_string).collect();
                write!(f, "({})", parts.join(", "))
            }
            ValueType::Array(element, length) => write!(f, "[{element}; {length}]"),
            ValueType::Struct(id) | ValueType::Enum(id) => write!(f, "t{}", id.0),
            ValueType::Option(inner) => write!(f, "Option<{inner}>"),
        }
    }
}

impl Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Real(r) => f.write_str(&Printer::real(*r)),
            Constant::Bool(b) => write!(f, "{b}"),
            Constant::Int(v) => write!(f, "{v}"),
            Constant::Aggregate(id) => write!(f, "agg{}", id.0),
            Constant::FloatBits(bits) => write!(f, "bits(0x{bits:x})"),
            Constant::Wide(id) => write!(f, "wide{}", id.0),
        }
    }
}

/// What names the locals, params and terminals of the code being printed.
#[derive(Clone, Copy)]
struct Scope<'a> {
    locals: &'a [Local],
    params: &'a [Param],
    nodes: &'a [BodyNode],
    carriers: &'a [Carrier],
    streams: &'a [Stream],
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
    /// The structural form (IR-11) rather than the full text (IR-04).
    structural: bool,
    /// Expressions already written out in the tree; a later use prints `eN` (in the structural
    /// form: `^k`, within one statement).
    printed: HashSet<ExprId>,
    /// Expressions being written out; meeting one again is a cycle.
    on_stack: HashSet<ExprId>,
    /// The structural form's numbering: the `k`-th expression written out in this statement.
    numbers: HashMap<ExprId, usize>,
}

impl<'a> Printer<'a> {
    fn new(ir: &'a Ir, structural: bool) -> Self {
        Printer { ir, out: String::new(), structural, printed: HashSet::new(), on_stack: HashSet::new(), numbers: HashMap::new() }
    }

    fn text(mut self) -> String {
        self.ir();
        self.out
    }

    /// `  @origin` after an item in the full text; nothing in the structural form.
    fn at(&self, origin: &Origin) -> String {
        if self.structural { String::new() } else { format!("  @{origin}") }
    }

    /// The text `write` appends to the output, taken out of it.
    fn chunk(&mut self, write: impl FnOnce(&mut Self)) -> String {
        let start = self.out.len();
        write(self);
        self.out.split_off(start)
    }

    /// Items printed in arena order in the full text; sorted by their text in the structural form.
    fn items(&mut self, mut chunks: Vec<String>) {
        if self.structural {
            chunks.sort();
        }
        for chunk in chunks {
            self.out.push_str(&chunk);
        }
    }
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
        let ir = self.ir;
        let natures = ir
            .natures
            .iter()
            .map(|nature| {
                self.chunk(|p| {
                    p.line(0, &format!("nature {}", Printer::name(&nature.name)));
                    p.line(1, &format!("potential {}", Printer::quantity(&nature.potential)));
                    if let Some(flow) = &nature.flow {
                        p.line(1, &format!("flow {}", Printer::quantity(flow)));
                    }
                })
            })
            .collect();
        self.items(natures);
        let types = ir.types.iter().enumerate().map(|(i, data)| self.chunk(|p| p.type_item(i, data))).collect();
        self.items(types);
        let aggregates = ir.aggregates.iter().enumerate().map(|(i, data)| self.chunk(|p| p.line(0, &format!("aggregate agg{i}: {} = {}", data.ty, p.constant(&Constant::Aggregate(AggregateId(i as u32)), 0))))).collect();
        self.items(aggregates);
        let bodies = ir.bodies.iter().map(|body| self.chunk(|p| p.body(body))).collect();
        self.items(bodies);
        let functions = ir.functions.iter().map(|function| self.chunk(|p| p.function(function))).collect();
        self.items(functions);
        for (i, instance) in ir.instances.iter().enumerate() {
            self.instance(i, instance);
        }
        let nodes = ir
            .nodes
            .iter()
            .map(|node| {
                let nature = self.nature_name(node.nature.index());
                let declarer = ir.instances.get(node.declared_by.index()).map_or("<bad instance>".to_string(), |i| Printer::name(&i.path));
                let init = node.init.map_or(String::new(), |r| format!("  init {}", Printer::real(r)));
                let at = self.at(&node.origin);
                self.chunk(|p| p.line(0, &format!("node {}: {nature}  declared by {declarer}{init}{at}", Printer::name(&node.path))))
            })
            .collect();
        self.items(nodes);
        let nets = ir
            .nets
            .iter()
            .map(|net| {
                let declarer = ir.instances.get(net.declared_by.index()).map_or("<bad instance>".to_string(), |i| Printer::name(&i.path));
                let init = net.init.map_or(String::new(), |c| format!("  init {c}"));
                let at = self.at(&net.origin);
                self.chunk(|p| p.line(0, &format!("net {}: {}  declared by {declarer}{init}{at}", Printer::name(&net.path), net.ty)))
            })
            .collect();
        self.items(nets);
        if !self.structural {
            self.arenas();
        }
    }

    /// The fields of a struct or a variant: `{ x: f64 }`, `(f64, bool)`, or nothing.
    fn fields_text(form: FieldsForm, fields: &[(String, ValueType)]) -> String {
        match form {
            FieldsForm::Unit => String::new(),
            FieldsForm::Tuple => format!("({})", fields.iter().map(|(_, ty)| ty.to_string()).collect::<Vec<_>>().join(", ")),
            FieldsForm::Named => format!(" {{ {} }}", fields.iter().map(|(name, ty)| format!("{}: {ty}", Printer::name(name))).collect::<Vec<_>>().join(", ")),
        }
    }

    /// One entry of the type arena: `type t0 = struct P { x: f64 }  path m::P  derive Clone, Copy`.
    fn type_item(&mut self, index: usize, data: &TypeData) {
        let shape = match &data.kind {
            TypeKind::Struct { form, fields } => format!("struct {}{}", Printer::name(&data.name), Printer::fields_text(*form, fields)),
            TypeKind::Enum { tag_levels, variants } => {
                let variants: Vec<String> = variants.iter().map(|v| format!("{}{} = {}", Printer::name(&v.name), Printer::fields_text(v.form, &v.fields), v.discriminant)).collect();
                format!("enum {} [tag {tag_levels}] {{ {} }}", Printer::name(&data.name), variants.join(", "))
            }
        };
        let d = data.derives;
        let flags = [("Clone", d.clone), ("Copy", d.copy), ("PartialEq", d.partial_eq), ("Eq", d.eq), ("PartialOrd", d.partial_ord), ("Ord", d.ord), ("Default", d.default), ("Debug", d.debug), ("Hash", d.hash), ("Repr", d.repr)];
        let derived: Vec<&str> = flags.iter().filter(|(_, on)| *on).map(|(name, _)| *name).collect();
        let derive = if derived.is_empty() { String::new() } else { format!("  derive {}", derived.join(", ")) };
        let at = self.at(&data.origin);
        let instance = data.instance.as_ref().map(|instance| format!("  instance of {}", instance.item)).unwrap_or_default();
        let repr = match data.repr {
            ReprForm::Derived => String::new(),
            ReprForm::HandWritten { width, encode, decode } => {
                let name = |id: FunctionId| self.ir.functions.get(id.index()).map_or_else(|| format!("<bad function {}>", id.0), |f| Printer::name(&f.name));
                format!("  repr hand-written {width} levels, encode {}, decode {}", name(encode), name(decode))
            }
        };
        self.line(0, &format!("type t{index} = {shape}  path {}{derive}{instance}{repr}{at}", Printer::name(&data.path)));
    }

    /// A constant as source: a scalar as `Display` writes it, a composite value spelled out through
    /// its type (`Pair(1.0, true)`, `Tx::Data(3)`, `[1, 2]`, `Some(1.0)`); `depth` ends a cycle.
    fn constant(&self, value: &Constant, depth: usize) -> String {
        if let Constant::Wide(id) = value {
            return self.wide_text(*id);
        }
        let Constant::Aggregate(id) = value else { return Printer::scalar(value) };
        let Some(data) = self.ir.aggregates.get(id.index()) else { return format!("<bad aggregate {}>", id.0) };
        if depth > 32 {
            return "..".to_string();
        }
        let parts: Vec<String> = data.parts.iter().map(|part| self.constant(part, depth + 1)).collect();
        let list = parts.join(", ");
        match &data.ty {
            ValueType::Tuple(_) if parts.len() == 1 => format!("({list},)"),
            ValueType::Tuple(_) => format!("({list})"),
            ValueType::Array(..) => format!("[{list}]"),
            ValueType::Struct(ty) => format!("{}({list})", self.type_name(*ty)),
            ValueType::Enum(ty) => format!("{}({list})", self.variant_name(*ty, data.variant.unwrap_or(u32::MAX))),
            ValueType::Option(_) if parts.is_empty() => "None".to_string(),
            _ => format!("Some({list})"),
        }
    }

    /// A scalar constant as source.
    fn scalar(value: &Constant) -> String {
        match value {
            Constant::Real(r) => Printer::real(*r),
            Constant::Bool(b) => b.to_string(),
            Constant::Int(v) => v.to_string(),
            Constant::Aggregate(id) => format!("agg{}", id.0),
            Constant::FloatBits(bits) => format!("bits(0x{bits:x})"),
            Constant::Wide(id) => format!("wide{}", id.0),
        }
    }

    /// A constant of an integer type wider than 64 bits as `wide(0x..)`: its limbs, high limb first
    /// (`<bad wide>` for an id outside the arena).
    fn wide_text(&self, id: WideId) -> String {
        let Some(data) = self.ir.wides.get(id.index()) else { return format!("<bad wide {}>", id.0) };
        let limbs: String = data.limbs.iter().rev().map(|limb| format!("{limb:016x}")).collect();
        format!("wide(0x{limbs})")
    }

    /// A float literal of a width other than `f64`: a finite `f32` as `1.5f32`, every other value
    /// of `f32`, `f16` and `f128` as its IEEE 754 bits, `f16(0x3c00)`.
    fn float_bits_text(ty: &ValueType, bits: u128) -> String {
        match ty {
            ValueType::F32 => {
                let value = f32::from_bits(bits as u32);
                if value.is_finite() { format!("{value:?}f32") } else { format!("f32(0x{bits:x})") }
            }
            other => format!("{other}(0x{bits:x})"),
        }
    }

    /// The name of a type of the arena, `<bad type>` for an id outside it.
    fn type_name(&self, ty: TypeId) -> String {
        self.ir.types.get(ty.index()).map_or("<bad type>".to_string(), |data| Printer::name(&data.name))
    }

    /// `Enum::Variant`, `<bad variant>` for one outside the enum.
    fn variant_name(&self, ty: TypeId, variant: u32) -> String {
        let name = match self.ir.types.get(ty.index()).map(|data| &data.kind) {
            Some(TypeKind::Enum { variants, .. }) => variants.get(variant as usize).map(|v| Printer::name(&v.name)),
            _ => None,
        };
        format!("{}::{}", self.type_name(ty), name.unwrap_or_else(|| "<bad variant>".to_string()))
    }

    /// The locals table of an owner: index order in the full text, sorted in the structural form.
    fn locals(&mut self, indent: usize, locals: &[Local]) {
        let lines = locals.iter().map(|l| self.chunk(|p| p.line(indent, &format!("local {}{}: {}", if l.mutable { "mut " } else { "" }, Printer::name(&l.name), l.ty)))).collect();
        self.items(lines);
    }

    fn nature_name(&self, i: usize) -> String {
        self.ir.natures.get(i).map_or("<bad nature>".to_string(), |n| Printer::name(&n.name))
    }

    fn quantity(q: &Quantity) -> String {
        format!("{} {} {:?} abstol {}", Printer::name(&q.name), Printer::name(&q.access), q.units, Printer::real(q.abstol))
    }

    fn body(&mut self, body: &BodyData) {
        let at = self.at(&body.origin);
        self.line(0, &format!("body {}{at}", Printer::name(&body.name)));
        let scope = Scope { locals: &body.locals, params: &body.params, nodes: &body.nodes, carriers: &body.carriers, streams: &body.streams };
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
            text.push_str(&self.at(&param.origin));
            self.line(1, &text);
        }
        for node in &body.nodes {
            let nature = self.nature_name(node.nature.index());
            let role = match node.role {
                crate::model::NodeRole::Port => "port",
                crate::model::NodeRole::Signal => "signal",
                crate::model::NodeRole::Reached => "reached",
            };
            let init = node.init.map_or(String::new(), |r| format!(" init {}", Printer::real(r)));
            let at = self.at(&node.origin);
            self.line(1, &format!("node {}: {nature} {role}{init}{at}", Printer::name(&node.name)));
        }
        for carrier in &body.carriers {
            let role = match carrier.role {
                CarrierRole::In => "in",
                CarrierRole::Out => "out",
                CarrierRole::Signal => "signal",
                CarrierRole::Reached => "reached",
            };
            let init = carrier.init.map_or(String::new(), |c| format!(" init {c}"));
            let at = self.at(&carrier.origin);
            self.line(1, &format!("carrier {}: {} {role}{init}{at}", Printer::name(&carrier.name), carrier.ty));
        }
        for stream in &body.streams {
            let fields: Vec<String> = stream.fields.iter().map(|f| format!("{}: {}", Printer::name(&f.name), f.ty)).collect();
            self.line(1, &format!("stream {} ({})", Printer::name(&stream.name), fields.join(", ")));
        }
        for (i, branch) in body.branches.iter().enumerate() {
            let kind = match branch.kind {
                BranchKind::Flow => "flow",
                BranchKind::Potential => "potential",
                BranchKind::Switch => "switch",
            };
            let plus = Printer::terminal(scope, branch.plus);
            let minus = Printer::terminal(scope, branch.minus);
            self.line(1, &format!("branch b{i} ({plus}, {minus}) {kind}"));
        }
        for (i, site) in body.limits.iter().enumerate() {
            let enabled = site.enabled.map_or(String::new(), |e| format!(" enabled {}", self.render(scope, Piece::Expr(e))));
            let at = self.at(&site.origin);
            self.line(1, &format!("limit L{i} {:?}{enabled}{at}", site.name));
        }
        for (i, site) in body.stims.iter().enumerate() {
            let at = self.at(&site.origin);
            self.line(1, &format!("stim S{i} {:?}{at}", site.name));
        }
        for (i, site) in body.derivatives.iter().enumerate() {
            let mut text = format!("derivative D{i} {:?} state {}", site.name, self.render(scope, Piece::Expr(site.state)));
            if let Some(abstol) = site.abstol {
                let _ = write!(text, " abstol {}", self.render(scope, Piece::Expr(abstol)));
            }
            text.push_str(&self.at(&site.origin));
            self.line(1, &text);
        }
        for hoisted in &body.hoisted {
            let at = self.at(&hoisted.origin);
            self.line(1, &format!("hoisted {}: {}{at}", Printer::name(&hoisted.name), Printer::name(&hoisted.module)));
        }
        for merge in &body.merges {
            self.line(1, &format!("merge ({}, {})", Printer::terminal(scope, merge.plus), Printer::terminal(scope, merge.minus)));
        }
        for reach in &body.reaches {
            let child = body.children.get(reach.child as usize);
            let callee = child.and_then(|c| self.ir.bodies.get(c.body.index()));
            let child_name = child.map_or("<bad child>".to_string(), |c| Printer::name(&c.name));
            let text = match reach.item {
                crate::model::ReachedItem::Node { node, child_node } => {
                    let own = scope.nodes.get(node as usize).map_or(format!("<bad node {node}>"), |n| Printer::name(&n.name));
                    let theirs = callee.and_then(|b| b.nodes.get(child_node as usize)).map_or(format!("<bad node {child_node}>"), |n| Printer::name(&n.name));
                    format!("reach node {own} = {child_name}.{theirs}")
                }
                crate::model::ReachedItem::Carrier { carrier, child_carrier } => {
                    let own = Printer::carrier(scope, carrier);
                    let theirs = callee.and_then(|b| b.carriers.get(child_carrier as usize)).map_or(format!("<bad carrier {child_carrier}>"), |c| Printer::name(&c.name));
                    format!("reach carrier {own} = {child_name}.{theirs}")
                }
            };
            self.line(1, &text);
        }
        for inflow in &body.inflows {
            let port = scope.nodes.get(inflow.port as usize).map_or(format!("<bad node {}>", inflow.port), |n| Printer::name(&n.name));
            self.line(1, &format!("inflow {port} measured by b{}", inflow.branch));
        }
        self.locals(1, &body.locals);
        for (ci, child) in body.children.iter().enumerate() {
            let callee = self.ir.bodies.get(child.body.index());
            let name = callee.map_or("<bad body>".to_string(), |b| Printer::name(&b.name));
            // A child's own node reads as the callee names it: `own:<name>` (Q1).
            let own = |k: u32| callee.and_then(|b| b.nodes.get(k as usize)).map_or(format!("own:<bad node {k}>"), |n| format!("own:{}", Printer::name(&n.name)));
            let nodes: Vec<String> = child.nodes.iter().map(|t| match t {
                Terminal::Own(k) => own(*k),
                other => Printer::terminal(scope, *other),
            }).collect();
            let pname = |i: u32| callee.and_then(|b| b.params.get(i as usize)).map_or("<bad param>".to_string(), |p| Printer::name(&p.name));
            // A value set beside the child (`ChildValue`) shows in place of the constant of the
            // same param; one with no constant follows the constants.
            let entries: &[(u32, ChildValue)] = body.child_params.get(ci).map_or(&[], Vec::as_slice);
            let mut params: Vec<String> = Vec::new();
            for (i, v) in &child.params {
                match entries.iter().find(|(j, _)| j == i) {
                    Some((_, value)) => params.push(format!("{} = {}", pname(*i), self.child_value(scope, value))),
                    None => params.push(format!("{} = {v}", pname(*i))),
                }
            }
            for (i, value) in entries {
                if !child.params.iter().any(|(j, _)| j == i) {
                    params.push(format!("{} = {}", pname(*i), self.child_value(scope, value)));
                }
            }
            // The carriers the placement connects, by the parent's names: `_` for an open port.
            let bound = body.child_carriers.get(ci).map_or(&[][..], Vec::as_slice);
            let carriers = if bound.is_empty() {
                String::new()
            } else {
                let names: Vec<String> = bound.iter().map(|c| c.map_or("_".to_string(), |c| Printer::carrier(scope, c))).collect();
                format!("  carriers ({})", names.join(", "))
            };
            let at = self.at(&child.origin);
            self.line(
                1,
                &format!("child {}: {name}  nodes ({})  params ({}){carriers}{at}", Printer::name(&child.name), nodes.join(", "), params.join(", ")),
            );
        }
        for drive in &body.drives {
            let mut text = format!("drive {}", Printer::carrier(scope, drive.carrier));
            if let Some(element) = drive.element {
                text.push_str(&format!("[{element}]"));
            }
            for step in &drive.path {
                match step {
                    Projection::Field(k) => text.push_str(&format!(".{k}")),
                    Projection::Index(index) => text.push_str(&format!("[{}]", self.render(scope, Piece::Expr(*index)))),
                }
            }
            text.push_str(" = ");
            text.push_str(&self.render(scope, Piece::Expr(drive.value)));
            if let Some(after) = drive.after {
                text.push_str(" after ");
                text.push_str(&self.render(scope, Piece::Expr(after)));
            }
            text.push_str(&self.at(&drive.origin));
            self.line(1, &text);
        }
        for (k, event) in body.events.iter().enumerate() {
            let mut text = format!("event E{k} ");
            match &event.source {
                EventSource::Changed(c) => text.push_str(&format!("changed {}", Printer::carrier(scope, *c))),
                // `settle` (§13 rule 17) names no carrier, param or expression.
                EventSource::Settle => text.push_str("settle"),
                EventSource::ElementChanged { carrier, element } => {
                    text.push_str(&format!("changed {}[{element}]", Printer::carrier(scope, *carrier)))
                }
                EventSource::ParamChanged(p) => {
                    let name = scope.params.get(*p as usize).map_or(format!("<bad param {p}>"), |param| Printer::name(&param.name));
                    text.push_str(&format!("param_changed {name}"));
                }
                EventSource::Cross { e, ttol, etol } => {
                    text.push_str("cross ");
                    text.push_str(&self.render(scope, Piece::Expr(*e)));
                    for (name, tol) in [("ttol", ttol), ("etol", etol)] {
                        if let Some(tol) = tol {
                            text.push_str(&format!(" {name} {}", self.render(scope, Piece::Expr(*tol))));
                        }
                    }
                }
                EventSource::At(e) => {
                    text.push_str("at ");
                    text.push_str(&self.render(scope, Piece::Expr(*e)));
                }
            }
            if let Some(guard) = event.guard {
                text.push_str(" guard ");
                text.push_str(&self.render(scope, Piece::Expr(guard)));
            }
            text.push_str(&self.at(&event.origin));
            self.line(1, &text);
        }
        if let Some(analog) = &body.analog {
            self.line(1, "analog");
            for &stmt in analog {
                let text = self.render(scope, Piece::Stmt(stmt));
                self.line(2, &text);
            }
        }
        for (k, reaction) in body.reactions.iter().enumerate() {
            let on: Vec<String> = reaction.on.iter().map(|e| format!("E{e}")).collect();
            let at = self.at(&reaction.origin);
            self.line(1, &format!("reaction R{k} on ({}){at}", on.join(", ")));
            self.locals(2, &reaction.locals);
            let scope = Scope { locals: &reaction.locals, ..scope };
            for &stmt in &reaction.stmts {
                let text = self.render(scope, Piece::Stmt(stmt));
                self.line(2, &text);
            }
        }
    }

    /// A param value of a placement: a constant as written, an expression in the parent's scope.
    fn child_value(&mut self, scope: Scope<'_>, value: &ChildValue) -> String {
        match value {
            ChildValue::Const(c) => c.to_string(),
            ChildValue::Expr(e) => self.render(scope, Piece::Expr(*e)),
        }
    }

    fn function(&mut self, function: &FunctionData) {
        let params: Vec<String> = function.params.iter().map(|p| format!("{}: {}", Printer::name(&p.name), p.ty)).collect();
        let at = self.at(&function.origin);
        let instance = function.instance.as_ref().map(|instance| format!("  // instance of {}", instance.item)).unwrap_or_default();
        self.line(0, &format!("fn {}({}) -> {}{at}{instance}", Printer::name(&function.name), params.join(", "), function.ret));
        self.locals(1, &function.locals);
        let scope = Scope { locals: &function.locals, params: &[], nodes: &[], carriers: &[], streams: &[] };
        let body = function.body;
        // A block body prints one statement per line. In the full text its id is printed and on
        // the stack meanwhile. In the structural form each line is a statement of its own,
        // guarded by its own walk, and a block whose type its value does not imply prints whole,
        // with its type.
        let lines = match self.ir.exprs.get(body.index()) {
            Some(ExprData { kind: Expr::Block { stmts, value }, ty, .. }) => {
                let fresh = if self.structural { *ty == Printer::block_type(self.ir, value) } else { !self.printed.contains(&body) };
                fresh.then_some((stmts, value))
            }
            _ => None,
        };
        let Some((stmts, value)) = lines else {
            let text = self.render(scope, Piece::Expr(body));
            self.line(1, &text);
            return;
        };
        if !self.structural {
            self.printed.insert(body);
            self.on_stack.insert(body);
        }
        for &stmt in stmts {
            let text = self.render(scope, Piece::Stmt(stmt));
            self.line(1, &text);
        }
        if let Some(value) = value {
            let text = self.render(scope, Piece::Expr(*value));
            self.line(1, &text);
        }
        if !self.structural {
            self.on_stack.remove(&body);
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
        let at = self.at(&instance.origin);
        let nets = if instance.nets.is_empty() {
            String::new()
        } else {
            let nets: Vec<String> = instance
                .nets
                .iter()
                .map(|id| self.ir.nets.get(id.index()).map_or(format!("<bad net {}>", id.0), |n| Printer::name(&n.path)))
                .collect();
            format!("  nets ({})", nets.join(", "))
        };
        self.line(
            0,
            &format!(
                "instance {}: {body_name}  nodes ({})  params ({}){nets}{top}{at}",
                Printer::name(&instance.path),
                nodes.join(", "),
                params.join(", ")
            ),
        );
    }

    fn terminal(scope: Scope<'_>, t: Terminal) -> String {
        match t {
            Terminal::Ground => "gnd".to_string(),
            Terminal::Node(i) => scope.nodes.get(i as usize).map_or(format!("<bad node {i}>"), |n| Printer::name(&n.name)),
            Terminal::Own(k) => format!("own{k}"),
        }
    }

    fn carrier(scope: Scope<'_>, i: u32) -> String {
        scope.carriers.get(i as usize).map_or(format!("<bad carrier {i}>"), |c| Printer::name(&c.name))
    }

    /// What a commit names inside its carrier, as pieces after the carrier's name: `[element]`, then `.k` for a field
    /// and `[index]` for an element of a value array.
    fn target(next: &mut Vec<Piece>, target: &CommitTarget) {
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
    fn drive_target_key(drive: &Drive) -> String {
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
    fn target_key(target: &CommitTarget) -> String {
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
    fn place(scope: Scope<'_>, place: &Place) -> Vec<Piece> {
        let mut pieces = vec![Piece::Text(Printer::local(scope, place.local))];
        for step in &place.path {
            match step {
                Projection::Field(k) => pieces.push(Piece::Text(format!(".{k}"))),
                Projection::Index(index) => pieces.extend([Piece::Text("[".into()), Piece::Expr(*index), Piece::Text("]".into())]),
            }
        }
        pieces
    }

    fn local(scope: Scope<'_>, i: u32) -> String {
        scope.locals.get(i as usize).map_or(format!("<bad local {i}>"), |l| Printer::name(&l.name))
    }

    // ---- the tree of one statement or expression -------------------------------------------

    /// The text of `root`, written by an explicit stack.
    fn render(&mut self, scope: Scope<'_>, root: Piece) -> String {
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
            Stmt::Emit { stream, fields } => {
                let name = scope.streams.get(*stream as usize).map_or(format!("<bad stream {stream}>"), |s| Printer::name(&s.name));
                next.push(Piece::Text(format!("emit {name}(")));
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        next.push(Piece::Text(", ".into()));
                    }
                    match f {
                        EmitField::Value { field, value } => {
                            let field = scope.streams.get(*stream as usize).and_then(|s| s.fields.get(*field as usize));
                            next.push(Piece::Text(format!("{} = ", field.map_or("<bad field>".to_string(), |f| Printer::name(&f.name)))));
                            next.push(Piece::Expr(*value));
                        }
                        EmitField::Text { name, text } => next.push(Piece::Text(format!("{} = {text:?}", Printer::name(name)))),
                    }
                }
                next.push(Piece::Text(")".into()));
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

    /// A loop body: `head { s0; s1 }`, the statements written in order.
    fn statements(next: &mut Vec<Piece>, head: &str, stmts: &[StmtId]) {
        next.push(Piece::Text(if head.is_empty() { "{ ".to_string() } else { format!("{head} {{ ") }));
        for (i, &s) in stmts.iter().enumerate() {
            if i > 0 {
                next.push(Piece::Text("; ".into()));
            }
            next.push(Piece::Stmt(s));
        }
        next.push(Piece::Text(" }".into()));
    }

    fn expr(&mut self, scope: Scope<'_>, id: ExprId, next: &mut Vec<Piece>) {
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
    fn reference(&self, id: ExprId) -> String {
        match self.numbers.get(&id) {
            Some(k) if self.structural => format!("^{k}"),
            _ => format!("e{}", id.0),
        }
    }

    /// The type of a block with `value`: the value's recorded type, `()` without one.
    fn block_type(ir: &Ir, value: &Option<ExprId>) -> ValueType {
        match value {
            Some(v) => ir.exprs.get(v.index()).map_or(ValueType::Unit, |e| e.ty.clone()),
            None => ValueType::Unit,
        }
    }

    /// The type an expression's form implies, given the recorded types of its operands; `None`
    /// when an id it needs is out of range or the operands fit no rule. An operand outside `f64`
    /// and `bool`, or an operator M1 does not have, selects the digital rules of `Typing`.
    fn implied(&self, scope: Scope<'_>, data: &ExprData) -> Option<ValueType> {
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
    fn implied_m1(&self, scope: Scope<'_>, kind: &Expr) -> Option<ValueType> {
        let ir = self.ir;
        let ty = |id: &ExprId| ir.exprs.get(id.index()).map(|e| e.ty.clone());
        match kind {
            Expr::Env(Env::Static | Env::Analysis(_)) | Expr::Fired(_) => Some(ValueType::Bool),
            Expr::Real(_) | Expr::Math { .. } | Expr::Table { .. } | Expr::TableSlope { .. } | Expr::Probe { .. } | Expr::Env(_) | Expr::Limit { .. } | Expr::Stim { .. } | Expr::Flow { .. } | Expr::Derivative(_) => {
                Some(ValueType::F64)
            }
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
            Expr::Carrier(_) | Expr::Previous(_) | Expr::CarrierElement { .. } | Expr::ElementPrevious { .. } | Expr::ElementChanged { .. } => None,
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

    // ---- the arenas -------------------------------------------------------------------------

    /// The id-level part of the text: what the tree shows by name or not at all.
    fn arenas(&mut self) {
        let ir = self.ir;
        self.line(0, "arenas");
        self.line(1, &format!("top {}", ir.top.0));
        let terminal = |t: &Terminal| match t {
            Terminal::Node(i) => i.to_string(),
            Terminal::Ground => "gnd".to_string(),
            Terminal::Own(k) => format!("own{k}"),
        };
        for (i, body) in ir.bodies.iter().enumerate() {
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
        for (i, function) in ir.functions.iter().enumerate() {
            self.line(1, &format!("fn {i}: body e{}", function.body.0));
        }
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
        for (i, node) in ir.nodes.iter().enumerate() {
            self.line(1, &format!("node {i}: nature {}  declared by {}", node.nature.0, node.declared_by.0));
        }
        for (i, net) in ir.nets.iter().enumerate() {
            self.line(1, &format!("net {i}: declared by {}", net.declared_by.0));
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
            Expr::CarrierElement { carrier, index } => format!("carrier_element {carrier} {}", e(index)),
            Expr::ElementPrevious { carrier, index } => format!("element_previous {carrier} {}", e(index)),
            Expr::ElementChanged { carrier, index } => format!("element_changed {carrier} {}", e(index)),
            Expr::Derivative(k) => format!("derivative D{k}"),
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
    fn stmt_ids(stmts: &[StmtId]) -> String {
        stmts.iter().map(|s| format!("s{}", s.0)).collect::<Vec<_>>().join(", ")
    }

    /// A statement's form with its operands as ids.
    fn stmt_form(stmt: &Stmt) -> String {
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
            BinaryOp::Rem => "%",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
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
            BinaryOp::Rem => "rem",
            BinaryOp::BitAnd => "bitand",
            BinaryOp::BitOr => "bitor",
            BinaryOp::BitXor => "bitxor",
            BinaryOp::Shl => "shl",
            BinaryOp::Shr => "shr",
        }
    }

    fn method_name(method: Method) -> &'static str {
        method.name()
    }

    /// The samples of a table model as `[(a, v), ..]`, each number as `real` writes it.
    fn samples(table: &[(Real, Real)]) -> String {
        let pairs: Vec<String> = table.iter().map(|(abscissa, value)| format!("({}, {})", Printer::real(*abscissa), Printer::real(*value))).collect();
        format!("[{}]", pairs.join(", "))
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
            Env::Time => "time",
            Env::Static => "static",
            Env::Freq => "freq",
            Env::Analysis(_) => "analysis",
        }
    }
}
