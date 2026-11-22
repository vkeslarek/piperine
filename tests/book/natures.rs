//! Scope: chapter 6 of the Book, "Natures and carriers" (feature `root-e2e`, RE-02 and RE-03): the
//! natures the standard library gives, the signal declarations, a resistor that heats itself, the
//! open-drain line and the bundle the Book builds (both refused today, asserted refused with their
//! diagnostics), an `out` port that gives the initial value, and the mistakes of the chapter's table,
//! each run through `piperine::`. Each test opens with the inventory ids it holds, from
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
    pub mod standard_library;
}

use piperine::{Field, Overrides};
use piperine_parity::golden::PrintedNumber;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
use support::refusal::Refusal;
use support::standard_library::StandardLibrary;

/// A number as the Book prints it: the printed digits are the resolution of the comparison.
struct Printed;

impl Printed {
    /// Asserts that `x` rounds to the number the Book prints as `printed` (`4.990040e-1`).
    fn holds(printed: &str, x: f64, what: &str) {
        let number = PrintedNumber::parse(printed).unwrap_or_else(|| panic!("`{printed}` is a printed number"));
        assert!(number.rounds_from(x), "{what}: the Book prints {printed}, the run gives {x:e}");
    }
}

/// The operating point of the top `Top`, the run of a file with a top and no analysis named.
struct OperatingPoint;

impl OperatingPoint {
    /// The overrides of `piperine run <file> --top Top --analysis op`.
    fn of_top() -> Overrides {
        Faulty::operating_point_of("Top")
    }
}

#[test]
fn the_book_shows_the_natures_the_standard_library_gives() {
    // natures#1 (run: the declarations, which nothing reaches, are accepted next to the prelude's);
    // natures~66574051: "These are the ones the standard library gives you": each declaration of the
    // block is a declaration of `std/carriers.msl` or `std/nets.msl`, comments and `pub` aside
    let chapter = Chapter::read();
    let model = chapter.wrapped("natures.msl", "", "natures#1", "module Top() {}\n");
    model.run(OperatingPoint::of_top()).unwrap_or_else(|failure| panic!("the block is refused: {failure}"));
    let library = StandardLibrary::normalized(&["carriers.msl", "nets.msl"]);
    let block = StandardLibrary::normalize(&chapter.text("natures#1"));
    let declarations: Vec<&str> = block.split("nature ").skip(1).collect();
    assert_eq!(declarations.len(), 4, "Bit, Electrical, Thermal and VSignal; the library declares no net nature: {block}");
    for declaration in declarations {
        let declaration = format!("nature {}", declaration.trim());
        assert!(library.contains(&declaration), "the standard library declares `{declaration}`");
    }
}

#[test]
fn signals_of_every_family_are_refused_for_the_arrays_the_option_the_logic_and_the_net() {
    // natures#2 (refused, S8): "signal en: Bit = false; ... signal last: Bit<Option<u8>>; ... signal lines:
    // [Bit; 8] = [false; 8]; ... signal bus: Bit<Logic<4>> ...; signal sda: Sda; ...": an array of carriers is
    // `P0001` (L24), an `Option` is not carried yet (E0112), `Logic` is a library struct the front end does
    // not take as a type yet (P0001) and `Sda` is the model's own nature, declared further on in the
    // chapter's example, so here it is unknown (E0161); the discrete, node and initial-value lines are
    // accepted. When the block runs this test fails and the row moves to `run`
    let chapter = Chapter::read();
    let alone = chapter.wrapped("signals.msl", "module Top() {\n", "natures#2", "}\n");
    let unknown = Refusal::of(alone.run(OperatingPoint::of_top()), "natures#2");
    assert!(unknown.contains(&("E0161".to_string(), "cannot find `Sda` in this scope".to_string())), "the nature `Sda` is not declared in the block: {unknown:?}");
    // with a nature of that name in scope the type checker reaches the other lines; the stand-in's value type
    // has a `Default`, so the line `signal sda: Sda;` stays valid once every carrier starts at its default (phase A)
    let model = chapter.wrapped("signals.msl", "nature Sda { value: Option<bool> }\nmodule Top() {\n", "natures#2", "}\n");
    let given = Refusal::of(model.run(OperatingPoint::of_top()), "natures#2");
    for (code, expected) in [("P0001", "array type is not supported yet"), ("E0112", "`Option<u8>` has no `Repr`: a carrier holds only a type that implements it (part I §2)"), ("P0001", "`struct` item used as a type is not supported yet")] {
        assert!(given.contains(&(code.to_string(), expected.to_string())), "{expected}: {given:?}");
    }
    Refusal::expect(chapter.wrapped("signals.msl", "nature Sda { value: Option<bool> }\nmodule Top() {\n", "natures#2", "}\n").run(OperatingPoint::of_top()), "natures#2", "P0001", "array type is not supported yet");
}

