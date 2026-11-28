//! `ShapePrinter`: any shape as the `ShapeBuf` calls that build it again. One call per pin,
//! term, input, watch, function and reaction, in the shape's order so each entry keeps its
//! index, each under a comment with the entry's index, name and origin. ABI constants print by
//! their names (`role::PARAM`, `part::VALUE`), a shipped layout by its static (`F64`), and each
//! cell and partial carries the name of what it addresses. The printer records every ABI item it
//! names, so the file that holds the calls imports exactly those.

use std::collections::BTreeSet;

use piperine_abi::layout::{BIT, BYTE, F64, NONE, layout_eq};
use piperine_abi::raw::{
    Cell, DEFAULT, Field, GROUND, Layout, Partial, Shape, Statement, ambient, cell, fn_flag, global, input,
    kind, leaf, op, part, pin_flag, reaction_flag, role, statement_flag, watch,
};

use crate::print::Text;

/// A named `u32` constant of the ABI.
type Named = (&'static str, u32);

const KINDS: &[Named] = &[("CONTINUOUS", kind::CONTINUOUS), ("DISCRETE", kind::DISCRETE), ("EVENT", kind::EVENT)];
const ROLES: &[Named] = &[("PORT", role::PORT), ("PARAM", role::PARAM), ("GLOBAL", role::GLOBAL), ("PRIVATE", role::PRIVATE)];
const GLOBALS: &[Named] = &[
    ("TEMPERATURE", global::TEMPERATURE),
    ("RESOLUTION", global::RESOLUTION),
    ("DC", global::DC),
    ("TRAN", global::TRAN),
    ("AC", global::AC),
    ("NOISE", global::NOISE),
    ("START", global::START),
    ("SETTLE", global::SETTLE),
    ("NAMED", global::NAMED),
    ("TEMPERATURE_GIVEN", global::TEMPERATURE_GIVEN),
];
const PIN_FLAGS: &[Named] = &[
    ("STRUCTURAL", pin_flag::STRUCTURAL),
    ("OPTIONAL", pin_flag::OPTIONAL),
    ("HAS_INIT", pin_flag::HAS_INIT),
    ("HAS_CHECK", pin_flag::HAS_CHECK),
    ("QUIET", pin_flag::QUIET),
    ("RECORD", pin_flag::RECORD),
    ("RELAX", pin_flag::RELAX),
];
const LEAVES: &[Named] = &[
    ("BOOL", leaf::BOOL),
    ("U8", leaf::U8),
    ("U16", leaf::U16),
    ("U32", leaf::U32),
    ("U64", leaf::U64),
    ("I8", leaf::I8),
    ("I16", leaf::I16),
    ("I32", leaf::I32),
    ("I64", leaf::I64),
    ("F32", leaf::F32),
    ("F64", leaf::F64),
];
const OPS: &[Named] = &[("FLOW", op::FLOW), ("DDT", op::DDT), ("DELAY", op::DELAY)];
const INPUTS: &[Named] = &[("LIMIT", input::LIMIT), ("NOISE", input::NOISE), ("STIM", input::STIM)];
const WATCHES: &[Named] = &[("VAR", watch::VAR), ("CROSS", watch::CROSS), ("CAP", watch::CAP), ("TRACE", watch::TRACE)];
const CELLS: &[Named] =
    &[("PIN", cell::PIN), ("TERM", cell::TERM), ("INPUT", cell::INPUT), ("WATCH", cell::WATCH), ("AMBIENT", cell::AMBIENT)];
const PARTS: &[Named] = &[
    ("VALUE", part::VALUE),
    ("PREVIOUS", part::PREVIOUS),
    ("CHANGED", part::CHANGED),
    ("TIME", part::TIME),
    ("LAST", part::LAST),
    ("AGE", part::AGE),
    ("PROPOSED", part::PROPOSED),
    ("LIMITED", part::LIMITED),
    ("PSD", part::PSD),
    ("RE", part::RE),
    ("IM", part::IM),
];
const AMBIENTS: &[Named] = &[
    ("TIME", ambient::TIME),
    ("FREQ", ambient::FREQ),
    ("STATIC", ambient::STATIC),
    ("GMIN", ambient::GMIN),
    ("SOURCE_SCALE", ambient::SOURCE_SCALE),
    ("TICK", ambient::TICK),
    ("HORIZON", ambient::HORIZON),
];
const FN_FLAGS: &[Named] = &[("OWNS_CORRECTION", fn_flag::OWNS_CORRECTION)];
const REACTION_FLAGS: &[Named] = &[("IRREVERSIBLE", reaction_flag::IRREVERSIBLE)];
const STATEMENT_FLAGS: &[Named] = &[("LATER", statement_flag::LATER), ("REPLACE", statement_flag::REPLACE)];

/// Prints shapes as `ShapeBuf` calls and records the ABI items the calls name.
#[derive(Debug, Default)]
pub struct ShapePrinter {
    uses: BTreeSet<&'static str>,
}

impl ShapePrinter {
    pub fn new() -> ShapePrinter {
        ShapePrinter::default()
    }

    /// The paths of every ABI item a printed shape names, sorted: what its file imports.
    pub fn uses(&self) -> &BTreeSet<&'static str> {
        &self.uses
    }

    /// `shape` as the statements that build it: `let mut s = ShapeBuf::new();`, one call per
    /// entry under its comment, and `s.freeze()` as the value.
    pub fn prototype(&mut self, shape: &Shape<'_>) -> String {
        self.uses.insert("piperine_abi::shape::ShapeBuf");
        let mut t = Text::new();
        t.line("let mut s = ShapeBuf::new();");
        for (i, p) in shape.pins.iter().enumerate() {
            t.line(&format!("// pin {i} `{}` @ {}", p.name.as_str(), p.origin.as_str()));
            t.line(&format!("s.pin({});", self.pin(p)));
        }
        for (i, x) in shape.terms.iter().enumerate() {
            t.line(&format!("// term {i} `{}` @ {}", x.name.as_str(), x.origin.as_str()));
            self.uses.insert("piperine_abi::shape::TermSpec");
            let (plus, minus) = (self.pin_index(x.plus), self.pin_index(x.minus));
            let (abstol, max) = (self.pin_index(x.abstol), self.pin_index(x.max));
            t.line(&format!(
                "s.term(TermSpec {{ name: {:?}, origin: {:?}, op: {}, plus: {plus}, minus: {minus}, abstol: {abstol}, max: {max} }});",
                x.name.as_str(),
                x.origin.as_str(),
                self.named("op", OPS, x.op),
            ));
        }
        for (i, x) in shape.inputs.iter().enumerate() {
            t.line(&format!("// input {i} `{}` @ {}", x.name.as_str(), x.origin.as_str()));
            self.uses.insert("piperine_abi::shape::InputSpec");
            t.line(&format!(
                "s.input(InputSpec {{ name: {:?}, origin: {:?}, kind: {} }});",
                x.name.as_str(),
                x.origin.as_str(),
                self.named("input", INPUTS, x.kind),
            ));
        }
        for (i, w) in shape.watches.iter().enumerate() {
            t.line(&format!("// watch {i} `{}` @ {}", w.name.as_str(), w.origin.as_str()));
            self.uses.insert("piperine_abi::shape::WatchSpec");
            let layout = self.layout(&w.layout);
            let (ttol, etol) = (self.pin_index(w.ttol), self.pin_index(w.etol));
            t.line(&format!(
                "s.watch(WatchSpec {{ name: {:?}, origin: {:?}, kind: {}, layout: {layout}, event: {}, ttol: {ttol}, etol: {etol} }});",
                w.name.as_str(),
                w.origin.as_str(),
                self.named("watch", WATCHES, w.kind),
                w.event,
            ));
        }
        for (i, f) in shape.functions.iter().enumerate() {
            t.line(&format!("// function {i} `{}` @ {}", f.name.as_str(), f.origin.as_str()));
            self.uses.insert("piperine_abi::shape::FunctionSpec");
            t.line("s.function(FunctionSpec {");
            t.indented(|t| {
                t.line(&format!("name: {:?},", f.name.as_str()));
                t.line(&format!("origin: {:?},", f.origin.as_str()));
                if f.flags != 0 {
                    t.line(&format!("flags: {},", self.flags("fn_flag", FN_FLAGS, f.flags)));
                }
                let reads: Vec<Cell> = f.reads.iter().copied().collect();
                let writes: Vec<Cell> = f.writes.iter().copied().collect();
                self.cells(t, shape, "reads", &reads);
                self.cells(t, shape, "writes", &writes);
                let partials: Vec<Partial> = f.partials.iter().copied().collect();
                self.partials(t, shape, &reads, &writes, &partials);
                Self::elements(t, &f.elements.iter().copied().collect::<Vec<u64>>());
                t.line("..FunctionSpec::default()");
            });
            t.line("});");
        }
        for (i, r) in shape.reactions.iter().enumerate() {
            t.line(&format!("// reaction {i} `{}` @ {}", r.name.as_str(), r.origin.as_str()));
            self.uses.insert("piperine_abi::shape::ReactionSpec");
            t.line("s.reaction(ReactionSpec {");
            t.indented(|t| {
                t.line(&format!("name: {:?},", r.name.as_str()));
                t.line(&format!("origin: {:?},", r.origin.as_str()));
                if r.flags != 0 {
                    t.line(&format!("flags: {},", self.flags("reaction_flag", REACTION_FLAGS, r.flags)));
                }
                let on: Vec<String> = r.on.iter().map(|p| format!("{p} /* {} */", Self::pin_name(shape, *p))).collect();
                t.line(&format!("on: vec![{}],", on.join(", ")));
                let reads: Vec<Cell> = r.reads.iter().copied().collect();
                self.cells(t, shape, "reads", &reads);
                let statements: Vec<Statement> = r.writes.iter().copied().collect();
                self.statements(t, shape, &statements);
                Self::elements(t, &r.elements.iter().copied().collect::<Vec<u64>>());
                t.line("..ReactionSpec::default()");
            });
            t.line("});");
        }
        t.line("s.freeze()");
        let mut text = t.finish();
        text.pop();
        text
    }

    /// The `elements:` field of a function or reaction: one element per read (`u64::MAX` is a `WHOLE` read), omitted
    /// when no read names an element.
    fn elements(t: &mut Text, elements: &[u64]) {
        if elements.is_empty() {
            return;
        }
        let each: Vec<String> = elements.iter().map(|e| if *e == u64::MAX { "piperine_abi::raw::WHOLE".to_string() } else { e.to_string() }).collect();
        t.line(&format!("elements: vec![{}],", each.join(", ")));
    }

    /// The arguments of `s.pin(…)`: the base constructor of the pin's kind, with every field
    /// that differs from it.
    fn pin(&mut self, p: &piperine_abi::raw::Pin<'_>) -> String {
        self.uses.insert("piperine_abi::shape::PinSpec");
        let (name, role) = (p.name.as_str(), self.named("role", ROLES, p.role));
        let base = if p.kind == kind::CONTINUOUS && Self::shipped(&p.layout) == Some("F64") {
            format!("PinSpec::continuous({name:?}, {role})")
        } else if p.kind == kind::EVENT && Self::shipped(&p.layout) == Some("NONE") {
            format!("PinSpec::event({name:?}, {role})")
        } else {
            format!("PinSpec::discrete({name:?}, {role}, {})", self.layout(&p.layout))
        };
        let mut fields = vec![format!("origin: {:?}", p.origin.as_str())];
        let base_kind = if base.starts_with("PinSpec::continuous") {
            kind::CONTINUOUS
        } else if base.starts_with("PinSpec::event") {
            kind::EVENT
        } else {
            kind::DISCRETE
        };
        if p.kind != base_kind {
            fields.push(format!("kind: {}", self.named("kind", KINDS, p.kind)));
        }
        if p.global != 0 {
            fields.push(format!("global: {}", self.named("global", GLOBALS, p.global)));
        }
        if p.flags != 0 {
            fields.push(format!("flags: {}", self.flags("pin_flag", PIN_FLAGS, p.flags)));
        }
        if !p.unit.is_empty() {
            fields.push(format!("unit: {:?}", p.unit.as_str()));
        }
        if !p.init.is_empty() {
            fields.push(format!("init: {}", Self::bytes(&p.layout, p.init.as_slice())));
        }
        if p.check != 0 {
            fields.push(format!("check: {}", p.check));
        }
        if p.count != 0 {
            fields.push(format!("count: {}", p.count));
        }
        if p.abstol.iter().any(|a| a.to_bits() != 0) {
            fields.push(format!("abstol: [{}, {}]", Text::real(p.abstol[0]), Text::real(p.abstol[1])));
        }
        format!("PinSpec {{ {}, ..{base} }}", fields.join(", "))
    }

    /// The name of a shipped layout `l` equals, field names included; `None` for any other.
    fn shipped(l: &Layout<'_>) -> Option<&'static str> {
        let same = |s: &Layout<'_>| {
            layout_eq(l, s) && l.fields.iter().zip(s.fields.iter()).all(|(a, b)| a.name.as_str() == b.name.as_str())
        };
        [("F64", &F64), ("BIT", &BIT), ("BYTE", &BYTE), ("NONE", &NONE)]
            .into_iter()
            .find_map(|(name, s)| same(s).then_some(name))
    }

    /// `&F64` for a shipped layout; a `Layout` literal for any other.
    fn layout(&mut self, l: &Layout<'_>) -> String {
        if let Some(name) = Self::shipped(l) {
            self.uses.insert(match name {
                "F64" => "piperine_abi::layout::F64",
                "BIT" => "piperine_abi::layout::BIT",
                "BYTE" => "piperine_abi::layout::BYTE",
                _ => "piperine_abi::layout::NONE",
            });
            return format!("&{name}");
        }
        for path in ["piperine_abi::raw::Layout", "piperine_abi::raw::List", "piperine_abi::raw::Field", "piperine_abi::raw::Str"] {
            self.uses.insert(path);
        }
        let fields: Vec<String> = l.fields.iter().map(|f| self.field(f)).collect();
        format!("&Layout {{ size: {}, align: {}, fields: List::from_slice(&[{}]) }}", l.size, l.align, fields.join(", "))
    }

    fn field(&mut self, f: &Field<'_>) -> String {
        format!(
            "Field {{ name: Str::from_static({:?}), leaf: {}, offset: {}, count: {} }}",
            f.name.as_str(),
            self.named("leaf", LEAVES, f.leaf),
            f.offset,
            f.count,
        )
    }

    /// Bytes of a value in `layout`: an `F64` as the real it holds, any other as a byte array.
    fn bytes(layout: &Layout<'_>, bytes: &[u8]) -> String {
        match <[u8; 8]>::try_from(bytes) {
            Ok(b) if Self::shipped(layout) == Some("F64") => format!("&f64::to_le_bytes({})", Text::real(f64::from_le_bytes(b))),
            _ => format!("&{bytes:?}"),
        }
    }

    /// A pin index, `GROUND` or `DEFAULT`.
    fn pin_index(&mut self, i: u32) -> String {
        match i {
            GROUND => {
                self.uses.insert("piperine_abi::raw::GROUND");
                "GROUND".to_string()
            }
            DEFAULT => {
                self.uses.insert("piperine_abi::raw::DEFAULT");
                "DEFAULT".to_string()
            }
            _ => i.to_string(),
        }
    }

    /// `group::NAME` for a known value, the number for any other.
    fn named(&mut self, group: &'static str, table: &[Named], value: u32) -> String {
        match table.iter().find(|(_, v)| *v == value) {
            Some((name, _)) => {
                self.uses.insert(Self::module(group));
                format!("{group}::{name}")
            }
            None => value.to_string(),
        }
    }

    /// Flag bits as `group::A | group::B`, any unnamed bits as a number; `0` when none.
    fn flags(&mut self, group: &'static str, table: &[Named], bits: u32) -> String {
        let mut parts = Vec::new();
        let mut left = bits;
        for (name, bit) in table {
            if bits & bit != 0 {
                parts.push(format!("{group}::{name}"));
                left &= !bit;
            }
        }
        if !parts.is_empty() {
            self.uses.insert(Self::module(group));
        }
        if left != 0 || parts.is_empty() {
            parts.push(format!("{left:#x}"));
        }
        parts.join(" | ")
    }

    fn module(group: &'static str) -> &'static str {
        match group {
            "kind" => "piperine_abi::raw::kind",
            "role" => "piperine_abi::raw::role",
            "global" => "piperine_abi::raw::global",
            "pin_flag" => "piperine_abi::raw::pin_flag",
            "leaf" => "piperine_abi::raw::leaf",
            "op" => "piperine_abi::raw::op",
            "input" => "piperine_abi::raw::input",
            "watch" => "piperine_abi::raw::watch",
            "cell" => "piperine_abi::raw::cell",
            "part" => "piperine_abi::raw::part",
            "ambient" => "piperine_abi::raw::ambient",
            "fn_flag" => "piperine_abi::raw::fn_flag",
            "reaction_flag" => "piperine_abi::raw::reaction_flag",
            _ => "piperine_abi::raw::statement_flag",
        }
    }

    fn pin_name(shape: &Shape<'_>, i: u32) -> String {
        shape.pins.get(i as usize).map_or_else(|| format!("pin {i}"), |p| p.name.as_str().to_string())
    }

    /// What a cell addresses, as a reader names it: `a`, `b0.flow`, `vd.LAST`, `GMIN`.
    fn cell_name(shape: &Shape<'_>, c: &Cell) -> String {
        let entry = match c.kind {
            cell::PIN => Self::pin_name(shape, c.index),
            cell::TERM => shape.terms.get(c.index as usize).map_or_else(|| format!("term {}", c.index), |x| x.name.as_str().to_string()),
            cell::INPUT => shape.inputs.get(c.index as usize).map_or_else(|| format!("input {}", c.index), |x| x.name.as_str().to_string()),
            cell::WATCH => shape.watches.get(c.index as usize).map_or_else(|| format!("watch {}", c.index), |x| x.name.as_str().to_string()),
            _ => AMBIENTS.iter().find(|(_, v)| *v == c.index).map_or_else(|| format!("ambient {}", c.index), |(n, _)| n.to_string()),
        };
        if c.part == part::VALUE {
            entry
        } else {
            let part = PARTS.iter().find(|(_, v)| *v == c.part).map_or_else(|| c.part.to_string(), |(n, _)| n.to_string());
            format!("{entry}.{part}")
        }
    }

    /// `field: vec![…],` one cell per line, each with what it addresses.
    fn cells(&mut self, t: &mut Text, shape: &Shape<'_>, field: &str, cells: &[Cell]) {
        if cells.is_empty() {
            t.line(&format!("{field}: vec![],"));
            return;
        }
        self.uses.insert("piperine_abi::raw::Cell");
        t.line(&format!("{field}: vec![",));
        t.indented(|t| {
            for c in cells {
                let index = if c.kind == cell::AMBIENT { self.named("ambient", AMBIENTS, c.index) } else { c.index.to_string() };
                t.line(&format!(
                    "Cell {{ kind: {}, index: {index}, part: {} }}, // {}",
                    self.named("cell", CELLS, c.kind),
                    self.named("part", PARTS, c.part),
                    Self::cell_name(shape, c),
                ));
            }
        });
        t.line("],");
    }

    /// `partials: vec![…],` one per line, each as the derivative it is.
    fn partials(&mut self, t: &mut Text, shape: &Shape<'_>, reads: &[Cell], writes: &[Cell], partials: &[Partial]) {
        if partials.is_empty() {
            t.line("partials: vec![],");
            return;
        }
        self.uses.insert("piperine_abi::raw::Partial");
        t.line("partials: vec![");
        t.indented(|t| {
            for p in partials {
                let of = writes.get(p.write as usize).map_or_else(|| format!("write {}", p.write), |c| Self::cell_name(shape, c));
                let by = reads.get(p.read as usize).map_or_else(|| format!("read {}", p.read), |c| Self::cell_name(shape, c));
                t.line(&format!(
                    "Partial {{ write: {}, to: {}, read: {}, at: {} }}, // d {of} / d {by}",
                    p.write, p.to, p.read, p.at
                ));
            }
        });
        t.line("],");
    }

    /// `writes: vec![…],` one statement per line.
    fn statements(&mut self, t: &mut Text, shape: &Shape<'_>, statements: &[Statement]) {
        if statements.is_empty() {
            t.line("writes: vec![],");
            return;
        }
        self.uses.insert("piperine_abi::raw::Statement");
        t.line("writes: vec![");
        t.indented(|t| {
            for s in statements {
                t.line(&format!(
                    "Statement {{ pin: {}, offset: {}, len: {}, flags: {}, min_after: {} }}, // {}",
                    s.pin,
                    s.offset,
                    s.len,
                    self.flags("statement_flag", STATEMENT_FLAGS, s.flags),
                    Text::real(s.min_after),
                    Self::pin_name(shape, s.pin),
                ));
            }
        });
        t.line("],");
    }
}

#[cfg(test)]
mod tests {
    use piperine_abi::layout::{BIT, F64};
    use piperine_abi::raw::{
        Cell, DEFAULT, Field, Layout, List, Shape, Statement, Str, cell, leaf, part, role, statement_flag, watch,
    };
    use piperine_abi::shape::{PinSpec, ReactionSpec, ShapeBuf, WatchSpec};
    use piperine_ir::IrBuilder;
    use piperine_plan::BodyPlan;

    use super::ShapePrinter;
    use piperine_ir_fixtures::devices::Devices;

    /// Associated test helpers of `shape`, grouped on one fixture type.
    struct ShapeFixture;

    impl ShapeFixture {
        /// Each entry's call: the comment line before it names the entry and its origin.
        fn entries(shape: &Shape<'_>) -> Vec<(&'static str, String, String)> {
            let mut out = Vec::new();
            out.extend(shape.pins.iter().map(|p| ("s.pin(", p.name.as_str().to_string(), p.origin.as_str().to_string())));
            out.extend(shape.terms.iter().map(|x| ("s.term(", x.name.as_str().to_string(), x.origin.as_str().to_string())));
            out.extend(shape.inputs.iter().map(|x| ("s.input(", x.name.as_str().to_string(), x.origin.as_str().to_string())));
            out.extend(shape.watches.iter().map(|x| ("s.watch(", x.name.as_str().to_string(), x.origin.as_str().to_string())));
            out.extend(shape.functions.iter().map(|x| ("s.function(", x.name.as_str().to_string(), x.origin.as_str().to_string())));
            out.extend(shape.reactions.iter().map(|x| ("s.reaction(", x.name.as_str().to_string(), x.origin.as_str().to_string())));
            out
        }

        /// The text holds one call per entry, in the shape's order, each on the line after a comment
        /// with the entry's name and origin, and no other call.
        fn one_call_per_entry(what: &str, shape: &Shape<'_>) {
            let text = ShapePrinter::new().prototype(shape);
            let lines: Vec<&str> = text.lines().collect();
            let calls: Vec<usize> = (0..lines.len()).filter(|i| lines[*i].trim_start().starts_with("s.")).collect();
            let expected = ShapeFixture::entries(shape);
            // `s.freeze()` closes the text; every other `s.` line is an entry's call
            assert_eq!(lines.last(), Some(&"s.freeze()"), "{what}");
            assert_eq!(calls.len(), expected.len() + 1, "{what}: calls\n{text}");
            for (at, (call, name, origin)) in calls.iter().zip(&expected) {
                assert!(lines[*at].starts_with(call), "{what}: line {at} is not `{call}…`: {}", lines[*at]);
                let comment = lines[at - 1];
                assert!(
                    comment.starts_with("// ") && comment.contains(&format!("`{name}`")) && comment.ends_with(&format!("@ {origin}")),
                    "{what}: the comment over `{name}` is `{comment}`"
                );
            }
            let comments = lines.iter().filter(|l| l.starts_with("// ")).count();
            assert_eq!(comments, expected.len(), "{what}: one comment per entry");
        }
    }

    #[test]
    fn every_shape_entry_is_one_call_under_its_comment() {
        let mut b = IrBuilder::new();
        let diode = Devices::diode(&mut b);
        let vsource = Devices::vsource(&mut b);
        let top = Devices::alone(&mut b, diode, "D", 1, vec![1e-14, 1.0, 0.0]);
        let ir = b.finish(top);
        for body in [diode, vsource] {
            let plan = BodyPlan::build(ir.body(body)).expect("the body plans");
            let shape = plan.shape().shape();
            // the diode: 3 params, 2 nodes, the temperature `vt()` reads and whether the run set it, 3 check pins; flow and
            // charge terms; one limit; the limit, terms and 3 check functions. The source: 2
            // params, 2 nodes, a branch flow pin; two FLOW terms; one stimulus; stim and terms
            let counts = (shape.pins.len(), shape.terms.len(), shape.inputs.len(), shape.functions.len());
            let expected = if body == diode { (10, 2, 1, 5) } else { (5, 2, 1, 2) };
            assert_eq!(counts, expected, "{}", ir.body(body).name());
            ShapeFixture::one_call_per_entry(ir.body(body).name(), shape);
        }

        // a shape with what M1 shapes lack: an event pin, a custom layout, a watch, a reaction
        let u9_fields =
            [Field { name: Str::from_static("v"), leaf: leaf::U16, offset: 0, count: 1 }];
        let u9 = Layout { size: 2, align: 2, fields: List::from_slice(&u9_fields) };
        let mut s = ShapeBuf::new();
        let clk = s.pin(PinSpec { origin: "dff.msl:1:20", ..PinSpec::discrete("clk", role::PORT, &BIT) });
        s.pin(PinSpec { origin: "dff.msl:2:5", ..PinSpec::discrete("count", role::PRIVATE, &u9) });
        let x = s.pin(PinSpec { origin: "dff.msl:1:40", ..PinSpec::continuous("x", role::PORT) });
        let ev = s.pin(PinSpec { origin: "dff.msl:3:5", ..PinSpec::event("event:0", role::PRIVATE) });
        s.watch(WatchSpec { name: "cross:0", origin: "dff.msl:3:9", kind: watch::CROSS, layout: &F64, event: ev, ttol: DEFAULT, etol: DEFAULT });
        s.reaction(ReactionSpec {
            name: "on clk",
            origin: "dff.msl:4:5",
            on: vec![ev],
            reads: vec![Cell { kind: cell::PIN, index: x, part: part::VALUE }],
            writes: vec![Statement { pin: clk, offset: 0, len: 1, flags: statement_flag::LATER, min_after: 1e-9 }],
            ..ReactionSpec::default()
        });
        let frozen = s.freeze();
        ShapeFixture::one_call_per_entry("the D flip-flop", frozen.shape());
        let text = ShapePrinter::new().prototype(frozen.shape());
        assert!(text.contains("PinSpec::event(\"event:0\", role::PRIVATE)"), "{text}");
        assert!(text.contains("flags: statement_flag::LATER, min_after: 1e-9"), "{text}");
        assert!(text.contains("leaf: leaf::U16, offset: 0, count: 1 }"), "{text}");
    }
}
