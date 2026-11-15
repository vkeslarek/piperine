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

use piperine_lang::pom::{Design, DesignBuilder, DesignDiagnostic, PathId, Reflect, Resolution};
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
    let prelude = "pub enum Mode { A, B }\npub struct Shared {}\nstruct Hidden {}\npub const LIMIT: u32 = 3;\n";
    let src = "use Mode::A;\nmod m { use Shared; mod deep { fn f(x: Shared) -> u32 { let y: Mode = Mode::B; LIMIT } } }\nuse Hidden;\n";
    let design = std_lib(Design::builder(), prelude).source("top.msl", src).build();
    // From the root and from a nested namespace, without any `use` of the prelude.
    let mode = Resolution::Decl(
        (0..design.decl_count()).map(|i| piperine_lang::pom::DeclId(i as u32)).find(|d| design.decl_data(*d).name.as_ref().is_some_and(|n| n.text == "Mode")).unwrap(),
    );
    let a = one(&design, "Mode::A");
    assert_eq!(a[0], mode);
    assert!(matches!(a[1], Resolution::Decl(_)));
    // Names read in a signature, a type and a body of a nested namespace, with no `use`.
    let shared = Resolution::Decl((0..design.decl_count()).map(|i| piperine_lang::pom::DeclId(i as u32)).find(|d| design.decl_data(*d).name.as_ref().is_some_and(|n| n.text == "Shared")).unwrap());
    assert_eq!(resolutions(&design, "Shared"), [[shared], [shared]], "the `use` in `m` and the parameter type in `deep`");
    let limit = &resolutions(&design, "LIMIT")[0];
    assert_eq!(limit.len(), 1);
    let Resolution::Decl(limit) = limit[0] else { panic!("LIMIT resolves") };
    assert_eq!(design.decl_data(limit).kind.label(), "Const");
    assert_eq!(design.decl_data(limit).file, design.files().iter().find(|f| f.name() == "prelude.msl").unwrap().id(), "the prelude's const, not a guess");
    assert_eq!(resolutions(&design, "Mode"), [[mode]], "the type annotation");
    assert_eq!(resolutions(&design, "Mode::B")[0][0], mode);
    // Only the `pub` items are visible: `Hidden` is not found.
    assert_eq!(one(&design, "Hidden"), [Resolution::Unresolved]);
    let missing = with_code(&design, "E0161");
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(primary_text(&design, missing[0]), "Hidden");
    // The same through the spec fixture: its natures are there for the asking.
    let design = std_lib(Design::builder(), PRELUDE_FIXTURE).source("top.msl", "use Electrical;\nuse Bit;\n").build();
    // (Whether the fixture builds clean by itself is `fixture_prelude_builds_clean`'s question.)
    let root_file = design.root().file().id();
    assert!(design.diagnostics().iter().all(|d| d.file != root_file), "{:?}", design.diagnostics());
    let Resolution::Decl(electrical) = one(&design, "Electrical")[0] else { panic!("Electrical resolves") };
    assert!(design.decl_data(electrical).file != design.root().file().id());
    assert!(design.root().file().id() == piperine_lang::pom::FileId(0));
    assert!(design.files().last().unwrap().krate().is_std());
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
    let design = std_lib(Design::builder(), prelude).source("top.msl", src).build();
    let found: Vec<(u32, usize, String)> = design.diagnostics().iter().map(|d| (d.file.0, d.diagnostic.span().start, d.diagnostic.code.to_string())).collect();
    let mut sorted = found.clone();
    sorted.sort();
    assert_eq!(found, sorted, "ordered by file, then offset, then code");
    // Nothing is lost by the ordering: the root's errors, then the library's, one of each code.
    let codes: Vec<&str> = found.iter().map(|(_, _, c)| c.as_str()).collect();
    assert_eq!(codes, ["E0161", "E0330", "E0161", "E0457", "E0161", "E0161"]);
    assert!(found[..5].iter().all(|(f, _, _)| *f == 0));
    assert!(found[5].0 > 0, "the library's file is numbered after the project's");
}

