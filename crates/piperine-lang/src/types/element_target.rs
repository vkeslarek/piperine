//! The target of a commit or a definition that names an element of an array of carriers or a part of a carrier's value
//! (composite-values CV-25; part II §5.2.4): `bits[3] <- x`, `mem[a] <- d`, `st.n <- 3`, `word[2] <- true`,
//! `mem[a].f <- v`, `bits[0] = a`. The carrier is the root of the place; the first indexes, one for each level of the
//! array of carriers, name the element; every step after names levels of that element's value. The type of the target
//! is the type of those levels (§12.1.9). One drive per element: the book tells a definition of one element from the
//! commits of another (`E0221` only when they share an element).

use crate::refusal::RefusedForm;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, Resolution};

use super::check::Checker;
use super::ty::Length;
use super::Ty;

/// The element of a carrier a drive reaches, for the rule that a carrier has one drive (§5.2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Element {
    /// The carrier itself, no element named.
    Whole,
    /// The element a constant index names (row-major for nested arrays).
    At(u64),
    /// A run of consecutive elements a sub-array names: `(first, length)`.
    Run(u64, u64),
    /// An element decided at run time: any of them.
    Any,
    /// An element an index names that the checker cannot fold yet but that reads nothing over time (a generic length, a
    /// param or a generation `for` index), or any drive written inside generation or of a carrier declared there, which
    /// exists only in the iterations and arms elaboration takes (§8.2.23, §8.2.24). The elaborator folds it per instance,
    /// so it reaches the elements it names there, and judges its overlaps there (`elab/drive_overlaps.rs`).
    Unfolded,
}

impl Element {
    /// Whether two drives reach one element as the checker can tell: an element decided at run time may be any of them;
    /// an element only the elaborator folds is judged there, per instance (§8.2.20, `elab/drive_overlaps.rs`).
    pub(super) fn overlaps(self, other: Element) -> bool {
        if self == Element::Any || other == Element::Any {
            return true;
        }
        if self == Element::Unfolded || other == Element::Unfolded {
            return false;
        }
        match (self.span(), other.span()) {
            (Some((first, end)), Some((other_first, other_end))) => first < other_end && other_first < end,
            _ => true,
        }
    }

    /// The subscript that names this element for a person (`[2]`, `[2..4]`, row-major flat), empty when it names no constant
    /// element.
    pub(super) fn subscript(self) -> String {
        match self {
            Element::At(at) => format!("[{at}]"),
            Element::Run(first, length) => format!("[{first}..{}]", first + length),
            Element::Whole | Element::Any | Element::Unfolded => String::new(),
        }
    }

    /// The elements `[first, end)` a constant element or a run covers; `None` for the whole carrier or any element.
    fn span(self) -> Option<(u64, u64)> {
        match self {
            Element::At(at) => Some((at, at + 1)),
            Element::Run(first, length) => Some((first, first + length)),
            Element::Whole | Element::Any | Element::Unfolded => None,
        }
    }
}

/// How one drive reaches a carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DriveKind {
    /// `place = value;`.
    Definition,
    /// `place <- value;`.
    Commit,
}

/// A target resolved: the carrier, the type of the levels it names and the element.
#[derive(Clone, Debug)]
pub(super) struct DriveTarget {
    /// The port or signal that is the root of the place.
    pub(super) carrier: DeclId,
    /// The type of the levels the place names: what the written value has.
    pub(super) held: Ty,
    /// The element the place names.
    pub(super) element: Element,
    /// Whether the place is a `pub` resolved signal of a child (`inst.name`), which this module holds a drive of; the
    /// module's own bookkeeping of definitions and commits does not apply to it.
    pub(super) reached: bool,
}

