//! Scope: the classifications the resolver makes over names (POM-09 and the
//! behavior-block binding): which module a behavior block belongs to and the
//! errors `E0401`, `E0402` and `E0403` when it does not; what a declared type
//! makes of a port (`PortKind`) and of a struct (the bundle flag); and what a
//! call or a `Name { .. }` expression is: instance, template, literal; and what
//! a member (`inst.name`, `bundle.field`, `carrier.method()`) resolves to.

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

#[test]
fn access_alias_resolves_by_first_argument() {
    let src = "\
module M(a: Electrical, b: VSignal, t: Thermal) {
    signal o: Electrical;
}
analog for M {
    let q = a;
    let x = v(a);
    let y = v(b);
    let z = temp(t);
    let c = i(a);
    let m = v(q);
    let p = potential(o);
    let n = nothing(a);
}
";
    let design = with_prelude(src);
    let callee_res = |call: &str| -> Resolution {
        let e = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text() == call && !e.enclosing().file().is_prelude()).unwrap_or_else(|| panic!("call `{call}`"));
        let ExprKind::Call { callee, .. } = e.kind() else { panic!("a call") };
        let ExprKind::Path(p) = design.expr(*callee).kind() else { panic!("a path callee") };
        design.path(*p).segments()[0].res
    };
    let quantity_of = |nature: &str, access: &str| {
        let q = design.nature(nature).unwrap().quantities().into_iter().find(|q| q.access().is_some_and(|a| a.text() == access)).unwrap();
        Resolution::Decl(q.decl().id())
    };
    // The nature of the first argument picks among the natures that declare `v`.
    assert_eq!(callee_res("v(a)"), quantity_of("Electrical", "v"));
    assert_eq!(callee_res("v(b)"), quantity_of("VSignal", "v"));
    // One nature declares `temp` and one `i`: no argument needed.
    assert_eq!(callee_res("temp(t)"), quantity_of("Thermal", "temp"));
    assert_eq!(callee_res("i(a)"), quantity_of("Electrical", "i"));
    // The first argument has no declared type and two natures declare `v`.
    assert_eq!(callee_res("v(q)"), Resolution::Deferred(piperine_lang::pom::DeferReason::AmbiguousAccess));
    // Every one of them is a probe; so is the built-in `potential`.
    let classes = call_classes(&design);
    for call in ["v(a)", "v(b)", "temp(t)", "i(a)", "v(q)", "potential(o)"] {
        assert_eq!(classes.iter().find(|(t, _)| t == call).unwrap().1, CallClass::Probe, "{call}");
    }
    // A name that is no alias is still reported once; an alias is not.
    assert_eq!(support::codes(&design), ["E0161"]);
    assert_eq!(primary_text(&design, &design.diagnostics()[0]), "nothing");
    assert_eq!(classes.iter().find(|(t, _)| t == "nothing(a)").unwrap().1, CallClass::Function);
    // A declaration in scope wins over the alias.
    let shadowed = with_prelude("fn v(x: f64) -> f64 { x }\nmodule M(a: Electrical) { let y = v(1.0); }\n");
    assert!(shadowed.diagnostics().is_empty(), "{:?}", shadowed.diagnostics());
    assert_eq!(call_classes(&shadowed), [("v(1.0)".to_string(), CallClass::Function)]);
}

/// What the member expression with this text resolves to (a field access or a method call).
fn member_res(design: &Design, text: &str) -> Resolution {
    let e = (0..design.expr_count())
        .map(|i| design.expr(ExprId(i as u32)))
        .find(|e| e.text() == text && !e.enclosing().file().is_prelude() && matches!(e.kind(), ExprKind::Field { .. } | ExprKind::MethodCall { .. }))
        .unwrap_or_else(|| panic!("member expression `{text}`"));
    match e.kind() {
        ExprKind::Field { res, .. } | ExprKind::MethodCall { res, .. } => *res,
        _ => unreachable!(),
    }
}

#[test]
fn instance_pub_member_resolves() {
    let src = "\
module Cell(a: Electrical) {
    pub signal sn: Electrical;
    pub k: f64 = 1.0;
    pub let q = 2.0;
}
module Top(a: Electrical) {
    let c = Cell(a);
    let x = c.sn;
    let y = c.k;
    let z = c.q;
    let p = c.a;
}
";
    let design = with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let cell = design.module("Cell").unwrap();
    let decl = |d: piperine_lang::pom::Decl| Resolution::Decl(d.id());
    assert_eq!(member_res(&design, "c.sn"), decl(cell.signal("sn").unwrap().decl()));
    assert_eq!(member_res(&design, "c.k"), decl(cell.param("k").unwrap().decl()));
    assert_eq!(member_res(&design, "c.q"), decl(cell.named("q").unwrap().decl()));
    // A port is the module's interface: always readable by its parent.
    assert_eq!(member_res(&design, "c.a"), decl(cell.port("a").unwrap().decl()));
}

