//! `IrBuilder`: the only writer of an [`Ir`]. Ids are handed out in call order, which makes
//! equal call sequences give equal IRs.

use std::collections::BTreeMap;

use crate::real::Real;
use crate::deps::DepsMemo;
use crate::ids::{AggregateId, WideId, BodyId, ExprId, FunctionId, InstanceId, NatureId, NetId, NodeId, StmtId, TypeId};
use crate::model::{AggregateData, WideData, CarrierWindow, EmptyArray, NodeAlias, NodeArray, 
    BodyData, BodyNode, Branch, Carrier, Child, ChildValue, Constant, DelaySite, Derivative, Drive, Event, NoiseSource, SessionParameter, TraceSite, PartialDerivative, Expr, ExprData, FunctionData, GenericInstance, Hoisted, InstanceData, Inflow, Ir, Merge, Reach,
    Local, NatureData, NetData, NodeData, NodeRef, Origin, Param, Quantity, Reaction, ReprForm, Resolution, Site, Stmt, Stream, Terminal, TypeData, ValueType,
};

/// The parts of a body, in the order a handle lists them.
#[derive(Debug, Clone, PartialEq)]
pub struct BodySpec {
    pub name: String,
    pub origin: Origin,
    pub params: Vec<Param>,
    pub nodes: Vec<BodyNode>,
    pub branches: Vec<Branch>,
    pub limits: Vec<Site>,
    pub stims: Vec<Site>,
    pub locals: Vec<Local>,
    pub analog: Option<Vec<StmtId>>,
    pub children: Vec<Child>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSpec {
    pub name: String,
    pub origin: Origin,
    pub params: Vec<Local>,
    pub ret: ValueType,
    pub locals: Vec<Local>,
    pub body: ExprId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeSpec {
    pub path: String,
    pub nature: NatureId,
    pub declared_by: InstanceId,
    pub init: Option<Real>,
    pub origin: Origin,
}

/// A discrete net of the design, the twin of `NodeSpec`.
#[derive(Debug, Clone, PartialEq)]
pub struct NetSpec {
    pub path: String,
    pub ty: ValueType,
    pub declared_by: InstanceId,
    pub init: Option<Constant>,
    pub origin: Origin,
}

/// An instance, added after its parent (pre-order); the builder links it into the parent's
/// children.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceSpec {
    pub path: String,
    pub name: String,
    pub body: BodyId,
    pub parent: Option<InstanceId>,
    pub nodes: Vec<NodeRef>,
    pub params: Vec<Constant>,
    pub origin: Origin,
}

/// The nodes merged into others so far: a merged node and the node (or ground) it became.
/// Merging is a union of two classes of nodes; the class's one survivor is the root `resolve`
/// returns.
#[derive(Debug, Default)]
struct NodeUnion {
    merged_into: BTreeMap<NodeId, NodeRef>,
}

impl NodeUnion {
    /// The survivor of `node`'s class: `node` itself while nothing merged it away.
    fn resolve(&self, node: NodeRef) -> NodeRef {
        let mut at = node;
        while let NodeRef::Node(id) = at {
            match self.merged_into.get(&id) {
                Some(next) => at = *next,
                None => break,
            }
        }
        at
    }

