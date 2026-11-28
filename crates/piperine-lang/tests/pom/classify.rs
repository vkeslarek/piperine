//! Scope: the classifications the resolver makes over names (POM-09 and the
//! behavior-block binding): which module a behavior block belongs to and the
//! errors `E0401`, `E0402` and `E0403` when it does not; what a declared type
//! makes of a port (`PortKind`) and of a struct (the bundle flag); and what a
//! call or a `Name { .. }` expression is: instance, template, literal; and what
//! a member (`inst.name`, `bundle.field`, `carrier.method()`) resolves to.


use piperine_lang::pom::{CallClass, Design, DesignDiagnostic, ExprId, ExprKind, PortKind, Reflect, Resolution, StructClass};
use piperine_lang_fixtures::pom::PomSource;

/// Associated test helpers of `classify`, grouped on one fixture type.
struct ClassifyFixture;

impl ClassifyFixture {
    /// A design of `src` over the embedded standard library.
    fn with_prelude(src: &str) -> Design {
        Design::builder().source("top.msl", src).build()
    }

    fn with_code<'a>(design: &'a Design, code: &str) -> Vec<&'a DesignDiagnostic> {
        design.diagnostics().iter().filter(|d| d.diagnostic.code == code).collect()
    }

    /// The authored text of the primary span of a diagnostic.
    fn primary_text<'a>(design: &'a Design, d: &DesignDiagnostic) -> &'a str {
        let file = &design.files()[d.file.index()];
        &file.text()[d.diagnostic.span().range()]
    }

    /// The class of every call of the root file, with the call's text, in source order.
    fn call_classes(design: &Design) -> Vec<(String, CallClass)> {
        (0..design.expr_count())
            .map(|i| design.expr(ExprId(i as u32)))
            .filter(|e| !e.enclosing().file().krate().is_std())
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
            .filter(|e| !e.enclosing().file().krate().is_std())
            .filter_map(|e| match e.kind() {
                ExprKind::Struct { class, .. } => Some((e.text().to_string(), *class)),
                _ => None,
            })
            .collect()
    }

    /// What the member expression with this text resolves to (a field access or a method call).
    fn member_res(design: &Design, text: &str) -> Resolution {
        let e = (0..design.expr_count())
            .map(|i| design.expr(ExprId(i as u32)))
            .find(|e| e.text() == text && !e.enclosing().file().krate().is_std() && matches!(e.kind(), ExprKind::Field { .. } | ExprKind::MethodCall { .. }))
            .unwrap_or_else(|| panic!("member expression `{text}`"));
        match e.kind() {
            ExprKind::Field { res, .. } | ExprKind::MethodCall { res, .. } => *res,
            _ => unreachable!(),
        }
    }
}

#[test]
fn behavior_without_module_is_e0401() {
    let src = "struct NotAModule {}\nanalog for Nope { }\nanalog for NotAModule { }\ndigital for b::C { }\n";
    let design = PomSource::build(src);
    let errors = ClassifyFixture::with_code(&design, "E0401");
    let at: Vec<_> = errors.iter().map(|d| ClassifyFixture::primary_text(&design, d)).collect();
    assert_eq!(at, ["Nope", "NotAModule", "b::C"]);
    // The blocks stay in the tree with no module, and each is one error, not two.
    let blocks: Vec<_> = design.root().children().into_iter().filter_map(|c| c.as_behavior()).collect();
    assert_eq!(blocks.len(), 3);
    assert!(blocks.iter().all(|b| b.module().is_none()));
    assert_eq!(PomSource::codes(&design).iter().filter(|c| c.as_str() == "E0161").count(), 0, "{:?}", design.diagnostics());
}

#[test]
fn behavior_for_extern_module_is_e0402() {
    let design = PomSource::build("extern \"osdi\" module Bsim4(d: Electrical) { l: f64 = 1u; }\nanalog for Bsim4 { }\n");
    let errors = ClassifyFixture::with_code(&design, "E0402");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(ClassifyFixture::primary_text(&design, errors[0]), "Bsim4");
    // The block is still linked: the module exists, it is just not allowed to have one.
    let block = design.root().children().into_iter().find_map(|c| c.as_behavior()).unwrap();
    assert_eq!(block.module(), design.module("Bsim4"));
    assert!(ClassifyFixture::with_code(&design, "E0401").is_empty());
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
    let design = PomSource::build(src);
    let errors = ClassifyFixture::with_code(&design, "E0403");
    let heads: Vec<_> = errors.iter().map(|d| ClassifyFixture::primary_text(&design, d)).collect();
    assert_eq!(heads, ["analog<A> for Short<N>", "analog<N, M> for Short<N, M>", "analog<N> for Short<A>", "analog for Short", "analog<N> for Plain<N>"]);
    // The first block and the one for a module without generics are fine.
    assert_eq!(errors.len(), 5);
    assert!(ClassifyFixture::with_code(&design, "E0401").is_empty() && ClassifyFixture::with_code(&design, "E0161").is_empty(), "{:?}", design.diagnostics());
}

