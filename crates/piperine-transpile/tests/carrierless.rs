//! A design whose reactions sit in a body with no carrier still runs them on the compiled
//! backend (msl-cli contract.md D12): the Book's RC low-pass, chapter 1, with its `Monitor`, a
//! body with one `Electrical` port, a `cross` and an `emit!`, and no `signal`. Built by hand from
//! the MSL text below, compiled once, and run as a transient; the record of the crossing must
//! reach the sink. Expected values are the analytic step response of the RC, never the printed
//! source.
//!
//! ```text
//! module Monitor(a: Electrical) { level: f64 = 0.5; }
//! digital for Monitor { on cross(v(a) - level) { emit!("trip", v = v(a)); } }
//!
//! module Step() {
//!     signal vin: Electrical;
//!     signal vout: Electrical = 0.0;
//!     let v1 = Vsource(vin, gnd) { dc: 1.0 };
//!     let r1 = Resistor(vin, vout) { r: 1k };
//!     let c1 = Capacitor(vout, gnd) { c: 1n };
//!     let mon = Monitor(vout);
//! }
//! ```
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use piperine_ir::{
    BinaryOp, BodyId, BodyNode, BodySpec, Branch, Child, Constant, EmitField, Event, EventSource, InstanceSpec, Ir, IrBuilder, NodeRef,
    NodeRole, NodeSpec, Origin, Real, Stmt, Stream, StreamField, Terminal, ValueType,
};
use piperine_plan::RecordLayout;
use piperine_solver::prelude::{Solver, TransientAnalysisOptions};
use piperine_solver::protocol::{AnalysisKind, Observation, ObservationKind, Sink};
use piperine_transpile::{Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;

use devices::Devices;
use digital::{Buck, Dx};

const F: &str = "step.msl";
const R: f64 = 1e3;
const C: f64 = 1e-9;
const LEVEL: f64 = 0.5;

fn at(line: u32) -> Origin {
    Origin { file: F.into(), line, col: 1 }
}

/// `Monitor` (step.msl:4-5): no carrier, one event, one reaction, one stream.
fn monitor(b: &mut IrBuilder) -> BodyId {
    let el = Buck::electrical(b);
    let body = b.body(BodySpec {
        params: vec![Buck::param("level", F, 4, Some(LEVEL), None)],
        nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: Buck::origin(F, 4, 16) }],
        ..Buck::spec("Monitor", F, 4)
    });
    let trip = b.stream(body, Stream { name: "trip".into(), fields: vec![StreamField { name: "v".into(), ty: ValueType::F64 }] });
    let mut x = Dx::new(b, F, 5, 32);
    let v = x.probe(Terminal::Node(0), Terminal::Ground);
    let level = x.param(0, ValueType::F64);
    let rising = x.bin(BinaryOp::Sub, v, level, ValueType::F64);
    x.at(5, 57);
    let v = x.probe(Terminal::Node(0), Terminal::Ground);
    let emit = x.b.stmt(Stmt::Emit { stream: trip, fields: vec![EmitField::Value { field: 0, value: v }] });
    let cross = b.event(body, Event { source: EventSource::Cross { e: rising, ttol: None, etol: None }, guard: None, origin: Buck::origin(F, 5, 26) });
    Buck::reaction(b, body, vec![cross], vec![emit], vec![], F, 5);
    body
}

