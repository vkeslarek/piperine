//! The whole-model check (S3, GN-08): every module, function, `const` and `impl` function of every
//! crate but `std` is typed, reached from a top or not, so a library with no `[run]` and an item
//! nothing places are checked as a run would check them.
//!
//! [`Types::check`](super::Types::check) stays the reach of one top: it builds the table the
//! elaborator reads. This check builds none; it keeps the diagnostics of one [`Checker`] seeded
//! with every item, in source order, each with the code the item has when reached. `std` is left
//! to its own check (a stdlib item is checked when reached). A generic function, and a method of a
//! generic `impl`, are checked once at their definition (Q1 (c), design D19). A generic module is
//! checked the same way (GN-13); a generic struct or enum, and what sits inside one, is left out.

use crate::pom::{Decl, DeclId, DeclKind, Design, DesignDiagnostic, Reflect};

use super::check::Checker;

/// One pass over the items of a design that no top need reach.
pub(super) struct ModelCheck<'d> {
    design: &'d Design,
}

impl<'d> ModelCheck<'d> {
    /// A check over every item of `design`.
    pub(super) fn new(design: &'d Design) -> Self {
        ModelCheck { design }
    }

    /// Every seeded item checked: `Ok` when none has a diagnostic.
    pub(super) fn run(self) -> Result<(), Vec<DesignDiagnostic>> {
        let items = self.items();
        let diagnostics = Checker::new(self.design).run_model(&items);
        if diagnostics.is_empty() { Ok(()) } else { Err(diagnostics) }
    }

    /// The items the check starts from, in declaration order.
    fn items(&self) -> Vec<DeclId> {
        (0..self.design.decl_count()).map(|i| DeclId(i as u32)).filter(|decl| self.is_seed(*decl)).collect()
    }

    /// Whether `decl` is a module, a function or a `const` of a crate but `std`, outside a trait
    /// (a trait function has no body to check), outside every generic item other than itself (a
    /// generic function or a method of a generic `impl` is checked once at its definition, Q1 (c); a generic
    /// struct or enum is not).
    fn is_seed(&self, decl: DeclId) -> bool {
        let data = self.design.decl_data(decl);
        // an `impl` is held to the declaration rules of §3.1 whatever its generics
        if matches!(data.kind, DeclKind::Impl(_)) {
            return !Decl::new(self.design, decl).krate().is_std();
        }
        let is_item = matches!(data.kind, DeclKind::Module(_) | DeclKind::Fn(_) | DeclKind::Const(_));
        is_item && !Decl::new(self.design, decl).krate().is_std() && !self.is_generic_beyond_a_module(decl) && !data.parent.is_some_and(|p| matches!(self.design.decl_data(p).kind, DeclKind::Trait(_)))
    }

    /// Whether `decl` is a generic struct or enum, or sits inside one: those wait for their own phase. A generic
    /// function, a method of a generic `impl` and a generic module are checked once at the definition (Q1 (c)).
    fn is_generic_beyond_a_module(&self, decl: DeclId) -> bool {
        let mut at = Some(decl);
        while let Some(current) = at {
            let data = self.design.decl_data(current);
            if data.generics_span.is_some() && matches!(data.kind, DeclKind::Struct(_) | DeclKind::Enum(_)) {
                return true;
            }
            at = data.parent;
        }
        false
    }
}
