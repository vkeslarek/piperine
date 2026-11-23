//! The traits and `impl`s of a design as one table (part I §3.1, §3.7, GN-03, GN-04): every trait
//! with its items and supertraits, every inherent `impl`, every trait `impl` and every blanket
//! `impl<T: B> Tr for T`. It answers four questions, each by one method: the inherent item `name`
//! of a type; the trait item `name` visible on a type; the implementation of a trait item for a
//! concrete type, the `impl`'s own item or else the trait's default body; and whether a type
//! implements a trait. A blanket `impl` applies to the types its bounds admit, which the table
//! cannot decide alone, so each question that reaches one takes the check of a bound.

use crate::pom::{DeclId, DeclKind, Design, GenericArgNode, GenericArgsNode, GenericParamData, Resolution, TypeId, TypeKind};

use super::generics::{Bound, GenericSignature};
use super::Ty;

/// Whether a type satisfies a bound: what a blanket `impl` asks of the type it may apply to.
pub type BoundOracle<'a> = &'a mut dyn FnMut(&Ty, &Bound) -> bool;

/// What an `impl` is written for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplTarget {
    /// A struct, an enum or a nature, by its declaration.
    Declared(DeclId),
    /// A built-in scalar or `str` (`impl Describe for f64`).
    Primitive(Ty),
    /// Every type that satisfies these bounds (`impl<T: Repr> Known for T`).
    Blanket(Vec<Bound>),
    /// A type the table does not index (a tuple, an array, an unresolved name).
    Other,
}

/// One `impl` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplEntry {
    /// The `impl` declaration; its items are its children.
    pub decl: DeclId,
    /// The trait it implements, `None` for an inherent `impl`.
    pub trait_decl: Option<DeclId>,
    /// The arguments the trait is written with, as written (`impl Resolve<Driven> for Wire`).
    pub trait_arguments: Vec<TypeId>,
    /// What it is written for.
    pub target: ImplTarget,
}

/// Every trait and `impl` of a design.
#[derive(Debug, Clone)]
pub struct ImplTable<'d> {
    design: &'d Design,
    impls: Vec<ImplEntry>,
    traits: Vec<DeclId>,
}

