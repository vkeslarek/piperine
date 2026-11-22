//! Definitions (DIG-04, part III §8.2): `place = expr [after d];` makes this module's drive of a
//! discrete carrier a function of what the expression reads. The target is one the module may
//! drive (`E0222`), the expression has the carrier's value type (`E0337`) and is discrete
//! (`E0336`), a delay is an `f64` (`E0433`), a carrier is defined once (`E0335`), an undelayed
//! definition leaves an initial value no room (`E0338`) and a loop of undelayed definitions is
//! `E0339`. The same targets, under the same rules, are those of a commit (`reaction.rs`).

use std::collections::{BTreeMap, BTreeSet};

use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, DefinitionData, ExprId, ExprKind, Resolution};

use super::check::Checker;
use super::Ty;

/// What one module drives, so far: by carrier, where.
#[derive(Debug, Default)]
pub(super) struct DriveBook {
    /// The carriers a definition drives.
    pub(super) defined: BTreeMap<DeclId, Span>,
    /// The carriers a commit drives.
    pub(super) committed: BTreeMap<DeclId, Span>,
    /// The carriers handed to an `out` port of an instance, in source order, with where and in
    /// which file.
    pub(super) handed: Vec<(DeclId, Span, crate::pom::FileId)>,
    /// The undelayed definitions in source order: the target, where, and the carriers its value
    /// reads (directly or through a `let`).
    loops: Vec<(DeclId, Span, BTreeSet<DeclId>)>,
    /// The carriers an undelayed definition drives: the definition gives the carrier its value from `(0, 0)`.
    pub(super) undelayed: BTreeSet<DeclId>,
}

