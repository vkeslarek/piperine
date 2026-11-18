//! `Session`: one run request taken from source to a circuit bound on a backend (CLI-04). The
//! top is chosen (the `top` key, else the one module without ports of the root crate), the
//! design is type-checked and elaborated, and the backend builds it once: the interpreter, or
//! the transpiler, which may call `rustc`. Every failure before an analysis happens here, each
//! naming its stage, and the run's `print` entries are resolved. No analysis ever compiles.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use piperine_digital_solver::net::NetId;
use piperine_interp::Interpreter;
use piperine_ir::{BranchKind, ChildValue, Expr, ExprId, Ir};
use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{Design, DesignDiagnostic, Reflect};
use piperine_lang::types::Types;
use piperine_plan::Netlist;
use piperine_solver::abi::{InitialValue, Value};
use piperine_solver::prelude::{
    AcSweepAnalysisOptions, AnalogVariable, BranchIdentifier, CircuitInstance, Context, Keep, Net, NodeIdentifier, ParamValue, PoleZeroOptions, PoleZeroResult, SensAnalysisOptions, Solver, TransferFunctionAnalysisOptions, PssAnalysisOptions,
    TransientAnalysisOptions,
};
use piperine_transpile::{Compiled, Settings, Transpiler};

use crate::backend::Backend;
use crate::failure::Failure;
use crate::host::HostAction;
use crate::metadata::Metadata;
use crate::plot::Plot;
use crate::probe::Probe;
use crate::record::{Delivery, RecordSink, Streams};
use crate::recorder::{Recorder, Tap};
use crate::table::Table;
use crate::project::Project;
use crate::request::{Analysis, Grid, Options, RunSpec};

/// How the design of a session was built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Built {
    /// `rustc` built it now; `source` is the generated Rust.
    Compiled { source: PathBuf },
    /// The cache held it; `source` is the generated Rust.
    Cached { source: PathBuf },
    /// The interpreter runs it; nothing was compiled.
    Interpreted,
}

/// A design compiled onto a backend, ready to run one request. The fields drop in order: the
/// circuit, which holds the loaded library's modules, before the library.
pub struct Session {
    pub(crate) circuit: Option<CircuitInstance>,
    ir: Arc<Ir>,
    compiled: Option<Compiled>,
    top: String,
    backend: Backend,
    spec: RunSpec,
    probes: Vec<Probe>,
    /// The param a `dc` run sweeps, resolved against the design.
    swept: Option<ParamPath>,
    /// The record streams of the design, filtered by the run's `streams`.
    pub(crate) streams: Streams,
    /// The table the run gave, kept when the request has `plot`: the plot draws the rows the
    /// run prints (CLI-22 AC 7 as amended, `long-run` LR-17).
    plotted: Option<Table>,
    /// The context every analysis of the run takes, built from the run's options (HII-02).
    context: Context,
    /// The `nodeset` entries, each node by what the solver knows it by, with the entry as written.
    nodeset: Vec<(NodeIdentifier, f64, String)>,
    /// The source and the node of a `tf` or `pz` run, resolved against the design.
    transfer: Option<Transfer>,
    /// The params a `sens` run differentiates by, resolved against the design, in the order given.
    wrt: Vec<ParamPath>,
    /// The host's actions due after the start of the transient, in the order given.
    pub(crate) actions: Vec<HostAction>,
}

/// The input and the output of a `tf` or `pz` run, by what the solver knows them by.
#[derive(Debug, Clone)]
struct Transfer {
    /// The first potential branch of the voltage source named by `input`.
    input: BranchIdentifier,
    output: NodeIdentifier,
}

/// A param of an instance of the design, by the instance's full path.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamPath {
    pub instance: String,
    pub param: String,
}

impl ParamPath {
    /// `entry`, `<instance>.<param>` split at the last dot, the instance in full or relative to
    /// `top`; `E0801` when it names no param of an instance of `ir`.
    pub fn resolve(ir: &Ir, top: &str, entry: &str) -> Result<ParamPath, Failure> {
        let nothing = |why: &str| Failure::Host { code: "E0801", message: format!("`{entry}` names no param of the design: {why}") };
        let (path, param) = entry.rsplit_once('.').ok_or_else(|| nothing("a param path is <instance>.<param>"))?;
        let instance = [path.to_string(), format!("{top}.{path}")].into_iter().find_map(|p| ir.instance_at(&p)).ok_or_else(|| nothing("no instance of that path"))?;
        if !instance.body().params().iter().any(|p| p.name == param) {
            return Err(nothing(&format!("the instance `{}` has no param `{param}`", instance.path())));
        }
        Ok(ParamPath { instance: instance.path().to_string(), param: param.to_string() })
    }

