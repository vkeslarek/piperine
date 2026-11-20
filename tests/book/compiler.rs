//! Scope: chapter 15 of the Book, "From source to solver" (feature `root-e2e`, RE-02 and RE-03): the
//! divider the chapter runs and the diode it can be swapped for, run through `piperine::`; the printed
//! intermediate form, the generated Rust and the library the Rust backend loads; the three compiler
//! codes a source can raise (`P0001` to `P0003`; `P0004` is an internal error no source raises); the
//! fault a digital `if` on an unknown value gives; and the comparison with ngspice the chapter
//! describes (a twin and a scenario test per circuit, the reverse-bias sweep measured against ngspice).
//! Each test opens with the inventory ids it holds, from `tests/book/inventory.tsv`.

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

use std::path::{Path, PathBuf};

use piperine::{Cell, Overrides, Project, Session};
use piperine_cli::{Cli, Invocation};
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::Design;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::corpus::{Corpus, Twin};
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_transpile::Settings;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::Model;
use support::printed::Printed;
use support::refusal::Refusal;

/// The divider of the chapter, laid out as the chapter says: `divider.msl` is the block, `devices.msl`
/// is the library the twin of the same circuit loads.
struct Divider;

impl Divider {
    /// The model: the block `compiler#1` beside the twin's `devices.msl`.
    fn model(chapter: &Chapter) -> Model {
        let devices = std::fs::read_to_string(Corpus::of_repository().twin("divider").directory().join("devices.msl")).expect("the twins' devices.msl");
        Model::written("divider.msl", &format!("{}\n", chapter.text("compiler#1"))).with_source("devices.msl", &devices)
    }

    /// The operating point of the top `Divider`.
    fn operating_point() -> Overrides {
        Faulty::operating_point_of("Divider")
    }
}

/// The files a session built into its own cache directory: the source the Rust backend printed and
/// the library it compiled from it.
struct BuiltFiles {
    /// Every file under the cache, by path.
    files: Vec<PathBuf>,
    /// The cache directory, removed when this value is dropped.
    _cache: tempfile::TempDir,
}

impl BuiltFiles {
    /// Opens a session of `model` on the compiled backend, building into a new cache directory.
    fn of(model: &Model, overrides: &Overrides) -> BuiltFiles {
        let cache = tempfile::tempdir().expect("a cache directory");
        let project = Project::file(model.opened_path()).unwrap_or_else(|failure| panic!("{failure}"));
        let spec = project.request(None, &Overrides { backend: Some("rust".to_string()), ..overrides.clone() }).unwrap_or_else(|failure| panic!("{failure}"));
        Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() }).unwrap_or_else(|failure| panic!("{failure}"));
        let mut files = Vec::new();
        Self::collect(cache.path(), &mut files);
        BuiltFiles { files, _cache: cache }
    }

    /// Adds every file under `directory` to `files`.
    fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).expect("a readable cache directory") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                Self::collect(&path, files);
            } else {
                files.push(path);
            }
        }
    }

    /// The design's own files, `design.rs` and the library beside it, not the shared ABI crate's.
    fn of_the_design(&self, name: &str) -> Vec<&PathBuf> {
        self.files.iter().filter(|path| path.file_name().is_some_and(|file| file == name) && !path.to_string_lossy().contains("/abi-")).collect()
    }
}

/// The twin of a circuit of the chapter's comparison table, and the command of its scenario.
struct Circuit;

impl Circuit {
    /// The overrides of the twin's `.command`, parsed as the binary parses it.
    fn overrides(twin: &Twin) -> Overrides {
        let line = std::iter::once("piperine".to_string()).chain(twin.command_arguments());
        match Cli::parse(line) {
            Ok(Invocation::Run { overrides, .. }) => overrides,
            Ok(other) => panic!("{}: the command is a `{}`, not a `run`", twin.name(), other.name()),
            Err(error) => panic!("{}: the command does not parse: {error}", twin.name()),
        }
    }

    /// The twin's model run as its command asks: the twin's own `.msl` and `devices.msl`, copied.
    fn model(twin: &Twin) -> Model {
        let model = std::fs::read_to_string(twin.model()).expect("the twin's model");
        let devices = std::fs::read_to_string(twin.directory().join("devices.msl")).expect("the twins' devices.msl");
        let file = twin.model().file_name().map(|name| name.to_string_lossy().into_owned()).expect("a file name");
        Model::written(&file, &model).with_source("devices.msl", &devices)
    }
}

