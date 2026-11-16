//! The forms the IR amendments of msl-digital added and this printer does not lower yet: each is
//! refused by name, with the body, the expression and its origin, before any text is printed.
//! A later task that lowers a form deletes its arm here.

use std::collections::BTreeSet;

use piperine_ir::{Body, Expr, ExprId, Ir};
use piperine_plan::BodyPlan;

use crate::Error;
use crate::print::{LibraryPrinter, ValuePrinter};

/// The refusal of the amended forms.
pub(crate) struct Amended;

impl Amended {
    /// `Error::Unsupported` for the first amended form a running body evaluates, directly or
    /// through the functions it calls.
    pub(crate) fn refuse(ir: &Ir, running: &[(Body<'_>, &BodyPlan)]) -> Result<(), Error> {
        for (body, _) in running {
            let mut exprs = LibraryPrinter::roots(ir, [*body]);
            let mut seen = BTreeSet::new();
            while let Some(e) = exprs.pop() {
                if let Some(form) = Self::form(ir.expr(e)) {
                    return Err(Self::unsupported(body, e, form, ir));
                }
                match ir.expr(e) {
                    Expr::Call { function, args } => {
                        if seen.insert(function.index()) {
                            exprs.push(ir.function(*function).body());
                        }
                        exprs.extend(args);
                    }
                    other => ValuePrinter::children(ir, other, &mut exprs),
                }
            }
        }
        Ok(())
    }

    /// The source form of an amended expression.
    fn form(expr: &Expr) -> Option<&'static str> {
        match expr {
            Expr::Flow { .. } => Some("the flow of a potential branch"),
            Expr::Previous(_) => Some("the previous value of a carrier"),
            _ => None,
        }
    }

    fn unsupported(body: &Body<'_>, e: ExprId, form: &str, ir: &Ir) -> Error {
        Error::Unsupported { form: form.to_string(), path: format!("body {} e{}", body.name(), e.index()), origin: ir.expr_origin(e).clone() }
    }
}