#[test]
fn instance_private_member_is_e0343() {
    let src = "\
module Cell(a: Electrical) {
    signal hid: Electrical;
    pub signal open: Electrical;
}
module Top(a: Electrical) {
    let c = Cell(a);
    let x = c.hid;
    let y = c.open;
}
";
    let design = with_prelude(src);
    let errors = with_code(&design, "E0343");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    // At the member name, and the member is still linked.
    assert_eq!(primary_text(&design, errors[0]), "hid");
    let cell = design.module("Cell").unwrap();
    assert_eq!(member_res(&design, "c.hid"), Resolution::Decl(cell.signal("hid").unwrap().decl().id()));
    assert_eq!(support::codes(&design), ["E0343"]);
}

#[test]
fn bundle_field_resolves() {
    let src = "\
struct Stream { out valid: Bit, in ready: Bit }
struct Outer { s: Stream }
fn f(h: Stream) { let t: Stream = h; h.valid; t.ready; }
module Tx(tx: Stream, rx: flip Stream, o: Outer, arr: [Stream; 2]) {
    signal bus: Stream;
    let a = tx.valid;
    let b = rx.ready;
    let c = bus.ready;
    let d = o.s.valid;
    let e = arr[1].valid;
}
";
    let design = with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let stream = design.find("Stream").and_then(|d| d.as_struct()).unwrap();
    let field = |s: &piperine_lang::pom::Struct, name: &str| Resolution::Decl(s.fields().into_iter().find(|f| f.name() == Some(name)).unwrap().decl().id());
    let valid = field(&stream, "valid");
    let ready = field(&stream, "ready");
    assert_eq!(member_res(&design, "tx.valid"), valid);
    assert_eq!(member_res(&design, "rx.ready"), ready);
    assert_eq!(member_res(&design, "bus.ready"), ready);
    assert_eq!(member_res(&design, "o.s.valid"), valid);
    assert_eq!(member_res(&design, "arr[1].valid"), valid);
    // A nested field is a field of the outer struct first.
    let outer = design.find("Outer").and_then(|d| d.as_struct()).unwrap();
    assert_eq!(member_res(&design, "o.s"), field(&outer, "s"));
    // A function parameter and an annotated local have declared types too.
    assert_eq!(member_res(&design, "h.valid"), valid);
    assert_eq!(member_res(&design, "t.ready"), ready);
}

#[test]
fn unknown_receiver_is_deferred_without_diagnostic() {
    let src = "\
fn f() -> f64 { 1.0 }
module M<T>(a: f64, g: T) {
    let q = a;
    let r = q.thing;
    let s = f().bar;
    let t = a.other;
    let u = g.field;
}
";
    let design = with_prelude(src);
    let deferred = Resolution::Deferred(piperine_lang::pom::DeferReason::ReceiverTypeUnknown);
    for text in ["q.thing", "f().bar", "a.other", "g.field"] {
        assert_eq!(member_res(&design, text), deferred, "`{text}`");
    }
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
}

#[test]
fn methods_resolve_to_impl_fn_or_builtin() {
    let src = "\
struct Pair { a: Electrical }
impl Pair { pub fn vd(self) -> f64 { 1.0 } }
impl Electrical { pub fn big(self) -> f64 { 1.0 } }
impl Bit { pub fn prev(self) -> bool { true } }
impl Logic { pub fn lowest(self) -> Logic { self } }
module M(in clk: Bit, p: Electrical, w: Wire, pr: Pair, a: f64) {
    signal s: Bit;
    let r = clk.rise();
    let ch = s.changed();
    let pv = s.prev();
    let ot = s.others();
    let inf = p.inflow();
    let lv = w.level(1);
    let bg = p.big();
    let vd = pr.vd();
    let lo = w.lowest();
    let no = clk.nothere();
    let un = a.rise();
    let tail = pr.a.changed();
}
";
    let design = with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let impl_fn = |owner: &str, name: &str| {
        let items = design.root().children().into_iter().filter_map(|c| c.as_impl()).filter(|i| i.self_ty().text() == owner).flat_map(|i| i.items());
        Resolution::Decl(items.into_iter().find(|f| f.name() == Some(name)).unwrap_or_else(|| panic!("`{name}` in an impl of {owner}")).id())
    };
    let builtin = |name: &str| Resolution::Builtin(piperine_lang::pom::builtin_method(name).unwrap());
    // `rise` is declared by the prelude's `impl Bit`.
    let rise = design.nature("Bit").unwrap().impls().into_iter().flat_map(|i| i.items()).find(|f| f.name() == Some("rise")).unwrap();
    let prelude_rise = Resolution::Decl(rise.id());
    assert_eq!(member_res(&design, "clk.rise()"), prelude_rise);
    // The carrier built-ins come first: a user `impl Bit` cannot take `prev` or `changed`.
    assert_eq!(member_res(&design, "s.changed()"), builtin("changed"));
    assert_eq!(member_res(&design, "s.prev()"), builtin("prev"));
    assert_eq!(member_res(&design, "s.others()"), builtin("others"));
    assert_eq!(member_res(&design, "p.inflow()"), builtin("inflow"));
    assert_eq!(member_res(&design, "w.level(1)"), builtin("level"));
    // Then the `impl`s of the nature or bundle, then those of the value type.
    assert_eq!(member_res(&design, "p.big()"), impl_fn("Electrical", "big"));
    assert_eq!(member_res(&design, "pr.vd()"), impl_fn("Pair", "vd"));
    assert_eq!(member_res(&design, "w.lowest()"), impl_fn("Logic", "lowest"));
    // A carrier field is a carrier: `pr.a.changed()` is the built-in too.
    assert_eq!(member_res(&design, "pr.a.changed()"), builtin("changed"));
    // A known receiver with no such method names nothing; an unknown receiver defers.
    assert_eq!(member_res(&design, "clk.nothere()"), Resolution::Unresolved);
    assert_eq!(member_res(&design, "a.rise()"), Resolution::Deferred(piperine_lang::pom::DeferReason::ReceiverTypeUnknown));
}

