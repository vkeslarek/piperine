//! Scope: chapter 3 of the Book, "Your own types" (feature `root-e2e`, RE-02 and RE-03): the three files
//! of the chapter (`option.msl`, `levels.msl`, `family.msl`) run through `piperine::` and give the values
//! the chapter states, optional arguments are left out and named, the listings whose forms the compiler
//! does not run yet (tuples, arrays, an enum with fields, structs, methods, range patterns) are asserted
//! refused where a top reaches them, the forms the status box lists are asserted refused one by one, and
//! each mistake of the chapter's table gets the code the chapter names. Each test opens with the inventory
//! ids it holds, from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod inventory;
    pub mod model;
    pub mod printed;
    pub mod records;
    pub mod refusal;
    pub mod value_file;
}

use piperine::{Failure, Field, Overrides};
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Ran;
use support::printed::Printed;
use support::records::Fields;
use support::refusal::Refusal;
use support::value_file::ValueFile;

/// A listing of the chapter whose forms the compiler does not run yet, reached from a top.
struct Reached;

impl Reached {
    /// The overrides of `piperine run <file> --top Top --analysis transient --stop 2e-6`.
    fn overrides() -> Overrides {
        Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() }
    }

    /// The text after a listing that makes a top run `statements` in a reaction at 1 us.
    fn top(statements: &str) -> String {
        format!("module Top() {{ }}\ndigital for Top {{ on at(1u) {{ {statements} }} }}\n")
    }

    /// The run of the block `id`, after `before`, on a top that runs `statements`.
    fn run(id: &str, before: &str, statements: &str) -> Result<Ran, Failure> {
        Chapter::read().wrapped("reached.msl", before, id, &Self::top(statements)).run(Self::overrides())
    }
}

#[test]
fn an_option_is_read_with_its_methods_a_match_and_an_if_let() {
    // types#1 (run, `option.msl`) with its listing types#3; types~d5d7690f "The ramp reaches 1.5 V at 1.5 us,
    // which is later than 1 us. never has no value, so unwrap_or(-1.0) yields the fallback and held takes its
    // None arm"
    let record = ValueFile::only(Chapter::read().marked("option.msl", "types#1").run(ValueFile::transient()), "absent");
    for (name, value) in [("found", true), ("late", true), ("inside", true)] {
        ValueFile::flag(&record, name, value);
    }
    for (name, printed) in [("when", "1.500000e-6"), ("fallback", "-1.000000e0"), ("held", "3.300000e0"), ("nothing", "0.000000e0")] {
        Printed::holds(printed, Fields::real(&record, name), name);
    }
    // the chapter's exercise: with a slope of 0.0 `rising` is `None`
    let flat = ValueFile::written("", "let slope = 0.0; let rising: Option<f64> = if slope > 0.0 { Some(1.5 / slope) } else { None }; let late = if let Some(t) = rising { t > 1u } else { false }; emit!(\"r\", found = rising.is_some(), when = rising.unwrap_or(0.0), late = late, none = rising.is_none());");
    ValueFile::flag(&flat, "found", false);
    ValueFile::flag(&flat, "late", false);
    ValueFile::flag(&flat, "none", true);
    Printed::holds("0.000000e0", Fields::real(&flat, "when"), "when");
    // "o.unwrap() is the value inside and a fault on None"
    let inside = ValueFile::written("", "let o: Option<f64> = Some(2.5); emit!(\"r\", v = o.unwrap());");
    Printed::holds("2.500000e0", Fields::real(&inside, "v"), "unwrap");
}

