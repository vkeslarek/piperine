//! Integer types of `piperine_lang::types` (feature `msl-digital`, DIG-01, DIG-02): `UInt<N>`,
//! `Int<N>`, `uN`, `iN` and their `const` items; the type an integer literal takes from its
//! position (§1 rule 7); the range of a literal (`E0909`); the widths and floats outside the
//! subset (`P0001`, `E0103`, part I §1 and its error table). Expected types and codes are those
//! of `docs/spec/part_i_values.md`, cited at each assertion.


use piperine_lang::pom::{DeclId, Design, ExprId, ExprKind, LitKind, Reflect};
use piperine_lang::types::{Ty, Types};
use piperine_lang_fixtures::buck::BuckSource;
use piperine_lang_fixtures::checked::CheckedSource;

/// The integer literals of a design, found by their source text.
struct IntegerLiteral;

impl IntegerLiteral {
    /// The integer literal whose source text is exactly `text` (each test writes it once).
    fn find(design: &Design, src: &str, text: &str) -> ExprId {
        (0..design.expr_count())
            .map(|i| ExprId(i as u32))
            .find(|e| {
                let node = design.expr_node(*e);
                matches!(&node.kind, ExprKind::Lit(l) if matches!(l.kind, LitKind::Int(_))) && &src[node.span.range()] == text
            })
            .unwrap_or_else(|| panic!("no integer literal `{text}`"))
    }
}

/// The links the checker recorded for the calls of a design.
struct CalleeLinks;

impl CalleeLinks {
    /// Asserts every call and method call inside a declaration `types` reached (a module and its
    /// `analog`/`digital` blocks, a function, a constant) is linked; gives how many were seen and
    /// how many of them are built-ins.
    fn assert_linked(design: &Design, types: &Types<'_>) -> (usize, usize) {
        let reached: std::collections::BTreeSet<DeclId> = types.reached().iter().copied().collect();
        let in_reached = |e: ExprId| {
            let mut decl = Some(design.expr_node(e).owner);
            while let Some(d) = decl {
                if reached.contains(&d) || design.decl(d).as_behavior().and_then(|b| b.module()).is_some_and(|m| reached.contains(&m.decl().id())) {
                    return true;
                }
                decl = design.decl_data(d).parent;
            }
            false
        };
        let (mut seen, mut builtins) = (0, 0);
        for e in (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| in_reached(*e)) {
            let text = design.expr(e).text();
            match &design.expr_node(e).kind {
                ExprKind::Call { callee, class: piperine_lang::pom::CallClass::Probe, .. } => {
                    let ExprKind::Path(path) = &design.expr_node(*callee).kind else { panic!("a probe through a path") };
                    let res = design.path_node(*path).segments.last().map(|s| s.res);
                    assert!(matches!(res, Some(piperine_lang::pom::Resolution::Decl(_) | piperine_lang::pom::Resolution::Builtin(_))), "{text}");
                }
                // `Some(x)` is linked as a probe is, by its own path, to the built-in variant
                ExprKind::Call { callee, .. } if matches!(&design.expr_node(*callee).kind, ExprKind::Path(path) if matches!(design.path_node(*path).segments.last().map(|s| s.res), Some(piperine_lang::pom::Resolution::Builtin(id)) if id.info().name == "Some")) => {}
                ExprKind::Call { .. } => assert!(types.callee(e).is_some() || types.builtin(e).is_some(), "no callee for `{text}`"),
                ExprKind::MethodCall { .. } => assert!(types.method(e).is_some() || types.builtin(e).is_some(), "no row for `{text}`"),
                _ => continue,
            }
            seen += 1;
            builtins += usize::from(types.builtin(e).is_some());
        }
        (seen, builtins)
    }
}

