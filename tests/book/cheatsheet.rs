//! Scope: chapter 16 of the Book, "Cheat sheet" (feature `root-e2e`, RE-02 and RE-03): the natures, the
//! analog block and the module with its params and signals as the sheet writes them (the first two run
//! or are refused with the form they show), the body items, the digital block, the debug prints, the
//! templates, the extern module and the file items (each refused today, asserted refused with its
//! diagnostics), and the sentence on what hides what, run through `piperine::`. Each test opens with the
//! inventory ids it holds, from `tests/book/inventory.tsv`.

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

use piperine::Overrides;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
use support::refusal::Refusal;
use support::standard_library::StandardLibrary;

/// The run the sheet's models need: an operating point of the top `Top`.
struct Commands;

impl Commands {
    /// `piperine run <file> --top Top --analysis op`.
    fn operating_point() -> Overrides {
        Faulty::operating_point_of("Top")
    }
}

/// What a refused sheet block is placed in, so its diagnostics are the ones of the block.
struct Surroundings;

impl Surroundings {
    /// A source and a resistor.
    const PARTS: &'static str = "module Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\n";

    /// The diagnostics `model` gives for its top `Top` as `(code, message)`, which must be a refusal.
    fn refusals(model: &Model, row: &str) -> Vec<(String, String)> {
        Refusal::of(model.run(Commands::operating_point()), row)
    }

    /// Whether `given` holds the diagnostic `message` under `code`.
    fn holds(given: &[(String, String)], code: &str, message: &str) -> bool {
        given.contains(&(code.to_string(), message.to_string()))
    }
}

#[test]
fn the_nature_card_is_the_standard_librarys() {
    // cheatsheet#1 (run): `Bit` and `Electrical` as the sheet writes them (the library declares no `Wire`, explicit-unknowns D20); each is a declaration of
    // `std/carriers.msl` or `std/nets.msl`, comments and `pub` aside, and the block is accepted
    let chapter = Chapter::read();
    let model = chapter.wrapped("natures.msl", "", "cheatsheet#1", "module Top() {}\n");
    model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("the block is refused: {failure}"));
    let library = StandardLibrary::normalized(&["carriers.msl", "nets.msl"]);
    let block = StandardLibrary::normalize(&chapter.text("cheatsheet#1"));
    let declarations: Vec<&str> = block.split("nature ").skip(1).collect();
    assert_eq!(declarations.len(), 2, "Bit and Electrical: {block}");
    for declaration in declarations {
        let declaration = format!("nature {}", declaration.trim());
        assert!(library.contains(&declaration), "the standard library declares `{declaration}`");
    }
}

#[test]
fn the_module_card_is_refused_for_its_const_generic() {
    // cheatsheet#2 (refused): `module M<const N: usize>(a: Electrical, in clk: Bit, out q: Bit<u8> = 0, x:
    // f64) where N >= 1 { .. }` with its params, an `Option<f64>` and an array of carriers, placed as
    // `M::<2>`: a generic module is `P0001` (L30)
    let chapter = Chapter::read();
    let after = "module Top() { signal a: Electrical; signal c: Bit; signal b: Bit<u8>; let m = M::<2>(a, c, b, 1.0); }\n";
    Refusal::expect(chapter.wrapped("shape.msl", "", "cheatsheet#2", after).run(Commands::operating_point()), "cheatsheet#2", "P0001", "generic module is not supported yet");
}

#[test]
fn the_body_items_card_is_refused_for_its_generation() {
    // cheatsheet#3 (refused): the instance, the named expression, the definition, the delayed definition and
    // `for k in 0..N { pub let rk = Resistor(a, mid) { r: 1k }; }`, placed in a body that declares the names
    // it uses: the generation `for` is `P0001` (L31)
    let chapter = Chapter::read();
    let before = "const N: u32 = 3;\nmodule Resistor(in p: Bit, in n: Bit) { r: f64 = 1k; }\nmodule Top() {\n    r: f64 = 1k;\n    tpd: f64 = 1n;\n    signal a: Bit;\n    signal b: Bit;\n    signal mid: Bit;\n    signal y: Bit;\n    signal q: Bit;\n    signal qn: Bit;\n";
    let model = chapter.wrapped("body.msl", before, "cheatsheet#3", "}\n");
    let given = Surroundings::refusals(&model, "cheatsheet#3");
    assert!(Surroundings::holds(&given, "P0001", "generation `for`/`if` in a module body is not supported yet"), "{given:?}");
    Refusal::expect(model.run(Commands::operating_point()), "cheatsheet#3", "P0001", "generation `for`/`if` in a module body is not supported yet");
}