    /// Makes `kept` and `merged` one node. `kept`'s survivor stays unless it is the node
    /// `merged` stands for, and ground always survives a node (a node merged into ground is
    /// gone).
    fn join(&mut self, kept: NodeRef, merged: NodeId) {
        let (kept, merged) = (self.resolve(kept), self.resolve(NodeRef::Node(merged)));
        match (kept, merged) {
            (kept, merged) if kept == merged => {}
            (NodeRef::Node(a), NodeRef::Node(b)) => {
                self.merged_into.insert(b, NodeRef::Node(a));
            }
            (NodeRef::Node(a), NodeRef::Ground) => {
                self.merged_into.insert(a, NodeRef::Ground);
            }
            (NodeRef::Ground, NodeRef::Node(b)) => {
                self.merged_into.insert(b, NodeRef::Ground);
            }
            (NodeRef::Ground, NodeRef::Ground) => {}
        }
    }
}

#[derive(Debug, Default)]
pub struct IrBuilder {
    natures: Vec<NatureData>,
    bodies: Vec<BodyData>,
    functions: Vec<FunctionData>,
    exprs: Vec<ExprData>,
    stmts: Vec<Stmt>,
    instances: Vec<InstanceData>,
    nodes: Vec<NodeData>,
    nets: Vec<NetData>,
    /// The paths of the merged nodes, set when `finish` applies the merges.
    aliases: Vec<NodeAlias>,
    /// The source structs and enums, by `TypeId`.
    types: Vec<TypeData>,
    /// The composite constants, by `AggregateId`.
    aggregates: Vec<AggregateData>,
    /// The constants of integer types wider than 64 bits, by `WideId`.
    wides: Vec<WideData>,
    /// The merges `merge_nodes` asked for, applied by `finish`.
    union: NodeUnion,
    /// The derived function of each `(function, argument)` the differentiation built; `None`
    /// while it is being built (a recursive function has none).
    derived: BTreeMap<(FunctionId, u32), Option<FunctionId>>,
}

impl IrBuilder {
    pub fn new() -> Self {
        IrBuilder::default()
    }

    /// Adds a source struct or enum to the type arena; `ValueType::Struct`/`Enum` name it by the id.
    pub fn type_data(&mut self, data: TypeData) -> TypeId {
        self.types.push(data);
        TypeId((self.types.len() - 1) as u32)
    }

    /// The composite constant `id` names, as added.
    pub fn aggregate_at(&self, id: AggregateId) -> &AggregateData {
        &self.aggregates[id.index()]
    }

    /// Adds a composite constant; `Constant::Aggregate` names it by the id.
    pub fn aggregate(&mut self, data: AggregateData) -> AggregateId {
        self.aggregates.push(data);
        AggregateId((self.aggregates.len() - 1) as u32)
    }

    /// Adds a constant of an integer type wider than 64 bits; `Constant::Wide` and `Expr::Wide` name it by the id.
    pub fn wide(&mut self, data: WideData) -> WideId {
        self.wides.push(data);
        WideId((self.wides.len() - 1) as u32)
    }

    pub fn nature(&mut self, name: &str, potential: Quantity, flow: Option<Quantity>) -> NatureId {
        self.natures.push(NatureData { name: name.to_string(), potential, flow });
        NatureId(self.natures.len() as u32 - 1)
    }

    /// The nature already added under `name`, if any.
    /// Kept for the IR builders of the test support, which look a nature up by name.
    pub fn find_nature(&self, name: &str) -> Option<NatureId> {
        let at = self.natures.iter().position(|n| n.name == name)?;
        Some(NatureId(at as u32))
    }

    pub fn body(&mut self, spec: BodySpec) -> BodyId {
        self.bodies.push(BodyData {
            name: spec.name,
            origin: spec.origin,
            params: spec.params,
            nodes: spec.nodes,
            branches: spec.branches,
            limits: spec.limits,
            stims: spec.stims,
            locals: spec.locals,
            analog: spec.analog,
            children: spec.children,
            instance: None,
            carriers: Vec::new(),
            node_arrays: Vec::new(),
            empty_arrays: Vec::new(),
            child_windows: Vec::new(),
            drives: Vec::new(),
            events: Vec::new(),
            reactions: Vec::new(),
            streams: Vec::new(),
            derivatives: Vec::new(),
            delays: Vec::new(),
            noise_sources: Vec::new(),
            session_parameters: Vec::new(),
            traces: Vec::new(),
            partial_derivatives: Vec::new(),
            hoisted: Vec::new(),
            merges: Vec::new(),
            reaches: Vec::new(),
            inflows: Vec::new(),
            child_params: Vec::new(),
            child_carriers: Vec::new(),
            deps: DepsMemo::default(),
        });
        BodyId(self.bodies.len() as u32 - 1)
    }

