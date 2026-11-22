//! `Metadata`: what a design declares, read from the IR and the session with no analysis
//! (HII-10): every param of every instance with its type, value, `where` text and whether it is
//! structural; the top's ports with their direction and type; every record stream with its
//! fields. The value of a param is the one the run uses (a `set` of the request is applied).

use piperine_ir::{CarrierRole, Constant, Ir, NodeRole};

use crate::params::Params;
use crate::table::Cell;

/// A param of an instance of the design.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamInfo {
    /// The instance's full path and the param's name: `Top.r1.r`.
    pub path: String,
    pub name: String,
    /// The type as the language writes it: `f64`, `bool`, `u8`.
    pub ty: String,
    /// The value the run uses: the elaborated one, the one a `set` of the request gave, or the one
    /// a placement derives from it.
    pub value: Cell,
    /// The text of the `where` clause; `None` for a param with none.
    pub check: Option<String>,
    /// Whether a write of the param re-creates the instance (a structural param).
    pub structural: bool,
}

/// A port of the top: a discrete `in` or `out` of a type, or an analog port of a nature.
#[derive(Debug, Clone, PartialEq)]
pub struct PortInfo {
    pub name: String,
    /// `in` or `out` for a discrete port, `inout` for an analog one.
    pub direction: &'static str,
    /// The type of a discrete port, the nature of an analog one.
    pub ty: String,
}

/// A record stream of an instance: its pin path and its value fields with their types.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamInfo {
    pub path: String,
    pub fields: Vec<(String, String)>,
}

/// What a session's design declares.
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    pub params: Vec<ParamInfo>,
    pub ports: Vec<PortInfo>,
    pub streams: Vec<StreamInfo>,
}

impl Metadata {
    /// The metadata of `ir`, each param shown with its value in `held`, the table a run starts
    /// from: the elaborated value, a `set`'s, or what a placement derives from a `set` param.
    pub(crate) fn of(ir: &Ir, held: &Params) -> Metadata {
        let mut params = Vec::new();
        for instance in ir.instances() {
            let body = instance.body();
            for (i, param) in body.params().iter().enumerate() {
                let path = format!("{}.{}", instance.path(), param.name);
                let current = held.get(instance.path(), &param.name);
                let value = match (instance.param(i), current) {
                    (Some(Constant::Bool(_)), Some(now)) => Cell::Bool(now != 0.0),
                    (Some(Constant::Int(_)), Some(now)) => Cell::Int(now as i128),
                    (Some(_), Some(now)) => Cell::Real(now),
                    _ => Cell::Unknown,
                };
                params.push(ParamInfo {
                    name: param.name.clone(),
                    ty: ir.written(&param.ty),
                    value,
                    check: body.param_check_text(i),
                    structural: param.structural,
                    path,
                });
            }
        }
        let top = ir.top();
        let body = top.body();
        let mut declared: Vec<(&piperine_ir::Origin, PortInfo)> = Vec::new();
        for node in body.nodes().iter().filter(|n| n.role == NodeRole::Port) {
            declared.push((&node.origin, PortInfo { name: node.name.clone(), direction: "inout", ty: ir.nature(node.nature).name().to_string() }));
        }
        for carrier in body.carriers().iter().filter(|c| c.role != CarrierRole::Signal) {
            let direction = if carrier.role == CarrierRole::In { "in" } else { "out" };
            declared.push((&carrier.origin, PortInfo { name: carrier.name.clone(), direction, ty: carrier.ty.to_string() }));
        }
        declared.sort_by_key(|(origin, _)| (origin.file.clone(), origin.line, origin.col));
        let ports = declared.into_iter().map(|(_, port)| port).collect();
        let mut streams = Vec::new();
        for instance in ir.instances() {
            for stream in instance.body().streams() {
                let fields = stream.fields.iter().map(|f| (f.name.clone(), ir.written(&f.ty))).collect();
                streams.push(StreamInfo { path: format!("{}.{}", instance.path(), stream.name), fields });
            }
        }
        Metadata { params, ports, streams }
    }
}
