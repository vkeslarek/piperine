//! Scope: chapter 9 of the Book, "Digital behavior" (feature `root-e2e`, RE-02 and RE-03): the counter
//! and its 257 `state` records, the set-reset latch, the multiply-accumulate pipeline, the debouncer
//! and the PWM (their integer params are refused today, asserted refused; their numbers are held on the
//! same designs with the param written as a literal), the checks and `finish()`, the monitors, and the
//! eight mistakes of the chapter's table, run through `piperine::`. Each test opens with the inventory
//! ids it holds, from `tests/book/inventory.tsv`.

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
    pub mod standard_library;
}

use piperine::{Cell, Field, Overrides, Record};
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::{Model, Ran};
use support::printed::Printed;
use support::records::Fields;
use support::refusal::Refusal;
use support::standard_library::StandardLibrary;

/// The Book's text with a piece replaced, for a design the compiler refuses written the way it accepts.
struct Edit;

impl Edit {
    /// `text` with `from` replaced by `to`. Panics naming `from` when the text does not hold it, so an
    /// edit that no longer applies to the Book fails by name instead of running another text.
    fn replace(text: &str, from: &str, to: &str) -> String {
        assert!(text.contains(from), "the Book's text no longer holds `{from}`");
        text.replace(from, to)
    }
}

/// The clock of chapter 5 that the counter, the pipeline and the benches of this chapter use.
struct Oscillator;

impl Oscillator {
    /// The `Osc` module and its behavior, the part of the chapter 5 listing before its counter.
    fn text(chapter: &Chapter) -> String {
        let listing = chapter.text("values#1");
        let (clock, _) = listing.split_once("/// Counts rising edges").expect("the listing of chapter 5 declares the clock first");
        assert!(clock.contains("module Osc"), "{clock}");
        clock.to_string()
    }
}

/// The debouncer of the chapter with its two refused forms written as the compiler accepts them: the
/// param `n` as the literal 4, (the button is a plain bool).
struct Debouncer;

impl Debouncer {
    /// The debouncer's file with `n` as a literal and `bench` in place of the
    /// bench's timed reactions.
    fn accepted(chapter: &Chapter, bench: &str) -> Model {
        let text = chapter.text("digital#7");
        let text = Edit::replace(&text, "    n: u8 = 4 where n >= 1;\n", "");
        let text = Edit::replace(&text, "run + 1 >= n", "run + 1 >= 4");
        let text = Edit::replace(&text, " { n: 4 }", "");
                let text = Edit::replace(&text, "    on at(2.0m) { button <- true; }\n    on at(2.2m) { button <- false; }\n    on at(2.4m) { button <- true; }\n", bench);
        Model::written("debounce.msl", &text)
    }

    /// The bench of the Book: the button bounces at 2.0 ms and 2.2 ms and goes high for good at 2.4 ms.
    const BOUNCING_BENCH: &'static str = "    on at(2.0m) { button <- true; }\n    on at(2.2m) { button <- false; }\n    on at(2.4m) { button <- true; }\n";

    /// The bench of the exercise: the button goes high at 2.0 ms and low again at 4.0 ms.
    const SHORT_PRESS_BENCH: &'static str = "    on at(2.0m) { button <- true; }\n    on at(4.0m) { button <- false; }\n";

    /// A transient to 7 ms keeping the stream `stream`.
    fn run(model: &Model, stream: &str) -> Ran {
        let overrides = Overrides { analysis: Some("transient".to_string()), stop: Some(7e-3), streams: vec![stream.to_string()], ..Overrides::default() };
        model.run(overrides).unwrap_or_else(|failure| panic!("{failure}"))
    }
}

/// The PWM of the chapter with its integer param written as the literal 100, and its checker.
struct Generator;

impl Generator {
    /// The PWM file with `period` as a literal; `with_check` places the checker (also with its two
    /// integer params as literals) beside the generator.
    fn accepted(chapter: &Chapter, with_check: bool) -> Model {
        let text = chapter.text("digital#11");
        let text = Edit::replace(&text, "    period: u8 = 100 where period >= 2;\n", "");
        let text = Edit::replace(&text, "count == period - 1", "count == 99");
        let mut text = Edit::replace(&text, " { period: 100 }", "");
        if with_check {
            text = Edit::replace(&text, "let pulse = Pwm(clk, duty, pwm, wrap);", "let pulse = Pwm(clk, duty, pwm, wrap);\n    let chk = PwmCheck(clk, wrap, pwm);");
            let check = chapter.text("digital#14");
            let check = Edit::replace(&check, "    limit: u8 = 90;\n    cycles: u32 = 20;\n", "");
            let check = Edit::replace(&check, "high <= limit", "high <= 90");
            let check = Edit::replace(&check, "seen + 1 >= cycles", "seen + 1 >= 20");
            text = format!("{text}\n{check}");
        }
        Model::written("pwm.msl", &text)
    }
}

/// The runs the chapter's commands make.
struct Commands;

impl Commands {
    /// A transient to `stop`, as `piperine run <file> --analysis transient --stop <stop>`.
    fn transient(stop: f64) -> Overrides {
        Overrides { analysis: Some("transient".to_string()), stop: Some(stop), ..Overrides::default() }
    }

