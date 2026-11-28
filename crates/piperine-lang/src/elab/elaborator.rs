//! The entry point and the pass order of one elaboration run: `Types::check` first, then the
//! passes over what it reached, each writing through `IrBuilder`.

use crate::elab::value::ValueLowering;
use std::collections::BTreeMap;

use piperine_ir::{BodyId, Env, Expr, ExprId as LoweredExpr, Ir, IrBuilder, Origin, Stmt, StmtId as LoweredStmt, ValueType};

use super::analog_state::{AnalogScope, AnalogSites};
use super::consts::{Bindings, ConstEval};
use super::digital_state::{DigitalScope, DigitalTables};
use super::function_state::FunctionTables;
use super::instances::ConcreteValue;
use super::loops::DriveLoops;
use super::value::expressions::ValueTypes;
use super::origin::Locator;
use super::signal_flow::SignalFlowCheck;
use super::structural::{BodyKey, BodyVariants, Overrides};
use super::structure_state::StructureTables;
use super::value::state::{ExpressionTables, ValueScope};
use crate::refusal::forms::ValueForm;
use crate::refusal::{Refusal, RefusedForm};
use crate::msl::{Diagnostic, Span};
use crate::pom::{DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, FileId};
use crate::types::{ConstTerm, GenericSignature, GenericValue, Length, Ty, Types};

/// The entry point: `Elaborator::new(&design).run("Top")`.
#[derive(Debug, Clone)]
pub struct Elaborator<'d> {
    design: &'d Design,
}

impl<'d> Elaborator<'d> {
    pub fn new(design: &'d Design) -> Self {
        Elaborator { design }
    }

    /// Elaborate `top`, a module of the root crate.
    ///
    /// The front-end and type-check diagnostics come back unchanged; an unknown top is one
    /// diagnostic naming it, and so is a top with a value input (`E0811`). A discrete port of the
    /// top is a net the host drives (§5.2.3), and a node port is driven by the host source the IR
    /// gains for it (§8.1.14, `host_sources.rs`).
    pub fn run(self, top: &str) -> Result<Ir, Vec<DesignDiagnostic>> {
        self.run_with(top, &Overrides::none())
    }

    /// [`run`](Self::run) with the params a host bound before the run: a binding of a key param
    /// selects the variant of its value at that instance (`Overrides`); any other is ignored.
    pub fn run_with(self, top: &str, overrides: &Overrides) -> Result<Ir, Vec<DesignDiagnostic>> {
        self.run_with_statistics(top, overrides).0
    }

    /// [`run_with`](Self::run_with), and what the run cost ([`RunStatistics`]).
    pub fn run_with_statistics(self, top: &str, overrides: &Overrides) -> (Result<Ir, Vec<DesignDiagnostic>>, RunStatistics) {
        let mut statistics = RunStatistics::default();
        let types = match Types::check(self.design, top) {
            Ok(types) => types,
            Err(diagnostics) => return (Err(diagnostics), statistics),
        };
        let top_decl = types.reached()[0];
        // the reads the text names through children of children are the demands of the first run (`reach/seed.rs`); a run that meets
        // one its tables lack (a hop through a type-parameter template) says so, and is made again with it
        // (`reach/demands.rs`)
        let mut demands = Elaboration::new(self.design, &types, overrides.clone(), Vec::new()).static_demands();
        loop {
            let (result, wanted) = Elaboration::new(self.design, &types, overrides.clone(), demands.clone()).run(top_decl);
            statistics.passes += 1;
            if wanted.is_empty() {
                return (result, statistics);
            }
            demands.extend(wanted);
        }
    }
}

/// What one run of the [`Elaborator`] cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunStatistics {
    /// How many times the design was elaborated: one, plus one for each run that met a read through a child of a child its tables
    /// lacked (a hop through a type-parameter template, `reach/demands.rs`); zero when the type check refused the design.
    pub passes: usize,
}