#[test]
fn block_reads_resolve_to_module_members() {
    let src = "\
module M<G: u32>(pin: f64) {
    rp: f64 = 1.0;
    signal sg: f64;
    let lt = 2.0;
    for k in 0..2 { signal gs: f64; }
}
analog<G> for M<G> {
    pin; rp; sg; lt; gs; G;
}
";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    let m = design.module("M").unwrap();
    let child = |name: &str| {
        let found = m.decl().children().into_iter().find(|c| c.name() == Some(name)).or_else(|| {
            let generation = m.decl().children().into_iter().find(|c| c.kind().label() == "Generation")?;
            generation.children().into_iter().find(|c| c.name() == Some(name))
        });
        Resolution::Decl(found.unwrap_or_else(|| panic!("member `{name}`")).id())
    };
    // The block reads a port, a param, a signal, a body `let`, a signal in a generation and a generic.
    for (read, declared) in [("pin", "pin"), ("rp", "rp"), ("sg", "sg"), ("gs", "gs")] {
        assert_eq!(resolutions(&design, read), [[child(declared)]], "`{read}`");
    }
    // `G` is written twice: in the target `M<G>` and in the body.
    assert_eq!(resolutions(&design, "G"), [[child("G")], [child("G")]]);
    assert_eq!(resolutions(&design, "lt").len(), 1);
    let Resolution::Decl(lt) = resolutions(&design, "lt")[0][0] else { panic!("`lt` resolves") };
    assert_eq!(design.decl_data(lt).kind.label(), "Binding");
    // The `G` of `analog<G> for M<G>` is the module's generic by position.
    let g = child("G");
    let target = design.root().children().into_iter().find_map(|c| c.as_behavior()).unwrap().target();
    let piperine_lang::pom::GenericArgsNode::Angle { args, .. } = target.segments()[0].args.as_ref().unwrap() else { panic!("angle args") };
    let piperine_lang::pom::GenericArgNode::Type(arg) = &args[0] else { panic!("a type argument") };
    let piperine_lang::pom::TypeKind::Path(arg_path) = &design.ty(*arg).kind() else { panic!("a path") };
    assert_eq!(design.path(*arg_path).segments()[0].res, g);
    // Names of one module are no names of another.
    let other = build("module A() { signal a: f64; }\nmodule B() { signal b: f64; }\nanalog for B { a; }");
    let errors = with_code(&other, "E0161");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(primary_text(&other, errors[0]), "a");
}

#[test]
fn body_names_are_order_independent() {
    let src = "\
module M(pin: f64) {
    let a = b + 1.0;
    let b = 2.0;
    early: f64 = late;
    late: f64 = pin;
    signal s: f64 = later_sig;
    signal later_sig: f64;
    for k in 0..2 { signal g: f64 = tail; }
    tail: f64 = 1.0;
}
";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    let m = design.module("M").unwrap();
    let named = |name: &str| m.decl().children().into_iter().find(|c| c.name() == Some(name)).map(|c| Resolution::Decl(c.id())).unwrap();
    // Read before the declaration: the parameter, the signal and the let binding.
    assert_eq!(resolutions(&design, "late"), [[named("late")]]);
    assert_eq!(resolutions(&design, "later_sig"), [[named("later_sig")]]);
    assert_eq!(resolutions(&design, "tail"), [[named("tail")]]);
    let let_b = design.decl_data(m.decl().children().into_iter().find(|c| c.kind().label() == "Let" && c.children().iter().any(|x| x.name() == Some("b"))).unwrap().id()).children[0];
    assert_eq!(resolutions(&design, "b"), [[Resolution::Decl(let_b)]]);
    assert_eq!(resolutions(&design, "pin"), [[named("pin")]]);
    // A generation's loop variable is a name of the generation only.
    let loopvar = build("module L() { for k in 0..2 { signal g: f64 = k; } early: f64 = k; }");
    let errors = with_code(&loopvar, "E0161");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(primary_text(&loopvar, errors[0]), "k");
}

#[test]
fn associated_paths_resolve_to_variant_const_fn() {
    let src = "\
enum Mode { A, B(u8) }
trait Tr { const X: u32; fn run() -> u32; }
struct S {}
impl S { const K: u32 = 1; fn make() -> u32 { Self::K } }
impl Tr for S { const X: u32 = 3; fn run() -> u32 { 4 } }
impl Mode { fn first() -> Mode { Mode::A } }
fn f() -> u32 { Mode::B(1); S::K + S::make() + S::X + Tr::run() }
";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    let inner = |outer: &str, name: &str| -> Resolution {
        let owner = design.find(outer).unwrap_or_else(|| panic!("`{outer}`"));
        let impls: Vec<_> = (0..design.decl_count())
            .map(|i| piperine_lang::pom::DeclId(i as u32))
            .filter(|d| design.decl_data(*d).kind.label() == "Impl" || *d == owner.id())
            .collect();
        let found = impls.iter().flat_map(|i| design.decl_data(*i).children.clone()).find(|c| design.decl_data(*c).name.as_ref().is_some_and(|n| n.text == name) && !matches!(design.decl_data(*c).kind.label(), "Binding"));
        Resolution::Decl(found.unwrap_or_else(|| panic!("`{name}` in an impl of `{outer}`")))
    };
    // A variant of an enum, with the enum as the first segment.
    let mode = decl(&design, "Mode");
    let variant = |n: &str| Resolution::Decl(design.decl_data(design.find("Mode").unwrap().id()).children.iter().copied().find(|c| design.decl_data(*c).name.as_ref().is_some_and(|x| x.text == n)).unwrap());
    assert_eq!(resolutions(&design, "Mode::A"), [[mode, variant("A")]]);
    assert_eq!(resolutions(&design, "Mode::B"), [[mode, variant("B")]]);
    // An associated const or fn of an impl of the type, inherent or of a trait.
    let s = decl(&design, "S");
    assert_eq!(resolutions(&design, "S::K"), [[s, inner("S", "K")]]);
    assert_eq!(resolutions(&design, "S::make"), [[s, inner("S", "make")]]);
    // The const of `impl Tr for S`, not the trait's declaration.
    assert_eq!(resolutions(&design, "S::X"), [[s, inner("S", "X")]]);
    assert_ne!(inner("S", "X"), inner("Tr", "X"));
    // `Self::K` in an impl: Self is the struct, K its const.
    assert_eq!(resolutions(&design, "Self::K"), [[s, inner("S", "K")]]);
    // `Trait::item` names the trait's own item.
    let tr = design.find("Tr").unwrap();
    let trait_run = tr.children().into_iter().find(|c| c.name() == Some("run")).unwrap();
    assert_eq!(resolutions(&design, "Tr::run"), [[decl(&design, "Tr"), Resolution::Decl(trait_run.id())]]);
    // An item the type does not have is one error at the whole path.
    let missing = build("struct S {}\nenum E { A }\nfn f() { S::nope; E::B; }");
    let errors = with_code(&missing, "E0161");
    let at: Vec<_> = errors.iter().map(|d| primary_text(&missing, d)).collect();
    assert_eq!(at, ["S::nope", "E::B"]);
    assert_eq!(one(&missing, "S::nope"), [decl(&missing, "S"), Resolution::Unresolved]);
}

