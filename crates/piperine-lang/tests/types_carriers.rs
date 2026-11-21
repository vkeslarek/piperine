//! Discrete carriers and definitions of `piperine_lang::types` (feature `msl-digital`, DIG-04,
//! DIG-02 AC 1): the type of a `signal` and of a port of `Bit<T>`, what a carrier may hold
//! (`E0112`), initial values (`E0210`), the flag reads `known`, `known_or`, `is_known`, `high`
//! and `prev`, and a catalogue method applied to a carrier. Sources are written over the embedded
//! standard library, the buck's own enums read as written.

#[path = "support/buck.rs"]
#[allow(dead_code)]
mod buck;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, Reflect};
use piperine_lang::types::{Builtin, Method, Ty, Types};

fn design_of(src: &str) -> Design {
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    design
}

fn checked<'d>(design: &'d Design, src: &str) -> Types<'d> {
    Types::check(design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()))
}

fn diagnostics(src: &str) -> Vec<DesignDiagnostic> {
    let design = design_of(src);
    Types::check(&design, "Top").expect_err("expected diagnostics")
}

fn codes(d: &[DesignDiagnostic]) -> Vec<String> {
    d.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

/// The declaration named `name` of one of the `kinds` (`port`, `signal`, `binding`).
fn decl(design: &Design, name: &str, kind: &str) -> DeclId {
    design
        .decls()
        .into_iter()
        .find(|d| {
            d.name() == Some(name)
                && match (d.kind(), kind) {
                    (DeclKind::Port(_), "port") | (DeclKind::Signal(_), "signal") | (DeclKind::Binding(_), "binding") | (DeclKind::Nature(_), "nature") => true,
                    (DeclKind::Enum(_), "enum") => true,
                    _ => false,
                }
        })
        .unwrap_or_else(|| panic!("no {kind} `{name}`"))
        .id()
}

/// Every method call named `name` of the reachable set, in source order.
fn method_calls(design: &Design, types: &Types, name: &str) -> Vec<ExprId> {
    (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && matches!(&design.expr_node(*e).kind, ExprKind::MethodCall { method, .. } if method.text == name)).collect()
}

/// The text of `item` in `examples/buck/src/<file>`, as the carriers test of the buck reads it.
fn buck_enum(name: &str) -> String {
    let path = format!("{}/../../examples/buck/src/supervisor.msl", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let at = text.find(&format!("pub enum {name}")).unwrap_or_else(|| panic!("no `{name}`"));
    let start = text[..at].rfind("#[derive").expect("a derive");
    let end = at + text[at..].find('}').expect("the enum closes") + 1;
    text[start..end].to_string()
}

#[test]
fn carriers_of_bit_are_typed_by_their_value() {
    // DIG-04 AC 1: a `signal`, an `in` port and an `out` port of `Bit<T>` for `T` a `bool`, an
    // integer type, `f64` or a unit enum; `Bit` alone is `Bit<bool>` (part II §4 rule 12).
    let src = format!(
        "{}\nmodule Top(in a: Bit, in b: Bit<u9>, out c: Bit<i16> = 0, out d: Bit<f64> = 0.0, out e: Bit<Mode> = Mode::Off) {{\n    signal f: Bit<bool>;\n    signal g: Bit<Mode>;\n    signal h: Bit<u3> = 5;\n    signal k: Bit<f64>;\n}}",
        buck_enum("Mode")
    );
    let design = design_of(&src);
    let types = checked(&design, &src);
    let bit = decl(&design, "Bit", "nature");
    let mode = decl(&design, "Mode", "enum");
    let carrier = |name: &str, kind: &str, value: Ty| (types.carrier(decl(&design, name, kind)).cloned(), Some(Ty::Carrier { nature: bit, value: Box::new(value) }));
    for (got, want) in [
        carrier("a", "port", Ty::Bool),
        carrier("b", "port", Ty::UInt(9)),
        carrier("c", "port", Ty::Int(16)),
        carrier("d", "port", Ty::F64),
        carrier("e", "port", Ty::Enum(mode)),
        carrier("f", "signal", Ty::Bool),
        carrier("g", "signal", Ty::Enum(mode)),
        carrier("h", "signal", Ty::UInt(3)),
        carrier("k", "signal", Ty::F64),
    ] {
        assert_eq!(got, want);
    }
    assert_eq!(types.carrier(decl(&design, "b", "port")).map(|t| t.name(&design)).as_deref(), Some("Bit<u9>"));
}

#[test]
fn a_carried_type_without_repr_is_e0112() {
    // DIG-04 AC 2 (part I §2): a carried type without `Repr` is `E0112`, at the type: `str`,
    // `()`, an `Option`, an enum that does not derive `Repr`, and a module.
    for (items, carried) in [
        ("", "str"),
        ("", "()"),
        ("", "Option<u8>"),
        ("#[derive(PartialEq)]\nenum Plain { A, B }", "Plain"),
        ("", "Event"),
    ] {
        let src = format!("{items}\nmodule Top() {{ signal s: Bit<{carried}>; }}");
        let d = diagnostics(&src);
        let want = if carried == "Event" { "E0441" } else { "E0112" };
        assert_eq!(codes(&d), [want], "{carried}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), carried);
    }
}

#[test]
fn a_carrier_read_has_its_value_type() {
    // DIG-04 AC 3: a read of a `Bit<u9>` has the type `u9`, through a module-body `let`, as the
    // argument of a function and as an operand. A read in a definition, a guard and a reaction
    // is asserted by the tasks that first type those positions (T10, T12, T13).
    let src = "fn plus(a: u9, b: u9) -> u9 { a + b }\nmodule Top(in count: Bit<u9>, in en: Bit) {\n    let next = count + 1;\n    let via_fn = plus(count, 2);\n    let flag = !en;\n}";
    let design = design_of(src);
    let types = checked(&design, src);
    let got: Vec<Option<&Ty>> = ["next", "via_fn", "flag"].iter().map(|n| types.local(decl(&design, n, "binding"))).collect();
    assert_eq!(got, [Some(&Ty::UInt(9)), Some(&Ty::UInt(9)), Some(&Ty::Bool)]);
    // The read of the carrier itself.
    let counts: Vec<_> = (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && matches!(&design.expr_node(*e).kind, ExprKind::Path(_)) && src[design.expr_node(*e).span.range()] == *"count").collect();
    assert!(counts.len() >= 2 && counts.iter().all(|e| types.ty(*e) == Some(&Ty::UInt(9))), "{counts:?}");
}

#[test]
fn the_flag_reads_are_typed() {
    // DIG-04 AC 4 (part I §3.3.4, §3.3.7): `known()` is `Option<T>`, `known_or(d)` is `T`,
    // `is_known()` and `high()` are `bool`, on a carrier and on a value, and the checker records
    // each as a built-in.
    let src = "module Top(in c: Bit<u9>, in g: Bit, in x: Bit<f64>) {\n    let a = c.known();\n    let b = c.known_or(0);\n    let d = c.is_known();\n    let e = g.high();\n    let f = (c + 1).known_or(0);\n    let h = (x * 2.0).known();\n    let k = x.known_or(1.5);\n}";
    let design = design_of(src);
    let types = checked(&design, src);
    let got: Vec<Option<&Ty>> = ["a", "b", "d", "e", "f", "h", "k"].iter().map(|n| types.local(decl(&design, n, "binding"))).collect();
    let (u9, f64_) = (Ty::UInt(9), Ty::F64);
    let (opt_u9, opt_f64) = (Ty::Option(Box::new(u9.clone())), Ty::Option(Box::new(f64_.clone())));
    assert_eq!(got, [Some(&opt_u9), Some(&u9), Some(&Ty::Bool), Some(&Ty::Bool), Some(&u9), Some(&opt_f64), Some(&f64_)]);
    let builtins: Vec<(String, Option<Builtin>)> = ["known", "known_or", "is_known", "high"].iter().flat_map(|n| method_calls(&design, &types, n)).map(|e| (src[design.expr_node(e).span.range()].to_string(), types.builtin(e))).collect();
    let kinds: Vec<Option<Builtin>> = builtins.iter().map(|(_, b)| *b).collect();
    assert_eq!(kinds.iter().filter(|b| **b == Some(Builtin::Known)).count(), 2);
    assert_eq!(kinds.iter().filter(|b| **b == Some(Builtin::KnownOr)).count(), 3);
    assert_eq!(kinds.iter().filter(|b| **b == Some(Builtin::IsKnown)).count(), 1);
    assert_eq!(kinds.iter().filter(|b| **b == Some(Builtin::High)).count(), 1);
    assert_eq!(kinds.len(), 7, "{builtins:?}");

    // `high` is the read of a `Bit`; a default of the wrong type is a mismatch.
    let src = "module Top(in c: Bit<u9>) { let a = c.high(); }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), d[0].diagnostic.message.contains("`high` on `u9`")), (vec!["P0001".to_string()], true), "{}", d[0].diagnostic.message);
    let src = "module Top(in c: Bit<u9>) { let a = c.known_or(true); }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0180"]);
    assert_eq!(snippet(src, &d[0]), "true");

    // §3.3.7: a project's own `high` is not the stdlib's. It is `P0001` at its use, whether the
    // POM resolved the receiver to it or left it to the checker.
    for src in [
        "impl Bit { pub fn high(self) -> bool { true } }\nmodule Top(in g: Bit) { let a = g.high(); }",
        "trait Mine { fn known_or(self, d: u8) -> u8; }\nimpl Mine for u8 { fn known_or(self, d: u8) -> u8 { d } }\nmodule Top(in c: Bit<u8>) { let a = (c + 1).known_or(0); }",
    ] {
        let d = diagnostics(src);
        assert_eq!(codes(&d), ["P0001"], "{src}");
        assert!(d[0].diagnostic.message.contains("defined by the project"), "{}", d[0].diagnostic.message);
    }
}

#[test]
fn an_initial_value_of_the_wrong_kind_is_e0210() {
    // DIG-04 AC 5 (part II §5.1.5, §8.1.7): the initial value of a carrier has the carried type,
    // and reads no carrier and no probe; the same for a node, whose initial value is an `f64`,
    // and for the initial value of an `out` port.
    for (src, at) in [
        ("module Top() { signal s: Bit<u8> = true; }", "true"),
        ("module Top() { signal s: Bit<f64> = 1; }", "1"),
        ("module Top(in a: Bit<u8>) { signal s: Bit<u8> = a; }", "a"),
        ("module Top(p: Electrical) { signal s: Bit<f64> = v(p); }", "v(p)"),
        ("module Top(out q: Bit<u8> = 1.5) { }", "1.5"),
        ("module Top() { signal n: Electrical = true; }", "true"),
        ("module Top(p: Electrical) { signal n: Electrical = v(p); }", "v(p)"),
    ] {
        let d = diagnostics(src);
        assert_eq!(codes(&d), ["E0210"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(src, &d[0]), at, "{src}");
    }
    // Initial values of the right kind, `Unknown` aside, type-check.
    let src = "module Top(out q: Bit<u8> = 7) { k: f64 = 2.0; signal s: Bit<f64> = k * 2.0; signal t: Bit<u9> = 511; }";
    checked(&design_of(src), src);
}

#[test]
fn a_catalogue_method_on_a_carrier_is_typed_on_its_value() {
    // DIG-02 AC 1: `d.min(SETPOINT_MAX)` on a `Bit<u8>` carrier is a `u8`, whether the carrier is
    // the receiver or `known()` unwrapped it first (the `SetpointReg` of the buck).
    let src = "const SETPOINT_MAX: u8 = 168;\nmodule Top(in data: Bit<u8>) {\n    let direct = data.min(SETPOINT_MAX);\n    let unwrapped = if let Some(d) = data.known() { d.min(SETPOINT_MAX) } else { 0 };\n}";
    let design = design_of(src);
    let types = checked(&design, src);
    assert_eq!([types.local(decl(&design, "direct", "binding")), types.local(decl(&design, "unwrapped", "binding"))], [Some(&Ty::UInt(8)), Some(&Ty::UInt(8))]);
    let mins = method_calls(&design, &types, "min");
    assert_eq!(mins.iter().map(|e| types.method(*e)).collect::<Vec<_>>(), [Some(Method::Min), Some(Method::Min)]);
    // DIG-02 AC 2: a method with no row names the method and the value type (`E0161`, Appendix H
    // rule 1, now that the integer catalogue is complete).
    let src = "module Top(in oc_count: Bit<u4>) { let n = oc_count.leading_zeros(); }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0161"]);
    assert!(d[0].diagnostic.message.contains("leading_zeros") && d[0].diagnostic.message.contains("u4"), "{}", d[0].diagnostic.message);
}

