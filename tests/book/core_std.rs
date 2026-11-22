//! Scope: chapter 13 of the Book, "Core and standard library" (feature `root-e2e`, RE-02 and RE-03): the
//! tour modules that run (`Tour`, `VT_300K`, `Rc`, `Latch`, `DacRamp`, `Pad`, `Probe`, a crate's own
//! `vt`), the ones refused today (the resolved line, `Reg` with its `Setup`, the watchdog's integer
//! param, asserted refused with their diagnostics), the statements the chapter makes about the core, the
//! monitors and the intrinsic items, and the six mistakes of its table, run through `piperine::`. Each
//! test opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

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

/// Parts the small designs of the chapter hang their module on.
struct Bench;

impl Bench {
    /// A resistor and a DC source, each with its behavior.
    const PARTS: &'static str = "module R(p: Electrical, n: Electrical) { r: f64 = 1k; }\nanalog for R { i(p, n) +> v(p, n) / r; }\nmodule Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\n";
}

/// The runs the chapter's models need.
struct Commands;

impl Commands {
    /// An operating point of the top `Top`.
    fn operating_point() -> Overrides {
        Faulty::operating_point_of("Top")
    }

    /// A transient of the top `Top` to `stop`, printing every `print_step` seconds when given.
    fn transient(stop: f64, print_step: Option<f64>) -> Overrides {
        Overrides { print_step, ..Faulty::transient_of_top(stop, None) }
    }
}

#[test]
fn the_tour_module_runs_its_analog_and_digital_blocks() {
    // core-std#1 (run): `Tour`, one module with an `analog` block that contributes zero current and a
    // reaction on `clk.rise()` that emits `seen`; the names it uses (Electrical, Bit, now, emit!) need no
    // import: the prelude
    let chapter = Chapter::read();
    let after = format!("{}module Top() {{ signal a: Electrical; signal clk: Bit = false; let t = Tour(a, clk); let r = R(a, gnd); }}\ndigital for Top {{ on at(1u) {{ clk <- true; }} }}\n", Bench::PARTS);
    let ran = chapter.wrapped("tour.msl", "", "core-std#1", &after).run(Commands::transient(3e-6, None)).unwrap_or_else(|failure| panic!("{failure}"));
    let seen = ran.stream("seen");
    assert_eq!(seen.len(), 1, "one rising edge");
    Printed::holds("1.000000e-6", Fields::real(seen[0], "t"), "the edge");
}

#[test]
fn a_constant_of_std_constants_is_the_thermal_voltage_at_300_kelvin() {
    // core-std#2 (run): `const VT_300K: f64 = K_B * 300.0 / Q;` from `std::constants`, read in `analog`
    let chapter = Chapter::read();
    let after = "module Top() { signal a: Electrical; }\nanalog for Top { v(a) +> VT_300K; }\n";
    let ran = chapter.wrapped("constant.msl", "", "core-std#2", after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("2.585200e-2", ran.signal("Top.a"), "kT/q at 300 K");
}

#[test]
fn the_rc_module_of_the_carriers_part_runs() {
    // core-std#3 (run): `Rc`, a resistor into a capacitor through its own `mid` node: at DC the capacitor
    // is open and the source's 1 V is across it
    let chapter = Chapter::read();
    let after = format!("{}module Top() {{ signal a: Electrical; let s = Vdc(a); let x = Rc(a, gnd); }}\n", Bench::PARTS);
    let ran = chapter.wrapped("rc.msl", "", "core-std#3", &after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.signal("Top.a"), 1.0);
}

#[test]
fn the_latch_takes_its_input_at_the_rising_edge() {
    // core-std#4 (run): `Latch`, `q <- d` on `clk.rise()` (replaces
    // `the_latch_takes_its_input_at_the_rising_edge_or_zero_when_unknown`, whose `known_or(0)` and unknown
    // input are gone): the input is latched at the edge, and before it `q` keeps its initial value
    let chapter = Chapter::read();
    let after = |data: &str| format!("module Top() {{ signal clk: Bit = false; signal d: Bit<u8>{data}; signal q: Bit<u8>; let l = Latch(clk, d, q); }}\ndigital for Top {{ on at(1u) {{ clk <- true; }} }}\n");
    let ran = chapter.wrapped("latch.msl", "", "core-std#4", &after(" = 5")).run(Commands::transient(3e-6, Some(1e-6))).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.table.rows[0][3], piperine::Cell::Int(0), "before the edge, the initial value");
    assert_eq!(ran.table.rows.last().expect("a last row")[3], piperine::Cell::Int(5), "after it, the input");
}

