//! The IR printer — stable, diffable, byte-identical across runs (LOV-02,
//! LOV-06, LOV-08). The format is what makes hand-written IR fixtures
//! possible (Phase 3, T15+): a fixture is text the parser (T9) reads back
//! into a structurally-equivalent module, with no front-end involved.
//!
//! ## Provenance: `\user` vs `$generated`
//!
//! Adopted verbatim from Yosys RTLIL (`ideal.md` §3): every named entity —
//! value, net, cell, block — is printed with one of two prefixes:
//!
//! - `$N` — a compiler-generated name (the default). `N` is the entity's
//!   dense index in its arena, so the printed id is byte-stable across runs
//!   (Phase-1 arenas give dense ids in declaration order).
//! - `\name` — an authored name, supplied through the [`Names`] side-table.
//!   Authorship is metadata that rides alongside the IR; the IR core is
//!   unchanged (Phase-1 shapes stand — `Names` is opt-in side-storage).
//!
//! Both forms can appear in one module's output; the test in
//! `tests/print.rs` proves both forms appear when a module mingles them.
//!
//! ## The grammar (T9's parser consumes this verbatim)
//!
//! The format is S-expressions — trivially unambiguous and easy to parse
//! with a hand-written recursive-descent parser (T9). One module is one
//! top-level form. Whitespace is non-semantic; comments start with `;` and
//! run to end of line.
//!
//! ```text
//! module       := (module section*)
//! section      := natures | nets | cells | values | consts | blocks
//! natures      := (natures nature*)
//! nature       := (nature NID (access "...") (across|through))
//! nets         := (nets net*)
//! net          := (net id (node across=NID through=NID))
//!                | (net id (signal ty))
//! cells        := (cells cell*)
//! cell         := (cell id ty init=KID writer=BID)
//! values       := (values value*)
//! value        := (value id ty)
//! consts       := (consts const*)
//! const        := (const KID real REAL_LITERAL)
//!                | (const KID bits INT_LITERAL)
//! blocks       := (blocks block*)
//! block        := (block id (analog|digital) trigger op*)
//! trigger      := always | initial
//!                | (edge sigref (rising|falling|any))
//!                | (cross VID (rising|falling|either))
//!                | (timer VID VID)
//! sigref       := (chunk*)
//! chunk        := (slice VID lo INT hi INT) | (const KID)
//! op           := (VID = pure pureop VID*)
//!                | (VID = call INTRINSICID VID*)
//!                | (VID = read-across NID NID)
//!                | (VID = read-through NID NID)
//!                | (contribute-through NID NID VID)
//!                | (force-across NID NID VID)
//!                | (VID = read-signal NID)
//!                | (drive NID VID)
//!                | (VID = cell-read CID (addr VID)?)
//!                | (cell-write CID VID (addr VID)?)
//!                | (if VID (then op*) (else op*))
//!                | (match VID arm+)
//! arm          := ((0|1|?) op*)
//! pureop       := add | sub | mul | div | neg | eq | ne | lt | le | gt | ge | select
//! ty           := real | (bits INT) | (array ty INT)
//! id           := $N |\name
//! REAL_LITERAL := Rust {:[e]} format with 16-digit exponent (byte-stable)
//! INT_LITERAL  := Rust decimal u128
//! ```
//!
//! Sections appear in the order above; entities within a section appear in
//! arena order. The grammar is byte-identical across two runs on the same
//! module because every ordering is the arena's declaration order, never a
//! traversal that could vary.

use crate::core::{
    Block, BlockId, Cell, CellId, Chunk, Const, ConstId, CrossDir, Domain, EdgeKind, Module,
    NatureInfo, NatureKind, Net, NetKind, Op, Pattern, PureOp, SigRef, Trigger, Ty, ValueId,
    NetId, NatureId,
};

