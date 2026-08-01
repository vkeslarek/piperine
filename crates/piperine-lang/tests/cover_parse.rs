//! T32 (DVT-42) — `cover` grammar, POM, and the elaboration-time bin-count
//! cap: `cover <name> : <expr> bins [lo:step:hi];`, `… in {…};`, and
//! `cover cross <name> : <point1>, <point2>, …;` parse inside a
//! `constraint` block (mirrors `require`'s own parse path,
//! `parse/parser/constraint_decl.rs`), `cover` is a reserved word, and a
//! `cover cross`'s joint bin count is checked at elaboration against a
//! `Context`-raisable cap (D10).

use piperine_lang::parse::ast::{ConstraintStmt, CoverBins, Item};
use piperine_lang::{parse_and_elaborate, parse_str, SourceMap};

fn elaborate(src: &str) -> piperine_lang::pom::Design {
    parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &SourceMap::dummy())
        .expect("fixture must elaborate")
}

fn elaborate_err(src: &str) -> String {
    parse_and_elaborate(&format!("use piperine::disciplines;\n{src}"), &SourceMap::dummy())
        .expect_err("fixture must fail to elaborate")
        .to_string()
}

fn parse_ok(src: &str) -> piperine_lang::parse::ast::SourceFile {
    parse_str(src).expect("must parse")
}

/// `cover <name> : <expr> bins [lo:step:hi];` parses inside a `constraint`
/// block.
#[test]
fn cover_bins_form_parses() {
    let program = parse_ok(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; }\n\
         constraint M { cover c1 : x bins [0.0:0.5:2.0]; }",
    );
    let Item::ConstraintDecl(decl) = &program.items[1] else { panic!("expected a ConstraintDecl") };
    assert_eq!(decl.body.len(), 1);
    let ConstraintStmt::Cover { name, bins, .. } = &decl.body[0] else { panic!("expected a Cover stmt") };
    assert_eq!(name, "c1");
    assert!(matches!(bins, CoverBins::Range { .. }), "the bins form parses to CoverBins::Range");
}

/// `cover <name> : <expr> in {e1, e2, …};` parses inside a `constraint`
/// block.
#[test]
fn cover_in_set_form_parses() {
    let program = parse_ok(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; }\n\
         constraint M { cover c1 : x in {0.0, 1.0, 2.0}; }",
    );
    let Item::ConstraintDecl(decl) = &program.items[1] else { panic!("expected a ConstraintDecl") };
    let ConstraintStmt::Cover { bins, .. } = &decl.body[0] else { panic!("expected a Cover stmt") };
    let CoverBins::Set(values) = bins else { panic!("expected CoverBins::Set") };
    assert_eq!(values.len(), 3, "the set form keeps every authored value");
}

/// `cover cross <name> : <point1>, <point2>, …;` parses inside a
/// `constraint` block.
#[test]
fn cover_cross_form_parses() {
    let program = parse_ok(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; param y : Real = 1.0; }\n\
         constraint M {\n\
             cover c1 : x bins [0.0:1.0:2.0];\n\
             cover c2 : y bins [0.0:1.0:2.0];\n\
             cover cross joint : c1, c2;\n\
         }",
    );
    let Item::ConstraintDecl(decl) = &program.items[1] else { panic!("expected a ConstraintDecl") };
    assert_eq!(decl.body.len(), 3);
    let ConstraintStmt::CoverCross { name, points, .. } = &decl.body[2] else {
        panic!("expected a CoverCross stmt")
    };
    assert_eq!(name, "joint");
    assert_eq!(points, &vec!["c1".to_string(), "c2".to_string()]);
}

/// `cover` is a reserved word (already reserved by dv-core's DVC-06 —
/// `Parser::RESERVED_WORDS` — this proves it is actually enforced, not
/// merely listed).
#[test]
fn cover_is_a_reserved_word() {
    let err = parse_str("mod cover ( inout p : Electrical ) {}")
        .expect_err("`cover` must not parse as an identifier");
    assert!(err.to_string().contains("reserved word"), "{err}");
}

/// A `cover` point's bin count folds correctly from its literal edges: 5
/// bins over `[0.0:0.5:2.0]` (0.0, 0.5, 1.0, 1.5, 2.0 — both ends
/// inclusive).
#[test]
fn a_cover_points_bin_count_folds_from_literal_edges() {
    let design = elaborate(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; }\n\
         constraint M { cover c1 : x bins [0.0:0.5:2.0]; }",
    );
    let block = &design.module("M").expect("M elaborates").constraints()[0];
    let piperine_lang::pom::StmtClassNode::Cover { bin_count, .. } = &block.classes()[0] else {
        panic!("expected a Cover class node")
    };
    assert_eq!(*bin_count, 5, "[0.0:0.5:2.0] is 5 bins, both ends inclusive");
}

/// A `cover cross`'s joint bin count is the product of its named points'
/// own counts — 3 x 4 = 12.
#[test]
fn cover_cross_joint_bin_count_is_the_product() {
    let design = elaborate(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; param y : Real = 1.0; }\n\
         constraint M {\n\
             cover c1 : x bins [0.0:1.0:2.0];\n\
             cover c2 : y in {0.0, 1.0, 2.0, 3.0};\n\
             cover cross joint : c1, c2;\n\
         }",
    );
    let block = &design.module("M").expect("M elaborates").constraints()[0];
    let piperine_lang::pom::StmtClassNode::CoverCross { joint_bin_count, .. } = &block.classes()[2] else {
        panic!("expected a CoverCross class node")
    };
    assert_eq!(*joint_bin_count, 3 * 4, "the joint count is the product of the two points' own counts");
}

/// D10: a `cover cross` whose joint bin count exceeds the elaboration cap
/// is refused loudly, naming the declared count and the cap — never
/// silently accepted or discovered only when something later allocates.
#[test]
fn an_oversized_cover_cross_is_refused_at_elaboration_naming_count_and_cap() {
    // Default cap is 4096; 100 x 100 = 10000 exceeds it.
    let err = elaborate_err(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; param y : Real = 1.0; }\n\
         constraint M {\n\
             cover c1 : x bins [0.0:1.0:99.0];\n\
             cover c2 : y bins [0.0:1.0:99.0];\n\
             cover cross joint : c1, c2;\n\
         }",
    );
    assert!(err.contains("10000"), "names the declared joint count: {err}");
    assert!(err.contains("4096"), "names the cap: {err}");
}

/// `cover cross` naming an undeclared point fails loud, naming the
/// missing point.
#[test]
fn cover_cross_naming_an_undeclared_point_is_loud() {
    let err = elaborate_err(
        "mod M ( inout p : Electrical ) { param x : Real = 1.0; }\n\
         constraint M {\n\
             cover c1 : x bins [0.0:1.0:2.0];\n\
             cover cross joint : c1, nonexistent;\n\
         }",
    );
    assert!(err.contains("nonexistent"), "{err}");
}
