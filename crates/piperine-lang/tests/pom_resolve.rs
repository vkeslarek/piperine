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
    // (Whether the fixture builds clean by itself is `fixture_prelude_builds_clean`'s question.)
    let root_file = design.root().file().id();
    assert!(design.diagnostics().iter().all(|d| d.file != root_file), "{:?}", design.diagnostics());
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

#[test]
fn type_paths_resolve_to_declarations() {
    let src = "\
nature Volt { potential: Pot { access: v } }
struct Pair { a: u8 }
enum Mode { A, B }
trait Tr {}
module Sub() {}
module M<N: Tr, const W: u32>(p: Volt, m: Mode, b: Pair, g: N) {
    signal s: [Sub; 2];
}
fn f<T: Tr>(x: T, t: Sub, w: Pair) -> Mode { x }
struct Holder<U: Tr = Pair> { u: U, e: Mode }
";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    let at = |path: &str, nth: usize| -> Resolution { resolutions(&design, path).remove(nth)[0] };
    assert_eq!(at("Volt", 0), decl(&design, "Volt"));
    assert_eq!(at("Mode", 0), decl(&design, "Mode"));
    assert_eq!(at("Pair", 0), decl(&design, "Pair"));
    // A trait as a bound, in a generic parameter list and a default.
    assert_eq!(resolutions(&design, "Tr").len(), 3);
    assert!(resolutions(&design, "Tr").iter().all(|r| r[0] == decl(&design, "Tr")));
    // A module is a type (section 3.4.5), also inside an array type.
    assert_eq!(resolutions(&design, "Sub").len(), 2);
    assert!(resolutions(&design, "Sub").iter().all(|r| r[0] == decl(&design, "Sub")));
    // A generic parameter resolves to its parameter declaration, not to an item.
    let n = design.module("M").unwrap().decl().children().into_iter().find(|c| c.name() == Some("N")).unwrap();
    assert_eq!(resolutions(&design, "N"), [[Resolution::Decl(n.id())]]);
    let t = design.find("f").unwrap().children().into_iter().find(|c| c.name() == Some("T")).unwrap();
    assert_eq!(resolutions(&design, "T"), [[Resolution::Decl(t.id())]]);
    let u = design.find("Holder").unwrap().children().into_iter().find(|c| c.name() == Some("U")).unwrap();
    assert_eq!(resolutions(&design, "U"), [[Resolution::Decl(u.id())]]);
    // Outside the declaration its generics are not names.
    let outside = build("fn g<T>(x: T) {}\nfn h(y: T) {}");
    let errors = with_code(&outside, "E0161");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(primary_text(&outside, errors[0]), "T");
}

#[test]
fn builtin_names_resolve_to_table_entries() {
    let src = "fn f(a: f64, b: u8, c: Option<f64>, d: [u128; 4]) {}\ntrait Tr<T: Repr> {}\n";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    for (name, section) in [("f64", "§1"), ("u8", "§1"), ("Option", "§1"), ("u128", "§1"), ("Repr", "§2")] {
        let all = resolutions(&design, name);
        assert!(!all.is_empty(), "`{name}` is written");
        for r in all {
            let Resolution::Builtin(id) = r[0] else { panic!("`{name}` is a built-in") };
            assert_eq!(id.name(), name);
            assert_eq!(id.info().section, section, "{name}");
        }
    }
    // A declaration shadows the table; so does an import.
    // Core primitives and constants are names of expressions too.
    let exprs = build("fn g() -> f64 { ddt(temperature()) * K_B }");
    assert!(with_code(&exprs, "E0161").is_empty(), "{:?}", exprs.diagnostics());
    for (name, section) in [("ddt", "§14.5"), ("temperature", "§14.4"), ("K_B", "§1")] {
        let Resolution::Builtin(id) = one(&exprs, name)[0] else { panic!("`{name}` is a built-in") };
        assert_eq!((id.name().as_str(), id.info().section), (name, section));
    }
    let shadow = build("struct Option {}\nmod m { pub struct f64 {} }\nuse m::f64;\nfn g(a: Option, b: f64) {}");
    assert!(with_code(&shadow, "E0161").is_empty(), "{:?}", shadow.diagnostics());
    assert_eq!(one(&shadow, "Option"), [decl(&shadow, "Option")]);
    let imported = resolutions(&shadow, "f64");
    assert_eq!(imported.last().unwrap(), &[decl(&shadow, "m::f64")]);
}

#[test]
fn self_type_resolves() {
    let src = "\
struct S { a: u8 }
trait Tr { fn make() -> Self; }
impl S { fn dup(self) -> Self { self } }
impl Tr for S { fn make() -> Self { S { a: 1 } } }
";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    // In arena order: the trait's `Self`, then the inherent impl's, then the trait impl's.
    let selfs = resolutions(&design, "Self");
    assert_eq!(selfs.len(), 3);
    assert_eq!(selfs[0], [decl(&design, "Tr")]);
    assert_eq!(selfs[1], [decl(&design, "S")]);
    assert_eq!(selfs[2], [decl(&design, "S")]);
    // The impl's own header: the trait and the self type.
    assert_eq!(resolutions(&design, "Tr").len(), 1);
    assert_eq!(resolutions(&design, "Tr")[0], [decl(&design, "Tr")]);
    // `Self` outside an impl or a trait names nothing.
    let stray = build("fn f() -> Self {}");
    assert_eq!(with_code(&stray, "E0161").len(), 1);
}

