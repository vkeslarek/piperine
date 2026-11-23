//! Scope: the ngspice twins through the user's entry point (feature `root-e2e`, RE-05). Each twin of
//! `tests/e2e/twins/` runs the command its `<name>.command` holds, parsed with the binary's own
//! parser, through `piperine::` (`Project`, `Session`) on every backend that runs it, and the table
//! it gives is held to what ngspice computes for the twin's `.cir` under the `ToleranceContract`.
//! `piperine-parity` stays the harness (corpus, ngspice, contract); the `parity_*` targets there keep
//! their own comparisons. One test per twin, named after it; ngspice runs once per twin.

use std::cell::{Cell as Counter, RefCell};
use std::path::Path;
use std::rc::Rc;

use piperine::{Cell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_cli::{Cli, Invocation};
use piperine_parity::backend::Backend;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::corpus::{Corpus, Twin};
use piperine_parity::golden::{Complex, Golden};
use piperine_parity::ngspice::{Lookup, Ngspice, Reference};
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::{Context, NodeIdentifier, TransientAnalysisOptions};
use piperine_transpile::Settings;

/// The twins only the Rust backend runs: their source reads `now()`, which the interpreter refuses
/// (the same list `piperine-cli/tests/twins.rs` holds the binary to); `uic_rc` and `diode_rs` read `analysis(..)`.
const COMPILED_ONLY: [&str; 8] = ["pss_rc", "tran_jump", "tran_corner", "tran_floor", "uic_rc", "diode_rs", "delay_tline", "noise_rc"];

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
    build: tempfile::TempDir,
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
        Ok(Ran { table, build })
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
    /// The directory the run built into.
    fn build_directory(&self) -> &Path {
        self.build.path()
    }

    /// The real number of `row`, `column`.
    fn real(&self, row: usize, column: usize) -> f64 {
        match &self.table.rows[row][column] {
            Cell::Real(value) => *value,
            other => panic!("row {row}, column {column}: a real number, found {other:?}"),
        }
    }

    /// The row of a table whose first column is a grid whose abscissa is within `1e-9 * step` of
    /// `at`, if the table has one.
    fn row_at(&self, at: f64, step: f64) -> Option<usize> {
        (0..self.table.rows.len()).find(|row| (self.real(*row, 0) - at).abs() <= 1e-9 * step)
    }

    /// The `(real, imaginary)` parts of the rows of a `kind | real | imag` table whose kind is
    /// `kind`, sorted by real then imaginary part.
    fn roots_of(&self, kind: &str) -> Vec<Complex> {
        let mut roots: Vec<Complex> = (0..self.table.rows.len())
            .filter(|row| self.table.rows[*row][0] == Cell::Text(kind.to_string()))
            .map(|row| Complex { re: self.real(row, 1), im: self.real(row, 2) })
            .collect();
        Roots::sort(&mut roots);
        roots
    }

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

/// The comparisons of one scenario on one backend against the ngspice run, when there is one: it
/// builds each [`Point`], asserts under a contract and keeps what the run did for its report. With
/// the comparison disabled it holds no golden and compares nothing.
struct Check<'a> {
    scenario: &'a str,
    golden: Option<&'a Golden>,
    backend: Backend,
    compared: Counter<usize>,
    worst: Counter<f64>,
}

impl<'a> Check<'a> {
    /// The check of `scenario` on `backend` against `reference`: a golden to compare with, or none when ngspice is skipped.
    fn new(scenario: &'a str, reference: &'a Reference, backend: Backend) -> Check<'a> {
        let golden = match reference {
            Reference::Golden(golden) => Some(golden),
            Reference::Skipped => None,
        };
        Check { scenario, golden, backend, compared: Counter::new(0), worst: Counter::new(0.0) }
    }

    /// Holds `actual` to ngspice's `wanted` of `quantity` at `at` under `contract`.
    fn assert(&self, contract: &ToleranceContract, quantity: &str, at: Coordinate, wanted: f64, actual: f64) {
        let Some(golden) = self.golden else { return };
        let point = Point::new(self.scenario, quantity, at).with_ngspice(golden.version()).with_backend(self.backend.name());
        contract.assert(&point, wanted, actual);
        self.compared.set(self.compared.get() + 1);
        self.worst.set(self.worst.get().max((wanted - actual).abs()));
    }

    /// Prints what the comparison did, and fails when ngspice ran and nothing was compared: a
    /// scenario that compares no point proves nothing.
    fn report(&self) {
        if self.golden.is_none() {
            return;
        }
        assert!(self.compared.get() > 0, "{}: nothing was compared on {}", self.scenario, self.backend.name());
        println!("{} on {}: {} points through piperine::, largest deviation from ngspice {:e}", self.scenario, self.backend.name(), self.compared.get(), self.worst.get());
    }
}

