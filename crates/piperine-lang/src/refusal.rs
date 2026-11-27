//! The front end's one owner of "this form is not supported yet" (`P0001`).
//!
//! Three channels reach a refusal: the checker, the elaborator and the constant evaluator. Each builds a
//! [`Refusal`]; [`Refusal::diagnostic`] is the only place that words the diagnostic. The forms are named in
//! [`forms`], one table row each.

pub mod forms;
mod repeated;
mod syntax;

use crate::msl::{Diagnostic, Span};
pub use forms::{AnalogForm, ConstantForm, DigitalForm, GenericsForm, PatternForm, StructureForm, ValueForm};

/// One construct the front end refuses by name (`P0001`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    form: RefusedForm,
    details: Vec<String>,
    position: Option<String>,
}

/// Who closes a refused form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalOwner {
    /// A row of `GAPS.md` owns it (the id).
    Gap(&'static str),
    /// Nobody has swept it yet (`spec-closure` does).
    Unswept,
}

/// One row of the refusal table: the words a user reads and who owns closing it. A `{}` in `text` is filled, in order, by a
/// detail of the [`Refusal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusalRow {
    /// The form the row words.
    pub form: RefusedForm,
    /// The subject of the message, with a `{}` for each detail.
    pub text: &'static str,
    /// Who closes the form.
    pub owner: RefusalOwner,
}

/// The forms the front end refuses, grouped by family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusedForm {
    /// A value, a type or an expression form.
    Value(ValueForm),
    /// A pattern form.
    Pattern(PatternForm),
    /// An analog form.
    Analog(AnalogForm),
    /// A digital or carrier form.
    Digital(DigitalForm),
    /// A structure form: modules, children, instances, generation.
    Structure(StructureForm),
    /// A generic, trait or derive form.
    Generics(GenericsForm),
    /// A form of the constant evaluator.
    Constant(ConstantForm),
}

impl RefusedForm {
    /// The words that name the form, with a `{}` for each detail.
    fn text(&self) -> &str {
        match self {
            RefusedForm::Value(form) => form.row().text,
            RefusedForm::Pattern(form) => form.row().text,
            RefusedForm::Analog(form) => form.row().text,
            RefusedForm::Digital(form) => form.row().text,
            RefusedForm::Structure(form) => form.row().text,
            RefusedForm::Generics(form) => form.row().text,
            RefusedForm::Constant(form) => form.row().text,
        }
    }
}

impl Refusal {
    /// The refusal of `form`.
    pub fn of(form: impl Into<RefusedForm>) -> Refusal {
        Refusal { form: form.into(), details: Vec::new(), position: None }
    }

    /// The same refusal with one more detail, the next `{}` of its words.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Refusal {
        self.details.push(detail.into());
        self
    }

    /// The same refusal, found while folding `position` (`"a param default"`).
    pub fn in_position(self, position: &str) -> Refusal {
        Refusal { position: Some(position.to_string()), ..self }
    }

    /// What is refused, as a user reads it.
    pub fn subject(&self) -> String {
        let mut details = self.details.iter();
        let mut words = String::new();
        for (index, piece) in self.form.text().split("{}").enumerate() {
            if index > 0 {
                words.push_str(details.next().map_or("", String::as_str));
            }
            words.push_str(piece);
        }
        match &self.position {
            Some(position) => format!("{words} in {position}"),
            None => words,
        }
    }

    /// The `P0001` diagnostic at `span`: the one place the code is worded.
    pub fn diagnostic(&self, span: Span) -> Diagnostic {
        Diagnostic::new("P0001", format!("{} is not supported yet", self.subject()), span, "not supported yet")
    }
}
