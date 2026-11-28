//! Scope: chapter 3 of the Book, "Your own types" (feature `root-e2e`, RE-02 and RE-03): the three files
//! of the chapter (`option.msl`, `levels.msl`, `family.msl`) run through `piperine::` and give the values
//! the chapter states, optional arguments are left out and named, the listings whose forms the compiler
//! does not run yet (tuples, arrays, an enum with fields, structs, methods, range patterns) are asserted
//! refused where a top reaches them, the forms the status box lists are asserted refused one by one, and
//! each mistake of the chapter's table gets the code the chapter names. Each test opens with the inventory
//! ids it holds, from `tests/book/inventory.tsv`.

use crate::support;

use piperine::{Field, Overrides};
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::printed::Printed;
use support::records::Fields;
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
fn an_enum_is_matched_compared_and_printed_by_the_name_of_its_variant() {
    // types#8 (run, `levels.msl`) with its listing types#10; types~10cb5d87 "The line above the enum is an
    // attribute. #[derive(..)] asks the compiler to write standard behavior for the type that follows:
    // PartialEq gives it == and !=, PartialOrd gives it < and >
    // in the order of the declaration, and Repr gives it a layout in bits" (the field `seen` is the variant's
    // name since composite-values CV-22, `high` uses `==`, `above` uses `>`); a `use Level::*;` names the variants bare
    let record = ValueFile::only(Chapter::read().marked("levels.msl", "types#8").run(ValueFile::transient()), "levels");
    ValueFile::variant(&record, "seen", "High");
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
    // types~39ba351d (E0180): the `match` of `levels.msl` with a variant `Over` added
    let source = Chapter::read().text("types#8").replace("enum Level { Low, Between, High }", "enum Level { Low, Between, High, Over }");
    let faulty = Faulty::elaborating("types~545f1643", &source, "Levels");
    // E0184 since composite-values T14: the exhaustiveness check names a pattern the arms miss (§1.8.6)
    assert_eq!(faulty.first("E0184").message, "a `match` over `Level` is not exhaustive: `Level::Over` is not covered (§1.8.6)");
    // the first row of the table: an `Option<f64>` is not an `f64`
    let added = ValueFile::refused("types~39ba351d", "", "let limit: Option<f64> = Some(3.3); let x = limit + 1.0; emit!(\"r\", x = x);");
    assert_eq!(added.first("E0180").message, "mismatched types: expected `f64`, found `Option<f64>`");
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
    // types#7 (run): `mean`, generic over the length; types#16 (run): the method `switches_at`.
    // (types#6, the tuple `ordered`, and types#12, the structs `Reading` and `Volts`, run since composite-values
    // T48 to T51, the table `TAPS` with `smoothed` of types#7 since T55, and types#11, the enum `Shape` with
    // fields and no `Repr`, since composite-values finding 3: see `tuples_and_structs_run_where_a_top_reaches_them`,
    // `a_table_const_runs_where_a_top_reaches_it` and `an_enum_without_repr_runs_as_a_value`.)
    // When one of them runs this test fails and its row moves to `run`
    let mean = Chapter::read().wrapped("reached.msl", "", "types#7", &Reached::top("emit!(\"r\", s = mean(TAPS));")).run(Reached::overrides()).unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("3.333333e-1", Fields::real(mean.stream("r")[0], "s"), "mean(TAPS)");
    let family = "#[derive(PartialEq, Repr)]\nenum Family { Cmos, Ttl }\n";
    let high = Chapter::read().wrapped("reached.msl", family, "types#16", &Reached::top("emit!(\"r\", s = reads_high(Family::Cmos, 2.0, 3.3));")).run(Reached::overrides()).unwrap_or_else(|failure| panic!("{failure}"));
    ValueFile::flag(high.stream("r")[0], "s", 2.0 > 0.5 * 3.3);
}

#[test]
fn tuples_and_structs_run_where_a_top_reaches_them() {
    // types#6 (run): `ordered` yields a tuple and `span` takes it apart; types#12 (run): the structs
    // `Reading` and `Volts`, built by a struct expression and read by a field
    let span = ValueFile::only(Chapter::read().wrapped("reached.msl", "", "types#6", &Reached::top("emit!(\"r\", s = span(2.0, 0.8));")).run(Reached::overrides()), "r");
    Printed::holds("1.200000e0", Fields::real(&span, "s"), "span");
    let statements = "let r = Reading { volts: 3.0, valid: true }; emit!(\"r\", s = scaled(r, 0.5).volts, ok = scaled(r, 0.5).valid);";
    let scaled = ValueFile::only(Chapter::read().wrapped("reached.msl", "", "types#12", &Reached::top(statements)).run(Reached::overrides()), "r");
    Printed::holds("1.500000e0", Fields::real(&scaled, "s"), "scaled");
    ValueFile::flag(&scaled, "ok", true);
}

