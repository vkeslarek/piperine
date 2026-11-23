//! Scope: chapter 8 of the Book, "Analog behavior" (feature `root-e2e`, RE-02 and RE-03): the capacitor
//! and the inductor, the Nullor and the coupled inductors, the clipper (its `limexp` and its `settle`
//! run since `unknown-semantics-events`; its numbers are held on the Book's own text), the traced diode,
//! the switch and the noise source (refused),
//! the `ac_stim` source, and each mistake of the chapter's table, run through `piperine::`. Each test
//! opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

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
use support::model::{Model, Ran};
use support::printed::Printed;
use support::records::Fields;
use support::refusal::Refusal;

/// The source and the resistor most models of the chapter hang their device on.
struct Bench;

impl Bench {
    /// A DC source `Vdc` and a resistor `R` (each with its behavior block).
    const PARTS: &'static str = "module Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc + ac_stim(1.0, 0.0); }\nmodule R(p: Electrical, n: Electrical) { r: f64 = 1k; }\nanalog for R { i(p, n) +> v(p, n) / r; }\n";

    /// A top `Top` with the two signals `a` and `b` and no body, which a behavior block of the test
    /// fills.
    const EMPTY_TOP: &'static str = "module Top() { signal a: Electrical; signal b: Electrical; }\n";
}

/// The runs the chapter's commands make.
struct Commands;

impl Commands {
    /// An operating point of the top `Top`.
    fn operating_point() -> Overrides {
        Faulty::operating_point_of("Top")
    }

    /// `--analysis transient --stop <stop>` of the top `Top`.
    fn transient(stop: f64) -> Overrides {
        Faulty::transient_of_top(stop, None)
    }

    /// `piperine run clipper.msl --analysis transient --stop 2e-3 --print vin --print vout --stream clip`,
    /// the run of the Run it box (its `--plot` is a file the library does not write).
    fn clipper() -> Overrides {
        Overrides { analysis: Some("transient".to_string()), stop: Some(2e-3), print: vec!["vin".to_string(), "vout".to_string()], streams: vec!["clip".to_string()], ..Overrides::default() }
    }

    /// An AC sweep over one decade, from 1 kHz to 10 kHz, of the top `Top`.
    fn small_signal() -> Overrides {
        Overrides { analysis: Some("ac".to_string()), top: Some("Top".to_string()), from: Some(1e3), to: Some(1e4), per_decade: Some(1), ..Overrides::default() }
    }
}

/// The clipper of the chapter's two listings, as the Book writes them and with its second diode
/// changed.
struct Clipper;

impl Clipper {
    /// The clipper as the Book assembles it in `clipper.msl`.
    fn book(chapter: &Chapter) -> Model {
        chapter.marked("clipper.msl", "analog#3")
    }

    /// The Book's clipper with `d2` written `second_diode` instead of `Diode(gnd, vout);`.
    fn with_second_diode(chapter: &Chapter, second_diode: &str) -> Model {
        let device = chapter.text("analog#2").replace("Diode(gnd, vout);", second_diode);
        assert!(device.contains(second_diode), "the substitute applies to the Book's text");
        Model::written("clipper.msl", &format!("{device}\n{}\n", chapter.text("analog#3")))
    }

