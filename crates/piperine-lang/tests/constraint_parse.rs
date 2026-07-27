//! dv-core T6 (DVC-04) — the `constraint Name { ... }` block as a top-level
//! item, sibling of `analog`/`digital`: `require <name> : <pred>;`,
//! `var <name> : <Type> = <expr>;` (type annotation mandatory), and
//! `target <expr> <cmp> <level> [tol <scale>];` all parse; each malformed
//! form fails loud at the offending token; a canonical block round-trips
//! through `piperine fmt` unchanged.

use piperine_lang::parse::ast::{ConstraintStmt, Item, TargetCmp};
use piperine_lang::parse::format::{FormatOptions, TokenFormatter};
use piperine_lang::parse::lexer::Lexer;
use piperine_lang::parse_str;

fn constraint_of(src: &str) -> piperine_lang::parse::ast::ConstraintDecl {
    let ast = parse_str(src).expect("fixture must parse");
    for item in ast.items {
        if let Item::ConstraintDecl(c) = item {
            return c;
        }
    }
    panic!("no constraint block found in fixture");
}

fn err_offset(src: &str) -> (String, usize) {
    let err = parse_str(src).expect_err("malformed fixture must not parse");
    let at = err.byte_offset().expect("parse error must carry a span");
    (err.to_string(), at)
}

/// DVC-04: the three statement forms parse — a named `require`, a typed
/// `var`, and a `target` with and without the `tol` scale clause.
#[test]
fn constraint_block_parses_all_statement_forms() {
    let c = constraint_of(
        "mod M ( inout p : Electrical ) { param vds_max : Real = 3.3; }\n\
         constraint M {\n\
             require vds : V(p, p) <= vds_max;\n\
             var gain_db : Real = 20.0;\n\
             target V(p, p) <= 3.3;\n\
             target V(p, p) >= 0.0 tol 0.1;\n\
         }",
    );
    assert_eq!(c.name, "M");
    assert_eq!(c.body.len(), 4);

    let ConstraintStmt::Require { name, .. } = &c.body[0] else {
        panic!("first statement must be a require: {:?}", c.body[0]);
    };
    assert_eq!(name, "vds");

    let ConstraintStmt::Var { name, ty, .. } = &c.body[1] else {
        panic!("second statement must be a var: {:?}", c.body[1]);
    };
    assert_eq!(name, "gain_db");
    assert_eq!(ty.name, "Real");

    let ConstraintStmt::Target { cmp, tol, .. } = &c.body[2] else {
        panic!("third statement must be a target: {:?}", c.body[2]);
    };
    assert_eq!(*cmp, TargetCmp::Le);
    assert!(tol.is_none(), "a target without `tol` carries no scale");

    let ConstraintStmt::Target { cmp, tol, .. } = &c.body[3] else {
        panic!("fourth statement must be a target: {:?}", c.body[3]);
    };
    assert_eq!(*cmp, TargetCmp::Ge);
    assert!(tol.is_some(), "`tol <scale>` must be captured");
}

/// DVC-04: the block is a sibling top-level item of `analog`/`digital` —
/// all three attach to the same module by name.
#[test]
fn constraint_block_is_a_sibling_of_analog_and_digital() {
    let src = "mod M ( inout p : Electrical ) { }\n\
               analog M { V(p, p) <+ 0.0; }\n\
               constraint M { require ok : V(p, p) >= 0.0; }\n";
    let ast = parse_str(src).expect("fixture must parse");
    let kinds: Vec<&str> = ast
        .items
        .iter()
        .map(|i| match i {
            Item::ModuleDeclaration(_) => "mod",
            Item::BehaviorDecl(_) => "behavior",
            Item::ConstraintDecl(_) => "constraint",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["mod", "behavior", "constraint"]);
}

/// DVC-04: `var` requires its type annotation, exactly as every other
/// `var` in the language — the bare `var x = 1.0;` form fails loud at the
/// `=` token.
#[test]
fn var_without_type_annotation_fails_loud() {
    let src = "constraint M { var x = 1.0; }";
    let (_, at) = err_offset(src);
    assert_eq!(&src[at..at + 1], "=", "error must point at the offending token");
}

/// DVC-04: each malformed statement form fails loud at the offending token.
#[test]
fn malformed_constraint_statements_fail_loud_at_the_offending_token() {
    // `require` without a name — the `:` is the offending token.
    let src = "constraint M { require : V(p, p) >= 0.0; }";
    let (_, at) = err_offset(src);
    assert_eq!(&src[at..at + 1], ":", "error must point at the offending token");

    // `require` with an empty predicate — the `;` is the offending token.
    let src = "constraint M { require vds : ; }";
    let (_, at) = err_offset(src);
    assert_eq!(&src[at..at + 1], ";", "error must point at the offending token");

    // `var` without a value — the `;` is the offending token.
    let src = "constraint M { var x : Real = ; }";
    let (_, at) = err_offset(src);
    assert_eq!(&src[at..at + 1], ";", "error must point at the offending token");

    // A strict `<` comparison — no signed-margin semantics (DVC-08); the
    // `<` is the offending token.
    let src = "constraint M { target V(p, p) < 1.0; }";
    let (msg, at) = err_offset(src);
    assert_eq!(&src[at..at + 1], "<", "error must point at the offending token");
    assert!(msg.contains("<="), "error must name the legal comparisons: {msg}");

    // `tol` without a scale — the `;` is the offending token.
    let src = "constraint M { target V(p, p) <= 1.0 tol; }";
    let (_, at) = err_offset(src);
    assert_eq!(&src[at..at + 1], ";", "error must point at the offending token");

    // An unrecognized statement keyword — `check` is the offending token.
    let src = "constraint M { check x : V(p, p) >= 0.0; }";
    let (msg, _) = err_offset(src);
    assert!(msg.contains("check"), "error must name the offending token: {msg}");

    // An unclosed block fails loud.
    let src = "constraint M { require ok : V(p, p) >= 0.0;";
    parse_str(src).expect_err("an unclosed constraint block must not parse");
}

/// DVC-04: a canonically formatted constraint block round-trips through
/// `piperine fmt` unchanged.
#[test]
fn constraint_block_round_trips_through_fmt() {
    let src = "mod M(inout p : Electrical) {\n\
               \x20   param vds_max : Real = 3.3;\n\
               }\n\
               constraint M {\n\
               \x20   require vds : V(p, p) <= vds_max;\n\
               \x20   var gain_db : Real = 20.0;\n\
               \x20   target V(p, p) >= 0.0 tol 0.1;\n\
               }\n";
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize_all().expect("lexer must succeed");
    let out = TokenFormatter::format_source(src, &tokens, FormatOptions::default());
    assert_eq!(out, src, "fmt must not move a constraint block");
}