/// One run: its state and the passes over it. Its passes are `impl` blocks in the files named after them.
pub(super) struct Elaboration<'d, 't> {
    /// The inputs of the run, the IR being built, and one table per concern.
    pub(super) state: ElaborationState<'d, 't>,
}

/// What names the code being lowered: the owner's params and locals (its nodes are those of its
/// frame, `Frame::nodes`), one part per concern.
pub(in crate::elab) struct Scope {
    /// The file, params, locals, frame and control flow of the code: what value lowering reads and writes.
    pub(in crate::elab) values: ValueScope,
    /// The branch and site context of the code: what analog lowering reads and writes.
    pub(in crate::elab) analog: AnalogScope,
    /// The carrier context of the code: what discrete lowering reads and writes.
    pub(in crate::elab) digital: DigitalScope,
}

impl Scope {
    /// Takes over what a standalone scope made from a copy of this one's (`standalone_scope`) added: the `ac_stim` sites
    /// and the statements that write them, the branches a flow read numbered, and the flow leaves. So a stimulus or a
    /// flow read inside a value a primitive evaluates on its own (a `noise` density, a `delay` input, a `ddt` state, a
    /// `ddx` value) is a site or a branch of the body. The `limit` sites it made stay its own: a `limit` there is read
    /// at a solution (`at_solution`) or differentiated through, never evaluated.
    pub(in crate::elab) fn adopt_standalone(&mut self, standalone: &mut Scope) {
        self.analog.stims = standalone.analog.stims.clone();
        self.analog.branches = standalone.analog.branches.clone();
        self.analog.flow_leaves.append(&mut standalone.analog.flow_leaves);
        self.analog.hoisted_steps.append(&mut standalone.analog.hoisted_steps);
    }

    /// The scope of code written in `file`: no param, no local, frame 0, outside every analog block, loop and function.
    pub(in crate::elab) fn new(file: FileId) -> Self {
        Scope { values: ValueScope::new(file), analog: AnalogScope::default(), digital: DigitalScope::default() }
    }
}

/// Everything one elaboration run holds: the inputs it only reads, the IR being built and what it reported, and one
/// table per concern. A table is filled and read by the lowering of its concern; the root fields are read by all.
pub(super) struct ElaborationState<'d, 't> {
    /// The design being elaborated, only read.
    pub(super) design: &'d Design,
    /// The types the check gave the design, only read.
    pub(super) types: &'t Types<'d>,
    /// The constant evaluator every concern folds through (§8.1.21).
    pub(super) consts: ConstEval<'d, 't>,
    /// The IR being built.
    pub(super) builder: IrBuilder,
    /// The source position of a declaration, an expression or a statement: the origin of what lowers from it.
    pub(super) locator: Locator<'d>,
    /// The params the host bound before the run.
    pub(super) overrides: Overrides,
    /// Every trait and `impl` of the design: what a call through a bound resolves against (`methods.rs`).
    pub(super) impls: crate::types::ImplTable<'d>,
    /// What the run has reported so far, in report order.
    pub(super) diagnostics: Vec<DesignDiagnostic>,
    /// The state of value code (§1.5, §3): the type, continuity and origin of every IR expression, the divergences, the
    /// type arena.
    pub(super) values: ExpressionTables,
    /// The state of functions (§3.4): the IR function of each variant a call reached.
    pub(super) functions: FunctionTables,
    /// The state of analog code (§14): natures, ammeters, loops and call cycles.
    pub(super) analog: AnalogSites,
    /// The state of discrete code (§12, §13): the carriers, drives, events and reactions of the body, the resolved nets.
    pub(super) digital: DigitalTables,
    /// The state of module bodies and the placed tree (§8): bodies, plans, key params, frames, generated items, reads
    /// through children.
    pub(super) structure: StructureTables,
}