/// The provenance of a name in the printed IR — `\user` vs `$generated`
/// (ideal.md §3, the RTLIL convention). The default for every entity is
/// [`Name::Generated`]; authored names are set through the [`Names`]
/// side-table that the printer and parser share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Name {
    /// Compiler-generated: printed as `$N` where `N` is the dense index.
    Generated,
    /// Authored: printed as `\name`.
    Authored(String),
}

/// Side-storage mapping entity ids to authored names, so the Phase-1 core
/// shapes stand unchanged (no field added to `Cell`/`Net`/`Block`). One
/// table covers all four named id spaces — `ValueId`/`NetId`/`CellId`/
/// `BlockId` are distinct newtypes, never confused at lookup.
///
/// Absent means [`Name::Generated`] (the default). Setting a name records
/// it as [`Name::Authored`]. The printer consults the table; the parser
/// (T9) round-trips it back.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Names {
    values: Vec<Option<String>>,
    nets: Vec<Option<String>>,
    cells: Vec<Option<String>>,
    blocks: Vec<Option<String>>,
}

impl Names {
    /// An empty name table — every entity prints as `$N`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Author the name of value `id`.
    pub fn set_value(&mut self, id: ValueId, name: impl Into<String>) {
        let i = id.0 as usize;
        if self.values.len() <= i {
            self.values.resize(i + 1, None);
        }
        self.values[i] = Some(name.into());
    }

    /// Author the name of net `id`.
    pub fn set_net(&mut self, id: NetId, name: impl Into<String>) {
        let i = id.0 as usize;
        if self.nets.len() <= i {
            self.nets.resize(i + 1, None);
        }
        self.nets[i] = Some(name.into());
    }

    /// Author the name of cell `id`.
    pub fn set_cell(&mut self, id: CellId, name: impl Into<String>) {
        let i = id.0 as usize;
        if self.cells.len() <= i {
            self.cells.resize(i + 1, None);
        }
        self.cells[i] = Some(name.into());
    }

    /// Author the name of block `id`.
    pub fn set_block(&mut self, id: BlockId, name: impl Into<String>) {
        let i = id.0 as usize;
        if self.blocks.len() <= i {
            self.blocks.resize(i + 1, None);
        }
        self.blocks[i] = Some(name.into());
    }

    /// The authored name of value `id`, if any.
    pub fn value(&self, id: ValueId) -> Option<&str> {
        self.values.get(id.0 as usize).and_then(|n| n.as_deref())
    }

    /// The authored name of net `id`, if any.
    pub fn net(&self, id: NetId) -> Option<&str> {
        self.nets.get(id.0 as usize).and_then(|n| n.as_deref())
    }

    /// The authored name of cell `id`, if any.
    pub fn cell(&self, id: CellId) -> Option<&str> {
        self.cells.get(id.0 as usize).and_then(|n| n.as_deref())
    }

    /// The authored name of block `id`, if any.
    pub fn block(&self, id: BlockId) -> Option<&str> {
        self.blocks.get(id.0 as usize).and_then(|n| n.as_deref())
    }

    /// Find the value id authored as `name`, if any. Reverse lookup for
    /// the parser (T9): a `\name` reference at a use site resolves to its
    /// declaring id through this scan. Linear in the number of authored
    /// names — fine for parse-time over hand-written fixtures.
    pub fn find_value(&self, name: &str) -> Option<ValueId> {
        self.values
            .iter()
            .position(|n| n.as_deref() == Some(name))
            .map(|i| ValueId(i as u32))
    }

    /// Find the net id authored as `name`.
    pub fn find_net(&self, name: &str) -> Option<NetId> {
        self.nets
            .iter()
            .position(|n| n.as_deref() == Some(name))
            .map(|i| NetId(i as u32))
    }

    /// Find the cell id authored as `name`.
    pub fn find_cell(&self, name: &str) -> Option<CellId> {
        self.cells
            .iter()
            .position(|n| n.as_deref() == Some(name))
            .map(|i| CellId(i as u32))
    }

