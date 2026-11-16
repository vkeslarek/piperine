//! Scope: value code of `piperine_lang::elab` for msl-digital (DIG-08): integer types, literals,
//! operators, casts and methods lowered to the IR's own nodes, and integer constant expressions
//! folded at elaboration by the rules the compiled code follows (§1 rule 6, ROADMAP G4 and G5),
//! compared with the compiled code's own integers (`piperine_transpile::rt`).

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{BinaryOp, Expr, ExprId, Ir, Method, ValueType};
use piperine_transpile::rt::int::{Int, UInt};
use support::Elab;

/// Reading the elaborated IR of a test.
struct Read<'a> {
    ir: &'a Ir,
}

impl<'a> Read<'a> {
    /// The body expression of the function named `name`.
    fn body_of(&self, name: &str) -> ExprId {
        self.ir.functions().find(|f| f.name() == name).unwrap_or_else(|| panic!("no function `{name}` in:\n{}", self.ir.print_structural())).body()
    }

    /// Every expression reachable from `root`, `root` first.
    fn reach(&self, root: ExprId) -> Vec<ExprId> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            out.push(e);
            match self.ir.expr(e) {
                Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::IsKnown(expr) | Expr::High(expr) => stack.push(*expr),
                Expr::Binary { lhs, rhs, .. } => stack.extend([*lhs, *rhs]),
                Expr::KnownOr { value, default } => stack.extend([*value, *default]),
                Expr::If { cond, then, otherwise } => stack.extend([*cond, *then, *otherwise]),
                Expr::Method { receiver, args, .. } => stack.extend(std::iter::once(*receiver).chain(args.iter().copied())),
                Expr::Call { args, .. } | Expr::Math { args, .. } => stack.extend(args.iter().copied()),
                Expr::Block { stmts, value } => {
                    for s in stmts {
                        if let piperine_ir::Stmt::Let { init, .. } | piperine_ir::Stmt::Expr(init) = self.ir.stmt(*s) {
                            stack.push(*init);
                        }
                    }
                    stack.extend(value.iter().copied());
                }
                _ => {}
            }
        }
        out
    }

    /// The text of the function named `name` in the structural print: its header and the lines
    /// under it.
    fn function_text(&self, name: &str) -> String {
        let text = self.ir.print_structural();
        let head = format!("fn {name}(");
        let start = text.find(&head).unwrap_or_else(|| panic!("no `{head}` in:\n{text}"));
        let rest = &text[start..];
        let end = rest[1..].find("\nfn ").or_else(|| rest[1..].find("\ninstance ")).map_or(rest.len(), |at| at + 1);
        rest[..end].to_string()
    }
}

/// A design whose `Top` calls each function of `functions` from its `analog` block, so that the
/// top reaches it.
fn reaching(items: &str, calls: &[&str]) -> String {
    let lets: Vec<String> = calls.iter().enumerate().map(|(k, call)| format!("let x{k} = {call};")).collect();
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {} }}", lets.join(" "))
}

