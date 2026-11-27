//! The top of a hand-built mixed design: the instance and the nets the bodies under test are placed on.

use std::collections::BTreeMap;

use piperine_ir::{
    BodyId, BodySpec, Constant, InstanceId, InstanceSpec, Ir, IrBuilder, NetId, NetSpec, NodeRef, NodeSpec, Origin, Real, ValueType,
};

/// The top instance and its nets, while bodies are placed under it.
pub struct TopDesign {
    /// The builder every body and instance of the design is made through.
    pub b: IrBuilder,
    top: InstanceId,
    nets: BTreeMap<String, NetId>,
    nodes: BTreeMap<String, NodeRef>,
}

impl TopDesign {
    /// The origin every placed item carries: line 1 of `top.msl`.
    fn at() -> Origin {
        Origin { file: "top.msl".to_string(), line: 1, col: 1 }
    }

    /// An empty design under its `Top` instance.
    pub fn new() -> TopDesign {
        let mut b = IrBuilder::new();
        let top_body = b.body(BodySpec {
            name: "Top".into(),
            origin: Self::at(),
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
            origin: Self::at(),
        });
        TopDesign { b, top, nets: BTreeMap::new(), nodes: BTreeMap::new() }
    }

    fn net(&mut self, name: &str, ty: ValueType) -> NetId {
        let (top, b) = (self.top, &mut self.b);
        *self.nets.entry(name.to_string()).or_insert_with(|| {
            b.net(NetSpec { path: format!("Top.{name}"), ty, declared_by: top, init: None, origin: Self::at() })
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
                NodeRef::Node(b.node(NodeSpec { path: format!("Top.{name}"), nature, declared_by: top, init: None, origin: Self::at() }))
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
            origin: Self::at(),
        });
        let ids: Vec<NetId> = nets.iter().map(|(n, ty)| self.net(n, ty.clone())).collect();
        if !ids.is_empty() {
            self.b.bind_nets(instance, ids);
        }
    }

    /// Places `body` as `Top.<name>`; `nets` is the design net of each carrier, in the body's
    /// order, by name under `Top` and with the carrier's type.
    pub fn place(&mut self, name: &str, body: BodyId, params: Vec<Constant>, nets: &[(&str, ValueType)]) {
        self.place_with(name, body, &[], params, nets);
    }

    /// The `module Hold(out q: <ty> = <init>)` body: a carrier that only holds its initial value, the constant
    /// input of a body under test.
    pub fn hold(&mut self, name: &str, ty: ValueType, init: Constant) -> BodyId {
        let body = self.b.body(crate::digital::Buck::spec(name, "top.msl", 1));
        self.b.carrier(body, crate::digital::Buck::carrier("q", ty, piperine_ir::CarrierRole::Out, Some(init), "top.msl", 1, 20));
        body
    }

    /// The solver instant of the `k`-th rising edge (from 1) of the 200 MHz oscillator, in ticks: the
    /// oscillator toggles after half a period (2.5 ns) from the start, so its rising edges are 2.5 ns,
    /// 7.5 ns, ...
    pub fn rising_edge(k: i64) -> i64 {
        2_500_000 + 5_000_000 * (k - 1)
    }

    /// The finished design, held to `Ir::check`.
    pub fn finish(self) -> Ir {
        let ir = self.b.finish(self.top);
        assert_eq!(ir.check(), vec![], "the fixture passes check()");
        ir
    }
}

