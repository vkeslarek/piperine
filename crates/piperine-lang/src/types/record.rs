//! `emit!` (DIG-05 AC 11 to 15, part VI §19): the one output statement, `emit!("stream", f = e,
//! ..)`, and the functions that reach it (`log`, `require`). An `emit!` is allowed in a reaction
//! and in `analog`; a `fn` that reaches it is callable only there, so a definition, a body
//! `let` or a const context that reaches one is `E0601`. A site reached from `analog` that writes
//! `sim` or `checks` is `E0603`. A field name is distinct in one `emit!` and is not `maybe`
//! (`E0604`); a field has one type per stream in a module, and the type the stdlib gives it on a
//! reserved stream (`E0605`); a field is a value (`E0606`).

use std::collections::{BTreeMap, BTreeSet};

use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FieldInit, FileId, Lit, Resolution};

use super::check::{Checker, Context};
use super::Ty;

/// One `emit!` of the reachable set: where it is written, its stream and its typed fields.
#[derive(Debug)]
struct Site {
    owner: DeclId,
    stream: String,
    fields: Vec<(String, Ty, Span, FileId)>,
}

/// What the checker knows of `emit!`.
#[derive(Debug, Default)]
pub(super) struct Records {
    sites: Vec<Site>,
    /// The emitting functions each module or function calls, in call order.
    calls: BTreeMap<DeclId, Vec<DeclId>>,
    /// The streams each function writes, directly or through the functions it calls.
    streams: BTreeMap<DeclId, BTreeSet<String>>,
    /// The functions whose streams are being collected, so a recursion ends.
    open: Vec<DeclId>,
}

impl Records {
    /// The streams of every function that writes one.
    pub(super) fn emitting(&self) -> BTreeMap<DeclId, BTreeSet<String>> {
        self.streams.iter().filter(|(_, s)| !s.is_empty()).map(|(f, s)| (*f, s.clone())).collect()
    }
}

/// The type the stdlib gives a field of a reserved stream (§19 rule 15, §22.6).
fn reserved_field(stream: &str, field: &str) -> Option<Ty> {
    match (stream, field) {
        ("sim", "finish" | "failed") | ("checks", "pass") => Some(Ty::Bool),
        ("sim", "msg") | ("checks", "kind" | "name") => Some(Ty::Str),
        ("checks", "margin") => Some(Ty::F64),
        _ => None,
    }
}

fn is_reserved(stream: &str) -> bool {
    matches!(stream, "sim" | "checks")
}