#[test]
fn integers_lower_to_typed_ir() {
    // DIG-08 AC 1: the buck's `rev3` and `dithered_width` (examples/buck/src/dpwm.msl), and the
    // remaining operators, a cast in each direction and the integer methods.
    let items = "fn rev3(k: u3) -> u3 { ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1) }\n\
                 fn dithered_width(duty: u12, frame: u3) -> u9 { let coarse = (duty >> 3) as u9; let fine = (duty & 7) as u3; if rev3(frame) < fine { coarse + 1 } else { coarse } }\n\
                 fn ops(a: i16, b: i16) -> i16 { let q = ((a / b) % 7) ^ !a; -q * (q - b) }\n\
                 fn bounded(x: i16) -> i16 { x.clamp(-256, 255).min(x).max(-1) }\n\
                 fn down(x: f64) -> u8 { x.floor() as u8 }\n\
                 fn up(x: u8) -> f64 { x as f64 }";
    let src = reaching(items, &["dithered_width(4095, 7)", "ops(1, 2)", "bounded(3)", "down(2.5)", "up(4)"]);
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };

    // The printed forms of the `ir-rust` T29 fixtures (crates/piperine-ir/tests/golden/buck/dpwm.txt),
    // here with every value known.
    assert!(read.function_text("rev3").contains("fn rev3(k: u3) -> u3\n  local k: u3\n  ((((k & 1u3) << 2i32) | (k & 2u3)) | ((k >> 2i32) & 1u3))"), "{}", read.function_text("rev3"));
    let dithered = read.function_text("dithered_width");
    for line in ["fn dithered_width(duty: u12, frame: u3) -> u9", "let coarse = ((duty >> 3i32) as u9)", "let fine = ((duty & 7u12) as u3)", "if (rev3(frame) < fine) { (coarse + 1u9) } else { coarse }"] {
        assert!(dithered.contains(line), "`{line}` in:\n{dithered}");
    }

    // Node by node: `UInt`, `Int`, integer literals, the operators, `Cast` and `Method`.
    let rev3 = read.reach(read.body_of("rev3"));
    assert_eq!(*ir.expr_type(rev3[0]), ValueType::UInt(3));
    assert!(matches!(ir.expr(rev3[0]), Expr::Binary { op: BinaryOp::BitOr, .. }));
    let ops_of = |name: &str| -> Vec<BinaryOp> { read.reach(read.body_of(name)).into_iter().filter_map(|e| if let Expr::Binary { op, .. } = ir.expr(e) { Some(*op) } else { None }).collect() };
    let rev3_ops = ops_of("rev3");
    for op in [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::Shl, BinaryOp::Shr] {
        assert!(rev3_ops.contains(&op), "{op:?} in rev3");
    }
    let literals: Vec<(i128, ValueType)> = rev3.iter().filter_map(|e| if let Expr::Int(v) = ir.expr(*e) { Some((*v, ir.expr_type(*e).clone())) } else { None }).collect();
    assert!(literals.contains(&(1, ValueType::UInt(3))) && literals.contains(&(2, ValueType::UInt(3))), "an operand literal takes the other operand's type: {literals:?}");
    assert!(literals.contains(&(2, ValueType::Int(32))), "a bare shift amount is an `i32` (§1 rule 7): {literals:?}");

    let ops_text = read.function_text("ops");
    assert!(ops_text.contains("fn ops(a: i16, b: i16) -> i16"), "{ops_text}");
    assert!(ops_text.contains("let q = (((a / b) % 7i16) ^ (!a))"), "{ops_text}");
    assert!(ops_text.contains("((-q) * (q - b))"), "{ops_text}");
    for op in [BinaryOp::Div, BinaryOp::Rem, BinaryOp::BitXor, BinaryOp::Mul, BinaryOp::Sub] {
        assert!(ops_of("ops").contains(&op), "{op:?} in ops");
    }

    let bounded = read.reach(read.body_of("bounded"));
    let methods: Vec<Method> = bounded.iter().filter_map(|e| if let Expr::Method { method, .. } = ir.expr(*e) { Some(*method) } else { None }).collect();
    assert_eq!(methods, [Method::Max, Method::Min, Method::Clamp], "outermost first");
    assert!(bounded.iter().all(|e| !matches!(ir.expr(*e), Expr::Method { .. }) || *ir.expr_type(*e) == ValueType::Int(16)), "each method gives the receiver's type");
    assert!(read.function_text("bounded").contains("x.clamp(-256i16, 255i16).min(x).max(-1i16)"), "{}", read.function_text("bounded"));

    let down = read.reach(read.body_of("down"));
    assert!(matches!(ir.expr(down[0]), Expr::Cast { to: ValueType::UInt(8), .. }) && *ir.expr_type(down[0]) == ValueType::UInt(8));
    assert!(down.iter().any(|e| matches!(ir.expr(*e), Expr::Method { method: Method::Floor, .. }) && *ir.expr_type(*e) == ValueType::F64));
    let up = read.reach(read.body_of("up"));
    assert!(matches!(ir.expr(up[0]), Expr::Cast { to: ValueType::F64, .. }) && *ir.expr_type(up[0]) == ValueType::F64);

    // The function signatures carry the integer types.
    let dw = ir.functions().find(|f| f.name() == "dithered_width").expect("dithered_width");
    assert_eq!(dw.params().iter().map(|p| p.ty.clone()).collect::<Vec<_>>(), [ValueType::UInt(12), ValueType::UInt(3)]);
    assert_eq!(*dw.ret(), ValueType::UInt(9));
}

