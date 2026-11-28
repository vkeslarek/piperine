//! Definitions (DIG-04, part III §8.2): `place = expr [after d];` makes this module's drive of a
//! discrete carrier a function of what the expression reads. The target is one the module may
//! drive (`E0222`; an element or a field of one: `element_target.rs`), the expression has the carrier's value type (`E0337`) and is discrete
//! (`E0336`), a delay is an `f64` (`E0433`), a carrier is defined once (`E0335`), an undelayed
//! definition leaves an initial value no room (`E0338`) and a loop of undelayed definitions is
//! `E0339`. The same targets, under the same rules, are those of a commit (`reaction.rs`).

use std::collections::{BTreeMap, BTreeSet};

use crate::refusal::forms::ValueForm;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, DefinitionData, ExprId, ExprKind, Resolution};

use crate::types::analog::nature_rules::NatureRules;
use super::CarrierTyping;
use super::element_target::{DriveKind, DriveTarget, Element};
use crate::types::Ty;

/// What one module drives, so far: by carrier, where.
#[derive(Debug, Default)]
pub(in crate::types) struct DriveBook {
    /// The carriers a definition drives.
    pub(super) defined: BTreeMap<DeclId, Span>,
    /// The carriers a commit drives.
    pub(super) committed: BTreeMap<DeclId, Span>,
    /// The carriers handed to an `out` port of an instance, in source order, with where and in
    /// which file.
    pub(super) handed: Vec<(DeclId, Element, Span, crate::pom::FileId)>,
    /// The undelayed definitions in source order: the target, where, and the carriers its value
    /// reads (directly or through a `let`).
    loops: Vec<(DeclId, Span, BTreeSet<DeclId>)>,
    /// The drives of elements of arrays of carriers, and of parts of values: by carrier, element, how and where.
    pub(super) elements: Vec<(DeclId, Element, DriveKind, Span)>,
    /// The carriers an undelayed definition drives: the definition gives the carrier its value from `(0, 0)`.
    pub(super) undelayed: BTreeSet<DeclId>,
    /// The elements of arrays of carriers an undelayed definition drives, with where: those elements have a value from
    /// `(0, 0)` on (`coverage.rs`).
    pub(super) undelayed_elements: Vec<(DeclId, Element, Span)>,
    /// The carriers a drive of which was refused as unsupported (a whole array, a sub-array): the refusal is the one
    /// diagnostic, so the carrier counts as driven and is not also reported as one nothing drives.
    pub(super) refused: BTreeSet<DeclId>,
}

impl CarrierTyping<'_, '_> {
    /// The carrier `place` names and the value type it holds, when this module may drive it;
    /// else `E0222` naming `what` and `None`. A form of place the subset does not have (an
    /// element, a field) is `P0001`.
    pub(super) fn drive_target(&mut self, place: ExprId, what: &str) -> Option<DriveTarget> {
        let written = self.written_target(place, what)?;
        let element = self.judged_element(place, written.carrier, written.element);
        Some(DriveTarget { element, ..written })
    }

