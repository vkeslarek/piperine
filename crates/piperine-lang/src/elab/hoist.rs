//! Hoisting a module with a result or a value input (part III §8.3.12, Appendix C.1.1; DIG-10,
//! AS-08, AS-09).
//!
//! A placement of a module that declares `-> T` or takes a value input gets no body and no
//! instance: its carriers, drives, events, reactions, nodes and `analog` blocks join the body of
//! the module that places it, under the placement's name (`gate.x` for `let gate = transition(..)`, else `<module>$<k>`, `k` counting
//! that module's placements in source order), and each read of it lowers the callee's result
//! expression where the read stands. The POM keeps the placement as written: hoisting is a view
//! of the IR, never a rewrite of the authored tree.
//!
//! Each placement is a [`Frame`]: the callee module, the frame of its caller, its params bound
//! to constants, and its value inputs bound to the caller's expressions. A name read in a frame
//! resolves there: a carrier to its hoisted carrier, a param to its constant, a value input to the
//! caller's expression lowered in the caller's frame (§8.3.8, §8.3.13), a node port to the
//! caller's terminal for its argument, a node `signal` to a node the body gained, a module-body
//! `let` to its value lowered in place. The `analog` blocks of every frame are lowered after the
//! body's own, in frame order (§8.3.16: the callee's rows read what its inputs probe in the
//! caller). A discrete `in` value input (`in x: Bit<f64>`) is a carrier of the
//! frame driven by the caller's expression, an anonymous definition. `cross` and `at` are core:
//! a placement of one is an event of the enclosing body, not a frame.

use piperine_ir::{Constant, Drive, ExprId as LoweredExpr, Hoisted, NodeRole, StmtId as LoweredStmt};

use super::digital::carriers::Frame;
use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::generation::Iteration;
use super::elaborator::Scope;
use super::reach::placement_params::PlacementSite;
use crate::refusal::forms::StructureForm;
use crate::msl::Diagnostic;
use crate::pom::{BehaviorKind, DeclId, DeclKind, ExprId, ExprKind, FieldInit, PatKind, PortArguments, StructClass};
use crate::types::Callee;

/// A hoisted placement as the frame that writes it knows it: the call, and the ordinals of the iteration of the generation
/// `for`s around it, outermost first (§8.2.23): each iteration of a placement written in generation is a frame of its own.
/// Empty outside generation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PlacementKey {
    /// The placement as written.
    pub(super) call: ExprId,
    /// The ordinals of its iteration.
    pub(super) iteration: Vec<u64>,
}

impl PlacementKey {
    /// The subscripts of the iteration, appended to the placement's name: `[1]`, `[0][2]`; empty outside generation.
    pub(super) fn subscripts(&self) -> String {
        self.iteration.iter().map(|ordinal| format!("[{ordinal}]")).collect()
    }
}

/// The frame a hoisted placement opens, as its contents are lowered: the call, the frame's index, the module it places and the name
/// of the frame, the ports of the module and the arguments the placement binds to them.
struct OpenedFrame<'a> {
    /// The placement as written.
    call: ExprId,
    /// The index of the frame it opens.
    index: usize,
    /// The module the placement places.
    module: DeclId,
    /// The name of the frame under the caller's.
    name: &'a str,
    /// The ports of `module`.
    ports: &'a [DeclId],
    /// The arguments of the placement, by port.
    bound: &'a PortArguments<'a>,
}

impl Elaboration<'_, '_> {
    /// The key of the placement `call` read or hoisted in `scope`: the iteration of the generation `for`s around it that the
    /// code being lowered sits in.
    pub(super) fn placement_key(&self, scope: &Scope, call: ExprId) -> PlacementKey {
        let iteration = self.implicit_ordinals(self.state.design.expr_node(call).owner, &scope.digital.ordinals);
        PlacementKey { call, iteration }
    }

    /// The module with a result that `call` places, if it places one. A call through a callable param reads the template the param
    /// holds in the frame being lowered (a hoisted module's own `s: C` holds what its placement bound).
    pub(super) fn result_callee(&self, call: ExprId) -> Option<DeclId> {
        self.placed_module(call, self.state.consts.frame_params()).filter(|m| matches!(&self.state.design.decl_data(*m).kind, DeclKind::Module(d) if d.ret.is_some()))
    }

