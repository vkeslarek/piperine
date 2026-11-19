//! Scope: chapter 11 of the Book, "Reading errors" (feature `root-e2e`, RE-02 and RE-03): the
//! models of the chapter run through `piperine::` (`Project`, `Session`), a model the Book shows
//! refused is asserted refused with the codes and messages the tool gives, and each code the
//! chapter's tables name is provoked by the smallest model that makes the mistake. Each test opens
//! with the inventory ids it holds, from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod inventory;
    pub mod model;
    pub mod records;
    pub mod refusal;
}

use piperine::Overrides;
use support::book::Book;
use support::inventory::Inventory;
use support::model::Model;
use support::refusal::{Refusal, Reported};

/// The Book's text, the inventory, and what a chapter test needs of both.
struct Chapter {
    book: Book,
    inventory: Inventory,
}

impl Chapter {
    /// The Book and the inventory as they stand in this repository.
    fn read() -> Chapter {
        Chapter { book: Book::read(), inventory: Inventory::read() }
    }

    /// A lone-file model of the blocks `ids`, written as `file_name`.
    fn lone(&self, file_name: &str, ids: &[&str]) -> Model {
        Model::lone(&self.book, &self.inventory, file_name, ids)
    }
}

/// The overrides of an operating point of the top `top`, the run a lone file's check makes.
fn operating_point_of(top: &str) -> Overrides {
    Overrides { analysis: Some("op".to_string()), top: Some(top.to_string()), ..Overrides::default() }
}

/// One faulty model of the chapter, run, and what the tool said of it.
struct Faulty {
    source: String,
    given: Vec<Reported>,
}

impl Faulty {
    /// The operating point of the top `top` of the lone file `m.msl` holding `source`, which the tool
    /// must refuse; panics naming `row` when it runs.
    fn elaborating(row: &str, source: &str, top: &str) -> Faulty {
        Faulty::running(row, source, operating_point_of(top))
    }

    /// The run `overrides` ask of the lone file `m.msl` holding `source`, which must fail.
    fn running(row: &str, source: &str, overrides: Overrides) -> Faulty {
        let model = Model::written("m.msl", source);
        Faulty { source: model.text_of("m.msl"), given: Refusal::reported_by(model.run(overrides), row) }
    }

    /// The first diagnostic with the code `code`; panics listing what the run gave when there is none.
    fn first(&self, code: &str) -> &Reported {
        self.given.iter().find(|reported| reported.code == code).unwrap_or_else(|| panic!("no {code} in {:?}", self.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>()))
    }

    /// The text of the source that the primary label of the first `code` points at.
    fn pointed_at(&self, code: &str) -> &str {
        self.first(code).primary_text(&self.source)
    }
}

/// The package of the Book's second rendering: `util` is both a dependency and a local module, and
/// `src/lib.msl` names `util::cells::Cell`.
fn ambiguous_package() -> Model {
    Model::example("rc")
        .with_source("Piperine.toml", "\n[dependencies]\nutil = { path = \"deps/util\" }\n")
        .with_source("deps/util/Piperine.toml", "[package]\nname = \"util\"\nversion = \"0.1.0\"\n")
        .with_source("deps/util/src/lib.msl", "pub mod cells;\n")
        .with_source("deps/util/src/cells.msl", "pub module Cell(p: Electrical) {}\n")
        .with_source("src/util.msl", "pub module Other(p: Electrical) {}\n")
        .with_source("src/lib.msl", "mod util;\nuse util::cells::Cell;\n")
}

/// The bench the Book asks for: a module without ports that places the faulty sample-and-hold,
/// "because checking starts from a top and works its way down".
const SAMPLE_HOLD_BENCH: &str = "module Top() {\n    signal a: Electrical;\n    signal b: Electrical;\n    signal c: Bit;\n    let h = SampleHold(a, b, c);\n}\n";

#[test]
fn a_refusal_that_runs_fails_naming_its_row() {
    // RE-02 AC 3: a block asserted refused that starts to run fails the test, naming its row, so the
    // row moves to `run` with the Book. `examples/rc` runs; asserting it refused panics.
    let runs = Model::example("rc").run_named("op", Overrides::default());
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Refusal::expect(runs, "errors#3", "E0310", "any message")));
    let payload = caught.expect_err("a model that runs is not a refusal");
    let message = payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(|text| text.to_string())).expect("the panic carries a message");
    assert!(message.contains("errors#3") && message.contains("now runs"), "the panic names the row and says it now runs: {message}");
}