    /// [`CarrierTyping::drive_target`] with the element as the place writes it, before generation is taken into account.
    fn written_target(&mut self, place: ExprId, what: &str) -> Option<DriveTarget> {
        let node = self.state.design.expr_node(place).clone();
        let file = self.file_of_expr(place);
        match &node.kind {
            ExprKind::Paren(inner) => self.written_target(*inner, what),
            ExprKind::Index { .. } | ExprKind::Field { res: Resolution::Deferred(_) | Resolution::Unresolved, .. } => {
                let target = self.projected_target(place, what)?;
                self.state.result.exprs[place.index()] = Some(target.held.clone());
                Some(target)
            }
            ExprKind::Field { .. } if let Some(found) = self.state.design.bundle_place(place).filter(|found| self.state.design.leaf_owner(found.decl).is_some()) => {
                // a field of a bundle port or signal is the carrier its leaf stands for (§7 rule 12); at an element of an array of
                // bundles (`tx[k].a`) it is that element of the leaf, a place like `x[k]`
                if found.elements.is_empty() {
                    return self.declared_target(place, found.decl, what);
                }
                let target = self.projected_target(place, what)?;
                self.state.result.exprs[place.index()] = Some(target.held.clone());
                Some(target)
            }
            ExprKind::Field { res: Resolution::Decl(item), .. } => {
                // a field of a bundle read through a handle: the handle is read-only (§7 rule 13), `generics` B's rule
                if matches!(&self.state.design.decl_data(*item).kind, DeclKind::Field(field) if field.dir.is_some()) {
                    return None;
                }
                if let Some(reached) = self.modules().reached_drive_target(place, *item) {
                    return Some(reached);
                }
                let text = match self.state.design.decl_data(*item).kind {
                    DeclKind::Port(_) | DeclKind::Signal(_) => "a carrier of an instance",
                    _ => "a member of an instance",
                };
                let message = format!("the target of a {what} is {text}: a parent never writes a child's drive (§8.2.10)");
                self.coded(file, "E0222", node.span, message, "a child's drive");
                None
            }
            ExprKind::Path(path) => {
                let res = self.state.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                let Resolution::Decl(decl) = res else {
                    if matches!(res, Resolution::Builtin(_)) {
                        self.coded(file, "E0222", node.span, format!("the target of a {what} is not a discrete carrier (§8.2.10)"), "not a carrier");
                    }
                    return None;
                };
                self.declared_target(place, decl, what)
            }
            _ => {
                self.coded(file, "E0222", node.span, format!("the target of a {what} is not a place (§8.2.10)"), "not a place");
                self.expressions().infer(place);
                None
            }
        }
    }

    /// The target of a definition or a commit that names the port or signal `decl` (a path, or a field of a bundle:
    /// the leaf stands for it, §7 rule 12): the carrier and its value type, or `E0222`.
    fn declared_target(&mut self, place: ExprId, decl: DeclId, what: &str) -> Option<DriveTarget> {
        let node = self.state.design.expr_node(place).clone();
        let file = self.file_of_expr(place);
        let (port_ty, direction) = match &self.state.design.decl_data(decl).kind {
            DeclKind::Port(p) => (Some(p.ty), p.dir),
            DeclKind::Signal(s) => (Some(s.ty), None),
            DeclKind::Param(_) => return self.not_drivable(file, node.span, what, "a param"),
            DeclKind::Binding(_) => return self.not_drivable(file, node.span, what, "a `let` name"),
            _ => return self.not_drivable(file, node.span, what, "not a discrete carrier"),
        };
        // a carrier declared inside generation is the array of its elements to a place outside its loops (§8.2.23)
        let ty = port_ty.and_then(|t| self.port_type(t)).map(|ty| self.modules().generated_read(place, decl, ty))?;
        match ty {
            Ty::Carrier { value, .. } => {
                if direction == Some(crate::msl::ast::Dir::In) {
                    return self.not_drivable(file, node.span, what, "an `in` port: a module never writes its own input (§5.2.5)");
                }
                self.state.result.exprs[place.index()] = Some((*value).clone());
                Some(DriveTarget { carrier: decl, held: *value, element: Element::Whole, reached: false })
            }
            array @ Ty::Array(..) if matches!(array.innermost(), Ty::Carrier { .. }) => {
                self.refuse_with(file, node.span, ValueForm::OfWholeArrayOfCarriersOrOf, [what.to_string()]);
                self.state.carriers.book.refused.insert(decl);
                None
            }
            _ => self.not_drivable(file, node.span, what, "a node, not a discrete carrier"),
        }
    }

    /// `E0222`: the target of a `what` is `why`, which this module may not drive; `None` after the report.
    fn not_drivable(&mut self, file: crate::pom::FileId, span: Span, what: &str, why: &str) -> Option<DriveTarget> {
        let why = if why.starts_with("a ") || why.starts_with("an ") || why.starts_with("not ") { why.to_string() } else { format!("a {why}") };
        self.coded(file, "E0222", span, format!("the target of a {what} is {why}, which this module may not drive (§8.2.10)"), "this module may not drive it");
        None
    }

    /// `place = value [after delay];`
    pub(in crate::types) fn definition(&mut self, data: &DefinitionData) {
        self.state.carriers.in_definition += 1;
        self.definition_parts(data);
        self.state.carriers.in_definition -= 1;
    }

