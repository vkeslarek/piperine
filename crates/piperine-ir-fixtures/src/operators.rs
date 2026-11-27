//! The body `Op(a)` the analog-operator tests of `piperine-ir`, `piperine-plan`, `piperine-interp` and
//! `piperine-transpile` build by hand: one flow branch `(a, gnd)`, the param `td`, and the sites a test writes
//! (delays, noise sources, session parameters, trace sites).

use crate::devices::{Devices, Ex};
use piperine_ir::{
    Addend, BodyNode, BodySpec, Branch, BranchKind, Carrier, Constant, DelaySite, EmitField, Event, Expr, ExprId, InstanceSpec, Ir, IrBuilder, NetSpec, NodeRef, NodeRole, NoiseSource, Operator, Origin, Param, QuantityRole, Reaction, Real,
    SessionParameter, Stmt, StmtId, Stream, StreamField, Terminal, TraceSite, ValueType,
};

/// What a test writes into the body `Operators` builds.
#[derive(Default)]
pub struct OperatorWrites {
    /// The statements of the `analog` block.
    pub analog: Vec<StmtId>,
    /// The delay sites of the body.
    pub delays: Vec<DelaySite>,
    /// The noise sources of the body.
    pub noise_sources: Vec<NoiseSource>,
    /// The session parameters of the body.
    pub session_parameters: Vec<SessionParameter>,
    /// The streams of the body.
    pub streams: Vec<Stream>,
    /// The trace sites of the body.
    pub traces: Vec<TraceSite>,
    /// The carriers of the body.
    pub carriers: Vec<Carrier>,
    /// The events of the body.
    pub events: Vec<Event>,
    /// The reactions of the body.
    pub reactions: Vec<Reaction>,
}

/// The one body `Op(a)` with the flow branch `(a, gnd)` and the param `td: f64` (index 0), whose
/// `analog` block the test writes; placed alone as the design.
pub struct Operators;

impl Operators {
    /// The file every node of `Operators` is attributed to.
    pub const FILE: &'static str = "operators.msl";

    /// The origin every form of the fixture shares.
    pub fn at() -> Origin {
        Origin { file: Self::FILE.into(), line: 3, col: 5 }
    }

    /// The expression helper at the fixed position.
    pub fn ex(b: &mut IrBuilder) -> Ex<'_> {
        Ex::new(b, Self::FILE, 3, 5)
    }

    /// `i(a, gnd) +> value;`
    pub fn contribute(b: &mut IrBuilder, value: ExprId) -> StmtId {
        b.stmt(Stmt::Contribute { branch: 0, quantity: QuantityRole::Flow, addends: vec![Addend { operator: Operator::Direct, negated: false, value }] })
    }

    /// The design `Op` with everything `write` returns.
    pub fn ir(write: impl FnOnce(&mut IrBuilder) -> OperatorWrites) -> Ir {
        let mut b = IrBuilder::new();
        let el = Devices::electrical(&mut b);
        let origin = Origin { file: Self::FILE.into(), line: 1, col: 1 };
        let OperatorWrites { analog, delays, noise_sources, session_parameters, streams, traces, carriers, events, reactions } = write(&mut b);
        let delay_time = Param { name: "td".into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1e-9))), check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec {
            name: "Op".into(),
            origin: origin.clone(),
            params: vec![delay_time],
            nodes: vec![BodyNode { name: "a".into(), nature: el, role: NodeRole::Port, init: None, origin: origin.clone() }],
            branches: vec![Branch { plus: Terminal::Node(0), minus: Terminal::Ground, kind: BranchKind::Flow }],
            limits: vec![],
            stims: vec![],
            locals: vec![],
            analog: Some(analog),
            children: vec![],
        });
        for site in delays {
            b.delay(body, site);
        }
        for source in noise_sources {
            b.noise_source(body, source);
        }
        for parameter in session_parameters {
            b.session_parameter(body, parameter);
        }
        for stream in streams {
            b.stream(body, stream);
        }
        for trace in traces {
            b.trace(body, trace);
        }
        let top = b.next_instance_id();
        let nets: Vec<_> = carriers.iter().map(|c| NetSpec { path: format!("Op.{}", c.name), ty: c.ty.clone(), declared_by: top, init: None, origin: origin.clone() }).collect();
        for carrier in carriers {
            b.carrier(body, carrier);
        }
        for event in events {
            b.event(body, event);
        }
        for reaction in reactions {
            b.reaction(body, reaction);
        }
        let nets: Vec<_> = nets.into_iter().map(|net| b.net(net)).collect();
        let params = vec![Constant::Real(Real::new(1e-9))];
        let top = b.instance(InstanceSpec { path: "Op".into(), name: "Op".into(), body, parent: None, nodes: vec![NodeRef::Ground], params, origin });
        b.bind_nets(top, nets);
        b.finish(top)
    }

    /// Whether any entry of `broken` says `what`.
    pub fn names(broken: &[piperine_ir::Broken], what: &str) -> bool {
        broken.iter().any(|entry| entry.what.contains(what))
    }


    /// `i(a, gnd) +> 0.5 * delay<k>.value;`
    pub fn contribute_half_the_delay(b: &mut IrBuilder, k: u32) -> StmtId {
        let read = b.expr(Expr::Delay(k), ValueType::F64, Self::at());
        let mut e = Self::ex(b);
        let half = e.real(0.5);
        let value = e.binary(piperine_ir::BinaryOp::Mul, half, read);
        Self::contribute(b, value)
    }

    /// The site `delay(v(a), td, <maximum>)` named `d`.
    pub fn delay_site(b: &mut IrBuilder, maximum: Option<f64>) -> DelaySite {
        let input = Self::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        let time = b.expr(Expr::Param(0), ValueType::F64, Self::at());
        let maximum = maximum.map(|m| Self::ex(b).real(m));
        DelaySite { name: "d".into(), input, time, maximum, origin: Self::at() }
    }
}


