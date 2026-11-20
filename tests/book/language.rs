//! Scope: chapter 2 of the Book, "The language rules" (feature `root-e2e`, RE-02 and RE-03): the four
//! files of the value language (`literals.msl`, `arith.msl`, `flow.msl`, `items.msl`) run through
//! `piperine::` and give the values their comments state, the rules the chapter states in prose (a
//! literal's type, no implicit conversion, `as`, faults, the order of items) are asserted on the
//! smallest model that shows each, the blinky model states what the chapter says of it (its three
//! records, its instance path, its parameter, its delayed commit), the forms the status box lists as
//! not run yet are asserted refused, `emit!` with a text field runs, and each mistake of the chapter's
//! table gets the code the chapter names. Each test opens with the inventory ids it holds, from
//! `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod inventory;
    pub mod model;
    pub mod records;
    pub mod refusal;
}

use piperine::{Field, Overrides, Record};
use piperine_parity::golden::PrintedNumber;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
use support::records::Fields;
use support::refusal::Refusal;

/// A number as the Book prints it: the printed digits are the resolution of the comparison.
struct Printed;

impl Printed {
    /// Asserts that `x` rounds to the number the Book prints as `printed` (`5.000000e-7`).
    fn holds(printed: &str, x: f64, what: &str) {
        let number = PrintedNumber::parse(printed).unwrap_or_else(|| panic!("`{printed}` is a printed number"));
        assert!(number.rounds_from(x), "{what}: the Book prints {printed}, the run gives {x:e}");
    }
}

/// The Book's blinky, in the file the Book names `blinky.msl`.
struct Blinky;

impl Blinky {
    /// The model: the blocks the Book marks `blinky.msl`.
    fn model() -> Model {
        Chapter::read().marked("blinky.msl", "language#13")
    }

    /// The overrides of `piperine run blinky.msl --analysis transient --stop 3e-6`, with `extra`
    /// changes applied.
    fn transient(extra: impl FnOnce(&mut Overrides)) -> Overrides {
        let mut overrides = Overrides { analysis: Some("transient".to_string()), stop: Some(3e-6), ..Overrides::default() };
        extra(&mut overrides);
        overrides
    }
}

/// The oscillator and the probe the toggling claims run: `Osc` of blinky with a probe on its clock.
const OSCILLATOR: &str = "module Osc(out clk: Bit = false) { f: f64 = 1M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\nmodule Probe(in clk: Bit) { }\ndigital for Probe { on clk.changed() { emit!(\"toggle\", t = now(), level = clk); } }\n";

/// One file of the value language: the single record its reaction writes at 1 us.
struct ValueFile;

impl ValueFile {
    /// The overrides of `piperine run <file> --analysis transient --stop 2e-6`.
    fn transient() -> Overrides {
        Overrides { analysis: Some("transient".to_string()), stop: Some(2e-6), ..Overrides::default() }
    }

    /// The one record of the stream `stream` of the run `result`, written at 1 us. Panics with the failure
    /// when the run failed.
    fn only(result: Result<support::model::Ran, piperine::Failure>, stream: &str) -> Record {
        let ran = result.unwrap_or_else(|failure| panic!("the file fails: {failure}"));
        let records = ran.stream(stream);
        assert_eq!(records.len(), 1, "the reaction on `at(1u)` fires once: {:?}", ran.records);
        Printed::holds("1.000000e-6", records[0].time, "the instant of the record");
        records[0].clone()
    }

    /// The one record of the stream `r` written by `statements` in a reaction at 1 us, after `items`.
    fn written(items: &str, statements: &str) -> Record {
        let source = format!("{items}\nmodule Top() {{}}\ndigital for Top {{ on at(1u) {{ {statements} }} }}\n");
        let ran = Model::written("rule.msl", &source).run(Self::transient()).unwrap_or_else(|failure| panic!("{source}\nfails: {failure}"));
        let records = ran.stream("r");
        assert_eq!(records.len(), 1, "{:?}", ran.records);
        records[0].clone()
    }

    /// The diagnostics of `statements` in a reaction at 1 us, after `items`, held under the row `row`.
    fn refused(row: &str, items: &str, statements: &str) -> Faulty {
        Faulty::elaborating(row, &format!("{items}\nmodule Top() {{}}\ndigital for Top {{ on at(1u) {{ {statements} }} }}\n"), "Top")
    }