    /// The full path, `<instance>.<param>`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.instance, self.param)
    }

    /// `E0803` when this param is structural: a structural param is fixed at elaboration, so a
    /// `set` of it during a run (at a time after the start) is refused (§23.1 rule 7).
    pub fn refuse_timed(&self, ir: &Ir) -> Result<(), Failure> {
        let structural = ir.instance_at(&self.instance).and_then(|i| i.body().params().iter().find(|p| p.name == self.param).map(|p| p.structural));
        match structural {
            Some(true) => Err(Failure::Host { code: "E0803", message: format!("`{}` is a structural param: it cannot change during a run, re-elaborate instead", self.label()) }),
            _ => Ok(()),
        }
    }

    /// The params of the children of this param's instance whose placement derives them from it
    /// (`ChildValue::Expr` reading this param), each as `<child>.<param>`; empty when none does.
    pub fn derived(&self, ir: &Ir) -> Vec<String> {
        let Some(instance) = ir.instance_at(&self.instance) else { return Vec::new() };
        let body = instance.body();
        let Some(index) = body.params().iter().position(|p| p.name == self.param) else { return Vec::new() };
        let mut derived = Vec::new();
        for (at, child) in body.children().iter().enumerate() {
            for (param, value) in body.child_params(at) {
                let ChildValue::Expr(expr) = value else { continue };
                if ParamPath::reads(ir, *expr, index as u32) {
                    let name = ir.body(child.body).params().get(*param as usize).map_or("?", |p| p.name.as_str()).to_string();
                    derived.push(format!("{}.{name}", child.name));
                }
            }
        }
        derived
    }

    /// Whether the expression `expr` reads param `index` of its body, through the operators a
    /// placement's value is built from.
    fn reads(ir: &Ir, expr: ExprId, index: u32) -> bool {
        let reads = |e: &ExprId| ParamPath::reads(ir, *e, index);
        match ir.expr(expr) {
            Expr::Param(p) => *p == index,
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => reads(expr),
            Expr::Binary { lhs, rhs, .. } => reads(lhs) || reads(rhs),
            Expr::If { cond, then, otherwise } => reads(cond) || reads(then) || reads(otherwise),
            Expr::KnownOr { value, default } => reads(value) || reads(default),
            Expr::Math { args, .. } | Expr::Call { args, .. } => args.iter().any(reads),
            Expr::Method { receiver, args, .. } => reads(receiver) || args.iter().any(reads),
            _ => false,
        }
    }
}

impl Session {
    /// `spec` of `project`, from source to a bound circuit on the backend `spec.backend` names
    /// (`rust` when it names none); `settings` locate `rustc` and the cache.
    pub fn open(project: &Project, spec: &RunSpec, settings: &Settings) -> Result<Session, Failure> {
        Session::open_announced(project, spec, settings, &mut |_| {})
    }