    /// The body `id` added before; an id this builder never handed out panics, as a handle given
    /// an id out of range does.
    fn body_data(&mut self, id: BodyId) -> &mut BodyData {
        let count = self.bodies.len();
        match self.bodies.get_mut(id.index()) {
            Some(body) => body,
            None => panic!("body id {} out of range ({count} bodies)", id.0),
        }
    }

    /// Adds a carrier to `body`, after the ones added before: returns its index, the identity
    /// `Expr::Carrier` and `Drive` read it by.
    pub fn carrier(&mut self, body: BodyId, carrier: Carrier) -> u32 {
        let carriers = &mut self.body_data(body).carriers;
        carriers.push(carrier);
        carriers.len() as u32 - 1
    }

    /// Adds a definition to `body`, in source order: returns its index.
    pub fn drive(&mut self, body: BodyId, drive: Drive) -> u32 {
        let drives = &mut self.body_data(body).drives;
        drives.push(drive);
        drives.len() as u32 - 1
    }

    /// Adds an event to `body`: returns its index, the identity `Reaction::on` names it by.
    pub fn event(&mut self, body: BodyId, event: Event) -> u32 {
        let events = &mut self.body_data(body).events;
        events.push(event);
        events.len() as u32 - 1
    }

    /// Adds a reaction to `body`, in source order: returns its index.
    pub fn reaction(&mut self, body: BodyId, reaction: Reaction) -> u32 {
        let reactions = &mut self.body_data(body).reactions;
        reactions.push(reaction);
        reactions.len() as u32 - 1
    }

    /// Sets the value `value` of param `param` of child `child` (an index into the body's
    /// children), beside the child (contract D4: `Child` itself is built literally and gains no
    /// field). A param set twice keeps both entries; `Ir::check` names that.
    pub fn child_param(&mut self, body: BodyId, child: u32, param: u32, value: ChildValue) {
        let lists = &mut self.body_data(body).child_params;
        if lists.len() <= child as usize {
            lists.resize(child as usize + 1, Vec::new());
        }
        lists[child as usize].push((param, value));
    }

    /// Names `count` consecutive nodes of `body` as one array of nodes (`signal bl: [Electrical; 4]`): returns its
    /// index. The nodes themselves are `BodyNode`s named `name[0]` and on, added with the body.
    pub fn node_array(&mut self, body: BodyId, array: NodeArray) -> u32 {
        let arrays = &mut self.body_data(body).node_arrays;
        arrays.push(array);
        arrays.len() as u32 - 1
    }

    /// Records the array `array` of `body` declared with no element: no carrier and no node stand for it.
    pub fn empty_array(&mut self, body: BodyId, array: EmptyArray) {
        self.body_data(body).empty_arrays.push(array);
    }

    /// Records the part of an array of carriers each carrier port of child `child` of `body` is given, parallel to
    /// `bind_child_carriers`: `None` for a port bound whole or to a scalar carrier. Replaces what was set before.
    pub fn bind_child_windows(&mut self, body: BodyId, child: u32, windows: Vec<Option<CarrierWindow>>) {
        let lists = &mut self.body_data(body).child_windows;
        if lists.len() <= child as usize {
            lists.resize(child as usize + 1, Vec::new());
        }
        lists[child as usize] = windows;
    }

    /// Records what child `child` of `body` (an index into its children) binds at each carrier
    /// port of its body, in the port order: the parent's carrier, or `None` for an open port. The
    /// authored tree shows what a placement connects, carriers as well as nodes (`Child` itself
    /// gains no field, contract D4). Replaces what was set before.
    pub fn bind_child_carriers(&mut self, body: BodyId, child: u32, carriers: Vec<Option<u32>>) {
        let lists = &mut self.body_data(body).child_carriers;
        if lists.len() <= child as usize {
            lists.resize(child as usize + 1, Vec::new());
        }
        lists[child as usize] = carriers;
    }

