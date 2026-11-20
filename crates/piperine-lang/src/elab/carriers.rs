//! Discrete carriers (part II §5): each discrete port and `signal` of a module is one carrier of
//! its body, ports first and then signals, in declaration order, with its value type, its role
//! and its folded initial value (DIG-09 AC 1).
//!
//! A body's carriers, drives, events and reactions are added by builder methods on its id
//! (contract D3), so they are gathered in a [`Structure`] while the body is lowered and added
//! once the body exists. Names are read against a [`Frame`]: the module whose declarations an
//! expression names. Frame 0 is the body's own module.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{BodyId, Carrier, CarrierRole, ChildValue, Constant, Drive, Event, Reaction, Stream, ValueType};

use super::consts::{Bindings, Folded};
use super::elaborator::Elaboration;
use crate::pom::{DeclId, DeclKind, Dir, ExprId};
use crate::types::Ty;

/// What a body holds besides its analog block, in the order the IR numbers it.
#[derive(Default)]
pub(super) struct Structure {
    pub(super) carriers: Vec<Carrier>,
    pub(super) drives: Vec<Drive>,
    pub(super) events: Vec<Event>,
    pub(super) reactions: Vec<Reaction>,
    pub(super) streams: Vec<Stream>,
    /// The carriers each child binds at its carrier ports: (child index, one entry per port).
    pub(super) child_carriers: Vec<(u32, Vec<Option<u32>>)>,
    /// The param values of children that read the parent's params: (child, param, value).
    pub(super) child_params: Vec<(u32, u32, ChildValue)>,
}

/// The module whose names an expression of the body reads, and what those names are in the body.
/// Frame 0 is the body's own module; every other is a placement of a module with a result,
/// hoisted into the body (`hoist.rs`).
pub(super) struct Frame {
    pub(super) module: DeclId,
    /// The frame of the placement's caller; `None` for the body's own module.
    pub(super) parent: Option<usize>,
    /// What the frame's carriers are named with in the body: empty for the body's own module,
    /// `gate.` for `let gate = transition(..)`.
    pub(super) prefix: String,
    /// The carrier of the body each discrete port or signal declaration of `module` is.
    pub(super) carriers: BTreeMap<DeclId, u32>,
    /// A hoisted module's params, bound to constants by the placement or their defaults.
    pub(super) params: BTreeMap<DeclId, Constant>,
    /// A hoisted module's value inputs: the caller's expression, read in the parent frame, or
    /// `None` for a trailing `Option` input left out.
    pub(super) inputs: BTreeMap<DeclId, Option<ExprId>>,
    /// The frames of the placements of modules with a result that `module` writes, by call.
    pub(super) placements: BTreeMap<ExprId, usize>,
    /// The placements whose hoisting failed, already reported.
    pub(super) failed: BTreeSet<ExprId>,
}

impl Frame {
    /// The frame of the body's own module.
    pub(super) fn root(module: DeclId) -> Frame {
        Frame { module, parent: None, prefix: String::new(), carriers: BTreeMap::new(), params: BTreeMap::new(), inputs: BTreeMap::new(), placements: BTreeMap::new(), failed: BTreeSet::new() }
    }
}

impl Elaboration<'_, '_> {
    /// The discrete ports, then the discrete signals, of `module`, as carriers of the frame
    /// `frame`: each added to the structure and named in the frame.
    pub(super) fn lower_carriers(&mut self, frame: usize, module: DeclId) {
        let children = self.design.decl_data(module).children.clone();
        let ports = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_)) && self.types.carrier(*c).is_some());
        let signals = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Signal(_)) && self.types.carrier(*c).is_some());
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
            Some(Ty::Carrier { value, .. }) => self.value_type(value),
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
        let init = match init {
            Some(expr) => self.carrier_init(expr)?,
            None => None,
        };
        let name = format!("{}{name}", self.frames[frame].prefix);
        Some(Carrier { name, ty, role, init, origin: self.locator.decl(decl) })
    }

    /// The initial value of a carrier, folded: a constant expression over literals and `const`s
    /// (§5.2.3). One that reads a param is `P0001` (a value per instance is a host concern). The
    /// outer `None` is a diagnostic; the inner one is an initial value that is `Unknown`, which
    /// is the same as none: every level unknown at `(0, 0)` and no `changed()` there (§3.3).
    fn carrier_init(&mut self, expr: ExprId) -> Option<Option<Constant>> {
        match self.consts.folded(expr, &Bindings::new()) {
            Ok(Folded::Known(value)) => Some(Some(value)),
            Ok(Folded::Unknown) => Some(None),
            Err(fault) => {
                self.const_fault(&fault, "a carrier's initial value");
                None
            }
        }
    }

    /// Add the gathered structure of the body `body` to the IR, in the order it was numbered.
    pub(super) fn finish_structure(&mut self, body: BodyId) {
        let structure = std::mem::take(&mut self.structure);
        for carrier in structure.carriers {
            self.ir.carrier(body, carrier);
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
        for (child, carriers) in structure.child_carriers {
            self.ir.bind_child_carriers(body, child, carriers);
        }
        for (child, param, value) in structure.child_params {
            self.ir.child_param(body, child, param, value);
        }
    }
}
