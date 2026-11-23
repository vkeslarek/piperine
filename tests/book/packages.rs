//! Scope: chapter 12 of the Book, "Files, modules and packages" (feature `root-e2e`, RE-02 and RE-03):
//! the `lowpass` project assembled from the chapter's four files, the crate root of the bigger `adc`
//! project, the `use` that is ambiguous between a local namespace and a dependency and its two ways out,
//! the `include_str!` table (refused today, asserted refused), the rules that split a crate over files
//! and a manifest, and each mistake of the chapter's table, run through `piperine::`. Each test opens
//! with the inventory ids it holds, from `tests/book/inventory.tsv`.

#[allow(dead_code)] // the support modules are shared by every Book target; this one uses part of each
#[path = "support"]
mod support {
    pub mod book;
    pub mod chapter;
    pub mod inventory;
    pub mod model;
    pub mod records;
    pub mod refusal;
}

use piperine::{Cell, Overrides};
use support::chapter::Chapter;
use support::model::Model;
use support::refusal::Refusal;

/// The manifest of a package with one run, an operating point of `top`, and `extra` after it.
struct Manifest;

impl Manifest {
    /// `[package]` of the package `name`, then the tables `extra`, then `[run.op]` of the top `top`.
    fn of(name: &str, extra: &str, top: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n{extra}[run.op]\nanalysis = \"op\"\ntop = \"{top}\"\n")
    }
}

/// The `lowpass` project of the chapter, one file per block.
struct Lowpass;

impl Lowpass {
    /// The project as the chapter lays it out: the manifest of packages#5 and the three sources of
    /// packages#2, packages#3 and packages#4.
    fn model(chapter: &Chapter) -> Model {
        Model::package(&chapter.text("packages#5"))
            .with_source("src/parts.msl", &format!("{}\n", chapter.text("packages#2")))
            .with_source("src/filter.msl", &format!("{}\n", chapter.text("packages#3")))
            .with_source("src/lib.msl", &format!("{}\n", chapter.text("packages#4")))
    }
}

/// The package with the dependency `util` (a package with `pub mod cells;` and `pub module Cell`) in
/// `deps/util`, whose own sources a test fills.
struct WithDependency;

impl WithDependency {
    /// The manifest naming the dependency `util` and its path.
    const EXTRA: &'static str = "[dependencies]\nutil = { path = \"deps/util\" }\n\n";

    /// A package `adc` whose root is `root`, depending on `util` as the Book's example does.
    fn model(root: &str) -> Model {
        Model::package(&Manifest::of("adc", Self::EXTRA, "Top"))
            .with_source("deps/util/Piperine.toml", "[package]\nname = \"util\"\nversion = \"0.1.0\"\n")
            .with_source("deps/util/src/lib.msl", "pub mod cells;\npub mod filters;\n")
            .with_source("deps/util/src/cells.msl", "pub module Cell(p: Electrical) {}\n")
            .with_source("deps/util/src/filters.msl", "pub module Rc(p: Electrical) {}\n")
            .with_source("src/lib.msl", root)
    }
}

#[test]
fn the_lowpass_project_runs_as_the_book_assembles_it() {
    // packages#2, packages#3 and packages#4 (run): `parts.msl`, `filter.msl` and `lib.msl` of the project;
    // packages#5 is its manifest and packages#7 the listing of `piperine run`. The three files, the
    // manifest with one run and no name asked for. "Splitting a design changes nothing about its
    // instances: the capacitor is still Bench.lp.c1": the paths follow the hierarchy of the design, not
    // the files it is written in
    let chapter = Chapter::read();
    let ran = Lowpass::model(&chapter).run(Overrides::default()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.signal("Bench.vin"), 1.0, "the source's 1 V");
    assert!((ran.signal("Bench.vout") - 1.0).abs() < 1e-9, "at DC the output equals the input");
    let corner = ran.metadata.params.iter().find(|param| param.path == "Bench.lp.c1.c").unwrap_or_else(|| panic!("no param Bench.lp.c1.c in {:?}", ran.metadata.params.iter().map(|param| &param.path).collect::<Vec<_>>()));
    match corner.value {
        Cell::Real(capacitance) => assert!((capacitance - 1.0 / (2.0 * std::f64::consts::PI * 1e3 * 10e3)).abs() < 1e-12, "the capacitor of the 10 kHz corner: {capacitance:e}"),
        ref other => panic!("a real capacitance: {other:?}"),
    }
}