    /// Find the block id authored as `name`.
    pub fn find_block(&self, name: &str) -> Option<BlockId> {
        self.blocks
            .iter()
            .position(|n| n.as_deref() == Some(name))
            .map(|i| BlockId(i as u32))
    }
}

/// The printer — owns the algorithm that walks the arenas in id order and
/// formats the S-expression grammar above. Methods on this struct (not
/// free functions — MD-13 rule 2). A [`Module::print`] convenience
/// delegates here.
pub struct Printer<'a> {
    module: &'a Module,
    names: &'a Names,
    out: String,
    indent: usize,
}

impl<'a> Printer<'a> {
    /// Build a printer that walks `module`, consulting `names` for authored
    /// identifiers.
    pub fn new(module: &'a Module, names: &'a Names) -> Self {
        Self { module, names, out: String::new(), indent: 0 }
    }

    /// Print the module and return the formatted text.
    pub fn print(mut self) -> String {
        self.write_module();
        self.out
    }

    /// `(module ...)`.
    fn write_module(&mut self) {
        self.write_line("(module");
        self.indented(|s| {
            s.write_section("natures", |s| {
                for (id, info) in s.module.natures() {
                    s.write_nature(id, info);
                }
            });
            s.write_section("nets", |s| {
                for (id, net) in s.module.nets() {
                    s.write_net(id, net);
                }
            });
            s.write_section("cells", |s| {
                for (id, cell) in s.module.cells() {
                    s.write_cell(id, cell);
                }
            });
            s.write_section("values", |s| {
                for (id, ty) in s.module.values() {
                    s.write_value(id, ty);
                }
            });
            s.write_section("consts", |s| {
                for (id, c) in s.module.consts() {
                    s.write_const(id, c);
                }
            });
            s.write_section("blocks", |s| {
                for (id, block) in s.module.blocks() {
                    s.write_block(id, block);
                }
            });
        });
        self.write_line(")");
    }

    fn write_nature(&mut self, id: NatureId, info: &NatureInfo) {
        let kind = match info.kind {
            NatureKind::Across => "across",
            NatureKind::Through => "through",
        };
        self.write_line_fmt(
            format_args!("(nature N{} (access \"{}\") {})", id.0, escape_str(&info.access), kind),
        );
    }

    fn write_net(&mut self, id: NetId, net: &Net) {
        match &net.kind {
            NetKind::Node { across, through } => self.write_line_fmt(format_args!(
                "(net {} (node across=N{} through=N{}))",
                self.net_id_str(id),
                across.0,
                through.0
            )),
            NetKind::Signal { ty } => {
                let mut buf = String::new();
                Self::fmt_ty(&mut buf, ty);
                self.write_line_fmt(format_args!("(net {} (signal {}))", self.net_id_str(id), buf));
            }
        }
    }

    fn write_cell(&mut self, id: CellId, cell: &Cell) {
        let mut ty = String::new();
        Self::fmt_ty(&mut ty, &cell.ty);
        self.write_line_fmt(format_args!(
            "(cell {} {} init=K{} writer={})",
            self.cell_id_str(id),
            ty,
            cell.init.0,
            self.block_id_str(cell.writer),
        ));
    }

    fn write_value(&mut self, id: ValueId, ty: &Ty) {
        let mut buf = String::new();
        Self::fmt_ty(&mut buf, ty);
        self.write_line_fmt(format_args!("(value {} {})", self.value_id_str(id), buf));
    }

    fn write_const(&mut self, id: ConstId, c: &Const) {
        match c {
            Const::Real(v) => self.write_line_fmt(format_args!(
                "(const K{} real {:.16e})",
                id.0, v,
            )),
            Const::Bits(v) => self.write_line_fmt(format_args!(
                "(const K{} bits {})",
                id.0, v,
            )),
        }
    }

