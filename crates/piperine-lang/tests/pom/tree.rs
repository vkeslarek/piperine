//! Scope: the declaration tree mirrors the source (requirement POM-02 and the
//! per-kind requirements): which declaration sits under which, in source
//! order, with its kind, name, span and visibility.

use crate::support;
use crate::lowering;

use piperine_lang::msl::parser::SourceParser;
use piperine_lang::pom::{DeclKind, NamespaceOrigin, Reflect};
use lowering::lowering_codes;
use support::build;

#[test]
fn root_children_mirror_items_in_source_order() {
    let text = "\
nature Voltage { potential: Voltage { access: v } }
pub struct S { a: u32 }
enum E { A, B }
trait Tr { }
impl S { }
const N: u32 = 8;
fn f() {}
use a::b;
mod m { }
pub module R(p: Electrical, n: Electrical) {}
analog for R { }
";
    let ast = SourceParser::parse(text).file;
    assert!(SourceParser::parse(text).diagnostics.is_empty(), "fixture must parse cleanly");
    let design = build(text);
    assert!(lowering_codes(&design).is_empty());
    let root = design.root();
    let children = root.children();
    assert_eq!(children.len(), ast.items.len());
    let expected = [
        ("Nature", Some("Voltage")),
        ("Struct", Some("S")),
        ("Enum", Some("E")),
        ("Trait", Some("Tr")),
        ("Impl", None),
        ("Const", Some("N")),
        ("Fn", Some("f")),
        ("Use", None),
        ("Namespace", Some("m")),
        ("Module", Some("R")),
        ("Behavior", None),
    ];
    for (i, child) in children.iter().enumerate() {
        assert_eq!((child.kind().label(), child.name()), expected[i], "item {i}");
        assert_eq!(child.span(), ast.items[i].span.0, "span of item {i}");
        assert_eq!(child.is_pub(), ast.items[i].public, "visibility of item {i}");
        assert_eq!(child.declared_in(), Some(root));
    }
    assert_eq!(root.declared_in(), None);
    assert!(children[1].is_pub() && !children[0].is_pub());
}

#[test]
fn inline_mod_is_a_namespace() {
    let design = build("mod a { module M() {} }");
    let a = design.find("a").expect("namespace a");
    assert!(matches!(a.kind(), DeclKind::Namespace(n) if n.origin == NamespaceOrigin::Inline));
    let m = design.find("a::M").expect("module M in a");
    assert_eq!(m.kind().label(), "Module");
    assert_eq!(m.declared_in(), Some(a));
    assert_eq!(a.children(), vec![m]);
    assert!(design.find("M").is_none(), "M is inside a, not at the root");
}

#[test]
fn const_type_and_value() {
    use piperine_lang::pom::{ExprKind, LitKind, TypeKind};
    let design = build("const N: u32 = 8;\nfn f() {}");
    assert!(design.diagnostics().is_empty());
    let n = design.find("N").and_then(|d| d.as_const()).expect("constant N");
    assert_eq!(n.name(), Some("N"));
    assert_eq!(n.ty().text(), "u32");
    assert!(matches!(n.ty().kind(), TypeKind::Path(_)));
    let value = n.value().expect("a value expression");
    assert_eq!(value.text(), "8");
    assert!(matches!(value.kind(), ExprKind::Lit(l) if l.kind == LitKind::Int("8".into())));
    assert_eq!(value.enclosing(), n.decl());
    assert!(design.find("f").unwrap().as_const().is_none());
}

#[test]
fn fn_abi_params_ret_body() {
    use piperine_lang::pom::{Reflect, TypeKind};
    let text = "\
#[derivative(f_dx)] extern \"C\" fn f(x: f64) -> f64;
const fn g<T: Repr, const N: u32 = 4>(self, a: T, b: f64) -> T where N >= 1 { a }
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());

    let f = design.find("f").and_then(|d| d.as_function()).expect("function f");
    assert_eq!(f.abi(), Some("C"));
    assert!(!f.is_const());
    let params = f.params();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name(), Some("x"));
    assert_eq!(params[0].ty().map(|t| t.text()), Some("f64"));
    assert!(!params[0].is_self());
    assert_eq!(f.ret().map(|t| t.text()), Some("f64"));
    assert!(f.body().is_none());
    assert!(f.where_clause().is_none());
    assert!(f.generics().is_empty());

    let g = design.find("g").and_then(|d| d.as_function()).expect("function g");
    assert!(g.is_const());
    assert_eq!(g.abi(), None);
    let params = g.params();
    assert_eq!(params.iter().map(|p| p.name()).collect::<Vec<_>>(), [Some("self"), Some("a"), Some("b")]);
    assert!(params[0].is_self() && params[0].ty().is_none());
    assert!(matches!(params[1].ty().map(|t| t.kind()), Some(TypeKind::Path(_))));
    assert_eq!(g.where_clause().map(|e| e.text()), Some("N >= 1"));
    let body = g.body().expect("a body");
    assert_eq!(body.stmts().len(), 1);

    let generics = g.generics();
    assert_eq!(generics.len(), 2);
    assert!(!generics[0].is_const());
    assert_eq!(generics[0].bounds().iter().map(|b| b.text()).collect::<Vec<_>>(), ["Repr"]);
    assert!(generics[1].is_const());
    assert_eq!(generics[1].const_type().map(|t| t.text()), Some("u32"));
    assert_eq!(generics[1].default_value().map(|e| e.text()), Some("4"));
    // Children of the function: generics, then params, in source order.
    let labels: Vec<_> = g.decl().children().iter().map(|c| c.kind().label()).collect();
    assert_eq!(labels, ["GenericParam", "GenericParam", "FnParam", "FnParam", "FnParam"]);
}

