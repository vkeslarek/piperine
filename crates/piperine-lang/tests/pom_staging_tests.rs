//! Tests for the POM staging layer (OverrideMap).

use piperine_lang::pom::{OverrideMap, Value};

#[test]
fn override_map_empty() {
    let map = OverrideMap::new();
    assert!(map.is_empty());
}

#[test]
fn override_map_set_and_get() {
    let mut map = OverrideMap::new();
    map.set("top.u1", "r", Value::Real(2.0e3));
    assert!(!map.is_empty());
    let v = map.get("top.u1", "r").expect("override present");
    assert_eq!(v.as_real(), Some(2.0e3));
}

#[test]
fn override_map_miss() {
    let map = OverrideMap::new();
    assert!(map.get("top.u1", "r").is_none());
}

#[test]
fn override_map_clear() {
    let mut map = OverrideMap::new();
    map.set("top", "r", Value::Real(1.0));
    assert!(!map.is_empty());
    map.clear();
    assert!(map.is_empty());
}

#[test]
fn override_map_overwrite() {
    let mut map = OverrideMap::new();
    map.set("top", "r", Value::Real(1.0));
    map.set("top", "r", Value::Real(2.0));
    let v = map.get("top", "r").expect("present");
    assert_eq!(v.as_real(), Some(2.0));
}

#[test]
fn override_map_distinct_paths() {
    let mut map = OverrideMap::new();
    map.set("top.u1", "r", Value::Real(1.0));
    map.set("top.u2", "r", Value::Real(2.0));
    assert_eq!(map.get("top.u1", "r").unwrap().as_real(), Some(1.0));
    assert_eq!(map.get("top.u2", "r").unwrap().as_real(), Some(2.0));
}