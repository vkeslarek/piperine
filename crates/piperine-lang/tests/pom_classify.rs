//! Scope: the classifications the resolver makes over names (POM-09 and the
//! behavior-block binding): which module a behavior block belongs to and the
//! errors `E0401`, `E0402` and `E0403` when it does not; what a declared type
//! makes of a port (`PortKind`) and of a struct (the bundle flag); and what a
//! call or a `Name { .. }` expression is: instance, template, literal.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{CallClass, Design, DesignDiagnostic, ExprId, ExprKind, PortKind, Reflect, Resolution, StructClass};
use support::build;

const PRELUDE: &str = include_str!("fixtures/pom/prelude.msl");

/// A design of `src` with the fixture prelude beside it.
fn with_prelude(src: &str) -> Design {
    Design::builder().prelude("prelude.msl", PRELUDE).source("top.msl", src).build()
}

fn with_code<'a>(design: &'a Design, code: &str) -> Vec<&'a DesignDiagnostic> {
    design.diagnostics().iter().filter(|d| d.diagnostic.code == code).collect()
}

/// The authored text of the primary span of a diagnostic.
fn primary_text<'a>(design: &'a Design, d: &DesignDiagnostic) -> &'a str {
    let file = &design.files()[d.file.index()];
    &file.text()[d.diagnostic.span().range()]
}

#[test]
fn behavior_without_module_is_e0401() {
    let src = "struct NotAModule {}\nanalog for Nope { }\nanalog for NotAModule { }\ndigital for b::C { }\n";
    let design = build(src);
    let errors = with_code(&design, "E0401");
    let at: Vec<_> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["Nope", "NotAModule", "b::C"]);
    // The blocks stay in the tree with no module, and each is one error, not two.
    let blocks: Vec<_> = design.root().children().into_iter().filter_map(|c| c.as_behavior()).collect();
    assert_eq!(blocks.len(), 3);
    assert!(blocks.iter().all(|b| b.module().is_none()));
    assert_eq!(support::codes(&design).iter().filter(|c| c.as_str() == "E0161").count(), 0, "{:?}", design.diagnostics());
}

#[test]
fn behavior_for_extern_module_is_e0402() {
    let design = build("extern \"osdi\" module Bsim4(d: Electrical) { l: f64 = 1u; }\nanalog for Bsim4 { }\n");
    let errors = with_code(&design, "E0402");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(primary_text(&design, errors[0]), "Bsim4");
    // The block is still linked: the module exists, it is just not allowed to have one.
    let block = design.root().children().into_iter().find_map(|c| c.as_behavior()).unwrap();
    assert_eq!(block.module(), design.module("Bsim4"));
    assert!(with_code(&design, "E0401").is_empty());
}

#[test]
fn behavior_names_mismatch_is_e0403() {
    let src = "\
module Short<N: u32>() {}
module Plain() {}
analog<N> for Short<N> { }
analog<A> for Short<N> { }
analog<N, M> for Short<N, M> { }
analog<N> for Short<A> { }
analog for Short { }
analog for Plain { }
analog<N> for Plain<N> { }
";
    let design = build(src);
    let errors = with_code(&design, "E0403");
    let heads: Vec<_> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(heads, ["analog<A> for Short<N>", "analog<N, M> for Short<N, M>", "analog<N> for Short<A>", "analog for Short", "analog<N> for Plain<N>"]);
    // The first block and the one for a module without generics are fine.
    assert_eq!(errors.len(), 5);
    assert!(with_code(&design, "E0401").is_empty() && with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
}

#[test]
fn behavior_for_module_in_other_source_is_e0401() {
    let top = "mod b;\nuse b::M;\nanalog for M { }\nanalog for b::M { }\n";
    let design = Design::builder().source("top.msl", top).source("b.msl", "pub module M() {}\n").build();
    let errors = with_code(&design, "E0401");
    let at: Vec<_> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["M", "b::M"]);
    // The module is found; it is in the wrong file, so no block is linked.
    let m = design.find("b::M").and_then(|d| d.as_module()).expect("module M in b.msl");
    assert!(m.behaviors().is_empty());
    assert!(design.root().children().into_iter().filter_map(|c| c.as_behavior()).all(|b| b.module().is_none()));
    // Beside its module, in its own file, a block is fine.
    let own = Design::builder().source("top.msl", "mod b;\n").source("b.msl", "pub module M() {}\nanalog for M { }\n").build();
    assert!(with_code(&own, "E0401").is_empty(), "{:?}", own.diagnostics());
    assert_eq!(own.find("b::M").and_then(|d| d.as_module()).unwrap().behaviors().len(), 1);
}