#[test]
fn integer_constants_fold() {
    // DIG-08 AC 2: `N_PERIOD - 1` with `N_PERIOD: u9 = 400` is the literal 399 of `u9`; the
    // other constant expressions of `Dpwm` fold the same way (400 - 4 = 396).
    let items = "const N_PERIOD: u9 = 400;\nconst DEAD_COUNTS: u9 = 4;\nconst E_MIN: i16 = -256;\n\
                 fn last() -> u9 { N_PERIOD - 1 }\n\
                 fn wrap(count: u9) -> bool { count == N_PERIOD - 1 }\n\
                 fn tail(next: u9, off_at: u9) -> bool { next >= off_at + DEAD_COUNTS && next < N_PERIOD - DEAD_COUNTS }\n\
                 fn floor_e() -> i16 { E_MIN }\n\
                 fn widened() -> i32 { (N_PERIOD as i32) * 2 }";
    let src = reaching(items, &["last()", "wrap(3)", "tail(1, 2)", "floor_e()", "widened()"]);
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    let folded = |name: &str| {
        let body = read.body_of(name);
        (ir.expr(body).clone(), ir.expr_type(body).clone())
    };
    assert_eq!(folded("last"), (Expr::Int(399), ValueType::UInt(9)));
    assert_eq!(folded("floor_e"), (Expr::Int(-256), ValueType::Int(16)));
    assert_eq!(folded("widened"), (Expr::Int(800), ValueType::Int(32)), "a cast of a constant reads only constants: folded");
    assert!(read.function_text("wrap").contains("(count == 399u9)"), "{}", read.function_text("wrap"));
    let tail = read.function_text("tail");
    assert!(tail.contains("(next >= (off_at + 4u9))") && tail.contains("(next < 396u9)"), "{tail}");
}

/// One folded case: the function, the type its result has, the value the specification gives
/// (with the rule), and the value the compiled code's integers give.
struct Fold {
    function: &'static str,
    ty: ValueType,
    spec: i128,
    compiled: i128,
}

