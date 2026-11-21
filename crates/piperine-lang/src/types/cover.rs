//! What the patterns of a `match`, `let` or `for` cover of the value they are matched against, and
//! the first value they leave out (§1.8.6). A `match` is exhaustive, and a pattern irrefutable
//! (§1.6.11), when nothing is left out.
//!
//! The judgement is the usual one over rows of patterns: the first column is split into the
//! constructors of its type (a variant, `Some`/`None`, `true`/`false`, a run of integers, the one
//! constructor of a tuple, a struct or an array), each constructor keeps the rows that cover it,
//! and the columns it opens (its fields) are judged with the rest.

use std::collections::BTreeMap;

use crate::pom::{DeclId, DeclKind, Design};

use super::aggregate::FieldType;
use super::Ty;

/// What one alternative of a pattern covers of its scrutinee.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Covers {
    /// Every value: `_` and a binding.
    All,
    /// One variant of an enum, without fields.
    Variant(DeclId),
    /// A tuple, a struct or an array, with what each field or element covers.
    Product(Vec<Covers>),
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

/// One row of the matrix: what one alternative covers in each column.
type Row = Vec<Covers>;

/// The judgement of coverage over one design.
pub(super) struct Coverage<'d> {
    design: &'d Design,
    layouts: &'d BTreeMap<DeclId, Vec<FieldType>>,
}

impl<'d> Coverage<'d> {
    pub(super) fn new(design: &'d Design, layouts: &'d BTreeMap<DeclId, Vec<FieldType>>) -> Self {
        Coverage { design, layouts }
    }

    /// The first value of `scrut` that `covered` leaves out, written as the pattern that would
    /// cover it; `None` when every value is covered.
    pub(super) fn missing(&self, scrut: &Ty, covered: &[Covers]) -> Option<String> {
        let rows: Vec<Row> = covered.iter().map(|c| vec![c.clone()]).collect();
        self.missing_rows(std::slice::from_ref(scrut), &rows).and_then(|found| found.into_iter().next())
    }