    /// Asserts that the field `name` of `record` is the integer `value`.
    fn int(record: &Record, name: &str, value: i128) {
        assert_eq!(Fields::of(record, name), &Field::Int(value), "the field `{name}`");
    }

    /// Asserts that the field `name` of `record` is the bool `value`.
    fn flag(record: &Record, name: &str, value: bool) {
        assert_eq!(Fields::of(record, name), &Field::Bool(value), "the field `{name}`");
    }
}

#[test]
fn the_literal_forms_have_the_values_their_comments_give() {
    // language#1 (run, `literals.msl`) with its listing language#3; language~2bf57b76 "module Literals() {} is
    // a piece of hardware with no ports and nothing inside, and on at(1u) { .. } is a reaction to the event
    // \"the time is 1 us\", which fires once" (one record, at 1 us: `ValueFile::only`); language~afc884b8 "A
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
}

#[test]
fn nothing_converts_by_itself_and_a_condition_is_a_bool() {
    // The chapter's rule 3 of "Types" ("1 + 1.0 is a type error, and so is passing a u8 where a u16 is asked
    // for") and "The condition of an if is a bool and nothing else"; language~03460845 "A literal too large
    // for its type is the error E0909, found before the run"
    for (statements, expected, found) in [
        ("let x = 1 + 1.0; emit!(\"r\", x = x);", "f64", "an integer literal"),
        ("let a: u8 = 3; let b: u16 = a; emit!(\"r\", b = b);", "u16", "u8"),
        ("let x = if 1 { 2 } else { 3 }; emit!(\"r\", x = x);", "bool", "i32"),
    ] {
        let faulty = ValueFile::refused("language~03460845", "", statements);
        assert_eq!(faulty.first("P0002").message, format!("mismatched types: expected `{expected}`, found `{found}`"), "{statements}");
    }
    let too_large = ValueFile::refused("language~03460845", "", "let a: u8 = 300; emit!(\"r\", a = a);");
    assert_eq!(too_large.first("E0909").message, "`300` is out of range: an integer literal beyond the range of `u8`");
    let partial = ValueFile::refused("language~03460845", "", "let x = 3u8; let y = match x { 0 => 1.0, 1 => 2.0 }; emit!(\"r\", y = y);");
    assert!(partial.first("P0002").message.contains("a `match` that misses some"), "the arms cover every value: {}", partial.first("P0002").message);
}