    /// Replaces the node slots of child number `child` of `body`: the terminals it connects, then one
    /// slot per node of the child's own that is no port. A child's own nodes are known once its body
    /// is built, which is after its parent's.
    pub fn set_child_nodes(&mut self, body: BodyId, child: u32, nodes: Vec<Terminal>) {
        if let Some(slot) = self.body_data(body).children.get_mut(child as usize) {
            slot.nodes = nodes;
        }
    }

    /// Adds a derivative site to `body`: returns its index, the identity `Expr::Derivative`
    /// reads it by.
    pub fn derivative(&mut self, body: BodyId, derivative: Derivative) -> u32 {
        let derivatives = &mut self.body_data(body).derivatives;
        derivatives.push(derivative);
        derivatives.len() as u32 - 1
    }

    /// Adds a delay site to `body`: returns its index, the identity `Expr::Delay` reads it by.
    pub fn delay(&mut self, body: BodyId, delay: DelaySite) -> u32 {
        let delays = &mut self.body_data(body).delays;
        delays.push(delay);
        delays.len() as u32 - 1
    }

    /// Adds a noise source to `body`: returns its index, the identity `Expr::Noise` reads it by.
    pub fn noise_source(&mut self, body: BodyId, source: NoiseSource) -> u32 {
        let sources = &mut self.body_data(body).noise_sources;
        sources.push(source);
        sources.len() as u32 - 1
    }

    /// Adds a session parameter to `body`: returns its index, the identity `Expr::SessionParameter`
    /// reads it by.
    pub fn session_parameter(&mut self, body: BodyId, parameter: SessionParameter) -> u32 {
        let parameters = &mut self.body_data(body).session_parameters;
        parameters.push(parameter);
        parameters.len() as u32 - 1
    }

    /// Records that `body` placed a module in an expression whose node or `analog` statements
    /// joined it: returns its index.
    pub fn hoisted(&mut self, body: BodyId, hoisted: Hoisted) -> u32 {
        let list = &mut self.body_data(body).hoisted;
        list.push(hoisted);
        list.len() as u32 - 1
    }

    /// Records that `body` dropped a fixed zero-potential branch as a node merge: returns its
    /// index. Placement then calls `merge_nodes` for each instance of the body.
    pub fn merge(&mut self, body: BodyId, merge: Merge) -> u32 {
        let list = &mut self.body_data(body).merges;
        list.push(merge);
        list.len() as u32 - 1
    }

    /// Records that `body` reads a `pub` node or carrier of one of its children (`inst.name`):
    /// returns its index.
    pub fn reach(&mut self, body: BodyId, reach: Reach) -> u32 {
        let list = &mut self.body_data(body).reaches;
        list.push(reach);
        list.len() as u32 - 1
    }

    /// Records the ammeter of a `p.inflow()` of `body`: returns its index.
    pub fn inflow(&mut self, body: BodyId, inflow: Inflow) -> u32 {
        let list = &mut self.body_data(body).inflows;
        list.push(inflow);
        list.len() as u32 - 1
    }

    /// Records a `ddx` of `body`: returns its index.
    pub fn partial_derivative(&mut self, body: BodyId, partial: PartialDerivative) -> u32 {
        let list = &mut self.body_data(body).partial_derivatives;
        list.push(partial);
        list.len() as u32 - 1
    }

    /// Adds a trace site to `body`: returns its index, the identity `Stmt::Trace` names it by.
    pub fn trace(&mut self, body: BodyId, trace: TraceSite) -> u32 {
        let traces = &mut self.body_data(body).traces;
        traces.push(trace);
        traces.len() as u32 - 1
    }

