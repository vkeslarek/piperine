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

use piperine_lang::pom::{Design, DesignDiagnostic, PathId, Reflect, Resolution};
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

/// The per-segment resolutions of every path whose segments, joined by `::`,
/// read `joined`, in arena order.
fn resolutions(design: &Design, joined: &str) -> Vec<Vec<Resolution>> {
    (0..design.path_count())
        .map(|i| design.path(PathId(i as u32)))
        .filter(|p| p.segments().iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::") == joined)
        .map(|p| p.segments().iter().map(|s| s.res).collect())
        .collect()
}

/// The single path written `joined`: its resolutions.
fn one(design: &Design, joined: &str) -> Vec<Resolution> {
    let mut all = resolutions(design, joined);
    assert_eq!(all.len(), 1, "exactly one path `{joined}`");
    all.remove(0)
}

fn decl(design: &Design, path: &str) -> Resolution {
    Resolution::Decl(design.find(path).unwrap_or_else(|| panic!("`{path}` is declared")).id())
}

const PRELUDE_FIXTURE: &str = include_str!("fixtures/pom/prelude.msl");

#[test]
fn use_and_qualified_paths_resolve() {
    let src = "\
mod a {
    pub fn f() {}
    pub const K: u32 = 1;
    pub struct S {}
    pub mod deep { pub fn g() {} pub fn h() {} }
}
mod b {
    use super::a::f;
    use crate::a::K as Kay;
    use super::a::{S, deep};
    pub use super::a::deep::*;
    use self::f as ff;
    use Kay as K2;
}
mod c {
    use super::b::h;
}
use a::deep::g;
";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    let root = Resolution::Decl(design.root().id());
    let ns_a = decl(&design, "a");
    // Simple, with `super`: every segment resolves to what it names.
    assert_eq!(one(&design, "super::a::f"), [root, ns_a, decl(&design, "a::f")]);
    // Alias, with `crate`.
    assert_eq!(one(&design, "crate::a::K"), [root, ns_a, decl(&design, "a::K")]);
    // Group leaves, and a glob's prefix names the namespace it imports.
    assert_eq!(one(&design, "super::a::S"), [root, ns_a, decl(&design, "a::S")]);
    let deep = [root, ns_a, decl(&design, "a::deep")];
    assert_eq!(resolutions(&design, "super::a::deep"), [deep, deep]);
    // `self::` reaches an import of the same namespace; an alias is a name like any other.
    assert_eq!(one(&design, "self::f"), [decl(&design, "b"), decl(&design, "a::f")]);
    assert_eq!(one(&design, "Kay"), [decl(&design, "a::K")]);
    // A `pub use` glob re-exports: `b::h` reaches `a::deep::h` from `c`.
    assert_eq!(one(&design, "super::b::h"), [root, decl(&design, "b"), decl(&design, "a::deep::h")]);
    // A path relative to the namespace that writes it.
    assert_eq!(one(&design, "a::deep::g"), [ns_a, decl(&design, "a::deep"), decl(&design, "a::deep::g")]);
}

#[test]
fn prelude_source_items_are_visible_everywhere() {
    let prelude = "pub enum Mode { A, B }\npub struct Shared {}\nstruct Hidden {}\n";
    let src = "use Mode::A;\nmod m { use Shared; }\nuse Hidden;\n";
    let design = Design::builder().prelude("std.msl", prelude).source("top.msl", src).build();
    // From the root and from a nested namespace, without any `use` of the prelude.
    let mode = Resolution::Decl(
        (0..design.decl_count()).map(|i| piperine_lang::pom::DeclId(i as u32)).find(|d| design.decl_data(*d).name.as_ref().is_some_and(|n| n.text == "Mode")).unwrap(),
    );
    let a = one(&design, "Mode::A");
    assert_eq!(a[0], mode);
    assert!(matches!(a[1], Resolution::Decl(_)));
    assert!(matches!(one(&design, "Shared")[0], Resolution::Decl(_)));
    // Only the `pub` items are visible: `Hidden` is not found.
    assert_eq!(one(&design, "Hidden"), [Resolution::Unresolved]);
    let missing = with_code(&design, "E0161");
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(primary_text(&design, missing[0]), "Hidden");
    // The same through the spec fixture: its natures are there for the asking.
    let design = Design::builder().prelude("prelude.msl", PRELUDE_FIXTURE).source("top.msl", "use Electrical;\nuse Bit;\n").build();
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    let Resolution::Decl(electrical) = one(&design, "Electrical")[0] else { panic!("Electrical resolves") };
    assert!(design.decl_data(electrical).file != design.root().file().id());
    assert!(design.root().file().id() != piperine_lang::pom::FileId(0));
    assert!(design.files()[0].is_prelude());
}

#[test]
fn private_item_links_and_reports_e0161() {
    let src = "\
mod a {
    fn secret() {}
    pub fn open() {}
    mod inner { use super::secret; }
    use self::secret as s;
}
use a::secret;
use a::open;
use a::nothing;
";
    let design = build(src);
    let errors = with_code(&design, "E0161");
    assert_eq!(errors.len(), 2, "{errors:?}");
    // The private item is linked, and one error says so at the import path.
    assert_eq!(one(&design, "a::secret"), [decl(&design, "a"), decl(&design, "a::secret")]);
    assert!(errors[0].diagnostic.message.contains("private"), "{:?}", errors[0]);
    assert_eq!(primary_text(&design, errors[0]), "a::secret");
    // Absent is another error: nothing to link.
    assert_eq!(one(&design, "a::nothing"), [decl(&design, "a"), Resolution::Unresolved]);
    assert!(!errors[1].diagnostic.message.contains("private"), "{:?}", errors[1]);
    assert_eq!(primary_text(&design, errors[1]), "a::nothing");
    // The public one and the accesses from inside the declaring namespace are clean.
    assert_eq!(one(&design, "a::open"), [decl(&design, "a"), decl(&design, "a::open")]);
    assert_eq!(one(&design, "super::secret"), [decl(&design, "a"), decl(&design, "a::secret")]);
    assert_eq!(one(&design, "self::secret"), [decl(&design, "a"), decl(&design, "a::secret")]);
}

#[test]
fn local_item_wins_over_glob() {
    let src = "\
mod a { pub fn dup() {} pub fn only() {} }
fn dup() {}
use a::*;
mod c { use super::dup as x; use super::only as y; }
";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    // The local `dup` is found, not the glob's; the glob still supplies what is not local.
    let root = Resolution::Decl(design.root().id());
    assert_eq!(one(&design, "super::dup"), [root, decl(&design, "dup")]);
    assert_eq!(one(&design, "super::only"), [root, decl(&design, "a::only")]);
}
