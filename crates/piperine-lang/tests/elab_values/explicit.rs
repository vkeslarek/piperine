//! Scope: the elaborator's side of the explicit model of unknowns (part I §3.3, §22.1): `ConstEval`
//! folds a value by the rules of §3.3.2 and §3.3.12 where every value is known, `T::WIDTH` is the
//! level count of a `Repr` type, `Unknown` is no value (`E0161`), and a known real goes through
//! each form `analog` takes one in. Each expected value is written from the rule it cites, one
//! source expression per row.


use piperine_ir::Constant;
use piperine_lang::elab::{ConstEval, ConstFault};
use piperine_lang::pom::{Design, Reflect};
use piperine_lang::types::Types;
use piperine_lang_fixtures::elaboration::SourceElaboration;

/// What a row of the table expects: a known value.
#[derive(Debug, Clone, Copy)]
struct Expect(Constant);

/// Associated test helpers of `explicit`, grouped on one fixture type.
struct ExplicitFixture;

impl ExplicitFixture {
    fn int(v: i128) -> Expect {
        Expect(Constant::Int(v))
    }

    fn boolean(v: bool) -> Expect {
        Expect(Constant::Bool(v))
    }

    /// One `analog` statement of `Amp` over a known real, and `Top` placing it.
    fn analog_source(statement: &str) -> String {
        format!("module Amp(p: Electrical) {{ }}\nanalog for Amp {{ {statement} }}\nmodule Top() {{ signal s: Electrical; let a = Amp(s); }}")
    }
}

/// A design of `Top` whose carriers read the rows as `const`s `R0`, `R1`, .. (an integer is no
/// param type, and reading a const from a carrier's initial value is what puts it in the checked
/// set), over the enum every row may read.
struct Table {
    design: Design,
}

impl Table {
    const ITEMS: &'static str = "#[derive(Repr)] enum Mode { A, B, C }\n";

    /// `rows` are `(type, expression)`: each becomes a `const`.
    fn new(rows: &[(&str, &str)]) -> Table {
        let consts: Vec<String> = rows.iter().enumerate().map(|(i, (ty, expr))| format!("const R{i}: {ty} = {expr};")).collect();
        let signals: Vec<String> = rows.iter().enumerate().map(|(i, (ty, _))| format!("signal s{i}: Bit<{ty}> = R{i};")).collect();
        let source = format!("{}{}\nmodule Top() {{ {} }}\n", Self::ITEMS, consts.join("\n"), signals.join(" "));
        let design = SourceElaboration::design(&source);
        assert!(design.diagnostics().is_empty(), "{:?}\n{source}", SourceElaboration::codes(design.diagnostics()));
        Table { design }
    }

    /// What `const R{i}` folds to: `Ok` a known value, `Err` the fault.
    fn fold(&self, types: &Types<'_>, i: usize) -> Result<Constant, ConstFault> {
        let name = format!("R{i}");
        let decl = self.design.decls().into_iter().find(|d| d.name() == Some(name.as_str())).expect("a const of that name");
        ConstEval::new(&self.design, types).constant(decl.id())
    }

    /// Checks every row against its expectation.
    fn check(rows: &[(&str, &str, Expect)]) {
        let table = Table::new(&rows.iter().map(|(ty, expr, _)| (*ty, *expr)).collect::<Vec<_>>());
        let types = Types::check(&table.design, "Top").expect("the source is in the subset");
        for (i, (_, expr, expect)) in rows.iter().enumerate() {
            match table.fold(&types, i) {
                Ok(got) => assert_eq!(got, expect.0, "`{expr}`"),
                Err(fault) => panic!("`{expr}` folded to the fault {fault:?}, wanted {:?}", expect.0),
            }
        }
    }
}

#[test]
fn known_values_fold_by_the_level_rules() {
    Table::check(&[
        // §3.3.2: arithmetic and comparison over known operands
        ("u8", "1u8 + 2u8", ExplicitFixture::int(3)),
        ("bool", "3u8 == 3u8", ExplicitFixture::boolean(true)),
        ("u8", "5u8 & 3u8", ExplicitFixture::int(1)),
        ("u8", "5u8 | 2u8", ExplicitFixture::int(7)),
        ("u8", "1u8 << 3u32", ExplicitFixture::int(8)),
        // §3.3.12: `&&` and `||` over known operands
        ("bool", "true && false", ExplicitFixture::boolean(false)),
        ("bool", "false || true", ExplicitFixture::boolean(true)),
        // §3.3.3: an `if` on a known condition takes one arm
        ("u8", "if true { 5u8 } else { 6u8 }", ExplicitFixture::int(5)),
        // a method of the catalogue over a known value folds
        ("u8", "4u8.max(3u8)", ExplicitFixture::int(4)),
    ]);
}

#[test]
fn width_is_the_level_count_of_each_repr_type() {
    // §2: 1 for `bool`, `N` for `uN` and `iN`, 64 for `f64`, the tag width `ceil(log2 V)` of an
    // enum of `V` unit variants (rule 5); a const `u32`
    Table::check(&[
        ("u32", "bool::WIDTH", ExplicitFixture::int(1)),
        ("u32", "u12::WIDTH", ExplicitFixture::int(12)),
        ("u32", "i7::WIDTH", ExplicitFixture::int(7)),
        ("u32", "u64::WIDTH", ExplicitFixture::int(64)),
        ("u32", "f64::WIDTH", ExplicitFixture::int(64)),
        ("u32", "Mode::WIDTH", ExplicitFixture::int(2)),
        ("u32", "u8::WIDTH + 1u32", ExplicitFixture::int(9)),
    ]);
}

#[test]
fn unknown_decides_no_structure_because_it_is_no_value() {
    // EU-02 AC 1, EU-01: where `Unknown` stood in a `where` condition or a param default (the retired `E0151`),
    // it is a name that resolves to nothing: `E0161`, at the name, and nowhere `E0150` or `E0151`
    for (src, decision) in [
        ("module Leaf() { r: f64 where Unknown; }\nmodule Top() { let r1 = Leaf { r: 1.0 }(); }", "Unknown"),
        ("module Top() { r: f64 = Unknown; }", "Unknown"),
    ] {
        let design = SourceElaboration::design(src);
        assert_eq!(SourceElaboration::codes(design.diagnostics()), ["E0161"], "{src}");
        let span = design.diagnostics()[0].diagnostic.span();
        assert_eq!(&src[span.start..span.end], decision);
    }
}

#[test]
fn a_known_real_goes_through_each_analog_form() {
    // US-06 AC 3: a real through a contribution, a math function, `ddt`, `ac_stim` and `limit` elaborates
    for statement in ["v(p) +> 1.0;", "v(p) +> 2.0.sqrt();", "i(p) +> ddt(1.0);", "v(p) +> ac_stim(1.0);", "v(p) +> limit(1.0, |n, o| n);"] {
        SourceElaboration::ir(&ExplicitFixture::analog_source(statement), "Top");
    }
}

#[test]
fn analog_has_no_refusal_of_a_flag() {
    // EU-02 AC 4 (C7): no source in `analog` is `E0150` or `P0001` for being unknown
    let design = SourceElaboration::design(&ExplicitFixture::analog_source("v(p) +> Unknown;"));
    assert_eq!(SourceElaboration::codes(design.diagnostics()), ["E0161"]);
}