impl<'d, 't> ElaborationState<'d, 't> {
    /// The state of a run over `design`, typed as `types`, with the params the host bound (`overrides`) and the reads
    /// through a child of a child the runs before it found (`demands`).
    fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides, demands: Vec<super::reach::demands::NestedDemand>) -> Self {
        ElaborationState {
            design,
            types,
            consts: ConstEval::new(design, types),
            builder: IrBuilder::new(),
            locator: Locator::new(design),
            overrides,
            impls: crate::types::ImplTable::new(design),
            diagnostics: Vec::new(),
            values: ExpressionTables::default(),
            functions: FunctionTables::default(),
            analog: AnalogSites::default(),
            digital: DigitalTables::default(),
            structure: StructureTables::new(demands),
        }
    }
}

impl<'d, 't> Elaboration<'d, 't> {
    /// The view that lowers value code.
    pub(super) fn values(&mut self) -> ValueLowering<'_, 'd, 't> {
        ValueLowering::new(self)
    }

    /// A run over `design`, typed as `types`, with the params the host bound (`overrides`) and the reads through a child
    /// of a child the runs before it found (`demands`).
    fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides, demands: Vec<super::reach::demands::NestedDemand>) -> Self {
        Elaboration { state: ElaborationState::new(design, types, overrides, demands) }
    }

    /// Copies the wide integer constants the evaluator made into the IR, in handle order, so each
    /// `Constant::Wide` and `Expr::Wide` the elaboration emitted names the entry it was made as
    /// (`consts/numbers.rs`).
    fn sync_wides(&mut self) {
        for data in self.state.consts.take_new_wides() {
            self.state.builder.wide(data);
        }
    }

    /// The run, and the demands it found that its tables did not hold (`reach/demands.rs`): when there are any, the
    /// result is not the run's and the caller runs again with them.
    fn run(mut self, top: DeclId) -> (Result<Ir, Vec<DesignDiagnostic>>, Vec<super::reach::demands::NestedDemand>) {
        let result = self.elaborate(top);
        (result, self.state.structure.nested_demands.into_wanted())
    }

    /// The passes of one run.
    fn elaborate(&mut self, top: DeclId) -> Result<Ir, Vec<DesignDiagnostic>> {
        if self.refuse_value_inputs(top) {
            return Err(std::mem::take(&mut self.state.diagnostics));
        }
        // A module with a result or a value input gets no body: it is hoisted into each body that
        // places it. Its natures are still the IR's before any body.
        let reached: Vec<DeclId> = self.state.types.reached().iter().copied().filter(|d| matches!(&self.state.design.decl_data(*d).kind, DeclKind::Module(_))).collect();
        let modules: Vec<DeclId> = reached.iter().copied().filter(|d| !self.is_hoisted(*d)).collect();
        // a generic module has no body of its own: each instance a placement makes is one (`generic_bodies.rs`)
        let with_generic = modules.clone();
        let modules: Vec<DeclId> = modules.into_iter().filter(|m| !self.is_generic_module(*m)).collect();
        self.values().read_included_files();
        self.values().build_type_arena();
        self.plan_params(&modules);
        self.classify_module_param_keys(&with_generic);
        self.classify_folded_param_keys(&with_generic);
        self.classify_key_params(&modules);
        // An unkeyed module keeps the id of its place in first-reach order; a keyed module's
        // variants take the ids after them, on request.
        let unkeyed: Vec<DeclId> = modules.iter().copied().filter(|m| self.state.structure.keys.of(*m).is_empty()).collect();
        self.state.structure.body_ids = unkeyed.iter().enumerate().map(|(k, m)| (*m, BodyId(k as u32))).collect();
        self.state.structure.body_variants = BodyVariants::starting_at(unkeyed.len() as u32);
        self.lower_natures(&reached);
        for module in &unkeyed {
            if let Some(id) = self.state.structure.body_ids.get(module).copied() {
                self.lower_body(*module, &Bindings::new(), BodyKey::unkeyed(*module), id);
            }
        }
        self.lower_pending_bodies();
        self.pad_child_nodes();
        let top_instance = self.place_top(top);
        // a body variant the placement asked for after the first padding (the top's own, keyed by a host override) is padded too
        self.pad_child_nodes();
        self.build_resolvers();
        let reprs = self.values().lower_hand_written_reprs();
        self.add_functions();
        self.values().set_hand_written_reprs(reprs);
        // the last net: an elaboration that placed no top and said nothing is refused by name, never an empty `Err`
        if top_instance.is_none() && self.state.diagnostics.is_empty() {
            let (file, span) = (self.state.design.decl_data(top).file, self.state.design.decl_data(top).span);
            self.refuse_silent_failure(file, span, ValueForm::TopModuleItsElaborationFailedWithoutDiagnostic);
        }
        match top_instance {
            Some(instance) if self.state.diagnostics.is_empty() => {
                self.sync_wides();
                let ir = std::mem::replace(&mut self.state.builder, IrBuilder::new()).finish(instance);
                self.state.diagnostics.extend(Self::signal_flow_diagnostics(self.state.design, top, &ir));
                self.state.diagnostics.extend(Self::loop_diagnostics(self.state.design, top, &ir));
                if !self.state.diagnostics.is_empty() {
                    return Err(std::mem::take(&mut self.state.diagnostics));
                }
                Self::well_formed(self.state.design, top, ir)
            }
            _ => Err(std::mem::take(&mut self.state.diagnostics)),
        }
    }

    /// The `Ir` itself when `check()` finds nothing; else one `P0004` internal-error diagnostic
    /// per broken invariant, at the top module, and no `Ir`. Never checked in debug builds only:
    /// a lowering gap must not reach a backend as an `Ok`.
    pub(super) fn well_formed(design: &Design, top: DeclId, ir: Ir) -> Result<Ir, Vec<DesignDiagnostic>> {
        let broken = ir.check();
        if broken.is_empty() {
            return Ok(ir);
        }
        let data = design.decl_data(top);
        let span = data.name.as_ref().map_or(data.span, |n| n.span);
        Err(broken
            .iter()
            .map(|b| DesignDiagnostic {
                file: data.file,
                diagnostic: Diagnostic::new("P0004", format!("internal error: elaboration produced an ill-formed IR: {} ({} at {})", b.what, b.item, b.origin), span, "this is a bug in the elaborator, not in the model"),
            })
            .collect())
    }

    /// `E0242` for each signal-flow node of `ir` that does not have exactly one potential driver
    /// (§6 rule 2), reported at the top module and naming the node and each driver.
    fn signal_flow_diagnostics(design: &Design, top: DeclId, ir: &Ir) -> Vec<DesignDiagnostic> {
        let data = design.decl_data(top);
        let (file, span) = (data.file, data.name.as_ref().map_or(data.span, |n| n.span));
        SignalFlowCheck::run(ir)
            .into_iter()
            .map(|fault| {
                let drivers = if fault.drivers.is_empty() { "none".to_string() } else { fault.drivers.join(", ") };
                let message = format!("the signal-flow node `{}` has {} potential driver(s), it takes exactly one (§6 rule 2); drivers: {drivers}", fault.node, fault.drivers.len());
                DesignDiagnostic { file, diagnostic: Diagnostic::new("E0242", message, span, "a signal-flow node with the wrong number of potential drivers") }
            })
            .collect()
    }

    /// `E0339` for each loop of undelayed drives of `ir` that the placed tree closes through a one-driver net
    /// (§8.2.3), reported at the top module and naming the drives.
    fn loop_diagnostics(design: &Design, top: DeclId, ir: &Ir) -> Vec<DesignDiagnostic> {
        let data = design.decl_data(top);
        let (file, span) = (data.file, data.name.as_ref().map_or(data.span, |n| n.span));
        DriveLoops::run(ir)
            .into_iter()
            .map(|fault| {
                let message = format!("undelayed definitions of {} depend on each other across instances: a loop through one-driver carriers (§8.2.3)", fault.drives.iter().map(|drive| format!("`{drive}`")).collect::<Vec<_>>().join(", "));
                DesignDiagnostic { file, diagnostic: Diagnostic::new("E0339", message, span, "a loop of definitions") }
            })
            .collect()
    }

    /// Records `diagnostic`, once: a construct lowered in two halves (the `is` and the `val` of an `Option`)
    /// reports the same refusal at the same place twice, which is one finding.
    pub(super) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        let found = DesignDiagnostic { file, diagnostic };
        if !self.state.diagnostics.contains(&found) {
            self.state.diagnostics.push(found);
        }
    }

    /// `P0004`: an invariant of the elaborator broken (a bug in it, not in the model), reported at the module of the body being
    /// lowered; the run then gives no `Ir`, as for an ill-formed one (`well_formed`).
    pub(super) fn internal_error(&mut self, what: &str) {
        // every lowering runs with frame 0 set; before one, the body asked for (`lowering`) names the module
        let module = self.state.structure.frames.first().map(|frame| frame.module).or_else(|| self.state.structure.lowering.as_ref().map(|key| key.module));
        let (file, span) = match module {
            Some(module) => {
                let data = self.state.design.decl_data(module);
                (data.file, data.name.as_ref().map_or(data.span, |n| n.span))
            }
            None => (FileId(0), Span::default()),
        };
        self.report(file, Diagnostic::new("P0004", format!("internal error: {what}"), span, "this is a bug in the elaborator, not in the model"));
    }

    /// `P0001` at `span`, naming `form`: a construct inside the type-checked subset that elaboration does not lower.
    pub(super) fn refuse(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        self.report(file, Refusal::of(form).diagnostic(span));
    }

    /// [`refuse`](Self::refuse) with `details` for the `{}` of the form's words.
    pub(super) fn refuse_with(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>, details: impl IntoIterator<Item = String>) {
        let refusal = details.into_iter().fold(Refusal::of(form), Refusal::with_detail);
        self.report(file, refusal.diagnostic(span));
    }

    /// [`refuse`](Self::refuse) unless the read through the frame `frame` passes through a module refused for its own
    /// cause (`refused_modules`): that refusal is the read's, reported once. Any other refusal of the run, of another module or of
    /// another read, hides nothing.
    pub(super) fn refuse_unless_refused(&mut self, frame: usize, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        if !self.reads_through_refused(frame) {
            self.refuse(file, span, form);
        }
    }

    /// Whether the frame `frame`, or a frame it hangs from, is of a module refused for its own cause (`refused_modules`).
    pub(super) fn reads_through_refused(&self, frame: usize) -> bool {
        let mut at = Some(frame);
        while let Some(index) = at {
            let Some(held) = self.state.structure.frames.get(index) else { return false };
            if self.state.structure.refused_modules.contains(&held.module) {
                return true;
            }
            at = held.parent;
        }
        false
    }

    /// The type the checker gave `expr`, with the generic parameters of the instance being lowered
    /// replaced by their arguments: the one read of the type table of an expression (`ConstEval::typed`).
    pub(super) fn typed(&self, expr: ExprId) -> Option<Ty> {
        self.state.consts.typed(expr).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The type of the binding `binding` under the substitution.
    pub(super) fn typed_local(&self, binding: DeclId) -> Option<Ty> {
        self.state.consts.typed_local(binding).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The type the type node `ty` writes, under the substitution.
    pub(super) fn typed_written(&self, ty: crate::pom::TypeId) -> Option<Ty> {
        self.state.consts.typed_written(ty).map(|ty| self.with_hand_written_widths(&ty))
    }

    /// The IR type of a type of the subset; `None` when no value has it in the IR.
    pub(super) fn value_type(&self, ty: &Ty) -> Option<ValueType> {
        ValueTypes { arena: &self.state.values.type_arena }.of(ty)
    }

    /// Add an expression to the IR, remembering its type (the builder does not hand it back) and
    /// whether it is continuous (`events.rs`).
    pub(super) fn emit(&mut self, kind: Expr, ty: ValueType, origin: Origin) -> LoweredExpr {
        self.state.values.expr_types.push(ty.clone());
        let continuous = self.continuous_kind(&kind);
        self.state.values.expr_continuous.push(continuous);
        self.state.values.expr_origins.push(origin.clone());
        self.state.builder.expr(kind, ty, origin)
    }

    /// Add a statement to the IR, remembering whether it reads a continuous value.
    pub(super) fn stmt(&mut self, stmt: Stmt) -> LoweredStmt {
        let continuous = stmt.operands().iter().any(|e| self.continuous(*e));
        self.state.values.stmt_continuous.push(continuous);
        let id = self.state.builder.stmt(stmt.clone());
        self.state.values.divergences.note_statement(id, &stmt);
        id
    }

    /// The type of an expression already added.
    pub(super) fn type_of(&self, expr: LoweredExpr) -> ValueType {
        self.state.values.expr_types[expr.index()].clone()
    }

    /// The origin of an expression: where its construct starts.
    pub(super) fn expr_origin(&self, expr: ExprId) -> Origin {
        let node = self.state.design.expr_node(expr);
        self.state.locator.origin(self.state.design.decl_data(node.owner).file, node.span)
    }

    /// `P0001` for something that failed to lower while no diagnostic was reported: the one place the elaborator says
    /// "nothing vanishes silently".
    pub(super) fn refuse_silent_failure(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        self.refuse(file, span, form);
    }

    /// `expr` without the parentheses around it.
    pub(super) fn peel_parens(&self, mut expr: ExprId) -> ExprId {
        while let ExprKind::Paren(inner) = &self.state.design.expr_node(expr).kind {
            expr = *inner;
        }
        expr
    }

    /// `ty` with each `[_; T::WIDTH]` that is still generic (the consts evaluator folds the built-in layouts) folded
    /// to the `WIDTH` of the hand-written `impl Repr` of the type `T` is given.
    pub(super) fn with_hand_written_widths(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Array(element, Length::Generic(ConstTerm::Width(parameter))) => {
                let element = Box::new(self.with_hand_written_widths(element));
                let width = match self.state.consts.substitution().get(*parameter) {
                    Some(GenericValue::Type(given)) => self.level_count(given),
                    _ => None,
                };
                match width {
                    Some(levels) => Ty::Array(element, Length::Fixed(u64::from(levels))),
                    None => Ty::Array(element, Length::Generic(ConstTerm::Width(*parameter))),
                }
            }
            Ty::Array(element, length) => Ty::Array(Box::new(self.with_hand_written_widths(element)), length.clone()),
            Ty::Tuple(items) => Ty::Tuple(items.iter().map(|item| self.with_hand_written_widths(item)).collect()),
            Ty::Option(inner) => Ty::Option(Box::new(self.with_hand_written_widths(inner))),
            other => other.clone(),
        }
    }

    /// The number of levels of the `Repr` of `ty` (§2): the width of its hand-written `impl`, else the count its layout
    /// gives (rules 2 to 6). `None` for a type with no laid-out value type.
    pub(super) fn level_count(&self, ty: &Ty) -> Option<u32> {
        let hand_written = self.state.impls.hand_written_repr(ty).map(|entry| entry.decl).and_then(|owner| self.hand_written_width(owner, &self.hand_written_arguments(owner, ty)?));
        hand_written.or_else(|| self.state.types.repr_levels(ty))
    }

    /// The arguments the `impl Repr` `owner` takes for the type `ty` it is written for: what its parameters stand for
    /// (`impl<T: Repr> Repr for Pair<T>` meeting `Pair<u8>` gives `u8`), in the order of its parameters; empty for an
    /// `impl` with none. `None` when a parameter stays open.
    pub(super) fn hand_written_arguments(&self, owner: DeclId, ty: &Ty) -> Option<Vec<ConcreteValue>> {
        let encode = self.state.impls.item(owner, "encode")?;
        if !self.is_generic_impl_item(encode) {
            return Some(Vec::new());
        }
        let found = self.state.types.impl_arguments(encode, ty)?;
        found
            .into_iter()
            .map(|value| match value {
                GenericValue::Type(argument) => Some(ConcreteValue::Type(argument)),
                GenericValue::Const(ConstTerm::Value(count)) => Some(ConcreteValue::Const(count)),
                _ => None,
            })
            .collect()
    }

    /// The folded `WIDTH` of the `impl Repr` `owner` for `arguments`: the level count of each type argument and the value
    /// of each const argument stand where the `impl`'s parameters are named (`2 * T::WIDTH`).
    pub(super) fn hand_written_width(&self, owner: DeclId, arguments: &[ConcreteValue]) -> Option<u32> {
        let width = self.state.impls.item(owner, "WIDTH")?;
        let DeclKind::Const(data) = &self.state.design.decl_data(width).kind else { return None };
        let signature = GenericSignature::of(self.state.design, owner);
        let values: BTreeMap<DeclId, u128> = signature
            .parameters()
            .iter()
            .zip(arguments)
            .filter_map(|(parameter, argument)| match argument {
                ConcreteValue::Type(ty) => self.level_count(ty).map(|levels| (parameter.decl, u128::from(levels))),
                ConcreteValue::Const(count) => u128::try_from(*count).ok().map(|count| (parameter.decl, count)),
                _ => None,
            })
            .collect();
        u32::try_from(self.state.types.fold_with(data.value?, values)?).ok()
    }

    /// Whether the lowered expression `expr` is continuous.
    pub(super) fn continuous(&self, expr: LoweredExpr) -> bool {
        self.state.values.expr_continuous.get(expr.index()).copied().unwrap_or(false)
    }

    /// Whether an expression of kind `kind`, whose operands are lowered already, is continuous.
    pub(super) fn continuous_kind(&self, kind: &Expr) -> bool {
        let any = |es: &[LoweredExpr]| es.iter().any(|e| self.continuous(*e));
        match kind {
            Expr::Probe { .. } | Expr::Flow { .. } | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::Env(Env::Time | Env::Static | Env::Gmin | Env::SourceScale | Env::Freq) => true,
            Expr::Env(Env::Temperature | Env::TemperatureGiven | Env::Analysis(_)) | Expr::SessionParameter { .. } | Expr::Fired(_) | Expr::Real(_) | Expr::Bool(_) | Expr::Unit | Expr::Local(_) | Expr::Param(_) | Expr::ParamPrevious(_) | Expr::Others(_) | Expr::Resolve { .. } | Expr::Int(_) | Expr::FloatBits(_) | Expr::Wide(_) | Expr::Carrier(_) | Expr::Previous(_) => false,
            Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::Table { x: expr, .. } | Expr::TableSlope { x: expr, .. } => self.continuous(*expr),
            Expr::CarrierElement { index, .. } | Expr::ElementPrevious { index, .. } | Expr::ElementChanged { index, .. } => self.continuous(*index),
            Expr::Binary { lhs, rhs, .. } => any(&[*lhs, *rhs]),
            Expr::Unwrap { is, value, .. } => any(&[*is, *value]),
            Expr::Same { a, b } => any(&[*a, *b]),
            Expr::LevelCast { expr, .. } => self.continuous(*expr),
            Expr::If { cond, then, otherwise } => any(&[*cond, *then, *otherwise]),
            Expr::Block { stmts, value } => stmts.iter().any(|s| self.state.values.stmt_continuous.get(s.index()).copied().unwrap_or(false)) || value.is_some_and(|v| self.continuous(v)),
            Expr::Call { function, args } => self.state.functions.variants.continuous(*function) || any(args),
            Expr::Math { args, .. } => any(args),
            Expr::Method { receiver, args, .. } => self.continuous(*receiver) || any(args),
            Expr::Closure { body, .. } => self.continuous(*body),
            Expr::Limit { x, f, seed, .. } => any(&[*x, *f]) || seed.is_some_and(|s| self.continuous(s)),
            Expr::Stim { mag, phase, .. } => self.continuous(*mag) || phase.is_some_and(|p| self.continuous(p)),
            Expr::Loop { .. } | Expr::While { .. } | Expr::ForRange { .. } | Expr::Break(_) | Expr::Continue | Expr::Return(_) | Expr::FunctionRef(_) | Expr::CallValue { .. } | Expr::Tuple(_) | Expr::Array(_) | Expr::ArrayRepeat { .. } | Expr::StructOf { .. } | Expr::VariantOf { .. } | Expr::Field { .. } | Expr::Index { .. } | Expr::Length(_) | Expr::IsVariant { .. } | Expr::VariantField { .. } | Expr::OptionPack { .. } | Expr::OptionPart { .. } | Expr::Default(_) => {
                any(&kind.operands()) || kind.statements().iter().any(|s| self.state.values.stmt_continuous.get(s.index()).copied().unwrap_or(false))
            }
        }
    }

    /// Whether `function` is declared in a trait.
    pub(in crate::elab) fn is_trait_function(&self, function: DeclId) -> bool {
        self.state.design.decl_data(function).parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Trait(_)))
    }

    /// Whether `function` is written in an `impl` that has generic parameters.
    pub(in crate::elab) fn is_generic_impl_item(&self, function: DeclId) -> bool {
        self.state.design.decl_data(function).parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Impl(_)) && self.state.design.decl_data(parent).generics_span.is_some())
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{InstanceId, IrBuilder, ValueType};

    use super::*;
    use crate::elab::carriers::Frame;

    #[test]
    fn an_ill_formed_ir_becomes_an_internal_error_and_no_ir() {
        let design = Design::builder().source("top.msl", "module Top() { }").build();
        let types = Types::check(&design, "Top").expect("a clean model");
        // No instance at all: the top id dangles, so `check()` has something to say.
        let ir = IrBuilder::new().finish(InstanceId(0));
        assert!(!ir.check().is_empty(), "the crafted IR must be broken");
        let err = Elaboration::well_formed(&design, types.reached()[0], ir).expect_err("no Ir");
        assert!(err.iter().all(|d| d.diagnostic.code == "P0004" && d.diagnostic.message.contains("internal error")), "{err:?}");
    }

    #[test]
    fn a_composite_constant_with_no_form_at_its_type_is_an_internal_error_not_a_panic() {
        // the checker's types never ask `constant_expr` for an array at `bool`; asked anyway, the elaborator says `P0004` at the
        // module being lowered and goes on, so the run ends with that diagnostic and no `Ir`
        let design = Design::builder().source("top.msl", "module Top() { }").build();
        let types = Types::check(&design, "Top").expect("a clean model");
        let top = types.reached()[0];
        let mut elaboration = Elaboration::new(&design, &types, Overrides::default(), Vec::new());
        elaboration.state.structure.frames = vec![Frame::root(top)];
        let pair = elaboration.state.builder.aggregate(piperine_ir::AggregateData { ty: ValueType::Array(Box::new(ValueType::F64), 2), variant: None, parts: vec![piperine_ir::Constant::Real(piperine_ir::Real::new(1.0)); 2] });
        let origin = piperine_ir::Origin { file: "top.msl".to_string(), line: 1, col: 1 };
        elaboration.values().constant_expr(piperine_ir::Constant::Aggregate(pair), ValueType::Bool, origin);
        let codes: Vec<&str> = elaboration.state.diagnostics.iter().map(|d| d.diagnostic.code.as_ref()).collect();
        assert_eq!(codes, ["P0004"], "{:?}", elaboration.state.diagnostics);
        let found = &elaboration.state.diagnostics[0];
        assert!(found.diagnostic.message.starts_with("internal error: the composite constant") && found.file == design.decl_data(top).file, "{found:?}");
    }
}
