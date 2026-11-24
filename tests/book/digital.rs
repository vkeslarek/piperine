//! Scope: chapter 9 of the Book, "Digital behavior" (feature `root-e2e`, RE-02 and RE-03): the counter
//! and its 257 `state` records, the set-reset latch, the multiply-accumulate pipeline, the debouncer
//! and the PWM (their integer params run since params-and-io, T35: the Book's own text, no param written as a literal), the checks and `finish()`, the monitors, and the
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
use support::records::{Fields, Rows};
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
/// param `n` of the Book (4), (the button is a plain bool).
struct Debouncer;

impl Debouncer {
    /// The debouncer's file as the Book writes it, with `bench` in place of the
    /// bench's timed reactions.
    fn accepted(chapter: &Chapter, bench: &str) -> Model {
        let text = chapter.text("digital#7");
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

/// The PWM of the chapter and its checker, as the Book writes them.
struct Generator;

impl Generator {
    /// The PWM file; `with_check` places the checker beside the generator.
    fn accepted(chapter: &Chapter, with_check: bool) -> Model {
        let text = chapter.text("digital#11");
        let mut text = text;
        if with_check {
            text = Edit::replace(&text, "let pulse = Pwm(clk, duty, pwm, wrap) { period: 100 };", "let pulse = Pwm(clk, duty, pwm, wrap) { period: 100 };\n    let chk = PwmCheck(clk, wrap, pwm);");
            text = format!("{text}\n{}", chapter.text("digital#14"));
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
fn the_debouncer_pwm_and_pwm_check_print_their_records() {
    // digital#7, digital#11 and digital#14 (run, L28 closed by params-and-io PI-01): the Book's own files, their
    // integer params `n: u8`, `period: u8`, `limit: u8` and `cycles: u32` unedited, build and print the records the
    // listings of digital#10 and digital#13 show: digital~c92f9704 "the output flipped once, at the fourth clean sample"
    let chapter = Chapter::read();
    let debounce = Overrides { analysis: Some("transient".to_string()), stop: Some(7e-3), streams: vec!["flip".to_string()], ..Overrides::default() };
    let flips = chapter.marked("debounce.msl", "digital#7").run(debounce).unwrap_or_else(|failure| panic!("digital#7 runs with its integer param: {failure:#}"));
    assert_eq!(flips.stream("flip").len(), 1, "the output flipped once");
    Printed::holds("5.500000e-3", flips.stream("flip")[0].time, "the fourth clean sample");

    let pwm = Overrides { analysis: Some("transient".to_string()), stop: Some(450e-6), streams: vec!["cycle".to_string()], ..Overrides::default() };
    let cycles = chapter.marked("pwm.msl", "digital#11").run(pwm).unwrap_or_else(|failure| panic!("digital#11 runs with its integer param: {failure:#}"));
    let duties: Vec<&Field> = cycles.stream("cycle").iter().map(|record| Fields::of(record, "duty")).collect();
    assert_eq!(duties, [&Field::Int(25), &Field::Int(25), &Field::Int(60), &Field::Int(60)], "one record per period, the duty word of the cycle");

    // digital#14 placed in the bench as digital~a0ca3f9d says: its `limit: u8` and `cycles: u32` take their defaults
    let checked = Generator::accepted(&chapter, true).run(Commands::transient(3e-3)).unwrap_or_else(|failure| panic!("digital#14 runs with its integer params: {failure:#}"));
    assert_eq!(checked.stream("checks").len(), 20, "one `duty_limit` check per wrap, twenty cycles");
}

#[test]
fn the_debouncer_flips_once_at_the_fourth_clean_sample() {
    // The debouncer of digital#7 as the Book writes it, `n` of 4: digital~c92f9704 "It prints one record line: the output flipped once, at
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
fn the_pwm_latches_its_duty_word_at_the_wrap() {
    // The PWM of digital#11 as the Book writes it, `period` of 100: digital~107c5906 "A counter that wraps every
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
fn the_check_passes_every_cycle_and_ends_the_run_at_the_twentieth() {
    // The PWM bench of digital#11 with the checker of digital#14, their integer params as the Book writes them:
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
    // digital~7b9de27c: "The library also has ready-made monitors built this way, such as Never(clk, bad)
    // and Setup(clk, d, late) { ts: 100p }": `std/monitor.msl` declares both, and `Never` is one `require` per
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

/// A listing of "Driving with strength" wrapped in a bench: the Book's own modules, a replica of the listing's board that
/// writes the net it resolves (a board keeps its net to itself), and the stimulus the table of the appendix names.
struct NetBench;

impl NetBench {
    /// The model of the listing `id` with the bench `after` that follows it; its top is `Top`.
    fn model(id: &str, after: &str) -> Model {
        Chapter::read().wrapped("net.msl", "", id, after)
    }

    /// The rows of the stream `stream` of `ran` with `fields`, one text per record (`Rows::of`).
    fn rows(ran: &Ran, stream: &str, fields: &[&str]) -> Vec<String> {
        Rows::of(&ran.stream(stream), fields)
    }

    /// The bench of digital#16: a watched copy of the board, one with no pull-up, the listing's own `Board`, and the stimulus.
    const IRQ_BENCH: &'static str = "\
module IrqLook(irq_n: IrqLine) {}
digital for IrqLook { on irq_n.changed() { emit!(\"line\", level = irq_n.v, strength = irq_n.s); } }
module BareLook(irq_n: IrqLine) {}
digital for BareLook { on irq_n.changed() { emit!(\"bare\", level = irq_n.v, strength = irq_n.s); } }
module Watched(in clk: Bit, in req0: Bit, in req1: Bit, out pending: Bit) {
    signal irq_n: IrqLine;
    let pu = Pullup(irq_n);
    let s0 = IrqSource(req0, irq_n);
    let s1 = IrqSource(req1, irq_n);
    let ic = IrqController(clk, irq_n, pending);
    let look = IrqLook(irq_n);
}
module Bare(in clk: Bit, in req0: Bit, out pending: Bit) {
    signal irq_n: IrqLine;
    let s0 = IrqSource(req0, irq_n);
    let ic = IrqController(clk, irq_n, pending);
    let look = BareLook(irq_n);
}
module Top() {
    signal clk: Bit = false;
    signal r0: Bit = false;
    signal r1: Bit = false;
    signal pending: Bit;
    signal bare_pending: Bit;
    signal listed: Bit;
    let w = Watched(clk, r0, r1, pending);
    let b = Bare(clk, r0, bare_pending);
    let board = Board(clk, r0, r1, listed);
}
digital for Top {
    on pending.changed() { emit!(\"pending\", v = pending); }
    on listed.changed() { emit!(\"listed\", v = listed); }
    on bare_pending.changed() { emit!(\"bare_pending\", v = bare_pending); }
    on at(500n) { clk <- true; }
    on at(700n) { clk <- false; }
    on at(1u) { r0 <- true; }
    on at(2u) { r1 <- true; }
    on at(3u) { clk <- true; }
    on at(3500n) { clk <- false; }
    on at(4u) { r0 <- false; r1 <- false; }
    on at(5u) { clk <- true; }
}
";

    /// The bench of digital#17: a watched copy of the board, a net nobody drives, the listing's `Board`, and the stimulus.
    const AND_WIRE_BENCH: &'static str = "\
module AndLook(irq_n: AndWire) {}
digital for AndLook { on irq_n.changed() { emit!(\"net\", value = irq_n); } }
module AloneLook(lone: AndWire) {}
digital for AloneLook { on lone.changed() { emit!(\"alone\", value = lone); } }
module Watched(in a: Bit, in b: Bit) {
    signal irq_n: AndWire;
    let s0 = IrqSource(irq_n, a);
    let s1 = IrqSource(irq_n, b);
    let look = AndLook(irq_n);
}
module Top() {
    signal a: Bit = false;
    signal b: Bit = false;
    signal lone: AndWire;
    let w = Watched(a, b);
    let board = Board(a, b);
    let l = AloneLook(lone);
}
digital for Top { on at(1u) { a <- true; } on at(2u) { b <- true; } on at(3u) { a <- false; b <- false; } }
";

    /// The bench of digital#18: a watched copy of the bus, one with no pull-up, the listing's `I2cBus`, and the stimulus.
    const SDA_BENCH: &'static str = "\
module SdaLook(sda: Sda) {}
digital for SdaLook { on sda.changed() { emit!(\"net\", level = sda.v, strength = sda.s); } }
module BareLook(sda: Sda) {}
digital for BareLook { on sda.changed() { emit!(\"bare\", level = sda.v, strength = sda.s, z = sda.v.is_z()); } }
module Watched(in m: Bit, in s: Bit) {
    signal sda: Sda;
    let master = OpenDrainPin(sda, m);
    let slave = OpenDrainPin(sda, s);
    let rp = Pullup(sda);
    let look = SdaLook(sda);
}
module Bare(in m: Bit, in s: Bit) {
    signal sda: Sda;
    let master = OpenDrainPin(sda, m);
    let slave = OpenDrainPin(sda, s);
    let look = BareLook(sda);
}
module Top() {
    signal m: Bit = false;
    signal s: Bit = false;
    let w = Watched(m, s);
    let bare = Bare(m, s);
    let bus = I2cBus(m, s);
}
digital for Top { on at(1u) { m <- true; } on at(2u) { s <- true; } on at(3u) { m <- false; s <- false; } }
";

    /// The bench of digital#19: a watched copy of the bus, the listing's `Bus`, and the stimulus.
    const BUS_BENCH: &'static str = "\
module BusLook(line: TriBus) {}
digital for BusLook { on line.changed() { emit!(\"net\", level = line.v, strength = line.s, x = line.v.is_x(), bit = line.v.to_bit()); } }
module Watched(in d0: Bit, in oe0: Bit, in d1: Bit, in oe1: Bit) {
    signal line: TriBus;
    let a = Tribuf(line, d0, oe0);
    let b = Tribuf(line, d1, oe1);
    let k = Keeper(line);
    let look = BusLook(line);
}
module Top() {
    signal d0: Bit = true;
    signal oe0: Bit = false;
    signal d1: Bit = false;
    signal oe1: Bit = false;
    let w = Watched(d0, oe0, d1, oe1);
    let bus = Bus(d0, oe0, d1, oe1);
}
digital for Top {
    on at(1u) { oe0 <- true; }
    on at(2u) { oe0 <- false; }
    on at(3u) { oe1 <- true; }
    on at(4u) { oe1 <- false; }
    on at(5u) { oe0 <- true; oe1 <- true; }
    on at(6u) { d1 <- true; }
    on at(7u) { oe0 <- false; oe1 <- false; }
}
";

    /// The bench of digital#20: two drivers of the typed bus, a reader and the stimulus.
    const DATA_BENCH: &'static str = "\
module Put(w: DataBus, in v: Bit<Option<u8>> = None) { w = v; }
module Look(w: DataBus) {}
digital for Look { on w.changed() { emit!(\"net\", free = w == Data::Free, five = w == Data::Byte(5), seven = w == Data::Byte(7), contended = w == Data::Contended); } }
module Top() {
    signal w: DataBus;
    signal a: Bit<Option<u8>> = None;
    signal b: Bit<Option<u8>> = None;
    let p = Put(w, a);
    let q = Put(w, b);
    let l = Look(w);
}
digital for Top { on at(1u) { a <- Some(5); } on at(2u) { b <- Some(7); } on at(3u) { a <- None; } }
";

    /// `rows` as the owned texts `NetBench::rows` gives, for a table written in the test.
    fn table(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|row| row.to_string()).collect()
    }

    /// The rows of the strength-resolved nets: the level and the strength.
    const DRIVEN: [&'static str; 2] = ["level", "strength"];
}

#[test]
fn the_irq_line_runs_on_its_resolved_nature() {
    // digital#16 (run; Appendix E.7): `IrqSource`, `IrqController` and `Board` over `IrqLine`, a nature the model declares with
    // `impl Resolve<Driven>`. Idle the pull-up wins (1 at Pull); a request drives strong 0; a second request changes nothing;
    // the controller samples the settled line at a clock edge; with every source released the line is high again. The same
    // sources and controller on a line with no pull-up read Z while released, and `pending` stays low
    let ran = NetBench::model("digital#16", NetBench::IRQ_BENCH).run(Commands::transient_of("Top", 6e-6)).unwrap_or_else(|failure| panic!("digital#16: {failure}"));
    assert_eq!(NetBench::rows(&ran, "line", &NetBench::DRIVEN), NetBench::table(&["0 1 Pull", "1000 0 Strong", "4000 1 Pull"]), "idle, a request, the second request changes nothing, released");
    assert_eq!(NetBench::rows(&ran, "pending", &["v"]), NetBench::table(&["0 false", "3000 true", "5000 false"]), "sampled at the edges that follow the request and its release");
    assert_eq!(NetBench::rows(&ran, "listed", &["v"]), NetBench::table(&["0 false", "3000 true", "5000 false"]), "the listing's own `Board` gives what its watched copy does");
    assert_eq!(NetBench::rows(&ran, "bare", &NetBench::DRIVEN), NetBench::table(&["0 z HighZ", "1000 0 Strong", "4000 z HighZ"]), "no pull-up: Z while released");
    assert_eq!(NetBench::rows(&ran, "bare_pending", &["v"]), NetBench::table(&["0 false", "3000 true", "5000 false"]), "`pending` stays low on the edge at 0.5 us, while the line reads Z");
}

#[test]
fn the_four_worked_nets_give_the_tables_of_f_4() {
    // digital#17, digital#18, digital#19 and digital#20 (run; Appendix F.4.1 to F.4.4): each net's table of outcomes, one bench per listing
    // F.4.1: a plain bool into a wired-AND net
    let ran = NetBench::model("digital#17", NetBench::AND_WIRE_BENCH).run(Commands::transient_of("Top", 4e-6)).unwrap_or_else(|failure| panic!("digital#17: {failure}"));
    assert_eq!(NetBench::rows(&ran, "net", &["value"]), NetBench::table(&["0 true", "1000 false", "3000 true"]), "(false, false) high; (true, false) and (true, true) low");
    assert_eq!(NetBench::rows(&ran, "alone", &["value"]), NetBench::table(&["0 true"]), "no `IrqSource`: `resolve` of an empty array is high");
    // the listing's own `Board` resolves the same table on its own net `irq_n`, sampled in the middle of each step of the stimulus
    let listed = NetBench::model("digital#17", NetBench::AND_WIRE_BENCH).run(Overrides { print_step: Some(0.5e-6), print: vec!["board.irq_n".to_string()], ..Commands::transient_of("Top", 4e-6) }).unwrap_or_else(|failure| panic!("digital#17: {failure}"));
    let at = |row: usize| Table::text(&listed, row, "Top.board.irq_n");
    assert_eq!([at(1), at(3), at(5), at(7)], ["Bool(true)", "Bool(false)", "Bool(false)", "Bool(true)"].map(str::to_string), "(false, false) high; (true, false) and (true, true) low; released, high");
    // a drive of another type picks no `impl`: E0180 naming the nature and the type (digital~4b2beb4c)
    let wrong = format!("{}module Wrong(irq_n: AndWire) {{ irq_n = logic(\"1\"); }}\nmodule Top() {{ signal n: AndWire; let w = Wrong(n); }}\n", Chapter::read().text("digital#17"));
    let refused = Refusal::of(Model::written("net.msl", &wrong).run(Commands::transient_of("Top", 1e-6)), "digital#17");
    assert!(refused.iter().any(|(code, message)| code == "E0180" && message.contains("AndWire") && message.contains("Logic")), "{refused:?}");
    // F.4.2: an open-drain line with a pull-up
    let ran = NetBench::model("digital#18", NetBench::SDA_BENCH).run(Commands::transient_of("Top", 4e-6)).unwrap_or_else(|failure| panic!("digital#18: {failure}"));
    assert_eq!(NetBench::rows(&ran, "net", &NetBench::DRIVEN), NetBench::table(&["0 1 Pull", "1000 0 Strong", "3000 1 Pull"]), "Z and Z: 1 at Pull; strong 0: 0 at Strong; two strong 0: still 0 at Strong");
    assert_eq!(NetBench::rows(&ran, "bare", &["level", "strength", "z"]), NetBench::table(&["0 z HighZ true", "1000 0 Strong false", "3000 z HighZ true"]), "no `Pullup`: `Driven::Z`, `sda.v.is_z()`");
    // F.4.3 and F.4.4: a tri-state bus with two drivers and a keeper, and a conflict of equal strengths
    let ran = NetBench::model("digital#19", NetBench::BUS_BENCH).run(Commands::transient_of("Top", 8e-6)).unwrap_or_else(|failure| panic!("digital#19: {failure}"));
    let held = NetBench::rows(&ran, "net", &["level", "strength", "x"]);
    assert_eq!(
        held,
        NetBench::table(&["0 x Weak true", "1000 1 Strong false", "2000 1 Weak false", "3000 0 Strong false", "4000 0 Weak false", "5000 x Strong true", "6000 1 Strong false", "7000 1 Weak false"]),
        "before any drive X at Weak; `d0` at Strong; the keeper's last level at Weak; `d1` at Strong; the conflict X at Strong; agreement 1 at Strong; the keeper remembers the 1"
    );
    // F.4.4's typed bus: the driven type is `Option<u8>`, the value a reader sees `Data`
    let ran = NetBench::model("digital#20", NetBench::DATA_BENCH).run(Commands::transient_of("Top", 4e-6)).unwrap_or_else(|failure| panic!("digital#20: {failure}"));
    assert_eq!(
        NetBench::rows(&ran, "net", &["free", "five", "seven", "contended"]),
        NetBench::table(&["0 true false false false", "1000 false true false false", "2000 false false false true", "3000 false false true false"]),
        "free, one driver's byte, two drivers contended, the other driver's byte"
    );
}

#[test]
fn the_flip_flop_runs_for_a_type_with_a_default() {
    // digital#21 (run): `Dff<T: Repr + Default>` at `Option<u8>`, a type with a `Default` of its own (§3.3.4); `q` starts at
    // `None`, the clock of chapter 5 rises at 0.5 us and commits `d` = `Some(42)`, and a reset that is held high never
    // fires. The records are written where `q` changes: at the start (`changed()` fires at (0, 0)), where `unwrap_or(7)` of
    // `None` is 7, and at the first edge, where it is 42
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit; signal r: Bit = true; signal d: Bit<Option<u8>> = Some(42); signal q: Bit<Option<u8>>; let o = Osc(clk) { f: 1M }; let f = Dff(clk, r, d, q) { reset: None }; }\n\
                 digital for Top { on q.changed() { emit!(\"q\", v = q.unwrap_or(7u8), some = q.is_some()); } }\n";
    let model = chapter.wrapped("dff.msl", &Oscillator::text(&chapter), "digital#21", after);
    let ran = model.run(Commands::transient_of("Top", 2e-6)).unwrap_or_else(|failure| panic!("{:?}", Refusal::of(Err(failure), "digital#21")));
    let seen: Vec<Field> = ran.stream("q").iter().map(|record| Fields::of(record, "v").clone()).collect();
    let flags: Vec<Field> = ran.stream("q").iter().map(|record| Fields::of(record, "some").clone()).collect();
    assert_eq!(flags, [Field::Bool(false), Field::Bool(true)], "`q.is_some()` is false at the start and true once `d` is committed: a record as large as `q` is no change of `q`");
    let (first, then) = (None::<u8>, Some(42u8));
    assert_eq!(seen, [Field::Int(i128::from(first.unwrap_or(7))), Field::Int(i128::from(then.unwrap_or(7)))], "None at the start, then `d` committed at the first edge");
    Printed::holds("5.000000e-7", ran.stream("q")[1].time, "the first rising edge of the 1 MHz clock");
}

#[test]
fn e0187_the_flip_flop_at_u8_has_no_default() {
    // digital#21 (§3.3.4, MD-78): `u8` has no `Default`, so `Dff::<u8>` is `E0187` naming `u8` and `Default`: the
    // type is refused, the model is not wrong
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit; signal r: Bit = true; signal d: Bit<u8> = 0; signal q: Bit<u8>; let o = Osc(clk) { f: 1M }; let f = Dff::<u8>(clk, r, d, q) { reset: 0 }; }\n";
    let model = chapter.wrapped("dff.msl", &Oscillator::text(&chapter), "digital#21", after);
    let given = Refusal::of(model.run(Commands::transient_of("Top", 2e-6)), "digital#21");
    // the placement fails its bound and is reported once: `q`, handed to its `out` port, is not judged again as a carrier nothing drives
    let wanted = ["`u8` does not satisfy the bound `Default` of `T` in `Dff` (§3.8.7)"];
    assert_eq!(given, wanted.map(|message| ("E0187".to_string(), message.to_string())), "the bound, once");
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
    let fixed = Model::written("fixed.msl", "module Top(in clk: Bit = false) {}\ndigital for Top { on clk.rise() { } }\n").run(Faulty::operating_point_of("Top"));
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
    let source = "module Top() { signal clk: Bit = false; }\ndigital for Top { on clk.changed() { clk <- !clk; } on at(1n) { clk <- true; } }\n";
    let given = Refusal::of(Model::written("spin.msl", source).run(Commands::transient_of("Top", 1e-6)), "digital~5c44f01d");
    assert_eq!(given.len(), 1, "{given:?}");
    assert_eq!(given[0].0, "E0501");
    assert!(given[0].1.contains("did not go quiet; still changing: Top.clk"), "{}", given[0].1);
    let chapter = Chapter::read();
    let fixed = format!("{}\nmodule Top() {{ signal clk: Bit; let o = Osc(clk) {{ f: 1M }}; }}\n", Oscillator::text(&chapter));
    Model::written("osc.msl", &fixed).run(Commands::transient_of("Top", 3e-6)).unwrap_or_else(|failure| panic!("`after 0.5 / f` settles: {failure}"));
}
