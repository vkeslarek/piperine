//! Scope: the reflection queries over a resolved design (POM-11): the lookups
//! and casts, the carrier nature of a port or signal, the instance views and
//! the reverse queries.

#[path = "support/spec_corpus.rs"]
mod spec_corpus;

use piperine_lang::pom::{Decl, Design, ExprId, PatId, Reference, Reflect, StmtId, TypeId, Writer};
use serde::{Serialize, de::DeserializeOwned};
use spec_corpus::{Class, corpus};

/// A design of `src` over the embedded standard library.
fn with_prelude(src: &str) -> Design {
    Design::builder().source("top.msl", src).build()
}

#[test]
fn carrier_nature_through_array_and_flip() {
    let src = "\
module M(a: Electrical, b: [Bit; 4], c: flip Electrical, d: [flip Bit; 2], e: f64, f: Nope) {
    signal s: Electrical;
    signal t: [[Bit; 2]; 3];
    signal u: u8;
}
";
    let design = with_prelude(src);
    let m = design.module("M").unwrap();
    let nature = |name: &str| design.nature(name).unwrap();
    let port = |name: &str| m.port(name).unwrap().nature();
    assert_eq!(port("a"), Some(nature("Electrical")));
    assert_eq!(port("b"), Some(nature("Bit")));
    assert_eq!(port("c"), Some(nature("Electrical")));
    assert_eq!(port("d"), Some(nature("Bit")));
    assert_eq!(port("e"), None);
    assert_eq!(port("f"), None);
    assert_eq!(m.signal("s").unwrap().nature(), Some(nature("Electrical")));
    assert_eq!(m.signal("t").unwrap().nature(), Some(nature("Bit")));
    assert_eq!(m.signal("u").unwrap().nature(), None);
}

#[test]
fn nature_impls_lists_its_impl_blocks() {
    let src = "impl Electrical { pub fn one(self) -> f64 { 1.0 } }\nimpl Electrical { pub fn two(self) -> f64 { 2.0 } }\n";
    let design = with_prelude(src);
    assert_eq!(design.nature("Electrical").unwrap().impls().len(), 2);
    // `std` writes `impl Bit` twice: the edges of §22.1.5 and `high`, which leaves with the flag methods (explicit-unknowns T29)
    assert_eq!(design.nature("Bit").unwrap().impls().len(), 2);
    assert!(design.nature("Wire").unwrap().impls().is_empty());
}

const NAMES: &str = "\
mod a {
    pub struct P { x: f64 }
    pub fn helper() -> f64 { 1.0 }
    module M(x: Bit, y: Bit) {
        signal w: Bit;
        if true { signal g: Bit; }
    }
    mod b { pub fn deep() -> f64 { 2.0 } }
}
module Top(p: Bit) { signal s: Bit; }
fn top_level() -> f64 { 3.0 }
struct Q { y: f64 }
";

#[test]
fn lookups_by_name_and_path() {
    let design = with_prelude(NAMES);
    assert_eq!(design.module("M").unwrap().name(), Some("M"));
    assert_eq!(design.module("Top").unwrap().name(), Some("Top"));
    assert!(design.module("Nope").is_none());
    assert!(design.module("a").is_none(), "a namespace is no module");
    assert_eq!(design.nature("Electrical").unwrap().name(), Some("Electrical"));
    assert!(design.nature("M").is_none());
    assert_eq!(design.find("a::M").unwrap().as_module(), design.module("M"));
    assert_eq!(design.find("a::b::deep").unwrap().name(), Some("deep"));
    assert!(design.find("a::nope").is_none());
    assert!(design.find("nope").is_none());
    let names = |v: Vec<Option<&str>>| v.into_iter().flatten().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(names(design.modules().iter().filter(|m| !m.file().krate().is_std()).map(|m| m.name()).collect()), ["M", "Top"]);
    let functions: Vec<_> = design.functions().iter().filter_map(|f| f.name()).collect();
    assert_eq!(functions, ["helper", "deep", "top_level"], "non-prelude free functions in source order");
    let structs: Vec<_> = design.structs().iter().filter_map(|s| s.name()).collect();
    assert_eq!(structs, ["P", "Q"]);
    let spaces: Vec<_> = design.namespaces().iter().filter_map(|n| n.name()).collect();
    assert_eq!(spaces, ["a", "b"], "named namespaces; file roots are unnamed");
}

/// A by-name lookup prefers the root crate's declaration over one in `std`.
#[test]
fn module_lookup_prefers_authored_files() {
    // The prelude declares `idt`; a design module of that name wins, else the prelude's is found.
    let design = with_prelude("module idt() {}\n");
    assert!(!design.module("idt").unwrap().file().krate().is_std());
    let design = with_prelude("module Top() {}\n");
    assert!(design.module("idt").unwrap().file().krate().is_std());
}

#[test]
fn qualified_names() {
    let design = with_prelude(NAMES);
    let q = |d: Decl<'_>| d.qualified_name();
    assert_eq!(q(design.module("M").unwrap().decl()), "a::M");
    assert_eq!(q(design.module("M").unwrap().signal("w").unwrap().decl()), "a::M.w");
    assert_eq!(q(design.module("M").unwrap().port("y").unwrap().decl()), "a::M.y");
    assert_eq!(q(design.module("Top").unwrap().decl()), "Top");
    assert_eq!(q(design.find("a::b::deep").unwrap()), "a::b::deep");
    assert_eq!(q(design.find("a::b").unwrap()), "a::b");
    assert_eq!(q(design.root()), "");
    // A generation adds no segment.
    let g = design.module("M").unwrap().member("g");
    let g = g.or_else(|| design.module("M").unwrap().items().into_iter().find(|d| d.name() == Some("g")));
    let g = g.or_else(|| design.module("M").unwrap().signals().into_iter().find(|s| s.name() == Some("g")).map(|s| s.decl())).expect("signal g");
    assert_eq!(q(g), "a::M.g");
    assert_eq!(design.module("M").unwrap().qualified_name(), "a::M");
}

