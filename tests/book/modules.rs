//! Scope: chapter 7 of the Book, "Modules and hierarchy" (feature `root-e2e`, RE-02 and RE-03): the
//! switched-capacitor integrator that runs, the library integrator the chapter shows, the generic
//! ladder, the module that returns a value, the memory bank, the template (all
//! refused today, asserted refused with their diagnostics), and each sentence and mistake of the
//! chapter, run through `piperine::`. Each test opens with the inventory ids it holds, from
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
    pub mod standard_library;
}

use piperine::{Field, Overrides};
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
use support::printed::Printed;
use support::records::Fields;
use support::refusal::Refusal;
use support::standard_library::StandardLibrary;

/// The parts the ladder of the chapter is made of, as the first listing declares them.
struct Parts;

impl Parts {
    /// The resistor, the capacitor, the source and the RC section of the ladder listing.
    const TEXT: &'static str = "module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\nmodule Capacitor(p: Electrical, n: Electrical) { c: f64 where c > 0.0; }\nanalog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }\nmodule Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\nmodule RcSection(a: Electrical, b: Electrical) {\n    r: f64 = 1k where r > 0.0;\n    c: f64 = 1n where c > 0.0;\n    let r1 = Resistor(a, b) { r: r };\n    let c1 = Capacitor(b, gnd) { c: c };\n}\n";

    /// The ladder of three sections written out section by section, the design the generic `Ladder`
    /// of the listing stands for while generic modules are refused.
    fn three_sections() -> String {
        format!("{}module LadderTop() {{\n    signal vin: Electrical;\n    signal t1: Electrical;\n    signal t2: Electrical;\n    pub signal vout: Electrical;\n    let src = Vdc(vin) {{ dc: 1.0 }};\n    let s0 = RcSection(vin, t1);\n    let s1 = RcSection(t1, t2);\n    let s2 = RcSection(t2, vout);\n}}\n", Self::TEXT)
    }

    /// A top `Top` that places one `RcSection` between two signals and holds `body`.
    fn with_top(body: &str) -> String {
        format!("{}module Top() {{\n    signal a: Electrical;\n    signal b: Electrical;\n{body}}}\n", Self::TEXT)
    }
}

/// The operations the chapter runs: an operating point and a DC sweep of the source.
struct Commands;

impl Commands {
    /// `piperine run <file> --analysis op`.
    fn operating_point() -> Overrides {
        Overrides { analysis: Some("op".to_string()), ..Overrides::default() }
    }

    /// `piperine run <file> --analysis dc --sweep src.dc --from 0 --to 1 --step 0.5`.
    fn sweep_of_the_source() -> Overrides {
        Overrides { analysis: Some("dc".to_string()), sweep: Some("src.dc".to_string()), from: Some(0.0), to: Some(1.0), step: Some(0.5), ..Overrides::default() }
    }

    /// An operating point of the top `Top`.
    fn of_top() -> Overrides {
        Faulty::operating_point_of("Top")
    }
}