#[test]
fn the_crate_root_of_the_bigger_project_loads_its_files_and_reaches_its_dependency() {
    // packages#9 (run): the `adc` crate root: `pub mod frontend;` and `pub mod logic;` load `src/frontend.msl` and
    // `src/logic/mod.msl`, `use util::filters::Rc;` reaches the dependency named in the manifest and
    // `use crate::logic::sar::SarLogic;` the file below `logic`; "the crate root is always src/lib.msl, and
    // `mod name;` loads name.msl or name/mod.msl". The Book shows only the root: the files below it and the
    // dependency are written by the test
    let chapter = Chapter::read();
    let root = format!("{}\nmodule Top() {{ let r = Rc(gnd); let s = SarLogic(); }}\n", chapter.text("packages#9"));
    let model = WithDependency::model(&root).with_source("src/frontend.msl", "module Frontend() {}\n").with_source("src/logic/mod.msl", "pub mod sar;\n").with_source("src/logic/sar.msl", "pub module SarLogic() {}\n");
    model.run(Overrides::default()).unwrap_or_else(|failure| panic!("{failure}"));
}

#[test]
fn a_local_namespace_and_a_dependency_of_one_name_make_the_use_ambiguous() {
    // packages#13 (refused): `mod util;` and `use util::cells::Cell;`, with a dependency `util`: `E0163`, "the
    // local `util` or the dependency `util`?"; packages~39a5ce1d "E0163 mod util; and a dependency util, then
    // use util::...": ambiguous first segment; packages#15 (fragment): `use crate::util::cells::Cell;` names
    // the local namespace and the package runs; packages#14 (shown) renames the dependency, `utilx`, which
    // runs too
    let chapter = Chapter::read();
    let local = "pub mod cells;\n";
    let cells = "pub module Other(p: Electrical) {}\n";
    let top = "module Top() { let c = Cell(gnd); }\n";
    let ambiguous = WithDependency::model(&format!("{}\n{top}", chapter.text("packages#13"))).with_source("src/util.msl", local).with_source("src/util/cells.msl", cells);
    Refusal::expect(ambiguous.run(Overrides::default()), "packages#13", "E0163", "`util` names both a dependency and an item of this namespace");
    let local_one = WithDependency::model(&format!("mod util;\n{}\n{top}", chapter.text("packages#15"))).with_source("src/util.msl", "pub mod cells;\n").with_source("src/util/cells.msl", "pub module Cell(p: Electrical) {}\n");
    local_one.run(Overrides::default()).unwrap_or_else(|failure| panic!("`crate::util` names the local namespace: {failure}"));
    let renamed = Model::package(&Manifest::of("adc", "[dependencies]\nutilx = { path = \"deps/util\", package = \"util\" }\n\n", "Top"))
        .with_source("deps/util/Piperine.toml", "[package]\nname = \"util\"\nversion = \"0.1.0\"\n")
        .with_source("deps/util/src/lib.msl", "pub mod cells;\n")
        .with_source("deps/util/src/cells.msl", "pub module Cell(p: Electrical) {}\n")
        .with_source("src/lib.msl", "mod util;\nuse utilx::cells::Cell;\nmodule Top() { let c = Cell(gnd); }\n")
        .with_source("src/util.msl", "pub mod cells;\n")
        .with_source("src/util/cells.msl", "pub module Other(p: Electrical) {}\n");
    renamed.run(Overrides::default()).unwrap_or_else(|failure| panic!("renaming the dependency in the manifest settles it: {failure}"));
}

#[test]
fn a_function_named_like_a_dependency_does_not_clash_with_it() {
    // packages~31625632: "A local fn util or const util lives in the value namespace, so util::cells::Cell
    // still reaches the crate": a `fn util` next to `use util::cells::Cell;` resolves to the dependency
    let root = "fn util() -> f64 { 1.0 }\nuse util::cells::Cell;\nmodule Top() { let c = Cell(gnd); }\n";
    WithDependency::model(root).run(Overrides::default()).unwrap_or_else(|failure| panic!("the value `util` does not shadow the crate: {failure}"));
}