/// `Step` (step.msl:7-14) as the top, its four children placed under it.
fn step() -> Ir {
    let mut b = IrBuilder::new();
    let (vsrc, res, cap) = (Devices::vsource(&mut b), Devices::resistor(&mut b), Devices::capacitor(&mut b));
    let mon = monitor(&mut b);
    let el = Devices::electrical(&mut b);
    let signals: [(&str, Option<f64>); 2] = [("vin", None), ("vout", Some(0.0))];
    // (name, body, terminals by signal index, params); `None` is ground
    let places: [(&str, BodyId, Vec<Option<usize>>, Vec<f64>); 4] = [
        ("v1", vsrc, vec![Some(0), None], vec![1.0, 0.0]),
        ("r1", res, vec![Some(0), Some(1)], vec![R]),
        ("c1", cap, vec![Some(1), None], vec![C]),
        ("mon", mon, vec![Some(1)], vec![LEVEL]),
    ];
    let terminal = |n: &Option<usize>| n.map_or(Terminal::Ground, |i| Terminal::Node(i as u32));
    let real = |v: &f64| Constant::Real(Real::new(*v));
    let top_body = b.body(BodySpec {
        name: "Step".into(),
        origin: at(7),
        params: vec![],
        nodes: signals
            .iter()
            .enumerate()
            .map(|(i, (n, init))| BodyNode { name: n.to_string(), nature: el, role: NodeRole::Signal, init: init.map(Real::new), origin: at(8 + i as u32) })
            .collect(),
        branches: Vec::<Branch>::new(),
        limits: vec![],
        stims: vec![],
        locals: vec![],
        analog: None,
        children: places
            .iter()
            .enumerate()
            .map(|(i, (name, body, nodes, params))| Child {
                name: name.to_string(),
                body: *body,
                nodes: nodes.iter().map(terminal).collect(),
                params: params.iter().enumerate().map(|(k, v)| (k as u32, real(v))).collect(),
                origin: at(10 + i as u32),
            })
            .collect(),
    });
    let top = b.next_instance_id();
    let ids: Vec<_> = signals
        .iter()
        .enumerate()
        .map(|(i, (n, init))| {
            b.node(NodeSpec { path: format!("Step.{n}"), nature: el, declared_by: top, init: init.map(Real::new), origin: at(8 + i as u32) })
        })
        .collect();
    let top = b.instance(InstanceSpec {
        path: "Step".into(),
        name: "Step".into(),
        body: top_body,
        parent: None,
        nodes: ids.iter().map(|id| NodeRef::Node(*id)).collect(),
        params: vec![],
        origin: at(7),
    });
    for (i, (name, body, nodes, params)) in places.iter().enumerate() {
        b.instance(InstanceSpec {
            path: format!("Step.{name}"),
            name: name.to_string(),
            body: *body,
            parent: Some(top),
            nodes: nodes.iter().map(|n| n.map_or(NodeRef::Ground, |k| NodeRef::Node(ids[k]))).collect(),
            params: params.iter().map(real).collect(),
            origin: at(10 + i as u32),
        });
    }
    let ir = b.finish(top);
    assert_eq!(ir.check(), vec![], "the fixture passes check()");
    ir
}

#[derive(Clone, Default)]
struct Seen(Rc<RefCell<Vec<Observation>>>);

impl Sink for Seen {
    fn observe(&mut self, o: &Observation) {
        self.0.borrow_mut().push(o.clone());
    }
}

#[test]
fn a_reaction_with_no_carrier_runs_and_its_record_arrives() {
    let ir = Arc::new(step());
    let cache = tempfile::tempdir().expect("a temp dir");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let compiled = Transpiler::new(ir.clone()).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
    let circuit = compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let seen = Seen::default();
    let mut options = TransientAnalysisOptions::new(5e-6, 1e-8);
    options.dt_max = 1e-8;
    let mut solver = Solver::new(circuit).with_tran_opts(options).with_sink(Box::new(seen.clone())).build();
    solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}"));

    // abi.md C5: a record arrives on the pin path `<instance path>.<stream>`
    let records: Vec<(f64, Vec<u8>)> = seen
        .0
        .borrow()
        .iter()
        .filter(|o| o.analysis == AnalysisKind::Transient && o.path == "Step.mon.trip")
        .filter_map(|o| match &o.kind {
            ObservationKind::Record { bytes, .. } => Some((o.time, bytes.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(records.len(), 1, "one crossing, one record: {records:?}");
    let (time, bytes) = &records[0];
    // v(vout) = 1 - exp(-t / RC) crosses 0.5 at RC ln 2 (the Book, chapter 1: 6.9315e-7 s);
    // 1e-3 relative is the parity contract's tolerance
    let expected = R * C * std::f64::consts::LN_2;
    assert!((time - expected).abs() <= 1e-3 * expected, "trip at {time}, expected {expected}");
    // the record's one field `v`, read through the stream's layout (contract.md D2), is equal to
    // the level it crossed
    let body = ir.instance_at("Step.mon").expect("the monitor instance").body();
    let layout = RecordLayout::of(&body.streams()[0]).expect("a layout for `trip`");
    let (offset, carrier) = &layout.fields[0];
    let leaf = carrier.fields().into_iter().next().expect("the value leaf of an `f64`");
    let value_at = (offset + leaf.offset) as usize;
    assert_eq!(bytes.len(), layout.size as usize, "the record is the layout's size");
    let value = f64::from_le_bytes(bytes[value_at..value_at + 8].try_into().expect("eight bytes"));
    assert!((value - LEVEL).abs() <= 1e-3 * LEVEL, "v = {value}, expected {LEVEL}");
}
