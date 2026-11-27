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
use std::fmt::{self, Display};

use crate::ids::{AggregateId, WideId, ExprId, FunctionId, StmtId, TypeId};
use crate::model::{BodyData, BodyNode, Carrier, Constant, FieldsForm, Ir, Local, Origin, Param, Quantity, Stream, ReprForm, TypeData, TypeKind, SessionParameter, ValueType};
use crate::real::Real;

impl Display for Ir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&Printer::new(self, false).text())
    }
}

impl Ir {
    /// The structural form of this IR (IR-11): the tree of the printed text with every
    /// expression's type where its form does not imply it, and no origin or arena id. Two IRs
    /// that differ only in origins or in the order of their arenas print it byte-identically.
    /// Kept for a host that prints the structure of an `Ir` without its expressions (`tests/structure.rs`).
    pub fn print_structural(&self) -> String {
        Printer::new(self, true).text()
    }
}

impl Ir {
    /// The `where` expression of param `param` of `body`, as the printer writes it after the
    /// word `where`; `None` for a param with no check or an index out of range.
    pub(crate) fn param_check_text(&self, body: &BodyData, param: usize) -> Option<String> {
        let check = body.params.get(param)?.check?;
        let scope = Scope { locals: &body.locals, params: &body.params, nodes: &body.nodes, carriers: &body.carriers, streams: &body.streams, session_parameters: &body.session_parameters };
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
    session_parameters: &'a [SessionParameter],
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

/// The state of one print: the IR, the text so far, and what the walk has already written.
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
    /// A printer of `ir`, in the structural form or the full text.
    fn new(ir: &'a Ir, structural: bool) -> Self {
        Printer { ir, out: String::new(), structural, printed: HashSet::new(), on_stack: HashSet::new(), numbers: HashMap::new() }
    }

    /// The printed text of the whole IR.
    fn text(mut self) -> String {
        self.ir();
        self.out
    }

    /// `  @origin` after an item in the full text; nothing in the structural form.
    fn at(&self, origin: &Origin) -> String {
        if self.structural { String::new() } else { format!("  @{origin}") }
    }

    /// `  resolved by <resolver> of <driven> drives (<net>, ..) held by (<instance>#<carrier>, ..) others (<k>: <net>, ..)` after a
    /// resolved net: the paths of what the net is made of (RN-03).
    fn resolution(&self, resolution: &crate::model::Resolution) -> String {
        let ir = self.ir;
        let net = |id: &crate::ids::NetId| ir.nets.get(id.index()).map_or("<bad net>".to_string(), |n| Printer::name(&n.path));
        let instance = |id: &crate::ids::InstanceId| ir.instances.get(id.index()).map_or("<bad instance>".to_string(), |i| Printer::name(&i.path));
        let resolver = instance(&resolution.resolver);
        let drives: Vec<String> = resolution.drives.iter().map(net).collect();
        let holders: Vec<String> = resolution.holders.iter().map(|holder| holder.as_ref().map_or("host".to_string(), |held| format!("{}#{}", instance(&held.instance), held.carrier))).collect();
        let others: Vec<String> = resolution.others.iter().map(|(drive, id)| format!("{drive}: {}", net(id))).collect();
        format!("  resolved by {resolver} of {} drives ({}) held by ({}) others ({})", resolution.driven, drives.join(", "), holders.join(", "), others.join(", "))
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

    /// One line of `text` at `indent` levels of two spaces.
    fn line(&mut self, indent: usize, text: &str) {
        for _ in 0..indent {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// The tree: natures, types, aggregates, bodies, functions, instances, nodes and nets, then the arenas in the full text.
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
                let resolution = net.resolution.as_ref().map_or(String::new(), |r| self.resolution(r));
                self.chunk(|p| p.line(0, &format!("net {}: {}  declared by {declarer}{init}{resolution}{at}", Printer::name(&net.path), net.ty)))
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

    /// The name of nature `i`, or `<bad nature>`.
    fn nature_name(&self, i: usize) -> String {
        self.ir.natures.get(i).map_or("<bad nature>".to_string(), |n| Printer::name(&n.name))
    }

    /// A nature's potential or flow quantity: name, access, units and absolute tolerance.
    fn quantity(q: &Quantity) -> String {
        format!("{} {} {:?} abstol {}", Printer::name(&q.name), Printer::name(&q.access), q.units, Printer::real(q.abstol))
    }
}

mod arenas;
mod body;
mod expression;
