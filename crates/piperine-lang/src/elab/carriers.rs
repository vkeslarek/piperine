//! Discrete carriers (part II §5): each discrete port and `signal` of a module is one carrier of
//! its body, ports first and then signals, in declaration order, with its value type, its role
//! and its folded initial value (DIG-09 AC 1).
//!
//! A body's carriers, drives, events and reactions are added by builder methods on its id
//! (contract D3), so they are gathered in a [`Structure`] while the body is lowered and added
//! once the body exists. Names are read against a [`Frame`]: the module whose declarations an
//! expression names. Frame 0 is the body's own module.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{AggregateData, BodyId, BodyNode, Carrier, CarrierRole, ChildValue, Constant, Derivative, Drive, Event, Hoisted, Reach, Reaction, Stream, Terminal, ValueType};

use super::body::NodePlan;
use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::pom::{DeclId, DeclKind, Dir, ExprId};
use crate::types::{Length, Nominal, Ty};

/// What a body holds besides its analog block, in the order the IR numbers it.
#[derive(Default)]
pub(super) struct Structure {
    /// The nodes of the body in the order the IR numbers them: its own ports, then its own signals,
    /// then the signals of the hoisted frames (`hoist.rs`), each with the plan placement reads.
    pub(super) nodes: Vec<(BodyNode, NodePlan)>,
    /// The windows of arrays of carriers each child binds: (child index, one entry per carrier port).
    pub(super) child_windows: Vec<(u32, Vec<Option<piperine_ir::CarrierWindow>>)>,
    /// The arrays of nodes of the body, in declaration order (`carrier_arrays.rs`).
    pub(super) node_arrays: Vec<piperine_ir::NodeArray>,
    /// The hoisted frames that add a node or an `analog` statement to the body (`BodyData::hoisted`).
    pub(super) hoisted: Vec<Hoisted>,
    pub(super) carriers: Vec<Carrier>,
    pub(super) drives: Vec<Drive>,
    pub(super) events: Vec<Event>,
    pub(super) reactions: Vec<Reaction>,
    pub(super) streams: Vec<Stream>,
    /// The derivative sites of the body, in the order their `Expr::Derivative` reads number them.
    pub(super) derivatives: Vec<Derivative>,
    /// The carriers each child binds at its carrier ports: (child index, one entry per port).
    pub(super) child_carriers: Vec<(u32, Vec<Option<u32>>)>,
    /// The param values of children that read the parent's params: (child, param, value).
    pub(super) child_params: Vec<(u32, u32, ChildValue)>,
    /// The `pub` nodes and carriers of children the body reads (`reach.rs`).
    pub(super) reaches: Vec<Reach>,
    /// The `p.inflow()` ammeters of the body (`inflow.rs`).
    pub(super) inflows: Vec<piperine_ir::Inflow>,
}