#[test]
fn the_ladder_runs_for_an_operating_point_and_a_sweep() {
    // modules#1 (run): the ladder file; modules~b53307eb "Ladder<const N: usize> takes the number of sections as a generic
    // argument, and where N >= 1 refuses an empty ladder", and the sentence after the listing: "at DC the capacitors are open,
    // so the output equals the input at every point of the sweep". The generic module places its three sections by a
    // generation `for` (T77): both commands of the Run it box run
    let chapter = Chapter::read();
    let model = chapter.marked("ladder.msl", "modules#1");
    let point = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((point.signal("LadderTop.vout") - 1.0).abs() < 1e-9, "the output equals the 1 V input at DC");
    let sweep = model.run(Commands::sweep_of_the_source()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(sweep.table.rows.len(), 3, "one row per point: 0, 0.5 and 1 V");
    assert_eq!(sweep.table.columns[0], "LadderTop.src.dc", "the param is named by its path below the top");
    for (row, level) in [0.0, 0.5, 1.0].into_iter().enumerate() {
        assert_eq!(sweep.real(row, "LadderTop.src.dc"), level);
        assert!((sweep.real(row, "LadderTop.vout") - level).abs() < 1e-9, "the output equals the input at every point");
    }
}

#[test]
fn the_half_reactions_report_each_stage_in_time_order() {
    // modules#2 (run): the digital block that reports when each tap passes half of the step; modules~87ce6f6e "Swap the DC source
    // for the Step ... and add a block that reports when each tap passes half of the step" and modules~3f8e84ba "The stream gets
    // one record per stage": three reactions, one per tap, and one record per stage, `stage=1` to `stage=3`, in time order.
    // The source is the ladder's own with its `Vdc` swapped for a step of 1 V at 1 us (a ramp of 1 ns, which keeps the solve
    // continuous); the times the Book leaves to the reader are checked against its own rule of thumb (modules~1a89f2ad)
    let chapter = Chapter::read();
    let step = "module StepSource(p: Electrical) { at: f64 = 1u; rise: f64 = 1n; }\nanalog for StepSource { v(p) +> if now() < at { 0.0 } else if now() < at + rise { (now() - at) / rise } else { 1.0 }; }\n";
    let ladder = chapter.text("modules#1").replace("let src = Vdc(vin) { dc: 1.0 };", "let src = StepSource(vin);");
    assert_ne!(ladder, chapter.text("modules#1"), "the listing places its DC source as written");
    let model = chapter.wrapped("ladder.msl", &format!("{step}{ladder}\n"), "modules#2", "");
    let ran = model.run(Overrides { analysis: Some("transient".to_string()), stop: Some(20e-6), streams: vec!["half".to_string()], ..Overrides::default() }).unwrap_or_else(|failure| panic!("{:?}", Refusal::of(Err(failure), "modules#2")));
    let half = ran.stream("half");
    assert_eq!(half.len(), 3, "one record per tap: {:?}", half.iter().map(|record| &record.fields).collect::<Vec<_>>());
    let stages: Vec<Field> = half.iter().map(|record| Fields::of(record, "stage").clone()).collect();
    assert_eq!(stages, [Field::Int(1), Field::Int(2), Field::Int(3)], "the stages in order");
    let times: Vec<f64> = half.iter().map(|record| Fields::real(record, "t")).collect();
    assert!(times.windows(2).all(|pair| pair[0] < pair[1]), "the edge reaches the taps one after another: {times:?}");
    // 0.69 times the Elmore delay of 3, 5 and 6 times r * c after the step at 1 us: the rule over-estimates a ladder
    // (every tap is loaded by the sections behind it), so the times lie between a half of it and all of it
    for (time, elmore) in times.iter().zip([3.0, 5.0, 6.0]) {
        let rule = 0.69 * elmore * 1e3 * 1e-9;
        let after = time - 1e-6;
        assert!(after > 0.5 * rule && after < 1.1 * rule, "{after:e} s after the step, against the rule's {rule:e} s");
    }
}

#[test]
fn a_ladder_written_out_runs_an_operating_point_and_a_dc_sweep() {
    // modules~85f8dd48: "An operating point prints the top's own signals; a DC sweep steps a param, named
    // by its path below the top, and prints one row per point", and the sentence after the listing: "at
    // DC the capacitors are open, so the output equals the input at every point of the sweep". Run on the
    // three sections written out, since the generic ladder is refused (modules#1)
    let model = Model::written("ladder.msl", &Parts::three_sections());
    let point = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    let names: Vec<String> = point.table.rows.iter().map(|row| format!("{:?}", row[0])).collect();
    assert_eq!(names.len(), 4, "the top's own signals, vin, t1, t2 and vout: {names:?}");
    assert!((point.signal("LadderTop.vout") - 1.0).abs() < 1e-9, "the output equals the 1 V input at DC");
    let sweep = model.run(Commands::sweep_of_the_source()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(sweep.table.rows.len(), 3, "one row per point: 0, 0.5 and 1 V");
    assert_eq!(sweep.table.columns[0], "LadderTop.src.dc", "the param is named by its path below the top");
    for (row, level) in [0.0, 0.5, 1.0].into_iter().enumerate() {
        assert_eq!(sweep.real(row, "LadderTop.src.dc"), level);
        assert!((sweep.real(row, "LadderTop.vout") - level).abs() < 1e-9, "the output equals the input at every point");
    }
}

#[test]
fn a_module_body_takes_a_param_with_a_where_and_an_instance_with_a_param_block() {
    // modules~9df7d4b2: "c: f64 = 1p where c > 0.0;" (a param the placer binds); modules~fbe506f7:
    // "let r1 = Resistor(a, b) { r: 1k };" (a placed child module), with a named expression and a signal,
    // the other items of the table
    let source = "module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\nmodule Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\nmodule Cell(a: Electrical, b: Electrical) {\n    c: f64 = 1p where c > 0.0;\n    r: f64 = 2k;\n    signal mid: Electrical;\n    let half = r / 2.0;\n    let r1 = Resistor(a, mid) { r: 1k };\n    let r2 = Resistor(mid, b) { r: 1k };\n}\nmodule Top() { signal a: Electrical; signal b: Electrical; let s = Vdc(a); let k = Cell(a, b) { c: 2p }; let load = Resistor(b, gnd) { r: 1k }; }\n";
    let ran = Model::written("body.msl", source).run(Commands::of_top()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.signal("Top.a"), 1.0);
    assert!((ran.signal("Top.b") - 1.0 / 3.0).abs() < 1e-9, "the cell's two 1k in series against the 1k load: a third of 1 V");
    let faulty = Faulty::elaborating("modules~9df7d4b2", "module Cell(a: Electrical) { c: f64 = 1p where c > 0.0; }\nmodule Top() { signal a: Electrical; let k = Cell(a) { c: -1.0 }; }\n", "Top");
    assert!(faulty.given.iter().any(|reported| reported.message.contains('c')), "a `where` that is false is refused: {:?}", faulty.given.iter().map(|reported| (&reported.code, &reported.message)).collect::<Vec<_>>());
}

#[test]
fn the_lowpass_module_runs_over_the_generic_integrator() {
    // modules#6 (run): `lowpass` and `SlowBuffer`; modules~8b927a42 "At each instance they stand for the expressions the
    // caller wrote, here v(a) and 1u". The `idt` inside `lowpass` is the standard generic module at its default nature
    // `State`, and `y` reads itself through it: `dy/dt = (x - y) / tau`. A 1 V step at the start (`a` is 0 at the operating
    // point) gives `y(t) = 1 - e^{-t/tau}`, which Rust computes at each time of the table
    let chapter = Chapter::read();
    let after = "module Top() { signal a: Electrical; signal b: Electrical; let s = SlowBuffer(a, b); }\nanalog for Top { v(a) +> if now() > 0.0 { 1.0 } else { 0.0 }; }\n";
    let model = chapter.wrapped("lowpass.msl", "", "modules#6", after);
    let overrides = Overrides { print: vec!["b".to_string()], ..Faulty::transient_of_top(5e-6, Some(1e-8)) };
    let ran = model.run(overrides).unwrap_or_else(|failure| panic!("{failure:#}"));
    let tau = 1e-6_f64;
    let mut checked = 0;
    for row in 0..ran.table.rows.len() {
        let time = ran.real(row, "time");
        if time < 5e-8 {
            continue;
        }
        let wanted = 1.0 - (-time / tau).exp();
        assert!((ran.real(row, "Top.b") - wanted).abs() < 2e-2, "t = {time:e}: {} against {wanted}", ran.real(row, "Top.b"));
        checked += 1;
    }
    assert!(checked > 100, "the step response is read at {checked} times");
}

#[test]
fn the_book_shows_the_integrator_the_standard_library_gives() {
    // modules#7 (run: the block defines `idt` next to the prelude's and is accepted); modules~ca384a57
    // "idt(e) is the time integral of e, itself a library module that returns a value": the block is the
    // declaration of `std/analog.msl`, comments and `pub` aside
    let chapter = Chapter::read();
    let model = chapter.wrapped("idt.msl", "", "modules#7", "module Top() {}\n");
    model.run(Commands::of_top()).unwrap_or_else(|failure| panic!("the block is refused: {failure}"));
    let library = StandardLibrary::normalized(&["analog.msl"]);
    let block = StandardLibrary::normalize(&chapter.text("modules#7"));
    assert!(library.contains(&block), "the standard library declares `{block}`");
}

#[test]
fn the_pi_controller_runs_with_its_generic_idt() {
    // modules#8 (run): `windup` and `PiAw`: the ports `e: VSignal` and `y: VSignal` are signal-flow nodes (spec §6 rule 2)
    // and the integral's `idt` is the generic module at its default nature. At DC the integral settles where
    // `ki e + kt windup(u) = 0` with `u = kp e + xi`: a large error saturates the output at `hi`, a negative one at `lo`
    let chapter = Chapter::read();
    let (kp, ki, kt, lo, hi) = (0.5_f64, 2e3_f64, 10e3_f64, 0.0_f64, 1.0_f64);
    for error in [3.0_f64, -1.0] {
        let after = format!("module Top() {{ signal a: VSignal; signal b: VSignal; let c = PiAw(a, b); }}\nanalog for Top {{ v(a) +> {error:?}; }}\n");
        let ran = chapter.wrapped("piaw.msl", "", "modules#8", &after).run(Commands::of_top()).unwrap_or_else(|failure| panic!("{failure:#}"));
        let bound = if error > 0.0 { hi } else { lo };
        // the settled `u = kp e + xi` lies beyond the limit, where `windup` is `bound - u`: `ki e + kt (bound - u) = 0`
        let settled = bound + ki * error / kt;
        assert!(if error > 0.0 { settled > hi } else { settled < lo }, "the controller saturates: u settles at {settled} (kp e = {})", kp * error);
        assert!((ran.signal("Top.b") - bound).abs() < 1e-6, "e = {error}: the output sits at its limit {bound}, found {}", ran.signal("Top.b"));
    }
}

#[test]
fn the_switched_capacitor_integrator_is_a_netlist_that_runs() {
    // modules#9 (run): the RSwitch, the ideal op-amp, the two-phase clock and the integrator that is only
    // structure, placed in a top with the clock as an `in` port. `Capacitor` is chapter 1's, written first
    let chapter = Chapter::read();
    let capacitor = "module Capacitor(p: Electrical, n: Electrical) { c: f64 where c > 0.0; }\nanalog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }\n";
    let after = "module Top(in clk: Bit = false) { signal a: Electrical; signal b: Electrical; let s = ScIntegrator(a, b, clk); }\n";
    let ran = chapter.wrapped("sc.msl", capacitor, "modules#9", after).run(Commands::of_top()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.table.rows.len(), 3, "the clock and the two analog nodes");
    assert!(ran.signal("Top.b").abs() < 1e-12, "the integrator's output with no input is at rest");
}

#[test]
fn the_memory_bank_of_bundle_cells_runs() {
    // modules#10 (run): the `Cell` bundle, `Dram1T1C` and the generic `MemoryBank`; modules#11 (fragment): the placement
    // `MemoryBank::<_, 64, 32>(wl, bl) { cell: Dram1T1C { cs: 30f } }` in a parent's body. The listing declares the cells' ports
    // and no behavior, so the harness adds the analog behavior of a cell (a conduction through `ron` from the bit line to the
    // storage node, a leak and the capacitor) and a driver of each bit line (1 V behind 1k): 64 x 32 cells, each a bundle `Cell`
    // of the word line `wl[r]` and the bit line `bl[c]`. A cell's storage node follows its bit line
    let chapter = Chapter::read();
    let top = format!("module Top() {{\n    signal wl: [Bit; 64] = [false; 64];\n    signal bl: [Electrical; 32];\n    {}\n}}\nanalog for Top {{ for k in 0..32 {{ i(bl[k], gnd) +> (v(bl[k]) - 1.0) / 1k; }} }}\n", chapter.text("modules#11"));
    let cell = "analog for Dram1T1C {\n    i(c.bl, sn) +> v(c.bl, sn) / ron;\n    i(sn, gnd) +> v(sn) / rleak + ddt(cs * v(sn));\n}\n";
    let overrides = Overrides { print: ["Top.bl[0]", "Top.bl[31]", "Top.bank.cells[0][0].sn", "Top.bank.cells[63][31].sn"].map(String::from).to_vec(), ..Commands::of_top() };
    let ran = chapter.wrapped("bank.msl", "", "modules#10", &format!("{cell}{top}")).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    for line in ["Top.bl[0]", "Top.bl[31]"] {
        // the 64 cells on a line leave it at 1 V less 64 x 1k / (5k + 1T)
        assert!((ran.signal(line) - 1.0).abs() < 1e-6, "{line}: {}", ran.signal(line));
    }
    for node in ["Top.bank.cells[0][0].sn", "Top.bank.cells[63][31].sn"] {
        assert!((ran.signal(node) - 1.0).abs() < 1e-6, "{node} follows its bit line: {}", ran.signal(node));
    }
}

#[test]
fn a_function_that_returns_a_template_places_its_result() {
    // modules#12 (run): `nch`, the binning function; modules#13 (fragment): `nch(0.13u, 2u)(d, g, s, b)` places its result,
    // a `Bsim4` with the `vth0` the length bins to (0.42 under 0.18u, §8.4.7). The `Bsim4` of the test pulls its terminal `d` to
    // `vth0` through a unit conductance, so the operating point reads the binned value
    let chapter = Chapter::read();
    let bsim4 = "module Bsim4(d: Electrical, g: Electrical, s: Electrical, b: Electrical) { l: f64 = 1u; w: f64 = 1u; vth0: f64 = 0.4; }\nanalog for Bsim4 { i(d, gnd) +> v(d, gnd) - vth0; i(g, gnd) +> v(g, gnd); i(s, gnd) +> v(s, gnd); i(b, gnd) +> v(b, gnd); }\n";
    let top = format!("module Top() {{\n    signal d: Electrical;\n    signal g: Electrical;\n    signal s: Electrical;\n    signal b: Electrical;\n    {}\n}}\n", chapter.text("modules#13"));
    let ran = chapter.wrapped("nch.msl", bsim4, "modules#12", &top).run(Commands::of_top()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.d") - 0.42).abs() < 1e-9, "the binned vth0 of a 0.13u length: {}", ran.signal("Top.d"));
}

#[test]
fn the_swappable_mosfet_listing_runs() {
    // modules#14 (run): `trait Mosfet`, the stand-in models `Bsim3` (`k g^2`) and `Mos1` (`k g`) and `DiffPair<N: Mosfet + Fn(..)>`;
    // modules#15 (fragment): the pair placed over `Mos1 { k: 3.0 }`, and over `Bsim3 { k: 3.0 }` once the model is swapped (§8.4). Each model
    // gives its own operating point, computed here from its formula at a gate held at 0.5 V
    let chapter = Chapter::read();
    let fragment = chapter.text("modules#15");
    let gate = 0.5;
    let drain = |model: &str| -> f64 {
        let placed = fragment.replace("Mos1", model);
        let top = format!("module Top() {{\n    signal g: Electrical;\n    signal d1: Electrical;\n    signal d2: Electrical;\n    {placed}\n}}\nanalog for Top {{ potential(g) +> {gate}; }}\n");
        let ran = chapter.wrapped("pair.msl", "", "modules#14", &top).run(Commands::of_top()).unwrap_or_else(|failure| panic!("{failure}"));
        assert!((ran.signal("Top.d1") - ran.signal("Top.d2")).abs() < 1e-9, "both legs of the pair hold the model's response");
        ran.signal("Top.d1")
    };
    let mos1 = 3.0 * gate;
    let bsim3 = 3.0 * gate * gate;
    assert!((drain("Mos1") - mos1).abs() < 1e-9, "Mos1 gives k g = {mos1}");
    assert!((drain("Bsim3") - bsim3).abs() < 1e-9, "Bsim3 gives k g^2 = {bsim3}");
}

#[test]
fn the_multiplicity_listing_runs_as_four_in_parallel() {
    // modules~93efd115 (generics phase D, GN-26 AC 3; replaces the claim modules~1faf8c12 and its test
    // `binding_m_and_dtemp_is_refused_as_an_unknown_param`): "Placed as Resistor(a, b) { r: 1k, m: Some(4.0) } this
    // resistor behaves as four in parallel; placed with no m it follows the run, so --simparam multiplicity=3 reaches
    // every instance that leaves m unset" (modules#16 is the listing), and modules~007f1097: "Read in a param default,
    // multiplicity() is E0321". The
    // resistor sits over a fixed kilo-ohm one under one volt: the node reads the divider of the effective multiplicity
    let chapter = Chapter::read();
    let after = |block: &str| format!("module Fixed(p: Electrical, n: Electrical) {{ r: f64 = 1k; }}\nanalog for Fixed {{ i(p, n) +> v(p, n) / r; }}\nmodule Source(p: Electrical) {{}}\nanalog for Source {{ v(p, gnd) +> 1.0; }}\nmodule Top() {{\n    signal vin: Electrical;\n    signal o: Electrical;\n    let s = Source(vin);\n    let top = {block}(vin, o);\n    let bottom = Fixed(o, gnd);\n}}\n");
    let divider = |count: f64| 1000.0 / (1000.0 / count + 1000.0);
    let run = |block: &str, overrides: Overrides| chapter.wrapped("multiplicity.msl", "", "modules#16", &after(block)).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let four = run("Resistor { m: Some(4.0) }", Commands::of_top());
    assert!((four.signal("Top.o") - divider(4.0)).abs() < 2e-3, "four in parallel: {}", four.signal("Top.o"));
    let followed = run("Resistor", Overrides { simparam: vec![("multiplicity".to_string(), 3.0)], ..Commands::of_top() });
    assert!((followed.signal("Top.o") - divider(3.0)).abs() < 2e-3, "the run's multiplicity 3 reaches an instance with no `m`: {}", followed.signal("Top.o"));
    let alone = run("Resistor", Commands::of_top());
    assert!((alone.signal("Top.o") - divider(1.0)).abs() < 2e-3, "nothing set: one resistor: {}", alone.signal("Top.o"));
    // read in a param default, `multiplicity()` is `E0321`
    let default = Model::written("default.msl", "module Top() { k: f64 = multiplicity().unwrap_or(1.0); signal o: Electrical; }\n").run(Commands::of_top());
    assert!(Refusal::of(default, "modules~007f1097").iter().any(|(code, _)| code == "E0321"), "a param default that reads the run");
    // there is no implicit param: binding `m` on a module that declares none is `E0320` like any unknown name
    let faulty = Faulty::elaborating("modules~93efd115", &Parts::with_top("    let r1 = Resistor(a, gnd) { r: 1k, m: 4.0 };\n"), "Top");
    assert_eq!(faulty.first("E0320").message, "no param `m` in `Resistor`");
}

#[test]
fn e0320_a_block_names_a_param_the_module_does_not_have() {
    // modules~4bd2871d and modules~0d2be82e: "E0320 RcSection(a, b) { res: 1k }": the param is called `r`
    let faulty = Faulty::elaborating("modules~4bd2871d", &Parts::with_top("    let s = RcSection(a, b) { res: 1k };\n"), "Top");
    assert_eq!(faulty.first("E0320").message, "no param `res` in `RcSection`");
    assert_eq!(faulty.pointed_at("E0320"), "res");
    let ran = Model::written("fixed.msl", &Parts::with_top("    let s = Vdc(a);\n    let t = RcSection(a, b) { r: 1k };\n")).run(Commands::of_top());
    assert!(ran.is_ok(), "the param `r` is accepted");
}

#[test]
fn e0312_a_port_is_left_unconnected() {
    // modules~4bc8171d and modules~0d5cece8: "E0312 RcSection(a) { r: 1k }": connections are positional,
    // give both
    let faulty = Faulty::elaborating("modules~4bc8171d", &Parts::with_top("    let s = RcSection(a) { r: 1k };\n"), "Top");
    assert_eq!(faulty.first("E0312").message, "`RcSection` has 2 port(s), found 1 connection(s)");
}

#[test]
fn e0343_a_parent_sees_only_what_the_child_publishes() {
    // modules~4bd9101d: "E0343 v(lad.head.a), or a tap that is not pub": a child instance `s` that is not
    // `pub` is not reachable from the parent through `o.s`
    let outer = "module Outer(a: Electrical, b: Electrical) { let s = RcSection(a, b); }\nmodule Top() { signal a: Electrical; signal b: Electrical; let o = Outer(a, b); }\nanalog for Top { v(a) +> v(o.s.a); }\n";
    let faulty = Faulty::elaborating("modules~4bd9101d", &format!("{}{outer}", Parts::TEXT), "Top");
    assert_eq!(faulty.first("E0343").message, "`s` is not `pub` in module `Outer`");
    assert_eq!(faulty.pointed_at("E0343"), "s");
}

#[test]
fn e0333_an_instance_used_as_a_value_names_the_instance() {
    // modules~4bcf201d: "E0333 let x = r1 * 2.0; where r1 is a resistor instance". `E0333` (section 8.2.14,
    // PI-12 AC 7) names the instance of a module without a result read as a value
    let faulty = Faulty::elaborating("modules~4bcf201d", &Parts::with_top("    let r1 = Resistor(a, gnd) { r: 1k };\n    let x = r1 * 2.0;\n"), "Top");
    assert_eq!(faulty.first("E0333").message, "the instance `r1` has no result: a module without a result is not a value (§8.2.14)");
    assert_eq!(faulty.pointed_at("E0333"), "r1");
}

#[test]
fn e0903_a_body_holds_no_variables() {
    // modules~80317e1d "E0903 let mut acc = 0.0; in a module body": `mut` is a reserved word; a state is a signal
    let faulty = Faulty::elaborating("modules~80317e1d", &Parts::with_top("    let mut acc = 0.0;\n"), "Top");
    assert_eq!(faulty.first("E0903").message, "`mut` is a reserved keyword");
    assert_eq!(faulty.first("E0903").help.as_deref(), Some("write the plain form `let x`: every local may be assigned"));
}

#[test]
fn e0340_a_generation_range_read_from_a_signal() {
    // modules~4bd9131d: "E0340 for k in 0..count { ... } with count a signal". The range of a generation is a const expression
    // over params, constants and const generics (section 8.2.7): a signal is read over time
    let faulty = Faulty::elaborating("modules~4bd9131d", &Parts::with_top("    signal count: Bit<u8> = 0;\n    for k in 0..count { }\n"), "Top");
    assert_eq!(faulty.first("E0340").message, "a generation range reads `count`: it is a const expression over params, constants and const generics (§8.2.7)");
    assert_eq!(faulty.pointed_at("E0340"), "0..count");
    let fixed = Model::written("range.msl", &format!("{}module Top() {{\n    count: usize = 2;\n    signal a: Electrical;\n    let s = Vdc(a);\n    for k in 0..count {{ let r = Resistor(a, gnd) {{ r: 1k }}; }}\n}}\n", Parts::TEXT)).run(Commands::of_top());
    assert!(fixed.is_ok(), "a param is a const expression: {:?}", fixed.err().map(|failure| failure.to_string()));
}

#[test]
fn e0330_one_name_is_declared_once_in_a_module() {
    // modules~4bcf211d: "E0330 a param c and let c = Capacitor(...)": rename the instance
    let source = format!("{}module Top(c: f64 = 1.0) {{ signal a: Electrical; let c = Capacitor(a, gnd) {{ c: 1p }}; }}\n", Parts::TEXT);
    let faulty = Faulty::elaborating("modules~4bcf211d", &source, "Top");
    assert_eq!(faulty.first("E0330").message, "`c` is defined more than once in module `Top`");
}

#[test]
fn e0401_a_module_and_its_behavior_live_in_one_file() {
    // modules~54e1b71d: "E0401 analog for RcSection { ... } in another file": the behavior block of a
    // module declared in `lib.msl`, written in `other.msl`
    let manifest = "[package]\nname = \"pk\"\nversion = \"0.1.0\"\n\n[run.op]\nbench = \"src/lib.msl\"\nanalysis = \"op\"\n";
    let model = Model::package(manifest)
        .with_source("src/lib.msl", "module RcSection(a: Electrical, b: Electrical) { r: f64 = 1k; }\nmodule Top() { signal a: Electrical; let s = RcSection(a, gnd); }\nmod other;\n")
        .with_source("src/other.msl", "analog for RcSection { i(a, b) +> v(a, b) / r; }\n");
    Refusal::expect(model.of_run("op").run(Overrides::default()), "modules~54e1b71d", "E0401", "cannot find a module `RcSection` for this behavior block");
}

#[test]
fn e0304_a_generic_ladder_of_zero_sections() {
    // modules~4bcb9f1d: "E0304 Ladder::<0>(vin, vout): the module's where N >= 1 is false". The instance is refused before its
    // body is lowered, naming it
    let chapter = Chapter::read();
    let zero = "module Empty() { signal vin: Electrical; pub signal vout: Electrical; pub let lad = Ladder::<0>(vin, vout) { r: 1k, c: 1n }; }\n";
    let block = chapter.text("modules#1");
    let (listing, _) = block.split_once("module LadderTop()").expect("modules#1 ends with LadderTop");
    let model = Model::written("empty.msl", &format!("{listing}\n{zero}"));
    let given = Refusal::of(model.run(Faulty::operating_point_of("Empty")), "modules~4bcb9f1d");
    assert_eq!(given.len(), 1, "{given:?}");
    assert_eq!(given[0].0, "E0304");
    assert!(given[0].1.contains("Ladder::<0>"), "the message names the instance: {given:?}");
}

#[test]
fn the_rule_of_thumb_for_the_ladder_gives_the_three_times_the_book_lists() {
    // modules~1a89f2ad: "A rough rule for an RC ladder is 0.69 times the Elmore delay, which here is 3, 5
    // and 6 times r * c for the three taps: about 2.1, 3.5 and 4.1 us after the step". The Elmore delay of
    // tap k is the sum over the sections of r times the capacitance behind it: 3, 5 and 6 times r*c
    let (r, c) = (1e3, 1e-9);
    let elmore: [f64; 3] = [3.0, 2.0 * 1.0 + 3.0, 2.0 * 1.0 + 3.0 + 1.0].map(|count| count * r * c);
    for (delay, seconds) in elmore.iter().zip([3e-6, 5e-6, 6e-6]) {
        assert!((delay - seconds).abs() < 1e-15, "3, 5 and 6 times r*c: {delay:e} against {seconds:e}");
    }
    for (delay, printed) in elmore.iter().zip(["2.1e-6", "3.5e-6", "4.1e-6"]) {
        Printed::holds(printed, 0.69 * delay, "the rule's estimate");
    }
}
