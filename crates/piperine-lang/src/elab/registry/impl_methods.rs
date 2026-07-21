//! The per-type native method table — the textual home for `extern impl
//! TypeName { fn method(...) -> Ret; }` methods (declared-language-surface
//! T10, DLS-13/14). Unlike `extern fn`, these methods are namespaced by
//! their owning type, not global, so the table is keyed by
//! `(type_name, method_name)` rather than by name alone. Overload-aware
//! exactly like `CallableRegistry`/`OperatorRegistry` — the first real
//! consumer is the cast migration (`Real::from(x: Integer)` / `Real::from(x:
//! Boolean)` as one overloaded `from` per target type, T17).

use super::callables::CallableDef;
use crate::pom::{ElabError, ElabErrorKind, ValueType};
use std::collections::HashMap;

pub struct ImplMethodTable {
    /// `(type_name, method_name)` maps to an overload set — mirrors
    /// `CallableRegistry`'s storage shape, namespaced by owning type.
    methods: HashMap<(String, String), Vec<Box<dyn CallableDef>>>,
}

impl Default for ImplMethodTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ImplMethodTable {
    pub fn new() -> Self {
        Self { methods: HashMap::new() }
    }

    /// Register a candidate for `type_name::method.name()` — appends to
    /// that (type, method) pair's overload set rather than replacing any
    /// existing candidate.
    pub fn register_impl_method<C: CallableDef + 'static>(&mut self, type_name: &str, method: C) {
        let key = (type_name.to_string(), method.name().to_string());
        self.methods.entry(key).or_default().push(Box::new(method));
    }

    /// Every registered candidate for `type_name::method_name`, in
    /// registration order.
    pub fn candidates(&self, type_name: &str, method_name: &str) -> &[Box<dyn CallableDef>] {
        self.methods
            .get(&(type_name.to_string(), method_name.to_string()))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Overload resolution — identical algorithm to
    /// `CallableRegistry::resolve` (SPEC DLS-07), applied to a single type's
    /// method namespace.
    pub fn resolve(&self, type_name: &str, method_name: &str, arg_types: &[ValueType]) -> Result<&dyn CallableDef, ElabError> {
        let candidates = self.candidates(type_name, method_name);

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
                "no overload of `{type_name}::{method_name}` matches argument types ({}); candidates tried: [{}]",
                arg_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
                candidates.iter().map(|c| c.signature_desc()).collect::<Vec<_>>().join(", "),
            )))),
            1 => Ok(matching[0]),
            _ => Err(ElabError::from(ElabErrorKind::Other(format!(
                "ambiguous call to `{type_name}::{method_name}` with argument types ({}); matching candidates: [{}]",
                arg_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
                matching.iter().map(|c| c.signature_desc()).collect::<Vec<_>>().join(", "),
            )))),
        }
    }
}