#[test]
fn item_in_fn_body_is_declared_in_the_fn() {
    let design = build("fn f() { const K: u32 = 1; { fn inner() {} } }\nfn g() {}");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let f = design.find("f").expect("function f");
    let k = design.find("f::K").expect("K is found through f");
    assert_eq!(k.declared_in(), Some(f));
    let inner = design.find("f::inner").expect("an item in a nested block belongs to the function");
    assert_eq!(inner.declared_in(), Some(f));
    // Not leaked to the namespace, nor to another function.
    assert!(design.find("K").is_none());
    assert!(design.find("g::K").is_none());
    assert_eq!(design.root().children().len(), 2);
    assert!(design.check().is_empty());
}

#[test]
fn doc_comments_reach_docs() {
    let text = "\
//! File line 1
//! File line 2
/// Module doc
/** block doc */
mod a { }
/// fn doc
fn f() {}
/// const doc
const N: u32 = 1;
fn plain() {}
struct S {
    /// field doc
    a: u32,
    b: u32,
}
enum E {
    /// variant doc
    A,
    B,
}
/// module doc
module Md(
    /// port doc
    a: Electrical,
    b: Electrical,
) {
    /// param doc
    r: f64 = 1.0;
    /// signal doc
    signal s: Bit;
    t: f64 = 2.0;
}
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let texts = |d: piperine_lang::pom::Decl<'_>| d.docs().iter().map(|x| x.text.clone()).collect::<Vec<_>>();
    assert_eq!(texts(design.root()), [" File line 1", " File line 2"]);
    let a = design.find("a").unwrap();
    assert_eq!(texts(a), [" Module doc", " block doc "]);
    assert_eq!(a.docs().iter().map(|d| d.block).collect::<Vec<_>>(), [false, true]);
    assert_eq!(texts(design.find("f").unwrap()), [" fn doc"]);
    assert_eq!(texts(design.find("N").unwrap()), [" const doc"]);
    assert!(design.find("plain").unwrap().docs().is_empty());
    // The doc span is the comment's own text range.
    let doc = &design.find("f").unwrap().docs()[0];
    assert_eq!(&text[doc.span.range()], "/// fn doc");
    // Fields and variants carry their own docs.
    let s = design.find("S").and_then(|d| d.as_struct()).unwrap();
    assert_eq!(texts(s.fields()[0].decl()), [" field doc"]);
    assert!(s.fields()[1].docs().is_empty());
    let e = design.find("E").and_then(|d| d.as_enum()).unwrap();
    assert_eq!(texts(e.variants()[0].decl()), [" variant doc"]);
    assert!(e.variants()[1].docs().is_empty());
    // Modules, ports, params and signals carry their own docs.
    let md = design.module("Md").unwrap();
    assert_eq!(texts(md.decl()), [" module doc"]);
    assert_eq!(texts(md.ports()[0].decl()), [" port doc"]);
    assert!(md.ports()[1].docs().is_empty());
    assert_eq!(texts(md.param("r").unwrap().decl()), [" param doc"]);
    assert!(md.param("t").unwrap().docs().is_empty());
    assert_eq!(texts(md.signal("s").unwrap().decl()), [" signal doc"]);
}

#[test]
fn attribute_arguments_are_exposed() {
    use piperine_lang::pom::LitKind;
    let text = "\
#[link(\"bsim4.osdi\", model = \"bsim4\")]
#[derive(Repr, Clone)]
#[doc = \"hello\"]
const N: u32 = 1;
const M: u32 = 2;
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let n = design.find("N").unwrap();
    assert_eq!(n.attrs().len(), 3);

    let link = n.attr("link").expect("attribute link");
    assert_eq!(link.name(), "link");
    let positional = link.positional();
    assert_eq!(positional.len(), 1);
    assert_eq!(positional[0].kind, LitKind::Str { text: "bsim4.osdi".into(), raw_hashes: None });
    assert_eq!(link.named("model").map(|l| &l.kind), Some(&LitKind::Str { text: "bsim4".into(), raw_hashes: None }));
    assert!(link.named("other").is_none());
    assert_eq!(link.text(), "#[link(\"bsim4.osdi\", model = \"bsim4\")]");

    let derive = n.attr("derive").unwrap();
    assert_eq!(derive.paths().iter().map(|p| p.text()).collect::<Vec<_>>(), ["Repr", "Clone"]);
    let doc = n.attr("doc").unwrap();
    assert_eq!(doc.value().map(|e| e.text()), Some("\"hello\""));

    // The path is stored as written, never resolved.
    assert_eq!(link.path().segments()[0].res, piperine_lang::pom::Resolution::Unresolved);
    assert!(n.attr("missing").is_none());
    assert!(design.find("M").unwrap().attrs().is_empty());
}