/// The roots ngspice printed: `pole(k)` and `zero(k)` lines, listed under `<kind>(k) : ...`.
struct Roots;

impl Roots {
    /// The roots ngspice printed as `<kind>(1)`, `<kind>(2)`, ..., sorted by real then imaginary part.
    fn printed(golden: &Golden, kind: &str) -> Vec<Complex> {
        let listed = golden.stdout().lines().filter(|line| line.trim_start().starts_with(&format!("{kind}(")) && line.contains(" : ")).count();
        let mut roots: Vec<Complex> = (1..=listed).map(|k| golden.printed_complex(&format!("{kind}({k})"))).collect();
        Roots::sort(&mut roots);
        roots
    }

    /// `roots` ordered by real part, then imaginary part, so two lists compare root for root.
    fn sort(roots: &mut [Complex]) {
        roots.sort_by(|a, b| a.re.total_cmp(&b.re).then(a.im.total_cmp(&b.im)));
    }
}

/// One analysis family's comparison of a run with ngspice's output.
enum Comparison {
    /// The node voltages of an operating point: the table's `signal | value` rows against
    /// ngspice's printed `v(<node>)`, under `DC_VOLTAGE`.
    OperatingPoint { nodes: &'static [&'static str] },
    /// A DC sweep of one printed current: each row of ngspice's `export` (`rows` of them, on the
    /// sweep's own grid) against the table's second column, under `DC_CURRENT`.
    DcSweep { export: &'static str, rows: usize },
    /// An AC sweep of one printed node: each row of ngspice's `export` against the table's
    /// magnitude and phase columns (phase in degrees there, radians in ngspice), under
    /// `AC_MAGNITUDE` and `AC_PHASE`.
    AcResponse { export: &'static str, rows: usize },
    /// A transient of one printed node on the print grid: each row of ngspice's `export` against
    /// the table row at its time, under `TRAN_VOLTAGE`.
    Transient { export: &'static str, rows: usize, node: &'static str },
    /// [`Transient`](Self::Transient) run on the compiled circuit of the session with the smallest
    /// step `dt_min`, which the command line has no flag for (ngspice's `delmin`): the waveform
    /// is read at the export's times, as `parity_tran.rs` does.
    FlooredTransient { export: &'static str, rows: usize, node: &'static str, dt_min: f64 },
    /// A transfer function: each `(ngspice's printed name, the table's row)` pair under `TF_RATIO`.
    Transfer { rows: &'static [(&'static str, &'static str)] },
    /// Poles and zeros: the table's `kind | real | imag` rows against ngspice's printed
    /// `pole(k)` and `zero(k)`, root for root after sorting, under `PZ_ROOT`.
    PoleZero,
    /// DC sensitivities of one printed node: each `(ngspice's printed name, the end of the table's
    /// param row)` pair under `SENSITIVITY`.
    Sensitivity { derivatives: &'static [(&'static str, &'static str)] },
    /// A periodic steady state of one printed node: the orbit's rows against the last period of
    /// ngspice's long transient `export` (`rows` rows, `periods` periods) at the same phase, and
    /// the end of the high half against the closed form of the twin's RC of time constant `tau`.
    PeriodicOrbit { export: &'static str, rows: usize, node: &'static str, tau: f64, periods: f64 },
    /// A noise run: the table's `onoise` column (and `inoise` when `input` names ngspice's
    /// `(density export, total export)` of the input-referred noise) against ngspice's `export` of
    /// `rows` rows, and the run's band totals against ngspice's `total` exports, under `NOISE`.
    Noise { export: &'static str, total: &'static str, input: Option<(&'static str, &'static str)>, rows: usize },
    /// A transient no step can settle: the run fails `E0508` at ngspice's 1 us, where ngspice
    /// stops with "Timestep too small".
    StepFloor,
}

impl Comparison {
    /// Runs `scenario` on `check`'s backend and holds what it gives to `check`'s ngspice run.
    fn hold(&self, scenario: &Scenario, check: &Check) {
        match self {
            Comparison::OperatingPoint { nodes } => Self::hold_operating_point(nodes, &scenario.run(check.backend), check),
            Comparison::DcSweep { export, rows } => Self::hold_dc_sweep(export, *rows, scenario, &scenario.run(check.backend), check),
            Comparison::AcResponse { export, rows } => Self::hold_ac_response(export, *rows, &scenario.run(check.backend), check),
            Comparison::Transient { export, rows, node } => Self::hold_transient(export, *rows, node, scenario, &scenario.run(check.backend), check),
            Comparison::FlooredTransient { export, rows, node, dt_min } => Self::hold_floored_transient(export, *rows, node, *dt_min, scenario, check),
            Comparison::Transfer { rows } => Self::hold_transfer(rows, &scenario.run(check.backend), check),
            Comparison::PoleZero => Self::hold_pole_zero(&scenario.run(check.backend), check),
            Comparison::Sensitivity { derivatives } => Self::hold_sensitivity(derivatives, &scenario.run(check.backend), check),
            Comparison::PeriodicOrbit { export, rows, node, tau, periods } => Self::hold_periodic_orbit(export, *rows, node, *tau, *periods, scenario, &scenario.run(check.backend), check),
            Comparison::StepFloor => Self::hold_step_floor(scenario, check),
            Comparison::Noise { export, total, input, rows } => Self::hold_noise(export, total, *input, *rows, scenario, check),
        }
    }

    /// The run's grid is ngspice's; each density of the table and each band total of the report agree
    /// with ngspice's under `NOISE`.
    fn hold_noise(export: &str, total: &str, input: Option<(&str, &str)>, rows: usize, scenario: &Scenario, check: &Check) {
        let (mut session, build) = scenario.open(check.backend).unwrap_or_else(|failure| panic!("{}: the session does not open on {}: {failure}", scenario.name(), check.backend.name()));
        let table = session.run(Rc::new(RefCell::new(Ignore))).unwrap_or_else(|failure| panic!("{}: the run fails on {}: {failure}", scenario.name(), check.backend.name()));
        let ran = Ran { table, build };
        let Some(golden) = check.golden else { return };
        let report = session.noise_report().expect("a noise run keeps its report");
        let wanted = golden.real(export);
        golden.expect_rows(export, &wanted, rows);
        golden.same_grid(export, &report.frequencies);
        let band = Coordinate::Band(wanted[0].0, wanted[rows - 1].0);
        for (row, (frequency, density)) in wanted.iter().enumerate() {
            check.assert(&ToleranceContract::NOISE, "onoise", Coordinate::Frequency(*frequency), *density, ran.real(row, 1));
        }
        check.assert(&ToleranceContract::NOISE, "onoise_total", band, golden.real(total)[0].1, report.output_total);
        if let Some((input_export, input_total)) = input {
            let referred = golden.real(input_export);
            golden.expect_rows(input_export, &referred, rows);
            for (row, (frequency, density)) in referred.iter().enumerate() {
                check.assert(&ToleranceContract::NOISE, "inoise", Coordinate::Frequency(*frequency), *density, ran.real(row, 2));
            }
            check.assert(&ToleranceContract::NOISE, "inoise_total", band, golden.real(input_total)[0].1, report.input_total.expect("an input run has its total"));
        }
    }

    /// Each node's row against ngspice's printed voltage.
    fn hold_operating_point(nodes: &[&str], ran: &Ran, check: &Check) {
        let Some(golden) = check.golden else { return };
        for node in nodes {
            let wanted = golden.printed(&format!("v({node})"));
            check.assert(&ToleranceContract::DC_VOLTAGE, &format!("v({node})"), Coordinate::OperatingPoint, wanted, ran.signal_ending(&format!(".{node}")));
        }
    }

    /// The sweep's grid is ngspice's, and each current is ngspice's under `DC_CURRENT`.
    fn hold_dc_sweep(export: &str, rows: usize, scenario: &Scenario, ran: &Ran, check: &Check) {
        let Some(golden) = check.golden else { return };
        let wanted = golden.real(export);
        golden.expect_rows(export, &wanted, rows);
        assert_eq!(ran.table.rows.len(), wanted.len(), "{}: one table row per sweep value of ngspice's {export}", check.scenario);
        // a grid row matches ngspice's sweep value to a billionth of the step: ngspice accumulates the
        // step, so its value at zero is 3e-16, not 0 (the grid is aligned here, no contract bound)
        let step = scenario.overrides.step.expect("a sweep command names its step").abs();
        for (row, (sweep, current)) in wanted.iter().enumerate() {
            assert!((ran.real(row, 0) - sweep).abs() <= 1e-9 * step, "{}: row {row} is at {:e}, ngspice's at {sweep:e}", check.scenario, ran.real(row, 0));
            check.assert(&ToleranceContract::DC_CURRENT, "i(v1)", Coordinate::Sweep(*sweep), *current, ran.real(row, 1));
        }
    }

    /// The frequency grid is ngspice's; magnitude and phase agree where the phase carries
    /// information (the magnitude clears its floor).
    fn hold_ac_response(export: &str, rows: usize, ran: &Ran, check: &Check) {
        let Some(golden) = check.golden else { return };
        let wanted = golden.complex(export);
        golden.expect_rows(export, &wanted, rows);
        let grid: Vec<f64> = (0..ran.table.rows.len()).map(|row| ran.real(row, 0)).collect();
        golden.same_grid(export, &grid);
        for (row, (frequency, value)) in wanted.iter().enumerate() {
            check.assert(&ToleranceContract::AC_MAGNITUDE, "|v(out)|", Coordinate::Frequency(*frequency), value.magnitude(), ran.real(row, 1));
            if ToleranceContract::phase_applies(value.magnitude()) {
                check.assert(&ToleranceContract::AC_PHASE, "arg v(out)", Coordinate::Frequency(*frequency), value.phase(), ran.real(row, 2).to_radians());
            }
        }
    }

    /// Every export row has a table row at its time, and the voltages agree.
    fn hold_transient(export: &str, rows: usize, node: &str, scenario: &Scenario, ran: &Ran, check: &Check) {
        let Some(golden) = check.golden else { return };
        let wanted = golden.real(export);
        golden.expect_rows(export, &wanted, rows);
        let step = scenario.overrides.print_step.expect("a transient command names its print step");
        for (time, voltage) in wanted {
            let row = ran.row_at(time, step).unwrap_or_else(|| panic!("{}: no table row at {time:e} s on {}", check.scenario, check.backend.name()));
            check.assert(&ToleranceContract::TRAN_VOLTAGE, &format!("v({node})"), Coordinate::Time(time), voltage, ran.real(row, 1));
        }
    }

    /// The compiled circuit of the scenario's session, run with the floor `dt_min`, read at the
    /// export's times.
    fn hold_floored_transient(export: &str, rows: usize, node: &str, dt_min: f64, scenario: &Scenario, check: &Check) {
        let (mut session, _build) = scenario.open(check.backend).unwrap_or_else(|failure| panic!("{}: the session does not open on {}: {failure}", scenario.name(), check.backend.name()));
        let path = format!("{}.{node}", session.top());
        let id = NodeIdentifier::Anonymous(session.ir().node_at(&path).unwrap_or_else(|| panic!("{}: no node `{path}`", scenario.name())).id().index());
        let context: Context = session.context().clone();
        let stop = scenario.overrides.stop.expect("a transient command names its stop time");
        let mut options = TransientAnalysisOptions::new(stop, scenario.overrides.max_step.expect("a transient command names its step cap"));
        options.dt_max = options.dt;
        options.dt_min = dt_min;
        let circuit = session.circuit().expect("a session holds its circuit until it runs");
        let result = circuit.transient(options, context).and_then(|mut solver| solver.solve()).unwrap_or_else(|error| panic!("{}: the transient does not solve on {}: {error}", scenario.name(), check.backend.name()));
        let steps: Vec<_> = result.iter().collect();
        let waveform = Waveform::new(steps.iter().map(|step| step.time()).collect(), steps.iter().map(|step| step.get_node(&id).unwrap_or_else(|| panic!("{}: no v({node})", scenario.name()))).collect());
        let Some(golden) = check.golden else { return };
        let wanted = golden.real(export);
        golden.expect_rows(export, &wanted, rows);
        for (time, voltage) in wanted {
            let actual = waveform.at(time).unwrap_or_else(|| panic!("{}: {time:e} s lies outside the result on {}", scenario.name(), check.backend.name()));
            check.assert(&ToleranceContract::TRAN_VOLTAGE, &format!("v({node})"), Coordinate::Time(time), voltage, actual);
        }
    }

    /// Each row of the table against what ngspice's `.tf` printed.
    fn hold_transfer(rows: &[(&str, &str)], ran: &Ran, check: &Check) {
        let Some(golden) = check.golden else { return };
        for (printed, row) in rows {
            check.assert(&ToleranceContract::TF_RATIO, row, Coordinate::OperatingPoint, golden.printed(printed), ran.signal_ending(row));
        }
    }

    /// The twin has one finite pole and one finite zero, so a root the run loses is a miss; each
    /// root is ngspice's, real and imaginary part, under `PZ_ROOT`.
    fn hold_pole_zero(ran: &Ran, check: &Check) {
        let Some(golden) = check.golden else { return };
        for kind in ["pole", "zero"] {
            let actual = ran.roots_of(kind);
            let wanted = Roots::printed(golden, kind);
            assert_eq!(wanted.len(), actual.len(), "{}: ngspice lists {} {kind}s, the table gives {} on {}: {actual:?}", check.scenario, wanted.len(), actual.len(), check.backend.name());
            for (index, (wanted, actual)) in wanted.iter().zip(&actual).enumerate() {
                for (part, wanted, actual) in [("re", wanted.re, actual.re), ("im", wanted.im, actual.im)] {
                    check.assert(&ToleranceContract::PZ_ROOT, &format!("{kind}({}) {part}", index + 1), Coordinate::OperatingPoint, wanted, actual);
                }
            }
        }
    }

    /// Each derivative of the table against what ngspice's `.sens` printed.
    fn hold_sensitivity(derivatives: &[(&str, &str)], ran: &Ran, check: &Check) {
        let Some(golden) = check.golden else { return };
        for (printed, param) in derivatives {
            check.assert(&ToleranceContract::SENSITIVITY, &format!("d v(mid) / d {param}"), Coordinate::OperatingPoint, golden.printed(printed), ran.signal_ending(param));
        }
    }

    /// The closed form at the end of the high half, then every row of the orbit whose phase is
    /// also a row of ngspice's grid (the command's print step is 0.1 us and ngspice's 0.04 us, so
    /// every fifth of the orbit's rows meets one).
    fn hold_periodic_orbit(export: &str, rows: usize, node: &str, tau: f64, periods: f64, scenario: &Scenario, ran: &Ran, check: &Check) {
        let period = scenario.overrides.period.expect("a pss command names its period");
        let stabilize = scenario.overrides.tstab.expect("a pss command names its stabilizing time");
        let step = scenario.overrides.print_step.expect("a pss command names its print step");
        let half = stabilize + period / 2.0;
        let closed = 1.0 / (1.0 + (-period / (2.0 * tau)).exp());
        let at_half = ran.row_at(half, step).unwrap_or_else(|| panic!("{}: the orbit holds no row at the end of the high half", scenario.name()));
        let point = Point::new(scenario.name(), "v(vout), end of the high half", Coordinate::Time(half)).with_backend(check.backend.name());
        ToleranceContract::TRAN_VOLTAGE.assert(&point, closed, ran.real(at_half, 1));
        let Some(golden) = check.golden else { return };
        let wanted = golden.real(export);
        golden.expect_rows(export, &wanted, rows);
        let grid = wanted[1].0 - wanted[0].0;
        // ngspice's last period is [(N - 1) T, N T], at the phase of the orbit's [tstab, tstab + T]
        let last_from = (periods - 1.0) * period;
        for row in 0..ran.table.rows.len() {
            let phase = ran.real(row, 0) - stabilize;
            let Some((time, voltage)) = wanted.iter().find(|(time, _)| (time - (last_from + phase)).abs() <= 1e-9 * grid) else { continue };
            check.assert(&ToleranceContract::TRAN_VOLTAGE, &format!("v({node})"), Coordinate::Time(*time), *voltage, ran.real(row, 1));
        }
        assert!(check.compared.get() >= 21, "{}: the orbit meets ngspice's grid at 21 phases, {} were compared", scenario.name(), check.compared.get());
    }

    /// The run fails `E0508` at the step after 1 us, and ngspice said it stopped at its floor.
    fn hold_step_floor(scenario: &Scenario, check: &Check) {
        let failure = scenario.try_run(check.backend).err().unwrap_or_else(|| panic!("{}: the step no Newton iteration settles must fail on {}", scenario.name(), check.backend.name()));
        let text = failure.to_string();
        assert!(text.contains("E0508"), "{}: {text}", check.backend.name());
        let time = text.split("at t = ").nth(1).and_then(|rest| rest.split(' ').next()).and_then(|time| time.parse::<f64>().ok()).unwrap_or_else(|| panic!("{}: the time of the failing step is in `{text}`", scenario.name()));
        assert!((1e-6..1e-6 + 1e-8).contains(&time), "{}: the failing step follows ngspice's 1 us: {text}", check.backend.name());
        let Some(golden) = check.golden else { return };
        assert!(golden.said("Timestep too small"), "ngspice stops at the floor:\n{}", golden.captured());
        check.compared.set(check.compared.get() + 1);
    }
}

impl Scenario {
    /// Runs the scenario on every backend that runs it and holds each run to `comparison`
    /// against ngspice.
    fn hold(&self, comparison: &Comparison) {
        self.hold_against(comparison, &Ngspice::from_environment().run(self.name()));
    }

    /// [`hold`](Self::hold) against an ngspice run already made. With the comparison disabled
    /// (`Reference::Skipped`) the run is still made and nothing is compared.
    fn hold_against(&self, comparison: &Comparison, reference: &Reference) {
        for backend in self.backends() {
            let check = Check::new(self.name(), reference, backend);
            comparison.hold(self, &check);
            check.report();
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

#[test]
fn diode_rs() {
    Scenario::of("diode_rs").hold(&Comparison::OperatingPoint { nodes: &["vin", "a"] });
}

#[test]
fn diode_iv() {
    Scenario::of("diode_iv").hold(&Comparison::DcSweep { export: "diode_iv_sweep", rows: 37 });
}

#[test]
fn diode_rev() {
    Scenario::of("diode_rev").hold(&Comparison::DcSweep { export: "diode_rev_sweep", rows: 20 });
}

#[test]
fn ac_rc() {
    Scenario::of("ac_rc").hold(&Comparison::AcResponse { export: "ac_rc_ac", rows: 41 });
}

#[test]
fn tran_rc() {
    Scenario::of("tran_rc").hold(&Comparison::Transient { export: "tran_rc_tran", rows: 100, node: "out" });
}

#[test]
fn uic_rc() {
    Scenario::of("uic_rc").hold(&Comparison::Transient { export: "uic_rc_tran", rows: 100, node: "out" });
}

#[test]
fn tran_dq() {
    Scenario::of("tran_dq").hold(&Comparison::Transient { export: "tran_dq_tran", rows: 100, node: "a" });
}

#[test]
fn tran_jump() {
    // a hard 0 to 1 V step at 1.03 us across 1 nF with the default floor: both complete the run
    Scenario::of("tran_jump").hold(&Comparison::Transient { export: "tran_jump_tran", rows: 20, node: "top" });
}

#[test]
fn tran_corner() {
    // a 5 ns tanh corner at 1.03 us with a 1 ns floor (ngspice's `delmin=1n`): the command line has
    // no flag for the floor, so the scenario takes the session's circuit to the library
    Scenario::of("tran_corner").hold(&Comparison::FlooredTransient { export: "tran_corner_tran", rows: 20, node: "top", dt_min: 1e-9 });
}

#[test]
fn tran_floor() {
    // a node no step can settle after 1 us: ngspice stops with "Timestep too small", piperine fails E0508
    Scenario::of("tran_floor").hold(&Comparison::StepFloor);
}

#[test]
fn tf_divider() {
    Scenario::of("tf_divider").hold(&Comparison::Transfer { rows: &[("transfer_function", "gain"), ("output_impedance_at_v(out)", "output_impedance"), ("v1#input_impedance", "input_impedance")] });
}

#[test]
fn pz_lowpass() {
    Scenario::of("pz_lowpass").hold(&Comparison::PoleZero);
}

#[test]
fn sens_divider() {
    Scenario::of("sens_divider").hold(&Comparison::Sensitivity { derivatives: &[("r1", ".r1.r"), ("r2", ".r2.r"), ("v1", ".v1.dc")] });
}

#[test]
fn pss_rc() {
    Scenario::of("pss_rc").hold(&Comparison::PeriodicOrbit { export: "pss_rc_tran", rows: 1001, node: "vout", tau: 1e-6, periods: 10.0 });
}

#[test]
fn temp_res() {
    // 75 C: ngspice's `.options temp=75`, the command's `--temp 75`; the same rows as an operating point
    Scenario::of("temp_res").hold(&Comparison::OperatingPoint { nodes: &["mid"] });
}

#[test]
fn delay_tline() {
    // AO-02: a lossless line matched at both ends is `0.5 * delay(v(src), 1.05 us)`; the 100 kHz sine reads `now()`
    Scenario::of("delay_tline").hold(&Comparison::Transient { export: "delay_tline_tran", rows: 101, node: "out" });
}

#[test]
fn noise_rc() {
    // AO-06: the thermal noise of the resistor of an RC pole; 301 points of `dec 100 1k 1Meg`
    Scenario::of("noise_rc").hold(&Comparison::Noise { export: "noise_rc_psd", total: "noise_rc_total", input: None, rows: 301 });
}

#[test]
fn every_twin_has_a_scenario_test() {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios.rs")).expect("this file is readable");
    let missing: Vec<String> = Corpus::of_repository().twins().iter().map(|twin| twin.name().to_string()).filter(|name| !source.contains(&format!("\nfn {name}() {{"))).collect();
    assert!(missing.is_empty(), "twins with no scenario test in tests/scenarios.rs: {missing:?}");
}

#[test]
fn a_disabled_ngspice_still_runs_the_scenario() {
    // the harness's skip line is printed and the scenario still gives its table on every backend
    let reference = Ngspice::new(Lookup::Skip).run("divider");
    assert!(matches!(reference, Reference::Skipped), "a disabled ngspice gives no golden");
    assert_eq!(Ngspice::skip_line("divider"), "SKIP divider: ngspice comparison disabled by PIPERINE_NGSPICE=skip");
    let scenario = Scenario::of("divider");
    scenario.hold_against(&Comparison::OperatingPoint { nodes: &["vin", "mid"] }, &reference);
    for backend in scenario.backends() {
        let ran = scenario.run(backend);
        assert_eq!(ran.table.rows.len(), 2, "the divider's table holds vin and mid on {}", backend.name());
        assert_eq!(ran.signal_ending(".mid"), 7.5, "the divider's mid on {} with nothing to compare against", backend.name());
    }
}

#[test]
fn each_scenario_runs_in_a_directory_of_its_own() {
    // two scenarios on the compiled backend: each builds only its own design, in a directory of its own
    let first = Scenario::of("divider").run(Backend::Rust);
    let second = Scenario::of("isrc_r").run(Backend::Rust);
    // the compiled backend keeps `rust/abi-<hash>` (the shared runtime) and one `rust/<hash>` per design
    let designs = |ran: &Ran| -> Vec<String> {
        let found = std::fs::read_dir(ran.build_directory().join("rust")).expect("a compiled run builds into its directory");
        let mut names: Vec<String> = found.map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned()).filter(|name| !name.starts_with("abi-")).collect();
        names.sort();
        names
    };
    assert_ne!(first.build_directory(), second.build_directory(), "two runs share no build directory");
    let (first_designs, second_designs) = (designs(&first), designs(&second));
    assert_eq!(first_designs.len(), 1, "the first directory holds its own design only: {first_designs:?}");
    assert_eq!(second_designs.len(), 1, "the second directory holds its own design only: {second_designs:?}");
    assert_ne!(first_designs, second_designs, "the two scenarios are two designs");
}