    /// The least and the greatest value of the column `column` of `ran`.
    fn extremes(ran: &Ran, column: &str) -> (f64, f64) {
        let values: Vec<f64> = (0..ran.table.rows.len()).map(|row| ran.real(row, column)).collect();
        (values.iter().copied().fold(f64::INFINITY, f64::min), values.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    }
}

#[test]
fn a_capacitor_is_open_and_an_inductor_is_a_short_at_the_operating_point() {
    // analog#1 (run): the Capacitor and the Inductor of the first listing: "ddt is zero, so a capacitor is
    // open and an inductor is a short, with nothing written by you". A source into a resistor, the
    // inductor to ground (a short: 0 V) and the capacitor to ground (open: the source's 1 V)
    let chapter = Chapter::read();
    let after = format!("{}module Top() {{ signal a: Electrical; signal b: Electrical; let s = Vdc(a); let r = R(a, b); let l = Inductor(b, gnd) {{ l: 1m }}; let c = Capacitor(a, gnd) {{ c: 1n }}; }}\n", Bench::PARTS);
    let ran = chapter.wrapped("reactive.msl", "", "analog#1", &after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!(ran.signal("Top.b").abs() < 1e-9, "the inductor shorts b to ground: {:e}", ran.signal("Top.b"));
    assert!((ran.signal("Top.a") - 1.0).abs() < 1e-9, "the capacitor draws nothing: a stays at the source's 1 V");
}

#[test]
fn the_clipper_prints_one_clip_record_per_period_near_0_72_volts() {
    // analog#2 and analog#3 (run since unknown-semantics-events T17 and T5): the Book's clipper with
    // `limexp` and its `settle` reaction (analog~750ad45d "The command runs: it prints the two clip records,
    // then the table of the rows, and writes the plot"). UE-06 AC 1: one `clip` record per period of the 1 kHz sine over
    // 2 ms, `vin` within 50 mV of 0.72 V (analog~8f90421c, analog~9e74b878), and one `wave` record per time
    // point (the reaction on `settle`, section 15 step 4): the table prints a row at each
    // accepted step and the `wave` stream holds one record at each of them
    let chapter = Chapter::read();
    let mut command = Commands::clipper();
    command.streams.push("wave".to_string());
    let ran = Clipper::book(&chapter).run(command).unwrap_or_else(|failure| panic!("{failure}"));
    let clips = ran.stream("clip");
    assert_eq!(clips.len(), 2, "one clip record per period over two periods");
    for clip in &clips {
        assert!((Fields::real(clip, "vin") - 0.72).abs() < 0.05, "vin within 50 mV of 0.72 V: {:?}", clip.fields);
    }
    let waves = ran.stream("wave");
    let time_points = ran.table.rows.len();
    assert!(time_points > 100, "a transient of 2 ms has many time points: {time_points}");
    assert_eq!(waves.len(), time_points, "one wave record per time point");
}

#[test]
fn the_clipper_clips_near_the_forward_voltage_and_the_clip_record_confirms_the_hand_calculation() {
    // On the Book's clipper (analog#2 and analog#3): analog~53c7822c "A 2 V sine drives a resistor into two
    // diodes back to back"; analog~a6eeb69c "The output follows the input while it is small and is clipped
    // near +-0.65 V"; analog~72e8923f "In an operating-point solve now() is 0 and the source sits at 0 V";
    // analog~8f90421c "The clip stream answers a sharper question ... when does the clipping start, and how
    // high is the input then?"; analog~9e74b878 "At 0.6 V the diode carries about 0.12 mA, which drops 0.12
    // V over the resistor, so the input is at 0.72 V: the record confirms the hand calculation"
    let chapter = Chapter::read();
    let ran = Clipper::book(&chapter).run(Commands::clipper()).unwrap_or_else(|failure| panic!("{failure}"));
    let (least, greatest) = Clipper::extremes(&ran, "Clipper.vout");
    assert!((greatest - 0.65).abs() < 0.05 && (least + 0.65).abs() < 0.05, "clipped near +-0.65 V: {least} to {greatest}");
    let (input_least, input_greatest) = Clipper::extremes(&ran, "Clipper.vin");
    assert!(input_greatest > 1.9 && input_least < -1.9, "the 2 V sine itself: {input_least} to {input_greatest}");
    let point = Clipper::book(&chapter).run(Faulty::operating_point_of("Clipper")).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(point.signal("Clipper.vin"), 0.0, "now() is 0 in the operating point and the source sits at 0 V");
    let clips = ran.stream("clip");
    assert_eq!(clips.len(), 2, "one clip record per period over two periods");
    Printed::holds("5.850282e-5", Fields::real(clips[0], "t"), "the first crossing, about 59 us");
    for clip in &clips {
        assert!((Fields::real(clip, "vin") - 0.72).abs() < 0.005, "the input at 0.6 V of output is about 0.72 V: {:?}", clip.fields);
    }
    assert!((clips[1].time - clips[0].time - 1e-3).abs() < 1e-6, "once per period of 1 kHz");
    let diode_current: f64 = (0.72 - 0.6) / 1e3;
    assert!((diode_current - 0.12e-3).abs() < 1e-9, "0.12 V over 1 k is 0.12 mA");
}

#[test]
fn a_louder_second_diode_clips_the_negative_half_higher() {
    // analog~e79bf782: "Give d2 a different diode: Diode(gnd, vout) { n: 2.0 }. Which half of the output
    // wave is clipped higher now, and at roughly what voltage? (The negative half: an ideality factor of 2
    // doubles the voltage for the same current, to about 1.3 V.)" On the Book's clipper with that second diode
    let chapter = Chapter::read();
    let ran = Clipper::with_second_diode(&chapter, "Diode(gnd, vout) { n: 2.0 };").run(Commands::clipper()).unwrap_or_else(|failure| panic!("{failure}"));
    let (least, greatest) = Clipper::extremes(&ran, "Clipper.vout");
    assert!((least + 1.3).abs() < 0.05, "the negative half is clipped near -1.3 V: {least}");
    assert!(greatest < 0.7, "the positive half is still clipped near 0.65 V: {greatest}");
}

#[test]
fn the_thermal_voltage_is_about_25_9_millivolts_at_the_default_temperature() {
    // analog~36d33f2a: "let nvt = n * vt(); A let inside analog is a local of this iteration. vt() is the
    // thermal voltage kT/q at the simulation temperature, about 25.9 mV at the default 300.15 K": a node
    // driven to `1.0 * vt()` reads 25.86 mV
    let source = "module Top() { signal a: Electrical; }\nanalog for Top { let nvt = 1.0 * vt(); v(a) +> nvt; }\n";
    let ran = Model::written("vt.msl", source).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("2.586493e-2", ran.signal("Top.a"), "kT/q at 300.15 K");
    assert!((ran.signal("Top.a") - 25.9e-3).abs() < 0.1e-3, "about 25.9 mV");
}

#[test]
fn the_diode_with_a_trace_runs() {
    // analog#7 (runs since analog-operators T37): the Diode with `emit!("newton", ..)` in its `analog` block is a
    // trace, written at every evaluation (analog~139c67b5 "This emit! runs when the code around it runs, and
    // analog runs at every evaluation"); the operating point runs and reads the same 0.4 to 0.7 V as without it
    let chapter = Chapter::read();
    let before = "module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\nmodule Diode(a: Electrical, k: Electrical) { is: f64 = 1e-14 where is > 0.0; n: f64 = 1.0 where n > 0.0; }\n";
    let after = format!("{}module Top() {{ signal a: Electrical; signal b: Electrical; let s = Vdc(a); let r = Resistor(a, b) {{ r: 1k }}; let d = Diode(b, gnd); }}\n", Bench::PARTS);
    let model = chapter.wrapped("traced.msl", before, "analog#7", &after);
    let ran = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    let forward = ran.signal("Top.b");
    assert!(forward > 0.4 && forward < 0.8, "the diode's forward voltage: {forward}");
}

#[test]
fn the_switch_shorts_its_nodes_while_its_control_is_high() {
    // analog#9 (run since `analog-structure`, reading `ctl` directly since `explicit-unknowns` T20): the ideal switch, `if ctl.high() { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; }`,
    // behind 1 kOhm from 1 V: with its control high it shorts `a` to ground (v(a) = 0), with it low it is open
    // (no flow, v(a) = 1 V)
    let chapter = Chapter::read();
    let top = |ctl: &str| format!("module Top() {{ signal s: Electrical; signal a: Electrical; signal c: Bit = {ctl}; let v1 = Vdc(s) {{ dc: 1.0 }}; let r = R(s, a) {{ r: 1k }}; let sw = Switch(a, gnd, c); }}\n");
    for (ctl, volts) in [("true", 0.0), ("false", 1.0)] {
        let model = chapter.wrapped("switch.msl", Bench::PARTS, "analog#9", &top(ctl));
        let ran = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
        assert!((ran.signal("Top.a") - volts).abs() < 1e-6, "control {ctl}: v(a) = {:e}, wanted {volts}", ran.signal("Top.a"));
    }
}

#[test]
fn the_nullor_drives_its_output_to_a_virtual_short_and_coupled_inductors_couple() {
    // analog#10 (run): the Servo nature, the Nullor and the Coupled inductors. As a non-inverting amplifier of
    // gain 2 the Nullor holds v(inp, inn) at zero: a 0.5 V input gives 1 V at the output. Two inductors
    // with k = 0.5 pass a small signal to the secondary, and with k = 0 none
    let chapter = Chapter::read();
    let top = |k: &str| format!("module Top() {{\n    signal vin: Electrical; signal fb: Electrical; signal vout: Electrical;\n    signal src2: Electrical; signal p1: Electrical; signal p2: Electrical;\n    let s = Vdc(vin) {{ dc: 0.5 }};\n    let amp = Nullor(vin, fb, vout);\n    let r1 = R(vout, fb) {{ r: 1k }};\n    let r2 = R(fb, gnd) {{ r: 1k }};\n    let v2 = Vdc(src2) {{ dc: 1.0 }};\n    let r3 = R(src2, p1) {{ r: 1k }};\n    let l = Coupled(p1, gnd, p2, gnd) {{ l1: 1m, l2: 1m, k: {k} }};\n    let r4 = R(p2, gnd) {{ r: 1k }};\n}}\n");
    let coupled = chapter.wrapped("nullor.msl", Bench::PARTS, "analog#10", &top("0.5"));
    let ran = coupled.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.vout") - 1.0).abs() < 1e-9, "the Nullor's gain of 2 on 0.5 V: {:e}", ran.signal("Top.vout"));
    assert!((ran.signal("Top.fb") - 0.5).abs() < 1e-9, "the nullor holds v(inp, inn) at zero");
    let secondary = |model: &Model| {
        let sweep = Overrides { print: vec!["p2".to_string()], from: Some(1e5), to: Some(1e6), ..Commands::small_signal() };
        let ran = model.run(sweep).unwrap_or_else(|failure| panic!("{failure}"));
        match ran.cell(0, "mag(Top.p2)") {
            Cell::Real(magnitude) => *magnitude,
            other => panic!("a magnitude: {other:?}"),
        }
    };
    assert!(secondary(&coupled) > 0.1, "k = 0.5 couples the secondary");
    let uncoupled = chapter.wrapped("nullor.msl", Bench::PARTS, "analog#10", &top("0.0"));
    assert_eq!(secondary(&uncoupled), 0.0, "k = 0 couples nothing");
}

#[test]
fn the_noise_source_runs_and_reads_as_zero_outside_a_noise_sweep() {
    // analog#11 (runs since analog-operators T34): `let n = noise(4.0 * K_B * temperature() / r, "thermal");`
    // and its two uses in a behavior block: the source is 0 in every solve but a noise sweep, so the two
    // contributions between `a`, `mid` and `b` carry no current at the operating point. `mid` has the one
    // resistor to ground the wrapper gives it, so the point is solvable: every node is at 0 V.
    let chapter = Chapter::read();
    let before = "module R(a: Electrical, b: Electrical) { r: f64 = 1k; signal mid: Electrical; }\nanalog for R {\ni(mid, gnd) +> v(mid, gnd) / r;\n";
    let model = chapter.wrapped("noise.msl", before, "analog#11", "}\nmodule Top() { signal a: Electrical; signal b: Electrical; let x = R(a, b); let la = Load(a); let lb = Load(b); }\nmodule Load(p: Electrical) {}\nanalog for Load { i(p, gnd) +> v(p, gnd) / 1k; }\n");
    let point = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    for row in &point.table.rows {
        for cell in row.iter().skip(1) {
            if let Cell::Real(volts) = cell {
                assert!(volts.abs() <= 1e-12, "a noise source carries no current at the operating point: {row:?}");
            }
        }
    }
}

#[test]
fn a_source_with_ac_stim_is_its_dc_value_and_a_one_volt_phasor_in_the_sweep() {
    // analog~239192ff: "v(p, n) +> dc + ac_stim(1.0, 0.0); is a DC source that becomes a 1 V phasor in the
    // sweep and stays at dc everywhere else": a 2 V source into two equal resistors reads 2 V and 1 V at
    // the operating point and in a transient, and a magnitude of 1 and 0.5 with phase 0 in the AC sweep
    let source = format!("{}module Top() {{ signal a: Electrical; signal b: Electrical; let s = Vdc(a) {{ dc: 2.0 }}; let r1 = R(a, b); let r2 = R(b, gnd); }}\n", Bench::PARTS);
    let model = Model::written("stimulus.msl", &source);
    let point = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!((point.signal("Top.a"), point.signal("Top.b")), (2.0, 1.0), "dc everywhere else");
    let transient = model.run(Commands::transient(1e-6)).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(transient.real(transient.table.rows.len() - 1, "Top.a"), 2.0, "dc in a transient");
    let sweep = model.run(Commands::small_signal()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(sweep.table.rows.len(), 2, "the two ends of one decade");
    for row in 0..2 {
        assert!((sweep.real(row, "mag(Top.a)") - 1.0).abs() < 1e-9, "a 1 V phasor");
        assert!((sweep.real(row, "mag(Top.b)") - 0.5).abs() < 1e-9, "half of it across the divider");
        assert_eq!(sweep.real(row, "phase(Top.a)"), 0.0, "phase 0.0");
    }
}

#[test]
fn the_sampling_time_constant_of_a_switch_is_a_tenth_of_a_nanosecond() {
    // analog~7b3efc50: "Sampling, phi1 high. x follows vin and y is grounded, so cs charges to v(vin), with a
    // time constant of ron . cs = 0.1 ns per switch": the integrator's switch `ron` is 100 and `cs` 1 pF
    let chapter = Chapter::read();
    let capacitor = "module Capacitor(p: Electrical, n: Electrical) { c: f64 where c > 0.0; }\nanalog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }\n";
    let after = "module Top(in clk: Bit = false) { signal a: Electrical; signal b: Electrical; let s = ScIntegrator(a, b, clk); }\n";
    let ran = chapter.wrapped("sc.msl", capacitor, "modules#9", after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    let param = |path: &str| match ran.metadata.params.iter().find(|param| param.path == path).unwrap_or_else(|| panic!("no param {path}")).value {
        Cell::Real(value) => value,
        ref other => panic!("{path}: a real number, found {other:?}"),
    };
    let tau = param("Top.s.s1.ron") * param("Top.s.cs");
    Printed::holds("1.000000e-10", tau, "ron * cs, in seconds");
}

#[test]
fn e0410_a_contribution_writes_a_probe_of_a_branch() {
    // analog~54e53c1d: "E0410 vout +> 1.0;": the left side must be a probe: v(vout) +> 1.0
    let faulty = Faulty::elaborating("analog~54e53c1d", "module Top(vout: Electrical) { }\nanalog for Top { vout +> 1.0; }\n", "Top");
    assert_eq!(faulty.first("E0410").message, "the left side of `+>` is not a potential or flow probe of a branch");
}

#[test]
fn e0417_a_contribution_is_an_f64_and_the_fix_runs() {
    // analog~54e5351d and analog~ecb4300e: "E0417 i(p, n) +> 1;": write 1.0, or an SI literal such as 1m
    let faulty = Faulty::elaborating("analog~54e5351d", &format!("{}analog for Top {{ i(a, b) +> 1; }}\n", Bench::EMPTY_TOP), "Top");
    assert_eq!(faulty.first("E0417").message, "mismatched types: expected `f64`, found `an integer literal`");
    for literal in ["1.0", "1m"] {
        let source = format!("module Top() {{ signal a: Electrical; }}\nanalog for Top {{ i(a, gnd) +> v(a, gnd) / 1k - {literal}; }}\n");
        Model::written("fixed.msl", &source).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("`{literal}` is an f64: {failure}"));
    }
}

#[test]
fn e0404_a_commit_belongs_in_a_reaction() {
    // analog~54e1b21d: "E0404 held <- v(vin); in analog": analog writes only with +>
    let faulty = Faulty::elaborating("analog~54e1b21d", "module Top(vin: Electrical) { signal held: Bit<f64> = 0.0; }\nanalog for Top { held <- v(vin); }\n", "Top");
    assert_eq!(faulty.first("E0404").message, "a commit (`<-`) in an `analog` block");
    assert_eq!(faulty.first("E0404").help.as_deref(), Some("a commit belongs in a reaction (`on event { .. }`) of a `digital` block; an `analog` block contributes with `+>`"));
}

#[test]
fn a_bit_in_an_if_is_read_as_its_value() {
    // analog~39a9371d (the former E0150 row, `if ctl { ... }` in `analog` with `ctl` a `Bit`): the read needs no gate
    // (part I section 3.3.6), so the source runs and `v(a)` is the arm the `Bit` picks
    let source = "module Sw(p: Electrical, in ctl: Bit) { }\nanalog for Sw { v(p) +> if ctl { 1.0 } else { 0.0 }; }\nmodule Top() { signal a: Electrical; signal c: Bit = true; let s = Sw(a, c); }\n";
    let ran = Model::written("m.msl", source).run(Faulty::operating_point_of("Top")).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.a") - 1.0).abs() < 1e-9, "a true `Bit` picks the first arm: {:e}", ran.signal("Top.a"));
}

#[test]
fn e0414_a_branch_that_switches_kind_on_a_probe_is_refused() {
    // analog~54e5381d and analog~c91537fe: "E0414 if v(a, b) > 0.0 { v(a, b) +> 0.0; } else { i(a, b) +> 0.0;
    // }": "A choice of kind that depends on a probe is an error (E0414)": raised since `analog-structure`, at
    // each contribution under the selector, naming the branch and the probe
    let source = format!("{}analog for Top {{ if v(a, b) > 0.0 {{ v(a, b) +> 0.0; }} else {{ i(a, b) +> 0.0; }} }}\n", Bench::EMPTY_TOP);
    let faulty = Faulty::elaborating("analog~54e5381d", &source, "Top");
    assert_eq!(faulty.given.len(), 2, "one `E0414` per contribution");
    assert!(faulty.given.iter().all(|reported| reported.code == "E0414" && reported.message.contains("the branch (a, b) reads the probe of (a, b)")), "{:?}", faulty.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
}

#[test]
fn e0241_a_branch_joins_two_different_nodes() {
    // analog~42a7e91d: "E0241 i(a, a) +> x;"
    let faulty = Faulty::elaborating("analog~42a7e91d", "module Top() { signal a: Electrical; }\nanalog for Top { i(a, a) +> 1.0; }\n", "Top");
    assert_eq!(faulty.first("E0241").message, "a contribution to a branch from a node to itself");
}

#[test]
fn e0603_checks_are_decided_by_reactions() {
    // analog~64ee8b1d: "E0603 require("ok", v(a) < 1.0); in analog": move it into a reaction
    let faulty = Faulty::elaborating("analog~64ee8b1d", "module Top() { signal a: Electrical; }\nanalog for Top { require(\"ok\", v(a) < 1.0); }\n", "Top");
    assert_eq!(faulty.first("E0603").message, "a site reached from `analog` writes the stream `checks`: only a reaction decides what the host reads there (§19 rules 4 and 14)");
}

#[test]
fn e0503_a_node_only_capacitors_touch_is_singular_at_dc() {
    // analog~5c44f21d: "E0503 a run error naming a node": a node that only a capacitor touches, in a DC
    // solve, is singular and the error names the node
    let source = "module C(p: Electrical, n: Electrical) { c: f64 = 1n; }\nanalog for C { i(p, n) +> ddt(c * v(p, n)); }\nmodule Top() { signal a: Electrical; let x = C(a, gnd); }\n";
    let given = Refusal::of(Model::written("floating.msl", source).run(Commands::operating_point()), "analog~5c44f21d");
    assert_eq!(given.len(), 1, "{given:?}");
    assert_eq!(given[0].0, "E0503");
    assert!(given[0].1.contains("node of Top.x.p"), "the run error names the node: {}", given[0].1);
}

#[test]
fn e0507_a_non_finite_contribution_is_a_run_error_naming_it() {
    // analog~5c44ee1d: "E0507 a run error naming a contribution": a division by a voltage that is zero
    let source = "module Top() { signal a: Electrical; signal b: Electrical; }\nanalog for Top { v(a) +> 1.0; i(b, gnd) +> 1.0 / (v(a) - 1.0); }\n";
    let given = Refusal::of(Model::written("nan.msl", source).run(Commands::operating_point()), "analog~5c44ee1d");
    assert_eq!(given.len(), 1, "{given:?}");
    assert_eq!(given[0].0, "E0507");
    assert!(given[0].1.contains("term `b1.flow`") && given[0].1.contains("is not finite"), "the error names the contribution: {}", given[0].1);
}

#[test]
fn e0180_a_logic_read_in_analog_is_a_mismatch() {
    // analog~39ba351d (E0180): the mistakes table's "E0180 if flags { … } in analog, flags a Bit<Logic>": a `Logic` where a
    // `bool` is required is E0180. The library struct `Logic` is a type since composite-values CV-09 (it was `P0001`
    // before), so the code is raised and names both types.
    let faulty = Faulty::running("analog~39ba351d", "module Top(in flags: Bit<Logic>) { signal a: Electrical; }\nanalog for Top { if flags { v(a) +> 1.0; } }\n", Faulty::operating_point_of("Top"));
    assert_eq!(faulty.first("E0180").message, "mismatched types: expected `bool`, found `struct `Logic<1>``");
}