    /// The module `call` places when that module is hoisted: one with a result or with a value input. A call through a param of
    /// a generic type bounded by `Fn(..)` (`s(5.0)` with `s: C`) places the module of the template the param holds in the frame
    /// being lowered.
    pub(super) fn hoisted_callee(&self, call: ExprId) -> Option<DeclId> {
        self.placed_module(call, self.state.consts.frame_params()).filter(|m| self.is_hoisted(*m))
    }

    /// Whether a placement of `module` is hoisted into the body that places it (Appendix C.1.1): it
    /// declares a result or takes a value input. Every other module keeps its own body.
    pub(super) fn is_hoisted(&self, module: DeclId) -> bool {
        let data = self.state.design.decl_data(module);
        matches!(&data.kind, DeclKind::Module(m) if m.ret.is_some()) || data.children.iter().any(|c| self.is_value_input(*c))
    }

    /// Whether `decl` is a carrier port declared `out`.
    fn is_out_port(&self, decl: DeclId) -> bool {
        matches!(&self.state.design.decl_data(decl).kind, DeclKind::Port(p) if self.state.types.carrier(decl).is_some() && p.dir == Some(crate::pom::Dir::Out))
    }

    /// Whether `decl` is a port of a value type: a value input.
    pub(super) fn is_value_input(&self, decl: DeclId) -> bool {
        super::structural::ParamReads::new(self.state.design, self.state.types).is_value_input(decl)
    }

    /// The placements, outermost first, that open the frame `frame`: the calls that lead from the
    /// body's own module to it. Empty for the body's own module.
    pub(super) fn placement_chain(&self, frame: usize) -> Vec<ExprId> {
        let mut chain = Vec::new();
        let mut at = frame;
        while let Some(call) = self.state.structure.frames[at].call {
            chain.push(call);
            match self.state.structure.frames[at].parent {
                Some(parent) => at = parent,
                None => break,
            }
        }
        chain.reverse();
        chain
    }

