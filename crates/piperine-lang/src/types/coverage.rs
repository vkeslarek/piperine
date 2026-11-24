//! The elements of a carrier a driver names (§3.3.4, §8.2.20, §8.2.21). A carrier has a value from `(0, 0)` on: its
//! initial value, or the value its one driver specifies, an undelayed definition or the `out` port of a child it is
//! handed to. An array of carriers is `K` carriers, so the rule holds per element: an element no driver names has no
//! value and needs an initial value (`E0187`), and the initial value of an array applies to the elements no undelayed
//! definition names, so it is `E0338` only once every element is defined.

use std::collections::BTreeSet;

use crate::pom::DeclId;

use super::check::Checker;
use super::element_target::Element;
use super::ty::Length;
use super::Ty;

/// The array carriers whose element coverage the checker cannot judge because a length is generic or a param: each
/// instance is judged by the elaborator, where lengths and generation indices are folded.
#[derive(Debug, Clone, Default)]
pub(super) struct DeferredCoverage {
    /// Carriers with no initial value whose type has no `Default` and that a constant element drive names in part (`E0187`).
    pub(super) missing_initial: BTreeSet<DeclId>,
    /// Carriers with an initial value that `out` ports of children hold in part (`E0223` once every element is handed).
    pub(super) handed_initial: BTreeSet<DeclId>,
    /// Carriers with an initial value that undelayed element definitions name in part (`E0338` once every element is defined).
    pub(super) defined_initial: BTreeSet<DeclId>,
}

/// Which elements of one carrier a set of drives names, in row-major order for nested arrays.
#[derive(Debug)]
pub(crate) struct ElementCoverage {
    /// The number of elements, `None` while a length is generic (the elaborator folds it per instantiation).
    total: Option<u64>,
    /// Whether a drive names every element: the whole carrier, or an element decided at run time.
    everything: bool,
    /// Whether a constant element or run names some of the elements of an array whose length is not known yet: the
    /// elaborator judges which are left once it folds the length (`elab/element_coverage.rs`).
    deferred: bool,
    /// The elements `[first, end)` the constant drives name.
    spans: Vec<(u64, u64)>,
}

impl ElementCoverage {
    /// The coverage of the carrier of type `held` with no drive yet.
    pub(super) fn of(held: &Ty) -> Self {
        let mut total = Some(1u64);
        let mut at = held;
        while let Ty::Array(element, length) = at {
            total = match (total, length) {
                (Some(size), Length::Fixed(count)) => size.checked_mul(*count),
                _ => None,
            };
            at = element;
        }
        ElementCoverage { total, everything: false, deferred: false, spans: Vec::new() }
    }

    /// The coverage of a carrier of `total` elements with no drive yet (the elaborator folded the length).
    pub(crate) fn counted(total: u64) -> Self {
        ElementCoverage { total: Some(total), everything: false, deferred: false, spans: Vec::new() }
    }

    /// A drive that reaches the whole carrier.
    pub(crate) fn cover_all(&mut self) {
        self.everything = true;
    }

    /// A drive that reaches the elements `[first, end)`.
    pub(crate) fn cover_run(&mut self, first: u64, end: u64) {
        self.spans.push((first, end));
    }

    /// A drive that reaches `element`.
    pub(super) fn cover(&mut self, element: Element) {
        match element {
            Element::Whole | Element::Any => self.everything = true,
            Element::Unfolded => self.deferred = true,
            Element::At(_) | Element::Run(..) if self.total.is_none() => self.deferred = true,
            Element::At(at) => self.spans.push((at, at + 1)),
            Element::Run(first, length) => self.spans.push((first, first + length)),
        }
    }

    /// Every element is named, or the length is not known yet and a constant element names some of them (judged per
    /// instance by the elaborator, `is_deferred`).
    pub(crate) fn is_complete(&self) -> bool {
        self.everything || self.deferred || (self.total.is_some() && self.gaps().is_empty())
    }