#[test]
fn blocks_ifs_matches_and_patterns_are_expressions_with_one_value() {
    // language#7 (run, `flow.msl`) with its listing language#9; language~fb958b67 "let names a value. let x =
    // expr; evaluates the expression and gives the result a name, visible from that statement to the end of
    // the block" (the second `level` hides the first; a name of an inner block ends with it); language~b76ed779
    // "So { let s = x * 10.0; s + 1.0 } is a number"
    let record = ValueFile::only(Chapter::read().marked("flow.msl", "language#7").run(ValueFile::transient()), "flow");
    for (name, printed) in [("level", "3.600000e0"), ("step", "1.000000e0"), ("top", "3.300000e0"), ("margin", "-3.000000e-1")] {
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
fn constants_an_enum_and_functions_with_an_optional_parameter() {
    // language#10 (run, `items.msl`) with its listing language#12; language~a52c9d86 "Running today: bool,
    // integers of up to 64 bits, f64, enums without fields that derive Repr, Option, constants, functions with
    // optional parameters, let and blocks, if, match and if let, and a tuple pattern in a let" (this file and
    // the three before it are those forms, and each runs)
    let record = ValueFile::only(Chapter::read().marked("items.msl", "language#10").run(ValueFile::transient()), "items");
    ValueFile::flag(&record, "high", true);
    ValueFile::int(&record, "weight", 2);
    for (name, printed) in [("plain", "4.900000e-1"), ("scaled", "4.900000e2"), ("named", "4.900000e2")] {
        Printed::holds(printed, Fields::real(&record, name), name);
    }
    // "Items may be written in any order: a function may call one declared further down, and a constant may
    // use a later constant"; 64-bit integers wrap at 64 bits
    let ordered = ValueFile::written("const H: f64 = 2.0 * V;\nconst V: f64 = 3.3;\nfn first() -> f64 { second() }\nfn second() -> f64 { H }", "let big: u64 = 18446744073709551615; emit!(\"r\", h = first(), big = big + 2);");
    Printed::holds("6.600000e0", Fields::real(&ordered, "h"), "a constant computed from a later one, through two functions");
    ValueFile::int(&ordered, "big", 1);
}

#[test]
fn a_fault_ends_the_analysis_or_the_build_and_a_non_finite_value_ends_the_solve() {
    // "Dividing an integer by zero is a fault: the analysis ends there, naming the instance and the time";
    // "A fault in a constant or a param default is reported when the design is built"; language~a3318a63 "A
    // value that is not finite and arrives at the analog solver ends the solve with an error, because no
    // circuit has an infinite voltage"
    let divided = Model::written("m.msl", "module Top() {}\ndigital for Top { on at(1u) { let a = 7u8; let b = 0u8; emit!(\"r\", q = a / b); } }\n").run(ValueFile::transient()).err().expect("an integer division by zero is a fault");
    let said = format!("{divided:#}");
    assert!(said.contains("fault 0 in `Top` at tick 1000000000: panic: integer division by zero"), "{said}");
    let unwrapped = Model::written("m.msl", "module Top() {}\ndigital for Top { on at(1u) { let o: Option<f64> = None; emit!(\"r\", v = o.unwrap()); } }\n").run(ValueFile::transient()).err().expect("`unwrap` on `None` is a fault");
    assert!(format!("{unwrapped:#}").contains("panic: called `Option::unwrap()` on a `None` value"), "{unwrapped:#}");
    let constant = ValueFile::refused("language~a3318a63", "const D: u8 = 1 / 0;", "emit!(\"r\", d = D);");
    assert_eq!(constant.first("E0104").message, "a constant expression panics while it is evaluated at elaboration: an integer division by zero");
    let source = "module R(p: Electrical, n: Electrical) { r: f64 = 1k; }\nanalog for R { i(p, n) +> v(p, n) / r; }\nmodule Bad(p: Electrical, n: Electrical) { z: f64 = 0.0; }\nanalog for Bad { v(p, n) +> 1.0 / z; }\nmodule Top() { signal a: Electrical; let s = Bad(a, gnd); let r = R(a, gnd); }\n";
    let solved = Model::written("m.msl", source).run(Faulty::operating_point_of("Top")).err().expect("a contribution that is not finite ends the solve");
    let said = format!("{solved:#}");
    assert!(said.contains("E0507: instance `Top.s`") && said.contains("is not finite"), "{said}");
}

#[test]
fn the_forms_still_on_the_way_are_refused_where_the_solver_reaches_them() {
    // language#20 (refused): the chapter's `range_of` and `mean`. A function nothing reaches is accepted
    // (GAPS.md S3); one the top reaches is `P0001`: the enum has no `#[derive(Repr)]`, a `match` arm holds a
    // range pattern (L6), `&[f64]` is a reference type (L3). When one of them runs this test fails and the
    // row moves to `run`
    let chapter = Chapter::read();
    let reaching = |call: &str| chapter.wrapped("carry.msl", "", "language#20", &format!("module Top() {{ }}\ndigital for Top {{ on at(1u) {{ {call} }} }}\n"));
    let overrides = || Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let range = Refusal::of(reaching("let z = range_of(3);").run(overrides()), "language#20");
    assert!(range.contains(&("P0001".to_string(), "range pattern in a `match` arm is not supported yet".to_string())), "{range:?}");
    assert!(range.contains(&("P0001".to_string(), "enum `Range` without `#[derive(Repr)]` is not supported yet".to_string())), "{range:?}");
    Refusal::expect(reaching("let z = mean(&[1.0, 2.0]);").run(overrides()), "language#20", "P0001", "reference type is not supported yet");
}

#[test]
fn a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it() {
    // language~5628120b "A model that uses one of them is stopped before the run with the code P0001 and a
    // message that names the form, so nothing is ever computed by a rule other than the ones above": the forms
    // of the status box, each on the smallest model (when one of them runs, this test fails and the box loses it)
    for (items, statements, named) in [
        ("struct P { x: f64 }", "let p = P { x: 1.0 }; emit!(\"r\", x = p.x);", "struct expression is not supported yet"),
        ("", "let a = [1.0, 2.0]; emit!(\"r\", x = a[0]);", "array expression is not supported yet"),
        ("", "let mut x = 5; x = 6; emit!(\"r\", x = x);", "`let mut` binding is not supported yet"),
        ("", "let x = 5; x = 6; emit!(\"r\", x = x);", "assignment is not supported yet"),
        ("", "let mut s = 0; for i in 0..4 { s += i; } emit!(\"r\", s = s);", "`for` loop is not supported yet"),
        ("", "let x = 3u8; let y = match x { 0..=9 => 1, _ => 2 }; emit!(\"r\", y = y);", "range pattern in a `match` arm is not supported yet"),
        ("", "let p = 7.5 % 2.0; emit!(\"r\", p = p);", "operator `%` is not supported yet"),
        ("#[derive(Clone, Copy, PartialEq, Repr)] enum Z { A, B }\nimpl Z { fn next(self) -> Z { match self { Z::A => Z::B, Z::B => Z::A } } }", "emit!(\"r\", y = Z::B.next() == Z::A);", "method `next` on `enum `Z`` is not supported yet"),
        ("fn same<T: Copy>(x: T) -> T { x }", "emit!(\"r\", y = same(1.0));", "generic function is not supported yet"),
        ("", "let x: f32 = 1.5; emit!(\"r\", x = x);", "type `f32` is not supported yet"),
    ] {
        let faulty = ValueFile::refused("language~5628120b", items, statements);
        assert!(faulty.given.iter().any(|reported| reported.code == "P0001" && reported.message == named), "{statements}: {:?}", faulty.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
    }
}

#[test]
fn blinky_counts_three_rising_edges_in_three_microseconds() {
    // language#13 (the blinky of the book, `blinky.msl`); language~82576d99 "A let in a module body
    // places hardware. let ctr = Counter(clk, count); is an instance named ctr"; the records of
    // language#14 and language#16 (language~4938c15f "The first three lines after the header are the
    // three records of the table above" is the binary's, held with the listing)
    let ran = Blinky::model().run(Blinky::transient(|_| {})).unwrap_or_else(|failure| panic!("blinky fails: {failure}"));
    let edges = ran.stream("edge");
    assert_eq!(edges.len(), 3, "{:?}", ran.records);
    for (record, (instant, count)) in edges.iter().zip([("5.000000e-7", 0), ("1.500000e-6", 1), ("2.500000e-6", 2)]) {
        assert_eq!(record.instance, "Blinky.ctr", "the instance is named ctr, under the top");
        Printed::holds(instant, record.time, "the instant of the edge");
        assert_eq!(record.fields, [("t".to_string(), record.fields[0].1.clone()), ("count".to_string(), Field::Int(count))], "count shows the value before the edge");
        assert!(matches!(record.fields[0].1, Field::Real(t) if (t - record.time).abs() < 1e-12), "t is the instant: {:?}", record.fields);
    }
    let table = &ran.table;
    assert_eq!(table.columns, ["time", "Blinky.clk", "Blinky.count"]);
    assert_eq!(table.rows.len(), 2, "a row for the start and one for the stop");
    assert_eq!(table.rows[1][2], piperine::Cell::Int(3), "three rising edges by 3 us");
    let printed = Blinky::transient(|overrides| {
        overrides.print_step = Some(1e-6);
        overrides.print = vec!["count".to_string()];
    });
    let every_microsecond = Blinky::model().run(printed).unwrap_or_else(|failure| panic!("blinky fails: {failure}"));
    let counts: Vec<piperine::Cell> = every_microsecond.table.rows.iter().map(|row| row[1].clone()).collect();
    assert_eq!(counts, [piperine::Cell::Int(0), piperine::Cell::Int(1), piperine::Cell::Int(2), piperine::Cell::Int(3)], "--print-step: a row every microsecond");
}

#[test]
fn a_param_has_a_default_a_check_and_a_binding_but_no_assignment() {
    // language~a0e60609 "A param is declared like a field. f: f64 = 1M where f > 0.0; has a default
    // and a check"; language~9703ccd2 "There is no assignment to it anywhere: the placement Osc(clk) {
    // f: 1M } binds it by name"
    let overrides = || Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(1e-6), ..Overrides::default() };
    let probe = "digital for Probe { on clk.rise() { emit!(\"rise\", t = now()); } }\nmodule Top() { signal clk: Bit; let osc = Osc(clk) { f: 4M }; let probe = Probe(clk); }\n";
    let rises = |placement: &str| {
        let source = format!("{OSCILLATOR}{}", probe.replace("{ f: 4M }", placement));
        Model::written("param.msl", &source).run(overrides()).unwrap_or_else(|failure| panic!("{failure}")).records.iter().filter(|record| record.stream == "rise").map(|record| record.time).collect::<Vec<f64>>()
    };
    Printed::holds("5.000000e-7", rises("")[0], "the default f = 1M rises at half a period");
    Printed::holds("1.250000e-7", rises("{ f: 4M }")[0], "the binding f: 4M rises at 0.125 us");
    let checked = Faulty::elaborating("language~a0e60609", &format!("{OSCILLATOR}module Top() {{ signal clk: Bit; let osc = Osc(clk) {{ f: 0.0 }}; }}\n"), "Top");
    assert_eq!(checked.first("E0317").message, "the `where` of the param `Top.osc.f` is false for the value 0.0");
    let assigned = Faulty::elaborating("language~9703ccd2", "module Osc(out clk: Bit = false) { f: f64 = 1M where f > 0.0; f = 2.0; }\nmodule Top() { signal clk: Bit; let o = Osc(clk); }\n", "Top");
    assert_eq!(assigned.first("E0222").message, "the target of a definition is a param, which this module may not drive (§8.2.10)");
}

#[test]
fn after_schedules_the_new_value_a_delay_later_and_each_toggle_fires_the_reaction_again() {
    // language~8414e064 "With after d the new value arrives d seconds later: the oscillator toggles
    // itself every half period, and each toggle is the change that fires the reaction again"; the
    // chapter's next paragraph: the first value is a change at time zero, the clock rises at 0.5 us
    let source = format!("{OSCILLATOR}module Top() {{ signal clk: Bit; let osc = Osc(clk) {{ f: 1M }}; let probe = Probe(clk); }}\n");
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2.2e-6), ..Overrides::default() };
    let ran = Model::written("toggle.msl", &source).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let toggles: Vec<(f64, Field)> = ran.stream("toggle").iter().map(|record| (record.time, record.fields[1].1.clone())).collect();
    let instants: Vec<&str> = vec!["0.000000e0", "5.000000e-7", "1.000000e-6", "1.500000e-6", "2.000000e-6"];
    assert_eq!(toggles.len(), instants.len(), "a toggle at time zero and one every half period: {toggles:?}");
    for ((time, _), printed) in toggles.iter().zip(&instants) {
        Printed::holds(printed, *time, "the instant of a toggle");
    }
    let levels: Vec<&Field> = toggles.iter().map(|(_, level)| level).collect();
    assert_eq!(levels, [&Field::Bool(false), &Field::Bool(true), &Field::Bool(false), &Field::Bool(true), &Field::Bool(false)], "the clock starts low and rises first at 0.5 us");
}