#[test]
fn behavior_for_module_in_other_source_is_e0401() {
    let top = "mod b;\nuse b::M;\nanalog for M { }\nanalog for b::M { }\n";
    let design = Design::builder().source("top.msl", top).source("b.msl", "pub module M() {}\n").build();
    let errors = ClassifyFixture::with_code(&design, "E0401");
    let at: Vec<_> = errors.iter().map(|d| ClassifyFixture::primary_text(&design, d)).collect();
    assert_eq!(at, ["M", "b::M"]);
    // The module is found; it is in the wrong file, so no block is linked.
    let m = design.find("b::M").and_then(|d| d.as_module()).expect("module M in b.msl");
    assert!(m.behaviors().is_empty());
    assert!(design.root().children().into_iter().filter_map(|c| c.as_behavior()).all(|b| b.module().is_none()));
    // Beside its module, in its own file, a block is fine.
    let own = Design::builder().source("top.msl", "mod b;\n").source("b.msl", "pub module M() {}\nanalog for M { }\n").build();
    assert!(ClassifyFixture::with_code(&own, "E0401").is_empty(), "{:?}", own.diagnostics());
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
    let design = ClassifyFixture::with_prelude(src);
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
    let design = ClassifyFixture::with_prelude(src);
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
    let design = ClassifyFixture::with_prelude(src);
    let bundle = |name: &str| design.find(name).and_then(|d| d.as_struct()).unwrap().is_bundle();
    // A cycle with no carrier in it is no bundle.
    assert!(!bundle("Me") && !bundle("A") && !bundle("B"));
    // A cycle with a carrier on it is a bundle all around, whichever member is met first.
    assert!(bundle("C") && bundle("D") && bundle("E"));
}

#[test]
fn named_instance_with_connections_and_params() {
    let src = "\
module Resistor(p: Electrical, n: Electrical, tc: Option<f64>) { r: f64 = 1.0; }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let r1 = Resistor(a, b) { r: 1k };
    let r2 = Resistor(a, b, tc: 0.5);
}
";
    let design = ClassifyFixture::with_prelude(src);
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
    assert_eq!(params[0].name(), "r");
    // The param binding links to the param it names.
    assert_eq!(params[0].node().res, Resolution::Decl(resistor.param("r").unwrap().decl().id()));
    // A named connection links to its port.
    let ExprKind::Call { args, .. } = r1.expr().kind() else { panic!("an instance is a call") };
    assert!(args.iter().all(|a| a.res == Resolution::Unresolved));
    let r2 = top.instance("r2").expect("instance r2");
    let ExprKind::Call { args, .. } = r2.expr().kind() else { panic!("an instance is a call") };
    assert_eq!(args[2].res, Resolution::Decl(resistor.port("tc").unwrap().decl().id()));
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
    let design = ClassifyFixture::with_prelude(src);
    let integ = design.module("Integ").unwrap();
    let instances = integ.instances();
    assert_eq!(instances.len(), 1, "`windup` is a function, not an instance");
    let idt = &instances[0];
    assert!(idt.is_anonymous() && idt.name().is_none());
    assert_eq!(idt.module(), design.module("idt"));
    assert_eq!(idt.declared_in(), integ.analog_blocks()[0].decl());
    let classes = ClassifyFixture::call_classes(&design);
    let class_of = |text: &str| classes.iter().find(|(t, _)| t == text).unwrap_or_else(|| panic!("call `{text}` in {classes:?}")).1;
    assert_eq!(class_of("windup(1.0)"), CallClass::Function);
    assert!(matches!(class_of("idt(1.0, 0.0)"), CallClass::Instance { module: Some(_), via: None }));
    // A built-in module is an instance of no declared module.
    let builtin = ClassifyFixture::with_prelude("module T() {}\nanalog for T { let d = ddt(1.0); }\n");
    assert_eq!(ClassifyFixture::call_classes(&builtin), [("ddt(1.0)".to_string(), CallClass::Instance { module: None, via: None })]);
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
    let design = ClassifyFixture::with_prelude(src);
    let r = design.module("R").unwrap();
    let id_of = |name: &str| design.find(name).unwrap().id();
    let classes = ClassifyFixture::struct_classes(&design);
    let class_of = |text: &str| classes.iter().find(|(t, _)| t == text).unwrap_or_else(|| panic!("`{text}` in {classes:?}")).1;
    assert_eq!(class_of("R { r: 2.0 }"), StructClass::Template(id_of("R")));
    assert_eq!(class_of("Pair { a: s, b: s }"), StructClass::BundleLiteral);
    assert_eq!(class_of("Plain { x: 1.0 }"), StructClass::StructLiteral);
    assert_eq!(class_of("E::V { k: 1 }"), StructClass::StructLiteral);
    // A name that resolves to nothing is classified as nothing (and `E0161` was reported once).
    assert_eq!(class_of("Nope { z: 1 }"), StructClass::Unclassified);
    assert_eq!(ClassifyFixture::with_code(&design, "E0161").len(), 1);
    // The names of a template's fields are its module's params, a literal's are the struct's fields.
    let field_res = |text: &str| -> Vec<Resolution> {
        let e = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text() == text && !e.enclosing().file().krate().is_std()).unwrap();
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
    let design = ClassifyFixture::with_prelude(src);
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
    let classes = ClassifyFixture::call_classes(&design);
    assert!(classes.iter().any(|(t, c)| t == "pick(true)" && *c == CallClass::Function), "{classes:?}");
}

#[test]
fn impl_on_module_is_no_classifier_error() {
    // §3.1.4 (generics phase D): a module type takes `impl`s, so the classifier reports nothing for one (`E0130` left the spec)
    let design = ClassifyFixture::with_prelude("module M() {}\nstruct S {}\nimpl M { pub fn f(self) {} }\nimpl S { pub fn g(self) {} }\n");
    assert!(ClassifyFixture::with_code(&design, "E0130").is_empty(), "{:?}", design.diagnostics());
}

#[test]
fn generation_body_is_not_unrolled() {
    let src = "\
module Cell(a: Bit) {}
module T() {
    for k in 0..1000 { signal s: Bit; let u = Cell(s); }
}
";
    let design = ClassifyFixture::with_prelude(src);
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
    let design = ClassifyFixture::with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let m = design.module("M").unwrap();
    let instances = m.instances();
    assert_eq!(instances.len(), 1);
    assert_eq!(instances[0].module(), Some(m));
    assert_eq!(instances[0].name(), Some("sub"));
}

