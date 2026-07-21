//! `OperatorRegistry` — the textual home for runtime operators (`ddt`,
//! `delay`, `slew`, `cross`, …), the first time these names have any
//! `piperine-lang`-level presence at all (declared-language-surface T10,
//! DLS-01 groundwork; SPEC P4-AC4 migrates real declarations in T22).
//!
//! Mirrors `CallableRegistry`'s overload-aware shape exactly — same
//! algorithm, a different backing map, keyed by operator name instead of
//! function name (design.md's "Registry extensions" component).

use super::callables::CallableDef;
use crate::parse::ast::ExternSig;
use crate::pom::{ElabError, ElabErrorKind, ValueType};
use std::collections::HashMap;

/// A registered `extern operator` declaration — wraps the parsed signature
/// so it can be stored as a `CallableDef` candidate in `OperatorRegistry`'s
/// overload sets, exactly like `extern fn`/`extern task` do for
/// `CallableRegistry`.
pub struct ExternOperatorDecl(pub ExternSig);

impl CallableDef for ExternOperatorDecl {
    fn name(&self) -> &str { &self.0.name }
    // No structural `param_types` yet — real `extern operator` bodies (with
    // resolvable param types) land in T22's migration; until then every
    // candidate is permissively "always matches" (the `CallableDef` default),
    // consistent with `FnDecl`'s current (pre-T16/T22) scope.
}

pub struct OperatorRegistry {
    /// Every operator name maps to an overload set — mirrors
    /// `CallableRegistry`'s storage shape.
    operators: HashMap<String, Vec<Box<dyn CallableDef>>>,
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl OperatorRegistry {
    pub fn new() -> Self {
        Self { operators: HashMap::new() }
    }

    /// Register a candidate for `def.name()` — appends to that name's
    /// overload set rather than replacing any existing candidate. Generic
    /// over `CallableDef` (like `CallableRegistry::register`) rather than
    /// fixed to `ExternOperatorDecl`, so the same registry machinery is
    /// exercisable with synthetic fixtures independent of AST plumbing.
    pub fn register<C: CallableDef + 'static>(&mut self, def: C) {
        self.operators.entry(def.name().to_string()).or_default().push(Box::new(def));
    }

    /// Every registered candidate for `name`, in registration order.
    pub fn candidates(&self, name: &str) -> &[Box<dyn CallableDef>] {
        self.operators.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// A plain, non-disambiguating lookup — mirrors
    /// `CallableRegistry::lookup`.
    pub fn lookup(&self, name: &str) -> Option<&dyn CallableDef> {
        self.operators.get(name).and_then(|v| v.first()).map(|c| c.as_ref())
    }

    /// Overload resolution — identical algorithm to
    /// `CallableRegistry::resolve` (SPEC DLS-07), applied to the operator
    /// namespace instead of the function namespace.
    pub fn resolve(&self, name: &str, arg_types: &[ValueType]) -> Result<&dyn CallableDef, ElabError> {
        let candidates = self.candidates(name);

        let matching: Vec<&dyn CallableDef> = candidates
            .iter()
            .map(|c| c.as_ref())
            .filter(|c| match c.param_types() {
                Some(params) => params == arg_types,
                None => true,
            })
            .collect();

        match matching.len() {
            0 => Err(ElabError::from(ElabErrorKind::Other(format!(
                "no overload of operator `{name}` matches argument types ({}); candidates tried: [{}]",
                arg_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
                candidates.iter().map(|c| c.signature_desc()).collect::<Vec<_>>().join(", "),
            )))),
            1 => Ok(matching[0]),
            _ => Err(ElabError::from(ElabErrorKind::Other(format!(
                "ambiguous operator call `{name}` with argument types ({}); matching candidates: [{}]",
                arg_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
                matching.iter().map(|c| c.signature_desc()).collect::<Vec<_>>().join(", "),
            )))),
        }
    }
}
