//! Scope: the standard library as shipped, read through the POM of the embedded design (STD-07 to
//! STD-10): which files exist and are embedded, what each namespace declares, that the prelude
//! makes the library visible by its single names, and that the library builds clean (no
//! diagnostic, a sound design, no member slot left unresolved). Every test builds its design from
//! text written in the test or from the embedded library; nothing here restates the library.

use std::collections::BTreeSet;
use std::path::PathBuf;

use piperine_lang::pom::{BUILTINS, BuiltinKind, Design, ExprKind, Reference, Reflect, Resolution};

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

/// The expressions of a declaration, with the design they belong to.
fn calls_named<'d>(design: &'d Design, decl: piperine_lang::pom::Decl<'d>, name: &str) -> Vec<piperine_lang::pom::Expr<'d>> {
    decl.exprs()
        .into_iter()
        .filter(|e| matches!(e.kind(), ExprKind::Call { callee, .. } if design.expr(*callee).text() == name))
        .collect()
}

#[test]
fn noise_table_is_noise_of_table_model() {
    let design = embedded();
    let table = design.std_crate().find("analog::noise_table").expect("std declares noise_table");
    let module = table.as_module().expect("noise_table is a module");
    // The module places `noise` (a core module) and its result is that call.
    let noise = calls_named(&design, table, "noise");
    assert_eq!(noise.len(), 1, "one placement of noise");
    assert_eq!(module.result(), Some(noise[0]), "the result is the noise call");
    let ExprKind::Call { callee, .. } = noise[0].kind() else { unreachable!() };
    assert!(matches!(design.expr(*callee).resolution(), Resolution::Builtin(id) if id.name() == "noise"));
    // Its first argument calls `table_model`, the intrinsic function of std.
    let model = calls_named(&design, table, "table_model");
    assert_eq!(model.len(), 1, "one call of table_model");
    let ExprKind::Call { callee, .. } = model[0].kind() else { unreachable!() };
    let Resolution::Decl(id) = design.expr(*callee).resolution() else { panic!("table_model resolves to a declaration") };
    assert!(design.decl(id).as_function().is_some_and(|f| f.is_intrinsic()));
}

#[test]
fn pure_bodies_name_no_primitive() {
    let design = embedded();
    let mut named = Vec::new();
    for name in ["values::same", "analog::deltalim", "analog::pnjlim", "analog::fetlim", "analog::limvds", "analog::db10", "analog::db20", "analog::normal"] {
        let item = design.std_crate().find(name).unwrap_or_else(|| panic!("std declares {name}"));
        assert!(item.as_function().is_some_and(|f| f.body().is_some()), "{name} has a body");
        for expr in item.exprs() {
            if let Resolution::Builtin(id) = expr.resolution() {
                let info = id.info();
                let primitive = matches!(info.kind, BuiltinKind::Fn | BuiltinKind::Module | BuiltinKind::Method | BuiltinKind::Event | BuiltinKind::Statement | BuiltinKind::Node | BuiltinKind::ImplicitParam);
                if primitive && info.name != "level" {
                    named.push(format!("{name}: {}", info.name));
                }
            }
        }
    }
    assert!(named.is_empty(), "bodies that name a core primitive: {named:?}");
}

#[test]
fn a_private_std_item_is_e0161() {
    let design = Design::builder()
        .source("top.msl", "fn f(s: std::analog::Slewing) {}\nfn g(s: Slewing) {}\n")
        .build();
    assert_eq!(codes(&design), ["E0161", "E0161"], "{:?}", design.diagnostics());
}

/// The modules of `std::analog` whose name starts with `prefix`, with their names.
fn analog_modules<'d>(design: &'d Design, prefix: &str) -> Vec<(String, piperine_lang::pom::Module<'d>)> {
    design
        .std_crate()
        .modules()
        .into_iter()
        .filter_map(|m| {
            let name = m.name()?;
            (name.starts_with(prefix) && m.file().name() == "analog.msl").then(|| (name.to_string(), m))
        })
        .collect()
}

#[test]
fn root_forms_place_their_coefficient_form() {
    let design = embedded();
    for (prefix, placed, forms) in [("zi_", "zi_nd", ["zi_np", "zi_zd", "zi_zp"]), ("laplace_", "laplace_nd", ["laplace_np", "laplace_zd", "laplace_zp"])] {
        let modules = analog_modules(&design, prefix);
        for form in forms {
            let (_, module) = modules.iter().find(|(n, _)| n == form).unwrap_or_else(|| panic!("std::analog declares {form}"));
            let calls = calls_named(&design, module.decl(), placed);
            assert_eq!(calls.len(), 1, "{form} places exactly one {placed}");
            let ExprKind::Call { callee, .. } = calls[0].kind() else { unreachable!() };
            let Resolution::Decl(id) = design.expr(*callee).resolution() else { panic!("{placed} resolves in {form}") };
            assert_eq!(design.decl(id).name(), Some(placed));
            assert!(design.decl(id).krate().is_std());
        }
    }
}

#[test]
fn root_params_carry_a_where() {
    let design = embedded();
    let mut roots = 0;
    for prefix in ["zi_", "laplace_"] {
        for (name, module) in analog_modules(&design, prefix) {
            for param in module.params().into_iter().filter(|p| matches!(p.name(), Some("zeros" | "poles"))) {
                roots += 1;
                assert!(param.where_clause().is_some(), "{name}.{} has a where", param.name().unwrap());
            }
        }
    }
    assert_eq!(roots, 8, "the six root forms declare eight lists of roots");
}

#[test]
fn laplace_nd_has_state_signals_and_an_analog_block() {
    let design = embedded();
    let module = design.std_crate().find("analog::laplace_nd").and_then(|d| d.as_module()).expect("std declares laplace_nd");
    assert!(module.signals().iter().any(|s| s.ty().text().starts_with("[State;")), "a signal that is an array of State");
    assert_eq!(module.analog_blocks().len(), 1, "one analog block");
}
