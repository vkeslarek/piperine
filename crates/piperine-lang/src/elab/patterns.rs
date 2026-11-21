//! Patterns as tests (part I §1.6): one pattern over a subject is a `bool` and the names it
//! binds. The IR has no pattern, so a `match` is an `if` chain (`compound.rs`) whose tests are
//! these, `if let` and `while let` are one arm and `let … else` leaves when the test fails.
//!
//! - a literal or a path (`3`, `Mode::Hold`, a `const`) is `subject == value`;
//! - a range `a..=b`, `a..b`, `a..` is `subject >= a && subject <= b` (`<` for `..b`);
//! - a name binds the subject, `name @ p` binds it when `p` matches, `_` matches everything;
//! - `p | q` is the disjunction, binding the names of each alternative;
//! - over an `Option`, `None` is `!is` and `Some(p)` is `is` and `p` over the payload.
//!
//! A test over a subject that may be unknown uses the bitwise `&` and `|`, which §3.3.12 gives the
//! table of `&&` and `||` (`known.rs`). A pattern that matches every value has no test.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Local, Origin, Stmt, StmtId as LoweredStmt, UnaryOp, ValueType};

use super::compound::EnumLayout;
use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use super::selectors::ArmCondition;
use crate::msl::Span;
use crate::pom::{BlockNode, DeclId, DeclKind, ExprId, ExprKind, FileId, LitKind, PatId, PatKind, Resolution, StmtId};
use crate::types::Ty;

/// The scrutinee of a `match`, as each test reads it.
pub(super) enum Subject {
    /// A local, a param or a literal: lowered again for each test.
    Reread(ExprId),
    /// Bound to this local of this type.
    Local(u32, ValueType),
    /// An `Option`, bound to its flag local and its payload local.
    Pair { is: (u32, ValueType), val: (u32, ValueType) },
    /// An `Option` in code with no local (a drive): its pair lowered again for each test.
    RereadPair(ExprId),
    /// An `Option` already lowered to its flag and payload expressions, which each test shares: no
    /// local holds it (`if let` over a pair read where it stands).
    Shared { is: LoweredExpr, val: LoweredExpr },
    /// A value already lowered, which each test shares: the payload of a `Shared` pair.
    Value(LoweredExpr),
}

/// What one pattern asks of its subject.
pub(super) struct PatternTest {
    /// The condition under which the pattern matches; `None` when every value does.
    pub(super) test: Option<LoweredExpr>,
    /// Each name the pattern binds, with the value it takes.
    pub(super) bindings: Vec<(DeclId, LoweredExpr)>,
}

impl PatternTest {
    /// The pattern that matches every value and binds nothing.
    fn everything() -> PatternTest {
        PatternTest { test: None, bindings: Vec::new() }
    }

    /// The pattern that matches when `test` holds and binds nothing.
    fn when(test: LoweredExpr) -> PatternTest {
        PatternTest { test: Some(test), bindings: Vec::new() }
    }
}

