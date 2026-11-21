//! Patterns over aggregates (§1.6.5 to §1.6.7): tuple, tuple-struct and variant, struct, slice and
//! reference patterns. Each types its names and says what it covers; the fields of a struct, a
//! variant or a tuple are columns of the coverage judgement (`cover.rs`).

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, FieldPatNode, FileId, PatId, PatKind, PathId, Resolution};

use super::check::Checker;
use super::cover::Covers;
use super::patterns::{Bound, Matched};
use super::Ty;

/// How the covers of one pattern's fields make the cover of the whole: a tuple, a struct and an
/// array are a product of their fields, a variant is a constructor over its own.
#[derive(Clone, Copy)]
enum Shape {
    Fields,
    Variant(DeclId),
}

impl Shape {
    /// The cover of the pattern whose fields cover `cells`.
    fn cover(self, cells: Vec<Covers>) -> Covers {
        match self {
            Shape::Fields => Covers::Product(cells),
            Shape::Variant(variant) => Covers::Constructor(variant, cells),
        }
    }
}

/// The most rows one pattern of alternatives expands into before the coverage gives up listing
/// them (`Covers::Opaque`): an or-pattern in every field multiplies.
const MOST_ROWS: usize = 512;

impl Checker<'_> {
    /// `(a, b)`, `(a, .., z)` (§1.6.5): one pattern per field of a tuple, `..` standing for the
    /// fields between.
    pub(super) fn tuple_pattern(&mut self, file: FileId, span: Span, pats: &[PatId], scrut: &Ty) -> Option<Matched> {
        let Ty::Tuple(items) = scrut else {
            self.mismatch_text(file, span, &scrut.name(self.design), "a tuple pattern");
            return None;
        };
        let columns = self.positions(file, span, pats, items.len())?;
        let (parts, bound) = self.fields_matched(file, span, &columns, items)?;
        Some(Matched::new(Self::product(&parts, Shape::Fields), bound))
    }

    /// `P(a, b)` and `Tx::Data(a)` (§1.6.5): a tuple struct or a variant with positional fields.
    pub(super) fn constructor_pattern(&mut self, file: FileId, span: Span, path: PathId, pats: &[PatId], scrut: &Ty) -> Option<Matched> {
        let (owner, shape) = self.constructor_of(file, span, path, scrut)?;
        let layout = self.fields_of(owner)?;
        if layout.iter().any(|f| f.name.is_some()) {
            self.mismatch_text(file, span, "a pattern with the fields' names", "a positional pattern");
            return None;
        }
        let columns = self.positions(file, span, pats, layout.len())?;
        let types: Vec<Ty> = layout.iter().map(|f| f.ty.clone()).collect();
        let (parts, bound) = self.fields_matched(file, span, &columns, &types)?;
        Some(Matched::new(Self::product(&parts, shape), bound))
    }

    /// `P { x, y: py, .. }` and `Tx::Data { n }` (§1.6.6): each field named once; a field left out
    /// needs `..`, else `E0184`.
    pub(super) fn struct_pattern(&mut self, file: FileId, span: Span, path: PathId, fields: &[FieldPatNode], rest: bool, scrut: &Ty) -> Option<Matched> {
        let (owner, shape) = self.constructor_of(file, span, path, scrut)?;
        let layout = self.fields_of(owner)?;
        let mut columns: Vec<Option<&FieldPatNode>> = vec![None; layout.len()];
        for field in fields {
            let at = match field.res {
                Resolution::Decl(decl) => layout.iter().position(|f| f.decl == decl),
                _ => layout.iter().position(|f| f.name.as_deref() == Some(field.name.text.as_str())),
            };
            match at {
                Some(i) if columns[i].is_none() => columns[i] = Some(field),
                Some(_) => {
                    self.coded(file, "E0184", field.span, format!("the field `{}` is named twice in one pattern (§1.6.6)", field.name.text), "named twice");
                    return None;
                }
                None => {
                    let of = scrut.name(self.design);
                    self.coded(file, "E0186", field.span, format!("{of} has no field `{}` (§1.6.6)", field.name.text), "no such field");
                    return None;
                }
            }
        }
        let left_out: Vec<String> = layout.iter().zip(&columns).filter(|(_, c)| c.is_none()).filter_map(|(f, _)| f.name.as_deref()).map(|n| format!("`{n}`")).collect();
        if !rest && !left_out.is_empty() {
            self.coded(file, "E0184", span, format!("this pattern leaves out the field {}: name it, or end the pattern with `..` (§1.6.6)", left_out.join(", ")), "field left out");
            return None;
        }
        let mut parts = Vec::new();
        let mut matched = Vec::new();
        for (field, column) in layout.iter().zip(&columns) {
            let part = match column {
                None => Matched::covering(Covers::All),
                Some(node) => match (node.pat, node.binding) {
                    (Some(pat), _) => self.pattern(file, pat, &field.ty)?,
                    (None, Some(binding)) => {
                        self.locals.insert(binding, field.ty.clone());
                        Matched::new(vec![Covers::All], vec![Bound::new(node.name.text.clone(), field.ty.clone())])
                    }
                    (None, None) => Matched::covering(Covers::All),
                },
            };
            parts.push(part.covers.clone());
            matched.push(part);
        }
        let bound = self.merge_bound(file, span, matched)?;
        Some(Matched::new(Self::product(&parts, shape), bound))
    }

    /// `[a, b]`, `[first, .., last]` and `[head, rest @ ..]` (§1.6.7): over an array, with `..`
    /// standing for the elements between.
    pub(super) fn slice_pattern(&mut self, file: FileId, span: Span, pats: &[PatId], scrut: &Ty) -> Option<Matched> {
        let Ty::Array(element, length) = scrut else { return self.not_sliceable(file, span, scrut) };
        let element = (**element).clone();
        let arity = usize::try_from(*length).ok()?;
        let rest_at = pats.iter().position(|p| self.is_rest_pattern(*p));
        let fixed = pats.len() - usize::from(rest_at.is_some());
        let mut matched = Vec::new();
        let mut columns: Vec<Option<PatId>> = Vec::new();
        match rest_at {
            None => columns.extend(pats.iter().map(|p| Some(*p))),
            Some(i) => {
                if fixed > arity {
                    self.mismatch_text(file, span, &format!("a pattern of at most {arity} elements"), &format!("{fixed} elements"));
                    return None;
                }
                columns.extend(pats[..i].iter().map(|p| Some(*p)));
                columns.extend(std::iter::repeat_n(None, arity - fixed));
                columns.extend(pats[i + 1..].iter().map(|p| Some(*p)));
                matched.extend(self.rest_binding(pats[i], &Ty::Array(Box::new(element.clone()), (arity - fixed) as u64)));
            }
        }
        if columns.len() != arity {
            self.mismatch_text(file, span, &format!("a pattern of {arity} elements"), &format!("{} elements", columns.len()));
            return None;
        }
        let items = vec![element; arity];
        let (parts, bound) = self.fields_matched(file, span, &columns, &items)?;
        let all = self.merge_bound(file, span, matched.into_iter().chain(std::iter::once(Matched::new(vec![Covers::All], bound))).collect())?;
        Some(Matched::new(Self::product(&parts, Shape::Fields), all))
    }

    // ---- helpers ---------------------------------------------------------------------------

    /// `E0184`-free mismatch for a slice pattern over a value that is no array or slice.
    fn not_sliceable(&mut self, file: FileId, span: Span, scrut: &Ty) -> Option<Matched> {
        self.mismatch_text(file, span, &scrut.name(self.design), "a slice pattern");
        None
    }

    /// Whether `pat` is `..` or `name @ ..` (the rest of a slice pattern).
    fn is_rest_pattern(&self, pat: PatId) -> bool {
        match &self.design.pat_node(pat).kind {
            PatKind::Rest => true,
            PatKind::Ident { sub: Some(sub), .. } => matches!(self.design.pat_node(*sub).kind, PatKind::Rest),
            _ => false,
        }
    }

    /// The binding of `name @ ..` typed as `ty`; nothing for a bare `..`.
    fn rest_binding(&mut self, pat: PatId, ty: &Ty) -> Option<Matched> {
        let PatKind::Ident { name, binding, .. } = &self.design.pat_node(pat).kind.clone() else { return None };
        self.locals.insert(*binding, ty.clone());
        Some(Matched::new(vec![Covers::All], vec![Bound::new(name.text.clone(), ty.clone())]))
    }

    /// The pattern of each of `arity` fields, `..` standing for those between the patterns before
    /// it and after it; `None` marks a field a `..` skips.
    fn positions(&mut self, file: FileId, span: Span, pats: &[PatId], arity: usize) -> Option<Vec<Option<PatId>>> {
        let rests: Vec<usize> = pats.iter().enumerate().filter(|(_, p)| matches!(self.design.pat_node(**p).kind, PatKind::Rest)).map(|(i, _)| i).collect();
        match rests.as_slice() {
            [] if pats.len() == arity => Some(pats.iter().map(|p| Some(*p)).collect()),
            [] => {
                self.mismatch_text(file, span, &format!("a pattern of {arity} fields"), &format!("{} patterns", pats.len()));
                None
            }
            [at] if pats.len() - 1 <= arity => {
                let mut columns: Vec<Option<PatId>> = pats[..*at].iter().map(|p| Some(*p)).collect();
                columns.extend(std::iter::repeat_n(None, arity - (pats.len() - 1)));
                columns.extend(pats[*at + 1..].iter().map(|p| Some(*p)));
                Some(columns)
            }
            [_] => {
                self.mismatch_text(file, span, &format!("a pattern of at most {arity} fields"), &format!("{} patterns", pats.len() - 1));
                None
            }
            _ => {
                self.coded(file, "E0184", span, "`..` appears once in a pattern (§1.6.5)".to_string(), "more than one `..`");
                None
            }
        }
    }

    /// Each column's pattern against its type (a skipped column matches everything): what each
    /// covers and the names they bind together.
    fn fields_matched(&mut self, file: FileId, span: Span, columns: &[Option<PatId>], types: &[Ty]) -> Option<(Vec<Vec<Covers>>, Vec<Bound>)> {
        let mut parts = Vec::new();
        let mut matched = Vec::new();
        for (column, ty) in columns.iter().zip(types) {
            let part = match column {
                Some(pat) => self.pattern(file, *pat, ty)?,
                None => Matched::covering(Covers::All),
            };
            parts.push(part.covers.clone());
            matched.push(part);
        }
        Some((parts, self.merge_bound(file, span, matched)?))
    }

    /// The names the parts of one pattern bind; one bound twice is `E0184` (§1.6.10).
    fn merge_bound(&mut self, file: FileId, span: Span, parts: Vec<Matched>) -> Option<Vec<Bound>> {
        let mut all: Vec<Bound> = Vec::new();
        for part in parts {
            for bound in part.bound {
                if all.iter().any(|b| b.name == bound.name) {
                    self.coded(file, "E0184", span, format!("the name `{}` is bound twice in one pattern (§1.6.10)", bound.name), "bound twice");
                    return None;
                }
                all.push(bound);
            }
        }
        Some(all)
    }

    /// Every combination of the alternatives of each column, each as one `Covers` made by `wrap`;
    /// past [`MOST_ROWS`] combinations the coverage is not listed.
    fn product(columns: &[Vec<Covers>], shape: Shape) -> Vec<Covers> {
        let mut rows: Vec<Vec<Covers>> = vec![Vec::new()];
        for alternatives in columns {
            if rows.len().saturating_mul(alternatives.len().max(1)) > MOST_ROWS {
                return vec![Covers::Opaque];
            }
            rows = rows.iter().flat_map(|row| alternatives.iter().map(move |a| row.iter().cloned().chain(std::iter::once(a.clone())).collect())).collect();
        }
        rows.into_iter().map(|cells| shape.cover(cells)).collect()
    }

    /// The declaration owning the fields a struct or variant pattern names, and how its coverage is
    /// made: the struct's `Product`, a variant's `Constructor`. The scrutinee must be the type.
    fn constructor_of(&mut self, file: FileId, span: Span, path: PathId, scrut: &Ty) -> Option<(DeclId, Shape)> {
        let Some(Resolution::Decl(target)) = self.design.path_node(path).segments.last().map(|s| s.res) else {
            self.unsupported(file, span, "a pattern naming something that is no struct or variant");
            return None;
        };
        let (ty, owner) = self.constructed_type(target, file, span)?;
        if ty != *scrut {
            self.mismatch(file, span, scrut, &ty);
            return None;
        }
        match self.design.decl_data(target).kind {
            DeclKind::Struct(_) => Some((owner, Shape::Fields)),
            _ => Some((owner, Shape::Variant(owner))),
        }
    }
}
