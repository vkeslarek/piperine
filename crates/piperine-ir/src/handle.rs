//! Reading: `Copy` handles that pair a borrow of the [`Ir`] with an id.

use crate::real::Real;
use crate::ids::{BodyId, ExprId, FunctionId, InstanceId, NatureId, NetId, NodeId, StmtId, TypeId, AggregateId};
use crate::model::{
    CarrierWindow, EmptyArray, NodeArray,
    BodyData, BodyNode, Branch, Carrier, CarrierRole, Child, ChildValue, Constant, DelaySite, Derivative, Drive, NoiseSource, SessionParameter, TraceSite, PartialDerivative, EmitField, Event, Expr, FunctionData, GenericInstance, Hoisted, InstanceData, Inflow, Ir, Local, Merge, Reach, ReachedItem,
    NatureData, NetData, NodeData, NodeRef, NodeRole, Origin, Param, Quantity, Reaction, Site, Stmt, Stream, ReprForm, TypeData, TypeKind, ValueType, AggregateData,
};
use crate::contributions::{Contribution, Contributions};
use crate::statements::Statements;

impl Ir {
    /// A source struct or enum of the type arena.
    pub fn type_data(&self, id: TypeId) -> &TypeData {
        &self.types[id.index()]
    }

    /// A composite constant.
    pub fn aggregate(&self, id: AggregateId) -> &AggregateData {
        &self.aggregates[id.index()]
    }

    /// The composite constants, in id order.
    pub fn aggregates(&self) -> &[AggregateData] {
        &self.aggregates
    }

    /// The scalar a pin holds for a value of `ty`: an enum whose variants have no fields is its tag
    /// (§2 rule 5), `UInt<tag_levels>`, so carried bytes are the tag's; any other type is itself.
    pub fn carried(&self, ty: &ValueType) -> ValueType {
        match ty {
            // an enum whose `Repr` the author wrote is its levels word (GN-05)
            ValueType::Enum(id) if matches!(self.types[id.index()].repr, ReprForm::HandWritten { width, .. } if width <= 64) => match self.types[id.index()].repr {
                ReprForm::HandWritten { width, .. } => ValueType::UInt(width),
                ReprForm::Derived => ty.clone(),
            },
            ValueType::Enum(id) => match &self.types[id.index()].kind {
                TypeKind::Enum { tag_levels, variants } if *tag_levels > 0 && variants.iter().all(|v| v.fields.is_empty()) => ValueType::UInt(*tag_levels),
                _ => ty.clone(),
            },
            other => other.clone(),
        }
    }

    /// `ty` as the source writes it: `f64`, `u8`, `Gains` for a struct (its bare name), `[f64; 3]`,
    /// `(u8, bool)`, `Option<u8>`. A type the source cannot write is spelt as `Display` spells it.
    pub fn written(&self, ty: &ValueType) -> String {
        match ty {
            ValueType::Tuple(parts) => {
                let parts: Vec<String> = parts.iter().map(|part| self.written(part)).collect();
                if parts.len() == 1 { format!("({},)", parts[0]) } else { format!("({})", parts.join(", ")) }
            }
            ValueType::Array(element, length) => format!("[{}; {length}]", self.written(element)),
            ValueType::Option(inner) => format!("Option<{}>", self.written(inner)),
            ValueType::Struct(id) | ValueType::Enum(id) => self.types[id.index()].name.clone(),
            other => other.to_string(),
        }
    }

    /// Whether a value of `ty` is a whole of parts a param or a pin holds as bytes: a tuple, an array, a struct, an `Option`
    /// or an enum some variant of which has fields. A unit enum is its tag and is not one (PI-02).
    pub fn is_composite_value(&self, ty: &ValueType) -> bool {
        match ty {
            ValueType::Enum(id) => matches!(&self.types[id.index()].kind, TypeKind::Enum { variants, .. } if variants.iter().any(|variant| !variant.fields.is_empty())),
            other => other.is_composite(),
        }
    }