    /// A transient of the top `top` to `stop`.
    fn transient_of(top: &str, stop: f64) -> Overrides {
        Overrides { top: Some(top.to_string()), ..Self::transient(stop) }
    }
}

/// What a test reads out of a run's table.
struct Table;

impl Table {
    /// The text of the cell of `row` under `column`.
    fn text(ran: &Ran, row: usize, column: &str) -> String {
        format!("{:?}", ran.cell(row, column))
    }

    /// The value of the bit-vector cell of `row` under `column` as an integer.
    fn integer(ran: &Ran, row: usize, column: &str) -> i128 {
        match ran.cell(row, column) {
            Cell::Int(value) => *value,
            other => panic!("row {row}, column `{column}`: an integer, found {other:?}"),
        }
    }
}

/// The counter of the chapter, with the position of its `emit!` moved.
struct Counter;

impl Counter {
    /// The `emit!` line of the counter's reaction.
    const EMIT: &'static str = "        emit!(\"state\", t = now(), count = count, full = full);\n";

    /// The counter and its bench `Count` with a clock, `emit_after` lines of the other two statements
    /// before the `emit!`: 0 is the Book's text (first), 1 the middle, 2 last.
    fn with_emit_after(chapter: &Chapter, emit_after: usize) -> Model {
        let counter = chapter.text("digital#1");
        let without = Edit::replace(&counter, Self::EMIT, "");
        let mut lines: Vec<&str> = without.lines().collect();
        let reaction = lines.iter().position(|line| line.contains("on clk.rise()")).expect("the reaction") + 1;
        lines.insert(reaction + emit_after, Self::EMIT.trim_end_matches('\n'));
        let moved = lines.join("\n");
        Model::written("count.msl", &format!("{}\n{moved}\n{}\n", Oscillator::text(chapter), chapter.text("digital#3")))
    }

    /// The `state` records of a run of `model` to 257 us.
    fn states(model: &Model) -> Vec<(f64, Vec<(String, Field)>)> {
        let ran = model.run(Commands::transient(257e-6)).unwrap_or_else(|failure| panic!("{failure}"));
        ran.stream("state").iter().map(|record| (record.time, record.fields.clone())).collect()
    }
}

#[test]
fn the_counter_raises_full_exactly_while_the_count_is_255() {
    // digital#1 and digital#3 (run): the counter and its bench with the clock of chapter 5; digital#2 (the
    // listing of the stream, shown); digital~d8ecede0 "one emit!(\"state\", ...) at the top of a clocked
    // reaction gives you the whole history of the block, one row per edge"; digital~6406776a "A transient
    // of a design with no analog node runs to its stop like any other and gives its table". The 257 records
    // are one per rising edge, exactly one has `full` true, the one at 255.5 us, and the Book's rows at 254.5
    // us (count 254, full false), 255.5 us (255, true) and 256.5 us (0, false)
    let chapter = Chapter::read();
    let model = chapter.lone("count.msl", &["digital#1", "digital#3"]).with_source("count.msl", &Oscillator::text(&chapter));
    let overrides = Overrides { print: vec!["full".to_string()], ..Commands::transient(257e-6) };
    let ran = model.run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let states = ran.stream("state");
    assert_eq!(states.len(), 257, "one record per rising edge of the 1 MHz clock, at 0.5, 1.5, ... 256.5 us");
    let counts: Vec<&Field> = states.iter().map(|record| Fields::of(record, "count")).collect();
    for (edge, count) in counts.iter().enumerate() {
        assert_eq!(**count, Field::Int((edge % 256) as i128), "the state the reaction found at edge {edge}");
    }
    let full: Vec<&&Record> = states.iter().filter(|record| *Fields::of(record, "full") == Field::Bool(true)).collect();
    assert_eq!(full.len(), 1, "exactly one record has `full` true");
    Printed::holds("2.555000e-4", full[0].time, "the edge where count reads 255");
    assert_eq!(Fields::of(states[254], "full"), &Field::Bool(false), "(254.5 us, 0): count = 254, full = false");
    assert_eq!(Fields::of(states[256], "count"), &Field::Int(0), "(256.5 us, 0): count = 0");
    assert_eq!(ran.table.columns, ["time", "Count.full"], "the table of the run holds the printed signal");
    let last = ran.table.rows.len() - 1;
    Printed::holds("2.570000e-4", ran.real(last, "time"), "a transient of a design with no analog node runs to its stop");
}

#[test]
fn the_emit_prints_the_same_first_in_the_middle_or_last() {
    // digital~cc20763d: "Put the emit! first, last or in the middle: it prints the same thing, because
    // nothing changes while a reaction runs": the 257 records are equal for the three positions
    let chapter = Chapter::read();
    let first = Counter::states(&Counter::with_emit_after(&chapter, 0));
    assert_eq!(first.len(), 257);
    assert_eq!(Counter::states(&Counter::with_emit_after(&chapter, 1)), first, "the middle");
    assert_eq!(Counter::states(&Counter::with_emit_after(&chapter, 2)), first, "last");
}