#[test]
fn a_message_with_values_is_a_record_with_a_text_field() {
    // language#19 (run): "A message with values in it is simply a record with a text field":
    // `emit!("log", msg = "overrange", vin = v(inp));` writes `msg` as text and `vin` as the node's voltage
    let chapter = Chapter::read();
    let before = "module Source(p: Electrical, n: Electrical) { dc: f64 = 3.0; }\nanalog for Source { v(p, n) +> dc; }\nmodule Top() {\n    signal inp: Electrical;\n    let s = Source(inp, gnd);\n}\ndigital for Top { on at(1u) {\n";
    let model = chapter.wrapped("log.msl", before, "language#19", "} }\n");
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let ran = model.run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let logs = ran.stream("log");
    assert_eq!(logs.len(), 1, "{:?}", ran.records);
    assert_eq!(logs[0].fields[0], ("msg".to_string(), Field::Text("overrange".to_string())));
    assert!(matches!(&logs[0].fields[1], (name, Field::Real(vin)) if name == "vin" && (vin - 3.0).abs() < 1e-9), "{:?}", logs[0].fields);
}

#[test]
fn only_two_built_in_bang_forms_exist() {
    // language~95095180: "macro_rules!, println!, format!: Only two built-in ! forms: emit! for
    // output and include_str! for data (E0901)": `println!` and `format!` are `E0901`, `macro_rules!` too
    for source in ["fn f() { println!(\"x\"); }\nmodule Top() {}\n", "fn f() { let s = format!(\"x\"); }\nmodule Top() {}\n", "macro_rules! m { () => {} }\nmodule Top() {}\n"] {
        let faulty = Faulty::elaborating("language~95095180", source, "Top");
        assert_eq!(faulty.first("E0901").message, "macros are not part of the language", "{source}");
    }
}