    /// Hoist every placement of a hoisted module that the module of the frame `scope.frame`
    /// writes, in source order.
    pub(super) fn hoist_placements(&mut self, scope: &Scope) {
        let module = self.state.structure.frames[scope.values.frame].module;
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return };
        // A placement in a loop is hoisted by the unrolling of the loop, once per iteration.
        let looped = self.analog().looped_placements(module);
        for call in data.instances.clone() {
            if self.hoisted_callee(call).is_none() || looped.contains(&call) {
                continue;
            }
            // a placement written in generation is hoisted once for each iteration that elaborates it (§8.2.23)
            let owner = self.state.design.expr_node(call).owner;
            let iterations = match self.state.structure.generated.of(owner) {
                Some(iterations) => iterations.clone(),
                None => vec![Iteration::default()],
            };
            for iteration in &iterations {
                let mut inner = Scope::sub_scope(scope);
                for (variable, value) in &iteration.values {
                    inner.values.unrolled.bind(*variable, Constant::Int(*value));
                }
                inner.digital.ordinals = iteration.ordinals.clone();
                self.frame_of(&inner, call);
            }
        }
    }

    /// The frame of the placement `call`, written in the frame `scope.frame`; hoisted the first
    /// time it is asked for. `None` after a diagnostic.
    pub(super) fn frame_of(&mut self, scope: &Scope, call: ExprId) -> Option<usize> {
        let key = self.placement_key(scope, call);
        if let Some(frame) = self.state.structure.frames[scope.values.frame].placements.get(&key) {
            return Some(*frame);
        }
        // A placement that failed has said why once; its reads stay silent.
        if self.state.structure.frames[scope.values.frame].failed.contains(&key) {
            return None;
        }
        let frame = self.hoist(scope, &key);
        if frame.is_none() {
            self.state.structure.frames[scope.values.frame].failed.insert(key);
        }
        frame
    }

    /// `call`'s result, lowered where it is read: the callee's result expression in its frame.
    pub(super) fn lower_result(&mut self, scope: &mut Scope, call: ExprId) -> Option<LoweredExpr> {
        let frame = self.frame_of(scope, call)?;
        let module = self.state.structure.frames[frame].module;
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return None };
        let result = data.result?;
        let entry = self.enter_frame(scope, frame);
        let lowered = self.values().lower_expr(scope, result);
        self.leave_frame(scope, entry);
        lowered
    }

    /// `call`'s result of `Option` type as the pair of its two halves, lowered where it is read: the callee's result expression in its
    /// frame (the `Option` form of [`Elaboration::lower_result`]).
    pub(super) fn lower_result_pair(&mut self, scope: &mut Scope, call: ExprId) -> Option<super::value::compound::Pair> {
        let frame = self.frame_of(scope, call)?;
        let module = self.state.structure.frames[frame].module;
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return None };
        let result = data.result?;
        let entry = self.enter_frame(scope, frame);
        let pair = self.values().lower_option(scope, result);
        self.leave_frame(scope, entry);
        pair
    }

    /// A read of the value input `decl` of the frame `scope.frame`: the caller's expression,
    /// lowered in the caller's frame. `None` when the frame has no such input (the read is not one).
    pub(super) fn input_of(&self, scope: &Scope, decl: DeclId) -> Option<Option<ExprId>> {
        self.state.structure.frames.get(scope.values.frame).and_then(|f| f.inputs.get(&decl)).copied()
    }

    /// Lower `expr`, the caller's expression of an input of the frame `scope.frame`, in the
    /// caller's frame, with `lower`.
    pub(super) fn in_caller<T>(&mut self, scope: &mut Scope, lower: impl FnOnce(&mut Self, &mut Scope) -> Option<T>) -> Option<T> {
        let parent = self.state.structure.frames[scope.values.frame].parent?;
        // the caller's expression reads the loop variables it was written under (`unroll.rs`)
        let seen = self.state.structure.frames[scope.values.frame].unrolled.clone();
        let iteration = self.state.structure.frames[scope.values.frame].ordinals.clone();
        let entry = self.enter_frame(scope, parent);
        let outer_unrolled = std::mem::replace(&mut scope.values.unrolled, seen);
        let outer_ordinals = std::mem::replace(&mut scope.digital.ordinals, iteration);
        let lowered = lower(self, scope);
        self.leave_frame(scope, entry);
        scope.values.unrolled = outer_unrolled;
        scope.digital.ordinals = outer_ordinals;
        lowered
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier
    /// pattern, the matching component of a tuple pattern over a tuple expression.
    pub(super) fn body_let_value(&self, binding: DeclId) -> Option<ExprId> {
        super::consts::ConstEval::body_let_value(self.state.design, binding)
    }

    /// A read of a module-body `let`: the result of the placement it binds, else its value
    /// lowered where the read stands (Appendix C.1.1: inlined at each read).
    pub(super) fn lower_body_let(&mut self, scope: &mut Scope, value: ExprId) -> Option<LoweredExpr> {
        let value = self.peel_parens(value);
        if matches!(self.state.design.expr_node(value).kind, ExprKind::Call { .. }) && self.result_callee(value).is_some() {
            return self.lower_result(scope, value);
        }
        self.values().lower_expr(scope, value)
    }

    /// The name of the placement `call` of `module` in the frame `parent`: the `let` it
    /// initializes, else `<module>$<k>`.
    fn hoisted_name(&mut self, parent: usize, call: ExprId, module: DeclId) -> String {
        let owner = self.state.design.expr_node(call).owner;
        if let DeclKind::Let(l) = &self.state.design.decl_data(owner).kind
            && self.peel_parens(l.value) == call
            && let PatKind::Ident { name, sub: None, .. } = &self.state.design.pat_node(l.pat).kind
        {
            return name.text.clone();
        }
        let caller = self.state.structure.frames[parent].module;
        let looped = self.analog().looped_placements(caller);
        let same = |c: &ExprId| self.result_callee(*c) == Some(module);
        let outside: Vec<ExprId> = match &self.state.design.decl_data(caller).kind {
            DeclKind::Module(m) => m.instances.iter().copied().filter(|c| same(c) && !looped.contains(c)).collect(),
            _ => Vec::new(),
        };
        let name = self.state.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str());
        // A placement in an unrolled loop is numbered after the ones outside any loop, in unrolled
        // order (`unroll.rs`, D16); the others count the placements before them in source order.
        let earlier = if looped.contains(&call) {
            let next = self.state.structure.unrolled_names.entry((parent, module)).or_insert(0);
            *next += 1;
            outside.len() + *next as usize - 1
        } else {
            outside.iter().take_while(|c| **c != call).count()
        };
        format!("{name}${earlier}")
    }

    /// Open the frame of `call`, placed in the frame `scope.frame`, and hoist what the callee
    /// holds into the body: its carriers, the drives of its discrete inputs and its definitions,
    /// its own placements, its reactions. `None` after a diagnostic.
    fn hoist(&mut self, scope: &Scope, key: &PlacementKey) -> Option<usize> {
        let call = key.call;
        let module = self.hoisted_callee(call)?;
        let parent = scope.values.frame;
        let node = self.state.design.expr_node(call).clone();
        let file = self.state.design.decl_data(node.owner).file;
        let mut chain = Some(parent);
        while let Some(k) = chain {
            if self.state.structure.frames[k].module == module {
                let name = self.state.design.decl_data(module).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.report(file, Diagnostic::new("E0341", format!("module `{name}` places itself through a hoisted placement"), node.span, "recursive placement"));
                return None;
            }
            chain = self.state.structure.frames[k].parent;
        }
        let ExprKind::Call { callee, args, params, class } = &node.kind else { return None };
        if !self.hoistable(module, file, node.span) {
            return None;
        }
        let fields: Vec<FieldInit> = match &self.state.design.expr_node(*callee).kind {
            ExprKind::Struct { fields, class: StructClass::Template(m), .. } if *m == module => fields.clone(),
            _ => params.clone().unwrap_or_default(),
        };
        // a generic module places an instance: its arguments folded under the caller's, its `where` discharged
        let instance = if self.is_generic_module(module) { Some(self.module_instance(call, module)?) } else { None };
        let substitution = instance.as_ref().map_or_else(|| self.state.consts.substitution().clone(), |instance| self.instance_substitution(instance.item, &instance.arguments));
        let outer = self.state.consts.set_substitution(substitution.clone());
        let caller_values = self.frame_bindings(parent).overlaid(&scope.values.unrolled);
        // a placement written through a template value (`t(x)`, `fs[k](x)`) takes the params the template folds to (§8.4.4)
        let through = match self.template_through(scope, call, class) {
            Ok(through) => through,
            Err(()) => {
                self.state.consts.set_substitution(outer);
                return None;
            }
        };
        let site = PlacementSite { fields: &fields, template: through.as_ref(), caller: &caller_values, caller_substitution: &outer };
        let bound = self.placement_params(module, &site, file, node.span);
        self.state.consts.set_substitution(outer);
        let bound = bound?;
        // a placement written in generation is named by its iteration too: `y[1]`, `idt$0[1]`
        let name = format!("{}{}", self.hoisted_name(parent, call, module), key.subscripts());
        let prefix = format!("{}{name}.", self.state.structure.frames[parent].prefix);
        let ports: Vec<DeclId> = self.state.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Port(_))).collect();
        let mut frame = Frame::root(module);
        frame.parent = Some(parent);
        frame.call = Some(call);
        frame.prefix = prefix;
        frame.params = bound;
        frame.unrolled = scope.values.unrolled.clone();
        frame.ordinals = scope.digital.ordinals.clone();
        frame.substitution = substitution.clone();
        let mut node_ports = Vec::new();
        let mut out_ports = Vec::new();
        let bound = PortArguments::bind(self.state.design, module, args);
        for (k, port) in ports.iter().enumerate() {
            if self.is_value_input(*port) {
                frame.inputs.insert(*port, bound.at(k).map(|a| a.value));
            } else if self.state.types.carrier(*port).is_none() {
                node_ports.push((*port, bound.at(k).map(|a| a.value)));
            } else if self.is_out_port(*port) {
                out_ports.push((*port, bound.at(k).map(|a| a.value)));
            }
        }
        // An `out` carrier port is the caller's own carrier for its argument: what the callee commits, the caller's carrier holds.
        for (port, arg) in out_ports {
            let (carrier, window) = self.lower_carrier_argument(scope, &name, port, arg?)?;
            if window.is_some() {
                self.refuse(file, node.span, StructureForm::WindowOfArrayOfCarriersAtOut);
                return None;
            }
            frame.carriers.insert(port, carrier);
        }
        // A node port is the caller's terminal for its argument, read in the caller's frame.
        for (port, arg) in node_ports {
            let terminal = self.analog().lower_terminal(scope, arg?)?;
            frame.nodes.insert(port, terminal);
        }
        self.state.structure.frames.push(frame);
        let index = self.state.structure.frames.len() - 1;
        self.state.structure.frames[parent].placements.insert(key.clone(), index);
        let outer = self.state.consts.set_substitution(substitution);
        let hoisted = self.hoist_frame(scope, OpenedFrame { call, index, module, name: &name, ports: &ports, bound: &bound });
        self.state.consts.set_substitution(outer);
        hoisted
    }

    /// The contents of the frame `index` of the placement `call`, lowered under the substitution of its instance.
    fn hoist_frame(&mut self, scope: &Scope, opened: OpenedFrame) -> Option<usize> {
        let OpenedFrame { index, module, .. } = opened;
        let caller = self.state.structure.frames[self.state.structure.frames[index].parent.unwrap_or(0)].substitution.clone();
        // the generation of the callee's body unrolls under the frame's params; the caller's is parked while the frame is lowered
        let parent = self.state.structure.frames[index].parent.unwrap_or(0);
        self.state.structure.frames[parent].generated = std::mem::take(&mut self.state.structure.generated);
        let keys = self.frame_bindings(index);
        let outer_keys = std::mem::replace(&mut self.state.structure.body_keys, keys.clone());
        let outer_params = self.state.consts.set_frame_params(keys);
        self.expand_generation_of(module, index);
        self.state.consts.set_frame_params(outer_params);
        let lowered = self.hoist_contents(scope, &opened, &caller);
        self.state.structure.body_keys = outer_keys;
        self.state.structure.frames[index].generated = std::mem::take(&mut self.state.structure.generated);
        self.state.structure.generated = std::mem::take(&mut self.state.structure.frames[parent].generated);
        lowered
    }

    /// What the frame `index` holds: its nodes, carriers, drives, definitions, placements and reactions.
    fn hoist_contents(&mut self, scope: &Scope, opened: &OpenedFrame, caller: &crate::types::Substitution) -> Option<usize> {
        let OpenedFrame { call, index, module, name, ports, bound } = *opened;
        let caller = caller.clone();
        let gained = self.hoist_nodes(index, module);
        self.record_hoisted(index, module, name, call, gained);
        self.digital().lower_carriers(index, module);
        // A discrete `in` value input: the hoisted carrier, driven by the caller's expression.
        for (position, port) in ports.iter().enumerate() {
            let Some(arg) = bound.at(position) else { continue };
            // an `out` port is the caller's carrier itself, driven by the callee alone
            if self.is_out_port(*port) {
                continue;
            }
            let Some(carrier) = self.state.structure.frames[index].carriers.get(port).copied() else { continue };
            let origin = self.expr_origin(arg.value);
            let wanted = self.state.digital.structure.carriers.get(carrier as usize).map(|c| c.ty.clone());
            // the caller's expression reads the caller's generic names, not the callee's
            let callee = self.state.consts.set_substitution(caller.clone());
            let value = self.digital().lower_drive_value(scope, arg.value, None, wanted.as_ref());
            self.state.consts.set_substitution(callee);
            if let Some((value, _)) = value {
                self.state.digital.structure.drives.push(Drive { carrier, value, after: None, origin, element: None, path: vec![] });
            }
        }
        let mut inner = Scope::sub_scope(scope);
        inner.values.frame = index;
        let outer_params = self.state.consts.set_frame_params(self.frame_bindings(index));
        self.digital().lower_definitions(&inner);
        self.hoist_placements(&inner);
        self.digital().lower_digital(&mut inner, module);
        self.state.consts.set_frame_params(outer_params);
        Some(index)
    }

    /// The node `signal`s of `module` as nodes of the body, named under the frame `frame`: its
    /// terminals are the frame's own. How many nodes the body gained.
    fn hoist_nodes(&mut self, frame: usize, module: DeclId) -> usize {
        let mut children = self.state.design.decl_data(module).children.clone();
        // the nodes declared in generation are arrays, one element per iteration (`generation.rs`)
        children.extend(self.state.structure.generated.elaborated().filter(|item| matches!(self.state.design.decl_data(*item).kind, DeclKind::Signal(_))));
        let signals: Vec<DeclId> = children.into_iter().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Signal(_)) && (self.state.types.carrier(*c).is_none() || self.state.types.node_array(*c).is_some())).collect();
        let prefix = self.state.structure.frames[frame].prefix.clone();
        let mut gained = 0;
        for signal in signals {
            if self.state.types.node_array(signal).is_some() {
                // an array of nodes: one node of the body per element, its shape kept for `w[k]` (`carrier_arrays.rs`)
                if let Some((shape, first)) = self.digital().add_node_array(signal, NodeRole::Signal, &prefix, frame) {
                    self.state.structure.frames[frame].nodes.insert(signal, first);
                    self.state.structure.frames[frame].node_shapes.insert(signal, shape);
                    gained += 1;
                }
            } else if let Some(terminal) = self.add_node(signal, NodeRole::Signal, &prefix, frame) {
                self.state.structure.frames[frame].nodes.insert(signal, terminal);
                gained += 1;
            }
        }
        gained
    }

    /// Records the frame `frame` of `module`, placed by `call` under `name`, in `BodyData::hoisted`
    /// when it adds a node or an `analog` statement to the body (the interpreter refuses it by
    /// name, and a reader of the IR sees what was placed where).
    fn record_hoisted(&mut self, frame: usize, module: DeclId, name: &str, call: ExprId, gained_nodes: usize) {
        let has_analog = matches!(&self.state.design.decl_data(module).kind, DeclKind::Module(m) if m.behaviors.iter().any(|b| matches!(&self.state.design.decl_data(*b).kind, DeclKind::Behavior(bd) if bd.kind == BehaviorKind::Analog)));
        if gained_nodes == 0 && !has_analog {
            return;
        }
        let node = self.state.design.expr_node(call);
        let origin = self.state.locator.origin(self.state.design.decl_data(node.owner).file, node.span);
        let module_name = self.state.design.decl_data(self.state.structure.frames[frame].module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        self.state.digital.structure.hoisted.push(Hoisted { name: name.to_string(), module: module_name, origin });
    }

    /// The `analog` blocks of every hoisted frame, lowered in frame order after the body's own
    /// (§8.3.16); `None` when no frame has a statement.
    pub(super) fn lower_hoisted_analog(&mut self, scope: &mut Scope) -> Option<Vec<LoweredStmt>> {
        let mut stmts: Option<Vec<LoweredStmt>> = None;
        // A frame the lowering of an earlier frame hoists lazily joins the walk.
        let mut frame = 1;
        while frame < self.state.structure.frames.len() {
            // A reach frame (`reach/fields.rs`) names a placed child, whose `analog` block is its own body's.
            if self.state.structure.frames[frame].is_reached() {
                frame += 1;
                continue;
            }
            let module = self.state.structure.frames[frame].module;
            let entry = self.enter_frame(scope, frame);
            let lowered = self.analog().lower_analog(scope, module);
            self.leave_frame(scope, entry);
            if let Some(lowered) = lowered {
                stmts.get_or_insert_with(Vec::new).extend(lowered);
            }
            frame += 1;
        }
        stmts
    }

    /// Whether `module` holds only what a hoisted module can hold: no placement of a module that is
    /// not hoisted (it would need a body inside a frame). `P0001` naming the first
    /// form that is not, at the placement.
    fn hoistable(&mut self, module: DeclId, file: crate::pom::FileId, span: crate::msl::Span) -> bool {
        let data = self.state.design.decl_data(module).clone();
        let DeclKind::Module(m) = &data.kind else { return false };
        let refuse = |this: &mut Self, what: &str| {
            this.state.structure.refused_modules.insert(module);
            this.refuse_with(file, span, StructureForm::HoistedModuleThatHolds, [what.to_string()]);
            false
        };
        if m.instances.iter().any(|c| matches!(self.state.types.callee(*c), Some(Callee::Module(_))) && self.hoisted_callee(*c).is_none()) {
            return refuse(self, "a placement of a module that is not hoisted");
        }
        true
    }

    /// The constant a param value `expr` folds to over `read`, as for a placement that keeps its body: a composite (an array of
    /// coefficients, a table) is a constant of the IR's arena, bound taken apart by `bind_param` (`frame_bindings`); an array of
    /// templates is the aggregate of their numbers, held as a key of the frame (§8.4.6).
    /// A value the placement gives is written in the caller, so it folds under the caller's generic arguments (`caller`).
    pub(super) fn param_constant(&mut self, expr: ExprId, read: &Bindings, caller: Option<&crate::types::Substitution>) -> Result<Constant, super::consts::ConstFault> {
        let callee = caller.map(|substitution| self.state.consts.set_substitution(substitution.clone()));
        // the fold of a placement that keeps its body (`fold_param_value`): a template or an array of them is the number(s) it interns
        let folded = self.fold_param_value(expr, read);
        if let Some(callee) = callee {
            self.state.consts.set_substitution(callee);
        }
        folded
    }
}
