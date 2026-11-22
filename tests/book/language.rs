//! Scope: chapter 2 of the Book, "Language basics" (feature `root-e2e`, RE-02 and RE-03): the four
//! files of the chapter (`literals.msl`, `arith.msl`, `flow.msl`, `functions.msl`) run through
//! `piperine::` and give the values their comments state, the rules the chapter states in prose (a
//! literal's type, no implicit conversion, `as`, faults, the order of items, the prelude's constants) are
//! asserted on the smallest model that shows each, the forms the status box lists as not run yet are
//! asserted refused, and each mistake of the chapter's table gets the code the chapter names. Each test
//! opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

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

use piperine::Overrides;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
use support::printed::Printed;
use support::records::Fields;
use support::value_file::ValueFile;

#[test]
fn the_literal_forms_have_the_values_their_comments_give() {
    // language#1 (run, `literals.msl`) with its listing language#3; language~11fe9817 "digital for Name { on
    // at(1u) { .. } } holds statements that run once, one microsecond into a transient" and values~2bf57b76
    // "module Literals() {} is a piece of hardware with no ports and nothing inside, and on at(1u) { .. } is a
    // reaction to the event \"the time is 1 us\", which fires once" (one record, at 1 us: `ValueFile::only`);
    // language~afc884b8 "A
    // let may leave the type out. let r = 4.7k; has the type of its right side" (`r * c` is float arithmetic)
    let record = ValueFile::only(Chapter::read().marked("literals.msl", "language#1").run(ValueFile::transient()), "literals");
    ValueFile::int(&record, "count", 1000);
    ValueFile::int(&record, "mask", 255);
    ValueFile::int(&record, "code", 12);
    ValueFile::flag(&record, "ready", true);
    for (name, printed) in [("gain", "2.500000e0"), ("tiny", "1.500000e-3"), ("r", "4.700000e3"), ("c", "1.000000e-8"), ("tau", "4.700000e-5")] {
        Printed::holds(printed, Fields::real(&record, name), name);
    }
    // language~e153bc6a "4.7k 10n 1M"; language~bb3267a4 "10n is the number 1e-8"; language~35824f5f "1u8 is the
    // integer 1 in eight bits and 1u is one millionth"; the octal and binary forms of the table
    let scaled = ValueFile::written("", "emit!(\"r\", k = 4.7k, n = 10n, m = 1M, typed = 1u8, u = 1u, octal = 0o17, binary = 0b1010);");
    for (name, value) in [("k", 4.7e3), ("n", 1e-8), ("m", 1e6), ("u", 1e-6)] {
        assert!((Fields::real(&scaled, name) / value - 1.0).abs() < 1e-12, "{name}: {:?}", scaled.fields);
    }
    ValueFile::int(&scaled, "typed", 1);
    ValueFile::int(&scaled, "octal", 15);
    ValueFile::int(&scaled, "binary", 10);
}

#[test]
fn arithmetic_keeps_the_low_bits_and_as_converts_without_failing() {
    // language#4 (run, `arith.msl`) with its listing language#6; language~332e938b "as converts between numeric
    // types, and never fails" (every row of the chapter's table, NaN included)
    let record = ValueFile::only(Chapter::read().marked("arith.msl", "language#4").run(ValueFile::transient()), "arith");
    for (name, value) in [("wrapped", 4), ("below", 255), ("quotient", 3), ("shifted", 8), ("narrow", 44), ("signed", 65535), ("clamped", 255)] {
        ValueFile::int(&record, name, value);
    }
    Printed::holds("3.500000e0", Fields::real(&record, "ratio"), "ratio");
    Printed::holds("1.250000e2", Fields::real(&record, "half"), "half");
    ValueFile::flag(&record, "both", true);
    assert_eq!(Fields::real(&record, "over_zero"), f64::INFINITY, "a float division by zero is an infinity");
    let converted = ValueFile::written("", "let nan = 0.0 / 0.0; emit!(\"r\", nan = nan as u8, low = -300.7 as i8, flag = true as u8, same = nan == nan, toward_zero = -7 / 2, i32_wraps = 2147483647 + 1, inverted = !0b1010_0101u8);");
    ValueFile::int(&converted, "nan", 0);
    ValueFile::int(&converted, "low", -128);
    ValueFile::int(&converted, "flag", 1);
    ValueFile::flag(&converted, "same", false);
    ValueFile::int(&converted, "toward_zero", -3);
    ValueFile::int(&converted, "i32_wraps", -2147483648);
    ValueFile::int(&converted, "inverted", 90);
    // language~3fa105cb "Any width is a type: a 3-bit duty word is a u3, a 10-bit converter code a u10": each
    // wraps at its own width
    let widths = ValueFile::written("", "let duty: u3 = 5; let code: u10 = 1000; emit!(\"r\", duty = duty + 3, code = code + 24);");
    ValueFile::int(&widths, "duty", 0);
    ValueFile::int(&widths, "code", 0);
    // language~84715108 "In arith.msl, change a + 10 to a + 100 and predict wrapped before you run" (94), and
    // the ratio the same exercise asks for: 4000 / 750
    let tried = ValueFile::written("fn parallel(a: f64, b: f64) -> f64 { a * b / (a + b) }\nfn series(a: f64, b: f64) -> f64 { a + b }", "let a: u8 = 250; emit!(\"r\", wrapped = a + 100, ratio = series(1k, 3k) / parallel(1k, 3k));");
    ValueFile::int(&tried, "wrapped", 94);
    Printed::holds("5.333333e0", Fields::real(&tried, "ratio"), "series over parallel");
}

