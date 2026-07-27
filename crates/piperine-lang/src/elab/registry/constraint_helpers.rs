//! The constraint-helper registry (dv-core T7, DVC-09): every helper
//! `headers/constraints.phdl` declares, with the class
//! ([`QuantityClass`]) and analysis its `@quantity(class, analysis)`
//! marker assigns it. The elaborator reads this table to classify `var`/
//! `target` expressions and to refuse an undeclared helper loud (T8/T10).

use std::collections::HashMap;

use crate::pom::constraint::QuantityClass;
use crate::pom::{ElabError, ElabErrorKind};

/// One declared constraint helper: its class and the analysis it belongs
/// to (`"ac"`, …) — recorded so a later pass can refuse a cross-analysis
/// read (DVC-08 AC6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintHelper {
    pub class: QuantityClass,
    pub analysis: String,
}

#[derive(Debug, Default)]
pub struct ConstraintHelperRegistry {
    helpers: HashMap<String, ConstraintHelper>,
}

impl ConstraintHelperRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a helper under `name`. A helper already registered is a
    /// duplicate declaration and fails loud — the declared-language-surface
    /// rule that an `extern` declaration is never shadowed applies to the
    /// class marker exactly as it does to the signature.
    pub fn register(&mut self, name: &str, helper: ConstraintHelper) -> Result<(), ElabError> {
        if self.helpers.contains_key(name) {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "constraint helper `{name}` is declared more than once"
            ))));
        }
        self.helpers.insert(name.to_string(), helper);
        Ok(())
    }

    /// The helper registered under `name`, if any.
    pub fn lookup(&self, name: &str) -> Option<&ConstraintHelper> {
        self.helpers.get(name)
    }

    /// The helper registered under `name`, or a loud error naming it —
    /// the path constraint elaboration (T8) takes for every helper call,
    /// so an undeclared helper in a constraint can never pass silently.
    pub fn require(&self, name: &str) -> Result<&ConstraintHelper, ElabError> {
        self.lookup(name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!(
                "undeclared constraint helper `{name}` \
                 — the declared helpers live in headers/constraints.phdl"
            )))
        })
    }
}
