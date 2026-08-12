//! Proves the expression grammar (tasks.md T7) against LRM chapter 4: every
//! operator parses with the right precedence/associativity (asserted on
//! parse **shape**, not just success), ternary, concat/replication,
//! part-select/index, math and time-domain functions, and both access
//! function arities.

use piperine_vams::ast::{BinOp, Expr, UnOp};
use piperine_vams::{Grammar, Lexer};

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

// ── error path ────────────────────────────────────────────────────────

#[test]
fn unclosed_paren_is_a_typed_parse_error() {
    // `expect_punct(")")` reports what it wanted and what it hit — here
    // end-of-file — rather than a generic "unexpected EOF".
    let toks = Lexer::new("(a + b").tokenize().unwrap();
    let mut g = Grammar::new(toks);
    let err = g.parse_expr().unwrap_err();
    match err.kind {
        piperine_vams::ParseErrorKind::UnexpectedToken { expected, found } => {
            assert_eq!(expected, "')'");
            assert_eq!(found, "end of file");
        }
        other => panic!("expected UnexpectedToken, got {other:?}"),
    }
}