#[test]
fn nothing_converts_by_itself_and_a_condition_is_a_bool() {
    // The chapter's rule 3 of "Types" ("1 + 1.0 is a type error, and so is passing a u8 where a u16 is asked
    // for"), which is the first row of the chapter's table of mistakes, language~39ba351d (E0180), and "The
    // condition of an if is a bool and nothing else"; language~03460845 "A literal too large
    // for its type is the error E0909, found before the run"
    for (statements, expected, found) in [
        ("let x = 1 + 1.0; emit!(\"r\", x = x);", "f64", "an integer literal"),
        ("let a: u8 = 3; let b: u16 = a; emit!(\"r\", b = b);", "u16", "u8"),
        ("let x = if 1 { 2 } else { 3 }; emit!(\"r\", x = x);", "bool", "i32"),
    ] {
        let faulty = ValueFile::refused("language~03460845", "", statements);
        assert_eq!(faulty.first("E0180").message, format!("mismatched types: expected `{expected}`, found `{found}`"), "{statements}");
    }
    let too_large = ValueFile::refused("language~03460845", "", "let a: u8 = 300; emit!(\"r\", a = a);");
    assert_eq!(too_large.first("E0909").message, "`300` is out of range: an integer literal beyond the range of `u8`");
    let partial = ValueFile::refused("language~03460845", "", "let x = 3u8; let y = match x { 0 => 1.0, 1 => 2.0 }; emit!(\"r\", y = y);");
    // E0184 since composite-values T14: the exhaustiveness check names a pattern the arms miss (§1.8.6)
    assert!(partial.first("E0184").message.contains("is not exhaustive"), "the arms cover every value: {}", partial.first("E0184").message);
}

#[test]
fn blocks_ifs_and_matches_are_expressions_with_one_value() {
    // language#7 (run, `flow.msl`) with its listing language#9; language~fb958b67 "let names a value. let x =
    // expr; evaluates the expression and gives the result a name, visible from that statement to the end of
    // the block" (the second `level` hides the first; a name of an inner block ends with it); language~b76ed779
    // "So { let s = x * 10.0; s + 1.0 } is a number"
    let record = ValueFile::only(Chapter::read().marked("flow.msl", "language#7").run(ValueFile::transient()), "flow");
    for (name, printed) in [("level", "3.600000e0"), ("step", "1.000000e0")] {
        Printed::holds(printed, Fields::real(&record, name), name);
    }
    ValueFile::int(&record, "code", 36);
    ValueFile::int(&record, "grade", 2);
    ValueFile::flag(&record, "inside", false);
    let scoped = ValueFile::written("", "let x = 2.0; let y = { let s = x * 10.0; s + 1.0 }; let outer = 5; { let outer = 1.5; emit!(\"inner\", outer = outer); } emit!(\"r\", y = y, outer = outer);");
    Printed::holds("2.100000e1", Fields::real(&scoped, "y"), "the block's last expression is its value");
    ValueFile::int(&scoped, "outer", 5);
    let gone = ValueFile::refused("language~fb958b67", "", "{ let inner = 1; } emit!(\"r\", x = inner);");
    assert_eq!(gone.first("E0161").message, "cannot find `inner` in this scope");
}