impl<'d> ImplTable<'d> {
    /// The table of `design`: its `impl`s in declaration order.
    pub fn new(design: &'d Design) -> ImplTable<'d> {
        let impls = (0..design.decl_count()).map(|i| DeclId(i as u32)).filter_map(|decl| Self::entry(design, decl)).collect();
        let traits = (0..design.decl_count()).map(|i| DeclId(i as u32)).filter(|decl| matches!(design.decl_data(*decl).kind, DeclKind::Trait(_))).collect();
        ImplTable { design, impls, traits }
    }

    /// The `impl` blocks, in declaration order.
    pub fn impls(&self) -> &[ImplEntry] {
        &self.impls
    }

    /// Whether a project's own `impl` or trait (the standard library's left out) defines a function
    /// called `name`: a method the checker must not answer from the Appendix H catalogue alone.
    pub fn defines_method(&self, name: &str, is_std: impl Fn(DeclId) -> bool) -> bool {
        let owners = self.impls.iter().map(|entry| entry.decl).chain(self.traits.iter().copied());
        owners.filter(|owner| !is_std(*owner)).any(|owner| self.design.decl_data(owner).children.iter().any(|child| matches!(self.design.decl_data(*child).kind, DeclKind::Fn(_)) && self.design.decl_data(*child).name.as_ref().is_some_and(|n| n.text == name)))
    }

    /// The traits of the design, in declaration order.
    pub fn traits(&self) -> &[DeclId] {
        &self.traits
    }

    /// The item `name` (a function or a constant) among the children of `owner`.
    pub fn item(&self, owner: DeclId, name: &str) -> Option<DeclId> {
        self.design.decl_data(owner).children.iter().copied().find(|child| {
            let data = self.design.decl_data(*child);
            matches!(data.kind, DeclKind::Fn(_) | DeclKind::Const(_)) && data.name.as_ref().is_some_and(|n| n.text == name)
        })
    }

    /// The item `name` of the inherent `impl` of `ty`, a function or a constant (`Logic::Z`).
    pub fn inherent(&self, ty: &Ty, name: &str) -> Option<DeclId> {
        self.impls.iter().filter(|entry| entry.trait_decl.is_none() && self.targets(entry, ty)).find_map(|entry| self.item(entry.decl, name))
    }

    /// The items called `name` that a trait gives `ty`: the `impl`'s own, else the trait's (a default
    /// body or the declaration the `impl` inherits), in declaration order of the `impl`s. More than
    /// one is two traits giving the name (G.67), which the caller reports.
    pub fn visible(&self, ty: &Ty, name: &str, oracle: BoundOracle<'_>) -> Vec<DeclId> {
        let mut found = Vec::new();
        for entry in &self.impls {
            let Some(trait_decl) = entry.trait_decl else { continue };
            if !self.applies(entry, ty, oracle) {
                continue;
            }
            if let Some(item) = self.item(entry.decl, name).or_else(|| self.item(trait_decl, name)) {
                found.push(item);
            }
        }
        found
    }

    /// The implementation of the trait item `trait_item` (a function or a constant of a trait) for
    /// `ty`: the item of the `impl` of that trait for `ty`, else the trait's own when it has a
    /// body or a value. `None` when `ty` does not implement the trait.
    pub fn implementation(&self, trait_item: DeclId, ty: &Ty, oracle: BoundOracle<'_>) -> Option<DeclId> {
        let trait_decl = self.design.decl_data(trait_item).parent.filter(|parent| matches!(self.design.decl_data(*parent).kind, DeclKind::Trait(_)))?;
        let name = self.design.decl_data(trait_item).name.as_ref()?.text.clone();
        let entry = self.impls.iter().find(|entry| entry.trait_decl == Some(trait_decl) && self.applies(entry, ty, oracle))?;
        match self.item(entry.decl, &name) {
            Some(own) => Some(own),
            None if self.has_default(trait_item) => Some(trait_item),
            None => None,
        }
    }

    /// Whether `ty` implements the trait `trait_decl`, through an `impl` for it or a blanket one.
    pub fn implements(&self, ty: &Ty, trait_decl: DeclId, oracle: BoundOracle<'_>) -> bool {
        !self.implementing(ty, trait_decl, oracle).is_empty()
    }

    /// The `impl`s of the trait `trait_decl` that apply to `ty`, in declaration order: one for each
    /// list of trait arguments the type implements it with (`Resolve<A>` and `Resolve<B>`).
    pub fn implementing(&self, ty: &Ty, trait_decl: DeclId, oracle: BoundOracle<'_>) -> Vec<&ImplEntry> {
        self.impls.iter().filter(|entry| entry.trait_decl == Some(trait_decl) && self.applies(entry, ty, oracle)).collect()
    }

    /// The traits `trait_decl` requires, as written after its name (§3.7.3).
    pub fn supertraits(&self, trait_decl: DeclId) -> Vec<DeclId> {
        let DeclKind::Trait(data) = &self.design.decl_data(trait_decl).kind else { return Vec::new() };
        data.supertraits
            .iter()
            .filter_map(|path| match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
                Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::Trait(_)) => Some(decl),
                _ => None,
            })
            .collect()
    }

    /// Whether the two `impl`s of one trait are written for types that have a type in common: the
    /// same declaration or scalar, or a blanket `impl` whose bounds the other's type satisfies (two
    /// blanket `impl`s always have one, §3.7.6). Their trait arguments are compared by the caller.
    pub fn overlap(&self, first: &ImplEntry, second: &ImplEntry, oracle: BoundOracle<'_>) -> bool {
        match (&first.target, &second.target) {
            (ImplTarget::Blanket(_), ImplTarget::Blanket(_)) => true,
            (ImplTarget::Blanket(bounds), other) | (other, ImplTarget::Blanket(bounds)) => self.representative(other).is_some_and(|ty| bounds.iter().all(|bound| oracle(&ty, bound))),
            (ImplTarget::Declared(first), ImplTarget::Declared(second)) => first == second,
            (ImplTarget::Primitive(first), ImplTarget::Primitive(second)) => first == second,
            _ => false,
        }
    }

    /// A type the target stands for, to ask a blanket `impl`'s bounds of.
    fn representative(&self, target: &ImplTarget) -> Option<Ty> {
        match target {
            ImplTarget::Declared(decl) => Some(match self.design.decl_data(*decl).kind {
                DeclKind::Enum(_) => Ty::enumeration(*decl),
                DeclKind::Nature(_) => Ty::Node(*decl),
                _ => Ty::structure(*decl),
            }),
            ImplTarget::Primitive(ty) => Some(ty.clone()),
            ImplTarget::Blanket(_) | ImplTarget::Other => None,
        }
    }

    /// Whether the trait item has a body (a function) or a value (a constant) of its own.
    pub fn has_default(&self, item: DeclId) -> bool {
        match &self.design.decl_data(item).kind {
            DeclKind::Fn(data) => data.body.is_some(),
            DeclKind::Const(data) => data.value.is_some(),
            _ => false,
        }
    }

    /// Whether `entry` is written for `ty`, its bounds not asked: a blanket `impl` is written for every type.
    fn targets(&self, entry: &ImplEntry, ty: &Ty) -> bool {
        match (&entry.target, ty) {
            (ImplTarget::Declared(decl), Ty::Struct(nominal) | Ty::Enum(nominal)) => nominal.decl == *decl,
            (ImplTarget::Declared(decl), Ty::Node(nature) | Ty::Carrier { nature, .. }) => nature == decl,
            (ImplTarget::Primitive(primitive), other) => primitive == other,
            (ImplTarget::Blanket(_), _) => true,
            _ => false,
        }
    }

    /// Whether `entry` applies to `ty`: written for it, or blanket with every bound satisfied.
    fn applies(&self, entry: &ImplEntry, ty: &Ty, oracle: BoundOracle<'_>) -> bool {
        match &entry.target {
            ImplTarget::Blanket(bounds) => bounds.iter().all(|bound| oracle(ty, bound)),
            _ => self.targets(entry, ty),
        }
    }

    /// The entry of the declaration `decl` when it is an `impl`.
    fn entry(design: &Design, decl: DeclId) -> Option<ImplEntry> {
        let DeclKind::Impl(data) = &design.decl_data(decl).kind else { return None };
        let (trait_decl, trait_arguments) = match data.trait_ {
            Some(path) => {
                let segment = design.path_node(path).segments.last()?;
                let arguments = match &segment.args {
                    Some(GenericArgsNode::Angle { args, .. }) => args.iter().filter_map(|arg| if let GenericArgNode::Type(ty) = arg { Some(*ty) } else { None }).collect(),
                    _ => Vec::new(),
                };
                match segment.res {
                    Resolution::Decl(trait_decl) if matches!(design.decl_data(trait_decl).kind, DeclKind::Trait(_)) => (Some(trait_decl), arguments),
                    _ => return None,
                }
            }
            None => (None, Vec::new()),
        };
        Some(ImplEntry { decl, trait_decl, trait_arguments, target: Self::target(design, decl, data.self_ty) })
    }

    /// What the self type of the `impl` `decl` names.
    fn target(design: &Design, decl: DeclId, self_ty: TypeId) -> ImplTarget {
        let TypeKind::Path(path) = &design.type_node(self_ty).kind else { return ImplTarget::Other };
        match design.path_node(*path).segments.last().map(|segment| segment.res) {
            Some(Resolution::Decl(target)) => match &design.decl_data(target).kind {
                DeclKind::GenericParam(GenericParamData::Type { bounds, .. }) if design.decl_data(target).parent == Some(decl) => ImplTarget::Blanket(bounds.iter().filter_map(|bound| GenericSignature::bound(design, *bound)).collect()),
                DeclKind::Struct(_) | DeclKind::Enum(_) | DeclKind::Nature(_) => ImplTarget::Declared(target),
                _ => ImplTarget::Other,
            },
            Some(Resolution::Builtin(id)) => match (id.info().name, id.width()) {
                ("fN", Some(64)) => ImplTarget::Primitive(Ty::F64),
                ("bool", _) => ImplTarget::Primitive(Ty::Bool),
                ("str", _) => ImplTarget::Primitive(Ty::Str),
                ("uN", Some(width)) => u8::try_from(width).map_or(ImplTarget::Other, |width| ImplTarget::Primitive(Ty::UInt(width))),
                ("iN", Some(width)) => u8::try_from(width).map_or(ImplTarget::Other, |width| ImplTarget::Primitive(Ty::Int(width))),
                ("usize", _) => ImplTarget::Primitive(Ty::UInt(64)),
                ("isize", _) => ImplTarget::Primitive(Ty::Int(64)),
                _ => ImplTarget::Other,
            },
            _ => ImplTarget::Other,
        }
    }
}
