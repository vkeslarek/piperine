//! Scope: struct, enum, tuple, array and `Option` values lowered by `piperine_lang::elab` (feature
//! `composite-values`, CV-09 to CV-13, CV-20 to CV-22; docs/spec Part I §1.2 to §1.6, §2). Each test
//! elaborates a design of `Top` and reads the type arena and the expressions of the IR it gives.
//! Run-time values are proven from source in `piperine-parity`'s `source_aggregates`.

use crate::support;
use crate::elab_forms;

use piperine_ir::{Expr, TypeKind, ValueType};
use elab_forms::{ElabSource, IrReading};
use support::Elab;

#[test]
fn every_reached_struct_and_enum_is_one_arena_type() {
    // CV-20 AC 4, CV-22, §2 rule 5: each struct and enum the design reaches is one entry of the IR's
    // type arena, named as the source names it, with its fields or its variants, their
    // discriminants, the width of the tag and the traits it derives; a unit enum is an IR enum, not
    // its tag, so a value of it is a variant of its entry and a `match` over it tests the variant.
    let items = "#[derive(Repr, PartialEq)]\npub enum Mode { Off, SoftStart, Regulate, Hiccup }\n\
                 #[derive(PartialEq, PartialOrd)]\nstruct Pair { x: f64, n: u8 }\n\
                 #[derive(Repr)]\nenum Never { A, B }\n\
                 fn active() -> Mode { Mode::Regulate }\n\
                 fn pair_x(p: Pair) -> f64 { 1.0 }\n\
                 fn is_hiccup(m: Mode) -> bool { match m { Mode::Hiccup => true, _ => false } }";
    let ir = Elab::ir(&ElabSource::reaching(items, &["active()", "is_hiccup(Mode::Off)", "{ let f = pair_x; 0.0 }"]), "Top");

    // exactly the types the design reaches: `Never` is declared and never reached
    let names: Vec<&str> = ir.types().iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Mode") && names.contains(&"Pair") && !names.contains(&"Never"), "{names:?}");
    assert_eq!(names.iter().filter(|n| **n == "Mode").count(), 1, "one entry per declaration: {names:?}");

    let (mode, data) = IrReading::arena_entry(&ir, "Mode");
    let TypeKind::Enum { tag_levels, variants } = &data.kind else { panic!("Mode is an enum") };
    assert_eq!(*tag_levels, 2, "four variants: ceil(log2 4) levels (§2 rule 5)");
    assert_eq!(variants.iter().map(|v| (v.name.as_str(), v.discriminant)).collect::<Vec<_>>(), [("Off", 0), ("SoftStart", 1), ("Regulate", 2), ("Hiccup", 3)]);
    assert!(data.derives.repr && data.derives.partial_eq && data.derives.clone && data.derives.copy, "{:?}", data.derives);

    let (_, pair) = IrReading::arena_entry(&ir, "Pair");
    let TypeKind::Struct { fields, .. } = &pair.kind else { panic!("Pair is a struct") };
    assert_eq!(fields.clone(), [("x".to_string(), ValueType::F64), ("n".to_string(), ValueType::UInt(8))]);
    assert!(pair.derives.partial_eq && pair.derives.partial_ord && !pair.derives.repr, "{:?}", pair.derives);

    // a value of the enum is the variant of its entry (`Mode::Regulate` is variant 2), and the
    // function's result and parameter have the enum type
    let active = ir.functions().find(|f| f.name() == "active").expect("active");
    assert_eq!(*active.ret(), ValueType::Enum(piperine_ir::TypeId(mode as u32)));
    assert!(ir.expressions().any(|e| matches!(e, Expr::VariantOf { variant: 2, fields, .. } if fields.is_empty())), "Mode::Regulate is VariantOf");
    // a `match` arm over a known enum tests its variant
    assert!(ir.expressions().any(|e| matches!(e, Expr::IsVariant { variant: 3, .. })), "`Mode::Hiccup =>` is IsVariant of variant 3");
    // no unit enum is lowered as its integer tag
    let is_hiccup = ir.functions().find(|f| f.name() == "is_hiccup").expect("is_hiccup");
    assert_eq!(is_hiccup.params()[0].ty, ValueType::Enum(piperine_ir::TypeId(mode as u32)));
}

