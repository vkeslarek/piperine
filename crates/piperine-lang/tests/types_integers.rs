//! Integer types of `piperine_lang::types` (feature `msl-digital`, DIG-01, DIG-02): `UInt<N>`,
//! `Int<N>`, `uN`, `iN` and their `const` items; the type an integer literal takes from its
//! position (§1 rule 7); the range of a literal (`E0909`); the widths and floats outside the
//! subset (`P0001`, `E0103`, part I §1 and its error table). Expected types and codes are those
//! of `docs/spec/part_i_values.md`, cited at each assertion.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, ExprId, ExprKind, LitKind, Reflect};
use piperine_lang::types::{Ty, Types};

/// A design of `Top` with `body` as its analog block and `items` before it.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

fn checked<'d>(design: &'d Design, src: &str) -> Types<'d> {
    Types::check(design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()))
}

fn diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = source(items, body);
    let design = design_of(&src);
    let err = Types::check(&design, "Top").expect_err("expected diagnostics");
    (src, err)
}

fn binding(design: &Design, name: &str) -> DeclId {
    design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id()
}

/// The integer literal whose source text is exactly `text` (each test writes it once).
fn literal(design: &Design, src: &str, text: &str) -> ExprId {
    (0..design.expr_count())
        .map(|i| ExprId(i as u32))
        .find(|e| {
            let node = design.expr_node(*e);
            matches!(&node.kind, ExprKind::Lit(l) if matches!(l.kind, LitKind::Int(_))) && &src[node.span.range()] == text
        })
        .unwrap_or_else(|| panic!("no integer literal `{text}`"))
}