#[test]
fn the_set_reset_latch_needs_no_reaction() {
    // digital#5 (run): `SrLatch`, two cross-coupled NOR definitions with a gate delay: "A set-reset latch
    // needs no reaction at all". A pulse on `s` sets it (q high, qn low), the output holds when `s` falls,
    // and a pulse on `r` resets it
    let chapter = Chapter::read();
    let after = "module Top() { signal s: Bit = false; signal r: Bit = false; signal q: Bit; signal qn: Bit; let l = SrLatch(s, r, q, qn); }\ndigital for Top { on at(1u) { s <- true; } on at(2u) { s <- false; } on at(3u) { r <- true; } on at(4u) { r <- false; } }\n";
    let overrides = Overrides { print_step: Some(5e-7), ..Commands::transient_of("Top", 6e-6) };
    let ran = chapter.wrapped("latch.msl", "", "digital#5", after).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let at = |microseconds: f64| ran.table.rows.iter().position(|row| matches!(row[0], Cell::Real(time) if (time - microseconds * 1e-6).abs() < 1e-9)).unwrap_or_else(|| panic!("no row at {microseconds} us"));
    let levels = |microseconds: f64| (Table::text(&ran, at(microseconds), "Top.q"), Table::text(&ran, at(microseconds), "Top.qn"));
    assert_eq!(levels(0.5), ("Bool(false)".to_string(), "Bool(true)".to_string()), "reset to begin with");
    assert_eq!(levels(1.5), ("Bool(true)".to_string(), "Bool(false)".to_string()), "set by the pulse on s");
    assert_eq!(levels(2.5), ("Bool(true)".to_string(), "Bool(false)".to_string()), "held after s falls");
    assert_eq!(levels(3.5), ("Bool(false)".to_string(), "Bool(true)".to_string()), "reset by the pulse on r");
}

