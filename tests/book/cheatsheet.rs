//! Scope: chapter 16 of the Book, "Cheat sheet" (feature `root-e2e`, RE-02 and RE-03): the natures, the
//! analog block and the module with its params and signals as the sheet writes them (the first two run
//! or are refused with the form they show), the body items, the digital block, the debug prints, the
//! templates and the file items (each refused today, asserted refused with its
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
fn the_module_card_runs_with_its_value_input_and_its_out_carrier() {
    // cheatsheet#2 (run, L76 closed by `generics`): `module M<const N: usize>(a: Electrical, in clk: Bit, out q: Bit<u8> = 0,
    // x: f64) where N >= 1 { .. }` placed as `M::<2>`. The value input `x` makes `M` a module that is called, hoisted into its
    // caller, and its `out` carrier `q` is the caller's own carrier for the argument `b`
    let chapter = Chapter::read();
    let after = "analog<N> for M<N> { v(mid) +> x; }\nmodule Top() { signal a: Electrical; signal c: Bit = false; signal b: Bit<u8>; let m = M::<2>(a, c, b, 1.0); }\nanalog for Top { v(a) +> 0.0; }\n";
    let ran = chapter.wrapped("shape.msl", "", "cheatsheet#2", after).run(Commands::operating_point()).unwrap_or_else(|failure| panic!("the card is refused: {:?}", Refusal::of(Err(failure), "cheatsheet#2")));
    let rows = format!("{:?}", ran.table.rows);
    assert!(rows.contains("Top.b") && rows.contains("Top.c"), "the operating point lists the top's signals: {rows}");
}

#[test]
fn the_module_card_runs_for_its_const_generic_without_its_value_input() {
    // cheatsheet#2 (run, T116 amended): the card's params, `Option<f64>` param, node signal and `signal s: [Bit; N] =
    // [false; N]`, written without the value input `x`, placed as `M::<2>`: `M` is a generic module with a body of its own
    let chapter = Chapter::read();
    let card = chapter.text("cheatsheet#2").replace(", x: f64)", ")");
    assert_ne!(card, chapter.text("cheatsheet#2"), "the card writes its value input");
    // the card is syntax alone: the harness holds the node `mid` and `a` with sources, or the operating point is singular
    let after = "analog<N> for M<N> { v(mid) +> 0.0; }\nmodule Top() { signal a: Electrical; signal c: Bit = false; signal b: Bit<u8>; let m = M::<2>(a, c, b); }\nanalog for Top { v(a) +> 0.0; }\n";
    let model = Model::written("shape.msl", &format!("{card}\n{after}"));
    let ran = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("the card is refused: {:?}", Refusal::of(Err(failure), "cheatsheet#2")));
    let rows = format!("{:?}", ran.table.rows);
    assert!(rows.contains("Top.b") && rows.contains("Top.c"), "the operating point lists the top's signals: {rows}");
}

#[test]
fn the_body_items_card_runs_with_its_generation() {
    // cheatsheet#3 (run): the instance, the named expression, the definition, the delayed definition and
    // `for k in 0..N { pub let rk = Resistor(a, mid) { r: 1k }; }`, placed in a body that declares the names it uses. The card
    // reads `y` in its delayed definition (it read the param `r`, a number, as a bit: BA4 of `generics`); the loop makes the
    // array `rk` of `N` instances, `rk[0]` to `rk[2]` here
    let chapter = Chapter::read();
    let before = "const N: usize = 3;\nmodule Resistor(in p: Bit, in n: Bit) { r: f64 = 1k; }\nmodule Top() {\n    r: f64 = 1k;\n    tpd: f64 = 1n;\n    signal a: Bit = false;\n    signal b: Bit = false;\n    signal mid: Bit = false;\n    signal y: Bit;\n    signal q: Bit = false;\n    signal qn: Bit = false;\n";
    let model = chapter.wrapped("body.msl", before, "cheatsheet#3", "}\n");
    let ran = model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("cheatsheet#3: {:?}", Refusal::of(Err(failure), "cheatsheet#3")));
    assert!(format!("{:?}", ran.table.rows).contains("Top.q"), "the operating point lists the top's signals");
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
fn the_debug_prints_card_runs_with_its_analog_trace() {
    // cheatsheet#7 (runs since analog-operators T37): the four reactions that print and `analog for M { emit!("newton", ..); }`:
    // the `settle` event (L52) and the trace in `analog` (L49) both run
    let chapter = Chapter::read();
    let before = "module M(a: Electrical, in clk: Bit, in d: Bit) { vth: f64 = 0.5; signal q: Bit = false; }\n";
    let after = "module Top() { signal a: Electrical; signal c: Bit = false; signal d: Bit = false; let m = M(a, c, d); }\nanalog for Top { v(a, gnd) +> 0.7; }\n";
    let model = chapter.wrapped("prints.msl", before, "cheatsheet#7", after);
    model.run(Commands::operating_point()).unwrap_or_else(|failure| panic!("cheatsheet#7: {failure}"));
}

