//! The params a placement binds in its module (§8.1.16, edge case 5): the value the placement's param block (or its template) gives,
//! else the default over the params before it. One fold serves the two frames that read a placed module's params outside its own
//! body: a hoisted frame (`hoist.rs`), whose params must all fold, and a reach frame (`reach/fields.rs`), which reads a child's `let`s in
//! the parent's context (§8.2.25).
//!
//! The fold reports nothing itself: it returns what it folded, what it could not (`DeferredParam`), and what is wrong with the
//! placement (`ParamFault`). A hoisted frame reports the faults and is refused on any. A reach frame stands for a child whose own
//! placement already reported them, so it reports none, and a param it could not fold (a value over a param of the reader that is
//! not a key, `Cell(a) { k: m * 2.0 }`) is read where a `let` of the child reads it, lowered in its place (`lower_deferred_param`).

use std::collections::BTreeMap;

use piperine_ir::{Constant, ExprId as LoweredExpr};

use crate::elab::consts::{Bindings, ConstFault};
use crate::elab::elaborator::Elaboration;
use crate::elab::lower::Scope;
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, ExprId, FieldInit, FileId, Resolution};
use crate::types::Substitution;

/// A param of a placed module whose value does not fold to a constant: the expression it is read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::elab) enum DeferredParam {
    /// The value the placement gives, written in the caller: lowered in the frame of the caller.
    Given(ExprId),
    /// The default of the param, over a param before it that did not fold: lowered in the frame of the placed module.
    Default(ExprId),
}

/// What is wrong with the params a placement binds, in the order the fold met it.
#[derive(Debug, Clone)]
pub(in crate::elab) enum ParamFault {
    /// A given value or a default that does not fold to a constant, and what was being folded.
    NotFolded(ConstFault, &'static str),
    /// A required param the placement leaves out, by name (`E0319`).
    Required(String),
    /// A param whose `where` is false for the placement, by name (`E0317`).
    WhereFalse(String),
}

/// The params a placement binds in its module.
#[derive(Debug, Clone, Default)]
pub(in crate::elab) struct PlacementParams {
    /// The params that fold, each to its constant.
    pub(in crate::elab) values: BTreeMap<DeclId, Constant>,
    /// The params that do not fold, each to the expression it is read as.
    pub(in crate::elab) deferred: BTreeMap<DeclId, DeferredParam>,
    /// What is wrong with the placement.
    pub(in crate::elab) faults: Vec<ParamFault>,
}

/// Where a placement is written: the param block (or the template) it gives its module, and the constants and generic arguments of
/// the caller its given values fold over.
pub(in crate::elab) struct PlacementSite<'a> {
    /// The fields of the placement's param block.
    pub(in crate::elab) fields: &'a [FieldInit],
    /// The template the placement is written through, which fixes every param.
    pub(in crate::elab) template: Option<&'a crate::elab::templates::TemplateValue>,
    /// The caller's constants: its params that fold, its loop variables.
    pub(in crate::elab) caller: &'a Bindings,
    /// The caller's generic arguments.
    pub(in crate::elab) caller_substitution: &'a Substitution,
}

impl Elaboration<'_, '_> {
    /// The params a placement of `module` at `site` binds, reporting nothing (see the module documentation).
    pub(in crate::elab) fn fold_placement_params(&mut self, module: DeclId, site: &PlacementSite) -> PlacementParams {
        let params: Vec<DeclId> = self.state.design.decl_data(module).children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::Param(_))).collect();
        let mut bindings = Bindings::new();
        let mut folded = PlacementParams::default();
        let template_values = match site.template.map(|template| self.template_constants(template)).transpose() {
            Ok(values) => values.unwrap_or_default(),
            Err(fault) => {
                folded.faults.push(ParamFault::NotFolded(fault, "a param value of a template"));
                return folded;
            }
        };
        for param in &params {
            let data = self.state.design.decl_data(*param).clone();
            let DeclKind::Param(declared) = &data.kind else { continue };
            let fixed = template_values.iter().find(|(bound, _, _)| bound == param).map(|(_, value, _)| *value);
            let given = site.fields.iter().find(|field| field.res == Resolution::Decl(*param)).map(|field| field.value).filter(|_| site.template.is_none());
            let name = data.name.as_ref().map_or("", |n| n.text.as_str()).to_string();
            let value = match given.or(declared.default) {
                // a template folded the value already
                _ if let Some(fixed) = fixed => fixed,
                // a given value reads the caller's constants (its params, its loop variables); a default the params before it
                Some(expr) => {
                    let read = if given.is_some() { site.caller.clone() } else { bindings.clone() };
                    match self.param_constant(expr, &read, given.is_some().then_some(site.caller_substitution)) {
                        Ok(value) => value,
                        Err(fault) => {
                            let deferred = if given.is_some() { DeferredParam::Given(expr) } else { DeferredParam::Default(expr) };
                            folded.deferred.insert(*param, deferred);
                            folded.faults.push(ParamFault::NotFolded(fault, "a param value of a placed module"));
                            continue;
                        }
                    }
                }
                // an `Option` param left out is the absent case (§3.2.10), as for a placement that keeps its body
                None if let Some(absent) = self.absent_option(declared) => absent,
                None => {
                    folded.faults.push(ParamFault::Required(name));
                    continue;
                }
            };
            self.bind_param(&mut bindings, *param, value);
            folded.values.insert(*param, value);
        }
        for param in &params {
            let data = self.state.design.decl_data(*param).clone();
            let DeclKind::Param(declared) = &data.kind else { continue };
            let Some(condition) = declared.where_ else { continue };
            if let Ok(Constant::Bool(false)) = self.state.consts.value(condition, &bindings) {
                folded.faults.push(ParamFault::WhereFalse(data.name.as_ref().map_or("", |n| n.text.as_str()).to_string()));
            }
        }
        folded
    }

    /// The params of a placed module that must all fold (a hoisted frame's): each fault reported (`E0319`, `E0317`, the fold's own),
    /// at `span` of `file`. `None` after a diagnostic.
    pub(in crate::elab) fn placement_params(&mut self, module: DeclId, site: &PlacementSite, file: FileId, span: Span) -> Option<BTreeMap<DeclId, Constant>> {
        let folded = self.fold_placement_params(module, site);
        for fault in &folded.faults {
            match fault {
                ParamFault::NotFolded(fault, position) => self.const_fault(fault, position),
                ParamFault::Required(name) => self.report(file, Diagnostic::new("E0319", format!("the placement leaves the required param `{name}` unbound"), span, "required param not bound")),
                ParamFault::WhereFalse(name) => self.report(file, Diagnostic::new("E0317", format!("the `where` of the param `{name}` is false for this placement"), span, "`where` is false")),
            }
        }
        folded.faults.is_empty().then_some(folded.values)
    }

    /// The param `decl` of the frame `scope.frame` that its placement left to be read in place (`DeferredParam`): the given value
    /// lowered in the caller's frame, the default in the frame itself. `None` for a param the frame does not defer.
    pub(in crate::elab) fn lower_deferred_param(&mut self, scope: &mut Scope, decl: DeclId) -> Option<LoweredExpr> {
        match self.state.structure.frames.get(scope.values.frame)?.deferred_params.get(&decl).copied()? {
            DeferredParam::Given(expr) => self.in_caller(scope, |this, scope| this.lower_expr(scope, expr)),
            DeferredParam::Default(expr) => self.lower_expr(scope, expr),
        }
    }
}
