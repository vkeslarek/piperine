//! The host's sources on the top's node ports (§8.1.14, §23.1 rule 1; PI-07).
//!
//! A top module's node port has nothing inside the design to drive it, so the host does: the
//! elaborator gives the IR one body `HostSource`, a potential source between its port and `gnd`,
//! and one instance of it per node port of the top, at the path `<top>.<port>.source`, its node
//! bound to the port's node. Nothing here touches the object model (the POM navigability rule,
//! MD-25): the source is IR the author never wrote, built from IR expressions, with no MSL
//! source behind it (design D8). A value input of the top has no such source and is `E0811`.

use std::collections::BTreeMap;

use piperine_ir::{Addend, BinaryOp, Branch, BranchKind, BodyId, BodyNode, BodySpec, Constant, Env, Expr, ExprId, InstanceId, InstanceSpec, IrBuilder, Method, NatureId, NodeId, NodeRef, NodeRole, Operator, Origin, Param, QuantityRole, Real, Site, Stmt, Terminal, ValueType};

use super::elaborator::Elaboration;
use crate::msl::Diagnostic;
use crate::pom::{DeclId, DeclKind};

/// A param of the body `HostSource`, in the order of the body's params.
///
/// The potential across the source is `v0 + (v1 - v0) * clamp((t - t0) / (t1 - t0), 0, 1)` plus
/// the AC stimulus `ac_stim(mag, phase)`: a DC value is `v0 = v1`, and a piecewise-linear
/// waveform sets the corners of one segment at a time (design D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostSourceParam {
    /// The AC stimulus magnitude (§14.4.4); the body's param is named `mag`.
    Magnitude,
    /// The AC stimulus phase in radians (§14.3.1).
    Phase,
    /// The time the segment starts.
    T0,
    /// The potential at `t0`.
    V0,
    /// The time the segment ends.
    T1,
    /// The potential at `t1`.
    V1,
}

impl HostSourceParam {
    /// Every param, in the order of the body's params.
    pub const ALL: [HostSourceParam; 6] = [HostSourceParam::Magnitude, HostSourceParam::Phase, HostSourceParam::T0, HostSourceParam::V0, HostSourceParam::T1, HostSourceParam::V1];

    /// The param's name in the body: the last segment of its path (`Top.vin.source.v0`).
    pub fn name(self) -> &'static str {
        match self {
            HostSourceParam::Magnitude => "mag",
            HostSourceParam::Phase => "phase",
            HostSourceParam::T0 => "t0",
            HostSourceParam::V0 => "v0",
            HostSourceParam::T1 => "t1",
            HostSourceParam::V1 => "v1",
        }
    }

    /// The value of the param before the host sets it: a source at 0 V with no stimulus.
    pub fn default_value(self) -> f64 {
        match self {
            HostSourceParam::T1 => 1.0,
            _ => 0.0,
        }
    }

    /// The index of the param among the body's params.
    fn index(self) -> u32 {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0) as u32
    }
}

/// A node port of the top and the node the placement made for it.
pub(super) struct HostPort {
    /// The port's name in the top.
    pub(super) name: String,
    /// The node the top's port is bound to.
    pub(super) node: NodeId,
    /// The port's nature, which the source's node shares.
    pub(super) nature: NatureId,
    /// The port's position in the source.
    pub(super) origin: Origin,
}

/// The expressions of the body `HostSource` over one `IrBuilder`: the data path from a param to
/// the potential the source contributes.
struct HostSourceExpressions<'a> {
    /// The builder the expressions are added to.
    ir: &'a mut IrBuilder,
    /// The source position every expression carries: the port's.
    origin: Origin,
}