#[test]
fn fixture_prelude_builds_clean() {
    // As a prelude beside an empty root, and as the root itself: no diagnostics either way.
    let as_prelude = std_lib(Design::builder(), PRELUDE_FIXTURE).source("top.msl", "").build();
    assert!(as_prelude.diagnostics().is_empty(), "{:?}", as_prelude.diagnostics());
    let as_root = Design::builder().source("prelude.msl", PRELUDE_FIXTURE).build();
    assert!(as_root.diagnostics().is_empty(), "{:?}", as_root.diagnostics());
    assert!(as_root.check().is_empty());
    // It declares what the spec says the prelude does.
    for name in ["Bit", "Wire", "Electrical", "Thermal", "VSignal"] {
        assert!(as_root.nature(name).is_some(), "nature `{name}`");
    }
    for name in ["idt", "above", "transition"] {
        assert!(as_root.module(name).is_some(), "module `{name}`");
    }
    // The module `idt` is named by its own block, and the block's generic is the module's.
    let idt = as_root.module("idt").unwrap();
    assert_eq!(idt.behaviors().len(), 1);
    assert_eq!(as_root.module("transition").unwrap().digital_blocks().len(), 1);
}

/// POM-08 AC 7 and AC 9: a `use` in a block is an item of that block, visible
/// in the whole block (Rust scoping), and an unresolvable one is one E0161.
#[test]
fn use_in_a_block_resolves_with_block_scope() {
    let src = "\
mod a { pub fn x() {} pub const K: u32 = 1; pub mod deep { pub fn g() {} } pub enum E { P } }
fn f() { early(); use a::x as early; use a::K as Kay; use a::deep::*; use a::E::*; Kay; g(); P; }
fn k() { { use a::x as y; } y(); }
fn h() { use a::nothing; }
";
    let design = build(src);
    let (ns_a, x) = (decl(&design, "a"), decl(&design, "a::x"));
    // Used before the `use` item in the same block: the item is block-wide.
    assert_eq!(one(&design, "early"), [x]);
    assert_eq!(resolutions(&design, "a::x"), [[ns_a, x], [ns_a, x]]);
    assert_eq!(one(&design, "Kay"), [decl(&design, "a::K")]);
    assert_eq!(one(&design, "g"), [decl(&design, "a::deep::g")]);
    let e = design.decl_data(design.find("a::E").unwrap().id()).children[0];
    assert_eq!(one(&design, "P"), [Resolution::Decl(e)]);
    // The name does not leave the block that imports it.
    assert_eq!(one(&design, "y"), [Resolution::Unresolved]);
    assert_eq!(one(&design, "a::nothing"), [ns_a, Resolution::Unresolved]);
    let errors = with_code(&design, "E0161");
    let at: Vec<&str> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["y", "a::nothing"], "{errors:?}");
}

/// A nature's `access: v` introduces the accessor name: its path names the
/// quantity that introduces it, not nothing, and no `E0161` is reported.
#[test]
fn access_name_resolves_to_its_quantity() {
    let src = "nature Volt { potential: Voltage { access: v, units: \"V\" } }\nmodule M(p: Volt) { signal s: f64; s = v(p); }\n";
    let design = build(src);
    assert!(design.diagnostics().iter().all(|d| d.diagnostic.code != "E0161" || primary_text(&design, d) != "v"), "{:?}", design.diagnostics());
    let nature = design.nature("Volt").unwrap();
    let quantity = nature.quantities()[0].decl().id();
    let access: Vec<_> = (0..design.path_count()).map(|i| design.path(PathId(i as u32))).filter(|p| p.text() == "v").collect();
    assert!(!access.is_empty());
    assert!(access.iter().all(|p| p.resolution() == Resolution::Decl(quantity)), "{access:?}");
}