#[test]
fn unresolved_callee_is_plain_call() {
    let design = ClassifyFixture::with_prelude("module T() { let a = nothere(1); }\n");
    assert_eq!(ClassifyFixture::call_classes(&design), [("nothere(1)".to_string(), CallClass::Function)]);
    // The missing name is reported once, by name resolution; classification adds nothing.
    assert_eq!(PomSource::codes(&design), ["E0161"]);
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
    let design = ClassifyFixture::with_prelude(src);
    let callee_res = |call: &str| -> Resolution {
        let e = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text() == call && !e.enclosing().file().krate().is_std()).unwrap_or_else(|| panic!("call `{call}`"));
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
    let classes = ClassifyFixture::call_classes(&design);
    for call in ["v(a)", "v(b)", "temp(t)", "i(a)", "v(q)", "potential(o)"] {
        assert_eq!(classes.iter().find(|(t, _)| t == call).unwrap().1, CallClass::Probe, "{call}");
    }
    // A name that is no alias is still reported once; an alias is not.
    assert_eq!(PomSource::codes(&design), ["E0161"]);
    assert_eq!(ClassifyFixture::primary_text(&design, &design.diagnostics()[0]), "nothing");
    assert_eq!(classes.iter().find(|(t, _)| t == "nothing(a)").unwrap().1, CallClass::Function);
    // A declaration in scope wins over the alias.
    let shadowed = ClassifyFixture::with_prelude("fn v(x: f64) -> f64 { x }\nmodule M(a: Electrical) { let y = v(1.0); }\n");
    assert!(shadowed.diagnostics().is_empty(), "{:?}", shadowed.diagnostics());
    assert_eq!(ClassifyFixture::call_classes(&shadowed), [("v(1.0)".to_string(), CallClass::Function)]);
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
    let design = ClassifyFixture::with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let cell = design.module("Cell").unwrap();
    let decl = |d: piperine_lang::pom::Decl| Resolution::Decl(d.id());
    assert_eq!(ClassifyFixture::member_res(&design, "c.sn"), decl(cell.signal("sn").unwrap().decl()));
    assert_eq!(ClassifyFixture::member_res(&design, "c.k"), decl(cell.param("k").unwrap().decl()));
    assert_eq!(ClassifyFixture::member_res(&design, "c.q"), decl(cell.named("q").unwrap().decl()));
    // A port is the module's interface: always readable by its parent.
    assert_eq!(ClassifyFixture::member_res(&design, "c.a"), decl(cell.port("a").unwrap().decl()));
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
    let design = ClassifyFixture::with_prelude(src);
    let errors = ClassifyFixture::with_code(&design, "E0343");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    // At the member name, and the member is still linked.
    assert_eq!(ClassifyFixture::primary_text(&design, errors[0]), "hid");
    let cell = design.module("Cell").unwrap();
    assert_eq!(ClassifyFixture::member_res(&design, "c.hid"), Resolution::Decl(cell.signal("hid").unwrap().decl().id()));
    assert_eq!(PomSource::codes(&design), ["E0343"]);
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
    let design = ClassifyFixture::with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let stream = design.find("Stream").and_then(|d| d.as_struct()).unwrap();
    let field = |s: &piperine_lang::pom::Struct, name: &str| Resolution::Decl(s.fields().into_iter().find(|f| f.name() == Some(name)).unwrap().decl().id());
    let valid = field(&stream, "valid");
    let ready = field(&stream, "ready");
    assert_eq!(ClassifyFixture::member_res(&design, "tx.valid"), valid);
    assert_eq!(ClassifyFixture::member_res(&design, "rx.ready"), ready);
    assert_eq!(ClassifyFixture::member_res(&design, "bus.ready"), ready);
    assert_eq!(ClassifyFixture::member_res(&design, "o.s.valid"), valid);
    assert_eq!(ClassifyFixture::member_res(&design, "arr[1].valid"), valid);
    // A nested field is a field of the outer struct first.
    let outer = design.find("Outer").and_then(|d| d.as_struct()).unwrap();
    assert_eq!(ClassifyFixture::member_res(&design, "o.s"), field(&outer, "s"));
    // A function parameter and an annotated local have declared types too.
    assert_eq!(ClassifyFixture::member_res(&design, "h.valid"), valid);
    assert_eq!(ClassifyFixture::member_res(&design, "t.ready"), ready);
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
    let design = ClassifyFixture::with_prelude(src);
    let deferred = Resolution::Deferred(piperine_lang::pom::DeferReason::ReceiverTypeUnknown);
    for text in ["q.thing", "f().bar", "a.other", "g.field"] {
        assert_eq!(ClassifyFixture::member_res(&design, text), deferred, "`{text}`");
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
#[derive(Repr, Default)]
struct Level { v: bool }
impl Level { pub fn lowest(self) -> Level { self } }
nature Wire { value: Level }
impl Resolve<Level> for Wire { fn resolve<const N: usize>(d: [Level; N]) -> Level { Level { v: true } } }
module M(in clk: Bit, p: Electrical, w: Wire, pr: Pair, a: f64) {
    signal s: Bit;
    let r = clk.rise();
    let ch = s.changed();
    let pv = s.prev();
    let ot = s.others();
    let inf = p.inflow();
    let bg = p.big();
    let vd = pr.vd();
    let lo = w.lowest();
    let no = clk.nothere();
    let un = a.rise();
    let tail = pr.a.changed();
}
";
    let design = ClassifyFixture::with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let impl_fn = |owner: &str, name: &str| {
        let items = design.root().children().into_iter().filter_map(|c| c.as_impl()).filter(|i| i.self_ty().text() == owner).flat_map(|i| i.items());
        Resolution::Decl(items.into_iter().find(|f| f.name() == Some(name)).unwrap_or_else(|| panic!("`{name}` in an impl of {owner}")).id())
    };
    let builtin = |name: &str| Resolution::Builtin(piperine_lang::pom::builtin_method(name).unwrap());
    // `rise` is declared by the prelude's `impl Bit`.
    let rise = design.nature("Bit").unwrap().impls().into_iter().flat_map(|i| i.items()).find(|f| f.name() == Some("rise")).unwrap();
    let prelude_rise = Resolution::Decl(rise.id());
    assert_eq!(ClassifyFixture::member_res(&design, "clk.rise()"), prelude_rise);
    // The carrier built-ins come first: a user `impl Bit` cannot take `prev` or `changed`.
    assert_eq!(ClassifyFixture::member_res(&design, "s.changed()"), builtin("changed"));
    assert_eq!(ClassifyFixture::member_res(&design, "s.prev()"), builtin("prev"));
    assert_eq!(ClassifyFixture::member_res(&design, "s.others()"), builtin("others"));
    assert_eq!(ClassifyFixture::member_res(&design, "p.inflow()"), builtin("inflow"));
    // Then the `impl`s of the nature or bundle, then those of the value type.
    assert_eq!(ClassifyFixture::member_res(&design, "p.big()"), impl_fn("Electrical", "big"));
    assert_eq!(ClassifyFixture::member_res(&design, "pr.vd()"), impl_fn("Pair", "vd"));
    assert_eq!(ClassifyFixture::member_res(&design, "w.lowest()"), impl_fn("Level", "lowest"));
    // A carrier field is a carrier: `pr.a.changed()` is the built-in too.
    assert_eq!(ClassifyFixture::member_res(&design, "pr.a.changed()"), builtin("changed"));
    // A known receiver with no such method names nothing; an unknown receiver defers.
    assert_eq!(ClassifyFixture::member_res(&design, "clk.nothere()"), Resolution::Unresolved);
    assert_eq!(ClassifyFixture::member_res(&design, "a.rise()"), Resolution::Deferred(piperine_lang::pom::DeferReason::ReceiverTypeUnknown));
}

/// POM-09 AC 1: a generic is a carrier because its bound resolves to the
/// core `Nature` (POL-03); an unresolved bound is `Unknown`, and another trait
/// that is merely spelled `Nature` is a value bound.
#[test]
fn generic_carrier_follows_the_resolved_bound() {
    let src = "module M<N: Nope>(g: N) {}";
    // No declaration of `Nope` anywhere: the bound is unresolved, nothing is guessed.
    let none = PomSource::build(src);
    assert_eq!(none.module("M").unwrap().port("g").unwrap().kind(), PortKind::Unknown);
    assert!(none.diagnostics().iter().any(|d| d.diagnostic.code == "E0161"));
    // A user `trait Nature` outside the prelude is not the carrier bound.
    let user = ClassifyFixture::with_prelude("mod x { pub trait Nature {} }\nmodule M<N: x::Nature>(g: N) {}");
    assert_eq!(user.module("M").unwrap().port("g").unwrap().kind(), PortKind::ValueInput);
    // The core `Nature` makes a carrier, through arrays too.
    let real = ClassifyFixture::with_prelude("module M<N: Nature>(g: N, ga: [N; 2]) {}");
    let kinds: Vec<_> = real.module("M").unwrap().ports().iter().map(|p| p.kind()).collect();
    assert_eq!(kinds, [PortKind::Carrier, PortKind::Carrier]);
}

/// A name written for a port, a param, a field or a parameter that the callee
/// does not have names nothing: the slot stays `Unresolved` and one diagnostic
/// is reported at the name, under the code the spec assigns (POL-18, D11):
/// `E0320` for a param, `E0140` for a named argument, `E0186` for a struct field.
#[test]
fn named_binding_without_a_target_reports_the_spec_code() {
    let src = "\
struct Plain { x: f64 }
fn f(a: f64) -> f64 { a }
module R(p: Electrical, n: Electrical, k: Option<f64>) { r: f64 = 1.0; }
module Top() {
    signal s: Electrical;
    let ok = R(s, s, k: 1.0) { r: 2.0 };
    let bad = R(s, s, q: s) { nope: 1.0 };
    let pl = Plain { x: 1.0, y: 2.0 };
    let t = R { zz: 3.0 };
    let v = f(b: 1.0);
}
";
    let design = ClassifyFixture::with_prelude(src);
    let found: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(found, [("E0140", "q"), ("E0320", "nope"), ("E0186", "y"), ("E0320", "zz"), ("E0140", "b")], "{:?}", design.diagnostics());
    assert!(design.diagnostics().iter().all(|d| d.diagnostic.message.contains("no ")), "{:?}", design.diagnostics());
    // The names that do exist link, the ones that do not stay unresolved.
    let call = |text: &str| (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text().starts_with(text)).unwrap();
    let ok = call("R(s, s, k: 1.0)");
    let ExprKind::Call { args, .. } = ok.kind() else { panic!("a call") };
    assert!(matches!(args[2].res, Resolution::Decl(_)));
    let bad = call("R(s, s, q: s)");
    let ExprKind::Call { args, params, .. } = bad.kind() else { panic!("a call") };
    assert_eq!(args[2].res, Resolution::Unresolved);
    assert_eq!(params.as_ref().unwrap()[0].res, Resolution::Unresolved);
}

/// POL-18 AC 1: a param block names no param of the module, or names one twice:
/// one `E0320` at the name, no `E0161`; the first of two equal names still links.
#[test]
fn param_block_errors_are_e0320() {
    let src = "\
module R(p: Electrical, n: Electrical) { r: f64 = 1.0; }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let ok = R(a, b) { r: 3.0 };
    let x = R(a, b) { zz: 1.0 };
    let y = R(a, b) { r: 1.0, r: 2.0 };
    let t = R { zz: 3.0 };
    let u = R { r: 1.0, r: 2.0 };
}
";
    let design = ClassifyFixture::with_prelude(src);
    let found: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(found, [("E0320", "zz"), ("E0320", "r"), ("E0320", "zz"), ("E0320", "r")], "{:?}", design.diagnostics());
    // A repeated name is reported at its second occurrence.
    let second = src.find("r: 1.0, r: 2.0").unwrap() + "r: 1.0, ".len();
    assert_eq!(design.diagnostics()[1].diagnostic.span().range().start, second);
    let call = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text().starts_with("R(a, b) { r: 1.0, r: 2.0 }")).unwrap();
    let ExprKind::Call { params, .. } = call.kind() else { panic!("a call") };
    let params = params.as_ref().unwrap();
    assert!(matches!(params[0].res, Resolution::Decl(_)));
    assert_eq!(params[1].res, Resolution::Unresolved);
}

