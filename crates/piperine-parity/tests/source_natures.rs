//! Natures of every kind run from MSL source on the Rust backend (feature `resolved-nets`, RN-01
//! and RN-08): each model is written in `tests/natures/`, elaborated by the front end, compiled on
//! the transpiled backend and run on the solver. Every expected value is written from the rule it
//! cites (§4 rule 12, §5.2.1, §6 rule 2), with its closed form beside the assertion, never read
//! back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use std::cell::RefCell;
use std::rc::Rc;

use piperine_api::{Cell, Failure, Field, NodeSource, Overrides, Project as HostProject, Record, RecordSink, Session, Streams};
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::project::Project;
use piperine_solver::prelude::{AcSweepAnalysisOptions, AnalogVariable, Context, NodeIdentifier};
use piperine_solver::protocol::AnalysisKind;
use piperine_transpile::Settings;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::{Built, Seen};
use support::{build, transient};

/// A sink that keeps nothing: a sweep reads its table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// One MSL file of `tests/natures/`.
struct Model {
    /// The project read from that file.
    project: Project,
}

impl Model {
    /// The project of `tests/natures/<file>`.
    fn of(file: &str) -> Model {
        let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/natures").join(file);
        Model { project: Project::new(&root) }
    }

    /// `top` elaborated and compiled once.
    fn built(&self, top: &str) -> Built {
        build(&self.project, top)
    }

    /// The node `<top>.<name>` of an elaborated design.
    fn node(ir: &piperine_ir::Ir, top: &str, name: &str) -> NodeIdentifier {
        NodeIdentifier::Anonymous(ir.node_at(&format!("{top}.{name}")).expect("a node").id().index())
    }

    /// `v(<name>)` of the bench `top` at its operating point.
    fn operating_point(&self, top: &str, name: &str) -> f64 {
        let mut running = self.project.compile(top).unwrap_or_else(|r| panic!("{r}"));
        let node = Model::node(&running.ir, top, name);
        running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves").get_node(&node).expect("a node voltage")
    }

    /// Every branch current of the bench `top` at its operating point: the variable and its value.
    fn branch_currents(&self, top: &str) -> Vec<(String, f64)> {
        let mut running = self.project.compile(top).unwrap_or_else(|r| panic!("{r}"));
        let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
        solved.values().iter().filter(|(variable, _)| matches!(variable.as_ref(), AnalogVariable::Branch(_))).map(|(variable, value)| (format!("{variable:?}"), *value)).collect()
    }

    /// `|v(<name>)|` of the bench `top` at each frequency of a sweep of `steps` points.
    fn ac_magnitudes(&self, top: &str, name: &str, start: f64, stop: f64, steps: usize) -> Vec<(f64, f64)> {
        let mut running = self.project.compile(top).unwrap_or_else(|r| panic!("{r}"));
        let node = Model::node(&running.ir, top, name);
        let mut ac = running.circuit.ac(Context::default()).expect("ac");
        let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: start, stop_frequency: stop, steps, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
        sweep.iter().map(|step| (step.frequency, step.get_node(&node).expect("a node voltage").norm())).collect()
    }

    /// The time and `v(<name>)` of the bench `top` at each accepted step of a transient to `stop` with
    /// the step cap `step`.
    fn transient_voltages(&self, top: &str, name: &str, stop: f64, step: f64) -> Vec<(f64, f64)> {
        let built = self.built(top);
        let node = Model::node(&built.ir, top, name);
        let (_, result) = built.run_with(stop, step);
        result.iter().map(|s| (s.time(), s.get_node(&node).expect("a node voltage"))).collect()
    }

    /// An `op` session of `top` printing `v(<name>)`, as the host opens it on the Rust backend.
    fn host_session(file: &str, top: &str, name: &str) -> Session {
        let overrides = Overrides { top: Some(top.into()), analysis: Some("op".into()), print: vec![name.into()], ..Overrides::default() };
        Model::open_host(file, overrides)
    }

    /// A session of `file` on the Rust backend with `overrides`.
    fn open_host(file: &str, overrides: Overrides) -> Session {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/natures").join(file);
        let project = HostProject::file(&path).unwrap_or_else(|f| panic!("{f}"));
        let spec = project.request(None, &Overrides { backend: Some("rust".into()), ..overrides }).unwrap_or_else(|f| panic!("{f}"));
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        Session::open(&project, &spec, &settings).unwrap_or_else(|f| panic!("{f}"))
    }

    /// The `dc` sweep of the param `sweep` of `top` from `from` to `to` in steps of `step`, as the
    /// host runs it: the swept value and `v(<name>)` at each point.
    fn dc_sweep(file: &str, top: &str, sweep: &str, (from, to, step): (f64, f64, f64), name: &str) -> Vec<(f64, f64)> {
        let overrides = Overrides { top: Some(top.into()), analysis: Some("dc".into()), sweep: Some(sweep.into()), from: Some(from), to: Some(to), step: Some(step), print: vec![name.into()], ..Overrides::default() };
        let table = Model::open_host(file, overrides).run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
        let real = |cell: &Cell| match cell {
            Cell::Real(value) => *value,
            other => panic!("a real, found {other:?}"),
        };
        table.rows.iter().map(|row| (real(&row[0]), real(&row[1]))).collect()
    }

