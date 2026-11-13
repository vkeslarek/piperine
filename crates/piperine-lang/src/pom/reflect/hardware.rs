//! Handles over hardware declarations: natures and their quantities, modules
//! with their ports and body items, generation, behavior blocks and
//! reactions.

use super::body::{Commit, Contribution, Expr, Pat, Path, Type, nested_stmts};
use super::{Decl, Reflect};
use crate::pom::body::{Block, BehaviorKind, QuantityRole, StmtKind, TypeKind};
use crate::pom::decl::{
    BehaviorData, BindingData, BindingOrigin, DeclKind, DefinitionData, Dir, GenElseNode, GenerationData, LetData,
    ModuleData, NatureData, NatureFieldNode, ParamData, PortData, QuantityData, ReactionData, SignalData,
};
use crate::pom::design::Design;
use crate::pom::ids::DeclId;
use crate::pom::resolution::{NatureKind, PortKind};
use crate::pom::body::LitKind;

/// The declarations of a module-body or digital-block item list, in source
/// order, with the items of every generation in place of the generation.
fn deep_items<'d>(parent: Decl<'d>, out: &mut Vec<Decl<'d>>) {
    for child in parent.children() {
        if matches!(child.kind(), DeclKind::Generation(_)) {
            deep_items(child, out);
        } else {
            out.push(child);
        }
    }
}

fn by_name<'d, T: Reflect<'d>>(items: Vec<T>, name: &str) -> Option<T> {
    items.into_iter().find(|i| i.name() == Some(name))
}

// ---- natures ---------------------------------------------------------------------------------

/// `nature Name { potential: .., flow: .., value: .., resolve: .. }`
#[derive(Clone, Copy)]
pub struct Nature<'d> {
    decl: Decl<'d>,
}

impl<'d> Nature<'d> {
    fn data(self) -> &'d NatureData {
        match self.decl.kind() {
            DeclKind::Nature(n) => n,
            _ => unreachable!("Nature handle over a non-nature declaration"),
        }
    }

    /// `Conservative`, `SignalFlow`, `Discrete`, `Resolved` or `Invalid`, by
    /// the fields declared.
    pub fn kind(self) -> NatureKind {
        self.data().kind
    }

    pub fn generics(self) -> Vec<super::GenericParam<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generic_param()).collect()
    }

    /// The quantities in source order, repeated ones included.
    pub fn quantities(self) -> Vec<Quantity<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_quantity()).collect()
    }

    /// The first `potential` quantity.
    pub fn potential(self) -> Option<Quantity<'d>> {
        self.quantities().into_iter().find(|q| q.role() == QuantityRole::Potential)
    }

    /// The first `flow` quantity.
    pub fn flow(self) -> Option<Quantity<'d>> {
        self.quantities().into_iter().find(|q| q.role() == QuantityRole::Flow)
    }

    /// The `impl` blocks written for this nature, in source order.
    pub fn impls(self) -> Vec<super::Impl<'d>> {
        let design = self.decl.design();
        design.impls_of(self.decl.id()).into_iter().filter_map(|i| Decl::new(design, i).as_impl()).collect()
    }

    /// The type of the first `value: T` field.
    pub fn value_type(self) -> Option<Type<'d>> {
        self.data().fields.iter().find_map(|f| match f {
            NatureFieldNode::Value { ty, .. } => Some(self.decl.design().ty(*ty)),
            _ => None,
        })
    }

    /// The expression of the first `resolve: f` field.
    pub fn resolve_fn(self) -> Option<Expr<'d>> {
        self.data().fields.iter().find_map(|f| match f {
            NatureFieldNode::Resolve { expr, .. } => Some(self.decl.design().expr(*expr)),
            _ => None,
        })
    }
}

/// `potential: Voltage { access: v, units: "V", abstol: 1u }`
#[derive(Clone, Copy)]
pub struct Quantity<'d> {
    decl: Decl<'d>,
}