/// POL-18 AC 2: a named argument that names no parameter written `Option<..>`,
/// or one an earlier argument bound, is one `E0140` at the name, no `E0161`.
#[test]
fn named_argument_errors_are_e0140() {
    let src = "\
fn g(a: f64, b: Option<f64>, c: Option<f64>) -> f64 { a }
module R(p: Electrical, n: Electrical, k: Option<f64>) { }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let ok1 = g(1.0, c: 2.0);
    let ok2 = g(1.0, c: 2.0, b: 3.0);
    let ok3 = R(a, b, k: 1.0);
    let e1 = g(1.0, d: 3.0);
    let e2 = g(1.0, a: 2.0);
    let e3 = g(1.0, 2.0, b: 3.0);
    let e4 = g(1.0, c: 1.0, c: 2.0);
    let e5 = R(a, b, q: b);
    let e6 = R(a, b, p: a);
}
";
    let design = ClassifyFixture::with_prelude(src);
    let found: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    let want = [("E0140", "d"), ("E0140", "a"), ("E0140", "b"), ("E0140", "c"), ("E0140", "q"), ("E0140", "p")];
    assert_eq!(found, want, "{:?}", design.diagnostics());
    let args_of = |text: &str| {
        let e = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text().starts_with(text)).unwrap();
        let ExprKind::Call { args, .. } = e.kind() else { panic!("a call") };
        args.iter().map(|a| a.res).collect::<Vec<_>>()
    };
    assert!(matches!(args_of("g(1.0, c: 2.0, b: 3.0)")[2], Resolution::Decl(_)));
    let e4 = args_of("g(1.0, c: 1.0, c: 2.0)");
    assert!(matches!(e4[1], Resolution::Decl(_)));
    assert_eq!(e4[2], Resolution::Unresolved);
    assert_eq!(args_of("g(1.0, 2.0, b: 3.0)")[2], Resolution::Unresolved);
}

