//! Patterns matched against composite constants: tuples, arrays, structs, enum variants and `Option`s (§3.5).

use piperine_ir::{Constant};
use crate::elab::value::compound::EnumLayout;
use super::{CompositeKind, Bindings, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm};
use crate::pom::{DeclId, DeclKind, ExprId, PatId, PatKind, Resolution};

impl ConstEval<'_, '_> {
    /// Whether `value` matches the pattern `pat` of a composite form (a tuple, a tuple struct, a
    /// struct, an array, `None`, a unit struct or a variant with fields); the names it binds are
    /// bound. `None` when the pattern is not one of those, so the scalar patterns decide.
    pub(super) fn matches_composite(&mut self, pat: PatId, value: Constant, at: ExprId, bindings: &Bindings) -> Option<Result<bool, ConstFault>> {
        let kind = self.design.pat_node(pat).kind.clone();
        match &kind {
            PatKind::Tuple(pats) => Some(self.matches_parts(pats, value, at, bindings)),
            PatKind::Array(pats) => Some(self.matches_array(pats, value, at, bindings)),
            PatKind::TupleStruct { path, pats } => {
                let name = self.builtin_name(*path);
                if name == Some("Some") {
                    return Some(self.matches_option(pats, value, at, bindings));
                }
                let variant = self.pattern_owner(*path);
                Some(self.matches_owner(variant, value, at, bindings, |this, parts| this.matches_each(pats, &parts, at, bindings)))
            }
            PatKind::Struct { path, fields, .. } => {
                let owner = self.pattern_owner(*path);
                let fields = fields.clone();
                Some(self.matches_owner(owner, value, at, bindings, |this, parts| {
                    for field in &fields {
                        let Resolution::Decl(decl) = field.res else { return Err(ConstFault::refused(at, ConstantForm::FieldPatternThatNamesNoField)) };
                        let layout = this.pattern_layout(owner);
                        let Some(position) = layout.iter().position(|f| *f == decl) else { return Err(ConstFault::refused(at, ConstantForm::FieldStructDoesNotHave)) };
                        let part = parts[position];
                        match (field.pat, field.binding) {
                            (Some(sub), _) => {
                                if !this.matches(sub, part, at, bindings)? {
                                    return Ok(false);
                                }
                            }
                            (None, Some(binding)) => {
                                this.locals.insert(binding, part);
                            }
                            (None, None) => {}
                        }
                    }
                    Ok(true)
                }))
            }
            PatKind::Ident { res: Resolution::Builtin(id), .. } if id.info().name == "None" => Some(Ok(self.composite_of(value).is_some_and(|c| c.kind == CompositeKind::Option && c.parts.is_empty()))),
            PatKind::Path(path) if self.builtin_name(*path) == Some("None") => Some(Ok(self.composite_of(value).is_some_and(|c| c.kind == CompositeKind::Option && c.parts.is_empty()))),
            PatKind::Path(path) => {
                let owner = self.pattern_owner(*path);
                let Some(decl) = owner else { return None };
                if matches!(self.design.decl_data(decl).kind, DeclKind::Struct(_)) || (matches!(self.design.decl_data(decl).kind, DeclKind::Variant(_)) && self.enum_has_fields(decl)) {
                    return Some(self.matches_owner(Some(decl), value, at, bindings, |_, _| Ok(true)));
                }
                None
            }
            PatKind::Ident { res: Resolution::Decl(decl), .. } if matches!(self.design.decl_data(*decl).kind, DeclKind::Variant(_)) && self.enum_has_fields(*decl) => Some(self.matches_owner(Some(*decl), value, at, bindings, |_, _| Ok(true))),
            PatKind::Ident { res: Resolution::Decl(decl), .. } if matches!(self.design.decl_data(*decl).kind, DeclKind::Struct(_)) => Some(self.matches_owner(Some(*decl), value, at, bindings, |_, _| Ok(true))),
            _ => None,
        }
    }

