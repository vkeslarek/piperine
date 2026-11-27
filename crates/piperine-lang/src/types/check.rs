//! The declaration worklist: from the top module, every module it places, every function it
//! calls and every constant it reads is checked once, in first-reach order.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::refusal::forms::ValueForm;
use crate::refusal::{Refusal, RefusedForm};
use crate::msl::{Diagnostic, Span};
use crate::pom::{BehaviorKind, BlockNode, Decl, DeclData, DeclId, DeclKind, Design, DesignDiagnostic, ExprId, FileId, FnParamData, GenericArgNode, GenericArgsNode, PathId, Reflect, Resolution, TypeId, TypeKind};

use super::continuity::Continuity;
use super::class::FunctionClass;
use super::closures::ClosureInfo;
use super::definition::DriveBook;
use super::assign::Places;
use super::flow::{ExitFrame, Flow};
use super::nature_rules::NatureRules;
use super::record::Records;
use super::{ConstTerm, Ty, Types};

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

/// The order a module body is checked in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemPhase {
    /// Ports, params and signals.
    Declarations,
    /// Named expressions, which read the declarations.
    Lets,
    /// Definitions, which read both.
    Definitions,
}

/// The parts of a function signature, each typed on its own.
pub(super) struct SignatureParts {
    params: Vec<(DeclId, Option<Ty>)>,
    ret: Option<Ty>,
    ok: bool,
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

/// What value expressions (§1.5, §3) are typed under, and what typing them has learnt that the result does not keep.
pub(super) struct ValueTables {
    /// The declaration being checked: a module, a function or a constant.
    pub(super) owner: Option<DeclId>,
    /// Where the expression being checked is written.
    pub(super) context: Context,
    /// Set while the `where` of a `fn` is checked: its parameters are not in scope (§3 rule 8).
    pub(super) fn_where: bool,
    /// The one expression that may be an `Event`: an event position hands it down just before it
    /// types the expression, which judges an `Event` anywhere else.
    pub(super) event_slot: Option<ExprId>,
    /// Set while the guard of an event, the right operand of `ev && guard`, is typed.
    pub(super) in_guard: bool,
    /// Bindings whose value could not be typed: a read of one is silent, so one mistake is
    /// one diagnostic.
    pub(super) poisoned: BTreeSet<DeclId>,
    /// The initializer of each `let` with a name, which says whether the name's value depends on the run.
    pub(super) let_values: BTreeMap<DeclId, ExprId>,
    /// Which names are places and which locals the body assigns (§1.5.12).
    pub(super) places: Places,
    /// The locals bound to a bare integer literal whose type no use has fixed yet (§1.4.4), each
    /// with the literal expression: `let mut total = 0;` waits for `total += k`.
    pub(super) open_literals: BTreeMap<DeclId, ExprId>,
    /// The locals declared without a value (`let x;`, §1.7.5).
    pub(super) deferred: BTreeSet<DeclId>,
    /// The control state of the body being checked.
    pub(super) flow: Flow,
    /// The captures and the function class of each closure typed so far.
    pub(super) closures: BTreeMap<ExprId, ClosureInfo>,
    /// The closure each local is bound to by `let` (the local's value is that closure).
    pub(super) closure_locals: BTreeMap<DeclId, ExprId>,
    /// The locals bound to a closure whose parameters wait for the first call to be typed.
    pub(super) pending_closures: BTreeMap<DeclId, ExprId>,
    /// The function class of each function whose class is known.
    pub(super) classes: BTreeMap<DeclId, FunctionClass>,
    /// The type the context expects of one call, handed down by `check_as` just before the call
    /// is typed: the result type of a call that is generic over its result (`logic`, `cast`).
    pub(super) call_expected: Option<(ExprId, Ty)>,
    /// The enums whose variants are typed (`enums.rs`).
    pub(super) declared: BTreeSet<DeclId>,
    /// The structs and enums whose fields are being typed: meeting one again is a type that
    /// contains itself (§1.3.8).
    pub(super) declaring: BTreeSet<DeclId>,
    /// The structs and enums that could not be typed: a use is silent, the cause is reported.
    pub(super) broken: BTreeSet<DeclId>,
}

impl Default for ValueTables {
    fn default() -> Self {
        ValueTables {
            owner: None,
            context: Context::Module,
            fn_where: false,
            event_slot: None,
            in_guard: false,
            poisoned: BTreeSet::new(),
            let_values: BTreeMap::new(),
            places: Places::default(),
            open_literals: BTreeMap::new(),
            deferred: BTreeSet::new(),
            flow: Flow::default(),
            closures: BTreeMap::new(),
            closure_locals: BTreeMap::new(),
            pending_closures: BTreeMap::new(),
            classes: BTreeMap::new(),
            call_expected: None,
            declared: BTreeSet::new(),
            declaring: BTreeSet::new(),
            broken: BTreeSet::new(),
        }
    }
}

/// What checking module bodies (§8) is in the middle of: the `let`s being typed, the placements being connected, the
/// reads through templates, the node choices left to judge, and the `emit!` sites.
#[derive(Default)]
pub(super) struct ModuleTables {
    /// The bindings of the module-body `let`s whose values are being typed, each with its value (a
    /// read of one is a feedback, `Checker::self_read`).
    pub(super) lets_open: BTreeMap<DeclId, ExprId>,
    /// The `let`s of module bodies already typed: a `let` is typed where it is first read, whatever its place in the body
    /// (§8.2: the items of a body appear in any order), and once.
    pub(super) lets_typed: BTreeSet<DeclId>,
    /// The placements whose arguments are being typed, innermost last: the module, or `None` for
    /// a core module.
    pub(super) placing: Vec<Option<DeclId>>,
    /// The reads of an item of a child only a folded template names (`reach/fields.rs`), typed as a node until a context wants a value
    /// of them (`template_items.rs`).
    pub(super) template_reads: BTreeSet<ExprId>,
    /// The reads through a template made through an instance whose `let` was being typed, to that `let`: a node connection, and
    /// a read of the `let` itself when a context wants a value of it (§8.2.16).
    pub(super) open_template_reads: BTreeMap<ExprId, DeclId>,
    /// The `let`s with no type bound to a read through a template, by binding: the first use that wants a value types them.
    pub(super) template_locals: BTreeMap<DeclId, ExprId>,
    /// The `if`s, `match`es and blocks refused for choosing a node (`E0133`, `handles.rs`): an operand that is one is no second
    /// diagnostic.
    pub(super) chosen_nodes: BTreeSet<ExprId>,
    /// The `if`s and `match`es whose result is a node, judged once the whole design is typed (`judge_node_choices`).
    pub(super) node_choices: Vec<ExprId>,
    /// Whether a bundle `struct` is being named as the type of a handle (`bundle_handles.rs`), where it is no refusal.
    pub(super) naming_bundle: bool,
    /// The `emit!` sites seen so far.
    pub(super) records: Records,
}

/// What checking analog behaviour (§14) keeps: which expressions are continuous, and how deep in a `ddx` the check is.
pub(super) struct AnalogTables {
    /// What the check knows of continuity, filled as it goes (`continuity.rs`).
    pub(super) continuity: Continuity,
    /// How many `ddx` values are being typed: a flow probe of a branch with no potential contribution may be read there,
    /// where it is a leaf the derivative reads symbolically (§14.2.3), though it is no value elsewhere (`probe`).
    pub(super) differentiated: u32,
}

/// What checking discrete carriers (§12, §13) keeps: what the module being checked drives, and how many definitions
/// the check is inside.
#[derive(Default)]
pub(super) struct CarrierTables {
    /// What the module being checked drives, by definition and by commit.
    pub(super) book: DriveBook,
    /// How many definitions the check is inside: their values are not read from the solve (`classes.rs`).
    pub(super) in_definition: u32,
}

/// What typing generic items (§3.8) is under: the generic item being checked and the substitutions in force.
#[derive(Default)]
pub(super) struct GenericsTables {
    /// The generic function whose definition is being checked or whose signature is being typed:
    /// what a definition defers is recorded under it.
    pub(super) generic_item: Option<DeclId>,
    /// The arguments of the generic module whose placement is being connected: the ports are typed under it (GN-13).
    pub(super) placement_substitution: Option<super::Substitution>,
    /// The arguments a bound writes for its trait, set for the call through it being typed (`bounds.rs`).
    pub(super) bound_substitution: Option<super::Substitution>,
}

impl<'d> CheckState<'d> {
    /// The empty state of a check of `design`.
    fn new(design: &'d Design) -> Self {
        CheckState {
            design,
            diagnostics: Diagnostics::default(),
            values: ValueTables::default(),
            modules: ModuleTables::default(),
            analog: AnalogTables { continuity: Continuity::new(design), differentiated: 0 },
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

    /// Whether the project itself defines a method called `name`, in an `impl` or a `trait`.
    pub(super) fn project_defines(&self, name: &str) -> bool {
        self.state.traits.defines_method(name, |decl| self.is_std(decl))
    }

    /// Check `top` and everything it reaches: the table, or the diagnostics in source order.
    pub(super) fn run(mut self, top: DeclId) -> Result<Types<'d>, Vec<DesignDiagnostic>> {
        self.check_attributes();
        self.reach(top);
        self.drain();
        self.report_unread_template_locals();
        self.judge_node_choices();
        if !self.state.diagnostics.list.is_empty() {
            return Err(self.sorted_diagnostics());
        }
        let typed: Vec<ExprId> = (0..self.state.result.exprs.len()).map(|i| ExprId(i as u32)).filter(|e| self.state.result.exprs[e.index()].is_some()).collect();
        for expr in typed {
            self.continuous(expr);
        }
        let continuous: BTreeSet<ExprId> = self.state.analog.continuity.continuous_set().collect();
        let emits = self.state.modules.records.emitting();
        let mut result = self.state.result;
        result.continuous = continuous;
        result.emits = emits;
        result.assigned = self.state.values.places.into_assigned();
        Ok(result)
    }

    /// Check every declaration of `items`, and what they reach, and keep only the diagnostics, in
    /// source order: the whole-model check (`model.rs`), which builds no table.
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
        self.record_conflicts();
    }