#[test]
fn a_crate_reaches_another_only_through_its_manifest() {
    // packages~62e1271f: "A crate reaches another crate only through a dependency named in its manifest":
    // the same `use util::cells::Cell;` in a package whose manifest names no dependency does not resolve,
    // though a `util` package sits on disk beside it
    let model = Model::package(&Manifest::of("adc", "", "Top"))
        .with_source("util/Piperine.toml", "[package]\nname = \"util\"\nversion = \"0.1.0\"\n")
        .with_source("util/src/lib.msl", "pub mod cells;\n")
        .with_source("util/src/cells.msl", "pub module Cell(p: Electrical) {}\n")
        .with_source("src/lib.msl", "use util::cells::Cell;\nmodule Top() { let c = Cell(gnd); }\n");
    Refusal::expect(model.run(Overrides::default()), "packages~62e1271f", "E0161", "cannot find `util` in this scope");
}

#[test]
fn the_include_str_table_runs() {
    // packages#12 (run, L57 closed by params-and-io PI-09 and PI-10): `const IV: [(f64, f64); 64] =
    // parse_table(include_str!("diode_iv.tbl"));` and `ROM` of `parse_hex(include_str!("boot.hex"))`, with the measured
    // diode's `table_model(v(a, k), IV, "3E")`, read from the files beside the source at elaboration: the package builds
    // and its operating point solves, the diode at 50 mV inside the table
    let chapter = Chapter::read();
    let table: String = (0..64).map(|row| format!("{} {:e}\n", f64::from(row) * 0.01, f64::from(row) * 1e-12)).collect();
    let rom: String = (0..1024).map(|word| format!("{word:x}\n")).collect();
    let holder = "module Hold(p: Electrical) { volts: f64 = 0.0; }\nanalog for Hold { v(p, gnd) +> volts; }\n";
    let top = "module Top() { signal a: Electrical; signal k: Electrical; let h1 = Hold { volts: 0.05 }(a); let h2 = Hold(k); let m = MeasuredDiode(a, k); }";
    let root = format!("{}\n{holder}{top}\n", chapter.text("packages#12"));
    let model = Model::package(&Manifest::of("iv", "", "Top")).with_source("src/lib.msl", &root).with_source("src/diode_iv.tbl", &table).with_source("src/boot.hex", &rom);
    let ran = model.run(Overrides::default()).unwrap_or_else(|failure| panic!("the measured diode runs from its files: {failure:#}"));
    assert!((ran.signal("Top.a") - 0.05).abs() < 1e-9, "the source holds the anode at 50 mV, inside the table");
}

#[test]
fn e0610_is_raised_for_include_str_in_a_module() {
    // packages~64f2121d: "E0610 let t = include_str!(\"iv.tbl\"); in a module": `include_str!` is for `const`
    // items (section 20 rule 1, PI-08 AC 2), so a use in a module body is `E0610`
    let source = "module Top() { signal a: Electrical; let t = include_str!(\"iv.tbl\"); }\n";
    let model = Model::package(&Manifest::of("iv", "", "Top")).with_source("src/lib.msl", source).with_source("src/iv.tbl", "0 0\n");
    let given = Refusal::of(model.run(Overrides::default()), "packages~64f2121d");
    assert!(given.iter().any(|(code, _)| code == "E0610"), "{given:?}");
    assert!(given.iter().all(|(_, message)| !message.contains("`include_str!` is not supported")), "{given:?}");
}

