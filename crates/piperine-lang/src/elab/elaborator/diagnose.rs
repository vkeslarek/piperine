//! What the run reports: the diagnostic of a position, a refusal by form, and the faults of the constant evaluator turned into the diagnostic of the position they sit in.

use crate::elab::consts::ConstFault;
use super::Elaboration;
use crate::refusal::forms::ValueForm;
use crate::msl::Diagnostic;
use crate::refusal::{Refusal, RefusedForm};
use crate::msl::Span;
use crate::pom::{DesignDiagnostic, FileId};

impl<'d, 't> Elaboration<'d, 't> {
    /// The name of the `const` item `fault` sits in and what is wrong with it, when the fault is
    /// one §3.9.2 makes `E0188`: a `const` initializer that reads a run value, a `const` that
    /// depends on itself, or a `const` without a value outside a trait.
    fn const_item_fault(&self, fault: &ConstFault) -> Option<(String, String)> {
        let named = |decl: crate::pom::DeclId| self.state.design.decl_data(decl).name.as_ref().map_or_else(String::new, |name| name.text.clone());
        match fault {
            ConstFault::NoValue { decl } if !self.state.consts.is_trait_item(*decl) => Some((named(*decl), "it has no value".to_string())),
            ConstFault::Cyclic { constant, .. } => Some((named(*constant), "it depends on itself".to_string())),
            ConstFault::NotConstant { constant, what, .. } => Some((named(*constant), format!("it reads {what}"))),
            _ => None,
        }
    }

    /// Report `fault`, found while folding `position` (`"a param default"`, `"a node's initial
    /// value"`): a read of a param is `P0001` (a value that depends on another param is M2),
    /// anything that needs a run is `E0321`, a form the evaluator does not fold is `P0001`, a panic
    /// (an integer division by zero) is `E0104` (§1 rule 11), and a value with no constant at
    /// elaboration (the argument of a function that has none) is `E0321` like any other value only a run gives.
    ///
    /// A fault found inside the initializer of a `const` item is `E0188` naming the `const`
    /// (§3.9.2): a read of something only a run gives, a `const` without a value outside a trait,
    /// a `const` that depends on itself. A param default keeps `E0321`.
    pub(in crate::elab) fn const_fault(&mut self, fault: &ConstFault, position: &str) {
        let (file, span) = match fault {
            ConstFault::NoValue { decl } => {
                let data = self.state.design.decl_data(*decl);
                (data.file, data.span)
            }
            other => {
                let expr = other.expr().map(|e| self.state.design.expr_node(e));
                let span = expr.map_or_else(Default::default, |e| e.span);
                let file = expr.map_or(crate::pom::FileId(0), |e| self.state.design.decl_data(e.owner).file);
                (file, span)
            }
        };
        if let Some((name, reason)) = self.const_item_fault(fault) {
            let message = format!("the `const` `{name}` is not a const expression: {reason}");
            self.report(file, Diagnostic::new("E0188", message, span, "not a const expression"));
            return;
        }
        match fault {
            // Reported above as E0188.
            ConstFault::Cyclic { .. } | ConstFault::NotConstant { .. } => {}
            // §20: the file of an `include_str!` has no text.
            ConstFault::File { fault, .. } => self.report(file, Diagnostic::new(fault.code(), fault.message(), span, "the file named here")),
            // §22.6.4: the text does not fit the `const`'s type (`E0614`) or is not in the format (`E0706`).
            ConstFault::Table { code, message, .. } => self.report(file, Diagnostic::new(code, message.clone(), span, "the text parsed here")),
            ConstFault::UnboundParam { .. } => self.refuse_with(file, span, ValueForm::ThatReadsParam, [position.to_string()]),
            ConstFault::Dynamic { what, .. } => {
                let message = format!("{position} is not an expression over params and constants: it reads {what}");
                self.report(file, Diagnostic::new("E0321", message, span, "not constant"));
            }
            ConstFault::Refused { refusal, .. } => self.report(file, Refusal::clone(refusal).in_position(position).diagnostic(span)),
            ConstFault::NoValue { .. } => self.refuse_with(file, span, ValueForm::ConstWithoutValueIn, [position.to_string()]),
            ConstFault::Ungenerated { message, .. } => self.report(file, Diagnostic::new("E0342", message.clone(), span, "no such generated item")),
            ConstFault::Leaves { .. } => self.refuse_with(file, span, ValueForm::BreakContinueOrReturnOutsideLoopOr, [position.to_string()]),
            ConstFault::Panic { what, .. } => {
                let message = format!("{position} panics while it is evaluated at elaboration: {what}");
                self.report(file, Diagnostic::new("E0104", message, span, "panics"));
            }
        }
    }