/// Noise sources.
impl Operators {
    /// `i(a, gnd) +> noise<k>.value;`
    pub fn contribute_the_noise(b: &mut IrBuilder, k: u32) -> StmtId {
        let read = b.expr(Expr::Noise(k), ValueType::F64, Self::at());
        Self::contribute(b, read)
    }

    /// The source `noise(4e-21, "thermal")` named `n`; `group` is its literal name.
    pub fn noise_site(b: &mut IrBuilder, group: Option<&str>) -> NoiseSource {
        let density = Self::ex(b).real(4e-21);
        NoiseSource { name: "n".into(), group: group.map(str::to_string), density, origin: Self::at() }
    }
}

/// Session parameters.
impl Operators {
    /// `i(a, gnd) +> simparam("gmin").unwrap_or_default();`
    pub fn contribute_the_session_parameter(b: &mut IrBuilder, parameter: u32) -> StmtId {
        let packed = b.expr(Expr::SessionParameter { parameter }, ValueType::Option(Box::new(ValueType::F64)), Self::at());
        let value = b.expr(Expr::OptionPart { option: packed, part: piperine_ir::LevelPart::Value }, ValueType::F64, Self::at());
        Self::contribute(b, value)
    }

    /// The session parameter `gmin`.
    pub fn gmin() -> SessionParameter {
        SessionParameter { name: "gmin".into(), origin: Self::at() }
    }
}

/// Trace sites.
impl Operators {
    /// The stream `newton` with the field `x: f64`.
    pub fn newton() -> Stream {
        Stream { name: "newton".into(), fields: vec![StreamField { name: "x".into(), ty: ValueType::F64 }] }
    }

    /// `emit!("newton", x = v(a), who = "diode")` as the trace of site `site`.
    pub fn trace_the_probe(b: &mut IrBuilder, site: u32) -> StmtId {
        let x = Self::ex(b).probe(Terminal::Node(0), Terminal::Ground);
        b.stmt(Stmt::Trace { site, fields: vec![EmitField::Value { field: 0, value: x }, EmitField::Text { name: "who".into(), text: "diode".into() }] })
    }

    /// The trace site of stream 0.
    pub fn trace_site() -> TraceSite {
        TraceSite { stream: 0, origin: Self::at() }
    }

    /// A design tracing the probe, the trace being in the `analog` block.
    pub fn tracing() -> Ir {
        Self::ir(|b| OperatorWrites { analog: vec![Self::trace_the_probe(b, 0)], streams: vec![Self::newton()], traces: vec![Self::trace_site()], ..OperatorWrites::default() })
    }
}
