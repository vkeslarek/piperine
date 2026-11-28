//! The entry point and the pass order of one elaboration run: `Types::check` first, then the
//! passes over what it reached, each writing through `IrBuilder`.

use crate::elab::analog::AnalogLowering;
use crate::elab::digital::DigitalLowering;
use crate::elab::functions::FunctionLowering;
use crate::elab::structure::StructureLowering;
use crate::elab::value::ValueLowering;
use std::collections::BTreeMap;

use piperine_ir::{BodyId, Constant, Env, Expr, ExprId as LoweredExpr, Ir, IrBuilder, Origin, Stmt, StmtId as LoweredStmt, ValueType};

use super::analog::state::{AnalogScope, AnalogSites};
use super::consts::{Bindings, CompositeKind, ConstEval, ConstTree};
use super::digital::state::{DigitalScope, DigitalTables};
use super::functions::state::FunctionTables;
use super::functions::instances::ConcreteValue;
use super::digital::loops::DriveLoops;
use super::value::expressions::ValueTypes;
use super::origin::Locator;
use super::analog::signal_flow::SignalFlowCheck;
use super::structure::structural::{BodyKey, BodyVariants, Overrides};
use super::structure::state::StructureTables;
use super::value::state::{ExpressionTables, ValueScope};
use crate::refusal::forms::ValueForm;
use crate::refusal::{Refusal, RefusedForm};
use crate::msl::{Diagnostic, Span};
use crate::pom::{self, BlockNode, CallClass, DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, FileId, GenerationData, PatKind, Resolution, StmtKind};
use crate::types::{Callee, ConstTerm, GenericSignature, GenericValue, Length, Substitution, Ty, Types};

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
        let mut demands = Elaboration::new(self.design, &types, overrides.clone(), Vec::new()).structure().static_demands();
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

    /// A scope for code of the body outside its `analog` block and its functions (a drive, a
    /// reaction): the names of `scope` (params, frame), with no local of its own.
    pub(in crate::elab) fn sub_scope(scope: &Scope) -> Scope {
        let mut sub = Scope::new(scope.values.file);
        sub.values.params = scope.values.params.clone();
        sub.values.keys = scope.values.keys.clone();
        sub.values.frame = scope.values.frame;
        sub.values.unrolled = scope.values.unrolled.clone();
        sub.digital.ordinals = scope.digital.ordinals.clone();
        sub
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
    fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides, demands: Vec<super::structure::reach::demands::NestedDemand>) -> Self {
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
    /// The view that lowers functions.
    pub(super) fn functions(&mut self) -> FunctionLowering<'_, 'd, 't> {
        FunctionLowering::new(self)
    }

    /// The view that lowers discrete code.
    pub(super) fn digital(&mut self) -> DigitalLowering<'_, 'd, 't> {
        DigitalLowering::new(self)
    }

    /// The view that lowers module structure.
    pub(super) fn structure(&mut self) -> StructureLowering<'_, 'd, 't> {
        StructureLowering::new(self)
    }

    /// The view that lowers analog code.
    pub(super) fn analog(&mut self) -> AnalogLowering<'_, 'd, 't> {
        AnalogLowering::new(self)
    }

    /// The view that lowers value code.
    pub(super) fn values(&mut self) -> ValueLowering<'_, 'd, 't> {
        ValueLowering::new(self)
    }

    /// A run over `design`, typed as `types`, with the params the host bound (`overrides`) and the reads through a child
    /// of a child the runs before it found (`demands`).
    fn new(design: &'d Design, types: &'t Types<'d>, overrides: Overrides, demands: Vec<super::structure::reach::demands::NestedDemand>) -> Self {
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
    fn run(mut self, top: DeclId) -> (Result<Ir, Vec<DesignDiagnostic>>, Vec<super::structure::reach::demands::NestedDemand>) {
        let result = self.elaborate(top);
        (result, self.state.structure.nested_demands.into_wanted())
    }

    /// The passes of one run.
    fn elaborate(&mut self, top: DeclId) -> Result<Ir, Vec<DesignDiagnostic>> {
        if self.analog().refuse_value_inputs(top) {
            return Err(std::mem::take(&mut self.state.diagnostics));
        }
        // A module with a result or a value input gets no body: it is hoisted into each body that
        // places it. Its natures are still the IR's before any body.
        let reached: Vec<DeclId> = self.state.types.reached().iter().copied().filter(|d| matches!(&self.state.design.decl_data(*d).kind, DeclKind::Module(_))).collect();
        let modules: Vec<DeclId> = reached.iter().copied().filter(|d| !self.is_hoisted(*d)).collect();
        // a generic module has no body of its own: each instance a placement makes is one (`generic_bodies.rs`)
        let with_generic = modules.clone();
        let modules: Vec<DeclId> = modules.into_iter().filter(|m| !self.structure().is_generic_module(*m)).collect();
        self.values().read_included_files();
        self.values().build_type_arena();
        self.structure().plan_params(&modules);
        self.structure().classify_module_param_keys(&with_generic);
        self.structure().classify_folded_param_keys(&with_generic);
        self.structure().classify_key_params(&modules);
        // An unkeyed module keeps the id of its place in first-reach order; a keyed module's
        // variants take the ids after them, on request.
        let unkeyed: Vec<DeclId> = modules.iter().copied().filter(|m| self.state.structure.keys.of(*m).is_empty()).collect();
        self.state.structure.body_ids = unkeyed.iter().enumerate().map(|(k, m)| (*m, BodyId(k as u32))).collect();
        self.state.structure.body_variants = BodyVariants::starting_at(unkeyed.len() as u32);
        self.analog().lower_natures(&reached);
        for module in &unkeyed {
            if let Some(id) = self.state.structure.body_ids.get(module).copied() {
                self.structure().lower_body(*module, &Bindings::new(), BodyKey::unkeyed(*module), id);
            }
        }
        self.structure().lower_pending_bodies();
        self.structure().pad_child_nodes();
        let top_instance = self.structure().place_top(top);
        // a body variant the placement asked for after the first padding (the top's own, keyed by a host override) is padded too
        self.structure().pad_child_nodes();
        self.digital().build_resolvers();
        let reprs = self.values().lower_hand_written_reprs();
        self.functions().add_functions();
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

    /// The name of `function` as written; a function of an `impl` with no parameters is named by the type the `impl` is for, `Sq.area`, so
    /// two types giving one method name stay two IR functions.
    pub(in crate::elab) fn impl_qualified_name(&self, function: DeclId) -> String {
        let data = self.state.design.decl_data(function);
        let own = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(parent) = data.parent else { return own };
        // an `impl` with parameters is told apart by the arguments of its instances
        let DeclKind::Impl(owner) = &self.state.design.decl_data(parent).kind else { return own };
        // an `impl` with parameters is told apart by the arguments of its instances, and a trait `impl` also by its type and
        // its trait arguments (`impl BitAnd for Logic<N>` and `impl BitAnd<UInt<N>, Logic<N>> for Logic<N>` give two `bitand`)
        if self.state.design.decl_data(parent).generics_span.is_some() && owner.trait_.is_none() {
            return own;
        }
        let suffix = self.trait_arguments_suffix(parent);
        // the type as written, under no substitution: the name of an instance is the same wherever it is asked
        match self.state.consts.written_as_declared(owner.self_ty) {
            Some(ty) => format!("{}.{own}{suffix}", ty.source_name(self.state.design)),
            None => match self.nature_target_name(parent) {
                Some(nature) => format!("{nature}.{own}{suffix}"),
                None => own,
            },
        }
    }

    /// The name of the item a closure is written in: two items each have a `closure$0`, so the instance names the item too.
    pub(super) fn closure_owner_name(&self, closure: ExprId) -> String {
        let owner = self.state.design.expr_node(closure).owner;
        self.state.design.decl_data(owner).name.as_ref().map_or_else(String::new, |name| name.text.clone())
    }

    /// The position of the closure `closure` among the closures of the item it is written in, in source order.
    pub(super) fn closure_ordinal(&self, closure: ExprId) -> usize {
        let owner = self.state.design.expr_node(closure).owner;
        (0..closure.0).filter(|index| {
            let node = self.state.design.expr_node(ExprId(*index));
            node.owner == owner && matches!(node.kind, crate::pom::ExprKind::Closure { .. })
        }).count()
    }

    /// The name of the nature a trait `impl` is written for (`impl Resolve<f64> for Total`): a nature is no written
    /// type, so two `impl`s of one trait for two natures tell their methods apart by it.
    fn nature_target_name(&self, implementation: DeclId) -> Option<String> {
        let entry = self.state.impls.impls().iter().find(|entry| entry.decl == implementation)?;
        match &entry.target {
            crate::types::ImplTarget::Declared(decl) => self.state.design.decl_data(*decl).name.as_ref().map(|n| n.text.clone()),
            _ => None,
        }
    }

    /// `::<f64>` for an `impl` of a trait written with arguments (`impl Convert<f64> for Cell`), empty for any other: two
    /// `impl`s of one trait for one type, written with different arguments, each give a function of one name, and the
    /// arguments tell them apart. Not inlined: it runs once for every instantiation, deep in a recursion.
    #[inline(never)]
    fn trait_arguments_suffix(&self, owner: DeclId) -> String {
        let arguments = self.state.impls.impls().iter().find(|entry| entry.decl == owner).map(|entry| entry.trait_arguments.clone()).unwrap_or_default();
        let written: Vec<String> = arguments.iter().filter_map(|argument| self.state.consts.written_as_declared(*argument)).map(|ty| ty.source_name(self.state.design)).collect();
        if written.is_empty() { String::new() } else { format!("::<{}>", written.join(", ")) }
    }

    /// What the generic parameters of `function` are replaced by in the body of the instance with `arguments`: a
    /// type for a type parameter and a value for a const one; a closure or a function has no type to give yet (T37).
    pub(in crate::elab) fn instance_substitution(&self, function: DeclId, arguments: &[ConcreteValue]) -> Substitution {
        let mut substitution = Substitution::new();
        // a method of a generic `impl` takes the parameters of the `impl`
        let owner = self.state.design.decl_data(function).parent.filter(|owner| matches!(self.state.design.decl_data(*owner).kind, DeclKind::Impl(_)) && self.state.design.decl_data(*owner).generics_span.is_some()).unwrap_or(function);
        let signature = GenericSignature::of(self.state.design, owner);
        // a function a trait declares runs for one type, which is `Self`: its one argument
        if let (true, Some(ConcreteValue::Type(ty))) = (self.is_trait_function(function), arguments.first())
            && let Some(owner) = self.state.design.decl_data(function).parent
        {
            substitution.bind(owner, GenericValue::Type(ty.clone()));
        }
        for (parameter, argument) in signature.parameters().iter().zip(arguments) {
            match argument {
                ConcreteValue::Type(ty) => substitution.bind(parameter.decl, GenericValue::Type(ty.clone())),
                ConcreteValue::Const(value) => substitution.bind(parameter.decl, GenericValue::Const(ConstTerm::Value(*value))),
                ConcreteValue::Bool(flag) => substitution.bind(parameter.decl, GenericValue::Const(ConstTerm::Bool(*flag))),
                // a closure or a function stands for its function type: the parameter that takes it is a function value
                ConcreteValue::Function(_) | ConcreteValue::Closure(_) => {
                    if let Some(ty) = self.callable_type(argument) {
                        substitution.bind(parameter.decl, GenericValue::Type(ty));
                    }
                }
            }
        }
        substitution
    }

    /// The function type a closure or a function name has: what the parameter bound by `Fn(A) -> R` is in the
    /// instance (D9: the instance takes the callable as a function value, called by `f(x)` like any local of
    /// function type). `None` for an argument that is no callable.
    fn callable_type(&self, value: &ConcreteValue) -> Option<Ty> {
        match value {
            ConcreteValue::Closure(closure) => self.typed(*closure),
            ConcreteValue::Function(function) => {
                let data = self.state.design.decl_data(*function);
                let DeclKind::Fn(signature) = &data.kind else { return None };
                let inputs: Option<Vec<Ty>> = data.children.iter().copied().filter(|child| matches!(self.state.design.decl_data(*child).kind, DeclKind::FnParam(_))).map(|param| self.typed_local(param)).collect();
                let output = signature.ret.and_then(|ret| self.typed_written(ret)).unwrap_or(Ty::Unit);
                Some(Ty::Fn(inputs?, Box::new(output)))
            }
            _ => None,
        }
    }

    /// Whether `function` takes a handle: a parameter the checker found to be one.
    pub(in crate::elab) fn takes_handle(&self, function: DeclId) -> bool {
        self.state.design.decl_data(function).children.iter().any(|child| matches!(self.state.design.decl_data(*child).kind, DeclKind::FnParam(_)) && self.state.types.handle(*child))
    }

    /// What the assignments of the `let` `local` read, in the block that declares it: each value assigned and each condition around
    /// an assignment (an `if`, a `match`, a loop's condition or range, and every condition inside a loop around it), whose params
    /// a choice by `local` makes structural as it does those of the initializer (`structural.rs`).
    pub(in crate::elab) fn assignment_reads(&self, local: DeclId) -> Vec<ExprId> {
        let Some((stmts, from)) = self.block_declaring(local) else { return Vec::new() };
        let mut inside = Vec::new();
        for stmt in &stmts[from..] {
            for child in crate::pom::BodyWalk::new(self.state.design).statement_children(*stmt) {
                crate::pom::BodyWalk::new(self.state.design).descendants_depth_first(child, &mut inside);
            }
        }
        let within = |outer: crate::msl::Span, inner: crate::msl::Span| outer.start <= inner.start && inner.end <= outer.end;
        let mut reads = Vec::new();
        for assignment in inside.iter().copied() {
            let ExprKind::Assign { lhs, rhs, .. } = &self.state.design.expr_node(assignment).kind else { continue };
            if !self.read_bindings(*lhs).contains(&local) {
                continue;
            }
            reads.push(*rhs);
            let at = self.state.design.expr_node(assignment).span;
            for around in inside.iter().copied().filter(|expr| within(self.state.design.expr_node(*expr).span, at)) {
                match &self.state.design.expr_node(around).kind {
                    ExprKind::While { .. } | ExprKind::Loop { .. } | ExprKind::For { .. } => {
                        let span = self.state.design.expr_node(around).span;
                        reads.extend(inside.iter().filter(|expr| within(span, self.state.design.expr_node(**expr).span)).flat_map(|expr| Self::conditions_of(&self.state.design.expr_node(*expr).kind)));
                    }
                    kind => reads.extend(Self::conditions_of(kind)),
                }
            }
        }
        reads
    }

    /// The bindings `expr` names, anywhere in it.
    pub(in crate::elab) fn read_bindings(&self, expr: ExprId) -> Vec<DeclId> {
        let node = self.state.design.expr_node(expr);
        match &node.kind {
            ExprKind::Path(path) => match self.state.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) if matches!(self.state.design.decl_data(decl).kind, crate::pom::DeclKind::Binding(_)) => vec![decl],
                _ => Vec::new(),
            },
            kind => crate::pom::BodyWalk::new(self.state.design).expression_children(kind).into_iter().flat_map(|child| self.read_bindings(child)).collect(),
        }
    }

    /// The initializer of the `let` that binds `local` with a name, in a statement or a module's body.
    pub(in crate::elab) fn let_initializer(&self, local: DeclId) -> Option<ExprId> {
        if let Some(value) = self.body_let_value(local) {
            return Some(value);
        }
        (0..self.state.design.stmt_count()).find_map(|index| match &self.state.design.stmt_node(crate::pom::StmtId(index as u32)).kind {
            StmtKind::Let { pat, init: Some(init), .. } if matches!(&self.state.design.pat_node(*pat).kind, PatKind::Ident { binding, sub: None, .. } if *binding == local) => Some(*init),
            _ => None,
        })
    }

    /// The quantity the call of `callee` reads or contributes: a nature's access alias (`i`, `v`)
    /// or the built-in `flow` and `potential`; `None` for anything else.
    pub(in crate::elab) fn quantity_role(&self, callee: ExprId) -> Option<pom::QuantityRole> {
        let ExprKind::Path(path) = &self.state.design.expr_node(callee).kind else { return None };
        match self.state.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res) {
            Resolution::Decl(q) => match &self.state.design.decl_data(q).kind {
                DeclKind::Quantity(data) => Some(data.role),
                _ => None,
            },
            Resolution::Builtin(id) if id.info().name == "flow" => Some(pom::QuantityRole::Flow),
            Resolution::Builtin(id) if id.info().name == "potential" => Some(pom::QuantityRole::Potential),
            // an access the resolver left ambiguous takes the role the checker read from the node (`v(y)`, `let y = x;`)
            Resolution::Deferred(_) => self.state.types.access_role(callee),
            _ => None,
        }
    }

    /// Every expression owned by `module` or by one of its behaviors.
    pub(in crate::elab) fn module_exprs(&self, module: DeclId) -> Vec<ExprId> {
        let DeclKind::Module(data) = &self.state.design.decl_data(module).kind else { return Vec::new() };
        let behaviors = &data.behaviors;
        let inside = |mut owner: DeclId| loop {
            if owner == module || behaviors.contains(&owner) {
                return true;
            }
            match self.state.design.decl_data(owner).parent {
                Some(parent) => owner = parent,
                None => return false,
            }
        };
        (0..self.state.design.expr_count() as u32).map(ExprId).filter(|e| inside(self.state.design.expr_node(*e).owner)).collect()
    }

    /// Every expression of `body`, nested ones included.
    pub(in crate::elab) fn body_exprs(&self, body: &BlockNode) -> Vec<ExprId> {
        let mut inside = Vec::new();
        for expr in crate::pom::BodyWalk::new(self.state.design).block_expressions(body) {
            crate::pom::BodyWalk::new(self.state.design).descendants_depth_first(expr, &mut inside);
        }
        inside
    }

    /// The statements of the block that declares the `let` `local` with a name, and the index of its `let` among them: the body
    /// of a function, an `analog` block or a reaction, or a block of an expression of the same owner.
    pub(super) fn block_declaring(&self, local: DeclId) -> Option<(Vec<crate::pom::StmtId>, usize)> {
        let declares = |stmt: &crate::pom::StmtId| matches!(&self.state.design.stmt_node(*stmt).kind, StmtKind::Let { pat, .. } if matches!(&self.state.design.pat_node(*pat).kind, PatKind::Ident { binding, sub: None, .. } if *binding == local));
        let statement = (0..self.state.design.stmt_count() as u32).map(crate::pom::StmtId).find(|stmt| declares(stmt))?;
        let owner = self.state.design.stmt_node(statement).owner;
        let mut blocks: Vec<Vec<crate::pom::StmtId>> = match &self.state.design.decl_data(owner).kind {
            crate::pom::DeclKind::Fn(data) => data.body.iter().map(|body| body.stmts.clone()).collect(),
            crate::pom::DeclKind::Behavior(data) => vec![data.body.clone()],
            crate::pom::DeclKind::Reaction(data) => vec![data.body.stmts.clone()],
            _ => Vec::new(),
        };
        for index in 0..self.state.design.expr_count() as u32 {
            let node = self.state.design.expr_node(ExprId(index));
            if node.owner != owner {
                continue;
            }
            match &node.kind {
                ExprKind::Block(block) | ExprKind::If { then: block, .. } | ExprKind::While { body: block, .. } | ExprKind::Loop { body: block } | ExprKind::For { body: block, .. } => blocks.push(block.stmts.clone()),
                _ => {}
            }
        }
        blocks.into_iter().find_map(|stmts| stmts.iter().position(declares).map(|from| (stmts, from)))
    }

    /// The conditions an expression of kind `kind` decides by: an `if`'s, a `match`'s scrutinee and guards, a `while`'s, a `for`'s
    /// range.
    pub(super) fn conditions_of(kind: &ExprKind) -> Vec<ExprId> {
        match kind {
            ExprKind::If { cond, .. } | ExprKind::While { cond, .. } => vec![*cond],
            ExprKind::For { iter, .. } => vec![*iter],
            ExprKind::Match { scrutinee, arms } => std::iter::once(*scrutinee).chain(arms.iter().filter_map(|arm| arm.guard)).collect(),
            _ => Vec::new(),
        }
    }

    /// The `impl Resolve` of `nature` the first drive of the resolved carrier `decl` picked. A carrier of a nature parameter
    /// bound by `Resolve<X, V>` picked the bound (the checker saw no concrete nature): the instance's nature answers with its
    /// `impl` whose drive type is `X`. `None` when no drive picked one.
    pub(in crate::elab) fn concrete_implementation(&self, decl: DeclId, nature: DeclId) -> Option<DeclId> {
        let picked = self.state.types.carrier_drive_impls(decl).first().copied()?;
        if !matches!(self.state.design.decl_data(picked).kind, DeclKind::GenericParam(_)) {
            return Some(picked);
        }
        let driven = &self.state.types.resolve_impls(picked).first()?.driven;
        self.state.types.resolve_impls(nature).iter().find(|candidate| candidate.driven == *driven).map(|candidate| candidate.implementation)
    }

    /// The values the params a frame's initial values may read take: the body's key params for the body's
    /// own module (frame 0, `body_keys`), the constants the placement bound for a hoisted module.
    pub(in crate::elab) fn frame_bindings(&self, frame: usize) -> Bindings {
        if frame == 0 {
            return self.state.structure.body_keys.clone();
        }
        let mut bindings = Bindings::new();
        for (param, value) in &self.state.structure.frames[frame].params {
            self.bind_param(&mut bindings, *param, *value);
        }
        bindings
    }

    /// The ordinals of the iteration `ordinals` the code being lowered sits in, for the generation `for`s around `decl` it is
    /// inside (a prefix of them, outermost first).
    pub(in crate::elab) fn implicit_ordinals(&self, decl: DeclId, ordinals: &[(DeclId, u32)]) -> Vec<u64> {
        self.generation_loops(decl).iter().map_while(|generation| ordinals.iter().find(|(held, _)| held == generation).map(|(_, ordinal)| u64::from(*ordinal))).collect()
    }

    /// The module `call` places when that module is hoisted: one with a result or with a value input. A call through a param of
    /// a generic type bounded by `Fn(..)` (`s(5.0)` with `s: C`) places the module of the template the param holds in the frame
    /// being lowered.
    pub(in crate::elab) fn hoisted_callee(&self, call: ExprId) -> Option<DeclId> {
        self.placed_module(call, self.state.consts.frame_params()).filter(|m| self.is_hoisted(*m))
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier
    /// pattern, the matching component of a tuple pattern over a tuple expression.
    pub(in crate::elab) fn body_let_value(&self, binding: DeclId) -> Option<ExprId> {
        crate::elab::consts::ConstEval::body_let_value(self.state.design, binding)
    }

    /// The generation `for`s around `decl`, outermost first.
    pub(in crate::elab) fn generation_loops(&self, decl: DeclId) -> Vec<DeclId> {
        let mut loops = Vec::new();
        let mut at = self.state.design.decl_data(decl).parent;
        while let Some(current) = at {
            let data = self.state.design.decl_data(current);
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

    /// The module the placement `call` places: the one the checker names, else (a call of a callable param, `cell(a)` with
    /// `cell: C` and `C: Fn(..)`) the module of the template the param holds in `keys`. `None` for a call that places nothing, or a
    /// param no template is bound to.
    pub(in crate::elab) fn placed_module(&self, call: ExprId, keys: &Bindings) -> Option<DeclId> {
        if let Some(Callee::Module(module)) = self.state.types.callee(call) {
            return Some(module);
        }
        let ExprKind::Call { class: CallClass::Instance { module: None, via: Some(via) }, .. } = &self.state.design.expr_node(call).kind else { return None };
        if !self.is_module_param(*via) {
            return None;
        }
        keys.get(*via).and_then(|number| self.state.consts.templates().get(number)).map(|template| template.module)
    }

    /// Whether a placement of `module` is hoisted into the body that places it (Appendix C.1.1): it
    /// declares a result or takes a value input. Every other module keeps its own body.
    pub(in crate::elab) fn is_hoisted(&self, module: DeclId) -> bool {
        let data = self.state.design.decl_data(module);
        matches!(&data.kind, DeclKind::Module(m) if m.ret.is_some()) || data.children.iter().any(|c| self.is_value_input(*c))
    }

    /// Whether `param` is declared with a module type, or with a type parameter bounded by `Fn(..)`: its value is a template.
    pub(super) fn is_module_param(&self, param: DeclId) -> bool {
        self.template_array_length(param).is_none()
            && matches!(&self.state.design.decl_data(param).kind, DeclKind::Param(p) if matches!(self.state.design.type_resolution(p.ty), Resolution::Decl(m) if matches!(self.state.design.decl_data(m).kind, DeclKind::Module(_)) || crate::types::GenericSignature::is_callable(self.state.design, m)))
    }

    /// Whether `decl` is a port of a value type: a value input.
    pub(in crate::elab) fn is_value_input(&self, decl: DeclId) -> bool {
        super::structure::structural::ParamReads::new(self.state.design, self.state.types).is_value_input(decl)
    }

    /// The length of the array of templates `param` is declared as, `None` for a param of any other type.
    pub(super) fn template_array_length(&self, param: DeclId) -> Option<usize> {
        let DeclKind::Param(p) = &self.state.design.decl_data(param).kind else { return None };
        match self.typed_written(p.ty)? {
            crate::types::Ty::Array(element, crate::types::Length::Fixed(length)) if matches!(*element, crate::types::Ty::Module(_)) => usize::try_from(length).ok(),
            _ => None,
        }
    }

    /// `value`, a param's constant, bound to `param` in `bindings`; a composite is also bound taken
    /// apart, for the evaluator that reads it (a `where`, a value over the param).
    pub(in crate::elab) fn bind_param(&self, bindings: &mut Bindings, param: crate::pom::DeclId, value: Constant) {
        match value {
            Constant::Aggregate(_) => bindings.bind_composite(param, value, self.imported_tree(value)),
            other => bindings.bind(param, other),
        }
    }

    /// The constant `value` of the IR's arena as the tree the evaluator reads: the reverse of
    /// `import_tree`. A unit variant of an enum with no field is its tag.
    pub(in crate::elab) fn imported_tree(&self, value: Constant) -> ConstTree {
        let Constant::Aggregate(id) = value else { return ConstTree::Scalar(value) };
        let data = self.state.builder.aggregate_at(id);
        let parts: Vec<ConstTree> = data.parts.iter().map(|part| self.imported_tree(*part)).collect();
        let kind = match (&data.ty, data.variant) {
            (ValueType::Tuple(_), _) => CompositeKind::Tuple,
            (ValueType::Array(..), _) => CompositeKind::Array,
            (ValueType::Option(_), _) => CompositeKind::Option,
            (ValueType::Enum(type_id), Some(position)) => {
                if self.state.values.type_arena.is_unit_only(*type_id) {
                    return ConstTree::Scalar(Constant::Int(i128::from(position)));
                }
                CompositeKind::Variant(position)
            }
            _ => CompositeKind::Struct,
        };
        ConstTree::Composite { kind, parts }
    }

    /// The text that tells the value `value` of a key param apart from every other, by what it holds: a composite is written out
    /// by its parts, so two equal composites (two equal arrays of templates) have one text and select one body variant, where the
    /// arena id of the aggregate (`agg2`) would give each its own.
    pub(in crate::elab) fn key_text(&self, value: Constant) -> String {
        Self::tree_text(&self.imported_tree(value))
    }

    /// `tree` written out: a scalar as its value, a composite as its parts in the brackets of its kind.
    fn tree_text(tree: &ConstTree) -> String {
        let (kind, parts) = match tree {
            ConstTree::Scalar(value) => return value.to_string(),
            ConstTree::Composite { kind, parts } => (kind, parts),
        };
        let written = parts.iter().map(Self::tree_text).collect::<Vec<_>>().join(",");
        match kind {
            CompositeKind::Tuple => format!("({written})"),
            CompositeKind::Array => format!("[{written}]"),
            CompositeKind::Struct => format!("{{{written}}}"),
            CompositeKind::Option => format!("option({written})"),
            CompositeKind::Variant(position) => format!("v{position}({written})"),
            CompositeKind::Range { .. } => format!("range({written})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{InstanceId, IrBuilder, ValueType};

    use super::*;
    use crate::elab::digital::carriers::Frame;

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
