//! Scope: body lowering (requirement POM-06): every expression, statement,
//! pattern and type form of the language becomes one node of the matching
//! kind, reachable from the declaration that owns it.

#[path = "support/pom.rs"]
mod support;
#[path = "support/lowering.rs"]
mod lowering;

use std::collections::BTreeSet;

use piperine_lang::pom::{
    BindingOrigin, DeclId, DeclKind, Design, ExprId, ExprKind, FnParamData, PatId, PatKind, PathId, Reflect, StmtId,
    StmtKind, TypeId, TypeKind,
};
use lowering::lowering_codes;
use support::{build, codes};

fn expr_kind_name(k: &ExprKind) -> &'static str {
    match k {
        ExprKind::Lit(_) => "Lit",
        ExprKind::Path(_) => "Path",
        ExprKind::Unary { .. } => "Unary",
        ExprKind::Binary { .. } => "Binary",
        ExprKind::Assign { .. } => "Assign",
        ExprKind::Cast { .. } => "Cast",
        ExprKind::Call { .. } => "Call",
        ExprKind::MethodCall { .. } => "MethodCall",
        ExprKind::Field { .. } => "Field",
        ExprKind::Index { .. } => "Index",
        ExprKind::Range { .. } => "Range",
        ExprKind::Paren(_) => "Paren",
        ExprKind::Tuple(_) => "Tuple",
        ExprKind::Array(_) => "Array",
        ExprKind::ArrayRepeat { .. } => "ArrayRepeat",
        ExprKind::Struct { .. } => "Struct",
        ExprKind::Block(_) => "Block",
        ExprKind::If { .. } => "If",
        ExprKind::Let { .. } => "Let",
        ExprKind::Match { .. } => "Match",
        ExprKind::While { .. } => "While",
        ExprKind::Loop { .. } => "Loop",
        ExprKind::For { .. } => "For",
        ExprKind::Break(_) => "Break",
        ExprKind::Continue => "Continue",
        ExprKind::Return(_) => "Return",
        ExprKind::Closure { .. } => "Closure",
        ExprKind::Try(_) => "Try",
        ExprKind::Emit { .. } => "Emit",
        ExprKind::IncludeStr(_) => "IncludeStr",
        ExprKind::Error => "Error",
    }
}

fn pat_kind_name(k: &PatKind) -> &'static str {
    match k {
        PatKind::Wild => "Wild",
        PatKind::Rest => "Rest",
        PatKind::Ident { .. } => "Ident",
        PatKind::Lit(_) => "Lit",
        PatKind::Path(_) => "Path",
        PatKind::Tuple(_) => "Tuple",
        PatKind::TupleStruct { .. } => "TupleStruct",
        PatKind::Struct { .. } => "Struct",
        PatKind::Or(_) => "Or",
        PatKind::Range { .. } => "Range",
        PatKind::Array(_) => "Array",
    }
}

fn type_kind_name(k: &TypeKind) -> &'static str {
    match k {
        TypeKind::Path(_) => "Path",
        TypeKind::Array { .. } => "Array",
        TypeKind::Tuple(_) => "Tuple",
        TypeKind::Flip(_) => "Flip",
        TypeKind::FnPtr { .. } => "FnPtr",
        TypeKind::Infer => "Infer",
    }
}

fn stmt_kind_name(k: &StmtKind) -> &'static str {
    match k {
        StmtKind::Let { .. } => "Let",
        StmtKind::Expr { .. } => "Expr",
        StmtKind::Item(_) => "Item",
        StmtKind::Contribution { .. } => "Contribution",
        StmtKind::Commit { .. } => "Commit",
        StmtKind::Empty => "Empty",
        StmtKind::Signal(_) => "Signal",
        StmtKind::Reaction(_) => "Reaction",
    }
}

fn expr_kinds(d: &Design) -> BTreeSet<&'static str> {
    (0..d.expr_count()).map(|i| expr_kind_name(&d.expr_node(ExprId(i as u32)).kind)).collect()
}

