//! Scope: chapter 3 of the Book, "Values and functions" (feature `root-e2e`, RE-02 and RE-03): the
//! statements on integer width and wrapping, the functions and their optional arguments, the window
//! monitor and the adder that reads an undriven input, the sample-and-hold that reads a digital carrier
//! through `known_or`, and the mistakes of the chapter's table, each run through `piperine::`. Each
//! test opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

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

#[test]
fn the_statements_wrap_and_convert_as_the_book_says() {
    // values#1 (run): the block's statements inside a reaction that emits what they computed;
    // values~ec65c268 "Arithmetic always wraps"; values~d5f52d81 "a 3-bit duty word is a u3, and a
    // 10-bit ADC code a u10"; values~df08bac4 "integer arithmetic wraps the way a register does, in every
    // build"; the block's own comments: 250 + 10 is 4, a u3 holds 0 to 7, 4.7k is an f64 4700.0
    let chapter = Chapter::read();
    let model = chapter.wrapped("statements.msl", Reaction::BEFORE, "values#1", "emit!(\"v\", a = a, b = b, duty = duty, r = r, n = n, x = x);\n} }\n");
    let ran = model.run(Transient::of("Top", 2e-6)).unwrap_or_else(|failure| panic!("the statements fail: {failure}"));
    let v = &ran.stream("v")[0];
    assert_eq!(Fields::of(v, "a"), &Field::Int(250));
    assert_eq!(Fields::of(v, "b"), &Field::Int(4), "250 + 10 wraps modulo 2^8");
    assert_eq!(Fields::of(v, "duty"), &Field::Int(5));
    assert_eq!(Fields::of(v, "r"), &Field::Real(4700.0), "an SI suffix makes an f64");
    assert_eq!(Fields::of(v, "n"), &Field::Int(3), "a plain literal is an integer");
    assert_eq!(Fields::of(v, "x"), &Field::Real(14100.0), "r * n as f64");
    let widths = Model::written("widths.msl", &Reaction::source("let duty: u3 = 7; let up = duty + 1; let code: u10 = 1023; let next = code + 1; let small: u3 = 1; let down = small - 2; emit!(\"w\", up = up, next = next, down = down);"));
    let w = Transient::ran(&widths, "Top", 2e-6);
    let w = &w.stream("w")[0];
    assert_eq!(Fields::of(w, "up"), &Field::Int(0), "u3: 7 + 1 wraps to 0");
    assert_eq!(Fields::of(w, "next"), &Field::Int(0), "u10: 1023 + 1 wraps to 0");
    assert_eq!(Fields::of(w, "down"), &Field::Int(7), "u3: 1 - 2 wraps to 7");
}

#[test]
fn the_prelude_gives_its_constants() {
    // values~9273ae3d: "The prelude gives you the constants Q, K_B, EPS0, PI, E and inf"
    let source = Reaction::source("emit!(\"c\", q = Q, kb = K_B, eps0 = EPS0, pi = PI, e = E, i = inf);");
    let ran = Transient::ran(&Model::written("constants.msl", &source), "Top", 2e-6);
    let c = &ran.stream("c")[0];
    let real = |name: &str| match Fields::of(c, name) {
        Field::Real(value) => *value,
        other => panic!("{name} is a real number: {other:?}"),
    };
    Printed::holds("1.602177e-19", real("q"), "Q");
    Printed::holds("1.380649e-23", real("kb"), "K_B");
    Printed::holds("8.854188e-12", real("eps0"), "EPS0");
    Printed::holds("3.141593e0", real("pi"), "PI");
    Printed::holds("2.718282e0", real("e"), "E");
    assert!(real("i").is_infinite() && real("i") > 0.0, "inf");
}

