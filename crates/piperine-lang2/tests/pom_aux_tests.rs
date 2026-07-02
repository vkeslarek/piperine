//! Tests for the Piperine Object Model (POM) auxiliary types.
//!
//! See `docs/reflection_api.md` for the full specification.

use piperine_lang::pom::{Id, ReflectError, Selection, Value};

// ── Value ──────────────────────────────────────────────────────────────────────

#[test]
fn value_real_construction_and_access() {
    let v = Value::Real(3.14);
    assert_eq!(v.as_real(), Some(3.14));
    assert_eq!(v.as_integer(), None);
}

#[test]
fn value_natural_construction_and_access() {
    let v = Value::Natural(42);
    assert_eq!(v.as_natural(), Some(42));
}

#[test]
fn value_boolean_construction_and_access() {
    let v = Value::Boolean(true);
    assert_eq!(v.as_boolean(), Some(true));
}

#[test]
fn value_string_construction_and_access() {
    let v = Value::String("hello".into());
    assert_eq!(v.as_string(), Some("hello"));
}

#[test]
fn value_integer_construction_and_access() {
    let v = Value::Integer(-7);
    assert_eq!(v.as_integer(), Some(-7));
}

#[test]
fn value_type_name() {
    assert_eq!(Value::Real(0.0).type_name(), "Real");
    assert_eq!(Value::Natural(0).type_name(), "Natural");
    assert_eq!(Value::Boolean(false).type_name(), "Boolean");
    assert_eq!(Value::String("".into()).type_name(), "String");
}

// ── Selection ──────────────────────────────────────────────────────────────────

#[test]
fn selection_empty() {
    let sel: Selection<i32> = Selection::new();
    assert!(sel.is_empty());
    assert_eq!(sel.len(), 0);
    assert_eq!(sel.first(), None);
}

#[test]
fn selection_from_vec() {
    let sel = Selection::from_vec(vec![1, 2, 3]);
    assert_eq!(sel.len(), 3);
    assert!(!sel.is_empty());
    assert_eq!(sel.first(), Some(&1));
}

#[test]
fn selection_get_bounds_checked() {
    let sel = Selection::from_vec(vec![10, 20]);
    assert_eq!(sel.get(0), Some(&10));
    assert_eq!(sel.get(1), Some(&20));
    assert_eq!(sel.get(2), None);
}

#[test]
fn selection_one_exactly_one() {
    let sel = Selection::from_vec(vec![42]);
    assert_eq!(sel.one(), Ok(42));
}

#[test]
fn selection_one_zero_errors() {
    let sel: Selection<i32> = Selection::new();
    let err = sel.one().unwrap_err();
    assert!(matches!(err, ReflectError::NotFound(_)));
}

#[test]
fn selection_one_many_errors() {
    let sel = Selection::from_vec(vec![1, 2]);
    let err = sel.one().unwrap_err();
    assert!(matches!(err, ReflectError::Other(_)));
}

#[test]
fn selection_iter() {
    let sel = Selection::from_vec(vec![1, 2, 3]);
    let collected: Vec<&i32> = sel.iter().collect();
    assert_eq!(collected, vec![&1, &2, &3]);
}

#[test]
fn selection_filter() {
    let sel = Selection::from_vec(vec![1, 2, 3, 4, 5]);
    let evens = sel.filter(|x| *x % 2 == 0);
    assert_eq!(evens.len(), 2);
    assert_eq!(evens.get(0), Some(&2));
}

#[test]
fn selection_map() {
    let sel = Selection::from_vec(vec![1, 2, 3]);
    let doubled: Vec<i32> = sel.map(|x| x * 2);
    assert_eq!(doubled, vec![2, 4, 6]);
}

// ── Id ─────────────────────────────────────────────────────────────────────────

#[test]
fn id_is_stable() {
    let a = Id::new(42);
    let b = Id::new(42);
    assert_eq!(a, b);
    assert_eq!(a.as_u64(), 42);
}

#[test]
fn id_is_distinct() {
    let a = Id::new(1);
    let b = Id::new(2);
    assert_ne!(a, b);
}

// ── ReflectError ───────────────────────────────────────────────────────────────

#[test]
fn reflect_error_display() {
    let e = ReflectError::NotFound("module `foo`".into());
    assert!(e.to_string().contains("foo"));
    let e = ReflectError::NotSettable("name".into());
    assert!(e.to_string().contains("settable"));
}
