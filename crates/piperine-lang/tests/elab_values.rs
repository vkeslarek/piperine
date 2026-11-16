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

/// The `local` lines of the structural print of an owner: `name: type` for each.
fn locals_in(text: &str) -> Vec<String> {
    text.lines().filter_map(|l| l.trim().strip_prefix("local ")).map(str::to_string).collect()
}

#[test]
fn a_carrier_read_is_flagged_and_the_flag_reads_are_not() {
    // DIG-08 AC 5, design "Unknown-ness: the `Known` analysis", row by row. `Unknown` (§3.3.1) is
    // the source of an unknown value here; a carrier read is the other, lowered with carriers.
    let items = "const K: u8 = 3;\n\
                 fn rows(p: u8) -> bool {\n\
                   let lit = 5u8;\n\
                   let konst = K;\n\
                   let param = p;\n\
                   let u: u8 = Unknown;\n\
                   let count = u + 1;\n\
                   let cast = count as u16;\n\
                   let m = count.min(p);\n\
                   let c = if count > p { 1u8 } else { 2u8 };\n\
                   let neg = !(count == p);\n\
                   let k = count.known_or(0);\n\
                   let ik = count.is_known();\n\
                   let b: bool = Unknown;\n\
                   let h = b.high();\n\
                   let copy = c;\n\
                   ik & h & (k > lit) & (konst == param)\n\
                 }";
    let src = format!("{items}\nmodule Top() {{ g: f64 = 1.0; signal s: Electrical; }}\nanalog for Top {{ let pr = v(s); let t = now(); let st = analysis(Static); let te = temperature(); let pa = g; let r = rows(3); }}");
    let ir = Elab::ir(&src, "Top");
    let read = Read { ir: &ir };
    let rows = read.function_text("rows");
    let locals = locals_in(&rows);
    for (name, ty) in [
        // literal, const, param: known.
        ("lit", "u8"),
        ("konst", "u8"),
        ("param", "u8"),
        // `Unknown`: may be unknown; so is every operator, cast, method and `if` over it.
        ("u", "flagged<u8>"),
        ("count", "flagged<u8>"),
        ("cast", "flagged<u16>"),
        ("m", "flagged<u8>"),
        ("c", "flagged<u8>"),
        ("neg", "flagged<bool>"),
        // The flag reads are known.
        ("k", "u8"),
        ("ik", "bool"),
        ("b", "flagged<bool>"),
        ("h", "bool"),
        // A local is what its initializer was.
        ("copy", "flagged<u8>"),
    ] {
        assert!(locals.contains(&format!("{name}: {ty}")), "local {name}: {ty} in:\n{rows}");
    }
    // The flag reads lower to the IR's own nodes over the unknown value.
    assert!(rows.contains("let k = count.known_or(0u8)") && rows.contains("let ik = count.is_known()") && rows.contains("let h = b.high()"), "{rows}");
    // An `if` on a value that may be unknown is kept as written: no default, no `known_or` added.
    assert!(rows.contains("let c = if (count > p) { 1u8 } else { 2u8 }"), "{rows}");
    assert_eq!(rows.matches("known_or").count(), 1, "the elaborator adds no `known_or`:\n{rows}");
    assert_eq!(*ir.functions().find(|f| f.name() == "rows").expect("rows").ret(), ValueType::Bool, "a result read through the flag reads is known");

    // Probe, `now()`, `analysis(Static)`, the environment and a param: known.
    let top = ir.print_structural();
    let body = &top[top.find("body Top").expect("body Top")..];
    let locals = locals_in(body);
    for local in ["pr: f64", "t: f64", "st: bool", "te: f64", "pa: f64", "r: bool"] {
        assert!(locals.contains(&local.to_string()), "local {local} in:\n{body}");
    }
    assert!(body.contains("let t = time()") && body.contains("let st = static()"), "{body}");
}

