//! Scope: chapter 1 of the Book, "Welcome" (feature `root-e2e`, RE-02 and RE-03): the resistor, the
//! RC project `piperine new rc` makes, the monitor, the delayed bench and the inverter run through
//! `piperine::` (`Project`, `Session`), and each value the chapter states about them (the 63 % and
//! 86 % of the charge, the crossing at 693.1 ns against 693.147 ns, the operating point's 1 V) is
//! asserted at the precision the Book prints it. Each test opens with the inventory ids it holds, from
//! `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod inventory;
    pub mod model;
    pub mod records;
    pub mod refusal;
}

use piperine::{Cell, Field, Overrides};
use piperine_parity::golden::PrintedNumber;
use support::chapter::Chapter;
use support::model::{Model, Ran};
use support::refusal::{Refusal, Reported};

/// A number as the Book prints it: the printed digits are the resolution of the comparison.
struct Printed;

impl Printed {
    /// Asserts that `x` rounds to the number the Book prints as `printed` (`6.320928e-1`); `what` says
    /// what the number is.
    fn holds(printed: &str, x: f64, what: &str) {
        let number = PrintedNumber::parse(printed).unwrap_or_else(|| panic!("`{printed}` is a printed number"));
        assert!(number.rounds_from(x), "{what}: the Book prints {printed}, the run gives {x:e}");
    }
}

/// The project `piperine new rc` makes, as `examples/rc`.
struct RcProject;

impl RcProject {
    /// The run `name` of the project, which must succeed.
    fn run(name: &str) -> Ran {
        Model::example("rc").of_run(name).run(Overrides::default()).unwrap_or_else(|failure| panic!("the run `{name}` of the rc project fails: {failure}"))
    }
}

#[test]
fn the_rc_project_holds_its_operating_point_and_its_netlist() {
    // welcome#5 and welcome#6 (the library and the bench `piperine new rc` writes, welcome#1 and
    // welcome#12 are pieces of them); welcome~37aa8247 "At rest no current flows, so the output sits
    // at the source's 1 V"; welcome~4ecc9f35 "an operating point holds nothing, which is why op printed
    // 1 V for the same node"; welcome~bc2a836b "Step has no ports ... the braces bind params by name.
    // gnd is the reference node"; welcome~0808ef8f "Bind it: Resistor(vin, vout) { r: 1k }."
    let chapter = Chapter::read();
    let model = Model::example("rc");
    assert_eq!(chapter.text("welcome#5").trim_end(), model.text_of("src/lib.msl").trim_end(), "welcome#5 is src/lib.msl");
    assert_eq!(chapter.text("welcome#6").trim_end(), model.text_of("benches/step.msl").trim_end(), "welcome#6 is benches/step.msl");
    let ran = model.of_run("op").run(Overrides::default()).unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("1.000000e0", ran.signal("Step.vin"), "the source's node");
    Printed::holds("1.000000e0", ran.signal("Step.vout"), "the output at rest");
    assert!(ran.metadata.ports.is_empty(), "Step has no ports: {:?}", ran.metadata.ports);
    let param = |path: &str| ran.metadata.params.iter().find(|param| param.path == path).unwrap_or_else(|| panic!("no param {path}")).value.clone();
    assert_eq!(param("Step.r1.r"), Cell::Real(1000.0), "{{ r: 1k }} binds r by name");
    assert_eq!(param("Step.c1.c"), Cell::Real(1e-9), "{{ c: 1n }} binds c by name");
    assert_eq!(ran.table.rows.len(), 2, "one row per signal of the bench: {:?}", ran.table.rows);
}

