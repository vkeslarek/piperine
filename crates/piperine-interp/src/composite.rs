//! The forms of composite-values a body holds (R1): the interpreter is frozen at the M1 analog
//! subset, so [`Interpreter::new`](crate::Interpreter::new) names each of these forms and refuses
//! the design instead of running it wrong.

use std::collections::HashSet;

use piperine_ir::{Body, EventSource, Expr, ExprId, Stmt, StmtId};

/// The composite-values forms one body evaluates, found by walking everything it evaluates: its
/// `analog` block, drives, events, reactions, `where` checks, derivative sites and child param
/// expressions, and every function those call or name.
pub(crate) struct CompositeForms<'a> {
    body: Body<'a>,
    /// The forms met so far, by their position in `CompositeForms::NAMES`.
    met: [bool; 22],
}

impl<'a> CompositeForms<'a> {
    /// The name of each form, in the order the refusal lists them.
    const NAMES: [&'static str; 22] = [
        "an assignment",
        "a loop",
        "a while loop",
        "a for loop",
        "break",
        "continue",
        "return",
        "a function value",
        "a call through a function value",
        "a tuple",
        "an array",
        "an array repeat",
        "a struct expression",
        "a variant expression",
        "a field read",
        "an element read",
        "a length read",
        "a variant test",
        "a variant field read",
        "an option pack",
        "an option part",
        "a default value",
    ];

    /// The forms `body` holds, each once, in a fixed order.
    pub(crate) fn of(body: Body<'a>) -> Vec<&'static str> {
        let mut forms = CompositeForms { body, met: [false; 22] };
        forms.walk();
        Self::NAMES.into_iter().zip(forms.met).filter_map(|(name, met)| met.then_some(name)).collect()
    }

    /// Every statement and expression the body evaluates, each visited once.
    fn walk(&mut self) {
        let body = self.body;
        let mut stmts: Vec<StmtId> = body.analog().unwrap_or_default().to_vec();
        let mut exprs: Vec<ExprId> = body.params().iter().filter_map(|param| param.check).collect();
        for drive in body.drives() {
            exprs.extend([drive.value].into_iter().chain(drive.after));
        }
        for event in body.events() {
            exprs.extend(event.guard);
            match event.source {
                EventSource::Cross { e, ttol, etol } => exprs.extend([e].into_iter().chain(ttol).chain(etol)),
                EventSource::At(e) => exprs.push(e),
                EventSource::Changed(_) | EventSource::ParamChanged(_) => {}
            }
        }
        stmts.extend(body.reactions().iter().flat_map(|reaction| reaction.stmts.iter().copied()));
        exprs.extend(body.limits().iter().filter_map(|site| site.enabled));
        exprs.extend(body.derivatives().iter().flat_map(|site| [site.state].into_iter().chain(site.abstol)));
        for child in 0..body.children().len() {
            exprs.extend(body.child_params(child).iter().filter_map(|(_, value)| match value {
                piperine_ir::ChildValue::Expr(e) => Some(*e),
                piperine_ir::ChildValue::Const(_) => None,
            }));
        }
        let ir = body.ir();
        let (mut seen_exprs, mut seen_stmts, mut seen_functions): (HashSet<ExprId>, HashSet<StmtId>, HashSet<_>) = Default::default();
        while !(stmts.is_empty() && exprs.is_empty()) {
            while let Some(id) = stmts.pop() {
                if !seen_stmts.insert(id) {
                    continue;
                }
                let stmt = ir.stmt(id);
                if matches!(stmt, Stmt::Assign { .. }) {
                    self.met[0] = true;
                }
                exprs.extend(stmt.operands());
            }
            let Some(id) = exprs.pop() else { continue };
            if !seen_exprs.insert(id) {
                continue;
            }
            let kind = ir.expr(id);
            if let Some(position) = Self::position(kind) {
                self.met[position] = true;
            }
            if let Expr::Call { function, .. } | Expr::FunctionRef(function) = kind
                && seen_functions.insert(*function)
            {
                exprs.push(ir.function(*function).body());
            }
            exprs.extend(kind.operands());
            stmts.extend(kind.statements());
        }
    }

    /// The position in `NAMES` of the composite-values form `kind` is, `None` for any other.
    fn position(kind: &Expr) -> Option<usize> {
        match kind {
            Expr::Loop { .. } => Some(1),
            Expr::While { .. } => Some(2),
            Expr::ForRange { .. } => Some(3),
            Expr::Break(_) => Some(4),
            Expr::Continue => Some(5),
            Expr::Return(_) => Some(6),
            Expr::FunctionRef(_) => Some(7),
            Expr::CallValue { .. } => Some(8),
            Expr::Tuple(_) => Some(9),
            Expr::Array(_) => Some(10),
            Expr::ArrayRepeat { .. } => Some(11),
            Expr::StructOf { .. } => Some(12),
            Expr::VariantOf { .. } => Some(13),
            Expr::Field { .. } => Some(14),
            Expr::Index { .. } => Some(15),
            Expr::Length(_) => Some(16),
            Expr::IsVariant { .. } => Some(17),
            Expr::VariantField { .. } => Some(18),
            Expr::OptionPack { .. } => Some(19),
            Expr::OptionPart { .. } => Some(20),
            Expr::Default(_) => Some(21),
            _ => None,
        }
    }
}
