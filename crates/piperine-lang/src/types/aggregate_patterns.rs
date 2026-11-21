//! Patterns over aggregates (§1.6.5 to §1.6.7): tuple, tuple-struct and variant, struct, slice and
//! reference patterns. Each types its names and says what it covers; the fields of a struct, a
//! variant or a tuple are columns of the coverage judgement (`cover.rs`).

use crate::msl::Span;
use crate::pom::{FileId, PatId, PatKind};

use super::check::Checker;
use super::cover::Covers;
use super::patterns::{Bound, Matched};
use super::Ty;

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
        Some(Matched::new(Self::product(&parts), bound))
    }

    // ---- helpers ---------------------------------------------------------------------------

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

    /// Every combination of the alternatives of each column, each as one `Covers::Product`;
    /// past [`MOST_ROWS`] combinations the coverage is not listed.
    fn product(columns: &[Vec<Covers>]) -> Vec<Covers> {
        let mut rows: Vec<Vec<Covers>> = vec![Vec::new()];
        for alternatives in columns {
            if rows.len().saturating_mul(alternatives.len().max(1)) > MOST_ROWS {
                return vec![Covers::Opaque];
            }
            rows = rows.iter().flat_map(|row| alternatives.iter().map(move |a| row.iter().cloned().chain(std::iter::once(a.clone())).collect())).collect();
        }
        rows.into_iter().map(Covers::Product).collect()
    }

}
