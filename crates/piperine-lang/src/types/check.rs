//! The declaration worklist: from the top module, every module it places, every function it
//! calls and every constant it reads is checked once, in first-reach order.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::msl::{Diagnostic, Span};
use crate::pom::{BehaviorKind, BlockNode, Decl, DeclData, DeclId, DeclKind, Design, DesignDiagnostic, ExprId, FileId, FnParamData, GenericArgNode, GenericArgsNode, PathId, Reflect, Resolution, TypeId, TypeKind};

use super::continuity::Continuity;
use super::class::FunctionClass;
use super::closures::ClosureInfo;
use super::coverage::DeferredCoverage;
use super::definition::DriveBook;
use super::assign::Places;
use super::flow::{ExitFrame, Flow};
use super::nature_rules::NatureRules;
use super::record::Records;
use super::unsupported::Unsupported;
use super::{Builtin, Callee, ConstTerm, Method, Ty, Types};

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

/// One pass over the reachable set. Everything it learns goes into side tables; the design is
/// only read.
pub(super) struct Checker<'d> {
    pub(super) design: &'d Design,
    pub(super) exprs: Vec<Option<Ty>>,
    pub(super) locals: BTreeMap<DeclId, Ty>,
    pub(super) methods: BTreeMap<ExprId, Method>,
    pub(super) callees: BTreeMap<ExprId, Callee>,
    /// The argument bound to each parameter of a call, `None` for one left out (§3.2).
    pub(super) bindings: BTreeMap<ExprId, Vec<Option<ExprId>>>,
    pub(super) reached: Vec<DeclId>,
    /// Arguments read as `Some` of themselves (§3.2.3).
    pub(super) somes: BTreeSet<ExprId>,
    /// The `Ty::Carrier` of each discrete port and signal declaration read or declared so far.
    pub(super) carriers: BTreeMap<DeclId, Ty>,
    /// The array carriers whose length is not known yet and whose element drives the checker cannot judge: the
    /// elaborator judges them per instance (`coverage.rs`, `elab/element_coverage.rs`).
    pub(super) deferred_coverage: DeferredCoverage,
    pub(super) builtins: BTreeMap<ExprId, Builtin>,
    pub(super) widths: BTreeMap<ExprId, u32>,
    /// The type of each `T::default()` call checked so far.
    pub(super) defaults: BTreeMap<ExprId, Ty>,
    /// The type each type node of the reachable set writes.
    pub(super) written: BTreeMap<TypeId, Ty>,
    /// The generic function whose definition is being checked or whose signature is being typed:
    /// what a definition defers is recorded under it.
    pub(super) generic_item: Option<DeclId>,
    /// The arguments of the generic module whose placement is being connected: the ports are typed under it (GN-13).
    pub(super) placement_substitution: Option<super::Substitution>,
    /// What each generic function defers to its instantiations.
    pub(super) obligations: BTreeMap<DeclId, Vec<super::Obligation>>,
    /// The instantiation each call of a generic function makes (§3.8.10).
    pub(super) instantiations: BTreeMap<ExprId, super::Instantiation>,
    /// The method names a project item (an `fn` of an `impl` or a `trait` outside the stdlib)
    /// defines: a receiver the POM could not type might mean any of them.
    /// The traits and `impl`s of the design (§3.7), built once.
    pub(super) impls: std::rc::Rc<super::ImplTable<'d>>,
    pub(super) continuity: Continuity,
    /// What the module being checked drives, by definition and by commit.
    pub(super) book: DriveBook,
    /// The `emit!` sites seen so far.
    pub(super) records: Records,
    /// The bindings of the module-body `let`s whose values are being typed, each with its value (a
    /// read of one is a feedback, `Checker::self_read`).
    pub(super) lets_open: BTreeMap<DeclId, ExprId>,
    /// The placements whose arguments are being typed, innermost last: the module, or `None` for
    /// a core module.
    pub(super) placing: Vec<Option<DeclId>>,
    /// How many `ddx` values are being typed: a flow probe of a branch with no potential contribution may be read there,
    /// where it is a leaf the derivative reads symbolically (§14.2.3), though it is no value elsewhere (`probe`).
    pub(super) differentiated: u32,
    /// The declaration being checked: a module, a function or a constant.
    pub(super) owner: Option<DeclId>,
    pub(super) context: Context,
    /// The one expression that may be an `Event`: an event position hands it down just before it
    /// types the expression, which judges an `Event` anywhere else.
    pub(super) event_slot: Option<ExprId>,
    /// Set while the guard of an event, the right operand of `ev && guard`, is typed.
    pub(super) in_guard: bool,
    /// The events read as a `bool` (§13 rules 2 and 9): the expressions of type `Event` that
    /// stand in a guard or a reaction body and are typed `bool`.
    pub(super) event_reads: BTreeSet<ExprId>,
    /// Bindings whose value could not be typed: a read of one is silent, so one mistake is
    /// one diagnostic.
    pub(super) poisoned: BTreeSet<DeclId>,
    /// The reads of an item of a child only a folded template names (`reach.rs`), typed as a node until a context wants a value
    /// of them (`template_items.rs`).
    pub(super) template_reads: BTreeSet<ExprId>,
    /// The `let`s with no type bound to a read through a template, by binding: the first use that wants a value types them.
    pub(super) template_locals: BTreeMap<DeclId, ExprId>,
    /// The type each read through a template was given by its context, which the elaborator checks the concrete item against.
    pub(super) template_items: BTreeMap<ExprId, Ty>,
    /// The parameters of functions that are handles: a node or a carrier of a nature (`handles.rs`).
    pub(super) handles: BTreeSet<DeclId>,
    /// The arguments a bound writes for its trait, set for the call through it being typed (`bounds.rs`).
    pub(super) bound_substitution: Option<super::Substitution>,
    /// The arguments of the trait bound each call through a bound names, by call (`Types::bound_arguments`).
    pub(super) bound_arguments: BTreeMap<ExprId, Vec<Ty>>,
    /// How many definitions the check is inside: their values are not read from the solve (`classes.rs`).
    pub(super) in_definition: u32,
    /// Set while the `where` of a `fn` is checked: its parameters are not in scope (§3 rule 8).
    pub(super) fn_where: bool,
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
    /// The typed fields of each struct and each enum variant typed so far (`aggregate.rs`).
    pub(super) layouts: BTreeMap<DeclId, Vec<super::aggregate::FieldType>>,
    /// The enums whose variants are typed (`enums.rs`).
    pub(super) declared: BTreeSet<DeclId>,
    /// The tag of each enum declared so far.
    pub(super) enum_tags: BTreeMap<DeclId, super::enums::EnumTag>,
    /// The structs and enums whose fields are being typed: meeting one again is a type that
    /// contains itself (§1.3.8).
    pub(super) declaring: BTreeSet<DeclId>,
    /// The structs and enums that could not be typed: a use is silent, the cause is reported.
    pub(super) broken: BTreeSet<DeclId>,
    queued: BTreeSet<DeclId>,
    /// The declarations already checked, so one reached ahead of the queue (`check_ahead`) is not
    /// checked twice.
    checked: BTreeSet<DeclId>,
    queue: VecDeque<DeclId>,
    diagnostics: Vec<DesignDiagnostic>,
    /// The spans already reported: one diagnostic per construct.
    reported: BTreeSet<(FileId, usize, usize)>,
}