#[test]
fn the_open_drain_line_is_refused_until_a_nature_can_implement_resolve() {
    // natures#3 (refused, S8): the alarm.msl of the Book, whose nature `AlarmLine` implements
    // `Resolve<Driven>`; natures~e99c1920 "Today the command refuses the file and runs nothing" (refused);
    // natures~6a449552 "Replace Pullup(alarm_n) with Pulldown(alarm_n) and predict the stream" (refused: it
    // needs the same nature). The names the file uses, `Driven`, `Resolve`, `Pullup` and the `Logic` behind
    // them, are not in the library yet (E0161), as the Book's listing prints. The test fails when the file runs
    let chapter = Chapter::read();
    let model = chapter.marked("alarm.msl", "natures#3");
    let overrides = Overrides { analysis: Some("transient".to_string()), stop: Some(4e-6), streams: vec!["line".to_string()], ..Overrides::default() };
    let given = Refusal::of(model.run(overrides.clone()), "natures#3");
    assert!(given.iter().all(|(code, _)| code == "E0161"), "every diagnostic is a name the library does not hold yet: {given:?}");
    for name in ["Driven", "Resolve"] {
        assert!(given.iter().any(|(_, message)| message == &format!("cannot find `{name}` in this scope")), "{name}: {given:?}");
    }
    Refusal::expect(model.run(overrides), "natures#3", "E0161", "cannot find `Driven` in this scope");
}

#[test]
fn a_resistor_that_heats_itself_finds_the_temperature_where_power_in_equals_power_out() {
    // natures#7 (run): HotResistor and ThermalR of the Book on a 1 V source: "Connect HotResistor and
    // ThermalR to the same thermal node and the solver finds the temperature at which the power in equals the
    // power out": T = rth * V^2 / (r0 (1 + tc T)), that is 0.008 T^2 + 2 T - 1 = 0 for r0 = 100, tc = 4m, rth = 50
    let chapter = Chapter::read();
    let after = "module V(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for V { v(p, n) +> dc; }\nmodule Top() {\n    signal a: Electrical;\n    signal t: Thermal;\n    let s = V(a, gnd);\n    let h = HotResistor(a, gnd, t) { r0: 100.0 };\n    let th = ThermalR(t) { rth: 50.0 };\n}\n";
    let ran = chapter.wrapped("hot.msl", "", "natures#7", after).run(OperatingPoint::of_top()).unwrap_or_else(|failure| panic!("{failure}"));
    let temperature = (-2.0 + (4.0f64 + 4.0 * 0.008).sqrt()) / (2.0 * 0.008);
    assert!((ran.signal("Top.t") - temperature).abs() < 1e-6, "the solver's {:e} against the balance {temperature:e}", ran.signal("Top.t"));
    Printed::holds("4.990040e-1", ran.signal("Top.t"), "the temperature rise, in kelvin");
}

#[test]
fn a_bundle_is_refused_where_the_solver_reaches_it() {
    // natures#8 (refused): the Stream bundle and the Slice of the Book; natures~52790298 "Directions are
    // written once, from the producer's side, and flip gives the consumer's view" (refused with it). A
    // `struct` of carriers and a generic module are `P0001` (L25, L30)
    let chapter = Chapter::read();
    let placing = "module Top(in clk: Bit) { signal b: Stream<u8>; signal d: Stream<u8>; let s = Slice::<u8>(clk, b, d); }\n";
    let given = Refusal::of(chapter.wrapped("bundle.msl", "", "natures#8", placing).run(OperatingPoint::of_top()), "natures#8");
    assert!(given.contains(&("P0001".to_string(), "`struct` item used as a type is not supported yet".to_string())), "{given:?}");
    assert!(given.contains(&("P0001".to_string(), "generic module is not supported yet".to_string())), "{given:?}");
    let generic = "module Top(in clk: Bit) { signal c: Bit; let s = Slice::<u8>(c, gnd, gnd); }\n";
    Refusal::expect(chapter.wrapped("bundle.msl", "", "natures#8", generic).run(OperatingPoint::of_top()), "natures#8", "P0001", "generic module is not supported yet");
}

#[test]
fn an_out_port_gives_the_initial_value() {
    // natures~eb8db94c: "Only a one-driver discrete port has a direction ... the module with the out port.
    // That module also gives the initial value": `out q: Bit<u8> = 5` starts its carrier at 5
    let source = "module Src(out q: Bit<u8> = 5) { }\nmodule Top() { signal s: Bit<u8>; let a = Src(s); }\ndigital for Top { on at(1u) { emit!(\"q\", q = s); } }\n";
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(2e-6), ..Overrides::default() };
    let ran = Model::written("initial.msl", source).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.stream("q")[0].fields, [("q".to_string(), Field::Int(5))], "the carrier starts at the out port's value");
}

