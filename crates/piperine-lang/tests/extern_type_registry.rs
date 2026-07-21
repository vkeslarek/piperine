//! `TypeRegistry` gains an `extern` variant (declared-language-surface T7,
//! DLS-01 groundwork). Proves: an `extern type` item registers into
//! `TypeRegistry` with its `decl_span`, and a duplicate name (extern vs
//! extern, or extern vs a plain type) is a duplicate-declaration error — no
//! shadowing (spec Edge Cases).

use piperine_lang::elab::registry::{TypeDefKind, TypeRegistry};
use piperine_lang::{parse_and_elaborate, SourceMap};

#[test]
fn extern_type_registers_with_its_decl_span() {
    let mut reg = TypeRegistry::new();
    let span = Some(miette::SourceSpan::from((0, 18)));
    reg.register(TypeDefKind::Extern { name: "Real".into(), decl_span: span });

    let found = reg.lookup("Real").expect("extern type should be registered");
    assert_eq!(found.name(), "Real");
    match found {
        TypeDefKind::Extern { decl_span, .. } => assert_eq!(*decl_span, span),
        other => panic!("expected TypeDefKind::Extern, got a different kind: {}", other.name()),
    }
}

#[test]
fn extern_type_alone_elaborates_without_error() {
    let sm = SourceMap::new(std::path::PathBuf::from("."));
    let result = parse_and_elaborate("extern type Foo;", &sm);
    assert!(result.is_ok(), "a lone `extern type` declaration should elaborate cleanly: {result:?}");
}

#[test]
fn duplicate_extern_type_same_name_is_a_duplicate_declaration_error() {
    let sm = SourceMap::new(std::path::PathBuf::from("."));
    let result = parse_and_elaborate("extern type Foo; extern type Foo;", &sm);
    let err = result.expect_err("two `extern type Foo;` declarations must be a duplicate-declaration error");
    let msg = err.to_string();
    assert!(msg.contains("Foo"), "error should name the colliding type: {msg}");
}

#[test]
fn extern_type_colliding_with_a_plain_type_is_a_duplicate_declaration_error() {
    let sm = SourceMap::new(std::path::PathBuf::from("."));
    let result = parse_and_elaborate("enum Foo { A, B } extern type Foo;", &sm);
    let err = result.expect_err("`extern type Foo;` colliding with an existing plain type `Foo` must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("Foo"), "error should name the colliding type: {msg}");
}