    fn write_block(&mut self, id: BlockId, block: &Block) {
        let domain = match block.domain {
            Domain::Analog => "analog",
            Domain::Digital => "digital",
        };
        // Header line: `(block <id> <domain> <trigger>` then ops on
        // subsequent lines and a closing `)` after the last op.
        let mut header = String::new();
        header.push_str("(block ");
        header.push_str(&self.block_id_str(id));
        header.push(' ');
        header.push_str(domain);
        header.push(' ');
        self.write_trigger(&mut header, &block.trigger);
        // Open the block body — write the header with no closing paren
        // yet, write the ops indented, then close.
        self.out.push_str(&"  ".repeat(self.indent));
        self.out.push_str(&header);
        self.out.push('\n');
        self.indent += 1;
        for op in block.ops() {
            let mut line = String::new();
            self.write_op(&mut line, op);
            self.out.push_str(&"  ".repeat(self.indent));
            self.out.push_str(&line);
            self.out.push('\n');
        }
        self.indent -= 1;
        self.write_line(")");
    }

    fn write_trigger(&self, out: &mut String, t: &Trigger) {
        match t {
            Trigger::Always => out.push_str("always"),
            Trigger::Initial => out.push_str("initial"),
            Trigger::Edge(sr, kind) => {
                let k = match kind {
                    EdgeKind::Rising => "rising",
                    EdgeKind::Falling => "falling",
                    EdgeKind::Any => "any",
                };
                let mut sref = String::new();
                self.write_sigref(&mut sref, sr);
                out.push_str("(edge ");
                out.push_str(&sref);
                out.push(' ');
                out.push_str(k);
                out.push(')');
            }
            Trigger::Cross(v, dir) => {
                let d = match dir {
                    CrossDir::Rising => "rising",
                    CrossDir::Falling => "falling",
                    CrossDir::Either => "either",
                };
                out.push_str("(cross ");
                out.push_str(&self.value_id_str(*v));
                out.push(' ');
                out.push_str(d);
                out.push(')');
            }
            Trigger::Timer { period, phase } => {
                out.push_str("(timer ");
                out.push_str(&self.value_id_str(*period));
                out.push(' ');
                out.push_str(&self.value_id_str(*phase));
                out.push(')');
            }
        }
    }

    fn write_sigref(&self, out: &mut String, sr: &SigRef) {
        out.push_str("(sigref");
        for chunk in &sr.0 {
            match chunk {
                Chunk::Slice { value, lo, hi } => {
                    out.push_str(&format!(" (slice {} lo {} hi {})", self.value_id_str(*value), lo, hi));
                }
                Chunk::Const(c) => {
                    out.push_str(&format!(" (const K{})", c.0));
                }
            }
        }
        out.push(')');
    }

