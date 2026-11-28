//! Generation in a module body (generics GN-17, part III §8.2.7, §8.2.23, §8.2.24): `for` and `if` over module items, run
//! at elaboration. [`GeneratedItems`] is what one body's generation unrolls into: for each item written inside a generation,
//! the iterations that elaborate it, each with the constant its loop variables stand for and the ordinal it has in every
//! `for` around it. A `for` over an empty range elaborates its items never (an empty array); an `if` elaborates the items of
//! the arm its condition selects and none of the others (a reference to one is `E0342`).
//!
//! Every item a generation holds is lowered: a placement is one child per iteration (`name[k]`), a node or a carrier an array
//! of one element per iteration, a definition one drive per iteration, a hoisted placement one frame per iteration, and a
//! `let` read outside its loops the array of its values. A name read inside its loops is the iteration's element. An item
//! of an `if` that only some iterations of a `for` elaborate would make an array with gaps: refused by name (`P0001`).

use super::StructureLowering;
use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{Constant, Expr, ExprId as LoweredExpr, Origin, ValueType};

use crate::elab::consts::Bindings;
use crate::elab::elaborator::Scope;
use crate::refusal::forms::StructureForm;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, GenElseNode, GenerationData, PatKind};

/// The most iterations one generation `for` unrolls into.
const TRIPS: i128 = 1 << 24;

/// One elaboration of an item written in generation: the loop variables bound to their constants and the ordinal of the
/// iteration in each `for` around the item, outermost first.
#[derive(Debug, Clone, Default)]
pub(in crate::elab) struct Iteration {
    /// The binding of each loop variable and its constant.
    pub(in crate::elab) values: Vec<(DeclId, i128)>,
    /// The generation `for` and the iteration's ordinal in it.
    pub(in crate::elab) ordinals: Vec<(DeclId, u32)>,
}

impl Iteration {
    /// The subscripts that name the item's element in its array: `[1]`, or `[0][2]` for nested loops; empty for an `if`.
    pub(in crate::elab) fn subscripts(&self) -> String {
        self.ordinals.iter().map(|(_, ordinal)| format!("[{ordinal}]")).collect()
    }
}

/// The items of the body being lowered that are written inside generation, and how each unrolls.
#[derive(Debug, Default)]
pub(in crate::elab) struct GeneratedItems {
    /// The iterations of each item written in generation; none for an item of an arm that was not elaborated.
    iterations: BTreeMap<DeclId, Vec<Iteration>>,
    /// The items refused for a gap in their array (`generated_items_complete`), so each is reported once and is the one
    /// diagnostic of its carrier.
    pub(in crate::elab) refused: BTreeSet<DeclId>,
    /// Every item met outside a generation or inside one, with its iteration, in the order the generation unrolls them: source
    /// order, a `for` iteration by iteration (§12.1.2).
    sequence: Vec<(DeclId, Iteration)>,
    /// The frame whose module's generation this is: the frame its array lengths and conditions fold over.
    pub(in crate::elab) frame: usize,
}

impl GeneratedItems {
    /// The iterations of `item`; `None` when it is written outside generation.
    pub(in crate::elab) fn of(&self, item: DeclId) -> Option<&Vec<Iteration>> {
        self.iterations.get(&item)
    }

    /// Every item of the body met in unrolling order, with the iteration it is elaborated in: an item outside generation
    /// once, with no loop variable; an item inside generation once for each iteration that elaborates it.
    pub(in crate::elab) fn unrolled(&self) -> &[(DeclId, Iteration)] {
        &self.sequence
    }

    /// The items elaborated at least once, in declaration order.
    pub(in crate::elab) fn elaborated(&self) -> impl Iterator<Item = DeclId> + '_ {
        self.iterations.iter().filter(|(_, iterations)| !iterations.is_empty()).map(|(item, _)| *item)
    }
}

/// The ordinals an iteration has in the generation `for`s around one item, outermost first.
struct GeneratedLevels(Vec<u32>);

impl GeneratedLevels {
    /// The ordinals of `iteration` in `loops`, outermost first.
    fn new(loops: &[DeclId], iteration: &Iteration) -> GeneratedLevels {
        GeneratedLevels(loops.iter().filter_map(|generation| iteration.ordinals.iter().find(|(held, _)| held == generation).map(|(_, ordinal)| *ordinal)).collect())
    }