#[test]
fn the_rc_step_charges_as_the_book_says() {
    // welcome~937d7801 "The second run is a transient of 5 µs, printed every microsecond";
    // welcome~566f4f37 "63 % after one time constant r · c = 1 µs, 86 % after two"; welcome~f510779c
    // "signal vout: Electrical = 0.0 makes a transient start with the output at 0 V"; welcome~7238752c
    // "its monitor saw the output pass 0.5 V at 693.1 ns"; welcome~e7b611bc "The theory puts it at
    // 1 µs · ln 2 = 693.147 ns; ... here 0.04 ns early"
    let ran = RcProject::run("step");
    assert_eq!(ran.table.columns, ["time", "Step.vin", "Step.vout"]);
    assert_eq!(ran.table.rows.len(), 6, "5 us, every microsecond: a row at 0 and one per microsecond");
    for (row, printed) in ["0.000000e0", "1.000000e-6", "2.000000e-6", "3.000000e-6", "4.000000e-6", "5.000000e-6"].iter().enumerate() {
        Printed::holds(printed, ran.real(row, "time"), "the time of the row");
    }
    Printed::holds("0.000000e0", ran.real(0, "Step.vout"), "vout starts at 0 V");
    Printed::holds("6.320928e-1", ran.real(1, "Step.vout"), "63 % after one time constant");
    Printed::holds("8.646682e-1", ran.real(2, "Step.vout"), "86 % after two");
    let charge = |periods: f64| 1.0 - (-periods).exp();
    assert!((ran.real(1, "Step.vout") - charge(1.0)).abs() < 1e-4 && (ran.real(2, "Step.vout") - charge(2.0)).abs() < 1e-4, "1 - e^(-t/tau) to about four digits");
    let trips = ran.stream("trip");
    assert_eq!(trips.len(), 1, "one trip record: {:?}", ran.records);
    assert_eq!(trips[0].instance, "Step.mon");
    Printed::holds("6.931071e-7", trips[0].time, "the trip instant");
    Printed::holds("693.1e-9", trips[0].time, "693.1 ns");
    assert!(matches!(&trips[0].fields[..], [(name, Field::Real(v))] if name == "v" && (v - 0.5).abs() < 1e-6), "v = 0.5 at the crossing: {:?}", trips[0].fields);
    let theory = 1e-6 * std::f64::consts::LN_2;
    Printed::holds("693.147e-9", theory, "the theory");
    let early = theory - trips[0].time;
    assert!((early - 0.04e-9).abs() < 0.005e-9, "the solver is 0.04 ns early: {early:e}");
}

#[test]
fn si_suffixes_are_f64_and_a_plain_number_is_an_integer() {
    // welcome~58ff8428 "1k is 10³ and 1n is 10⁻⁹" (and the next sentences: an SI suffix is always an
    // f64, a plain 1 is an integer)
    let source = "module Top() { }\ndigital for Top { on at(1u) { emit!(\"numbers\", kilo = 1k, nano = 1n, plain = 1); } }\n";
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let ran = Model::written("numbers.msl", source).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let numbers = ran.stream("numbers");
    assert_eq!(numbers[0].fields, [("kilo".to_string(), Field::Real(1000.0)), ("nano".to_string(), Field::Real(1e-9)), ("plain".to_string(), Field::Int(1))]);
}

#[test]
fn a_monitor_that_checks_every_edge_writes_one_record_per_edge() {
    // welcome~b1f1d1ff "a monitor that checks every clock edge writes one record per edge": three
    // rising edges (0.5, 1.5 and 2.5 us) are three `checks` records, each passing, which the terminal
    // hides (welcome~b7e6723a, held by piperine-cli/options)
    let source = "module Osc(out clk: Bit = false) { f: f64 = 1M where f > 0.0; }\ndigital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\nmodule Monitor(in clk: Bit) { }\ndigital for Monitor { on clk.rise() { require(\"edge\", true); } }\nmodule Top() { signal clk: Bit; let osc = Osc(clk); let mon = Monitor(clk); }\n";
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(3e-6), ..Overrides::default() };
    let ran = Model::written("edges.msl", source).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let checks = ran.stream("checks");
    assert_eq!(checks.len(), 3, "one checks record per edge: {:?}", ran.records);
    assert!(checks.iter().all(|record| record.is_passing_check()), "every one passes: {checks:?}");
}

#[test]
fn adding_or_removing_an_emit_gives_the_same_waveforms() {
    // welcome~aa92bd2f "Adding or removing an emit! gives the same waveforms": the bench of
    // welcome#6 without its `emit!` line gives the table it gives with it
    let chapter = Chapter::read();
    let quiet = chapter.text("welcome#6").replace("        emit!(\"trip\", v = v(a));\n", "");
    assert_ne!(quiet, chapter.text("welcome#6"), "the emit! line is gone");
    let with_emit = RcProject::run("step");
    let model = Model::example("rc").with_source("benches/quiet.msl", &quiet);
    let overrides = Overrides { bench: Some("benches/quiet.msl".into()), ..Overrides::default() };
    let without = model.of_run("step").run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(without.stream("trip").len(), 0, "no trip record without the emit!");
    assert_eq!(without.table, with_emit.table, "the same waveforms");
}

