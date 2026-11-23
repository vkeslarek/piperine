//! The checker's side of the operator traits (Appendix H.10, EU-03, EU-08, EU-09): `&`, `|`, `^`, `!`, `<<`, `>>`,
//! `==` and `!=` on a struct are the function of the one `impl` of their trait for the two operand types and take
//! its `Out`; no `impl` is `E0180`; two `impl`s for one type and one `Rhs` are `E0187`; arithmetic, ordering and
//! control refuse a four-state value (`E0180`). The sources use `Quad`, a four-state struct written in the test,
//! until the library's own `Logic` is built from `std/logic.msl`.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/checked.rs"]
mod checked_source;

use checked_source::CheckedSource;
use piperine_lang::types::{Ty, Types};

/// A four-state value of four levels as two planes, with the operators of H.10 written as the library writes
/// them: `&` of two `Quad`, `&` of a `Quad` and a `u4`, `!`, `<<`, and `==` that gives a `Quad`.
const QUAD: &str = "
#[derive(Repr)] struct Quad { binary: u4, value: u4 }
impl BitAnd for Quad { fn bitand(self, r: Quad) -> Quad { Quad { binary: self.binary & r.binary, value: self.value & r.value } } }
impl BitAnd<u4, Quad> for Quad { fn bitand(self, r: u4) -> Quad { Quad { binary: self.binary, value: self.value & r } } }
impl BitOr for Quad { fn bitor(self, r: Quad) -> Quad { Quad { binary: self.binary | r.binary, value: self.value | r.value } } }
impl BitXor for Quad { fn bitxor(self, r: Quad) -> Quad { Quad { binary: self.binary & r.binary, value: self.value ^ r.value } } }
impl Not for Quad { fn not(self) -> Quad { Quad { binary: self.binary, value: !self.value } } }
impl Shl for Quad { fn shl(self, k: usize) -> Quad { Quad { binary: self.binary << k, value: self.value << k } } }
impl PartialEq<Quad, Quad> for Quad {
    fn eq(self, r: Quad) -> Quad { Quad { binary: self.binary & r.binary, value: !(self.value ^ r.value) } }
    fn ne(self, r: Quad) -> Quad { !self.eq(r) }
}
fn quad() -> Quad { Quad { binary: 15, value: 5 } }
";

/// The checker's answer to `items` and the analog block `body` of `Top`, as `(code, message)`.
fn outcome(items: &str, body: &str) -> Vec<(String, String)> {
    CheckedSource::outcome(&CheckedSource::source(&format!("{QUAD}\n{items}"), body))
}

/// The diagnostics of the whole-model check of the sources after `QUAD` and `items`, as `(code, message)`.
fn model_outcome(items: &str) -> Vec<(String, String)> {
    let design = CheckedSource::design_of(&format!("{QUAD}\n{items}"));
    Types::check_model(&design).err().unwrap_or_default().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
}

/// The type of the `let` binding `name` of `body`, which must check clean.
fn local_type(body: &str, name: &str) -> Ty {
    CheckedSource::local_type(&CheckedSource::source(QUAD, body), name)
}

/// `found` holds one diagnostic of `code` whose message holds every word of `words`.
fn assert_one(found: &[(String, String)], what: &str, code: &str, words: &[&str]) {
    assert_eq!(found.len(), 1, "one diagnostic for `{what}`, got {found:?}");
    assert_eq!(found[0].0, code, "`{what}`: {found:?}");
    for word in words {
        assert!(found[0].1.contains(word), "`{what}`: `{}` does not name `{word}`", found[0].1);
    }
}

/// The source of `body` is refused with one diagnostic of `code` whose message holds every word of `words`.
fn assert_refused(items: &str, body: &str, code: &str, words: &[&str]) {
    assert_one(&outcome(items, body), body, code, words);
}

#[test]
fn an_operator_on_a_struct_is_its_impl_function_and_takes_its_out() {
    // H.10 rule 1: `&`, `|`, `^`, `!`, `<<` give the `Quad` the `impl` returns; `==` and `!=` give the `Out` the `impl`
    // names, a `Quad` here, not a `bool`
    let body = "let q = quad(); let a = q & q; let b = q | q; let c = q ^ q; let d = !q; let e = q << 1; let f = q == q; let g = q != q;";
    let quad = |name: &str| local_type(body, name);
    let wanted = quad("q");
    assert!(matches!(wanted, Ty::Struct(_)), "`quad()` is a struct: {wanted:?}");
    for name in ["a", "b", "c", "d", "e", "f", "g"] {
        assert_eq!(quad(name), wanted, "`{name}` is the `Out` of its `impl`, a `Quad`");
    }
}