#[test]
fn the_analog_card_runs() {
    // cheatsheet#4 (run): `analog for M` with a flow and a potential contribution, a `ddt` and `bound_step(1n)`:
    // placed on a 1 V source it draws 1 V / 1 k, drives `vout` to `gain * v(inp)` = 2 V and leaves the
    // capacitor open at DC
    let chapter = Chapter::read();
    let before = format!("{}module M(a: Electrical, b: Electrical, vout: Electrical, inp: Electrical, p: Electrical, n: Electrical) {{ r: f64 = 1k; gain: f64 = 2.0; c: f64 = 1p; }}\n", Surroundings::PARTS);
    let after = "module Top() { signal inp: Electrical; signal drive: Electrical; let s = Vdc(inp); let m = M(inp, gnd, drive, inp, drive, gnd); }\n";
    let ran = chapter.wrapped("contributions.msl", &before, "cheatsheet#4", after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.signal("Top.inp"), 1.0);
    assert!((ran.signal("Top.drive") - 2.0).abs() < 1e-9, "gain * v(inp): {}", ran.signal("Top.drive"));
}

#[test]
fn the_digital_card_runs() {
    // cheatsheet#5 (run): the four reactions, the last `on settle { require("hi", v(p, n) <= hi); }`: the
    // `settle` event runs since unknown-semantics-events T5 (L52). The third reaction commits the pulse to
    // `q`, a `Bit`, not to the node `p` the check probes (E0222 otherwise): the card says `q` since T6
    let chapter = Chapter::read();
    // `p` sits 0.5 V above `n`, both driven, so the check `v(p, n) <= hi` holds and the operating point is not singular
    let before = format!("{}module M(in clk: Bit, in rst_n: Bit, in d: Bit, out q: Bit = false, p: Electrical, n: Electrical, in en: Bit) {{ t0: f64 = 1u; w: f64 = 1n; hi: f64 = 1.0; }}\n", Surroundings::PARTS);
    let after = "module Top() { signal c: Bit = false; signal r: Bit = false; signal d: Bit = false; signal q: Bit; signal p: Electrical; signal n: Electrical; signal e: Bit = false; let sp = Vdc(p) { dc: 0.5 }; let sn = Vdc(n) { dc: 0.0 }; let m = M(c, r, d, q, p, n, e); }\n";
    let model = chapter.wrapped("digital.msl", &before, "cheatsheet#5", after);
    model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("cheatsheet#5, the digital card: {failure}"));
}

#[test]
fn the_debug_prints_card_is_refused_for_the_analog_trace() {
    // cheatsheet#7 (refused): the four reactions that print and `analog for M { emit!("newton", ..); }`: the
    // `settle` event runs (L52, closed) and an `emit!` in `analog` is `P0001` (L49)
    let chapter = Chapter::read();
    let before = "module M(a: Electrical, in clk: Bit, in d: Bit) { vth: f64 = 0.5; signal q: Bit; }\n";
    let after = "module Top() { signal a: Electrical; signal c: Bit; signal d: Bit; let m = M(a, c, d); }\n";
    let model = chapter.wrapped("prints.msl", before, "cheatsheet#7", after);
    let given = Surroundings::refusals(&model, "cheatsheet#7");
    assert!(Surroundings::holds(&given, "P0001", "an `emit!` in an `analog` block is not supported yet"), "{given:?}");
    assert!(!given.iter().any(|(_, message)| message.contains("`settle`")), "`settle` is no longer refused: {given:?}");
    Refusal::expect(model.run(Commands::operating_point()), "cheatsheet#7", "P0001", "an `emit!` in an `analog` block is not supported yet");
}

#[test]
fn the_template_card_is_refused_for_its_generic_module_and_its_placements() {
    // cheatsheet#8 (refused): `cell: C;`, `cell(Cell { wl: w, bl: b })` and `nch(0.13u, 2u)(d, g, s, b)` in a
    // module generic over the template: a generic module is `P0001` (L32, with the placement through a
    // function of modules#12)
    let chapter = Chapter::read();
    let before = "struct Cell { in wl: Bit, bl: Electrical }\nmodule Bsim4(d: Electrical, g: Electrical, s: Electrical, b: Electrical) { l: f64 = 1u; w: f64 = 1u; vth0: f64 = 0.4; }\npub fn nch(l: f64, w: f64) -> Bsim4 { Bsim4 { l: l, w: w, vth0: 0.4 } }\nmodule Inner<C: Fn(Cell)>() {\n    signal w: Bit;\n    signal b: Electrical;\n    signal d: Electrical;\n    signal g: Electrical;\n    signal s: Electrical;\n";
    let after = "}\nmodule Top() { let i = Inner::<Bsim4>(); }\n";
    Refusal::expect(chapter.wrapped("templates.msl", before, "cheatsheet#8", after).run(Commands::operating_point()), "cheatsheet#8", "P0001", "generic module is not supported yet");
}