    /// The integer fields of every record of the stream `name` a run wrote, in order: the
    /// operating point's first, then the transient's.
    fn records(built: &Built, seen: &Seen, name: &str) -> Vec<Vec<(String, i128)>> {
        let streams = Streams::of(built.ir.clone());
        seen.all()
            .iter()
            .filter_map(|o| streams.decode(o))
            .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
            .filter(|r| r.stream == name)
            .map(|r| r.fields.into_iter().map(|(field, value)| match value {
                Field::Int(value) => (field, value),
                other => panic!("no integer field `{field}`: {other:?}"),
            }).collect())
            .collect()
    }

    /// The value of a `Byte` as the net holds it: the value byte.
    fn known_byte(value: u8) -> Vec<u8> {
        vec![value]
    }
}

#[test]
fn a_counter_over_a_project_byte_nature_commits_and_records() {
    // §4 rule 12: a carrier of `Byte` is built like `Bit<u8>`. The clock has a period of 10 ns and
    // rises at 5 ns, 15 ns, ...; the counter commits `count + 1` at each rise, so after the `k`-th
    // rise the net holds `k`, known.
    let built = Model::of("byte_counter.msl").built("ConnectedBench");
    let seen = built.run(60e-9, 1e-9);
    let committed: Vec<Vec<u8>> = transient(&seen, "ConnectedBench.count").into_iter().map(|(_, bytes)| bytes).collect();
    let expected: Vec<Vec<u8>> = (1..=6).map(Model::known_byte).collect();
    assert_eq!(committed, expected, "one commit per rising edge");
    // The reader records the count it sees at each rising edge: the value before that edge's
    // commit, so `0, 1, 2, ...` (§5.3, a commit lands after the reactions of its instant).
    let streams = Streams::of(built.ir.clone());
    let seen_counts: Vec<i128> = seen
        .all()
        .iter()
        .filter(|o| o.analysis == AnalysisKind::Transient)
        .filter_map(|o| streams.decode(o))
        .map(|r| r.unwrap_or_else(|f| panic!("{f}")))
        .filter(|r| r.stream == "seen")
        .map(|r| match r.fields.iter().find(|(name, _)| name == "count") {
            Some((_, Field::Int(value))) => *value,
            other => panic!("no integer field `count`: {other:?}"),
        })
        .collect();
    assert_eq!(seen_counts, (0..6).collect::<Vec<i128>>());
    // §8.2.4: the definition `twice = count + count` is recomputed at every change of `count`, so
    // the net holds `2k` after the `k`-th rise.
    let doubled: Vec<Vec<u8>> = transient(&seen, "ConnectedBench.twice").into_iter().map(|(_, bytes)| bytes).collect();
    assert_eq!(doubled, (1..=6).map(|k| Model::known_byte(2 * k)).collect::<Vec<_>>(), "a definition follows its operand");
    // §5.3: `changed()` fires at each change of `count`: at `(0, 0)` for its first commit `0`, whose
    // `prev()` is the initial value, `0` (§5.3.5, D8: the first commit seeds `PREVIOUS` with its own
    // bytes; the earlier model read it as all unknown, 255), then at every rise for `k`, with `prev()` the
    // value before that change, `k - 1`.
    let changes = Model::records(&built, &seen, "changes");
    let expected: Vec<Vec<(String, i128)>> = (0..=6i128).map(|count| vec![("count".to_string(), count), ("before".to_string(), (count - 1).max(0))]).collect();
    assert_eq!(changes, expected, "one `changed()` per change, with `prev()` the value before it");
}

#[test]
fn a_project_nature_carrier_connects_to_a_port_of_the_same_nature() {
    // §6.1.3, §8.1.10: the counter's `Byte` signal reaches the reader's `Byte` port and a bench
    // with no reader runs as well; the net is one net either way, named by its path. The reader
    // sees the net: it records the count it reads at each rising edge, `0, 1, ..., 5`.
    let alone = Model::of("byte_counter.msl").built("CounterBench").run(60e-9, 1e-9);
    let connected_bench = Model::of("byte_counter.msl").built("ConnectedBench");
    let connected = connected_bench.run(60e-9, 1e-9);
    assert_eq!(transient(&alone, "CounterBench.count"), transient(&connected, "ConnectedBench.count"));
    let read: Vec<Vec<(String, i128)>> = (0..6).map(|count| vec![("count".to_string(), count)]).collect();
    assert_eq!(Model::records(&connected_bench, &connected, "seen"), read, "the reader reads the connected net");
}