#[test]
fn a_module_reaches_other_nodes_only_through_its_ports_and_gnd_is_everywhere() {
    // natures~8ebfe866: "gnd is the reference node of every continuous nature. It is not a global net you
    // can abuse: a module reaches any other node only through its ports": a child that names a node of
    // its parent that is no port does not resolve (`E0161`), while `gnd` is written in any module
    let reaching = "module M(p: Electrical) { }\nanalog for M { v(other) +> 1.0; }\nmodule Top() { signal a: Electrical; signal other: Electrical; let m = M(a); }\n";
    let faulty = Faulty::elaborating("natures~8ebfe866", reaching, "Top");
    assert_eq!(faulty.first("E0161").message, "cannot find `other` in this scope");
    let ground = "module M(p: Electrical) { }\nanalog for M { v(p, gnd) +> 1.0; }\nmodule Top() { signal a: Electrical; let m = M(a); }\n";
    let ran = Model::written("ground.msl", ground).run(OperatingPoint::of_top()).unwrap_or_else(|failure| panic!("`gnd` is written in any module: {failure}"));
    assert_eq!(ran.signal("Top.a"), 1.0, "a source from `a` to `gnd` in a child");
}

#[test]
fn behavior_crosses_freely_inside_one_module() {
    // natures~5c7f5269: "Inside one module, behavior crosses freely: analog reads digital carriers, and
    // reactions probe voltages": one module whose reaction probes `v(vin)` and whose analog block reads the
    // carrier it committed through `known_or`
    let source = "module Hold(vin: Electrical, vout: Electrical) { signal held: Bit<f64> = 0.0; }\ndigital for Hold { on at(1u) { held <- v(vin); } }\nanalog for Hold { v(vout) +> held.known_or(0.0); }\nmodule Source(p: Electrical, n: Electrical) { dc: f64 = 2.0; }\nanalog for Source { v(p, n) +> dc; }\nmodule Top() { signal a: Electrical; signal b: Electrical; let s = Source(a, gnd); let h = Hold(a, b); }\n";
    let overrides = Overrides { analysis: Some("transient".to_string()), top: Some("Top".to_string()), stop: Some(3e-6), ..Overrides::default() };
    let ran = Model::written("cross.msl", source).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    let last = ran.table.rows.len() - 1;
    assert_eq!(ran.real(0, "Top.b"), 0.0, "before the reaction the analog side reads the initial 0.0");
    assert_eq!(ran.real(last, "Top.b"), 2.0, "after it the analog side reads what the reaction probed");
}

#[test]
fn e0220_a_bit_has_one_driver() {
    // natures~42ae741d: "E0220 two instances with an out port on the same Bit"
    let source = "module Source(out q: Bit) { }\nmodule Top() { signal s: Bit; let a = Source(s); let b = Source(s); }\n";
    let faulty = Faulty::elaborating("natures~42ae741d", source, "Top");
    assert_eq!(faulty.pointed_at("E0220"), "s");
}

#[test]
fn e0222_a_module_cannot_drive_its_own_input() {
    // natures~42ae761d: "E0222 req <- false; where req is an in port"
    let source = "module M(in req: Bit) { }\ndigital for M { on at(1u) { req <- false; } }\nmodule Top() { signal r: Bit; let m = M(r); }\n";
    let faulty = Faulty::elaborating("natures~42ae761d", source, "Top");
    assert!(faulty.first("E0222").message.contains("the target of a commit is an `in` port"), "{}", faulty.first("E0222").message);
}

#[test]
fn e0310_a_one_driver_port_needs_a_direction() {
    // natures~4bc8151d: "E0310 module M(clk: Bit)": and an Electrical port takes none
    let faulty = Faulty::elaborating("natures~4bc8151d", "module M(clk: Bit) { }\nmodule Top() { signal c: Bit; let m = M(c); }\n", "Top");
    assert_eq!(faulty.pointed_at("E0310"), "Bit");
    let directed = Faulty::elaborating("natures~4bc8151d", "module M(in p: Electrical) { }\nmodule Top() { signal a: Electrical; let m = M(a); }\n", "Top");
    assert_eq!(directed.first("E0310").message, "a port of a continuous nature takes no direction (§8.1.5)");
}

