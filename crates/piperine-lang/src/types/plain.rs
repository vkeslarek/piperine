//! Plain data: the composite values a param or a record field may hold (§8.1.15, §19 rule 11).
//!
//! A value built of numbers, booleans and enums through structs, tuples, arrays and `Option` is
//! plain; a record field may also hold text. A function value, a module, a carrier and an event
//! are never plain, so a host can read and write every leaf of a plain value.

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// Whether `ty` is a struct, a tuple, an array or an `Option` all of whose leaves are plain
    /// scalars (`f64`, `bool`, an integer, an enum), or `str` when `text` is allowed. Anything
    /// else, a bare scalar included, is not a plain composite.
    pub(super) fn is_plain_composite(&mut self, ty: &Ty, text: bool) -> bool {
        ty.is_composite() && self.is_plain(ty, text)
    }

    /// Whether a param may hold `ty`: a plain composite that is neither an `Option` nor an enum, which
    /// are `params-and-io`'s (L28) and are refused by name until it lowers them.
    pub(super) fn is_param_composite(&mut self, ty: &Ty) -> bool {
        !matches!(ty, Ty::Option(_) | Ty::Enum(_)) && self.is_plain_composite(ty, false)
    }

    /// Whether every leaf of `ty` is a plain scalar, or `str` when `text` is allowed.
    fn is_plain(&mut self, ty: &Ty, text: bool) -> bool {
        match ty {
            Ty::F64 | Ty::Bool | Ty::UInt(_) | Ty::Int(_) | Ty::Enum(_) | Ty::Unit => true,
            Ty::Str => text,
            Ty::Tuple(items) => items.iter().all(|item| self.is_plain(item, text)),
            Ty::Array(element, _) | Ty::Option(element) => self.is_plain(element, text),
            Ty::Struct(decl) => match self.fields_of(*decl) {
                Some(fields) => fields.iter().all(|field| self.is_plain(&field.ty, text)),
                None => false,
            },
            _ => false,
        }
    }
}
