//! Scope: the MSL parser (`piperine_lang::msl::parser`) building spanned AST
//! nodes (requirement MSLP-02, MSLP-06). Types, paths, generics and patterns
//! (T5), extended with expressions/statements (T6) and items (T7).
//! Fixtures are minimal sources; assertions name the AST node and, where the
//! spec says a node's span covers its text, the exact byte range.

use piperine_lang::msl::Span;
use piperine_lang::msl::ast::*;
use piperine_lang::msl::parser::{parse_block, parse_expr, parse_generics, parse_pat, parse_type};

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

fn expr(src: &str) -> Expr {
    let (e, diags) = parse_expr(src);
    assert!(diags.is_empty(), "{src:?}: {diags:?}");
    e.unwrap_or_else(|| panic!("{src:?} did not parse"))
}

fn block(src: &str) -> Block {
    let (b, diags) = parse_block(src);
    assert!(diags.is_empty(), "{src:?}: {diags:?}");
    b.unwrap_or_else(|| panic!("{src:?} did not parse"))
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

/// The expression is `lhs op rhs`; returns both sides.
fn binary(e: &Expr, op: BinOp) -> (&Expr, &Expr) {
    match &e.kind {
        ExprKind::Binary { op: o, lhs, rhs } if *o == op => (lhs, rhs),
        other => panic!("expected {op:?}, got {other:?}"),
    }
}

fn is_path(e: &Expr, name: &str) -> bool {
    matches!(&e.kind, ExprKind::Path(p) if path_names(p) == [name])
}

fn is_int(e: &Expr, text: &str) -> bool {
    matches!(&e.kind, ExprKind::Lit(Lit { kind: LitKind::Int(t), .. }) if t == text)
}

#[test]
fn inherited_rust_forms_parse_expressions() {
    // ---- precedence, Rust's ----
    let e = expr("1 + 2 * 3");
    let (l, r) = binary(&e, BinOp::Add);
    assert!(is_int(l, "1"));
    binary(r, BinOp::Mul);
    assert_eq!(e.span.0, Span::new(0, 9));
    // `&` binds tighter than `==`; `as` tighter than `*`; `||` loosest.
    let e = expr("(byte >> 7) & 1 == 1");
    let (l, _) = binary(&e, BinOp::Eq);
    binary(l, BinOp::BitAnd);
    let e = expr("k as f64 * half");
    let (l, _) = binary(&e, BinOp::Mul);
    assert!(matches!(l.kind, ExprKind::Cast { .. }));
    let e = expr("a || b && c == d");
    let (_, r) = binary(&e, BinOp::Or);
    let (_, r) = binary(r, BinOp::And);
    binary(r, BinOp::Eq);
    // Unary binds tighter than a cast: `-x as f64` is `(-x) as f64`.
    let e = expr("-x as f64");
    let ExprKind::Cast { expr: inner, .. } = &e.kind else { panic!("{e:?}") };
    assert!(matches!(inner.kind, ExprKind::Unary { op: UnOp::Neg, .. }));
    assert!(matches!(expr("!clk").kind, ExprKind::Unary { op: UnOp::Not, .. }));
    assert!(matches!(expr("&IV").kind, ExprKind::Unary { op: UnOp::Ref, .. }));
    assert!(matches!(expr("*x").kind, ExprKind::Unary { op: UnOp::Deref, .. }));
    // The `>` family: `>>`, `>=`, `>`, `>>=` from touching `>` tokens.
    binary(&expr("a >> b"), BinOp::Shr);
    binary(&expr("a >= b"), BinOp::Ge);
    binary(&expr("a > b"), BinOp::Gt);
    binary(&expr("a <= b"), BinOp::Le);
    binary(&expr("a << b"), BinOp::Shl);
    assert!(matches!(expr("a >>= 1").kind, ExprKind::Assign { op: Some(BinOp::Shr), .. }));
    // Assignment, compound assignment, right associative.
    assert!(matches!(expr("a = b").kind, ExprKind::Assign { op: None, .. }));
    assert!(matches!(expr("s += 1").kind, ExprKind::Assign { op: Some(BinOp::Add), .. }));
    let ExprKind::Assign { rhs, .. } = &expr("a = b = c").kind else { panic!() };
    assert!(matches!(rhs.kind, ExprKind::Assign { .. }));
    // Ranges.
    assert!(matches!(expr("0..7").kind, ExprKind::Range { lo: Some(_), hi: Some(_), inclusive: false }));
    assert!(matches!(expr("0..=ROWS").kind, ExprKind::Range { inclusive: true, .. }));
    assert!(matches!(expr("..4").kind, ExprKind::Range { lo: None, hi: Some(_), .. }));

    // ---- literals, paths, tuples, arrays ----
    assert!(matches!(expr("Unknown").kind, ExprKind::Lit(Lit { kind: LitKind::Unknown, .. })));
    assert!(matches!(expr("true").kind, ExprKind::Lit(Lit { kind: LitKind::Bool(true), .. })));
    assert!(matches!(expr("2.2u").kind, ExprKind::Lit(Lit { kind: LitKind::Float { raw, .. }, .. }) if raw == "2.2u"));
    assert!(matches!(expr("Phase::Liquid").kind, ExprKind::Path(_)));
    assert!(matches!(expr("(a)").kind, ExprKind::Paren(_)));
    assert!(matches!(&expr("(a, b)").kind, ExprKind::Tuple(v) if v.len() == 2));
    assert!(matches!(&expr("()").kind, ExprKind::Tuple(v) if v.is_empty()));
    assert!(matches!(&expr("[a, b]").kind, ExprKind::Array(v) if v.len() == 2));
    assert!(matches!(&expr("[]").kind, ExprKind::Array(v) if v.is_empty()));
    assert!(matches!(expr("[0; 4]").kind, ExprKind::ArrayRepeat { .. }));

    // ---- calls: positional, named, turbofish, instances, templates ----
    let ExprKind::Call { args, params: None, .. } = &expr("idt(x, 1.0, reset: rst)").kind else { panic!() };
    assert_eq!(args.len(), 3);
    assert!(args[0].name.is_none() && args[1].name.is_none());
    assert_eq!(args[2].name.as_ref().map(|n| n.name.as_str()), Some("reset"));
    let ExprKind::Call { callee, .. } = &expr("idt::<Heat>(pwr(t), 0.0, reset: ph != Phase::Boiling)").kind else { panic!() };
    let ExprKind::Path(p) = &callee.kind else { panic!() };
    assert!(matches!(&p.segments[0].args, Some(GenericArgs::Angle { turbofish: true, .. })));
    // An instance: a call followed by a param block; the callee may be a call.
    let ExprKind::Call { params: Some(params), .. } =
        &expr("MemoryBank::<_, 64, 32>(wl, bl) { cell: Dram1T1C { cs: 30f } }").kind
    else {
        panic!()
    };
    assert_eq!(params.len(), 1);
    assert!(matches!(params[0].value.kind, ExprKind::Struct { .. }));
    let ExprKind::Call { callee, .. } = &expr("nch(0.13u, 2u)(d, g, s, b)").kind else { panic!() };
    assert!(matches!(callee.kind, ExprKind::Call { .. }));
    // A struct expression or template, with and without fields.
    assert!(matches!(&expr("Cell { wl: wl[r], bl: bl[c] }").kind, ExprKind::Struct { fields, .. } if fields.len() == 2));
    assert!(matches!(&expr("Dram1T1C { cs: 30f }").kind, ExprKind::Struct { .. }));
    // Method calls, fields, tuple indexes, indexing, `?`.
    assert!(matches!(&expr("x.known().unwrap_or(d)").kind, ExprKind::MethodCall { method, .. } if method.name == "unwrap_or"));
    assert!(matches!(&expr("self.p").kind, ExprKind::Field { name, .. } if name.name == "p"));
    assert!(matches!(&expr("t.0").kind, ExprKind::Field { name, .. } if name.name == "0"));
    assert!(matches!(expr("sh[k + 1]").kind, ExprKind::Index { .. }));
    assert!(matches!(expr("self.level(k)?").kind, ExprKind::Try(_)));
    let ExprKind::MethodCall { generics: Some(_), .. } = &expr("it.collect::<Vec<_>>()").kind else { panic!() };

    // ---- closures ----
    let ExprKind::Closure { params, .. } = &expr("|new, old| deltalim(new, old, 1.0)").kind else { panic!() };
    assert_eq!(params.len(), 2);
    assert!(matches!(&expr("|x: f64| -> f64 { x }").kind, ExprKind::Closure { ret: Some(_), .. }));
    assert!(matches!(&expr("|| 0").kind, ExprKind::Closure { params, .. } if params.is_empty()));
    assert!(matches!(expr("move |x| x").kind, ExprKind::Closure { is_move: true, .. }));
    // A closure passed to a method, holding a `match` with or-patterns.
    expr("d.iter().fold(None, |a, x| match (a, *x) { (None, y) | (y, None) => y, _ => Some(Unknown) })");

    // ---- control flow ----
    let e = expr("if l < 0.18u { 0.42 } else if l < 1u { 0.45 } else { 0.47 }");
    let ExprKind::If { else_: Some(e1), .. } = &e.kind else { panic!() };
    let ExprKind::If { else_: Some(e2), .. } = &e1.kind else { panic!() };
    assert!(matches!(e2.kind, ExprKind::Block(_)));
    assert!(matches!(expr("if let Some(x) = y { x } else { 0 }").kind, ExprKind::If { .. }));
    let e = expr("match k { 0 => { a } 16 => b, _ if k % 2 == 1 => c, _ => d }");
    let ExprKind::Match { arms, .. } = &e.kind else { panic!() };
    assert_eq!(arms.len(), 4);
    assert!(arms[2].guard.is_some() && arms[0].guard.is_none());
    assert!(matches!(expr("while i < 3 { i += 1; }").kind, ExprKind::While { .. }));
    assert!(matches!(expr("loop { break; }").kind, ExprKind::Loop { .. }));
    assert!(matches!(expr("for r in 0..ROWS { f(r); }").kind, ExprKind::For { .. }));
    assert!(matches!(expr("{ let x = 1; x + 1 }").kind, ExprKind::Block(_)));
    assert!(matches!(expr("return x").kind, ExprKind::Return(Some(_))));
    // A condition does not take a struct literal: `x { .. }` is the block.
    let ExprKind::If { cond, then, .. } = &expr("if flag { 1 } else { 2 }").kind else { panic!() };
    assert!(is_path(cond, "flag"));
    assert_eq!(then.stmts.len(), 1);

    // ---- built-in `!` forms ----
    let ExprKind::Emit { stream, fields } = &expr("emit!(\"sim\", finish = true, msg = msg)").kind else { panic!() };
    assert!(matches!(&stream.kind, LitKind::Str(s) if s == "sim"));
    assert_eq!(fields.len(), 2);
    assert!(matches!(&expr("include_str!(\"diode_iv.tbl\")").kind, ExprKind::IncludeStr(_)));

    // ---- statements ----
    let b = block("{ let (a, b) = (1, 2); let mut s: i2 = state; s += 1; bound_step(t / 50.0); s }");
    assert_eq!(b.stmts.len(), 5);
    assert!(matches!(&b.stmts[0].kind, StmtKind::Let { init: Some(_), ty: None, .. }));
    assert!(matches!(&b.stmts[1].kind, StmtKind::Let { ty: Some(_), .. }));
    assert!(matches!(&b.stmts[2].kind, StmtKind::Expr { semi: true, .. }));
    assert!(matches!(&b.stmts[4].kind, StmtKind::Expr { semi: false, .. }));
    // A block-like statement needs no `;` and does not swallow what follows.
    let b = block("{ if a { f(); } g(); }");
    assert_eq!(b.stmts.len(), 2);
    assert!(matches!(&b.stmts[0].kind, StmtKind::Expr { semi: false, .. }));
    assert_eq!(block("{ ; }").stmts.len(), 1);
    assert_eq!(block("{}").span.0, Span::new(0, 2));
    // Spans: a statement covers its text including the `;`.
    let b = block("{ let x = 1; }");
    assert_eq!(b.stmts[0].span.0, Span::new(2, 12));
    assert_eq!(b.span.0, Span::new(0, 14));

    // ---- types holding expressions ----
    let t = ty("UInt<{N + 1}>");
    assert!(matches!(&angle_args(&t)[0], GenericArg::Const(Expr { kind: ExprKind::Block(_), .. })));
    assert!(matches!(ty("[Bit; N + 1]").kind, TypeKind::Array { .. }));
}

/// Counts of (contributions, commits) in a block, at any nesting.
fn count_stmts(b: &Block) -> (usize, usize) {
    fn expr(e: &Expr, acc: &mut (usize, usize)) {
        match &e.kind {
            ExprKind::Block(b) => blk(b, acc),
            ExprKind::If { then, else_, .. } => {
                blk(then, acc);
                if let Some(e) = else_ {
                    expr(e, acc);
                }
            }
            ExprKind::Match { arms, .. } => arms.iter().for_each(|a| expr(&a.body, acc)),
            ExprKind::For { body, .. } | ExprKind::While { body, .. } | ExprKind::Loop { body } => blk(body, acc),
            _ => {}
        }
    }
    fn blk(b: &Block, acc: &mut (usize, usize)) {
        for s in &b.stmts {
            match &s.kind {
                StmtKind::Contribution { .. } => acc.0 += 1,
                StmtKind::Commit { .. } => acc.1 += 1,
                StmtKind::Expr { expr: e, .. } => expr(e, acc),
                _ => {}
            }
        }
    }
    let mut acc = (0, 0);
    blk(b, &mut acc);
    acc
}

#[test]
fn contribution_commit_at_any_depth() {
    // Top level.
    let b = block("{ i(p, n) +> v(p, n) / r; }");
    assert!(matches!(&b.stmts[0].kind, StmtKind::Contribution { .. }));
    assert_eq!(b.stmts[0].span.0, Span::new(2, 25));
    let b = block("{ q <- d; }");
    assert!(matches!(&b.stmts[0].kind, StmtKind::Commit { after: None, .. }));
    // `after` belongs to the commit; the expression stops in front of it.
    let b = block("{ clk <- !clk after period / 2.0 + j; }");
    let StmtKind::Commit { value, after: Some(after), .. } = &b.stmts[0].kind else { panic!() };
    assert!(matches!(value.kind, ExprKind::Unary { op: UnOp::Not, .. }));
    binary(after, BinOp::Add);
    // Places: a carrier, an element, a field.
    let b = block("{ st.n <- 1; sh[3] <- x; sh[7] <- rx; }");
    assert_eq!(count_stmts(&b), (0, 3));
    let StmtKind::Commit { place, .. } = &b.stmts[0].kind else { panic!() };
    assert!(matches!(place.kind, ExprKind::Field { .. }));
    let StmtKind::Commit { place, .. } = &b.stmts[1].kind else { panic!() };
    assert!(matches!(place.kind, ExprKind::Index { .. }));
    // Under `if` / `else`.
    let b = block("{ if ctl.high() { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; } }");
    assert_eq!(count_stmts(&b), (2, 0));
    // Under `match` arms with a block body.
    let b = block(
        "{ match ph { Phase::Liquid => { pwr(t) +> ddt(m * cl * temp(t)); } \
           Phase::Boiling => { temp(t) +> tb; } \
           Phase::Vapor => { pwr(t) +> ddt(m * cv * temp(t)); } } }",
    );
    assert_eq!(count_stmts(&b), (3, 0));
    // Under `for`, and nested several levels deep, commits as well.
    let b = block("{ for k in 0..7 { sh[k] <- sh[k + 1]; } }");
    assert_eq!(count_stmts(&b), (0, 1));
    let b = block("{ if a { for k in 0..3 { match k { 0 => { x +> 1.0; } _ => { y <- 2; } } } } }");
    assert_eq!(count_stmts(&b), (1, 1));
    // `while`, `loop` and a plain nested block.
    let b = block("{ while c { x +> 1.0; } loop { y <- 1; } { z +> 2.0; } }");
    assert_eq!(count_stmts(&b), (2, 1));
}

#[test]
fn unknown_as_value_ok_as_name_e0908() {
    // The value half: `Unknown` parses wherever a value is expected.
    assert!(matches!(expr("Unknown").kind, ExprKind::Lit(Lit { kind: LitKind::Unknown, .. })));
    let b = block("{ let x = Unknown; f(Unknown, 1); y <- Unknown; }");
    assert!(matches!(&b.stmts[0].kind, StmtKind::Let { init: Some(Expr { kind: ExprKind::Lit(Lit { kind: LitKind::Unknown, .. }), .. }), .. }));
    let ExprKind::Call { args, .. } = &expr("f(Unknown, 1)").kind else { panic!() };
    assert!(matches!(args[0].value.kind, ExprKind::Lit(Lit { kind: LitKind::Unknown, .. })));
    assert!(matches!(&expr("Some(Unknown)").kind, ExprKind::Call { .. }));
}
