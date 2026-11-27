//! A hand-built mixed design placed under a top, compiled once, and run on the solver with a
//! sink that keeps what the solver observed: the value changes of each net with their instants,
//! and nothing the printed source says.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use piperine_ir::{
    BodyId, BodySpec, Constant, InstanceId, InstanceSpec, Ir, IrBuilder, NetId, NetSpec, NodeRef, NodeSpec, Origin, Real, ValueType,
};
use piperine_solver::prelude::{Context, Solver, TransientAnalysisOptions, TransientAnalysisResult};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};
use piperine_transpile::{Compiled, Settings, Transpiler};

fn at() -> Origin {
    Origin { file: "top.msl".to_string(), line: 1, col: 1 }
}

/// The top instance and its nets, while bodies are placed under it.
pub struct Wire {
    pub b: IrBuilder,
    top: InstanceId,
    nets: BTreeMap<String, NetId>,
    nodes: BTreeMap<String, NodeRef>,
}

impl Wire {
    pub fn new() -> Wire {
        let mut b = IrBuilder::new();
        let top_body = b.body(BodySpec {
            name: "Top".into(),
            origin: at(),
            params: vec![],
            nodes: vec![],
            branches: vec![],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: vec![],
        });
        let top = b.instance(InstanceSpec {
            path: "Top".into(),
            name: "Top".into(),
            body: top_body,
            parent: None,
            nodes: vec![],
            params: vec![],
            origin: at(),
        });
        Wire { b, top, nets: BTreeMap::new(), nodes: BTreeMap::new() }
    }

    fn net(&mut self, name: &str, ty: ValueType) -> NetId {
        let (top, b) = (self.top, &mut self.b);
        *self.nets.entry(name.to_string()).or_insert_with(|| {
            b.net(NetSpec { path: format!("Top.{name}"), ty, declared_by: top, init: None, origin: at() })
        })
    }

    /// The analog node `Top.<name>` (an `Electrical` node, made once); `"gnd"` is the reference.
    pub fn node(&mut self, name: &str) -> NodeRef {
        if name == "gnd" {
            return NodeRef::Ground;
        }
        let (top, b) = (self.top, &mut self.b);
        self.nodes
            .entry(name.to_string())
            .or_insert_with(|| {
                let nature = crate::digital::Buck::electrical(b);
                NodeRef::Node(b.node(NodeSpec { path: format!("Top.{name}"), nature, declared_by: top, init: None, origin: at() }))
            })
            .clone()
    }

    /// Places an analog body: `nodes` are its ports, in order, `params` its `f64` params.
    pub fn place_analog(&mut self, name: &str, body: BodyId, nodes: &[&str], params: &[f64]) {
        self.place_with(name, body, nodes, params.iter().map(|v| Constant::Real(Real::new(*v))).collect(), &[]);
    }

    /// Places `body` as `Top.<name>` on `nodes` and `nets`, as `place` does.
    pub fn place_with(&mut self, name: &str, body: BodyId, nodes: &[&str], params: Vec<Constant>, nets: &[(&str, ValueType)]) {
        let refs: Vec<NodeRef> = nodes.iter().map(|n| self.node(n)).collect();
        let top = self.top;
        let instance = self.b.instance(InstanceSpec {
            path: format!("Top.{name}"),
            name: name.into(),
            body,
            parent: Some(top),
            nodes: refs,
            params,
            origin: at(),
        });
        let ids: Vec<NetId> = nets.iter().map(|(n, ty)| self.net(n, ty.clone())).collect();
        if !ids.is_empty() {
            self.b.bind_nets(instance, ids).unwrap();
        }
    }

    /// Places `body` as `Top.<name>`; `nets` is the design net of each carrier, in the body's
    /// order, by name under `Top` and with the carrier's type.
    pub fn place(&mut self, name: &str, body: BodyId, params: Vec<Constant>, nets: &[(&str, ValueType)]) {
        self.place_with(name, body, &[], params, nets);
    }

    pub fn finish(self) -> Ir {
        let ir = self.b.finish(self.top);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        ir
    }
}

/// What the sink kept.
#[derive(Clone, Default)]
pub struct Seen(Rc<RefCell<Vec<Observation>>>);

struct Keep(Seen);

impl Sink for Keep {
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
pub struct Built {
    pub ir: Arc<Ir>,
    pub compiled: Compiled,
    _cache: tempfile::TempDir,
}

impl Built {
    pub fn new(ir: Ir) -> Built {
        let ir = Arc::new(ir);
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let compiled = Transpiler::new(ir.clone())
            .unwrap_or_else(|e| panic!("{e}"))
            .compile(&settings)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(compiled.report().warnings, "", "the generated source compiles with no warning");
        Built { ir, compiled, _cache: cache }
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
            .with_sink(Box::new(Keep(seen.clone())))
            .build();
        let result = solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}"));
        (seen, result)
    }
}

/// `module Hold(out q: <ty> = <init>)`: a carrier that only holds its initial value, the
/// constant input of a body under test.
pub fn constant(b: &mut IrBuilder, name: &str, ty: ValueType, init: Constant) -> BodyId {
    let body = b.body(crate::digital::Buck::spec(name, "top.msl", 1));
    b.carrier(body, crate::digital::Buck::carrier("q", ty, piperine_ir::CarrierRole::Out, Some(init), "top.msl", 1, 20));
    body
}

/// The solver instant of the `k`-th rising edge (from 1) of the 200 MHz oscillator, in ticks:
/// the oscillator toggles after half a period (2.5 ns) from the start, so its rising edges are
/// 2.5 ns, 7.5 ns, ...
pub fn rise(k: i64) -> i64 {
    2_500_000 + 5_000_000 * (k - 1)
}