#[test]
fn every_expression_form_has_its_kind() {
    let forms: [(&str, &str); 30] = [
        ("Lit", "1"),
        ("Path", "a"),
        ("Unary", "-a"),
        ("Binary", "a + b"),
        ("Assign", "a = b"),
        ("Cast", "a as f64"),
        ("Call", "f(1)"),
        ("MethodCall", "a.m(1)"),
        ("Field", "a.b"),
        ("Index", "a[0]"),
        ("Range", "0..4"),
        ("Paren", "(a)"),
        ("Tuple", "(a, b)"),
        ("Array", "[1, 2]"),
        ("ArrayRepeat", "[0; 4]"),
        ("Struct", "S { x: 1 }"),
        ("Block", "{ 1 }"),
        ("If", "if a { 1 } else { 2 }"),
        ("Let", "if let Some(x) = y { 1 }"),
        ("Match", "match a { _ => 1 }"),
        ("While", "while a { }"),
        ("Loop", "loop { }"),
        ("For", "for i in 0..4 { }"),
        ("Break", "loop { break; }"),
        ("Continue", "loop { continue; }"),
        ("Return", "return 1"),
        ("Closure", "|x| x + 1"),
        ("Try", "a?"),
        ("Emit", "emit!(\"s\", a = 1)"),
        ("IncludeStr", "include_str!(\"f.txt\")"),
    ];
    let mut seen = BTreeSet::new();
    for (expected, snippet) in forms {
        let design = build(&format!("const X: u32 = {snippet};"));
        assert!(lowering_codes(&design).is_empty(), "`{snippet}` must parse cleanly: {:?}", design.diagnostics());
        let kinds = expr_kinds(&design);
        assert!(kinds.contains(expected), "`{snippet}` yields no {expected} node, only {kinds:?}");
        assert!(!kinds.contains("Error"), "`{snippet}` must not yield an Error node");
        seen.insert(expected);
    }
    assert_eq!(seen.len(), 30);
    // The outermost node of a whole-value form is the form itself.
    let design = build("const X: u32 = a + b;");
    let value = design.find("X").and_then(|d| d.as_const()).and_then(|c| c.value()).unwrap();
    assert_eq!(expr_kind_name(value.kind()), "Binary");
    assert_eq!(value.text(), "a + b");
}

#[test]
fn error_expressions_become_error_nodes() {
    let design = build("const X: u32 = emit!(1);");
    assert!(!design.diagnostics().is_empty());
    assert!(expr_kinds(&design).contains("Error"));
}

#[test]
fn statement_pattern_and_type_forms_lower() {
    let src = "\
const X: u32 = {
    let a = 1;
    let (b, c) = (1, 2);
    let S { d, e: f, .. } = s;
    let T(g, ..) = t;
    let Phase::Liquid = p;
    let h @ 1..=5 = n;
    let (1 | 2) = n;
    let [i, j] = arr;
    let _ = z;
    let -1 = n;
    const K: u32 = 1;
    ;
    a + 1;
    a +> b;
    a <- b after c;
    signal sg: T = 1;
    on ev { }
    let t1: [u8; 4] = x;
    let t3: (A, B) = x;
    let t5: flip T = x;
    let t6: _ = x;
    a + 1
};";
    let design = build(src);
    let stmts: BTreeSet<_> =
        (0..design.stmt_count()).map(|i| stmt_kind_name(&design.stmt_node(StmtId(i as u32)).kind)).collect();
    assert_eq!(
        stmts.into_iter().collect::<Vec<_>>(),
        ["Commit", "Contribution", "Empty", "Expr", "Item", "Let", "Reaction", "Signal"]
    );
    let pats: BTreeSet<_> = (0..design.pat_count()).map(|i| pat_kind_name(&design.pat_node(PatId(i as u32)).kind)).collect();
    assert_eq!(
        pats.into_iter().collect::<Vec<_>>(),
        ["Array", "Ident", "Lit", "Or", "Path", "Range", "Rest", "Struct", "Tuple", "TupleStruct", "Wild"]
    );
    let types: BTreeSet<_> =
        (0..design.type_count()).map(|i| type_kind_name(&design.type_node(TypeId(i as u32)).kind)).collect();
    assert_eq!(types.into_iter().collect::<Vec<_>>(), ["Array", "Flip", "Infer", "Path", "Tuple"]);

    // One Binding per bound identifier, owned by the const, in source order.
    let x = design.find("X").expect("const X");
    let kids = x.children();
    let bound: Vec<_> = kids
        .iter()
        .filter_map(|k| match k.kind() {
            DeclKind::Binding(b) => Some((k.name().unwrap(), b.origin)),
            _ => None,
        })
        .collect();
    let names: Vec<_> = bound.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, ["a", "b", "c", "d", "f", "g", "h", "i", "j", "t1", "t3", "t5", "t6"]);
    assert!(bound.iter().all(|(_, o)| *o == BindingOrigin::Local));
    // The item, the signal and the reaction are children too, in place.
    let labels: Vec<_> = kids.iter().map(|k| k.kind().label()).collect();
    assert!(labels.contains(&"Const") && labels.contains(&"Signal") && labels.contains(&"Reaction"));
    let k = kids.iter().find(|k| k.name() == Some("K")).unwrap();
    assert_eq!(k.declared_in(), Some(x));
    assert!(design.check().is_empty(), "{:?}", design.check());
}

