//! Overload resolution algorithm (declared-language-surface T9, DLS-07).
//!
//! Dedicated, isolated fixture proving `CallableRegistry::resolve`'s four
//! paths on synthetic candidates — proven independently of any P4 migration
//! or real PHDL body, per design.md's risk-mitigation isolation requirement:
//!
//! 1. 1-candidate resolves normally.
//! 2. N-candidate-disjoint-types resolves the matching one by arg type.
//! 3. 0-match fails loud, naming the call site and every candidate signature
//!    tried.
//! 4. Ambiguous-match (two structurally identical signatures) fails loud,
//!    naming every matching candidate.

use piperine_lang::elab::registry::{CallableDef, CallableRegistry};
use piperine_lang::pom::{ElabError, ValueType};

/// `Result::expect_err` requires `Debug` on the `Ok` type, which `&dyn
/// CallableDef` doesn't implement — this helper sidesteps that.
fn expect_err(result: Result<&dyn CallableDef, ElabError>, msg: &str) -> ElabError {
    match result {
        Ok(c) => panic!("{msg}: got Ok({})", c.name()),
        Err(e) => e,
    }
}

/// A minimal synthetic `CallableDef` — a name plus a declared param-type
/// signature, nothing else. Lets the fixture drive the algorithm directly
/// without any AST/typecheck plumbing.
struct Candidate {
    name: &'static str,
    params: Vec<ValueType>,
}

impl CallableDef for Candidate {
    fn name(&self) -> &str { self.name }
    fn param_types(&self) -> Option<&[ValueType]> { Some(&self.params) }
}

#[test]
fn one_candidate_resolves_normally() {
    let mut reg = CallableRegistry::new();
    reg.register(Candidate { name: "sin", params: vec![ValueType::Real] });

    let resolved = reg.resolve("sin", &[ValueType::Real]).expect("sole candidate should resolve");
    assert_eq!(resolved.name(), "sin");
}

#[test]
fn n_candidate_disjoint_types_resolves_by_argument_type() {
    // Real::from(Integer) / Real::from(Boolean) / Real::from(Quad) — the
    // cast use case this whole mechanism exists for (P4-AC7).
    let mut reg = CallableRegistry::new();
    reg.register(Candidate { name: "from", params: vec![ValueType::Integer] });
    reg.register(Candidate { name: "from", params: vec![ValueType::Boolean] });
    reg.register(Candidate { name: "from", params: vec![ValueType::Quad] });

    let via_integer = reg.resolve("from", &[ValueType::Integer]).expect("Integer overload should resolve");
    assert_eq!(via_integer.param_types(), Some(&[ValueType::Integer][..]));

    let via_boolean = reg.resolve("from", &[ValueType::Boolean]).expect("Boolean overload should resolve");
    assert_eq!(via_boolean.param_types(), Some(&[ValueType::Boolean][..]));

    let via_quad = reg.resolve("from", &[ValueType::Quad]).expect("Quad overload should resolve");
    assert_eq!(via_quad.param_types(), Some(&[ValueType::Quad][..]));
}

#[test]
fn zero_match_fails_loud_naming_call_site_and_every_candidate_tried() {
    let mut reg = CallableRegistry::new();
    reg.register(Candidate { name: "from", params: vec![ValueType::Integer] });
    reg.register(Candidate { name: "from", params: vec![ValueType::Boolean] });

    // No overload accepts a Str argument.
    let err = expect_err(reg.resolve("from", &[ValueType::Str]), "no candidate should match Str");
    let msg = err.to_string();

    assert!(msg.contains("from"), "error should name the call site: {msg}");
    assert!(msg.contains("Integer"), "error should name the Integer candidate tried: {msg}");
    assert!(msg.contains("Boolean"), "error should name the Boolean candidate tried: {msg}");
}

#[test]
fn zero_match_on_arity_mismatch_also_fails_loud() {
    // An arity mismatch is just a structural type mismatch (no separate
    // arity step) — still a 0-match, still names the candidate tried.
    let mut reg = CallableRegistry::new();
    reg.register(Candidate { name: "clamp", params: vec![ValueType::Real, ValueType::Real] });

    let err = expect_err(reg.resolve("clamp", &[ValueType::Real]), "wrong arity should not match");
    let msg = err.to_string();
    assert!(msg.contains("clamp"), "error should name the call site: {msg}");
}

#[test]
fn ambiguous_match_fails_loud_naming_every_matching_candidate() {
    // Two structurally identical signatures registered for the same name —
    // the defensive backstop path (in practice caught earlier at
    // Register-pass time as a duplicate declaration, per design.md, but the
    // resolution algorithm itself must never silently pick one).
    let mut reg = CallableRegistry::new();
    reg.register(Candidate { name: "weird", params: vec![ValueType::Real] });
    reg.register(Candidate { name: "weird", params: vec![ValueType::Real] });

    let err = expect_err(reg.resolve("weird", &[ValueType::Real]), "duplicate signatures must be ambiguous");
    let msg = err.to_string();

    assert!(msg.contains("ambiguous"), "error should flag ambiguity distinctly from a 0-match error: {msg}");
    assert!(msg.contains("weird"), "error should name the call site: {msg}");
}