#[test]
fn raw_identifier_names_drop_the_prefix() {
    // POL-10 AC 4: `signal r#out: Bit;` is named `out`; the read `r#out`
    // resolves to it; `r#out` and `out`-less lookups agree; to_ast restores
    // the prefix.
    let src = "nature Bit { value: bool }\nmodule M() {\n    signal r#out: Bit;\n    let x = r#out;\n}\n";
    let design = build(src);
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", codes(&design));
    let m = design.module("M").expect("module M");
    let sig = m.signal("out").expect("the signal is named `out`");
    assert_eq!(sig.name(), Some("out"));
    let read = one(&design, "out");
    assert!(matches!(read[0], Resolution::Decl(d) if d == sig.decl().id()), "{read:?}");
    let parsed = piperine_lang::msl::parser::parse(src).file;
    assert_eq!(design.files()[0].to_ast(), parsed);
    assert_eq!(format!("{:?}", design.files()[0].to_ast()), format!("{parsed:?}"));
}

#[test]
fn module_with_a_bad_port_still_resolves() {
    // POL-16 AC 4: the module with a parse error in one port is still found,
    // a placement of it is an instance, and neither E0161 nor E0401 appears.
    let src = "\
module M(in a: , b: bool) { signal s: bool; }
module Top() {
    signal x: bool;
    signal y: bool;
    let m = M(x, y);
}
";
    let design = build(src);
    let module = design.module("M").expect("`M` is in the design");
    assert_eq!(module.name(), Some("M"));
    let top = design.module("Top").unwrap();
    let instances = top.instances();
    assert_eq!(instances.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(instances[0].module(), Some(module));
    assert!(with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
    assert!(with_code(&design, "E0401").is_empty(), "{:?}", design.diagnostics());
}

// ---- core names (POL-03) -----------------------------------------------------------------------

/// The built-in kind a path of one segment resolved to, for every path spelled `name`.
fn builtin_kinds(design: &Design, name: &str) -> Vec<Option<BuiltinKind>> {
    resolutions(design, name)
        .into_iter()
        .map(|segments| match segments.as_slice() {
            [Resolution::Builtin(b)] => Some(b.info().kind),
            _ => None,
        })
        .collect()
}

#[test]
fn str_is_a_builtin_type() {
    let design = build("const S: &'static str = \"x\";\n");
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    assert_eq!(builtin_kinds(&design, "str"), [Some(BuiltinKind::Type)]);
}

#[test]
fn gnd_is_a_builtin_node() {
    let src = "\
module M(p: f64) { let a = gnd; }
analog for M { let b = gnd; }
digital for M { on at(1.0) { let c = gnd; } }
";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    assert_eq!(builtin_kinds(&design, "gnd"), [Some(BuiltinKind::Node); 3]);
}

#[test]
fn m_and_dtemp_resolve_inside_a_module() {
    let src = "\
module M(p: f64) { let a = m; let b = dtemp; }
analog for M { let c = m; let d = dtemp; }
";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    assert_eq!(builtin_kinds(&design, "m"), [Some(BuiltinKind::ImplicitParam); 2]);
    assert_eq!(builtin_kinds(&design, "dtemp"), [Some(BuiltinKind::ImplicitParam); 2]);
}

#[test]
fn m_and_dtemp_are_unknown_outside_a_module() {
    let src = "fn f() -> f64 { m }\nconst C: f64 = dtemp;\n";
    let design = build(src);
    let found = with_code(&design, "E0161");
    let at: Vec<&str> = found.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["m", "dtemp"], "{:?}", design.diagnostics());
    assert_eq!(design.diagnostics().len(), 2);
    assert_eq!(one(&design, "m"), [Resolution::Unresolved]);
    assert_eq!(one(&design, "dtemp"), [Resolution::Unresolved]);
}

#[test]
fn a_local_shadows_an_implicit_param() {
    let src = "\
module M() {}
analog for M { let m = 2.0; let k = m; let f = |dtemp: f64| dtemp; }
";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    assert!(matches!(one(&design, "m")[0], Resolution::Decl(_)), "{:?}", one(&design, "m"));
    assert!(matches!(one(&design, "dtemp")[0], Resolution::Decl(_)), "{:?}", one(&design, "dtemp"));
}

#[test]
fn core_names_cannot_be_redeclared() {
    let src = "fn gnd() {}\ntrait Nature {}\nstruct str {}\n";
    let design = build(src);
    let found = with_code(&design, "E0457");
    let at: Vec<&str> = found.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["gnd", "Nature", "str"], "{:?}", design.diagnostics());
}

#[test]
fn fixture_prelude_declares_no_core_name() {
    let design = Design::builder().source("prelude.msl", PRELUDE_FIXTURE).build();
    for name in ["Nature", "gnd", "m", "dtemp", "str"] {
        assert!(design.find(name).is_none(), "the fixture prelude declares `{name}`");
    }
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
}

// ---- the catalogue of Appendix H (POL-02) ------------------------------------------------------