#[test]
fn struct_fields_and_enum_variants() {
    use piperine_lang::pom::{Dir, FieldsForm};
    let text = "\
struct S<T> { pub in a: u32, out valid: Bit, c: T }
struct P(u8, u16);
struct U;
enum E { A, Data(u8) = 3, Named { x: u8, y: u16 } }
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());

    let s = design.find("S").and_then(|d| d.as_struct()).expect("struct S");
    assert_eq!(s.form(), FieldsForm::Named);
    assert_eq!(s.generics().len(), 1);
    assert_eq!(s.bundle(), Some(false), "a classified non-bundle struct carries Some(false)");
    let fields = s.fields();
    assert_eq!(fields.iter().map(|f| f.name()).collect::<Vec<_>>(), [Some("a"), Some("valid"), Some("c")]);
    assert_eq!(fields.iter().map(|f| f.dir()).collect::<Vec<_>>(), [Some(Dir::In), Some(Dir::Out), None]);
    assert_eq!(fields.iter().map(|f| f.is_pub()).collect::<Vec<_>>(), [true, false, false]);
    assert_eq!(fields.iter().map(|f| f.ty().text()).collect::<Vec<_>>(), ["u32", "Bit", "T"]);
    assert_eq!(fields[1].declared_in(), Some(s.decl()));
    assert_eq!(fields[1].text(), "out valid: Bit");

    let p = design.find("P").and_then(|d| d.as_struct()).expect("tuple struct P");
    assert_eq!(p.form(), FieldsForm::Tuple);
    let pf = p.fields();
    assert_eq!(pf.iter().map(|f| f.name()).collect::<Vec<_>>(), [None, None]);
    assert_eq!(pf.iter().map(|f| f.ty().text()).collect::<Vec<_>>(), ["u8", "u16"]);

    let u = design.find("U").and_then(|d| d.as_struct()).expect("unit struct U");
    assert_eq!(u.form(), FieldsForm::Unit);
    assert!(u.fields().is_empty());

    let e = design.find("E").and_then(|d| d.as_enum()).expect("enum E");
    let variants = e.variants();
    assert_eq!(variants.iter().map(|v| v.name()).collect::<Vec<_>>(), [Some("A"), Some("Data"), Some("Named")]);
    assert_eq!(variants.iter().map(|v| v.form()).collect::<Vec<_>>(), [FieldsForm::Unit, FieldsForm::Tuple, FieldsForm::Named]);
    assert!(variants[0].discriminant().is_none());
    assert_eq!(variants[1].discriminant().map(|d| d.text()), Some("3"));
    assert_eq!(variants[1].fields().iter().map(|f| f.ty().text()).collect::<Vec<_>>(), ["u8"]);
    assert_eq!(variants[2].fields().iter().map(|f| f.name()).collect::<Vec<_>>(), [Some("x"), Some("y")]);
    assert_eq!(variants[2].declared_in(), Some(e.decl()));
    assert_eq!(variants[2].fields()[0].declared_in(), Some(variants[2].decl()));
    assert!(design.find("S").unwrap().as_enum().is_none());
    assert!(design.check().is_empty());
}

#[test]
fn impl_and_trait_parts() {
    let text = "\
struct Stream<T> { v: T }
impl<T> Stream<T> { const K: u32 = 1; fn get(self) -> T { self.v } }
impl Tr for S { fn f(self); }
trait Tr: Base + Other { const C: u32; fn f(self); }
";
    let design = build(text);
    // `impl Tr for S { fn f(self); }`: a bodiless function outside a trait is E0125 (D11).
    assert_eq!(lowering_codes(&design), ["E0125"], "{:?}", design.diagnostics());
    let items = design.root().children();
    assert_eq!(items.iter().map(|d| d.kind().label()).collect::<Vec<_>>(), ["Struct", "Impl", "Impl", "Trait"]);

    let inherent = items[1].as_impl().expect("impl block");
    assert_eq!(inherent.name(), None);
    assert_eq!(inherent.generics().len(), 1);
    assert_eq!(inherent.self_ty().text(), "Stream<T>");
    assert!(inherent.trait_path().is_none());
    let inner = inherent.items();
    assert_eq!(inner.iter().map(|d| (d.kind().label(), d.name())).collect::<Vec<_>>(), [("Const", Some("K")), ("Fn", Some("get"))]);
    assert_eq!(inner[0].declared_in(), Some(inherent.decl()));
    assert!(inner[1].as_function().unwrap().params()[0].is_self());

    let of_trait = items[2].as_impl().expect("trait impl");
    assert_eq!(of_trait.trait_path().map(|p| p.text()), Some("Tr"));
    assert_eq!(of_trait.self_ty().text(), "S");
    assert_eq!(of_trait.items().len(), 1);
    assert!(of_trait.items()[0].as_function().unwrap().body().is_none());

    let tr = items[3].as_trait().expect("trait Tr");
    assert_eq!(tr.name(), Some("Tr"));
    assert_eq!(tr.supertraits().iter().map(|p| p.text()).collect::<Vec<_>>(), ["Base", "Other"]);
    let ti = tr.items();
    assert_eq!(ti.iter().map(|d| (d.kind().label(), d.name())).collect::<Vec<_>>(), [("Const", Some("C")), ("Fn", Some("f"))]);
    assert!(ti[0].as_const().unwrap().value().is_none());
    assert!(items[0].as_impl().is_none() && items[0].as_trait().is_none());
    assert!(design.check().is_empty());
}