/// A source with every body-node kind, nested in a const, a fn, a module and
/// both behavior blocks.
const AUDIT_SRC: &str = "\
const A: u32 = { let (b, c) = (1, 2); if b < c { b } else { c } };
fn f(x: u32, y: [u8; 4]) -> u32 {
    let z: u32 = x;
    match x { 0 | 1 => 2, n if n > 3 => n, _ => 0 }
}
module M(a: electrical, b: electrical) {
    signal s: u32 = 1;
}
analog for M {
    a +> b.v / 2.0;
    let k: f64 = 3.0;
}
digital for M {
    on rising(a) { s <- s + 1 after 2; }
}
";

#[test]
fn node_text_is_authored_text_and_enclosing_is_nearest_decl() {
    // Specific, independently written expectations.
    let design = build("const X: u32 = { let a = 1; a + 1 };");
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let x = design.find("X").unwrap();
    let value = x.as_const().and_then(|c| c.value()).unwrap();
    assert_eq!(value.text(), "{ let a = 1; a + 1 }");
    assert_eq!(value.enclosing(), x);
    let texts: BTreeSet<&str> = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32)).text()).collect();
    for want in ["1", "a + 1", "a"] {
        assert!(texts.contains(want), "no expression with text {want:?} in {texts:?}");
    }
    let pat_texts: Vec<&str> = (0..design.pat_count()).map(|i| design.pat(PatId(i as u32)).text()).collect();
    assert_eq!(pat_texts, ["a"]);
    let ty_texts: Vec<&str> = (0..design.type_count()).map(|i| design.ty(TypeId(i as u32)).text()).collect();
    assert_eq!(ty_texts, ["u32"]);

    // A body inside a function is enclosed by the function, not the file.
    let design = build("fn f(x: u32) -> u32 { x + 1 }");
    let f = design.find("f").unwrap();
    let body_expr = (0..design.expr_count()).map(|i| design.expr(ExprId(i as u32))).find(|e| e.text() == "x + 1").unwrap();
    assert_eq!(body_expr.enclosing(), f);
    // A parameter's type is enclosed by the parameter.
    let param = f.children().into_iter().find(|c| c.name() == Some("x")).unwrap();
    let ty = (0..design.type_count()).map(|i| design.ty(TypeId(i as u32))).find(|t| t.text() == "u32" && t.enclosing() == param);
    assert!(ty.is_some(), "the parameter type is enclosed by the parameter");

    // The audit: every node of every arena, nearest declaration that contains it.
    let design = build(AUDIT_SRC);
    assert!(lowering_codes(&design).is_empty(), "{:?}", design.diagnostics());
    let text = design.files().into_iter().filter(|f| !f.krate().is_std()).last().unwrap().text();
    let decls: Vec<_> = (0..design.decl_count()).map(|i| (DeclId(i as u32), design.decl_data(DeclId(i as u32)))).filter(|(id, _)| !design.decl(*id).file().krate().is_std()).collect();
    let nearest = |span: piperine_lang::msl::Span, owner: DeclId| {
        // The innermost non-binding declaration whose span holds `span`.
        let best = decls
            .iter()
            .filter(|(_, d)| !matches!(d.kind, DeclKind::Binding(_)))
            .filter(|(_, d)| d.span.start <= span.start && span.end <= d.span.end && d.parent.is_some())
            .min_by_key(|(_, d)| d.span.len())
            .map(|(id, _)| *id);
        assert_eq!(best, Some(owner), "span {span:?} ({:?}) is enclosed by the wrong declaration", &text[span.range()]);
    };
    let mut audited = 0;
    for i in 0..design.expr_count() {
        let e = design.expr(ExprId(i as u32));
        assert!(!e.span().is_empty(), "expression {i} has an empty span");
        assert_eq!(e.text(), &text[e.span().range()]);
        nearest(e.span(), e.enclosing().id());
        audited += 1;
    }
    for i in 0..design.stmt_count() {
        let s = design.stmt(StmtId(i as u32));
        assert_eq!(s.text(), &text[s.span().range()]);
        nearest(s.span(), s.enclosing().id());
        audited += 1;
    }
    for i in 0..design.pat_count() {
        let p = design.pat(PatId(i as u32));
        assert_eq!(p.text(), &text[p.span().range()]);
        nearest(p.span(), p.enclosing().id());
        audited += 1;
    }
    for i in 0..design.type_count() {
        let t = design.ty(TypeId(i as u32));
        assert_eq!(t.text(), &text[t.span().range()]);
        nearest(t.span(), t.enclosing().id());
        audited += 1;
    }
    for i in 0..design.path_count() {
        let p = design.path(PathId(i as u32));
        assert_eq!(p.text(), &text[p.span().range()]);
        nearest(p.span(), p.enclosing().id());
        audited += 1;
    }
    assert!(audited > 60, "the audit must cover a real body, audited {audited}");
}