#[test]
fn the_worked_sample_hold_is_refused_with_its_three_codes() {
    // errors#3: the Book's faulty sample-and-hold, placed in a bench, "has three mistakes that
    // elaboration finds": E0310 on `clk: Bit`, E0430 on `on clk`, E0150 on `+> held`, sorted by position
    let chapter = Chapter::read();
    let model = chapter.lone("sample_hold.msl", &["errors#3"]).with_source("sample_hold.msl", SAMPLE_HOLD_BENCH);
    let given = Refusal::of(model.run(operating_point_of("Top")), "errors#3");
    let codes: Vec<&str> = given.iter().map(|(code, _)| code.as_str()).collect();
    assert_eq!(codes, ["E0310", "E0430", "E0150"], "the three mistakes, top to bottom: {given:?}");
    Refusal::expect(model.run(operating_point_of("Top")), "errors#3", "E0310", "a port of a discrete nature takes `in` or `out` (§8.1.5)");
}

#[test]
fn the_discrete_read_diagnostic_has_the_parts_the_book_shows() {
    // errors#1 (the rendering of E0150): a code, a message, the primary label on the carrier `held`
    // and, in the Book, a help line naming `known()`, `known_or(d)` and `high()`. The tool gives
    // no help line today: a missing part, listed in GAPS.md section 11 (Phase 9 adds it)
    let chapter = Chapter::read();
    let model = chapter.lone("sample_hold.msl", &["errors#3"]).with_source("sample_hold.msl", SAMPLE_HOLD_BENCH);
    let source = model.text_of("sample_hold.msl");
    let given = Refusal::reported_by(model.run(operating_point_of("Top")), "errors#1");
    let read = given.iter().find(|reported| reported.code == "E0150").expect("E0150 is one of the three");
    assert_eq!(read.primary_text(&source), "held", "the label points at the carrier");
    assert_eq!(read.primary_label, "a discrete carrier read in `analog`");
    assert!(read.message.contains("reads the discrete carrier `held`"), "{}", read.message);
    assert_eq!(read.help, None, "errors#1 diverges: the Book shows a help line, the tool gives none");
}

#[test]
fn the_ambiguous_name_diagnostic_has_the_parts_the_book_shows() {
    // errors#2 (the rendering of E0163): a code, a message naming `util`, a primary label on the
    // ambiguous path and, in the Book, a secondary label on `mod util;` ("the local namespace") and
    // a help line. The tool gives the primary label and the help line, and no secondary label: a
    // missing part, listed in GAPS.md section 11 (Phase 9 adds it)
    let model = ambiguous_package();
    let source = model.text_of("src/lib.msl");
    let given = Refusal::reported_by(model.run_named("op", Overrides::default()), "errors#2");
    let ambiguous = given.iter().find(|reported| reported.code == "E0163").expect("E0163 is given");
    assert_eq!(ambiguous.message, "`util` names both a dependency and an item of this namespace");
    assert_eq!(ambiguous.primary_text(&source), "util::cells::Cell", "the label points at the path, not at its first segment");
    assert_eq!(ambiguous.primary_label, "ambiguous name");
    assert!(ambiguous.help.as_deref().is_some_and(|help| help.contains("package = \"util\"") && help.contains("crate::util")), "{:?}", ambiguous.help);
    assert!(ambiguous.secondary_labels.is_empty(), "errors#2 diverges: the Book shows a secondary label on `mod util;`, the tool gives none");
}

/// A fixed model of the worked example: the three lines the Book gives, with a source on `vin` so the
/// operating point has a solution.
const SAMPLE_HOLD_FIXED: &str = "module SampleHold(vin: Electrical, vout: Electrical, in clk: Bit) {\n    signal held: Bit<f64> = 0.0;\n}\ndigital for SampleHold {\n    on clk.rise() { held <- v(vin); }\n}\nanalog for SampleHold {\n    v(vout) +> held.known_or(0.0);\n}\nmodule Source(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Source { v(p, n) +> dc; }\nmodule Top() {\n    signal a: Electrical;\n    signal b: Electrical;\n    signal c: Bit;\n    let s = Source(a, gnd);\n    let h = SampleHold(a, b, c);\n}\n";

impl Chapter {
    /// The diagnostics the worked example gives, with the source they point into.
    fn worked_example(&self, row: &str) -> Faulty {
        let model = self.lone("sample_hold.msl", &["errors#3"]).with_source("sample_hold.msl", SAMPLE_HOLD_BENCH);
        Faulty { source: model.text_of("sample_hold.msl"), given: Refusal::reported_by(model.run(operating_point_of("Top")), row) }
    }
}

