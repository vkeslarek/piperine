//! The declaration worklist: from the top module, every module it places, every function it
//! calls and every constant it reads is checked once, in first-reach order.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::msl::{Diagnostic, Span};
use crate::msl::lexer;
use crate::pom::{BehaviorKind, BlockNode, Decl, DeclData, DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, FileId, FnParamData, GenericArgNode, GenericArgsNode, LitKind, NatureKind, PathId, Reflect, Resolution, TypeId, TypeKind};

use super::continuity::Continuity;
use super::definition::DriveBook;
use super::flow::{ExitFrame, Flow};
use super::record::Records;
use super::unsupported::Unsupported;
use super::{Builtin, Callee, Method, Ty, Types};

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
struct SignatureParts {
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
    pub(super) builtins: BTreeMap<ExprId, Builtin>,
    pub(super) widths: BTreeMap<ExprId, u32>,
    /// The type each type node of the reachable set writes.
    pub(super) written: BTreeMap<TypeId, Ty>,
    /// The method names a project item (an `fn` of an `impl` or a `trait` outside the stdlib)
    /// defines: a receiver the POM could not type might mean any of them.
    pub(super) project_methods: BTreeSet<String>,
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
    /// The declaration being checked: a module, a function or a constant.
    pub(super) owner: Option<DeclId>,
    pub(super) context: Context,
    /// The one expression that may be an `Event`: an event position hands it down just before it
    /// types the expression, which judges an `Event` anywhere else.
    pub(super) event_slot: Option<ExprId>,
    /// Set while the guard of an event, the right operand of `ev && guard`, is typed.
    pub(super) in_guard: bool,
    /// Set while the value of a commit is typed: an `Event` there is committed (`E0441`), not
    /// read as a `bool`.
    pub(super) committing: bool,
    /// Bindings whose value could not be typed: a read of one is silent, so one mistake is
    /// one diagnostic.
    pub(super) poisoned: BTreeSet<DeclId>,
    /// Set while the `where` of a `fn` is checked: its parameters are not in scope (§3 rule 8).
    pub(super) fn_where: bool,
    /// The one expression that may be a function used as a value: an argument written
    /// directly where the parameter has a function type.
    pub(super) fn_value_at: Option<ExprId>,
    /// The locals declared with `mut` (§1.7.7).
    pub(super) mutable: BTreeSet<DeclId>,
    /// The locals declared without a value (`let x;`, §1.7.5), each with the loop depth it is
    /// declared at.
    pub(super) deferred: BTreeMap<DeclId, usize>,
    /// The control state of the body being checked.
    pub(super) flow: Flow,
    /// The type the context expects of one call, handed down by `check_as` just before the call
    /// is typed: the result type of a call that is generic over its result (`logic`, `cast`).
    pub(super) call_expected: Option<(ExprId, Ty)>,
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
            builtins: BTreeMap::new(),
            widths: BTreeMap::new(),
            written: BTreeMap::new(),
            project_methods: Self::project_methods(design),
            continuity: Continuity::new(design),
            book: DriveBook::default(),
            records: Records::default(),
            lets_open: BTreeMap::new(),
            placing: Vec::new(),
            owner: None,
            context: Context::Module,
            event_slot: None,
            in_guard: false,
            committing: false,
            poisoned: BTreeSet::new(),
            fn_where: false,
            fn_value_at: None,
            mutable: BTreeSet::new(),
            deferred: BTreeMap::new(),
            flow: Flow::default(),
            call_expected: None,
            queued: BTreeSet::new(),
            checked: BTreeSet::new(),
            queue: VecDeque::new(),
            diagnostics: Vec::new(),
            reported: BTreeSet::new(),
        }
    }

    /// The names of the methods the project itself defines, in an `impl` or a `trait`.
    fn project_methods(design: &'d Design) -> BTreeSet<String> {
        (0..design.decl_count())
            .map(|i| DeclId(i as u32))
            .filter(|d| matches!(design.decl_data(*d).kind, DeclKind::Fn(_)) && design.decl_data(*d).parent.is_some_and(|p| matches!(design.decl_data(p).kind, DeclKind::Impl(_) | DeclKind::Trait(_))) && !Decl::new(design, *d).krate().is_std())
            .filter_map(|d| design.decl_data(d).name.as_ref().map(|n| n.text.clone()))
            .collect()
    }

    /// Check `top` and everything it reaches: the table, or the diagnostics in source order.
    pub(super) fn run(mut self, top: DeclId) -> Result<Types<'d>, Vec<DesignDiagnostic>> {
        self.reach(top);
        while let Some(decl) = self.queue.pop_front() {
            self.declaration(decl);
        }
        self.record_conflicts();
        if !self.diagnostics.is_empty() {
            let mut diagnostics = self.diagnostics;
            diagnostics.sort_by_key(|d| (d.file, d.diagnostic.span().start));
            return Err(diagnostics);
        }
        let typed: Vec<ExprId> = (0..self.exprs.len()).map(|i| ExprId(i as u32)).filter(|e| self.exprs[e.index()].is_some()).collect();
        for expr in typed {
            self.continuous(expr);
        }
        let continuous: BTreeSet<ExprId> = self.continuity.continuous_set().collect();
        let emits = self.records.emitting();
        Ok(Types { design: self.design, exprs: self.exprs, locals: self.locals, methods: self.methods, callees: self.callees, bindings: self.bindings, reached: self.reached, somes: self.somes, carriers: self.carriers, builtins: self.builtins, widths: self.widths, written: self.written, continuous, emits })
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
        let (owner, context, event_slot, in_guard, committing, fn_where, fn_value_at) = (self.owner, self.context, self.event_slot, self.in_guard, self.committing, self.fn_where, self.fn_value_at);
        let (book, lets_open, placing, flow) = (std::mem::take(&mut self.book), std::mem::take(&mut self.lets_open), std::mem::take(&mut self.placing), std::mem::take(&mut self.flow));
        self.context = Context::Module;
        self.event_slot = None;
        self.in_guard = false;
        self.committing = false;
        self.fn_where = false;
        self.fn_value_at = None;
        self.declaration(module);
        (self.owner, self.context, self.event_slot, self.in_guard, self.committing, self.fn_where, self.fn_value_at) = (owner, context, event_slot, in_guard, committing, fn_where, fn_value_at);
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
            other => self.unsupported_decl(decl, Unsupported::item(other)),
        }
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

    /// `P0002`: a plain type mismatch at `span`, naming both types.
    pub(super) fn mismatch(&mut self, file: FileId, span: Span, expected: &Ty, found: &Ty) {
        let (expected, found) = (expected.name(self.design), found.name(self.design));
        self.mismatch_text(file, span, &expected, &found);
    }

    /// `P0002` with the two types already written out.
    pub(super) fn mismatch_text(&mut self, file: FileId, span: Span, expected: &str, found: &str) {
        self.mismatch_coded(file, span, "P0002", expected, found);
    }

    /// A mismatch under the code of the rule that was broken.
    pub(super) fn mismatch_coded(&mut self, file: FileId, span: Span, code: &'static str, expected: &str, found: &str) {
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

    fn lower_type_node(&mut self, ty: TypeId) -> Option<Ty> {
        let node = self.design.type_node(ty).clone();
        let file = self.design.decl_data(node.owner).file;
        match &node.kind {
            TypeKind::Path(path) => match self.design.type_resolution(ty) {
                Resolution::Builtin(id) => match (id.info().name, id.width()) {
                    ("fN", Some(64)) => Some(Ty::F64),
                    ("bool", _) => Some(Ty::Bool),
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
                Resolution::Decl(d) if matches!(&self.design.decl_data(d).kind, DeclKind::Nature(n) if n.kind == NatureKind::Discrete) && self.is_std(d) && self.design.decl_data(d).name.as_ref().is_some_and(|n| n.text == "Bit") => self.bit_type(d, file, node.span, *path),
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Nature(_)) => Some(Ty::Node(d)),
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Enum(_)) => self.enum_type(d, file, node.span),
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
            // `&str` (also `&'static str`): the type of a string literal, which no `let`, no carrier
            // and no param holds.
            TypeKind::Ref { inner, .. } if matches!(self.design.type_resolution(*inner), Resolution::Builtin(id) if id.info().name == "str") => Some(Ty::Str),
            TypeKind::Tuple(_) | TypeKind::Array { .. } | TypeKind::Slice(_) | TypeKind::Ref { .. } | TypeKind::Flip(_) | TypeKind::Infer => {
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
        let width = match &args {
            Some(GenericArgsNode::Angle { args, .. }) if args.len() == 1 => match &args[0] {
                GenericArgNode::Const(expr) => match &self.design.expr_node(*expr).kind {
                    ExprKind::Lit(lit) if matches!(lit.kind, LitKind::Int(_)) => match &lit.kind {
                        LitKind::Int(text) => lexer::int_value(text),
                        _ => None,
                    },
                    _ => None,
                },
                GenericArgNode::Type(_) => None,
            },
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
            _ => match width {
                64 => Some(Ty::F64),
                16 | 32 | 128 => {
                    self.unsupported(file, span, &format!("type `{written}` (`{family}` of width {width}; the subset's one float is `f64`)"));
                    None
                }
                _ => {
                    let message = format!("`{written}`: a float width is one of 16, 32, 64 and 128");
                    self.report(file, Diagnostic::new("E0103", message, span, "float width outside {16, 32, 64, 128}"));
                    None
                }
            },
        }
    }

    /// An integer type of width `width`, written `written`: 1..=64 is in the subset, a wider one
    /// is `P0001` naming it, a width below 1 is `E0103` (§1 rule 5).
    pub(super) fn integer_type(&mut self, file: FileId, span: Span, make: fn(u8) -> Ty, width: u128, written: &str) -> Option<Ty> {
        match width {
            0 => {
                let message = format!("`{written}`: an integer width is a const expression `>= 1`");
                self.report(file, Diagnostic::new("E0103", message, span, "integer width below 1"));
                None
            }
            1..=64 => Some(make(width as u8)),
            _ => {
                self.unsupported(file, span, &format!("integer type `{written}`"));
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
                self.unsupported(file, span, "`const` without a value");
            }
        }
    }

    /// The declared parameter types and result of a function, or `None` after a `P0001`.
    pub(super) fn signature(&mut self, decl: DeclId) -> Option<(Vec<Ty>, Ty)> {
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
    fn signature_parts(&mut self, decl: DeclId) -> Option<SignatureParts> {
        let data: DeclData = self.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let mut ok = true;
        if data.generics_span.is_some() {
            self.unsupported(data.file, data.span, "generic function");
            ok = false;
        }
        if f.abi.is_some() {
            self.unsupported(data.file, data.span, "`extern` function");
            ok = false;
        }
        let mut params = Vec::new();
        for child in data.children.iter().copied() {
            let DeclKind::FnParam(p) = &self.design.decl_data(child).kind else { continue };
            let ty = match p {
                FnParamData::Named { ty } => self.lower_type(*ty),
                FnParamData::SelfValue | FnParamData::Error => {
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
        let Some(parts) = self.signature_parts(decl) else { return };
        let data = self.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return };
        // §3 rule 8: the `where` ranges over const generics and associated consts, which the
        // subset has none of. Checked before the parameters are typed, a read of a value
        // parameter in it is refused (`P0001`) and the specification gives a non-`bool`
        // condition no code of its own, so that is the plain mismatch.
        if let Some(cond) = f.where_ {
            self.fn_where = true;
            self.within(Context::Const, |this| this.check(cond, &Ty::Bool));
            self.fn_where = false;
        }
        for (param, ty) in parts.params {
            match ty {
                Some(ty) => {
                    self.locals.insert(param, ty);
                }
                None => {
                    self.poisoned.insert(param);
                }
            }
        }
        self.context = Context::Function;
        self.flow.exit = Some(ExitFrame::new(parts.ret.clone(), false));
        match (&f.body, parts.ret) {
            (Some(body), Some(ret)) => self.function_body(body, &ret, data.file),
            (Some(body), None) => {
                self.statements(&body.stmts, None);
            }
            (None, _) => self.unsupported(data.file, data.span, "function without a body"),
        }
        self.context = Context::Module;
    }

    /// The statements of a body against the declared result. A missing result is reported
    /// whatever else the body got wrong, since it is its own mistake.
    fn function_body(&mut self, body: &BlockNode, ret: &Ty, file: FileId) {
        if let Some(found) = self.statements(&body.stmts, Some(ret))
            && found != *ret
            && found != Ty::Never
        {
            self.mismatch(file, body.span, ret, &found);
        }
    }

    /// A module of the reachable set: its params, nodes and placements, then its analog blocks.
    /// The modules it places are reached.
    fn module(&mut self, decl: DeclId) {
        let data = self.design.decl_data(decl).clone();
        let DeclKind::Module(m) = &data.kind else { return };
        if data.generics_span.is_some() {
            self.unsupported(data.file, data.span, "generic module");
            return;
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
                let item = self.design.decl_data(child).clone();
                match (&item.kind, phase) {
                    (DeclKind::Port(p), ItemPhase::Declarations) => self.port(child, &p.clone(), item.file),
                    (DeclKind::Param(p), ItemPhase::Declarations) => self.param(child, &p.clone()),
                    (DeclKind::Signal(s), ItemPhase::Declarations) => self.signal(child, &s.clone()),
                    (DeclKind::Let(l), ItemPhase::Lets) => self.module_let(&l.clone()),
                    (DeclKind::Definition(d), ItemPhase::Definitions) => self.definition(&d.clone()),
                    (DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_) | DeclKind::Let(_) | DeclKind::Definition(_), _) => {}
                    (DeclKind::GenericParam(_) | DeclKind::Behavior(_) | DeclKind::Error, _) => {}
                    (DeclKind::Generation(_), ItemPhase::Declarations) => self.unsupported(item.file, item.span, "generation `for`/`if` in a module body"),
                    (other, ItemPhase::Declarations) => self.unsupported(item.file, item.span, &format!("{} inside a module", other.label().to_lowercase())),
                    (_, _) => {}
                }
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
                    self.analog_reads(&bd.body);
                }
                BehaviorKind::Digital => self.digital_block(behavior),
            }
        }
        // A placement no statement above typed (one written where only the module lists it).
        for instance in m.instances.iter().copied() {
            if self.exprs[instance.index()].is_none() {
                self.infer(instance);
            }
        }
        self.handed_drives();
    }
}
