//! Scope: the elaborator's side of `unknown-semantics` (part I §3.3, §22.1): `ConstEval` folds a
//! value that may be unknown by the level rules of §3.3.2, §3.3.3 and §3.3.12, `T::WIDTH` is the
//! level count of a `Repr` type.
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