#[test]
fn e0310_is_found_on_the_bare_port_of_the_worked_example() {
    // errors~1f3eeee4: "E0310 on `clk: Bit`": the label sits on the type of the port
    let worked = Chapter::read().worked_example("errors~1f3eeee4");
    assert_eq!(worked.pointed_at("E0310"), "Bit");
}

#[test]
fn e0430_is_found_on_the_head_of_the_worked_example_s_reaction() {
    // errors~d6787e9a: "E0430 on `on clk`": the head of the reaction is the level `clk`
    let worked = Chapter::read().worked_example("errors~d6787e9a");
    assert_eq!(worked.pointed_at("E0430"), "clk");
}

#[test]
fn e0150_is_found_on_the_read_of_the_carrier_in_the_worked_example() {
    // errors~4cc71bd2: "E0150 on `+> held`: `analog` reads a digital carrier without saying what
    // unknown means": the label sits on `held`, and the message says what the read lacks
    let worked = Chapter::read().worked_example("errors~4cc71bd2");
    assert_eq!(worked.pointed_at("E0150"), "held");
    assert!(worked.first("E0150").message.contains("other than through `known`, `known_or`, `is_known` or `high`"), "{}", worked.first("E0150").message);
}

#[test]
fn the_three_fixes_make_the_worked_example_elaborate() {
    // errors~2db98d22: "Fix the three lines and the same command prints one `ok` line and exits with
    // 0": `in clk`, `on clk.rise()` and `held.known_or(0.0)` leave no diagnostic and the operating
    // point runs (the `ok` line and the exit status are the binary's, `piperine-cli/commands`)
    let ran = Model::written("sample_hold.msl", SAMPLE_HOLD_FIXED).run(operating_point_of("Top"));
    let table = ran.unwrap_or_else(|failure| panic!("the fixed example is refused: {failure}")).table;
    assert!(!table.rows.is_empty(), "the operating point has rows");
}

#[test]
fn a_reaction_on_a_changed_unknown_emits_the_inputs() {
    // errors~473f7809: "A reaction on `q.changed() && !q.is_known()` that emits the inputs of `q`":
    // `sum` becomes unknown when `b` is, and the reaction emits `a = 3` and `b` unknown
    let source = "module Adder(in clk: Bit, in a: Bit<u8>, in b: Bit<u8>, out sum: Bit<u8> = 0) {}\ndigital for Adder {\n    on clk.rise() { sum <- a + b; }\n    on sum.changed() && !sum.is_known() { emit!(\"lost\", a = a, b = b); }\n}\nmodule Top(in b: Bit<u8>) {\n    signal clk: Bit = false;\n    signal sum: Bit<u8>;\n    let add = Adder(clk, 3, b, sum);\n}\ndigital for Top { on at(1u) { clk <- true; } }\n";
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let ran = Model::written("adder.msl", source).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let lost = ran.stream("lost");
    assert_eq!(lost.len(), 1, "one record: {:?}", ran.records);
    assert_eq!(lost[0].fields, [("a".to_string(), piperine::Field::Int(3)), ("b".to_string(), piperine::Field::Unknown)]);
    assert!((lost[0].time - 1e-6).abs() < 1e-12, "at the clock edge: {}", lost[0].time);
}

#[test]
fn e0900_is_a_syntax_error_that_names_the_token() {
    // errors~80317d1d: "E0900: A syntax error. Read the token the message names"
    let faulty = Faulty::elaborating("errors~80317d1d", "module M( {\n", "M");
    let syntax = faulty.first("E0900");
    assert_eq!(syntax.message, "unexpected end of input, expected `)`");
    assert_eq!(faulty.pointed_at("E0900"), "(", "the label points at the delimiter that is never closed");
}

#[test]
fn e0901_is_a_macro_other_than_emit() {
    // errors~80317c1d: "E0901: A macro other than `emit!` or `include_str!`"
    let faulty = Faulty::elaborating("errors~80317c1d", "fn f() { println!(\"x\"); }\nmodule Top() {}\n", "Top");
    assert_eq!(faulty.pointed_at("E0901"), "println!(\"x\")");
}

