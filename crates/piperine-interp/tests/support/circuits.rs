//! The M1 circuits of this feature's tests, built by hand over `devices.rs`: the divider, a
//! diode behind a source and 1 ohm, an RC low-pass with an AC source, and an RC with `out` held
//! at 0. Each is a top body with signal nodes and child placements, as the `.msl` twins read.

use piperine_ir::{
    BodyId, BodyNode, BodySpec, Branch, Child, Constant, InstanceSpec, Ir, IrBuilder, NodeRef,
    NodeRole, NodeSpec, Origin, Real, Terminal,
};

use crate::devices::Devices;

/// One child placement: its name, body, terminals (`None` is ground) and bound params.
pub struct Place {
    pub name: &'static str,
    pub body: BodyId,
    pub nodes: Vec<Option<usize>>,
    pub params: Vec<f64>,
}

pub struct Circuits;

impl Circuits {
    fn at(line: u32) -> Origin {
        Origin { file: "circuits.msl".into(), line, col: 1 }
    }

    /// A top `name` with `signals` (name, initial potential) and the placements, as an `Ir`.
    fn top(mut b: IrBuilder, name: &str, signals: &[(&str, Option<f64>)], places: Vec<Place>) -> Ir {
        let el = Devices::electrical(&mut b);
        let terminal = |n: &Option<usize>| n.map_or(Terminal::Ground, |i| Terminal::Node(i as u32));
        let body = b.body(BodySpec {
            name: name.to_string(),
            origin: Self::at(1),
            params: vec![],
            nodes: signals
                .iter()
                .enumerate()
                .map(|(i, (n, init))| BodyNode {
                    name: n.to_string(),
                    nature: el,
                    role: NodeRole::Signal,
                    init: init.map(Real::new),
                    origin: Self::at(2 + i as u32),
                })
                .collect(),
            branches: Vec::<Branch>::new(),
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: None,
            children: places
                .iter()
                .enumerate()
                .map(|(i, p)| Child {
                    name: p.name.to_string(),
                    body: p.body,
                    nodes: p.nodes.iter().map(terminal).collect(),
                    params: p.params.iter().enumerate().map(|(k, v)| (k as u32, Constant::Real(Real::new(*v)))).collect(),
                    origin: Self::at(10 + i as u32),
                })
                .collect(),
        });
        let top = b.next_instance_id();
        let ids: Vec<_> = signals
            .iter()
            .enumerate()
            .map(|(i, (n, init))| {
                b.node(NodeSpec {
                    path: format!("{name}.{n}"),
                    nature: el,
                    declared_by: top,
                    init: init.map(Real::new),
                    origin: Self::at(2 + i as u32),
                })
            })
            .collect();
        let top = b.instance(InstanceSpec {
            path: name.to_string(),
            name: name.to_string(),
            body,
            parent: None,
            nodes: ids.iter().map(|id| NodeRef::Node(*id)).collect(),
            params: vec![],
            origin: Self::at(1),
        });
        for (i, p) in places.iter().enumerate() {
            b.instance(InstanceSpec {
                path: format!("{name}.{}", p.name),
                name: p.name.to_string(),
                body: p.body,
                parent: Some(top),
                nodes: p.nodes.iter().map(|n| n.map_or(NodeRef::Ground, |k| NodeRef::Node(ids[k]))).collect(),
                params: p.params.iter().map(|v| Constant::Real(Real::new(*v))).collect(),
                origin: Self::at(10 + i as u32),
            });
        }
        b.finish(top)
    }

    /// The low-pass capacitor: with 1 k, its corner is 100 kHz.
    pub const FARADS: f64 = 1.5915494309189535e-9;

    /// `Divider`: 10 V across 1 k and 3 k; `mid` reads 7.5 V.
    pub fn divider() -> Ir {
        let mut b = IrBuilder::new();
        let top = Devices::divider(&mut b);
        b.finish(top)
    }

    /// `DiodeCircuit`: a 0.8 V source, 1 ohm, a default diode to ground.
    pub fn diode() -> Ir {
        let mut b = IrBuilder::new();
        let (v, r, d) = (Devices::vsource(&mut b), Devices::resistor(&mut b), Devices::diode(&mut b));
        let places = vec![
            Place { name: "v1", body: v, nodes: vec![Some(0), None], params: vec![0.8, 0.0] },
            Place { name: "r1", body: r, nodes: vec![Some(0), Some(1)], params: vec![1.0] },
            Place { name: "d1", body: d, nodes: vec![Some(1), None], params: vec![1e-14, 1.0, 0.0] },
        ];
        Self::top(b, "DiodeCircuit", &[("vin", None), ("a", None)], places)
    }

    /// `RcLowpass`: a source with `ac = 1`, 1 k, 1.59 nF to ground.
    pub fn rc_lowpass() -> Ir {
        Self::rc("RcLowpass", None, vec![0.0, 1.0])
    }

    /// `RcHeld`: a 1 V source, 1 k, 1.59 nF, with `out` held at 0 V.
    pub fn rc_held() -> Ir {
        Self::rc("RcHeld", Some(0.0), vec![1.0, 0.0])
    }

    fn rc(name: &str, out_init: Option<f64>, source: Vec<f64>) -> Ir {
        let mut b = IrBuilder::new();
        let (v, r, c) = (Devices::vsource(&mut b), Devices::resistor(&mut b), Devices::capacitor(&mut b));
        let places = vec![
            Place { name: "v1", body: v, nodes: vec![Some(0), None], params: source },
            Place { name: "r1", body: r, nodes: vec![Some(0), Some(1)], params: vec![1000.0] },
            Place { name: "c1", body: c, nodes: vec![Some(1), None], params: vec![Self::FARADS] },
        ];
        Self::top(b, name, &[("vin", None), ("out", out_init)], places)
    }
}