    /// Whether the outermost ordinals are `fixed`.
    fn starts_with(&self, fixed: &[u32]) -> bool {
        self.0.starts_with(fixed)
    }

    /// The ordinal at `level`.
    fn at(&self, level: usize) -> Option<u32> {
        self.0.get(level).copied()
    }
}

/// The `for` of a generation that is being unrolled: what `expand_range` needs of it besides the iteration it sits in.
#[derive(Clone, Copy)]
struct GenerationRange<'a> {
    /// The generation `for` item.
    generation: DeclId,
    /// The items each iteration elaborates.
    body: &'a [DeclId],
    /// The first value of the range.
    start: crate::pom::ExprId,
    /// The end of the range.
    end: crate::pom::ExprId,
    /// Whether the range includes its `end`.
    includes_end: bool,
    /// The loop variable, when the `for` binds one.
    variable: Option<DeclId>,
}

/// A `let` of the body that is written in generation, read at one place: where `generated_let_level` finds what the
/// iterations that reach it gave the `let`.
#[derive(Clone, Copy)]
struct GeneratedLet<'a> {
    /// The expression that reads the `let`.
    expression: ExprId,
    /// The value the `let` is bound to.
    value: ExprId,
    /// The generation `for`s around the `let`, outermost first.
    loops: &'a [DeclId],
    /// Where the read is written.
    origin: &'a Origin,
}

