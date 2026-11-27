//! Handles over modules: their ports, params, signals, `let` names, definitions and generation.

use super::{Behavior, Decl, Expr, Instance, Nature, Pat, Reflect, Type};
use crate::pom::body::{BehaviorKind, TypeKind};
use crate::pom::decl::{
    BindingData, BindingOrigin, DeclKind, DefinitionData, Dir, GenElseNode, GenerationData, LetData,
    ModuleData, ParamData, PortData, SignalData,
};
use crate::pom::bundles::BundleView;
use crate::pom::design::Design;
use crate::pom::ids::DeclId;
use crate::pom::resolution::{PortKind, Resolution};
use crate::pom::body::LitKind;



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

    /// Every instance placed by this module's body and blocks, in source
    /// order: named ones written as `let name = Callee(..)` and anonymous
    /// ones (a built-in or declared module called in a block). Generation is
    /// not unrolled: an instance in a loop body is listed once.
    pub fn instances(self) -> Vec<Instance<'d>> {
        let design = self.decl.design();
        self.data().instances.iter().map(|e| Instance::new(design, *e)).collect()
    }

    /// The instance named `name` by its `let`.
    pub fn instance(self, name: &str) -> Option<Instance<'d>> {
        self.instances().into_iter().find(|i| i.name() == Some(name))
    }

    /// The `analog` blocks of this module, in source order.
    pub fn analog_blocks(self) -> Vec<Behavior<'d>> {
        self.behaviors().into_iter().filter(|b| b.kind() == BehaviorKind::Analog).collect()
    }

    /// The `digital` blocks of this module, in source order.
    pub fn digital_blocks(self) -> Vec<Behavior<'d>> {
        self.behaviors().into_iter().filter(|b| b.kind() == BehaviorKind::Digital).collect()
    }

    /// The first port, param or signal of this module named `name` that
    /// `cast` accepts, from the sorted member table (generation entered).
    fn first_declared<T>(self, name: &str, cast: impl Fn(Decl<'d>) -> Option<T>) -> Option<T> {
        let design = self.decl.design();
        let ids = design.tables().members.get(&(self.decl.id(), name.to_string()))?;
        ids.iter().find_map(|id| cast(Decl::new(design, *id)))
    }

    /// The ports in source order.
    pub fn ports(self) -> Vec<Port<'d>> {
        self.decl.children().into_iter().filter_map(|c| c.as_port()).collect()
    }

    pub fn port(self, name: &str) -> Option<Port<'d>> {
        self.first_declared(name, Decl::as_port)
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
        self.decl.deep_items(&mut out);
        out
    }

    /// The params in source order.
    pub fn params(self) -> Vec<Param<'d>> {
        self.deep().into_iter().filter_map(|c| c.as_param()).collect()
    }

    pub fn param(self, name: &str) -> Option<Param<'d>> {
        self.first_declared(name, Decl::as_param)
    }

    /// The signals in source order, including those written inside
    /// generation.
    pub fn signals(self) -> Vec<Signal<'d>> {
        self.deep().into_iter().filter_map(|c| c.as_signal()).collect()
    }

    pub fn signal(self, name: &str) -> Option<Signal<'d>> {
        self.first_declared(name, Decl::as_signal)
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
                LitKind::Str { text, .. } => Some(text.as_str()),
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

    /// The view of a bundle port (`tx: Stream` a producer, `rx: flip Stream` a consumer, `in s: Stream` an observer);
    /// none for any other port. The port stays one port with its bundle type: its fields are no ports of the module.
    pub fn bundle_view(self) -> Option<BundleView> {
        let design = self.decl.design();
        design.is_bundle_owner(self.decl.id()).then(|| BundleView::of_port(design, self.data().dir, self.data().ty))
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

    /// Does the name bind a value? It does not when the pattern is a plain
    /// identifier that names a constant, a unit variant or a unit struct:
    /// that pattern is a path (§1), and this declaration stays for navigation.
    pub fn is_binding(self) -> bool {
        !matches!(self.pattern().kind(), crate::pom::body::PatKind::Ident { res, .. } if *res != Resolution::Unresolved)
    }

    /// The pattern that introduces the name.
    pub fn pattern(self) -> Pat<'d> {
        self.decl.design().pat(self.data().pat)
    }

    /// The `let` item the name comes from, for a body `let`.
    pub fn let_item(self) -> Option<Let<'d>> {
        self.declared_in().and_then(|d| d.as_let())
    }

    /// The instance this name names: the `let` value is a call classified as
    /// an instance and the pattern is this one identifier.
    pub fn instance(self) -> Option<Instance<'d>> {
        let value = self.value()?;
        let instance = value.as_instance()?;
        (instance.name() == self.name()).then_some(instance)
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

    /// The items of the `for` body, or of every arm of an `if` chain in
    /// source order (`else if` arms included; the nested generation is not
    /// itself an item here, see [`Generation::arms`]).
    pub fn items(self) -> Vec<Decl<'d>> {
        let design = self.decl.design();
        match self.data() {
            GenerationData::For { body, .. } => body.iter().map(|d| Decl::new(design, *d)).collect(),
            GenerationData::If { .. } => self.arms().into_iter().flat_map(GenArm::items).collect(),
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

impl<'d> Decl<'d> {
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
}