#[test]
fn constants_and_functions_compute_before_and_during_the_run() {
    // language#10 (run, `functions.msl`) with its listing language#12; language~49c521bb "Running today: bool,
    // integers of up to 64 bits, f64, constants, functions, let and blocks, if, and match over literals" (this
    // file and the three before it are those forms, and each runs)
    let record = ValueFile::only(Chapter::read().marked("functions.msl", "language#10").run(ValueFile::transient()), "functions");
    for (name, printed) in [("high", "2.310000e0"), ("rp", "7.500000e2"), ("vt", "2.585200e-2"), ("margin", "4.900000e-1")] {
        Printed::holds(printed, Fields::real(&record, name), name);
    }
    ValueFile::flag(&record, "valid", true);
    ValueFile::flag(&record, "between", false);
    // "Items may be written in any order: a function may call one declared further down, and a constant may
    // use a later constant"; 64-bit integers wrap at 64 bits
    let ordered = ValueFile::written("const H: f64 = 2.0 * V;\nconst V: f64 = 3.3;\nfn first() -> f64 { second() }\nfn second() -> f64 { H }", "let big: u64 = 18446744073709551615; emit!(\"r\", h = first(), big = big + 2);");
    Printed::holds("6.600000e0", Fields::real(&ordered, "h"), "a constant computed from a later one, through two functions");
    ValueFile::int(&ordered, "big", 1);
    // "A function with no -> yields ()": it runs for what it writes
    let unit = Model::written("unit.msl", "fn say(x: f64) { emit!(\"said\", x = x); }\nmodule Top() {}\ndigital for Top { on at(1u) { say(2.0); } }\n").run(ValueFile::transient()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(unit.stream("said").len(), 1, "{:?}", unit.records);
}

#[test]
fn the_prelude_gives_its_constants() {
    // language~9273ae3d: "The prelude gives you the constants Q, K_B, EPS0, PI, E and inf"
    let c = ValueFile::written("", "emit!(\"r\", q = Q, kb = K_B, eps0 = EPS0, pi = PI, e = E, i = inf);");
    let real = |name: &str| Fields::real(&c, name);
    Printed::holds("1.602177e-19", real("q"), "Q");
    Printed::holds("1.380649e-23", real("kb"), "K_B");
    Printed::holds("8.854188e-12", real("eps0"), "EPS0");
    Printed::holds("3.141593e0", real("pi"), "PI");
    Printed::holds("2.718282e0", real("e"), "E");
    assert!(real("i").is_infinite() && real("i") > 0.0, "inf");
}

#[test]
fn a_fault_ends_the_analysis_or_the_build_and_a_non_finite_value_ends_the_solve() {
    // "Dividing an integer by zero is a fault: the analysis ends there, naming the instance and the time";
    // "A fault in a constant or a param default is reported when the design is built"; language~a3318a63 "A
    // value that is not finite and arrives at the analog solver ends the solve with an error, because no
    // circuit has an infinite voltage"
    let divided = Model::written("m.msl", "module Top() {}\ndigital for Top { on at(1u) { let a = 7u8; let b = 0u8; emit!(\"r\", q = a / b); } }\n").run(ValueFile::transient()).err().expect("an integer division by zero is a fault");
    let said = format!("{divided:#}");
    assert!(said.contains("E0105") && said.contains("in `Top` at tick 1000000000: integer division by zero"), "the fault is E0105 naming the instance and the time: {said}");
    let unwrapped = Model::written("m.msl", "module Top() {}\ndigital for Top { on at(1u) { let o: Option<f64> = None; emit!(\"r\", v = o.unwrap()); } }\n").run(ValueFile::transient()).err().expect("`unwrap` on `None` is a fault");
    assert!(format!("{unwrapped:#}").contains("E0105") && format!("{unwrapped:#}").contains("called `Option::unwrap()` on a `None` value"), "{unwrapped:#}");
    let constant = ValueFile::refused("language~a3318a63", "const D: u8 = 1 / 0;", "emit!(\"r\", d = D);");
    assert_eq!(constant.first("E0104").message, "a constant expression panics while it is evaluated at elaboration: an integer division by zero");
    let source = "module R(p: Electrical, n: Electrical) { r: f64 = 1k; }\nanalog for R { i(p, n) +> v(p, n) / r; }\nmodule Bad(p: Electrical, n: Electrical) { z: f64 = 0.0; }\nanalog for Bad { v(p, n) +> 1.0 / z; }\nmodule Top() { signal a: Electrical; let s = Bad(a, gnd); let r = R(a, gnd); }\n";
    let solved = Model::written("m.msl", source).run(Faulty::operating_point_of("Top")).err().expect("a contribution that is not finite ends the solve");
    let said = format!("{solved:#}");
    assert!(said.contains("E0507: instance `Top.s`") && said.contains("is not finite"), "{said}");
}

#[test]
fn the_sum_below_listing_runs_with_the_literal_typed_by_its_use() {
    // language#13 (run): the chapter's `sum_below` is a local that is updated, an assignment and a loop (composite-values
    // T11 to T20, every local a place since value-semantics T2 and T3); `let total = 0` then `total += k` with `k: u32`
    // is a literal whose type the later use fixes (§1.4.4), so `total` is a `u32` and the sum of 0, 1, 2 and 3 is 6.
    let chapter = Chapter::read();
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let source = "module Top() { }\ndigital for Top { on at(1u) { emit!(\"r\", s = sum_below(4)); } }\n";
    let record = ValueFile::only(chapter.wrapped("sum.msl", "", "language#13", source).run(overrides), "r");
    ValueFile::int(&record, "s", 6);
}

#[test]
fn a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it() {
    // language~56bbb0e9 "Parsed and checked, and not run yet: an item declared inside a block, % on floats,
    // floats other than f64, and integers wider than 64 bits"; language~5628120b "A model that uses one of them is stopped before the run with the
    // code P0001 and a message that names the form, so nothing is ever computed by a rule other than the ones
    // above": the forms of the status box, each on the smallest model (when one of them runs, this test fails
    // and the box loses it)
    for (items, statements, named) in [
        ("", "const K: f64 = 4.0; emit!(\"r\", y = K);", "item inside a block is not supported yet"),
        ("", "let p = 7.5 % 2.0; emit!(\"r\", p = p);", "operator `%` is not supported yet"),
        ("", "let x: f32 = 1.5; emit!(\"r\", x = x);", "type `f32` is not supported yet"),
        ("", "let x: u128 = 1; emit!(\"r\", x = x);", "integer type `u128` is not supported yet"),
    ] {
        let faulty = ValueFile::refused("language~5628120b", items, statements);
        assert!(faulty.given.iter().any(|reported| reported.code == "P0001" && reported.message == named), "{statements}: {:?}", faulty.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
    }
    // an assignment to a parameter is the error E0183 (§1.5.12, §3.1.10), no longer a form that waits
    let parameter = ValueFile::refused("language~5628120b", "fn bump(x: u32) -> u32 { x = 6; x }", "emit!(\"r\", x = bump(1));");
    assert!(parameter.given.iter().any(|reported| reported.code == "E0183"), "{:?}", parameter.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
}

#[test]
fn e0909_e0905_and_e0903_a_literal_too_large_a_character_and_a_keyword_as_a_name() {
    // language~8031841d: "E0909 let a: u8 = 300;"; language~8031801d: "E0905 let c = 'a';"; language~80317e1d:
    // "E0903 let module = 2;" and its fix, the raw form `r#module`
    let too_large = ValueFile::refused("language~8031841d", "", "let a: u8 = 300; emit!(\"r\", a = a);");
    assert_eq!(too_large.pointed_at("E0909"), "300");
    let character = ValueFile::refused("language~8031801d", "", "let c = 'a'; emit!(\"r\", x = 1);");
    assert_eq!(character.first("E0905").message, "character literal is not part of the language");
    let keyword = ValueFile::refused("language~80317e1d", "", "let module = 2; emit!(\"r\", y = module);");
    assert_eq!(keyword.first("E0903").message, "`module` is a reserved keyword and cannot be used as an identifier");
    let raw = ValueFile::written("", "let r#module = 2; emit!(\"r\", y = r#module);");
    ValueFile::int(&raw, "y", 2);
}

#[test]
fn e0903_mut_is_not_a_word_of_the_language() {
    // language~80317e1d "E0903 let mut x = 0;": `mut` is a reserved word; a local needs no marker to be assigned, so the
    // plain `let x = 0;` runs and assigns
    let refused = ValueFile::refused("language~80317e1d", "", "let mut x = 5; x = 6; emit!(\"r\", x = x);");
    assert_eq!(refused.first("E0903").message, "`mut` is a reserved keyword");
    assert_eq!(refused.first("E0903").help.as_deref(), Some("write the plain form `let x`: every local may be assigned"));
    let plain = ValueFile::written("", "let x = 5; x = 6; emit!(\"r\", x = x);");
    ValueFile::int(&plain, "x", 6);
}

#[test]
fn the_book_says_numbers_have_no_default() {
    // EU-18 AC 2: the chapter on the basics says, in its own words and without borrowing another
    // language's name for it, that no primitive numeric type has a default value, and the types
    // chapter lists it among the things a model does not do (generics~8faad162, the operator traits,
    // is prose).
    let book = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/book/content.html")).expect("the Book");
    let chapter = |id: &str| {
        let start = book.find(&format!("<section id=\"{id}\"")).unwrap_or_else(|| panic!("chapter {id}"));
        book[start..start + book[start..].find("</section>").expect("a chapter closes")].to_string()
    };
    let language = chapter("language");
    assert!(language.contains("<strong>a number has no default value</strong>"), "the language chapter states it");
    assert!(language.contains("<code>u8::default()</code> does not exist"), "and names the case");
    assert!(!book.contains("same as Rust") && !book.contains("as in Rust"), "the Book never explains by another language's name");
    let types = chapter("types");
    assert!(types.contains("A default value for a number (<code>u8::default()</code>)"), "the types chapter lists it among the limits");
    let generics = chapter("generics");
    assert!(generics.contains("numbers and <code>bool</code> have the traits of the other rows but not <code>Default</code>"), "the Default row says it");
    assert!(generics.contains("<strong>Operators are traits too.</strong>"), "the operator traits are taught");
}