#[test]
fn e0339_a_loop_of_definitions_needs_a_delay() {
    // digital~4bcf1a1d: "E0339 q = !(r | qn); qn = !(s | q);": a loop of definitions with no delay in it
    let source = "module Top() { signal s: Bit = false; signal r: Bit = false; signal q: Bit; signal qn: Bit; q = !(r | qn); qn = !(s | q); }\n";
    let faulty = Faulty::elaborating("digital~4bcf1a1d", source, "Top");
    assert!(faulty.first("E0339").message.starts_with("undelayed definitions of `q`, `qn` depend on each other"), "{}", faulty.first("E0339").message);
    assert_eq!(faulty.given.len(), 1, "{:?}", faulty.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
}

#[test]
fn without_its_delay_the_book_latch_is_a_loop_e0339() {
    // digital~ce04e7a4: "Without the after, the two definitions would form a loop with no delay in it, which
    // is an elaboration error (E0339)": the latch of digital#5 with its `after tpd` removed gives the loop,
    // beside an `E0338` for each initial value that sits under an undelayed definition
    let chapter = Chapter::read();
    let latch = Edit::replace(&chapter.text("digital#5"), " after tpd", "");
    let source = format!("{latch}\nmodule Top() {{ signal s: Bit = false; signal r: Bit = false; signal q: Bit; signal qn: Bit; let l = SrLatch(s, r, q, qn); }}\n");
    let faulty = Faulty::elaborating("digital~ce04e7a4", &source, "Top");
    let mut codes: Vec<&str> = faulty.given.iter().map(|reported| reported.code.as_str()).collect();
    codes.sort_unstable();
    assert_eq!(codes, ["E0338", "E0338", "E0339"], "the loop, and one initial value for `q` and one for `qn`");
    assert!(faulty.first("E0339").message.contains("`q`, `qn`"), "{}", faulty.first("E0339").message);
}

#[test]
fn the_phase_detector_runs_assignment_and_events_read_as_bools() {
    // digital#6 (run): `Pfd3`: `s += 1` on a local of a reaction (every local may be assigned, composite-values T11)
    // and `fref.rise()` tested as a `bool` (§13 rule 2, unknown-semantics-events T14). A rising `fref` at 1 us
    // raises `up`, a rising `fvco` at 2 us brings the state back, and both rising at 3 us cancel
    let chapter = Chapter::read();
    let after = "module Top() { signal a: Bit = false; signal b: Bit = false; signal up: Bit; signal dn: Bit; let p = Pfd3(a, b, up, dn); }\ndigital for Top { on at(1u) { a <- true; } on at(2u) { b <- true; } on at(2.2u) { a <- false; b <- false; } on at(3u) { a <- true; b <- true; } }\n";
    let overrides = Overrides { print_step: Some(5e-7), ..Commands::transient_of("Top", 4e-6) };
    let ran = chapter.wrapped("pfd.msl", "", "digital#6", after).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let at = |microseconds: f64| ran.table.rows.iter().position(|row| matches!(row[0], Cell::Real(time) if (time - microseconds * 1e-6).abs() < 1e-9)).unwrap_or_else(|| panic!("no row at {microseconds} us"));
    let levels = |microseconds: f64| (Table::text(&ran, at(microseconds), "Top.up"), Table::text(&ran, at(microseconds), "Top.dn"));
    assert_eq!(levels(0.5), ("Bool(false)".to_string(), "Bool(false)".to_string()), "no edge yet");
    assert_eq!(levels(1.5), ("Bool(true)".to_string(), "Bool(false)".to_string()), "fref first: up");
    assert_eq!(levels(2.5), ("Bool(false)".to_string(), "Bool(false)".to_string()), "fvco next: back to zero");
    assert_eq!(levels(3.5), ("Bool(false)".to_string(), "Bool(false)".to_string()), "both at once: up first, then down");
}

#[test]
fn the_debouncer_is_refused_for_its_integer_param_alone() {
    // digital#7 (refused): the debouncer's file (`debounce.msl`); digital#10 holds its listing;
    // digital~bc5825b1 "One thing in it is not compiled yet, a param of an integer type (n: u8), so today
    // the command refuses the file there and runs nothing"; digital~d7032a06 (the same form);
    // digital~d1d69398 "That stops the debouncer and the PWM for now ...; the counter runs" (the counter:
    // the test of digital#1). The form is `P0001` (L28); with the param written as a literal the file
    // runs
    let chapter = Chapter::read();
    let overrides = Overrides { analysis: Some("transient".to_string()), stop: Some(7e-3), streams: vec!["flip".to_string()], ..Overrides::default() };
    let model = chapter.marked("debounce.msl", "digital#7");
    Refusal::expect(model.run(overrides.clone()), "digital#7", "P0001", "param of type `u8` is not supported yet");
    let text = chapter.text("digital#7");
    let literal = Edit::replace(&Edit::replace(&Edit::replace(&text, "    n: u8 = 4 where n >= 1;\n", ""), "run + 1 >= n", "run + 1 >= 4"), " { n: 4 }", "");
    Model::written("debounce.msl", &literal).run(overrides).unwrap_or_else(|failure| panic!("the file runs with `n` as a literal: {failure:#}"));
}

#[test]
fn the_debouncer_flips_once_at_the_fourth_clean_sample() {
    // The debouncer of digital#7 with `n` as the literal 4 (digital#7 refuses the
    // Book's text): digital~6733ba9f "Once it runs, it prints one record line: the output flipped once, at
    // the fourth clean sample, @5.500000e-3 Press.db flip t=5.500000e-3 to=true"; digital~9d7f8cfe "its three
    // timed reactions are the stimulus: the button starts low, bounces high at 2.0 ms, low at 2.2 ms, and
    // high for good at 2.4 ms"; digital~7f3906df "The clock is 1 kHz, so it samples at 0.5, 1.5, 2.5 ms and
    // so on, and never sees the bounce"; the `state` rows of digital#8
    let chapter = Chapter::read();
    let model = Debouncer::accepted(&chapter, Debouncer::BOUNCING_BENCH);
    let flips = Debouncer::run(&model, "flip");
    let flip = flips.stream("flip");
    assert_eq!(flip.len(), 1, "the output flipped once");
    assert_eq!(flip[0].instance, "Press.db");
    Printed::holds("5.500000e-3", flip[0].time, "the fourth clean sample");
    assert_eq!(Fields::of(flip[0], "to"), &Field::Bool(true));
    let states = Debouncer::run(&model, "state");
    let rows: Vec<(f64, i128, bool, bool)> = states
        .stream("state")
        .iter()
        .map(|record| {
            let level = |name: &str| *Fields::of(record, name) == Field::Bool(true);
            (record.time, match Fields::of(record, "run") { Field::Int(run) => *run, other => panic!("{other:?}") }, level("raw"), level("clean"))
        })
        .collect();
    let expected = [(0.5e-3, 0, false, false), (1.5e-3, 0, false, false), (2.5e-3, 0, true, false), (3.5e-3, 1, true, false), (4.5e-3, 2, true, false), (5.5e-3, 3, true, false), (6.5e-3, 0, true, true)];
    assert_eq!(rows.len(), expected.len(), "one row per sample of the 1 kHz clock: {rows:?}");
    for (row, (time, run, raw, clean)) in rows.iter().zip(expected) {
        assert!((row.0 - time).abs() < 1e-9 && (row.1, row.2, row.3) == (run, raw, clean), "the Book's row at {time:e}: {row:?}");
    }
}

#[test]
fn a_press_released_before_the_fourth_sample_gives_no_flip() {
    // digital~5351d4c4 and digital~28c24505: "Change the bench of the debouncer so that button goes high at
    // 2.0 ms and low again at 4.0 ms. Write down the run column of the state stream up to 6.5 ms, and say
    // whether a flip record appears. (run reads 0, 0, 0, 1, 2, 0, 0 ... No flip.)" On the debouncer of the
    // test above
    let chapter = Chapter::read();
    let model = Debouncer::accepted(&chapter, Debouncer::SHORT_PRESS_BENCH);
    let states = Debouncer::run(&model, "state");
    let runs: Vec<&Field> = states.stream("state").iter().map(|record| Fields::of(record, "run")).collect();
    assert_eq!(runs, [&Field::Int(0), &Field::Int(0), &Field::Int(0), &Field::Int(1), &Field::Int(2), &Field::Int(0), &Field::Int(0)], "the run column up to 6.5 ms");
    assert!(Debouncer::run(&model, "flip").stream("flip").is_empty(), "no flip record appears");
}

#[test]
fn the_pwm_is_refused_for_its_integer_param() {
    // digital#11 (refused): the PWM's file (`pwm.msl`); digital#13 holds its listing; digital~bdd2508f "Its
    // period is a param of an integer type, which the compiler does not take yet, so today the command
    // refuses the file and runs nothing": `P0001` (L28)
    let chapter = Chapter::read();
    let overrides = Overrides { analysis: Some("transient".to_string()), stop: Some(450e-6), streams: vec!["cycle".to_string()], ..Overrides::default() };
    Refusal::expect(chapter.marked("pwm.msl", "digital#11").run(overrides), "digital#11", "P0001", "param of type `u8` is not supported yet");
}

#[test]
fn the_pwm_latches_its_duty_word_at_the_wrap() {
    // The PWM of digital#11 with `period` as the literal 100: digital~107c5906 "A counter that wraps every
    // period clocks, and an output that is high while the count is below a duty word"; digital~0727161d
    // "With a 1 MHz clock the PWM runs at 10 kHz, and the cycle stream has one record per period carrying
    // the duty word that period will use": the records at 99.5, 199.5, 299.5 and 399.5 us carry duty 25,
    // 25, 60 and 60, the change made at 250 us showing up at the next wrap
    let chapter = Chapter::read();
    let overrides = Overrides { streams: vec!["cycle".to_string()], ..Commands::transient(450e-6) };
    let ran = Generator::accepted(&chapter, false).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let cycles = ran.stream("cycle");
    assert_eq!(cycles.len(), 4, "one record per period of 100 us over 450 us");
    for ((record, instant), duty) in cycles.iter().zip(["9.950000e-5", "1.995000e-4", "2.995000e-4", "3.995000e-4"]).zip([25, 25, 60, 60]) {
        Printed::holds(instant, record.time, "the wrap");
        assert_eq!(Fields::of(record, "duty"), &Field::Int(duty), "the duty word of the cycle");
    }
    let period = cycles[1].time - cycles[0].time;
    assert!((1.0 / period - 10e3).abs() < 1.0, "a 1 MHz clock and a period of 100 clocks is 10 kHz: {period:e} s");
}

#[test]
fn the_pwm_check_is_refused_for_its_integer_params() {
    // digital#14 (refused): `PwmCheck`, with `limit: u8` and `cycles: u32`; digital~a0ca3f9d "With the check
    // placed in the bench, piperine run pwm.msl ... stops by itself at the twentieth cycle" (refused with
    // it: the bench does not run as the Book writes it): both params are `P0001` (L28)
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit; signal wrap: Bit; signal pwm: Bit; let c = PwmCheck(clk, wrap, pwm); }\n";
    let model = chapter.wrapped("check.msl", "", "digital#14", after);
    let given = Refusal::of(model.run(Commands::transient_of("Top", 1e-6)), "digital#14");
    for expected in ["param of type `u8` is not supported yet", "param of type `u32` is not supported yet"] {
        assert!(given.contains(&("P0001".to_string(), expected.to_string())), "{expected}: {given:?}");
    }
    Refusal::expect(model.run(Commands::transient_of("Top", 1e-6)), "digital#14", "P0001", "param of type `u32` is not supported yet");
}