/// The `Binding` or `FnParam` declaration named `name` among the children of `owner`.
fn local(design: &Design, owner: &str, name: &str) -> Resolution {
    let owner = design.find(owner).unwrap_or_else(|| panic!("`{owner}` is declared"));
    let found = owner.children().into_iter().filter(|c| c.name() == Some(name) && matches!(c.kind().label(), "Binding" | "FnParam")).collect::<Vec<_>>();
    assert_eq!(found.len(), 1, "one local `{name}` in `{}`", owner.name().unwrap_or("-"));
    Resolution::Decl(found[0].id())
}

#[test]
fn shadowed_local_resolves_to_previous_binding() {
    let design = build("fn f() -> u32 { let x = 1; let x = x + 1; x }");
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    let bindings: Vec<_> = design.find("f").unwrap().children().into_iter().filter(|c| c.name() == Some("x")).map(|c| c.id()).collect();
    assert_eq!(bindings.len(), 2);
    // In source order: the right-hand `x` is the first local, the tail `x` the second.
    assert_eq!(resolutions(&design, "x"), [[Resolution::Decl(bindings[0])], [Resolution::Decl(bindings[1])]]);
}

#[test]
fn binders_scope_their_names() {
    let src = "\
fn f(p: u32, opt: Option<u32>) -> u32 {
    let g = |c| c + 1;
    let m = match opt { Some(a) => a, None => 0 };
    for i in 0..p { i; }
    if let Some(q) = opt { q; }
    while let Some(w) = opt { w; }
    c; a; i; q; w;
    p
}
";
    let design = build(src);
    // Each binder's name resolves inside its scope and not after it.
    for name in ["c", "a", "i", "q", "w"] {
        assert_eq!(resolutions(&design, name), [[local(&design, "f", name)], [Resolution::Unresolved]], "`{name}`");
    }
    let outside = with_code(&design, "E0161");
    let texts: Vec<_> = outside.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(texts, ["c", "a", "i", "q", "w"]);
    // Function parameters are names of the whole body, and of nothing outside it.
    assert!(resolutions(&design, "p").iter().all(|r| r[0] == local(&design, "f", "p")));
    assert!(resolutions(&design, "opt").iter().all(|r| r[0] == local(&design, "f", "opt")));
    // `Some(a)` names the built-in variant; `None` is a pattern binding, not a reference.
    assert!(resolutions(&design, "Some").iter().all(|r| matches!(r[0], Resolution::Builtin(_))));
    // Items of a function body are names of that body; an inner item sees no local of the outer one.
    let nested = build("fn outer() { let x = 1; fn inner() -> u32 { x } inner(); helper(); fn helper() {} }");
    let errors = with_code(&nested, "E0161");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(primary_text(&nested, errors[0]), "x");
}

#[test]
fn unresolved_path_is_one_e0161() {
    let src = "fn f() -> u32 { missing + 1 }\nstruct S { a: Nope }\nconst K: u32 = gone::far;\n";
    let design = build(src);
    // The build completed: the declarations are all there.
    assert!(design.find("f").is_some() && design.find("S").is_some() && design.find("K").is_some());
    let errors = with_code(&design, "E0161");
    let texts: Vec<_> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(texts, ["missing", "Nope", "gone::far"]);
    assert_eq!(one(&design, "missing"), [Resolution::Unresolved]);
    assert_eq!(one(&design, "gone::far"), [Resolution::Unresolved, Resolution::Unresolved]);
    // The same path read twice is two paths, so two errors.
    let twice = build("fn g() { nope; nope; }");
    assert_eq!(with_code(&twice, "E0161").len(), 2);
}

#[test]
fn diagnostics_are_ordered() {
    let src = "\
fn a() { first_missing; }
module M(x: f64, x: f64) {}
use nowhere::x;
fn ddt() {}
mod nofile;
";
    let prelude = "pub fn pf() { prelude_missing; }\n";
    let design = Design::builder().prelude("std.msl", prelude).source("top.msl", src).build();
    let found: Vec<(u32, usize, String)> = design.diagnostics().iter().map(|d| (d.file.0, d.diagnostic.span().start, d.diagnostic.code.to_string())).collect();
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(found, sorted, "ordered by file, then offset, then code");
    // Nothing is lost by the ordering: the prelude's error, then the root's, one of each code.
    let codes: Vec<&str> = found.iter().map(|(_, _, c)| c.as_str()).collect();
    assert_eq!(codes, ["E0161", "E0161", "E0330", "E0161", "E0457", "E0161"]);
    assert_eq!(found[0].0, 0);
    assert!(found[1..].iter().all(|(f, _, _)| *f == 1));
}