#[test]
fn use_tree_flattens_to_imports() {
    let text = "use a::{b, c as d, e::*};\nuse x::y;\nuse z::*;\n";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let uses = design.root().children();
    assert_eq!(uses.len(), 3);
    assert!(uses.iter().all(|u| u.name().is_none() && u.kind().label() == "Use"));

    let segs = |p: piperine_lang::pom::Path<'_>| p.segments().iter().map(|s| s.name.text.clone()).collect::<Vec<_>>();
    let imports = uses[0].as_use().expect("use item").imports();
    assert_eq!(imports.len(), 3);
    assert_eq!(segs(imports[0].path), ["a", "b"]);
    assert_eq!((imports[0].alias, imports[0].glob), (None, false));
    assert_eq!(segs(imports[1].path), ["a", "c"]);
    assert_eq!((imports[1].alias, imports[1].glob), (Some("d"), false));
    assert_eq!(segs(imports[2].path), ["a", "e"]);
    assert_eq!((imports[2].alias, imports[2].glob), (None, true));

    let single = uses[1].as_use().unwrap().imports();
    assert_eq!(single.len(), 1);
    assert_eq!(segs(single[0].path), ["x", "y"]);
    assert_eq!(single[0].path.text(), "x::y");
    let glob = uses[2].as_use().unwrap().imports();
    assert_eq!((glob.len(), glob[0].glob, segs(glob[0].path)), (1, true, vec!["z".to_string()]));

    // The tree as written is kept: one group under the prefix `a`.
    let tree = &uses[0].as_use().unwrap().tree();
    assert_eq!(tree.path.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(), ["a"]);
    assert!(matches!(&tree.kind, piperine_lang::pom::UseTreeKind::Group(g) if g.len() == 3));
    assert!(design.find("x").is_none());
}

#[test]
fn nature_quantities_and_kind() {
    use piperine_lang::msl::ast::Quantity as Role;
    use piperine_lang::pom::NatureKind;
    let text = "\
nature Electrical {
    potential: Voltage { access: v, units: \"V\", abstol: 1u },
    flow: Current { access: i }
}
nature Signalf { potential: Voltage { access: v } }
nature Disc { value: f64 }
nature Res { value: f64 }
nature Twice { potential: A { access: a }, potential: B { access: b } }
nature Mixed { potential: A { access: a }, value: f64 }
nature Empty { flow: F { access: f } }
";
    let design = build(text);
    // The three invalid natures below are `E0201` (D11), one each.
    assert_eq!(lowering_codes(&design), ["E0201"; 3], "{:?}", design.diagnostics());
    let kind = |n: &str| design.nature(n).unwrap_or_else(|| panic!("nature {n}")).kind();
    assert_eq!(kind("Electrical"), NatureKind::Conservative);
    assert_eq!(kind("Signalf"), NatureKind::SignalFlow);
    assert_eq!(kind("Disc"), NatureKind::Discrete);
    assert_eq!(kind("Res"), NatureKind::Discrete);
    assert_eq!(kind("Twice"), NatureKind::Invalid);
    assert_eq!(kind("Mixed"), NatureKind::Invalid);
    assert_eq!(kind("Empty"), NatureKind::Invalid);

    let e = design.nature("Electrical").unwrap();
    let qs = e.quantities();
    assert_eq!(qs.len(), 2);
    assert_eq!((qs[0].role(), qs[0].name()), (Role::Potential, Some("Voltage")));
    assert_eq!((qs[1].role(), qs[1].name()), (Role::Flow, Some("Current")));
    assert_eq!(qs[0].access().map(|x| x.text()), Some("v"));
    assert_eq!(qs[0].units().map(|x| x.text()), Some("\"V\""));
    assert_eq!(qs[0].abstol().map(|x| x.text()), Some("1u"));
    assert_eq!(qs[1].access().map(|x| x.text()), Some("i"));
    assert!(qs[1].units().is_none() && qs[1].abstol().is_none());
    assert_eq!(e.potential(), Some(qs[0]));
    assert_eq!(e.flow(), Some(qs[1]));
    assert_eq!(qs[0].declared_in(), Some(e.decl()));
    // The quantities are the nature's children, nothing is hoisted.
    assert_eq!(e.decl().children().len(), 2);
    // A repeated field stays in the list as written.
    assert_eq!(design.nature("Twice").unwrap().quantities().len(), 2);
    let res = design.nature("Res").unwrap();
    assert_eq!(res.value_type().map(|t| t.text()), Some("f64"));
}