#[test]
fn integer_types_and_constants_have_types() {
    // §1 table and rule 5: `uN`/`iN` are aliases of `UInt<N>`/`Int<N>`; the Resolution under §1:
    // `usize`/`isize` are `UInt<64>`/`Int<64>`.
    let items = "const N: u9 = 400;\nconst E_MIN: i16 = -256;\nconst W: UInt<12> = 2880;\nconst S: Int<32> = 4;\nconst ONE: u1 = 1;\nconst BIG: u64 = 18446744073709551615;\nconst NEG: i64 = -9223372036854775808;";
    let body = "let a = N; let b = E_MIN; let c = W; let d = S; let e = ONE; let f = BIG; let g = NEG; let h: usize = 3; let i: isize = 3; let j: u7 = 1;";
    let src = CheckedSource::source(items, body);
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"].iter().map(|n| types.local(CheckedSource::binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(9), Ty::Int(16), Ty::UInt(12), Ty::Int(32), Ty::UInt(1), Ty::UInt(64), Ty::Int(64), Ty::UInt(64), Ty::Int(64), Ty::UInt(7)]);
    assert_eq!(Ty::UInt(9).name(&design), "u9");
    assert_eq!(Ty::Int(32).name(&design), "i32");

    // The constants of the buck (`examples/buck/src/spec.msl`) check as written.
    let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/buck/src/spec.msl")).expect("examples/buck/src/spec.msl");
    let body = "let a = N_PERIOD; let b = E_MIN; let c = KP_Q4; let d = HICCUP_SAMPLES; let e = PG_DEASSERT;";
    let src = CheckedSource::source(&spec, body);
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d", "e"].iter().map(|n| types.local(CheckedSource::binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(9), Ty::Int(16), Ty::Int(32), Ty::UInt(11), Ty::UInt(3)]);
}

#[test]
fn a_literal_takes_the_expected_integer_type_else_i32() {
    // §1 rule 7, DIG-01 AC 2: the position's type, else `i32`. The positions this task types:
    // an annotation, a `const`, a parameter, a function result, the other arm of an `if` (on
    // either side), and a suffix; none gives `i32`.
    let items = "const C: u4 = 9;\nfn f(k: u3) -> u3 { k }\nfn g() -> u10 { 7 }";
    let body = "let a: u8 = 3; let p = f(5); let b: u9 = 11; let c = if true { 0 } else { b }; let d = if false { b } else { 2 }; let e = 4; let s = 5u7; let r = g(); let q = C;";
    let src = CheckedSource::source(items, body);
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let at = |text: &str| types.literal(IntegerLiteral::find(&design, &src, text)).cloned();
    assert_eq!(at("3"), Some(Ty::UInt(8)), "an annotation");
    assert_eq!(at("9"), Some(Ty::UInt(4)), "a `const` type");
    assert_eq!(at("5"), Some(Ty::UInt(3)), "a parameter");
    assert_eq!(at("7"), Some(Ty::UInt(10)), "a function result");
    assert_eq!(at("0"), Some(Ty::UInt(9)), "the other arm of an `if`, written after it");
    assert_eq!(at("2"), Some(Ty::UInt(9)), "the other arm of an `if`, written before it");
    assert_eq!(at("5u7"), Some(Ty::UInt(7)), "a suffix");
    assert_eq!(at("4"), Some(Ty::Int(32)), "no expected type: `i32`");
    assert_eq!(types.local(CheckedSource::binding(&design, "c")), Some(&Ty::UInt(9)));
    assert_eq!(types.local(CheckedSource::binding(&design, "e")), Some(&Ty::Int(32)));

    // A suffix of another type than the position's is a plain mismatch, never a conversion.
    let (src, d) = CheckedSource::body_diagnostics("", "let a: u8 = 3u9;");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "3u9");
    assert!(d[0].diagnostic.message.contains("u8") && d[0].diagnostic.message.contains("u9"), "{}", d[0].diagnostic.message);
}

#[test]
fn a_literal_out_of_range_is_e0909() {
    // §1 rule 7: a literal out of its type's range is `E0909`; the range edges are not.
    let edges = CheckedSource::source("const LO: i8 = -128;\nconst HI: i8 = 127;\nconst M: u8 = 255;", "let z: u1 = 1; let y = 2147483647;");
    CheckedSource::checked(&CheckedSource::design_of(&edges), &edges);

    for (items, body, text) in [
        ("", "let x: u8 = 256;", "256"),
        ("const C: i8 = -129;", "let x = C;", "129"),
        ("const C: i8 = 128;", "let x = C;", "128"),
        ("fn f(k: u3) -> u3 { k }", "let x = f(8);", "8"),
        ("", "let x = 2147483648;", "2147483648"),
    ] {
        let (src, d) = CheckedSource::body_diagnostics(items, body);
        assert_eq!(CheckedSource::codes(&d), ["E0909"], "{src}");
        assert_eq!(CheckedSource::snippet(&src, &d[0]), text, "{src}");
    }
}