#[test]
fn a_single_variant_enum_cannot_be_carried() {
    // Edge case 3: a single-variant enum has a tag of zero levels (part I §2 rule 5): `P0001`.
    let src = "#[derive(Repr, PartialEq)]\nenum One { A }\nmodule Top() { signal s: Bit<One>; }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("single-variant enum `One`"), "{}", d[0].diagnostic.message);
    assert_eq!(snippet(src, &d[0]), "One");
    // Two variants are the smallest carried enum.
    let src = "#[derive(Repr, PartialEq)]\nenum Two { A, B }\nmodule Top() { signal s: Bit<Two>; }";
    checked(&design_of(src), src);
}

#[test]
fn prev_is_typed_on_a_carrier() {
    // DIG-04 AC 17 (§5.3): `c.prev()` has the carrier's value type; a node has no `prev()`.
    let src = "module Top(in c: Bit<u9>, in g: Bit) { let a = c.prev(); let b = g.prev(); }";
    let design = design_of(src);
    let types = checked(&design, src);
    assert_eq!([types.local(decl(&design, "a", "binding")), types.local(decl(&design, "b", "binding"))], [Some(&Ty::UInt(9)), Some(&Ty::Bool)]);
    assert_eq!(method_calls(&design, &types, "prev").iter().map(|e| types.builtin(*e)).collect::<Vec<_>>(), [Some(Builtin::Prev), Some(Builtin::Prev)]);
    let src = "module Top(p: Electrical) { let a = p.prev(); }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0230"]);
    assert_eq!(snippet(src, &d[0]), "p.prev()");
}