/// The counter of the Book's blinky with its reaction's body replaced by `body`, on a top that
/// places it: the faulty models of the chapter's table.
struct CounterWith;

impl CounterWith {
    /// The model whose counter reacts to `clk.rise()` with `body`.
    fn source(body: &str) -> String {
        format!("module Counter(in clk: Bit, out count: Bit<u8> = 0) {{}}\ndigital for Counter {{ on clk.rise() {{ {body} }} }}\nmodule Top() {{ signal clk: Bit; signal count: Bit<u8>; let c = Counter(clk, count); }}\n")
    }
}

#[test]
fn e0901_there_is_no_println() {
    // language~80317c1d: "E0901 println!(\"count = {}\", count);": not a built-in form
    let faulty = Faulty::elaborating("language~80317c1d", &CounterWith::source("println!(\"count = {}\", count);"), "Top");
    assert_eq!(faulty.pointed_at("E0901"), "println!(\"count = {}\", count)");
}

#[test]
fn e0602_an_emit_has_no_format_string() {
    // language~64ee8a1d: "E0602 emit!(\"edge {}\", count);": there is no format string, and a field needs a name
    let faulty = Faulty::elaborating("language~64ee8a1d", &CounterWith::source("emit!(\"edge {}\", count);"), "Top");
    assert_eq!(faulty.pointed_at("E0602"), "count");
    assert_eq!(faulty.first("E0602").message, "a field is not `name = value` in `emit!`");
}