#[test]
fn casts_match_kind_only() {
    let design = with_prelude(NAMES);
    let count = |d: Decl<'_>| {
        [
            d.as_namespace().is_some(),
            d.as_use().is_some(),
            d.as_const().is_some(),
            d.as_function().is_some(),
            d.as_fn_param().is_some(),
            d.as_struct().is_some(),
            d.as_field().is_some(),
            d.as_enum().is_some(),
            d.as_variant().is_some(),
            d.as_trait().is_some(),
            d.as_impl().is_some(),
            d.as_generic_param().is_some(),
            d.as_nature().is_some(),
            d.as_quantity().is_some(),
            d.as_module().is_some(),
            d.as_port().is_some(),
            d.as_param().is_some(),
            d.as_signal().is_some(),
            d.as_let().is_some(),
            d.as_binding().is_some(),
            d.as_definition().is_some(),
            d.as_generation().is_some(),
            d.as_behavior().is_some(),
            d.as_reaction().is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count()
    };
    let mut seen = 0;
    for d in design.decls() {
        assert_eq!(count(d), 1, "{d:?} answers exactly one cast");
        seen += 1;
    }
    assert!(seen > 20);
    assert!(design.find("a::M").unwrap().as_signal().is_none());
    assert!(design.find("a::M").unwrap().as_module().is_some());
    assert!(design.module("M").unwrap().signal("w").unwrap().decl().as_module().is_none());
}

#[test]
fn instance_connection_by_port() {
    let src = "\
module Resistor(p: Electrical, n: Electrical, tc: Option<f64>) {}
module Top() {
    signal a: Electrical;
    signal b: Electrical;
    let r1 = Resistor(a, b);
    let r2 = Resistor(a, b, tc: 0.5);
    let r3 = Unknown(a, b);
}
";
    let design = with_prelude(src);
    let res = design.module("Resistor").unwrap();
    let top = design.module("Top").unwrap();
    let (p, n) = (res.port("p").unwrap(), res.port("n").unwrap());
    let r1 = top.instance("r1").unwrap();
    assert_eq!(r1.connection(p).unwrap().text(), "a");
    assert_eq!(r1.connection(n).unwrap().text(), "b");
    let r2 = top.instance("r2").unwrap();
    assert_eq!(r2.connection(p).unwrap().text(), "a");
    assert_eq!(r2.connection(n).unwrap().text(), "b");
    assert_eq!(r2.connection(res.port("tc").unwrap()).unwrap().text(), "0.5");
    // A port of another module, and an instance whose module is unknown, give none.
    let other = design.module("Top").unwrap();
    assert!(other.ports().is_empty());
    let q = design.module("Resistor").unwrap().port("p").unwrap();
    let r3 = top.instance("r3");
    assert!(r3.is_none_or(|r| r.connection(q).is_none()));
}

#[test]
fn handle_equality() {
    let a = with_prelude(NAMES);
    let b = with_prelude(NAMES);
    assert_eq!(a.module("M").unwrap(), a.module("M").unwrap());
    assert_ne!(a.module("M").unwrap(), b.module("M").unwrap(), "same node of another design");
    assert_ne!(a.module("M").unwrap().decl(), a.module("Top").unwrap().decl());
    assert_eq!(a.module("M").unwrap().decl(), a.find("a::M").unwrap());
    assert_eq!(a.module("M").unwrap().signal("w").unwrap(), a.module("M").unwrap().signal("w").unwrap());
}

const REVERSE: &str = "\
module Cell(p: Bit) {}
module Top(a: Bit, b: Bit) {
    signal s: Bit;
    signal t: Bit;
    s = a;
    let u = Cell(s);
    let v = Cell(s);
    t = b & s;
}
digital for Top {
    on a { s <- true; t[0] <- false; }
    on b { (s) <- false; }
}
";

#[test]
fn references_in_source_order() {
    let design = with_prelude(REVERSE);
    let top = design.module("Top").unwrap();
    let s = top.signal("s").unwrap();
    let refs = s.references();
    let texts: Vec<_> = refs
        .iter()
        .map(|r| match r {
            Reference::Path(p) => (p.text().to_string(), p.span().start),
            Reference::Member(e) => (e.text().to_string(), e.span().start),
            Reference::Pattern(p) => (p.text().to_string(), p.span().start),
        })
        .collect();
    // `s = a;`, `Cell(s)` twice, `b & s`, `s <- true`, `(s) <- false`.
    assert_eq!(texts.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(), ["s", "s", "s", "s", "s", "s"]);
    assert!(texts.windows(2).all(|w| w[0].1 < w[1].1), "source order: {texts:?}");
    // A reference through `decl()` handle is the same list.
    assert_eq!(s.decl().references(), refs);
    // A declaration nothing names has none.
    assert!(design.module("Cell").unwrap().port("p").unwrap().references().is_empty());
    // A module is referenced by the callee of each placement.
    assert_eq!(design.module("Cell").unwrap().references().len(), 2);
}