    /// The discriminant a constant of the enum type `ty` holds: the tag a pin carries for it.
    pub fn discriminant(&self, ty: &ValueType, value: Constant) -> Option<i128> {
        let (ValueType::Enum(id), Constant::Aggregate(aggregate)) = (ty, value) else { return None };
        let variant = self.aggregates.get(aggregate.index())?.variant?;
        match &self.types.get(id.index())?.kind {
            TypeKind::Enum { variants, .. } => variants.get(variant as usize).map(|v| v.discriminant),
            TypeKind::Struct { .. } => None,
        }
    }

    /// The type arena, in id order.
    pub fn types(&self) -> &[TypeData] {
        &self.types
    }

    /// The kind of an expression. An id out of range panics; `Ir::check` is the gate that makes
    /// that unreachable.
    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.index()].kind
    }

    pub fn expr_type(&self, id: ExprId) -> &ValueType {
        &self.exprs[id.index()].ty
    }

    pub fn expr_origin(&self, id: ExprId) -> &Origin {
        &self.exprs[id.index()].origin
    }

    /// Every expression of the design, in the order they were added.
    pub fn expressions(&self) -> impl Iterator<Item = &Expr> {
        self.exprs.iter().map(|data| &data.kind)
    }

    /// The type of every expression, in id order.
    pub fn expression_types(&self) -> impl Iterator<Item = &ValueType> {
        self.exprs.iter().map(|data| &data.ty)
    }

    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.index()]
    }

    pub fn body(&self, id: BodyId) -> Body<'_> {
        Body { ir: self, id }
    }

    /// Every body, in the order they were added.
    pub fn bodies(&self) -> impl Iterator<Item = Body<'_>> {
        (0..self.bodies.len() as u32).map(|i| Body { ir: self, id: BodyId(i) })
    }

    pub fn function(&self, id: FunctionId) -> Function<'_> {
        Function { ir: self, id }
    }

    /// Every function, in the order they were added.
    pub fn functions(&self) -> impl Iterator<Item = Function<'_>> {
        (0..self.functions.len() as u32).map(|i| Function { ir: self, id: FunctionId(i) })
    }

    /// The instance the design is rooted at.
    pub fn top(&self) -> Instance<'_> {
        Instance { ir: self, id: self.top }
    }

    pub fn instance(&self, id: InstanceId) -> Instance<'_> {
        Instance { ir: self, id }
    }

    /// Every instance, in pre-order of the hierarchy.
    pub fn instances(&self) -> impl Iterator<Item = Instance<'_>> {
        (0..self.instances.len() as u32).map(|i| Instance { ir: self, id: InstanceId(i) })
    }

    pub fn node(&self, id: NodeId) -> Node<'_> {
        Node { ir: self, id }
    }

    /// Every node of the design, in the order they were added.
    pub fn nodes(&self) -> impl Iterator<Item = Node<'_>> {
        (0..self.nodes.len() as u32).map(|i| Node { ir: self, id: NodeId(i) })
    }

    /// The instance with this path, or nothing.
    pub fn instance_at(&self, path: &str) -> Option<Instance<'_>> {
        self.instances().find(|i| i.path() == path)
    }

    /// Decodes one record the solver delivered: `pin` is its path (`<instance path>.<stream>`,
    /// abi.md C5) and `reaction`/`statement` the site the record carries. Gives the instance
    /// that made it and the stream and text fields of its `emit!`; `None` when no instance has
    /// that path, the statement is no emit, or the emit writes another stream. The one decoder a
    /// host needs: it reads nothing but the record, so it holds whatever order the records of
    /// one microstep arrive in.
    pub fn record_at(&self, pin: &str, reaction: u32, statement: u32) -> Option<(Instance<'_>, RecordSite<'_>)> {
        let (path, stream) = pin.rsplit_once('.')?;
        let instance = self.instance_at(path)?;
        let site = instance.body().record_site(reaction, statement)?;
        (site.stream == stream).then_some((instance, site))
    }

    /// The node with this path, or nothing. The path of a node a merge removed (a fixed
    /// zero-potential branch joined it to another) is the surviving node it stands for now.
    pub fn node_at(&self, path: &str) -> Option<Node<'_>> {
        self.nodes().find(|n| n.path() == path).or_else(|| match self.alias_at(path)? {
            NodeRef::Node(id) => Some(Node { ir: self, id }),
            NodeRef::Ground => None,
        })
    }

    /// Whether `path` names a node that a merge joined to ground: it has no node of its own, and its
    /// potential is zero.
    pub fn is_ground_alias(&self, path: &str) -> bool {
        self.alias_at(path) == Some(NodeRef::Ground)
    }

    /// What the merged-away node `path` stands for now.
    fn alias_at(&self, path: &str) -> Option<NodeRef> {
        self.aliases.iter().find(|a| a.path == path).map(|a| a.node)
    }

    pub fn net(&self, id: NetId) -> Net<'_> {
        Net { ir: self, id }
    }

    /// Every discrete net of the design, in the order they were added.
    pub fn nets(&self) -> impl Iterator<Item = Net<'_>> {
        (0..self.nets.len() as u32).map(|i| Net { ir: self, id: NetId(i) })
    }

    /// The net with this path, or nothing.
    pub fn net_at(&self, path: &str) -> Option<Net<'_>> {
        self.nets().find(|n| n.path() == path)
    }

    pub fn nature(&self, id: NatureId) -> Nature<'_> {
        Nature { ir: self, id }
    }

    /// Every nature, in the order they were added.
    pub fn natures(&self) -> impl Iterator<Item = Nature<'_>> {
        (0..self.natures.len() as u32).map(|i| Nature { ir: self, id: NatureId(i) })
    }
}