    /// Records `diagnostic`, once: a construct lowered in two halves (the `is` and the `val` of an `Option`)
    /// reports the same refusal at the same place twice, which is one finding.
    pub(in crate::elab) fn report(&mut self, file: FileId, diagnostic: Diagnostic) {
        let found = DesignDiagnostic { file, diagnostic };
        if !self.state.diagnostics.contains(&found) {
            self.state.diagnostics.push(found);
        }
    }

    /// What the builder gave back, or `P0004` for a body id it never handed out (a bug of the elaborator, not of the model);
    /// `None` after the diagnostic.
    pub(in crate::elab) fn built<T>(&mut self, result: Result<T, piperine_ir::BuildError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.internal_error(&error.to_string());
                None
            }
        }
    }

    /// `P0004`: an invariant of the elaborator broken (a bug in it, not in the model), reported at the module of the body being
    /// lowered; the run then gives no `Ir`, as for an ill-formed one (`well_formed`).
    pub(in crate::elab) fn internal_error(&mut self, what: &str) {
        // every lowering runs with frame 0 set; before one, the body asked for (`lowering`) names the module
        let module = self.state.structure.frames.first().map(|frame| frame.module).or_else(|| self.state.structure.lowering.as_ref().map(|key| key.module));
        let (file, span) = match module {
            Some(module) => {
                let data = self.state.design.decl_data(module);
                (data.file, data.name.as_ref().map_or(data.span, |n| n.span))
            }
            None => (FileId(0), Span::default()),
        };
        self.report(file, Diagnostic::new("P0004", format!("internal error: {what}"), span, "this is a bug in the elaborator, not in the model"));
    }

    /// `P0001` at `span`, naming `form`: a construct inside the type-checked subset that elaboration does not lower.
    pub(in crate::elab) fn refuse(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        self.report(file, Refusal::of(form).diagnostic(span));
    }

    /// [`refuse`](Self::refuse) with `details` for the `{}` of the form's words.
    pub(in crate::elab) fn refuse_with(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>, details: impl IntoIterator<Item = String>) {
        let refusal = details.into_iter().fold(Refusal::of(form), Refusal::with_detail);
        self.report(file, refusal.diagnostic(span));
    }

    /// [`refuse`](Self::refuse) unless the read through the frame `frame` passes through a module refused for its own
    /// cause (`refused_modules`): that refusal is the read's, reported once. Any other refusal of the run, of another module or of
    /// another read, hides nothing.
    pub(in crate::elab) fn refuse_unless_refused(&mut self, frame: usize, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        if !self.reads_through_refused(frame) {
            self.refuse(file, span, form);
        }
    }

    /// Whether the frame `frame`, or a frame it hangs from, is of a module refused for its own cause (`refused_modules`).
    pub(in crate::elab) fn reads_through_refused(&self, frame: usize) -> bool {
        let mut at = Some(frame);
        while let Some(index) = at {
            let Some(held) = self.state.structure.frames.get(index) else { return false };
            if self.state.structure.refused_modules.contains(&held.module) {
                return true;
            }
            at = held.parent;
        }
        false
    }

    /// `P0001` for something that failed to lower while no diagnostic was reported: the one place the elaborator says
    /// "nothing vanishes silently".
    pub(in crate::elab) fn refuse_silent_failure(&mut self, file: FileId, span: Span, form: impl Into<RefusedForm>) {
        self.refuse(file, span, form);
    }
}
