//! Scope: the ngspice twins through the user's entry point (feature `root-e2e`, RE-05). Each twin of
//! `tests/e2e/twins/` runs the command its `<name>.command` holds, parsed with the binary's own
//! parser, through `piperine::` (`Project`, `Session`) on every backend that runs it, and the table
//! it gives is held to what ngspice computes for the twin's `.cir` under the `ToleranceContract`.
//! `piperine-parity` stays the harness (corpus, ngspice, contract); the `parity_*` targets there keep
//! their own comparisons. One test per twin, named after it; ngspice runs once per twin.

use std::cell::{Cell as Counter, RefCell};
use std::rc::Rc;

use piperine::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_cli::{Cli, Invocation};
use piperine_parity::backend::Backend;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::corpus::{Corpus, Twin};
use piperine_parity::golden::Golden;
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_transpile::Settings;

/// The twins only the Rust backend runs: their source reads `now()`, which the interpreter refuses
/// (the same list `piperine-cli/tests/twins.rs` holds the binary to).
const COMPILED_ONLY: [&str; 4] = ["pss_rc", "tran_jump", "tran_corner", "tran_floor"];

/// A sink that keeps nothing: a scenario reads the table.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// One twin and the run its command asks for.
struct Scenario {
    twin: Twin,
    overrides: Overrides,
}

/// What one run of a scenario gave: the table, and the directory the run built into, removed when
/// this value is dropped.
struct Ran {
    table: Table,
    _build: tempfile::TempDir,
}

impl Scenario {
    /// The scenario of the twin `name`: its `<name>.command` parsed the way the binary parses it.
    /// Panics naming the twin when the command is not a `run` of the twin's own model.
    fn of(name: &str) -> Scenario {
        let twin = Corpus::of_repository().twin(name);
        let line = std::iter::once("piperine".to_string()).chain(twin.command_arguments());
        match Cli::parse(line) {
            Ok(Invocation::Run { target, overrides, .. }) => {
                let model = twin.model().file_name().map(|file| file.to_string_lossy().into_owned());
                assert_eq!(target, model, "{name}: the command runs the twin's own model");
                Scenario { twin, overrides }
            }
            Ok(other) => panic!("{name}: the command is a `{}`, not a `run`", other.name()),
            Err(error) => panic!("{name}: the command does not parse: {error}"),
        }
    }

    /// The scenario's name, the twin's.
    fn name(&self) -> &str {
        self.twin.name()
    }

    /// The backends that run this scenario among the selected ones.
    fn backends(&self) -> Vec<Backend> {
        let compiled_only = COMPILED_ONLY.contains(&self.name());
        Backend::selected().into_iter().filter(|backend| !compiled_only || *backend == Backend::Rust).collect()
    }

    /// The run on `backend`, in a build directory of its own: the table, or the failure.
    fn try_run(&self, backend: Backend) -> Result<Ran, Failure> {
        let (mut session, build) = self.open(backend)?;
        let table = session.run(Rc::new(RefCell::new(Ignore)))?;
        Ok(Ran { table, _build: build })
    }

    /// The run on `backend`; a failure panics naming the scenario, the backend and the failure.
    fn run(&self, backend: Backend) -> Ran {
        self.try_run(backend).unwrap_or_else(|failure| panic!("{}: the run fails on {}: {failure}", self.name(), backend.name()))
    }

    /// A session of the command on `backend`, building into a new directory (returned: it must
    /// outlive the session).
    fn open(&self, backend: Backend) -> Result<(Session, tempfile::TempDir), Failure> {
        let build = tempfile::Builder::new().prefix(&format!("piperine-scenario-{}-", self.name())).tempdir().unwrap_or_else(|error| panic!("{}: no build directory: {error}", self.name()));
        let project = Project::file(&self.twin.model())?;
        let overrides = Overrides { backend: Some(backend.name().to_string()), ..self.overrides.clone() };
        let spec = project.request(None, &overrides)?;
        let session = Session::open(&project, &spec, &Settings { rustc: None, cache: build.path().to_path_buf() })?;
        Ok((session, build))
    }
}