#[test]
fn member_references_are_listed() {
    let src = "\
module Cell(p: Bit) { pub signal q: Bit; }
module Top() {
    signal s: Bit;
    let c = Cell(s);
    signal t: Bit;
    t = c.q;
}
";
    let design = with_prelude(src);
    let q = design.module("Cell").unwrap().signal("q").unwrap();
    let refs = q.references();
    assert_eq!(refs.len(), 1);
    assert!(matches!(refs[0], Reference::Member(e) if e.text() == "c.q"));
}

#[test]
fn writers_are_definitions_and_commits() {
    let design = with_prelude(REVERSE);
    let top = design.module("Top").unwrap();
    let s = top.signal("s").unwrap();
    let t = top.signal("t").unwrap();
    let writers = s.writers();
    assert_eq!(writers.len(), 3, "{writers:?}");
    assert!(matches!(writers[0], Writer::Definition(d) if d.place().text() == "s"));
    assert!(matches!(writers[1], Writer::Commit(c) if c.place().text() == "s"));
    assert!(matches!(writers[2], Writer::Commit(c) if c.place().text() == "(s)"), "Paren is stripped");
    // `t = ..` is a definition; `t[0] <- ..` has `t` as the root of an index place.
    let tw = t.writers();
    assert_eq!(tw.len(), 2, "{tw:?}");
    assert!(matches!(tw[0], Writer::Definition(_)));
    assert!(matches!(tw[1], Writer::Commit(c) if c.place().text() == "t[0]"));
    // Reads are not writes: `a` and `b` are only read.
    assert!(top.port("a").unwrap().writers().is_empty());
}

#[test]
fn port_writers_and_field_roots() {
    let src = "\
struct Bus { valid: Bit }
module M(o: Bit, bus: Bus) {
    o = true;
    bus.valid = false;
}
";
    let design = with_prelude(src);
    let m = design.module("M").unwrap();
    assert_eq!(m.port("o").unwrap().writers().len(), 1);
    assert_eq!(m.port("bus").unwrap().writers().len(), 1, "a field place is rooted at its carrier");
}

/// POL-19 AC 3: a parent's `c.w = ..` and `c.w <- ..` are writers of `C.w`.
#[test]
fn parent_drive_is_a_writer_of_the_child_carrier() {
    let src = "\
module C(a: Bit) { pub signal w: Bit; }
module P(a: Bit) {
    let c = C(a);
    c.w = true;
}
digital for P {
    on a.rise() { c.w <- false; }
}
";
    let design = with_prelude(src);
    let child = design.module("C").unwrap();
    let writers = child.signal("w").unwrap().writers();
    assert_eq!(writers.len(), 2, "{writers:?}");
    assert!(matches!(writers[0], Writer::Definition(d) if d.place().text() == "c.w"));
    assert!(matches!(writers[1], Writer::Commit(c) if c.place().text() == "c.w"));
    // The other signals of the child are not touched.
    assert!(child.port("a").unwrap().writers().is_empty());
}

#[test]
fn instantiated_by_lists_instances() {
    let src = "\
module Cell(p: Bit) {}
module A() { signal s: Bit; let u = Cell(s); }
module B() { signal s: Bit; let v = Cell(s); let w = Cell(s); }
module C() { signal s: Bit; }
";
    let design = with_prelude(src);
    let cell = design.module("Cell").unwrap();
    let by: Vec<_> = cell.instantiated_by().iter().map(|i| (i.name().unwrap().to_string(), i.module())).collect();
    assert_eq!(by.iter().map(|b| b.0.as_str()).collect::<Vec<_>>(), ["u", "v", "w"]);
    assert!(by.iter().all(|b| b.1 == Some(cell)));
    assert!(design.module("C").unwrap().instantiated_by().is_empty());
    assert_eq!(cell.instantiated_by()[0].declared_in().module(), design.module("A"));
}

/// Every parse-clean ```` ```rust ```` fence of Appendices D, E and F, with its origin.
fn gallery() -> Vec<(String, String)> {
    corpus()
        .into_iter()
        .filter(|b| b.class == Class::Valid)
        .filter(|b| ["appendix_d", "appendix_e", "appendix_f"].iter().any(|a| b.file.starts_with(a)))
        .map(|b| (format!("{}:{}", b.file, b.line), b.text))
        .collect()
}

/// What a file of a design holds: the number of each node kind owned by its declarations,
/// and the kinds of its declarations, in arena order.
fn shape(design: &Design, file: &str) -> (usize, usize, usize, usize, Vec<&'static str>) {
    let in_file = |owner: piperine_lang::pom::DeclId| design.decl_data(owner).file == design.files().iter().find(|f| f.name() == file).unwrap().id();
    let exprs = (0..design.expr_count()).filter(|i| in_file(design.expr_node(ExprId(*i as u32)).owner)).count();
    let stmts = (0..design.stmt_count()).filter(|i| in_file(design.stmt_node(StmtId(*i as u32)).owner)).count();
    let pats = (0..design.pat_count()).filter(|i| in_file(design.pat_node(PatId(*i as u32)).owner)).count();
    let types = (0..design.type_count()).filter(|i| in_file(design.type_node(TypeId(*i as u32)).owner)).count();
    let decls = design.decls().into_iter().filter(|d| in_file(d.id())).map(|d| d.kind().label()).collect();
    (exprs, stmts, pats, types, decls)
}