impl<'d> Quantity<'d> {
    fn data(self) -> &'d QuantityData {
        match self.decl.kind() {
            DeclKind::Quantity(q) => q,
            _ => unreachable!("Quantity handle over a non-quantity declaration"),
        }
    }

    pub fn role(self) -> QuantityRole {
        self.data().role
    }

    /// The value of the attribute `name`, as written.
    fn attr_value(self, name: &str) -> Option<Expr<'d>> {
        let init = self.data().attrs.iter().find(|a| a.name.text == name)?;
        Some(self.decl.design().expr(init.value))
    }

    /// The `access: v` attribute: the name of the access function.
    pub fn access(self) -> Option<Expr<'d>> {
        self.attr_value("access")
    }

    pub fn units(self) -> Option<Expr<'d>> {
        self.attr_value("units")
    }

    pub fn abstol(self) -> Option<Expr<'d>> {
        self.attr_value("abstol")
    }
}

// ---- modules ---------------------------------------------------------------------------------

/// `[extern "abi"] module Name<..>(ports) -> ret where cond { body }`
#[derive(Clone, Copy)]
pub struct Module<'d> {
    decl: Decl<'d>,
}

impl<'d> Module<'d> {
    fn data(self) -> &'d ModuleData {
        match self.decl.kind() {
            DeclKind::Module(m) => m,
            _ => unreachable!("Module handle over a non-module declaration"),
        }
    }

    pub fn generics(self) -> Vec<super::GenericParam<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generic_param()).collect()
    }

    /// The behavior blocks of this module (in its file), in source order.
    pub fn behaviors(self) -> Vec<Behavior<'d>> {
        let design = self.decl.design();
        self.data().behaviors.iter().filter_map(|b| Decl::new(design, *b).as_behavior()).collect()
    }

    /// The `analog` blocks of this module, in source order.
    pub fn analog_blocks(self) -> Vec<Behavior<'d>> {
        self.behaviors().into_iter().filter(|b| b.kind() == BehaviorKind::Analog).collect()
    }

    /// The `digital` blocks of this module, in source order.
    pub fn digital_blocks(self) -> Vec<Behavior<'d>> {
        self.behaviors().into_iter().filter(|b| b.kind() == BehaviorKind::Digital).collect()
    }

    /// The ports in source order.
    pub fn ports(self) -> Vec<Port<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_port()).collect()
    }

    pub fn port(self, name: &str) -> Option<Port<'d>> {
        by_name(self.ports(), name)
    }

    /// The body's items in source order, generation held as one item.
    pub fn items(self) -> Vec<Decl<'d>> {
        let is_body = |k: &DeclKind| {
            matches!(
                k,
                DeclKind::Param(_) | DeclKind::Signal(_) | DeclKind::Let(_) | DeclKind::Definition(_) | DeclKind::Generation(_) | DeclKind::Error
            )
        };
        self.decl.children().into_iter().filter(|c| is_body(c.kind())).collect()
    }

    fn deep(self) -> Vec<Decl<'d>> {
        let mut out = Vec::new();
        deep_items(self.decl, &mut out);
        out
    }

    /// The params in source order.
    pub fn params(self) -> Vec<Param<'d>> {
        self.deep().into_iter().filter_map(|c| c.as_param()).collect()
    }

    pub fn param(self, name: &str) -> Option<Param<'d>> {
        by_name(self.params(), name)
    }

    /// The signals in source order, including those written inside
    /// generation.
    pub fn signals(self) -> Vec<Signal<'d>> {
        self.deep().into_iter().filter_map(|c| c.as_signal()).collect()
    }

    pub fn signal(self, name: &str) -> Option<Signal<'d>> {
        by_name(self.signals(), name)
    }

    /// The `let` items in source order, including those inside generation.
    pub fn lets(self) -> Vec<Let<'d>> {
        self.deep().into_iter().filter_map(|c| c.as_let()).collect()
    }

    /// The named expression `name` introduced by a body `let`.
    pub fn named(self, name: &str) -> Option<Binding<'d>> {
        self.lets().into_iter().flat_map(|l| l.bindings()).find(|b| b.name() == Some(name))
    }

    /// The definitions in source order, including those inside generation.
    pub fn definitions(self) -> Vec<Definition<'d>> {
        self.deep().into_iter().filter_map(|c| c.as_definition()).collect()
    }

    /// The generation items written directly in the body.
    pub fn generations(self) -> Vec<Generation<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generation()).collect()
    }

    /// What the name `name` denotes in the module: a port, param, signal or
    /// `let`-named expression, as a declaration.
    pub fn member(self, name: &str) -> Option<Decl<'d>> {
        let named = |d: &Decl<'d>| d.name() == Some(name);
        if let Some(p) = self.ports().into_iter().map(|p| p.decl()).find(named) {
            return Some(p);
        }
        if let Some(d) = self.deep().into_iter().find(|d| named(d) && !matches!(d.kind(), DeclKind::Let(_))) {
            return Some(d);
        }
        self.named(name).map(|b| b.decl())
    }

    /// The trailing expression: the module's result.
    pub fn result(self) -> Option<Expr<'d>> {
        self.data().result.map(|e| self.decl.design().expr(e))
    }

    /// The `-> T` type.
    pub fn result_type(self) -> Option<Type<'d>> {
        self.data().ret.map(|t| self.decl.design().ty(t))
    }

    pub fn where_clause(self) -> Option<Expr<'d>> {
        self.data().where_.map(|e| self.decl.design().expr(e))
    }

    pub fn is_extern(self) -> bool {
        self.data().abi.is_some()
    }

    /// The ABI string of `extern "abi" module`, without quotes.
    pub fn abi(self) -> Option<&'d str> {
        match &self.data().abi {
            Some(l) => match &l.kind {
                LitKind::Str(text) => Some(text.as_str()),
                _ => None,
            },
            None => None,
        }
    }
}

