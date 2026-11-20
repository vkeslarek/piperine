//! Scope: chapter 12 of the Book, "The object model" (feature `root-e2e`, RE-02 and RE-03): the `Front`
//! comparator the chapter walks through, run through `piperine::`, and the statements the chapter makes
//! about what building a design gives (it always builds, a syntax error goes to `diagnostics()`, a
//! generation `for` stays one `for`, a later stage raises the type errors), asserted on the object
//! model of `piperine_lang::pom`. Each test opens with the inventory ids it holds, from
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

use piperine_lang::pom::Design;
use support::chapter::Chapter;
use support::faulty::Faulty;

/// The designs the chapter's sentences are about, written as the smallest source that makes the point.
struct Sources;

impl Sources {
    /// A source with a syntax error: a module whose parameter list never closes.
    const BROKEN: &'static str = "module Broken(a: Electrical {\n";

    /// A module that places one resistor in a generation `for` over eight cells.
    const EIGHT_CELLS: &'static str = "module R(p: Electrical, n: Electrical) { r: f64 = 1k; }\nmodule Chain() {\n    signal n: Electrical;\n    for i in 0..8 { let r = R(n, gnd); }\n}\n";

    /// A module whose commit has the wrong type: a type error, which the object model does not report.
    const WRONG_TYPE: &'static str = "module Top(out q: Bit) { }\ndigital for Top { on at(1u) { q <- 1.0; } }\n";

    /// The design the builder makes of `text`, as the one file `m.msl`.
    fn design_of(text: &str) -> Design {
        Design::builder().source("m.msl", text).build()
    }
}

#[test]
fn front_runs_with_its_analog_and_digital_events() {
    // pom#2 (run): `Front`, a comparator with hysteresis that drives a flip-flop, placed in a top with
    // a 1 V source on its input and run for an operating point
    let chapter = Chapter::read();
    let after = "module Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\nmodule Top() { signal vin: Electrical; signal clk: Bit = false; signal q: Bit; let s = Vdc(vin); let f = Front(vin, clk, q); }\n";
    let ran = chapter.wrapped("front.msl", "", "pom#2", after).run(Faulty::operating_point_of("Top")).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.signal("Top.vin"), 1.0, "the source's volt reaches the comparator's input");
}

#[test]
fn building_always_gives_a_design_and_a_syntax_error_goes_to_its_diagnostics() {
    // pom~d65bb809 "Always builds. build() returns a Design however broken the input."; pom~27751c3c
    // "build() fails on a syntax error." and its answer pom~806e9e4f "It always returns a Design."; pom~dfd54aa0
    // "Check diagnostics() before you trust the answers."
    let design = Sources::design_of(Sources::BROKEN);
    assert!(!design.diagnostics().is_empty(), "the parse error is a diagnostic of the design");
    assert!(design.diagnostics().iter().all(|found| !found.diagnostic.code.is_empty()), "each carries its code");
    let sound = Sources::design_of("module Fine() { }\n");
    assert!(sound.diagnostics().is_empty(), "a sound source gives none: {:?}", sound.diagnostics().iter().map(|found| &found.diagnostic.message).collect::<Vec<_>>());
    assert!(sound.module("Fine").is_some(), "and its module is there");
}

#[test]
fn a_generation_for_over_eight_cells_stays_one_for_with_one_placement() {
    // pom~3bce7c96 "A generation for over 8 cells gives 8 instances to walk."; pom~f5af2797 "It gives one
    // for with one placement inside, as written."
    let design = Sources::design_of(Sources::EIGHT_CELLS);
    assert!(design.diagnostics().is_empty(), "the source is sound: {:?}", design.diagnostics().iter().map(|found| &found.diagnostic.message).collect::<Vec<_>>());
    let chain = design.module("Chain").expect("the module Chain");
    let generations = chain.generations();
    assert_eq!(generations.len(), 1, "one generation");
    assert!(generations[0].is_for(), "and it is a for");
    assert_eq!(generations[0].items().len(), 1, "with one placement inside, as written");
    assert_eq!(chain.instances().len(), 1, "the loop body is listed once, not eight times");
}

#[test]
fn a_type_error_is_not_the_object_models_to_report() {
    // pom~20d38737 "An E0432 or E0417 comes from a later stage.": the object model resolves names and
    // gives no type error; the run of the same source does
    let design = Sources::design_of(Sources::WRONG_TYPE);
    assert!(design.diagnostics().iter().all(|found| found.diagnostic.code != "E0432" && found.diagnostic.code != "E0417"), "the object model gives no type error");
    let faulty = Faulty::elaborating("pom~20d38737", Sources::WRONG_TYPE, "Top");
    assert_eq!(faulty.first("E0432").message, "mismatched types: expected `bool`, found `f64`");
}