#[test]
fn the_delayed_bench_steps_at_one_microsecond() {
    // welcome#17 (benches/delayed.msl added to the rc project); welcome~3afc3430 "on at(t0) {
    // fired <- true; } means when time reaches t0, set fired"; welcome~6c6cac8d "the record is
    // written when vout rises through 63.2 % of the step"; welcome~174ddaf4 "The first line under the header is the record the bench wrote: one time constant after
    // the step: vout is 1 − 1/e to the digit, at 0.07 ns before 2 µs"
    let overrides = Overrides { analysis: Some("transient".to_string()), stop: Some(5e-6), print_step: Some(1e-6), ..Overrides::default() };
    let ran = Chapter::read().marked("rc/benches/delayed.msl", "welcome#17").run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.real(1, "Delayed.vin"), 0.0, "the source is at 0 V until t0 = 1 us");
    assert_eq!(ran.real(2, "Delayed.vin"), 1.0, "and at v1 = 1 V after it");
    Printed::holds("6.321445e-1", ran.real(2, "Delayed.vout"), "vout one time constant after the step");
    let tau = ran.stream("tau");
    assert_eq!(tau.len(), 1, "{:?}", ran.records);
    Printed::holds("1.999933e-6", tau[0].time, "the tau record's instant");
    let Field::Real(vout) = tau[0].fields[1].1 else { panic!("vout is a real number: {:?}", tau[0].fields) };
    Printed::holds("6.321206e-1", vout, "vout at the record");
    assert!((vout - (1.0 - (-1.0f64).exp())).abs() < 5e-7, "1 - 1/e to the digit");
    let before = 2e-6 - tau[0].time;
    assert!((before - 0.07e-9).abs() < 0.01e-9, "0.07 ns before 2 us: {before:e}");
}

#[test]
fn the_inverter_drives_its_output_a_propagation_delay_after_its_input() {
    // welcome#21: "its one line drives the output from the input, tpd seconds later": `y` starts at
    // false (that first value is a change, so the reaction records it at time zero), `a` rises at
    // 1 ns, and `y` is true 10 ps after the start and falls at 1 ns + 10 ps
    let chapter = Chapter::read();
    let top = "module Top() { signal a: Bit = false; signal y: Bit; let inv = Inv(a, y); }\ndigital for Top { on at(1n) { a <- true; } on y.changed() { emit!(\"edge\", t = now(), v = y); } }\n";
    let model = chapter.lone("inv.msl", &["welcome#21"]).with_source("inv.msl", top);
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-9), ..Overrides::default() };
    let ran = model.run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let edges: Vec<(f64, &Field)> = ran.stream("edge").iter().map(|record| (record.time, &record.fields[1].1)).collect();
    assert_eq!(edges.len(), 3, "{:?}", ran.records);
    assert_eq!((edges[0].0, edges[0].1), (0.0, &Field::Bool(false)), "the initial value is the first change");
    Printed::holds("1.000000e-11", edges[1].0, "y first drives after tpd");
    assert_eq!(edges[1].1, &Field::Bool(true));
    Printed::holds("1.010000e-9", edges[2].0, "y falls tpd after a rises");
    assert_eq!(edges[2].1, &Field::Bool(false));
}

/// A mistake the chapter's table names: a bench of the Book written wrong.
struct Mistake;

impl Mistake {
    /// The bench `bench` of the Book with `from` replaced by `to`, run as the run `op` of the rc
    /// project whose bench is that text with `run` over it; the run must be refused.
    fn refused(row: &str, bench: &str, from: &str, to: &str, run: Overrides) -> Vec<Reported> {
        let text = Chapter::read().text(bench);
        let mistake = text.replace(from, to);
        assert_ne!(mistake, text, "{row}: `{from}` is in {bench}");
        let model = Model::example("rc").with_source("benches/mistake.msl", &mistake);
        Refusal::reported_by(model.of_run("op").run(Overrides { bench: Some("benches/mistake.msl".into()), ..run }), row)
    }
}

#[test]
fn e0319_a_param_with_no_default_must_be_bound() {
    // welcome~4bc80e1d: "E0319 let r1 = Resistor(vin, vout);": `r` has no default, so it is required
    let given = Mistake::refused("welcome~4bc80e1d", "welcome#6", "Resistor(vin, vout) { r: 1k }", "Resistor(vin, vout)", Overrides::default());
    assert_eq!(given[0].code, "E0319");
    assert_eq!(given[0].message, "`Step.r1` leaves the required param `r` unbound");
}

#[test]
fn e0312_a_port_must_be_connected() {
    // welcome~4bc8171d and welcome~81d65119: "E0312 let c1 = Capacitor(vout) { c: 1n };": every port must be connected
    let given = Mistake::refused("welcome~4bc8171d", "welcome#6", "Capacitor(vout, gnd) { c: 1n }", "Capacitor(vout) { c: 1n }", Overrides::default());
    assert_eq!(given[0].code, "E0312");
    assert_eq!(given[0].message, "`Capacitor` has 2 port(s), found 1 connection(s)");
}

