//! Discrete carriers and definitions of `piperine_lang::types` (feature `msl-digital`, DIG-04,
//! DIG-02 AC 1): the type of a `signal` and of a port of `Bit<T>`, what a carrier may hold
//! (`E0112`), initial values (`E0210`), the flag reads `known`, `known_or`, `is_known`, `high`
//! and `prev`, and a catalogue method applied to a carrier. Sources are written over the embedded
//! standard library, the buck's own enums read as written.

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
    // DIG-04 AC 2 (part I §2): a carried type without `Repr` is `E0112`, at the type: `&str`,
    // `()`, an `Option`, an enum that does not derive `Repr`, and a module.
    for (items, carried) in [
        ("", "&str"),
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
    assert_eq!(codes(&d), ["P0002"]);
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
    // DIG-02 AC 2: a method with no row names the method and the value type.
    let src = "module Top(in oc_count: Bit<u4>) { let n = oc_count.saturating_add(1); }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("saturating_add") && d[0].diagnostic.message.contains("u4"), "{}", d[0].diagnostic.message);
}

#[test]
fn a_single_variant_enum_cannot_be_carried() {
    // Edge case 3: a single-variant enum has a tag of zero levels (part I §2 rule 5): `P0001`.
    let src = "#[derive(Repr, Clone, Copy, PartialEq)]\nenum One { A }\nmodule Top() { signal s: Bit<One>; }";
    let d = diagnostics(src);
    assert_eq!(codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("single-variant enum `One`"), "{}", d[0].diagnostic.message);
    assert_eq!(snippet(src, &d[0]), "One");
    // Two variants are the smallest carried enum.
    let src = "#[derive(Repr, Clone, Copy, PartialEq)]\nenum Two { A, B }\nmodule Top() { signal s: Bit<Two>; }";
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