    /// The diagnostics so far, in source order.
    fn sorted_diagnostics(self) -> Vec<DesignDiagnostic> {
        let mut diagnostics = self.state.diagnostics.list;
        diagnostics.sort_by_key(|d| (d.file, d.diagnostic.span().start));
        diagnostics
    }

    /// Run `body` with `context` as the place being checked, then restore the one outside.
    pub(super) fn within<R>(&mut self, context: Context, body: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.state.values.context, context);
        let result = body(self);
        self.state.values.context = outer;
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
        let (owner, context, event_slot, in_guard, fn_where) = (self.state.values.owner, self.state.values.context, self.state.values.event_slot, self.state.values.in_guard, self.state.values.fn_where);
        let (book, lets_open, placing, flow) = (std::mem::take(&mut self.state.carriers.book), std::mem::take(&mut self.state.modules.lets_open), std::mem::take(&mut self.state.modules.placing), std::mem::take(&mut self.state.values.flow));
        self.state.values.context = Context::Module;
        self.state.values.event_slot = None;
        self.state.values.in_guard = false;
        self.state.values.fn_where = false;
        self.declaration(module);
        (self.state.values.owner, self.state.values.context, self.state.values.event_slot, self.state.values.in_guard, self.state.values.fn_where) = (owner, context, event_slot, in_guard, fn_where);
        (self.state.carriers.book, self.state.modules.lets_open, self.state.modules.placing, self.state.values.flow) = (book, lets_open, placing, flow);
    }

