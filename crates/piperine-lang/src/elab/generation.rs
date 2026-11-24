//! Generation in a module body (generics GN-17, part III §8.2.7, §8.2.23, §8.2.24): `for` and `if` over module items, run
//! at elaboration. [`GeneratedItems`] is what one body's generation unrolls into: for each item written inside a generation,
//! the iterations that elaborate it, each with the constant its loop variables stand for and the ordinal it has in every
//! `for` around it. A `for` over an empty range elaborates its items never (an empty array); an `if` elaborates the items of
//! the arm its condition selects and none of the others (a reference to one is `E0342`).
//!
//! The items a generation holds that this elaboration lowers are placements, `let`s over constants and nodes (an array of
//! nodes, one element per iteration); a carrier, a definition or a hoisted placement inside generation is refused loudly.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::Constant;

use super::consts::Bindings;
use super::elaborator::Elaboration;
use crate::pom::{DeclId, DeclKind, ExprKind, GenElseNode, GenerationData, PatKind};

/// The most iterations one generation `for` unrolls into.
const TRIPS: i128 = 1 << 24;

/// One elaboration of an item written in generation: the loop variables bound to their constants and the ordinal of the
/// iteration in each `for` around the item, outermost first.
#[derive(Debug, Clone, Default)]
pub(super) struct Iteration {
    /// The binding of each loop variable and its constant.
    pub(super) values: Vec<(DeclId, i128)>,
    /// The generation `for` and the iteration's ordinal in it.
    pub(super) ordinals: Vec<(DeclId, u32)>,
}

impl Iteration {
    /// The subscripts that name the item's element in its array: `[1]`, or `[0][2]` for nested loops; empty for an `if`.
    pub(super) fn subscripts(&self) -> String {
        self.ordinals.iter().map(|(_, ordinal)| format!("[{ordinal}]")).collect()
    }
}

/// The items of the body being lowered that are written inside generation, and how each unrolls.
#[derive(Debug, Default)]
pub(super) struct GeneratedItems {
    /// The iterations of each item written in generation; none for an item of an arm that was not elaborated.
    iterations: BTreeMap<DeclId, Vec<Iteration>>,
    /// The items already refused, so each is reported once.
    refused: BTreeSet<DeclId>,
}

impl GeneratedItems {
    /// The iterations of `item`; `None` when it is written outside generation.
    pub(super) fn of(&self, item: DeclId) -> Option<&Vec<Iteration>> {
        self.iterations.get(&item)
    }

    /// The items elaborated at least once, in declaration order.
    pub(super) fn elaborated(&self) -> impl Iterator<Item = DeclId> + '_ {
        self.iterations.iter().filter(|(_, iterations)| !iterations.is_empty()).map(|(item, _)| *item)
    }
}

impl Elaboration<'_, '_> {
    /// Unrolls the generation of `module`'s body, over the key params of the body being lowered.
    pub(super) fn expand_generation(&mut self, module: DeclId) {
        self.generated = GeneratedItems::default();
        let children = self.design.decl_data(module).children.clone();
        self.expand_items(&children, None);
    }

    /// The items `items` of a body or an arm, elaborated under `now` (`None` outside every `for`).
    fn expand_items(&mut self, items: &[DeclId], now: Option<&Iteration>) {
        for item in items {
            match &self.design.decl_data(*item).kind {
                DeclKind::Generation(_) => self.expand_generation_item(*item, now),
                DeclKind::Binding(_) => {}
                _ => {
                    if let Some(now) = now {
                        self.admit_generated(*item);
                        self.generated.iterations.entry(*item).or_default().push(now.clone());
                    }
                }
            }
        }
    }

    /// The generation `generation`, written under the iteration `now`.
    fn expand_generation_item(&mut self, generation: DeclId, now: Option<&Iteration>) {
        let data = self.design.decl_data(generation).clone();
        let DeclKind::Generation(kind) = &data.kind else { return };
        let scope_values = self.iteration_bindings(now);
        match kind {
            GenerationData::For { pat, iter, body } => {
                let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = self.design.expr_node(*iter).kind else {
                    self.unsupported(data.file, self.design.expr_node(*iter).span, "a generation `for` over anything but a range with both bounds");
                    self.mark_untaken(body);
                    return;
                };
                let PatKind::Ident { binding, sub: None, .. } = self.design.pat_node(*pat).kind else {
                    if !matches!(self.design.pat_node(*pat).kind, PatKind::Wild) {
                        self.unsupported(data.file, self.design.pat_node(*pat).span, "a generation `for` whose variable is not a name");
                    }
                    return self.expand_range(generation, body, lo, hi, inclusive, None, now, &scope_values);
                };
                self.expand_range(generation, body, lo, hi, inclusive, Some(binding), now, &scope_values);
            }
            GenerationData::If { cond, then, else_ } => match self.fold_generation(*cond, &scope_values, "a generation condition") {
                Some(Constant::Bool(true)) => {
                    self.expand_items(then, now);
                    self.mark_untaken_else(else_);
                }
                Some(Constant::Bool(false)) => {
                    self.mark_untaken(then);
                    match else_ {
                        Some(GenElseNode::Block(items)) => self.expand_items(items, now),
                        Some(GenElseNode::If(next)) => self.expand_generation_item(*next, now),
                        None => {}
                    }
                }
                Some(_) => {
                    self.unsupported(data.file, self.design.expr_node(*cond).span, "a generation condition that is not a `bool`");
                    self.mark_untaken(then);
                    self.mark_untaken_else(else_);
                }
                None => {
                    self.mark_untaken(then);
                    self.mark_untaken_else(else_);
                }
            },
        }
    }