#[test]
fn an_enum_without_repr_runs_as_a_value() {
    // types#11 (run, composite-values CV-11 AC 1, part I section 1.3 and 2: every enum is a value type): `area` of the
    // chapter takes an enum with fields and no `Repr` apart by `match`; a signal needs `Repr`, a value does not
    let areas = "let c = area(Shape::Circle(1.0)); let r = area(Shape::Rect { w: 2.0, h: 3.0 }); let d = area(Shape::Dot); emit!(\"r\", circle = c, rect = r, dot = d);";
    let record = ValueFile::only(Chapter::read().wrapped("reached.msl", "", "types#11", &Reached::top(areas)).run(Reached::overrides()), "r");
    Printed::holds("3.141593e0", Fields::real(&record, "circle"), "circle");
    Printed::holds("6.000000e0", Fields::real(&record, "rect"), "rect");
    Printed::holds("0.000000e0", Fields::real(&record, "dot"), "dot");
}

#[test]
fn a_table_const_runs_where_a_top_reaches_it() {
    // types#7 (run, composite-values T55): `smoothed` reads the table `TAPS` of the chapter, a `const` array written
    // out where it is read, and sums `TAPS[k] * x[k]`: 0.25 * 1 + 0.5 * 2 + 0.25 * 3
    let smoothed = ValueFile::only(Chapter::read().wrapped("reached.msl", "", "types#7", &Reached::top("emit!(\"r\", s = smoothed([1.0, 2.0, 3.0]));")).run(Reached::overrides()), "r");
    Printed::holds("2.000000e0", Fields::real(&smoothed, "s"), "smoothed");
}

#[test]
fn a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it() {
    // types~5628120b "A model that uses one of them is stopped before the run with the code P0001 and a message
    // that names the form, so nothing is ever computed by a rule other than the ones above": the forms of the
    // chapter's status box, each on the smallest model (when one of them runs, this test fails and the box
    // loses it; tuples, arrays, structs, enums with fields and numbered variants run since composite-values
    // T48 to T51, `tuples_and_structs_run_where_a_top_reaches_them`)
    // the one form left on the box ran through a method outside the catalogue: it runs now, with the answer Rust gives
    let level = "#[derive(PartialEq, Repr)] enum Z { A, B }\nimpl Z { fn next(self) -> Z { match self { Z::A => Z::B, Z::B => Z::A } } }";
    let ran = ValueFile::written(level, "emit!(\"r\", y = Z::B.next() == Z::A);");
    ValueFile::flag(&ran, "y", true);
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
    // (GAPS.md S3); one the top reaches runs, an enum without `Repr` being a value (composite-values CV-11) and the range
    // pattern of a `match` arm since T14, except `mean`, a generic function (L7). When it runs this test fails and the
    // row moves to `run`
    let chapter = Chapter::read();
    let reaching = |call: &str| chapter.wrapped("carry.msl", "", "types#17", &format!("module Top() {{ }}\ndigital for Top {{ on at(1u) {{ {call} }} }}\n"));
    let overrides = || Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let ran = reaching("let z = range_of(3);").run(overrides());
    assert!(ran.is_ok(), "`range_of` runs: an enum without `Repr` is a value and a range pattern an arm");
    assert!(reaching("let z = mean([1.0, 2.0]);").run(overrides()).is_ok(), "`mean` runs: a generic function over the length of an array");
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
fn e0101_nothing_is_lent() {
    // types~399f441d "E0101 fn mean(xs: &[f64])": a reference is no form of the language; the table is taken itself (a function with a const length is not run yet, so only the refusal is held)
    let faulty = Faulty::elaborating("types~399f441d", "fn mean(xs: &[f64]) -> f64 { 0.0 }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0101").message, "references and borrows are not part of the language");
    assert_eq!(faulty.first("E0101").help.as_deref(), Some("pass the value itself: every value is copied where it is used"));
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
    assert_eq!(integer.first("E0180").message, "mismatched types: expected `f64`, found `an integer literal`");
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
fn e0161_copying_is_not_a_trait() {
    // types~39a5d01d "E0161 #[derive(Clone, Copy)]": `Clone` and `Copy` are no names; every value is copied where it is
    // used, so the enum is carried and compared without them
    let faulty = Faulty::elaborating("types~39a5d01d", "#[derive(Clone, Copy, PartialEq, Repr)] enum Z { A, B }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0161").message, "cannot find `Clone` in this scope");
    assert_eq!(faulty.first("E0161").help.as_deref(), Some("every value is copied; delete the name"));
    let plain = ValueFile::written("#[derive(PartialEq, Repr)] enum Z { A, B }", "let z = Z::B; emit!(\"r\", same = z == Z::B);");
    ValueFile::flag(&plain, "same", true);
}
