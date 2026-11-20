//! Scope: the elaborator's side of `unknown-semantics` (part I §3.3, §22.1): `ConstEval` folds a
//! value that may be unknown by the level rules of §3.3.2, §3.3.3 and §3.3.12, `T::WIDTH` is the
//! level count of a `Repr` type, and `Unknown` where it decides structure is `E0151` (§3.3.8).
//! Each expected value is written from the rule it cites, one source expression per row.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::Constant;
use piperine_lang::elab::{ConstEval, ConstFault};
use piperine_lang::pom::{Design, Reflect};
use piperine_lang::types::Types;
use support::Elab;

/// What a row of the table expects: a known value, or `Unknown`.
#[derive(Debug, Clone, Copy)]
enum Expect {
    Known(Constant),
    Unknown,
}

fn int(v: i128) -> Expect {
    Expect::Known(Constant::Int(v))
}

fn boolean(v: bool) -> Expect {
    Expect::Known(Constant::Bool(v))
}

/// A design of `Top` whose carriers read the rows as `const`s `R0`, `R1`, .. (an integer is no
/// param type, and reading a const from a carrier's initial value is what puts it in the checked
/// set), over the `const`s every row may read: `U` an unknown `u8`, `UB` an unknown `bool`, `UI`
/// an unknown `i8`.
struct Table {
    design: Design,
}

impl Table {
    const ITEMS: &'static str = "const U: u8 = Unknown;\nconst UB: bool = Unknown;\nconst UI: i8 = Unknown;\n#[derive(Repr)] enum Mode { A, B, C }\n";

    /// `rows` are `(type, expression)`: each becomes a `const`.
    fn new(rows: &[(&str, &str)]) -> Table {
        let consts: Vec<String> = rows.iter().enumerate().map(|(i, (ty, expr))| format!("const R{i}: {ty} = {expr};")).collect();
        let signals: Vec<String> = rows.iter().enumerate().map(|(i, (ty, _))| format!("signal s{i}: Bit<{ty}> = R{i};")).collect();
        let source = format!("{}{}\nmodule Top() {{ {} }}\n", Self::ITEMS, consts.join("\n"), signals.join(" "));
        let design = Elab::design(&source);
        assert!(design.diagnostics().is_empty(), "{:?}\n{source}", Elab::codes(design.diagnostics()));
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
            match (table.fold(&types, i), expect) {
                (Ok(got), Expect::Known(want)) => assert_eq!(got, *want, "`{expr}`"),
                (Err(ConstFault::Unknown { .. }), Expect::Unknown) => {}
                (got, want) => panic!("`{expr}` folded to {got:?}, wanted {want:?}"),
            }
        }
    }
}

#[test]
fn unknown_folds_by_the_level_rules() {
    Table::check(&[
        // §3.3.2: arithmetic, comparison and `^` are all-or-nothing; an unknown operand is not
        // evaluated, so an unknown dividend over zero is no panic
        ("u8", "U + 1u8", Expect::Unknown),
        ("u8", "U / 0u8", Expect::Unknown),
        ("bool", "U == 3u8", Expect::Unknown),
        ("bool", "UB == UB", Expect::Unknown),
        ("u8", "U ^ 0u8", Expect::Unknown),
        ("u8", "-U", Expect::Unknown),
        ("bool", "!UB", Expect::Unknown),
        ("u8", "(U as u8)", Expect::Unknown),
        // §3.3.12: a known all-false operand decides `&` and `&&`, a known all-true operand `|`
        // and `||`, whatever the other; anything else is unknown. `&&` short-circuits on a known
        // false left operand
        ("bool", "UB && false", boolean(false)),
        ("bool", "UB || true", boolean(true)),
        ("bool", "UB && true", Expect::Unknown),
        ("bool", "UB || false", Expect::Unknown),
        ("bool", "true && UB", Expect::Unknown),
        ("bool", "false && UB", boolean(false)),
        ("bool", "true || UB", boolean(true)),
        ("bool", "UB & false", boolean(false)),
        ("bool", "false | UB", Expect::Unknown),
        ("bool", "true | UB", boolean(true)),
        ("u8", "U & 0u8", int(0)),
        ("u8", "U & 3u8", Expect::Unknown),
        ("u8", "U | 255u8", int(255)),
        ("u8", "U | 1u8", Expect::Unknown),
        ("i8", "UI | -1i8", int(-1)),
        // §3.3.11: an unknown value shifted by `N` or more is known 0, a signed `>>` fills with
        // the sign and stays unknown; an unknown amount is unknown
        ("u8", "U << 8u32", int(0)),
        ("u8", "U >> 8u32", int(0)),
        ("u8", "U << 3u32", Expect::Unknown),
        ("i8", "UI << 8u32", int(0)),
        ("i8", "UI >> 8u32", Expect::Unknown),
        ("u8", "1u8 << U", Expect::Unknown),
        // §3.3.4, §22.1.1: the flag reads are known whatever the value holds; `known_or` gives
        // its default, which may itself be unknown (rule 4)
        ("u8", "U.known_or(7u8)", int(7)),
        ("u8", "5u8.known_or(7u8)", int(5)),
        ("u8", "U.known_or(U)", Expect::Unknown),
        ("bool", "U.is_known()", boolean(false)),
        ("bool", "5u8.is_known()", boolean(true)),
        ("bool", "UB.high()", boolean(false)),
        ("bool", "true.high()", boolean(true)),
        // §3.3.3: an `if` on an unknown condition is the merge of its arms: equal known values are
        // kept, anything else is unknown; §1 rule 13: an arm that panics is no error and makes
        // the merge unknown
        ("u8", "if UB { 1u8 } else { 1u8 }", int(1)),
        ("u8", "if UB { 1u8 } else { 2u8 }", Expect::Unknown),
        ("u8", "if UB { 1u8 / 0u8 } else { 5u8 }", Expect::Unknown),
        ("u8", "if UB { 5u8 } else { U }", Expect::Unknown),
        ("u8", "if true { 5u8 } else { U }", int(5)),
        // a method of the catalogue over an unknown value is unknown, and a known one still folds
        ("f64", "(U as f64).sqrt()", Expect::Unknown),
        ("u8", "U.max(3u8)", Expect::Unknown),
        ("u8", "4u8.max(3u8)", int(4)),
    ]);
}

