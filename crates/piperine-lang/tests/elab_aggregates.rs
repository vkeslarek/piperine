//! Scope: struct, enum, tuple, array and `Option` values lowered by `piperine_lang::elab` (feature
//! `composite-values`, CV-09 to CV-13, CV-20 to CV-22; docs/spec Part I §1.2 to §1.6, §2). Each test
//! elaborates a design of `Top` and reads the type arena and the expressions of the IR it gives.
//! Run-time values are proven from source in `piperine-parity`'s `source_aggregates`.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{Expr, Ir, TypeKind, ValueType};
use support::Elab;

/// A design of `Top` with `items` before it, calling each of `calls` from its `analog` block.
fn reaching(items: &str, calls: &[&str]) -> String {
    let lets: Vec<String> = calls.iter().enumerate().map(|(k, call)| format!("let x{k} = {call};")).collect();
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {} }}", lets.join(" "))
}

/// The entry of the arena named `name`.
fn arena_entry<'a>(ir: &'a Ir, name: &str) -> (usize, &'a piperine_ir::TypeData) {
    ir.types().iter().enumerate().find(|(_, data)| data.name == name).unwrap_or_else(|| panic!("no arena type `{name}`: {:?}", ir.types().iter().map(|t| &t.name).collect::<Vec<_>>()))
}

#[test]
fn every_reached_struct_and_enum_is_one_arena_type() {
    // CV-20 AC 4, CV-22, §2 rule 5: each struct and enum the design reaches is one entry of the IR's
    // type arena, named as the source names it, with its fields or its variants, their
    // discriminants, the width of the tag and the traits it derives; a unit enum is an IR enum, not
    // its tag, so a value of it is a variant of its entry and a `match` over it tests the variant.
    let items = "#[derive(Repr, PartialEq)]\npub enum Mode { Off, SoftStart, Regulate, Hiccup }\n\
                 #[derive(PartialEq, PartialOrd)]\nstruct Pair { x: f64, n: u8 }\n\
                 #[derive(Repr)]\nenum Never { A, B }\n\
                 fn active() -> Mode { Mode::Regulate }\n\
                 fn pair_x(p: Pair) -> f64 { 1.0 }\n\
                 fn is_hiccup(m: Mode) -> bool { match m { Mode::Hiccup => true, _ => false } }";
    let ir = Elab::ir(&reaching(items, &["active()", "is_hiccup(Mode::Off)", "{ let f = pair_x; 0.0 }"]), "Top");

    // exactly the types the design reaches: `Never` is declared and never reached
    let names: Vec<&str> = ir.types().iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Mode") && names.contains(&"Pair") && !names.contains(&"Never"), "{names:?}");
    assert_eq!(names.iter().filter(|n| **n == "Mode").count(), 1, "one entry per declaration: {names:?}");

    let (mode, data) = arena_entry(&ir, "Mode");
    let TypeKind::Enum { tag_levels, variants } = &data.kind else { panic!("Mode is an enum") };
    assert_eq!(*tag_levels, 2, "four variants: ceil(log2 4) levels (§2 rule 5)");
    assert_eq!(variants.iter().map(|v| (v.name.as_str(), v.discriminant)).collect::<Vec<_>>(), [("Off", 0), ("SoftStart", 1), ("Regulate", 2), ("Hiccup", 3)]);
    assert!(data.derives.repr && data.derives.partial_eq && data.derives.clone && data.derives.copy, "{:?}", data.derives);

    let (_, pair) = arena_entry(&ir, "Pair");
    let TypeKind::Struct { fields, .. } = &pair.kind else { panic!("Pair is a struct") };
    assert_eq!(fields.clone(), [("x".to_string(), ValueType::F64), ("n".to_string(), ValueType::UInt(8))]);
    assert!(pair.derives.partial_eq && pair.derives.partial_ord && !pair.derives.repr, "{:?}", pair.derives);

    // a value of the enum is the variant of its entry (`Mode::Regulate` is variant 2), and the
    // function's result and parameter have the enum type
    let active = ir.functions().find(|f| f.name() == "active").expect("active");
    assert_eq!(*active.ret(), ValueType::Enum(piperine_ir::TypeId(mode as u32)));
    assert!(ir.expressions().any(|e| matches!(e, Expr::VariantOf { variant: 2, fields, .. } if fields.is_empty())), "Mode::Regulate is VariantOf");
    // a `match` arm over a known enum tests its variant
    assert!(ir.expressions().any(|e| matches!(e, Expr::IsVariant { variant: 3, .. })), "`Mode::Hiccup =>` is IsVariant of variant 3");
    // no unit enum is lowered as its integer tag
    let is_hiccup = ir.functions().find(|f| f.name() == "is_hiccup").expect("is_hiccup");
    assert_eq!(is_hiccup.params()[0].ty, ValueType::Enum(piperine_ir::TypeId(mode as u32)));
}