    /// The parts of a definition, checked with the position recorded (`definition`).
    fn definition_parts(&mut self, data: &DefinitionData) {
        let place_span = self.state.design.expr_node(data.place).span;
        let file = self.file_of_expr(data.place);
        let target = self.drive_target(data.place, "definition");
        match &target {
            Some(target) => self.analog().check_drive_value(target, data.value, "E0337"),
            None => {
                self.expressions().infer(data.value);
            }
        }
        if let Some(after) = data.after {
            self.expressions().check_as(after, &Ty::F64, "E0433");
        }
        for (expr, what) in [(Some(data.value), "a definition"), (data.after, "the delay of a definition")] {
            let Some(expr) = expr else { continue };
            if self.analog().continuous(expr) {
                let span = self.state.design.expr_node(expr).span;
                let message = format!("{what} reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`, directly or through a `let`, a function or a result: it must be a discrete expression (§8.2.5)");
                self.coded(file, "E0336", span, message, "a continuous expression");
            }
        }
        let Some(DriveTarget { carrier, element, reached, .. }) = target else { return };
        if reached {
            return;
        }
        if element != Element::Whole {
            return self.define_element(file, carrier, element, place_span, data.after.is_none());
        }
        if self.state.carriers.book.defined.insert(carrier, place_span).is_some() {
            let name = self.state.design.decl_data(carrier).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            self.coded(file, "E0335", place_span, format!("two definitions of `{name}` share a level (§8.2.20)"), "defined twice");
            return;
        }
        if data.after.is_none() {
            if self.has_initial(carrier) {
                let name = self.state.design.decl_data(carrier).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.coded(file, "E0338", place_span, format!("`{name}` has an initial value and an undelayed definition: the definition would replace it in the microstep that commits it (§8.2.21)"), "initial value under an undelayed definition");
            }
            let mut reads = BTreeSet::new();
            self.carrier_reads(data.value, &mut reads, &mut BTreeSet::new());
            self.state.carriers.book.loops.push((carrier, place_span, reads));
            self.state.carriers.book.undelayed.insert(carrier);
        }
    }

    /// A definition of one element of an array of carriers (`bits[0] = a`): `E0335` when another definition shares the
    /// element, `E0221` when a commit does (§5.2.4); a carrier with a definition of a part and an initial value
    /// is `E0338` once undelayed definitions cover every element (`coverage.rs`).
    fn define_element(&mut self, file: crate::pom::FileId, carrier: DeclId, element: Element, span: Span, undelayed: bool) {
        let name = self.state.design.decl_data(carrier).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let shared = |kind: DriveKind, book: &DriveBook| book.elements.iter().any(|(c, e, k, _)| *c == carrier && *k == kind && e.overlaps(element));
        if shared(DriveKind::Definition, &self.state.carriers.book) {
            self.coded(file, "E0335", span, format!("two definitions of `{name}` share an element (§8.2.20)"), "defined twice");
        } else if shared(DriveKind::Commit, &self.state.carriers.book) || (element != Element::Unfolded && self.state.carriers.book.committed.contains_key(&carrier)) {
            self.coded(file, "E0221", span, format!("`{name}` has a definition and a commit of one element in one module: an element is driven by one or the other (§5.2, §8.2.20)"), "definition and commit");
        }
        // an initial value under a definition of some elements is the value of the others (§8.2.21, "on the levels defined"): `E0338` is raised once every element is defined (`coverage.rs`)
        if undelayed {
            self.state.carriers.book.undelayed_elements.push((carrier, element, span));
        }
        self.state.carriers.book.elements.push((carrier, element, DriveKind::Definition, span));
    }

    /// A continuous argument for the value input `input` of `module`, which a definition of `module`
    /// reads, directly or through a `let` (§8.3.9): each read is checked as if the caller's expression
    /// were written there, and a definition is discrete (§8.2.5), so it is `E0336` at the call `call`
    /// naming the input. A discrete argument, or an input only an `analog` block reads, is accepted.
    pub(in crate::types) fn definition_value_input(&mut self, call: ExprId, module: DeclId, input: DeclId, arg: ExprId) {
        let read_by_a_definition = self.state.design.decl_data(module).children.clone().into_iter().any(|child| match &self.state.design.decl_data(child).kind {
            DeclKind::Definition(definition) => [Some(definition.value), definition.after].into_iter().flatten().any(|expr| self.modules().reads_decl(expr, input, &mut Vec::new())),
            _ => false,
        });
        if !read_by_a_definition || !self.analog().continuous(arg) {
            return;
        }
        let name = |decl: DeclId| self.state.design.decl_data(decl).name.as_ref().map_or(String::new(), |n| n.text.clone());
        let message = format!("the argument for the value input `{}` of `{}` is continuous (it reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`) and a definition of `{}` reads it: a definition is a discrete expression (§8.3.9, §8.2.5)", name(input), name(module), name(module));
        let (file, span) = (self.file_of_expr(call), self.state.design.expr_node(call).span);
        self.coded(file, "E0336", span, message, "a continuous argument read by a definition");
    }