#[test]
fn struct_fields_are_evaluated_in_written_order() {
    // §1.3.11: the fields of a struct expression run in the order they are written. Written in
    // declaration order, or as plain reads, the parts stand where they are used; written out of order
    // with a part that is more than a read, each is bound to a local in written order first.
    let items = "struct P { x: f64, y: u8 }\nfn half(a: f64) -> f64 { a / 2.0 }\nfn one() -> u8 { 1 }\n\
                 fn ordered() -> P { P { x: half(1.0), y: one() } }\nfn shuffled() -> P { P { y: one(), x: half(1.0) } }";
    let ir = Elab::ir(&ElabSource::reaching(items, &["ordered()", "shuffled()"]), "Top");
    let text = ir.print_structural();
    let function = |name: &str| {
        let start = text.find(&format!("fn {name}(")).unwrap_or_else(|| panic!("no fn {name}"));
        let rest = &text[start..];
        rest[..rest[1..].find("\nfn ").map_or(rest.len(), |at| at + 1)].to_string()
    };
    assert!(!function("ordered").contains("field$"), "{}", function("ordered"));
    let shuffled = function("shuffled");
    let (first, second) = (shuffled.find("let field$").expect("a local per field"), shuffled.rfind("let field$").expect("a second"));
    assert!(first < second && shuffled[first..].find("one()").is_some_and(|at| at < shuffled[first..].find("half(").unwrap_or(usize::MAX)), "`y` runs before `x`: {shuffled}");
}

#[test]
fn aggregates_fold_in_const_contexts() {
    // CV-36 AC 1 (§1.2.4, §1.2.5, §1.3, H.5 to H.8): tuples, arrays, structs and `Option`s evaluate in a `const`, a
    // `const fn`, a param default, a `where` and a generation range with the value they have at run time, and
    // the stdlib's `poly_q`, `den_q` and `conjugate_closed` fold; a composite that is a param's default
    // becomes a `Constant::Aggregate` of the IR. Each expected value is the same expression evaluated by Rust.
    #[derive(Clone, Copy, PartialEq)]
    struct Pair {
        a: f64,
        n: u8,
    }
    let array = [1.0f64, 2.0, 3.0];
    let pair = Pair { a: 0.5, n: 4 };
    let tuple = (1.5f64, 2u8);
    let total = |xs: [f64; 3]| {
        let mut s = 0.0;
        for k in 0..3 {
            s += xs[k];
        }
        s
    };
    let doubled = |xs: [f64; 3]| {
        let mut r = xs;
        for k in 0..3 {
            r[k] = xs[k] * 2.0;
        }
        r
    };
    let items = "#[derive(PartialEq)]\nstruct Pair { a: f64, n: u8 }\n\
                 const A: [f64; 3] = [1.0, 2.0, 3.0];\n\
                 const P: Pair = Pair { a: 0.5, n: 4 };\n\
                 const T: (f64, u8) = (1.5, 2);\n\
                 const fn total(xs: [f64; 3]) -> f64 { let s = 0.0; for k in 0..3 { s += xs[k]; } s }\n\
                 const fn doubled(xs: [f64; 3]) -> [f64; 3] { let r = xs; for k in 0..3 { r[k] = xs[k] * 2.0; } r }\n\
                 const fn head(xs: [f64; 3]) -> Option<f64> { xs.first() }\n\
                 const fn pick(p: Pair) -> u8 { match p { Pair { n: 4, .. } => 40, Pair { n, .. } => n } }\n\
                 const fn swap(t: (f64, u8)) -> (u8, f64) { let (x, y) = t; (y, x) }";
    let params = "picked: f64 = A[1] + P.a + T.0; summed: f64 = total(A); both: f64 = doubled(A)[2]; counted: f64 = (P.n + T.1) as f64; \
                  swapped: f64 = swap(T).0 as f64; matched: f64 = pick(P) as f64; iterated: f64 = A.iter().map(|x| x * 2.0).fold(0.0, |acc, x| acc + x); \
                  first: f64 = head(A).unwrap_or(9.0); equal: bool = P == P && A != doubled(A); ranged: bool = (1..4).contains(3); \
                  sliced: f64 = A[1..3][0]; checked: f64 = 1.0 where A[0] < A[2] && P.n == 4;";
    let values = IrReading::param_values(&ElabSource::with_params(items, params));
    let real = |v: f64| format!("{:?}", v);
    assert_eq!(
        values,
        [
            real(array[1] + pair.a + tuple.0),
            real(total(array)),
            real(doubled(array)[2]),
            real(f64::from(pair.n + tuple.1)),
            real(f64::from(tuple.1)),
            real(40.0),
            real(array.iter().map(|x| x * 2.0).fold(0.0, |acc, x| acc + x)),
            real(*array.first().unwrap_or(&9.0)),
            (pair == pair && array != doubled(array)).to_string(),
            (1..4).contains(&3).to_string(),
            real(array[1..3][0]),
            "1.0".to_string(),
        ]
    );
}