#[test]
fn the_check_passes_every_cycle_and_ends_the_run_at_the_twentieth() {
    // The PWM bench of digital#11 with the checker of digital#14, their integer params written as literals:
    // the rest of digital~a0ca3f9d "prints each failing checks record as it is written (add --all-checks to
    // see the passing ones too) and stops by itself at the twentieth cycle": no failing record, twenty
    // passing `require` records, and a `sim` record `finish` at the twentieth wrap, 1900.5 us, where the
    // table ends. digital~aa0c04f9 "finish() writes to the reserved sim stream and ends the analysis at that
    // instant"
    let chapter = Chapter::read();
    let ran = Generator::accepted(&chapter, true).run(Commands::transient(3e-3)).unwrap_or_else(|failure| panic!("{failure}"));
    let checks = ran.stream("checks");
    assert_eq!(checks.len(), 20, "one `duty_limit` check per wrap, twenty cycles");
    assert!(checks.iter().all(|record| *Fields::of(record, "pass") == Field::Bool(true) && *Fields::of(record, "name") == Field::Text("duty_limit".to_string())), "no failing record: {checks:?}");
    let finish = ran.stream("sim");
    assert_eq!(finish.len(), 1, "one `sim` record");
    assert_eq!(Fields::of(finish[0], "finish"), &Field::Bool(true));
    Printed::holds("1.900500e-3", finish[0].time, "the twentieth wrap");
    let last = ran.table.rows.len() - 1;
    Printed::holds("1.900500e-3", ran.real(last, "time"), "the analysis ends at the instant of finish()");
}