#[test]
fn error_nodes_are_kept() {
    // Item.
    let src = "struct ;";
    let design = build(src);
    assert!(!design.diagnostics().is_empty());
    let kids = design.root().children();
    let err = kids.iter().find(|k| matches!(k.kind(), DeclKind::Error)).expect("an Error item");
    assert_eq!(err.text(), src);

    // Module item.
    let src = "module M() { module N() {} }";
    let design = build(src);
    assert!(codes(&design).contains(&"E0303".to_string()));
    let m = design.find("M").unwrap();
    let err = m.children().into_iter().find(|k| matches!(k.kind(), DeclKind::Error)).expect("an Error module item");
    assert_eq!(err.text(), "module N() {}");
    assert_eq!(err.declared_in(), Some(m));

    // Digital item.
    let src = "digital for M { q <- 1; }";
    let design = build(src);
    assert!(codes(&design).contains(&"E0434".to_string()));
    let beh = design.root().children().into_iter().find(|k| matches!(k.kind(), DeclKind::Behavior(_))).expect("behavior");
    let err = beh.children().into_iter().find(|k| matches!(k.kind(), DeclKind::Error)).expect("an Error digital item");
    assert_eq!(err.text(), "q <- 1;");

    // Function parameter.
    let src = "fn f(a: u32, ...) { }";
    let design = build(src);
    assert!(codes(&design).contains(&"E0143".to_string()));
    let f = design.find("f").unwrap();
    let err = f
        .children()
        .into_iter()
        .find(|k| matches!(k.kind(), DeclKind::FnParam(FnParamData::Error)))
        .expect("an Error fn param");
    assert_eq!(err.text(), "...");

    // Expression.
    let design = build("const X: u32 = emit!(1);");
    let e = (0..design.expr_count())
        .map(|i| design.expr(ExprId(i as u32)))
        .find(|e| matches!(e.kind(), ExprKind::Error))
        .expect("an Error expression");
    assert_eq!(e.text(), "emit!(1)");
    assert!(design.check().is_empty(), "{:?}", design.check());
}

