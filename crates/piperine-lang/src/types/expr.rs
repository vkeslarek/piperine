//! Value expressions: `check` against an expected type where the position gives one, `infer`
//! otherwise. A mismatch is one `P0002` at the expression and the expression then takes the
//! expected type, so one mistake is one diagnostic.

use crate::msl::lexer;
use crate::msl::{Diagnostic, Span};
use crate::pom::{BinOp, BlockNode, FileId, BuiltinKind, DeclKind, ExprId, ExprKind, LitKind, PatKind, Resolution, StmtId, StmtKind, StructClass, UnOp};
use crate::stack;

use super::check::{Checker, Context};
use super::unsupported::Unsupported;
use super::Ty;

impl Checker<'_> {
    /// Check `expr` against `expected`. A mismatch is one diagnostic; the expression then takes
    /// the expected type.
    pub(super) fn check(&mut self, expr: ExprId, expected: &Ty) {
        self.check_as(expr, expected, "P0002");
    }

    /// `check` of an argument written directly: the one place a function may stand as a value,
    /// for a parameter of function type. Through an `if` or a block it is no named function.
    pub(super) fn check_argument(&mut self, expr: ExprId, expected: &Ty) {
        if let Ty::Option(inner) = expected {
            self.option_argument(expr, expected, inner);
            return;
        }
        self.fn_value_at = Some(expr);
        self.check(expr, expected);
    }

    /// `check`, where a mismatch of `expr` itself carries `code` (a rule of the specification
    /// with a code of its own, `E0318` for a `where`) instead of the plain `P0002`.
    pub(super) fn check_as(&mut self, expr: ExprId, expected: &Ty, code: &'static str) {
        stack::deep(|| {
            let node = self.design.expr_node(expr).clone();
            let file = self.file_of_expr(expr);
            if *expected == Ty::Event {
                self.event_slot = Some(expr);
            }
            // Where the expected type flows into the parts of the expression: the value of a
            // block or of the arms of an `if` is where a mismatch is reported.
            let found = match &node.kind {
                ExprKind::Paren(inner) => {
                    self.check(*inner, expected);
                    Some(expected.clone())
                }
                ExprKind::Block(block) => self.statements(&block.stmts, Some(expected)),
                ExprKind::Closure { .. } => self.closure(expr, expected),
                ExprKind::Path(_) if matches!(expected, Ty::Fn(..)) && self.fn_value_at == Some(expr) && self.function_named(expr).is_some() => self.function_value(expr),
                ExprKind::If { cond, then, else_: Some(else_) } => {
                    self.condition(*cond);
                    self.statements(&then.stmts, Some(expected));
                    self.check(*else_, expected);
                    Some(expected.clone())
                }
                // `100` where an `f64` is required: an integer literal never converts (§1 rule 7).
                ExprKind::Lit(lit) if matches!(&lit.kind, LitKind::Int(text) if !text.ends_with("f64")) && *expected == Ty::F64 => {
                    self.mismatch_coded(file, node.span, code, "f64", "an integer literal");
                    Some(expected.clone())
                }
                ExprKind::Match { .. } => self.match_expr(expr, Some(expected)),
                // `Unknown` where a carried type is expected (§3.3.1: of any `Repr` type). `analog`
                // computes with known reals (§3.3.5): there it is refused by name.
                ExprKind::Lit(lit) if matches!(lit.kind, LitKind::Unknown) && expected.has_repr() && self.context != Context::Analog => Some(expected.clone()),
                // `None` and `Some(e)` where an `Option<T>` is expected.
                ExprKind::Path(_) if matches!(expected, Ty::Option(_)) && self.is_none_literal(expr) => Some(expected.clone()),
                ExprKind::Call { args, .. } if matches!(self.callee_resolution_of_call(expr), Some(Resolution::Builtin(id)) if id.info().name == "Some") => match expected {
                    Ty::Option(inner) => self.some(expr, args, Some(inner)),
                    _ => self.some(expr, args, None),
                },
                // An integer literal where an integer type is expected takes that type (§1 rule 7).
                ExprKind::Lit(lit) if expected.is_integer() && matches!(&lit.kind, LitKind::Int(text) if !text.ends_with("f64")) => {
                    let LitKind::Int(text) = &lit.kind else { return };
                    self.integer_literal(file, node.span, text, Some(expected), false)
                }
                // `-256` where an `i16` is expected: the minus takes the expected type, and a literal
                // operand may reach the magnitude of the type's minimum.
                ExprKind::Unary { op: UnOp::Neg, expr: operand } if expected.is_integer() => {
                    match self.integer_literal_text(*operand) {
                        Some(text) => {
                            let span = self.design.expr_node(*operand).span;
                            let found = self.integer_literal(file, span, &text, Some(expected), true);
                            if let Some(found) = &found
                                && found != expected
                            {
                                self.mismatch(file, span, expected, found);
                            }
                            self.exprs[operand.index()] = Some(expected.clone());
                        }
                        None => self.check(*operand, expected),
                    }
                    Some(expected.clone())
                }
                // An operator whose result has its operands' type passes the expected integer type
                // down to both; a shift passes it to its left operand only.
                ExprKind::Binary { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor, lhs, rhs } if expected.is_integer() => {
                    self.check(*lhs, expected);
                    self.check(*rhs, expected);
                    Some(expected.clone())
                }
                ExprKind::Binary { op: BinOp::Shl | BinOp::Shr, lhs, rhs } if expected.is_integer() => {
                    self.check(*lhs, expected);
                    self.shift_amount(*rhs);
                    Some(expected.clone())
                }
                ExprKind::Unary { op: UnOp::Not, expr: operand } if expected.is_integer() => {
                    self.check(*operand, expected);
                    Some(expected.clone())
                }
                _ => self.infer(expr),
            };
            if let Some(found) = found
                && found != *expected
            {
                let (expected_name, found_name) = (expected.name(self.design), found.name(self.design));
                self.mismatch_coded(file, node.span, code, &expected_name, &found_name);
            }
            self.exprs[expr.index()] = Some(expected.clone());
        })
    }

    /// The function a plain path names, if it names one.
    fn function_named(&self, expr: ExprId) -> Option<crate::pom::DeclId> {
        match self.callee_resolution(expr) {
            Some(Resolution::Decl(decl)) if matches!(self.design.decl_data(decl).kind, DeclKind::Fn(_)) => Some(decl),
            _ => None,
        }
    }

    /// A function passed as an argument whose parameter has a function type: reached, typed from
    /// its signature.
    fn function_value(&mut self, expr: ExprId) -> Option<Ty> {
        let decl = self.function_named(expr)?;
        self.reach(decl);
        let (params, ret) = self.signature(decl)?;
        Some(Ty::Fn(params, Box::new(ret)))
    }

    /// The type of `expr` from the expression alone; `None` after a diagnostic.
    pub(super) fn infer(&mut self, expr: ExprId) -> Option<Ty> {
        stack::deep(|| {
            let in_event_position = self.event_slot == Some(expr);
            let ty = self.infer_kind(expr);
            if ty == Some(Ty::Event) && !in_event_position {
                self.event_out_of_place(expr);
            }
            self.exprs[expr.index()] = ty.clone();
            ty
        })
    }

    fn infer_kind(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let file = self.file_of_expr(expr);
        match &node.kind {
            ExprKind::Lit(lit) => self.literal(&lit.kind, file, node.span),
            ExprKind::Path(_) => self.path(expr),
            ExprKind::Paren(inner) => {
                if self.event_slot == Some(expr) {
                    self.event_slot = Some(*inner);
                }
                self.infer(*inner)
            }
            ExprKind::Unary { op: UnOp::Neg, expr: operand } => {
                if let Some(text) = self.integer_literal_text(*operand) {
                    let span = self.design.expr_node(*operand).span;
                    let ty = self.integer_literal(file, span, &text, None, true);
                    self.exprs[operand.index()] = ty.clone();
                    return ty;
                }
                match self.infer(*operand) {
                    Some(t) if t.is_integer() => Some(t),
                    Some(Ty::F64) | None => Some(Ty::F64),
                    Some(other) => {
                        let span = self.design.expr_node(*operand).span;
                        self.mismatch(file, span, &Ty::F64, &other);
                        Some(Ty::F64)
                    }
                }
            }
            ExprKind::Unary { op: UnOp::Not, expr: operand } => {
                // An `Event` operand is judged here, as `E0440`, not as an `Event` out of place.
                self.event_slot = Some(*operand);
                match self.infer(*operand) {
                    Some(t) if t.is_integer() => Some(t),
                    Some(Ty::Bool) | None => Some(Ty::Bool),
                    Some(Ty::Event) => {
                        self.negated_event(expr);
                        None
                    }
                    Some(other) => {
                        let span = self.design.expr_node(*operand).span;
                        self.mismatch(file, span, &Ty::Bool, &other);
                        Some(Ty::Bool)
                    }
                }
            }
            ExprKind::Cast { expr: operand, ty } => self.cast(*operand, *ty),
            ExprKind::Binary { op, lhs, rhs } => self.binary(expr, *op, *lhs, *rhs),
            ExprKind::Block(block) => self.statements(&block.stmts, None),
            ExprKind::If { cond, then, else_ } => self.conditional(*cond, then, *else_),
            ExprKind::Call { .. } => self.call(expr),
            ExprKind::Match { .. } => self.match_expr(expr, None),
            ExprKind::MethodCall { .. } => self.method_call(expr),
            ExprKind::Closure { .. } => {
                self.unsupported(file, node.span, "closure outside an argument whose parameter has a function type");
                None
            }
            ExprKind::Emit { stream, fields } => self.emit(expr, stream, fields),
            ExprKind::Struct { class: StructClass::Template(_), .. } => {
                self.unsupported(file, node.span, "module template `Name { .. }` outside a placement");
                None
            }
            other => {
                self.unsupported(file, node.span, Unsupported::expr(other));
                None
            }
        }
    }

    fn binary(&mut self, whole: ExprId, op: BinOp, lhs: ExprId, rhs: ExprId) -> Option<Ty> {
        let file = self.file_of_expr(lhs);
        let span = self.design.expr_node(whole).span;
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let comparison = matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
                let ty = match self.operands(lhs, rhs, |t| t.is_integer()) {
                    Some(t) => t,
                    None => Ty::F64,
                };
                Some(if comparison { Ty::Bool } else { ty })
            }
            BinOp::And | BinOp::Or => self.logical(whole, op == BinOp::And, lhs, rhs),
            BinOp::Eq | BinOp::Ne => {
                let (first, second) = self.literal_last(lhs, rhs);
                match self.infer(first) {
                    Some(t @ (Ty::F64 | Ty::Bool | Ty::UInt(_) | Ty::Int(_))) => self.check(second, &t),
                    // §2 rule 5: an enum compares when it derives `PartialEq`.
                    Some(t @ Ty::Enum(e)) => {
                        self.check(second, &t);
                        if !self.derives(e, "PartialEq") {
                            let span = self.design.expr_node(first).span;
                            let found = t.name(self.design);
                            self.mismatch_text(file, span, "a type that derives `PartialEq`", &found);
                        }
                    }
                    Some(_) => {
                        let span = self.design.expr_node(first).span;
                        self.unsupported(file, span, "comparison of values that are neither `f64`, `bool` nor an integer");
                    }
                    None => {
                        self.infer(second);
                    }
                }
                Some(Ty::Bool)
            }
            BinOp::Rem | BinOp::BitXor | BinOp::BitAnd | BinOp::BitOr => {
                let bitwise = op != BinOp::Rem;
                match self.operands(lhs, rhs, |t| t.is_integer() || (bitwise && *t == Ty::Bool)) {
                    Some(t) => Some(t),
                    None => {
                        self.unsupported(file, span, Unsupported::binary(op));
                        None
                    }
                }
            }
            BinOp::Shl | BinOp::Shr => match self.infer(lhs) {
                Some(t) if t.is_integer() => {
                    self.shift_amount(rhs);
                    Some(t)
                }
                _ => {
                    self.unsupported(file, span, Unsupported::binary(op));
                    None
                }
            },
        }
    }

    /// The two operands of an operator that takes one type: the operand that is not a bare
    /// integer literal is typed first and the other is checked against it (local literal
    /// typing). `Some` of that type when `accepts` it; otherwise both operands are held to `f64`,
    /// as the M1 operators always were, and `None` comes back.
    fn operands(&mut self, lhs: ExprId, rhs: ExprId, accepts: impl Fn(&Ty) -> bool) -> Option<Ty> {
        let (first, second) = self.literal_last(lhs, rhs);
        match self.infer(first) {
            Some(t) if accepts(&t) => {
                self.check(second, &t);
                Some(t)
            }
            found => {
                if let Some(found) = found
                    && found != Ty::F64
                {
                    let file = self.file_of_expr(first);
                    let span = self.design.expr_node(first).span;
                    self.mismatch(file, span, &Ty::F64, &found);
                }
                self.check(second, &Ty::F64);
                None
            }
        }
    }

    /// `(lhs, rhs)`, or `(rhs, lhs)` when only `lhs` is a bare integer literal: the order in which
    /// two operands of one type are typed.
    fn literal_last(&self, lhs: ExprId, rhs: ExprId) -> (ExprId, ExprId) {
        let bare = |this: &Self, e: ExprId| this.integer_literal_text(e).is_some_and(|t| lexer::int_suffix_text(&t).is_empty());
        if bare(self, lhs) && !bare(self, rhs) { (rhs, lhs) } else { (lhs, rhs) }
    }

    /// The right operand of `<<` or `>>`: any integer type (DIG-01 AC 6); a bare literal there
    /// expects nothing and is `i32`.
    fn shift_amount(&mut self, rhs: ExprId) {
        match self.infer(rhs) {
            Some(t) if t.is_integer() => {}
            Some(other) => {
                let file = self.file_of_expr(rhs);
                let span = self.design.expr_node(rhs).span;
                self.mismatch_text(file, span, "an integer type", &other.name(self.design));
            }
            None => {}
        }
    }

    /// `expr as T`: between any two of the integer types and `f64`, and from `bool` to an integer
    /// type (§1 rule 1); anything else is a plain mismatch.
    fn cast(&mut self, operand: ExprId, ty: crate::pom::TypeId) -> Option<Ty> {
        let target = self.lower_type(ty);
        let source = self.infer(operand);
        let target = target?;
        let numeric = |t: &Ty| t.is_integer() || *t == Ty::F64;
        if let Some(source) = source
            && !(numeric(&target) && (numeric(&source) || (source == Ty::Bool && target.is_integer())))
        {
            let file = self.file_of_expr(operand);
            let span = self.design.expr_node(operand).span;
            let found = source.name(self.design);
            self.mismatch_text(file, span, &format!("a value `as` converts to `{}`", target.name(self.design)), &found);
        }
        Some(target)
    }

    /// `if cond { then } else { else_ }`: with an `else` the common type of the two blocks;
    /// without one, `()`.
    fn conditional(&mut self, cond: ExprId, then: &BlockNode, else_: Option<ExprId>) -> Option<Ty> {
        self.condition(cond);
        // `if c { 0 } else { count + 1 }`: the first arm that is not a bare integer literal gives
        // the type the literal takes (local literal typing).
        if let Some(else_) = else_
            && self.block_waits(then)
        {
            let else_ty = self.infer(else_);
            if let Some(t) = &else_ty {
                self.statements(&then.stmts, Some(t));
            } else {
                self.statements(&then.stmts, None);
            }
            return else_ty;
        }
        let then_ty = self.statements(&then.stmts, None);
        match (else_, then_ty) {
            (Some(else_), Some(t)) => {
                self.check(else_, &t);
                Some(t)
            }
            (Some(else_), None) => {
                self.infer(else_);
                None
            }
            (None, Some(t)) if t != Ty::Unit => {
                let file = self.file_of_expr(cond);
                self.mismatch(file, then.span, &Ty::Unit, &t);
                Some(Ty::Unit)
            }
            (None, t) => t,
        }
    }

    fn literal(&mut self, kind: &LitKind, file: crate::pom::FileId, span: crate::msl::Span) -> Option<Ty> {
        match kind {
            LitKind::Float { raw, .. } => match raw.rsplit_once('_') {
                Some((_, suffix)) if suffix.starts_with('f') && suffix != "f64" => {
                    self.unsupported(file, span, &format!("`{suffix}` literal"));
                    None
                }
                _ => Some(Ty::F64),
            },
            LitKind::Bool(_) => Some(Ty::Bool),
            LitKind::Int(text) if text.ends_with("f64") => Some(Ty::F64),
            LitKind::Int(text) => self.integer_literal(file, span, text, None, false),
            LitKind::Str { .. } => Some(Ty::Str),
            LitKind::Unknown if self.context == Context::Analog => {
                self.unsupported(file, span, "`Unknown` literal in an `analog` block");
                None
            }
            LitKind::Unknown => {
                self.unsupported(file, span, "`Unknown` literal where no carried type is expected");
                None
            }
        }
    }

    /// The type of the integer literal `text`: its suffix, else `expected`, else `i32` (§1 rule
    /// 7). A suffix of another type than `expected` is a mismatch; a value outside the type's
    /// range is `E0909`; a suffix outside the subset (`u128`, `f32`) is `P0001` naming it. With
    /// `negated` the literal is the operand of a unary `-`.
    fn integer_literal(&mut self, file: FileId, span: Span, text: &str, expected: Option<&Ty>, negated: bool) -> Option<Ty> {
        let suffix = lexer::int_suffix_text(text);
        let ty = match suffix {
            "" => expected.cloned().unwrap_or(Ty::Int(32)),
            _ => {
                let width = suffix[1..].parse::<u8>().unwrap_or(u8::MAX);
                match (&suffix[..1], width) {
                    ("u", 1..=64) => Ty::UInt(width),
                    ("i", 1..=64) => Ty::Int(width),
                    ("u" | "i", _) => {
                        self.unsupported(file, span, &format!("integer type `{suffix}`"));
                        return None;
                    }
                    _ => {
                        self.unsupported(file, span, &format!("`{suffix}` literal"));
                        return None;
                    }
                }
            }
        };
        if let Some(expected) = expected
            && *expected != ty
        {
            return Some(ty);
        }
        let value = lexer::int_value(text);
        if let (Some(value), Some(max)) = (value, ty.literal_max(negated))
            && value > max
        {
            let name = ty.name(self.design);
            let message = format!("`{text}` is out of range: an integer literal beyond the range of `{name}`");
            self.report(file, Diagnostic::new("E0909", message, span, "literal out of range").with_help("write a value the type holds, or a wider type"));
        }
        Some(ty)
    }

    /// The text of `expr` when it is an integer literal without an `f64` suffix, through parens.
    fn integer_literal_text(&self, expr: ExprId) -> Option<String> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.integer_literal_text(*inner),
            ExprKind::Lit(lit) => match &lit.kind {
                LitKind::Int(text) if !text.ends_with("f64") => Some(text.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether the value of `block` takes its type from another arm: an integer literal with no
    /// suffix, or `None`.
    fn block_waits(&self, block: &BlockNode) -> bool {
        let Some(last) = block.stmts.last() else { return false };
        match &self.design.stmt_node(*last).kind {
            StmtKind::Expr { expr, semi: false } => self.integer_literal_waits(*expr) || self.is_none_literal(*expr),
            _ => false,
        }
    }

    /// Whether `expr` is an integer literal with no suffix: its type comes from its position.
    pub(super) fn integer_literal_waits(&self, expr: ExprId) -> bool {
        self.integer_literal_text(expr).is_some_and(|t| lexer::int_suffix_text(&t).is_empty())
    }

    /// The resolution of the callee of the call `expr`.
    fn callee_resolution_of_call(&self, expr: ExprId) -> Option<Resolution> {
        match &self.design.expr_node(expr).kind {
            ExprKind::Call { callee, .. } => self.callee_resolution(*callee),
            _ => None,
        }
    }

    /// A name: a param, a constant, a parameter or local binding, or a function as a value.
    fn path(&mut self, expr: ExprId) -> Option<Ty> {
        let node = self.design.expr_node(expr).clone();
        let ExprKind::Path(path) = &node.kind else { return None };
        let file = self.file_of_expr(expr);
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        match res {
            Resolution::Decl(decl) => {
                let kind = self.design.decl_data(decl).kind.clone();
                match kind {
                    DeclKind::Const(c) => {
                        self.reach(decl);
                        self.lower_type(c.ty)
                    }
                    DeclKind::Param(p) => self.lower_type(p.ty),
                    DeclKind::Port(p) => self.read_type(decl, p.ty),
                    DeclKind::Signal(s) => self.read_type(decl, s.ty),
                    DeclKind::FnParam(_) | DeclKind::Binding(_) => {
                        let local = self.locals.get(&decl).cloned();
                        if local.is_none() && self.lets_open.contains(&decl) {
                            self.self_read(file, node.span, decl);
                            return None;
                        }
                        if local.is_none() && !self.poisoned.contains(&decl) {
                            let what = if self.fn_where { "a value parameter in the `where` of a `fn`" } else { "a name whose type is unknown" };
                            self.unsupported(file, node.span, what);
                        }
                        local
                    }
                    DeclKind::Variant(_) => match self.design.decl_data(decl).parent {
                        Some(parent) if matches!(self.design.decl_data(parent).kind, DeclKind::Enum(_)) => self.enum_type(parent, file, node.span),
                        _ => {
                            self.unsupported(file, node.span, "`enum` variant used as a value");
                            None
                        }
                    },
                    DeclKind::Fn(_) => {
                        self.unsupported(file, node.span, "function used as a value outside an argument whose parameter has a function type");
                        None
                    }
                    other => {
                        self.unsupported(file, node.span, &format!("{} used as a value", Unsupported::item(&other)));
                        None
                    }
                }
            }
            Resolution::Builtin(id) if id.info().kind == BuiltinKind::Node => Some(Ty::AnyNode),
            Resolution::Builtin(id) if id.info().name == "None" => {
                let message = "`None` has no type here: write it where an `Option<T>` is expected";
                self.report(file, Diagnostic::new("P0002", format!("mismatched types: {message}"), node.span, "`Option` of what?"));
                None
            }
            Resolution::Builtin(id) => {
                self.unsupported(file, node.span, &format!("built-in `{}`", id.name()));
                None
            }
            Resolution::Unresolved | Resolution::Deferred(_) => {
                self.unsupported(file, node.span, "an unresolved name");
                None
            }
        }
    }

    /// The statements of a block, a function body or an analog block; the type of the block is
    /// that of its last expression without a `;`, else `()`. With an `expected` type the last
    /// expression is checked against it.
    pub(super) fn statements(&mut self, stmts: &[StmtId], expected: Option<&Ty>) -> Option<Ty> {
        let mut value = Some(Ty::Unit);
        for (i, stmt) in stmts.iter().enumerate() {
            let last = i + 1 == stmts.len();
            value = Some(Ty::Unit);
            let node = self.design.stmt_node(*stmt).clone();
            let file = self.design.decl_data(node.owner).file;
            match &node.kind {
                StmtKind::Let { pat, ty, init, else_ } => {
                    if else_.is_some() {
                        self.unsupported(file, node.span, Unsupported::stmt(&node.kind));
                    } else {
                        self.let_statement(file, node.span, *pat, *ty, *init);
                    }
                }
                StmtKind::Expr { expr, semi } => {
                    if last && !semi {
                        match expected {
                            Some(expected) => {
                                self.check(*expr, expected);
                                value = Some(expected.clone());
                            }
                            None => value = self.infer(*expr),
                        }
                    } else {
                        self.infer(*expr);
                    }
                }
                StmtKind::Empty => {}
                StmtKind::Contribution { lhs, rhs } => self.contribution(file, node.span, *lhs, *rhs),
                StmtKind::Commit { place, value, after } => self.commit(file, node.span, *place, *value, *after),
                StmtKind::Item(_) | StmtKind::Signal(_) | StmtKind::Reaction(_) => {
                    self.unsupported(file, node.span, Unsupported::stmt(&node.kind));
                }
            }
        }
        value
    }

    /// `let x = init;` or `let x: T = init;`: the binding takes the annotation, else the type of
    /// the initializer.
    fn let_statement(&mut self, file: crate::pom::FileId, span: crate::msl::Span, pat: crate::pom::PatId, ty: Option<crate::pom::TypeId>, init: Option<ExprId>) {
        let pat_node = self.design.pat_node(pat).clone();
        if let (PatKind::Tuple(_), None, Some(init)) = (&pat_node.kind, ty, init) {
            self.tuple_let(file, &pat_node, init);
            return;
        }
        let PatKind::Ident { by_ref: false, mutable: false, sub: None, binding, res: Resolution::Unresolved, .. } = &pat_node.kind else {
            self.unsupported(file, pat_node.span, Unsupported::pat(&pat_node.kind));
            return;
        };
        let annotated = match ty {
            Some(t) => match self.lower_type(t) {
                Some(t) => Some(t),
                None => {
                    if let Some(init) = init {
                        self.infer(init);
                    }
                    self.poisoned.insert(*binding);
                    return;
                }
            },
            None => None,
        };
        let bound = match (annotated, init) {
            (Some(t), Some(init)) => {
                self.check(init, &t);
                Some(t)
            }
            (None, Some(init)) => self.infer(init),
            (Some(_), None) | (None, None) => {
                self.unsupported(file, span, "`let` without a value");
                None
            }
        };
        match bound {
            Some(Ty::Str) => {
                self.unsupported(file, span, "a string literal bound by a `let`");
                self.poisoned.insert(*binding);
            }
            Some(t) => {
                self.locals.insert(*binding, t);
            }
            None => {
                self.poisoned.insert(*binding);
            }
        }
    }
}
