//! `CallableRegistry` overload-aware storage (declared-language-surface T8,
//! DLS-06). Proves: registration appends into a per-name overload set rather
//! than replacing a prior candidate, a single-candidate name still resolves
//! exactly as it did before overloading existed, and `validate_call` does a
//! real, exact structural param-type match (no implicit widening).

use piperine_lang::elab::registry::{CallableDef, CallableRegistry};
use piperine_lang::pom::ValueType;

/// A minimal synthetic `CallableDef` for exercising the registry without
/// needing a real `FnDecl`/AST — the registry's storage/lookup contract is
/// independent of what backs a candidate.
struct Candidate {
    name: &'static str,
    params: Vec<ValueType>,
}

impl CallableDef for Candidate {
    fn name(&self) -> &str { self.name }
    fn param_types(&self) -> Option<&[ValueType]> { Some(&self.params) }
}

#[test]
fn single_registration_resolves_via_lookup_exactly_as_before_overloading() {
    let mut reg = CallableRegistry::new();
    reg.register(Candidate { name: "sin", params: vec![ValueType::Real] });

    assert_eq!(reg.candidates("sin").len(), 1);
    let found = reg.lookup("sin").expect("single candidate should be found");
    assert_eq!(found.name(), "sin");
}

#[test]
fn duplicate_name_with_different_signature_forms_an_overload_set() {
    let mut reg = CallableRegistry::new();
    reg.register(Candidate { name: "from", params: vec![ValueType::Integer] });
    reg.register(Candidate { name: "from", params: vec![ValueType::Boolean] });

    // Not a duplicate-declaration error — both candidates survive (DLS-06).
    let candidates = reg.candidates("from");
    assert_eq!(candidates.len(), 2, "differing signatures must be accepted as an overload set (DLS-06)");
}

#[test]
fn validate_call_accepts_exact_structural_param_type_match() {
    let c = Candidate { name: "from", params: vec![ValueType::Integer] };
    assert!(c.validate_call(&[ValueType::Integer]).is_ok());
}

#[test]
fn validate_call_rejects_mismatched_param_types_no_implicit_widening() {
    // Mirrors the existing "implicit cast from Integer to Real not allowed"
    // rule (type_casts.rs) — an Integer arg against a Real-typed candidate
    // must not be silently accepted.
    let c = Candidate { name: "from", params: vec![ValueType::Real] };
    assert!(c.validate_call(&[ValueType::Integer]).is_err());
}

#[test]
fn validate_call_rejects_arity_mismatch() {
    let c = Candidate { name: "from", params: vec![ValueType::Real, ValueType::Real] };
    assert!(c.validate_call(&[ValueType::Real]).is_err());
}