impl Checker<'_> {
    /// The target of an element or field place `place` (`what`: "commit" or "definition"); `None` after reporting a
    /// target that is no carrier of this module's.
    pub(super) fn projected_target(&mut self, place: ExprId, what: &str) -> Option<DriveTarget> {
        let file = self.file_of_expr(place);
        let span = self.design.expr_node(place).span;
        let (root, steps) = self.place_steps(place);
        let Some(decl) = self.carrier_root(root) else {
            self.coded(file, "E0222", span, format!("the target of a {what} is not a discrete carrier of this module (§8.2.10)"), "not a carrier");
            self.infer(place);
            return None;
        };
        let (port_ty, direction) = match &self.design.decl_data(decl).kind {
            DeclKind::Port(p) => (p.ty, p.dir),
            DeclKind::Signal(s) => (s.ty, None),
            _ => return None,
        };
        // a carrier declared inside generation is the array of its elements to a place outside its loops (§8.2.23)
        let declared = self.port_type(port_ty).map(|declared| self.generated_read(root, decl, declared))?;
        if !matches!(declared.innermost(), Ty::Carrier { .. }) {
            self.coded(file, "E0222", span, format!("the target of a {what} is a node, not a discrete carrier (§8.2.10)"), "a node, not a carrier");
            return None;
        }
        if direction == Some(crate::msl::ast::Dir::In) {
            self.coded(file, "E0222", span, format!("the target of a {what} is an `in` port: a module never writes its own input (§5.2.5)"), "an `in` port");
            return None;
        }
        // one index for each level of the array of carriers names the element
        let (mut levels, mut at) = (Vec::new(), &declared);
        while let Ty::Array(element, length) = at {
            levels.push(length.clone());
            at = element;
        }
        if steps.len() < levels.len() || steps[..levels.len()].iter().any(|step| !matches!(step, Step::Index(_))) {
            self.refuse_with(file, span, RefusedForm::of_whole_array_of_carriers_or_of(), [what.to_string()]);
            self.book.refused.insert(decl);
            self.infer(place);
            return None;
        }
        let element = self.element_of(&steps[..levels.len()], &levels);
        let held = self.infer(place)?;
        Some(DriveTarget { carrier: decl, held, element, reached: false })
    }

    /// The steps from the root of `place` to it, outermost last, and the root expression.
    fn place_steps(&self, place: ExprId) -> (ExprId, Vec<Step>) {
        let mut steps = Vec::new();
        let mut at = place;
        loop {
            match &self.design.expr_node(at).kind {
                ExprKind::Paren(inner) => at = *inner,
                ExprKind::Index { expr, index } => {
                    steps.push(Step::Index(*index));
                    at = *expr;
                }
                // a field of a bundle port or signal is the carrier its leaf stands for: the root of the place (`pom/bundles.rs`),
                // and the element of an array of bundles it is in names the first levels of that leaf (`rx[k].valid`)
                ExprKind::Field { .. } if let Some(place) = self.design.bundle_place(at).filter(|place| self.design.leaf_owner(place.decl).is_some()) => {
                    steps.extend(place.elements.iter().rev().map(|index| Step::Index(*index)));
                    break;
                }
                ExprKind::Field { expr, .. } => {
                    steps.push(Step::Field);
                    at = *expr;
                }
                _ => break,
            }
        }
        steps.reverse();
        (at, steps)
    }

    /// The port or signal a place is rooted at: a name, or the leaf a field of a bundle stands for.
    fn carrier_root(&self, root: ExprId) -> Option<DeclId> {
        if let Some(leaf) = self.design.bundle_field(root) {
            return Some(leaf);
        }
        let ExprKind::Path(path) = &self.design.expr_node(root).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::Port(_) | DeclKind::Signal(_)) => Some(decl),
            _ => None,
        }
    }

    /// The elements the carrier argument `expr` (`x`, `x[k]`, `x[r]` of a nested array, `rx[k].valid`) hands to a port or reads:
    /// a constant element, a run for a sub-array, any element for an index that does not fold, else the whole carrier.
    pub(super) fn element_named_by(&mut self, expr: ExprId) -> Element {
        let (root, steps) = self.place_steps(expr);
        let Some(decl) = self.carrier_root(root).filter(|_| !steps.is_empty()) else { return Element::Whole };
        let port_ty = match &self.design.decl_data(decl).kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return Element::Whole,
        };
        let Some(declared) = self.port_type(port_ty).map(|declared| self.generated_read(root, decl, declared)) else { return Element::Whole };
        self.element_reached(declared, &steps)
    }

    /// The elements of the leaf `decl` the element indexes `indexes` of an array of bundles name (`lanes[k]` given to a bundle
    /// port reaches the element `k` of each of its leaves, struct of arrays): the whole leaf when there is none.
    pub(super) fn element_of_indexes(&mut self, decl: DeclId, indexes: &[ExprId]) -> Element {
        if indexes.is_empty() {
            return Element::Whole;
        }
        let port_ty = match &self.design.decl_data(decl).kind {
            DeclKind::Port(p) => p.ty,
            DeclKind::Signal(s) => s.ty,
            _ => return Element::Whole,
        };
        let Some(declared) = self.port_type(port_ty) else { return Element::Whole };
        let steps: Vec<Step> = indexes.iter().map(|index| Step::Index(*index)).collect();
        self.element_reached(declared, &steps)
    }

    /// The elements the index `steps` reach of a carrier declared `declared`: a constant element, a run for a sub-array, any
    /// element for an index that does not fold, else the whole carrier.
    fn element_reached(&mut self, declared: Ty, steps: &[Step]) -> Element {
        let mut at = declared;
        let mut levels = Vec::new();
        while let Ty::Array(element, length) = at {
            levels.push(length);
            at = *element;
        }
        if steps.len() > levels.len() {
            return Element::Whole;
        }
        let indexed = self.element_of(steps, &levels[..steps.len()]);
        let below: Option<u64> = levels[steps.len()..].iter().try_fold(1u64, |size, length| match length {
            Length::Fixed(count) => size.checked_mul(*count),
            _ => None,
        });
        match (indexed, below) {
            (Element::At(_), None) => Element::Unfolded,
            (Element::At(prefix), Some(size)) if steps.len() < levels.len() => Element::Run(prefix * size, size),
            (found, _) => found,
        }
    }

    /// The element a drive written at `place` reaches of the carrier `carrier`, `written` as its place names it: a drive
    /// written inside generation, or of a carrier declared there, is [`Element::Unfolded`], since which iterations and
    /// arms elaborate it, and so which elements it reaches, is known per instance only (§8.2.23, §8.2.24).
    pub(super) fn judged_element(&self, place: ExprId, carrier: DeclId, written: Element) -> Element {
        let generated = self.inside_generation(self.design.expr_node(place).owner) || self.inside_generation(carrier);
        if generated && written != Element::Any { Element::Unfolded } else { written }
    }

    /// The element the first `indexes` name in an array of `levels`: a constant when every index folds, else any.
    fn element_of(&mut self, indexes: &[Step], levels: &[Length]) -> Element {
        let mut flat = 0u64;
        for (level, (step, length)) in indexes.iter().zip(levels).enumerate() {
            let Step::Index(index) = step else { return Element::Any };
            // the length of the outermost level scales nothing, so a generic one still names the element by its index
            let count = match length {
                Length::Fixed(count) => Some(*count),
                _ if level == 0 => Some(0),
                _ => None,
            };
            let (Some(at), Some(count)) = (self.fold_length(*index), count) else {
                return if self.runtime_read(*index).is_none() { Element::Unfolded } else { Element::Any };
            };
            flat = flat * count + at;
        }
        Element::At(flat)
    }
}

/// One step of a place from its root.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// `[index]`.
    Index(ExprId),
    /// `.name` or `.0`.
    Field,
}