fn codes(d: &[DesignDiagnostic]) -> Vec<String> {
    d.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

#[test]
fn integer_types_and_constants_have_types() {
    // §1 table and rule 5: `uN`/`iN` are aliases of `UInt<N>`/`Int<N>`; the Resolution under §1:
    // `usize`/`isize` are `UInt<64>`/`Int<64>`.
    let items = "const N: u9 = 400;\nconst E_MIN: i16 = -256;\nconst W: UInt<12> = 2880;\nconst S: Int<32> = 4;\nconst ONE: u1 = 1;\nconst BIG: u64 = 18446744073709551615;\nconst NEG: i64 = -9223372036854775808;";
    let body = "let a = N; let b = E_MIN; let c = W; let d = S; let e = ONE; let f = BIG; let g = NEG; let h: usize = 3; let i: isize = 3; let j: u7 = 1;";
    let src = source(items, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"].iter().map(|n| types.local(binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(9), Ty::Int(16), Ty::UInt(12), Ty::Int(32), Ty::UInt(1), Ty::UInt(64), Ty::Int(64), Ty::UInt(64), Ty::Int(64), Ty::UInt(7)]);
    assert_eq!(Ty::UInt(9).name(&design), "u9");
    assert_eq!(Ty::Int(32).name(&design), "i32");

    // The constants of the buck (`examples/buck/src/spec.msl`) check as written.
    let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/buck/src/spec.msl")).expect("examples/buck/src/spec.msl");
    let body = "let a = N_PERIOD; let b = E_MIN; let c = KP_Q4; let d = HICCUP_SAMPLES; let e = PG_DEASSERT;";
    let src = source(&spec, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d", "e"].iter().map(|n| types.local(binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(9), Ty::Int(16), Ty::Int(32), Ty::UInt(11), Ty::UInt(3)]);
}

#[test]
fn a_literal_takes_the_expected_integer_type_else_i32() {
    // §1 rule 7, DIG-01 AC 2: the position's type, else `i32`. The positions this task types:
    // an annotation, a `const`, a parameter, a function result, the other arm of an `if` (on
    // either side), and a suffix; none gives `i32`.
    let items = "const C: u4 = 9;\nfn f(k: u3) -> u3 { k }\nfn g() -> u10 { 7 }";
    let body = "let a: u8 = 3; let p = f(5); let b: u9 = 11; let c = if true { 0 } else { b }; let d = if false { b } else { 2 }; let e = 4; let s = 5u7; let r = g(); let q = C;";
    let src = source(items, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let at = |text: &str| types.literal(literal(&design, &src, text)).cloned();
    assert_eq!(at("3"), Some(Ty::UInt(8)), "an annotation");
    assert_eq!(at("9"), Some(Ty::UInt(4)), "a `const` type");
    assert_eq!(at("5"), Some(Ty::UInt(3)), "a parameter");
    assert_eq!(at("7"), Some(Ty::UInt(10)), "a function result");
    assert_eq!(at("0"), Some(Ty::UInt(9)), "the other arm of an `if`, written after it");
    assert_eq!(at("2"), Some(Ty::UInt(9)), "the other arm of an `if`, written before it");
    assert_eq!(at("5u7"), Some(Ty::UInt(7)), "a suffix");
    assert_eq!(at("4"), Some(Ty::Int(32)), "no expected type: `i32`");
    assert_eq!(types.local(binding(&design, "c")), Some(&Ty::UInt(9)));
    assert_eq!(types.local(binding(&design, "e")), Some(&Ty::Int(32)));

    // A suffix of another type than the position's is a plain mismatch, never a conversion.
    let (src, d) = diagnostics("", "let a: u8 = 3u9;");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "3u9");
    assert!(d[0].diagnostic.message.contains("u8") && d[0].diagnostic.message.contains("u9"), "{}", d[0].diagnostic.message);
}

#[test]
fn a_literal_out_of_range_is_e0909() {
    // §1 rule 7: a literal out of its type's range is `E0909`; the range edges are not.
    let edges = source("const LO: i8 = -128;\nconst HI: i8 = 127;\nconst M: u8 = 255;", "let z: u1 = 1; let y = 2147483647;");
    checked(&design_of(&edges), &edges);

    for (items, body, text) in [
        ("", "let x: u8 = 256;", "256"),
        ("const C: i8 = -129;", "let x = C;", "129"),
        ("const C: i8 = 128;", "let x = C;", "128"),
        ("fn f(k: u3) -> u3 { k }", "let x = f(8);", "8"),
        ("", "let x = 2147483648;", "2147483648"),
    ] {
        let (src, d) = diagnostics(items, body);
        assert_eq!(codes(&d), ["E0909"], "{src}");
        assert_eq!(snippet(&src, &d[0]), text, "{src}");
    }
}

#[test]
fn an_unsupported_width_or_float_is_named() {
    // DIG-01 AC 9: an integer wider than 64 bits and every float but `f64` is `P0001` naming the
    // type, at the type as written: in a parameter, a result and a `let` (`ir-rust` F-B8 for
    // `Float<16>` and `Float<128>`, which the IR cannot hold).
    for (items, body, written) in [
        ("fn f(x: u128) -> f64 { 1.0 }", "let y = f(1);", "u128"),
        ("fn f() -> UInt<65> { 1 }", "let y = f();", "UInt<65>"),
        ("", "let y: Int<100> = 1;", "Int<100>"),
        ("", "let y: f32 = 1.0;", "f32"),
        ("fn f(x: f16) -> f64 { 1.0 }", "let y = f(1.0);", "f16"),
        ("fn f() -> f128 { 1.0 }", "let y = f();", "f128"),
        ("", "let y: Float<16> = 1.0;", "Float<16>"),
        ("fn f(x: Float<128>) -> f64 { 1.0 }", "let y = f(1.0);", "Float<128>"),
    ] {
        let (src, d) = diagnostics(items, body);
        assert_eq!(codes(&d), ["P0001"], "{src}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
        assert!(d[0].diagnostic.message.contains(&format!("`{written}`")), "{src}: {}", d[0].diagnostic.message);
        assert_eq!(snippet(&src, &d[0]), written, "{src}");
    }

    // `Float<64>` is `f64`.
    let ok = source("", "let y: Float<64> = 1.0; let z: f64 = y;");
    checked(&design_of(&ok), &ok);
}

#[test]
fn a_width_below_one_is_e0103() {
    // Part I error table: `E0103`, an integer width that is not `>= 1`, or a float width outside
    // {16, 32, 64, 128}.
    for (body, written) in [("let y: UInt<0> = 1;", "UInt<0>"), ("let y: Int<0> = 1;", "Int<0>"), ("let y: Float<8> = 1.0;", "Float<8>")] {
        let (src, d) = diagnostics("", body);
        assert_eq!(codes(&d), ["E0103"], "{src}");
        assert_eq!(snippet(&src, &d[0]), written, "{src}");
    }
}
