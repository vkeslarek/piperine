//! Scope: chapter 8 of the Book, "Mixed-signal" (feature `root-e2e`, RE-02 and RE-03): the cycle
//! counter through its comparator with hysteresis, the `D2A` converter, the delta-sigma modulator, the
//! buck converter's control loop (its `settle` check, the comparator without hysteresis and the SAR
//! ADC are refused today, asserted refused with their diagnostics), and each mistake of the chapter's
//! table, run through `piperine::`. Each test opens with the inventory ids it holds, from
//! `tests/book/inventory.tsv`.

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
}

use piperine::{Cell, Overrides};
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
use support::printed::Printed;
use support::records::Fields;
use support::refusal::Refusal;

/// A DC source, the stimulus of most small designs of the chapter.
const SOURCE: &str = "module Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\n";

/// The runs the chapter's commands make.
struct Commands;

impl Commands {
    /// A transient of the top `top` to `stop`, printing every `print_step` seconds when given.
    fn transient(top: &str, stop: f64, print_step: Option<f64>) -> Overrides {
        Overrides { analysis: Some("transient".to_string()), top: Some(top.to_string()), stop: Some(stop), print_step, ..Overrides::default() }
    }

    /// `piperine run cycles.msl --analysis transient --stop 3e-3 --print-step 1e-3 --print count --stream pulse`.
    fn cycles() -> Overrides {
        Overrides { analysis: Some("transient".to_string()), stop: Some(3e-3), print_step: Some(1e-3), print: vec!["count".to_string()], streams: vec!["pulse".to_string()], ..Overrides::default() }
    }

    /// `piperine run buck.msl --analysis transient --stop 12e-3 --print vout --print-step 1e-3 --stream loop`.
    fn buck() -> Overrides {
        Overrides { analysis: Some("transient".to_string()), stop: Some(12e-3), print: vec!["vout".to_string()], print_step: Some(1e-3), streams: vec!["loop".to_string()], ..Overrides::default() }
    }
}

/// The cycle counter of the chapter's first example.
struct CycleCounter;

impl CycleCounter {
    /// The Book's `cycles.msl`: the comparator block and the complete design.
    fn model(chapter: &Chapter) -> Model {
        chapter.marked("cycles.msl", "mixed#2")
    }

}

/// The buck converter of the chapter's second example.
struct Buck;

impl Buck {
    /// The `loop` records of a run of `model` to 12 ms, as `(time, vout, err, duty)`.
    fn loop_records(model: &Model) -> Vec<(f64, f64, f64, f64)> {
        let ran = model.run(Commands::buck()).unwrap_or_else(|failure| panic!("{failure}"));
        ran.stream("loop").iter().map(|record| (record.time, Fields::real(record, "vout"), Fields::real(record, "err"), Fields::real(record, "duty"))).collect()
    }

    /// The converter with the integral gain `ki` (the Book's text has `1m`).
    fn with_gain(chapter: &Chapter, ki: &str) -> Model {
        let text = chapter.text("mixed#8");
        let from = "ki: f64 = 1m where ki > 0.0;";
        assert!(text.contains(from), "the Book's text holds `{from}`");
        Model::written("buck.msl", &text.replace(from, &format!("ki: f64 = {ki} where ki > 0.0;")))
    }
}

