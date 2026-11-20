//! Records (part VI §19; DIG-11 AC 1 to 3): `emit!("stream", f = e, …)` in a reaction is one
//! `Stmt::Emit` naming the body's stream and its fields in source order. A string literal field
//! is text kept on the site, never sent (abi.md C5); every other field is a value field of the
//! stream, typed by its first site. A function that reaches `emit!` (`log`, `require`) is no IR
//! function, IR functions being pure values: it is inlined at its call, its `&str` params bound
//! to the literals the call gives and its other params to locals of the reaction holding the
//! arguments. A `&str` field that is no literal after inlining is `P0001`.

use piperine_ir::{EmitField, Expr, ExprId as LoweredExpr, Origin, Stmt, Stream, StreamField, StmtId as LoweredStmt, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, FieldInit, Lit, Resolution};
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
                Some(Callee::Function(function)) if self.types.emits(function).is_some() => {
                    let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
                    Some(self.inline_emitting(scope, out, function, &args))
                }
                _ => None,
            },
            _ => None,
        }
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
            if matches!(self.types.ty(field.value), Some(Ty::Str)) {
                match self.text_of(scope, field.value) {
                    Some(text) => lowered.push(EmitField::Text { name: field.name.text.clone(), text }),
                    None => {
                        let span = self.design.expr_node(field.value).span;
                        self.unsupported(scope.file, span, "a `&str` field that is not a literal after inlining");
                        ok = false;
                    }
                }
                continue;
            }
            let Some(value) = self.lower_expr(scope, field.value) else {
                ok = false;
                continue;
            };
            let ty = match self.type_of(value) {
                piperine_ir::ValueType::Flagged(inner) => *inner,
                other => other,
            };
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

    /// The text a `&str` expression stands for after inlining: a string literal, or a `&str`
    /// param of an inlined function bound to one.
    fn text_of(&self, scope: &Scope, expr: ExprId) -> Option<String> {
        match &self.design.expr_node(self.peel_parens(expr)).kind {
            ExprKind::Lit(lit) => lit.str_value(),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Decl(decl)) => scope.texts.get(&decl).cloned(),
                _ => None,
            },
            _ => None,
        }
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
        let (stmts, value) = self.inline_emitting_parts(scope, function, args)?;
        let ty = value.map_or(ValueType::Unit, |v| self.type_of(v));
        Some(self.emit(Expr::Block { stmts, value }, ty, origin))
    }

    /// The statements and the value of the body of `function` inlined for a call with `args`:
    /// each `&str` param bound to its text, each other param to a local holding its argument,
    /// then the body's statements. `None` after a diagnostic.
    fn inline_emitting_parts(&mut self, scope: &mut Scope, function: DeclId, args: &[ExprId]) -> Option<(Vec<LoweredStmt>, Option<LoweredExpr>)> {
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let body = f.body.clone()?;
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let params: Vec<DeclId> = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        let mut out = Vec::new();
        let mut ok = true;
        for (param, arg) in params.iter().zip(args) {
            let param_name = self.design.decl_data(*param).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            if matches!(self.types.ty(*arg), Some(Ty::Str)) {
                match self.text_of(scope, *arg) {
                    Some(text) => {
                        scope.texts.insert(*param, text);
                    }
                    None => {
                        let span = self.design.expr_node(*arg).span;
                        self.unsupported(scope.file, span, "a `&str` field that is not a literal after inlining");
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
            scope.locals.push(piperine_ir::Local { name: format!("{name}.{param_name}"), ty });
            scope.local_index.insert(*param, local);
            out.push(self.stmt(Stmt::Let { local, init: value }));
        }
        if !ok {
            return None;
        }
        let (stmts, value) = self.lower_statements(scope, &body.stmts)?;
        out.extend(stmts);
        Some((out, value))
    }
}