    /// Adds a record stream to `body`: returns its index, the identity `Stmt::Emit` names it by.
    pub fn stream(&mut self, body: BodyId, stream: Stream) -> u32 {
        let streams = &mut self.body_data(body).streams;
        streams.push(stream);
        streams.len() as u32 - 1
    }

    pub fn function(&mut self, spec: FunctionSpec) -> FunctionId {
        self.functions.push(FunctionData {
            name: spec.name,
            origin: spec.origin,
            params: spec.params,
            ret: spec.ret,
            locals: spec.locals,
            body: spec.body,
            instance: None,
        });
        FunctionId(self.functions.len() as u32 - 1)
    }

    /// Records that `body` lowers `instance` of a generic module (GN-15); the body's name must be the instance's, then
    /// its key params' suffix (`Ir::check`). Set after `body` so that a module as written needs no extra field in its
    /// `BodySpec`.
    pub fn mark_body_instance(&mut self, body: BodyId, instance: GenericInstance) {
        self.body_data(body).instance = Some(instance);
    }

    /// Records that `function` is `instance` of a generic item (GN-02); the function's name must
    /// be the instance's (`Ir::check`). Set after `function` so that a function as written needs
    /// no extra field in its `FunctionSpec`.
    pub fn mark_instance(&mut self, function: FunctionId, instance: GenericInstance) {
        self.functions[function.index()].instance = Some(instance);
    }

    /// Sets how `ty` crosses the device ABI. Set after the type and its `encode` and `decode`
    /// functions exist: a hand-written `Repr` names functions that mention the type (GN-05).
    pub fn set_repr(&mut self, ty: TypeId, repr: ReprForm) {
        self.types[ty.index()].repr = repr;
    }

    /// The function already added under `name`, if any.
    /// Kept for the IR builders of the test support, which look a function up by name.
    pub fn find_function(&self, name: &str) -> Option<FunctionId> {
        let at = self.functions.iter().position(|f| f.name == name)?;
        Some(FunctionId(at as u32))
    }

    pub fn expr(&mut self, kind: Expr, ty: ValueType, origin: Origin) -> ExprId {
        self.exprs.push(ExprData { kind, ty, origin });
        ExprId(self.exprs.len() as u32 - 1)
    }

    /// The form of expression `id` added before, for a pass that classifies what it just built
    /// (the elaborator reads the conditions of a contribution this way); `None` for an id this
    /// builder never handed out.
    pub fn expr_kind(&self, id: ExprId) -> Option<&Expr> {
        self.exprs.get(id.index()).map(|e| &e.kind)
    }

    /// The data of expression `id` added before; `None` for an id this builder never handed out.
    pub(crate) fn expr_data(&self, id: ExprId) -> Option<&ExprData> {
        self.exprs.get(id.index())
    }

    /// How many expressions the builder holds: the id the next one takes. A pass that builds
    /// expressions outside the elaborator's own `emit` reads this to find what it adds.
    pub fn expr_count(&self) -> usize {
        self.exprs.len()
    }

    /// The type of expression `id` added before; `None` for an id this builder never handed out.
    pub fn expr_type(&self, id: ExprId) -> Option<&ValueType> {
        self.exprs.get(id.index()).map(|e| &e.ty)
    }

    /// The origin of expression `id` added before; `None` for an id this builder never handed out.
    pub fn expr_origin(&self, id: ExprId) -> Option<&Origin> {
        self.exprs.get(id.index()).map(|e| &e.origin)
    }

    /// The statement `id` added before, for a pass that walks what it just built (the elaborator reads what a drive
    /// reads this way); `None` for an id this builder never handed out.
    pub fn stmt_kind(&self, id: StmtId) -> Option<&Stmt> {
        self.stmts.get(id.index())
    }

    /// The statement `id` added before; `None` for an id this builder never handed out.
    pub(crate) fn stmt_data(&self, id: StmtId) -> Option<&Stmt> {
        self.stmts.get(id.index())
    }