#[test]
fn the_cycle_counter_counts_the_located_crossings_of_the_sine() {
    // mixed#1 and mixed#2 (run): the comparator block and the complete design, one file (`cycles.msl`);
    // mixed#5 holds the listing of the command; mixed~ba1dc599 "EdgeCounter's reaction runs, emits the
    // record and commits count"; mixed~b52d3509 "With a threshold of 0 V and 0.2 V of hysteresis, the
    // comparator goes high when the sine rises through +0.1 V, which first happens 15.9 us into each
    // period"; mixed~306eca43 "It prints three record lines from CycleCounter.ctr, with n=1, n=2 and n=3,
    // about 15.9 us into each millisecond. --print-step 1e-3 keeps the table to one row per millisecond, and
    // --print count to the one signal that matters, so its last row shows CycleCounter.count at 3. The
    // record times are the located crossings, not rows of the table, and bound_step keeps every step of the
    // sine at most 20 us"
    let chapter = Chapter::read();
    let ran = CycleCounter::model(&chapter).run(Commands::cycles()).unwrap_or_else(|failure| panic!("{failure}"));
    let pulses = ran.stream("pulse");
    assert_eq!(pulses.len(), 3, "three record lines");
    for ((record, count), instant) in pulses.iter().zip(1..).zip(["1.594214e-5", "1.015942e-3", "2.015942e-3"]) {
        assert_eq!(record.instance, "CycleCounter.ctr");
        assert_eq!(Fields::of(record, "n"), &piperine::Field::Int(count), "n counts the pulses");
        Printed::holds(instant, record.time, "the located crossing");
        Printed::holds(instant, Fields::real(record, "t"), "the record's own `t`");
    }
    let rising_through_a_tenth = (0.1f64).asin() / (2.0 * std::f64::consts::PI * 1e3);
    assert!((pulses[0].time - rising_through_a_tenth).abs() < 5e-9, "the sine reaches +0.1 V at {rising_through_a_tenth:e} s, about 15.9 us");
    assert_eq!(ran.table.columns, ["time", "CycleCounter.count"], "--print count keeps the one signal");
    assert_eq!(ran.table.rows.len(), 4, "one row per millisecond, from 0 to 3 ms");
    assert_eq!(ran.table.rows[3][1], Cell::Int(3), "the last row shows the count at 3");
    let every_step = CycleCounter::model(&chapter).run(Overrides { print_step: None, print: Vec::new(), ..Commands::cycles() }).unwrap_or_else(|failure| panic!("{failure}"));
    let times: Vec<f64> = (0..every_step.table.rows.len()).map(|row| every_step.real(row, "time")).collect();
    assert!(times.len() > 150, "a row for every accepted step: {}", times.len());
    assert!(times.windows(2).all(|pair| pair[1] - pair[0] <= 20e-6 * (1.0 + 1e-6)), "bound_step keeps every step of the sine at most 20 us");
}

#[test]
fn the_math_methods_run_and_the_settle_event_does_not() {
    // mixed~22d4126e: "Two things the examples here use are not compiled yet and are refused with P0001: the
    // math methods of f64 such as sin (the sine sources) and the settle event (the converter's check)": the
    // sine source runs (the cycle counter above), the `settle` check is `P0001` (mixed#10). The first half is
    // the same stale sentence as welcome~5b11a01a (D2: the code outran the Book)
    let chapter = Chapter::read();
    CycleCounter::model(&chapter).run(Commands::cycles()).unwrap_or_else(|failure| panic!("`sin` runs: {failure}"));
    Refusal::expect(chapter.marked("buck.msl", "mixed#10").run(Commands::buck()), "mixed~22d4126e", "P0001", "built-in `settle` is not supported yet");
}

#[test]
fn the_comparator_without_hysteresis_is_refused_for_its_event_read_as_a_bool() {
    // mixed#6 (refused): `Comparator`, `on up || dn { q <- up; }`: "in the body, up read as a bool says
    // whether it was the rising one" is an event read as a `bool` (L51): the checker answers `E0441`, an
    // `Event` committed, where the spec allows it
    let chapter = Chapter::read();
    let after = format!("{SOURCE}module Top() {{ signal a: Electrical; signal b: Electrical; signal q: Bit; let s = Vdc(a) {{ dc: 1.0 }}; let s2 = Vdc(b) {{ dc: 0.5 }}; let c = Comparator(a, b, q); }}\n");
    let model = chapter.wrapped("comparator.msl", "", "mixed#6", &after);
    Refusal::expect(model.run(Commands::transient("Top", 1e-6, None)), "mixed#6", "E0441", "an `Event` value outside an event position, a guard or a reaction body: an event is never committed, carried or stored (§13 rule 8)");
}