#[test]
fn port_kinds() {
    let src = "\
struct Pair { a: Electrical, b: Electrical }
struct Val { x: f64 }
enum Mode { A, B }
module M<N: Nature, T: Repr>(
    p: Electrical, q: [Bit; 4], r: Pair, in s: Pair, t: flip Pair, u: [flip Pair; 2],
    f: f64, o: Option<f64>, v: Val, m: Mode, g: N, ga: [N; 2], h: T, z: Nope,
) {}
";
    let design = with_prelude(src);
    let m = design.module("M").unwrap();
    let kinds: Vec<(&str, PortKind)> = m.ports().iter().map(|p| (p.name().unwrap(), p.kind())).collect();
    let expected = [
        ("p", PortKind::Carrier),
        ("q", PortKind::Carrier),
        ("r", PortKind::Bundle),
        ("s", PortKind::Bundle),
        ("t", PortKind::Bundle),
        ("u", PortKind::Bundle),
        ("f", PortKind::ValueInput),
        ("o", PortKind::ValueInput),
        ("v", PortKind::ValueInput),
        ("m", PortKind::ValueInput),
        // A generic bounded by `Nature` is a carrier; any other generic is a value.
        ("g", PortKind::Carrier),
        ("ga", PortKind::Carrier),
        ("h", PortKind::ValueInput),
        // Nothing declares `Nope`.
        ("z", PortKind::Unknown),
    ];
    assert_eq!(kinds, expected);
}

#[test]
fn struct_with_carrier_field_is_bundle() {
    let src = "\
struct Cell { in wl: Bit, bl: Electrical }
struct Arr { xs: [Electrical; 2] }
struct Nested { c: Cell, n: u8 }
struct Deep { inner: Nested }
struct Plain { x: f64, y: Option<f64> }
struct Empty {}
struct Generic<N: Nature> { a: N }
struct Dangling { q: Nope }
";
    let design = with_prelude(src);
    let bundle = |name: &str| design.find(name).and_then(|d| d.as_struct()).unwrap_or_else(|| panic!("struct {name}: {:?}", design.diagnostics())).is_bundle();
    for yes in ["Cell", "Arr", "Nested", "Deep", "Generic"] {
        assert!(bundle(yes), "`{yes}` is a bundle");
    }
    for no in ["Plain", "Empty", "Dangling"] {
        assert!(!bundle(no), "`{no}` is not a bundle");
    }
}

#[test]
fn self_referential_struct_bundle_check_ends() {
    let src = "\
struct Me { next: Me, x: f64 }
struct A { b: B }
struct B { a: A }
struct C { d: D }
struct D { c: C, wire: Electrical }
struct E { c: C }
";
    let design = with_prelude(src);
    let bundle = |name: &str| design.find(name).and_then(|d| d.as_struct()).unwrap().is_bundle();
    // A cycle with no carrier in it is no bundle.
    assert!(!bundle("Me") && !bundle("A") && !bundle("B"));
    // A cycle with a carrier on it is a bundle all around, whichever member is met first.
    assert!(bundle("C") && bundle("D") && bundle("E"));
}

/// The class of every call of the root file, with the call's text, in source order.
fn call_classes(design: &Design) -> Vec<(String, CallClass)> {
    (0..design.expr_count())
        .map(|i| design.expr(ExprId(i as u32)))
        .filter(|e| !e.enclosing().file().is_prelude())
        .filter_map(|e| match e.kind() {
            ExprKind::Call { class, .. } => Some((e.text().to_string(), *class)),
            _ => None,
        })
        .collect()
}

/// The class of every `Name { .. }` expression of the root file, with its text.
fn struct_classes(design: &Design) -> Vec<(String, StructClass)> {
    (0..design.expr_count())
        .map(|i| design.expr(ExprId(i as u32)))
        .filter(|e| !e.enclosing().file().is_prelude())
        .filter_map(|e| match e.kind() {
            ExprKind::Struct { class, .. } => Some((e.text().to_string(), *class)),
            _ => None,
        })
        .collect()
}