#[test]
fn resolution_creates_no_node() {
    let mut sources = vec![("local".to_string(), REVERSE.to_string()), ("names".to_string(), NAMES.to_string())];
    sources.extend(gallery());
    assert!(sources.len() > 10);
    for (at, text) in sources {
        let without = Design::builder().source("top.msl", text.clone()).build();
        let with = with_prelude(&text);
        assert_eq!(shape(&with, "top.msl"), shape(&without, "top.msl"), "{at}: resolving against a prelude changed the node set");
        let (a, b) = (with.files().into_iter().find(|f| f.name() == "top.msl").unwrap(), without.files().into_iter().find(|f| f.name() == "top.msl").unwrap());
        assert_eq!(a.to_ast(), b.to_ast(), "{at}: the rebuilt tree depends on resolution");
    }
}

#[test]
fn gallery_signals_are_declared_in_their_module() {
    let mut signals = 0;
    for (at, text) in gallery() {
        let design = with_prelude(&text);
        for m in design.modules().into_iter().filter(|m| !m.file().krate().is_std()) {
            for s in m.signals() {
                signals += 1;
                assert_eq!(s.module(), Some(m), "{at}: signal {:?} is declared in {:?}", s.name(), m.name());
                assert!(
                    s.declared_in().is_some_and(|d| d == m.decl() || d.as_generation().is_some()),
                    "{at}: the parent of {:?} is its module or a generation of it",
                    s.name()
                );
                assert_eq!(m.signal(s.name().unwrap()).and_then(|x| x.module()), Some(m));
            }
        }
        // Every signal declaration of the design is listed by exactly one module.
        for d in design.decls().into_iter().filter(|d| !d.file().krate().is_std()) {
            if let Some(s) = d.as_signal() {
                let owner = s.module().unwrap_or_else(|| panic!("{at}: signal {:?} is in no module", s.name()));
                assert_eq!(owner.signals().iter().filter(|x| **x == s).count(), 1, "{at}: {:?}", s.name());
            }
        }
    }
    assert!(signals > 20, "the galleries declare signals; found {signals}");
}

#[test]
fn sar_adc_instances_name_their_modules() {
    // F.1 with the two models it takes from other sections (§12.1, §13), as one source.
    let fence = |needle: &str| corpus().into_iter().find(|b| b.text.contains(needle)).unwrap_or_else(|| panic!("a fence holds `{needle}`")).text;
    let text = [fence("module SarAdc"), fence("module SampleHold"), fence("module Comparator")].join("\n");
    let design = with_prelude(&text);
    let adc = design.module("SarAdc").unwrap();
    let placed: Vec<_> = adc.instances().iter().filter_map(|i| Some((i.name()?.to_string(), i.module()?.name()?.to_string()))).collect();
    assert_eq!(
        placed,
        [("sh", "SampleHold"), ("dac", "Dac8"), ("comp", "Comparator"), ("logic", "SarLogic")].map(|(a, b)| (a.to_string(), b.to_string()))
    );
    for (name, callee) in [("sh", "SampleHold"), ("dac", "Dac8"), ("comp", "Comparator"), ("logic", "SarLogic")] {
        assert_eq!(adc.instance(name).unwrap().callee().text(), callee);
    }
    assert_eq!(design.module("Dac8").unwrap().instantiated_by().len(), 1);
    let code = design.module("Dac8").unwrap().port("code").unwrap();
    assert_eq!(adc.instance("dac").unwrap().connection(code).unwrap().text(), "code");
}

#[test]
fn design_and_storage_types_implement_serde() {
    use piperine_lang::pom::*;
    fn serde<T: Serialize + DeserializeOwned>() {}
    serde::<Design>();
    serde::<DesignDiagnostic>();
    serde::<SourceFileData>();
    serde::<SourceFileRepr>();
    serde::<(DeclId, ExprId, StmtId, PatId, TypeId, PathId, FileId)>();
    serde::<(DeclData, DeclKind, NamespaceData, NamespaceOrigin, UseData, UseTreeNode, UseTreeKind, ConstData, FnData, FnParamData)>();
    serde::<(StructData, FieldData, EnumData, VariantData, TraitData, ImplData, GenericParamData, FieldsForm)>();
    serde::<(NatureData, QuantityData, NatureFieldNode, ModuleData, PortData, ParamData, SignalData, LetData)>();
    serde::<(BindingData, BindingOrigin, DefinitionData, GenerationData, GenElseNode, BehaviorData, ReactionData)>();
    serde::<(ExprNode, ExprKind, StmtNode, StmtKind, PatNode, PatKind, TypeNode, TypeKind, PathNode, PathSegment)>();
    serde::<(Arg, Arm, BlockNode, ClosureParam, FieldInit, FieldPatNode, GenericArgNode, GenericArgsNode, Lit, LitKind)>();
    serde::<(Resolution, DeferReason, CallClass, StructClass, PortKind, NatureKind, BuiltinId)>();
    serde::<(Meta, Doc, AttrNode, AttrInputNode, AttrArgNode, Import, Index, ReferenceId, WriterId)>();
}

/// The F.1 design with the models it takes from other sections, built with the fixture prelude.
fn sar_adc() -> Design {
    let fence = |needle: &str| corpus().into_iter().find(|b| b.text.contains(needle)).unwrap().text;
    with_prelude(&[fence("module SarAdc"), fence("module SampleHold"), fence("module Comparator")].join("\n"))
}