    /// The first combination of values of the column types `tys` that no row covers, one pattern
    /// per column; `None` when the rows cover them all.
    fn missing_rows(&self, tys: &[Ty], rows: &[Row]) -> Option<Vec<String>> {
        let Some((first, rest)) = tys.split_first() else {
            return rows.is_empty().then(Vec::new);
        };
        match first {
            Ty::Bool => [false, true].into_iter().find_map(|value| {
                let sub = Self::specialize(rows, |c| match c {
                    Covers::All => vec![vec![]],
                    Covers::Bool(b) if *b == value => vec![vec![]],
                    _ => vec![],
                });
                self.missing_rows(rest, &sub).map(|w| Self::prefixed(value.to_string(), w))
            }),
            Ty::Option(inner) => {
                let none = Self::specialize(rows, |c| match c {
                    Covers::All | Covers::Nothing => vec![vec![]],
                    _ => vec![],
                });
                if let Some(found) = self.missing_rows(rest, &none) {
                    return Some(Self::prefixed("None".to_string(), found));
                }
                let some = Self::specialize(rows, |c| match c {
                    Covers::All => vec![vec![Covers::All]],
                    Covers::Some(alternatives) => alternatives.iter().map(|a| vec![a.clone()]).collect(),
                    _ => vec![],
                });
                let mut columns = vec![(**inner).clone()];
                columns.extend_from_slice(rest);
                self.missing_rows(&columns, &some).map(|mut w| {
                    let rest = w.split_off(1);
                    Self::prefixed(format!("Some({})", w[0]), rest)
                })
            }
            Ty::Enum(decl) => self.missing_variant(*decl, rest, rows),
            Ty::Struct(decl) => {
                let fields = self.layouts.get(decl).cloned().unwrap_or_default();
                let types: Vec<Ty> = fields.iter().map(|f| f.ty.clone()).collect();
                let name = self.design.decl_data(*decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.missing_product(&types, rest, rows, |parts| Self::spell_fields(&name, &fields, parts))
            }
            Ty::Tuple(items) => self.missing_product(items, rest, rows, |parts| format!("({})", parts.join(", "))),
            Ty::Array(element, length) if *length <= 256 => {
                let items = vec![(**element).clone(); *length as usize];
                self.missing_product(&items, rest, rows, |parts| format!("[{}]", parts.join(", ")))
            }
            Ty::UInt(n) => self.missing_integers(0, (1i128 << n) - 1, rest, rows),
            Ty::Int(n) => self.missing_integers(-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1, rest, rows),
            _ => {
                let sub = Self::specialize(rows, |c| match c {
                    Covers::All => vec![vec![]],
                    _ => vec![],
                });
                self.missing_rows(rest, &sub).map(|w| Self::prefixed("_".to_string(), w))
            }
        }
    }

    /// The rows left after one constructor: each row whose first cell `cells` maps to
    /// replacement cells (none: the row does not cover the constructor) is rewritten with them in
    /// front of its other cells.
    fn specialize(rows: &[Row], cells: impl Fn(&Covers) -> Vec<Vec<Covers>>) -> Vec<Row> {
        rows.iter()
            .flat_map(|row| {
                let (first, tail) = row.split_first().map_or((Covers::Opaque, &[][..]), |(f, t)| (f.clone(), t));
                cells(&first).into_iter().map(|mut replacement| {
                    replacement.extend_from_slice(tail);
                    replacement
                })
            })
            .collect()
    }

    /// `first` in front of the witnesses `rest`.
    fn prefixed(first: String, rest: Vec<String>) -> Vec<String> {
        std::iter::once(first).chain(rest).collect()
    }

    /// A type with one constructor of the field types `types`: tuple, struct or array. `spell`
    /// writes the witnesses of the fields as one pattern.
    fn missing_product(&self, types: &[Ty], rest: &[Ty], rows: &[Row], spell: impl Fn(&[String]) -> String) -> Option<Vec<String>> {
        let arity = types.len();
        let sub = Self::specialize(rows, |c| match c {
            Covers::All => vec![vec![Covers::All; arity]],
            Covers::Product(cells) if cells.len() == arity => vec![cells.clone()],
            _ => vec![],
        });
        let columns: Vec<Ty> = types.iter().chain(rest).cloned().collect();
        self.missing_rows(&columns, &sub).map(|mut found| {
            let tail = found.split_off(arity);
            Self::prefixed(spell(&found), tail)
        })
    }

    /// `Name { x: _, y: 1 }` over the witnesses of the fields; a positional struct writes `Name(..)`.
    fn spell_fields(name: &str, fields: &[FieldType], parts: &[String]) -> String {
        if fields.is_empty() {
            return name.to_string();
        }
        if fields.iter().all(|f| f.name.is_none()) {
            return format!("{name}({})", parts.join(", "));
        }
        let named: Vec<String> = fields.iter().zip(parts).map(|(f, p)| format!("{}: {p}", f.name.as_deref().unwrap_or("_"))).collect();
        format!("{name} {{ {} }}", named.join(", "))
    }

    /// An enum: each variant is a constructor opening its fields as columns.
    fn missing_variant(&self, decl: DeclId, rest: &[Ty], rows: &[Row]) -> Option<Vec<String>> {
        let data = self.design.decl_data(decl);
        let enum_name = data.name.as_ref().map_or("", |n| n.text.as_str());
        for variant in data.children.iter().copied().filter(|v| matches!(self.design.decl_data(*v).kind, DeclKind::Variant(_))) {
            let fields = self.layouts.get(&variant).cloned().unwrap_or_default();
            let arity = fields.len();
            let sub = Self::specialize(rows, |c| match c {
                Covers::All => vec![vec![Covers::All; arity]],
                Covers::Variant(v) if *v == variant => vec![vec![]],
                _ => vec![],
            });
            let columns: Vec<Ty> = fields.iter().map(|f| f.ty.clone()).chain(rest.iter().cloned()).collect();
            if let Some(mut found) = self.missing_rows(&columns, &sub) {
                let tail = found.split_off(arity);
                let name = self.design.decl_data(variant).name.as_ref().map_or("", |n| n.text.as_str());
                let spelled = if arity == 0 { format!("{enum_name}::{name}") } else { format!("{enum_name}::{}", Self::spell_fields(name, &fields, &found)) };
                return Some(Self::prefixed(spelled, tail));
            }
        }
        None
    }

    /// An integer column over `lo..=hi`: the range is cut where a row's interval starts or ends,
    /// and each piece keeps the rows that hold all of it.
    fn missing_integers(&self, lo: i128, hi: i128, rest: &[Ty], rows: &[Row]) -> Option<Vec<String>> {
        let mut cuts: Vec<i128> = vec![lo];
        for row in rows {
            if let Some(Covers::Integers(a, b)) = row.first() {
                cuts.extend([(*a).clamp(lo, hi + 1), (*b + 1).clamp(lo, hi + 1)]);
            }
        }
        cuts.push(hi + 1);
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (from, to) = (pair[0], pair[1] - 1);
            let sub = Self::specialize(rows, |c| match c {
                Covers::All => vec![vec![]],
                Covers::Integers(a, b) if *a <= from && to <= *b => vec![vec![]],
                _ => vec![],
            });
            if let Some(found) = self.missing_rows(rest, &sub) {
                return Some(Self::prefixed(Self::run(from, to), found));
            }
        }
        None
    }

    /// `first` alone, or `first..=last`.
    fn run(first: i128, last: i128) -> String {
        if first == last { first.to_string() } else { format!("{first}..={last}") }
    }
}