#[test]
fn the_right_operand_picks_the_impl() {
    // H.10 rule 1: two `impl BitAnd` for `Quad` differ by `Rhs`; `q & q` and `q & 3` each take the one whose `Rhs` they have
    CheckedSource::assert_clean(&CheckedSource::source(QUAD, "let q = quad(); let a = q & q; let b = q & 3_u4;"));
}

#[test]
fn an_operator_without_an_impl_is_e0180() {
    // H.10 rule 1: `Quad` has no `Shr`, no `BitAnd<bool>`: the operator is `E0180` naming the type and the trait
    assert_refused("", "let q = quad(); let a = q >> 1;", "E0180", &["`Quad`", "Shr"]);
    assert_refused("", "let q = quad(); let a = q & true;", "E0180", &["BitAnd", "`Quad`", "`bool`"]);
    assert_refused("struct Plain { a: u4 }", "let p = Plain { a: 1_u4 }; let b = p & p;", "E0180", &["`Plain`", "BitAnd"]);
}

#[test]
fn two_impls_for_one_type_and_one_rhs_are_e0187() {
    // H.10 rule 2: a second `impl BitAnd for Quad` with the same `Rhs` is `E0187`
    assert_one(&model_outcome("impl BitAnd for Quad { fn bitand(self, r: Quad) -> Quad { r } }"), "a second `impl BitAnd for Quad`", "E0187", &["BitAnd", "Quad"]);
}

#[test]
fn control_refuses_a_logic_and_an_option() {
    // EU-03 AC 1 (§3.3.5): a four-state value or an `Option` as a condition, a guard or a short-circuit operand is
    // `E0180` naming the type and `bool`
    assert_refused("", "let q = quad(); if q { }", "E0180", &["`Quad`", "`bool`"]);
    assert_refused("", "let q = quad(); while q { break; }", "E0180", &["`Quad`", "`bool`"]);
    assert_refused("", "let q = quad(); let a = q && true;", "E0180", &["`Quad`", "`bool`"]);
    assert_refused("", "let q = quad(); let a = true || q;", "E0180", &["`Quad`", "`bool`"]);
    assert_refused("", "let o: Option<u4> = None; if o { }", "E0180", &["`Option<u4>`", "`bool`"]);
    assert_refused("", "let o: Option<u4> = None; let a = o && true;", "E0180", &["`Option<u4>`", "`bool`"]);
}

#[test]
fn analog_refuses_a_bare_logic() {
    // EU-03 AC 3 (§3.3.6): a four-state value where `analog` needs an `f64` or a `bool` is `E0180`
    assert_refused("", "let q = quad(); let a: f64 = q;", "E0180", &["`Quad`", "`f64`"]);
    assert_refused("", "let q = quad(); let a: bool = q;", "E0180", &["`Quad`", "`bool`"]);
    assert_refused("fn volts(x: f64) -> f64 { x }", "let q = quad(); let a = volts(q);", "E0180", &["`Quad`", "`f64`"]);
}

#[test]
fn arithmetic_and_ordering_refuse_a_logic() {
    // EU-08 AC 7 (H.10 rule 5): `+`, `-`, `*`, `/` and `<`, `<=`, `>`, `>=` have no trait; a four-state value is
    // `E0180` under each
    for operator in ["+", "-", "*", "/", "<", "<=", ">", ">="] {
        let found = outcome("", &format!("let q = quad(); let a = q {operator} q;"));
        assert!(!found.is_empty(), "`q {operator} q` is refused");
        for (code, message) in &found {
            assert_eq!(code, "E0180", "`q {operator} q`: {found:?}");
            let needs = if operator.starts_with('<') || operator.starts_with('>') { "PartialOrd" } else { "`f64`" };
            assert!(message.contains("Quad") && message.contains(needs), "`q {operator} q`: `{message}` names the type and what the operator needs ({needs})");
        }
    }
}

#[test]
fn deriving_partial_eq_over_a_logic_is_e0187() {
    // EU-09 AC 4 (§22.1.2 rule 5): the derived `==` returns a `bool` and a field whose `==` returns a `Quad` gives none:
    // `E0187` naming the field and its type
    let items = "#[derive(PartialEq)] struct Pair { first: Quad, second: u4 }";
    assert_refused(items, "let p = Pair { first: quad(), second: 1_u4 };", "E0187", &["first", "`Quad`", "PartialEq"]);
}
