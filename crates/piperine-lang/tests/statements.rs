//! LRM-COVERS: 5.6.1, 5.7, 5.3, 5.3.1, 5.3.2, 5.8, 5.8.1, 5.8.2, 5.8.3, 5.9.1, 5.9.2, 5.10, 5.10.1
//!
//! Proves the statement grammar (tasks.md T8) against LRM chapter 5
//! (analog) and the inherited 1364 procedural statements: contribution,
//! blocking/non-blocking assignment, control flow, event control, and
//! task calls.
//!
//! T69 (`lrm-conformance-wave2`) re-reading, no new tests: `contribution_
//! statement_parses` is §5.6.1's direct branch contribution form;
//! `blocking_assignment_parses` is §5.7's scalar analog procedural
//! assignment; `named_block_with_block_item_decls_parses`/`unnamed_block_
//! with_no_decls_parses` are §5.3.1's sequential-block syntax, and the
//! named form's local parameter/variable declarations are §5.3.2's own
//! "naming a block allows local variables to be declared for that block";
//! `if_else_parses_both_branches`/`if_without_else_leaves_it_none` are
//! §5.8.1's if-else-if form, and the nested-if `else`-association rule
//! §5.8.2 walks through in prose has no independent grammar of its own —
//! the same production this file already exercises; `case_casex_casez_
//! and_analog_case_all_parse_with_default_arm`/`case_arm_supports_
//! multiple_comma_separated_labels` are §5.8.3's case statement (the
//! `analog_case` arm specifically); `while_repeat_forever_all_parse` is
//! §5.9.1's repeat/while forms; `for_loop_parses_init_cond_step_body` is
//! §5.9.2's for-statement; `event_control_with_or_joined_expressions_
//! parses_all` is §5.10.1's event OR operator (`or`-joined event
//! expressions triggering on any one event).
//! LRM-COVERS: A.6.9, A.8.5

use piperine_lang::ast::{CaseKind, EventExpr, Expr, Stmt};
use piperine_lang::{Grammar, Lexer};

fn parse(src: &str) -> Stmt {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error for {src:?}: {e}"));
    let mut g = Grammar::new(toks);
    g.parse_stmt().unwrap_or_else(|e| panic!("parse error for {src:?}: {e}"))
}

// ── contribution / assignment, §5.4/§5.5 ─────────────────────────────────

#[test]
fn contribution_statement_parses() {
    match parse("V(p,n) <+ r * I(p,n);") {
        Stmt::Contribution { target, value, .. } => {
            assert!(matches!(target, Expr::AccessFunction { .. }));
            assert!(matches!(value, Expr::Binary { .. }));
        }
        other => panic!("expected Stmt::Contribution, got {other:?}"),
    }
}

#[test]
fn blocking_assignment_parses() {
    match parse("x = a + 1;") {
        Stmt::Assign { target, value, .. } => {
            assert!(matches!(target, Expr::Ident { .. }));
            assert!(matches!(value, Expr::Binary { .. }));
        }
        other => panic!("expected Stmt::Assign, got {other:?}"),
    }
}

#[test]
fn non_blocking_assignment_parses() {
    match parse("q <= d;") {
        Stmt::NonBlockingAssign { target, value, .. } => {
            assert!(matches!(target, Expr::Ident { .. }));
            assert!(matches!(value, Expr::Ident { .. }));
        }
        other => panic!("expected Stmt::NonBlockingAssign, got {other:?}"),
    }
}

// ── control flow, Annex A.6.7.1 ─────────────────────────────────────────

#[test]
fn if_else_parses_both_branches() {
    match parse("if (a) x = 1; else x = 2;") {
        Stmt::If { cond, then_branch, else_branch, .. } => {
            assert!(matches!(cond, Expr::Ident { .. }));
            assert!(matches!(*then_branch, Stmt::Assign { .. }));
            assert!(matches!(else_branch.as_deref(), Some(Stmt::Assign { .. })));
        }
        other => panic!("expected Stmt::If, got {other:?}"),
    }
}