impl<'d> Checker<'d> {
    pub(super) fn new(design: &'d Design) -> Self {
        Checker {
            design,
            exprs: vec![None; design.expr_count()],
            locals: BTreeMap::new(),
            methods: BTreeMap::new(),
            callees: BTreeMap::new(),
            bindings: BTreeMap::new(),
            reached: Vec::new(),
            somes: BTreeSet::new(),
            carriers: BTreeMap::new(),
            deferred_coverage: DeferredCoverage::default(),
            builtins: BTreeMap::new(),
            widths: BTreeMap::new(),
            defaults: BTreeMap::new(),
            written: BTreeMap::new(),
            generic_item: None,
            placement_substitution: None,
            obligations: BTreeMap::new(),
            instantiations: BTreeMap::new(),
            impls: std::rc::Rc::new(super::ImplTable::new(design)),
            continuity: Continuity::new(design),
            book: DriveBook::default(),
            records: Records::default(),
            lets_open: BTreeMap::new(),
            placing: Vec::new(),
            differentiated: 0,
            owner: None,
            context: Context::Module,
            event_slot: None,
            in_guard: false,
            event_reads: BTreeSet::new(),
            poisoned: BTreeSet::new(),
            template_reads: BTreeSet::new(),
            template_locals: BTreeMap::new(),
            template_items: BTreeMap::new(),
            handles: BTreeSet::new(),
            bound_substitution: None,
            bound_arguments: BTreeMap::new(),
            in_definition: 0,
            fn_where: false,
            places: Places::default(),
            open_literals: BTreeMap::new(),
            deferred: BTreeSet::new(),
            flow: Flow::default(),
            closures: BTreeMap::new(),
            closure_locals: BTreeMap::new(),
            pending_closures: BTreeMap::new(),
            classes: BTreeMap::new(),
            call_expected: None,
            layouts: BTreeMap::new(),
            declared: BTreeSet::new(),
            enum_tags: BTreeMap::new(),
            declaring: BTreeSet::new(),
            broken: BTreeSet::new(),
            queued: BTreeSet::new(),
            checked: BTreeSet::new(),
            queue: VecDeque::new(),
            diagnostics: Vec::new(),
            reported: BTreeSet::new(),
        }
    }