    /// Whether a port or a signal declares an initial value.
    pub(in crate::types) fn has_initial(&self, carrier: DeclId) -> bool {
        match &self.state.design.decl_data(carrier).kind {
            DeclKind::Port(p) => p.default.is_some(),
            DeclKind::Signal(s) => s.init.is_some(),
            _ => false,
        }
    }

    /// The discrete carriers `expr` reads, directly or through a `let`.
    pub(super) fn carrier_reads(&mut self, expr: ExprId, out: &mut BTreeSet<DeclId>, seen: &mut BTreeSet<ExprId>) {
        if !seen.insert(expr) {
            return;
        }
        let node = self.state.design.expr_node(expr);
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(d)) = self.state.design.path_node(*path).segments.last().map(|s| s.res)
        {
            if self.state.result.carriers.contains_key(&d) {
                out.insert(d);
            } else if let Some(init) = self.analog().binding_init(d) {
                self.carrier_reads(init, out, seen);
            }
        }
        for child in crate::pom::BodyWalk::new(self.state.design).expression_children(&node.kind) {
            self.carrier_reads(child, out, seen);
        }
    }

    /// Whether `carrier` is a carrier of a nature with no `impl Resolve`: one drive, no combination (§5.2.1).
    fn is_one_driver_carrier(&self, carrier: DeclId) -> bool {
        matches!(self.state.result.carriers.get(&carrier).map(Ty::innermost), Some(Ty::Carrier { nature, .. }) if NatureRules::of(self.state.design, *nature).one_driver)
    }

    /// `E0339` once per loop of undelayed definitions: a drive that depends on itself with no
    /// reaction and no `after` in between (§8.2.3).
    pub(in crate::types) fn definition_loops(&mut self) {
        let loops = std::mem::take(&mut self.state.carriers.book.loops);
        let targets: Vec<DeclId> = loops.iter().map(|(t, _, _)| *t).collect();
        let edges: BTreeMap<DeclId, BTreeSet<DeclId>> = loops.iter().map(|(t, _, reads)| (*t, reads.iter().copied().filter(|r| targets.contains(r)).collect())).collect();
        let reach = |from: DeclId| {
            let mut seen = BTreeSet::new();
            let mut stack: Vec<DeclId> = edges[&from].iter().copied().collect();
            while let Some(next) = stack.pop() {
                if seen.insert(next) {
                    stack.extend(edges[&next].iter().copied());
                }
            }
            seen
        };
        let mut reported = BTreeSet::new();
        for (target, span, _) in &loops {
            if reported.contains(target) {
                continue;
            }
            let from = reach(*target);
            if !from.contains(target) {
                continue;
            }
            let cycle: Vec<DeclId> = targets.iter().copied().filter(|t| from.contains(t) && reach(*t).contains(target)).collect();
            reported.extend(cycle.iter().copied());
            // a loop of resolved drives is left to relaxation (§8.2.3): only one through a one-driver carrier cannot settle
            if !cycle.iter().any(|carrier| self.is_one_driver_carrier(*carrier)) {
                continue;
            }
            let names: Vec<String> = cycle.iter().map(|d| format!("`{}`", self.state.design.decl_data(*d).name.as_ref().map_or("", |n| n.text.as_str()))).collect();
            let file = self.state.design.decl_data(*target).file;
            let message = format!("undelayed definitions of {} depend on each other: a loop through one-driver carriers (§8.2.3)", names.join(", "));
            self.state.diagnostics.report_distinct(file, Diagnostic::new("E0339", message, *span, "a loop of definitions"));
        }
    }
}