#[test]
fn a_width_below_one_is_e0103() {
    // Part I error table: `E0103`, an integer width that is not `>= 1`, or a float width outside
    // {16, 32, 64, 128}.
    for (body, written) in [("let y: UInt<0> = 1;", "UInt<0>"), ("let y: Int<0> = 1;", "Int<0>"), ("let y: Float<8> = 1.0;", "Float<8>")] {
        let (src, d) = CheckedSource::body_diagnostics("", body);
        assert_eq!(CheckedSource::codes(&d), ["E0103"], "{src}");
        assert_eq!(CheckedSource::snippet(&src, &d[0]), written, "{src}");
    }
}

#[test]
fn integer_operators_take_one_type() {
    // DIG-01 AC 4: `+ - * / %`, unary `-`, `& | ^ !` and the comparisons on operands of one
    // integer type; arithmetic and bit operators give that type, comparisons `bool`.
    for op in ["+", "-", "*", "/", "%", "&", "|", "^"] {
        let items = format!("fn f(a: u9, b: u9) -> u9 {{ a {op} b }}\nfn g(a: i32, b: i32) -> i32 {{ a {op} b }}");
        let src = CheckedSource::source(&items, "let x = f(1, 2); let y = g(3, 4);");
        let design = CheckedSource::design_of(&src);
        let types = CheckedSource::checked(&design, &src);
        assert_eq!(types.local(CheckedSource::binding(&design, "x")), Some(&Ty::UInt(9)), "{op}");
    }
    for op in ["<", "<=", ">", ">=", "==", "!="] {
        let src = CheckedSource::source("", &format!("let a: u10 = 5; let b: u10 = 6; let c = a {op} b;"));
        let design = CheckedSource::design_of(&src);
        let types = CheckedSource::checked(&design, &src);
        assert_eq!(types.local(CheckedSource::binding(&design, "c")), Some(&Ty::Bool), "{op}");
    }
    let src = CheckedSource::source("", "let a: i8 = 5; let n = -a; let m = !a; let k: u4 = 3; let l = !k;");
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let got: Vec<Ty> = ["n", "m", "l"].iter().map(|n| types.local(CheckedSource::binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::Int(8), Ty::Int(8), Ty::UInt(4)]);

    // DIG-01 AC 2, the binary-operand position: a literal takes the other operand's type, on
    // either side; an expected type reaches both operands of `+`.
    let src = CheckedSource::source("", "let count: u9 = 7; let a = count + 11; let b = 13 + count; let c = count == 17; let d: u12 = 19 + 23;");
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let at = |text: &str| types.literal(IntegerLiteral::find(&design, &src, text)).cloned();
    assert_eq!([at("11"), at("13"), at("17"), at("19"), at("23")], [Some(Ty::UInt(9)), Some(Ty::UInt(9)), Some(Ty::UInt(9)), Some(Ty::UInt(12)), Some(Ty::UInt(12))]);
    assert_eq!(types.local(CheckedSource::binding(&design, "b")), Some(&Ty::UInt(9)));

    // The integer functions of the buck, read from `examples/buck/src`, check as written and
    // give their declared results.
    let items = format!(
        "{}\n{}\n{}\n{}\n{}",
        BuckSource::spec(),
        BuckSource::function("dpwm.msl", "rev3"),
        BuckSource::function("dpwm.msl", "dithered_width"),
        BuckSource::function("control.msl", "target_codes"),
        BuckSource::function("supervisor.msl", "ovp_threshold"),
    );
    let src = CheckedSource::source(&items, "let a = rev3(1); let b = dithered_width(1325, 3); let c = target_codes(5); let d = ovp_threshold(200);");
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d"].iter().map(|n| types.local(CheckedSource::binding(&design, n)).cloned().expect("typed")).collect();
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
        let (src, d) = CheckedSource::body_diagnostics("", body);
        assert_eq!(CheckedSource::codes(&d), ["E0180"], "{src}");
        assert_eq!(CheckedSource::snippet(&src, &d[0]), at, "{src}");
    }
    let (_, d) = CheckedSource::body_diagnostics("", "let a: u9 = 1; let b: u12 = 2; let c = a + b;");
    let message = &d[0].diagnostic.message;
    assert!(message.contains("`u9`") && message.contains("`u12`"), "{message}");
}