#[test]
fn an_enum_is_matched_compared_and_printed_by_the_number_of_its_variant() {
    // types#8 (run, `levels.msl`) with its listing types#10; types~10cb5d87 "The line above the enum is an
    // attribute. #[derive(..)] asks the compiler to write standard behavior for the type that follows:
    // PartialEq gives it == and !=, PartialOrd gives it < and >
    // in the order of the declaration, and Repr gives it a layout in bits" (the field `seen` is the variant's
    // number, `high` uses `==`, `above` uses `>`); a `use Level::*;` names the variants bare
    let record = ValueFile::only(Chapter::read().marked("levels.msl", "types#8").run(ValueFile::transient()), "levels");
    ValueFile::int(&record, "seen", 2);
    ValueFile::flag(&record, "high", true);
    ValueFile::flag(&record, "above", true);
    ValueFile::int(&record, "weight", 2);
    let level = "#[derive(PartialEq, PartialOrd, Repr)]\nenum Level { Low, Between, High }\nuse Level::*;";
    let bare = ValueFile::written(level, "let l = Between; emit!(\"r\", same = l == Level::Between, ordered = Low < Between && Between < High);");
    ValueFile::flag(&bare, "same", true);
    ValueFile::flag(&bare, "ordered", true);
}

#[test]
fn a_match_that_misses_a_variant_stops_the_build() {
    // types~545f1643 "Add a fourth variant later and this match stops the build until it has an arm for it, so
    // a new case cannot be forgotten silently" and the second row of the chapter's table of mistakes,
    // types~79f450b1 (P0002): the `match` of `levels.msl` with a variant `Over` added
    let source = Chapter::read().text("types#8").replace("enum Level { Low, Between, High }", "enum Level { Low, Between, High, Over }");
    let faulty = Faulty::elaborating("types~545f1643", &source, "Levels");
    // E0184 since composite-values T14: the exhaustiveness check names a pattern the arms miss (§1.8.6)
    assert_eq!(faulty.first("E0184").message, "a `match` over `enum `Level`` is not exhaustive: `Level::Over` is not covered (§1.8.6)");
    // the first row of the table: an `Option<f64>` is not an `f64`
    let added = ValueFile::refused("types~79f450b1", "", "let limit: Option<f64> = Some(3.3); let x = limit + 1.0; emit!(\"r\", x = x);");
    assert_eq!(added.first("P0002").message, "mismatched types: expected `f64`, found `Option<f64>`");
    let fixed = ValueFile::written("", "let limit: Option<f64> = Some(3.3); let x = limit.unwrap_or(0.0) + 1.0; emit!(\"r\", x = x);");
    Printed::holds("4.300000e0", Fields::real(&fixed, "x"), "the fix of the table");
}

#[test]
fn an_impl_gives_a_type_its_constants_and_functions() {
    // types#13 (run, `family.msl`) with its listing types#15: an associated constant, an associated function
    // and `Self`; types~1c8de7ec "The same notation reaches into a namespace, as in std::constants::PI"
    let record = ValueFile::only(Chapter::read().marked("family.msl", "types#13").run(ValueFile::transient()), "family");
    ValueFile::flag(&record, "ttl", true);
    Printed::holds("1.400000e0", Fields::real(&record, "switch"), "switch");
    Printed::holds("1.650000e0", Fields::real(&record, "default_switch"), "default_switch");
    let path = ValueFile::written("", "emit!(\"r\", pi = std::constants::PI, same = std::constants::PI == PI);");
    Printed::holds("3.141593e0", Fields::real(&path, "pi"), "a path into a namespace");
    ValueFile::flag(&path, "same", true);
    // "the word pub in front of it makes it visible from outside": an item of a nested namespace is reached
    // when it is `pub`, and is private otherwise
    let nested = ValueFile::written("mod inner { pub fn open() -> f64 { 2.0 } pub const K: f64 = 3.0; }\nuse inner::open;", "emit!(\"r\", y = open() + inner::K);");
    Printed::holds("5.000000e0", Fields::real(&nested, "y"), "a pub item of a nested namespace");
    let hidden = ValueFile::refused("types~1c8de7ec", "mod inner { fn hidden() -> f64 { 2.0 } }", "emit!(\"r\", y = inner::hidden());");
    assert_eq!(hidden.first("E0161").message, "`hidden` is private");
}

