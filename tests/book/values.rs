//! Scope: chapter 5 of the Book, "Values in hardware" (feature `root-e2e`, RE-02 and RE-03): the blinky
//! model states what the chapter says of it (its three records, its instance path, its parameter, its
//! delayed commit), `emit!` with a text field runs, a function has the class its body gives, the window
//! monitor and the adder that reads an undriven input run, the sample-and-hold reads a digital carrier
//! directly, and the mistakes of the chapter's table get the codes the chapter names, each run
//! through `piperine::`. Each test opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

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
use support::model::{Model, Ran};
use support::refusal::Refusal;

/// A number as the Book prints it: the printed digits are the resolution of the comparison.
struct Printed;

impl Printed {
    /// Asserts that `x` rounds to the number the Book prints as `printed` (`1.052399e-4`).
    fn holds(printed: &str, x: f64, what: &str) {
        let number = PrintedNumber::parse(printed).unwrap_or_else(|| panic!("`{printed}` is a printed number"));
        assert!(number.rounds_from(x), "{what}: the Book prints {printed}, the run gives {x:e}");
    }
}

/// A transient to `stop` of the top `top`, the run the chapter's commands make.
struct Transient;

impl Transient {
    /// The overrides of `piperine run <file> --top <top> --analysis transient --stop <stop>`.
    fn of(top: &str, stop: f64) -> Overrides {
        Overrides { analysis: Some("transient".to_string()), top: Some(top.to_string()), stop: Some(stop), ..Overrides::default() }
    }

    /// The run of `model`, which must succeed.
    fn ran(model: &Model, top: &str, stop: f64) -> Ran {
        model.run(Transient::of(top, stop)).unwrap_or_else(|failure| panic!("the run fails: {failure}"))
    }
}

/// The fields of a record by name.
struct Fields;

impl Fields {
    /// The value of the field `name` of `record`. Panics naming the field when there is none.
    fn of<'a>(record: &'a Record, name: &str) -> &'a Field {
        &record.fields.iter().find(|(field, _)| field == name).unwrap_or_else(|| panic!("no field `{name}` in {:?}", record.fields)).1
    }
}

/// A reaction at 1 us whose body is `body`, on a top with no ports.
struct Reaction;

impl Reaction {
    /// The text before a block that is the body of the reaction.
    const BEFORE: &'static str = "module Top() { }\ndigital for Top { on at(1u) {\n";

    /// The text of a reaction whose body is `body`.
    fn source(body: &str) -> String {
        format!("{}{body}\n}} }}\n", Reaction::BEFORE)
    }
}

/// The Book's blinky, in the file the Book names `blinky.msl`.
struct Blinky;