#[test]
fn named_instance_with_connections_and_params() {
    let src = "\
module Resistor(p: Electrical, n: Electrical) { r: f64 = 1.0; }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let r1 = Resistor(a, b) { r: 1k };
    let r2 = Resistor(a, n: b);
}
";
    let design = with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let top = design.module("Top").unwrap();
    let resistor = design.module("Resistor").unwrap();
    let instances = top.instances();
    assert_eq!(instances.len(), 2);
    let r1 = &instances[0];
    assert_eq!(r1.name(), Some("r1"));
    assert!(!r1.is_anonymous());
    assert_eq!(r1.module(), Some(resistor));
    assert!(!r1.via_template());
    assert_eq!(r1.declared_in(), top.decl());
    assert_eq!(r1.connections().iter().map(|c| c.text()).collect::<Vec<_>>(), ["a", "b"]);
    let params = r1.param_bindings();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name.text, "r");
    // The param binding links to the param it names.
    assert_eq!(params[0].res, Resolution::Decl(resistor.param("r").unwrap().decl().id()));
    // A named connection links to its port.
    let ExprKind::Call { args, .. } = r1.expr().kind() else { panic!("an instance is a call") };
    assert!(args.iter().all(|a| a.res == Resolution::Unresolved));
    let r2 = top.instance("r2").expect("instance r2");
    let ExprKind::Call { args, .. } = r2.expr().kind() else { panic!("an instance is a call") };
    assert_eq!(args[1].res, Resolution::Decl(resistor.port("n").unwrap().decl().id()));
    // The name is the binding of the `let`, and the binding reaches the instance back.
    let binding = top.named("r1").unwrap();
    assert_eq!(binding.instance(), Some(r1.clone()));
    assert_eq!(r1.expr().as_instance(), Some(r1.clone()));
    assert!(top.instance("nope").is_none());
}

#[test]
fn anonymous_instance_in_analog_and_plain_call() {
    let src = "\
fn windup(u: f64) -> f64 { u }
module Integ(inp: Electrical) { signal o: Electrical; }
analog for Integ {
    potential(o) +> idt(1.0, 0.0);
    let w = windup(1.0);
}
";
    let design = with_prelude(src);
    let integ = design.module("Integ").unwrap();
    let instances = integ.instances();
    assert_eq!(instances.len(), 1, "`windup` is a function, not an instance");
    let idt = &instances[0];
    assert!(idt.is_anonymous() && idt.name().is_none());
    assert_eq!(idt.module(), design.module("idt"));
    assert_eq!(idt.declared_in(), integ.analog_blocks()[0].decl());
    let classes = call_classes(&design);
    let class_of = |text: &str| classes.iter().find(|(t, _)| t == text).unwrap_or_else(|| panic!("call `{text}` in {classes:?}")).1;
    assert_eq!(class_of("windup(1.0)"), CallClass::Function);
    assert!(matches!(class_of("idt(1.0, 0.0)"), CallClass::Instance { module: Some(_), via: None }));
    // A built-in module is an instance of no declared module.
    let builtin = with_prelude("module T() {}\nanalog for T { let d = ddt(1.0); }\n");
    assert_eq!(call_classes(&builtin), [("ddt(1.0)".to_string(), CallClass::Instance { module: None, via: None })]);
    let anonymous = builtin.module("T").unwrap().instances();
    assert_eq!(anonymous.len(), 1);
    assert!(anonymous[0].module().is_none());
}

#[test]
fn struct_expression_classes() {
    let src = "\
struct Pair { a: Electrical, b: Electrical }
struct Plain { x: f64 }
enum E { V { k: u8 } }
module R(p: Electrical) { r: f64 = 1.0; }
module Top() {
    signal s: Electrical;
    let t = R { r: 2.0 };
    let b = Pair { a: s, b: s };
    let pl = Plain { x: 1.0 };
    let e = E::V { k: 1 };
    let u = Nope { z: 1 };
}
";
    let design = with_prelude(src);
    let r = design.module("R").unwrap();
    let id_of = |name: &str| design.find(name).unwrap().id();
    let classes = struct_classes(&design);
    let class_of = |text: &str| classes.iter().find(|(t, _)| t == text).unwrap_or_else(|| panic!("`{text}` in {classes:?}")).1;
    assert_eq!(class_of("R { r: 2.0 }"), StructClass::Template(id_of("R")));
    assert_eq!(class_of("Pair { a: s, b: s }"), StructClass::BundleLiteral);
    assert_eq!(class_of("Plain { x: 1.0 }"), StructClass::StructLiteral);
    assert_eq!(class_of("E::V { k: 1 }"), StructClass::StructLiteral);
    // A name that resolves to nothing is classified as nothing (and `E0161` was reported once).
    assert_eq!(class_of("Nope { z: 1 }"), StructClass::Unclassified);
    assert_eq!(with_code(&design, "E0161").len(), 1);
    // The names of a template's fields are its module's params, a literal's are the struct's fields.
    let field_res = |text: &str| -> Vec<Resolution> {
        let e = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text() == text && !e.enclosing().file().is_prelude()).unwrap();
        let ExprKind::Struct { fields, .. } = e.kind() else { panic!("a struct expression") };
        fields.iter().map(|f| f.res).collect()
    };
    assert_eq!(field_res("R { r: 2.0 }"), [Resolution::Decl(r.param("r").unwrap().decl().id())]);
    let pair = design.find("Pair").and_then(|d| d.as_struct()).unwrap();
    let pair_fields: Vec<_> = pair.fields().iter().map(|f| Resolution::Decl(f.decl().id())).collect();
    assert_eq!(field_res("Pair { a: s, b: s }"), pair_fields);
}

