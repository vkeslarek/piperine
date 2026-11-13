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
