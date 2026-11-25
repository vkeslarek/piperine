//! Which params a placement or a template must bind (§8.1.17, §8.4.2): one rule for the checker's templates and the
//! elaborator's placements. A param with no default is required unless its type is `Option<T>`, which a placement that leaves
//! it out reads as `None` (§3.2.10).

use crate::pom::ParamData;

use super::Ty;

/// A param as the requirement rule reads it: its declaration and its type, once lowered.
pub struct ParamRequirement<'a> {
    /// The param's declaration.
    declared: &'a ParamData,
    /// The param's type, `None` when it did not lower.
    ty: Option<&'a Ty>,
}

impl<'a> ParamRequirement<'a> {
    /// The param `declared`, of the type `ty`.
    pub fn new(declared: &'a ParamData, ty: Option<&'a Ty>) -> Self {
        ParamRequirement { declared, ty }
    }

    /// Whether every placement and every template must bind the param (§8.1.17): it has no default and is no `Option<T>`.
    pub fn is_required(&self) -> bool {
        self.declared.default.is_none() && !matches!(self.ty, Some(Ty::Option(_)))
    }
}
