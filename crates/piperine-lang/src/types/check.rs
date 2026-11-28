//! The declaration worklist: from the top module, every module it places, every function it
//! calls and every constant it reads is checked once, in first-reach order.

use std::collections::{BTreeSet, VecDeque};

use crate::refusal::{Refusal, RefusedForm};
use crate::msl::{Diagnostic, Span};
use crate::pom::{Decl, DeclId, DeclKind, Design, DesignDiagnostic, ExprId, FileId, Reflect};

use super::attributes::AttributeCatalogue;
use super::generics::tables::GenericsTables;
use super::analog::tables::AnalogTables;
use super::carrier::tables::CarrierTables;
use super::module::tables::ModuleTables;
use super::value::tables::ValueTables;
use super::value::ExpressionTyping;
use super::carrier::CarrierTyping;
use super::analog::AnalogTyping;
use super::module::ModuleTyping;
use super::traits::TraitTyping;
use super::generics::GenericsSolver;
use super::pattern::PatternTyping;
use super::{Ty, Types};

/// Where the expression being checked is written: the §14 table allows a core primitive by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    /// A module body.
    Module,
    /// An `analog` block.
    Analog,
    /// A reaction body (`on ev { .. }`).
    Reaction,
    /// A `fn` or a closure.
    Function,
    /// A const context (§8.1.21): a param default, a `where`, a `const` initializer.
    Const,
}



/// One pass over the reachable set: the state it fills and the declarations it still has to check. Everything it learns
/// goes into the tables of [`CheckState`]; the design is only read.
pub(super) struct Checker<'d> {
    /// The design, the diagnostics, and one table per concern.
    pub(super) state: CheckState<'d>,
    /// The declarations reached and not checked yet.
    worklist: DeclarationWorklist,
}

/// Everything one check holds, one table per concern: what a form of one concern learns is read from that concern's
/// table, and what the elaborator reads afterwards is the result.
pub(super) struct CheckState<'d> {
    /// The design being checked, only read.
    pub(super) design: &'d Design,
    /// What the check has reported so far.
    pub(super) diagnostics: Diagnostics,
    /// The state of value expressions (§1.5, §3): the declaration and the place being checked, the control flow, the
    /// locals, closures, structs and enums.
    pub(super) values: ValueTables,
    /// The state of module bodies (§8): `let`s, placements, reads through templates, node choices, records.
    pub(super) modules: ModuleTables,
    /// The state of analog behaviour (§14): continuity and derivatives.
    pub(super) analog: AnalogTables,
    /// The state of discrete carriers (§12, §13): what the module drives and the definitions being checked.
    pub(super) carriers: CarrierTables,
    /// The state of generic items (§3.8): the item and the substitutions under which signatures are typed.
    pub(super) generics: GenericsTables,
    /// The traits and `impl`s of the design (§3.7), built once.
    pub(super) traits: std::rc::Rc<super::ImplTable<'d>>,
    /// The tables the elaborator reads once the check is done: types, callees, layouts, instantiations.
    pub(super) result: Types<'d>,
}

/// The reached declarations, each checked once, in first-reach order.
#[derive(Default)]
struct DeclarationWorklist {
    /// The declarations already put on the queue.
    queued: BTreeSet<DeclId>,
    /// The declarations already checked, so one reached ahead of the queue (`check_ahead`) is not
    /// checked twice.
    checked: BTreeSet<DeclId>,
    /// The declarations reached and waiting to be checked.
    queue: VecDeque<DeclId>,
}

/// The diagnostics a check reports, one per construct.
#[derive(Default)]
pub(super) struct Diagnostics {
    /// The diagnostics reported so far, in report order.
    pub(super) list: Vec<DesignDiagnostic>,
    /// The spans already reported: one diagnostic per construct.
    pub(super) reported_spans: BTreeSet<(FileId, usize, usize)>,
}

impl<'d> CheckState<'d> {
    /// The empty state of a check of `design`.
    fn new(design: &'d Design) -> Self {
        CheckState {
            design,
            diagnostics: Diagnostics::default(),
            values: ValueTables::default(),
            modules: ModuleTables::default(),
            analog: AnalogTables::new(design),
            carriers: CarrierTables::default(),
            generics: GenericsTables::default(),
            traits: std::rc::Rc::new(super::ImplTable::new(design)),
            result: Types::empty(design),
        }
    }
}

impl<'d> Checker<'d> {
    /// A check of `design` with nothing reached yet.
    pub(super) fn new(design: &'d Design) -> Self {
        Checker { state: CheckState::new(design), worklist: DeclarationWorklist::default() }
    }

    /// The view that types value expressions.
    pub(super) fn expressions(&mut self) -> ExpressionTyping<'_, 'd> {
        ExpressionTyping::new(self)
    }

    /// The view that types patterns.
    pub(super) fn patterns(&mut self) -> PatternTyping<'_, 'd> {
        PatternTyping::new(self)
    }

    /// The view that solves generic items.
    pub(super) fn generics(&mut self) -> GenericsSolver<'_, 'd> {
        GenericsSolver::new(self)
    }