/// The first cell of every row of the table under the heading starting `heading` in Appendix H,
/// without its backticks and generic arguments (`UInt<N>` gives `UInt`).
fn appendix_h_names(heading: &str) -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/appendix_h_library.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let start = text.find(heading).unwrap_or_else(|| panic!("heading `{heading}`"));
    let body = &text[start + heading.len()..];
    let body = &body[..body.find("\n## ").unwrap_or(body.len())];
    body.lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| {
            let cell = l.trim_start_matches('|').split('|').next().unwrap().trim().trim_matches('`');
            cell.split('<').next().unwrap().to_string()
        })
        .collect()
}

#[test]
fn catalogue_traits_and_types_resolve() {
    let traits = appendix_h_names("## H.8");
    let types = appendix_h_names("## H.9");
    assert!(traits.len() >= 9 && types.len() >= 6, "{traits:?} {types:?}");
    // A derive name resolves like a bound: to the trait row it names (D2).
    let derived = build("#[derive(Default)]\nstruct S { a: f64 }\n#[derive(PartialOrd, Debug, Hash, Clone, Repr)]\nstruct U;\ntrait Mine {}\n#[derive(Mine)]\nstruct V;\n");
    assert!(derived.diagnostics().is_empty(), "{:?}", derived.diagnostics());
    for (item, want) in [("S", vec!["Default"]), ("U", vec!["PartialOrd", "Debug", "Hash", "Clone", "Repr"]), ("V", vec!["Mine"])] {
        let decl = derived.find(item).unwrap();
        let attr = decl.attr("derive").unwrap();
        let got: Vec<_> = attr.paths().iter().map(|p| (p.segments()[0].name.text.clone(), p.segments()[0].res)).collect();
        assert_eq!(got.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), want);
        for (name, res) in got {
            match res {
                Resolution::Builtin(b) => assert_eq!(b.info().kind, BuiltinKind::Trait, "{name}"),
                Resolution::Decl(d) => assert_eq!(d, derived.find("Mine").unwrap().id(), "{name}"),
                other => panic!("derive `{name}` is {other:?}"),
            }
        }
    }
    // Anything else is E0161 at the name: an unknown word, a type, a value.
    for name in ["Bogus", "Iterator", "Option", "PI"] {
        let src = format!("#[derive(Clone, {name})]\nstruct S;\n");
        let bad = build(&src);
        let at = src.find(name).unwrap();
        let found: Vec<_> = bad.diagnostics().iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.span().start)).collect();
        assert_eq!(found, vec![("E0161".to_string(), at)], "derive({name})");
    }
    let mut src = String::new();
    for (i, t) in traits.iter().enumerate() {
        src.push_str(&format!("fn t{i}<T: {t}>(a: T) {{}}\n"));
    }
    for (i, t) in types.iter().enumerate() {
        let written = if matches!(t.as_str(), "UInt" | "Int" | "Float" | "Option") { format!("{t}<f64>") } else { t.clone() };
        src.push_str(&format!("fn y{i}(a: {written}) {{}}\n"));
    }
    let design = build(&src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    for t in &traits {
        let kinds = builtin_kinds(&design, t);
        assert!(!kinds.is_empty() && kinds.iter().all(|k| *k == Some(BuiltinKind::Trait)), "trait `{t}`: {kinds:?}");
    }
    for t in &types {
        let kinds = builtin_kinds(&design, t);
        assert!(!kinds.is_empty() && kinds.iter().all(|k| *k == Some(BuiltinKind::Type)), "type `{t}`: {kinds:?}");
    }
}

#[test]
fn builtin_table_matches_appendix_h() {
    use std::collections::BTreeSet;
    let rows = |kind: BuiltinKind| -> BTreeSet<String> {
        piperine_lang::pom::BUILTINS.iter().filter(|b| b.kind == kind).map(|b| b.name.to_string()).collect()
    };
    let own = |names: &[&str]| -> BTreeSet<String> { names.iter().map(|s| s.to_string()).collect() };
    // Traits: Appendix H's list, plus the language's own `Repr`, `Nature` and `Fn`.
    let mut traits: BTreeSet<String> = appendix_h_names("## H.8").into_iter().collect();
    traits.extend(own(&["Repr", "Nature", "Fn"]));
    assert_eq!(rows(BuiltinKind::Trait), traits);
    // Types: every H.9 type has a row; the others are the sized families, `usize`/`isize`, `Event` and `Analysis`.
    let types = rows(BuiltinKind::Type);
    let catalogue: BTreeSet<String> = appendix_h_names("## H.9").into_iter().collect();
    assert!(catalogue.is_subset(&types), "missing: {:?}", catalogue.difference(&types).collect::<Vec<_>>());
    let extra = own(&["uN", "iN", "fN", "usize", "isize", "Event", "Analysis"]);
    let unexplained: Vec<_> = types.iter().filter(|t| !catalogue.contains(*t) && !extra.contains(*t)).collect();
    assert!(unexplained.is_empty(), "{unexplained:?}");
    // Every row of the catalogue is under the `App. H` label, apart from the §1 types.
    for b in piperine_lang::pom::BUILTINS.iter().filter(|b| b.kind == BuiltinKind::Trait && catalogue_trait(b.name)) {
        assert_eq!(b.section, "App. H", "{}", b.name);
    }
}