#[test]
fn require_writes_to_checks_as_a_pass_or_a_failure() {
    // digital~aa0c04f9 (replaces `require_writes_to_checks_and_an_unknown_pass_is_unknown`, whose unknown
    // input has no meaning now): "The library's require(name, pass) is a function over emit! that writes a
    // record to the reserved checks stream; the host reports each one as a pass or a failure, and there is no
    // third verdict. finish() writes to the reserved sim stream and ends the analysis at that instant": a
    // true and a false `require`, and a `finish()` at 2 us that ends a run set for 5 us
    let source = "module Top() { signal ok: Bit = true; signal bad: Bit = false; }\ndigital for Top { on at(1u) { require(\"good\", ok); require(\"bad\", bad); } on at(2u) { finish(); } }\n";
    let ran = Model::written("require.msl", source).run(Commands::transient_of("Top", 5e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let checks = ran.stream("checks");
    assert_eq!(checks.len(), 2, "one record per require");
    assert_eq!(Fields::of(checks[0], "pass"), &Field::Bool(true));
    assert_eq!(Fields::of(checks[1], "name"), &Field::Text("bad".to_string()));
    assert_eq!(Fields::of(checks[1], "pass"), &Field::Bool(false), "a failed check is a failure, with no third verdict");
    assert_eq!(ran.stream("sim").len(), 1, "finish() writes to `sim`");
    let last = ran.table.rows.len() - 1;
    Printed::holds("2.000000e-6", ran.real(last, "time"), "the analysis ends at the instant of finish()");
}

#[test]
fn the_library_declares_the_monitors_and_never_runs() {
    // digital~5f53905d: "The library also has ready-made monitors built this way, such as Never(clk, bad)
    // and Setup(clk, d) { ts: 100p }": `std/monitor.msl` declares both, and `Never` is one `require` per
    // rising edge (`Setup` is generic, which the compiler refuses: chapter 13, core-std)
    let library = StandardLibrary::normalized(&["monitor.msl"]);
    assert!(library.contains("module Never(in clk: Bit, in bad: Bit)"), "Never is declared");
    assert!(library.contains("module Setup<T: Repr>(in clk: Bit, in d: Bit<T>"), "Setup is declared");
    let source = "module Top() { signal clk: Bit = false; signal bad: Bit = false; let m = Never(clk, bad); }\ndigital for Top { on at(1u) { clk <- true; } }\n";
    let ran = Model::written("never.msl", source).run(Commands::transient_of("Top", 3e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let checks = ran.stream("checks");
    assert_eq!(checks.len(), 1, "one `never` check at the one rising edge");
    assert_eq!(Fields::of(checks[0], "name"), &Field::Text("never".to_string()));
    assert_eq!(Fields::of(checks[0], "pass"), &Field::Bool(true));
}

#[test]
fn a_reaction_at_a_time_runs_when_time_reaches_it() {
    // digital~27b7fac4: "at(t): time reaches t": `on at(2.5u)` reads `now()` as 2.5 us
    let source = "module Top() { }\ndigital for Top { on at(2.5u) { emit!(\"hit\", t = now()); } }\n";
    let ran = Model::written("at.msl", source).run(Commands::transient_of("Top", 5e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let hits = ran.stream("hit");
    assert_eq!(hits.len(), 1);
    Printed::holds("2.500000e-6", hits[0].time, "the instant of the reaction");
    Printed::holds("2.500000e-6", Fields::real(hits[0], "t"), "now() inside it");
}

#[test]
fn the_irq_line_and_the_four_nets_are_refused_until_a_nature_can_implement_resolve() {
    // digital#16 (refused, S8): `IrqSource`, `IrqController` and `Board` over `IrqLine`, a nature the model
    // declares with `impl Resolve<Driven>`; digital#17 to digital#20 (refused, S8): the four worked nets of
    // "Driving with strength": a plain bool into `AndWire`, the open-drain `Sda` with its pull-up, the
    // tri-state `TriBus` with a keeper, and the typed `DataBus` with `Resolve<Option<u8>, Data>`. The
    // trait `Resolve`, the type `Driven` and the library modules are not in the standard library yet, so the
    // checker reports the name (E0161); the test fails when a listing starts to run, and the row moves to
    // `run`. The sentences of the section (digital~ids listed in the inventory) are held by these refusals
    let chapter = Chapter::read();
    for (id, top) in [("digital#16", "Board"), ("digital#17", "Board"), ("digital#18", "I2cBus"), ("digital#19", "Bus"), ("digital#20", "")] {
        let model = chapter.lone("net.msl", &[id]);
        let overrides = if top.is_empty() { Commands::transient_of("Top", 1e-6) } else { Commands::transient_of(top, 1e-6) };
        let given = Refusal::of(model.run(overrides), id);
        assert!(given.iter().all(|(code, _)| code == "E0161" || code == "E0112"), "{id}: names the library does not hold yet: {given:?}");
        assert!(given.iter().any(|(_, message)| message.starts_with("cannot find `Resolve")), "{id}: {given:?}");
    }
    Refusal::expect(chapter.lone("net.msl", &["digital#16"]).run(Commands::transient_of("Board", 1e-6)), "digital#16", "E0161", "cannot find `Driven` in this scope");
    Refusal::expect(chapter.lone("net.msl", &["digital#17"]).run(Commands::transient_of("Board", 1e-6)), "digital#17", "E0161", "cannot find `Resolve` in this scope");
    Refusal::expect(chapter.lone("net.msl", &["digital#18"]).run(Commands::transient_of("I2cBus", 1e-6)), "digital#18", "E0161", "cannot find `Driven` in this scope");
    Refusal::expect(chapter.lone("net.msl", &["digital#19"]).run(Commands::transient_of("Bus", 1e-6)), "digital#19", "E0161", "cannot find `Driven` in this scope");
    Refusal::expect(chapter.lone("net.msl", &["digital#20"]).run(Commands::transient_of("Top", 1e-6)), "digital#20", "E0161", "cannot find `Resolve` in this scope");
}

#[test]
fn the_flip_flop_is_refused_for_its_generic_module() {
    // digital#21 (refused): `Dff<T: Repr>`: a generic module is `P0001` (L30)
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit; signal r: Bit; signal d: Bit<u8>; signal q: Bit<u8>; let f = Dff::<u8>(clk, r, d, q) { reset: 0 }; }\n";
    let model = chapter.wrapped("dff.msl", "", "digital#21", after);
    Refusal::expect(model.run(Commands::transient_of("Top", 1e-6)), "digital#21", "P0001", "generic module is not supported yet");
}

/// The multiply-accumulate pipeline of the chapter, in a bench that holds its inputs.
struct Pipeline;

impl Pipeline {
    /// The bench: the clock of chapter 5, inputs 3 and -4 held valid.
    const BENCH: &'static str = "module Top() { signal clk: Bit; signal a: Bit<i16> = 3; signal b: Bit<i16> = -4; signal v: Bit = true; signal acc: Bit<i40>; signal vo: Bit;\n    let o = Osc(clk) { f: 1M }; let m = MacPipe(clk, a, b, v, acc, vo); }\n";

    /// The model of the Book's pipeline, its three stage lines in the order the Book writes them
    /// (`reversed` false) or the other way round.
    fn model(chapter: &Chapter, reversed: bool) -> Model {
        let text = chapter.text("digital#22");
        let text = if reversed { Self::reversed(&text) } else { text };
        Model::written("mac.msl", &format!("{}\n{text}\n{}", Oscillator::text(chapter), Self::BENCH))
    }

    /// `text` with its three stage lines (the ones that end in `// stage N`) in the opposite order.
    fn reversed(text: &str) -> String {
        let mut lines: Vec<&str> = text.lines().collect();
        let stages: Vec<usize> = lines.iter().enumerate().filter(|(_, line)| line.contains("// stage")).map(|(at, _)| at).collect();
        assert_eq!(stages.len(), 3, "the three stage lines");
        let kept: Vec<&str> = stages.iter().rev().map(|at| lines[*at]).collect();
        for (at, line) in stages.iter().zip(kept) {
            lines[*at] = line;
        }
        lines.join("\n")
    }

    /// The accumulator and the valid flag at each microsecond from 1 to 7.
    fn trace(model: &Model) -> Vec<(i128, String)> {
        let overrides = Overrides { print_step: Some(1e-6), print: vec!["acc".to_string(), "vo".to_string()], ..Commands::transient_of("Top", 7e-6) };
        let ran = model.run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
        (0..ran.table.rows.len()).map(|row| (Table::integer(&ran, row, "Top.acc"), Table::text(&ran, row, "Top.vo"))).collect()
    }
}

#[test]
fn a_pipeline_in_one_reaction_reads_every_register_before_the_edge() {
    // digital#22 (run): `MacPipe`: "Pipelines fall out of 'reads see the past'. One reaction can describe all
    // three stages of a multiply-accumulate pipeline, in any statement order, because every right-hand side
    // reads the registers as they were before the edge". Inputs 3 and -4: the first product reaches `acc`
    // at the third edge (2.5 us), then -12 more at each edge; written in the opposite statement order the
    // trace is the same
    let chapter = Chapter::read();
    let ran_book = Pipeline::model(&chapter, false);
    chapter.wrapped("mac.msl", &format!("{}\n", Oscillator::text(&chapter)), "digital#22", Pipeline::BENCH).run(Commands::transient_of("Top", 7e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    let trace = Pipeline::trace(&ran_book);
    let accumulators: Vec<i128> = trace.iter().map(|(acc, _)| *acc).collect();
    assert_eq!(accumulators, [0, 0, 0, -12, -24, -36, -48, -60], "three stages of latency, then -12 per edge");
    assert!(trace[2].1.contains("false") && trace[3].1.contains("true"), "valid_out follows the third stage: {trace:?}");
    assert_eq!(Pipeline::trace(&Pipeline::model(&chapter, true)), trace, "the statement order does not matter");
}

#[test]
fn e0430_a_bit_is_a_level_not_an_event() {
    // digital~54ec481d and digital~4f9a5b9e: "E0430 on clk { ... }": "A plain bool alone is not an event
    // (E0430): on enable { ... } has no 'when'": say which change: on clk.rise()
    let faulty = Faulty::elaborating("digital~54ec481d", "module Top(in clk: Bit) {}\ndigital for Top { on clk { } }\n", "Top");
    assert_eq!(faulty.first("E0430").message, "the head of `on` has type `bool`: it must be an `Event`; a `bool` alone is not an event (§12.1.10)");
    let fixed = Model::written("fixed.msl", "module Top(in clk: Bit) {}\ndigital for Top { on clk.rise() { } }\n").run(Faulty::operating_point_of("Top"));
    assert!(fixed.is_ok(), "`clk.rise()` is an event");
}

#[test]
fn e0221_one_carrier_is_driven_one_way() {
    // digital~42ae751d: "E0221 wrap = count == 0; and also wrap <- true;"
    let source = "module Top() { signal count: Bit<u8> = 0; signal wrap: Bit; wrap = count == 0; }\ndigital for Top { on at(1u) { wrap <- true; } }\n";
    let faulty = Faulty::elaborating("digital~42ae751d", source, "Top");
    assert_eq!(faulty.first("E0221").message, "`wrap` has a definition and a commit in one module: a carrier is driven by one or the other (§5.2, §8.2.20)");
}

#[test]
fn e0338_an_undelayed_definition_has_no_initial_value() {
    // digital~4bcf191d: "E0338 out wrap: Bit = false with wrap = count == 0;"
    let source = "module Top(out wrap: Bit = false) { signal count: Bit<u8> = 0; wrap = count == 0; }\n";
    let faulty = Faulty::elaborating("digital~4bcf191d", source, "Top");
    assert!(faulty.first("E0338").message.starts_with("`wrap` has an initial value and an undelayed definition"), "{}", faulty.first("E0338").message);
}

#[test]
fn e0432_a_commit_of_another_type_is_a_mismatch() {
    // digital~54ec4a1d: "E0432 count <- count + 1.0;": "The committed type must be the carrier's. count is
    // a u8: commit count + 1": the operand `1.0` is the mismatch, with the Book's code
    let book = Faulty::running("digital~54ec4a1d", "module Top() { signal count: Bit<u8> = 0; }\ndigital for Top { on at(1u) { count <- count + 1.0; } }\n", Commands::transient_of("Top", 1e-6));
    assert_eq!(book.first("E0432").message, "mismatched types: expected `u8`, found `f64`");
    assert!(book.given.iter().all(|reported| reported.code != "E0180"), "the Book's code, not the plain mismatch");
    let commit = Faulty::running("digital~54ec4a1d", "module Top() { signal count: Bit<u8> = 0; }\ndigital for Top { on at(1u) { count <- 1.0; } }\n", Commands::transient_of("Top", 1e-6));
    assert_eq!(commit.first("E0432").message, "mismatched types: expected `u8`, found `f64`");
}

#[test]
fn e0434_a_digital_block_holds_only_reactions() {
    // digital~54ec441d: "E0434 count <- 0; directly in digital for M { ... }": for a start value use = 0 on
    // the signal, or on init()
    let faulty = Faulty::elaborating("digital~54ec441d", "module Top() { signal count: Bit<u8> = 0; }\ndigital for Top { count <- 0; }\n", "Top");
    assert_eq!(faulty.first("E0434").message, "a statement outside a reaction in a `digital` block");
    assert_eq!(faulty.first("E0434").help.as_deref(), Some("a `digital` block holds reactions: `on event { .. }`"));
}

#[test]
fn e0350_an_instance_is_not_placed_inside_a_reaction() {
    // digital~4bd60d1d: "E0350 on clk.rise() { let e = above(v(a)); ... }": name it in the body
    let faulty = Faulty::elaborating("digital~4bd60d1d", "module Top() { signal a: Electrical; }\ndigital for Top { on at(1u) { let e = above(v(a)); } }\n", "Top");
    assert_eq!(faulty.first("E0350").message, "`above` is placed in a reaction body: it places no instance (§8.3.4)");
}

#[test]
fn e0501_a_clock_with_no_delay_never_goes_quiet() {
    // digital~5c44f01d and digital~6b910006: "E0501 on clk.changed() { clk <- !clk; }": a run error, with no
    // delay the microsteps at one time never go quiet. "Add after 0.5 / f": the Osc of chapter 5 runs
    let source = "module Top() { signal clk: Bit; }\ndigital for Top { on clk.changed() { clk <- !clk; } on at(1n) { clk <- true; } }\n";
    let given = Refusal::of(Model::written("spin.msl", source).run(Commands::transient_of("Top", 1e-6)), "digital~5c44f01d");
    assert_eq!(given.len(), 1, "{given:?}");
    assert_eq!(given[0].0, "E0501");
    assert!(given[0].1.contains("did not go quiet; still changing: Top.clk"), "{}", given[0].1);
    let chapter = Chapter::read();
    let fixed = format!("{}\nmodule Top() {{ signal clk: Bit; let o = Osc(clk) {{ f: 1M }}; }}\n", Oscillator::text(&chapter));
    Model::written("osc.msl", &fixed).run(Commands::transient_of("Top", 3e-6)).unwrap_or_else(|failure| panic!("`after 0.5 / f` settles: {failure}"));
}
