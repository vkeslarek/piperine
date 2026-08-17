//! `constraint begin … end` — the sanctioned Piperine extension (MD-49)
//! as grammar: parses as a module body item beside `analog begin … end`,
//! `require name : pred [tol scale];` statements inside, contextual
//! keywords only (`constraint`/`require`/`tol` are identifier text at
//! their production sites, never Annex B reserved words), and anything
//! else inside the block is a typed error — never a silent skip.

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
    assert_eq!(block.requires.len(), 1);
    let req = &block.requires[0];
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
    let names: Vec<&str> = block.requires.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["vgs_ox", "vds_ox"], "authored order preserved");
}

#[test]
fn optional_tol_clause_parses_with_its_expression() {
    let block = constraint_block(
        "constraint begin require head : V(p, n) <= hi tol 0.1; end",
    );
    let req = &block.requires[0];
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
fn a_non_require_statement_is_a_typed_error_naming_the_token() {
    let src = "constraint begin target ac_unity_gain_freq(V(out)) >= 10e6; end";
    let full = format!("module m(); {src} endmodule");
    let err = grammar(&full)
        .parse_module_decl()
        .expect_err("non-require statement must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("require"), "the error names what is legal: {msg}");
    assert!(!msg.is_empty(), "typed error, not a silent skip");
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
    assert!(block.requires.is_empty(), "no statements, no requires");
}