#[test]
fn the_dac_ramp_scales_its_code_by_the_reference() {
    // core-std#5 (run): `DacRamp`: `transition(vref * (code as f64) / 256.0, ..)`: code 64 and a 1 V reference
    // give 0.25 V
    let chapter = Chapter::read();
    let after = format!("module Top() {{ signal c: Bit<u8> = 64; signal v: Electrical; let d = DacRamp(c, v); let r = R(v, gnd); }}\n{}", Bench::PARTS);
    let ran = chapter.wrapped("dac.msl", "", "core-std#5", &after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.v") - 0.25).abs() < 1e-9, "64 / 256 of 1 V: {}", ran.signal("Top.v"));
}

#[test]
fn the_resolved_line_is_refused_until_a_nature_can_implement_resolve() {
    // core-std#6 (refused, S8): `IrqLine`, a nature the model declares with `impl Resolve<Driven>`, and a `Board`
    // with a `Pullup`: `Driven` and `Resolve` are not in the library yet (E0161)
    let chapter = Chapter::read();
    let model = chapter.lone("board.msl", &["core-std#6"]);
    let given = Refusal::of(model.run(Faulty::operating_point_of("Board")), "core-std#6");
    assert!(given.iter().all(|(code, _)| code == "E0161"), "names the library does not hold yet: {given:?}");
    Refusal::expect(model.run(Faulty::operating_point_of("Board")), "core-std#6", "E0161", "cannot find `Driven` in this scope");
}

#[test]
fn the_pad_drives_its_node_and_reads_it_back() {
    // core-std#7 (run): `Pad`: a `D2A` and an `A2D` on one node: a high `d` puts the supply on the node and
    // `q` reads high; a low `d` at 1 us takes both down
    let chapter = Chapter::read();
    let after = format!("{}module Top() {{ signal d: Bit = true; signal q: Bit; signal n: Electrical; signal vdd: Electrical; let s = Vdc(vdd) {{ dc: 2.0 }}; let p = Pad(d, q, n, vdd); }}\ndigital for Top {{ on at(1u) {{ d <- false; }} }}\n", Bench::PARTS);
    let ran = chapter.wrapped("pad.msl", "", "core-std#7", &after).run(Commands::transient(4e-6, Some(1e-6))).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.real(0, "Top.n"), 2.0, "high drives the supply");
    assert_eq!(ran.table.rows[0][2], piperine::Cell::Bool(true), "and `q` reads it high");
    let last = ran.table.rows.len() - 1;
    assert!(ran.real(last, "Top.n").abs() < 1e-6, "low drives zero");
    assert_eq!(ran.table.rows[last][2], piperine::Cell::Bool(false), "and `q` follows it");
}

#[test]
fn the_register_with_a_setup_check_is_refused_for_the_generic_monitor() {
    // core-std#8 (refused): `Reg` places `Setup(clk, d) { ts: 100p }`, and `Setup<T: Repr>` is a generic
    // module (L30)
    let chapter = Chapter::read();
    let after = "module Top(in clk: Bit) { signal d: Bit; signal q: Bit; let r = Reg(clk, d, q); }\n";
    Refusal::expect(chapter.wrapped("reg.msl", "", "core-std#8", after).run(Commands::operating_point()), "core-std#8", "P0001", "generic module is not supported yet");
}

#[test]
fn the_probe_writes_a_check_and_a_log_record() {
    // core-std#9 (run): `Probe`: `require("ok_at_edge", ok)` and `log(Severity::Info, "edge")` on the rising
    // edge: a `checks` record that passes and a `log` record with severity 0 and the message
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit = false; signal ok: Bit = true; let p = Probe(clk, ok); }\ndigital for Top { on at(1u) { clk <- true; } }\n";
    let ran = chapter.wrapped("probe.msl", "", "core-std#9", after).run(Commands::transient(3e-6, None)).unwrap_or_else(|failure| panic!("{failure}"));
    let checks = ran.stream("checks");
    assert_eq!(checks.len(), 1);
    assert_eq!(Fields::of(checks[0], "name"), &Field::Text("ok_at_edge".to_string()));
    assert_eq!(Fields::of(checks[0], "pass"), &Field::Bool(true));
    let logs = ran.stream("log");
    assert_eq!(logs.len(), 1);
    assert_eq!(Fields::of(logs[0], "msg"), &Field::Text("edge".to_string()));
    assert_eq!(Fields::of(logs[0], "severity"), &Field::Int(0), "Severity::Info");
}