// ---- definitions (T10) ----

/// The text of every expression of the reachable set that reads as `text`, in source order.
fn exprs_reading(design: &Design, types: &Types, src: &str, text: &str) -> Vec<ExprId> {
    (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| types.ty(*e).is_some() && design.files()[design.decl_data(design.expr_node(*e).owner).file.0 as usize].name() == "top.msl" && src[design.expr_node(*e).span.range()] == *text).collect()
}

#[test]
fn a_definition_of_another_type_is_e0337() {
    // DIG-04 AC 6 (§8.2.19): the expression of a definition has the value type of its target. The
    // read of a carrier in a definition has its value type (DIG-04 AC 3).
    let src = "module Top(in x: Bit<u8>, out y: Bit<u10>, out z: Bit<u8>, out b: Bit) { z = x; b = true; y = x; }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0337"]);
    assert_eq!(snippet(src, &d[0]), "x");
    assert!(d[0].diagnostic.message.contains("u10") && d[0].diagnostic.message.contains("u8"), "{}", d[0].diagnostic.message);
    let src = "module Top(out b: Bit<u9>) { b = true; }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), snippet(src, &d[0])), (vec!["E0337".to_string()], "true"));
    // A literal takes the type of the carrier it defines; a read takes the carried type.
    let src = "module Top(in x: Bit<u8>, out y: Bit<u10>, out w: Bit<f64>) { y = (x as u10) + 1; w = 2.5; }";
    let design = design_of(src);
    let types = checked(&design, src);
    assert!(exprs_reading(&design, &types, src, "x").iter().all(|e| types.ty(*e) == Some(&Ty::UInt(8))));
    assert_eq!(types.ty(exprs_reading(&design, &types, src, "1")[0]), Some(&Ty::UInt(10)));
}

