//! The checker's side of the operator traits (Appendix H.10, EU-03, EU-08, EU-09): `&`, `|`, `^`, `!`, `<<`, `>>`,
//! `==` and `!=` on a struct are the function of the one `impl` of their trait for the two operand types and take
//! its `Out`; no `impl` is `E0180`; two `impl`s for one type and one `Rhs` are `E0187`; arithmetic, ordering and
//! control refuse a four-state value (`E0180`). The operator mechanics use `Quad`, a four-state struct written in the test; the
//! refusals of control, `analog`, arithmetic and `derive` are asked of `Quad` and of the library's own `Logic<4>`.

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/checked.rs"]
mod checked_source;

use checked_source::CheckedSource;
use piperine_lang::pom::{BUILTINS, DeclKind, Reflect};
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

/// A four-state value the checker's refusals are asked of: the test-local `Quad` (the operator mechanics, over an empty
/// standard library) and the library's own `Logic<4>` (over the embedded one), so a refusal holds for both.
struct Subject {
    /// The type as the author writes it, and as a diagnostic names it between backquotes.
    written: &'static str,
    /// The items a source with this value needs before `Top`.
    items: &'static str,
    /// An expression of the value, of four levels.
    value: &'static str,
    /// Whether the source is checked over the embedded standard library.
    library: bool,
}

impl Subject {
    /// The test-local four-state struct.
    const STAND_IN: Subject = Subject { written: "Quad", items: QUAD, value: "quad()", library: false };

    /// The four-state value of `std/logic.msl`.
    const LIBRARY: Subject = Subject { written: "Logic<4>", items: "", value: "Logic::from(5u4)", library: true };

    /// Both subjects.
    const BOTH: [Subject; 2] = [Subject::STAND_IN, Subject::LIBRARY];

    /// The source of `items` and the analog block `body` of `Top`, where `q` is the value.
    fn source(&self, items: &str, body: &str) -> String {
        CheckedSource::source(&format!("{}\n{items}", self.items), &format!("let q = {}; {body}", self.value))
    }

    /// The checker's answer to `items` and `body`, as `(code, message)`.
    fn outcome(&self, items: &str, body: &str) -> Vec<(String, String)> {
        let source = self.source(items, body);
        if self.library {
            Types::check(&CheckedSource::standard_design_of(&source), "Top").err().unwrap_or_default().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
        } else {
            CheckedSource::outcome(&source)
        }
    }

    /// The source of `body` is refused with one diagnostic of `code` whose message holds every word of `words`.
    fn assert_refused(&self, items: &str, body: &str, code: &str, words: &[&str]) {
        Verdict::assert_one(&self.outcome(items, body), body, code, words);
    }

    /// The diagnostics of the whole-model check of the sources after the stand-in and `items`, as `(code, message)`.
    fn model_outcome(items: &str) -> Vec<(String, String)> {
        let design = CheckedSource::design_of(&format!("{QUAD}\n{items}"));
        Types::check_model(&design).err().unwrap_or_default().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }

    /// The type of the `let` binding `name` of `body` over the stand-in, which must check clean.
    fn local_type(body: &str, name: &str) -> Ty {
        CheckedSource::local_type(&CheckedSource::source(QUAD, body), name)
    }