#[test]
fn e0161_is_a_name_that_does_not_resolve() {
    // errors~39a5d01d: "E0161: A path, `use` or `mod` that does not resolve"
    let faulty = Faulty::elaborating("errors~39a5d01d", "module Top() { let x = Missing(); }\n", "Top");
    assert_eq!(faulty.pointed_at("E0161"), "Missing");
    assert_eq!(faulty.first("E0161").message, "cannot find `Missing` in this scope");
}

#[test]
fn e0162_is_a_name_declared_twice() {
    // errors~39a5cf1d: "E0162: A name declared twice in one namespace"
    let faulty = Faulty::elaborating("errors~39a5cf1d", "module A() {}\nmodule A() {}\nmodule Top() {}\n", "Top");
    assert_eq!(faulty.pointed_at("E0162"), "A");
    assert_eq!(faulty.first("E0162").message, "`A` is defined more than once in this namespace");
}

#[test]
fn e0163_is_a_name_that_is_a_dependency_and_a_local_item() {
    // errors~39a5ce1d: "E0163: A `use` whose first segment is both a dependency and a local item"
    let model = ambiguous_package();
    let given = Refusal::of(model.run_named("op", Overrides::default()), "errors~39a5ce1d");
    assert!(given.iter().any(|(code, message)| code == "E0163" && message.contains("`util` names both a dependency and an item")), "{given:?}");
}

#[test]
fn e0143_is_a_default_in_a_signature_or_an_overload() {
    // errors~39acbe1d: "E0143: A default value in a `fn` signature, or an overload"
    let default = Faulty::elaborating("errors~39acbe1d", "fn f(x: f64 = 1.0) -> f64 { x }\nmodule Top() {}\n", "Top");
    assert_eq!(default.pointed_at("E0143"), "= 1.0");
    let overload = Faulty::elaborating("errors~39acbe1d", "fn f() {}\nfn f(x: u8) {}\nmodule Top() {}\n", "Top");
    assert_eq!(overload.first("E0143").message, "an overload of `f` in a function signature");
}

#[test]
fn e0150_is_an_analog_read_of_a_digital_carrier() {
    // errors~39a9371d: "E0150: `analog` reads a digital carrier without `known_or` / `high()`"
    let worked = Chapter::read().worked_example("errors~39a9371d");
    assert!(worked.first("E0150").message.contains("an `analog` block reads the discrete carrier `held`"), "{}", worked.first("E0150").message);
}

#[test]
fn e0220_is_a_second_drive_of_a_one_driver_carrier() {
    // errors~42ae741d: "E0220: A second drive of a one-driver carrier"
    let faulty = Faulty::elaborating("errors~42ae741d", "module Source(out q: Bit) { }\nmodule Top() { signal s: Bit; let a = Source(s); let b = Source(s); }\n", "Top");
    assert_eq!(faulty.pointed_at("E0220"), "s");
    assert!(faulty.first("E0220").message.contains("handed to a second `out` port"), "{}", faulty.first("E0220").message);
}

#[test]
fn e0221_is_a_definition_and_a_commit_to_one_carrier() {
    // errors~42ae751d: "E0221: A definition and a commit to the same carrier in one module"
    let source = "module Top(in x: Bit, in y: Bit) { signal s: Bit; s = x; }\ndigital for Top { on at(1u) { s <- y; } }\n";
    let faulty = Faulty::elaborating("errors~42ae751d", source, "Top");
    assert_eq!(faulty.pointed_at("E0221"), "s");
    assert!(faulty.first("E0221").message.contains("has a definition and a commit in one module"), "{}", faulty.first("E0221").message);
}

#[test]
fn e0222_is_a_write_to_what_the_module_may_not_drive() {
    // errors~42ae761d: "E0222: Writing something this module may not drive, such as an `in` port"
    let faulty = Faulty::elaborating("errors~42ae761d", "module Top(in a: Bit, out q: Bit) { a = q; }\n", "Top");
    assert_eq!(faulty.pointed_at("E0222"), "a");
    assert!(faulty.first("E0222").message.contains("the target of a definition is an `in` port"), "{}", faulty.first("E0222").message);
}

#[test]
fn e0312_is_a_port_left_unconnected_or_too_many_connections() {
    // errors~4bc8171d: "E0312: A port left unconnected, or too many connections"
    let few = Faulty::elaborating("errors~4bc8171d", "module Child(in a: Bit, out q: Bit) { }\nmodule Top() { signal s: Bit; let c = Child(s); }\n", "Top");
    assert_eq!(few.first("E0312").message, "`Child` has 2 port(s), found 1 connection(s)");
    let many = Faulty::elaborating("errors~4bc8171d", "module Child(in a: Bit) { }\nmodule Top() { signal s: Bit; signal t: Bit; let c = Child(s, t); }\n", "Top");
    assert_eq!(many.first("E0312").message, "`Child` has 1 port(s), found 2 connection(s)");
}