#[test]
fn a_short_circuit_over_a_flagged_operand_becomes_bitwise() {
    // DIG-08 AC 6, §3.3.12: `&&`/`||` with an operand that may be unknown is `&`/`|`; over
    // known operands it stays the short-circuit form.
    let items = "fn trip(a: bool, b: bool) -> bool {\n\
                   let hs: bool = Unknown;\n\
                   let oc: bool = Unknown;\n\
                   let t = hs && oc.known_or(true);\n\
                   let o = a || hs;\n\
                   let plain = a && b;\n\
                   let either = a || b;\n\
                   t.known_or(false) && o.known_or(false) && plain && either\n\
                 }";
    let ir = Elab::ir(&reaching(items, &["trip(true, false)"]), "Top");
    let read = Read { ir: &ir };
    let trip = read.function_text("trip");
    assert!(trip.contains("let t = (hs & oc.known_or(true))"), "{trip}");
    assert!(trip.contains("let o = (a | hs)"), "{trip}");
    assert!(trip.contains("let plain = (a && b)"), "{trip}");
    assert!(trip.contains("let either = (a || b)"), "{trip}");
    let locals = locals_in(&trip);
    for local in ["t: flagged<bool>", "o: flagged<bool>", "plain: bool", "either: bool"] {
        assert!(locals.contains(&local.to_string()), "local {local} in:\n{trip}");
    }
    let ops: Vec<BinaryOp> = read.reach(read.body_of("trip")).into_iter().filter_map(|e| if let Expr::Binary { op, .. } = ir.expr(e) { Some(*op) } else { None }).collect();
    assert!(ops.contains(&BinaryOp::BitAnd) && ops.contains(&BinaryOp::BitOr) && ops.contains(&BinaryOp::And) && ops.contains(&BinaryOp::Or), "{ops:?}");
}

#[test]
fn a_partial_right_operand_under_a_flagged_short_circuit_is_p0001() {
    // DIG-08 AC 7: as `&`, the right operand runs whatever the left gives; one that can panic
    // (an integer `/` or `%`) is refused, at the operand.
    for (body, at) in [
        ("let c: bool = Unknown; let r = c && (x / n > 1); r.known_or(false)", "(x / n > 1)"),
        ("let c: bool = Unknown; let r = c || (x % n == 0); r.known_or(false)", "(x % n == 0)"),
    ] {
        let src = reaching(&format!("fn f(x: u8, n: u8) -> bool {{ {body} }}"), &["f(1, 2)"]);
        let err = Elab::run(&src, "Top").expect_err(at);
        assert_eq!(Elab::codes(&err), ["P0001"], "{at}");
        let span = err[0].diagnostic.span();
        assert_eq!(&src[span.start..span.end], at);
        assert!(err[0].diagnostic.message.contains("integer `/` or `%`"), "{}", err[0].diagnostic.message);
    }
    // Over known operands the short circuit guards the division: accepted, still `&&`.
    let ir = Elab::ir(&reaching("fn f(c: bool, x: u8, n: u8) -> bool { c && (x / n > 1) }", &["f(true, 1, 2)"]), "Top");
    assert!(Read { ir: &ir }.function_text("f").contains("(c && ((x / n) > 1u8))"));
}

#[test]
fn a_function_gets_one_variant_per_flag_pattern() {
    // DIG-08 AC 8. The call of `Dpwm`'s reaction (examples/buck/src/dpwm.msl), here in a function
    // whose `duty` and `frame` may be unknown as the carriers are: the pattern `01` lowers
    // `dithered_width~01`, whose `rev3(frame)` lowers `rev3~1`; no variant of all-known
    // arguments is reached, so no `dithered_width` and no `rev3`.
    let functions = "fn rev3(k: u3) -> u3 { ((k & 1) << 2) | (k & 2) | ((k >> 2) & 1) }\n\
                     fn dithered_width(duty: u12, frame: u3) -> u9 {\n\
                       let coarse = (duty >> 3) as u9;\n\
                       let fine = (duty & 7) as u3;\n\
                       if rev3(frame) < fine { coarse + 1 } else { coarse }\n\
                     }\n\
                     fn latch() -> u9 { let duty: u12 = Unknown; let frame: u3 = Unknown; let w = dithered_width(duty.known_or(0), frame + 1); w.known_or(0) }";
    let ir = Elab::ir(&reaching(functions, &["latch()"]), "Top");
    let names: Vec<&str> = ir.functions().map(|f| f.name()).collect();
    assert_eq!(names, ["latch", "dithered_width~01", "rev3~1"], "one function per pattern reached, in first-reach order");
    // The two variants print as the `ir-rust` T29 fixture prints them
    // (crates/piperine-ir/tests/golden/buck/dpwm.txt).
    let read = Read { ir: &ir };
    assert_eq!(
        read.function_text("\"dithered_width~01\""),
        "fn \"dithered_width~01\"(duty: u12, frame: flagged<u3>) -> flagged<u9>\n  local coarse: u9\n  local duty: u12\n  local fine: u3\n  local frame: flagged<u3>\n  let coarse = ((duty >> 3i32) as u9)\n  let fine = ((duty & 7u12) as u3)\n  if (\"rev3~1\"(frame) < fine) { (coarse + 1u9) } else { coarse }"
    );
    assert_eq!(read.function_text("\"rev3~1\""), "fn \"rev3~1\"(k: flagged<u3>) -> flagged<u3>\n  local k: flagged<u3>\n  ((((k & 1u3) << 2i32) | (k & 2u3)) | ((k >> 2i32) & 1u3))");
    assert!(read.function_text("latch").contains("let w = \"dithered_width~01\"(duty.known_or(0u12), (frame + 1u3))"), "{}", read.function_text("latch"));

    // A call with every argument known reaches the variant under the source name, beside the
    // other: each pattern its own function.
    let ir = Elab::ir(&reaching(functions, &["latch()", "dithered_width(1325, 4)"]), "Top");
    let mut names: Vec<&str> = ir.functions().map(|f| f.name()).collect();
    names.sort_unstable();
    assert_eq!(names, ["dithered_width", "dithered_width~01", "latch", "rev3", "rev3~1"]);
    let plain = ir.functions().find(|f| f.name() == "dithered_width").expect("dithered_width");
    assert_eq!(plain.params().iter().map(|p| p.ty.clone()).collect::<Vec<_>>(), [ValueType::UInt(12), ValueType::UInt(3)]);
    assert_eq!(*plain.ret(), ValueType::UInt(9));
    // One variant per pattern, however many calls reach it.
    let ir = Elab::ir(&reaching(functions, &["latch()", "latch()", "dithered_width(1, 2)", "dithered_width(3, 4)"]), "Top");
    assert_eq!(ir.functions().count(), 5);
}