    /// Whether the project itself defines a method called `name`, in an `impl` or a `trait`.
    pub(super) fn project_defines(&self, name: &str) -> bool {
        self.impls.defines_method(name, |decl| self.is_std(decl))
    }

    /// Check `top` and everything it reaches: the table, or the diagnostics in source order.
    pub(super) fn run(mut self, top: DeclId) -> Result<Types<'d>, Vec<DesignDiagnostic>> {
        self.check_attributes();
        self.reach(top);
        self.drain();
        self.report_unread_template_locals();
        if !self.diagnostics.is_empty() {
            return Err(self.sorted_diagnostics());
        }
        let typed: Vec<ExprId> = (0..self.exprs.len()).map(|i| ExprId(i as u32)).filter(|e| self.exprs[e.index()].is_some()).collect();
        for expr in typed {
            self.continuous(expr);
        }
        let continuous: BTreeSet<ExprId> = self.continuity.continuous_set().collect();
        let emits = self.records.emitting();
        Ok(Types { design: self.design, exprs: self.exprs, locals: self.locals, methods: self.methods, callees: self.callees, bindings: self.bindings, reached: self.reached, somes: self.somes, carriers: self.carriers, deferred_coverage: self.deferred_coverage, builtins: self.builtins, event_reads: self.event_reads, widths: self.widths, defaults: self.defaults, layouts: self.layouts, enum_tags: self.enum_tags, written: self.written, handles: self.handles, bound_arguments: self.bound_arguments, instantiations: self.instantiations, template_items: self.template_items, obligations: self.obligations, continuous, emits, assigned: self.places.into_assigned() })
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
        while let Some(decl) = self.queue.pop_front() {
            self.declaration(decl);
        }
        self.record_conflicts();
    }

    /// The diagnostics so far, in source order.
    fn sorted_diagnostics(self) -> Vec<DesignDiagnostic> {
        let mut diagnostics = self.diagnostics;
        diagnostics.sort_by_key(|d| (d.file, d.diagnostic.span().start));
        diagnostics
    }

    /// Run `body` with `context` as the place being checked, then restore the one outside.
    pub(super) fn within<R>(&mut self, context: Context, body: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.context, context);
        let result = body(self);
        self.context = outer;
        result
    }

    /// Put a declaration on the worklist the first time it is reached.
    pub(super) fn reach(&mut self, decl: DeclId) {
        if self.queued.insert(decl) {
            self.reached.push(decl);
            self.queue.push_back(decl);
            // a function of an `impl` brings its `impl` along: the declaration rules of the block and the types it writes
            // (the arguments of its trait) are judged once, wherever a function of it is first reached
            if let Some(owner) = self.design.decl_data(decl).parent.filter(|owner| matches!(self.design.decl_data(*owner).kind, DeclKind::Impl(_))) {
                self.reach(owner);
            }
        }
    }

    /// Check `module` now, out of queue order, when it is not checked yet: a read of a child's
    /// `let` needs the type of its value (`reach.rs`). The state of the declaration being checked
    /// is set aside and restored; a module already checked (or being checked) is left alone.
    pub(super) fn check_ahead(&mut self, module: DeclId) {
        if self.checked.contains(&module) {
            return;
        }
        self.reach(module);
        let (owner, context, event_slot, in_guard, fn_where) = (self.owner, self.context, self.event_slot, self.in_guard, self.fn_where);
        let (book, lets_open, placing, flow) = (std::mem::take(&mut self.book), std::mem::take(&mut self.lets_open), std::mem::take(&mut self.placing), std::mem::take(&mut self.flow));
        self.context = Context::Module;
        self.event_slot = None;
        self.in_guard = false;
        self.fn_where = false;
        self.declaration(module);
        (self.owner, self.context, self.event_slot, self.in_guard, self.fn_where) = (owner, context, event_slot, in_guard, fn_where);
        (self.book, self.lets_open, self.placing, self.flow) = (book, lets_open, placing, flow);
    }

    fn declaration(&mut self, decl: DeclId) {
        if !self.checked.insert(decl) {
            return;
        }
        self.owner = Some(decl);
        self.flow = Flow::default();
        match &self.design.decl_data(decl).kind {
            DeclKind::Module(_) => self.module(decl),
            DeclKind::Fn(_) => self.function(decl),
            DeclKind::Const(_) => self.constant(decl),
            DeclKind::Impl(_) => self.coherence(decl),
            other => self.unsupported_decl(decl, Unsupported::item(other)),
        }
        self.report_uncalled_closures();
    }

    // ---- reporting -------------------------------------------------------------------------

    /// One diagnostic per construct: a second one at the same span is dropped.
    pub(super) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        let span = diagnostic.span();
        if self.reported.insert((file, span.start, span.end)) {
            self.diagnostics.push(DesignDiagnostic { file, diagnostic });
        }
    }

    /// Reports `diagnostic` whatever else is already reported at its span: a finding about a
    /// relation between constructs (a loop of definitions) is not the same finding as one about
    /// the construct at the span where it is shown.
    pub(super) fn report_distinct(&mut self, file: FileId, diagnostic: Diagnostic) {
        self.diagnostics.push(DesignDiagnostic { file, diagnostic });
    }

    pub(super) fn file_of_expr(&self, expr: ExprId) -> FileId {
        self.design.decl_data(self.design.expr_node(expr).owner).file
    }

    /// `P0001`: a construct outside the M1 subset, named.
    pub(super) fn unsupported(&mut self, file: FileId, span: Span, what: &str) {
        self.report(file, Diagnostic::new("P0001", format!("{what} is not supported yet"), span, format!("{what} is outside the M1 subset")));
    }

    fn unsupported_decl(&mut self, decl: DeclId, what: &str) {
        let data = self.design.decl_data(decl);
        let (file, span) = (data.file, data.span);
        self.unsupported(file, span, what);
    }

    /// `E0180`: a plain type mismatch at `span`, naming both types.
    pub(super) fn mismatch(&mut self, file: FileId, span: Span, expected: &Ty, found: &Ty) {
        let (expected, found) = (expected.name(self.design), found.name(self.design));
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
            self.written.insert(ty, t.clone());
        }
        lowered
    }

    /// Whether the path of a struct or enum type writes no generic argument where the declaration `decl`
    /// has no generic parameter: `Pair<4>` over a plain `struct Pair` is `P0001` (it would be read as `Pair`,
    /// silently ignoring `4`). A generic declaration is read by `generic_nominal`.
    fn writes_no_arguments_to_a_plain_type(&mut self, path: PathId, decl: DeclId, file: FileId, span: Span) -> bool {
        let written = self.design.path_node(path).segments.last().is_some_and(|segment| segment.args.is_some());
        if !written || self.design.decl_data(decl).generics_span.is_some() {
            return true;
        }
        let name = self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        self.unsupported(file, span, &format!("generic arguments on `{name}`, which has no generic parameter"));
        false
    }

    fn lower_type_node(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.design.type_node(ty).clone();
        let file = self.design.decl_data(node.owner).file;
        match &node.kind {
            TypeKind::Path(path) => match self.design.type_resolution(ty) {
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
                        self.unsupported(file, node.span, &format!("type `{}`", id.name()));
                        None
                    }
                },
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Nature(_)) && NatureRules::of(self.design, d).carries_values && NatureRules::of(self.design, d).one_driver => self.discrete_carrier_type(d, file, node.span, *path),
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Nature(_)) => Some(Ty::Node(d)),
                // §3.8.5: a generic struct or enum is written with its arguments; its fields are typed where it is used
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && self.design.decl_data(d).generics_span.is_some() => self.generic_nominal(*path, d, file, node.span),
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Enum(_) | DeclKind::Struct(_)) && !self.writes_no_arguments_to_a_plain_type(*path, d, file, node.span) => None,
                // §3.8.2: a type or nature parameter is a type of the generic body, opaque
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::GenericParam(crate::pom::GenericParamData::Type { .. })) => Some(Ty::Param(d)),
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Enum(_)) => self.enum_type(d, file, node.span),
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Struct(_)) => self.struct_type(d, file, node.span),
                // A module named as a type is the type of a param (§8.1.1, PI-04): a template of it; a generic one's names its
                // arguments, `G<2>` (§8.4.3)
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Module(_)) && self.design.decl_data(d).generics_span.is_some() => self.generic_module_type(*path, d, file, node.span),
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Module(_)) => self.writes_no_arguments_to_a_plain_type(*path, d, file, node.span).then(|| Ty::module(d)),
                Resolution::Decl(d) => {
                    let what = Unsupported::item(&self.design.decl_data(d).kind);
                    self.unsupported(file, node.span, &format!("{what} used as a type"));
                    None
                }
                _ => {
                    self.unsupported(file, node.span, "this type");
                    None
                }
            },
            TypeKind::Tuple(items) if items.is_empty() => Some(Ty::Unit),
            TypeKind::Tuple(items) => self.tuple_type(items),
            TypeKind::Array { elem, len } => self.array_type(file, node.span, *elem, *len),
            TypeKind::Flip(_) | TypeKind::Infer => {
                self.unsupported(file, node.span, Unsupported::ty(&node.kind));
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
        Decl::new(self.design, decl).krate().is_std()
    }

    /// `UInt<N>`, `Int<N>` or `Float<N>` written with its width argument. The width is an integer
    /// literal; a width computed from a const expression is outside the subset (`P0001`).
    fn sized_type(&mut self, file: FileId, span: Span, path: PathId, family: &str) -> Option<Ty> {
        let args = self.design.path_node(path).segments.last().and_then(|s| s.args.clone());
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
            self.unsupported(file, span, &format!("`{family}` with a width that is not an integer literal"));
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
        let DeclKind::Const(data) = self.design.decl_data(decl).kind.clone() else { return };
        let Some(ty) = self.lower_type(data.ty) else { return };
        match data.value {
            Some(value) => self.within(Context::Const, |this| this.check(value, &ty)),
            None => {
                let d = self.design.decl_data(decl);
                let (file, span) = (d.file, d.span);
                let name = d.name.as_ref().map_or_else(String::new, |name| name.text.clone());
                let in_trait = d.parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Trait(_)));
                if in_trait {
                    self.unsupported(file, span, "`const` without a value");
                } else {
                    // §3.9.2: a `const` has a value everywhere but in a trait.
                    self.coded(file, "E0188", span, format!("the `const` `{name}` has no value"), "a `const` outside a trait has a value");
                }
            }
        }
    }

    /// The declared parameter types and result of a function, or `None` after a `P0001`.
    pub(super) fn signature(&mut self, decl: DeclId) -> Option<(Vec<Ty>, Ty)> {
        let data = self.design.decl_data(decl);
        if data.generics_span.is_some() {
            // a use of a generic function binds its arguments after inference (T20, T21): until then
            // a call or a function value of one is outside the subset
            let (file, span) = (data.file, data.span);
            self.unsupported(file, span, "generic function");
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
        let outer = std::mem::replace(&mut self.generic_item, self.design.decl_data(decl).generics_span.map(|_| decl));
        let parts = self.signature_parts_of(decl);
        self.generic_item = outer;
        parts
    }

    /// [`Checker::signature_parts`] with the generic item set.
    fn signature_parts_of(&mut self, decl: DeclId) -> Option<SignatureParts> {
        let data: DeclData = self.design.decl_data(decl).clone();
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
            self.unsupported(data.file, data.span, "`extern` function");
            ok = false;
        }
        let mut params = Vec::new();
        for child in data.children.iter().copied() {
            let DeclKind::FnParam(p) = &self.design.decl_data(child).kind else { continue };
            let ty = match p {
                FnParamData::Named { ty } => self.lower_type(*ty),
                FnParamData::SelfValue => self.self_parameter(decl, child),
                FnParamData::Error => {
                    let d = self.design.decl_data(child);
                    self.unsupported(d.file, d.span, "`self` parameter");
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
        let outer = std::mem::replace(&mut self.generic_item, self.design.decl_data(decl).generics_span.map(|_| decl));
        self.function_of(decl);
        self.generic_item = outer;
    }

    /// [`Checker::function`] with the generic item set.
    fn function_of(&mut self, decl: DeclId) {
        let Some(parts) = self.signature_parts(decl) else { return };
        let data = self.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return };
        // §3 rule 8: the `where` ranges over const generics and associated consts, which the
        // subset has none of. Checked before the parameters are typed, a read of a value
        // parameter in it is refused (`P0001`) and the specification gives a non-`bool`
        // condition no code of its own, so that is the plain mismatch.
        if let Some(cond) = f.where_ {
            if self.generic_item == Some(decl) {
                self.defer(super::Obligation::Where { item: decl, condition: cond });
            }
            self.fn_where = true;
            self.within(Context::Const, |this| this.check(cond, &Ty::Bool));
            self.fn_where = false;
        }
        for (param, ty) in parts.params {
            match ty {
                // QC3: §3 rule 7 makes a nature parameter a handle and says nothing of an array of them
                Some(array @ Ty::Array(..)) if array.is_carrier_element() => {
                    let (file, span) = (self.design.decl_data(param).file, self.design.decl_data(param).span);
                    self.unsupported(file, span, "an array of carriers as a function parameter");
                    self.poisoned.insert(param);
                }
                Some(ty) => {
                    self.note_handle(param, &ty);
                    self.locals.insert(param, ty);
                }
                None => {
                    self.poisoned.insert(param);
                }
            }
        }
        // before the body: a handle kept is `E0133`, not the mismatch the same span would also report
        self.check_handle_result(decl, parts.ret.as_ref());
        self.check_handle_uses(decl);
        self.context = Context::Function;
        self.flow.exit = Some(ExitFrame::new(parts.ret.clone(), false));
        match (&f.body, parts.ret) {
            (Some(body), Some(ret)) => self.function_body(body, &ret, data.file),
            (Some(body), None) => {
                self.statements(&body.stmts, None);
            }
            // a function a trait declares has no body; its implementations do
            (None, _) if data.parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Trait(_))) => {}
            // an `extern` function is implemented outside the language: its signature is all there is to check
            (None, _) if f.abi.is_some() => {}
            (None, _) => self.unsupported(data.file, data.span, "function without a body"),
        }
        self.context = Context::Module;
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
            if matches!(self.design.decl_data(current).kind, DeclKind::Generation(_)) {
                return true;
            }
            at = self.design.decl_data(current).parent;
        }
        false
    }

    /// One item of a module body (or of a generation in it) in `phase`.
    fn module_item(&mut self, child: DeclId, phase: ItemPhase) {
        let item = self.design.decl_data(child).clone();
        match (&item.kind, phase) {
            (DeclKind::Port(p), ItemPhase::Declarations) => self.port(child, &p.clone(), item.file),
            (DeclKind::Param(p), ItemPhase::Declarations) => self.param(child, &p.clone()),
            (DeclKind::Signal(s), ItemPhase::Declarations) => self.signal(child, &s.clone()),
            (DeclKind::Let(l), ItemPhase::Lets) => self.module_let(child, &l.clone()),
            (DeclKind::Definition(d), ItemPhase::Definitions) => self.definition(&d.clone()),
            (DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) | DeclKind::Let(_) | DeclKind::Definition(_), _) => {}
            (DeclKind::GenericParam(_) | DeclKind::Behavior(_) | DeclKind::Error, _) => {}
            // A closure's parameters (`|new, old| ..` of `limexp`, §22.2.4) are children of the
            // module that holds the closure, typed with it, not module items (design D9).
            (DeclKind::Binding(_), _) => {}
            (DeclKind::Generation(_), _) => self.generation(child, phase == ItemPhase::Declarations, |this, inner| this.module_item(inner, phase)),
            (other, ItemPhase::Declarations) => self.unsupported(item.file, item.span, &format!("{} inside a module", other.label().to_lowercase())),
            (_, _) => {}
        }
    }

    /// A module of the reachable set: its params, nodes and placements, then its analog blocks.
    /// The modules it places are reached.
    fn module(&mut self, decl: DeclId) {
        let outer = std::mem::replace(&mut self.generic_item, self.design.decl_data(decl).generics_span.map(|_| decl));
        self.module_of(decl);
        self.generic_item = outer;
    }

    /// [`Checker::module`] with the generic item set: a generic module is typed once at its definition, its type and
    /// nature parameters opaque (`Ty::Param`) and its const parameters `ConstTerm::Param` (GN-13, Q1 (c)).
    fn module_of(&mut self, decl: DeclId) {
        let data = self.design.decl_data(decl).clone();
        let DeclKind::Module(m) = &data.kind else { return };
        if data.generics_span.is_some() {
            self.generic_signature(decl);
        }
        if m.abi.is_some() {
            self.unsupported(data.file, data.span, "`extern` module");
        }
        if let Some(cond) = m.where_ {
            // Part III §8 errors: the `where` after the port list that is not a `bool` const expression.
            self.within(Context::Const, |this| this.check_as(cond, &Ty::Bool, "E0304"));
        }
        // Ports, params and signals first, then the `let`s in order, then definitions: every
        // name of the body is visible in the whole body (§8.2), and a carrier's type is known
        // before anything reads it.
        self.book = DriveBook::default();
        for phase in [ItemPhase::Declarations, ItemPhase::Lets, ItemPhase::Definitions] {
            for child in data.children.iter().copied() {
                self.module_item(child, phase);
            }
        }
        self.definition_loops();
        self.module_result(decl, m);
        for behavior in m.behaviors.iter().copied() {
            let b = self.design.decl_data(behavior).clone();
            let DeclKind::Behavior(bd) = &b.kind else { continue };
            match bd.kind {
                BehaviorKind::Analog => {
                    self.context = Context::Analog;
                    self.statements(&bd.body, None);
                    self.context = Context::Module;
                }
                BehaviorKind::Digital => self.digital_block(behavior),
            }
        }
        // A placement no statement above typed (one written where only the module lists it).
        for instance in m.instances.iter().copied() {
            // a placement inside generation is typed with generation itself (T72); until then its loop variable has no type
            let in_generation = self.inside_generation(self.design.expr_node(instance).owner);
            if self.exprs[instance.index()].is_none() && !in_generation {
                self.infer(instance);
            }
        }
        // the carriers declared inside generation are the module's too, one array each (§8.2.23)
        let carriers = self.generated_items(&data.children);
        self.missing_initial_values(&carriers);
        self.initial_values_under_element_definitions(&carriers);
        self.defer_unfolded_overlaps();
        self.handed_drives();
    }
}
