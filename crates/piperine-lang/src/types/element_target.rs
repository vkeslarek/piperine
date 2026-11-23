//! The target of a commit or a definition that names an element of an array of carriers or a part of a carrier's value
//! (composite-values CV-25; part II §5.2.4): `bits[3] <- x`, `mem[a] <- d`, `st.n <- 3`, `word[2] <- true`,
//! `mem[a].f <- v`, `bits[0] = a`. The carrier is the root of the place; the first indexes, one for each level of the
//! array of carriers, name the element; every step after names levels of that element's value. The type of the target
//! is the type of those levels (§12.1.9). One drive per element: the book tells a definition of one element from the
//! commits of another (`E0221` only when they share an element).

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
    /// An element decided at run time: any of them.
    Any,
}

impl Element {
    /// Whether two drives may reach one element.
    pub(super) fn overlaps(self, other: Element) -> bool {
        match (self, other) {
            (Element::At(a), Element::At(b)) => a == b,
            _ => true,
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
        let declared = self.port_type(port_ty)?;
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
            self.unsupported(file, span, &format!("a {what} of a whole array of carriers or of a part of one: name an element"));
            self.infer(place);
            return None;
        }
        let element = self.element_of(&steps[..levels.len()], &levels);
        let held = self.infer(place)?;
        Some(DriveTarget { carrier: decl, held, element })
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

    /// The port or signal a place is rooted at.
    fn carrier_root(&self, root: ExprId) -> Option<DeclId> {
        let ExprKind::Path(path) = &self.design.expr_node(root).kind else { return None };
        match self.design.path_node(*path).segments.last().map(|s| s.res) {
            Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::Port(_) | DeclKind::Signal(_)) => Some(decl),
            _ => None,
        }
    }

    /// The element the first `indexes` name in an array of `levels`: a constant when every index folds, else any.
    fn element_of(&mut self, indexes: &[Step], levels: &[Length]) -> Element {
        let mut flat = 0u64;
        for (step, length) in indexes.iter().zip(levels) {
            let Step::Index(index) = step else { return Element::Any };
            let (Some(at), Length::Fixed(count)) = (self.fold_length(*index), length) else { return Element::Any };
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