#[test]
fn the_extern_card_is_refused() {
    // cheatsheet#9 (refused): `#[link("bsim4.osdi", model = "bsim4")] extern "osdi" module Bsim4(..)`, placed:
    // an `extern` module is `P0001` (L33)
    let chapter = Chapter::read();
    let after = "module Top() { signal d: Electrical; signal g: Electrical; signal s: Electrical; signal b: Electrical; let m = Bsim4(d, g, s, b); }\n";
    Refusal::expect(chapter.wrapped("foreign.msl", "", "cheatsheet#9", after).run(Commands::operating_point()), "cheatsheet#9", "P0001", "`extern` module is not supported yet");
}

#[test]
fn the_files_card_runs() {
    // cheatsheet#10 (run, L57 closed by params-and-io): `mod logic;`, two `use` lines, `const IV: [(f64, f64); 64] =
    // parse_table(include_str!("iv.tbl"));` and `emit!("log", ..)` among the items. The `emit!` is a statement among the
    // items, so the file as the sheet writes it is `E0601` (§19 rule 10); without that last line, in a package that has the
    // files and the dependency it names, and a top that reads the table (the checker reaches only what the top reaches),
    // the card runs: `parse_table` reads `iv.tbl` and `table_model` reads it
    let chapter = Chapter::read();
    let card = chapter.text("cheatsheet#10");
    let (items, statement) = card.rsplit_once('\n').expect("the card has several lines");
    assert!(statement.starts_with("emit!("), "the last line of the card is the `emit!`: {statement}");
    let manifest = "[package]\nname = \"adc\"\nversion = \"0.1.0\"\n\n[dependencies]\nutil = { path = \"deps/util\" }\n\n[run.op]\nanalysis = \"op\"\ntop = \"Top\"\n";
    let package = |root: String| {
        Model::package(manifest)
            .with_source("deps/util/Piperine.toml", "[package]\nname = \"util\"\nversion = \"0.1.0\"\n")
            .with_source("deps/util/src/lib.msl", "pub mod filters;\n")
            .with_source("deps/util/src/filters.msl", "pub module Rc(p: Electrical) {}\n")
            .with_source("src/lib.msl", &root)
            .with_source("src/logic.msl", "pub module Sar() {}\n")
            .with_source("src/iv.tbl", &(0..64).map(|row| format!("{} {}\n", row, row)).collect::<String>())
    };
    let whole = Refusal::of(package(format!("{card}\nmodule Top() {{ }}\n")).run(Overrides::default()), "cheatsheet#10");
    assert!(whole.iter().any(|(code, message)| code == "E0601" && message == "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)"), "{whole:?}");
    let reading = "module Top() { signal a: Electrical; }\nanalog for Top { i(a) +> table_model(v(a), IV, \"3L\"); }\n";
    let items_only = package(format!("{items}\n{reading}"));
    items_only.run(Overrides::default()).unwrap_or_else(|failure| panic!("the card without its statement runs: {failure:#}"));
}

#[test]
fn a_name_of_yours_hides_the_library_but_not_the_core() {
    // cheatsheet~a9561024: "Yours hides both, except a core name (E0457)": a `fn db20` of the crate is the one
    // its scope finds, where the library's `db20(10.0)` is 20 dB, and a `fn ddt` is `E0457`
    let own = "fn db20(x: f64) -> f64 { 42.0 }\nmodule Top() { signal a: Electrical; }\nanalog for Top { v(a) +> db20(10.0); }\n";
    let ran = Model::written("own.msl", own).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.signal("Top.a"), 42.0, "this crate's db20 hides the library's");
    let library = "module Top() { signal a: Electrical; }\nanalog for Top { v(a) +> db20(10.0); }\n";
    let ran = Model::written("library.msl", library).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.a") - 20.0).abs() < 1e-9, "the library's db20 of 10 is 20 dB: {}", ran.signal("Top.a"));
    let faulty = Faulty::elaborating("cheatsheet~a9561024", "fn ddt(x: f64) -> f64 { x }\nmodule Top() { }\n", "Top");
    assert_eq!(faulty.first("E0457").message, "`ddt` is a core primitive and cannot be redefined (§14 rule 1)");
}

#[test]
fn an_array_of_bits_with_no_initial_value_is_e0187() {
    // cheatsheet#2 amended (T116, BA2): the old card wrote `signal s: [Bit; N];`; `bool` has no `Default`
    // (§3.3.4, MD-78), so a carrier array nothing initializes is refused on a plain module.
    let source = "module M(in clk: Bit) { signal s: [Bit; 2]; }\nmodule Top() { signal c: Bit; let m = M(c); }\n";
    let faulty = Faulty::elaborating("cheatsheet#2", source, "Top");
    faulty.first("E0187");
}
