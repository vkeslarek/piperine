//! The number widths in the checker (composite-values phase D, CV-29 to CV-33): `f32`, `f16` and
//! `f128` (`Float<N>`, `fN`), integers of any width from 1 to 2^32 - 1 (`UInt<N>`, `uN`), a float
//! literal typed by its position, the range of a literal in its own format (`E0909`), the widths
//! outside the rules (`E0103`) and an `f32` where an `f64` is required (`E0180`). Expected types
//! and codes are those of `docs/spec/part_i_values.md` §1 and `appendix_a_lexical.md` A.8, cited at
//! each assertion.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

/// Designs of one `analog` body and what the tests ask of them.
struct Fixture;

impl Fixture {
    /// A design of `Top` with `body` as its analog block and `items` before it.
    fn source(items: &str, body: &str) -> String {
        format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
    }

    /// The design of `src`, which must parse and resolve.
    fn design_of(src: &str) -> Design {
        let design = support::build(src);
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
        design
    }

    /// The type of each of the locals `names`, the check of `items`/`body` being clean.
    fn types_of(items: &str, body: &str, names: &[&str]) -> Vec<Ty> {
        let src = Self::source(items, body);
        let design = Self::design_of(&src);
        let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", Self::messages(&d)));
        names.iter().map(|name| types.local(Self::binding(&design, name)).cloned().expect("typed")).collect()
    }

    /// The diagnostics of `items`/`body`, which must not check clean.
    fn diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
        let src = Self::source(items, body);
        let design = Self::design_of(&src);
        let found = Types::check(&design, "Top").expect_err("expected diagnostics");
        (src, found)
    }

    fn messages(found: &[DesignDiagnostic]) -> Vec<(String, String)> {
        found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }

    fn codes(found: &[DesignDiagnostic]) -> Vec<String> {
        found.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    fn binding(design: &Design, name: &str) -> DeclId {
        design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id()
    }

    /// The text a diagnostic points at.
    fn snippet<'a>(src: &'a str, found: &DesignDiagnostic) -> &'a str {
        &src[found.diagnostic.span().range()]
    }
}

#[test]
fn every_float_and_integer_width_type_checks() {
    // §1 rule 5: `fN` is an alias of `Float<N>`; `uN`/`iN` of `UInt<N>`/`Int<N>`; the width is any `N >= 1`
    let got = Fixture::types_of(
        "",
        "let a: f32 = 1.0; let b: f16 = 1.0; let c: f128 = 1.0; let d: Float<32> = 1.0; let e: Float<16> = 1.0; let f: Float<128> = 1.0; let g: Float<64> = 1.0; \
         let h: u128 = 1; let i: UInt<65> = 1; let j: Int<100> = 1; let k: UInt<200> = 1; let l: UInt<4294967295> = 1; let m: Int<129> = 1;",
        &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m"],
    );
    assert_eq!(got, [Ty::F32, Ty::F16, Ty::F128, Ty::F32, Ty::F16, Ty::F128, Ty::F64, Ty::UInt(128), Ty::UInt(65), Ty::Int(100), Ty::UInt(200), Ty::UInt(4_294_967_295), Ty::Int(129)]);
    // the checker's names of the new types
    let design = Fixture::design_of(&Fixture::source("", ""));
    assert_eq!([Ty::F32.name(&design), Ty::F16.name(&design), Ty::F128.name(&design), Ty::UInt(200).name(&design)], ["f32", "f16", "f128", "u200"]);

    // §1.4.2: a suffix gives a float literal its type, a position gives it the position's, `f64` otherwise
    let got = Fixture::types_of("fn half(x: f16) -> f16 { x }", "let a = 1.5f32; let b = 2f32; let c = half(1.5); let d = 1.5; let e: f32 = -1.5; let f: f128 = 1.0 + 2.0; let g = 1.5k;", &["a", "b", "c", "d", "e", "f", "g"]);
    assert_eq!(got, [Ty::F32, Ty::F32, Ty::F16, Ty::F64, Ty::F32, Ty::F128, Ty::F64], "a literal takes its position's float type; an SI-suffixed literal is always f64 (A.2 rule 1)");

    // the H.2 methods are the rows of every float type, the H.1 methods those of every integer type (§1 rule 5; CV-30, CV-31)
    let got = Fixture::types_of(
        "",
        "let a: f32 = 1.0; let b: f16 = 4.0; let c: f128 = 2.0; let d: UInt<200> = 7; let e: Int<100> = 7; \
         let sa = a.floor(); let sb = b.sqrt(); let sc = c.sin(); let sd = d.min(d); let se = e.abs(); let sf = a < a; let sg = c.is_nan();",
        &["sa", "sb", "sc", "sd", "se", "sf", "sg"],
    );
    assert_eq!(got, [Ty::F32, Ty::F16, Ty::F128, Ty::UInt(200), Ty::Int(100), Ty::Bool, Ty::Bool]);

    // §1.4.5: `as` converts between any two of the integer and float types, whatever the widths
    let got = Fixture::types_of("", "let a: f32 = 1.0; let b = a as f16; let c = b as f128; let d = c as UInt<200>; let e = d as f64; let f = 1.0 as f128; let g = 7u8 as Int<129>;", &["b", "c", "d", "e", "f", "g"]);
    assert_eq!(got, [Ty::F16, Ty::F128, Ty::UInt(200), Ty::F64, Ty::F128, Ty::Int(129)]);

    // the types of the arithmetic follow the operands
    let got = Fixture::types_of("", "let a: UInt<200> = 3; let b: f16 = 1.0; let s = a + a; let t = a * 2; let w = b * 2.0; let x = 2.0 + b;", &["s", "t", "w", "x"]);
    assert_eq!(got, [Ty::UInt(200), Ty::UInt(200), Ty::F16, Ty::F16]);
}