#[test]
fn an_enum_is_its_tag_and_a_match_is_an_if_chain() {
    // DIG-08 AC 9, §2 rule 5: an enum of `V` unit variants is `UInt<ceil(log2 V)>`, variant `k`
    // the literal `k`; a `match` is an `if` chain that reads its scrutinee once. The enums are
    // the buck's (examples/buck/src/supervisor.msl) and the stdlib's `Severity` (std/io.msl).
    let items = "#[derive(Repr, Clone, Copy, PartialEq)]\npub enum Mode { Off, SoftStart, Regulate, Hiccup }\n\
                 #[derive(Repr, Clone, Copy, PartialEq)]\npub enum Fault { Clear, OverCurrent, OverVoltage }\n\
                 fn hiccup() -> Mode { Mode::Hiccup }\n\
                 fn clear() -> Fault { Fault::Clear }\n\
                 fn warning(s: Severity) -> bool { s == Severity::Warning }\n\
                 fn next(m: Mode, ok: bool) -> Mode {\n\
                   match m {\n\
                     Mode::SoftStart | Mode::Regulate => Mode::Regulate,\n\
                     Mode::Off => if ok { Mode::SoftStart } else { Mode::Off },\n\
                     Mode::Hiccup => Mode::SoftStart,\n\
                   }\n\
                 }\n\
                 fn severe(f: Fault) -> Severity { match f { Fault::Clear => Severity::Info, _ => Severity::Error } }\n\
                 fn once(f: Fault) -> u8 { match severe(f) { Severity::Info => 0, Severity::Warning => 1, Severity::Error => 2 } }\n\
                 fn unknown_mode() -> bool { let m: Mode = Unknown; let s = match m { Mode::SoftStart | Mode::Regulate => true, _ => false }; s.known_or(false) }";
    let calls = ["hiccup()", "clear()", "warning(Severity::Warning)", "next(Mode::Off, true)", "once(Fault::Clear)", "unknown_mode()"];
    let ir = Elab::ir(&reaching(items, &calls), "Top");
    let read = Read { ir: &ir };
    let function = |name: &str| ir.functions().find(|f| f.name() == name).unwrap_or_else(|| panic!("no {name}"));

    // `Mode` (4 variants) is `UInt(2)` and `Hiccup` its 4th, 3; `Fault` and `Severity` (3 each)
    // are `UInt(2)`.
    assert_eq!(*function("hiccup").ret(), ValueType::UInt(2));
    assert_eq!(ir.expr(read.body_of("hiccup")), &Expr::Int(3));
    assert_eq!((*function("clear").ret()).clone(), ValueType::UInt(2));
    assert_eq!(ir.expr(read.body_of("clear")), &Expr::Int(0));
    assert_eq!(function("warning").params()[0].ty, ValueType::UInt(2));
    assert!(read.function_text("warning").contains("(s == 1u2)"), "{}", read.function_text("warning"));
    assert_eq!(function("next").params().iter().map(|p| p.ty.clone()).collect::<Vec<_>>(), [ValueType::UInt(2), ValueType::Bool]);

    // The arms in order; an or-pattern over a known scrutinee is `||`; the last arm is the final
    // `else`; a param is read where it is.
    let next = read.function_text("next");
    assert!(next.contains("if ((m == 1u2) || (m == 2u2)) { 2u2 } else { if (m == 0u2) { if ok { 1u2 } else { 0u2 } } else { 1u2 } }"), "{next}");
    // `_` is the final `else`.
    assert!(read.function_text("severe").contains("if (f == 0u2) { 0u2 } else { 2u2 }"), "{}", read.function_text("severe"));
    // A scrutinee that is not a plain read is bound to a local once, and read through it.
    let once = read.function_text("once");
    assert_eq!(once.matches("severe(f)").count(), 1, "{once}");
    assert!(once.contains("local match$0: u2") && once.contains("let match$0 = severe(f)"), "{once}");
    assert!(once.contains("if (match$0 == 0u2) { 0u8 } else { if (match$0 == 1u2) { 1u8 } else { 2u8 } }"), "{once}");
    // Over a scrutinee that may be unknown, the or-pattern is `|` and every test is flagged.
    let unknown = read.function_text("unknown_mode");
    assert!(unknown.contains("let s = if ((m == 1u2) | (m == 2u2)) { true } else { false }"), "{unknown}");
    assert!(locals_in(&unknown).contains(&"s: flagged<bool>".to_string()), "{unknown}");
    let ops: Vec<BinaryOp> = read.reach(read.body_of("unknown_mode")).into_iter().filter_map(|e| if let Expr::Binary { op, .. } = ir.expr(e) { Some(*op) } else { None }).collect();
    assert!(ops.contains(&BinaryOp::BitOr) && !ops.contains(&BinaryOp::Or), "{ops:?}");

    // An enum of one variant has a tag of no level: no IR type holds it (spec Edge Cases).
    let src = reaching("#[derive(Repr, Clone, Copy, PartialEq)]\npub enum One { Only }\nfn only() -> One { One::Only }", &["only()"]);
    let err = Elab::run(&src, "Top").expect_err("a tag of no level");
    assert!(Elab::codes(&err).iter().all(|c| c == "P0001"), "{:?}", Elab::codes(&err));
    assert!(err.iter().any(|d| d.diagnostic.message.contains("an enum of one variant")), "{:?}", err.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>());
}