#[test]
fn the_digital_to_analog_converter_drives_half_the_supply_for_unknown() {
    // mixed#7 (run): `D2A`; mixed~e784c042 "d.known() gives three cases, and the model drives half the supply
    // for 'don't know'"; mixed~f53cc3cb "That is the choice E0150 forces every analog reader to make.
    // transition turns the stepped level into a ramp of duration tr, so the output has a real edge": with
    // vdd = 2 V and `tr` = 1 us, an unknown input gives 1 V, a high at 1 us ramps to 2 V (1.5 V halfway) and a
    // low at 3 us ramps back to 0 V
    let chapter = Chapter::read();
    let after = format!("{SOURCE}module Top() {{ signal d: Bit; signal a: Electrical; signal vdd: Electrical; let s = Vdc(vdd) {{ dc: 2.0 }}; let c = D2A(d, a, vdd) {{ tr: 1u }}; }}\ndigital for Top {{ on at(1u) {{ d <- true; }} on at(3u) {{ d <- false; }} }}\n");
    let ran = chapter.wrapped("d2a.msl", "", "mixed#7", &after).run(Commands::transient("Top", 5e-6, Some(5e-7))).unwrap_or_else(|failure| panic!("{failure}"));
    let level = |microseconds: f64| {
        let row = ran.table.rows.iter().position(|row| matches!(row[0], Cell::Real(time) if (time - microseconds * 1e-6).abs() < 1e-9)).unwrap_or_else(|| panic!("no row at {microseconds} us"));
        ran.real(row, "Top.a")
    };
    assert!((level(0.5) - 1.0).abs() < 1e-6, "unknown drives half the supply: {}", level(0.5));
    assert!((level(1.5) - 1.5).abs() < 1e-6, "halfway up the ramp of `tr`: {}", level(1.5));
    assert!((level(2.5) - 2.0).abs() < 1e-6, "high is the supply: {}", level(2.5));
    assert!((level(4.5)).abs() < 1e-6, "low is zero: {}", level(4.5));
}

#[test]
fn the_buck_control_loop_regulates_at_3_3_volts_with_a_duty_of_0_28() {
    // mixed#8 (run): the converter, the controller and the load, without the `digital for Buck` check
    // ("Leave the `digital for Buck` block out and the converter runs, loop records and all"); mixed#10
    // (refused): with it the file is `P0001`, the `settle` event (L52); mixed~9d07c4b9 "It checks, at every
    // time point after start-up, that the output stays within 50 mV of the target" (refused with it);
    // mixed~758eca03 "A buck converter steps 12 V down to 3.3 V ... the output settles near d . 12 V";
    // mixed~a32dcb77 "emit!(\"loop\", ...) is the window into the loop: one record per period with the sample,
    // the error and the duty just decided. duty = d prints the new value"; mixed~3bce4014 "At the end the error
    // is zero and the duty sits where 12 V times the duty, less the small drop in the switch, equals 3.3 V:
    // about 0.28"
    let chapter = Chapter::read();
    Refusal::expect(chapter.marked("buck.msl", "mixed#10").run(Commands::buck()), "mixed#10", "P0001", "built-in `settle` is not supported yet");
    let records = Buck::loop_records(&chapter.marked("buck.msl", "mixed#8"));
    assert_eq!(records.len(), 1201, "one record per switching period of 10 us over 12 ms, the first at 0");
    assert!((records[1].0 - 1e-5).abs() < 1e-12 && (records[1200].0 - 12e-3).abs() < 1e-9, "the periods");
    let (_, vout, err, duty) = records[1200];
    assert!((vout - 3.3).abs() < 1e-3, "the output settles at 3.3 V: {vout}");
    assert!(err.abs() < 1e-3, "the error is zero at the end: {err}");
    assert!((duty - 0.28).abs() < 0.005, "the duty sits at about 0.28: {duty}");
    assert!((vout - duty * 12.0).abs() < 0.1, "near d . 12 V (the switch's drop is the difference): {} against {vout}", duty * 12.0);
    for pair in records.windows(2).skip(2).take(300) {
        let expected = (pair[0].3 + 1e-3 * pair[1].2).clamp(0.0, 0.9);
        assert!((pair[1].3 - expected).abs() < 1e-12, "`duty = d` is the new value: the duty of one record plus ki times the next error: {pair:?}");
    }
}

#[test]
fn the_buck_start_up_does_not_climb_by_0_0033_per_period_today() {
    // mixed~68328637: "Read the duty field down the lines: it climbs by 0.0033 per period at first and
    // settles near 0.28, while the Buck.vout column of the table rises to 3.3". The first record is the
    // Book's (t = 0, vout 0, err 3.300, duty 0.0033) and the end is the Book's (above), but the run starts
    // from an operating point where the controller's first `hs <- true` has already been applied: the
    // output is 11.8 V, the second record (10 us) reads vout 11.6 V, err -8.3 V and a duty of 0 (clamped),
    // not 0.0066, and the table's first row is 11.8 V, not 0 (D1; Phase 9: the operating point is the
    // initial state, reactions at t = 0 follow it)
    let chapter = Chapter::read();
    let model = chapter.marked("buck.msl", "mixed#8");
    let records = Buck::loop_records(&model);
    Printed::holds("3.299960e0", records[0].2, "the first error, the Book's 3.300");
    Printed::holds("3.299960e-3", records[0].3, "the first duty, the Book's 0.0033");
    assert!((records[1].1 - 11.56).abs() < 0.05 && (records[1].2 + 8.26).abs() < 0.05, "10 us: vout and err today: {:?}", records[1]);
    assert_eq!(records[1].3, 0.0, "the duty of the second record, which the Book says is 0.0066");
    let ran = model.run(Commands::buck()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.real(0, "Buck.vout") - 11.82).abs() < 0.05, "the table's first row, which the Book has near 0: {}", ran.real(0, "Buck.vout"));
    assert!((ran.real(ran.table.rows.len() - 1, "Buck.vout") - 3.3).abs() < 1e-3, "the column ends at 3.3");
}

