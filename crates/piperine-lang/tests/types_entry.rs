//! The entry point, reach and determinism of `piperine_lang::types` (TYP-01, TYP-06): what
//! `Types::check` returns for a design with diagnostics, for an unknown top, for the same
//! design twice, and for a `const` read by several expressions. The M1 corpus joins in T9.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclKind, Design, Reflect};
use piperine_lang::types::{Ty, Types};

#[test]
fn a_design_with_diagnostics_gives_them_back() {
    let design = support::build("module Top() { x: f64 = ; }");
    assert!(!design.diagnostics().is_empty(), "the source must hold a diagnostic");
    let err = Types::check(&design, "Top").expect_err("no table for a design with diagnostics");
    assert_eq!(err, design.diagnostics().to_vec());
    assert_eq!(err.len(), support::codes(&design).len());
}

#[test]
fn an_unknown_top_is_one_diagnostic() {
    let design = support::build("module Top() { }");
    assert!(design.diagnostics().is_empty());
    let err = Types::check(&design, "Missing").expect_err("no such module");
    assert_eq!(err.len(), 1);
    assert!(err[0].diagnostic.message.contains("Missing"), "{}", err[0].diagnostic.message);
    assert_eq!(err[0].diagnostic.code, "P0003", "an unknown top has the compiler-owned code P0003");

    // A module of the standard library is not a module of the root crate.
    let design = Design::builder().std_source("lib.msl", "pub mod prelude;\npub module Lib() { }\n").std_source("prelude.msl", "").source("top.msl", "module Top() { }").build();
    let err = Types::check(&design, "Lib").expect_err("a std module is no top");
    assert_eq!(err.len(), 1);
    assert!(err[0].diagnostic.message.contains("Lib"));
    assert_eq!(err[0].diagnostic.code, "P0003");
}

const SHARED: &str = "const K: f64 = 2.0;\nmodule Top() { }\nanalog for Top { let a = K; let b = a; }";

#[test]
fn checking_twice_gives_equal_tables() {
    let design = support::build(SHARED);
    assert!(design.diagnostics().is_empty(), "{:?}", support::codes(&design));
    let first = Types::check(&design, "Top").expect("the source is in the subset");
    let second = Types::check(&design, "Top").expect("the source is in the subset");
    assert_eq!(first, second);
    assert!(!first.reached().is_empty());
}

#[test]
fn a_const_is_typed_once_by_its_declared_type() {
    let design = support::build(SHARED);
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let top = design.module("Top").unwrap().decl().id();
    let konst = design.find("K").expect("the const").id();
    // Reached in first-reach order: the top, then what its body reads, each once.
    assert_eq!(types.reached().iter().filter(|d| **d == konst).count(), 1);
    assert_eq!(types.reached()[0], top);
    // Its initializer is typed by the declared type.
    let DeclKind::Const(data) = &design.decl_data(konst).kind else { panic!("K is a const") };
    assert_eq!(types.ty(data.value.expect("a value")), Some(&Ty::F64));

    // However many readers it has, a bad initializer is reported once.
    let design = support::build("const K: f64 = true;\nmodule Top() { }\nanalog for Top { let a = K; let b = K; }");
    let err = Types::check(&design, "Top").expect_err("a bool initializer of an f64 const");
    assert_eq!(err.len(), 1, "{err:?}");
}

// ---- the M1 corpus (TYP-01, TYP-06) --------------------------------------------------------

use std::collections::BTreeSet;
use std::path::PathBuf;

use piperine_lang::pom::{CallClass, DeclId, ExprId, ExprKind};

/// The six twins of `tests/e2e/twins/` and the module each places.
const TWINS: &[(&str, &str)] = &[("divider", "Divider"), ("diode_iv", "DiodeIv"), ("diode_series", "DiodeSeries"), ("ac_rc", "AcRc"), ("tran_rc", "TranRc"), ("isrc_r", "IsrcR")];

fn twin(file: &str) -> Design {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "e2e", "twins", &format!("{file}.msl")].iter().collect();
    Design::builder().root_file(path).build()
}

/// A declaration and everything under it, with the behavior blocks bound to a module.
fn with_descendants(design: &Design, decl: DeclId, out: &mut BTreeSet<DeclId>) {
    if !out.insert(decl) {
        return;
    }
    let data = design.decl_data(decl);
    for child in &data.children {
        with_descendants(design, *child, out);
    }
    if let DeclKind::Module(m) = &data.kind {
        for behavior in &m.behaviors {
            with_descendants(design, *behavior, out);
        }
    }
}