/// The module whose names an expression of the body reads, and what those names are in the body.
/// Frame 0 is the body's own module; every other is a placement of a module with a result,
/// hoisted into the body (`hoist.rs`).
pub(super) struct Frame {
    pub(super) module: DeclId,
    /// The frame of the placement's caller; `None` for the body's own module.
    pub(super) parent: Option<usize>,
    /// The placement (the call) that opened this frame; `None` for the body's own module and for
    /// a reach frame.
    pub(super) call: Option<ExprId>,
    /// What the frame's carriers are named with in the body: empty for the body's own module,
    /// `gate.` for `let gate = transition(..)`.
    pub(super) prefix: String,
    /// The carrier of the body each discrete port or signal declaration of `module` is.
    pub(super) carriers: BTreeMap<DeclId, u32>,
    /// The terminal of the body each node port and node `signal` declaration of `module` is: frame
    /// 0's are the body's own nodes; a hoisted frame's node port is the caller's terminal for its
    /// argument, and its node `signal` a node the body gained (`hoist.rs`).
    pub(super) nodes: BTreeMap<DeclId, Terminal>,
    /// The shape of each array of nodes of `nodes`: its first terminal is element 0 (`carrier_arrays.rs`).
    pub(super) node_shapes: BTreeMap<DeclId, super::carrier_arrays::ArrayShape>,
    /// A hoisted module's params, bound to constants by the placement or their defaults.
    pub(super) params: BTreeMap<DeclId, Constant>,
    /// A hoisted module's value inputs: the caller's expression, read in the parent frame, or
    /// `None` for a trailing `Option` input left out.
    pub(super) inputs: BTreeMap<DeclId, Option<ExprId>>,
    /// The frames of the placements of modules with a result that `module` writes, by call.
    pub(super) placements: BTreeMap<ExprId, usize>,
    /// The placements whose hoisting failed, already reported.
    pub(super) failed: BTreeSet<ExprId>,
    /// The derivative site of each `ddt` call read in the frame (`derivatives.rs`): one call, one site.
    pub(super) derivatives: BTreeMap<ExprId, u32>,
    /// For a reach frame (`reach.rs`), the index of the child it stands for in the body's children.
    pub(super) child: Option<u32>,
    /// The loop variables of the unrolled analog loops the placement sat in when it was hoisted,
    /// each a constant: what the caller's expressions of its inputs read (`unroll.rs`).
    pub(super) unrolled: Bindings,
}

impl Frame {
    /// The frame of the body's own module.
    pub(super) fn root(module: DeclId) -> Frame {
        Frame { module, parent: None, call: None, prefix: String::new(), carriers: BTreeMap::new(), nodes: BTreeMap::new(), node_shapes: BTreeMap::new(), params: BTreeMap::new(), inputs: BTreeMap::new(), placements: BTreeMap::new(), failed: BTreeSet::new(), derivatives: BTreeMap::new(), child: None, unrolled: Bindings::new() }
    }
}