#[test]
fn e0317_a_where_that_is_false_is_refused() {
    // welcome~4bc81c1d: "E0317 Resistor(vin, vout) { r: 0.0 }": the where `r > 0.0` is false
    let given = Mistake::refused("welcome~4bc81c1d", "welcome#6", "{ r: 1k }", "{ r: 0.0 }", Overrides::default());
    assert_eq!(given[0].code, "E0317");
    assert_eq!(given[0].message, "the `where` of the param `Step.r1.r` is false for the value 0.0");
}

#[test]
fn e0901_there_is_no_println() {
    // welcome~80317c1d: "E0901 println!(\"vout = {}\", v(vout));": there is no println!
    let given = Mistake::refused("welcome~80317c1d", "welcome#6", "emit!(\"trip\", v = v(a));", "println!(\"vout = {}\", v(a));", Overrides::default());
    assert_eq!(given[0].code, "E0901");
    assert_eq!(given[0].message, "macros are not part of the language");
}

#[test]
fn e0404_a_commit_belongs_in_a_reaction() {
    // welcome~54e1b21d: "E0404 fired <- true; in the module body": a commit belongs in a reaction
    let run = Overrides { analysis: Some("op".to_string()), ..Overrides::default() };
    let given = Mistake::refused("welcome~54e1b21d", "welcome#17", "signal fired: Bit = false;\n}", "signal fired: Bit = false;\n    fired <- true;\n}", run);
    assert_eq!(given[0].code, "E0404");
    assert_eq!(given[0].message, "a commit (`<-`) in a module body");
}

#[test]
fn the_status_paragraph_lists_what_is_still_refused_and_one_item_runs() {
    // welcome~e1871864: "Parts of the language beyond that are still refused by name with P0001: in analog,
    // noise, idt and delay; elsewhere, generic modules, params of an integer type and resolved natures (a nature
    // that implements Resolve)." Every item is `P0001`; the math methods such as `sin` and the `settle` event, which the sentence
    // no longer lists, run (unknown-semantics-events T5 and T22)
    let p0001 = |row: &str, source: &str, message: &str| {
        let model = Model::written("refused.msl", source);
        let overrides = Overrides { analysis: Some("op".to_string()), top: Some("Top".to_string()), ..Overrides::default() };
        let given = Refusal::of(model.run(overrides), row);
        assert!(given.iter().any(|(code, text)| code == "P0001" && text == message), "{source:?} gives {given:?}");
    };
    let row = "welcome~e1871864";
    p0001(row, "module Top() { signal p: Electrical; }\nanalog for Top { i(p, gnd) +> white_noise(1e-12, \"n\"); }\n", "core primitive `noise` is not supported yet");
    p0001(row, "module Top() { signal p: Electrical; }\nanalog for Top { i(p, gnd) +> idt(v(p)); }\n", "generic module is not supported yet");
    p0001(row, "module Top() { signal p: Electrical; signal q: Electrical; }\nanalog for Top { v(q) +> delay(v(p), 1n); }\n", "core primitive `delay` is not supported yet");
    p0001(row, "module G<T: Repr>() {}\nmodule Top() { let g = G::<f64>(); }\n", "generic module is not supported yet");
    p0001(row, "module Top() { n: u8 = 3; }\n", "param of type `u8` is not supported yet");
    p0001(row, "module Top() { signal w: Wire; }\n", "resolved nature `Wire` is not supported yet");
    // the `settle` event is compiled since unknown-semantics-events T5 (L52)
    let settle = Model::written("settle.msl", "module Top() { }\ndigital for Top { on settle { } }\n");
    settle.run(Overrides { analysis: Some("op".to_string()), top: Some("Top".to_string()), ..Overrides::default() }).unwrap_or_else(|failure| panic!("{row}: `on settle` is refused: {failure}"));
    let sin = Model::written("sin.msl", "module Top() { signal p: Electrical; x: f64 = 1.0; }\nanalog for Top { v(p) +> x.sin(); }\n");
    let ran = sin.run(Overrides { analysis: Some("op".to_string()), top: Some("Top".to_string()), ..Overrides::default() }).unwrap_or_else(|failure| panic!("{row}: `sin` is refused again: {failure}"));
    Printed::holds("8.414710e-1", ran.signal("Top.p"), "sin(1.0) runs");
}