/// POL-18 AC 3: a bundle literal that names a field the bundle lacks, or one
/// twice, is one `E0263` at the name, no `E0161`; the first of two equal names links.
#[test]
fn bundle_literal_field_errors_are_e0263() {
    let src = "\
struct Port2 { hi: Electrical, lo: Electrical }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let ok = Port2 { hi: a, lo: b };
    let x = Port2 { hi: a, lo: b, zz: 1.0 };
    let y = Port2 { hi: a, hi: b, lo: b };
}
";
    let design = ClassifyFixture::with_prelude(src);
    let found: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(found, [("E0263", "zz"), ("E0263", "hi")], "{:?}", design.diagnostics());
    let second = src.find("hi: a, hi: b").unwrap() + "hi: a, ".len();
    assert_eq!(design.diagnostics()[1].diagnostic.span().range().start, second);
    let e = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text().starts_with("Port2 { hi: a, hi: b")).unwrap();
    let ExprKind::Struct { fields, class, .. } = e.kind() else { panic!("a struct expression") };
    assert_eq!(*class, StructClass::BundleLiteral);
    assert!(matches!(fields[0].res, Resolution::Decl(_)));
    assert_eq!(fields[1].res, Resolution::Unresolved);
}

/// POL-18 AC 4: an unknown field of a struct literal and of a struct pattern is
/// one `E0186` at the name; the shorthand pattern `S { a }` still binds and links.
#[test]
fn unknown_struct_field_is_reported_in_literals_and_patterns() {
    let src = "\
struct S { a: f64, b: f64 }
fn h(s: S) -> f64 {
    let lit = S { a: 1.0, c: 2.0 };
    match s {
        S { a, q } => a,
        S { b: bb, .. } => bb,
    }
}
";
    let design = ClassifyFixture::with_prelude(src);
    let found: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(found, [("E0186", "c"), ("E0186", "q")], "{:?}", design.diagnostics());
    let decl_s = design.find("S").expect("struct S");
    let field = |name: &str| Resolution::Decl(decl_s.children().into_iter().find(|f| f.name().as_deref() == Some(name)).unwrap().id());
    let mut seen = Vec::new();
    for i in 0..design.pat_count() {
        if let piperine_lang::pom::PatKind::Struct { fields, .. } = &design.pat_node(piperine_lang::pom::PatId(i as u32)).kind {
            seen.extend(fields.iter().map(|f| (f.name.text.clone(), f.res)));
        }
    }
    assert_eq!(seen, [("a".to_string(), field("a")), ("q".to_string(), Resolution::Unresolved), ("b".to_string(), field("b"))]);
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
    let design = ClassifyFixture::with_prelude(src);
    let errors = ClassifyFixture::with_code(&design, "E0343");
    assert_eq!(errors.len(), 1, "{:?}", design.diagnostics());
    assert_eq!(ClassifyFixture::primary_text(&design, errors[0]), "nope");
    assert!(errors[0].diagnostic.message.contains("Cell"), "{:?}", errors[0]);
    assert_eq!(ClassifyFixture::member_res(&design, "c.nope"), Resolution::Unresolved);
    assert_eq!(PomSource::codes(&design), ["E0343"]);
}