#[test]
fn depth_256_builds_on_default_stack() {
    // The test thread has the default 2 MiB stack. The parser's limit is 256
    // levels: 256 nested parentheses parse, and the build must complete.
    for open_close in [("(", ")"), ("{ ", " }"), ("-", ""), ("[", "]")] {
        let (open, close) = open_close;
        let src = format!("const X: u32 = {}1{};", open.repeat(256), close.repeat(256));
        let design = build(&src);
        assert!(design.expr_count() >= 256, "`{open}` x256: {} expression nodes", design.expr_count());
        assert!(design.check().is_empty(), "{:?}", design.check());
    }
    // One past the limit is reported, not a crash.
    let src = format!("const X: u32 = {}1{};", "(".repeat(300), ")".repeat(300));
    let design = build(&src);
    assert!(!design.diagnostics().is_empty());
}

/// Parse `src`, build it, rebuild the tree from the arenas and demand the same
/// tree, spans included (`Loc` equality ignores spans, `Debug` prints them).
fn assert_round_trips(src: &str) {
    let parsed = piperine_lang::msl::parser::parse(src).file;
    let design = build(src);
    let rebuilt = design.files()[0].to_ast();
    assert_eq!(rebuilt, parsed, "structure differs for:\n{src}");
    assert_eq!(format!("{rebuilt:?}"), format!("{parsed:?}"), "spans differ for:\n{src}");
}

#[test]
fn value_bodies_round_trip() {
    // Every expression form, as the value of a const.
    let forms = [
        "1", "2.5e3", "\"s\"", "true", "a", "-a", "!a", "a + b * c - d / e % f", "a ^ b & c | d",
        "a << 1 >> 2", "a && b || c", "a == b", "a != b", "a < b", "a <= b", "a > b", "a >= b", "a = b", "a += b",
        "a <<= b", "a as f64", "f(1, x: 2)", "a.m(1)", "a.m::<u8>(1, x: 2)", "a.b", "t.0", "a[0]", "0..4", "0..=4", "..4",
        "0..", "..", "(a)", "(a, b)", "()", "[1, 2]", "[0; 4]", "S { x: 1, y: 2 }", "{ 1 }", "{ }",
        "if a { 1 } else { 2 }", "if a { 1 } else if b { 2 } else { 3 }", "if a { 1 }", "if let Some(x) = y { 1 }",
        "match a { _ => 1 }", "match a { 0 | 1 => 2, n if n > 3 => n, #[x] _ => 0, }", "while a { }", "loop { }",
        "for i in 0..4 { }", "loop { break; }", "loop { break 1; }", "loop { continue; }", "return 1", "return",
        "|x| x + 1", "|x: u32, y| -> u32 { x }", "a?", "emit!(\"s\", a = 1)", "include_str!(\"f.txt\")",
        "m(1) { w: 2 }", "a::b::<T>::c", "self::a", "super::a", "crate::a",
    ];
    for form in forms {
        assert_round_trips(&format!("const X: u32 = {form};"));
    }
    // Statements, patterns and types.
    assert_round_trips(
        "\
const X: u32 = {
    let a = 1;
    let b: u32;
    let (c, d) = (1, 2);
    let S { e, f: g, .. } = s;
    let T(h, ..) = t;
    let Phase::Liquid = p;
    let i @ 1..=5 = n;
    let (1 | 2) = n;
    let [j, k] = arr;
    let _ = z;
    let -1 = n;
    let Some(v) = o else { return };
    let 1.. = n;
    let ..5 = n;
    const K: u32 = 1;
    fn inner(x: u32) -> u32 { x }
    ;
    a + 1;
    a +> b;
    a <- b after c;
    a <- b;
    signal sg: T = 1;
    signal sg2: T;
    on ev { x <- 1; }
    let t1: [u8; 4] = x;
    let t3: (A, B) = x;
    let t6: flip T = x;
    let t7: _ = x;
    let t8: () = x;
    let t9: Vec<u8, { N }, 3> = x;
    let t10: Box<Fn(A, B) -> C> = x;
    a + 1
};",
    );
}