#[test]
fn gallery_design_round_trips_through_json() {
    let design = sar_adc();
    assert!(!design.diagnostics().is_empty() || design.decl_count() > 50);
    let json = serde_json::to_string(&design).unwrap();
    let back: Design = serde_json::from_str(&json).unwrap();
    assert!(back == design, "the deserialized design is equal to the original");
    assert_eq!(back.dump(), design.dump());
    assert_eq!(back.diagnostics(), design.diagnostics());
    assert_eq!(back.files().len(), design.files().len());
    for (a, b) in back.files().iter().zip(design.files().iter()) {
        assert_eq!(a.ast(), b.ast(), "{}: the retained tree is rebuilt from the text", a.name());
        assert_eq!(a.text(), b.text());
    }
    // The reverse index survives too.
    assert_eq!(back.module("Dac8").unwrap().instantiated_by().len(), 1);
}

#[test]
fn json_is_deterministic() {
    let design = sar_adc();
    let first = serde_json::to_string(&design).unwrap();
    assert_eq!(first, serde_json::to_string(&design).unwrap());
    // A second build of the same sources serializes to the same bytes.
    assert_eq!(first, serde_json::to_string(&sar_adc()).unwrap());
    // The retained syntax tree is not part of the serialized form.
    let value: serde_json::Value = serde_json::from_str(&first).unwrap();
    fn has_key(v: &serde_json::Value, key: &str) -> bool {
        match v {
            serde_json::Value::Object(m) => m.contains_key(key) || m.values().any(|v| has_key(v, key)),
            serde_json::Value::Array(a) => a.iter().any(|v| has_key(v, key)),
            _ => false,
        }
    }
    assert!(!has_key(&value, "ast"), "no `ast` key in the JSON");
}

/// POM-08 story: from a reference, a reader reaches the declaration handle
/// (path, expression, member, struct literal) and from the id, a handle.
#[test]
fn references_navigate_to_declarations() {
    use piperine_lang::pom::{ExprKind, Resolution};
    let src = "\
struct Stream { out valid: Bit }
fn pick(h: Stream) -> Bit { h.valid }
module M(a: Bit, out q: Bit, h: Stream) { signal s: Bit; s = a; let v = h.valid; let n = 1; }
digital for M { on a { q <- s; } }
";
    let design = with_prelude(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let m = design.module("M").unwrap();
    let s = m.signal("s").unwrap();

    // Design::decl turns any id into a handle that answers every question.
    let root_id = design.root().id();
    assert_eq!(design.decl(root_id), design.root());
    assert_eq!(design.decl(s.decl().id()).as_signal(), Some(s));

    // A signal read in a reaction: expr -> path -> the declaration -> its module.
    let commit = m.digital_blocks()[0].reactions()[0].commits()[0];
    let read = commit.value();
    let path = read.as_path().expect("`s` is a path expression");
    assert_eq!(path.target(), Some(s.decl()));
    assert_eq!(read.resolution(), Resolution::Decl(s.decl().id()));
    assert_eq!(read.target().and_then(|d| d.as_signal()).and_then(|x| x.declared_in()), Some(m.decl()));
    // The place `q` of the commit reaches the port.
    assert_eq!(commit.place().target(), Some(m.port("q").unwrap().decl()));

    // A type path reaches its declaration.
    let piperine_lang::pom::TypeKind::Path(p) = m.port("a").unwrap().ty().kind() else { panic!("`Bit` is a path type") };
    let type_path = design.path(*p);
    let bit = design.nature("Bit").unwrap().decl();
    assert_eq!(type_path.target(), Some(bit));
    assert_eq!(type_path.resolution(), Resolution::Decl(bit.id()));

    // A member access reaches the field.
    let field = design.find("Stream").and_then(|d| d.as_struct()).unwrap().fields()[0].decl();
    let accesses: Vec<_> = (0..design.expr_count())
        .map(|i| design.expr(ExprId(i as u32)))
        .filter(|e| matches!(e.kind(), ExprKind::Field { .. }) && e.enclosing().krate().is_root())
        .collect();
    assert_eq!(accesses.len(), 2, "the two accesses of the root file; the library has its own");
    assert!(accesses.iter().all(|e| e.target() == Some(field) && e.as_path().is_none()));

    // An expression that names nothing has no target.
    let literals: Vec<_> = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).filter(|e| matches!(e.kind(), ExprKind::Lit(_))).collect();
    assert!(!literals.is_empty());
    assert!(literals.iter().all(|e| e.target().is_none() && e.resolution() == Resolution::Unresolved));
}

/// A qualified name tells apart items that share a short name, and `find`
/// accepts every form `qualified_name` prints.
#[test]
fn qualified_names_are_unambiguous_and_findable() {
    let src = "\
trait Tr { fn run() -> u32; }
struct S {}
enum E { A }
impl S { const X: u32 = 1; }
impl Tr for S { const X: u32 = 3; fn run() -> u32 { 4 } }
mod a { pub enum E { A } pub module M() {} }
module Z(p: Electrical) { signal q: Electrical; }
analog for Z { signal z: Electrical; }
";
    let design = with_prelude(src);
    let names: Vec<String> = design.decls().into_iter().filter(|d| !d.file().krate().is_std()).map(|d| d.qualified_name()).collect();
    for expected in ["impl S.X", "impl Tr for S.X", "impl Tr for S.run", "a::E.A", "Z.q", "analog for Z.z"] {
        assert!(names.iter().any(|n| n == expected), "`{expected}` in {names:?}");
    }
    // Two items named `X` do not share a qualified name.
    let xs: Vec<_> = design.decls().into_iter().filter(|d| d.name() == Some("X") && !d.file().krate().is_std()).map(|d| d.qualified_name()).collect();
    assert_eq!(xs, ["impl S.X", "impl Tr for S.X"]);
    // Every name printed finds the declaration that printed it (the first one, for equal names).
    for d in design.decls().into_iter().filter(|d| !d.file().krate().is_std() && d.name().is_some() && !matches!(d.kind(), piperine_lang::pom::DeclKind::Binding(_))) {
        let q = d.qualified_name();
        let found = design.find(&q).unwrap_or_else(|| panic!("`{q}` finds nothing"));
        assert_eq!(found.qualified_name(), q);
    }
    assert_eq!(design.find("a::E.A").map(|d| d.id()), design.find("a::E::A").map(|d| d.id()));
    assert!(design.find("a::E.B").is_none());
}

