//! Scope: body lowering (requirement POM-06): every expression, statement,
//! pattern and type form of the language becomes one node of the matching
//! kind, reachable from the declaration that owns it.

#[path = "support/pom.rs"]
mod support;

use std::collections::BTreeSet;

use piperine_lang::pom::{
    BindingOrigin, DeclKind, Design, ExprId, ExprKind, PatId, PatKind, Reflect, StmtId, StmtKind, TypeId, TypeKind,
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
