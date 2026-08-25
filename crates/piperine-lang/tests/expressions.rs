//! LRM-COVERS: 2.5, 4, 4.1, 4.2, 4.2.2, 4.2.4, 4.2.5, 4.2.7, 4.2.8, 4.2.9, 4.2.10, 4.2.11, 4.2.12, 4.3.1, 4.3.2, 4.5
//! Proves the expression grammar (tasks.md T7) against LRM chapter 4: every
//! operator parses with the right precedence/associativity (asserted on
//! parse **shape**, not just success), ternary, concat/replication,
//! part-select/index, math and time-domain functions, and both access
//! function arities.
//!
//! Wave-2 (T69) re-reading, no new tests: §2.5 "Operators" ("Operators are
//! single, double, or triple character sequences ... used in expressions")
//! and §4.2 "Operators" are the containing statements `every_binary_
//! operator_parses_to_its_binop`/`every_unary_reduction_operator_parses`
//! prove directly (single `+`, double `**`/`==`/`&&`, triple `===`/`!==`
//! forms, all recognized in expression position); §4.2.2 "Operator
//! precedence" is the precedence/associativity block (`mul_binds_
//! tighter_than_add` through `parens_override_precedence`); §4.2.4
//! "Arithmetic operators" (`+ - * / % **`), §4.2.5 "Relational operators"
//! (`< <= > >=`), §4.2.7 "Logical equality operators" (`== !=`), §4.2.8
//! "Logical operators" (`&& || !`), §4.2.9 "Bitwise operators" (`& | ^
//! ~^ ~`), §4.2.11 "Shift operators" (`<< >>`) are each a subset of the
//! same table-driven `every_binary_operator_parses_to_its_binop`/`every_
//! unary_reduction_operator_parses` cases; §4.2.10 "Reduction operators"
//! (`& ~& | ~| ^`) is the reduction cases in `every_unary_reduction_
//! operator_parses`; §4.2.12 "Conditional operator" is the `ternary_*`
//! tests; §4.3.1/§4.3.2 "Standard mathematical functions"/"Transcendental
//! functions" are `math_functions_parse_as_calls` (`sqrt`/`exp`/`ln`/
//! `min`/`max`/`pow`/`sin`/`atan2`); §4.5 "Analog operators" is `time_
//! domain_functions_parse_as_calls` (`ddt`/`idt`/`idtmod`/`absdelay`/
//! `transition`/`slew`/`laplace_nd`/`zi_np`, each recognized as an analog
//! operator call). §4 "Expressions" and §4.1 "Overview" are the
//! containing chapter/overview headings for every subsection above — §4.1
//! itself is purely narrative ("this section describes the operators and
//! operands ... how to use them to form expressions"), with no
//! independent rule of its own beyond what §4.2 onward states.
//!
//! LRM-COVERS: A.8.2, A.8.4, A.8.6, A.8.7, A.8.8, A.8.9

use piperine_lang::ast::{BinOp, Expr, UnOp};
use piperine_lang::{Grammar, Lexer};

fn parse(src: &str) -> Expr {
    let toks = Lexer::new(src).tokenize().unwrap_or_else(|e| panic!("lex error for {src:?}: {e}"));
    let mut g = Grammar::new(toks);
    g.parse_expr().unwrap_or_else(|e| panic!("parse error for {src:?}: {e}"))
}

fn binop(e: &Expr) -> BinOp {
    match e {
        Expr::Binary { op, .. } => *op,
        other => panic!("expected Expr::Binary, got {other:?}"),
    }
}

// ── §4.1.3 Table 4-1 — every binary operator recognized ────────────────

#[test]
fn every_binary_operator_parses_to_its_binop() {
    let cases: &[(&str, BinOp)] = &[
        ("a ** b", BinOp::Pow),
        ("a * b", BinOp::Mul),
        ("a / b", BinOp::Div),
        ("a % b", BinOp::Mod),
        ("a + b", BinOp::Add),
        ("a - b", BinOp::Sub),
        ("a << b", BinOp::Shl),
        ("a >> b", BinOp::Shr),
        ("a < b", BinOp::Lt),
        ("a <= b", BinOp::Le),
        ("a > b", BinOp::Gt),
        ("a >= b", BinOp::Ge),
        ("a == b", BinOp::Eq),
        ("a != b", BinOp::Ne),
        ("a === b", BinOp::CaseEq),
        ("a !== b", BinOp::CaseNe),
        ("a & b", BinOp::BitAnd),
        ("a ^ b", BinOp::BitXor),
        ("a ~^ b", BinOp::BitXnor),
        ("a | b", BinOp::BitOr),
        ("a && b", BinOp::LogAnd),
        ("a || b", BinOp::LogOr),
    ];
    for (src, expected) in cases {
        let e = parse(src);
        assert_eq!(binop(&e), *expected, "operator mismatch for {src:?}");
    }
}