#[test]
fn e0319_is_a_required_param_not_bound() {
    // errors~4bc80e1d: "E0319: A required param not bound"
    let source = "module R(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nmodule Top() { signal a: Electrical; let r1 = R(a, gnd); }\n";
    let faulty = Faulty::elaborating("errors~4bc80e1d", source, "Top");
    assert_eq!(faulty.first("E0319").message, "`Top.r1` leaves the required param `r` unbound");
}

#[test]
fn e0336_is_a_definition_that_reads_a_probe() {
    // errors~4bcf1b1d: "E0336: A definition reads a probe or `now()`"
    let probe = Faulty::elaborating("errors~4bcf1b1d", "module Top(p: Electrical, out s: Bit) { s = v(p) > 0.5; }\n", "Top");
    assert_eq!(probe.pointed_at("E0336"), "v(p) > 0.5");
    let clock = Faulty::elaborating("errors~4bcf1b1d", "module Top(out f: Bit<f64>) { f = now(); }\n", "Top");
    assert_eq!(clock.pointed_at("E0336"), "now()");
}

#[test]
fn e0339_is_a_combinational_loop_through_one_driver_carriers() {
    // errors~4bcf1a1d: "E0339: A combinational loop through one-driver carriers"
    let faulty = Faulty::elaborating("errors~4bcf1a1d", "module Top() { signal a: Bit; signal b: Bit; a = !b; b = !a; }\n", "Top");
    assert!(faulty.first("E0339").message.contains("undelayed definitions of `a`, `b` depend on each other"), "{}", faulty.first("E0339").message);
}

#[test]
fn e0350_is_an_instance_placed_where_none_is_placed() {
    // errors~4bd60d1d: "E0350: An instance placed in a `fn`, closure or reaction body"
    let reaction = Faulty::elaborating("errors~4bd60d1d", "module R(p: Electrical) { }\nmodule Top() { }\ndigital for Top { on at(1u) { let r = R(gnd); } }\n", "Top");
    assert_eq!(reaction.pointed_at("E0350"), "R(gnd)");
    let function = Faulty::elaborating("errors~4bd60d1d", "module R(p: Electrical) { }\nmodule Top() { }\nfn f() -> f64 { let r = R(gnd); 1.0 }\ndigital for Top { on at(1u) { let x = f(); } }\n", "Top");
    assert_eq!(function.first("E0350").message, "`R` is placed in a `fn` or a closure: it places no instance (§8.3.4)");
}

#[test]
fn e0404_is_a_statement_in_the_wrong_kind_of_block() {
    // errors~54e1b21d: "E0404: A statement in the wrong kind of block, like `<-` in `analog`"
    let faulty = Faulty::elaborating("errors~54e1b21d", "module Top() { signal q: Bit; }\nanalog for Top { q <- true; }\n", "Top");
    assert_eq!(faulty.pointed_at("E0404"), "q <- true;");
}

#[test]
fn e0414_is_not_raised_a_branch_that_switches_kind_is_refused_as_p0001() {
    // errors~54e5381d (unraised): "E0414: A branch switches kind on a probe". No source raises E0414
    // (GAPS.md section 10): a contribution under an `if` is `P0001` (L40), on both branches. When the
    // code is raised this test fails and the row becomes `holds`
    let source = "module Top() { signal p: Electrical; }\nanalog for Top { if v(p) > 0.5 { v(p) +> 1.0; } else { i(p) +> 1.0; } }\n";
    let faulty = Faulty::elaborating("errors~54e5381d", source, "Top");
    let codes: Vec<&str> = faulty.given.iter().map(|reported| reported.code.as_str()).collect();
    assert_eq!(codes, ["P0001", "P0001"], "no E0414: {codes:?}");
    assert_eq!(faulty.first("P0001").message, "a contribution under an `if` or in a closure is not supported yet");
}

#[test]
fn e0430_is_a_head_of_on_that_is_not_an_event() {
    // errors~54ec481d: "E0430: The head of `on` is not an `Event`"
    let faulty = Faulty::elaborating("errors~54ec481d", "module Top(in clk: Bit) { }\ndigital for Top { on clk { } }\n", "Top");
    assert_eq!(faulty.pointed_at("E0430"), "clk");
}