const SHADOW_PRELUDE: &str = "\
pub nature Bit { value: bool }
pub module Buf(a: Bit) {}
pub struct S { p: f64 }
pub fn f() -> f64 { 1.0 }
pub enum E { P }
pub trait Tr {}
pub const K: f64 = 1.0;
";

const SHADOW_PROJECT: &str = "\
nature Bit { value: bool }
module Buf(a: Bit) {}
struct S { q: f64 }
fn f() -> f64 { 2.0 }
enum E { Q }
trait Tr {}
const K: f64 = 2.0;
impl S { fn m(self) -> f64 { 1.0 } }
";

fn shadowed() -> Design {
    Design::builder().std_source("lib.msl", format!("pub mod prelude {{\n{SHADOW_PRELUDE}\n}}\n")).source("top.msl", SHADOW_PROJECT).build()
}

/// The root crate's declarations win every by-name lookup.
#[test]
fn by_name_lookups_prefer_project_files() {
    let design = shadowed();
    let project = |d: Decl<'_>| !d.file().krate().is_std();
    assert!(project(design.nature("Bit").unwrap().decl()));
    assert!(project(design.module("Buf").unwrap().decl()));
    assert!(project(design.struct_named("S").unwrap().decl()));
    assert!(project(design.function("f").unwrap().decl()));
    assert!(project(design.find("Bit").unwrap()));
    assert!(project(design.find("S").unwrap()));
    assert!(project(design.find("f").unwrap()));
    // A name only the prelude declares is still found.
    let only = Design::builder().std_source("lib.msl", format!("pub mod prelude {{\n{SHADOW_PRELUDE}\n}}\n")).source("top.msl", "module Top() {}\n").build();
    assert!(only.nature("Bit").unwrap().file().krate().is_std());
    assert!(only.struct_named("S").unwrap().file().krate().is_std());
    assert!(only.function("f").unwrap().file().krate().is_std());
}

#[test]
fn collections_list_project_files_only() {
    let design = shadowed();
    let all_project = |v: Vec<Decl<'_>>| v.iter().all(|d| !d.file().krate().is_std());
    assert!(all_project(design.modules().iter().map(|x| x.decl()).collect()));
    assert!(all_project(design.natures().iter().map(|x| x.decl()).collect()));
    assert!(all_project(design.structs().iter().map(|x| x.decl()).collect()));
    assert!(all_project(design.enums().iter().map(|x| x.decl()).collect()));
    assert!(all_project(design.traits().iter().map(|x| x.decl()).collect()));
    assert!(all_project(design.consts().iter().map(|x| x.decl()).collect()));
    assert!(all_project(design.impls().iter().map(|x| x.decl()).collect()));
    assert!(all_project(design.functions().iter().map(|x| x.decl()).collect()));
    assert_eq!(design.natures().len(), 1);
    assert_eq!(design.modules().len(), 1);
    let prelude = design.std_crate().files();
    assert_eq!(prelude.len(), 1);
    assert!(prelude[0].krate().is_std());
    assert_eq!(prelude[0].name(), "lib.msl");
    // `decls()` and `files()` still list everything.
    assert_eq!(design.files().len(), 2);
    assert!(design.decls().iter().any(|d| d.file().krate().is_std()));
}

#[test]
fn design_has_a_collection_and_lookup_per_item_kind() {
    let design = shadowed();
    assert_eq!(design.enums().iter().map(|e| e.name()).collect::<Vec<_>>(), [Some("E")]);
    assert_eq!(design.traits().iter().map(|e| e.name()).collect::<Vec<_>>(), [Some("Tr")]);
    assert_eq!(design.consts().iter().map(|e| e.name()).collect::<Vec<_>>(), [Some("K")]);
    assert_eq!(design.impls().len(), 1);
    assert_eq!(design.struct_named("S").unwrap().fields()[0].name(), Some("q"));
    assert_eq!(design.function("f").unwrap().name(), Some("f"));
    assert!(design.struct_named("Nope").is_none());
    assert!(design.function("Nope").is_none());
}