impl Blinky {
    /// The model: the blocks the Book marks `blinky.msl`.
    fn model() -> Model {
        Chapter::read().marked("blinky.msl", "values#1")
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
fn blinky_counts_three_rising_edges_in_three_microseconds() {
    // values#1 (the blinky of the book, `blinky.msl`); values~82576d99 "A let in a module body
    // places hardware. let ctr = Counter(clk, count); is an instance named ctr"; the records of
    // values#2 and values#4 (values~4938c15f "The first three lines after the header are the
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
    // values~a0e60609 "A param is declared like a field. f: f64 = 1M where f > 0.0; has a default
    // and a check"; values~9703ccd2 "There is no assignment to it anywhere: the placement Osc(clk) {
    // f: 1M } binds it by name"
    let overrides = || Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(1e-6), ..Overrides::default() };
    let probe = "digital for Probe { on clk.rise() { emit!(\"rise\", t = now()); } }\nmodule Top() { signal clk: Bit; let osc = Osc(clk) { f: 4M }; let probe = Probe(clk); }\n";
    let rises = |placement: &str| {
        let source = format!("{OSCILLATOR}{}", probe.replace("{ f: 4M }", placement));
        Model::written("param.msl", &source).run(overrides()).unwrap_or_else(|failure| panic!("{failure}")).records.iter().filter(|record| record.stream == "rise").map(|record| record.time).collect::<Vec<f64>>()
    };
    Printed::holds("5.000000e-7", rises("")[0], "the default f = 1M rises at half a period");
    Printed::holds("1.250000e-7", rises("{ f: 4M }")[0], "the binding f: 4M rises at 0.125 us");
    let checked = Faulty::elaborating("values~a0e60609", &format!("{OSCILLATOR}module Top() {{ signal clk: Bit; let osc = Osc(clk) {{ f: 0.0 }}; }}\n"), "Top");
    assert_eq!(checked.first("E0317").message, "the `where` of the param `Top.osc.f` is false for the value 0.0");
    let assigned = Faulty::elaborating("values~9703ccd2", "module Osc(out clk: Bit = false) { f: f64 = 1M where f > 0.0; f = 2.0; }\nmodule Top() { signal clk: Bit; let o = Osc(clk); }\n", "Top");
    assert_eq!(assigned.first("E0222").message, "the target of a definition is a param, which this module may not drive (§8.2.10)");
}

#[test]
fn after_schedules_the_new_value_a_delay_later_and_each_toggle_fires_the_reaction_again() {
    // values~8414e064 "With after d the new value arrives d seconds later: the oscillator toggles
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
    // values#7 (run): "A message with values in it is simply a record with a text field":
    // `emit!("log", msg = "overrange", vin = v(inp));` writes `msg` as text and `vin` as the node's voltage
    let chapter = Chapter::read();
    let before = "module Source(p: Electrical, n: Electrical) { dc: f64 = 3.0; }\nanalog for Source { v(p, n) +> dc; }\nmodule Top() {\n    signal inp: Electrical;\n    let s = Source(inp, gnd);\n}\ndigital for Top { on at(1u) {\n";
    let model = chapter.wrapped("log.msl", before, "values#7", "} }\n");
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let ran = model.run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let logs = ran.stream("log");
    assert_eq!(logs.len(), 1, "{:?}", ran.records);
    assert_eq!(logs[0].fields[0], ("msg".to_string(), Field::Text("overrange".to_string())));
    assert!(matches!(&logs[0].fields[1], (name, Field::Real(vin)) if name == "vin" && (vin - 3.0).abs() < 1e-9), "{:?}", logs[0].fields);
}

#[test]
fn e0901_there_is_no_println() {
    // values~80317c1d: "E0901 println!(\"count = {}\", count);": not a built-in form
    let faulty = Faulty::elaborating("values~80317c1d", &CounterWith::source("println!(\"count = {}\", count);"), "Top");
    assert_eq!(faulty.pointed_at("E0901"), "println!(\"count = {}\", count)");
}

#[test]
fn e0602_an_emit_has_no_format_string() {
    // values~64ee8a1d: "E0602 emit!(\"edge {}\", count);": there is no format string, and a field needs a name
    let faulty = Faulty::elaborating("values~64ee8a1d", &CounterWith::source("emit!(\"edge {}\", count);"), "Top");
    assert_eq!(faulty.pointed_at("E0602"), "count");
    assert_eq!(faulty.first("E0602").message, "a field is not `name = value` in `emit!`");
}

#[test]
fn e0604_a_field_name_is_used_twice() {
    // values~64ee881d: "E0604 emit!(\"edge\", t = now(), t = count);": a field name is used twice
    let faulty = Faulty::elaborating("values~64ee881d", &CounterWith::source("emit!(\"edge\", t = now(), t = count);"), "Top");
    assert_eq!(faulty.first("E0604").message, "the field `t` is written twice in one `emit!` (§19 rule 11)");
}

#[test]
fn e0601_an_emit_in_a_module_body_is_misplaced_output() {
    // values~64ee8d1d: "E0601 emit!(\"hello\"); in a module body: Output runs when behavior runs. Put it
    // in a reaction": the statement is `E0601`, the same code a module-body `let` that holds an `emit!` gets
    let faulty = Faulty::elaborating("values~64ee8d1d", "module Top() { emit!(\"hello\"); }\n", "Top");
    assert_eq!(faulty.given.iter().map(|reported| reported.code.as_str()).collect::<Vec<_>>(), ["E0601"]);
    assert_eq!(faulty.first("E0601").message, "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)");
    assert!(faulty.first("E0601").help.as_deref().is_some_and(|help| help.contains("put it in a reaction")), "the Book's fix is a help line");
    let held = Faulty::elaborating("values~64ee8d1d", "module Top() { let x = emit!(\"hello\"); }\n", "Top");
    assert_eq!(held.first("E0601").message, "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)");
}

#[test]
fn a_function_has_the_class_its_body_gives() {
    // values#8 (refused): `diode_current` is pure and `handshake` returns an `Event`; values~c389077a
    // (refused): "The compiler infers a class from the body: a pure function, like diode_current, works
    // anywhere; one that probes a voltage or reads now() works where the solver is running; one that
    // returns Event, like handshake, works where events go; and one that calls emit! works where emit!
    // does." The pure, `now()` and `emit!` classes run; an `Event` function is `P0001` (GAPS.md L53)
    let chapter = Chapter::read();
    let top = Reaction::source("emit!(\"d\", i = diode_current(0.6, 1e-14, 0.026), t = clock(), x = 1);").replace("module Top() { }", "fn clock() -> f64 { now() }\nfn say() { emit!(\"say\", x = 1.0); }\nmodule Top() { }").replace("emit!(\"d\"", "say(); emit!(\"d\"");
    let ran = Transient::ran(&chapter.wrapped("functions.msl", "", "values#8", &top), "Top", 2e-6);
    let d = &ran.stream("d")[0];
    let formula = 1e-14 * ((0.6f64 / 0.026).exp() - 1.0);
    assert!(matches!(Fields::of(d, "i"), Field::Real(i) if (i - formula).abs() < 1e-9 * formula), "diode_current: {:?}", Fields::of(d, "i"));
    assert!(matches!(Fields::of(d, "t"), Field::Real(t) if (t - 1e-6).abs() < 1e-12), "a function that reads now() runs where the solver does: {:?}", Fields::of(d, "t"));
    assert_eq!(ran.stream("say").len(), 1, "a function that calls emit! writes where emit! does");
    let reaching = "module Top(in req: Bit, in ack: Bit) { }\ndigital for Top { on handshake(req, ack) { emit!(\"h\", t = now()); } }\n";
    let refused = chapter.wrapped("handshake.msl", "", "values#8", reaching).run(Transient::of("Top", 2e-6));
    Refusal::expect(refused, "values#8", "P0001", "method `rise` on this receiver is not supported yet");
}

#[test]
fn the_monitor_file_runs_its_window_monitor_and_refuses_its_option_and_logic() {
    // values#10 (run: the window monitor of monitor.msl), values#9 is the enum of values#10; values~d0e86096
    // "This file watches a node and reports, on every clock edge, whether its voltage is under, inside or
    // over a window"; values~8ad56d03 "The source is a ramp of 1 V/µs, so the node walks through all three
    // zones"; values~6ecca079 "The clock rises at 0.5, 1.5 and 2.5 µs, where the ramp is near 0.5, 1.5 and
    // 2.5 V"; values~3c87d509 "if z != zone { emit!(…) } prints only when the zone changes. zone on the
    // right still reads the old value"; values~4f830aa0 "The command prints an enum field as the name of
    // its variant, so from=Ok to=Under is a move from Ok to Under" (the command's line is held with the
    // listing). The adder and the peek that followed it (values#14 and values#18) are refused, below.
    let chapter = Chapter::read();
    let monitor = chapter.marked("monitor.msl", "values#10").run(Transient::of("MonitorTop", 3e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let zones = monitor.stream("zone");
    assert_eq!(zones.len(), 3, "one record per change of zone: {:?}", monitor.records);
    for (record, (instant, near, from, to)) in zones.iter().zip([("5.000000e-7", 0.5, "Ok", "Under"), ("1.500000e-6", 1.5, "Under", "Ok"), ("2.500000e-6", 2.5, "Ok", "Over")]) {
        assert_eq!(record.instance, "MonitorTop.mon");
        Printed::holds(instant, record.time, "the clock edge");
        assert!(matches!(Fields::of(record, "v"), Field::Real(v) if (v - near).abs() < 0.01), "v is near {near}: {:?}", Fields::of(record, "v"));
        assert_eq!((Fields::of(record, "from"), Fields::of(record, "to")), (&Field::Variant(from.to_string()), &Field::Variant(to.to_string())), "the variants by name");
    }
    let longer = chapter.marked("monitor.msl", "values#10").run(Transient::of("MonitorTop", 5e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(longer.stream("zone").len(), 3, "the edges at 3.5 and 4.5 us find the zone unchanged and print nothing");
    // values#14 and values#18 (refused, GAPS.md S8): the adder whose `b` is a `Bit<Option<u8>>` and the peek
    // over `Bit<Logic<4>>`. The `Option` carrier has a `Repr` since composite-values, and `b` is never driven, so
    // reading it is the run fault that names it (E0105; `explicit-unknowns` phase A starts every carrier at a
    // value); `Logic<4>` is the library struct `Logic`, which has no generic parameter until phase B (P0001). The
    // test fails when either starts to run, and the row then becomes `run`. values~2bbc575e "It is placed in a top that gives a the constant 3 and takes b
    // from an in port of its own, which nothing drives, so b starts at None" and values~c30bad4d "The
    // undriven b starts at xxxx ..." are held by the listings' refusal (S8)
    let adder = chapter.marked("monitor.msl", "values#14").run(Transient::of("AdderTop", 3e-6));
    Refusal::expect(adder, "values#14", "E0105", "a model panicked under known control in `AdderTop.add` at tick 500000000: `b` is a composite that holds no value yet, so it has no field or copy to read; test `b.is_known()` first");
    let peek = chapter.marked("monitor.msl", "values#18").run(Transient::of("PeekTop", 3e-6));
    Refusal::expect(peek, "values#18", "P0001", "generic arguments on `Logic`, which has no generic parameter is not supported yet");
}

#[test]
fn the_values_chapter_teaches_the_explicit_model() {
    // EU-18 AC 1 (replaces `the_flag_is_read_with_known_known_or_and_is_known`, which held values~1aeb5afb
    // on `x.known()`, `x.known_or(d)` and `x.is_known()`): the chapter's sections are binary types and
    // `Logic`, a signal that starts with a value (the `Default` table), absence with `Option`, deciding on a
    // `Logic`, and the analog side reading a value; it uses none of the removed names.
    let book = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/book/content.html")).expect("the Book");
    let start = book.find("<section id=\"values\"").expect("chapter values");
    let chapter = &book[start..start + book[start..].find("</section>").expect("a chapter closes")];
    for heading in ["<h3>What a type may hold</h3>", "<h3>A signal starts with a value</h3>", "<h3>Catching an absent input with <code>emit!</code></h3>", "<h3>Deciding on a <code>Logic</code></h3>", "<h3>Analog code reads values</h3>"] {
        assert!(chapter.contains(heading), "the chapter lacks {heading}");
    }
    for fact in ["no default: the value must be written", "<code>signal en: Bit;</code> is error E0187", "<code>a == b</code> on two <code>Logic</code> values is itself a <code>Logic</code>", "take a <code>bool</code> and nothing else", "<code>same(a, b)</code> is true when every level holds the same one of the four states", "<code>x.to_volts(vdd)</code>"] {
        assert!(chapter.contains(fact), "the chapter does not say {fact}");
    }
    for removed in ["Known and unknown", "Catching an unknown", "Analog code has no unknowns", "known_or", "is_known", ".known()", "<code>Unknown</code>", "level(", ".high()", "maybe", "E0150"] {
        assert!(!chapter.contains(removed), "the chapter still says {removed}");
    }
}

#[test]
fn analog_reads_a_digital_carrier_directly() {
    // values#21 (refused, GAPS.md S8): the sample-and-hold of the Book reads `held` with no gate. The
    // earlier front end still reports E0150 for the read (replaces
    // `analog_reads_a_digital_carrier_through_known_or`, which ran the listing with `held.known_or(0.0)`);
    // the test fails when the read is accepted. values~267f8cfe is now the sentence "held is a real number
    // from the first instant, so the contribution needs no gate"
    let chapter = Chapter::read();
    let after = "module Source(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Source { v(p, n) +> dc; }\nmodule Top() { signal a: Electrical; signal b: Electrical; signal c: Bit = false; let s = Source(a, gnd); let h = SampleHold(a, b, c); }\ndigital for Top { on at(1u) { c <- true; } }\n";
    let model = chapter.wrapped("sample_hold.msl", "", "values#21", after);
    Refusal::expect(model.run(Transient::of("Top", 3e-6)), "values#21", "E0150", "an `analog` block reads the discrete carrier `held` other than through `known`, `known_or`, `is_known` or `high`: it has no unknowns (§3.3.5)");
}

#[test]
fn e0187_is_not_raised_yet() {
    // values~23c03fc8 "So signal en: Bit; is error E0187", the E0187 cell of the table (values~39ba3a1d) and values~37a548f2 (an
    // undriven top-level in port of a type with no default) (all `unraised`, GAPS.md S8): the earlier front end
    // starts a carrier with no value unknown, so `signal en: Bit;` elaborates and runs. The test fails when E0187
    // is raised. (values~13ffaea6, E0180 for a `Logic`, is raised: `e0180_a_logic_is_no_f64_and_no_bool`.)
    let source = "module Top() { signal en: Bit; }\ndigital for Top { on at(1u) { emit!(\"e\", t = now()); } }\n";
    let ran = Transient::ran(&Model::written("no_default.msl", source), "Top", 2e-6);
    assert_eq!(ran.stream("e").len(), 1, "a signal with no value and no default still runs: E0187 is not raised");
    let port = Transient::ran(&Model::written("undriven.msl", "module Top(in b: Bit<u8>) { }\ndigital for Top { on at(1u) { emit!(\"p\", t = now()); } }\n"), "Top", 2e-6);
    assert_eq!(port.stream("p").len(), 1, "an undriven in port of a type with no default still runs");
}

#[test]
fn e0180_a_logic_is_no_f64_and_no_bool() {
    // values~13ffaea6 "Using a Logic where an f64 or a bool is required is error E0180": the library struct `Logic` is
    // a type since composite-values CV-09, so a read of one where a real or a `bool` is wanted is a mismatch
    let real = Faulty::running("values~13ffaea6", "module Top(in l: Bit<Logic>) { signal a: Electrical; }\nanalog for Top { let y: f64 = l; v(a) +> y; }\n", Faulty::operating_point_of("Top"));
    assert_eq!(real.first("E0180").message, "mismatched types: expected `f64`, found `struct `Logic``");
    let flag = Faulty::running("values~13ffaea6", "module Top(in l: Bit<Logic>) { signal a: Electrical; }\nanalog for Top { if l { v(a) +> 1.0; } }\n", Faulty::operating_point_of("Top"));
    assert_eq!(flag.first("E0180").message, "mismatched types: expected `bool`, found `struct `Logic``");
}

#[test]
fn e0112_a_carried_type_needs_repr() {
    // values~399bbd1d: "E0112 out zone: Bit<Zone> with Zone not deriving Repr"
    let faulty = Faulty::elaborating("values~399bbd1d", "enum Zone { Under, Ok, Over }\nmodule M(out zone: Bit<Zone>) {}\nmodule Top() { signal z: Bit<Zone>; let m = M(z); }\n", "Top");
    assert!(faulty.first("E0112").message.contains("has no `Repr`: a carrier holds only a type that implements it"), "{}", faulty.first("E0112").message);
}

#[test]
fn e0350_a_fn_cannot_hold_ddt() {
    // values~4bd60d1d: "E0350 fn slope(x: f64) -> f64 { ddt(x) }": ddt is an instance with memory
    let faulty = Faulty::elaborating("values~4bd60d1d", "fn slope(x: f64) -> f64 { ddt(x) }\nmodule Top() { }\nanalog for Top { let y = slope(1.0); }\n", "Top");
    assert_eq!(faulty.first("E0350").message, "`ddt` is a core module and cannot be placed in a `fn` or a closure");
}

#[test]
fn an_extern_function_is_refused_with_or_without_its_derivative() {
    // values#22 (refused): `#[derivative(erfcx_dx)] extern "C" fn erfcx` and its derivative; values~3998361d
    // (unraised): "E0123 extern \"C\" fn erfcx(x: f64) -> f64; called in analog: Add #[derivative]". No
    // source raises E0123 (GAPS.md section 10): an `extern` function is `P0001` (L12), with the
    // derivative the Book asks for or without. When it runs this test fails and the rows move
    let chapter = Chapter::read();
    let analog = "module Top() { signal p: Electrical; }\nanalog for Top { v(p) +> erfcx(1.0); }\n";
    let overrides = || Overrides { analysis: Some("op".to_string()), top: Some("Top".to_string()), ..Overrides::default() };
    Refusal::expect(chapter.wrapped("foreign.msl", "", "values#22", analog).run(overrides()), "values#22", "P0001", "`extern` function is not supported yet");
    let bare = Refusal::of(Model::written("bare.msl", &format!("extern \"C\" fn erfcx(x: f64) -> f64;\n{analog}")).run(overrides()), "values~3998361d");
    assert!(bare.iter().all(|(code, _)| code != "E0123") && bare.contains(&("P0001".to_string(), "`extern` function is not supported yet".to_string())), "no E0123: {bare:?}");
}
