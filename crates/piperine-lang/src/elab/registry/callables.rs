use crate::pom::{ElabError, ElabErrorKind, ValueType};
use std::collections::HashMap;

pub trait CallableDef: Send + Sync {
    fn name(&self) -> &str;

    /// Declared parameter types, in call order — the structural signature
    /// overload resolution matches against (SPEC declared-language-surface
    /// DLS-06/07). `None` means this candidate carries no structural
    /// signature (e.g. a legacy/untyped registration); such a candidate
    /// always validates, preserving today's arity-agnostic behavior for
    /// any `CallableDef` that hasn't opted into typed signatures.
    fn param_types(&self) -> Option<&[ValueType]> { None }

    /// Whether `arg_types` structurally match this candidate's declared
    /// signature — exact match, no implicit widening (mirrors the existing
    /// "no implicit Integer→Real cast" rule proven in `type_casts.rs`).
    /// A candidate with no `param_types` always validates.
    fn validate_call(&self, arg_types: &[ValueType]) -> Result<(), ElabError> {
        match self.param_types() {
            Some(params) if params != arg_types => Err(ElabError::from(ElabErrorKind::Other(format!(
                "call to `{}` does not match its declared signature `{}({})` — got ({})",
                self.name(),
                self.name(),
                params.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
                arg_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
            )))),
            _ => Ok(()),
        }
    }

    fn is_capability(&self) -> bool { false }
}

pub struct CallableRegistry {
    /// Every name maps to an **overload set** — a `fn`/`extern fn`/`extern
    /// task` redeclared with a different parameter-type signature is a
    /// valid overload, not a duplicate-declaration error (SPEC DLS-06).
    callables: HashMap<String, Vec<Box<dyn CallableDef>>>,
}

impl Default for CallableRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CallableRegistry {
    pub fn new() -> Self {
        Self { callables: HashMap::new() }
    }

    /// Register a candidate for `def.name()` — appends to that name's
    /// overload set rather than replacing any existing candidate.
    pub fn register<C: CallableDef + 'static>(&mut self, def: C) {
        self.callables.entry(def.name().to_string()).or_default().push(Box::new(def));
    }

    /// Every registered candidate for `name`, in registration order.
    pub fn candidates(&self, name: &str) -> &[Box<dyn CallableDef>] {
        self.callables.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// A plain, non-disambiguating lookup: the sole registered candidate for
    /// `name`, or the first-registered one if the name is overloaded. A
    /// single-candidate name resolves exactly as it did before overloading
    /// existed. Callers that need to pick the right overload by argument
    /// type use the overload-resolution algorithm instead.
    pub fn lookup(&self, name: &str) -> Option<&dyn CallableDef> {
        self.callables.get(name).and_then(|v| v.first()).map(|c| c.as_ref())
    }

    /// Walk a program and resolve calls.
    pub fn resolve_calls(&self, design: &mut crate::pom::Design) -> Result<(), ElabError> {
        crate::elab::resolve::resolve_calls(design)
    }
}