#[test]
fn the_template_card_runs() {
    // cheatsheet#8 (run): `cell: C;`, `cell(Cell { wl: w, bl: b })` and `nch(0.13u, 2u)(d, g, s, b)` in a module generic over the
    // template (generics phase D, and the bundle `Cell` of `resolved-nets-bundles`). The harness gives the module the sheet's
    // names mean: a `Dram` whose one port is a `Cell` and which holds its bit line at 0.25 V, a `Bsim4` that pulls `d` to its
    // `vth0` (0.4, what `nch` gives) and grounds `g` and `s`, and a parent that places `Inner` over the template `Dram {}`
    let chapter = Chapter::read();
    let before = "struct Cell { in wl: Bit, bl: Electrical }\nmodule Dram(c: Cell) { }\nanalog for Dram { i(c.bl, gnd) +> v(c.bl) - 0.25; }\nmodule Bsim4(d: Electrical, g: Electrical, s: Electrical, b: Electrical) { l: f64 = 1u; w: f64 = 1u; vth0: f64 = 0.4; }\nanalog for Bsim4 { i(d, gnd) +> v(d) - vth0; i(g, gnd) +> v(g); i(s, gnd) +> v(s); }\npub fn nch(l: f64, w: f64) -> Bsim4 { Bsim4 { l: l, w: w, vth0: 0.4 } }\nmodule Inner<C: Fn(Cell)>() {\n    signal w: Bit = false;\n    pub signal b: Electrical;\n    pub signal d: Electrical;\n    signal g: Electrical;\n    signal s: Electrical;\n";
    let after = "}\nmodule Top() { let i = Inner::<Dram>() { cell: Dram { } }; }\n";
    let overrides = Overrides { print: ["Top.i.b", "Top.i.d"].map(String::from).to_vec(), ..Commands::operating_point() };
    let ran = chapter.wrapped("templates.msl", before, "cheatsheet#8", after).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Top.i.b") - 0.25).abs() < 1e-9, "the cell holds its bit line: {}", ran.signal("Top.i.b"));
    assert!((ran.signal("Top.i.d") - 0.4).abs() < 1e-9, "the binned transistor pulls its drain to `vth0`: {}", ran.signal("Top.i.d"));
}

#[test]
fn the_files_card_runs() {
    // cheatsheet#9 (run, L57 closed by params-and-io): `mod logic;`, two `use` lines, `const IV: [(f64, f64); 64] =
    // parse_table(include_str!("iv.tbl"));` and `emit!("log", ..)` among the items. The `emit!` is a statement among the
    // items, so the file as the sheet writes it is `E0601` (§19 rule 10); without that last line, in a package that has the
    // files and the dependency it names, and a top that reads the table (the checker reaches only what the top reaches),
    // the card runs: `parse_table` reads `iv.tbl` and `table_model` reads it
    let chapter = Chapter::read();
    let card = chapter.text("cheatsheet#9");
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
    let whole = Refusal::of(package(format!("{card}\nmodule Top() {{ }}\n")).run(Overrides::default()), "cheatsheet#9");
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
    let source = "module M(in clk: Bit) { signal s: [Bit; 2]; }\nmodule Top() { signal c: Bit = false; let m = M(c); }\n";
    let faulty = Faulty::elaborating("cheatsheet#2", source, "Top");
    assert_eq!(faulty.given.iter().map(|reported| reported.code.as_str()).collect::<Vec<_>>(), ["E0187"], "only the array is refused");
    assert!(faulty.first("E0187").message.starts_with("`s` has no initial value and its type `bool` does not implement `Default`"), "{}", faulty.first("E0187").message);
    // the same hole through a width, a wider carried type, and an array read by a reaction in the top itself
    for source in [
        "module M(in clk: Bit) { signal s: [Bit<u8>; 2]; }\nmodule Top() { signal c: Bit = false; let m = M(c); }\n",
        "module M<const N: usize>(in clk: Bit) { signal s: [Bit; N]; }\nmodule Top() { signal c: Bit = false; let m = M::<2>(c); }\n",
        "module Top() { signal c: Bit = false; signal s: [Bit; 2]; }\ndigital for Top { on at(1u) { s[0] <- c; } }\n",
    ] {
        let faulty = Faulty::elaborating("cheatsheet#2", source, "Top");
        assert!(faulty.first("E0187").message.starts_with("`s` has no initial value"), "{source}: {}", faulty.first("E0187").message);
    }
}
