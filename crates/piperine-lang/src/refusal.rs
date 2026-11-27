//! The front end's one owner of "this form is not supported yet" (`P0001`).
//!
//! Three channels reach a refusal: the checker, the elaborator and the constant evaluator. Each builds a
//! [`Refusal`]; [`Refusal::diagnostic`] is the only place that words the diagnostic.

use crate::msl::{Diagnostic, Span};

/// One construct the front end refuses by name (`P0001`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    form: RefusedForm,
    position: Option<String>,
}

/// The forms the front end refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusedForm {
    /// A form written as free text: the words a user reads.
    Text(String),
}

impl RefusedForm {
    /// The words that name the form.
    fn words(&self) -> &str {
        match self {
            RefusedForm::Text(words) => words,
        }
    }
}

impl Refusal {
    /// The refusal of `form`.
    pub fn of(form: RefusedForm) -> Refusal {
        Refusal { form, position: None }
    }

    /// The same refusal, found while folding `position` (`"a param default"`).
    pub fn in_position(self, position: &str) -> Refusal {
        Refusal { position: Some(position.to_string()), ..self }
    }

    /// What is refused, as a user reads it.
    pub fn subject(&self) -> String {
        match &self.position {
            Some(position) => format!("{} in {position}", self.form.words()),
            None => self.form.words().to_string(),
        }
    }

    /// The `P0001` diagnostic at `span`: the one place the code is worded.
    pub fn diagnostic(&self, span: Span) -> Diagnostic {
        let subject = self.subject();
        Diagnostic::new("P0001", format!("{subject} is not supported yet"), span, format!("{subject} is outside the M1 subset"))
    }
}