/// One module variant.
#[derive(Debug, Clone, Copy)]
pub struct Body<'a> {
    ir: &'a Ir,
    id: BodyId,
}

impl<'a> Body<'a> {
    fn data(&self) -> &'a BodyData {
        &self.ir.bodies[self.id.index()]
    }

    pub fn id(&self) -> BodyId {
        self.id
    }

    pub fn ir(&self) -> &'a Ir {
        self.ir
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    pub fn params(&self) -> &'a [Param] {
        &self.data().params
    }

    /// The `where` text of param `param` (its index in `params`), as the printer writes it;
    /// `None` for a param with no `where`.
    pub fn param_check_text(&self, param: usize) -> Option<String> {
        self.ir.param_check_text(self.data(), param)
    }

    /// Ports first, then signals.
    pub fn nodes(&self) -> &'a [BodyNode] {
        &self.data().nodes
    }

    pub fn branches(&self) -> &'a [Branch] {
        &self.data().branches
    }

    pub fn limits(&self) -> &'a [Site] {
        &self.data().limits
    }

    pub fn stims(&self) -> &'a [Site] {
        &self.data().stims
    }

    pub fn locals(&self) -> &'a [Local] {
        &self.data().locals
    }

    /// The statements of the `analog` block; `None` when the body has none.
    pub fn analog(&self) -> Option<&'a [StmtId]> {
        self.data().analog.as_deref()
    }

    /// Every contribution of the `analog` block, nested ones included, each with the conditions
    /// that enclose it, in source order. Empty for a body with no `analog` block.
    pub fn contributions(&self) -> Vec<Contribution<'a>> {
        Contributions::new(self.ir).of(self.analog().unwrap_or_default())
    }

    /// The modules placed in an expression whose nodes or `analog` statements joined this body.
    pub fn hoisted(&self) -> &'a [Hoisted] {
        &self.data().hoisted
    }

    /// The branches dropped as a node merge: each is one node in every instance.
    pub fn merges(&self) -> &'a [Merge] {
        &self.data().merges
    }

    /// The `pub` nodes and carriers of children this body reads (`inst.name`).
    pub fn reaches(&self) -> &'a [Reach] {
        &self.data().reaches
    }

    /// The `p.inflow()` ammeters, one per port.
    pub fn inflows(&self) -> &'a [Inflow] {
        &self.data().inflows
    }

    /// The derivative sites, by index (`Expr::Derivative`).
    pub fn derivatives(&self) -> &'a [Derivative] {
        &self.data().derivatives
    }

    /// The delay sites, by index (`Expr::Delay`).
    pub fn delays(&self) -> &'a [DelaySite] {
        &self.data().delays
    }

    /// The noise sources, by index (`Expr::Noise`).
    pub fn noise_sources(&self) -> &'a [NoiseSource] {
        &self.data().noise_sources
    }

    /// The session parameters the body reads, by index (`Expr::SessionParameter`).
    pub fn session_parameters(&self) -> &'a [SessionParameter] {
        &self.data().session_parameters
    }

    /// The `ddx` calls the elaborator differentiated, by index.
    pub fn partial_derivatives(&self) -> &'a [PartialDerivative] {
        &self.data().partial_derivatives
    }

    /// The text fields of trace site `site`, as `(name, text)`, in source order: what the site's
    /// `emit!` wrote that never crosses the ABI. Empty for a site the body does not have.
    pub fn trace_texts(&self, site: u32) -> Vec<(&'a str, &'a str)> {
        let ir = self.ir;
        let analog = self.data().analog.as_deref().unwrap_or(&[]);
        Statements::of(ir, analog)
            .into_iter()
            .find_map(|id| match ir.stmt(id) {
                Stmt::Trace { site: s, fields } if *s == site => Some(
                    fields
                        .iter()
                        .filter_map(|f| match f {
                            EmitField::Text { name, text } => Some((name.as_str(), text.as_str())),
                            EmitField::Value { .. } => None,
                        })
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The trace sites, by index (`Stmt::Trace`).
    pub fn traces(&self) -> &'a [TraceSite] {
        &self.data().traces
    }

    /// The authored placements, in source order.
    pub fn children(&self) -> &'a [Child] {
        &self.data().children
    }

    /// The param values of child `child` (an index into `children`) that `Child::params` does
    /// not hold, in the order they were set: the expressions over this body's params (and the
    /// constants set beside them). Empty for a child with none, or out of range.
    pub fn child_params(&self, child: usize) -> &'a [(u32, ChildValue)] {
        self.data().child_params.get(child).map_or(&[], Vec::as_slice)
    }

    /// What child `child` (an index into `children`) binds at each carrier port of its body, in
    /// the port order (the `in` and `out` carriers, signals left out): the index of the parent's
    /// carrier, or `None` for an open port. Empty for a child with none recorded, or out of range.
    pub fn child_carriers(&self, child: usize) -> &'a [Option<u32>] {
        self.data().child_carriers.get(child).map_or(&[], Vec::as_slice)
    }

    /// Whether a parent body reads carrier `c` of this body as `inst.name` (a `Reach` into a child
    /// that places this body): the carrier is then met on a net, as a port is.
    pub fn is_reached(&self, c: u32) -> bool {
        self.ir.bodies.iter().any(|parent| {
            parent.reaches.iter().any(|r| matches!(r.item, ReachedItem::Carrier { child_carrier, .. } if child_carrier == c) && parent.children.get(r.child as usize).is_some_and(|child| child.body == self.id))
        })
    }

    /// The discrete carriers, ports first, then signals.
    pub fn carriers(&self) -> &'a [Carrier] {
        &self.data().carriers
    }

    /// The arrays declared with no element: no carrier and no node stand for them.
    pub fn empty_arrays(&self) -> &'a [EmptyArray] {
        &self.data().empty_arrays
    }

    /// The arrays of nodes: each names consecutive entries of the body's nodes.
    pub fn node_arrays(&self) -> &'a [NodeArray] {
        &self.data().node_arrays
    }

    /// The part of an array of carriers each carrier port of child `child` is given, parallel to the child's carrier
    /// bindings; empty for a child with none, or out of range.
    pub fn child_windows(&self, child: usize) -> &'a [Option<CarrierWindow>] {
        self.data().child_windows.get(child).map_or(&[], Vec::as_slice)
    }

    /// The definitions of carriers, in source order.
    pub fn drives(&self) -> &'a [Drive] {
        &self.data().drives
    }

    /// The events the reactions wait on, by index.
    pub fn events(&self) -> &'a [Event] {
        &self.data().events
    }

    /// The reactions of the body's `digital` blocks, in source order.
    pub fn reactions(&self) -> &'a [Reaction] {
        &self.data().reactions
    }

    /// The record streams the body's `emit!` sites write, by index.
    pub fn streams(&self) -> &'a [Stream] {
        &self.data().streams
    }

    /// The commits and emits of reaction `reaction`, in source order, nested ones included: the
    /// ABI's statement numbering counts them together, so statement `i` of the reaction is
    /// entry `i` of this list. A reaction out of range has none.
    pub fn reaction_statements(&self, reaction: u32) -> Vec<StmtId> {
        match self.data().reactions.get(reaction as usize) {
            Some(r) => Statements::of(self.ir, &r.stmts),
            None => Vec::new(),
        }
    }

    /// Where a record delivered by the solver came from: the stream and the text fields of the
    /// emit that is statement `statement` of reaction `reaction`. `None` when that statement is a
    /// commit, or there is none (a host decodes a record from the instance, the reaction and the
    /// statement the solver's sink gives it).
    pub fn record_site(&self, reaction: u32, statement: u32) -> Option<RecordSite<'a>> {
        let id = *self.reaction_statements(reaction).get(statement as usize)?;
        let Stmt::Emit { stream, fields } = self.ir.stmt(id) else { return None };
        let texts = fields
            .iter()
            .filter_map(|f| match f {
                EmitField::Text { name, text } => Some((name.as_str(), text.as_str())),
                EmitField::Value { .. } => None,
            })
            .collect();
        Some(RecordSite { stream: self.data().streams.get(*stream as usize)?.name.as_str(), texts })
    }
}