#[test]
fn the_watchdog_is_refused_for_its_integer_param() {
    // core-std#10 (refused): `Watchdog`, with `limit: u16 = 1000`: a param of an integer type is `P0001`
    // (L28)
    let chapter = Chapter::read();
    let after = "module Top() { signal clk: Bit; signal busy: Bit = true; let w = Watchdog(clk, busy); }\n";
    Refusal::expect(chapter.wrapped("watchdog.msl", "", "core-std#10", after).run(Commands::transient(1e-6, None)), "core-std#10", "P0001", "param of type `u16` is not supported yet");
}

#[test]
fn the_watchdog_ends_the_run_at_the_fourth_edge_with_the_button_held() {
    // The watchdog of core-std#10 with `limit` written as the literal 3 (the Book's text is refused): the
    // exercise of the chapter, "the fourth edge with the button down: the first three count, the fourth
    // finds run at 3". The `stuck` record carries the time and the count, `fatal` ends the run at that
    // instant and marks it failed
    let chapter = Chapter::read();
    let watchdog = chapter.text("core-std#10");
    assert!(watchdog.contains("    limit: u16 = 1000;\n") && watchdog.contains("run >= limit"), "the Book's text");
    let watchdog = watchdog.replace("    limit: u16 = 1000;\n", "").replace("run >= limit", "run >= 3");
    let clock = chapter.text("values#1");
    let clock = clock.split_once("/// Counts rising edges").expect("the clock comes first").0;
    let source = format!("{watchdog}\n{clock}\nmodule Top() {{ signal clk: Bit; signal busy: Bit = true; let o = Osc(clk) {{ f: 1M }}; let w = Watchdog(clk, busy); }}\n");
    let ran = Model::written("watchdog.msl", &source).run(Commands::transient(10e-6, None)).unwrap_or_else(|failure| panic!("{failure}"));
    let stuck = ran.stream("stuck");
    assert_eq!(stuck.len(), 1);
    Printed::holds("3.500000e-6", stuck[0].time, "the fourth rising edge of a 1 MHz clock");
    assert_eq!(Fields::of(stuck[0], "edges"), &Field::Int(3), "the count");
    let fatal = ran.stream("sim");
    assert_eq!(fatal.len(), 1, "one `sim` record");
    assert_eq!(Fields::of(fatal[0], "failed"), &Field::Bool(true), "fatal marks the end as failed");
    assert_eq!(Fields::of(fatal[0], "msg"), &Field::Text("busy was never released".to_string()));
    Printed::holds("3.500000e-6", ran.real(ran.table.rows.len() - 1, "time"), "the run ends at that instant");
}

#[test]
fn a_crates_own_vt_replaces_the_library_one() {
    // core-std#11 (run): `pub fn vt() -> f64 { 0.0259 }`: "Declare an item with the same name in your own
    // namespace and yours is the one the names in that scope find; the library's stays untouched": a node
    // driven to `vt()` reads 25.9 mV where the library's reads 25.86 mV at the default temperature
    let chapter = Chapter::read();
    let after = "module Top() { signal a: Electrical; }\nanalog for Top { v(a) +> vt(); }\n";
    let own = chapter.wrapped("vt.msl", "", "core-std#11", after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(own.signal("Top.a"), 0.0259, "this crate's vt");
    let library = Model::written("vt.msl", after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    Printed::holds("2.586493e-2", library.signal("Top.a"), "the library's vt, kT/q at 300.15 K");
}

#[test]
fn the_probes_have_the_short_names_the_nature_gives() {
    // core-std~c59252de: "Probes: potential, flow, inflow ... Your nature gives them their short names, v and
    // i": on `Electrical`, `v(a)` is `potential(a)`, so a source written either way gives the same node
    let write = |probe: &str| format!("module Top() {{ signal a: Electrical; }}\nanalog for Top {{ {probe}(a) +> 1.5; }}\n");
    let short = Model::written("short.msl", &write("v")).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    let long = Model::written("long.msl", &write("potential")).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!((short.signal("Top.a"), long.signal("Top.a")), (1.5, 1.5), "`v` and `potential` write the same branch");
}

#[test]
fn the_library_declares_the_noise_sources_and_the_monitors_only_report() {
    // core-std~e4393c2d: "white_noise, flicker_noise, noise_table: noise sources with a flat, a 1/f and a
    // tabulated spectrum": `std/analog.msl` declares the three (each is refused when placed: `noise` is
    // L45); core-std~96074bac "Monitors only report: each is a module that calls require when the property it
    // watches fails": a `Never` records a failed check when `bad` is high at an edge, and no monitor of
    // `std/monitor.msl` has an `out` port but `Setup`'s own `viol`
    let analog = StandardLibrary::normalized(&["analog.msl"]);
    for declared in ["module white_noise(psd: f64", "module flicker_noise(k: f64, exp: f64", "module noise_table<const N: usize>"] {
        assert!(analog.contains(declared), "std::analog declares `{declared}`");
    }
    let monitors = StandardLibrary::normalized(&["monitor.msl"]);
    let outputs: Vec<&str> = monitors.match_indices(" out ").map(|(at, _)| monitors[at..].split(|character| character == ',' || character == ')').next().unwrap_or("")).collect();
    assert_eq!(outputs, [" out viol: Bit = false"], "`Setup` drives its own `viol`, which it does not watch");
    let source = "module Top() { signal clk: Bit = false; signal bad: Bit = true; let m = Never(clk, bad); }\ndigital for Top { on at(1u) { clk <- true; } }\n";
    let ran = Model::written("never.msl", source).run(Commands::transient(3e-6, None)).unwrap_or_else(|failure| panic!("{failure}"));
    let checks = ran.stream("checks");
    assert_eq!(checks.len(), 1);
    assert_eq!(Fields::of(checks[0], "pass"), &Field::Bool(false), "the property failed and the monitor reports it");
}

#[test]
fn e0457_a_core_name_cannot_be_redefined() {
    // core-std~ece4c57c and core-std~54d7c51d: "A core name needs no import and cannot be redefined: a
    // namespace item with the same name is error E0457"; "E0457 fn ddt(x: f64) -> f64 { ... }"
    let faulty = Faulty::elaborating("core-std~54d7c51d", "fn ddt(x: f64) -> f64 { x }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0457").message, "`ddt` is a core primitive and cannot be redefined (§14 rule 1)");
}

#[test]
fn e0125_only_std_declares_an_engine_item() {
    // core-std~2d426657 and core-std~39983c1d: "Only std may declare one; the same line in your crate is error
    // E0125"; "E0125 extern \"intrinsic\" fn my_parse(text: &str) -> f64;"
    let faulty = Faulty::elaborating("core-std~39983c1d", "extern \"intrinsic\" fn my_parse(text: &str) -> f64;\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0125").message, "`extern \"intrinsic\"` is accepted only in the standard library");
    assert_eq!(faulty.first("E0125").help.as_deref(), Some("only the standard library declares intrinsic functions; write a body or use `extern \"C\"`"));
}

