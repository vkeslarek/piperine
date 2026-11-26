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
    /// The arrays of no element the body declares (`carrier_arrays.rs`).
    pub(super) empty_arrays: Vec<piperine_ir::EmptyArray>,
    /// The hoisted frames that add a node or an `analog` statement to the body (`BodyData::hoisted`).
    pub(super) hoisted: Vec<Hoisted>,
    pub(super) carriers: Vec<Carrier>,
    /// How many of the leading `carriers` are ports of the body's own module (they come first): a resolved
    /// port binds the parent's net like an `in` or `out` one, a resolved signal makes its own.
    pub(super) port_carriers: usize,
    /// The resolved carriers of the body, in carrier order: the index, the declaration and the nature (`resolvers.rs`).
    pub(super) resolved_carriers: Vec<(u32, DeclId, DeclId)>,
    /// The carriers some reaction of the body commits to.
    pub(super) committed: BTreeSet<u32>,
    /// The carriers whose `others()` the body reads.
    pub(super) others_read: BTreeSet<u32>,
    pub(super) drives: Vec<Drive>,
    /// The carriers of the body (with the window of an array of them) that children hold at an `out` port: the part
    /// of each the instance drives (`element_coverage.rs`).
    pub(super) handed: Vec<(u32, Option<piperine_ir::CarrierWindow>)>,
    pub(super) events: Vec<Event>,
    pub(super) reactions: Vec<Reaction>,
    pub(super) streams: Vec<Stream>,
    /// The derivative sites of the body, in the order their `Expr::Derivative` reads number them.
    pub(super) derivatives: Vec<Derivative>,
    /// The `ddx` calls of the body, in source order (`partial.rs`).
    pub(super) partial_derivatives: Vec<piperine_ir::PartialDerivative>,
    /// The delay sites of the body, in the order their `Expr::Delay` reads number them.
    pub(super) delays: Vec<piperine_ir::DelaySite>,
    /// The noise sources of the body, in the order their `Expr::Noise` reads number them.
    pub(super) noise_sources: Vec<piperine_ir::NoiseSource>,
    /// The session parameters the body reads, one per name, in the order they were first read.
    pub(super) session_parameters: Vec<piperine_ir::SessionParameter>,
    /// The trace sites of the body (`emit!` in `analog`), in the order their `Stmt::Trace` numbers them.
    pub(super) traces: Vec<piperine_ir::TraceSite>,
    /// The carriers each child binds at its carrier ports: (child index, one entry per port).
    pub(super) child_carriers: Vec<(u32, Vec<Option<u32>>)>,
    /// The param values of children that read the parent's params: (child, param, value).
    pub(super) child_params: Vec<(u32, u32, ChildValue)>,
    /// The `pub` nodes and carriers of children the body reads (`reach/fields.rs`).
    pub(super) reaches: Vec<Reach>,
    /// The `p.inflow()` ammeters of the body (`inflow.rs`).
    pub(super) inflows: Vec<piperine_ir::Inflow>,
    /// Where each definition, commit and hand-over of a carrier of the body is written, for the overlaps judged per
    /// instance (`drive_overlaps.rs`); no part of the IR.
    pub(super) drive_sites: Vec<super::drive_overlaps::DriveSite>,
    /// The scalar carrier mirroring each element of an array carrier the `analog` block reads, by (carrier, element)
    /// (`elements.rs`).
    pub(super) analog_mirrors: BTreeMap<(u32, u64), u32>,
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
    /// The arrays of carriers declared with length 0 (§1.2.3): no carrier of the body, since no element exists to hold
    /// (`carrier_arrays.rs`). Every element of one is out of range, and read whole it is the empty array.
    pub(super) empty_arrays: BTreeSet<DeclId>,
    /// The params of a hoisted module or of a reach frame's, bound to constants by the placement or their defaults
    /// (`reach/placement_params.rs`).
    pub(super) params: BTreeMap<DeclId, Constant>,
    /// The params of a reach frame's module whose value does not fold (a value over a param of the reader that is not a key): the
    /// expression each is read as, where a `let` of the child reads it (`reach/placement_params.rs`).
    pub(super) deferred_params: BTreeMap<DeclId, super::reach::placement_params::DeferredParam>,
    /// A hoisted module's value inputs: the caller's expression, read in the parent frame, or
    /// `None` for a trailing `Option` input left out.
    pub(super) inputs: BTreeMap<DeclId, Option<ExprId>>,
    /// The frames of the placements of modules with a result that `module` writes, by call and generation iteration.
    pub(super) placements: BTreeMap<super::hoist::PlacementKey, usize>,
    /// The placements whose hoisting failed, already reported.
    pub(super) failed: BTreeSet<super::hoist::PlacementKey>,
    /// The derivative site of each `ddt` call read in the frame (`derivatives.rs`): one call, one site.
    pub(super) derivatives: BTreeMap<ExprId, u32>,
    /// The delay site of each `delay` call read in the frame (`delays.rs`): one call, one site.
    pub(super) delays: BTreeMap<ExprId, u32>,
    /// The noise source of each `noise` call read in the frame (`noise.rs`): one call, one source.
    pub(super) noise_sources: BTreeMap<ExprId, u32>,
    /// For a reach frame (`reach/fields.rs`), the index of the child it stands for in the body's children.
    pub(super) child: Option<u32>,
    /// For a reach frame below another (`reach/fields.rs`), the placement it stands for in the parent reach frame and the ordinals of its
    /// iteration there: a child of a child, reached through the chain of `parent`s up to the frame of a child of the body.
    pub(super) placement: Option<(DeclId, Vec<u64>)>,
    /// The loop variables of the unrolled analog loops the placement sat in when it was hoisted,
    /// each a constant: what the caller's expressions of its inputs read (`unroll.rs`).
    pub(super) unrolled: Bindings,
    /// What the generation of the frame's module unrolls into while the frame is not the one being lowered (parked by `enter_frame`;
    /// `Elaboration::generated` holds the active frame's, `hoist.rs`).
    pub(super) generated: super::generation::GeneratedItems,
    /// The iteration of the generation `for`s the placement sat in when it was hoisted: what the caller's expressions of
    /// its inputs read of the carriers and `let`s declared in those loops (§8.2.23).
    pub(super) ordinals: Vec<(DeclId, u32)>,
    /// What the generic parameters the frame's code names are replaced by: the arguments of the instance of a generic module
    /// the frame is, else those of the body it sits in (`generic_bodies.rs`).
    pub(super) substitution: crate::types::Substitution,
}