#[test]
fn folded_casts_shifts_and_sums_wrap_as_compiled_code_does() {
    let cases = [
        // G4: `as` from `f64` saturates to the target's range.
        Fold { function: "fn f0() -> u8 { 300.0 as u8 }", ty: ValueType::UInt(8), spec: 255, compiled: i128::from(UInt::<8>::from_f64(300.0).get()) },
        // G4: NaN gives 0.
        Fold { function: "fn f1() -> u8 { (0.0 / 0.0) as u8 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from(UInt::<8>::from_f64(f64::NAN).get()) },
        // G5: `<<` by the width or more gives 0.
        Fold { function: "fn f2() -> u8 { 1u8 << 9 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from((UInt::<8>::new(1) << Int::<32>::new(9)).get()) },
        // G5: a signed `>>` by the width or more gives the sign.
        Fold { function: "fn f3() -> i8 { -1i8 >> 9 }", ty: ValueType::Int(8), spec: -1, compiled: i128::from((Int::<8>::new(-1) >> Int::<32>::new(9)).get()) },
        // §1 rule 6: arithmetic wraps modulo 2^N.
        Fold { function: "fn f4() -> u9 { 511u9 + 1 }", ty: ValueType::UInt(9), spec: 0, compiled: i128::from((UInt::<9>::new(511) + UInt::<9>::new(1)).get()) },
        // G4 on a signed target, both ends, and truncation toward zero.
        Fold { function: "fn f5() -> i8 { -200.7 as i8 }", ty: ValueType::Int(8), spec: -128, compiled: i128::from(Int::<8>::from_f64(-200.7).get()) },
        Fold { function: "fn f6() -> i8 { 126.9 as i8 }", ty: ValueType::Int(8), spec: 126, compiled: i128::from(Int::<8>::from_f64(126.9).get()) },
        Fold { function: "fn f7() -> u8 { -3.9 as u8 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from(UInt::<8>::from_f64(-3.9).get()) },
        // §1 rule 1: among integer types `as` truncates (300 mod 256) and extends by the sign.
        Fold { function: "fn f8() -> u8 { 300 as u8 }", ty: ValueType::UInt(8), spec: 44, compiled: i128::from(Int::<32>::new(300).to_uint::<8>().get()) },
        Fold { function: "fn f9() -> u16 { -1i8 as u16 }", ty: ValueType::UInt(16), spec: 65535, compiled: i128::from(Int::<8>::new(-1).to_uint::<16>().get()) },
        // §1 rule 6 in two's complement: -128 / -1 wraps to -128; `*` wraps.
        Fold { function: "fn f10() -> i8 { -128i8 / -1 }", ty: ValueType::Int(8), spec: -128, compiled: i128::from((Int::<8>::new(-128) / Int::<8>::new(-1)).get()) },
        Fold { function: "fn f11() -> u8 { 255u8 * 2 }", ty: ValueType::UInt(8), spec: 254, compiled: i128::from((UInt::<8>::new(255) * UInt::<8>::new(2)).get()) },
        // `%` keeps the dividend's sign, as in Rust; `!` and unary `-` wrap.
        Fold { function: "fn f12() -> i8 { -7i8 % 3 }", ty: ValueType::Int(8), spec: -1, compiled: i128::from((Int::<8>::new(-7) % Int::<8>::new(3)).get()) },
        Fold { function: "fn f13() -> u4 { !0u4 }", ty: ValueType::UInt(4), spec: 15, compiled: i128::from((!UInt::<4>::new(0)).get()) },
        Fold { function: "fn f14() -> u4 { -1u4 }", ty: ValueType::UInt(4), spec: 15, compiled: i128::from((-UInt::<4>::new(1)).get()) },
        // An unsigned `>>` by the width or more is 0; within the width it is logical.
        Fold { function: "fn f15() -> u8 { 128u8 >> 8 }", ty: ValueType::UInt(8), spec: 0, compiled: i128::from((UInt::<8>::new(128) >> Int::<32>::new(8)).get()) },
        Fold { function: "fn f16() -> i8 { -128i8 >> 7 }", ty: ValueType::Int(8), spec: -1, compiled: i128::from((Int::<8>::new(-128) >> Int::<32>::new(7)).get()) },
        // `clamp`, `min`, `max` of constants.
        Fold { function: "fn f17() -> i16 { 300i16.clamp(-256, 255) }", ty: ValueType::Int(16), spec: 255, compiled: i128::from(Int::<16>::new(300).clamp(Int::<16>::new(-256), Int::<16>::new(255)).get()) },
    ];
    let items: Vec<&str> = cases.iter().map(|c| c.function).collect();
    let calls: Vec<String> = (0..cases.len()).map(|k| format!("f{k}()")).collect();
    let src = reaching(&items.join("\n"), &calls.iter().map(String::as_str).collect::<Vec<_>>());
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    for (k, case) in cases.iter().enumerate() {
        assert_eq!(case.spec, case.compiled, "{}: the compiled code's integers disagree with the specification", case.function);
        let body = read.body_of(&format!("f{k}"));
        assert_eq!((ir.expr(body).clone(), ir.expr_type(body).clone()), (Expr::Int(case.spec), case.ty.clone()), "{}", case.function);
    }
}

#[test]
fn a_constant_division_by_zero_is_e0104() {
    // DIG-08 AC 4, §1 rule 11: a panic while a const expression is evaluated is `E0104`.
    for (src, at) in [
        ("const K: u8 = 1 / 0;\nfn f() -> u8 { K }", "1 / 0"),
        ("fn f() -> u8 { 7 % 0 }", "7 % 0"),
        ("const Z: i8 = 0;\nfn f() -> i8 { 5i8 / Z }", "5i8 / Z"),
        ("fn f() -> i16 { 3i16.clamp(2, 1) }", "3i16.clamp(2, 1)"),
    ] {
        let src = reaching(src, &["f()"]);
        let err = Elab::run(&src, "Top").expect_err(at);
        assert_eq!(Elab::codes(&err), ["E0104"], "{at}");
        let span = err[0].diagnostic.span();
        assert_eq!(&src[span.start..span.end], at);
    }
    // A division whose divisor is not a constant is left to the run (§1 rule 11).
    Elab::ir(&reaching("fn f(d: u8) -> u8 { 7 / d }", &["f(0)"]), "Top");
}
