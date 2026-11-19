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
use support::refusal::Refusal;

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
    let model = Model::example("rc")
        .with_source("Piperine.toml", "\n[dependencies]\nutil = { path = \"deps/util\" }\n")
        .with_source("deps/util/Piperine.toml", "[package]\nname = \"util\"\nversion = \"0.1.0\"\n")
        .with_source("deps/util/src/lib.msl", "pub mod cells;\n")
        .with_source("deps/util/src/cells.msl", "pub module Cell(p: Electrical) {}\n")
        .with_source("src/util.msl", "pub module Other(p: Electrical) {}\n")
        .with_source("src/lib.msl", "mod util;\nuse util::cells::Cell;\n");
    let source = model.text_of("src/lib.msl");
    let given = Refusal::reported_by(model.run_named("op", Overrides::default()), "errors#2");
    let ambiguous = given.iter().find(|reported| reported.code == "E0163").expect("E0163 is given");
    assert_eq!(ambiguous.message, "`util` names both a dependency and an item of this namespace");
    assert_eq!(ambiguous.primary_text(&source), "util::cells::Cell", "the label points at the path, not at its first segment");
    assert_eq!(ambiguous.primary_label, "ambiguous name");
    assert!(ambiguous.help.as_deref().is_some_and(|help| help.contains("package = \"util\"") && help.contains("crate::util")), "{:?}", ambiguous.help);
    assert!(ambiguous.secondary_labels.is_empty(), "errors#2 diverges: the Book shows a secondary label on `mod util;`, the tool gives none");
}
