//! Scope: the declaration tree mirrors the source (requirement POM-02 and the
//! per-kind requirements): which declaration sits under which, in source
//! order, with its kind, name, span and visibility.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::msl::parser::parse;
use piperine_lang::pom::{DeclKind, NamespaceOrigin, Reflect};
use support::{build, codes};

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
    let ast = parse(text).file;
    assert!(parse(text).diagnostics.is_empty(), "fixture must parse cleanly");
    let design = build(text);
    assert!(codes(&design).is_empty());
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
    assert!(support::codes(&design).is_empty(), "{:?}", design.diagnostics());

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
    assert_eq!(body.stmts.len(), 1);

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
    assert!(support::codes(&design).is_empty(), "{:?}", design.diagnostics());
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