#[test]
fn a_width_outside_the_rules_is_e0103() {
    // Part I error table: an integer width below 1, a float width outside {16, 32, 64, 128}, and an
    // integer width past 2^32 - 1 (Appendix C.2, QD1); `E0103` names the type as written
    for (body, written) in [
        ("let y: UInt<0> = 1;", "UInt<0>"),
        ("let y: Int<0> = 1;", "Int<0>"),
        ("let y: Float<8> = 1.0;", "Float<8>"),
        ("let y: Float<256> = 1.0;", "Float<256>"),
        ("let y: UInt<4294967296> = 1;", "UInt<4294967296>"),
        ("let y: Int<99999999999> = 1;", "Int<99999999999>"),
    ] {
        let (src, found) = Fixture::diagnostics("", body);
        assert_eq!(Fixture::codes(&found), ["E0103"], "{src}: {:?}", Fixture::messages(&found));
        assert_eq!(Fixture::snippet(&src, &found[0]), written, "{src}");
    }
    let (_, found) = Fixture::diagnostics("", "let y: UInt<4294967296> = 1;");
    assert!(found[0].diagnostic.message.contains("2^32 - 1") && found[0].diagnostic.message.contains("C.2"), "{}", found[0].diagnostic.message);
    // a width that is a const expression folds as an array length folds (VD1)
    let got = Fixture::types_of("const A: usize = 64;", "let y: UInt<{ A + 1 }> = 1; let z: Int<{ A * 2 }> = 1;", &["y", "z"]);
    assert_eq!(got, [Ty::UInt(65), Ty::Int(128)], "§1 rule 5: the width of `UInt<N>` is any const expression `N >= 1`");
}

#[test]
fn a_literal_out_of_its_type_is_e0909() {
    // A.2 rule 8 read per format (QD5): a float literal whose nearest value in its own format is infinite,
    // and an integer literal past its type
    for (body, text) in [
        ("let y: f16 = 70000.0;", "70000.0"),
        ("let y = 1e39f32;", "1e39f32"),
        ("let y: f32 = 1e39;", "1e39"),
        ("let y: u8 = 256;", "256"),
    ] {
        let (src, found) = Fixture::diagnostics("", body);
        assert_eq!(Fixture::codes(&found), ["E0909"], "{src}: {:?}", Fixture::messages(&found));
        assert_eq!(Fixture::snippet(&src, &found[0]), text, "{src}");
    }
    // an integer literal past its own suffix is `E0909` of the parse phase (A.2 rule 8)
    for (body, text) in [("let y = 300u8;", "300u8"), ("let y = 0x1_0000_0000u32;", "0x1_0000_0000u32")] {
        let src = Fixture::source("", body);
        let design = support::build(&src);
        assert_eq!(support::codes(&design), ["E0909"], "{src}");
        assert!(design.diagnostics().iter().any(|d| src[d.diagnostic.span().range()] == *text), "{src}");
    }
    // the edges are values: the largest finite f16 and f32, a subnormal, and an integer at its maximum
    let got = Fixture::types_of("", "let a: f16 = 65504.0; let b = 3.4028235e38f32; let c: f16 = 1e-7; let d = 255u8; let e: UInt<200> = 1;", &["a", "b", "c", "d", "e"]);
    assert_eq!(got, [Ty::F16, Ty::F32, Ty::F16, Ty::UInt(8), Ty::UInt(200)]);
}

#[test]
fn an_f32_where_an_f64_is_required_is_a_mismatch() {
    // §1.4.3 and CV-29 AC 3: no implicit conversion; a contribution, a probe comparison and an `f64` parameter take
    // `f64` only, a float literal does not convert and an integer literal is no float
    let (src, found) = Fixture::diagnostics("fn takes(x: f64) -> f64 { x }", "let a: f32 = 1.0; let y = takes(a);");
    assert_eq!(Fixture::codes(&found), ["E0180"], "{src}: {:?}", Fixture::messages(&found));
    assert_eq!(Fixture::snippet(&src, &found[0]), "a");
    let (src, found) = Fixture::diagnostics("", "let a: f32 = 1.0; let b: f16 = 1.0; let y = a + b;");
    assert_eq!(Fixture::codes(&found), ["E0180"], "{src}: {:?}", Fixture::messages(&found));
    let (src, found) = Fixture::diagnostics("", "let a: f32 = 1;");
    assert_eq!(Fixture::codes(&found), ["E0180"], "§1 rule 7: an integer literal never converts: {src}: {:?}", Fixture::messages(&found));
    let (src, found) = Fixture::diagnostics("", "let a: f64 = 1.0; let b: f32 = a;");
    assert_eq!(Fixture::codes(&found), ["E0180"], "{src}: {:?}", Fixture::messages(&found));
}