#[test]
fn ports_in_order_with_direction_type_default() {
    use piperine_lang::pom::Dir;
    let design = build("module M(in a: Bit, out b: Bit<u8>, c: Electrical, d: f64 = 2.0) {}");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("M").expect("module M");
    let ports = m.ports();
    assert_eq!(ports.iter().map(|p| p.name()).collect::<Vec<_>>(), [Some("a"), Some("b"), Some("c"), Some("d")]);
    assert_eq!(ports.iter().map(|p| p.index()).collect::<Vec<_>>(), [0, 1, 2, 3]);
    assert_eq!(ports.iter().map(|p| p.direction()).collect::<Vec<_>>(), [Some(Dir::In), Some(Dir::Out), None, None]);
    assert_eq!(ports.iter().map(|p| p.ty().text()).collect::<Vec<_>>(), ["Bit", "Bit<u8>", "Electrical", "f64"]);
    assert_eq!(ports.iter().map(|p| p.default().map(|e| e.text())).collect::<Vec<_>>(), [None, None, None, Some("2.0")]);
    assert_eq!(m.port("c"), Some(ports[2]));
    assert!(m.port("zz").is_none());
    assert_eq!(ports[0].declared_in(), Some(m.decl()));
}

#[test]
fn extern_module_abi() {
    let text = "\
#[link(\"bsim4.osdi\", model = \"bsim4\")]
extern \"osdi\" module Bsim4(d: Electrical) { l: f64 = 1u; }
module Plain() {}
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let b = design.module("Bsim4").unwrap();
    assert!(b.is_extern());
    assert_eq!(b.abi(), Some("osdi"));
    assert_eq!(b.attr("link").map(|a| a.named("model").is_some()), Some(true));
    let p = design.module("Plain").unwrap();
    assert!(!p.is_extern());
    assert_eq!(p.abi(), None);
}

#[test]
fn module_result_and_result_type() {
    let design = build("module Mx(a: f64) -> f64 { a * 2.0 }\nmodule Nx(a: f64) {}");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("Mx").unwrap();
    assert_eq!(m.result_type().map(|t| t.text()), Some("f64"));
    assert_eq!(m.result().map(|e| e.text()), Some("a * 2.0"));
    let n = design.module("Nx").unwrap();
    assert!(n.result_type().is_none() && n.result().is_none());
}

#[test]
fn module_generics_and_where() {
    let design = build("module G<T: Repr, const A: u32>(p: T) where A >= 1 {}");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let g = design.module("G").unwrap();
    let generics = g.generics();
    assert_eq!(generics.iter().map(|x| x.name()).collect::<Vec<_>>(), [Some("T"), Some("A")]);
    assert!(!generics[0].is_const());
    assert_eq!(generics[0].bounds().iter().map(|b| b.text()).collect::<Vec<_>>(), ["Repr"]);
    assert!(generics[1].is_const());
    assert_eq!(generics[1].const_type().map(|t| t.text()), Some("u32"));
    assert_eq!(g.where_clause().map(|e| e.text()), Some("A >= 1"));
    // Children: generics first, then ports.
    let labels: Vec<_> = g.decl().children().iter().map(|c| c.kind().label()).collect();
    assert_eq!(labels, ["GenericParam", "GenericParam", "Port"]);
}