#[test]
fn a_signal_flow_node_with_one_driver_solves_and_loads_nothing() {
    // §6 rule 2: the one potential driver forces `v(a)` = 1.5 V and a signal-flow node has no flow, so
    // the readers (two gains, whose own nodes `b` and `c` are driven by them) change nothing: `v(a)`
    // is the driver's 1.5 V with one reader or with two, and `v(b)` = k * 1.5 V. Nothing draws flow
    // from the driver either: every branch current is 0 A.
    let chain = Model::of("gain_chain.msl");
    for (top, node, expected) in [("GainBench", "a", 1.5), ("LoadlessBench", "a", 1.5), ("LoadlessBench", "b", 2.0 * 1.5), ("LoadlessBench", "c", 7.0 * 1.5)] {
        ToleranceContract::DC_VOLTAGE.assert(&Point::new("signal flow", &format!("v({node})"), Coordinate::OperatingPoint), expected, chain.operating_point(top, node));
    }
    // No flow: a node that one driver forces and two readers probe carries no current, so the
    // branch of the driver (and of each reader's own driven node) holds 0 A.
    let currents = chain.branch_currents("LoadlessBench");
    assert_eq!(currents.len(), 3, "one branch per potential contribution: {currents:?}");
    for (branch, current) in currents {
        ToleranceContract::DC_CURRENT.assert(&Point::new("signal flow", &format!("i({branch})"), Coordinate::OperatingPoint), 0.0, current);
    }
}

#[test]
fn a_gain_chain_over_vsignal_gives_the_product_of_the_gains() {
    // §6 rule 2, §11 rule 1: `v(c)` = k2 * (k1 * v(a)) = 3 * 2 * 1.5 V = 9 V, in the operating point
    // and at every step of a transient (a constant source); the unit AC stimulus on `a` reaches `c`
    // multiplied by the same product, |v(c)| = 3 * 2 * 1 = 6 at every frequency.
    let chain = Model::of("gain_chain.msl");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("gain chain", "v(c)", Coordinate::OperatingPoint), 9.0, chain.operating_point("GainBench", "c"));
    for (frequency, magnitude) in chain.ac_magnitudes("GainBench", "c", 1e3, 1e6, 4) {
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("gain chain ac", "|v(c)|", Coordinate::Frequency(frequency)), 6.0, magnitude);
    }
    // RN-08 AC 2: sweeping the source `level` from 0 V to 2 V in `dc` gives `v(c) = 6 * level`.
    let swept = Model::dc_sweep("gain_chain.msl", "GainBench", "source.level", (0.0, 2.0, 0.5), "c");
    assert_eq!(swept.len(), 5, "0, 0.5, 1, 1.5 and 2 V");
    for (level, voltage) in swept {
        ToleranceContract::DC_VOLTAGE.assert(&Point::new("gain chain dc", "v(c)", Coordinate::Sweep(level)), 6.0 * level, voltage);
    }
    let steps = chain.transient_voltages("GainBench", "c", 1e-6, 1e-7);
    assert!(steps.len() > 1, "a transient runs more than one step");
    for (time, voltage) in steps {
        ToleranceContract::DC_VOLTAGE.assert(&Point::new("gain chain tran", "v(c)", Coordinate::Time(time)), 9.0, voltage);
    }
}

#[test]
fn vsignal_connected_to_an_electrical_port_is_conservative() {
    // §4 rule 13, §6 Resolution of rule 2: a resistor on the `VSignal` node `a` makes it conservative,
    // so a 1 V source over two 1 kOhm resistors puts `v(m)` = 1 V * 1k / (1k + 1k) = 0.5 V.
    let mixed = Model::of("mixed_node.msl");
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("mixed node", "v(m)", Coordinate::OperatingPoint), 0.5, mixed.operating_point("MixedBench", "m"));
}

#[test]
fn an_undriven_top_signal_flow_port_is_driven_by_the_host_or_refused_at_the_run() {
    // RN-08 AC 1, params-and-io §23.1 rule 1: a `VSignal` port of the top that no module of the
    // design drives elaborates, since the host drives the top's node ports; a run with no host
    // source on it is `E0810` naming the port, and with one the chain gives 6 times the source.
    let mut session = Model::host_session("gain_chain.msl", "OpenChain", "c");
    match session.run(Rc::new(RefCell::new(Ignore))) {
        Err(Failure::Host { code, message }) => {
            assert_eq!(code, "E0810");
            assert!(message.contains("`a`"), "{message}");
        }
        other => panic!("E0810, found {other:?}"),
    }
    session.source("a", NodeSource::dc(1.5)).expect("a source on `a`");
    let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|f| panic!("{f}"));
    let Cell::Real(volts) = table.rows[0][1] else { panic!("a real, found {:?}", table.rows[0][1]) };
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("open chain", "v(c)", Coordinate::OperatingPoint), 9.0, volts);
}
