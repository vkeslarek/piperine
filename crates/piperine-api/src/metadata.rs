//! `Metadata`: what a design declares, read from the IR and the session with no analysis
//! (HII-10): every param of every instance with its type, value, `where` text and whether it is
//! structural; the top's ports with their direction and type; every record stream with its
//! fields. The value of a param is the one the run uses (a `set` of the request is applied).

use piperine_ir::{CarrierRole, Constant, Ir, NodeRole};

use crate::composite::CellShape;
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
                let composite = if param.ty.is_composite() {
                    CellShape::of(ir, &param.ty).zip(held.composite_bytes(instance.path(), &param.name)).and_then(|(shape, bytes)| shape.decode(bytes).ok())
                } else {
                    None
                };
                let value = match (composite, instance.param(i), current) {
                    (Some(value), _, _) => value,
                    (None, Some(Constant::Bool(_)), Some(now)) => Cell::Bool(now != 0.0),
                    (None, Some(Constant::Int(_)), Some(now)) => Cell::Int(now as i128),
                    (None, Some(_), Some(now)) => Cell::Real(now),
                    // a param with no default holds no value until a placement gives it one: the absent cell
                    (None, _, _) => Cell::Absent,
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

#[cfg(test)]
mod tests {
    use super::*;
    use piperine_ir::{BodySpec, InstanceSpec, IrBuilder, Origin, Param, ValueType};

    #[test]
    fn a_param_no_placement_gave_a_value_is_the_absent_cell() {
        // part VIII: a param with no value shows the absent cell (an empty field, `null` in JSON), never an empty text
        // standing in for a value; a param that has one shows it
        let origin = Origin { file: "top.msl".into(), line: 1, col: 1 };
        let mut b = IrBuilder::new();
        let param = |name: &str| Param { name: name.into(), ty: ValueType::F64, default: None, check: None, structural: false, origin: origin.clone() };
        let body = b.body(BodySpec { name: "Top".into(), origin: origin.clone(), params: vec![param("given"), param("missing")], nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children: vec![] });
        let top = b.instance(InstanceSpec { path: "Top".into(), name: "Top".into(), body, parent: None, nodes: vec![], params: vec![Constant::Real(piperine_ir::Real::new(2.0))], origin });
        let ir = b.finish(top);
        let metadata = Metadata::of(&ir, &Params::of(&ir));
        let values: Vec<(&str, &Cell)> = metadata.params.iter().map(|p| (p.name.as_str(), &p.value)).collect();
        assert_eq!(values.len(), 2, "{values:?}");
        assert!(values.iter().any(|(name, value)| *name == "missing" && **value == Cell::Absent), "{values:?}");
        assert!(values.iter().all(|(_, value)| **value != Cell::Text(String::new())), "no empty text stands for a value: {values:?}");
    }
}