#[test]
fn width_is_the_level_count_of_each_repr_type() {
    // §2: 1 for `bool`, `N` for `uN` and `iN`, 64 for `f64`, the tag width `ceil(log2 V)` of an
    // enum of `V` unit variants (rule 5); a const `u32`
    Table::check(&[
        ("u32", "bool::WIDTH", int(1)),
        ("u32", "u12::WIDTH", int(12)),
        ("u32", "i7::WIDTH", int(7)),
        ("u32", "u64::WIDTH", int(64)),
        ("u32", "f64::WIDTH", int(64)),
        ("u32", "Mode::WIDTH", int(2)),
        ("u32", "u8::WIDTH + 1u32", int(9)),
    ]);
}

#[test]
fn a_real_that_is_nan_merges_with_itself() {
    // §3.3.9, §22.1.2 rule 2: a NaN is the same as itself, so two arms that both yield NaN merge
    // to NaN, known
    let table = Table::new(&[("f64", "if UB { 0.0 / 0.0 } else { 0.0 / 0.0 }")]);
    let types = Types::check(&table.design, "Top").expect("the source is in the subset");
    match table.fold(&types, 0) {
        Ok(Constant::Real(r)) => assert!(r.get().is_nan(), "{r:?}"),
        other => panic!("{other:?}"),
    }
}

/// The diagnostics of elaborating `Top` of `src`, which must be refused.
fn refused(src: &str) -> Vec<piperine_lang::pom::DesignDiagnostic> {
    Elab::run(src, "Top").expect_err("elaboration refuses a structure decided by `Unknown`")
}

#[test]
fn unknown_deciding_structure_is_e0151() {
    // §3.3.8: a `where` condition and a param default fix structure, so a value that may be
    // unknown cannot decide them: `E0151` naming the decision, at the expression
    let src = "module Leaf() { r: f64 where Unknown; }\nmodule Top() { let r1 = Leaf { r: 1.0 }(); }";
    let err = refused(src);
    assert_eq!(Elab::codes(&err), ["E0151"]);
    assert!(err[0].diagnostic.message.contains("a `where` condition"), "names the decision: {}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "Unknown");

    let src = "module Top() { r: f64 = Unknown; }";
    let err = refused(src);
    assert_eq!(Elab::codes(&err), ["E0151"]);
    assert!(err[0].diagnostic.message.contains("a param default"), "names the decision: {}", err[0].diagnostic.message);
}

/// One `analog` statement of `Amp` over `const U`, an unknown real, and `Top` placing it.
fn analog_source(statement: &str) -> String {
    format!("const U: f64 = Unknown;\nmodule Amp(p: Electrical) {{ }}\nanalog for Amp {{ {statement} }}\nmodule Top() {{ signal s: Electrical; let a = Amp(s); }}")
}

/// The statements of `analog` that read the unknown `U` through each form the elaborator takes a
/// known real in, with the text the diagnostic points at.
const ANALOG_FORMS: [(&str, &str); 7] = [
    ("v(p) +> U;", "U"),
    ("v(p) +> U.sqrt();", "U.sqrt()"),
    ("v(p) +> 1.0.known_or(U);", "1.0.known_or(U)"),
    ("i(p) +> ddt(U);", "U"),
    ("v(p) +> ac_stim(U);", "U"),
    ("v(p) +> limit(U, |n, o| n);", "U"),
    ("v(p) +> Unknown;", "Unknown"),
];

#[test]
fn analog_reading_a_maybe_unknown_value_is_e0150() {
    // §3.3.5: `analog` computes with known reals, so a value that may be unknown in a
    // contribution, a `known_or` default, a math function or the argument of `ddt` or `ac_stim`
    // is `E0150` naming its source `U`, at the leaf
    for (statement, at) in ANALOG_FORMS {
        let src = analog_source(statement);
        let err = refused(&src);
        assert_eq!(Elab::codes(&err), ["E0150"], "{statement}");
        let span = err[0].diagnostic.span();
        assert_eq!(&src[span.start..span.end], at, "{statement}");
        let source = if at == "Unknown" { "`Unknown`" } else { "`U`" };
        assert!(err[0].diagnostic.message.contains(source), "names the source: {statement}: {}", err[0].diagnostic.message);
    }
}

#[test]
fn no_analog_refusal_of_an_unknown_is_p0001() {
    // US-06 AC 3: the backstops of the elaborator in `analog` are never the placeholder `P0001`;
    // a known value through the same forms elaborates
    for (statement, _) in ANALOG_FORMS {
        let err = refused(&analog_source(statement));
        assert!(Elab::codes(&err).iter().all(|c| c != "P0001"), "{statement}: {:?}", Elab::codes(&err));
    }
    for statement in ["v(p) +> 1.0;", "v(p) +> 2.0.sqrt();", "v(p) +> 1.0.known_or(2.0);", "i(p) +> ddt(1.0);", "v(p) +> ac_stim(1.0);", "v(p) +> limit(1.0, |n, o| n);"] {
        let src = analog_source(statement).replace("const U: f64 = Unknown;\n", "");
        Elab::ir(&src, "Top");
    }
}