impl Checker<'_> {
    /// `emit!("stream", f = e, ..)`: an expression of type `()`.
    pub(super) fn emit(&mut self, expr: ExprId, stream: &Lit, fields: &[FieldInit]) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        let name = stream.str_value().unwrap_or_default();
        match self.context {
            Context::Reaction | Context::Analog | Context::Function => {}
            Context::Module | Context::Const => {
                let message = "`emit!` is written in a reaction, in `analog` or in a `fn` only (§19 rule 10)".to_string();
                self.coded(file, "E0601", span, message, "`emit!` out of place");
            }
        }
        if self.context == Context::Analog && is_reserved(&name) {
            self.analog_writes_reserved(file, span, &name);
        }
        let mut seen = BTreeSet::new();
        let mut typed = Vec::new();
        for field in fields {
            // An event is judged below, as a field that is not a value (`E0606`).
            self.event_slot = Some(field.value);
            let found = self.infer(field.value);
            let field_name = field.name.text.clone();
            if field_name == "maybe" || !seen.insert(field_name.clone()) {
                let message = if field_name == "maybe" { "the field name `maybe` is reserved (§19 rule 11)".to_string() } else { format!("the field `{field_name}` is written twice in one `emit!` (§19 rule 11)") };
                self.coded(file, "E0604", field.span, message, "a field name that is not free");
                continue;
            }
            let Some(found) = found else { continue };
            if !(found.has_repr() || found == Ty::Str) {
                let message = format!("the field `{field_name}` has type `{}`, which is not a value type (§19 rule 11)", found.name(self.design));
                self.coded(file, "E0606", field.span, message, "not a value");
                continue;
            }
            if let Some(want) = reserved_field(&name, &field_name)
                && want != found
            {
                let message = format!("the field `{field_name}` of the stream `{name}` has type `{}`: the stdlib gives it `{}` (§19 rule 15)", found.name(self.design), want.name(self.design));
                self.coded(file, "E0605", field.span, message, "type of a reserved field");
                continue;
            }
            typed.push((field_name, found, field.span, file));
        }
        if let Some(owner) = self.owner {
            self.records.sites.push(Site { owner, stream: name, fields: typed });
        }
        Some(Ty::Unit)
    }

    fn analog_writes_reserved(&mut self, file: FileId, span: Span, stream: &str) {
        let message = format!("a site reached from `analog` writes the stream `{stream}`: only a reaction decides what the host reads there (§19 rules 4 and 14)");
        self.coded(file, "E0603", span, message, "`analog` writes a reserved stream");
    }

    /// A call of `function`, which may reach `emit!`: where it stands is where `emit!` may stand
    /// (`E0601`), and from `analog` it writes neither `sim` nor `checks` (`E0603`).
    pub(super) fn emitting_call(&mut self, expr: ExprId, function: DeclId) {
        let streams = self.function_streams(function);
        if streams.is_empty() {
            return;
        }
        if let Some(owner) = self.owner {
            self.records.calls.entry(owner).or_default().push(function);
        }
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        match self.context {
            Context::Module | Context::Const => {
                let name = self.design.decl_data(function).name.as_ref().map_or("", |n| n.text.as_str()).to_string();
                let message = format!("`{name}` reaches `emit!`: it is called where `emit!` may be written, in a reaction or in `analog`, never in a definition, a body `let` or a const context (§19 rule 10)");
                self.coded(file, "E0601", span, message, "an emitting `fn` out of place");
            }
            Context::Analog => {
                if let Some(stream) = streams.iter().find(|s| is_reserved(s)) {
                    self.analog_writes_reserved(file, span, &stream.clone());
                }
            }
            Context::Reaction | Context::Function => {}
        }
    }

    /// The streams `function` writes, directly or through the functions it calls: read off the
    /// source, so a function not checked yet answers too.
    fn function_streams(&mut self, function: DeclId) -> BTreeSet<String> {
        if let Some(known) = self.records.streams.get(&function) {
            return known.clone();
        }
        if self.records.open.contains(&function) {
            return BTreeSet::new();
        }
        let DeclKind::Fn(f) = &self.design.decl_data(function).kind else { return BTreeSet::new() };
        let Some(body) = f.body.clone() else { return BTreeSet::new() };
        self.records.open.push(function);
        let mut found = BTreeSet::new();
        let mut all = Vec::new();
        for e in crate::pom::block_exprs(self.design, &body) {
            crate::pom::preorder(self.design, e, &mut all);
        }
        for e in all {
            match &self.design.expr_node(e).kind {
                ExprKind::Emit { stream, .. } => {
                    found.insert(stream.str_value().unwrap_or_default());
                }
                ExprKind::Call { callee, .. } => {
                    if let Some(Resolution::Decl(g)) = self.callee_resolution(*callee)
                        && matches!(self.design.decl_data(g).kind, DeclKind::Fn(_))
                    {
                        found.extend(self.function_streams(g));
                    }
                }
                _ => {}
            }
        }
        self.records.open.pop();
        self.records.streams.insert(function, found.clone());
        found
    }

    /// One field has one type per stream in a module, the sites of the functions it calls
    /// included (`E0605`, §19 rule 12): the library's sites are read first, so the one that
    /// disagrees is the module's own.
    pub(super) fn record_conflicts(&mut self) {
        let modules: Vec<DeclId> = self.reached.iter().copied().filter(|d| matches!(self.design.decl_data(*d).kind, DeclKind::Module(_))).collect();
        for module in modules {
            let mut owners = Vec::new();
            self.called_from(module, &mut owners);
            owners.push(module);
            let mut types: BTreeMap<(String, String), Ty> = BTreeMap::new();
            for owner in owners {
                let sites: Vec<(String, Vec<(String, Ty, Span, FileId)>)> = self.records.sites.iter().filter(|s| s.owner == owner).map(|s| (s.stream.clone(), s.fields.clone())).collect();
                for (stream, fields) in sites {
                    for (field, ty, span, file) in fields {
                        match types.get(&(stream.clone(), field.clone())) {
                            Some(first) if *first != ty => {
                                let message = format!("the field `{field}` of the stream `{stream}` has type `{}` here and `{}` at another site (§19 rule 12)", ty.name(self.design), first.name(self.design));
                                self.coded(file, "E0605", span, message, "a field with two types");
                            }
                            Some(_) => {}
                            None => {
                                types.insert((stream.clone(), field), ty);
                            }
                        }
                    }
                }
            }
        }
    }

    /// The functions `owner` calls that reach `emit!`, transitively, the deepest first.
    fn called_from(&self, owner: DeclId, out: &mut Vec<DeclId>) {
        for callee in self.records.calls.get(&owner).into_iter().flatten() {
            if !out.contains(callee) {
                self.called_from(*callee, out);
                if !out.contains(callee) {
                    out.push(*callee);
                }
            }
        }
    }
}
