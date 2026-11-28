//! The names the elaborator gives what it lowers: the qualified name of an `impl` item, of a closure and of an instance.

use crate::pom::{DeclId, DeclKind, ExprId};
use super::Elaboration;

impl<'d, 't> Elaboration<'d, 't> {
    /// Whether `function` is declared in a trait.
    pub(in crate::elab) fn is_trait_function(&self, function: DeclId) -> bool {
        self.state.design.decl_data(function).parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Trait(_)))
    }

    /// Whether `function` is written in an `impl` that has generic parameters.
    pub(in crate::elab) fn is_generic_impl_item(&self, function: DeclId) -> bool {
        self.state.design.decl_data(function).parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Impl(_)) && self.state.design.decl_data(parent).generics_span.is_some())
    }

    /// The name of `function` as written; a function of an `impl` with no parameters is named by the type the `impl` is for, `Sq.area`, so
    /// two types giving one method name stay two IR functions.
    pub(in crate::elab) fn impl_qualified_name(&self, function: DeclId) -> String {
        let data = self.state.design.decl_data(function);
        let own = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(parent) = data.parent else { return own };
        // an `impl` with parameters is told apart by the arguments of its instances
        let DeclKind::Impl(owner) = &self.state.design.decl_data(parent).kind else { return own };
        // an `impl` with parameters is told apart by the arguments of its instances, and a trait `impl` also by its type and
        // its trait arguments (`impl BitAnd for Logic<N>` and `impl BitAnd<UInt<N>, Logic<N>> for Logic<N>` give two `bitand`)
        if self.state.design.decl_data(parent).generics_span.is_some() && owner.trait_.is_none() {
            return own;
        }
        let suffix = self.trait_arguments_suffix(parent);
        // the type as written, under no substitution: the name of an instance is the same wherever it is asked
        match self.state.consts.written_as_declared(owner.self_ty) {
            Some(ty) => format!("{}.{own}{suffix}", ty.source_name(self.state.design)),
            None => match self.nature_target_name(parent) {
                Some(nature) => format!("{nature}.{own}{suffix}"),
                None => own,
            },
        }
    }

    /// The name of the item a closure is written in: two items each have a `closure$0`, so the instance names the item too.
    pub(in crate::elab) fn closure_owner_name(&self, closure: ExprId) -> String {
        let owner = self.state.design.expr_node(closure).owner;
        self.state.design.decl_data(owner).name.as_ref().map_or_else(String::new, |name| name.text.clone())
    }

    /// The position of the closure `closure` among the closures of the item it is written in, in source order.
    pub(in crate::elab) fn closure_ordinal(&self, closure: ExprId) -> usize {
        let owner = self.state.design.expr_node(closure).owner;
        (0..closure.0).filter(|index| {
            let node = self.state.design.expr_node(ExprId(*index));
            node.owner == owner && matches!(node.kind, crate::pom::ExprKind::Closure { .. })
        }).count()
    }

    /// The name of the nature a trait `impl` is written for (`impl Resolve<f64> for Total`): a nature is no written
    /// type, so two `impl`s of one trait for two natures tell their methods apart by it.
    pub(in crate::elab) fn nature_target_name(&self, implementation: DeclId) -> Option<String> {
        let entry = self.state.impls.impls().iter().find(|entry| entry.decl == implementation)?;
        match &entry.target {
            crate::types::ImplTarget::Declared(decl) => self.state.design.decl_data(*decl).name.as_ref().map(|n| n.text.clone()),
            _ => None,
        }
    }

    /// `::<f64>` for an `impl` of a trait written with arguments (`impl Convert<f64> for Cell`), empty for any other: two
    /// `impl`s of one trait for one type, written with different arguments, each give a function of one name, and the
    /// arguments tell them apart. Not inlined: it runs once for every instantiation, deep in a recursion.
    #[inline(never)]
    pub(in crate::elab) fn trait_arguments_suffix(&self, owner: DeclId) -> String {
        let arguments = self.state.impls.impls().iter().find(|entry| entry.decl == owner).map(|entry| entry.trait_arguments.clone()).unwrap_or_default();
        let written: Vec<String> = arguments.iter().filter_map(|argument| self.state.consts.written_as_declared(*argument)).map(|ty| ty.source_name(self.state.design)).collect();
        if written.is_empty() { String::new() } else { format!("::<{}>", written.join(", ")) }
    }
}