    /// [`Session::open`], calling `compiling` with the top's name once, before `rustc` runs, when
    /// the Rust backend builds a design the cache does not hold (CLI-07 AC 8): a host says so
    /// while the build runs, not after.
    pub fn open_announced(project: &Project, spec: &RunSpec, settings: &Settings, compiling: &mut dyn FnMut(&str)) -> Result<Session, Failure> {
        let backend = Backend::choose(None, None, spec.backend.as_deref())?;
        let (ir, top) = Session::elaborate(project, spec)?;
        let probes = Session::probes_of(&ir, spec)?;
        let mut streams = Streams::of(ir.clone());
        if matches!(spec.analysis, Analysis::Dc { .. }) && !streams.paths().is_empty() {
            // CLI-09 AC 10: a point of a sweep has no time, and a record line is stamped
            // `@<time>`, so the records of a sweep would be lost; refused before any compile
            let message = format!(
                "a `dc` run of a design with record streams is refused: a point of a DC sweep has no time to stamp its records with, so they would be lost (the streams are {}); run `op` at each value, or a `transient`",
                streams.paths().join(", ")
            );
            return Err(Failure::Host { code: "P0010", message });
        }
        streams.keep(&spec.streams)?;
        let swept = match &spec.analysis {
            Analysis::Dc { sweep, .. } => Some(ParamPath::resolve(&ir, ir.top().path(), sweep)?),
            _ => None,
        };
        // every param path resolves before anything compiles: a typo costs no `rustc`
        let params = spec.params.iter().map(|(entry, value)| Ok((ParamPath::resolve(&ir, ir.top().path(), entry)?, entry, *value))).collect::<Result<Vec<_>, Failure>>()?;
        let context = Session::context_of(&spec.options);
        let nodeset = Session::nodeset_of(&ir, spec)?;
        let (transfer, wrt) = Session::small_signal_of(&ir, spec, &probes)?;
        let netlist = Session::netlist_of(&ir, &spec.options);
        let (mut circuit, compiled) = match backend {
            Backend::Interp => {
                let interpreter = Interpreter::new(ir.clone()).map_err(|rejected| {
                    let entries: Vec<String> = rejected.entries.iter().map(|e| format!("  {}: {}", e.item, e.what)).collect();
                    Failure::Backend(format!("the interpreter cannot run this design (use --backend rust):\n{}", entries.join("\n")))
                })?;
                let circuit = match &netlist {
                    Some(netlist) => netlist.circuit(&interpreter),
                    None => interpreter.circuit(),
                };
                (circuit.map_err(|e| Failure::Backend(format!("the interpreter cannot bind the design: {e}")))?, None)
            }
            Backend::Rust => {
                let transpiler = Transpiler::new(ir.clone()).map_err(|e| Failure::Backend(e.to_string()))?;
                let compiled = transpiler.compile_announced(settings, &mut || compiling(&top)).map_err(|e| Failure::Backend(e.to_string()))?;
                let circuit = match &netlist {
                    Some(netlist) => netlist.circuit(&compiled),
                    None => compiled.circuit(),
                };
                let circuit = circuit.map_err(|e| Failure::Backend(format!("the compiled design cannot bind: {e}")))?;
                (circuit, Some(compiled))
            }
        };
        // the table's params, then the flags', in the order given: the design's own check may
        // refuse a value, and then no analysis runs
        for (path, entry, value) in &params {
            let refused = |error| Failure::Host { code: "E0802", message: format!("`{entry}` refuses the value {value}: {error}") };
            circuit.set(&path.instance, &path.param, ParamValue::Real(*value)).map_err(refused)?;
        }
        Ok(Session { circuit: Some(circuit), ir, compiled, top, backend, spec: spec.clone(), probes, swept, streams, plotted: None, context, nodeset, transfer, wrt, actions: Vec::new() })
    }

    /// The context a run's options give: the solver's own default with each given option in its
    /// place; `temp` is Celsius in the run and Kelvin in the solver (HII-02 AC 2 and 3).
    fn context_of(options: &Options) -> Context {
        let mut context = Context::default();
        let tolerances = &mut context.tolerances;
        for (given, slot) in [
            (options.reltol, &mut tolerances.reltol),
            (options.abstol, &mut tolerances.abstol),
            (options.vntol, &mut tolerances.vntol),
            (options.trtol, &mut tolerances.trtol),
        ] {
            if let Some(value) = given {
                *slot = value;
            }
        }
        if let Some(celsius) = options.temp {
            tolerances.temperature = celsius + 273.15;
        }
        if let Some(rounds) = options.maxrounds {
            context.max_rounds = rounds;
        }
        context
    }