#[test]
fn the_divider_puts_mid_at_7_5_volts_and_ngspice_prints_the_same() {
    // compiler#1 (run): the divider, a 10 V source over 1k and 3k; compiler~05f756b4 "Its operating point
    // puts mid at 7.5 V, the same number ngspice prints for the same circuit."
    let chapter = Chapter::read();
    let ran = Divider::model(&chapter).run(Divider::operating_point()).unwrap_or_else(|failure| panic!("{failure}"));
    let mid = ran.signal("Divider.mid");
    assert!((mid - 7.5).abs() < 1e-9, "10 V * 3k / (1k + 3k): {mid}");
    match Ngspice::from_environment().run("divider") {
        Reference::Golden(golden) => {
            let wanted = golden.printed("v(mid)");
            assert!((wanted - 7.5).abs() < 1e-9, "ngspice prints the same number: {wanted}");
            ToleranceContract::DC_VOLTAGE.assert(&Point::new("divider", "v(mid)", Coordinate::OperatingPoint), wanted, mid);
        }
        Reference::Skipped => {}
    }
}

#[test]
fn the_diode_with_a_limited_junction_settles_where_the_junction_equation_says() {
    // compiler#2 (run): `Diode`, `limit(..)`, `gmin()` and `ddt(cj0 * vd)`: fed from 5 V through 1k, its
    // node settles where the current of the resistor is the junction's (is = 1e-14, n = 1, 300.15 K)
    let chapter = Chapter::read();
    let before = "module Vdc(p: Electrical) { dc: f64 = 1.0; }\nanalog for Vdc { v(p) +> dc; }\nmodule R(p: Electrical, n: Electrical) { r: f64 = 1k; }\nanalog for R { i(p, n) +> v(p, n) / r; }\nmodule Diode(a: Electrical, k: Electrical) { n: f64 = 1.0; is: f64 = 1e-14; cj0: f64 = 0.0; }\n";
    let after = "module Top() { signal s: Electrical; signal a: Electrical; let v = Vdc(s) { dc: 5.0 }; let r = R(s, a); let d = Diode(a, gnd); }\n";
    let ran = chapter.wrapped("diode.msl", before, "compiler#2", after).run(Faulty::operating_point_of("Top")).unwrap_or_else(|failure| panic!("{failure}"));
    let a = ran.signal("Top.a");
    let thermal = 1.380649e-23 * 300.15 / 1.602176634e-19;
    let through_resistor = (5.0 - a) / 1000.0;
    let through_junction = 1e-14 * ((a / thermal).exp() - 1.0) + 1e-12 * a;
    assert!((through_resistor - through_junction).abs() <= 0.01 * through_resistor, "{through_resistor:e} A through the resistor, {through_junction:e} A through the junction at {a} V");
}

#[test]
fn the_rust_backend_prints_one_source_file_and_loads_the_library_it_builds() {
    // compiler~931761f9 "The Rust backend prints the whole design as one Rust source file, compiles it
    // with the rustc it finds on the machine, and loads the library through the device ABI."; compiler~13572839
    // "The block is generic over a number type: run with plain floats it gives the current, run with dual
    // numbers it gives the current and its derivatives, which is the Jacobian."
    let chapter = Chapter::read();
    let built = BuiltFiles::of(&Divider::model(&chapter), &Divider::operating_point());
    let sources = built.of_the_design("design.rs");
    assert_eq!(sources.len(), 1, "one source file for the whole design: {:?}", built.files);
    assert_eq!(built.of_the_design("libdesign.so").len(), 1, "and the library compiled from it, beside it");
    let source = std::fs::read_to_string(sources[0]).expect("the generated source");
    assert!(source.contains("use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};"), "the library is loaded through the device ABI");
    assert!(source.contains("// Resistor.analog @ devices.msl:4:1"), "every item names the MSL it came from, as the listing of compiler#3 says");
    assert!(source.contains("// contribution to i(p, n), branch b0 @ devices.msl:8:34"), "and the position of the contribution inside it");
    assert!(source.contains("fn terms_block<R: Real>(p: R, n: R, r: R) -> [R; 1] {"), "the block is generic over a number type");
    assert!(source.contains("let b0_flow_ = (p - n) / r;"), "the current of the resistor, over any number type");
    assert!(source.contains("Self::terms_block(Dual::<2>::seeded(") && source.contains("Dual::<2>::constant(r)"), "run with dual numbers it also gives the derivatives");
}

#[test]
fn a_compiler_code_says_what_the_chapter_says() {
    // compiler~79f44db1 "P0001": valid MSL the compiler cannot run yet, naming the construct;
    // compiler~79f450b1 "P0002": a value of the wrong type, or a call with the wrong number of arguments;
    // compiler~79f44fb1 "P0003": the top module asked for does not exist. (compiler~79f44ab1 "P0004" is
    // a bug of Piperine no source raises: a `prose` row.)
    let generic = "module Gen<const N: u32>() { }\nmodule Top() { let g = Gen::<2>(); }\n";
    let given = Refusal::of(Model::written("m.msl", generic).run(Faulty::operating_point_of("Top")), "compiler~79f44db1");
    assert!(given.iter().any(|(code, message)| code == "P0001" && message.contains("not supported yet")), "P0001 names the construct it cannot run: {given:?}");
    let arguments = "module Top() { signal a: Electrical; }\nanalog for Top { i(a, gnd) +> (2.0).powf(); }\n";
    Refusal::expect(Model::written("m.msl", arguments).run(Faulty::operating_point_of("Top")), "compiler~79f450b1", "P0002", "mismatched types: expected 1 argument(s), found 0");
    Refusal::expect(Model::written("m.msl", "module Top() { }\n").run(Faulty::operating_point_of("Nope")), "compiler~79f44fb1", "P0003", "`Nope` is not a module of the root crate");
}