#[test]
fn a_faster_integral_gain_makes_the_buck_loop_swing() {
    // mixed~41a87153: "In the buck, raise ki to 20m and look at the loop stream: does duty still settle, or
    // does it swing around 0.28? (It swings: the loop now reacts faster than the LC filter can follow.)": over
    // the last two milliseconds the duty still covers most of its range, and the output swings by volts
    let chapter = Chapter::read();
    let records = Buck::loop_records(&Buck::with_gain(&chapter, "20m"));
    let last: Vec<&(f64, f64, f64, f64)> = records.iter().filter(|record| record.0 > 10e-3).collect();
    let duties = last.iter().map(|record| record.3);
    let swing = duties.clone().fold(f64::NEG_INFINITY, f64::max) - duties.fold(f64::INFINITY, f64::min);
    assert!(swing > 0.5, "the duty does not settle: it covers {swing} of its range of 0 to 0.9");
    let outputs = last.iter().map(|record| record.1);
    let volts = outputs.clone().fold(f64::NEG_INFINITY, f64::max) - outputs.fold(f64::INFINITY, f64::min);
    assert!(volts > 5.0, "the output swings by {volts} V");
}

#[test]
fn the_delta_sigma_modulator_averages_its_input_and_starts_held_at_zero() {
    // mixed#14 (run): `DeltaSigma1` with a clock; mixed~8ab17f2d "x has one branch, so KCL gives c . dv(x)/dt
    // = (v(vin) - fb) / r": with vref = 1 V and 0.3 V in, the loop keeps the integrator bounded by driving the
    // bit high 65 % of the clocks; mixed~ac0aefe3 "Its = 0.0 holds it at 0 V through the transient's
    // operating point": the transient runs, where a Dc solve is singular and names `x` (the Book's step 6)
    let chapter = Chapter::read();
    let clock = chapter.text("from-rust#2");
    let clock = clock.split_once("/// Counts rising edges").expect("the clock comes first").0;
    let after = format!("{SOURCE}module Top() {{ signal vin: Electrical; signal clk: Bit; signal bit: Bit; let s = Vdc(vin) {{ dc: 0.3 }}; let m = DeltaSigma1(vin, clk, bit); let o = Osc(clk) {{ f: 1M }}; }}\n{clock}");
    let model = chapter.wrapped("delta_sigma.msl", "", "mixed#14", &after);
    let ran = model.run(Commands::transient("Top", 200e-6, Some(1e-6))).unwrap_or_else(|failure| panic!("{failure}"));
    let high = (0..ran.table.rows.len()).filter(|row| ran.cell(*row, "Top.bit") == &Cell::Bool(true)).count();
    let density = high as f64 / ran.table.rows.len() as f64;
    assert!((density - 0.65).abs() < 0.03, "the average of the bit is (0.3 / 1 + 1) / 2: {density}");
    let singular = Refusal::of(model.run(Faulty::operating_point_of("Top")), "mixed~ac0aefe3");
    assert_eq!(singular.len(), 1, "{singular:?}");
    assert_eq!(singular[0].0, "E0503");
    assert!(singular[0].1.contains("node of Top.m.x"), "a Dc solve is singular and names x: {}", singular[0].1);
}