    /// The type, written as the author writes it, of the `let` binding `name` of `body` in a module checked over the library.
    fn library_local_type(body: &str, name: &str) -> String {
        let src = CheckedSource::source("", body);
        let design = CheckedSource::standard_design_of(&src);
        let types = Types::check(&design, "Top").unwrap_or_else(|found| panic!("{:?}\n{src}", found.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
        let binding = CheckedSource::declaration(&design, name, |kind| matches!(kind, DeclKind::Binding(_)));
        types.local(binding).unwrap_or_else(|| panic!("`{name}` has no type")).source_name(&design)
    }
}

/// What a checker's answer is held to.
struct Verdict;

impl Verdict {
    /// `found` holds one diagnostic of `code` whose message holds every word of `words`.
    fn assert_one(found: &[(String, String)], what: &str, code: &str, words: &[&str]) {
        assert_eq!(found.len(), 1, "one diagnostic for `{what}`, got {found:?}");
        assert_eq!(found[0].0, code, "`{what}`: {found:?}");
        for word in words {
            assert!(found[0].1.contains(word), "`{what}`: `{}` does not name `{word}`", found[0].1);
        }
    }
}

#[test]
fn an_operator_on_a_struct_is_its_impl_function_and_takes_its_out() {
    // H.10 rule 1: `&`, `|`, `^`, `!`, `<<` give the `Quad` the `impl` returns; `==` and `!=` give the `Out` the `impl`
    // names, a `Quad` here, not a `bool`
    let body = "let q = quad(); let a = q & q; let b = q | q; let c = q ^ q; let d = !q; let e = q << 1; let f = q == q; let g = q != q;";
    let wanted = Subject::local_type(body, "q");
    assert!(matches!(wanted, Ty::Struct(_)), "`quad()` is a struct: {wanted:?}");
    for name in ["a", "b", "c", "d", "e", "f", "g"] {
        assert_eq!(Subject::local_type(body, name), wanted, "`{name}` is the `Out` of its `impl`, a `Quad`");
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
    Subject::STAND_IN.assert_refused("", "let a = q >> 1;", "E0180", &["`Quad`", "Shr"]);
    Subject::STAND_IN.assert_refused("", "let a = q & true;", "E0180", &["BitAnd", "`Quad`", "`bool`"]);
    Subject::STAND_IN.assert_refused("struct Plain { a: u4 }", "let p = Plain { a: 1_u4 }; let b = p & p;", "E0180", &["`Plain`", "BitAnd"]);
}

#[test]
fn two_impls_for_one_type_and_one_rhs_are_e0187() {
    // H.10 rule 2: a second `impl BitAnd for Quad` with the same `Rhs` is `E0187`
    Verdict::assert_one(&Subject::model_outcome("impl BitAnd for Quad { fn bitand(self, r: Quad) -> Quad { r } }"), "a second `impl BitAnd for Quad`", "E0187", &["BitAnd", "Quad"]);
}

#[test]
fn control_refuses_a_logic_and_an_option() {
    // EU-03 AC 1 (§3.3.5, C8): a four-state value, the test-local one and the library's `Logic<4>` alike, or an `Option`
    // as a condition, a guard or a short-circuit operand is `E0180` naming the type and `bool`
    for subject in Subject::BOTH {
        let named = format!("`{}`", subject.written);
        subject.assert_refused("", "if q { }", "E0180", &[&named, "`bool`"]);
        subject.assert_refused("", "while q { break; }", "E0180", &[&named, "`bool`"]);
        subject.assert_refused("", "let a = q && true;", "E0180", &[&named, "`bool`"]);
        subject.assert_refused("", "let a = true || q;", "E0180", &[&named, "`bool`"]);
    }
    Subject::STAND_IN.assert_refused("", "let o: Option<u4> = None; if o { }", "E0180", &["`Option<u4>`", "`bool`"]);
    Subject::STAND_IN.assert_refused("", "let o: Option<u4> = None; let a = o && true;", "E0180", &["`Option<u4>`", "`bool`"]);
}

#[test]
fn analog_refuses_a_bare_logic() {
    // EU-03 AC 3 (§3.3.6, C10): a four-state value, the test-local one and the library's alike, where `analog` needs an
    // `f64` or a `bool` is `E0180`
    for subject in Subject::BOTH {
        let named = format!("`{}`", subject.written);
        subject.assert_refused("", "let a: f64 = q;", "E0180", &[&named, "`f64`"]);
        subject.assert_refused("", "let a: bool = q;", "E0180", &[&named, "`bool`"]);
        subject.assert_refused("fn volts(x: f64) -> f64 { x }", "let a = volts(q);", "E0180", &[&named, "`f64`"]);
    }
}

#[test]
fn arithmetic_and_ordering_refuse_a_logic() {
    // EU-08 AC 7 (H.10 rule 5, C33): `+`, `-`, `*`, `/` and `<`, `<=`, `>`, `>=` have no trait; a four-state value, the
    // test-local one and the library's alike, is `E0180` under each
    for subject in Subject::BOTH {
        for operator in ["+", "-", "*", "/", "<", "<=", ">", ">="] {
            let found = subject.outcome("", &format!("let a = q {operator} q;"));
            assert!(!found.is_empty(), "`q {operator} q` on `{}` is refused", subject.written);
            for (code, message) in &found {
                assert_eq!(code, "E0180", "`q {operator} q` on `{}`: {found:?}", subject.written);
                let needs = if operator.starts_with('<') || operator.starts_with('>') { "PartialOrd" } else { "`f64`" };
                assert!(message.contains(subject.written) && message.contains(needs), "`q {operator} q`: `{message}` names the type and what the operator needs ({needs})");
            }
        }
    }
}

#[test]
fn deriving_partial_eq_over_a_logic_is_e0187() {
    // EU-09 AC 4 (§22.1.2 rule 5, C37): the derived `==` returns a `bool` and a field whose `==` returns a four-state
    // value, the test-local one or the library's `Logic<4>`, gives none: `E0187` naming the field and its type
    for subject in Subject::BOTH {
        let items = format!("#[derive(PartialEq)] struct Pair {{ first: {}, second: u4 }}", subject.written);
        let named = format!("`{}`", subject.written);
        subject.assert_refused(&items, "let p = Pair { first: q, second: 1_u4 };", "E0187", &["first", &named, "PartialEq"]);
    }
}

#[test]
fn logic_is_compiled_from_the_library_source() {
    // EU-07 AC 1 (C23, §22.1.1): `Logic<N>` is a struct of `std/logic.msl` and no table of the language names it; the
    // operators of H.10 and `From` are its `impl`s, so `Logic::from(5u4) & ..` is a `Logic<4>`, `==` is a `Logic<1>`
    // (`Out`), and `to_bits()` is an `Option<UInt<4>>`
    assert!(BUILTINS.iter().all(|row| !row.name.starts_with("Logic")), "no built-in row names Logic");
    let design = piperine_lang::pom::Design::builder().source("top.msl", "module Top() { }").build();
    let logic = design.std_crate().find("logic::Logic").expect("std declares logic::Logic");
    assert!(logic.file().name().ends_with("logic.msl"), "declared in {}", logic.file().name());
    let body = "let a = Logic::from(5u4); let b = a & a; let c = a == a; let d = a.to_bits(); let e = !a; let f = a & 3u4; let g = 3u4 | a;";
    assert_eq!(Subject::library_local_type(body, "a"), "Logic<4>");
    for name in ["b", "e", "f", "g"] {
        assert_eq!(Subject::library_local_type(body, name), "Logic<4>", "`{name}` is the `Out` of its `impl`");
    }
    assert_eq!(Subject::library_local_type(body, "c"), "Logic<1>", "`==` gives a `Logic`, not a `bool` (H.10)");
    assert_eq!(Subject::library_local_type(body, "d"), "Option<u4>");
}

#[test]
fn an_associated_call_that_gives_no_const_argument_is_e0181_naming_its_function() {
    // §3.8.7: `Logic::default()` has nothing that gives the `N` of its `impl` when it is an operand of `==` with a
    // `Logic<1>`; the `E0181` names the function and writes the turbofish that would give it, never an empty name
    let found = Subject::LIBRARY.outcome("", "let a = Logic::default() == Logic::from(true);");
    Verdict::assert_one(&found, "Logic::default()", "E0181", &["`N`", "`Logic::default`", "Logic::<..>::default"]);
}