#[test]
fn instance_through_template_value() {
    let src = "\
module Cell(p: Electrical) {}
const T: Cell = Cell {};
fn pick(sel: bool) -> Cell { Cell {} }
module Bank<C: Fn(Electrical)>(a: Electrical) {
    cell: C;
    let x = cell(a);
}
module Top(tpl: Cell, a: Electrical) {
    keep: Cell = Cell {};
    let plain = Cell(a);
    let by_port = tpl(a);
    let by_param = keep(a);
    let by_const = T(a);
    let by_fn = pick(true)(a);
    let tl = Cell {};
    let by_let = tl(a);
}
";
    let design = with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let cell = design.module("Cell");
    let top = design.module("Top").unwrap();
    let via = |name: &str| {
        let i = top.instance(name).unwrap_or_else(|| panic!("instance `{name}`"));
        (i.via_template(), i.via().and_then(|v| v.name().map(str::to_string)), i.module())
    };
    assert_eq!(via("plain"), (false, None, cell));
    assert_eq!(via("by_port"), (true, Some("tpl".into()), cell));
    assert_eq!(via("by_param"), (true, Some("keep".into()), cell));
    assert_eq!(via("by_const"), (true, Some("T".into()), cell));
    assert_eq!(via("by_fn"), (true, Some("pick".into()), cell));
    assert_eq!(via("by_let"), (true, Some("tl".into()), cell));
    // A `Fn(..)`-bounded generic names no module.
    let bank = design.module("Bank").unwrap();
    let x = bank.instance("x").unwrap();
    assert_eq!((x.via_template(), x.via().and_then(|v| v.name().map(str::to_string)), x.module()), (true, Some("cell".into()), None));
    // The call that picks the template is a plain call.
    let classes = call_classes(&design);
    assert!(classes.iter().any(|(t, c)| t == "pick(true)" && *c == CallClass::Function), "{classes:?}");
}

#[test]
fn impl_on_module_is_e0130() {
    let design = with_prelude("module M() {}\nstruct S {}\nimpl M { pub fn f(self) {} }\nimpl S { pub fn g(self) {} }\n");
    let errors = with_code(&design, "E0130");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(primary_text(&design, errors[0]), "M");
}

#[test]
fn generation_body_is_not_unrolled() {
    let src = "\
module Cell(a: Bit) {}
module T() {
    for k in 0..1000 { signal s: Bit; let u = Cell(s); }
}
";
    let design = with_prelude(src);
    let t = design.module("T").unwrap();
    assert_eq!(t.signals().len(), 1);
    let instances = t.instances();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].name(), Some("u"));
    // The instance sits in the loop, as written.
    assert!(instances[0].declared_in().as_generation().is_some());
    assert_eq!(instances[0].module(), design.module("Cell"));
}

#[test]
fn self_placement_under_generation_if() {
    let src = "module M(a: Electrical) {\n    if false { let sub = M(a); }\n}\n";
    let design = with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let m = design.module("M").unwrap();
    let instances = m.instances();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].module(), Some(m));
    assert_eq!(instances[0].name(), Some("sub"));
}

#[test]
fn unresolved_callee_is_plain_call() {
    let design = with_prelude("module T() { let a = nothere(1); }\n");
    assert_eq!(call_classes(&design), [("nothere(1)".to_string(), CallClass::Function)]);
    // The missing name is reported once, by name resolution; classification adds nothing.
    assert_eq!(support::codes(&design), ["E0161"]);
    assert!(design.module("T").unwrap().instances().is_empty());
}