#[test]
fn e0161_a_private_item_and_a_module_file_that_is_not_there_or_is_there_twice() {
    // packages~39a5d01d: "E0161 use crate::parts::Resistor; with module Resistor not pub" and "E0161 mod
    // filters; with the file named filter.msl"; packages~5018120b "Having neither, or both, is E0161": a
    // private module reached from another file, a `mod` with no file, and a `mod` with both `name.msl` and
    // `name/mod.msl`
    let parts = "module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\n";
    let private = Model::package(&Manifest::of("pk", "", "Top")).with_source("src/parts.msl", parts).with_source("src/lib.msl", "pub mod parts;\nuse crate::parts::Resistor;\nmodule Top() { signal a: Electrical; let r = Resistor(a, gnd) { r: 1k }; }\n");
    Refusal::expect(private.run(Overrides::default()), "packages~39a5d01d", "E0161", "`Resistor` is private");
    let neither = Model::package(&Manifest::of("pk", "", "Top")).with_source("src/filter.msl", parts).with_source("src/lib.msl", "mod filters;\nmodule Top() { }\n");
    let given = Refusal::of(neither.run(Overrides::default()), "packages~5018120b");
    assert!(given.iter().any(|(code, message)| code == "E0161" && message.starts_with("cannot find module `filters`: neither ")), "{given:?}");
    let both = Model::package(&Manifest::of("pk", "", "Top")).with_source("src/filters.msl", "module X() {}\n").with_source("src/filters/mod.msl", "module Y() {}\n").with_source("src/lib.msl", "mod filters;\nmodule Top() { }\n");
    let given = Refusal::of(both.run(Overrides::default()), "packages~5018120b");
    assert!(given.iter().any(|(code, message)| code == "E0161" && message.starts_with("module `filters` is ambiguous: both ")), "{given:?}");
}

#[test]
fn e0401_a_behavior_block_lives_in_the_file_of_its_module() {
    // packages~54e1b71d: "E0401 analog for Resistor { ... } in filter.msl", where `filter.msl` imports
    // `Resistor` from `parts.msl`: move it to parts.msl
    let chapter = Chapter::read();
    let model = Lowpass::model(&chapter).with_source("src/filter.msl", "analog for Resistor { i(p, n) +> v(p, n) / r; }\n");
    Refusal::expect(model.run(Overrides::default()), "packages~54e1b71d", "E0401", "`Resistor` is declared in another file: a module and its behavior blocks share one file (§3.4.3)");
}

#[test]
fn e0162_one_name_in_one_namespace_is_declared_once() {
    // packages~39a5cf1d: "E0162 use crate::parts::Resistor; beside a local module Resistor": rename one, or
    // import with `as`
    let parts = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\n";
    let root = "pub mod parts;\nuse crate::parts::Resistor;\nmodule Resistor(p: Electrical, n: Electrical) { r: f64 = 1.0; }\nmodule Top() { }\n";
    let model = Model::package(&Manifest::of("pk", "", "Top")).with_source("src/parts.msl", parts).with_source("src/lib.msl", root);
    Refusal::expect(model.run(Overrides::default()), "packages~39a5cf1d", "E0162", "`Resistor` is defined more than once in this namespace");
}

#[test]
fn e0171_a_manifest_has_only_package_and_dependencies() {
    // packages~77a1260e and packages~39a2cc1d: "Any other table or key is E0171"; "E0171 [dev-dependencies] in
    // Piperine.toml": an unknown table, and an unknown key of `[package]`
    let table = Model::package(&Manifest::of("pk", "[dev-dependencies]\nx = 1\n\n", "Top")).with_source("src/lib.msl", "module Top() { }\n");
    Refusal::expect(table.run(Overrides::default()), "packages~39a2cc1d", "E0171", "unknown table or key `dev-dependencies` in the manifest");
    let key = Model::package("[package]\nname = \"pk\"\nversion = \"0.1.0\"\nlicense = \"MIT\"\n\n[run.op]\nanalysis = \"op\"\ntop = \"Top\"\n").with_source("src/lib.msl", "module Top() { }\n");
    Refusal::expect(key.run(Overrides::default()), "packages~77a1260e", "E0171", "unknown key `license` in `[package]`");
}

#[test]
fn e0175_the_crate_root_is_always_src_lib_msl() {
    // packages~39a2c81d: "E0175 a package with src/main.msl only"
    let model = Model::package(&Manifest::of("pk", "", "Top")).with_source("src/main.msl", "module Top() { }\n");
    let given = Refusal::of(model.run(Overrides::default()), "packages~39a2c81d");
    assert_eq!(given.len(), 1, "{given:?}");
    assert_eq!(given[0].0, "E0175");
    assert!(given[0].1.starts_with("the package `pk` has no crate root ") && given[0].1.ends_with("src/lib.msl`"), "{}", given[0].1);
}