#[test]
fn the_listings_still_on_the_way_are_refused_where_a_top_reaches_them() {
    // types#6 (refused, L2): `ordered` yields a tuple and `span` takes it apart; types#7 (refused, L3): the
    // table `TAPS`, `smoothed` and `mean`; types#11 (refused, L4): `Shape`, an enum with fields; types#12
    // (refused, L1): the structs `Reading` and `Volts`; types#16 (refused, L7): the method `switches_at`.
    // When one of them runs this test fails and its row moves to `run`
    Refusal::expect(Reached::run("types#6", "", "emit!(\"r\", s = span(2.0, 0.8));"), "types#6", "P0001", "a tuple `let` whose value is not a tuple expression is not supported yet");
    Refusal::expect(Reached::run("types#7", "", "emit!(\"r\", s = smoothed([1.0, 2.0, 3.0]));"), "types#7", "P0001", "an expression the type checker gave no value type is not supported yet");
    Refusal::expect(Reached::run("types#7", "", "emit!(\"r\", s = mean(TAPS));"), "types#7", "P0001", "generic function is not supported yet");
    Refusal::expect(Reached::run("types#11", "", "emit!(\"r\", s = area(Shape::Circle(1.0)));"), "types#11", "P0001", "enum `Shape` without `#[derive(Repr)]` is not supported yet");
    Refusal::expect(Reached::run("types#12", "", "let r = Reading { volts: 3.0, valid: true }; emit!(\"r\", s = scaled(r, 0.5).volts);"), "types#12", "P0001", "a name whose type is not a value type is not supported yet");
    Refusal::expect(Reached::run("types#12", "", "let r = Reading { volts: 3.0, valid: true }; emit!(\"r\", s = scaled(r, 0.5).volts);"), "types#12", "P0001", "a field access that is not an item of a placed module is not supported yet");
    let family = "#[derive(PartialEq, Repr)]\nenum Family { Cmos, Ttl }\n";
    Refusal::expect(Reached::run("types#16", family, "emit!(\"r\", s = reads_high(Family::Cmos, 2.0, 3.3));"), "types#16", "P0001", "method `switches_at` on `enum `Family`` is not supported yet");
}