/// One port of a module.
#[derive(Clone, Copy)]
pub struct Port<'d> {
    decl: Decl<'d>,
}

impl<'d> Port<'d> {
    fn data(self) -> &'d PortData {
        match self.decl.kind() {
            DeclKind::Port(p) => p,
            _ => unreachable!("Port handle over a non-port declaration"),
        }
    }

    /// The 0-based position among the module's ports.
    pub fn index(self) -> usize {
        match self.declared_in().and_then(|m| m.as_module()) {
            Some(m) => m.ports().iter().position(|p| *p == self).unwrap_or(0),
            None => 0,
        }
    }

    pub fn direction(self) -> Option<Dir> {
        self.data().dir
    }

    pub fn ty(self) -> Type<'d> {
        self.decl.design().ty(self.data().ty)
    }

    pub fn default(self) -> Option<Expr<'d>> {
        self.data().default.map(|e| self.decl.design().expr(e))
    }

    /// The classification of the port (`Unknown` until classified).
    pub fn kind(self) -> PortKind {
        self.data().kind
    }

    /// The nature of the port's type, through arrays and `flip`.
    pub fn nature(self) -> Option<Nature<'d>> {
        let design = self.decl.design();
        design.nature_of(self.data().ty).and_then(|n| Decl::new(design, n).as_nature())
    }
}

/// `name: ty = default where cond;` in a module body.
#[derive(Clone, Copy)]
pub struct Param<'d> {
    decl: Decl<'d>,
}

impl<'d> Param<'d> {
    fn data(self) -> &'d ParamData {
        match self.decl.kind() {
            DeclKind::Param(p) => p,
            _ => unreachable!("Param handle over a non-param declaration"),
        }
    }

    pub fn ty(self) -> Type<'d> {
        self.decl.design().ty(self.data().ty)
    }

    pub fn default(self) -> Option<Expr<'d>> {
        self.data().default.map(|e| self.decl.design().expr(e))
    }

    pub fn where_clause(self) -> Option<Expr<'d>> {
        self.data().where_.map(|e| self.decl.design().expr(e))
    }

    /// Syntactic: no default, and a type that is not a path ending in
    /// `Option`.
    pub fn is_required(self) -> bool {
        if self.data().default.is_some() {
            return false;
        }
        let design = self.decl.design();
        match &design.type_node(self.data().ty).kind {
            TypeKind::Path(p) => design.path_node(*p).segments.last().is_none_or(|s| s.name.text != "Option"),
            _ => true,
        }
    }
}

/// `signal name: ty = init;`
#[derive(Clone, Copy)]
pub struct Signal<'d> {
    decl: Decl<'d>,
}