    fn write_op(&mut self, out: &mut String, op: &Op) {
        match op {
            Op::Pure { op: pure, args, out: o } => {
                out.push('(');
                out.push_str(&self.value_id_str(*o));
                out.push_str(" = pure ");
                out.push_str(Self::fmt_pureop(*pure));
                for a in args {
                    out.push(' ');
                    out.push_str(&self.value_id_str(*a));
                }
                out.push(')');
            }
            Op::Call { intrinsic, args, out: o } => {
                out.push('(');
                out.push_str(&self.value_id_str(*o));
                out.push_str(&format!(" = call I{}", intrinsic.0));
                for a in args {
                    out.push(' ');
                    out.push_str(&self.value_id_str(*a));
                }
                out.push(')');
            }
            Op::ReadAcross { a, b, out: o } => {
                out.push('(');
                out.push_str(&self.value_id_str(*o));
                out.push_str(&format!(" = read-across {} {})", self.net_id_str(*a), self.net_id_str(*b)));
            }
            Op::ReadThrough { a, b, out: o } => {
                out.push('(');
                out.push_str(&self.value_id_str(*o));
                out.push_str(&format!(" = read-through {} {})", self.net_id_str(*a), self.net_id_str(*b)));
            }
            Op::ContributeThrough { a, b, value } => {
                out.push_str(&format!(
                    "(contribute-through {} {} {})",
                    self.net_id_str(*a),
                    self.net_id_str(*b),
                    self.value_id_str(*value)
                ));
            }
            Op::ForceAcross { a, b, value } => {
                out.push_str(&format!(
                    "(force-across {} {} {})",
                    self.net_id_str(*a),
                    self.net_id_str(*b),
                    self.value_id_str(*value)
                ));
            }
            Op::ReadSignal { net, out: o } => {
                out.push('(');
                out.push_str(&self.value_id_str(*o));
                out.push_str(&format!(" = read-signal {})", self.net_id_str(*net)));
            }
            Op::Drive { net, value } => {
                out.push_str(&format!(
                    "(drive {} {})",
                    self.net_id_str(*net),
                    self.value_id_str(*value)
                ));
            }
            Op::CellRead { cell, addr, out: o } => {
                out.push('(');
                out.push_str(&self.value_id_str(*o));
                out.push_str(&format!(" = cell-read {}", self.cell_id_str(*cell)));
                if let Some(a) = addr {
                    out.push_str(&format!(" (addr {})", self.value_id_str(*a)));
                }
                out.push(')');
            }
            Op::CellWrite { cell, addr, value } => {
                out.push_str(&format!(
                    "(cell-write {} {}",
                    self.cell_id_str(*cell),
                    self.value_id_str(*value)
                ));
                if let Some(a) = addr {
                    out.push_str(&format!(" (addr {})", self.value_id_str(*a)));
                }
                out.push(')');
            }
            Op::If { cond, then_ops, else_ops } => {
                out.push_str(&format!("(if {}", self.value_id_str(*cond)));
                out.push_str(" (then");
                for inner in then_ops {
                    out.push(' ');
                    let mut buf = String::new();
                    self.write_op(&mut buf, inner);
                    out.push_str(&buf);
                }
                out.push_str(") (else");
                for inner in else_ops {
                    out.push(' ');
                    let mut buf = String::new();
                    self.write_op(&mut buf, inner);
                    out.push_str(&buf);
                }
                out.push_str("))");
            }
            Op::Match { scrutinee, arms } => {
                out.push_str(&format!("(match {}", self.value_id_str(*scrutinee)));
                for (pat, body) in arms {
                    out.push(' ');
                    out.push_str(Self::fmt_pattern(*pat));
                    out.push(' ');
                    out.push('(');
                    let mut first = true;
                    for inner in body {
                        if !first {
                            out.push(' ');
                        }
                        first = false;
                        let mut buf = String::new();
                        self.write_op(&mut buf, inner);
                        out.push_str(&buf);
                    }
                    out.push(')');
                }
                out.push(')');
            }
        }
    }