#[test]
fn e0171_is_a_manifest_key_the_format_does_not_allow() {
    // errors~39a2cc1d: "E0171: A manifest key the format does not allow"
    let model = Model::package("[package]\nname = \"b\"\nversion = \"0.1.0\"\nbogus = 1\n").with_source("src/lib.msl", "module Top() {}\n");
    let manifest = model.text_of("Piperine.toml");
    let given = Refusal::reported_by(model.run(Overrides::default()), "errors~39a2cc1d");
    assert_eq!(given[0].code, "E0171");
    assert_eq!(given[0].primary_text(&manifest), "bogus");
    assert_eq!(given[0].message, "unknown key `bogus` in `[package]`");
}

#[test]
fn e0175_is_a_package_without_a_crate_root() {
    // errors~39a2c81d: "E0175: A package without `src/lib.msl`"
    let model = Model::package("[package]\nname = \"a\"\nversion = \"0.1.0\"\n");
    let given = Refusal::reported_by(model.run(Overrides::default()), "errors~39a2c81d");
    assert_eq!(given[0].code, "E0175");
    assert!(given[0].message.starts_with("the package `a` has no crate root") && given[0].message.ends_with("src/lib.msl`"), "{}", given[0].message);
}

#[test]
fn e0250_is_a_carrier_at_a_node_port_or_the_reverse() {
    // errors~42ab6e1d: "E0250: A digital carrier connected to an analog port, or the reverse"
    let node_at_carrier = Faulty::elaborating("errors~42ab6e1d", "module Wide(in a: Bit<u10>, out q: Bit<u10>) { }\nmodule Top(p: Electrical) { signal wide: Bit<u10>; let c = Wide(p, wide); }\n", "Top");
    assert_eq!(node_at_carrier.first("E0250").message, "a node is connected to a port of a discrete nature (§6.1)");
    let carrier_at_node = Faulty::elaborating("errors~42ab6e1d", "module Node(p: Electrical) { }\nmodule Top(in b: Bit) { let c = Node(b); }\n", "Top");
    assert_eq!(carrier_at_node.pointed_at("E0250"), "b");
}

#[test]
fn e0310_is_a_bit_port_with_no_direction_or_a_direction_where_none_belongs() {
    // errors~4bc8151d: "E0310: A `Bit` port with no `in` or `out`, or a direction where none belongs"
    let bare = Faulty::elaborating("errors~4bc8151d", "module Top(clk: Bit) { }\n", "Top");
    assert_eq!(bare.pointed_at("E0310"), "Bit");
    let directed = Faulty::elaborating("errors~4bc8151d", "module Top(in p: Electrical) { }\n", "Top");
    assert_eq!(directed.first("E0310").message, "a port of a continuous nature takes no direction (§8.1.5)");
}

#[test]
fn e0317_is_a_param_value_that_fails_its_where() {
    // errors~4bc81c1d and errors~4263fe89: "E0317: A param value that fails its `where`"
    let source = "module R(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nmodule Top() { signal a: Electrical; let r1 = R(a, gnd) { r: -1.0 }; }\n";
    let faulty = Faulty::elaborating("errors~4bc81c1d", source, "Top");
    assert_eq!(faulty.pointed_at("E0317"), "-1.0");
    assert_eq!(faulty.first("E0317").message, "the `where` of the param `Top.r1.r` is false for the value -1.0");
}

#[test]
fn e0338_is_an_initial_value_under_an_undelayed_definition() {
    // errors~4bcf191d: "E0338: An initial value on a carrier driven by an undelayed definition"
    let faulty = Faulty::elaborating("errors~4bcf191d", "module Top(in a: Bit, in b: Bit, out y: Bit = false) { y = a & b; }\n", "Top");
    assert_eq!(faulty.pointed_at("E0338"), "y");
}

#[test]
fn e0343_is_an_item_of_a_child_that_is_not_pub() {
    // errors~4bd9101d and errors~e692f0c1: "E0343: `inst.name` reaches an item that is not `pub`"
    let faulty = Faulty::elaborating("errors~4bd9101d", "module Child() { signal q: Bit; }\nmodule Top() { let c = Child(); signal z: Bit; z = c.q; }\n", "Top");
    assert_eq!(faulty.pointed_at("E0343"), "q");
    assert_eq!(faulty.first("E0343").message, "`q` is not `pub` in module `Child`");
}

