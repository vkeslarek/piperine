//! Scope: chapter 2 of the Book, "If you know Rust" (feature `root-e2e`, RE-02 and RE-03): the
//! blinky model runs through `piperine::` and states what the chapter says of it (its three records,
//! its instance path, its parameter, its delayed commit), the Rust that "carries over" is asserted
//! refused where the solver reaches it, `emit!` with a text field runs, and each mistake of the
//! chapter's table gets the code the chapter names. Each test opens with the inventory ids it holds,
//! from `tests/book/inventory.tsv`.

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

use piperine::{Field, Overrides};
use piperine_parity::golden::PrintedNumber;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
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
        Chapter::read().marked("blinky.msl", "from-rust#2")
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

#[test]
fn the_rust_that_carries_over_is_refused_where_the_solver_reaches_it() {
    // from-rust#1 (refused): "This is valid MSL and valid Rust at the same time" and the chapter's first
    // example. A function nothing reaches is accepted (GAPS.md S3); one the top reaches is `P0001`: the
    // enum has no `#[derive(Repr)]`, a `match` arm holds a range pattern (L6), `&[f64]` is a reference
    // type (L3). When one of them runs this test fails and the row moves to `run`
    let chapter = Chapter::read();
    let reaching = |call: &str| chapter.wrapped("carry.msl", "", "from-rust#1", &format!("module Top() {{ }}\ndigital for Top {{ on at(1u) {{ {call} }} }}\n"));
    let overrides = || Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let range = Refusal::of(reaching("let z = range_of(3);").run(overrides()), "from-rust#1");
    assert!(range.contains(&("P0001".to_string(), "range pattern in a `match` arm is not supported yet".to_string())), "{range:?}");
    assert!(range.contains(&("P0001".to_string(), "enum `Range` without `#[derive(Repr)]` is not supported yet".to_string())), "{range:?}");
    Refusal::expect(reaching("let z = mean(&[1.0, 2.0]);").run(overrides()), "from-rust#1", "P0001", "reference type is not supported yet");
}

#[test]
fn blinky_counts_three_rising_edges_in_three_microseconds() {
    // from-rust#2 (the blinky of the book, `blinky.msl`); from-rust~82576d99 "A let in a module body
    // places hardware. let ctr = Counter(clk, count); is an instance named ctr"; the records of
    // from-rust#3 and from-rust#5 (from-rust~4938c15f "The first three lines after the header are the
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
    // from-rust~065a9173 "A param looks like a struct field. f: f64 = 1M where f > 0.0; has a default
    // and a check"; from-rust~272b0be7 "There is no assignment to it anywhere: the placement Osc(clk) {
    // f: 1M } binds it, like a struct literal"
    let overrides = || Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(1e-6), ..Overrides::default() };
    let probe = "digital for Probe { on clk.rise() { emit!(\"rise\", t = now()); } }\nmodule Top() { signal clk: Bit; let osc = Osc(clk) { f: 4M }; let probe = Probe(clk); }\n";
    let rises = |placement: &str| {
        let source = format!("{OSCILLATOR}{}", probe.replace("{ f: 4M }", placement));
        Model::written("param.msl", &source).run(overrides()).unwrap_or_else(|failure| panic!("{failure}")).records.iter().filter(|record| record.stream == "rise").map(|record| record.time).collect::<Vec<f64>>()
    };
    Printed::holds("5.000000e-7", rises("")[0], "the default f = 1M rises at half a period");
    Printed::holds("1.250000e-7", rises("{ f: 4M }")[0], "the binding f: 4M rises at 0.125 us");
    let checked = Faulty::elaborating("from-rust~065a9173", &format!("{OSCILLATOR}module Top() {{ signal clk: Bit; let osc = Osc(clk) {{ f: 0.0 }}; }}\n"), "Top");
    assert_eq!(checked.first("E0317").message, "the `where` of the param `Top.osc.f` is false for the value 0.0");
    let assigned = Faulty::elaborating("from-rust~272b0be7", "module Osc(out clk: Bit = false) { f: f64 = 1M where f > 0.0; f = 2.0; }\nmodule Top() { signal clk: Bit; let o = Osc(clk); }\n", "Top");
    assert_eq!(assigned.first("E0222").message, "the target of a definition is a param, which this module may not drive (§8.2.10)");
}

