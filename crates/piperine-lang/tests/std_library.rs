//! Scope: the standard library as shipped, read through the POM of the embedded design (STD-07 to
//! STD-10): which files exist and are embedded, what each namespace declares, that the prelude
//! makes the library visible by its single names, and that the library builds clean (no
//! diagnostic, a sound design, no member slot left unresolved). Every test builds its design from
//! text written in the test or from the embedded library; nothing here restates the library.

use std::collections::BTreeSet;
use std::path::PathBuf;

use piperine_lang::pom::{BUILTINS, Design, ExprKind, Reference, Reflect, Resolution};

fn embedded() -> Design {
    Design::builder().source("top.msl", "").build()
}

fn codes(design: &Design) -> Vec<String> {
    design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn std_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("std")
}

/// The text of the initializer of the constant `std::<path>`.
fn const_text(design: &Design, path: &str) -> String {
    let decl = design.std_crate().find(path).unwrap_or_else(|| panic!("std declares `{path}`"));
    decl.as_const().and_then(|c| c.value()).unwrap_or_else(|| panic!("`{path}` is a constant with a value")).text().to_string()
}

#[test]
fn every_std_file_is_embedded() {
    let design = embedded();
    let embedded: BTreeSet<String> = design
        .std_crate()
        .files()
        .iter()
        .map(|f| f.name().rsplit('/').next().unwrap().to_string())
        .collect();
    let mut on_disk: Vec<String> = std::fs::read_dir(std_dir())
        .expect("the std directory exists")
        .map(|e| e.expect("directory entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".msl"))
        .collect();
    on_disk.sort();
    let missing: Vec<&String> = on_disk.iter().filter(|n| !embedded.contains(*n)).collect();
    assert!(missing.is_empty(), "std files that the embedded list does not name: {missing:?}");
    assert_eq!(design.std_crate().files()[0].name(), "std/lib.msl", "the root is first");
}

#[test]
fn constants_have_the_values_of_s1() {
    let design = embedded();
    let value = |name: &str| -> f64 { const_text(&design, &format!("constants::{name}")).parse().unwrap_or_else(|e| panic!("{name}: {e}")) };
    assert_eq!(value("Q"), 1.602176634e-19);
    assert_eq!(value("K_B"), 1.380649e-23);
    assert_eq!(value("EPS0"), 8.8541878128e-12);
    assert_eq!(value("PI"), std::f64::consts::PI);
    assert_eq!(value("E"), std::f64::consts::E);
    assert_eq!(const_text(&design, "constants::inf"), "1.0 / 0.0");
    for name in ["Q", "K_B", "EPS0", "PI", "E", "inf"] {
        assert!(design.std_crate().find(&format!("constants::{name}")).unwrap().is_pub(), "{name} is pub");
    }
}

#[test]
fn a_std_item_resolves_by_its_single_name() {
    let design = Design::builder().source("top.msl", "module M(a: Electrical, b: Bit) { }\nconst X: f64 = K_B;\n").build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    for path in ["carriers::Electrical", "carriers::Bit", "constants::K_B"] {
        let item = design.std_crate().find(path).unwrap_or_else(|| panic!("std declares `{path}`"));
        assert!(item.krate().is_std());
        let single = path.rsplit("::").next().unwrap();
        let uses = item.references();
        assert!(uses.iter().any(|r| matches!(r, Reference::Path(p) if p.text() == single)), "`{path}` is referenced by its single name `{single}`: {} uses", uses.len());
    }
}

#[test]
fn no_export_has_a_core_name() {
    let design = embedded();
    let core: BTreeSet<&str> = BUILTINS
        .iter()
        .map(|row| row.name)
        // T20 removes the six `Const` rows; until then these names are rows and exports at once.
        .filter(|n| !["Q", "K_B", "EPS0", "PI", "E", "inf"].contains(n))
        .collect();
    let mut clashes = Vec::new();
    for namespace in design.std_crate().root().submodules() {
        if namespace.name() == Some("prelude") {
            continue;
        }
        for item in namespace.items().into_iter().filter(|i| i.is_pub()) {
            if let Some(name) = item.name() {
                if core.contains(name) {
                    clashes.push(format!("{}::{name}", namespace.name().unwrap_or("?")));
                }
            }
        }
    }
    assert!(clashes.is_empty(), "std exports named like a core name: {clashes:?}");
}

#[test]
fn embedded_std_has_no_diagnostics() {
    let design = embedded();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
}

#[test]
fn std_has_no_unresolved_member() {
    let design = embedded();
    let mut unresolved = Vec::new();
    for decl in design.std_crate().decls() {
        for expr in decl.exprs() {
            if matches!(expr.kind(), ExprKind::Field { .. } | ExprKind::MethodCall { .. }) && expr.resolution() == Resolution::Unresolved {
                unresolved.push(format!("{}: `{}`", decl.qualified_name(), expr.text()));
            }
        }
    }
    assert!(unresolved.is_empty(), "member slots left Unresolved in std: {unresolved:?}");
}

#[test]
fn embedded_std_passes_check() {
    let design = embedded();
    assert_eq!(design.check(), Vec::<String>::new());
}

#[test]
fn a_replaced_std_without_an_item_is_e0161() {
    let design = Design::builder()
        .std_source("lib.msl", "pub mod prelude;\n")
        .std_source("prelude.msl", "")
        .source("top.msl", "module M(a: Electrical) { }\n")
        .build();
    assert_eq!(codes(&design), ["E0161"]);
    assert!(design.module("M").is_some(), "the build completes");
}

#[test]
fn a_std_file_sees_another_namespace_through_the_prelude() {
    let design = Design::builder()
        .std_source("std/lib.msl", "pub mod a;\npub mod b;\npub mod prelude;\n")
        .std_source("std/a.msl", "pub const A: u32 = 1;\n")
        .std_source("std/b.msl", "pub const B: u32 = A;\n")
        .std_source("std/prelude.msl", "pub use crate::a::*;\npub use crate::b::*;\n")
        .source("top.msl", "const C: u32 = B;\n")
        .build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    assert!(design.std_crate().find("a::A").unwrap().references().len() >= 1, "b.msl reads A by its single name");
}

/// The declaration the method call `text` of the model `top.msl` resolves to.
fn called_decl<'d>(design: &'d Design, text: &str) -> piperine_lang::pom::Decl<'d> {
    let call = design
        .root_crate()
        .decls()
        .into_iter()
        .flat_map(|d| d.exprs())
        .find(|e| e.text() == text && matches!(e.kind(), ExprKind::MethodCall { .. }))
        .unwrap_or_else(|| panic!("the model calls `{text}`"));
    match call.resolution() {
        Resolution::Decl(id) => design.decl(id),
        other => panic!("`{text}` resolves to {other:?}, not to a declaration"),
    }
}

#[test]
fn bit_rise_resolves_into_std_values() {
    let design = Design::builder().source("top.msl", "module M(in clk: Bit) { let e = clk.rise(); }\n").build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    let rise = called_decl(&design, "clk.rise()");
    assert_eq!(rise.name(), Some("rise"));
    assert!(rise.krate().is_std());
    assert_eq!(rise.file().name(), "values.msl");
    assert!(rise.declared_in().and_then(|d| d.as_impl()).is_some(), "rise is a function of an `impl`");
}

#[test]
fn known_resolves_into_std_values() {
    let design = Design::builder().source("top.msl", "module M(in d: Bit<u8>) { let k = d.known(); }\n").build();
    assert_eq!(codes(&design), Vec::<String>::new(), "{:?}", design.diagnostics());
    let known = called_decl(&design, "d.known()");
    assert_eq!(known.name(), Some("known"));
    assert!(known.krate().is_std());
    assert_eq!(known.file().name(), "values.msl");
    assert_eq!(known.declared_in().and_then(|d| d.name()), Some("Known"));
}
