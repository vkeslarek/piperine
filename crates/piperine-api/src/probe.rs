//! `Probe`: a thing a table column shows (CLI-05 AC 3 to 6). A node's potential, a discrete
//! net's value, or `i(<instance>)`, the flow of the instance's first potential branch. An entry
//! is written in full or relative to the top; one that names nothing is `E0801`, before any
//! analysis runs.

use piperine_ir::{BranchKind, Ir, ValueType};
use piperine_solver::prelude::{AnalogVariable, BranchIdentifier, CircuitInstance, NodeIdentifier};

use crate::failure::Failure;

/// A column's signal, with its full path and what the solver knows it by.
#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    /// The potential of the node at `path`.
    Node { path: String, id: NodeIdentifier },
    /// The value of the discrete net at `path`, of type `ty`.
    Net { path: String, ty: ValueType },
    /// The flow of the instance at `instance`: its first potential branch's.
    Flow { instance: String, branch: BranchIdentifier },
}

/// Where a probe's value sits in a run, found before the run starts.
#[derive(Debug, Clone, PartialEq)]
pub enum Column {
    /// The index of a node or a flow in the analog solution.
    Unknown { index: usize },
    /// A discrete net, by its id in the digital plan, with its type.
    Net { id: u32, ty: ValueType },
}

impl Probe {
    /// The probe `entry` names: `i(<instance>)` or a node or net path, each in full or relative
    /// to `top`; `E0801` when it names nothing of `ir`.
    pub fn resolve(ir: &Ir, top: &str, entry: &str) -> Result<Probe, Failure> {
        let full = |path: &str| [path.to_string(), format!("{top}.{path}")];
        if let Some(inner) = entry.strip_prefix("i(").and_then(|rest| rest.strip_suffix(')')) {
            let instance = full(inner.trim()).into_iter().find_map(|p| ir.instance_at(&p));
            let Some(instance) = instance else { return Err(Probe::nothing(entry, "no instance of that path")) };
            let Some(branch) = instance.body().branches().iter().position(|b| b.kind == BranchKind::Potential) else {
                return Err(Probe::nothing(entry, "the instance has no potential branch, so no flow to print"));
            };
            let path = instance.path().to_string();
            return Ok(Probe::Flow { branch: BranchIdentifier::new(path.clone(), format!("b{branch}.flow")), instance: path });
        }
        for path in full(entry) {
            if let Some(node) = ir.node_at(&path) {
                return Ok(Probe::Node { id: NodeIdentifier::Anonymous(node.id().index()), path });
            }
            if let Some(net) = ir.net_at(&path) {
                return Ok(Probe::Net { ty: net.ty().clone(), path });
            }
        }
        Err(Probe::nothing(entry, "no node, net or `i(<instance>)` of that path"))
    }

    /// The top's own signals and ports, in declaration order: its nodes and its discrete
    /// carriers, never a child's.
    pub fn defaults(ir: &Ir, top: &str) -> Vec<Probe> {
        let Some(instance) = ir.instance_at(top) else { return Vec::new() };
        let body = instance.body();
        let mut declared: Vec<(&piperine_ir::Origin, &str)> = body.nodes().iter().map(|n| (&n.origin, n.name.as_str())).collect();
        declared.extend(body.carriers().iter().map(|c| (&c.origin, c.name.as_str())));
        declared.sort_by_key(|(o, _)| (o.file.clone(), o.line, o.col));
        declared.into_iter().filter_map(|(_, name)| Probe::resolve(ir, top, name).ok()).collect()
    }

    /// The column's name: the full path, `i(<full instance path>)` for a flow.
    pub fn label(&self) -> String {
        match self {
            Probe::Node { path, .. } | Probe::Net { path, .. } => path.clone(),
            Probe::Flow { instance, .. } => format!("i({instance})"),
        }
    }

    /// Where this probe's value sits in a run of `circuit`: a node or a flow to its index in the
    /// solution, a discrete net to its id in the digital plan. A probe with neither is a
    /// `Failure::Backend` naming its label, before any step.
    pub fn column(&self, circuit: &CircuitInstance) -> Result<Column, Failure> {
        let none = |why: &str| Failure::Backend(format!("`{}` has no value in the circuit: {why}", self.label()));
        match self {
            Probe::Node { id, .. } => Probe::unknown(circuit, AnalogVariable::Node(id.clone())).ok_or_else(|| none("no unknown of the solution is that node")),
            Probe::Flow { branch, .. } => Probe::unknown(circuit, AnalogVariable::Branch(branch.clone())).ok_or_else(|| none("no unknown of the solution is that branch current")),
            Probe::Net { path, ty } => {
                let bound = circuit.bound().ok_or_else(|| none("the circuit binds no design"))?;
                let at = bound.digital.plan().nets.iter().position(|n| n.name == *path).ok_or_else(|| none("no net of the digital plan has that path"))?;
                Ok(Column::Net { id: at as u32, ty: ty.clone() })
            }
        }
    }

    fn unknown(circuit: &CircuitInstance, variable: AnalogVariable) -> Option<Column> {
        circuit.netlist().reference_for(&variable)?.idx().map(|index| Column::Unknown { index })
    }

    fn nothing(entry: &str, why: &str) -> Failure {
        Failure::Host { code: "E0801", message: format!("`{entry}` names nothing in the design: {why}") }
    }
}
