//! Records (part VI §19; DIG-11 AC 1 to 3): `emit!("stream", f = e, …)` in a reaction is one
//! `Stmt::Emit` naming the body's stream and its fields in source order. A string literal field
//! is text kept on the site, never sent (abi.md C5); every other field is a value field of the
//! stream, typed by its first site. A `str` is text known at elaboration (§1.2.9): a literal, a
//! `const` of that type, a `let` bound to a text or a `str` param of an inlined call; `==` and `!=`
//! between texts are known at elaboration too. A function that reaches `emit!` (`log`, `require`)
//! or takes a `str` is no IR function, IR functions being pure values: it is inlined at its call,
//! its `str` params bound to the texts the call gives and its other params to locals of the
//! reaction holding the arguments. A `str` that is no text after inlining is `P0001`.

use piperine_ir::{EmitField, Expr, ExprId as LoweredExpr, Origin, Stmt, Stream, StreamField, StmtId as LoweredStmt, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{BinOp, BlockNode, DeclId, DeclKind, ExprId, ExprKind, FieldInit, Lit, Resolution};
use crate::types::{Callee, Ty};

impl Elaboration<'_, '_> {
    /// When `expr`, a statement of a block, is `emit!` or a call of a function that emits: its
    /// statements appended to `out`, and `Some(false)` after a diagnostic. `None` for any other
    /// expression.
    pub(super) fn lower_record_statement(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, expr: ExprId) -> Option<bool> {
        let expr = self.peel_parens(expr);
        let node = self.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Emit { stream, fields } => Some(self.lower_emit(scope, out, stream, fields)),
            ExprKind::Call { args, .. } => match self.types.callee(expr) {
                Some(Callee::Function(function)) if self.is_inlined(function) => {
                    let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                    Some(self.inline_emitting(scope, out, function, &args))
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether a call of `function` is inlined where it stands: it reaches `emit!` (§19 rule 10) or
    /// takes a `str`, a text that exists at elaboration only (§1.2.9).
    pub(super) fn is_inlined(&self, function: DeclId) -> bool {
        let takes_text = self.design.decl_data(function).children.iter().any(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_)) && matches!(self.typed_local(*c), Some(Ty::Str)));
        self.types.emits(function).is_some() || takes_text || self.takes_handle(function)
    }

    /// `emit!(stream, fields)`: one site of the body's stream named `stream`.
    fn lower_emit(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, stream: &Lit, fields: &[FieldInit]) -> bool {
        let name = stream.str_value().unwrap_or_default();
        let index = match self.structure.streams.iter().position(|s| s.name == name) {
            Some(at) => at,
            None => {
                self.structure.streams.push(Stream { name: name.clone(), fields: Vec::new() });
                self.structure.streams.len() - 1
            }
        };
        let mut lowered = Vec::new();
        let mut ok = true;
        for field in fields {
            if matches!(self.typed(field.value), Some(Ty::Str)) {
                match self.text_of(scope, field.value) {
                    Some(text) => lowered.push(EmitField::Text { name: field.name.text.clone(), text }),
                    // A file with no text was reported as `E0612` or `E0613` by `read_included_files`.
                    None if self.names_unreadable_file(field.value) => ok = false,
                    None => {
                        let span = self.design.expr_node(field.value).span;
                        self.unsupported(scope.file, span, "a `str` that is not a text after inlining");
                        ok = false;
                    }
                }
                continue;
            }
            // an `Option` field is its presence and its payload, packed as one value (§19 rule 11, D8)
            let lowered_value = match self.typed(field.value) {
                Some(Ty::Option(payload)) => match self.value_type(&payload) {
                    Some(payload) => self.lower_stored(scope, field.value, &ValueType::Option(Box::new(payload))),
                    None => {
                        let span = self.design.expr_node(field.value).span;
                        self.unsupported(scope.file, span, "an `Option` record field whose payload has no value in the IR");
                        None
                    }
                },
                _ => self.lower_expr(scope, field.value),
            };
            let Some(value) = lowered_value else {
                ok = false;
                continue;
            };
            let ty = self.type_of(value);
            let fields_of = &mut self.structure.streams[index].fields;
            let at = match fields_of.iter().position(|f| f.name == field.name.text) {
                Some(at) => at,
                None => {
                    fields_of.push(StreamField { name: field.name.text.clone(), ty });
                    fields_of.len() - 1
                }
            };
            lowered.push(EmitField::Value { field: at as u32, value });
        }
        if ok {
            out.push(self.stmt(Stmt::Emit { stream: index as u32, fields: lowered }));
        }
        ok
    }

    /// The text a `str` expression stands for after inlining: a string literal, a `const` of type
    /// `str` (its literal, or the file an `include_str!` reads, once, §20 rule 3), a `let` bound to a
    /// text, or a `str` param of an inlined function bound to one.
    pub(super) fn text_of(&mut self, scope: &Scope, expr: ExprId) -> Option<String> {
        let design = self.design;
        match &design.expr_node(self.peel_parens(expr)).kind {
            ExprKind::Lit(lit) => lit.str_value(),
            ExprKind::Path(path) => match design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => match &design.decl_data(decl).kind {
                    DeclKind::Const(constant) => match constant.value {
                        Some(value) if matches!(design.expr_node(value).kind, ExprKind::IncludeStr(_)) => self.consts.text(decl).ok().map(|text| text.to_string()),
                        Some(value) => self.text_of(scope, value),
                        None => None,
                    },
                    _ => scope.texts.get(&decl).cloned(),
                },
                _ => None,
            },
            _ => None,
        }
    }

    /// `let name = text;` of type `str`: the name stands for the text, and no local holds it. `false`
    /// after a diagnostic.
    pub(super) fn lower_text_let(&mut self, scope: &mut Scope, binding: DeclId, init: ExprId) -> bool {
        match self.text_of(scope, init) {
            Some(text) => {
                scope.texts.insert(binding, text);
                true
            }
            None => {
                let span = self.design.expr_node(init).span;
                self.unsupported(scope.file, span, "a `str` that is not a text after inlining");
                false
            }
        }
    }

    /// `a == b` and `a != b` of two `str`: known at elaboration, the `bool` of whether the texts
    /// are the same. `None` after a diagnostic.
    pub(super) fn lower_text_comparison(&mut self, scope: &Scope, op: BinOp, lhs: ExprId, rhs: ExprId, origin: Origin) -> Option<LoweredExpr> {
        let (Some(left), Some(right)) = (self.text_of(scope, lhs), self.text_of(scope, rhs)) else {
            let span = self.design.expr_node(lhs).span;
            self.unsupported(scope.file, span, "a `str` that is not a text after inlining");
            return None;
        };
        Some(self.emit(Expr::Bool((left == right) == (op == BinOp::Eq)), ValueType::Bool, origin))
    }

    /// A call of `function`, which reaches `emit!`, inlined where it stands as statements of
    /// `out`; its value, if any, ends them as an expression statement. `false` after a
    /// diagnostic.
    fn inline_emitting(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, function: DeclId, args: &[ExprId]) -> bool {
        let Some((stmts, value)) = self.inline_emitting_parts(scope, function, args) else { return false };
        out.extend(stmts);
        if let Some(value) = value {
            out.push(self.stmt(Stmt::Expr(value)));
        }
        true
    }

    /// A call of `function`, which reaches `emit!`, in a value position (`q <- noisy()`,
    /// `u && noisy()`): inlined as a block whose statements are the call's, so the `emit!` runs
    /// in the reaction where the call stands (§19 rule 10), and whose value is the function's.
    /// `None` after a diagnostic.
    pub(super) fn inline_emitting_value(&mut self, scope: &mut Scope, function: DeclId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        if self.takes_handle(function) && self.reaches_recursion(function) {
            let span = self.design.decl_data(function).span;
            self.unfoldable_recursion(scope, function, span, "a recursion through a function that takes a handle is not inlined");
            return None;
        }
        let (stmts, value) = self.inline_emitting_parts(scope, function, args)?;
        let ty = value.map_or(ValueType::Unit, |v| self.type_of(v));
        Some(self.emit(Expr::Block { stmts, value }, ty, origin))
    }

    /// The statements and the value of the body of `function` inlined for a call with `args`:
    /// each `&str` param bound to its text, each other param to a local holding its argument,
    /// then the body's statements. `None` after a diagnostic.
    fn inline_emitting_parts(&mut self, scope: &mut Scope, function: DeclId, args: &[ExprId]) -> Option<(Vec<LoweredStmt>, Option<LoweredExpr>)> {
        let (mut out, body) = self.bind_inlined_arguments(scope, function, args)?;
        let (stmts, value) = self.lower_statements(scope, &body.stmts)?;
        out.extend(stmts);
        Some((out, value))
    }

    /// The `let`s that bind the params of `function` for a call with `args` (each `str` param to its
    /// text, each other param to a local holding its argument), and the body to inline after them.
    /// `None` after a diagnostic.
    pub(super) fn bind_inlined_arguments(&mut self, scope: &mut Scope, function: DeclId, args: &[ExprId]) -> Option<(Vec<LoweredStmt>, BlockNode)> {
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let body = f.body.clone()?;
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let params: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        let mut out = Vec::new();
        let mut ok = true;
        for (param, arg) in params.iter().zip(args) {
            let param_name = self.design.decl_data(*param).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            if self.types.handle(*param) {
                scope.handles.insert(*param, self.handle_argument_target(scope, *arg));
                continue;
            }
            if matches!(self.typed(*arg), Some(Ty::Str)) {
                match self.text_of(scope, *arg) {
                    Some(text) => {
                        scope.texts.insert(*param, text);
                    }
                    None => {
                        let span = self.design.expr_node(*arg).span;
                        self.unsupported(scope.file, span, "a `str` that is not a text after inlining");
                        ok = false;
                    }
                }
                continue;
            }
            let Some(value) = self.lower_expr(scope, *arg) else {
                ok = false;
                continue;
            };
            let local = scope.locals.len() as u32;
            let ty = self.type_of(value);
            scope.locals.push(piperine_ir::Local { name: format!("{name}.{param_name}"), ty, mutable: false });
            scope.local_index.insert(*param, local);
            out.push(self.stmt(Stmt::Let { local, init: value }));
        }
        if !ok {
            return None;
        }
        Some((out, body))
    }
}