#[test]
fn if_without_else_leaves_it_none() {
    match parse("if (a) x = 1;") {
        Stmt::If { else_branch, .. } => assert!(else_branch.is_none()),
        other => panic!("expected Stmt::If, got {other:?}"),
    }
}

#[test]
fn case_casex_casez_and_analog_case_all_parse_with_default_arm() {
    for (src, expected_kind) in [
        ("case (s) 0: x=1; default: x=2; endcase", CaseKind::Case),
        ("casex (s) 0: x=1; default: x=2; endcase", CaseKind::Casex),
        ("casez (s) 0: x=1; default: x=2; endcase", CaseKind::Casez),
        ("analog_case (s) 0: x=1; default: x=2; endcase", CaseKind::AnalogCase),
    ] {
        match parse(src) {
            Stmt::Case { kind, selector, arms, .. } => {
                assert_eq!(kind, expected_kind, "kind mismatch for {src:?}");
                assert!(matches!(selector, Expr::Ident { .. }));
                assert_eq!(arms.len(), 2, "arm count mismatch for {src:?}");
                assert!(arms[0].labels.is_some());
                assert!(arms[1].labels.is_none(), "second arm should be the default");
            }
            other => panic!("expected Stmt::Case for {src:?}, got {other:?}"),
        }
    }
}

#[test]
fn case_arm_supports_multiple_comma_separated_labels() {
    match parse("case (s) 0, 1, 2: x=1; endcase") {
        Stmt::Case { arms, .. } => {
            assert_eq!(arms.len(), 1);
            assert_eq!(arms[0].labels.as_ref().unwrap().len(), 3);
        }
        other => panic!("expected Stmt::Case, got {other:?}"),
    }
}

#[test]
fn for_loop_parses_init_cond_step_body() {
    match parse("for (i = 0; i < 4; i = i + 1) x = i;") {
        Stmt::For { init, cond, step, body, .. } => {
            assert!(matches!(*init, Stmt::Assign { .. }));
            assert!(matches!(cond, Expr::Binary { .. }));
            assert!(matches!(*step, Stmt::Assign { .. }));
            assert!(matches!(*body, Stmt::Assign { .. }));
        }
        other => panic!("expected Stmt::For, got {other:?}"),
    }
}

#[test]
fn while_repeat_forever_all_parse() {
    match parse("while (a) x = 1;") {
        Stmt::While { .. } => {}
        other => panic!("expected Stmt::While, got {other:?}"),
    }
    match parse("repeat (3) x = 1;") {
        Stmt::Repeat { count, .. } => assert!(matches!(count, Expr::Number { .. })),
        other => panic!("expected Stmt::Repeat, got {other:?}"),
    }
    match parse("forever x = 1;") {
        Stmt::Forever { .. } => {}
        other => panic!("expected Stmt::Forever, got {other:?}"),
    }
}

// ── begin/end named blocks with block item declarations ────────────────

#[test]
fn named_block_with_block_item_decls_parses() {
    match parse("begin : blk integer i; real r; x = 1; y = 2; end") {
        Stmt::Block { name, decls, stmts, .. } => {
            assert_eq!(name.as_deref(), Some("blk"));
            assert_eq!(decls.len(), 2);
            assert_eq!(stmts.len(), 2);
        }
        other => panic!("expected Stmt::Block, got {other:?}"),
    }
}

#[test]
fn unnamed_block_with_no_decls_parses() {
    match parse("begin x = 1; end") {
        Stmt::Block { name, decls, stmts, .. } => {
            assert!(name.is_none());
            assert!(decls.is_empty());
            assert_eq!(stmts.len(), 1);
        }
        other => panic!("expected Stmt::Block, got {other:?}"),
    }
}

// ── event control, §5.6/Annex A.6.5 ──────────────────────────────────────