#[test]
fn a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it() {
    // types~5628120b "A model that uses one of them is stopped before the run with the code P0001 and a message
    // that names the form, so nothing is ever computed by a rule other than the ones above": the forms of the
    // chapter's status box, each on the smallest model (when one of them runs, this test fails and the box
    // loses it)
    let level = "#[derive(PartialEq, Repr)] enum Z { A, B }";
    for (items, statements, named) in [
        ("", "let t = (1.5, true); emit!(\"r\", f = t.0);", "a field access that is not an item of a placed module is not supported yet"),
        ("", "let a = [1.0, 2.0]; emit!(\"r\", x = a[0]);", "index expression is not supported yet"),
        ("struct P { x: f64 }", "let p = P { x: 1.0 }; emit!(\"r\", x = p.x);", "a field access that is not an item of a placed module is not supported yet"),
        ("#[derive(PartialEq, Repr)] enum Cmd { Idle, Set(u8) }", "let c = Cmd::Set(3); emit!(\"r\", y = c == Cmd::Idle);", "a call the type checker did not resolve is not supported yet"),
        ("#[derive(PartialEq, Repr)] enum Op { Add = 0b00, Sub = 0b01 }", "let o = Op::Sub; emit!(\"r\", y = o as u8);", "an `as` cast of an enum that writes discriminants or `#[repr]` is not supported yet"),
        ("#[derive(PartialEq)] enum Lv { Low, High }", "let a = Lv::Low; emit!(\"r\", y = a == Lv::Low);", "enum `Lv` without `#[derive(Repr)]` is not supported yet"),
        (level, "emit!(\"r\", y = Z::B.next() == Z::A);", "method `next` on `enum `Z`` is not supported yet"),
    ] {
        let items = if statements.contains(".next()") { format!("{items}\nimpl Z {{ fn next(self) -> Z {{ match self {{ Z::A => Z::B, Z::B => Z::A }} }} }}") } else { items.to_string() };
        let faulty = ValueFile::refused("types~5628120b", &items, statements);
        assert!(faulty.given.iter().any(|reported| reported.code == "P0001" && reported.message == named), "{statements}: {:?}", faulty.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
    }
}

#[test]
fn e0161_an_enum_has_no_other_variant() {
    // types~39a5d01d (E0161), the last row of the chapter's table: "Level::Mid: The enum has no such variant,
    // so the path does not resolve"
    let faulty = ValueFile::refused("types~39a5d01d", "#[derive(PartialEq, Repr)] enum Level { Low, Between, High }", "let l = Level::Mid; emit!(\"r\", y = 1);");
    assert_eq!(faulty.first("E0161").message, "cannot find `Mid` in this scope");
}

#[test]
fn the_forms_still_on_the_way_are_refused_where_the_solver_reaches_them() {
    // types#17 (refused): the chapter's `range_of` and `mean`, the example of its part on what runs today. A function nothing reaches is accepted
    // (GAPS.md S3); one the top reaches is `P0001`: the enum has no `#[derive(Repr)]`, `mean` is a generic function
    // (L7); the range pattern of a `match` arm (L6) is accepted since composite-values T14. When one of
    // them runs this test fails and the row moves to `run`
    let chapter = Chapter::read();
    let reaching = |call: &str| chapter.wrapped("carry.msl", "", "types#17", &format!("module Top() {{ }}\ndigital for Top {{ on at(1u) {{ {call} }} }}\n"));
    let overrides = || Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let range = Refusal::of(reaching("let z = range_of(3);").run(overrides()), "types#17");
    assert!(!range.iter().any(|(_, message)| message.contains("range pattern")), "a range pattern is accepted: {range:?}");
    assert!(range.contains(&("P0001".to_string(), "enum `Range` without `#[derive(Repr)]` is not supported yet".to_string())), "{range:?}");
    Refusal::expect(reaching("let z = mean([1.0, 2.0]);").run(overrides()), "types#17", "P0001", "generic function is not supported yet");
}

#[test]
fn only_two_built_in_bang_forms_exist() {
    // types~95095180: "macro_rules!, println!, format!: Only two built-in ! forms: emit! for
    // output and include_str! for data (E0901)": `println!` and `format!` are `E0901`, `macro_rules!` too
    for source in ["fn f() { println!(\"x\"); }\nmodule Top() {}\n", "fn f() { let s = format!(\"x\"); }\nmodule Top() {}\n", "macro_rules! m { () => {} }\nmodule Top() {}\n"] {
        let faulty = Faulty::elaborating("types~95095180", source, "Top");
        assert_eq!(faulty.first("E0901").message, "macros are not part of the language", "{source}");
    }
}

#[test]
fn e0101_there_are_no_mutable_references() {
    // types~399f441d: "E0101 fn bump(c: &mut u8) { *c += 1; }": no mutable references
    let faulty = Faulty::elaborating("types~399f441d", "fn bump(c: &mut u8) { *c += 1; }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0101").message, "a mutable reference is not part of the language");
}

