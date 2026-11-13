//! Scope: body lowering (requirement POM-06): every expression, statement,
//! pattern and type form of the language becomes one node of the matching
//! kind, reachable from the declaration that owns it.

#[path = "support/pom.rs"]
mod support;

use std::collections::BTreeSet;

use piperine_lang::pom::{
    BindingOrigin, DeclId, DeclKind, Design, ExprId, ExprKind, FnParamData, PatId, PatKind, PathId, Reflect, StmtId,
    StmtKind, TypeId, TypeKind,
};
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
        PatKind::Ref(_) => "Ref",
        PatKind::Slice(_) => "Slice",
    }
}

fn type_kind_name(k: &TypeKind) -> &'static str {
    match k {
        TypeKind::Path(_) => "Path",
        TypeKind::Array { .. } => "Array",
        TypeKind::Slice(_) => "Slice",
        TypeKind::Tuple(_) => "Tuple",
        TypeKind::Ref { .. } => "Ref",
        TypeKind::Flip(_) => "Flip",
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
        assert!(codes(&design).is_empty(), "`{snippet}` must parse cleanly: {:?}", design.diagnostics());
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
    let &r = q;
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
    let t2: [u8] = x;
    let t3: (A, B) = x;
    let t4: &T = x;
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
        ["Ident", "Lit", "Or", "Path", "Range", "Ref", "Rest", "Slice", "Struct", "Tuple", "TupleStruct", "Wild"]
    );
    let types: BTreeSet<_> =
        (0..design.type_count()).map(|i| type_kind_name(&design.type_node(TypeId(i as u32)).kind)).collect();
    assert_eq!(types.into_iter().collect::<Vec<_>>(), ["Array", "Flip", "Infer", "Path", "Ref", "Slice", "Tuple"]);

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
    assert_eq!(names, ["a", "b", "c", "d", "f", "g", "h", "r", "i", "j", "t1", "t2", "t3", "t4", "t5", "t6"]);
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
    let z: &u32 = &x;
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
    assert!(codes(&design).is_empty(), "{:?}", design.diagnostics());
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
    assert!(codes(&design).is_empty(), "{:?}", design.diagnostics());
    let text = design.files().last().unwrap().text();
    let decls: Vec<_> = (0..design.decl_count()).map(|i| (DeclId(i as u32), design.decl_data(DeclId(i as u32)))).collect();
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