#[test]
fn an_option_is_a_flag_and_a_payload() {
    // DIG-08 AC 10: an `Option<T>` is a `bool` and a `T` through `Some`, `None`, `known()`,
    // `is_some`, `is_none`, `unwrap_or`, `if let` and `match`; no IR type is an `Option`.
    // `setpoint` is the body of `SetpointReg`'s reaction (examples/buck/src/control.msl) and
    // `supervise` holds `Supervisor`'s `fault_now` (examples/buck/src/supervisor.msl), their
    // carriers here locals that may be unknown and their commits results.
    let items = "#[derive(Repr, Clone, Copy, PartialEq)]\npub enum Fault { Clear, OverCurrent, OverVoltage }\n\
                 const SETPOINT_MAX: u8 = 168;\nconst OC_PERIODS: u4 = 8;\nconst OV_SAMPLES: u2 = 2;\n\
                 fn setpoint(code: u8) -> u8 { let data: u8 = Unknown; if let Some(d) = data.known() { d.min(SETPOINT_MAX) } else { code } }\n\
                 fn supervise(oc_n: u4, ov_n: u2) -> u8 {\n\
                   let fault_now = if oc_n >= OC_PERIODS { Some(Fault::OverCurrent) }\n\
                                   else if ov_n >= OV_SAMPLES { Some(Fault::OverVoltage) }\n\
                                   else { None };\n\
                   let latched = if let Some(f) = fault_now { f } else { Fault::Clear };\n\
                   let quiet = fault_now.is_none();\n\
                   let tripped = fault_now.is_some();\n\
                   let cause = fault_now.unwrap_or(Fault::Clear);\n\
                   let code: u8 = match fault_now { Some(Fault::OverCurrent) => 1, Some(f) => if f == latched { 2 } else { 3 }, None => 0 };\n\
                   if quiet && !tripped && cause == Fault::Clear { code } else { code + 1 }\n\
                 }\n\
                 fn literal() -> u8 { Some(3u8).unwrap_or(9) }\n\
                 fn read(x: u8) -> u8 { let k = x.known(); k.unwrap_or(5) }";
    let ir = Elab::ir(&reaching(items, &["setpoint(1)", "supervise(0, 0)", "literal()", "read(2)"]), "Top");
    let text = ir.print_structural();
    assert!(!text.contains("Option"), "no IR type is an `Option`:\n{text}");
    let read = Read { ir: &ir };

    // `if let Some(d) = data.known()`: the pair written where it is read, as the `ir-rust` T29
    // fixture writes it (crates/piperine-ir/tests/golden/buck/setpoint_reg.txt).
    let setpoint = read.function_text("setpoint");
    assert!(setpoint.contains("if data.is_known() { let d = data.known_or(0u8); d.min(168u8) } else { code }"), "{setpoint}");
    assert!(locals_in(&setpoint).contains(&"d: u8".to_string()), "`known()` gives a known payload:\n{setpoint}");

    // A `let` of an `Option` binds its flag and its payload; `Some`/`None` give each.
    let supervise = read.function_text("supervise");
    let locals = locals_in(&supervise);
    for local in ["fault_now.is: bool", "fault_now.val: u2", "f: u2", "latched: u2", "quiet: bool", "tripped: bool", "cause: u2", "match$0.is: bool", "match$0.val: u2"] {
        assert!(locals.contains(&local.to_string()), "local {local} in:\n{supervise}");
    }
    for line in [
        "let fault_now.is = if (oc_n >= 8u4) { true } else { if (ov_n >= 2u2) { true } else { false } }",
        "let fault_now.val = if (oc_n >= 8u4) { 1u2 } else { if (ov_n >= 2u2) { 2u2 } else { 0u2 } }",
        // `if let`: the flag decides, the binding reads the payload.
        "let latched = if fault_now.is { let f = fault_now.val; f } else { 0u2 }",
        "let quiet = (!fault_now.is)",
        "let tripped = fault_now.is",
        "let cause = if fault_now.is { fault_now.val } else { 0u2 }",
        // `match`: `Some(v)` is the flag and the payload equal to `v`, `Some(f)` binds the payload,
        // `None` the last arm.
        "let match$0.is = fault_now.is",
        "let match$0.val = fault_now.val",
        "if (match$0.is && (match$0.val == 1u2)) { 1u8 } else { if match$0.is { let f = match$0.val; if (f == latched) { 2u8 } else { 3u8 } } else { 0u8 } }",
    ] {
        assert!(supervise.contains(line), "`{line}` in:\n{supervise}");
    }

    // `unwrap_or` of a literal `Some` is its payload; of a pair it is an `if` on the flag.
    assert_eq!(ir.expr(read.body_of("literal")), &Expr::Int(3));
    let known = read.function_text("read");
    assert!(known.contains("let k.is = x.is_known()") && known.contains("let k.val = x.known_or(0u8)") && known.contains("if k.is { k.val } else { 5u8 }"), "{known}");
}