    /// The view that judges traits and `impl`s.
    pub(super) fn trait_typing(&mut self) -> TraitTyping<'_, 'd> {
        TraitTyping::new(self)
    }

    /// The view that types module bodies.
    pub(super) fn modules(&mut self) -> ModuleTyping<'_, 'd> {
        ModuleTyping::new(self)
    }

    /// The view that types analog behaviour.
    pub(super) fn analog(&mut self) -> AnalogTyping<'_, 'd> {
        AnalogTyping::new(self)
    }

    /// The view that types discrete carriers.
    pub(super) fn carriers(&mut self) -> CarrierTyping<'_, 'd> {
        CarrierTyping::new(self)
    }

    /// Whether the project itself defines a method called `name`, in an `impl` or a `trait`.
    pub(super) fn project_defines(&self, name: &str) -> bool {
        self.state.traits.defines_method(name, |decl| self.is_std(decl))
    }

    /// Check `top` and everything it reaches: the table, or the diagnostics in source order.
    pub(super) fn run(mut self, top: DeclId) -> Result<Types<'d>, Vec<DesignDiagnostic>> {
        for misplaced in AttributeCatalogue::misplaced(self.state.design) {
            self.coded(misplaced.file, "E0904", misplaced.span, misplaced.message, "an attribute A.1 does not allow here");
        }
        self.reach(top);
        self.drain();
        self.modules().report_unread_template_locals();
        self.modules().judge_node_choices();
        if !self.state.diagnostics.list.is_empty() {
            return Err(self.sorted_diagnostics());
        }
        let typed: Vec<ExprId> = (0..self.state.result.exprs.len()).map(|i| ExprId(i as u32)).filter(|e| self.state.result.exprs[e.index()].is_some()).collect();
        for expr in typed {
            self.analog().continuous(expr);
        }
        let continuous: BTreeSet<ExprId> = self.state.analog.continuous_set().collect();
        let emits = self.state.modules.emitting();
        let mut result = self.state.result;
        result.continuous = continuous;
        result.emits = emits;
        result.assigned = self.state.values.into_assigned();
        Ok(result)
    }

    /// Check every declaration of `items`, and what they reach, and keep only the diagnostics, in
    /// source order: the whole-model check (`whole_model.rs`), which builds no table.
    pub(super) fn run_model(mut self, items: &[DeclId]) -> Vec<DesignDiagnostic> {
        for item in items {
            self.reach(*item);
        }
        self.drain();
        self.sorted_diagnostics()
    }

    /// Check the worklist until it is empty, then the conflicts between definitions.
    fn drain(&mut self) {
        while let Some(decl) = self.worklist.queue.pop_front() {
            self.declaration(decl);
        }
        self.modules().record_conflicts();
    }

    /// The diagnostics so far, in source order.
    fn sorted_diagnostics(self) -> Vec<DesignDiagnostic> {
        let mut diagnostics = self.state.diagnostics.list;
        diagnostics.sort_by_key(|d| (d.file, d.diagnostic.span().start));
        diagnostics
    }

    /// Run `body` with `context` as the place being checked, then restore the one outside.
    pub(super) fn within<R>(&mut self, context: Context, body: impl FnOnce(&mut Self) -> R) -> R {
        let outer = self.state.values.replace_context(context);
        let result = body(self);
        self.state.values.set_context(outer);
        result
    }

    /// Put a declaration on the worklist the first time it is reached.
    pub(super) fn reach(&mut self, decl: DeclId) {
        if self.worklist.queued.insert(decl) {
            self.state.result.reached.push(decl);
            self.worklist.queue.push_back(decl);
            // a function of an `impl` brings its `impl` along: the declaration rules of the block and the types it writes
            // (the arguments of its trait) are judged once, wherever a function of it is first reached
            if let Some(owner) = self.state.design.decl_data(decl).parent.filter(|owner| matches!(self.state.design.decl_data(*owner).kind, DeclKind::Impl(_))) {
                self.reach(owner);
            }
        }
    }

    /// Check `module` now, out of queue order, when it is not checked yet: a read of a child's
    /// `let` needs the type of its value (`reach/fields.rs`). The state of the declaration being checked
    /// is set aside and restored; a module already checked (or being checked) is left alone.
    pub(super) fn check_ahead(&mut self, module: DeclId) {
        if self.worklist.checked.contains(&module) {
            return;
        }
        self.reach(module);
        let (carriers, modules) = (self.state.carriers.save(), self.state.modules.save());
        let saved = self.state.values.save();
        self.declaration(module);
        self.state.values.restore(saved);
        self.state.carriers.restore(carriers);
        self.state.modules.restore(modules);
    }

    /// Check one declaration of the worklist, once: a module, a function, a constant or an `impl`; anything else is refused.
    fn declaration(&mut self, decl: DeclId) {
        if !self.worklist.checked.insert(decl) {
            return;
        }
        self.state.values.begin_declaration(decl);
        match &self.state.design.decl_data(decl).kind {
            DeclKind::Module(_) => self.modules().module(decl),
            DeclKind::Fn(_) => self.expressions().function(decl),
            DeclKind::Const(_) => self.expressions().constant(decl),
            DeclKind::Impl(_) => self.trait_typing().coherence(decl),
            other => self.refuse_declaration(decl, RefusedForm::item(other)),
        }
        self.expressions().report_uncalled_closures();
    }

    /// `P0001` at the declaration `decl`, naming `form`.
    fn refuse_declaration(&mut self, decl: DeclId, form: RefusedForm) {
        let data = self.state.design.decl_data(decl);
        let (file, span) = (data.file, data.span);
        self.refuse(file, span, form);
    }

    /// Whether `decl` is written in the standard library.
    pub(super) fn is_std(&self, decl: DeclId) -> bool {
        Decl::new(self.state.design, decl).krate().is_std()
    }

    /// The file the expression `expr` is written in.
    pub(super) fn file_of_expr(&self, expr: ExprId) -> FileId {
        self.state.design.decl_data(self.state.design.expr_node(expr).owner).file
    }

    /// One diagnostic per construct: [`Diagnostics::report`], asked of the checker by every concern.
    pub(super) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        self.state.diagnostics.report(file, diagnostic);
    }

    /// A diagnostic of a rule of the specification that carries its own code: [`Diagnostics::coded`].
    pub(super) fn coded(&mut self, file: FileId, code: &'static str, span: Span, message: String, label: &str) {
        self.state.diagnostics.coded(file, code, span, message, label);
    }

    /// `P0001` at `span`, naming `form`: [`Diagnostics::refuse`].
    pub(super) fn refuse(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        self.state.diagnostics.refuse(file, span, form);
    }

    /// `P0001` at `span`, naming `form` with `details`: [`Diagnostics::refuse_with`].
    pub(super) fn refuse_with(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>, details: impl IntoIterator<Item = String>) {
        self.state.diagnostics.refuse_with(file, span, form, details);
    }

    /// `E0180`: a plain type mismatch at `span`, naming both types.
    pub(super) fn mismatch(&mut self, file: FileId, span: Span, expected: &Ty, found: &Ty) {
        let (expected, found) = (expected.name(self.state.design), found.name(self.state.design));
        self.mismatch_text(file, span, &expected, &found);
    }

    /// `E0180` with the two types already written out.
    pub(super) fn mismatch_text(&mut self, file: FileId, span: Span, expected: &str, found: &str) {
        self.mismatch_coded(file, span, "E0180", expected, found);
    }

    /// A mismatch under the code of the rule that was broken; a read through an untyped template is the module concern's to
    /// report, so it is asked first.
    pub(super) fn mismatch_coded(&mut self, file: FileId, span: Span, code: &'static str, expected: &str, found: &str) {
        if self.modules().untyped_template_read(file, span, found) {
            return;
        }
        self.report(file, Diagnostic::new(code, format!("mismatched types: expected `{expected}`, found `{found}`"), span, format!("expected `{expected}`, found `{found}`")));
    }
}

