//! LRM-COVERS: 5.6.7
//!
//! LRMW2-41 (`lrm-conformance-wave2`, T46): `branch_lvalue :
//! indirect_expression == analog_expression;` (§5.6.7, Annex A.6.10) —
//! grammar-level tests. `crates/piperine-codegen/tests/indirect_contribution.rs`
//! and root `tests/indirect_contribution_opamp_e2e.rs` cover lowering and
//! the AC15 simulate proof; this file covers the parser: the LRM's own
//! ideal-opamp form parses into the dedicated AST node, and a malformed
//! probe expression (§5.6.7: "The left-hand side of the equality operator
//! must either be an access function, or ddt, idt or idtmod applied to an
//! access function") is a typed `ParseErrorKind`, never `is_err()`.

use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::{Grammar, Lexer};

fn parse_module(src: &str) -> piperine_lang::ast::ModuleDecl {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error: {e}"));
    Grammar::new(toks).parse_module_decl().unwrap_or_else(|e| panic!("parse error for {src:?}: {e}"))
}

fn one_analog_stmt(src: &str) -> Stmt {
    let m = parse_module(src);
    match &m.items[0] {
        piperine_lang::ast::ModuleItem::Behavior(b) => (*b.body).clone(),
        other => panic!("expected a Behavior item, got {other:?}"),
    }
}

/// §5.6.7's own opamp example: `V(out):V(pin,nin) == 0;` parses into the
/// dedicated `IndirectContribution` node — `lhs` is the driven branch,
/// `probe` the tolerance-supplying access-function read, `rhs` the
/// equation's right-hand side.
#[test]
fn opamp_indirect_form_parses_into_the_dedicated_node() {
    let src = "module opamp (inout electrical out, pin, nin);
        analog V(out):V(pin,nin) == 0;
    endmodule";
    let stmt = one_analog_stmt(src);
    match stmt {
        Stmt::IndirectContribution { lhs, probe, rhs, .. } => {
            assert!(matches!(lhs, Expr::AccessFunction { ref function, .. } if function == "V"));
            assert!(matches!(probe, Expr::AccessFunction { ref function, .. } if function == "V"));
            assert!(matches!(rhs, Expr::Number { .. }));
        }
        other => panic!("expected Stmt::IndirectContribution, got {other:?}"),
    }
}

/// §5.6.7.1's own `ddt` form: `V(x):ddt(V(x)) == f(...)` — the probe is
/// `ddt` applied to an access function, also legal.
#[test]
fn ddt_wrapped_probe_parses() {
    let src = "module intg (inout electrical x);
        analog V(x):ddt(V(x)) == 1.0;
    endmodule";
    let stmt = one_analog_stmt(src);
    match stmt {
        Stmt::IndirectContribution { probe, .. } => {
            assert!(matches!(probe, Expr::Call { ref name, .. } if name == "ddt"));
        }
        other => panic!("expected Stmt::IndirectContribution, got {other:?}"),
    }
}

/// §5.6.7: a probe that is not an access function (or `ddt`/`idt`/
/// `idtmod` applied to one) — here a bare literal — is a malformed
/// indirect form, a typed `ParseErrorKind`, never `is_err()`.
#[test]
fn non_access_function_probe_is_a_typed_parse_error() {
    let full = "module dut (inout electrical out); analog V(out): 5 == 0; endmodule";
    let toks = Lexer::new(full).tokenize().unwrap_or_else(|e| panic!("lex error: {e}"));
    let mut g = Grammar::new(toks);
    let err = g.parse_module_decl().unwrap_err();
    match err.kind {
        piperine_lang::ParseErrorKind::UnexpectedToken { ref expected, .. } => {
            assert!(
                expected.contains("indirect contribution"),
                "expected the indirect-contribution-probe-shape message, got: {expected}"
            );
        }
        other => panic!("expected UnexpectedToken, got {other:?}"),
    }
}