// ── precedence, asserted on shape ───────────────────────────────────────

#[test]
fn mul_binds_tighter_than_add() {
    // `a + b * c` must parse as `a + (b * c)`, i.e. the top node is Add
    // whose rhs is a Mul — not `(a + b) * c`.
    let e = parse("a + b * c");
    match e {
        Expr::Binary { op: BinOp::Add, rhs, .. } => {
            assert_eq!(binop(&rhs), BinOp::Mul);
        }
        other => panic!("expected top-level Add with Mul rhs, got {other:?}"),
    }
}

#[test]
fn pow_is_right_associative() {
    // `a ** b ** c` must parse as `a ** (b ** c)`.
    let e = parse("a ** b ** c");
    match e {
        Expr::Binary { op: BinOp::Pow, lhs, rhs, .. } => {
            assert!(matches!(*lhs, Expr::Ident { .. }));
            assert_eq!(binop(&rhs), BinOp::Pow);
        }
        other => panic!("expected right-assoc Pow tree, got {other:?}"),
    }
}

#[test]
fn add_is_left_associative() {
    // `a - b - c` must parse as `(a - b) - c`.
    let e = parse("a - b - c");
    match e {
        Expr::Binary { op: BinOp::Sub, lhs, rhs, .. } => {
            assert_eq!(binop(&lhs), BinOp::Sub);
            assert!(matches!(*rhs, Expr::Ident { .. }));
        }
        other => panic!("expected left-assoc Sub tree, got {other:?}"),
    }
}

#[test]
fn logical_or_binds_loosest_of_the_chain() {
    // `a && b || c && d` must parse as `(a && b) || (c && d)`.
    let e = parse("a && b || c && d");
    match e {
        Expr::Binary { op: BinOp::LogOr, lhs, rhs, .. } => {
            assert_eq!(binop(&lhs), BinOp::LogAnd);
            assert_eq!(binop(&rhs), BinOp::LogAnd);
        }
        other => panic!("expected top-level LogOr over two LogAnd arms, got {other:?}"),
    }
}

#[test]
fn parens_override_precedence() {
    let e = parse("(a + b) * c");
    match e {
        Expr::Binary { op: BinOp::Mul, lhs, .. } => {
            assert_eq!(binop(&lhs), BinOp::Add);
        }
        other => panic!("expected top-level Mul with Add lhs, got {other:?}"),
    }
}

// ── unary operators ──────────────────────────────────────────────────

#[test]
fn every_unary_reduction_operator_parses() {
    let cases: &[(&str, UnOp)] = &[
        ("&a", UnOp::AndReduce),
        ("~&a", UnOp::NandReduce),
        ("|a", UnOp::OrReduce),
        ("~|a", UnOp::NorReduce),
        ("^a", UnOp::XorReduce),
        ("!a", UnOp::LogNot),
        ("~a", UnOp::BitNot),
        ("-a", UnOp::Neg),
        ("+a", UnOp::Plus),
    ];
    for (src, expected) in cases {
        match parse(src) {
            Expr::Unary { op, .. } => assert_eq!(op, *expected, "unary op mismatch for {src:?}"),
            other => panic!("expected Expr::Unary for {src:?}, got {other:?}"),
        }
    }
}

#[test]
fn unary_binds_tighter_than_binary() {
    // `-a * b` must parse as `(-a) * b`, not `-(a * b)`.
    let e = parse("-a * b");
    match e {
        Expr::Binary { op: BinOp::Mul, lhs, .. } => {
            assert!(matches!(*lhs, Expr::Unary { op: UnOp::Neg, .. }));
        }
        other => panic!("expected Mul with Neg lhs, got {other:?}"),
    }
}

// ── ternary, §4.1.7 ─────────────────────────────────────────────────────