    /// The netlist that binds the design when the run gives a `resolution` or a `maxmicro`; `None`
    /// leaves the backend's own `circuit()`, which is what a run with neither has always used.
    fn netlist_of<'a>(ir: &'a Ir, options: &Options) -> Option<Netlist<'a>> {
        if options.resolution.is_none() && options.maxmicro.is_none() {
            return None;
        }
        let mut netlist = Netlist::new(ir);
        if let Some(seconds) = options.resolution {
            netlist = netlist.with_resolution(seconds);
        }
        if let Some(bound) = options.maxmicro {
            netlist = netlist.with_microsteps(bound as usize);
        }
        Some(netlist)
    }

    /// What the `tf`, `pz` and `sens` requests name, resolved against `ir` before anything compiles:
    /// the voltage source `input` (`P0010` for an instance with no potential branch, `E0801` for
    /// none), the node `output` (`E0801`), the `wrt` params (`E0801`; `P0010` for one a child
    /// derives a param from), and for `sens` that no printed signal is a discrete net (`P0010`).
    fn small_signal_of(ir: &Ir, spec: &RunSpec, probes: &[Probe]) -> Result<(Option<Transfer>, Vec<ParamPath>), Failure> {
        let top = ir.top().path();
        let full = |entry: &str| [entry.to_string(), format!("{top}.{entry}")];
        match &spec.analysis {
            Analysis::Tf { input, output } | Analysis::Pz { input, output } => {
                let instance = full(input).into_iter().find_map(|path| ir.instance_at(&path));
                let instance = instance.ok_or_else(|| Failure::Host { code: "E0801", message: format!("`input` names no instance of the design: `{input}`") })?;
                let branch = instance.body().branches().iter().position(|b| b.kind == BranchKind::Potential).ok_or_else(|| Failure::Host {
                    code: "P0010",
                    message: format!("`input` must be a voltage source: the instance `{}` has no potential contribution", instance.path()),
                })?;
                let node = full(output).into_iter().find_map(|path| ir.node_at(&path));
                let node = node.ok_or_else(|| Failure::Host { code: "E0801", message: format!("`output` names no node of the design: `{output}`") })?;
                let input = BranchIdentifier::new(instance.path().to_string(), format!("b{branch}.flow"));
                Ok((Some(Transfer { input, output: NodeIdentifier::Anonymous(node.id().index()) }), Vec::new()))
            }
            Analysis::Sens { wrt } => {
                if let Some(path) = probes.iter().find_map(|probe| match probe { Probe::Net { path, .. } | Probe::Param { path, .. } => Some(path.clone()), Probe::Term { instance, .. } => Some(format!("i({instance})")), _ => None }) {
                    let message = format!("`{path}` is a discrete net, a param or a flow term: a `sens` run differentiates nodes and flows, not a discrete value");
                    return Err(Failure::Host { code: "P0010", message });
                }
                let params = wrt.iter().map(|entry| ParamPath::resolve(ir, top, entry)).collect::<Result<Vec<_>, Failure>>()?;
                for param in &params {
                    let derived = param.derived(ir);
                    if !derived.is_empty() {
                        let message = format!(
                            "`wrt` names `{}`, from which the params {} derive: the solver's finite difference writes one instance only, so the derivative would skip them",
                            param.label(),
                            derived.join(", ")
                        );
                        return Err(Failure::Host { code: "P0010", message });
                    }
                }
                Ok((None, params))
            }
            _ => Ok((None, Vec::new())),
        }
    }

    /// The `nodeset` entries of `spec`, each resolved to a node of `ir` (`E0801` for one that
    /// names none). A nodeset seeds an operating point the session takes itself, so the analyses
    /// that take theirs inside the solver (`ac`, `transient`) refuse it (`P0010`) rather than
    /// ignore it.
    fn nodeset_of(ir: &Ir, spec: &RunSpec) -> Result<Vec<(NodeIdentifier, f64, String)>, Failure> {
        let top = ir.top().path();
        let seeds = spec
            .options
            .nodeset
            .iter()
            .map(|(entry, value)| {
                let node = [entry.clone(), format!("{top}.{entry}")].iter().find_map(|path| ir.node_at(path));
                let node = node.ok_or_else(|| Failure::Host { code: "E0801", message: format!("the nodeset entry `{entry}` names no node of the design") })?;
                Ok((NodeIdentifier::Anonymous(node.id().index()), *value, entry.clone()))
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        if !seeds.is_empty() && matches!(spec.analysis, Analysis::Ac { .. } | Analysis::Transient { .. } | Analysis::Tf { .. } | Analysis::Pz { .. } | Analysis::Sens { .. } | Analysis::Pss { .. }) {
            let message = format!(
                "run `{}`: `nodeset` seeds an operating point, and a `{}` run takes its own inside the solver, where the host cannot seed it; use `op` or `dc`",
                spec.name,
                spec.analysis.name()
            );
            return Err(Failure::Host { code: "P0010", message });
        }
        Ok(seeds)
    }

    /// The nodeset as the first Newton iterate of `circuit`'s operating point.
    fn seeds(&self, circuit: &CircuitInstance) -> Result<Vec<InitialValue<piperine_solver::prelude::AnalogReference, f64>>, Failure> {
        self.nodeset
            .iter()
            .map(|(id, value, entry)| {
                let reference = circuit.netlist().reference_for(&AnalogVariable::Node(id.clone()));
                let reference = reference.ok_or_else(|| Failure::Backend(format!("the nodeset entry `{entry}` is no unknown of the circuit")))?;
                Ok(InitialValue { reference: reference.clone(), value: *value })
            })
            .collect()
    }

    /// The context every analysis of this session takes.
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// The stages before a backend: the design of `spec` read, its top chosen, type-checked and
    /// elaborated. `open` and `Project::check` run the same code, so a check finds what a run
    /// would.
    pub(crate) fn elaborate(project: &Project, spec: &RunSpec) -> Result<(Arc<Ir>, String), Failure> {
        let design = project.design(spec)?;
        if !design.diagnostics().is_empty() {
            let found = design.diagnostics().to_vec();
            return Err(Failure::Diagnostics { stage: "parse", design: Box::new(design), found });
        }
        let top = Session::top_of(&design, spec)?;
        if let Err(found) = Types::check(&design, &top) {
            return Err(Session::diagnosed("typeck", design, found));
        }
        match Elaborator::new(&design).run(&top) {
            Ok(ir) => Ok((Arc::new(ir), top)),
            Err(found) => Err(Session::diagnosed("elaborate", design, found)),
        }
    }

    /// The top module's name.
    pub fn top(&self) -> &str {
        &self.top
    }

    /// The backend the design runs on.
    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// The elaborated design.
    pub fn ir(&self) -> &Ir {
        &self.ir
    }

    /// The request this session runs.
    pub fn spec(&self) -> &RunSpec {
        &self.spec
    }

    /// The columns the result shows: the run's `print` entries resolved, in order, or the top's
    /// own signals and ports when it has none.
    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    /// How the design was built: compiled now, found in the cache, or interpreted.
    pub fn built(&self) -> Built {
        match &self.compiled {
            Some(c) if c.report().cache_hit => Built::Cached { source: c.source_path() },
            Some(c) => Built::Compiled { source: c.source_path() },
            None => Built::Interpreted,
        }
    }

    /// The bound circuit, for a host that runs an analysis of its own; `None` once the
    /// session's run has taken it.
    pub fn circuit(&mut self) -> Option<&mut CircuitInstance> {
        self.circuit.as_mut()
    }

    /// The request's analysis on the bound circuit, its result as a table; the records of the
    /// run reach `sink` as the solver delivers them. A session runs once: the analysis takes its
    /// circuit.
    pub fn run(&mut self, sink: Rc<RefCell<dyn RecordSink>>) -> Result<Table, Failure> {
        let circuit = self.circuit.take().ok_or_else(|| Failure::Backend("this session has already run: open another".into()))?;
        let failure = Rc::new(RefCell::new(None));
        let delivery = Box::new(Delivery { streams: self.streams.clone(), sink, failure: failure.clone(), recorder: None, finished: Rc::new(RefCell::new(None)) });
        let table = self.analysis(circuit, delivery);
        if let (Ok(table), Some(_)) = (&table, &self.spec.plot) {
            self.plotted.get_or_insert_with(|| table.clone());
        }
        // a record that could not be decoded is a defect the run reports first: the records
        // before it were delivered, none after it was guessed
        match failure.borrow_mut().take() {
            Some(broken) => Err(broken),
            None => table,
        }
    }

    /// The request's analysis on `circuit`, every observation of the solver to `delivery`.
    fn analysis(&mut self, circuit: CircuitInstance, mut delivery: Box<Delivery>) -> Result<Table, Failure> {
        let timed = std::mem::take(&mut self.actions);
        match &self.spec.analysis {
            Analysis::Op => {
                let seeds = self.seeds(&circuit)?;
                let mut solver = Solver::new(circuit).with_context(self.context.clone()).with_sink(delivery).build();
                let result = solver
                    .dc()
                    .and_then(|mut dc| {
                        dc.apply_initial_conditions(seeds);
                        dc.solve()
                    })
                    .map_err(|error| Failure::Solver { analysis: "op", error })?;
                let circuit = solver.circuit();
                let net = |path: &str| Session::discrete(circuit, path);
                let term = |instance: &str, name: &str| circuit.bound().and_then(|b| b.analog.term_value(instance, name));
                Table::operating_point(&self.probes, &result, &net, &term)
            }
            Analysis::Transient { stop, max_step, print_step, from } => {
                // every printed signal finds its place in a step before the first one is taken
                let columns = self.probes.iter().map(|probe| probe.column(&circuit)).collect::<Result<Vec<_>, Failure>>()?;
                let labels = self.probes.iter().map(Probe::label).collect();
                let mut recorder = Recorder::new(columns, labels, from.unwrap_or(0.0));
                if let Some(step) = print_step {
                    recorder = recorder.with_grid(*step, *stop);
                }
                let recorder = Rc::new(RefCell::new(recorder));
                let finished = delivery.finished.clone();
                delivery.recorder = Some(recorder.clone());
                let options = Session::transient_options(*stop, *max_step);
                let options = match self.spec.options.max_steps {
                    Some(budget) => options.with_max_steps(budget as usize),
                    None => options,
                };
                let mut solver = Solver::new(circuit)
                    .with_context(self.context.clone())
                    .with_tran_opts(options)
                    .with_sink(delivery)
                    .with_step_observer(Box::new(Tap(recorder.clone())))
                    .build();
                solver
                    .tran()
                    .and_then(|mut tran| {
                        for action in &timed {
                            match action {
                                HostAction::Drive { net, value, at } => tran.schedule_write(*at, net.clone(), value.clone()),
                                HostAction::Set { path, value, at } => tran.schedule_set(*at, path.instance.clone(), path.param.clone(), Value::Real(*value)),
                            }
                        }
                        tran.solve()
                    })
                    .map_err(|error| Failure::Solver { analysis: "transient", error })?;
                // CLI-05 AC 11: a run that stopped short has no table to give. How far it got is the
                // last step the observer saw, which a design with no analog unknown reaches too (its
                // result holds no analog accept, HII-17). A model that wrote `finish` ended the run at
                // that instant: it is the run's end (FIX-02 AC 1), and the grid stops there
                let end = finished.borrow().map_or(*stop, |at| at.min(*stop));
                recorder.borrow_mut().end_at(end);
                let reached = recorder.borrow().reached();
                if reached.is_none_or(|last| last < end * (1.0 - 1e-9)) {
                    let at = reached.map_or_else(|| "no step".to_string(), |t| format!("the last step at {t:e}"));
                    return Err(Failure::Backend(format!("the transient ended with {at}, before its stop at {end:e}")));
                }
                // the solver held the observers; with it gone the recorder has one owner
                drop(solver);
                let recorder = Rc::try_unwrap(recorder).map_err(|_| Failure::Backend("the run's recorder is still shared after the run".into()))?;
                recorder.into_inner().finish()
            }
            Analysis::Dc { from, to, step, .. } => {
                let mut circuit = circuit;
                let seeds = self.seeds(&circuit)?;
                let swept = self.swept.as_ref().ok_or_else(|| Failure::Backend("a dc run resolved no param to sweep".into()))?;
                // `from + k*step` for every k while the point is not past `to`; the tolerance on
                // k absorbs the rounding of `(to - from) / step` (a whole number of steps that
                // lands a hair under)
                let count = ((to - from) / step + 1e-9).floor() as usize + 1;
                let mut points = Vec::with_capacity(count);
                for k in 0..count {
                    let value = from + k as f64 * step;
                    let refused = |error| Failure::Host { code: "E0802", message: format!("`{}` refuses {value}: {error}", swept.label()) };
                    circuit.set(&swept.instance, &swept.param, ParamValue::Real(value)).map_err(refused)?;
                    let result = circuit
                        .dc(self.context.clone())
                        .and_then(|mut dc| {
                            dc.apply_initial_conditions(seeds.clone());
                            dc.solve()
                        })
                        .map_err(|error| Failure::Solver { analysis: "dc", error })?;
                    let net = |path: &str| Session::discrete(&circuit, path);
                    let term = |instance: &str, name: &str| circuit.bound().and_then(|b| b.analog.term_value(instance, name));
                    points.push((value, Table::point(&self.probes, &result, &net, &term)?));
                }
                Ok(Table::sweep(&swept.label(), &self.probes, points))
            }
            Analysis::Ac { from, to, grid } => {
                let options = AcSweepAnalysisOptions {
                    start_frequency: *from,
                    stop_frequency: *to,
                    steps: grid.steps(*from, *to),
                    logarithmic: matches!(grid, Grid::PerDecade(_)),
                };
                let mut solver = Solver::new(circuit).with_context(self.context.clone()).with_sink(delivery).build();
                let result = solver.ac().and_then(|mut ac| ac.solve_sweep(options)).map_err(|error| Failure::Solver { analysis: "ac", error })?;
                Table::frequency(&self.probes, &result)
            }
            Analysis::Tf { .. } => {
                let mut circuit = circuit;
                let transfer = self.transfer()?;
                let options = TransferFunctionAnalysisOptions {
                    output: AnalogVariable::Node(transfer.output.clone()),
                    output_ref: None,
                    input_source: transfer.input.clone(),
                };
                let result = circuit
                    .transfer_function(options, self.context.clone())
                    .and_then(|mut tf| tf.solve())
                    .map_err(|error| Failure::Solver { analysis: "tf", error })?;
                Ok(Table::transfer(&result))
            }
            Analysis::Pz { .. } => {
                let mut circuit = circuit;
                let transfer = self.transfer()?;
                let options = PoleZeroOptions { input_source: transfer.input.clone(), output: AnalogVariable::Node(transfer.output.clone()), output_ref: None };
                let roots = circuit
                    .pz(options, self.context.clone())
                    .and_then(|pz| Ok(PoleZeroResult { poles: pz.poles()?, zeros: pz.zeros()? }))
                    .map_err(|error| Failure::Solver { analysis: "pz", error })?;
                Ok(Table::roots(&roots))
            }
            Analysis::Pss { period, tstab, max_step, print_step } => {
                let mut circuit = circuit;
                let mut options = PssAnalysisOptions::new(*period).with_tstab(*tstab);
                options.dt = *max_step;
                options.dt_max = *max_step;
                options.max_steps = self.spec.options.max_steps.map(|budget| budget as usize);
                let orbit = circuit
                    .pss(options, self.context.clone())
                    .and_then(|pss| pss.solve())
                    .map_err(|error| Failure::Solver { analysis: "pss", error })?;
                // the orbit starts at `tstab` and holds one period
                let table = Table::transient(&self.probes, &orbit.trace)?;
                match print_step {
                    Some(step) => table.on_grid(*step, *tstab, tstab + period),
                    None => Ok(table),
                }
            }
            Analysis::Sens { .. } => {
                let mut circuit = circuit;
                let nets = circuit.nets();
                let outputs = self.probes.iter().map(|probe| Session::net_of(&nets, probe)).collect::<Result<Vec<Net>, Failure>>()?;
                let params = self.wrt.iter().map(|path| (path.instance.clone(), path.param.clone())).collect();
                let result = circuit
                    .sens(SensAnalysisOptions::new(outputs.clone(), params), self.context.clone())
                    .and_then(|sens| sens.solve())
                    .map_err(|error| Failure::Solver { analysis: "sens", error })?;
                Table::sensitivities(&self.probes, &outputs, &self.wrt, &result)
            }
        }
    }

    /// The source and the node a `tf` or `pz` run resolved at open.
    fn transfer(&self) -> Result<&Transfer, Failure> {
        self.transfer.as_ref().ok_or_else(|| Failure::Backend("a transfer run resolved no input and output".into()))
    }

    /// The solved net of `probe` among `nets`, the one a result is keyed by.
    fn net_of(nets: &[Net], probe: &Probe) -> Result<Net, Failure> {
        let variable = match probe {
            Probe::Node { id, .. } => AnalogVariable::Node(id.clone()),
            Probe::Flow { branch, .. } => AnalogVariable::Branch(branch.clone()),
            Probe::Net { path, .. } | Probe::Param { path, .. } => return Err(Failure::Backend(format!("`{path}` is a discrete net or a param: it has no solved analog net"))),
            Probe::Term { instance, .. } => return Err(Failure::Backend(format!("`i({instance})` is a flow term: it has no solved analog net"))),
        };
        let found = nets.iter().find(|net| net.analog_variable().is_some_and(|v| **v == variable));
        found.cloned().ok_or_else(|| Failure::Backend(format!("`{}` is no solved net of the circuit", probe.label())))
    }

    /// The options of a transient: `max_step` is the first step and the largest; without it the
    /// first step is `stop / 1000` and the largest the solver's default. The run keeps only its
    /// last step, and no window: the rows are the recorder's (`long-run` LR-07).
    fn transient_options(stop: f64, max_step: Option<f64>) -> TransientAnalysisOptions {
        let options = match max_step {
            Some(step) => TransientAnalysisOptions::new(stop, step).with_dt_max(step),
            None => TransientAnalysisOptions::new(stop, stop / 1000.0),
        };
        options.with_keep(Keep::Last)
    }

    /// The bytes of the discrete net at `path` in the bound design: its index in the digital
    /// plan is its id.
    fn discrete(circuit: &CircuitInstance, path: &str) -> Option<Vec<u8>> {
        let b = circuit.bound()?;
        let at = b.digital.plan().nets.iter().position(|n| n.name == path)?;
        Some(b.digital.read(NetId(at as u32)).to_vec())
    }

    /// What the design declares (HII-10): every param with its value as the run uses it, the
    /// top's ports and every record stream. No analysis runs, and none is needed.
    pub fn metadata(&self) -> Metadata {
        Session::metadata_of(&self.ir, &self.spec)
    }

    /// The top of `spec` and what its design declares, elaborated and nothing else: no backend
    /// builds it and no analysis runs (`piperine info`). Diagnostics are the `Failure` of a
    /// design that does not elaborate, as `Session::open` gives them.
    pub fn describe(project: &Project, spec: &RunSpec) -> Result<(String, Metadata), Failure> {
        let (ir, top) = Session::elaborate(project, spec)?;
        Ok((top, Session::metadata_of(&ir, spec)))
    }

    fn metadata_of(ir: &Ir, spec: &RunSpec) -> Metadata {
        // the request's `params`, each by the full path of its param, the last one given winning
        let set: Vec<(String, f64)> = spec
            .params
            .iter()
            .filter_map(|(entry, value)| ParamPath::resolve(ir, ir.top().path(), entry).ok().map(|p| (format!("{}.{}", p.instance, p.param), *value)))
            .collect();
        Metadata::of(ir, &|path| set.iter().rev().find(|(p, _)| p == path).map(|(_, v)| *v))
    }

    /// The plot of the run that has completed, drawn from the rows of its table: every accepted
    /// step without `print_step`, the grid's rows with it; `P0010` for an operating point, and a
    /// `Failure` before the run.
    pub fn plot(&self) -> Result<Plot, Failure> {
        let table = self.plotted.as_ref().ok_or_else(|| Failure::Backend("no plot: the request has no `plot`, or the session has not run".into()))?;
        Plot::of(table, &self.spec.analysis)
    }

    /// The `top` key, else the one module without ports of the root crate (`P0011` when there
    /// is none or several, the candidates listed).
    fn top_of(design: &Design, spec: &RunSpec) -> Result<String, Failure> {
        if let Some(top) = &spec.top {
            return Ok(top.clone());
        }
        let root = design.root_crate();
        let candidates: Vec<&str> = root.modules().into_iter().filter(|m| m.ports().is_empty()).filter_map(|m| m.name()).collect();
        match candidates.as_slice() {
            [one] => Ok(one.to_string()),
            [] => Err(Failure::Host {
                code: "P0011",
                message: format!("no top: the crate `{}` holds no module without ports; name one with `top`", root.name()),
            }),
            several => Err(Failure::Host {
                code: "P0011",
                message: format!("no top: the crate `{}` holds several modules without ports ({}); name one with `top`", root.name(), several.join(", ")),
            }),
        }
    }

    /// The run's `print` entries resolved against `ir` (`E0801` for one that names nothing),
    /// or the top's defaults.
    fn probes_of(ir: &Ir, spec: &RunSpec) -> Result<Vec<Probe>, Failure> {
        let top = ir.top().path();
        if spec.print.is_empty() {
            return Ok(Probe::defaults(ir, top));
        }
        spec.print.iter().map(|entry| Probe::resolve(ir, top, entry)).collect()
    }

    fn diagnosed(stage: &'static str, design: Design, found: Vec<DesignDiagnostic>) -> Failure {
        Failure::Diagnostics { stage, design: Box::new(design), found }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transient_keeps_only_the_last_step() {
        // LR-07 AC 1: a session's transient keeps the last step alone and sets no `record_from`
        // window; the first step and the largest are what the request gave
        let options = Session::transient_options(1.0, Some(0.25));
        assert_eq!(options.keep, Keep::Last);
        assert_eq!(options.record_from, 0.0);
        assert_eq!((options.stop_time, options.dt_max), (1.0, 0.25));
        assert_eq!(Session::transient_options(1.0, None).keep, Keep::Last);
    }
}