#[test]
fn e0143_there_are_no_default_arguments() {
    // types~39acbe1d: "E0143 fn scale(x: f64, k: f64 = 2.0)": no default arguments
    let faulty = Faulty::elaborating("types~39acbe1d", "fn scale(x: f64, k: f64 = 2.0) -> f64 { x * k }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.pointed_at("E0143"), "= 2.0");
}

#[test]
fn e0161_there_are_no_heap_types() {
    // types~39a5d01d: "E0161 let xs: Vec<f64> = Vec::new();": no heap types, so the path does not resolve
    let source = "module Top() { }\ndigital for Top { on at(1u) { let xs: Vec<f64> = Vec::new(); } }\n";
    let faulty = Faulty::elaborating("types~39a5d01d", source, "Top");
    assert_eq!(faulty.first("E0161").message, "cannot find `Vec` in this scope");
}

#[test]
fn optional_arguments_may_be_left_out_or_named() {
    // types#4 (run) and types#5 (the three calls, a fragment of it); types~4b156a2d: "Passing a plain
    // T wraps it in Some, the one implicit conversion in the language": `scale(v, 3.0)` is `Some(3.0)`,
    // and an integer literal is never an f64
    let chapter = Chapter::read();
    for call in ["scale(v)", "scale(v, 3.0)", "scale(v, limit: 1.0)"] {
        assert!(chapter.text("types#5").contains(call), "types#5 shows {call}");
    }
    let body = "let v = 5.0; emit!(\"s\", left_out = scale(v), positional = scale(v, 3.0), named = scale(v, limit: 1.0));";
    let model = chapter.wrapped("scale.msl", "", "types#4", &format!("module Top() {{ }}\ndigital for Top {{ on at(1u) {{ {body} }} }}\n"));
    let s = ValueFile::only(model.run(ValueFile::transient()), "s");
    let s = &s;
    assert_eq!(Fields::of(s, "left_out"), &Field::Real(10.0), "k = 2.0 unless given");
    assert_eq!(Fields::of(s, "positional"), &Field::Real(15.0), "k = Some(3.0)");
    assert_eq!(Fields::of(s, "named"), &Field::Real(1.0), "limit skips k and clamps 10.0 to 1.0");
    let integer = Faulty::elaborating("types~4b156a2d", "module Top() { r: f64 = 100; }\n", "Top");
    assert_eq!(integer.first("P0002").message, "mismatched types: expected `f64`, found `an integer literal`");
}

#[test]
fn e0140_a_named_argument_must_name_an_optional_parameter() {
    // types~39acbd1d: "E0140 scale(v, gain: 3.0)": it is called k
    let source = "fn scale(x: f64, k: Option<f64>) -> f64 { x * k.unwrap_or(2.0) }\nmodule Top() { }\n";
    let faulty = Faulty::elaborating("types~39acbd1d", &format!("{source}digital for Top {{ on at(1u) {{ let v = 5.0; emit!(\"s\", a = scale(v, gain: 3.0)); }} }}\n"), "Top");
    assert_eq!(faulty.first("E0140").message, "no optional parameter `gain` in `scale`");
}

#[test]
fn e0142_a_positional_argument_cannot_follow_a_named_one() {
    // types~39acbf1d: "E0142 scale(v, limit: 1.0, 3.0)"
    let source = "fn scale(x: f64, k: Option<f64>, limit: Option<f64>) -> f64 { x * k.unwrap_or(2.0) }\nmodule Top() { }\n";
    let faulty = Faulty::elaborating("types~39acbf1d", &format!("{source}digital for Top {{ on at(1u) {{ let v = 5.0; emit!(\"s\", a = scale(v, limit: 1.0, 3.0)); }} }}\n"), "Top");
    assert_eq!(faulty.first("E0142").message, "a positional argument after a named one");
}

#[test]
fn the_earlier_borrow_and_copy_spellings_are_still_known() {
    // types~ee044194 "Today's compiler still knows both: it stops a model that lends a table with the code P0001
    // and names a reference type, and it accepts the two names in a derive, where they change nothing": `&`,
    // `Clone` and `Copy` left the language with value semantics (spec Appendix G.71, GAPS.md S7). When the
    // front end refuses them this test fails and the status box loses the sentence
    let lent = ValueFile::refused("types~ee044194", "fn mean(xs: &[f64]) -> f64 { xs.len() as f64 }", "emit!(\"r\", x = mean(&[1.0, 2.0]));");
    assert!(lent.given.iter().any(|reported| reported.code == "P0001" && reported.message == "reference type is not supported yet"), "{:?}", lent.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
    let plain = ValueFile::written("#[derive(PartialEq, Repr)] enum Z { A, B }", "let z = Z::B; emit!(\"r\", same = z == Z::B);");
    let derived = ValueFile::written("#[derive(Clone, Copy, PartialEq, Repr)] enum Z { A, B }", "let z = Z::B; emit!(\"r\", same = z == Z::B);");
    ValueFile::flag(&plain, "same", true);
    ValueFile::flag(&derived, "same", true);
}