    fn fmt_pureop(op: PureOp) -> &'static str {
        match op {
            PureOp::Add => "add",
            PureOp::Sub => "sub",
            PureOp::Mul => "mul",
            PureOp::Div => "div",
            PureOp::Neg => "neg",
            PureOp::Eq => "eq",
            PureOp::Ne => "ne",
            PureOp::Lt => "lt",
            PureOp::Le => "le",
            PureOp::Gt => "gt",
            PureOp::Ge => "ge",
            PureOp::Select => "select",
        }
    }

    fn fmt_pattern(p: Pattern) -> &'static str {
        match p {
            Pattern::Zero => "0",
            Pattern::One => "1",
            Pattern::Wildcard => "?",
        }
    }

    fn fmt_ty(out: &mut String, ty: &Ty) {
        match ty {
            Ty::Real => out.push_str("real"),
            Ty::Bits(n) => out.push_str(&format!("(bits {})", n)),
            Ty::Array(elem, dim) => {
                out.push_str("(array ");
                Self::fmt_ty(out, elem);
                out.push(' ');
                out.push_str(&dim.to_string());
                out.push(')');
            }
        }
    }

    // ── id renderers: `$N` for generated, `\name` for authored ─────────

    fn value_id_str(&self, id: ValueId) -> String {
        match self.names.value(id) {
            Some(name) => format!("\\{}", name),
            None => format!("${}", id.0),
        }
    }
    fn net_id_str(&self, id: NetId) -> String {
        match self.names.net(id) {
            Some(name) => format!("\\{}", name),
            None => format!("${}", id.0),
        }
    }
    fn cell_id_str(&self, id: CellId) -> String {
        match self.names.cell(id) {
            Some(name) => format!("\\{}", name),
            None => format!("${}", id.0),
        }
    }
    fn block_id_str(&self, id: BlockId) -> String {
        match self.names.block(id) {
            Some(name) => format!("\\{}", name),
            None => format!("${}", id.0),
        }
    }

    // ── small format helpers ──────────────────────────────────────────

    fn write_line(&mut self, s: &str) {
        self.out.push_str(&"  ".repeat(self.indent));
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn write_line_fmt(&mut self, args: std::fmt::Arguments<'_>) {
        self.out.push_str(&"  ".repeat(self.indent));
        use std::fmt::Write;
        let _ = self.out.write_fmt(args);
        self.out.push('\n');
    }

    fn write_section(&mut self, name: &str, body: impl FnOnce(&mut Self)) {
        self.write_line_fmt(format_args!("({}", name));
        self.indent += 1;
        body(self);
        self.indent -= 1;
        self.write_line(")");
    }

    fn indented(&mut self, body: impl FnOnce(&mut Self)) {
        self.indent += 1;
        body(self);
        self.indent -= 1;
    }
}

/// Escape a string for embedding in `"..."` (currently only `"` and `\`).
fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

impl Module {
    /// Print this module as the stable S-expression text. Every entity
    /// prints as `$N` (no authored names). Use [`Module::print_with_names`]
    /// to author names for provenance.
    pub fn print(&self) -> String {
        Printer::new(self, &Names::new()).print()
    }

    /// Print with the supplied authored names — entities named in `names`
    /// appear as `\name`, all others as `$N`.
    pub fn print_with_names(&self, names: &Names) -> String {
        Printer::new(self, names).print()
    }
}

#[cfg(test)]
mod tests {
    //! Inline unit tests for the printer's determinism and provenance.
    //! The non-trivial repeat-print-equality integration test lives in
    //! `crates/piperine-ir/tests/print.rs` (MD-28: integration grouped by
    //! concern).
    use super::*;
    use crate::core::{
        Block, Cell, Chunk, Const, Domain, EdgeKind, Net, NatureInfo, NatureKind, Op,
        Pattern, SigRef, Trigger, Ty,
    };

    /// LOV-06: printing the same simple module twice is byte-identical.
    /// This is the inline sanity; the full non-trivial proof is the
    /// integration test.
    #[test]
    fn repeat_print_is_byte_identical_simple() {
        let mut m = Module::new();
        let v_n = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
        let i_n = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
        let n = m.push_net(Net::node(v_n, i_n));
        let g = m.push_net(Net::node(v_n, i_n));
        let init = m.push_const(Const::Real(0.0));
        let writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
        let _ = m.push_cell(Cell::new(Ty::Real, init, writer));
        let out = m.push_value(Ty::Real);
        m.block_at_mut(writer).push(Op::ReadAcross { a: n, b: g, out });
        let a = m.print();
        let b = m.print();
        assert_eq!(a.as_bytes(), b.as_bytes(), "byte-identical across two print() calls");
        // The same equality at the bytes level — explicit, the load-bearing
        // property.
        assert_eq!(a, b);
    }