#[test]
fn a_continuous_definition_is_e0336() {
    // DIG-04 AC 7 (§8.2.5): a definition, and its delay, reads no probe, `now()`, `freq()`,
    // `gmin()`, `source_scale()` and `analysis(Static)`, directly or through a `let` or a function.
    let items = "fn pass(x: f64) -> f64 { x }\nfn clock() -> f64 { now() }\n";
    for (body, at) in [
        ("s = v(p) > 0.5;", "v(p) > 0.5"),
        ("b = analysis(Static);", "analysis(Static)"),
        ("f = now();", "now()"),
        ("f = gmin();", "gmin()"),
        ("f = source_scale();", "source_scale()"),
        ("let k = v(p); s = k > 0.5;", "k > 0.5"),
        ("let k = v(p) + 1.0; let m = k * 2.0; f = m;", "m"),
        ("f = pass(now());", "pass(now())"),
        ("f = clock();", "clock()"),
        ("s = x after now();", "now()"),
        ("s = x after v(p);", "v(p)"),
    ] {
        let src = format!("{items}module Top(p: Electrical, in x: Bit, out s: Bit, out b: Bit, out f: Bit<f64>) {{ {body} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0336"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
    // The environment that is constant during an analysis is discrete: `temperature()` may be read.
    let src = "module Top(out f: Bit<f64>, in x: Bit<f64>) { f = temperature() + x; }";
    let design = design_of(src);
    let types = checked(&design, src);
    assert!(exprs_reading(&design, &types, src, "temperature() + x").iter().all(|e| types.continuity(*e) == Some(piperine_lang::types::Continuity::Discrete)));
    // `continuity` says which: the probe and what reads it are continuous, a literal is not.
    let src = "module Top(p: Electrical) { let k = v(p) * 2.0; let n = 3.0; }";
    let design = design_of(src);
    let types = checked(&design, src);
    let cont = |text: &str| exprs_reading(&design, &types, src, text).iter().map(|e| types.continuity(*e)).collect::<Vec<_>>();
    assert_eq!(cont("v(p) * 2.0"), [Some(piperine_lang::types::Continuity::Continuous)]);
    assert_eq!(cont("3.0"), [Some(piperine_lang::types::Continuity::Discrete)]);
}

#[test]
fn a_target_the_module_may_not_drive_is_e0222() {
    // DIG-04 AC 8 (§8.2.10): a definition of an `in` port, of a carrier of a child, of a node, of
    // a param or of a `let` name is `E0222`, at the target.
    let items = "module Child() { pub signal q: Bit; }\n";
    for (header, body, at) in [
        ("in a: Bit, out q: Bit", "a = q;", "a"),
        ("in a: Bit, out q: Bit", "let c = Child(); c.q = true;", "c.q"),
        ("in a: Bit, out q: Bit", "signal n: Electrical; n = 1.0;", "n"),
        ("in a: Bit, out q: Bit", "k: f64 = 1.0; k = 2.0;", "k"),
        ("in a: Bit, out q: Bit", "let m = a; m = q;", "m"),
    ] {
        let src = format!("{items}module Top({header}) {{ {body} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0222"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
    // An `out` port and a signal of the module are its own to drive.
    let src = "module Top(in a: Bit, out q: Bit) { signal s: Bit; s = a; q = s; }";
    checked(&design_of(src), src);
}

#[test]
fn two_definitions_of_one_carrier_are_e0335() {
    // DIG-04 AC 9 (§8.2.20): two definitions that share a level. The second is the error.
    let src = "module Top(in a: Bit, in b: Bit, out q: Bit) { q = a; q = b; }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0335"]);
    assert_eq!(snippet(src, &d[0]), "q");
    assert_eq!(d[0].diagnostic.span().start, src.rfind("q = b").unwrap());
    // Two carriers are two drives.
    let src = "module Top(in a: Bit, out q: Bit, out r: Bit) { q = a; r = a; }";
    checked(&design_of(src), src);
}

#[test]
fn an_initial_value_under_an_undelayed_definition_is_e0338() {
    // DIG-04 AC 13 (§8.2.21): `out y: Bit = false` with `y = a & b;`; a delayed definition may
    // have an initial value (`out q: Bit = false` with `q = !(r | qn) after tpd;`).
    let src = "module Top(in a: Bit, in b: Bit, out y: Bit = false) { y = a & b; }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0338"]);
    assert_eq!(snippet(src, &d[0]), "y");
    let src = "module Top(in a: Bit, out y: Bit) { signal s: Bit = true; s = a; y = s; }";
    let d = diagnostics(src);
    assert_eq!((codes(&d), snippet(src, &d[0])), (vec!["E0338".to_string()], "s"));
    let src = "module Top(in a: Bit, in b: Bit, out q: Bit = false) { tpd: f64 = 1n; q = !(a | b) after tpd; }";
    checked(&design_of(src), src);
}