#[test]
fn shorthand_field_binds_param_and_reads_the_local() {
    // POL-11 AC 3: `R(a, b) { r }` binds the param `r` of `R`, its value reads
    // the `r` in scope, the field is a shorthand, and to_ast restores it.
    let src = "\
module R(p: Electrical, n: Electrical) { r: f64 = 1.0; }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    r: f64 = 2.0;
    let x = R(a, b) { r };
}
";
    let design = ClassifyFixture::with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let top = design.module("Top").unwrap();
    let inst = &top.instances()[0];
    let fields = inst.param_bindings();
    assert_eq!(fields.len(), 1);
    assert!(fields[0].is_shorthand());
    assert_eq!(fields[0].node().res, Resolution::Decl(design.module("R").unwrap().param("r").unwrap().decl().id()));
    let ExprKind::Path(p) = fields[0].value().kind() else { panic!("a path value") };
    assert_eq!(design.path(*p).segments()[0].res, Resolution::Decl(top.param("r").unwrap().decl().id()));
    let parsed = piperine_lang::msl::parser::SourceParser::parse(src).file;
    let files = design.files();
    let top_file = files.iter().find(|f| !f.krate().is_std()).expect("the root file");
    assert_eq!(top_file.to_ast(), parsed);
}

/// POL-03 AC 3: `Nature` is a core name, so a generic bounded by it is a
/// carrier with no prelude registered and no diagnostic.
#[test]
fn nature_bound_is_builtin_and_classifies_the_port() {
    let design = PomSource::build("module M<N: Nature>(g: N, ga: [N; 2]) {}");
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let kinds: Vec<_> = design.module("M").unwrap().ports().iter().map(|p| p.kind()).collect();
    assert_eq!(kinds, [PortKind::Carrier, PortKind::Carrier]);
    let bound = (0..design.path_count())
        .map(|i| design.path(piperine_lang::pom::PathId(i as u32)))
        .find(|p| p.segments().iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>() == ["Nature"])
        .expect("the bound path");
    assert!(matches!(bound.segments()[0].res, Resolution::Builtin(b) if b.info().kind == piperine_lang::pom::BuiltinKind::Trait));
}

/// POL-05 AC 3: on `E0403` each name of the block links to the module's
/// generic of the same name, never the one at the same position.
#[test]
fn mismatched_behavior_names_bind_by_name() {
    let src = "\
module M<T: Repr, const N: u32>() {}
digital<N, T> for M<N, T> { on at(1.0) { let a = N; let b = T::WIDTH; let c = Other; } }
";
    let design = PomSource::build(src);
    assert_eq!(ClassifyFixture::with_code(&design, "E0403").len(), 1, "{:?}", design.diagnostics());
    let generic = |name: &str| {
        let m = design.module("M").unwrap().decl();
        m.children().into_iter().find(|c| c.name() == Some(name)).unwrap_or_else(|| panic!("generic {name}")).id()
    };
    let read = |joined: &str| -> Vec<Resolution> {
        (0..design.path_count())
            .map(|i| design.path(piperine_lang::pom::PathId(i as u32)))
            .filter(|p| p.segments().iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::") == joined)
            .flat_map(|p| p.segments().iter().map(|s| s.res).collect::<Vec<_>>())
            .collect()
    };
    // The paths written in the block: `N` (the const), `T::WIDTH`.
    assert!(read("N").contains(&Resolution::Decl(generic("N"))), "{:?}", read("N"));
    assert_eq!(read("T::WIDTH")[0], Resolution::Decl(generic("T")));
    // The block's own name list and its target's arguments are the other `N` and `T` paths; none names the other generic.
    assert!(!read("N").contains(&Resolution::Decl(generic("T"))), "{:?}", read("N"));
    assert!(!read("T").contains(&Resolution::Decl(generic("N"))), "{:?}", read("T"));
    // A name the module has no generic of stays unresolved, and E0403 already said why.
    assert_eq!(read("Other"), [Resolution::Unresolved]);
    assert!(ClassifyFixture::with_code(&design, "E0161").is_empty());
}

/// POL-05 AC 4 (D13): a block binds to its module from any namespace of the
/// module's own file.
#[test]
fn behavior_binds_across_namespaces_of_one_file() {
    let src = "\
mod a { pub module M() {} }
analog for a::M { }
mod b { analog for super::a::M { } }
";
    let design = PomSource::build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let m = design.find("a::M").and_then(|d| d.as_module()).unwrap();
    assert_eq!(m.behaviors().len(), 2);
}

/// POL-18 AC 5: an `extern` module lists params only, has no result and no
/// value input: one `E0370` for each offender, at the item, the result type or
/// the port.
#[test]
fn extern_module_shape_is_e0370() {
    let src = "\
extern \"osdi\" module Fine(p: Electrical, q: Electrical) { r: f64 = 1.0; }
extern \"cosim\" module Bad(in a: f64, p: Electrical) -> f64 {
    k: f64 = 1.0;
    signal s: Electrical;
    let w = Fine(p, p);
}
module Plain(in a: f64, p: Electrical) -> f64 { signal s: Electrical; }
";
    let design = ClassifyFixture::with_prelude(src);
    let at: Vec<(&str, &str)> = design.diagnostics().iter().filter(|d| d.diagnostic.code == "E0370").map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    let texts: Vec<&str> = at.iter().map(|(_, t)| *t).collect();
    assert_eq!(texts.len(), 4, "{:?}", design.diagnostics());
    // In source order: the value input, the result type, then each body item.
    assert!(texts[0].starts_with("in a") && texts[0].contains("f64"), "{texts:?}");
    assert_eq!(texts[1], "f64");
    assert!(texts[2].starts_with("signal s"), "{texts:?}");
    assert!(texts[3].starts_with("let w"), "{texts:?}");
    // The result type is the one after `->`.
    assert_eq!(design.diagnostics().iter().filter(|d| d.diagnostic.code == "E0370").nth(1).unwrap().diagnostic.span().range().start, src.find("-> f64").unwrap() + 3);
    // Nothing else is reported: a plain module may have all of it.
    assert_eq!(PomSource::codes(&design), ["E0370"; 4], "{:?}", design.diagnostics());
}

