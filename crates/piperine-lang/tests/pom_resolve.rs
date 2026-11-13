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