#[test]
fn a_shift_takes_any_integer_amount() {
    // DIG-01 AC 6: the left operand is an integer, the amount any integer type, the result the
    // left type; a bare literal amount expects nothing and is `i32`.
    let src = CheckedSource::source("", "let x: u10 = 3; let s: i32 = 2; let k: u3 = 1; let a = x << s; let b = x >> k; let c = x << 5; let d = (x >> 7) & 9;");
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "d"].iter().map(|n| types.local(CheckedSource::binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(10), Ty::UInt(10), Ty::UInt(10), Ty::UInt(10)]);
    assert_eq!(types.literal(IntegerLiteral::find(&design, &src, "5")), Some(&Ty::Int(32)));
    assert_eq!(types.literal(IntegerLiteral::find(&design, &src, "9")), Some(&Ty::UInt(10)), "the other operand of `&`");

    let (src, d) = CheckedSource::body_diagnostics("", "let x: u10 = 3; let a = x << true;");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "true");
    let (_, d) = CheckedSource::body_diagnostics("", "let x = 1.0; let a = x << 2;");
    assert_eq!(CheckedSource::codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("`<<`"), "{}", d[0].diagnostic.message);
}

#[test]
fn as_goes_between_integers_and_f64() {
    // DIG-01 AC 7: `as` between any two of the integer types and `f64`.
    let types_ = ["u1", "u8", "u64", "i16", "i64", "f64"];
    for from in types_ {
        for to in types_ {
            let src = CheckedSource::source(&format!("fn f(x: {from}) -> {to} {{ x as {to} }}"), "let y = 0;");
            let design = CheckedSource::design_of(&src);
            CheckedSource::checked(&design, &src);
        }
    }
    let src = CheckedSource::source("", "let duty: u12 = 1325; let w = (duty >> 3) as u9; let v = w as f64; let b = true as u1;");
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let got: Vec<Ty> = ["w", "v", "b"].iter().map(|n| types.local(CheckedSource::binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(9), Ty::F64, Ty::UInt(1)]);

    let (src, d) = CheckedSource::body_diagnostics("", "let x = 1.0; let b = x as bool;");
    // §1.4.5: `f64` to `bool` is no row of the table, so `E0182` (it was `E0180`).
    assert_eq!(CheckedSource::codes(&d), ["E0182"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "x");
}

#[test]
fn min_max_clamp_and_floor_are_typed() {
    // DIG-01 AC 8: `min`, `max`, `clamp` on the integer types and `f64`, `floor` on `f64`, each
    // giving the receiver's type (Appendix H); a literal argument takes the receiver's type
    // (DIG-01 AC 2, the receiver position).
    let body = "let d: u8 = 200; let a = d.min(168); let e: i16 = 3; let b = e.clamp(-256, 255); let w: u12 = 9; let c = w.max(31); \
                let x = 2.5; let f = x.floor(); let g = x.min(1.0); let h = x.clamp(0.0, 1023.0);";
    let src = CheckedSource::source("", body);
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    let got: Vec<Ty> = ["a", "b", "c", "f", "g", "h"].iter().map(|n| types.local(CheckedSource::binding(&design, n)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(8), Ty::Int(16), Ty::UInt(12), Ty::F64, Ty::F64, Ty::F64]);
    let at = |text: &str| types.literal(IntegerLiteral::find(&design, &src, text)).cloned();
    assert_eq!([at("168"), at("256"), at("255"), at("31")], [Some(Ty::UInt(8)), Some(Ty::Int(16)), Some(Ty::Int(16)), Some(Ty::UInt(12))]);
    let calls: Vec<_> = (0..design.expr_count()).map(|i| ExprId(i as u32)).filter(|e| matches!(design.expr_node(*e).kind, ExprKind::MethodCall { .. })).collect();
    let rows: Vec<_> = calls.iter().map(|e| types.method(*e)).collect();
    use piperine_lang::types::Method;
    assert_eq!(rows, [Some(Method::Min), Some(Method::Clamp), Some(Method::Max), Some(Method::Floor), Some(Method::Min), Some(Method::Clamp)]);

    // `floor` is a row of `f64` only; an argument of another type is a mismatch.
    let (src, d) = CheckedSource::body_diagnostics("", "let d: u8 = 2; let a = d.floor();");
    assert_eq!(CheckedSource::codes(&d), ["E0161"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "floor");
    let (src, d) = CheckedSource::body_diagnostics("", "let d: u8 = 2; let b: u9 = 3; let a = d.min(b);");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "b");
}

#[test]
fn an_uncatalogued_method_is_e0161_by_name() {
    // DIG-02 AC 2 (buck GAPS.md G3), as Appendix H rule 1 now closes it: a method with no row of
    // its receiver's complete catalogue is `E0161` naming the method and the type, never an
    // unresolved call that runs as nothing.
    let (src, d) = CheckedSource::body_diagnostics("", "let x: u4 = 3; let y = x.leading_zeros();");
    assert_eq!(CheckedSource::codes(&d), ["E0161"], "{src}");
    let message = &d[0].diagnostic.message;
    assert!(message.contains("leading_zeros") && message.contains("`u4`"), "{message}");
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "leading_zeros");
}