#[test]
fn ternary_parses_condition_then_else() {
    let e = parse("sel ? a : b");
    match e {
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            assert!(matches!(*cond, Expr::Ident { .. }));
            assert!(matches!(*then_branch, Expr::Ident { .. }));
            assert!(matches!(*else_branch, Expr::Ident { .. }));
        }
        other => panic!("expected Expr::Ternary, got {other:?}"),
    }
}

#[test]
fn ternary_is_lower_precedence_than_logical_or() {
    // `a || b ? c : d` must parse as `(a || b) ? c : d`.
    let e = parse("a || b ? c : d");
    match e {
        Expr::Ternary { cond, .. } => assert_eq!(binop(&cond), BinOp::LogOr),
        other => panic!("expected Expr::Ternary with LogOr cond, got {other:?}"),
    }
}

// ── concatenation / replication, §4.6.1 ──────────────────────────────────

#[test]
fn concatenation_parses_all_parts() {
    let e = parse("{a, b, c}");
    match e {
        Expr::Concat { parts, .. } => assert_eq!(parts.len(), 3),
        other => panic!("expected Expr::Concat, got {other:?}"),
    }
}

#[test]
fn replication_parses_count_and_parts() {
    let e = parse("{3{a, b}}");
    match e {
        Expr::Replication { count, parts, .. } => {
            assert!(matches!(*count, Expr::Number { .. }));
            assert_eq!(parts.len(), 2);
        }
        other => panic!("expected Expr::Replication, got {other:?}"),
    }
}

// ── index / part-select ──────────────────────────────────────────────────

#[test]
fn array_index_parses() {
    let e = parse("bus[2]");
    match e {
        Expr::Index { base, index, .. } => {
            assert!(matches!(*base, Expr::Ident { .. }));
            assert!(matches!(*index, Expr::Number { .. }));
        }
        other => panic!("expected Expr::Index, got {other:?}"),
    }
}

#[test]
fn part_select_parses_msb_and_lsb() {
    let e = parse("bus[7:0]");
    match e {
        Expr::PartSelect { msb, lsb, .. } => {
            assert!(matches!(*msb, Expr::Number { .. }));
            assert!(matches!(*lsb, Expr::Number { .. }));
        }
        other => panic!("expected Expr::PartSelect, got {other:?}"),
    }
}

// ── §4.3 math functions / §4.5 time-domain functions ─────────────────────

#[test]
fn math_functions_parse_as_calls() {
    for name in ["sqrt", "exp", "ln", "min", "max", "pow", "sin", "atan2"] {
        let src = format!("{name}(a, b)");
        match parse(&src) {
            Expr::Call { name: got, .. } => assert_eq!(got, name),
            other => panic!("expected Expr::Call for {src:?}, got {other:?}"),
        }
    }
}

