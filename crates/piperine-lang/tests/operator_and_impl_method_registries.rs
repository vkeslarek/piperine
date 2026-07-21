//! `OperatorRegistry` + the per-type `extern impl` method table
//! (declared-language-surface T10, DLS-01/13/14 groundwork). Proves both
//! new registries share `CallableRegistry`'s overload-resolution algorithm
//! (same four paths as `overload_resolution.rs`, different backing map),
//! and that `SchemaRegistry`'s `AttrField`s carry a `decl_span`.

use piperine_lang::elab::registry::{
    AttrField, CallableDef, ImplMethodTable, OperatorRegistry, SchemaRegistry, SchemaShape,
};
use piperine_lang::pom::{ElabError, ValueType};

/// A minimal synthetic `CallableDef`, identical in spirit to
/// `overload_resolution.rs`'s fixture — proves the registry mechanism
/// independent of AST/typecheck plumbing.
struct Candidate {
    name: &'static str,
    params: Vec<ValueType>,
}

impl CallableDef for Candidate {
    fn name(&self) -> &str { self.name }
    fn param_types(&self) -> Option<&[ValueType]> { Some(&self.params) }
}

fn expect_err<T>(result: Result<T, ElabError>, msg: &str) -> ElabError {
    match result {
        Ok(_) => panic!("{msg}: got Ok"),
        Err(e) => e,
    }
}

// ─────────────────────────── OperatorRegistry ──────────────────────────────

#[test]
fn operator_registry_one_candidate_resolves_normally() {
    let mut reg = OperatorRegistry::new();
    reg.register(Candidate { name: "ddt", params: vec![ValueType::Real] });

    let resolved = reg.resolve("ddt", &[ValueType::Real]).expect("sole candidate should resolve");
    assert_eq!(resolved.name(), "ddt");
}

#[test]
fn operator_registry_disjoint_overloads_resolve_by_argument_type() {
    let mut reg = OperatorRegistry::new();
    reg.register(Candidate { name: "delay", params: vec![ValueType::Real] });
    reg.register(Candidate { name: "delay", params: vec![ValueType::Integer] });

    let via_real = reg.resolve("delay", &[ValueType::Real]).expect("Real overload should resolve");
    assert_eq!(via_real.param_types(), Some(&[ValueType::Real][..]));

    let via_int = reg.resolve("delay", &[ValueType::Integer]).expect("Integer overload should resolve");
    assert_eq!(via_int.param_types(), Some(&[ValueType::Integer][..]));
}

#[test]
fn operator_registry_zero_match_fails_loud_naming_candidates_tried() {
    let mut reg = OperatorRegistry::new();
    reg.register(Candidate { name: "slew", params: vec![ValueType::Real] });

    let err = expect_err(reg.resolve("slew", &[ValueType::Str]), "no candidate should match Str");
    let msg = err.to_string();
    assert!(msg.contains("slew"), "error should name the operator: {msg}");
    assert!(msg.contains("Real"), "error should name the candidate tried: {msg}");
}

#[test]
fn operator_registry_ambiguous_match_fails_loud_naming_every_matching_candidate() {
    let mut reg = OperatorRegistry::new();
    reg.register(Candidate { name: "cross", params: vec![ValueType::Real] });
    reg.register(Candidate { name: "cross", params: vec![ValueType::Real] });

    let err = expect_err(reg.resolve("cross", &[ValueType::Real]), "duplicate signatures must be ambiguous");
    let msg = err.to_string();
    assert!(msg.contains("ambiguous"), "error should flag ambiguity: {msg}");
    assert!(msg.contains("cross"), "error should name the operator: {msg}");
}

// ─────────────────────────── ImplMethodTable ───────────────────────────────

#[test]
fn impl_method_table_resolves_overloaded_methods_on_the_same_type() {
    let mut table = ImplMethodTable::new();
    table.register_impl_method("Real", Candidate { name: "from", params: vec![ValueType::Integer] });
    table.register_impl_method("Real", Candidate { name: "from", params: vec![ValueType::Boolean] });

    let via_integer = table.resolve("Real", "from", &[ValueType::Integer]).expect("Integer overload should resolve");
    assert_eq!(via_integer.param_types(), Some(&[ValueType::Integer][..]));

    let via_boolean = table.resolve("Real", "from", &[ValueType::Boolean]).expect("Boolean overload should resolve");
    assert_eq!(via_boolean.param_types(), Some(&[ValueType::Boolean][..]));
}

#[test]
fn impl_method_table_namespaces_methods_by_owning_type() {
    // Same method name (`from`) on two different types must not collide —
    // the table is keyed by (type_name, method_name), not method_name alone.
    let mut table = ImplMethodTable::new();
    table.register_impl_method("Real", Candidate { name: "from", params: vec![ValueType::Integer] });
    table.register_impl_method("Boolean", Candidate { name: "from", params: vec![ValueType::Integer] });

    assert_eq!(table.candidates("Real", "from").len(), 1);
    assert_eq!(table.candidates("Boolean", "from").len(), 1);

    // A method that only exists on `Boolean` must not resolve under `Real`.
    let err = expect_err(
        table.resolve("Real", "does_not_exist", &[ValueType::Integer]),
        "unregistered method on a real type should not resolve",
    );
    assert!(err.to_string().contains("does_not_exist"));
}

#[test]
fn impl_method_table_zero_match_fails_loud() {
    let mut table = ImplMethodTable::new();
    table.register_impl_method("Real", Candidate { name: "from", params: vec![ValueType::Integer] });

    let err = expect_err(
        table.resolve("Real", "from", &[ValueType::Str]),
        "no candidate should match Str",
    );
    let msg = err.to_string();
    assert!(msg.contains("Real::from"), "error should name the type::method call site: {msg}");
}

// ─────────────────────────── SchemaRegistry AttrField ──────────────────────

#[test]
fn attr_field_carries_a_decl_span() {
    let mut schemas = SchemaRegistry::new();
    let span = miette::SourceSpan::from((5, 6));
    schemas.register_declared(
        "device",
        vec![AttrField {
            name: "plugin".into(),
            ty: "String".into(),
            required: true,
            default: None,
            decl_span: Some(span),
        }],
        None,
    );

    let SchemaShape::Declared(fields) = schemas.shape("device").expect("schema should be registered") else {
        panic!("expected a Declared schema shape");
    };
    assert_eq!(fields[0].decl_span, Some(span), "the field's own decl_span must round-trip through the registry");
}