#[test]
fn e0410_is_a_left_side_of_a_contribution_that_is_not_a_probe() {
    // errors~54e53c1d: "E0410: The left side of `+>` is not a probe"
    let faulty = Faulty::elaborating("errors~54e53c1d", "module Top() { signal p: Electrical; }\nanalog for Top { 1.0 +> 2.0; }\n", "Top");
    assert_eq!(faulty.pointed_at("E0410"), "1.0");
}

#[test]
fn e0432_is_a_commit_of_the_wrong_type() {
    // errors~54ec4a1d: "E0432: A commit of the wrong type"
    let faulty = Faulty::elaborating("errors~54ec4a1d", "module Top(out q: Bit) { }\ndigital for Top { on at(1u) { q <- 1.0; } }\n", "Top");
    assert_eq!(faulty.pointed_at("E0432"), "1.0");
    assert_eq!(faulty.first("E0432").message, "mismatched types: expected `bool`, found `f64`");
}

#[test]
fn e0601_is_an_emit_outside_a_reaction_analog_or_fn() {
    // errors~64ee8d1d: "E0601: `emit!` outside a reaction, `analog` or a `fn`"
    let faulty = Faulty::elaborating("errors~64ee8d1d", "module Top() { let x = emit!(\"s\", f = 1.0); }\n", "Top");
    assert_eq!(faulty.pointed_at("E0601"), "emit!(\"s\", f = 1.0)");
}

#[test]
fn e0602_is_an_emit_without_a_literal_stream_name() {
    // errors~64ee8a1d: "E0602: `emit!` without a literal stream name, or with a field that is not `name = value`"
    let faulty = Faulty::elaborating("errors~64ee8a1d", "module Top() { }\ndigital for Top { on at(1u) { emit!(1, a = 2); } }\n", "Top");
    assert_eq!(faulty.pointed_at("E0602"), "1");
    assert_eq!(faulty.first("E0602").message, "the first argument is not a string literal in `emit!`");
}

#[test]
fn e0603_is_a_check_reached_from_analog() {
    // errors~64ee8b1d: "E0603: A check or `finish` reached from `analog`"
    let faulty = Faulty::elaborating("errors~64ee8b1d", "module Top() { signal p: Electrical; }\nanalog for Top { v(p) +> 1.0; require(\"x\", true); }\n", "Top");
    assert_eq!(faulty.pointed_at("E0603"), "require(\"x\", true)");
}

/// The overrides of a transient to `stop` with the step capped at `max_step` on the top `Top`.
fn transient_of_top(stop: f64, max_step: Option<f64>) -> Overrides {
    Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(stop), max_step, ..Overrides::default() }
}

/// The resistor and the source every run-time fixture uses.
const CIRCUIT_PARTS: &str = "module R(p: Electrical, n: Electrical) { r: f64 = 1000.0; }\nanalog for R { i(p, n) +> v(p, n) / r; }\nmodule V(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for V { v(p, n) +> dc; }\n";

#[test]
fn e0501_is_microsteps_that_never_go_quiet() {
    // errors~5c44f01d: "E0501: The microsteps at one time never go quiet" (a feedback loop of commits
    // with no delay); the report names the instance and the instant
    let source = "module Top() { signal a: Bit = false; }\ndigital for Top { on at(1u) { a <- true; } on a.changed() { a <- !a; } }\n";
    let faulty = Faulty::running("errors~5c44f01d", source, transient_of_top(3e-6, None));
    let run = faulty.first("E0501");
    assert!(run.message.contains("did not go quiet") && run.message.contains("at tick 0") && run.message.contains("Top.a"), "{}", run.message);
}

#[test]
fn e0503_is_a_singular_solve_that_names_the_node_or_the_branches() {
    // errors~5c44f21d: "E0503: A solve is singular; the report names the node or branches" (a node
    // with no DC path, or a loop of voltage sources)
    let floating = format!("{CIRCUIT_PARTS}module Isrc(p: Electrical, n: Electrical) {{ dc: f64 = 1m; }}\nanalog for Isrc {{ i(p, n) +> dc; }}\nmodule Top() {{ signal a: Electrical; let s = Isrc(a, gnd); }}\n");
    let node = Faulty::elaborating("errors~5c44f21d", &floating, "Top");
    assert!(node.first("E0503").message.contains("the Jacobian is singular") && node.first("E0503").message.ends_with("node of Top.s.p"), "{}", node.first("E0503").message);
    let loop_of_sources = format!("{CIRCUIT_PARTS}module Top() {{ signal a: Electrical; let s1 = V(a, gnd) {{ dc: 1.0 }}; let s2 = V(a, gnd) {{ dc: 2.0 }}; }}\n");
    let branches = Faulty::elaborating("errors~5c44f21d", &loop_of_sources, "Top");
    assert!(branches.first("E0503").message.ends_with("flow of Top.s1 (b0.flow), flow of Top.s2 (b0.flow)"), "{}", branches.first("E0503").message);
}