    /// The struct or variant a pattern path names.
    pub(super) fn pattern_owner(&self, path: crate::pom::PathId) -> Option<DeclId> {
        match self.design.path_node(path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Struct(_) | DeclKind::Variant(_)) => Some(d),
            _ => None,
        }
    }

    /// The field declarations of the struct or variant `owner`.
    pub(super) fn pattern_layout(&self, owner: Option<DeclId>) -> Vec<DeclId> {
        owner.and_then(|owner| self.types.fields(owner)).map(|fields| fields.iter().map(|f| f.decl).collect()).unwrap_or_default()
    }

    /// `value` against the struct or variant `owner`: the same variant (a struct has one shape), then
    /// `parts_match` over its parts.
    pub(super) fn matches_owner(&mut self, owner: Option<DeclId>, value: Constant, at: ExprId, _bindings: &Bindings, parts_match: impl FnOnce(&mut Self, Vec<Constant>) -> Result<bool, ConstFault>) -> Result<bool, ConstFault> {
        let Some(owner) = owner else { return Err(ConstFault::refused(at, ConstantForm::PatternThatNamesNoStructOrVariant)) };
        let (kind, parts) = self.parts_of(at, value, "a pattern over a value")?;
        match (self.design.decl_data(owner).kind.clone(), kind) {
            (DeclKind::Struct(_), CompositeKind::Struct) => parts_match(self, parts),
            (DeclKind::Variant(_), CompositeKind::Variant(position)) => match EnumLayout::variant_tag(self.design, owner) {
                Some(wanted) if wanted as u32 == position => parts_match(self, parts),
                _ => Ok(false),
            },
            _ => Ok(false),
        }
    }

    /// `(a, b, ..)` over a tuple.
    pub(super) fn matches_parts(&mut self, pats: &[PatId], value: Constant, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        let (_, parts) = self.parts_of(at, value, "a tuple pattern over a value")?;
        self.matches_each(pats, &parts, at, bindings)
    }

    /// Each sub-pattern against the part at its position; `..` skips what lies between.
    pub(super) fn matches_each(&mut self, pats: &[PatId], parts: &[Constant], at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        let rest = pats.iter().position(|p| matches!(self.design.pat_node(*p).kind, PatKind::Range { lo: None, hi: None, .. }));
        let (before, after): (&[PatId], &[PatId]) = match rest {
            Some(at_rest) => (&pats[..at_rest], &pats[at_rest + 1..]),
            None => (pats, &[]),
        };
        if parts.len() < before.len() + after.len() {
            return Ok(false);
        }
        for (pat, part) in before.iter().zip(parts) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        for (pat, part) in after.iter().rev().zip(parts.iter().rev()) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `[a, .., z]`, `[h, rest @ ..]` over an array: with no `..`, the lengths agree.
    pub(super) fn matches_array(&mut self, pats: &[PatId], value: Constant, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        let (_, parts) = self.parts_of(at, value, "an array pattern over a value")?;
        let rest = pats.iter().position(|p| self.is_rest(*p));
        let Some(at_rest) = rest else {
            return if parts.len() == pats.len() { self.matches_each(pats, &parts, at, bindings) } else { Ok(false) };
        };
        let (before, after) = (&pats[..at_rest], &pats[at_rest + 1..]);
        if parts.len() < before.len() + after.len() {
            return Ok(false);
        }
        let middle = parts[before.len()..parts.len() - after.len()].to_vec();
        if let PatKind::Ident { binding, sub: Some(_), .. } = self.design.pat_node(pats[at_rest]).kind {
            let slice = self.make(CompositeKind::Array, middle);
            self.locals.insert(binding, slice);
        }
        for (pat, part) in before.iter().zip(&parts) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        for (pat, part) in after.iter().rev().zip(parts.iter().rev()) {
            if !self.matches(*pat, *part, at, bindings)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Whether `pat` is `..` or `name @ ..`.
    pub(super) fn is_rest(&self, pat: PatId) -> bool {
        match &self.design.pat_node(pat).kind {
            PatKind::Range { lo: None, hi: None, .. } => true,
            PatKind::Ident { sub: Some(sub), .. } => self.is_rest(*sub),
            _ => false,
        }
    }

    /// `Some(p)` over an `Option`.
    pub(super) fn matches_option(&mut self, pats: &[PatId], value: Constant, at: ExprId, bindings: &Bindings) -> Result<bool, ConstFault> {
        let (_, parts) = self.parts_of(at, value, "a `Some` pattern over a value")?;
        match (parts.first(), pats) {
            (Some(part), [inner]) => self.matches(*inner, *part, at, bindings),
            _ => Ok(false),
        }
    }
}