impl<'d> Signal<'d> {
    fn data(self) -> &'d SignalData {
        match self.decl.kind() {
            DeclKind::Signal(s) => s,
            _ => unreachable!("Signal handle over a non-signal declaration"),
        }
    }

    pub fn ty(self) -> Type<'d> {
        self.decl.design().ty(self.data().ty)
    }

    pub fn init(self) -> Option<Expr<'d>> {
        self.data().init.map(|e| self.decl.design().expr(e))
    }

    /// The nature of the signal's type, through arrays and `flip`.
    pub fn nature(self) -> Option<Nature<'d>> {
        let design = self.decl.design();
        design.nature_of(self.data().ty).and_then(|n| Decl::new(design, n).as_nature())
    }

    /// The generation the signal is written in, if its parent is one.
    pub fn generation(self) -> Option<Generation<'d>> {
        self.declared_in().and_then(|d| d.as_generation())
    }
}

/// `let pat = value;` in a module body.
#[derive(Clone, Copy)]
pub struct Let<'d> {
    decl: Decl<'d>,
}

impl<'d> Let<'d> {
    fn data(self) -> &'d LetData {
        match self.decl.kind() {
            DeclKind::Let(l) => l,
            _ => unreachable!("Let handle over a non-let declaration"),
        }
    }

    pub fn pattern(self) -> Pat<'d> {
        self.decl.design().pat(self.data().pat)
    }

    /// The whole value expression.
    pub fn value(self) -> Expr<'d> {
        self.decl.design().expr(self.data().value)
    }

    /// The names the pattern introduces, in source order.
    pub fn bindings(self) -> Vec<Binding<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_binding()).collect()
    }
}

/// One identifier a pattern introduces.
#[derive(Clone, Copy)]
pub struct Binding<'d> {
    decl: Decl<'d>,
}

impl<'d> Binding<'d> {
    fn data(self) -> &'d BindingData {
        match self.decl.kind() {
            DeclKind::Binding(b) => b,
            _ => unreachable!("Binding handle over a non-binding declaration"),
        }
    }

    pub fn origin(self) -> BindingOrigin {
        self.data().origin
    }

    /// The pattern that introduces the name.
    pub fn pattern(self) -> Pat<'d> {
        self.decl.design().pat(self.data().pat)
    }

    /// The `let` item the name comes from, for a body `let`.
    pub fn let_item(self) -> Option<Let<'d>> {
        self.declared_in().and_then(|d| d.as_let())
    }

    /// The value of the `let` the name comes from: the whole expression,
    /// the pattern is not matched against it.
    pub fn value(self) -> Option<Expr<'d>> {
        self.let_item().map(|l| l.value())
    }
}

/// `place = value after delay;`
#[derive(Clone, Copy)]
pub struct Definition<'d> {
    decl: Decl<'d>,
}

impl<'d> Definition<'d> {
    fn data(self) -> &'d DefinitionData {
        match self.decl.kind() {
            DeclKind::Definition(d) => d,
            _ => unreachable!("Definition handle over a non-definition declaration"),
        }
    }

    pub fn place(self) -> Expr<'d> {
        self.decl.design().expr(self.data().place)
    }

    pub fn value(self) -> Expr<'d> {
        self.decl.design().expr(self.data().value)
    }

    pub fn after(self) -> Option<Expr<'d>> {
        self.data().after.map(|e| self.decl.design().expr(e))
    }
}

// ---- generation ------------------------------------------------------------------------------

/// A generation `for` or `if` over module or digital items.
#[derive(Clone, Copy)]
pub struct Generation<'d> {
    decl: Decl<'d>,
}