#[test]
fn e0603_the_reserved_streams_take_records_from_reactions_only() {
    // core-std~64ee8b1d: "E0603 require(...), finish() or fatal(...) in analog": here `finish()`
    let faulty = Faulty::elaborating("core-std~64ee8b1d", "module Top() { signal a: Electrical; }\nanalog for Top { finish(); }\n", "Top");
    assert_eq!(faulty.first("E0603").message, "a site reached from `analog` writes the stream `sim`: only a reaction decides what the host reads there (§19 rules 4 and 14)");
}

#[test]
fn e0605_a_reserved_stream_field_has_a_fixed_type() {
    // core-std~64ee891d: "E0605 emit!(\"checks\", name = 3);": `name` is text
    let source = "module Top() { }\ndigital for Top { on at(1u) { emit!(\"checks\", name = 3); } }\n";
    let faulty = Faulty::running("core-std~64ee891d", source, Commands::transient(2e-6, None));
    assert_eq!(faulty.first("E0605").message, "the field `name` of the stream `checks` has type `i32`: the stdlib gives it `&str` (§19 rule 15)");
}

#[test]
fn e0161_the_inherited_library_is_a_closed_list() {
    // core-std~39a5d01d: "E0161 x.cbrt(), or another Rust method the library does not list": write
    // x.powf(1.0 / 3.0), which runs and gives the cube root
    let faulty = Faulty::elaborating("core-std~39a5d01d", "module Top() { signal a: Electrical; }\nanalog for Top { let x = 8.0; v(a) +> x.cbrt(); }\n", "Top");
    assert_eq!(faulty.first("E0161").message, "no method `cbrt` on `f64`");
    let fixed = "module Top() { signal a: Electrical; }\nanalog for Top { let x = 8.0; v(a) +> x.powf(1.0 / 3.0); }\n";
    let ran = Model::written("fixed.msl", fixed).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.a") - 2.0).abs() < 1e-9, "the cube root of 8");
}

#[test]
fn e0702_is_raised_for_a_logic_pattern_with_z() {
    // core-std~6e04c51d: "E0702 logic(\"10z1\")": a pattern holds only 0, 1 and x. `logic` is typed by its
    // contract (L18 closed by `unknown-semantics`), so the checker raises `E0702` at the pattern
    let faulty = Faulty::elaborating("core-std~6e04c51d", "module Top() { signal a: Bit<u4> = logic(\"10z1\"); }\n", "Top");
    faulty.first("E0702");
}
