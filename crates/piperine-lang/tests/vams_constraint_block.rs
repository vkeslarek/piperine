//! `constraint begin … end` — the sanctioned Piperine extension (MD-49)
//! as grammar: parses as a module body item beside `analog begin … end`,
//! `require name : pred [tol scale];` statements inside, contextual
//! keywords only (`constraint`/`require`/`tol` are identifier text at
//! their production sites, never Annex B reserved words), and anything
//! else inside the block is a typed error — never a silent skip.
//!
//! SPEC-COVERS: part_iii#6

use piperine_lang::ast::*;
use piperine_lang::keywords::Keywords;
use piperine_lang::{Grammar, Lexer};

fn grammar(src: &str) -> Grammar {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error for {src:?}: {e}"));
    Grammar::new(toks)
}

fn one_module_item(src: &str) -> ModuleItem {
    let full = format!("module m(); {src} endmodule");
    let m = grammar(&full).parse_module_decl().unwrap_or_else(|e| panic!("parse error for {src:?}: {e}"));
    assert_eq!(m.items.len(), 1, "expected exactly one item for {src:?}, got {:?}", m.items);
    let mut items = m.items;
    items.remove(0)
}

fn constraint_block(src: &str) -> ConstraintBlockDecl {
    match one_module_item(src) {
        ModuleItem::Constraint(c) => c,
        other => panic!("expected a constraint block for {src:?}, got {other:?}"),
    }
}

#[test]
fn block_with_a_require_parses_as_a_module_item() {
    let block = constraint_block(
        "constraint begin require vgs_ox : abs(V(g, s)) <= vgs_max; end",
    );
    assert_eq!(block.stmts.len(), 1);
    let piperine_lang::ast::ConstraintStmt::Require(req) = &block.stmts[0] else {
        panic!("a require statement")
    };
    assert_eq!(req.name, "vgs_ox");
    assert!(req.tol.is_none(), "no tol clause authored");
    assert!(
        matches!(&req.pred, Expr::Binary { op: BinOp::Le, .. }),
        "predicate is the authored comparison: {:?}",
        req.pred
    );
}

