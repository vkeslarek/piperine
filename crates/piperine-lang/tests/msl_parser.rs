//! Scope: the MSL parser (`piperine_lang::msl::parser`) building spanned AST
//! nodes (requirement MSLP-02, MSLP-06). Types, paths, generics and patterns
//! (T5), extended with expressions/statements (T6) and items (T7).
//! Fixtures are minimal sources; assertions name the AST node and, where the
//! spec says a node's span covers its text, the exact byte range.

use piperine_lang::msl::Span;
use piperine_lang::msl::ast::*;
use piperine_lang::msl::parser::{parse_generics, parse_pat, parse_type};

fn ty(src: &str) -> Type {
    let (t, diags) = parse_type(src);
    assert!(diags.is_empty(), "{src:?}: {diags:?}");
    t.unwrap_or_else(|| panic!("{src:?} did not parse"))
}

fn pat(src: &str) -> Pat {
    let (p, diags) = parse_pat(src);
    assert!(diags.is_empty(), "{src:?}: {diags:?}");
    p.unwrap_or_else(|| panic!("{src:?} did not parse"))
}

fn path_names(p: &Path) -> Vec<&str> {
    p.segments.iter().map(|s| s.ident.name.as_str()).collect()
}

/// The type is a path whose segment names are `names`.
fn assert_path_type(t: &Type, names: &[&str]) {
    match &t.kind {
        TypeKind::Path(p) => assert_eq!(path_names(p), names),
        other => panic!("expected a path type, got {other:?}"),
    }
}

fn angle_args(t: &Type) -> &[GenericArg] {
    let TypeKind::Path(p) = &t.kind else { panic!("not a path type: {t:?}") };
    match p.segments.last().and_then(|s| s.args.as_ref()) {
        Some(GenericArgs::Angle { args, .. }) => args,
        other => panic!("expected angle args, got {other:?}"),
    }
}