impl<'d> Generation<'d> {
    fn data(self) -> &'d GenerationData {
        match self.decl.kind() {
            DeclKind::Generation(g) => g,
            _ => unreachable!("Generation handle over a non-generation declaration"),
        }
    }

    pub fn is_for(self) -> bool {
        matches!(self.data(), GenerationData::For { .. })
    }

    /// The pattern of a `for`.
    pub fn pattern(self) -> Option<Pat<'d>> {
        match self.data() {
            GenerationData::For { pat, .. } => Some(self.decl.design().pat(*pat)),
            GenerationData::If { .. } => None,
        }
    }

    /// The iterated expression of a `for`.
    pub fn iter(self) -> Option<Expr<'d>> {
        match self.data() {
            GenerationData::For { iter, .. } => Some(self.decl.design().expr(*iter)),
            GenerationData::If { .. } => None,
        }
    }

    /// The items of the `for` body, or of the first arm of an `if`.
    pub fn items(self) -> Vec<Decl<'d>> {
        let design = self.decl.design();
        match self.data() {
            GenerationData::For { body, .. } => body.iter().map(|d| Decl::new(design, *d)).collect(),
            GenerationData::If { then, .. } => then.iter().map(|d| Decl::new(design, *d)).collect(),
        }
    }

    /// The arms of an `if` chain in order: each `if` and `else if`
    /// condition, then the final `else` with none. A `for` has no arms.
    pub fn arms(self) -> Vec<GenArm<'d>> {
        let design = self.decl.design();
        let mut arms = Vec::new();
        let mut at = Some(self);
        while let Some(g) = at.take() {
            let GenerationData::If { cond, then, else_ } = g.data() else { break };
            arms.push(GenArm { design, condition: Some(*cond), items: then });
            match else_ {
                Some(GenElseNode::If(next)) => at = Decl::new(design, *next).as_generation(),
                Some(GenElseNode::Block(items)) => arms.push(GenArm { design, condition: None, items }),
                None => {}
            }
        }
        arms
    }

    /// The generations written directly inside this one (an `else if` is
    /// one of them).
    pub fn generations(self) -> Vec<Generation<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generation()).collect()
    }
}

/// One arm of a generation `if` chain.
#[derive(Clone, Copy)]
pub struct GenArm<'d> {
    design: &'d Design,
    condition: Option<crate::pom::ids::ExprId>,
    items: &'d [DeclId],
}

impl<'d> GenArm<'d> {
    /// The arm's condition; none for the final `else`.
    pub fn condition(self) -> Option<Expr<'d>> {
        self.condition.map(|e| self.design.expr(e))
    }

    pub fn items(self) -> Vec<Decl<'d>> {
        self.items.iter().map(|d| Decl::new(self.design, *d)).collect()
    }
}

// ---- behavior --------------------------------------------------------------------------------

/// `analog<N> for Target { .. }` or `digital for Target { .. }`
#[derive(Clone, Copy)]
pub struct Behavior<'d> {
    decl: Decl<'d>,
}

impl<'d> Behavior<'d> {
    fn data(self) -> &'d BehaviorData {
        match self.decl.kind() {
            DeclKind::Behavior(b) => b,
            _ => unreachable!("Behavior handle over a non-behavior declaration"),
        }
    }

    pub fn kind(self) -> BehaviorKind {
        self.data().kind
    }

    /// The generic names of `analog<N, M>`, in order.
    pub fn names(self) -> Vec<&'d str> {
        self.data().names.iter().map(|n| n.text.as_str()).collect()
    }

    /// The module this block belongs to: the one its target names, when that
    /// module is declared in the same file and is not `extern`.
    pub fn module(self) -> Option<Module<'d>> {
        self.data().module.and_then(|m| Decl::new(self.decl.design(), m).as_module())
    }

    /// The target path as written, generic arguments included.
    pub fn target(self) -> Path<'d> {
        self.decl.design().path(self.data().target)
    }

    /// The statements of an `analog` block; none for `digital`.
    pub fn statements(self) -> Vec<super::Stmt<'d>> {
        self.data().body.iter().map(|s| self.decl.design().stmt(*s)).collect()
    }

    /// Every contribution of an `analog` block at any depth, in source order.
    pub fn contributions(self) -> Vec<Contribution<'d>> {
        let design = self.decl.design();
        nested_stmts(design, &self.data().body)
            .into_iter()
            .filter(|s| matches!(design.stmt_node(*s).kind, StmtKind::Contribution { .. }))
            .map(|s| Contribution::new(design, s))
            .collect()
    }

    /// The reactions of a `digital` block in source order, including those
    /// inside generation.
    pub fn reactions(self) -> Vec<Reaction<'d>> {
        let mut out = Vec::new();
        deep_items(self.decl, &mut out);
        out.into_iter().filter_map(|d| d.as_reaction()).collect()
    }

    /// The generation items written directly in a `digital` block.
    pub fn generations(self) -> Vec<Generation<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_generation()).collect()
    }
}