#[test]
fn after_schedules_the_new_value_a_delay_later_and_each_toggle_fires_the_reaction_again() {
    // from-rust~8414e064 "With after d the new value arrives d seconds later: the oscillator toggles
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
    // from-rust#8 (run): "A message with values in it is simply a record with a text field":
    // `emit!("log", msg = "overrange", vin = v(inp));` writes `msg` as text and `vin` as the node's voltage
    let chapter = Chapter::read();
    let before = "module Source(p: Electrical, n: Electrical) { dc: f64 = 3.0; }\nanalog for Source { v(p, n) +> dc; }\nmodule Top() {\n    signal inp: Electrical;\n    let s = Source(inp, gnd);\n}\ndigital for Top { on at(1u) {\n";
    let model = chapter.wrapped("log.msl", before, "from-rust#8", "} }\n");
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let ran = model.run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let logs = ran.stream("log");
    assert_eq!(logs.len(), 1, "{:?}", ran.records);
    assert_eq!(logs[0].fields[0], ("msg".to_string(), Field::Text("overrange".to_string())));
    assert!(matches!(&logs[0].fields[1], (name, Field::Real(vin)) if name == "vin" && (vin - 3.0).abs() < 1e-9), "{:?}", logs[0].fields);
}

#[test]
fn only_two_built_in_bang_forms_exist() {
    // from-rust~95095180: "macro_rules!, println!, format!: Only two built-in ! forms: emit! for
    // output and include_str! for data (E0901)": `println!` and `format!` are `E0901`, `macro_rules!` too
    for source in ["fn f() { println!(\"x\"); }\nmodule Top() {}\n", "fn f() { let s = format!(\"x\"); }\nmodule Top() {}\n", "macro_rules! m { () => {} }\nmodule Top() {}\n"] {
        let faulty = Faulty::elaborating("from-rust~95095180", source, "Top");
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
    // from-rust~80317c1d: "E0901 println!(\"count = {}\", count);": not a built-in form
    let faulty = Faulty::elaborating("from-rust~80317c1d", &CounterWith::source("println!(\"count = {}\", count);"), "Top");
    assert_eq!(faulty.pointed_at("E0901"), "println!(\"count = {}\", count)");
}

#[test]
fn e0602_an_emit_has_no_format_string() {
    // from-rust~64ee8a1d: "E0602 emit!(\"edge {}\", count);": there is no format string, and a field needs a name
    let faulty = Faulty::elaborating("from-rust~64ee8a1d", &CounterWith::source("emit!(\"edge {}\", count);"), "Top");
    assert_eq!(faulty.pointed_at("E0602"), "count");
    assert_eq!(faulty.first("E0602").message, "a field is not `name = value` in `emit!`");
}

#[test]
fn e0604_a_field_name_is_used_twice() {
    // from-rust~64ee881d: "E0604 emit!(\"edge\", t = now(), t = count);": a field name is used twice
    let faulty = Faulty::elaborating("from-rust~64ee881d", &CounterWith::source("emit!(\"edge\", t = now(), t = count);"), "Top");
    assert_eq!(faulty.first("E0604").message, "the field `t` is written twice in one `emit!` (§19 rule 11)");
}

#[test]
fn e0601_an_emit_in_a_module_body_is_misplaced_output() {
    // from-rust~64ee8d1d: "E0601 emit!(\"hello\"); in a module body: Output runs when behavior runs. Put it
    // in a reaction": the statement is `E0601`, the same code a module-body `let` that holds an `emit!` gets
    let faulty = Faulty::elaborating("from-rust~64ee8d1d", "module Top() { emit!(\"hello\"); }\n", "Top");
    assert_eq!(faulty.given.iter().map(|reported| reported.code.as_str()).collect::<Vec<_>>(), ["E0601"]);
    assert_eq!(faulty.first("E0601").message, "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)");
    assert!(faulty.first("E0601").help.as_deref().is_some_and(|help| help.contains("put it in a reaction")), "the Book's fix is a help line");
    let held = Faulty::elaborating("from-rust~64ee8d1d", "module Top() { let x = emit!(\"hello\"); }\n", "Top");
    assert_eq!(held.first("E0601").message, "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)");
}

#[test]
fn e0101_there_are_no_mutable_references() {
    // from-rust~399f441d: "E0101 fn bump(c: &mut u8) { *c += 1; }": no mutable references
    let faulty = Faulty::elaborating("from-rust~399f441d", "fn bump(c: &mut u8) { *c += 1; }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0101").message, "a mutable reference is not part of the language");
}

#[test]
fn e0143_there_are_no_default_arguments() {
    // from-rust~39acbe1d: "E0143 fn gain(x: f64, k: f64 = 2.0) -> f64": no default arguments
    let faulty = Faulty::elaborating("from-rust~39acbe1d", "fn gain(x: f64, k: f64 = 2.0) -> f64 { x * k }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.pointed_at("E0143"), "= 2.0");
}

#[test]
fn e0161_there_are_no_heap_types() {
    // from-rust~39a5d01d: "E0161 let xs: Vec<f64> = Vec::new();": no heap types, so the path does not resolve
    let source = "module Top() { }\ndigital for Top { on at(1u) { let xs: Vec<f64> = Vec::new(); } }\n";
    let faulty = Faulty::elaborating("from-rust~39a5d01d", source, "Top");
    assert_eq!(faulty.first("E0161").message, "cannot find `Vec` in this scope");
}