    /// `\user` vs `$generated`: a module that mingles authored and
    /// generated names prints BOTH forms.
    #[test]
    fn authored_and_generated_names_both_appear() {
        let mut m = Module::new();
        let v = m.push_value(Ty::Real); // generated $0
        let init = m.push_const(Const::Real(0.0));
        let writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
        let c = m.push_cell(Cell::new(Ty::Real, init, writer)); // generated $0
        m.block_at_mut(writer).push(Op::CellWrite { cell: c, addr: None, value: v });

        let mut names = Names::new();
        names.set_value(v, "vin");      // \vin for the value
        // The cell stays generated — `$0`.
        let text = m.print_with_names(&names);

        assert!(text.contains("\\vin"), "authored name appears: {text}");
        assert!(text.contains("$0"), "generated name appears: {text}");
    }

    /// Determinism across two distinct printer instances (LOV-06): the
    /// output depends only on the module + names, never on transient state.
    #[test]
    fn two_printer_instances_produce_identical_output() {
        let mut m = Module::new();
        let _ = m.push_value(Ty::Bits(8));
        let _ = m.push_value(Ty::Real);
        let a = Printer::new(&m, &Names::new()).print();
        let b = Printer::new(&m, &Names::new()).print();
        assert_eq!(a, b);
    }

    /// Section order is fixed: natures → nets → cells → values → consts →
    /// blocks. A reader can grep for the section headers in this order.
    #[test]
    fn section_order_is_natures_nets_cells_values_consts_blocks() {
        let mut m = Module::new();
        let init = m.push_const(Const::Real(0.0)); // const
        let writer = m.push_block(Block::new(Domain::Digital, Trigger::Initial)); // block
        let _ = m.push_cell(Cell::new(Ty::Real, init, writer)); // cell
        let _ = m.push_value(Ty::Real); // value
        let v_n = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
        let i_n = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
        let _ = m.push_net(Net::node(v_n, i_n)); // net
        // (pushed in non-output order on purpose)
        let text = m.print();
        let n_pos = text.find("(natures").unwrap();
        let ne_pos = text.find("(nets").unwrap();
        let c_pos = text.find("(cells").unwrap();
        let va_pos = text.find("(values").unwrap();
        let co_pos = text.find("(consts").unwrap();
        let b_pos = text.find("(blocks").unwrap();
        assert!(n_pos < ne_pos, "natures before nets");
        assert!(ne_pos < c_pos, "nets before cells");
        assert!(c_pos < va_pos, "cells before values");
        assert!(va_pos < co_pos, "values before consts");
        assert!(co_pos < b_pos, "consts before blocks");
    }

    /// Real-literal formatting is byte-stable: the same f64 prints the same
    /// bytes every time, full `{:.16e}` precision.
    #[test]
    fn real_const_formatting_is_byte_stable() {
        let mut m = Module::new();
        let _ = m.push_const(Const::Real(1.0_f64 / 3.0_f64));
        let a = m.print();
        let b = m.print();
        assert_eq!(a, b);
        assert!(a.contains("3.3333333333333331e-1"), "16-digit exponent form: {a}");
    }

    /// A bits-typed signal net prints its type inline (the signal carries a
    /// `Ty`, the node carries natures).
    #[test]
    fn signal_net_carries_type_and_node_net_carries_natures() {
        let mut m = Module::new();
        let v_n = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
        let i_n = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
        let _node = m.push_net(Net::node(v_n, i_n));
        let _sig = m.push_net(Net::signal(Ty::Bits(8)));
        let text = m.print();
        assert!(text.contains("(node across=N0 through=N1)"), "node prints its nature ids: {text}");
        assert!(text.contains("(signal (bits 8))"), "signal prints its type: {text}");
    }