#[test]
fn fns_consts_metas_and_generics_round_trip() {
    for src in [
        "const A: u32 = 1;",
        "pub const A: u32 = 1;",
        "/// docs\n#[a] #[b(1, \"s\", k = 2, c::d)] #[e = 3] pub const A: u32 = 1;",
        "/** block */ const A: u32 = 1;",
        "//! inner\n//! more\nconst A: u32 = 1;",
        "fn f() { }",
        "pub const fn f(self, x: u32, y: f64) -> f64 where x > 0 { x }",
        "fn f<T>() { }",
        "fn f<>() { }",
        "fn f<T: A + B = C, const N: u32 = 4, U>(a: [T; N]) -> T { a[0] }",
        "extern \"C\" fn erfcx(x: f64) -> f64;",
        "#[derivative(erfcx_dx)] extern \"C\" fn erfcx(x: f64) -> f64;",
        "fn f() -> u32 { const K: u32 = 1; fn g() { } K }",
        "const A: u32 = 1;\nconst B: u32 = 2;\nfn f() { }",
        "",
        "  // a comment\n",
    ] {
        assert_round_trips(src);
    }
}

#[test]
fn function_pointer_type_round_trips() {
    // POL-09 AC 4: a function-pointer type lowers to `TypeKind::FnPtr`
    // (params and return as type nodes) and converts back to the parsed tree.
    let src = "fn apply(f: fn(f64) -> f64, g: fn(), h: fn([u8; 2]) -> u8) {}\nstruct S { r: fn(f64) -> f64 }";
    assert_round_trips(src);
    let design = build(src);
    assert!(codes(&design).is_empty(), "{:?}", codes(&design));
    let ptrs: Vec<_> = (0..design.type_count())
        .filter_map(|i| match &design.type_node(TypeId(i as u32)).kind {
            TypeKind::FnPtr { params, ret } => Some((params.len(), ret.is_some())),
            _ => None,
        })
        .collect();
    assert_eq!(ptrs.len(), 4);
    assert!(ptrs.contains(&(1, true)) && ptrs.contains(&(0, false)));
}

/// One source per expression form, written in positions that make every form a node of a body.
const WALK_SOURCES: [&str; 12] = [
    "fn f(a: f64, b: f64) -> f64 { let c = a + -b; let d = (a, b)[0]; c as f64 }",
    "fn f(a: f64) -> f64 { g(a, x: 1.0) + a.m(2.0, y: a)[0] }",
    "fn f(a: f64) -> f64 { if a > 0.0 { a } else if a < 1.0 { 2.0 } else { 3.0 } }",
    "fn f(a: f64) -> f64 { match a { x if x > 0.0 => x, _ => 0.0 } }",
    "fn f(a: f64) -> f64 { let mut t = 0.0; for i in 0..4 { t += a; } while t < 3.0 { t = t + 1.0; } loop { break t; } }",
    "fn f(a: f64) -> f64 { let h = |x: f64| x + a; let v = [a; 4]; let w = [a, a]; let s = S { x: a, y: 2.0 }; h(a)? }",
    "fn f(a: f64) -> f64 { let Some(x) = g(a) else { return a + 1.0 }; if let Some(y) = g(x) { y } else { x.0 } }",
    "fn f(a: f64) -> f64 { { a }; return a; }",
    "fn f(a: f64) { loop { continue; } }",
    "const X: f64 = emit_free(1.0) + 2.0;",
    "module M(a: Bit) { signal s: Bit = 1.0; let k = 2.0 + 3.0; s = a after 1.0 + 2.0; if k > 1.0 { signal t: Bit; t = a; } }",
    "module M(a: Bit) {}\ndigital for M { on a.rise() { let q = 1.0 + 2.0; a <- q; } }",
];

