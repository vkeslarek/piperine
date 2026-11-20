//! What the patterns of a `match`, `let` or `for` cover of the value they are matched against, and
//! the first value they leave out (§1.8.6). A `match` is exhaustive, and a pattern irrefutable
//! (§1.6.11), when nothing is left out.

use crate::pom::{DeclId, DeclKind, Design};

use super::Ty;

/// What one alternative of a pattern covers of its scrutinee.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Covers {
    /// Every value: `_` and a binding.
    All,
    /// One variant of an enum.
    Variant(DeclId),
    Bool(bool),
    /// The integers from the first through the second, both included: a literal, or a range
    /// pattern with bounds the checker knows.
    Integers(i128, i128),
    /// A value the checker cannot list: a float, or a `const` it does not fold. It covers nothing
    /// that `missing` counts, so only `_` or a binding completes a `match` over such a type.
    Opaque,
    /// `None` of an `Option`.
    Nothing,
    /// `Some(p)`, with what `p` covers of the payload.
    Some(Vec<Covers>),
}

/// The judgement of coverage over one design.
pub(super) struct Coverage<'d> {
    design: &'d Design,
}

impl<'d> Coverage<'d> {
    pub(super) fn new(design: &'d Design) -> Self {
        Coverage { design }
    }

    /// The first value of `scrut` that `covered` leaves out, written as the pattern that would
    /// cover it; `None` when every value is covered.
    pub(super) fn missing(&self, scrut: &Ty, covered: &[Covers]) -> Option<String> {
        if covered.contains(&Covers::All) {
            return None;
        }
        match scrut {
            Ty::Bool => [false, true].into_iter().find(|b| !covered.contains(&Covers::Bool(*b))).map(|b| b.to_string()),
            Ty::Option(inner) => {
                if !covered.contains(&Covers::Nothing) {
                    return Some("None".to_string());
                }
                let payloads: Vec<Covers> = covered
                    .iter()
                    .filter_map(|c| match c {
                        Covers::Some(p) => Some(p.clone()),
                        _ => None,
                    })
                    .flatten()
                    .collect();
                self.missing(inner, &payloads).map(|m| format!("Some({m})"))
            }
            Ty::Enum(e) => self.missing_variant(*e, covered),
            Ty::UInt(n) => Self::missing_integer(0, (1i128 << n) - 1, covered),
            Ty::Int(n) => Self::missing_integer(-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1, covered),
            _ => Some("_".to_string()),
        }
    }

    /// The first variant of the enum `decl` that `covered` leaves out, as `Enum::Variant`.
    fn missing_variant(&self, decl: DeclId, covered: &[Covers]) -> Option<String> {
        let data = self.design.decl_data(decl);
        let enum_name = data.name.as_ref().map_or("", |n| n.text.as_str());
        data.children.iter().copied().filter(|v| matches!(self.design.decl_data(*v).kind, DeclKind::Variant(_))).find(|v| !covered.contains(&Covers::Variant(*v))).map(|v| format!("{enum_name}::{}", self.design.decl_data(v).name.as_ref().map_or("", |n| n.text.as_str())))
    }

    /// The first integer of `lo..=hi` that no interval of `covered` holds, as a pattern: the value
    /// itself, or the run of values it starts.
    fn missing_integer(lo: i128, hi: i128, covered: &[Covers]) -> Option<String> {
        let mut intervals: Vec<(i128, i128)> = covered
            .iter()
            .filter_map(|c| match c {
                Covers::Integers(a, b) => Some((*a, *b)),
                _ => None,
            })
            .collect();
        intervals.sort_unstable();
        let mut next = lo;
        for (a, b) in intervals {
            if a > next {
                return Some(Self::run(next, a - 1));
            }
            next = next.max(b + 1);
        }
        (next <= hi).then(|| Self::run(next, hi))
    }

    /// `first` alone, or `first..=last`.
    fn run(first: i128, last: i128) -> String {
        if first == last { first.to_string() } else { format!("{first}..={last}") }
    }
}
