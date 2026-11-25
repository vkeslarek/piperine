//! Static dispatch through a bound in the constant evaluator (§3.7.8, GN-04, §3.1.4): `n.vth()` of a parameter `N: Mosfet` runs
//! the `impl` of the concrete type the instance being evaluated names, or the trait's own default body.

use super::ConstEval;
use crate::pom::{DeclId, ExprId};
use crate::types::{Bound, ImplTable, Ty};

impl ConstEval<'_, '_> {
    /// The item that runs for the call `expr` of the trait function `item` through a bound, on a receiver of type `receiver`:
    /// of the `impl`s of the trait that apply to the type, the first written with the arguments the bound names
    /// (`C: Convert<f64>` picks `impl Convert<f64> for Cell`, not `impl Convert<u8> for Cell`); the trait's own body
    /// when that `impl` leaves the function out and the trait gives one.
    pub(in crate::elab) fn bound_implementation(&mut self, expr: ExprId, item: DeclId, receiver: &Ty) -> Option<DeclId> {
        let wanted: Vec<Ty> = self.types.bound_arguments(expr).iter().map(|ty| self.substitution().apply(ty)).collect();
        let trait_decl = self.design.decl_data(item).parent?;
        let name = self.design.decl_data(item).name.as_ref()?.text.clone();
        let table = &self.impls;
        let entries: Vec<(DeclId, Vec<crate::pom::TypeId>)> = table.implementing(receiver, trait_decl, &mut |ty, bound| Self::bound_holds(table, ty, bound)).into_iter().map(|entry| (entry.decl, entry.trait_arguments.clone())).collect();
        let chosen = entries.into_iter().find(|(_, arguments)| {
            wanted.is_empty() || {
                let given: Option<Vec<Ty>> = arguments.iter().map(|argument| self.typed_written(*argument)).collect();
                given.is_some_and(|given| given == wanted)
            }
        })?;
        match self.impls.item(chosen.0, &name) {
            Some(own) => Some(own),
            None if self.impls.has_default(item) => Some(item),
            None => None,
        }
    }

    /// Whether `ty` satisfies `bound` for choosing among the `impl`s of the design: a trait bound is met by an `impl`
    /// (blanket ones included); the checker has judged every other bound before the instance exists.
    fn bound_holds(table: &ImplTable<'_>, ty: &Ty, bound: &Bound) -> bool {
        match bound {
            Bound::Trait { decl, .. } => table.implements(ty, *decl, &mut |candidate, inner| Self::bound_holds(table, candidate, inner)),
            _ => true,
        }
    }
}
