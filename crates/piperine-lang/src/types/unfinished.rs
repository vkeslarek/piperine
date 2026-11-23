//! The unfinished-forms gate (generics phase C, design-c.md "Interim"): a form the checker can type before the
//! elaborator lowers it stays refused, with the `P0001` `main` gives it, by every production entry until the task that
//! finishes it flips it. A test admits a form by naming it ([`Types::check_admitting`],
//! [`Types::check_model_admitting`]); no production entry admits any.
//!
//! [`Types::check_admitting`]: super::Types::check_admitting
//! [`Types::check_model_admitting`]: super::Types::check_model_admitting

/// One form of the language whose checking runs ahead of its lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnfinishedForm {
    /// A generic module: checked at its definition and placed with inferred arguments (T60, T61), lowered per instance
    /// (T65 to T68); flipped by T71.
    GenericModules,
    /// Generation `for`/`if` in a module body (T72 to T76); flipped by T77.
    Generation,
    /// Generation `for`/`if` around reactions of a `digital` block (T78, T79); flipped by T80, which deletes the gate.
    DigitalGeneration,
}

/// The forms an entry admits: none for production, the ones a test names otherwise.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnfinishedForms {
    /// The admitted forms.
    admitted: Vec<UnfinishedForm>,
}

impl UnfinishedForms {
    /// The forms production still refuses, in the order their tasks flip them.
    pub const IN_FORCE: [UnfinishedForm; 3] = [UnfinishedForm::GenericModules, UnfinishedForm::Generation, UnfinishedForm::DigitalGeneration];

    /// The gate production uses: every form refused.
    pub fn closed() -> UnfinishedForms {
        UnfinishedForms { admitted: Vec::new() }
    }

    /// The gate that admits exactly `forms`.
    pub fn admitting(forms: &[UnfinishedForm]) -> UnfinishedForms {
        UnfinishedForms { admitted: forms.to_vec() }
    }

    /// Whether `form` is admitted.
    pub fn admits(&self, form: UnfinishedForm) -> bool {
        self.admitted.contains(&form)
    }
}