    /// The iterations of the `for` `generation` over `lo..hi`, each elaborating `body`.
    #[allow(clippy::too_many_arguments)]
    fn expand_range(&mut self, generation: DeclId, body: &[DeclId], lo: crate::pom::ExprId, hi: crate::pom::ExprId, inclusive: bool, variable: Option<DeclId>, now: Option<&Iteration>, values: &Bindings) {
        let (Some(Constant::Int(first)), Some(Constant::Int(last))) = (self.fold_generation(lo, values, "a generation range"), self.fold_generation(hi, values, "a generation range")) else {
            self.mark_untaken(body);
            return;
        };
        let end = last + i128::from(inclusive);
        if end - first > TRIPS {
            let data = self.design.decl_data(generation);
            let message = format!("a generation `for` of {} iterations: at most {TRIPS} are unrolled", end - first);
            self.report(data.file, crate::msl::Diagnostic::new("E0416", message, data.span, "too many iterations"));
            self.mark_untaken(body);
            return;
        }
        if end <= first {
            self.mark_untaken(body);
            return;
        }
        for (ordinal, value) in (first..end).enumerate() {
            let mut iteration = now.cloned().unwrap_or_default();
            if let Some(variable) = variable {
                iteration.values.push((variable, value));
            }
            iteration.ordinals.push((generation, ordinal as u32));
            self.expand_items(body, Some(&iteration));
        }
    }

    /// The constant of `expr` over `values`, or `None` after a diagnostic.
    fn fold_generation(&mut self, expr: crate::pom::ExprId, values: &Bindings, what: &str) -> Option<Constant> {
        match self.consts.value(expr, values) {
            Ok(value) => Some(value),
            Err(fault) => {
                self.const_fault(&fault, what);
                None
            }
        }
    }

    /// The keys of the body overlaid with the loop variables of `now`.
    pub(super) fn iteration_bindings(&self, now: Option<&Iteration>) -> Bindings {
        let mut bindings = self.body_keys.clone();
        for (variable, value) in now.map(|n| n.values.as_slice()).unwrap_or(&[]) {
            bindings.bind(*variable, Constant::Int(*value));
        }
        bindings
    }

    /// The items of an arm that was not elaborated: each is known, and has no iteration.
    fn mark_untaken(&mut self, items: &[DeclId]) {
        for item in items {
            match self.design.decl_data(*item).kind.clone() {
                DeclKind::Generation(GenerationData::For { body, .. }) => self.mark_untaken(&body),
                DeclKind::Generation(GenerationData::If { then, else_, .. }) => {
                    self.mark_untaken(&then);
                    self.mark_untaken_else(&else_);
                }
                DeclKind::Binding(_) => {}
                _ => {
                    self.generated.iterations.entry(*item).or_default();
                }
            }
        }
    }

    /// The items of the `else` of a generation `if` that was not elaborated.
    fn mark_untaken_else(&mut self, else_: &Option<GenElseNode>) {
        match else_ {
            Some(GenElseNode::Block(items)) => self.mark_untaken(items),
            Some(GenElseNode::If(next)) => self.mark_untaken(&[*next]),
            None => {}
        }
    }

    /// Refuses, once, an item written in generation that this elaboration does not lower: it would be dropped silently.
    fn admit_generated(&mut self, item: DeclId) {
        let data = self.design.decl_data(item).clone();
        let what = match &data.kind {
            DeclKind::Signal(_) if self.types.carrier(item).is_some() && self.types.node_array(item).is_none() => Some("a carrier"),
            DeclKind::Definition(_) => Some("a definition"),
            DeclKind::Let(l) if self.types.callee(l.value).is_some() && self.hoisted_callee(l.value).is_some() => Some("a placement of a module with a value input or a result"),
            _ => None,
        };
        if let Some(what) = what
            && self.generated.refused.insert(item)
        {
            self.unsupported(data.file, data.span, &format!("{what} inside generation"));
        }
    }

    /// The generation `for`s around `decl`, outermost first.
    pub(super) fn generation_loops(&self, decl: DeclId) -> Vec<DeclId> {
        let mut loops = Vec::new();
        let mut at = self.design.decl_data(decl).parent;
        while let Some(current) = at {
            let data = self.design.decl_data(current);
            if matches!(data.kind, DeclKind::Generation(GenerationData::For { .. })) {
                loops.push(current);
            }
            if matches!(data.kind, DeclKind::Module(_)) {
                break;
            }
            at = data.parent;
        }
        loops.reverse();
        loops
    }

    /// The ordinals of the iteration `ordinals` the code being lowered sits in, for the generation `for`s around `decl` it is
    /// inside (a prefix of them, outermost first).
    pub(super) fn implicit_ordinals(&self, decl: DeclId, ordinals: &[(DeclId, u32)]) -> Vec<u64> {
        self.generation_loops(decl).iter().map_while(|generation| ordinals.iter().find(|(held, _)| held == generation).map(|(_, ordinal)| u64::from(*ordinal))).collect()
    }

    /// `E0342` for a reference to `decl`, an item written in generation: whose arm was not elaborated, or whose array has no
    /// element at `index`.
    pub(super) fn refer_outside_generation(&mut self, file: crate::pom::FileId, span: crate::msl::Span, decl: DeclId, index: Option<(u64, u64)>) {
        let name = self.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = match index {
            Some((at, length)) => format!("the element {at} of the generated array `{name}` does not exist: it has {length} (§8.2.23)"),
            None => format!("`{name}` is declared in an arm of a generation that was not elaborated (§8.2.24)"),
        };
        self.report(file, crate::msl::Diagnostic::new("E0342", message, span, "no such generated item"));
    }
}
