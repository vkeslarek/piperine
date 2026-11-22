//! `Probe`: a thing a table column shows (CLI-05 AC 3 to 6). A node's potential, a discrete
//! net's value, or `i(<instance>)`, the flow of the instance's first potential branch. An entry
//! is written in full or relative to the top; one that names nothing is `E0801`, before any
//! analysis runs.

use piperine_ir::{BranchKind, Ir, ValueType};
use piperine_solver::prelude::{AnalogVariable, BranchIdentifier, CircuitInstance, NodeIdentifier};

use crate::composite::CellShape;
use crate::failure::Failure;
use crate::table::Cell;
use crate::held::Held;

/// A column's signal, with its full path and what the solver knows it by.
#[derive(Debug, Clone, PartialEq)]
pub enum Probe {
    /// The potential of the node at `path`.
    Node { path: String, id: NodeIdentifier },
    /// The value of the discrete net at `path`, of type `ty`.
    Net { path: String, ty: ValueType },
    /// The value of the discrete net at `path` when it holds an enum, a tuple, an array, a struct or an
    /// `Option`, read by `shape`; with `leaf`, the one leaf of it the entry named (`Top.st.n`,
    /// `Top.v[2]`), which a column shows alone.
    Shaped { path: String, shape: CellShape, leaf: Option<String> },
    /// The flow of the instance at `instance`: its first potential branch's.
    Flow { instance: String, branch: BranchIdentifier },
    /// The value of the param `path` (`<instance>.<param>`), of type `ty`, as the run uses it:
    /// the elaborated value, or the one a `set` gave. Every row shows the same value.
    Param { path: String, ty: ValueType },
    /// The one flow contribution `term` of the instance `instance`, which has no potential
    /// branch: its value at each solution.
    Term { instance: String, term: String },
}