impl Frame {
    /// Whether the frame names a placed child (`reach/fields.rs`) rather than the body's own module or a hoisted placement: a child of the
    /// body, or a child of such a child.
    pub(super) fn is_reached(&self) -> bool {
        self.child.is_some() || self.placement.is_some()
    }

    /// The frame of the body's own module.
    pub(super) fn root(module: DeclId) -> Frame {
        Frame { module, parent: None, call: None, prefix: String::new(), carriers: BTreeMap::new(), nodes: BTreeMap::new(), node_shapes: BTreeMap::new(), empty_arrays: BTreeSet::new(), params: BTreeMap::new(), deferred_params: BTreeMap::new(), inputs: BTreeMap::new(), placements: BTreeMap::new(), failed: BTreeSet::new(), derivatives: BTreeMap::new(), delays: BTreeMap::new(), noise_sources: BTreeMap::new(), child: None, placement: None, unrolled: Bindings::new(), generated: super::generation::GeneratedItems::default(), ordinals: Vec::new(), substitution: crate::types::Substitution::new() }
    }
}

impl Elaboration<'_, '_> {
    /// The discrete ports, then the discrete signals, of `module`, as carriers of the frame
    /// `frame`: each added to the structure and named in the frame.
    pub(super) fn lower_carriers(&mut self, frame: usize, module: DeclId) {
        let mut children = self.design.decl_data(module).children.clone();
        // the carriers of the frame's module declared in generation are arrays, one element per iteration (`generation.rs`)
        children.extend(self.generated.elaborated().filter(|item| matches!(self.design.decl_data(*item).kind, DeclKind::Signal(_))));
        // a bundle port or signal stands for its fields: each leaf is a carrier or a node, the bundle itself neither (`bundles.rs`)
        let children = self.design.module_items(&children);
        let discrete = |this: &Self, c: &DeclId| this.types.carrier(*c).is_some() && this.types.node_array(*c).is_none();
        let ports = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Port(_)) && discrete(self, c));
        let signals = children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::Signal(_)) && discrete(self, c));
        for decl in ports.chain(signals).collect::<Vec<_>>() {
            // a carrier the frame already holds is the caller's own, bound at an `out` port of a hoisted module (`hoist.rs`)
            if let Some(held) = self.frames[frame].carriers.get(&decl).copied() {
                self.give_initial_value(frame, decl, held);
                continue;
            }
            self.generated_items_complete(decl);
            if self.is_empty_array(frame, decl) {
                self.frames[frame].empty_arrays.insert(decl);
                self.record_empty_array(frame, decl);
            } else if let Some(carrier) = self.lower_carrier(frame, decl) {
                let index = self.add_carrier(carrier);
                self.frames[frame].carriers.insert(decl, index);
                if frame == 0 && matches!(self.design.decl_data(decl).kind, DeclKind::Port(_)) {
                    self.structure.port_carriers += 1;
                }
                let resolved_nature = match self.consts.typed_carrier(decl).as_ref() {
                    Some(Ty::Carrier { nature, .. }) if self.design.is_resolved_nature(*nature) => Some(*nature),
                    _ => None,
                };
                if let Some(nature) = resolved_nature {
                    self.structure.resolved_carriers.push((index, decl, nature));
                }
            }
        }
    }

    /// The initial value the `out` port `decl` of a hoisted frame gives the caller's carrier `held` it is bound to (§5.2.6: the
    /// `out` port, not the declaring module, gives it). A carrier that already starts somewhere keeps its value.
    fn give_initial_value(&mut self, frame: usize, decl: DeclId, held: u32) {
        if self.structure.carriers[held as usize].init.is_some() {
            return;
        }
        if let Some(carrier) = self.lower_carrier(frame, decl) {
            self.structure.carriers[held as usize].init = carrier.init;
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
        let carried = match self.consts.typed_carrier(decl).as_ref() {
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
            DeclKind::Port(p) if frame != 0 => (CarrierRole::Signal, if p.dir == Some(Dir::Out) { p.default } else { None }),
            DeclKind::Port(p) => (if p.dir == Some(Dir::Out) { CarrierRole::Out } else { CarrierRole::In }, p.default),
            DeclKind::Signal(s) => (CarrierRole::Signal, s.init),
            _ => return None,
        };
        let resolved = match self.consts.typed_carrier(decl).as_ref() {
            Some(Ty::Carrier { nature, .. }) if self.design.is_resolved_nature(*nature) => Some(*nature),
            Some(array) if array.is_carrier_element() && matches!(array.innermost(), Ty::Carrier { nature, .. } if self.design.is_resolved_nature(*nature)) => {
                self.unsupported(data.file, data.span, "an array of resolved carriers");
                return None;
            }
            _ => None,
        };
        if let Some(nature) = resolved {
            return self.lower_resolved_carrier(frame, decl, nature, init);
        }
        let ty = self.carried_type(decl)?;
        let array = match self.types.carrier_array(decl).is_some() {
            true => Some(self.carrier_array_shape(decl, frame)?.0),
            false => None,
        };
        // a generated carrier refused for its gaps keeps no initial value: the refusal is the one diagnostic
        let init = init.filter(|_| !self.generated.refused.contains(&decl));
        let init = match init {
            // a carrier declared in generation: its initial value is one element's, folded for each iteration (§8.2.23)
            Some(expr) if self.generated.of(decl).is_some() => {
                let shape = array.clone()?;
                Some(self.generated_array_init(decl, expr, &shape, &ty)?)
            }
            Some(expr) if array.is_some() => {
                let shape = array.clone()?;
                Some(self.array_init(decl, expr, &shape, &ty, frame)?)
            }
            Some(expr) => match self.hand_written_encode(decl) {
                Some((encode, arguments)) => Some(self.encoded_init(expr, encode, &arguments, frame)?),
                None => {
                    let value = self.carrier_init(expr, frame)?;
                    Some(self.ir_constant(value, &ty))
                }
            },
            None if resolved.is_some() => None,
            // a type with a hand-written `Repr` and a `Default` starts at the levels `encode` gives for its default (GN-32 AC 2); a
            // type with no `Default` has none to start at: the carrier takes its value from what drives it
            None if let Some((encode, arguments)) = self.hand_written_encode(decl)
                && (frame == 0 || matches!(data.kind, DeclKind::Signal(_))) =>
            {
                match self.default_initial_value(decl, &ty) {
                    Some(default) => Some(self.encoded_default(encode, &arguments, default)?),
                    None => None,
                }
            }
            None if frame == 0 || matches!(data.kind, DeclKind::Signal(_)) => self.default_initial_value(decl, &ty),
            None => None,
        };
        let name = format!("{}{name}", self.frames[frame].prefix);
        Some(Carrier { name, ty, role, init, origin: self.locator.decl(decl), count: array.map(|shape| shape.count()) })
    }

    /// The carrier of the resolved port or signal `decl` of the nature `nature`: it reads the nature's `value` and is driven
    /// with the type `X` of the `impl Resolve` its drives picked (§5.2.15). An initial value `= v` is the module's first drive of
    /// it at `(0, 0)`, of type `X` (§5.2.3); a drive with none starts at the `Default` of `X` when it has one (§5.2.12).
    /// A carrier this module does not drive has no initial value. `None` after a diagnostic.
    fn lower_resolved_carrier(&mut self, frame: usize, decl: DeclId, nature: DeclId, init: Option<ExprId>) -> Option<Carrier> {
        let data = self.design.decl_data(decl).clone();
        let (chosen, picked) = self.chosen_resolve_impl(decl, nature)?;
        let Some(driven) = self.stored_type(&chosen.driven) else {
            self.unsupported(data.file, data.span, "a resolved carrier whose drive type has no value in the IR");
            return None;
        };
        let ty = self.carried_type(decl)?;
        let init = match init {
            Some(expr) => {
                let value = self.carrier_init(expr, frame)?;
                Some(self.ir_constant(value, &driven))
            }
            None if picked => self.default_constant(&chosen.driven, &driven),
            None => None,
        };
        let name = format!("{}{}", self.frames[frame].prefix, data.name.as_ref().map_or_else(String::new, |n| n.text.clone()));
        Some(Carrier { name, ty, role: CarrierRole::Resolved { driven, port: matches!(data.kind, DeclKind::Port(_)) }, init, origin: self.locator.decl(decl), count: None })
    }

    /// The `impl Resolve` the resolved carrier `decl` of `nature` is driven by, and whether a drive picked it: the one the
    /// first drive picked (§5.2.15), else the nature's only one, else the one whose `X` is `V`.
    pub(super) fn chosen_resolve_impl(&self, decl: DeclId, nature: DeclId) -> Option<(crate::types::ResolveImpl, bool)> {
        let impls = self.types.resolve_impls(nature).to_vec();
        let picked = self.concrete_implementation(decl, nature);
        let chosen = match picked {
            Some(implementation) => impls.iter().find(|candidate| candidate.implementation == implementation),
            None if impls.len() == 1 => impls.first(),
            None => impls.iter().find(|candidate| candidate.driven == candidate.value).or(impls.first()),
        }?
        .clone();
        Some((chosen, picked.is_some()))
    }

    /// The `impl Resolve` of `nature` the first drive of the resolved carrier `decl` picked. A carrier of a nature parameter
    /// bound by `Resolve<X, V>` picked the bound (the checker saw no concrete nature): the instance's nature answers with its
    /// `impl` whose drive type is `X`. `None` when no drive picked one.
    pub(super) fn concrete_implementation(&self, decl: DeclId, nature: DeclId) -> Option<DeclId> {
        let picked = self.types.carrier_drive_impls(decl).first().copied()?;
        if !matches!(self.design.decl_data(picked).kind, DeclKind::GenericParam(_)) {
            return Some(picked);
        }
        let driven = &self.types.resolve_impls(picked).first()?.driven;
        self.types.resolve_impls(nature).iter().find(|candidate| candidate.driven == *driven).map(|candidate| candidate.implementation)
    }

    /// The initial value of a carrier, folded: a constant expression over literals, `const`s and the
    /// params of its frame (§5.2.3, §5.1.5): a param it reads is a key param of the body, so each
    /// variant folds its own value (`structural.rs`). `None` is a diagnostic.
    pub(super) fn carrier_init(&mut self, expr: ExprId, frame: usize) -> Option<Constant> {
        let bindings = self.frame_bindings(frame);
        self.carrier_init_over(expr, &bindings)
    }

    /// The initial value `expr` of a carrier folded over `bindings`: the params of its frame, and the loop variables of the
    /// iteration of a carrier declared in generation. `None` is a diagnostic.
    pub(super) fn carrier_init_over(&mut self, expr: ExprId, bindings: &Bindings) -> Option<Constant> {
        let bindings = bindings.clone();
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
        let Some(Ty::Carrier { value, .. }) = self.consts.typed_carrier(decl) else { return None };
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
            (Ty::Struct(nominal) | Ty::Enum(nominal), _) if self.impls.has_written_default(ty) => return self.written_default(ty, nominal, stored),
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
    fn written_default(&mut self, ty: &Ty, nominal: &Nominal, stored: &ValueType) -> Option<Constant> {
        let function = *self.impls.builtin_functions("Default", "default", ty).first()?;
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
        // a unit variant of an enum with no field is its tag, no composite
        let folded = if self.typed(expr).is_some_and(|ty| ty.is_composite()) { self.composite_constant(expr, &Bindings::new()) } else { self.consts.value(expr, &Bindings::new()) };
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
        for array in structure.empty_arrays {
            self.ir.empty_array(body, array);
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
        for delay in structure.delays {
            self.ir.delay(body, delay);
        }
        for trace in structure.traces {
            self.ir.trace(body, trace);
        }
        for parameter in structure.session_parameters {
            self.ir.session_parameter(body, parameter);
        }
        for source in structure.noise_sources {
            self.ir.noise_source(body, source);
        }
        for partial in structure.partial_derivatives {
            self.ir.partial_derivative(body, partial);
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