#[test]
fn signal_is_declared_in_its_module_only() {
    let design = build("module abc() { signal w: Bit; }\nmodule other() { signal z: Bit; }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let abc = design.module("abc").unwrap();
    let w = abc.signal("w").expect("signal w");
    assert_eq!(w.declared_in(), Some(abc.decl()));
    assert_eq!(w.module(), Some(abc));
    assert_eq!(abc.signals().iter().map(|s| s.name()).collect::<Vec<_>>(), [Some("w")]);
    let other = design.module("other").unwrap();
    assert!(other.signal("w").is_none());
    assert_eq!(other.signals().iter().map(|s| s.name()).collect::<Vec<_>>(), [Some("z")]);
    assert!(design.root().children().iter().all(|c| c.kind().label() == "Module"));
}

#[test]
fn params_type_default_where_required() {
    let design = build("module P() { r: f64 where r > 0.0; tc1: f64 = 0.0; o: Option<f64>; }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("P").unwrap();
    let params = m.params();
    assert_eq!(params.iter().map(|p| p.name()).collect::<Vec<_>>(), [Some("r"), Some("tc1"), Some("o")]);
    let r = m.param("r").unwrap();
    assert_eq!(r.ty().text(), "f64");
    assert!(r.default().is_none());
    assert_eq!(r.where_clause().map(|e| e.text()), Some("r > 0.0"));
    assert!(r.is_required());
    let tc1 = m.param("tc1").unwrap();
    assert_eq!(tc1.default().map(|e| e.text()), Some("0.0"));
    assert!(tc1.where_clause().is_none());
    assert!(!tc1.is_required());
    let o = m.param("o").unwrap();
    assert!(o.default().is_none());
    assert!(!o.is_required());
}

#[test]
fn signal_pub_type_init() {
    let design = build("module S() { pub signal sn: Electrical = 0.0; signal hid: Bit; }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("S").unwrap();
    let sn = m.signal("sn").unwrap();
    assert!(sn.is_pub());
    assert_eq!(sn.ty().text(), "Electrical");
    assert_eq!(sn.init().map(|e| e.text()), Some("0.0"));
    let hid = m.signal("hid").unwrap();
    assert!(!hid.is_pub() && hid.init().is_none());
}

#[test]
fn body_let_pattern_names_each_component() {
    use piperine_lang::pom::BindingOrigin;
    let design = build("module L(a: f64, b: f64) { let (r, f) = (a, b); }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("L").unwrap();
    let lets = m.lets();
    assert_eq!(lets.len(), 1);
    assert_eq!(lets[0].value().text(), "(a, b)");
    let r = m.named("r").expect("name r");
    let f = m.named("f").expect("name f");
    assert_eq!(r.let_item(), Some(lets[0]));
    assert_eq!(f.let_item(), Some(lets[0]));
    assert_eq!(r.origin(), BindingOrigin::BodyLet);
    assert_eq!(r.value().map(|e| e.text()), Some("(a, b)"));
    assert_eq!(lets[0].bindings().iter().map(|b| b.name()).collect::<Vec<_>>(), [Some("r"), Some("f")]);
    assert!(m.named("a").is_none());
}

#[test]
fn definition_place_value_after() {
    let design = build("module D() { signal q: Bit; signal qn: Bit; q = !(r | qn) after tpd; }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("D").unwrap();
    let defs = m.definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].place().text(), "q");
    assert_eq!(defs[0].value().text(), "!(r | qn)");
    assert_eq!(defs[0].after().map(|e| e.text()), Some("tpd"));
    // Body order: two signals and the definition.
    let labels: Vec<_> = m.items().iter().map(|d| d.kind().label()).collect();
    assert_eq!(labels, ["Signal", "Signal", "Definition"]);
}

#[test]
fn generation_for_holds_its_signal_once() {
    use piperine_lang::pom::BindingOrigin;
    let design = build("module G() { for k in 0..4 { signal s: Bit; } }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("G").unwrap();
    let signals = m.signals();
    assert_eq!(signals.len(), 1, "nothing is unrolled");
    let s = signals[0];
    assert_eq!(s.name(), Some("s"));
    let g = s.generation().expect("parent is a generation");
    assert_eq!(s.declared_in(), Some(g.decl()));
    assert_eq!(g.declared_in(), Some(m.decl()));
    assert_eq!(s.module(), Some(m));
    assert!(g.is_for());
    assert_eq!(g.iter().map(|e| e.text()), Some("0..4"));
    assert_eq!(g.pattern().map(|p| p.text()), Some("k"));
    // The pattern's binding is a GenFor child before the items.
    let kids: Vec<_> = g.decl().children().iter().map(|c| c.kind().label()).collect();
    assert_eq!(kids, ["Binding", "Signal"]);
    let binding = g.decl().children()[0].as_binding().unwrap();
    assert_eq!(binding.origin(), BindingOrigin::GenFor);
    assert_eq!(m.generations(), vec![g]);
    assert_eq!(m.signal("s"), Some(s));
    // The module's own children hold the generation, not the signal.
    assert_eq!(m.items().len(), 1);
}

#[test]
fn generation_if_chain_exposes_three_arms() {
    let design = build("module G() { if a { signal x: Bit; } else if b { signal y: Bit; } else { signal z: Bit; } }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("G").unwrap();
    let gens = m.generations();
    assert_eq!(gens.len(), 1);
    assert!(!gens[0].is_for());
    let arms = gens[0].arms();
    assert_eq!(arms.len(), 3);
    assert_eq!(arms.iter().map(|a| a.condition().map(|c| c.text())).collect::<Vec<_>>(), [Some("a"), Some("b"), None]);
    let names = |i: usize| arms[i].items().iter().map(|d| d.name()).collect::<Vec<_>>();
    assert_eq!((names(0), names(1), names(2)), (vec![Some("x")], vec![Some("y")], vec![Some("z")]));
    // Deep accessors reach all three, in source order, each held once.
    assert_eq!(m.signals().iter().map(|s| s.name()).collect::<Vec<_>>(), [Some("x"), Some("y"), Some("z")]);
    // The `else if` is a generation inside the first.
    assert_eq!(gens[0].generations().len(), 1);
    assert_eq!(m.signal("y").unwrap().generation(), Some(gens[0].generations()[0]));
}

#[test]
fn analog_block_is_a_file_item() {
    use piperine_lang::pom::BehaviorKind;
    let design = build("module M(p: Electrical) {}\nanalog for M { }\nanalog for M { }");
    let root = design.root();
    let kids = root.children();
    assert_eq!(kids.iter().map(|k| k.kind().label()).collect::<Vec<_>>(), ["Module", "Behavior", "Behavior"]);
    let b = kids[1].as_behavior().expect("behavior");
    assert_eq!(b.kind(), BehaviorKind::Analog);
    assert_eq!(b.declared_in(), Some(root));
    assert_eq!(b.name(), None);
    assert_eq!(b.target().text(), "M");
    assert!(b.names().is_empty());
    assert!(b.contributions().is_empty() && b.reactions().is_empty());
}

#[test]
fn nested_contribution_is_listed() {
    let text = "\
module R(p: Electrical, n: Electrical) { r: f64 = 1.0; }
analog for R {
    if r > 0.0 { v(p, n) / r +> i(p, n); } else { 0.0 +> i(p, n); }
    for k in 0..2 { match k { 0 => { v(p) +> i(n); } _ => {} } }
}
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let b = design.root().children()[1].as_behavior().unwrap();
    let cs = b.contributions();
    assert_eq!(cs.len(), 3);
    assert_eq!(cs[0].lhs().text(), "v(p, n) / r");
    assert_eq!(cs[0].rhs().text(), "i(p, n)");
    assert_eq!(cs[1].lhs().text(), "0.0");
    assert_eq!(cs[2].lhs().text(), "v(p)");
    assert_eq!(cs[2].rhs().text(), "i(n)");
    assert_eq!(b.statements().len(), 2);
}

#[test]
fn behavior_generic_names() {
    let design = build("module Short<N: u32>() {}\nanalog<N> for Short<N> { }\nanalog<A, B> for Two<A, B> { }");
    let b = design.root().children()[1].as_behavior().unwrap();
    assert_eq!(b.names(), ["N"]);
    assert_eq!(b.target().text(), "Short<N>");
    assert_eq!(b.target().segments()[0].name.text, "Short");
    let two = design.root().children()[2].as_behavior().unwrap();
    assert_eq!(two.names(), ["A", "B"]);
}

#[test]
fn reactions_and_deep_commits() {
    let text = "\
module Q() { signal a: Bit; signal b: Bit; }
digital for Q {
    on clk.rise() { a <- true; if c { b <- false after 1n; } }
    on rst { a <- false; }
}
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let b = design.root().children()[1].as_behavior().unwrap();
    let rs = b.reactions();
    assert_eq!(rs.len(), 2);
    assert_eq!(rs[0].event().text(), "clk.rise()");
    assert_eq!(rs[1].event().text(), "rst");
    assert_eq!(rs[0].body().stmts().len(), 2);
    let commits = rs[0].commits();
    assert_eq!(commits.len(), 2);
    assert_eq!((commits[0].place().text(), commits[0].value().text()), ("a", "true"));
    assert!(commits[0].after().is_none());
    assert_eq!((commits[1].place().text(), commits[1].value().text()), ("b", "false"));
    assert_eq!(commits[1].after().map(|e| e.text()), Some("1n"));
    assert_eq!(rs[1].commits().len(), 1);
    assert_eq!(rs[0].declared_in(), Some(b.decl()));
    assert_eq!(b.kind(), piperine_lang::pom::BehaviorKind::Digital);
}

#[test]
fn reaction_in_digital_generation() {
    let text = "\
module Q() { signal s: Bit; }
digital for Q {
    on a { s <- true; }
    for k in 0..3 { on clk { s <- false; } }
}
";
    let design = build(text);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let b = design.root().children()[1].as_behavior().unwrap();
    let rs = b.reactions();
    assert_eq!(rs.iter().map(|r| r.event().text()).collect::<Vec<_>>(), ["a", "clk"], "deep, in source order");
    let gens = b.generations();
    assert_eq!(gens.len(), 1);
    assert_eq!(rs[1].declared_in(), Some(gens[0].decl()));
    assert_eq!(gens[0].declared_in(), Some(b.decl()));
    assert_eq!(rs[0].declared_in(), Some(b.decl()));
    assert_eq!(rs[1].commits().len(), 1);
}

#[test]
fn analog_block_is_file_item_linked_to_module() {
    use piperine_lang::pom::BehaviorKind;
    let design = build("module M(p: Electrical) {}\nmodule N() {}\nanalog for M { }\ndigital for N { }\nanalog for M { }");
    let root = design.root();
    let m = design.module("M").expect("module M");
    let blocks: Vec<_> = root.children().into_iter().filter_map(|c| c.as_behavior()).collect();
    assert_eq!(blocks.len(), 3);
    // Declared in the file namespace, linked to the module they name.
    assert!(blocks.iter().all(|b| b.declared_in() == Some(root)));
    assert_eq!(blocks[0].kind(), BehaviorKind::Analog);
    assert_eq!(blocks[0].module(), Some(m));
    assert_eq!(blocks[1].module(), design.module("N"));
    assert_eq!(blocks[2].module(), Some(m));
    // The module lists its blocks in source order, and nothing else's.
    assert_eq!(m.behaviors(), [blocks[0], blocks[2]]);
    assert_eq!(design.module("N").unwrap().behaviors(), [blocks[1]]);
    // A declaration inside a block reaches its module through the block.
    assert_eq!(blocks[1].decl().module(), design.module("N"));
}

#[test]
fn several_analog_blocks_in_order() {
    let src = "\
module M(p: Electrical, n: Electrical) {}
analog for M { v(p, n) +> i(p, n); }
digital for M { on p.changed() { } }
analog for M { 0.0 +> i(p); }
";
    let design = build(src);
    let m = design.module("M").unwrap();
    let all = m.behaviors();
    assert_eq!(all.len(), 3);
    assert_eq!(all.iter().map(|b| b.text().split(" for").next().unwrap()).collect::<Vec<_>>(), ["analog", "digital", "analog"]);
    let analog = m.analog_blocks();
    assert_eq!(analog.len(), 2);
    assert_eq!(analog[0].contributions()[0].lhs().text(), "v(p, n)");
    assert_eq!(analog[1].contributions()[0].lhs().text(), "0.0");
    let digital = m.digital_blocks();
    assert_eq!(digital.len(), 1);
    assert_eq!(digital[0].reactions().len(), 1);
    // The three are in file order, as written.
    assert!(all[0].span().start < all[1].span().start && all[1].span().start < all[2].span().start);
}

#[test]
fn docs_reach_reactions_quantities_definitions_and_statements() {
    let text = "\
nature Volt {
    /// the potential
    potential: Voltage { access: v, units: \"V\", abstol: 1u },
    flow: Current { access: i, units: \"A\", abstol: 1p },
}
module M(a: Bit, out q: Bit) {
    /// the definition
    q = a;
    /// the generation
    for k in 0..2 { }
    /// the if
    if true { }
    plain: f64 = 1.0;
}
digital for M {
    /// the reaction
    on a { q <- a; }
    on q { }
}
fn f() -> f64 {
    /// the let
    let x = 1.0;
    /// the expression
    x;
    x
}
";
    let design = build(text);
    let docs = |d: &[piperine_lang::pom::Doc]| d.iter().map(|x| x.text.clone()).collect::<Vec<_>>();
    let volt = design.nature("Volt").unwrap();
    let quantities = volt.quantities();
    assert_eq!(docs(quantities[0].docs()), [" the potential"]);
    assert!(quantities[1].docs().is_empty());
    let m = design.module("M").unwrap();
    assert_eq!(docs(m.definitions()[0].docs()), [" the definition"]);
    let gens = m.generations();
    assert_eq!(docs(gens[0].docs()), [" the generation"]);
    assert_eq!(docs(gens[1].docs()), [" the if"]);
    let reactions = m.behaviors()[0].reactions();
    assert_eq!(docs(reactions[0].docs()), [" the reaction"]);
    assert!(reactions[1].docs().is_empty());
    let stmts = design.function("f").unwrap().body().unwrap().stmts();
    assert_eq!(docs(stmts[0].docs()), [" the let"]);
    assert_eq!(docs(stmts[1].docs()), [" the expression"]);
    assert!(stmts[2].docs().is_empty());
}

#[test]
fn generation_if_items_cover_every_arm() {
    let design = build("module G() { if a { signal x: Bit; } else if b { signal y: Bit; signal w: Bit; } else { signal z: Bit; } }");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let m = design.module("G").unwrap();
    let g = m.generations()[0];
    fn names<'a>(v: Vec<piperine_lang::pom::Decl<'a>>) -> Vec<Option<&'a str>> {
        v.iter().map(|d| d.name()).collect()
    }
    // Every arm's items in source order: the `else if` arm's items are listed, not the nested generation.
    let items = g.items();
    let named: Vec<_> = items.iter().map(|d| (d.kind().label(), d.name())).collect();
    assert_eq!(named, [("Signal", Some("x")), ("Signal", Some("y")), ("Signal", Some("w")), ("Signal", Some("z"))]);
    // `arms()` is unchanged: three arms.
    assert_eq!(g.arms().len(), 3);
    assert_eq!(names(g.arms()[1].items()), [Some("y"), Some("w")]);
    // A `for` lists its body.
    let design = build("module G() { for i in 0..2 { signal a: Bit; signal b: Bit; } }");
    assert_eq!(names(design.module("G").unwrap().generations()[0].items()), [Some("a"), Some("b")]);
}