#[test]
fn e0504_is_a_step_after_a_digital_change_that_does_not_converge() {
    // errors~5c44ed1d: "E0504: The step after a digital change does not converge" (an ideal edge into
    // a circuit with no solution after it): the report names the carrier whose delivery the step reads
    let source = format!("{CIRCUIT_PARTS}module Chatter(p: Electrical, n: Electrical, in go: Bit) {{ }}\nanalog for Chatter {{ i(p, n) +> if go.high() {{ if v(p, n) > 0.5 {{ 1e-3 }} else {{ -1e-3 }} }} else {{ 0.0 }}; }}\nmodule Top() {{ signal n: Electrical; signal go: Bit = false; let r1 = R(n, gnd); let c = Chatter(n, gnd, go); }}\ndigital for Top {{ on at(5e-7) {{ go <- true; }} }}\n");
    let faulty = Faulty::running("errors~5c44ed1d", &source, transient_of_top(2e-6, Some(1e-8)));
    let run = faulty.first("E0504");
    assert!(run.message.contains("it reads the delivery of Top.go") && run.message.contains("at t = 5.0000000"), "{}", run.message);
}

#[test]
fn e0505_is_an_operating_point_that_does_not_converge() {
    // errors~5c44ec1d: "E0505: The operating point does not converge" (a device that is +1 mA above
    // 0.5 V and -1 mA below it, behind 1 kOhm, has no equilibrium)
    let source = format!("{CIRCUIT_PARTS}module Chatter(p: Electrical, n: Electrical) {{ }}\nanalog for Chatter {{ i(p, n) +> if v(p, n) > 0.5 {{ 1e-3 }} else {{ -1e-3 }}; }}\nmodule Top() {{ signal n: Electrical; let r1 = R(n, gnd); let c = Chatter(n, gnd); }}\n");
    let faulty = Faulty::elaborating("errors~5c44ec1d", &source, "Top");
    assert!(faulty.first("E0505").message.contains("Newton failed to converge after 500 iterations") && faulty.first("E0505").message.ends_with("node of Top.r1.p, Top.c.p"), "{}", faulty.first("E0505").message);
}

#[test]
fn e0507_is_a_non_finite_value_in_a_solve() {
    // errors~5c44ee1d: "E0507: A non-finite value in a solve" (a division by a quantity that reached
    // zero): the report names the instance
    let source = format!("{CIRCUIT_PARTS}module D(p: Electrical, n: Electrical) {{ }}\nanalog for D {{ i(p, n) +> 1.0 / v(p, n); }}\nmodule Top() {{ signal a: Electrical; let s = V(a, gnd) {{ dc: 0.0 }}; let d = D(a, gnd); }}\n");
    let faulty = Faulty::elaborating("errors~5c44ee1d", &source, "Top");
    assert!(faulty.first("E0507").message.contains("instance `Top.d`: term `b0.flow`") && faulty.first("E0507").message.contains("is not finite: inf"), "{}", faulty.first("E0507").message);
}

#[test]
fn e0508_is_a_transient_step_that_cannot_be_completed_at_the_smallest_step() {
    // errors~5c44e91d: "E0508: A transient step cannot be completed at the smallest step" (a
    // discontinuity in an `analog` expression): the report names the instant
    let source = format!("{CIRCUIT_PARTS}module Chatter(p: Electrical, n: Electrical) {{ from_time: f64 = 1e-6; }}\nanalog for Chatter {{ i(p, n) +> if now() > from_time {{ if v(p, n) > 0.5 {{ 1e-3 }} else {{ -1e-3 }} }} else {{ 0.0 }}; }}\nmodule Top() {{ signal n: Electrical; let r1 = R(n, gnd); let c = Chatter(n, gnd); }}\n");
    let faulty = Faulty::running("errors~5c44e91d", &source, transient_of_top(2e-6, Some(1e-8)));
    let run = faulty.first("E0508");
    assert!(run.message.contains("the step fails at the smallest step") && run.message.contains("at t = 1.0000000"), "{}", run.message);
}