#[test]
fn a_loop_of_definitions_is_e0339() {
    // DIG-04 AC 16 (§8.2.3): undelayed definitions that depend on each other, directly or through
    // a `let`, are a loop through one-driver carriers: one `E0339` per loop. An `after`, or a chain
    // with no cycle, is none.
    for body in ["a = !b; b = !a;", "let n = !b; a = n; b = !a;", "a = !a;"] {
        let src = format!("module Top() {{ signal a: Bit; signal b: Bit; {body} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0339"], "{body}");
        assert_eq!(d[0].diagnostic.span().start, src.find("a =").unwrap(), "{body}: reported at the first definition of the loop");
    }
    let src = "module Top(in x: Bit) { signal a: Bit; signal b: Bit; a = !b after 1n; b = !a; }";
    checked(&design_of(src), src);
    let src = "module Top(in x: Bit) { signal a: Bit; signal b: Bit; a = !x; b = !a; }";
    checked(&design_of(src), src);
}

#[test]
fn a_loop_of_definitions_is_reported_beside_an_initial_value() {
    // digital~ce04e7a4 of the Book (chapter 7, part III §8.2.3): the latch with initial values and
    // no `after` is two `E0338` and the loop, `E0339`, at the first definition
    let src = "module Top(in s: Bit, in r: Bit, out q: Bit = false, out qn: Bit = true) { q = !(r | qn); qn = !(s | q); }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["E0338", "E0339", "E0338"], "{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}

// ---- carrier ports at a placement (T11) ----

/// The buck's `Controller` and `PowerStage` over children that keep only what the placement reads:
/// their ports, copied from the buck, and the params the parents set.
fn buck_tops() -> String {
    let header = |file: &str, name: &str| {
        let text = buck::item(file, &format!("pub module {name}("));
        let end = text.find("{").expect("a body");
        text[..end].trim_end().to_string()
    };
    let mut stubs = String::new();
    for (file, name, params) in [
        ("control.msl", "SetpointReg", ""),
        ("control.msl", "RefRamp", ""),
        ("control.msl", "Pid", ""),
        ("supervisor.msl", "Supervisor", ""),
        ("dpwm.msl", "Dpwm", ""),
        ("power.msl", "GateDriver", "tpd: f64 = 10n;"),
        ("power.msl", "PowerSwitch", "ron: f64 = 20m;"),
        ("power.msl", "Inductor", "l: f64; dcr: f64 = 0.0;"),
        ("power.msl", "CurrentSense", ""),
        ("power.msl", "Capacitor", "c: f64; esr: f64 = 1m;"),
    ] {
        stubs += &format!("{} {{ {params} }}\n", header(file, name).trim_start_matches("pub "));
    }
    format!(
        "{}\n{}\n{}\n{}\n{stubs}{}\n{}",
        buck::spec(),
        buck::item("supervisor.msl", "pub enum Mode"),
        buck::item("supervisor.msl", "pub enum Fault"),
        buck::item("control.msl", "pub fn target_codes"),
        buck::item("top.msl", "pub module Controller("),
        buck::item("power.msl", "pub module PowerStage(")
    )
}

fn checked_top(src: &str, top: &str) {
    let design = design_of(src);
    if let Err(d) = Types::check(&design, top) {
        panic!("{top}: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
}

#[test]
fn a_continuous_expression_at_an_in_port_is_e0316() {
    // DIG-04 AC 7 (§8.1.12): an expression at an `in` port is an anonymous definition and must be
    // discrete: a probe, `now()` and `analysis(Static)`, directly or through a `let`, are `E0316`
    // at the argument. A discrete expression is the positive case: `Controller` of the buck hands
    // `target_codes(setpoint.known_or(0))` to the ramp.
    let items = "module Child(in a: Bit<f64>) { }\n";
    for (body, at) in [
        ("let c = Child(v(p));", "v(p)"),
        ("let c = Child(now());", "now()"),
        ("let c = Child(if analysis(Static) { 1.0 } else { 0.0 });", "if analysis(Static) { 1.0 } else { 0.0 }"),
        ("let k = v(p) * 2.0; let c = Child(k);", "k"),
    ] {
        let src = format!("{items}module Top(p: Electrical) {{ {body} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0316"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
    let src = format!("{items}module Top(in x: Bit<f64>) {{ let c = Child(2.0 * x + 1.0); let d = Child(x); }}");
    checked_top(&src, "Top");
    checked_top(&buck_tops(), "Controller");
}

#[test]
fn a_second_drive_is_e0220() {
    // DIG-04 AC 11 (§5.2.1, §5.2.6): a carrier at two `out` ports, or at one while its module
    // drives it by a definition, has a second drive; the error is at the second connection.
    let items = "module Source(out q: Bit) { }\n";
    let src = format!("{items}module Top() {{ signal s: Bit; let a = Source(s); let b = Source(s); }}");
    let d = diagnostics(&src);
    assert_eq!(codes(&d), ["E0220"]);
    assert_eq!(d[0].diagnostic.span().start, src.rfind("s)").unwrap());
    let src = format!("{items}module Top(in x: Bit) {{ signal s: Bit; s = x; let a = Source(s); }}");
    let d = diagnostics(&src);
    assert_eq!((codes(&d), snippet(&src, &d[0])), (vec!["E0220".to_string()], "s"));
    // An `out` port of the parent handed down is the instance's drive; the parent defines nothing.
    let src = format!("{items}module Top(out q: Bit) {{ let a = Source(q); }}");
    checked_top(&src, "Top");
}

#[test]
fn an_initial_value_without_the_drive_is_e0223() {
    // DIG-04 AC 12 (§5.2.6): `signal up: Bit = false;` connected to an `out` port: the instance
    // holds the drive, so the declaring module may give no initial value. The initial value is
    // the `out` port's own (`out up: Bit = false`).
    let items = "module Source(out q: Bit = false) { }\n";
    let src = format!("{items}module Top() {{ signal up: Bit = false; let a = Source(up); }}");
    let d = diagnostics(&src);
    assert_eq!((codes(&d), snippet(&src, &d[0])), (vec!["E0223".to_string()], "up"));
    let src = format!("{items}module Top(out q: Bit = true) {{ let a = Source(q); }}");
    let d = diagnostics(&src);
    assert_eq!((codes(&d), snippet(&src, &d[0])), (vec!["E0223".to_string()], "q"));
    let src = format!("{items}module Top() {{ signal up: Bit; let a = Source(up); }}");
    checked_top(&src, "Top");
}

#[test]
fn an_out_port_takes_only_a_carrier_the_parent_hands_over() {
    // DIG-04 AC 14 (§8.1.11): an expression at an `out` port, and an `in` port of the parent, are
    // `E0314`. A signal of the parent and an `out` port of the parent are the carriers it hands
    // over: `PowerStage` of the buck hands its own signals to `GateDriver`.
    let items = "module Source(out q: Bit) { }\n";
    for (header, body, at) in [
        ("in x: Bit", "let a = Source(!x);", "!x"),
        ("in x: Bit", "let a = Source(true);", "true"),
        ("in x: Bit", "let a = Source(x);", "x"),
    ] {
        let src = format!("{items}module Top({header}) {{ {body} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0314"], "{body}");
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
    }
    checked_top(&buck_tops(), "PowerStage");
}

#[test]
fn a_carrier_of_another_value_type_is_e0251() {
    // DIG-04 AC 15 (§6.1.3, §8.1.10): a `Bit<u8>` carrier connected to a `Bit<u10>` port is `E0251`,
    // at an `in` and at an `out` port; a node and a carrier never connect (`E0250`, §6.1); an
    // expression of another type at an `in` port is a definition of the wrong type (`E0337`).
    let items = "module Wide(in a: Bit<u10>, out q: Bit<u10>) { }\n";
    for (body, at) in [("let c = Wide(narrow, wide);", "narrow"), ("let c = Wide(wide, narrow);", "narrow")] {
        let src = format!("{items}module Top(in narrow: Bit<u8>) {{ signal wide: Bit<u10>; {body} }}");
        let d = diagnostics(&src);
        assert_eq!(codes(&d), ["E0251"], "{body}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert_eq!(snippet(&src, &d[0]), at, "{body}");
        assert!(d[0].diagnostic.message.contains("u8") && d[0].diagnostic.message.contains("u10"), "{}", d[0].diagnostic.message);
    }
    let src = format!("{items}module Top(in narrow: Bit<u8>, p: Electrical) {{ signal wide: Bit<u10>; let c = Wide(p, wide); }}");
    let d = diagnostics(&src);
    assert_eq!((codes(&d), snippet(&src, &d[0])), (vec!["E0250".to_string()], "p"));
    let src = format!("module Node(p: Electrical) {{ }}\nmodule Top(in b: Bit) {{ let c = Node(b); }}");
    let d = diagnostics(&src);
    assert_eq!((codes(&d), snippet(&src, &d[0])), (vec!["E0250".to_string()], "b"));
    let src = format!("{items}module Top(in narrow: Bit<u8>) {{ signal wide: Bit<u10>; let c = Wide(true, wide); }}");
    let d = diagnostics(&src);
    assert_eq!((codes(&d), snippet(&src, &d[0])), (vec!["E0337".to_string()], "true"));
    // The same type connects, a literal takes the type of the port it is connected to.
    let src = format!("{items}module Top() {{ signal w: Bit<u10>; signal v: Bit<u10>; let c = Wide(5, w); let d = Wide(w, v); }}");
    checked_top(&src, "Top");
}

#[test]
fn a_probe_on_a_childs_private_node_is_e0244() {
    // §6 rule 7 (`E0244`): a probe or a contribution naming a node of a child that is not `pub`
    // cannot reach it. The same node under `pub`, and a port of the child, are accepted.
    let child = "module Child(p: Electrical) { signal hidden: Electrical; }\n";
    for (body, at) in [("i(x) +> v(c.hidden);", "hidden"), ("i(c.hidden) +> 1.0;", "hidden"), ("i(x) +> v(c.hidden, gnd);", "hidden")] {
        let src = format!("{child}module Step() {{ signal x: Electrical; let c = Child(x); }}\nanalog for Step {{ {body} }}");
        let design = Design::builder().source("top.msl", &src).build();
        assert_eq!(codes(design.diagnostics()), ["E0244"], "{body}");
        assert_eq!(snippet(&src, &design.diagnostics()[0]), at, "{body}");
        assert!(design.diagnostics()[0].diagnostic.message.contains("`hidden`"), "{}", design.diagnostics()[0].diagnostic.message);
    }
    for (child, body) in [("module Child(p: Electrical) { pub signal hidden: Electrical; }\n", "i(x) +> v(c.hidden);"), (child, "i(x) +> v(c.p);")] {
        let src = format!("{child}module Step() {{ signal x: Electrical; let c = Child(x); }}\nanalog for Step {{ {body} }}");
        assert!(design_of(&src).diagnostics().is_empty(), "{body}");
    }
}