#[test]
fn a_tuple_let_is_one_local_per_component() {
    // DIG-08 AC 11: `let (m, r) = (x, y);` is two locals, each typed by its own component.
    // `in_window` of the buck (examples/buck/src/supervisor.msl).
    let items = "pub fn in_window(meas: u10, reference: u10) -> bool {\n\
                   let (m, r) = (meas as u11, reference as u11);\n\
                   let half = r >> 4;\n\
                   m + half >= r && m <= r + half\n\
                 }\n\
                 fn mixed() -> u8 { let u: u8 = Unknown; let (a, b) = (u, 1u8); a.known_or(b) + b }";
    let ir = Elab::ir(&reaching(items, &["in_window(1, 2)", "mixed()"]), "Top");
    let read = Read { ir: &ir };
    let window = read.function_text("in_window");
    assert!(window.contains("let m = (meas as u11)\n  let r = (reference as u11)"), "{window}");
    let locals = locals_in(&window);
    assert!(locals.contains(&"m: u11".to_string()) && locals.contains(&"r: u11".to_string()), "{window}");
    let mixed = read.function_text("mixed");
    let locals = locals_in(&mixed);
    assert!(locals.contains(&"a: flagged<u8>".to_string()) && locals.contains(&"b: u8".to_string()), "{mixed}");
}