#[test]
fn expr_children_and_parent_are_inverse() {
    let mut with_children = 0;
    for src in WALK_SOURCES {
        let design = build(src);
        let mut parented = 0;
        for i in 0..design.expr_count() {
            let e = design.expr(ExprId(i as u32));
            for c in e.children() {
                assert_eq!(c.parent(), Some(e), "`{}` must report `{}` as its parent in {src}", c.text(), e.text());
            }
            if !e.children().is_empty() {
                with_children += 1;
            }
            if let Some(p) = e.parent() {
                parented += 1;
                assert!(p.children().contains(&e), "`{}` names `{}` as its parent, which does not list it, in {src}", e.text(), p.text());
            }
        }
        assert!(parented > 0, "{src}: no expression has a parent");
        assert!(design.check().is_empty(), "{:?}", design.check());
    }
    assert!(with_children > 30);

    // Direct sub-expressions, in source order, for the forms with a known shape.
    let design = build("const X: f64 = f(a + b, c: d) { p: e };");
    let call = design.find("X").and_then(|d| d.as_const()).and_then(|c| c.value()).unwrap();
    let texts: Vec<_> = call.children().iter().map(|c| c.text()).collect();
    assert_eq!(texts, ["f", "a + b", "d", "e"]);
    assert_eq!(call.parent(), None, "nothing contains the whole value");
    let sum = call.children()[1];
    assert_eq!(sum.parent(), Some(call));
    assert_eq!(sum.children().iter().map(|c| c.text()).collect::<Vec<_>>(), ["a", "b"]);
    assert!(sum.children()[0].children().is_empty());

    // A block form lists the expressions of its statements; the condition comes first, `else` last.
    let design = build("fn f(a: f64) -> f64 { if a > 0.0 { a; 1.0 } else { 2.0 } }");
    let body = design.function("f").unwrap().exprs();
    let iff = body[0];
    assert_eq!(iff.children().iter().map(|c| c.text()).collect::<Vec<_>>(), ["a > 0.0", "a", "1.0", "{ 2.0 }"]);
    assert_eq!(iff.children()[1].parent(), Some(iff));
    assert_eq!(iff.parent(), None, "a statement's expression has no expression above it");
}

#[test]
fn decl_exprs_is_preorder_over_the_body() {
    let src = "\
fn f(x: f64) -> f64 {
    let y = g(1.0);
    fn inner() -> f64 { k(2.0) }
    if y > 0.0 { h(y) } else { 0.0 }
}
";
    let design = build(src);
    let texts = |d: &Design, name: &str| d.function(name).unwrap().exprs().iter().map(|e| e.text().to_string()).collect::<Vec<_>>();
    assert_eq!(
        texts(&design, "f"),
        ["g(1.0)", "g", "1.0", "if y > 0.0 { h(y) } else { 0.0 }", "y > 0.0", "y", "0.0", "h(y)", "h", "y", "{ 0.0 }", "0.0"],
    );
    let inner = design.decls().into_iter().find(|d| d.name() == Some("inner")).unwrap();
    assert_eq!(inner.exprs().iter().map(|e| e.text()).collect::<Vec<_>>(), ["k(2.0)", "k", "2.0"]);
    // Pre-order: a node comes before its children and after its previous sibling's subtree.
    let all = design.function("f").unwrap().exprs();
    for (at, e) in all.iter().enumerate() {
        for c in e.children() {
            assert!(all.iter().position(|x| *x == c).unwrap() > at);
        }
    }

    // A module, a behavior block and a reaction list their own expressions.
    let design = build("module M(a: Bit) { let k = 1.0 + 2.0; s = a; signal s: Bit = 4.0; }\nanalog for M { let z = 5.0; }\n");
    let m = design.module("M").unwrap();
    assert_eq!(m.exprs().iter().map(|e| e.text()).collect::<Vec<_>>(), ["1.0 + 2.0", "1.0", "2.0", "s", "a", "4.0"]);
    let block = m.behaviors()[0];
    assert_eq!(block.exprs().iter().map(|e| e.text()).collect::<Vec<_>>(), ["5.0"]);
    let design = build("module M(a: Bit) {}\ndigital for M { on a.rise() { let q = 1.0 + 2.0; } }\n");
    let reaction = design.module("M").unwrap().behaviors()[0].reactions()[0];
    assert_eq!(reaction.exprs().iter().map(|e| e.text()).collect::<Vec<_>>(), ["a.rise()", "a", "1.0 + 2.0", "1.0", "2.0"]);
}