#[test]
fn a_function_has_the_class_its_body_gives() {
    // values#2 (refused): `diode_current` is pure and `handshake` returns an `Event`; values~c389077a
    // (refused): "The compiler infers a class from the body: a pure function, like diode_current, works
    // anywhere; one that probes a voltage or reads now() works where the solver is running; one that
    // returns Event, like handshake, works where events go; and one that calls emit! works where emit!
    // does." The pure, `now()` and `emit!` classes run; an `Event` function is `P0001` (GAPS.md L53)
    let chapter = Chapter::read();
    let top = Reaction::source("emit!(\"d\", i = diode_current(0.6, 1e-14, 0.026), t = clock(), x = 1);").replace("module Top() { }", "fn clock() -> f64 { now() }\nfn say() { emit!(\"say\", x = 1.0); }\nmodule Top() { }").replace("emit!(\"d\"", "say(); emit!(\"d\"");
    let ran = Transient::ran(&chapter.wrapped("functions.msl", "", "values#2", &top), "Top", 2e-6);
    let d = &ran.stream("d")[0];
    let formula = 1e-14 * ((0.6f64 / 0.026).exp() - 1.0);
    assert!(matches!(Fields::of(d, "i"), Field::Real(i) if (i - formula).abs() < 1e-9 * formula), "diode_current: {:?}", Fields::of(d, "i"));
    assert!(matches!(Fields::of(d, "t"), Field::Real(t) if (t - 1e-6).abs() < 1e-12), "a function that reads now() runs where the solver does: {:?}", Fields::of(d, "t"));
    assert_eq!(ran.stream("say").len(), 1, "a function that calls emit! writes where emit! does");
    let reaching = "module Top(in req: Bit, in ack: Bit) { }\ndigital for Top { on handshake(req, ack) { emit!(\"h\", t = now()); } }\n";
    let refused = chapter.wrapped("handshake.msl", "", "values#2", reaching).run(Transient::of("Top", 2e-6));
    Refusal::expect(refused, "values#2", "P0001", "method `rise` on this receiver is not supported yet");
}

#[test]
fn optional_arguments_may_be_left_out_or_named() {
    // values#3 (run) and values#4 (the three calls, a fragment of it); values~4b156a2d: "Passing a plain
    // T wraps it in Some, the one implicit conversion in the language": `scale(v, 3.0)` is `Some(3.0)`,
    // and an integer literal is never an f64
    let chapter = Chapter::read();
    for call in ["scale(v)", "scale(v, 3.0)", "scale(v, limit: 1.0)"] {
        assert!(chapter.text("values#4").contains(call), "values#4 shows {call}");
    }
    let body = "let v = 5.0; emit!(\"s\", left_out = scale(v), positional = scale(v, 3.0), named = scale(v, limit: 1.0));";
    let model = chapter.wrapped("scale.msl", "", "values#3", &Reaction::source(body));
    let ran = model.run(Transient::of("Top", 2e-6)).unwrap_or_else(|failure| panic!("scale fails: {failure}"));
    let s = &ran.stream("s")[0];
    assert_eq!(Fields::of(s, "left_out"), &Field::Real(10.0), "k = 2.0 unless given");
    assert_eq!(Fields::of(s, "positional"), &Field::Real(15.0), "k = Some(3.0)");
    assert_eq!(Fields::of(s, "named"), &Field::Real(1.0), "limit skips k and clamps 10.0 to 1.0");
    let integer = Faulty::elaborating("values~4b156a2d", "module Top() { r: f64 = 100; }\n", "Top");
    assert_eq!(integer.first("P0002").message, "mismatched types: expected `f64`, found `an integer literal`");
}