    fn declaration(&mut self, decl: DeclId) {
        if !self.worklist.checked.insert(decl) {
            return;
        }
        self.state.values.owner = Some(decl);
        self.state.values.flow = Flow::default();
        match &self.state.design.decl_data(decl).kind {
            DeclKind::Module(_) => self.module(decl),
            DeclKind::Fn(_) => self.function(decl),
            DeclKind::Const(_) => self.constant(decl),
            DeclKind::Impl(_) => self.coherence(decl),
            other => self.refuse_declaration(decl, RefusedForm::item(other)),
        }
        self.report_uncalled_closures();
    }

    // ---- reporting -------------------------------------------------------------------------

    /// How many diagnostics are reported so far: a mark for [`Checker::annotate_since`].
    pub(super) fn diagnostics_mark(&self) -> usize {
        self.state.diagnostics.list.len()
    }

    /// Appends `note` to the message of each diagnostic coded `code` reported after `mark`.
    pub(super) fn annotate_since(&mut self, mark: usize, code: &str, note: &str) {
        for reported in self.state.diagnostics.list[mark..].iter_mut().filter(|reported| reported.diagnostic.code == code) {
            reported.diagnostic.message.push_str(note);
        }
    }

    /// One diagnostic per construct: a second one at the same span is dropped.
    pub(super) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        let span = diagnostic.span();
        if self.state.diagnostics.reported_spans.insert((file, span.start, span.end)) {
            self.state.diagnostics.list.push(DesignDiagnostic { file, diagnostic });
        }
    }

    /// Reports `diagnostic` whatever else is already reported at its span: a finding about a
    /// relation between constructs (a loop of definitions) is not the same finding as one about
    /// the construct at the span where it is shown.
    pub(super) fn report_distinct(&mut self, file: FileId, diagnostic: Diagnostic) {
        self.state.diagnostics.list.push(DesignDiagnostic { file, diagnostic });
    }

    pub(super) fn file_of_expr(&self, expr: ExprId) -> FileId {
        self.state.design.decl_data(self.state.design.expr_node(expr).owner).file
    }

    /// How many diagnostics the check has reported so far: a form compares the count before and after typing a part to know
    /// whether that part was refused.
    pub(super) fn reported(&self) -> usize {
        self.state.diagnostics.list.len()
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

    /// `P0001` at the declaration `decl`, naming `form`.
    fn refuse_declaration(&mut self, decl: DeclId, form: RefusedForm) {
        let data = self.state.design.decl_data(decl);
        let (file, span) = (data.file, data.span);
        self.refuse(file, span, form);
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

    /// A mismatch under the code of the rule that was broken.
    pub(super) fn mismatch_coded(&mut self, file: FileId, span: Span, code: &'static str, expected: &str, found: &str) {
        if self.untyped_template_read(file, span, found) {
            return;
        }
        self.report(file, Diagnostic::new(code, format!("mismatched types: expected `{expected}`, found `{found}`"), span, format!("expected `{expected}`, found `{found}`")));
    }

    // ---- types as written ------------------------------------------------------------------

    /// The type a type node writes, or a `P0001` and `None` for one outside the subset.
    pub(super) fn lower_type(&mut self, ty: TypeId) -> Option<Ty> {
        let lowered = self.lower_type_node(ty);
        if let Some(t) = &lowered {
            self.state.result.written.insert(ty, t.clone());
        }
        lowered
    }

    /// Whether the path of a struct or enum type writes no generic argument where the declaration `decl`
    /// has no generic parameter: `Pair<4>` over a plain `struct Pair` is `P0001` (it would be read as `Pair`,
    /// silently ignoring `4`). A generic declaration is read by `generic_nominal`.
    fn writes_no_arguments_to_a_plain_type(&mut self, path: PathId, decl: DeclId, file: FileId, span: Span) -> bool {
        let written = self.state.design.path_node(path).segments.last().is_some_and(|segment| segment.args.is_some());
        if !written || self.state.design.decl_data(decl).generics_span.is_some() {
            return true;
        }
        let name = self.state.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        self.refuse_with(file, span, ValueForm::GenericArgumentsOnWhichHasNoGeneric, [name.to_string()]);
        false
    }

    fn lower_type_node(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.state.design.type_node(ty).clone();
        let file = self.state.design.decl_data(node.owner).file;
        match &node.kind {
            TypeKind::Path(path) => match self.state.design.type_resolution(ty) {
                Resolution::Builtin(id) => match (id.info().name, id.width()) {
                    ("fN", Some(w)) => Ty::float(u32::from(w)),
                    ("bool", _) => Some(Ty::Bool),
                    // §1.2.9: `str` is a value type, the type of a string.
                    ("str", _) => Some(Ty::Str),
                    ("uN", Some(w)) => self.integer_type(file, node.span, Ty::UInt, u128::from(w), &id.name()),
                    ("iN", Some(w)) => self.integer_type(file, node.span, Ty::Int, u128::from(w), &id.name()),
                    // §1 Resolution: `usize` and `isize` are `UInt<64>` and `Int<64>` everywhere.
                    ("usize", _) => Some(Ty::UInt(64)),
                    ("isize", _) => Some(Ty::Int(64)),
                    (family @ ("UInt" | "Int" | "Float"), _) => self.sized_type(file, node.span, *path, family),
                    ("Option", _) => self.option_type(file, node.span, *path),
                    ("Event", _) => Some(Ty::Event),
                    _ => {
                        self.refuse_with(file, node.span, ValueForm::Type, [id.name().to_string()]);
                        None
                    }
                },
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Nature(_)) && NatureRules::of(self.state.design, d).carries_values => self.discrete_carrier_type(d, file, node.span, *path),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Nature(_)) => Some(Ty::Node(d)),
                // §3.8.5: a generic struct or enum is written with its arguments; its fields are typed where it is used
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && self.state.design.decl_data(d).generics_span.is_some() => self.generic_nominal(*path, d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && !self.writes_no_arguments_to_a_plain_type(*path, d, file, node.span) => None,
                // §3.8.2: a type or nature parameter is a type of the generic body, opaque
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::GenericParam(crate::pom::GenericParamData::Type { .. })) => Some(Ty::Param(d)),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_)) => self.enum_type(d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Struct(_)) => self.struct_type(d, file, node.span),
                // A module named as a type is the type of a param (§8.1.1, PI-04): a template of it; a generic one's names its
                // arguments, `G<2>` (§8.4.3)
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Module(_)) && self.state.design.decl_data(d).generics_span.is_some() => self.generic_module_type(*path, d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Module(_)) => self.writes_no_arguments_to_a_plain_type(*path, d, file, node.span).then(|| Ty::module(d)),
                Resolution::Decl(d) => {
                    let what = RefusedForm::declaration_name(&self.state.design.decl_data(d).kind);
                    self.refuse_with(file, node.span, ValueForm::UsedAsType, [what.to_string()]);
                    None
                }
                _ => {
                    self.refuse(file, node.span, ValueForm::ThisType);
                    None
                }
            },
            TypeKind::Tuple(items) if items.is_empty() => Some(Ty::Unit),
            TypeKind::Tuple(items) => self.tuple_type(items),
            TypeKind::Array { elem, len } => self.array_type(file, node.span, *elem, *len),
            TypeKind::Flip(_) | TypeKind::Infer => {
                self.refuse(file, node.span, RefusedForm::type_form(&node.kind));
                None
            }
            TypeKind::FnPtr { params, ret } => {
                let params: Option<Vec<Ty>> = params.iter().map(|p| self.lower_type(*p)).collect();
                let ret = match ret {
                    Some(r) => self.lower_type(*r),
                    None => Some(Ty::Unit),
                };
                Some(Ty::Fn(params?, Box::new(ret?)))
            }
        }
    }

    /// Whether `decl` is written in the standard library.
    pub(super) fn is_std(&self, decl: DeclId) -> bool {
        Decl::new(self.state.design, decl).krate().is_std()
    }

    /// `UInt<N>`, `Int<N>` or `Float<N>` written with its width argument. The width is an integer
    /// literal; a width computed from a const expression is outside the subset (`P0001`).
    fn sized_type(&mut self, file: FileId, span: Span, path: PathId, family: &str) -> Option<Ty> {
        let args = self.state.design.path_node(path).segments.last().and_then(|s| s.args.clone());
        // a width that names const parameters is an integer of the instantiation's width (`UInt<N>`, `Int<{ N + 1 }>`)
        let term = match &args {
            Some(GenericArgsNode::Angle { args, .. }) if args.len() == 1 => match &args[0] {
                GenericArgNode::Const(expr) => self.const_term(*expr, file),
                GenericArgNode::Type(ty) => self.const_parameter_named_by(*ty).map(ConstTerm::Param),
            },
            _ => None,
        };
        let width = match term {
            Some(ConstTerm::Value(value)) => u128::try_from(value).ok(),
            Some(generic @ (ConstTerm::Param(_) | ConstTerm::Expr(_))) if matches!(family, "UInt" | "Int") => {
                if let ConstTerm::Expr(expr) = &generic {
                    self.defer_constant(*expr);
                }
                return Some(Ty::GenericInteger { signed: family == "Int", width: generic });
            }
            _ => None,
        };
        let Some(width) = width else {
            self.refuse_with(file, span, ValueForm::WithWidthThatIsNotIntegerLiteral, [family.to_string()]);
            return None;
        };
        let written = format!("{family}<{width}>");
        match family {
            "UInt" => self.integer_type(file, span, Ty::UInt, width, &written),
            "Int" => self.integer_type(file, span, Ty::Int, width, &written),
            _ => match u32::try_from(width).ok().and_then(Ty::float) {
                Some(float) => Some(float),
                None => {
                    let message = format!("`{written}`: a float width is one of 16, 32, 64 and 128");
                    self.report(file, Diagnostic::new("E0103", message, span, "float width outside {16, 32, 64, 128}"));
                    None
                }
            },
        }
    }

    /// An integer type of width `width`, written `written`: any width from 1 to 2^32 - 1 (§1 rule 5; the
    /// limit is an implementation limit, Appendix C.2). A width below 1 or past the limit is `E0103`.
    pub(super) fn integer_type(&mut self, file: FileId, span: Span, make: fn(u32) -> Ty, width: u128, written: &str) -> Option<Ty> {
        match width {
            0 => {
                let message = format!("`{written}`: an integer width is a const expression `>= 1`");
                self.report(file, Diagnostic::new("E0103", message, span, "integer width below 1"));
                None
            }
            1..=0xFFFF_FFFF => Some(make(width as u32)),
            _ => {
                let message = format!("`{written}`: an integer width is at most 2^32 - 1 (Appendix C.2)");
                self.report(file, Diagnostic::new("E0103", message, span, "integer width past the implementation limit"));
                None
            }
        }
    }

    // ---- declarations ----------------------------------------------------------------------

    /// `const NAME: T = value;`: typed by its declared type, its initializer checked once.
    fn constant(&mut self, decl: DeclId) {
        let DeclKind::Const(data) = self.state.design.decl_data(decl).kind.clone() else { return };
        let Some(ty) = self.lower_type(data.ty) else { return };
        match data.value {
            Some(value) => self.within(Context::Const, |this| this.check(value, &ty)),
            None => {
                let d = self.state.design.decl_data(decl);
                let (file, span) = (d.file, d.span);
                let name = d.name.as_ref().map_or_else(String::new, |name| name.text.clone());
                let in_trait = d.parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Trait(_)));
                if in_trait {
                    self.refuse(file, span, ValueForm::ConstWithoutValue);
                } else {
                    // §3.9.2: a `const` has a value everywhere but in a trait.
                    self.coded(file, "E0188", span, format!("the `const` `{name}` has no value"), "a `const` outside a trait has a value");
                }
            }
        }
    }

    /// The declared parameter types and result of a function, or `None` after a `P0001`.
    pub(super) fn signature(&mut self, decl: DeclId) -> Option<(Vec<Ty>, Ty)> {
        let data = self.state.design.decl_data(decl);
        if data.generics_span.is_some() {
            // a use of a generic function binds its arguments after inference (T20, T21): until then
            // a call or a function value of one is outside the subset
            let (file, span) = (data.file, data.span);
            self.refuse(file, span, ValueForm::GenericFunction);
            return None;
        }
        let parts = self.signature_parts(decl)?;
        let params: Option<Vec<Ty>> = parts.params.iter().map(|(_, ty)| ty.clone()).collect();
        match (parts.ok, params, parts.ret) {
            (true, Some(params), Some(ret)) => Some((params, ret)),
            _ => None,
        }
    }

    /// The declared parameter types and result of a function whatever its generics: a generic
    /// function's mention its type parameters as `Ty::Param`. `None` after a refusal.
    pub(super) fn declared_signature(&mut self, decl: DeclId) -> Option<(Vec<Ty>, Ty)> {
        let parts = self.signature_parts(decl)?;
        let params: Option<Vec<Ty>> = parts.params.iter().map(|(_, ty)| ty.clone()).collect();
        match (parts.ok, params, parts.ret) {
            (true, Some(params), Some(ret)) => Some((params, ret)),
            _ => None,
        }
    }

    /// The signature piece by piece: each parameter with its type if it has one, the result if
    /// it has one, and whether the signature as a whole is inside the subset. A refused piece
    /// does not stop the others from being typed, so the body can still be checked.
    pub(super) fn signature_parts(&mut self, decl: DeclId) -> Option<SignatureParts> {
        // what the types of a generic signature defer (`[T; N + 1]`) is recorded under the function
        let outer = std::mem::replace(&mut self.state.generics.generic_item, self.state.design.decl_data(decl).generics_span.map(|_| decl));
        let parts = self.signature_parts_of(decl);
        self.state.generics.generic_item = outer;
        parts
    }

    /// [`Checker::signature_parts`] with the generic item set.
    fn signature_parts_of(&mut self, decl: DeclId) -> Option<SignatureParts> {
        let data: DeclData = self.state.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let mut ok = true;
        if data.generics_span.is_some() {
            // the list is held to §3.8; the parameters below are typed with `Ty::Param`, so the body
            // is checked once at its definition (Q1 (c))
            self.generic_signature(decl);
        }
        // a generic `extern "intrinsic"` function (the library's `logic`, `matches_bits`, `parse_table`) is typed through
        // the generic path like any generic function; the other `extern` functions keep their refusal
        if f.abi.is_some() && data.generics_span.is_none() {
            self.refuse(data.file, data.span, ValueForm::ExternFunction);
            ok = false;
        }
        let mut params = Vec::new();
        for child in data.children.iter().copied() {
            let DeclKind::FnParam(p) = &self.state.design.decl_data(child).kind else { continue };
            let ty = match p {
                FnParamData::Named { ty } => self.lower_type(*ty),
                FnParamData::SelfValue => self.self_parameter(decl, child),
                FnParamData::Error => {
                    let d = self.state.design.decl_data(child);
                    self.refuse(d.file, d.span, ValueForm::SelfParameter);
                    None
                }
            };
            ok &= ty.is_some();
            params.push((child, ty));
        }
        let ret = match f.ret {
            Some(r) => self.lower_type(r),
            None => Some(Ty::Unit),
        };
        ok &= ret.is_some();
        Some(SignatureParts { params, ret, ok })
    }

    /// A function of the reachable set: its `where`, then its body against its declared result.
    /// A call to it is typed from the signature alone, so a recursive function never loops. A
    /// signature outside the subset is refused once and the body is still checked, with the
    /// parameters that could not be typed silent.
    fn function(&mut self, decl: DeclId) {
        let outer = std::mem::replace(&mut self.state.generics.generic_item, self.state.design.decl_data(decl).generics_span.map(|_| decl));
        self.function_of(decl);
        self.state.generics.generic_item = outer;
    }

    /// [`Checker::function`] with the generic item set.
    fn function_of(&mut self, decl: DeclId) {
        let Some(parts) = self.signature_parts(decl) else { return };
        let data = self.state.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return };
        // §3 rule 8: the `where` ranges over const generics and associated consts, which the
        // subset has none of. Checked before the parameters are typed, a read of a value
        // parameter in it is refused (`P0001`) and the specification gives a non-`bool`
        // condition no code of its own, so that is the plain mismatch.
        if let Some(cond) = f.where_ {
            if self.state.generics.generic_item == Some(decl) {
                self.defer(super::Obligation::Where { item: decl, condition: cond });
            }
            self.state.values.fn_where = true;
            self.within(Context::Const, |this| this.check(cond, &Ty::Bool));
            self.state.values.fn_where = false;
        }
        for (param, ty) in parts.params {
            match ty {
                // QC3: §3 rule 7 makes a nature parameter a handle and says nothing of an array of them
                Some(array @ Ty::Array(..)) if array.is_carrier_element() => {
                    let (file, span) = (self.state.design.decl_data(param).file, self.state.design.decl_data(param).span);
                    self.refuse(file, span, ValueForm::ArrayOfCarriersAsFunctionParameter);
                    self.state.values.poisoned.insert(param);
                }
                Some(ty) => {
                    self.note_handle(param, &ty);
                    self.state.result.locals.insert(param, ty);
                }
                None => {
                    self.state.values.poisoned.insert(param);
                }
            }
        }
        // before the body: a handle kept is `E0133`, not the mismatch the same span would also report
        self.check_handle_result(decl, parts.ret.as_ref());
        self.check_handle_uses(decl);
        self.state.values.context = Context::Function;
        self.state.values.flow.exit = Some(ExitFrame::new(parts.ret.clone(), false));
        match (&f.body, parts.ret) {
            (Some(body), Some(ret)) => self.function_body(body, &ret, data.file),
            (Some(body), None) => {
                self.statements(&body.stmts, None);
            }
            // a function a trait declares has no body; its implementations do
            (None, _) if data.parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Trait(_))) => {}
            // an `extern` function is implemented outside the language: its signature is all there is to check
            (None, _) if f.abi.is_some() => {}
            (None, _) => self.refuse(data.file, data.span, ValueForm::FunctionWithoutBody),
        }
        self.state.values.context = Context::Module;
    }

    /// The statements of a body against the declared result. A missing result is reported
    /// whatever else the body got wrong, since it is its own mistake.
    fn function_body(&mut self, body: &BlockNode, ret: &Ty, file: FileId) {
        if let Some(found) = self.statements(&body.stmts, Some(ret))
            && !self.compatible_at(file, body.span, &found, ret)
            && found != Ty::Never
        {
            self.mismatch(file, body.span, ret, &found);
        }
    }

    /// Whether `decl`, or a declaration above it, is a generation `for`/`if` (a module body item generated per iteration).
    pub(super) fn inside_generation(&self, decl: DeclId) -> bool {
        let mut at = Some(decl);
        while let Some(current) = at {
            if matches!(self.state.design.decl_data(current).kind, DeclKind::Generation(_)) {
                return true;
            }
            at = self.state.design.decl_data(current).parent;
        }
        false
    }

    /// One item of a module body (or of a generation in it) in `phase`.
    fn module_item(&mut self, child: DeclId, phase: ItemPhase) {
        let item = self.state.design.decl_data(child).clone();
        // a bundle is no value, carrier or node: its fields are the items (`pom/bundles.rs`)
        if self.state.design.is_bundle_owner(child) || self.state.design.is_bundle_group(child) {
            return;
        }
        match (&item.kind, phase) {
            (DeclKind::Port(p), ItemPhase::Declarations) => self.port(child, &p.clone(), item.file),
            (DeclKind::Param(p), ItemPhase::Declarations) => self.param(child, &p.clone()),
            (DeclKind::Signal(s), ItemPhase::Declarations) => self.signal(child, &s.clone()),
            (DeclKind::Let(_), ItemPhase::Lets) => self.type_module_let(child),
            (DeclKind::Definition(d), ItemPhase::Definitions) => self.definition(&d.clone()),
            (DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) | DeclKind::Let(_) | DeclKind::Definition(_), _) => {}
            (DeclKind::GenericParam(_) | DeclKind::Behavior(_) | DeclKind::Error, _) => {}
            // A closure's parameters (`|new, old| ..` of `limexp`, §22.2.4) are children of the
            // module that holds the closure, typed with it, not module items (design D9).
            (DeclKind::Binding(_), _) => {}
            (DeclKind::Generation(_), _) => self.generation(child, phase == ItemPhase::Declarations, |this, inner| this.module_item(inner, phase)),
            (other, ItemPhase::Declarations) => self.refuse_with(item.file, item.span, ValueForm::InsideModule, [other.label().to_lowercase().to_string()]),
            (_, _) => {}
        }
    }

    /// A module of the reachable set: its params, nodes and placements, then its analog blocks.
    /// The modules it places are reached.
    fn module(&mut self, decl: DeclId) {
        let outer = std::mem::replace(&mut self.state.generics.generic_item, self.state.design.decl_data(decl).generics_span.map(|_| decl));
        self.module_of(decl);
        self.state.generics.generic_item = outer;
    }

    /// [`Checker::module`] with the generic item set: a generic module is typed once at its definition, its type and
    /// nature parameters opaque (`Ty::Param`) and its const parameters `ConstTerm::Param` (GN-13, Q1 (c)).
    fn module_of(&mut self, decl: DeclId) {
        let data = self.state.design.decl_data(decl).clone();
        let DeclKind::Module(m) = &data.kind else { return };
        if data.generics_span.is_some() {
            self.generic_signature(decl);
        }
        if m.abi.is_some() {
            self.refuse(data.file, data.span, ValueForm::ExternModule);
        }
        if let Some(cond) = m.where_ {
            // Part III §8 errors: the `where` after the port list that is not a `bool` const expression.
            self.within(Context::Const, |this| this.check_as(cond, &Ty::Bool, "E0304"));
        }
        // Ports, params and signals first, then the `let`s in order, then definitions: every
        // name of the body is visible in the whole body (§8.2), and a carrier's type is known
        // before anything reads it.
        self.state.carriers.book = DriveBook::default();
        // each bundle port or signal is followed by the fields it stands for, typed like ports and signals of their own (§7 rule 14)
        let items = self.state.design.module_items(&data.children);
        for phase in [ItemPhase::Declarations, ItemPhase::Lets, ItemPhase::Definitions] {
            for child in items.iter().copied() {
                self.module_item(child, phase);
            }
        }
        self.definition_loops();
        self.module_result(decl, m);
        for behavior in m.behaviors.iter().copied() {
            let b = self.state.design.decl_data(behavior).clone();
            let DeclKind::Behavior(bd) = &b.kind else { continue };
            match bd.kind {
                BehaviorKind::Analog => {
                    self.state.values.context = Context::Analog;
                    self.statements(&bd.body, None);
                    self.state.values.context = Context::Module;
                }
                BehaviorKind::Digital => self.digital_block(behavior),
            }
        }
        // A placement no statement above typed (one written where only the module lists it).
        for instance in m.instances.iter().copied() {
            // a placement inside generation is typed with generation itself (T72); until then its loop variable has no type
            let in_generation = self.inside_generation(self.state.design.expr_node(instance).owner);
            if self.state.result.exprs[instance.index()].is_none() && !in_generation {
                self.infer(instance);
            }
        }
        // the carriers declared inside generation are the module's too, one array each (§8.2.23)
        let carriers = self.generated_items(&items);
        self.missing_initial_values(&carriers);
        self.initial_values_under_element_definitions(&carriers);
        self.defer_unfolded_overlaps();
        self.handed_drives();
    }
}