#[test]
fn check_reports_a_broken_parent_table() {
    let design = build("const X: f64 = a + b;");
    assert!(design.check().is_empty());
    let mut json: serde_json::Value = serde_json::to_value(&design).unwrap();
    let table = json["expr_parent"].as_array_mut().expect("the parent table serializes with the design");
    assert_eq!(table.len(), design.expr_count());
    // `a` (index 1) is a child of the sum; claim it is nobody's child.
    let at = table.iter().position(|p| !p.is_null()).unwrap();
    table[at] = serde_json::Value::Null;
    let broken: Design = serde_json::from_value(json).unwrap();
    assert!(!broken.check().is_empty());
}

#[test]
fn block_and_stmt_handles_walk_a_body() {
    let src = "fn f(a: f64) -> f64 { let b = g(a); if a > 0.0 { h(b); } b + 1.0 }\nfn e() { 1.0; }\nfn n() {}\nfn g(x: f64) -> f64 { x }\nfn h(x: f64) {}";
    let design = build(src);
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    // POL-20 AC 6: the handle is `pom::Block`; its storage type is `BlockNode`.
    let body: piperine_lang::pom::Block<'_> = design.function("f").unwrap().body().unwrap();
    let _raw: &piperine_lang::pom::BlockNode = body.node();
    assert_eq!(body.span().range(), 20..src.find("\nfn e").unwrap());
    let stmts = body.stmts();
    assert_eq!(stmts.len(), 3);
    assert_eq!(stmts.iter().map(|s| s.text()).collect::<Vec<_>>(), ["let b = g(a);", "if a > 0.0 { h(b); }", "b + 1.0"]);
    assert_eq!(body.value().unwrap().text(), "b + 1.0");
    assert_eq!(body.node().stmts.len(), 3, "the raw block stays reachable as node()");
    assert!(design.function("e").unwrap().body().unwrap().value().is_none(), "a trailing `;` makes no value");
    assert!(design.function("n").unwrap().body().unwrap().stmts().is_empty());

    // A statement lists its direct expressions; a nested statement belongs to its own block.
    assert_eq!(stmts[0].children().iter().map(|e| e.text()).collect::<Vec<_>>(), ["g(a)"]);
    assert_eq!(stmts[2].children().iter().map(|e| e.text()).collect::<Vec<_>>(), ["b + 1.0"]);
    let call_g = stmts[0].children()[0];
    assert_eq!(call_g.stmt(), Some(stmts[0]));
    let a_in_g = call_g.children()[1];
    assert_eq!(a_in_g.text(), "a");
    assert_eq!(a_in_g.stmt(), Some(stmts[0]), "the nearest enclosing statement of a nested expression");
    let h_call = design.function("f").unwrap().exprs().into_iter().find(|e| e.text() == "h(b)").unwrap();
    let inner = h_call.stmt().unwrap();
    assert_eq!(inner.text(), "h(b);");
    assert_ne!(Some(inner), stmts.get(1).copied(), "the `if` statement is not the nearest statement of what its block holds");
    let b_in_h = h_call.children()[1];
    assert_eq!(b_in_h.stmt(), Some(inner));
    // Round trip: every statement child names its statement.
    for s in &stmts {
        for c in s.children() {
            assert_eq!(c.stmt(), Some(*s));
        }
    }
    // A condition that is no statement's expression still finds the statement above it.
    let cond = stmts[1].children()[0].children()[0];
    assert_eq!(cond.text(), "a > 0.0");
    assert_eq!(cond.stmt(), Some(stmts[1]));

    // A reaction's body is a handle too.
    let design = build("module M(a: Bit) {}\ndigital for M { on a.rise() { a <- 1.0 after 2.0; } }");
    let reaction = design.module("M").unwrap().behaviors()[0].reactions()[0];
    let stmts = reaction.body().stmts();
    assert_eq!(stmts.len(), 1);
    assert_eq!(stmts[0].children().iter().map(|e| e.text()).collect::<Vec<_>>(), ["a", "1.0", "2.0"]);
    assert_eq!(reaction.body().node().stmts.len(), 1);
}