    /// The function `id` added before; `None` for an id this builder never handed out.
    pub(crate) fn function_data(&self, id: FunctionId) -> Option<&FunctionData> {
        self.functions.get(id.index())
    }

    /// The derived function of `function` against its argument `argument` the differentiation
    /// built (`None` when none is built or it is still being built).
    pub fn derived_function(&self, function: FunctionId, argument: u32) -> Option<FunctionId> {
        self.derived.get(&(function, argument)).copied().flatten()
    }

    /// Marks the derived function of `(function, argument)` as being built: `false` when it
    /// already is, which means the function reaches itself.
    pub(crate) fn begin_derived(&mut self, function: FunctionId, argument: u32) -> bool {
        self.derived.insert((function, argument), None).is_none()
    }

    /// Records the derived function of `(function, argument)` once built.
    pub(crate) fn end_derived(&mut self, function: FunctionId, argument: u32, derived: FunctionId) {
        self.derived.insert((function, argument), Some(derived));
    }

    /// Drops the mark of a derived function whose construction failed.
    pub(crate) fn abandon_derived(&mut self, function: FunctionId, argument: u32) {
        self.derived.remove(&(function, argument));
    }

    pub fn stmt(&mut self, stmt: Stmt) -> StmtId {
        self.stmts.push(stmt);
        StmtId(self.stmts.len() as u32 - 1)
    }

    pub fn node(&mut self, spec: NodeSpec) -> NodeId {
        self.nodes.push(NodeData {
            path: spec.path,
            nature: spec.nature,
            declared_by: spec.declared_by,
            init: spec.init,
            origin: spec.origin,
        });
        NodeId(self.nodes.len() as u32 - 1)
    }

    /// The id the next `instance` call returns, for nodes that name their declaring instance
    /// before it is added.
    pub fn next_instance_id(&self) -> InstanceId {
        InstanceId(self.instances.len() as u32)
    }

    pub fn instance(&mut self, spec: InstanceSpec) -> InstanceId {
        let id = InstanceId(self.instances.len() as u32);
        if let Some(parent) = spec.parent {
            if let Some(data) = self.instances.get_mut(parent.index()) {
                data.children.push(id);
            }
        }
        self.instances.push(InstanceData {
            path: spec.path,
            name: spec.name,
            body: spec.body,
            parent: spec.parent,
            children: Vec::new(),
            nodes: spec.nodes,
            params: spec.params,
            origin: spec.origin,
            nets: Vec::new(),
        });
        id
    }

    pub fn net(&mut self, spec: NetSpec) -> NetId {
        self.nets.push(NetData { path: spec.path, ty: spec.ty, declared_by: spec.declared_by, init: spec.init, origin: spec.origin, resolution: None });
        NetId(self.nets.len() as u32 - 1)
    }

    /// Records that `net` is a resolved net made of `resolution` (RN-03). An id this builder never handed out panics.
    pub fn resolve_net(&mut self, net: NetId, resolution: Resolution) {
        let count = self.nets.len();
        match self.nets.get_mut(net.index()) {
            Some(data) => data.resolution = Some(resolution),
            None => panic!("net id {} out of range ({count} nets)", net.0),
        }
    }

    /// Binds the carriers of `instance`'s body, in their order, to `nets`. An id this builder
    /// never handed out panics.
    pub fn bind_nets(&mut self, instance: InstanceId, nets: Vec<NetId>) {
        let count = self.instances.len();
        match self.instances.get_mut(instance.index()) {
            Some(data) => data.nets = nets,
            None => panic!("instance id {} out of range ({count} instances)", instance.0),
        }
    }

