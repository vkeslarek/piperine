//! Scope: chapter 15 of the Book, "The command line" (feature `root-e2e`, RE-02 and RE-03): the RC project
//! the chapter prints file for file, the lowpass and the square-wave RC it saves, and the buck bench, run
//! through `piperine::` with the keys and flags the chapter gives; the values and records a run gives (the
//! `from` trim, the shapes of the tables, the derivatives of `sens`, the Bode exercise), the refusals the
//! mistakes table lists with their codes, and the bounds of the solver. What only the binary can state
//! (the layout of a line, an exit code, a file written, the commands other than `run`) is a `binary` row
//! naming a test of `piperine-cli`. Each test opens with the inventory ids it holds, from
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

use piperine::{Cell, Failure, Overrides};
use piperine_cli::{Cli, Invocation};
use piperine_solver::prelude::TransientAnalysisOptions;
use support::chapter::Chapter;
use support::faulty::Faulty;
use support::model::{Model, Ran};
use support::printed::Printed;
use support::refusal::Refusal;

/// The RC project of the chapter: the manifest, the library and the bench the Book prints.
struct RcProject;

impl RcProject {
    /// The project as the Book prints it (`cli#2`, `cli#3`, `cli#4`), with `extra` runs added to the manifest.
    fn model(chapter: &Chapter, extra: &str) -> Model {
        let manifest = format!("{}\n{extra}", chapter.text("cli#2"));
        Model::package(&manifest).with_source("src/lib.msl", &chapter.text("cli#3")).with_source("benches/step.msl", &chapter.text("cli#4"))
    }

    /// The run `name` of the project as the Book prints it, with `overrides`.
    fn run(chapter: &Chapter, name: &str, overrides: Overrides) -> Result<Ran, Failure> {
        Self::model(chapter, "").of_run(name).run(overrides)
    }

    /// The run `name`, which must succeed.
    fn ran(chapter: &Chapter, name: &str, overrides: Overrides) -> Ran {
        Self::run(chapter, name, overrides).unwrap_or_else(|failure| panic!("the run `{name}` fails: {failure}"))
    }
}

/// The lowpass the chapter asks to save as `lowpass.msl`: a divider with a capacitor.
struct Lowpass;

impl Lowpass {
    /// The model: the block `cli#25`.
    fn model(chapter: &Chapter) -> Model {
        Model::written("lowpass.msl", &format!("{}\n", chapter.text("cli#25")))
    }

    /// The run of the top `Lowpass` for `analysis`.
    fn overrides(analysis: &str) -> Overrides {
        Overrides { analysis: Some(analysis.to_string()), ..Faulty::operating_point_of("Lowpass") }
    }
}

/// The diagnostics of a failed run as `(code, message)`, naming `row` when the run did not fail.
struct Codes;

impl Codes {
    /// The `(code, message)` pairs of the failure of `result`.
    fn of(result: Result<Ran, Failure>, row: &str) -> Vec<(String, String)> {
        Refusal::of(result, row)
    }

    /// The one diagnostic of the failure of `result`; panics listing what the run gave when it is not one.
    fn single(result: Result<Ran, Failure>, row: &str) -> (String, String) {
        let mut given = Self::of(result, row);
        assert_eq!(given.len(), 1, "{row}: one diagnostic, the run gave {given:?}");
        given.remove(0)
    }
}