#[test]
fn an_index_out_of_range_in_a_const_is_e0104() {
    // CV-13 AC 3, C58 (§1 rule 11, §1.5.25): an index past the end of an array in a const context is an
    // elaboration error, `E0104`, at the expression; so is a range index whose end is past the array's.
    for (items, at) in [
        ("const A: [f64; 3] = [1.0, 2.0, 3.0];\nconst BAD: f64 = A[5];", "A[5]"),
        ("const A: [f64; 3] = [1.0, 2.0, 3.0];\nconst fn element(i: usize) -> f64 { A[i] }\nconst BAD: f64 = element(3);", "A[i]"),
        ("const A: [f64; 3] = [1.0, 2.0, 3.0];\nconst BAD: f64 = A[1..4][0];", "A[1..4]"),
    ] {
        let src = ElabSource::with_params(items, "x: f64 = BAD;");
        let err = Elab::run(&src, "Top").expect_err("the index is out of range");
        assert_eq!(Elab::codes(&err), ["E0104"], "{items}");
        let span = err[0].diagnostic.span();
        assert!(src[span.start..span.end].contains(at), "the error names the expression {at}: {:?}", &src[span.start..span.end]);
    }
}

#[test]
fn the_stdlib_polynomial_helpers_fold() {
    // CV-36 AC 1, C96 (§22.2): `conjugate_closed`, `poly_q` and `den_q` of `std/analog.msl`, their bodies as
    // written there (tuples in arrays, `let (a, b) = ..`, `(1..=n + 1).rev()`, `iter().all(..)`, element
    // assignment) at a fixed length, fold in `const` items to the values Rust gives for the same algorithm.
    // The length stays fixed here because a function generic over `const N` is the `generics` feature's.
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/std/analog.msl")).expect("the stdlib's analog.msl");
    let function = |name: &str| -> String {
        let start = text.find(&format!("const fn {name}")).unwrap_or_else(|| panic!("no const fn {name} in analog.msl"));
        let end = start + text[start..].find("\n}\n").expect("the end of the function") + 3;
        [("<const N: usize>", ""), ("<const D: usize>", ""), ("N + 1", "3"), ("0..N", "0..2"), ("0..=N", "0..=2"), ("0..D", "0..2"), ("; N]", "; 2]"), ("; D]", "; 2]")].iter().fold(text[start..end].to_string(), |body, (from, to)| body.replace(from, to))
    };
    let items = format!(
        "{}\n{}\n{}\n\
         const ROOTS: [(f64, f64); 2] = [(0.5, 0.0), (0.25, 0.5)];\n\
         const C: [f64; 3] = poly_q(ROOTS);\n\
         const D: [f64; 2] = den_q(ROOTS);\n\
         const CLOSED: bool = conjugate_closed([(1.0, 1.0), (1.0, -1.0)]);\n\
         const OPEN: bool = conjugate_closed([(1.0, 1.0), (2.0, -1.0)]);",
        function("conjugate_closed"),
        function("poly_q"),
        function("den_q")
    );
    let values = IrReading::param_values(&ElabSource::with_params(&items, "c1: f64 = C[1]; c2: f64 = C[2]; d0: f64 = D[0]; d1: f64 = D[1]; ok: bool = CLOSED; no: bool = OPEN;"));
    // the same product of `1 - rho * q` over the roots, in Rust
    let roots = [(0.5f64, 0.0f64), (0.25, 0.5)];
    let mut c = [(0.0f64, 0.0f64); 3];
    c[0] = (1.0, 0.0);
    for n in 0..2 {
        let (a, b) = roots[n];
        for k in (1..=n + 1).rev() {
            let (pr, pi) = c[k - 1];
            c[k] = (c[k].0 - (a * pr - b * pi), c[k].1 - (a * pi + b * pr));
        }
    }
    let real = |v: f64| format!("{v:?}");
    assert_eq!(values, [real(c[1].0), real(c[2].0), real(c[1].0), real(c[2].0), "true".to_string(), "false".to_string()]);
}

