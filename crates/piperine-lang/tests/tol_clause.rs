//! dv-core T3 (DVC-01) — the `tol <dist>(<keyed args>) [global]` clause on
//! `param`: parses with and without `global`, is optional everywhere a
//! `param` appears, fails loud at the offending token when malformed, and
//! round-trips through `piperine fmt` unchanged.

use piperine_lang::parse::ast::{Item, ModuleStatement};
use piperine_lang::parse::format::{FormatOptions, TokenFormatter};
use piperine_lang::parse::lexer::Lexer;
use piperine_lang::parse_str;

fn param_of(src: &str, module: &str, param: &str) -> ModuleStatement {
    let ast = parse_str(src).expect("fixture must parse");
    for item in ast.items {
        if let Item::ModuleDeclaration(m) = item
            && m.name == module
        {
            for stmt in m.body {
                if let ModuleStatement::ParamDecl { name, .. } = &stmt
                    && name == param
                {
                    return stmt;
                }
            }
            panic!("param `{param}` not found in module `{module}`");
        }
    }
    panic!("module `{module}` not found");
}

/// DVC-01: `param vto : Real = 0.7 tol gauss(sigma = 0.005);` parses — the
/// clause carries the distribution name, its keyed args, and the mismatch
/// (no `global`) marker.
#[test]
fn tol_clause_parses_without_global() {
    let stmt = param_of(
        "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol gauss(sigma = 0.005); }",
        "M",
        "vto",
    );
    let ModuleStatement::ParamDecl { tol: Some(tol), .. } = stmt else {
        panic!("tol clause must be present: {stmt:?}");
    };
    assert_eq!(tol.dist, "gauss");
    assert!(!tol.global, "no `global` keyword means mismatch variation");
    assert_eq!(tol.args.len(), 1);
    assert_eq!(tol.args[0].name, "sigma");
}

/// DVC-01: the same clause with `global` parses, marked as process variation.
#[test]
fn tol_clause_parses_with_global() {
    let stmt = param_of(
        "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol gauss(sigma = 0.005) global; }",
        "M",
        "vto",
    );
    let ModuleStatement::ParamDecl { tol: Some(tol), .. } = stmt else {
        panic!("tol clause must be present: {stmt:?}");
    };
    assert_eq!(tol.dist, "gauss");
    assert!(tol.global, "`global` marks process variation");
}

/// DVC-01: the clause is optional — a plain `param` parses with `tol: None`,
/// with and without a default.
#[test]
fn tol_clause_is_optional() {
    let with_default = param_of(
        "mod M ( inout p : Electrical ) { param vto : Real = 0.7; }",
        "M",
        "vto",
    );
    assert!(
        matches!(with_default, ModuleStatement::ParamDecl { tol: None, .. }),
        "plain param must have no tol clause: {with_default:?}"
    );
    let no_default = param_of(
        "mod M ( inout p : Electrical ) { param vto : Real; }",
        "M",
        "vto",
    );
    assert!(
        matches!(no_default, ModuleStatement::ParamDecl { tol: None, .. }),
        "plain param must have no tol clause: {no_default:?}"
    );
}

/// DVC-01: a malformed clause fails loud, pointing at the offending token.
#[test]
fn malformed_tol_clause_fails_loud_at_the_offending_token() {
    // Missing distribution name — the `(` is the offending token.
    let src = "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol (sigma = 0.005); }";
    let err = parse_str(src).expect_err("a tol clause without a distribution name must not parse");
    let at = err.byte_offset().expect("parse error must carry a span");
    assert_eq!(&src[at..at + 1], "(", "error must point at the offending token: {err}");

    // Missing `(` after the distribution name — `sigma` is the offending token.
    let src = "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol gauss sigma = 0.005); }";
    let err = parse_str(src).expect_err("a tol clause without parentheses must not parse");
    let at = err.byte_offset().expect("parse error must carry a span");
    assert_eq!(&src[at..at + 5], "sigma", "error must point at the offending token: {err}");

    // Unclosed argument list — the `;` is the offending token.
    let src = "mod M ( inout p : Electrical ) { param vto : Real = 0.7 tol gauss(sigma = 0.005; }";
    let err = parse_str(src).expect_err("an unclosed tol argument list must not parse");
    let at = err.byte_offset().expect("parse error must carry a span");
    assert_eq!(&src[at..at + 1], ";", "error must point at the offending token: {err}");
}

/// DVC-01: a canonically formatted `tol` clause round-trips through
/// `piperine fmt` unchanged.
#[test]
fn tol_clause_round_trips_through_fmt() {
    let src = "mod M(inout p : Electrical) {\n    param vto : Real = 0.7 tol gauss(sigma = 0.005);\n    param w : Real = 1u tol gauss(sigma = 0.005) global;\n}\n";
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize_all().expect("lexer must succeed");
    let out = TokenFormatter::format_source(src, &tokens, FormatOptions::default());
    assert_eq!(out, src, "fmt must not move a tol clause");
}