/// The source of one record: the stream it belongs to and the text fields its site wrote, which
/// never cross the ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSite<'a> {
    pub stream: &'a str,
    /// `(field, text)` in source order.
    pub texts: Vec<(&'a str, &'a str)>,
}

/// A nature.
#[derive(Debug, Clone, Copy)]
pub struct Nature<'a> {
    ir: &'a Ir,
    id: NatureId,
}

impl<'a> Nature<'a> {
    fn data(&self) -> &'a NatureData {
        &self.ir.natures[self.id.index()]
    }

    pub fn id(&self) -> NatureId {
        self.id
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn potential(&self) -> &'a Quantity {
        &self.data().potential
    }

    pub fn flow(&self) -> Option<&'a Quantity> {
        self.data().flow.as_ref()
    }
}

/// A function.
#[derive(Debug, Clone, Copy)]
pub struct Function<'a> {
    ir: &'a Ir,
    id: FunctionId,
}

impl<'a> Function<'a> {
    fn data(&self) -> &'a FunctionData {
        &self.ir.functions[self.id.index()]
    }

    pub fn id(&self) -> FunctionId {
        self.id
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    pub fn params(&self) -> &'a [Local] {
        &self.data().params
    }

    pub fn ret(&self) -> &'a ValueType {
        &self.data().ret
    }

    /// The locals, parameters first.
    pub fn locals(&self) -> &'a [Local] {
        &self.data().locals
    }

    pub fn body(&self) -> ExprId {
        self.data().body
    }

    /// The generic item this function instantiates, `None` for a function as written.
    pub fn instance(&self) -> Option<&'a GenericInstance> {
        self.data().instance.as_ref()
    }
}