#[test]
fn a_folded_composite_becomes_an_aggregate_constant() {
    // CV-36 (§1.5.25): a composite that is a param's default is a `Constant::Aggregate` of the IR's arena: its
    // type, and one `Constant` per part, nested through further aggregates.
    use piperine_ir::{Constant, Real};
    let items = "struct Pair { a: f64, n: u8 }\nconst A: [f64; 3] = [1.0, 2.0, 3.0];\nconst P: Pair = Pair { a: 0.5, n: 4 };\n\
                 const fn doubled(xs: [f64; 3]) -> [f64; 3] { let r = xs; for k in 0..3 { r[k] = xs[k] * 2.0; } r }";
    let ir = Elab::ir(&ElabSource::with_params(items, "table: [f64; 3] = doubled(A); p: Pair = P; t: (f64, bool) = (1.5, true); n: ((f64, f64), [bool; 2]) = ((0.5, 1.5), [true, false]);"), "Top");
    let params: Vec<Constant> = ir.instance_at("Top").expect("the top instance").params().iter().copied().collect();
    let parts = |value: &Constant| match value {
        Constant::Aggregate(id) => ir.aggregate(*id).parts.clone(),
        other => panic!("a composite default is an aggregate, found {other:?}"),
    };
    let real = |v: f64| Constant::Real(Real::new(v));
    assert_eq!(parts(&params[0]), [real(2.0), real(4.0), real(6.0)]);
    assert_eq!(parts(&params[1]), [real(0.5), Constant::Int(4)]);
    assert_eq!(parts(&params[2]), [real(1.5), Constant::Bool(true)]);
    let nested = parts(&params[3]);
    assert_eq!(parts(&nested[0]), [real(0.5), real(1.5)]);
    assert_eq!(parts(&nested[1]), [Constant::Bool(true), Constant::Bool(false)]);
}

#[test]
fn a_composite_const_read_in_value_code_is_written_out() {
    // CV-36 (§1.5.25): a `const` of composite type read in a function or an `analog` block is the expression that
    // builds its value, so `A[2]` indexes an array of the constants and `P.a` reads a field of a struct of them.
    let items = "struct Pair { a: f64, n: u8 }\nconst A: [f64; 3] = [1.0, 2.0, 3.0];\nconst P: Pair = Pair { a: 0.5, n: 4 };\nfn pick(i: usize) -> f64 { A[i] + P.a }";
    let ir = Elab::ir(&ElabSource::reaching(items, &["pick(1)", "A[2]"]), "Top");
    assert!(ir.expressions().any(|e| matches!(e, Expr::Array(parts) if parts.len() == 3)), "the array constant is written out");
    assert!(ir.expressions().any(|e| matches!(e, Expr::StructOf { fields, .. } if fields.len() == 2)), "the struct constant is written out");
}

#[test]
fn a_function_over_an_array_of_options_reads_each_element_as_a_pair() {
    // resolved-nets T28 (Appendix F.4.4: a `resolve` over `[Option<u8>; N]`): an array whose elements are `Option`s is an IR
    // array of packed options, and `d[k]` is the pair of its two halves, each unpacked from the element read
    let items = "fn first_or(d: [Option<u8>; 3], k: usize) -> u8 { match d[k] { Some(b) => b, None => 0 } }";
    let ir = Elab::ir(&ElabSource::reaching(items, &["{ let f = first_or; 0.0 }"]), "Top");
    assert!(ir.expressions().any(|e| matches!(e, Expr::OptionPart { .. })), "the element is unpacked into its halves");
    assert!(ir.expressions().any(|e| matches!(e, Expr::Index { .. })), "the element is read from the array");
}