/// POM-09 AC 1: a generic is a carrier because its bound resolves to the
/// prelude's `Nature`; an unresolved bound is `Unknown`, and another trait
/// that is merely spelled `Nature` is a value bound.
#[test]
fn generic_carrier_follows_the_resolved_bound() {
    let src = "module M<N: Nature>(g: N) {}";
    // No declaration of `Nature` anywhere: the bound is unresolved, nothing is guessed.
    let none = build(src);
    assert_eq!(none.module("M").unwrap().port("g").unwrap().kind(), PortKind::Unknown);
    assert!(none.diagnostics().iter().any(|d| d.diagnostic.code == "E0161"));
    // A user `trait Nature` outside the prelude is not the carrier bound.
    let user = with_prelude("mod x { pub trait Nature {} }\nmodule M<N: x::Nature>(g: N) {}");
    assert_eq!(user.module("M").unwrap().port("g").unwrap().kind(), PortKind::ValueInput);
    // The prelude's `Nature` still makes a carrier, through arrays too.
    let real = with_prelude("module M<N: Nature>(g: N, ga: [N; 2]) {}");
    let kinds: Vec<_> = real.module("M").unwrap().ports().iter().map(|p| p.kind()).collect();
    assert_eq!(kinds, [PortKind::Carrier, PortKind::Carrier]);
}

/// A name written for a port, a param, a field or a parameter that the
/// callee does not have names nothing: one `E0161` at that name (resolution.md,
/// "the slot stays `Unresolved`, one `E0161` is reported").
#[test]
fn named_binding_without_a_target_is_e0161() {
    let src = "\
struct Plain { x: f64 }
fn f(a: f64) -> f64 { a }
module R(p: Electrical, n: Electrical) { r: f64 = 1.0; }
module Top() {
    signal s: Electrical;
    let ok = R(p: s, n: s) { r: 2.0 };
    let bad = R(q: s, n: s) { nope: 1.0 };
    let pl = Plain { x: 1.0, y: 2.0 };
    let t = R { zz: 3.0 };
    let v = f(b: 1.0);
}
";
    let design = with_prelude(src);
    let errors = with_code(&design, "E0161");
    let at: Vec<&str> = errors.iter().map(|d| primary_text(&design, d)).collect();
    assert_eq!(at, ["q", "nope", "y", "zz", "b"], "{:?}", design.diagnostics());
    assert!(errors.iter().all(|d| d.diagnostic.message.contains("no ")), "{errors:?}");
    // The names that do exist link, the ones that do not stay unresolved.
    let call = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text().starts_with("R(q: s")).unwrap();
    let ExprKind::Call { args, params, .. } = call.kind() else { panic!("a call") };
    assert_eq!(args[0].res, Resolution::Unresolved);
    assert!(matches!(args[1].res, Resolution::Decl(_)));
    assert_eq!(params.as_ref().unwrap()[0].res, Resolution::Unresolved);
}

/// §8.4.8: a member that is missing or not `pub` is `E0343`.
#[test]
fn missing_instance_member_is_e0343() {
    let src = "\
module Cell(a: Electrical) { pub signal open: Electrical; }
module Top(a: Electrical) {
    let c = Cell(a);
    let y = c.open;
    let z = c.nope;
}
";
    let design = with_prelude(src);
    let errors = with_code(&design, "E0343");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(primary_text(&design, errors[0]), "nope");
    assert!(errors[0].diagnostic.message.contains("Cell"), "{:?}", errors[0]);
    assert_eq!(member_res(&design, "c.nope"), Resolution::Unresolved);
    assert_eq!(support::codes(&design), ["E0343"]);
}
