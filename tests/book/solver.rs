//! Scope: chapter 14 of the Book, "The solver" (feature `root-e2e`, RE-03): the statements the chapter
//! makes about what the solver does and reports, asserted through `piperine::` on the smallest design
//! that shows each (an instant that never goes quiet, an operating point that never settles, a step
//! that lands on a digital instant, a loop of undelayed definitions) and on the manifests of the two
//! sides of the solver. The sentences that describe the algorithm (the homotopy steps, the rollback
//! of a rejected step, the purpose of the chapter) are `prose` rows: no value of a run states them. The
//! chapter has no block. Each test opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod faulty;
    pub mod inventory;
    pub mod manifests;
    pub mod model;
    pub mod records;
    pub mod refusal;
}

use piperine::Cell;
use support::faulty::Faulty;
use support::manifests::Manifests;
use support::model::Model;

/// A top whose analog node follows a digital bit through a `transition`: the smallest crossing of the
/// two sides.
struct Crossing;

impl Crossing {
    /// The analog side: node `n` takes 1 V when `d` is high, 0 V otherwise.
    const ANALOG: &'static str = "module Top() { signal n: Electrical; signal d: Bit = false; }\nanalog for Top { v(n) +> transition(if d.high() { 1.0 } else { 0.0 }); }\n";
}

#[test]
fn the_two_sides_of_the_solver_never_depend_on_each_other() {
    // solver~83cb975b "The analog and digital solvers never call each other: everything that crosses between
    // them is an event the owner carries.": neither crate lists the other, nor the owner above them
    for (side, other) in [("piperine-analog-solver", "piperine-digital-solver"), ("piperine-digital-solver", "piperine-analog-solver")] {
        assert!(!Manifests::depends_on(side, other), "{side} lists {other}");
        assert!(!Manifests::depends_on(side, "piperine-solver"), "{side} lists the owner of time");
    }
    assert!(Manifests::depends_on("piperine-solver", "piperine-analog-solver") && Manifests::depends_on("piperine-solver", "piperine-digital-solver"), "the owner carries both");
}

#[test]
fn an_operating_point_that_never_settles_stops_after_20_rounds_with_e0506_naming_its_nets() {
    // simulation~5c44ef1d "E0506"; solver~53123473 "After 20 rounds that still change something the solver stops with E0506 and names
    // the nets that kept moving, rather than return a wrong answer.": a comparator that inverts the
    // level it reads never settles
    let source = format!("{}digital for Top {{ on above(v(n) - 0.5) {{ d <- false; }} on above(0.5 - v(n)) {{ d <- true; }} }}\n", Crossing::ANALOG);
    let failure = Model::written("m.msl", &source).run(Faulty::operating_point_of("Top")).err().expect("the operating point does not settle");
    let said = format!("{failure:#}");
    assert!(said.contains("E0506: the operating point did not settle in 20 rounds; nets still changing: "), "{said}");
    assert!(said.contains("Top.transition$0.x"), "the nets that kept moving are named: {said}");
}

#[test]
fn an_instant_that_never_goes_quiet_stops_after_1000_microsteps_with_e0501_naming_its_nets() {
    // solver~dbaf149d "After 1000 microsteps the solver stops with E0501 and names the nets that keep
    // changing.": a bit that inverts itself every time it changes
    let source = "module Top() { signal x: Bit = false; }\ndigital for Top { on x.changed() { x <- !x; } on at(1u) { x <- true; } }\n";
    let failure = Model::written("m.msl", source).run(Faulty::transient_of_top(3e-6, None)).err().expect("the instant does not go quiet");
    let said = format!("{failure:#}");
    assert!(said.contains("E0501: the instant at tick 0 did not go quiet; still changing: Top.x (the bound is 1000 microsteps)"), "{said}");
}

#[test]
fn a_step_lands_exactly_on_a_digital_instant_and_what_it_changed_reaches_the_analog_side() {
    // solver~ce518374 "It stops at the first instant that touches the analog side, and the step is shortened
    // to land exactly there."; solver~c2aa698e "What the instant changed reaches the analog side.": a bit
    // that rises at 1.234567 us moves the node from 0 V to 1 V, and the table has the row of that instant
    let source = format!("{}digital for Top {{ on at(1.234567u) {{ d <- true; }} }}\n", Crossing::ANALOG);
    let ran = Model::written("m.msl", &source).run(Faulty::transient_of_top(3e-6, None)).unwrap_or_else(|failure| panic!("{failure}"));
    let landed = (0..ran.table.rows.len()).find(|row| (ran.real(*row, "time") - 1.234567e-6).abs() < 1e-12).expect("a row at the instant of the edge");
    assert_eq!(ran.cell(landed - 1, "Top.d"), &Cell::Bool(false), "the bit was low on the row before the instant");
    assert_eq!(ran.cell(landed, "Top.d"), &Cell::Bool(true), "and is high on the row that lands exactly on it");
    assert_eq!(ran.cell(landed, "Top.n"), &Cell::Real(0.0), "the analog side was solved on the values the digital side had just before the instant");
    assert_eq!(ran.cell(landed + 1, "Top.n"), &Cell::Real(1.0), "and sees what the instant changed on the next step");
}

#[test]
fn a_loop_of_undelayed_definitions_is_refused_when_the_design_is_built_naming_its_functions() {
    // solver~4642dd47 "A loop of functions is accepted only if it passes through a net marked to relax, which
    // is iterated until it stops changing. Any other loop is refused when the design is built, naming its
    // functions.": no source form marks a net to relax, so the refusal of the other loop is what a source
    // shows
    let faulty = Faulty::elaborating("solver~4642dd47", "module Top() { signal a: Bit; signal b: Bit; a = !b; b = !a; }\n", "Top");
    assert!(faulty.first("E0339").message.contains("undelayed definitions of `a`, `b` depend on each other"), "{}", faulty.first("E0339").message);
}