#[test]
fn time_domain_functions_parse_as_calls() {
    for name in ["ddt", "idt", "idtmod", "absdelay", "transition", "slew", "laplace_nd", "zi_np"] {
        let src = format!("{name}(x)");
        match parse(&src) {
            Expr::Call { name: got, args, .. } => {
                assert_eq!(got, name);
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected Expr::Call for {src:?}, got {other:?}"),
        }
    }
}

#[test]
fn call_with_no_arguments_parses() {
    match parse("noargs()") {
        Expr::Call { name, args, .. } => {
            assert_eq!(name, "noargs");
            assert!(args.is_empty());
        }
        other => panic!("expected empty-arg Expr::Call, got {other:?}"),
    }
}

// ── access functions, §5.3/§5.4 ──────────────────────────────────────────

#[test]
fn access_function_one_arg_form() {
    match parse("V(br)") {
        Expr::AccessFunction { function, pos, neg, .. } => {
            assert_eq!(function, "V");
            assert!(matches!(*pos, Expr::Ident { .. }));
            assert!(neg.is_none());
        }
        other => panic!("expected one-arg AccessFunction, got {other:?}"),
    }
}

#[test]
fn access_function_two_arg_form() {
    match parse("I(p, n)") {
        Expr::AccessFunction { function, pos, neg, .. } => {
            assert_eq!(function, "I");
            assert!(matches!(*pos, Expr::Ident { .. }));
            assert!(matches!(neg.as_deref(), Some(Expr::Ident { .. })));
        }
        other => panic!("expected two-arg AccessFunction, got {other:?}"),
    }
}

// ── LRMC-C1-08: access-function grammar generalization (§4.4) ────────────

#[test]
fn generic_potential_access_function_parses_like_v() {
    // §4.4 Table 4-16: `potential(b1)` — "Alternative access of the
    // voltage across the branch b1", same one-arg shape as `V(b1)`.
    match parse("potential(b1)") {
        Expr::AccessFunction { function, pos, neg, .. } => {
            assert_eq!(function, "potential");
            assert!(matches!(*pos, Expr::Ident { .. }));
            assert!(neg.is_none());
        }
        other => panic!("expected one-arg AccessFunction, got {other:?}"),
    }
}

#[test]
fn generic_flow_access_function_parses_like_i() {
    // §4.4 Table 4-16: `flow(n1)` — "Alternative access of the current
    // flowing in the unnamed branch from n1 to ground", same shape as
    // `I(n1)`.
    match parse("flow(n1)") {
        Expr::AccessFunction { function, pos, neg, .. } => {
            assert_eq!(function, "flow");
            assert!(matches!(*pos, Expr::Ident { .. }));
            assert!(neg.is_none());
        }
        other => panic!("expected one-arg AccessFunction, got {other:?}"),
    }
}

#[test]
fn flow_two_arg_form_parses_like_i() {
    match parse("flow(p, n)") {
        Expr::AccessFunction { function, pos, neg, .. } => {
            assert_eq!(function, "flow");
            assert!(matches!(*pos, Expr::Ident { .. }));
            assert!(matches!(neg.as_deref(), Some(Expr::Ident { .. })));
        }
        other => panic!("expected two-arg AccessFunction, got {other:?}"),
    }
}

#[test]
fn a_custom_discipline_access_name_parses_as_a_plain_call() {
    // The grammar has no symbol table, so an access-function name that
    // isn't one of the four statically-known ones (V/I/potential/flow)
    // parses as an ordinary call — "the shape the elaborator can promote"
    // once it knows the discipline's `access` attribute (LRMC-C1-09,
    // resolved at elaboration, not here).
    match parse("Volts(b1)") {
        Expr::Call { name, args, .. } => {
            assert_eq!(name, "Volts");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected a plain Expr::Call, got {other:?}"),
    }
}

// ── system function calls, chapter 9 (used as a value) ───────────────────

#[test]
fn system_function_call_parses() {
    let toks = Lexer::new("$abstime").tokenize().unwrap();
    let mut g = Grammar::new(toks);
    match g.parse_expr().unwrap() {
        Expr::SystemFunctionCall { name, args, .. } => {
            assert_eq!(name, "abstime");
            assert!(args.is_empty());
        }
        other => panic!("expected SystemFunctionCall, got {other:?}"),
    }
}

// ── primaries, Annex A.8.4/A.8.7/A.8.8 ───────────────────────────────────

#[test]
fn number_and_string_literals_parse_as_primaries() {
    // A number literal (§2.6) and a string literal (§2.7) each parse as
    // their own `Expr` primary — Annex A.8.4's `primary` includes both
    // `number` (A.8.7) and `string` (A.8.8) alongside a plain identifier
    // and a parenthesized `mintypmax_expression`.
    match parse("42") {
        Expr::Number { .. } => {}
        other => panic!("expected Expr::Number, got {other:?}"),
    }
    match parse("3.5") {
        Expr::Number { .. } => {}
        other => panic!("expected Expr::Number, got {other:?}"),
    }
    match parse("\"hello\"") {
        Expr::Str { value, .. } => assert_eq!(value, "hello"),
        other => panic!("expected Expr::Str, got {other:?}"),
    }
    match parse("plain_ident") {
        Expr::Ident { name, .. } => assert_eq!(name, "plain_ident"),
        other => panic!("expected Expr::Ident, got {other:?}"),
    }
}

// ── error path ────────────────────────────────────────────────────────

#[test]
fn unclosed_paren_is_a_typed_parse_error() {
    // `expect_punct(")")` reports what it wanted and what it hit — here
    // end-of-file — rather than a generic "unexpected EOF".
    let toks = Lexer::new("(a + b").tokenize().unwrap();
    let mut g = Grammar::new(toks);
    let err = g.parse_expr().unwrap_err();
    match err.kind {
        piperine_lang::ParseErrorKind::UnexpectedToken { expected, found } => {
            assert_eq!(expected, "')'");
            assert_eq!(found, "end of file");
        }
        other => panic!("expected UnexpectedToken, got {other:?}"),
    }
}