impl StructureLowering<'_, '_, '_> {
    /// Unrolls the generation of `module`'s body, over the key params of the body being lowered.
    pub(super) fn expand_generation(&mut self, module: DeclId) {
        self.expand_generation_of(module, 0);
    }

    /// Unrolls the generation of `module`, the module of the frame `frame`, over the keys of the frame being lowered.
    pub(super) fn expand_generation_of(&mut self, module: DeclId, frame: usize) {
        self.state.structure.generated = GeneratedItems { frame, ..GeneratedItems::default() };
        let children = self.state.design.decl_data(module).children.clone();
        self.expand_items(&children, None);
    }

    /// The items `items` of a body or an arm, elaborated under `now` (`None` outside every `for`).
    fn expand_items(&mut self, items: &[DeclId], now: Option<&Iteration>) {
        for item in items {
            match &self.state.design.decl_data(*item).kind {
                DeclKind::Generation(_) => self.expand_generation_item(*item, now),
                DeclKind::Binding(_) => {}
                _ => {
                    self.state.structure.generated.sequence.push((*item, now.cloned().unwrap_or_default()));
                    if let Some(now) = now {
                        self.state.structure.generated.iterations.entry(*item).or_default().push(now.clone());
                    }
                }
            }
        }
    }

    /// The reactions of a `digital` block whose items are `items`, each with the iteration it is made for, in the order the
    /// generation unrolls them: a `for` around reactions makes one per iteration, iteration by iteration, inside the place of
    /// the `for` in source order (GN-19, §12.1.2).
    pub(in crate::elab) fn digital_sequence(&mut self, items: &[DeclId]) -> Vec<(DeclId, Iteration)> {
        let outer = std::mem::take(&mut self.state.structure.generated.sequence);
        self.expand_items(items, None);
        std::mem::replace(&mut self.state.structure.generated.sequence, outer)
    }

    /// The generation `generation`, written under the iteration `now`.
    fn expand_generation_item(&mut self, generation: DeclId, now: Option<&Iteration>) {
        let data = self.state.design.decl_data(generation).clone();
        let DeclKind::Generation(kind) = &data.kind else { return };
        let scope_values = self.iteration_bindings(now);
        let inherited = now.cloned().unwrap_or_default();
        match kind {
            GenerationData::For { pat, iter, body } => {
                let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = self.state.design.expr_node(*iter).kind else {
                    self.elaboration.refuse(data.file, self.state.design.expr_node(*iter).span, StructureForm::GenerationForOverAnythingButRangeWith);
                    self.mark_untaken(body);
                    return;
                };
                let PatKind::Ident { binding, sub: None, .. } = self.state.design.pat_node(*pat).kind else {
                    if !matches!(self.state.design.pat_node(*pat).kind, PatKind::Wild) {
                        self.elaboration.refuse(data.file, self.state.design.pat_node(*pat).span, StructureForm::GenerationForWhoseVariableIsNotName);
                    }
                    return self.expand_range(&GenerationRange { generation, body, start: lo, end: hi, includes_end: inclusive, variable: None }, now, &scope_values);
                };
                self.expand_range(&GenerationRange { generation, body, start: lo, end: hi, includes_end: inclusive, variable: Some(binding) }, now, &scope_values);
            }
            GenerationData::If { cond, then, else_ } => match self.fold_generation(*cond, &scope_values, "a generation condition") {
                Some(Constant::Bool(true)) => {
                    self.expand_items(then, Some(&inherited));
                    self.mark_untaken_else(else_);
                }
                Some(Constant::Bool(false)) => {
                    self.mark_untaken(then);
                    match else_ {
                        Some(GenElseNode::Block(items)) => self.expand_items(items, Some(&inherited)),
                        Some(GenElseNode::If(next)) => self.expand_generation_item(*next, now),
                        None => {}
                    }
                }
                Some(_) => {
                    self.elaboration.refuse(data.file, self.state.design.expr_node(*cond).span, StructureForm::GenerationConditionThatIsNotBool);
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
    fn expand_range(&mut self, range: &GenerationRange<'_>, now: Option<&Iteration>, values: &Bindings) {
        let GenerationRange { generation, body, start: lo, end: hi, includes_end: inclusive, variable } = *range;
        let (Some(Constant::Int(first)), Some(Constant::Int(last))) = (self.fold_generation(lo, values, "a generation range"), self.fold_generation(hi, values, "a generation range")) else {
            self.mark_untaken(body);
            return;
        };
        let end = last + i128::from(inclusive);
        if end - first > TRIPS {
            let data = self.state.design.decl_data(generation);
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
        match self.state.consts.value(expr, values) {
            Ok(value) => Some(value),
            Err(fault) => {
                self.const_fault(&fault, what);
                None
            }
        }
    }

    /// The keys of the body overlaid with the loop variables of `now`.
    pub(in crate::elab) fn iteration_bindings(&self, now: Option<&Iteration>) -> Bindings {
        let mut bindings = self.state.structure.body_keys.clone();
        for (variable, value) in now.map(|n| n.values.as_slice()).unwrap_or(&[]) {
            bindings.bind(*variable, Constant::Int(*value));
        }
        bindings
    }

    /// The items of an arm that was not elaborated: each is known, and has no iteration.
    fn mark_untaken(&mut self, items: &[DeclId]) {
        for item in items {
            match self.state.design.decl_data(*item).kind.clone() {
                DeclKind::Generation(GenerationData::For { body, .. }) => self.mark_untaken(&body),
                DeclKind::Generation(GenerationData::If { then, else_, .. }) => {
                    self.mark_untaken(&then);
                    self.mark_untaken_else(&else_);
                }
                DeclKind::Binding(_) => {}
                _ => {
                    self.state.structure.generated.iterations.entry(*item).or_default();
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

    /// Whether the item `decl` written in generation has an element for every iteration of the `for`s around it: an arm
    /// some iterations skip would leave a gap in its array, which this elaboration refuses, once, by name (`P0001`). An item
    /// outside generation, or in an `if` outside every `for`, has no gap.
    pub(in crate::elab) fn generated_items_complete(&mut self, decl: DeclId) -> bool {
        let Some(elaborated) = self.state.structure.generated.of(decl).map(Vec::len) else { return true };
        let loops = self.generation_loops(decl).len();
        let typed = self.state.consts.typed_carrier(decl).or_else(|| self.state.consts.typed_node_array(decl));
        let frame = self.state.structure.generated.frame;
        let Some(shape) = typed.and_then(|ty| self.digital().shape_in(&ty, frame)) else { return true };
        let trips: u64 = shape.dims.iter().take(loops).product();
        if loops == 0 || elaborated as u64 == trips {
            return true;
        }
        if self.state.structure.generated.refused.insert(decl) {
            let data = self.state.design.decl_data(decl).clone();
            self.refuse(data.file, data.span, StructureForm::ItemDeclaredInGenerationIfThatNot);
        }
        false
    }

    /// A read at `expr` of the `let` binding `binding` written in generation: inside the loops that declare it, the value of
    /// the iteration being lowered; outside some of them, the array of the values of their iterations, nested once for each,
    /// of the type `ty` the read has (§8.2.23). `E0342` for a `let` of an arm that was not elaborated. `None` after a
    /// diagnostic.
    pub(in crate::elab) fn lower_generated_let(&mut self, scope: &mut Scope, expr: ExprId, binding: DeclId, ty: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let item = self.state.design.decl_data(binding).parent?;
        let value = self.body_let_value(binding)?;
        let iterations = self.state.structure.generated.of(item)?.clone();
        if iterations.is_empty() && self.generation_loops(item).is_empty() {
            self.refer_outside_generation(scope.values.file, self.state.design.expr_node(expr).span, binding, None);
            return None;
        }
        let loops = self.generation_loops(item);
        let fixed: Vec<u32> = self.implicit_ordinals(item, &scope.digital.ordinals).into_iter().map(|ordinal| ordinal as u32).collect();
        let reached: Vec<Iteration> = iterations.into_iter().filter(|iteration| GeneratedLevels::new(&loops, iteration).starts_with(&fixed)).collect();
        self.generated_let_level(scope, &GeneratedLet { expression: expr, value, loops: &loops, origin: &origin }, fixed.len(), &reached, ty)
    }

    /// The values of the `let` `value` over the iterations `reached`, from the level `level` of the generation `for`s
    /// `loops` on: one value at the innermost level, else the array of the values below each ordinal of this level.
    fn generated_let_level(&mut self, scope: &mut Scope, site: &GeneratedLet<'_>, level: usize, reached: &[Iteration], ty: ValueType) -> Option<LoweredExpr> {
        let GeneratedLet { expression: expr, value, loops, origin } = *site;
        if level == loops.len() {
            let iteration = reached.first()?;
            let outer = (scope.values.unrolled.clone(), std::mem::replace(&mut scope.digital.ordinals, iteration.ordinals.clone()));
            for (variable, constant) in &iteration.values {
                scope.values.unrolled.bind(*variable, Constant::Int(*constant));
            }
            let lowered = self.lower_body_let(scope, value);
            (scope.values.unrolled, scope.digital.ordinals) = outer;
            return lowered;
        }
        let ValueType::Array(element, length) = &ty else {
            self.elaboration.refuse(scope.values.file, self.state.design.expr_node(expr).span, StructureForm::LetWrittenInGenerationReadAsNo);
            return None;
        };
        let ordinals: BTreeSet<u32> = reached.iter().filter_map(|iteration| GeneratedLevels::new(loops, iteration).at(level)).collect();
        // an arm some iterations skip leaves a gap: the array the type names has an element the generation never made
        if ordinals.len() as u64 != *length || ordinals.iter().enumerate().any(|(position, ordinal)| position as u32 != *ordinal) {
            self.elaboration.refuse(scope.values.file, self.state.design.expr_node(expr).span, StructureForm::LetDeclaredInGenerationIfThatNot);
            return None;
        }
        let mut parts = Vec::with_capacity(ordinals.len());
        for ordinal in ordinals {
            let below: Vec<Iteration> = reached.iter().filter(|iteration| GeneratedLevels::new(loops, iteration).at(level) == Some(ordinal)).cloned().collect();
            parts.push(self.generated_let_level(scope, site, level + 1, &below, (**element).clone())?);
        }
        Some(self.emit(Expr::Array(parts), ty, origin.clone()))
    }

    /// The position, among the iterations that elaborate the item `item`, of the one whose ordinals in the generation `for`s
    /// around it are `ordinals`, outermost first: the instance of a generated placement it is. `None` for one the
    /// generation never made.
    pub(super) fn iteration_position(&self, item: DeclId, ordinals: &[u64]) -> Option<usize> {
        let loops = self.generation_loops(item);
        let wanted: Vec<u32> = ordinals.iter().map(|ordinal| u32::try_from(*ordinal).unwrap_or(u32::MAX)).collect();
        self.state.structure.generated.of(item)?.iter().position(|iteration| GeneratedLevels::new(&loops, iteration).0 == wanted)
    }

    /// `E0342` for a reference to `decl`, an item written in generation: whose arm was not elaborated, or whose array has no
    /// element at `index`.
    pub(in crate::elab) fn refer_outside_generation(&mut self, file: crate::pom::FileId, span: crate::msl::Span, decl: DeclId, index: Option<(u64, u64)>) {
        let name = self.state.design.decl_data(decl).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
        let message = match index {
            Some((at, length)) => format!("the element {at} of the generated array `{name}` does not exist: it has {length} (§8.2.23)"),
            None => format!("`{name}` is declared in an arm of a generation that was not elaborated (§8.2.24)"),
        };
        self.report(file, crate::msl::Diagnostic::new("E0342", message, span, "no such generated item"));
    }
}