#[test]
fn the_sar_adc_is_refused_for_the_field_access_on_its_converter() {
    // mixed#15 (refused): `SarAdc` and its `analog for SarAdc { i(dac.core) +> ... }` on a public node of its
    // DAC (`field access`, L36); with the pieces the chapter does not show written by the test (a sample-hold,
    // `Dac8`, `SarLogic`) and the chapter's own `Comparator` (mixed#6), which is refused too
    let chapter = Chapter::read();
    let pieces = "module SampleHold(vin: Electrical, vout: Electrical, in clk: Bit) { signal held: Bit<f64> = 0.0; }\ndigital for SampleHold { on clk.rise() { held <- v(vin); } }\nanalog for SampleHold { v(vout) +> held.known_or(0.0); }\nmodule Dac8(in code: Bit<u8>, vout: Electrical) { pub signal core: Electrical; }\nanalog for Dac8 { v(core) +> 0.5; v(vout) +> v(core); }\nmodule SarLogic(in clk: Bit, in start: Bit, in cmp: Bit, out code: Bit<u8> = 0, out done: Bit = false) {}\n";
    let after = "module Top(in clk: Bit, in start: Bit) { signal vin: Electrical; signal code: Bit<u8>; signal done: Bit; let a = SarAdc(vin, clk, start, code, done); }\n";
    let before = format!("{}\n{pieces}", chapter.text("mixed#6"));
    let model = chapter.wrapped("sar.msl", &before, "mixed#15", after);
    let given = Refusal::of(model.run(Faulty::operating_point_of("Top")), "mixed#15");
    assert!(given.iter().any(|(code, message)| code == "E0441" && message.starts_with("an `Event` value outside an event position")), "the comparator: {given:?}");
    Refusal::expect(model.run(Faulty::operating_point_of("Top")), "mixed#15", "P0001", "field access is not supported yet");
}

#[test]
fn e0250_a_node_cannot_go_to_a_bit_port() {
    // mixed~42ab6e1d: "E0250 EdgeCounter(vin, count) with vin: Electrical": put an A2D in between
    let source = "module EdgeCounter(in pulse: Bit, out count: Bit<u16> = 0) {}\nmodule Top() { signal vin: Electrical; signal count: Bit<u16>; let c = EdgeCounter(vin, count); }\n";
    let faulty = Faulty::elaborating("mixed~42ab6e1d", source, "Top");
    assert_eq!(faulty.first("E0250").message, "a node is connected to a port of a discrete nature (§6.1)");
}

#[test]
fn e0430_a_comparison_is_a_bool_and_the_crossing_is_the_event() {
    // mixed~54ec481d: "E0430 on v(vin) > 0.1 { ... }": the event is the crossing, `on above(v(vin) - 0.1)`;
    // mixed~fa428686 "For 'when the voltage reaches...' use above": the fix runs and fires at the start,
    // where the voltage is already above
    let faulty = Faulty::elaborating("mixed~54ec481d", "module Top() { signal vin: Electrical; }\ndigital for Top { on v(vin) > 0.1 { } }\n", "Top");
    assert_eq!(faulty.first("E0430").message, "the head of `on` has type `bool`: it must be an `Event`; a `bool` alone is not an event (§12.1.10)");
    let fixed = format!("{SOURCE}module Top() {{ signal vin: Electrical; let s = Vdc(vin) {{ dc: 0.5 }}; }}\ndigital for Top {{ on above(v(vin) - 0.1) {{ emit!(\"t\", t = now()); }} }}\n");
    let ran = Model::written("above.msl", &fixed).run(Commands::transient("Top", 3e-5, None)).unwrap_or_else(|failure| panic!("`above` is an event: {failure}"));
    assert_eq!(ran.stream("t").len(), 1, "one crossing, at time 0");
}

#[test]
fn e0336_a_definition_cannot_read_a_probe() {
    // mixed~4bcf1b1d: "E0336 hit = v(vin) > 0.1; in a module body": a definition is digital
    let faulty = Faulty::elaborating("mixed~4bcf1b1d", "module Top() { signal vin: Electrical; signal hit: Bit; hit = v(vin) > 0.1; }\n", "Top");
    assert!(faulty.first("E0336").message.starts_with("a definition reads a probe, `now()`"), "{}", faulty.first("E0336").message);
}

#[test]
fn e0316_an_in_port_takes_a_digital_expression_only() {
    // mixed~4bc81b1d: "E0316 EdgeCounter(v(vin) > 0.1, count)": the same mistake through a port
    let source = "module EdgeCounter(in pulse: Bit, out count: Bit<u16> = 0) {}\nmodule Top() { signal vin: Electrical; signal count: Bit<u16>; let c = EdgeCounter(v(vin) > 0.1, count); }\n";
    let faulty = Faulty::elaborating("mixed~4bc81b1d", source, "Top");
    assert!(faulty.first("E0316").message.starts_with("a continuous expression is connected to an `in` port"), "{}", faulty.first("E0316").message);
}