#[test]
fn the_rc_project_of_the_chapter_runs_its_operating_point_and_its_step_response() {
    // cli#3 and cli#4 (run): the library and the bench `piperine new rc` writes, with `[run.op]` and
    // `[run.step]` of cli#2; cli#13 (fragment of cli#4): the monitor's `digital for`; cli~24bdf414 "one per
    // printed signal": an operating point has one row per printed signal
    let chapter = Chapter::read();
    let op = RcProject::model(&chapter, "").of_run("op").run(Overrides::default()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(op.table.columns, ["signal", "value"]);
    assert_eq!(op.table.rows.len(), 2, "no print: the top's own two signals");
    let printed = RcProject::ran(&chapter, "op", Overrides { print: vec!["vout".to_string()], ..Overrides::default() });
    assert_eq!(printed.table.rows.len(), 1, "one row per printed signal");
    assert_eq!(printed.signal("Step.vout"), 1.0, "the capacitor is open at rest");
    let step = RcProject::ran(&chapter, "step", Overrides::default());
    assert_eq!(step.table.rows.len(), 6, "5 us every microsecond");
    let trips = step.stream("trip");
    assert_eq!(trips.len(), 1, "the monitor's emit! writes one record: {:?}", step.records);
    assert_eq!(trips[0].instance, "Step.mon");
    Printed::holds("6.931071e-7", trips[0].time, "the crossing's instant");
}

#[test]
fn the_records_of_one_instant_come_in_the_order_the_solver_delivers_them() {
    // cli~18bf4b5d "At one instant the records come in the order the solver delivers them, not necessarily
    // the order of the statements: here the check, written second, prints first." (cli#17's listing)
    let chapter = Chapter::read();
    let step = RcProject::ran(&chapter, "step", Overrides::default());
    let streams: Vec<&str> = step.records.iter().map(|record| record.stream.as_str()).collect();
    assert_eq!(streams, ["checks", "trip"], "the `require`, written second, is delivered first: {:?}", step.records);
    assert_eq!(step.records[0].time, step.records[1].time, "at the same instant");
}

#[test]
fn from_trims_the_table_and_leaves_the_solver_and_the_records_alone() {
    // cli~4b2ad568 "The solver still runs from 0; steps before it are computed and not kept."; cli~868f4e5f
    // "from trims the start of that table ... With print_step too, the first row is the first multiple of
    // print_step at or after from" (the memory the sentence also speaks of is not observable from a table);
    // cli~5f073732 "Records are not affected; every one prints."
    let chapter = Chapter::read();
    let whole = RcProject::ran(&chapter, "step", Overrides::default());
    let trimmed = RcProject::ran(&chapter, "step", Overrides { from: Some(2e-6), ..Overrides::default() });
    assert_eq!(trimmed.table.rows.len(), 4, "the rows at 2, 3, 4 and 5 us: the table holds only the grid from `from`");
    Printed::holds("2.000000e-6", trimmed.real(0, "time"), "the first row is the first multiple at or after from");
    assert_eq!(trimmed.real(0, "Step.vout"), whole.real(2, "Step.vout"), "the solver ran from 0: the same value at 2 us");
    assert_eq!(trimmed.stream("trip").len(), 1, "the trip record, written before `from`, is still a record: {:?}", trimmed.records);
}

#[test]
fn a_node_gives_its_voltage_a_digital_signal_its_value_and_an_instance_its_current() {
    // cli~22d81a5c "A node gives its voltage, a digital signal its value, and i(<instance>) the current
    // through an instance such as a source.": `vout` is `Step.vout`, a column is headed by the full path
    let chapter = Chapter::read();
    let step = RcProject::ran(&chapter, "step", Overrides { print: vec!["vout".to_string(), "i(v1)".to_string()], ..Overrides::default() });
    assert_eq!(step.table.columns, ["time", "Step.vout", "i(Step.v1)"], "full paths, in the order printed");
    assert!(step.real(0, "i(Step.v1)") < 0.0, "the source delivers current: {}", step.real(0, "i(Step.v1)"));
    let source = "module Top() { signal d: Bit = false; }\ndigital for Top { on at(1u) { d <- true; } }\n";
    let digital = Model::written("m.msl", source).run(Overrides { print_step: Some(1e-6), print: vec!["d".to_string()], ..Faulty::transient_of_top(2e-6, None) }).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(digital.table.columns, ["time", "Top.d"]);
    assert_eq!(digital.cell(0, "Top.d"), &Cell::Bool(false), "a digital signal gives its value");
    assert_eq!(digital.cell(2, "Top.d"), &Cell::Bool(true));
}

#[test]
fn a_design_with_no_emit_prints_no_record_line() {
    // cli~37f6ca25 "A design with no emit! prints no record line.": the lowpass writes no record
    let chapter = Chapter::read();
    let ran = Lowpass::model(&chapter).run(Overrides { stop: Some(1e-3), ..Lowpass::overrides("transient") }).unwrap_or_else(|failure| panic!("{failure}"));
    assert!(ran.records.is_empty(), "no record: {:?}", ran.records);
}

#[test]
fn a_path_that_names_nothing_is_e0801_before_any_analysis() {
    // cli~fe2da74d "A path that names nothing is E0801."; cli~771ae31d "E0801"; cli~b47893cf "--print vuot,
    // --stream tripp, --param r9.r=1k": the three mistakes of the table, each refused with no table
    let chapter = Chapter::read();
    let print = Codes::single(RcProject::run(&chapter, "step", Overrides { print: vec!["vuot".to_string()], ..Overrides::default() }), "cli~b47893cf");
    assert_eq!(print.0, "E0801");
    assert!(print.1.starts_with("`vuot` names nothing in the design"), "{}", print.1);
    let stream = Codes::single(RcProject::run(&chapter, "step", Overrides { streams: vec!["tripp".to_string()], ..Overrides::default() }), "cli~b47893cf");
    assert_eq!(stream, ("E0801".to_string(), "`tripp` names no record stream of the design: the streams are Step.mon.checks, Step.mon.trip".to_string()));
    let param = Codes::single(RcProject::run(&chapter, "step", Overrides { params: vec![("r9.r".to_string(), 1000.0)], ..Overrides::default() }), "cli~b47893cf");
    assert_eq!(param, ("E0801".to_string(), "`r9.r` names no param of the design: no instance of that path".to_string()));
}

#[test]
fn a_value_the_design_refuses_is_e0802_and_no_analysis_runs() {
    // cli~9685ce4d "A value the design refuses, such as one that fails the param's where, is E0802: no
    // analysis runs."; cli~771ae41d "E0802"; cli~a6b80de0 "The design refuses the value: the param's where
    // r > 0.0 fails.": `--param r1.r=-5`
    let chapter = Chapter::read();
    let (code, message) = Codes::single(RcProject::run(&chapter, "step", Overrides { params: vec![("r1.r".to_string(), -5.0)], ..Overrides::default() }), "cli~771ae41d");
    assert_eq!(code, "E0802");
    assert!(message.starts_with("`Step.r1.r` refuses the value -5") && message.contains("check Step.r1.check:r is false"), "the where of r fails: {message}");
}

#[test]
fn each_mistake_of_the_run_table_is_p0010() {
    // cli~79f0c8b1 "P0010" and its rows of the mistakes table: an unknown key, a value of the wrong type
    // (cli~5d988a44 `stop = "5u"`; cli~381401c6 "Numbers are TOML numbers: write 10e-6, not 10u."), an
    // analysis that is not one, none, a transient with no stop, a key its analysis does not read, an AC grid
    // that is both or neither, a bench that does not exist
    let chapter = Chapter::read();
    let bench = "bench = \"benches/step.msl\"\n";
    let table: [(&str, String, &str); 9] = [
        ("stopp = 1e-3", format!("{bench}analysis = \"op\"\nstopp = 1e-3\n"), "unknown key `stopp` in a run: a run takes bench, top, analysis, stop,"),
        ("stop = \"5u\"", format!("{bench}analysis = \"transient\"\nstop = \"5u\"\n"), "`stop` must be a number, found string"),
        ("analysis = \"tran\"", format!("{bench}analysis = \"tran\"\n"), "unknown analysis `tran`: the analyses are `op`, `dc`, `ac`, `transient`, `tf`, `pz`, `sens`, `pss`"),
        ("no analysis", bench.to_string(), "run `bad` has no `analysis`: it is one of `op`, `dc`, `ac`, `transient`, `tf`, `pz`, `sens`, `pss`"),
        ("no stop", format!("{bench}analysis = \"transient\"\n"), "run `bad`: a `transient` run needs `stop`"),
        ("op with stop", format!("{bench}analysis = \"op\"\nstop = 1e-3\n"), "run `bad`: `stop` is not read by a `op` run"),
        ("op with plot", format!("{bench}analysis = \"op\"\nplot = \"a.svg\"\n"), "run `bad`: `plot` is not read by a `op` run: an operating point has nothing to plot"),
        ("per_decade and points", format!("{bench}analysis = \"ac\"\nfrom = 1e3\nto = 1e6\nper_decade = 10\npoints = 5\n"), "run `bad`: an `ac` run gives exactly one of `per_decade` and `points`, not both"),
        ("no grid", format!("{bench}analysis = \"ac\"\nfrom = 1e3\nto = 1e6\n"), "run `bad`: an `ac` run gives exactly one of `per_decade` and `points`"),
    ];
    for (wrote, run, message) in table {
        let (code, given) = Codes::single(RcProject::model(&chapter, &format!("[run.bad]\n{run}")).of_run("bad").run(Overrides::default()), "cli~79f0c8b1");
        assert_eq!((code.as_str(), given.starts_with(message)), ("P0010", true), "`{wrote}`: {given}");
    }
    let missing = format!("[run.bad]\nbench = \"benches/stepp.msl\"\nanalysis = \"op\"\n");
    let (code, given) = Codes::single(RcProject::model(&chapter, &missing).of_run("bad").run(Overrides::default()), "cli~79f0c8b1");
    assert_eq!((code.as_str(), given.as_str()), ("P0010", "run `bad`: the bench `benches/stepp.msl` does not exist"));
    let numbers = RcProject::model(&chapter, &format!("[run.short]\n{bench}analysis = \"transient\"\nstop = 10e-6\nprint_step = 5e-6\n")).of_run("short").run(Overrides::default()).unwrap_or_else(|failure| panic!("`10e-6` is a TOML number: {failure}"));
    Printed::holds("1.000000e-5", numbers.real(numbers.table.rows.len() - 1, "time"), "a run to 10e-6 ends at 10 us");
}

#[test]
fn a_dc_run_of_a_design_that_writes_records_is_p0010() {
    // cli~d26b56cd "A point of a sweep has no time to stamp a record with, so a dc run of a design that
    // writes records is refused with P0010; run op at each value instead"
    let chapter = Chapter::read();
    let sweep = Overrides { analysis: Some("dc".to_string()), sweep: Some("v1.dc".to_string()), from: Some(0.0), to: Some(1.0), step: Some(0.5), ..Overrides::default() };
    let (code, message) = Codes::single(RcProject::run(&chapter, "op", sweep), "cli~d26b56cd");
    assert_eq!(code, "P0010");
    assert!(message.starts_with("a `dc` run of a design with record streams is refused: a point of a DC sweep has no time to stamp its records with"), "{message}");
    assert!(message.ends_with("run `op` at each value, or a `transient`"), "{message}");
}

#[test]
fn a_bench_with_no_top_or_with_several_is_p0011_and_the_candidates_are_listed() {
    // cli~a1e67f93 "If there is no such module, or more than one, the run fails with P0011 and the candidates
    // are listed."; cli~79f0c9b1 "P0011" (a bench file with two modules without ports, and no top)
    let overrides = || Overrides { analysis: Some("op".to_string()), ..Overrides::default() };
    let several = Codes::single(Model::written("m.msl", "module A() { }\nmodule B() { }\n").run(overrides()), "cli~79f0c9b1");
    assert_eq!(several, ("P0011".to_string(), "no top: the crate `m` holds several modules without ports (A, B); name one with `top`".to_string()));
    let none = Codes::single(Model::written("m.msl", "").run(overrides()), "cli~a1e67f93");
    assert_eq!(none, ("P0011".to_string(), "no top: the crate `m` holds no module without ports; name one with `top`".to_string()));
}

#[test]
fn the_bounds_of_the_solver_have_the_defaults_and_the_keys_the_chapter_gives() {
    // cli~253dec67 "The most rounds an operating point may take before it fails, 20 by default."; cli~b9333d20
    // "The most microsteps one instant of the digital engine may take before it fails."; cli~7e41462d "The
    // budget of attempts a transient may make before it fails naming it, 2000000 by default."; cli~bdc8428b
    // "A transient that needs more attempts than max_steps fails naming the budget; raise it and the same run
    // succeeds."
    let chapter = Chapter::read();
    let ring = "module Top() { signal n: Electrical; signal d: Bit = false; }\nanalog for Top { v(n) +> transition(if d.high() { 1.0 } else { 0.0 }); }\ndigital for Top { on above(v(n) - 0.5) { d <- false; } on above(0.5 - v(n)) { d <- true; } }\n";
    let rounds = |maxrounds: Option<u32>| format!("{:#}", Model::written("m.msl", ring).run(Overrides { maxrounds, ..Faulty::operating_point_of("Top") }).err().expect("never settles"));
    assert!(rounds(None).contains("did not settle in 20 rounds"), "20 by default: {}", rounds(None));
    assert!(rounds(Some(7)).contains("did not settle in 7 rounds"), "the key sets it: {}", rounds(Some(7)));
    let spin = "module Top() { signal x: Bit = false; }\ndigital for Top { on x.changed() { x <- !x; } on at(1u) { x <- true; } }\n";
    let microsteps = format!("{:#}", Model::written("m.msl", spin).run(Overrides { maxmicro: Some(50), ..Faulty::transient_of_top(3e-6, None) }).err().expect("never goes quiet"));
    assert!(microsteps.contains("(the bound is 50 microsteps)"), "the key sets the bound: {microsteps}");
    assert_eq!(TransientAnalysisOptions::new(1e-6, 1e-9).max_steps, 2_000_000, "the budget of a transient, by default");
    let starved = format!("{:#}", RcProject::run(&chapter, "step", Overrides { max_steps: Some(3), ..Overrides::default() }).err().expect("the budget is exceeded"));
    assert!(starved.contains("exceeded the 3 step budget"), "the failure names the budget: {starved}");
    RcProject::ran(&chapter, "step", Overrides { max_steps: Some(5000), ..Overrides::default() });
}

#[test]
fn the_options_carry_the_names_ngspice_gives_its_options_in_lowercase() {
    // cli~d3235913 "Tolerances, the temperature and the bounds the solver gives up at are keys of a run too,
    // each with its flag (table above)."; cli~44b8ab83 "They carry the names ngspice gives its .options, in
    // lowercase.": each key as a flag of the binary's own parser, and as a key of the run table
    let line = "piperine run lowpass.msl --analysis op --reltol 1e-4 --abstol 1e-13 --vntol 2e-6 --trtol 5 --temp 75 --resolution 1e-12 --maxrounds 30 --maxmicro 500 --max-steps 4000000 --nodeset vout=5";
    let Ok(Invocation::Run { overrides, .. }) = Cli::parse(line.split_whitespace().map(str::to_string)) else { panic!("the options do not parse as the flags of `run`") };
    assert_eq!((overrides.reltol, overrides.abstol, overrides.vntol, overrides.trtol, overrides.temp), (Some(1e-4), Some(1e-13), Some(2e-6), Some(5.0), Some(75.0)));
    assert_eq!((overrides.resolution, overrides.maxrounds, overrides.maxmicro, overrides.max_steps), (Some(1e-12), Some(30), Some(500), Some(4_000_000)));
    assert_eq!(overrides.nodeset, [("vout".to_string(), 5.0)]);
    let chapter = Chapter::read();
    let keys = "reltol = 1e-4\nabstol = 1e-13\nvntol = 2e-6\ntrtol = 5\ntemp = 75\nresolution = 1e-12\nmaxrounds = 30\nmaxmicro = 500\nmax_steps = 4000000\n";
    RcProject::model(&chapter, &format!("[run.tuned]\nbench = \"benches/step.msl\"\nanalysis = \"op\"\n{keys}")).of_run("tuned").run(Overrides::default()).unwrap_or_else(|failure| panic!("every key is a key of the run table: {failure}"));
}

/// A node with two stable operating points, `+-sqrt(3)` and an unstable one at 0: a source of 0 V through
/// 1 ohm into the node `a`, where a capacitor and the cubic conductance `v^3 - 4v` hang to ground. The seed
/// of a `nodeset` picks the root the operating point converges to.
struct Bistable;

impl Bistable {
    /// The design, as one file.
    const SOURCE: &'static str = "pub module Source(p: Electrical, n: Electrical) { dc: f64 = 0.0; }\nanalog for Source { v(p, n) +> dc + ac_stim(1.0); }\npub module Res(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Res { i(p, n) +> v(p, n) / r; }\npub module Cap(p: Electrical, n: Electrical) { c: f64 = 1u; }\nanalog for Cap { i(p, n) +> ddt(c * v(p, n)); }\npub module Cubic(p: Electrical, n: Electrical) {}\nanalog for Cubic { i(p, n) +> v(p, n) * v(p, n) * v(p, n) - 4.0 * v(p, n); }\nmodule Top() {\n    signal s: Electrical;\n    signal a: Electrical;\n    let v1 = Source(s, gnd);\n    let r1 = Res(s, a) { r: 1.0 };\n    let c1 = Cap(a, gnd);\n    let nl = Cubic(a, gnd);\n}\n";

    /// The run `analysis` with the node `a` seeded at `seed`, tight so a root is held to its digits.
    fn run(analysis: Overrides, seed: f64) -> Ran {
        let tight = Overrides { top: Some("Top".to_string()), reltol: Some(1e-12), vntol: Some(1e-12), print: vec!["a".to_string()], nodeset: vec![("a".to_string(), seed)], ..analysis };
        Model::written("bistable.msl", Self::SOURCE).run(tight).unwrap_or_else(|failure| panic!("{failure}"))
    }

    /// The last value of `a` of a transient seeded at `seed`.
    fn transient_end(seed: f64) -> f64 {
        let ran = Self::run(Overrides { analysis: Some("transient".to_string()), stop: Some(1e-6), print_step: Some(5e-7), ..Overrides::default() }, seed);
        ran.real(ran.table.rows.len() - 1, "Top.a")
    }

    /// The magnitude of `a` over the source at the first frequency of an ac sweep seeded at `seed`.
    fn gain(seed: f64) -> f64 {
        let ran = Self::run(Overrides { analysis: Some("ac".to_string()), from: Some(1e-3), to: Some(1e-2), per_decade: Some(1), ..Overrides::default() }, seed);
        match &ran.table.rows[0][1] {
            Cell::Real(value) => *value,
            other => panic!("a real magnitude, found {other:?}"),
        }
    }
}

#[test]
fn a_nodeset_seeds_the_operating_point_of_ac_and_transient() {
    // cli~271fd4c4 "A nodeset seeds the operating point of every analysis, ac and transient included: where the
    // circuit has more than one equilibrium, the seed picks the one the run starts from": a bistable node
    // starts a transient at +sqrt(3) or -sqrt(3) as the seed says, and an ac sweep takes its gain at the
    // root the seed picks (1/6 at +-sqrt(3), 1/3 at 0)
    let root = 3.0_f64.sqrt();
    assert!((Bistable::transient_end(1.7) - root).abs() < 1e-6, "a seed above zero");
    assert!((Bistable::transient_end(-1.7) + root).abs() < 1e-6, "a seed below zero");
    assert!((Bistable::gain(1.7) - 1.0 / 6.0).abs() < 1e-6, "the gain at the upper root");
    assert!((Bistable::gain(0.1) - 1.0 / 3.0).abs() < 1e-6, "the gain at the root at zero");
    let chapter = Chapter::read();
    let seed = vec![("vout".to_string(), 0.5)];
    let transient = |nodeset: Vec<(String, f64)>| RcProject::ran(&chapter, "op", Overrides { analysis: Some("transient".to_string()), stop: Some(1e-6), nodeset, ..Overrides::default() });
    assert_eq!(transient(seed).table, transient(Vec::new()).table, "a seed on a circuit with one equilibrium is accepted and changes nothing");
}

#[test]
fn the_lowpass_runs_with_a_seed_and_tight_tolerances_and_gives_the_divider() {
    // cli#25 (run): `lowpass.msl`: `--reltol 1e-9 --vntol 1e-12 --nodeset vout=5 --print vout` gives the
    // divider's 7.5 V whatever the guess (cli~d3235913's "the iteration converges on the same point")
    let chapter = Chapter::read();
    let seeded = Overrides { reltol: Some(1e-9), vntol: Some(1e-12), nodeset: vec![("vout".to_string(), 5.0)], print: vec!["vout".to_string()], ..Lowpass::overrides("op") };
    let ran = Lowpass::model(&chapter).run(seeded).unwrap_or_else(|failure| panic!("{failure}"));
    assert!((ran.signal("Lowpass.vout") - 7.5).abs() < 1e-9, "10 V * 3k / (1k + 3k): {}", ran.signal("Lowpass.vout"));
}

#[test]
fn tf_names_its_source_by_instance_or_in_full_and_refuses_what_is_no_voltage_source() {
    // cli~fc253bca "The source is named by its instance, v1 or in full, Lowpass.v1; a source that is not a
    // voltage source is a P0010 error."; the chapter: "The divider has gain 3/4, and the source sees 1 kΩ in
    // series with 3 kΩ"
    let chapter = Chapter::read();
    let transfer = |input: &str| Lowpass::model(&chapter).run(Overrides { input: Some(input.to_string()), output: Some("vout".to_string()), ..Lowpass::overrides("tf") });
    let by_instance = transfer("v1").unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(by_instance.table.columns, ["quantity", "value"]);
    assert_eq!(by_instance.table.rows[0], [Cell::Text("gain".to_string()), Cell::Real(0.75)], "3/4");
    assert_eq!(by_instance.table.rows[1], [Cell::Text("input_impedance".to_string()), Cell::Real(4000.0)], "1 kOhm in series with 3 kOhm");
    assert_eq!(transfer("Lowpass.v1").unwrap_or_else(|failure| panic!("{failure}")).table, by_instance.table, "v1 and Lowpass.v1 are the same source");
    let (code, message) = Codes::single(transfer("r1"), "cli~fc253bca");
    assert_eq!(code, "P0010");
    assert!(message.starts_with("`input` must be a voltage source"), "{message}");
}

#[test]
fn sens_gives_the_derivative_of_each_printed_signal_with_respect_to_each_param() {
    // cli~1c685364 "sens gives the derivative of each printed signal with respect to each param named by wrt,
    // at the operating point:"; cli~662c11ca "Raising r1 by one ohm lowers the output by 1.875 mV, and
    // raising r2 by one ohm raises it by 0.625 mV: the output is 10·R2/(R1+R2)"
    let chapter = Chapter::read();
    let overrides = Overrides { wrt: vec!["r1.r".to_string(), "r2.r".to_string()], print: vec!["vout".to_string()], ..Lowpass::overrides("sens") };
    let ran = Lowpass::model(&chapter).run(overrides).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.table.columns, ["param", "d(Lowpass.vout)"], "one row per wrt param, then d(<path>) per signal");
    assert_eq!(ran.table.rows.len(), 2);
    assert!((ran.real(0, "d(Lowpass.vout)") + 1.875e-3).abs() < 5e-7, "d/dR1 = -10 R2 / (R1 + R2)^2: {}", ran.real(0, "d(Lowpass.vout)"));
    assert!((ran.real(1, "d(Lowpass.vout)") - 0.625e-3).abs() < 5e-7, "d/dR2 = 10 R1 / (R1 + R2)^2: {}", ran.real(1, "d(Lowpass.vout)"));
}