#[test]
fn e0250_discrete_and_continuous_never_connect() {
    // natures~42ab6e1d and natures~b038d791: "E0250 Resistor(clk, gnd) { r: 1k } with clk: Bit"
    let source = "module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nmodule Top() { signal clk: Bit; let r1 = Resistor(clk, gnd) { r: 1k }; }\n";
    let faulty = Faulty::elaborating("natures~42ab6e1d", source, "Top");
    assert_eq!(faulty.pointed_at("E0250"), "clk");
    assert_eq!(faulty.first("E0250").message, "a discrete carrier is connected to a port of node of nature `Electrical` (§6.1)");
}

#[test]
fn e0206_a_probe_must_be_one_the_nature_declares() {
    // natures~42b5621d: "E0206 temp(p) with p: Electrical": on Electrical the probes are v and i
    let faulty = Faulty::elaborating("natures~42b5621d", "module Top() { signal p: Electrical; }\nanalog for Top { v(p) +> temp(p); }\n", "Top");
    assert_eq!(faulty.first("E0206").message, "the nature of the first argument declares no access `temp`");
}

#[test]
fn e0240_a_branch_joins_two_nodes_of_one_nature() {
    // natures~42a7e81d: "E0240 v(p, t) with p: Electrical, t: Thermal"
    let faulty = Faulty::elaborating("natures~42a7e81d", "module Top() { signal p: Electrical; signal t: Thermal; }\nanalog for Top { v(p) +> v(p, t); }\n", "Top");
    assert_eq!(faulty.first("E0240").message, "the nodes of a probe have different natures");
}

#[test]
fn e0210_an_initial_value_cannot_read_a_carrier() {
    // natures~42b8e61d: "E0210 signal y: Bit<u8> = count;": an initial value is a constant
    let faulty = Faulty::elaborating("natures~42b8e61d", "module Top(in count: Bit<u8>) { signal y: Bit<u8> = count; }\n", "Top");
    assert!(faulty.first("E0210").message.starts_with("an initial value reads a carrier: it is committed at (0, 0)"), "{}", faulty.first("E0210").message);
}

#[test]
fn the_book_drives_nets_with_strength() {
    // EU-18 AC 4: the Book teaches nets through natures the reader declares (a trait is a capability,
    // `impl Resolve<X> for` a nature), has the section "Driving with strength" with the four worked nets in
    // order, answers why `resolve` is a function and why the library has no net, and names no library `Wire`.
    // The four listings are digital#17 to digital#20 (refused, S8); the alarm of natures#3 is the first net.
    let book = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/book/content.html")).expect("the Book");
    let chapter = |id: &str| {
        let start = book.find(&format!("<section id=\"{id}\"")).unwrap_or_else(|| panic!("chapter {id}"));
        book[start..start + book[start..].find("</section>").expect("a chapter closes")].to_string()
    };
    let natures = chapter("natures");
    assert!(natures.contains("<h3>What a nature can do: traits as capabilities</h3>"), "the capability section");
    assert!(natures.contains("a second one is error E0220: its message names the nature and <code>Resolve</code>"), "E0220 names the nature and Resolve");
    assert!(natures.contains("<code>resolve</code> is a function and not a module"), "why resolve is a function");
    assert!(natures.contains("the library declares no net nature"), "why there is no library net");
    assert!(natures.contains("<code>impl Resolve&lt;Driven&gt; for AlarmLine</code>") || natures.contains("impl Resolve<Driven> for AlarmLine"), "an impl on a nature the reader declares");
    let digital = chapter("digital");
    let at = digital.find("<h3>Driving with strength</h3>").expect("the section Driving with strength");
    let section = &digital[at..];
    let mut last = 0;
    for net in ["<strong>1. A plain bit into a wired-AND net.</strong>", "<strong>2. An open-drain line with a pull-up.</strong>", "<strong>3. A tri-state bus with two drivers and a keeper.</strong>", "<strong>4. A conflict is a value; an instability is an error.</strong>"] {
        let position = section.find(net).unwrap_or_else(|| panic!("the section lacks {net}"));
        assert!(position >= last, "the nets come in order: {net}");
        last = position;
    }
    for fact in ["impl Resolve<bool> for AndWire", "impl Resolve<Driven> for Sda", "impl Resolve<Driven> for TriBus", "impl Resolve<Option<u8>, Data> for DataBus", "After its bounded passes the run stops with error E0511"] {
        assert!(section.contains(fact), "the section lacks {fact}");
    }
    // no chapter of the Book names a library `Wire`: a `Wire` that is not the tail of a longer name (`AndWire`)
    let mut standalone = book.match_indices("Wire").filter(|(at, _)| !book[..*at].ends_with(|c: char| c.is_ascii_alphabetic()));
    assert!(standalone.next().is_none(), "the Book names a library Wire (AlarmLine and AndWire are the model's own)");
}