fn catalogue_trait(name: &str) -> bool {
    appendix_h_names("## H.8").iter().any(|t| t == name)
}

// ---- `use a::{self}` and leaf spans (POL-17) ---------------------------------------------------

#[test]
fn use_self_imports_the_namespace() {
    let src = "mod a { pub const K: f64 = 1.0; }\nuse a::{self, K};\nconst Z: f64 = a::K + K;\n";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let k = decl(&design, "a::K");
    let ns_a = decl(&design, "a");
    // The `a::K` of the import and the one of the expression, and the bare `K`.
    for path in resolutions(&design, "a::K") {
        assert_eq!(path, [ns_a, k]);
    }
    assert_eq!(one(&design, "K"), [k]);
}

#[test]
fn use_self_with_alias_and_nesting() {
    let src = "\
mod a { pub const K: f64 = 1.0; pub mod n { pub const J: f64 = 2.0; } }
use a::{self as aa, n::{self, J as JJ}};
const Z: f64 = aa::K + n::J + JJ;
";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    assert_eq!(one(&design, "aa::K"), [decl(&design, "a"), decl(&design, "a::K")]);
    assert_eq!(one(&design, "JJ"), [decl(&design, "a::n::J")]);
    for path in resolutions(&design, "n::J") {
        assert_eq!(path, [decl(&design, "a::n"), decl(&design, "a::n::J")]);
    }
}

#[test]
fn use_leaf_text_is_its_own_segments() {
    let src = "mod a { pub mod c { pub fn d() {} } pub fn b() {} pub mod e {} }\nuse a::{b, c::d, e::*, self as s};\nuse a::b as bb;\n";
    let design = build(src);
    let uses: Vec<_> = design.root().children().into_iter().filter_map(|c| c.as_use()).collect();
    let leaves: Vec<_> = uses.iter().flat_map(|u| u.imports()).map(|i| (i.path.text(), &src[i.path.span().range()])).collect();
    assert_eq!(leaves, [("b", "b"), ("c::d", "c::d"), ("e", "e"), ("self", "self"), ("a::b", "a::b")].to_vec());
    for (text, _) in &leaves {
        assert!(!text.contains(['{', '}', ',']), "{text}");
    }
}

// ---- members of `Repr` and of built-in bounds (POL-17) -----------------------------------------

#[test]
fn repr_members_resolve_through_a_bound() {
    let src = "fn cast<A: Repr, B: Repr>(a: A) -> B where A::WIDTH == B::WIDTH { B::decode(a.encode()).unwrap() }\n";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let member = |joined: &str, name: &str| {
        let res = one(&design, joined);
        assert!(matches!(res[0], Resolution::Decl(_)), "{joined}: {res:?}");
        let Resolution::Builtin(b) = res[1] else { panic!("{joined}: {res:?}") };
        assert_eq!((b.info().kind, b.info().name), (BuiltinKind::Member, name), "{joined}");
    };
    member("B::decode", "Repr::decode");
    for width in resolutions(&design, "A::WIDTH").into_iter().chain(resolutions(&design, "B::WIDTH")) {
        let Resolution::Builtin(b) = width[1] else { panic!("{width:?}") };
        assert_eq!(b.info().name, "Repr::WIDTH");
    }
    // `encode` is a method on a value: it needs the receiver's type.
    assert!(builtin("Repr::WIDTH").is_none(), "a member row is not a name");
}

#[test]
fn unknown_repr_member_is_e0161() {
    let src = "fn f<A: Repr>(a: A) { A::NOPE; }\n";
    let design = build(src);
    let found = with_code(&design, "E0161");
    assert_eq!(found.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(design.diagnostics().len(), 1);
    assert_eq!(primary_text(&design, found[0]), "A::NOPE");
    assert_eq!(one(&design, "A::NOPE")[1], Resolution::Unresolved);
}

#[test]
fn member_through_a_builtin_bound_is_deferred() {
    use piperine_lang::pom::DeferReason;
    let src = "trait Tr { fn run() {} }\nfn f<T: PartialOrd>(a: T) { T::foo; }\nfn g<T: Repr + Clone>(a: T) { T::foo; T::WIDTH; }\nfn h<T: Tr>() { T::run(); }\n";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let foo = resolutions(&design, "T::foo");
    assert_eq!(foo.len(), 2);
    for res in &foo {
        assert_eq!(res[1], Resolution::Deferred(DeferReason::BuiltinTraitMember));
    }
    // A member `Repr` has still resolves, beside another built-in bound.
    assert!(matches!(one(&design, "T::WIDTH")[1], Resolution::Builtin(_)));
    // A user trait bound resolves to the trait's own item.
    assert_eq!(one(&design, "T::run")[1], decl(&design, "Tr::run"));
}

// ---- E0162: one name per table per namespace (POL-04) ------------------------------------------

#[test]
fn duplicate_item_is_e0162_and_both_are_kept() {
    let src = "module M() {}\nmodule M() {}\nstruct N;\nconst N: f64 = 1.0;\nmod a {}\nmod a {}\nstruct T { x: f64 }\nenum T { A }\n";
    let design = build(src);
    let found = with_code(&design, "E0162");
    let at: Vec<(&str, usize)> = found.iter().map(|d| (primary_text(&design, d), d.diagnostic.span().start as usize)).collect();
    let nth = |needle: &str, n: usize| src.match_indices(needle).nth(n).unwrap().0;
    assert_eq!(at, [("M", nth("M()", 1)), ("N", nth("N:", 0)), ("a", nth("a {}", 1)), ("T", nth("enum T", 0) + 5)], "{:?}", design.diagnostics());
    assert_eq!(design.diagnostics().len(), 4, "{:?}", design.diagnostics());
    // Every declaration stays, and the name is the first one.
    assert_eq!(design.root().children().len(), 8);
    let first = design.root().children()[0].id();
    assert_eq!(design.find("M").map(|d| d.id()), Some(first));
}

#[test]
fn import_colliding_with_an_item_is_e0162() {
    let src = "\
mod a { pub const K: f64 = 1.0; }
mod b { pub const K: f64 = 2.0; }
const K: f64 = 0.0;
use a::K;
mod m { use super::a::K; use super::b::K; }
mod n { pub const J: f64 = 1.0; use super::a::K as J; }
";
    let design = build(src);
    let found = with_code(&design, "E0162");
    let at: Vec<&str> = found.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["a::K", "super::b::K", "super::a::K"], "{:?}", design.diagnostics());
    assert_eq!(design.diagnostics().len(), 3, "{:?}", design.diagnostics());
}