#[test]
fn posedge_and_negedge_event_control_parse() {
    match parse("@(posedge clk) q <= d;") {
        Stmt::EventControl { events, body, .. } => {
            assert_eq!(events.len(), 1);
            assert!(matches!(events[0], EventExpr::Posedge(_)));
            assert!(matches!(*body, Stmt::NonBlockingAssign { .. }));
        }
        other => panic!("expected Stmt::EventControl, got {other:?}"),
    }
    match parse("@(negedge clk) q <= d;") {
        Stmt::EventControl { events, .. } => assert!(matches!(events[0], EventExpr::Negedge(_))),
        other => panic!("expected Stmt::EventControl, got {other:?}"),
    }
}

#[test]
fn event_control_with_or_joined_expressions_parses_all() {
    match parse("@(posedge clk or negedge rst) q <= d;") {
        Stmt::EventControl { events, .. } => assert_eq!(events.len(), 2),
        other => panic!("expected Stmt::EventControl, got {other:?}"),
    }
}

#[test]
fn implicit_event_star_forms_both_parse() {
    match parse("@* q = d;") {
        Stmt::EventControl { events, .. } => assert!(matches!(events[0], EventExpr::ImplicitAll)),
        other => panic!("expected Stmt::EventControl, got {other:?}"),
    }
    match parse("@(*) q = d;") {
        Stmt::EventControl { events, .. } => assert!(matches!(events[0], EventExpr::ImplicitAll)),
        other => panic!("expected Stmt::EventControl, got {other:?}"),
    }
}

#[test]
fn analog_event_functions_parse_as_event_control_level_expressions() {
    // `cross`/`above`/`timer`/`initial_step`/`final_step` are ordinary
    // calls at the expression grammar level (T7); the event-control
    // statement wraps a bare expression as EventExpr::Level.
    match parse("@(cross(V(p,n), 0)) x = 1;") {
        Stmt::EventControl { events, .. } => {
            assert!(matches!(&events[0], EventExpr::Level(Expr::Call { name, .. }) if name == "cross"));
        }
        other => panic!("expected Stmt::EventControl, got {other:?}"),
    }
}

// ── disable, Annex A.6.4 ─────────────────────────────────────────────────

#[test]
fn disable_parses_the_named_block() {
    match parse("disable blk;") {
        Stmt::Disable { name, .. } => assert_eq!(name, "blk"),
        other => panic!("expected Stmt::Disable, got {other:?}"),
    }
}

// ── task / system-task calls ─────────────────────────────────────────────

#[test]
fn system_task_call_with_args_parses_as_statement() {
    match parse("$display(\"x=%d\", x);") {
        Stmt::TaskCall { name, args, .. } => {
            assert_eq!(name, "display");
            assert_eq!(args.len(), 2);
        }
        other => panic!("expected Stmt::TaskCall, got {other:?}"),
    }
}

#[test]
fn bare_user_task_call_parses_as_statement() {
    match parse("my_task(a, b);") {
        Stmt::TaskCall { name, args, .. } => {
            assert_eq!(name, "my_task");
            assert_eq!(args.len(), 2);
        }
        other => panic!("expected Stmt::TaskCall, got {other:?}"),
    }
}

// ── null statement + errors ──────────────────────────────────────────────

#[test]
fn null_statement_parses() {
    assert!(matches!(parse(";"), Stmt::Null { .. }));
}

#[test]
fn malformed_statement_is_a_typed_parse_error() {
    let toks = Lexer::new("x ??? y;").tokenize().unwrap_or_else(|e| panic!("{e}"));
    let mut g = Grammar::new(toks);
    let err = g.parse_stmt().unwrap_err();
    assert!(
        matches!(err.kind, piperine_lang::ParseErrorKind::UnexpectedToken { .. }),
        "expected UnexpectedToken, got {:?}",
        err.kind
    );
}