impl HostSourceExpressions<'_> {
    /// An `f64` expression of `kind` at the port's origin.
    fn emit(&mut self, kind: Expr) -> ExprId {
        self.ir.expr(kind, ValueType::F64, self.origin.clone())
    }

    /// `lhs op rhs`.
    fn binary(&mut self, op: BinaryOp, lhs: ExprId, rhs: ExprId) -> ExprId {
        self.emit(Expr::Binary { op, lhs, rhs })
    }

    /// A read of the body's param `param`.
    fn param(&mut self, param: HostSourceParam) -> ExprId {
        self.emit(Expr::Param(param.index()))
    }

    /// `(t - t0) / (t1 - t0)` clamped to `[0, 1]`: how far along the segment the time is.
    fn fraction(&mut self) -> ExprId {
        let time = self.emit(Expr::Env(Env::Time));
        let (t0, t1) = (self.param(HostSourceParam::T0), self.param(HostSourceParam::T1));
        let elapsed = self.binary(BinaryOp::Sub, time, t0);
        let length = self.binary(BinaryOp::Sub, t1, t0);
        let ratio = self.binary(BinaryOp::Div, elapsed, length);
        let (low, high) = (self.emit(Expr::Real(Real::new(0.0))), self.emit(Expr::Real(Real::new(1.0))));
        self.emit(Expr::Method { method: Method::Clamp, receiver: ratio, args: vec![low, high] })
    }

    /// The potential the source holds: the segment's value plus the AC stimulus (site 0).
    fn potential(&mut self) -> ExprId {
        let (v0, v1) = (self.param(HostSourceParam::V0), self.param(HostSourceParam::V1));
        let fraction = self.fraction();
        let rise = self.binary(BinaryOp::Sub, v1, v0);
        let travelled = self.binary(BinaryOp::Mul, rise, fraction);
        let segment = self.binary(BinaryOp::Add, v0, travelled);
        let (magnitude, phase) = (self.param(HostSourceParam::Magnitude), self.param(HostSourceParam::Phase));
        let stimulus = self.emit(Expr::Stim { site: 0, mag: magnitude, phase: Some(phase) });
        self.binary(BinaryOp::Add, segment, stimulus)
    }
}

/// The host sources of one elaboration: the body `HostSource` of each nature a top port has,
/// and the instances placed from them.
pub(super) struct HostSources<'a> {
    /// The builder the bodies and the instances are added to.
    ir: &'a mut IrBuilder,
    /// The body `HostSource` built for each nature so far.
    bodies: BTreeMap<NatureId, BodyId>,
}

impl<'a> HostSources<'a> {
    /// The host sources of an elaboration that has built none yet.
    pub(super) fn new(ir: &'a mut IrBuilder) -> Self {
        HostSources { ir, bodies: BTreeMap::new() }
    }

    /// One instance of `HostSource` per port, children of `top` at `<top_path>.<port>.source`.
    pub(super) fn place(&mut self, top: InstanceId, top_path: &str, ports: &[HostPort]) {
        for port in ports {
            let body = self.body_for(port);
            let params = HostSourceParam::ALL.iter().map(|p| Constant::Real(Real::new(p.default_value()))).collect();
            let path = format!("{top_path}.{}.source", port.name);
            self.ir.instance(InstanceSpec { path, name: "source".to_string(), body, parent: Some(top), nodes: vec![NodeRef::Node(port.node)], params, origin: port.origin.clone() });
        }
    }

    /// The body `HostSource` over `port`'s nature, built the first time a port of it is placed.
    fn body_for(&mut self, port: &HostPort) -> BodyId {
        if let Some(body) = self.bodies.get(&port.nature) {
            return *body;
        }
        let name = if self.bodies.is_empty() { "HostSource".to_string() } else { format!("HostSource__{}", self.bodies.len()) };
        let value = HostSourceExpressions { ir: &mut *self.ir, origin: port.origin.clone() }.potential();
        let statement = self.ir.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Potential, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] });
        let params = HostSourceParam::ALL
            .iter()
            .map(|p| Param { name: p.name().to_string(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(p.default_value()))), check: None, structural: false, origin: port.origin.clone() })
            .collect();
        let body = self.ir.body(BodySpec {
            name,
            origin: port.origin.clone(),
            params,
            nodes: vec![BodyNode { name: "p".to_string(), nature: port.nature, role: NodeRole::Port, init: None, origin: port.origin.clone() }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Potential }],
            limits: Vec::new(),
            stims: vec![Site { name: "ac".to_string(), origin: port.origin.clone(), enabled: None }],
            locals: Vec::new(),
            analog: Some(vec![statement]),
            children: Vec::new(),
        });
        self.bodies.insert(port.nature, body);
        body
    }
}

impl Elaboration<'_, '_> {
    /// `E0811` for each value input of `top`, and `true` when there is one (§8.1.14, §23.1 rule
    /// 1): the host drives a top's node ports and discrete `in` ports, and a value input has no
    /// source to hold it.
    pub(super) fn refuse_value_inputs(&mut self, top: DeclId) -> bool {
        let data = self.state.design.decl_data(top);
        let module = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let inputs: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Port(_)) && self.is_value_input(*c)).collect();
        for input in &inputs {
            let port = self.state.design.decl_data(*input);
            let name = port.name.as_ref().map_or_else(String::new, |n| n.text.clone());
            let (file, span) = (port.file, port.name.as_ref().map_or(port.span, |n| n.span));
            let message = format!("top module `{module}` has the value input `{name}`, which the host cannot drive");
            self.report(file, Diagnostic::new("E0811", message, span, "the host drives a top's node ports and discrete `in` ports (§23.1 rule 1)"));
        }
        !inputs.is_empty()
    }
}