/// A placed instance of the elaborated design.
#[derive(Debug, Clone, Copy)]
pub struct Instance<'a> {
    ir: &'a Ir,
    id: InstanceId,
}

impl<'a> Instance<'a> {
    fn data(&self) -> &'a InstanceData {
        &self.ir.instances[self.id.index()]
    }

    pub fn id(&self) -> InstanceId {
        self.id
    }

    pub fn path(&self) -> &'a str {
        &self.data().path
    }

    pub fn name(&self) -> &'a str {
        &self.data().name
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    pub fn body(&self) -> Body<'a> {
        Body { ir: self.ir, id: self.data().body }
    }

    pub fn parent(&self) -> Option<Instance<'a>> {
        self.data().parent.map(|id| Instance { ir: self.ir, id })
    }

    /// The instances placed inside this one, in declaration order.
    pub fn children(&self) -> impl Iterator<Item = Instance<'a>> {
        let ir = self.ir;
        self.data().children.iter().map(move |&id| Instance { ir, id })
    }

    /// One binding per node of the body, in the body's order.
    pub fn bindings(&self) -> &'a [NodeRef] {
        &self.data().nodes
    }

    /// What node `i` of the body is bound to.
    pub fn binding(&self, i: usize) -> Option<NodeRef> {
        self.data().nodes.get(i).copied()
    }

    /// The design node bound to node `i` of the body; `None` for ground or an index out of range.
    pub fn node(&self, i: usize) -> Option<Node<'a>> {
        match self.binding(i)? {
            NodeRef::Node(id) => Some(Node { ir: self.ir, id }),
            NodeRef::Ground => None,
        }
    }

    /// One bound value per param of the body, defaults applied.
    pub fn params(&self) -> &'a [Constant] {
        &self.data().params
    }

    pub fn param(&self, i: usize) -> Option<&'a Constant> {
        self.data().params.get(i)
    }

    /// One net per carrier of the body, in the body's order.
    pub fn nets(&self) -> &'a [NetId] {
        &self.data().nets
    }

    /// The design net bound to carrier `i` of the body; `None` for an index out of range.
    pub fn net(&self, i: usize) -> Option<Net<'a>> {
        self.data().nets.get(i).map(|&id| Net { ir: self.ir, id })
    }
}