impl Checker<'_> {
    /// The carrier `place` names and the value type it holds, when this module may drive it;
    /// else `E0222` naming `what` and `None`. A form of place the subset does not have (an
    /// element, a field) is `P0001`.
    pub(super) fn drive_target(&mut self, place: ExprId, what: &str) -> Option<(DeclId, Ty)> {
        let node = self.design.expr_node(place).clone();
        let file = self.file_of_expr(place);
        match &node.kind {
            ExprKind::Paren(inner) => self.drive_target(*inner, what),
            ExprKind::Index { .. } | ExprKind::Field { res: Resolution::Deferred(_) | Resolution::Unresolved, .. } => {
                self.unsupported(file, node.span, &format!("an element or field {what}"));
                None
            }
            ExprKind::Field { res: Resolution::Decl(item), .. } => {
                let text = match self.design.decl_data(*item).kind {
                    DeclKind::Port(_) | DeclKind::Signal(_) => "a carrier of an instance",
                    _ => "a member of an instance",
                };
                let message = format!("the target of a {what} is {text}: a parent never writes a child's drive (§8.2.10)");
                self.coded(file, "E0222", node.span, message, "a child's drive");
                None
            }
            ExprKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                let Resolution::Decl(decl) = res else {
                    if matches!(res, Resolution::Builtin(_)) {
                        self.coded(file, "E0222", node.span, format!("the target of a {what} is not a discrete carrier (§8.2.10)"), "not a carrier");
                    }
                    return None;
                };
                let (port_ty, direction) = match &self.design.decl_data(decl).kind {
                    DeclKind::Port(p) => (Some(p.ty), p.dir),
                    DeclKind::Signal(s) => (Some(s.ty), None),
                    DeclKind::Param(_) => return self.not_drivable(file, node.span, what, "a param"),
                    DeclKind::Binding(_) => return self.not_drivable(file, node.span, what, "a `let` name"),
                    _ => return self.not_drivable(file, node.span, what, "not a discrete carrier"),
                };
                let ty = port_ty.and_then(|t| self.port_type(t))?;
                match ty {
                    Ty::Carrier { value, .. } => {
                        if direction == Some(crate::msl::ast::Dir::In) {
                            return self.not_drivable(file, node.span, what, "an `in` port: a module never writes its own input (§5.2.5)");
                        }
                        self.exprs[place.index()] = Some((*value).clone());
                        Some((decl, *value))
                    }
                    _ => self.not_drivable(file, node.span, what, "a node, not a discrete carrier"),
                }
            }
            _ => {
                self.coded(file, "E0222", node.span, format!("the target of a {what} is not a place (§8.2.10)"), "not a place");
                self.infer(place);
                None
            }
        }
    }

    fn not_drivable(&mut self, file: crate::pom::FileId, span: Span, what: &str, why: &str) -> Option<(DeclId, Ty)> {
        let why = if why.starts_with("a ") || why.starts_with("an ") || why.starts_with("not ") { why.to_string() } else { format!("a {why}") };
        self.coded(file, "E0222", span, format!("the target of a {what} is {why}, which this module may not drive (§8.2.10)"), "this module may not drive it");
        None
    }

    /// `place = value [after delay];`
    pub(super) fn definition(&mut self, data: &DefinitionData) {
        let place_span = self.design.expr_node(data.place).span;
        let file = self.file_of_expr(data.place);
        let target = self.drive_target(data.place, "definition");
        match &target {
            Some((_, value)) => self.check_as(data.value, value, "E0337"),
            None => {
                self.infer(data.value);
            }
        }
        if let Some(after) = data.after {
            self.check_as(after, &Ty::F64, "E0433");
        }
        for (expr, what) in [(Some(data.value), "a definition"), (data.after, "the delay of a definition")] {
            let Some(expr) = expr else { continue };
            if self.continuous(expr) {
                let span = self.design.expr_node(expr).span;
                let message = format!("{what} reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`, directly or through a `let`, a function or a result: it must be a discrete expression (§8.2.5)");
                self.coded(file, "E0336", span, message, "a continuous expression");
            }
        }
        let Some((carrier, _)) = target else { return };
        if self.book.defined.insert(carrier, place_span).is_some() {
            let name = self.design.decl_data(carrier).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            self.coded(file, "E0335", place_span, format!("two definitions of `{name}` share a level (§8.2.20)"), "defined twice");
            return;
        }
        if data.after.is_none() {
            if self.has_initial(carrier) {
                let name = self.design.decl_data(carrier).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                self.coded(file, "E0338", place_span, format!("`{name}` has an initial value and an undelayed definition: the definition would replace it in the microstep that commits it (§8.2.21)"), "initial value under an undelayed definition");
            }
            let mut reads = BTreeSet::new();
            self.carrier_reads(data.value, &mut reads, &mut BTreeSet::new());
            self.book.loops.push((carrier, place_span, reads));
            self.book.undelayed.insert(carrier);
        }
    }

    /// A continuous argument for the value input `input` of `module`, which a definition of `module`
    /// reads, directly or through a `let` (§8.3.9): each read is checked as if the caller's expression
    /// were written there, and a definition is discrete (§8.2.5), so it is `E0336` at the call `call`
    /// naming the input. A discrete argument, or an input only an `analog` block reads, is accepted.
    pub(super) fn definition_value_input(&mut self, call: ExprId, module: DeclId, input: DeclId, arg: ExprId) {
        let read_by_a_definition = self.design.decl_data(module).children.clone().into_iter().any(|child| match &self.design.decl_data(child).kind {
            DeclKind::Definition(definition) => [Some(definition.value), definition.after].into_iter().flatten().any(|expr| self.reads_decl(expr, input, &mut Vec::new())),
            _ => false,
        });
        if !read_by_a_definition || !self.continuous(arg) {
            return;
        }
        let name = |decl: DeclId| self.design.decl_data(decl).name.as_ref().map_or(String::new(), |n| n.text.clone());
        let message = format!("the argument for the value input `{}` of `{}` is continuous (it reads a probe, `now()`, `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`) and a definition of `{}` reads it: a definition is a discrete expression (§8.3.9, §8.2.5)", name(input), name(module), name(module));
        let (file, span) = (self.file_of_expr(call), self.design.expr_node(call).span);
        self.coded(file, "E0336", span, message, "a continuous argument read by a definition");
    }

    /// Whether a port or a signal declares an initial value.
    pub(super) fn has_initial(&self, carrier: DeclId) -> bool {
        match &self.design.decl_data(carrier).kind {
            DeclKind::Port(p) => p.default.is_some(),
            DeclKind::Signal(s) => s.init.is_some(),
            _ => false,
        }
    }

    /// The discrete carriers `expr` reads, directly or through a `let`.
    pub(super) fn carrier_reads(&self, expr: ExprId, out: &mut BTreeSet<DeclId>, seen: &mut BTreeSet<ExprId>) {
        if !seen.insert(expr) {
            return;
        }
        let node = self.design.expr_node(expr);
        if let ExprKind::Path(path) = &node.kind
            && let Some(Resolution::Decl(d)) = self.design.path_node(*path).segments.last().map(|s| s.res)
        {
            if self.carriers.contains_key(&d) {
                out.insert(d);
            } else if let Some(init) = self.binding_init(d) {
                self.carrier_reads(init, out, seen);
            }
        }
        for child in crate::pom::expr_children(&node.kind, self.design) {
            self.carrier_reads(child, out, seen);
        }
    }

    /// `E0339` once per loop of undelayed definitions: a drive that depends on itself with no
    /// reaction and no `after` in between (§8.2.3).
    pub(super) fn definition_loops(&mut self) {
        let loops = std::mem::take(&mut self.book.loops);
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
            let names: Vec<String> = cycle.iter().map(|d| format!("`{}`", self.design.decl_data(*d).name.as_ref().map_or("", |n| n.text.as_str()))).collect();
            let file = self.design.decl_data(*target).file;
            let message = format!("undelayed definitions of {} depend on each other: a loop through one-driver carriers (§8.2.3)", names.join(", "));
            self.report_distinct(file, Diagnostic::new("E0339", message, *span, "a loop of definitions"));
        }
    }
}