impl Diagnostics {
    /// How many diagnostics are reported so far: a mark for [`Diagnostics::annotate_since`], and what a form compares before
    /// and after typing a part to know whether that part was refused.
    pub(super) fn count(&self) -> usize {
        self.list.len()
    }

    /// Appends `note` to the message of each diagnostic coded `code` reported after `mark`.
    pub(super) fn annotate_since(&mut self, mark: usize, code: &str, note: &str) {
        for reported in self.list[mark..].iter_mut().filter(|reported| reported.diagnostic.code == code) {
            reported.diagnostic.message.push_str(note);
        }
    }

    /// One diagnostic per construct: a second one at the same span is dropped.
    pub(super) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        let span = diagnostic.span();
        if self.reported_spans.insert((file, span.start, span.end)) {
            self.list.push(DesignDiagnostic { file, diagnostic });
        }
    }

    /// Reports `diagnostic` whatever else is already reported at its span: a finding about a relation between constructs (a
    /// loop of definitions) is not the same finding as one about the construct at the span where it is shown.
    pub(super) fn report_distinct(&mut self, file: FileId, diagnostic: Diagnostic) {
        self.list.push(DesignDiagnostic { file, diagnostic });
    }

    /// A diagnostic of a rule of the specification that carries its own code.
    pub(super) fn coded(&mut self, file: FileId, code: &'static str, span: Span, message: String, label: &str) {
        self.report(file, Diagnostic::new(code, message, span, label));
    }

    /// `P0001` at `span`, naming `form`: a construct outside the supported subset.
    pub(super) fn refuse(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        self.report(file, Refusal::of(form).diagnostic(span));
    }

    /// `P0001` at `span`, naming `form` with `details` for the `{}` of its words.
    pub(super) fn refuse_with(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>, details: impl IntoIterator<Item = String>) {
        let refusal = details.into_iter().fold(Refusal::of(form), Refusal::with_detail);
        self.report(file, refusal.diagnostic(span));
    }
}