#[test]
fn types_list_their_impls_and_methods() {
    let src = "\
struct S { a: f64 }
enum E { A, B }
trait Tr { fn go(self) -> f64; }
impl S { fn one(self) -> f64 { 1.0 } }
impl Tr for S { fn go(self) -> f64 { 2.0 } }
impl E { fn two(self) -> f64 { 2.0 } fn three(self) -> f64 { 3.0 } }
impl Tr for E { fn go(self) -> f64 { 4.0 } }
struct Lone { z: f64 }
";
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    let s = design.struct_named("S").unwrap();
    assert_eq!(s.impls().len(), 2);
    assert_eq!(s.impls()[0].trait_path().map(|p| p.text()), None);
    assert_eq!(s.impls()[1].trait_path().map(|p| p.text()), Some("Tr"));
    assert_eq!(s.method("one").unwrap().name(), Some("one"));
    assert_eq!(s.method("go").unwrap().name(), Some("go"), "a method of a trait impl is found too");
    assert!(s.method("two").is_none());
    let e = design.enums()[0];
    assert_eq!(e.impls().len(), 2);
    assert_eq!(e.method("three").unwrap().name(), Some("three"));
    assert!(e.method("one").is_none());
    let tr = design.traits()[0];
    let implementors: Vec<_> = tr.impls().iter().map(|i| i.self_ty().text()).collect();
    assert_eq!(implementors, ["S", "E"], "implementors in source order");
    assert!(design.struct_named("Lone").unwrap().impls().is_empty());
    assert!(design.struct_named("Lone").unwrap().method("go").is_none());
}

const FLOATS: &str = "const A: f64 = 1e999;\nconst B: f64 = 1e308;\nconst C: f64 = 5e-324;\nconst D: f64 = 1.5k;\nconst E: f64 = 2.5e-3f32;\n";

#[test]
fn float_literals_round_trip_through_json() {
    // `1e999` is infinite as an `f64`: the lexer reports it, the design still serializes and reads back.
    let design = Design::builder().source("top.msl", FLOATS).build();
    assert!(design.diagnostics().iter().any(|d| d.diagnostic.code == "E0909"));
    let json = serde_json::to_string(&design).expect("a design with an infinite literal serializes");
    let back: Design = serde_json::from_str(&json).expect("and reads back");
    assert_eq!(back.dump(), design.dump());
    assert!(back == design);
    assert_eq!(serde_json::to_string(&back).unwrap(), json);
    // The values are those of the text.
    let value = |d: &Design, name: &str| match d.find(name).and_then(|c| c.as_const()).and_then(|c| c.value()).map(|e| e.kind().clone()) {
        Some(piperine_lang::pom::ExprKind::Lit(l)) => match l.kind {
            piperine_lang::pom::LitKind::Float { value, .. } => value,
            other => panic!("not a float: {other:?}"),
        },
        other => panic!("not a literal: {other:?}"),
    };
    assert_eq!(value(&back, "A"), f64::INFINITY);
    assert_eq!(value(&back, "B"), 1e308);
    assert_eq!(value(&back, "C"), 5e-324);
    assert_eq!(value(&back, "D"), 1500.0);
    assert_eq!(value(&back, "E"), 2.5e-3);
}

#[test]
fn float_literal_is_serialized_from_its_text() {
    use piperine_lang::pom::LitKind;
    let lit = LitKind::Float { value: 1500.0, raw: "1.5k".to_string() };
    let json = serde_json::to_string(&lit).unwrap();
    assert!(json.contains("1.5k"), "{json}");
    assert!(!json.contains("value") && !json.contains("1500"), "no numeric value is stored: {json}");
    let back: LitKind = serde_json::from_str(&json).unwrap();
    assert_eq!(back, lit, "the value is recomputed from the text");
    // The value is a function of the text: a stored number cannot disagree with it.
    let doctored: LitKind = serde_json::from_str(&json.replace("1.5k", "2u")).unwrap();
    assert_eq!(doctored, LitKind::Float { value: 2e-6, raw: "2u".to_string() });
    // Other literal kinds keep their form.
    for other in [LitKind::Int("5u8".into()), LitKind::Bool(true), LitKind::Str { text: "a".into(), raw_hashes: Some(1) }] {
        let again: LitKind = serde_json::from_str(&serde_json::to_string(&other).unwrap()).unwrap();
        assert_eq!(again, other);
    }
}

#[test]
fn from_json_reads_back_a_valid_design() {
    for design in [sar_adc(), Design::builder().source("top.msl", FLOATS).build()] {
        let json = serde_json::to_string(&design).unwrap();
        let back = Design::from_json(&json).expect("a valid design reads back");
        assert_eq!(back.dump(), design.dump());
        assert!(back == design);
    }
}

#[test]
fn from_json_rejects_a_corrupted_design() {
    let design = Design::builder().source("top.msl", "module M(a: Bit) { signal s: Bit; s = a; }\nfn f() -> f64 { 1.0 + 2.0 }").build();
    let good = serde_json::to_value(&design).unwrap();
    let edit = |f: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        f(&mut v);
        Design::from_json(&v.to_string())
    };
    let big = serde_json::json!(99_999);
    // An id out of range, in each arena.
    let cases = [
        edit(&|v| v["exprs"][0]["owner"] = big.clone()),
        edit(&|v| v["decls"][0]["children"].as_array_mut().unwrap().push(big.clone())),
        edit(&|v| v["files"][0]["root"] = big.clone()),
        edit(&|v| v["stmts"][0]["owner"] = big.clone()),
    ];
    for (i, case) in cases.iter().enumerate() {
        let problems = case.as_ref().err().unwrap_or_else(|| panic!("case {i}: a corrupted design was accepted"));
        assert!(!problems.is_empty(), "case {i}");
    }
    // A link `check()` rejects: the parent table disagrees with the nodes.
    let broken = edit(&|v| {
        let table = v["expr_parent"].as_array_mut().unwrap();
        let at = table.iter().position(|p| !p.is_null()).unwrap();
        table[at] = serde_json::Value::Null;
    });
    assert!(broken.unwrap_err().iter().any(|p| p.contains("parent")));
    // Text that is not a design.
    for text in ["", "[1, 2]", "{\"files\": 3}", "not json"] {
        let problems = Design::from_json(text).err().unwrap_or_else(|| panic!("`{text}` was accepted"));
        assert!(!problems.is_empty());
    }
    // The derived `Deserialize` stays unvalidated; `from_json` is the entry point.
    let unchecked: Design = serde_json::from_value(good.clone()).unwrap();
    assert!(unchecked.check().is_empty());
}