#[test]
fn inherited_rust_forms_parse() {
    // ---- types ----
    // Paths and generic arguments (types, literals, braced const blocks).
    assert_path_type(&ty("f64"), &["f64"]);
    assert_path_type(&ty("Self::WIDTH"), &["Self", "WIDTH"]);
    assert_path_type(&ty("crate::a::B"), &["crate", "a", "B"]);
    let bit = ty("Bit<u8>");
    assert_path_type(&bit, &["Bit"]);
    assert!(matches!(&angle_args(&bit)[0], GenericArg::Type(t) if matches!(&t.kind, TypeKind::Path(_))));
    let uint = ty("UInt<{N}>");
    assert!(matches!(&angle_args(&uint)[0], GenericArg::Const(Expr { kind: ExprKind::Block(_), .. })));
    let lit_arg = ty("Foo<3, true>");
    assert!(matches!(&angle_args(&lit_arg)[0], GenericArg::Const(Expr { kind: ExprKind::Lit(_), .. })));
    assert_eq!(angle_args(&lit_arg).len(), 2);
    // Nested generics close with two separate `>` tokens.
    let nested = ty("Vec<Vec<T>>");
    let GenericArg::Type(inner) = &angle_args(&nested)[0] else { panic!() };
    assert_path_type(inner, &["Vec"]);
    // Trailing comma in generic arguments.
    assert_eq!(angle_args(&ty("Map<K, V,>")).len(), 2);

    // Arrays, slices, tuples, unit, parenthesized, references, `_`.
    assert!(matches!(ty("[Bit; 8]").kind, TypeKind::Array { .. }));
    assert!(matches!(ty("[(f64, f64); N]").kind, TypeKind::Array { .. }));
    assert!(matches!(ty("[bool; Self::WIDTH]").kind, TypeKind::Array { .. }));
    assert!(matches!(ty("[T]").kind, TypeKind::Slice(_)));
    assert!(matches!(&ty("(f64, f64)").kind, TypeKind::Tuple(v) if v.len() == 2));
    assert!(matches!(&ty("(T,)").kind, TypeKind::Tuple(v) if v.len() == 1));
    assert!(matches!(&ty("()").kind, TypeKind::Tuple(v) if v.is_empty()));
    assert_path_type(&ty("(T)"), &["T"]);
    assert!(matches!(ty("_").kind, TypeKind::Infer));
    assert!(matches!(&ty("&'static str").kind, TypeKind::Ref { static_lifetime: true, .. }));
    assert!(matches!(&ty("&[Option<T>]").kind, TypeKind::Ref { static_lifetime: false, inner } if matches!(inner.kind, TypeKind::Slice(_))));

    // `flip` reverses a bundle type; it nests inside arrays (`sigtype`).
    assert!(matches!(&ty("flip Stream<T>").kind, TypeKind::Flip(t) if matches!(t.kind, TypeKind::Path(_))));
    let flipped_array = ty("[flip Stream<T>; N]");
    let TypeKind::Array { elem, .. } = &flipped_array.kind else { panic!() };
    assert!(matches!(elem.kind, TypeKind::Flip(_)));

    // Spans cover the text exactly.
    assert_eq!(ty("Bit<u8>").span.0, Span::new(0, 7));
    assert_eq!(ty("[Bit; 8]").span.0, Span::new(0, 8));
    assert_eq!(ty("flip Stream<T>").span.0, Span::new(0, 14));

    // ---- generics and bounds ----
    let (g, diags) = parse_generics("<T: Repr = bool, const N: u32, C: Fn(Cell), U: A + B>");
    assert!(diags.is_empty(), "{diags:?}");
    let g = g.unwrap();
    assert_eq!(g.params.len(), 4);
    assert!(matches!(&g.params[0].kind, GenericParamKind::Type { bounds, default: Some(_) } if bounds.len() == 1));
    assert!(matches!(&g.params[1].kind, GenericParamKind::Const { default: None, .. }));
    let GenericParamKind::Type { bounds, .. } = &g.params[2].kind else { panic!() };
    assert!(matches!(
        bounds[0].segments[0].args,
        Some(GenericArgs::Paren { ref inputs, output: None, .. }) if inputs.len() == 1
    ));
    assert!(matches!(&g.params[3].kind, GenericParamKind::Type { bounds, default: None } if bounds.len() == 2));
    assert_eq!(g.span.0, Span::new(0, 53));

    // ---- patterns ----
    assert!(matches!(pat("_").kind, PatKind::Wild));
    assert!(matches!(&pat("x").kind, PatKind::Ident { by_ref: false, mutable: false, sub: None, .. }));
    assert!(matches!(&pat("mut x").kind, PatKind::Ident { mutable: true, .. }));
    assert!(matches!(&pat("ref x").kind, PatKind::Ident { by_ref: true, .. }));
    assert!(matches!(&pat("x @ 1..=5").kind, PatKind::Ident { sub: Some(_), .. }));
    assert!(matches!(&pat("Phase::Liquid").kind, PatKind::Path(p) if path_names(p) == ["Phase", "Liquid"]));
    assert!(matches!(&pat("Some(x)").kind, PatKind::TupleStruct { pats, .. } if pats.len() == 1));
    assert!(matches!(&pat("Data { n }").kind, PatKind::Struct { fields, rest: false, .. } if fields.len() == 1 && fields[0].pat.is_none()));
    assert!(matches!(&pat("P { x: 0, .. }").kind, PatKind::Struct { fields, rest: true, .. } if fields.len() == 1 && fields[0].pat.is_some()));
    assert!(matches!(&pat("(a, b)").kind, PatKind::Tuple(v) if v.len() == 2));
    assert!(matches!(&pat("()").kind, PatKind::Tuple(v) if v.is_empty()));
    assert!(matches!(pat("(a)").kind, PatKind::Ident { .. }));
    assert!(matches!(&pat("[a, b, ..]").kind, PatKind::Slice(v) if v.len() == 3 && matches!(v[2].kind, PatKind::Rest)));
    assert!(matches!(&pat("&x").kind, PatKind::Ref(_)));
    // Alternatives, including nested in a tuple (spec: `(None, y) | (y, None)`).
    assert!(matches!(&pat("(None, y) | (y, None)").kind, PatKind::Or(v) if v.len() == 2));
    // Literals: integers, negative, strings, bools, `Unknown`.
    assert!(matches!(&pat("0").kind, PatKind::Lit(_)));
    assert!(matches!(&pat("-1").kind, PatKind::Lit(e) if matches!(e.kind, ExprKind::Unary { op: UnOp::Neg, .. })));
    assert!(matches!(&pat("\"s\"").kind, PatKind::Lit(_)));
    assert!(matches!(&pat("true").kind, PatKind::Lit(e) if matches!(&e.kind, ExprKind::Lit(Lit { kind: LitKind::Bool(true), .. }))));
    assert!(matches!(&pat("Unknown").kind, PatKind::Lit(e) if matches!(&e.kind, ExprKind::Lit(Lit { kind: LitKind::Unknown, .. }))));
    // Ranges.
    assert!(matches!(&pat("0..=9").kind, PatKind::Range { inclusive: true, lo: Some(_), hi: Some(_) }));
    assert!(matches!(&pat("0b00..=0b11").kind, PatKind::Range { inclusive: true, .. }));
    assert!(matches!(&pat("..=9").kind, PatKind::Range { lo: None, hi: Some(_), inclusive: true }));
    assert!(matches!(&pat("0..").kind, PatKind::Range { lo: Some(_), hi: None, inclusive: false }));
    assert!(matches!(&pat("A..=B").kind, PatKind::Range { .. }));
    // Spans.
    assert_eq!(pat("Some(x)").span.0, Span::new(0, 7));
    assert_eq!(pat("(None, y) | (y, None)").span.0, Span::new(0, 21));
}