#[test]
fn several_requires_parse_in_authored_order() {
    let block = constraint_block(
        "constraint begin
            require vgs_ox : abs(V(g, s)) <= vgs_max;
            require vds_ox : abs(V(d, s)) <= vds_max;
         end",
    );
    let requires: Vec<&str> = block
        .stmts
        .iter()
        .filter_map(|s| match s {
            ConstraintStmt::Require(r) => Some(r.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(requires, vec!["vgs_ox", "vds_ox"], "authored order preserved");
}

#[test]
fn optional_tol_clause_parses_with_its_expression() {
    let block = constraint_block(
        "constraint begin require head : V(p, n) <= hi tol 0.1; end",
    );
    let ConstraintStmt::Require(req) = &block.stmts[0] else {
        panic!("a require statement")
    };
    let tol = req.tol.as_ref().expect("tol clause authored");
    assert!(
        matches!(tol, Expr::Number { .. }),
        "the scale expression travels: {tol:?}"
    );
}

#[test]
fn attributes_on_the_block_are_carried() {
    let block = constraint_block(
        "(* soa *) constraint begin require r1 : V(p, n) <= 1.0; end",
    );
    assert_eq!(block.attrs.len(), 1, "attribute instances carried");
}

#[test]
fn an_unknown_statement_is_a_typed_error_naming_what_is_legal() {
    let src = "constraint begin expect_this : V(p, n) <= 1.0; end";
    let full = format!("module m(); {src} endmodule");
    let err = grammar(&full)
        .parse_module_decl()
        .expect_err("an unknown statement must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("constraint statement"), "the error names the statement forms: {msg}");
}

#[test]
fn an_annex_b_keyword_as_require_label_is_rejected() {
    let src = "constraint begin require begin : V(p, n) <= 1.0; end";
    let full = format!("module m(); {src} endmodule");
    let err = grammar(&full)
        .parse_module_decl()
        .expect_err("keyword label must fail loud");
    assert!(
        err.to_string().contains("begin"),
        "the error names the keyword: {err}"
    );
}

#[test]
fn contextual_keywords_stay_out_of_annex_b() {
    // The extension must not silently mutate the reserved set — LRM
    // sources keep the right to use these words as identifiers elsewhere.
    for word in ["constraint", "require", "tol"] {
        assert!(
            !Keywords::is_reserved(word),
            "`{word}` must remain a contextual keyword, not an Annex B reserved word"
        );
    }
}

#[test]
fn require_and_tol_remain_usable_as_identifiers_outside_the_block() {
    // A net and a parameter named `require`/`tol` still parse — the words
    // only mean something at their production sites inside the block.
    let m = grammar(
        "module m(input electrical p, input electrical n);
            electrical require;
            parameter real tol = 1.0;
            constraint begin require head : V(p, n) <= tol; end
         endmodule",
    )
    .parse_module_decl()
    .expect("contextual keywords do not leak into ordinary declarations");
    assert!(m.items.iter().any(|i| matches!(i, ModuleItem::Constraint(_))));
}

#[test]
fn a_require_with_no_colon_is_an_error() {
    let src = "constraint begin require head V(p, n) <= 1.0; end";
    let full = format!("module m(); {src} endmodule");
    assert!(
        grammar(&full).parse_module_decl().is_err(),
        "the `:` between label and predicate is mandatory"
    );
}

#[test]
fn an_empty_block_parses_with_no_requires() {
    let block = constraint_block("constraint begin end");
    assert!(block.stmts.is_empty(), "no statements");
}

// ── elaboration onto the POM ───────────────────────────────────────────────

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

const CONSTRAINED: &str = r#"
    module dut (inout electrical p, inout electrical n);
        parameter real hi = 3.3;
        analog begin
            I(p, n) <+ 0.0;
        end
        constraint begin
            require head : V(p, n) <= hi;
            require lowlim : V(p, n) >= -hi tol 0.1;
        end
    endmodule
"#;

#[test]
fn requires_are_visible_on_the_pom_in_authored_order() {
    let d = elab(CONSTRAINED);
    let m = d.module("dut").expect("dut elaborated");
    let cs = m.constraints();
    assert_eq!(cs.len(), 2, "both requires land on the POM");
    assert_eq!(cs[0].label(), Some("head"));
    assert_eq!(cs[1].label(), Some("lowlim"));
    assert!(
        matches!(&cs[1], piperine_lang::pom::PomConstraintStmt::Require(r) if r.tol.is_some()),
        "the tol clause travels"
    );
    assert!(cs[0].label_span().is_some(), "provenance spans preserved");
}

#[test]
fn multiple_constraint_blocks_compose_none_dropped() {
    let d = elab(r#"
        module dut (inout electrical p, inout electrical n);
            constraint begin require a1 : V(p, n) <= 1.0; end
            constraint begin require a2 : V(p, n) >= -1.0; end
        endmodule
    "#);
    let names: Vec<&str> = d.module("dut").unwrap().constraints().iter().filter_map(|s| s.label()).collect();
    assert_eq!(names, vec!["a1", "a2"], "both blocks' requires apply in authored order");
}

#[test]
fn a_duplicate_require_label_fails_loud_naming_both() {
    let msg = elab_err(r#"
        module dut (inout electrical p, inout electrical n);
            constraint begin
                require head : V(p, n) <= 1.0;
                require head : V(p, n) <= 2.0;
            end
        endmodule
    "#);
    assert!(msg.contains("head"), "names the duplicated label: {msg}");
    assert!(msg.contains("duplicate"), "says what happened: {msg}");
}

#[test]
fn a_duplicate_label_across_blocks_fails_loud_too() {
    let msg = elab_err(r#"
        module dut (inout electrical p, inout electrical n);
            constraint begin require head : V(p, n) <= 1.0; end
            constraint begin require head : V(p, n) <= 2.0; end
        endmodule
    "#);
    assert!(msg.contains("head"), "composition does not open a shadowing hole: {msg}");
}

#[test]
fn a_module_without_a_block_has_no_constraints() {
    let d = elab(r#"
        module dut (inout electrical p, inout electrical n);
            analog begin I(p, n) <+ 0.0; end
        endmodule
    "#);
    assert!(d.module("dut").unwrap().constraints().is_empty());
}

#[test]
fn an_uninstantiated_constrained_module_elaborates_silently() {
    // Edge case: nothing is evaluated and no diagnostic is produced for
    // a constrained module that is never instantiated.
    let d = elab(CONSTRAINED);
    assert_eq!(d.module("dut").unwrap().constraints().len(), 2);
}

// ── var / target / cover statements (`constraint-statements`) ──────────────

#[test]
fn a_var_statement_parses_without_a_type_annotation() {
    let block = constraint_block(
        "constraint begin var ugbw = ac_unity_gain_freq(V(out, gnd)); end",
    );
    let ConstraintStmt::Var { name, .. } = &block.stmts[0] else {
        panic!("a var statement, got {:?}", block.stmts[0])
    };
    assert_eq!(name, "ugbw");
}

#[test]
fn a_target_parses_as_a_single_comparison_with_optional_tol() {
    let block = constraint_block(
        "constraint begin target ugbw >= 1e4 tol 1e3; end",
    );
    let ConstraintStmt::Target { expr, cmp, level, tol, .. } = &block.stmts[0] else {
        panic!("a target statement, got {:?}", block.stmts[0])
    };
    assert!(matches!(expr, Expr::Ident { .. }), "the operand is the var read");
    assert_eq!(*cmp, TargetCmp::Ge);
    assert!(matches!(level, Expr::Number { .. }), "the level is a literal");
    assert!(tol.is_some(), "the tol clause travels");
}

#[test]
fn a_target_without_a_comparison_is_a_typed_error() {
    let src = "constraint begin target ugbw; end";
    let full = format!("module m(); {src} endmodule");
    let err = grammar(&full).parse_module_decl().expect_err("a target needs <= or >=");
    assert!(err.to_string().contains("comparison"), "names what is legal: {err}");
}

#[test]
fn cover_range_and_set_bins_parse() {
    let block = constraint_block(
        "constraint begin
             cover c1 : V(p, n) bins [0.0:1.0:3.0];
             cover c2 : V(p, n) bins in {0.25, 0.75};
             cover cross joint : c1, c2;
         end",
    );
    assert_eq!(block.stmts.len(), 3, "three statements in authored order");
    assert!(matches!(&block.stmts[0], ConstraintStmt::Cover { name, bins: CoverBins::Range { .. }, .. } if name == "c1"));
    assert!(matches!(&block.stmts[1], ConstraintStmt::Cover { name, bins: CoverBins::Set(_), .. } if name == "c2"));
    assert!(matches!(&block.stmts[2], ConstraintStmt::CoverCross { name, points, .. } if name == "joint" && points.len() == 2));
}

#[test]
fn statements_land_on_the_pom_in_authored_order_across_kinds() {
    let d = elab(r#"
        module dut (inout electrical p, inout electrical n);
            parameter real hi = 0.4;
            analog begin
                I(p, n) <+ V(p, n) / 1e3;
            end
            constraint begin
                var x = hi;
                require head : V(p, n) <= hi;
                target x <= hi;
                cover c1 : V(p, n) bins [0.0:1.0:3.0];
            end
        endmodule
    "#);
    let cs = d.module("dut").expect("dut elaborated").constraints();
    assert_eq!(cs.len(), 4, "all four kinds, one ordered list");
    assert!(matches!(cs[0], piperine_lang::pom::PomConstraintStmt::Var { .. }));
    assert!(matches!(cs[1], piperine_lang::pom::PomConstraintStmt::Require { .. }));
    assert!(matches!(cs[2], piperine_lang::pom::PomConstraintStmt::Target { .. }));
    assert!(matches!(cs[3], piperine_lang::pom::PomConstraintStmt::Cover { .. }));
}

#[test]
fn a_duplicate_label_across_kinds_fails_loud() {
    let msg = elab_err(r#"
        module dut (inout electrical p, inout electrical n);
            constraint begin
                var head = 1.0;
                require head : V(p, n) <= 1.0;
            end
        endmodule
    "#);
    assert!(msg.contains("head"), "names the duplicated label: {msg}");
    assert!(msg.contains("duplicate"), "says what happened: {msg}");
}

/// CONSTMT-01: a var reading an earlier var substitutes transitively —
/// and an unknown name in a var fails loud at compile (the placeholder
/// that reaches the kernel is a loud unresolved-identifier error, never
/// a silent 0.0).
#[test]
fn vars_substitute_transitively_at_the_lowering() {
    // The value-level proof lives in codegen (margin_lowering's PAR);
    // here we prove the authored order survives onto the POM — the
    // second var legitimately references the first.
    let d = elab(r#"
        module dut (inout electrical p, inout electrical n);
            constraint begin
                var base = 1.0;
                var twice = base * 2.0;
                require head : twice <= 4.0;
            end
        endmodule
    "#);
    let cs = d.module("dut").expect("dut elaborated").constraints();
    assert_eq!(cs.len(), 3, "authored order preserved: var, var, require");
    // Compiles: the transitive substitution resolves every name.
    let bodies = piperine_codegen::resolve::lower_bodies(&d).expect("lowers");
    let margins = bodies["dut"].constraints.as_ref().expect("margins lowered");
    assert_eq!(margins[0].label, "head", "the vars ride inside the require, not as margins");
    assert_eq!(margins.len(), 1);
}

#[test]
fn a_var_referencing_an_unknown_name_fails_loud_at_compile() {
    let d = elab(r#"
        module dut (inout electrical p, inout electrical n);
            analog begin
                I(p, n) <+ V(p, n) / 1e3;
            end
            constraint begin
                var bad = nope + 1.0;
                require head : bad <= 4.0;
            end
        endmodule
    "#);
    let bodies = piperine_codegen::resolve::lower_bodies(&d).expect("lowering keeps authored idents");
    let err = match piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["dut"]) {
        Ok(_) => panic!("an unresolved var name must fail loud, not compile"),
        Err(e) => format!("{e:?}"),
    };
    assert!(err.contains("nope") || err.contains("unresolved"), "names it: {err}");
}

// ── @ dc/@ tran/@ ac scoping groups (`constraint-scoping`) ────────────────

#[test]
fn a_scope_group_parses_and_carries_analysis_and_window() {
    let block = constraint_block(
        "constraint begin
             @ tran(after = 5e-3, dur = 2e-3) { require vds : V(p, n) <= 0.4; }
             @ dc { require head : V(p, n) <= 1.0; }
         end",
    );
    let ConstraintStmt::Scope { analysis: ScopeAnalysis::Tran, window: Some(w), stmts, .. } =
        &block.stmts[0]
    else {
        panic!("a tran scope group, got {:?}", block.stmts[0])
    };
    assert!(matches!(w.after, Expr::Number { .. }));
    assert!(w.dur.is_some());
    assert_eq!(stmts.len(), 1);
    assert!(matches!(&block.stmts[1], ConstraintStmt::Scope { analysis: ScopeAnalysis::Dc, window: None, .. }));
}

#[test]
fn open_ended_window_and_loud_errors() {
    // after without dur is open-ended.
    let block = constraint_block("constraint begin @ tran(after = 1.0) { require a : V(p, n) <= 1.0; } end");
    assert!(matches!(&block.stmts[0], ConstraintStmt::Scope { window: Some(w), .. } if w.dur.is_none()));

    // Unknown analysis, nesting, dur without after: all loud.
    for src in [
        "constraint begin @ noise { require a : V(p, n) <= 1.0; } end",
        "constraint begin @ dc { @ tran { require a : V(p, n) <= 1.0; } } end",
        "constraint begin @ tran(dur = 1.0) { require a : V(p, n) <= 1.0; } end",
    ] {
        let full = format!("module m(); {src} endmodule");
        assert!(
            grammar(&full).parse_module_decl().is_err(),
            "must fail loud: {src}"
        );
    }
}

#[test]
fn scoped_statements_reach_the_pom_and_duplicate_labels_cross_scopes() {
    let d = elab(r#"
        module dut (inout electrical p, inout electrical n);
            analog begin
                I(p, n) <+ V(p, n) / 1e3;
            end
            constraint begin
                @ dc { require head : V(p, n) <= 1.0; }
            end
        endmodule
    "#);
    let cs = d.module("dut").expect("dut").constraints();
    assert!(matches!(&cs[0], piperine_lang::pom::PomConstraintStmt::Scope { analysis, .. } if analysis == "dc"));

    let err = parse_and_elaborate(
        r#"
        module dut (inout electrical p, inout electrical n);
            constraint begin
                require head : V(p, n) <= 1.0;
                @ dc { require head : V(p, n) <= 2.0; }
            end
        endmodule
        "#,
        &SourceMap::dummy(),
    )
    .expect_err("duplicate across scopes fails loud");
    assert!(err.to_string().contains("head"));
}