fn number_leaves(v: &serde_json::Value, path: Vec<String>, out: &mut Vec<Vec<String>>) {
    match v {
        serde_json::Value::Number(_) => out.push(path),
        serde_json::Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                number_leaves(x, [path.clone(), vec![i.to_string()]].concat(), out);
            }
        }
        serde_json::Value::Object(o) => {
            for (k, x) in o {
                number_leaves(x, [path.clone(), vec![k.clone()]].concat(), out);
            }
        }
        _ => {}
    }
}

/// Every read a host makes over a design that `from_json` returned.
fn exercise(d: &Design) {
    use piperine_lang::pom::{DeclId, ExprId};
    let _ = d.dump();
    for i in 0..d.decl_count() {
        let decl = d.decl(DeclId(i as u32));
        let _ = (decl.children(), decl.qualified_name(), decl.text(), decl.references(), decl.exprs());
    }
    for i in 0..d.expr_count() {
        let e = d.expr(ExprId(i as u32));
        let _ = (e.children(), e.parent(), e.stmt(), e.text());
    }
    for m in d.modules() {
        let _ = m.instances();
        for s in m.signals() {
            let _ = s.writers();
        }
    }
    for f in d.files() {
        let _ = f.to_ast();
    }
}

#[test]
fn a_nature_field_that_names_no_quantity_is_an_err_not_a_panic() {
    // The standard library declares natures, so a corrupted field id reaches `to_ast`: `from_json`
    // reports it and no read panics. The first `Quantity` field of `Electrical` is pointed at the
    // root namespace (decl 0), which is no quantity.
    let design = Design::builder().std_source("lib.msl", "pub mod prelude;\n").std_source("prelude.msl", "pub nature N { potential: P { access: v, units: \"V\", abstol: 1u } }\n").source("top.msl", "").build();
    let mut json = serde_json::to_value(&design).unwrap();
    let mut found = false;
    fn point_quantities_at_root(v: &mut serde_json::Value, found: &mut bool) {
        match v {
            serde_json::Value::Object(map) => {
                if let Some(q) = map.get_mut("Quantity") {
                    if q.is_number() {
                        *q = serde_json::json!(0);
                        *found = true;
                    }
                }
                for (_, c) in map.iter_mut() {
                    point_quantities_at_root(c, found);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(|c| point_quantities_at_root(c, found)),
            _ => {}
        }
    }
    point_quantities_at_root(&mut json, &mut found);
    assert!(found, "the serialized design has a nature field that names a quantity");
    let text = json.to_string();
    let result = std::panic::catch_unwind(|| Design::from_json(&text));
    let result = result.expect("from_json never panics");
    let problems = result.err().expect("a nature field naming no quantity is rejected");
    assert!(problems.iter().any(|p| p.contains("quantity")), "{problems:?}");
}

#[test]
fn from_json_never_panics_on_a_corrupted_field() {
    // POL-21 AC 2 (A-33): every numeric leaf of a design, set to an id out of
    // range and to 1, is either an `Err` or a design every read survives.
    std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(|| {
            let src = "mod a { pub const K: f64 = 1.5k; }\nuse a::K;\nmodule M(a: E) { signal s: E; s = a; let x = R(a, a) { r: K }; }\ndigital for M { on e { s[0] <- 1; } }\nfn f(o: Option<u8>) -> u8 { match o { None => 0, Some(x) => x + 1 } }";
            let design = Design::builder().std_source("lib.msl", "pub mod prelude;\n").std_source("prelude.msl", "").source("top.msl", src).build();
            let good = serde_json::to_value(&design).unwrap();
            let mut leaves = Vec::new();
            number_leaves(&good, Vec::new(), &mut leaves);
            assert!(leaves.len() > 300, "{}", leaves.len());
            let mut panics = Vec::new();
            for leaf in &leaves {
                for value in [99_999u64, 1] {
                    let mut v = good.clone();
                    let mut cur = &mut v;
                    for k in leaf {
                        cur = match k.parse::<usize>() {
                            Ok(i) if cur.is_array() => &mut cur[i],
                            _ => &mut cur[k.as_str()],
                        };
                    }
                    *cur = serde_json::json!(value);
                    let text = v.to_string();
                    match std::panic::catch_unwind(|| Design::from_json(&text)) {
                        Err(_) => panics.push(format!("from_json: {}={value}", leaf.join("."))),
                        Ok(Err(problems)) => assert!(!problems.is_empty()),
                        Ok(Ok(d)) => {
                            if std::panic::catch_unwind(|| exercise(&d)).is_err() {
                                panics.push(format!("use: {}={value}", leaf.join(".")));
                            }
                        }
                    }
                }
            }
            assert!(panics.is_empty(), "{} panics, e.g. {:?}", panics.len(), &panics[..panics.len().min(6)]);
        })
        .unwrap()
        .join()
        .unwrap();
}