/// `on event { .. }`
#[derive(Clone, Copy)]
pub struct Reaction<'d> {
    decl: Decl<'d>,
}

impl<'d> Reaction<'d> {
    fn data(self) -> &'d ReactionData {
        match self.decl.kind() {
            DeclKind::Reaction(r) => r,
            _ => unreachable!("Reaction handle over a non-reaction declaration"),
        }
    }

    pub fn event(self) -> Expr<'d> {
        self.decl.design().expr(self.data().event)
    }

    pub fn body(self) -> &'d Block {
        &self.data().body
    }

    /// Every commit of the body at any depth, in source order.
    pub fn commits(self) -> Vec<Commit<'d>> {
        let design = self.decl.design();
        nested_stmts(design, &self.data().body.stmts)
            .into_iter()
            .filter(|s| matches!(design.stmt_node(*s).kind, StmtKind::Commit { .. }))
            .map(|s| Commit::new(design, s))
            .collect()
    }
}

// ---- Reflect, equality, debug, casts --------------------------------------------------------

impl<'d> Reflect<'d> for Nature<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Nature<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Nature<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Nature({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Quantity<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Quantity<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Quantity<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quantity({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Module<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Module<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Module<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Module({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Port<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Port<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Port<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Port({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Param<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Param<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Param<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Param({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Signal<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Signal<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Signal<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signal({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Let<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Let<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Let<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Let({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Binding<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Binding<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Binding<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Binding({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Definition<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Definition<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Definition<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Definition({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Generation<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Generation<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Generation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Generation({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Behavior<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Behavior<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Behavior<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Behavior({:?})", self.decl.id())
    }
}

impl<'d> Reflect<'d> for Reaction<'d> {
    fn decl(self) -> Decl<'d> {
        self.decl
    }
}

impl PartialEq for Reaction<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.decl == other.decl
    }
}

impl std::fmt::Debug for Reaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reaction({:?})", self.decl.id())
    }
}

impl<'d> Decl<'d> {
    pub fn as_nature(self) -> Option<Nature<'d>> {
        matches!(self.kind(), DeclKind::Nature(_)).then_some(Nature { decl: self })
    }

    pub fn as_quantity(self) -> Option<Quantity<'d>> {
        matches!(self.kind(), DeclKind::Quantity(_)).then_some(Quantity { decl: self })
    }

    pub fn as_module(self) -> Option<Module<'d>> {
        matches!(self.kind(), DeclKind::Module(_)).then_some(Module { decl: self })
    }

    pub fn as_port(self) -> Option<Port<'d>> {
        matches!(self.kind(), DeclKind::Port(_)).then_some(Port { decl: self })
    }

    pub fn as_param(self) -> Option<Param<'d>> {
        matches!(self.kind(), DeclKind::Param(_)).then_some(Param { decl: self })
    }

    pub fn as_signal(self) -> Option<Signal<'d>> {
        matches!(self.kind(), DeclKind::Signal(_)).then_some(Signal { decl: self })
    }

    pub fn as_let(self) -> Option<Let<'d>> {
        matches!(self.kind(), DeclKind::Let(_)).then_some(Let { decl: self })
    }

    pub fn as_binding(self) -> Option<Binding<'d>> {
        matches!(self.kind(), DeclKind::Binding(_)).then_some(Binding { decl: self })
    }

    pub fn as_definition(self) -> Option<Definition<'d>> {
        matches!(self.kind(), DeclKind::Definition(_)).then_some(Definition { decl: self })
    }

    pub fn as_generation(self) -> Option<Generation<'d>> {
        matches!(self.kind(), DeclKind::Generation(_)).then_some(Generation { decl: self })
    }

    pub fn as_behavior(self) -> Option<Behavior<'d>> {
        matches!(self.kind(), DeclKind::Behavior(_)).then_some(Behavior { decl: self })
    }

    pub fn as_reaction(self) -> Option<Reaction<'d>> {
        matches!(self.kind(), DeclKind::Reaction(_)).then_some(Reaction { decl: self })
    }
}