#[test]
fn the_square_wave_rc_closes_its_orbit_over_one_period() {
    // cli#34 (run): `pulse.msl`, a square wave over an RC: `pss` with `--period 4e-6 --tstab 8e-6 --max-step
    // 1e-9 --print-step 1e-6`: the chapter, "The first and last rows are equal: the circuit is back where the
    // period began."
    let chapter = Chapter::read();
    let overrides = Overrides { period: Some(4e-6), tstab: Some(8e-6), max_step: Some(1e-9), print_step: Some(1e-6), print: vec!["vout".to_string()], ..Faulty::operating_point_of("Steady") };
    let ran = Model::written("pulse.msl", &format!("{}\n", chapter.text("cli#34"))).run(Overrides { analysis: Some("pss".to_string()), ..overrides }).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.table.columns, ["time", "Steady.vout"]);
    let last = ran.table.rows.len() - 1;
    assert_eq!(last, 4, "one row per microsecond across the 4 us period, both ends included");
    assert!((ran.real(0, "Steady.vout") - ran.real(last, "Steady.vout")).abs() < 1e-3, "the orbit closes: {} against {}", ran.real(0, "Steady.vout"), ran.real(last, "Steady.vout"));
}

#[test]
fn the_bode_exercise_reads_0_707_and_minus_45_degrees_at_159_kilohertz() {
    // cli~76428b9c "In the project of chapter 1, add a run [run.bode] that sweeps the filter from 1 kHz to
    // 10 MHz with 10 points per decade and plots it. (The source needs a small-signal stimulus ...)";
    // cli~e1afdf04 "The table needs analysis = "ac", from = 1e3, to = 1e7, per_decade = 10, print = ["vout"]
    // and plot = "bode.svg".) At which frequency should mag(Step.vout) read 0.707 and phase(Step.vout) read
    // −45? (At 1 / (2π · r · c) = 159 kHz.)"
    let chapter = Chapter::read();
    let bode = "[run.bode]\nbench = \"benches/step.msl\"\nanalysis = \"ac\"\nfrom = 1e3\nto = 1e7\nper_decade = 10\nprint = [\"vout\"]\nplot = \"bode.svg\"\n";
    let library = chapter.text("cli#3");
    let stimulus = library.replace("v(p, n) +> dc;", "v(p, n) +> dc + ac_stim(1.0, 0.0);");
    assert_ne!(stimulus, library, "the Book's source has the contribution the exercise changes");
    let model = Model::package(&format!("{}\n{bode}", chapter.text("cli#2"))).with_source("src/lib.msl", &stimulus).with_source("benches/step.msl", &chapter.text("cli#4"));
    let ran = model.of_run("bode").run(Overrides::default()).unwrap_or_else(|failure| panic!("{failure}"));
    assert_eq!(ran.table.columns, ["freq", "mag(Step.vout)", "phase(Step.vout)"]);
    assert_eq!(ran.table.rows.len(), 41, "4 decades at 10 points, both ends");
    let corner = 1.0 / (2.0 * std::f64::consts::PI * 1e3 * 1e-9);
    assert!((corner - 159.0e3).abs() < 0.2e3, "1 / (2 pi r c) = 159 kHz: {corner}");
    let nearest = (0..ran.table.rows.len()).min_by(|a, b| (ran.real(*a, "freq") - corner).abs().total_cmp(&(ran.real(*b, "freq") - corner).abs())).expect("a row");
    assert!((ran.real(nearest, "mag(Step.vout)") - 0.707).abs() < 2e-3, "the magnitude at {} Hz: {}", ran.real(nearest, "freq"), ran.real(nearest, "mag(Step.vout)"));
    assert!((ran.real(nearest, "phase(Step.vout)") + 45.0).abs() < 0.5, "the phase at {} Hz: {}", ran.real(nearest, "freq"), ran.real(nearest, "phase(Step.vout)"));
}

#[test]
fn the_buck_bench_of_the_chapter_is_the_example() {
    // cli#44 (text): the chapter's listing is the buck's `benches/startup.msl`, which the package of
    // examples/buck runs as `startup`; its short run is `book_buck` (on demand, MD-77)
    let chapter = Chapter::read();
    let buck = Model::example("buck");
    assert_eq!(chapter.text("cli#44").trim_end(), buck.text_of("benches/startup.msl").trim_end(), "cli#44 is benches/startup.msl as it is");
}