    /// Makes `merged` the same node as `kept`, in every instance (a fixed zero-potential branch
    /// between them: `Body::merges`). `kept` may be ground, and then `merged` is gone. Nothing
    /// changes until `finish`, which rewrites every instance's node refs and compacts the node
    /// arena, so ids handed out before stay valid while the builder is in use.
    pub fn merge_nodes(&mut self, kept: NodeRef, merged: NodeId) {
        self.union.join(kept, merged);
    }

    /// Binds slot `slot` (a node of role `Reached`) of `instance` to `node`, once the child it
    /// reaches into has been placed and has its own nodes. An id this builder never handed out
    /// panics.
    pub fn bind_reached_node(&mut self, instance: InstanceId, slot: u32, node: NodeRef) {
        let count = self.instances.len();
        let Some(data) = self.instances.get_mut(instance.index()) else { panic!("instance id {} out of range ({count} instances)", instance.0) };
        match data.nodes.get_mut(slot as usize) {
            Some(bound) => *bound = node,
            None => panic!("instance {}: node slot {slot} out of range ({} nodes)", data.path, data.nodes.len()),
        }
    }

    /// Binds carrier `carrier` (of role `Reached`) of `instance` to `net`, once the child it reads
    /// is placed. The instance's nets are bound first (`bind_nets`), with any net at this slot; it
    /// is replaced here. An id this builder never handed out panics.
    pub fn bind_reached_net(&mut self, instance: InstanceId, carrier: u32, net: NetId) {
        let count = self.instances.len();
        let Some(data) = self.instances.get_mut(instance.index()) else { panic!("instance id {} out of range ({count} instances)", instance.0) };
        match data.nets.get_mut(carrier as usize) {
            Some(bound) => *bound = net,
            None => panic!("instance {}: carrier {carrier} out of range ({} nets bound)", data.path, data.nets.len()),
        }
    }

    /// Applies the node merges: drops every merged node from the arena, renumbers the survivors
    /// in order and rewrites every instance's refs through the union.
    fn apply_merges(&mut self) {
        if self.union.merged_into.is_empty() {
            return;
        }
        let mut renumbered: Vec<Option<NodeId>> = Vec::with_capacity(self.nodes.len());
        let mut kept: Vec<NodeData> = Vec::new();
        let mut merged_away: Vec<(String, NodeId)> = Vec::new();
        for (at, node) in std::mem::take(&mut self.nodes).into_iter().enumerate() {
            if self.union.merged_into.contains_key(&NodeId(at as u32)) {
                merged_away.push((node.path.clone(), NodeId(at as u32)));
                renumbered.push(None);
            } else {
                renumbered.push(Some(NodeId(kept.len() as u32)));
                kept.push(node);
            }
        }
        self.nodes = kept;
        self.aliases = merged_away
            .into_iter()
            .map(|(path, node)| NodeAlias {
                path,
                node: match self.union.resolve(NodeRef::Node(node)) {
                    NodeRef::Node(id) => renumbered.get(id.index()).copied().flatten().map_or(NodeRef::Ground, NodeRef::Node),
                    NodeRef::Ground => NodeRef::Ground,
                },
            })
            .collect();
        for instance in &mut self.instances {
            for slot in &mut instance.nodes {
                *slot = match self.union.resolve(*slot) {
                    NodeRef::Node(id) => renumbered.get(id.index()).copied().flatten().map_or(NodeRef::Ground, NodeRef::Node),
                    NodeRef::Ground => NodeRef::Ground,
                };
            }
        }
    }

    /// Consumes the builder: the `Ir` has no writer left.
    pub fn finish(mut self, top: InstanceId) -> Ir {
        self.apply_merges();
        Ir {
            natures: self.natures,
            bodies: self.bodies,
            functions: self.functions,
            exprs: self.exprs,
            stmts: self.stmts,
            instances: self.instances,
            nodes: self.nodes,
            top,
            nets: self.nets,
            aliases: self.aliases,
            types: self.types,
            aggregates: self.aggregates,
            wides: self.wides,
        }
    }
}