#[test]
fn e0150_an_unknown_switch_control_is_read_with_high() {
    // mixed~39a9371d: "E0150 i(a, b) +> v(a, b) / if ctl { ron } else { roff };": say what an unknown ctl
    // means, `ctl.high()`
    let source = "module Sw(a: Electrical, b: Electrical, in ctl: Bit) { ron: f64 = 50m; roff: f64 = 1M; }\nanalog for Sw { i(a, b) +> v(a, b) / if ctl { ron } else { roff }; }\nmodule Top(in c: Bit) { signal a: Electrical; signal b: Electrical; let s = Sw(a, b, c); }\n";
    let faulty = Faulty::elaborating("mixed~39a9371d", source, "Top");
    assert!(faulty.first("E0150").message.contains("reads the discrete carrier `ctl`"), "{}", faulty.first("E0150").message);
}

#[test]
fn e0442_at_takes_a_discrete_time() {
    // mixed~54d4401d: "E0442 on at(v(vin)) { ... }": at takes a time computed from digital values
    let faulty = Faulty::elaborating("mixed~54d4401d", "module Top() { signal vin: Electrical; }\ndigital for Top { on at(v(vin)) { } }\n", "Top");
    assert!(faulty.first("E0442").message.starts_with("the argument of `at` reads a probe"), "{}", faulty.first("E0442").message);
}

#[test]
fn e0350_every_is_not_placed_in_a_fn_and_a_named_tick_runs() {
    // mixed~4bd60d1d: "E0350 on every(0.0, 10u) { ... } written inside another reaction, or every called in
    // a fn": an `every` called in a `fn` is `E0350`; mixed~6430a6c6 "Name them in the module body: let tick
    // = every(0.0, 10u);": the named tick runs, firing at 0, 10, 20 and 30 us
    let in_function = "fn f() -> f64 { let t = every(0.0, 10u); 1.0 }\nmodule Top() { signal vin: Electrical; }\nanalog for Top { v(vin) +> f(); }\n";
    let faulty = Faulty::elaborating("mixed~4bd60d1d", in_function, "Top");
    assert_eq!(faulty.first("E0350").message, "`every` is placed in a `fn` or a closure: it places no instance (§8.3.4)");
    let named = format!("{SOURCE}module Top() {{ signal vin: Electrical; let s = Vdc(vin); let tick = every(0.0, 10u); }}\ndigital for Top {{ on tick {{ emit!(\"t\", t = now()); }} }}\n");
    let ran = Model::written("tick.msl", &named).run(Commands::transient("Top", 3e-5, None)).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.stream("t").len(), 4, "at 0, 10, 20 and 30 us");
}

#[test]
fn an_every_written_inside_a_reaction_is_e0404_not_e0350() {
    // mixed~98e4dc1b: "on every(0.0, 10u) { ... } written inside another reaction, or every called in a fn"
    // (the code of the row is E0350): the first form is `E0404`, `on` in a reaction, a syntax error, not
    // `E0350` (D1; Phase 9 gives the Book's code); the second is `E0350` (the test above)
    let faulty = Faulty::running("mixed~98e4dc1b", "module Top() { signal vin: Electrical; }\ndigital for Top { on at(1u) { on every(0.0, 10u) { } } }\n", Commands::transient("Top", 1e-6, None));
    assert_eq!(faulty.first("E0404").message, "`on` in a reaction");
    assert!(faulty.given.iter().all(|reported| reported.code != "E0350"), "E0350 is not given for this form");
}

#[test]
fn e0504_the_step_after_a_jump_that_does_not_converge_names_the_carrier() {
    // mixed~5c44ed1d: "E0504 a run error at a digital edge": a node with no solution after the carrier `lvl`
    // jumps (`v^2 + v/1k + lvl = 0` has no real root at lvl = 1) fails at the smallest step, and the error
    // names the delivery of `Top.lvl`
    let source = "module Top() { signal b: Electrical; signal lvl: Bit<f64> = 0.0; }\nanalog for Top { i(b, gnd) +> v(b) * v(b) + v(b) / 1k + lvl.known_or(0.0); }\ndigital for Top { on at(1u) { lvl <- 1.0; } }\n";
    let given = Refusal::of(Model::written("jump.msl", source).run(Commands::transient("Top", 3e-6, Some(1e-6))), "mixed~5c44ed1d");
    assert_eq!(given.len(), 1, "{given:?}");
    assert_eq!(given[0].0, "E0504");
    assert!(given[0].1.contains("the step after a jump fails at the smallest step; it reads the delivery of Top.lvl"), "{}", given[0].1);
}