impl Elaboration<'_, '_> {
    /// The discrete ports, then the discrete signals, of `module`, as carriers of the frame
    /// `frame`: each added to the structure and named in the frame.
    pub(super) fn lower_carriers(&mut self, frame: usize, module: DeclId) {
        let children = self.design.decl_data(module).children.clone();
        let discrete = |this: &Self, c: &DeclId| this.types.carrier(*c).is_some() && this.types.node_array(*c).is_none();
        let ports = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_)) && discrete(self, c));
        let signals = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Signal(_)) && discrete(self, c));
        for decl in ports.chain(signals).collect::<Vec<_>>() {
            if let Some(carrier) = self.lower_carrier(frame, decl) {
                let index = self.add_carrier(carrier);
                self.frames[frame].carriers.insert(decl, index);
            }
        }
    }

    /// Append `carrier` to the body's carriers: its index.
    pub(super) fn add_carrier(&mut self, carrier: Carrier) -> u32 {
        self.structure.carriers.push(carrier);
        self.structure.carriers.len() as u32 - 1
    }

    /// The carried value type of a carrier declaration; `None` after a diagnostic.
    pub(super) fn carried_type(&mut self, decl: DeclId) -> Option<ValueType> {
        let data = self.design.decl_data(decl);
        let (file, span) = (data.file, data.span);
        let carried = match self.types.carrier(decl) {
            Some(Ty::Carrier { value, .. }) => self.stored_type(value),
            // an array of carriers holds its elements' value type (`carrier_arrays.rs`)
            Some(array @ Ty::Array(..)) => match array.innermost() {
                Ty::Carrier { value, .. } => self.stored_type(value),
                _ => None,
            },
            _ => None,
        };
        if carried.is_none() {
            self.unsupported(file, span, "a carrier whose value type has no value in the IR");
        }
        carried
    }

    fn lower_carrier(&mut self, frame: usize, decl: DeclId) -> Option<Carrier> {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let (role, init) = match &data.kind {
            // A hoisted module's carriers are signals of the enclosing body: its `in` value input
            // is driven by the caller's expression (`hoist.rs`).
            DeclKind::Port(_) if frame != 0 => (CarrierRole::Signal, None),
            DeclKind::Port(p) => (if p.dir == Some(Dir::Out) { CarrierRole::Out } else { CarrierRole::In }, p.default),
            DeclKind::Signal(s) => (CarrierRole::Signal, s.init),
            _ => return None,
        };
        let ty = self.carried_type(decl)?;
        let array = match self.types.carrier_array(decl).is_some() {
            true => Some(self.carrier_array_shape(decl)?.0),
            false => None,
        };
        let init = match init {
            Some(expr) if array.is_some() => {
                let shape = array.clone()?;
                Some(self.array_init(decl, expr, &shape, &ty, frame)?)
            }
            Some(expr) => match self.hand_written_encode(decl) {
                Some(encode) => Some(self.encoded_init(expr, encode, frame)?),
                None => {
                    let value = self.carrier_init(expr, frame)?;
                    Some(self.ir_constant(value, &ty))
                }
            },
            // the levels of a `Default` need `encode` run on a value no expression writes: refused by name until phase C
            // (GAPS L69). A type with no `Default` has none to start at: the carrier takes its value from what drives it
            None if self.hand_written_encode(decl).is_some() && (frame == 0 || matches!(data.kind, DeclKind::Signal(_))) => {
                if self.default_initial_value(decl, &ty).is_some() {
                    self.unsupported(data.file, data.span, "a carrier of a type with a hand-written `Repr` and no initial value");
                    return None;
                }
                None
            }
            None if frame == 0 || matches!(data.kind, DeclKind::Signal(_)) => self.default_initial_value(decl, &ty),
            None => None,
        };
        let name = format!("{}{name}", self.frames[frame].prefix);
        Some(Carrier { name, ty, role, init, origin: self.locator.decl(decl), count: array.map(|shape| shape.count()) })
    }

    /// The initial value of a carrier, folded: a constant expression over literals, `const`s and the
    /// params of its frame (§5.2.3, §5.1.5): a param it reads is a key param of the body, so each
    /// variant folds its own value (`structural.rs`). `None` is a diagnostic.
    pub(super) fn carrier_init(&mut self, expr: ExprId, frame: usize) -> Option<Constant> {
        let bindings = self.frame_bindings(frame);
        // a composite folds to a constant of the IR's arena (`consts/imported.rs`)
        let folded = if self.typed(expr).is_some_and(|ty| ty.is_composite()) { self.composite_constant(expr, &bindings) } else { self.consts.value(expr, &bindings) };
        match folded {
            Ok(value) => Some(value),
            Err(fault) => {
                self.const_fault(&fault, "a carrier's initial value");
                None
            }
        }
    }

    /// What carrier `decl` of value type `stored` holds at `(0, 0)` when no initial value is written:
    /// the `Default::default()` of its type (§3.3.4), when it has one. A type without `Default` (a
    /// `bool`, a number, a `str`) has none here: the checker asks for the written value.
    fn default_initial_value(&mut self, decl: DeclId, stored: &ValueType) -> Option<Constant> {
        let Some(Ty::Carrier { value, .. }) = self.types.carrier(decl).cloned() else { return None };
        self.default_constant(&value, stored)
    }

    /// The constant `Default::default()` of the type `ty`, held as `stored`: `None` (the zero bytes)
    /// for an `Option`, the defaults of the parts of a tuple, an array or a struct that derives
    /// `Default` (H.8 rule 4). `None` for a type that has no default.
    pub(super) fn default_constant(&mut self, ty: &Ty, stored: &ValueType) -> Option<Constant> {
        let (variant, parts) = match (ty, stored) {
            (Ty::Option(_), ValueType::Option(_)) => (None, Vec::new()),
            (Ty::Tuple(items), ValueType::Tuple(held)) => (None, items.iter().zip(held).map(|(item, held)| self.default_constant(item, held)).collect::<Option<Vec<_>>>()?),
            (Ty::Array(element, Length::Fixed(length)), ValueType::Array(held, _)) => {
                let one = self.default_constant(element, held)?;
                (None, vec![one; usize::try_from(*length).ok()?])
            }
            // a type with a hand-written `impl Default` starts at what its `default` returns (`Logic<N>`: every level X)
            (Ty::Struct(nominal), _) if self.impls.has_written_default(ty) => return self.written_default(nominal, stored),
            (Ty::Struct(Nominal { decl: struct_decl, .. }), ValueType::Struct(id)) if self.types.derives(*struct_decl, "Default") => {
                let fields: Vec<Ty> = self.types.fields(*struct_decl)?.iter().map(|field| field.ty.clone()).collect();
                let held = self.type_arena.field_types(*id).to_vec();
                (None, fields.iter().zip(&held).map(|(field, held)| self.default_constant(field, held)).collect::<Option<Vec<_>>>()?)
            }
            _ => return None,
        };
        Some(Constant::Aggregate(self.ir.aggregate(AggregateData { ty: stored.clone(), variant, parts })))
    }

    /// The constant a hand-written `impl Default` gives the struct `nominal`, held as `stored`: the tail of its `default`,
    /// folded for the arguments of the `impl` (`N` of a `Logic<N>`). `None` when the body is no tail expression that folds.
    fn written_default(&mut self, nominal: &Nominal, stored: &ValueType) -> Option<Constant> {
        let function = *self.impls.builtin_functions("Default", "default", &Ty::Struct(nominal.clone())).first()?;
        let DeclKind::Fn(data) = &self.design.decl_data(function).kind else { return None };
        let body = data.body.clone()?;
        let Some(crate::pom::StmtKind::Expr { expr, semi: false }) = body.stmts.last().map(|stmt| self.design.stmt_node(*stmt).kind.clone()) else { return None };
        let arguments: Vec<super::instances::ConcreteValue> = nominal
            .arguments
            .iter()
            .map(|argument| match argument {
                crate::types::GenericValue::Const(crate::types::ConstTerm::Value(value)) => Some(super::instances::ConcreteValue::Const(*value)),
                crate::types::GenericValue::Type(ty) => Some(super::instances::ConcreteValue::Type(ty.clone())),
                _ => None,
            })
            .collect::<Option<_>>()?;
        let outer = self.consts.set_substitution(self.instance_substitution(function, &arguments));
        let folded = self.composite_constant(expr, &Bindings::new());
        self.consts.set_substitution(outer);
        let value = folded.ok()?;
        Some(self.ir_constant(value, stored))
    }

    /// Add the gathered structure of the body `body` to the IR, in the order it was numbered.
    pub(super) fn finish_structure(&mut self, body: BodyId) {
        let structure = std::mem::take(&mut self.structure);
        for hoisted in structure.hoisted {
            self.ir.hoisted(body, hoisted);
        }
        for carrier in structure.carriers {
            self.ir.carrier(body, carrier);
        }
        for array in structure.node_arrays {
            self.ir.node_array(body, array);
        }
        for drive in structure.drives {
            self.ir.drive(body, drive);
        }
        for event in structure.events {
            self.ir.event(body, event);
        }
        for reaction in structure.reactions {
            self.ir.reaction(body, reaction);
        }
        for stream in structure.streams {
            self.ir.stream(body, stream);
        }
        for derivative in structure.derivatives {
            self.ir.derivative(body, derivative);
        }
        for (child, carriers) in structure.child_carriers {
            self.ir.bind_child_carriers(body, child, carriers);
        }
        for (child, windows) in structure.child_windows {
            self.ir.bind_child_windows(body, child, windows);
        }
        for (child, param, value) in structure.child_params {
            self.ir.child_param(body, child, param, value);
        }
        for reach in structure.reaches {
            self.ir.reach(body, reach);
        }
        for inflow in structure.inflows {
            self.ir.inflow(body, inflow);
        }
    }
}