#[test]
fn the_monitor_file_runs_its_window_monitor_and_its_adder() {
    // values#6, values#10 and values#14 (run: the three parts of monitor.msl), values#5 is the enum of values#6;
    // values~d0e86096 "This file watches a node and reports, on every clock edge, whether its voltage is
    // under, inside or over a window"; values~8ad56d03 "The source is a ramp of 1 V/µs, so the node walks
    // through all three zones"; values~6ecca079 "The clock rises at 0.5, 1.5 and 2.5 µs, where the ramp is
    // near 0.5, 1.5 and 2.5 V"; values~3c87d509 "if z != zone { emit!(…) } prints only when the zone
    // changes. zone on the right still reads the old value"; values~2931bd2e "The command prints an
    // enum field as the number of its variant, counted from 0 ... Under is 0, Ok 1 and Over 2" (the
    // command's line is held with the listing); values~362f6b51 "This adder is placed in a top that gives
    // a the constant 3 and takes b from an in port of its own, which nothing drives"; values~9a0ece05
    // "An unknown value prints as x" (the library holds it as an unknown field)
    let chapter = Chapter::read();
    let monitor = chapter.marked("monitor.msl", "values#6").run(Transient::of("MonitorTop", 3e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let zones = monitor.stream("zone");
    assert_eq!(zones.len(), 3, "one record per change of zone: {:?}", monitor.records);
    for (record, (instant, near, from, to)) in zones.iter().zip([("5.000000e-7", 0.5, 1, 0), ("1.500000e-6", 1.5, 0, 1), ("2.500000e-6", 2.5, 1, 2)]) {
        assert_eq!(record.instance, "MonitorTop.mon");
        Printed::holds(instant, record.time, "the clock edge");
        assert!(matches!(Fields::of(record, "v"), Field::Real(v) if (v - near).abs() < 0.01), "v is near {near}: {:?}", Fields::of(record, "v"));
        assert_eq!((Fields::of(record, "from"), Fields::of(record, "to")), (&Field::Int(from), &Field::Int(to)), "Under is 0, Ok 1 and Over 2");
    }
    let longer = chapter.marked("monitor.msl", "values#6").run(Transient::of("MonitorTop", 5e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(longer.stream("zone").len(), 3, "the edges at 3.5 and 4.5 us find the zone unchanged and print nothing");
    let adder = chapter.marked("monitor.msl", "values#10").run(Transient::of("AdderTop", 3e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let lost = adder.stream("lost");
    assert_eq!(lost.len(), 1, "only one record is written: unknown to unknown is not a change: {:?}", adder.records);
    Printed::holds("5.000000e-7", lost[0].time, "the first clock edge");
    assert_eq!((Fields::of(lost[0], "a"), Fields::of(lost[0], "b")), (&Field::Int(3), &Field::Unknown), "a is the constant 3, b is unknown");
    assert_eq!(adder.table.rows[1][adder.table.columns.iter().position(|name| name == "AdderTop.sum").expect("the table shows sum")], piperine::Cell::Unknown, "sum is unknown at 3 us");
    // values#14 (PeekTop): `if a > b` with b unknown takes both paths, and each record of the arm says
    // `maybe`; the listing prints `maybe=true` on the three rising edges
    let peek = chapter.marked("monitor.msl", "values#14").run(Transient::of("PeekTop", 3e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let bigger = peek.stream("bigger");
    assert_eq!(bigger.len(), 3, "one record per rising edge: {:?}", peek.records);
    for (record, instant) in bigger.iter().zip(["5.000000e-7", "1.500000e-6", "2.500000e-6"]) {
        assert_eq!(record.instance, "PeekTop.peek");
        assert!(record.maybe, "a record under an unknown condition is a maybe");
        Printed::holds(instant, record.time, "the clock edge");
        assert_eq!(Fields::of(record, "a"), &Field::Int(3));
    }
}

#[test]
fn the_flag_is_read_with_known_known_or_and_is_known() {
    // values~1aeb5afb: "You read the flag with three methods every Repr value has: x.known() gives an
    // Option (None when any level is unknown), x.known_or(d) gives the value or a default, and
    // x.is_known() a bool" (b is an `in` port nothing drives)
    let source = "module Top(in b: Bit<u8>) { }\ndigital for Top { on at(1u) { let k = b.known(); let n = match k { Some(v) => 1, None => 0 }; emit!(\"k\", ok = b.is_known(), d = b.known_or(7), n = n); } }\n";
    let ran = Transient::ran(&Model::written("flag.msl", source), "Top", 2e-6);
    let k = &ran.stream("k")[0];
    assert_eq!(Fields::of(k, "ok"), &Field::Bool(false), "is_known is a bool");
    assert_eq!(Fields::of(k, "d"), &Field::Int(7), "known_or gives the default");
    assert_eq!(Fields::of(k, "n"), &Field::Int(0), "known gives None");
}

#[test]
fn analog_reads_a_digital_carrier_through_known_or() {
    // values#17 (run): the sample-and-hold of the Book, with a source on its input and a clock edge at
    // 1 us; values~267f8cfe "Writing v(vout) + held; instead is error E0150, and the diagnostic names
    // the carrier" and values~39a9371d (E0150 in the table)
    let chapter = Chapter::read();
    let after = "module Source(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Source { v(p, n) +> dc; }\nmodule Top() { signal a: Electrical; signal b: Electrical; signal c: Bit = false; let s = Source(a, gnd); let h = SampleHold(a, b, c); }\ndigital for Top { on at(1u) { c <- true; } }\n";
    let model = chapter.wrapped("sample_hold.msl", "", "values#17", after);
    let ran = model.run(Transient::of("Top", 3e-6)).unwrap_or_else(|failure| panic!("the sample-and-hold fails: {failure}"));
    let last = ran.table.rows.len() - 1;
    assert_eq!(ran.real(0, "Top.b"), 0.0, "before the first edge vout sits at 0 V: held starts at 0.0");
    assert_eq!(ran.real(last, "Top.b"), 1.0, "after the edge vout holds vin");
    let raw = chapter.text("values#17").replace("held.known_or(0.0)", "held");
    let faulty = Faulty::elaborating("values~267f8cfe", &format!("{raw}{after}"), "Top");
    assert!(faulty.first("E0150").message.contains("the discrete carrier `held`"), "{}", faulty.first("E0150").message);
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
fn e0143_there_are_no_default_arguments() {
    // values~39acbe1d: "E0143 fn scale(x: f64, k: f64 = 2.0)": no default arguments
    let faulty = Faulty::elaborating("values~39acbe1d", "fn scale(x: f64, k: f64 = 2.0) -> f64 { x * k }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.pointed_at("E0143"), "= 2.0");
}

#[test]
fn e0140_a_named_argument_must_name_an_optional_parameter() {
    // values~39acbd1d: "E0140 scale(v, gain: 3.0)": it is called k
    let source = "fn scale(x: f64, k: Option<f64>) -> f64 { x * k.unwrap_or(2.0) }\nmodule Top() { }\n";
    let faulty = Faulty::elaborating("values~39acbd1d", &format!("{source}digital for Top {{ on at(1u) {{ let v = 5.0; emit!(\"s\", a = scale(v, gain: 3.0)); }} }}\n"), "Top");
    assert_eq!(faulty.first("E0140").message, "no optional parameter `gain` in `scale`");
}

#[test]
fn e0142_a_positional_argument_cannot_follow_a_named_one() {
    // values~39acbf1d: "E0142 scale(v, limit: 1.0, 3.0)"
    let source = "fn scale(x: f64, k: Option<f64>, limit: Option<f64>) -> f64 { x * k.unwrap_or(2.0) }\nmodule Top() { }\n";
    let faulty = Faulty::elaborating("values~39acbf1d", &format!("{source}digital for Top {{ on at(1u) {{ let v = 5.0; emit!(\"s\", a = scale(v, limit: 1.0, 3.0)); }} }}\n"), "Top");
    assert_eq!(faulty.first("E0142").message, "a positional argument after a named one");
}

#[test]
fn an_extern_function_is_refused_with_or_without_its_derivative() {
    // values#18 (refused): `#[derivative(erfcx_dx)] extern "C" fn erfcx` and its derivative; values~3998361d
    // (unraised): "E0123 extern \"C\" fn erfcx(x: f64) -> f64; called in analog: Add #[derivative]". No
    // source raises E0123 (GAPS.md section 10): an `extern` function is `P0001` (L12), with the
    // derivative the Book asks for or without. When it runs this test fails and the rows move
    let chapter = Chapter::read();
    let analog = "module Top() { signal p: Electrical; }\nanalog for Top { v(p) +> erfcx(1.0); }\n";
    let overrides = || Overrides { analysis: Some("op".to_string()), top: Some("Top".to_string()), ..Overrides::default() };
    Refusal::expect(chapter.wrapped("foreign.msl", "", "values#18", analog).run(overrides()), "values#18", "P0001", "`extern` function is not supported yet");
    let bare = Refusal::of(Model::written("bare.msl", &format!("extern \"C\" fn erfcx(x: f64) -> f64;\n{analog}")).run(overrides()), "values~3998361d");
    assert!(bare.iter().all(|(code, _)| code != "E0123") && bare.contains(&("P0001".to_string(), "`extern` function is not supported yet".to_string())), "no E0123: {bare:?}");
}
