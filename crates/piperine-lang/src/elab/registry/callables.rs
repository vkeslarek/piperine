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

    /// A human-readable signature, used in overload-resolution error
    /// messages (SPEC DLS-07: "naming every candidate signature tried").
    fn signature_desc(&self) -> String {
        match self.param_types() {
            Some(params) => format!(
                "{}({})",
                self.name(),
                params.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
            ),
            None => format!("{}(..)", self.name()),
        }
    }
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

    /// Overload resolution (SPEC DLS-07): pick the candidate registered for
    /// `name` whose declared parameter types structurally match
    /// `arg_types` exactly (no implicit widening — a candidate's arity is
    /// simply `param_types().len()`, so an arity mismatch is just a
    /// structural type mismatch, no separate arity step is needed).
    ///
    /// - Zero matching candidates → fail loud, naming every original
    ///   candidate's signature (so the author sees what *was* available).
    /// - Exactly one matching candidate → that's the resolution.
    /// - More than one matching candidate → fail loud as an ambiguous call,
    ///   naming every matching candidate (only possible with a duplicate
    ///   signature registered twice — a defensive backstop).
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
                "no overload of `{name}` matches argument types ({}); candidates tried: [{}]",
                arg_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
                candidates.iter().map(|c| c.signature_desc()).collect::<Vec<_>>().join(", "),
            )))),
            1 => Ok(matching[0]),
            _ => Err(ElabError::from(ElabErrorKind::Other(format!(
                "ambiguous call to `{name}` with argument types ({}); matching candidates: [{}]",
                arg_types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>().join(", "),
                matching.iter().map(|c| c.signature_desc()).collect::<Vec<_>>().join(", "),
            )))),
        }
    }
}