impl Ran {
    /// The value (second cell) of the one row of a `signal | value` table whose first cell ends
    /// with `ending`.
    fn signal_ending(&self, ending: &str) -> f64 {
        let found: Vec<&Vec<Cell>> = self.table.rows.iter().filter(|row| matches!(&row[0], Cell::Text(name) if name.ends_with(ending))).collect();
        match found.as_slice() {
            [row] => match &row[1] {
                Cell::Real(value) => *value,
                other => panic!("`{ending}`: a real number, found {other:?}"),
            },
            _ => panic!("no single row ends with `{ending}` in {:?}", self.table),
        }
    }
}

/// The comparisons of one scenario against one ngspice run, on one backend: it builds each
/// [`Point`], asserts under a contract and keeps what the run did for its report.
struct Check<'a> {
    scenario: &'a str,
    golden: &'a Golden,
    backend: Backend,
    compared: Counter<usize>,
    worst: Counter<f64>,
}

impl<'a> Check<'a> {
    fn new(scenario: &'a str, golden: &'a Golden, backend: Backend) -> Check<'a> {
        Check { scenario, golden, backend, compared: Counter::new(0), worst: Counter::new(0.0) }
    }

    /// Holds `actual` to ngspice's `wanted` of `quantity` at `at` under `contract`.
    fn assert(&self, contract: &ToleranceContract, quantity: &str, at: Coordinate, wanted: f64, actual: f64) {
        let point = Point::new(self.scenario, quantity, at).with_ngspice(self.golden.version()).with_backend(self.backend.name());
        contract.assert(&point, wanted, actual);
        self.compared.set(self.compared.get() + 1);
        self.worst.set(self.worst.get().max((wanted - actual).abs()));
    }

    /// Prints what the comparison did, and fails when it compared nothing: a scenario that
    /// compares no point proves nothing.
    fn report(&self) {
        assert!(self.compared.get() > 0, "{}: nothing was compared on {}", self.scenario, self.backend.name());
        println!("{} on {}: {} points through piperine::, largest deviation from ngspice {:e}", self.scenario, self.backend.name(), self.compared.get(), self.worst.get());
    }
}

/// One analysis family's comparison of a table with ngspice's output.
enum Comparison {
    /// The node voltages of an operating point: the table's `signal | value` rows against
    /// ngspice's printed `v(<node>)`, under `DC_VOLTAGE`.
    OperatingPoint { nodes: &'static [&'static str] },
}

impl Comparison {
    /// Holds `ran`, the table of `scenario` on `check`'s backend, to `check`'s ngspice run.
    fn hold(&self, ran: &Ran, check: &Check) {
        match self {
            Comparison::OperatingPoint { nodes } => Self::hold_operating_point(nodes, ran, check),
        }
    }

    /// Each node's row against ngspice's printed voltage.
    fn hold_operating_point(nodes: &[&str], ran: &Ran, check: &Check) {
        for node in nodes {
            let wanted = check.golden.printed(&format!("v({node})"));
            check.assert(&ToleranceContract::DC_VOLTAGE, &format!("v({node})"), Coordinate::OperatingPoint, wanted, ran.signal_ending(&format!(".{node}")));
        }
    }
}

impl Scenario {
    /// Runs the scenario on every backend that runs it and holds each table to `comparison`
    /// against ngspice. With the comparison disabled the table is still produced, and the harness
    /// prints its skip line.
    fn hold(&self, comparison: &Comparison) {
        let reference = Ngspice::from_environment().run(self.name());
        for backend in self.backends() {
            let ran = self.run(backend);
            if let Reference::Golden(golden) = &reference {
                let check = Check::new(self.name(), golden, backend);
                comparison.hold(&ran, &check);
                check.report();
            }
        }
    }
}

#[test]
fn divider() {
    Scenario::of("divider").hold(&Comparison::OperatingPoint { nodes: &["vin", "mid"] });
}

#[test]
fn isrc_r() {
    Scenario::of("isrc_r").hold(&Comparison::OperatingPoint { nodes: &["a"] });
}

#[test]
fn diode_series() {
    Scenario::of("diode_series").hold(&Comparison::OperatingPoint { nodes: &["a", "b"] });
}
