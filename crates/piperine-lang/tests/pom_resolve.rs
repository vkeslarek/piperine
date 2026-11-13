//! Scope: name resolution over the POM (requirement POM-08): the built-in
//! table, namespace and module scopes, imports and the prelude, and the
//! resolution of every path in types, signatures and bodies, with the
//! binding errors `E0161`, `E0330` and `E0457`.

use piperine_lang::pom::{BuiltinKind, builtin};

#[test]
fn builtin_table_has_every_core_primitive() {
    // Primitive types and the sized families.
    for name in ["bool", "Option", "Event", "Analysis", "usize", "isize", "u1", "u8", "u128", "i1", "i64", "i128", "f16", "f32", "f64", "f128"] {
        let id = builtin(name).unwrap_or_else(|| panic!("`{name}` is a built-in"));
        assert_eq!(id.info().kind, BuiltinKind::Type, "{name}");
        assert_eq!(id.name(), name);
    }
    for name in ["u0", "u129", "i0", "i129", "f8", "f64x", "f", "u", "u01", "u+1"] {
        assert!(builtin(name).is_none(), "`{name}` is not a built-in");
    }
    // Prelude constants and the option variants (section 1).
    for name in ["Q", "K_B", "EPS0", "PI", "E", "inf"] {
        let info = builtin(name).expect(name).info();
        assert_eq!((info.kind, info.section), (BuiltinKind::Const, "§1"), "{name}");
    }
    for name in ["Some", "None"] {
        assert_eq!(builtin(name).expect(name).info().kind, BuiltinKind::Variant);
    }
    // Every row of the section 14 table, with its subsection.
    let core = [
        ("potential", BuiltinKind::Fn, "§14.1"),
        ("flow", BuiltinKind::Fn, "§14.1"),
        ("ddx", BuiltinKind::Fn, "§14.2"),
        ("ac_stim", BuiltinKind::Fn, "§14.3"),
        ("temperature", BuiltinKind::Fn, "§14.4"),
        ("simparam", BuiltinKind::Fn, "§14.4"),
        ("analysis", BuiltinKind::Fn, "§14.4"),
        ("now", BuiltinKind::Fn, "§14.4"),
        ("freq", BuiltinKind::Fn, "§14.4"),
        ("ddt", BuiltinKind::Module, "§14.5"),
        ("delay", BuiltinKind::Module, "§14.6"),
        ("limit", BuiltinKind::Module, "§14.7"),
        ("noise", BuiltinKind::Module, "§14.8"),
        ("cross", BuiltinKind::Module, "§14.9"),
        ("at", BuiltinKind::Module, "§14.9"),
        ("settle", BuiltinKind::Event, "§14.9"),
        ("bound_step", BuiltinKind::Statement, "§14.10"),
    ];
    for (name, kind, section) in core {
        let info = builtin(name).unwrap_or_else(|| panic!("`{name}` is a core primitive")).info();
        assert_eq!((info.kind, info.section), (kind, section), "{name}");
    }
    // Names that are standard library (section 14.13), not core.
    for name in ["idt", "transition", "above", "limexp", "discontinuity"] {
        assert!(builtin(name).is_none(), "`{name}` is not core");
    }
}

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{Design, DesignDiagnostic};
use support::{build, codes};

/// The diagnostics of one code.
fn with_code<'a>(design: &'a Design, code: &str) -> Vec<&'a DesignDiagnostic> {
    design.diagnostics().iter().filter(|d| d.diagnostic.code == code).collect()
}

/// The text of the primary span of a diagnostic of the root file.
fn primary_text<'a>(design: &'a Design, d: &DesignDiagnostic) -> &'a str {
    let file = &design.files()[d.file.index()];
    &file.text()[d.diagnostic.span().range()]
}

#[test]
fn core_name_redefined_is_e0457() {
    let src = "fn ddt(x: f64) -> f64 { x }\nmodule temperature() {}\nmod inner { struct bound_step {} }\nfn idt() {}\nfn f64x() {}\nstruct u8 {}\n";
    let design = build(src);
    let found = with_code(&design, "E0457");
    assert_eq!(found.len(), 3, "{found:?}");
    let at: Vec<&str> = found.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["ddt", "temperature", "bound_step"]);
    assert_eq!(found[0].diagnostic.span().start as usize, src.find("ddt").unwrap());
    assert_eq!(found[2].diagnostic.span().start as usize, src.find("bound_step").unwrap());
}

#[test]
fn duplicate_member_name_is_e0330() {
    let src = "module M(a: f64, a: f64) { p: f64 = 1.0; signal p: f64; let q = 1; let q = 2; }\nmodule N(x: f64) { signal y: f64; signal z: f64; }\n";
    let design = build(src);
    let found = with_code(&design, "E0330");
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(codes(&design).iter().filter(|c| *c == "E0330").count(), 3);
    let first = |name: &str, nth: usize| src.match_indices(name).nth(nth).unwrap().0;
    // Second `a`, second `p`, second `q`; each with a label at the first.
    let expected = [(first("a:", 1), first("a:", 0)), (first("p:", 1), first("p:", 0)), (first("q =", 1), first("q =", 0))];
    for (d, (second, label)) in found.iter().zip(expected) {
        assert_eq!(d.diagnostic.span().start as usize, second, "{d:?}");
        assert_eq!(d.diagnostic.secondary.len(), 1, "{d:?}");
        assert_eq!(d.diagnostic.secondary[0].0.start as usize, label, "{d:?}");
    }
}
