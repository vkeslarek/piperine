//! The declaration worklist: from the top module, every module it places, every function it
//! calls and every constant it reads is checked once, in first-reach order.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::msl::{Diagnostic, Span};
use crate::pom::{BehaviorKind, BlockNode, DeclData, DeclId, DeclKind, Design, DesignDiagnostic, ExprId, FileId, FnParamData, Resolution, TypeId, TypeKind};

use super::unsupported::Unsupported;
use super::{Callee, Method, Ty, Types};

/// Where the expression being checked is written: the §14 table allows a core primitive by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Context {
    /// A module body.
    Module,
    /// An `analog` block.
    Analog,
    /// A `fn` or a closure.
    Function,
}

/// One pass over the reachable set. Everything it learns goes into side tables; the design is
/// only read.
pub(super) struct Checker<'d> {
    pub(super) design: &'d Design,
    pub(super) exprs: Vec<Option<Ty>>,
    pub(super) locals: BTreeMap<DeclId, Ty>,
    pub(super) methods: BTreeMap<ExprId, Method>,
    pub(super) callees: BTreeMap<ExprId, Callee>,
    pub(super) reached: Vec<DeclId>,
    pub(super) context: Context,
    /// Bindings whose value could not be typed: a read of one is silent, so one mistake is
    /// one diagnostic.
    pub(super) poisoned: BTreeSet<DeclId>,
    queued: BTreeSet<DeclId>,
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
            reached: Vec::new(),
            context: Context::Module,
            poisoned: BTreeSet::new(),
            queued: BTreeSet::new(),
            queue: VecDeque::new(),
            diagnostics: Vec::new(),
            reported: BTreeSet::new(),
        }
    }

    /// Check `top` and everything it reaches: the table, or the diagnostics in source order.
    pub(super) fn run(mut self, top: DeclId) -> Result<Types<'d>, Vec<DesignDiagnostic>> {
        self.reach(top);
        while let Some(decl) = self.queue.pop_front() {
            self.declaration(decl);
        }
        if !self.diagnostics.is_empty() {
            let mut diagnostics = self.diagnostics;
            diagnostics.sort_by_key(|d| (d.file, d.diagnostic.span().start));
            return Err(diagnostics);
        }
        Ok(Types { design: self.design, exprs: self.exprs, locals: self.locals, methods: self.methods, callees: self.callees, reached: self.reached })
    }

    /// Put a declaration on the worklist the first time it is reached.
    pub(super) fn reach(&mut self, decl: DeclId) {
        if self.queued.insert(decl) {
            self.reached.push(decl);
            self.queue.push_back(decl);
        }
    }

    fn declaration(&mut self, decl: DeclId) {
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
        let node = self.design.type_node(ty).clone();
        let file = self.design.decl_data(node.owner).file;
        match &node.kind {
            TypeKind::Path(_) => match self.design.type_resolution(ty) {
                Resolution::Builtin(id) => match (id.info().name, id.width()) {
                    ("fN", Some(64)) => Some(Ty::F64),
                    ("bool", _) => Some(Ty::Bool),
                    ("uN" | "iN" | "usize" | "isize", _) => {
                        self.unsupported(file, node.span, &format!("integer type `{}`", id.name()));
                        None
                    }
                    _ => {
                        self.unsupported(file, node.span, &format!("type `{}`", id.name()));
                        None
                    }
                },
                Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Nature(_)) => Some(Ty::Node(d)),
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

    // ---- declarations ----------------------------------------------------------------------

    /// `const NAME: T = value;`: typed by its declared type, its initializer checked once.
    fn constant(&mut self, decl: DeclId) {
        let DeclKind::Const(data) = self.design.decl_data(decl).kind.clone() else { return };
        let Some(ty) = self.lower_type(data.ty) else { return };
        match data.value {
            Some(value) => self.check(value, &ty),
            None => {
                let d = self.design.decl_data(decl);
                let (file, span) = (d.file, d.span);
                self.unsupported(file, span, "`const` without a value");
            }
        }
    }

    /// The declared parameter types and result of a function, or `None` after a `P0001`.
    pub(super) fn signature(&mut self, decl: DeclId) -> Option<(Vec<Ty>, Ty)> {
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
            match p {
                FnParamData::Named { ty } => match self.lower_type(*ty) {
                    Some(t) => params.push(t),
                    None => ok = false,
                },
                FnParamData::SelfValue | FnParamData::Error => {
                    let d = self.design.decl_data(child);
                    self.unsupported(d.file, d.span, "`self` parameter");
                    ok = false;
                }
            }
        }
        let ret = match f.ret {
            Some(r) => self.lower_type(r),
            None => Some(Ty::Unit),
        };
        match (ok, ret) {
            (true, Some(ret)) => Some((params, ret)),
            _ => None,
        }
    }

    /// A function of the reachable set: its body against its declared result. A call to it is
    /// typed from the signature alone, so a recursive function never loops.
    fn function(&mut self, decl: DeclId) {
        let Some((params, ret)) = self.signature(decl) else { return };
        let data = self.design.decl_data(decl).clone();
        let DeclKind::Fn(f) = &data.kind else { return };
        let named = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_)));
        for (param, ty) in named.zip(params) {
            self.locals.insert(param, ty);
        }
        self.context = Context::Function;
        match &f.body {
            Some(body) => self.function_body(body, &ret, data.file),
            None => self.unsupported(data.file, data.span, "function without a body"),
        }
        self.context = Context::Module;
    }

    fn function_body(&mut self, body: &BlockNode, ret: &Ty, file: FileId) {
        let before = self.diagnostics.len();
        // A body that already raised a diagnostic is not also compared with its result: one
        // mistake is one diagnostic.
        if let Some(found) = self.statements(&body.stmts, Some(ret))
            && found != *ret
            && self.diagnostics.len() == before
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
        if let Some(ret) = m.ret {
            let span = self.design.type_node(ret).span;
            self.unsupported(data.file, span, "module result `-> T`");
        }
        for child in data.children.iter().copied() {
            let item = self.design.decl_data(child).clone();
            match &item.kind {
                DeclKind::Port(p) => self.port(&p.clone(), item.file),
                DeclKind::Param(p) => self.param(&p.clone()),
                DeclKind::Signal(s) => self.signal(&s.clone()),
                DeclKind::Let(l) => self.module_let(&l.clone()),
                DeclKind::GenericParam(_) | DeclKind::Behavior(_) | DeclKind::Error => {}
                DeclKind::Generation(_) => self.unsupported(item.file, item.span, "generation `for`/`if` in a module body"),
                DeclKind::Definition(_) => self.unsupported(item.file, item.span, "definition `=`"),
                other => self.unsupported(item.file, item.span, &format!("{} inside a module", other.label().to_lowercase())),
            }
        }
        for behavior in m.behaviors.iter().copied() {
            let b = self.design.decl_data(behavior).clone();
            let DeclKind::Behavior(bd) = &b.kind else { continue };
            match bd.kind {
                BehaviorKind::Analog => {
                    self.context = Context::Analog;
                    self.statements(&bd.body, None);
                    self.context = Context::Module;
                }
                BehaviorKind::Digital => self.unsupported(b.file, b.span, "`digital` block"),
            }
        }
        // A placement no statement above typed (one written where only the module lists it).
        for instance in m.instances.iter().copied() {
            if self.exprs[instance.index()].is_none() {
                self.infer(instance);
            }
        }
        if let (Some(result), None) = (m.result, m.ret) {
            let file = self.file_of_expr(result);
            let span = self.design.expr_node(result).span;
            self.unsupported(file, span, "module result");
        }
    }
}