/// Where a probe's value sits in a run, found before the run starts.
#[derive(Debug, Clone, PartialEq)]
pub enum Column {
    /// The index of a node or a flow in the analog solution.
    Unknown { index: usize },
    /// A discrete net, by its id in the digital plan, with its type.
    Net { id: u32, ty: ValueType },
    /// A discrete net holding a composite or an enum, by its id in the digital plan and its path (the
    /// net's own, whatever leaf the column shows), and how to read it.
    Shaped { id: u32, path: String, shape: CellShape, leaf: Option<String> },
    /// A param, by the id of its net in the digital plan, with its type: the value only.
    Param { id: u32, ty: ValueType },
    /// A param no net holds, read from the host's table over the run.
    Held { held: Held },
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
                // no potential contribution: the instance's single flow contribution is the flow
                let flows: Vec<usize> = instance.body().branches().iter().enumerate().filter(|(_, b)| b.kind == BranchKind::Flow).map(|(i, _)| i).collect();
                return match flows.as_slice() {
                    [only] => Ok(Probe::Term { instance: instance.path().to_string(), term: format!("b{only}.flow") }),
                    [] => Err(Probe::nothing(entry, "the instance has no potential branch and no flow contribution, so no flow to print")),
                    several => {
                        let named: Vec<String> = several.iter().map(|i| format!("b{i}.flow")).collect();
                        Err(Probe::nothing(entry, &format!("the instance `{}` has several flow contributions and no potential one ({}): name one by its own node", instance.path(), named.join(", "))))
                    }
                };
            };
            let path = instance.path().to_string();
            return Ok(Probe::Flow { branch: BranchIdentifier::new(path.clone(), format!("b{branch}.flow")), instance: path });
        }
        for path in full(entry) {
            if let Some(node) = ir.node_at(&path) {
                return Ok(Probe::Node { id: NodeIdentifier::Anonymous(node.id().index()), path });
            }
            if let Some(net) = ir.net_at(&path) {
                return Ok(Probe::of_net(ir, net.ty(), path, None));
            }
            if let Some((net, leaf)) = Probe::leaf_of(ir, &path) {
                return Ok(Probe::of_net(ir, net.ty(), net.path().to_string(), Some(leaf)));
            }
            if ir.is_ground_alias(&path) {
                let why = "a fixed zero-potential branch joins it to ground, so it has no unknown of its own: its potential is 0";
                return Err(Failure::Host { code: "E0801", message: format!("`{entry}` is a node the design merges with ground: {why}") });
            }
        }
        if let Some((instance, param)) = entry.rsplit_once('.') {
            for path in full(instance) {
                let Some(found) = ir.instance_at(&path) else { continue };
                if let Some(declared) = found.body().params().iter().find(|p| p.name == param) {
                    return Ok(Probe::Param { path: format!("{}.{param}", found.path()), ty: declared.ty.clone() });
                }
            }
        }
        Err(Probe::nothing(entry, "no node, net, param or `i(<instance>)` of that path"))
    }

    /// The probe of the discrete net `path` of type `ty`: a plain one for a scalar, a shaped one for an
    /// enum, a tuple, an array, a struct or an `Option`, which reads its parts and an enum's variant name.
    fn of_net(ir: &Ir, ty: &ValueType, path: String, leaf: Option<String>) -> Probe {
        let scalar = matches!(ty, ValueType::Bool | ValueType::F64 | ValueType::UInt(_) | ValueType::Int(_));
        match CellShape::of(ir, ty) {
            Some(shape) if !scalar => Probe::Shaped { path, shape, leaf },
            _ => Probe::Net { ty: ir.carried(ty), path },
        }
    }

    /// The net a path names with a leaf after it: `Top.st.n` is the leaf `n` of the net `Top.st`,
    /// `Top.v[2]` the leaf `[2]` of `Top.v`. The longest net path wins.
    fn leaf_of<'ir>(ir: &'ir Ir, entry: &str) -> Option<(piperine_ir::Net<'ir>, String)> {
        let (base, tail) = match entry.find('[') {
            Some(at) => entry.split_at(at),
            None => (entry, ""),
        };
        let segments: Vec<&str> = base.split('.').collect();
        for split in (1..=segments.len()).rev() {
            let path = segments[..split].join(".");
            let dotted = segments[split..].join(".");
            let leaf = match (dotted.is_empty(), tail.is_empty()) {
                (true, true) => continue,
                (true, false) => tail.to_string(),
                (false, _) => format!("{dotted}{tail}"),
            };
            if let Some(net) = ir.net_at(&path) {
                return Some((net, leaf));
            }
        }
        None
    }

    /// The value a shaped probe shows from the bytes of its net: the whole value, or the one leaf.
    pub(crate) fn shown(shape: &CellShape, leaf: Option<&str>, bytes: &[u8]) -> Result<Cell, Failure> {
        let value = shape.decode(bytes)?;
        match (leaf, value) {
            (None, value) => Ok(value),
            // a leaf of a variant the value does not hold, or the `value` of an absent `Option`, shows an empty text at this row
            (Some(leaf), Cell::Composite(whole)) => match whole.leaf(leaf) {
                Some(cell) => Ok(cell),
                None if shape.can_hold_leaf(leaf) => Ok(Cell::Text(String::new())),
                None => Err(Failure::Backend(format!("the value has no leaf `{leaf}`"))),
            },
            (Some(leaf), other) => (leaf.is_empty()).then_some(other).ok_or_else(|| Failure::Backend(format!("the value has no leaf `{leaf}`"))),
        }
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
            Probe::Node { path, .. } | Probe::Net { path, .. } | Probe::Param { path, .. } => path.clone(),
            Probe::Shaped { path, leaf: None, .. } => path.clone(),
            Probe::Shaped { path, leaf: Some(leaf), .. } => if leaf.starts_with('[') { format!("{path}{leaf}") } else { format!("{path}.{leaf}") },
            Probe::Term { instance, .. } => format!("i({instance})"),
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
            Probe::Term { instance, .. } => Err(Failure::Host {
                code: "E0801",
                message: format!("`i({instance})` is a flow term: only an `op` or a `dc` run prints it, at each solution; a step of a transient does not carry it"),
            }),
            Probe::Shaped { path, shape, leaf } => {
                let bound = circuit.bound().ok_or_else(|| none("the circuit binds no design"))?;
                let at = bound.digital.plan().nets.iter().position(|n| n.name == *path).ok_or_else(|| none("no net of the digital plan has that path"))? as u32;
                Ok(Column::Shaped { id: at, path: path.clone(), shape: shape.clone(), leaf: leaf.clone() })
            }
            Probe::Net { path, ty } | Probe::Param { path, ty } => {
                let bound = circuit.bound().ok_or_else(|| none("the circuit binds no design"))?;
                let at = bound.digital.plan().nets.iter().position(|n| n.name == *path).ok_or_else(|| none("no net of the digital plan has that path"))? as u32;
                Ok(if matches!(self, Probe::Param { .. }) { Column::Param { id: at, ty: ty.clone() } } else { Column::Net { id: at, ty: ty.clone() } })
            }
        }
    }

    /// [`Probe::column`], a param the circuit has no net for read from `held` (HII-09 AC 1).
    pub fn column_with(&self, circuit: &CircuitInstance, held: &[Held]) -> Result<Column, Failure> {
        match self {
            Probe::Param { path, .. } => match held.iter().find(|h| h.path == *path) {
                Some(found) => Ok(Column::Held { held: found.clone() }),
                None => self.column(circuit),
            },
            _ => self.column(circuit),
        }
    }

    fn unknown(circuit: &CircuitInstance, variable: AnalogVariable) -> Option<Column> {
        circuit.netlist().reference_for(&variable)?.idx().map(|index| Column::Unknown { index })
    }

    fn nothing(entry: &str, why: &str) -> Failure {
        Failure::Host { code: "E0801", message: format!("`{entry}` names nothing in the design: {why}") }
    }
}
