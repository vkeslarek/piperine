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

/// The text of `fn name` in `file` of `examples/buck/src`, from `fn` to its closing brace.
fn buck_fn(file: &str, name: &str) -> String {
    let path = format!("{}/../../examples/buck/src/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let start = text.find(&format!("fn {name}(")).unwrap_or_else(|| panic!("no fn {name} in {file}"));
    let mut depth = 0usize;
    for (i, c) in text[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return text[start..start + i + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("fn {name} in {file} does not close")
}

/// The constants of `examples/buck/src/spec.msl`, as items.
fn buck_spec() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/buck/src/spec.msl")).expect("examples/buck/src/spec.msl")
}

#[test]
fn integer_operators_take_one_type() {
    // DIG-01 AC 4: `+ - * / %`, unary `-`, `& | ^ !` and the comparisons on operands of one
    // integer type; arithmetic and bit operators give that type, comparisons `bool`.
    for op in ["+", "-", "*", "/", "%", "&", "|", "^"] {
        let items = format!("fn f(a: u9, b: u9) -> u9 {{ a {op} b }}\nfn g(a: i32, b: i32) -> i32 {{ a {op} b }}");
        let src = source(&items, "let x = f(1, 2); let y = g(3, 4);");
        let design = design_of(&src);
        let types = checked(&design, &src);
        assert_eq!(types.local(binding(&design, "x")), Some(&Ty::UInt(9)), "{op}");
    }
    for op in ["<", "<=", ">", ">=", "==", "!="] {
        let src = source("", &format!("let a: u10 = 5; let b: u10 = 6; let c = a {op} b;"));
        let design = design_of(&src);
        let types = checked(&design, &src);
        assert_eq!(types.local(binding(&design, "c")), Some(&Ty::Bool), "{op}");
    }
    let src = source("", "let a: i8 = 5; let n = -a; let m = !a; let k: u4 = 3; let l = !k;");
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["n", "m", "l"].iter().map(|n| types.local(binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::Int(8), Ty::Int(8), Ty::UInt(4)]);

    // DIG-01 AC 2, the binary-operand position: a literal takes the other operand's type, on
    // either side; an expected type reaches both operands of `+`.
    let src = source("", "let count: u9 = 7; let a = count + 11; let b = 13 + count; let c = count == 17; let d: u12 = 19 + 23;");
    let design = design_of(&src);
    let types = checked(&design, &src);
    let at = |text: &str| types.literal(literal(&design, &src, text)).cloned();
    assert_eq!([at("11"), at("13"), at("17"), at("19"), at("23")], [Some(Ty::UInt(9)), Some(Ty::UInt(9)), Some(Ty::UInt(9)), Some(Ty::UInt(12)), Some(Ty::UInt(12))]);
    assert_eq!(types.local(binding(&design, "b")), Some(&Ty::UInt(9)));

    // The integer functions of the buck, read from `examples/buck/src`, check as written and
    // give their declared results.
    let items = format!(
        "{}\n{}\n{}\n{}\n{}",
        buck_spec(),
        buck_fn("dpwm.msl", "rev3"),
        buck_fn("dpwm.msl", "dithered_width"),
        buck_fn("control.msl", "target_codes"),
        buck_fn("supervisor.msl", "ovp_threshold"),
    );
    let src = source(&items, "let a = rev3(1); let b = dithered_width(1325, 3); let c = target_codes(5); let d = ovp_threshold(200);");
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d"].iter().map(|n| types.local(binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(3), Ty::UInt(9), Ty::UInt(10), Ty::UInt(10)]);
}

#[test]
fn mixed_integer_operands_are_p0002() {
    // DIG-01 AC 5: two operands of different types are a plain mismatch, naming both.
    for (body, at) in [
        ("let a: u9 = 1; let b: u12 = 2; let c = a + b;", "b"),
        ("let a: u9 = 1; let b: u12 = 2; let c = a < b;", "b"),
        ("let a: u9 = 1; let b: i9 = 2; let c = a & b;", "b"),
        ("let a: u9 = 1; let b = 2.0; let c = a == b;", "b"),
        ("let a: u9 = 1; let c = a * 2.0;", "2.0"),
    ] {
        let (src, d) = diagnostics("", body);
        assert_eq!(codes(&d), ["P0002"], "{src}");
        assert_eq!(snippet(&src, &d[0]), at, "{src}");
    }
    let (_, d) = diagnostics("", "let a: u9 = 1; let b: u12 = 2; let c = a + b;");
    let message = &d[0].diagnostic.message;
    assert!(message.contains("`u9`") && message.contains("`u12`"), "{message}");
}

#[test]
fn a_shift_takes_any_integer_amount() {
    // DIG-01 AC 6: the left operand is an integer, the amount any integer type, the result the
    // left type; a bare literal amount expects nothing and is `i32`.
    let src = source("", "let x: u10 = 3; let s: i32 = 2; let k: u3 = 1; let a = x << s; let b = x >> k; let c = x << 5; let d = (x >> 7) & 9;");
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d"].iter().map(|n| types.local(binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(10), Ty::UInt(10), Ty::UInt(10), Ty::UInt(10)]);
    assert_eq!(types.literal(literal(&design, &src, "5")), Some(&Ty::Int(32)));
    assert_eq!(types.literal(literal(&design, &src, "9")), Some(&Ty::UInt(10)), "the other operand of `&`");

    let (src, d) = diagnostics("", "let x: u10 = 3; let a = x << true;");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "true");
    let (_, d) = diagnostics("", "let x = 1.0; let a = x << 2;");
    assert_eq!(codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("`<<`"), "{}", d[0].diagnostic.message);
}

#[test]
fn as_goes_between_integers_and_f64() {
    // DIG-01 AC 7: `as` between any two of the integer types and `f64`.
    let types_ = ["u1", "u8", "u64", "i16", "i64", "f64"];
    for from in types_ {
        for to in types_ {
            let src = source(&format!("fn f(x: {from}) -> {to} {{ x as {to} }}"), "let y = 0;");
            let design = design_of(&src);
            checked(&design, &src);
        }
    }
    let src = source("", "let duty: u12 = 1325; let w = (duty >> 3) as u9; let v = w as f64; let b = true as u1;");
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["w", "v", "b"].iter().map(|n| types.local(binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(9), Ty::F64, Ty::UInt(1)]);

    let (src, d) = diagnostics("", "let x = 1.0; let b = x as bool;");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "x");
}
