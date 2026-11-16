//! Compound values lowered to nodes the IR already has (design "From a source form to IR nodes
//! the contract already has").
//!
//! - An enum of `V` unit variants is its tag (§2 rule 5): `UInt<ceil(log2 V)>`, variant `k` (in
//!   declaration order, from 0) the literal `k`. No IR type names the enum.
//! - A `match` is an `if` chain over its scrutinee, read once: bound to a local unless it already
//!   is a local, a param or a literal. Each arm's pattern is a comparison with the scrutinee, an
//!   or-pattern the disjunction of its alternatives (`|` when the scrutinee may be unknown, `||`
//!   when it is known); the last arm of the exhaustive `match` is the final `else`, and so is an
//!   arm that matches everything.

use piperine_ir::{BinaryOp, Expr, ExprId as LoweredExpr, Local, Origin, Stmt, UnaryOp, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use crate::pom::{Arm, DeclId, DeclKind, Design, ExprId, ExprKind, LitKind, PatId, PatKind, Resolution};

/// The scrutinee of a `match`, as each test reads it.
enum Subject {
    /// A local, a param or a literal: lowered again for each test.
    Reread(ExprId),
    /// Bound to this local of this type.
    Local(u32, ValueType),
}

/// The tag of an enum: its width and its variants in declaration order.
pub(super) struct EnumLayout {
    variants: Vec<DeclId>,
}

impl EnumLayout {
    pub(super) fn of(design: &Design, decl: DeclId) -> EnumLayout {
        let variants = design.decl_data(decl).children.iter().copied().filter(|v| matches!(design.decl_data(*v).kind, DeclKind::Variant(_))).collect();
        EnumLayout { variants }
    }

    /// `ceil(log2 V)`: 0 for an enum of one variant, whose tag has no level.
    pub(super) fn width(&self) -> u32 {
        let v = self.variants.len().max(1) as u64;
        u64::BITS - (v - 1).leading_zeros()
    }

    /// The tag's IR type; `None` for an enum of one variant.
    pub(super) fn ty(&self) -> Option<ValueType> {
        let width = self.width();
        (width > 0).then_some(ValueType::UInt(width))
    }

    /// The tag of `variant`, its position among the enum's variants.
    pub(super) fn tag_of(&self, variant: DeclId) -> Option<i128> {
        self.variants.iter().position(|v| *v == variant).map(|k| k as i128)
    }

    /// The tag of a variant declaration, through its enum.
    pub(super) fn variant_tag(design: &Design, variant: DeclId) -> Option<i128> {
        let parent = design.decl_data(variant).parent?;
        EnumLayout::of(design, parent).tag_of(variant)
    }
}

impl Elaboration<'_, '_> {
    /// `match scrutinee { arms }`, whose value has the base type `base`.
    pub(super) fn lower_match(&mut self, scope: &mut Scope, scrutinee: ExprId, arms: &[Arm], base: ValueType, origin: Origin) -> Option<LoweredExpr> {
        // Read once: a local, a param or a literal is read again where a test needs it; anything
        // else is bound to a local first.
        let (subject, binding) = if self.simple_read(scrutinee) {
            (Subject::Reread(scrutinee), None)
        } else {
            let lowered = self.lower_expr(scope, scrutinee)?;
            let ty = self.type_of(lowered);
            let index = scope.locals.len() as u32;
            let name = format!("match${}", scope.locals.iter().filter(|l| l.name.starts_with("match$")).count());
            scope.locals.push(Local { name, ty: ty.clone() });
            (Subject::Local(index, ty), Some(self.ir.stmt(Stmt::Let { local: index, init: lowered })))
        };
        scope.nesting += 1;
        let chain = self.arms_chain(scope, &subject, arms, &base, &origin);
        scope.nesting -= 1;
        let chain = chain?;
        Some(match binding {
            Some(stmt) => {
                let ty = self.type_of(chain);
                self.emit(Expr::Block { stmts: vec![stmt], value: Some(chain) }, ty, origin)
            }
            None => chain,
        })
    }

    /// A fresh read of the scrutinee.
    fn read_subject(&mut self, scope: &mut Scope, subject: &Subject, origin: Origin) -> Option<LoweredExpr> {
        match subject {
            Subject::Reread(expr) => self.lower_expr(scope, *expr),
            Subject::Local(index, ty) => Some(self.emit(Expr::Local(*index), ty.clone(), origin)),
        }
    }

    /// The `if` chain of `arms` over `subject`, from the last arm back: the last arm, or the
    /// first that matches everything, is the final value.
    fn arms_chain(&mut self, scope: &mut Scope, subject: &Subject, arms: &[Arm], base: &ValueType, origin: &Origin) -> Option<LoweredExpr> {
        let mut tests = Vec::new();
        let mut bodies = Vec::new();
        let mut ok = true;
        for (k, arm) in arms.iter().enumerate() {
            let last = k + 1 == arms.len();
            let test = if last { None } else { self.pattern_test(scope, subject, arm.pat) };
            let everything = !last && test.is_none() && self.matches_all(arm.pat);
            if !last && test.is_none() && !everything {
                ok = false;
            }
            match self.lower_expr(scope, arm.body) {
                Some(body) => bodies.push(body),
                None => ok = false,
            }
            tests.push(test);
            if everything {
                break;
            }
        }
        if !ok {
            return None;
        }
        let mut value = bodies.pop()?;
        tests.pop();
        while let (Some(body), Some(test)) = (bodies.pop(), tests.pop()) {
            let test = test?;
            let ty = self.flags(&[test, body, value]).wrap(base.clone());
            value = self.emit(Expr::If { cond: test, then: body, otherwise: value }, ty, origin.clone());
        }
        Some(value)
    }

    /// Whether pattern `pat` matches every value (`_`, or an or-pattern holding one), so no arm
    /// after it is reached.
    fn matches_all(&self, pat: PatId) -> bool {
        match &self.design.pat_node(pat).kind {
            PatKind::Wild => true,
            PatKind::Or(alternatives) => alternatives.iter().any(|a| self.matches_all(*a)),
            _ => false,
        }
    }

    /// The test pattern `pat` makes of `subject`: a `bool`, flagged when `subject` is; `None` for
    /// a pattern that matches everything, or after a diagnostic.
    fn pattern_test(&mut self, scope: &mut Scope, subject: &Subject, pat: PatId) -> Option<LoweredExpr> {
        let node = self.design.pat_node(pat).clone();
        let origin = self.locator.origin(scope.file, node.span);
        if self.matches_all(pat) {
            return None;
        }
        if let PatKind::Or(alternatives) = &node.kind {
            let mut tests = Vec::new();
            for alt in alternatives {
                tests.push(self.pattern_test(scope, subject, *alt)?);
            }
            let flag = self.flags(&tests);
            let op = if flag == Flag::Flagged { BinaryOp::BitOr } else { BinaryOp::Or };
            let mut tests = tests.into_iter();
            let first = tests.next()?;
            return Some(tests.fold(first, |lhs, rhs| {
                let ty = self.flags(&[lhs, rhs]).wrap(ValueType::Bool);
                self.emit(Expr::Binary { op, lhs, rhs }, ty, origin.clone())
            }));
        }
        let subject = self.read_subject(scope, subject, origin.clone())?;
        let base = match self.type_of(subject) {
            ValueType::Flagged(inner) => *inner,
            other => other,
        };
        let flag = self.flag(subject);
        match &node.kind {
            // `true` is the subject itself, `false` its negation.
            PatKind::Lit(expr) => match self.bool_literal(*expr) {
                Some(true) => Some(subject),
                Some(false) => Some(self.emit(Expr::Unary { op: UnaryOp::Not, expr: subject }, flag.wrap(ValueType::Bool), origin)),
                None => {
                    let value = self.lower_expr(scope, *expr)?;
                    Some(self.equals(subject, value, origin))
                }
            },
            PatKind::Path(path) => {
                let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
                let value = self.path_value(scope, res, &base, node.span, origin.clone())?;
                Some(self.equals(subject, value, origin))
            }
            PatKind::Ident { res: res @ (Resolution::Decl(_) | Resolution::Builtin(_)), sub: None, .. } => {
                let value = self.path_value(scope, *res, &base, node.span, origin.clone())?;
                Some(self.equals(subject, value, origin))
            }
            _ => {
                self.unsupported(scope.file, node.span, "this pattern");
                None
            }
        }
    }

    /// The value a path pattern names: a unit variant's tag or a constant, of the subject's type.
    fn path_value(&mut self, scope: &Scope, res: Resolution, base: &ValueType, span: crate::msl::Span, origin: Origin) -> Option<LoweredExpr> {
        match res {
            Resolution::Decl(d) if matches!(self.design.decl_data(d).kind, DeclKind::Variant(_)) => {
                let tag = EnumLayout::variant_tag(self.design, d)?;
                Some(self.emit(Expr::Int(tag), base.clone(), origin))
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

    /// `subject == value`, flagged when `subject` is.
    fn equals(&mut self, subject: LoweredExpr, value: LoweredExpr, origin: Origin) -> LoweredExpr {
        let ty = self.flags(&[subject, value]).wrap(ValueType::Bool);
        self.emit(Expr::Binary { op: BinaryOp::Eq, lhs: subject, rhs: value }, ty, origin)
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

    /// Whether `expr` reads a local, a param or a literal: reading it again costs nothing and
    /// gives the same value.
    fn simple_read(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.simple_read(*inner),
            ExprKind::Lit(lit) => matches!(lit.kind, LitKind::Int(_) | LitKind::Float { .. } | LitKind::Bool(_)),
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Decl(d)) if matches!(self.design.decl_data(d).kind, DeclKind::Param(_) | DeclKind::FnParam(_) | DeclKind::Binding(_) | DeclKind::Const(_) | DeclKind::Variant(_))),
            _ => false,
        }
    }
}