impl Elaboration<'_, '_> {
    /// The test pattern `pat` makes of `subject`, and the names it binds. `None` after a diagnostic.
    pub(super) fn test_pattern(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId) -> Option<PatternTest> {
        let node = self.design.pat_node(pat).clone();
        let origin = self.locator.origin(scope.file, node.span);
        if let PatKind::Or(alternatives) = &node.kind {
            return self.test_alternatives(scope, subject, alternatives, origin);
        }
        if let Subject::Pair { .. } | Subject::RereadPair(_) | Subject::Shared { .. } = subject {
            return self.option_test(scope, subject, pat);
        }
        match &node.kind {
            PatKind::Wild | PatKind::Rest => Some(PatternTest::everything()),
            // A name binds the subject; `name @ p` binds it when `p` matches.
            PatKind::Ident { res: Resolution::Unresolved, binding, sub, .. } => {
                let mut found = match sub {
                    Some(inner) => self.test_pattern(scope, subject, *inner)?,
                    None => PatternTest::everything(),
                };
                let value = self.read_subject(scope, subject, origin)?;
                found.bindings.push((*binding, value));
                Some(found)
            }
            PatKind::Range { lo, hi, inclusive } => self.test_range(scope, subject, *lo, *hi, *inclusive, origin),
            // A tuple, a tuple struct and a struct: each sub-pattern over its field (`aggregate.rs`).
            PatKind::Tuple(_) | PatKind::TupleStruct { .. } | PatKind::Struct { .. } => {
                let read = self.read_subject(scope, subject, origin.clone())?;
                let base = self.base_of(read);
                match base {
                    ValueType::Enum(_) => self.test_variant_pattern(scope, subject, pat, &base, origin),
                    ValueType::Tuple(_) | ValueType::Struct(_) => self.test_aggregate_pattern(scope, subject, pat, &base, origin),
                    _ => {
                        self.unsupported(scope.file, node.span, "a pattern with fields over a value that is not a struct, a tuple or an enum");
                        None
                    }
                }
            }
            PatKind::Array(pats) => {
                let read = self.read_subject(scope, subject, origin.clone())?;
                let base = self.base_of(read);
                self.test_array_pattern(scope, subject, pats, &base, origin)
            }
            PatKind::Lit(expr) => {
                let read = self.read_subject(scope, subject, origin.clone())?;
                match self.bool_literal(*expr) {
                    // `true` is the subject itself, `false` its negation.
                    Some(true) => Some(PatternTest::when(read)),
                    Some(false) => {
                        let flag = self.flag(read);
                        Some(PatternTest::when(self.emit(Expr::Unary { op: UnaryOp::Not, expr: read }, flag.wrap(ValueType::Bool), origin)))
                    }
                    None => {
                        let value = self.lower_expr(scope, *expr)?;
                        Some(PatternTest::when(self.compared(BinaryOp::Eq, read, value, origin)))
                    }
                }
            }
            PatKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                self.test_named_value(scope, subject, res, node.span, origin)
            }
            PatKind::Ident { res: res @ (Resolution::Decl(_) | Resolution::Builtin(_)), sub: None, .. } => self.test_named_value(scope, subject, *res, node.span, origin),
            _ => {
                self.unsupported(scope.file, node.span, "this pattern");
                None
            }
        }
    }

    /// `subject == the value the path names`: a unit variant or a `const`.
    fn test_named_value(&mut self, scope: &mut Scope, subject: &Subject, res: Resolution, span: Span, origin: Origin) -> Option<PatternTest> {
        let read = self.read_subject(scope, subject, origin.clone())?;
        let base = self.base_of(read);
        // A known enum value is tested for its variant; one that may be unknown is compared with it,
        // which is unknown as the value is (§3.3.12).
        if let (Resolution::Decl(variant), ValueType::Enum(_), false) = (res, &base, matches!(self.type_of(read), ValueType::Flagged(_))) {
            if matches!(self.design.decl_data(variant).kind, DeclKind::Variant(_)) {
                let position = EnumLayout::variant_tag(self.design, variant)? as u32;
                return Some(PatternTest::when(self.emit(Expr::IsVariant { expr: read, variant: position }, ValueType::Bool, origin)));
            }
        }
        let value = self.path_value(scope, res, &base, span, origin.clone())?;
        Some(PatternTest::when(self.compared(BinaryOp::Eq, read, value, origin)))
    }

    /// `p | q`: matches when an alternative does and binds the names of each. An alternative that
    /// matches everything makes the whole one.
    fn test_alternatives(&mut self, scope: &mut Scope, subject: &Subject, alternatives: &[PatId], origin: Origin) -> Option<PatternTest> {
        let (mut tests, mut bindings, mut everything) = (Vec::new(), Vec::new(), false);
        for alternative in alternatives {
            let found = self.test_pattern(scope, subject, *alternative)?;
            match found.test {
                Some(test) => tests.push(test),
                None => everything = true,
            }
            bindings.extend(found.bindings);
        }
        if everything {
            return Some(PatternTest { test: None, bindings });
        }
        let flag = self.flags(&tests);
        let op = if flag == Flag::Flagged { BinaryOp::BitOr } else { BinaryOp::Or };
        let mut tests = tests.into_iter();
        let first = tests.next()?;
        let test = tests.fold(first, |lhs, rhs| {
            let ty = self.flags(&[lhs, rhs]).wrap(ValueType::Bool);
            self.emit(Expr::Binary { op, lhs, rhs }, ty, origin.clone())
        });
        Some(PatternTest { test: Some(test), bindings })
    }

    /// `lo..=hi`, `lo..hi`, `lo..` and `..hi` over a scalar subject: the subject read again for
    /// each bound.
    fn test_range(&mut self, scope: &mut Scope, subject: &Subject, lo: Option<ExprId>, hi: Option<ExprId>, inclusive: bool, origin: Origin) -> Option<PatternTest> {
        let mut tests = Vec::new();
        if let Some(lo) = lo {
            let (read, bound) = (self.read_subject(scope, subject, origin.clone())?, self.lower_expr(scope, lo)?);
            tests.push(self.compared(BinaryOp::Ge, read, bound, origin.clone()));
        }
        if let Some(hi) = hi {
            let (read, bound) = (self.read_subject(scope, subject, origin.clone())?, self.lower_expr(scope, hi)?);
            tests.push(self.compared(if inclusive { BinaryOp::Le } else { BinaryOp::Lt }, read, bound, origin.clone()));
        }
        let mut tests = tests.into_iter();
        let Some(first) = tests.next() else { return Some(PatternTest::everything()) };
        let test = tests.fold(first, |lhs, rhs| self.both(lhs, rhs, origin.clone()));
        Some(PatternTest::when(test))
    }

    /// `lhs op rhs`, flagged when an operand is.
    fn compared(&mut self, op: BinaryOp, lhs: LoweredExpr, rhs: LoweredExpr, origin: Origin) -> LoweredExpr {
        let ty = self.flags(&[lhs, rhs]).wrap(ValueType::Bool);
        self.emit(Expr::Binary { op, lhs, rhs }, ty, origin)
    }

    /// `lhs && rhs` of two tests: `&` where either may be unknown (§3.3.12).
    pub(super) fn both(&mut self, lhs: LoweredExpr, rhs: LoweredExpr, origin: Origin) -> LoweredExpr {
        let flag = self.flags(&[lhs, rhs]);
        let op = if flag == Flag::Flagged { BinaryOp::BitAnd } else { BinaryOp::And };
        self.emit(Expr::Binary { op, lhs, rhs }, flag.wrap(ValueType::Bool), origin)
    }

    /// The base type of `expr`, without its flag.
    fn base_of(&self, expr: LoweredExpr) -> ValueType {
        match self.type_of(expr) {
            ValueType::Flagged(inner) => *inner,
            other => other,
        }
    }

    /// A fresh read of the scrutinee.
    pub(super) fn read_subject(&mut self, scope: &mut Scope, subject: &Subject, origin: Origin) -> Option<LoweredExpr> {
        match subject {
            Subject::Reread(expr) => self.lower_expr(scope, *expr),
            Subject::Local(index, ty) => Some(self.emit(Expr::Local(*index), ty.clone(), origin)),
            Subject::Value(expr) => Some(*expr),
            // An `Option` is read through its patterns (`option_test`), never as one value.
            Subject::Pair { .. } | Subject::RereadPair(_) | Subject::Shared { .. } => None,
        }
    }

    /// A fresh read of the flag (`payload` false) or of the payload of an `Option` subject.
    pub(super) fn read_pair(&mut self, scope: &mut Scope, subject: &Subject, payload: bool, origin: Origin) -> Option<LoweredExpr> {
        match subject {
            Subject::Pair { is, val } => {
                let (index, ty) = if payload { val } else { is };
                Some(self.emit(Expr::Local(*index), ty.clone(), origin))
            }
            Subject::RereadPair(expr) => {
                let pair = self.lower_option(scope, *expr)?;
                Some(if payload { pair.val } else { pair.is })
            }
            Subject::Shared { is, val } => Some(if payload { *val } else { *is }),
            Subject::Reread(_) | Subject::Local(..) | Subject::Value(_) => None,
        }
    }

    /// The test of a pattern over an `Option` subject: `None` is `!is`; `Some(p)` is `is` and `p`
    /// over the payload, whose names it binds.
    fn option_test(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId) -> Option<PatternTest> {
        let node = self.design.pat_node(pat).clone();
        let origin = self.locator.origin(scope.file, node.span);
        let present = self.read_pair(scope, subject, false, origin.clone())?;
        let res = match &node.kind {
            PatKind::Path(path) => self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res),
            PatKind::Ident { res, .. } => *res,
            _ => Resolution::Unresolved,
        };
        if matches!(res, Resolution::Builtin(id) if id.info().name == "None") {
            let ty = self.flag(present).wrap(ValueType::Bool);
            return Some(PatternTest::when(self.emit(Expr::Unary { op: UnaryOp::Not, expr: present }, ty, origin)));
        }
        let PatKind::TupleStruct { pats, .. } = &node.kind else {
            self.unsupported(scope.file, node.span, "this pattern of an `Option`");
            return None;
        };
        let [inner] = pats.as_slice() else { return None };
        let payload = match subject {
            Subject::Pair { val, .. } => Subject::Local(val.0, val.1.clone()),
            Subject::Shared { val, .. } => Subject::Value(*val),
            _ => return self.reread_payload_test(scope, subject, present, *inner),
        };
        let found = self.test_pattern(scope, &payload, *inner)?;
        let test = match found.test {
            Some(inner) => self.both(present, inner, origin),
            None => present,
        };
        Some(PatternTest { test: Some(test), bindings: found.bindings })
    }

    /// `Some(p)` over an `Option` in code with no local (a drive): `_` or a value, tested against
    /// the payload read again; a name has no local to bind.
    fn reread_payload_test(&mut self, scope: &mut Scope, subject: &Subject, present: LoweredExpr, inner: PatId) -> Option<PatternTest> {
        let node = self.design.pat_node(inner).clone();
        let origin = self.locator.origin(scope.file, node.span);
        let value = match &node.kind {
            PatKind::Wild => return Some(PatternTest::when(present)),
            PatKind::Ident { res: Resolution::Unresolved, sub: None, .. } => {
                self.unsupported(scope.file, node.span, "a binding inside `Some` in a definition");
                return None;
            }
            PatKind::Lit(expr) => self.lower_expr(scope, *expr)?,
            PatKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                let read = self.read_pair(scope, subject, true, origin.clone())?;
                let base = self.base_of(read);
                self.path_value(scope, res, &base, node.span, origin.clone())?
            }
            PatKind::Ident { res: res @ Resolution::Decl(_), sub: None, .. } => {
                let read = self.read_pair(scope, subject, true, origin.clone())?;
                let base = self.base_of(read);
                self.path_value(scope, *res, &base, node.span, origin.clone())?
            }
            _ => {
                self.unsupported(scope.file, node.span, "this pattern inside `Some` in a definition");
                return None;
            }
        };
        let payload = self.read_pair(scope, subject, true, origin.clone())?;
        let equal = self.compared(BinaryOp::Eq, payload, value, origin.clone());
        Some(PatternTest::when(self.both(present, equal, origin)))
    }

    /// The value a path pattern names: a unit variant or a constant, of the subject's type.
    fn path_value(&mut self, scope: &Scope, res: Resolution, base: &ValueType, span: Span, origin: Origin) -> Option<LoweredExpr> {
        match res {
            Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Variant(_)) => {
                let ValueType::Enum(id) = base else { return None };
                let position = EnumLayout::variant_tag(self.design, d)? as u32;
                Some(self.emit(Expr::VariantOf { ty: *id, variant: position, fields: Vec::new() }, base.clone(), origin))
            }
            Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Const(_)) => match self.consts.constant(d) {
                Ok(value) => Some(self.constant_expr(value, base.clone(), origin)),
                Err(fault) => {
                    self.const_fault(&fault, "a `const` pattern");
                    None
                }
            },
            _ => {
                self.unsupported(scope.file, span, "this name as a pattern");
                None
            }
        }
    }

    /// The value of a `bool` literal pattern; `None` for any other literal.
    fn bool_literal(&self, expr: ExprId) -> Option<bool> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Lit(lit) => match lit.kind {
                LitKind::Bool(b) => Some(b),
                _ => None,
            },
            ExprKind::Paren(inner) => self.bool_literal(*inner),
            _ => None,
        }
    }

    // ---- the names a pattern binds ----------------------------------------------------------

    /// A name a loop's pattern binds that the body assigns (§1.5.12, §1.6.9): `let name = variable;`
    /// into a fresh assignable local at the head of the loop body, which the name then stands
    /// for, so the loop's own variable is never written. `None` when the body does not assign it.
    pub(super) fn assignable_copy(&mut self, scope: &mut Scope, binding: Option<DeclId>, variable: u32) -> Option<LoweredStmt> {
        let binding = binding.filter(|binding| self.types.is_assigned(*binding))?;
        let held = scope.locals[variable as usize].clone();
        let ty = Self::mutable_type(scope, held.ty.clone());
        let copy = scope.locals.len() as u32;
        scope.locals.push(Local { name: held.name, ty: ty.clone(), mutable: true });
        scope.local_index.insert(binding, copy);
        let origin = self.locator.origin(scope.file, self.design.decl_data(binding).span);
        let read = self.emit(Expr::Local(variable), held.ty, origin);
        let init = self.coerced_to(scope, read, &ty);
        Some(self.stmt(Stmt::Let { local: copy, init }))
    }

    /// A local for each name of `bindings`, holding the type of its value: the pairs of (local,
    /// value) that `binding_lets` writes where the names are read. `None` after a diagnostic.
    pub(super) fn bind_names(&mut self, scope: &mut Scope, bindings: &[(DeclId, LoweredExpr)]) -> Option<Vec<(u32, LoweredExpr)>> {
        if scope.no_locals && !bindings.is_empty() {
            let span = self.design.decl_data(bindings[0].0).span;
            self.unsupported(scope.file, span, "a name bound by a pattern in a definition");
            return None;
        }
        let mut names = Vec::with_capacity(bindings.len());
        for (decl, value) in bindings {
            let local = self.bind_local(scope, *decl)?;
            scope.locals[local as usize].ty = self.type_of(*value);
            names.push((local, *value));
        }
        Some(names)
    }

    /// `let name = value;` for each name: a fresh statement each time, so a guard and an arm's
    /// body (two blocks) both bind them.
    pub(super) fn binding_lets(&mut self, names: &[(u32, LoweredExpr)]) -> Vec<LoweredStmt> {
        names.iter().map(|(local, init)| self.stmt(Stmt::Let { local: *local, init: *init })).collect()
    }

    // ---- refutable patterns in conditions ---------------------------------------------------

    /// `scrutinee` read once, as the subject of patterns, and the statements that bind it: a local,
    /// a param or a literal is read again where a test needs it; anything else is bound to a local
    /// first; an `Option` to its flag and payload locals, or, with `share_option`, kept as the two
    /// expressions of its pair.
    pub(super) fn subject_of(&mut self, scope: &mut Scope, scrutinee: ExprId, share_option: bool) -> Option<(Subject, Vec<LoweredStmt>)> {
        let option = matches!(self.types.ty(scrutinee), Some(Ty::Option(_)));
        if option && scope.no_locals {
            return Some((Subject::RereadPair(scrutinee), Vec::new()));
        }
        if option && share_option {
            let pair = self.lower_option(scope, scrutinee)?;
            return Some((Subject::Shared { is: pair.is, val: pair.val }, Vec::new()));
        }
        if option {
            let pair = self.lower_option(scope, scrutinee)?;
            let k = scope.locals.iter().filter(|l| l.name.starts_with("match$") && !l.name.ends_with(".val")).count();
            let (is_ty, val_ty) = (self.type_of(pair.is), self.type_of(pair.val));
            let is = scope.locals.len() as u32;
            scope.locals.push(Local { name: format!("match${k}.is"), ty: is_ty.clone(), mutable: false });
            scope.locals.push(Local { name: format!("match${k}.val"), ty: val_ty.clone(), mutable: false });
            let first = self.stmt(Stmt::Let { local: is, init: pair.is });
            let second = self.stmt(Stmt::Let { local: is + 1, init: pair.val });
            return Some((Subject::Pair { is: (is, is_ty), val: (is + 1, val_ty) }, vec![first, second]));
        }
        if scope.no_locals || self.simple_read(scrutinee) {
            return Some((Subject::Reread(scrutinee), Vec::new()));
        }
        let lowered = self.lower_expr(scope, scrutinee)?;
        let ty = self.type_of(lowered);
        let index = scope.locals.len() as u32;
        let name = format!("match${}", scope.locals.iter().filter(|l| l.name.starts_with("match$")).count());
        scope.locals.push(Local { name, ty: ty.clone(), mutable: false });
        Some((Subject::Local(index, ty), vec![self.stmt(Stmt::Let { local: index, init: lowered })]))
    }

    /// The block `stmts` under the `lets` that bind the names of its pattern, as one expression.
    fn bound_block(&mut self, scope: &mut Scope, lets: Vec<LoweredStmt>, stmts: &[StmtId], span: Span) -> Option<LoweredExpr> {
        let origin = self.locator.origin(scope.file, span);
        let (rest, result) = self.lower_statements(scope, stmts)?;
        let mut lowered = lets;
        lowered.extend(rest);
        let ty = result.map_or(ValueType::Unit, |v| self.type_of(v));
        let block = self.emit(Expr::Block { stmts: lowered.clone(), value: result }, ty, origin);
        self.note_block(block, &lowered, result);
        Some(block)
    }

    /// The test of `matched` as a condition: `true` when it matches everything.
    fn condition_of(&mut self, matched: &PatternTest, origin: &Origin) -> LoweredExpr {
        match matched.test {
            Some(test) => test,
            None => self.emit(Expr::Bool(true), ValueType::Bool, origin.clone()),
        }
    }

    /// `if let p = e { A } else { B }`: `{ let s = e; if test(s, p) { let names; A } else { B } }`.
    pub(super) fn lower_if_let(&mut self, scope: &mut Scope, cond: ExprId, then: &BlockNode, else_: Option<ExprId>, base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        let ExprKind::Let { pat, expr } = self.design.expr_node(cond).kind.clone() else { return None };
        let (subject, prelude) = self.subject_of(scope, expr, true)?;
        let matched = self.test_pattern(scope, &subject, pat)?;
        let names = self.bind_names(scope, &matched.bindings)?;
        let test = self.condition_of(&matched, &origin);
        let arm = Self::enter_arm(scope, Some(test), true);
        scope.guards.push(ArmCondition::IfLet);
        let lets = self.binding_lets(&names);
        let taken = self.bound_block(scope, lets, &then.stmts, then.span);
        scope.guards.pop();
        Self::leave_arm(scope, arm);
        let arm = Self::enter_arm(scope, Some(test), false);
        scope.guards.push(ArmCondition::IfLet);
        let otherwise = match else_ {
            Some(else_) => self.lower_expr(scope, else_),
            None => Some(self.emit(Expr::Unit, ValueType::Unit, origin.clone())),
        };
        scope.guards.pop();
        Self::leave_arm(scope, arm);
        let (taken, otherwise) = (taken?, otherwise?);
        let (taken, otherwise) = (self.fit_to(taken, &base), self.fit_to(otherwise, &base));
        let ty = self.flags(&[test, taken, otherwise]).wrap(base);
        let whole = self.emit(Expr::If { cond: test, then: taken, otherwise }, ty.clone(), origin.clone());
        if prelude.is_empty() {
            return Some(whole);
        }
        Some(self.emit(Expr::Block { stmts: prelude, value: Some(whole) }, ty, origin))
    }

    /// `while let p = e { body }` as `loop { let s = e; if test(s, p) { let names; body } else { break } }`,
    /// `e` read again at each iteration.
    pub(super) fn lower_while_let(&mut self, scope: &mut Scope, pat: PatId, scrutinee: ExprId, body: &BlockNode, origin: Origin) -> Option<LoweredExpr> {
        scope.loops.push(super::control::LoopFrame { value: ValueType::Unit });
        let looped = self.lower_while_let_body(scope, pat, scrutinee, body, &origin);
        scope.loops.pop();
        let body = looped?;
        Some(self.emit(Expr::Loop { body }, ValueType::Unit, origin))
    }

    /// The statements of the loop of a `while let`: the subject, then the `if` that runs the body
    /// or leaves.
    fn lower_while_let_body(&mut self, scope: &mut Scope, pat: PatId, scrutinee: ExprId, body: &BlockNode, origin: &Origin) -> Option<Vec<LoweredStmt>> {
        let (subject, mut stmts) = self.subject_of(scope, scrutinee, true)?;
        let matched = self.test_pattern(scope, &subject, pat)?;
        let names = self.bind_names(scope, &matched.bindings)?;
        let test = self.condition_of(&matched, origin);
        let lets = self.binding_lets(&names);
        let taken = self.bound_block(scope, lets, &body.stmts, body.span)?;
        let taken = self.fit_to(taken, &ValueType::Unit);
        let leave = self.emit_exit(Expr::Break(None), origin.clone());
        let ty = self.flag(test).wrap(ValueType::Unit);
        let step = self.emit(Expr::If { cond: test, then: taken, otherwise: leave }, ty, origin.clone());
        stmts.push(self.stmt(Stmt::Expr(step)));
        Some(stmts)
    }

    /// `let p = e else { diverge };`: the subject, the `if` that runs the else block when the test
    /// fails, then the names (§1.7.6).
    pub(super) fn lower_let_else(&mut self, scope: &mut Scope, out: &mut Vec<LoweredStmt>, pat: PatId, init: ExprId, otherwise: &BlockNode, file: FileId) -> bool {
        let Some((subject, prelude)) = self.subject_of(scope, init, false) else { return false };
        let Some(matched) = self.test_pattern(scope, &subject, pat) else { return false };
        let Some(names) = self.bind_names(scope, &matched.bindings) else { return false };
        out.extend(prelude);
        if let Some(test) = matched.test {
            let origin = self.locator.origin(file, otherwise.span);
            let flag = self.flag(test);
            let failed = self.emit(Expr::Unary { op: UnaryOp::Not, expr: test }, flag.wrap(ValueType::Bool), origin.clone());
            let Some(leave) = self.lower_block(scope, file, &otherwise.stmts, otherwise.span) else { return false };
            let leave = self.fit_to(leave, &ValueType::Unit);
            let nothing = self.emit(Expr::Unit, ValueType::Unit, origin.clone());
            let guard = self.emit(Expr::If { cond: failed, then: leave, otherwise: nothing }, flag.wrap(ValueType::Unit), origin);
            out.push(self.stmt(Stmt::Expr(guard)));
        }
        out.extend(self.binding_lets(&names));
        true
    }
}