#[test]
fn the_reachable_set_of_a_twin_is_typed() {
    for (file, top) in TWINS {
        let design = twin(file);
        assert!(design.diagnostics().is_empty(), "{file}: {:?}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        let types = Types::check(&design, top).unwrap_or_else(|d| panic!("{file}: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));

        // Every expression owned by a reached declaration has a type, but the name a probe or a
        // core primitive is called through: that is a name, not a value.
        let mut owned = BTreeSet::new();
        for decl in types.reached() {
            with_descendants(&design, *decl, &mut owned);
        }
        let mut names = BTreeSet::new();
        for i in 0..design.expr_count() {
            if let ExprKind::Call { callee, class, .. } = &design.expr_node(ExprId(i as u32)).kind {
                let by_name = matches!(class, CallClass::Probe | CallClass::Instance { module: None, .. }) || types.callee(ExprId(i as u32)).is_some_and(|c| matches!(c, piperine_lang::types::Callee::Core(_)));
                if by_name {
                    names.insert(*callee);
                }
            }
        }
        let mut typed = 0;
        for i in 0..design.expr_count() {
            let expr = ExprId(i as u32);
            if !owned.contains(&design.expr_node(expr).owner) || names.contains(&expr) {
                continue;
            }
            assert!(types.ty(expr).is_some(), "{file}: no type for the expression at {:?}: {:?}", design.expr_node(expr).span, design.expr_node(expr).kind);
            typed += 1;
        }
        assert!(typed > 0, "{file}: the reachable set holds expressions");
    }

    // The diode's own expressions: the method call is resolved, the limiter closure is typed.
    let design = twin("diode_iv");
    let types = Types::check(&design, "DiodeIv").expect("the diode twin");
    let exp = (0..design.expr_count()).map(|i| ExprId(i as u32)).find(|e| matches!(&design.expr_node(*e).kind, ExprKind::MethodCall { method, .. } if method.text == "exp")).expect("`.exp()` of the diode");
    assert_eq!(types.method(exp), Some(piperine_lang::types::Method::Exp));
    assert_eq!(types.ty(exp), Some(&Ty::F64));

    // For `DiodeIv` the reachable set is the three devices, the two library items they call and
    // the constants those read: nothing generic.
    let names: Vec<&str> = types.reached().iter().filter_map(|d| design.decl_data(*d).name.as_ref().map(|n| n.text.as_str())).collect();
    assert_eq!(names, ["DiodeIv", "Vsource", "Resistor", "Diode", "vt", "pnjlim", "K_B", "Q"]);
}

#[test]
fn the_design_is_unchanged_by_the_check() {
    for (file, top) in TWINS {
        let design = twin(file);
        let before = design.dump();
        Types::check(&design, top).unwrap_or_else(|d| panic!("{file}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
        assert_eq!(before, design.dump(), "{file}: the POM is read, never written");
    }
}

#[test]
fn every_diagnostic_has_a_span_and_a_code() {
    // One source per rule of the feature, covering every code the checker raises.
    let natures = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\nnature T { potential: Tp { access: temp, units: \"K\", abstol: 1m }, flow: P { access: pwr, units: \"W\", abstol: 1n } }\nmodule Resistor(p: E, n: E) { r: f64; }\n";
    let bodies = [
        "module Top() { a: f64 = true; }",
        "module Top() { a: str; }",
        "module Top() { a: f64 = 1.0 where 1.0; }",
        "module Top() { signal s: E = true; }",
        "module Top(a: E) { let r = Resistor { r: 1.0 }(a); }",
        "module Top(a: E, t: T) { let r = Resistor { r: 1.0 }(a, t); }",
        "module Top(a: E, t: T) { }\nanalog for Top { let x = v(a, t); }",
        "module Top(a: E) { }\nanalog for Top { let x = 1.0; x +> 1.0; }",
        "module Top(a: E) { }\nanalog for Top { v(a) +> true; }",
        "fn f(x: f64) -> f64 { ddt(x) }\nmodule Top() { }\nanalog for Top { let y = f(1.0); }",
        "module Top() { }\nanalog for Top { let m = match \"a\" { _ => 1.0 }; }",
    ];
    for body in bodies {
        let src = format!("{natures}{body}");
        let design = support::build(&src);
        assert!(design.diagnostics().is_empty(), "{:?}\n{src}", support::codes(&design));
        let err = Types::check(&design, "Top").expect_err("the source holds a mistake");
        assert!(!err.is_empty());
        for d in &err {
            let code: &str = &d.diagnostic.code;
            assert!(code.len() == 5 && (code.starts_with('E') || code.starts_with('P')) && code[1..].bytes().all(|b| b.is_ascii_digit()), "a code: {code}");
            let span = d.diagnostic.span();
            assert!(span.end > span.start && span.end <= src.len(), "a non-empty span inside the source: {span:?} in\n{src}");
        }
    }
    // The unknown top has a code and no span to point at: the start of the first file.
    let err = Types::check(&support::build("module Top() { }"), "Nope").expect_err("no such top");
    assert_eq!(err[0].diagnostic.code, "P0003");
}