#[test]
fn ambiguous_glob_is_e0162_only_when_used() {
    let src = "\
mod a { pub const K: f64 = 1.0; }
mod b { pub const K: f64 = 2.0; }
mod idle { use super::a::*; use super::b::*; }
mod busy { use super::a::*; use super::b::*; const Z: f64 = K; }
";
    let design = build(src);
    let found = with_code(&design, "E0162");
    assert_eq!(found.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(primary_text(&design, found[0]), "K");
    assert_eq!(found[0].diagnostic.span().start as usize, src.rfind("K;").unwrap());
    assert_eq!(design.diagnostics().len(), 1, "no E0161 beside it: {:?}", design.diagnostics());
    assert_eq!(one(&design, "K"), [Resolution::Unresolved]);
}

#[test]
fn two_globs_of_one_declaration_are_clean() {
    let src = "\
mod a { pub const K: f64 = 1.0; }
mod u { pub use super::a::*; }
mod v { use super::a::*; use super::u::*; const Z: f64 = K; }
";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    assert_eq!(one(&design, "K"), [decl(&design, "a::K")]);
}

#[test]
fn two_functions_of_one_name_stay_e0143() {
    let design = build("fn f() {}\nfn f() {}\n");
    assert_eq!(support::codes(&design), ["E0143"]);
}

#[test]
fn one_name_in_two_tables_is_clean() {
    let design = build("struct S { a: f64 }\nfn S() {}\n");
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
}

// ---- identifier patterns that name a constant (POL-06) -----------------------------------------

use piperine_lang::pom::{PatId, PatKind, Reference};

/// Every plain identifier pattern named `name`: its id, resolution and binding declaration.
fn ident_patterns(design: &Design, name: &str) -> Vec<(PatId, Resolution, piperine_lang::pom::DeclId)> {
    (0..design.pat_count())
        .filter_map(|i| {
            let id = PatId(i as u32);
            match design.pat(id).kind() {
                PatKind::Ident { name: n, res, binding, .. } if n.text == name => Some((id, *res, *binding)),
                _ => None,
            }
        })
        .collect()
}

fn is_binding(design: &Design, binding: piperine_lang::pom::DeclId) -> bool {
    design.decl(binding).as_binding().expect("a Binding declaration").is_binding()
}

#[test]
fn identifier_pattern_naming_none_is_a_path() {
    let src = "fn f(o: Option<u32>) -> Option<u32> { match o { None => None, Some(x) => Some(x) } }\n";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let found = ident_patterns(&design, "None");
    assert_eq!(found.len(), 1);
    let (_, res, binding) = found[0];
    assert_eq!(res, Resolution::Builtin(builtin("None").unwrap()));
    assert!(!is_binding(&design, binding));
    // The arm's scope holds no name `None`: the body's `None` is the built-in too.
    assert_eq!(one(&design, "None"), [Resolution::Builtin(builtin("None").unwrap())]);
    // `x` stays a binding.
    assert!(is_binding(&design, ident_patterns(&design, "x")[0].2));
}

#[test]
fn identifier_pattern_naming_a_const_variant_or_unit_struct_is_a_path() {
    let src = "\
const ZERO: u32 = 0;
enum E { A, B(u32) }
struct U;
use E::*;
fn f(n: u32, e: E, u: U, a: Analysis) -> u32 {
    let p = match n { ZERO => 1, _ => 2 };
    let q = match e { A => 1, B(_) => 2 };
    let r = match u { U => 1 };
    let s = match a { Dc => 1, _ => 2 };
    p + q + r + s
}
";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    for (name, expected) in [
        ("ZERO", decl(&design, "ZERO")),
        ("A", decl(&design, "E::A")),
        ("U", decl(&design, "U")),
        ("Dc", Resolution::Builtin(builtin("Dc").unwrap())),
    ] {
        let found = ident_patterns(&design, name);
        assert_eq!(found.len(), 1, "{name}");
        let (id, res, binding) = found[0];
        assert_eq!(res, expected, "{name}");
        assert!(!is_binding(&design, binding), "{name}");
        if let Resolution::Decl(d) = res {
            let referenced = design.decl(d).references().iter().any(|r| matches!(r, Reference::Pattern(p) if p.id() == id));
            assert!(referenced, "{name} is listed in references()");
        }
    }
}

