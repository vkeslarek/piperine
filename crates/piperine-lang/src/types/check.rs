//! The declaration worklist: from the top module, every module it places, every function it
//! calls and every constant it reads is checked once, in first-reach order.

use std::collections::{BTreeSet, VecDeque};

use crate::refusal::forms::ValueForm;
use crate::refusal::{Refusal, RefusedForm};
use crate::msl::{Diagnostic, Span};
use crate::pom::{BehaviorKind, BlockNode, BuiltinId, Decl, DeclData, DeclId, DeclKind, Design, DesignDiagnostic, ExprId, FileId, FnParamData, GenericArgNode, GenericArgsNode, PathId, Reflect, Resolution, TypeId, TypeKind};

use super::attributes::AttributeCatalogue;
use super::value::flow::ExitFrame;
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
use super::analog::nature_rules::NatureRules;
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

    fn declaration(&mut self, decl: DeclId) {
        if !self.worklist.checked.insert(decl) {
            return;
        }
        self.state.values.begin_declaration(decl);
        match &self.state.design.decl_data(decl).kind {
            DeclKind::Module(_) => self.module(decl),
            DeclKind::Fn(_) => self.function(decl),
            DeclKind::Const(_) => self.constant(decl),
            DeclKind::Impl(_) => self.trait_typing().coherence(decl),
            other => self.refuse_declaration(decl, RefusedForm::item(other)),
        }
        self.expressions().report_uncalled_closures();
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
    /// A diagnostic of a rule of the specification that carries its own code.
    pub(super) fn coded(&mut self, file: FileId, code: &'static str, span: Span, message: String, label: &str) {
        self.report(file, Diagnostic::new(code, message, span, label));
    }

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
        if self.modules().untyped_template_read(file, span, found) {
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

    /// The type a built-in name written as a type stands for: a primitive, `Option` or `Event`; any other built-in is
    /// refused.
    fn lower_builtin_type(&mut self, id: BuiltinId, path: PathId, file: FileId, span: Span) -> Option<Ty> {
        match (id.info().name, id.width()) {
            ("fN", Some(w)) => Ty::float(u32::from(w)),
            ("bool", _) => Some(Ty::Bool),
            // §1.2.9: `str` is a value type, the type of a string.
            ("str", _) => Some(Ty::Str),
            ("uN", Some(w)) => self.integer_type(file, span, Ty::UInt, u128::from(w), &id.name()),
            ("iN", Some(w)) => self.integer_type(file, span, Ty::Int, u128::from(w), &id.name()),
            // §1 Resolution: `usize` and `isize` are `UInt<64>` and `Int<64>` everywhere.
            ("usize", _) => Some(Ty::UInt(64)),
            ("isize", _) => Some(Ty::Int(64)),
            (family @ ("UInt" | "Int" | "Float"), _) => self.sized_type(file, span, path, family),
            ("Option", _) => self.expressions().option_type(file, span, path),
            ("Event", _) => Some(Ty::Event),
            _ => {
                self.refuse_with(file, span, ValueForm::Type, [id.name().to_string()]);
                None
            }
        }
    }

    fn lower_type_node(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.state.design.type_node(ty).clone();
        let file = self.state.design.decl_data(node.owner).file;
        match &node.kind {
            TypeKind::Path(path) => match self.state.design.type_resolution(ty) {
                Resolution::Builtin(id) => self.lower_builtin_type(id, *path, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Nature(_)) && NatureRules::of(self.state.design, d).carries_values => self.carriers().discrete_carrier_type(d, file, node.span, *path),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Nature(_)) => Some(Ty::Node(d)),
                // §3.8.5: a generic struct or enum is written with its arguments; its fields are typed where it is used
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && self.state.design.decl_data(d).generics_span.is_some() => self.generics().generic_nominal(*path, d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && !self.writes_no_arguments_to_a_plain_type(*path, d, file, node.span) => None,
                // §3.8.2: a type or nature parameter is a type of the generic body, opaque
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::GenericParam(crate::pom::GenericParamData::Type { .. })) => Some(Ty::Param(d)),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Enum(_)) => self.expressions().enum_type(d, file, node.span),
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Struct(_)) => self.expressions().struct_type(d, file, node.span),
                // A module named as a type is the type of a param (§8.1.1, PI-04): a template of it; a generic one's names its
                // arguments, `G<2>` (§8.4.3)
                Resolution::Decl(d) if matches!(self.state.design.decl_data(d).kind, DeclKind::Module(_)) && self.state.design.decl_data(d).generics_span.is_some() => self.generics().generic_module_type(*path, d, file, node.span),
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
            TypeKind::Tuple(items) => self.expressions().tuple_type(items),
            TypeKind::Array { elem, len } => self.expressions().array_type(file, node.span, *elem, *len),
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
                GenericArgNode::Const(expr) => self.generics().const_term(*expr, file),
                GenericArgNode::Type(ty) => self.generics().const_parameter_named_by(*ty).map(ConstTerm::Param),
            },
            _ => None,
        };
        let width = match term {
            Some(ConstTerm::Value(value)) => u128::try_from(value).ok(),
            Some(generic @ (ConstTerm::Param(_) | ConstTerm::Expr(_))) if matches!(family, "UInt" | "Int") => {
                if let ConstTerm::Expr(expr) = &generic {
                    self.generics().defer_constant(*expr);
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
            Some(value) => self.within(Context::Const, |this| this.expressions().check(value, &ty)),
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
        let outer = self.state.generics.replace_generic_item(self.state.design.decl_data(decl).generics_span.map(|_| decl));
        let parts = self.signature_parts_of(decl);
        self.state.generics.replace_generic_item(outer);
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
            self.generics().generic_signature(decl);
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
                FnParamData::SelfValue => self.expressions().self_parameter(decl, child),
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
        let outer = self.state.generics.replace_generic_item(self.state.design.decl_data(decl).generics_span.map(|_| decl));
        self.function_of(decl);
        self.state.generics.replace_generic_item(outer);
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
            if self.state.generics.generic_item() == Some(decl) {
                self.generics().defer(super::Obligation::Where { item: decl, condition: cond });
            }
            self.state.values.set_fn_where(true);
            self.within(Context::Const, |this| this.expressions().check(cond, &Ty::Bool));
            self.state.values.set_fn_where(false);
        }
        for (param, ty) in parts.params {
            match ty {
                // QC3: §3 rule 7 makes a nature parameter a handle and says nothing of an array of them
                Some(array @ Ty::Array(..)) if array.is_carrier_element() => {
                    let (file, span) = (self.state.design.decl_data(param).file, self.state.design.decl_data(param).span);
                    self.refuse(file, span, ValueForm::ArrayOfCarriersAsFunctionParameter);
                    self.state.values.poison(param);
                }
                Some(ty) => {
                    self.modules().note_handle(param, &ty);
                    self.state.result.locals.insert(param, ty);
                }
                None => {
                    self.state.values.poison(param);
                }
            }
        }
        // before the body: a handle kept is `E0133`, not the mismatch the same span would also report
        self.modules().check_handle_result(decl, parts.ret.as_ref());
        self.modules().check_handle_uses(decl);
        self.state.values.begin_function(ExitFrame::new(parts.ret.clone(), false));
        match (&f.body, parts.ret) {
            (Some(body), Some(ret)) => self.function_body(body, &ret, data.file),
            (Some(body), None) => {
                self.expressions().statements(&body.stmts, None);
            }
            // a function a trait declares has no body; its implementations do
            (None, _) if data.parent.is_some_and(|parent| matches!(self.state.design.decl_data(parent).kind, DeclKind::Trait(_))) => {}
            // an `extern` function is implemented outside the language: its signature is all there is to check
            (None, _) if f.abi.is_some() => {}
            (None, _) => self.refuse(data.file, data.span, ValueForm::FunctionWithoutBody),
        }
        self.state.values.set_context(Context::Module);
    }

    /// The statements of a body against the declared result. A missing result is reported
    /// whatever else the body got wrong, since it is its own mistake.
    fn function_body(&mut self, body: &BlockNode, ret: &Ty, file: FileId) {
        if let Some(found) = self.expressions().statements(&body.stmts, Some(ret))
            && !self.generics().compatible_at(file, body.span, &found, ret)
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
            (DeclKind::Port(p), ItemPhase::Declarations) => self.modules().port(child, &p.clone(), item.file),
            (DeclKind::Param(p), ItemPhase::Declarations) => self.modules().param(child, &p.clone()),
            (DeclKind::Signal(s), ItemPhase::Declarations) => self.modules().signal(child, &s.clone()),
            (DeclKind::Let(_), ItemPhase::Lets) => self.modules().type_module_let(child),
            (DeclKind::Definition(d), ItemPhase::Definitions) => self.carriers().definition(&d.clone()),
            (DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) | DeclKind::Let(_) | DeclKind::Definition(_), _) => {}
            (DeclKind::GenericParam(_) | DeclKind::Behavior(_) | DeclKind::Error, _) => {}
            // A closure's parameters (`|new, old| ..` of `limexp`, §22.2.4) are children of the
            // module that holds the closure, typed with it, not module items (design D9).
            (DeclKind::Binding(_), _) => {}
            (DeclKind::Generation(_), _) => self.modules().generation(child, phase == ItemPhase::Declarations, |this, inner| this.module_item(inner, phase)),
            (other, ItemPhase::Declarations) => self.refuse_with(item.file, item.span, ValueForm::InsideModule, [other.label().to_lowercase().to_string()]),
            (_, _) => {}
        }
    }

    /// A module of the reachable set: its params, nodes and placements, then its analog blocks.
    /// The modules it places are reached.
    fn module(&mut self, decl: DeclId) {
        let outer = self.state.generics.replace_generic_item(self.state.design.decl_data(decl).generics_span.map(|_| decl));
        self.module_of(decl);
        self.state.generics.replace_generic_item(outer);
    }

    /// [`Checker::module`] with the generic item set: a generic module is typed once at its definition, its type and
    /// nature parameters opaque (`Ty::Param`) and its const parameters `ConstTerm::Param` (GN-13, Q1 (c)).
    fn module_of(&mut self, decl: DeclId) {
        let data = self.state.design.decl_data(decl).clone();
        let DeclKind::Module(m) = &data.kind else { return };
        if data.generics_span.is_some() {
            self.generics().generic_signature(decl);
        }
        if m.abi.is_some() {
            self.refuse(data.file, data.span, ValueForm::ExternModule);
        }
        if let Some(cond) = m.where_ {
            // Part III §8 errors: the `where` after the port list that is not a `bool` const expression.
            self.within(Context::Const, |this| this.expressions().check_as(cond, &Ty::Bool, "E0304"));
        }
        // Ports, params and signals first, then the `let`s in order, then definitions: every
        // name of the body is visible in the whole body (§8.2), and a carrier's type is known
        // before anything reads it.
        self.state.carriers.reset_book();
        // each bundle port or signal is followed by the fields it stands for, typed like ports and signals of their own (§7 rule 14)
        let items = self.state.design.module_items(&data.children);
        for phase in [ItemPhase::Declarations, ItemPhase::Lets, ItemPhase::Definitions] {
            for child in items.iter().copied() {
                self.module_item(child, phase);
            }
        }
        self.carriers().definition_loops();
        self.modules().module_result(decl, m);
        for behavior in m.behaviors.iter().copied() {
            let b = self.state.design.decl_data(behavior).clone();
            let DeclKind::Behavior(bd) = &b.kind else { continue };
            match bd.kind {
                BehaviorKind::Analog => {
                    self.state.values.set_context(Context::Analog);
                    self.expressions().statements(&bd.body, None);
                    self.state.values.set_context(Context::Module);
                }
                BehaviorKind::Digital => self.carriers().digital_block(behavior),
            }
        }
        // A placement no statement above typed (one written where only the module lists it).
        for instance in m.instances.iter().copied() {
            // a placement inside generation is typed with generation itself (T72); until then its loop variable has no type
            let in_generation = self.inside_generation(self.state.design.expr_node(instance).owner);
            if self.state.result.exprs[instance.index()].is_none() && !in_generation {
                self.expressions().infer(instance);
            }
        }
        // the carriers declared inside generation are the module's too, one array each (§8.2.23)
        let carriers = self.modules().generated_items(&items);
        self.modules().missing_initial_values(&carriers);
        self.carriers().initial_values_under_element_definitions(&carriers);
        self.carriers().defer_unfolded_overlaps();
        self.modules().handed_drives();
    }
}
