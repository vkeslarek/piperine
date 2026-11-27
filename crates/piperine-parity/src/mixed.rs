//! A hand-built mixed design (placed under a `TopDesign`), compiled once, and run on the solver with a
//! sink that keeps what the solver observed: the value changes of each net with their instants,
//! and nothing the printed source says.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use piperine_ir::Ir;
use piperine_solver::prelude::{Context, Solver, TransientAnalysisOptions, TransientAnalysisResult};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};
use piperine_transpile::{Compiled, Settings, Transpiler};

/// What the sink kept.
#[derive(Clone, Default)]
pub struct Seen(Rc<RefCell<Vec<Observation>>>);

/// The sink the solver reports to: it appends every observation to what a `Seen` holds.
struct SeenSink(Seen);

impl Sink for SeenSink {
    fn observe(&mut self, o: &Observation) {
        (self.0).0.borrow_mut().push(o.clone());
    }
}

impl Seen {
    /// The value changes of the net `Top.<name>`: instant in seconds and bytes, in order.
    pub fn changes(&self, name: &str) -> Vec<(f64, Vec<u8>)> {
        let path = format!("Top.{name}");
        self.0
            .borrow()
            .iter()
            .filter(|o| o.path == path)
            .filter_map(|o| match &o.kind {
                ObservationKind::Change { bytes } => Some((o.time, bytes.clone())),
                _ => None,
            })
            .collect()
    }

    /// The value changes of the net `Top.<name>` during the transient (the operating point's
    /// initial values left out): instant in solver ticks (one femtosecond) and bytes, in order.
    pub fn transient(&self, name: &str) -> Vec<(i64, Vec<u8>)> {
        let path = format!("Top.{name}");
        self.0
            .borrow()
            .iter()
            .filter(|o| o.path == path && o.analysis == AnalysisKind::Transient)
            .filter_map(|o| match &o.kind {
                ObservationKind::Change { bytes } => Some(((o.time * 1e15).round() as i64, bytes.clone())),
                _ => None,
            })
            .collect()
    }

    /// Every change of the net, the operating point's initial value first: instant in ticks and
    /// bytes. The value at an instant is the last entry at or before it.
    pub fn series(&self, name: &str) -> Vec<(i64, Vec<u8>)> {
        self.series_of(&format!("Top.{name}"))
    }

    /// The same for a private pin of an instance, by `<instance>.<pin>`.
    pub fn series_of(&self, path: &str) -> Vec<(i64, Vec<u8>)> {
        self.0
            .borrow()
            .iter()
            .filter(|o| o.path == path)
            .filter_map(|o| match &o.kind {
                ObservationKind::Change { bytes } => Some(((o.time * 1e15).round() as i64, bytes.clone())),
                _ => None,
            })
            .collect()
    }

    /// The instants, in ticks, at which the instance at `path` made a commit during the
    /// transient.
    pub fn commits(&self, path: &str) -> Vec<i64> {
        self.0
            .borrow()
            .iter()
            .filter(|o| o.path == path && o.analysis == AnalysisKind::Transient)
            .filter_map(|o| match &o.kind {
                ObservationKind::Commit { made, .. } => Some(*made),
                _ => None,
            })
            .collect()
    }

    /// Every observation, in the order the sink saw them.
    pub fn all(&self) -> Vec<Observation> {
        self.0.borrow().clone()
    }
}

/// A design compiled once.
pub struct BuiltDesign {
    /// The design that was compiled.
    pub ir: Arc<Ir>,
    /// The compiled library and its circuit.
    pub compiled: Compiled,
    _cache: tempfile::TempDir,
}

impl BuiltDesign {
    /// Compiles `ir` into a cache of its own; a warning in the generated source fails the build.
    pub fn new(ir: Ir) -> BuiltDesign {
        let ir = Arc::new(ir);
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let compiled = Transpiler::new(ir.clone())
            .unwrap_or_else(|e| panic!("{e}"))
            .compile(&settings)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(compiled.report().warnings, "", "the generated source compiles with no warning");
        BuiltDesign { ir, compiled, _cache: cache }
    }

    /// Runs a transient to `stop` seconds, at most `step` between points, and returns what the
    /// solver observed.
    pub fn run(&self, stop: f64, step: f64) -> Seen {
        self.run_with(stop, step).0
    }

    /// The same, with the analog result of the transient.
    pub fn run_with(&self, stop: f64, step: f64) -> (Seen, TransientAnalysisResult) {
        self.run_in(Context::default(), stop, step)
    }

    /// The same, in the session `context` (its temperature, its `gmin`, ...).
    pub fn run_in(&self, context: Context, stop: f64, step: f64) -> (Seen, TransientAnalysisResult) {
        let circuit = self.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
        let seen = Seen::default();
        let mut options = TransientAnalysisOptions::new(stop, step);
        options.dt_max = step;
        let mut solver = Solver::new(circuit)
            .with_context(context)
            .with_tran_opts(options)
            .with_sink(Box::new(SeenSink(seen.clone())))
            .build();
        let result = solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}"));
        (seen, result)
    }
}