#[test]
fn other_identifier_patterns_stay_bindings() {
    let src = "\
enum E { A, B(u32) }
use E::*;
fn g() {}
fn f(o: Option<u32>, e: E) -> u32 {
    let local = 1;
    let a = match o { Some(y) => y, mut m => 0 };
    let b = match e { ref r => 0 };
    let c = match local { whole @ 1 => whole, nothing => nothing, g => 0, B => 0, local => local };
    a + b + c
}
";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    for name in ["y", "m", "r", "whole", "nothing", "g", "B", "local"] {
        for (_, res, binding) in ident_patterns(&design, name) {
            assert_eq!(res, Resolution::Unresolved, "{name}");
            assert!(is_binding(&design, binding), "{name}");
        }
    }
    // A later use resolves to the binding the pattern made.
    for name in ["whole", "nothing", "local"] {
        let (_, _, binding) = *ident_patterns(&design, name).last().unwrap();
        let uses = resolutions(&design, name);
        assert_eq!(uses.last().unwrap(), &[Resolution::Decl(binding)], "{name}");
    }
}

// ---- a misplaced signal declares nothing (POL-14) ----------------------------------------------

#[test]
fn misplaced_signal_declares_nothing() {
    let src = "\
module M() { signal ok: f64; }
analog for M { signal s: f64; let x = s; }
digital for M { on at(1.0) { signal t: f64; let y = t; } }
fn f() { signal u: f64; let z = u; }
";
    let design = build(src);
    // The parser reports each misplaced `signal`; each later use is an unresolved name.
    assert_eq!(with_code(&design, "E0404").len(), 2, "{:?}", design.diagnostics());
    assert_eq!(with_code(&design, "E0121").len(), 1, "{:?}", design.diagnostics());
    let missing: Vec<&str> = with_code(&design, "E0161").iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(missing, ["s", "t", "u"], "{:?}", design.diagnostics());
    for name in ["s", "t", "u"] {
        assert_eq!(one(&design, name), [Resolution::Unresolved], "{name}");
    }
    // No module lists them; the declarations stay in the tree with no module.
    let signals: Vec<_> = design.module("M").unwrap().signals().iter().map(|s| s.name().unwrap().to_string()).collect();
    assert_eq!(signals, ["ok"]);
    for name in ["s", "t", "u"] {
        let kept = design.decls().into_iter().filter_map(|d| d.as_signal()).find(|s| s.name() == Some(name)).unwrap_or_else(|| panic!("signal `{name}` stays"));
        assert!(kept.module().is_none(), "{name}");
    }
}

#[test]
fn duplicate_items_in_a_block_are_e0162() {
    // D5: a block holds items as a namespace does; one name twice in one table is E0162 at the
    // later name (two `fn` of one name included: the parser's E0143 covers namespaces only).
    let src = "fn f() { fn g() {} fn g() {} struct S; struct S; const A: u8 = 1; const A: u8 = 2; }\n";
    let design = build(src);
    let found = with_code(&design, "E0162");
    let at: Vec<(&str, usize)> = found.iter().map(|d| (primary_text(&design, d), d.diagnostic.span().start as usize)).collect();
    let nth = |needle: &str, n: usize| src.match_indices(needle).nth(n).unwrap().0;
    assert_eq!(at, [("g", nth("g()", 1)), ("S", nth("S;", 1)), ("A", nth("A:", 1))], "{:?}", design.diagnostics());
    assert_eq!(design.diagnostics().len(), 3, "{:?}", design.diagnostics());
    // Different tables, different blocks and shadowing an outer item are clean.
    for clean in [
        "fn f() { struct S { a: u8 } fn S() {} }",
        "fn f() { fn g() {} } fn h() { fn g() {} }",
        "fn g() {} fn f() { fn g() {} }",
        "fn f() { fn g() {} { fn g() {} } }",
    ] {
        let design = build(clean);
        assert!(design.diagnostics().is_empty(), "{clean}: {:?}", design.diagnostics());
    }
}

/// `items` as the `pub` content of `std::prelude` (what `.prelude(name, items)` used to register).
/// It is a second std file so the fixture's own `//!` comments stay legal.
fn std_lib(builder: DesignBuilder, items: &str) -> DesignBuilder {
    builder.std_source("lib.msl", "pub mod prelude;\n").std_source("prelude.msl", items)
}