#[test]
fn a_digital_if_on_an_unknown_value_runs_both_arms_and_merges_what_they_commit() {
    // compiler chapter, "Digital code reads the same way: a reaction is a function whose commits go out
    // together when it ends, and an if on a value that may be unknown runs both arms and merges what they
    // commit." (`d` is unknown: one arm commits `true` to `q`, the other commits nothing, so `q` is
    // `true` merged with its prior `false` and reads unknown; the run does not stop)
    let source = "module Top() { signal d: Bit; signal q: Bit = false; }\ndigital for Top { on at(1u) { if d { q <- true; } } }\n";
    let ran = Model::written("m.msl", source).run(Faulty::transient_of_top(3e-6, None)).unwrap_or_else(|failure| panic!("{failure:#}"));
    let column = ran.table.columns.iter().position(|name| name == "Top.q").expect("the table shows q");
    assert_eq!(ran.table.rows.last().expect("a last row")[column], Cell::Unknown, "q merges its two arms");
}

#[test]
fn the_intermediate_form_prints_in_a_stable_form() {
    // compiler~696ad77d "It is an object model, not a text format, but it prints in a stable form for
    // reading and for tests.": the same source prints the same text, in two elaborations
    let chapter = Chapter::read();
    let model = Divider::model(&chapter);
    let print = || {
        let design = Design::load(model.opened_path());
        assert!(design.diagnostics().is_empty(), "the divider is sound: {:?}", design.diagnostics().iter().map(|found| &found.diagnostic.message).collect::<Vec<_>>());
        let ir = Elaborator::new(&design).run("Divider").unwrap_or_else(|failure| panic!("{failure:?}"));
        ir.print_structural()
    };
    let first = print();
    assert!(first.contains("Resistor") && first.contains("Vsource"), "the print names the bodies: {first}");
    assert_eq!(first, print(), "the same text twice");
}

#[test]
fn every_circuit_of_the_comparison_table_has_a_twin_and_a_scenario_test() {
    // compiler~0f9a1fec "Every circuit the path runs has a twin written for ngspice, and a test runs both and
    // compares every printed number.": the six circuits of the table, each a twin with its netlist and the
    // command that runs it, and a test of `tests/scenarios.rs` named after it
    let scenarios = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios.rs")).expect("tests/scenarios.rs");
    for name in ["divider", "isrc_r", "diode_series", "diode_iv", "diode_rev", "ac_rc", "tran_rc"] {
        let twin = Corpus::of_repository().twin(name);
        for file in [twin.model(), twin.netlist(), twin.command()] {
            assert!(file.is_file(), "{name}: {} is missing", file.display());
        }
        assert!(scenarios.contains(&format!("fn {name}() {{")), "{name}: no scenario test compares it with ngspice");
    }
}

#[test]
fn the_reverse_bias_sweep_goes_down_to_minus_20_volts() {
    // compiler~a16be3e7 "Diode under reverse bias to −20 V": the twin sweeps the source from -1 V to -20 V
    let twin = Corpus::of_repository().twin("diode_rev");
    let ran = Circuit::model(&twin).run(Circuit::overrides(&twin)).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.table.rows.len(), 20, "-1 V to -20 V in steps of 1 V");
    assert_eq!(ran.cell(0, "DiodeRev.v1.dc"), &Cell::Real(-1.0));
    assert_eq!(ran.cell(19, "DiodeRev.v1.dc"), &Cell::Real(-20.0));
}

#[test]
fn the_reverse_sweep_worst_difference_is_the_one_the_book_prints() {
    // compiler~dc42a66e "5.0 × 10−20 A": the table gives the worst difference of the reverse-bias sweep
    // from ngspice, measured with the harness; it stays inside the 1 pA floor of the chapter's tolerance
    let twin = Corpus::of_repository().twin("diode_rev");
    let ran = Circuit::model(&twin).run(Circuit::overrides(&twin)).unwrap_or_else(|failure| panic!("{failure}"));
    let Reference::Golden(golden) = Ngspice::from_environment().run("diode_rev") else { return };
    let wanted = golden.real("diode_rev_sweep");
    assert_eq!(wanted.len(), ran.table.rows.len(), "one row per sweep value");
    let worst = wanted.iter().enumerate().map(|(row, (_, current))| (current - ran.real(row, "i(DiodeRev.v1)")).abs()).fold(0.0, f64::max);
    Printed::holds("5.0e-20", worst, "the worst difference of the reverse-bias sweep, in amperes");
    assert!(worst <= ToleranceContract::DC_CURRENT.floor(), "the worst difference {worst:e} A is inside the floor");
}