    /// Every trigger kind prints a distinct, parseable form. The integration
    /// test exercises all five in one module; this test names them
    /// individually so a regression points at the broken kind.
    #[test]
    fn every_trigger_kind_prints_a_distinct_form() {
        fn first_trigger_line(text: &str) -> &str {
            text.lines().find(|l| l.contains("(block ")).unwrap()
        }
        // Always
        let mut m = Module::new();
        let _ = m.push_block(Block::new(Domain::Analog, Trigger::Always));
        assert!(first_trigger_line(&m.print()).contains("always"));
        // Initial
        let mut m = Module::new();
        let _ = m.push_block(Block::new(Domain::Digital, Trigger::Initial));
        assert!(first_trigger_line(&m.print()).contains("initial"));
        // Edge
        let mut m = Module::new();
        let v = m.push_value(Ty::Bits(1));
        let sr = SigRef(vec![Chunk::Slice { value: v, lo: 0, hi: 1 }]);
        let _ = m.push_block(Block::new(Domain::Digital, Trigger::Edge(sr, EdgeKind::Rising)));
        assert!(first_trigger_line(&m.print()).contains("(edge "));
        assert!(first_trigger_line(&m.print()).contains("rising"));
        // Cross
        let mut m = Module::new();
        let v = m.push_value(Ty::Real);
        let _ = m.push_block(Block::new(Domain::Analog, Trigger::Cross(v, CrossDir::Either)));
        assert!(first_trigger_line(&m.print()).contains("(cross "));
        // Timer
        let mut m = Module::new();
        let p = m.push_value(Ty::Real);
        let ph = m.push_value(Ty::Real);
        let _ = m.push_block(Block::new(Domain::Analog, Trigger::Timer { period: p, phase: ph }));
        assert!(first_trigger_line(&m.print()).contains("(timer "));
    }

    /// An `If` op prints both arms as `(then ...)` and `(else ...)`. A
    /// `Match` op prints its arms with patterns `0`/`1`/`?`.
    #[test]
    fn if_and_match_print_arms() {
        let mut m = Module::new();
        let cond = m.push_value(Ty::Bits(1));
        let out = m.push_value(Ty::Bits(1));
        let net = {
            let v_n = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
            let i_n = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
            m.push_net(Net::node(v_n, i_n))
        };
        let writer = m.push_block(Block::new(Domain::Digital, Trigger::Always));
        m.block_at_mut(writer).push(Op::If {
            cond,
            then_ops: vec![Op::Drive { net, value: out }],
            else_ops: vec![Op::Drive { net, value: cond }],
        });
        m.block_at_mut(writer).push(Op::Match {
            scrutinee: cond,
            arms: vec![
                (Pattern::Zero, vec![Op::Drive { net, value: out }]),
                (Pattern::One, vec![]),
                (Pattern::Wildcard, vec![Op::Drive { net, value: cond }]),
            ],
        });
        let text = m.print();
        assert!(text.contains("(if "), "If prints: {text}");
        assert!(text.contains("(then"), "then arm prints: {text}");
        assert!(text.contains("(else"), "else arm prints: {text}");
        assert!(text.contains("(match "), "Match prints: {text}");
        assert!(text.contains("0 ("), "Zero pattern prints: {text}");
        assert!(text.contains("1 ("), "One pattern prints: {text}");
        assert!(text.contains("? ("), "Wildcard pattern prints: {text}");
    }

    /// Authored names may contain hyphens and other identifier-ish chars.
    /// The grammar tolerates anything except whitespace and parens.
    #[test]
    fn authored_names_with_hyphens_print_verbatim() {
        let mut m = Module::new();
        let v = m.push_value(Ty::Real);
        let mut names = Names::new();
        names.set_value(v, "my-signal");
        let text = m.print_with_names(&names);
        assert!(text.contains("\\my-signal"), "hyphenated authored name: {text}");
    }

    /// An empty module still prints a valid (if hollow) form.
    #[test]
    fn empty_module_prints_hollow_form() {
        let m = Module::new();
        let text = m.print();
        assert!(text.starts_with("(module"), "starts with module: {text}");
        // All six sections appear, even when empty.
        for sec in ["(natures", "(nets", "(cells", "(values", "(consts", "(blocks"] {
            assert!(text.contains(sec), "section {sec} appears even when empty: {text}");
        }
    }
}