#[test]
fn every_accepted_call_has_a_callee() {
    // DIG-02 AC 3: every call and method call of an accepted model has a callee (a function, a
    // core primitive, a module), a catalogue row or a built-in (a carrier's flag read or edge,
    // an `Option` method); a probe and `Some(x)` are linked by their own path to their quantity
    // or built-in. Every
    // call the checker reached is visited - the ones it left untyped too - in three small models
    // and in `Buck`, whose carriers, enums, flag reads and events the small ones lack.
    let models = [
        CheckedSource::source(&format!("{}\n{}\n{}", BuckSource::spec(), BuckSource::function("dpwm.msl", "rev3"), BuckSource::function("dpwm.msl", "dithered_width")), "let a = dithered_width(1325, 3); let b = (1.5).floor().min(2.0);"),
        "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
         module R(p: E, n: E) { r: f64 = 1k; }\nanalog for R { i(p, n) +> v(p, n) / r + ddt(1p * v(p, n)); }\n\
         module Top(a: E) { let r = R { r: 2k }(a, gnd); }"
            .to_string(),
        CheckedSource::source("fn sq(x: f64) -> f64 { x * x }\nconst K: u8 = 3;", "let a = sq(2.0).exp(); let b = K.clamp(1, 2); let c = temperature();"),
    ];
    for src in &models {
        let design = CheckedSource::design_of(src);
        let types = CheckedSource::checked(&design, src);
        let (seen, _) = CalleeLinks::assert_linked(&design, &types);
        assert!(seen >= 3, "the model holds calls: {src}");
    }
    let design = BuckSource::design();
    let types = Types::check(&design, "Buck").unwrap_or_else(|d| panic!("Buck checks: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    let (seen, builtins) = CalleeLinks::assert_linked(&design, &types);
    assert!(seen >= 100, "Buck holds many calls: {seen}");
    assert!(builtins >= 10, "Buck reads flags and edges of carriers through built-ins: {builtins}");
}

#[test]
fn every_h1_row_types_with_its_signature() {
    // CAT-02 AC 1 (Appendix H.1): 13 rows on `UInt<N>` and `Int<N>`; `pow` takes a `u32`;
    // `checked_*` give `Option<Self>`; every other row gives `Self`.
    use piperine_lang::types::Method::*;
    type Row = (&'static str, piperine_lang::types::Method, fn(Ty) -> Ty);
    let same: fn(Ty) -> Ty = |t| t;
    let option: fn(Ty) -> Ty = |t| Ty::Option(Box::new(t));
    let rows: [Row; 13] = [
        ("abs()", IntAbs, same),
        ("min(y)", Min, same),
        ("max(y)", Max, same),
        ("clamp(y, y)", Clamp, same),
        ("pow(3)", Pow, same),
        ("rem_euclid(y)", RemEuclid, same),
        ("signum()", Signum, same),
        ("checked_add(y)", CheckedAdd, option),
        ("checked_sub(y)", CheckedSub, option),
        ("checked_mul(y)", CheckedMul, option),
        ("saturating_add(y)", SaturatingAdd, same),
        ("saturating_sub(y)", SaturatingSub, same),
        ("saturating_mul(y)", SaturatingMul, same),
    ];
    // `abs` and `signum` exist on the signed type; every row on `i8`, 12 of them on `u8`.
    for (decl, ty) in [("i8", Ty::Int(8)), ("u8", Ty::UInt(8))] {
        let signed = decl == "i8";
        let rows: Vec<&Row> = rows.iter().filter(|(call, ..)| signed || !(call.starts_with("abs") || call.starts_with("signum"))).collect();
        assert_eq!(rows.len(), if signed { 13 } else { 11 });
        let lets: String = rows.iter().enumerate().map(|(i, (call, ..))| format!("let r{i} = x.{call}; ")).collect();
        let src = CheckedSource::source("", &format!("let x: {decl} = 5; let y: {decl} = 3; {lets}"));
        let design = CheckedSource::design_of(&src);
        let types = CheckedSource::checked(&design, &src);
        for (i, (call, method, result)) in rows.iter().enumerate() {
            assert_eq!(types.local(CheckedSource::binding(&design, &format!("r{i}"))), Some(&result(ty.clone())), "{decl}.{call}");
            let at = (0..design.expr_count()).map(|e| ExprId(e as u32)).find(|e| matches!(design.expr_node(*e).kind, ExprKind::MethodCall { .. }) && src[design.expr_node(*e).span.range()] == format!("x.{call}")).unwrap_or_else(|| panic!("{call}"));
            assert_eq!(types.method(at), Some(*method), "{decl}.{call}");
        }
    }
    // `pow` takes a `u32`: an `i32` exponent is a mismatch.
    let (src, d) = CheckedSource::body_diagnostics("", "let x: u8 = 2; let e: i32 = 3; let r = x.pow(e);");
    assert_eq!(CheckedSource::codes(&d), ["E0180"]);
    assert_eq!(CheckedSource::snippet(&src, &d[0]), "e");
}

#[test]
fn abs_and_signum_on_an_unsigned_integer_are_e0161() {
    // CAT-02 AC 7 (H.1: "signed types only").
    for name in ["abs", "signum"] {
        let (src, d) = CheckedSource::body_diagnostics("", &format!("let x: u8 = 5; let y = x.{name}();"));
        assert_eq!(CheckedSource::codes(&d), ["E0161"], "{name}");
        let message = &d[0].diagnostic.message;
        assert!(message.contains(name) && message.contains("`u8`"), "{message}");
        assert_eq!(CheckedSource::snippet(&src, &d[0]), name);
    }
}

#[test]
fn a_bare_literal_takes_the_type_its_later_uses_fix() {
    // §1.4.4: where the initializer does not fix its own type (a literal without a suffix), the
    // uses of the binding in the same body fix it. Each form of use below is such a use.
    let typed = |body: &str, name: &str| {
        let src = CheckedSource::source("fn k() -> u32 { 7 }", body);
        let design = CheckedSource::design_of(&src);
        let types = CheckedSource::checked(&design, &src);
        types.local(CheckedSource::binding(&design, name)).cloned().expect("typed")
    };
    // a compound assignment with a `u32` operand (the Book's `sum_below`)
    assert_eq!(typed("let total = 0; let n: u32 = 4; total += n;", "total"), Ty::UInt(32));
    // a plain assignment of a `u8`
    assert_eq!(typed("let t = 0; let n: u8 = 4; t = n;", "t"), Ty::UInt(8));
    // an operator and a comparison with a typed operand, either side
    assert_eq!(typed("let t = 1; let n: u16 = 4; let s = n + t;", "t"), Ty::UInt(16));
    assert_eq!(typed("let t = 1; let n: i64 = 4; let s = t < n;", "t"), Ty::Int(64));
    // an argument of a call, and the value of a block checked against the function result
    assert_eq!(typed("let t = 1; let s: u32 = t;", "t"), Ty::UInt(32));
    // with no use that requires a type, the default `i32` (§1.4.2)
    assert_eq!(typed("let t = 1; let s = t + 1;", "t"), Ty::Int(32));
    // the loop of `sum_below`, reached from a function result
    let src = CheckedSource::source("fn sum_below(n: u32) -> u32 { let total = 0; for k in 0..n { total += k; } total }", "let s = sum_below(4);");
    let design = CheckedSource::design_of(&src);
    let types = CheckedSource::checked(&design, &src);
    assert_eq!(types.local(CheckedSource::binding(&design, "total")), Some(&Ty::UInt(32)), "§1.4.4: `total` is the `u32` its uses fix");
    // the literal is range-checked against the type the use fixes: 300 does not fit a `u8`
    let (src, d) = CheckedSource::body_diagnostics("", "let t = 300; let n: u8 = 1; let s = n + t;");
    assert_eq!(CheckedSource::codes(&d), ["E0909"], "{src}");
}

#[test]
fn a_read_that_requires_no_type_closes_the_literal_at_i32() {
    // §1.4.2: `i32` when the position requires none; a later `u32` use is then the mismatch it was.
    let (src, d) = CheckedSource::body_diagnostics("", "let t = 0; let a = t; let n: u32 = 4; t += n;");
    assert_eq!(CheckedSource::codes(&d), ["E0180"], "{src}");
}