/// POL-18 AC 6: an `extern "C" fn` with a body, or a function that is neither
/// `extern` nor a trait item without one, is one `E0125` at the function's name.
#[test]
fn function_body_rule_is_e0125() {
    let src = "\
extern \"C\" fn with_body(x: f64) -> f64 { x }
fn without_body(x: f64) -> f64;
extern \"C\" fn fine_extern(x: f64) -> f64;
fn fine(x: f64) -> f64 { x }
trait Tr { fn in_trait(self) -> f64; }
struct S { a: f64 }
impl Tr for S { fn in_impl(self) -> f64; }
";
    let design = ClassifyFixture::with_prelude(src);
    let at: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(at, [("E0125", "with_body"), ("E0125", "without_body"), ("E0125", "in_impl")], "{:?}", design.diagnostics());
    // The message states the fault, not the rule read backwards.
    let messages: Vec<&str> = design.diagnostics().iter().map(|d| d.diagnostic.message.as_ref()).collect();
    assert!(messages[0].contains("cannot have a body"), "{messages:?}");
    for m in &messages[1..] {
        assert!(m.contains("has no body") && m.contains("trait item"), "{m}");
    }
}

/// POL-18 AC 7: a nature that repeats a field, mixes discrete and continuous
/// fields, or has neither `value` nor `potential` is one `E0201` at its name
/// and `kind() == NatureKind::Invalid`.
#[test]
fn nature_field_errors_are_e0201() {
    use piperine_lang::pom::NatureKind;
    let src = "\
nature Good { potential: P { access: pv, units: \"V\", abstol: 1u }, flow: F { access: pi, units: \"A\", abstol: 1p } }
nature Rep { potential: P2 { access: v2, units: \"V\", abstol: 1u }, potential: P3 { access: v3, units: \"V\", abstol: 1u } }
nature Mix { potential: P4 { access: v4, units: \"V\", abstol: 1u }, value: f64 }
nature Nil { flow: F5 { access: i5, units: \"A\", abstol: 1p } }
nature Disc { value: f64 }
";
    let design = PomSource::build(src);
    let at: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(at, [("E0201", "Rep"), ("E0201", "Mix"), ("E0201", "Nil")], "{:?}", design.diagnostics());
    let kind = |n: &str| design.nature(n).unwrap().kind();
    assert_eq!(kind("Good"), NatureKind::Conservative);
    assert_eq!(kind("Disc"), NatureKind::Discrete);
    for bad in ["Rep", "Mix", "Nil"] {
        assert_eq!(kind(bad), NatureKind::Invalid, "{bad}");
    }
}

/// POL-19 AC 1: the first argument of an access call has a declared nature
/// that declares no access of that name: the callee stays `Unresolved`, the
/// call is not a `Probe`, and one `E0206` is reported at the callee.
#[test]
fn access_of_another_nature_is_e0206() {
    let src = "\
nature El { potential: V1 { access: v }, flow: I1 { access: i } }
nature Th { potential: V2 { access: v }, flow: P2 { access: pwr } }
module M(a: El, t: Th) {}
analog for M {
    let x = i(t);
    let y = v(t);
    let z = i(a);
    let w = pwr(t);
}
";
    let design = PomSource::build(src);
    let at: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(at, [("E0206", "i")], "{:?}", design.diagnostics());
    assert_eq!(design.diagnostics()[0].diagnostic.span().range().start, src.find("i(t)").unwrap());
    let callee_res = |call: &str| -> Resolution {
        let e = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text() == call).unwrap();
        let ExprKind::Call { callee, class, .. } = e.kind() else { panic!("a call") };
        let ExprKind::Path(p) = design.expr(*callee).kind() else { panic!("a path callee") };
        assert_eq!(*class == CallClass::Probe, call != "i(t)", "{call}");
        design.path(*p).segments()[0].res
    };
    assert_eq!(callee_res("i(t)"), Resolution::Unresolved);
    for call in ["v(t)", "i(a)", "pwr(t)"] {
        assert!(matches!(callee_res(call), Resolution::Decl(_)), "{call}");
    }
}

/// POL-19 AC 2: a carrier method called on an array of carriers is `E0211`
/// and stays `Unresolved`; indexed to an element it resolves to the `impl`
/// function; a name that is not a carrier method defers with no diagnostic.
#[test]
fn carrier_method_on_an_array_is_e0211() {
    let src = "\
module M(arr: [Bit; 2], one: Bit, grid: [[Bit; 2]; 2]) {
    let a = arr.rise();
    let b = arr[0].rise();
    let c = arr.len();
    let d = grid[1].rise();
    let e = grid[1][0].rise();
    let f = one.rise();
    let g = arr.changed();
}
";
    let design = ClassifyFixture::with_prelude(src);
    let at: Vec<(&str, &str)> = design.diagnostics().iter().map(|d| (d.diagnostic.code.as_ref(), ClassifyFixture::primary_text(&design, d))).collect();
    assert_eq!(at, [("E0211", "rise"), ("E0211", "rise"), ("E0211", "changed")], "{:?}", design.diagnostics());
    let starts: Vec<usize> = design.diagnostics().iter().map(|d| d.diagnostic.span().range().start).collect();
    assert_eq!(starts, [src.find("arr.rise").unwrap() + 4, src.find("grid[1].rise").unwrap() + 8, src.find("arr.changed").unwrap() + 4]);
    assert_eq!(ClassifyFixture::member_res(&design, "arr.rise()"), Resolution::Unresolved);
    assert_eq!(ClassifyFixture::member_res(&design, "grid[1].rise()"), Resolution::Unresolved);
    assert_eq!(ClassifyFixture::member_res(&design, "arr.len()"), Resolution::Deferred(piperine_lang::pom::DeferReason::ReceiverTypeUnknown));
    for call in ["arr[0].rise()", "grid[1][0].rise()", "one.rise()"] {
        assert!(matches!(ClassifyFixture::member_res(&design, call), Resolution::Decl(_)), "{call}");
    }
    assert!(matches!(ClassifyFixture::member_res(&design, "arr[0].rise()"), Resolution::Decl(d) if design.decl_data(d).name.as_ref().unwrap().text == "rise"));
}

#[test]
fn arg_and_field_handles_expose_name_value_target() {
    let src = "\
module Resistor(p: Electrical, n: Electrical, tc: Option<f64>) { r: f64 = 1.0; }
struct P { x: f64, y: f64 }
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    r: f64 = 2.0;
    let r1 = Resistor(a, b, tc: 0.5) { r };
    let r2 = Resistor(a, b) { r: 3.0 };
    let p = P { x: 1.0, y: 2.0 };
}
";
    let design = ClassifyFixture::with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let top = design.module("Top").unwrap();
    let resistor = design.module("Resistor").unwrap();
    let r1 = top.instance("r1").unwrap();
    let r2 = top.instance("r2").unwrap();

    // Arguments: positional ones have no name and no target; a named one targets its port.
    let args = r1.expr().args();
    assert_eq!(args.iter().map(|a| a.name()).collect::<Vec<_>>(), [None, None, Some("tc")]);
    assert_eq!(args.iter().map(|a| a.value().text()).collect::<Vec<_>>(), ["a", "b", "0.5"]);
    assert_eq!(args[0].target(), None);
    assert_eq!(args[2].target(), Some(resistor.port("tc").unwrap().decl()));

    // Param bindings: handles, with the shorthand kept and the param as the target.
    let shorthand = r1.param_bindings();
    assert_eq!(shorthand.len(), 1);
    assert_eq!(shorthand[0].name(), "r");
    assert!(shorthand[0].is_shorthand());
    assert_eq!(shorthand[0].value().text(), "r");
    assert_eq!(shorthand[0].target(), Some(resistor.param("r").unwrap().decl()));
    let full = r2.param_bindings();
    assert!(!full[0].is_shorthand());
    assert_eq!(full[0].value().text(), "3.0");
    assert_eq!(full[0].target(), Some(resistor.param("r").unwrap().decl()));
    assert_eq!(r1.expr().fields().len(), 1, "an instance's param block is the call's fields");

    // A struct literal's fields target the struct's fields.
    let lit = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text() == "P { x: 1.0, y: 2.0 }").unwrap();
    let fields = lit.fields();
    assert_eq!(fields.iter().map(|f| f.name()).collect::<Vec<_>>(), ["x", "y"]);
    let p = design.struct_named("P").unwrap();
    assert_eq!(fields[1].target(), Some(p.fields()[1].decl()));
    // Anything else has none.
    assert!(lit.args().is_empty());
    assert!(r1.callee().args().is_empty() && r1.callee().fields().is_empty());
    // A name the callee does not declare targets nothing.
    let design = ClassifyFixture::with_prelude("module R(p: Electrical) {}\nmodule T() { signal a: Electrical; let x = R(a, zz: a); }\n");
    let x = design.module("T").unwrap().instance("x").unwrap();
    assert_eq!(x.expr().args()[1].target(), None);
}

#[test]
fn builtin_instance_names_its_primitive() {
    let design = ClassifyFixture::with_prelude("module T() { let c = cross(1.0); let d = ddt(1.0); }\nmodule U() { signal a: Electrical; }\nmodule V() { let u = U(); }\n");
    let top = design.module("T").unwrap();
    let c = top.instance("c").unwrap();
    assert_eq!(c.builtin().map(|b| b.info().name), Some("cross"));
    assert_eq!(top.instance("d").unwrap().builtin().map(|b| b.info().name), Some("ddt"));
    assert!(c.module().is_none());
    let user = design.module("V").unwrap().instance("u").unwrap();
    assert_eq!(user.module(), design.module("U"));
    assert_eq!(user.builtin(), None);
}