    /// Whether only the elaborator can tell if every element is named: some constant element is, over an array whose
    /// length is not known yet.
    pub(super) fn is_deferred(&self) -> bool {
        self.deferred && !self.everything
    }

    /// The runs `[first, end)` of elements no drive names; empty while the total is not known.
    fn gaps(&self) -> Vec<(u64, u64)> {
        let Some(total) = self.total else { return Vec::new() };
        let mut spans = self.spans.clone();
        spans.sort_unstable();
        let (mut gaps, mut next) = (Vec::new(), 0u64);
        for (first, end) in spans {
            if first > next {
                gaps.push((next, first.min(total)));
            }
            next = next.max(end);
        }
        if next < total {
            gaps.push((next, total));
        }
        gaps
    }

    /// The carrier `name` as the elements no drive names (`b[2]`, `b[1..4]`, `b[0..1], b[3..4]`), `None` when none
    /// is left; the carrier's own name when there is nothing to tell apart.
    pub(crate) fn unnamed(&self, name: &str) -> Option<String> {
        if self.is_complete() {
            return None;
        }
        let Some(total) = self.total.filter(|total| *total > 1) else { return Some(name.to_string()) };
        let runs: Vec<String> = self
            .gaps()
            .into_iter()
            .map(|(first, end)| if end - first == 1 { format!("{name}[{first}]") } else if (first, end) == (0, total) { name.to_string() } else { format!("{name}[{first}..{end}]") })
            .collect();
        Some(runs.join(", "))
    }
}

impl Checker<'_> {
    /// The elements of `decl` (of type `held`) an undelayed definition names.
    pub(super) fn undelayed_coverage(&self, decl: DeclId, held: &Ty) -> ElementCoverage {
        let mut coverage = ElementCoverage::of(held);
        if self.book.undelayed.contains(&decl) {
            coverage.cover(Element::Whole);
        }
        for (carrier, element, _) in &self.book.undelayed_elements {
            if *carrier == decl {
                coverage.cover(*element);
            }
        }
        coverage
    }

    /// The elements of `decl` a driver specifies the value of: an undelayed definition, an `out` port of a child it is
    /// handed to, or a drive refused as unsupported (the refusal is the diagnostic, no second one for the carrier).
    pub(super) fn driven_coverage(&self, decl: DeclId, held: &Ty) -> ElementCoverage {
        let mut coverage = self.undelayed_coverage(decl, held);
        if self.book.refused.contains(&decl) {
            coverage.cover(Element::Whole);
        }
        for (carrier, element, _, _) in &self.book.handed {
            if *carrier == decl {
                coverage.cover(*element);
            }
        }
        coverage
    }

    /// `E0338` for an array of carriers with an initial value whose every element an undelayed definition names: the
    /// initial value would apply to no element (§8.2.21). With an element left, the initial value is that element's.
    pub(super) fn initial_values_under_element_definitions(&mut self, children: &[DeclId]) {
        let defined: BTreeSet<DeclId> = self.book.undelayed_elements.iter().map(|(carrier, _, _)| *carrier).collect();
        for decl in children.iter().copied().filter(|decl| defined.contains(decl)) {
            let Some(held) = self.carriers.get(&decl).cloned() else { continue };
            if !self.has_initial(decl) {
                continue;
            }
            let coverage = self.undelayed_coverage(decl, &held);
            if coverage.is_deferred() {
                self.deferred_coverage.defined_initial.insert(decl);
                continue;
            }
            if !coverage.is_complete() {
                continue;
            }
            let Some((_, _, span)) = self.book.undelayed_elements.iter().find(|(carrier, _, _)| *carrier == decl).copied() else { continue };
            let data = self.design.decl_data(decl);
            let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let message = format!("`{name}` has an initial value and an undelayed definition of every element: the definitions would replace it in the microstep that commits them (§8.2.21)");
            self.coded(data.file, "E0338", span, message, "initial value under an undelayed definition");
        }
    }
}