/// A node of the elaborated design.
#[derive(Debug, Clone, Copy)]
pub struct Node<'a> {
    ir: &'a Ir,
    id: NodeId,
}

impl<'a> Node<'a> {
    fn data(&self) -> &'a NodeData {
        &self.ir.nodes[self.id.index()]
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn path(&self) -> &'a str {
        &self.data().path
    }

    pub fn nature(&self) -> Nature<'a> {
        Nature { ir: self.ir, id: self.data().nature }
    }

    pub fn declared_by(&self) -> Instance<'a> {
        Instance { ir: self.ir, id: self.data().declared_by }
    }

    pub fn init(&self) -> Option<Real> {
        self.data().init
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    /// The instances this node is wired to through a port of their body, in pre-order. The
    /// instance that declares a signal binds it as a signal of its own body and is not listed.
    pub fn instances(&self) -> Vec<Instance<'a>> {
        let ir = self.ir;
        ir.instances()
            .filter(|inst| {
                let body = inst.body();
                inst.bindings().iter().enumerate().any(|(i, binding)| {
                    *binding == NodeRef::Node(self.id)
                        && body.nodes().get(i).is_some_and(|n| n.role == NodeRole::Port)
                })
            })
            .collect()
    }
}

/// A discrete net of the elaborated design.
#[derive(Debug, Clone, Copy)]
pub struct Net<'a> {
    ir: &'a Ir,
    id: NetId,
}

impl<'a> Net<'a> {
    fn data(&self) -> &'a NetData {
        &self.ir.nets[self.id.index()]
    }

    pub fn id(&self) -> NetId {
        self.id
    }

    pub fn path(&self) -> &'a str {
        &self.data().path
    }

    /// The value type every carrier bound to it holds.
    pub fn ty(&self) -> &'a ValueType {
        &self.data().ty
    }

    pub fn declared_by(&self) -> Instance<'a> {
        Instance { ir: self.ir, id: self.data().declared_by }
    }

    pub fn init(&self) -> Option<Constant> {
        self.data().init
    }

    pub fn origin(&self) -> &'a Origin {
        &self.data().origin
    }

    /// The instances this net is wired to through a port carrier of their body, in pre-order. The
    /// instance that declares it binds it as a signal of its own body and is not listed.
    pub fn instances(&self) -> Vec<Instance<'a>> {
        let ir = self.ir;
        ir.instances()
            .filter(|inst| {
                let body = inst.body();
                inst.nets().iter().enumerate().any(|(i, net)| {
                    *net == self.id && body.carriers().get(i).is_some_and(|c| matches!(c.role, CarrierRole::In | CarrierRole::Out))
                })
            })
            .collect()
    }
}