#[test]
fn e0604_a_field_name_is_used_twice() {
    // language~64ee881d: "E0604 emit!(\"edge\", t = now(), t = count);": a field name is used twice
    let faulty = Faulty::elaborating("language~64ee881d", &CounterWith::source("emit!(\"edge\", t = now(), t = count);"), "Top");
    assert_eq!(faulty.first("E0604").message, "the field `t` is written twice in one `emit!` (§19 rule 11)");
}

#[test]
fn e0601_an_emit_in_a_module_body_is_misplaced_output() {
    // language~64ee8d1d: "E0601 emit!(\"hello\"); in a module body: Output runs when behavior runs. Put it
    // in a reaction": the statement is `E0601`, the same code a module-body `let` that holds an `emit!` gets
    let faulty = Faulty::elaborating("language~64ee8d1d", "module Top() { emit!(\"hello\"); }\n", "Top");
    assert_eq!(faulty.given.iter().map(|reported| reported.code.as_str()).collect::<Vec<_>>(), ["E0601"]);
    assert_eq!(faulty.first("E0601").message, "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)");
    assert!(faulty.first("E0601").help.as_deref().is_some_and(|help| help.contains("put it in a reaction")), "the Book's fix is a help line");
    let held = Faulty::elaborating("language~64ee8d1d", "module Top() { let x = emit!(\"hello\"); }\n", "Top");
    assert_eq!(held.first("E0601").message, "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)");
}

#[test]
fn e0101_there_are_no_mutable_references() {
    // language~399f441d: "E0101 fn bump(c: &mut u8) { *c += 1; }": no mutable references
    let faulty = Faulty::elaborating("language~399f441d", "fn bump(c: &mut u8) { *c += 1; }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0101").message, "a mutable reference is not part of the language");
}

#[test]
fn e0143_there_are_no_default_arguments() {
    // language~39acbe1d: "E0143 fn gain(x: f64, k: f64 = 2.0) -> f64": no default arguments
    let faulty = Faulty::elaborating("language~39acbe1d", "fn gain(x: f64, k: f64 = 2.0) -> f64 { x * k }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.pointed_at("E0143"), "= 2.0");
}

#[test]
fn e0161_there_are_no_heap_types() {
    // language~39a5d01d: "E0161 let xs: Vec<f64> = Vec::new();": no heap types, so the path does not resolve
    let source = "module Top() { }\ndigital for Top { on at(1u) { let xs: Vec<f64> = Vec::new(); } }\n";
    let faulty = Faulty::elaborating("language~39a5d01d", source, "Top");
    assert_eq!(faulty.first("E0161").message, "cannot find `Vec` in this scope");
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
